#!/usr/bin/env bash
# Monta o pacote .deb do DotCrow a partir de um binário já compilado.
#
# Uso:
#   cargo build --release
#   packaging/linux/build-deb.sh [binário] [pasta-de-saída]
#
# Padrões: binário = target/release/dotcrow, saída = dist/.
# A arquitetura vem de DEB_ARCH (padrão: amd64).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${1:-$ROOT/target/release/dotcrow}"
OUT="${2:-$ROOT/dist}"
ARCH="${DEB_ARCH:-amd64}"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)"
ICON="$ROOT/assets/brand/png/dotcrow-app-icon-512.png"

[ -x "$BIN" ] || { echo "binário não encontrado: $BIN (rode: cargo build --release)" >&2; exit 1; }

PKG="$(mktemp -d)"
trap 'rm -rf "$PKG"' EXIT

install -Dm755 "$BIN"                               "$PKG/usr/bin/dotcrow"
install -Dm644 "$ROOT/packaging/linux/dotcrow.desktop" "$PKG/usr/share/applications/dotcrow.desktop"
install -Dm644 "$ICON"                              "$PKG/usr/share/icons/hicolor/512x512/apps/dotcrow.png"
install -Dm644 "$ROOT/LICENSE"                      "$PKG/usr/share/doc/dotcrow/copyright"

# Tamanho instalado, em KiB, para o campo Installed-Size.
SIZE="$(du -ks "$PKG/usr" | cut -f1)"

install -d "$PKG/DEBIAN"
cat > "$PKG/DEBIAN/control" <<EOF
Package: dotcrow
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Depends: libc6, ca-certificates
Installed-Size: $SIZE
Maintainer: Napoleão de Sousa Carvalho Junior <contato.napoleao2023@gmail.com>
Homepage: https://dotcrow.si
Description: Agentes open source que trabalham no seu computador
 DotCrow é um bando de agentes (os corvos) que recebe uma tarefa completa e a
 resolve: um planeja, um revisa os riscos, um executa e um resume. Inclui um
 painel visual (dotcrow painel) para guardar e testar chaves de API, falar com
 vários provedores de LLM (Claude, GPT, Gemini e outros) e acompanhar cada
 etapa ao vivo, no computador e no celular.
EOF

mkdir -p "$OUT"
DEB="$OUT/dotcrow_${VERSION}_${ARCH}.deb"
dpkg-deb --root-owner-group --build "$PKG" "$DEB" >/dev/null
echo "$DEB"
