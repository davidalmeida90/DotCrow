//! `dotcrow`: a linha de comando do DotCrow.
//!
//! Nesta fase ela cuida do cofre de chaves e mostra os agentes. O daemon que
//! executa tarefas no computador está no ROADMAP (fase 1).

use std::{io::IsTerminal, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use dotcrow_agents::{AGENTS, TaskKind, default_flow, profile};
use dotcrow_crypto::sym;
use dotcrow_providers::{CompletionRequest, LlmProvider, Message, ProviderError, Role, Router, mock::MockProvider};
use dotcrow_vault::{Vault, write_private};
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(name = "dotcrow", version, about = "DotCrow: agentes open source que trabalham no seu computador")]
struct Cli {
    /// Pasta de dados (padrão: pasta de dados do usuário, ou a variável DOTCROW_HOME).
    #[arg(long, global = true, env = "DOTCROW_HOME")]
    home: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Gerencia as chaves de API (só dá para adicionar, listar e apagar).
    #[command(visible_alias = "keys")]
    Chaves {
        #[command(subcommand)]
        acao: ChavesAcao,
    },
    /// Mostra os agentes e como cada um pensa.
    #[command(visible_alias = "agents")]
    Agentes,
    /// Mostra o fluxo de uma tarefa entre os agentes.
    #[command(visible_alias = "flow")]
    Fluxo {
        /// Tipo de tarefa: organizar, operar ou escrever.
        #[arg(default_value = "operar")]
        tipo: String,
    },
    /// Demonstrações para quem está conhecendo o projeto.
    Demo {
        #[command(subcommand)]
        qual: DemoQual,
    },
}

#[derive(Subcommand)]
enum ChavesAcao {
    /// Adiciona uma chave. Ela é lida sem aparecer na tela e nunca mais é mostrada.
    #[command(visible_alias = "add")]
    Adicionar {
        /// Provedor: anthropic, openai, gemini, openrouter, ollama...
        provedor: String,
        /// Rótulo opcional, como "conta pessoal".
        #[arg(long)]
        rotulo: Option<String>,
    },
    /// Lista as chaves (provedor, rótulo, data e impressão digital). Nunca mostra a chave.
    #[command(visible_alias = "list")]
    Listar,
    /// Apaga uma chave pelo id mostrado em `listar`.
    #[command(visible_alias = "rm")]
    Apagar {
        /// Id da chave.
        id: String,
    },
}

#[derive(Subcommand)]
enum DemoQual {
    /// Simula a troca automática quando a API de um provedor acaba.
    #[command(visible_alias = "failover")]
    Troca,
}

fn data_dir(cli: &Cli) -> Result<PathBuf> {
    if let Some(h) = &cli.home {
        return Ok(h.clone());
    }
    directories::ProjectDirs::from("si", "dotcrow", "dotcrow")
        .map(|d| d.data_dir().to_path_buf())
        .context("não encontrei a pasta de dados do usuário; use --home")
}

/// Chave mestra do cofre.
///
/// TEMPORÁRIO: fica num arquivo 0600 ao lado do cofre. A issue "Guardar a chave mestra
/// no chaveiro do sistema" troca isso pelo Keychain (macOS), Credential Manager
/// (Windows) e Secret Service (Linux).
fn master_key(dir: &std::path::Path) -> Result<sym::SymKey> {
    let path = dir.join("master.key");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let arr: [u8; 32] = bytes.as_slice().try_into().context("master.key corrompida")?;
            Ok(Zeroizing::new(arr))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let k = sym::generate_key()?;
            write_private(&path, k.as_ref())?;
            Ok(k)
        }
        Err(e) => Err(e.into()),
    }
}

fn open_vault(cli: &Cli) -> Result<Vault> {
    let dir = data_dir(cli)?;
    let key = master_key(&dir)?;
    Ok(Vault::open_file(dir.join("vault.bin"), key)?)
}

