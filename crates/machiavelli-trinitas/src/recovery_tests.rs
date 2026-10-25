//! Transport fault injection at the boundary after local fsync, before remote ACK.
use super::*;
use crate::{
    actions::{Action, Policy, Request},
    external::{Completion, Intent, Receipt},
};
use machiavelli_core::{ContinentiaMeta, ContinentiaNode, NodeId, PhaseVector};
use machiavelli_proto::heraclitus::{
    AdminRequest, AdminResponse, AppendResponse,
    heraclitus_server::{Heraclitus, HeraclitusServer},
};
use machiavelli_storage::{Mutation, Store};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[derive(Default)]
struct Remote {
    completions_fail: AtomicBool,
    receipt: Mutex<Option<Receipt>>,
    completion: Mutex<Option<Completion>>,
}
#[derive(Clone)]
struct Service(Arc<Remote>);
#[tonic::async_trait]
impl Heraclitus for Service {
    async fn append(
        &self,
        _: tonic::Request<AppendRequest>,
    ) -> Result<tonic::Response<AppendResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("unused"))
    }
    async fn admin(
        &self,
        request: tonic::Request<AdminRequest>,
    ) -> Result<tonic::Response<AdminResponse>, tonic::Status> {
        if request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            != Some("Bearer recovery-token")
        {
            return Err(tonic::Status::unauthenticated("token"));
        }
        let request = request.into_inner();
        let value = match request.op.as_str() {
            "external-intent-v1" => {
                let intent: Intent = serde_json::from_str(&request.arg).unwrap();
                let mut saved = self.0.receipt.lock().unwrap();
                let receipt = Receipt {
                    intent,
                    principal: "test-admin".into(),
                    tenant: "local".into(),
                    intent_lsn: 0,
                };
                if let Some(before) = saved.as_ref() {
                    assert_eq!(before, &receipt);
                } else {
                    *saved = Some(receipt.clone());
                }
                serde_json::to_value(receipt).unwrap()
            }
            "external-result-v1" => {
                let completion: Completion = serde_json::from_str(&request.arg).unwrap();
                // Simulate a durable remote completion whose response was lost.
                let mut saved = self.0.completion.lock().unwrap();
                if let Some(before) = saved.as_ref() {
                    assert_eq!(before, &completion);
                } else {
                    *saved = Some(completion.clone());
                }
                if self.0.completions_fail.load(Ordering::SeqCst) {
                    return Err(tonic::Status::unavailable("lost completion response"));
                }
                serde_json::to_value(completion).unwrap()
            }
            "external-status-v1" => {
                let receipt = self.0.receipt.lock().unwrap().clone();
                let result = self.0.completion.lock().unwrap().clone();
                serde_json::json!({"version":1,"state":"recorded","receipt":receipt,"result":result})
            }
            _ => return Err(tonic::Status::unimplemented("unknown")),
        };
        Ok(tonic::Response::new(AdminResponse {
            ok: true,
            message: value.to_string(),
        }))
    }
}
async fn serve(remote: Arc<Remote>) -> (AuditBridge, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let service = Service(remote);
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(HeraclitusServer::new(service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let bridge = AuditBridge::connect(format!("http://{address}"))
        .await
        .unwrap()
        .with_bearer("recovery-token")
        .unwrap();
    (bridge, task)
}
fn funded_pair(store: &mut Store, credits: f64) -> (NodeId, NodeId) {
    let (a, b) = (NodeId::new(), NodeId::new());
    let mut funded = ContinentiaMeta::new(a, 1);
    funded.vcg_credit_balance = credits;
    store
        .commit(&[
            Mutation::Put(ContinentiaNode::new(funded, PhaseVector::zeros(1))),
            Mutation::Put(ContinentiaNode::new(
                ContinentiaMeta::new(b, 1),
                PhaseVector::zeros(1),
            )),
        ])
        .unwrap();
    (a, b)
}
/// Expiry between prepare and drive: never applied, whether or not Heraclitus
/// had already authorized it before the crash; the remote side is closed too.
#[tokio::test]
async fn expired_pending_operation_resolves_unapplied_with_or_without_remote_receipt() {
    let policy = Policy { quorum: None };
    for authorized_before_crash in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let (a, b) = funded_pair(&mut store, 100.0);
        let request = Request {
            operation_id: "expiring-transfer".into(),
            expected_sequence: 1,
            expires_ms: coordinator::now_ms() + 50,
            action: Action::Tribute {
                from: a,
                to: b,
                amount: 25,
            },
            votes: vec![],
        };
        let entry =
            coordinator::prepare(&mut store, &request, &policy, coordinator::now_ms()).unwrap();
        drop(store);
        let remote = Arc::new(Remote::default());
        if authorized_before_crash {
            *remote.receipt.lock().unwrap() = Some(Receipt {
                intent: Intent {
                    version: 1,
                    operation_id: entry.operation_id.clone(),
                    consumer: "MachiavelliDB".into(),
                    source_sequence: entry.source_sequence,
                    payload_digest: entry.payload_digest.clone(),
                    expires_ms: entry.expires_ms,
                },
                principal: "test-admin".into(),
                tenant: "local".into(),
                intent_lsn: 0,
            });
        }
        let (mut bridge, task) = serve(remote.clone()).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let shared = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
        let resolved =
            coordinator::drive(shared.clone(), &mut bridge, &policy, "expiring-transfer")
                .await
                .unwrap();
        assert!(resolved.acknowledged);
        assert!(!resolved.result.as_ref().unwrap().applied);
        assert_eq!(resolved.receipt.is_some(), authorized_before_crash);
        let remote_result = remote.completion.lock().unwrap().clone();
        assert_eq!(remote_result.is_some(), authorized_before_crash);
        assert!(remote_result.is_none_or(|c| !c.applied));
        let store = shared.lock().unwrap();
        assert_eq!(
            store.snapshot().get(a).unwrap().meta.vcg_credit_balance,
            100.0
        );
        assert_eq!(
            store.snapshot().get(b).unwrap().meta.vcg_credit_balance,
            0.0
        );
        // Pending (2), unapplied result (3) and, after a receipt, the ACK (4).
        let expected = if authorized_before_crash { 4 } else { 3 };
        assert_eq!(store.snapshot().sequence(), expected);
        task.abort();
    }
}
#[tokio::test]
async fn reopen_pending_then_reopen_applied_retries_only_result_and_never_duplicates_credits() {
    let dir = tempfile::tempdir().unwrap();
    let a = NodeId::new();
    let b = NodeId::new();
    let mut store = Store::open(dir.path()).unwrap();
    let mut funded = ContinentiaMeta::new(a, 1);
    funded.vcg_credit_balance = 100.0;
    store
        .commit(&[
            Mutation::Put(ContinentiaNode::new(funded, PhaseVector::zeros(1))),
            Mutation::Put(ContinentiaNode::new(
                ContinentiaMeta::new(b, 1),
                PhaseVector::zeros(1),
            )),
        ])
        .unwrap();
    let policy = Policy { quorum: None };
    let request = Request {
        operation_id: "recover-transfer".into(),
        expected_sequence: 1,
        expires_ms: coordinator::now_ms() + 60_000,
        action: Action::Tribute {
            from: a,
            to: b,
            amount: 25,
        },
        votes: vec![],
    };
    coordinator::prepare(&mut store, &request, &policy, coordinator::now_ms()).unwrap();
    assert!(store.commit(&[Mutation::Delete(b)]).is_err());
    assert_eq!(
        store.snapshot().get(a).unwrap().meta.vcg_credit_balance,
        100.0
    );
    drop(store);
    let remote = Arc::new(Remote::default());
    remote.completions_fail.store(true, Ordering::SeqCst);
    let (mut bridge, task) = serve(remote.clone()).await;
    let shared = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
    assert!(
        coordinator::drive(shared.clone(), &mut bridge, &policy, &request.operation_id)
            .await
            .is_err()
    );
    {
        let store = shared.lock().unwrap();
        assert_eq!(
            store.snapshot().get(a).unwrap().meta.vcg_credit_balance,
            75.0
        );
        assert_eq!(
            store.snapshot().get(b).unwrap().meta.vcg_credit_balance,
            25.0
        );
        assert!(
            coordinator::read(&store, &request.operation_id)
                .unwrap()
                .result
                .unwrap()
                .applied
        );
    }
    drop(shared);
    remote.completions_fail.store(false, Ordering::SeqCst);
    let shared = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
    let result = coordinator::drive(shared.clone(), &mut bridge, &policy, &request.operation_id)
        .await
        .unwrap();
    assert!(result.acknowledged);
    coordinator::drive(shared.clone(), &mut bridge, &policy, &request.operation_id)
        .await
        .unwrap();
    let mut store = shared.lock().unwrap();
    assert_eq!(
        store.snapshot().get(a).unwrap().meta.vcg_credit_balance,
        75.0
    );
    assert_eq!(
        store.snapshot().get(b).unwrap().meta.vcg_credit_balance,
        25.0
    );
    assert_eq!(store.snapshot().sequence(), 4);
    coordinator::prepare(&mut store, &request, &policy, coordinator::now_ms()).unwrap();
    let mut conflicting = request;
    conflicting.action = Action::Tribute {
        from: a,
        to: b,
        amount: 26,
    };
    assert!(
        coordinator::prepare(&mut store, &conflicting, &policy, coordinator::now_ms()).is_err()
    );
    task.abort();
}
