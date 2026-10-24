//! Alinhamento: o Huginn pergunta antes de o bando trabalhar.
//!
//! Duas situações:
//!
//! - **Conhecer o projeto** (a primeira vez numa pasta): o Huginn olha um retrato da
//!   pasta (arquivos, README) e faz poucas perguntas sobre o que é o projeto, o que não
//!   pode ser tocado e como testar. As respostas viram o **perfil do projeto**, que vai
//!   junto em toda tarefa daquela pasta.
//! - **Antes de cada tarefa**: se algo no pedido mudaria o resultado (escopo, onde mexer,
//!   estilo, o que preservar), o Huginn pergunta antes de começar.
//!
//! As perguntas vêm da LLM em JSON; aqui ficam os prompts e a leitura robusta da resposta.

use dotcrow_providers::{CompletionRequest, Message, Role, Router, RouterError, Tier};
use serde::{Deserialize, Serialize};

/// Máximo de perguntas antes de uma tarefa (mais que isso cansa o dono).
pub const MAX_ANTES: usize = 3;
/// Máximo de perguntas na entrevista do projeto.
pub const MAX_PROJETO: usize = 5;

/// Uma pergunta do Huginn, com respostas prováveis para o dono só escolher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pergunta {
    /// O que ele quer saber.
    pub texto: String,
    /// Respostas prováveis (pode ser vazia: resposta livre).
    pub opcoes: Vec<String>,
}

/// Uma pergunta já respondida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resposta {
    /// A pergunta.
    pub pergunta: String,
    /// O que o dono respondeu (vazio = "decida você").
    pub resposta: String,
}

const FORMATO: &str = "Responda SOMENTE com um JSON, sem nada fora dele, neste formato:\n\
{\"perguntas\": [{\"pergunta\": \"texto curto\", \"opcoes\": [\"resposta provável 1\", \"resposta provável 2\"]}]}\n\
Cada pergunta deve ser curta, concreta e fácil de responder no celular. Dê de 2 a 4 opções quando houver respostas \
prováveis; deixe `opcoes` vazio quando a resposta for livre (um nome, um endereço). Escreva no idioma do dono.";

/// Perguntas antes de começar uma tarefa. Lista vazia = o pedido já está claro.
///
/// `contexto` traz o perfil do projeto, as pastas e a conversa até aqui; `pedido` é o
/// que o dono acabou de pedir (com os arquivos anexados, se houver).
pub async fn perguntas_antes(router: &Router, contexto: &str, pedido: &str) -> Result<Vec<Pergunta>, RouterError> {
    let sistema = format!(
        "Você é o Huginn, o estrategista do DotCrow. Antes de o bando começar, decida se precisa perguntar algo ao dono \
para entregar EXATAMENTE o que ele quer. O dono prefere ser consultado: na dúvida sobre algo que mudaria o resultado, \
pergunte. Bons motivos para perguntar: escopo (só isso ou também aquilo?), onde mexer (qual arquivo, qual pasta), \
estilo e tom, o que precisa ser preservado, como ele vai conferir que ficou certo, dados que só ele sabe. \
NÃO pergunte o que dá para descobrir lendo os arquivos ou rodando um comando, nem o que o perfil do projeto ou a \
conversa já responderam. Se o pedido for uma consulta simples (ler, listar, explicar) ou já estiver claro, devolva \
a lista vazia. No máximo {MAX_ANTES} perguntas.\n\n{FORMATO}"
    );
    let usuario = format!("{contexto}\n\n# Pedido do dono agora\n{pedido}");
    let texto = pedir(router, sistema, usuario).await?;
    let mut ps = ler_perguntas(&texto);
    ps.truncate(MAX_ANTES);
    Ok(ps)
}

