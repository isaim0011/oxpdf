//! AES-128 and AES-256 CBC decryption per PDF specifications.
//!
//! Handles 16-byte random IV prefix and PKCS#7 unpadding using the `aes` and `cbc` crates.

use aes::{Aes128, Aes256};
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use crate::error::{Error, Result};

type Aes128CbcDec = cbc::Decryptor<Aes128>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;
type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes256CbcEnc = cbc::Encryptor<Aes256>;

/// Decrypts AES-128 CBC ciphertext prepended with a 16-byte IV and padded with PKCS#7.
///
/// In PDF encryption, the first 16 bytes of encrypted data are the random IV.
pub fn aes_128_cbc_decrypt(key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 16 {
        return Err(Error::Decryption(format!(
            "AES-128 requires 16-byte key, received {}",
            key.len()
        )));
    }
    if ciphertext.is_empty() {
        return Ok(Vec::new());
    }
    if ciphertext.len() < 16 || (ciphertext.len() - 16) % 16 != 0 {
        return Err(Error::Decryption(format!(
            "AES-128 ciphertext length {} is invalid (must be >= 16 and a multiple of 16)",
            ciphertext.len()
        )));
    }
    if ciphertext.len() == 16 {
        return Ok(Vec::new());
    }

    let iv: [u8; 16] = ciphertext[..16]
        .try_into()
        .map_err(|_| Error::Decryption("Failed to read 16-byte IV".into()))?;
    let mut buffer = ciphertext[16..].to_vec();

    let dec = Aes128CbcDec::new(key.into(), &iv.into());
    let decrypted = dec
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|e| Error::Decryption(format!("AES-128 PKCS#7 unpad failed: {e}")))?;

    Ok(decrypted.to_vec())
}

/// Decrypts AES-256 CBC ciphertext prepended with a 16-byte IV and padded with PKCS#7.
///
/// In PDF encryption, the first 16 bytes of encrypted data are the random IV.
pub fn aes_256_cbc_decrypt(key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 32 {
        return Err(Error::Decryption(format!(
            "AES-256 requires 32-byte key, received {}",
            key.len()
        )));
    }
    if ciphertext.is_empty() {
        return Ok(Vec::new());
    }
    if ciphertext.len() < 16 || (ciphertext.len() - 16) % 16 != 0 {
        return Err(Error::Decryption(format!(
            "AES-256 ciphertext length {} is invalid (must be >= 16 and a multiple of 16)",
            ciphertext.len()
        )));
    }
    if ciphertext.len() == 16 {
        return Ok(Vec::new());
    }

    let iv: [u8; 16] = ciphertext[..16]
        .try_into()
        .map_err(|_| Error::Decryption("Failed to read 16-byte IV".into()))?;
    let mut buffer = ciphertext[16..].to_vec();

    let dec = Aes256CbcDec::new(key.into(), &iv.into());
    let decrypted = dec
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|e| Error::Decryption(format!("AES-256 PKCS#7 unpad failed: {e}")))?;

    Ok(decrypted.to_vec())
}

/// Decrypts AES-256 CBC blocks without padding using an explicit IV (used for 32-byte UE / OE).
pub fn aes_256_cbc_decrypt_raw(key: &[u8], iv: &[u8; 16], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 32 {
        return Err(Error::Decryption(format!(
            "AES-256 requires 32-byte key, received {}",
            key.len()
        )));
    }
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err(Error::Decryption(format!(
            "Ciphertext length {} must be non-empty and a multiple of 16 bytes",
            ciphertext.len()
        )));
    }

    let mut buffer = ciphertext.to_vec();
    let mut dec = Aes256CbcDec::new(key.into(), iv.into());
    for block in buffer.chunks_exact_mut(16) {
        dec.decrypt_block_mut(block.into());
    }

    Ok(buffer)
}

