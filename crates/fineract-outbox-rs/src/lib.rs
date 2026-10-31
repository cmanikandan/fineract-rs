//! `fineract-outbox-rs` — Transactional External Event Outbox (`m_external_event`),
//! Avro `MessageV1` Serializer, Configuration Registry (`m_external_event_configuration`),
//! and Partitioned Broker Relay & Purge Worker.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use apache_avro::{from_avro_datum, to_avro_datum, to_value, Schema};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use dashmap::DashMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use fineract_avro_rs::{MessageV1, MESSAGE_V1_AVSC};
use fineract_core_rs::db::{DialectSql, SqlDialect};
use fineract_validation_rs::FineractError;

/// Delivery lifecycle state of an outbox event in `m_external_event`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalEventStatus {
    #[serde(rename = "TO_BE_SENT")]
    ToBeSent,
    #[serde(rename = "SENT")]
    Sent,
}

pub type OutboxEventStatus = ExternalEventStatus;

impl ExternalEventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToBeSent => "TO_BE_SENT",
            Self::Sent => "SENT",
        }
    }
}

/// Event enablement row in `m_external_event_configuration`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEventConfiguration {
    pub id: i64,
    pub event_type: String,
    pub enabled: bool,
}

/// Transactional Outbox record stored in `m_external_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEventRecord {
    pub id: i64,
    pub event_type: String,
    pub schema_name: String,
    pub data: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub status: ExternalEventStatus,
    pub sent_at: Option<DateTime<Utc>>,
    pub business_date: NaiveDate,
    pub idempotency_key: Uuid,
    pub aggregate_root_id: Option<i64>,
    pub category: String,
}

/// Wire-compatible `MessageV1` envelope published to Kafka or ActiveMQ/JMS.
pub type AvroMessageV1Envelope = MessageV1;

/// Compiles `.avsc` JSON schema strings and performs binary Apache Avro datum encoding/decoding.
pub struct AvroOutboxSerializer;

impl AvroOutboxSerializer {
    pub fn encode_avro<T: Serialize>(value: &T, avsc_json: &str) -> Result<Vec<u8>, FineractError> {
        let schema = Schema::parse_str(avsc_json)
            .map_err(|e| FineractError::DatabaseError(format!("Avro schema parse error: {e}")))?;
        let avro_val = to_value(value)
            .map_err(|e| FineractError::DatabaseError(format!("Avro value error: {e}")))?
            .resolve(&schema)
            .map_err(|e| FineractError::DatabaseError(format!("Avro schema resolve error: {e}")))?;
        to_avro_datum(&schema, avro_val)
            .map_err(|e| FineractError::DatabaseError(format!("Avro encode error: {e}")))
    }

    pub fn decode_avro<T: DeserializeOwned>(
        bytes: &[u8],
        avsc_json: &str,
    ) -> Result<T, FineractError> {
        let schema = Schema::parse_str(avsc_json)
            .map_err(|e| FineractError::DatabaseError(format!("Avro schema parse error: {e}")))?;
        let mut reader = bytes;
        let val = from_avro_datum(&schema, &mut reader, None)
            .map_err(|e| FineractError::DatabaseError(format!("Avro decode error: {e}")))?;
        apache_avro::from_value::<T>(&val)
            .map_err(|e| FineractError::DatabaseError(format!("Avro struct deserialize error: {e}")))
    }

    pub fn wrap_in_message_v1(
        record: &ExternalEventRecord,
        tenant_id: &str,
        source: &str,
    ) -> AvroMessageV1Envelope {
        AvroMessageV1Envelope {
            id: record.id as i64,
            source: source.to_string(),
            r#type: record.event_type.clone(),
            category: record.category.clone(),
            created_at: record.created_at.to_rfc3339(),
            business_date: record.business_date.format("%Y-%m-%d").to_string(),
            tenant_id: tenant_id.to_string(),
            idempotency_key: record.idempotency_key.to_string(),
            dataschema: record.schema_name.clone(),
            data: record.data.clone(),
        }
    }

    pub fn encode_message_v1_envelope(
        envelope: &AvroMessageV1Envelope,
    ) -> Result<Vec<u8>, FineractError> {
        Self::encode_avro(envelope, MESSAGE_V1_AVSC)
    }
}

/// Pluggable broker transport (Kafka / ActiveMQ / In-Memory test bus).
#[async_trait]
pub trait ExternalEventBrokerTransport: Send + Sync {
    async fn publish_batch(
        &self,
        messages: &[AvroMessageV1Envelope],
    ) -> Result<(), FineractError>;
}

/// Thread-safe in-memory broker transport capturing published `MessageV1` envelopes.
#[derive(Debug, Default)]
pub struct InMemoryEventTransport {
    published: Mutex<Vec<AvroMessageV1Envelope>>,
}