/// Perguntas para conhecer um projeto novo, a partir de um retrato da pasta.
pub async fn perguntas_do_projeto(router: &Router, retrato: &str) -> Result<Vec<Pergunta>, RouterError> {
    let sistema = format!(
        "Você é o Huginn, o estrategista do DotCrow. O dono abriu o DotCrow numa pasta pela primeira vez. Olhe o retrato \
da pasta e faça as perguntas que mais ajudam o bando a trabalhar alinhado com ele daqui para a frente: o que é o \
projeto e para quem, o que ele quer conseguir agora, o que NÃO pode ser alterado (produção, banco de dados, arquivos \
de configuração), como testar ou ver funcionando, e preferências de estilo (idioma, padrão de código). Não pergunte \
o que o retrato já mostra (a linguagem, por exemplo, se estiver óbvia): use o que viu para fazer perguntas \
específicas deste projeto. Entre 3 e {MAX_PROJETO} perguntas.\n\n{FORMATO}"
    );
    let texto = pedir(router, sistema, format!("# Retrato da pasta\n{retrato}")).await?;
    let mut ps = ler_perguntas(&texto);
    ps.truncate(MAX_PROJETO);
    Ok(ps)
}

/// Escreve (ou atualiza) o perfil do projeto em Markdown curto, a partir do retrato e
/// das respostas do dono. É o que vai junto em todas as tarefas daquela pasta.
pub async fn perfil_do_projeto(
    router: &Router,
    retrato: &str,
    respostas: &[Resposta],
    perfil_antigo: Option<&str>,
) -> Result<String, RouterError> {
    let sistema = "Você é a Pena, a escritora do DotCrow. Escreva o PERFIL DO PROJETO: um resumo em Markdown, com no \
máximo 25 linhas, que os outros corvos vão ler antes de toda tarefa nesta pasta. Seções curtas: **O que é**, \
**Objetivo agora**, **Não mexer**, **Como testar**, **Preferências**. Use só o que o dono respondeu e o que o retrato \
mostra; não invente. Se o dono respondeu \"decida você\" ou deixou em branco, não registre nada sobre isso. Se houver \
um perfil antigo, atualize-o com as respostas novas (as novas valem mais). Responda só com o Markdown."
        .to_string();
    let mut usuario =
        format!("# Retrato da pasta\n{retrato}\n\n# Respostas do dono\n{}", formatar_respostas(respostas));
    if let Some(antigo) = perfil_antigo.filter(|a| !a.trim().is_empty()) {
        usuario.push_str(&format!("\n\n# Perfil antigo\n{antigo}"));
    }
    let texto = pedir(router, sistema, usuario).await?;
    Ok(tirar_cerca(&texto).trim().to_string())
}

