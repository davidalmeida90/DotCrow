//! Mãos no celular Android: ler a tela, tocar, segurar, rolar, deslizar, digitar, apertar
//! botões, tirar print, abrir apps, compartilhar texto com um app (ex.: o X), abrir links,
//! ligar, mandar SMS, falar, ouvir e notificar.
//!
//! Dois caminhos, e o DotCrow usa o que estiver disponível:
//!
//! - **ADB** (`adb`; no Termux, `pkg install android-tools`) controla a tela. No próprio
//!   celular: Opções do desenvolvedor → Depuração sem fio → parear com código, e uma vez só
//!   `adb pair IP:PORTA CÓDIGO` e `adb connect IP:PORTA`. No PC: o celular no cabo USB com a
//!   depuração USB ligada. Com vários aparelhos, defina `ANDROID_SERIAL`.
//! - **Termux:API** (o app Termux:API + `pkg install termux-api`): ligar, SMS, falar, ouvir,
//!   notificação e área de transferência (que também serve para digitar com acento).
//!
//! Os corvos não enxergam imagens: para saber o que há na tela eles usam `tela`, que lê a
//! árvore da interface (`uiautomator dump`) e devolve cada botão e texto com a posição. Aí
//! dá para `tocar {texto: "Postar"}` sem saber coordenadas.
//!
//! Cada ação passa por duas fases: [`planejar`] descobre o que vai acontecer e quanto isso
//! pesa ([`Permissao`]); as mãos perguntam ao dono quando precisa; [`executar`] faz. Tocar em
//! um botão de publicar, enviar, pagar ou apagar, ligar e mandar SMS são
//! [`Permissao::Irreversivel`]: o dono confirma sempre, mesmo no modo Livre.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use dotcrow_agents::hands::{ToolCall, ToolOutput};

/// Um elemento da tela do celular.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elemento {
    /// Texto visível.
    pub texto: String,
    /// Descrição de acessibilidade (ícones sem texto costumam ter).
    pub desc: String,
    /// Id do recurso (ex.: `com.twitter.android:id/button_tweet`).
    pub id: String,
    /// Se dá para tocar.
    pub clicavel: bool,
    /// Limites na tela: esquerda, topo, direita, base.
    pub limites: [i64; 4],
    /// Centro, em pixels.
    pub x: i64,
    /// Centro, em pixels.
    pub y: i64,
}

impl Elemento {
    /// Nome para mostrar: o texto, a descrição ou o fim do id.
    pub fn nome(&self) -> String {
        let mut nome = self.texto.clone();
        if !self.desc.is_empty() && self.desc != self.texto {
            nome = if nome.is_empty() { self.desc.clone() } else { format!("{nome} ({})", self.desc) };
        }
        if nome.is_empty() {
            nome = self.id.rsplit('/').next().unwrap_or("").to_string();
        }
        nome.chars().take(80).collect()
    }
}

fn atributo(no: &str, nome: &str) -> String {
    let chave = format!(" {nome}=\"");
    let Some(i) = no.find(&chave) else { return String::new() };
    let resto = &no[i + chave.len()..];
    let fim = resto.find('"').unwrap_or(resto.len());
    resto[..fim]
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#10;", " ")
}

/// Lê o XML do `uiautomator dump` e devolve os elementos que têm texto, descrição ou são
/// tocáveis.
pub fn ler_tela(xml: &str) -> Vec<Elemento> {
    let mut out = Vec::new();
    for pedaco in xml.split("<node").skip(1) {
        let no = pedaco.split('>').next().unwrap_or("");
        let texto = atributo(no, "text");
        let desc = atributo(no, "content-desc");
        let id = atributo(no, "resource-id");
        let clicavel = atributo(no, "clickable") == "true";
        let nums: Vec<i64> = atributo(no, "bounds")
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if nums.len() != 4 || (texto.is_empty() && desc.is_empty() && !clicavel) {
            continue;
        }
        let limites = [nums[0], nums[1], nums[2], nums[3]];
        out.push(Elemento {
            texto,
            desc,
            id,
            clicavel,
            limites,
            x: (nums[0] + nums[2]) / 2,
            y: (nums[1] + nums[3]) / 2,
        });
    }
    out
}

fn normalizar(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            o => o,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Acha o elemento pelo texto, descrição ou id: primeiro igual, depois "contém";
/// tocáveis vencem.
pub fn achar<'a>(els: &'a [Elemento], alvo: &str) -> Option<&'a Elemento> {
    let a = normalizar(alvo);
    if a.is_empty() {
        return None;
    }
    let campos = |e: &Elemento| [normalizar(&e.texto), normalizar(&e.desc), normalizar(&e.id)];
    let igual = |e: &&Elemento| campos(e).iter().any(|c| *c == a || c.ends_with(&format!("/{a}")));
    let contem = |e: &&Elemento| campos(e).iter().any(|c| !c.is_empty() && c.contains(&a));
    els.iter().filter(igual).max_by_key(|e| e.clicavel).or_else(|| els.iter().filter(contem).max_by_key(|e| e.clicavel))
}

