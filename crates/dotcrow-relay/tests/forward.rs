//! O relay pareia host e guest pela sala e repassa os dois sentidos, sem tocar nos bytes.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dotcrow_relay::{Relay, app};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

async fn sobe() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app(Relay::new())).await });
    // Dá um instante para o servidor começar a aceitar conexões.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    port
}

async fn conecta(
    port: u16,
    sala: &str,
    rol: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("ws://127.0.0.1:{port}/t?sala={sala}&rol={rol}");
    tokio_tungstenite::connect_async(url).await.unwrap().0
}

#[tokio::test]
async fn repassa_nos_dois_sentidos() {
    let port = sobe().await;
    let mut host = conecta(port, "abcd-1234", "host").await;
    let mut guest = conecta(port, "abcd-1234", "guest").await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // guest → host
    guest.send(Message::Binary(b"ola host".to_vec())).await.unwrap();
    match host.next().await.unwrap().unwrap() {
        Message::Binary(b) => assert_eq!(&b[..], b"ola host"),
        outro => panic!("esperava binário, veio {outro:?}"),
    }

    // host → guest
    host.send(Message::Binary(b"ola celular".to_vec())).await.unwrap();
    match guest.next().await.unwrap().unwrap() {
        Message::Binary(b) => assert_eq!(&b[..], b"ola celular"),
        outro => panic!("esperava binário, veio {outro:?}"),
    }
}

#[tokio::test]
async fn salas_diferentes_nao_se_misturam() {
    let port = sobe().await;
    let mut host_a = conecta(port, "sala-a", "host").await;
    let mut guest_b = conecta(port, "sala-b", "guest").await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    guest_b.send(Message::Binary(b"so na B".to_vec())).await.unwrap();
    // host_a não pode receber nada da sala B.
    let nada = tokio::time::timeout(std::time::Duration::from_millis(300), host_a.next()).await;
    assert!(nada.is_err(), "não pode vazar entre salas");
}
