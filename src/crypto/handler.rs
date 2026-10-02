//! Standard Security Handler (oxpdf-crypto) per ISO 32000-1 / ISO 32000-2 §7.6.
//!
//! Supports password validation, Document Encryption Key (DEK) derivation,
//! per-object key computation, and stream/string decryption for revisions R2 through R6.

use aes::cipher::{BlockEncryptMut, KeyIvInit};
use md5::{Digest as _, Md5};
use sha2::{Sha256, Sha384, Sha512};
use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::crypto::aes::{aes_128_cbc_decrypt, aes_256_cbc_decrypt, aes_256_cbc_decrypt_raw};
use crate::crypto::rc4::Rc4;
use crate::error::{Error, Result};
use crate::types::Object;

/// Standard 32-byte padding sequence defined in PDF 1.7 Algorithm 3.2.
pub const PAD_BYTES: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

/// Supported cipher algorithms for stream and string decryption.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CipherAlgorithm {
    None,
    Rc4 { key_len_bytes: usize },
    Aes128,
    Aes256,
}

/// Parameters extracted from the PDF `/Encrypt` dictionary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptionParams {
    pub version: u32,
    pub revision: u32,
    pub key_length_bytes: usize,
    pub stream_cipher: CipherAlgorithm,
    pub string_cipher: CipherAlgorithm,
    pub encrypt_metadata: bool,
    pub permissions: i32,
}

/// Standard Security Handler for PDF encryption (V1-V5, R2-R6).
#[derive(Clone, Debug)]
pub struct StandardSecurityHandler {
    params: EncryptionParams,
    dek: Vec<u8>,
}

impl StandardSecurityHandler {
    /// Constructs a new security handler directly with parsed params and DEK.
    pub fn new(params: EncryptionParams, dek: Vec<u8>) -> Self {
        Self { params, dek }
    }

    /// Access the parsed encryption parameters.
    pub fn params(&self) -> &EncryptionParams {
        &self.params
    }

    /// Whether document metadata streams should be encrypted.
    pub fn encrypt_metadata(&self) -> bool {
        self.params.encrypt_metadata
    }

    /// Access the Document Encryption Key (DEK).
    pub fn dek(&self) -> &[u8] {
        &self.dek
    }