/// O elemento que está no ponto (o menor que contém o ponto; tocáveis vencem).
pub fn no_ponto(els: &[Elemento], x: i64, y: i64) -> Option<&Elemento> {
    els.iter()
        .filter(|e| e.limites[0] <= x && x <= e.limites[2] && e.limites[1] <= y && y <= e.limites[3])
        .min_by_key(|e| (!e.clicavel, (e.limites[2] - e.limites[0]) * (e.limites[3] - e.limites[1])))
}

/// Palavras de botões cujo toque não tem volta (publica, envia, paga, apaga, liga...).
const SEM_VOLTA: &[&str] = &[
    "postar",
    "post",
    "publicar",
    "tweet",
    "tuitar",
    "repost",
    "retweet",
    "enviar",
    "send",
    "compartilhar",
    "share",
    "ligar",
    "call",
    "chamar",
    "pagar",
    "pay",
    "comprar",
    "buy",
    "assinar",
    "subscribe",
    "confirmar",
    "confirm",
    "excluir",
    "apagar",
    "delete",
    "remover",
    "remove",
    "transferir",
    "transfer",
    "pix",
    "responder",
    "reply",
    "aceitar",
    "accept",
    "instalar",
    "install",
    "desinstalar",
    "uninstall",
];

/// Se tocar neste elemento publica, envia, paga, apaga ou liga (pede confirmação sempre).
pub fn sem_volta(e: &Elemento) -> bool {
    let tudo = normalizar(&format!("{} {} {}", e.texto, e.desc, e.id.rsplit('/').next().unwrap_or("")));
    tudo.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .any(|w| SEM_VOLTA.iter().any(|p| w.starts_with(p)))
}

/// Resumo da tela para o corvo: um elemento por linha, com posição.
pub fn resumo(els: &[Elemento], max: usize) -> String {
    let mut linhas = Vec::new();
    for (i, e) in els.iter().take(max).enumerate() {
        let toque = if e.clicavel { " [tocável]" } else { "" };
        linhas.push(format!("{}. \"{}\" em {},{}{toque}", i + 1, e.nome(), e.x, e.y));
    }
    if els.len() > max {
        linhas.push(format!("… e mais {} elementos", els.len() - max));
    }
    linhas.join("\n")
}

/// Texto para `adb shell input text` (só ASCII): espaço vira `%s` e os símbolos do shell
/// ganham barra.
pub fn escapar_input(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            ' ' => out.push_str("%s"),
            '&' | '|' | ';' | '<' | '>' | '(' | ')' | '$' | '`' | '\\' | '"' | '\'' | '*' | '?' | '~' | '#' | '!'
            | '[' | ']' | '{' | '}' | '%' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\t' => out.push_str("%s"),
            c => out.push(c),
        }
    }
    out
}

/// Põe um texto entre aspas simples para o shell do celular.
pub fn aspas(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Tira os acentos (para quando não há como colar).
pub fn sem_acentos(texto: &str) -> String {
    texto
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' => 'A',
            'é' | 'ê' => 'e',
            'É' | 'Ê' => 'E',
            'í' => 'i',
            'Í' => 'I',
            'ó' | 'ô' | 'õ' => 'o',
            'Ó' | 'Ô' | 'Õ' => 'O',
            'ú' | 'ü' => 'u',
            'Ú' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            c if c.is_ascii() => c,
            _ => ' ',
        })
        .collect()
}

/// Nome do app → pacote Android (os mais comuns; o resto, passe o pacote).
pub fn pacote(app: &str) -> String {
    match normalizar(app).as_str() {
        "x" | "twitter" => "com.twitter.android",
        "whatsapp" | "zap" => "com.whatsapp",
        "whatsapp business" => "com.whatsapp.w4b",
        "instagram" | "insta" => "com.instagram.android",
        "facebook" => "com.facebook.katana",
        "telegram" => "org.telegram.messenger",
        "chrome" => "com.android.chrome",
        "youtube" => "com.google.android.youtube",
        "gmail" => "com.google.android.gm",
        "maps" | "google maps" => "com.google.android.apps.maps",
        "linkedin" => "com.linkedin.android",
        "tiktok" => "com.zhiliaoapp.musically",
        "reddit" => "com.reddit.frontpage",
        "drive" | "google drive" => "com.google.android.apps.docs",
        "termux" => "com.termux",
        "camera" => "com.android.camera",
        outro => return outro.to_string(),
    }
    .to_string()
}

