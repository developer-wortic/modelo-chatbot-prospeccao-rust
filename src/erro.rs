//! Erros de uma chamada a um provedor de IA. Nenhuma mensagem leva a chave.

use std::fmt;

/// Falha ao falar com um provedor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erro {
    /// 401 ou 403: o provedor não aceitou a chave.
    ChaveRecusada,
    /// 429: limite de chamadas. `espere` é o `retry-after`, em segundos, se o provedor mandou.
    LimiteDeTaxa { espere: Option<u64> },
    /// 5xx: o provedor falhou, mesmo depois dos reenvios.
    Servidor(u16),
    /// O tempo-limite da chamada estourou.
    TempoEsgotado,
    /// Falha de rede: conexão recusada, DNS ou leitura interrompida.
    Rede(String),
    /// Outro status de erro, que não é chave nem limite.
    Recusado(u16),
    /// A resposta não veio no formato esperado, ou o provedor mandou erro no meio do fluxo.
    Formato(String),
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChaveRecusada => write!(f, "o provedor não aceitou a chave"),
            Self::LimiteDeTaxa { espere: Some(s) } => {
                write!(f, "limite de chamadas atingido, tente de novo em {s} s")
            }
            Self::LimiteDeTaxa { espere: None } => {
                write!(f, "limite de chamadas atingido, tente mais tarde")
            }
            Self::Servidor(codigo) => write!(f, "o provedor falhou (status {codigo})"),
            Self::TempoEsgotado => write!(f, "o provedor demorou demais para responder"),
            Self::Rede(detalhe) => write!(f, "falha de rede: {detalhe}"),
            Self::Recusado(codigo) => write!(f, "o provedor recusou a chamada (status {codigo})"),
            Self::Formato(detalhe) => write!(f, "resposta inesperada do provedor: {detalhe}"),
        }
    }
}

impl std::error::Error for Erro {}

#[cfg(test)]
mod testes {
    use super::Erro;

    #[test]
    fn cada_erro_tem_mensagem_em_portugues_sem_detalhe_secreto() {
        let casos = [
            Erro::ChaveRecusada,
            Erro::LimiteDeTaxa { espere: Some(7) },
            Erro::LimiteDeTaxa { espere: None },
            Erro::Servidor(503),
            Erro::TempoEsgotado,
            Erro::Rede("conexão recusada".into()),
            Erro::Recusado(400),
            Erro::Formato("sem texto".into()),
        ];
        let mensagens: Vec<String> = casos.iter().map(ToString::to_string).collect();
        assert!(mensagens[1].contains("7 s"));
        assert!(mensagens[2].contains("mais tarde"));
        assert!(mensagens[3].contains("503"));
        assert!(mensagens.iter().all(|m| !m.is_empty()));
    }
}
