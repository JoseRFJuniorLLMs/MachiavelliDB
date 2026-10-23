//! Registos de auditoria (causa-raiz de `BETRAY`, decisões `PRINCE`, `CONQUER`, …).
//! A ponte Trinitas implementa [`AuditSink`] sobre o HeraclitusDB.

use crate::ids::NodeId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditKind {
    Betrayal,
    Prince,
    Conquer,
    DikeFired,
    Fortify,
    Deception,
    Tribute,
    ConstitutionalViolation,
}

impl AuditKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditKind::Betrayal => "betrayal",
            AuditKind::Prince => "prince",
            AuditKind::Conquer => "conquer",
            AuditKind::DikeFired => "dike_fired",
            AuditKind::Fortify => "fortify",
            AuditKind::Deception => "deception",
            AuditKind::Tribute => "tribute",
            AuditKind::ConstitutionalViolation => "constitutional_violation",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub kind: AuditKind,
    pub subject: NodeId,
    pub summary: String,
    pub payload: serde_json::Value,
    pub at_hlc: u64,
}

impl AuditRecord {
    /// BLAKE3 do JSON canónico (as chaves de `serde_json::Value` são ordenadas).
    pub fn digest(&self) -> [u8; 32] {
        let bytes = serde_json::to_vec(self).expect("AuditRecord serializa sempre");
        *blake3::hash(&bytes).as_bytes()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditReceipt {
    pub digest_hex: String,
    pub sink: String,
    pub sequence: u64,
    /// Referência externa (por exemplo `lsn:1234` no HeraclitusDB).
    pub external_ref: Option<String>,
}

/// Destino durável de registos de auditoria.
pub trait AuditSink: Send + Sync + fmt::Debug {
    fn submit(&self, record: &AuditRecord) -> Result<AuditReceipt, String>;
}

/// Sink em memória para testes e modo offline.
#[derive(Debug, Default)]
pub struct MemoryAuditSink {
    records: Mutex<Vec<(AuditRecord, AuditReceipt)>>,
}

impl MemoryAuditSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn records(&self) -> Vec<(AuditRecord, AuditReceipt)> {
        self.records.lock().unwrap().clone()
    }

    pub fn len(&self) -> usize {
        self.records.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AuditSink for MemoryAuditSink {
    fn submit(&self, record: &AuditRecord) -> Result<AuditReceipt, String> {
        let mut guard = self.records.lock().unwrap();
        let receipt = AuditReceipt {
            digest_hex: hex::encode(record.digest()),
            sink: "memory".into(),
            sequence: guard.len() as u64 + 1,
            external_ref: None,
        };
        guard.push((record.clone(), receipt.clone()));
        Ok(receipt)
    }
}