/// Tecla por nome (ou número) → código de tecla do Android.
pub fn tecla(nome: &str) -> Option<String> {
    let n = normalizar(nome);
    if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
        return Some(n);
    }
    let c = match n.as_str() {
        "voltar" | "back" => "4",
        "inicio" | "home" | "casa" => "3",
        "enter" | "ok" => "66",
        "apagar" | "backspace" | "del" => "67",
        "recentes" | "apps recentes" => "187",
        "volume_mais" | "volume+" | "aumentar volume" => "24",
        "volume_menos" | "volume-" | "diminuir volume" => "25",
        "energia" | "power" => "26",
        "menu" => "82",
        "buscar" | "pesquisar" => "84",
        "colar" | "paste" => "279",
        "tab" => "61",
        "espaco" | "space" => "62",
        "desligar" | "encerrar" | "endcall" => "6",
        _ => return None,
    };
    Some(c.to_string())
}

fn so_numero(n: &str) -> String {
    n.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect()
}

/// Procura um programa no PATH.
pub fn tem(prog: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(prog).is_file())).unwrap_or(false)
}

async fn rodar(prog: &str, args: &[String], secs: u64) -> ToolOutput {
    let mut c = tokio::process::Command::new(prog);
    c.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => return ToolOutput::err(format!("não consegui rodar {prog}: {e}")),
    };
    match tokio::time::timeout(Duration::from_secs(secs), child.wait_with_output()).await {
        Ok(Ok(out)) => {
            let mut t = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let e = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if !e.is_empty() {
                if !t.is_empty() {
                    t.push('\n');
                }
                t.push_str(&e);
            }
            ToolOutput { ok: out.status.success(), text: if t.is_empty() { "(feito)".into() } else { t } }
        }
        Ok(Err(e)) => ToolOutput::err(format!("falha em {prog}: {e}")),
        Err(_) => ToolOutput::err(format!("{prog} passou de {secs} s")),
    }
}

fn s(v: &str) -> String {
    v.to_string()
}

const SEM_ADB: &str = "o ADB não está disponível. No Termux: `pkg install android-tools`, ligue Opções do \
desenvolvedor → Depuração sem fio, toque em \"Parear com código\" e rode `adb pair IP:PORTA CÓDIGO` e depois \
`adb connect IP:PORTA`. No PC: celular no cabo USB com a depuração USB ligada. Teste com `dotcrow celular`.";

const SEM_API: &str = "instale o app Termux:API (F-Droid ou GitHub) e rode `pkg install termux-api`";

async fn adb_shell(cmd: &str, secs: u64) -> ToolOutput {
    if !tem("adb") {
        return ToolOutput::err(SEM_ADB);
    }
    let r = rodar("adb", &[s("shell"), s(cmd)], secs).await;
    if !r.ok && (r.text.contains("no devices") || r.text.contains("device offline") || r.text.contains("not found")) {
        return ToolOutput::err(format!("{}\n{SEM_ADB}", r.text));
    }
    r
}

/// Tamanho da tela (largura, altura).
async fn tamanho() -> (i64, i64) {
    let r = adb_shell("wm size", 10).await;
    let t = r.text.rsplit(':').next().unwrap_or("").trim().to_string();
    let mut it = t.split('x').filter_map(|n| n.trim().parse::<i64>().ok());
    (it.next().unwrap_or(1080), it.next().unwrap_or(2340))
}

/// Lê a tela atual (árvore da interface).
pub async fn elementos() -> Result<Vec<Elemento>, String> {
    let r = adb_shell("uiautomator dump /sdcard/dotcrow-tela.xml >/dev/null && cat /sdcard/dotcrow-tela.xml", 30).await;
    if !r.text.contains("<hierarchy") {
        return Err(r.text);
    }
    Ok(ler_tela(&r.text))
}

/// Quanto uma ação no celular pesa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permissao {
    /// Só olha ou avisa (ler a tela, print, falar, notificar).
    Nenhuma,
    /// Mexe na tela (tocar, rolar, digitar, abrir app).
    Controle,
    /// Liga o microfone.
    Microfone,
    /// Não tem volta: ligar, SMS, tocar em publicar/enviar/pagar/apagar. Pergunta sempre.
    Irreversivel,
}

