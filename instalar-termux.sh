#!/data/data/com.termux/files/usr/bin/bash
# Instalador do DotCrow no Android, pelo Termux
#
# Uso, dentro do Termux (instale o Termux pelo F-Droid ou pelo GitHub; o da Play Store
# é antigo):
#   curl -fsSL https://raw.githubusercontent.com/napoleaomp4/dotcrow/main/instalar-termux.sh | bash
#
# O que ele faz:
#   1. tenta baixar o dotcrow pronto para celular (arm64) da última versão publicada;
#   2. se não houver, instala o Rust do próprio Termux e compila (leva uns minutos e
#      precisa de uns 2 GB livres);
#   3. coloca o comando `dotcrow` no PATH do Termux.
#
# Variáveis opcionais:
#   DOTCROW_COMPILAR=1  pula o binário pronto e compila do código
#   DOTCROW_SRC         pasta do código quando compila (padrão: ~/dotcrow)

set -euo pipefail

REPO="napoleaomp4/dotcrow"
PREFIX="${PREFIX:-/data/data/com.termux/files/usr}"
DESTINO="$PREFIX/bin/dotcrow"

info() { printf '\033[1;36m▸\033[0m %s\n' "$*"; }
ok()   { printf '\033[1;32m✔\033[0m %s\n' "$*"; }
erro() { printf '\033[1;31m✖\033[0m %s\n' "$*" >&2; exit 1; }

case "$PREFIX" in
  *com.termux*) ;;
  *) erro "Este instalador é para o Termux (Android). No Linux, macOS ou WSL, use o instalar.sh." ;;
esac

arq="$(uname -m)"
[ "$arq" = "aarch64" ] || info "Aviso: este celular é $arq; o binário pronto é só para aarch64 (arm64). Vou compilar."

command -v curl >/dev/null 2>&1 || pkg install -y curl

# ---------------------------------------------------------------------------
# 1. Binário pronto (rápido, sem compilar)
# ---------------------------------------------------------------------------
instalado=0
if [ "${DOTCROW_COMPILAR:-0}" != "1" ] && [ "$arq" = "aarch64" ]; then
  info "Procurando o DotCrow pronto para celular..."
  tmp="$(mktemp)"
  if curl -fsSL "https://github.com/$REPO/releases/latest/download/dotcrow-android-arm64" -o "$tmp" 2>/dev/null \
     && chmod 755 "$tmp" && "$tmp" --version >/dev/null 2>&1; then
    mv "$tmp" "$DESTINO"
    instalado=1
    ok "Baixado o $("$DESTINO" --version)"
  else
    rm -f "$tmp"
    info "Não achei o binário pronto nesta versão. Vou compilar no celular."
  fi
fi

# ---------------------------------------------------------------------------
# 2. Compilar no celular (Rust e clang do próprio Termux)
# ---------------------------------------------------------------------------
if [ "$instalado" -eq 0 ]; then
  info "Instalando Rust, clang e git do Termux"
  pkg install -y rust clang git make
  PASTA="${DOTCROW_SRC:-$HOME/dotcrow}"
  if [ -d "$PASTA/.git" ]; then
    info "Atualizando o código em $PASTA"
    git -C "$PASTA" pull --ff-only --quiet
  else
    info "Baixando o código para $PASTA"
    git clone --depth 1 --quiet "https://github.com/$REPO.git" "$PASTA"
  fi
  info "Compilando (no celular leva de 5 a 20 minutos; deixe a tela ligada)"
  # Dois trabalhos por vez: compilar com todos os núcleos pode faltar memória no celular.
  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" cargo build --release --locked -p dotcrow-cli --manifest-path "$PASTA/Cargo.toml"
  install -m 755 "$PASTA/target/release/dotcrow" "$DESTINO"
  ok "Compilado: $("$DESTINO" --version)"
fi

# ---------------------------------------------------------------------------
# 3. Pronto
# ---------------------------------------------------------------------------
echo
ok "DotCrow instalado no Termux!"
echo
if [ ! -d "$HOME/storage/shared" ]; then
  echo "  Para os corvos verem os arquivos do celular (Downloads, fotos, documentos), rode uma vez:"
  echo "    termux-setup-storage"
  echo "  e aceite o pedido de acesso."
  echo
fi
echo "  Experimente:"
echo
echo "    cd ~/meu-projeto && dotcrow     # modo terminal dentro da pasta do projeto"
echo "    /chave anthropic                # dentro do DotCrow: guarda sua chave de API"
echo "    /arquivos                       # navega pelas pastas do celular"
echo "    dotcrow painel                  # o painel visual, no navegador do celular"
echo
