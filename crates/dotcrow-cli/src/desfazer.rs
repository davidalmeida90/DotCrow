//! Diário de alterações, para desfazer uma tarefa.
//!
//! Antes de um corvo mudar um arquivo do projeto pela primeira vez numa tarefa, uma
//! cópia do original vai para a pasta de dados do DotCrow (fora do projeto, para não
//! ir parar no servidor junto com o site). Arquivos que a tarefa criou ficam anotados
//! para serem apagados. `desfazer` volta tudo como estava antes da tarefa.
//!
//! Só cobre o que passa pelas ferramentas de arquivo (escrever, editar, baixar). O que
//! um comando de terminal muda por conta própria não entra: para isso, use o git.

use std::{
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

/// Arquivo maior que isso não ganha cópia (fica anotado como "sem cópia").
const MAX_COPIA: u64 = 25 * 1024 * 1024;

/// Uma alteração anotada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Item {
    /// O arquivo do projeto.
    pub original: PathBuf,
    /// Nome da cópia dentro da pasta do diário; `None` quando o arquivo não existia (foi criado).
    pub copia: Option<String>,
    /// Existia, mas era grande demais para guardar cópia.
    #[serde(default)]
    pub sem_copia: bool,
}

/// O índice gravado em disco, para desfazer até depois de fechar o DotCrow.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Indice {
    /// Quando a tarefa começou (segundos desde 1970).
    pub criado_em: u64,
    /// O pedido da tarefa.
    pub pedido: String,
    /// Arquivos alterados ou criados.
    pub itens: Vec<Item>,
    /// Já foi desfeito.
    #[serde(default)]
    pub desfeito: bool,
}

/// O diário de uma tarefa.
pub struct Diario {
    dir: PathBuf,
    indice: Mutex<Indice>,
}

impl Diario {
    /// Diário novo dentro de `raiz` (a pasta `desfazer` do projeto).
    pub fn novo(raiz: &Path, pedido: &str) -> io::Result<Self> {
        let agora = agora();
        let mut dir = raiz.join(format!("{agora:012}"));
        let mut n = 1;
        while dir.exists() {
            n += 1;
            dir = raiz.join(format!("{agora:012}-{n}"));
        }
        std::fs::create_dir_all(&dir)?;
        let indice = Indice { criado_em: agora, pedido: pedido.chars().take(300).collect(), ..Default::default() };
        let d = Self { dir, indice: Mutex::new(indice) };
        d.salvar()?;
        Ok(d)
    }

    /// Anota `p` antes de ele mudar. Só a primeira vez por arquivo conta (é o original).
    pub fn antes_de_mudar(&self, p: &Path) {
        let mut idx = self.indice.lock().unwrap_or_else(|e| e.into_inner());
        if idx.itens.iter().any(|i| i.original == p) {
            return;
        }
        let item = match std::fs::metadata(p) {
            Ok(m) if m.is_file() && m.len() <= MAX_COPIA => {
                let nome = format!("{:04}.copia", idx.itens.len() + 1);
                match std::fs::copy(p, self.dir.join(&nome)) {
                    Ok(_) => Item { original: p.to_path_buf(), copia: Some(nome), sem_copia: false },
                    Err(_) => Item { original: p.to_path_buf(), copia: None, sem_copia: true },
                }
            }
            Ok(m) if m.is_file() => Item { original: p.to_path_buf(), copia: None, sem_copia: true },
            // Pasta ou outra coisa: não é com o diário.
            Ok(_) => return,
            Err(_) => Item { original: p.to_path_buf(), copia: None, sem_copia: false },
        };
        idx.itens.push(item);
        drop(idx);
        let _ = self.salvar();
    }

    /// Quantos arquivos a tarefa mudou ou criou.
    pub fn quantos(&self) -> usize {
        self.indice.lock().map(|i| i.itens.len()).unwrap_or(0)
    }

    /// Os itens anotados.
    pub fn itens(&self) -> Vec<Item> {
        self.indice.lock().map(|i| i.itens.clone()).unwrap_or_default()
    }

