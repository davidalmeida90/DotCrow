//! Áudio do dono: grava o microfone até ele apertar Enter (ou 2 minutos) e transcreve.
//!
//! - No Termux: `termux-microphone-record` (app Termux:API + `pkg install termux-api`).
//! - No Linux: `arecord` (ALSA) ou `rec` (SoX) ou `ffmpeg`.
//! - No macOS: `rec` (SoX, `brew install sox`) ou `ffmpeg`.
//!
//! A transcrição usa a chave da OpenAI (`gpt-4o-mini-transcribe`, com `whisper-1` de
//! reserva) ou, sem ela, a do Groq (Whisper). O áudio fica salvo em
//! `Downloads/DotCrow/audios`, para o dono ouvir de novo.
//!
//! No modo terminal, `/audio [contexto]` ou uma linha como "Áudio pra post no Twitter"
//! abre o microfone; o que o dono falou vira o pedido para os corvos.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use dotcrow_providers::openai;
use tokio::io::AsyncWriteExt;

use crate::Paint;
use crate::celular::tem;
use crate::store::Store;

/// Tempo máximo de uma gravação.
pub const LIMITE: Duration = Duration::from_secs(120);
/// Tamanho máximo aceito pela API de transcrição.
const MAX_BYTES: u64 = 25 * 1024 * 1024;

/// Como gravar neste aparelho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravador {
    /// Termux:API.
    Termux,
    /// ALSA (`arecord`).
    Arecord,
    /// SoX (`rec`).
    Sox,
    /// `ffmpeg`.
    Ffmpeg,
}

impl Gravador {
    /// O primeiro que existe aqui.
    pub fn achar() -> Option<Self> {
        if tem("termux-microphone-record") {
            Some(Self::Termux)
        } else if cfg!(target_os = "linux") && tem("arecord") {
            Some(Self::Arecord)
        } else if tem("rec") {
            Some(Self::Sox)
        } else if tem("ffmpeg") && !cfg!(target_os = "windows") {
            Some(Self::Ffmpeg)
        } else {
            None
        }
    }

    fn extensao(self) -> &'static str {
        match self {
            Self::Termux => "m4a",
            _ => "wav",
        }
    }
}

const SEM_GRAVADOR: &str = "não achei como gravar o microfone. No Termux: instale o app Termux:API e rode \
`pkg install termux-api`. No Linux: `sudo apt install alsa-utils` (arecord). No macOS: `brew install sox`.";

