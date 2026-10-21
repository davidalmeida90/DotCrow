# Changelog

Todas as mudanças relevantes do DotCrow ficam registradas aqui.

O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e o projeto usa [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

## [0.3.0] - 2026-10-21

### Adicionado

- **Cada etapa usa o modelo do porte certo.** Planejar e revisar (Huginn e Vigia) usam a faixa forte (Deep); executar, a equilibrada (Standard); o resumo da Pena, a leve (Fast). No Anthropic isso vira Opus, Sonnet e Haiku. Quem fixa um modelo na aba Provedores continua mandando. O `tier` entra no `CompletionRequest`; cada provedor escolhe o modelo da sua faixa.

### Alterado

- Testar as chaves usa a faixa leve (mais barato).

### Segurança

- Comandos que destroem o sistema ou um disco (`rm -rf /`, `mkfs`, `dd of=/dev/...`, `wipefs`, fork bomb, `format c:`) agora são **recusados em qualquer modo**, não só confirmados.

## [0.2.0] - 2026-10-21

### Adicionado

- **Os corvos agem no computador.** Quem executa trabalha em ciclos (pede uma ação, vê o resultado, decide a próxima) com as ferramentas `terminal`, `ler_arquivo`, `escrever_arquivo`, `listar`, `web` (internet sem navegador), `navegador` (Chrome invisível com JavaScript e captura), `baixar`, `abrir`, `janelas`, `focar`, `mouse`, `teclado` e `tela`. Cada ação aparece ao vivo no painel e no `dotcrow tarefa`.
- Três modos por conversa: **Computador** (pergunta o que é sensível), **Livre** (não pergunta, exceto o que destrói o sistema) e **Sandbox** (caixa isolada com bubblewrap; só a pasta da conversa muda).
- Confirmações no painel (**Permitir**, **Permitir sempre nesta conversa**, **Negar**) e no terminal. O cofre de chaves nunca é lido pelos corvos.
- Pastas por conversa em `~/DotCrow/conversas/`, com `conversa.md`; pedidos seguidos na mesma conversa lembram o que já foi feito. O painel lista os arquivos e abre a pasta.
- O painel abre numa janela própria (modo aplicativo do Chrome/Edge/Brave), sem barra de endereço.
- API: `modo` e `conversa` em `POST /api/tarefas`, `POST /api/aprovacoes/{id}`, `GET /api/conversas/{id}/arquivos`, `POST /api/abrir` e os eventos `action_*` e `approval_*`.

### Alterado

- Visual em preto e branco.
- Com o provedor só ocupado (limite de uso ou fora do ar), os corvos esperam e tentam de novo em vez de parar a tarefa; o erro final diz o motivo de cada provedor.
- O `.deb` recomenda `xdotool`, `wmctrl`, `bubblewrap` e um programa de captura de tela.

### Corrigido

- Recusa do modelo (Anthropic `stop_reason: refusal`) aparece como recusa, não como "resposta sem texto".

### Corrigido

- O ícone do Linux (`.deb`) chama `/usr/bin/dotcrow` pelo caminho completo. Antes, um `dotcrow` antigo em `~/.local/bin` passava na frente, e clicar no ícone não abria nada.
- Quando o painel não consegue abrir pelo ícone (sem terminal), o erro aparece na tela: notificação no Linux e janela de aviso no Windows e no macOS. Antes, o erro sumia sem ninguém ver.

### Adicionado

- `instalar.sh` cria o ícone do DotCrow no menu de aplicativos do Linux, apontando para o comando que ele acabou de instalar.

- O ícone do DotCrow (Linux e Windows) abre o painel no navegador sem passar pelo terminal. `dotcrow` sem comando abre o painel; clicar de novo com ele aberto só traz a página de volta; o botão "Fechar painel" (`POST /api/desligar`) encerra.
- Pacotes prontos para instalar sem compilar: `.deb` (Linux), `dotcrow.exe` portátil e `dotcrow-setup.exe` (instalador do Windows, NSIS), além do `instalar.ps1` para Windows. Gerados pelo workflow `release.yml` a cada tag `v*`.

- Painel visual (`dotcrow painel`): chaves de API, testes, tarefas com cada etapa ao vivo, pergunta rápida, provedores e atividade. Funciona no celular com `--rede` e QR code, e pode ser instalado na tela de início.
- API JSON do painel (estado, chaves, provedores, testes, perguntas, tarefas e eventos ao vivo por SSE), com código de acesso, checagem de `Host` e CSP. Documentada em `docs/painel.md`.
- Adaptadores reais de LLM: Anthropic (Claude), OpenAI, Google Gemini e os compatíveis com a OpenAI (OpenRouter, DeepSeek, Groq e Ollama), com classificação de erros específica de cada provedor.
- `catalog` de provedores, `KeySource` para os adaptadores pegarem a chave do cofre só na hora e o erro `Misconfigured` (modelo ou endereço errado), que também troca de provedor.
- Orquestrador dos corvos com LLM real: Huginn planeja, Vigia revisa e pode bloquear, Pena/Gralha/Bico entregam, Vigia confere e Pena resume. Ainda sem executar nada no computador.
- Comandos `testar`, `perguntar`, `tarefa`, `provedores` (ordem, modelo, url) e `painel`. Apelidos como `claude`, `gpt` e `google` viram o id certo.
- `instalar.sh`: instala Rust, compilador e o DotCrow com um comando.
- Documentação inicial: README, guia de contribuição, código de conduta, política de segurança, governança e roadmap.
- `dotcrow-crypto`: envelope híbrido ML-KEM-768 + X25519 com XChaCha20-Poly1305 e cifra simétrica para dados em repouso.
- `dotcrow-vault`: cofre de chaves de API somente-escrita (adicionar, listar, apagar), cifrado em disco.
- `dotcrow-providers`: interface comum de LLM e roteador com troca automática de provedor.
- `dotcrow-core`: permissões, níveis de risco e política de confirmação.
- `dotcrow-agents`: os seis agentes (Huginn, Muninn, Gralha, Bico, Pena e Vigia) e o fluxo padrão.
- `dotcrow-cli`: comandos `chaves`, `agentes`, `fluxo` e `demo troca`.
- Identidade visual (logo, favicon, ícone e capa do repositório).
