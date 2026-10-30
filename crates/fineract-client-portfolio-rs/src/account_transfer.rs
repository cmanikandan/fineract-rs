//! Account Transfers, Portfolio Account Associations, and Standing Instructions.
//!
//! Ported from `org.apache.fineract.portfolio.account`:
//! - `PortfolioAccountType` (`PortfolioAccountType.java`)
//! - `AccountTransferType` (`AccountTransferType.java`)
//! - `StandingInstructionType` (`StandingInstructionType.java`)
//! - `StandingInstructionPriority` (`StandingInstructionPriority.java`)
//! - `StandingInstructionStatus` (`StandingInstructionStatus.java`)
//! - `AccountAssociationType` (`AccountAssociationType.java`)
//! - `AccountTransferRecurrenceType` (`AccountTransferRecurrenceType.java`)
//! - `AccountAssociations` (`AccountAssociations.java` -> `m_portfolio_account_associations`)
//! - `AccountTransferDetails` (`AccountTransferDetails.java` -> `m_account_transfer_details`)
//! - `AccountTransferTransaction` (`AccountTransferTransaction.java` -> `m_account_transfer_transaction`)
//! - `AccountTransferStandingInstruction` (`AccountTransferStandingInstruction.java` -> `m_account_transfer_standing_instructions`)
//! - `StandingInstructionExecutionHistory` (`m_account_transfer_standing_instructions_history`)
//! - `AccountTransferValidatorAndEngine` (`AccountTransfersDetailDataValidator.java`,
//!   `AccountTransfersDataValidator.java`, `StandingInstructionDataValidator.java`,
//!   `AccountTransfersWritePlatformServiceImpl.java`, `ExecuteStandingInstructionsTasklet.java`)
//! - `DualDbAccountTransferRepository` and command handlers

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Datelike, Months, NaiveDate, NaiveDateTime};
use dashmap::DashMap;
use fineract_command_rs::pipeline::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::calendar::{days_in_year_month, CalendarFrequencyType};
use fineract_core_rs::context::ExecutionContext;
use fineract_core_rs::db::{DialectSql, SqlDialect};
use fineract_core_rs::json_command::JsonCommand;
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{
    ApiErrorArg, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException, PlatformDomainRuleException,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{AnyPool, Row};

// =============================================================================
// API Parameter & Resource Constants (AccountDetailConstants / AccountTransfersApiConstants / StandingInstructionApiConstants)
// =============================================================================

pub const ACCOUNT_TRANSFER_RESOURCE_NAME: &str = "accounttransfer";
pub const STANDING_INSTRUCTION_RESOURCE_NAME: &str = "standinginstruction";

pub const PARAM_LOCALE: &str = "locale";
pub const PARAM_DATE_FORMAT: &str = "dateFormat";
pub const PARAM_FROM_OFFICE_ID: &str = "fromOfficeId";
pub const PARAM_FROM_CLIENT_ID: &str = "fromClientId";
pub const PARAM_FROM_ACCOUNT_TYPE: &str = "fromAccountType";
pub const PARAM_FROM_ACCOUNT_ID: &str = "fromAccountId";
pub const PARAM_TO_OFFICE_ID: &str = "toOfficeId";
pub const PARAM_TO_CLIENT_ID: &str = "toClientId";
pub const PARAM_TO_ACCOUNT_TYPE: &str = "toAccountType";
pub const PARAM_TO_ACCOUNT_ID: &str = "toAccountId";
pub const PARAM_TRANSFER_TYPE: &str = "transferType";

pub const PARAM_TRANSFER_AMOUNT: &str = "transferAmount";
pub const PARAM_TRANSFER_DATE: &str = "transferDate";
pub const PARAM_TRANSFER_DESCRIPTION: &str = "transferDescription";
pub const PARAM_PAYMENT_TYPE_ID: &str = "paymentTypeId";
pub const PARAM_ACCOUNT_NUMBER: &str = "accountNumber";
pub const PARAM_CHECK_NUMBER: &str = "checkNumber";
pub const PARAM_ROUTING_CODE: &str = "routingCode";
pub const PARAM_RECEIPT_NUMBER: &str = "receiptNumber";
pub const PARAM_BANK_NUMBER: &str = "bankNumber";

pub const PARAM_NAME: &str = "name";
pub const PARAM_PRIORITY: &str = "priority";
pub const PARAM_INSTRUCTION_TYPE: &str = "instructionType";
pub const PARAM_STATUS: &str = "status";
pub const PARAM_AMOUNT: &str = "amount";
pub const PARAM_VALID_FROM: &str = "validFrom";
pub const PARAM_VALID_TILL: &str = "validTill";
pub const PARAM_RECURRENCE_TYPE: &str = "recurrenceType";
pub const PARAM_RECURRENCE_FREQUENCY: &str = "recurrenceFrequency";
pub const PARAM_RECURRENCE_INTERVAL: &str = "recurrenceInterval";
pub const PARAM_RECURRENCE_ON_MONTH_DAY: &str = "recurrenceOnMonthDay";
pub const PARAM_MONTH_DAY_FORMAT: &str = "monthDayFormat";

pub const CREATE_ACCOUNT_TRANSFER_REQUEST_PARAMS: &[&str] = &[
    PARAM_LOCALE,
    PARAM_DATE_FORMAT,
    PARAM_FROM_OFFICE_ID,
    PARAM_FROM_CLIENT_ID,
    PARAM_FROM_ACCOUNT_TYPE,
    PARAM_FROM_ACCOUNT_ID,
    PARAM_TO_OFFICE_ID,
    PARAM_TO_CLIENT_ID,
    PARAM_TO_ACCOUNT_TYPE,
    PARAM_TO_ACCOUNT_ID,
    PARAM_TRANSFER_DATE,
    PARAM_TRANSFER_AMOUNT,
    PARAM_TRANSFER_DESCRIPTION,
];

pub const REFUND_BY_TRANSFER_REQUEST_PARAMS: &[&str] = &[
    PARAM_LOCALE,
    PARAM_DATE_FORMAT,
    PARAM_FROM_OFFICE_ID,
    PARAM_FROM_CLIENT_ID,
    PARAM_FROM_ACCOUNT_TYPE,
    PARAM_FROM_ACCOUNT_ID,
    PARAM_TO_OFFICE_ID,
    PARAM_TO_CLIENT_ID,
    PARAM_TO_ACCOUNT_TYPE,
    PARAM_TO_ACCOUNT_ID,
    PARAM_TRANSFER_DATE,
    PARAM_TRANSFER_AMOUNT,
    PARAM_TRANSFER_DESCRIPTION,
    PARAM_PAYMENT_TYPE_ID,
    PARAM_ACCOUNT_NUMBER,
    PARAM_CHECK_NUMBER,
    PARAM_ROUTING_CODE,
    PARAM_RECEIPT_NUMBER,
    PARAM_BANK_NUMBER,
];

pub const CREATE_STANDING_INSTRUCTION_REQUEST_PARAMS: &[&str] = &[
    PARAM_LOCALE,
    PARAM_DATE_FORMAT,
    PARAM_FROM_OFFICE_ID,
    PARAM_FROM_CLIENT_ID,
    PARAM_FROM_ACCOUNT_TYPE,
    PARAM_FROM_ACCOUNT_ID,
    PARAM_TO_OFFICE_ID,
    PARAM_TO_CLIENT_ID,
    PARAM_TO_ACCOUNT_TYPE,
    PARAM_TO_ACCOUNT_ID,
    PARAM_NAME,
    PARAM_PRIORITY,
    PARAM_INSTRUCTION_TYPE,
    PARAM_STATUS,
    PARAM_AMOUNT,
    PARAM_VALID_FROM,
    PARAM_VALID_TILL,
    PARAM_RECURRENCE_TYPE,
    PARAM_RECURRENCE_FREQUENCY,
    PARAM_RECURRENCE_INTERVAL,
    PARAM_RECURRENCE_ON_MONTH_DAY,
    PARAM_TRANSFER_TYPE,
    PARAM_MONTH_DAY_FORMAT,
];

pub const UPDATE_STANDING_INSTRUCTION_REQUEST_PARAMS: &[&str] = &[
    PARAM_LOCALE,
    PARAM_DATE_FORMAT,
    PARAM_PRIORITY,
    PARAM_INSTRUCTION_TYPE,
    PARAM_STATUS,
    PARAM_AMOUNT,
    PARAM_VALID_FROM,
    PARAM_VALID_TILL,
    PARAM_RECURRENCE_TYPE,
    PARAM_RECURRENCE_FREQUENCY,
    PARAM_RECURRENCE_INTERVAL,
    PARAM_RECURRENCE_ON_MONTH_DAY,
    PARAM_MONTH_DAY_FORMAT,
];

/// Enum option metadata matching Fineract `EnumOptionData` for account transfer enums.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTransferEnumOptionData {
    pub id: i64,
    pub code: String,
    pub value: String,
}

impl AccountTransferEnumOptionData {
    pub fn new(id: i64, code: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            id,
            code: code.into(),
            value: value.into(),
        }
    }
}

// =============================================================================
// 1. PortfolioAccountType
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.PortfolioAccountType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum PortfolioAccountType {
    Invalid = 0,
    Loan = 1,
    Savings = 2,
}

impl PortfolioAccountType {
    pub const INVALID: Self = Self::Invalid;
    pub const LOAN: Self = Self::Loan;
    pub const SAVINGS: Self = Self::Savings;

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Loan,
            2 => Self::Savings,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Loan,
            Some(2) => Self::Savings,
            _ => Self::Invalid,
        }
    }

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "accountType.invalid",
            Self::Loan => "accountType.loan",
            Self::Savings => "accountType.savings",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Loan => "Loan Account",
            Self::Savings => "Savings Account",
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_loan_account(self) -> bool {
        matches!(self, Self::Loan)
    }

    pub const fn is_savings_account(self) -> bool {
        matches!(self, Self::Savings)
    }

    pub fn to_option_data(self) -> AccountTransferEnumOptionData {
        AccountTransferEnumOptionData::new(self.as_i16() as i64, self.code(), self.label())
    }
}

impl Default for PortfolioAccountType {
    fn default() -> Self {
        Self::Invalid
    }
}

// =============================================================================
// 2. AccountTransferType
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.AccountTransferType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum AccountTransferType {
    Invalid = 0,
    AccountTransfer = 1,
    LoanRepayment = 2,
    ChargePayment = 3,
    InterestTransfer = 4,
}

impl AccountTransferType {
    pub const INVALID: Self = Self::Invalid;
    pub const ACCOUNT_TRANSFER: Self = Self::AccountTransfer;
    pub const LOAN_REPAYMENT: Self = Self::LoanRepayment;
    pub const CHARGE_PAYMENT: Self = Self::ChargePayment;
    pub const INTEREST_TRANSFER: Self = Self::InterestTransfer;

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::AccountTransfer,
            2 => Self::LoanRepayment,
            3 => Self::ChargePayment,
            4 => Self::InterestTransfer,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::AccountTransfer,
            Some(2) => Self::LoanRepayment,
            Some(3) => Self::ChargePayment,
            Some(4) => Self::InterestTransfer,
            _ => Self::Invalid,
        }
    }

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "accountTransferType.invalid",
            Self::AccountTransfer => "accountTransferType.account.transfer",
            Self::LoanRepayment => "accountTransferType.loan.repayment",
            Self::ChargePayment => "accountTransferType.charge.payment",
            Self::InterestTransfer => "accountTransferType.interest.transfer",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::AccountTransfer => "Account Transfer",
            Self::LoanRepayment => "Loan Repayment",
            Self::ChargePayment => "Charge Payment",
            Self::InterestTransfer => "Interest Transfer",
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_account_transfer(self) -> bool {
        matches!(self, Self::AccountTransfer)
    }

    pub const fn is_loan_repayment(self) -> bool {
        matches!(self, Self::LoanRepayment)
    }

    pub const fn is_charge_payment(self) -> bool {
        matches!(self, Self::ChargePayment)
    }

    pub const fn is_interest_transfer(self) -> bool {
        matches!(self, Self::InterestTransfer)
    }

    pub fn to_option_data(self) -> AccountTransferEnumOptionData {
        AccountTransferEnumOptionData::new(self.as_i16() as i64, self.code(), self.label())
    }
}

impl Default for AccountTransferType {
    fn default() -> Self {
        Self::Invalid
    }
}

// =============================================================================
// 3. StandingInstructionType
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.StandingInstructionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum StandingInstructionType {
    Invalid = 0,
    Fixed = 1,
    Dues = 2,
}

impl StandingInstructionType {
    pub const INVALID: Self = Self::Invalid;
    pub const FIXED: Self = Self::Fixed;
    pub const DUES: Self = Self::Dues;

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Fixed,
            2 => Self::Dues,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Fixed,
            Some(2) => Self::Dues,
            _ => Self::Invalid,
        }
    }

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "standingInstructionType.invalid",
            Self::Fixed => "standingInstructionType.fixed",
            Self::Dues => "standingInstructionType.dues",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Fixed => "Fixed",
            Self::Dues => "Dues",
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_fixed_amount_transfer(self) -> bool {
        matches!(self, Self::Fixed)
    }

    pub const fn is_dues_amount_transfer(self) -> bool {
        matches!(self, Self::Dues)
    }

    pub fn to_option_data(self) -> AccountTransferEnumOptionData {
        AccountTransferEnumOptionData::new(self.as_i16() as i64, self.code(), self.label())
    }
}

impl Default for StandingInstructionType {
    fn default() -> Self {
        Self::Invalid
    }
}

