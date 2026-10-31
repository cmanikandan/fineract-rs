//! `fineract-branch-rs` — Branch, Teller, and Cashier Cash Management & Vault Settlement Engine.
//!
//! Ported from:
//! - `org.apache.fineract.organisation.teller.domain.TellerStatus`
//! - `org.apache.fineract.organisation.teller.domain.CashierTxnType`
//! - `org.apache.fineract.organisation.teller.domain.Teller`
//! - `org.apache.fineract.organisation.teller.domain.Cashier`
//! - `org.apache.fineract.organisation.teller.domain.CashierTransaction`
//! - `org.apache.fineract.organisation.teller.data.TellerData`
//! - `org.apache.fineract.organisation.teller.data.CashierData`
//! - `org.apache.fineract.organisation.teller.data.CashierTransactionData`
//! - `org.apache.fineract.organisation.teller.data.CashierTransactionTypeTotalsData`
//! - `org.apache.fineract.organisation.teller.data.CashierTransactionsWithSummaryData`
//! - `org.apache.fineract.organisation.teller.data.CashierTransactionDataValidator`
//! - `org.apache.fineract.organisation.teller.serialization.TellerCommandFromApiJsonDeserializer`
//! - `org.apache.fineract.organisation.teller.util.DateRange`
//! - `org.apache.fineract.organisation.teller.exception.*`
//! - `org.apache.fineract.organisation.teller.handler.*`
//! - `org.apache.fineract.organisation.teller.service.{TellerWritePlatformServiceJpaImpl, TellerManagementReadPlatformServiceImpl}`

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AnyPool, Row};
use thiserror::Error;

use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{
    DialectSql, ExecutionContext, JsonCommand, Page, SearchParameters, SqlDialect,
};
use fineract_money::{CurrencyData, FineractDecimal, FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{
    ApiErrorArg, ApiParameterError, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException, PlatformDomainRuleException,
};

// =============================================================================
// 1. TELLER STATUS & CASHIER TRANSACTION TYPE ENUMS
// =============================================================================

/// Enum representation of teller status states (`m_tellers.state`).
///
/// Ported from `org.apache.fineract.organisation.teller.domain.TellerStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum TellerStatus {
    #[default]
    Invalid = 0,
    Pending = 100,
    Active = 300,
    Inactive = 400,
    Closed = 600,
}

impl TellerStatus {
    /// Java-compatible constant alias for [`TellerStatus::Invalid`].
    pub const INVALID: Self = Self::Invalid;
    /// Java-compatible constant alias for [`TellerStatus::Pending`].
    pub const PENDING: Self = Self::Pending;
    /// Java-compatible constant alias for [`TellerStatus::Active`].
    pub const ACTIVE: Self = Self::Active;
    /// Java-compatible constant alias for [`TellerStatus::Inactive`].
    pub const INACTIVE: Self = Self::Inactive;
    /// Java-compatible constant alias for [`TellerStatus::Closed`].
    pub const CLOSED: Self = Self::Closed;

    /// Returns the integer discriminant matching `TellerStatus.getValue()`.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Returns the `SMALLINT` (`i16`) database discriminant stored in `m_tellers.state`.
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Returns the globalisation message code matching `TellerStatus.getCode()`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "tellerStatusType.invalid",
            Self::Pending => "tellerStatusType.pending",
            Self::Active => "tellerStatusType.active",
            Self::Inactive => "tellerStatusType.inactive",
            Self::Closed => "tellerStatusType.closed",
        }
    }

    /// Converts an `i32` status value into [`TellerStatus`], defaulting unknown values to `Invalid`.
    ///
    /// Ported from `TellerStatus.fromInt(final Integer statusValue)`.
    pub const fn from_i32(status_value: i32) -> Self {
        match status_value {
            100 => Self::Pending,
            300 => Self::Active,
            400 => Self::Inactive,
            600 => Self::Closed,
            _ => Self::Invalid,
        }
    }

    /// Converts an optional `i32` status value into [`TellerStatus`], matching `TellerStatus.fromInt(Integer)`.
    pub const fn from_int(status_value: Option<i32>) -> Self {
        match status_value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    /// Converts an `i16` database discriminant from `m_tellers.state` into [`TellerStatus`].
    pub const fn from_i16(status_value: i16) -> Self {
        Self::from_i32(status_value as i32)
    }

    /// Parses either a numeric string (`"100"`, `"300"`, `"400"`, `"600"`), enum name (`"ACTIVE"`),
    /// or globalisation code (`"tellerStatusType.active"`) into [`TellerStatus`].
    pub fn parse_flexible(raw: &str) -> Self {
        let trimmed = raw.trim();
        if let Ok(num) = trimmed.parse::<i32>() {
            return Self::from_i32(num);
        }
        match trimmed.to_ascii_uppercase().as_str() {
            "PENDING" | "TELLERSTATUSTYPE.PENDING" => Self::Pending,
            "ACTIVE" | "TELLERSTATUSTYPE.ACTIVE" => Self::Active,
            "INACTIVE" | "TELLERSTATUSTYPE.INACTIVE" => Self::Inactive,
            "CLOSED" | "TELLERSTATUSTYPE.CLOSED" => Self::Closed,
            _ => Self::Invalid,
        }
    }

    /// Returns `true` if `self == TellerStatus::Invalid`.
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    /// Returns `true` if `self == TellerStatus::Pending`.
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Returns `true` if `self == TellerStatus::Active`.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Returns `true` if `self == TellerStatus::Inactive`.
    pub const fn is_inactive(self) -> bool {
        matches!(self, Self::Inactive)
    }

    /// Returns `true` if `self == TellerStatus::Closed`.
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

impl fmt::Display for TellerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Cashier transaction type (`m_cashier_transactions.txn_type`).
///
/// Ported from `org.apache.fineract.organisation.teller.domain.CashierTxnType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum CashierTxnType {
    /// Vault-to-cashier cash allocation (`101`, `"Allocate Cash"`).
    Allocate = 101,
    /// Cashier-to-vault cash settlement (`102`, `"Settle Cash"`).
    Settle = 102,
    /// Inward cash transaction from client/loan/savings (`103`, `"Cash In"`).
    CashIn = 103,
    /// Outward cash transaction to client/loan/savings (`104`, `"Cash Out"`).
    CashOut = 104,
}

impl CashierTxnType {
    /// Java-compatible constant alias for `CashierTxnType.ALLOCATE`.
    pub const ALLOCATE: Self = Self::Allocate;
    /// Java-compatible constant alias for `CashierTxnType.SETTLE`.
    pub const SETTLE: Self = Self::Settle;
    /// Java-compatible constant alias for `CashierTxnType.INWARD_CASH_TXN`.
    pub const INWARD_CASH_TXN: Self = Self::CashIn;
    /// Java-compatible constant alias for `CashierTxnType.OUTWARD_CASH_TXN`.
    pub const OUTWARD_CASH_TXN: Self = Self::CashOut;
    /// Alias for [`CashierTxnType::CashIn`].
    pub const CASH_IN: Self = Self::CashIn;
    /// Alias for [`CashierTxnType::CashOut`].
    pub const CASH_OUT: Self = Self::CashOut;

    /// Returns the integer identifier matching `CashierTxnType.getId()`.
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Returns the `SMALLINT` (`i16`) database discriminant stored in `m_cashier_transactions.txn_type`.
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Returns the human-readable description matching `CashierTxnType.getValue()`.
    pub const fn value(self) -> &'static str {
        match self {
            Self::Allocate => "Allocate Cash",
            Self::Settle => "Settle Cash",
            Self::CashIn => "Cash In",
            Self::CashOut => "Cash Out",
        }
    }

    /// Resolves a [`CashierTxnType`] from its integer ID, matching `CashierTxnType.getCashierTxnType(Integer)`.
    pub const fn get_cashier_txn_type(id: i32) -> Option<Self> {
        match id {
            101 => Some(Self::Allocate),
            102 => Some(Self::Settle),
            103 => Some(Self::CashIn),
            104 => Some(Self::CashOut),
            _ => None,
        }
    }

    /// Resolves a [`CashierTxnType`] from an optional integer ID.
    pub const fn from_int(id: Option<i32>) -> Option<Self> {
        match id {
            Some(v) => Self::get_cashier_txn_type(v),
            None => None,
        }
    }

    /// Resolves a [`CashierTxnType`] from an `i16` database value.
    pub const fn from_i16(id: i16) -> Option<Self> {
        Self::get_cashier_txn_type(id as i32)
    }

    /// Returns `true` if this transaction increases the cashier's till balance (`Allocate` or `CashIn`).
    pub const fn increases_till_balance(self) -> bool {
        matches!(self, Self::Allocate | Self::CashIn)
    }

    /// Returns `true` if this transaction decreases the cashier's till balance (`Settle` or `CashOut`).
    pub const fn decreases_till_balance(self) -> bool {
        matches!(self, Self::Settle | Self::CashOut)
    }
}

impl fmt::Display for CashierTxnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.value())
    }
}

// =============================================================================
// 2. DOMAIN EXCEPTIONS & WIRE-COMPATIBLE ERROR CONSTRUCTORS
// =============================================================================

/// Exception thrown when a cashier's date range lies outside the parent teller's active date range.
///
/// Ported from `org.apache.fineract.organisation.teller.exception.CashierDateRangeOutOfTellerDateRangeException`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("Cashier date range should be in teller date range. (cashier.date.range.out.of.teller.date.range.exception)")]
pub struct CashierDateRangeOutOfTellerDateRangeException;

impl CashierDateRangeOutOfTellerDateRangeException {
    pub const CODE: &'static str = "cashier.date.range.out.of.teller.date.range.exception";
    pub const MESSAGE: &'static str = "Cashier date range should be in teller date range.";

