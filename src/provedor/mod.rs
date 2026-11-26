//! Um traço, `Provedor`, e uma implementação por provedor (um arquivo cada).
//! Todos recebem a mesma `Conversa` e devolvem o mesmo texto. Todos usam streaming por SSE.

pub mod anthropic;
pub mod compativel;
pub mod gemini;
pub mod groq;
pub mod openai;
pub mod openrouter;
mod transporte;

use std::fmt;
use std::time::Duration;

use serde_json::Value; // borda-json: fala com a API do provedor (terceiro), que responde em JSON

use crate::erro::Erro;
use crate::sse;
pub use transporte::{Fluxo, Transporte};

/// Limite de tokens de saída de cada resposta do bot. Uma resposta de prospecção é curta.
pub const MAX_TOKENS: u32 = 600;

/// Quem falou na conversa: o visitante ou o bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    Visitante,
    Assistente,
}

/// Uma fala da conversa. O texto do visitante é sempre dado, nunca instrução.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mensagem {
    pub papel: Papel,
    pub texto: String,
}

/// A conversa que entra num provedor: as instruções do sistema e as falas, em ordem.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Conversa {
    pub sistema: String,
    pub mensagens: Vec<Mensagem>,
}

impl Conversa {
    /// Conversa vazia com as instruções do sistema.
    pub fn nova(sistema: impl Into<String>) -> Self {
        Self {
            sistema: sistema.into(),
            mensagens: Vec::new(),
        }
    }
}

/// Um provedor de IA. `ao_receber` recebe cada pedaço do streaming, e o texto final volta no `Ok`.
pub trait Provedor: Send + Sync {
    /// Envia a conversa e devolve o texto completo da resposta.
    ///
    /// # Errors
    ///
    /// Devolve `Erro` para chave recusada, limite, falha do provedor, tempo-limite, rede ou formato.
    fn enviar(&self, conversa: &Conversa, ao_receber: &mut dyn FnMut(&str))
    -> Result<String, Erro>;
}

/// Chave que não aparece em log nem em `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Segredo(String);

impl Segredo {
    pub fn new(valor: impl Into<String>) -> Self {
        Self(valor.into())
    }

    /// O valor, só para montar o cabeçalho de autenticação.
    #[must_use]
    pub fn expor(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Segredo(***)")
    }
}

/// Ajustes comuns de todo provedor: chave, endereço base, modelo e transporte.
#[derive(Debug, Clone)]
pub struct Ajustes {
    pub chave: Segredo,
    pub base: String,
    pub modelo: String,
    pub transporte: Transporte,
}

/// Os cinco provedores aceitos: `OpenAI`, `Anthropic`, `Gemini`, `Groq` e `OpenRouter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    OpenAi,
    Anthropic,
    Gemini,
    Groq,
    OpenRouter,
}

impl Tipo {
    /// Todos os tipos, na ordem da tela de onboarding.
    pub const TODOS: [Tipo; 5] = [
        Tipo::OpenAi,
        Tipo::Anthropic,
        Tipo::Gemini,
        Tipo::Groq,
        Tipo::OpenRouter,
    ];

    /// Código usado em arquivos e na API local (`openai`, `anthropic`...).
    #[must_use]
    pub fn codigo(self) -> &'static str {
        match self {
            Tipo::OpenAi => "openai",
            Tipo::Anthropic => "anthropic",
            Tipo::Gemini => "gemini",
            Tipo::Groq => "groq",
            Tipo::OpenRouter => "openrouter",
        }
    }

    /// Lê o código; `None` para um provedor desconhecido.
    #[must_use]
    pub fn de_codigo(codigo: &str) -> Option<Tipo> {
        Tipo::TODOS.into_iter().find(|t| t.codigo() == codigo)
    }

    /// Nome para a tela.
    #[must_use]
    pub fn nome(self) -> &'static str {
        match self {
            Tipo::OpenAi => "OpenAI",
            Tipo::Anthropic => "Anthropic (Claude)",
            Tipo::Gemini => "Google (Gemini)",
            Tipo::Groq => "Groq",
            Tipo::OpenRouter => "OpenRouter",
        }
    }

    /// Página oficial onde se cria a chave. Esses endereços vêm da documentação de cada provedor.
    #[must_use]
    pub fn pagina_da_chave(self) -> &'static str {
        match self {
            Tipo::OpenAi => "https://platform.openai.com/api-keys",
            Tipo::Anthropic => "https://platform.claude.com/settings/keys",
            Tipo::Gemini => "https://aistudio.google.com/apikey",
            Tipo::Groq => "https://console.groq.com/keys",
            Tipo::OpenRouter => "https://openrouter.ai/settings/keys",
        }
    }

    /// Endereço base da API oficial, sem barra no fim.
    #[must_use]
    pub fn base_padrao(self) -> &'static str {
        match self {
            Tipo::OpenAi => "https://api.openai.com/v1",
            Tipo::Anthropic => "https://api.anthropic.com/v1",
            Tipo::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            Tipo::Groq => "https://api.groq.com/openai/v1",
            Tipo::OpenRouter => "https://openrouter.ai/api/v1",
        }
    }

    /// Modelo padrão: um modelo barato e atual, conferido na documentação de cada provedor.
    #[must_use]
    pub fn modelo_padrao(self) -> &'static str {
        match self {
            Tipo::OpenAi => "gpt-6-astra",
            Tipo::Anthropic => "claude-haiku-5-5",
            Tipo::Gemini => "gemini-3.5-flash-lite",
            Tipo::Groq => "llama-3.1-8b-instant",
            Tipo::OpenRouter => "anthropic/claude-haiku-5.5",
        }
    }
}

