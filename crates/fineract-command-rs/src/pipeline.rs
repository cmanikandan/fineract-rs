//! Apache Fineract CQRS Command Pipeline: `CommandProcessingResultType`, `CommandWrapper`,
//! `CommandProcessingResult`, `CommandProcessingResultBuilder`, `CommandEnvelope<P>`,
//! `CommandHandler`, `MakerCheckerPolicy`, `IdempotencyMiddleware`, and `CommandPipeline`.
//!
//! Ported from:
//! - `org.apache.fineract.commands.domain.CommandProcessingResultType`
//! - `org.apache.fineract.commands.domain.CommandWrapper`
//! - `org.apache.fineract.commands.domain.CommandSource`
//! - `org.apache.fineract.commands.service.CommandWrapperBuilder`
//! - `org.apache.fineract.commands.service.SynchronousCommandProcessingService`
//! - `org.apache.fineract.commands.service.PortfolioCommandSourceWritePlatformServiceImpl`
//! - `org.apache.fineract.commands.service.IdempotencyKeyResolver`
//! - `org.apache.fineract.commands.service.IdempotencyKeyGenerator`
//! - `org.apache.fineract.infrastructure.core.data.CommandProcessingResult`
//! - `org.apache.fineract.infrastructure.core.data.CommandProcessingResultBuilder`
//! - `org.apache.fineract.command.core.Command`
//! - `org.apache.fineract.command.core.CommandState`
//! - `org.apache.fineract.command.core.CommandHandler`
//! - `org.apache.fineract.command.audit.hook.{AuditCommandHookBefore, AuditCommandHookAfter, AuditCommandHookError}`

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use fineract_core_rs::{with_execution_context, ExecutionContext, ExternalId, JsonCommand};
use fineract_validation_rs::{FineractError, PlatformDomainRuleException};

use crate::store_and_exec::{CommandSourceRecord, CommandSourceStore};

/// Lifecycle state of a command in `m_portfolio_command_source` (`status` / `processing_result_enum`).
///
/// Ported from `org.apache.fineract.commands.domain.CommandProcessingResultType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum CommandProcessingResultType {
    #[default]
    Invalid = 0,
    Processed = 1,
    AwaitingApproval = 2,
    Rejected = 3,
    UnderProcessing = 4,
    Error = 5,
}

impl CommandProcessingResultType {
    /// Java-compatible constant alias for `CommandProcessingResultType::Invalid`.
    pub const INVALID: Self = Self::Invalid;
    /// Java-compatible constant alias for `CommandProcessingResultType::Processed`.
    pub const PROCESSED: Self = Self::Processed;
    /// Java-compatible constant alias for `CommandProcessingResultType::AwaitingApproval`.
    pub const AWAITING_APPROVAL: Self = Self::AwaitingApproval;
    /// Java-compatible constant alias for `CommandProcessingResultType::Rejected`.
    pub const REJECTED: Self = Self::Rejected;
    /// Java-compatible constant alias for `CommandProcessingResultType::UnderProcessing`.
    pub const UNDER_PROCESSING: Self = Self::UnderProcessing;
    /// Java-compatible constant alias for `CommandProcessingResultType::Error`.
    pub const ERROR: Self = Self::Error;

    /// Returns the `SMALLINT` (`i16`) database discriminant stored in `m_portfolio_command_source.status`.
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Returns the integer value matching `CommandProcessingResultType.getValue()`.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Returns the globalisation message code matching `CommandProcessingResultType.getCode()`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "commandProcessingResultType.invalid",
            Self::Processed => "commandProcessingResultType.processed",
            Self::AwaitingApproval => "commandProcessingResultType.awaiting.approval",
            Self::Rejected => "commandProcessingResultType.rejected",
            Self::UnderProcessing => "commandProcessingResultType.underProcessing",
            Self::Error => "commandProcessingResultType.error",
        }
    }

    /// Converts an `i16` status value into [`CommandProcessingResultType`], defaulting unknown values to `Invalid`.
    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Processed,
            2 => Self::AwaitingApproval,
            3 => Self::Rejected,
            4 => Self::UnderProcessing,
            5 => Self::Error,
            _ => Self::Invalid,
        }
    }

    /// Converts an optional `i32` status value into [`CommandProcessingResultType`], matching
    /// `CommandProcessingResultType.fromInt(final Integer value)`.
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Processed,
            Some(2) => Self::AwaitingApproval,
            Some(3) => Self::Rejected,
            Some(4) => Self::UnderProcessing,
            Some(5) => Self::Error,
            _ => Self::Invalid,
        }
    }

    /// Returns `true` if `self == CommandProcessingResultType::Processed`.
    pub const fn is_processed(self) -> bool {
        matches!(self, Self::Processed)
    }

    /// Returns `true` if `self == CommandProcessingResultType::AwaitingApproval`.
    pub const fn is_awaiting_approval(self) -> bool {
        matches!(self, Self::AwaitingApproval)
    }

    /// Returns `true` if `self == CommandProcessingResultType::Rejected`.
    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    /// Returns `true` if `self == CommandProcessingResultType::UnderProcessing`.
    pub const fn is_under_processing(self) -> bool {
        matches!(self, Self::UnderProcessing)
    }

    /// Returns `true` if `self == CommandProcessingResultType::Error`.
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }
}

impl fmt::Display for CommandProcessingResultType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Command state enumeration from `fineract-command` (`org.apache.fineract.command.core.CommandState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandState {
    Invalid,
    Processed,
    AwaitingApproval,
    Rejected,
    UnderProcessing,
    Error,
    #[default]
    Unknown,
}

impl From<CommandProcessingResultType> for CommandState {
    fn from(value: CommandProcessingResultType) -> Self {
        match value {
            CommandProcessingResultType::Invalid => Self::Invalid,
            CommandProcessingResultType::Processed => Self::Processed,
            CommandProcessingResultType::AwaitingApproval => Self::AwaitingApproval,
            CommandProcessingResultType::Rejected => Self::Rejected,
            CommandProcessingResultType::UnderProcessing => Self::UnderProcessing,
            CommandProcessingResultType::Error => Self::Error,
        }
    }
}

/// Typed CQRS command envelope ported from `org.apache.fineract.command.core.Command<T>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope<P> {
    pub command_id: Uuid,
    pub idempotency_key: String,
    pub action_name: String,
    pub entity_name: String,
    pub created_at: DateTime<Utc>,
    pub maker_user_id: Option<i64>,
    pub checker_user_id: Option<i64>,
    pub payload: P,
}

/// Type alias matching `org.apache.fineract.command.core.Command<T>`.
pub type Command<P> = CommandEnvelope<P>;

impl<P> CommandEnvelope<P> {
    /// Creates a new [`CommandEnvelope<P>`] with a fresh UUID v4 `command_id` and `Utc::now()` timestamp.
    pub fn new(
        action_name: impl Into<String>,
        entity_name: impl Into<String>,
        idempotency_key: impl Into<String>,
        payload: P,
    ) -> Self {
        Self {
            command_id: Uuid::new_v4(),
            idempotency_key: idempotency_key.into(),
            action_name: action_name.into(),
            entity_name: entity_name.into(),
            created_at: Utc::now(),
            maker_user_id: None,
            checker_user_id: None,
            payload,
        }
    }

    /// Sets `maker_user_id` on the command envelope.
    pub fn with_maker_user_id(mut self, maker_user_id: i64) -> Self {
        self.maker_user_id = Some(maker_user_id);
        self
    }

    /// Sets `checker_user_id` on the command envelope.
    pub fn with_checker_user_id(mut self, checker_user_id: i64) -> Self {
        self.checker_user_id = Some(checker_user_id);
        self
    }

    /// Returns the `{action_name}_{entity_name}` permission code.
    pub fn permission_code(&self) -> String {
        format!("{}_{}", self.action_name, self.entity_name)
    }
}

/// Descriptor and routing metadata for an incoming write command.
///
/// Ported from `org.apache.fineract.commands.domain.CommandWrapper`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandWrapper {
    pub command_id: Option<i64>,
    pub action_name: String,
    pub entity_name: String,
    pub entity_id: Option<i64>,
    pub subentity_id: Option<i64>,
    pub href: Option<String>,
    pub json: String,
    pub idempotency_key: Option<String>,
    pub office_id: Option<i64>,
    pub group_id: Option<i64>,
    pub client_id: Option<i64>,
    pub loan_id: Option<i64>,
    pub savings_id: Option<i64>,
    pub product_id: Option<i64>,
    pub creditbureau_id: Option<i64>,
    pub organisation_creditbureau_id: Option<i64>,
}

