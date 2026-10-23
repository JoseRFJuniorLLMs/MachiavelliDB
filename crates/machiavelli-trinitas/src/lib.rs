//! Explicit separation of ordinary audit append from privileged execution tokens.
//! The current Heraclitus wire contract cannot authorize external side effects.
use machiavelli_core::AuditRecord;
use machiavelli_proto::heraclitus::{AppendRequest, heraclitus_client::HeraclitusClient};
use std::collections::HashMap;
pub struct AuditAppendReceipt {
    pub lsn: u64,
    pub event_id: String,
    pub deduplicated: bool,
    pub record_digest: String,
}
pub struct AuditBridge {
    client: HeraclitusClient<tonic::transport::Channel>,
    bearer: Option<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>,
}
impl AuditBridge {
    pub async fn connect(endpoint: String) -> Result<Self, tonic::transport::Error> {
        let endpoint = tonic::transport::Endpoint::from_shared(endpoint)?
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(5));
        Ok(Self {
            client: HeraclitusClient::new(endpoint.connect().await?),
            bearer: None,
        })
    }
    pub fn with_bearer(
        mut self,
        token: &str,
    ) -> Result<Self, tonic::metadata::errors::InvalidMetadataValue> {
        self.bearer = Some(format!("Bearer {token}").parse()?);
        Ok(self)
    }
    /// Appends an ordinary audit record with a stable 64-character idempotency key.
    /// An Append receipt is deliberately not an AdminExecutionToken.
    pub async fn append(
        &mut self,
        record: &AuditRecord,
    ) -> Result<AuditAppendReceipt, Box<dyn std::error::Error + Send + Sync>> {
        let digest = hex::encode(record.digest());
        let request = AppendRequest {
            agent_id: "MachiavelliDB".into(),
            session_id: "trinitas-audit".into(),
            kind: "Action".into(),
            content: serde_json::to_vec(record)?,
            hyp: Vec::new(),
            sph: Vec::new(),
            euc: Vec::new(),
            attrs: HashMap::from([
                ("record_digest".into(), digest.clone()),
                ("audit_kind".into(), record.kind.as_str().into()),
            ]),
            parents: Vec::new(),
            idempotency_key: digest.clone(),
        };
        let mut request = tonic::Request::new(request);
        if let Some(token) = &self.bearer {
            request
                .metadata_mut()
                .insert("authorization", token.clone());
        }
        let response = self.client.append(request).await?.into_inner();
        if response.lsn == 0 || response.event_id.is_empty() {
            return Err("invalid audit acknowledgement".into());
        }
        Ok(AuditAppendReceipt {
            lsn: response.lsn,
            event_id: response.event_id,
            deduplicated: response.deduplicated,
            record_digest: digest,
        })
    }
}
/// No public constructor: audit append acknowledgements cannot forge this token.
/// A future adapter must verify a real SPEC-0089 receipt before adding construction.
pub struct AdminExecutionToken {
    _private: (),
}

/// Projection log_0^c(x) for curvature -c, with Poincare ball radius 1/sqrt(c).
/// Uses the convention inverse to exp_0(v)=tanh(sqrt(c)|v|)v/(sqrt(c)|v|).
pub fn poincare_to_tangent(x: &[f64], curvature: f64) -> Result<Vec<f64>, &'static str> {
    if x.is_empty()
        || x.iter().any(|v| !v.is_finite())
        || !curvature.is_finite()
        || curvature <= 0.0
    {
        return Err("invalid projection input");
    }
    let norm = x.iter().fold(0.0_f64, |r, v| r.hypot(*v));
    let scaled = curvature.sqrt() * norm;
    if !scaled.is_finite() || scaled >= 1.0 {
        return Err("point outside Poincare ball");
    }
    let factor = if scaled == 0.0 {
        1.0
    } else {
        scaled.atanh() / scaled
    };
    let result: Vec<_> = x.iter().map(|v| factor * v).collect();
    if result.iter().any(|v| !v.is_finite()) {
        return Err("projection overflow");
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_is_finite_radial_and_rejects_boundary() {
        assert_eq!(
            poincare_to_tangent(&[0.0, 0.0], 1.0).unwrap(),
            vec![0.0, 0.0]
        );
        let v = poincare_to_tangent(&[0.5, 0.0], 1.0).unwrap();
        assert!((v[0] - 0.5_f64.atanh()).abs() < 1e-14);
        assert!(poincare_to_tangent(&[1.0], 1.0).is_err());
        assert!(poincare_to_tangent(&[f64::NAN], 1.0).is_err());
    }
}

#[cfg(test)]
mod rpc_tests {
    use super::*;
    use machiavelli_proto::heraclitus::{
        AppendResponse,
        heraclitus_server::{Heraclitus, HeraclitusServer},
    };
    #[derive(Default)]
    struct Mock {
        seen: std::sync::Mutex<std::collections::BTreeSet<String>>,
    }
    #[tonic::async_trait]
    impl Heraclitus for Mock {
        async fn append(
            &self,
            request: tonic::Request<AppendRequest>,
        ) -> Result<tonic::Response<AppendResponse>, tonic::Status> {
            if request
                .metadata()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                != Some("Bearer test-token")
            {
                return Err(tonic::Status::unauthenticated("missing token"));
            }
            let request = request.into_inner();
            if request.idempotency_key.len() != 64 || request.agent_id != "MachiavelliDB" {
                return Err(tonic::Status::invalid_argument("invalid producer envelope"));
            }
            let deduplicated = !self.seen.lock().unwrap().insert(request.idempotency_key);
            Ok(tonic::Response::new(AppendResponse {
                lsn: 1,
                deduplicated,
                event_id: "test-event".into(),
            }))
        }
    }
    #[tokio::test]
    async fn wire_client_authenticates_and_retries_with_stable_idempotency_key() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let service = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(HeraclitusServer::new(Mock::default()))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        let mut bridge = AuditBridge::connect(format!("http://{address}"))
            .await
            .unwrap()
            .with_bearer("test-token")
            .unwrap();
        let record = AuditRecord {
            kind: machiavelli_core::AuditKind::Prince,
            subject: machiavelli_core::NodeId::new(),
            summary: "mock transport test; no external action".into(),
            payload: serde_json::json!({}),
            at_hlc: 1,
        };
        let first = bridge.append(&record).await.unwrap();
        let second = bridge.append(&record).await.unwrap();
        assert!(!first.deduplicated);
        assert!(second.deduplicated);
        assert_eq!(first.lsn, second.lsn);
        assert_eq!(first.record_digest, second.record_digest);
        service.abort();
    }
}
