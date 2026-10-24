//! Ver arquivos do aparelho sem sair do DotCrow: navegador de pastas numerado,
//! prévia de arquivos com número de linha e busca.
//!
//! Pensado para o celular também: tudo se faz digitando um número ou uma letra.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{
    entrada::{Entrada, Leitura},
    projeto::{Projeto, expandir},
};
use crate::{Paint, busca};

/// Um item de uma pasta.
#[derive(Debug, Clone)]
pub struct Item {
    /// Nome do arquivo ou pasta.
    pub nome: String,
    /// Caminho completo.
    pub caminho: PathBuf,
    /// É pasta.
    pub pasta: bool,
    /// Tamanho em bytes (arquivos).
    pub tamanho: u64,
}

/// Lista uma pasta: pastas primeiro, depois arquivos, em ordem alfabética.
pub fn listar(dir: &Path, ocultos: bool) -> std::io::Result<Vec<Item>> {
    let mut itens: Vec<Item> = std::fs::read_dir(dir)?
        .flatten()
        .filter_map(|e| {
            let nome = e.file_name().to_string_lossy().to_string();
            if !ocultos && nome.starts_with('.') {
                return None;
            }
            let caminho = e.path();
            // `metadata` segue links (o ~/storage/shared do Termux é um link).
            let meta = std::fs::metadata(&caminho).ok();
            let pasta = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let tamanho = meta.map(|m| m.len()).unwrap_or(0);
            Some(Item { nome, caminho, pasta, tamanho })
        })
        .collect();
    itens.sort_by(|a, b| b.pasta.cmp(&a.pasta).then_with(|| a.nome.to_lowercase().cmp(&b.nome.to_lowercase())));
    Ok(itens)
}

/// Tamanho legível.
pub fn tamanho(b: u64) -> String {
    match b {
        0..1024 => format!("{b} B"),
        1024..1_048_576 => format!("{:.1} KB", b as f64 / 1024.0),
        1_048_576..1_073_741_824 => format!("{:.1} MB", b as f64 / 1_048_576.0),
        _ => format!("{:.1} GB", b as f64 / 1_073_741_824.0),
    }
}

/// Prévia de um arquivo de texto com números de linha, a partir de `inicio` (1 = topo).
pub fn previa(p: &Path, inicio: usize, linhas: usize, paint: Paint) -> String {
    let meta = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(e) => return format!("não consegui abrir {}: {e}", p.display()),
    };
    if meta.is_dir() {
        return match listar(p, false) {
            Ok(itens) => {
                let mut t = format!("{} é uma pasta com {} itens:\n", p.display(), itens.len());
                for i in itens.iter().take(60) {
                    t.push_str(&format!("  {}{}\n", i.nome, if i.pasta { "/" } else { "" }));
                }
                t
            }
            Err(e) => format!("não consegui listar {}: {e}", p.display()),
        };
    }
    if meta.len() > 20 * 1024 * 1024 {
        return format!("{} tem {} — grande demais para mostrar aqui.", p.display(), tamanho(meta.len()));
    }
    let bytes = match std::fs::read(p) {
        Ok(b) => b,
        Err(e) => return format!("não consegui ler {}: {e}", p.display()),
    };
    if busca::parece_binario(&bytes) {
        let tipo = p.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_else(|| "binário".into());
        return format!(
            "{} é um arquivo {tipo} ({}), não é texto. Os corvos podem usá-lo mesmo assim (copiar, enviar, converter).",
            p.display(),
            tamanho(meta.len())
        );
    }
    let texto = String::from_utf8_lossy(&bytes);
    let total = texto.lines().count();
    let inicio = inicio.clamp(1, total.max(1));
    let largura = total.to_string().len().max(3);
    let mut out = paint.dim(&format!("── {} · {total} linhas · {} ──", p.display(), tamanho(meta.len())));
    out.push('\n');
    for (i, l) in texto.lines().enumerate().skip(inicio - 1).take(linhas) {
        let l: String = l.chars().take(400).collect();
        out.push_str(&format!("{} {l}\n", paint.dim(&format!("{:>largura$} │", i + 1))));
    }
    let fim = (inicio - 1 + linhas).min(total);
    if fim < total {
        out.push_str(&paint.dim(&format!(
            "… mais {} linhas · continue com /ver {} {}",
            total - fim,
            p.display(),
            fim + 1
        )));
        out.push('\n');
    }
    out
}

