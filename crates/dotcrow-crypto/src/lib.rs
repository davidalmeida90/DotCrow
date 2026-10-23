//! Criptografia do DotCrow.
//!
//! Este crate oferece duas peças:
//!
//! - [`envelope`]: cifra uma mensagem para a identidade pública de um destinatário
//!   usando um KEM **híbrido** (ML-KEM-768, pós-quântico, combinado com X25519,
//!   clássico). Um atacante precisa quebrar os dois para ler a mensagem.
//! - [`sym`]: cifra simétrica autenticada (XChaCha20-Poly1305) para dados em repouso,
//!   como o cofre de chaves de API.
//!
//! **Atenção:** este código ainda não passou por auditoria externa. Veja `SECURITY.md`
//! e `docs/seguranca.md` antes de usar com dados reais.

pub mod envelope;
pub mod sym;

use sha2::{Digest, Sha256};

/// Erros de criptografia. As mensagens são propositalmente genéricas para não
/// vazar detalhes úteis a um atacante.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CryptoError {
    /// O gerador de números aleatórios do sistema falhou.
    #[error("falha ao obter bytes aleatórios do sistema")]
    Rng,
    /// Os bytes recebidos não têm o formato esperado.
    #[error("formato inválido")]
    Format,
    /// A versão do formato não é suportada por esta build.
    #[error("versão de formato não suportada: {0}")]
    UnsupportedVersion(u8),
    /// Falha ao decifrar: chave errada, dados adulterados ou dados associados diferentes.
    #[error("não foi possível decifrar (chave errada ou dados adulterados)")]
    Decrypt,
    /// Falha ao cifrar.
    #[error("não foi possível cifrar")]
    Encrypt,
}

/// Preenche `buf` com bytes aleatórios do sistema operacional.
pub fn random_bytes(buf: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(buf).map_err(|_| CryptoError::Rng)
}

/// Impressão digital curta e estável de um segredo ou chave pública.
///
/// Usa SHA-256 com separação de domínio e mostra só os 8 primeiros bytes, em
/// grupos de 4 dígitos hex (`a1b2-c3d4-e5f6-0718`). Serve para o dono identificar
/// *qual* chave é qual sem que a chave em si apareça em lugar nenhum.
pub fn fingerprint(domain: &str, data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"dotcrow/fingerprint/v1\0");
    h.update(domain.as_bytes());
    h.update([0u8]);
    h.update(data);
    let digest = h.finalize();
    digest[..8].chunks(2).map(|c| format!("{:02x}{:02x}", c[0], c[1])).collect::<Vec<_>>().join("-")
}

/// Id de sala do túnel remoto: hash de 128 bits (32 dígitos hex) de `data`, estável e
/// difícil de adivinhar. O relay usa isto para parear computador e celular; como é
/// derivado da identidade pública do computador, o celular confere que a sala bate com
/// a identidade que ele conhece (anti-impostor), sem o relay saber de quem é a sala.
pub fn room_id(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"dotcrow/room/v1\0");
    h.update(data);
    let digest = h.finalize();
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// Token de pareamento de curta duração, para o QR que troca a cada 10 s.
///
/// Depende do código de acesso (secreto) e da `janela` (o tempo em blocos de 10 s), então
/// muda a cada bloco e um atacante que vê um QR não consegue prever o próximo nem achar o
/// código. Quem tem o código confere se o token corresponde a uma janela recente.
pub fn pairing_token(codigo: &str, janela: u64) -> String {
    let mut h = Sha256::new();
    h.update(b"dotcrow/pareamento/v1\0");
    h.update(codigo.as_bytes());
    h.update(janela.to_le_bytes());
    let digest = h.finalize();
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_token_muda_por_janela_e_por_codigo() {
        let a = pairing_token("abcd-efgh", 100);
        assert_eq!(a, pairing_token("abcd-efgh", 100));
        assert_ne!(a, pairing_token("abcd-efgh", 101), "janela nova, token novo");
        assert_ne!(a, pairing_token("outro-codigo", 100));
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn room_id_is_stable_hex_and_depends_on_input() {
        let a = room_id(b"identidade-publica");
        assert_eq!(a, room_id(b"identidade-publica"));
        assert_ne!(a, room_id(b"outra"));
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn fingerprint_is_stable_and_domain_separated() {
        let a = fingerprint("api-key", b"segredo");
        assert_eq!(a, fingerprint("api-key", b"segredo"));
        assert_ne!(a, fingerprint("outro-dominio", b"segredo"));
        assert_eq!(a.len(), 19);
        assert!(!a.contains("segredo"));
    }
}
