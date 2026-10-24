//! Ponte FFI (ABI C) do núcleo do túnel do DotCrow.
//!
//! O app iOS (SwiftUI) cuida da rede (WebSocket com o relay, via `URLSessionWebSocketTask`)
//! e da moldura dos frames (`tag || payload` — o 1º byte é o tipo). **Toda a criptografia**
//! — o handshake de ponta a ponta e a cifra da sessão — fica aqui, reusando
//! [`dotcrow_tunel`] byte a byte, para o celular e o computador falarem exatamente o mesmo
//! protocolo. Nada de reimplementar cripto em Swift.
//!
//! ## Como o app usa
//!
//! 1. Lê o QR (`dotcrow://p?...`) → [`dc_parse_pairing`] devolve relay, sala, token e janela.
//! 2. Conecta no relay (`wss://…/t?sala=…&rol=guest`) e manda `HI` (1 byte: `0x01`).
//! 3. Recebe `HELLO` (tag `0x02`); passa o **payload** (sem o byte de tag) para
//!    [`dc_parear`] (1º pareamento) ou [`dc_reconectar`] (com o código guardado). Recebe
//!    de volta o frame `KEY` pronto e um handle de sessão ([`DcSessao`]).
//! 4. Manda o `KEY` como está. Recebe `OK` (tag `0x04`); passa o payload para
//!    [`dc_sessao_decifrar`] — se abrir, o túnel está de pé.
//! 5. No 1º pareamento, recebe `CRED` (tag `0x05`): decifra e guarda o **código durável**
//!    no Keychain. Nas próximas vezes, reconecta sem QR.
//! 6. Daí em diante, cada requisição à API do painel vai num `DATA` (tag `0x10`):
//!    [`dc_sessao_cifrar`] no envio, [`dc_sessao_decifrar`] na resposta.
//!
//! ## Memória
//!
//! Todo buffer devolvido ([`DcBytes`]) foi alocado pelo Rust e **tem de ser liberado** com
//! [`dc_bytes_free`]. Toda sessão ([`DcSessao`]) tem de ser liberada com [`dc_sessao_free`].
//! As funções escrevem o resultado em ponteiros de saída e devolvem `0` em sucesso ou `1`
//! em erro (com a mensagem, em UTF-8, em `out_err`).

#![allow(clippy::missing_safety_doc)] // cada função tem a seção "# Segurança" em prosa.

use std::ffi::c_char;

use dotcrow_tunel::{Guest, Sessao};

/// Código de retorno: deu certo.
pub const DC_OK: i32 = 0;
/// Código de retorno: deu erro (a mensagem fica em `out_err`).
pub const DC_ERRO: i32 = 1;

/// Um buffer de bytes dono da própria memória, alocado pelo Rust.
///
/// Libere **sempre** com [`dc_bytes_free`]. Um `ptr` nulo com `len`/`cap` zero é um buffer
/// vazio e válido (por exemplo, `out_err` quando não houve erro).
#[repr(C)]
pub struct DcBytes {
    /// Início dos bytes (ou nulo, se vazio).
    pub ptr: *mut u8,
    /// Quantos bytes valem.
    pub len: usize,
    /// Capacidade alocada (uso interno do Rust ao liberar; o app não mexe).
    pub cap: usize,
}

impl DcBytes {
    fn vazio() -> Self {
        Self { ptr: std::ptr::null_mut(), len: 0, cap: 0 }
    }
}

/// A sessão cifrada do túnel (opaca para o app; só um ponteiro).
pub struct DcSessao {
    sessao: Sessao,
}

/// Transforma um `Vec<u8>` num [`DcBytes`] cedendo a posse ao chamador (C).
fn ceder(mut v: Vec<u8>) -> DcBytes {
    let ptr = v.as_mut_ptr();
    let len = v.len();
    let cap = v.capacity();
    std::mem::forget(v);
    DcBytes { ptr, len, cap }
}

/// Escreve `msg` em `*out_err` (se não for nulo) e devolve [`DC_ERRO`].
unsafe fn falhar(out_err: *mut DcBytes, msg: impl Into<String>) -> i32 {
    if !out_err.is_null() {
        unsafe { *out_err = ceder(msg.into().into_bytes()) };
    }
    DC_ERRO
}

/// Lê uma string C (`*const c_char`) como `&str`, ou `None` se for nula/UTF-8 inválido.
unsafe fn ler_cstr<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    unsafe { std::ffi::CStr::from_ptr(p) }.to_str().ok()
}

/// Lê um `(ptr, len)` como fatia de bytes, ou uma fatia vazia se o ponteiro for nulo.
unsafe fn ler_bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() || len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(ptr, len) } }
}

