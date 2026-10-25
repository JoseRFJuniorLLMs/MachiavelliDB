//! Portable node snapshots. Destination directories must not exist.
//! COMPLETE is written last. Incomplete backups/restores are left for inspection.
use super::*;
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    nodes: usize,
    #[serde(default)]
    strategic_records: usize,
    source_sequence: u64,
    wal_digest: String,
}
fn write_synced(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), StorageError> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn file_digest(path: impl AsRef<Path>) -> Result<String, StorageError> {
    use std::io::Read;
    let mut input = std::fs::File::open(path)?;
    let mut hash = blake3::Hasher::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hash.finalize().to_hex().to_string())
}
// Build a new, locked destination and publish its completion marker only after
// restoring final postures. This preserves edges attached before fortification.
fn populate(
    snapshot: &Snapshot,
    mut commit: impl FnMut(&[Mutation]) -> Result<u64, StorageError>,
) -> Result<(), StorageError> {
    for node in snapshot.nodes() {
        let mut initial = node.clone();
        if matches!(
            initial.meta.posture,
            StrategicPosture::Fortified | StrategicPosture::Betrayed
        ) {
            initial.meta.posture = StrategicPosture::Expanding;
        }
        commit(&[Mutation::Put(initial)])?;
    }
    for operation in snapshot.strategy.operations() {
        commit(&[operation])?;
    }
    for node in snapshot.nodes() {
        if matches!(
            node.meta.posture,
            StrategicPosture::Fortified | StrategicPosture::Betrayed
        ) {
            commit(&[Mutation::Put(node.clone())])?;
        }
    }
    Ok(())
}
pub fn backup(snapshot: &Snapshot, destination: impl AsRef<Path>) -> Result<(), StorageError> {
    // A portable snapshot changes sequence and cannot clone an active consumer
    // journal safely. Use the native RocksDB directory for disaster recovery.
    if snapshot
        .records()
        .any(|r| matches!(r, StrategicRecord::Sync { .. }))
    {
        return Err(StorageError::Rejected(
            "portable backup of Trinitas journals requires an explicit identity/sequence migration",
        ));
    }
    let destination = destination.as_ref();
    std::fs::create_dir(destination)?;
    let mut store = WalStore::open(destination)?;
    populate(snapshot, |ops| store.commit(ops))?;
    drop(store);
    let manifest = Manifest {
        version: 2,
        nodes: snapshot.len(),
        strategic_records: snapshot.strategic_len(),
        source_sequence: snapshot.sequence(),
        wal_digest: file_digest(destination.join("machiavelli.wal"))?,
    };
    write_synced(
        destination.join("COMPLETE"),
        &serde_json::to_vec(&manifest)?,
    )?;
    Ok(())
}
pub fn restore(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<Store, StorageError> {
    let source = source.as_ref();
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(source.join("COMPLETE"))?)?;
    if !matches!(manifest.version, 1 | 2)
        || file_digest(source.join("machiavelli.wal"))? != manifest.wal_digest
    {
        return Err(StorageError::Rejected(
            "invalid backup manifest or checksum",
        ));
    }
    let original = WalStore::open(source)?;
    let snapshot = original.snapshot();
    if original.recovered_bytes() != 0
        || snapshot.len() != manifest.nodes
        || snapshot.strategic_len() != manifest.strategic_records
    {
        return Err(StorageError::Rejected("backup count mismatch"));
    }
    let destination = destination.as_ref();
    std::fs::create_dir(destination)?;
    let mut store = Store::open(destination)?;
    populate(&snapshot, |ops| store.commit(ops))?;
    write_synced(
        destination.join("RESTORED"),
        &serde_json::to_vec(&manifest)?,
    )?;
    Ok(store)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_backup_restores_exact_nodes_and_refuses_overwrite_or_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let mut original = Store::open(dir.path().join("original")).unwrap();
        let node = ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::new(), 1),
            PhaseVector::zeros(2),
        );
        original.commit(&[Mutation::Put(node.clone())]).unwrap();
        let copy = dir.path().join("backup");
        backup(&original.snapshot(), &copy).unwrap();
        assert!(backup(&original.snapshot(), &copy).is_err());
        let restored = restore(&copy, dir.path().join("restored")).unwrap();
        assert_eq!(restored.snapshot().get(node.meta.id), Some(&node));
        assert!(restore(&copy, dir.path().join("restored")).is_err());
        std::fs::write(copy.join("machiavelli.wal"), b"broken").unwrap();
        assert!(restore(&copy, dir.path().join("bad")).is_err());
    }
}
