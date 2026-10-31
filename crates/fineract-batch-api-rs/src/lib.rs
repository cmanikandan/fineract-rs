//! `fineract-batch-api-rs` — Transactional JSON Batch API Engine with Parent-Child
//! Dependency Tree Validation and JSONPath (`$.resourceId`, `$.clientId`, `$.loanId`) Resolution.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use fineract_core_rs::context::ExecutionContext;
use fineract_validation_rs::{ApiGlobalErrorResponse, FineractError};

/// HTTP Header pair inside a `BatchRequest` or `BatchResponse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

pub type BatchHeader = Header;

/// Wire-compatible `BatchRequest` matching `org.apache.fineract.batch.domain.BatchRequest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub request_id: i64,
    pub relative_url: String,
    pub method: String,
    #[serde(default)]
    pub headers: Vec<Header>,
    pub reference: Option<i64>,
    pub body: Option<String>,
}

/// Wire-compatible `BatchResponse` matching `org.apache.fineract.batch.domain.BatchResponse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResponse {
    pub request_id: i64,
    pub status_code: u16,
    #[serde(default)]
    pub headers: Vec<Header>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

pub type BatchRequestItem = BatchRequest;
pub type BatchResponseItem = BatchResponse;

/// Resolves `$.field.subfield` JSONPath expressions in `relative_url` and `body` against parent response JSON.
pub struct CommandStrategyUtils;

