//! Servidor falso local: finge ser um provedor. Escuta só em 127.0.0.1 e não gasta crédito de ninguém.
//! Usado pelos testes e pelo exemplo `provedor_falso`.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json}; // borda-json: corpo que o provedor falso recebe e devolve (fio do provedor)
use tiny_http::{Header, Response, Server};

/// Um pedido recebido pelo servidor falso.
#[derive(Debug, Clone)]
pub struct Pedido {
    pub metodo: String,
    pub caminho: String,
    pub cabecalhos: Vec<(String, String)>,
    pub corpo: String,
}

impl Pedido {
    /// Valor de um cabeçalho, sem diferenciar maiúsculas.
    pub fn cabecalho(&self, nome: &str) -> Option<&str> {
        self.cabecalhos
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
            .map(|(_, v)| v.as_str())
    }

    /// Corpo lido como JSON.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.corpo).expect("corpo JSON") // borda-json: corpo recebido do cliente
    }
}

/// Resposta que o falso devolve: status, corpo, cabeçalhos e um atraso opcional.
pub struct Resposta {
    pub status: u16,
    pub corpo: String,
    pub cabecalhos: Vec<(String, String)>,
    pub atraso: Duration,
}

impl Resposta {
    /// Resposta de streaming SSE: status 200 e `content-type: text/event-stream`.
    pub fn sse(eventos: &[&str]) -> Self {
        Self {
            status: 200,
            corpo: eventos.concat(),
            cabecalhos: vec![("content-type".into(), "text/event-stream".into())],
            atraso: Duration::ZERO,
        }
    }

    /// Resposta de status com corpo simples.
    pub fn com(status: u16, corpo: &str) -> Self {
        Self {
            status,
            corpo: corpo.to_string(),
            cabecalhos: vec![("content-type".into(), "application/json".into())],
            atraso: Duration::ZERO,
        }
    }

    pub fn cabecalho(mut self, nome: &str, valor: &str) -> Self {
        self.cabecalhos.push((nome.into(), valor.into()));
        self
    }

    pub fn atrasada(mut self, atraso: Duration) -> Self {
        self.atraso = atraso;
        self
    }
}

/// O falso no ar. Ao sair de escopo, para de escutar.
pub struct Falso {
    pub base: String,
    pub pedidos: Arc<Mutex<Vec<Pedido>>>,
    servidor: Arc<Server>,
}

impl Drop for Falso {
    fn drop(&mut self) {
        self.servidor.unblock();
    }
}

impl Falso {
    /// Quantos pedidos chegaram até agora.
    pub fn recebidos(&self) -> usize {
        self.pedidos.lock().unwrap().len()
    }

    /// Cópia do último pedido recebido.
    pub fn ultimo(&self) -> Pedido {
        self.pedidos
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("um pedido")
    }
}

/// Sobe o falso numa porta livre. `roteiro` recebe o pedido e a sua posição (0, 1, ...) e decide a resposta.
pub fn subir<F>(roteiro: F) -> Falso
where
    F: Fn(&Pedido, usize) -> Resposta + Send + Sync + 'static,
{
    let servidor = Arc::new(Server::http("127.0.0.1:0").expect("porta livre"));
    let porta = servidor.server_addr().to_ip().expect("endereço IP").port();
    let pedidos = Arc::new(Mutex::new(Vec::new()));
    let (no_fio, gravados) = (servidor.clone(), pedidos.clone());
    thread::spawn(move || {
        for mut pedido in no_fio.incoming_requests() {
            let mut corpo = String::new();
            let _ = pedido.as_reader().read_to_string(&mut corpo);
            let cabecalhos = pedido
                .headers()
                .iter()
                .map(|h| (h.field.as_str().to_string(), h.value.as_str().to_string()))
                .collect();
            let recebido = Pedido {
                metodo: pedido.method().to_string(),
                caminho: pedido.url().to_string(),
                cabecalhos,
                corpo,
            };
            let posicao = {
                let mut lista = gravados.lock().unwrap();
                lista.push(recebido.clone());
                lista.len() - 1
            };
            let resposta = roteiro(&recebido, posicao);
            if !resposta.atraso.is_zero() {
                thread::sleep(resposta.atraso);
            }
            let mut saida =
                Response::from_data(resposta.corpo.into_bytes()).with_status_code(resposta.status);
            for (nome, valor) in resposta.cabecalhos {
                saida = saida
                    .with_header(Header::from_bytes(nome.as_bytes(), valor.as_bytes()).unwrap());
            }
            let _ = pedido.respond(saida);
        }
    });
    Falso {
        base: format!("http://127.0.0.1:{porta}"),
        pedidos,
        servidor,
    }
}

/// Eco no formato da Responses API: repete a última fala do visitante em eventos de streaming.
pub fn eco_openai(pedido: &Pedido) -> Resposta {
    let corpo = pedido.json();
    let texto = corpo["input"]
        .as_array()
        .and_then(|falas| falas.iter().rev().find(|f| f["role"] == "user"))
        .and_then(|f| f["content"].as_str())
        .unwrap_or("")
        .to_string();
    let eventos = [
        json!({"type": "response.output_text.delta", "delta": texto, "sequence_number": 1})
            .to_string(), // borda-json: eco no fio da OpenAI
        json!({"type": "response.completed", "sequence_number": 2}).to_string(), // borda-json: eco no fio da OpenAI
    ];
    let linhas: Vec<String> = eventos
        .iter()
        .map(|e| format!("event: x\ndata: {e}\n\n"))
        .collect();
    let refs: Vec<&str> = linhas.iter().map(String::as_str).collect();
    Resposta::sse(&refs)
}

/// Resposta de streaming no formato da Responses API, com o texto dado num único pedaço.
pub fn resposta_openai(texto: &str) -> Resposta {
    let delta = json!({"type": "response.output_text.delta", "delta": texto, "sequence_number": 1}); // borda-json: resposta simulada no fio da OpenAI
    let fim = json!({"type": "response.completed", "sequence_number": 2}); // borda-json: resposta simulada no fio da OpenAI
    Resposta::sse(&[
        &format!("event: response.output_text.delta\ndata: {delta}\n\n"),
        &format!("event: response.completed\ndata: {fim}\n\n"),
    ])
}
