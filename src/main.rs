//! Ponto de entrada. Toda a lógica de partida fica em `servidor::principal`, que os testes conseguem chamar.

use std::process::ExitCode;

use chatbot_prospeccao::servidor;

fn main() -> ExitCode {
    servidor::principal(std::env::vars())
}