/// O que o navegador devolve para o modo terminal.
#[derive(Debug, Default)]
pub struct Saida {
    /// Arquivos que o dono marcou para anexar ao próximo pedido.
    pub anexar: Vec<PathBuf>,
    /// Pasta que o dono quer pôr no projeto.
    pub adicionar: Option<PathBuf>,
}

/// Navegador de pastas interativo. Começa em `inicio` e devolve o que o dono escolheu.
pub async fn navegar(entrada: &Arc<Entrada>, projeto: &Projeto, inicio: PathBuf, paint: Paint) -> Saida {
    let mut saida = Saida::default();
    let mut dir = inicio;
    let mut ocultos = false;
    let mut pagina = 0usize;
    const POR_PAGINA: usize = 40;
    let shared = expandir("~/storage/shared", &projeto.raiz);
    // A lista só volta à tela quando muda (pasta, página, ocultos) ou com `l`: depois de
    // ver um arquivo, a prévia fica visível, o que faz diferença no celular.
    let mut mostrar = true;
    loop {
        let itens = match listar(&dir, ocultos) {
            Ok(i) => i,
            Err(e) => {
                println!("{}", paint.red(&format!("não consegui abrir {}: {e}", dir.display())));
                match dir.parent() {
                    Some(pai) => {
                        dir = pai.to_path_buf();
                        continue;
                    }
                    None => return saida,
                }
            }
        };
        let paginas = itens.len().div_ceil(POR_PAGINA).max(1);
        pagina = pagina.min(paginas - 1);
        if mostrar {
            mostrar_lista(projeto, &dir, &itens, pagina, paginas, shared.is_dir(), paint);
        }
        mostrar = true;
        let linha = match entrada.ler("arquivos › ", &paint.cyan("arquivos › "), false).await {
            Leitura::Linha(l) => l.trim().to_string(),
            Leitura::Interrompido | Leitura::Fim => return saida,
        };
        let numero =
            |s: &str| s.trim().parse::<usize>().ok().filter(|n| *n >= 1 && *n <= itens.len()).map(|n| &itens[n - 1]);
        match linha.as_str() {
            "" | "q" | "sair" => return saida,
            "l" => {}
            "0" | ".." => {
                if let Some(pai) = dir.parent() {
                    dir = pai.to_path_buf();
                    pagina = 0;
                }
            }
            "~" => {
                dir = expandir("~", &projeto.raiz);
                pagina = 0;
            }
            "p" => {
                dir = projeto.raiz.clone();
                pagina = 0;
            }
            "s" if shared.is_dir() => {
                dir = shared.clone();
                pagina = 0;
            }
            "d" => {
                dir = crate::maos::pasta_de_downloads();
                pagina = 0;
            }
            "." => ocultos = !ocultos,
            "m" => pagina = (pagina + 1).min(paginas - 1),
            "M" => pagina = pagina.saturating_sub(1),
            "+" => {
                if projeto.acesso(&dir).is_some() && dir.starts_with(&projeto.raiz) {
                    println!("{}", paint.dim("Esta pasta já faz parte do projeto (está dentro da pasta principal)."));
                    mostrar = false;
                } else {
                    saida.adicionar = Some(dir.clone());
                    return saida;
                }
            }
            outra => {
                // Ver ou anexar não muda a pasta: a lista não precisa voltar à tela.
                mostrar = false;
                if let Some(resto) = outra.strip_prefix('@') {
                    match numero(resto) {
                        Some(it) if !it.pasta => {
                            println!("{}", paint.green(&format!("📎 {} vai junto no próximo pedido", it.nome)));
                            saida.anexar.push(it.caminho.clone());
                        }
                        Some(it) => {
                            println!(
                                "{}",
                                paint.green(&format!("📎 a lista da pasta {}/ vai junto no próximo pedido", it.nome))
                            );
                            saida.anexar.push(it.caminho.clone());
                        }
                        None => println!("{}", paint.yellow("número fora da lista")),
                    }
                } else if let Some(resto) = outra.strip_prefix("v ").or_else(|| outra.strip_prefix("ver ")) {
                    match numero(resto) {
                        Some(it) => print!("{}", previa(&it.caminho, 1, 60, paint)),
                        None => println!("{}", paint.yellow("número fora da lista")),
                    }
                } else if let Some(it) = numero(outra) {
                    if it.pasta {
                        dir = it.caminho.clone();
                        pagina = 0;
                        mostrar = true;
                    } else {
                        print!("{}", previa(&it.caminho, 1, 40, paint));
                    }
                } else {
                    // Um caminho digitado direto (~/site, /sdcard, ../outro).
                    let alvo = expandir(outra, &dir);
                    if alvo.is_dir() {
                        dir = crate::maos::normalizar(&alvo);
                        pagina = 0;
                        mostrar = true;
                    } else if alvo.is_file() {
                        print!("{}", previa(&alvo, 1, 40, paint));
                    } else {
                        println!("{}", paint.yellow("não entendi: digite um número, um caminho ou Enter para sair"));
                    }
                }
            }
        }
    }
}

