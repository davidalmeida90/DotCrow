//! Núcleo do túnel remoto do DotCrow, compartilhado pelo computador e pelo app.
//!
//! Tem a parte que não depende de rede: a **identidade** do computador, o **handshake**
//! de ponta a ponta e a **sessão** cifrada. A ponte que conversa com o relay (no lado do
//! computador) fica no `dotcrow-cli`; o app (SwiftUI) usa este mesmo núcleo via FFI
//! (`dotcrow-mobile`), para falar byte a byte a mesma criptografia.
//!
//! ## Handshake (resumo)
//!
//! 1. O celular entra na sala e manda `HI`.
//! 2. O computador responde `HELLO` com sua identidade pública (é pública, vai em claro).
//! 3. O celular confere que `room_id(identidade) == sala` (a sala veio no QR). Isso
//!    impede um impostor no relay de se passar pelo computador.
//! 4. O celular sorteia uma chave de sessão `K`, e manda `KEY` = um envelope
//!    **pós-quântico** selado para a identidade do computador, contendo o código de
//!    acesso + `K`. Só o computador abre; o relay não vê nada.
//! 5. O computador abre, confere o código de acesso (autentica o celular) e adota `K`.
//! 6. Dali em diante, tudo vai cifrado com `K` (XChaCha20-Poly1305). O relay só vê ruído.

use std::path::{Path, PathBuf};

use dotcrow_crypto::{
    envelope::{PublicIdentity, SecretIdentity},
    room_id, sym,
};
use dotcrow_vault::write_private;

/// Dados associados que amarram os frames de dados a este protocolo.
const DATA_AAD: &[u8] = b"dotcrow/remoto/v1/data";
/// Dados associados do envelope do handshake.
const KEY_AAD: &[u8] = b"dotcrow/remoto/v1/key";

/// Marcadores de cada tipo de frame no túnel (o primeiro byte).
pub mod tag {
    /// Celular → computador: cheguei, me mande sua identidade.
    pub const HI: u8 = 0x01;
    /// Computador → celular: minha identidade pública (em claro).
    pub const HELLO: u8 = 0x02;
    /// Celular → computador: envelope selado com {código de acesso, chave de sessão}.
    pub const KEY: u8 = 0x03;
    /// Computador → celular: confirmação cifrada com a chave de sessão.
    pub const OK: u8 = 0x04;
    /// Computador → celular (só no 1º pareamento): o código durável, cifrado, para o
    /// celular guardar e reconectar depois sem ler o QR de novo.
    pub const CRED: u8 = 0x05;
    /// Ambos os lados: um frame de dados (requisição/resposta da API), cifrado.
    pub const DATA: u8 = 0x10;
    /// Computador → celular: erro de handshake em texto claro (ex.: código errado).
    pub const ERRO: u8 = 0x1f;
}

/// Monta um frame: `tag || payload`.
pub fn frame(t: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(1 + payload.len());
    v.push(t);
    v.extend_from_slice(payload);
    v
}

/// Separa o `tag` do resto de um frame.
pub fn split(f: &[u8]) -> Option<(u8, &[u8])> {
    f.split_first().map(|(t, rest)| (*t, rest))
}

/// A identidade deste computador: carrega de `identidade.bin`, ou cria na primeira vez.
pub fn carregar_identidade(dir: &Path) -> Result<SecretIdentity, String> {
    let caminho = caminho_identidade(dir);
    match std::fs::read(&caminho) {
        Ok(bytes) => SecretIdentity::from_secret_bytes(&bytes).map_err(|e| format!("identidade inválida: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let id = SecretIdentity::generate().map_err(|e| e.to_string())?;
            write_private(&caminho, &id.to_secret_bytes()).map_err(|e| e.to_string())?;
            Ok(id)
        }
        Err(e) => Err(e.to_string()),
    }
}

fn caminho_identidade(dir: &Path) -> PathBuf {
    dir.join("identidade.bin")
}

/// Id da sala deste computador (derivado da identidade pública).
pub fn sala_de(identidade: &PublicIdentity) -> String {
    room_id(&identidade.to_bytes())
}

/// Chave de sessão combinada do túnel.
pub struct Sessao {
    k: sym::SymKey,
}

impl Sessao {
    /// Cifra um frame de dados (uma requisição ou resposta da API) para o outro lado.
    pub fn cifrar(&self, json: &[u8]) -> Result<Vec<u8>, String> {
        sym::seal(&self.k, json, DATA_AAD).map(|c| frame(tag::DATA, &c)).map_err(|e| e.to_string())
    }

    /// Abre um frame de dados vindo do outro lado (sem o byte de tag).
    pub fn decifrar(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        sym::open(&self.k, payload, DATA_AAD).map(|z| z.to_vec()).map_err(|e| e.to_string())
    }
}

/// Duração de cada janela do QR: ele troca a cada 10 s.
pub const JANELA_SEGUNDOS: u64 = 10;
/// Quantas janelas para trás o host ainda aceita (tolera atraso de leitura e relógio).
const JANELAS_TOLERADAS: u64 = 3;

/// A janela de tempo atual (o tempo em blocos de 10 s).
pub fn janela_atual() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
        / JANELA_SEGUNDOS
}