// =============================================================================
// 4. StandingInstructionPriority
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.StandingInstructionPriority`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum StandingInstructionPriority {
    Invalid = 0,
    Urgent = 1,
    High = 2,
    Medium = 3,
    Low = 4,
}

impl StandingInstructionPriority {
    pub const INVALID: Self = Self::Invalid;
    pub const URGENT: Self = Self::Urgent;
    pub const HIGH: Self = Self::High;
    pub const MEDIUM: Self = Self::Medium;
    pub const LOW: Self = Self::Low;

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Urgent,
            2 => Self::High,
            3 => Self::Medium,
            4 => Self::Low,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Urgent,
            Some(2) => Self::High,
            Some(3) => Self::Medium,
            Some(4) => Self::Low,
            _ => Self::Invalid,
        }
    }

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "standingInstructionPriority.invalid",
            Self::Urgent => "standingInstructionPriority.urgent",
            Self::High => "standingInstructionPriority.high",
            Self::Medium => "standingInstructionPriority.medium",
            Self::Low => "standingInstructionPriority.low",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Urgent => "Urgent Priority",
            Self::High => "High Priority",
            Self::Medium => "Medium Priority",
            Self::Low => "Low Priority",
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_urgent_priority(self) -> bool {
        matches!(self, Self::Urgent)
    }

    pub const fn is_high_priority(self) -> bool {
        matches!(self, Self::High)
    }

    pub const fn is_medium_priority(self) -> bool {
        matches!(self, Self::Medium)
    }

    pub const fn is_low_priority(self) -> bool {
        matches!(self, Self::Low)
    }

    pub fn to_option_data(self) -> AccountTransferEnumOptionData {
        AccountTransferEnumOptionData::new(self.as_i16() as i64, self.code(), self.label())
    }
}

impl Default for StandingInstructionPriority {
    fn default() -> Self {
        Self::Invalid
    }
}

// =============================================================================
// 5. StandingInstructionStatus
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.StandingInstructionStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum StandingInstructionStatus {
    Invalid = 0,
    Active = 1,
    Disabled = 2,
    Deleted = 3,
}

impl StandingInstructionStatus {
    pub const INVALID: Self = Self::Invalid;
    pub const ACTIVE: Self = Self::Active;
    pub const DISABLED: Self = Self::Disabled;
    pub const DELETED: Self = Self::Deleted;

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Active,
            2 => Self::Disabled,
            3 => Self::Deleted,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Active,
            Some(2) => Self::Disabled,
            Some(3) => Self::Deleted,
            _ => Self::Invalid,
        }
    }

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "standingInstructionStatus.invalid",
            Self::Active => "standingInstructionStatus.active",
            Self::Disabled => "standingInstructionStatus.disabled",
            Self::Deleted => "standingInstructionStatus.deleted",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Active => "Active",
            Self::Disabled => "Disabled",
            Self::Deleted => "Deleted",
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub const fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub const fn is_deleted(self) -> bool {
        matches!(self, Self::Deleted)
    }

    pub fn to_option_data(self) -> AccountTransferEnumOptionData {
        AccountTransferEnumOptionData::new(self.as_i16() as i64, self.code(), self.label())
    }
}

impl Default for StandingInstructionStatus {
    fn default() -> Self {
        Self::Invalid
    }
}

// =============================================================================
// 6. AccountAssociationType
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.AccountAssociationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum AccountAssociationType {
    Invalid = 0,
    LinkedAccount = 1,
    GuarantorAccount = 2,
}

impl AccountAssociationType {
    pub const INVALID: Self = Self::Invalid;
    pub const LINKED_ACCOUNT: Self = Self::LinkedAccount;
    pub const GUARANTOR_ACCOUNT: Self = Self::GuarantorAccount;

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::LinkedAccount,
            2 => Self::GuarantorAccount,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::LinkedAccount,
            Some(2) => Self::GuarantorAccount,
            _ => Self::Invalid,
        }
    }

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "accountAssociationType.invalid",
            Self::LinkedAccount => "accountAssociationType.linked.account.association",
            Self::GuarantorAccount => "accountAssociationType.guarantor.account.association",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::LinkedAccount => "Linked Account Association",
            Self::GuarantorAccount => "Guarantor Account Association",
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_linked_account_association(self) -> bool {
        matches!(self, Self::LinkedAccount)
    }

    pub const fn is_guarantor_account_association(self) -> bool {
        matches!(self, Self::GuarantorAccount)
    }

    pub fn to_option_data(self) -> AccountTransferEnumOptionData {
        AccountTransferEnumOptionData::new(self.as_i16() as i64, self.code(), self.label())
    }
}

impl Default for AccountAssociationType {
    fn default() -> Self {
        Self::Invalid
    }
}

// =============================================================================
// 7. AccountTransferRecurrenceType
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.AccountTransferRecurrenceType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum AccountTransferRecurrenceType {
    Invalid = 0,
    Periodic = 1,
    AsPerDues = 2,
}

impl AccountTransferRecurrenceType {
    pub const INVALID: Self = Self::Invalid;
    pub const PERIODIC: Self = Self::Periodic;
    pub const AS_PER_DUES: Self = Self::AsPerDues;

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Periodic,
            2 => Self::AsPerDues,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Periodic,
            Some(2) => Self::AsPerDues,
            _ => Self::Invalid,
        }
    }

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "accountTransferRecurrenceType.invalid",
            Self::Periodic => "accountTransferRecurrenceType.periodic",
            Self::AsPerDues => "accountTransferRecurrenceType.as.per.dues",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Periodic => "Periodic Recurrence",
            Self::AsPerDues => "As Per Dues Recurrence",
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_periodic_recurrence(self) -> bool {
        matches!(self, Self::Periodic)
    }

    pub const fn is_dues_recurrence(self) -> bool {
        matches!(self, Self::AsPerDues)
    }

    pub fn to_option_data(self) -> AccountTransferEnumOptionData {
        AccountTransferEnumOptionData::new(self.as_i16() as i64, self.code(), self.label())
    }
}

impl Default for AccountTransferRecurrenceType {
    fn default() -> Self {
        Self::Invalid
    }
}

// =============================================================================
// 8. AccountAssociations (`m_portfolio_account_associations`)
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.AccountAssociations`.
///
/// Maps to `m_portfolio_account_associations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountAssociations {
    pub id: i64,
    pub loan_account_id: Option<i64>,
    pub savings_account_id: Option<i64>,
    pub linked_loan_account_id: Option<i64>,
    pub linked_savings_account_id: Option<i64>,
    pub association_type: AccountAssociationType,
    pub is_active: bool,
}

impl AccountAssociations {
    pub const fn new(
        id: i64,
        loan_account_id: Option<i64>,
        savings_account_id: Option<i64>,
        linked_loan_account_id: Option<i64>,
        linked_savings_account_id: Option<i64>,
        association_type: AccountAssociationType,
        is_active: bool,
    ) -> Self {
        Self {
            id,
            loan_account_id,
            savings_account_id,
            linked_loan_account_id,
            linked_savings_account_id,
            association_type,
            is_active,
        }
    }

    /// Associates a loan account with a linked savings account (`AccountAssociations.associateSavingsAccount`).
    pub const fn associate_savings_account_to_loan(
        id: i64,
        loan_account_id: i64,
        linked_savings_account_id: i64,
        association_type: AccountAssociationType,
        is_active: bool,
    ) -> Self {
        Self::new(
            id,
            Some(loan_account_id),
            None,
            None,
            Some(linked_savings_account_id),
            association_type,
            is_active,
        )
    }

    /// Associates a savings account with a linked savings account (`AccountAssociations.associateSavingsAccount`).
    pub const fn associate_savings_account_to_savings(
        id: i64,
        savings_account_id: i64,
        linked_savings_account_id: i64,
        association_type: AccountAssociationType,
        is_active: bool,
    ) -> Self {
        Self::new(
            id,
            None,
            Some(savings_account_id),
            None,
            Some(linked_savings_account_id),
            association_type,
            is_active,
        )
    }

    /// Updates the linked savings account ID (`AccountAssociations.updateLinkedSavingsAccount`).
    pub fn update_linked_savings_account(&mut self, linked_savings_account_id: Option<i64>) {
        self.linked_savings_account_id = linked_savings_account_id;
    }

    /// Deactivates the association.
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Activates the association.
    pub fn activate(&mut self) {
        self.is_active = true;
    }
}

// =============================================================================
// 9. AccountTransferDetails (`m_account_transfer_details`)
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.AccountTransferDetails`.
///
/// Maps to `m_account_transfer_details`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTransferDetails {
    pub id: i64,
    pub from_office_id: i64,
    pub to_office_id: i64,
    pub from_client_id: i64,
    pub to_client_id: i64,
    pub from_savings_account_id: Option<i64>,
    pub to_savings_account_id: Option<i64>,
    pub from_loan_account_id: Option<i64>,
    pub to_loan_account_id: Option<i64>,
    pub transfer_type: AccountTransferType,
}

impl AccountTransferDetails {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: i64,
        from_office_id: i64,
        to_office_id: i64,
        from_client_id: i64,
        to_client_id: i64,
        from_savings_account_id: Option<i64>,
        to_savings_account_id: Option<i64>,
        from_loan_account_id: Option<i64>,
        to_loan_account_id: Option<i64>,
        transfer_type: AccountTransferType,
    ) -> Self {
        Self {
            id,
            from_office_id,
            to_office_id,
            from_client_id,
            to_client_id,
            from_savings_account_id,
            to_savings_account_id,
            from_loan_account_id,
            to_loan_account_id,
            transfer_type,
        }
    }

    /// Port of `AccountTransferDetails.savingsToSavingsTransfer(...)`.
    pub const fn savings_to_savings_transfer(
        id: i64,
        from_office_id: i64,
        from_client_id: i64,
        from_savings_account_id: i64,
        to_office_id: i64,
        to_client_id: i64,
        to_savings_account_id: i64,
        transfer_type: AccountTransferType,
    ) -> Self {
        Self::new(
            id,
            from_office_id,
            to_office_id,
            from_client_id,
            to_client_id,
            Some(from_savings_account_id),
            Some(to_savings_account_id),
            None,
            None,
            transfer_type,
        )
    }

    /// Port of `AccountTransferDetails.savingsToLoanTransfer(...)`.
    pub const fn savings_to_loan_transfer(
        id: i64,
        from_office_id: i64,
        from_client_id: i64,
        from_savings_account_id: i64,
        to_office_id: i64,
        to_client_id: i64,
        to_loan_account_id: i64,
        transfer_type: AccountTransferType,
    ) -> Self {
        Self::new(
            id,
            from_office_id,
            to_office_id,
            from_client_id,
            to_client_id,
            Some(from_savings_account_id),
            None,
            None,
            Some(to_loan_account_id),
            transfer_type,
        )
    }

    /// Port of `AccountTransferDetails.loanToSavingsTransfer(...)`.
    pub const fn loan_to_savings_transfer(
        id: i64,
        from_office_id: i64,
        from_client_id: i64,
        from_loan_account_id: i64,
        to_office_id: i64,
        to_client_id: i64,
        to_savings_account_id: i64,
        transfer_type: AccountTransferType,
    ) -> Self {
        Self::new(
            id,
            from_office_id,
            to_office_id,
            from_client_id,
            to_client_id,
            None,
            Some(to_savings_account_id),
            Some(from_loan_account_id),
            None,
            transfer_type,
        )
    }

    /// Port of `AccountTransferDetails.LoanToLoanTransfer(...)`.
    pub const fn loan_to_loan_transfer(
        id: i64,
        from_office_id: i64,
        from_client_id: i64,
        from_loan_account_id: i64,
        to_office_id: i64,
        to_client_id: i64,
        to_loan_account_id: i64,
        transfer_type: AccountTransferType,
    ) -> Self {
        Self::new(
            id,
            from_office_id,
            to_office_id,
            from_client_id,
            to_client_id,
            None,
            None,
            Some(from_loan_account_id),
            Some(to_loan_account_id),
            transfer_type,
        )
    }

    /// Returns the source [`PortfolioAccountType`].
    pub const fn from_account_type(&self) -> PortfolioAccountType {
        if self.from_savings_account_id.is_some() {
            PortfolioAccountType::Savings
        } else if self.from_loan_account_id.is_some() {
            PortfolioAccountType::Loan
        } else {
            PortfolioAccountType::Invalid
        }
    }

    /// Returns the destination [`PortfolioAccountType`].
    pub const fn to_account_type(&self) -> PortfolioAccountType {
        if self.to_savings_account_id.is_some() {
            PortfolioAccountType::Savings
        } else if self.to_loan_account_id.is_some() {
            PortfolioAccountType::Loan
        } else {
            PortfolioAccountType::Invalid
        }
    }

    /// Returns the source account ID.
    pub const fn from_account_id(&self) -> Option<i64> {
        if self.from_savings_account_id.is_some() {
            self.from_savings_account_id
        } else {
            self.from_loan_account_id
        }
    }

    /// Returns the destination account ID.
    pub const fn to_account_id(&self) -> Option<i64> {
        if self.to_savings_account_id.is_some() {
            self.to_savings_account_id
        } else {
            self.to_loan_account_id
        }
    }
}

// =============================================================================
// 10. AccountTransferTransaction (`m_account_transfer_transaction`)
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.AccountTransferTransaction`.
///
/// Maps to `m_account_transfer_transaction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTransferTransaction {
    pub id: i64,
    pub account_transfer_details_id: i64,
    pub from_savings_transaction_id: Option<i64>,
    pub to_savings_transaction_id: Option<i64>,
    pub to_loan_transaction_id: Option<i64>,
    pub from_loan_transaction_id: Option<i64>,
    pub is_reversed: bool,
    pub transaction_date: NaiveDate,
    pub currency_code: String,
    pub amount: Decimal,
    pub description: String,
}

impl AccountTransferTransaction {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        account_transfer_details_id: i64,
        from_savings_transaction_id: Option<i64>,
        to_savings_transaction_id: Option<i64>,
        to_loan_transaction_id: Option<i64>,
        from_loan_transaction_id: Option<i64>,
        is_reversed: bool,
        transaction_date: NaiveDate,
        currency_code: impl Into<String>,
        amount: Decimal,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id,
            account_transfer_details_id,
            from_savings_transaction_id,
            to_savings_transaction_id,
            to_loan_transaction_id,
            from_loan_transaction_id,
            is_reversed,
            transaction_date,
            currency_code: currency_code.into(),
            amount,
            description: description.into(),
        }
    }

    /// Port of `AccountTransferTransaction.savingsToSavingsTransfer(...)`.
    #[allow(clippy::too_many_arguments)]
    pub fn savings_to_savings_transfer(
        id: i64,
        account_transfer_details_id: i64,
        withdrawal_savings_txn_id: i64,
        deposit_savings_txn_id: i64,
        transaction_date: NaiveDate,
        currency_code: impl Into<String>,
        amount: Decimal,
        description: impl Into<String>,
    ) -> Self {
        Self::new(
            id,
            account_transfer_details_id,
            Some(withdrawal_savings_txn_id),
            Some(deposit_savings_txn_id),
            None,
            None,
            false,
            transaction_date,
            currency_code,
            amount,
            description,
        )
    }

    /// Port of `AccountTransferTransaction.savingsToLoanTransfer(...)`.
    #[allow(clippy::too_many_arguments)]
    pub fn savings_to_loan_transfer(
        id: i64,
        account_transfer_details_id: i64,
        withdrawal_savings_txn_id: i64,
        repayment_loan_txn_id: i64,
        transaction_date: NaiveDate,
        currency_code: impl Into<String>,
        amount: Decimal,
        description: impl Into<String>,
    ) -> Self {
        Self::new(
            id,
            account_transfer_details_id,
            Some(withdrawal_savings_txn_id),
            None,
            Some(repayment_loan_txn_id),
            None,
            false,
            transaction_date,
            currency_code,
            amount,
            description,
        )
    }

    /// Port of `AccountTransferTransaction.loanToSavingsTransfer(...)`.
    #[allow(clippy::too_many_arguments)]
    pub fn loan_to_savings_transfer(
        id: i64,
        account_transfer_details_id: i64,
        deposit_savings_txn_id: i64,
        loan_refund_txn_id: i64,
        transaction_date: NaiveDate,
        currency_code: impl Into<String>,
        amount: Decimal,
        description: impl Into<String>,
    ) -> Self {
        Self::new(
            id,
            account_transfer_details_id,
            None,
            Some(deposit_savings_txn_id),
            None,
            Some(loan_refund_txn_id),
            false,
            transaction_date,
            currency_code,
            amount,
            description,
        )
    }

    /// Port of `AccountTransferTransaction.LoanToLoanTransfer(...)`.
    #[allow(clippy::too_many_arguments)]
    pub fn loan_to_loan_transfer(
        id: i64,
        account_transfer_details_id: i64,
        disburse_loan_txn_id: i64,
        repayment_loan_txn_id: i64,
        transaction_date: NaiveDate,
        currency_code: impl Into<String>,
        amount: Decimal,
        description: impl Into<String>,
    ) -> Self {
        Self::new(
            id,
            account_transfer_details_id,
            None,
            None,
            Some(repayment_loan_txn_id),
            Some(disburse_loan_txn_id),
            false,
            transaction_date,
            currency_code,
            amount,
            description,
        )
    }

    /// Marks this transfer transaction as reversed (`AccountTransferTransaction.reverse()`).
    pub fn reverse(&mut self) {
        self.is_reversed = true;
    }

    /// Updates the destination loan transaction reference (`AccountTransferTransaction.updateToLoanTransaction`).
    pub fn update_to_loan_transaction(&mut self, to_loan_transaction_id: Option<i64>) {
        self.to_loan_transaction_id = to_loan_transaction_id;
    }

    /// Returns the amount wrapped in a [`Money`] instance for the given digits after decimal.
    pub fn monetary_amount(&self, digits_after_decimal: u32) -> Money {
        let currency = MonetaryCurrency::with_code(self.currency_code.clone(), digits_after_decimal);
        Money::of(currency, self.amount, FineractRoundingMode::HalfEven)
    }
}

// =============================================================================
// 11. AccountTransferStandingInstruction (`m_account_transfer_standing_instructions`)
// =============================================================================

/// Ported from `org.apache.fineract.portfolio.account.domain.AccountTransferStandingInstruction`.
///
/// Maps to `m_account_transfer_standing_instructions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTransferStandingInstruction {
    pub id: i64,
    pub name: String,
    pub account_transfer_details_id: i64,
    pub priority: StandingInstructionPriority,
    pub instruction_type: StandingInstructionType,
    pub status: StandingInstructionStatus,
    pub amount: Option<Decimal>,
    pub valid_from: NaiveDate,
    pub valid_till: Option<NaiveDate>,
    pub recurrence_type: i16,
    pub recurrence_frequency: Option<i16>,
    pub recurrence_interval: Option<i16>,
    pub recurrence_on_month: Option<i16>,
    pub recurrence_on_day: Option<i16>,
    pub last_run_date: Option<NaiveDate>,
}