/// Mostra a pasta: cabeçalho, itens numerados da página e as dicas.
fn mostrar_lista(
    projeto: &Projeto,
    dir: &Path,
    itens: &[Item],
    pagina: usize,
    paginas: usize,
    celular: bool,
    paint: Paint,
) {
    const POR_PAGINA: usize = 40;
    let marca = match projeto.acesso(dir) {
        Some(true) => paint.green("  [no projeto · leitura e escrita]"),
        Some(false) => paint.yellow("  [no projeto · só leitura]"),
        None => String::new(),
    };
    println!("\n{}{marca}", paint.bold(&format!("📂 {}", dir.display())));
    println!("   {}  ..", paint.dim(" 0"));
    for (i, it) in itens.iter().enumerate().skip(pagina * POR_PAGINA).take(POR_PAGINA) {
        let n = paint.dim(&format!("{:>2}", i + 1));
        if it.pasta {
            println!("   {n}  {}", paint.cyan(&format!("{}/", it.nome)));
        } else {
            println!("   {n}  {:<38} {}", it.nome, paint.dim(&tamanho(it.tamanho)));
        }
    }
    if itens.is_empty() {
        println!("   {}", paint.dim("(pasta vazia)"));
    }
    if paginas > 1 {
        println!("   {}", paint.dim(&format!("página {} de {paginas} · m = mais · M = voltar página", pagina + 1)));
    }
    let mut dicas =
        vec!["N abre", "v N vê", "@N anexa", "+ põe a pasta no projeto", "~ pessoal", "p projeto", "d downloads"];
    if celular {
        dicas.push("s celular");
    }
    dicas.extend([". ocultos", "l lista", "Enter sai"]);
    println!("   {}", paint.dim(&dicas.join(" · ")));
}

/// Busca por nome e por conteúdo nas pastas do projeto, com resultados numerados.
pub fn buscar(projeto: &Projeto, termo: &str) -> (Vec<PathBuf>, String) {
    let raizes = projeto.raizes();
    let por_nome = busca::buscar(&raizes, &busca::Consulta { nome: Some(termo.into()), ..Default::default() }, 40);
    let por_texto = busca::buscar(&raizes, &busca::Consulta { texto: Some(termo.into()), ..Default::default() }, 60);
    let mut arquivos = Vec::new();
    let mut t = String::new();
    if !por_nome.achados.is_empty() {
        t.push_str("Nomes:\n");
        for a in &por_nome.achados {
            arquivos.push(a.caminho.clone());
            t.push_str(&format!("  {:>2}  {}\n", arquivos.len(), busca::exibir(&a.caminho, &raizes)));
        }
    }
    if !por_texto.achados.is_empty() {
        t.push_str("Conteúdo:\n");
        for a in &por_texto.achados {
            let (n, l) = a.linha.clone().unwrap_or((0, String::new()));
            arquivos.push(a.caminho.clone());
            t.push_str(&format!("  {:>2}  {}:{n}  {l}\n", arquivos.len(), busca::exibir(&a.caminho, &raizes)));
        }
    }
    if por_nome.cortado || por_texto.cortado {
        t.push_str("  … há mais resultados; refine a busca\n");
    }
    (arquivos, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previa_mostra_linhas_e_continua() {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let d = std::env::temp_dir().join(format!("dotcrow-arq-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let arq = d.join("notas.php");
        let conteudo: String = (1..=100).map(|i| format!("linha {i}\n")).collect();
        std::fs::write(&arq, conteudo).unwrap();
        let p = Paint { on: false };
        let t = previa(&arq, 10, 5, p);
        assert!(t.contains(" 10 │ linha 10"));
        assert!(t.contains(" 14 │ linha 14"));
        assert!(!t.contains("linha 15\n"));
        assert!(t.contains("mais 86 linhas"));
        std::fs::write(d.join("logo.png"), [0u8, 1, 2]).unwrap();
        assert!(previa(&d.join("logo.png"), 1, 5, p).contains("não é texto"));
        let itens = listar(&d, false).unwrap();
        assert_eq!(itens.len(), 2);
        let _ = std::fs::remove_dir_all(d);
    }
}
