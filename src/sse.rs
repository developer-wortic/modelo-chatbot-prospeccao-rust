//! Lê um fluxo de Server-Sent Events (SSE). Linhas de comentário (que começam com `:`) são ignoradas.

use std::io::{self, BufRead};

/// Um evento do fluxo: o campo `event:` (se veio) e o `data:`, com as linhas juntadas por `\n`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evento {
    pub evento: Option<String>,
    pub dados: String,
}

/// Iterador de eventos sobre um leitor de linhas. Não é genérico, para existir uma só cópia do código.
pub struct Eventos<'a> {
    leitor: Box<dyn BufRead + 'a>,
}

/// Cria o iterador de eventos sobre `leitor`.
#[must_use]
pub fn eventos(leitor: Box<dyn BufRead + '_>) -> Eventos<'_> {
    Eventos { leitor }
}

impl Iterator for Eventos<'_> {
    type Item = io::Result<Evento>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut evento: Option<String> = None;
        let mut dados: Vec<String> = Vec::new();
        let mut linha = String::new();
        loop {
            linha.clear();
            match self.leitor.read_line(&mut linha) {
                Err(erro) => return Some(Err(erro)),
                Ok(0) if dados.is_empty() => return None,
                Ok(0) => return Some(Ok(montar(evento, &dados))),
                Ok(_) => {}
            }
            let limpa = linha.trim_end_matches(['\r', '\n']);
            if limpa.is_empty() {
                if dados.is_empty() {
                    evento = None;
                    continue;
                }
                return Some(Ok(montar(evento, &dados)));
            }
            if limpa.starts_with(':') {
                continue;
            }
            if let Some(nome) = limpa.strip_prefix("event:") {
                evento = Some(nome.trim_start().to_string());
            } else if let Some(valor) = limpa.strip_prefix("data:") {
                dados.push(valor.strip_prefix(' ').unwrap_or(valor).to_string());
            }
        }
    }
}

fn montar(evento: Option<String>, dados: &[String]) -> Evento {
    Evento {
        evento,
        dados: dados.join("\n"),
    }
}

#[cfg(test)]
mod testes {
    use super::{Evento, eventos};

    fn todos(texto: &str) -> Vec<Evento> {
        eventos(Box::new(texto.as_bytes()))
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn junta_event_e_data_e_ignora_comentarios() {
        let fluxo = ": OPENROUTER PROCESSING\n\nevent: response.created\ndata: {\"a\":1}\n\ndata: uma\ndata: duas\r\n\r\n";
        let lidos = todos(fluxo);
        assert_eq!(lidos.len(), 2);
        assert_eq!(lidos[0].evento.as_deref(), Some("response.created"));
        assert_eq!(lidos[0].dados, "{\"a\":1}");
        assert_eq!(lidos[1].evento, None);
        assert_eq!(lidos[1].dados, "uma\nduas");
    }

    #[test]
    fn evento_sem_linha_em_branco_final_ainda_sai() {
        let lidos = todos("data: fim");
        assert_eq!(lidos.len(), 1);
        assert_eq!(lidos[0].dados, "fim");
    }

    #[test]
    fn bloco_vazio_e_fluxo_vazio_nao_geram_evento() {
        assert_eq!(todos("\n\nevent: so-nome\n\n"), Vec::<Evento>::new());
        assert_eq!(todos(""), Vec::<Evento>::new());
    }

    #[test]
    fn erro_de_leitura_aparece_como_erro() {
        struct Quebrado;
        impl std::io::Read for Quebrado {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("queda"))
            }
        }
        let leitor = std::io::BufReader::new(Quebrado);
        let primeiro = eventos(Box::new(leitor)).next().unwrap();
        assert!(primeiro.is_err());
    }

    #[test]
    fn data_sem_espaco_depois_do_dois_pontos_tambem_vale() {
        let lidos = todos("data:cru\n\n");
        assert_eq!(lidos[0].dados, "cru");
    }

    #[test]
    fn campos_que_nao_sao_event_nem_data_sao_ignorados() {
        let lidos = todos("id: 7\nretry: 1000\ndata: ok\n\n");
        assert_eq!(lidos.len(), 1);
        assert_eq!(lidos[0].dados, "ok");
    }
}