    /// Parses the `/Encrypt` dictionary, validates the user or owner password,
    /// and derives the Document Encryption Key (DEK).
    pub fn from_encrypt_dict(
        dict: &BTreeMap<Cow<'_, str>, Object<'_>>,
        trailer_id_0: Option<&[u8]>,
        password: &[u8],
    ) -> Result<Self> {
        let filter = dict.get("Filter").and_then(|f| f.as_name()).unwrap_or("");
        if !filter.is_empty() && filter != "Standard" {
            return Err(Error::Unsupported("Non-standard PDF security handler"));
        }

        let version = match dict.get("V") {
            Some(Object::Integer(v)) if *v >= 0 => *v as u32,
            _ => 0,
        };

        let revision = match dict.get("R") {
            Some(Object::Integer(r)) if *r >= 2 => *r as u32,
            _ => {
                return Err(Error::Decryption(
                    "Missing or invalid /R revision in /Encrypt dictionary".into(),
                ))
            }
        };

        let o_bytes = match dict.get("O") {
            Some(Object::String(bytes)) => bytes.as_slice(),
            _ => {
                return Err(Error::Decryption(
                    "Missing or invalid /O entry in /Encrypt dictionary".into(),
                ))
            }
        };

        let u_bytes = match dict.get("U") {
            Some(Object::String(bytes)) => bytes.as_slice(),
            _ => {
                return Err(Error::Decryption(
                    "Missing or invalid /U entry in /Encrypt dictionary".into(),
                ))
            }
        };

        let permissions = match dict.get("P") {
            Some(Object::Integer(p)) => *p as i32,
            _ => {
                return Err(Error::Decryption(
                    "Missing or invalid /P entry in /Encrypt dictionary".into(),
                ))
            }
        };

        let length_bits = match dict.get("Length") {
            Some(Object::Integer(l)) if *l > 0 => *l as usize,
            _ => {
                if version == 1 {
                    40
                } else if version == 5 {
                    256
                } else {
                    128
                }
            }
        };

        if !(40..=256).contains(&length_bits) || length_bits % 8 != 0 {
            return Err(Error::Decryption(format!(
                "Invalid /Length in /Encrypt dictionary: {} bits",
                length_bits
            )));
        }

        let key_length_bytes = length_bits / 8;
        if (revision == 2 || revision == 3) && key_length_bytes > 16 {
            return Err(Error::Decryption(format!(
                "Invalid key length for R={}: {} bytes (max 16)",
                revision, key_length_bytes
            )));
        }

        let encrypt_metadata = match dict.get("EncryptMetadata") {
            Some(Object::Boolean(b)) => *b,
            _ => true,
        };

        let (stream_cipher, string_cipher) =
            Self::resolve_ciphers(version, key_length_bytes, dict)?;

        let id_0 = trailer_id_0.unwrap_or(b"");

        // Authenticate password and derive DEK
        let dek = match revision {
            2 => Self::auth_r2(password, o_bytes, u_bytes, permissions, id_0)?,
            3 | 4 => Self::auth_r3_r4(
                password,
                o_bytes,
                u_bytes,
                permissions,
                id_0,
                revision,
                key_length_bytes,
                encrypt_metadata,
            )?,
            5 => {
                let oe_bytes = dict.get("OE").and_then(|o| match o {
                    Object::String(b) => Some(b.as_slice()),
                    _ => None,
                });
                let ue_bytes = dict.get("UE").and_then(|u| match u {
                    Object::String(b) => Some(b.as_slice()),
                    _ => None,
                });
                Self::auth_r5(password, o_bytes, u_bytes, oe_bytes, ue_bytes)?
            }
            6 => {
                let oe_bytes = dict.get("OE").and_then(|o| match o {
                    Object::String(b) => Some(b.as_slice()),
                    _ => None,
                });
                let ue_bytes = dict.get("UE").and_then(|u| match u {
                    Object::String(b) => Some(b.as_slice()),
                    _ => None,
                });
                Self::auth_r6(password, o_bytes, u_bytes, oe_bytes, ue_bytes)?
            }
            rev => {
                return Err(Error::Decryption(format!(
                    "Unsupported revision R={rev} in security handler"
                )))
            }
        };

        let params = EncryptionParams {
            version,
            revision,
            key_length_bytes,
            stream_cipher,
            string_cipher,
            encrypt_metadata,
            permissions,
        };

        Ok(Self { params, dek })
    }