const KIND_CODIGO: u8 = 0;
const KIND_PAREAMENTO: u8 = 1;

/// O que o celular prova no handshake.
enum Credencial {
    /// Código durável (reconexão, sem QR).
    Codigo(String),
    /// Token do QR + a janela em que foi gerado (1º pareamento).
    Pareamento { token: String, janela: u64 },
}

/// O lado do computador (host) no handshake.
pub struct Host<'a> {
    secreta: &'a SecretIdentity,
    codigo: &'a str,
}

impl<'a> Host<'a> {
    /// `codigo` é o código de acesso do painel (o segredo durável).
    pub fn new(secreta: &'a SecretIdentity, codigo: &'a str) -> Self {
        Self { secreta, codigo }
    }

    /// Resposta ao `HI` do celular: a identidade pública, em claro.
    pub fn hello(&self) -> Vec<u8> {
        frame(tag::HELLO, &self.secreta.public().to_bytes())
    }

    /// Recebe o frame `KEY`, confere a credencial e devolve a sessão. O segundo campo é
    /// `Some(código durável)` quando foi um 1º pareamento (por QR): a ponte manda esse
    /// código cifrado ao celular, que guarda para reconectar depois sem ler o QR.
    pub fn aceitar_key(&self, payload: &[u8]) -> Result<(Sessao, Option<String>), String> {
        let aberto = dotcrow_crypto::envelope::open(self.secreta, payload, KEY_AAD)
            .map_err(|_| "não consegui abrir o envelope do celular".to_string())?;
        let (cred, k) = ler_key(&aberto).ok_or("formato do KEY inválido")?;
        match cred {
            Credencial::Codigo(c) => {
                if iguais(c.as_bytes(), self.codigo.as_bytes()) {
                    Ok((Sessao { k }, None))
                } else {
                    Err("código de acesso errado".into())
                }
            }
            Credencial::Pareamento { token, janela } => {
                let agora = janela_atual();
                let recente = janela + JANELAS_TOLERADAS >= agora && janela <= agora + 1;
                let esperado = dotcrow_crypto::pairing_token(self.codigo, janela);
                if recente && iguais(token.as_bytes(), esperado.as_bytes()) {
                    Ok((Sessao { k }, Some(self.codigo.to_string())))
                } else {
                    Err("QR expirado ou inválido; aponte para o QR atual".into())
                }
            }
        }
    }
}

/// O lado do celular, aqui em Rust só para testes e como referência do protocolo que o
/// app (SwiftUI) vai implementar.
#[allow(dead_code)]
pub struct Guest {
    sala_esperada: String,
    cred: Credencial,
}

#[allow(dead_code)]
impl Guest {
    /// Primeiro pareamento: `token` e `janela` (`pt` e `w`) vêm do QR; `sala`, do QR também.
    pub fn parear(sala: &str, token: &str, janela: u64) -> Self {
        Self { sala_esperada: sala.to_string(), cred: Credencial::Pareamento { token: token.to_string(), janela } }
    }

    /// Reconexão: usa o código durável que o celular guardou no 1º pareamento.
    pub fn reconectar(sala: &str, codigo: &str) -> Self {
        Self { sala_esperada: sala.to_string(), cred: Credencial::Codigo(codigo.to_string()) }
    }