impl AccountTransferStandingInstruction {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        name: impl Into<String>,
        account_transfer_details_id: i64,
        priority: StandingInstructionPriority,
        instruction_type: StandingInstructionType,
        status: StandingInstructionStatus,
        amount: Option<Decimal>,
        valid_from: NaiveDate,
        valid_till: Option<NaiveDate>,
        recurrence_type: i16,
        recurrence_frequency: Option<i16>,
        recurrence_interval: Option<i16>,
        recurrence_on_month: Option<i16>,
        recurrence_on_day: Option<i16>,
        last_run_date: Option<NaiveDate>,
    ) -> FineractResult<Self> {
        let instruction = Self {
            id,
            name: name.into(),
            account_transfer_details_id,
            priority,
            instruction_type,
            status,
            amount,
            valid_from,
            valid_till,
            recurrence_type,
            recurrence_frequency,
            recurrence_interval,
            recurrence_on_month,
            recurrence_on_day,
            last_run_date,
        };
        instruction.validate_domain_rules()?;
        Ok(instruction)
    }

    /// Parses a month-day value (`[month, day]` JSON array or string like `"15 March"` / `"03-15"` / `"15/03"`)
    /// into `(month: i16, day: i16)`.
    pub fn extract_month_day(command: &JsonCommand, param: &str) -> Option<(i16, i16)> {
        let val = command.json_element(param)?;
        match val {
            Value::Array(arr) if arr.len() >= 2 => {
                let m = arr[0].as_i64()? as i16;
                let d = arr[1].as_i64()? as i16;
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    Some((m, d))
                } else {
                    None
                }
            }
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return None;
                }
                // Try splitting on whitespace, hyphen, or slash
                let tokens: Vec<&str> = trimmed
                    .split(|c: char| c.is_whitespace() || c == '-' || c == '/')
                    .filter(|t| !t.is_empty())
                    .collect();
                if tokens.len() != 2 {
                    return None;
                }
                let locale_str = command.locale();
                let locale = fineract_core_rs::json_command::FineractLocale::parse(if locale_str.is_empty() {
                    "en"
                } else {
                    &locale_str
                })
                .ok();
                // Check if first token is day and second is month name (e.g. "15 March" or "02 April")
                if let Ok(d) = tokens[0].parse::<i16>() {
                    if let Some(m) = tokens[1]
                        .parse::<i16>()
                        .ok()
                        .or_else(|| locale.as_ref()?.parse_month(tokens[1]).map(|m| m as i16))
                    {
                        let month_day_format = command
                            .string_value_of_parameter_named(PARAM_MONTH_DAY_FORMAT)
                            .unwrap_or_default();
                        if month_day_format.trim().starts_with('M') && (1..=12).contains(&d) && (1..=31).contains(&m) {
                            return Some((d, m));
                        }
                        if (1..=12).contains(&m) && (1..=31).contains(&d) {
                            return Some((m, d));
                        }
                    }
                }
                // Or first token is month name and second is day (e.g. "March 15")
                if let Some(m) = locale.as_ref().and_then(|l| l.parse_month(tokens[0])).map(|m| m as i16) {
                    if let Ok(d) = tokens[1].parse::<i16>() {
                        if (1..=12).contains(&m) && (1..=31).contains(&d) {
                            return Some((m, d));
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Port of `AccountTransferStandingInstruction.create(AccountTransferDetails, JsonCommand)`.
    pub fn from_json(
        id: i64,
        account_transfer_details_id: i64,
        command: &JsonCommand,
    ) -> FineractResult<Self> {
        let name = command
            .string_value_of_parameter_named(PARAM_NAME)
            .unwrap_or_default();
        let priority = StandingInstructionPriority::from_int(
            command.integer_value_of_parameter_named(PARAM_PRIORITY)?,
        );
        let instruction_type = StandingInstructionType::from_int(
            command.integer_value_of_parameter_named(PARAM_INSTRUCTION_TYPE)?,
        );
        let status = StandingInstructionStatus::from_int(
            command.integer_value_of_parameter_named(PARAM_STATUS)?,
        );
        let amount = command.big_decimal_value_of_parameter_named(PARAM_AMOUNT)?;
        let valid_from = command
            .local_date_value_of_parameter_named(PARAM_VALID_FROM)?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.standinginstruction.validFrom.cannot.be.blank",
                        "The parameter `validFrom` cannot be blank.",
                        PARAM_VALID_FROM,
                        Vec::new(),
                    ),
                )
            })?;
        let valid_till = command.local_date_value_of_parameter_named(PARAM_VALID_TILL)?;
        let recurrence_type = command
            .integer_value_of_parameter_named(PARAM_RECURRENCE_TYPE)?
            .unwrap_or(0) as i16;
        let recurrence_frequency = command
            .integer_value_of_parameter_named(PARAM_RECURRENCE_FREQUENCY)?
            .map(|v| v as i16);
        let recurrence_interval = command
            .integer_value_of_parameter_named(PARAM_RECURRENCE_INTERVAL)?
            .map(|v| v as i16);
        let month_day = Self::extract_month_day(command, PARAM_RECURRENCE_ON_MONTH_DAY);
        let recurrence_on_month = month_day.map(|(m, _)| m);
        let recurrence_on_day = month_day.map(|(_, d)| d);

        Self::new(
            id,
            name,
            account_transfer_details_id,
            priority,
            instruction_type,
            status,
            amount,
            valid_from,
            valid_till,
            recurrence_type,
            recurrence_frequency,
            recurrence_interval,
            recurrence_on_month,
            recurrence_on_day,
            None,
        )
    }

    /// Port of `AccountTransferStandingInstruction.validateDomainRules()`.
    pub fn validate_domain_rules(&self) -> FineractResult<()> {
        let base_validator = DataValidatorBuilder::new().resource(STANDING_INSTRUCTION_RESOURCE_NAME);

        if self.instruction_type.is_fixed_amount_transfer() {
            base_validator
                .reset()
                .parameter(PARAM_AMOUNT)
                .value(self.amount)
                .not_null();
        }

        if let Some(valid_till) = self.valid_till {
            base_validator
                .reset()
                .parameter(PARAM_VALID_TILL)
                .value(valid_till)
                .validate_date_after(self.valid_from);
        }

        let recurrence_type_enum = AccountTransferRecurrenceType::from_i16(self.recurrence_type);
        if recurrence_type_enum.is_periodic_recurrence() {
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_INTERVAL)
                .value(self.recurrence_interval.map(i64::from))
                .not_null()
                .integer_greater_than_zero();
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_FREQUENCY)
                .value(self.recurrence_frequency.map(i64::from))
                .not_null();

            if let Some(freq_raw) = self.recurrence_frequency {
                if let Some(freq) = CalendarFrequencyType::from_int(freq_raw as i32) {
                    if freq.is_monthly() || freq.is_yearly() {
                        base_validator
                            .reset()
                            .parameter(PARAM_RECURRENCE_ON_MONTH_DAY)
                            .value(self.recurrence_on_day.map(i64::from))
                            .not_null();
                        if freq.is_yearly() {
                            base_validator
                                .reset()
                                .parameter(PARAM_RECURRENCE_ON_MONTH_DAY)
                                .value(self.recurrence_on_month.map(i64::from))
                                .not_null();
                        }
                    }
                }
            }
        }

        base_validator.throw_validation_errors()?;
        Ok(())
    }

    /// Port of `AccountTransferStandingInstruction.update(JsonCommand)`.
    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        let base_validator = DataValidatorBuilder::new().resource(STANDING_INSTRUCTION_RESOURCE_NAME);

        if self.status.is_deleted() {
            base_validator
                .reset()
                .parameter(PARAM_STATUS)
                .value(self.status.as_i16() as i64)
                .fail_with_code("can.not.modify.once.deleted", "Standing instruction cannot be modified once deleted.");
            base_validator.throw_validation_errors()?;
        }

        let mut actual_changes = BTreeMap::new();

        if command.is_change_in_integer_parameter_named(PARAM_PRIORITY, Some(self.priority.as_i32()))? {
            if let Some(new_val) = command.integer_value_of_parameter_named(PARAM_PRIORITY)? {
                actual_changes.insert(PARAM_PRIORITY.to_string(), json!(new_val));
                self.priority = StandingInstructionPriority::from_int(Some(new_val));
            }
        }

        if command.is_change_in_integer_parameter_named(PARAM_STATUS, Some(self.status.as_i32()))? {
            if let Some(new_val) = command.integer_value_of_parameter_named(PARAM_STATUS)? {
                actual_changes.insert(PARAM_STATUS.to_string(), json!(new_val));
                self.status = StandingInstructionStatus::from_int(Some(new_val));
            }
        }

        if command.is_change_in_integer_parameter_named(
            PARAM_INSTRUCTION_TYPE,
            Some(self.instruction_type.as_i32()),
        )? {
            if let Some(new_val) = command.integer_value_of_parameter_named(PARAM_INSTRUCTION_TYPE)? {
                actual_changes.insert(PARAM_INSTRUCTION_TYPE.to_string(), json!(new_val));
                self.instruction_type = StandingInstructionType::from_int(Some(new_val));
            }
        }

        if command.is_change_in_big_decimal_parameter_named(PARAM_AMOUNT, self.amount)? {
            let new_val = command.big_decimal_value_of_parameter_named(PARAM_AMOUNT)?;
            actual_changes.insert(
                PARAM_AMOUNT.to_string(),
                new_val.map(|d| json!(d)).unwrap_or(Value::Null),
            );
            self.amount = new_val;
        }

        if command.is_change_in_local_date_parameter_named(PARAM_VALID_FROM, Some(self.valid_from))? {
            if let Some(new_val) = command.local_date_value_of_parameter_named(PARAM_VALID_FROM)? {
                actual_changes.insert(
                    PARAM_VALID_FROM.to_string(),
                    Value::String(
                        command
                            .string_value_of_parameter_named(PARAM_VALID_FROM)
                            .unwrap_or_else(|| new_val.to_string()),
                    ),
                );
                self.valid_from = new_val;
            }
        }

        if command.is_change_in_local_date_parameter_named(PARAM_VALID_TILL, self.valid_till)? {
            let new_val = command.local_date_value_of_parameter_named(PARAM_VALID_TILL)?;
            actual_changes.insert(
                PARAM_VALID_TILL.to_string(),
                new_val
                    .map(|d| {
                        Value::String(
                            command
                                .string_value_of_parameter_named(PARAM_VALID_TILL)
                                .unwrap_or_else(|| d.to_string()),
                        )
                    })
                    .unwrap_or(Value::Null),
            );
            self.valid_till = new_val;
        }

        if command.is_change_in_integer_parameter_named(
            PARAM_RECURRENCE_TYPE,
            Some(self.recurrence_type as i32),
        )? {
            if let Some(new_val) = command.integer_value_of_parameter_named(PARAM_RECURRENCE_TYPE)? {
                actual_changes.insert(PARAM_RECURRENCE_TYPE.to_string(), json!(new_val));
                self.recurrence_type = new_val as i16;
            }
        }

        let is_periodic = AccountTransferRecurrenceType::from_i16(self.recurrence_type).is_periodic_recurrence();
        if is_periodic {
            if command.is_change_in_integer_parameter_named(
                PARAM_RECURRENCE_FREQUENCY,
                self.recurrence_frequency.map(i32::from),
            )? {
                let new_val = command.integer_value_of_parameter_named(PARAM_RECURRENCE_FREQUENCY)?;
                actual_changes.insert(
                    PARAM_RECURRENCE_FREQUENCY.to_string(),
                    new_val.map(|v| json!(v)).unwrap_or(Value::Null),
                );
                self.recurrence_frequency = new_val.map(|v| v as i16);
            }

            if command.is_change_in_integer_parameter_named(
                PARAM_RECURRENCE_INTERVAL,
                self.recurrence_interval.map(i32::from),
            )? {
                let new_val = command.integer_value_of_parameter_named(PARAM_RECURRENCE_INTERVAL)?;
                actual_changes.insert(
                    PARAM_RECURRENCE_INTERVAL.to_string(),
                    new_val.map(|v| json!(v)).unwrap_or(Value::Null),
                );
                self.recurrence_interval = new_val.map(|v| v as i16);
            }

            if command.parameter_exists(PARAM_RECURRENCE_ON_MONTH_DAY) {
                let new_md = Self::extract_month_day(command, PARAM_RECURRENCE_ON_MONTH_DAY);
                let current_md = match (self.recurrence_on_month, self.recurrence_on_day) {
                    (Some(m), Some(d)) => Some((m, d)),
                    _ => None,
                };
                if new_md != current_md {
                    actual_changes.insert(
                        PARAM_RECURRENCE_ON_MONTH_DAY.to_string(),
                        command
                            .json_element(PARAM_RECURRENCE_ON_MONTH_DAY)
                            .cloned()
                            .unwrap_or(Value::Null),
                    );
                    self.recurrence_on_month = new_md.map(|(m, _)| m);
                    self.recurrence_on_day = new_md.map(|(_, d)| d);
                }
            }

            if let Some(freq_raw) = self.recurrence_frequency {
                if let Some(freq) = CalendarFrequencyType::from_int(freq_raw as i32) {
                    if !freq.is_monthly() && !freq.is_yearly() {
                        self.recurrence_on_day = None;
                        self.recurrence_on_month = None;
                    }
                }
            }
        } else {
            self.recurrence_frequency = None;
            self.recurrence_interval = None;
            self.recurrence_on_day = None;
            self.recurrence_on_month = None;
        }

        if self.instruction_type.is_dues_amount_transfer() {
            self.amount = None;
        }

        self.validate_domain_rules()?;
        Ok(actual_changes)
    }

    /// Port of `AccountTransferStandingInstruction.delete()`:
    /// sets status to `Deleted (3)` and appends `_deleted_<id>` to `name`.
    pub fn delete(&mut self) {
        self.status = StandingInstructionStatus::Deleted;
        self.name = format!("{}_deleted_{}", self.name, self.id);
    }

    /// Updates `last_run_date` (`AccountTransferStandingInstruction.updateLatsRunDate`).
    pub fn update_last_run_date(&mut self, last_run_date: NaiveDate) {
        self.last_run_date = Some(last_run_date);
    }

    /// Updates `status` (`AccountTransferStandingInstruction.updateStatus`).
    pub fn update_status(&mut self, status: StandingInstructionStatus) {
        self.status = status;
    }

    pub const fn recurrence_type_enum(&self) -> AccountTransferRecurrenceType {
        AccountTransferRecurrenceType::from_i16(self.recurrence_type)
    }

    pub const fn recurrence_frequency_enum(&self) -> Option<CalendarFrequencyType> {
        match self.recurrence_frequency {
            Some(v) => CalendarFrequencyType::from_int(v as i32),
            None => None,
        }
    }
}

// =============================================================================
// 12. StandingInstructionExecutionHistory & Dues Data (`m_account_transfer_standing_instructions_history`)
// =============================================================================

/// Loan dues summary used when executing a `StandingInstructionType::Dues` standing instruction.
///
/// Ported from `org.apache.fineract.portfolio.account.data.StandingInstructionDuesData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingInstructionDuesData {
    pub due_date: Option<NaiveDate>,
    pub principal_due: Decimal,
    pub interest_due: Decimal,
    pub fee_charges_due: Decimal,
    pub penalty_charges_due: Decimal,
}

impl StandingInstructionDuesData {
    pub fn new(
        due_date: Option<NaiveDate>,
        principal_due: Decimal,
        interest_due: Decimal,
        fee_charges_due: Decimal,
        penalty_charges_due: Decimal,
    ) -> Self {
        Self {
            due_date,
            principal_due,
            interest_due,
            fee_charges_due,
            penalty_charges_due,
        }
    }

    /// Total due amount (`principal_due + interest_due + fee_charges_due + penalty_charges_due`).
    pub fn total_due_amount(&self) -> Decimal {
        self.principal_due + self.interest_due + self.fee_charges_due + self.penalty_charges_due
    }
}

/// Execution history record stored in `m_account_transfer_standing_instructions_history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingInstructionExecutionHistory {
    pub id: i64,
    pub standing_instruction_id: i64,
    pub status: String,
    pub execution_time: NaiveDateTime,
    pub amount: Decimal,
    pub error_log: Option<String>,
}

impl StandingInstructionExecutionHistory {
    pub const STATUS_SUCCESS: &'static str = "success";
    pub const STATUS_PARTIAL: &'static str = "partial";
    pub const STATUS_FAILED: &'static str = "failed";

