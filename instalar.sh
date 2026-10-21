#!/usr/bin/env bash
# Instalador do DotCrow
#
# Faz tudo sozinho: instala o que falta (Rust, compilador C, git), compila
# o projeto e coloca o comando `dotcrow` no seu PATH.
#
# Uso, de dentro da pasta do projeto:
#   ./instalar.sh
#
# Ou direto da internet, sem clonar antes:
#   curl -fsSL https://raw.githubusercontent.com/napoleaomp4/dotcrow/main/instalar.sh | bash
#
# Variáveis opcionais:
#   DOTCROW_BIN  pasta onde o comando é instalado (padrão: ~/.local/bin)
#   DOTCROW_SRC  pasta do código quando o script clona (padrão: ~/dotcrow)

set -euo pipefail

VERSAO_MINIMA="1.88"
REPO="https://github.com/napoleaomp4/dotcrow.git"
DESTINO="${DOTCROW_BIN:-$HOME/.local/bin}"

info() { printf '\033[1;36m▸\033[0m %s\n' "$*"; }
ok()   { printf '\033[1;32m✔\033[0m %s\n' "$*"; }
erro() { printf '\033[1;31m✖\033[0m %s\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# 1. Pacotes do sistema (git, curl e um compilador C para o Rust linkar)
# ---------------------------------------------------------------------------
faltando=()
command -v git  >/dev/null 2>&1 || faltando+=(git)
command -v curl >/dev/null 2>&1 || faltando+=(curl)
command -v cc   >/dev/null 2>&1 || faltando+=(compilador)

if [ ${#faltando[@]} -gt 0 ]; then
  info "Instalando o que falta no sistema: ${faltando[*]} (pode pedir sua senha)"
  SUDO=""
  [ "$(id -u)" -ne 0 ] && SUDO="sudo"
  if [ "$(uname -s)" = "Darwin" ]; then
    # No macOS, o compilador e o git vêm das "Command Line Tools" da Apple.
    xcode-select --install 2>/dev/null || true
    erro "Instale as ferramentas da Apple na janela que abriu e rode este instalador de novo."
  elif command -v apt-get >/dev/null 2>&1; then
    pacotes=("${faltando[@]/compilador/build-essential}")
    $SUDO apt-get update -qq
    $SUDO apt-get install -y -qq "${pacotes[@]}"
  elif command -v dnf >/dev/null 2>&1; then
    pacotes=("${faltando[@]/compilador/gcc}")
    $SUDO dnf install -y "${pacotes[@]}"
  elif command -v pacman >/dev/null 2>&1; then
    pacotes=("${faltando[@]/compilador/base-devel}")
    $SUDO pacman -S --needed --noconfirm "${pacotes[@]}"
  else
    erro "Não sei instalar pacotes neste sistema. Instale manualmente: ${faltando[*]}"
  fi
  ok "Pacotes do sistema prontos"
fi

# ---------------------------------------------------------------------------
# 2. Rust 1.88 ou mais novo (via rustup; o cargo do apt é antigo demais)
# ---------------------------------------------------------------------------
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

versao_rust() { rustc --version 2>/dev/null | awk '{print $2}'; }
versao_ok() {
  [ -n "$1" ] && [ "$(printf '%s\n%s\n' "$VERSAO_MINIMA" "$1" | sort -V | head -n1)" = "$VERSAO_MINIMA" ]
}

atual="$(versao_rust || true)"
if versao_ok "$atual"; then
  ok "Rust $atual encontrado"
elif command -v rustup >/dev/null 2>&1; then
  info "Atualizando o Rust (${atual:-nenhum} → estável mais recente)"
  rustup toolchain install stable --profile minimal
  rustup default stable
else
  info "Instalando o Rust pelo rustup (o oficial)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

if ! versao_ok "$atual"; then
  atual="$(versao_rust || true)"
  versao_ok "$atual" || erro "O Rust ativo é ${atual:-desconhecido}, mas o DotCrow precisa de $VERSAO_MINIMA+. Se você instalou 'cargo' pelo apt, remova com: sudo apt remove cargo rustc"
  ok "Rust $atual pronto"
fi

# ---------------------------------------------------------------------------
# 3. Código-fonte: usa esta pasta se o script estiver no projeto, senão clona
# ---------------------------------------------------------------------------
PASTA=""
if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
  dir_script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  [ -f "$dir_script/crates/dotcrow-cli/Cargo.toml" ] && PASTA="$dir_script"
fi
if [ -z "$PASTA" ]; then
  PASTA="${DOTCROW_SRC:-$HOME/dotcrow}"
  if [ -d "$PASTA/.git" ]; then
    info "Atualizando o código em $PASTA"
    git -C "$PASTA" pull --ff-only --quiet
  else
    info "Baixando o código para $PASTA"
    git clone --depth 1 --quiet "$REPO" "$PASTA"
  fi
fi

# ---------------------------------------------------------------------------
# 4. Compilar e instalar
# ---------------------------------------------------------------------------
info "Compilando (na primeira vez leva uns minutos)"
cargo build --release --locked --manifest-path "$PASTA/Cargo.toml"

mkdir -p "$DESTINO"
install -m 755 "$PASTA/target/release/dotcrow" "$DESTINO/dotcrow"
ok "Comando instalado em $DESTINO/dotcrow"

# ---------------------------------------------------------------------------
# 5. Garantir que a pasta está no PATH
# ---------------------------------------------------------------------------
precisa_abrir_terminal=0
case ":$PATH:" in
  *":$DESTINO:"*) ;;
  *)
    linha="export PATH=\"$DESTINO:\$PATH\""
    for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
      if [ -f "$rc" ] && ! grep -qF "$linha" "$rc"; then
        printf '\n# DotCrow\n%s\n' "$linha" >> "$rc"
      fi
    done
    [ -f "$HOME/.bashrc" ] || printf '# DotCrow\n%s\n' "$linha" > "$HOME/.bashrc"
    precisa_abrir_terminal=1
    ;;
esac

# ---------------------------------------------------------------------------
# 6. Ícone no menu de aplicativos (Linux)
# ---------------------------------------------------------------------------
# O atalho aponta para o caminho exato do comando instalado agora, para não
# abrir outro dotcrow (mais antigo) que esteja no PATH.
icone_criado=0
if [ "$(uname -s)" = "Linux" ]; then
  apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  icones="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/512x512/apps"
  mkdir -p "$apps" "$icones"
  install -m 644 "$PASTA/assets/brand/png/dotcrow-app-icon-512.png" "$icones/dotcrow.png"
  sed -e "s|^Exec=.*|Exec=\"$DESTINO/dotcrow\"|" \
      -e "s|^Icon=.*|Icon=$icones/dotcrow.png|" \
      -e '/^#/d' \
      "$PASTA/packaging/linux/dotcrow.desktop" > "$apps/dotcrow.desktop"
  chmod 644 "$apps/dotcrow.desktop"
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" >/dev/null 2>&1 || true
  icone_criado=1
  ok "Ícone do DotCrow criado no menu de aplicativos"
fi

echo
ok "DotCrow $("$DESTINO/dotcrow" --version | awk '{print $2}') instalado!"
echo
if [ "$icone_criado" -eq 1 ]; then
  echo "  O jeito mais fácil: abra o menu de aplicativos e clique em DotCrow."
  echo
fi
if [ "$precisa_abrir_terminal" -eq 1 ]; then
  echo "  Feche e abra o terminal (ou rode: source ~/.bashrc). Depois experimente:"
else
  echo "  Experimente:"
fi
echo
echo "    dotcrow painel           # painel visual: chaves, tarefas e acompanhamento ao vivo"
echo "    dotcrow painel --rede    # o mesmo painel no celular (mesmo Wi-Fi, com QR code)"
echo "    dotcrow testar           # testa suas chaves de API"
echo "    dotcrow --help           # todos os comandos"
echo
