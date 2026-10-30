//! Dual-Database Command Persistence (`m_portfolio_command_source`) and High-Throughput Command Executors.
//!
//! Ported from:
//! - `fineract-core`: `CommandSource.java`, `CommandSourceRepository.java`, `CommandSourceService.java`,
//!   `SynchronousCommandProcessingService.java`, `PurgeProcessedCommandsTasklet.java`
//! - `fineract-command-jdbc`: `JdbcCommandStore.java`, `CommandEntity.java`, `CommandRepository.java`
//! - `fineract-command-async`: `AsyncCommandProperties.java`, `AsyncCommandDispatcher.java`
//! - `fineract-command-disruptor`: `DisruptorCommandProperties.java`, `DisruptorCommandConfiguration.java`,
//!   `DisruptorCommandDispatcher.java`

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::{AnyPool, Row};
use tokio::sync::{mpsc, oneshot, Mutex, Notify};

use fineract_core_rs::{DialectSql, ExecutionContext, JsonCommand, SqlDialect};
use fineract_validation_rs::FineractError;

use crate::pipeline::{
    CommandHandler, CommandPipeline, CommandProcessingResult, CommandProcessingResultType,
    CommandWrapper,
};

/// Canonical record mapping to `m_portfolio_command_source` in `anchor/fineract_schema_dual.sql`.
///
/// Tracks the full lifecycle of a command across Maker-Checker two-step approval, idempotency
/// replay, and audit logging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSourceRecord {
    pub id: i64,
    pub action_name: String,
    pub entity_name: String,
    pub office_id: Option<i64>,
    pub group_id: Option<i64>,
    pub client_id: Option<i64>,
    pub loan_id: Option<i64>,
    pub savings_account_id: Option<i64>,
    pub api_get_url: String,
    pub resource_id: Option<i64>,
    pub subresource_id: Option<i64>,
    pub command_as_json: String,
    pub maker_id: i64,
    pub made_on_date: DateTime<Utc>,
    pub checker_id: Option<i64>,
    pub checked_on_date: Option<DateTime<Utc>>,
    pub processing_result_enum: i16,
    pub product_id: Option<i64>,
    pub transaction_id: Option<String>,
    pub creditbureau_id: Option<i64>,
    pub organisation_creditbureau_id: Option<i64>,
    pub idempotency_key: Option<String>,
    pub result: Option<String>,
    pub result_status_code: Option<i32>,
    pub resource_external_id: Option<String>,
    pub subresource_external_id: Option<String>,
}

impl CommandSourceRecord {
    /// Creates an initial `CommandSourceRecord` from a [`CommandWrapper`] in `UNDER_PROCESSING` state
    /// (matching `CommandSource.fullEntryFrom` and `CommandSourceService.getInitialCommandSource`).
    pub fn from_wrapper(
        wrapper: &CommandWrapper,
        maker_id: i64,
        status: CommandProcessingResultType,
    ) -> Self {
        let json_body = if wrapper.json.trim().is_empty() {
            "{}".to_string()
        } else {
            wrapper.json.clone()
        };
        Self {
            id: wrapper.command_id.unwrap_or(0),
            action_name: wrapper.action_name.clone(),
            entity_name: wrapper.entity_name.clone(),
            office_id: wrapper.office_id,
            group_id: wrapper.group_id,
            client_id: wrapper.client_id,
            loan_id: wrapper.loan_id,
            savings_account_id: wrapper.savings_id,
            api_get_url: wrapper.href.clone().unwrap_or_default(),
            resource_id: wrapper.entity_id,
            subresource_id: wrapper.subentity_id,
            command_as_json: json_body,
            maker_id,
            made_on_date: Utc::now(),
            checker_id: None,
            checked_on_date: None,
            processing_result_enum: status.as_i16(),
            product_id: wrapper.product_id,
            transaction_id: None,
            creditbureau_id: wrapper.creditbureau_id,
            organisation_creditbureau_id: wrapper.organisation_creditbureau_id,
            idempotency_key: wrapper.idempotency_key.clone(),
            result: None,
            result_status_code: None,
            resource_external_id: None,
            subresource_external_id: None,
        }
    }

    /// Returns `{action_name}_{entity_name}` permission code (matching `CommandSource.getPermissionCode()`).
    pub fn permission_code(&self) -> String {
        format!("{}_{}", self.action_name, self.entity_name)
    }

    /// Parses `processing_result_enum` into [`CommandProcessingResultType`].
    pub fn status_type(&self) -> CommandProcessingResultType {
        CommandProcessingResultType::from_i16(self.processing_result_enum)
    }

    /// Updates `processing_result_enum` from [`CommandProcessingResultType`].
    pub fn set_status_type(&mut self, status: CommandProcessingResultType) {
        self.processing_result_enum = status.as_i16();
    }

    pub fn is_processed(&self) -> bool {
        self.status_type() == CommandProcessingResultType::Processed
    }

    pub fn is_awaiting_approval(&self) -> bool {
        self.status_type() == CommandProcessingResultType::AwaitingApproval
    }

    pub fn is_rejected(&self) -> bool {
        self.status_type() == CommandProcessingResultType::Rejected
    }

    pub fn is_under_processing(&self) -> bool {
        self.status_type() == CommandProcessingResultType::UnderProcessing
    }

    pub fn is_error(&self) -> bool {
        self.status_type() == CommandProcessingResultType::Error
    }