    /// Resolves stream and string cipher algorithms from `/V`, `/CF`, `/StmF`, and `/StrF`.
    fn resolve_ciphers(
        version: u32,
        key_length_bytes: usize,
        dict: &BTreeMap<Cow<'_, str>, Object<'_>>,
    ) -> Result<(CipherAlgorithm, CipherAlgorithm)> {
        match version {
            1 | 2 => {
                let cipher = CipherAlgorithm::Rc4 {
                    key_len_bytes: key_length_bytes,
                };
                Ok((cipher, cipher))
            }
            4 => {
                let parse_cf = |name_opt: Option<&Object<'_>>| -> Result<CipherAlgorithm> {
                    let cf_name = name_opt.and_then(|o| o.as_name()).unwrap_or("Identity");
                    if cf_name == "Identity" {
                        return Ok(CipherAlgorithm::None);
                    }
                    if let Some(Object::Dictionary(cf_dict)) = dict.get("CF") {
                        if let Some(Object::Dictionary(target_cf)) = cf_dict.get(cf_name) {
                            let cfm = target_cf.get("CFM").and_then(|c| c.as_name()).unwrap_or("");
                            return match cfm {
                                "None" => Ok(CipherAlgorithm::None),
                                "V2" => Ok(CipherAlgorithm::Rc4 {
                                    key_len_bytes: key_length_bytes,
                                }),
                                "AESV2" => Ok(CipherAlgorithm::Aes128),
                                "AESV3" => Ok(CipherAlgorithm::Aes256),
                                other => Err(Error::Unsupported(match other {
                                    "V1" => "CFM V1",
                                    _ => "Unknown CFM in CryptFilter",
                                })),
                            };
                        }
                    }
                    // Default fallback if CF dictionary is omitted or standard
                    if cf_name == "StdCF" {
                        Ok(CipherAlgorithm::Aes128)
                    } else {
                        Ok(CipherAlgorithm::None)
                    }
                };

                let stm = parse_cf(dict.get("StmF"))?;
                let str_ = parse_cf(dict.get("StrF"))?;
                Ok((stm, str_))
            }
            5 => Ok((CipherAlgorithm::Aes256, CipherAlgorithm::Aes256)),
            _ => Err(Error::Decryption(format!(
                "Unsupported V={version} in /Encrypt"
            ))),
        }
    }

    /// Computes the per-object encryption key: MD5(DEK || obj_num[0..2] || gen_num[0..1] [|| b"sAlT"]).
    pub fn compute_object_key(&self, obj_id: u32, gen: u16, is_aes: bool) -> Vec<u8> {
        let mut hasher = Md5::new();
        hasher.update(&self.dek);
        hasher.update(&obj_id.to_le_bytes()[..3]);
        hasher.update(&gen.to_le_bytes()[..2]);
        if is_aes {
            hasher.update(b"sAlT");
        }
        let digest = hasher.finalize();
        let key_len = if is_aes {
            16
        } else {
            std::cmp::min(self.dek.len() + 5, 16)
        };
        digest[..key_len].to_vec()
    }

    /// Decrypts a stream payload using the document's stream cipher.
    pub fn decrypt_stream(&self, obj_id: u32, gen: u16, raw: &[u8]) -> Result<Vec<u8>> {
        self.decrypt_data(obj_id, gen, raw, self.params.stream_cipher)
    }

    /// Decrypts a string slice using the document's string cipher.
    pub fn decrypt_string(&self, obj_id: u32, gen: u16, raw: &[u8]) -> Result<Vec<u8>> {
        self.decrypt_data(obj_id, gen, raw, self.params.string_cipher)
    }

    fn decrypt_data(
        &self,
        obj_id: u32,
        gen: u16,
        raw: &[u8],
        cipher: CipherAlgorithm,
    ) -> Result<Vec<u8>> {
        match cipher {
            CipherAlgorithm::None => Ok(raw.to_vec()),
            CipherAlgorithm::Rc4 { .. } => {
                if raw.is_empty() {
                    return Ok(Vec::new());
                }
                let key = self.compute_object_key(obj_id, gen, false);
                Ok(Rc4::new(&key).decrypt(raw))
            }
            CipherAlgorithm::Aes128 => {
                if raw.is_empty() {
                    return Ok(Vec::new());
                }
                let key = self.compute_object_key(obj_id, gen, true);
                aes_128_cbc_decrypt(&key, raw)
            }
            CipherAlgorithm::Aes256 => {
                if raw.is_empty() {
                    return Ok(Vec::new());
                }
                aes_256_cbc_decrypt(&self.dek, raw)
            }
        }
    }

    // =========================================================================
    // Password validation and DEK derivation implementations (R2 to R6)
    // =========================================================================

    fn pad_password(password: &[u8]) -> [u8; 32] {
        let mut padded = [0u8; 32];
        let len = password.len().min(32);
        padded[..len].copy_from_slice(&password[..len]);
        padded[len..].copy_from_slice(&PAD_BYTES[..32 - len]);
        padded
    }

    /// Derives the encryption key for R2, R3, R4 per Algorithm 3.2.
    pub fn derive_key_r2_r4(
        password: &[u8],
        o_entry: &[u8],
        permissions: i32,
        file_id_0: &[u8],
        revision: u32,
        key_len: usize,
        encrypt_metadata: bool,
    ) -> Vec<u8> {
        let padded = Self::pad_password(password);

        let mut hasher = Md5::new();
        hasher.update(padded);
        hasher.update(o_entry);
        hasher.update((permissions as u32).to_le_bytes());
        hasher.update(file_id_0);

        if revision >= 4 && !encrypt_metadata {
            hasher.update(b"\xff\xff\xff\xff");
        }

        let mut hash = hasher.finalize();

        if revision >= 3 {
            for _ in 0..50 {
                let mut next = Md5::new();
                next.update(&hash[..key_len]);
                hash = next.finalize();
            }
        }

        hash[..key_len].to_vec()
    }

    /// Authenticates Revision 2 (40-bit RC4).
    fn auth_r2(
        password: &[u8],
        o_entry: &[u8],
        u_entry: &[u8],
        permissions: i32,
        file_id_0: &[u8],
    ) -> Result<Vec<u8>> {
        if o_entry.len() < 32 || u_entry.len() < 32 {
            return Err(Error::Decryption("Invalid /O or /U length for R=2".into()));
        }

        // Try user password
        let user_dek =
            Self::derive_key_r2_r4(password, o_entry, permissions, file_id_0, 2, 5, true);
        let test_u = Rc4::new(&user_dek).encrypt(&PAD_BYTES);
        if test_u == u_entry[..32] {
            return Ok(user_dek);
        }

        // Try owner password
        let padded_owner = Self::pad_password(password);
        let owner_key = &Md5::digest(padded_owner)[..5];
        let user_pw = Rc4::new(owner_key).decrypt(&o_entry[..32]);
        let dek = Self::derive_key_r2_r4(&user_pw, o_entry, permissions, file_id_0, 2, 5, true);
        let test_u = Rc4::new(&dek).encrypt(&PAD_BYTES);
        if test_u == u_entry[..32] {
            return Ok(dek);
        }

        Err(Error::Decryption("Invalid password for R=2 PDF".into()))
    }

    /// Authenticates Revision 3 and Revision 4 (128-bit RC4 / AES-128).
    #[allow(clippy::too_many_arguments)]
    fn auth_r3_r4(
        password: &[u8],
        o_entry: &[u8],
        u_entry: &[u8],
        permissions: i32,
        file_id_0: &[u8],
        revision: u32,
        key_len: usize,
        encrypt_metadata: bool,
    ) -> Result<Vec<u8>> {
        if o_entry.len() < 32 || u_entry.len() < 16 {
            return Err(Error::Decryption(
                "Invalid /O or /U length for R=3/4".into(),
            ));
        }

        // Helper to test if a derived DEK satisfies Algorithm 3.5 against /U
        let check_user_dek = |dek: &[u8]| -> bool {
            let mut hasher = Md5::new();
            hasher.update(PAD_BYTES);
            hasher.update(file_id_0);
            let hash = hasher.finalize();

            let mut res = Rc4::new(dek).encrypt(&hash);
            let mut key = vec![0u8; dek.len()];
            for i in 1..=19 {
                for (in_b, out_b) in dek.iter().zip(key.iter_mut()) {
                    *out_b = *in_b ^ i;
                }
                res = Rc4::new(&key).encrypt(&res);
            }
            res[..16] == u_entry[..16]
        };

        // 1. Try as user password
        let user_dek = Self::derive_key_r2_r4(
            password,
            o_entry,
            permissions,
            file_id_0,
            revision,
            key_len,
            encrypt_metadata,
        );
        if check_user_dek(&user_dek) {
            return Ok(user_dek);
        }

        // 2. Try as owner password
        let padded_owner = Self::pad_password(password);
        let mut hash = Md5::digest(padded_owner);
        for _ in 0..50 {
            hash = Md5::digest(hash);
        }
        let owner_key = &hash[..key_len];

        let mut res = o_entry[..32].to_vec();
        let mut key = vec![0u8; key_len];
        for i in (1..=19).rev() {
            for (in_b, out_b) in owner_key.iter().zip(key.iter_mut()) {
                *out_b = *in_b ^ i;
            }
            res = Rc4::new(&key).decrypt(&res);
        }
        let user_pw = Rc4::new(owner_key).decrypt(&res);

        let owner_derived_dek = Self::derive_key_r2_r4(
            &user_pw,
            o_entry,
            permissions,
            file_id_0,
            revision,
            key_len,
            encrypt_metadata,
        );
        if check_user_dek(&owner_derived_dek) {
            return Ok(owner_derived_dek);
        }

        Err(Error::Decryption("Invalid password for R=3/4 PDF".into()))
    }

    /// Authenticates Revision 5 (Adobe Extension Level 3 AES-256).
    fn auth_r5(
        password: &[u8],
        o_entry: &[u8],
        u_entry: &[u8],
        oe_bytes: Option<&[u8]>,
        ue_bytes: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        if o_entry.len() < 48 || u_entry.len() < 48 {
            return Err(Error::Decryption(
                "R=5 requires at least 48 bytes for /O and /U".into(),
            ));
        }
        let ue = ue_bytes.ok_or_else(|| Error::Decryption("Missing /UE in R=5".into()))?;
        let oe = oe_bytes.ok_or_else(|| Error::Decryption("Missing /OE in R=5".into()))?;

        // Truncate UTF-8 password to 127 bytes
        let pw = if password.len() > 127 {
            &password[..127]
        } else {
            password
        };

        let zero_iv = [0u8; 16];

        // 1. Try user password
        let user_val_salt = &u_entry[32..40];
        let mut hasher = Sha256::new();
        hasher.update(pw);
        hasher.update(user_val_salt);
        if hasher.finalize().as_slice() == &u_entry[..32] {
            let user_key_salt = &u_entry[40..48];
            let mut k_hasher = Sha256::new();
            k_hasher.update(pw);
            k_hasher.update(user_key_salt);
            let user_key = k_hasher.finalize();
            return aes_256_cbc_decrypt_raw(&user_key, &zero_iv, ue);
        }

        // 2. Try owner password
        let owner_val_salt = &o_entry[32..40];
        let mut hasher = Sha256::new();
        hasher.update(pw);
        hasher.update(owner_val_salt);
        hasher.update(&u_entry[..48]);
        if hasher.finalize().as_slice() == &o_entry[..32] {
            let owner_key_salt = &o_entry[40..48];
            let mut k_hasher = Sha256::new();
            k_hasher.update(pw);
            k_hasher.update(owner_key_salt);
            k_hasher.update(&u_entry[..48]);
            let owner_key = k_hasher.finalize();
            return aes_256_cbc_decrypt_raw(&owner_key, &zero_iv, oe);
        }

        Err(Error::Decryption("Invalid password for R=5 PDF".into()))
    }

    /// Authenticates Revision 6 (ISO 32000-2 AES-256 with 100,000 SHA-2 iterations).
    fn auth_r6(
        password: &[u8],
        o_entry: &[u8],
        u_entry: &[u8],
        oe_bytes: Option<&[u8]>,
        ue_bytes: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        if o_entry.len() < 48 || u_entry.len() < 48 {
            return Err(Error::Decryption(
                "R=6 requires at least 48 bytes for /O and /U".into(),
            ));
        }
        let ue = ue_bytes.ok_or_else(|| Error::Decryption("Missing /UE in R=6".into()))?;
        let oe = oe_bytes.ok_or_else(|| Error::Decryption("Missing /OE in R=6".into()))?;

        let zero_iv = [0u8; 16];

        // 1. Try owner password
        let owner_val_salt = &o_entry[32..40];
        let owner_hash = Self::compute_hash_r6(password, owner_val_salt, Some(&u_entry[..48]));
        if owner_hash == o_entry[..32] {
            let owner_key_salt = &o_entry[40..48];
            let owner_key = Self::compute_hash_r6(password, owner_key_salt, Some(&u_entry[..48]));
            return aes_256_cbc_decrypt_raw(&owner_key, &zero_iv, oe);
        }

        // 2. Try user password
        let user_val_salt = &u_entry[32..40];
        let user_hash = Self::compute_hash_r6(password, user_val_salt, None);
        if user_hash == u_entry[..32] {
            let user_key_salt = &u_entry[40..48];
            let user_key = Self::compute_hash_r6(password, user_key_salt, None);
            return aes_256_cbc_decrypt_raw(&user_key, &zero_iv, ue);
        }

        Err(Error::Decryption("Invalid password for R=6 PDF".into()))
    }

    /// Computes hash for Revision 6 per ISO 32000-2:2020 Algorithm 2.B.
    pub fn compute_hash_r6(password: &[u8], salt: &[u8], u_entry: Option<&[u8]>) -> [u8; 32] {
        let pw = if password.len() > 127 {
            &password[..127]
        } else {
            password
        };

        let mut hasher = Sha256::new();
        hasher.update(pw);
        hasher.update(salt);
        if let Some(u) = u_entry {
            hasher.update(u);
        }
        let mut k = hasher.finalize().to_vec();

        let mut k1 = Vec::with_capacity(64 * (pw.len() + 64 + u_entry.map_or(0, |u| u.len())));

        for round in 1.. {
            k1.clear();
            for _ in 0..64 {
                k1.extend_from_slice(pw);
                k1.extend_from_slice(&k);
                if let Some(u) = u_entry {
                    k1.extend_from_slice(u);
                }
            }

            let key = &k[0..16];
            let iv = &k[16..32];
            let mut encryptor = cbc::Encryptor::<aes::Aes128>::new(key.into(), iv.into());
            for block in k1.chunks_exact_mut(16) {
                encryptor.encrypt_block_mut(block.into());
            }

            // Remainder modulo 3 of first 16 bytes as big-endian int (sum % 3 == big_endian % 3)
            let sum: u32 = k1[..16].iter().map(|&b| b as u32).sum();
            k = match sum % 3 {
                0 => Sha256::digest(&k1).to_vec(),
                1 => Sha384::digest(&k1).to_vec(),
                2 => Sha512::digest(&k1).to_vec(),
                _ => unreachable!(),
            };

            if round >= 64 {
                let last_byte = *k1.last().unwrap_or(&0) as u32;
                if last_byte <= round - 32 {
                    break;
                }
            }
        }

        let mut out = [0u8; 32];
        out.copy_from_slice(&k[..32]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::SmallVec;

    #[test]
    fn test_per_object_key_derivation() {
        let params = EncryptionParams {
            version: 2,
            revision: 3,
            key_length_bytes: 16,
            stream_cipher: CipherAlgorithm::Rc4 { key_len_bytes: 16 },
            string_cipher: CipherAlgorithm::Rc4 { key_len_bytes: 16 },
            encrypt_metadata: true,
            permissions: -4,
        };
        let dek = vec![
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];
        let handler = StandardSecurityHandler::new(params, dek.clone());

        // Test RC4 per-object key (obj 1, gen 0)
        let key_rc4 = handler.compute_object_key(1, 0, false);
        assert_eq!(key_rc4.len(), 16);

        // Test AES per-object key (includes "sAlT")
        let key_aes = handler.compute_object_key(1, 0, true);
        assert_eq!(key_aes.len(), 16);
        assert_ne!(key_rc4, key_aes);
    }

    #[test]
    fn test_handler_r2_encryption_and_decryption_cycle() {
        // Construct R2 test encrypt dictionary
        let password = b"user123";
        let file_id_0 = b"file-identifier-16b";
        let permissions = -64i32;

        // Create dummy O and U using Algorithm 3.2
        let padded = StandardSecurityHandler::pad_password(password);
        let mut hasher = Md5::new();
        hasher.update(padded);
        hasher.update([0u8; 32]);
        hasher.update((permissions as u32).to_le_bytes());
        hasher.update(file_id_0);
        let dek = hasher.finalize()[..5].to_vec();

        let u_val = Rc4::new(&dek).encrypt(&PAD_BYTES);

        let mut dict = BTreeMap::new();
        dict.insert(
            Cow::Borrowed("Filter"),
            Object::Name(Cow::Borrowed("Standard")),
        );
        dict.insert(Cow::Borrowed("V"), Object::Integer(1));
        dict.insert(Cow::Borrowed("R"), Object::Integer(2));
        dict.insert(
            Cow::Borrowed("O"),
            Object::String(SmallVec::from_slice(&[0u8; 32])),
        );
        dict.insert(
            Cow::Borrowed("U"),
            Object::String(SmallVec::from_slice(&u_val)),
        );
        dict.insert(Cow::Borrowed("P"), Object::Integer(permissions as i64));

        let handler = StandardSecurityHandler::from_encrypt_dict(&dict, Some(file_id_0), password)
            .expect("R2 auth failed");
        assert_eq!(handler.dek(), &dek);

        // Test stream decryption with RC4
        let obj_id = 42;
        let gen = 0;
        let obj_key = handler.compute_object_key(obj_id, gen, false);
        let plaintext = b"Stream decrypted successfully under V1/R2";
        let ciphertext = Rc4::new(&obj_key).encrypt(plaintext);

        let decrypted = handler
            .decrypt_stream(obj_id, gen, &ciphertext)
            .expect("decrypt stream failed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_handler_r3_r4_aes_128() {
        let password = b"secret";
        let file_id_0 = b"document-id-hash";
        let permissions = -4i32;
        let dummy_o = [0x55u8; 32];

        let dek = StandardSecurityHandler::derive_key_r2_r4(
            password,
            &dummy_o,
            permissions,
            file_id_0,
            4,
            16,
            true,
        );

        // Generate U per Algorithm 3.5
        let mut hasher = Md5::new();
        hasher.update(PAD_BYTES);
        hasher.update(file_id_0);
        let hash = hasher.finalize();

        let mut res = Rc4::new(&dek).encrypt(&hash);
        let mut key = vec![0u8; 16];
        for i in 1..=19 {
            for (in_b, out_b) in dek.iter().zip(key.iter_mut()) {
                *out_b = *in_b ^ i;
            }
            res = Rc4::new(&key).encrypt(&res);
        }
        let mut u_val = res;
        u_val.extend_from_slice(&[0u8; 16]); // total 32 bytes

        let mut dict = BTreeMap::new();
        dict.insert(
            Cow::Borrowed("Filter"),
            Object::Name(Cow::Borrowed("Standard")),
        );
        dict.insert(Cow::Borrowed("V"), Object::Integer(4));
        dict.insert(Cow::Borrowed("R"), Object::Integer(4));
        dict.insert(Cow::Borrowed("Length"), Object::Integer(128));
        dict.insert(
            Cow::Borrowed("O"),
            Object::String(SmallVec::from_slice(&dummy_o)),
        );
        dict.insert(
            Cow::Borrowed("U"),
            Object::String(SmallVec::from_slice(&u_val)),
        );
        dict.insert(Cow::Borrowed("P"), Object::Integer(permissions as i64));
        dict.insert(Cow::Borrowed("StmF"), Object::Name(Cow::Borrowed("StdCF")));
        dict.insert(Cow::Borrowed("StrF"), Object::Name(Cow::Borrowed("StdCF")));

        let handler = StandardSecurityHandler::from_encrypt_dict(&dict, Some(file_id_0), password)
            .expect("R4 AES-128 auth failed");
        assert_eq!(handler.dek(), &dek);

        // Test AES-128 stream decryption
        let obj_id = 10;
        let gen = 2;
        let obj_key = handler.compute_object_key(obj_id, gen, true);
        let iv = [0x77u8; 16];
        let plaintext = b"PDF AES-128 CBC Content Stream";
        let ciphertext = crate::crypto::aes::aes_128_cbc_encrypt(&obj_key, &iv, plaintext)
            .expect("encrypt failed");

        let decrypted = handler
            .decrypt_stream(obj_id, gen, &ciphertext)
            .expect("decrypt failed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_handler_r5_aes_256() {
        let password = b"supersecret";
        let dek = [0x33u8; 32];
        let zero_iv = [0u8; 16];

        let user_val_salt = [0x11u8; 8];
        let user_key_salt = [0x22u8; 8];

        let mut hasher = Sha256::new();
        hasher.update(password);
        hasher.update(user_val_salt);
        let user_val_hash = hasher.finalize();

        let mut u_bytes = Vec::new();
        u_bytes.extend_from_slice(&user_val_hash);
        u_bytes.extend_from_slice(&user_val_salt);
        u_bytes.extend_from_slice(&user_key_salt);

        let mut k_hasher = Sha256::new();
        k_hasher.update(password);
        k_hasher.update(user_key_salt);
        let user_key = k_hasher.finalize();

        // Encrypt DEK raw without padding
        let mut ue = dek.to_vec();
        let mut enc = cbc::Encryptor::<aes::Aes256>::new(&user_key, (&zero_iv).into());
        for block in ue.chunks_exact_mut(16) {
            enc.encrypt_block_mut(block.into());
        }

        let mut dict = BTreeMap::new();
        dict.insert(
            Cow::Borrowed("Filter"),
            Object::Name(Cow::Borrowed("Standard")),
        );
        dict.insert(Cow::Borrowed("V"), Object::Integer(5));
        dict.insert(Cow::Borrowed("R"), Object::Integer(5));
        dict.insert(Cow::Borrowed("Length"), Object::Integer(256));
        dict.insert(
            Cow::Borrowed("O"),
            Object::String(SmallVec::from_slice(&[0u8; 48])),
        );
        dict.insert(
            Cow::Borrowed("U"),
            Object::String(SmallVec::from_slice(&u_bytes)),
        );
        dict.insert(
            Cow::Borrowed("OE"),
            Object::String(SmallVec::from_slice(&[0u8; 32])),
        );
        dict.insert(
            Cow::Borrowed("UE"),
            Object::String(SmallVec::from_slice(&ue)),
        );
        dict.insert(Cow::Borrowed("P"), Object::Integer(-4));

        let handler = StandardSecurityHandler::from_encrypt_dict(&dict, None, password)
            .expect("R5 auth failed");
        assert_eq!(handler.dek(), &dek);

        // AES-256 stream decryption uses DEK directly
        let iv = [0x88u8; 16];
        let plaintext = b"High-security AES-256 encrypted payload";
        let ciphertext =
            crate::crypto::aes::aes_256_cbc_encrypt(&dek, &iv, plaintext).expect("encrypt failed");

        let decrypted = handler
            .decrypt_stream(100, 0, &ciphertext)
            .expect("decrypt failed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_handler_r6_aes_256() {
        let password = b"iso32000-2";
        let dek = [0x5Au8; 32];
        let zero_iv = [0u8; 16];

        let user_val_salt = [0x33u8; 8];
        let user_key_salt = [0x44u8; 8];

        let user_val_hash =
            StandardSecurityHandler::compute_hash_r6(password, &user_val_salt, None);

        let mut u_bytes = Vec::new();
        u_bytes.extend_from_slice(&user_val_hash);
        u_bytes.extend_from_slice(&user_val_salt);
        u_bytes.extend_from_slice(&user_key_salt);

        let user_key = StandardSecurityHandler::compute_hash_r6(password, &user_key_salt, None);

        // Encrypt DEK raw without padding
        let mut ue = dek.to_vec();
        let mut enc = cbc::Encryptor::<aes::Aes256>::new((&user_key).into(), (&zero_iv).into());
        for block in ue.chunks_exact_mut(16) {
            enc.encrypt_block_mut(block.into());
        }

        let mut dict = BTreeMap::new();
        dict.insert(
            Cow::Borrowed("Filter"),
            Object::Name(Cow::Borrowed("Standard")),
        );
        dict.insert(Cow::Borrowed("V"), Object::Integer(5));
        dict.insert(Cow::Borrowed("R"), Object::Integer(6));
        dict.insert(Cow::Borrowed("Length"), Object::Integer(256));
        dict.insert(
            Cow::Borrowed("O"),
            Object::String(SmallVec::from_slice(&[0u8; 48])),
        );
        dict.insert(
            Cow::Borrowed("U"),
            Object::String(SmallVec::from_slice(&u_bytes)),
        );
        dict.insert(
            Cow::Borrowed("OE"),
            Object::String(SmallVec::from_slice(&[0u8; 32])),
        );
        dict.insert(
            Cow::Borrowed("UE"),
            Object::String(SmallVec::from_slice(&ue)),
        );
        dict.insert(Cow::Borrowed("P"), Object::Integer(-4));

        let handler = StandardSecurityHandler::from_encrypt_dict(&dict, None, password)
            .expect("R6 auth failed");
        assert_eq!(handler.dek(), &dek);

        // AES-256 stream decryption
        let iv = [0x12u8; 16];
        let plaintext = b"PDF 2.0 ISO 32000-2 AES-256 decrypted successfully";
        let ciphertext =
            crate::crypto::aes::aes_256_cbc_encrypt(&dek, &iv, plaintext).expect("encrypt failed");

        let decrypted = handler
            .decrypt_stream(200, 0, &ciphertext)
            .expect("decrypt failed");
        assert_eq!(decrypted, plaintext);
    }
}
