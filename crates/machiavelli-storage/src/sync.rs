//! Durable Trinitas outbox. Effects and their Applied marker share one commit.
use crate::{Mutation, StorageError, StrategicRecord};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncEntry {
    pub operation_id: String,
    pub source_sequence: u64,
    pub expires_ms: u64,
    pub request: serde_json::Value,
    pub operations_json: Vec<u8>,
    pub payload_digest: String,
    pub receipt: Option<serde_json::Value>,
    pub result: Option<SyncResult>,
    pub acknowledged: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncResult {
    pub applied: bool,
    pub local_sequence: u64,
    pub result_digest: String,
}
impl SyncEntry {
    pub fn digest(&self) -> String {
        blake3::hash(
            &serde_json::to_vec(&(
                "machiavelli-external-v1",
                self.source_sequence,
                self.expires_ms,
                &self.request,
                &self.operations_json,
            ))
            .expect("JSON values serialize"),
        )
        .to_hex()
        .to_string()
    }
    pub fn validate(&self) -> Result<(), StorageError> {
        let reject = || StorageError::Rejected("invalid or inconsistent Trinitas entry");
        if self.operation_id.is_empty()
            || self.operation_id.len() > 128
            || self
                .operation_id
                .bytes()
                .any(|b| !b.is_ascii_alphanumeric() && b != b'-')
            || self.payload_digest != self.digest()
            || self.operations_json.len() > 512 * 1024
            || (self.acknowledged && self.result.is_none())
        {
            return Err(reject());
        }
        let operations: Vec<Mutation> = serde_json::from_slice(&self.operations_json)?;
        if operations.is_empty()
            || operations.len() > 256
            || operations.iter().any(|op| {
                matches!(
                    op,
                    Mutation::PutStrategic(StrategicRecord::Sync { .. })
                        | Mutation::DeleteStrategic(crate::RecordKey {
                            family: crate::Family::Sync,
                            ..
                        })
                )
            })
        {
            return Err(reject());
        }
        if let Some(result) = &self.result
            && ((result.applied && self.receipt.is_none())
                || result.result_digest != self.payload_digest
                || result.local_sequence
                    != self.source_sequence.checked_add(2).ok_or_else(reject)?)
        {
            return Err(reject());
        }
        Ok(())
    }
    pub fn transition_from(&self, before: &Self) -> Result<(), StorageError> {
        let mut expected = before.clone();
        expected.receipt = self.receipt.clone();
        expected.result = self.result.clone();
        expected.acknowledged = self.acknowledged;
        if &expected != self
            || before.result.is_some() && before.result != self.result
            || before.receipt.is_some() && before.receipt != self.receipt
            || before.acknowledged && !self.acknowledged
        {
            return Err(StorageError::Rejected(
                "Trinitas entries are immutable except forward progress",
            ));
        }
        Ok(())
    }
}