    /// Recebe o `HELLO`, confere que a sala bate com a identidade (anti-impostor) e
    /// devolve o frame `KEY` já selado, mais a sessão derivada da chave sorteada.
    pub fn responder_hello(&self, hello_payload: &[u8]) -> Result<(Vec<u8>, Sessao), String> {
        let ident = PublicIdentity::from_bytes(hello_payload).map_err(|_| "identidade do HELLO inválida")?;
        if sala_de(&ident) != self.sala_esperada {
            return Err("a sala não bate com a identidade do computador (possível impostor)".into());
        }
        let k = sym::generate_key().map_err(|e| e.to_string())?;
        let plano = escrever_key(&self.cred, &k);
        let env = dotcrow_crypto::envelope::seal(&ident, &plano, KEY_AAD).map_err(|e| e.to_string())?;
        Ok((frame(tag::KEY, &env), Sessao { k }))
    }
}

/// Serializa a credencial + a chave de sessão para dentro do envelope.
#[allow(dead_code)]
fn escrever_key(cred: &Credencial, k: &sym::SymKey) -> Vec<u8> {
    let mut v = Vec::new();
    match cred {
        Credencial::Codigo(c) => {
            let c = c.as_bytes();
            v.push(KIND_CODIGO);
            v.push(c.len().min(255) as u8);
            v.extend_from_slice(&c[..c.len().min(255)]);
        }
        Credencial::Pareamento { token, janela } => {
            let t = token.as_bytes();
            v.push(KIND_PAREAMENTO);
            v.push(t.len().min(255) as u8);
            v.extend_from_slice(&t[..t.len().min(255)]);
            v.extend_from_slice(&janela.to_le_bytes());
        }
    }
    v.extend_from_slice(k.as_ref());
    v
}

fn ler_key(bytes: &[u8]) -> Option<(Credencial, sym::SymKey)> {
    let (&kind, rest) = bytes.split_first()?;
    let (&n, rest) = rest.split_first()?;
    let n = n as usize;
    let texto = String::from_utf8(rest.get(..n)?.to_vec()).ok()?;
    let rest = &rest[n..];
    let (cred, rest) = match kind {
        KIND_CODIGO => (Credencial::Codigo(texto), rest),
        KIND_PAREAMENTO => {
            let janela = u64::from_le_bytes(rest.get(..8)?.try_into().ok()?);
            (Credencial::Pareamento { token: texto, janela }, &rest[8..])
        }
        _ => return None,
    };
    if rest.len() != 32 {
        return None;
    }
    let mut k = zeroize::Zeroizing::new([0u8; 32]);
    k.copy_from_slice(rest);
    Some((cred, k))
}