/// O que fazer, já resolvido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acao {
    /// Lê a tela.
    Tela,
    /// Lista os apps instalados.
    Apps,
    /// Tira um print.
    Print,
    /// Um comando no shell do celular (toque, deslize, tecla, abrir app, link).
    Shell(String),
    /// Rola a tela.
    Rolar { gesto: String, vezes: i64, direcao: String },
    /// Digita um texto.
    Digitar(String),
    /// Abre a tela de compartilhar de um app com o texto pronto.
    Compartilhar(String),
    /// Faz uma ligação.
    Ligar(String),
    /// Manda um SMS.
    Sms { numero: String, texto: String },
    /// Fala em voz alta.
    Falar(String),
    /// Ouve pelo microfone.
    Ouvir,
    /// Mostra uma notificação.
    Notificar { titulo: String, texto: String },
}

/// Uma ação planejada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plano {
    /// Quanto pesa.
    pub permissao: Permissao,
    /// O que mostrar ao dono na confirmação.
    pub titulo: String,
    /// O que fazer.
    pub acao: Acao,
    /// O começo da resposta para o corvo (ex.: "Toquei em 900,140 (\"Postar\")").
    pub nota: String,
}

impl Plano {
    fn novo(permissao: Permissao, titulo: impl Into<String>, acao: Acao) -> Self {
        Self { permissao, titulo: titulo.into(), acao, nota: String::new() }
    }
}

/// As ferramentas do celular, para o despacho das mãos.
pub const FERRAMENTAS: &[&str] = &["celular", "ligar", "sms", "falar", "ouvir", "notificar"];

/// Descobre o que a chamada vai fazer (pode ler a tela para achar um botão) e quanto pesa.
pub async fn planejar(call: &ToolCall) -> Result<Plano, ToolOutput> {
    let txt = |k: &str| call.text(k).map(str::to_string);
    match call.tool.as_str() {
        "ligar" => {
            let numero = txt("numero").map(|n| so_numero(&n)).filter(|n| n.len() >= 3);
            let Some(numero) = numero else { return Err(ToolOutput::err("faltou args.numero")) };
            Ok(Plano::novo(Permissao::Irreversivel, format!("ligar para {numero}"), Acao::Ligar(numero)))
        }
        "sms" => {
            let (Some(numero), Some(texto)) = (txt("numero").map(|n| so_numero(&n)), txt("texto")) else {
                return Err(ToolOutput::err("sms precisa de numero e texto"));
            };
            Ok(Plano::novo(
                Permissao::Irreversivel,
                format!("SMS para {numero}: \"{}\"", texto.chars().take(200).collect::<String>()),
                Acao::Sms { numero, texto },
            ))
        }
        "falar" => match txt("texto") {
            Some(t) => Ok(Plano::novo(Permissao::Nenhuma, format!("falar \"{t}\""), Acao::Falar(t))),
            None => Err(ToolOutput::err("faltou args.texto")),
        },
        "ouvir" => Ok(Plano::novo(Permissao::Microfone, "ouvir pelo microfone do celular", Acao::Ouvir)),
        "notificar" => {
            let titulo = txt("titulo").unwrap_or_else(|| "DotCrow".into());
            let texto = txt("texto").unwrap_or_default();
            Ok(Plano::novo(Permissao::Nenhuma, format!("notificar: {titulo}"), Acao::Notificar { titulo, texto }))
        }
        _ => planejar_celular(call).await,
    }
}

