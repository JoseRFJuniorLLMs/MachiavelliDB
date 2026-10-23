//! Single-writer persistent node store backed by an append-only WAL.
//! Immutable Arc snapshots isolate simulations. This is not the planned RocksDB/mmap backend.
//! Fortified mutations fail closed until a cryptographic quorum verifier exists.
pub mod backup;
use machiavelli_core::{ContinentiaMeta, ContinentiaNode, NodeId, PhaseVector, StrategicPosture};
use machiavelli_wal::Wal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

type Nodes = BTreeMap<NodeId, ContinentiaNode>;

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
#[derive(Clone, Debug)]
// Batches own nodes inline; avoid an extra allocation for every put.
#[allow(clippy::large_enum_variant)]
pub enum Mutation {
    Put(ContinentiaNode),
    Delete(NodeId),
}

#[derive(Serialize, Deserialize)]
enum RecordMutation {
    Put { meta: Vec<u8>, phase: Vec<u8> },
    Delete(NodeId),
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
            })
        })
        .collect::<Result<Vec<_>, StorageError>>()?;
    Ok(serde_json::to_vec(&Record {
        version: 1,
        operations,
    })?)
}
fn decode(payload: &[u8]) -> Result<Vec<Mutation>, StorageError> {
    let record: Record = serde_json::from_slice(payload)?;
    if record.version != 1 {
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
            })
        })
        .collect()
}
fn apply(nodes: &mut Nodes, operations: &[Mutation]) -> Result<(), StorageError> {
    if operations.is_empty() {
        return Err(StorageError::Rejected("empty transaction"));
    }
    for op in operations {
        let (id, after) = match op {
            Mutation::Put(node) => {
                node.meta.validate()?;
                node.phase.validate()?;
                (node.meta.id, Some(node))
            }
            Mutation::Delete(id) => (*id, None),
        };
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
                nodes.insert(id, node.clone());
            }
            None => {
                nodes.remove(&id);
            }
        }
    }
    Ok(())
}

/// O(1) immutable snapshot; first mutation copies the map, not OS pages.
#[derive(Clone)]
pub struct Snapshot {
    nodes: Arc<Nodes>,
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
        self.nodes.get(&id)
    }
    pub fn nodes(&self) -> impl Iterator<Item = &ContinentiaNode> {
        self.nodes.values()
    }
    /// A speculative branch never writes the durable store.
    pub fn branch(&self) -> Branch {
        Branch {
            nodes: self.nodes.clone(),
        }
    }
}
pub struct Branch {
    nodes: Arc<Nodes>,
}
impl Branch {
    pub fn get(&self, id: NodeId) -> Option<&ContinentiaNode> {
        self.nodes.get(&id)
    }
    pub fn apply(&mut self, operations: &[Mutation]) -> Result<(), StorageError> {
        let mut next = (*self.nodes).clone();
        apply(&mut next, operations)?;
        self.nodes = Arc::new(next);
        Ok(())
    }
}

pub struct WalStore {
    wal: Wal,
    nodes: Arc<Nodes>,
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
        for payload in records {
            apply(&mut nodes, &decode(&payload)?)?;
        }
        Ok(Self {
            wal,
            nodes: Arc::new(nodes),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            nodes: self.nodes.clone(),
            sequence: self.wal.sequence(),
        }
    }
    pub fn recovered_bytes(&self) -> u64 {
        self.wal.recovered_bytes()
    }
    /// Validate the whole transaction before durable append; publish only after sync.
    pub fn commit(&mut self, operations: &[Mutation]) -> Result<u64, StorageError> {
        let mut next = (*self.nodes).clone();
        apply(&mut next, operations)?;
        let payload = encode(operations)?;
        let sequence = self.wal.append(&payload)?;
        self.nodes = Arc::new(next);
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
        wal.append(br#"{"version":2,"operations":[]}"#).unwrap();
        drop(wal);
        assert!(Store::open(dir.path()).is_err());
    }
}