/// Comparação de bytes em tempo constante.
fn iguais(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Monta o link de pareamento que vira QR, para a `janela` dada. O app registra o esquema
/// `dotcrow://`. O QR leva só um **token de curta duração** (muda a cada 10 s), nunca o
/// código durável. `relay` é a base WebSocket, ex.: `wss://dotcrow.si/t`.
pub fn link_pareamento(relay: &str, sala: &str, codigo: &str, janela: u64) -> String {
    let pt = dotcrow_crypto::pairing_token(codigo, janela);
    format!("dotcrow://p?r={}&s={}&pt={}&w={}", esc(relay), esc(sala), esc(&pt), janela)
}

/// Escape mínimo de URL para os poucos caracteres que aparecem aqui.
fn esc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_completo_e_dados_nos_dois_sentidos() {
        let secreta = SecretIdentity::generate().unwrap();
        let sala = sala_de(&secreta.public());
        let codigo = "abcd-efgh-jkmn-pqrs";

        let host = Host::new(&secreta, codigo);
        let guest = Guest::reconectar(&sala, codigo);

        // HI → HELLO → KEY → OK
        let hello = host.hello();
        let (_t, hello_p) = split(&hello).unwrap();
        let (key_frame, sessao_guest) = match guest.responder_hello(hello_p) {
            Ok(x) => x,
            Err(e) => panic!("responder_hello falhou: {e}"),
        };
        let (_t, key_payload) = split(&key_frame).unwrap();
        let (sessao_host, cred) = match host.aceitar_key(key_payload) {
            Ok(s) => s,
            Err(e) => panic!("aceitar_key falhou: {e}"),
        };
        assert!(cred.is_none(), "reconexão não reenvia o código");

        // Os dois lados têm a mesma chave: o que um cifra, o outro abre.
        let pedido = br#"{"method":"GET","path":"/api/estado"}"#;
        let c1 = sessao_guest.cifrar(pedido).unwrap();
        let (_t, cif) = split(&c1).unwrap();
        assert_eq!(&sessao_host.decifrar(cif).unwrap()[..], &pedido[..]);
        let c2 = sessao_host.cifrar(b"resposta").unwrap();
        let (_t, cif2) = split(&c2).unwrap();
        assert_eq!(&sessao_guest.decifrar(cif2).unwrap()[..], b"resposta");
    }

    #[test]
    fn codigo_errado_e_recusado() {
        let secreta = SecretIdentity::generate().unwrap();
        let sala = sala_de(&secreta.public());
        let host = Host::new(&secreta, "codigo-certo-000");
        let guest = Guest::reconectar(&sala, "codigo-errado-00");
        let hello = host.hello();
        let (_t, hello_p) = split(&hello).unwrap();
        let (key_frame, _s) = match guest.responder_hello(hello_p) {
            Ok(x) => x,
            Err(e) => panic!("responder_hello falhou: {e}"),
        };
        let (_t, key_payload) = split(&key_frame).unwrap();
        match host.aceitar_key(key_payload) {
            Ok(_) => panic!("devia recusar o código errado"),
            Err(e) => assert_eq!(e, "código de acesso errado"),
        }
    }

    #[test]
    fn sala_de_outra_identidade_e_impostor() {
        let real = SecretIdentity::generate().unwrap();
        let impostor = SecretIdentity::generate().unwrap();
        // O celular espera a sala do computador real, mas recebe o HELLO do impostor.
        let guest = Guest::reconectar(&sala_de(&real.public()), "x");
        let hello_impostor = impostor.public().to_bytes();
        match guest.responder_hello(&hello_impostor) {
            Ok(_) => panic!("devia recusar um HELLO de outra identidade"),
            Err(e) => assert!(e.contains("impostor"), "erro foi: {e}"),
        }
    }

    #[test]
    fn pareamento_por_qr_rotativo() {
        let secreta = SecretIdentity::generate().unwrap();
        let sala = sala_de(&secreta.public());
        let codigo = "codigo-durave-l0";
        let host = Host::new(&secreta, codigo);

        // QR da janela atual: o celular lê pt + w e pareia.
        let janela = janela_atual();
        let pt = dotcrow_crypto::pairing_token(codigo, janela);
        let guest = Guest::parear(&sala, &pt, janela);
        let hello = host.hello();
        let (_t, hello_p) = split(&hello).unwrap();
        let (key_frame, _sg) = guest.responder_hello(hello_p).unwrap();
        let (_t, key_payload) = split(&key_frame).unwrap();
        let (_sh, cred) = host.aceitar_key(key_payload).unwrap();
        assert_eq!(cred.as_deref(), Some(codigo), "pareamento por QR devolve o código durável para guardar");

        // QR velho (janela expirada) é recusado.
        let velho = janela - (JANELAS_TOLERADAS + 2);
        let pt_velho = dotcrow_crypto::pairing_token(codigo, velho);
        let guest2 = Guest::parear(&sala, &pt_velho, velho);
        let (kf2, _s) = guest2.responder_hello(hello_p).unwrap();
        let (_t, kp2) = split(&kf2).unwrap();
        assert!(host.aceitar_key(kp2).is_err(), "QR expirado deve ser recusado");
    }

    #[test]
    fn link_de_pareamento_rotativo() {
        let l = link_pareamento("wss://dotcrow.si/t", "abcd1234", "co-de", 42);
        assert!(l.starts_with("dotcrow://p?r=wss%3A%2F%2Fdotcrow.si%2Ft&s=abcd1234&pt="));
        assert!(l.ends_with("&w=42"));
        assert!(!l.contains("co-de"), "o código durável nunca entra no QR");
    }
}
