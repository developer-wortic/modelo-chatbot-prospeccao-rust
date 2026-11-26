//! Roda o binário de verdade. Nada sai da máquina. A porta é a que o sistema escolhe (PORTA=0),
//! lida do aviso de partida: assim nenhum teste disputa porta com outro.

mod comum;

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use comum::falso::{eco_openai, subir};
use serde_json::json; // borda-json: corpo enviado ao binário

/// Pasta temporária única para cada chamada, para os testes não pisarem uns nos outros.
fn pasta(nome: &str) -> PathBuf {
    let agora = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let caminho =
        std::env::temp_dir().join(format!("chatbot-bin-{nome}-{}-{agora}", std::process::id()));
    std::fs::create_dir_all(&caminho).unwrap();
    caminho
}

/// Variáveis herdadas do ambiente de teste, sem as do modelo (CHATBOT_*, PORTA, ENDERECO).
/// Mantém `LLVM_PROFILE_FILE`, para a cobertura também contar o binário.
fn herdado() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| !k.starts_with("CHATBOT_") && k != "PORTA" && k != "ENDERECO")
        .collect()
}

/// Sobe o binário com ambiente limpo, porta 0 e as variáveis dadas. Devolve o processo e as linhas do stdout.
fn subir_binario(ambiente: &[(&str, String)]) -> (Child, Receiver<String>) {
    let mut comando = Command::new(env!("CARGO_BIN_EXE_chatbot-prospeccao"));
    comando
        .env_clear()
        .envs(herdado())
        .env(
            "CHATBOT_DADOS",
            pasta("dados").to_string_lossy().to_string(),
        )
        .env("CHATBOT_ROTEIRO", "roteiro-que-nao-existe.toml")
        .env("PORTA", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (chave, valor) in ambiente {
        comando.env(chave, valor);
    }
    let mut filho = comando.spawn().expect("sobe o binário");
    let saida = filho.stdout.take().expect("saída do binário");
    let (envio, recebe) = mpsc::channel();
    thread::spawn(move || {
        for linha in BufReader::new(saida).lines().map_while(Result::ok) {
            if envio.send(linha).is_err() {
                break;
            }
        }
    });
    (filho, recebe)
}

/// Espera o aviso de partida e tira dele a porta que o sistema abriu.
fn porta_anunciada(linhas: &Receiver<String>) -> u16 {
    let inicio = Instant::now();
    while inicio.elapsed() < Duration::from_secs(10) {
        if let Ok(linha) = linhas.recv_timeout(Duration::from_millis(200))
            && linha.contains("no ar em http://")
        {
            let porta = linha.trim_end_matches('/').rsplit(':').next().unwrap();
            return porta.parse().expect("porta no aviso");
        }
    }
    panic!("o binário não anunciou a porta");
}

fn linhas_de_partida(linhas: &Receiver<String>) -> String {
    (0..3)
        .filter_map(|_| linhas.recv_timeout(Duration::from_secs(10)).ok())
        .collect::<Vec<String>>()
        .join("\n")
}

#[test]
fn porta_invalida_sai_com_erro_de_configuracao() {
    let saida = Command::new(env!("CARGO_BIN_EXE_chatbot-prospeccao"))
        .env_clear()
        .envs(herdado())
        .env(
            "CHATBOT_DADOS",
            pasta("porta").to_string_lossy().to_string(),
        )
        .env("PORTA", "abc")
        .output()
        .expect("roda o binário");
    assert!(!saida.status.success());
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(
        erro.contains("não consegui abrir a porta"),
        "stderr: {erro}"
    );
}

#[test]
fn provedor_desconhecido_sai_com_erro_de_configuracao() {
    let saida = Command::new(env!("CARGO_BIN_EXE_chatbot-prospeccao"))
        .env_clear()
        .envs(herdado())
        .env(
            "CHATBOT_DADOS",
            pasta("provedor").to_string_lossy().to_string(),
        )
        .env("CHATBOT_PROVEDOR", "nenhum")
        .output()
        .expect("roda o binário");
    assert!(!saida.status.success());
    assert!(String::from_utf8_lossy(&saida.stderr).contains("provedor desconhecido"));
}

#[test]
fn porta_ocupada_sai_com_erro_ao_abrir() {
    let ocupada = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ocupada.local_addr().unwrap().port();
    let saida = Command::new(env!("CARGO_BIN_EXE_chatbot-prospeccao"))
        .env_clear()
        .envs(herdado())
        .env(
            "CHATBOT_DADOS",
            pasta("ocupada").to_string_lossy().to_string(),
        )
        .env("CHATBOT_ROTEIRO", "roteiro-que-nao-existe.toml")
        .env("PORTA", porta.to_string())
        .output()
        .expect("roda o binário");
    assert!(!saida.status.success());
    assert!(String::from_utf8_lossy(&saida.stderr).contains("não consegui abrir a porta"));
}

#[test]
fn roteiro_que_nao_le_sai_com_erro() {
    let dados = pasta("roteiro-ruim");
    let ruim = dados.join("ruim.toml");
    std::fs::write(&ruim, "isto = [").unwrap();
    let saida = Command::new(env!("CARGO_BIN_EXE_chatbot-prospeccao"))
        .env_clear()
        .envs(herdado())
        .env("CHATBOT_DADOS", dados.to_string_lossy().to_string())
        .env("CHATBOT_ROTEIRO", ruim.to_string_lossy().to_string())
        .env("PORTA", "0")
        .output()
        .expect("roda o binário");
    assert!(!saida.status.success());
    assert!(String::from_utf8_lossy(&saida.stderr).contains("roteiro inválido"));
}

#[test]
fn binario_sobe_e_avisa_que_nao_tem_chave_e_que_usa_o_roteiro_de_exemplo() {
    let (mut filho, linhas) = subir_binario(&[]);
    let todas = linhas_de_partida(&linhas);
    let _ = filho.kill();
    let _ = filho.wait();
    assert!(todas.contains("no ar"), "{todas}");
    assert!(todas.contains("sem chave"), "{todas}");
    assert!(todas.contains("roteiro de exemplo"), "{todas}");
}

#[test]
fn binario_responde_a_tela_e_o_chat_com_o_provedor_falso() {
    let falso = subir(|pedido, _| eco_openai(pedido));
    let (mut filho, linhas) = subir_binario(&[
        ("CHATBOT_CHAVE", "chave-de-teste-123".into()),
        ("CHATBOT_BASE_URL", format!("{}/v1", falso.base)),
        ("ENDERECO", "0.0.0.0".into()),
    ]);
    let porta = porta_anunciada(&linhas);
    let url = format!("http://127.0.0.1:{porta}");
    let tela = ureq::get(&format!("{url}/"))
        .timeout(Duration::from_secs(5))
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    let chat = ureq::post(&format!("{url}/api/chat"))
        .timeout(Duration::from_secs(5))
        .send_string(&json!({"mensagem": "Oi, quero conhecer o sistema"}).to_string()); // borda-json: corpo enviado ao chat
    let resposta = chat.expect("o chat responde").into_string().unwrap();
    let _ = filho.kill();
    let _ = filho.wait();
    assert!(tela.contains("<!doctype html"), "tela: {tela:.80}");
    assert!(
        resposta.contains("Oi, quero conhecer o sistema"),
        "chat: {resposta}"
    );
    assert_eq!(falso.recebidos(), 1);
}

#[test]
fn saida_com_aviso_de_rede_quando_endereco_nao_e_local() {
    let (mut filho, linhas) = subir_binario(&[
        ("ENDERECO", "0.0.0.0".into()),
        ("CHATBOT_CHAVE", "chave-de-teste-123".into()),
    ]);
    let todas = linhas_de_partida(&linhas);
    let _ = filho.kill();
    let _ = filho.wait();
    assert!(todas.contains("no ar"), "{todas}");
    assert!(todas.contains("aberto na rede"), "{todas}");
    assert!(!todas.contains("sem chave"), "{todas}");
}