async fn planejar_celular(call: &ToolCall) -> Result<Plano, ToolOutput> {
    let acao = call.text("acao").unwrap_or("tela").to_lowercase();
    let xy = match (call.number("x"), call.number("y")) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    let p = match acao.as_str() {
        "tela" | "ler" | "ler_tela" => Plano::novo(Permissao::Nenhuma, "ler a tela do celular", Acao::Tela),
        "apps" => Plano::novo(Permissao::Nenhuma, "listar os apps do celular", Acao::Apps),
        "print" | "captura" | "screenshot" => Plano::novo(Permissao::Nenhuma, "print da tela do celular", Acao::Print),
        "tocar" | "clicar" | "apertar" | "segurar" => {
            let texto = call.text("texto").or_else(|| call.text("botao"));
            let (x, y, elemento) = match (xy, texto) {
                (Some((x, y)), _) => {
                    let el = elementos().await.ok().and_then(|els| no_ponto(&els, x, y).cloned());
                    (x, y, el)
                }
                (None, Some(t)) => {
                    let els =
                        elementos().await.map_err(|e| ToolOutput::err(format!("não consegui ler a tela: {e}")))?;
                    let Some(el) = achar(&els, t).cloned() else {
                        return Err(ToolOutput::err(format!(
                            "não achei \"{t}\" na tela. O que existe agora:\n{}",
                            resumo(&els, 40)
                        )));
                    };
                    (el.x, el.y, Some(el))
                }
                (None, None) => return Err(ToolOutput::err("diga onde tocar: {texto} de um botão ou {x, y}")),
            };
            let rotulo = elemento.as_ref().map(|e| format!(" (\"{}\")", e.nome())).unwrap_or_default();
            let permissao =
                if elemento.as_ref().is_some_and(sem_volta) { Permissao::Irreversivel } else { Permissao::Controle };
            let (cmd, infinitivo, verbo) = if acao == "segurar" {
                let ms = call.number("ms").unwrap_or(800).clamp(200, 5000);
                (format!("input swipe {x} {y} {x} {y} {ms}"), "segurar", "Segurei")
            } else {
                (format!("input tap {x} {y}"), "tocar", "Toquei")
            };
            Plano {
                permissao,
                titulo: format!("{infinitivo} em {x},{y}{rotulo} no celular"),
                acao: Acao::Shell(cmd),
                nota: format!("{verbo} em {x},{y}{rotulo}."),
            }
        }
        "rolar" | "scroll" => {
            let (w, h) = tamanho().await;
            let vezes = call.number("quantidade").unwrap_or(1).clamp(1, 20);
            let direcao = call.text("direcao").unwrap_or("baixo").to_lowercase();
            let (x1, y1, x2, y2) = match direcao.as_str() {
                "cima" | "up" => (w / 2, h * 3 / 10, w / 2, h * 7 / 10),
                "esquerda" | "left" => (w * 2 / 10, h / 2, w * 8 / 10, h / 2),
                "direita" | "right" => (w * 8 / 10, h / 2, w * 2 / 10, h / 2),
                _ => (w / 2, h * 7 / 10, w / 2, h * 3 / 10),
            };
            Plano::novo(
                Permissao::Controle,
                format!("rolar para {direcao} no celular"),
                Acao::Rolar { gesto: format!("input swipe {x1} {y1} {x2} {y2} 350"), vezes, direcao },
            )
        }
        "deslizar" | "swipe" => {
            let (Some(x), Some(y), Some(x2), Some(y2)) =
                (call.number("x"), call.number("y"), call.number("x2"), call.number("y2"))
            else {
                return Err(ToolOutput::err("deslizar precisa de x, y, x2, y2"));
            };
            let ms = call.number("ms").unwrap_or(300).clamp(50, 5000);
            Plano::novo(
                Permissao::Controle,
                format!("deslizar de {x},{y} até {x2},{y2} no celular"),
                Acao::Shell(format!("input swipe {x} {y} {x2} {y2} {ms}")),
            )
        }
        "digitar" | "escrever" => match call.text("texto") {
            Some(t) => Plano::novo(
                Permissao::Controle,
                format!("digitar \"{}\" no celular", t.chars().take(200).collect::<String>()),
                Acao::Digitar(t.to_string()),
            ),
            None => return Err(ToolOutput::err("faltou args.texto")),
        },
        "tecla" | "botao" | "botão" => {
            let nome = call.text("tecla").or_else(|| call.text("nome")).unwrap_or("");
            let Some(cod) = tecla(nome) else {
                return Err(ToolOutput::err(
                    "tecla: voltar, inicio, enter, apagar, recentes, volume_mais, volume_menos, energia, menu, colar, \
tab, espaco, desligar (ou o número do código)",
                ));
            };
            Plano::novo(
                Permissao::Controle,
                format!("apertar {nome} no celular"),
                Acao::Shell(format!("input keyevent {cod}")),
            )
        }
        "abrir" | "abrir_app" | "app" => {
            let Some(app) = call.text("app").or_else(|| call.text("pacote")) else {
                return Err(ToolOutput::err("faltou args.app (ex.: \"x\", \"whatsapp\" ou o nome do pacote)"));
            };
            let pkg = pacote(app);
            Plano::novo(
                Permissao::Controle,
                format!("abrir {app} no celular"),
                Acao::Shell(format!("monkey -p {} -c android.intent.category.LAUNCHER 1", aspas(&pkg))),
            )
        }
        "compartilhar" | "postar" => {
            let Some(texto) = call.text("texto") else { return Err(ToolOutput::err("faltou args.texto")) };
            let app = call.text("app");
            let alvo = app.map(pacote).map(|p| format!(" -p {}", aspas(&p))).unwrap_or_default();
            Plano::novo(
                Permissao::Controle,
                format!(
                    "abrir {} com o texto \"{}\" pronto (ainda sem publicar)",
                    app.unwrap_or("o compartilhar"),
                    texto.chars().take(200).collect::<String>()
                ),
                Acao::Compartilhar(format!(
                    "am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT {}{alvo}",
                    aspas(texto)
                )),
            )
        }
        "link" | "url" => match call.text("url") {
            Some(u) => Plano::novo(
                Permissao::Controle,
                format!("abrir {u} no celular"),
                Acao::Shell(format!("am start -a android.intent.action.VIEW -d {}", aspas(u))),
            ),
            None => return Err(ToolOutput::err("faltou args.url")),
        },
        outra => {
            return Err(ToolOutput::err(format!(
                "acao \"{outra}\" não existe. Use: tela, tocar, segurar, rolar, deslizar, digitar, tecla, print, \
abrir, apps, compartilhar ou link."
            )));
        }
    };
    Ok(p)
}

