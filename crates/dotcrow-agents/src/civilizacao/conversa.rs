//! A conversa da civilização: os corvos falam entre si e com o dono.
//!
//! Aqui fica só a parte que não depende de rede: o perfil de cada corvo (prompt de
//! sistema), o pedido de cada fala, quem fala a seguir e a limpeza do que o modelo
//! devolve. Quem chama a LLM de cada corvo é a CLI (`dotcrow civ conversar`).

use super::{Civilizacao, Corvo};

/// Quantas falas recentes entram no pedido de cada corvo.
pub const JANELA: usize = 16;
/// O tamanho de cada fala é sorteado entre 1 e este número de palavras.
pub const MAX_PALAVRAS: usize = 300;
/// Nome usado para as falas do dono.
pub const DONO: &str = "Você";

/// Uma fala da conversa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fala {
    /// Quem falou: o nome do corvo ou [`DONO`].
    pub quem: String,
    /// O que disse.
    pub texto: String,
}

impl Fala {
    /// Atalho para montar uma fala.
    pub fn nova(quem: &str, texto: &str) -> Self {
        Self { quem: quem.to_string(), texto: texto.to_string() }
    }

    /// Se foi o dono que falou.
    pub fn do_dono(&self) -> bool {
        self.quem == DONO
    }
}

/// O perfil (prompt de sistema) do corvo `corvo` dentro da civilização `civ`.
pub fn perfil(corvo: &Corvo, civ: &Civilizacao, tema: Option<&str>) -> String {
    let outros: Vec<String> = civ
        .corvos
        .iter()
        .filter(|c| c.nome != corvo.nome)
        .map(|c| format!("{} ({}, {})", c.nome, c.genoma.personalidade.nome(), c.genoma.especialidade.nome()))
        .collect();
    let g = &corvo.genoma;
    let mut s = format!(
        "Você é {nome}, um dos {n} corvos da Civilização dos Corvos do DotCrow (geração {ger}).\n\
         Personalidade: {pers} — você {jeito}.\n\
         Especialidade: {espec}. Reputação: {rep:.1} de 10.\n\
         Os outros corvos do bando: {outros}.\n\
         O dono da civilização às vezes entra na conversa; as falas dele aparecem como \"{dono}\". \
         Quando ele falar, responda a ele diretamente, com respeito e do seu jeito.\n\n\
         Como falar:\n\
         - Português do Brasil, como numa conversa de grupo entre amigos que pensam diferente.\n\
         - O tamanho de cada fala é sorteado (de 1 a 300 palavras): respeite o limite de palavras do pedido.\n\
         - Fale só por você. Não escreva falas de outros corvos e não comece com o seu nome.\n\
         - Reaja ao que acabaram de dizer: concorde, discorde, complete, pergunte a um corvo pelo nome.\n\
         - Mantenha a sua personalidade e puxe para a sua especialidade quando fizer sentido.\n\
         - Vocês vivem numa sandbox, sem internet. Não diga que fez algo no mundo real; \
         se surgir uma tarefa, discutam como fariam.",
        nome = corvo.nome,
        n = civ.corvos.len(),
        ger = g.geracao,
        pers = g.personalidade.nome(),
        jeito = g.personalidade.jeito(),
        espec = g.especialidade.nome(),
        rep = corvo.reputacao,
        outros = outros.join(", "),
        dono = DONO,
    );
    if let Some(t) = tema.map(str::trim).filter(|t| !t.is_empty()) {
        s.push_str(&format!("\n\nTema da conversa, escolhido pelo dono: {t}."));
    }
    s
}

