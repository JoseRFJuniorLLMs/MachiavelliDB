//! Durable bounded frames. Only incomplete final frames are recovered automatically.
//! Complete frames with bad magic, sequence, size or digest fail closed.
//! A sync error has an ambiguous outcome; discard the handle and reopen before retrying.
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

const HEADER: usize = 80;
pub const MAX_RECORD: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum WalError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("database is locked by another writer")]
    Locked,
    #[error("corrupt WAL at offset {0}")]
    Corrupt(u64),
    #[error("record exceeds 16 MiB or is empty")]
    InvalidSize,
    #[error("writer poisoned after an I/O failure; reopen before retrying")]
    Poisoned,
    #[error("sequence exhausted")]
    SequenceExhausted,
}

pub struct Wal {
    file: File,
    sequence: u64,
    poisoned: bool,
    recovered_bytes: u64,
}

fn digest(length: u32, sequence: u64, payload: &[u8]) -> blake3::Hash {
    let mut hash = blake3::Hasher::new();
    hash.update(b"MCV1");
    hash.update(&length.to_le_bytes());
    hash.update(&sequence.to_le_bytes());
    hash.update(payload);
    hash.finalize()
}

impl Wal {
    /// Opens one writer with an OS file lock released automatically on crash.
    /// The returned records are complete, verified payloads in commit order.
    pub fn open(path: impl AsRef<Path>) -> Result<(Self, Vec<Vec<u8>>), WalError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.try_lock().map_err(|_| WalError::Locked)?;
        let size = file.metadata()?.len();
        let mut offset = 0;
        let mut sequence = 0u64;
        let mut records = Vec::new();
        while offset < size {
            if size - offset < HEADER as u64 {
                break;
            }
            let mut header = [0u8; HEADER];
            file.read_exact(&mut header)?;
            if &header[..4] != b"MCV1" {
                return Err(WalError::Corrupt(offset));
            }
            if blake3::hash(&header[..16]).as_bytes() != &header[16..48] {
                return Err(WalError::Corrupt(offset));
            }
            let length = u32::from_le_bytes(header[4..8].try_into().unwrap());
            let seq = u64::from_le_bytes(header[8..16].try_into().unwrap());
            if length == 0 || length as usize > MAX_RECORD || Some(seq) != sequence.checked_add(1) {
                return Err(WalError::Corrupt(offset));
            }
            if size - offset - (HEADER as u64) < length as u64 {
                break;
            }
            let mut payload = vec![0; length as usize];
            file.read_exact(&mut payload)?;
            if digest(length, seq, &payload).as_bytes() != &header[48..80] {
                return Err(WalError::Corrupt(offset));
            }
            records.push(payload);
            sequence = seq;
            offset += HEADER as u64 + length as u64;
        }
        let recovered_bytes = size - offset;
        if recovered_bytes != 0 {
            file.set_len(offset)?;
            file.sync_all()?;
        }
        file.seek(SeekFrom::End(0))?;
        Ok((
            Self {
                file,
                sequence,
                poisoned: false,
                recovered_bytes,
            },
            records,
        ))
    }

    /// Returns only after sync_all succeeds. No later append is allowed after an error.
    pub fn append(&mut self, payload: &[u8]) -> Result<u64, WalError> {
        if self.poisoned {
            return Err(WalError::Poisoned);
        }
        if payload.is_empty() || payload.len() > MAX_RECORD {
            return Err(WalError::InvalidSize);
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(WalError::SequenceExhausted)?;
        let length = payload.len() as u32;
        let mut frame = Vec::with_capacity(HEADER + payload.len());
        frame.extend_from_slice(b"MCV1");
        frame.extend_from_slice(&length.to_le_bytes());
        frame.extend_from_slice(&sequence.to_le_bytes());
        frame.extend_from_slice(blake3::hash(&frame).as_bytes());
        frame.extend_from_slice(digest(length, sequence, payload).as_bytes());
        frame.extend_from_slice(payload);
        self.poisoned = true;
        self.file.write_all(&frame)?;
        self.file.sync_all()?;
        self.sequence = sequence;
        self.poisoned = false;
        Ok(sequence)
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn recovered_bytes(&self) -> u64 {
        self.recovered_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writer_lock_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wal");
        let (mut wal, records) = Wal::open(&path).unwrap();
        assert!(records.is_empty());
        assert!(matches!(Wal::open(&path), Err(WalError::Locked)));
        assert_eq!(wal.append(b"first").unwrap(), 1);
        assert_eq!(wal.append(b"second").unwrap(), 2);
        drop(wal);
        let (wal, records) = Wal::open(path).unwrap();
        assert_eq!(records, vec![b"first".to_vec(), b"second".to_vec()]);
        assert_eq!(wal.sequence(), 2);
    }
    #[test]
    fn every_torn_suffix_recovers_only_committed_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wal");
        let (mut wal, _) = Wal::open(&path).unwrap();
        wal.append(b"committed").unwrap();
        drop(wal);
        let prefix = std::fs::read(&path).unwrap();
        let (mut wal, _) = Wal::open(&path).unwrap();
        wal.append(b"next").unwrap();
        drop(wal);
        let full = std::fs::read(&path).unwrap();
        for end in prefix.len() + 1..full.len() {
            std::fs::write(&path, &full[..end]).unwrap();
            let (mut wal, records) = Wal::open(&path).unwrap();
            assert_eq!(records, vec![b"committed".to_vec()]);
            assert_eq!(wal.recovered_bytes(), (end - prefix.len()) as u64);
            assert_eq!(wal.append(b"replacement").unwrap(), 2);
        }
    }
    #[test]
    fn complete_corruption_fails_without_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wal");
        let (mut wal, _) = Wal::open(&path).unwrap();
        wal.append(b"payload").unwrap();
        drop(wal);
        let original = std::fs::read(&path).unwrap();
        for byte in [0, 4, 8, 16, 48, 80] {
            let mut broken = original.clone();
            broken[byte] ^= 0x80;
            std::fs::write(&path, &broken).unwrap();
            assert!(Wal::open(&path).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), broken);
        }
    }
    #[test]
    fn invalid_records_do_not_advance_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let (mut wal, _) = Wal::open(dir.path().join("wal")).unwrap();
        assert!(wal.append(b"").is_err());
        assert!(wal.append(&vec![0; MAX_RECORD + 1]).is_err());
        assert_eq!(wal.append(b"valid").unwrap(), 1);
    }
}
