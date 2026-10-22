//! Relay do DotCrow: o ponto de encontro na internet entre o computador e o celular.
//!
//! Os dois lados **discam para fora** (conexão de saída), então funciona mesmo atrás
//! de NAT, CGNAT ou dados móveis, sem abrir porta no roteador. Cada instalação tem
//! uma **sala** (o id é a impressão digital da identidade daquela instalação). O
//! computador entra como `host`; o celular, como `guest`. O relay só **repassa os
//! bytes** de um lado para o outro.
//!
//! O relay é **zero-conhecimento**: todo o conteúdo já chega cifrado de ponta a ponta
//! (envelope pós-quântico + chave de sessão). O relay nunca vê o código de acesso, o
//! pedido nem o que os corvos fazem — só ciphertext e o id da sala. Por isso ele não
//! guarda nada em disco e não tem chave nenhuma.
//!
//! Fica atrás de um proxy TLS (nginx/caddy) que serve `wss://dotcrow.si/t`; o relay
//! em si escuta `ws://` no localhost da VPS. Veja `docs/remoto.md`.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

/// Frame maior que isto é recusado (a conexão cai). Protege a memória do relay.
pub const MAX_FRAME: usize = 8 * 1024 * 1024;
/// Sem nenhuma mensagem por este tempo, a conexão é encerrada.
const IDLE: Duration = Duration::from_secs(120);
/// Tamanho máximo do id da sala (impressão digital em hex com hífens).
const MAX_SALA: usize = 80;

/// Papel de quem conectou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Papel {
    /// O computador (dono da sala).
    Host,
    /// O celular.
    Guest,
}

impl Papel {
    fn ler(s: &str) -> Option<Self> {
        match s {
            "host" => Some(Self::Host),
            "guest" => Some(Self::Guest),
            _ => None,
        }
    }

    fn outro(self) -> Self {
        match self {
            Self::Host => Self::Guest,
            Self::Guest => Self::Host,
        }
    }
}

/// Um lado conectado numa sala: por aqui entregamos as mensagens destinadas a ele.
struct Lado {
    tx: mpsc::UnboundedSender<Vec<u8>>,
}

/// Uma sala com até dois lados (host e guest).
#[derive(Default)]
struct Sala {
    host: Option<Lado>,
    guest: Option<Lado>,
}

impl Sala {
    fn slot(&mut self, papel: Papel) -> &mut Option<Lado> {
        match papel {
            Papel::Host => &mut self.host,
            Papel::Guest => &mut self.guest,
        }
    }

    fn vazia(&self) -> bool {
        self.host.is_none() && self.guest.is_none()
    }
}

/// Estado compartilhado: as salas abertas agora.
#[derive(Clone, Default)]
pub struct Relay {
    salas: Arc<Mutex<HashMap<String, Sala>>>,
}

impl Relay {
    /// Cria um relay vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Quantas salas estão abertas (para métricas e testes).
    pub fn salas_abertas(&self) -> usize {
        self.salas.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Registra um lado na sala e devolve o canal por onde ele recebe as mensagens do
    /// outro. Substitui um lado antigo com o mesmo papel (reconexão), fechando o canal dele.
    fn entrar(&self, sala: &str, papel: Papel) -> mpsc::UnboundedReceiver<Vec<u8>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut salas = self.salas.lock().unwrap_or_else(|e| e.into_inner());
        let s = salas.entry(sala.to_string()).or_default();
        *s.slot(papel) = Some(Lado { tx }); // o canal antigo cai sozinho ao ser descartado
        rx
    }

    /// Entrega uma mensagem ao outro lado da sala, se ele estiver conectado.
    fn repassar(&self, sala: &str, de: Papel, dados: Vec<u8>) {
        let salas = self.salas.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = salas.get(sala)
            && let Some(outro) = match de.outro() {
                Papel::Host => &s.host,
                Papel::Guest => &s.guest,
            }
        {
            let _ = outro.tx.send(dados);
        }
    }