impl CommandStrategyUtils {
    pub fn resolve_json_path(parent_json: &Value, path: &str) -> Option<String> {
        let trimmed = path.strip_prefix("$.").unwrap_or(path);
        let mut cursor = parent_json;
        for part in trimmed.split('.') {
            cursor = cursor.get(part)?;
        }
        match cursor {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    pub fn interpolate_template(template: &str, parent_body: &str) -> Result<String, FineractError> {
        if !template.contains("$.") {
            return Ok(template.to_string());
        }
        let parsed: Value = serde_json::from_str(parent_body).map_err(|e| {
            FineractError::DatabaseError(format!("Invalid parent JSON body in batch: {e}"))
        })?;

        let mut output = String::with_capacity(template.len());
        let chars: Vec<char> = template.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '.' {
                let start = i;
                i += 2;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                {
                    i += 1;
                }
                let expr: String = chars[start..i].iter().collect();
                let resolved = Self::resolve_json_path(&parsed, &expr).ok_or_else(|| {
                    FineractError::ResourceNotFound {
                        globalisation_code: "error.msg.batch.jsonpath.not.resolved".to_string(),
                        default_message: format!("Unable to resolve batch JSONPath {expr}"),
                        resource_id: expr.clone(),
                    }
                })?;
                output.push_str(&resolved);
            } else {
                output.push(chars[i]);
                i += 1;
            }
        }
        Ok(output)
    }
}

/// Strategy trait executing an individual `BatchRequest`.
#[async_trait]
pub trait CommandStrategy: Send + Sync {
    async fn execute(&self, ctx: &ExecutionContext, req: &BatchRequest) -> BatchResponse;
}

/// Transactional checkpoint interface supporting `enclosingTransaction=true` rollback.
#[async_trait]
pub trait BatchTransactionManager: Send + Sync {
    async fn begin_checkpoint(&self) -> u64;
    async fn rollback_to_checkpoint(&self, checkpoint_id: u64);
    async fn commit_checkpoint(&self, checkpoint_id: u64);
}

/// Core Fineract Batch API Service (`BatchApiServiceImpl.java` parity).
pub struct BatchApiService {
    strategy: Arc<dyn CommandStrategy>,
    tx_manager: Option<Arc<dyn BatchTransactionManager>>,
}

struct DefaultBatchStrategy;

#[async_trait]
impl CommandStrategy for DefaultBatchStrategy {
    async fn execute(&self, _ctx: &ExecutionContext, req: &BatchRequest) -> BatchResponse {
        BatchResponse {
            request_id: req.request_id,
            status_code: 200,
            headers: vec![],
            body: Some(format!(r#"{{"resourceId": {}}}"#, req.request_id)),
        }
    }
}

impl Default for BatchApiService {
    fn default() -> Self {
        Self::new(Arc::new(DefaultBatchStrategy), None)
    }
}

impl BatchApiService {
    pub fn new(
        strategy: Arc<dyn CommandStrategy>,
        tx_manager: Option<Arc<dyn BatchTransactionManager>>,
    ) -> Self {
        Self {
            strategy,
            tx_manager,
        }
    }

    pub fn validate_dependency_graph(requests: &[BatchRequest]) -> Result<(), FineractError> {
        let mut ids = HashSet::new();
        for req in requests {
            if !ids.insert(req.request_id) {
                return Err(FineractError::DomainRule(
                    fineract_validation_rs::PlatformDomainRuleException::new(
                        "error.msg.batch.duplicate.request.id",
                        format!("Duplicate batch requestId {}", req.request_id),
                    ),
                ));
            }
        }
        let parent_map: HashMap<i64, Option<i64>> =
            requests.iter().map(|r| (r.request_id, r.reference)).collect();

        for req in requests {
            if let Some(ref_id) = req.reference {
                if !ids.contains(&ref_id) {
                    return Err(FineractError::DomainRule(
                        fineract_validation_rs::PlatformDomainRuleException::new(
                            "error.msg.batch.reference.invalid",
                            format!("Batch request {} references non-existent {ref_id}", req.request_id),
                        ),
                    ));
                }
                // Cycle check
                let mut visited = HashSet::new();
                visited.insert(req.request_id);
                let mut curr = Some(ref_id);
                while let Some(c) = curr {
                    if !visited.insert(c) {
                        return Err(FineractError::DomainRule(
                            fineract_validation_rs::PlatformDomainRuleException::new(
                                "error.msg.batch.reference.cycle",
                                "Circular reference detected in BatchRequest list",
                            ),
                        ));
                    }
                    curr = parent_map.get(&c).copied().flatten();
                }
            }
        }
        Ok(())
    }

    pub async fn handle_batch_requests_without_enclosing_transaction(
        &self,
        ctx: &ExecutionContext,
        requests: &[BatchRequest],
    ) -> Result<Vec<BatchResponse>, FineractError> {
        Self::validate_dependency_graph(requests)?;
        let mut responses_by_id: HashMap<i64, BatchResponse> = HashMap::new();
        let mut pending: Vec<BatchRequest> = requests.to_vec();
        let mut ordered_output = Vec::with_capacity(requests.len());

        while !pending.is_empty() {
            let mut progressed = false;
            let mut next_pending = Vec::new();

            for mut req in pending {
                if let Some(ref_id) = req.reference {
                    if let Some(parent_resp) = responses_by_id.get(&ref_id) {
                        progressed = true;
                        if parent_resp.status_code >= 400 {
                            let failed_dep = BatchResponse {
                                request_id: req.request_id,
                                status_code: 424,
                                headers: vec![],
                                body: Some(
                                    json!({
                                        "developerMessage": format!("Reference request with id {ref_id} failed"),
                                        "httpStatusCode": "424",
                                        "userMessageGlobalisationCode": "error.msg.batch.reference.failed"
                                    })
                                    .to_string(),
                                ),
                            };
                            responses_by_id.insert(req.request_id, failed_dep.clone());
                            ordered_output.push(failed_dep);
                            continue;
                        }
                        let parent_body = parent_resp.body.as_deref().unwrap_or("{}");
                        req.relative_url =
                            CommandStrategyUtils::interpolate_template(&req.relative_url, parent_body)?;
                        if let Some(body_tmpl) = &req.body {
                            req.body = Some(CommandStrategyUtils::interpolate_template(
                                body_tmpl,
                                parent_body,
                            )?);
                        }
                        let resp = self.strategy.execute(ctx, &req).await;
                        responses_by_id.insert(req.request_id, resp.clone());
                        ordered_output.push(resp);
                    } else {
                        next_pending.push(req);
                    }
                } else {
                    progressed = true;
                    let resp = self.strategy.execute(ctx, &req).await;
                    responses_by_id.insert(req.request_id, resp.clone());
                    ordered_output.push(resp);
                }
            }
            if !progressed {
                break;
            }
            pending = next_pending;
        }
        Ok(ordered_output)
    }

    pub async fn handle_batch_requests_with_enclosing_transaction(
        &self,
        ctx: &ExecutionContext,
        requests: &[BatchRequest],
    ) -> Result<Vec<BatchResponse>, FineractError> {
        let checkpoint = match &self.tx_manager {
            Some(tm) => Some(tm.begin_checkpoint().await),
            None => None,
        };
        let results = self
            .handle_batch_requests_without_enclosing_transaction(ctx, requests)
            .await?;

        if let Some(failed) = results.iter().find(|r| r.status_code >= 400 && r.status_code != 424)
        {
            if let (Some(tm), Some(cp)) = (&self.tx_manager, checkpoint) {
                tm.rollback_to_checkpoint(cp).await;
            }
            return Ok(vec![BatchResponse {
                request_id: failed.request_id,
                status_code: failed.status_code,
                headers: vec![Header {
                    name: "Fineract-Batch-Transaction-Rolled-Back".to_string(),
                    value: "true".to_string(),
                }],
                body: failed.body.clone(),
            }]);
        }

        if let (Some(tm), Some(cp)) = (&self.tx_manager, checkpoint) {
            tm.commit_checkpoint(cp).await;
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBatchStrategy;

    #[async_trait]
    impl CommandStrategy for MockBatchStrategy {
        async fn execute(&self, _ctx: &ExecutionContext, req: &BatchRequest) -> BatchResponse {
            if req.relative_url.contains("fail") {
                return BatchResponse {
                    request_id: req.request_id,
                    status_code: 400,
                    headers: vec![],
                    body: Some(r#"{"userMessageGlobalisationCode":"error.msg.failed"}"#.to_string()),
                };
            }
            BatchResponse {
                request_id: req.request_id,
                status_code: 200,
                headers: vec![],
                body: Some(
                    json!({
                        "resourceId": req.request_id * 100,
                        "clientId": 555,
                        "loanId": 777,
                        "resolvedUrl": req.relative_url,
                        "resolvedBody": req.body
                    })
                    .to_string(),
                ),
            }
        }
    }

    #[tokio::test]
    async fn test_batch_jsonpath_interpolation_and_dependency_chain() {
        let svc = BatchApiService::new(Arc::new(MockBatchStrategy), None);
        let ctx = ExecutionContext::default_for_tenant("default");
        let requests = vec![
            BatchRequest {
                request_id: 1,
                relative_url: "v1/clients".to_string(),
                method: "POST".to_string(),
                headers: vec![],
                reference: None,
                body: Some(r#"{"firstname":"Ada"}"#.to_string()),
            },
            BatchRequest {
                request_id: 2,
                relative_url: "v1/clients/$.clientId/loans".to_string(),
                method: "POST".to_string(),
                headers: vec![],
                reference: Some(1),
                body: Some(r#"{"clientId":$.resourceId,"principal":1000}"#.to_string()),
            },
        ];

        let responses = svc
            .handle_batch_requests_without_enclosing_transaction(&ctx, &requests)
            .await
            .unwrap();
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[1].status_code, 200);
        let body2: Value = serde_json::from_str(responses[1].body.as_ref().unwrap()).unwrap();
        assert_eq!(body2["resolvedUrl"], "v1/clients/555/loans");
        assert_eq!(body2["resolvedBody"], r#"{"clientId":100,"principal":1000}"#);
    }

    #[tokio::test]
    async fn test_failed_parent_propagates_424_failed_dependency() {
        let svc = BatchApiService::new(Arc::new(MockBatchStrategy), None);
        let ctx = ExecutionContext::default_for_tenant("default");
        let requests = vec![
            BatchRequest {
                request_id: 10,
                relative_url: "v1/fail".to_string(),
                method: "POST".to_string(),
                headers: vec![],
                reference: None,
                body: None,
            },
            BatchRequest {
                request_id: 11,
                relative_url: "v1/loans/$.loanId".to_string(),
                method: "GET".to_string(),
                headers: vec![],
                reference: Some(10),
                body: None,
            },
        ];

        let responses = svc
            .handle_batch_requests_without_enclosing_transaction(&ctx, &requests)
            .await
            .unwrap();
        assert_eq!(responses[0].status_code, 400);
        assert_eq!(responses[1].status_code, 424);
    }
}
