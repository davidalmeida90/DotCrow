//! Cofre de chaves de API do DotCrow.
//!
//! O cofre é **somente escrita** do ponto de vista do dono do computador:
//!
//! - dá para **adicionar** uma chave;
//! - dá para **listar** as chaves (provedor, rótulo, data e impressão digital);
//! - dá para **apagar** uma chave;
//! - **não existe** operação que devolva o texto da chave para a interface, a CLI ou a API local.
//!
//! O único caminho até o segredo é [`Vault::lease`], usado pelo runtime para montar a
//! requisição ao provedor de LLM, dentro do processo do DotCrow. Para trocar uma chave,
//! apague a antiga e adicione uma nova.
//!
//! Em disco, o cofre inteiro é um único blob cifrado com XChaCha20-Poly1305
//! (ver `dotcrow-crypto::sym`). Veja `docs/cofre-de-chaves.md` para o modelo de ameaças.

use std::{
    fmt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use dotcrow_crypto::{CryptoError, fingerprint, random_bytes, sym};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const AAD: &[u8] = b"dotcrow/vault/v1";
const MIN_SECRET_LEN: usize = 8;
const MAX_SECRET_LEN: usize = 1024;

/// Erros do cofre.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    /// Nome de provedor inválido.
    #[error("nome de provedor inválido: use letras minúsculas, números e hífen (ex.: anthropic, openai, ollama)")]
    InvalidProvider,
    /// A chave informada é vazia, curta demais ou tem espaços.
    #[error("chave inválida: precisa ter entre {MIN_SECRET_LEN} e {MAX_SECRET_LEN} caracteres e nenhum espaço")]
    InvalidSecret,
    /// Essa mesma chave já está guardada.
    #[error("essa chave já está no cofre (id {0})")]
    Duplicate(String),
    /// Nenhuma chave com esse id.
    #[error("nenhuma chave com o id {0}")]
    NotFound(String),
    /// Falha de criptografia (arquivo adulterado ou chave mestra errada).
    #[error("falha ao abrir o cofre: {0}")]
    Crypto(#[from] CryptoError),
    /// Falha de leitura ou escrita em disco.
    #[error("erro de arquivo: {0}")]
    Io(#[from] std::io::Error),
    /// Conteúdo do cofre corrompido.
    #[error("conteúdo do cofre corrompido")]
    Corrupt,
}

/// Informações públicas de uma chave. É tudo o que o dono consegue ver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyInfo {
    /// Identificador curto e aleatório da chave no cofre.
    pub id: String,
    /// Provedor da chave (ex.: `anthropic`, `openai`, `gemini`, `openrouter`).
    pub provider: String,
    /// Rótulo opcional dado pelo dono (ex.: "conta pessoal").
    pub label: Option<String>,
    /// Impressão digital (hash truncado) para saber qual chave é qual sem mostrá-la.
    pub fingerprint: String,
    /// Quando a chave foi adicionada (segundos desde 1970, UTC).
    pub created_at: u64,
}

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    info: KeyInfo,
    secret: String,
}

struct Entry {
    info: KeyInfo,
    secret: Zeroizing<String>,
}

enum Backend {
    Memory,
    File { path: PathBuf, master_key: sym::SymKey },
}

/// Acesso temporário a um segredo, só para o runtime montar uma requisição.
///
/// Não implementa `Display` e o `Debug` é mascarado, para o segredo não escapar
/// por acidente em logs.
pub struct Lease<'a> {
    info: &'a KeyInfo,
    secret: &'a str,
}

impl Lease<'_> {
    /// Metadados públicos da chave emprestada.
    pub fn info(&self) -> &KeyInfo {
        self.info
    }

    /// Usa o segredo dentro de `f`. Não guarde cópias dele.
    pub fn with_secret<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        f(self.secret)
    }
}

impl fmt::Debug for Lease<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lease({} {}, segredo=***)", self.info.provider, self.info.fingerprint)
    }
}

/// O cofre de chaves.
pub struct Vault {
    entries: Vec<Entry>,
    backend: Backend,
}

impl fmt::Debug for Vault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vault").field("keys", &self.list()).finish_non_exhaustive()
    }
}

impl Vault {
    /// Cofre só em memória, usado em testes.
    pub fn in_memory() -> Self {
        Self { entries: Vec::new(), backend: Backend::Memory }
    }

