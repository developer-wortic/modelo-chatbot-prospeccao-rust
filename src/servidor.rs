//! Servidor HTTP do modelo. Um binário só, com a tela embutida.
//! Rotas públicas (o chat e o widget): `/widget.js`, `POST /api/chat`, `POST /api/encerrar`.
//! Rotas locais (a administração, só a partir desta máquina): `/`, `/demo`, `/api/config`, `/api/chave`,
//! `/api/testar`, `/api/roteiro` e `/api/leads`. A chave nunca volta na resposta.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::Path;
use std::process::ExitCode;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json}; // borda-json: a tela (navegador) e o widget falam JSON com este servidor
use tiny_http::{Header, Method, Request, Response, Server};

use crate::atendimento::{Atendimento, ErroChat, Limites, Sessoes};
use crate::configuracao::Configuracao;
use crate::erro::Erro;
use crate::lead::Leads;
use crate::provedor::{self, Ajustes, Conversa, Mensagem, Papel, Segredo, Tipo, Transporte};
use crate::roteiro::{self, Roteiro};

const ADMINISTRACAO: &str = include_str!("../static/admin.html");
const DEMONSTRACAO: &str = include_str!("../static/demo.html");
const WIDGET: &str = include_str!("../static/widget.js");

/// Tamanho máximo de um corpo de requisição (64 KB). Quem manda mais leva 400.
const LIMITE_CORPO: u64 = 64 * 1024;

/// Estado compartilhado entre as requisições.
pub struct Estado {
    pub configuracao: RwLock<Configuracao>,
    pub roteiro: RwLock<Roteiro>,
    pub sessoes: Sessoes,
    pub leads: Leads,
    pub transporte: Transporte,
    pub limites: Limites,
}

impl Estado {
    /// Estado novo, com os padrões de transporte e de limites.
    #[must_use]
    pub fn novo(configuracao: Configuracao, roteiro: Roteiro) -> Self {
        Self {
            configuracao: RwLock::new(configuracao),
            roteiro: RwLock::new(roteiro),
            sessoes: Sessoes::default(),
            leads: Leads::default(),
            transporte: Transporte::default(),
            limites: Limites::default(),
        }
    }
}

/// Resposta montada antes de sair pelo `tiny_http`.
struct Resposta {
    status: u16,
    tipo: &'static str,
    corpo: Vec<u8>,
    extras: Vec<(&'static str, String)>,
}

impl Resposta {
    fn texto(status: u16, tipo: &'static str, corpo: &str) -> Self {
        Self {
            status,
            tipo,
            corpo: corpo.as_bytes().to_vec(),
            extras: Vec::new(),
        }
    }

    fn json(status: u16, valor: &Value) -> Self {
        // borda-json: resposta em JSON para a tela e o widget
        Self::texto(
            status,
            "application/json; charset=utf-8",
            &valor.to_string(),
        )
    }

    fn erro(status: u16, codigo: &str, mensagem: &str) -> Self {
        Self::json(status, &json!({"erro": codigo, "mensagem": mensagem})) // borda-json: erro em JSON
    }

    fn com(mut self, nome: &'static str, valor: impl Into<String>) -> Self {
        self.extras.push((nome, valor.into()));
        self
    }

    fn vazia(status: u16) -> Self {
        Self::texto(status, "text/plain; charset=utf-8", "")
    }

