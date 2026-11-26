//! `OpenAI` pela Responses API: `POST {base}/responses` com `stream: true` (SSE).
//! A doc atual recomenda a Responses API para apps de texto novos. Cabeçalho: `Authorization: Bearer`.

use serde_json::{Value, json}; // borda-json: corpo e eventos trocados com a API da OpenAI (terceiro)

use super::{
    Ajustes, Conversa, MAX_TOKENS, Passo, Provedor, ler_fluxo, mensagem_de_erro, papel_padrao,
    texto_de,
};
use crate::erro::Erro;

/// Provedor `OpenAI`.
#[derive(Debug, Clone)]
pub struct OpenAi {
    ajustes: Ajustes,
}

impl OpenAi {
    #[must_use]
    pub fn new(ajustes: Ajustes) -> Self {
        Self { ajustes }
    }
}

impl Provedor for OpenAi {
    fn enviar(
        &self,
        conversa: &Conversa,
        ao_receber: &mut dyn FnMut(&str),
    ) -> Result<String, Erro> {
        let entrada: Vec<Value> = conversa
            .mensagens
            .iter()
            .map(|m| json!({"role": papel_padrao(m.papel), "content": m.texto})) // borda-json: falas no formato da API
            .collect();
        let corpo = json!({ // borda-json: corpo da requisição à OpenAI
            "model": self.ajustes.modelo,
            "instructions": conversa.sistema,
            "input": entrada,
            "max_output_tokens": MAX_TOKENS,
            "stream": true,
            "store": false,
        });
        let url = format!("{}/responses", self.ajustes.base);
        let autorizacao = format!("Bearer {}", self.ajustes.chave.expor());
        let fluxo =
            self.ajustes
                .transporte
                .enviar(&url, &[("authorization", &autorizacao)], &corpo)?;
        ler_fluxo(
            fluxo,
            ao_receber,
            &mut |_, dados| match texto_de(dados, &["type"]) {
                Some("response.output_text.delta") => {
                    texto_de(dados, &["delta"]).map_or(Passo::Nada, |t| Passo::Texto(t.to_string()))
                }
                Some("response.completed") => Passo::Fim,
                Some("response.failed" | "error") => {
                    Passo::Falha(Erro::Formato(mensagem_de_erro(dados)))
                }
                _ => Passo::Nada,
            },
        )
    }
}
