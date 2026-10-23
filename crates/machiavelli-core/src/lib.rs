//! # machiavelli-core
//!
//! Tipos primitivos partilhados por todo o MachiavelliDB (camada I, SPEC-0001/0002/0003).
//!
//! - [`ContinentiaMeta`]: metadados táticos de 128 bytes alinhados em cache line.
//! - [`PhaseVector`]: estado conjugado `z = (q, p)` mais as coordenadas de Casimir `c`.
//! - [`StrategicEdge`] / [`StrategicAdj`]: arestas do grafo estratégico.
//! - [`VirtuScore`] / [`FortunaRisk`]: escalares limitados a `[0, 1]`.
//! - [`Hlc`]: relógio lógico híbrido usado em `created_at_hlc`.
//! - [`TransitionGuard`] e [`AuditSink`]: pontos de extensão que o armazenamento e a
//!   ponte Trinitas implementam sem criar dependências circulares.

pub mod audit;
pub mod codec;
pub mod deadline;
pub mod edge;
pub mod error;
pub mod guard;
pub mod hlc;
pub mod ids;
pub mod meta;
pub mod node;
pub mod payoff;
pub mod phase;
pub mod scores;

pub use audit::{AuditKind, AuditReceipt, AuditRecord, AuditSink, MemoryAuditSink};
pub use deadline::Deadline;
pub use edge::{EdgeKind, StrategicAdj, StrategicEdge};
pub use error::CoreError;
pub use guard::{Authorization, GuardViolation, Transition, TransitionGuard};
pub use hlc::Hlc;
pub use ids::{EdgeId, InfosetHash, InvariantId, NodeId, SubgraphId, TriggerHash};
pub use meta::{ContinentiaMeta, StrategicPosture};
pub use node::ContinentiaNode;
pub use payoff::PayoffMatrix;
pub use phase::PhaseVector;
pub use scores::{FortunaRisk, VirtuScore};

/// Dimensões de configuração típicas da SPEC-0001 (`n`); o vetor de fase tem `2n` floats.
pub const TYPICAL_DIMS: [usize; 2] = [64, 128];
