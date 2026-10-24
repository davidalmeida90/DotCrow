# Modo terminal

O DotCrow também roda como um assistente de linha de comando, dentro da pasta do seu
projeto. Os corvos leem e editam os arquivos dali (um `notas.php`, por exemplo), perguntam
o que precisam para acertar de primeira e deixam tudo pronto para desfazer.

O ícone do DotCrow (o `.exe` no Windows, o `.deb` no Linux) continua abrindo o painel no
navegador. O modo terminal é só para quem abre o DotCrow **de dentro de um terminal**.

## Como abrir

```bash
cd ~/meu-site
dotcrow
```

Num terminal, `dotcrow` sem nada abre o modo terminal na pasta atual. Também funciona com
`dotcrow terminal` (ou `dotcrow t`), em qualquer sistema:

```bash
dotcrow terminal                          # na pasta atual
dotcrow terminal -C ~/meu-site            # em outra pasta
dotcrow terminal "arrume o @notas.php"    # já começa com um pedido
dotcrow terminal --direto                 # o Huginn não pergunta antes das tarefas
dotcrow terminal --modo livre             # os corvos não pedem confirmação
```

No Windows, `dotcrow` digitado no PowerShell, no CMD, no Git Bash ou no terminal do VS Code
abre o modo terminal; clicar no `dotcrow.exe` ou no atalho abre o painel, como sempre.
`DOTCROW_TERMINAL=1` força o modo terminal e `DOTCROW_TERMINAL=0` força o painel.

## Onde os corvos trabalham

| Situação | Pasta principal | Entregas (relatórios, PDFs, imagens...) |
|---|---|---|
| Dentro de um projeto (`cd ~/meu-site`) | a pasta do projeto | a pasta **Downloads** |
| Fora de um projeto (pasta pessoal, raiz do sistema) | a pasta **Downloads** | a pasta **Downloads** |

- O que é do projeto (código, páginas, configurações) fica no projeto.
- O que é entrega para você e não faz parte do projeto (um relatório, uma planilha, uma
  imagem gerada, algo baixado) vai para a Downloads, sem perguntar.
- No Termux, a Downloads é a pasta Download do celular (`~/storage/downloads`), depois de
  rodar `termux-setup-storage` uma vez.

## Pastas do projeto

Além da pasta principal, você pode pôr outras no projeto, cada uma com o seu acesso:

```
/pasta + ~/assets        # pergunta se é "ler e alterar" ou "só ler", e para que serve
/pastas                  # mostra todas, com o acesso e a anotação
/pasta - 2               # tira a pasta 2
```

- **Ler e alterar:** os corvos gravam ali sem perguntar.
- **Só ler:** os corvos leem à vontade; gravar ali pede sua confirmação.

As pastas ficam salvas: na próxima vez que você abrir o DotCrow nessa pasta, elas voltam.
A pasta de dados do DotCrow (onde fica o cofre de chaves) nunca entra no projeto.

## Ver arquivos

| Comando | O que faz |
|---|---|
| `/arquivos` | navegador de pastas numerado: `N` abre, `v N` mostra o arquivo, `@N` anexa ao próximo pedido, `+` põe a pasta no projeto, `~` pasta pessoal, `d` Downloads, `s` armazenamento do celular, `p` volta ao projeto |
| `/ver notas.php 120` | mostra o arquivo com números de linha, a partir da linha 120 |
| `/buscar titulo` | acha arquivos pelo nome e pelo conteúdo nas pastas do projeto (aceita `*.php`) |
| `@notas.php` no pedido | anexa o arquivo (ou a lista de uma pasta) ao pedido |
| Tab | completa comandos e caminhos (`@no` + Tab → `@notas.php`) |

Os corvos também ganharam a ferramenta `buscar`, para achar arquivos antes de sair abrindo
um por um.

## Alinhamento: o DotCrow pergunta

- **Primeira vez numa pasta:** o Huginn olha a estrutura e o README e faz de 3 a 5 perguntas
  sobre o projeto (o que é, o que não pode ser mexido, como testar). A Pena transforma as
  respostas no **perfil do projeto**, que vai junto em toda tarefa daquela pasta.
  `/perfil` mostra e `/alinhar` refaz ou completa.
- **Antes de cada tarefa:** o Huginn pergunta o que mudaria o resultado (escopo, onde mexer,
  o que preservar). Cada pergunta vem com respostas prováveis para escolher pelo número;
  Enter deixa a decisão com ele e `/pular` começa sem responder.
  `/perguntas direto` desliga e `/perguntas sempre` liga de novo.
- **No meio da tarefa:** os corvos podem usar a ferramenta `perguntar` quando uma decisão
  depende de você.

## Segurança e desfazer

- Os corvos editam só o trecho que precisa (`editar_arquivo`), em vez de reescrever o arquivo.
- Antes de mudar um arquivo pela primeira vez numa tarefa, uma cópia do original vai para a
  pasta de dados do DotCrow (fora do projeto, para não subir para o servidor junto com o site).
- `/desfazer` volta os arquivos da última tarefa e apaga os que ela criou. Mudanças feitas
  por comandos de terminal não entram: para isso, use o git.
- Apagar arquivos do projeto por comando (`rm`, `git reset --hard`, `git clean`...) sempre
  pede confirmação, mesmo dentro da pasta.
- Ctrl+C para a tarefa em andamento (numa confirmação, também).

## Outros comandos

| Comando | O que faz |
|---|---|
| `!git status` | roda um comando na pasta do projeto, aqui mesmo |
| `/modo computador \| livre \| sandbox` | quanto os corvos podem fazer sem perguntar |
| `/provedor anthropic` | usa só esse provedor nesta sessão (`/provedor auto` volta à troca automática) |
| `/chave anthropic` | guarda uma chave de API (ela não aparece na tela) |
| `/testar` | testa as chaves |
| `/nova` | conversa nova (o perfil e as pastas continuam) |
| `/ajuda` | todos os comandos |
| `/sair` ou Ctrl+D | sai |

## No celular (Termux)

1. Instale o [Termux](https://f-droid.org/packages/com.termux/) pelo F-Droid ou pelo GitHub
   (o da Play Store é antigo).
2. No Termux:
   ```bash
   curl -fsSL https://raw.githubusercontent.com/napoleaomp4/dotcrow/main/instalar-termux.sh | bash
   termux-setup-storage   # para os corvos verem os arquivos do celular
   ```
   O instalador baixa o binário pronto para celular (arm64) da última versão; se não houver,
   instala o Rust do Termux e compila (de 5 a 20 minutos).
3. Entre na pasta do projeto e rode `dotcrow`. Dentro dele, `/chave anthropic` guarda a chave.

No Android não há mouse, teclado nem janelas para os corvos usarem, e o modo sandbox não
existe. Terminal, arquivos, internet, perguntas e desfazer funcionam como no computador.
`dotcrow painel` abre o painel no navegador do celular.
