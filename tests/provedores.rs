//! Os cinco provedores contra o servidor falso. O formato da requisição é conferido campo a campo com a doc.
//! Nenhuma chamada sai da máquina.

mod comum;

use std::time::Duration;

use chatbot_prospeccao::erro::Erro;
use chatbot_prospeccao::provedor::{
    self, Ajustes, Conversa, Mensagem, Papel, Segredo, Tipo, Transporte,
};
use comum::falso::{Falso, Pedido, Resposta, eco_openai, subir};
use comum::fixtures::{ANTHROPIC_SSE, CHAT_SSE, GEMINI_SSE, OPENAI_SSE};
use serde_json::{Value, json}; // borda-json: corpo das requisições conferidas com a doc

const CHAVE: &str = "chave-de-teste-123";

fn rapido() -> Transporte {
    Transporte {
        tempo_limite: Duration::from_secs(5),
        reenvios: 2,
        pausa: Duration::ZERO,
    }
}

/// Endereço de teste do provedor: o falso, mais o caminho do endereço oficial (ex.: `/v1`).
fn base_falsa(tipo: Tipo, falso: &Falso) -> String {
    let oficial = tipo.base_padrao();
    let caminho = oficial
        .trim_start_matches("https://")
        .split_once('/')
        .map_or("", |(_, c)| c);
    format!("{}/{}", falso.base, caminho)
}

fn ajustes(tipo: Tipo, falso: &Falso, transporte: Transporte) -> Ajustes {
    Ajustes {
        chave: Segredo::new(CHAVE),
        base: base_falsa(tipo, falso),
        modelo: tipo.modelo_padrao().to_string(),
        transporte,
    }
}

fn conversa() -> Conversa {
    Conversa {
        sistema: "Você é o atendente da Loja Exemplo.".to_string(),
        mensagens: vec![
            Mensagem {
                papel: Papel::Visitante,
                texto: "Oi, tudo bem?".into(),
            },
            Mensagem {
                papel: Papel::Assistente,
                texto: "Tudo sim! Como posso ajudar?".into(),
            },
            Mensagem {
                papel: Papel::Visitante,
                texto: "Quero um orçamento.".into(),
            },
        ],
    }
}

fn sucesso_de(tipo: Tipo) -> Resposta {
    match tipo {
        Tipo::OpenAi => Resposta::sse(OPENAI_SSE),
        Tipo::Anthropic => Resposta::sse(ANTHROPIC_SSE),
        Tipo::Gemini => Resposta::sse(GEMINI_SSE),
        Tipo::Groq | Tipo::OpenRouter => Resposta::sse(CHAT_SSE),
    }
}

/// Texto final esperado para o fixture de sucesso de cada provedor.
fn texto_esperado(tipo: Tipo) -> &'static str {
    match tipo {
        Tipo::OpenAi => "Hi there!",
        Tipo::Anthropic => "Hello!",
        Tipo::Gemini => "Olá, tudo bem?",
        Tipo::Groq | Tipo::OpenRouter => "Olá tudo bem?",
    }
}

fn corpo_do(pedido: &Pedido) -> Value {
    pedido.json()
}

fn chaves(valor: &Value) -> Vec<String> {
    let mut nomes: Vec<String> = valor.as_object().unwrap().keys().cloned().collect();
    nomes.sort();
    nomes
}

// ---------- formato da requisição, conferido com a doc de cada provedor ----------

