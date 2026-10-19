//! Envelope híbrido pós-quântico.
//!
//! Cifra uma mensagem para a [`PublicIdentity`] de um destinatário. O segredo de
//! sessão vem de **dois** acordos de chave independentes:
//!
//! 1. ML-KEM-768 (FIPS 203), resistente a computadores quânticos;
//! 2. X25519 efêmero, clássico e muito estudado.
//!
//! Os dois segredos são combinados com HKDF-SHA256, amarrando também o texto
//! cifrado do KEM e as chaves públicas envolvidas (no espírito do combinador
//! X-Wing). A mensagem é então cifrada com XChaCha20-Poly1305. Para ler, um
//! atacante precisa quebrar **os dois** acordos de chave.
//!
//! Formato binário (v1):
//!
//! ```text
//! versão (1) || x25519 efêmera (32) || ct ML-KEM-768 (1088) || nonce (24) || cifra+tag
//! ```

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use ml_kem::{
    Decapsulate, Encapsulate, KeyExport,
    ml_kem_768::{Ciphertext, DecapsulationKey, EncapsulationKey},
};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{CryptoError, fingerprint, random_bytes};

/// Versão atual do formato de envelope.
pub const VERSION: u8 = 1;
/// Tamanho da chave pública ML-KEM-768 em bytes.
pub const MLKEM_PK_LEN: usize = 1184;
/// Tamanho do texto cifrado ML-KEM-768 em bytes.
pub const MLKEM_CT_LEN: usize = 1088;
const X25519_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const HEADER_LEN: usize = 1 + X25519_LEN + MLKEM_CT_LEN + NONCE_LEN;
const KDF_INFO: &[u8] = b"dotcrow/v1/envelope/mlkem768-x25519/xchacha20poly1305";

/// Identidade pública: o que alguém precisa conhecer para cifrar para você.
#[derive(Clone, PartialEq, Eq)]
pub struct PublicIdentity {
    x25519: [u8; X25519_LEN],
    mlkem: Vec<u8>,
}

impl PublicIdentity {
    /// Serializa como `x25519 (32) || ML-KEM-768 (1184)`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(X25519_LEN + MLKEM_PK_LEN);
        out.extend_from_slice(&self.x25519);
        out.extend_from_slice(&self.mlkem);
        out
    }

    /// Lê o formato produzido por [`PublicIdentity::to_bytes`], validando a chave ML-KEM.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != X25519_LEN + MLKEM_PK_LEN {
            return Err(CryptoError::Format);
        }
        let (x, m) = bytes.split_at(X25519_LEN);
        parse_ek(m)?;
        Ok(Self { x25519: x.try_into().map_err(|_| CryptoError::Format)?, mlkem: m.to_vec() })
    }

    /// Impressão digital da identidade, para conferência visual entre pessoas.
    pub fn fingerprint(&self) -> String {
        fingerprint("identity", &self.to_bytes())
    }
}

impl std::fmt::Debug for PublicIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PublicIdentity({})", self.fingerprint())
    }
}

/// Identidade secreta. Nunca sai do dispositivo; é apagada da memória ao ser descartada.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretIdentity {
    x25519: [u8; X25519_LEN],
    mlkem_seed: [u8; 64],
}

impl SecretIdentity {
    /// Gera uma identidade nova com o RNG do sistema.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut s = Self { x25519: [0; X25519_LEN], mlkem_seed: [0; 64] };
        random_bytes(&mut s.x25519)?;
        random_bytes(&mut s.mlkem_seed)?;
        Ok(s)
    }

    /// Deriva a identidade pública correspondente.
    pub fn public(&self) -> PublicIdentity {
        let x = PublicKey::from(&StaticSecret::from(self.x25519));
        let dk = self.mlkem_dk();
        PublicIdentity { x25519: x.to_bytes(), mlkem: dk.encapsulation_key().to_bytes().to_vec() }
    }

    /// Exporta os 96 bytes secretos para guardar no cofre do sistema. Trate como senha.
    pub fn to_secret_bytes(&self) -> Zeroizing<Vec<u8>> {
        let mut v = Zeroizing::new(Vec::with_capacity(96));
        v.extend_from_slice(&self.x25519);
        v.extend_from_slice(&self.mlkem_seed);
        v
    }

    /// Restaura a partir de [`SecretIdentity::to_secret_bytes`].
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != 96 {
            return Err(CryptoError::Format);
        }
        let mut s = Self { x25519: [0; X25519_LEN], mlkem_seed: [0; 64] };
        s.x25519.copy_from_slice(&bytes[..32]);
        s.mlkem_seed.copy_from_slice(&bytes[32..]);
        Ok(s)
    }

    fn mlkem_dk(&self) -> DecapsulationKey {
        DecapsulationKey::from_seed(self.mlkem_seed.into())
    }
}

fn parse_ek(bytes: &[u8]) -> Result<EncapsulationKey, CryptoError> {
    let arr = bytes.try_into().map_err(|_| CryptoError::Format)?;
    EncapsulationKey::new(&arr).map_err(|_| CryptoError::Format)
}

fn derive_key(
    ss_mlkem: &[u8],
    ss_x25519: &[u8],
    mlkem_ct: &[u8],
    eph_pub: &[u8; X25519_LEN],
    recipient: &PublicIdentity,
) -> Zeroizing<[u8; 32]> {
    let mut ikm = Zeroizing::new(Vec::with_capacity(64));
    ikm.extend_from_slice(ss_mlkem);
    ikm.extend_from_slice(ss_x25519);
    let mut info = Vec::with_capacity(KDF_INFO.len() + MLKEM_CT_LEN + 64);
    info.extend_from_slice(KDF_INFO);
    info.extend_from_slice(mlkem_ct);
    info.extend_from_slice(eph_pub);
    info.extend_from_slice(&recipient.x25519);
    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut okm = Zeroizing::new([0u8; 32]);
    // 32 bytes está muito abaixo do limite do HKDF-SHA256 (8160 bytes).
    let _ = hk.expand(&info, okm.as_mut());
    okm
}