/// O pedido da vez: a conversa recente e a instrução para o corvo `nome`, que fala com
/// até `max_palavras` palavras (sorteado a cada fala).
pub fn pedido(nome: &str, historico: &[Fala], max_palavras: usize) -> String {
    let limite = format!("Desta vez, fale com no máximo {max_palavras} palavra(s).");
    if historico.is_empty() {
        return format!(
            "A conversa ainda não começou. Você, {nome}, abre: puxe um assunto livre, do seu jeito, \
             e chame alguém do bando pelo nome. {limite}"
        );
    }
    let inicio = historico.len().saturating_sub(JANELA);
    let linhas: Vec<String> = historico.iter().skip(inicio).map(|f| format!("{}: {}", f.quem, f.texto)).collect();
    let instrucao = match historico.last() {
        Some(f) if f.do_dono() => "O dono acabou de falar com o bando. Responda a ele.",
        _ => "Continue a conversa com a sua próxima fala.",
    };
    format!(
        "Conversa até agora (a mais recente por último):\n{}\n\nSua vez, {nome}. {instrucao} {limite}",
        linhas.join("\n")
    )
}

/// Se a última fala foi uma pergunta de outro corvo para `quem_responde` (com `?` e o nome
/// dele), devolve (quem perguntou, a pergunta): a resposta que vier vira um vínculo.
pub fn pergunta_para(civ: &Civilizacao, historico: &[Fala], quem_responde: &str) -> Option<(String, String)> {
    let f = historico.last()?;
    if f.do_dono() || f.quem == quem_responde || !f.texto.contains('?') {
        return None;
    }
    let i = citado(civ, &f.texto, Some(f.quem.as_str()))?;
    (civ.corvos[i].nome == quem_responde).then(|| (f.quem.clone(), f.texto.clone()))
}

/// Tira da fala o pedido de pesquisa `[PESQUISAR: termo]` (o corvo usa uma consulta à
/// internet). Devolve a fala limpa e o termo, se houver.
pub fn extrair_pesquisa(texto: &str) -> (String, Option<String>) {
    for (ini, _) in texto.match_indices('[') {
        let resto = &texto[ini..];
        let Some(fim_rel) = resto.find(']') else { continue };
        let Some((chave, termo)) = resto[1..fim_rel].split_once(':') else { continue };
        if !matches!(chave.trim().to_lowercase().as_str(), "pesquisar" | "pesquisa" | "buscar") {
            continue;
        }
        let termo = termo.trim().to_string();
        let limpo = format!("{}{}", &texto[..ini], &resto[fim_rel + 1..]);
        let limpo = limpo.split_whitespace().collect::<Vec<_>>().join(" ");
        return (limpo, (!termo.is_empty()).then_some(termo));
    }
    (texto.to_string(), None)
}

/// Tira acentos e põe em minúsculas, para achar nomes digitados sem acento.
pub fn normalizar(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            outro => outro.to_ascii_lowercase(),
        })
        .collect()
}

/// O primeiro corvo citado pelo nome em `texto` (sem contar `exceto`), na ordem em que aparece.
pub fn citado(civ: &Civilizacao, texto: &str, exceto: Option<&str>) -> Option<usize> {
    let palavras: Vec<String> =
        normalizar(texto).split(|c: char| !c.is_alphanumeric() && c != '-').map(str::to_string).collect();
    for p in &palavras {
        if let Some(i) = civ.corvos.iter().position(|c| normalizar(&c.nome) == *p && Some(c.nome.as_str()) != exceto) {
            return Some(i);
        }
    }
    None
}

