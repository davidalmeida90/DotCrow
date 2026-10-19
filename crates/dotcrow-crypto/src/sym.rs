//! Cifra simétrica autenticada para dados em repouso (XChaCha20-Poly1305).
//!
//! Formato da saída: `versão (1 byte) || nonce (24 bytes) || texto cifrado + tag`.
//! O nonce de 24 bytes é aleatório a cada chamada, então não há risco prático de
//! repetição mesmo cifrando muitas vezes com a mesma chave.

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use zeroize::Zeroizing;

use crate::{CryptoError, random_bytes};

/// Versão atual do formato simétrico.
pub const VERSION: u8 = 1;
const NONCE_LEN: usize = 24;

/// Chave simétrica de 32 bytes, apagada da memória quando sai de escopo.
pub type SymKey = Zeroizing<[u8; 32]>;

/// Gera uma chave simétrica nova a partir do RNG do sistema.
pub fn generate_key() -> Result<SymKey, CryptoError> {
    let mut k = Zeroizing::new([0u8; 32]);
    random_bytes(k.as_mut())?;
    Ok(k)
}

/// Cifra `plaintext` com `key`. `aad` são dados associados: não são cifrados, mas
/// precisam ser idênticos na hora de decifrar (use para amarrar o contexto, por
/// exemplo `b"dotcrow/vault/v1"`).
pub fn seal(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let mut nonce = [0u8; NONCE_LEN];
    random_bytes(&mut nonce)?;
    let cipher = XChaCha20Poly1305::new(&(*key).into());
    let ct = cipher.encrypt(&XNonce::from(nonce), Payload { msg: plaintext, aad }).map_err(|_| CryptoError::Encrypt)?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
    out.push(VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decifra o que foi produzido por [`seal`].
pub fn open(key: &[u8; 32], sealed: &[u8], aad: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let (&version, rest) = sealed.split_first().ok_or(CryptoError::Format)?;
    if version != VERSION {
        return Err(CryptoError::UnsupportedVersion(version));
    }
    if rest.len() < NONCE_LEN + 16 {
        return Err(CryptoError::Format);
    }
    let (nonce, ct) = rest.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CryptoError::Format)?;
    let cipher = XChaCha20Poly1305::new(&(*key).into());
    cipher.decrypt(&XNonce::from(nonce), Payload { msg: ct, aad }).map(Zeroizing::new).map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let k = generate_key().unwrap();
        let sealed = seal(&k, b"ola corvo", b"ctx").unwrap();
        assert_eq!(open(&k, &sealed, b"ctx").unwrap().as_slice(), b"ola corvo");
    }

    #[test]
    fn wrong_key_or_aad_fails() {
        let k1 = generate_key().unwrap();
        let k2 = generate_key().unwrap();
        let sealed = seal(&k1, b"x", b"ctx").unwrap();
        assert_eq!(open(&k2, &sealed, b"ctx").unwrap_err(), CryptoError::Decrypt);
        assert_eq!(open(&k1, &sealed, b"outro").unwrap_err(), CryptoError::Decrypt);
    }

    #[test]
    fn tampering_is_detected() {
        let k = generate_key().unwrap();
        let mut sealed = seal(&k, b"dados", b"").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert_eq!(open(&k, &sealed, b"").unwrap_err(), CryptoError::Decrypt);
    }

    #[test]
    fn nonces_are_unique() {
        let k = generate_key().unwrap();
        let a = seal(&k, b"igual", b"").unwrap();
        let b = seal(&k, b"igual", b"").unwrap();
        assert_ne!(a, b);
    }
}