/// Dá cabo de um handshake do lado do celular e devolve o frame `KEY` + a sessão.
unsafe fn concluir(
    guest: Guest,
    hello_ptr: *const u8,
    hello_len: usize,
    out_key: *mut DcBytes,
    out_sessao: *mut *mut DcSessao,
    out_err: *mut DcBytes,
) -> i32 {
    if out_key.is_null() || out_sessao.is_null() {
        return unsafe { falhar(out_err, "ponteiros de saída nulos") };
    }
    let hello = unsafe { ler_bytes(hello_ptr, hello_len) };
    match guest.responder_hello(hello) {
        Ok((key_frame, sessao)) => {
            unsafe {
                *out_key = ceder(key_frame);
                *out_sessao = Box::into_raw(Box::new(DcSessao { sessao }));
                if !out_err.is_null() {
                    *out_err = DcBytes::vazio();
                }
            }
            DC_OK
        }
        Err(e) => unsafe { falhar(out_err, e) },
    }
}

/// 1º pareamento (por QR): `sala`, `token` (`pt`) e `janela` (`w`) vêm do QR; `hello` é o
/// payload do frame `HELLO` (sem o byte de tag). Em sucesso, `*out_key` recebe o frame
/// `KEY` para mandar ao relay e `*out_sessao` recebe a sessão.
///
/// # Segurança
/// `sala` e `token` são strings C válidas e terminadas em NUL. `hello_ptr`/`hello_len`
/// descrevem uma região legível (ou `hello_ptr` nulo com `hello_len` 0). Os ponteiros de
/// saída, se não nulos, apontam para memória gravável. Libere `*out_sessao` com
/// [`dc_sessao_free`] e `*out_key`/`*out_err` com [`dc_bytes_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dc_parear(
    sala: *const c_char,
    token: *const c_char,
    janela: u64,
    hello_ptr: *const u8,
    hello_len: usize,
    out_key: *mut DcBytes,
    out_sessao: *mut *mut DcSessao,
    out_err: *mut DcBytes,
) -> i32 {
    let (Some(sala), Some(token)) = (unsafe { ler_cstr(sala) }, unsafe { ler_cstr(token) }) else {
        return unsafe { falhar(out_err, "sala ou token inválidos") };
    };
    let guest = Guest::parear(sala, token, janela);
    unsafe { concluir(guest, hello_ptr, hello_len, out_key, out_sessao, out_err) }
}

/// Reconexão (sem QR): usa o `codigo` durável que o app guardou no 1º pareamento.
/// Mesmos parâmetros de saída de [`dc_parear`].
///
/// # Segurança
/// Igual a [`dc_parear`]: `sala` e `codigo` são strings C válidas; os ponteiros de saída
/// apontam para memória gravável; libere o que sair.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dc_reconectar(
    sala: *const c_char,
    codigo: *const c_char,
    hello_ptr: *const u8,
    hello_len: usize,
    out_key: *mut DcBytes,
    out_sessao: *mut *mut DcSessao,
    out_err: *mut DcBytes,
) -> i32 {
    let (Some(sala), Some(codigo)) = (unsafe { ler_cstr(sala) }, unsafe { ler_cstr(codigo) }) else {
        return unsafe { falhar(out_err, "sala ou código inválidos") };
    };
    let guest = Guest::reconectar(sala, codigo);
    unsafe { concluir(guest, hello_ptr, hello_len, out_key, out_sessao, out_err) }
}

/// Cifra um JSON (uma requisição da API do painel) num frame `DATA` pronto para o relay.
///
/// # Segurança
/// `sessao` é um ponteiro válido vindo de [`dc_parear`]/[`dc_reconectar`] e ainda não
/// liberado. `json_ptr`/`json_len` descrevem uma região legível. `out`/`out_err`, se não
/// nulos, apontam para memória gravável; libere `*out` com [`dc_bytes_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dc_sessao_cifrar(
    sessao: *const DcSessao,
    json_ptr: *const u8,
    json_len: usize,
    out: *mut DcBytes,
    out_err: *mut DcBytes,
) -> i32 {
    if sessao.is_null() || out.is_null() {
        return unsafe { falhar(out_err, "sessão ou saída nula") };
    }
    let sessao = unsafe { &*sessao };
    let json = unsafe { ler_bytes(json_ptr, json_len) };
    match sessao.sessao.cifrar(json) {
        Ok(frame) => {
            unsafe {
                *out = ceder(frame);
                if !out_err.is_null() {
                    *out_err = DcBytes::vazio();
                }
            }
            DC_OK
        }
        Err(e) => unsafe { falhar(out_err, e) },
    }
}

