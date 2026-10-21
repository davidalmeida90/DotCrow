//! A memória do DotCrow (o RAG do Muninn): o que o bando lembra sobre o dono.
//!
//! Cada lembrança fica **cifrada em disco** com a chave-mestra deste computador
//! (XChaCha20-Poly1305), então só este computador abre. Antes de guardar e antes de
//! mandar qualquer coisa para a IA, passa pelo [`redigir`], que apaga o que parece
//! senha, chave ou segredo. A busca é por palavras (sem precisar de modelo externo).

use std::path::{Path, PathBuf};

use dotcrow_agents::Memory;
use dotcrow_crypto::sym::{self, SymKey};
use serde::{Deserialize, Serialize};

/// Amarra o contexto da cifra: dados de outra finalidade não decifram aqui.
const AAD: &[u8] = b"dotcrow/memoria/v1";
/// Quantas lembranças a busca devolve por padrão.
const TOP: usize = 6;
/// Teto de lembranças lidas numa busca (as mais novas primeiro).
const MAX_SCAN: usize = 2000;
/// Marca do que foi removido por parecer segredo.
const REMOVIDO: &str = "[removido: possível segredo]";

/// Uma lembrança guardada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lembranca {
    /// Id (hex).
    pub id: String,
    /// Quando foi guardada (segundos desde 1970).
    pub ts: u64,
    /// O texto (já sem segredos).
    pub texto: String,
    /// De onde veio (ex.: "conversa", "arquivo: notas.md", "manual").
    pub fonte: String,
}

/// A memória cifrada deste computador.
pub struct Memoria {
    dir: PathBuf,
    key: SymKey,
}

impl Memoria {
    /// Abre (ou cria) a memória em `dir/memoria`, cifrada com `key`.
    pub fn abrir(dir: &Path, key: SymKey) -> std::io::Result<Self> {
        let dir = dir.join("memoria");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir, key })
    }

    fn now() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }

    fn id() -> String {
        sym::generate_key()
            .map(|k| k.iter().take(8).map(|b| format!("{b:02x}")).collect())
            .unwrap_or_else(|_| format!("{:x}", Self::now()))
    }

    /// Guarda uma lembrança. Remove segredos antes; se sobrar só segredo, não guarda.
    /// Devolve o id, ou `None` se não guardou.
    pub fn guardar(&self, texto: &str, fonte: &str) -> std::io::Result<Option<String>> {
        let limpo = redigir(texto);
        if limpo.trim().is_empty() || limpo.trim() == REMOVIDO {
            return Ok(None);
        }
        let l = Lembranca { id: Self::id(), ts: Self::now(), texto: limpo, fonte: fonte.to_string() };
        let json = serde_json::to_vec(&l).unwrap_or_default();
        let selado = sym::seal(&self.key, &json, AAD).map_err(std::io::Error::other)?;
        std::fs::write(self.dir.join(format!("{}.mem", l.id)), selado)?;
        Ok(Some(l.id))
    }

    /// Lê todas as lembranças (as mais novas primeiro), decifrando. Ignora arquivos corrompidos.
    pub fn todas(&self) -> Vec<Lembranca> {
        let mut arquivos: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "mem"))
            .collect();
        // Nome começa com bytes aleatórios, então ordeno por data de modificação (mais novo primeiro).
        arquivos.sort_by_key(|p| std::cmp::Reverse(p.metadata().and_then(|m| m.modified()).ok()));
        arquivos
            .into_iter()
            .take(MAX_SCAN)
            .filter_map(|p| {
                let bytes = std::fs::read(&p).ok()?;
                let plano = sym::open(&self.key, &bytes, AAD).ok()?;
                serde_json::from_slice::<Lembranca>(&plano).ok()
            })
            .collect()
    }

    /// Apaga uma lembrança pelo id. Devolve se existia.
    pub fn esquecer(&self, id: &str) -> std::io::Result<bool> {
        if !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(false);
        }
        let p = self.dir.join(format!("{id}.mem"));
        match std::fs::remove_file(&p) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Quantas lembranças existem.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn quantidade(&self) -> usize {
        std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "mem"))
            .count()
    }
}