    fn para_tiny(self) -> Response<Cursor<Vec<u8>>> {
        let mut resposta = Response::from_data(self.corpo)
            .with_status_code(self.status)
            .with_header(cabecalho("content-type", self.tipo));
        for (nome, valor) in &self.extras {
            resposta = resposta.with_header(cabecalho(nome, valor));
        }
        resposta
    }
}

fn cabecalho(nome: &str, valor: &str) -> Header {
    Header::from_bytes(nome.as_bytes(), valor.as_bytes()).expect("cabeçalho válido")
}

/// Servidor já com a porta aberta, o estado montado e os avisos de partida para o log.
pub struct Preparado {
    pub servidor: Arc<Server>,
    pub estado: Arc<Estado>,
    pub avisos: Vec<String>,
}

/// Lê o roteiro (ou o de exemplo, se o arquivo não existir), abre a porta e monta os avisos. Não serve ainda.
///
/// # Errors
///
/// Devolve a mensagem quando o roteiro não lê ou a porta não abre.
pub fn preparar(configuracao: Configuracao) -> Result<Preparado, String> {
    let (roteiro, sem_arquivo) = carregar_roteiro(&configuracao.roteiro)?;
    let endereco = format!("{}:{}", configuracao.endereco, configuracao.porta);
    let servidor = Server::http(&endereco)
        .map_err(|e| format!("não consegui abrir a porta {}: {e}", configuracao.porta))?;
    // A porta real, para o aviso mostrar o endereço de verdade mesmo quando PORTA=0 (o sistema escolhe).
    let porta_real = servidor
        .server_addr()
        .to_ip()
        .map_or(configuracao.porta, |a| a.port());
    let url = format!("http://{}:{porta_real}/", configuracao.endereco);
    let mut avisos = vec![format!("chatbot no ar em {url}")];
    if let Some(caminho) = sem_arquivo {
        avisos.push(format!(
            "AVISO: {caminho} não existe; usando o roteiro de exemplo."
        ));
    }
    if configuracao.chave.is_none() {
        avisos.push(format!(
            "AVISO: sem chave. Abra {url} para escolher o provedor e colar a sua chave."
        ));
    }
    if !configuracao.endereco.starts_with("127.") {
        avisos.push("AVISO: o chat está aberto na rede. A administração só responde nesta máquina; use HTTPS num proxy.".to_string());
    }
    Ok(Preparado {
        servidor: Arc::new(servidor),
        estado: Arc::new(Estado::novo(configuracao, roteiro)),
        avisos,
    })
}

/// Imprime os avisos e serve até o servidor ser parado.
pub fn iniciar(preparado: Preparado) -> ExitCode {
    let Preparado {
        servidor,
        estado,
        avisos,
    } = preparado;
    for aviso in &avisos {
        println!("{aviso}");
    }
    servir(&servidor, &estado);
    ExitCode::SUCCESS
}

/// Ponto de entrada do binário: configuração, preparação e serviço. Erros vão para o stderr.
pub fn principal<I>(ambiente: I) -> ExitCode
where
    I: IntoIterator<Item = (String, String)>,
{
    let ambiente: HashMap<String, String> = ambiente.into_iter().collect();
    Configuracao::ler(&ambiente).and_then(preparar).map_or_else(
        |erro| {
            eprintln!("{erro}");
            ExitCode::FAILURE
        },
        iniciar,
    )
}

/// Roteiro do caminho dado. Sem arquivo, usa o de exemplo e devolve o caminho para o aviso.
fn carregar_roteiro(caminho: &Path) -> Result<(Roteiro, Option<String>), String> {
    match fs::read_to_string(caminho) {
        Ok(texto) => Ok((Roteiro::ler(&texto)?, None)),
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => Ok((
            Roteiro::ler(roteiro::EXEMPLO).expect("o roteiro de exemplo é válido"),
            Some(caminho.display().to_string()),
        )),
        Err(erro) => Err(format!("não consegui ler o roteiro: {erro}")),
    }
}

/// Atende as requisições, uma thread por pedido. Volta quando o servidor é parado.
pub fn servir(servidor: &Server, estado: &Arc<Estado>) {
    for pedido in servidor.incoming_requests() {
        let estado = Arc::clone(estado);
        thread::spawn(move || atender(pedido, &estado));
    }
}

fn atender(mut pedido: Request, estado: &Estado) {
    let resposta = rotear(&mut pedido, estado);
    let _ = pedido.respond(resposta.para_tiny());
}

fn rotear(pedido: &mut Request, estado: &Estado) -> Resposta {
    let alvo = pedido.url().split('?').next().unwrap_or("/").to_string();
    let metodo = pedido.method().clone();
    if matches!(alvo.as_str(), "/widget.js" | "/api/chat" | "/api/encerrar") {
        let origem = valor_do_cabecalho(pedido, "origin");
        return rota_publica(pedido, estado, &metodo, &alvo, origem.as_deref());
    }
    if !e_local(pedido) {
        return Resposta::erro(
            403,
            "so_local",
            "A administração só responde a partir deste computador.",
        );
    }
    rota_local(pedido, estado, &metodo, &alvo)
}

fn rota_publica(
    pedido: &mut Request,
    estado: &Estado,
    metodo: &Method,
    alvo: &str,
    origem: Option<&str>,
) -> Resposta {
    match (metodo, alvo) {
        (Method::Get, "/widget.js") => {
            Resposta::texto(200, "text/javascript; charset=utf-8", WIDGET)
        }
        (Method::Options, _) => {
            let previa = Resposta::vazia(204)
                .com("access-control-allow-methods", "POST, OPTIONS")
                .com("access-control-allow-headers", "content-type");
            com_cors(previa, origem, estado)
        }
        (Method::Post, "/api/chat") => com_cors(chat(pedido, estado), origem, estado),
        (Method::Post, "/api/encerrar") => com_cors(encerrar(pedido, estado), origem, estado),
        _ => Resposta::erro(405, "metodo", "método não aceito nesta rota"),
    }
}

fn rota_local(pedido: &mut Request, estado: &Estado, metodo: &Method, alvo: &str) -> Resposta {
    match (metodo, alvo) {
        (Method::Get, "/") => Resposta::texto(200, "text/html; charset=utf-8", ADMINISTRACAO),
        (Method::Get, "/demo") => Resposta::texto(200, "text/html; charset=utf-8", DEMONSTRACAO),
        (Method::Get, "/api/config") => configuracao_atual(estado),
        (Method::Post, "/api/chave") => salvar_chave(pedido, estado),
        (Method::Post, "/api/testar") => testar(pedido, estado),
        (Method::Get, "/api/roteiro") => roteiro_atual(estado),
        (Method::Post | Method::Put, "/api/roteiro") => salvar_roteiro(pedido, estado),
        (Method::Get, "/api/leads") => {
            Resposta::json(200, &json!({"leads": estado.leads.listar()}))
        } // borda-json
        _ => Resposta::erro(404, "nao_encontrado", "esta rota não existe"),
    }
}

/// Só a própria máquina chega à administração, e nunca por proxy (que aparece como `127.0.0.1`).
fn e_local(pedido: &Request) -> bool {
    let vem_de_proxy = valor_do_cabecalho(pedido, "x-forwarded-for").is_some()
        || valor_do_cabecalho(pedido, "forwarded").is_some();
    pedido
        .remote_addr()
        .is_some_and(|endereco| endereco.ip().is_loopback())
        && !vem_de_proxy
}

fn valor_do_cabecalho(pedido: &Request, nome: &'static str) -> Option<String> {
    pedido
        .headers()
        .iter()
        .find(|h| h.field.equiv(nome))
        .map(|h| h.value.as_str().to_string())
}

/// Libera a origem só se ela estiver na lista `CHATBOT_ORIGENS`. Sem lista, não há CORS.
fn com_cors(resposta: Resposta, origem: Option<&str>, estado: &Estado) -> Resposta {
    let Some(origem) = origem else {
        return resposta;
    };
    let permitida = estado
        .configuracao
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .origens
        .iter()
        .any(|o| o == origem);
    if permitida {
        resposta
            .com("access-control-allow-origin", origem)
            .com("vary", "origin")
    } else {
        resposta
    }
}

/// Corpo JSON da requisição, limitado a 64 KB.
fn corpo_json(pedido: &mut Request) -> Result<Value, Resposta> {
    let mut texto = String::new();
    pedido
        .as_reader()
        .take(LIMITE_CORPO)
        .read_to_string(&mut texto)
        .map_err(|_| {
            Resposta::erro(
                400,
                "corpo_ilegivel",
                "não consegui ler o corpo da requisição",
            )
        })?;
    serde_json::from_str(&texto) // borda-json: corpo que a tela ou o widget enviam
        .map_err(|_| Resposta::erro(400, "json_invalido", "o corpo precisa ser JSON"))
}

fn texto_do_corpo<'a>(corpo: &'a Value, campo: &str) -> &'a str {
    corpo.get(campo).and_then(Value::as_str).unwrap_or("")
}

