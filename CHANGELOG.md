# Changelog

Todas as mudanças relevantes do DotCrow ficam registradas aqui.

O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e o projeto usa [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado

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
