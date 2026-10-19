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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_and_domain_separated() {
        let a = fingerprint("api-key", b"segredo");
        assert_eq!(a, fingerprint("api-key", b"segredo"));
        assert_ne!(a, fingerprint("outro-dominio", b"segredo"));
        assert_eq!(a.len(), 19);
        assert!(!a.contains("segredo"));
    }
}