    pub const fn new() -> Self {
        Self
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        PlatformDomainRuleException::new(Self::CODE, Self::MESSAGE)
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

impl Default for CashierDateRangeOutOfTellerDateRangeException {
    fn default() -> Self {
        Self::new()
    }
}

impl From<CashierDateRangeOutOfTellerDateRangeException> for PlatformDomainRuleException {
    fn from(ex: CashierDateRangeOutOfTellerDateRangeException) -> Self {
        ex.into_domain_rule_exception()
    }
}

impl From<CashierDateRangeOutOfTellerDateRangeException> for FineractError {
    fn from(ex: CashierDateRangeOutOfTellerDateRangeException) -> Self {
        ex.into_fineract_error()
    }
}

/// Exception thrown when a staff member is already allocated to an overlapping cashier shift.
///
/// Ported from `org.apache.fineract.organisation.teller.exception.CashierAlreadyAllocated`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("Cashier already allocated for given date and time range. (cashier.already.allocated.for.given.data.and.time.exception)")]
pub struct CashierAlreadyAllocated;

impl CashierAlreadyAllocated {
    pub const CODE: &'static str = "cashier.already.allocated.for.given.data.and.time.exception";
    pub const MESSAGE: &'static str = "Cashier already allocated for given date and time range.";

    pub const fn new() -> Self {
        Self
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        PlatformDomainRuleException::new(Self::CODE, Self::MESSAGE)
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

impl Default for CashierAlreadyAllocated {
    fn default() -> Self {
        Self::new()
    }
}

impl From<CashierAlreadyAllocated> for PlatformDomainRuleException {
    fn from(ex: CashierAlreadyAllocated) -> Self {
        ex.into_domain_rule_exception()
    }
}

impl From<CashierAlreadyAllocated> for FineractError {
    fn from(ex: CashierAlreadyAllocated) -> Self {
        ex.into_fineract_error()
    }
}

/// Exception thrown when settling cash or disbursing/paying out more cash than available in the cashier's till.
///
/// Ported from `org.apache.fineract.organisation.teller.exception.CashierInsufficientAmountException`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("Cashier do not have sufficient amount for this transaction. (cashier.insufficient.amount.exception)")]
pub struct CashierInsufficientAmountException;

impl CashierInsufficientAmountException {
    pub const CODE: &'static str = "cashier.insufficient.amount.exception";
    pub const MESSAGE: &'static str =
        "Cashier do not have sufficient amount for this transaction.";

    pub const fn new() -> Self {
        Self
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        PlatformDomainRuleException::new(Self::CODE, Self::MESSAGE)
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

impl Default for CashierInsufficientAmountException {
    fn default() -> Self {
        Self::new()
    }
}

impl From<CashierInsufficientAmountException> for PlatformDomainRuleException {
    fn from(ex: CashierInsufficientAmountException) -> Self {
        ex.into_domain_rule_exception()
    }
}

impl From<CashierInsufficientAmountException> for FineractError {
    fn from(ex: CashierInsufficientAmountException) -> Self {
        ex.into_fineract_error()
    }
}

/// Exception thrown when there is insufficient cash in the till or main vault for a cash settlement/allocation operation.
///
/// Maps to Fineract's `"cashier.insufficient.amount.exception"` domain rule violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("Cashier do not have sufficient amount for this transaction. (cashier.insufficient.amount.exception)")]
pub struct NotEnoughCashInTheMainVault {
    pub requested_amount: Option<Decimal>,
    pub available_amount: Option<Decimal>,
}

impl NotEnoughCashInTheMainVault {
    pub const CODE: &'static str = CashierInsufficientAmountException::CODE;
    pub const MESSAGE: &'static str = CashierInsufficientAmountException::MESSAGE;

    pub const fn new() -> Self {
        Self {
            requested_amount: None,
            available_amount: None,
        }
    }

    pub const fn with_amounts(requested_amount: Decimal, available_amount: Decimal) -> Self {
        Self {
            requested_amount: Some(requested_amount),
            available_amount: Some(available_amount),
        }
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        match (self.requested_amount, self.available_amount) {
            (Some(req), Some(avail)) => PlatformDomainRuleException::with_args(
                Self::CODE,
                Self::MESSAGE,
                vec![ApiErrorArg::from_decimal(req), ApiErrorArg::from_decimal(avail)],
            ),
            _ => PlatformDomainRuleException::new(Self::CODE, Self::MESSAGE),
        }
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

impl Default for NotEnoughCashInTheMainVault {
    fn default() -> Self {
        Self::new()
    }
}

impl From<NotEnoughCashInTheMainVault> for PlatformDomainRuleException {
    fn from(ex: NotEnoughCashInTheMainVault) -> Self {
        ex.into_domain_rule_exception()
    }
}

impl From<NotEnoughCashInTheMainVault> for FineractError {
    fn from(ex: NotEnoughCashInTheMainVault) -> Self {
        ex.into_fineract_error()
    }
}

/// Exception thrown when attempting to delete a teller that still has associated cashiers.
///
/// Ported from `org.apache.fineract.organisation.teller.exception.CashierExistForTellerException`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("Cannot delete teller, Cashier is associated for this teller (error.msg.cashier.is.associated.with.this.teller): {teller_id}")]
pub struct CashierExistForTellerException {
    pub teller_id: i64,
}

impl CashierExistForTellerException {
    pub const CODE: &'static str = "error.msg.cashier.is.associated.with.this.teller";
    pub const MESSAGE: &'static str =
        "Cannot delete teller, Cashier is associated for this teller ";

    pub const fn new(teller_id: i64) -> Self {
        Self { teller_id }
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        PlatformDomainRuleException::with_args(
            Self::CODE,
            Self::MESSAGE,
            vec![ApiErrorArg::from(self.teller_id)],
        )
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

impl From<CashierExistForTellerException> for PlatformDomainRuleException {
    fn from(ex: CashierExistForTellerException) -> Self {
        ex.into_domain_rule_exception()
    }
}

impl From<CashierExistForTellerException> for FineractError {
    fn from(ex: CashierExistForTellerException) -> Self {
        ex.into_fineract_error()
    }
}

/// Exception thrown when a teller cannot be found by ID.
///
/// Ported from `org.apache.fineract.organisation.teller.exception.TellerNotFoundException`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("Teller with identifier {teller_id} not found! (error.msg.teller.not.found)")]
pub struct TellerNotFoundException {
    pub teller_id: i64,
}

impl TellerNotFoundException {
    pub const CODE: &'static str = "error.msg.teller.not.found";

    pub const fn new(teller_id: i64) -> Self {
        Self { teller_id }
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::resource_not_found(
            Self::CODE,
            format!("Teller with identifier {} not found!", self.teller_id),
            self.teller_id.to_string(),
        )
    }
}

impl From<TellerNotFoundException> for FineractError {
    fn from(ex: TellerNotFoundException) -> Self {
        ex.into_fineract_error()
    }
}

/// Exception thrown when a cashier cannot be found by ID.
///
/// Ported from `org.apache.fineract.organisation.teller.exception.CashierNotFoundException`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("Cashier with identifier {cashier_id} not found! (error.msg.cashier.not.found)")]
pub struct CashierNotFoundException {
    pub cashier_id: i64,
}

impl CashierNotFoundException {
    pub const CODE: &'static str = "error.msg.cashier.not.found";

    pub const fn new(cashier_id: i64) -> Self {
        Self { cashier_id }
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::resource_not_found(
            Self::CODE,
            format!("Cashier with identifier {} not found!", self.cashier_id),
            self.cashier_id.to_string(),
        )
    }
}

impl From<CashierNotFoundException> for FineractError {
    fn from(ex: CashierNotFoundException) -> Self {
        ex.into_fineract_error()
    }
}

/// Exception thrown when `endDate` is before `startDate`.
///
/// Ported from `org.apache.fineract.organisation.teller.exception.InvalidDateInputException`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("EndDate {end_date} cannot be before StartDate {start_date} (error.msg.invalid.endDate)")]
pub struct InvalidDateInputException {
    pub start_date: String,
    pub end_date: String,
}

impl InvalidDateInputException {
    pub const CODE: &'static str = "error.msg.invalid.endDate";

    pub fn new(start_date: impl Into<String>, end_date: impl Into<String>) -> Self {
        Self {
            start_date: start_date.into(),
            end_date: end_date.into(),
        }
    }

    pub fn into_fineract_error(self) -> FineractError {
        let msg = format!(
            "EndDate {} cannot be before StartDate {}",
            self.end_date, self.start_date
        );
        FineractError::resource_not_found(Self::CODE, msg, self.end_date)
    }
}

impl From<InvalidDateInputException> for FineractError {
    fn from(ex: InvalidDateInputException) -> Self {
        ex.into_fineract_error()
    }
}

/// Helper to construct the duplicate teller name error (`error.msg.teller.duplicate.name`).
pub fn duplicate_teller_name_error(name: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.teller.duplicate.name",
        format!("Teller with name `{name}` already exists"),
        vec![ApiErrorArg::from("name"), ApiErrorArg::from(name)],
    ))
}

// =============================================================================
// 3. JSON COMMAND DESERIALIZER & VALIDATOR (TellerCommandFromApiJsonDeserializer)
// =============================================================================

/// Deserializer and validator of JSON payloads for the Teller & Cashier API.
///
/// Ported from `org.apache.fineract.organisation.teller.serialization.TellerCommandFromApiJsonDeserializer`.
#[derive(Debug, Clone, Default)]
pub struct TellerCommandFromApiJsonDeserializer;

impl TellerCommandFromApiJsonDeserializer {
    pub const OFFICE_ID: &'static str = "officeId";
    pub const NAME: &'static str = "name";
    pub const DESCRIPTION: &'static str = "description";
    pub const START_DATE: &'static str = "startDate";
    pub const END_DATE: &'static str = "endDate";
    pub const STATUS: &'static str = "status";
    pub const DATE_FORMAT: &'static str = "dateFormat";
    pub const LOCALE: &'static str = "locale";
    pub const IS_FULL_DAY: &'static str = "isFullDay";
    pub const STAFF_ID: &'static str = "staffId";
    pub const ENTITY_TYPE: &'static str = "entityType";
    pub const ENTITY_ID: &'static str = "entityId";
    pub const CURRENCY_CODE: &'static str = "currencyCode";
    pub const HOUR_START_TIME: &'static str = "hourStartTime";
    pub const MIN_START_TIME: &'static str = "minStartTime";
    pub const HOUR_END_TIME: &'static str = "hourEndTime";
    pub const MIN_END_TIME: &'static str = "minEndTime";
    pub const TXN_AMOUNT: &'static str = "txnAmount";
    pub const TXN_DATE: &'static str = "txnDate";
    pub const TXN_NOTE: &'static str = "txnNote";
    pub const TELLER: &'static str = "teller";
    pub const START_TIME: &'static str = "startTime";
    pub const END_TIME: &'static str = "endTime";
    pub const DEBIT_ACCOUNT_ID: &'static str = "debitAccountId";
    pub const CREDIT_ACCOUNT_ID: &'static str = "creditAccountId";

    /// The parameters supported by Teller & Cashier commands.
    pub const SUPPORTED_PARAMETERS: &'static [&'static str] = &[
        Self::OFFICE_ID,
        Self::NAME,
        Self::DESCRIPTION,
        Self::START_DATE,
        Self::END_DATE,
        Self::STATUS,
        Self::DATE_FORMAT,
        Self::LOCALE,
        Self::IS_FULL_DAY,
        Self::STAFF_ID,
        Self::HOUR_START_TIME,
        Self::MIN_START_TIME,
        Self::HOUR_END_TIME,
        Self::MIN_END_TIME,
        Self::START_TIME,
        Self::END_TIME,
        Self::TXN_AMOUNT,
        Self::TXN_DATE,
        Self::TXN_NOTE,
        Self::ENTITY_TYPE,
        Self::ENTITY_ID,
        Self::CURRENCY_CODE,
        Self::DEBIT_ACCOUNT_ID,
        Self::CREDIT_ACCOUNT_ID,
    ];

    pub const fn new() -> Self {
        Self
    }

    /// Validates a `CREATE` or `UPDATE` teller JSON command payload.
    ///
    /// Ported from `TellerCommandFromApiJsonDeserializer.validateForCreateAndUpdateTeller(String)`.
    pub fn validate_for_create_and_update_teller(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(Self::SUPPORTED_PARAMETERS)?;

        let base_validator = DataValidatorBuilder::for_resource(Self::TELLER);

        let office_id = command.long_value_of_parameter_named(Self::OFFICE_ID)?;
        base_validator
            .reset()
            .parameter(Self::OFFICE_ID)
            .value(office_id)
            .not_null()
            .integer_greater_than_zero();

        let name = command.string_value_of_parameter_named(Self::NAME);
        base_validator
            .reset()
            .parameter(Self::NAME)
            .value(name)
            .not_blank()
            .not_exceeding_length_of(50);

        let description = command.string_value_of_parameter_named(Self::DESCRIPTION);
        base_validator
            .reset()
            .parameter(Self::DESCRIPTION)
            .value(description)
            .not_exceeding_length_of(100);

        let start_date = command.local_date_value_of_parameter_named(Self::START_DATE)?;
        base_validator
            .reset()
            .parameter(Self::START_DATE)
            .value(start_date)
            .not_null();

        let end_date = command.local_date_value_of_parameter_named(Self::END_DATE)?;
        base_validator
            .reset()
            .parameter(Self::END_DATE)
            .value(end_date)
            .ignore_if_null();

        let status = command.string_value_of_parameter_named(Self::STATUS);
        base_validator
            .reset()
            .parameter(Self::STATUS)
            .value(status)
            .not_blank()
            .not_exceeding_length_of(50);

        if let (Some(sd), Some(ed)) = (start_date, end_date) {
            if ed < sd {
                return Err(InvalidDateInputException::new(sd.to_string(), ed.to_string()).into());
            }
        }

        base_validator.throw_validation_errors()?;
        Ok(())
    }

    /// Validates an `ALLOCATECASHIER` or `UPDATECASHIERALLOCATION` JSON command payload.
    ///
    /// Ported from `TellerCommandFromApiJsonDeserializer.validateForAllocateCashier(String)`.
    pub fn validate_for_allocate_cashier(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(Self::SUPPORTED_PARAMETERS)?;

        let base_validator = DataValidatorBuilder::for_resource(Self::TELLER);

        let staff_id = command.long_value_of_parameter_named(Self::STAFF_ID)?;
        base_validator
            .reset()
            .parameter(Self::STAFF_ID)
            .value(staff_id)
            .not_null()
            .integer_greater_than_zero();

        let description = command.string_value_of_parameter_named(Self::DESCRIPTION);
        base_validator
            .reset()
            .parameter(Self::DESCRIPTION)
            .value(description)
            .not_exceeding_length_of(100);

        let start_date = command.local_date_value_of_parameter_named(Self::START_DATE)?;
        base_validator
            .reset()
            .parameter(Self::START_DATE)
            .value(start_date)
            .not_null();

        let end_date = command.local_date_value_of_parameter_named(Self::END_DATE)?;
        base_validator
            .reset()
            .parameter(Self::END_DATE)
            .value(end_date)
            .not_null();

        let is_full_day = command.boolean_object_value_of_parameter_named(Self::IS_FULL_DAY);
        base_validator
            .reset()
            .parameter(Self::IS_FULL_DAY)
            .value(is_full_day)
            .not_null();

        if is_full_day == Some(false) {
            let has_explicit_times = command
                .string_value_of_parameter_named(Self::START_TIME)
                .is_some()
                && command
                    .string_value_of_parameter_named(Self::END_TIME)
                    .is_some();

            if !has_explicit_times {
                let hour_start_time =
                    command.string_value_of_parameter_named(Self::HOUR_START_TIME);
                base_validator
                    .reset()
                    .parameter(Self::START_TIME)
                    .value(hour_start_time)
                    .not_blank();

                let min_start_time = command.string_value_of_parameter_named(Self::MIN_START_TIME);
                base_validator
                    .reset()
                    .parameter(Self::START_TIME)
                    .value(min_start_time)
                    .not_blank();

                let hour_end_time = command.string_value_of_parameter_named(Self::HOUR_END_TIME);
                base_validator
                    .reset()
                    .parameter(Self::HOUR_END_TIME)
                    .value(hour_end_time)
                    .not_blank();

                let min_end_time = command.string_value_of_parameter_named(Self::MIN_END_TIME);
                base_validator
                    .reset()
                    .parameter(Self::MIN_END_TIME)
                    .value(min_end_time)
                    .not_blank();
            }
        }

        base_validator.throw_validation_errors()?;
        Ok(())
    }

    /// Validates an `ALLOCATECASHTOCASHIER` or `SETTLECASHFROMCASHIER` JSON command payload.
    ///
    /// Ported from `TellerCommandFromApiJsonDeserializer.validateForCashTxnForCashier(String)`.
    pub fn validate_for_cash_txn_for_cashier(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(Self::SUPPORTED_PARAMETERS)?;

        let base_validator = DataValidatorBuilder::for_resource(Self::TELLER);

        let txn_amount = command.big_decimal_value_of_parameter_named(Self::TXN_AMOUNT)?;
        base_validator
            .reset()
            .parameter(Self::TXN_AMOUNT)
            .value(txn_amount)
            .not_null();

        let txn_date = command.local_date_value_of_parameter_named(Self::TXN_DATE)?;
        base_validator
            .reset()
            .parameter(Self::TXN_DATE)
            .value(txn_date)
            .not_null();

        let txn_note = command.string_value_of_parameter_named(Self::TXN_NOTE);
        base_validator
            .reset()
            .parameter(Self::TXN_NOTE)
            .value(txn_note)
            .not_exceeding_length_of(200);

        let currency_code = command.string_value_of_parameter_named(Self::CURRENCY_CODE);
        base_validator
            .reset()
            .parameter(Self::CURRENCY_CODE)
            .value(currency_code)
            .not_exceeding_length_of(3);

        base_validator.throw_validation_errors()?;
        Ok(())
    }
}

// =============================================================================
// 4. DOMAIN AGGREGATES: TELLER (`m_tellers`), CASHIER (`m_cashiers`),
//    AND CASHIER TRANSACTION (`m_cashier_transactions`)
// =============================================================================

/// Teller aggregate representing a row in the `m_tellers` table.
///
/// Ported from `org.apache.fineract.organisation.teller.domain.Teller`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Teller {
    pub id: i64,
    pub office_id: i64,
    pub debit_account_id: Option<i64>,
    pub credit_account_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub status: TellerStatus,
}

impl Teller {
    /// Creates a new [`Teller`] entity.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        office_id: i64,
        debit_account_id: Option<i64>,
        credit_account_id: Option<i64>,
        name: impl Into<String>,
        description: Option<String>,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
        status: TellerStatus,
    ) -> Self {
        Self {
            id,
            office_id,
            debit_account_id,
            credit_account_id,
            name: name.into(),
            description,
            start_date,
            end_date,
            status,
        }
    }

    /// Constructs a [`Teller`] from a validated [`JsonCommand`] (`Teller.fromJson`).
    pub fn from_json(command: &JsonCommand) -> FineractResult<Self> {
        TellerCommandFromApiJsonDeserializer::validate_for_create_and_update_teller(command)?;

        let office_id = command
            .long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::OFFICE_ID)?
            .unwrap_or_default();
        let debit_account_id =
            command.long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::DEBIT_ACCOUNT_ID)?;
        let credit_account_id = command
            .long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::CREDIT_ACCOUNT_ID)?;
        let name = command
            .string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::NAME)
            .unwrap_or_default();
        let description =
            command.string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::DESCRIPTION);
        let start_date = command
            .local_date_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::START_DATE)?
            .expect("validated startDate is not null");
        let end_date =
            command.local_date_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::END_DATE)?;

        let status = match command
            .integer_value_sans_locale_of_parameter_named(TellerCommandFromApiJsonDeserializer::STATUS)
        {
            Ok(Some(code)) => TellerStatus::from_i32(code),
            _ => command
                .string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::STATUS)
                .as_deref()
                .map(TellerStatus::parse_flexible)
                .unwrap_or(TellerStatus::Invalid),
        };

        Ok(Self {
            id: command.entity_id.unwrap_or(0),
            office_id,
            debit_account_id,
            credit_account_id,
            name,
            description,
            start_date,
            end_date,
            status,
        })
    }

    /// Applies a partial update from [`JsonCommand`] and returns the map of changed attributes (`Teller.update`).
    pub fn update(
        &mut self,
        command: &JsonCommand,
    ) -> FineractResult<BTreeMap<String, Value>> {
        let mut actual_changes = BTreeMap::new();
        let date_format = command.date_format();
        let locale = command.locale();

        if command.is_change_in_long_parameter_named("officeId", Some(self.office_id))? {
            if let Some(new_val) = command.long_value_of_parameter_named("officeId")? {
                actual_changes.insert("officeId".to_string(), Value::from(new_val));
                self.office_id = new_val;
            }
        }

        if command.is_change_in_string_parameter_named("name", Some(&self.name)) {
            if let Some(new_val) = command.string_value_of_parameter_named("name") {
                actual_changes.insert("name".to_string(), Value::String(new_val.clone()));
                self.name = new_val;
            }
        }

        if command.is_change_in_string_parameter_named("description", self.description.as_deref()) {
            let new_val = command.string_value_of_parameter_named("description");
            actual_changes.insert(
                "description".to_string(),
                new_val
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            self.description = new_val;
        }

        if command.is_change_in_local_date_parameter_named("startDate", Some(self.start_date))? {
            let value_as_input = command.string_value_of_parameter_named("startDate");
            if let Some(raw) = value_as_input {
                actual_changes.insert("startDate".to_string(), Value::String(raw));
            }
            if !date_format.is_empty() {
                actual_changes.insert("dateFormat".to_string(), Value::String(date_format.clone()));
            }
            if !locale.is_empty() {
                actual_changes.insert("locale".to_string(), Value::String(locale.clone()));
            }
            if let Some(new_date) = command.local_date_value_of_parameter_named("startDate")? {
                self.start_date = new_date;
            }
        }

        if command.is_change_in_local_date_parameter_named("endDate", self.end_date)? {
            let value_as_input = command.string_value_of_parameter_named("endDate");
            actual_changes.insert(
                "endDate".to_string(),
                value_as_input.map(Value::String).unwrap_or(Value::Null),
            );
            if !date_format.is_empty() {
                actual_changes.insert("dateFormat".to_string(), Value::String(date_format));
            }
            if !locale.is_empty() {
                actual_changes.insert("locale".to_string(), Value::String(locale));
            }
            self.end_date = command.local_date_value_of_parameter_named("endDate")?;
        }

        if let Some(ed) = self.end_date {
            if ed < self.start_date {
                return Err(
                    InvalidDateInputException::new(self.start_date.to_string(), ed.to_string())
                        .into(),
                );
            }
        }

        if command.parameter_exists("status") {
            let new_status = match command.integer_value_sans_locale_of_parameter_named("status") {
                Ok(Some(code)) => TellerStatus::from_i32(code),
                _ => command
                    .string_value_of_parameter_named("status")
                    .as_deref()
                    .map(TellerStatus::parse_flexible)
                    .unwrap_or(TellerStatus::Invalid),
            };
            if new_status != self.status {
                actual_changes.insert("status".to_string(), Value::from(new_status.value()));
                if !new_status.is_invalid() {
                    self.status = new_status;
                }
            }
        }

        Ok(actual_changes)
    }

    /// Returns `true` if the given `[cashier_start, cashier_end]` date range falls inside this teller's
    /// active `[start_date, end_date]` window.
    pub fn contains_date_range(&self, cashier_start: NaiveDate, cashier_end: NaiveDate) -> bool {
        if cashier_start < self.start_date || cashier_end < self.start_date {
            return false;
        }
        if let Some(teller_end) = self.end_date {
            if cashier_start > teller_end || cashier_end > teller_end {
                return false;
            }
        }
        cashier_end >= cashier_start
    }

    /// Validates that a cashier's `[cashier_start, cashier_end]` lies within this teller's date range,
    /// returning [`CashierDateRangeOutOfTellerDateRangeException`] on violation.
    pub fn validate_cashier_date_range(
        &self,
        cashier_start: NaiveDate,
        cashier_end: NaiveDate,
    ) -> FineractResult<()> {
        if cashier_start < self.start_date
            || cashier_end < self.start_date
            || self
                .end_date
                .is_some_and(|teller_end| cashier_start > teller_end || cashier_end > teller_end)
        {
            return Err(CashierDateRangeOutOfTellerDateRangeException::new().into());
        }
        if cashier_end < cashier_start {
            return Err(InvalidDateInputException::new(
                cashier_start.to_string(),
                cashier_end.to_string(),
            )
            .into());
        }
        Ok(())
    }
}