/// Papel de uma fala no formato da `OpenAI` e da `Anthropic` (`user` ou `assistant`).
pub(crate) fn papel_padrao(papel: Papel) -> &'static str {
    match papel {
        Papel::Visitante => "user",
        Papel::Assistente => "assistant",
    }
}

/// Cria o provedor pedido. `ajustes.base` substitui o endereço padrão (usado em testes e em proxies).
#[must_use]
pub fn criar(tipo: Tipo, ajustes: Ajustes) -> Box<dyn Provedor> {
    match tipo {
        Tipo::OpenAi => Box::new(openai::OpenAi::new(ajustes)),
        Tipo::Anthropic => Box::new(anthropic::Anthropic::new(ajustes)),
        Tipo::Gemini => Box::new(gemini::Gemini::new(ajustes)),
        Tipo::Groq => Box::new(groq::Groq::new(ajustes)),
        Tipo::OpenRouter => Box::new(openrouter::OpenRouter::new(ajustes)),
    }
}

/// Resultado de olhar um evento do fluxo.
pub(crate) enum Passo {
    /// Pedaço de texto para somar à resposta.
    Texto(String),
    /// O provedor avisou que terminou.
    Fim,
    /// Evento sem texto (ping, metadados...).
    Nada,
    /// O provedor mandou erro no meio do fluxo.
    Falha(Erro),
}

/// Percorre o fluxo SSE de um provedor. `passo` decide o que cada evento significa.
/// Não é genérico, para existir uma só cópia desta função no binário.
pub(crate) fn ler_fluxo(
    fluxo: Fluxo,
    ao_receber: &mut dyn FnMut(&str),
    passo: &mut dyn FnMut(Option<&str>, &Value) -> Passo,
) -> Result<String, Erro> {
    let mut texto = String::new();
    for evento in sse::eventos(Box::new(fluxo)) {
        let evento = evento.map_err(|e| Erro::Rede(e.to_string()))?;
        if evento.dados == "[DONE]" {
            break;
        }
        let dados: Value = serde_json::from_str(&evento.dados) // borda-json: lê o evento vindo da API do provedor
            .map_err(|e| Erro::Formato(e.to_string()))?;
        match passo(evento.evento.as_deref(), &dados) {
            Passo::Texto(pedaco) => {
                ao_receber(&pedaco);
                texto.push_str(&pedaco);
            }
            Passo::Fim => break,
            Passo::Falha(erro) => return Err(erro),
            Passo::Nada => {}
        }
    }
    if texto.is_empty() {
        return Err(Erro::Formato("a resposta veio sem texto".into()));
    }
    Ok(texto)
}

/// Texto de um campo do JSON, ou `None` se não for texto.
pub(crate) fn texto_de<'a>(dados: &'a Value, caminho: &[&str]) -> Option<&'a str> {
    caminho
        .iter()
        .try_fold(dados, |atual, chave| atual.get(*chave))
        .and_then(Value::as_str)
}

/// Mensagem de erro que o provedor mandou no corpo, ou um texto genérico. borda-json: corpo da API (terceiro)
pub(crate) fn mensagem_de_erro(dados: &Value) -> String {
    texto_de(dados, &["error", "message"])
        .or_else(|| texto_de(dados, &["message"]))
        .unwrap_or("erro sem detalhe")
        .to_string()
}

