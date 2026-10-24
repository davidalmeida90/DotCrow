//! O projeto de uma pasta: as pastas que participam, o perfil e o que mais o DotCrow
//! lembra dela entre uma vez e outra.
//!
//! Tudo fica na pasta de dados do DotCrow (`projetos/<nome>-<código>`), **fora** do
//! projeto: assim cópias de arquivos e a conversa não sobem para o servidor junto com o
//! site, e os corvos não conseguem mexer nelas (a pasta de dados é o cofre).

use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{busca::PASTAS_IGNORADAS, maos};

/// Uma pasta extra do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PastaCfg {
    /// Caminho absoluto.
    pub caminho: PathBuf,
    /// Os corvos podem gravar nela sem perguntar.
    pub escrita: bool,
    /// Para que serve, nas palavras do dono.
    #[serde(default)]
    pub nota: Option<String>,
}

/// O que fica salvo de um projeto.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjetoCfg {
    /// A pasta principal (para conferência).
    #[serde(default)]
    pub raiz: Option<PathBuf>,
    /// As outras pastas que participam.
    #[serde(default)]
    pub pastas: Vec<PastaCfg>,
    /// Verdadeiro = o Huginn não pergunta antes das tarefas.
    #[serde(default)]
    pub direto: bool,
    /// Modo das mãos (sandbox, computador ou livre).
    #[serde(default)]
    pub modo: Option<String>,
    /// O dono não quis a entrevista inicial (não oferece de novo; `/alinhar` faz quando quiser).
    #[serde(default)]
    pub sem_entrevista: bool,
}

/// Um projeto aberto no modo terminal.
pub struct Projeto {
    /// A pasta principal: onde os corvos trabalham e o diretório do terminal.
    pub raiz: PathBuf,
    /// Onde o DotCrow guarda as coisas deste projeto.
    pub dados: PathBuf,
    /// O que fica salvo.
    pub cfg: ProjetoCfg,
}

impl Projeto {
    /// Abre (ou cria) o projeto da pasta `raiz`, guardando os dados dentro de `cofre`.
    pub fn abrir(cofre: &Path, raiz: PathBuf) -> io::Result<Self> {
        let raiz = maos::normalizar(&raiz);
        let nome: String = raiz
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "raiz".into())
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .take(40)
            .collect();
        let dados = cofre.join("projetos").join(format!("{nome}-{}", codigo(&raiz)));
        std::fs::create_dir_all(&dados)?;
        let cfg = std::fs::read(dados.join("projeto.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<ProjetoCfg>(&b).ok())
            .unwrap_or_default();
        let mut p = Self { raiz, dados, cfg };
        p.cfg.raiz = Some(p.raiz.clone());
        Ok(p)
    }

    /// Grava a configuração.
    pub fn salvar(&self) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(&self.cfg).map_err(io::Error::other)?;
        std::fs::write(self.dados.join("projeto.json"), json)
    }

