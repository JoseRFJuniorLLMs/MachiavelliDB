use super::*;
use machiavelli_proto::machiavelli::{self as pb, machiavelli_server::Machiavelli};
#[derive(Clone)]
pub struct Rpc {
    pub state: Shared,
    pub limiter: Arc<tokio::sync::Semaphore>,
}
fn error(failure: Failure) -> tonic::Status {
    let message = failure.1.0["error"]
        .as_str()
        .unwrap_or("request failed")
        .to_owned();
    match failure.0 {
        StatusCode::NOT_FOUND => tonic::Status::not_found(message),
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
            tonic::Status::invalid_argument(message)
        }
        StatusCode::CONFLICT => tonic::Status::failed_precondition(message),
        StatusCode::TOO_MANY_REQUESTS => tonic::Status::resource_exhausted(message),
        _ => tonic::Status::internal(message),
    }
}
#[tonic::async_trait]
impl Machiavelli for Rpc {
    async fn health(
        &self,
        _: tonic::Request<pb::Empty>,
    ) -> Result<tonic::Response<pb::HealthReply>, tonic::Status> {
        let value = super::health(State(self.state.clone()))
            .await
            .map_err(error)?
            .0;
        Ok(tonic::Response::new(pb::HealthReply {
            mode: "local_alpha".into(),
            nodes: value["nodes"].as_u64().unwrap(),
            sequence: value["sequence"].as_u64().unwrap(),
        }))
    }
    async fn get(
        &self,
        request: tonic::Request<pb::NodeKey>,
    ) -> Result<tonic::Response<pb::NodeRecord>, tonic::Status> {
        let value = super::read(State(self.state.clone()), Path(request.into_inner().id))
            .await
            .map_err(error)?
            .0;
        let node: ContinentiaNode =
            serde_json::from_value(value).map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(pb::NodeRecord {
            metadata: node.meta.encode().to_vec(),
            phase: node
                .phase
                .encode()
                .map_err(|e| tonic::Status::internal(e.to_string()))?,
        }))
    }
    async fn put(
        &self,
        request: tonic::Request<pb::NodeRecord>,
    ) -> Result<tonic::Response<pb::WriteReceipt>, tonic::Status> {
        let record = request.into_inner();
        let meta = machiavelli_core::ContinentiaMeta::decode(&record.metadata)
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
        let phase = machiavelli_core::PhaseVector::decode(&record.phase)
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
        let value = super::put(
            State(self.state.clone()),
            Json(ContinentiaNode::new(meta, phase)),
        )
        .await
        .map_err(error)?
        .0;
        Ok(tonic::Response::new(pb::WriteReceipt {
            sequence: value["sequence"].as_u64().unwrap(),
        }))
    }
    async fn delete(
        &self,
        request: tonic::Request<pb::NodeKey>,
    ) -> Result<tonic::Response<pb::WriteReceipt>, tonic::Status> {
        let value = super::delete(State(self.state.clone()), Path(request.into_inner().id))
            .await
            .map_err(error)?
            .0;
        Ok(tonic::Response::new(pb::WriteReceipt {
            sequence: value["sequence"].as_u64().unwrap(),
        }))
    }
    async fn query(
        &self,
        request: tonic::Request<pb::PqlQuery>,
    ) -> Result<tonic::Response<pb::JsonReply>, tonic::Status> {
        let value = super::pql(
            State(self.state.clone()),
            axum::Extension(self.limiter.clone()),
            Json(PqlRequest {
                query: request.into_inner().query,
            }),
        )
        .await
        .map_err(error)?
        .0;
        Ok(tonic::Response::new(pb::JsonReply {
            json: value.to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use machiavelli_proto::machiavelli::machiavelli_client::MachiavelliClient;
    #[tokio::test]
    async fn authenticated_grpc_and_rest_share_validated_binary_state() {
        let directory = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(Store::open(directory.path()).unwrap()));
        let token = "0123456789abcdef0123456789abcdef";
        let app = application(state, token.into());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut client = MachiavelliClient::connect(format!("http://{address}"))
            .await
            .unwrap();
        assert!(client.health(pb::Empty {}).await.is_err());
        let request = |message| {
            let mut r = tonic::Request::new(message);
            r.metadata_mut()
                .insert("authorization", format!("Bearer {token}").parse().unwrap());
            r
        };
        let node = ContinentiaNode::new(
            machiavelli_core::ContinentiaMeta::new(NodeId::new(), 1),
            machiavelli_core::PhaseVector::new(vec![0.3], vec![0.7]).unwrap(),
        );
        let record = pb::NodeRecord {
            metadata: node.meta.encode().to_vec(),
            phase: node.phase.encode().unwrap(),
        };
        let mut put_request = tonic::Request::new(record.clone());
        put_request
            .metadata_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        assert_eq!(
            client.put(put_request).await.unwrap().into_inner().sequence,
            1
        );
        let mut get_request = tonic::Request::new(pb::NodeKey {
            id: node.meta.id.to_string(),
        });
        get_request
            .metadata_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        let fetched = client.get(get_request).await.unwrap().into_inner();
        assert_eq!(fetched.metadata, record.metadata);
        assert_eq!(fetched.phase, record.phase);
        let query = request(pb::PqlQuery {
            query: "SCOUT ALL".into(),
        });
        let json = client.query(query).await.unwrap().into_inner().json;
        let rows: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 1);
        server.abort();
        let _ = server.await;
    }
}
