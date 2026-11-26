# Segurança

Se você achar uma falha de segurança, não abra issue pública. Relate em privado pelo [aviso de segurança do GitHub](https://github.com/developer-wortic/modelo-chatbot-prospeccao-rust/security/advisories/new).

Inclua o que você encontrou, como reproduzir e o impacto que você vê.

Não envie chave de API, senha, dado de visitante ou outro dado pessoal no relato. Se uma chave vazou, revogue-a na página do provedor e crie outra.

Áreas que mais importam: vazamento da chave (log, resposta, tela), escape de instrução pela fala do visitante (injeção), exposição da administração fora da máquina local e CORS aberto além da lista `CHATBOT_ORIGENS`.

Só a versão mais recente da branch principal recebe correção. Vamos responder pelo mesmo canal.