/// Digita um texto. Com acento (o `input text` do Android não aceita), usa a área de
/// transferência do Termux:API e cola.
async fn digitar(texto: &str) -> ToolOutput {
    if texto.is_ascii() {
        return adb_shell(&format!("input text {}", aspas(&escapar_input(texto))), 30).await;
    }
    if tem("termux-clipboard-set") {
        let r = rodar("termux-clipboard-set", &[texto.to_string()], 15).await;
        if r.ok {
            let colou = adb_shell("input keyevent 279", 10).await;
            return ToolOutput { ok: colou.ok, text: format!("Colei o texto (com acentos). {}", colou.text) };
        }
    }
    let r = adb_shell(&format!("input text {}", aspas(&escapar_input(&sem_acentos(texto)))), 30).await;
    ToolOutput { ok: r.ok, text: format!("Digitei sem acentos ({SEM_API} para acentos). {}", r.text) }
}

/// Faz o que foi planejado (o dono já autorizou, se precisava).
pub async fn executar(plano: Plano, pasta: &Path, n: usize) -> ToolOutput {
    let r = match plano.acao {
        Acao::Tela => match elementos().await {
            Ok(els) => {
                let (w, h) = tamanho().await;
                ToolOutput::ok(format!(
                    "Tela {w}x{h}, {} elementos:\n{}\nUse celular {{acao: tocar, texto}} com o texto de um elemento, ou {{x, y}}.",
                    els.len(),
                    resumo(&els, 90)
                ))
            }
            Err(e) => ToolOutput::err(format!("não consegui ler a tela: {e}")),
        },
        Acao::Apps => adb_shell("pm list packages -3", 20).await,
        Acao::Print => {
            if !tem("adb") {
                return ToolOutput::err(SEM_ADB);
            }
            let arq = pasta.join(format!("celular-{n}.png"));
            match tokio::process::Command::new("adb").args(["exec-out", "screencap", "-p"]).output().await {
                Ok(o) if o.status.success() && !o.stdout.is_empty() => match std::fs::write(&arq, &o.stdout) {
                    Ok(()) => ToolOutput::ok(format!(
                        "Print salvo em {}. (Você não vê a imagem; para saber o que há na tela use acao tela.)",
                        arq.display()
                    )),
                    Err(e) => ToolOutput::err(format!("não consegui gravar o print: {e}")),
                },
                Ok(o) => ToolOutput::err(String::from_utf8_lossy(&o.stderr).to_string()),
                Err(e) => ToolOutput::err(format!("adb: {e}")),
            }
        }
        Acao::Shell(cmd) => adb_shell(&cmd, 20).await,
        Acao::Rolar { gesto, vezes, direcao } => {
            let mut ultimo = ToolOutput::ok("");
            for _ in 0..vezes {
                ultimo = adb_shell(&gesto, 15).await;
                if !ultimo.ok {
                    break;
                }
            }
            ToolOutput { ok: ultimo.ok, text: format!("Rolei para {direcao} {vezes} vez(es). {}", ultimo.text) }
        }
        Acao::Digitar(t) => digitar(&t).await,
        Acao::Compartilhar(cmd) => {
            let r = adb_shell(&cmd, 20).await;
            ToolOutput {
                ok: r.ok,
                text: format!(
                    "Abri o app com o texto pronto. Agora leia a tela (acao tela) e toque no botão de publicar/enviar \
(o dono confirma). {}",
                    r.text
                ),
            }
        }
        Acao::Ligar(numero) => {
            if tem("termux-telephony-call") {
                rodar("termux-telephony-call", &[numero], 20).await
            } else {
                adb_shell(&format!("am start -a android.intent.action.CALL -d tel:{numero}"), 20).await
            }
        }
        Acao::Sms { numero, texto } => {
            if tem("termux-sms-send") {
                rodar("termux-sms-send", &[s("-n"), numero, texto], 30).await
            } else {
                let r = adb_shell(
                    &format!(
                        "am start -a android.intent.action.SENDTO -d sms:{numero} --es sms_body {}",
                        aspas(&texto)
                    ),
                    20,
                )
                .await;
                ToolOutput {
                    ok: r.ok,
                    text: format!(
                        "Abri o SMS pronto (sem o Termux:API não dá para enviar direto): toque em enviar. {}",
                        r.text
                    ),
                }
            }
        }
        Acao::Falar(t) => {
            if !tem("termux-tts-speak") {
                return ToolOutput::err(format!("{SEM_API} para o celular falar"));
            }
            rodar("termux-tts-speak", &[t], 120).await
        }
        Acao::Ouvir => {
            if !tem("termux-speech-to-text") {
                return ToolOutput::err(format!("{SEM_API} para ouvir pelo microfone"));
            }
            rodar("termux-speech-to-text", &[], 90).await
        }
        Acao::Notificar { titulo, texto } => {
            if !tem("termux-notification") {
                return ToolOutput::err(format!("{SEM_API} para notificar"));
            }
            rodar("termux-notification", &[s("-t"), titulo, s("-c"), texto], 15).await
        }
    };
    if plano.nota.is_empty() { r } else { ToolOutput { ok: r.ok, text: format!("{} {}", plano.nota, r.text) } }
}

