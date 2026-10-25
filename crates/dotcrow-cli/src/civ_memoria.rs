//! A memória própria de cada corvo da civilização.
//!
//! Fica num arquivo JSON por corvo, na pasta da civilização (na Downloads), para
//! sobreviver a desligar o celular e até a reinstalar o Termux:
//!
//! - **falas**: o que ele disse nas conversas;
//! - **vínculos**: perguntas que outros corvos fizeram e a resposta dele (é isso que os
//!   jurados e o oráculo avaliam);
//! - **aprendizados**: o que ele trouxe das consultas à internet (o RAG dele);
//! - **lições**: o que ele concluiu ao refletir sobre o que viveu (como ser melhor);
//! - **pensamentos**: o que ele pensou sozinho, sem falar;
//! - **vetores**: cada lembrança importante ganha um embedding (vetor de ML, OpenAI
//!   `text-embedding-3-small`, 256 dimensões, guardado em 1 byte por número). O corvo
//!   lembra pelo significado; sem chave da OpenAI, lembra pelas palavras;
//! - **livro-razão**: cada acontecimento (nascimento, notas, prêmios, consultas, morte)
//!   num lançamento com o hash SHA-256 do anterior. Mudar um lançamento antigo quebra a
//!   corrente, e [`Memoria::integra`] percebe.
//!
//! O tamanho do arquivo respeita a memória (RAG) do corvo em MB: quando passa, as
//! lembranças mais antigas saem primeiro. O livro-razão nunca é apagado.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Uma fala, lição ou pensamento guardado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registro {
    /// Quando (segundos Unix).
    pub quando: u64,
    /// O texto.
    pub texto: String,
    /// Embedding (hex, 1 byte por dimensão), quando houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vetor: Option<String>,
}

/// Uma pergunta de outro corvo e a resposta deste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vinculo {
    /// Quando.
    pub quando: u64,
    /// Quem perguntou.
    pub quem_perguntou: String,
    /// A pergunta.
    pub pergunta: String,
    /// A resposta.
    pub resposta: String,
    /// Embedding (hex), quando houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vetor: Option<String>,
}

/// Algo aprendido numa consulta à internet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aprendizado {
    /// Quando.
    pub quando: u64,
    /// O que ele pesquisou.
    pub termo: String,
    /// Título da fonte.
    pub titulo: String,
    /// Endereço da fonte.
    pub fonte: String,
    /// O resumo que entrou na memória (já aprovado pela Alfândega).
    pub texto: String,
    /// Embedding (hex), quando houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vetor: Option<String>,
}

/// Um lançamento no livro-razão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lancamento {
    /// Número do lançamento (começa em 1).
    pub n: u64,
    /// Quando.
    pub quando: u64,
    /// Tipo: nascimento, nota, julgamento, premio, consulta, condenado, morte...
    pub tipo: String,
    /// O que aconteceu.
    pub descricao: String,
    /// Hash do lançamento anterior (64 zeros no primeiro).
    pub anterior: String,
    /// Hash deste lançamento.
    pub hash: String,
}

/// A memória de um corvo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memoria {
    /// Nome do corvo.
    pub nome: String,
    /// O que ele disse.
    #[serde(default)]
    pub falas: Vec<Registro>,
    /// Perguntas de outros corvos e as respostas dele.
    #[serde(default)]
    pub vinculos: Vec<Vinculo>,
    /// O que aprendeu na internet.
    #[serde(default)]
    pub aprendizados: Vec<Aprendizado>,
    /// Lições que tirou ao refletir.
    #[serde(default)]
    pub licoes: Vec<Registro>,
    /// O que pensou sozinho.
    #[serde(default)]
    pub pensamentos: Vec<Registro>,
    /// O livro-razão, com hashes encadeados.
    #[serde(default)]
    pub livro_razao: Vec<Lancamento>,
}

/// Segundos Unix agora.
pub fn agora() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn nome_de_arquivo(nome: &str) -> String {
    let limpo: String =
        nome.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if limpo.is_empty() { "corvo".into() } else { limpo }
}

/// Onde fica a memória de um corvo vivo.
pub fn arquivo(pasta: &Path, nome: &str) -> PathBuf {
    pasta.join("corvos").join(format!("{}.json", nome_de_arquivo(nome)))
}