    /// Nome curto para o prompt.
    pub fn nome(&self) -> String {
        self.raiz
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.raiz.display().to_string())
    }

    /// O perfil do projeto (o que o dono contou na entrevista), se já existir.
    pub fn perfil(&self) -> Option<String> {
        std::fs::read_to_string(self.dados.join("perfil.md")).ok().filter(|t| !t.trim().is_empty())
    }

    /// Grava o perfil do projeto.
    pub fn salvar_perfil(&self, texto: &str) -> io::Result<()> {
        std::fs::write(self.dados.join("perfil.md"), format!("{}\n", texto.trim()))
    }

    /// As pastas extras que ainda existem, no formato das mãos.
    pub fn extras(&self) -> Vec<maos::PastaExtra> {
        self.cfg
            .pastas
            .iter()
            .filter(|p| p.caminho.is_dir())
            .map(|p| maos::PastaExtra { caminho: p.caminho.clone(), escrita: p.escrita })
            .collect()
    }

    /// A pasta principal e as extras.
    pub fn raizes(&self) -> Vec<PathBuf> {
        std::iter::once(self.raiz.clone()).chain(self.cfg.pastas.iter().map(|p| p.caminho.clone())).collect()
    }

    /// Se `p` está dentro do projeto e se dá para gravar lá sem perguntar.
    pub fn acesso(&self, p: &Path) -> Option<bool> {
        if p.starts_with(&self.raiz) {
            return Some(true);
        }
        self.cfg.pastas.iter().filter(|e| p.starts_with(&e.caminho)).map(|e| e.escrita).max()
    }

    /// Confere se a pasta pode entrar no projeto (antes de perguntar qualquer coisa ao dono).
    pub fn validar_pasta(&self, caminho: &Path, cofre: &Path) -> Result<PathBuf, String> {
        let c = maos::normalizar(caminho);
        if !c.is_dir() {
            return Err(format!("{} não é uma pasta que existe", c.display()));
        }
        if c.parent().is_none() {
            return Err("a raiz do sistema inteira não pode entrar no projeto".into());
        }
        if c.starts_with(cofre) || cofre.starts_with(&c) {
            return Err("a pasta de dados do DotCrow (onde fica o cofre de chaves) não pode entrar no projeto".into());
        }
        if c.starts_with(&self.raiz) {
            return Err("essa pasta já está dentro da pasta principal do projeto".into());
        }
        Ok(c)
    }

    /// Põe uma pasta no projeto (ou troca o acesso, se já estiver).
    pub fn adicionar_pasta(
        &mut self,
        caminho: &Path,
        escrita: bool,
        nota: Option<String>,
        cofre: &Path,
    ) -> Result<PathBuf, String> {
        let c = self.validar_pasta(caminho, cofre)?;
        let nota = nota.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        match self.cfg.pastas.iter_mut().find(|p| p.caminho == c) {
            Some(p) => {
                p.escrita = escrita;
                if nota.is_some() {
                    p.nota = nota;
                }
            }
            None => self.cfg.pastas.push(PastaCfg { caminho: c.clone(), escrita, nota }),
        }
        self.salvar().map_err(|e| format!("não consegui salvar: {e}"))?;
        Ok(c)
    }

    /// Tira uma pasta extra, pelo número de `/pastas` (2 em diante) ou pelo caminho.
    pub fn remover_pasta(&mut self, alvo: &str, base: &Path) -> Option<PathBuf> {
        let alvo = alvo.trim();
        let idx = match alvo.parse::<usize>() {
            Ok(n) if n >= 2 => Some(n - 2),
            Ok(_) => None,
            Err(_) => {
                let c = maos::normalizar(&expandir(alvo, base));
                self.cfg.pastas.iter().position(|p| p.caminho == c)
            }
        }?;
        if idx >= self.cfg.pastas.len() {
            return None;
        }
        let tirada = self.cfg.pastas.remove(idx).caminho;
        let _ = self.salvar();
        Some(tirada)
    }

    /// Pasta dos diários de desfazer.
    pub fn pasta_desfazer(&self) -> PathBuf {
        self.dados.join("desfazer")
    }

    /// Acrescenta um bloco à conversa guardada deste projeto.
    pub fn anotar_conversa(&self, bloco: &str) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.dados.join("conversa.md")) {
            let _ = f.write_all(bloco.as_bytes());
        }
    }

    /// As pastas, com acesso e nota, em texto para os corvos.
    pub fn descrever_pastas(&self) -> String {
        let mut t = format!("- {} (pasta principal, leitura e escrita)", self.raiz.display());
        for p in &self.cfg.pastas {
            let acesso = if p.escrita { "leitura e escrita" } else { "só leitura" };
            t.push_str(&format!("\n- {} ({acesso})", p.caminho.display()));
            if let Some(n) = &p.nota {
                t.push_str(&format!(" — {n}"));
            }
        }
        t
    }

    /// Um retrato curto da pasta (estrutura e arquivos de descrição) para o Huginn
    /// entender o projeto antes de perguntar.
    pub fn retrato(&self) -> String {
        const MAX: usize = 9000;
        let mut t = format!("Pasta principal: {}\n\n## Estrutura\n", self.raiz.display());
        arvore(&self.raiz, 0, 2, &mut t, &mut 0);
        for p in &self.cfg.pastas {
            t.push_str(&format!(
                "\n## Pasta extra: {} ({})\n",
                p.caminho.display(),
                if p.escrita { "leitura e escrita" } else { "só leitura" }
            ));
            if let Some(n) = &p.nota {
                t.push_str(&format!("Para que serve: {n}\n"));
            }
            arvore(&p.caminho, 0, 1, &mut t, &mut 0);
        }
        for nome in [
            "README.md",
            "README.txt",
            "README",
            "LEIA-ME.md",
            "LEIAME.md",
            "composer.json",
            "package.json",
            "Cargo.toml",
            "pubspec.yaml",
            "requirements.txt",
        ] {
            let arq = self.raiz.join(nome);
            if let Ok(conteudo) = std::fs::read_to_string(&arq) {
                let trecho: Vec<&str> = conteudo.lines().take(40).collect();
                t.push_str(&format!("\n## {nome} (começo)\n{}\n", trecho.join("\n")));
            }
            if t.len() > MAX {
                break;
            }
        }
        if t.len() > MAX {
            let mut corte = MAX;
            while !t.is_char_boundary(corte) {
                corte -= 1;
            }
            t.truncate(corte);
            t.push_str("\n[...]");
        }
        t
    }
}

