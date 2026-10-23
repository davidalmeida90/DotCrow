#!/usr/bin/env bash
# Compila o núcleo de criptografia (dotcrow-mobile) para iOS e monta o
# DotCrowMobile.xcframework que o app Swift linka. Roda no macOS (local ou Codemagic).
#
# Produz:
#   apps/ios/Rust/DotCrowMobile.xcframework   (device arm64 + simulador)
#
# Pré-requisitos: Rust (rustup) e Xcode. O script instala os alvos iOS se faltarem.
set -euo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAIZ="$(cd "$AQUI/../.." && pwd)"         # raiz do repositório (onde está o Cargo.toml)
SAIDA="$AQUI/Rust"
HEADERS="$AQUI/Rust/include"              # dotcrow_mobile.h + module.modulemap
LIB="libdotcrow_mobile.a"

ALVO_DEVICE="aarch64-apple-ios"
ALVO_SIM_ARM="aarch64-apple-ios-sim"
ALVO_SIM_X86="x86_64-apple-ios"

echo "==> Garantindo os alvos iOS do Rust"
rustup target add "$ALVO_DEVICE" "$ALVO_SIM_ARM" "$ALVO_SIM_X86"

echo "==> Copiando o cabeçalho C para dentro dos headers do XCFramework"
mkdir -p "$HEADERS"
cp "$RAIZ/crates/dotcrow-mobile/include/dotcrow_mobile.h" "$HEADERS/"
# module.modulemap já vive em apps/ios/Rust/include/ (versionado no repo).

echo "==> cargo build (release) para cada alvo iOS"
cd "$RAIZ"
cargo build --release -p dotcrow-mobile --target "$ALVO_DEVICE"
cargo build --release -p dotcrow-mobile --target "$ALVO_SIM_ARM"
cargo build --release -p dotcrow-mobile --target "$ALVO_SIM_X86"

echo "==> Unindo as libs de simulador (arm64 + x86_64) com lipo"
SIM_DIR="$(mktemp -d)"
lipo -create \
  "$RAIZ/target/$ALVO_SIM_ARM/release/$LIB" \
  "$RAIZ/target/$ALVO_SIM_X86/release/$LIB" \
  -output "$SIM_DIR/$LIB"

echo "==> Montando o XCFramework"
rm -rf "$SAIDA/DotCrowMobile.xcframework"
xcodebuild -create-xcframework \
  -library "$RAIZ/target/$ALVO_DEVICE/release/$LIB" -headers "$HEADERS" \
  -library "$SIM_DIR/$LIB" -headers "$HEADERS" \
  -output "$SAIDA/DotCrowMobile.xcframework"

rm -rf "$SIM_DIR"
echo "==> Pronto: $SAIDA/DotCrowMobile.xcframework"