fn hash_de(n: u64, quando: u64, tipo: &str, descricao: &str, anterior: &str) -> String {
    dotcrow_crypto::sha256_hex(format!("{n}\u{1f}{quando}\u{1f}{tipo}\u{1f}{descricao}\u{1f}{anterior}").as_bytes())
}

/// Guarda um embedding em 1 byte por dimensão (hex), para caber na memória do corvo.
pub fn compactar(v: &[f32]) -> String {
    let max = v.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-9);
    v.iter().map(|x| format!("{:02x}", ((x / max * 127.0).round() as i32).clamp(-127, 127) as i8 as u8)).collect()
}

/// Volta o embedding compactado para números.
pub fn expandir(hex: &str) -> Vec<f32> {
    (0..hex.len() / 2)
        .filter_map(|i| hex.get(i * 2..i * 2 + 2))
        .filter_map(|b| u8::from_str_radix(b, 16).ok())
        .map(|b| f32::from(b as i8) / 127.0)
        .collect()
}

/// Semelhança de cosseno entre dois vetores (0 se tiverem tamanhos diferentes).
pub fn cosseno(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut ab, mut aa, mut bb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    if aa == 0.0 || bb == 0.0 { 0.0 } else { ab / (aa.sqrt() * bb.sqrt()) }
}

fn palavras(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(str::to_string)
        .collect()
}

impl Memoria {
    /// Abre a memória do corvo (ou começa uma nova, com o lançamento de nascimento).
    pub fn abrir(pasta: &Path, nome: &str) -> Memoria {
        let caminho = arquivo(pasta, nome);
        if let Ok(bytes) = std::fs::read(&caminho)
            && let Ok(m) = serde_json::from_slice::<Memoria>(&bytes)
        {
            return m;
        }
        let mut m = Memoria { nome: nome.to_string(), ..Default::default() };
        m.registrar("nascimento", &format!("{nome} nasceu na civilização"));
        m
    }

    /// Acrescenta um lançamento ao livro-razão, encadeado ao anterior.
    pub fn registrar(&mut self, tipo: &str, descricao: &str) {
        let n = self.livro_razao.last().map(|l| l.n + 1).unwrap_or(1);
        let anterior = self.livro_razao.last().map(|l| l.hash.clone()).unwrap_or_else(|| "0".repeat(64));
        let quando = agora();
        let hash = hash_de(n, quando, tipo, descricao, &anterior);
        self.livro_razao.push(Lancamento { n, quando, tipo: tipo.into(), descricao: descricao.into(), anterior, hash });
    }

    /// Confere a corrente de hashes do livro-razão.
    pub fn integra(&self) -> bool {
        let mut anterior = "0".repeat(64);
        for l in &self.livro_razao {
            if l.anterior != anterior || l.hash != hash_de(l.n, l.quando, &l.tipo, &l.descricao, &l.anterior) {
                return false;
            }
            anterior = l.hash.clone();
        }
        true
    }

    /// Guarda uma fala.
    pub fn falou(&mut self, texto: &str) {
        self.falas.push(Registro { quando: agora(), texto: texto.into(), vetor: None });
    }

    /// Guarda um pensamento.
    pub fn pensou(&mut self, texto: &str) {
        self.pensamentos.push(Registro { quando: agora(), texto: texto.into(), vetor: None });
    }

    /// Guarda as lições de uma reflexão e lança no livro-razão.
    pub fn refletiu(&mut self, licoes: &[String], vetores: Option<Vec<Vec<f32>>>) {
        let vetores = vetores.unwrap_or_default();
        for (i, l) in licoes.iter().enumerate() {
            self.licoes.push(Registro {
                quando: agora(),
                texto: l.clone(),
                vetor: vetores.get(i).map(|v| compactar(v)),
            });
        }
        if !licoes.is_empty() {
            self.registrar("reflexao", &format!("aprendeu: {}", licoes.join(" | ")));
        }
    }

    /// As últimas `n` lições, da mais antiga para a mais nova.
    pub fn ultimas_licoes(&self, n: usize) -> Vec<String> {
        let ini = self.licoes.len().saturating_sub(n);
        self.licoes[ini..].iter().map(|l| l.texto.clone()).collect()
    }

    /// Se alguma lembrança tem embedding (vale a pena vetorizar a busca).
    pub fn tem_vetores(&self) -> bool {
        self.vinculos.iter().any(|v| v.vetor.is_some())
            || self.aprendizados.iter().any(|a| a.vetor.is_some())
            || self.licoes.iter().any(|l| l.vetor.is_some())
    }