    /// Apaga a pasta do diário se a tarefa não mudou nada (para não acumular lixo).
    pub fn descartar_se_vazio(&self) {
        if self.quantos() == 0 {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn salvar(&self) -> io::Result<()> {
        let idx = self.indice.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let json = serde_json::to_vec_pretty(&idx).map_err(io::Error::other)?;
        std::fs::write(self.dir.join("indice.json"), json)
    }
}

/// O diário mais recente que ainda pode ser desfeito (e que mudou alguma coisa).
pub fn ultimo(raiz: &Path) -> Option<(PathBuf, Indice)> {
    let mut dirs: Vec<PathBuf> =
        std::fs::read_dir(raiz).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.into_iter().rev().find_map(|d| {
        let idx = ler(&d)?;
        (!idx.desfeito && !idx.itens.is_empty()).then_some((d, idx))
    })
}

fn ler(dir: &Path) -> Option<Indice> {
    serde_json::from_slice(&std::fs::read(dir.join("indice.json")).ok()?).ok()
}

/// Volta os arquivos do diário em `dir` ao que eram antes da tarefa. Devolve uma linha
/// por arquivo dizendo o que foi feito.
pub fn desfazer(dir: &Path) -> io::Result<Vec<String>> {
    let mut idx = ler(dir).ok_or_else(|| io::Error::other("diário não encontrado"))?;
    if idx.desfeito {
        return Err(io::Error::other("essa tarefa já foi desfeita"));
    }
    let mut linhas = Vec::new();
    for item in &idx.itens {
        let alvo = &item.original;
        let msg = match (&item.copia, item.sem_copia) {
            (Some(nome), _) => {
                if let Some(pai) = alvo.parent() {
                    let _ = std::fs::create_dir_all(pai);
                }
                match std::fs::copy(dir.join(nome), alvo) {
                    Ok(_) => format!("↩ voltou ao original: {}", alvo.display()),
                    Err(e) => format!("✗ não consegui restaurar {}: {e}", alvo.display()),
                }
            }
            (None, true) => format!("! sem cópia (era grande demais): {}", alvo.display()),
            (None, false) => {
                if alvo.is_file() {
                    match std::fs::remove_file(alvo) {
                        Ok(()) => format!("🗑 apagado (a tarefa tinha criado): {}", alvo.display()),
                        Err(e) => format!("✗ não consegui apagar {}: {e}", alvo.display()),
                    }
                } else {
                    format!("· já não existia: {}", alvo.display())
                }
            }
        };
        linhas.push(msg);
    }
    idx.desfeito = true;
    let json = serde_json::to_vec_pretty(&idx).map_err(io::Error::other)?;
    std::fs::write(dir.join("indice.json"), json)?;
    Ok(linhas)
}

fn agora() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(nome: &str) -> PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let d = std::env::temp_dir().join(format!("dotcrow-desfazer-{nome}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn desfaz_edicao_e_criacao() {
        let proj = tmp("proj");
        let dados = tmp("dados");
        let notas = proj.join("notas.php");
        std::fs::write(&notas, "original").unwrap();
        let novo = proj.join("novo.php");

        let d = Diario::novo(&dados, "mude o notas").unwrap();
        d.antes_de_mudar(&notas);
        std::fs::write(&notas, "mudado").unwrap();
        d.antes_de_mudar(&notas); // a segunda vez não sobrescreve a cópia do original
        std::fs::write(&notas, "mudado de novo").unwrap();
        d.antes_de_mudar(&novo);
        std::fs::write(&novo, "criado").unwrap();
        assert_eq!(d.quantos(), 2);

        let (dir, idx) = ultimo(&dados).expect("tem diário");
        assert_eq!(idx.pedido, "mude o notas");
        let linhas = desfazer(&dir).unwrap();
        assert_eq!(linhas.len(), 2);
        assert_eq!(std::fs::read_to_string(&notas).unwrap(), "original");
        assert!(!novo.exists());
        // Não dá para desfazer duas vezes, e o "último" passa a não existir.
        assert!(desfazer(&dir).is_err());
        assert!(ultimo(&dados).is_none());
        let _ = std::fs::remove_dir_all(proj);
        let _ = std::fs::remove_dir_all(dados);
    }
}