    pub fn mark_as_awaiting_approval(&mut self) {
        self.set_status_type(CommandProcessingResultType::AwaitingApproval);
    }

    pub fn mark_as_checked(&mut self, checker_id: i64, checked_on: DateTime<Utc>) {
        self.checker_id = Some(checker_id);
        self.checked_on_date = Some(checked_on);
        self.set_status_type(CommandProcessingResultType::Processed);
    }

    pub fn mark_as_rejected(&mut self, checker_id: i64, checked_on: DateTime<Utc>) {
        self.checker_id = Some(checker_id);
        self.checked_on_date = Some(checked_on);
        self.set_status_type(CommandProcessingResultType::Rejected);
    }

    /// Updates entity/resource audit identifiers from a [`CommandProcessingResult`]
    /// (ported from `CommandSource.updateForAudit`).
    pub fn update_for_audit(&mut self, result: &CommandProcessingResult) {
        if result.office_id.is_some() {
            self.office_id = result.office_id;
        }
        if result.group_id.is_some() {
            self.group_id = result.group_id;
        }
        if result.client_id.is_some() {
            self.client_id = result.client_id;
        }
        if result.loan_id.is_some() {
            self.loan_id = result.loan_id;
        }
        if result.savings_id.is_some() {
            self.savings_account_id = result.savings_id;
        }
        if result.resource_id.is_some() {
            self.resource_id = result.resource_id;
        }
        if result.sub_resource_id.is_some() {
            self.subresource_id = result.sub_resource_id;
        }
        if result.transaction_id.is_some() {
            self.transaction_id = result.transaction_id.clone();
        }
        if let Some(ref ext_id) = result.resource_external_id {
            self.resource_external_id = ext_id.value().map(str::to_owned);
        }
        if let Some(ref sub_ext_id) = result.sub_resource_external_id {
            self.subresource_external_id = sub_ext_id.value().map(str::to_owned);
        }
    }
}

/// Async persistence contract for Fineract command audit & idempotency (`m_portfolio_command_source`).
#[async_trait]
pub trait CommandSourceStore: Send + Sync {
    /// Looks up a command source by `(action_name, entity_name, idempotency_key)`
    /// (matching `CommandSourceRepository.findByActionNameAndEntityNameAndIdempotencyKey`).
    async fn find_by_idempotency_key(
        &self,
        action_name: &str,
        entity_name: &str,
        idempotency_key: &str,
    ) -> Result<Option<CommandSourceRecord>, FineractError>;

    /// Looks up a command source by primary key `id`.
    async fn find_by_id(&self, id: i64) -> Result<Option<CommandSourceRecord>, FineractError>;

    /// Inserts a new command source record (`UNDER_PROCESSING` or `AWAITING_APPROVAL`).
    /// Returns `Err(FineractError::IdempotencyConflict)` if a record with the same
    /// `(action_name, entity_name, idempotency_key)` already exists.
    async fn insert_command_source(
        &self,
        record: CommandSourceRecord,
    ) -> Result<CommandSourceRecord, FineractError>;

    /// Updates a command source record after execution, approval, rejection, or failure.
    #[allow(clippy::too_many_arguments)]
    async fn update_command_result(
        &self,
        id: i64,
        processing_result_enum: i16,
        result_status_code: Option<i32>,
        result: Option<String>,
        checker_id: Option<i64>,
        checked_on_date: Option<DateTime<Utc>>,
        resource_id: Option<i64>,
        subresource_id: Option<i64>,
        transaction_id: Option<String>,
        resource_external_id: Option<String>,
        subresource_external_id: Option<String>,
    ) -> Result<CommandSourceRecord, FineractError>;
}

/// Fast concurrent in-memory implementation of [`CommandSourceStore`] backed by [`DashMap`].
#[derive(Debug)]
pub struct InMemoryCommandSourceStore {
    records_by_id: DashMap<i64, CommandSourceRecord>,
    id_by_composite_key: DashMap<(String, String, String), i64>,
    next_id: AtomicI64,
}

impl Default for InMemoryCommandSourceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryCommandSourceStore {
    pub fn new() -> Self {
        Self {
            records_by_id: DashMap::new(),
            id_by_composite_key: DashMap::new(),
            next_id: AtomicI64::new(1),
        }
    }

    pub fn len(&self) -> usize {
        self.records_by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records_by_id.is_empty()
    }
}

