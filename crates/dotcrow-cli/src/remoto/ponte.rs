//! A ponte remota: conecta no relay como `host` e faz o túnel da API do painel.
//!
//! Para cada requisição que o celular manda (já decifrada), a ponte faz a mesma
//! chamada no painel local (`http://127.0.0.1:porta`) com o código de acesso e
//! devolve a resposta cifrada — inclusive o fluxo de eventos ao vivo (SSE), pedaço a
//! pedaço. Assim o celular reaproveita todo o painel e vê os corvos em tempo real,
//! sem nada novo do lado do servidor.
//!
//! Se a conexão com o relay cair, a ponte reconecta sozinha, com espera crescente.

use std::{sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use super::{Host, Sessao, split, tag};
use dotcrow_crypto::envelope::SecretIdentity;

/// O tipo de conexão que o `connect_async` devolve (com ou sem TLS).
type WsStream = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Tudo o que a ponte precisa para rodar.
pub struct Config {
    /// Base WebSocket do relay, ex.: `wss://dotcrow.si/t`.
    pub relay: String,
    /// Sala deste computador (derivada da identidade).
    pub sala: String,
    /// Porta do painel local.
    pub porta: u16,
    /// Código de acesso do painel.
    pub codigo: String,
}

/// Fica conectando no relay e servindo, reconectando sozinha quando cai.
pub async fn servir(identidade: Arc<SecretIdentity>, cfg: Config) {
    let url = format!("{}?sala={}&rol=host", cfg.relay, cfg.sala);
    let mut espera = Duration::from_secs(1);
    loop {
        match tokio_tungstenite::connect_async(&url).await {
            Ok((socket, _)) => {
                espera = Duration::from_secs(1); // reconectou: zera a espera
                servir_conexao(socket, &identidade, &cfg).await;
            }
            Err(e) => {
                eprintln!("  (remoto) não consegui falar com o relay {}: {e}", cfg.relay);
            }
        }
        tokio::time::sleep(espera).await;
        espera = (espera * 2).min(Duration::from_secs(30));
    }
}

/// Serve uma conexão até ela cair.
async fn servir_conexao(socket: WsStream, identidade: &SecretIdentity, cfg: &Config) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Uma tarefa só escreve no socket, para vários pedidos poderem responder juntos.
    let escritor = tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if sink.send(Message::Binary(frame)).await.is_err() {
                break;
            }
        }
    });

    let host = Host::new(identidade, &cfg.codigo);
    let mut sessao: Option<Arc<Sessao>> = None;

    while let Some(Ok(msg)) = stream.next().await {
        let dados = match msg {
            Message::Binary(b) => b,
            Message::Text(t) => t.into_bytes(),
            Message::Close(_) => break,
            _ => continue,
        };
        let Some((t, payload)) = split(&dados) else { continue };
        match t {
            tag::HI => {
                let _ = out_tx.send(host.hello());
            }
            tag::KEY => match host.aceitar_key(payload) {
                Ok(s) => {
                    let s = Arc::new(s);
                    // Confirmação cifrada: o celular só confia quando abre com a chave certa.
                    if let Ok(ok) = s.cifrar(b"{\"ok\":true}") {
                        // Reusa o frame DATA como OK: o primeiro frame cifrado é a confirmação.
                        let _ = out_tx.send(reetiquetar(ok, tag::OK));
                    }
                    sessao = Some(s);
                }
                Err(e) => {
                    let _ = out_tx.send(super::frame(tag::ERRO, e.as_bytes()));
                }
            },
            tag::DATA => {
                if let Some(s) = &sessao {
                    let s = s.clone();
                    let porta = cfg.porta;
                    let codigo = cfg.codigo.clone();
                    let out = out_tx.clone();
                    let payload = payload.to_vec();
                    tokio::spawn(async move { atender(s, porta, &codigo, &payload, out).await });
                }
            }
            _ => {}
        }
    }
    drop(out_tx);
    let _ = escritor.await;
}

