//! Guardas de transição: o kernel de armazenamento consulta-as antes de gravar
//! qualquer mutação (Casimir — SPEC-0017, Cidadela — SPEC-0010).

use crate::ids::NodeId;
use crate::meta::ContinentiaMeta;
use crate::phase::PhaseVector;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Provas que acompanham uma mutação.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Authorization {
    /// Certificado de quórum MinBFT serializado (verificado por `machiavelli-fortify`).
    pub quorum_certificate: Option<Vec<u8>>,
    /// Referência do `DurableIntent` no HeraclitusDB, quando a mutação tem efeito externo.
    pub intent_receipt: Option<String>,
    /// Principal que pediu a mutação.
    pub principal: Option<String>,
}

impl Authorization {
    pub fn none() -> Self {
        Self::default()
    }
}

/// Uma mutação candidata de um nó: estado anterior e posterior.
#[derive(Clone, Copy, Debug)]
pub struct Transition<'a> {
    pub node: NodeId,
    pub before_meta: Option<&'a ContinentiaMeta>,
    /// `None` significa remoção do nó.
    pub after_meta: Option<&'a ContinentiaMeta>,
    pub before_phase: Option<&'a PhaseVector>,
    pub after_phase: Option<&'a PhaseVector>,
    pub authorization: &'a Authorization,
}

/// Recusa de uma guarda: a transição tem probabilidade zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuardViolation {
    pub guard: String,
    pub node: NodeId,
    pub reason: String,
}

impl fmt::Display for GuardViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.guard, self.node, self.reason)
    }
}

impl std::error::Error for GuardViolation {}

/// Predicado sobre transições. Implementações devem ser puras e rápidas: correm no
/// caminho de escrita de cada mutação.
pub trait TransitionGuard: Send + Sync + fmt::Debug {
    fn name(&self) -> &'static str;
    fn check(&self, transition: &Transition<'_>) -> Result<(), GuardViolation>;
}