#[test]
fn openai_responses_manda_instructions_input_e_stream_como_na_doc() {
    let falso = subir(|_, _| sucesso_de(Tipo::OpenAi));
    let provedor = provedor::criar(Tipo::OpenAi, ajustes(Tipo::OpenAi, &falso, rapido()));
    let mut pedacos = Vec::new();
    let texto = provedor
        .enviar(&conversa(), &mut |p| pedacos.push(p.to_string()))
        .unwrap();
    assert_eq!(texto, "Hi there!");
    assert_eq!(pedacos, vec!["Hi", " there!"]);

    let pedido = falso.ultimo();
    assert_eq!(pedido.metodo, "POST");
    assert_eq!(pedido.caminho, "/v1/responses");
    assert_eq!(
        pedido.cabecalho("authorization"),
        Some("Bearer chave-de-teste-123")
    );
    assert_eq!(pedido.cabecalho("x-api-key"), None);
    assert_eq!(pedido.cabecalho("content-type"), Some("application/json"));
    let corpo = corpo_do(&pedido);
    assert_eq!(
        chaves(&corpo),
        [
            "input",
            "instructions",
            "max_output_tokens",
            "model",
            "store",
            "stream"
        ]
    );
    assert_eq!(corpo["model"], Tipo::OpenAi.modelo_padrao());
    assert_eq!(corpo["instructions"], "Você é o atendente da Loja Exemplo.");
    assert_eq!(corpo["max_output_tokens"], 600);
    assert_eq!(corpo["stream"], true);
    assert_eq!(corpo["store"], false);
    let entrada = corpo["input"].as_array().unwrap();
    assert_eq!(entrada.len(), 3);
    assert_eq!(
        entrada[0],
        json!({"role": "user", "content": "Oi, tudo bem?"})
    ); // borda-json: conferido com a doc
    assert_eq!(entrada[1]["role"], "assistant");
    assert_eq!(entrada[2]["content"], "Quero um orçamento.");
}

#[test]
fn anthropic_messages_manda_x_api_key_versao_e_system_como_na_doc() {
    let falso = subir(|_, _| sucesso_de(Tipo::Anthropic));
    let provedor = provedor::criar(Tipo::Anthropic, ajustes(Tipo::Anthropic, &falso, rapido()));
    let mut pedacos = Vec::new();
    let texto = provedor
        .enviar(&conversa(), &mut |p| pedacos.push(p.to_string()))
        .unwrap();
    assert_eq!(texto, "Hello!");
    assert_eq!(pedacos, vec!["Hello", "!"]);

    let pedido = falso.ultimo();
    assert_eq!(pedido.caminho, "/v1/messages");
    assert_eq!(pedido.cabecalho("x-api-key"), Some("chave-de-teste-123"));
    assert_eq!(pedido.cabecalho("anthropic-version"), Some("2026-10-21"));
    assert_eq!(pedido.cabecalho("authorization"), None);
    let corpo = corpo_do(&pedido);
    assert_eq!(
        chaves(&corpo),
        ["max_tokens", "messages", "model", "stream", "system"]
    );
    assert_eq!(corpo["model"], "claude-haiku-5-5");
    assert_eq!(corpo["max_tokens"], 600);
    assert_eq!(corpo["system"], "Você é o atendente da Loja Exemplo.");
    assert_eq!(corpo["stream"], true);
    assert_eq!(
        corpo["messages"][0],
        json!({"role": "user", "content": "Oi, tudo bem?"})
    ); // borda-json: conferido com a doc
    assert_eq!(corpo["messages"][1]["role"], "assistant");
}