/// Troca o byte de tag de um frame já pronto (o `cifrar` sempre marca DATA).
fn reetiquetar(mut frame: Vec<u8>, novo: u8) -> Vec<u8> {
    if let Some(primeiro) = frame.first_mut() {
        *primeiro = novo;
    }
    frame
}

/// Atende uma requisição do celular: faz a chamada no painel local e devolve a resposta
/// (ou o fluxo de eventos) cifrada.
async fn atender(sessao: Arc<Sessao>, porta: u16, codigo: &str, payload: &[u8], out: mpsc::UnboundedSender<Vec<u8>>) {
    let enviar = |v: serde_json::Value| {
        if let Ok(c) = sessao.cifrar(v.to_string().as_bytes()) {
            let _ = out.send(c);
        }
    };
    let pedido = match sessao.decifrar(payload).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
    {
        Some(v) => v,
        None => return,
    };
    let id = pedido.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let caminho = pedido.get("caminho").and_then(|v| v.as_str()).unwrap_or("");
    // Só falamos com o painel local; nada de caminho estranho.
    if !caminho.starts_with('/') || caminho.contains("://") || caminho.contains("..") {
        enviar(serde_json::json!({ "id": id, "erro": "caminho inválido" }));
        return;
    }
    let metodo = pedido.get("metodo").and_then(|v| v.as_str()).unwrap_or("GET").to_uppercase();
    let sse = pedido.get("sse").and_then(|v| v.as_bool()).unwrap_or(false) || caminho.ends_with("/eventos");
    let url = format!("http://127.0.0.1:{porta}{caminho}");

    let client = match reqwest::Client::builder().no_proxy().build() {
        Ok(c) => c,
        Err(e) => return enviar(serde_json::json!({ "id": id, "erro": e.to_string() })),
    };
    let metodo_http = reqwest::Method::from_bytes(metodo.as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut req = client.request(metodo_http, &url).bearer_auth(codigo);
    if let Some(corpo) = pedido.get("corpo").filter(|c| !c.is_null()) {
        req = req.json(corpo);
    }
    if !sse {
        req = req.timeout(Duration::from_secs(60));
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => return enviar(serde_json::json!({ "id": id, "erro": e.to_string() })),
    };
    let status = resp.status().as_u16();

    if sse {
        let mut resp = resp;
        loop {
            match resp.chunk().await {
                Ok(Some(bytes)) if !bytes.is_empty() => {
                    enviar(serde_json::json!({ "id": id, "sse": String::from_utf8_lossy(&bytes) }));
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => break,
            }
        }
        enviar(serde_json::json!({ "id": id, "fim": true, "status": status }));
    } else {
        let corpo = resp.text().await.unwrap_or_default();
        enviar(serde_json::json!({ "id": id, "status": status, "corpo": corpo }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remoto::{Guest, frame, sala_de, split, tag};
    use dotcrow_crypto::envelope::SecretIdentity;
    use tokio_tungstenite::tungstenite::Message as TMsg;

    type Fone = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

    async fn porta_livre() -> (tokio::net::TcpListener, u16) {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let p = l.local_addr().unwrap().port();
        (l, p)
    }

    /// Um "painel" mínimo: responde /api/estado e um fluxo SSE em /api/x/eventos.
    fn painel_falso() -> axum::Router {
        use axum::{
            response::sse::{Event, Sse},
            routing::get,
        };
        axum::Router::new()
            .route("/api/estado", get(|| async { axum::Json(serde_json::json!({ "ola": "mundo" })) }))
            .route(
                "/api/x/eventos",
                get(|| async {
                    let s = futures_util::stream::iter(vec![
                        Ok::<_, std::convert::Infallible>(Event::default().data("um")),
                        Ok(Event::default().data("dois")),
                    ]);
                    Sse::new(s)
                }),
            )
    }

    async fn recebe(fone: &mut Fone) -> Vec<u8> {
        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(5), fone.next()).await {
                Ok(Some(Ok(TMsg::Binary(b)))) => return b,
                Ok(Some(Ok(_))) => continue,
                _ => panic!("não chegou frame do túnel"),
            }
        }
    }

    #[tokio::test]
    async fn tunel_completo_handshake_pedido_e_sse() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let codigo = "abcd-efgh-jkmn-pqrs";

        // Relay.
        let (relay_l, relay_port) = porta_livre().await;
        tokio::spawn(async move { axum::serve(relay_l, dotcrow_relay::app(dotcrow_relay::Relay::new())).await });

        // Painel local falso.
        let (painel_l, painel_port) = porta_livre().await;
        tokio::spawn(async move { axum::serve(painel_l, painel_falso()).await });

        // A ponte (lado computador), com uma identidade nova.
        let identidade = Arc::new(SecretIdentity::generate().unwrap());
        let sala = sala_de(&identidade.public());
        let cfg = Config {
            relay: format!("ws://127.0.0.1:{relay_port}/t"),
            sala: sala.clone(),
            porta: painel_port,
            codigo: codigo.to_string(),
        };
        let id2 = identidade.clone();
        tokio::spawn(async move { servir(id2, cfg).await });
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // O celular entra na sala.
        let url = format!("ws://127.0.0.1:{relay_port}/t?sala={sala}&rol=guest");
        let (mut fone, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // HI → HELLO → KEY → OK
        fone.send(TMsg::Binary(frame(tag::HI, b""))).await.unwrap();
        let hello = recebe(&mut fone).await;
        let (t, payload) = split(&hello).unwrap();
        assert_eq!(t, tag::HELLO);
        let guest = Guest::new(&sala, codigo);
        let (key_frame, sessao) = guest.responder_hello(payload).unwrap();
        fone.send(TMsg::Binary(key_frame)).await.unwrap();
        let ok = recebe(&mut fone).await;
        let (t, p) = split(&ok).unwrap();
        assert_eq!(t, tag::OK);
        assert_eq!(sessao.decifrar(p).unwrap(), b"{\"ok\":true}");

        // Um pedido normal: GET /api/estado.
        let req = serde_json::json!({ "id": 1, "metodo": "GET", "caminho": "/api/estado" });
        fone.send(TMsg::Binary(sessao.cifrar(req.to_string().as_bytes()).unwrap())).await.unwrap();
        let resp = recebe(&mut fone).await;
        let (_t, p) = split(&resp).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&sessao.decifrar(p).unwrap()).unwrap();
        assert_eq!(v["id"], 1);
        assert_eq!(v["status"], 200);
        assert!(v["corpo"].as_str().unwrap().contains("mundo"), "corpo: {}", v["corpo"]);

        // O fluxo ao vivo (SSE): deve chegar em pedaços e terminar com fim.
        let req = serde_json::json!({ "id": 2, "metodo": "GET", "caminho": "/api/x/eventos", "sse": true });
        fone.send(TMsg::Binary(sessao.cifrar(req.to_string().as_bytes()).unwrap())).await.unwrap();
        let mut texto_sse = String::new();
        let mut terminou = false;
        for _ in 0..10 {
            let f = recebe(&mut fone).await;
            let (_t, p) = split(&f).unwrap();
            let v: serde_json::Value = serde_json::from_slice(&sessao.decifrar(p).unwrap()).unwrap();
            if let Some(chunk) = v["sse"].as_str() {
                texto_sse.push_str(chunk);
            }
            if v["fim"].as_bool() == Some(true) {
                terminou = true;
                break;
            }
        }
        assert!(terminou, "o fluxo SSE precisa terminar com fim");
        assert!(texto_sse.contains("um") && texto_sse.contains("dois"), "veio pelo túnel: {texto_sse:?}");
    }
}