/// As respostas do dono no formato que vai para os corvos.
pub fn formatar_respostas(respostas: &[Resposta]) -> String {
    respostas
        .iter()
        .map(|r| {
            let resp = if r.resposta.trim().is_empty() { "(decida você)" } else { r.resposta.trim() };
            format!("- {}\n  → {resp}", r.pergunta.trim())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn pedir(router: &Router, sistema: String, usuario: String) -> Result<String, RouterError> {
    let mut req = CompletionRequest {
        messages: vec![
            Message { role: Role::System, content: sistema },
            Message { role: Role::User, content: usuario },
        ],
        max_tokens: 1500,
        tier: Tier::Standard,
    };
    // Triagem: CPF, chaves e senhas saem antes de qualquer coisa ir para a LLM.
    crate::triagem::redigir_request(&mut req);
    Ok(router.complete(&req).await?.response.text)
}

/// Lê as perguntas da resposta da LLM, tolerando texto em volta, cerca de código e
/// pequenas variações de nome. Se não houver JSON, cai para "uma pergunta por linha
/// terminada em ?". Nunca falha: no pior caso, devolve a lista vazia.
pub fn ler_perguntas(texto: &str) -> Vec<Pergunta> {
    let limpo = tirar_cerca(texto);
    if let (Some(i), Some(f)) = (limpo.find('{'), limpo.rfind('}'))
        && i < f
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&limpo[i..=f])
    {
        let lista = v.get("perguntas").or_else(|| v.get("questions")).and_then(|l| l.as_array());
        if let Some(lista) = lista {
            return lista
                .iter()
                .filter_map(|item| {
                    let texto = item
                        .get("pergunta")
                        .or_else(|| item.get("question"))
                        .or_else(|| item.get("texto"))
                        .and_then(|t| t.as_str())
                        .or_else(|| item.as_str())?
                        .trim()
                        .to_string();
                    if texto.is_empty() {
                        return None;
                    }
                    let opcoes = item
                        .get("opcoes")
                        .or_else(|| item.get("options"))
                        .and_then(|o| o.as_array())
                        .map(|o| {
                            o.iter()
                                .filter_map(|x| x.as_str())
                                .map(|x| x.trim().to_string())
                                .filter(|x| !x.is_empty())
                                .take(4)
                                .collect()
                        })
                        .unwrap_or_default();
                    Some(Pergunta { texto, opcoes })
                })
                .collect();
        }
    }
    // Sem JSON: aproveita linhas que são perguntas.
    limpo
        .lines()
        .map(|l| l.trim().trim_start_matches(|c: char| c.is_ascii_digit() || matches!(c, '.' | ')' | '-' | '*')).trim())
        .filter(|l| l.ends_with('?') && l.chars().count() > 5)
        .map(|l| Pergunta { texto: l.to_string(), opcoes: Vec::new() })
        .collect()
}

/// Tira a cerca ```...``` que algumas LLMs colocam em volta da resposta.
fn tirar_cerca(texto: &str) -> &str {
    let t = texto.trim();
    let Some(resto) = t.strip_prefix("```") else { return t };
    let resto = resto.split_once('\n').map(|(_, r)| r).unwrap_or(resto);
    resto.strip_suffix("```").unwrap_or(resto).trim()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dotcrow_providers::{LlmProvider, mock::MockProvider};

    use super::*;

    #[test]
    fn le_json_com_texto_em_volta() {
        let t = "Claro! ```json\n{\"perguntas\": [{\"pergunta\": \"Só o notas.php ou também o admin?\", \
\"opcoes\": [\"Só o notas.php\", \"Os dois\"]}, {\"pergunta\": \"Qual cor?\", \"opcoes\": []}]}\n```";
        let ps = ler_perguntas(t);
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].texto, "Só o notas.php ou também o admin?");
        assert_eq!(ps[0].opcoes, vec!["Só o notas.php", "Os dois"]);
        assert!(ps[1].opcoes.is_empty());
    }

    #[test]
    fn lista_vazia_quando_esta_claro() {
        assert!(ler_perguntas("{\"perguntas\": []}").is_empty());
        assert!(ler_perguntas("Está tudo claro, pode seguir.").is_empty());
    }

    #[test]
    fn sem_json_usa_linhas_com_interrogacao() {
        let ps = ler_perguntas("1. Para quem é o site?\n2) Pode mexer no banco?\nObrigado.");
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[1].texto, "Pode mexer no banco?");
    }

    #[test]
    fn respostas_em_branco_viram_decida_voce() {
        let r = formatar_respostas(&[
            Resposta { pergunta: "Cor?".into(), resposta: String::new() },
            Resposta { pergunta: "Arquivo?".into(), resposta: "notas.php".into() },
        ]);
        assert!(r.contains("(decida você)"));
        assert!(r.contains("→ notas.php"));
    }

    #[tokio::test]
    async fn no_maximo_tres_antes_da_tarefa() {
        let p = MockProvider::scripted(
            "anthropic",
            &[
                "{\"perguntas\": [{\"pergunta\": \"a?\"}, {\"pergunta\": \"b?\"}, {\"pergunta\": \"c?\"}, {\"pergunta\": \"d?\"}]}",
            ],
        );
        let router = Router::new(vec![p.clone() as Arc<dyn LlmProvider>]);
        let ps = perguntas_antes(&router, "# Perfil\nsite", "mude o título").await.unwrap();
        assert_eq!(ps.len(), MAX_ANTES);
        // O pedido e o contexto chegaram à LLM.
        let req = &p.requests()[0];
        assert!(req.messages[1].content.contains("mude o título"));
        assert!(req.messages[1].content.contains("# Perfil"));
    }
}
