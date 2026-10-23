//! RocksDB materialization with the twelve specified column families.
use super::*;
use rocksdb::{ColumnFamilyDescriptor, DB, Options, WriteBatch, WriteOptions};
pub const COLUMN_FAMILIES: [&str; 12] = [
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
];
pub struct RocksStore {
    db: DB,
    nodes: Arc<Nodes>,
    sequence: u64,
    poisoned: bool,
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
            if key.as_ref() != meta.id.as_bytes() {
                return Err(StorageError::Rejected("node key mismatch"));
            }
            let phase = db
                .get_cf(phase_cf, &key)?
                .ok_or(StorageError::Rejected("missing phase"))?;
            let node = ContinentiaNode::new(meta, PhaseVector::decode(&phase)?);
            nodes.insert(meta.id, node);
        }
        // Orphan phase records indicate incompatible/corrupted state.
        let mut count = 0;
        for entry in db.iterator_cf(phase_cf, rocksdb::IteratorMode::Start) {
            entry?;
            count += 1;
        }
        if count != nodes.len() {
            return Err(StorageError::Rejected("orphan phase"));
        }
        let sequence = match db.get(b"machiavelli/sequence")? {
            Some(bytes) => u64::from_le_bytes(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| StorageError::Rejected("invalid sequence"))?,
            ),
            None if nodes.is_empty() => 0,
            None => return Err(StorageError::Rejected("missing sequence")),
        };
        Ok(Self {
            db,
            nodes: Arc::new(nodes),
            sequence,
            poisoned: false,
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            nodes: self.nodes.clone(),
            sequence: self.sequence,
        }
    }
    pub fn recovered_bytes(&self) -> u64 {
        0
    }
    pub fn commit(&mut self, operations: &[Mutation]) -> Result<u64, StorageError> {
        if self.poisoned {
            return Err(StorageError::Rejected(
                "RocksDB writer requires reopen after I/O failure",
            ));
        }
        let mut next = (*self.nodes).clone();
        apply(&mut next, operations)?;
        // Retain identical transaction limits to the standalone WAL backend.
        if encode(operations)?.len() > machiavelli_wal::MAX_RECORD {
            return Err(StorageError::Rejected("transaction too large"));
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(StorageError::Rejected("sequence exhausted"))?;
        let mut batch = WriteBatch::default();
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
            }
        }
        batch.put(b"machiavelli/sequence", sequence.to_le_bytes());
        let mut options = WriteOptions::default();
        options.set_sync(true);
        self.poisoned = true;
        self.db.write_opt(batch, &options)?;
        self.poisoned = false;
        self.nodes = Arc::new(next);
        self.sequence = sequence;
        Ok(sequence)
    }
}