/// Cashier assignment aggregate representing a row in the `m_cashiers` table.
///
/// Ported from `org.apache.fineract.organisation.teller.domain.Cashier`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cashier {
    pub id: i64,
    pub office_id: i64,
    pub staff_id: i64,
    pub teller_id: i64,
    pub description: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub is_full_day: bool,
}

impl Cashier {
    /// Creates a new [`Cashier`] entity.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        office_id: i64,
        staff_id: i64,
        teller_id: i64,
        description: Option<String>,
        start_date: NaiveDate,
        end_date: NaiveDate,
        start_time: Option<String>,
        end_time: Option<String>,
        is_full_day: bool,
    ) -> Self {
        Self {
            id,
            office_id,
            staff_id,
            teller_id,
            description,
            start_date,
            end_date,
            start_time,
            end_time,
            is_full_day,
        }
    }

    /// Formats `(hour, min)` into `"H:MM"` / `"HH:MM"` matching `TellerWritePlatformServiceJpaImpl.allocateCashierToTeller`.
    pub fn format_shift_time(hour: i64, min: i64) -> String {
        if min == 0 {
            format!("{hour}:00")
        } else {
            format!("{hour}:{min}")
        }
    }

    /// Constructs a [`Cashier`] from a [`JsonCommand`] and parent [`Teller`] (`Cashier.fromJson`).
    pub fn from_json(
        office_id: i64,
        teller: &Teller,
        command: &JsonCommand,
    ) -> FineractResult<Self> {
        TellerCommandFromApiJsonDeserializer::validate_for_allocate_cashier(command)?;

        let staff_id = command
            .long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::STAFF_ID)?
            .unwrap_or_default();
        let description =
            command.string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::DESCRIPTION);
        let start_date = command
            .local_date_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::START_DATE)?
            .expect("validated startDate is not null");
        let end_date = command
            .local_date_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::END_DATE)?
            .expect("validated endDate is not null");
        let is_full_day = command
            .boolean_object_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::IS_FULL_DAY)
            .unwrap_or(true);

        let (start_time, end_time) = if is_full_day {
            (Some(" ".to_string()), Some(" ".to_string()))
        } else if let (Some(st), Some(et)) = (
            command.string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::START_TIME),
            command.string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::END_TIME),
        ) {
            (Some(st), Some(et))
        } else {
            let hour_start = command
                .long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::HOUR_START_TIME)?
                .unwrap_or(0);
            let min_start = command
                .long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::MIN_START_TIME)?
                .unwrap_or(0);
            let hour_end = command
                .long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::HOUR_END_TIME)?
                .unwrap_or(0);
            let min_end = command
                .long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::MIN_END_TIME)?
                .unwrap_or(0);
            (
                Some(Self::format_shift_time(hour_start, min_start)),
                Some(Self::format_shift_time(hour_end, min_end)),
            )
        };

        Ok(Self {
            id: command.subresource_id.unwrap_or(0),
            office_id,
            staff_id,
            teller_id: teller.id,
            description,
            start_date,
            end_date,
            start_time,
            end_time,
            is_full_day,
        })
    }

    /// Applies a partial update from [`JsonCommand`] and returns changed fields (`Cashier.update`).
    pub fn update(
        &mut self,
        command: &JsonCommand,
    ) -> FineractResult<BTreeMap<String, Value>> {
        let mut actual_changes = BTreeMap::new();
        let date_format = command.date_format();
        let locale = command.locale();

        if command.is_change_in_long_parameter_named("staffId", Some(self.staff_id))? {
            if let Some(new_staff_id) = command.long_value_of_parameter_named("staffId")? {
                actual_changes.insert("staffId".to_string(), Value::from(new_staff_id));
                self.staff_id = new_staff_id;
            }
        }

        if command.is_change_in_string_parameter_named("description", self.description.as_deref()) {
            let new_val = command.string_value_of_parameter_named("description");
            actual_changes.insert(
                "description".to_string(),
                new_val.clone().map(Value::String).unwrap_or(Value::Null),
            );
            self.description = new_val;
        }

        if command.is_change_in_local_date_parameter_named("startDate", Some(self.start_date))? {
            if let Some(raw) = command.string_value_of_parameter_named("startDate") {
                actual_changes.insert("startDate".to_string(), Value::String(raw));
            }
            if !date_format.is_empty() {
                actual_changes.insert("dateFormat".to_string(), Value::String(date_format.clone()));
            }
            if !locale.is_empty() {
                actual_changes.insert("locale".to_string(), Value::String(locale.clone()));
            }
            if let Some(new_date) = command.local_date_value_of_parameter_named("startDate")? {
                self.start_date = new_date;
            }
        }

        if command.is_change_in_local_date_parameter_named("endDate", Some(self.end_date))? {
            if let Some(raw) = command.string_value_of_parameter_named("endDate") {
                actual_changes.insert("endDate".to_string(), Value::String(raw));
            }
            if !date_format.is_empty() {
                actual_changes.insert("dateFormat".to_string(), Value::String(date_format));
            }
            if !locale.is_empty() {
                actual_changes.insert("locale".to_string(), Value::String(locale));
            }
            if let Some(new_date) = command.local_date_value_of_parameter_named("endDate")? {
                self.end_date = new_date;
            }
        }

        let full_day_flag = command
            .boolean_object_value_of_parameter_named("isFullDay")
            .unwrap_or(self.is_full_day);
        if command.is_change_in_boolean_parameter_named("isFullDay", Some(self.is_full_day)) {
            actual_changes.insert("isFullDay".to_string(), Value::Bool(full_day_flag));
            self.is_full_day = full_day_flag;
        }

        if !full_day_flag {
            let start_changed = command
                .is_change_in_long_parameter_named("hourStartTime", self.hour_from_start_time())?
                || command
                    .is_change_in_long_parameter_named("minStartTime", self.min_from_start_time())?;
            if start_changed {
                let mut new_start_hour = command
                    .string_value_of_parameter_named("hourStartTime")
                    .unwrap_or_default();
                if new_start_hour.eq_ignore_ascii_case("0") {
                    new_start_hour.push('0');
                }
                actual_changes.insert(
                    "hourStartTime".to_string(),
                    Value::String(new_start_hour.clone()),
                );

                let mut new_start_min = command
                    .string_value_of_parameter_named("minStartTime")
                    .unwrap_or_default();
                if new_start_min.eq_ignore_ascii_case("0") {
                    new_start_min.push('0');
                }
                actual_changes.insert(
                    "minStartTime".to_string(),
                    Value::String(new_start_min.clone()),
                );
                self.start_time = Some(format!("{new_start_hour}:{new_start_min}"));
            }

            let end_changed = command
                .is_change_in_long_parameter_named("hourEndTime", self.hour_from_end_time())?
                || command
                    .is_change_in_long_parameter_named("minEndTime", self.min_from_end_time())?;
            if end_changed {
                let mut new_end_hour = command
                    .string_value_of_parameter_named("hourEndTime")
                    .unwrap_or_default();
                if new_end_hour.eq_ignore_ascii_case("0") {
                    new_end_hour.push('0');
                }
                actual_changes.insert(
                    "hourEndTime".to_string(),
                    Value::String(new_end_hour.clone()),
                );

                let mut new_end_min = command
                    .string_value_of_parameter_named("minEndTime")
                    .unwrap_or_default();
                if new_end_min.eq_ignore_ascii_case("0") {
                    new_end_min.push('0');
                }
                actual_changes.insert("minEndTime".to_string(), Value::String(new_end_min.clone()));
                self.end_time = Some(format!("{new_end_hour}:{new_end_min}"));
            }
        }

        Ok(actual_changes)
    }

    /// Extracts the hour component from `start_time` (`Cashier.getHourFromStartTime`).
    pub fn hour_from_start_time(&self) -> Option<i64> {
        let s = self.start_time.as_deref()?.trim();
        if s.is_empty() {
            return None;
        }
        s.split(':').next()?.parse::<i64>().ok()
    }

    /// Extracts the minute component from `start_time` (`Cashier.getMinFromStartTime`).
    pub fn min_from_start_time(&self) -> Option<i64> {
        let s = self.start_time.as_deref()?.trim();
        if s.is_empty() {
            return None;
        }
        s.split(':').nth(1)?.parse::<i64>().ok()
    }

    /// Extracts the hour component from `end_time` (`Cashier.getHourFromEndTime`).
    pub fn hour_from_end_time(&self) -> Option<i64> {
        let s = self.end_time.as_deref()?.trim();
        if s.is_empty() {
            return None;
        }
        s.split(':').next()?.parse::<i64>().ok()
    }

    /// Extracts the minute component from `end_time` (`Cashier.getMinFromEndTime`).
    pub fn min_from_end_time(&self) -> Option<i64> {
        let s = self.end_time.as_deref()?.trim();
        if s.is_empty() {
            return None;
        }
        s.split(':').nth(1)?.parse::<i64>().ok()
    }

    /// Parses `start_time` into a [`NaiveTime`], returning `None` if unreadable (`CashierTransactionDataValidator.startTimeOf`).
    pub fn start_local_time(&self) -> Option<NaiveTime> {
        let h = u32::try_from(self.hour_from_start_time()?).ok()?;
        let m = u32::try_from(self.min_from_start_time()?).ok()?;
        NaiveTime::from_hms_opt(h, m, 0)
    }

    /// Parses `end_time` into a [`NaiveTime`], returning `None` if unreadable (`CashierTransactionDataValidator.endTimeOf`).
    pub fn end_local_time(&self) -> Option<NaiveTime> {
        let h = u32::try_from(self.hour_from_end_time()?).ok()?;
        let m = u32::try_from(self.min_from_end_time()?).ok()?;
        NaiveTime::from_hms_opt(h, m, 0)
    }

    /// Checks whether this cashier's date range overlaps `[from_date, to_date]` inclusive.
    pub fn dates_overlap(&self, from_date: NaiveDate, to_date: NaiveDate) -> bool {
        self.start_date <= to_date && self.end_date >= from_date
    }

    /// Checks whether this part-day cashier's shift hours overlap `[start_time, end_time]` (end times included).
    ///
    /// Unreadable stored times on an existing record count as an overlap (`CashierTransactionDataValidator.hoursOverlap`).
    pub fn hours_overlap(&self, start_time: NaiveTime, end_time: NaiveTime) -> bool {
        let (Some(existing_start), Some(existing_end)) =
            (self.start_local_time(), self.end_local_time())
        else {
            return true;
        };
        start_time <= existing_end && existing_start <= end_time
    }

    /// Returns `true` if this cashier is on duty on `date` at `time` (`CashierTransactionDataValidator.isOnDuty`).
    pub fn is_on_duty(&self, date: NaiveDate, time: NaiveTime) -> bool {
        if !self.dates_overlap(date, date) {
            return false;
        }
        if self.is_full_day {
            return true;
        }
        let (Some(start_time), Some(end_time)) = (self.start_local_time(), self.end_local_time())
        else {
            return false;
        };
        time >= start_time && time <= end_time
    }

    /// Validates this cashier's date range against the parent [`Teller`], validates part-day shift start/end times,
    /// and ensures no overlapping assignment exists for the same staff member.
    ///
    /// Ported from `CashierTransactionDataValidator.validateCashierAllowedDateAndTime(Cashier, Teller)`.
    pub fn validate_allowed_date_and_time(
        &self,
        teller: &Teller,
        existing_staff_cashiers: &[Cashier],
    ) -> FineractResult<()> {
        // 1. Validate cashier date range in range of teller date range
        if self.start_date < teller.start_date
            || self.end_date < teller.start_date
            || teller
                .end_date
                .is_some_and(|te| self.start_date > te || self.end_date > te)
        {
            return Err(CashierDateRangeOutOfTellerDateRangeException::new().into());
        }

        if self.end_date < self.start_date {
            return Err(InvalidDateInputException::new(
                self.start_date.to_string(),
                self.end_date.to_string(),
            )
            .into());
        }

        // 2. Validate part-day shift ends strictly after it starts on the same day
        let start_time = if self.is_full_day {
            None
        } else {
            self.start_local_time()
        };
        let end_time = if self.is_full_day {
            None
        } else {
            self.end_local_time()
        };

        if !self.is_full_day {
            let valid_times =
                matches!((start_time, end_time), (Some(st), Some(et)) if et > st);
            if !valid_times {
                let validator = DataValidatorBuilder::for_resource(
                    TellerCommandFromApiJsonDeserializer::TELLER,
                );
                validator
                    .parameter(TellerCommandFromApiJsonDeserializer::HOUR_END_TIME)
                    .value(self.end_time.clone())
                    .fail_with_code(
                        "must.be.after.start.time",
                        "The end time of a part-day cashier must be after its start time.",
                    );
                validator.throw_validation_errors()?;
            }
        }

        // 3. Validate the staff member has no other cashier assignment whose dates and hours overlap
        let allocated = existing_staff_cashiers
            .iter()
            .filter(|existing| existing.staff_id == self.staff_id)
            .filter(|existing| !(self.id > 0 && existing.id == self.id))
            .filter(|existing| existing.dates_overlap(self.start_date, self.end_date))
            .any(|existing| {
                self.is_full_day
                    || existing.is_full_day
                    || existing.hours_overlap(
                        start_time.expect("part-day start_time validated"),
                        end_time.expect("part-day end_time validated"),
                    )
            });

        if allocated {
            return Err(CashierAlreadyAllocated::new().into());
        }

        Ok(())
    }
}

