//! Mãos: o que deixa um corvo **agir** no computador, e não só descrever passos.
//!
//! O orquestrador não sabe rodar comandos nem mexer em arquivos: ele conversa com
//! a LLM, lê o pedido de ação que ela devolve (um bloco JSON) e entrega para uma
//! implementação de [`Hands`]. Quem implementa decide o que pode rodar direto e o
//! que precisa da confirmação do dono (a CLI faz isso no painel).

use serde_json::Value;

/// Um pedido de ação feito pela LLM.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// Número da ação dentro da tarefa, começando em 1.
    pub n: usize,
    /// Nome da ferramenta (`terminal`, `escrever_arquivo`, `web`...).
    pub tool: String,
    /// Argumentos, como a LLM mandou.
    pub args: Value,
    /// Por que o corvo quer fazer isso (o "pensamento"), para mostrar ao dono.
    pub why: String,
}

impl ToolCall {
    /// Argumento de texto, se existir.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.args.get(key).and_then(Value::as_str)
    }

    /// Argumento numérico, se existir.
    pub fn number(&self, key: &str) -> Option<i64> {
        self.args.get(key).and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())))
    }
}

/// O que voltou de uma ação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    /// Se deu certo.
    pub ok: bool,
    /// Saída para a LLM e para o dono (já limitada em tamanho).
    pub text: String,
}

impl ToolOutput {
    /// Deu certo.
    pub fn ok(text: impl Into<String>) -> Self {
        Self { ok: true, text: text.into() }
    }

    /// Deu errado.
    pub fn err(text: impl Into<String>) -> Self {
        Self { ok: false, text: text.into() }
    }
}

/// Quem executa as ações no computador.
#[async_trait::async_trait]
pub trait Hands: Send + Sync {
    /// Manual das ferramentas, em texto, para o prompt do corvo.
    fn manual(&self) -> String;

    /// Título curto e legível da ação, para o painel (ex.: `terminal: ls -la`).
    fn title(&self, call: &ToolCall) -> String;

    /// Executa a ação (pedindo confirmação ao dono quando a política mandar).
    async fn run(&self, call: &ToolCall) -> ToolOutput;
}

/// Lê o primeiro bloco JSON da resposta da LLM: `{"ferramenta": "...", "args": {...}, "pensamento": "..."}`.
///
/// Aceita texto ou cercas de código em volta. Devolve `None` se não houver um pedido de ação.
pub fn parse_call(text: &str) -> Option<(String, Value, String)> {
    let mut from = 0;
    while let Some(i) = text[from..].find('{') {
        let start = from + i;
        let mut it = serde_json::Deserializer::from_str(&text[start..]).into_iter::<Value>();
        if let Some(Ok(Value::Object(obj))) = it.next() {
            let tool = obj.get("ferramenta").or_else(|| obj.get("tool")).and_then(Value::as_str);
            if let Some(tool) = tool {
                let args = obj.get("args").cloned().unwrap_or(Value::Object(Default::default()));
                let why = obj.get("pensamento").or_else(|| obj.get("why")).and_then(Value::as_str).unwrap_or("");
                return Some((tool.trim().to_lowercase(), args, why.trim().to_string()));
            }
        }
        from = start + 1;
    }
    None
}

/// Corta `text` em `max` caracteres, avisando quanto ficou de fora.
pub fn clip(text: &str, max: usize) -> String {
    let total = text.chars().count();
    if total <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}\n[... cortado: mais {} caracteres]", total - max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_call_inside_fences_and_text() {
        let t = "Vou listar.\n```json\n{\"pensamento\": \"ver a pasta\", \"ferramenta\": \"Terminal\", \"args\": {\"comando\": \"ls\"}}\n```";
        let (tool, args, why) = parse_call(t).unwrap();
        assert_eq!(tool, "terminal");
        assert_eq!(args["comando"], "ls");
        assert_eq!(why, "ver a pasta");
    }

    #[test]
    fn skips_json_that_is_not_a_call() {
        let t = "exemplo {\"a\": 1} e depois {\"ferramenta\": \"concluir\", \"args\": {\"texto\": \"pronto\"}}";
        assert_eq!(parse_call(t).unwrap().0, "concluir");
        assert!(parse_call("sem json nenhum").is_none());
        assert!(parse_call("{quebrado").is_none());
    }

    #[test]
    fn clips_long_text() {
        assert_eq!(clip("abc", 5), "abc");
        assert!(clip("abcdef", 3).starts_with("abc\n[... cortado: mais 3"));
    }

    #[test]
    fn numbers_accept_text() {
        let c =
            ToolCall { n: 1, tool: "mouse".into(), args: serde_json::json!({"x": "10", "y": 20}), why: String::new() };
        assert_eq!(c.number("x"), Some(10));
        assert_eq!(c.number("y"), Some(20));
        assert_eq!(c.number("z"), None);
    }
}