/// Abre um frame vindo do computador. Passe o **payload** (sem o byte de tag) de um `DATA`,
/// `OK` ou `CRED`. Devolve o JSON/credencial em claro em `*out`.
///
/// # Segurança
/// Igual a [`dc_sessao_cifrar`]: `sessao` válido e vivo; `payload_ptr`/`payload_len`
/// legíveis; libere `*out` com [`dc_bytes_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dc_sessao_decifrar(
    sessao: *const DcSessao,
    payload_ptr: *const u8,
    payload_len: usize,
    out: *mut DcBytes,
    out_err: *mut DcBytes,
) -> i32 {
    if sessao.is_null() || out.is_null() {
        return unsafe { falhar(out_err, "sessão ou saída nula") };
    }
    let sessao = unsafe { &*sessao };
    let payload = unsafe { ler_bytes(payload_ptr, payload_len) };
    match sessao.sessao.decifrar(payload) {
        Ok(claro) => {
            unsafe {
                *out = ceder(claro);
                if !out_err.is_null() {
                    *out_err = DcBytes::vazio();
                }
            }
            DC_OK
        }
        Err(e) => unsafe { falhar(out_err, e) },
    }
}

/// Lê um link de pareamento `dotcrow://p?r=<relay>&s=<sala>&pt=<token>&w=<janela>` e
/// devolve as partes (strings UTF-8, sem NUL) em `*out_relay`, `*out_sala`, `*out_token` e a
/// janela em `*out_janela`. Em erro, devolve [`DC_ERRO`] e preenche `out_err`.
///
/// # Segurança
/// `uri` é uma string C válida; os ponteiros de saída, se não nulos, apontam para memória
/// gravável. Libere cada `*out_*` de bytes com [`dc_bytes_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dc_parse_pairing(
    uri: *const c_char,
    out_relay: *mut DcBytes,
    out_sala: *mut DcBytes,
    out_token: *mut DcBytes,
    out_janela: *mut u64,
    out_err: *mut DcBytes,
) -> i32 {
    let Some(uri) = (unsafe { ler_cstr(uri) }) else {
        return unsafe { falhar(out_err, "uri inválida") };
    };
    let Some(partes) = parse_pairing(uri) else {
        return unsafe { falhar(out_err, "link de pareamento mal formado") };
    };
    unsafe {
        if !out_relay.is_null() {
            *out_relay = ceder(partes.relay.into_bytes());
        }
        if !out_sala.is_null() {
            *out_sala = ceder(partes.sala.into_bytes());
        }
        if !out_token.is_null() {
            *out_token = ceder(partes.token.into_bytes());
        }
        if !out_janela.is_null() {
            *out_janela = partes.janela;
        }
        if !out_err.is_null() {
            *out_err = DcBytes::vazio();
        }
    }
    DC_OK
}

/// Libera um [`DcBytes`] devolvido por esta biblioteca. Chamar com um buffer vazio
/// (ptr nulo) não faz nada. Não chame duas vezes no mesmo buffer.
///
/// # Segurança
/// `b` tem de ser um [`DcBytes`] vindo desta biblioteca e ainda não liberado.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dc_bytes_free(b: DcBytes) {
    if !b.ptr.is_null() {
        drop(unsafe { Vec::from_raw_parts(b.ptr, b.len, b.cap) });
    }
}

/// Libera uma [`DcSessao`]. Chamar com nulo não faz nada.
///
/// # Segurança
/// `sessao` tem de ser um ponteiro vindo de [`dc_parear`]/[`dc_reconectar`] e ainda não
/// liberado.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dc_sessao_free(sessao: *mut DcSessao) {
    if !sessao.is_null() {
        drop(unsafe { Box::from_raw(sessao) });
    }
}

/// A versão da biblioteca, como string C estática (não liberar).
#[unsafe(no_mangle)]
pub extern "C" fn dc_versao() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// As partes de um link de pareamento.
struct Pareamento {
    relay: String,
    sala: String,
    token: String,
    janela: u64,
}

/// Lê `dotcrow://p?r=…&s=…&pt=…&w=…` (sem depender de crate de URL).
fn parse_pairing(uri: &str) -> Option<Pareamento> {
    let resto = uri.strip_prefix("dotcrow://p?")?;
    let (mut relay, mut sala, mut token, mut janela) = (None, None, None, None);
    for par in resto.split('&') {
        let (chave, valor) = par.split_once('=')?;
        let valor = url_decode(valor);
        match chave {
            "r" => relay = Some(valor),
            "s" => sala = Some(valor),
            "pt" => token = Some(valor),
            "w" => janela = valor.parse::<u64>().ok(),
            _ => {}
        }
    }
    Some(Pareamento { relay: relay?, sala: sala?, token: token?, janela: janela? })
}

/// Decodifica os `%XX` de um pedaço de URL (o esc mínimo que o túnel usa).
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(b) = hex2(bytes[i + 1], bytes[i + 2])
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex2(a: u8, b: u8) -> Option<u8> {
    Some(hex1(a)? << 4 | hex1(b)?)
}