fn configuracao_atual(estado: &Estado) -> Resposta {
    let c = estado
        .configuracao
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let provedores: Vec<Value> = Tipo::TODOS
        .iter()
        .map(|t| {
            json!({ // borda-json: catálogo de provedores para a tela
                "codigo": t.codigo(),
                "nome": t.nome(),
                "pagina_da_chave": t.pagina_da_chave(),
                "modelo_padrao": t.modelo_padrao(),
            })
        })
        .collect();
    Resposta::json(
        200,
        &json!({ // borda-json: estado público da configuração, sem a chave
            "provedor": c.provedor.codigo(),
            "modelo": c.modelo,
            "tem_chave": c.chave.is_some(),
            "provedores": provedores,
        }),
    )
}

fn salvar_chave(pedido: &mut Request, estado: &Estado) -> Resposta {
    let corpo = match corpo_json(pedido) {
        Ok(corpo) => corpo,
        Err(resposta) => return resposta,
    };
    let Some(tipo) = corpo
        .get("provedor")
        .and_then(Value::as_str)
        .and_then(Tipo::de_codigo)
    else {
        return Resposta::erro(400, "provedor_invalido", "escolha um dos cinco provedores");
    };
    let chave = texto_do_corpo(&corpo, "chave").trim().to_string();
    if chave.is_empty() {
        return Resposta::erro(400, "chave_vazia", "cole a chave antes de salvar");
    }
    let modelo = texto_do_corpo(&corpo, "modelo").trim();
    let modelo = if modelo.is_empty() {
        tipo.modelo_padrao().to_string()
    } else {
        modelo.to_string()
    };
    let dados = estado
        .configuracao
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .dados
        .clone();
    if let Err(motivo) = Configuracao::salvar_escolha(&dados, tipo, &modelo, &chave) {
        return Resposta::erro(500, "gravacao", &motivo);
    }
    let mut c = estado
        .configuracao
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    c.provedor = tipo;
    c.modelo = modelo;
    c.chave = Some(Segredo::new(chave));
    Resposta::json(200, &json!({"ok": true})) // borda-json
}