/// Para a gravação quando `parar` terminar (true = usar o áudio, false = cancelar) ou no
/// limite de tempo. Devolve o arquivo, ou `None` se o dono cancelou.
pub async fn gravar(pasta: &Path, parar: impl Future<Output = bool>, limite: Duration) -> Result<Option<PathBuf>> {
    let Some(g) = Gravador::achar() else { bail!(SEM_GRAVADOR) };
    std::fs::create_dir_all(pasta).with_context(|| format!("não consegui criar {}", pasta.display()))?;
    let segundos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let arquivo = pasta.join(format!("audio-{segundos}.{}", g.extensao()));
    let a = arquivo.display().to_string();
    let mut filho = None;
    match g {
        Gravador::Termux => {
            let out = tokio::process::Command::new("termux-microphone-record")
                .args(["-f", &a, "-l", &limite.as_secs().to_string()])
                .output()
                .await
                .context("termux-microphone-record")?;
            let txt = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if !out.status.success() || txt.contains("error") || txt.contains("permission") {
                bail!(
                    "o Termux:API não conseguiu abrir o microfone ({}). Dê a permissão de microfone ao app Termux:API.",
                    String::from_utf8_lossy(&out.stdout).trim()
                );
            }
        }
        Gravador::Arecord | Gravador::Sox | Gravador::Ffmpeg => {
            let mut c = match g {
                Gravador::Arecord => {
                    let mut c = tokio::process::Command::new("arecord");
                    c.args(["-q", "-f", "S16_LE", "-r", "16000", "-c", "1", "-t", "wav", &a]);
                    c
                }
                Gravador::Sox => {
                    let mut c = tokio::process::Command::new("rec");
                    c.args(["-q", "-c", "1", "-r", "16000", &a]);
                    c
                }
                _ => {
                    let mut c = tokio::process::Command::new("ffmpeg");
                    let entrada: &[&str] = if cfg!(target_os = "macos") {
                        &["-f", "avfoundation", "-i", ":0"]
                    } else {
                        &["-f", "pulse", "-i", "default"]
                    };
                    c.args(["-hide_banner", "-loglevel", "error", "-y"])
                        .args(entrada)
                        .args(["-ac", "1", "-ar", "16000", &a]);
                    c
                }
            };
            c.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
            filho = Some(c.spawn().with_context(|| format!("não consegui abrir o gravador ({g:?})"))?);
        }
    }

    let usar = tokio::select! {
        u = parar => u,
        _ = tokio::time::sleep(limite) => true,
    };

    // Para o gravador do jeito que ele fecha o arquivo direito.
    match g {
        Gravador::Termux => {
            let _ = tokio::process::Command::new("termux-microphone-record").arg("-q").output().await;
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
        Gravador::Ffmpeg => {
            if let Some(f) = filho.as_mut() {
                if let Some(mut entrada) = f.stdin.take() {
                    let _ = entrada.write_all(b"q").await;
                }
                let _ = tokio::time::timeout(Duration::from_secs(5), f.wait()).await;
            }
        }
        Gravador::Arecord | Gravador::Sox => {
            if let Some(f) = filho.as_mut() {
                if let Some(pid) = f.id() {
                    let _ = tokio::process::Command::new("kill").args(["-INT", &pid.to_string()]).output().await;
                }
                if tokio::time::timeout(Duration::from_secs(5), f.wait()).await.is_err() {
                    let _ = f.kill().await;
                }
            }
        }
    }
    if !usar {
        let _ = std::fs::remove_file(&arquivo);
        return Ok(None);
    }
    let tamanho = std::fs::metadata(&arquivo).map(|m| m.len()).unwrap_or(0);
    if tamanho < 1000 {
        bail!("a gravação ficou vazia ({tamanho} bytes): o microfone está liberado para o {g:?}?");
    }
    Ok(Some(arquivo))
}

/// Transcreve o arquivo com a OpenAI ou, sem ela, com o Groq.
pub async fn transcrever(store: &Store, arquivo: &Path) -> Result<String> {
    let tamanho = std::fs::metadata(arquivo).map(|m| m.len()).unwrap_or(0);
    if tamanho > MAX_BYTES {
        bail!("o áudio passou de 25 MB; grave um pedaço menor");
    }
    let dados = std::fs::read(arquivo).with_context(|| format!("não consegui ler {}", arquivo.display()))?;
    let nome = arquivo.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "audio.wav".into());
    let chaves: Vec<String> = store.list_keys().into_iter().map(|k| k.provider).collect();
    let keys = store.keys();
    let mut erros = Vec::new();
    for (flavor, modelos) in [(openai::OPENAI, openai::TRANSCRICAO_OPENAI), (openai::GROQ, openai::TRANSCRICAO_GROQ)] {
        if !chaves.iter().any(|c| c == flavor.id) {
            continue;
        }
        match openai::transcrever(keys.as_ref(), flavor, modelos, &dados, &nome, Some("pt")).await {
            Ok(t) if !t.trim().is_empty() => return Ok(t),
            Ok(_) => erros.push(format!("{}: não entendi nada no áudio", flavor.id)),
            Err(e) => erros.push(format!("{}: {e}", flavor.id)),
        }
    }
    if erros.is_empty() {
        bail!(
            "para transcrever o áudio preciso da chave da OpenAI ou do Groq: dotcrow chaves adicionar openai (ou groq)"
        );
    }
    bail!("não consegui transcrever: {}", erros.join(" · "))
}

/// A pasta dos áudios: `Downloads/DotCrow/audios`.
pub fn pasta() -> PathBuf {
    crate::maos::pasta_de_downloads().join("DotCrow").join("audios")
}

