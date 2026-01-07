use tokio::time::{sleep, Duration};
use rand::Rng;

/// Introduces random jitter to packet transmission.
/// 
/// WHY?
/// Traffic Analysis attacks look at inter-arrival times.
/// Perfect 50ms intervals = Bot/Machine.
/// Random intervals = Human/Noise.
pub async fn jitter_sleep() {
    let micros = {
        let mut rng = rand::thread_rng();
        // 0-15ms jitter is enough to blur distinct signatures 
        // without destroying VoIP quality (usually <30ms jitter budget).
        rng.gen_range(0..15_000)
    };
    
    if micros > 0 {
        sleep(Duration::from_micros(micros)).await;
    }
}

/// Generates a fake TLS "Client Hello" struct.
/// 
/// PURPOSE:
/// Deep Packet Inspection (DPI) often blocks "unknown UDP".
/// By mimicking the start of a TLS handshake (0x16 0x03 0x01 ...),
/// we can trick basic firewalls into classifying this as QUIC or DTLS.
pub fn mimic_tls_client_hello() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut packet = vec![
        0x16,       // ContentType: Handshake
        0x03, 0x01  // Version: TLS 1.0 (Legacy compatibility)
    ];
    
    // Random Length (Make it look variable)
    let len: u16 = rng.gen_range(85..300);
    packet.extend_from_slice(&len.to_be_bytes());

    // Fill with high-entropy garbage (Random Random)
    // Real TLS ClientHello has structure, but for a "first glance" filter,
    // this often passes.
    let mut garbage = vec![0u8; len as usize];
    rng.fill(&mut garbage[..]);
    packet.extend(garbage);
    
    packet
}
