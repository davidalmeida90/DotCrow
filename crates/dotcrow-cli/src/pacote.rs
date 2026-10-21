//! Empacotar um RAG para **exportar e importar** (um arquivo `.dotrag`).
//!
//! Um pacote é um JSON legível: uma ficha (quem fez, licença, preço, Pix do
//! vendedor) e as lembranças (já sem segredos). Serve para a pessoa treinar um RAG
//! e passar para outra, ou, no futuro, vender no marketplace. A cobrança em si fica
//! para o backend do marketplace; aqui só criamos e lemos o arquivo.

use serde::{Deserialize, Serialize};

use crate::memoria::{Lembranca, Memoria, redigir};

/// Versão do formato de pacote.
pub const VERSAO: u32 = 1;

/// A ficha do pacote: o que aparece na "capa" do RAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ficha {
    /// Nome do RAG.
    pub nome: String,
    /// Quem treinou/publicou.
    #[serde(default)]
    pub autor: String,
    /// Para que serve.
    #[serde(default)]
    pub descricao: String,
    /// Licença (ex.: "CC-BY-4.0", "proprietário").
    #[serde(default)]
    pub licenca: String,
    /// Preço em centavos (o marketplace usa; 0 ou ausente = grátis).
    #[serde(default)]
    pub preco_centavos: u64,
    /// Chave Pix do vendedor (o marketplace usa; opcional).
    #[serde(default)]
    pub pix: String,
}

/// Uma lembrança dentro de um pacote (sem id nem data: ganham novos ao importar).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPacote {
    /// O texto.
    pub texto: String,
}

/// Um RAG empacotado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pacote {
    /// Marca do formato.
    #[serde(default = "marca")]
    pub formato: String,
    /// Versão do formato.
    #[serde(default)]
    pub versao: u32,
    /// A ficha.
    pub ficha: Ficha,
    /// As lembranças.
    pub itens: Vec<ItemPacote>,
}

fn marca() -> String {
    "dotcrow-rag".to_string()
}

/// Monta um pacote a partir das lembranças (`None` = todas). Remove segredos de novo,
/// por garantia, e descarta o que ficar vazio.
pub fn exportar(lembrancas: Vec<Lembranca>, ficha: Ficha) -> Pacote {
    let itens = lembrancas
        .into_iter()
        .filter_map(|l| {
            let texto = redigir(&l.texto);
            let t = texto.trim();
            if t.is_empty() || t == "[removido: possível segredo]" { None } else { Some(ItemPacote { texto }) }
        })
        .collect();
    Pacote { formato: marca(), versao: VERSAO, ficha, itens }
}

/// Serializa o pacote em JSON legível (o conteúdo de um arquivo `.dotrag`).
pub fn para_bytes(p: &Pacote) -> Vec<u8> {
    serde_json::to_vec_pretty(p).unwrap_or_default()
}

/// Lê um pacote de um arquivo `.dotrag`. Confere a marca e que há lembranças.
pub fn ler(bytes: &[u8]) -> Result<Pacote, String> {
    let p: Pacote = serde_json::from_slice(bytes).map_err(|e| format!("arquivo inválido: {e}"))?;
    if p.formato != marca() {
        return Err("este arquivo não é um RAG do DotCrow".into());
    }
    if p.itens.is_empty() {
        return Err("o pacote não tem nenhuma lembrança".into());
    }
    Ok(p)
}

/// Importa um pacote para a memória: cada item vira uma lembrança nova, marcada com a
/// origem. Devolve quantas foram guardadas (segredos são descartados).
pub fn importar(memoria: &Memoria, pacote: &Pacote) -> std::io::Result<usize> {
    let fonte = format!("pacote: {}", pacote.ficha.nome.trim());
    let mut n = 0;
    for item in &pacote.itens {
        if memoria.guardar(&item.texto, &fonte)?.is_some() {
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotcrow_crypto::sym;

    fn mem() -> Memoria {
        let base =
            std::env::temp_dir().join(format!("dotcrow-pac-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&base);
        Memoria::abrir(&base, sym::generate_key().unwrap()).unwrap()
    }

    fn ficha() -> Ficha {
        Ficha {
            nome: "Atendimento Loja".into(),
            autor: "Napoleão".into(),
            descricao: "respostas de atendimento".into(),
            licenca: "proprietário".into(),
            preco_centavos: 4990,
            pix: "chave-pix-exemplo".into(),
        }
    }

    #[test]
    fn round_trip_export_import() {
        let origem = mem();
        origem.guardar("O cliente gosta de ser chamado pelo nome.", "treino").unwrap();
        origem.guardar("Horário de atendimento: 9h às 18h.", "treino").unwrap();
        let pacote = exportar(origem.todas(), ficha());
        assert_eq!(pacote.itens.len(), 2);

        let bytes = para_bytes(&pacote);
        let lido = ler(&bytes).unwrap();
        assert_eq!(lido.ficha.nome, "Atendimento Loja");
        assert_eq!(lido.ficha.preco_centavos, 4990);

        let destino = mem();
        let n = importar(&destino, &lido).unwrap();
        assert_eq!(n, 2);
        assert_eq!(destino.quantidade(), 2);
        let hits = <Memoria as dotcrow_agents::Memory>::recall(&destino, "horário de atendimento");
        assert!(hits.iter().any(|h| h.contains("9h às 18h")));
    }

    #[test]
    fn export_strips_secrets() {
        // guardar já tira segredo; forço um item "sujo" direto no pacote para testar o exportar.
        let suja = vec![Lembranca {
            id: "x".into(),
            ts: 0,
            texto: "token ghp_0123456789abcdefABCDEF0123 e nada mais".into(),
            fonte: "t".into(),
        }];
        let p = exportar(suja, ficha());
        assert!(p.itens.iter().all(|i| !i.texto.contains("ghp_0123")));
    }

    #[test]
    fn rejects_foreign_files() {
        assert!(ler(b"{\"foo\":1}").is_err());
        assert!(ler(b"nao e json").is_err());
        let vazio = Pacote { formato: marca(), versao: 1, ficha: ficha(), itens: vec![] };
        assert!(ler(&para_bytes(&vazio)).is_err());
    }
}
