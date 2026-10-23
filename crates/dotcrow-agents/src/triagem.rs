//! Triagem de dados sensíveis antes de qualquer coisa ir para uma LLM.
//!
//! Regra do dono: **nada com API key, token, senha, chave privada, CPF ou CNPJ sai do
//! computador para uma IA**. Antes de cada envio, este módulo varre o texto (o pedido, o
//! contexto e, principalmente, o que os corvos leram de arquivos durante a execução — um
//! `.env`, por exemplo) e troca o que for sensível por um marcador `[REDIGIDO: ...]`.
//!
//! É uma rede de segurança, não um substituto do bom senso: foca no que tem formato
//! reconhecível (chaves conhecidas, blocos de chave privada, atribuições de senha/segredo)
//! e em CPF/CNPJ **validados pelo dígito verificador**, para não apagar número à toa.

use std::sync::LazyLock;

use dotcrow_providers::{CompletionRequest, Role};
use regex::Regex;

/// Varre um pedido inteiro e redige os dados sensíveis das mensagens que vão para a LLM.
/// Devolve quantos trechos foram redigidos. Não mexe no texto do sistema (prompt do corvo).
pub fn redigir_request(req: &mut CompletionRequest) -> usize {
    let mut total = 0;
    for m in &mut req.messages {
        if m.role == Role::System {
            continue;
        }
        let (limpo, n) = redigir(&m.content);
        if n > 0 {
            m.content = limpo;
            total += n;
        }
    }
    total
}

/// Redige os dados sensíveis de um texto. Devolve o texto limpo e quantos trechos saíram.
pub fn redigir(texto: &str) -> (String, usize) {
    let mut n = 0;
    let mut out = texto.to_string();

    // 1. Blocos de chave privada (PEM): tira o bloco inteiro.
    out = CHAVE_PRIVADA
        .replace_all(&out, |_: &regex::Captures| {
            n += 1;
            "[REDIGIDO: chave privada]"
        })
        .into_owned();

    // 2. Atribuições do tipo senha/segredo/token/api_key = "valor": guarda o nome, tira o valor.
    out = ATRIBUICAO_SEGREDO
        .replace_all(&out, |c: &regex::Captures| {
            n += 1;
            format!("{}{}[REDIGIDO]", &c[1], &c[2])
        })
        .into_owned();

    // 3. Chaves e tokens de formato conhecido (OpenAI, Google, AWS, GitHub, Slack, JWT...).
    out = TOKEN_CONHECIDO
        .replace_all(&out, |_: &regex::Captures| {
            n += 1;
            "[REDIGIDO: chave]"
        })
        .into_owned();

    // 4. CPF e CNPJ — só quando o dígito verificador bate (evita apagar número qualquer).
    out = CPF_CNPJ
        .replace_all(&out, |c: &regex::Captures| {
            let m = c.get(0).map(|x| x.as_str()).unwrap_or("");
            let digitos: Vec<u8> = m.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
            match digitos.len() {
                11 if cpf_valido(&digitos) => {
                    n += 1;
                    "[REDIGIDO: cpf]".to_string()
                }
                14 if cnpj_valido(&digitos) => {
                    n += 1;
                    "[REDIGIDO: cnpj]".to_string()
                }
                _ => m.to_string(),
            }
        })
        .into_owned();

    (out, n)
}

#[allow(clippy::unwrap_used)]
static CHAVE_PRIVADA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----").unwrap()
});

#[allow(clippy::unwrap_used)]
static ATRIBUICAO_SEGREDO: LazyLock<Regex> = LazyLock::new(|| {
    // Grupo 1: a chave (senha, token, api_key...). Grupo 2: o operador (= ou :) com aspas.
    // O valor (6+ caracteres, sem espaço) é o que some.
    Regex::new(
        r#"(?i)([A-Za-z0-9_-]*(?:pass(?:word)?|senha|secret|segredo|token|api[_-]?key|apikey|client[_-]?secret|access[_-]?key|secret[_-]?key|auth(?:orization)?|bearer))(\s*[:=]\s*["']?)[^\s"']{6,}"#,
    )
    .unwrap()
});

#[allow(clippy::unwrap_used)]
static TOKEN_CONHECIDO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(sk-[A-Za-z0-9_-]{16,}",        // OpenAI / Anthropic (sk-, sk-ant-, sk-proj-)
        r"|AIza[0-9A-Za-z_-]{20,}",       // Google API
        r"|AKIA[0-9A-Z]{16}",             // AWS access key id
        r"|gh[pousr]_[A-Za-z0-9]{20,}",   // GitHub tokens
        r"|xox[baprs]-[A-Za-z0-9-]{10,}", // Slack
        r"|eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", // JWT
        r")",
    ))
    .unwrap()
});

#[allow(clippy::unwrap_used)]
static CPF_CNPJ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{2}\.?\d{3}\.?\d{3}/?\d{4}-?\d{2}|\d{3}\.?\d{3}\.?\d{3}-?\d{2})\b").unwrap());

