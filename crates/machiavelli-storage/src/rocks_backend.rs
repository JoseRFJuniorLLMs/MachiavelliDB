//! RocksDB materialization with synchronous, atomic writes across column families.
use super::*;
use rocksdb::{ColumnFamilyDescriptor, DB, Options, WriteBatch, WriteOptions};
pub const COLUMN_FAMILIES: [&str; 14] = [
    "cf_continentia",
    "cf_phase_space",
    "cf_payoff_tensors",
    "cf_adj_strategic",
    "cf_infosets",
    "cf_regret_tables",
    "cf_dikes",
    "cf_vcg_ledger",
    "cf_deception",
    "cf_invariants",
    "cf_bft_quorum",
    "cf_trinitas_sync",
    "cf_beliefs",
    "cf_neural_models",
];
pub struct RocksStore {
    db: DB,
    nodes: Arc<Nodes>,
    strategy: Arc<Strategy>,
    sequence: u64,
    poisoned: bool,
}
fn adjacency_key(id: NodeId) -> Vec<u8> {
    [b"adj/".as_slice(), id.as_bytes()].concat()
}
impl RocksStore {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, StorageError> {
        if directory.as_ref().join("machiavelli.wal").exists() {
            return Err(StorageError::Rejected(
                "standalone WAL directory requires explicit migration",
            ));
        }
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let db = DB::open_cf_descriptors(
            &options,
            directory,
            COLUMN_FAMILIES
                .iter()
                .map(|name| ColumnFamilyDescriptor::new(*name, Options::default())),
        )?;
        let meta_cf = db.cf_handle("cf_continentia").unwrap();
        let phase_cf = db.cf_handle("cf_phase_space").unwrap();
        let mut nodes = Nodes::new();
        for entry in db.iterator_cf(meta_cf, rocksdb::IteratorMode::Start) {
            let (key, value) = entry?;
            let meta = ContinentiaMeta::decode(&value)?;
            if key.as_ref() != meta.id.as_bytes() || meta.id.is_nil() {
                return Err(StorageError::Rejected("node key mismatch"));
            }
            let phase = db
                .get_cf(phase_cf, &key)?
                .ok_or(StorageError::Rejected("missing phase"))?;
            nodes.insert(
                meta.id,
                Arc::new(ContinentiaNode::new(meta, PhaseVector::decode(&phase)?)),
            );
        }
        let mut count = 0;
        for entry in db.iterator_cf(phase_cf, rocksdb::IteratorMode::Start) {
            entry?;
            count += 1;
        }
        if count != nodes.len() {
            return Err(StorageError::Rejected("orphan phase"));
        }
        let mut strategy = Strategy::default();
        let mut persisted_adjacency = std::collections::BTreeMap::new();
        for family in Family::ALL {
            let cf = db.cf_handle(family.column_family()).unwrap();
            for entry in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
                let (key, value) = entry?;
                if family == Family::Edges && key.len() == 20 && key.starts_with(b"adj/") {
                    let id = NodeId::from_bytes(key[4..].try_into().unwrap());
                    let list: std::collections::BTreeMap<
                        machiavelli_core::EdgeId,
                        machiavelli_core::StrategicAdj,
                    > = serde_json::from_slice(&value)?;
                    persisted_adjacency.insert(id, list);
                    continue;
                }
                let record: StrategicRecord = serde_json::from_slice(&value)?;
                if record.key() != RecordKey::new(family, &key) {
                    return Err(StorageError::Rejected("strategic record key mismatch"));
                }
                strategy.load(&nodes, &record)?;
            }
        }
        if persisted_adjacency.len() != strategy.adjacency.len()
            || strategy
                .adjacency
                .iter()
                .any(|(id, list)| persisted_adjacency.get(id) != Some(list.as_ref()))
        {
            return Err(StorageError::Rejected("adjacency index mismatch"));
        }
        let sequence = match db.get(b"machiavelli/sequence")? {
            Some(bytes) => u64::from_le_bytes(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| StorageError::Rejected("invalid sequence"))?,
            ),
            None if nodes.is_empty() && strategy.records.is_empty() => 0,
            None => return Err(StorageError::Rejected("missing sequence")),
        };
        Ok(Self {
            db,
            nodes: Arc::new(nodes),
            strategy: Arc::new(strategy),
            sequence,
            poisoned: false,
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            nodes: self.nodes.clone(),
            strategy: self.strategy.clone(),
            sequence: self.sequence,
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
        0
    }
    pub fn commit(&mut self, operations: &[Mutation]) -> Result<u64, StorageError> {
        validate_sync_sequence(&self.strategy, operations, self.sequence)?;
        if self.poisoned {
            return Err(StorageError::Rejected(
                "RocksDB writer requires reopen after I/O failure",
            ));
        }
        let mut nodes = (*self.nodes).clone();
        let mut strategy = (*self.strategy).clone();
        apply(&mut nodes, &mut strategy, operations)?;
        if encode(operations)?.len() > machiavelli_wal::MAX_RECORD {
            return Err(StorageError::Rejected("transaction too large"));
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(StorageError::Rejected("sequence exhausted"))?;
        let mut batch = WriteBatch::default();
        let mut affected = std::collections::BTreeSet::new();
        for op in operations {
            match op {
                Mutation::Put(node) => {
                    batch.put_cf(
                        self.db.cf_handle("cf_continentia").unwrap(),
                        node.meta.id.as_bytes(),
                        node.meta.encode(),
                    );
                    batch.put_cf(
                        self.db.cf_handle("cf_phase_space").unwrap(),
                        node.meta.id.as_bytes(),
                        node.phase.encode()?,
                    );
                }
                Mutation::Delete(id) => {
                    batch.delete_cf(self.db.cf_handle("cf_continentia").unwrap(), id.as_bytes());
                    batch.delete_cf(self.db.cf_handle("cf_phase_space").unwrap(), id.as_bytes());
                }
                Mutation::PutStrategic(record) => {
                    let key = record.key();
                    if key.family == Family::Edges {
                        if let Some(StrategicRecord::Edge { edge }) = self.strategy.record(&key) {
                            affected.extend([edge.from, edge.to]);
                        }
                        if let Some(StrategicRecord::Edge { edge }) = strategy.record(&key) {
                            affected.extend([edge.from, edge.to]);
                        }
                    }
                    batch.put_cf(
                        self.db.cf_handle(key.family.column_family()).unwrap(),
                        &key.id,
                        serde_json::to_vec(record)?,
                    );
                }
                Mutation::DeleteStrategic(key) => {
                    if let Some(StrategicRecord::Edge { edge }) = self.strategy.record(key) {
                        affected.extend([edge.from, edge.to]);
                    }
                    batch.delete_cf(
                        self.db.cf_handle(key.family.column_family()).unwrap(),
                        &key.id,
                    );
                }
            }
        }
        // Derive index writes from final state, including repeated updates in one batch.
        for op in operations {
            if let Mutation::PutStrategic(StrategicRecord::Edge { edge }) = op {
                affected.extend([edge.from, edge.to]);
            }
        }
        let cf = self.db.cf_handle("cf_adj_strategic").unwrap();
        for id in affected {
            let key = adjacency_key(id);
            if let Some(list) = strategy.adjacency.get(&id) {
                batch.put_cf(cf, key, serde_json::to_vec(list.as_ref())?);
            } else {
                batch.delete_cf(cf, key);
            }
        }
        batch.put(b"machiavelli/sequence", sequence.to_le_bytes());
        let mut options = WriteOptions::default();
        options.set_sync(true);
        self.poisoned = true;
        self.db.write_opt(batch, &options)?;
        self.poisoned = false;
        self.nodes = Arc::new(nodes);
        self.strategy = Arc::new(strategy);
        self.sequence = sequence;
        Ok(sequence)
    }
}
#[cfg(test)]
mod integrity_tests {
    use super::*;
    use machiavelli_core::{EdgeId, EdgeKind, StrategicEdge};
    #[test]
    fn corrupted_materialized_adjacency_is_rejected_on_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RocksStore::open(dir.path()).unwrap();
        let a = ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::new(), 1),
            PhaseVector::zeros(1),
        );
        let b = ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::new(), 1),
            PhaseVector::zeros(1),
        );
        let edge = StrategicEdge {
            id: EdgeId::new(),
            from: a.meta.id,
            to: b.meta.id,
            kind: EdgeKind::Threat,
            weight: 1.0,
        };
        store
            .commit(&[
                Mutation::Put(a.clone()),
                Mutation::Put(b),
                Mutation::PutStrategic(StrategicRecord::Edge { edge }),
            ])
            .unwrap();
        drop(store);
        let db = DB::open_cf(&Options::default(), dir.path(), COLUMN_FAMILIES).unwrap();
        db.delete_cf(
            db.cf_handle("cf_adj_strategic").unwrap(),
            adjacency_key(a.meta.id),
        )
        .unwrap();
        drop(db);
        assert!(matches!(
            RocksStore::open(dir.path()),
            Err(StorageError::Rejected("adjacency index mismatch"))
        ));
    }
}