impl CommandWrapper {
    /// Creates a minimal [`CommandWrapper`] matching `CommandWrapper.wrap(actionName, entityName, resourceId, subresourceId)`.
    pub fn wrap(
        action_name: impl Into<String>,
        entity_name: impl Into<String>,
        resource_id: Option<i64>,
        subresource_id: Option<i64>,
    ) -> Self {
        Self {
            command_id: None,
            action_name: action_name.into(),
            entity_name: entity_name.into(),
            entity_id: resource_id,
            subentity_id: subresource_id,
            href: None,
            json: "{}".to_string(),
            idempotency_key: None,
            office_id: None,
            group_id: None,
            client_id: None,
            loan_id: None,
            savings_id: None,
            product_id: None,
            creditbureau_id: None,
            organisation_creditbureau_id: None,
        }
    }

    /// Reconstructs a [`CommandWrapper`] from an existing [`CommandSourceRecord`],
    /// matching `CommandWrapper.fromExistingCommand(...)` in `SynchronousCommandProcessingService`.
    pub fn from_existing_record(record: &CommandSourceRecord) -> Self {
        Self {
            command_id: Some(record.id),
            action_name: record.action_name.clone(),
            entity_name: record.entity_name.clone(),
            entity_id: record.resource_id,
            subentity_id: record.subresource_id,
            href: if record.api_get_url.is_empty() {
                None
            } else {
                Some(record.api_get_url.clone())
            },
            json: record.command_as_json.clone(),
            idempotency_key: record.idempotency_key.clone(),
            office_id: record.office_id,
            group_id: record.group_id,
            client_id: record.client_id,
            loan_id: record.loan_id,
            savings_id: record.savings_account_id,
            product_id: record.product_id,
            creditbureau_id: record.creditbureau_id,
            organisation_creditbureau_id: record.organisation_creditbureau_id,
        }
    }

    pub fn command_id(&self) -> Option<i64> {
        self.command_id
    }

    pub fn action_name(&self) -> &str {
        &self.action_name
    }

    pub fn entity_name(&self) -> &str {
        &self.entity_name
    }

    pub fn resource_id(&self) -> Option<i64> {
        self.entity_id
    }

    pub fn subresource_id(&self) -> Option<i64> {
        self.subentity_id
    }

    /// Returns `{action_name}_{entity_name}` matching `CommandWrapper.taskPermissionName()`.
    pub fn task_permission_name(&self) -> String {
        format!("{}_{}", self.action_name, self.entity_name)
    }

    /// Returns `{action_name}_{entity_name}` matching `CommandWrapper.commandName()`.
    pub fn command_name(&self) -> String {
        format!("{}_{}", self.action_name, self.entity_name)
    }

    /// Returns `{action_name} {entity_name}` matching `CommandWrapper.operationName()`.
    pub fn operation_name(&self) -> String {
        format!("{} {}", self.action_name, self.entity_name)
    }

    pub fn is_create(&self) -> bool {
        self.action_name.eq_ignore_ascii_case("CREATE")
    }

    pub fn is_update(&self) -> bool {
        self.action_name.eq_ignore_ascii_case("UPDATE")
    }

    pub fn is_delete(&self) -> bool {
        self.action_name.eq_ignore_ascii_case("DELETE")
    }

    pub fn is_permission_resource(&self) -> bool {
        self.entity_name.eq_ignore_ascii_case("PERMISSION")
    }

    pub fn is_user_resource(&self) -> bool {
        self.entity_name.eq_ignore_ascii_case("USER")
    }

    pub fn is_currency_resource(&self) -> bool {
        self.entity_name.eq_ignore_ascii_case("CURRENCY")
    }

    pub fn is_cache_resource(&self) -> bool {
        self.entity_name.eq_ignore_ascii_case("CACHE")
    }

    pub fn is_working_days_resource(&self) -> bool {
        self.entity_name.eq_ignore_ascii_case("WORKINGDAYS")
    }

    pub fn is_loan_disburse_detail_resource(&self) -> bool {
        self.entity_name.eq_ignore_ascii_case("DISBURSEMENTDETAIL")
    }
}

/// Fluent builder for [`CommandWrapper`], ported from `org.apache.fineract.commands.service.CommandWrapperBuilder`.
#[derive(Debug, Clone, Default)]
pub struct CommandWrapperBuilder {
    wrapper: CommandWrapper,
}

impl CommandWrapperBuilder {
    pub fn new() -> Self {
        Self {
            wrapper: CommandWrapper {
                json: "{}".to_string(),
                ..CommandWrapper::default()
            },
        }
    }

    pub fn with_command_id(mut self, command_id: i64) -> Self {
        self.wrapper.command_id = Some(command_id);
        self
    }

    pub fn with_action_name(mut self, action_name: impl Into<String>) -> Self {
        self.wrapper.action_name = action_name.into();
        self
    }

    pub fn with_entity_name(mut self, entity_name: impl Into<String>) -> Self {
        self.wrapper.entity_name = entity_name.into();
        self
    }

    pub fn with_entity_id(mut self, entity_id: i64) -> Self {
        self.wrapper.entity_id = Some(entity_id);
        self
    }

    pub fn with_subentity_id(mut self, subentity_id: i64) -> Self {
        self.wrapper.subentity_id = Some(subentity_id);
        self
    }

    pub fn with_href(mut self, href: impl Into<String>) -> Self {
        self.wrapper.href = Some(href.into());
        self
    }

    pub fn with_json(mut self, json: impl Into<String>) -> Self {
        self.wrapper.json = json.into();
        self
    }

