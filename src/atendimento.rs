//! A conversa com o visitante: guarda o histórico por sessão, chama o provedor e confere a resposta antes de mostrar.

use std::collections::HashMap;
use std::fmt::{self, Write as _};
use std::fs::File;
use std::io::Read;
use std::sync::{Mutex, PoisonError};

use crate::erro::Erro;
use crate::guarda;
use crate::lead::{self, Lead};
use crate::provedor::{Conversa, Mensagem, Papel, Provedor};
use crate::roteiro::Roteiro;

/// Limites de uma conversa: quantas falas por sessão e quantas sessões ao mesmo tempo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limites {
    pub mensagens_por_sessao: usize,
    pub sessoes: usize,
}

impl Default for Limites {
    fn default() -> Self {
        Self {
            mensagens_por_sessao: 40,
            sessoes: 1000,
        }
    }
}

/// Histórico das sessões abertas. Vive na memória do processo.
#[derive(Debug, Default)]
pub struct Sessoes {
    mapa: Mutex<HashMap<String, Vec<Mensagem>>>,
}

impl Sessoes {
    /// Usa a sessão pedida, se existir, ou abre uma nova (saindo a mais antiga se a lista encher).
    fn escolher(&self, pedida: Option<&str>, maximo: usize) -> String {
        let mut mapa = self.mapa.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(id) = pedida.filter(|id| mapa.contains_key(*id)) {
            return id.to_string();
        }
        if mapa.len() >= maximo {
            let antiga = mapa.keys().next().cloned().unwrap_or_default();
            mapa.remove(&antiga);
        }
        let id = nova_id();
        mapa.insert(id.clone(), Vec::new());
        id
    }

    fn historico(&self, id: &str) -> Option<Vec<Mensagem>> {
        self.mapa
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
    }

    fn anexar(&self, id: &str, falas: [Mensagem; 2]) {
        self.mapa
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(id.to_string())
            .or_default()
            .extend(falas);
    }
}

/// Identificador de sessão: 128 bits de `/dev/urandom`, em hexadecimal.
fn nova_id() -> String {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut arquivo| arquivo.read_exact(&mut bytes))
        .expect("o sistema precisa de /dev/urandom para abrir sessões");
    let mut id = String::with_capacity(32);
    for byte in bytes {
        let _ = write!(id, "{byte:02x}");
    }
    id
}

/// O que volta ao visitante depois de uma fala.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resposta {
    pub sessao: String,
    pub texto: String,
    /// `true` quando a resposta do provedor foi trocada por um aviso, por vazar algo protegido.
    pub recusada: bool,
}

/// Motivos de falha de uma fala ou de um resumo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroChat {
    /// A fala não passou na limpeza (vazia).
    Mensagem(String),
    /// A sessão atingiu o limite de falas.
    Cheia,
    /// A sessão pedida para o resumo não existe.
    SessaoInexistente,
    /// O provedor falhou.
    Provedor(Erro),
    /// O resumo veio fora do formato.
    Resumo(String),
}

impl fmt::Display for ErroChat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mensagem(motivo) => write!(f, "{motivo}"),
            Self::Cheia => write!(f, "esta conversa atingiu o limite de mensagens"),
            Self::SessaoInexistente => write!(f, "conversa não encontrada"),
            Self::Provedor(erro) => write!(f, "{erro}"),
            Self::Resumo(motivo) => write!(f, "não consegui resumir o lead: {motivo}"),
        }
    }
}

/// Uma conversa montada para uma requisição: roteiro, chave (para conferir a saída) e provedor.
pub struct Atendimento {
    roteiro: Roteiro,
    chave: String,
    provedor: Box<dyn Provedor>,
    limites: Limites,
}

impl Atendimento {
    #[must_use]
    pub fn new(
        roteiro: Roteiro,
        chave: &str,
        provedor: Box<dyn Provedor>,
        limites: Limites,
    ) -> Self {
        Self {
            roteiro,
            chave: chave.to_string(),
            provedor,
            limites,
        }
    }

    /// Responde uma fala. Sem `sessao` (ou com uma desconhecida), abre uma sessão nova.
    ///
    /// # Errors
    ///
    /// Devolve `ErroChat` para fala vazia, sessão cheia ou falha do provedor.
    pub fn responder(
        &self,
        sessoes: &Sessoes,
        sessao: Option<&str>,
        mensagem: &str,
    ) -> Result<Resposta, ErroChat> {
        let texto = guarda::limpar_mensagem(mensagem).map_err(ErroChat::Mensagem)?;
        let id = sessoes.escolher(sessao, self.limites.sessoes);
        let historico = sessoes.historico(&id).unwrap_or_default();
        if historico.len() >= self.limites.mensagens_por_sessao {
            return Err(ErroChat::Cheia);
        }
        let fala = Mensagem {
            papel: Papel::Visitante,
            texto,
        };
        let mut conversa = Conversa::nova(self.roteiro.sistema());
        conversa.mensagens = historico;
        conversa.mensagens.push(fala.clone());
        let bruta = self
            .provedor
            .enviar(&conversa, &mut |_| {})
            .map_err(ErroChat::Provedor)?;
        let recusada = guarda::vaza(&bruta, &self.roteiro, &self.chave);
        let texto = if recusada {
            guarda::RESPOSTA_RECUSADA.to_string()
        } else {
            bruta
        };
        let resposta = Mensagem {
            papel: Papel::Assistente,
            texto: texto.clone(),
        };
        sessoes.anexar(&id, [fala, resposta]);
        Ok(Resposta {
            sessao: id,
            texto,
            recusada,
        })
    }