    pub fn new(
        id: i64,
        standing_instruction_id: i64,
        status: impl Into<String>,
        execution_time: NaiveDateTime,
        amount: Decimal,
        error_log: Option<String>,
    ) -> Self {
        Self {
            id,
            standing_instruction_id,
            status: status.into(),
            execution_time,
            amount,
            error_log,
        }
    }
}

/// Result of evaluating a standing instruction for execution on a given business date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingInstructionExecutionOutcome {
    pub standing_instruction_id: i64,
    pub is_due: bool,
    pub requested_amount: Decimal,
    pub executed_amount: Decimal,
    pub status: String,
    pub error_log: Option<String>,
    pub updated_last_run_date: Option<NaiveDate>,
    pub history_record: Option<StandingInstructionExecutionHistory>,
}

// =============================================================================
// 13. AccountTransferValidatorAndEngine
// =============================================================================

/// Validator and execution engine for Account Transfers and Standing Instructions.
///
/// Consolidates:
/// - `AccountTransfersDetailDataValidator`
/// - `AccountTransfersDataValidator`
/// - `StandingInstructionDataValidator`
/// - `AccountTransfersWritePlatformServiceImpl` transfer direction checks
/// - `ExecuteStandingInstructionsTasklet` schedule, dues, partial transfer, and history engine
#[derive(Debug, Clone, Default)]
pub struct AccountTransferValidatorAndEngine;

impl AccountTransferValidatorAndEngine {
    pub const fn new() -> Self {
        Self
    }

    /// Port of `AccountTransfersDetailDataValidator.validate(JsonCommand, DataValidatorBuilder)`.
    pub fn validate_detail_params(
        command: &JsonCommand,
        base_validator: &DataValidatorBuilder<'_>,
    ) -> FineractResult<()> {
        let from_office_id = command.long_value_of_parameter_named(PARAM_FROM_OFFICE_ID)?;
        base_validator
            .reset()
            .parameter(PARAM_FROM_OFFICE_ID)
            .value(from_office_id)
            .not_null()
            .integer_greater_than_zero();

        let from_client_id = command.long_value_of_parameter_named(PARAM_FROM_CLIENT_ID)?;
        base_validator
            .reset()
            .parameter(PARAM_FROM_CLIENT_ID)
            .value(from_client_id)
            .not_null()
            .integer_greater_than_zero();

        let from_account_id = command.long_value_of_parameter_named(PARAM_FROM_ACCOUNT_ID)?;
        base_validator
            .reset()
            .parameter(PARAM_FROM_ACCOUNT_ID)
            .value(from_account_id)
            .not_null()
            .integer_greater_than_zero();

        let to_office_id = command.long_value_of_parameter_named(PARAM_TO_OFFICE_ID)?;
        base_validator
            .reset()
            .parameter(PARAM_TO_OFFICE_ID)
            .value(to_office_id)
            .not_null()
            .integer_greater_than_zero();

        let to_client_id = command.long_value_of_parameter_named(PARAM_TO_CLIENT_ID)?;
        base_validator
            .reset()
            .parameter(PARAM_TO_CLIENT_ID)
            .value(to_client_id)
            .not_null()
            .integer_greater_than_zero();

        let to_account_id = command.long_value_of_parameter_named(PARAM_TO_ACCOUNT_ID)?;
        base_validator
            .reset()
            .parameter(PARAM_TO_ACCOUNT_ID)
            .value(to_account_id)
            .not_null()
            .integer_greater_than_zero();

        let from_account_type = command
            .integer_value_sans_locale_of_parameter_named(PARAM_FROM_ACCOUNT_TYPE)?
            .map(i64::from);
        base_validator
            .reset()
            .parameter(PARAM_FROM_ACCOUNT_TYPE)
            .value(from_account_type)
            .not_null()
            .is_one_of_these_values([1_i64, 2_i64]);

        let to_account_type = command
            .integer_value_sans_locale_of_parameter_named(PARAM_TO_ACCOUNT_TYPE)?
            .map(i64::from);
        base_validator
            .reset()
            .parameter(PARAM_TO_ACCOUNT_TYPE)
            .value(to_account_type)
            .not_null()
            .is_one_of_these_values([1_i64, 2_i64]);

        Ok(())
    }

