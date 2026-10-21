//! Mostra um erro na tela quando o DotCrow foi aberto pelo ícone.
//!
//! Sem terminal, um erro no `stderr` some sem ninguém ver: a pessoa clica no
//! ícone e nada acontece. Aqui o erro vira uma notificação (Linux) ou uma janela
//! de aviso (Windows e macOS). O texto vai por variável de ambiente, nunca
//! montado dentro do comando, para não dar problema com aspas.

use std::process::{Command, Stdio};

const TITULO: &str = "O DotCrow não abriu";

/// Mostra `erro` na tela. Se nada der certo, não faz nada (o erro já foi para o `stderr`).
pub fn erro_na_tela(erro: &str) {
    let texto = format!("{erro}\n\nPara ver os detalhes, abra um terminal e rode: dotcrow");
    mostrar(&texto);
}

fn roda(cmd: &mut Command) -> bool {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

#[cfg(target_os = "windows")]
fn mostrar(texto: &str) {
    let script = "Add-Type -AssemblyName PresentationFramework; \
        [System.Windows.MessageBox]::Show($env:DOTCROW_AVISO, $env:DOTCROW_TITULO, 'OK', 'Error') | Out-Null";
    let _ = roda(
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", script])
            .env("DOTCROW_AVISO", texto)
            .env("DOTCROW_TITULO", TITULO),
    );
}

#[cfg(target_os = "macos")]
fn mostrar(texto: &str) {
    let script = "display alert (system attribute \"DOTCROW_TITULO\") \
        message (system attribute \"DOTCROW_AVISO\") as critical";
    let _ =
        roda(Command::new("osascript").args(["-e", script]).env("DOTCROW_AVISO", texto).env("DOTCROW_TITULO", TITULO));
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn mostrar(texto: &str) {
    // notify-send vem no Mint, Ubuntu e Fedora; zenity e kdialog ficam de reserva.
    let _ = roda(Command::new("notify-send").args([
        "--app-name=DotCrow",
        "--icon=dotcrow",
        "--urgency=critical",
        TITULO,
        texto,
    ])) || roda(Command::new("zenity").args(["--error", "--no-markup", "--title", TITULO, "--text", texto]))
        || roda(Command::new("kdialog").args(["--title", TITULO, "--error", texto]));
}