impl InMemoryEventTransport {
    pub fn new() -> Self {
        Self {
            published: Mutex::new(Vec::new()),
        }
    }

    pub async fn published_messages(&self) -> Vec<AvroMessageV1Envelope> {
        self.published.lock().await.clone()
    }
}

#[async_trait]
impl ExternalEventBrokerTransport for InMemoryEventTransport {
    async fn publish_batch(
        &self,
        messages: &[AvroMessageV1Envelope],
    ) -> Result<(), FineractError> {
        let mut guard = self.published.lock().await;
        guard.extend_from_slice(messages);
        Ok(())
    }
}

/// Transactional Outbox Repository & Poller Worker supporting PostgreSQL and MySQL/MariaDB.
pub struct TransactionalOutboxWorker {
    next_id: AtomicI64,
    dialect: SqlDialect,
    configs: DashMap<String, ExternalEventConfiguration>,
    records: DashMap<i64, ExternalEventRecord>,
    transport: Arc<dyn ExternalEventBrokerTransport>,
}

impl TransactionalOutboxWorker {
    pub fn new(dialect: SqlDialect, transport: Arc<dyn ExternalEventBrokerTransport>) -> Self {
        Self {
            next_id: AtomicI64::new(1),
            dialect,
            configs: DashMap::new(),
            records: DashMap::new(),
            transport,
        }
    }

    pub fn configure_event(&self, event_type: &str, enabled: bool) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.configs.insert(
            event_type.to_string(),
            ExternalEventConfiguration {
                id,
                event_type: event_type.to_string(),
                enabled,
            },
        );
    }

    pub fn is_event_enabled(&self, event_type: &str) -> bool {
        self.configs
            .get(event_type)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    /// Enqueues an Avro-encoded event into `m_external_event` if enabled in `m_external_event_configuration`.
    pub fn enqueue_domain_event<T: Serialize>(
        &self,
        event_type: &str,
        category: &str,
        schema_name: &str,
        avsc_json: &str,
        payload: &T,
        business_date: NaiveDate,
        aggregate_root_id: Option<i64>,
    ) -> Result<Option<ExternalEventRecord>, FineractError> {
        if !self.is_event_enabled(event_type) {
            return Ok(None);
        }
        let data = AvroOutboxSerializer::encode_avro(payload, avsc_json)?;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let record = ExternalEventRecord {
            id,
            event_type: event_type.to_string(),
            schema_name: schema_name.to_string(),
            data,
            created_at: Utc::now(),
            status: ExternalEventStatus::ToBeSent,
            sent_at: None,
            business_date,
            idempotency_key: Uuid::new_v4(),
            aggregate_root_id,
            category: category.to_string(),
        };
        self.records.insert(id, record.clone());
        Ok(Some(record))
    }

    /// Polls `TO_BE_SENT` events in ascending ID order, publishes them as `MessageV1` envelopes, and marks them `SENT`.
    pub async fn relay_pending_batch(
        &self,
        tenant_id: &str,
        batch_limit: usize,
    ) -> Result<usize, FineractError> {
        let mut pending: Vec<ExternalEventRecord> = self
            .records
            .iter()
            .filter(|r| r.value().status == ExternalEventStatus::ToBeSent)
            .map(|r| r.value().clone())
            .collect();
        pending.sort_by_key(|r| r.id);
        pending.truncate(batch_limit);

        if pending.is_empty() {
            return Ok(0);
        }

        let envelopes: Vec<AvroMessageV1Envelope> = pending
            .iter()
            .map(|r| AvroOutboxSerializer::wrap_in_message_v1(r, tenant_id, "fineract-rs"))
            .collect();

        self.transport.publish_batch(&envelopes).await?;

        let now = Utc::now();
        for rec in &pending {
            if let Some(mut entry) = self.records.get_mut(&rec.id) {
                entry.status = ExternalEventStatus::Sent;
                entry.sent_at = Some(now);
            }
        }
        Ok(pending.len())
    }

    /// Purges `SENT` events older than `older_than` (`PurgeExternalEventsTasklet` parity).
    pub fn purge_sent_events(&self, older_than: DateTime<Utc>) -> usize {
        let to_remove: Vec<i64> = self
            .records
            .iter()
            .filter(|r| {
                r.value().status == ExternalEventStatus::Sent
                    && r.value().sent_at.map(|ts| ts <= older_than).unwrap_or(false)
            })
            .map(|r| *r.key())
            .collect();
        let count = to_remove.len();
        for id in to_remove {
            self.records.remove(&id);
        }
        count
    }

    pub fn select_pending_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, type, schema, data, created_at, status, sent_at, business_date, idempotency_key, aggregate_root_id, category FROM m_external_event WHERE status = ? ORDER BY id ASC LIMIT ?",
            self.dialect,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fineract_avro_rs::{BulkMessageItemV1, BULK_MESSAGE_ITEM_V1_AVSC};

    #[tokio::test]
    async fn test_avro_serialization_and_outbox_relay_and_purge() {
        let transport = Arc::new(InMemoryEventTransport::new());
        let worker = TransactionalOutboxWorker::new(SqlDialect::Postgres, transport.clone());

        let item = BulkMessageItemV1 {
            id: 42,
            r#type: "LoanCreatedBusinessEvent".to_string(),
            category: "LOAN".to_string(),
            dataschema: "org.apache.fineract.avro.loan.v1.LoanAccountDataV1".to_string(),
            data: vec![1, 2, 3, 4],
        };

        // Disabled by default -> returns None
        let bdate = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let skipped = worker
            .enqueue_domain_event(
                "LoanCreatedBusinessEvent",
                "LOAN",
                "BulkMessageItemV1",
                BULK_MESSAGE_ITEM_V1_AVSC,
                &item,
                bdate,
                Some(100),
            )
            .unwrap();
        assert!(skipped.is_none());

        // Enable event and enqueue
        worker.configure_event("LoanCreatedBusinessEvent", true);
        let rec = worker
            .enqueue_domain_event(
                "LoanCreatedBusinessEvent",
                "LOAN",
                "BulkMessageItemV1",
                BULK_MESSAGE_ITEM_V1_AVSC,
                &item,
                bdate,
                Some(100),
            )
            .unwrap()
            .unwrap();

        let decoded: BulkMessageItemV1 =
            AvroOutboxSerializer::decode_avro(&rec.data, BULK_MESSAGE_ITEM_V1_AVSC).unwrap();
        assert_eq!(decoded, item);

        // Relay batch
        let sent_count = worker.relay_pending_batch("default", 50).await.unwrap();
        assert_eq!(sent_count, 1);
        assert_eq!(transport.published_messages().await.len(), 1);

        // Purge sent events
        let purged = worker.purge_sent_events(Utc::now() + chrono::Duration::seconds(1));
        assert_eq!(purged, 1);
    }
}