/// Helper for encryption: generates 16-byte IV + PKCS#7 padded AES-128 CBC ciphertext.
pub fn aes_128_cbc_encrypt(key: &[u8], iv: &[u8; 16], plaintext: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 16 {
        return Err(Error::Decryption("Key must be 16 bytes".into()));
    }
    let pad_len = 16 - (plaintext.len() % 16);
    let mut buf = Vec::with_capacity(16 + plaintext.len() + pad_len);
    buf.extend_from_slice(iv);
    buf.extend_from_slice(plaintext);
    buf.resize(16 + plaintext.len() + pad_len, 0);

    let enc = Aes128CbcEnc::new(key.into(), iv.into());
    enc.encrypt_padded_mut::<Pkcs7>(&mut buf[16..], plaintext.len())
        .map_err(|e| Error::Decryption(format!("AES-128 pad error: {e}")))?;
    Ok(buf)
}

/// Helper for encryption: generates 16-byte IV + PKCS#7 padded AES-256 CBC ciphertext.
pub fn aes_256_cbc_encrypt(key: &[u8], iv: &[u8; 16], plaintext: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 32 {
        return Err(Error::Decryption("Key must be 32 bytes".into()));
    }
    let pad_len = 16 - (plaintext.len() % 16);
    let mut buf = Vec::with_capacity(16 + plaintext.len() + pad_len);
    buf.extend_from_slice(iv);
    buf.extend_from_slice(plaintext);
    buf.resize(16 + plaintext.len() + pad_len, 0);

    let enc = Aes256CbcEnc::new(key.into(), iv.into());
    enc.encrypt_padded_mut::<Pkcs7>(&mut buf[16..], plaintext.len())
        .map_err(|e| Error::Decryption(format!("AES-256 pad error: {e}")))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes_128_cbc_roundtrip() {
        let key = b"0123456789abcdef"; // 16 bytes
        let iv = [0x55u8; 16];
        let message = b"Hello, PDF AES-128 encryption!";

        let encrypted = aes_128_cbc_encrypt(key, &iv, message).expect("encrypt failed");
        assert_eq!(&encrypted[..16], &iv);
        assert_eq!(encrypted.len() % 16, 0);

        let decrypted = aes_128_cbc_decrypt(key, &encrypted).expect("decrypt failed");
        assert_eq!(decrypted, message);
    }

    #[test]
    fn test_aes_256_cbc_roundtrip() {
        let key = b"0123456789abcdef0123456789abcdef"; // 32 bytes
        let iv = [0xAAu8; 16];
        let message = b"Confidential document stream content under ISO 32000-2 AES-256";

        let encrypted = aes_256_cbc_encrypt(key, &iv, message).expect("encrypt failed");
        assert_eq!(&encrypted[..16], &iv);
        assert_eq!(encrypted.len() % 16, 0);

        let decrypted = aes_256_cbc_decrypt(key, &encrypted).expect("decrypt failed");
        assert_eq!(decrypted, message);
    }

    #[test]
    fn test_aes_invalid_key_or_length() {
        let bad_key = b"short_key";
        let ct = [0u8; 32];
        assert!(aes_128_cbc_decrypt(bad_key, &ct).is_err());
        assert!(aes_256_cbc_decrypt(bad_key, &ct).is_err());

        let key128 = [0u8; 16];
        let unaligned = [0u8; 25]; // Not multiple of 16
        assert!(aes_128_cbc_decrypt(&key128, &unaligned).is_err());
    }

    #[test]
    fn test_aes_256_cbc_raw_roundtrip() {
        let key = [0x42u8; 32];
        let iv = [0x00u8; 16];
        let raw_dek = [0x77u8; 32]; // 32 bytes exact (2 blocks)

        // Raw CBC encrypt
        let mut encrypted = raw_dek.to_vec();
        let mut enc = Aes256CbcEnc::new((&key).into(), (&iv).into());
        for block in encrypted.chunks_exact_mut(16) {
            enc.encrypt_block_mut(block.into());
        }

        // Raw CBC decrypt
        let decrypted = aes_256_cbc_decrypt_raw(&key, &iv, &encrypted).expect("raw decrypt failed");
        assert_eq!(decrypted, raw_dek);
    }
}