    pub fn with_no_json(mut self) -> Self {
        self.wrapper.json = "{}".to_string();
        self
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.wrapper.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn with_office_id(mut self, office_id: i64) -> Self {
        self.wrapper.office_id = Some(office_id);
        self
    }

    pub fn with_group_id(mut self, group_id: i64) -> Self {
        self.wrapper.group_id = Some(group_id);
        self
    }

    pub fn with_client_id(mut self, client_id: i64) -> Self {
        self.wrapper.client_id = Some(client_id);
        self
    }

    pub fn with_loan_id(mut self, loan_id: i64) -> Self {
        self.wrapper.loan_id = Some(loan_id);
        self
    }

    pub fn with_savings_id(mut self, savings_id: i64) -> Self {
        self.wrapper.savings_id = Some(savings_id);
        self
    }

    pub fn with_product_id(mut self, product_id: i64) -> Self {
        self.wrapper.product_id = Some(product_id);
        self
    }

    pub fn with_creditbureau_id(mut self, creditbureau_id: i64) -> Self {
        self.wrapper.creditbureau_id = Some(creditbureau_id);
        self
    }

    pub fn with_organisation_creditbureau_id(mut self, org_cb_id: i64) -> Self {
        self.wrapper.organisation_creditbureau_id = Some(org_cb_id);
        self
    }

    pub fn create_client(mut self) -> Self {
        self.wrapper.action_name = "CREATE".to_string();
        self.wrapper.entity_name = "CLIENT".to_string();
        self.wrapper.href = Some("/clients/template".to_string());
        self
    }

    pub fn activate_client(mut self, client_id: i64) -> Self {
        self.wrapper.action_name = "ACTIVATE".to_string();
        self.wrapper.entity_name = "CLIENT".to_string();
        self.wrapper.entity_id = Some(client_id);
        self.wrapper.client_id = Some(client_id);
        self.wrapper.href = Some(format!("/clients/{client_id}"));
        self
    }

    pub fn create_loan(mut self) -> Self {
        self.wrapper.action_name = "CREATE".to_string();
        self.wrapper.entity_name = "LOAN".to_string();
        self.wrapper.href = Some("/loans/template".to_string());
        self
    }

    pub fn approve_loan(mut self, loan_id: i64) -> Self {
        self.wrapper.action_name = "APPROVE".to_string();
        self.wrapper.entity_name = "LOAN".to_string();
        self.wrapper.entity_id = Some(loan_id);
        self.wrapper.loan_id = Some(loan_id);
        self.wrapper.href = Some(format!("/loans/{loan_id}"));
        self
    }

    pub fn disburse_loan(mut self, loan_id: i64) -> Self {
        self.wrapper.action_name = "DISBURSE".to_string();
        self.wrapper.entity_name = "LOAN".to_string();
        self.wrapper.entity_id = Some(loan_id);
        self.wrapper.loan_id = Some(loan_id);
        self.wrapper.href = Some(format!("/loans/{loan_id}"));
        self
    }

    pub fn create_savings_account(mut self) -> Self {
        self.wrapper.action_name = "CREATE".to_string();
        self.wrapper.entity_name = "SAVINGSACCOUNT".to_string();
        self.wrapper.href = Some("/savingsaccounts/template".to_string());
        self
    }

    pub fn build(self) -> CommandWrapper {
        self.wrapper
    }
}

/// Wire-compatible response envelope returned by all Fineract write commands.
///
/// Ported from `org.apache.fineract.infrastructure.core.data.CommandProcessingResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandProcessingResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub office_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loan_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub savings_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_resource_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_external_id: Option<ExternalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_resource_external_id: Option<ExternalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_transaction: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_identifier: Option<String>,
}

impl CommandProcessingResult {
    /// Creates an empty [`CommandProcessingResult`] matching `CommandProcessingResult.empty()`.
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_resource_id(id: i64) -> Self {
        Self {
            resource_id: Some(id),
            ..Self::default()
        }
    }

    /// Creates a result containing only `command_id` matching `CommandProcessingResult.commandOnlyResult(commandId)`.
    pub fn command_only_result(command_id: i64) -> Self {
        Self {
            command_id: Some(command_id),
            ..Self::default()
        }
    }

    /// Creates a result for `resource_id` with optional `command_id` and `changes`,
    /// matching `CommandProcessingResult.resourceResult(...)`.
    pub fn resource_result(
        resource_id: i64,
        command_id: Option<i64>,
        changes: Option<BTreeMap<String, serde_json::Value>>,
    ) -> Self {
        let normalized_changes = changes.filter(|c| !c.is_empty());
        Self {
            command_id,
            resource_id: Some(resource_id),
            changes: normalized_changes,
            ..Self::default()
        }
    }

    /// Creates a result with `resource_id` and `changes`, matching `CommandProcessingResult.withChanges(...)`.
    pub fn with_changes(resource_id: i64, changes: BTreeMap<String, serde_json::Value>) -> Self {
        Self::resource_result(resource_id, None, Some(changes))
    }

    /// Clones `command` while overriding `resource_id` when needed, matching
    /// `CommandProcessingResult.fromCommandProcessingResult(command, resourceId)`.
    pub fn from_command_processing_result(
        mut command: CommandProcessingResult,
        resource_id: Option<i64>,
    ) -> Self {
        if resource_id.is_some() {
            command.resource_id = resource_id;
        }
        command
    }

    /// Returns `true` if `self.changes` contains at least one entry, matching `CommandProcessingResult.hasChanges()`.
    pub fn has_changes(&self) -> bool {
        self.changes.as_ref().is_some_and(|c| !c.is_empty())
    }

    /// Returns `true` if `rollback_transaction` is `Some(true)`, matching `CommandProcessingResult.isRollbackTransaction()`.
    pub fn is_rollback_transaction(&self) -> bool {
        self.rollback_transaction.unwrap_or(false)
    }

    /// Updates `rollback_transaction`.
    pub fn set_rollback_transaction(&mut self, rollback_transaction: Option<bool>) {
        self.rollback_transaction = rollback_transaction;
    }

    /// Updates `office_id`.
    pub fn set_office_id(&mut self, office_id: Option<i64>) {
        self.office_id = office_id;
    }

    fn normalize_external_id(ext_id: ExternalId) -> Option<ExternalId> {
        if ext_id.is_empty() {
            None
        } else {
            Some(ext_id)
        }
    }
}

/// Fluent builder for [`CommandProcessingResult`], ported from
/// `org.apache.fineract.infrastructure.core.data.CommandProcessingResultBuilder`.
#[derive(Debug, Clone, Default)]
pub struct CommandProcessingResultBuilder {
    command_id: Option<i64>,
    office_id: Option<i64>,
    group_id: Option<i64>,
    client_id: Option<i64>,
    loan_id: Option<i64>,
    savings_id: Option<i64>,
    resource_id: Option<i64>,
    sub_resource_id: Option<i64>,
    transaction_id: Option<String>,
    changes: Option<BTreeMap<String, serde_json::Value>>,
    resource_external_id: Option<ExternalId>,
    sub_resource_external_id: Option<ExternalId>,
    rollback_transaction: bool,
}

