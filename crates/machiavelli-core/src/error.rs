use thiserror::Error;

/// Erros de validação e codificação dos tipos primitivos.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum CoreError {
    #[error("tamanho inválido: esperado {expected} bytes, recebido {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("postura estratégica desconhecida: {0}")]
    UnknownPosture(u8),
    #[error("tipo de aresta desconhecido: {0}")]
    UnknownEdgeKind(u8),
    #[error("vetor de fase inválido: {0}")]
    InvalidPhase(String),
    #[error("valor fora do domínio: {field} = {value} (domínio {domain})")]
    OutOfDomain {
        field: &'static str,
        value: f64,
        domain: &'static str,
    },
    #[error("falha de (de)serialização: {0}")]
    Codec(String),
    #[error("identificador inválido: {0}")]
    InvalidId(String),
}
