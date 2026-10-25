//! Authenticated REST/gRPC surface bound to loopback.
mod rpc;
mod tactics;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    routing::get,
};
use machiavelli_core::{ContinentiaNode, NodeId};
use machiavelli_storage::{Mutation, StorageError, Store};
use std::sync::{Arc, Mutex};
type Shared = Arc<Mutex<Store>>;
type Failure = (StatusCode, Json<serde_json::Value>);
type Response = Result<Json<serde_json::Value>, Failure>;

fn failure(status: StatusCode, message: &str) -> Failure {
    (status, Json(serde_json::json!({"error":message})))
}
fn storage_failure(error: StorageError) -> Failure {
    match error {
        StorageError::Rejected(_) => failure(StatusCode::CONFLICT, &error.to_string()),
        StorageError::Core(_) | StorageError::Codec(_) => {
            failure(StatusCode::UNPROCESSABLE_ENTITY, &error.to_string())
        }
        _ => {
            eprintln!("storage failure: {error}");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage failure; inspect server log",
            )
        }
    }
}
async fn execute<F>(state: Shared, operation: F) -> Response
where
    F: FnOnce(&mut Store) -> Response + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let mut store = state
            .lock()
            .map_err(|_| failure(StatusCode::INTERNAL_SERVER_ERROR, "store unavailable"))?;
        operation(&mut store)
    })
    .await
    .map_err(|_| failure(StatusCode::INTERNAL_SERVER_ERROR, "worker failed"))?
}
async fn health(State(state): State<Shared>) -> Response {
    execute(state, |store| {
        Ok(Json(serde_json::json!({
            "status":"ok","version":env!("CARGO_PKG_VERSION"),"nodes":store.snapshot().len(),
            "strategic_records":store.snapshot().strategic_len(), "sequence":store.snapshot().sequence(), "mode":"local_alpha"
        })))
    })
    .await
}
#[derive(serde::Deserialize, Default)]
struct ListParams {
    after: Option<String>,
    limit: Option<usize>,
    at_sequence: Option<u64>,
}
async fn list(
    State(state): State<Shared>,
    axum::extract::Query(params): axum::extract::Query<ListParams>,
) -> Response {
    let limit = params.limit.unwrap_or(100);
    if limit == 0 || limit > 1000 {
        return Err(failure(StatusCode::BAD_REQUEST, "limit must be 1..1000"));
    }
    let after = params
        .after
        .map(|id| id.parse::<NodeId>())
        .transpose()
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "invalid cursor"))?;
    execute(state, move |store| {
        let snapshot = store.snapshot();
        if params.at_sequence.is_some_and(|s| s != snapshot.sequence()) {
            return Err(failure(
                StatusCode::CONFLICT,
                "snapshot changed; restart pagination",
            ));
        }
        serde_json::to_value(
            snapshot
                .nodes()
                .filter(|node| after.is_none_or(|id| node.meta.id > id))
                .take(limit)
                .collect::<Vec<_>>(),
        )
        .map(Json)
        .map_err(|_| failure(StatusCode::INTERNAL_SERVER_ERROR, "serialization failed"))
    })
    .await
}
async fn read(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    let id: NodeId = id
        .parse()
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "invalid node id"))?;
    execute(state, move |store| {
        let snapshot = store.snapshot();
        let node = snapshot
            .get(id)
            .ok_or_else(|| failure(StatusCode::NOT_FOUND, "node not found"))?;
        serde_json::to_value(node)
            .map(Json)
            .map_err(|_| failure(StatusCode::INTERNAL_SERVER_ERROR, "serialization failed"))
    })
    .await
}
async fn put(State(state): State<Shared>, Json(node): Json<ContinentiaNode>) -> Response {
    if !matches!(
        node.meta.posture,
        machiavelli_core::StrategicPosture::Expanding
            | machiavelli_core::StrategicPosture::Expendable
    ) {
        return Err(failure(
            StatusCode::CONFLICT,
            "tactical posture requires verified administrative intent",
        ));
    }
    execute(state, move |store| {
        validate_ordinary_metadata(&store.snapshot(), &node)?;
        let sequence = store
            .commit(&[Mutation::Put(node)])
            .map_err(storage_failure)?;
        Ok(Json(serde_json::json!({"sequence":sequence})))
    })
    .await
}
async fn delete(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    let id: NodeId = id
        .parse()
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "invalid node id"))?;
    execute(state, move |store| {
        if store.snapshot().get(id).is_none() {
            return Err(failure(StatusCode::NOT_FOUND, "node not found"));
        }
        let sequence = store
            .commit(&[Mutation::Delete(id)])
            .map_err(storage_failure)?;
        Ok(Json(serde_json::json!({"sequence":sequence})))
    })
    .await
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionRequest {
    expected_sequence: u64,
    operations: Vec<Mutation>,
}
fn ordinary_node(node: &ContinentiaNode) -> bool {
    matches!(
        node.meta.posture,
        machiavelli_core::StrategicPosture::Expanding
            | machiavelli_core::StrategicPosture::Expendable
    )
}
fn validate_ordinary_metadata(
    snapshot: &machiavelli_storage::Snapshot,
    node: &ContinentiaNode,
) -> Result<(), Failure> {
    let allowed = if let Some(before) = snapshot.get(node.meta.id) {
        before.meta.vcg_credit_balance == node.meta.vcg_credit_balance
            && before.meta.last_heraclitus_lsn == node.meta.last_heraclitus_lsn
            && before.meta.dike_count == node.meta.dike_count
            && before.meta.alliance_cluster == node.meta.alliance_cluster
    } else {
        node.meta.vcg_credit_balance == 0.0
            && node.meta.last_heraclitus_lsn == 0
            && node.meta.dike_count == 0
            && node.meta.alliance_cluster == 0
    };
    if !allowed {
        return Err(failure(
            StatusCode::CONFLICT,
            "credits and tactical metadata require authorized actions",
        ));
    }
    Ok(())
}
async fn transaction(
    State(state): State<Shared>,
    Json(request): Json<TransactionRequest>,
) -> Response {
    if request.operations.is_empty() || request.operations.len() > 256 {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "transaction requires 1..256 operations",
        ));
    }
    for op in &request.operations {
        let allowed = match op {
            Mutation::Put(node) => ordinary_node(node),
            Mutation::Delete(_) => true,
            Mutation::PutStrategic(record) => !matches!(
                record,
                machiavelli_storage::StrategicRecord::Edge { .. }
                    | machiavelli_storage::StrategicRecord::Dike { .. }
                    | machiavelli_storage::StrategicRecord::Sync { .. }
                    | machiavelli_storage::StrategicRecord::Belief { .. }
            ),
            Mutation::DeleteStrategic(key) => !matches!(
                key.family,
                machiavelli_storage::Family::Edges
                    | machiavelli_storage::Family::Dikes
                    | machiavelli_storage::Family::Sync
                    | machiavelli_storage::Family::Beliefs
            ),
        };
        if !allowed {
            return Err(failure(
                StatusCode::CONFLICT,
                "tactical mutation requires verified administrative intent",
            ));
        }
    }
    execute(state, move |store| {
        let snapshot = store.snapshot();
        for op in &request.operations {
            if let Mutation::Put(node) = op {
                validate_ordinary_metadata(&snapshot, node)?;
            }
        }
        let sequence = store
            .commit_at(request.expected_sequence, &request.operations)
            .map_err(storage_failure)?;
        Ok(Json(
            serde_json::json!({"sequence":sequence,"operations":request.operations.len()}),
        ))
    })
    .await
}
#[derive(serde::Deserialize, Default)]
struct StrategicParams {
    family: Option<machiavelli_storage::Family>,
    offset: Option<usize>,
    limit: Option<usize>,
    at_sequence: Option<u64>,
}
async fn strategic_records(
    State(state): State<Shared>,
    axum::extract::Query(params): axum::extract::Query<StrategicParams>,
) -> Response {
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    if limit == 0 || limit > 1000 || offset > 1_000_000 {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "invalid pagination bounds",
        ));
    }
    execute(state, move |store| {
        let snapshot = store.snapshot();
        if params.at_sequence.is_some_and(|s| s != snapshot.sequence()) {
            return Err(failure(
                StatusCode::CONFLICT,
                "snapshot changed; restart pagination",
            ));
        }
        let records = snapshot
            .records()
            .filter(|r| params.family.is_none_or(|f| r.key().family == f))
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        Ok(Json(
            serde_json::json!({"sequence":snapshot.sequence(),"records":records}),
        ))
    })
    .await
}
async fn adjacency(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    let id: NodeId = id
        .parse()
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "invalid node id"))?;
    execute(state,move|store| {
        let snapshot=store.snapshot();
        if snapshot.get(id).is_none() { return Err(failure(StatusCode::NOT_FOUND,"node not found")); }
        Ok(Json(serde_json::json!({"sequence":snapshot.sequence(),"adjacency":snapshot.adjacency(id).collect::<Vec<_>>()})))
    }).await
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.len() > 2 {
        return Err("usage: machiavelli-server DATA_DIR [PORT]".into());
    }
    let port: u16 = args.get(1).map_or(Ok(8787), |value| value.parse())?;
    let state = Arc::new(Mutex::new(Store::open(&args[0])?));
    let token = std::env::var("MACHIAVELLI_TOKEN")
        .map_err(|_| "set MACHIAVELLI_TOKEN to at least 32 ASCII characters")?;
    if token.len() < 32 || !token.is_ascii() {
        return Err("MACHIAVELLI_TOKEN requires at least 32 ASCII characters".into());
    }
    let runtime = tactics::Runtime::from_env()?;
    if let Some(runtime) = &runtime {
        runtime.clone().recover(state.clone());
    }
    let app = application_with_tactics(state, token, runtime);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    eprintln!("MachiavelliDB listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
#[cfg(test)]
fn application(state: Shared, token: String) -> Router {
    application_with_tactics(state, token, None)
}
fn application_with_tactics(
    state: Shared,
    token: String,
    runtime: Option<Arc<tactics::Runtime>>,
) -> Router {
    let limiter = Arc::new(tokio::sync::Semaphore::new(2));
    let grpc = tonic::service::Routes::new(
        machiavelli_proto::machiavelli::machiavelli_server::MachiavelliServer::new(rpc::Rpc {
            state: state.clone(),
            limiter: limiter.clone(),
        })
        .max_decoding_message_size(1024 * 1024),
    )
    .into_axum_router();
    // Always routed: the gRPC fallback would answer an unknown path with 200.
    let router = match runtime {
        Some(runtime) => Router::new()
            .route("/actions", axum::routing::post(tactics::submit))
            .layer(axum::Extension(runtime)),
        None => Router::new().route("/actions", axum::routing::post(tactics::unavailable)),
    };
    router
        .route("/actions/{id}", get(tactics::status))
        .route("/pql", axum::routing::post(pql))
        .route("/transactions", axum::routing::post(transaction))
        .route("/strategic", get(strategic_records))
        .route("/nodes/{id}/adjacency", get(adjacency))
        .route(
            "/observatory",
            get(|| async { axum::response::Html(machiavelli_observatory::DASHBOARD) }),
        )
        .route("/health", get(health))
        .route("/nodes", get(list).post(put))
        .route("/nodes/{id}", get(read).delete(delete))
        .layer(axum::Extension(limiter))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(state)
        .merge(grpc)
        .layer(axum::middleware::from_fn_with_state(
            Arc::new(token),
            authorize,
        ))
}
#[derive(serde::Deserialize)]
struct PqlRequest {
    query: String,
}
async fn pql(
    State(state): State<Shared>,
    axum::Extension(limiter): axum::Extension<Arc<tokio::sync::Semaphore>>,
    Json(request): Json<PqlRequest>,
) -> Response {
    let _permit = limiter
        .try_acquire_owned()
        .map_err(|_| failure(StatusCode::TOO_MANY_REQUESTS, "solver capacity exhausted"))?;
    let query =
        machiavelli_pql::parse(&request.query).map_err(|e| failure(StatusCode::BAD_REQUEST, &e))?;
    let deadline = machiavelli_core::Deadline::from_millis(query.deadline_ms);
    match query.command{
        machiavelli_pql::Command::Scout(Some(id))=>read(State(state),Path(id.to_string())).await,
        machiavelli_pql::Command::Scout(None)=>list(State(state),axum::extract::Query(ListParams::default())).await,
        machiavelli_pql::Command::PrinceKuhn{iterations}=>{
            tokio::task::spawn_blocking(move||{
                let result=machiavelli_cfr::extensive::kuhn().solve(iterations,&deadline)
                    .map_err(|e|failure(StatusCode::UNPROCESSABLE_ENTITY,&e.to_string()))?;
                Ok(Json(serde_json::json!({"game":"kuhn","strategies":result.strategies,
                    "value":result.value,"exploitability":result.exploitability,"iterations":result.iterations,
                    "external_action":false})))
            }).await.map_err(|_|failure(StatusCode::INTERNAL_SERVER_ERROR,"solver worker failed"))?
        },
        machiavelli_pql::Command::PrinceMcts{target,iterations,horizon,seed}=>{
            let snapshot=tokio::task::spawn_blocking(move||{
                state.lock().map(|store|store.snapshot()).map_err(|_|failure(StatusCode::INTERNAL_SERVER_ERROR,"store unavailable"))
            }).await.map_err(|_|failure(StatusCode::INTERNAL_SERVER_ERROR,"snapshot worker failed"))??;
            tokio::task::spawn_blocking(move||{
                let _permit=_permit;
                let result=machiavelli_centaur::planning::plan(&snapshot,target,machiavelli_centaur::planning::Budget{iterations,horizon,seed},&deadline)
                    .map_err(|e|{
                        let status=match &e {machiavelli_centaur::planning::PlanningError::NotFound=>StatusCode::NOT_FOUND,machiavelli_centaur::planning::PlanningError::InvalidBudget=>StatusCode::BAD_REQUEST,_=>StatusCode::UNPROCESSABLE_ENTITY};
                        failure(status,&e.to_string())
                    })?;
                serde_json::to_value(result).map(Json).map_err(|_|failure(StatusCode::INTERNAL_SERVER_ERROR,"serialization failed"))
            }).await.map_err(|_|failure(StatusCode::INTERNAL_SERVER_ERROR,"planning worker failed"))?
        },
        machiavelli_pql::Command::Wargame{target,steps,dt}=>{
            let snapshot=tokio::task::spawn_blocking(move||{
                state.lock().map(|store|store.snapshot())
                    .map_err(|_|failure(StatusCode::INTERNAL_SERVER_ERROR,"store unavailable"))
            }).await.map_err(|_|failure(StatusCode::INTERNAL_SERVER_ERROR,"snapshot worker failed"))??;
            tokio::task::spawn_blocking(move||{
                use machiavelli_symplectic::{Integrator,Scheme,systems::Harmonic};
                use machiavelli_hamiltonian::{StrategicMass,StrategicHamiltonian,AdversarialPotential};

                let node=snapshot.get(target).ok_or_else(||failure(StatusCode::NOT_FOUND,"node not found"))?;
                let n=node.phase.dim();
                let mass=StrategicMass::from_node(&node.meta,n,0.0,Default::default())
                    .map_err(|e|failure(StatusCode::CONFLICT,&e.to_string()))?;
                let model=StrategicHamiltonian::new(mass,Harmonic::isotropic(n,1.0),
                    AdversarialPotential::new(vec![0.0;n],0.0,1.0).unwrap())
                    .map_err(|e|failure(StatusCode::CONFLICT,&e.to_string()))?;
                let mut phase=node.phase.clone();
                let mut integrator=Integrator::new(model.separable(),Scheme::Yoshida4).compensated();
                let mut completed=0;
                while completed<steps && !deadline.expired(){
                    let count=(steps-completed).min(128);
                    let (q,p)=phase.qp_mut();
                    integrator.run(q,p,dt,count).map_err(|e|failure(StatusCode::UNPROCESSABLE_ENTITY,&e.to_string()))?;
                    completed+=count;
                }
                let energy=model.evaluate(&phase).map_err(|e|failure(StatusCode::UNPROCESSABLE_ENTITY,&e.to_string()))?;
                Ok(Json(serde_json::json!({"model":"reference_harmonic","persisted":false,
                    "phase":phase,"energy":energy,"steps":completed,"source_sequence":snapshot.sequence()})))
            }).await.map_err(|_|failure(StatusCode::INTERNAL_SERVER_ERROR,"simulation worker failed"))?
        },
        _=>Err(failure(StatusCode::CONFLICT,"tactical mutation requires configured constitutional/quorum policy and confirmed Heraclitus DurableIntent; no side effect executed")),
    }
}
async fn authorize(
    State(token): State<Arc<String>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if matches!(request.uri().path(), "/health" | "/observatory") {
        return next.run(request).await;
    }
    let expected = format!("Bearer {}", token);
    let given = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let mut difference = expected.len() ^ given.len();
    for (a, b) in expected.bytes().zip(given.bytes()) {
        difference |= (a ^ b) as usize;
    }
    if difference != 0 {
        return failure(StatusCode::UNAUTHORIZED, "authentication required").into_response();
    }
    next.run(request).await
}
#[cfg(test)]
mod transaction_tests {
    use super::*;
    use machiavelli_core::{ContinentiaMeta, PhaseVector, StrategicPosture};
    fn node() -> ContinentiaNode {
        ContinentiaNode::new(
            ContinentiaMeta::new(NodeId::new(), 1),
            PhaseVector::zeros(1),
        )
    }
    #[tokio::test]
    async fn batches_compare_sequence_validate_all_operations_and_respect_tactical_gate() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
        let a = node();
        let mut b = node();
        b.meta.virtu = f32::NAN;
        let result = transaction(
            State(state.clone()),
            Json(TransactionRequest {
                expected_sequence: 0,
                operations: vec![Mutation::Put(a.clone()), Mutation::Put(b.clone())],
            }),
        )
        .await;
        assert_eq!(result.unwrap_err().0, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(state.lock().unwrap().snapshot().len(), 0);
        b.meta.virtu = 0.5;
        let result = transaction(
            State(state.clone()),
            Json(TransactionRequest {
                expected_sequence: 0,
                operations: vec![Mutation::Put(a.clone()), Mutation::Put(b.clone())],
            }),
        )
        .await
        .unwrap();
        assert_eq!(result.0["sequence"], 1);
        let conflict = transaction(
            State(state.clone()),
            Json(TransactionRequest {
                expected_sequence: 0,
                operations: vec![Mutation::Delete(a.meta.id)],
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(conflict.0, StatusCode::CONFLICT);
        assert_eq!(state.lock().unwrap().snapshot().len(), 2);
        b.meta.posture = StrategicPosture::Fortified;
        assert_eq!(
            transaction(
                State(state.clone()),
                Json(TransactionRequest {
                    expected_sequence: 1,
                    operations: vec![Mutation::Put(b)]
                })
            )
            .await
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
        let records = strategic_records(
            State(state.clone()),
            axum::extract::Query(StrategicParams {
                at_sequence: Some(0),
                ..Default::default()
            }),
        )
        .await;
        assert_eq!(records.unwrap_err().0, StatusCode::CONFLICT);
        assert!(
            adjacency(State(state), Path(a.meta.id.to_string()))
                .await
                .unwrap()
                .0["adjacency"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    #[tokio::test]
    async fn strategic_writes_cannot_bypass_the_tactical_gate() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(Store::open(dir.path()).unwrap()));
        let edge = machiavelli_core::StrategicEdge {
            id: machiavelli_core::EdgeId::new(),
            from: NodeId::new(),
            to: NodeId::new(),
            kind: machiavelli_core::EdgeKind::Ally,
            weight: 1.0,
        };
        let result = transaction(
            State(state.clone()),
            Json(TransactionRequest {
                expected_sequence: 0,
                operations: vec![Mutation::PutStrategic(
                    machiavelli_storage::StrategicRecord::Edge { edge },
                )],
            }),
        )
        .await;
        assert_eq!(result.unwrap_err().0, StatusCode::CONFLICT);
        assert_eq!(state.lock().unwrap().snapshot().sequence(), 0);
    }
}
