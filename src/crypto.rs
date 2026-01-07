use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce, Key
};
use anyhow::{Result, anyhow};

/// Wrapper around ChaCha20Poly1305 AEAD.
/// 
/// WHY CHACHA20?
/// - It's faster than AES-GCM on mobile/ARM devices (no hardware AES requirement).
/// - It's constant-time (software implementation) preventing cache-timing attacks.
/// 
pub struct CryptoCipher {
    cipher: ChaCha20Poly1305,
}

impl CryptoCipher {
    /// Initialize the cipher state.
    /// FIXME: Currently using a static PSK. We should implement Noise_IK or 
    /// similar for ephemeral key exchange and Perfect Forward Secrecy (PFS).
    pub fn new(key_bytes: &[u8; 32]) -> Self {
        let key = Key::from_slice(key_bytes);
        let cipher = ChaCha20Poly1305::new(key);
        Self { cipher }
    }

    /// Encrypts data into a wire-ready packet.
    /// Format: [NONCE (12B) | CIPHERTEXT (N) | TAG (16B)]
    /// Note: The `encrypt` method of the crate appends the tag automatically.
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        // We generate a random nonce for every packet.
        // Impact: slight bandwidth overhead (12 bytes), but stateless and simple.
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng); 
        
        let ciphertext = self.cipher.encrypt(&nonce, data)
            .map_err(|e| anyhow!("Crypto::EncryptFail: {}", e))?;
        
        // Prepend nonce so the receiver can derive the keystream
        let mut packet = nonce.to_vec();
        packet.extend(ciphertext);
        
        Ok(packet)
    }

    /// Decrypts a wire packet.
    /// Expects: [NONCE (12B) | ...]
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            return Err(anyhow!("Crypto::ShortPacket: {} bytes", data.len()));
        }

        let nonce = Nonce::from_slice(&data[0..12]);
        let ciphertext = &data[12..];

        let plaintext = self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Crypto::DecryptFail: {}", e))?;

        Ok(plaintext)
    }
}