impl CommandProcessingResultBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_command_id(mut self, command_id: i64) -> Self {
        self.command_id = Some(command_id);
        self
    }

    pub fn with_command_id_opt(mut self, command_id: Option<i64>) -> Self {
        self.command_id = command_id;
        self
    }

    pub fn with_office_id(mut self, office_id: i64) -> Self {
        self.office_id = Some(office_id);
        self
    }

    pub fn with_office_id_opt(mut self, office_id: Option<i64>) -> Self {
        self.office_id = office_id;
        self
    }

    pub fn with_group_id(mut self, group_id: i64) -> Self {
        self.group_id = Some(group_id);
        self
    }

    pub fn with_group_id_opt(mut self, group_id: Option<i64>) -> Self {
        self.group_id = group_id;
        self
    }

    pub fn with_client_id(mut self, client_id: i64) -> Self {
        self.client_id = Some(client_id);
        self
    }

    pub fn with_client_id_opt(mut self, client_id: Option<i64>) -> Self {
        self.client_id = client_id;
        self
    }

    pub fn with_loan_id(mut self, loan_id: i64) -> Self {
        self.loan_id = Some(loan_id);
        self
    }

    pub fn with_loan_id_opt(mut self, loan_id: Option<i64>) -> Self {
        self.loan_id = loan_id;
        self
    }

    pub fn with_savings_id(mut self, savings_id: i64) -> Self {
        self.savings_id = Some(savings_id);
        self
    }

    pub fn with_savings_id_opt(mut self, savings_id: Option<i64>) -> Self {
        self.savings_id = savings_id;
        self
    }

        #[inline]
    pub fn with_entity_id_opt(self, id: Option<i64>) -> Self {
        self.with_resource_id_opt(id)
    }

    pub fn with_resource_id(mut self, entity_id: i64) -> Self {
        self.resource_id = Some(entity_id);
        self
    }

    /// Alias matching `CommandProcessingResultBuilder.withEntityId(Long)`.
    pub fn with_entity_id(mut self, entity_id: i64) -> Self {
        self.resource_id = Some(entity_id);
        self
    }

    pub fn with_resource_id_opt(mut self, entity_id: Option<i64>) -> Self {
        self.resource_id = entity_id;
        self
    }

    pub fn with_sub_resource_id(mut self, sub_entity_id: i64) -> Self {
        self.sub_resource_id = Some(sub_entity_id);
        self
    }

    /// Alias matching `CommandProcessingResultBuilder.withSubEntityId(Long)`.
    pub fn with_sub_entity_id(mut self, sub_entity_id: i64) -> Self {
        self.sub_resource_id = Some(sub_entity_id);
        self
    }

    pub fn with_sub_resource_id_opt(mut self, sub_entity_id: Option<i64>) -> Self {
        self.sub_resource_id = sub_entity_id;
        self
    }

    pub fn with_transaction_id(mut self, transaction_id: impl Into<String>) -> Self {
        let tx = transaction_id.into();
        if !tx.trim().is_empty() {
            self.transaction_id = Some(tx);
        }
        self
    }

    pub fn with_transaction_id_opt(mut self, transaction_id: Option<String>) -> Self {
        self.transaction_id = transaction_id.filter(|s| !s.trim().is_empty());
        self
    }

    pub fn with_changes(mut self, changes: BTreeMap<String, serde_json::Value>) -> Self {
        if !changes.is_empty() {
            self.changes = Some(changes);
        } else {
            self.changes = None;
        }
        self
    }

    pub fn with_change(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.changes
            .get_or_insert_with(BTreeMap::new)
            .insert(key.into(), value.into());
        self
    }

    pub fn with_resource_external_id(mut self, external_id: impl Into<ExternalId>) -> Self {
        self.resource_external_id =
            CommandProcessingResult::normalize_external_id(external_id.into());
        self
    }

    /// Alias matching `CommandProcessingResultBuilder.withEntityExternalId(ExternalId)`.
    pub fn with_entity_external_id(self, external_id: impl Into<ExternalId>) -> Self {
        self.with_resource_external_id(external_id)
    }

    pub fn with_sub_resource_external_id(mut self, external_id: impl Into<ExternalId>) -> Self {
        self.sub_resource_external_id =
            CommandProcessingResult::normalize_external_id(external_id.into());
        self
    }

    /// Alias matching `CommandProcessingResultBuilder.withSubEntityExternalId(ExternalId)`.
    pub fn with_sub_entity_external_id(self, external_id: impl Into<ExternalId>) -> Self {
        self.with_sub_resource_external_id(external_id)
    }

    /// Sets `rollback_transaction |= rollback_transaction` matching `CommandProcessingResultBuilder.setRollbackTransaction(boolean)`.
    pub fn set_rollback_transaction(mut self, rollback_transaction: bool) -> Self {
        self.rollback_transaction |= rollback_transaction;
        self
    }

    /// Alias for [`CommandProcessingResultBuilder::set_rollback_transaction`].
    pub fn with_rollback_transaction(self, rollback_transaction: bool) -> Self {
        self.set_rollback_transaction(rollback_transaction)
    }

    pub fn build(self) -> CommandProcessingResult {
        CommandProcessingResult {
            command_id: self.command_id,
            office_id: self.office_id,
            group_id: self.group_id,
            client_id: self.client_id,
            loan_id: self.loan_id,
            savings_id: self.savings_id,
            resource_id: self.resource_id,
            sub_resource_id: self.sub_resource_id,
            transaction_id: self.transaction_id,
            changes: self.changes.filter(|c| !c.is_empty()),
            resource_external_id: self.resource_external_id,
            sub_resource_external_id: self.sub_resource_external_id,
            rollback_transaction: if self.rollback_transaction {
                Some(true)
            } else {
                None
            },
            resource_identifier: None,
        }
    }
}

/// Asynchronous domain command handler trait.
///
/// Ported from `org.apache.fineract.commands.handler.NewCommandSourceHandler` and
/// `org.apache.fineract.command.core.CommandHandler`.
#[async_trait]
pub trait CommandHandler: Send + Sync {
    fn action_name(&self) -> &'static str {
        ""
    }

    fn entity_name(&self) -> &'static str {
        ""
    }

    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError>;
}

/// Maker-Checker governance policy determining whether a command requires two-step approval.
///
/// Ported from `SynchronousCommandProcessingService` and `ConfigurationDomainService.isMakerCheckerEnabledForTask`.
#[derive(Debug)]
pub struct MakerCheckerPolicy {
    global_enabled: AtomicBool,
    allow_same_maker_checker: AtomicBool,
    permission_overrides: DashMap<String, bool>,
}

impl Default for MakerCheckerPolicy {
    fn default() -> Self {
        Self::new(false)
    }
}

impl MakerCheckerPolicy {
    /// Creates a new [`MakerCheckerPolicy`].
    ///
    /// When `global_enabled` is `true`, maker-checker is enabled for all commands unless explicitly
    /// disabled via [`MakerCheckerPolicy::disable_permission`]. When `global_enabled` is `false`,
    /// individual commands can still be enabled via [`MakerCheckerPolicy::enable_permission`].
    pub fn new(global_enabled: bool) -> Self {
        Self {
            global_enabled: AtomicBool::new(global_enabled),
            allow_same_maker_checker: AtomicBool::new(false),
            permission_overrides: DashMap::new(),
        }
    }

    /// Enables or disables global Maker-Checker governance.
    pub fn set_global_enabled(&self, enabled: bool) {
        self.global_enabled.store(enabled, Ordering::Release);
    }

    /// Returns whether global Maker-Checker is enabled.
    pub fn is_global_enabled(&self) -> bool {
        self.global_enabled.load(Ordering::Acquire)
    }

    /// Configures whether the same user is allowed to both initiate (maker) and approve (checker)
    /// a command without superuser privileges (`maker-checker-same-user` configuration).
    pub fn set_allow_same_maker_checker(&self, allow: bool) {
        self.allow_same_maker_checker
            .store(allow, Ordering::Release);
    }

    /// Returns whether the same user can act as both maker and checker.
    pub fn allow_same_maker_checker(&self) -> bool {
        self.allow_same_maker_checker.load(Ordering::Acquire)
    }

    /// Enables Maker-Checker requirement for a specific `(action_name, entity_name)` pair.
    pub fn enable_permission(&self, action_name: &str, entity_name: &str) {
        let key = Self::permission_key(action_name, entity_name);
        self.permission_overrides.insert(key, true);
    }

    /// Disables Maker-Checker requirement for a specific `(action_name, entity_name)` pair.
    pub fn disable_permission(&self, action_name: &str, entity_name: &str) {
        let key = Self::permission_key(action_name, entity_name);
        self.permission_overrides.insert(key, false);
    }

    /// Returns `true` if Maker-Checker two-step approval is required for `(action_name, entity_name)`.
    pub fn is_maker_checker_enabled_for(&self, action_name: &str, entity_name: &str) -> bool {
        let key = Self::permission_key(action_name, entity_name);
        if let Some(explicit) = self.permission_overrides.get(&key) {
            return *explicit.value();
        }
        self.global_enabled.load(Ordering::Acquire)
    }

    fn permission_key(action_name: &str, entity_name: &str) -> String {
        format!(
            "{}_{}",
            action_name.trim().to_ascii_uppercase(),
            entity_name.trim().to_ascii_uppercase()
        )
    }
}

/// Outcome returned by [`IdempotencyMiddleware::check_existing`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyCheckOutcome {
    /// No prior command exists for `(action_name, entity_name, idempotency_key)`.
    ProceedNew,
    /// Prior command is in `AWAITING_APPROVAL` and a Checker is now approving it.
    ProceedCheckerApproval(CommandSourceRecord),
    /// Prior command already completed (`PROCESSED` or `AWAITING_APPROVAL` re-submission); replay cached result.
    ReplayResult {
        status_code: i32,
        result: CommandProcessingResult,
    },
    /// Prior command failed (`ERROR`); return the cached HTTP status code and error body.
    PreviousError {
        status_code: i32,
        error_body: String,
    },
}