/// Cashier transaction aggregate representing a row in the `m_cashier_transactions` table.
///
/// Ported from `org.apache.fineract.organisation.teller.domain.CashierTransaction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashierTransaction {
    pub id: i64,
    pub cashier_id: i64,
    pub txn_type: CashierTxnType,
    pub txn_amount: Decimal,
    pub txn_date: NaiveDate,
    pub created_date: DateTime<Utc>,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub txn_note: Option<String>,
    pub currency_code: String,
}

impl CashierTransaction {
    /// Creates a new [`CashierTransaction`] entity.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        cashier_id: i64,
        txn_type: CashierTxnType,
        txn_amount: Decimal,
        txn_date: NaiveDate,
        created_date: DateTime<Utc>,
        entity_type: Option<String>,
        entity_id: Option<i64>,
        txn_note: Option<String>,
        currency_code: impl Into<String>,
    ) -> Self {
        Self {
            id,
            cashier_id,
            txn_type,
            txn_amount,
            txn_date,
            created_date,
            entity_type,
            entity_id,
            txn_note,
            currency_code: currency_code.into(),
        }
    }

    /// Constructs a [`CashierTransaction`] from a validated [`JsonCommand`] (`CashierTransaction.fromJson`).
    pub fn from_json(
        cashier_id: i64,
        txn_type: CashierTxnType,
        command: &JsonCommand,
    ) -> FineractResult<Self> {
        TellerCommandFromApiJsonDeserializer::validate_for_cash_txn_for_cashier(command)?;

        let txn_amount = command
            .big_decimal_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::TXN_AMOUNT)?
            .expect("validated txnAmount is not null");
        let txn_date = command
            .local_date_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::TXN_DATE)?
            .expect("validated txnDate is not null");
        let entity_type =
            command.string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::ENTITY_TYPE);
        let entity_id =
            command.long_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::ENTITY_ID)?;
        let txn_note =
            command.string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::TXN_NOTE);
        let currency_code = command
            .string_value_of_parameter_named(TellerCommandFromApiJsonDeserializer::CURRENCY_CODE)
            .unwrap_or_else(|| "USD".to_string());

        Ok(Self {
            id: 0,
            cashier_id,
            txn_type,
            txn_amount,
            txn_date,
            created_date: Utc::now(),
            entity_type,
            entity_id,
            txn_note,
            currency_code,
        })
    }

    /// Applies a partial update from [`JsonCommand`] and returns changed fields (`CashierTransaction.update`).
    pub fn update(
        &mut self,
        command: &JsonCommand,
    ) -> FineractResult<BTreeMap<String, Value>> {
        let mut actual_changes = BTreeMap::new();
        let date_format = command.date_format();
        let locale = command.locale();

        if command.is_change_in_integer_parameter_named("txnType", Some(self.txn_type.id()))? {
            if let Some(new_val) = command.integer_value_of_parameter_named("txnType")? {
                if let Some(parsed_type) = CashierTxnType::get_cashier_txn_type(new_val) {
                    actual_changes.insert("txnType".to_string(), Value::from(new_val));
                    self.txn_type = parsed_type;
                }
            }
        }

        if command.is_change_in_local_date_parameter_named("txnDate", Some(self.txn_date))? {
            if let Some(raw) = command.string_value_of_parameter_named("txnDate") {
                actual_changes.insert("txnDate".to_string(), Value::String(raw));
            }
            if !date_format.is_empty() {
                actual_changes.insert("dateFormat".to_string(), Value::String(date_format));
            }
            if !locale.is_empty() {
                actual_changes.insert("locale".to_string(), Value::String(locale));
            }
            if let Some(new_date) = command.local_date_value_of_parameter_named("txnDate")? {
                self.txn_date = new_date;
            }
        }

        if command.is_change_in_big_decimal_parameter_named("txnAmount", Some(self.txn_amount))? {
            if let Some(new_amt) = command.big_decimal_value_of_parameter_named("txnAmount")? {
                actual_changes.insert(
                    "txnAmount".to_string(),
                    serde_json::json!(new_amt),
                );
                self.txn_amount = new_amt;
            }
        }

        if command.is_change_in_string_parameter_named("txnNote", self.txn_note.as_deref()) {
            let new_note = command.string_value_of_parameter_named("txnNote");
            actual_changes.insert(
                "txnNote".to_string(),
                new_note.clone().map(Value::String).unwrap_or(Value::Null),
            );
            self.txn_note = new_note;
        }

        if command.is_change_in_string_parameter_named("currencyCode", Some(&self.currency_code)) {
            if let Some(new_curr) = command.string_value_of_parameter_named("currencyCode") {
                actual_changes.insert("currencyCode".to_string(), Value::String(new_curr.clone()));
                self.currency_code = new_curr;
            }
        }

        Ok(actual_changes)
    }

    /// Converts `txn_amount` into a typed [`Money`] using `fineract-money`.
    pub fn to_money(&self, digits_after_decimal: u32, mode: FineractRoundingMode) -> Money {
        let currency = MonetaryCurrency::with_code(self.currency_code.clone(), digits_after_decimal);
        Money::of(currency, self.txn_amount, mode)
    }
}

// =============================================================================
// 5. READ DTOs & CASHIER SUMMARY AND TRANSACTIONS (TILL BALANCE ENGINE)
// =============================================================================

/// Immutable read DTO for teller data (`TellerData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TellerData {
    pub id: i64,
    pub office_id: i64,
    pub debit_account_id: Option<i64>,
    pub credit_account_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub status: TellerStatus,
    pub office_name: Option<String>,
    pub has_transactions: Option<bool>,
    pub has_mapped_cashiers: Option<bool>,
}

impl From<&Teller> for TellerData {
    fn from(teller: &Teller) -> Self {
        Self {
            id: teller.id,
            office_id: teller.office_id,
            debit_account_id: teller.debit_account_id,
            credit_account_id: teller.credit_account_id,
            name: teller.name.clone(),
            description: teller.description.clone(),
            start_date: teller.start_date,
            end_date: teller.end_date,
            status: teller.status,
            office_name: None,
            has_transactions: None,
            has_mapped_cashiers: None,
        }
    }
}

/// Immutable read DTO for cashier data (`CashierData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashierData {
    pub id: i64,
    pub teller_id: i64,
    pub office_id: i64,
    pub staff_id: i64,
    pub description: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_full_day: bool,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub office_name: Option<String>,
    pub teller_name: Option<String>,
    pub staff_name: Option<String>,
}

impl From<&Cashier> for CashierData {
    fn from(cashier: &Cashier) -> Self {
        Self {
            id: cashier.id,
            teller_id: cashier.teller_id,
            office_id: cashier.office_id,
            staff_id: cashier.staff_id,
            description: cashier.description.clone(),
            start_date: cashier.start_date,
            end_date: cashier.end_date,
            is_full_day: cashier.is_full_day,
            start_time: cashier.start_time.clone(),
            end_time: cashier.end_time.clone(),
            office_name: None,
            teller_name: None,
            staff_name: None,
        }
    }
}

/// Response record listing cashiers assigned to a teller (`CashiersForTeller.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashiersForTeller {
    pub teller_id: i64,
    pub teller_name: String,
    pub office_id: i64,
    pub office_name: Option<String>,
    pub cashiers: Vec<CashierData>,
}

/// Aggregated transaction total for a single [`CashierTxnType`] (`CashierTransactionTypeTotalsData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashierTransactionTypeTotalsData {
    pub cashier_txn_type: CashierTxnType,
    pub cash_total: Decimal,
}

impl CashierTransactionTypeTotalsData {
    pub const fn instance(cashier_txn_type: CashierTxnType, cash_total: Decimal) -> Self {
        Self {
            cashier_txn_type,
            cash_total,
        }
    }
}

/// Immutable read DTO for a cashier transaction (`CashierTransactionData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashierTransactionData {
    pub id: i64,
    pub cashier_id: i64,
    pub txn_type: CashierTxnType,
    pub txn_amount: Decimal,
    pub txn_date: NaiveDate,
    pub entity_id: Option<i64>,
    pub entity_type: Option<String>,
    pub txn_note: Option<String>,
    pub created_date: DateTime<Utc>,
    pub office_id: Option<i64>,
    pub office_name: Option<String>,
    pub teller_id: Option<i64>,
    pub teller_name: Option<String>,
    pub cashier_name: Option<String>,
    pub cashier_data: Option<CashierData>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub currency_options: Option<Vec<CurrencyData>>,
}

impl From<&CashierTransaction> for CashierTransactionData {
    fn from(txn: &CashierTransaction) -> Self {
        Self {
            id: txn.id,
            cashier_id: txn.cashier_id,
            txn_type: txn.txn_type,
            txn_amount: txn.txn_amount,
            txn_date: txn.txn_date,
            entity_id: txn.entity_id,
            entity_type: txn.entity_type.clone(),
            txn_note: txn.txn_note.clone(),
            created_date: txn.created_date,
            office_id: None,
            office_name: None,
            teller_id: None,
            teller_name: None,
            cashier_name: None,
            cashier_data: None,
            start_date: None,
            end_date: None,
            currency_options: None,
        }
    }
}

/// Summary and paginated transactions for a cashier till, computing net cash in till:
/// `net_cash = sum(Allocate) + sum(CashIn) - sum(Settle) - sum(CashOut)`
/// and enforcing [`NotEnoughCashInTheMainVault`] / [`CashierInsufficientAmountException`] when
/// settling or paying out more cash than available in the cashier's till.
///
/// Ported from `org.apache.fineract.organisation.teller.data.CashierTransactionsWithSummaryData`
/// and `org.apache.fineract.organisation.teller.data.CashierTransactionDataValidator`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashierSummaryAndTransactions {
    pub sum_cash_allocation: Decimal,
    pub sum_inward_cash: Decimal,
    pub sum_outward_cash: Decimal,
    pub sum_cash_settlement: Decimal,
    pub net_cash: Decimal,
    pub office_name: Option<String>,
    pub teller_id: i64,
    pub teller_name: Option<String>,
    pub cashier_id: i64,
    pub cashier_name: Option<String>,
    pub cashier_transactions: Page<CashierTransaction>,
}

/// Alias matching the upstream Java class name `CashierTransactionsWithSummaryData`.
pub type CashierTransactionsWithSummaryData = CashierSummaryAndTransactions;