    /// Guarda um vínculo (pergunta de outro corvo → resposta deste).
    pub fn vincular(&mut self, quem_perguntou: &str, pergunta: &str, resposta: &str) {
        self.vinculos.push(Vinculo {
            quando: agora(),
            quem_perguntou: quem_perguntou.into(),
            pergunta: pergunta.into(),
            resposta: resposta.into(),
            vetor: None,
        });
    }

    /// Guarda o que aprendeu numa consulta e lança no livro-razão.
    pub fn aprendeu(&mut self, a: Aprendizado) {
        self.registrar("consulta", &format!("pesquisou \"{}\" e guardou: {} ({})", a.termo, a.titulo, a.fonte));
        self.aprendizados.push(a);
    }

    /// Tamanho do arquivo em bytes.
    pub fn tamanho(&self) -> usize {
        serde_json::to_vec(self).map(|v| v.len()).unwrap_or(0)
    }

    /// Corta as lembranças mais antigas até caber em `limite_mb` (falas primeiro, depois
    /// aprendizados, depois vínculos, sempre deixando 2). O livro-razão fica inteiro.
    pub fn caber(&mut self, limite_mb: f32) {
        let limite = (f64::from(limite_mb.max(0.01)) * 1024.0 * 1024.0) as usize;
        while self.tamanho() > limite {
            if !self.falas.is_empty() {
                self.falas.remove(0);
            } else if !self.pensamentos.is_empty() {
                self.pensamentos.remove(0);
            } else if !self.aprendizados.is_empty() {
                self.aprendizados.remove(0);
            } else if self.vinculos.len() > 2 {
                self.vinculos.remove(0);
            } else if self.licoes.len() > 5 {
                self.licoes.remove(0);
            } else {
                break;
            }
        }
    }

