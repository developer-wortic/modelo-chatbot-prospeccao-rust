//! Formato de chat completions (`POST {base}/chat/completions`, com `stream: true`), usado por `Groq` e `OpenRouter`.
//! O sistema entra como mensagem `system`; a fala do visitante, sempre como `user`.

use serde_json::{Value, json}; // borda-json: corpo e eventos trocados com APIs de terceiros (Groq e OpenRouter)

use super::{Ajustes, Conversa, MAX_TOKENS, Passo, ler_fluxo, mensagem_de_erro, papel_padrao};
use crate::erro::Erro;

/// Envia a conversa no formato de chat completions. `campo_tokens` é o nome do limite de saída de cada API.
pub(crate) fn enviar(
    ajustes: &Ajustes,
    conversa: &Conversa,
    ao_receber: &mut dyn FnMut(&str),
    extras: &[(&str, &str)],
    campo_tokens: &str,
) -> Result<String, Erro> {
    let mut mensagens = vec![json!({"role": "system", "content": conversa.sistema})]; // borda-json: mensagem no formato da API
    mensagens.extend(
        conversa
            .mensagens
            .iter()
            .map(|m| json!({"role": papel_padrao(m.papel), "content": m.texto})), // borda-json: falas no formato da API
    );
    let mut corpo = json!({ // borda-json: corpo da requisição de chat completions
        "model": ajustes.modelo,
        "messages": mensagens,
        "stream": true,
    });
    corpo[campo_tokens] = json!(MAX_TOKENS);
    let url = format!("{}/chat/completions", ajustes.base);
    let autorizacao = format!("Bearer {}", ajustes.chave.expor());
    let mut cabecalhos = vec![("authorization", autorizacao.as_str())];
    cabecalhos.extend_from_slice(extras);
    let fluxo = ajustes.transporte.enviar(&url, &cabecalhos, &corpo)?;
    ler_fluxo(fluxo, ao_receber, &mut passo)
}

/// Um pedaço de chat completions: o texto em `choices[0].delta.content`, ou um erro no próprio fluxo.
fn passo(_evento: Option<&str>, dados: &Value) -> Passo {
    if dados.get("error").is_some() {
        return Passo::Falha(Erro::Formato(mensagem_de_erro(dados)));
    }
    match dados
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        Some(texto) if !texto.is_empty() => Passo::Texto(texto.to_string()),
        _ => Passo::Nada,
    }
}
