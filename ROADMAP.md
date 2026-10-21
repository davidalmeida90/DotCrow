# Roadmap

O DotCrow avança em fases. Cada fase entrega algo utilizável e seguro antes de dar mais poder aos agentes. As issues de cada fase ficam nos marcos (*milestones*) do GitHub com o mesmo nome.

## Fase 0 · Fundação ✅

- [x] Documentação, licença e guias de contribuição e segurança
- [x] Cofre de chaves somente-escrita, cifrado em disco
- [x] Roteador de LLMs com troca automática e "castigo" para provedores que falham
- [x] Envelope híbrido pós-quântico ML-KEM-768 + X25519
- [x] Política de permissões e confirmação
- [x] Perfis e fluxo dos seis agentes
- [x] CLI `dotcrow` e CI

## Fase 1 · Ninho

O serviço local que recebe tarefas e conversa com as LLMs de verdade.

- [x] Adaptadores: Anthropic (Claude), OpenAI, Google Gemini, OpenRouter, DeepSeek, Groq, Ollama
- [x] Ordem de preferência dos provedores configurável
- [x] Painel visual no navegador e no celular, com API JSON para apps
- [x] Instalador de um comando (`instalar.sh`)
- [ ] Chave mestra do cofre no chaveiro do sistema (Keychain, Credential Manager, Secret Service)
- [ ] Ninho: serviço local com comunicação autenticada com a CLI, sem nenhum comando de leitura de chave
- [x] Orquestrador rodando o fluxo dos agentes com uma LLM real (só leitura e planejamento)
- [ ] Tarefas guardadas em disco (hoje ficam na memória enquanto o painel está aberto)
- [ ] HTTPS no painel para o modo `--rede`
- [ ] Log de auditoria encadeado por hash
- [x] Ferramentas de leitura de arquivos com escopo de pastas (pasta por conversa; cofre sempre de fora)

## Fase 2 · Mãos no computador

- [ ] Ponte com o WhatsApp: mandar tarefas e receber o resultado pelo celular, usando a API do painel
- [ ] Ver a tela do computador ao vivo no painel (computador e celular), com o que cada corvo está fazendo
- [x] Confirmar ou recusar ações sérias pelo painel (computador e celular)
- [x] Ferramentas de escrita e execução com confirmação (v0.2.0)
- [ ] Desfazer automático das ações
- [x] Isolamento (sandbox) por conversa, com bubblewrap no Linux
- [ ] Sandbox no Windows e no macOS
- [x] Navegador: internet direta (`web`) e Chrome invisível com JavaScript (`navegador`)
- [x] Mouse, teclado, janelas e captura de tela no Linux (xdotool, wmctrl)
- [ ] Ver a captura de tela com modelos de visão; mouse e teclado no Windows e no macOS
- [x] Janela própria do painel (modo aplicativo)
- [ ] App de desktop nativo (Windows, macOS, Linux)
- [ ] Bateria de testes contra prompt injection que o Vigia precisa barrar

## Fase 3 · Memória e modo aprendizagem

- [x] Memória local cifrada com busca por palavras (v0.5.0)
- [ ] Busca semântica (embeddings locais)
- [x] Gravação automática pelo Muninn ao fim de cada tarefa (v0.5.0)
- [x] Remoção de segredos (chaves/senhas) antes de guardar e de enviar à IA (v0.5.0)
- [ ] Modo aprendizagem: o dono mostra como faz, o DotCrow vira receita

## Fase 4 · Revoada

A memória coletiva: o que um corvo aprende serve para o bando.

- [ ] Assinaturas híbridas Ed25519 + ML-DSA-65
- [ ] Identidade por instalação com certificado digital
- [ ] Servidor da Revoada com TLS híbrido pós-quântico
- [ ] Compartilhamento opcional de receitas, com revisão pelo dono
- [ ] Reputação das receitas e defesa contra envenenamento
- [ ] Sincronização ponta a ponta entre os dispositivos do mesmo dono

## Fase 5 · 1.0

- [ ] Auditoria de segurança externa
- [ ] Instaladores assinados e atualização automática verificada
- [ ] Documentação completa em português e inglês