/// Idempotency resolution and replay middleware ported from `SynchronousCommandProcessingService`,
/// `IdempotencyKeyResolver`, `IdempotencyKeyGenerator`, `IdempotentCommandProcessSucceedException`,
/// `IdempotentCommandProcessUnderProcessingException`, and `IdempotentCommandProcessFailedException`.
#[derive(Debug, Clone, Default)]
pub struct IdempotencyMiddleware {
    header_name: String,
}

impl IdempotencyMiddleware {
    /// Default HTTP header name for Fineract idempotency keys (`Idempotency-Key`).
    pub const DEFAULT_HEADER_NAME: &'static str = "Idempotency-Key";

    pub fn new() -> Self {
        Self {
            header_name: Self::DEFAULT_HEADER_NAME.to_string(),
        }
    }

    pub fn with_header_name(header_name: impl Into<String>) -> Self {
        Self {
            header_name: header_name.into(),
        }
    }

    pub fn header_name(&self) -> &str {
        &self.header_name
    }

    /// Resolves the idempotency key from [`CommandWrapper`], falling back to a generated UUID v4
    /// (matching `IdempotencyKeyResolver` + `IdempotencyKeyGenerator`).
    pub fn resolve_key(&self, wrapper: &CommandWrapper) -> String {
        wrapper
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    }

    /// Evaluates an existing [`CommandSourceRecord`] for idempotent replay, conflict (`425 Too Early`),
    /// or cached error status reproduction.
    pub fn evaluate_record(
        &self,
        record: CommandSourceRecord,
        approved_by_checker: bool,
    ) -> Result<IdempotencyCheckOutcome, FineractError> {
        match record.status_type() {
            CommandProcessingResultType::UnderProcessing => Err(FineractError::IdempotencyConflict),
            CommandProcessingResultType::Processed => {
                let parsed = record
                    .result
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<CommandProcessingResult>(raw).ok())
                    .unwrap_or_else(|| Self::fallback_result_from_record(&record, false));
                Ok(IdempotencyCheckOutcome::ReplayResult {
                    status_code: record.result_status_code.unwrap_or(200),
                    result: parsed,
                })
            }
            CommandProcessingResultType::AwaitingApproval => {
                if approved_by_checker {
                    Ok(IdempotencyCheckOutcome::ProceedCheckerApproval(record))
                } else {
                    let parsed = record
                        .result
                        .as_deref()
                        .and_then(|raw| serde_json::from_str::<CommandProcessingResult>(raw).ok())
                        .unwrap_or_else(|| Self::fallback_result_from_record(&record, true));
                    Ok(IdempotencyCheckOutcome::ReplayResult {
                        status_code: record.result_status_code.unwrap_or(200),
                        result: parsed,
                    })
                }
            }
            CommandProcessingResultType::Error => {
                let status_code = record.result_status_code.unwrap_or(500);
                let error_body = record
                    .result
                    .clone()
                    .unwrap_or_else(|| "Command failed previously".to_string());
                Ok(IdempotencyCheckOutcome::PreviousError {
                    status_code,
                    error_body,
                })
            }
            CommandProcessingResultType::Rejected | CommandProcessingResultType::Invalid => {
                let status_code = record.result_status_code.unwrap_or(422);
                let error_body = record
                    .result
                    .clone()
                    .unwrap_or_else(|| format!("Command is in state {}", record.status_type()));
                Ok(IdempotencyCheckOutcome::PreviousError {
                    status_code,
                    error_body,
                })
            }
        }
    }

    fn fallback_result_from_record(
        record: &CommandSourceRecord,
        rollback_transaction: bool,
    ) -> CommandProcessingResult {
        CommandProcessingResult {
            command_id: Some(record.id),
            office_id: record.office_id,
            group_id: record.group_id,
            client_id: record.client_id,
            loan_id: record.loan_id,
            savings_id: record.savings_account_id,
            resource_id: record.resource_id,
            sub_resource_id: record.subresource_id,
            transaction_id: record.transaction_id.clone(),
            changes: None,
            resource_external_id: record
                .resource_external_id
                .clone()
                .map(ExternalId::from)
                .filter(|e| !e.is_empty()),
            sub_resource_external_id: record
                .subresource_external_id
                .clone()
                .map(ExternalId::from)
                .filter(|e| !e.is_empty()),
            rollback_transaction: if rollback_transaction {
                Some(true)
            } else {
                None
            },
            resource_identifier: None,
        }
    }
}

/// Core CQRS Command Pipeline orchestrating:
/// 1. Idempotency check (`PROCESSED` -> replay cached result; `UNDER_PROCESSING` -> `425 Too Early` /
///    `IdempotencyConflict`; `ERROR` -> return previous HTTP status).
/// 2. Maker-Checker check (`is_maker_checker_enabled_for(action_name, entity_name)` and `!checker_approval`
///    -> save as `AwaitingApproval` and return result with `rollback_transaction = Some(true)`).
/// 3. Handler dispatch & `m_portfolio_command_source` audit update.
pub struct CommandPipeline {
    store: Arc<dyn CommandSourceStore>,
    maker_checker_policy: MakerCheckerPolicy,
    idempotency: IdempotencyMiddleware,
    handlers: DashMap<(String, String), Arc<dyn CommandHandler>>,
}

impl CommandPipeline {
    /// Creates a new [`CommandPipeline`] backed by `store` and `maker_checker_policy`.
    pub fn new(store: Arc<dyn CommandSourceStore>, maker_checker_policy: MakerCheckerPolicy) -> Self {
        Self {
            store,
            maker_checker_policy,
            idempotency: IdempotencyMiddleware::new(),
            handlers: DashMap::new(),
        }
    }

    /// Returns a reference to the underlying [`CommandSourceStore`].
    pub fn store(&self) -> &Arc<dyn CommandSourceStore> {
        &self.store
    }

    /// Returns a reference to the [`MakerCheckerPolicy`].
    pub fn maker_checker_policy(&self) -> &MakerCheckerPolicy {
        &self.maker_checker_policy
    }

    /// Returns a reference to the [`IdempotencyMiddleware`].
    pub fn idempotency(&self) -> &IdempotencyMiddleware {
        &self.idempotency
    }

    /// Registers a [`CommandHandler`] for `(action_name, entity_name)`.
    pub fn register_handler(
        &self,
        action_name: impl AsRef<str>,
        entity_name: impl AsRef<str>,
        handler: Arc<dyn CommandHandler>,
    ) {
        let key = Self::handler_key(action_name.as_ref(), entity_name.as_ref());
        self.handlers.insert(key, handler);
    }

    /// Resolves a registered [`CommandHandler`] for `(action_name, entity_name)`.
    pub fn find_handler(
        &self,
        action_name: &str,
        entity_name: &str,
    ) -> Option<Arc<dyn CommandHandler>> {
        let key = Self::handler_key(action_name, entity_name);
        self.handlers.get(&key).map(|entry| Arc::clone(entry.value()))
    }

    fn handler_key(action_name: &str, entity_name: &str) -> (String, String) {
        (
            action_name.trim().to_ascii_uppercase(),
            entity_name.trim().to_ascii_uppercase(),
        )
    }