#[test]
fn gemini_generate_content_manda_system_instruction_e_chave_no_cabecalho() {
    let falso = subir(|_, _| sucesso_de(Tipo::Gemini));
    let provedor = provedor::criar(Tipo::Gemini, ajustes(Tipo::Gemini, &falso, rapido()));
    let mut pedacos = Vec::new();
    let texto = provedor
        .enviar(&conversa(), &mut |p| pedacos.push(p.to_string()))
        .unwrap();
    assert_eq!(texto, "Olá, tudo bem?");
    assert_eq!(pedacos.len(), 2);

    let pedido = falso.ultimo();
    assert_eq!(
        pedido.caminho,
        "/v1beta/models/gemini-3.5-flash-lite:streamGenerateContent?alt=sse"
    );
    assert!(!pedido.caminho.contains(CHAVE), "a chave não vai na URL");
    assert_eq!(
        pedido.cabecalho("x-goog-api-key"),
        Some("chave-de-teste-123")
    );
    assert_eq!(pedido.cabecalho("authorization"), None);
    let corpo = corpo_do(&pedido);
    assert_eq!(
        chaves(&corpo),
        ["contents", "generationConfig", "systemInstruction"]
    );
    assert_eq!(
        corpo["systemInstruction"]["parts"][0]["text"],
        "Você é o atendente da Loja Exemplo."
    );
    assert_eq!(corpo["generationConfig"]["maxOutputTokens"], 600);
    let conteudos = corpo["contents"].as_array().unwrap();
    assert_eq!(conteudos.len(), 3);
    assert_eq!(
        conteudos[0],
        json!({"role": "user", "parts": [{"text": "Oi, tudo bem?"}]})
    ); // borda-json: conferido com a doc
    assert_eq!(conteudos[1]["role"], "model");
    assert_eq!(conteudos[2]["parts"][0]["text"], "Quero um orçamento.");
}

#[test]
fn groq_chat_completions_usa_max_completion_tokens() {
    let falso = subir(|_, _| sucesso_de(Tipo::Groq));
    let provedor = provedor::criar(Tipo::Groq, ajustes(Tipo::Groq, &falso, rapido()));
    let mut pedacos = Vec::new();
    let texto = provedor
        .enviar(&conversa(), &mut |p| pedacos.push(p.to_string()))
        .unwrap();
    assert_eq!(texto, "Olá tudo bem?");
    assert_eq!(pedacos, vec!["Olá", " tudo bem?"]);

    let pedido = falso.ultimo();
    assert_eq!(pedido.caminho, "/openai/v1/chat/completions");
    assert_eq!(
        pedido.cabecalho("authorization"),
        Some("Bearer chave-de-teste-123")
    );
    let corpo = corpo_do(&pedido);
    assert_eq!(
        chaves(&corpo),
        ["max_completion_tokens", "messages", "model", "stream"]
    );
    assert_eq!(corpo["model"], "llama-3.1-8b-instant");
    assert_eq!(corpo["max_completion_tokens"], 600);
    assert_eq!(corpo["stream"], true);
    let falas = corpo["messages"].as_array().unwrap();
    assert_eq!(
        falas[0],
        json!({"role": "system", "content": "Você é o atendente da Loja Exemplo."})
    ); // borda-json: conferido com a doc
    assert_eq!(
        falas[1],
        json!({"role": "user", "content": "Oi, tudo bem?"})
    ); // borda-json: conferido com a doc
    assert_eq!(falas.len(), 4);
}

#[test]
fn openrouter_chat_completions_manda_titulo_do_app_e_max_tokens() {
    let falso = subir(|_, _| sucesso_de(Tipo::OpenRouter));
    let provedor = provedor::criar(
        Tipo::OpenRouter,
        ajustes(Tipo::OpenRouter, &falso, rapido()),
    );
    let texto = provedor.enviar(&conversa(), &mut |_| {}).unwrap();
    assert_eq!(texto, "Olá tudo bem?");

    let pedido = falso.ultimo();
    assert_eq!(pedido.caminho, "/api/v1/chat/completions");
    assert_eq!(
        pedido.cabecalho("authorization"),
        Some("Bearer chave-de-teste-123")
    );
    assert!(pedido.cabecalho("x-openrouter-title").is_some());
    let corpo = corpo_do(&pedido);
    assert_eq!(corpo["max_tokens"], 600);
    assert!(corpo.get("max_completion_tokens").is_none());
    assert_eq!(corpo["model"], "anthropic/claude-haiku-5.5");
}

