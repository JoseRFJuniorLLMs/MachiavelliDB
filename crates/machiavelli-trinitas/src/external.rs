use crate::AuditBridge;
use machiavelli_proto::heraclitus::AdminRequest;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub version: u32,
    pub operation_id: String,
    pub consumer: String,
    pub source_sequence: u64,
    pub payload_digest: String,
    pub expires_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub intent: Intent,
    pub principal: String,
    pub tenant: String,
    pub intent_lsn: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    pub receipt: Receipt,
    pub applied: bool,
    pub local_sequence: u64,
    pub result_digest: String,
}
#[derive(Deserialize)]
pub struct Status {
    pub version: u32,
    pub state: String,
    pub receipt: Option<Receipt>,
    pub result: Option<Completion>,
}
/// Constructed only after an authenticated remote authorization bound to the
/// exact transaction. Deserializing a JSON receipt never constructs authority.
pub struct AdminExecutionToken {
    receipt: Receipt,
}
impl AdminExecutionToken {
    pub fn receipt(&self) -> &Receipt {
        &self.receipt
    }
    pub fn matches(&self, intent: &Intent, now_ms: u64) -> bool {
        self.receipt.intent == *intent && now_ms < intent.expires_ms
    }
}
impl AuditBridge {
    async fn admin<I: Serialize, O: DeserializeOwned>(
        &mut self,
        op: &str,
        input: &I,
    ) -> Result<O, Error> {
        let mut request = tonic::Request::new(AdminRequest {
            op: op.into(),
            arg: serde_json::to_string(input)?,
        });
        if let Some(token) = &self.bearer {
            request
                .metadata_mut()
                .insert("authorization", token.clone());
        }
        let response = self.client.admin(request).await?.into_inner();
        if !response.ok {
            return Err(format!("Heraclitus refused {op}: {}", response.message).into());
        }
        Ok(serde_json::from_str(&response.message)?)
    }
    pub async fn authorize(&mut self, intent: &Intent) -> Result<AdminExecutionToken, Error> {
        let receipt: Receipt = self.admin("external-intent-v1", intent).await?;
        if receipt.intent != *intent || receipt.principal.is_empty() || receipt.tenant.is_empty() {
            return Err("Heraclitus returned an unbound authorization".into());
        }
        Ok(AdminExecutionToken { receipt })
    }
    pub async fn status(&mut self, operation_id: &str) -> Result<Status, Error> {
        self.admin(
            "external-status-v1",
            &serde_json::json!({"operation_id":operation_id}),
        )
        .await
    }
    pub async fn complete(&mut self, completion: &Completion) -> Result<(), Error> {
        let saved: Completion = self.admin("external-result-v1", completion).await?;
        if saved != *completion {
            return Err("unbound completion acknowledgement".into());
        }
        Ok(())
    }
}
