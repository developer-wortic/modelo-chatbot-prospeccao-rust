//! `OpenRouter` pela API compatível com a `OpenAI`: `POST {base}/chat/completions`. Cabeçalho: `Authorization: Bearer`.
//! Manda só o nome do app em `X-OpenRouter-Title`. Não manda site, e-mail nem dado do usuário.

use super::{Ajustes, Conversa, Provedor, compativel};
use crate::erro::Erro;

/// Nome do app enviado ao `OpenRouter`, para aparecer no painel dele.
const TITULO_DO_APP: &str = "Chatbot de prospeccao (modelo Wortic)";

/// Provedor `OpenRouter`.
#[derive(Debug, Clone)]
pub struct OpenRouter {
    ajustes: Ajustes,
}

impl OpenRouter {
    #[must_use]
    pub fn new(ajustes: Ajustes) -> Self {
        Self { ajustes }
    }
}

impl Provedor for OpenRouter {
    fn enviar(
        &self,
        conversa: &Conversa,
        ao_receber: &mut dyn FnMut(&str),
    ) -> Result<String, Erro> {
        let extras = [("x-openrouter-title", TITULO_DO_APP)];
        compativel::enviar(&self.ajustes, conversa, ao_receber, &extras, "max_tokens")
    }
}
