//! Pure-Rust RC4 stream cipher implementation.
//!
//! Follows the standard RC4 Key-Scheduling Algorithm (KSA) and
//! Pseudo-Random Generation Algorithm (PRGA).

#[derive(Clone, Debug)]
pub struct Rc4 {
    state: [u8; 256],
}

impl Rc4 {
    /// Initializes an RC4 instance using the standard KSA.
    ///
    /// # Panics
    /// Panics if `key` is empty.
    pub fn new(key: &[u8]) -> Self {
        assert!(!key.is_empty(), "RC4 key cannot be empty");
        let mut state = [0u8; 256];
        for (i, val) in state.iter_mut().enumerate() {
            *val = i as u8;
        }

        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(state[i]).wrapping_add(key[i % key.len()]);
            state.swap(i, j as usize);
        }

        Self { state }
    }

    /// Encrypts or decrypts bytes from `input` into `output`.
    ///
    /// Clones the initial state so `&self` remains reusable.
    pub fn apply_keystream(&self, input: &[u8], output: &mut [u8]) {
        let mut state = self.state;
        let mut i = 0u8;
        let mut j = 0u8;

        for (in_b, out_b) in input.iter().zip(output.iter_mut()) {
            i = i.wrapping_add(1);
            j = j.wrapping_add(state[i as usize]);
            state.swap(i as usize, j as usize);
            let k = state[(state[i as usize].wrapping_add(state[j as usize])) as usize];
            *out_b = *in_b ^ k;
        }
    }

    /// Encrypts or decrypts `buf` in-place.
    pub fn apply_keystream_in_place(&self, buf: &mut [u8]) {
        let mut state = self.state;
        let mut i = 0u8;
        let mut j = 0u8;

        for b in buf.iter_mut() {
            i = i.wrapping_add(1);
            j = j.wrapping_add(state[i as usize]);
            state.swap(i as usize, j as usize);
            let k = state[(state[i as usize].wrapping_add(state[j as usize])) as usize];
            *b ^= k;
        }
    }

    /// Decrypts input bytes by applying the RC4 keystream.
    pub fn decrypt(&self, input: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; input.len()];
        self.apply_keystream(input, &mut out);
        out
    }

    /// Encrypts input bytes by applying the RC4 keystream (symmetric with decrypt).
    pub fn encrypt(&self, input: &[u8]) -> Vec<u8> {
        self.decrypt(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rc4_rfc_test_vectors() {
        // Test Vector 1: Key = "Key", Plaintext = "Plaintext"
        let rc4 = Rc4::new(b"Key");
        let ct = rc4.encrypt(b"Plaintext");
        let expected = [0xBB, 0xF3, 0x16, 0xE8, 0xD9, 0x40, 0xAF, 0x0A, 0xD3];
        assert_eq!(ct, expected);
        assert_eq!(rc4.decrypt(&ct), b"Plaintext");

        // Test Vector 2: Key = "Wiki", Plaintext = "pedia"
        let rc4 = Rc4::new(b"Wiki");
        let ct = rc4.encrypt(b"pedia");
        let expected = [0x10, 0x21, 0xBF, 0x04, 0x20];
        assert_eq!(ct, expected);
        assert_eq!(rc4.decrypt(&ct), b"pedia");

        // Test Vector 3: Key = "Secret", Plaintext = "Attack at dawn"
        let rc4 = Rc4::new(b"Secret");
        let ct = rc4.encrypt(b"Attack at dawn");
        let expected = [
            0x45, 0xA0, 0x1F, 0x64, 0x5F, 0xC3, 0x5B, 0x38, 0x35, 0x52, 0x54, 0x4B, 0x9B, 0xF5,
        ];
        assert_eq!(ct, expected);
        assert_eq!(rc4.decrypt(&ct), b"Attack at dawn");
    }

    #[test]
    fn test_rc4_roundtrip_in_place() {
        let key = b"oxpdf-crypto-key-128";
        let plaintext = b"PDF Standard Security Handler RC4 Stream Test Payload!";
        let rc4 = Rc4::new(key);

        let mut buffer = plaintext.to_vec();
        rc4.apply_keystream_in_place(&mut buffer);
        assert_ne!(&buffer, plaintext);

        rc4.apply_keystream_in_place(&mut buffer);
        assert_eq!(&buffer, plaintext);
    }
}
