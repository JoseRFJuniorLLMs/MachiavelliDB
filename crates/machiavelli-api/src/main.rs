//! Local REST surface. Bound only to loopback; no production authentication yet.
mod rpc;
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
            "sequence":store.snapshot().sequence(), "mode":"local_alpha"
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
    let app = application(state, token);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    eprintln!("MachiavelliDB listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
fn application(state: Shared, token: String) -> Router {
    let limiter = Arc::new(tokio::sync::Semaphore::new(2));
    let grpc = tonic::service::Routes::new(
        machiavelli_proto::machiavelli::machiavelli_server::MachiavelliServer::new(rpc::Rpc {
            state: state.clone(),
            limiter: limiter.clone(),
        })
        .max_decoding_message_size(1024 * 1024),
    )
    .into_axum_router();
    Router::new()
        .route("/pql", axum::routing::post(pql))
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
