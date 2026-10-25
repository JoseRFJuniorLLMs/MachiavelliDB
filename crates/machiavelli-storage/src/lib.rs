//! Single-writer persistent node store backed by an append-only WAL.
//! Immutable Arc snapshots isolate simulations. This is not the planned RocksDB/mmap backend.
//! Fortified mutations fail closed until a cryptographic quorum verifier exists.
pub mod backup;
pub mod strategic;
pub mod sync;
use machiavelli_core::{ContinentiaMeta, ContinentiaNode, NodeId, PhaseVector, StrategicPosture};
use machiavelli_wal::Wal;
use serde::{Deserialize, Serialize};
use strategic::Strategy;
pub use strategic::{Family, RecordKey, StrategicRecord};
mod map;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

type Nodes = map::Map<NodeId, Arc<ContinentiaNode>>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[cfg(feature = "rocksdb-backend")]
    #[error(transparent)]
    Rocks(#[from] rocksdb::Error),
    #[error(transparent)]
    Wal(#[from] machiavelli_wal::WalError),
    #[error(transparent)]
    Core(#[from] machiavelli_core::CoreError),
    #[error(transparent)]
    Codec(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("transition rejected: {0}")]
    Rejected(&'static str),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
// Batches own nodes inline; avoid an extra allocation for every put.
#[allow(clippy::large_enum_variant)]
pub enum Mutation {
    Put(ContinentiaNode),
    Delete(NodeId),
    PutStrategic(StrategicRecord),
    DeleteStrategic(RecordKey),
}

#[derive(Serialize, Deserialize)]
// Transient encode/decode form; mirrors Mutation.
#[allow(clippy::large_enum_variant)]
enum RecordMutation {
    Put { meta: Vec<u8>, phase: Vec<u8> },
    Delete(NodeId),
    PutStrategic(StrategicRecord),
    DeleteStrategic(RecordKey),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    operations: Vec<RecordMutation>,
}

fn encode(operations: &[Mutation]) -> Result<Vec<u8>, StorageError> {
    let operations = operations
        .iter()
        .map(|op| {
            Ok(match op {
                Mutation::Put(node) => RecordMutation::Put {
                    meta: node.meta.encode().to_vec(),
                    phase: node.phase.encode()?,
                },
                Mutation::Delete(id) => RecordMutation::Delete(*id),
                Mutation::PutStrategic(record) => RecordMutation::PutStrategic(record.clone()),
                Mutation::DeleteStrategic(key) => RecordMutation::DeleteStrategic(key.clone()),
            })
        })
        .collect::<Result<Vec<_>, StorageError>>()?;
    Ok(serde_json::to_vec(&Record {
        version: 2,
        operations,
    })?)
}
fn decode(payload: &[u8]) -> Result<Vec<Mutation>, StorageError> {
    let record: Record = serde_json::from_slice(payload)?;
    if !matches!(record.version, 1 | 2) {
        return Err(StorageError::Rejected("unknown storage version"));
    }
    record
        .operations
        .into_iter()
        .map(|op| {
            Ok(match op {
                RecordMutation::Put { meta, phase } => Mutation::Put(ContinentiaNode::new(
                    ContinentiaMeta::decode(&meta)?,
                    PhaseVector::decode(&phase)?,
                )),
                RecordMutation::Delete(id) => Mutation::Delete(id),
                RecordMutation::PutStrategic(record) => Mutation::PutStrategic(record),
                RecordMutation::DeleteStrategic(key) => Mutation::DeleteStrategic(key),
            })
        })
        .collect()
}
fn apply(
    nodes: &mut Nodes,
    strategy: &mut Strategy,
    operations: &[Mutation],
) -> Result<(), StorageError> {
    if operations.is_empty() {
        return Err(StorageError::Rejected("empty transaction"));
    }
    validate_sync_batch(strategy, operations)?;
    for op in operations {
        let (id, after) = match op {
            Mutation::Put(node) => {
                node.meta.validate()?;
                node.phase.validate()?;
                (node.meta.id, Some(node))
            }
            Mutation::Delete(id) => (*id, None),
            Mutation::PutStrategic(record) => {
                strategy.put(nodes, record)?;
                continue;
            }
            Mutation::DeleteStrategic(key) => {
                strategy.delete(nodes, key)?;
                continue;
            }
        };
        if after.is_none() && strategy.node_referenced(id) {
            return Err(StorageError::Rejected(
                "remove incident edges and dikes before node",
            ));
        }
        if id.is_nil() {
            return Err(StorageError::Rejected("nil node id"));
        }
        if let Some(before) = nodes.get(&id) {
            if before.meta.posture == StrategicPosture::Fortified {
                return Err(StorageError::Rejected(
                    "fortified node requires verified quorum",
                ));
            }
            if before.meta.posture == StrategicPosture::Betrayed {
                return Err(StorageError::Rejected("betrayed node is frozen"));
            }
            match after {
                Some(next) => {
                    if before.phase.c() != next.phase.c() {
                        return Err(StorageError::Rejected(
                            "constitutional coordinates are immutable",
                        ));
                    }
                    if before.phase.dim() != next.phase.dim() {
                        return Err(StorageError::Rejected("phase dimension is immutable"));
                    }
                }
                None if !before.phase.c().is_empty() => {
                    return Err(StorageError::Rejected("cannot delete constitutional node"));
                }
                _ => {}
            }
        } else if after.is_none() {
            return Err(StorageError::Rejected("node does not exist"));
        }
        match after {
            Some(node) => {
                nodes.insert(id, Arc::new(node.clone()));
            }
            None => {
                nodes.remove(&id);
            }
        }
    }
    Ok(())
}

fn validate_sync_batch(strategy: &Strategy, operations: &[Mutation]) -> Result<(), StorageError> {
    let pending = strategy.records.values().find_map(|r| match r.as_ref() {
        StrategicRecord::Sync { entry } if entry.result.is_none() => Some(entry),
        _ => None,
    });
    let updates: Vec<_> = operations
        .iter()
        .filter_map(|op| match op {
            Mutation::PutStrategic(StrategicRecord::Sync { entry }) => Some(entry),
            _ => None,
        })
        .collect();
    if updates.len() > 1 {
        return Err(StorageError::Rejected(
            "only one Trinitas transition per commit",
        ));
    }
    if let Some(before) = pending {
        let after = updates.first().ok_or(StorageError::Rejected(
            "Trinitas recovery pending; writes fenced",
        ))?;
        if after.operation_id != before.operation_id || after.result.is_none() {
            return Err(StorageError::Rejected(
                "pending Trinitas operation must be resolved first",
            ));
        }
        let expected: Vec<Mutation> = serde_json::from_slice(&before.operations_json)?;
        let effects: Vec<_> = operations
            .iter()
            .filter(|op| !matches!(op, Mutation::PutStrategic(StrategicRecord::Sync { .. })))
            .cloned()
            .collect();
        let applied = after.result.as_ref().unwrap().applied;
        if applied && serde_json::to_vec(&expected)? != serde_json::to_vec(&effects)?
            || !applied && !effects.is_empty()
        {
            return Err(StorageError::Rejected("Trinitas result/effects mismatch"));
        }
    } else if let Some(entry) = updates.first() {
        if operations.len() != 1 {
            return Err(StorageError::Rejected(
                "outbox creation/ack must be isolated",
            ));
        }
        if strategy
            .record(
                &StrategicRecord::Sync {
                    entry: (*entry).clone(),
                }
                .key(),
            )
            .is_none()
            && (entry.result.is_some() || entry.receipt.is_some() || entry.acknowledged)
        {
            return Err(StorageError::Rejected("new outbox entries must be pending"));
        }
    }
    Ok(())
}
fn validate_sync_sequence(
    strategy: &Strategy,
    operations: &[Mutation],
    sequence: u64,
) -> Result<(), StorageError> {
    for op in operations {
        if let Mutation::PutStrategic(record @ StrategicRecord::Sync { entry }) = op {
            match strategy.record(&record.key()) {
                None if entry.source_sequence != sequence => {
                    return Err(StorageError::Rejected("outbox source sequence mismatch"));
                }
                Some(StrategicRecord::Sync { entry: before })
                    if before.result.is_none()
                        && (entry.source_sequence.checked_add(1) != Some(sequence)
                            || entry.result.as_ref().map(|r| r.local_sequence)
                                != sequence.checked_add(1)) =>
                {
                    return Err(StorageError::Rejected("outbox result sequence mismatch"));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// O(1) immutable snapshot; mutations share untouched AVL subtrees and node payloads.
#[derive(Clone)]
pub struct Snapshot {
    nodes: Arc<Nodes>,
    strategy: Arc<Strategy>,
    sequence: u64,
}
impl Snapshot {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn get(&self, id: NodeId) -> Option<&ContinentiaNode> {
        self.nodes.get(&id).map(AsRef::as_ref)
    }
    pub fn nodes(&self) -> impl Iterator<Item = &ContinentiaNode> {
        self.nodes.values().map(AsRef::as_ref)
    }
    pub fn strategic_len(&self) -> usize {
        self.strategy.records.len()
    }
    pub fn record(&self, key: &RecordKey) -> Option<&StrategicRecord> {
        self.strategy.record(key)
    }
    pub fn records(&self) -> impl Iterator<Item = &StrategicRecord> {
        self.strategy.records.values().map(AsRef::as_ref)
    }
    pub fn adjacency(&self, id: NodeId) -> impl Iterator<Item = &machiavelli_core::StrategicAdj> {
        self.strategy
            .adjacency
            .get(&id)
            .into_iter()
            .flat_map(|a| a.values())
    }
    /// A speculative branch never writes the durable store.
    pub fn branch(&self) -> Branch {
        Branch {
            nodes: self.nodes.clone(),
            strategy: self.strategy.clone(),
        }
    }
}
pub struct Branch {
    nodes: Arc<Nodes>,
    strategy: Arc<Strategy>,
}
impl Branch {
    pub fn record(&self, key: &RecordKey) -> Option<&StrategicRecord> {
        self.strategy.record(key)
    }
    pub fn get(&self, id: NodeId) -> Option<&ContinentiaNode> {
        self.nodes.get(&id).map(AsRef::as_ref)
    }
    pub fn apply(&mut self, operations: &[Mutation]) -> Result<(), StorageError> {
        let mut next = (*self.nodes).clone();
        let mut strategy = (*self.strategy).clone();
        apply(&mut next, &mut strategy, operations)?;
        self.nodes = Arc::new(next);
        self.strategy = Arc::new(strategy);
        Ok(())
    }
}

pub struct WalStore {
    wal: Wal,
    nodes: Arc<Nodes>,
    strategy: Arc<Strategy>,
}
impl WalStore {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, StorageError> {
        if directory.as_ref().join("CURRENT").exists() {
            return Err(StorageError::Rejected(
                "RocksDB directory requires rocksdb-backend feature",
            ));
        }
        std::fs::create_dir_all(directory.as_ref())?;
        let (wal, records) = Wal::open(directory.as_ref().join("machiavelli.wal"))?;
        let mut nodes = Nodes::new();
        let mut strategy = Strategy::default();
        for (sequence, payload) in records.into_iter().enumerate() {
            let operations = decode(&payload)?;
            validate_sync_sequence(&strategy, &operations, sequence as u64)?;
            apply(&mut nodes, &mut strategy, &operations)?;
        }
        Ok(Self {
            wal,
            nodes: Arc::new(nodes),
            strategy: Arc::new(strategy),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            nodes: self.nodes.clone(),
            strategy: self.strategy.clone(),
            sequence: self.wal.sequence(),
        }
    }
    /// Compare-and-commit while holding the store's single-writer handle.
    pub fn commit_at(
        &mut self,
        expected_sequence: u64,
        operations: &[Mutation],
    ) -> Result<u64, StorageError> {
        if self.snapshot().sequence() != expected_sequence {
            return Err(StorageError::Rejected(
                "snapshot changed; refresh before committing",
            ));
        }
        self.commit(operations)
    }
    pub fn recovered_bytes(&self) -> u64 {
        self.wal.recovered_bytes()
    }
    /// Validate the whole transaction before durable append; publish only after sync.
    pub fn commit(&mut self, operations: &[Mutation]) -> Result<u64, StorageError> {
        validate_sync_sequence(&self.strategy, operations, self.wal.sequence())?;
        let mut next = (*self.nodes).clone();
        let mut strategy = (*self.strategy).clone();
        apply(&mut next, &mut strategy, operations)?;
        let payload = encode(operations)?;
        let sequence = self.wal.append(&payload)?;
        self.nodes = Arc::new(next);
        self.strategy = Arc::new(strategy);
        Ok(sequence)
    }
}

#[cfg(feature = "rocksdb-backend")]
pub mod rocks_backend;
#[cfg(feature = "rocksdb-backend")]
pub use rocks_backend::RocksStore as Store;
#[cfg(not(feature = "rocksdb-backend"))]
pub type Store = WalStore;

#[cfg(test)]
mod tests {
    use super::*;
    fn node(name: &str) -> ContinentiaNode {
        ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::derive("test", name), 1),
            PhaseVector::new(vec![0.3], vec![0.7]).unwrap(),
        )
    }
    #[test]
    fn durable_roundtrip_retains_exact_binary_state() {
        let dir = tempfile::tempdir().unwrap();
        let original = node("one");
        let mut store = Store::open(dir.path()).unwrap();
        assert_eq!(store.commit(&[Mutation::Put(original.clone())]).unwrap(), 1);
        drop(store);
        let mut store = Store::open(dir.path()).unwrap();
        assert_eq!(store.snapshot().get(original.meta.id), Some(&original));
        assert_eq!(
            store.commit(&[Mutation::Delete(original.meta.id)]).unwrap(),
            2
        );
        drop(store);
        assert!(Store::open(dir.path()).unwrap().snapshot().is_empty());
    }
    #[test]
    fn invalid_batch_is_atomic_and_never_replayed() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let valid = node("one");
        let mut invalid = node("two");
        invalid.meta.virtu = f32::NAN;
        assert!(
            store
                .commit(&[Mutation::Put(valid.clone()), Mutation::Put(invalid)])
                .is_err()
        );
        assert!(store.snapshot().is_empty());
        assert_eq!(store.snapshot().sequence(), 0);
        drop(store);
        assert!(Store::open(dir.path()).unwrap().snapshot().is_empty());
    }
    #[test]
    fn snapshot_and_speculation_never_change_live_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let original = node("one");
        store.commit(&[Mutation::Put(original.clone())]).unwrap();
        let snapshot = store.snapshot();
        let mut branch = snapshot.branch();
        let mut changed = original.clone();
        changed.phase.q_mut()[0] = 99.0;
        branch.apply(&[Mutation::Put(changed.clone())]).unwrap();
        assert_eq!(store.snapshot().get(original.meta.id), Some(&original));
        assert_eq!(snapshot.get(original.meta.id), Some(&original));
        store.commit(&[Mutation::Put(changed)]).unwrap();
        assert_eq!(snapshot.get(original.meta.id), Some(&original));
        assert_eq!(branch.get(original.meta.id).unwrap().phase.q(), &[99.0]);
    }
    #[test]
    fn constitutional_and_fortified_transitions_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let mut original = node("constitution");
        original.phase = PhaseVector::with_constitution(vec![0.0], vec![0.0], vec![1.0]).unwrap();
        store.commit(&[Mutation::Put(original.clone())]).unwrap();
        let mut changed = original.clone();
        changed.phase = PhaseVector::with_constitution(vec![0.0], vec![0.0], vec![2.0]).unwrap();
        assert!(store.commit(&[Mutation::Put(changed)]).is_err());
        assert!(store.commit(&[Mutation::Delete(original.meta.id)]).is_err());
        original.meta.posture = StrategicPosture::Fortified;
        store.commit(&[Mutation::Put(original.clone())]).unwrap();
        assert!(store.commit(&[Mutation::Put(original.clone())]).is_err());
        assert!(store.commit(&[Mutation::Delete(original.meta.id)]).is_err());
    }
    #[test]
    #[cfg(not(feature = "rocksdb-backend"))]
    fn corrupted_payload_and_unknown_versions_fail_on_replay() {
        let dir = tempfile::tempdir().unwrap();
        let (mut wal, _) = Wal::open(dir.path().join("machiavelli.wal")).unwrap();
        wal.append(br#"{"version":999,"operations":[]}"#).unwrap();
        drop(wal);
        assert!(Store::open(dir.path()).is_err());
    }
}
#[cfg(all(test, not(feature = "rocksdb-backend")))]
mod legacy_tests {
    use super::*;
    #[test]
    fn version_one_wal_and_backup_remain_readable() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let node = ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::new(), 1),
            PhaseVector::zeros(1),
        );
        let payload = serde_json::to_vec(&Record {
            version: 1,
            operations: vec![RecordMutation::Put {
                meta: node.meta.encode().to_vec(),
                phase: node.phase.encode().unwrap(),
            }],
        })
        .unwrap();
        let (mut wal, _) = Wal::open(source.join("machiavelli.wal")).unwrap();
        wal.append(&payload).unwrap();
        drop(wal);
        let store = WalStore::open(&source).unwrap();
        assert_eq!(store.snapshot().get(node.meta.id), Some(&node));
        drop(store);
        let digest = blake3::hash(&std::fs::read(source.join("machiavelli.wal")).unwrap())
            .to_hex()
            .to_string();
        std::fs::write(
            source.join("COMPLETE"),
            serde_json::to_vec(
                &serde_json::json!({"version":1,"nodes":1,"source_sequence":1,"wal_digest":digest}),
            )
            .unwrap(),
        )
        .unwrap();
        let restored = backup::restore(&source, dir.path().join("restored")).unwrap();
        assert_eq!(restored.snapshot().get(node.meta.id), Some(&node));
    }
}