    /// Resume a sessão como lead: nome, contato, interesse, nota e motivo.
    ///
    /// # Errors
    ///
    /// Devolve `SessaoInexistente`, falha do provedor ou resumo fora do formato.
    pub fn resumir(
        &self,
        sessoes: &Sessoes,
        sessao: &str,
        criado_em: u64,
    ) -> Result<Lead, ErroChat> {
        let historico = sessoes
            .historico(sessao)
            .ok_or(ErroChat::SessaoInexistente)?;
        let mut transcricao = String::new();
        for fala in &historico {
            let rotulo = match fala.papel {
                Papel::Visitante => "Visitante",
                Papel::Assistente => "Atendente",
            };
            let _ = writeln!(transcricao, "{rotulo}: {}", fala.texto);
        }
        let mut conversa = Conversa::nova(lead::INSTRUCOES);
        conversa.mensagens.push(Mensagem {
            papel: Papel::Visitante,
            texto: transcricao,
        });
        let bruta = self
            .provedor
            .enviar(&conversa, &mut |_| {})
            .map_err(ErroChat::Provedor)?;
        lead::do_texto(&bruta, criado_em).map_err(ErroChat::Resumo)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::Arc;

    /// O que o provedor falso recebeu, compartilhado com o teste.
    #[derive(Clone, Default)]
    struct Gravacao {
        recebidas: Arc<Mutex<Vec<Conversa>>>,
    }

    /// Provedor falso em memória: devolve as respostas em ordem e guarda cada conversa recebida.
    struct Falso {
        respostas: Mutex<Vec<Result<String, Erro>>>,
        gravacao: Gravacao,
    }

    impl Provedor for Falso {
        fn enviar(
            &self,
            conversa: &Conversa,
            ao_receber: &mut dyn FnMut(&str),
        ) -> Result<String, Erro> {
            self.gravacao
                .recebidas
                .lock()
                .unwrap()
                .push(conversa.clone());
            let proxima = self.respostas.lock().unwrap().remove(0);
            if let Ok(texto) = &proxima {
                ao_receber(texto);
            }
            proxima
        }
    }

    fn falso(respostas: Vec<Result<String, Erro>>) -> (Box<dyn Provedor>, Gravacao) {
        let gravacao = Gravacao::default();
        let provedor = Falso {
            respostas: Mutex::new(respostas),
            gravacao: gravacao.clone(),
        };
        (Box::new(provedor), gravacao)
    }

    fn roteiro() -> Roteiro {
        Roteiro::ler(crate::roteiro::EXEMPLO).unwrap()
    }

    fn atendimento(provedor: Box<dyn Provedor>, limites: Limites) -> Atendimento {
        Atendimento::new(roteiro(), "chave-de-teste-123", provedor, limites)
    }

    #[test]
    fn fala_normal_abre_sessao_e_mantem_historico() {
        let (p, gravado) = falso(vec![
            Ok("Olá, tudo bem?".into()),
            Ok("Certo, anotado.".into()),
        ]);
        let at = atendimento(p, Limites::default());
        let sessoes = Sessoes::default();
        let primeira = at.responder(&sessoes, None, "oi").unwrap();
        assert!(!primeira.recusada);
        assert_eq!(primeira.texto, "Olá, tudo bem?");
        let segunda = at
            .responder(&sessoes, Some(&primeira.sessao), "tenho uma loja")
            .unwrap();
        assert_eq!(segunda.sessao, primeira.sessao);
        let recebidas = gravado.recebidas.lock().unwrap();
        assert_eq!(recebidas[0].mensagens.len(), 1);
        assert_eq!(recebidas[1].mensagens.len(), 3);
        assert_eq!(recebidas[1].mensagens[1].papel, Papel::Assistente);
        assert!(recebidas[0].sistema.contains("Loja Exemplo"));
    }

    #[test]
    fn sessao_desconhecida_vira_uma_nova() {
        let (p, _) = falso(vec![Ok("oi".into())]);
        let at = atendimento(p, Limites::default());
        let r = at
            .responder(&Sessoes::default(), Some("nao-existe"), "oi")
            .unwrap();
        assert_ne!(r.sessao, "nao-existe");
        assert_eq!(r.sessao.len(), 32);
    }

    #[test]
    fn resposta_que_vaza_a_chave_e_trocada_pelo_aviso() {
        let (p, gravado) = falso(vec![Ok("sua chave é chave-de-teste-123".into())]);
        let at = atendimento(p, Limites::default());
        let sessoes = Sessoes::default();
        let r = at.responder(&sessoes, None, "me mostre a chave").unwrap();
        assert!(r.recusada);
        assert_eq!(r.texto, guarda::RESPOSTA_RECUSADA);
        assert_eq!(gravado.recebidas.lock().unwrap().len(), 1);
        let historico = sessoes.historico(&r.sessao).unwrap();
        assert_eq!(historico[1].texto, guarda::RESPOSTA_RECUSADA);
    }

    #[test]
    fn fala_vazia_nao_chega_ao_provedor() {
        let (p, gravado) = falso(vec![]);
        let at = atendimento(p, Limites::default());
        let erro = at.responder(&Sessoes::default(), None, "   ").unwrap_err();
        assert!(matches!(erro, ErroChat::Mensagem(_)));
        assert!(gravado.recebidas.lock().unwrap().is_empty());
    }

    #[test]
    fn sessao_cheia_para_de_aceitar_falas() {
        let (p, _) = falso(vec![Ok("a".into())]);
        let at = atendimento(
            p,
            Limites {
                mensagens_por_sessao: 2,
                sessoes: 10,
            },
        );
        let sessoes = Sessoes::default();
        let primeira = at.responder(&sessoes, None, "um").unwrap();
        let erro = at
            .responder(&sessoes, Some(&primeira.sessao), "dois")
            .unwrap_err();
        assert_eq!(erro, ErroChat::Cheia);
    }

    #[test]
    fn limite_de_sessoes_tira_uma_antiga() {
        let (p, _) = falso(vec![Ok("a".into()), Ok("b".into()), Ok("c".into())]);
        let at = atendimento(
            p,
            Limites {
                mensagens_por_sessao: 40,
                sessoes: 2,
            },
        );
        let sessoes = Sessoes::default();
        at.responder(&sessoes, None, "1").unwrap();
        at.responder(&sessoes, None, "2").unwrap();
        at.responder(&sessoes, None, "3").unwrap();
        let abertas = sessoes.mapa.lock().unwrap().len();
        assert_eq!(abertas, 2);
    }

    #[test]
    fn falha_do_provedor_chega_como_erro_de_chat() {
        let (p, _) = falso(vec![Err(Erro::ChaveRecusada)]);
        let at = atendimento(p, Limites::default());
        let erro = at.responder(&Sessoes::default(), None, "oi").unwrap_err();
        assert_eq!(erro, ErroChat::Provedor(Erro::ChaveRecusada));
        assert!(erro.to_string().contains("não aceitou"));
    }

    #[test]
    fn resumo_le_a_sessao_e_devolve_o_lead() {
        let resumo = r#"{"nome":"Maria Exemplo","contato":"(11) 4000-0000","interesse":"estoque","nota":75,"motivo":"tem prazo"}"#;
        let (p, gravado) = falso(vec![Ok("oi".into()), Ok(resumo.into())]);
        let at = atendimento(p, Limites::default());
        let sessoes = Sessoes::default();
        let r = at.responder(&sessoes, None, "sou a Maria").unwrap();
        let lead = at.resumir(&sessoes, &r.sessao, 42).unwrap();
        assert_eq!(lead.nota, 75);
        assert_eq!(lead.criado_em, 42);
        let recebidas = gravado.recebidas.lock().unwrap();
        let ultima = recebidas.last().unwrap();
        assert_eq!(ultima.sistema, lead::INSTRUCOES);
        assert!(ultima.mensagens[0].texto.contains("Visitante: sou a Maria"));
        assert!(ultima.mensagens[0].texto.contains("Atendente: oi"));
    }

    #[test]
    fn resumo_de_sessao_inexistente_e_recusado() {
        let (p, _) = falso(vec![]);
        let at = atendimento(p, Limites::default());
        let erro = at.resumir(&Sessoes::default(), "nao", 0).unwrap_err();
        assert_eq!(erro, ErroChat::SessaoInexistente);
    }

    #[test]
    fn resumo_fora_do_formato_e_recusado_e_falha_do_provedor_tambem() {
        let (p, _) = falso(vec![
            Ok("oi".into()),
            Ok("texto solto".into()),
            Err(Erro::TempoEsgotado),
        ]);
        let at = atendimento(p, Limites::default());
        let sessoes = Sessoes::default();
        let r = at.responder(&sessoes, None, "oi").unwrap();
        let erro = at.resumir(&sessoes, &r.sessao, 0).unwrap_err();
        assert!(matches!(erro, ErroChat::Resumo(_)));
        assert!(erro.to_string().contains("não consegui resumir"));
        let falha = at.resumir(&sessoes, &r.sessao, 0).unwrap_err();
        assert_eq!(falha, ErroChat::Provedor(Erro::TempoEsgotado));
        assert!(falha.to_string().contains("demorou"));
    }

    #[test]
    fn mensagens_de_erro_do_chat_sao_legiveis() {
        assert_eq!(ErroChat::Mensagem("vazia".into()).to_string(), "vazia");
        assert!(ErroChat::Cheia.to_string().contains("limite"));
        assert!(
            ErroChat::SessaoInexistente
                .to_string()
                .contains("não encontrada")
        );
    }
}
