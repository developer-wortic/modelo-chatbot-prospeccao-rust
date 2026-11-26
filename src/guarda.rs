//! Proteção contra injeção de instruções. Duas pontas: a fala do visitante entra limpa e como dado,
//! e a resposta do bot é conferida antes de sair. Nenhuma fala do visitante vira instrução de sistema.

use crate::roteiro::Roteiro;

/// Tamanho máximo de uma fala do visitante, em caracteres. Corta excesso antes de chegar ao provedor.
pub const MAX_CARACTERES: usize = 1000;

/// Tamanho mínimo de uma chave para a conferência de saída. Chave curta demais geraria falso positivo.
const CHAVE_MINIMA: usize = 8;

/// Quantas perguntas do roteiro juntas já contam como o roteiro sendo listado.
const PERGUNTAS_PARA_VAZAR: usize = 3;

/// Texto que substitui uma resposta recusada. Não explica a regra, para não dar pista.
pub const RESPOSTA_RECUSADA: &str =
    "Não posso compartilhar isso. Me conte um pouco sobre o que você procura que eu sigo daí.";

/// Limpa a fala do visitante: tira controles (menos a quebra de linha), corta o excesso e recusa vazia.
///
/// # Errors
///
/// Devolve a mensagem de erro quando a fala fica vazia depois de limpa.
pub fn limpar_mensagem(texto: &str) -> Result<String, String> {
    let sem_controle: String = texto
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect();
    let limpo: String = sem_controle.trim().chars().take(MAX_CARACTERES).collect();
    if limpo.is_empty() {
        Err("a mensagem está vazia".into())
    } else {
        Ok(limpo)
    }
}

/// Verdadeiro se a resposta do bot vazaria algo protegido: a chave, uma regra ou critério interno
/// do roteiro, ou a lista de perguntas do roteiro.
#[must_use]
pub fn vaza(resposta: &str, roteiro: &Roteiro, chave: &str) -> bool {
    if chave.chars().count() >= CHAVE_MINIMA && resposta.contains(chave) {
        return true;
    }
    let minuscula = resposta.to_lowercase();
    if roteiro
        .protegidos()
        .iter()
        .any(|trecho| minuscula.contains(trecho.as_str()))
    {
        return true;
    }
    let perguntas = roteiro
        .perguntas()
        .iter()
        .filter(|p| minuscula.contains(&p.trim().to_lowercase()))
        .count();
    perguntas >= PERGUNTAS_PARA_VAZAR
}

#[cfg(test)]
mod testes {
    use super::*;

    fn roteiro() -> Roteiro {
        Roteiro::ler(crate::roteiro::EXEMPLO).unwrap()
    }

    #[test]
    fn limpa_controles_e_corta_o_excesso() {
        assert_eq!(
            limpar_mensagem("  oi\u{0007}\nvamos  ").unwrap(),
            "oi\nvamos"
        );
        let longa = "a".repeat(MAX_CARACTERES + 50);
        assert_eq!(
            limpar_mensagem(&longa).unwrap().chars().count(),
            MAX_CARACTERES
        );
    }

    #[test]
    fn mensagem_vazia_depois_de_limpa_e_recusada() {
        assert!(limpar_mensagem("   \u{0001}  ").is_err());
    }

    #[test]
    fn resposta_comum_passa() {
        let r = roteiro();
        assert!(!vaza(
            "Olá! Quantas pessoas trabalham na loja hoje?",
            &r,
            "chave-longa-123"
        ));
    }

    #[test]
    fn repetir_a_chave_e_vazamento_mas_chave_curta_nao_conta() {
        let r = roteiro();
        assert!(vaza("a chave é chave-longa-123", &r, "chave-longa-123"));
        assert!(!vaza("sim, tenho certeza", &r, "abc"));
        assert!(!vaza("qualquer coisa", &r, ""));
    }

    #[test]
    fn repetir_uma_regra_do_roteiro_e_vazamento_mesmo_em_maiusculas() {
        let r = roteiro();
        let regra = crate::roteiro::REGRAS[1].to_uppercase();
        assert!(vaza(&regra, &r, "x"));
    }

    #[test]
    fn listar_tres_ou_mais_perguntas_do_roteiro_e_vazamento() {
        let r = roteiro();
        let perguntas = r.perguntas();
        let duas = format!("{} {}", perguntas[0], perguntas[1]);
        assert!(!vaza(&duas, &r, "x"));
        let tres = format!("{} {} {}", perguntas[0], perguntas[1], perguntas[2]);
        assert!(vaza(&tres, &r, "x"));
    }

    #[test]
    fn resposta_recusada_nao_explica_a_regra() {
        assert!(!RESPOSTA_RECUSADA.contains("regra"));
        assert!(!RESPOSTA_RECUSADA.contains("instru"));
    }
}