/// Se vale a pena contar aos corvos que há um celular para usar.
pub fn disponivel() -> bool {
    tem("adb") || tem("termux-sms-send") || tem("termux-telephony-call")
}

/// Manual das ferramentas do celular, para o prompt.
pub fn manual() -> &'static str {
    "- celular {acao, ...}: usa a tela do celular Android como uma pessoa. Ações: `tela` lê o que está na tela (cada \
botão e texto com a posição) — faça isso antes de tocar; `tocar {texto}` toca no botão com esse texto (ou {x, y}); \
`segurar {texto|x,y, ms?}`; `rolar {direcao: baixo|cima|esquerda|direita, quantidade?}`; `deslizar {x, y, x2, y2}`; \
`digitar {texto}` no campo em foco (toque no campo antes); `tecla {tecla: voltar|inicio|enter|apagar|recentes|\
volume_mais|volume_menos|energia}`; `abrir {app: x|whatsapp|instagram|telegram|gmail|chrome|youtube ou o pacote}`; \
`compartilhar {texto, app?}` abre o app com o texto pronto (ex.: um post no X); `link {url}`; `print` salva uma \
imagem; `apps` lista os apps. Tocar em publicar, enviar, pagar ou apagar sempre pede a confirmação do dono.\n\
- ligar {numero}: faz uma ligação normal (o dono confirma).\n\
- sms {numero, texto}: manda um SMS (o dono confirma).\n\
- falar {texto}: o celular fala em voz alta.\n\
- ouvir {}: escuta o dono pelo microfone e devolve o texto.\n\
- notificar {titulo, texto}: mostra uma notificação no celular.\n"
}