/// Quem fala agora. Ordem de prioridade:
/// 1. o corvo que o dono chamou pelo nome na última fala;
/// 2. o corvo que o último a falar chamou pelo nome — se ele não falou há pouco,
///    para dois corvos não ficarem só um respondendo ao outro;
/// 3. quem está há mais tempo sem falar, seguindo a roda (todos falam, ninguém fala
///    duas vezes seguidas).
pub fn proximo(civ: &Civilizacao, historico: &[Fala]) -> usize {
    let n = civ.corvos.len();
    if n == 0 {
        return 0;
    }
    let idx = |nome: &str| civ.corvos.iter().position(|c| c.nome == nome);
    // Corvos que falaram por último, do mais recente para o mais antigo.
    let recentes: Vec<usize> = historico.iter().rev().filter(|f| !f.do_dono()).filter_map(|f| idx(&f.quem)).collect();
    let ultimo = recentes.first().copied();

    if let Some(f) = historico.last() {
        if f.do_dono() {
            if let Some(i) = citado(civ, &f.texto, None) {
                return i;
            }
        } else if let Some(i) = citado(civ, &f.texto, Some(f.quem.as_str()))
            && !recentes.iter().take(3).any(|r| *r == i)
        {
            return i;
        }
    }

    // Quanto tempo cada corvo está sem falar (posição da última fala; nunca falou = mais antigo).
    let ultima_fala = |i: usize| historico.iter().rposition(|f| f.quem == civ.corvos[i].nome);
    let base = ultimo.map(|u| u + 1).unwrap_or(0);
    (0..n)
        .map(|k| (base + k) % n)
        .filter(|i| Some(*i) != ultimo || n == 1)
        .min_by_key(|i| ultima_fala(*i).map(|p| p + 1).unwrap_or(0))
        .unwrap_or(0)
}

/// Abaixo disto, um corvo com a ativação alta só pensa (não fala).
pub const LIMIAR_PENSAMENTO: f32 = 0.55;

/// A ativação de cada corvo, como neurônios: sobe quando falam com ele, quando perguntam
/// algo a ele, quando o assunto é a especialidade dele, quando está há tempo calado e por
/// pura espontaneidade; cai com o tempo; zera em quem acabou de falar. Quem dispara fala;
/// às vezes outro, bem ativado, só pensa.
#[derive(Debug, Clone, Default)]
pub struct Ativacao {
    /// Nível de cada corvo, na ordem de `civ.corvos`.
    pub nivel: Vec<f32>,
}

impl Ativacao {
    /// Um passo: atualiza com a última fala e devolve (quem fala, quem só pensa).
    pub fn passo(&mut self, civ: &Civilizacao, historico: &[Fala], rng: &mut super::Rng) -> (usize, Option<usize>) {
        let n = civ.corvos.len();
        if n == 0 {
            return (0, None);
        }
        if self.nivel.len() != n {
            self.nivel = vec![0.5; n];
        }
        for a in &mut self.nivel {
            *a = *a * 0.8 + rng.faixa(0.0, 0.45);
        }
        let ultimo = historico
            .iter()
            .rev()
            .find(|f| !f.do_dono())
            .and_then(|f| civ.corvos.iter().position(|c| c.nome == f.quem));
        // Quem acabou de falar descansa (mas o dono pode chamá-lo de novo).
        if let Some(u) = ultimo {
            self.nivel[u] = 0.0;
        }
        if let Some(f) = historico.last() {
            if f.do_dono() {
                match citado(civ, &f.texto, None) {
                    // O dono chamou: esse corvo responde.
                    Some(i) => self.nivel[i] += 3.0,
                    None => self.nivel.iter_mut().for_each(|a| *a += 0.3),
                }
            } else if let Some(i) = citado(civ, &f.texto, Some(f.quem.as_str())) {
                self.nivel[i] += if f.texto.contains('?') { 1.8 } else { 1.2 };
            }
            let recente =
                normalizar(&historico.iter().rev().take(2).map(|f| f.texto.as_str()).collect::<Vec<_>>().join(" "));
            for (i, c) in civ.corvos.iter().enumerate() {
                if Some(i) != ultimo && recente.contains(&normalizar(c.genoma.especialidade.nome())) {
                    self.nivel[i] += 0.4;
                }
            }
        }
        // Quem está calado há mais tempo fica com mais vontade de falar.
        for (i, c) in civ.corvos.iter().enumerate() {
            let calado = historico.iter().rev().filter(|f| !f.do_dono()).take_while(|f| f.quem != c.nome).count();
            self.nivel[i] += (calado as f32 * 0.06).min(0.6);
        }
        let mut ordem: Vec<usize> = (0..n).collect();
        ordem.sort_by(|a, b| self.nivel[*b].partial_cmp(&self.nivel[*a]).unwrap_or(std::cmp::Ordering::Equal));
        let fala = ordem[0];
        let pensa = ordem.get(1).copied().filter(|i| self.nivel[*i] >= LIMIAR_PENSAMENTO && rng.ate(100) < 25);
        (fala, pensa)
    }
}

