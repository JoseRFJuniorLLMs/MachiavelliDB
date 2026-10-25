use super::*;
use machiavelli_trinitas::{
    AuditBridge,
    actions::{Policy, Request},
    coordinator,
};
pub struct Runtime {
    pub policy: Arc<Policy>,
    endpoint: String,
    bearer: String,
    serial: tokio::sync::Mutex<()>,
}
impl Runtime {
    pub fn from_env() -> Result<Option<Arc<Self>>, Box<dyn std::error::Error>> {
        let Ok(endpoint) = std::env::var("HERACLITUS_ENDPOINT") else {
            return Ok(None);
        };
        let uri: axum::http::Uri = endpoint.parse()?;
        if uri.scheme_str() != Some("http")
            || !matches!(uri.host(), Some("127.0.0.1" | "[::1]" | "localhost"))
        {
            return Err("HERACLITUS_ENDPOINT requires trusted loopback HTTP".into());
        }
        let bearer = std::env::var("HERACLITUS_TOKEN")?;
        if bearer.len() < 32 || !bearer.is_ascii() {
            return Err("HERACLITUS_TOKEN requires at least 32 ASCII characters".into());
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Configuration {
            keys: Vec<String>,
            faults: usize,
            epoch: u64,
        }
        let quorum = std::env::var("MACHIAVELLI_QUORUM_FILE")
            .ok()
            .map(|path| -> Result<_, Box<dyn std::error::Error>> {
                let config: Configuration = serde_json::from_slice(&std::fs::read(path)?)?;
                let keys = config
                    .keys
                    .iter()
                    .map(|s| {
                        Ok(hex::decode(s)?
                            .try_into()
                            .map_err(|_| "invalid quorum key length")?)
                    })
                    .collect::<Result<Vec<[u8; 32]>, Box<dyn std::error::Error>>>()?;
                Ok(machiavelli_fortify::Quorum::new(
                    keys,
                    config.faults,
                    config.epoch,
                )?)
            })
            .transpose()?;
        Ok(Some(Arc::new(Self {
            policy: Arc::new(Policy { quorum }),
            endpoint,
            bearer,
            serial: tokio::sync::Mutex::new(()),
        })))
    }
    async fn drive(
        &self,
        state: Shared,
        id: &str,
    ) -> Result<machiavelli_storage::sync::SyncEntry, machiavelli_trinitas::external::Error> {
        let _serial = self.serial.lock().await;
        let mut bridge = AuditBridge::connect(self.endpoint.clone())
            .await?
            .with_bearer(&self.bearer)?;
        coordinator::drive(state, &mut bridge, &self.policy, id).await
    }
    pub fn recover(self: Arc<Self>, state: Shared) {
        tokio::spawn(async move {
            loop {
                let copy = state.clone();
                let ids = tokio::task::spawn_blocking(move || {
                    copy.lock()
                        .map(|store| {
                            store
                                .snapshot()
                                .records()
                                .filter_map(|r| match r {
                                    machiavelli_storage::StrategicRecord::Sync { entry }
                                        if !entry.acknowledged =>
                                    {
                                        Some(entry.operation_id.clone())
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .await
                .unwrap_or_default();
                for id in ids {
                    if let Err(error) = self.drive(state.clone(), &id).await {
                        eprintln!("Trinitas recovery {id}: {error}");
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    }
}
pub async fn submit(
    State(state): State<Shared>,
    axum::Extension(runtime): axum::Extension<Arc<Runtime>>,
    Json(request): Json<Request>,
) -> Response {
    let policy = runtime.policy.clone();
    let id = request.operation_id.clone();
    let entry = execute(state.clone(), move |store| {
        coordinator::prepare(store, &request, &policy, coordinator::now_ms())
            .map(|entry| Json(serde_json::to_value(entry).unwrap()))
            .map_err(|e| failure(StatusCode::CONFLICT, &e.to_string()))
    })
    .await?;
    match runtime.drive(state, &id).await {
        Ok(entry) => Ok(Json(serde_json::to_value(entry).unwrap())),
        Err(error) => {
            eprintln!("Trinitas pending {id}: {error}");
            Ok(entry)
        }
    }
}
/// Without a configured Heraclitus there is no authority to execute anything.
pub async fn unavailable() -> Response {
    Err(failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "tactical actions require HERACLITUS_ENDPOINT and HERACLITUS_TOKEN",
    ))
}
pub async fn status(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    execute(state, move |store| {
        coordinator::read(store, &id)
            .map(|e| Json(serde_json::to_value(e).unwrap()))
            .ok_or_else(|| failure(StatusCode::NOT_FOUND, "operation absent"))
    })
    .await
}