/// Uma chamada mínima ao provedor, com a chave da tela (ou a salva). Nada é gravado aqui.
fn testar(pedido: &mut Request, estado: &Estado) -> Resposta {
    let corpo = match corpo_json(pedido) {
        Ok(corpo) => corpo,
        Err(resposta) => return resposta,
    };
    let (tipo, modelo, base, chave) = {
        let c = estado
            .configuracao
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tipo = corpo
            .get("provedor")
            .and_then(Value::as_str)
            .and_then(Tipo::de_codigo)
            .unwrap_or(c.provedor);
        let digitada = texto_do_corpo(&corpo, "chave").trim().to_string();
        let chave = if digitada.is_empty() {
            c.chave.as_ref().map(|s| s.expor().to_string())
        } else {
            Some(digitada)
        };
        let modelo = texto_do_corpo(&corpo, "modelo").trim().to_string();
        let modelo = if modelo.is_empty() {
            tipo.modelo_padrao().to_string()
        } else {
            modelo
        };
        let base = c
            .base
            .clone()
            .unwrap_or_else(|| tipo.base_padrao().to_string());
        (tipo, modelo, base, chave)
    };
    let Some(chave) = chave else {
        return Resposta::erro(400, "sem_chave", "cole a chave antes de testar");
    };
    let ajustes = Ajustes {
        chave: Segredo::new(chave),
        base,
        modelo,
        transporte: estado.transporte,
    };
    let provedor = provedor::criar(tipo, ajustes);
    let conversa = Conversa {
        sistema: "Responda apenas com a palavra: ok".to_string(),
        mensagens: vec![Mensagem {
            papel: Papel::Visitante,
            texto: "Teste de conexão.".to_string(),
        }],
    };
    match provedor.enviar(&conversa, &mut |_| {}) {
        Ok(texto) => Resposta::json(
            200,
            &json!({"ok": true, "resposta": texto.chars().take(80).collect::<String>()}),
        ), // borda-json
        Err(erro) => Resposta::json(200, &json!({"ok": false, "mensagem": erro.to_string()})), // borda-json
    }
}

fn roteiro_atual(estado: &Estado) -> Resposta {
    let caminho = estado
        .configuracao
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .roteiro
        .clone();
    match fs::read_to_string(&caminho) {
        Ok(texto) => Resposta::json(200, &json!({"toml": texto})), // borda-json
        Err(erro) if erro.kind() == io::ErrorKind::NotFound => {
            Resposta::json(200, &json!({"toml": roteiro::EXEMPLO})) // borda-json
        }
        Err(erro) => Resposta::erro(500, "leitura", &erro.to_string()),
    }
}

fn salvar_roteiro(pedido: &mut Request, estado: &Estado) -> Resposta {
    let corpo = match corpo_json(pedido) {
        Ok(corpo) => corpo,
        Err(resposta) => return resposta,
    };
    let texto = texto_do_corpo(&corpo, "toml");
    let novo = match Roteiro::ler(texto) {
        Ok(novo) => novo,
        Err(motivo) => return Resposta::erro(400, "roteiro_invalido", &motivo),
    };
    let caminho = estado
        .configuracao
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .roteiro
        .clone();
    if let Err(erro) = fs::write(&caminho, texto) {
        return Resposta::erro(500, "gravacao", &erro.to_string());
    }
    *estado
        .roteiro
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = novo;
    Resposta::json(200, &json!({"ok": true})) // borda-json
}