fn read_secret() -> Result<Zeroizing<String>> {
    if std::io::stdin().is_terminal() {
        let s = rpassword::prompt_password("Cole a chave (ela não vai aparecer): ")?;
        Ok(Zeroizing::new(s))
    } else {
        let mut s = String::new();
        std::io::stdin().read_line(&mut s)?;
        Ok(Zeroizing::new(s))
    }
}

fn when(ts: u64) -> String {
    // Data simples em UTC sem dependências extras: AAAA-MM-DD.
    let days = (ts / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

// Algoritmo de Howard Hinnant (domínio público) para converter dias em data civil.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

async fn demo_troca() -> Result<()> {
    println!("Simulação: Claude (anthropic) está sem créditos, GPT (openai) está no limite, Gemini responde.\n");
    let providers: Vec<Arc<dyn LlmProvider>> = vec![
        MockProvider::failing("anthropic", ProviderError::QuotaExhausted),
        MockProvider::failing("openai", ProviderError::RateLimited { retry_after: None }),
        MockProvider::ok("gemini"),
    ];
    let router = Router::new(providers);
    let req = CompletionRequest {
        messages: vec![Message { role: Role::User, content: "organize minha pasta Downloads".into() }],
        max_tokens: 256,
    };
    for rodada in 1..=2 {
        let out = router.complete(&req).await?;
        println!("Pedido {rodada}:");
        for a in &out.attempts {
            match &a.error {
                Some(e) => println!("  ✗ {:<10} {e} → trocando", a.provider),
                None => println!("  ↷ {:<10} de castigo, pulado", a.provider),
            }
        }
        println!("  ✓ {:<10} {}\n", out.provider, out.response.text);
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Chaves { acao } => match acao {
            ChavesAcao::Adicionar { provedor, rotulo } => {
                let mut vault = open_vault(&cli)?;
                let secret = read_secret()?;
                let info = vault.add(provedor, secret, rotulo.clone())?;
                println!("Chave guardada. id {}  impressão digital {}", info.id, info.fingerprint);
                println!("Ela não pode ser exibida. Para trocar, apague e adicione outra.");
            }
            ChavesAcao::Listar => {
                let vault = open_vault(&cli)?;
                let keys = vault.list();
                if keys.is_empty() {
                    println!("Nenhuma chave ainda. Use: dotcrow chaves adicionar anthropic");
                }
                for k in keys {
                    println!(
                        "{}  {:<11} {:<20} {}  adicionada em {}",
                        k.id,
                        k.provider,
                        k.label.as_deref().unwrap_or("-"),
                        k.fingerprint,
                        when(k.created_at)
                    );
                }
            }
            ChavesAcao::Apagar { id } => {
                let mut vault = open_vault(&cli)?;
                let info = vault.delete(id)?;
                println!("Chave {} ({}) apagada.", info.id, info.provider);
            }
        },
        Command::Agentes => {
            for a in &AGENTS {
                println!("{} · {}", a.name, a.role);
                println!("  pensa:    {}", a.thinks);
                println!("  organiza: {}\n", a.organizes);
            }
        }
        Command::Fluxo { tipo } => {
            let kind = match tipo.as_str() {
                "organizar" | "organize" => TaskKind::Organize,
                "operar" | "operate" => TaskKind::Operate,
                "escrever" | "write" => TaskKind::Write,
                outro => bail!("tipo desconhecido: {outro} (use organizar, operar ou escrever)"),
            };
            for (i, s) in default_flow(kind).iter().enumerate() {
                println!("{}. {:<7} {}", i + 1, profile(s.agent).name, s.what);
            }
        }
        Command::Demo { qual: DemoQual::Troca } => demo_troca().await?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(when(0), "1970-01-01");
        assert_eq!(when(1_792_368_000), "2026-10-19");
        assert_eq!(when(951_782_400), "2000-02-29");
    }
}