    /// Abre (ou cria, se não existir) o cofre cifrado em `path` com a chave mestra.
    pub fn open_file(path: impl AsRef<Path>, master_key: sym::SymKey) -> Result<Self, VaultError> {
        let path = path.as_ref().to_path_buf();
        let entries = match std::fs::read(&path) {
            Ok(bytes) => {
                let plain = sym::open(&master_key, &bytes, AAD)?;
                let stored: Vec<StoredEntry> = serde_json::from_slice(&plain).map_err(|_| VaultError::Corrupt)?;
                stored.into_iter().map(|s| Entry { info: s.info, secret: Zeroizing::new(s.secret) }).collect()
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self { entries, backend: Backend::File { path, master_key } })
    }

    /// Adiciona uma chave de API. Devolve só os metadados públicos.
    pub fn add(
        &mut self,
        provider: &str,
        secret: Zeroizing<String>,
        label: Option<String>,
    ) -> Result<KeyInfo, VaultError> {
        validate_provider(provider)?;
        let trimmed = secret.trim();
        if trimmed.len() < MIN_SECRET_LEN || trimmed.len() > MAX_SECRET_LEN || trimmed.chars().any(char::is_whitespace)
        {
            return Err(VaultError::InvalidSecret);
        }
        let fp = fingerprint(&format!("api-key/{provider}"), trimmed.as_bytes());
        if let Some(e) = self.entries.iter().find(|e| e.info.provider == provider && e.info.fingerprint == fp) {
            return Err(VaultError::Duplicate(e.info.id.clone()));
        }
        let info = KeyInfo {
            id: new_id()?,
            provider: provider.to_string(),
            label: label.map(|l| l.trim().to_string()).filter(|l| !l.is_empty()),
            fingerprint: fp,
            created_at: now(),
        };
        self.entries.push(Entry { info: info.clone(), secret: Zeroizing::new(trimmed.to_string()) });
        self.persist()?;
        Ok(info)
    }

    /// Lista as chaves guardadas, na ordem em que foram adicionadas. Nunca inclui o segredo.
    pub fn list(&self) -> Vec<KeyInfo> {
        self.entries.iter().map(|e| e.info.clone()).collect()
    }

    /// Apaga a chave com o `id` informado.
    pub fn delete(&mut self, id: &str) -> Result<KeyInfo, VaultError> {
        let pos = self.entries.iter().position(|e| e.info.id == id).ok_or_else(|| VaultError::NotFound(id.into()))?;
        let removed = self.entries.remove(pos);
        self.persist()?;
        Ok(removed.info)
    }

    /// Empresta ao runtime a chave mais antiga do `provider`, se houver.
    pub fn lease(&self, provider: &str) -> Option<Lease<'_>> {
        self.entries
            .iter()
            .find(|e| e.info.provider == provider)
            .map(|e| Lease { info: &e.info, secret: e.secret.as_str() })
    }

    fn persist(&self) -> Result<(), VaultError> {
        let Backend::File { path, master_key } = &self.backend else { return Ok(()) };
        let stored: Vec<StoredEntry> =
            self.entries.iter().map(|e| StoredEntry { info: e.info.clone(), secret: e.secret.to_string() }).collect();
        let plain = Zeroizing::new(serde_json::to_vec(&stored).map_err(|_| VaultError::Corrupt)?);
        for mut s in stored {
            zeroize::Zeroize::zeroize(&mut s.secret);
        }
        let sealed = sym::seal(master_key, &plain, AAD)?;
        write_private(path, &sealed)?;
        Ok(())
    }
}

fn validate_provider(p: &str) -> Result<(), VaultError> {
    let ok = !p.is_empty()
        && p.len() <= 40
        && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !p.starts_with('-');
    if ok { Ok(()) } else { Err(VaultError::InvalidProvider) }
}

fn new_id() -> Result<String, VaultError> {
    let mut b = [0u8; 4];
    random_bytes(&mut b)?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Grava de forma atômica (arquivo temporário + rename) e, no Unix, com permissão 0600.
pub fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> Zeroizing<String> {
        Zeroizing::new(x.to_string())
    }

    #[test]
    fn add_list_delete() {
        let mut v = Vault::in_memory();
        let k = v.add("anthropic", s("sk-teste-12345678"), Some("pessoal".into())).unwrap();
        assert_eq!(v.list(), vec![k.clone()]);
        v.delete(&k.id).unwrap();
        assert!(v.list().is_empty());
    }

    #[test]
    fn listing_never_contains_the_secret() {
        let mut v = Vault::in_memory();
        v.add("openai", s("sk-super-secreta-999"), None).unwrap();
        let shown = format!("{:?} {}", v, serde_json::to_string(&v.list()).unwrap());
        assert!(!shown.contains("sk-super-secreta-999"));
        let lease = v.lease("openai").unwrap();
        assert!(!format!("{lease:?}").contains("sk-super-secreta-999"));
        assert_eq!(lease.with_secret(|k| k.len()), 20);
    }

    #[test]
    fn rejects_bad_input_and_duplicates() {
        let mut v = Vault::in_memory();
        assert!(matches!(v.add("Anthropic", s("sk-12345678"), None), Err(VaultError::InvalidProvider)));
        assert!(matches!(v.add("anthropic", s("curta"), None), Err(VaultError::InvalidSecret)));
        assert!(matches!(v.add("anthropic", s("tem espaco 123"), None), Err(VaultError::InvalidSecret)));
        v.add("anthropic", s("sk-12345678"), None).unwrap();
        assert!(matches!(v.add("anthropic", s("  sk-12345678 "), None), Err(VaultError::Duplicate(_))));
    }

    #[test]
    fn file_roundtrip_is_encrypted_at_rest() {
        let dir = std::env::temp_dir().join(format!("dotcrow-vault-test-{}", new_id().unwrap()));
        let path = dir.join("vault.bin");
        let key = sym::generate_key().unwrap();
        {
            let mut v = Vault::open_file(&path, Zeroizing::new(*key)).unwrap();
            v.add("gemini", s("AIza-segredo-gemini"), None).unwrap();
        }
        let raw = std::fs::read(&path).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("AIza-segredo-gemini"));
        let v = Vault::open_file(&path, Zeroizing::new(*key)).unwrap();
        assert_eq!(v.list().len(), 1);
        let wrong = sym::generate_key().unwrap();
        assert!(matches!(Vault::open_file(&path, wrong), Err(VaultError::Crypto(_))));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