/// Monta o atendimento com a configuração atual. Sem chave, responde 503 e não chama nada.
fn montar(estado: &Estado) -> Result<Atendimento, Resposta> {
    let c = estado
        .configuracao
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let chave = c.chave.clone().ok_or_else(|| {
        Resposta::erro(
            503,
            "sem_chave",
            "O atendimento ainda não está configurado.",
        )
    })?;
    let ajustes = Ajustes {
        chave: chave.clone(),
        base: c
            .base
            .clone()
            .unwrap_or(c.provedor.base_padrao().to_string()),
        modelo: c.modelo.clone(),
        transporte: estado.transporte,
    };
    let provedor = provedor::criar(c.provedor, ajustes);
    let roteiro = estado
        .roteiro
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    Ok(Atendimento::new(
        roteiro,
        chave.expor(),
        provedor,
        estado.limites,
    ))
}

fn chat(pedido: &mut Request, estado: &Estado) -> Resposta {
    let corpo = match corpo_json(pedido) {
        Ok(corpo) => corpo,
        Err(resposta) => return resposta,
    };
    let atendimento = match montar(estado) {
        Ok(atendimento) => atendimento,
        Err(resposta) => return resposta,
    };
    let sessao = corpo.get("sessao").and_then(Value::as_str);
    match atendimento.responder(&estado.sessoes, sessao, texto_do_corpo(&corpo, "mensagem")) {
        Ok(resposta) => Resposta::json(
            200,
            &json!({"sessao": resposta.sessao, "resposta": resposta.texto, "recusada": resposta.recusada}), // borda-json
        ),
        Err(erro) => erro_do_chat(&erro),
    }
}

fn encerrar(pedido: &mut Request, estado: &Estado) -> Resposta {
    let corpo = match corpo_json(pedido) {
        Ok(corpo) => corpo,
        Err(resposta) => return resposta,
    };
    let atendimento = match montar(estado) {
        Ok(atendimento) => atendimento,
        Err(resposta) => return resposta,
    };
    let agora = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    match atendimento.resumir(&estado.sessoes, texto_do_corpo(&corpo, "sessao"), agora) {
        Ok(lead) => {
            estado.leads.guardar(lead.clone());
            Resposta::json(200, &json!({"ok": true, "lead": lead})) // borda-json
        }
        Err(erro) => erro_do_chat(&erro),
    }
}

/// Mensagem para o visitante. Detalhe de provedor vai para o log do servidor, nunca para a resposta.
fn erro_do_chat(erro: &ErroChat) -> Resposta {
    match erro {
        ErroChat::Mensagem(motivo) => Resposta::erro(400, "mensagem_vazia", motivo),
        ErroChat::Cheia => Resposta::erro(429, "conversa_cheia", &erro.to_string()),
        ErroChat::SessaoInexistente => {
            Resposta::erro(404, "conversa_nao_encontrada", &erro.to_string())
        }
        ErroChat::Provedor(detalhe) => {
            eprintln!("provedor: {detalhe}");
            let aviso = match detalhe {
                Erro::ChaveRecusada => "O atendimento está com a chave recusada. Avise a equipe.",
                _ => "O atendimento está indisponível agora. Tente de novo em instantes.",
            };
            Resposta::erro(503, "provedor", aviso)
        }
        ErroChat::Resumo(motivo) => {
            eprintln!("resumo: {motivo}");
            Resposta::erro(502, "resumo", "não consegui resumir a conversa agora.")
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resposta_monta_cabecalhos_e_status() {
        let r = Resposta::erro(418, "bule", "teste").com("retry-after", "3");
        assert_eq!(r.status, 418);
        assert_eq!(r.extras.len(), 1);
        let _ = r.para_tiny();
        let vazia = Resposta::vazia(204).para_tiny();
        let _ = vazia;
    }

    #[test]
    fn origem_sem_lista_nao_ganha_cors() {
        let estado = Estado::novo(
            Configuracao {
                provedor: Tipo::OpenAi,
                modelo: "m".into(),
                chave: None,
                base: None,
                origens: vec!["https://ok.exemplo".into()],
                porta: 1,
                endereco: "127.0.0.1".into(),
                dados: "dados".into(),
                roteiro: "roteiro.toml".into(),
            },
            Roteiro::ler(roteiro::EXEMPLO).unwrap(),
        );
        let sem = com_cors(Resposta::vazia(200), None, &estado);
        assert_eq!(sem.extras, Vec::<(&'static str, String)>::new());
        let outra = com_cors(Resposta::vazia(200), Some("https://mau.exemplo"), &estado);
        assert_eq!(outra.extras, Vec::<(&'static str, String)>::new());
        let boa = com_cors(Resposta::vazia(200), Some("https://ok.exemplo"), &estado);
        assert_eq!(boa.extras.len(), 2);
    }
}