    /// Validates transfer direction pairs (`Savings -> Savings`, `Savings -> Loan`, `Loan -> Savings`).
    ///
    /// Disallows:
    /// - Transferring from and to the exact same account (`from_account_type == to_account_type && from_account_id == to_account_id`).
    /// - `Loan -> Loan` unless `allow_loan_to_loan_refund` is explicitly `true`.
    /// - Invalid `PortfolioAccountType`.
    pub fn validate_transfer_direction(
        from_account_type: PortfolioAccountType,
        from_account_id: i64,
        to_account_type: PortfolioAccountType,
        to_account_id: i64,
        transfer_type: AccountTransferType,
        allow_loan_to_loan_refund: bool,
    ) -> FineractResult<()> {
        if from_account_type.is_invalid() || to_account_type.is_invalid() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.accounttransfer.invalid.account.type",
                "Source and destination account types must be valid Loan (1) or Savings (2) accounts.",
            )));
        }

        if from_account_type == to_account_type && from_account_id == to_account_id {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.accounttransfer.from.and.to.account.cannot.be.same",
                "Source and destination accounts cannot be the same account.",
            )));
        }

        match (from_account_type, to_account_type) {
            (PortfolioAccountType::Savings, PortfolioAccountType::Savings) => {
                if transfer_type.is_loan_repayment() {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.accounttransfer.savings.to.savings.cannot.be.loan.repayment",
                        "Cannot execute a loan repayment transfer between two savings accounts.",
                    )));
                }
                Ok(())
            }
            (PortfolioAccountType::Savings, PortfolioAccountType::Loan) => Ok(()),
            (PortfolioAccountType::Loan, PortfolioAccountType::Savings) => {
                if transfer_type.is_loan_repayment() || transfer_type.is_charge_payment() {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.accounttransfer.loan.to.savings.must.be.refund.transfer",
                        "Transfer from a loan account to a savings account is only permitted for overpaid loan refunds.",
                    )));
                }
                Ok(())
            }
            (PortfolioAccountType::Loan, PortfolioAccountType::Loan) => {
                if allow_loan_to_loan_refund {
                    Ok(())
                } else {
                    Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.accounttransfer.loan.to.loan.not.supported",
                        "Direct account transfer from a Loan account to another Loan account is not supported.",
                    )))
                }
            }
            _ => Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.accounttransfer.invalid.transfer.direction",
                "Unsupported account transfer direction.",
            ))),
        }
    }

    /// Infers the [`AccountTransferType`] for a transfer given its source and destination [`PortfolioAccountType`].
    pub const fn infer_transfer_type(
        from_account_type: PortfolioAccountType,
        to_account_type: PortfolioAccountType,
        explicit_type: Option<AccountTransferType>,
    ) -> AccountTransferType {
        if let Some(t) = explicit_type {
            if !t.is_invalid() {
                return t;
            }
        }
        match (from_account_type, to_account_type) {
            (PortfolioAccountType::Savings, PortfolioAccountType::Loan) => {
                AccountTransferType::LoanRepayment
            }
            (PortfolioAccountType::Savings, PortfolioAccountType::Savings)
            | (PortfolioAccountType::Loan, PortfolioAccountType::Savings) => {
                AccountTransferType::AccountTransfer
            }
            (PortfolioAccountType::Loan, PortfolioAccountType::Loan) => {
                AccountTransferType::LoanRepayment
            }
            _ => AccountTransferType::Invalid,
        }
    }

    /// Port of `AccountTransfersDataValidator.validate(JsonCommand)`.
    pub fn validate_create_transfer(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(CREATE_ACCOUNT_TRANSFER_REQUEST_PARAMS)?;

        let base_validator = DataValidatorBuilder::new().resource(ACCOUNT_TRANSFER_RESOURCE_NAME);
        Self::validate_detail_params(command, &base_validator)?;

        let transaction_date = command.local_date_value_of_parameter_named(PARAM_TRANSFER_DATE)?;
        base_validator
            .reset()
            .parameter(PARAM_TRANSFER_DATE)
            .value(transaction_date)
            .not_null();

        let transaction_amount =
            command.big_decimal_value_of_parameter_named(PARAM_TRANSFER_AMOUNT)?;
        base_validator
            .reset()
            .parameter(PARAM_TRANSFER_AMOUNT)
            .value(transaction_amount)
            .not_null()
            .positive_amount();

        let transaction_description =
            command.string_value_of_parameter_named(PARAM_TRANSFER_DESCRIPTION);
        base_validator
            .reset()
            .parameter(PARAM_TRANSFER_DESCRIPTION)
            .value(transaction_description.as_deref())
            .not_blank()
            .not_exceeding_length_of(200);

        base_validator.throw_validation_errors()?;

        let from_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_FROM_ACCOUNT_TYPE)?,
        );
        let to_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_TO_ACCOUNT_TYPE)?,
        );
        let from_account_id = command
            .long_value_of_parameter_named(PARAM_FROM_ACCOUNT_ID)?
            .unwrap_or_default();
        let to_account_id = command
            .long_value_of_parameter_named(PARAM_TO_ACCOUNT_ID)?
            .unwrap_or_default();
        let transfer_type = Self::infer_transfer_type(from_account_type, to_account_type, None);

        Self::validate_transfer_direction(
            from_account_type,
            from_account_id,
            to_account_type,
            to_account_id,
            transfer_type,
            false,
        )
    }

    /// Port of `AccountTransfersDataValidator.validateForRefundByTransfer(JsonCommand)`.
    pub fn validate_refund_by_transfer(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(REFUND_BY_TRANSFER_REQUEST_PARAMS)?;

        let base_validator = DataValidatorBuilder::new().resource(ACCOUNT_TRANSFER_RESOURCE_NAME);
        Self::validate_detail_params(command, &base_validator)?;

        let transaction_date = command.local_date_value_of_parameter_named(PARAM_TRANSFER_DATE)?;
        base_validator
            .reset()
            .parameter(PARAM_TRANSFER_DATE)
            .value(transaction_date)
            .not_null();

        let transaction_amount =
            command.big_decimal_value_of_parameter_named(PARAM_TRANSFER_AMOUNT)?;
        base_validator
            .reset()
            .parameter(PARAM_TRANSFER_AMOUNT)
            .value(transaction_amount)
            .not_null()
            .positive_amount();

        let transaction_description =
            command.string_value_of_parameter_named(PARAM_TRANSFER_DESCRIPTION);
        base_validator
            .reset()
            .parameter(PARAM_TRANSFER_DESCRIPTION)
            .value(transaction_description.as_deref())
            .not_blank()
            .not_exceeding_length_of(200);

        let mut has_payment_detail_field = false;
        for param in [
            PARAM_ACCOUNT_NUMBER,
            PARAM_CHECK_NUMBER,
            PARAM_ROUTING_CODE,
            PARAM_RECEIPT_NUMBER,
            PARAM_BANK_NUMBER,
        ] {
            if command.parameter_exists(param) {
                let val = command.string_value_of_parameter_named(param);
                base_validator
                    .reset()
                    .parameter(param)
                    .value(val.as_deref())
                    .ignore_if_null()
                    .not_exceeding_length_of(50);
                if val.as_ref().is_some_and(|s| !s.trim().is_empty()) {
                    has_payment_detail_field = true;
                }
            }
        }

        if command.parameter_exists(PARAM_PAYMENT_TYPE_ID) || has_payment_detail_field {
            let payment_type_id = command.long_value_of_parameter_named(PARAM_PAYMENT_TYPE_ID)?;
            base_validator
                .reset()
                .parameter(PARAM_PAYMENT_TYPE_ID)
                .value(payment_type_id)
                .not_null()
                .integer_greater_than_zero();
        }

        base_validator.throw_validation_errors()?;

        let from_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_FROM_ACCOUNT_TYPE)?,
        );
        let to_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_TO_ACCOUNT_TYPE)?,
        );
        let from_account_id = command
            .long_value_of_parameter_named(PARAM_FROM_ACCOUNT_ID)?
            .unwrap_or_default();
        let to_account_id = command
            .long_value_of_parameter_named(PARAM_TO_ACCOUNT_ID)?
            .unwrap_or_default();

        if !from_account_type.is_loan_account() || !to_account_type.is_savings_account() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.accounttransfer.refund.must.be.loan.to.savings",
                "Refund by transfer requires a source Loan account and a destination Savings account.",
            )));
        }

        Self::validate_transfer_direction(
            from_account_type,
            from_account_id,
            to_account_type,
            to_account_id,
            AccountTransferType::AccountTransfer,
            false,
        )
    }

    /// Port of `StandingInstructionDataValidator.validateForCreate(JsonCommand)`.
    pub fn validate_for_standing_instruction_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(CREATE_STANDING_INSTRUCTION_REQUEST_PARAMS)?;

        let base_validator =
            DataValidatorBuilder::new().resource(STANDING_INSTRUCTION_RESOURCE_NAME);
        Self::validate_detail_params(command, &base_validator)?;

        let status = command
            .integer_value_of_parameter_named(PARAM_STATUS)?
            .map(i64::from);
        base_validator
            .reset()
            .parameter(PARAM_STATUS)
            .value(status)
            .not_null()
            .in_min_max_range(1, 2);

        let valid_from = command.local_date_value_of_parameter_named(PARAM_VALID_FROM)?;
        base_validator
            .reset()
            .parameter(PARAM_VALID_FROM)
            .value(valid_from)
            .not_null();

        let valid_till = command.local_date_value_of_parameter_named(PARAM_VALID_TILL)?;
        if let (Some(vf), Some(vt)) = (valid_from, valid_till) {
            base_validator
                .reset()
                .parameter(PARAM_VALID_TILL)
                .value(vt)
                .validate_date_after(vf);
        }

        let amount = command.big_decimal_value_of_parameter_named(PARAM_AMOUNT)?;
        base_validator
            .reset()
            .parameter(PARAM_AMOUNT)
            .value(amount)
            .ignore_if_null()
            .positive_amount();

        let transfer_type_int = command.integer_value_of_parameter_named(PARAM_TRANSFER_TYPE)?;
        base_validator
            .reset()
            .parameter(PARAM_TRANSFER_TYPE)
            .value(transfer_type_int.map(i64::from))
            .not_null()
            .in_min_max_range(1, 3);

        let priority = command
            .integer_value_of_parameter_named(PARAM_PRIORITY)?
            .map(i64::from);
        base_validator
            .reset()
            .parameter(PARAM_PRIORITY)
            .value(priority)
            .not_null()
            .in_min_max_range(1, 4);

        let standing_instruction_type = command
            .integer_value_of_parameter_named(PARAM_INSTRUCTION_TYPE)?
            .map(i64::from);
        base_validator
            .reset()
            .parameter(PARAM_INSTRUCTION_TYPE)
            .value(standing_instruction_type)
            .not_null()
            .in_min_max_range(1, 2);

        let recurrence_type = command
            .integer_value_of_parameter_named(PARAM_RECURRENCE_TYPE)?
            .map(i64::from);
        base_validator
            .reset()
            .parameter(PARAM_RECURRENCE_TYPE)
            .value(recurrence_type)
            .not_null()
            .in_min_max_range(1, 2);

        let recurrence_frequency = command.integer_value_of_parameter_named(PARAM_RECURRENCE_FREQUENCY)?;
        base_validator
            .reset()
            .parameter(PARAM_RECURRENCE_FREQUENCY)
            .value(recurrence_frequency.map(i64::from))
            .ignore_if_null()
            .in_min_max_range(0, 3);

        if let Some(freq_int) = recurrence_frequency {
            if let Some(freq) = CalendarFrequencyType::from_int(freq_int) {
                if freq.is_monthly() || freq.is_yearly() {
                    let month_day =
                        AccountTransferStandingInstruction::extract_month_day(command, PARAM_RECURRENCE_ON_MONTH_DAY);
                    base_validator
                        .reset()
                        .parameter(PARAM_RECURRENCE_ON_MONTH_DAY)
                        .value(month_day.map(|(_, d)| d as i64))
                        .not_null();
                }
            }
        }

        let recurrence_interval = command
            .integer_value_of_parameter_named(PARAM_RECURRENCE_INTERVAL)?
            .map(i64::from);
        if recurrence_type == Some(AccountTransferRecurrenceType::Periodic.as_i16() as i64) {
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_INTERVAL)
                .value(recurrence_interval)
                .not_null()
                .integer_greater_than_zero();
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_FREQUENCY)
                .value(recurrence_frequency.map(i64::from))
                .not_null();
        } else {
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_INTERVAL)
                .value(recurrence_interval)
                .ignore_if_null()
                .integer_greater_than_zero();
        }

        let name = command.string_value_of_parameter_named(PARAM_NAME);
        base_validator
            .reset()
            .parameter(PARAM_NAME)
            .value(name.as_deref())
            .not_null();

        let to_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_TO_ACCOUNT_TYPE)?,
        );
        if to_account_type.is_savings_account() {
            base_validator
                .reset()
                .parameter(PARAM_INSTRUCTION_TYPE)
                .value(standing_instruction_type)
                .not_null()
                .in_min_max_range(1, 1);
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_TYPE)
                .value(recurrence_type)
                .not_null()
                .in_min_max_range(1, 1);
        }

        let from_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_FROM_ACCOUNT_TYPE)?,
        );
        let transfer_type = AccountTransferType::from_int(transfer_type_int);
        if transfer_type.is_account_transfer()
            && (from_account_type.is_loan_account() || to_account_type.is_loan_account())
        {
            base_validator
                .reset()
                .parameter(PARAM_TRANSFER_TYPE)
                .value(transfer_type.as_i16() as i64)
                .fail_with_code(
                    "not.account.transfer",
                    "Account transfer is not valid when either source or destination is a loan account.",
                );
        } else if transfer_type.is_loan_repayment()
            && (from_account_type.is_loan_account() || to_account_type.is_savings_account())
        {
            base_validator
                .reset()
                .parameter(PARAM_TRANSFER_TYPE)
                .value(transfer_type.as_i16() as i64)
                .fail_with_code(
                    "not.loan.repayment",
                    "Loan repayment is only valid from a savings account to a loan account.",
                );
        }

        if StandingInstructionType::from_int(standing_instruction_type.map(|v| v as i32))
            .is_fixed_amount_transfer()
        {
            base_validator
                .reset()
                .parameter(PARAM_AMOUNT)
                .value(amount)
                .not_null();
        }

        base_validator.throw_validation_errors()?;

        let from_account_id = command
            .long_value_of_parameter_named(PARAM_FROM_ACCOUNT_ID)?
            .unwrap_or_default();
        let to_account_id = command
            .long_value_of_parameter_named(PARAM_TO_ACCOUNT_ID)?
            .unwrap_or_default();

        Self::validate_transfer_direction(
            from_account_type,
            from_account_id,
            to_account_type,
            to_account_id,
            transfer_type,
            false,
        )
    }

    /// Port of `StandingInstructionDataValidator.validateForUpdate(JsonCommand)`.
    pub fn validate_for_standing_instruction_update(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(UPDATE_STANDING_INSTRUCTION_REQUEST_PARAMS)?;

        let base_validator =
            DataValidatorBuilder::new().resource(STANDING_INSTRUCTION_RESOURCE_NAME);

        if command.parameter_exists(PARAM_VALID_FROM) {
            let valid_from = command.local_date_value_of_parameter_named(PARAM_VALID_FROM)?;
            base_validator
                .reset()
                .parameter(PARAM_VALID_FROM)
                .value(valid_from)
                .not_null();
        }

        let mut valid_till = None;
        if command.parameter_exists(PARAM_VALID_TILL) {
            valid_till = command.local_date_value_of_parameter_named(PARAM_VALID_TILL)?;
            base_validator
                .reset()
                .parameter(PARAM_VALID_TILL)
                .value(valid_till)
                .not_null();
        }

        if let (Some(vf), Some(vt)) = (
            command.local_date_value_of_parameter_named(PARAM_VALID_FROM)?,
            valid_till,
        ) {
            base_validator
                .reset()
                .parameter(PARAM_VALID_TILL)
                .value(vt)
                .validate_date_after(vf);
        }

        if command.parameter_exists(PARAM_AMOUNT) {
            let amount = command.big_decimal_value_of_parameter_named(PARAM_AMOUNT)?;
            base_validator
                .reset()
                .parameter(PARAM_AMOUNT)
                .value(amount)
                .ignore_if_null()
                .positive_amount();
        }

        if command.parameter_exists(PARAM_STATUS) {
            let status = command
                .integer_value_of_parameter_named(PARAM_STATUS)?
                .map(i64::from);
            base_validator
                .reset()
                .parameter(PARAM_STATUS)
                .value(status)
                .not_null()
                .in_min_max_range(1, 2);
        }

        if command.parameter_exists(PARAM_PRIORITY) {
            let priority = command
                .integer_value_of_parameter_named(PARAM_PRIORITY)?
                .map(i64::from);
            base_validator
                .reset()
                .parameter(PARAM_PRIORITY)
                .value(priority)
                .not_null()
                .in_min_max_range(1, 4);
        }

        if command.parameter_exists(PARAM_INSTRUCTION_TYPE) {
            let instruction_type = command
                .integer_value_of_parameter_named(PARAM_INSTRUCTION_TYPE)?
                .map(i64::from);
            base_validator
                .reset()
                .parameter(PARAM_INSTRUCTION_TYPE)
                .value(instruction_type)
                .not_null()
                .in_min_max_range(1, 2);
        }

        if command.parameter_exists(PARAM_RECURRENCE_TYPE) {
            let recurrence_type = command
                .integer_value_of_parameter_named(PARAM_RECURRENCE_TYPE)?
                .map(i64::from);
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_TYPE)
                .value(recurrence_type)
                .not_null()
                .in_min_max_range(1, 2);
        }

        if command.parameter_exists(PARAM_RECURRENCE_FREQUENCY) {
            let recurrence_frequency = command
                .integer_value_of_parameter_named(PARAM_RECURRENCE_FREQUENCY)?
                .map(i64::from);
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_FREQUENCY)
                .value(recurrence_frequency)
                .ignore_if_null()
                .in_min_max_range(0, 3);
        }

        if command.parameter_exists(PARAM_RECURRENCE_INTERVAL) {
            let recurrence_interval = command
                .integer_value_of_parameter_named(PARAM_RECURRENCE_INTERVAL)?
                .map(i64::from);
            base_validator
                .reset()
                .parameter(PARAM_RECURRENCE_INTERVAL)
                .value(recurrence_interval)
                .ignore_if_null()
                .integer_greater_than_zero();
        }

        base_validator.throw_validation_errors()?;
        Ok(())
    }

    /// Computes the due amount for a standing instruction (`Fixed` vs `Dues`).
    ///
    /// Ported from `ExecuteStandingInstructionsTasklet.execute`.
    pub fn compute_due_amount(
        instruction: &AccountTransferStandingInstruction,
        dues: Option<&StandingInstructionDuesData>,
    ) -> FineractResult<Decimal> {
        match instruction.instruction_type {
            StandingInstructionType::Fixed => instruction.amount.ok_or_else(|| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.standinginstruction.fixed.amount.missing",
                    "Fixed standing instruction requires a non-null amount.",
                ))
            }),
            StandingInstructionType::Dues => {
                let Some(dues_data) = dues else {
                    return Ok(Decimal::ZERO);
                };
                Ok(dues_data.total_due_amount())
            }
            StandingInstructionType::Invalid => {
                Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.standinginstruction.invalid.instruction.type",
                    "Standing instruction has an invalid instruction type.",
                )))
            }
        }
    }

    /// Evaluates whether `instruction` is scheduled to execute on `business_date`.
    ///
    /// Supports both:
    /// - `AccountTransferRecurrenceType::Periodic` (`Days = 0`, `Weeks = 1`, `Months = 2`, `Years = 3`)
    /// - `AccountTransferRecurrenceType::AsPerDues` (comparing `dues.due_date <= business_date`)
    pub fn is_due_on_date(
        instruction: &AccountTransferStandingInstruction,
        business_date: NaiveDate,
        dues: Option<&StandingInstructionDuesData>,
    ) -> bool {
        if !instruction.status.is_active() {
            return false;
        }
        if business_date < instruction.valid_from {
            return false;
        }
        if let Some(valid_till) = instruction.valid_till {
            if business_date > valid_till {
                return false;
            }
        }
        if instruction.last_run_date == Some(business_date) {
            return false;
        }

        match instruction.recurrence_type_enum() {
            AccountTransferRecurrenceType::AsPerDues => {
                let Some(dues_data) = dues else {
                    return false;
                };
                if dues_data.total_due_amount() <= Decimal::ZERO {
                    return false;
                }
                match dues_data.due_date {
                    Some(due_date) => due_date <= business_date,
                    None => true,
                }
            }
            AccountTransferRecurrenceType::Periodic => {
                let interval = instruction.recurrence_interval.unwrap_or(1).max(1) as u32;
                let Some(freq) = instruction.recurrence_frequency_enum() else {
                    return false;
                };

                let mut start_date = instruction.valid_from;
                if freq.is_monthly() || freq.is_yearly() {
                    if let Some(on_day) = instruction.recurrence_on_day {
                        let target_day = on_day.max(1) as u32;
                        let target_month = if freq.is_yearly() {
                            instruction
                                .recurrence_on_month
                                .map(|m| m.clamp(1, 12) as u32)
                                .unwrap_or_else(|| start_date.month())
                        } else {
                            start_date.month()
                        };
                        let candidate = Self::build_clamped_date(
                            start_date.year(),
                            target_month,
                            target_day,
                        );
                        start_date = if candidate < start_date {
                            if freq.is_monthly() {
                                candidate
                                    .checked_add_months(Months::new(1))
                                    .map(|d| {
                                        Self::build_clamped_date(d.year(), d.month(), target_day)
                                    })
                                    .unwrap_or(candidate)
                            } else {
                                Self::build_clamped_date(
                                    start_date.year() + 1,
                                    target_month,
                                    target_day,
                                )
                            }
                        } else {
                            candidate
                        };
                    }
                }

                if business_date < start_date {
                    return false;
                }

                match freq {
                    CalendarFrequencyType::Daily => {
                        let days = (business_date - start_date).num_days();
                        days >= 0 && (days as u32) % interval == 0
                    }
                    CalendarFrequencyType::Weekly => {
                        let days = (business_date - start_date).num_days();
                        days >= 0 && (days as u32) % (interval * 7) == 0
                    }
                    CalendarFrequencyType::Monthly => {
                        let months_diff = (business_date.year() - start_date.year()) * 12
                            + (business_date.month() as i32 - start_date.month() as i32);
                        if months_diff < 0 || (months_diff as u32) % interval != 0 {
                            return false;
                        }
                        let desired_day = instruction
                            .recurrence_on_day
                            .map(|d| d.max(1) as u32)
                            .unwrap_or_else(|| start_date.day());
                        let expected_date = Self::build_clamped_date(
                            business_date.year(),
                            business_date.month(),
                            desired_day,
                        );
                        business_date == expected_date
                    }
                    CalendarFrequencyType::Yearly => {
                        let years_diff = business_date.year() - start_date.year();
                        if years_diff < 0 || (years_diff as u32) % interval != 0 {
                            return false;
                        }
                        let desired_month = instruction
                            .recurrence_on_month
                            .map(|m| m.clamp(1, 12) as u32)
                            .unwrap_or_else(|| start_date.month());
                        let desired_day = instruction
                            .recurrence_on_day
                            .map(|d| d.max(1) as u32)
                            .unwrap_or_else(|| start_date.day());
                        let expected_date = Self::build_clamped_date(
                            business_date.year(),
                            desired_month,
                            desired_day,
                        );
                        business_date == expected_date
                    }
                    CalendarFrequencyType::Invalid => false,
                }
            }
            AccountTransferRecurrenceType::Invalid => false,
        }
    }

    fn build_clamped_date(year: i32, month: u32, desired_day: u32) -> NaiveDate {
        let max_day = days_in_year_month(year, month);
        let clamped_day = desired_day.clamp(1, max_day);
        NaiveDate::from_ymd_opt(year, month, clamped_day)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, month, 1).expect("valid month start"))
    }

    /// Port of `ExecuteStandingInstructionsTasklet.coverableAmount`:
    ///
    /// Computes the maximum amount that can be covered from `withdrawable_balance`
    /// taking an optional transfer fee (`flat_fee` or `percentage_fee`) into account,
    /// rounded down (`RoundingMode.DOWN` / `FineractRoundingMode::Down`) to `currency_digits`.
    pub fn calculate_coverable_amount(
        requested_amount: Decimal,
        withdrawable_balance: Decimal,
        percentage_fee: Option<Decimal>,
        flat_fee: Option<Decimal>,
        currency_digits: u32,
    ) -> Decimal {
        if withdrawable_balance <= Decimal::ZERO || requested_amount <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let compute_fee = |amt: Decimal| -> Decimal {
            if let Some(pct) = percentage_fee {
                let raw = (amt * pct) / Decimal::ONE_HUNDRED;
                raw.round_dp_with_strategy(
                    currency_digits,
                    FineractRoundingMode::HalfEven.to_rounding_strategy(),
                )
            } else if let Some(flat) = flat_fee {
                flat
            } else {
                Decimal::ZERO
            }
        };

        let fee_for_requested = compute_fee(requested_amount);
        if requested_amount + fee_for_requested <= withdrawable_balance {
            return requested_amount;
        }

        let fee_for_withdrawable = compute_fee(withdrawable_balance);
        let available = withdrawable_balance - fee_for_withdrawable;
        if available <= Decimal::ZERO {
            Decimal::ZERO
        } else {
            available.round_dp_with_strategy(
                currency_digits,
                FineractRoundingMode::Down.to_rounding_strategy(),
            )
        }
    }

    /// Evaluates and executes a standing instruction for `business_date`, producing an
    /// [`StandingInstructionExecutionOutcome`] and a [`StandingInstructionExecutionHistory`] entry
    /// for `m_account_transfer_standing_instructions_history`.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_standing_instruction(
        instruction: &mut AccountTransferStandingInstruction,
        details: &AccountTransferDetails,
        business_date: NaiveDate,
        execution_time: NaiveDateTime,
        dues: Option<&StandingInstructionDuesData>,
        source_withdrawable_balance: Decimal,
        allow_partial_transfer: bool,
        percentage_fee: Option<Decimal>,
        flat_fee: Option<Decimal>,
        currency_digits: u32,
        history_id: i64,
    ) -> FineractResult<StandingInstructionExecutionOutcome> {
        let from_type = details.from_account_type();
        let to_type = details.to_account_type();
        let from_id = details.from_account_id().unwrap_or_default();
        let to_id = details.to_account_id().unwrap_or_default();
        Self::validate_transfer_direction(
            from_type,
            from_id,
            to_type,
            to_id,
            details.transfer_type,
            false,
        )?;

        if !Self::is_due_on_date(instruction, business_date, dues) {
            return Ok(StandingInstructionExecutionOutcome {
                standing_instruction_id: instruction.id,
                is_due: false,
                requested_amount: Decimal::ZERO,
                executed_amount: Decimal::ZERO,
                status: "skipped".to_string(),
                error_log: None,
                updated_last_run_date: instruction.last_run_date,
                history_record: None,
            });
        }

        let requested_amount = Self::compute_due_amount(instruction, dues)?;
        if requested_amount <= Decimal::ZERO {
            return Ok(StandingInstructionExecutionOutcome {
                standing_instruction_id: instruction.id,
                is_due: false,
                requested_amount: Decimal::ZERO,
                executed_amount: Decimal::ZERO,
                status: "skipped".to_string(),
                error_log: None,
                updated_last_run_date: instruction.last_run_date,
                history_record: None,
            });
        }

        let coverable = Self::calculate_coverable_amount(
            requested_amount,
            source_withdrawable_balance,
            percentage_fee,
            flat_fee,
            currency_digits,
        );

        let (executed_amount, status, error_log) = if coverable >= requested_amount {
            (
                requested_amount,
                StandingInstructionExecutionHistory::STATUS_SUCCESS.to_string(),
                None,
            )
        } else if allow_partial_transfer && coverable > Decimal::ZERO {
            (
                coverable,
                StandingInstructionExecutionHistory::STATUS_PARTIAL.to_string(),
                None,
            )
        } else {
            (
                Decimal::ZERO,
                StandingInstructionExecutionHistory::STATUS_FAILED.to_string(),
                Some(format!(
                    "Insufficient Account Balance Exception while transferring {} (withdrawable: {})",
                    requested_amount, source_withdrawable_balance
                )),
            )
        };

        if status != StandingInstructionExecutionHistory::STATUS_FAILED {
            instruction.update_last_run_date(business_date);
        }

        let history_amount = if executed_amount > Decimal::ZERO {
            executed_amount
        } else {
            requested_amount
        };
        let history = StandingInstructionExecutionHistory::new(
            history_id,
            instruction.id,
            status.clone(),
            execution_time,
            history_amount,
            error_log.clone(),
        );

        Ok(StandingInstructionExecutionOutcome {
            standing_instruction_id: instruction.id,
            is_due: true,
            requested_amount,
            executed_amount,
            status,
            error_log,
            updated_last_run_date: instruction.last_run_date,
            history_record: Some(history),
        })
    }
}