fn hex1(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    /// Vai e volta pela ABI C de verdade: pareia, abre o KEY no host e troca dados nos dois
    /// sentidos — a mesma cripto que o app usará.
    #[test]
    fn handshake_e_dados_pela_abi() {
        use dotcrow_crypto::envelope::SecretIdentity;
        use dotcrow_crypto::pairing_token;
        use dotcrow_tunel::{Host, janela_atual, sala_de, split};

        let secreta = SecretIdentity::generate().unwrap();
        let sala = sala_de(&secreta.public());
        let codigo = "codigo-durave-l0";
        let host = Host::new(&secreta, codigo);

        // O "QR" da janela atual.
        let janela = janela_atual();
        let pt = pairing_token(codigo, janela);

        let hello = host.hello();
        let (_t, hello_payload) = split(&hello).unwrap();

        // --- lado do app, só pela ABI C ---
        let c_sala = CString::new(sala.clone()).unwrap();
        let c_pt = CString::new(pt).unwrap();
        let mut key = DcBytes::vazio();
        let mut sessao: *mut DcSessao = std::ptr::null_mut();
        let mut err = DcBytes::vazio();
        let rc = unsafe {
            dc_parear(
                c_sala.as_ptr(),
                c_pt.as_ptr(),
                janela,
                hello_payload.as_ptr(),
                hello_payload.len(),
                &mut key,
                &mut sessao,
                &mut err,
            )
        };
        assert_eq!(rc, DC_OK, "dc_parear falhou");
        assert!(!sessao.is_null());

        // O host abre o KEY produzido pela ABI.
        let key_frame = unsafe { std::slice::from_raw_parts(key.ptr, key.len) };
        let (_t, key_payload) = split(key_frame).unwrap();
        let (sessao_host, cred) = host.aceitar_key(key_payload).unwrap();
        assert_eq!(cred.as_deref(), Some(codigo), "pareamento devolve o código durável");

        // App cifra um pedido; host decifra.
        let pedido = br#"{"id":1,"metodo":"GET","caminho":"/api/estado"}"#;
        let mut cif = DcBytes::vazio();
        let rc = unsafe { dc_sessao_cifrar(sessao, pedido.as_ptr(), pedido.len(), &mut cif, std::ptr::null_mut()) };
        assert_eq!(rc, DC_OK);
        let frame = unsafe { std::slice::from_raw_parts(cif.ptr, cif.len) };
        let (_t, payload) = split(frame).unwrap();
        assert_eq!(&sessao_host.decifrar(payload).unwrap()[..], &pedido[..]);

        // Host cifra uma resposta; app decifra pela ABI.
        let resp = sessao_host.cifrar(b"ok do host").unwrap();
        let (_t, resp_payload) = split(&resp).unwrap();
        let mut claro = DcBytes::vazio();
        let rc = unsafe {
            dc_sessao_decifrar(sessao, resp_payload.as_ptr(), resp_payload.len(), &mut claro, std::ptr::null_mut())
        };
        assert_eq!(rc, DC_OK);
        let claro_bytes = unsafe { std::slice::from_raw_parts(claro.ptr, claro.len) };
        assert_eq!(claro_bytes, b"ok do host");

        unsafe {
            dc_bytes_free(key);
            dc_bytes_free(cif);
            dc_bytes_free(claro);
            dc_sessao_free(sessao);
        }
    }

    #[test]
    fn le_link_de_pareamento() {
        assert!(parse_pairing("dotcrow://x").is_none());
        let p = parse_pairing("dotcrow://p?r=wss%3A%2F%2Fdotcrow.si%2Ft&s=abc123&pt=deadbeef&w=42").expect("devia ler");
        assert_eq!(p.relay, "wss://dotcrow.si/t");
        assert_eq!(p.sala, "abc123");
        assert_eq!(p.token, "deadbeef");
        assert_eq!(p.janela, 42);
    }

    #[test]
    fn link_pela_abi() {
        let uri = CString::new("dotcrow://p?r=wss%3A%2F%2Fr&s=sala&pt=tok&w=7").unwrap();
        let (mut relay, mut sala, mut token) = (DcBytes::vazio(), DcBytes::vazio(), DcBytes::vazio());
        let mut janela = 0u64;
        let rc = unsafe {
            dc_parse_pairing(uri.as_ptr(), &mut relay, &mut sala, &mut token, &mut janela, std::ptr::null_mut())
        };
        assert_eq!(rc, DC_OK);
        let s = |b: &DcBytes| unsafe { std::slice::from_raw_parts(b.ptr, b.len) }.to_vec();
        assert_eq!(s(&relay), b"wss://r");
        assert_eq!(s(&sala), b"sala");
        assert_eq!(s(&token), b"tok");
        assert_eq!(janela, 7);
        unsafe {
            dc_bytes_free(relay);
            dc_bytes_free(sala);
            dc_bytes_free(token);
        }
    }
}