#[async_trait]
impl CommandSourceStore for InMemoryCommandSourceStore {
    async fn find_by_idempotency_key(
        &self,
        action_name: &str,
        entity_name: &str,
        idempotency_key: &str,
    ) -> Result<Option<CommandSourceRecord>, FineractError> {
        let composite = (
            action_name.to_owned(),
            entity_name.to_owned(),
            idempotency_key.to_owned(),
        );
        if let Some(id_ref) = self.id_by_composite_key.get(&composite) {
            let id = *id_ref.value();
            return Ok(self.records_by_id.get(&id).map(|r| r.value().clone()));
        }
        Ok(None)
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<CommandSourceRecord>, FineractError> {
        Ok(self.records_by_id.get(&id).map(|r| r.value().clone()))
    }

    async fn insert_command_source(
        &self,
        mut record: CommandSourceRecord,
    ) -> Result<CommandSourceRecord, FineractError> {
        let assigned_id = if record.id > 0 {
            record.id
        } else {
            self.next_id.fetch_add(1, Ordering::SeqCst)
        };
        record.id = assigned_id;

        if let Some(ref idem_key) = record.idempotency_key {
            let composite = (
                record.action_name.clone(),
                record.entity_name.clone(),
                idem_key.clone(),
            );
            // Atomic check-and-insert to emulate UNIQUE_PORTFOLIO_COMMAND_SOURCE
            use dashmap::mapref::entry::Entry;
            match self.id_by_composite_key.entry(composite) {
                Entry::Occupied(_) => {
                    return Err(FineractError::IdempotencyConflict);
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(assigned_id);
                }
            }
        }

        self.records_by_id.insert(assigned_id, record.clone());
        Ok(record)
    }

    async fn update_command_result(
        &self,
        id: i64,
        processing_result_enum: i16,
        result_status_code: Option<i32>,
        result: Option<String>,
        checker_id: Option<i64>,
        checked_on_date: Option<DateTime<Utc>>,
        resource_id: Option<i64>,
        subresource_id: Option<i64>,
        transaction_id: Option<String>,
        resource_external_id: Option<String>,
        subresource_external_id: Option<String>,
    ) -> Result<CommandSourceRecord, FineractError> {
        let mut entry = self.records_by_id.get_mut(&id).ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.command.id.invalid".to_string(),
                default_message: format!("Command source with identifier {id} does not exist"),
                resource_id: id.to_string(),
            }
        })?;

        entry.processing_result_enum = processing_result_enum;
        entry.result_status_code = result_status_code;
        entry.result = result;
        if checker_id.is_some() {
            entry.checker_id = checker_id;
        }
        if checked_on_date.is_some() {
            entry.checked_on_date = checked_on_date;
        }
        if resource_id.is_some() {
            entry.resource_id = resource_id;
        }
        if subresource_id.is_some() {
            entry.subresource_id = subresource_id;
        }
        if transaction_id.is_some() {
            entry.transaction_id = transaction_id;
        }
        if resource_external_id.is_some() {
            entry.resource_external_id = resource_external_id;
        }
        if subresource_external_id.is_some() {
            entry.subresource_external_id = subresource_external_id;
        }

        Ok(entry.value().clone())
    }
}

/// Dual-Database (`PostgreSQL` + `MySQL/MariaDB`) JDBC/SQLx store for `m_portfolio_command_source`.
///
/// Uses [`sqlx::AnyPool`] and [`DialectSql::rewrite_placeholders`] so the exact same repository
/// runs natively on both PostgreSQL (`$1..$N`) and MySQL/MariaDB (`?`).
#[derive(Debug, Clone)]
pub struct DualDbCommandSourceJdbcStore {
    pool: AnyPool,
    dialect: SqlDialect,
}

/// Alias for [`DualDbCommandSourceJdbcStore`].
pub type CommandSourceJdbcStore = DualDbCommandSourceJdbcStore;

