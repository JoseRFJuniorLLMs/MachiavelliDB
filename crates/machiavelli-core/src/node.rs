use crate::meta::ContinentiaMeta;
use crate::phase::PhaseVector;
use serde::{Deserialize, Serialize};

/// Estado completo em disputa: metadados táticos + coordenadas de fase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContinentiaNode {
    pub meta: ContinentiaMeta,
    pub phase: PhaseVector,
}

impl ContinentiaNode {
    pub fn new(meta: ContinentiaMeta, phase: PhaseVector) -> Self {
        Self { meta, phase }
    }
}
