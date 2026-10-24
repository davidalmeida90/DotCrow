//! Certificados das conexões HTTPS.
//!
//! No Windows, no macOS e no Linux, o `reqwest` confere os certificados com o
//! verificador do próprio sistema. No Android esse verificador precisa da máquina Java
//! de um app Android, que não existe num programa de terminal como o Termux: sem ela, a
//! primeira conexão HTTPS derruba o programa ("Expect rustls-platform-verifier to be
//! initialized"). Lá, o DotCrow confere os certificados com a lista de autoridades da
//! Mozilla embutida no binário.

/// Ajusta um cliente HTTP para conferir certificados do jeito que funciona no sistema
/// em que o DotCrow está rodando. Todo cliente que fala HTTPS passa por aqui.
pub fn preparar(cliente: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    if cfg!(target_os = "android") { com_raizes_embutidas(cliente) } else { cliente }
}

/// Usa só as autoridades da Mozilla embutidas no binário, sem o verificador do sistema.
pub fn com_raizes_embutidas(cliente: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    cliente.tls_certs_only(raizes())
}

/// As autoridades da Mozilla, no formato do `reqwest`.
fn raizes() -> Vec<reqwest::Certificate> {
    webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|c| reqwest::Certificate::from_der(c.as_ref()).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raizes_embutidas_carregam() {
        let n = raizes().len();
        assert!(n > 100, "a lista da Mozilla tem mais de cem autoridades, veio {n}");
        assert_eq!(n, webpki_root_certs::TLS_SERVER_ROOT_CERTS.len(), "nenhuma autoridade pode ficar de fora");
    }

    #[test]
    fn cliente_com_raizes_embutidas_monta() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        assert!(com_raizes_embutidas(reqwest::Client::builder()).build().is_ok());
        assert!(preparar(reqwest::Client::builder()).build().is_ok());
    }
}