// =============================================================================
// 14. DualDbAccountTransferRepository
// =============================================================================

/// Dual-database (PostgreSQL & MySQL) and in-memory repository for Account Transfers,
/// Portfolio Account Associations, and Standing Instructions.
#[derive(Debug)]
pub struct DualDbAccountTransferRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    next_id: AtomicI64,
    associations: DashMap<i64, AccountAssociations>,
    transfer_details: DashMap<i64, AccountTransferDetails>,
    transfer_transactions: DashMap<i64, AccountTransferTransaction>,
    standing_instructions: DashMap<i64, AccountTransferStandingInstruction>,
    standing_instruction_history: DashMap<i64, StandingInstructionExecutionHistory>,
}

impl Default for DualDbAccountTransferRepository {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl DualDbAccountTransferRepository {
    pub const ASSOCIATIONS_COLUMNS: &'static str = "id, loan_account_id, savings_account_id, \
        linked_loan_account_id, linked_savings_account_id, association_type_enum, is_active";

    pub const TRANSFER_DETAILS_COLUMNS: &'static str = "id, from_office_id, to_office_id, \
        from_client_id, to_client_id, from_savings_account_id, to_savings_account_id, \
        from_loan_account_id, to_loan_account_id, transfer_type";

    pub const TRANSFER_TRANSACTION_COLUMNS: &'static str = "id, account_transfer_details_id, \
        from_savings_transaction_id, to_savings_transaction_id, to_loan_transaction_id, \
        from_loan_transaction_id, is_reversed, transaction_date, currency_code, amount, description";

    pub const STANDING_INSTRUCTION_COLUMNS: &'static str = "id, name, account_transfer_details_id, \
        priority, instruction_type, status, amount, valid_from, valid_till, recurrence_type, \
        recurrence_frequency, recurrence_interval, recurrence_on_month, recurrence_on_day, last_run_date";

    pub const STANDING_INSTRUCTION_HISTORY_COLUMNS: &'static str =
        "id, standing_instruction_id, status, execution_time, amount, error_log";

    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            next_id: AtomicI64::new(1),
            associations: DashMap::new(),
            transfer_details: DashMap::new(),
            transfer_transactions: DashMap::new(),
            standing_instructions: DashMap::new(),
            standing_instruction_history: DashMap::new(),
        }
    }

    pub fn new_in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    pub fn in_memory() -> Self {
        Self::new_in_memory()
    }

    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            pool: None,
            dialect,
            next_id: AtomicI64::new(1),
            associations: DashMap::new(),
            transfer_details: DashMap::new(),
            transfer_transactions: DashMap::new(),
            standing_instructions: DashMap::new(),
            standing_instruction_history: DashMap::new(),
        }
    }

    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub const fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    pub fn allocate_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    // -------------------------------------------------------------------------
    // Dialect-aware SQL Generators
    // -------------------------------------------------------------------------

    pub fn insert_association_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_portfolio_account_associations \
             (id, loan_account_id, savings_account_id, linked_loan_account_id, linked_savings_account_id, association_type_enum, is_active) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            dialect,
        )
    }

    pub fn select_association_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_portfolio_account_associations WHERE id = $1",
                Self::ASSOCIATIONS_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_transfer_details_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_account_transfer_details \
             (id, from_office_id, to_office_id, from_client_id, to_client_id, \
              from_savings_account_id, to_savings_account_id, from_loan_account_id, to_loan_account_id, transfer_type) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            dialect,
        )
    }

    pub fn select_transfer_details_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_account_transfer_details WHERE id = $1",
                Self::TRANSFER_DETAILS_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_transfer_transaction_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_account_transfer_transaction \
             (id, account_transfer_details_id, from_savings_transaction_id, to_savings_transaction_id, \
              to_loan_transaction_id, from_loan_transaction_id, is_reversed, transaction_date, \
              currency_code, currency_digits, amount, description) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 2, $10, $11)",
            dialect,
        )
    }

    pub fn select_transfer_transaction_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_account_transfer_transaction WHERE id = $1",
                Self::TRANSFER_TRANSACTION_COLUMNS
            ),
            dialect,
        )
    }

    pub fn reverse_transfer_transaction_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_account_transfer_transaction SET is_reversed = $1 WHERE id = $2",
            dialect,
        )
    }

    pub fn insert_standing_instruction_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_account_transfer_standing_instructions \
             (id, name, account_transfer_details_id, priority, instruction_type, status, amount, \
              valid_from, valid_till, recurrence_type, recurrence_frequency, recurrence_interval, \
              recurrence_on_month, recurrence_on_day, last_run_date) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
            dialect,
        )
    }

    pub fn update_standing_instruction_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_account_transfer_standing_instructions \
             SET name = $1, priority = $2, instruction_type = $3, status = $4, amount = $5, \
                 valid_from = $6, valid_till = $7, recurrence_type = $8, recurrence_frequency = $9, \
                 recurrence_interval = $10, recurrence_on_month = $11, recurrence_on_day = $12, \
                 last_run_date = $13 \
             WHERE id = $14",
            dialect,
        )
    }

    pub fn select_standing_instruction_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_account_transfer_standing_instructions WHERE id = $1",
                Self::STANDING_INSTRUCTION_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_standing_instruction_history_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_account_transfer_standing_instructions_history \
             (id, standing_instruction_id, status, execution_time, amount, error_log) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            dialect,
        )
    }

    pub fn select_standing_instruction_history_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_account_transfer_standing_instructions_history WHERE standing_instruction_id = $1 ORDER BY id ASC",
                Self::STANDING_INSTRUCTION_HISTORY_COLUMNS
            ),
            dialect,
        )
    }

    // -------------------------------------------------------------------------
    // AccountAssociations Operations
    // -------------------------------------------------------------------------

    pub async fn save_association(
        &self,
        mut association: AccountAssociations,
    ) -> FineractResult<AccountAssociations> {
        if association.id <= 0 {
            association.id = self.allocate_id();
        }
        if let Some(pool) = &self.pool {
            let sql = Self::insert_association_sql(self.dialect);
            sqlx::query(&sql)
                .bind(association.id)
                .bind(association.loan_account_id)
                .bind(association.savings_account_id)
                .bind(association.linked_loan_account_id)
                .bind(association.linked_savings_account_id)
                .bind(association.association_type.as_i16())
                .bind(association.is_active)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.associations
            .insert(association.id, association.clone());
        Ok(association)
    }

    pub async fn get_association(&self, id: i64) -> FineractResult<AccountAssociations> {
        if let Some(found) = self.associations.get(&id) {
            return Ok(found.clone());
        }
        if let Some(pool) = &self.pool {
            let sql = Self::select_association_by_id_sql(self.dialect);
            if let Some(row) = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?
            {
                let assoc = AccountAssociations {
                    id: row.try_get("id").unwrap_or(id),
                    loan_account_id: row.try_get("loan_account_id").ok().flatten(),
                    savings_account_id: row.try_get("savings_account_id").ok().flatten(),
                    linked_loan_account_id: row.try_get("linked_loan_account_id").ok().flatten(),
                    linked_savings_account_id: row
                        .try_get("linked_savings_account_id")
                        .ok()
                        .flatten(),
                    association_type: AccountAssociationType::from_i16(
                        row.try_get::<i16, _>("association_type_enum").unwrap_or(1),
                    ),
                    is_active: row.try_get("is_active").unwrap_or(true),
                };
                self.associations.insert(assoc.id, assoc.clone());
                return Ok(assoc);
            }
        }
        Err(FineractError::resource_not_found(
            "error.msg.account.association.id.invalid",
            format!("Account association with identifier {id} does not exist"),
            id.to_string(),
        ))
    }

    // -------------------------------------------------------------------------
    // AccountTransferDetails Operations
    // -------------------------------------------------------------------------

    pub async fn save_transfer_details(
        &self,
        mut details: AccountTransferDetails,
    ) -> FineractResult<AccountTransferDetails> {
        if details.id <= 0 {
            details.id = self.allocate_id();
        }
        if let Some(pool) = &self.pool {
            let sql = Self::insert_transfer_details_sql(self.dialect);
            sqlx::query(&sql)
                .bind(details.id)
                .bind(details.from_office_id)
                .bind(details.to_office_id)
                .bind(details.from_client_id)
                .bind(details.to_client_id)
                .bind(details.from_savings_account_id)
                .bind(details.to_savings_account_id)
                .bind(details.from_loan_account_id)
                .bind(details.to_loan_account_id)
                .bind(details.transfer_type.as_i16())
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.transfer_details.insert(details.id, details.clone());
        Ok(details)
    }

    pub async fn get_transfer_details(&self, id: i64) -> FineractResult<AccountTransferDetails> {
        if let Some(found) = self.transfer_details.get(&id) {
            return Ok(found.clone());
        }
        if let Some(pool) = &self.pool {
            let sql = Self::select_transfer_details_by_id_sql(self.dialect);
            if let Some(row) = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?
            {
                let details = AccountTransferDetails {
                    id: row.try_get("id").unwrap_or(id),
                    from_office_id: row.try_get("from_office_id").unwrap_or_default(),
                    to_office_id: row.try_get("to_office_id").unwrap_or_default(),
                    from_client_id: row.try_get("from_client_id").unwrap_or_default(),
                    to_client_id: row.try_get("to_client_id").unwrap_or_default(),
                    from_savings_account_id: row.try_get("from_savings_account_id").ok().flatten(),
                    to_savings_account_id: row.try_get("to_savings_account_id").ok().flatten(),
                    from_loan_account_id: row.try_get("from_loan_account_id").ok().flatten(),
                    to_loan_account_id: row.try_get("to_loan_account_id").ok().flatten(),
                    transfer_type: AccountTransferType::from_i16(
                        row.try_get::<i16, _>("transfer_type").unwrap_or(0),
                    ),
                };
                self.transfer_details.insert(details.id, details.clone());
                return Ok(details);
            }
        }
        Err(FineractError::resource_not_found(
            "error.msg.accounttransfer.details.id.invalid",
            format!("Account transfer details with identifier {id} does not exist"),
            id.to_string(),
        ))
    }

    // -------------------------------------------------------------------------
    // AccountTransferTransaction Operations
    // -------------------------------------------------------------------------

    pub async fn save_transfer_transaction(
        &self,
        mut txn: AccountTransferTransaction,
    ) -> FineractResult<AccountTransferTransaction> {
        if txn.id <= 0 {
            txn.id = self.allocate_id();
        }
        if let Some(pool) = &self.pool {
            let sql = Self::insert_transfer_transaction_sql(self.dialect);
            sqlx::query(&sql)
                .bind(txn.id)
                .bind(txn.account_transfer_details_id)
                .bind(txn.from_savings_transaction_id)
                .bind(txn.to_savings_transaction_id)
                .bind(txn.to_loan_transaction_id)
                .bind(txn.from_loan_transaction_id)
                .bind(txn.is_reversed)
                .bind(txn.transaction_date.to_string())
                .bind(&txn.currency_code)
                .bind(txn.amount.to_string())
                .bind(&txn.description)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.transfer_transactions.insert(txn.id, txn.clone());
        Ok(txn)
    }

    pub async fn get_transfer_transaction(
        &self,
        id: i64,
    ) -> FineractResult<AccountTransferTransaction> {
        if let Some(found) = self.transfer_transactions.get(&id) {
            return Ok(found.clone());
        }
        if let Some(pool) = &self.pool {
            let sql = Self::select_transfer_transaction_by_id_sql(self.dialect);
            if let Some(row) = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?
            {
                let date_str: String = row.try_get("transaction_date").unwrap_or_default();
                let amount_str: String = row.try_get("amount").unwrap_or_else(|_| "0".to_string());
                let txn = AccountTransferTransaction {
                    id: row.try_get("id").unwrap_or(id),
                    account_transfer_details_id: row
                        .try_get("account_transfer_details_id")
                        .unwrap_or_default(),
                    from_savings_transaction_id: row
                        .try_get("from_savings_transaction_id")
                        .ok()
                        .flatten(),
                    to_savings_transaction_id: row
                        .try_get("to_savings_transaction_id")
                        .ok()
                        .flatten(),
                    to_loan_transaction_id: row.try_get("to_loan_transaction_id").ok().flatten(),
                    from_loan_transaction_id: row
                        .try_get("from_loan_transaction_id")
                        .ok()
                        .flatten(),
                    is_reversed: row.try_get("is_reversed").unwrap_or(false),
                    transaction_date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    currency_code: row
                        .try_get("currency_code")
                        .unwrap_or_else(|_| "USD".to_string()),
                    amount: Decimal::from_str_exact(&amount_str).unwrap_or(Decimal::ZERO),
                    description: row.try_get("description").unwrap_or_default(),
                };
                self.transfer_transactions.insert(txn.id, txn.clone());
                return Ok(txn);
            }
        }
        Err(FineractError::resource_not_found(
            "error.msg.accounttransfer.id.invalid",
            format!("Account transfer with identifier {id} does not exist"),
            id.to_string(),
        ))
    }

    pub async fn reverse_transfer_transaction(
        &self,
        id: i64,
    ) -> FineractResult<AccountTransferTransaction> {
        let mut txn = self.get_transfer_transaction(id).await?;
        txn.reverse();
        if let Some(pool) = &self.pool {
            let sql = Self::reverse_transfer_transaction_sql(self.dialect);
            sqlx::query(&sql)
                .bind(true)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.transfer_transactions.insert(id, txn.clone());
        Ok(txn)
    }

    pub fn list_transfer_transactions_in_memory(&self) -> Vec<AccountTransferTransaction> {
        let mut list: Vec<_> = self
            .transfer_transactions
            .iter()
            .map(|e| e.value().clone())
            .collect();
        list.sort_by_key(|t| t.id);
        list
    }

    // -------------------------------------------------------------------------
    // AccountTransferStandingInstruction Operations
    // -------------------------------------------------------------------------

    pub async fn save_standing_instruction(
        &self,
        mut instruction: AccountTransferStandingInstruction,
    ) -> FineractResult<AccountTransferStandingInstruction> {
        if instruction.id <= 0 {
            instruction.id = self.allocate_id();
        }
        // Check for duplicate standing instruction name
        if self.standing_instructions.iter().any(|entry| {
            entry.key() != &instruction.id
                && entry.value().name.eq_ignore_ascii_case(&instruction.name)
        }) {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::with_args(
                "error.msg.standinginstruction.duplicate.name",
                format!(
                    "Standing instruction with name `{}` already exists",
                    instruction.name
                ),
                vec![ApiErrorArg::new(instruction.name.clone())],
            )));
        }
        if let Some(pool) = &self.pool {
            let sql = Self::insert_standing_instruction_sql(self.dialect);
            sqlx::query(&sql)
                .bind(instruction.id)
                .bind(&instruction.name)
                .bind(instruction.account_transfer_details_id)
                .bind(instruction.priority.as_i16())
                .bind(instruction.instruction_type.as_i16())
                .bind(instruction.status.as_i16())
                .bind(instruction.amount.map(|d| d.to_string()))
                .bind(instruction.valid_from.to_string())
                .bind(instruction.valid_till.map(|d| d.to_string()))
                .bind(instruction.recurrence_type)
                .bind(instruction.recurrence_frequency)
                .bind(instruction.recurrence_interval)
                .bind(instruction.recurrence_on_month)
                .bind(instruction.recurrence_on_day)
                .bind(instruction.last_run_date.map(|d| d.to_string()))
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.standing_instructions
            .insert(instruction.id, instruction.clone());
        Ok(instruction)
    }

    pub async fn update_standing_instruction(
        &self,
        instruction: AccountTransferStandingInstruction,
    ) -> FineractResult<AccountTransferStandingInstruction> {
        if let Some(pool) = &self.pool {
            let sql = Self::update_standing_instruction_sql(self.dialect);
            sqlx::query(&sql)
                .bind(&instruction.name)
                .bind(instruction.priority.as_i16())
                .bind(instruction.instruction_type.as_i16())
                .bind(instruction.status.as_i16())
                .bind(instruction.amount.map(|d| d.to_string()))
                .bind(instruction.valid_from.to_string())
                .bind(instruction.valid_till.map(|d| d.to_string()))
                .bind(instruction.recurrence_type)
                .bind(instruction.recurrence_frequency)
                .bind(instruction.recurrence_interval)
                .bind(instruction.recurrence_on_month)
                .bind(instruction.recurrence_on_day)
                .bind(instruction.last_run_date.map(|d| d.to_string()))
                .bind(instruction.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.standing_instructions
            .insert(instruction.id, instruction.clone());
        Ok(instruction)
    }

    pub async fn get_standing_instruction(
        &self,
        id: i64,
    ) -> FineractResult<AccountTransferStandingInstruction> {
        if let Some(found) = self.standing_instructions.get(&id) {
            return Ok(found.clone());
        }
        if let Some(pool) = &self.pool {
            let sql = Self::select_standing_instruction_by_id_sql(self.dialect);
            if let Some(row) = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?
            {
                let valid_from_str: String = row.try_get("valid_from").unwrap_or_default();
                let valid_till_str: Option<String> = row.try_get("valid_till").ok().flatten();
                let last_run_str: Option<String> = row.try_get("last_run_date").ok().flatten();
                let amount_str: Option<String> = row.try_get("amount").ok().flatten();

                let instruction = AccountTransferStandingInstruction {
                    id: row.try_get("id").unwrap_or(id),
                    name: row.try_get("name").unwrap_or_default(),
                    account_transfer_details_id: row
                        .try_get("account_transfer_details_id")
                        .unwrap_or_default(),
                    priority: StandingInstructionPriority::from_i16(
                        row.try_get::<i16, _>("priority").unwrap_or(0),
                    ),
                    instruction_type: StandingInstructionType::from_i16(
                        row.try_get::<i16, _>("instruction_type").unwrap_or(0),
                    ),
                    status: StandingInstructionStatus::from_i16(
                        row.try_get::<i16, _>("status").unwrap_or(0),
                    ),
                    amount: amount_str
                        .and_then(|s| Decimal::from_str_exact(&s).ok()),
                    valid_from: NaiveDate::parse_from_str(&valid_from_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    valid_till: valid_till_str
                        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    recurrence_type: row.try_get::<i16, _>("recurrence_type").unwrap_or(0),
                    recurrence_frequency: row.try_get("recurrence_frequency").ok().flatten(),
                    recurrence_interval: row.try_get("recurrence_interval").ok().flatten(),
                    recurrence_on_month: row.try_get("recurrence_on_month").ok().flatten(),
                    recurrence_on_day: row.try_get("recurrence_on_day").ok().flatten(),
                    last_run_date: last_run_str
                        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                };
                self.standing_instructions
                    .insert(instruction.id, instruction.clone());
                return Ok(instruction);
            }
        }
        Err(FineractError::resource_not_found(
            "error.msg.standinginstruction.id.invalid",
            format!("Standing instruction with identifier {id} does not exist"),
            id.to_string(),
        ))
    }

    pub fn list_active_standing_instructions_in_memory(
        &self,
    ) -> Vec<AccountTransferStandingInstruction> {
        let mut list: Vec<_> = self
            .standing_instructions
            .iter()
            .filter(|e| e.value().status.is_active())
            .map(|e| e.value().clone())
            .collect();
        // Sort by priority (Urgent=1 < High=2 < Medium=3 < Low=4), then by ID
        list.sort_by_key(|si| (si.priority.as_i16(), si.id));
        list
    }

    // -------------------------------------------------------------------------
    // StandingInstructionExecutionHistory Operations
    // -------------------------------------------------------------------------

    pub async fn save_standing_instruction_history(
        &self,
        mut history: StandingInstructionExecutionHistory,
    ) -> FineractResult<StandingInstructionExecutionHistory> {
        if history.id <= 0 {
            history.id = self.allocate_id();
        }
        if let Some(pool) = &self.pool {
            let sql = Self::insert_standing_instruction_history_sql(self.dialect);
            sqlx::query(&sql)
                .bind(history.id)
                .bind(history.standing_instruction_id)
                .bind(&history.status)
                .bind(history.execution_time.to_string())
                .bind(history.amount.to_string())
                .bind(&history.error_log)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.standing_instruction_history
            .insert(history.id, history.clone());
        Ok(history)
    }

    pub fn list_standing_instruction_history_in_memory(
        &self,
        standing_instruction_id: i64,
    ) -> Vec<StandingInstructionExecutionHistory> {
        let mut list: Vec<_> = self
            .standing_instruction_history
            .iter()
            .filter(|e| e.value().standing_instruction_id == standing_instruction_id)
            .map(|e| e.value().clone())
            .collect();
        list.sort_by_key(|h| h.id);
        list
    }
}

// =============================================================================
// 15. Command Handlers
// =============================================================================

/// Ported from `CreateAccountTransferCommandHandler` (`AccountTransfersWritePlatformServiceImpl.create`).
#[derive(Debug, Clone)]
pub struct CreateAccountTransferCommandHandler {
    repo: Arc<DualDbAccountTransferRepository>,
}

pub type AccountTransferCreateCommandHandler = CreateAccountTransferCommandHandler;

impl CreateAccountTransferCommandHandler {
    pub fn new(repo: Arc<DualDbAccountTransferRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbAccountTransferRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateAccountTransferCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        AccountTransferValidatorAndEngine::validate_create_transfer(command)?;

        let from_office_id = command
            .long_value_of_parameter_named(PARAM_FROM_OFFICE_ID)?
            .unwrap_or_default();
        let from_client_id = command
            .long_value_of_parameter_named(PARAM_FROM_CLIENT_ID)?
            .unwrap_or_default();
        let from_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_FROM_ACCOUNT_TYPE)?,
        );
        let from_account_id = command
            .long_value_of_parameter_named(PARAM_FROM_ACCOUNT_ID)?
            .unwrap_or_default();

        let to_office_id = command
            .long_value_of_parameter_named(PARAM_TO_OFFICE_ID)?
            .unwrap_or_default();
        let to_client_id = command
            .long_value_of_parameter_named(PARAM_TO_CLIENT_ID)?
            .unwrap_or_default();
        let to_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_TO_ACCOUNT_TYPE)?,
        );
        let to_account_id = command
            .long_value_of_parameter_named(PARAM_TO_ACCOUNT_ID)?
            .unwrap_or_default();

        let transaction_date = command
            .local_date_value_of_parameter_named(PARAM_TRANSFER_DATE)?
            .unwrap();
        let transaction_amount = command
            .big_decimal_value_of_parameter_named(PARAM_TRANSFER_AMOUNT)?
            .unwrap();
        let description = command
            .string_value_of_parameter_named(PARAM_TRANSFER_DESCRIPTION)
            .unwrap_or_default();

        let details_id = self.repo.allocate_id();
        let txn_id = self.repo.allocate_id();
        let source_sub_txn_id = self.repo.allocate_id();
        let dest_sub_txn_id = self.repo.allocate_id();

        let (details, transfer_txn) = match (from_account_type, to_account_type) {
            (PortfolioAccountType::Savings, PortfolioAccountType::Loan) => {
                let d = AccountTransferDetails::savings_to_loan_transfer(
                    details_id,
                    from_office_id,
                    from_client_id,
                    from_account_id,
                    to_office_id,
                    to_client_id,
                    to_account_id,
                    AccountTransferType::LoanRepayment,
                );
                let t = AccountTransferTransaction::savings_to_loan_transfer(
                    txn_id,
                    details_id,
                    source_sub_txn_id,
                    dest_sub_txn_id,
                    transaction_date,
                    "USD",
                    transaction_amount,
                    description,
                );
                (d, t)
            }
            (PortfolioAccountType::Savings, PortfolioAccountType::Savings) => {
                let d = AccountTransferDetails::savings_to_savings_transfer(
                    details_id,
                    from_office_id,
                    from_client_id,
                    from_account_id,
                    to_office_id,
                    to_client_id,
                    to_account_id,
                    AccountTransferType::AccountTransfer,
                );
                let t = AccountTransferTransaction::savings_to_savings_transfer(
                    txn_id,
                    details_id,
                    source_sub_txn_id,
                    dest_sub_txn_id,
                    transaction_date,
                    "USD",
                    transaction_amount,
                    description,
                );
                (d, t)
            }
            (PortfolioAccountType::Loan, PortfolioAccountType::Savings) => {
                let d = AccountTransferDetails::loan_to_savings_transfer(
                    details_id,
                    from_office_id,
                    from_client_id,
                    from_account_id,
                    to_office_id,
                    to_client_id,
                    to_account_id,
                    AccountTransferType::AccountTransfer,
                );
                let t = AccountTransferTransaction::loan_to_savings_transfer(
                    txn_id,
                    details_id,
                    dest_sub_txn_id,
                    source_sub_txn_id,
                    transaction_date,
                    "USD",
                    transaction_amount,
                    description,
                );
                (d, t)
            }
            _ => {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.accounttransfer.invalid.transfer.direction",
                    "Unsupported account transfer direction.",
                )));
            }
        };

        self.repo.save_transfer_details(details).await?;
        let saved_txn = self.repo.save_transfer_transaction(transfer_txn).await?;

        let mut builder = CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(from_office_id)
            .with_client_id(from_client_id)
            .with_entity_id(saved_txn.id);

        if from_account_type.is_savings_account() {
            builder = builder.with_savings_id(from_account_id);
        } else if from_account_type.is_loan_account() {
            builder = builder.with_loan_id(from_account_id);
        }

        Ok(builder.build())
    }
}