    /// Grava no disco, respeitando a memória do corvo em MB.
    pub fn salvar(&mut self, pasta: &Path, limite_mb: f32) -> Result<()> {
        self.caber(limite_mb);
        let caminho = arquivo(pasta, &self.nome);
        if let Some(dir) = caminho.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
        }
        std::fs::write(&caminho, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("gravando a memória de {}", self.nome))?;
        Ok(())
    }

    /// O RAG do corvo: até `n` lembranças (aprendizados, vínculos e lições) que mais têm
    /// a ver com `contexto`. Com `vetor` (o embedding do contexto), mede pelo significado
    /// (cosseno); sem ele, pelas palavras em comum.
    pub fn lembrancas(&self, contexto: &str, n: usize, vetor: Option<&[f32]>) -> Vec<String> {
        let chave = palavras(contexto);
        let pontua = |texto: &str, emb: &Option<String>| -> f32 {
            match (vetor, emb) {
                (Some(q), Some(e)) => cosseno(q, &expandir(e)) * 10.0,
                _ => palavras(texto).iter().filter(|w| chave.contains(w)).count() as f32,
            }
        };
        let mut itens: Vec<(f32, u64, String)> = Vec::new();
        for a in &self.aprendizados {
            let texto = format!("Aprendi na internet ({}): {}", a.titulo, a.texto);
            let pontos = pontua(&format!("{} {} {}", a.termo, a.titulo, a.texto), &a.vetor);
            itens.push((pontos + 0.5, a.quando, texto));
        }
        for v in &self.vinculos {
            let texto = format!("{} me perguntou \"{}\" e eu respondi: {}", v.quem_perguntou, v.pergunta, v.resposta);
            itens.push((pontua(&format!("{} {}", v.pergunta, v.resposta), &v.vetor), v.quando, texto));
        }
        for l in &self.licoes {
            itens.push((pontua(&l.texto, &l.vetor), l.quando, format!("Lição minha: {}", l.texto)));
        }
        itens.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(b.1.cmp(&a.1)));
        itens.into_iter().filter(|(p, _, _)| *p > 0.9).take(n).map(|(_, _, t)| t.chars().take(500).collect()).collect()
    }

    /// O corvo deixou de existir: lança a morte e move o arquivo para `mortos/` (a alma
    /// fica guardada para poder ser exportada depois).
    pub fn enterrar(mut self, pasta: &Path, motivo: &str) -> Result<PathBuf> {
        self.registrar("morte", motivo);
        let destino = pasta.join("mortos").join(format!("{}-{}.json", nome_de_arquivo(&self.nome), agora()));
        if let Some(dir) = destino.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&destino, serde_json::to_vec_pretty(&self)?)?;
        let _ = std::fs::remove_file(arquivo(pasta, &self.nome));
        Ok(destino)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta() -> PathBuf {
        let d = std::env::temp_dir().join(format!("dotcrow-mem-{}-{}", std::process::id(), agora()));
        std::fs::create_dir_all(&d).unwrap_or_default();
        d
    }

    #[test]
    fn livro_razao_encadeado_e_detecta_adulteracao() {
        let mut m = Memoria::abrir(&pasta(), "Nyx");
        m.registrar("nota", "recebeu 9 do dono");
        m.registrar("premio", "+3 MB e 6 consultas");
        assert!(m.integra());
        assert_eq!(m.livro_razao.len(), 3);
        assert_eq!(m.livro_razao[1].anterior, m.livro_razao[0].hash);
        m.livro_razao[1].descricao = "recebeu 10 do dono".into();
        assert!(!m.integra(), "mudar um lançamento antigo quebra a corrente");
    }

    #[test]
    fn grava_volta_e_respeita_o_tamanho() {
        let d = pasta();
        let mut m = Memoria::abrir(&d, "Ícaro");
        for i in 0..400 {
            m.falou(&format!("fala número {i} {}", "x".repeat(400)));
        }
        m.vincular("Nyx", "Você voa?", "Voo sim.");
        m.salvar(&d, 0.05).unwrap_or_else(|e| panic!("{e}"));
        let de_volta = Memoria::abrir(&d, "Ícaro");
        assert!(de_volta.tamanho() <= (0.05 * 1024.0 * 1024.0) as usize + 1);
        assert!(de_volta.falas.len() < 400, "as falas mais antigas saem primeiro");
        assert_eq!(de_volta.vinculos.len(), 1, "o vínculo fica");
        assert!(de_volta.integra());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lembrancas_trazem_o_que_tem_a_ver() {
        let mut m = Memoria::abrir(&pasta(), "Bran");
        m.aprendeu(Aprendizado {
            quando: 1,
            termo: "vulcões".into(),
            titulo: "Vulcão".into(),
            fonte: "https://pt.wikipedia.org/wiki/Vulc%C3%A3o".into(),
            texto: "Um vulcão é uma abertura na crosta terrestre.".into(),
            vetor: None,
        });
        m.vincular("Nyx", "Qual o melhor café?", "O passado na hora.");
        let l = m.lembrancas("vamos falar de vulcão e crosta", 3, None);
        assert!(l.first().is_some_and(|t| t.contains("Vulcão")));
        assert!(m.integra());
    }

    #[test]
    fn embedding_compactado_volta_parecido() {
        let v = vec![0.3, -0.9, 0.0, 0.45];
        let hex = compactar(&v);
        assert_eq!(hex.len(), 8);
        let w = expandir(&hex);
        assert!(cosseno(&v, &w) > 0.999);
        assert!((cosseno(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
    }

    #[test]
    fn lembra_pelo_significado_com_vetores() {
        let mut m = Memoria::abrir(&pasta(), "Lenore");
        m.refletiu(&["Ser mais curta.".into(), "Citar fontes.".into()], Some(vec![vec![1.0, 0.0], vec![0.0, 1.0]]));
        assert!(m.tem_vetores());
        let l = m.lembrancas("qualquer coisa", 1, Some(&[0.0, 1.0]));
        assert_eq!(l, vec!["Lição minha: Citar fontes.".to_string()]);
        assert_eq!(m.ultimas_licoes(1), vec!["Citar fontes.".to_string()]);
        assert!(m.livro_razao.iter().any(|x| x.tipo == "reflexao"));
    }

    #[test]
    fn enterrar_move_para_mortos() {
        let d = pasta();
        let mut m = Memoria::abrir(&d, "Grimm");
        m.salvar(&d, 0.1).unwrap_or_else(|e| panic!("{e}"));
        let destino = m.enterrar(&d, "ficou entre os últimos").unwrap_or_else(|e| panic!("{e}"));
        assert!(destino.exists());
        assert!(!arquivo(&d, "Grimm").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
