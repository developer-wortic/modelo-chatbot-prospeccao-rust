//! `Anthropic` pela Messages API: `POST {base}/messages` com `stream: true` (SSE).
//! Cabeçalhos: `x-api-key` e `anthropic-version: 2026-10-21` (a versão atual da doc).

use serde_json::{Value, json}; // borda-json: corpo e eventos trocados com a API da Anthropic (terceiro)

use super::{
    Ajustes, Conversa, MAX_TOKENS, Passo, Provedor, ler_fluxo, mensagem_de_erro, papel_padrao,
    texto_de,
};
use crate::erro::Erro;

/// Versão da API enviada no cabeçalho `anthropic-version`, conforme a documentação atual.
pub const VERSAO_DA_API: &str = "2026-10-21";

/// Provedor `Anthropic` (Claude).
#[derive(Debug, Clone)]
pub struct Anthropic {
    ajustes: Ajustes,
}

impl Anthropic {
    #[must_use]
    pub fn new(ajustes: Ajustes) -> Self {
        Self { ajustes }
    }
}

impl Provedor for Anthropic {
    fn enviar(
        &self,
        conversa: &Conversa,
        ao_receber: &mut dyn FnMut(&str),
    ) -> Result<String, Erro> {
        let mensagens: Vec<Value> = conversa
            .mensagens
            .iter()
            .map(|m| json!({"role": papel_padrao(m.papel), "content": m.texto})) // borda-json: falas no formato da API
            .collect();
        let corpo = json!({ // borda-json: corpo da requisição à Anthropic
            "model": self.ajustes.modelo,
            "max_tokens": MAX_TOKENS,
            "system": conversa.sistema,
            "messages": mensagens,
            "stream": true,
        });
        let url = format!("{}/messages", self.ajustes.base);
        let cabecalhos = [
            ("x-api-key", self.ajustes.chave.expor()),
            ("anthropic-version", VERSAO_DA_API),
        ];
        let fluxo = self.ajustes.transporte.enviar(&url, &cabecalhos, &corpo)?;
        ler_fluxo(
            fluxo,
            ao_receber,
            &mut |_, dados| match texto_de(dados, &["type"]) {
                Some("content_block_delta")
                    if texto_de(dados, &["delta", "type"]) == Some("text_delta") =>
                {
                    texto_de(dados, &["delta", "text"])
                        .map_or(Passo::Nada, |t| Passo::Texto(t.to_string()))
                }
                Some("message_stop") => Passo::Fim,
                Some("error") => Passo::Falha(erro_da_anthropic(dados)),
                _ => Passo::Nada,
            },
        )
    }
}

/// Traduz o erro do fluxo para `Erro`, pelo tipo que a doc da `Anthropic` descreve.
fn erro_da_anthropic(dados: &Value) -> Erro {
    match texto_de(dados, &["error", "type"]) {
        Some("overloaded_error") => Erro::Servidor(529),
        Some("rate_limit_error") => Erro::LimiteDeTaxa { espere: None },
        Some("authentication_error" | "permission_error") => Erro::ChaveRecusada,
        _ => Erro::Formato(mensagem_de_erro(dados)),
    }
}
