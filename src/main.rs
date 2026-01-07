use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use anyhow::{Context, Result};
use tokio::net::UdpSocket;
use tun::Configuration;
use parking_lot::Mutex;
use std::sync::mpsc; // Sync channel for TUI interaction

// Internal Modules
mod protocol;
mod crypto;
mod compression;
mod tui;
mod obfuscation;

use protocol::{WireFrame, FrameType};
use tui::TelemetryUpdate;
use tokio::io::{AsyncReadExt, AsyncWriteExt}; 
use tokio::time::{sleep, Duration}; 

/// The maximum transmission unit.
/// TODO: Implement Path MTU Discovery (PMTUD) instead of hardcoding.
const MTU: usize = 1280;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
struct TunnelOptions {
    /// Interface bind address (e.g., 0.0.0.0:8000)
    #[arg(long)] bind: String,
    
    /// Initial peer address to connect to (optional)
    #[arg(long)] peer: Option<String>,
    
    /// Virtual IP for the TUN interface
    #[arg(long, default_value = "10.0.0.1")] tun_ip: String,
    
    /// Pre-shared key (32 bytes hex). 
    /// FIXME: Replace with ephemeral key exchange (Noise Protocol).
    #[arg(long, default_value = "0000000000000000000000000000000000000000000000000000000000000000")] key: String,
    
    /// Enable chaos mode (simulated packet loss)
    #[arg(long)] chaos: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = TunnelOptions::parse();

    // Telemetry Channel -> TUI Thread
    let (stats_tx, stats_rx) = mpsc::channel::<TelemetryUpdate>();
    let tui_handle = tui::spawn_dashboard(stats_rx);

    // Crypto Setup
    let key_bytes = hex::decode(&opts.key).context("Found malformed hex key")?;
    let key_arr: [u8; 32] = key_bytes.try_into().map_err(|_| anyhow::anyhow!("Key must be exactly 32 bytes"))?;
    
    // We share the cipher primitive across threads. 
    // Arc<T> is cheap here, and ChaCha state is immutable until encryption.
    let cipher_enc = Arc::new(crypto::CryptoCipher::new(&key_arr));
    let cipher_dec = cipher_enc.clone();

    // TUN Interface Setup
    // We use a small MTU to avoid fragmentation issues over UDP overlays.
    let mut config = Configuration::default();
    config.address(opts.tun_ip.parse::<std::net::Ipv4Addr>()?)
          .destination(opts.tun_ip.parse::<std::net::Ipv4Addr>()?)
          .netmask((255, 255, 255, 0))
          .mtu(MTU as i32)
          .up();
    
    #[cfg(target_os = "linux")]
    config.platform(|c| { c.packet_information(true); });

    let tun_dev = tun::create_as_async(&config).context("Failed to open TUN device. Do you have root privileges?")?;
    let (mut tun_reader, mut tun_writer) = tokio::io::split(tun_dev);

    // UDP Socket Setup
    let socket = UdpSocket::bind(&opts.bind).await.context("Failed to bind UDP socket")?;
    let socket = Arc::new(socket);
    
    // Pre-flight: Send random junk to punch NAT or confuse DPI before real handshake.
    if let Some(peer_str) = &opts.peer {
        let fake_hello = obfuscation::mimic_tls_client_hello();
        if let Ok(addr) = peer_str.parse::<SocketAddr>() {
             let _ = socket.send_to(&fake_hello, addr).await;
             let _ = stats_tx.send(TelemetryUpdate::Log("OBSF: Deployed fake TLS ClientHello".to_string()));
        }
    }

    let initial_peer: Option<SocketAddr> = opts.peer.as_deref().map(|p| p.parse()).transpose()?;
    let active_peer = Arc::new(Mutex::new(initial_peer));
    
    // Sequence number for basic replay protection (monotonic counter)
    let tx_seq = Arc::new(AtomicU64::new(1));

    // ----------------------------------------------------------------
    // TX LOOP: TUN Interface -> UDP Socket
    // Reads IP packets, compresses, encrypts, and blasts them over UDP.
    // ----------------------------------------------------------------
    let socket_tx = socket.clone();
    let peer_tx = active_peer.clone();
    let stats_tx_1 = stats_tx.clone();
    