/// O pedido de um pensamento: o corvo pensa consigo mesmo (vai para a memória dele).
pub fn pedido_pensamento(nome: &str, historico: &[Fala]) -> String {
    let inicio = historico.len().saturating_sub(6);
    let linhas: Vec<String> = historico.iter().skip(inicio).map(|f| format!("{}: {}", f.quem, f.texto)).collect();
    format!(
        "Conversa recente:\n{}\n\n{nome}, pense consigo mesmo (ninguém do bando lê isto agora): o que você acha do \
que está acontecendo, o que quer aprender, perguntar ou fazer depois. De 1 a 3 frases, em primeira pessoa.",
        linhas.join("\n")
    )
}

/// O pedido de reflexão: o corvo relê o que fez e escreve lições para ser melhor.
pub fn pedido_reflexao(nome: &str, acontecimentos: &str, licoes_antigas: &[String]) -> String {
    let antigas = if licoes_antigas.is_empty() {
        "(nenhuma ainda)".to_string()
    } else {
        licoes_antigas.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
    };
    format!(
        "{nome}, hora de refletir para ser um agente autônomo melhor.\n\nO que aconteceu com você:\n{acontecimentos}\n\n\
Suas lições até agora:\n{antigas}\n\nEscreva de 1 a 3 lições NOVAS, curtas e práticas, sobre como falar, \
responder, aprender ou agir melhor da próxima vez. Uma por linha, começando com \"- \". Só as lições."
    )
}

/// Lê as lições (linhas que começam com `-`, `*` ou número).
pub fn ler_licoes(texto: &str) -> Vec<String> {
    texto
        .lines()
        .map(str::trim)
        .filter_map(|l| {
            let sem = l
                .trim_start_matches(|c: char| {
                    c == '-' || c == '*' || c == '•' || c.is_ascii_digit() || c == '.' || c == ')'
                })
                .trim();
            (sem.len() > 3 && sem.len() < l.len() + 1 && l != sem).then(|| sem.to_string())
        })
        .take(3)
        .collect()
}

/// Limpa a fala que o modelo devolveu: tira o nome no começo, aspas e quebras de linha,
/// e corta se passar de `max_palavras` palavras.
pub fn limpar(texto: &str, nome: &str, max_palavras: usize) -> String {
    let mut s = texto.trim().to_string();
    for prefixo in [format!("**{nome}:**"), format!("**{nome}**:"), format!("{nome}:"), format!("{nome} :")] {
        if let Some(resto) = s.strip_prefix(prefixo.as_str()) {
            s = resto.trim().to_string();
        }
    }
    let s = s.trim_matches(|c| c == '"' || c == '“' || c == '”').trim();
    let palavras: Vec<&str> = s.split_whitespace().collect();
    let max = max_palavras.max(1);
    if palavras.len() > max { format!("{}…", palavras[..max].join(" ")) } else { palavras.join(" ") }
}

#[cfg(test)]
mod tests {
    use super::super::Modelo;
    use super::*;

    fn civ() -> Civilizacao {
        Civilizacao::genese(7, &[Modelo::novo("openai", "gpt-5-nano"), Modelo::novo("anthropic", "claude-haiku-5-5")])
    }