impl CashierSummaryAndTransactions {
    /// Computes net cash in till using exact `Decimal` arithmetic:
    /// `sum(Allocate) + sum(CashIn) - sum(Settle) - sum(CashOut)`.
    ///
    /// Matches `CashierTransactionsWithSummaryData.instance`:
    /// `sumCashAllocation.add(sumInwardCash).subtract(sumOutwardCash).subtract(sumCashSettlement)`.
    pub fn compute_net_cash(
        sum_cash_allocation: Decimal,
        sum_inward_cash: Decimal,
        sum_outward_cash: Decimal,
        sum_cash_settlement: Decimal,
    ) -> Decimal {
        let alloc = FineractDecimal::new(sum_cash_allocation);
        let cash_in = FineractDecimal::new(sum_inward_cash);
        let cash_out = FineractDecimal::new(sum_outward_cash);
        let settle = FineractDecimal::new(sum_cash_settlement);
        (alloc.inner() + cash_in.inner()) - cash_out.inner() - settle.inner()
    }

    /// Creates a new [`CashierSummaryAndTransactions`] instance, automatically computing `net_cash`.
    ///
    /// Ported from `CashierTransactionsWithSummaryData.instance(...)`.
    #[allow(clippy::too_many_arguments)]
    pub fn instance(
        cashier_transactions: Page<CashierTransaction>,
        sum_cash_allocation: Decimal,
        sum_inward_cash: Decimal,
        sum_outward_cash: Decimal,
        sum_cash_settlement: Decimal,
        office_name: Option<String>,
        teller_id: i64,
        teller_name: Option<String>,
        cashier_id: i64,
        cashier_name: Option<String>,
    ) -> Self {
        let net_cash = Self::compute_net_cash(
            sum_cash_allocation,
            sum_inward_cash,
            sum_outward_cash,
            sum_cash_settlement,
        );
        Self {
            sum_cash_allocation,
            sum_inward_cash,
            sum_outward_cash,
            sum_cash_settlement,
            net_cash,
            office_name,
            teller_id,
            teller_name,
            cashier_id,
            cashier_name,
            cashier_transactions,
        }
    }

    /// Aggregates a list of [`CashierTransaction`]s for a cashier (optionally filtered by `currency_code`
    /// and cashier active window `[cashier_start_date, cashier_end_date]`, where `Allocate` transactions
    /// are always included per `TellerManagementReadPlatformServiceImpl`:
    /// `((txn.created_date between c.start_date AND c.end_date) or txn.txn_type = 101)`).
    pub fn from_transactions(
        cashier_id: i64,
        teller_id: i64,
        office_name: Option<String>,
        teller_name: Option<String>,
        cashier_name: Option<String>,
        transactions: &[CashierTransaction],
        currency_code: Option<&str>,
    ) -> Self {
        let mut sum_cash_allocation = Decimal::ZERO;
        let mut sum_inward_cash = Decimal::ZERO;
        let mut sum_outward_cash = Decimal::ZERO;
        let mut sum_cash_settlement = Decimal::ZERO;
        let mut filtered = Vec::new();

        for txn in transactions {
            if txn.cashier_id != cashier_id {
                continue;
            }
            if let Some(code) = currency_code {
                if !txn.currency_code.eq_ignore_ascii_case(code) {
                    continue;
                }
            }
            match txn.txn_type {
                CashierTxnType::Allocate => sum_cash_allocation += txn.txn_amount,
                CashierTxnType::Settle => sum_cash_settlement += txn.txn_amount,
                CashierTxnType::CashIn => sum_inward_cash += txn.txn_amount,
                CashierTxnType::CashOut => sum_outward_cash += txn.txn_amount,
            }
            filtered.push(txn.clone());
        }

        filtered.sort_by_key(|t| (t.created_date, t.id));
        let total_records = filtered.len() as i64;
        let page = Page::new(filtered, total_records);

        Self::instance(
            page,
            sum_cash_allocation,
            sum_inward_cash,
            sum_outward_cash,
            sum_cash_settlement,
            office_name,
            teller_id,
            teller_name,
            cashier_id,
            cashier_name,
        )
    }

    pub const fn sum_cash_allocation(&self) -> Decimal {
        self.sum_cash_allocation
    }

    pub const fn sum_inward_cash(&self) -> Decimal {
        self.sum_inward_cash
    }

    pub const fn sum_outward_cash(&self) -> Decimal {
        self.sum_outward_cash
    }

    pub const fn sum_cash_settlement(&self) -> Decimal {
        self.sum_cash_settlement
    }

    pub const fn net_cash(&self) -> Decimal {
        self.net_cash
    }

    /// Enforces that a settlement (`Settle`) or cash payout (`CashOut`) does not exceed `net_cash` in the till.
    ///
    /// Returns [`CashierInsufficientAmountException`] (`"cashier.insufficient.amount.exception"`)
    /// wrapped in [`FineractError::DomainRule`] when `transaction_amount > self.net_cash`.
    ///
    /// Ported from `CashierTransactionDataValidator.validateSettleCashAndCashOutTransactions`.
    pub fn validate_settle_cash_and_cash_out(
        &self,
        transaction_amount: Decimal,
    ) -> FineractResult<()> {
        if transaction_amount > self.net_cash {
            return Err(CashierInsufficientAmountException::new().into());
        }
        Ok(())
    }

    /// Validates a settlement or outward cash transaction and returns [`NotEnoughCashInTheMainVault`]
    /// if `transaction_amount > self.net_cash`.
    pub fn ensure_sufficient_till_cash(
        &self,
        transaction_amount: Decimal,
    ) -> Result<(), NotEnoughCashInTheMainVault> {
        if transaction_amount > self.net_cash {
            return Err(NotEnoughCashInTheMainVault::with_amounts(
                transaction_amount,
                self.net_cash,
            ));
        }
        Ok(())
    }
}

/// Stateless/stateful validator ported from `org.apache.fineract.organisation.teller.data.CashierTransactionDataValidator`.
#[derive(Debug, Clone, Default)]
pub struct CashierTransactionDataValidator;

impl CashierTransactionDataValidator {
    pub const fn new() -> Self {
        Self
    }

    /// Validates that `transaction_amount <= summary.net_cash()`.
    ///
    /// Ported from `CashierTransactionDataValidator.validateSettleCashAndCashOutTransactions`.
    pub fn validate_settle_cash_and_cash_out_transactions(
        summary: &CashierSummaryAndTransactions,
        transaction_amount: Decimal,
    ) -> FineractResult<()> {
        summary.validate_settle_cash_and_cash_out(transaction_amount)
    }

    /// Validates cashier date range, part-day shift hours, and staff assignment overlap.
    ///
    /// Ported from `CashierTransactionDataValidator.validateCashierAllowedDateAndTime`.
    pub fn validate_cashier_allowed_date_and_time(
        cashier: &Cashier,
        teller: &Teller,
        existing_staff_cashiers: &[Cashier],
    ) -> FineractResult<()> {
        cashier.validate_allowed_date_and_time(teller, existing_staff_cashiers)
    }

    /// Validates a cash loan disbursal against the active cashier on duty for `staff_id` at `(tenant_date, tenant_time)`.
    ///
    /// If the staff member is not currently on duty as a cashier, no balance check is enforced (matching upstream Fineract).
    /// Ported from `CashierTransactionDataValidator.validateOnLoanDisbursal`.
    pub fn validate_on_loan_disbursal(
        staff_id: Option<i64>,
        tenant_date: NaiveDate,
        tenant_time: NaiveTime,
        staff_cashiers: &[Cashier],
        cashier_summaries: &BTreeMap<i64, CashierSummaryAndTransactions>,
        transaction_amount: Decimal,
    ) -> FineractResult<()> {
        let Some(sid) = staff_id else {
            return Ok(());
        };
        let on_duty: Vec<i64> = staff_cashiers
            .iter()
            .filter(|c| c.staff_id == sid && c.is_on_duty(tenant_date, tenant_time))
            .map(|c| c.id)
            .collect();

        if let [cashier_id] = on_duty.as_slice() {
            if let Some(summary) = cashier_summaries.get(cashier_id) {
                summary.validate_settle_cash_and_cash_out(transaction_amount)?;
            }
        }
        Ok(())
    }
}

/// Date range parser ported from `org.apache.fineract.organisation.teller.util.DateRange`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateRange {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

impl DateRange {
    pub const RANGE_DELIMITER: &'static str = "..";

    pub const fn new(start_date: Option<NaiveDate>, end_date: Option<NaiveDate>) -> Self {
        Self {
            start_date,
            end_date,
        }
    }

    /// Parses a `"yyyy-MM-dd..yyyy-MM-dd"` or single `"yyyy-MM-dd"` string into a [`DateRange`],
    /// defaulting `None` to `business_date`.
    ///
    /// Ported from `DateRange.fromString(final String dateToParse)`.
    pub fn from_str_with_default(date_to_parse: Option<&str>, business_date: NaiveDate) -> Self {
        let Some(raw) = date_to_parse.map(str::trim).filter(|s| !s.is_empty()) else {
            return Self {
                start_date: Some(business_date),
                end_date: None,
            };
        };

        let mut parts = raw.split(Self::RANGE_DELIMITER).filter(|s| !s.is_empty());
        let start_date = parts
            .next()
            .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());
        let end_date = parts
            .next()
            .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());

        Self {
            start_date,
            end_date,
        }
    }
}

impl FromStr for DateRange {
    type Err = chrono::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(Self::RANGE_DELIMITER).filter(|p| !p.is_empty());
        let start_date = match parts.next() {
            Some(tok) => Some(NaiveDate::parse_from_str(tok.trim(), "%Y-%m-%d")?),
            None => None,
        };
        let end_date = match parts.next() {
            Some(tok) => Some(NaiveDate::parse_from_str(tok.trim(), "%Y-%m-%d")?),
            None => None,
        };
        Ok(Self {
            start_date,
            end_date,
        })
    }
}

// =============================================================================
// 6. DUAL-DATABASE TELLER, CASHIER & TRANSACTION REPOSITORY (`DualDbTellerRepository`)
// =============================================================================

/// Dual-Database (`PostgreSQL 16+` & `MySQL / MariaDB`) repository for `m_tellers`, `m_cashiers`,
/// and `m_cashier_transactions`, with integrated concurrent in-memory fallback for unit and
/// pipeline testing without a live database server.
///
/// Uses [`sqlx::AnyPool`] and [`DialectSql::rewrite_placeholders`] (`?` -> `$1..$N` on PostgreSQL)
/// in strict compliance with Invariant 2 of the Fineract-RS Architectural Constitution.
#[derive(Debug, Clone)]
pub struct DualDbTellerRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    tellers: Arc<DashMap<i64, Teller>>,
    cashiers: Arc<DashMap<i64, Cashier>>,
    transactions: Arc<DashMap<i64, CashierTransaction>>,
    main_vault_balances: Arc<DashMap<(i64, String), Decimal>>,
    next_teller_id: Arc<AtomicI64>,
    next_cashier_id: Arc<AtomicI64>,
    next_txn_id: Arc<AtomicI64>,
}

impl Default for DualDbTellerRepository {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl DualDbTellerRepository {
    /// Columns selected from `m_tellers`.
    pub const TELLER_COLUMNS: &'static str =
        "id, office_id, debit_account_id, credit_account_id, name, description, valid_from, valid_to, state";

    /// Columns selected from `m_cashiers`.
    pub const CASHIER_COLUMNS: &'static str =
        "id, staff_id, teller_id, description, start_date, end_date, start_time, end_time, full_day";

    /// Columns selected from `m_cashier_transactions`.
    pub const CASHIER_TXN_COLUMNS: &'static str =
        "id, cashier_id, txn_type, txn_amount, txn_date, created_date, entity_type, entity_id, txn_note, currency_code";

    /// Creates a new [`DualDbTellerRepository`] backed by a live [`sqlx::AnyPool`] and [`SqlDialect`].
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            tellers: Arc::new(DashMap::new()),
            cashiers: Arc::new(DashMap::new()),
            transactions: Arc::new(DashMap::new()),
            main_vault_balances: Arc::new(DashMap::new()),
            next_teller_id: Arc::new(AtomicI64::new(1)),
            next_cashier_id: Arc::new(AtomicI64::new(1)),
            next_txn_id: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Creates an in-memory [`DualDbTellerRepository`] (defaulting to [`SqlDialect::Postgres`]).
    pub fn in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    /// Creates an in-memory [`DualDbTellerRepository`] configured with the specified [`SqlDialect`].
    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            pool: None,
            dialect,
            tellers: Arc::new(DashMap::new()),
            cashiers: Arc::new(DashMap::new()),
            transactions: Arc::new(DashMap::new()),
            main_vault_balances: Arc::new(DashMap::new()),
            next_teller_id: Arc::new(AtomicI64::new(1)),
            next_cashier_id: Arc::new(AtomicI64::new(1)),
            next_txn_id: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Returns the configured [`SqlDialect`].
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    /// Returns the underlying [`AnyPool`] if connected to a live database.
    pub const fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    /// Configures an explicit main vault balance for `(office_id, currency_code)` to enforce
    /// [`NotEnoughCashInTheMainVault`] when allocating cash from the main vault to a cashier.
    pub fn set_main_vault_balance(
        &self,
        office_id: i64,
        currency_code: impl Into<String>,
        balance: Decimal,
    ) {
        self.main_vault_balances.insert(
            (office_id, currency_code.into().to_ascii_uppercase()),
            balance,
        );
    }

    /// Returns the tracked main vault balance for `(office_id, currency_code)`, if configured.
    pub fn get_main_vault_balance(&self, office_id: i64, currency_code: &str) -> Option<Decimal> {
        self.main_vault_balances
            .get(&(office_id, currency_code.to_ascii_uppercase()))
            .map(|v| *v.value())
    }

    // -------------------------------------------------------------------------
    // Dialect-Aware SQL Generators (`PostgreSQL` $1..$N vs `MySQL` ?)
    // -------------------------------------------------------------------------