/// Cifra `plaintext` para `recipient`. `aad` precisa ser repetido na abertura.
pub fn seal(recipient: &PublicIdentity, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    // Parte pós-quântica
    let ek = parse_ek(&recipient.mlkem)?;
    let (ct, ss_mlkem) = ek.encapsulate();

    // Parte clássica (chave efêmera)
    let mut eph = Zeroizing::new([0u8; X25519_LEN]);
    random_bytes(eph.as_mut())?;
    let eph_secret = StaticSecret::from(*eph);
    let eph_pub = PublicKey::from(&eph_secret).to_bytes();
    let ss_x = eph_secret.diffie_hellman(&PublicKey::from(recipient.x25519));
    if !ss_x.was_contributory() {
        return Err(CryptoError::Format);
    }

    let key = derive_key(&ss_mlkem, ss_x.as_bytes(), &ct, &eph_pub, recipient);
    let mut nonce = [0u8; NONCE_LEN];
    random_bytes(&mut nonce)?;

    let mut out = Vec::with_capacity(HEADER_LEN + plaintext.len() + 16);
    out.push(VERSION);
    out.extend_from_slice(&eph_pub);
    out.extend_from_slice(&ct);
    out.extend_from_slice(&nonce);

    let mut full_aad = out.clone();
    full_aad.extend_from_slice(aad);
    let cipher = XChaCha20Poly1305::new(&(*key).into());
    let body = cipher
        .encrypt(&XNonce::from(nonce), Payload { msg: plaintext, aad: &full_aad })
        .map_err(|_| CryptoError::Encrypt)?;
    out.extend_from_slice(&body);
    Ok(out)
}

/// Abre um envelope produzido por [`seal`].
pub fn open(secret: &SecretIdentity, envelope: &[u8], aad: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if envelope.len() < HEADER_LEN + 16 {
        return Err(CryptoError::Format);
    }
    if envelope[0] != VERSION {
        return Err(CryptoError::UnsupportedVersion(envelope[0]));
    }
    let (header, body) = envelope.split_at(HEADER_LEN);
    let eph_pub: [u8; X25519_LEN] = header[1..1 + X25519_LEN].try_into().map_err(|_| CryptoError::Format)?;
    let ct_bytes = &header[1 + X25519_LEN..1 + X25519_LEN + MLKEM_CT_LEN];
    let nonce: [u8; NONCE_LEN] = header[HEADER_LEN - NONCE_LEN..].try_into().map_err(|_| CryptoError::Format)?;

    let ct: Ciphertext = ct_bytes.try_into().map_err(|_| CryptoError::Format)?;
    let ss_mlkem = secret.mlkem_dk().decapsulate(&ct);
    let ss_x = StaticSecret::from(secret.x25519).diffie_hellman(&PublicKey::from(eph_pub));
    if !ss_x.was_contributory() {
        return Err(CryptoError::Decrypt);
    }

    let recipient = secret.public();
    let key = derive_key(&ss_mlkem, ss_x.as_bytes(), ct_bytes, &eph_pub, &recipient);
    let mut full_aad = header.to_vec();
    full_aad.extend_from_slice(aad);
    let cipher = XChaCha20Poly1305::new(&(*key).into());
    cipher
        .decrypt(&XNonce::from(nonce), Payload { msg: body, aad: &full_aad })
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let alice = SecretIdentity::generate().unwrap();
        let env = seal(&alice.public(), b"tarefa: organizar downloads", b"ctx").unwrap();
        assert_eq!(open(&alice, &env, b"ctx").unwrap().as_slice(), b"tarefa: organizar downloads");
    }

    #[test]
    fn other_identity_cannot_open() {
        let alice = SecretIdentity::generate().unwrap();
        let mallory = SecretIdentity::generate().unwrap();
        let env = seal(&alice.public(), b"segredo", b"").unwrap();
        assert_eq!(open(&mallory, &env, b"").unwrap_err(), CryptoError::Decrypt);
    }

    #[test]
    fn header_and_aad_are_authenticated() {
        let alice = SecretIdentity::generate().unwrap();
        let env = seal(&alice.public(), b"x", b"aad").unwrap();
        assert!(open(&alice, &env, b"outro").is_err());
        for pos in [5, 40, 1130, env.len() - 1] {
            let mut bad = env.clone();
            bad[pos] ^= 0x80;
            assert!(open(&alice, &bad, b"aad").is_err(), "byte {pos} adulterado passou");
        }
    }

    #[test]
    fn identity_serialization_roundtrip() {
        let s = SecretIdentity::generate().unwrap();
        let restored = SecretIdentity::from_secret_bytes(&s.to_secret_bytes()).unwrap();
        assert_eq!(s.public(), restored.public());
        let pub_bytes = s.public().to_bytes();
        assert_eq!(pub_bytes.len(), 32 + MLKEM_PK_LEN);
        assert_eq!(PublicIdentity::from_bytes(&pub_bytes).unwrap(), s.public());
        assert!(PublicIdentity::from_bytes(&pub_bytes[1..]).is_err());
    }

    #[test]
    fn envelope_size_is_predictable() {
        let alice = SecretIdentity::generate().unwrap();
        let env = seal(&alice.public(), &[0u8; 100], b"").unwrap();
        assert_eq!(env.len(), HEADER_LEN + 100 + 16);
    }
}
