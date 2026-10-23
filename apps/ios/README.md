# App iOS do DotCrow (esqueleto)

O celular acessa o computador **de qualquer lugar**, pelo túnel cifrado de ponta a ponta
(o mesmo de `docs/remoto.md`). Este é o **esqueleto** do app em SwiftUI: parear por QR,
abrir o túnel, listar os corvos e acompanhá-los ao vivo.

## A ideia em uma frase

A rede e a tela ficam em Swift; **toda a criptografia fica em Rust**, reusada byte a byte do
`dotcrow-tunel` através do crate `dotcrow-mobile` (ABI C). O celular e o computador falam,
assim, exatamente o mesmo protocolo — sem reimplementar cripto em Swift (o CryptoKit nem
tem XChaCha20-Poly1305, então reimplementar seria arriscado e divergiria do computador).

```
SwiftUI  ──▶  Tunnel.swift (WebSocket + frames)  ──▶  DotCrowFFI.swift
                                                           │  (ABI C)
                                                           ▼
                                        libdotcrow_mobile  ◀── dotcrow-tunel ◀── dotcrow-crypto
                                        (handshake + sessão, ML-KEM-768 + X25519 + XChaCha20)
```

## Estrutura

- `Sources/` — o app:
  - `Tunnel/DotCrowFFI.swift` — a casca fina sobre a ABI C (handshake e sessão).
  - `Tunnel/Tunnel.swift` — WebSocket com o relay, frames `HI/HELLO/KEY/OK/CRED/DATA`, e os
    pedidos à API do painel (inclusive SSE ao vivo).
  - `Tunnel/Frames.swift` — as tags e a moldura `tag || payload`.
  - `Models/AppModel.swift` — o estado (pareamento, corvos, tarefa) e os eventos do painel.
  - `Models/Keychain.swift` — guarda o **código durável** no Keychain (reconecta sem QR).
  - `Views/` — telas: ler o QR, lista de corvos ao vivo, caixa de conversa.
- `Rust/include/` — `module.modulemap` (o `dotcrow_mobile.h` é copiado no build).
- `project.yml` — spec do XcodeGen (gera o `.xcodeproj`).
- `build-rust.sh` — compila o `dotcrow-mobile` para iOS e monta o `DotCrowMobile.xcframework`.
- `../../codemagic.yaml` — o pipeline que faz tudo isso e exporta o IPA.

## Compilar localmente (macOS + Xcode)

```sh
brew install xcodegen
cd apps/ios
./build-rust.sh          # monta Rust/DotCrowMobile.xcframework
xcodegen generate        # gera DotCrow.xcodeproj
open DotCrow.xcodeproj   # rode no Xcode (ajuste o seu Team de assinatura)
```

## No Codemagic

Suba o repositório, aponte o app para o `codemagic.yaml` da raiz e configure o grupo de
variáveis `appstore` (veja os comentários no topo do arquivo). O pipeline gera o IPA.

## O que falta (é um esqueleto)

- Ícone e tela de abertura próprios.
- Push/retomada do túnel em segundo plano (hoje o túnel vive enquanto o app está aberto).
- Afinar os campos de `/api/estado` e dos eventos conforme o painel evoluir.

> Observação honesta: as partes de Swift/Xcode/Codemagic **não foram compiladas** no
> ambiente onde o esqueleto foi escrito (lá só há Linux, sem toolchain da Apple). O lado
> Rust (`dotcrow-mobile`) compila e tem teste de ida-e-volta passando. O primeiro build real
> no Codemagic/Xcode deve ser usado para acertar os últimos detalhes de assinatura e de
> ligação do XCFramework.
