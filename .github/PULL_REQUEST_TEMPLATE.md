## O que muda

<!-- Uma ou duas frases. Ligue a issue: Closes #123 -->

## Por quê

<!-- Que problema isso resolve para quem usa ou contribui? -->

## Como testar

<!-- Comandos ou passos para conferir. -->

## Checklist

- [ ] `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings` e `cargo test` passam
- [ ] Commits assinados com `git commit -s`
- [ ] Testes novos cobrem o comportamento novo
- [ ] Documentação e CHANGELOG atualizados, se fizer sentido
- [ ] Nenhuma chave, senha ou dado pessoal no código, nos testes ou nos logs

### Se mexe em área sensível (cripto, cofre, permissões, Vigia, Revoada)

- [ ] Expliquei o impacto na segurança acima
- [ ] Segredos continuam fora de `Debug`, `Display`, erros e logs
- [ ] Nada irreversível passa a rodar sem confirmação

<!-- Partes feitas com ajuda de IA? Conte aqui. -->