impl Memory for Memoria {
    fn recall(&self, query: &str) -> Vec<String> {
        let termos = tokenizar(query);
        if termos.is_empty() {
            return Vec::new();
        }
        let mut pontuadas: Vec<(u32, Lembranca)> =
            self.todas().into_iter().map(|l| (pontuar(&termos, &l.texto), l)).filter(|(p, _)| *p > 0).collect();
        pontuadas.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.ts.cmp(&a.1.ts)));
        pontuadas.into_iter().take(TOP).map(|(_, l)| redigir(&l.texto)).collect()
    }

    fn remember(&self, text: &str) {
        let _ = self.guardar(text, "conversa");
    }
}

/// Palavras (minúsculas, sem acento simples, ≥3 letras, sem palavras muito comuns).
fn tokenizar(texto: &str) -> Vec<String> {
    const COMUNS: [&str; 40] = [
        "para", "com", "que", "uma", "the", "and", "você", "voce", "dos", "das", "por", "mais", "como", "num", "nos",
        "sua", "seu", "meu", "minha", "isso", "esse", "essa", "este", "esta", "foi", "são", "sao", "tem", "ser",
        "fazer", "sobre", "entre", "pelo", "pela", "aos", "the", "for", "was", "are", "not",
    ];
    texto
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !COMUNS.contains(w))
        .map(String::from)
        .collect()
}

/// Nota de uma lembrança para a consulta: soma de quantas vezes cada termo aparece.
fn pontuar(termos: &[String], texto: &str) -> u32 {
    let toks = tokenizar(texto);
    termos.iter().map(|t| toks.iter().filter(|w| *w == t).count() as u32).sum()
}

/// Apaga do texto o que parece senha, chave ou segredo. Conservador: na dúvida, remove.
pub fn redigir(texto: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut pulando_pem = false;
    for linha in texto.lines() {
        let l = linha.trim();
        if l.contains("-----BEGIN") && l.contains("PRIVATE KEY") {
            pulando_pem = true;
            out.push(REMOVIDO.to_string());
            continue;
        }
        if pulando_pem {
            if l.contains("-----END") {
                pulando_pem = false;
            }
            continue;
        }
        if linha_sensivel(l) {
            out.push(REMOVIDO.to_string());
            continue;
        }
        out.push(redigir_tokens(linha));
    }
    out.join("\n")
}

/// A linha inteira é um segredo? (atribuição de senha/chave, .env, etc.)
fn linha_sensivel(l: &str) -> bool {
    let baixo = l.to_lowercase();
    // chave=valor onde a chave fala de segredo e o valor não é vazio
    if let Some((nome, valor)) = l.split_once(['=', ':']) {
        let nome = nome.to_lowercase();
        let tem_rotulo = [
            "senha",
            "password",
            "passwd",
            "secret",
            "token",
            "api_key",
            "apikey",
            "api-key",
            "chave",
            "client_secret",
            "access_key",
            "private_key",
        ]
        .iter()
        .any(|x| nome.contains(x));
        if tem_rotulo && valor.trim().len() >= 4 {
            return true;
        }
    }
    baixo.contains("authorization: bearer ") || baixo.contains("authorization: basic ")
}

