use crate::sync::protocol::P2PMessage;
use bytes::BytesMut;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use rand::{thread_rng, RngCore};
use tokio_util::codec::{Decoder, Encoder};

// Length-delimited codec for Bincode-serialized P2PMessages.
// Format: [Length (u32, 4 bytes)][Payload]
// If Encrypted: Payload = [Nonce (12 bytes)][Ciphertext]
// If Plaintext: Payload = [Bincode Data]
// Max message size: 100 MB
const MAX_FRAME_SIZE: usize = 100 * 1024 * 1024;

#[derive(Default)]
pub struct P2PCodec {
    cipher: Option<ChaCha20Poly1305>,
}

impl P2PCodec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_key(&mut self, key_bytes: &[u8; 32]) {
        let key = Key::from(*key_bytes);
        self.cipher = Some(ChaCha20Poly1305::new(&key));
    }
}

impl Encoder<P2PMessage> for P2PCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: P2PMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let data = bincode::serialize(&item)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        if let Some(cipher) = &self.cipher {
            let mut nonce_bytes = [0u8; 12];
            thread_rng().fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from(nonce_bytes);

            let ciphertext = cipher
                .encrypt(&nonce, data.as_ref())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

            // Frame: [Length u32][Nonce][Ciphertext]
            let total_len = 12 + ciphertext.len();
            if total_len > MAX_FRAME_SIZE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Frame too large: {} bytes", total_len),
                ));
            }

            dst.reserve(4 + total_len);
            dst.extend_from_slice(&(total_len as u32).to_be_bytes());
            dst.extend_from_slice(&nonce_bytes);
            dst.extend_from_slice(&ciphertext);
        } else {
            // Plaintext
            if data.len() > MAX_FRAME_SIZE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Frame too large: {} bytes", data.len()),
                ));
            }

            dst.reserve(4 + data.len());
            dst.extend_from_slice(&(data.len() as u32).to_be_bytes());
            dst.extend_from_slice(&data);
        }

        Ok(())
    }
}

impl Decoder for P2PCodec {
    type Item = P2PMessage;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);
        let length = u32::from_be_bytes(length_bytes) as usize;

        if length > MAX_FRAME_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame too large: {} bytes", length),
            ));
        }

        if src.len() < 4 + length {
            src.reserve(4 + length - src.len());
            return Ok(None);
        }

        let _ = src.split_to(4); // Discard length
        let frame = src.split_to(length); // Take payload

        let payload = if let Some(cipher) = &self.cipher {
            if frame.len() < 12 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Frame too short for decryption",
                ));
            }
            let nonce_bytes = &frame[..12];
            let ciphertext = &frame[12..];
            let mut n_bytes = [0u8; 12];
            n_bytes.copy_from_slice(nonce_bytes);
            let nonce = Nonce::from(n_bytes);

            cipher.decrypt(&nonce, ciphertext).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Decrypt: {}", e))
            })?
        } else {
            frame.to_vec()
        };

        let item: P2PMessage = bincode::deserialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(Some(item))
    }
}
