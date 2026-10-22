//! Binário do relay do DotCrow.
//!
//! Escuta WebSocket (`ws://`) e pareia computador e celular por sala, repassando só
//! bytes cifrados. Fica atrás de um proxy TLS (nginx/caddy) que serve `wss://.../t`.
//!
//! Variáveis de ambiente:
//! - `DOTCROW_RELAY_BIND`: endereço de escuta (padrão `127.0.0.1:8787`).

use anyhow::Result;
use dotcrow_relay::{Relay, app};

#[tokio::main]
async fn main() -> Result<()> {
    let bind = std::env::var("DOTCROW_RELAY_BIND").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("DotCrow relay escutando em {bind} (ws). Repassa só bytes cifrados; não lê nem guarda nada.");
    axum::serve(listener, app(Relay::new())).await?;
    Ok(())
}