    pub fn select_teller_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_tellers WHERE id = ?",
            Self::TELLER_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_tellers_by_office_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_tellers WHERE office_id = ? ORDER BY id",
            Self::TELLER_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn insert_teller_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_tellers (\
            id, office_id, debit_account_id, credit_account_id, name, description, valid_from, valid_to, state\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn update_teller_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_tellers SET \
            office_id = ?, debit_account_id = ?, credit_account_id = ?, name = ?, description = ?, \
            valid_from = ?, valid_to = ?, state = ? \
            WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn delete_teller_sql(dialect: SqlDialect) -> String {
        let raw = "DELETE FROM m_tellers WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn select_cashier_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT c.id, c.staff_id, c.teller_id, c.description, c.start_date, c.end_date, \
             c.start_time, c.end_time, c.full_day, t.office_id \
             FROM m_cashiers c JOIN m_tellers t ON t.id = c.teller_id WHERE c.id = ?"
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_cashiers_by_teller_sql(dialect: SqlDialect) -> String {
        let raw = "SELECT c.id, c.staff_id, c.teller_id, c.description, c.start_date, c.end_date, \
            c.start_time, c.end_time, c.full_day, t.office_id \
            FROM m_cashiers c JOIN m_tellers t ON t.id = c.teller_id WHERE c.teller_id = ? ORDER BY c.id";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn select_overlapping_cashiers_of_staff_sql(dialect: SqlDialect) -> String {
        let raw = "SELECT c.id, c.staff_id, c.teller_id, c.description, c.start_date, c.end_date, \
            c.start_time, c.end_time, c.full_day, COALESCE(t.office_id, 0) AS office_id \
            FROM m_cashiers c LEFT JOIN m_tellers t ON t.id = c.teller_id \
            WHERE c.staff_id = ? AND c.start_date <= ? AND c.end_date >= ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn insert_cashier_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_cashiers (\
            id, staff_id, teller_id, description, start_date, end_date, start_time, end_time, full_day\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn update_cashier_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_cashiers SET \
            staff_id = ?, teller_id = ?, description = ?, start_date = ?, end_date = ?, \
            start_time = ?, end_time = ?, full_day = ? \
            WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn delete_cashier_sql(dialect: SqlDialect) -> String {
        let raw = "DELETE FROM m_cashiers WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn insert_cashier_txn_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_cashier_transactions (\
            id, cashier_id, txn_type, txn_amount, txn_date, created_date, entity_type, entity_id, txn_note, currency_code\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn select_cashier_txns_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_cashier_transactions WHERE cashier_id = ? AND currency_code = ? ORDER BY created_date, id",
            Self::CASHIER_TXN_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_cashier_txn_totals_sql(dialect: SqlDialect) -> String {
        let raw = "SELECT txn_type AS cash_txn_type, SUM(txn_amount) AS txn_total \
            FROM m_cashier_transactions \
            WHERE cashier_id = ? AND currency_code = ? \
            GROUP BY txn_type";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    // -------------------------------------------------------------------------
    // Row Mapping Helpers
    // -------------------------------------------------------------------------

    fn parse_date_str(s: &str) -> Option<NaiveDate> {
    let part = s.split(|c: char| c == ' ' || c == 'T').next().unwrap_or(s);
    NaiveDate::parse_from_str(part, "%Y-%m-%d").ok()
}

fn parse_datetime_str(s: &str) -> Option<NaiveDateTime> {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
            .ok()
            .or_else(|| Self::parse_date_str(s).map(|d| d.and_hms_opt(0, 0, 0).unwrap_or_default()))
    }

    fn map_teller_row(row: &sqlx::any::AnyRow) -> FineractResult<Teller> {
        let id: i64 = row
            .try_get("id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let office_id: i64 = row
            .try_get("office_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let debit_account_id: Option<i64> = row
            .try_get("debit_account_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let credit_account_id: Option<i64> = row
            .try_get("credit_account_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let name: String = row
            .try_get("name")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let description: Option<String> = row
            .try_get("description")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let start_date_str: String = row
            .try_get("valid_from")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let start_date = Self::parse_date_str(&start_date_str)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        let end_date: Option<NaiveDate> = row
            .try_get::<Option<String>, _>("valid_to")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?
            .and_then(|s| Self::parse_date_str(&s));
        let state: i16 = row
            .try_get("state")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(Teller {
            id,
            office_id,
            debit_account_id,
            credit_account_id,
            name,
            description,
            start_date,
            end_date,
            status: TellerStatus::from_i16(state),
        })
    }

    fn map_cashier_row(row: &sqlx::any::AnyRow) -> FineractResult<Cashier> {
        let id: i64 = row
            .try_get("id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let office_id: i64 = row.try_get("office_id").unwrap_or(0);
        let staff_id: i64 = row
            .try_get("staff_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let teller_id: i64 = row
            .try_get("teller_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let description: Option<String> = row
            .try_get("description")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let start_date_str: String = row
            .try_get("start_date")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let start_date = Self::parse_date_str(&start_date_str)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        let end_date_str: String = row
            .try_get("end_date")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let end_date = Self::parse_date_str(&end_date_str)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        let start_time: Option<String> = row
            .try_get("start_time")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let end_time: Option<String> = row
            .try_get("end_time")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let is_full_day: bool = row.try_get("full_day").unwrap_or(false);

        Ok(Cashier {
            id,
            office_id,
            staff_id,
            teller_id,
            description,
            start_date,
            end_date,
            start_time,
            end_time,
            is_full_day,
        })
    }

    fn map_cashier_txn_row(row: &sqlx::any::AnyRow) -> FineractResult<CashierTransaction> {
        let id: i64 = row
            .try_get("id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let cashier_id: i64 = row
            .try_get("cashier_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let txn_type_raw: i16 = row
            .try_get("txn_type")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let txn_type =
            CashierTxnType::from_i16(txn_type_raw).unwrap_or(CashierTxnType::Allocate);
        let amount_str: String = row
            .try_get::<String, _>("txn_amount")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let txn_amount = Decimal::from_str(&amount_str)
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let txn_date_str: String = row
            .try_get("txn_date")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let txn_date = Self::parse_date_str(&txn_date_str)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        let created_naive_str: String = row
            .try_get("created_date")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let created_naive = Self::parse_datetime_str(&created_naive_str)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap());
        let entity_type: Option<String> = row
            .try_get("entity_type")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let entity_id: Option<i64> = row
            .try_get("entity_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let txn_note: Option<String> = row
            .try_get("txn_note")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let currency_code: Option<String> = row
            .try_get("currency_code")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(CashierTransaction {
            id,
            cashier_id,
            txn_type,
            txn_amount,
            txn_date,
            created_date: DateTime::<Utc>::from_naive_utc_and_offset(created_naive, Utc),
            entity_type,
            entity_id,
            txn_note,
            currency_code: currency_code.unwrap_or_else(|| "USD".to_string()),
        })
    }

    // -------------------------------------------------------------------------
    // Teller Persistence Operations
    // -------------------------------------------------------------------------

    /// Finds a [`Teller`] by ID or returns [`TellerNotFoundException`].
    pub async fn find_teller_with_not_found_detection(&self, teller_id: i64) -> FineractResult<Teller> {
        self.find_teller_by_id(teller_id)
            .await?
            .ok_or_else(|| TellerNotFoundException::new(teller_id).into())
    }

    /// Looks up a [`Teller`] by ID.
    pub async fn find_teller_by_id(&self, teller_id: i64) -> FineractResult<Option<Teller>> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_teller_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(teller_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            return maybe_row.as_ref().map(Self::map_teller_row).transpose();
        }
        Ok(self.tellers.get(&teller_id).map(|r| r.value().clone()))
    }

    /// Lists all [`Teller`]s for an office (or all tellers if `office_id` is `None`).
    pub async fn list_tellers(&self, office_id: Option<i64>) -> FineractResult<Vec<Teller>> {
        if let (Some(ref pool), Some(oid)) = (&self.pool, office_id) {
            let sql = Self::select_tellers_by_office_sql(self.dialect);
            let rows = sqlx::query(&sql)
                .bind(oid)
                .fetch_all(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            return rows.iter().map(Self::map_teller_row).collect();
        }
        let mut items: Vec<Teller> = self
            .tellers
            .iter()
            .filter(|entry| office_id.map_or(true, |oid| entry.value().office_id == oid))
            .map(|entry| entry.value().clone())
            .collect();
        items.sort_by_key(|t| t.id);
        Ok(items)
    }

    /// Inserts or updates a [`Teller`] after enforcing date range and unique name constraints.
    pub async fn save_teller(&self, mut teller: Teller) -> FineractResult<Teller> {
        if let Some(ed) = teller.end_date {
            if ed < teller.start_date {
                return Err(InvalidDateInputException::new(
                    teller.start_date.to_string(),
                    ed.to_string(),
                )
                .into());
            }
        }

        // Enforce unique teller name (`ux_tellers_name` / `m_tellers_office_name_unq`)
        let duplicate = self.tellers.iter().any(|entry| {
            entry.value().id != teller.id
                && entry.value().name.eq_ignore_ascii_case(&teller.name)
        });
        if duplicate {
            return Err(duplicate_teller_name_error(&teller.name));
        }

        let is_new = teller.id <= 0 || !self.tellers.contains_key(&teller.id);
        if teller.id <= 0 {
            teller.id = self.next_teller_id.fetch_add(1, Ordering::SeqCst);
        }

        if let Some(ref pool) = self.pool {
            let sql = if is_new {
                Self::insert_teller_sql(self.dialect)
            } else {
                Self::update_teller_sql(self.dialect)
            };
            let res = if is_new {
                sqlx::query(&sql)
                    .bind(teller.id)
                    .bind(teller.office_id)
                    .bind(teller.debit_account_id)
                    .bind(teller.credit_account_id)
                    .bind(&teller.name)
                    .bind(&teller.description)
                    .bind(teller.start_date.to_string())
                    .bind(teller.end_date.map(|d| d.to_string()))
                    .bind(teller.status.as_i16())
                    .execute(pool)
                    .await
            } else {
                sqlx::query(&sql)
                    .bind(teller.office_id)
                    .bind(teller.debit_account_id)
                    .bind(teller.credit_account_id)
                    .bind(&teller.name)
                    .bind(&teller.description)
                    .bind(teller.start_date.to_string())
                    .bind(teller.end_date.map(|d| d.to_string()))
                    .bind(teller.status.as_i16())
                    .bind(teller.id)
                    .execute(pool)
                    .await
            };

            if let Err(e) = res {
                let msg = e.to_string();
                if msg.contains("m_tellers_office_name_unq")
                    || msg.contains("ux_tellers_name")
                    || msg.to_ascii_uppercase().contains("UNIQUE")
                    || msg.to_ascii_uppercase().contains("DUPLICATE")
                {
                    return Err(duplicate_teller_name_error(&teller.name));
                }
                return Err(FineractError::DatabaseError(msg));
            }
        }

        self.tellers.insert(teller.id, teller.clone());
        Ok(teller)
    }

    /// Deletes a [`Teller`] by ID, rejecting with [`CashierExistForTellerException`] if any cashier is mapped to it.
    pub async fn delete_teller(&self, teller_id: i64) -> FineractResult<()> {
        let _teller = self.find_teller_with_not_found_detection(teller_id).await?;
        let cashiers = self.list_cashiers_for_teller(teller_id).await?;
        if !cashiers.is_empty() {
            return Err(CashierExistForTellerException::new(teller_id).into());
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::delete_teller_sql(self.dialect);
            sqlx::query(&sql)
                .bind(teller_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.tellers.remove(&teller_id);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Cashier Persistence Operations
    // -------------------------------------------------------------------------

    /// Finds a [`Cashier`] by ID or returns [`CashierNotFoundException`].
    pub async fn find_cashier_with_not_found_detection(
        &self,
        cashier_id: i64,
    ) -> FineractResult<Cashier> {
        self.find_cashier_by_id(cashier_id)
            .await?
            .ok_or_else(|| CashierNotFoundException::new(cashier_id).into())
    }

    /// Looks up a [`Cashier`] by ID.
    pub async fn find_cashier_by_id(&self, cashier_id: i64) -> FineractResult<Option<Cashier>> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_cashier_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(cashier_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            return maybe_row.as_ref().map(Self::map_cashier_row).transpose();
        }
        Ok(self.cashiers.get(&cashier_id).map(|r| r.value().clone()))
    }

    /// Lists all [`Cashier`] assignments for `teller_id`.
    pub async fn list_cashiers_for_teller(&self, teller_id: i64) -> FineractResult<Vec<Cashier>> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_cashiers_by_teller_sql(self.dialect);
            let rows = sqlx::query(&sql)
                .bind(teller_id)
                .fetch_all(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            return rows.iter().map(Self::map_cashier_row).collect();
        }
        let mut items: Vec<Cashier> = self
            .cashiers
            .iter()
            .filter(|entry| entry.value().teller_id == teller_id)
            .map(|entry| entry.value().clone())
            .collect();
        items.sort_by_key(|c| c.id);
        Ok(items)
    }

    /// Queries all [`Cashier`] assignments of `staff_id` whose dates overlap `[from_date, to_date]`.
    ///
    /// Ported from `CashierTransactionDataValidator.findCashiersOfStaff`.
    pub async fn find_cashiers_of_staff(
        &self,
        staff_id: i64,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> FineractResult<Vec<Cashier>> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_overlapping_cashiers_of_staff_sql(self.dialect);
            let rows = sqlx::query(&sql)
                .bind(staff_id)
                .bind(to_date.to_string())
                .bind(from_date.to_string())
                .fetch_all(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            return rows.iter().map(Self::map_cashier_row).collect();
        }
        let mut items: Vec<Cashier> = self
            .cashiers
            .iter()
            .filter(|entry| {
                let c = entry.value();
                c.staff_id == staff_id && c.dates_overlap(from_date, to_date)
            })
            .map(|entry| entry.value().clone())
            .collect();
        items.sort_by_key(|c| c.id);
        Ok(items)
    }

    /// Allocates or updates a [`Cashier`] after validating teller date range, shift times, and staff overlap.
    pub async fn allocate_cashier(&self, mut cashier: Cashier) -> FineractResult<Cashier> {
        let teller = self
            .find_teller_with_not_found_detection(cashier.teller_id)
            .await?;
        cashier.office_id = teller.office_id;

        let existing_staff_cashiers = self
            .find_cashiers_of_staff(cashier.staff_id, cashier.start_date, cashier.end_date)
            .await?;
        cashier.validate_allowed_date_and_time(&teller, &existing_staff_cashiers)?;

        let is_new = cashier.id <= 0 || !self.cashiers.contains_key(&cashier.id);
        if cashier.id <= 0 {
            cashier.id = self.next_cashier_id.fetch_add(1, Ordering::SeqCst);
        }

        if let Some(ref pool) = self.pool {
            if is_new {
                let sql = Self::insert_cashier_sql(self.dialect);
                sqlx::query(&sql)
                    .bind(cashier.id)
                    .bind(cashier.staff_id)
                    .bind(cashier.teller_id)
                    .bind(&cashier.description)
                    .bind(cashier.start_date.to_string())
                    .bind(cashier.end_date.to_string())
                    .bind(&cashier.start_time)
                    .bind(&cashier.end_time)
                    .bind(cashier.is_full_day)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            } else {
                let sql = Self::update_cashier_sql(self.dialect);
                sqlx::query(&sql)
                    .bind(cashier.staff_id)
                    .bind(cashier.teller_id)
                    .bind(&cashier.description)
                    .bind(cashier.start_date.to_string())
                    .bind(cashier.end_date.to_string())
                    .bind(&cashier.start_time)
                    .bind(&cashier.end_time)
                    .bind(cashier.is_full_day)
                    .bind(cashier.id)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }
        }

        self.cashiers.insert(cashier.id, cashier.clone());
        Ok(cashier)
    }

    /// Deletes a [`Cashier`] assignment by ID.
    pub async fn delete_cashier(&self, teller_id: i64, cashier_id: i64) -> FineractResult<()> {
        let _teller = self.find_teller_with_not_found_detection(teller_id).await?;
        let _cashier = self
            .find_cashier_with_not_found_detection(cashier_id)
            .await?;

        if let Some(ref pool) = self.pool {
            let sql = Self::delete_cashier_sql(self.dialect);
            sqlx::query(&sql)
                .bind(cashier_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.cashiers.remove(&cashier_id);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Cashier Transactions & Summary Operations
    // -------------------------------------------------------------------------

    /// Lists all [`CashierTransaction`]s for `(cashier_id, currency_code)`.
    pub async fn list_cashier_transactions(
        &self,
        cashier_id: i64,
        currency_code: Option<&str>,
    ) -> FineractResult<Vec<CashierTransaction>> {
        if let (Some(ref pool), Some(curr)) = (&self.pool, currency_code) {
            let sql = Self::select_cashier_txns_sql(self.dialect);
            let rows = sqlx::query(&sql)
                .bind(cashier_id)
                .bind(curr)
                .fetch_all(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            return rows.iter().map(Self::map_cashier_txn_row).collect();
        }
        let mut txns: Vec<CashierTransaction> = self
            .transactions
            .iter()
            .filter(|entry| {
                let t = entry.value();
                t.cashier_id == cashier_id
                    && currency_code.map_or(true, |c| t.currency_code.eq_ignore_ascii_case(c))
            })
            .map(|entry| entry.value().clone())
            .collect();
        txns.sort_by_key(|t| (t.created_date, t.id));
        Ok(txns)
    }

    /// Retrieves the [`CashierSummaryAndTransactions`] for `(cashier_id, currency_code)`.
    ///
    /// Ported from `TellerManagementReadPlatformServiceImpl.retrieveCashierTransactionsWithSummary`.
    pub async fn retrieve_cashier_transactions_with_summary(
        &self,
        cashier_id: i64,
        currency_code: Option<&str>,
        _search_params: Option<&SearchParameters>,
    ) -> FineractResult<CashierSummaryAndTransactions> {
        let cashier = self
            .find_cashier_with_not_found_detection(cashier_id)
            .await?;
        let teller = self.find_teller_by_id(cashier.teller_id).await?;
        let teller_name = teller.as_ref().map(|t| t.name.clone());

        let txns = self
            .list_cashier_transactions(cashier_id, currency_code)
            .await?;

        Ok(CashierSummaryAndTransactions::from_transactions(
            cashier_id,
            cashier.teller_id,
            None,
            teller_name,
            None,
            &txns,
            currency_code,
        ))
    }

    /// Records a [`CashierTransaction`] (`Allocate`, `Settle`, `CashIn`, or `CashOut`),
    /// enforcing:
    /// - [`NotEnoughCashInTheMainVault`] if allocating cash from a tracked main vault with insufficient funds, and
    /// - [`CashierInsufficientAmountException`] / [`NotEnoughCashInTheMainVault`] if settling or paying out
    ///   more cash than available in the cashier's till (`txn_amount > summary.net_cash()`).
    pub async fn record_cashier_transaction(
        &self,
        mut txn: CashierTransaction,
    ) -> FineractResult<CashierTransaction> {
        let cashier = self
            .find_cashier_with_not_found_detection(txn.cashier_id)
            .await?;

        if txn.txn_type.decreases_till_balance() {
            let summary = self
                .retrieve_cashier_transactions_with_summary(
                    txn.cashier_id,
                    Some(&txn.currency_code),
                    None,
                )
                .await?;
            summary.validate_settle_cash_and_cash_out(txn.txn_amount)?;
        }

        if txn.txn_type == CashierTxnType::Allocate {
            let key = (cashier.office_id, txn.currency_code.to_ascii_uppercase());
            if let Some(mut vault_entry) = self.main_vault_balances.get_mut(&key) {
                if txn.txn_amount > *vault_entry.value() {
                    return Err(NotEnoughCashInTheMainVault::with_amounts(
                        txn.txn_amount,
                        *vault_entry.value(),
                    )
                    .into());
                }
                *vault_entry.value_mut() -= txn.txn_amount;
            }
        } else if txn.txn_type == CashierTxnType::Settle {
            let key = (cashier.office_id, txn.currency_code.to_ascii_uppercase());
            if let Some(mut vault_entry) = self.main_vault_balances.get_mut(&key) {
                *vault_entry.value_mut() += txn.txn_amount;
            }
        }

        if txn.id <= 0 {
            txn.id = self.next_txn_id.fetch_add(1, Ordering::SeqCst);
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_cashier_txn_sql(self.dialect);
            sqlx::query(&sql)
                .bind(txn.id)
                .bind(txn.cashier_id)
                .bind(txn.txn_type.as_i16())
                .bind(txn.txn_amount.to_string())
                .bind(txn.txn_date.to_string())
                .bind(txn.created_date.naive_utc().to_string())
                .bind(&txn.entity_type)
                .bind(txn.entity_id)
                .bind(&txn.txn_note)
                .bind(&txn.currency_code)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.transactions.insert(txn.id, txn.clone());
        Ok(txn)
    }

    /// Allocates cash from the main vault to `cashier_id` (`TellerWritePlatformServiceJpaImpl.allocateCashToCashier`).
    pub async fn allocate_cash_to_cashier(
        &self,
        cashier_id: i64,
        command: &JsonCommand,
    ) -> FineractResult<CommandProcessingResult> {
        let cashier = self
            .find_cashier_with_not_found_detection(cashier_id)
            .await?;
        let txn = CashierTransaction::from_json(cashier_id, CashierTxnType::Allocate, command)?;
        let saved = self.record_cashier_transaction(txn).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(cashier.id)
            .with_sub_entity_id(saved.id)
            .with_office_id(cashier.office_id)
            .build())
    }

    /// Settles cash from `cashier_id` back to the main vault (`TellerWritePlatformServiceJpaImpl.settleCashFromCashier`).
    pub async fn settle_cash_from_cashier(
        &self,
        cashier_id: i64,
        command: &JsonCommand,
    ) -> FineractResult<CommandProcessingResult> {
        let cashier = self
            .find_cashier_with_not_found_detection(cashier_id)
            .await?;
        let txn = CashierTransaction::from_json(cashier_id, CashierTxnType::Settle, command)?;
        let saved = self.record_cashier_transaction(txn).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(cashier.id)
            .with_sub_entity_id(saved.id)
            .with_office_id(cashier.office_id)
            .build())
    }
}

// =============================================================================
// 7. CQRS COMMAND HANDLERS
// =============================================================================

/// Handles `CREATE` `TELLER` commands (`CreateTellerCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct CreateTellerCommandHandler {
    repository: Arc<DualDbTellerRepository>,
}

impl CreateTellerCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "TELLER";

    pub fn new(repository: Arc<DualDbTellerRepository>) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &Arc<DualDbTellerRepository> {
        &self.repository
    }
}

#[async_trait]
impl CommandHandler for CreateTellerCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let teller = Teller::from_json(command)?;
        let saved = self.repository.save_teller(teller).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_office_id(saved.office_id)
            .build())
    }
}

/// Handles `UPDATE` `TELLER` commands (`UpdateTellerCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct UpdateTellerCommandHandler {
    repository: Arc<DualDbTellerRepository>,
}

impl UpdateTellerCommandHandler {
    pub const ACTION_NAME: &'static str = "UPDATE";
    pub const ENTITY_NAME: &'static str = "TELLER";

    pub fn new(repository: Arc<DualDbTellerRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl CommandHandler for UpdateTellerCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        TellerCommandFromApiJsonDeserializer::validate_for_create_and_update_teller(command)?;
        let teller_id = command.entity_id.unwrap_or_default();
        let mut teller = self
            .repository
            .find_teller_with_not_found_detection(teller_id)
            .await?;
        let changes = teller.update(command)?;
        if !changes.is_empty() {
            self.repository.save_teller(teller.clone()).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(teller.id)
            .with_office_id(teller.office_id)
            .with_changes(changes)
            .build())
    }
}

/// Handles `DELETE` `TELLER` commands (`DeleteTellerCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct DeleteTellerCommandHandler {
    repository: Arc<DualDbTellerRepository>,
}

impl DeleteTellerCommandHandler {
    pub const ACTION_NAME: &'static str = "DELETE";
    pub const ENTITY_NAME: &'static str = "TELLER";

    pub fn new(repository: Arc<DualDbTellerRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl CommandHandler for DeleteTellerCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let teller_id = command.entity_id.unwrap_or_default();
        self.repository.delete_teller(teller_id).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(teller_id)
            .build())
    }
}

/// Handles `ALLOCATECASHIER` `TELLER` commands (`AllocateCashierToTellerCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct AllocateCashierToTellerCommandHandler {
    repository: Arc<DualDbTellerRepository>,
}

impl AllocateCashierToTellerCommandHandler {
    pub const ACTION_NAME: &'static str = "ALLOCATECASHIER";
    pub const ENTITY_NAME: &'static str = "TELLER";

    pub fn new(repository: Arc<DualDbTellerRepository>) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &Arc<DualDbTellerRepository> {
        &self.repository
    }
}

#[async_trait]
impl CommandHandler for AllocateCashierToTellerCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let teller_id = match command.entity_id {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("tellerId")?
                .unwrap_or_default(),
        };
        let teller = self
            .repository
            .find_teller_with_not_found_detection(teller_id)
            .await?;
        let cashier = Cashier::from_json(teller.office_id, &teller, command)?;
        let saved = self.repository.allocate_cashier(cashier).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(teller.id)
            .with_sub_entity_id(saved.id)
            .with_office_id(teller.office_id)
            .build())
    }
}

/// Handles `UPDATECASHIERALLOCATION` `TELLER` commands (`UpdateCashierAllocationCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct UpdateCashierAllocationCommandHandler {
    repository: Arc<DualDbTellerRepository>,
}

impl UpdateCashierAllocationCommandHandler {
    pub const ACTION_NAME: &'static str = "UPDATECASHIERALLOCATION";
    pub const ENTITY_NAME: &'static str = "TELLER";

    pub fn new(repository: Arc<DualDbTellerRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl CommandHandler for UpdateCashierAllocationCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        TellerCommandFromApiJsonDeserializer::validate_for_allocate_cashier(command)?;
        let teller_id = command.entity_id.unwrap_or_default();
        let cashier_id = command.subresource_id.unwrap_or_default();
        let _teller = self
            .repository
            .find_teller_with_not_found_detection(teller_id)
            .await?;
        let mut cashier = self
            .repository
            .find_cashier_with_not_found_detection(cashier_id)
            .await?;
        let changes = cashier.update(command)?;
        if !changes.is_empty() {
            self.repository.allocate_cashier(cashier.clone()).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(cashier.teller_id)
            .with_sub_entity_id(cashier.id)
            .with_office_id(cashier.office_id)
            .with_changes(changes)
            .build())
    }
}

/// Handles `DELETECASHIERALLOCATION` `TELLER` commands (`DeleteCashierAllocationCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct DeleteCashierAllocationCommandHandler {
    repository: Arc<DualDbTellerRepository>,
}

impl DeleteCashierAllocationCommandHandler {
    pub const ACTION_NAME: &'static str = "DELETECASHIERALLOCATION";
    pub const ENTITY_NAME: &'static str = "TELLER";

    pub fn new(repository: Arc<DualDbTellerRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl CommandHandler for DeleteCashierAllocationCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let teller_id = command.entity_id.unwrap_or_default();
        let cashier_id = command.subresource_id.unwrap_or_default();
        self.repository.delete_cashier(teller_id, cashier_id).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(cashier_id)
            .build())
    }
}

/// Handles `ALLOCATECASHTOCASHIER` `TELLER` commands (`AllocateCashToCashierCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct AllocateCashToCashierCommandHandler {
    repository: Arc<DualDbTellerRepository>,
}

impl AllocateCashToCashierCommandHandler {
    pub const ACTION_NAME: &'static str = "ALLOCATECASHTOCASHIER";
    pub const ENTITY_NAME: &'static str = "TELLER";

    pub fn new(repository: Arc<DualDbTellerRepository>) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &Arc<DualDbTellerRepository> {
        &self.repository
    }
}

#[async_trait]
impl CommandHandler for AllocateCashToCashierCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let cashier_id = match command.subresource_id.or(command.entity_id) {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("cashierId")?
                .unwrap_or_default(),
        };
        self.repository
            .allocate_cash_to_cashier(cashier_id, command)
            .await
    }
}

/// Handles `SETTLECASHFROMCASHIER` `TELLER` commands (`SettleCashFromCashierCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct SettleCashFromCashierCommandHandler {
    repository: Arc<DualDbTellerRepository>,
}

