//! `Groq` pela API compatível com a `OpenAI`: `POST {base}/chat/completions`. Cabeçalho: `Authorization: Bearer`.
//! O limite de saída vai em `max_completion_tokens`, porque `max_tokens` está marcado como depreciado na doc da `Groq`.

use super::{Ajustes, Conversa, Provedor, compativel};
use crate::erro::Erro;

/// Provedor `Groq`.
#[derive(Debug, Clone)]
pub struct Groq {
    ajustes: Ajustes,
}

impl Groq {
    #[must_use]
    pub fn new(ajustes: Ajustes) -> Self {
        Self { ajustes }
    }
}

impl Provedor for Groq {
    fn enviar(
        &self,
        conversa: &Conversa,
        ao_receber: &mut dyn FnMut(&str),
    ) -> Result<String, Erro> {
        compativel::enviar(
            &self.ajustes,
            conversa,
            ao_receber,
            &[],
            "max_completion_tokens",
        )
    }
}