#[test]
fn cada_provedor_recebe_a_mesma_conversa_e_devolve_texto() {
    for tipo in Tipo::TODOS {
        let falso = subir(move |_, _| sucesso_de(tipo));
        let provedor = provedor::criar(tipo, ajustes(tipo, &falso, rapido()));
        let texto = provedor.enviar(&conversa(), &mut |_| {}).unwrap();
        assert_eq!(texto, texto_esperado(tipo), "provedor {}", tipo.codigo());
        let corpo = falso.ultimo().corpo;
        assert!(
            corpo.contains("Quero um orçamento."),
            "a fala do visitante chega ao {}",
            tipo.codigo()
        );
    }
}

// ---------- erros de HTTP, iguais para os cinco ----------

#[test]
fn chave_recusada_e_401_ou_403_e_nao_e_reenviada() {
    for tipo in Tipo::TODOS {
        for codigo in [401, 403] {
            let falso =
                subir(move |_, _| Resposta::com(codigo, "{\"error\":{\"message\":\"nope\"}}"));
            let erro = provedor::criar(tipo, ajustes(tipo, &falso, rapido()))
                .enviar(&conversa(), &mut |_| {})
                .unwrap_err();
            assert_eq!(erro, Erro::ChaveRecusada, "{} {codigo}", tipo.codigo());
            assert_eq!(falso.recebidos(), 1, "401/403 não reenvia");
            assert!(!erro.to_string().contains(CHAVE));
        }
    }
}

