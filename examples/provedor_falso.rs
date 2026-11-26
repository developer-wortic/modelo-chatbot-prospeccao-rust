//! Provedor falso para demonstrar o chatbot sem gastar crédito de ninguém.
//! Responde um roteiro fixo no formato de streaming da `Responses API` da `OpenAI`, e um resumo de lead quando pedido.
//! Não é um modelo de IA. Serve para ver a tela e o widget funcionando, e para as capturas do README.
//!
//! Uso: `cargo run --example provedor_falso`. Ele imprime o endereço; depois, em outro terminal:
//! `CHATBOT_BASE_URL=<endereço>/v1 CHATBOT_CHAVE=demo cargo run`.

#[allow(dead_code)]
#[path = "../tests/comum/falso.rs"]
mod falso;

use std::thread;
use std::time::Duration;

use falso::{Pedido, Resposta, resposta_openai, subir};

/// Respostas do roteiro de exemplo, uma por fala do visitante (a primeira é a saudação).
const RESPOSTAS: [&str; 5] = [
    "Olá! Sou o atendente da Loja Exemplo. Quantas pessoas trabalham na loja hoje?",
    "Obrigado! Hoje o estoque é controlado em planilha, caderno ou sistema?",
    "Entendi. Vocês já têm um valor em mente para investir nisso por mês?",
    "Perfeito. Quando gostariam de começar a usar algo novo?",
    "Ótimo, anotei. Quem decide a compra dessas ferramentas na loja?",
];

/// Resumo de lead de exemplo, com dados inventados.
const LEAD: &str = r#"{"nome":"Maria Exemplo","contato":"contato@exemplo.com.br","interesse":"controle de estoque","nota":81,"motivo":"tem prazo para começar e quer um sistema simples"}"#;

fn responder(pedido: &Pedido, _posicao: usize) -> Resposta {
    let corpo = pedido.json(); // borda-json: corpo recebido do chatbot (fio da OpenAI)
    let instrucoes = corpo["instructions"].as_str().unwrap_or("");
    if instrucoes.contains("resumo de um lead") {
        return resposta_openai(LEAD);
    }
    let falas = corpo["input"].as_array().map_or(0, |falas| {
        falas.iter().filter(|f| f["role"] == "user").count()
    });
    let indice = falas.saturating_sub(1).min(RESPOSTAS.len() - 1);
    resposta_openai(RESPOSTAS[indice])
}

fn main() {
    let falso = subir(responder);
    println!(
        "provedor falso no ar em {}/v1 (Ctrl+C para parar)",
        falso.base
    );
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