/// O que está disponível neste aparelho, para o `dotcrow celular`.
pub async fn diagnostico() -> Vec<(String, bool, String)> {
    let mut out = Vec::new();
    let adb = tem("adb");
    let mut aparelhos = String::new();
    if adb {
        let r = rodar("adb", &[s("devices")], 10).await;
        aparelhos = r
            .text
            .lines()
            .skip(1)
            .filter(|l| l.contains("\tdevice"))
            .map(|l| l.split('\t').next().unwrap_or("").to_string())
            .collect::<Vec<_>>()
            .join(", ");
    }
    out.push((
        "ADB: tela, tocar, rolar, digitar, botões, print, apps".into(),
        adb && !aparelhos.is_empty(),
        if !adb {
            "instale: pkg install android-tools (Termux) ou o platform-tools (PC)".into()
        } else if aparelhos.is_empty() {
            "nenhum aparelho conectado (veja os passos abaixo)".into()
        } else {
            format!("conectado: {aparelhos}")
        },
    ));
    for (prog, uso) in [
        ("termux-telephony-call", "ligar"),
        ("termux-sms-send", "mandar SMS"),
        ("termux-tts-speak", "falar"),
        ("termux-speech-to-text", "ouvir"),
        ("termux-microphone-record", "gravar áudio (dotcrow audio)"),
        ("termux-clipboard-set", "digitar com acento"),
        ("termux-notification", "notificar"),
    ] {
        let ok = tem(prog);
        out.push((format!("Termux:API · {uso}"), ok, if ok { prog.to_string() } else { SEM_API.to_string() }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?><hierarchy rotation="0"><node index="0" text="" resource-id="" class="android.widget.FrameLayout" content-desc="" clickable="false" bounds="[0,0][1080,2340]"><node index="1" text="Postar" resource-id="com.twitter.android:id/button_tweet" class="android.widget.Button" content-desc="" clickable="true" bounds="[800,100][1000,180]" /><node index="2" text="" resource-id="" class="android.widget.ImageButton" content-desc="Voltar" clickable="true" bounds="[0,100][120,220]" /><node index="3" text="O que está acontecendo?" resource-id="com.twitter.android:id/tweet_text" class="android.widget.EditText" content-desc="" clickable="true" bounds="[40,300][1040,600]" /></node></hierarchy>"#;

    #[test]
    fn le_a_tela_e_acha_botoes() {
        let els = ler_tela(XML);
        assert_eq!(els.len(), 3);
        let postar = achar(&els, "postar").unwrap_or_else(|| panic!("achar Postar"));
        assert_eq!((postar.x, postar.y), (900, 140));
        assert_eq!(achar(&els, "voltar").map(|e| (e.x, e.y)), Some((60, 160)));
        assert_eq!(achar(&els, "button_tweet").map(|e| e.texto.as_str()), Some("Postar"));
        assert_eq!(achar(&els, "acontecendo").map(|e| e.y), Some(450));
        assert!(achar(&els, "inexistente").is_none());
        assert!(resumo(&els, 10).contains("\"Postar\" em 900,140 [tocável]"));
        assert_eq!(no_ponto(&els, 850, 120).map(|e| e.texto.as_str()), Some("Postar"));
        assert!(no_ponto(&els, 5000, 5000).is_none());
    }

    #[test]
    fn botoes_sem_volta_pedem_confirmacao() {
        let els = ler_tela(XML);
        let postar = achar(&els, "postar").unwrap_or_else(|| panic!("Postar"));
        let voltar = achar(&els, "voltar").unwrap_or_else(|| panic!("Voltar"));
        assert!(sem_volta(postar));
        assert!(!sem_volta(voltar));
        let enviar = Elemento {
            texto: String::new(),
            desc: "Enviar".into(),
            id: String::new(),
            clicavel: true,
            limites: [0, 0, 10, 10],
            x: 5,
            y: 5,
        };
        assert!(sem_volta(&enviar));
        let campo = Elemento { desc: "Mensagem".into(), ..enviar.clone() };
        assert!(!sem_volta(&campo));
    }

    #[tokio::test]
    async fn ligar_e_sms_sao_irreversiveis() {
        let call = |tool: &str, args: serde_json::Value| ToolCall { n: 1, tool: tool.into(), args, why: String::new() };
        let p = planejar(&call("ligar", serde_json::json!({"numero": "+55 (98) 9 9999-0000"})))
            .await
            .unwrap_or_else(|e| panic!("{}", e.text));
        assert_eq!(p.permissao, Permissao::Irreversivel);
        assert_eq!(p.acao, Acao::Ligar("+5598999990000".into()));
        let p = planejar(&call("sms", serde_json::json!({"numero": "190", "texto": "oi"})))
            .await
            .unwrap_or_else(|e| panic!("{}", e.text));
        assert_eq!(p.permissao, Permissao::Irreversivel);
        let p = planejar(&call("celular", serde_json::json!({"acao": "tecla", "tecla": "voltar"})))
            .await
            .unwrap_or_else(|e| panic!("{}", e.text));
        assert_eq!((p.permissao, p.acao), (Permissao::Controle, Acao::Shell("input keyevent 4".into())));
        assert!(planejar(&call("sms", serde_json::json!({"numero": "190"}))).await.is_err());
        let p = planejar(&call("celular", serde_json::json!({"acao": "compartilhar", "texto": "olá", "app": "x"})))
            .await
            .unwrap_or_else(|e| panic!("{}", e.text));
        assert_eq!(p.permissao, Permissao::Controle);
        assert!(matches!(p.acao, Acao::Compartilhar(ref c) if c.contains("com.twitter.android")));
    }

    #[test]
    fn escapa_texto_para_o_shell() {
        assert_eq!(escapar_input("oi mundo & cia"), "oi%smundo%s\\&%scia");
        assert_eq!(aspas("it's"), "'it'\\''s'");
        assert_eq!(sem_acentos("ação é"), "acao e");
    }

    #[test]
    fn nomes_de_apps_e_teclas() {
        assert_eq!(pacote("X"), "com.twitter.android");
        assert_eq!(pacote("com.exemplo.app"), "com.exemplo.app");
        assert_eq!(tecla("voltar").as_deref(), Some("4"));
        assert_eq!(tecla("Enter").as_deref(), Some("66"));
        assert_eq!(tecla("123").as_deref(), Some("123"));
        assert!(tecla("nada").is_none());
        assert_eq!(so_numero("+55 (98) 99198-0000"), "+5598991980000");
    }
}