/// Troca por [removido] qualquer palavra que pareça uma chave/token solta.
fn redigir_tokens(linha: &str) -> String {
    linha
        .split(' ')
        .map(|p| {
            if parece_segredo(p.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | '(' | ')' | '<' | '>'))) {
                REMOVIDO
            } else {
                p
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Uma palavra solta parece uma chave/token?
fn parece_segredo(p: &str) -> bool {
    const PREFIXOS: [&str; 12] = [
        "sk-",
        "sk_live",
        "pk_live",
        "ghp_",
        "gho_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "akia",
        "asia",
        "aiza",
        "glpat-",
    ];
    let baixo = p.to_lowercase();
    if PREFIXOS.iter().any(|x| baixo.starts_with(x)) {
        return true;
    }
    // Token comprido e "embaralhado": muitas letras e dígitos misturados, sem espaços.
    if p.len() >= 40 && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/' | '=')) {
        let tem_digito = p.chars().any(|c| c.is_ascii_digit());
        let tem_letra = p.chars().any(|c| c.is_ascii_alphabetic());
        if tem_digito && tem_letra {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> (Memoria, PathBuf) {
        let base =
            std::env::temp_dir().join(format!("dotcrow-mem-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&base);
        let key = sym::generate_key().unwrap();
        (Memoria::abrir(&base, key).unwrap(), base)
    }

    #[test]
    fn redaction_removes_secrets_keeps_prose() {
        let t = "O cliente gosta de café.\nAPI_KEY=sk-abcdef1234567890abcdef\nsenha: minhaSenhaForte\nnormal line";
        let r = redigir(t);
        assert!(r.contains("O cliente gosta de café."));
        assert!(r.contains("normal line"));
        assert!(!r.contains("sk-abcdef"));
        assert!(!r.contains("minhaSenhaForte"));
        assert_eq!(r.matches(REMOVIDO).count(), 2);
    }

    #[test]
    fn redaction_strips_pem_blocks_and_loose_tokens() {
        let t = "antes\n-----BEGIN PRIVATE KEY-----\nMIIBVAIB...\n-----END PRIVATE KEY-----\ndepois ghp_0123456789abcdefABCDEF0123 fim";
        let r = redigir(t);
        assert!(r.contains("antes"));
        assert!(r.contains("depois"));
        assert!(r.contains("fim"));
        assert!(!r.contains("MIIBVAIB"));
        assert!(!r.contains("ghp_0123"));
    }

    #[test]
    fn remembers_recalls_and_forgets_across_instances() {
        let (m, base) = mem();
        m.guardar("O dono prefere respostas curtas e em português.", "conversa").unwrap();
        m.guardar("O projeto principal chama-se Compra Programada.", "conversa").unwrap();
        m.guardar("Nada a ver: receita de bolo de cenoura.", "conversa").unwrap();
        assert_eq!(m.quantidade(), 3);

        // Uma instância nova (mesma pasta e chave) ainda lê e busca: memória persiste.
        let key2 = {
            let bytes = std::fs::read(base.join("memoria").join("..").join("x")).err();
            let _ = bytes;
            m.key.clone()
        };
        let m2 = Memoria { dir: base.join("memoria"), key: key2 };
        let hits = m2.recall("como o dono prefere as respostas");
        assert!(hits.iter().any(|h| h.contains("respostas curtas")), "deveria lembrar da preferência: {hits:?}");
        assert!(!hits.iter().any(|h| h.contains("bolo de cenoura")), "não deveria trazer o irrelevante");

        let all = m.todas();
        let id = &all[0].id;
        assert!(m.esquecer(id).unwrap());
        assert_eq!(m.quantidade(), 2);
        assert!(!m.esquecer("zz").unwrap());
    }

    #[test]
    fn secret_only_memory_is_not_saved() {
        let (m, _) = mem();
        assert!(m.guardar("sk-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "conversa").unwrap().is_none());
        assert_eq!(m.quantidade(), 0);
    }

    #[test]
    fn stored_file_is_encrypted_at_rest() {
        let (m, base) = mem();
        m.guardar("segredo comercial do dono", "conversa").unwrap();
        let arq = std::fs::read_dir(base.join("memoria")).unwrap().next().unwrap().unwrap().path();
        let bruto = std::fs::read(arq).unwrap();
        assert!(!String::from_utf8_lossy(&bruto).contains("segredo comercial"), "não pode estar em texto puro");
    }
}