/// Valida o dígito verificador de um CPF (11 dígitos). Rejeita sequências repetidas.
fn cpf_valido(d: &[u8]) -> bool {
    if d.len() != 11 || d.iter().all(|&x| x == d[0]) {
        return false;
    }
    let dv = |ate: usize, peso0: usize| -> u8 {
        let soma: usize = (0..ate).map(|i| d[i] as usize * (peso0 - i)).sum();
        let r = (soma * 10) % 11;
        if r == 10 { 0 } else { r as u8 }
    };
    dv(9, 10) == d[9] && dv(10, 11) == d[10]
}

/// Valida o dígito verificador de um CNPJ (14 dígitos). Rejeita sequências repetidas.
fn cnpj_valido(d: &[u8]) -> bool {
    if d.len() != 14 || d.iter().all(|&x| x == d[0]) {
        return false;
    }
    let dv = |ate: usize| -> u8 {
        let pesos: [usize; 13] =
            if ate == 12 { [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2, 0] } else { [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2] };
        let soma: usize = (0..ate).map(|i| d[i] as usize * pesos[i]).sum();
        let r = soma % 11;
        if r < 2 { 0 } else { (11 - r) as u8 }
    };
    dv(12) == d[12] && dv(13) == d[13]
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotcrow_providers::Message;

    #[test]
    fn tira_api_keys_e_tokens() {
        let (out, n) = redigir("minha chave é sk-ant-api03-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123 e pronto");
        assert!(out.contains("[REDIGIDO: chave]"), "{out}");
        assert!(!out.contains("sk-ant"));
        assert_eq!(n, 1);
        assert!(redigir("AIzaSyA1234567890abcdefghijklmnopqrstuv").1 >= 1);
        assert!(redigir("token eyJhbGciOiJIUzI1.eyJzdWIiOiIxMjM.SflKxwRJSMeKKF2").1 >= 1);
    }

    #[test]
    fn tira_senhas_em_atribuicao_mantendo_o_nome() {
        let (out, n) = redigir("DB_PASSWORD=sup3rs3cr3ta99\napi_key: \"minhachavesecreta\"");
        assert!(out.contains("[REDIGIDO]"));
        assert!(!out.contains("sup3rs3cr3ta99"));
        assert!(!out.contains("minhachavesecreta"));
        assert!(out.to_lowercase().contains("password") && out.to_lowercase().contains("api_key"));
        assert_eq!(n, 2);
    }

    #[test]
    fn tira_bloco_de_chave_privada() {
        let pem =
            "antes\n-----BEGIN RSA PRIVATE KEY-----\nMIIEoQ...linha...\nABjunk\n-----END RSA PRIVATE KEY-----\ndepois";
        let (out, n) = redigir(pem);
        assert_eq!(n, 1);
        assert!(out.contains("[REDIGIDO: chave privada]"));
        assert!(out.contains("antes") && out.contains("depois"));
        assert!(!out.contains("MIIEoQ"));
    }

    #[test]
    fn cpf_valido_sai_invalido_fica() {
        // CPF válido conhecido (gerado só para teste).
        let (out, n) = redigir("cliente CPF 529.982.247-25 fechado");
        assert!(out.contains("[REDIGIDO: cpf]"), "{out}");
        assert_eq!(n, 1);
        // Um número de 11 dígitos que não é CPF válido não é tocado.
        let (out2, n2) = redigir("pedido 123.456.789-00 só");
        assert!(out2.contains("123.456.789-00"));
        assert_eq!(n2, 0);
    }

    #[test]
    fn cnpj_valido_sai() {
        let (out, n) = redigir("empresa 11.222.333/0001-81 ok");
        assert!(out.contains("[REDIGIDO: cnpj]"), "{out}");
        assert_eq!(n, 1);
    }

    #[test]
    fn nao_mexe_no_prompt_do_sistema() {
        let mut req = CompletionRequest {
            messages: vec![
                Message { role: Role::System, content: "você é Bico, senha=naomexa123".into() },
                Message { role: Role::User, content: "a chave é sk-abcDEFghiJKLmnoPQRstu1234".into() },
            ],
            max_tokens: 10,
            tier: Default::default(),
        };
        let n = redigir_request(&mut req);
        assert_eq!(n, 1, "só a mensagem do usuário é redigida");
        assert!(req.messages[0].content.contains("naomexa123"), "o prompt do sistema fica intacto");
        assert!(!req.messages[1].content.contains("sk-abc"));
    }

    #[test]
    fn texto_limpo_passa_sem_mudar() {
        let (out, n) = redigir("organize a pasta Downloads por cliente e me mande um resumo");
        assert_eq!(n, 0);
        assert_eq!(out, "organize a pasta Downloads por cliente e me mande um resumo");
    }
}
