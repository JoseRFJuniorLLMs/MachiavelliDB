//! SPEC-0002: energia estratégica, massa diagonal e análise local de risco.
//!
//! A massa e as métricas são snapshots constantes durante cada rollout. Uma
//! atualização externa exige reconstruir o sistema entre rollouts: não é um
//! Hamiltoniano autónomo se os parâmetros variarem no meio da integração.
//! A classificação da Hessiana é local; não prova bacias globais ou separatrizes.

pub mod error;
pub mod landscape;
pub mod mass;
pub mod potential;
pub mod strategic;

pub use error::HamiltonianError;
pub use landscape::{LandscapeKind, LandscapeReport, LandscapeTolerances, analyze};
pub use mass::{MassPolicy, StrategicMass};
pub use potential::{
    AdversarialPotential, FortunaPotential, RiskSnapshot, RiskWeights, SumPotential,
};
pub use strategic::{EnergyBreakdown, StrategicHamiltonian, StrategicPotential};
