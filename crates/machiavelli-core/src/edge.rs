//! Arestas do grafo estratégico (`cf_adj_strategic`).

use crate::error::CoreError;
use crate::ids::{EdgeId, NodeId};
use serde::{Deserialize, Serialize};

/// Natureza de uma relação entre dois estados.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq, Eq))]
pub enum EdgeKind {
    /// Aliança (partilha de créditos e acoplamento de fase — `ALLY`).
    Ally = 0,
    /// Dependência crítica (a queda do destino derruba a origem).
    CriticalDependency = 1,
    /// Vetor de ameaça.
    Threat = 2,
    /// Rota de fuga / failover.
    EscapeRoute = 3,
    /// Cooperação de soma positiva sem aliança formal.
    Cooperation = 4,
}

impl TryFrom<u8> for EdgeKind {
    type Error = CoreError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => EdgeKind::Ally,
            1 => EdgeKind::CriticalDependency,
            2 => EdgeKind::Threat,
            3 => EdgeKind::EscapeRoute,
            4 => EdgeKind::Cooperation,
            other => return Err(CoreError::UnknownEdgeKind(other)),
        })
    }
}

/// Aresta dirigida `from → to`.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct StrategicEdge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
    pub weight: f32,
}

/// Entrada da lista de adjacência de um nó.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct StrategicAdj {
    pub edge: EdgeId,
    pub neighbor: NodeId,
    pub kind: EdgeKind,
    pub weight: f32,
    /// `true` se a aresta sai deste nó.
    pub outgoing: bool,
}

impl StrategicEdge {
    /// As duas entradas de adjacência (origem e destino) desta aresta.
    pub fn adjacency_pair(&self) -> (StrategicAdj, StrategicAdj) {
        (
            StrategicAdj {
                edge: self.id,
                neighbor: self.to,
                kind: self.kind,
                weight: self.weight,
                outgoing: true,
            },
            StrategicAdj {
                edge: self.id,
                neighbor: self.from,
                kind: self.kind,
                weight: self.weight,
                outgoing: false,
            },
        )
    }
}