#[test]
fn outro_4xx_vira_recusado_com_o_status() {
    let falso = subir(|_, _| Resposta::com(400, "{}"));
    let erro = provedor::criar(Tipo::Groq, ajustes(Tipo::Groq, &falso, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert_eq!(erro, Erro::Recusado(400));
    assert_eq!(falso.recebidos(), 1);
}

#[test]
fn limite_de_taxa_com_retry_after_curto_reenvia_e_depois_responde() {
    for tipo in Tipo::TODOS {
        let falso = subir(move |_, n| {
            if n == 0 {
                Resposta::com(429, "{}").cabecalho("retry-after", "0")
            } else {
                sucesso_de(tipo)
            }
        });
        let texto = provedor::criar(tipo, ajustes(tipo, &falso, rapido()))
            .enviar(&conversa(), &mut |_| {})
            .unwrap();
        assert_eq!(texto, texto_esperado(tipo));
        assert_eq!(falso.recebidos(), 2, "{} reenviou uma vez", tipo.codigo());
    }
}

#[test]
fn limite_de_taxa_com_retry_after_longo_nao_espera_e_devolve_o_tempo() {
    let falso = subir(|_, _| Resposta::com(429, "{}").cabecalho("retry-after", "60"));
    let erro = provedor::criar(Tipo::OpenAi, ajustes(Tipo::OpenAi, &falso, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert_eq!(erro, Erro::LimiteDeTaxa { espere: Some(60) });
    assert_eq!(falso.recebidos(), 1);
}

#[test]
fn limite_de_taxa_sem_retry_after_nao_reenvia() {
    let falso = subir(|_, _| Resposta::com(429, "{}"));
    let erro = provedor::criar(Tipo::Anthropic, ajustes(Tipo::Anthropic, &falso, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert_eq!(erro, Erro::LimiteDeTaxa { espere: None });
    assert_eq!(falso.recebidos(), 1);
}

#[test]
fn falha_5xx_reenvia_com_limite_e_depois_desiste() {
    for tipo in Tipo::TODOS {
        let falso = subir(|_, _| Resposta::com(503, "{}"));
        let erro = provedor::criar(tipo, ajustes(tipo, &falso, rapido()))
            .enviar(&conversa(), &mut |_| {})
            .unwrap_err();
        assert_eq!(erro, Erro::Servidor(503), "{}", tipo.codigo());
        assert_eq!(falso.recebidos(), 3, "uma tentativa e dois reenvios");
    }
}

#[test]
fn falha_5xx_que_some_no_reenvio_vira_resposta_normal() {
    let falso = subir(|_, n| {
        if n == 0 {
            Resposta::com(500, "{}")
        } else {
            sucesso_de(Tipo::OpenAi)
        }
    });
    let texto = provedor::criar(Tipo::OpenAi, ajustes(Tipo::OpenAi, &falso, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap();
    assert_eq!(texto, "Hi there!");
    assert_eq!(falso.recebidos(), 2);
}

#[test]
fn tempo_limite_estourado_vira_tempo_esgotado() {
    let lento = Transporte {
        tempo_limite: Duration::from_millis(150),
        reenvios: 0,
        pausa: Duration::ZERO,
    };
    let falso = subir(|_, _| sucesso_de(Tipo::OpenAi).atrasada(Duration::from_millis(1500)));
    let erro = provedor::criar(Tipo::OpenAi, ajustes(Tipo::OpenAi, &falso, lento))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert_eq!(erro, Erro::TempoEsgotado);
}

#[test]
fn servidor_fora_do_ar_vira_erro_de_rede_sem_a_chave() {
    let morto = Ajustes {
        chave: Segredo::new(CHAVE),
        base: "http://127.0.0.1:1/v1".into(),
        modelo: "m".into(),
        transporte: rapido(),
    };
    let erro = provedor::criar(Tipo::OpenAi, morto)
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert!(matches!(erro, Erro::Rede(_)), "{erro:?}");
    assert!(!erro.to_string().contains(CHAVE));
}

// ---------- erros no meio do streaming e formatos ruins ----------

#[test]
fn openai_erro_no_meio_do_fluxo_vira_formato() {
    let falso = subir(|_, _| {
        Resposta::sse(&["event: error\ndata: {\"type\":\"error\",\"message\":\"cota acabou\"}\n\n"])
    });
    let erro = provedor::criar(Tipo::OpenAi, ajustes(Tipo::OpenAi, &falso, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert_eq!(erro, Erro::Formato("cota acabou".into()));
}

#[test]
fn openai_response_failed_e_evento_sem_texto_sao_tratados() {
    let falso = subir(|_, _| {
        Resposta::sse(&[
            "data: {\"type\":\"response.created\"}\n\n",
            "data: {\"type\":\"response.failed\"}\n\n",
        ])
    });
    let erro = provedor::criar(Tipo::OpenAi, ajustes(Tipo::OpenAi, &falso, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert_eq!(erro, Erro::Formato("erro sem detalhe".into()));
}

#[test]
fn anthropic_erros_do_fluxo_viram_o_erro_certo() {
    let casos = [
        ("overloaded_error", Erro::Servidor(529)),
        ("rate_limit_error", Erro::LimiteDeTaxa { espere: None }),
        ("authentication_error", Erro::ChaveRecusada),
        ("permission_error", Erro::ChaveRecusada),
        ("invalid_request_error", Erro::Formato("pedido ruim".into())),
    ];
    for (tipo_do_erro, esperado) in casos {
        let evento = format!(
            "event: error\ndata: {{\"type\": \"error\", \"error\": {{\"type\": \"{tipo_do_erro}\", \"message\": \"pedido ruim\"}}}}\n\n"
        );
        let falso = subir(move |_, _| Resposta::sse(&[evento.as_str()]));
        let erro = provedor::criar(Tipo::Anthropic, ajustes(Tipo::Anthropic, &falso, rapido()))
            .enviar(&conversa(), &mut |_| {})
            .unwrap_err();
        assert_eq!(erro, esperado, "{tipo_do_erro}");
    }
}

#[test]
fn anthropic_ignora_eventos_que_nao_sao_texto() {
    let falso = subir(|_, _| {
        Resposta::sse(&[
            "data: {\"type\": \"content_block_delta\", \"index\": 1, \"delta\": {\"type\": \"input_json_delta\", \"partial_json\": \"{}\"}}\n\n",
            "data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"ok\"}}\n\n",
            "data: {\"type\": \"message_stop\"}\n\n",
            "data: {\"type\": \"depois_do_fim\"}\n\n",
        ])
    });
    let texto = provedor::criar(Tipo::Anthropic, ajustes(Tipo::Anthropic, &falso, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap();
    assert_eq!(texto, "ok");
}

#[test]
fn gemini_bloqueio_do_prompt_e_pedaco_sem_texto() {
    let bloqueado = subir(|_, _| {
        Resposta::sse(&["data: {\"promptFeedback\": {\"blockReason\": \"SAFETY\"}}\n\n"])
    });
    let erro = provedor::criar(Tipo::Gemini, ajustes(Tipo::Gemini, &bloqueado, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert_eq!(erro, Erro::Formato("prompt bloqueado: SAFETY".into()));

    let com_pedaco_vazio = subir(|_, _| {
        Resposta::sse(&[
            "data: {\"usageMetadata\": {\"totalTokenCount\": 3}}\n\n",
            "data: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"fim\"}]}}]}\n\n",
        ])
    });
    let texto = provedor::criar(
        Tipo::Gemini,
        ajustes(Tipo::Gemini, &com_pedaco_vazio, rapido()),
    )
    .enviar(&conversa(), &mut |_| {})
    .unwrap();
    assert_eq!(texto, "fim");
}

#[test]
fn chat_completions_erro_no_fluxo_e_pedaco_sem_conteudo() {
    for tipo in [Tipo::Groq, Tipo::OpenRouter] {
        let erro_no_fluxo = subir(|_, _| {
            Resposta::sse(&[
                "data: {\"error\": {\"code\": 502, \"message\": \"provedor caiu\"}}\n\n",
            ])
        });
        let erro = provedor::criar(tipo, ajustes(tipo, &erro_no_fluxo, rapido()))
            .enviar(&conversa(), &mut |_| {})
            .unwrap_err();
        assert_eq!(
            erro,
            Erro::Formato("provedor caiu".into()),
            "{}",
            tipo.codigo()
        );

        let usage = subir(|_, _| {
            Resposta::sse(&[
                "data: {\"choices\": []}\n\n",
                "data: {\"choices\": [{\"delta\": {\"content\": \"\"}}]}\n\n",
                "data: {\"choices\": [{\"delta\": {\"content\": \"sim\"}}]}\n\n",
            ])
        });
        let texto = provedor::criar(tipo, ajustes(tipo, &usage, rapido()))
            .enviar(&conversa(), &mut |_| {})
            .unwrap();
        assert_eq!(texto, "sim", "{}", tipo.codigo());
    }
}

#[test]
fn fluxo_sem_texto_ou_com_lixo_vira_formato() {
    let vazio = subir(|_, _| Resposta::sse(&[]));
    let erro = provedor::criar(Tipo::OpenAi, ajustes(Tipo::OpenAi, &vazio, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert!(matches!(erro, Erro::Formato(_)));

    let lixo = subir(|_, _| Resposta::sse(&["data: isto não é json\n\n"]));
    let erro = provedor::criar(Tipo::Anthropic, ajustes(Tipo::Anthropic, &lixo, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap_err();
    assert!(matches!(erro, Erro::Formato(_)));
}

#[test]
fn eco_do_falso_openai_repete_a_ultima_fala_do_visitante() {
    let falso = subir(|pedido, _| eco_openai(pedido));
    let texto = provedor::criar(Tipo::OpenAi, ajustes(Tipo::OpenAi, &falso, rapido()))
        .enviar(&conversa(), &mut |_| {})
        .unwrap();
    assert_eq!(texto, "Quero um orçamento.");
}

#[test]
fn chave_nao_aparece_no_debug_do_provedor_nem_do_erro() {
    let segredo = Segredo::new(CHAVE);
    assert!(!format!("{segredo:?}").contains(CHAVE));
    let falso = subir(|_, _| Resposta::com(401, "{}"));
    let ajustes = ajustes(Tipo::Groq, &falso, rapido());
    assert!(!format!("{ajustes:?}").contains(CHAVE));
}