/// Grava e transcreve, mostrando o andamento. `parar` decide quando parar (true = usar).
/// Devolve `None` se o dono cancelou.
pub async fn ouvir_o_dono(store: &Store, parar: impl Future<Output = bool>, p: &Paint) -> Result<Option<String>> {
    println!(
        "  {} {}",
        p.red("● Gravando…"),
        p.dim(&format!("fale agora. Enter termina · Ctrl+C cancela · no máximo {} min", LIMITE.as_secs() / 60))
    );
    let Some(arquivo) = gravar(&pasta(), parar, LIMITE).await? else {
        println!("  {}", p.dim("Gravação cancelada."));
        return Ok(None);
    };
    println!("  {}", p.dim("⏳ Transcrevendo…"));
    let texto = transcrever(store, &arquivo).await?;
    println!("  {} {}", p.bold("🗣 Você disse:"), texto);
    println!("  {}", p.dim(&format!("(áudio salvo em {})", arquivo.display())));
    Ok(Some(texto))
}

/// Se a linha pede para abrir o microfone: "/audio ...", "áudio", "Áudio pra post no
/// Twitter", "audio para o grupo". Devolve o contexto (o que vem depois de "áudio").
pub fn gatilho(linha: &str) -> Option<String> {
    let l = linha.trim();
    for prefixo in ["/audio", "/áudio", "/microfone", "/mic"] {
        if let Some(resto) = l.strip_prefix(prefixo)
            && (resto.is_empty() || resto.starts_with(char::is_whitespace))
        {
            return Some(resto.trim().to_string());
        }
    }
    if l.split_whitespace().count() > 12 {
        return None;
    }
    let mut palavras = l.split_whitespace();
    let primeira = palavras.next()?.to_lowercase();
    let primeira = primeira.trim_matches(|c: char| !c.is_alphanumeric());
    if !matches!(primeira, "áudio" | "audio" | "🎤") && !l.starts_with('🎤') {
        return None;
    }
    let resto: Vec<&str> = palavras.collect();
    match resto.first().map(|w| w.to_lowercase()) {
        None => Some(String::new()),
        Some(w) if matches!(w.as_str(), "pra" | "para" | "pro" | "p/" | "pras" | "pros" | "no" | "na" | "ao") => {
            Some(resto.join(" "))
        }
        _ => None,
    }
}

/// O pedido que vai para os corvos depois de um áudio.
pub fn pedido_do_audio(contexto: &str, fala: &str) -> String {
    let contexto = contexto.trim();
    let para = if contexto.is_empty() { String::new() } else { format!(" ({contexto})") };
    format!(
        "O dono mandou um áudio{para}. Transcrição do que ele falou:\n\"{fala}\"\n\n\
Faça o que ele pediu, até o fim. Se for um texto para publicar (post no X/Twitter, Instagram, LinkedIn, \
WhatsApp...), finalize o texto (corrija, deixe no tamanho e no tom da rede; no X, até 280 caracteres) e \
publique pelo caminho que existir: o celular (celular {{acao: compartilhar, texto, app}} e depois tocar no \
botão de publicar), o navegador, uma API ou o terminal. Seja o rei da gambiarra: se um caminho falhar, tente \
outro, pesquise como fazer e não desista na primeira. Publicar, enviar, ligar e mandar SMS pedem a \
confirmação do dono na hora: peça e siga. No fim, diga o que publicou e onde."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gatilho_abre_o_microfone_so_quando_pedem() {
        assert_eq!(gatilho("Áudio pra post no Twitter").as_deref(), Some("pra post no Twitter"));
        assert_eq!(gatilho("audio").as_deref(), Some(""));
        assert_eq!(gatilho("/audio post no X").as_deref(), Some("post no X"));
        assert_eq!(gatilho("/audio").as_deref(), Some(""));
        assert_eq!(gatilho("🎤").as_deref(), Some(""));
        assert_eq!(gatilho("áudio para o grupo").as_deref(), Some("para o grupo"));
        assert!(gatilho("audio do projeto está quebrado, conserta").is_none());
        assert!(gatilho("/audiobook").is_none());
        assert!(gatilho("faz um post").is_none());
    }

    #[test]
    fn pedido_leva_o_contexto_e_a_fala() {
        let p = pedido_do_audio("pra post no Twitter", "fala sobre o lançamento do DotCrow");
        assert!(p.contains("(pra post no Twitter)"));
        assert!(p.contains("\"fala sobre o lançamento do DotCrow\""));
        assert!(p.contains("rei da gambiarra"));
    }
}