/// Duração padrão de tempo-limite, usada quando a configuração não diz outra.
pub const TEMPO_LIMITE_PADRAO: Duration = Duration::from_secs(60);

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn segredo_nunca_aparece_no_debug() {
        let segredo = Segredo::new("sk-chave-de-teste");
        assert_eq!(format!("{segredo:?}"), "Segredo(***)");
        assert_eq!(segredo.expor(), "sk-chave-de-teste");
    }

    #[test]
    fn cada_tipo_tem_codigo_pagina_base_e_modelo() {
        for tipo in Tipo::TODOS {
            assert_eq!(Tipo::de_codigo(tipo.codigo()), Some(tipo));
            assert!(tipo.pagina_da_chave().starts_with("https://"));
            assert!(tipo.base_padrao().starts_with("https://"));
            assert_ne!(tipo.modelo_padrao(), "");
            assert_ne!(tipo.nome(), "");
        }
        assert_eq!(Tipo::de_codigo("qualquer"), None);
    }

    #[test]
    fn texto_de_e_mensagem_de_erro_leem_o_json() {
        let dados: Value = serde_json::json!({"error": {"message": "cheio"}}); // borda-json: resposta simulada da API
        assert_eq!(mensagem_de_erro(&dados), "cheio");
        let plano: Value = serde_json::json!({"message": "simples"}); // borda-json: resposta simulada da API
        assert_eq!(mensagem_de_erro(&plano), "simples");
        assert_eq!(mensagem_de_erro(&serde_json::json!({})), "erro sem detalhe"); // borda-json: resposta simulada da API
        assert_eq!(texto_de(&dados, &["error", "message"]), Some("cheio"));
        assert_eq!(texto_de(&dados, &["nao", "existe"]), None);
    }

    #[test]
    fn conversa_nova_comeca_vazia() {
        let conversa = Conversa::nova("regras");
        assert_eq!(conversa.sistema, "regras");
        assert_eq!(conversa.mensagens, Vec::<Mensagem>::new());
    }

    fn nada(_: &str) {}
    fn passo_nada(_: Option<&str>, _: &Value) -> Passo {
        Passo::Nada
    }
    fn passo_fim(_: Option<&str>, _: &Value) -> Passo {
        Passo::Fim
    }
    fn passo_texto(_: Option<&str>, _: &Value) -> Passo {
        Passo::Texto("oi".into())
    }
    fn passo_falha(_: Option<&str>, _: &Value) -> Passo {
        Passo::Falha(Erro::Servidor(529))
    }

    #[test]
    fn ler_fluxo_sem_texto_e_erro_de_formato() {
        let fluxo = transporte::fluxo_de_teste(b"data: {\"x\":1}\n\n");
        let resultado = ler_fluxo(fluxo, &mut nada, &mut passo_nada);
        assert!(matches!(resultado, Err(Erro::Formato(_))));
    }

    #[test]
    fn ler_fluxo_json_invalido_e_erro_de_formato() {
        let fluxo = transporte::fluxo_de_teste(b"data: nao-e-json\n\n");
        let resultado = ler_fluxo(fluxo, &mut nada, &mut passo_nada);
        assert!(matches!(resultado, Err(Erro::Formato(_))));
    }

    #[test]
    fn ler_fluxo_para_no_done_e_soma_os_pedacos() {
        let fluxo = transporte::fluxo_de_teste(b"data: {\"t\":\"a\"}\n\ndata: {}\n\ndata: {\"t\":\"b\"}\n\ndata: [DONE]\n\ndata: {\"t\":\"c\"}\n\n");
        let mut vistos = Vec::new();
        let texto = ler_fluxo(
            fluxo,
            &mut |p| vistos.push(p.to_string()),
            &mut |_, dados| match dados["t"].as_str() {
                Some(t) => Passo::Texto(t.to_string()),
                None => Passo::Nada,
            },
        )
        .unwrap();
        assert_eq!(texto, "ab");
        assert_eq!(vistos, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn ler_fluxo_com_texto_devolve_o_que_chegou() {
        let fluxo = transporte::fluxo_de_teste(b"data: {}\n\n");
        let texto = ler_fluxo(fluxo, &mut nada, &mut passo_texto);
        assert_eq!(texto.unwrap(), "oi");
    }

    #[test]
    fn ler_fluxo_respeita_fim_e_falha() {
        let fim = transporte::fluxo_de_teste(b"data: {}\n\ndata: {}\n\n");
        let texto = ler_fluxo(fim, &mut nada, &mut passo_fim);
        assert!(matches!(texto, Err(Erro::Formato(_))));
        let falha = transporte::fluxo_de_teste(b"data: {}\n\n");
        let erro = ler_fluxo(falha, &mut nada, &mut passo_falha);
        assert_eq!(erro, Err(Erro::Servidor(529)));
    }

    #[test]
    fn ler_fluxo_repassa_erro_de_leitura() {
        struct Quebrado;
        impl std::io::Read for Quebrado {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("queda"))
            }
        }
        let fluxo: Fluxo = std::io::BufReader::new(Box::new(Quebrado));
        let erro = ler_fluxo(fluxo, &mut nada, &mut passo_nada);
        assert!(matches!(erro, Err(Erro::Rede(_))));
    }
}