    /// Executes a command through the 3-stage CQRS pipeline:
    /// 1. Idempotency check (`PROCESSED` -> replay; `UNDER_PROCESSING` -> `IdempotencyConflict`;
    ///    `ERROR` -> cached HTTP status error).
    /// 2. Maker-Checker check (`is_maker_checker_enabled_for` && `!approved_by_checker` -> persist
    ///    as `AwaitingApproval` and return `CommandProcessingResult` with `rollback_transaction = Some(true)`).
    /// 3. Handler dispatch & audit update in `m_portfolio_command_source`.
    pub async fn execute(
        &self,
        ctx: &ExecutionContext,
        wrapper: &CommandWrapper,
        command: &JsonCommand,
        approved_by_checker: bool,
    ) -> Result<CommandProcessingResult, FineractError> {
        let idempotency_key = self.idempotency.resolve_key(wrapper);
        let mut effective_wrapper = wrapper.clone();
        effective_wrapper.idempotency_key = Some(idempotency_key.clone());

        // Step 1: Lookup existing CommandSourceRecord by command_id (if supplied) or (action, entity, idempotency_key)
        let existing_record = if let Some(cmd_id) = effective_wrapper.command_id {
            match self.store.find_by_id(cmd_id).await? {
                Some(rec) => Some(rec),
                None => {
                    self.store
                        .find_by_idempotency_key(
                            &effective_wrapper.action_name,
                            &effective_wrapper.entity_name,
                            &idempotency_key,
                        )
                        .await?
                }
            }
        } else {
            self.store
                .find_by_idempotency_key(
                    &effective_wrapper.action_name,
                    &effective_wrapper.entity_name,
                    &idempotency_key,
                )
                .await?
        };

        let existing_awaiting_record = if let Some(record) = existing_record {
            match self
                .idempotency
                .evaluate_record(record, approved_by_checker)?
            {
                IdempotencyCheckOutcome::ProceedNew => None,
                IdempotencyCheckOutcome::ProceedCheckerApproval(rec) => Some(rec),
                IdempotencyCheckOutcome::ReplayResult { result, .. } => {
                    return Ok(result);
                }
                IdempotencyCheckOutcome::PreviousError {
                    status_code,
                    error_body,
                } => {
                    return Err(Self::error_from_cached_failure(
                        &effective_wrapper,
                        &idempotency_key,
                        status_code,
                        &error_body,
                    ));
                }
            }
        } else {
            None
        };

        let maker_id = ctx.authenticated_user_id.unwrap_or(1);

        // Step 2: Maker-Checker two-step governance check
        let maker_checker_required = self
            .maker_checker_policy
            .is_maker_checker_enabled_for(&effective_wrapper.action_name, &effective_wrapper.entity_name)
            && !effective_wrapper.is_permission_resource();

        if maker_checker_required && !approved_by_checker {
            let initial_record = CommandSourceRecord::from_wrapper(
                &effective_wrapper,
                maker_id,
                CommandProcessingResultType::AwaitingApproval,
            );
            let saved = self.store.insert_command_source(initial_record).await?;

            let awaiting_result = CommandProcessingResultBuilder::new()
                .with_command_id(saved.id)
                .with_office_id_opt(effective_wrapper.office_id)
                .with_group_id_opt(effective_wrapper.group_id)
                .with_client_id_opt(effective_wrapper.client_id)
                .with_loan_id_opt(effective_wrapper.loan_id)
                .with_savings_id_opt(effective_wrapper.savings_id)
                .with_resource_id_opt(effective_wrapper.entity_id)
                .with_sub_resource_id_opt(effective_wrapper.subentity_id)
                .set_rollback_transaction(true)
                .build();

            let serialized_result = serde_json::to_string(&awaiting_result).ok();
            self.store
                .update_command_result(
                    saved.id,
                    CommandProcessingResultType::AwaitingApproval.as_i16(),
                    Some(200),
                    serialized_result,
                    None,
                    None,
                    awaiting_result.resource_id,
                    awaiting_result.sub_resource_id,
                    None,
                    None,
                    None,
                )
                .await?;

            return Ok(awaiting_result);
        }

        // Step 3: Acquire or transition record to UNDER_PROCESSING and dispatch to CommandHandler
        let active_record = if let Some(awaiting_rec) = existing_awaiting_record {
            // Verify maker != checker invariant unless superuser or policy permits same user
            let checker_id = ctx.authenticated_user_id.unwrap_or(1);
            if awaiting_rec.maker_id == checker_id
                && !ctx.is_super_user()
                && !self.maker_checker_policy.allow_same_maker_checker()
            {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.maker.cannot.be.checker",
                    "Maker user cannot approve their own command as Checker",
                )));
            }
            awaiting_rec
        } else {
            let initial = CommandSourceRecord::from_wrapper(
                &effective_wrapper,
                maker_id,
                CommandProcessingResultType::UnderProcessing,
            );
            self.store.insert_command_source(initial).await?
        };

        let handler = match self.find_handler(
            &effective_wrapper.action_name,
            &effective_wrapper.entity_name,
        ) {
            Some(h) => h,
            None => {
                let err = FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.command.unsupported",
                    format!(
                        "No CommandHandler registered for {}_{}",
                        effective_wrapper.action_name, effective_wrapper.entity_name
                    ),
                ));
                let _ = self
                    .store
                    .update_command_result(
                        active_record.id,
                        CommandProcessingResultType::Error.as_i16(),
                        Some(400),
                        Some(err.to_string()),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;
                return Err(err);
            }
        };

        let mut command_for_handler = command.clone();
        if command_for_handler.command_id.is_none() {
            command_for_handler.command_id = Some(active_record.id);
        }
        if command_for_handler.entity_id.is_none() {
            command_for_handler.entity_id = effective_wrapper.entity_id;
        }
        if command_for_handler.subresource_id.is_none() {
            command_for_handler.subresource_id = effective_wrapper.subentity_id;
        }
        if command_for_handler.client_id.is_none() {
            command_for_handler.client_id = effective_wrapper.client_id;
        }
        if command_for_handler.loan_id.is_none() {
            command_for_handler.loan_id = effective_wrapper.loan_id;
        }
        if command_for_handler.savings_id.is_none() {
            command_for_handler.savings_id = effective_wrapper.savings_id;
        }

        let handler_outcome = with_execution_context(ctx.clone(), async {
            handler.handle(ctx, &command_for_handler).await
        })
        .await;

        match handler_outcome {
            Ok(mut result) => {
                result.command_id = Some(active_record.id);
                result.rollback_transaction = None;

                let serialized_result = serde_json::to_string(&result).ok();
                let checker_id = if approved_by_checker {
                    ctx.authenticated_user_id
                } else {
                    None
                };
                let checked_on = if approved_by_checker {
                    Some(Utc::now())
                } else {
                    None
                };
                let resource_ext = result
                    .resource_external_id
                    .as_ref()
                    .and_then(|e| e.value().map(str::to_owned));
                let subresource_ext = result
                    .sub_resource_external_id
                    .as_ref()
                    .and_then(|e| e.value().map(str::to_owned));

                self.store
                    .update_command_result(
                        active_record.id,
                        CommandProcessingResultType::Processed.as_i16(),
                        Some(200),
                        serialized_result,
                        checker_id,
                        checked_on,
                        result.resource_id,
                        result.sub_resource_id,
                        result.transaction_id.clone(),
                        resource_ext,
                        subresource_ext,
                    )
                    .await?;

                Ok(result)
            }
            Err(err) => {
                let status_code = Self::http_status_for_error(&err);
                let error_payload = serde_json::json!({
                    "httpStatusCode": status_code,
                    "defaultUserMessage": err.to_string(),
                })
                .to_string();

                let _ = self
                    .store
                    .update_command_result(
                        active_record.id,
                        CommandProcessingResultType::Error.as_i16(),
                        Some(status_code),
                        Some(error_payload),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;

                Err(err)
            }
        }
    }

    /// Checker approval entry point for a command currently in `AWAITING_APPROVAL` (`status = 2`),
    /// ported from `PortfolioCommandSourceWritePlatformServiceImpl.approveEntry(Long makerCheckerId)`.
    pub async fn approve_command(
        &self,
        checker_ctx: &ExecutionContext,
        command_id: i64,
    ) -> Result<CommandProcessingResult, FineractError> {
        let record = self
            .store
            .find_by_id(command_id)
            .await?
            .ok_or_else(|| FineractError::ResourceNotFound {
                globalisation_code: "error.msg.command.id.invalid".to_string(),
                default_message: format!("Identifier {command_id} not found"),
                resource_id: command_id.to_string(),
            })?;

        if !record.is_awaiting_approval() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.command.awaiting.approval.does.not.exist",
                format!(
                    "Maker-checker command with identifier {command_id} is not awaiting approval"
                ),
            )));
        }

        let wrapper = CommandWrapper::from_existing_record(&record);
        let mut cmd = JsonCommand::from_raw(record.command_as_json.clone(), record.resource_id);
        cmd.command_id = Some(record.id);
        cmd.subresource_id = record.subresource_id;
        cmd.client_id = record.client_id;
        cmd.loan_id = record.loan_id;
        cmd.savings_id = record.savings_account_id;

        self.execute(checker_ctx, &wrapper, &cmd, true).await
    }

    /// Checker rejection entry point for a command currently in `AWAITING_APPROVAL` (`status = 2`),
    /// ported from `PortfolioCommandSourceWritePlatformServiceImpl.rejectEntry(Long makerCheckerId)`.
    pub async fn reject_command(
        &self,
        checker_ctx: &ExecutionContext,
        command_id: i64,
    ) -> Result<CommandProcessingResult, FineractError> {
        let record = self
            .store
            .find_by_id(command_id)
            .await?
            .ok_or_else(|| FineractError::ResourceNotFound {
                globalisation_code: "error.msg.command.id.invalid".to_string(),
                default_message: format!("Identifier {command_id} not found"),
                resource_id: command_id.to_string(),
            })?;

        if !record.is_awaiting_approval() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.command.awaiting.approval.does.not.exist",
                format!(
                    "Maker-checker command with identifier {command_id} is not awaiting approval"
                ),
            )));
        }

        let checker_id = checker_ctx.authenticated_user_id.unwrap_or(1);
        if record.maker_id == checker_id
            && !checker_ctx.is_super_user()
            && !self.maker_checker_policy.allow_same_maker_checker()
        {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.maker.cannot.be.checker",
                "Maker user cannot reject their own command as Checker",
            )));
        }

        let result = CommandProcessingResult::command_only_result(command_id);
        let serialized = serde_json::to_string(&result).ok();

        self.store
            .update_command_result(
                command_id,
                CommandProcessingResultType::Rejected.as_i16(),
                Some(200),
                serialized,
                Some(checker_id),
                Some(Utc::now()),
                record.resource_id,
                record.subresource_id,
                record.transaction_id,
                record.resource_external_id,
                record.subresource_external_id,
            )
            .await?;

        Ok(result)
    }

    /// Maps a [`FineractError`] to its corresponding HTTP status code for `m_portfolio_command_source.result_status_code`.
    pub const fn http_status_for_error(err: &FineractError) -> i32 {
        match err {
            FineractError::DataValidation(_) => 400,
            FineractError::Unauthenticated => 401,
            FineractError::AccessDenied => 403,
            FineractError::ResourceNotFound { .. } => 404,
            FineractError::DomainRule(_) => 422,
            FineractError::IdempotencyConflict => 425,
            FineractError::DatabaseError(_) => 500,
            FineractError::Conflict(_) => 409,
            FineractError::Internal(_) => 500,
        }
    }

    fn error_from_cached_failure(
        wrapper: &CommandWrapper,
        idempotency_key: &str,
        status_code: i32,
        error_body: &str,
    ) -> FineractError {
        FineractError::DomainRule(
            PlatformDomainRuleException::new(
                "error.msg.idempotent.command.process.failed",
                format!(
                    "Idempotent command ({}_{}, key={}) previously failed with HTTP {}: {}",
                    wrapper.action_name,
                    wrapper.entity_name,
                    idempotency_key,
                    status_code,
                    error_body
                ),
            )
            .with_arg(status_code)
            .with_arg(idempotency_key.to_string())
            .with_arg(error_body.to_string()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store_and_exec::InMemoryCommandSourceStore;
    use chrono::NaiveDate;
    use std::sync::atomic::AtomicUsize;

    struct CountingApproveLoanHandler {
        invocations: AtomicUsize,
        should_fail: AtomicBool,
    }

    impl CountingApproveLoanHandler {
        fn new() -> Self {
            Self {
                invocations: AtomicUsize::new(0),
                should_fail: AtomicBool::new(false),
            }
        }

        fn calls(&self) -> usize {
            self.invocations.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl CommandHandler for CountingApproveLoanHandler {
        async fn handle(
            &self,
            _ctx: &ExecutionContext,
            command: &JsonCommand,
        ) -> Result<CommandProcessingResult, FineractError> {
            self.invocations.fetch_add(1, Ordering::SeqCst);
            if self.should_fail.load(Ordering::SeqCst) {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::new(
                        "error.msg.loan.approve.invalid.state",
                        "Loan is not in submitted and pending approval state",
                    ),
                ));
            }

            let loan_id = command.loan_id.or(command.entity_id).unwrap_or(501);
            Ok(CommandProcessingResultBuilder::new()
                .with_command_id_opt(command.command_id)
                .with_office_id(1)
                .with_client_id(command.client_id.unwrap_or(11))
                .with_loan_id(loan_id)
                .with_resource_id(loan_id)
                .with_resource_external_id(ExternalId::new("loan-ext-501"))
                .with_transaction_id("TX-APPROVE-501")
                .with_change("status", serde_json::json!({"id": 200, "code": "loanStatusType.approved"}))
                .build())
        }
    }

    #[tokio::test]
    async fn test_idempotency_replay_and_previous_error_status() {
        let store = Arc::new(InMemoryCommandSourceStore::new());
        let policy = MakerCheckerPolicy::new(false);
        let pipeline = CommandPipeline::new(store.clone(), policy);

        let handler = Arc::new(CountingApproveLoanHandler::new());
        pipeline.register_handler("APPROVE", "LOAN", handler.clone());

        let ctx = ExecutionContext::new(
            "default",
            NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
        );
        let wrapper = CommandWrapperBuilder::new()
            .approve_loan(501)
            .with_office_id(1)
            .with_client_id(11)
            .with_idempotency_key("idem-approve-501")
            .with_json(r#"{"approvedOnDate":"30 October 2026","locale":"en","dateFormat":"dd MMMM yyyy"}"#)
            .build();
        let json_cmd = JsonCommand::from_raw(wrapper.json.clone(), Some(501));

        // First execution invokes the handler once and persists PROCESSED (1)
        let first = pipeline
            .execute(&ctx, &wrapper, &json_cmd, false)
            .await
            .expect("first command execution must succeed");
        assert_eq!(handler.calls(), 1);
        assert_eq!(first.loan_id, Some(501));
        assert_eq!(first.resource_id, Some(501));
        assert_eq!(
            first.resource_external_id.as_ref().and_then(|e| e.value()),
            Some("loan-ext-501")
        );

        // Second execution with identical (action_name, entity_name, idempotency_key) must replay cached result without re-running handler
        let replayed = pipeline
            .execute(&ctx, &wrapper, &json_cmd, false)
            .await
            .expect("second idempotent execution must replay cached result");
        assert_eq!(handler.calls(), 1, "handler must not be invoked on idempotent replay");
        assert_eq!(replayed, first);

        // Verify previous ERROR status is persisted and re-surfaced on retry with same idempotency key
        handler.should_fail.store(true, Ordering::SeqCst);
        let failing_wrapper = CommandWrapperBuilder::new()
            .approve_loan(502)
            .with_idempotency_key("idem-approve-fail-502")
            .build();
        let failing_cmd = JsonCommand::from_raw("{}".to_string(), Some(502));

        let err1 = pipeline
            .execute(&ctx, &failing_wrapper, &failing_cmd, false)
            .await
            .expect_err("handler failure must return Err");
        assert!(matches!(err1, FineractError::DomainRule(_)));
        assert_eq!(handler.calls(), 2);

        let stored_err_rec = store
            .find_by_idempotency_key("APPROVE", "LOAN", "idem-approve-fail-502")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            stored_err_rec.status_type(),
            CommandProcessingResultType::Error
        );
        assert_eq!(stored_err_rec.result_status_code, Some(422));

        // Retry with same failing idempotency key must not re-invoke handler and must report the cached 422 failure
        let err2 = pipeline
            .execute(&ctx, &failing_wrapper, &failing_cmd, false)
            .await
            .expect_err("retry of failed idempotent command must return cached error");
        assert_eq!(handler.calls(), 2, "failed idempotent command must not re-run handler");
        match err2 {
            FineractError::DomainRule(ex) => {
                assert_eq!(
                    ex.globalisation_message_code,
                    "error.msg.idempotent.command.process.failed"
                );
                assert!(ex.default_user_message.contains("HTTP 422"));
            }
            other => panic!("Expected DomainRule for cached failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_in_flight_under_processing_conflict_detection() {
        let store = Arc::new(InMemoryCommandSourceStore::new());
        let policy = MakerCheckerPolicy::new(false);
        let pipeline = CommandPipeline::new(store.clone(), policy);

        let handler = Arc::new(CountingApproveLoanHandler::new());
        pipeline.register_handler("APPROVE", "LOAN", handler.clone());

        let wrapper = CommandWrapperBuilder::new()
            .approve_loan(777)
            .with_idempotency_key("in-flight-key-777")
            .build();

        // Simulate an in-flight command in UNDER_PROCESSING (status = 4)
        let in_flight_record = CommandSourceRecord::from_wrapper(
            &wrapper,
            1,
            CommandProcessingResultType::UnderProcessing,
        );
        store
            .insert_command_source(in_flight_record)
            .await
            .expect("insert in-flight record");

        let ctx = ExecutionContext::default();
        let json_cmd = JsonCommand::from_raw("{}".to_string(), Some(777));

        let err = pipeline
            .execute(&ctx, &wrapper, &json_cmd, false)
            .await
            .expect_err("concurrent command with UNDER_PROCESSING record must fail with 425 IdempotencyConflict");
        assert!(matches!(err, FineractError::IdempotencyConflict));
        assert_eq!(
            CommandPipeline::http_status_for_error(&err),
            425,
            "IdempotencyConflict maps to HTTP 425 Too Early"
        );
        assert_eq!(handler.calls(), 0);
    }

    #[tokio::test]
    async fn test_maker_checker_two_step_approval_and_rejection() {
        let store = Arc::new(InMemoryCommandSourceStore::new());
        let policy = MakerCheckerPolicy::new(false);
        policy.enable_permission("APPROVE", "LOAN");
        assert!(policy.is_maker_checker_enabled_for("APPROVE", "LOAN"));
        assert!(!policy.is_maker_checker_enabled_for("CREATE", "CLIENT"));

        let pipeline = CommandPipeline::new(store.clone(), policy);
        let handler = Arc::new(CountingApproveLoanHandler::new());
        pipeline.register_handler("APPROVE", "LOAN", handler.clone());

        // Step 1: Maker (user_id = 10, non-superuser) submits APPROVE_LOAN command
        let mut maker_ctx = ExecutionContext::default();
        maker_ctx.authenticated_user_id = Some(10);
        maker_ctx.is_super_user = false;

        let wrapper = CommandWrapperBuilder::new()
            .approve_loan(900)
            .with_office_id(1)
            .with_client_id(55)
            .with_idempotency_key("mc-approve-900")
            .with_json(r#"{"approvedOnDate":"2026-10-30"}"#)
            .build();
        let json_cmd = JsonCommand::from_raw(wrapper.json.clone(), Some(900));

        let maker_res = pipeline
            .execute(&maker_ctx, &wrapper, &json_cmd, false)
            .await
            .expect("maker submission must succeed as AwaitingApproval");

        // Handler must NOT have executed yet; result must have rollback_transaction = Some(true)
        assert_eq!(handler.calls(), 0);
        assert_eq!(maker_res.rollback_transaction, Some(true));
        assert!(maker_res.is_rollback_transaction());
        let cmd_id = maker_res.command_id.expect("command_id must be assigned");

        let stored = store.find_by_id(cmd_id).await.unwrap().unwrap();
        assert_eq!(
            stored.status_type(),
            CommandProcessingResultType::AwaitingApproval
        );
        assert_eq!(stored.maker_id, 10);
        assert_eq!(stored.checker_id, None);

        // Maker attempting to approve their own command without superuser must be rejected
        let self_approve_err = pipeline
            .approve_command(&maker_ctx, cmd_id)
            .await
            .expect_err("maker cannot approve own command when allow_same_maker_checker is false");
        assert!(matches!(self_approve_err, FineractError::DomainRule(_)));
        assert_eq!(handler.calls(), 0);

        // Step 2: Checker (user_id = 20) approves the pending command
        let mut checker_ctx = ExecutionContext::default();
        checker_ctx.authenticated_user_id = Some(20);
        checker_ctx.is_super_user = false;

        let approved_res = pipeline
            .approve_command(&checker_ctx, cmd_id)
            .await
            .expect("checker approval must execute handler and succeed");

        assert_eq!(handler.calls(), 1);
        assert_eq!(approved_res.command_id, Some(cmd_id));
        assert_eq!(approved_res.loan_id, Some(900));
        assert_eq!(approved_res.resource_id, Some(900));
        assert_eq!(approved_res.rollback_transaction, None);

        let final_record = store.find_by_id(cmd_id).await.unwrap().unwrap();
        assert_eq!(
            final_record.status_type(),
            CommandProcessingResultType::Processed
        );
        assert_eq!(final_record.maker_id, 10);
        assert_eq!(final_record.checker_id, Some(20));
        assert!(final_record.checked_on_date.is_some());
    }

    #[test]
    fn test_command_processing_result_json_serialization_parity() {
        // 1. Minimal resource result omits all None fields (SkipSerializingIf::is_none)
        let minimal = CommandProcessingResultBuilder::new()
            .with_office_id(1)
            .with_client_id(42)
            .with_resource_id(42)
            .build();

        let json_val = serde_json::to_value(&minimal).unwrap();
        assert_eq!(
            json_val,
            serde_json::json!({
                "officeId": 1,
                "clientId": 42,
                "resourceId": 42
            })
        );

        // 2. Full result with ExternalId, changes, transactionId, and rollbackTransaction
        let full = CommandProcessingResultBuilder::new()
            .with_command_id(7)
            .with_office_id(1)
            .with_group_id(2)
            .with_client_id(3)
            .with_loan_id(4)
            .with_savings_id(5)
            .with_resource_id(4)
            .with_sub_resource_id(99)
            .with_transaction_id("L-TX-99")
            .with_resource_external_id(ExternalId::new("ext-loan-4"))
            .with_sub_resource_external_id(ExternalId::new("ext-tx-99"))
            .with_change("locale", "en")
            .set_rollback_transaction(true)
            .build();

        let full_json = serde_json::to_value(&full).unwrap();
        assert_eq!(
            full_json,
            serde_json::json!({
                "commandId": 7,
                "officeId": 1,
                "groupId": 2,
                "clientId": 3,
                "loanId": 4,
                "savingsId": 5,
                "resourceId": 4,
                "subResourceId": 99,
                "transactionId": "L-TX-99",
                "changes": {
                    "locale": "en"
                },
                "resourceExternalId": "ext-loan-4",
                "subResourceExternalId": "ext-tx-99",
                "rollbackTransaction": true
            })
        );

        // Round-trip deserialization parity
        let round_trip: CommandProcessingResult = serde_json::from_value(full_json).unwrap();
        assert_eq!(round_trip, full);

        // 3. CommandEnvelope<P> construction and serialization check
        let env = CommandEnvelope::new(
            "CREATE",
            "LOAN",
            "idem-uuid-1",
            serde_json::json!({"principal": 5000}),
        )
        .with_maker_user_id(10)
        .with_checker_user_id(20);
        assert_eq!(env.permission_code(), "CREATE_LOAN");
        assert_eq!(env.maker_user_id, Some(10));
        assert_eq!(env.checker_user_id, Some(20));
    }
}
