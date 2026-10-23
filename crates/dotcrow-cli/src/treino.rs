//! Modo treinamento: gravar, **com consentimento**, como uma pessoa faz tarefas
//! repetitivas (movimentos do mouse, capturas de tela e passos rotulados), para
//! depois virar memória (RAG) do DotCrow.
//!
//! Regra inegociável: a gravação **só começa com o consentimento explícito** de quem
//! vai ser gravado (nome + "autorizo"). Nada de gravação escondida. Quem grava vê que
//! está gravando e pode parar quando quiser. Isso é o que separa treino de vigilância.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::memoria::Memoria;

/// Consentimento de quem está sendo gravado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consentimento {
    /// Nome de quem autoriza (a pessoa gravada).
    pub nome: String,
    /// Tem de ser `true` para a gravação começar.
    pub autorizado: bool,
    /// Pago ou voluntário (só para registro).
    #[serde(default)]
    pub papel: String,
    /// Quando autorizou.
    #[serde(default)]
    pub ts: u64,
}

/// Ficha da sessão (meta.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    /// Id da sessão.
    pub id: String,
    /// Título (que tarefa está sendo treinada).
    pub titulo: String,
    /// Quem autorizou e como.
    pub consentimento: Consentimento,
    /// Início (segundos desde 1970).
    pub inicio: u64,
    /// Fim, quando parar.
    #[serde(default)]
    pub fim: u64,
    /// Nº de amostras de mouse.
    #[serde(default)]
    pub amostras_mouse: u64,
    /// Nº de passos rotulados.
    #[serde(default)]
    pub passos: u64,
    /// Nº de capturas de tela.
    #[serde(default)]
    pub capturas: u64,
    /// Nº de amostras de atividade (janela ativa, navegadores, sistema).
    #[serde(default)]
    pub atividades: u64,
}

/// Uma sessão de treino gravando no disco.
pub struct Sessao {
    dir: PathBuf,
    meta: Meta,
}

fn agora() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Pasta onde ficam as sessões de treino.
pub fn pasta_treino(dir: &Path) -> PathBuf {
    dir.join("treino")
}

impl Sessao {
    /// Começa uma sessão **se houver consentimento**. Sem `autorizado = true`, recusa.
    pub fn iniciar(dir: &Path, titulo: &str, mut consentimento: Consentimento) -> Result<Self, String> {
        if !consentimento.autorizado || consentimento.nome.trim().is_empty() {
            return Err("a gravação só começa com o consentimento explícito da pessoa (nome + autorizo)".into());
        }
        consentimento.ts = agora();
        let id = format!("{:x}-{:04x}", agora(), (agora() as u16).wrapping_mul(40503));
        let pasta = pasta_treino(dir).join(&id);
        std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
        let meta = Meta {
            id,
            titulo: titulo.trim().to_string(),
            consentimento,
            inicio: agora(),
            fim: 0,
            amostras_mouse: 0,
            passos: 0,
            capturas: 0,
            atividades: 0,
        };
        let s = Self { dir: pasta, meta };
        s.salvar_meta().map_err(|e| e.to_string())?;
        Ok(s)
    }

    /// Id da sessão.
    pub fn id(&self) -> &str {
        &self.meta.id
    }

    /// A ficha atual da sessão.
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    /// A pasta da sessão.
    #[allow(dead_code)]
    pub fn pasta(&self) -> &Path {
        &self.dir
    }

    fn salvar_meta(&self) -> std::io::Result<()> {
        std::fs::write(self.dir.join("meta.json"), serde_json::to_vec_pretty(&self.meta).unwrap_or_default())
    }

