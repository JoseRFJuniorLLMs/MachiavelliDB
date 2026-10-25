//! At-least-once transport, exactly one local atomic effect per operation ID.
use crate::{
    AuditBridge,
    actions::{self, Policy, Request},
    external::{Completion, Error, Intent, Receipt},
};
use machiavelli_storage::{
    Family, Mutation, RecordKey, Store, StrategicRecord,
    sync::{SyncEntry, SyncResult},
};
use std::sync::{Arc, Mutex};
pub type SharedStore = Arc<Mutex<Store>>;
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
pub fn key(id: &str) -> RecordKey {
    RecordKey::new(Family::Sync, blake3::hash(id.as_bytes()).as_bytes())
}
pub fn read(store: &Store, id: &str) -> Option<SyncEntry> {
    match store.snapshot().record(&key(id)) {
        Some(StrategicRecord::Sync { entry }) => Some(entry.clone()),
        _ => None,
    }
}
pub fn prepare(
    store: &mut Store,
    request: &Request,
    policy: &Policy,
    now: u64,
) -> Result<SyncEntry, Error> {
    if let Some(entry) = read(store, &request.operation_id) {
        if entry.request != serde_json::to_value(request)? {
            return Err("operation ID already binds another request".into());
        }
        return Ok(entry);
    }
    if request.expires_ms <= now || request.expires_ms.saturating_sub(now) > 300_000 {
        return Err("expiry requires 1..300000ms in future".into());
    }
    let snapshot = store.snapshot();
    if request.expected_sequence != snapshot.sequence() {
        return Err("snapshot changed".into());
    }
    policy.verify(request, now)?;
    let operations = actions::plan(&snapshot, request, now)?;
    let mut entry = SyncEntry {
        operation_id: request.operation_id.clone(),
        source_sequence: snapshot.sequence(),
        expires_ms: request.expires_ms,
        request: serde_json::to_value(request)?,
        operations_json: serde_json::to_vec(&operations)?,
        payload_digest: String::new(),
        receipt: None,
        result: None,
        acknowledged: false,
    };
    entry.payload_digest = entry.digest();
    store.commit_at(
        snapshot.sequence(),
        &[Mutation::PutStrategic(StrategicRecord::Sync {
            entry: entry.clone(),
        })],
    )?;
    Ok(entry)
}
fn intent(entry: &SyncEntry) -> Intent {
    Intent {
        version: 1,
        operation_id: entry.operation_id.clone(),
        consumer: "MachiavelliDB".into(),
        source_sequence: entry.source_sequence,
        payload_digest: entry.payload_digest.clone(),
        expires_ms: entry.expires_ms,
    }
}
async fn access<T: Send + 'static>(
    store: SharedStore,
    f: impl FnOnce(&mut Store) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(move || {
        let mut guard = store.lock().map_err(|_| "store unavailable")?;
        f(&mut guard)
    })
    .await?
}
pub async fn drive(
    store: SharedStore,
    bridge: &mut AuditBridge,
    policy: &Policy,
    id: &str,
) -> Result<SyncEntry, Error> {
    let operation_id = id.to_string();
    let mut entry = access(store.clone(), move |s| {
        read(s, &operation_id).ok_or_else(|| "operation absent".into())
    })
    .await?;
    if entry.acknowledged {
        return Ok(entry);
    }
    if entry.result.is_none() {
        let bound = intent(&entry);
        let mut authorized = false;
        let receipt: Option<Receipt> = if now_ms() < entry.expires_ms {
            let token = bridge.authorize(&bound).await?;
            authorized = token.matches(&bound, now_ms());
            Some(token.receipt().clone())
        } else {
            let status = bridge.status(id).await?;
            if status.version != 1 || status.result.is_some() {
                return Err("unexpected remote result without local atomic marker".into());
            }
            status.receipt
        };
        if receipt.as_ref().is_some_and(|r| r.intent != bound) {
            return Err("unbound recovered receipt".into());
        }
        let request: Request = serde_json::from_value(entry.request.clone())?;
        let applied = authorized && policy.verify(&request, now_ms()).is_ok();
        entry.receipt = receipt.map(serde_json::to_value).transpose()?;
        entry.result = Some(SyncResult {
            applied,
            local_sequence: entry
                .source_sequence
                .checked_add(2)
                .ok_or("sequence overflow")?,
            result_digest: entry.payload_digest.clone(),
        });
        entry.acknowledged = entry.receipt.is_none();
        let mut next = entry.clone();
        entry = access(store.clone(), move |s| {
            if s.snapshot().sequence() != next.source_sequence + 1 {
                return Err("pending operation sequence mismatch".into());
            }
            let valid_dike = match &request.action {
                actions::Action::FireDike { id } => match s
                    .snapshot()
                    .record(&RecordKey::new(Family::Dikes, id.as_bytes()))
                {
                    Some(StrategicRecord::Dike { plan, .. }) => s
                        .snapshot()
                        .get(plan.subject())
                        .is_some_and(|n| plan.matches(n, now_ms(), request.expected_sequence)),
                    _ => false,
                },
                _ => true,
            };
            let applied = applied && now_ms() < next.expires_ms && valid_dike;
            next.result.as_mut().unwrap().applied = applied;
            let mut operations: Vec<Mutation> = if applied {
                serde_json::from_slice(&next.operations_json)?
            } else {
                Vec::new()
            };
            operations.push(Mutation::PutStrategic(StrategicRecord::Sync {
                entry: next.clone(),
            }));
            s.commit_at(next.source_sequence + 1, &operations)?;
            Ok(next)
        })
        .await?;
    }
    if !entry.acknowledged {
        let result = entry.result.as_ref().ok_or("result absent")?;
        let completion = Completion {
            receipt: serde_json::from_value(entry.receipt.clone().ok_or("receipt absent")?)?,
            applied: result.applied,
            local_sequence: result.local_sequence,
            result_digest: result.result_digest.clone(),
        };
        bridge.complete(&completion).await?;
        entry.acknowledged = true;
        let next = entry.clone();
        access(store, move |s| {
            s.commit(&[Mutation::PutStrategic(StrategicRecord::Sync {
                entry: next,
            })])?;
            Ok(())
        })
        .await?;
    }
    Ok(entry)
}
