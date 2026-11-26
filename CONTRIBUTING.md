# Como contribuir

Obrigado por querer ajudar. Este é um modelo pequeno, feito para ser fácil de ler e de adaptar.

## Antes de começar

- Para mudança grande, abra uma issue antes do código.
- Não ponha chave de API, dado de visitante ou dado pessoal em issue ou pull request. Use dados de exemplo, como a "Loja Exemplo" do roteiro.

## Rodar as checagens

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings -W clippy::pedantic
cargo test
cargo llvm-cov --summary-only --ignore-filename-regex '(tests/|examples/|/rustc/|\.cargo/)'
```

O CI roda formatação, clippy (pedantic) e os testes em cada push e em cada pull request. A cobertura de linhas de `src/` deve continuar em 100%.

## Regras do projeto

- Dependência nova precisa de motivo claro.
- Mudança de código vem com teste. Provedor novo vem com teste contra o servidor falso, conferindo o formato da requisição com a documentação oficial.
- Nenhum teste faz chamada real a um provedor. Nenhum teste gasta crédito de ninguém.
- A tela continua em HTML e JavaScript puros, em `static/`, sem build de front-end.
- A chave nunca aparece em log, em resposta de rota, em erro nem na `Debug`.
- O projeto usa a licença AAL (uso livre com créditos à Wortic). Ao contribuir, você concorda que sua contribuição entra sob a mesma licença.

## Pull request

- Diga o que muda e por quê.
- Inclua a saída de `cargo test` e do `cargo llvm-cov`.
