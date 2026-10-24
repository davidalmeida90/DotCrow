//! Busca de arquivos por nome e por conteúdo, sem depender de programas externos.
//!
//! Serve ao dono (`/buscar` no modo terminal) e aos corvos (ferramenta `buscar`).
//! Pula as pastas que só fazem barulho (`.git`, `node_modules`, `vendor`, `target`...)
//! e para cedo em pastas gigantes, para responder rápido até no celular.

use std::path::{Path, PathBuf};

/// Pastas que nunca entram na busca.
pub const PASTAS_IGNORADAS: [&str; 12] = [
    ".git",
    "node_modules",
    "vendor",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    ".gradle",
    ".idea",
    ".next",
    ".dart_tool",
];

/// Arquivo maior que isto não tem o conteúdo lido na busca por texto.
const MAX_BYTES_TEXTO: u64 = 2 * 1024 * 1024;
/// Quantos itens a busca visita no máximo (evita varrer o disco inteiro).
const MAX_VISITADOS: usize = 60_000;
/// Profundidade máxima de pastas.
const MAX_PROFUNDIDADE: usize = 14;

/// Um resultado da busca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achado {
    /// Arquivo encontrado.
    pub caminho: PathBuf,
    /// Linha (número e texto) quando a busca foi por conteúdo.
    pub linha: Option<(usize, String)>,
}

/// O que procurar.
#[derive(Debug, Clone, Default)]
pub struct Consulta {
    /// Parte do nome do arquivo (sem diferenciar maiúsculas). Aceita `*` como curinga.
    pub nome: Option<String>,
    /// Texto dentro do arquivo (sem diferenciar maiúsculas).
    pub texto: Option<String>,
    /// Incluir arquivos e pastas ocultos (que começam com ponto).
    pub ocultos: bool,
}

/// Resultado de uma busca: os achados e se ela parou antes de olhar tudo.
#[derive(Debug, Clone, Default)]
pub struct Resultado {
    /// Achados, na ordem em que apareceram.
    pub achados: Vec<Achado>,
    /// Verdadeiro se bateu o limite de resultados ou de itens visitados.
    pub cortado: bool,
}

/// Procura em `raizes` (pastas ou arquivos) até `limite` achados.
pub fn buscar(raizes: &[PathBuf], consulta: &Consulta, limite: usize) -> Resultado {
    let nome = consulta.nome.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_lowercase);
    let texto = consulta.texto.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_lowercase);
    let mut out = Resultado::default();
    if nome.is_none() && texto.is_none() {
        return out;
    }
    let mut visitados = 0usize;
    let mut pilha: Vec<(PathBuf, usize)> = raizes.iter().rev().map(|r| (r.clone(), 0)).collect();
    while let Some((p, prof)) = pilha.pop() {
        visitados += 1;
        if visitados > MAX_VISITADOS || out.achados.len() >= limite {
            out.cortado = true;
            break;
        }
        let Ok(meta) = std::fs::symlink_metadata(&p) else { continue };
        if meta.is_dir() {
            if prof >= MAX_PROFUNDIDADE {
                continue;
            }
            let Ok(rd) = std::fs::read_dir(&p) else { continue };
            let mut filhos: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|c| {
                    let n = c.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    (consulta.ocultos || !n.starts_with('.')) && !PASTAS_IGNORADAS.contains(&n.as_str())
                })
                .collect();
            filhos.sort();
            for f in filhos.into_iter().rev() {
                pilha.push((f, prof + 1));
            }
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let nome_arq = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        if let Some(n) = &nome
            && !casa_nome(&nome_arq, n)
        {
            continue;
        }
        match &texto {
            None => out.achados.push(Achado { caminho: p, linha: None }),
            Some(t) => {
                if meta.len() > MAX_BYTES_TEXTO {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&p) else { continue };
                if parece_binario(&bytes) {
                    continue;
                }
                let conteudo = String::from_utf8_lossy(&bytes);
                for (i, l) in conteudo.lines().enumerate() {
                    if l.to_lowercase().contains(t.as_str()) {
                        let trecho: String = l.trim().chars().take(160).collect();
                        out.achados.push(Achado { caminho: p.clone(), linha: Some((i + 1, trecho)) });
                        if out.achados.len() >= limite {
                            break;
                        }
                    }
                }
            }
        }
    }
    out
}

/// `padrao` já em minúsculas. Sem `*`, basta conter; com `*`, cada pedaço na ordem.
fn casa_nome(nome: &str, padrao: &str) -> bool {
    if !padrao.contains('*') {
        return nome.contains(padrao);
    }
    let pedacos: Vec<&str> = padrao.split('*').collect();
    let mut resto = nome;
    for (i, pedaco) in pedacos.iter().enumerate() {
        if pedaco.is_empty() {
            continue;
        }
        if i == 0 {
            let Some(r) = resto.strip_prefix(pedaco) else { return false };
            resto = r;
        } else if i == pedacos.len() - 1 {
            return resto.ends_with(pedaco);
        } else {
            let Some(pos) = resto.find(pedaco) else { return false };
            resto = &resto[pos + pedaco.len()..];
        }
    }
    true
}

/// Binário se tiver byte zero no começo (o mesmo critério do git).
pub fn parece_binario(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|b| *b == 0)
}

/// Caminho para mostrar: relativo a uma das raízes quando possível.
pub fn exibir(p: &Path, raizes: &[PathBuf]) -> String {
    for r in raizes {
        if let Ok(rel) = p.strip_prefix(r)
            && !rel.as_os_str().is_empty()
        {
            return rel.display().to_string();
        }
    }
    p.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta() -> PathBuf {
        let d = std::env::temp_dir().join(format!("dotcrow-busca-{}-{}", std::process::id(), rand_sufixo()));
        std::fs::create_dir_all(d.join("admin")).unwrap();
        std::fs::create_dir_all(d.join("node_modules/lib")).unwrap();
        std::fs::write(d.join("notas.php"), "<?php\n$titulo = 'Notas';\necho $titulo;\n").unwrap();
        std::fs::write(d.join("admin/painel.php"), "<?php // painel de notas\n").unwrap();
        std::fs::write(d.join("node_modules/lib/notas.js"), "notas").unwrap();
        std::fs::write(d.join("logo.png"), [0u8, 1, 2, 3]).unwrap();
        d
    }

    fn rand_sufixo() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    #[test]
    fn por_nome_pula_node_modules() {
        let d = pasta();
        let r = buscar(std::slice::from_ref(&d), &Consulta { nome: Some("notas".into()), ..Default::default() }, 50);
        let nomes: Vec<String> = r.achados.iter().map(|a| exibir(&a.caminho, std::slice::from_ref(&d))).collect();
        assert_eq!(nomes, vec!["notas.php"]);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn por_texto_mostra_linha() {
        let d = pasta();
        let r = buscar(std::slice::from_ref(&d), &Consulta { texto: Some("NOTAS".into()), ..Default::default() }, 50);
        assert!(r.achados.iter().any(|a| a.linha == Some((2, "$titulo = 'Notas';".into()))));
        assert!(r.achados.iter().any(|a| a.caminho.ends_with("admin/painel.php")));
        assert!(!r.achados.iter().any(|a| a.caminho.ends_with("logo.png")));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn curinga_no_nome() {
        assert!(casa_nome("notas.php", "*.php"));
        assert!(casa_nome("notas.php", "no*php"));
        assert!(!casa_nome("notas.js", "*.php"));
        assert!(casa_nome("painel-notas.php", "*notas*"));
    }
}