impl SettleCashFromCashierCommandHandler {
    pub const ACTION_NAME: &'static str = "SETTLECASHFROMCASHIER";
    pub const ENTITY_NAME: &'static str = "TELLER";

    pub fn new(repository: Arc<DualDbTellerRepository>) -> Self {
        Self { repository }
    }

    pub fn repository(&self) -> &Arc<DualDbTellerRepository> {
        &self.repository
    }
}

#[async_trait]
impl CommandHandler for SettleCashFromCashierCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let cashier_id = match command.subresource_id.or(command.entity_id) {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("cashierId")?
                .unwrap_or_default(),
        };
        self.repository
            .settle_cash_from_cashier(cashier_id, command)
            .await
    }
}

// =============================================================================
// 8. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_teller() -> Teller {
        Teller::new(
            1,
            10,
            Some(1001),
            Some(1002),
            "Main Branch Teller 1",
            Some("Front desk teller".to_string()),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            TellerStatus::Active,
        )
    }

    fn part_day_cashier(
        id: i64,
        staff_id: i64,
        start_date: NaiveDate,
        end_date: NaiveDate,
        start_time: &str,
        end_time: &str,
    ) -> Cashier {
        Cashier::new(
            id,
            10,
            staff_id,
            1,
            Some("Shift cashier".to_string()),
            start_date,
            end_date,
            Some(start_time.to_string()),
            Some(end_time.to_string()),
            false,
        )
    }

    fn full_day_cashier(id: i64, staff_id: i64, start_date: NaiveDate, end_date: NaiveDate) -> Cashier {
        Cashier::new(
            id,
            10,
            staff_id,
            1,
            Some("Full day cashier".to_string()),
            start_date,
            end_date,
            Some(" ".to_string()),
            Some(" ".to_string()),
            true,
        )
    }

    #[test]
    fn test_teller_status_and_cashier_txn_type_discriminants() {
        assert_eq!(TellerStatus::Invalid.value(), 0);
        assert_eq!(TellerStatus::Pending.value(), 100);
        assert_eq!(TellerStatus::Active.value(), 300);
        assert_eq!(TellerStatus::Inactive.value(), 400);
        assert_eq!(TellerStatus::Closed.value(), 600);

        assert_eq!(TellerStatus::from_int(Some(100)), TellerStatus::Pending);
        assert_eq!(TellerStatus::from_int(Some(300)), TellerStatus::Active);
        assert_eq!(TellerStatus::from_int(Some(400)), TellerStatus::Inactive);
        assert_eq!(TellerStatus::from_int(Some(600)), TellerStatus::Closed);
        assert_eq!(TellerStatus::from_int(Some(999)), TellerStatus::Invalid);

        assert_eq!(CashierTxnType::Allocate.id(), 101);
        assert_eq!(CashierTxnType::Settle.id(), 102);
        assert_eq!(CashierTxnType::CashIn.id(), 103);
        assert_eq!(CashierTxnType::CashOut.id(), 104);
        assert_eq!(CashierTxnType::Allocate.value(), "Allocate Cash");
        assert_eq!(CashierTxnType::Settle.value(), "Settle Cash");
        assert_eq!(CashierTxnType::CashIn.value(), "Cash In");
        assert_eq!(CashierTxnType::CashOut.value(), "Cash Out");
    }

    #[test]
    fn test_teller_and_cashier_date_range_validation_rules() {
        let teller = sample_teller();
        let oct_1 = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let oct_31 = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();

        // Valid cashier within teller's [2026-01-01, 2026-12-31] window
        let valid_cashier = full_day_cashier(0, 1, oct_1, oct_31);
        assert!(valid_cashier.validate_allowed_date_and_time(&teller, &[]).is_ok());

        // Cashier starting before teller start_date -> CashierDateRangeOutOfTellerDateRangeException
        let before_teller = full_day_cashier(
            0,
            1,
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            oct_31,
        );
        let err_before = before_teller
            .validate_allowed_date_and_time(&teller, &[])
            .unwrap_err();
        match err_before {
            FineractError::DomainRule(ex) => {
                assert_eq!(
                    ex.globalisation_message_code,
                    CashierDateRangeOutOfTellerDateRangeException::CODE
                );
            }
            other => panic!("Expected DomainRule error, got: {other:?}"),
        }

        // Cashier ending after teller end_date -> CashierDateRangeOutOfTellerDateRangeException
        let after_teller = full_day_cashier(
            0,
            1,
            oct_1,
            NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
        );
        let err_after = after_teller
            .validate_allowed_date_and_time(&teller, &[])
            .unwrap_err();
        match err_after {
            FineractError::DomainRule(ex) => {
                assert_eq!(
                    ex.globalisation_message_code,
                    CashierDateRangeOutOfTellerDateRangeException::CODE
                );
            }
            other => panic!("Expected DomainRule error, got: {other:?}"),
        }
    }

    #[test]
    fn test_cashier_shift_overlap_and_part_day_time_rules() {
        let teller = sample_teller();
        let oct_1 = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let oct_31 = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();

        let existing_morning = part_day_cashier(1, 50, oct_1, oct_31, "09:00", "13:00");

        // 1. Overlapping part-day shift (12:00-17:00 overlaps 09:00-13:00) -> rejected
        let overlapping = part_day_cashier(0, 50, oct_1, oct_31, "12:00", "17:00");
        let err = overlapping
            .validate_allowed_date_and_time(&teller, std::slice::from_ref(&existing_morning))
            .unwrap_err();
        assert!(matches!(
            err,
            FineractError::DomainRule(ref ex) if ex.globalisation_message_code == CashierAlreadyAllocated::CODE
        ));

        // 2. Boundary-touching shift (13:00-17:00 shares 13:00 end/start time) -> rejected
        let boundary_touch = part_day_cashier(0, 50, oct_1, oct_31, "13:00", "17:00");
        assert!(boundary_touch
            .validate_allowed_date_and_time(&teller, std::slice::from_ref(&existing_morning))
            .is_err());

        // 3. Shift starting one minute after previous ends (13:01-17:00) -> allowed
        let non_overlapping = part_day_cashier(0, 50, oct_1, oct_31, "13:01", "17:00");
        assert!(non_overlapping
            .validate_allowed_date_and_time(&teller, std::slice::from_ref(&existing_morning))
            .is_ok());

        // 4. Updating the same cashier assignment (id = 1) does not conflict with itself
        let updated_self = part_day_cashier(1, 50, oct_1, oct_31, "10:00", "14:00");
        assert!(updated_self
            .validate_allowed_date_and_time(&teller, std::slice::from_ref(&existing_morning))
            .is_ok());

        // 5. Overnight / invalid part-day shift where end_time <= start_time (22:00 to 02:00) -> rejected
        let overnight = part_day_cashier(0, 50, oct_1, oct_31, "22:00", "02:00");
        let time_err = overnight
            .validate_allowed_date_and_time(&teller, &[])
            .unwrap_err();
        match time_err {
            FineractError::DataValidation(ex) => {
                assert_eq!(
                    ex.errors[0].user_message_globalisation_code,
                    "validation.msg.teller.hourEndTime.must.be.after.start.time"
                );
            }
            other => panic!("Expected DataValidation error, got: {other:?}"),
        }
    }

    #[test]
    fn test_cashier_summary_net_cash_math_and_insufficient_till_rejection() {
        let txn_date = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let now = Utc::now();

        let txns = vec![
            // Allocate 1,000.00
            CashierTransaction::new(
                1,
                7,
                CashierTxnType::Allocate,
                dec!(1000.00),
                txn_date,
                now,
                None,
                None,
                Some("Morning float".to_string()),
                "USD",
            ),
            // CashIn 350.50 (client deposit / loan repayment)
            CashierTransaction::new(
                2,
                7,
                CashierTxnType::CashIn,
                dec!(350.50),
                txn_date,
                now,
                Some("savings".to_string()),
                Some(101),
                Some("Savings deposit".to_string()),
                "USD",
            ),
            // CashOut 200.25 (client withdrawal / loan disbursal)
            CashierTransaction::new(
                3,
                7,
                CashierTxnType::CashOut,
                dec!(200.25),
                txn_date,
                now,
                Some("loans".to_string()),
                Some(202),
                Some("Loan disbursal".to_string()),
                "USD",
            ),
            // Settle 400.00 back to vault
            CashierTransaction::new(
                4,
                7,
                CashierTxnType::Settle,
                dec!(400.00),
                txn_date,
                now,
                None,
                None,
                Some("Midday vault drop".to_string()),
                "USD",
            ),
        ];

        let summary = CashierSummaryAndTransactions::from_transactions(
            7,
            1,
            Some("Head Office".to_string()),
            Some("Teller 1".to_string()),
            Some("Alice Cashier".to_string()),
            &txns,
            Some("USD"),
        );

        assert_eq!(summary.sum_cash_allocation(), dec!(1000.00));
        assert_eq!(summary.sum_inward_cash(), dec!(350.50));
        assert_eq!(summary.sum_outward_cash(), dec!(200.25));
        assert_eq!(summary.sum_cash_settlement(), dec!(400.00));
        // Net cash = 1000.00 + 350.50 - 400.00 - 200.25 = 750.25
        assert_eq!(summary.net_cash(), dec!(750.25));

        // Settling <= 750.25 succeeds
        assert!(summary.validate_settle_cash_and_cash_out(dec!(750.25)).is_ok());
        assert!(summary.ensure_sufficient_till_cash(dec!(750.25)).is_ok());

        // Settling 750.26 (> 750.25) fails with CashierInsufficientAmountException / NotEnoughCashInTheMainVault
        let err = summary
            .validate_settle_cash_and_cash_out(dec!(750.26))
            .unwrap_err();
        match err {
            FineractError::DomainRule(ex) => {
                assert_eq!(
                    ex.globalisation_message_code,
                    CashierInsufficientAmountException::CODE
                );
            }
            other => panic!("Expected DomainRule error, got: {other:?}"),
        }

        let vault_err = summary.ensure_sufficient_till_cash(dec!(800.00)).unwrap_err();
        assert_eq!(vault_err.requested_amount, Some(dec!(800.00)));
        assert_eq!(vault_err.available_amount, Some(dec!(750.25)));
    }

    #[tokio::test]
    async fn test_command_handlers_end_to_end_lifecycle_and_vault_settlement() {
        let repo = Arc::new(DualDbTellerRepository::in_memory());
        repo.set_main_vault_balance(1, "USD", dec!(5000.00));

        let ctx = ExecutionContext::new("default", NaiveDate::from_ymd_opt(2026, 10, 15).unwrap());

        let create_teller_handler = CreateTellerCommandHandler::new(Arc::clone(&repo));
        let allocate_cashier_handler = AllocateCashierToTellerCommandHandler::new(Arc::clone(&repo));
        let allocate_cash_handler = AllocateCashToCashierCommandHandler::new(Arc::clone(&repo));
        let settle_cash_handler = SettleCashFromCashierCommandHandler::new(Arc::clone(&repo));

        // 1. Create Teller
        let create_teller_cmd = JsonCommand::from_json(
            r#"{
                "officeId": 1,
                "name": "Teller-A",
                "description": "Main Hall Teller",
                "locale": "en",
                "dateFormat": "yyyy-MM-dd",
                "startDate": "2026-01-01",
                "endDate": "2026-12-31",
                "status": 300
            }"#,
        )
        .unwrap()
        .with_command_id(10);

        let teller_res = create_teller_handler
            .handle(&ctx, &create_teller_cmd)
            .await
            .unwrap();
        let teller_id = teller_res.resource_id.unwrap();
        assert_eq!(teller_id, 1);
        assert_eq!(teller_res.office_id, Some(1));

        // 2. Allocate Cashier to Teller
        let alloc_cashier_cmd = JsonCommand::from_json(
            r#"{
                "staffId": 42,
                "description": "Morning shift",
                "locale": "en",
                "dateFormat": "yyyy-MM-dd",
                "startDate": "2026-10-01",
                "endDate": "2026-10-31",
                "isFullDay": false,
                "hourStartTime": 8,
                "minStartTime": 0,
                "hourEndTime": 16,
                "minEndTime": 30
            }"#,
        )
        .unwrap()
        .with_entity_id(teller_id);

        let cashier_res = allocate_cashier_handler
            .handle(&ctx, &alloc_cashier_cmd)
            .await
            .unwrap();
        let cashier_id = cashier_res.sub_resource_id.unwrap();
        assert_eq!(cashier_id, 1);

        // 3. Allocate 1,200.00 USD from Main Vault to Cashier
        let alloc_cash_cmd = JsonCommand::from_json(
            r#"{
                "locale": "en",
                "dateFormat": "yyyy-MM-dd",
                "txnDate": "2026-10-15",
                "txnAmount": 1200.00,
                "txnNote": "Opening float",
                "currencyCode": "USD"
            }"#,
        )
        .unwrap()
        .with_entity_id(teller_id)
        .with_subresource_id(cashier_id);

        let alloc_txn_res = allocate_cash_handler
            .handle(&ctx, &alloc_cash_cmd)
            .await
            .unwrap();
        assert_eq!(alloc_txn_res.resource_id, Some(cashier_id));
        assert_eq!(repo.get_main_vault_balance(1, "USD"), Some(dec!(3800.00)));

        // 4. Attempting to settle 1,500.00 USD (> 1,200.00 in till) must fail
        let over_settle_cmd = JsonCommand::from_json(
            r#"{
                "locale": "en",
                "dateFormat": "yyyy-MM-dd",
                "txnDate": "2026-10-15",
                "txnAmount": 1500.00,
                "txnNote": "Excess settlement",
                "currencyCode": "USD"
            }"#,
        )
        .unwrap()
        .with_entity_id(teller_id)
        .with_subresource_id(cashier_id);

        let over_settle_err = settle_cash_handler
            .handle(&ctx, &over_settle_cmd)
            .await
            .unwrap_err();
        assert!(matches!(
            over_settle_err,
            FineractError::DomainRule(ref ex) if ex.globalisation_message_code == CashierInsufficientAmountException::CODE
        ));

        // 5. Settling 500.00 USD succeeds and leaves 700.00 USD in till (and restores 500.00 to main vault)
        let valid_settle_cmd = JsonCommand::from_json(
            r#"{
                "locale": "en",
                "dateFormat": "yyyy-MM-dd",
                "txnDate": "2026-10-15",
                "txnAmount": 500.00,
                "txnNote": "End of shift partial settle",
                "currencyCode": "USD"
            }"#,
        )
        .unwrap()
        .with_entity_id(teller_id)
        .with_subresource_id(cashier_id);

        settle_cash_handler
            .handle(&ctx, &valid_settle_cmd)
            .await
            .unwrap();

        let summary = repo
            .retrieve_cashier_transactions_with_summary(cashier_id, Some("USD"), None)
            .await
            .unwrap();
        assert_eq!(summary.sum_cash_allocation(), dec!(1200.00));
        assert_eq!(summary.sum_cash_settlement(), dec!(500.00));
        assert_eq!(summary.net_cash(), dec!(700.00));
        assert_eq!(repo.get_main_vault_balance(1, "USD"), Some(dec!(4300.00)));
    }

    #[test]
    fn test_dual_db_sql_placeholder_rewriting() {
        let pg_insert = DualDbTellerRepository::insert_teller_sql(SqlDialect::Postgres);
        assert!(pg_insert.contains("$1") && pg_insert.contains("$9"));
        assert!(!pg_insert.contains('?'));

        let mysql_insert = DualDbTellerRepository::insert_teller_sql(SqlDialect::MySql);
        assert!(mysql_insert.contains('?'));
        assert!(!mysql_insert.contains("$1"));
    }
}