/// Ported from `RefundByTransferCommandHandler` (`AccountTransfersWritePlatformServiceImpl.refundByTransfer`).
#[derive(Debug, Clone)]
pub struct RefundByTransferCommandHandler {
    repo: Arc<DualDbAccountTransferRepository>,
}

impl RefundByTransferCommandHandler {
    pub fn new(repo: Arc<DualDbAccountTransferRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbAccountTransferRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for RefundByTransferCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        AccountTransferValidatorAndEngine::validate_refund_by_transfer(command)?;

        let from_office_id = command
            .long_value_of_parameter_named(PARAM_FROM_OFFICE_ID)?
            .unwrap_or_default();
        let from_client_id = command
            .long_value_of_parameter_named(PARAM_FROM_CLIENT_ID)?
            .unwrap_or_default();
        let from_loan_account_id = command
            .long_value_of_parameter_named(PARAM_FROM_ACCOUNT_ID)?
            .unwrap_or_default();

        let to_office_id = command
            .long_value_of_parameter_named(PARAM_TO_OFFICE_ID)?
            .unwrap_or_default();
        let to_client_id = command
            .long_value_of_parameter_named(PARAM_TO_CLIENT_ID)?
            .unwrap_or_default();
        let to_savings_account_id = command
            .long_value_of_parameter_named(PARAM_TO_ACCOUNT_ID)?
            .unwrap_or_default();

        let transaction_date = command
            .local_date_value_of_parameter_named(PARAM_TRANSFER_DATE)?
            .unwrap();
        let transaction_amount = command
            .big_decimal_value_of_parameter_named(PARAM_TRANSFER_AMOUNT)?
            .unwrap();
        let description = command
            .string_value_of_parameter_named(PARAM_TRANSFER_DESCRIPTION)
            .unwrap_or_default();

        let details_id = self.repo.allocate_id();
        let txn_id = self.repo.allocate_id();
        let loan_refund_txn_id = self.repo.allocate_id();
        let savings_deposit_txn_id = self.repo.allocate_id();

        let details = AccountTransferDetails::loan_to_savings_transfer(
            details_id,
            from_office_id,
            from_client_id,
            from_loan_account_id,
            to_office_id,
            to_client_id,
            to_savings_account_id,
            AccountTransferType::AccountTransfer,
        );
        let txn = AccountTransferTransaction::loan_to_savings_transfer(
            txn_id,
            details_id,
            savings_deposit_txn_id,
            loan_refund_txn_id,
            transaction_date,
            "USD",
            transaction_amount,
            description,
        );

        self.repo.save_transfer_details(details).await?;
        let saved_txn = self.repo.save_transfer_transaction(txn).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(from_office_id)
            .with_client_id(from_client_id)
            .with_loan_id(from_loan_account_id)
            .with_savings_id(to_savings_account_id)
            .with_entity_id(saved_txn.id)
            .build())
    }
}

/// Reverses an existing account transfer transaction (`AccountTransfersWritePlatformServiceImpl.reverseAllTransactions`).
#[derive(Debug, Clone)]
pub struct UndoAccountTransferCommandHandler {
    repo: Arc<DualDbAccountTransferRepository>,
}

impl UndoAccountTransferCommandHandler {
    pub fn new(repo: Arc<DualDbAccountTransferRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbAccountTransferRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UndoAccountTransferCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let transfer_txn_id = command.entity_id.ok_or_else(|| {
            FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.accounttransfer.id.cannot.be.blank",
                    "Account transfer transaction ID is required.",
                    "id",
                    Vec::new(),
                ),
            )
        })?;
        let reversed = self
            .repo
            .reverse_transfer_transaction(transfer_txn_id)
            .await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(reversed.id)
            .build())
    }
}

/// Ported from `CreateStandingInstructionCommandHandler` (`StandingInstructionWritePlatformServiceImpl.create`).
#[derive(Debug, Clone)]
pub struct CreateStandingInstructionCommandHandler {
    repo: Arc<DualDbAccountTransferRepository>,
}

pub type StandingInstructionCreateCommandHandler = CreateStandingInstructionCommandHandler;

impl CreateStandingInstructionCommandHandler {
    pub fn new(repo: Arc<DualDbAccountTransferRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbAccountTransferRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateStandingInstructionCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        AccountTransferValidatorAndEngine::validate_for_standing_instruction_create(command)?;

        let from_office_id = command
            .long_value_of_parameter_named(PARAM_FROM_OFFICE_ID)?
            .unwrap_or_default();
        let from_client_id = command
            .long_value_of_parameter_named(PARAM_FROM_CLIENT_ID)?
            .unwrap_or_default();
        let from_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_FROM_ACCOUNT_TYPE)?,
        );
        let from_account_id = command
            .long_value_of_parameter_named(PARAM_FROM_ACCOUNT_ID)?
            .unwrap_or_default();

        let to_office_id = command
            .long_value_of_parameter_named(PARAM_TO_OFFICE_ID)?
            .unwrap_or_default();
        let to_client_id = command
            .long_value_of_parameter_named(PARAM_TO_CLIENT_ID)?
            .unwrap_or_default();
        let to_account_type = PortfolioAccountType::from_int(
            command.integer_value_sans_locale_of_parameter_named(PARAM_TO_ACCOUNT_TYPE)?,
        );
        let to_account_id = command
            .long_value_of_parameter_named(PARAM_TO_ACCOUNT_ID)?
            .unwrap_or_default();

        let transfer_type = AccountTransferType::from_int(
            command.integer_value_of_parameter_named(PARAM_TRANSFER_TYPE)?,
        );

        let details_id = self.repo.allocate_id();
        let details = match (from_account_type, to_account_type) {
            (PortfolioAccountType::Savings, PortfolioAccountType::Loan) => {
                AccountTransferDetails::savings_to_loan_transfer(
                    details_id,
                    from_office_id,
                    from_client_id,
                    from_account_id,
                    to_office_id,
                    to_client_id,
                    to_account_id,
                    transfer_type,
                )
            }
            (PortfolioAccountType::Savings, PortfolioAccountType::Savings) => {
                AccountTransferDetails::savings_to_savings_transfer(
                    details_id,
                    from_office_id,
                    from_client_id,
                    from_account_id,
                    to_office_id,
                    to_client_id,
                    to_account_id,
                    transfer_type,
                )
            }
            (PortfolioAccountType::Loan, PortfolioAccountType::Savings) => {
                AccountTransferDetails::loan_to_savings_transfer(
                    details_id,
                    from_office_id,
                    from_client_id,
                    from_account_id,
                    to_office_id,
                    to_client_id,
                    to_account_id,
                    transfer_type,
                )
            }
            _ => {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.standinginstruction.invalid.transfer.direction",
                    "Unsupported standing instruction account transfer direction.",
                )));
            }
        };

        let saved_details = self.repo.save_transfer_details(details).await?;
        let instruction_id = self.repo.allocate_id();
        let instruction = AccountTransferStandingInstruction::from_json(
            instruction_id,
            saved_details.id,
            command,
        )?;
        let saved_instruction = self.repo.save_standing_instruction(instruction).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(from_office_id)
            .with_client_id(from_client_id)
            .with_entity_id(saved_instruction.id)
            .build())
    }
}

