use thiserror::Error;

/// Erros de configuração e avaliação do Hamiltoniano estratégico.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum HamiltonianError {
    #[error("parâmetro inválido: {field} = {value}")]
    InvalidParameter { field: &'static str, value: f64 },
    #[error("dimensão incoerente: esperado {expected}, recebido {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("valor não finito em {field}")]
    NonFinite { field: &'static str },
    #[error("análise espectral não convergiu em {iterations} iterações")]
    NotConverged { iterations: usize },
    #[error("nó traído está isolado e não pode ser integrado")]
    InactiveNode,
    #[error(transparent)]
    Core(#[from] machiavelli_core::CoreError),
    #[error(transparent)]
    Integrator(#[from] machiavelli_symplectic::IntegratorError),
}
