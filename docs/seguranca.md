# Segurança e modelo de ameaças

O DotCrow vai mexer no computador das pessoas, guardar chaves de API que custam dinheiro e compartilhar aprendizado entre instalações. Este documento diz **o que protegemos, como, e o que não conseguimos proteger**. Se encontrar uma falha, siga o [SECURITY.md](../SECURITY.md).

> **Estado atual:** a criptografia usa bibliotecas reconhecidas (RustCrypto e dalek), mas a forma como as combinamos **não passou por auditoria externa**. Isso está previsto para antes da versão 1.0.

## O que queremos proteger

| Ativo | Por quê |
|---|---|
| Chaves de API | Custam dinheiro e dão acesso às contas do dono nos provedores. |
| Arquivos e contas do dono | O DotCrow vai poder ler, escrever e agir neles. |
| Dados pessoais | Não podem sair da máquina, nem para a Revoada. |
| Integridade das ações | Nada pode acontecer sem passar pela política e, quando sério, pelo dono. |
| Identidade da instalação | Ninguém pode se passar por outro DotCrow na Revoada. |

## De quem nos defendemos

1. **Conteúdo malicioso** (o mais provável): uma página, e-mail, PDF ou receita da Revoada com instruções escondidas para o agente, o chamado *prompt injection*.
2. **Uma LLM que erra ou "alucina"** um comando perigoso.
3. **Outros programas e usuários sem privilégio** na mesma máquina tentando ler o cofre ou falar com o Ninho.
4. **A rede**: quem intercepta o tráfego hoje para tentar decifrar no futuro com um computador quântico ("guarde agora, decifre depois").
5. **Um servidor da Revoada comprometido**, ou receitas envenenadas enviadas por instalações mal-intencionadas.

## O que **não** conseguimos proteger

- **Malware com privilégio de administrador** na máquina do dono. Ele pode ler a memória de qualquer processo, inclusive a chave no momento em que é usada. Nenhum programa local resolve isso; o sistema operacional precisa estar limpo.
- **O próprio dono decidindo fazer algo ruim.** O DotCrow pede confirmação e explica o risco, mas a decisão é de quem é dono do computador.
- **O que os provedores de LLM fazem com o que recebem.** O conteúdo enviado ao Claude, GPT etc. segue a política de cada provedor. Para dados sensíveis, use um modelo local (Ollama).

## Defesas

### 1. Política de permissões

Toda ação proposta por um agente vira uma `Action` com permissões (`ler`, `escrever`, `apagar`, `executar`, `rede`, `sistema`), risco e se é reversível. A `Policy` (em `dotcrow-core`) decide:

- ação **irreversível** → sempre pede confirmação;
- risco acima do limite → pede confirmação;
- permissão bloqueada → recusa, mesmo com confirmação;
- padrão de fábrica: só **ler** roda sozinho; **sistema** é bloqueado.

Cada agente também tem uma lista fixa do que pode pedir (veja [agentes.md](agentes.md)).

### 2. Conteúdo externo é dado, não ordem

Respostas de LLM, páginas, e-mails, arquivos e receitas entram no contexto marcados como **não confiáveis**. O Vigia revisa cada passo sabendo disso, e a política vale de qualquer forma: mesmo que um texto convença um agente, ele não ganha permissões que o agente não tem nem pula confirmações. Vamos manter uma bateria de ataques conhecidos como teste automatizado (issue na fase 2).

### 3. Cofre de chaves somente-escrita

Resumo (detalhes em [cofre-de-chaves.md](cofre-de-chaves.md)):

- não existe operação que devolva a chave para a CLI, o app ou a API local;
- em disco, tudo é um blob cifrado com XChaCha20-Poly1305;
- na memória, segredos ficam em `Zeroizing` e são apagados ao sair de uso;
- `Debug` mascarado e testes garantindo que a chave não aparece em listagens e logs.

### 4. Criptografia híbrida pós-quântica

**Envelope (mensagens entre identidades)** — implementado em `dotcrow-crypto::envelope`:

| Etapa | Algoritmo |
|---|---|
| Acordo de chave pós-quântico | ML-KEM-768 (FIPS 203) |
| Acordo de chave clássico | X25519 efêmero |
| Combinação | HKDF-SHA256 sobre os dois segredos, amarrando o texto cifrado do KEM e as chaves públicas |
| Cifra | XChaCha20-Poly1305, cabeçalho inteiro autenticado |

Para ler uma mensagem, o atacante precisa quebrar **os dois** acordos de chave. Se um dia aparecer uma falha no ML-KEM, o X25519 continua protegendo, e vice-versa contra computadores quânticos. Decisão registrada no [ADR 0002](adr/0002-criptografia-hibrida.md).

**Dados em repouso** — `dotcrow-crypto::sym`: XChaCha20-Poly1305 com nonce aleatório de 24 bytes e dados associados por contexto.

**Planejado:**

- assinaturas híbridas **Ed25519 + ML-DSA-65** (FIPS 204) para receitas e certificados;
- **certificado digital por instalação**: cada DotCrow gera seu par de chaves localmente e a Revoada emite um certificado que liga a instalação às chaves públicas; a chave privada nunca sai da máquina;
- **TLS com troca de chaves híbrida** (X25519 + ML-KEM-768) entre o Ninho e a Revoada;
- **ponta a ponta de verdade** na sincronização entre os dispositivos do mesmo dono: o servidor guarda só blobs cifrados que não consegue ler.

### 5. Ponta a ponta: onde se aplica e onde não

Para não prometer o que não existe:

- **Seus dados privados** (histórico, memória local, sincronização entre seus dispositivos) serão cifrados ponta a ponta: só os seus dispositivos têm as chaves.
- **As receitas compartilhadas na Revoada** precisam ser lidas pelo servidor para que a busca funcione para todo o bando. Por isso elas são **anônimas por construção**: passam pelo filtro de privacidade e pela revisão do dono antes de sair, e viajam cifradas com TLS híbrido. Veja [revoada.md](revoada.md).

### 6. Auditoria

Toda decisão da política, confirmação do dono, ação executada e troca de provedor vai para um log local encadeado por hash, para que alterações posteriores sejam detectáveis (fase 1).

## Práticas de desenvolvimento

- `unsafe` proibido no workspace inteiro.
- Dependências revisadas por `cargo-deny` no CI (licenças e vulnerabilidades conhecidas) e atualizadas pelo Dependabot.
- Mudanças em áreas sensíveis exigem duas aprovações (veja [GOVERNANCE.md](../GOVERNANCE.md)).