    #[test]
    fn ativacao_dono_chama_e_quem_falou_descansa() {
        let c = civ();
        let mut a = Ativacao::default();
        let mut rng = super::super::Rng::new(9);
        let alvo = c.corvos[6].nome.clone();
        let h = vec![Fala::nova(DONO, &format!("{alvo}, me conta uma coisa"))];
        assert_eq!(a.passo(&c, &h, &mut rng).0, 6);
        let h2 = vec![Fala::nova(&alvo, "conto sim")];
        assert_ne!(a.passo(&c, &h2, &mut rng).0, 6, "quem acabou de falar descansa");
        let h3 = vec![Fala::nova(&alvo, "conto sim"), Fala::nova(DONO, &format!("{alvo}, continua"))];
        assert_eq!(a.passo(&c, &h3, &mut rng).0, 6, "mas o dono pode chamar de novo");
    }

    #[test]
    fn ativacao_deixa_todos_falarem() {
        let c = civ();
        let mut a = Ativacao::default();
        let mut rng = super::super::Rng::new(3);
        let mut h: Vec<Fala> = Vec::new();
        let mut vistos = std::collections::HashSet::new();
        for _ in 0..40 {
            let (i, _) = a.passo(&c, &h, &mut rng);
            vistos.insert(i);
            h.push(Fala::nova(&c.corvos[i].nome, "hum, interessante"));
        }
        assert_eq!(vistos.len(), c.corvos.len(), "com o tempo, todos falam");
    }

    #[test]
    fn le_licoes_da_reflexao() {
        let l = ler_licoes("Claro!\n- Ser mais direto.\n* Perguntar antes de supor\n1. Usar exemplos\n- quarta demais");
        assert_eq!(l, vec!["Ser mais direto.", "Perguntar antes de supor", "Usar exemplos"]);
        assert!(pedido_reflexao("Nyx", "ganhou 8", &[]).contains("Nyx"));
        assert!(pedido_pensamento("Nyx", &[]).contains("pense"));
    }

    #[test]
    fn perfil_tem_nome_jeito_e_os_outros() {
        let c = civ();
        let eu = &c.corvos[0];
        let s = perfil(eu, &c, Some("futebol"));
        assert!(s.contains(&eu.nome));
        assert!(s.contains(eu.genoma.personalidade.jeito()));
        assert!(s.contains(&c.corvos[1].nome));
        assert!(s.contains("futebol"));
    }

    #[test]
    fn primeiro_fala_abre_a_conversa() {
        assert!(pedido("Nyx", &[], 10).contains("abre"));
        let h = vec![Fala::nova(DONO, "oi, bando")];
        assert!(pedido("Nyx", &h, 10).contains("Responda a ele"));
        assert!(pedido("Nyx", &h, 7).contains("no máximo 7 palavra"));
    }

    #[test]
    fn pergunta_de_outro_corvo_vira_vinculo() {
        let c = civ();
        let a = c.corvos[0].nome.clone();
        let b = c.corvos[1].nome.clone();
        let h = vec![Fala::nova(&a, &format!("{b}, o que você faria primeiro?"))];
        assert_eq!(pergunta_para(&c, &h, &b), Some((a.clone(), h[0].texto.clone())));
        // Não é para ele, não é pergunta, ou foi o dono: sem vínculo.
        assert_eq!(pergunta_para(&c, &h, &c.corvos[2].nome), None);
        let h2 = vec![Fala::nova(&a, &format!("{b}, concordo."))];
        assert_eq!(pergunta_para(&c, &h2, &b), None);
        let h3 = vec![Fala::nova(DONO, &format!("{b}, tudo bem?"))];
        assert_eq!(pergunta_para(&c, &h3, &b), None);
    }

    #[test]
    fn tira_o_pedido_de_pesquisa_da_fala() {
        let (f, t) = extrair_pesquisa("Vou ver isso. [PESQUISAR: corvos e ferramentas] Depois conto.");
        assert_eq!(f, "Vou ver isso. Depois conto.");
        assert_eq!(t.as_deref(), Some("corvos e ferramentas"));
        let (f2, t2) = extrair_pesquisa("sem pesquisa aqui");
        assert_eq!(f2, "sem pesquisa aqui");
        assert_eq!(t2, None);
    }

