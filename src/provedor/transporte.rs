//! Envio HTTP comum a todos os provedores: tempo-limite, 429 com `retry-after`, 5xx com reenvio limitado.
//! Erro de chave (401/403) nunca é reenviado. Nenhum log sai daqui.

use std::io::{self, BufReader, Read};
use std::thread;
use std::time::Duration;

use crate::erro::Erro;
use serde_json::Value; // borda-json: corpo que vai para a API do provedor (terceiro)

/// Fluxo de bytes da resposta, lido linha a linha pelo leitor de SSE.
pub type Fluxo = BufReader<Box<dyn Read + Send + Sync>>;

/// Maior espera, em segundos, que o transporte aceita antes de reenviar um 429.
const ESPERA_MAXIMA_SEGUNDOS: u64 = 10;

/// Como o transporte se comporta: tempo-limite, quantos reenvios e a pausa base entre eles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transporte {
    pub tempo_limite: Duration,
    pub reenvios: u32,
    pub pausa: Duration,
}

impl Default for Transporte {
    fn default() -> Self {
        Self {
            tempo_limite: super::TEMPO_LIMITE_PADRAO,
            reenvios: 2,
            pausa: Duration::from_secs(1),
        }
    }
}

impl Transporte {
    /// Faz um POST com corpo JSON e devolve o fluxo da resposta, já com status de sucesso.
    ///
    /// # Errors
    ///
    /// Devolve o erro da última tentativa, depois dos reenvios permitidos.
    pub fn enviar(
        &self,
        url: &str,
        cabecalhos: &[(&str, &str)],
        corpo: &Value, // borda-json: corpo que vai para a API do provedor (terceiro)
    ) -> Result<Fluxo, Erro> {
        let texto = serde_json::to_string(corpo).unwrap_or_default(); // borda-json: serializa para a API do provedor (terceiro); um Value sempre serializa
        let mut tentativa = 0;
        loop {
            match self.uma_vez(url, cabecalhos, &texto) {
                Ok(fluxo) => return Ok(fluxo),
                Err(erro) if tentativa < self.reenvios && reenviavel(&erro) => {
                    thread::sleep(self.espera(&erro, tentativa));
                    tentativa += 1;
                }
                Err(erro) => return Err(erro),
            }
        }
    }

    fn espera(&self, erro: &Erro, tentativa: u32) -> Duration {
        match erro {
            Erro::LimiteDeTaxa {
                espere: Some(segundos),
            } => Duration::from_secs(*segundos),
            _ => self.pausa * 2u32.pow(tentativa),
        }
    }

    fn uma_vez(&self, url: &str, cabecalhos: &[(&str, &str)], corpo: &str) -> Result<Fluxo, Erro> {
        let mut pedido = ureq::post(url)
            .timeout(self.tempo_limite)
            .set("content-type", "application/json") // borda-json: tipo do corpo enviado ao provedor
            .set("accept", "text/event-stream");
        for (nome, valor) in cabecalhos {
            pedido = pedido.set(nome, valor);
        }
        match pedido.send_string(corpo) {
            Ok(resposta) => Ok(BufReader::new(resposta.into_reader())),
            Err(ureq::Error::Status(codigo, resposta)) => Err(erro_de_status(codigo, &resposta)),
            Err(ureq::Error::Transport(transporte)) => Err(erro_de_transporte(&transporte)),
        }
    }
}

fn reenviavel(erro: &Erro) -> bool {
    match erro {
        Erro::Servidor(_) => true,
        Erro::LimiteDeTaxa {
            espere: Some(segundos),
        } => *segundos <= ESPERA_MAXIMA_SEGUNDOS,
        _ => false,
    }
}

/// Traduz o status de erro em `Erro`. A chave nunca entra na mensagem.
fn erro_de_status(codigo: u16, resposta: &ureq::Response) -> Erro {
    match codigo {
        401 | 403 => Erro::ChaveRecusada,
        429 => Erro::LimiteDeTaxa {
            espere: resposta
                .header("retry-after")
                .and_then(|valor| valor.trim().parse().ok()),
        },
        500..=599 => Erro::Servidor(codigo),
        _ => Erro::Recusado(codigo),
    }
}

fn erro_de_transporte(transporte: &ureq::Transport) -> Erro {
    let tempo = std::error::Error::source(transporte)
        .and_then(|causa| causa.downcast_ref::<io::Error>())
        .is_some_and(|e| e.kind() == io::ErrorKind::TimedOut);
    if tempo {
        Erro::TempoEsgotado
    } else {
        Erro::Rede(transporte.kind().to_string())
    }
}

/// Fluxo a partir de bytes, para os testes do módulo.
#[cfg(test)]
pub(crate) fn fluxo_de_teste(bytes: &[u8]) -> Fluxo {
    let dentro: Box<dyn Read + Send + Sync> = Box::new(std::io::Cursor::new(bytes.to_vec()));
    BufReader::new(dentro)
}