    let _tx_task = tokio::spawn(async move {
        let mut frame_buffer = [0u8; 4096]; // Oversized buffer for safety
        loop {
            match tun_reader.read(&mut frame_buffer).await {
                Ok(n) if n > 0 => {
                    let target = *peer_tx.lock();
                    if let Some(remote_addr) = target {
                        let ip_packet = &frame_buffer[..n];
                        
                        // Introduce jitter to mitigate timing analysis correlation
                        obfuscation::jitter_sleep().await;

                        // Pipeline: Compress -> Encrypt -> Wrap
                        let processed = compression::adaptive_compress(ip_packet).unwrap_or(ip_packet.to_vec());
                        let encrypted = cipher_enc.encrypt(&processed).unwrap();
                        
                        let seq = tx_seq.fetch_add(1, Ordering::Relaxed);
                        let frame = WireFrame::new_data(seq, encrypted);
                        
                        // Serialization (Bincode is fast, but we might want Protobuf later for schema evolution)
                        let encoded = bincode::serialize(&frame).unwrap();

                        if let Err(e) = socket_tx.send_to(&encoded, remote_addr).await {
                             let _ = stats_tx_1.send(TelemetryUpdate::Log(format!("UDP::SendErr: {}", e)));
                        } else {
                             let _ = stats_tx_1.send(TelemetryUpdate::Throughput { 
                                 tx_bytes: n as u64, 
                                 rx_bytes: 0 
                             });
                        }
                    }
                }
                Ok(_) => break, // EOF from TUN usually means interface went down
                Err(e) => {
                    let _ = stats_tx_1.send(TelemetryUpdate::Log(format!("TUN::ReadErr: {}", e)));
                    // Cool-down to prevent CPU spin loop on device errors
                    sleep(Duration::from_millis(10)).await;
                    break;
                }
            }
        }
    });

    // ----------------------------------------------------------------
    // RX LOOP: UDP Socket -> TUN Interface
    // Listens for encrypted frames, validates, decrypts, writes to kernel.
    // ----------------------------------------------------------------
    let socket_rx = socket.clone();
    let peer_rx = active_peer.clone();
    let stats_tx_2 = stats_tx.clone();

    let _rx_task = tokio::spawn(async move {
        let mut udp_buffer = [0u8; 65535]; // Max UDP size
        loop {
            match socket_rx.recv_from(&mut udp_buffer).await {
                Ok((size, src_addr)) => {
                    // "Roam" the peer address (Mobility support)
                    // If we receive a valid packet from a new IP, update our target.
                    {
                        let mut lock = peer_rx.lock();
                        if lock.is_none() || *lock != Some(src_addr) {
                             *lock = Some(src_addr);
                             let _ = stats_tx_2.send(TelemetryUpdate::Log(format!("NET: Peer roamed to {}", src_addr)));
                        }
                    }

                    // Deserialize & Unwrap
                    if let Ok(frame) = bincode::deserialize::<WireFrame>(&udp_buffer[..size]) {
                        match frame.header.frame_type {
                            FrameType::Transport => {
                                if let Ok(decrypted) = cipher_dec.decrypt(&frame.payload) {
                                    // If decryption passes, we trust the logic (Authenticated Encryption)
                                    if let Ok(decompressed) = compression::adaptive_decompress(&decrypted) {
                                        if tun_writer.write_all(&decompressed).await.is_ok() {
                                            let _ = stats_tx_2.send(TelemetryUpdate::Throughput { 
                                                tx_bytes: 0, 
                                                rx_bytes: size as u64 
                                            });
                                        }
                                    }
                                }
                                // Note: Silently drop decryption failures (prevent oracle attacks)
                            },
                            _ => {} // Ignore heartbeats/handshakes for now
                        }
                    }
                },
                Err(e) => {
                    let _ = stats_tx_2.send(TelemetryUpdate::Log(format!("UDP::RecvErr: {}", e)));
                    sleep(Duration::from_millis(10)).await;
                }
            }
        }
    });

    // Keep main thread alive until TUI exits
    let _ = tui_handle.join();
    Ok(())
}