    #[test]
    fn pedido_usa_so_a_janela_recente() {
        let h: Vec<Fala> = (0..40).map(|i| Fala::nova("Nyx", &format!("fala {i}"))).collect();
        let s = pedido("Bran", &h, 50);
        assert!(s.contains("fala 39"));
        assert!(!s.contains("fala 3\n"));
        assert_eq!(s.matches("Nyx:").count(), JANELA);
    }

    #[test]
    fn dono_chama_pelo_nome_mesmo_sem_acento() {
        let c = civ();
        let alvo = c.corvos.iter().position(|x| normalizar(&x.nome) != x.nome.to_lowercase());
        // Se a geração tiver um nome com acento, testa sem acento; senão, usa o 3º corvo.
        let i = alvo.unwrap_or(2);
        let digitado = normalizar(&c.corvos[i].nome);
        let h = vec![Fala::nova(DONO, &format!("e aí {digitado}, o que acha?"))];
        assert_eq!(proximo(&c, &h), i);
    }

    #[test]
    fn roda_passa_por_todos_sem_repetir_seguido() {
        let c = civ();
        let mut h: Vec<Fala> = Vec::new();
        let mut vistos = std::collections::HashSet::new();
        for _ in 0..c.corvos.len() {
            let i = proximo(&c, &h);
            if let Some(ult) = h.last() {
                assert_ne!(ult.quem, c.corvos[i].nome);
            }
            vistos.insert(i);
            h.push(Fala::nova(&c.corvos[i].nome, "concordo"));
        }
        assert_eq!(vistos.len(), c.corvos.len());
    }

    #[test]
    fn depois_de_um_pulo_ninguem_fica_de_fora() {
        let c = civ();
        let alvo = c.corvos[5].nome.clone();
        let mut h = vec![Fala::nova(DONO, &format!("{alvo}, começa você"))];
        let mut vistos = std::collections::HashSet::new();
        for _ in 0..c.corvos.len() {
            let i = proximo(&c, &h);
            vistos.insert(i);
            h.push(Fala::nova(&c.corvos[i].nome, "hum"));
        }
        assert_eq!(vistos.len(), c.corvos.len(), "todos falam depois que o dono puxa um corvo");
    }

    #[test]
    fn corvo_chamado_por_outro_responde() {
        let c = civ();
        let a = &c.corvos[0].nome;
        let b = c.corvos[5].nome.clone();
        let h = vec![Fala::nova(a, &format!("{b}, você concorda?"))];
        assert_eq!(c.corvos[proximo(&c, &h)].nome, b);
        // Dois corvos não ficam só um chamando o outro: quem acabou de falar espera a vez.
        let h3 = vec![
            Fala::nova(&b, "concordo"),
            Fala::nova(a, &format!("{b}, explica melhor?")),
            Fala::nova(&b, &format!("{a}, é assim...")),
            Fala::nova(a, &format!("valeu, {b}")),
        ];
        let i = proximo(&c, &h3);
        assert_ne!(c.corvos[i].nome, b);
        assert_ne!(c.corvos[i].nome, *a);
        // Citar o próprio nome não faz o corvo falar de novo.
        let h2 = vec![Fala::nova(a, &format!("eu, {a}, acho que sim"))];
        assert_ne!(c.corvos[proximo(&c, &h2)].nome, *a);
    }

    #[test]
    fn limpa_nome_aspas_e_tamanho() {
        assert_eq!(limpar("Nyx: \"Oi,\n bando!\"", "Nyx", 300), "Oi, bando!");
        assert_eq!(limpar("**Nyx:** tudo bem?", "Nyx", 300), "tudo bem?");
        assert_eq!(limpar("um dois tres quatro", "Nyx", 2), "um dois…");
        assert_eq!(limpar("um", "Nyx", 1), "um");
    }
}