impl DualDbCommandSourceJdbcStore {
    pub const SELECT_COLUMNS: &'static str = "id, action_name, entity_name, office_id, group_id, \
        client_id, loan_id, savings_account_id, api_get_url, resource_id, subresource_id, \
        command_as_json, maker_id, made_on_date, checker_id, checked_on_date, \
        status AS processing_result_enum, product_id, transaction_id, creditbureau_id, \
        organisation_creditbureau_id, idempotency_key, result, result_status_code, \
        resource_external_id, subresource_external_id";

    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self { pool, dialect }
    }

    pub fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }

    /// Generates the dialect-specific SQL for `find_by_idempotency_key`.
    pub fn select_by_idempotency_key_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_portfolio_command_source WHERE action_name = ? AND entity_name = ? AND idempotency_key = ?",
            Self::SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    /// Generates the dialect-specific SQL for `find_by_id`.
    pub fn select_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_portfolio_command_source WHERE id = ?",
            Self::SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    /// Generates the dialect-specific SQL for `insert_command_source`.
    pub fn insert_command_source_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_portfolio_command_source (\
            id, action_name, entity_name, office_id, group_id, client_id, loan_id, \
            savings_account_id, api_get_url, resource_id, subresource_id, command_as_json, \
            maker_id, made_on_date, checker_id, checked_on_date, status, product_id, \
            transaction_id, creditbureau_id, organisation_creditbureau_id, idempotency_key, \
            result, result_status_code, resource_external_id, subresource_external_id, is_sanitized\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Generates the dialect-specific SQL for `update_command_result`.
    pub fn update_command_result_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_portfolio_command_source SET \
            status = ?, result_status_code = ?, result = ?, checker_id = ?, checked_on_date = ?, \
            resource_id = ?, subresource_id = ?, transaction_id = ?, resource_external_id = ?, \
            subresource_external_id = ? \
            WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Generates the dialect-specific SQL for purging older commands by status
    /// (ported from `CommandSourceRepository.deleteOlderEventsWithStatus`).
    pub fn delete_older_events_with_status_sql(dialect: SqlDialect) -> String {
        let raw = "DELETE FROM m_portfolio_command_source WHERE status = ? AND made_on_date IS NOT NULL AND made_on_date <= ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    fn map_row(row: &sqlx::any::AnyRow) -> Result<CommandSourceRecord, FineractError> {
        let made_on_naive: NaiveDateTime = row
            .try_get("made_on_date")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let checked_on_naive: Option<NaiveDateTime> = row
            .try_get("checked_on_date")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(CommandSourceRecord {
            id: row
                .try_get("id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            action_name: row
                .try_get("action_name")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            entity_name: row
                .try_get("entity_name")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            office_id: row
                .try_get("office_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            group_id: row
                .try_get("group_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            client_id: row
                .try_get("client_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            loan_id: row
                .try_get("loan_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            savings_account_id: row
                .try_get("savings_account_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            api_get_url: row
                .try_get("api_get_url")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            resource_id: row
                .try_get("resource_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            subresource_id: row
                .try_get("subresource_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            command_as_json: row
                .try_get("command_as_json")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            maker_id: row
                .try_get("maker_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            made_on_date: DateTime::<Utc>::from_naive_utc_and_offset(made_on_naive, Utc),
            checker_id: row
                .try_get("checker_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            checked_on_date: checked_on_naive
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
            processing_result_enum: row
                .try_get("processing_result_enum")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            product_id: row
                .try_get("product_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            transaction_id: row
                .try_get("transaction_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            creditbureau_id: row
                .try_get("creditbureau_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            organisation_creditbureau_id: row
                .try_get("organisation_creditbureau_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            idempotency_key: row
                .try_get("idempotency_key")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            result: row
                .try_get("result")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            result_status_code: row
                .try_get("result_status_code")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            resource_external_id: row
                .try_get("resource_external_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            subresource_external_id: row
                .try_get("subresource_external_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
        })
    }
}

#[async_trait]
impl CommandSourceStore for DualDbCommandSourceJdbcStore {
    async fn find_by_idempotency_key(
        &self,
        action_name: &str,
        entity_name: &str,
        idempotency_key: &str,
    ) -> Result<Option<CommandSourceRecord>, FineractError> {
        let sql = Self::select_by_idempotency_key_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(action_name)
            .bind(entity_name)
            .bind(idempotency_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(Self::map_row(row)?)),
            None => Ok(None),
        }
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<CommandSourceRecord>, FineractError> {
        let sql = Self::select_by_id_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(Self::map_row(row)?)),
            None => Ok(None),
        }
    }

    async fn insert_command_source(
        &self,
        record: CommandSourceRecord,
    ) -> Result<CommandSourceRecord, FineractError> {
        let sql = Self::insert_command_source_sql(self.dialect);
        let made_on_naive = record.made_on_date.naive_utc();
        let checked_on_naive = record.checked_on_date.map(|dt| dt.naive_utc());

        let exec_res = sqlx::query(&sql)
            .bind(record.id)
            .bind(&record.action_name)
            .bind(&record.entity_name)
            .bind(record.office_id)
            .bind(record.group_id)
            .bind(record.client_id)
            .bind(record.loan_id)
            .bind(record.savings_account_id)
            .bind(&record.api_get_url)
            .bind(record.resource_id)
            .bind(record.subresource_id)
            .bind(&record.command_as_json)
            .bind(record.maker_id)
            .bind(made_on_naive)
            .bind(record.checker_id)
            .bind(checked_on_naive)
            .bind(record.processing_result_enum)
            .bind(record.product_id)
            .bind(&record.transaction_id)
            .bind(record.creditbureau_id)
            .bind(record.organisation_creditbureau_id)
            .bind(&record.idempotency_key)
            .bind(&record.result)
            .bind(record.result_status_code)
            .bind(&record.resource_external_id)
            .bind(&record.subresource_external_id)
            .bind(false)
            .execute(&self.pool)
            .await;

        match exec_res {
            Ok(_) => Ok(record),
            Err(e) => {
                let msg = e.to_string().to_uppercase();
                if msg.contains("UNIQUE") || msg.contains("DUPLICATE") {
                    Err(FineractError::IdempotencyConflict)
                } else {
                    Err(FineractError::DatabaseError(e.to_string()))
                }
            }
        }
    }

    async fn update_command_result(
        &self,
        id: i64,
        processing_result_enum: i16,
        result_status_code: Option<i32>,
        result: Option<String>,
        checker_id: Option<i64>,
        checked_on_date: Option<DateTime<Utc>>,
        resource_id: Option<i64>,
        subresource_id: Option<i64>,
        transaction_id: Option<String>,
        resource_external_id: Option<String>,
        subresource_external_id: Option<String>,
    ) -> Result<CommandSourceRecord, FineractError> {
        let sql = Self::update_command_result_sql(self.dialect);
        let checked_on_naive = checked_on_date.map(|dt| dt.naive_utc());

        sqlx::query(&sql)
            .bind(processing_result_enum)
            .bind(result_status_code)
            .bind(&result)
            .bind(checker_id)
            .bind(checked_on_naive)
            .bind(resource_id)
            .bind(subresource_id)
            .bind(&transaction_id)
            .bind(&resource_external_id)
            .bind(&subresource_external_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        self.find_by_id(id)
            .await?
            .ok_or_else(|| FineractError::ResourceNotFound {
                globalisation_code: "error.msg.command.id.invalid".to_string(),
                default_message: format!("Command source with identifier {id} does not exist"),
                resource_id: id.to_string(),
            })
    }
}

// =============================================================================
// HIGH-THROUGHPUT COMMAND EXECUTORS (fineract-command-async & fineract-command-disruptor)
// =============================================================================

/// Configuration for [`AsyncCommandExecutor`], ported from `AsyncCommandProperties.java`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncCommandConfig {
    pub enabled: bool,
    pub queue_capacity: usize,
    pub timeout: Duration,
}

impl Default for AsyncCommandConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            queue_capacity: 1024,
            timeout: Duration::from_secs(3),
        }
    }
}

struct AsyncWorkItem {
    ctx: ExecutionContext,
    wrapper: CommandWrapper,
    command: JsonCommand,
    approved_by_checker: bool,
    reply_tx: oneshot::Sender<Result<CommandProcessingResult, FineractError>>,
}

/// Bounded `tokio::sync::mpsc` worker pool with `oneshot` response channels,
/// ported from `org.apache.fineract.command.async.implementation.AsyncCommandDispatcher`.
#[derive(Clone)]
pub struct AsyncCommandExecutor {
    sender: mpsc::Sender<AsyncWorkItem>,
    config: AsyncCommandConfig,
    dispatched_count: Arc<AtomicU64>,
}

impl AsyncCommandExecutor {
    /// Spawns an asynchronous command dispatcher loop backed by a bounded `mpsc` channel.
    pub fn new(pipeline: Arc<CommandPipeline>, config: AsyncCommandConfig) -> Self {
        let capacity = config.queue_capacity.max(1);
        let (tx, mut rx) = mpsc::channel::<AsyncWorkItem>(capacity);
        let dispatched_count = Arc::new(AtomicU64::new(0));

        tokio::spawn(async move {
            while let Some(item) = rx.recv().await {
                let pipeline_clone = Arc::clone(&pipeline);
                tokio::spawn(async move {
                    let outcome = pipeline_clone
                        .execute(
                            &item.ctx,
                            &item.wrapper,
                            &item.command,
                            item.approved_by_checker,
                        )
                        .await;
                    let _ = item.reply_tx.send(outcome);
                });
            }
        });

        Self {
            sender: tx,
            config,
            dispatched_count,
        }
    }

    /// Dispatches a command to the async worker pool and awaits the `oneshot` response within `config.timeout`.
    pub async fn dispatch(
        &self,
        ctx: ExecutionContext,
        wrapper: CommandWrapper,
        command: JsonCommand,
        approved_by_checker: bool,
    ) -> Result<CommandProcessingResult, FineractError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let work_item = AsyncWorkItem {
            ctx,
            wrapper,
            command,
            approved_by_checker,
            reply_tx,
        };

        self.sender
            .send(work_item)
            .await
            .map_err(|e| FineractError::DatabaseError(format!("AsyncCommandExecutor closed: {e}")))?;

        self.dispatched_count.fetch_add(1, Ordering::Relaxed);

        match tokio::time::timeout(self.config.timeout, reply_rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err(FineractError::DatabaseError(
                "AsyncCommandExecutor worker dropped reply channel".to_string(),
            )),
            Err(_) => Err(FineractError::DatabaseError(format!(
                "AsyncCommandExecutor timed out after {:?}",
                self.config.timeout
            ))),
        }
    }

    pub fn dispatched_count(&self) -> u64 {
        self.dispatched_count.load(Ordering::Relaxed)
    }
}

/// Configuration for [`RingBufferDisruptorExecutor`], ported from `DisruptorCommandProperties.java`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisruptorCommandConfig {
    pub enabled: bool,
    /// Must be a power of two (default: `1024`, matching `DisruptorCommandProperties.ringBufferSize`).
    pub ring_buffer_size: usize,
    /// Maximum batch size drained in a single ring-buffer sweep (`endOfBatch` boundary).
    pub max_batch_size: usize,
}

impl Default for DisruptorCommandConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ring_buffer_size: 1024,
            max_batch_size: 64,
        }
    }
}

struct DisruptorSlotEvent {
    ctx: ExecutionContext,
    wrapper: CommandWrapper,
    command: JsonCommand,
    approved_by_checker: bool,
    reply_tx: oneshot::Sender<Result<CommandProcessingResult, FineractError>>,
}

/// Power-of-two pre-allocated ring buffer batching command commits for ultra-low-latency throughput,
/// ported from `org.apache.fineract.command.disruptor.implementation.DisruptorCommandDispatcher`.
///
/// Mirrors the LMAX Disruptor mechanical-sympathy design:
/// - Pre-allocated power-of-two slot array indexed via `sequence & index_mask` (`ring_buffer_size - 1`).
/// - Producers claim a monotonically increasing sequence via `claim_cursor`, populate the slot,
///   and notify the batch consumer processor.
/// - The consumer drains all contiguous available slots up to `max_batch_size` in a single batch
///   pass (`on_event(event, sequence, end_of_batch)`), amortizing wakeups and commit overhead.
#[derive(Clone)]
pub struct RingBufferDisruptorExecutor {
    slots: Arc<Vec<Mutex<Option<DisruptorSlotEvent>>>>,
    index_mask: usize,
    ring_buffer_size: usize,
    claim_cursor: Arc<AtomicU64>,
    consumer_cursor: Arc<AtomicU64>,
    data_available: Arc<Notify>,
    space_available: Arc<Notify>,
    total_processed: Arc<AtomicU64>,
    total_batches: Arc<AtomicU64>,
    max_batch_observed: Arc<AtomicU64>,
}

/// Alias for [`RingBufferDisruptorExecutor`].
pub type DisruptorCommandExecutor = RingBufferDisruptorExecutor;

impl RingBufferDisruptorExecutor {
    /// Creates and starts a new `RingBufferDisruptorExecutor` with a power-of-two ring buffer.
    ///
    /// # Panics
    /// Panics if `config.ring_buffer_size` is 0 or not a power of two (matching LMAX Disruptor's
    /// `RingBuffer` invariant `Integer.bitCount(bufferSize) == 1`).
    pub fn new(pipeline: Arc<CommandPipeline>, config: DisruptorCommandConfig) -> Self {
        assert!(
            config.ring_buffer_size > 0 && config.ring_buffer_size.is_power_of_two(),
            "ring_buffer_size must be a positive power of two (got {})",
            config.ring_buffer_size
        );

        let ring_buffer_size = config.ring_buffer_size;
        let index_mask = ring_buffer_size - 1;
        let max_batch_size = config.max_batch_size.max(1);

        let mut raw_slots = Vec::with_capacity(ring_buffer_size);
        for _ in 0..ring_buffer_size {
            raw_slots.push(Mutex::new(None));
        }
        let slots = Arc::new(raw_slots);
        let claim_cursor = Arc::new(AtomicU64::new(0));
        let consumer_cursor = Arc::new(AtomicU64::new(0));
        let data_available = Arc::new(Notify::new());
        let space_available = Arc::new(Notify::new());
        let total_processed = Arc::new(AtomicU64::new(0));
        let total_batches = Arc::new(AtomicU64::new(0));
        let max_batch_observed = Arc::new(AtomicU64::new(0));

        let slots_consumer = Arc::clone(&slots);
        let claim_consumer = Arc::clone(&claim_cursor);
        let cursor_consumer = Arc::clone(&consumer_cursor);
        let data_notify = Arc::clone(&data_available);
        let space_notify = Arc::clone(&space_available);
        let processed_counter = Arc::clone(&total_processed);
        let batches_counter = Arc::clone(&total_batches);
        let max_batch_counter = Arc::clone(&max_batch_observed);

        tokio::spawn(async move {
            loop {
                let current_read = cursor_consumer.load(Ordering::Acquire);
                let current_write = claim_consumer.load(Ordering::Acquire);

                if current_read >= current_write {
                    data_notify.notified().await;
                    continue;
                }

                // Drain up to `max_batch_size` contiguous ready events from the ring buffer
                let available = (current_write - current_read) as usize;
                let batch_limit = available.min(max_batch_size);
                let mut batch = Vec::with_capacity(batch_limit);

                for offset in 0..batch_limit {
                    let seq = current_read + offset as u64;
                    let slot_idx = (seq as usize) & index_mask;
                    let maybe_event = {
                        let mut guard = slots_consumer[slot_idx].lock().await;
                        guard.take()
                    };
                    if let Some(ev) = maybe_event {
                        batch.push((seq, ev));
                    } else {
                        // Producer claimed `seq` but has not finished writing the slot yet;
                        // stop the batch at this contiguous boundary and yield briefly.
                        break;
                    }
                }

                if batch.is_empty() {
                    tokio::task::yield_now().await;
                    continue;
                }

                let batch_len = batch.len() as u64;
                cursor_consumer.fetch_add(batch_len, Ordering::Release);
                space_notify.notify_waiters();

                batches_counter.fetch_add(1, Ordering::Relaxed);
                max_batch_counter.fetch_max(batch_len, Ordering::Relaxed);

                // Execute all events in the drained batch (`endOfBatch` is true on the final element)
                let last_idx = batch.len() - 1;
                for (idx, (_seq, event)) in batch.into_iter().enumerate() {
                    let _end_of_batch = idx == last_idx;
                    let outcome = pipeline
                        .execute(
                            &event.ctx,
                            &event.wrapper,
                            &event.command,
                            event.approved_by_checker,
                        )
                        .await;
                    processed_counter.fetch_add(1, Ordering::Relaxed);
                    let _ = event.reply_tx.send(outcome);
                }
            }
        });

        Self {
            slots,
            index_mask,
            ring_buffer_size,
            claim_cursor,
            consumer_cursor,
            data_available,
            space_available,
            total_processed,
            total_batches,
            max_batch_observed,
        }
    }

    /// Publishes a command into the next pre-allocated ring-buffer slot and awaits batch completion.
    pub async fn dispatch(
        &self,
        ctx: ExecutionContext,
        wrapper: CommandWrapper,
        command: JsonCommand,
        approved_by_checker: bool,
    ) -> Result<CommandProcessingResult, FineractError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let seq = self.claim_cursor.fetch_add(1, Ordering::AcqRel);

        // Backpressure check: ensure we do not wrap around and overwrite an unconsumed slot
        loop {
            let read_seq = self.consumer_cursor.load(Ordering::Acquire);
            if seq - read_seq < self.ring_buffer_size as u64 {
                break;
            }
            self.space_available.notified().await;
        }

        let slot_idx = (seq as usize) & self.index_mask;
        {
            let mut slot_guard = self.slots[slot_idx].lock().await;
            *slot_guard = Some(DisruptorSlotEvent {
                ctx,
                wrapper,
                command,
                approved_by_checker,
                reply_tx,
            });
        }

        self.data_available.notify_one();

        reply_rx.await.map_err(|_| {
            FineractError::DatabaseError(
                "RingBufferDisruptorExecutor consumer dropped reply channel".to_string(),
            )
        })?
    }

    pub fn ring_buffer_size(&self) -> usize {
        self.ring_buffer_size
    }

    pub fn index_mask(&self) -> usize {
        self.index_mask
    }

    pub fn total_processed(&self) -> u64 {
        self.total_processed.load(Ordering::Acquire)
    }

    pub fn total_batches(&self) -> u64 {
        self.total_batches.load(Ordering::Acquire)
    }

    pub fn max_batch_observed(&self) -> u64 {
        self.max_batch_observed.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{
        CommandHandler, CommandPipeline, CommandProcessingResultBuilder, MakerCheckerPolicy,
    };
    use fineract_core_rs::ExternalId;

    struct EchoLoanHandler;

    #[async_trait]
    impl CommandHandler for EchoLoanHandler {
        async fn handle(
            &self,
            ctx: &ExecutionContext,
            command: &JsonCommand,
        ) -> Result<CommandProcessingResult, FineractError> {
            let resource_id = command.entity_id.unwrap_or(100);
            Ok(CommandProcessingResultBuilder::new()
                .with_command_id(command.command_id.unwrap_or(1))
                .with_office_id(1)
                .with_client_id(command.client_id.unwrap_or(10))
                .with_loan_id(resource_id)
                .with_resource_id(resource_id)
                .with_resource_external_id(ExternalId::new("ext-loan-1"))
                .with_transaction_id(format!("TX-{}-{}", ctx.tenant_id, resource_id))
                .build())
        }
    }

    #[tokio::test]
    async fn test_in_memory_command_source_store_crud_and_idempotency_conflict() {
        let store = InMemoryCommandSourceStore::new();
        let wrapper = CommandWrapper {
            command_id: None,
            action_name: "CREATE".to_string(),
            entity_name: "LOAN".to_string(),
            entity_id: Some(42),
            subentity_id: None,
            href: Some("/v1/loans".to_string()),
            json: r#"{"principal": 1000}"#.to_string(),
            idempotency_key: Some("idem-key-001".to_string()),
            office_id: Some(1),
            group_id: None,
            client_id: Some(7),
            loan_id: Some(42),
            savings_id: None,
            product_id: Some(3),
            creditbureau_id: None,
            organisation_creditbureau_id: None,
        };

        let initial = CommandSourceRecord::from_wrapper(
            &wrapper,
            99,
            CommandProcessingResultType::UnderProcessing,
        );
        let inserted = store
            .insert_command_source(initial.clone())
            .await
            .expect("initial insert should succeed");
        assert_eq!(inserted.id, 1);
        assert!(inserted.is_under_processing());
        assert_eq!(inserted.permission_code(), "CREATE_LOAN");

        // Duplicate insert with same (action_name, entity_name, idempotency_key) must fail with IdempotencyConflict
        let dup_err = store
            .insert_command_source(initial)
            .await
            .expect_err("duplicate idempotency key must conflict");
        assert!(matches!(dup_err, FineractError::IdempotencyConflict));

        // Lookup by idempotency key and by ID
        let by_key = store
            .find_by_idempotency_key("CREATE", "LOAN", "idem-key-001")
            .await
            .unwrap()
            .expect("record must be found by idempotency key");
        assert_eq!(by_key.id, 1);
        assert_eq!(by_key.maker_id, 99);

        // Update command result to PROCESSED
        let checked_at = Utc::now();
        let updated = store
            .update_command_result(
                1,
                CommandProcessingResultType::Processed.as_i16(),
                Some(200),
                Some(r#"{"resourceId":42}"#.to_string()),
                Some(100),
                Some(checked_at),
                Some(42),
                None,
                Some("TX-42".to_string()),
                Some("ext-42".to_string()),
                None,
            )
            .await
            .expect("update must succeed");

        assert!(updated.is_processed());
        assert_eq!(updated.result_status_code, Some(200));
        assert_eq!(updated.checker_id, Some(100));
        assert_eq!(updated.transaction_id.as_deref(), Some("TX-42"));
        assert_eq!(updated.resource_external_id.as_deref(), Some("ext-42"));
    }

    #[test]
    fn test_dual_db_command_source_jdbc_store_sql_dialect_generation() {
        // Verify PostgreSQL $1..$N placeholder rewriting
        let pg_select_idem =
            DualDbCommandSourceJdbcStore::select_by_idempotency_key_sql(SqlDialect::Postgres);
        assert!(
            pg_select_idem.contains("action_name = $1 AND entity_name = $2 AND idempotency_key = $3"),
            "Postgres SELECT by idempotency key SQL: {pg_select_idem}"
        );
        assert!(!pg_select_idem.contains('?'));

        // Verify MySQL/MariaDB ? placeholder preservation
        let mysql_select_idem =
            DualDbCommandSourceJdbcStore::select_by_idempotency_key_sql(SqlDialect::MySql);
        assert!(
            mysql_select_idem.contains("action_name = ? AND entity_name = ? AND idempotency_key = ?"),
            "MySQL SELECT by idempotency key SQL: {mysql_select_idem}"
        );
        assert!(!mysql_select_idem.contains("$1"));

        // Verify INSERT SQL has 27 placeholders ($1..$27 on Postgres, 27 '?' on MySQL)
        let pg_insert =
            DualDbCommandSourceJdbcStore::insert_command_source_sql(SqlDialect::Postgres);
        assert!(pg_insert.contains("$1,") && pg_insert.contains("$27)"));
        assert!(!pg_insert.contains('?'));

        let mysql_insert =
            DualDbCommandSourceJdbcStore::insert_command_source_sql(SqlDialect::MySql);
        assert_eq!(mysql_insert.matches('?').count(), 27);

        // Verify UPDATE SQL has 11 placeholders ($1..$11 on Postgres, 11 '?' on MySQL)
        let pg_update =
            DualDbCommandSourceJdbcStore::update_command_result_sql(SqlDialect::Postgres);
        assert!(pg_update.contains("status = $1") && pg_update.contains("WHERE id = $11"));
        assert!(!pg_update.contains('?'));

        let mysql_update =
            DualDbCommandSourceJdbcStore::update_command_result_sql(SqlDialect::MySql);
        assert_eq!(mysql_update.matches('?').count(), 11);
    }

    #[tokio::test]
    async fn test_ring_buffer_disruptor_and_async_executor_concurrent_throughput() {
        let store = Arc::new(InMemoryCommandSourceStore::new());
        let policy = MakerCheckerPolicy::new(false);
        let pipeline = Arc::new(CommandPipeline::new(store.clone(), policy));
        pipeline.register_handler("CREATE", "LOAN", Arc::new(EchoLoanHandler));

        let disruptor = RingBufferDisruptorExecutor::new(
            Arc::clone(&pipeline),
            DisruptorCommandConfig {
                enabled: true,
                ring_buffer_size: 64,
                max_batch_size: 16,
            },
        );
        assert_eq!(disruptor.ring_buffer_size(), 64);
        assert_eq!(disruptor.index_mask(), 63);

        // Dispatch 32 concurrent commands through the power-of-two RingBufferDisruptorExecutor
        let mut handles = Vec::with_capacity(32);
        for i in 1..=32_i64 {
            let exec = disruptor.clone();
            handles.push(tokio::spawn(async move {
                let ctx = ExecutionContext::default();
                let wrapper = CommandWrapper {
                    command_id: None,
                    action_name: "CREATE".to_string(),
                    entity_name: "LOAN".to_string(),
                    entity_id: Some(1000 + i),
                    subentity_id: None,
                    href: Some(format!("/v1/loans/{}", 1000 + i)),
                    json: format!(r#"{{"principal": {}}}"#, i * 100),
                    idempotency_key: Some(format!("disruptor-key-{i}")),
                    office_id: Some(1),
                    group_id: None,
                    client_id: Some(10),
                    loan_id: Some(1000 + i),
                    savings_id: None,
                    product_id: Some(1),
                    creditbureau_id: None,
                    organisation_creditbureau_id: None,
                };
                let cmd = JsonCommand {
                    command_id: None,
                    raw_json: wrapper.json.clone(),
                    parsed: serde_json::from_str(&wrapper.json).unwrap(),
                    entity_id: Some(1000 + i),
                    subresource_id: None,
                    client_id: Some(10),
                    loan_id: Some(1000 + i),
                    savings_id: None,
                };
                exec.dispatch(ctx, wrapper, cmd, false).await
            }));
        }

        for (idx, h) in handles.into_iter().enumerate() {
            let expected_loan_id = 1000 + (idx as i64 + 1);
            let res = h.await.unwrap().expect("disruptor command should succeed");
            assert_eq!(res.loan_id, Some(expected_loan_id));
            assert_eq!(res.resource_id, Some(expected_loan_id));
        }

        assert_eq!(disruptor.total_processed(), 32);
        assert!(disruptor.total_batches() >= 1);
        assert_eq!(store.len(), 32);

        // Also verify AsyncCommandExecutor executes and replays idempotently
        let async_exec = AsyncCommandExecutor::new(
            Arc::clone(&pipeline),
            AsyncCommandConfig {
                enabled: true,
                queue_capacity: 32,
                timeout: Duration::from_secs(3),
            },
        );
        let ctx = ExecutionContext::default();
        let replay_wrapper = CommandWrapper {
            command_id: None,
            action_name: "CREATE".to_string(),
            entity_name: "LOAN".to_string(),
            entity_id: Some(1001),
            subentity_id: None,
            href: Some("/v1/loans/1001".to_string()),
            json: r#"{"principal": 100}"#.to_string(),
            idempotency_key: Some("disruptor-key-1".to_string()),
            office_id: Some(1),
            group_id: None,
            client_id: Some(10),
            loan_id: Some(1001),
            savings_id: None,
            product_id: Some(1),
            creditbureau_id: None,
            organisation_creditbureau_id: None,
        };
        let replay_cmd = JsonCommand {
            command_id: None,
            raw_json: replay_wrapper.json.clone(),
            parsed: serde_json::from_str(&replay_wrapper.json).unwrap(),
            entity_id: Some(1001),
            subresource_id: None,
            client_id: Some(10),
            loan_id: Some(1001),
            savings_id: None,
        };
        let replayed = async_exec
            .dispatch(ctx, replay_wrapper, replay_cmd, false)
            .await
            .expect("idempotent replay via AsyncCommandExecutor should succeed");
        assert_eq!(replayed.loan_id, Some(1001));
        assert_eq!(async_exec.dispatched_count(), 1);
    }
}
