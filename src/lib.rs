//! Chatbot de prospecção da Wortic: modelo em Rust de uso livre com créditos (licença AAL), com a sua chave (BYOK).
//!
//! Um roteiro em TOML diz quem é a empresa, o que vende, para quem, quais perguntas de qualificação fazer
//! e quando passar para um humano. O bot conversa, qualifica e resume o lead (nome, contato, interesse,
//! nota de 0 a 100 com o motivo). Funciona com `OpenAI`, `Anthropic`, Google (`Gemini`), `Groq` e `OpenRouter`.

pub mod atendimento;
pub mod configuracao;
pub mod erro;
pub mod guarda;
pub mod lead;
pub mod provedor;
pub mod roteiro;
pub mod servidor;
pub mod sse;
