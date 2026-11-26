//! Google `Gemini` pela `generateContent` em modo de streaming: `POST {base}/models/{modelo}:streamGenerateContent?alt=sse`.
//! A chave vai no cabeçalho `x-goog-api-key`, e não na URL, para não aparecer em log.

use serde_json::{Value, json}; // borda-json: corpo e eventos trocados com a API do Gemini (terceiro)

use super::{Ajustes, Conversa, MAX_TOKENS, Papel, Passo, Provedor, ler_fluxo, texto_de};
use crate::erro::Erro;

/// Provedor Google (`Gemini`).
#[derive(Debug, Clone)]
pub struct Gemini {
    ajustes: Ajustes,
}

impl Gemini {
    #[must_use]
    pub fn new(ajustes: Ajustes) -> Self {
        Self { ajustes }
    }
}

/// No `Gemini`, quem responde é `model`, e não `assistant`.
fn papel(papel: Papel) -> &'static str {
    match papel {
        Papel::Visitante => "user",
        Papel::Assistente => "model",
    }
}

impl Provedor for Gemini {
    fn enviar(
        &self,
        conversa: &Conversa,
        ao_receber: &mut dyn FnMut(&str),
    ) -> Result<String, Erro> {
        let conteudos: Vec<Value> = conversa
            .mensagens
            .iter()
            .map(|m| json!({"role": papel(m.papel), "parts": [{"text": m.texto}]})) // borda-json: falas no formato da API
            .collect();
        let corpo = json!({ // borda-json: corpo da requisição ao Gemini
            "systemInstruction": {"parts": [{"text": conversa.sistema}]},
            "contents": conteudos,
            "generationConfig": {"maxOutputTokens": MAX_TOKENS},
        });
        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.ajustes.base, self.ajustes.modelo
        );
        let cabecalhos = [("x-goog-api-key", self.ajustes.chave.expor())];
        let fluxo = self.ajustes.transporte.enviar(&url, &cabecalhos, &corpo)?;
        ler_fluxo(fluxo, ao_receber, &mut |_, dados| {
            if let Some(bloqueio) = texto_de(dados, &["promptFeedback", "blockReason"]) {
                return Passo::Falha(Erro::Formato(format!("prompt bloqueado: {bloqueio}")));
            }
            let partes = dados
                .pointer("/candidates/0/content/parts")
                .and_then(Value::as_array);
            let texto: String = partes
                .into_iter()
                .flatten()
                .filter_map(|parte| parte["text"].as_str())
                .collect();
            if texto.is_empty() {
                Passo::Nada
            } else {
                Passo::Texto(texto)
            }
        })
    }
}