/// Ported from `UpdateStandingInstructionCommandHandler` (`StandingInstructionWritePlatformServiceImpl.update`).
#[derive(Debug, Clone)]
pub struct UpdateStandingInstructionCommandHandler {
    repo: Arc<DualDbAccountTransferRepository>,
}

pub type StandingInstructionUpdateCommandHandler = UpdateStandingInstructionCommandHandler;

impl UpdateStandingInstructionCommandHandler {
    pub fn new(repo: Arc<DualDbAccountTransferRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbAccountTransferRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateStandingInstructionCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let standing_instruction_id = command.entity_id.ok_or_else(|| {
            FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.standinginstruction.id.cannot.be.blank",
                    "Standing instruction ID is required.",
                    "id",
                    Vec::new(),
                ),
            )
        })?;

        AccountTransferValidatorAndEngine::validate_for_standing_instruction_update(command)?;

        let mut instruction = self
            .repo
            .get_standing_instruction(standing_instruction_id)
            .await?;
        let actual_changes = instruction.update(command)?;
        if !actual_changes.is_empty() {
            self.repo.update_standing_instruction(instruction).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(standing_instruction_id)
            .with_changes(actual_changes)
            .build())
    }
}

/// Ported from `DeleteStandingInstructionCommandHandler` (`StandingInstructionWritePlatformServiceImpl.delete`).
#[derive(Debug, Clone)]
pub struct DeleteStandingInstructionCommandHandler {
    repo: Arc<DualDbAccountTransferRepository>,
}

pub type StandingInstructionDeleteCommandHandler = DeleteStandingInstructionCommandHandler;

impl DeleteStandingInstructionCommandHandler {
    pub fn new(repo: Arc<DualDbAccountTransferRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbAccountTransferRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DeleteStandingInstructionCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let standing_instruction_id = command.entity_id.ok_or_else(|| {
            FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.standinginstruction.id.cannot.be.blank",
                    "Standing instruction ID is required.",
                    "id",
                    Vec::new(),
                ),
            )
        })?;

        let mut instruction = self
            .repo
            .get_standing_instruction(standing_instruction_id)
            .await?;
        instruction.delete();
        self.repo.update_standing_instruction(instruction).await?;

        let mut changes = BTreeMap::new();
        changes.insert(
            PARAM_STATUS.to_string(),
            json!(StandingInstructionStatus::Deleted.as_i16()),
        );

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(standing_instruction_id)
            .with_changes(changes)
            .build())
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_context() -> ExecutionContext {
        ExecutionContext::new("default", NaiveDate::from_ymd_opt(2026, 3, 15).unwrap())
    }

    #[test]
    fn test_enums_discriminants_and_codes() {
        assert_eq!(PortfolioAccountType::Invalid as i16, 0);
        assert_eq!(PortfolioAccountType::Loan as i16, 1);
        assert_eq!(PortfolioAccountType::Savings as i16, 2);
        assert_eq!(
            PortfolioAccountType::from_i16(1),
            PortfolioAccountType::Loan
        );
        assert_eq!(
            PortfolioAccountType::Savings.code(),
            "accountType.savings"
        );

        assert_eq!(AccountTransferType::Invalid as i16, 0);
        assert_eq!(AccountTransferType::AccountTransfer as i16, 1);
        assert_eq!(AccountTransferType::LoanRepayment as i16, 2);
        assert_eq!(AccountTransferType::ChargePayment as i16, 3);
        assert_eq!(AccountTransferType::InterestTransfer as i16, 4);
        assert!(AccountTransferType::from_i16(2).is_loan_repayment());

        assert_eq!(StandingInstructionType::Invalid as i16, 0);
        assert_eq!(StandingInstructionType::Fixed as i16, 1);
        assert_eq!(StandingInstructionType::Dues as i16, 2);
        assert!(StandingInstructionType::Fixed.is_fixed_amount_transfer());
        assert!(StandingInstructionType::Dues.is_dues_amount_transfer());

        assert_eq!(StandingInstructionPriority::Invalid as i16, 0);
        assert_eq!(StandingInstructionPriority::Urgent as i16, 1);
        assert_eq!(StandingInstructionPriority::High as i16, 2);
        assert_eq!(StandingInstructionPriority::Medium as i16, 3);
        assert_eq!(StandingInstructionPriority::Low as i16, 4);

        assert_eq!(StandingInstructionStatus::Invalid as i16, 0);
        assert_eq!(StandingInstructionStatus::Active as i16, 1);
        assert_eq!(StandingInstructionStatus::Disabled as i16, 2);
        assert_eq!(StandingInstructionStatus::Deleted as i16, 3);

        assert_eq!(AccountAssociationType::Invalid as i16, 0);
        assert_eq!(AccountAssociationType::LinkedAccount as i16, 1);
        assert_eq!(AccountAssociationType::GuarantorAccount as i16, 2);
        assert!(AccountAssociationType::LinkedAccount.is_linked_account_association());
    }

    #[test]
    fn test_validate_transfer_direction_pairs() {
        // 1. Savings -> Savings (AccountTransfer) is valid
        assert!(AccountTransferValidatorAndEngine::validate_transfer_direction(
            PortfolioAccountType::Savings,
            10,
            PortfolioAccountType::Savings,
            20,
            AccountTransferType::AccountTransfer,
            false,
        )
        .is_ok());

        // 2. Savings -> Savings (same account ID) is rejected
        assert!(AccountTransferValidatorAndEngine::validate_transfer_direction(
            PortfolioAccountType::Savings,
            10,
            PortfolioAccountType::Savings,
            10,
            AccountTransferType::AccountTransfer,
            false,
        )
        .is_err());

        // 3. Savings -> Loan (LoanRepayment / ChargePayment) is valid
        assert!(AccountTransferValidatorAndEngine::validate_transfer_direction(
            PortfolioAccountType::Savings,
            10,
            PortfolioAccountType::Loan,
            30,
            AccountTransferType::LoanRepayment,
            false,
        )
        .is_ok());
        assert!(AccountTransferValidatorAndEngine::validate_transfer_direction(
            PortfolioAccountType::Savings,
            10,
            PortfolioAccountType::Loan,
            30,
            AccountTransferType::ChargePayment,
            false,
        )
        .is_ok());

        // 4. Loan -> Savings (Refund / AccountTransfer) is valid, but LoanRepayment is rejected
        assert!(AccountTransferValidatorAndEngine::validate_transfer_direction(
            PortfolioAccountType::Loan,
            30,
            PortfolioAccountType::Savings,
            10,
            AccountTransferType::AccountTransfer,
            false,
        )
        .is_ok());
        assert!(AccountTransferValidatorAndEngine::validate_transfer_direction(
            PortfolioAccountType::Loan,
            30,
            PortfolioAccountType::Savings,
            10,
            AccountTransferType::LoanRepayment,
            false,
        )
        .is_err());

        // 5. Loan -> Loan is disallowed unless allow_loan_to_loan_refund is true
        assert!(AccountTransferValidatorAndEngine::validate_transfer_direction(
            PortfolioAccountType::Loan,
            30,
            PortfolioAccountType::Loan,
            40,
            AccountTransferType::LoanRepayment,
            false,
        )
        .is_err());
        assert!(AccountTransferValidatorAndEngine::validate_transfer_direction(
            PortfolioAccountType::Loan,
            30,
            PortfolioAccountType::Loan,
            40,
            AccountTransferType::LoanRepayment,
            true,
        )
        .is_ok());
    }

    #[test]
    fn test_standing_instruction_dues_and_fixed_calculation() {
        let valid_from = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let fixed_si = AccountTransferStandingInstruction::new(
            1,
            "Monthly Savings to Loan Fixed",
            100,
            StandingInstructionPriority::High,
            StandingInstructionType::Fixed,
            StandingInstructionStatus::Active,
            Some(dec!(250.50)),
            valid_from,
            None,
            AccountTransferRecurrenceType::Periodic.as_i16(),
            Some(CalendarFrequencyType::Monthly.as_int() as i16),
            Some(1),
            Some(1),
            Some(15),
            None,
        )
        .unwrap();

        assert_eq!(
            AccountTransferValidatorAndEngine::compute_due_amount(&fixed_si, None).unwrap(),
            dec!(250.50)
        );

        let dues_si = AccountTransferStandingInstruction::new(
            2,
            "Loan Installment Dues",
            101,
            StandingInstructionPriority::Urgent,
            StandingInstructionType::Dues,
            StandingInstructionStatus::Active,
            None,
            valid_from,
            None,
            AccountTransferRecurrenceType::AsPerDues.as_i16(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let dues_data = StandingInstructionDuesData::new(
            Some(NaiveDate::from_ymd_opt(2026, 2, 15).unwrap()),
            dec!(400.00),
            dec!(45.25),
            dec!(10.00),
            dec!(4.75),
        );
        assert_eq!(
            AccountTransferValidatorAndEngine::compute_due_amount(&dues_si, Some(&dues_data))
                .unwrap(),
            dec!(460.00)
        );
    }

    #[test]
    fn test_standing_instruction_schedule_and_execution_history() {
        let valid_from = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let details = AccountTransferDetails::savings_to_loan_transfer(
            10,
            1,
            100,
            500,
            1,
            100,
            600,
            AccountTransferType::LoanRepayment,
        );
        let mut instruction = AccountTransferStandingInstruction::new(
            55,
            "Month End Repayment",
            details.id,
            StandingInstructionPriority::Urgent,
            StandingInstructionType::Fixed,
            StandingInstructionStatus::Active,
            Some(dec!(300.00)),
            valid_from,
            None,
            AccountTransferRecurrenceType::Periodic.as_i16(),
            Some(CalendarFrequencyType::Monthly.as_int() as i16),
            Some(1),
            Some(1),
            Some(31),
            None,
        )
        .unwrap();

        // February 28, 2026 (non-leap year): clamped from day 31 -> 28
        let feb_28 = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
        assert!(AccountTransferValidatorAndEngine::is_due_on_date(
            &instruction,
            feb_28,
            None
        ));

        // Partial transfer when withdrawable is 200.00 and allow_partial_transfer = true
        let exec_time = feb_28.and_hms_opt(9, 30, 0).unwrap();
        let outcome = AccountTransferValidatorAndEngine::evaluate_standing_instruction(
            &mut instruction,
            &details,
            feb_28,
            exec_time,
            None,
            dec!(200.00),
            true,
            None,
            None,
            2,
            9001,
        )
        .unwrap();

        assert!(outcome.is_due);
        assert_eq!(outcome.status, "partial");
        assert_eq!(outcome.executed_amount, dec!(200.00));
        assert_eq!(instruction.last_run_date, Some(feb_28));
        let hist = outcome.history_record.unwrap();
        assert_eq!(hist.id, 9001);
        assert_eq!(hist.standing_instruction_id, 55);
        assert_eq!(hist.status, "partial");
        assert_eq!(hist.amount, dec!(200.00));

        // Re-running on same business date is skipped because last_run_date == feb_28
        assert!(!AccountTransferValidatorAndEngine::is_due_on_date(
            &instruction,
            feb_28,
            None
        ));
    }

    #[tokio::test]
    async fn test_command_handlers_and_repository_roundtrip() {
        let repo = Arc::new(DualDbAccountTransferRepository::new_in_memory());
        let ctx = sample_context();

        // 1. Create account transfer (Savings -> Loan)
        let create_transfer_handler = CreateAccountTransferCommandHandler::new(repo.clone());
        let transfer_cmd = JsonCommand::from_value(
            None,
            json!({
                "fromOfficeId": 1,
                "fromClientId": 10,
                "fromAccountType": 2,
                "fromAccountId": 101,
                "toOfficeId": 1,
                "toClientId": 10,
                "toAccountType": 1,
                "toAccountId": 202,
                "transferDate": "15 March 2026",
                "transferAmount": "150.75",
                "transferDescription": "Loan installment via transfer",
                "dateFormat": "dd MMMM yyyy",
                "locale": "en"
            }),
        );
        let transfer_res = create_transfer_handler
            .handle(&ctx, &transfer_cmd)
            .await
            .unwrap();
        let txn_id = transfer_res.resource_id.unwrap();
        let saved_txn = repo.get_transfer_transaction(txn_id).await.unwrap();
        assert_eq!(saved_txn.amount, dec!(150.75));
        assert!(!saved_txn.is_reversed);
        let saved_details = repo
            .get_transfer_details(saved_txn.account_transfer_details_id)
            .await
            .unwrap();
        assert_eq!(saved_details.transfer_type, AccountTransferType::LoanRepayment);
        assert_eq!(saved_details.from_savings_account_id, Some(101));
        assert_eq!(saved_details.to_loan_account_id, Some(202));

        // 2. Create standing instruction
        let create_si_handler = CreateStandingInstructionCommandHandler::new(repo.clone());
        let si_cmd = JsonCommand::from_value(
            None,
            json!({
                "fromOfficeId": 1,
                "fromClientId": 10,
                "fromAccountType": 2,
                "fromAccountId": 101,
                "toOfficeId": 1,
                "toClientId": 10,
                "toAccountType": 1,
                "toAccountId": 202,
                "transferType": 2,
                "name": "Auto Loan Repayment SI",
                "priority": 1,
                "instructionType": 1,
                "status": 1,
                "amount": 150.75,
                "validFrom": "01 March 2026",
                "recurrenceType": 1,
                "recurrenceFrequency": 2,
                "recurrenceInterval": 1,
                "recurrenceOnMonthDay": "15 March",
                "monthDayFormat": "dd MMMM",
                "dateFormat": "dd MMMM yyyy",
                "locale": "en"
            }),
        );
        let si_res = create_si_handler.handle(&ctx, &si_cmd).await.unwrap();
        let si_id = si_res.resource_id.unwrap();
        let si = repo.get_standing_instruction(si_id).await.unwrap();
        assert_eq!(si.name, "Auto Loan Repayment SI");
        assert_eq!(si.priority, StandingInstructionPriority::Urgent);
        assert_eq!(si.instruction_type, StandingInstructionType::Fixed);
        assert_eq!(si.status, StandingInstructionStatus::Active);
        assert_eq!(si.recurrence_on_month, Some(3));
        assert_eq!(si.recurrence_on_day, Some(15));

        // 3. Update standing instruction
        let update_si_handler = UpdateStandingInstructionCommandHandler::new(repo.clone());
        let update_cmd = JsonCommand::from_value(
            Some(si_id),
            json!({
                "priority": 2,
                "amount": 175.00,
                "locale": "en"
            }),
        );
        let update_res = update_si_handler.handle(&ctx, &update_cmd).await.unwrap();
        assert!(update_res.has_changes());
        let updated_si = repo.get_standing_instruction(si_id).await.unwrap();
        assert_eq!(updated_si.priority, StandingInstructionPriority::High);
        assert_eq!(updated_si.amount, Some(dec!(175.00)));

        // 4. Soft-delete standing instruction
        let delete_si_handler = DeleteStandingInstructionCommandHandler::new(repo.clone());
        let delete_cmd = JsonCommand::from_value(Some(si_id), json!({}));
        delete_si_handler.handle(&ctx, &delete_cmd).await.unwrap();
        let deleted_si = repo.get_standing_instruction(si_id).await.unwrap();
        assert_eq!(deleted_si.status, StandingInstructionStatus::Deleted);
        assert_eq!(
            deleted_si.name,
            format!("Auto Loan Repayment SI_deleted_{si_id}")
        );

        // Modifying a deleted standing instruction must fail with validation error
        assert!(update_si_handler.handle(&ctx, &update_cmd).await.is_err());
    }

    #[test]
    fn test_dual_db_sql_generation() {
        let pg_sql =
            DualDbAccountTransferRepository::insert_standing_instruction_sql(SqlDialect::Postgres);
        let mysql_sql =
            DualDbAccountTransferRepository::insert_standing_instruction_sql(SqlDialect::MySql);
        assert!(pg_sql.contains("$15"));
        assert!(!mysql_sql.contains("$1"));
        assert!(mysql_sql.contains("VALUES (?, ?, ?"));
    }
}
