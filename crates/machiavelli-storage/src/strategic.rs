//! Typed strategic records shared by WAL, RocksDB, backups and speculative snapshots.
//! Ledger entries record computed auctions; they do not execute resource transfers.
use crate::{Mutation, Nodes, StorageError, map};
use machiavelli_core::{
    EdgeId, InfosetHash, NodeId, PayoffMatrix, StrategicAdj, StrategicEdge, StrategicPosture,
    TriggerHash,
};
use machiavelli_dikes::{Action, CompiledDike};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Edges,
    Payoffs,
    Infosets,
    Regrets,
    Dikes,
    Ledger,
    Beliefs,
    Models,
    Sync,
}
impl Family {
    pub const ALL: [Self; 9] = [
        Self::Edges,
        Self::Payoffs,
        Self::Infosets,
        Self::Regrets,
        Self::Dikes,
        Self::Ledger,
        Self::Beliefs,
        Self::Models,
        Self::Sync,
    ];
    pub fn column_family(self) -> &'static str {
        match self {
            Self::Edges => "cf_adj_strategic",
            Self::Payoffs => "cf_payoff_tensors",
            Self::Infosets => "cf_infosets",
            Self::Regrets => "cf_regret_tables",
            Self::Dikes => "cf_dikes",
            Self::Ledger => "cf_vcg_ledger",
            Self::Beliefs => "cf_beliefs",
            Self::Models => "cf_neural_models",
            Self::Sync => "cf_trinitas_sync",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordKey {
    pub family: Family,
    pub id: Vec<u8>,
}
impl RecordKey {
    pub fn new(family: Family, id: &[u8]) -> Self {
        Self {
            family,
            id: id.to_vec(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StrategicRecord {
    // Boxed: every resident record is sized by its largest variant.
    Neural {
        id: InfosetHash,
        game: Box<machiavelli_cfr::extensive::Tree>,
        checkpoint: Box<machiavelli_cfr::extensive::deep::Checkpoint>,
    },
    Belief {
        node: NodeId,
        observation_id: String,
        observed_at_ms: u64,
        particles: Vec<machiavelli_core::PhaseVector>,
        weights: Vec<f64>,
    },
    Sync {
        entry: crate::sync::SyncEntry,
    },
    Edge {
        edge: StrategicEdge,
    },
    Payoff {
        edge: EdgeId,
        matrix: PayoffMatrix,
    },
    Infoset {
        id: InfosetHash,
        player: u8,
        actions: Vec<String>,
    },
    Regrets {
        id: InfosetHash,
        regrets: Vec<f64>,
        strategy_sum: Vec<f64>,
        iterations: u64,
    },
    Dike {
        id: TriggerHash,
        plan: CompiledDike,
    },
    Auction {
        id: TriggerHash,
        bidders: Vec<NodeId>,
        valuations: Vec<Vec<f64>>,
        outcome: usize,
        payments: Vec<f64>,
        welfare: f64,
    },
}
impl StrategicRecord {
    pub fn key(&self) -> RecordKey {
        match self {
            Self::Neural { id, .. } => RecordKey::new(Family::Models, id.as_bytes()),
            Self::Belief { node, .. } => RecordKey::new(Family::Beliefs, node.as_bytes()),
            Self::Sync { entry } => RecordKey::new(
                Family::Sync,
                blake3::hash(entry.operation_id.as_bytes()).as_bytes(),
            ),
            Self::Edge { edge } => RecordKey::new(Family::Edges, edge.id.as_bytes()),
            Self::Payoff { edge, .. } => RecordKey::new(Family::Payoffs, edge.as_bytes()),
            Self::Infoset { id, .. } => RecordKey::new(Family::Infosets, id.as_bytes()),
            Self::Regrets { id, .. } => RecordKey::new(Family::Regrets, id.as_bytes()),
            Self::Dike { id, .. } => RecordKey::new(Family::Dikes, id.as_bytes()),
            Self::Auction { id, .. } => RecordKey::new(Family::Ledger, id.as_bytes()),
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct Strategy {
    pub records: map::Map<RecordKey, Arc<StrategicRecord>>,
    pub adjacency: map::Map<NodeId, Arc<BTreeMap<EdgeId, StrategicAdj>>>,
}
fn rejected(reason: &'static str) -> StorageError {
    StorageError::Rejected(reason)
}
fn edge_mutable(nodes: &Nodes, edge: &StrategicEdge) -> Result<(), StorageError> {
    for id in [edge.from, edge.to] {
        let node = nodes
            .get(&id)
            .ok_or_else(|| rejected("edge endpoint does not exist"))?;
        if matches!(
            node.meta.posture,
            StrategicPosture::Fortified | StrategicPosture::Betrayed
        ) {
            return Err(rejected(
                "edge endpoint requires verified administrative authority",
            ));
        }
    }
    Ok(())
}
impl Strategy {
    pub fn record(&self, key: &RecordKey) -> Option<&StrategicRecord> {
        self.records.get(key).map(AsRef::as_ref)
    }
    pub fn node_referenced(&self, id: NodeId) -> bool {
        self.adjacency.get(&id).is_some_and(|a| !a.is_empty())
            || self.records.values().any(
                |r| matches!(r.as_ref(), StrategicRecord::Dike{plan,..} if plan.references(id)),
            )
    }
    fn index(&mut self, edge: &StrategicEdge, insert: bool) {
        let (from, to) = edge.adjacency_pair();
        for (id, adj) in [(edge.from, from), (edge.to, to)] {
            let mut list = self
                .adjacency
                .get(&id)
                .map(|a| (**a).clone())
                .unwrap_or_default();
            if insert {
                list.insert(edge.id, adj);
            } else {
                list.remove(&edge.id);
            }
            if list.is_empty() {
                self.adjacency.remove(&id);
            } else {
                self.adjacency.insert(id, Arc::new(list));
            }
        }
    }
    pub fn put(&mut self, nodes: &Nodes, record: &StrategicRecord) -> Result<(), StorageError> {
        self.put_inner(nodes, record, false)
    }
    #[cfg(feature = "rocksdb-backend")]
    pub fn load(&mut self, nodes: &Nodes, record: &StrategicRecord) -> Result<(), StorageError> {
        self.put_inner(nodes, record, true)
    }
    fn put_inner(
        &mut self,
        nodes: &Nodes,
        record: &StrategicRecord,
        loading: bool,
    ) -> Result<(), StorageError> {
        let key = record.key();
        if key.id.iter().all(|b| *b == 0) {
            return Err(rejected("nil strategic record id"));
        }
        match record {
            StrategicRecord::Neural {
                game, checkpoint, ..
            } => {
                let game = machiavelli_cfr::extensive::Game::new(game.as_ref().clone())
                    .map_err(|_| rejected("invalid neural game"))?;
                machiavelli_cfr::extensive::deep::validate(&game, checkpoint)
                    .map_err(|_| rejected("invalid neural checkpoint"))?;
            }
            StrategicRecord::Belief {
                node,
                observation_id,
                observed_at_ms,
                particles,
                weights,
            } => {
                let subject = nodes
                    .get(node)
                    .ok_or_else(|| rejected("belief subject absent"))?;
                if observation_id.is_empty()
                    || observation_id.len() > 128
                    || *observed_at_ms == 0
                    || particles.first().is_none_or(|p| {
                        p.dim() != subject.phase.dim() || p.c() != subject.phase.c()
                    })
                {
                    return Err(rejected("invalid belief observation identity/shape"));
                }
                machiavelli_fog::ParticleFilter::new(particles.clone(), weights, 0)
                    .map_err(|_| rejected("invalid belief particles"))?;
                if weights.iter().any(|w| !w.is_finite() || *w < 0.0)
                    || (weights.iter().sum::<f64>() - 1.0).abs() > 1e-12
                {
                    return Err(rejected("belief weights must be normalized"));
                }
                if let Some(StrategicRecord::Belief {
                    observed_at_ms: before,
                    ..
                }) = self.record(&key)
                    && observed_at_ms <= before
                {
                    return Err(rejected("observation time must advance"));
                }
            }
            StrategicRecord::Sync { entry } => {
                entry.validate()?;
                if let Some(StrategicRecord::Sync { entry: before }) = self.record(&key) {
                    entry.transition_from(before)?;
                }
            }
            StrategicRecord::Edge { edge } => {
                if !edge.weight.is_finite() || edge.from == edge.to {
                    return Err(rejected("invalid edge weight or self-loop"));
                }
                if loading {
                    if nodes.get(&edge.from).is_none() || nodes.get(&edge.to).is_none() {
                        return Err(rejected("missing edge endpoint"));
                    }
                } else {
                    edge_mutable(nodes, edge)?;
                }
                if let Some(StrategicRecord::Edge { edge: old }) = self.record(&key) {
                    if !loading {
                        edge_mutable(nodes, old)?;
                    }
                    let old = *old;
                    self.index(&old, false);
                }
                self.index(edge, true);
            }
            StrategicRecord::Payoff { edge, matrix } => {
                if !loading
                    && let Some(StrategicRecord::Edge { edge }) =
                        self.record(&RecordKey::new(Family::Edges, edge.as_bytes()))
                {
                    edge_mutable(nodes, edge)?;
                }
                matrix.validate()?;
                if self
                    .record(&RecordKey::new(Family::Edges, edge.as_bytes()))
                    .is_none()
                {
                    return Err(rejected("payoff requires existing edge"));
                }
            }
            StrategicRecord::Infoset {
                id,
                player,
                actions,
            } => {
                if *player > 1
                    || actions.is_empty()
                    || actions.len() > 4096
                    || actions.iter().any(|a| a.trim().is_empty() || a.len() > 256)
                    || actions.iter().collect::<BTreeSet<_>>().len() != actions.len()
                {
                    return Err(rejected("invalid information set"));
                }
                if self
                    .record(&RecordKey::new(Family::Regrets, id.as_bytes()))
                    .is_some()
                    && self.record(&key) != Some(record)
                {
                    return Err(rejected("remove regrets before changing information set"));
                }
            }
            StrategicRecord::Regrets {
                id,
                regrets,
                strategy_sum,
                ..
            } => {
                let Some(StrategicRecord::Infoset { actions, .. }) =
                    self.record(&RecordKey::new(Family::Infosets, id.as_bytes()))
                else {
                    return Err(rejected("regrets require information set"));
                };
                if regrets.len() != actions.len()
                    || strategy_sum.len() != actions.len()
                    || regrets
                        .iter()
                        .chain(strategy_sum)
                        .any(|v| !v.is_finite() || *v < 0.0)
                {
                    return Err(rejected("invalid regret table"));
                }
            }
            StrategicRecord::Dike { plan, .. } => {
                plan.validate().map_err(rejected)?;
                if nodes.get(&plan.subject()).is_none() {
                    return Err(rejected("dike subject does not exist"));
                }
                for action in plan.actions() {
                    let (Action::Isolate(id) | Action::Fortify(id)) = action;
                    if nodes.get(id).is_none() {
                        return Err(rejected("dike action node does not exist"));
                    }
                }
            }
            StrategicRecord::Auction {
                bidders,
                valuations,
                outcome,
                payments,
                welfare,
                ..
            } => {
                if self.record(&key).is_some() {
                    return Err(rejected("auction ledger is append-only"));
                }
                if bidders.len() != valuations.len()
                    || bidders.iter().collect::<BTreeSet<_>>().len() != bidders.len()
                    || bidders.iter().any(|id| id.is_nil())
                {
                    return Err(rejected("invalid auction bidders"));
                }
                let result = machiavelli_vcg::allocate(valuations).map_err(rejected)?;
                if result.outcome != *outcome
                    || result.payments != *payments
                    || result.welfare != *welfare
                {
                    return Err(rejected("auction result does not match VCG calculation"));
                }
            }
        }
        self.records.insert(key, Arc::new(record.clone()));
        Ok(())
    }
    pub fn delete(&mut self, nodes: &Nodes, key: &RecordKey) -> Result<(), StorageError> {
        let record = self
            .record(key)
            .ok_or_else(|| rejected("strategic record does not exist"))?
            .clone();
        match &record {
            StrategicRecord::Edge { edge } => {
                edge_mutable(nodes, edge)?;
                if self
                    .record(&RecordKey::new(Family::Payoffs, edge.id.as_bytes()))
                    .is_some()
                {
                    return Err(rejected("remove payoff before edge"));
                }
                self.index(edge, false);
            }
            StrategicRecord::Payoff { edge, .. } => {
                if let Some(StrategicRecord::Edge { edge }) =
                    self.record(&RecordKey::new(Family::Edges, edge.as_bytes()))
                {
                    edge_mutable(nodes, edge)?;
                }
            }
            StrategicRecord::Infoset { id, .. }
                if self
                    .record(&RecordKey::new(Family::Regrets, id.as_bytes()))
                    .is_some() =>
            {
                return Err(rejected("remove regrets before information set"));
            }
            StrategicRecord::Auction { .. } | StrategicRecord::Sync { .. } => {
                return Err(rejected("auction ledger is append-only"));
            }
            _ => {}
        }
        self.records.remove(key);
        Ok(())
    }
    pub fn operations(&self) -> impl Iterator<Item = Mutation> + '_ {
        // Family ordering ensures edges precede payoffs and infosets precede regrets.
        self.records
            .values()
            .map(|r| Mutation::PutStrategic((**r).clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mutation, Store, backup};
    use machiavelli_core::{ContinentiaMeta, ContinentiaNode, EdgeKind, PhaseVector};
    fn node(name: &str) -> ContinentiaNode {
        ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::derive("strategic", name), 1),
            PhaseVector::zeros(1),
        )
    }
    fn records(a: NodeId, b: NodeId) -> Vec<StrategicRecord> {
        let id = InfosetHash::of(b"test");
        vec![
            StrategicRecord::Edge {
                edge: StrategicEdge {
                    id: EdgeId::derive("edge", "test"),
                    from: a,
                    to: b,
                    kind: EdgeKind::Threat,
                    weight: 0.5,
                },
            },
            StrategicRecord::Payoff {
                edge: EdgeId::derive("edge", "test"),
                matrix: PayoffMatrix::zero_sum(2, 2, vec![1.0, -1.0, -1.0, 1.0]).unwrap(),
            },
            StrategicRecord::Infoset {
                id,
                player: 0,
                actions: vec!["hold".into(), "retreat".into()],
            },
            StrategicRecord::Regrets {
                id,
                regrets: vec![0.3, 0.7],
                strategy_sum: vec![1.2345678901234567, 2.0],
                iterations: 5,
            },
            StrategicRecord::Dike {
                id: TriggerHash::of(b"dike"),
                plan: CompiledDike::new(a, 1.0, 999, 1, 0.5, vec![Action::Isolate(b)]).unwrap(),
            },
            StrategicRecord::Auction {
                id: TriggerHash::of(b"auction"),
                bidders: vec![a, b],
                valuations: vec![vec![0.0, 10.0, 0.0], vec![0.0, 0.0, 7.0]],
                outcome: 1,
                payments: vec![7.0, 0.0],
                welfare: 10.0,
            },
        ]
    }
    #[test]
    fn all_families_roundtrip_with_indexes_and_isolated_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let a = node("a");
        let b = node("b");
        let data = records(a.meta.id, b.meta.id);
        let mut store = Store::open(dir.path()).unwrap();
        let mut ops = vec![Mutation::Put(a.clone()), Mutation::Put(b.clone())];
        ops.extend(data.iter().cloned().map(Mutation::PutStrategic));
        assert_eq!(store.commit(&ops).unwrap(), 1);
        let old = store.snapshot();
        let key = data[0].key();
        let mut branch = old.branch();
        assert_eq!(old.strategic_len(), 6);
        assert!(old.adjacency(a.meta.id).next().unwrap().outgoing);
        assert!(!old.adjacency(b.meta.id).next().unwrap().outgoing);
        let remove = [
            Mutation::DeleteStrategic(data[1].key()),
            Mutation::DeleteStrategic(key.clone()),
        ];
        branch.apply(&remove).unwrap();
        assert!(branch.record(&key).is_none());
        assert!(old.record(&key).is_some());
        drop(store);
        let mut store = Store::open(dir.path()).unwrap();
        for record in &data {
            assert_eq!(store.snapshot().record(&record.key()), Some(record));
        }
        assert_eq!(store.snapshot().adjacency(a.meta.id).count(), 1);
        store.commit(&remove).unwrap();
        assert_eq!(store.snapshot().adjacency(a.meta.id).count(), 0);
        assert_eq!(old.adjacency(a.meta.id).count(), 1);
        drop(store);
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.snapshot().strategic_len(), 4);
        assert_eq!(store.snapshot().sequence(), 2);
    }
    #[test]
    fn invalid_graph_or_solver_batch_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let a = node("a");
        let b = node("b");
        let data = records(a.meta.id, b.meta.id);
        assert!(
            store
                .commit(&[
                    Mutation::Put(a.clone()),
                    Mutation::PutStrategic(data[0].clone())
                ])
                .is_err()
        );
        assert_eq!(store.snapshot().sequence(), 0);
        assert!(store.snapshot().is_empty());
        store
            .commit(&[
                Mutation::Put(a.clone()),
                Mutation::Put(b.clone()),
                Mutation::PutStrategic(data[0].clone()),
            ])
            .unwrap();
        assert!(store.commit(&[Mutation::Delete(a.meta.id)]).is_err());
        let mut invalid = data[3].clone();
        if let StrategicRecord::Regrets { regrets, .. } = &mut invalid {
            regrets[0] = -1.0;
        }
        assert!(
            store
                .commit(&[
                    Mutation::PutStrategic(data[2].clone()),
                    Mutation::PutStrategic(invalid)
                ])
                .is_err()
        );
        assert!(store.snapshot().record(&data[2].key()).is_none());
        assert_eq!(store.snapshot().sequence(), 1);
        drop(store);
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.snapshot().strategic_len(), 1);
    }
    #[test]
    fn auction_ledger_is_verified_and_append_only() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let a = node("a");
        let b = node("b");
        let data = records(a.meta.id, b.meta.id);
        let auction = &data[5];
        let mut bad = auction.clone();
        if let StrategicRecord::Auction { payments, .. } = &mut bad {
            payments[0] = 0.0;
        }
        assert!(store.commit(&[Mutation::PutStrategic(bad)]).is_err());
        store
            .commit(&[Mutation::PutStrategic(auction.clone())])
            .unwrap();
        assert!(
            store
                .commit(&[Mutation::PutStrategic(auction.clone())])
                .is_err()
        );
        assert!(
            store
                .commit(&[Mutation::DeleteStrategic(auction.key())])
                .is_err()
        );
        assert_eq!(store.snapshot().sequence(), 1);
    }
    #[test]
    fn portable_backup_preserves_all_records_and_final_fortified_posture() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("source")).unwrap();
        let mut a = node("a");
        let b = node("b");
        let data = records(a.meta.id, b.meta.id);
        let mut ops = vec![Mutation::Put(a.clone()), Mutation::Put(b.clone())];
        ops.extend(data.iter().cloned().map(Mutation::PutStrategic));
        store.commit(&ops).unwrap();
        a.meta.posture = StrategicPosture::Fortified;
        store.commit(&[Mutation::Put(a.clone())]).unwrap();
        let copy = dir.path().join("backup");
        backup::backup(&store.snapshot(), &copy).unwrap();
        let restored = backup::restore(&copy, dir.path().join("restored")).unwrap();
        assert_eq!(restored.snapshot().get(a.meta.id), Some(&a));
        for record in &data {
            assert_eq!(restored.snapshot().record(&record.key()), Some(record));
        }
        assert_eq!(restored.snapshot().adjacency(a.meta.id).count(), 1);
    }
    #[test]
    fn retargeting_edges_updates_both_sides_and_guards_fortified_nodes() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let mut a = node("a");
        let b = node("b");
        let c = node("c");
        let mut edge = StrategicEdge {
            id: EdgeId::new(),
            from: a.meta.id,
            to: b.meta.id,
            kind: EdgeKind::Threat,
            weight: 1.0,
        };
        let put = |edge| Mutation::PutStrategic(StrategicRecord::Edge { edge });
        store
            .commit(&[
                Mutation::Put(a.clone()),
                Mutation::Put(b.clone()),
                Mutation::Put(c.clone()),
                put(edge),
            ])
            .unwrap();
        edge.to = c.meta.id;
        store.commit(&[put(edge)]).unwrap();
        assert_eq!(store.snapshot().adjacency(b.meta.id).count(), 0);
        assert_eq!(store.snapshot().adjacency(c.meta.id).count(), 1);
        a.meta.posture = StrategicPosture::Fortified;
        store.commit(&[Mutation::Put(a)]).unwrap();
        assert!(
            store
                .commit(&[Mutation::DeleteStrategic(
                    StrategicRecord::Edge { edge }.key()
                )])
                .is_err()
        );
        drop(store);
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.snapshot().adjacency(c.meta.id).count(), 1);
    }
    #[test]
    fn sequential_edge_rewrites_in_one_batch_do_not_leave_stale_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let a = node("a");
        let b = node("b");
        let c = node("c");
        let mut edge = StrategicEdge {
            id: EdgeId::new(),
            from: a.meta.id,
            to: b.meta.id,
            kind: EdgeKind::Threat,
            weight: 1.0,
        };
        let mut ops = vec![
            Mutation::Put(a),
            Mutation::Put(b.clone()),
            Mutation::Put(c.clone()),
            Mutation::PutStrategic(StrategicRecord::Edge { edge }),
        ];
        edge.to = c.meta.id;
        ops.push(Mutation::PutStrategic(StrategicRecord::Edge { edge }));
        store.commit(&ops).unwrap();
        drop(store);
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.snapshot().adjacency(b.meta.id).count(), 0);
        assert_eq!(store.snapshot().adjacency(c.meta.id).count(), 1);
    }
}