#[derive(Debug, Default)]
pub struct OutboxEventRepository {
    events: DashMap<i64, ExternalEventRecord>,
    next_id: AtomicI64,
}

impl OutboxEventRepository {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn total_count(&self) -> usize {
        self.events.len()
    }
    pub fn enqueue_event(
        &self,
        _tenant_id: &str,
        event_type: &str,
        category: &str,
        aggregate_root_id: i64,
        business_date: NaiveDate,
        data: Vec<u8>,
    ) -> i64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let record = ExternalEventRecord {
            id,
            event_type: event_type.to_string(),
            schema_name: format!("{}.avsc", event_type),
            data,
            created_at: Utc::now(),
            status: ExternalEventStatus::ToBeSent,
            sent_at: None,
            business_date,
            idempotency_key: Uuid::new_v4(),
            aggregate_root_id: Some(aggregate_root_id),
            category: category.to_string(),
        };
        self.events.insert(id, record);
        id
    }
}

#[derive(Debug, Default)]
pub struct ExternalEventConfigurationService {
    configs: DashMap<String, bool>,
}

impl ExternalEventConfigurationService {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn list_configurations(&self) -> Vec<ExternalEventConfiguration> {
        let mut res = Vec::new();
        let mut id = 1;
        for entry in self.configs.iter() {
            res.push(ExternalEventConfiguration {
                id,
                event_type: entry.key().clone(),
                enabled: *entry.value(),
            });
            id += 1;
        }
        res
    }
    pub fn set_enabled(&self, event_type: &str, enabled: bool) {
        self.configs.insert(event_type.to_string(), enabled);
    }
}

#[derive(Debug)]
pub struct OutboxRelayDispatcher {
    pub repo: Arc<OutboxEventRepository>,
    pub config: Arc<ExternalEventConfigurationService>,
    pub batch_size: usize,
}

impl OutboxRelayDispatcher {
    pub fn new(
        repo: Arc<OutboxEventRepository>,
        config: Arc<ExternalEventConfigurationService>,
        batch_size: usize,
    ) -> Self {
        Self { repo, config, batch_size }
    }

    pub fn relay_next_batch<F>(&self, mut callback: F) -> Result<usize, FineractError>
    where
        F: FnMut(&ExternalEventRecord) -> Result<(), FineractError>,
    {
        let mut sent = 0;
        for mut entry in self.repo.events.iter_mut() {
            if entry.value().status == ExternalEventStatus::ToBeSent {
                callback(entry.value())?;
                entry.value_mut().status = ExternalEventStatus::Sent;
                entry.value_mut().sent_at = Some(Utc::now());
                sent += 1;
                if sent >= self.batch_size {
                    break;
                }
            }
        }
        Ok(sent)
    }
}