    fn append(&self, arquivo: &str, linha: &str) -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(self.dir.join(arquivo))?;
        writeln!(f, "{linha}")
    }

    /// Guarda uma posição do mouse (ts x y). Chamado em intervalos pelo painel.
    pub fn amostrar_mouse(&mut self, x: i64, y: i64) {
        if self.append("mouse.log", &format!("{} {x} {y}", agora())).is_ok() {
            self.meta.amostras_mouse += 1;
        }
    }

    /// Registra um passo rotulado pela pessoa ("abri o sistema", "dei baixa"...).
    pub fn registrar_passo(&mut self, rotulo: &str) {
        let r = rotulo.replace(['\n', '\r'], " ");
        if self.append("passos.log", &format!("{}\t{r}", agora())).is_ok() {
            self.meta.passos += 1;
            let _ = self.salvar_meta();
        }
    }

    /// Caminho da próxima captura de tela (o painel tira a foto e conta aqui).
    pub fn nova_captura(&mut self) -> PathBuf {
        self.meta.capturas += 1;
        let _ = self.salvar_meta();
        self.dir.join(format!("tela-{:03}.png", self.meta.capturas))
    }

    /// Registra o que estava em foco (janela ativa, navegador ou outro programa).
    /// `classe` é a classe da janela (ex.: "google-chrome") e `titulo`, o título.
    pub fn registrar_atividade(&mut self, classe: &str, titulo: &str) {
        let classe = classe.replace(['\n', '\r', '\t'], " ");
        let titulo = titulo.replace(['\n', '\r', '\t'], " ");
        if self.append("atividade.log", &format!("{}\t{classe}\t{titulo}", agora())).is_ok() {
            self.meta.atividades += 1;
            if self.meta.atividades.is_multiple_of(10) {
                let _ = self.salvar_meta();
            }
        }
    }

    /// Encerra a sessão.
    pub fn parar(&mut self) {
        self.meta.fim = agora();
        let _ = self.salvar_meta();
    }

    /// Transforma a sessão em memória (RAG): um resumo do que foi treinado + os passos.
    /// Devolve o texto guardado, ou `None` se não havia nada útil.
    pub fn virar_memoria(&self, memoria: &Memoria) -> std::io::Result<Option<String>> {
        let passos = std::fs::read_to_string(self.dir.join("passos.log")).unwrap_or_default();
        let lista: Vec<String> = passos
            .lines()
            .filter_map(|l| l.split_once('\t').map(|(_, r)| r.trim().to_string()))
            .filter(|r| !r.is_empty())
            .collect();
        let dur = self.meta.fim.saturating_sub(self.meta.inicio);
        let mut texto = format!(
            "Treino \"{}\" (autorizado por {}). Duração {}s, {} passos, {} capturas, {} amostras de mouse.",
            self.meta.titulo,
            self.meta.consentimento.nome,
            dur,
            lista.len(),
            self.meta.capturas,
            self.meta.amostras_mouse
        );
        if !lista.is_empty() {
            texto.push_str("\nPassos: ");
            texto.push_str(&lista.join(" → "));
        }
        let apps = self.resumo_atividade();
        if !apps.is_empty() {
            texto.push_str("\nApps e sites usados: ");
            texto.push_str(&apps.join("; "));
        }
        memoria.guardar(&texto, &format!("treino: {}", self.meta.titulo))
    }

    /// Lista curta e sem repetições dos programas/sites que estiveram em foco no treino.
    fn resumo_atividade(&self) -> Vec<String> {
        let log = std::fs::read_to_string(self.dir.join("atividade.log")).unwrap_or_default();
        let mut vistos = Vec::new();
        for linha in log.lines() {
            let mut campos = linha.splitn(3, '\t');
            let (_ts, classe, titulo) = (campos.next(), campos.next().unwrap_or(""), campos.next().unwrap_or(""));
            let item = format!("{classe} — {}", titulo.chars().take(60).collect::<String>());
            let item = item.trim_matches([' ', '—']).to_string();
            if !item.is_empty() && !vistos.contains(&item) {
                vistos.push(item);
            }
            if vistos.len() >= 12 {
                break;
            }
        }
        vistos
    }
}

/// Lê a ficha de uma sessão pelo id (para listar/retomar).
pub fn ler_meta(dir: &Path, id: &str) -> Option<Meta> {
    let p = pasta_treino(dir).join(id).join("meta.json");
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

/// Retoma uma sessão já começada (para parar/converter depois de reabrir o painel).
#[allow(dead_code)]
pub fn retomar(dir: &Path, id: &str) -> Option<Sessao> {
    let meta = ler_meta(dir, id)?;
    Some(Sessao { dir: pasta_treino(dir).join(id), meta })
}

/// Lista as sessões (mais novas primeiro).
pub fn listar(dir: &Path) -> Vec<Meta> {
    let base = pasta_treino(dir);
    let mut metas: Vec<Meta> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            ler_meta(dir, &id)
        })
        .collect();
    metas.sort_by_key(|m| std::cmp::Reverse(m.inicio));
    metas
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotcrow_crypto::sym;

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "dotcrow-treino-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn consent(ok: bool) -> Consentimento {
        Consentimento {
            nome: if ok { "Maria".into() } else { String::new() },
            autorizado: ok,
            papel: "voluntária".into(),
            ts: 0,
        }
    }

    #[test]
    fn refuses_without_consent() {
        let d = tmp();
        assert!(Sessao::iniciar(&d, "dar baixa", consent(false)).is_err());
        assert!(
            Sessao::iniciar(
                &d,
                "dar baixa",
                Consentimento { nome: "X".into(), autorizado: false, papel: String::new(), ts: 0 }
            )
            .is_err()
        );
        assert!(listar(&d).is_empty(), "nada foi gravado sem consentimento");
    }

    #[test]
    fn records_and_turns_into_memory() {
        let d = tmp();
        let mut s = Sessao::iniciar(&d, "Dar baixa no carnê", consent(true)).unwrap();
        s.amostrar_mouse(10, 20);
        s.amostrar_mouse(30, 40);
        s.registrar_passo("abri o sistema");
        s.registrar_passo("busquei o cliente");
        s.registrar_passo("dei baixa na parcela");
        s.registrar_atividade("google-chrome", "Sistema da loja - Google Chrome");
        s.registrar_atividade("google-chrome", "Sistema da loja - Google Chrome");
        s.registrar_atividade("gnome-terminal", "bash");
        s.parar();

        let metas = listar(&d);
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].passos, 3);
        assert_eq!(metas[0].amostras_mouse, 2);
        assert_eq!(metas[0].atividades, 3);
        assert!(metas[0].consentimento.autorizado);

        let mem = Memoria::abrir(&d.join("mem"), sym::generate_key().unwrap()).unwrap();
        let guardado = s.virar_memoria(&mem).unwrap();
        assert!(guardado.is_some());
        let hits = <Memoria as dotcrow_agents::Memory>::recall(&mem, "como dar baixa no carnê");
        assert!(hits.iter().any(|h| h.contains("dei baixa na parcela")), "memória deve conter os passos: {hits:?}");
        assert!(hits.iter().any(|h| h.contains("Sistema da loja")), "memória deve citar os apps/sites: {hits:?}");
    }

    #[test]
    fn can_resume_after_reopen() {
        let d = tmp();
        let s = Sessao::iniciar(&d, "tarefa", consent(true)).unwrap();
        let id = s.id().to_string();
        drop(s);
        let mut r = retomar(&d, &id).expect("deveria retomar");
        r.registrar_passo("mais um passo");
        r.parar();
        assert_eq!(ler_meta(&d, &id).unwrap().passos, 1);
    }
}