/// Árvore simples de pastas e arquivos, até `max_prof`, com limite de linhas.
fn arvore(dir: &Path, prof: usize, max_prof: usize, out: &mut String, linhas: &mut usize) {
    const MAX_LINHAS: usize = 140;
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut itens: Vec<(bool, String, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let nome = e.file_name().to_string_lossy().to_string();
            if nome.starts_with('.') || PASTAS_IGNORADAS.contains(&nome.as_str()) {
                return None;
            }
            let eh_pasta = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            Some((!eh_pasta, nome, e.path()))
        })
        .collect();
    itens.sort();
    for (eh_arquivo, nome, caminho) in itens {
        if *linhas >= MAX_LINHAS {
            out.push_str(&format!("{}...\n", "  ".repeat(prof)));
            return;
        }
        *linhas += 1;
        if eh_arquivo {
            out.push_str(&format!("{}{nome}\n", "  ".repeat(prof)));
        } else {
            out.push_str(&format!("{}{nome}/\n", "  ".repeat(prof)));
            if prof + 1 < max_prof {
                arvore(&caminho, prof + 1, max_prof, out, linhas);
            }
        }
    }
}

/// `~` vira a pasta pessoal; relativo vira relativo a `base`.
pub fn expandir(caminho: &str, base: &Path) -> PathBuf {
    let c = caminho.trim().trim_matches(|ch| ch == '"' || ch == '\'');
    let home = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf());
    if c == "~" {
        return home.unwrap_or_else(|| base.to_path_buf());
    }
    if let (Some(rest), Some(h)) = (c.strip_prefix("~/"), home) {
        return h.join(rest);
    }
    let p = PathBuf::from(c);
    if p.is_absolute() { p } else { base.join(p) }
}

/// Código curto e estável de um caminho (FNV-1a), para separar projetos de mesmo nome.
pub fn codigo(p: &Path) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in p.to_string_lossy().as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:08x}", h as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(nome: &str) -> PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let d = std::env::temp_dir().join(format!("dotcrow-proj-{nome}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn pastas_ficam_salvas_e_voltam() {
        let cofre = tmp("cofre");
        let raiz = tmp("site");
        let assets = tmp("assets");
        std::fs::write(raiz.join("notas.php"), "<?php").unwrap();

        let mut p = Projeto::abrir(&cofre, raiz.clone()).unwrap();
        assert!(p.adicionar_pasta(&assets, false, Some("imagens do site".into()), &cofre).is_ok());
        // Recusa: o cofre, uma subpasta da principal e algo que não existe.
        assert!(p.adicionar_pasta(&cofre, true, None, &cofre).is_err());
        std::fs::create_dir_all(raiz.join("admin")).unwrap();
        assert!(p.adicionar_pasta(&raiz.join("admin"), true, None, &cofre).is_err());
        assert!(p.adicionar_pasta(&raiz.join("nao-existe"), true, None, &cofre).is_err());

        let p2 = Projeto::abrir(&cofre, raiz.clone()).unwrap();
        assert_eq!(p2.cfg.pastas.len(), 1);
        assert_eq!(p2.acesso(&assets.join("logo.png")), Some(false));
        assert_eq!(p2.acesso(&raiz.join("notas.php")), Some(true));
        assert_eq!(p2.acesso(Path::new("/etc/passwd")), None);
        assert!(p2.descrever_pastas().contains("imagens do site"));
        assert!(p2.retrato().contains("notas.php"));

        let mut p3 = p2;
        assert_eq!(p3.remover_pasta("2", &raiz), Some(maos::normalizar(&assets)));
        assert!(p3.cfg.pastas.is_empty());
        for d in [cofre, raiz, assets] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn codigo_e_estavel() {
        assert_eq!(codigo(Path::new("/home/a/site")), codigo(Path::new("/home/a/site")));
        assert_ne!(codigo(Path::new("/home/a/site")), codigo(Path::new("/home/b/site")));
    }
}
