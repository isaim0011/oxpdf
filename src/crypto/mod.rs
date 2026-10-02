//! Standard PDF Security and Cryptography Engine (`oxpdf-crypto`).
//!
//! Provides pure-Rust stream/block ciphers (RC4, AES-128, AES-256) and the
//! standard security handler for Adobe/ISO PDF revisions R2 through R6.

pub mod aes;
pub mod handler;
pub mod rc4;

pub use aes::{
    aes_128_cbc_decrypt, aes_128_cbc_encrypt, aes_256_cbc_decrypt, aes_256_cbc_decrypt_raw,
    aes_256_cbc_encrypt,
};
pub use handler::{CipherAlgorithm, EncryptionParams, StandardSecurityHandler, PAD_BYTES};
pub use rc4::Rc4;