    /// Tira um lado da sala ao desconectar; some com a sala se ela ficar vazia.
    fn sair(&self, sala: &str, papel: Papel) {
        let mut salas = self.salas.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = salas.get_mut(sala) {
            *s.slot(papel) = None;
            if s.vazia() {
                salas.remove(sala);
            }
        }
    }
}

/// Monta as rotas do relay.
pub fn app(relay: Relay) -> Router {
    Router::new().route("/", get(saude)).route("/t", get(upgrade)).with_state(relay)
}

async fn saude() -> &'static str {
    "DotCrow relay: só repasso bytes cifrados. Nada é lido nem guardado aqui."
}

async fn upgrade(
    State(relay): State<Relay>,
    ws: WebSocketUpgrade,
    axum::extract::RawQuery(q): axum::extract::RawQuery,
) -> axum::response::Response {
    let (mut sala, mut rol) = (String::new(), String::new());
    for par in q.as_deref().unwrap_or("").split('&') {
        match par.split_once('=') {
            Some(("sala", v)) => sala = v.to_string(),
            Some(("rol", v)) => rol = v.to_string(),
            _ => {}
        }
    }
    let Some(papel) = Papel::ler(&rol) else {
        return (axum::http::StatusCode::BAD_REQUEST, "rol deve ser host ou guest").into_response();
    };
    if sala.is_empty() || sala.len() > MAX_SALA || !sala.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return (axum::http::StatusCode::BAD_REQUEST, "sala inválida").into_response();
    }
    ws.max_message_size(MAX_FRAME).on_upgrade(move |socket| conduzir(relay, socket, sala, papel))
}

/// Conduz uma conexão: entrega ao outro lado o que chega, e escreve no socket o que o
/// outro lado manda. Encerra em erro, inatividade ou desconexão.
async fn conduzir(relay: Relay, socket: WebSocket, sala: String, papel: Papel) {
    let mut rx = relay.entrar(&sala, papel);
    let (mut sink, mut stream) = socket.split();

    loop {
        tokio::select! {
            // Mensagem vinda do outro lado → escreve no nosso socket.
            entrada = rx.recv() => match entrada {
                Some(dados) => {
                    if sink.send(Message::Binary(dados.into())).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
            // Mensagem que chegou do nosso socket → repassa ao outro lado.
            recebida = tokio::time::timeout(IDLE, stream.next()) => match recebida {
                Ok(Some(Ok(Message::Binary(b)))) => relay.repassar(&sala, papel, b.to_vec()),
                Ok(Some(Ok(Message::Text(t)))) => relay.repassar(&sala, papel, t.as_bytes().to_vec()),
                Ok(Some(Ok(Message::Ping(_)))) | Ok(Some(Ok(Message::Pong(_)))) => {}
                Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) | Ok(None) | Err(_) => break,
            },
        }
    }
    relay.sair(&sala, papel);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn papel_e_slots() {
        assert_eq!(Papel::ler("host"), Some(Papel::Host));
        assert_eq!(Papel::ler("guest"), Some(Papel::Guest));
        assert_eq!(Papel::ler("outro"), None);
        assert_eq!(Papel::Host.outro(), Papel::Guest);

        let relay = Relay::new();
        let _rx = relay.entrar("sala1", Papel::Host);
        assert_eq!(relay.salas_abertas(), 1);
        relay.sair("sala1", Papel::Host);
        assert_eq!(relay.salas_abertas(), 0, "sala vazia some");
    }

    #[test]
    fn repassa_para_o_outro_lado() {
        let relay = Relay::new();
        let _host = relay.entrar("s", Papel::Host);
        let mut guest = relay.entrar("s", Papel::Guest);
        relay.repassar("s", Papel::Host, b"oi".to_vec());
        assert_eq!(guest.try_recv().unwrap(), b"oi");
        // Sem o outro lado conectado, repassar não quebra.
        relay.sair("s", Papel::Guest);
        relay.repassar("s", Papel::Host, b"ninguem".to_vec());
    }
}
