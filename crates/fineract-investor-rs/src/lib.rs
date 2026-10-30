//! `fineract-investor-rs` — External Asset Owner (Investor) Loan Sale, Buyback, Settlement,
//! and Sub-Ledger Accounting.
//!
//! Faithfully ports Apache Fineract's `fineract-investor` module:
//! - `org.apache.fineract.investor.data.ExternalTransferStatus`
//! - `org.apache.fineract.investor.data.ExternalTransferSubStatus`
//! - `org.apache.fineract.investor.data.ExternalTransferRequestParameters`
//! - `org.apache.fineract.investor.domain.ExternalAssetOwner` (`m_external_asset_owner`)
//! - `org.apache.fineract.investor.domain.ExternalAssetOwnerTransfer` (`m_external_asset_owner_transfer`)
//! - `org.apache.fineract.investor.domain.ExternalAssetOwnerTransferDetails` (`m_external_asset_owner_transfer_details`)
//! - `org.apache.fineract.investor.domain.ExternalAssetOwnerJournalEntryMapping` (`m_external_asset_owner_journal_entry_mapping`)
//! - `org.apache.fineract.investor.domain.ExternalAssetOwnerTransferJournalEntryMapping` (`m_external_asset_owner_transfer_journal_entry_mapping`)
//! - `org.apache.fineract.investor.domain.ExternalAssetOwnerTransferLoanMapping` (`m_external_asset_owner_transfer_loan_mapping`)
//! - `org.apache.fineract.investor.cob.loan.LoanAccountOwnerTransferBusinessStep`
//! - `org.apache.fineract.investor.service.ExternalAssetOwnersWriteServiceImpl`
//! - `org.apache.fineract.investor.service.LoanAccountOwnerTransferServiceImpl`
//! - `org.apache.fineract.investor.service.AccountingServiceImpl`
//! - `org.apache.fineract.investor.accounting.journalentry.service.InvestorAccountingHelper`
//! - `org.apache.fineract.investor.enricher.LoanAccountDataV1Enricher`
//! - `org.apache.fineract.investor.domain.LoanOwnershipTransferBusinessEvent`
//! - `org.apache.fineract.investor.service.serialization.serializer.investor.InvestorBusinessEventSerializer`

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use fineract_avro_rs::{
    CurrencyDataV1, LoanAccountDataV1, LoanChargeDataV1, LoanOwnershipTransferDataV1,
    UnpaidChargeDataV1, LOAN_ACCOUNT_DATA_V1_AVSC, LOAN_OWNERSHIP_TRANSFER_DATA_V1_AVSC,
};
use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{DialectSql, ExecutionContext, ExternalId, JsonCommand, SqlDialect};
use fineract_validation_rs::{
    DataValidatorBuilder, FineractError, FineractResult, PlatformDomainRuleException,
};

/// Sentinel open-ended effective date (`9999-12-31`) used by Fineract for active/pending transfer records.
///
/// Ported from `LoanAccountOwnerTransferBusinessStep.FUTURE_DATE_9999_12_31`.
pub const FUTURE_DATE_9999_12_31: NaiveDate = match NaiveDate::from_ymd_opt(9999, 12, 31) {
    Some(date) => date,
    None => unreachable!(),
};

/// Prefix applied to investor transfer journal entry transaction IDs (`InvestorAccountingHelper.INVESTOR_TRANSFER_IDENTIFIER`).
pub const INVESTOR_TRANSFER_IDENTIFIER: &str = "I";

// =============================================================================
// 1. Core Investor Enumerations
// =============================================================================

/// Lifecycle status of an external asset owner transfer (`m_external_asset_owner_transfer.status`).
///
/// Ported from `org.apache.fineract.investor.data.ExternalTransferStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalTransferStatus {
    #[default]
    Pending,
    Active,
    Declined,
    Buyback,
    Cancelled,
    PendingIntermediate,
    ActiveIntermediate,
    BuybackIntermediate,
}

impl ExternalTransferStatus {
    pub const PENDING: Self = Self::Pending;
    pub const ACTIVE: Self = Self::Active;
    pub const DECLINED: Self = Self::Declined;
    pub const BUYBACK: Self = Self::Buyback;
    pub const CANCELLED: Self = Self::Cancelled;
    pub const PENDING_INTERMEDIATE: Self = Self::PendingIntermediate;
    pub const ACTIVE_INTERMEDIATE: Self = Self::ActiveIntermediate;
    pub const BUYBACK_INTERMEDIATE: Self = Self::BuybackIntermediate;

    /// Returns the canonical database/Java enum name (`ExternalTransferStatus.name()`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Active => "ACTIVE",
            Self::Declined => "DECLINED",
            Self::Buyback => "BUYBACK",
            Self::Cancelled => "CANCELLED",
            Self::PendingIntermediate => "PENDING_INTERMEDIATE",
            Self::ActiveIntermediate => "ACTIVE_INTERMEDIATE",
            Self::BuybackIntermediate => "BUYBACK_INTERMEDIATE",
        }
    }

    /// Alias for [`ExternalTransferStatus::as_str`].
    pub const fn code(self) -> &'static str {
        self.as_str()
    }

    /// Alias for [`ExternalTransferStatus::as_str`].
    pub const fn java_name(self) -> &'static str {
        self.as_str()
    }

    /// Parses a string into [`ExternalTransferStatus`].
    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "PENDING" => Some(Self::Pending),
            "ACTIVE" => Some(Self::Active),
            "DECLINED" => Some(Self::Declined),
            "BUYBACK" => Some(Self::Buyback),
            "CANCELLED" | "CANCELED" => Some(Self::Cancelled),
            "PENDING_INTERMEDIATE" | "PENDINGINTERMEDIATE" => Some(Self::PendingIntermediate),
            "ACTIVE_INTERMEDIATE" | "ACTIVEINTERMEDIATE" => Some(Self::ActiveIntermediate),
            "BUYBACK_INTERMEDIATE" | "BUYBACKINTERMEDIATE" => Some(Self::BuybackIntermediate),
            _ => None,
        }
    }

    /// Returns `true` if `self` is `Pending` or `PendingIntermediate`.
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending | Self::PendingIntermediate)
    }

    /// Returns `true` if `self` is `Active` or `ActiveIntermediate`.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active | Self::ActiveIntermediate)
    }

    /// Returns `true` if `self` is `Declined`.
    pub const fn is_declined(self) -> bool {
        matches!(self, Self::Declined)
    }

    /// Returns `true` if `self` is `Buyback` or `BuybackIntermediate`.
    pub const fn is_buyback(self) -> bool {
        matches!(self, Self::Buyback | Self::BuybackIntermediate)
    }

    /// Returns `true` if `self` is `Cancelled`.
    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Returns `true` if the transfer has been executed (`Active`, `ActiveIntermediate`, `Buyback`, or `BuybackIntermediate`).
    pub const fn is_executed(self) -> bool {
        matches!(
            self,
            Self::Active | Self::ActiveIntermediate | Self::Buyback | Self::BuybackIntermediate
        )
    }

    /// Maps the internal status to the Avro `transferStatus` string (`InvestorBusinessEventSerializer.getStatus`).
    pub const fn avro_transfer_status(self) -> &'static str {
        match self {
            Self::Active
            | Self::ActiveIntermediate
            | Self::Buyback
            | Self::BuybackIntermediate => "EXECUTED",
            Self::Declined => "DECLINED",
            Self::Cancelled => "CANCELLED",
            Self::Pending | Self::PendingIntermediate => "UNKNOWN",
        }
    }
}

impl fmt::Display for ExternalTransferStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ExternalTransferStatus {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.external.asset.owner.status.invalid",
                format!("Invalid external transfer status: {s}"),
            )
        })
    }
}

/// Sub-status reason for a declined or cancelled external asset owner transfer (`m_external_asset_owner_transfer.sub_status`).
///
/// Ported from `org.apache.fineract.investor.data.ExternalTransferSubStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExternalTransferSubStatus {
    #[serde(rename = "BALANCE_ZERO")]
    BalanceZero,
    #[serde(rename = "BALANCE_NEGATIVE")]
    BalanceNegative,
    #[serde(rename = "SAMEDAY_TRANSFERS", alias = "SAM_DAY_TRANSFER", alias = "SAME_DAY_TRANSFER")]
    SamDayTransfer,
    #[serde(rename = "USER_REQUESTED")]
    UserRequested,
    #[serde(rename = "UNSOLD")]
    Unsold,
}

#[allow(non_upper_case_globals)]
impl ExternalTransferSubStatus {
    pub const BALANCE_ZERO: Self = Self::BalanceZero;
    pub const BALANCE_NEGATIVE: Self = Self::BalanceNegative;
    pub const SAM_DAY_TRANSFER: Self = Self::SamDayTransfer;
    pub const SAME_DAY_TRANSFER: Self = Self::SamDayTransfer;
    pub const SAMEDAY_TRANSFERS: Self = Self::SamDayTransfer;
    pub const SameDayTransfer: Self = Self::SamDayTransfer;
    pub const SameDayTransfers: Self = Self::SamDayTransfer;
    pub const USER_REQUESTED: Self = Self::UserRequested;
    pub const UNSOLD: Self = Self::Unsold;

    /// Returns the canonical database/Java enum name (`ExternalTransferSubStatus.name()`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BalanceZero => "BALANCE_ZERO",
            Self::BalanceNegative => "BALANCE_NEGATIVE",
            Self::SamDayTransfer => "SAMEDAY_TRANSFERS",
            Self::UserRequested => "USER_REQUESTED",
            Self::Unsold => "UNSOLD",
        }
    }

    /// Alias for [`ExternalTransferSubStatus::as_str`].
    pub const fn code(self) -> &'static str {
        self.as_str()
    }

    /// Alias for [`ExternalTransferSubStatus::as_str`].
    pub const fn java_name(self) -> &'static str {
        self.as_str()
    }

    /// Parses a string into [`ExternalTransferSubStatus`].
    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "BALANCE_ZERO" | "BALANCEZERO" => Some(Self::BalanceZero),
            "BALANCE_NEGATIVE" | "BALANCENEGATIVE" => Some(Self::BalanceNegative),
            "SAMEDAY_TRANSFERS"
            | "SAMEDAY_TRANSFER"
            | "SAME_DAY_TRANSFER"
            | "SAME_DAY_TRANSFERS"
            | "SAM_DAY_TRANSFER"
            | "SAMDAYTRANSFER"
            | "SAMEDAYTRANSFER"
            | "SAMEDAYTRANSFERS" => Some(Self::SamDayTransfer),
            "USER_REQUESTED" | "USERREQUESTED" => Some(Self::UserRequested),
            "UNSOLD" => Some(Self::Unsold),
            _ => None,
        }
    }

    pub const fn is_balance_zero(self) -> bool {
        matches!(self, Self::BalanceZero)
    }

    pub const fn is_balance_negative(self) -> bool {
        matches!(self, Self::BalanceNegative)
    }

    pub const fn is_same_day_transfer(self) -> bool {
        matches!(self, Self::SamDayTransfer)
    }

    pub const fn is_user_requested(self) -> bool {
        matches!(self, Self::UserRequested)
    }

    pub const fn is_unsold(self) -> bool {
        matches!(self, Self::Unsold)
    }
}

impl fmt::Display for ExternalTransferSubStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ExternalTransferSubStatus {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.external.asset.owner.sub.status.invalid",
                format!("Invalid external transfer sub-status: {s}"),
            )
        })
    }
}

/// Request type initiating an external asset owner transfer (`SALE` or `BUYBACK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalTransferRequestType {
    #[default]
    Sale,
    Buyback,
    IntermediarySale,
}

impl ExternalTransferRequestType {
    pub const SALE: Self = Self::Sale;
    pub const BUYBACK: Self = Self::Buyback;
    pub const INTERMEDIARY_SALE: Self = Self::IntermediarySale;

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sale => "SALE",
            Self::Buyback => "BUYBACK",
            Self::IntermediarySale => "INTERMEDIARYSALE",
        }
    }

    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "SALE" => Some(Self::Sale),
            "BUYBACK" => Some(Self::Buyback),
            "INTERMEDIARYSALE" | "INTERMEDIARY_SALE" | "INTERMEDIARY-SALE" => {
                Some(Self::IntermediarySale)
            }
            _ => None,
        }
    }

    pub const fn is_sale(self) -> bool {
        matches!(self, Self::Sale)
    }

    pub const fn is_buyback(self) -> bool {
        matches!(self, Self::Buyback)
    }

    pub const fn is_intermediary_sale(self) -> bool {
        matches!(self, Self::IntermediarySale)
    }
}

impl fmt::Display for ExternalTransferRequestType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// =============================================================================
// 2. Domain Entities (`m_external_asset_owner*`)
// =============================================================================

/// External Asset Owner (Investor) entity matching `m_external_asset_owner`.
///
/// Ported from `org.apache.fineract.investor.domain.ExternalAssetOwner`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAssetOwner {
    pub id: i64,
    pub external_id: ExternalId,
}

impl ExternalAssetOwner {
    pub fn new(id: i64, external_id: impl Into<ExternalId>) -> Self {
        Self {
            id,
            external_id: external_id.into(),
        }
    }

    pub fn external_id_str(&self) -> Option<&str> {
        self.external_id.value()
    }
}

/// External Asset Owner Transfer entity matching `m_external_asset_owner_transfer`.
///
/// Ported from `org.apache.fineract.investor.domain.ExternalAssetOwnerTransfer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAssetOwnerTransfer {
    pub id: i64,
    pub owner_id: i64,
    pub external_id: ExternalId,
    pub status: ExternalTransferStatus,
    pub sub_status: Option<ExternalTransferSubStatus>,
    pub purchase_price_ratio: Decimal,
    pub settlement_date: NaiveDate,
    pub effective_date_from: NaiveDate,
    pub effective_date_to: NaiveDate,
    pub loan_id: i64,
    pub loan_account_number: Option<String>,
}

impl Default for ExternalAssetOwnerTransfer {
    fn default() -> Self {
        Self {
            id: 0,
            owner_id: 0,
            external_id: ExternalId::empty(),
            status: ExternalTransferStatus::Pending,
            sub_status: None,
            purchase_price_ratio: Decimal::ONE,
            settlement_date: NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
            effective_date_from: NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
            effective_date_to: FUTURE_DATE_9999_12_31,
            loan_id: 0,
            loan_account_number: None,
        }
    }
}

impl ExternalAssetOwnerTransfer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        owner_id: i64,
        external_id: impl Into<ExternalId>,
        status: ExternalTransferStatus,
        sub_status: Option<ExternalTransferSubStatus>,
        purchase_price_ratio: Decimal,
        settlement_date: NaiveDate,
        effective_date_from: NaiveDate,
        effective_date_to: NaiveDate,
        loan_id: i64,
        loan_account_number: Option<String>,
    ) -> Self {
        Self {
            id,
            owner_id,
            external_id: external_id.into(),
            status,
            sub_status,
            purchase_price_ratio,
            settlement_date,
            effective_date_from,
            effective_date_to,
            loan_id,
            loan_account_number,
        }
    }

    /// Returns `true` if `effective_date_to == 9999-12-31`.
    pub fn is_open_ended(&self) -> bool {
        self.effective_date_to == FUTURE_DATE_9999_12_31
    }

    /// Returns `true` if this transfer is currently effective on `business_date`.
    pub fn is_effective_on(&self, business_date: NaiveDate) -> bool {
        self.effective_date_to == FUTURE_DATE_9999_12_31
            || (business_date >= self.effective_date_from && business_date < self.effective_date_to)
    }

    /// Expires this transfer record by setting `effective_date_to = settlement_date`.
    pub fn expire(&mut self, settlement_date: NaiveDate) {
        self.effective_date_to = settlement_date;
    }

    /// Infers the request type (`Sale`, `Buyback`, or `IntermediarySale`) from the transfer status.
    pub const fn request_type(&self) -> ExternalTransferRequestType {
        match self.status {
            ExternalTransferStatus::Buyback | ExternalTransferStatus::BuybackIntermediate => {
                ExternalTransferRequestType::Buyback
            }
            ExternalTransferStatus::PendingIntermediate
            | ExternalTransferStatus::ActiveIntermediate => {
                ExternalTransferRequestType::IntermediarySale
            }
            _ => ExternalTransferRequestType::Sale,
        }
    }
}

/// Monetary breakdown captured at settlement (`m_external_asset_owner_transfer_details`).
///
/// Ported from `org.apache.fineract.investor.domain.ExternalAssetOwnerTransferDetails`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAssetOwnerTransferDetails {
    pub id: i64,
    pub asset_owner_transfer_id: i64,
    pub details_id: i64,
    pub total_outstanding: Decimal,
    pub total_principal_outstanding: Decimal,
    pub total_interest_outstanding: Decimal,
    pub total_fee_charges_outstanding: Decimal,
    pub total_penalty_charges_outstanding: Decimal,
    pub total_overpaid: Decimal,
}

impl ExternalAssetOwnerTransferDetails {
    /// Creates a new [`ExternalAssetOwnerTransferDetails`] and automatically computes `total_outstanding`
    /// as the sum of principal, interest, fee charges, and penalty charges outstanding, matching
    /// `ExternalAssetOwnerTransferDetails.updateTotalOutstanding()`.
    pub fn new(
        id: i64,
        asset_owner_transfer_id: i64,
        total_principal_outstanding: Decimal,
        total_interest_outstanding: Decimal,
        total_fee_charges_outstanding: Decimal,
        total_penalty_charges_outstanding: Decimal,
        total_overpaid: Decimal,
    ) -> Self {
        let total_outstanding = total_principal_outstanding
            + total_interest_outstanding
            + total_fee_charges_outstanding
            + total_penalty_charges_outstanding;
        Self {
            id,
            asset_owner_transfer_id,
            details_id: id,
            total_outstanding,
            total_principal_outstanding,
            total_interest_outstanding,
            total_fee_charges_outstanding,
            total_penalty_charges_outstanding,
            total_overpaid,
        }
    }

    /// Recomputes `total_outstanding = principal + interest + fees + penalties`.
    pub fn recalculate_total_outstanding(&mut self) {
        self.total_outstanding = self.total_principal_outstanding
            + self.total_interest_outstanding
            + self.total_fee_charges_outstanding
            + self.total_penalty_charges_outstanding;
    }

    pub fn set_total_principal_outstanding(&mut self, value: Decimal) {
        self.total_principal_outstanding = value;
        self.recalculate_total_outstanding();
    }

    pub fn set_total_interest_outstanding(&mut self, value: Decimal) {
        self.total_interest_outstanding = value;
        self.recalculate_total_outstanding();
    }

    pub fn set_total_fee_charges_outstanding(&mut self, value: Decimal) {
        self.total_fee_charges_outstanding = value;
        self.recalculate_total_outstanding();
    }

    pub fn set_total_penalty_charges_outstanding(&mut self, value: Decimal) {
        self.total_penalty_charges_outstanding = value;
        self.recalculate_total_outstanding();
    }

    pub fn set_total_overpaid(&mut self, value: Decimal) {
        self.total_overpaid = value;
    }
}

/// Sub-ledger mapping between a GL journal entry and an external asset owner (`m_external_asset_owner_journal_entry_mapping`).
///
/// Ported from `org.apache.fineract.investor.domain.ExternalAssetOwnerJournalEntryMapping`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAssetOwnerJournalEntryMapping {
    pub id: i64,
    pub journal_entry_id: i64,
    pub owner_id: i64,
}

impl ExternalAssetOwnerJournalEntryMapping {
    pub const fn new(id: i64, journal_entry_id: i64, owner_id: i64) -> Self {
        Self {
            id,
            journal_entry_id,
            owner_id,
        }
    }
}

/// Mapping between a GL journal entry and an external asset owner transfer (`m_external_asset_owner_transfer_journal_entry_mapping`).
///
/// Ported from `org.apache.fineract.investor.domain.ExternalAssetOwnerTransferJournalEntryMapping`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAssetOwnerTransferJournalEntryMapping {
    pub id: i64,
    pub journal_entry_id: i64,
    pub owner_transfer_id: i64,
}

impl ExternalAssetOwnerTransferJournalEntryMapping {
    pub const fn new(id: i64, journal_entry_id: i64, owner_transfer_id: i64) -> Self {
        Self {
            id,
            journal_entry_id,
            owner_transfer_id,
        }
    }
}

/// Active ownership mapping between a loan and an external asset owner transfer (`m_external_asset_owner_transfer_loan_mapping`).
///
/// Ported from `org.apache.fineract.investor.domain.ExternalAssetOwnerTransferLoanMapping`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAssetOwnerTransferLoanMapping {
    pub id: i64,
    pub loan_id: i64,
    pub owner_transfer_id: i64,
}

impl ExternalAssetOwnerTransferLoanMapping {
    pub const fn new(id: i64, loan_id: i64, owner_transfer_id: i64) -> Self {
        Self {
            id,
            loan_id,
            owner_transfer_id,
        }
    }
}

// =============================================================================
// 3. Domain Exceptions & Request Parameters / Validator
// =============================================================================

/// Exception raised when initiating or cancelling an external asset owner transfer violates domain rules.
///
/// Ported from `org.apache.fineract.investor.exception.ExternalAssetOwnerInitiateTransferException`
/// and `ExternalAssetOwnerInitiateTransferExceptionMapper`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct ExternalAssetOwnerInitiateTransferException {
    pub message: String,
}

impl ExternalAssetOwnerInitiateTransferException {
    pub const CODE: &'static str = "error.msg.external.asset.owner.initiate";

    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        PlatformDomainRuleException::new(Self::CODE, self.message)
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

impl From<ExternalAssetOwnerInitiateTransferException> for FineractError {
    fn from(value: ExternalAssetOwnerInitiateTransferException) -> Self {
        value.into_fineract_error()
    }
}

/// Exception raised when creating an `ExternalAssetOwner` with a duplicate `external_id`.
///
/// Ported from `org.apache.fineract.investor.exception.ExternalAssetOwnerDuplicateException`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Provided external id already exists with Id {external_id}")]
pub struct ExternalAssetOwnerDuplicateException {
    pub external_id: String,
}

impl ExternalAssetOwnerDuplicateException {
    pub const CODE: &'static str = "error.msg.provided.external.id.already.exists";

    pub fn new(external_id: impl Into<String>) -> Self {
        Self {
            external_id: external_id.into(),
        }
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::domain_rule(Self::CODE, self.to_string())
    }
}

impl From<ExternalAssetOwnerDuplicateException> for FineractError {
    fn from(value: ExternalAssetOwnerDuplicateException) -> Self {
        value.into_fineract_error()
    }
}

/// Canonical JSON parameter names for External Asset Owner API requests.
///
/// Ported from `org.apache.fineract.investor.data.ExternalTransferRequestParameters`.
pub struct ExternalTransferRequestParameters;

impl ExternalTransferRequestParameters {
    pub const SETTLEMENT_DATE: &'static str = "settlementDate";
    pub const OWNER_EXTERNAL_ID: &'static str = "ownerExternalId";
    pub const TRANSFER_EXTERNAL_ID: &'static str = "transferExternalId";
    pub const TRANSFER_EXTERNAL_GROUP_ID: &'static str = "transferExternalGroupId";
    pub const PURCHASE_PRICE_RATIO: &'static str = "purchasePriceRatio";
    pub const DATEFORMAT: &'static str = "dateFormat";
    pub const LOCALE: &'static str = "locale";

    pub const SALE_PARAMETERS: &'static [&'static str] = &[
        Self::SETTLEMENT_DATE,
        Self::OWNER_EXTERNAL_ID,
        Self::TRANSFER_EXTERNAL_ID,
        Self::TRANSFER_EXTERNAL_GROUP_ID,
        Self::PURCHASE_PRICE_RATIO,
        Self::DATEFORMAT,
        Self::LOCALE,
    ];

    pub const BUYBACK_PARAMETERS: &'static [&'static str] = &[
        Self::SETTLEMENT_DATE,
        Self::TRANSFER_EXTERNAL_ID,
        Self::DATEFORMAT,
        Self::LOCALE,
    ];
}

/// Validator for External Asset Owner API payloads.
///
/// Ported from `ExternalAssetOwnersWriteServiceImpl` and `ExternalAssetOwnerValidator`.
pub struct ExternalAssetOwnerValidator;

impl ExternalAssetOwnerValidator {
    pub fn validate_for_sale(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(
            ExternalTransferRequestParameters::SALE_PARAMETERS,
        )?;
        let owner_external_id = command
            .string_value_of_parameter_named(ExternalTransferRequestParameters::OWNER_EXTERNAL_ID);
        let transfer_external_id = command.string_value_of_parameter_named(
            ExternalTransferRequestParameters::TRANSFER_EXTERNAL_ID,
        );
        let purchase_price_ratio_str = command
            .string_value_of_parameter_named(ExternalTransferRequestParameters::PURCHASE_PRICE_RATIO);
        let settlement_date = command
            .local_date_value_of_parameter_named(ExternalTransferRequestParameters::SETTLEMENT_DATE)?;
        let group_id = command.string_value_of_parameter_named(
            ExternalTransferRequestParameters::TRANSFER_EXTERNAL_GROUP_ID,
        );

        let v = DataValidatorBuilder::for_resource("loantransfer")
            .reset()
            .parameter(ExternalTransferRequestParameters::OWNER_EXTERNAL_ID)
            .value(owner_external_id.as_deref())
            .not_blank()
            .not_exceeding_length_of(100)
            .reset()
            .parameter(ExternalTransferRequestParameters::TRANSFER_EXTERNAL_ID)
            .value(transfer_external_id.as_deref())
            .ignore_if_null()
            .not_exceeding_length_of(100)
            .reset()
            .parameter(ExternalTransferRequestParameters::PURCHASE_PRICE_RATIO)
            .value(purchase_price_ratio_str.as_deref())
            .not_blank()
            .not_exceeding_length_of(50)
            .reset()
            .parameter(ExternalTransferRequestParameters::SETTLEMENT_DATE)
            .value(settlement_date)
            .not_null()
            .reset()
            .parameter(ExternalTransferRequestParameters::TRANSFER_EXTERNAL_GROUP_ID)
            .value(group_id.as_deref())
            .ignore_if_null()
            .not_exceeding_length_of(100);

        v.throw_validation_errors()?;
        Ok(())
    }

    pub fn validate_for_buyback(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(
            ExternalTransferRequestParameters::BUYBACK_PARAMETERS,
        )?;
        let transfer_external_id = command.string_value_of_parameter_named(
            ExternalTransferRequestParameters::TRANSFER_EXTERNAL_ID,
        );
        let settlement_date = command
            .local_date_value_of_parameter_named(ExternalTransferRequestParameters::SETTLEMENT_DATE)?;

        let v = DataValidatorBuilder::for_resource("loantransfer")
            .reset()
            .parameter(ExternalTransferRequestParameters::TRANSFER_EXTERNAL_ID)
            .value(transfer_external_id.as_deref())
            .ignore_if_null()
            .not_exceeding_length_of(100)
            .reset()
            .parameter(ExternalTransferRequestParameters::SETTLEMENT_DATE)
            .value(settlement_date)
            .not_null();

        v.throw_validation_errors()?;
        Ok(())
    }
}

// =============================================================================
// 4. Investor Sub-Ledger GL Journal Entries & Avro Business Events
// =============================================================================

/// Debit/Credit classification for an investor GL journal entry (`acc_gl_journal_entry.type_enum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum InvestorJournalEntryType {
    Credit = 1,
    Debit = 2,
}

impl InvestorJournalEntryType {
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn is_debit(self) -> bool {
        matches!(self, Self::Debit)
    }

    pub const fn is_credit(self) -> bool {
        matches!(self, Self::Credit)
    }
}

/// GL Account mapping configuration used by [`ExternalAssetOwnerTransferEngine`] to generate
/// balanced double-entry journal postings during Sale and Buyback settlement.
///
/// Ported from `AccountingServiceImpl` and `InvestorAccountingHelper`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestorGlAccountConfig {
    /// `AccountingConstants.FinancialActivity.ASSET_TRANSFER` (100) clearing GL account.
    pub asset_transfer_gl_account_id: i64,
    /// `AccrualAccountsForLoan.LOAN_PORTFOLIO` (2).
    pub loan_portfolio_gl_account_id: i64,
    /// `AccrualAccountsForLoan.INTEREST_RECEIVABLE` (7).
    pub interest_receivable_gl_account_id: i64,
    /// `AccrualAccountsForLoan.FEES_RECEIVABLE` (8).
    pub fees_receivable_gl_account_id: i64,
    /// `AccrualAccountsForLoan.PENALTIES_RECEIVABLE` (9).
    pub penalties_receivable_gl_account_id: i64,
    /// `AccrualAccountsForLoan.OVERPAYMENT` (11).
    pub overpayment_gl_account_id: i64,
    /// `AccrualAccountsForLoan.CHARGE_OFF_EXPENSE` (16).
    pub charge_off_expense_gl_account_id: i64,
    /// `AccrualAccountsForLoan.CHARGE_OFF_FRAUD_EXPENSE` (17).
    pub charge_off_fraud_expense_gl_account_id: i64,
    /// `AccrualAccountsForLoan.INCOME_FROM_CHARGE_OFF_INTEREST` (14).
    pub income_from_charge_off_interest_gl_account_id: i64,
    /// `AccrualAccountsForLoan.INCOME_FROM_CHARGE_OFF_FEES` (15).
    pub income_from_charge_off_fees_gl_account_id: i64,
    /// `AccrualAccountsForLoan.INCOME_FROM_CHARGE_OFF_PENALTY` (18).
    pub income_from_charge_off_penalty_gl_account_id: i64,
}

impl Default for InvestorGlAccountConfig {
    fn default() -> Self {
        Self {
            asset_transfer_gl_account_id: 100,
            loan_portfolio_gl_account_id: 1102,
            interest_receivable_gl_account_id: 1107,
            fees_receivable_gl_account_id: 1108,
            penalties_receivable_gl_account_id: 1109,
            overpayment_gl_account_id: 2111,
            charge_off_expense_gl_account_id: 5116,
            charge_off_fraud_expense_gl_account_id: 5117,
            income_from_charge_off_interest_gl_account_id: 4114,
            income_from_charge_off_fees_gl_account_id: 4115,
            income_from_charge_off_penalty_gl_account_id: 4118,
        }
    }
}

/// Persistent GL journal entry posted during an investor asset transfer (`acc_gl_journal_entry`).
///
/// Ported from `InvestorAccountingHelper` and `AccountingServiceImpl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestorJournalEntry {
    pub id: i64,
    pub gl_account_id: i64,
    pub office_id: i64,
    pub currency_code: String,
    pub transaction_id: String,
    pub entry_date: NaiveDate,
    pub entry_type: InvestorJournalEntryType,
    pub amount: Decimal,
    pub loan_id: i64,
    pub transfer_id: i64,
    pub owner_id: Option<i64>,
    pub manual_entry: bool,
}

impl InvestorJournalEntry {
    pub const fn is_debit(&self) -> bool {
        self.entry_type.is_debit()
    }

    pub const fn is_credit(&self) -> bool {
        self.entry_type.is_credit()
    }
}

/// Loan balance & metadata snapshot consumed by [`ExternalAssetOwnerTransferEngine`] during COB settlement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanTransferSnapshot {
    pub loan_id: i64,
    pub loan_account_number: Option<String>,
    pub loan_external_id: Option<ExternalId>,
    pub loan_product_id: i64,
    pub office_id: i64,
    pub currency_code: String,
    pub currency_digits: u32,
    pub currency_in_multiples_of: Option<i32>,
    pub total_principal_outstanding: Decimal,
    pub total_interest_outstanding: Decimal,
    pub total_fee_charges_outstanding: Decimal,
    pub total_penalty_charges_outstanding: Decimal,
    pub total_overpaid: Decimal,
    pub charged_off: bool,
    pub fraud: bool,
    pub unpaid_charges: Vec<UnpaidChargeDataV1>,
}

impl Default for LoanTransferSnapshot {
    fn default() -> Self {
        Self {
            loan_id: 1,
            loan_account_number: Some("000000001".to_string()),
            loan_external_id: None,
            loan_product_id: 1,
            office_id: 1,
            currency_code: "USD".to_string(),
            currency_digits: 2,
            currency_in_multiples_of: None,
            total_principal_outstanding: Decimal::ZERO,
            total_interest_outstanding: Decimal::ZERO,
            total_fee_charges_outstanding: Decimal::ZERO,
            total_penalty_charges_outstanding: Decimal::ZERO,
            total_overpaid: Decimal::ZERO,
            charged_off: false,
            fraud: false,
            unpaid_charges: Vec::new(),
        }
    }
}

impl LoanTransferSnapshot {
    pub fn new(
        loan_id: i64,
        total_principal_outstanding: Decimal,
        total_interest_outstanding: Decimal,
        total_fee_charges_outstanding: Decimal,
        total_penalty_charges_outstanding: Decimal,
        total_overpaid: Decimal,
    ) -> Self {
        Self {
            loan_id,
            loan_account_number: Some(format!("{loan_id:09}")),
            total_principal_outstanding,
            total_interest_outstanding,
            total_fee_charges_outstanding,
            total_penalty_charges_outstanding,
            total_overpaid,
            ..Self::default()
        }
    }

    /// Computes `total_outstanding = principal + interest + fees + penalties`.
    pub fn total_outstanding(&self) -> Decimal {
        self.total_principal_outstanding
            + self.total_interest_outstanding
            + self.total_fee_charges_outstanding
            + self.total_penalty_charges_outstanding
    }
}

/// Business event emitted when a loan ownership transfer settles, is declined, or is cancelled.
///
/// Ported from `org.apache.fineract.investor.domain.LoanOwnershipTransferBusinessEvent`
/// and `org.apache.fineract.investor.service.serialization.serializer.investor.InvestorBusinessEventSerializer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanOwnershipTransferBusinessEvent {
    pub event_type: String,
    pub category: String,
    pub schema_name: String,
    pub loan_id: i64,
    pub transfer_id: i64,
    pub transfer_status: ExternalTransferStatus,
    pub transfer_sub_status: Option<ExternalTransferSubStatus>,
    pub payload: LoanOwnershipTransferDataV1,
    pub loan_account_data: LoanAccountDataV1,
}

impl LoanOwnershipTransferBusinessEvent {
    pub const TYPE: &'static str = "LoanOwnershipTransferBusinessEvent";
    pub const CATEGORY: &'static str = "LOAN";
    pub const SCHEMA_NAME: &'static str =
        "org.apache.fineract.avro.loan.v1.LoanOwnershipTransferDataV1";

    pub fn to_avro_dto(&self) -> &LoanOwnershipTransferDataV1 {
        &self.payload
    }

    pub fn loan_account_data(&self) -> &LoanAccountDataV1 {
        &self.loan_account_data
    }

    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.payload).unwrap_or_default()
    }
}

/// Complete result of executing the COB loan ownership transfer step for a loan on `business_date`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobTransferSettlementOutcome {
    pub loan_id: i64,
    pub business_date: NaiveDate,
    pub settled_transfers: Vec<ExternalAssetOwnerTransfer>,
    pub transfer_details: Vec<ExternalAssetOwnerTransferDetails>,
    pub active_loan_mapping: Option<ExternalAssetOwnerTransferLoanMapping>,
    pub journal_entries: Vec<InvestorJournalEntry>,
    pub owner_journal_mappings: Vec<ExternalAssetOwnerJournalEntryMapping>,
    pub transfer_journal_mappings: Vec<ExternalAssetOwnerTransferJournalEntryMapping>,
    pub business_events: Vec<LoanOwnershipTransferBusinessEvent>,
    pub loan_account_data: LoanAccountDataV1,
}

impl CobTransferSettlementOutcome {
    /// Returns the total debits posted across all generated GL journal entries.
    pub fn total_debits(&self) -> Decimal {
        self.journal_entries
            .iter()
            .filter(|e| e.is_debit())
            .map(|e| e.amount)
            .sum()
    }

    /// Returns the total credits posted across all generated GL journal entries.
    pub fn total_credits(&self) -> Decimal {
        self.journal_entries
            .iter()
            .filter(|e| e.is_credit())
            .map(|e| e.amount)
            .sum()
    }

    /// Returns `true` if total debits equal total credits.
    pub fn is_balanced(&self) -> bool {
        self.total_debits() == self.total_credits()
    }
}

// =============================================================================
// 5. Dual-Database Investor Repository (`DualDbInvestorRepository`)
// =============================================================================

/// Dual-dialect (`PostgreSQL` + `MySQL/MariaDB`) repository for External Asset Owner entities,
/// transfers, transfer details, active loan ownership mappings, and sub-ledger journal mappings.
#[derive(Debug)]
pub struct DualDbInvestorRepository {
    dialect: SqlDialect,
    pool: Option<sqlx::AnyPool>,
    next_owner_id: AtomicI64,
    next_transfer_id: AtomicI64,
    next_details_id: AtomicI64,
    next_loan_mapping_id: AtomicI64,
    next_journal_entry_id: AtomicI64,
    next_owner_je_mapping_id: AtomicI64,
    next_transfer_je_mapping_id: AtomicI64,
    owners: DashMap<i64, ExternalAssetOwner>,
    owners_by_ext_id: DashMap<String, i64>,
    transfers: DashMap<i64, ExternalAssetOwnerTransfer>,
    transfer_details: DashMap<i64, ExternalAssetOwnerTransferDetails>,
    loan_mappings: DashMap<i64, ExternalAssetOwnerTransferLoanMapping>,
    journal_entries: DashMap<i64, InvestorJournalEntry>,
    owner_journal_mappings: DashMap<i64, ExternalAssetOwnerJournalEntryMapping>,
    transfer_journal_mappings: DashMap<i64, ExternalAssetOwnerTransferJournalEntryMapping>,
    business_events: DashMap<i64, Vec<LoanOwnershipTransferBusinessEvent>>,
}

impl Default for DualDbInvestorRepository {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl DualDbInvestorRepository {
    pub const OWNER_COLUMNS: &'static str = "id, external_id";
    pub const TRANSFER_COLUMNS: &'static str = "id, owner_id, external_id, status, sub_status, \
        purchase_price_ratio, settlement_date, effective_date_from, effective_date_to, loan_id";
    pub const TRANSFER_DETAILS_COLUMNS: &'static str = "id, asset_owner_transfer_id, \
        total_outstanding_derived, principal_outstanding_derived, interest_outstanding_derived, \
        fee_charges_outstanding_derived, penalty_charges_outstanding_derived, total_overpaid_derived";
    pub const LOAN_MAPPING_COLUMNS: &'static str = "id, loan_id, owner_transfer_id";
    pub const OWNER_JE_MAPPING_COLUMNS: &'static str = "id, journal_entry_id, owner_id";
    pub const TRANSFER_JE_MAPPING_COLUMNS: &'static str =
        "id, journal_entry_id, owner_transfer_id";

    pub fn new(pool: sqlx::AnyPool, dialect: SqlDialect) -> Self {
        Self {
            dialect,
            pool: Some(pool),
            ..Self::with_dialect(dialect)
        }
    }

    pub fn in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    pub fn new_in_memory() -> Self {
        Self::in_memory()
    }

    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            pool: None,
            next_owner_id: AtomicI64::new(1),
            next_transfer_id: AtomicI64::new(1),
            next_details_id: AtomicI64::new(1),
            next_loan_mapping_id: AtomicI64::new(1),
            next_journal_entry_id: AtomicI64::new(1),
            next_owner_je_mapping_id: AtomicI64::new(1),
            next_transfer_je_mapping_id: AtomicI64::new(1),
            owners: DashMap::new(),
            owners_by_ext_id: DashMap::new(),
            transfers: DashMap::new(),
            transfer_details: DashMap::new(),
            loan_mappings: DashMap::new(),
            journal_entries: DashMap::new(),
            owner_journal_mappings: DashMap::new(),
            transfer_journal_mappings: DashMap::new(),
            business_events: DashMap::new(),
        }
    }

    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub const fn pool(&self) -> Option<&sqlx::AnyPool> {
        self.pool.as_ref()
    }

    // --- Dialect-aware SQL builders ---

    pub fn select_owner_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_external_asset_owner WHERE id = ?",
                Self::OWNER_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_owner_by_external_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_external_asset_owner WHERE external_id = ?",
                Self::OWNER_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_owner_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_external_asset_owner (id, external_id) VALUES (?, ?)",
            dialect,
        )
    }

    pub fn select_transfer_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_external_asset_owner_transfer WHERE id = ?",
                Self::TRANSFER_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_transfers_by_loan_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_external_asset_owner_transfer WHERE loan_id = ? ORDER BY id ASC",
                Self::TRANSFER_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_transfer_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_external_asset_owner_transfer \
             (id, owner_id, external_id, status, sub_status, purchase_price_ratio, \
              settlement_date, effective_date_from, effective_date_to, loan_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn update_transfer_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_external_asset_owner_transfer \
             SET status = ?, sub_status = ?, effective_date_from = ?, effective_date_to = ? \
             WHERE id = ?",
            dialect,
        )
    }

    pub fn insert_transfer_details_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_external_asset_owner_transfer_details \
             (id, asset_owner_transfer_id, total_outstanding_derived, principal_outstanding_derived, \
              interest_outstanding_derived, fee_charges_outstanding_derived, \
              penalty_charges_outstanding_derived, total_overpaid_derived) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn insert_loan_mapping_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_external_asset_owner_transfer_loan_mapping \
             (id, loan_id, owner_transfer_id) VALUES (?, ?, ?)",
            dialect,
        )
    }

    pub fn delete_loan_mapping_by_loan_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_external_asset_owner_transfer_loan_mapping WHERE loan_id = ?",
            dialect,
        )
    }

    pub fn insert_owner_journal_mapping_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_external_asset_owner_journal_entry_mapping \
             (id, journal_entry_id, owner_id) VALUES (?, ?, ?)",
            dialect,
        )
    }

    pub fn insert_transfer_journal_mapping_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_external_asset_owner_transfer_journal_entry_mapping \
             (id, journal_entry_id, owner_transfer_id) VALUES (?, ?, ?)",
            dialect,
        )
    }

    // --- Owner operations ---

    pub fn create_owner(&self, external_id: impl Into<ExternalId>) -> FineractResult<ExternalAssetOwner> {
        let ext_id = external_id.into();
        let key = ext_id.value().unwrap_or_default().trim().to_string();
        if key.is_empty() {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "Owner external id cannot be blank",
            )
            .into());
        }
        if self.owners_by_ext_id.contains_key(&key) {
            return Err(ExternalAssetOwnerDuplicateException::new(key).into());
        }
        let id = self.next_owner_id.fetch_add(1, Ordering::SeqCst);
        let owner = ExternalAssetOwner::new(id, ExternalId::new(key.clone()));
        self.owners.insert(id, owner.clone());
        self.owners_by_ext_id.insert(key, id);
        Ok(owner)
    }

    pub fn find_or_create_owner(
        &self,
        external_id: impl Into<ExternalId>,
    ) -> FineractResult<ExternalAssetOwner> {
        let ext_id = external_id.into();
        let key = ext_id.value().unwrap_or_default().trim().to_string();
        if key.is_empty() {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "Owner external id cannot be blank",
            )
            .into());
        }
        if let Some(id_ref) = self.owners_by_ext_id.get(&key) {
            if let Some(owner) = self.owners.get(&id_ref) {
                return Ok(owner.clone());
            }
        }
        let id = self.next_owner_id.fetch_add(1, Ordering::SeqCst);
        let owner = ExternalAssetOwner::new(id, ExternalId::new(key.clone()));
        self.owners.insert(id, owner.clone());
        self.owners_by_ext_id.insert(key, id);
        Ok(owner)
    }

    pub fn find_owner_by_id(&self, owner_id: i64) -> Option<ExternalAssetOwner> {
        self.owners.get(&owner_id).map(|o| o.clone())
    }

    pub fn find_owner_by_external_id(&self, external_id: &ExternalId) -> Option<ExternalAssetOwner> {
        let key = external_id.value()?.trim();
        let id = *self.owners_by_ext_id.get(key)?;
        self.find_owner_by_id(id)
    }

    pub fn list_owners(&self) -> Vec<ExternalAssetOwner> {
        let mut list: Vec<ExternalAssetOwner> =
            self.owners.iter().map(|e| e.value().clone()).collect();
        list.sort_by_key(|o| o.id);
        list
    }

    // --- Transfer operations ---

    pub fn save_transfer(
        &self,
        mut transfer: ExternalAssetOwnerTransfer,
    ) -> FineractResult<ExternalAssetOwnerTransfer> {
        if transfer.id <= 0 {
            transfer.id = self.next_transfer_id.fetch_add(1, Ordering::SeqCst);
        }
        self.transfers.insert(transfer.id, transfer.clone());
        Ok(transfer)
    }

    pub fn find_transfer_by_id(&self, transfer_id: i64) -> Option<ExternalAssetOwnerTransfer> {
        self.transfers.get(&transfer_id).map(|t| t.clone())
    }

    pub fn find_transfer_by_external_id(
        &self,
        external_id: &ExternalId,
    ) -> Option<ExternalAssetOwnerTransfer> {
        let target = external_id.value()?;
        self.transfers
            .iter()
            .filter(|e| e.value().external_id.value() == Some(target))
            .map(|e| e.value().clone())
            .max_by_key(|t| t.id)
    }

    pub fn exists_transfer_with_external_id(&self, external_id: &ExternalId) -> bool {
        self.find_transfer_by_external_id(external_id).is_some()
    }

    pub fn list_transfers_by_loan_id(&self, loan_id: i64) -> Vec<ExternalAssetOwnerTransfer> {
        let mut list: Vec<ExternalAssetOwnerTransfer> = self
            .transfers
            .iter()
            .filter(|e| e.value().loan_id == loan_id)
            .map(|e| e.value().clone())
            .collect();
        list.sort_by_key(|t| t.id);
        list
    }

    /// Returns currently effective transfers (`effective_date_to >= 9999-12-31`) for `loan_id`,
    /// ordered by `id DESC` matching `findEffectiveTransfersOrderByIdDesc`.
    pub fn find_effective_transfers_order_by_id_desc(
        &self,
        loan_id: i64,
        _business_date: NaiveDate,
    ) -> Vec<ExternalAssetOwnerTransfer> {
        let mut list: Vec<ExternalAssetOwnerTransfer> = self
            .transfers
            .iter()
            .filter(|e| {
                let t = e.value();
                t.loan_id == loan_id
                    && t.effective_date_to >= FUTURE_DATE_9999_12_31
                    && !t.status.is_declined()
                    && !t.status.is_cancelled()
            })
            .map(|e| e.value().clone())
            .collect();
        list.sort_by(|a, b| b.id.cmp(&a.id));
        list
    }

    /// Returns the currently active (`Active` or `ActiveIntermediate`) transfer for `loan_id`, if any.
    pub fn find_active_transfer_by_loan_id(
        &self,
        loan_id: i64,
    ) -> Option<ExternalAssetOwnerTransfer> {
        if let Some(mapping) = self.find_loan_mapping_by_loan_id(loan_id) {
            if let Some(transfer) = self.find_transfer_by_id(mapping.owner_transfer_id) {
                if transfer.status.is_active() {
                    return Some(transfer);
                }
            }
        }
        self.transfers
            .iter()
            .filter(|e| {
                let t = e.value();
                t.loan_id == loan_id
                    && t.status.is_active()
                    && t.effective_date_to >= FUTURE_DATE_9999_12_31
            })
            .map(|e| e.value().clone())
            .max_by_key(|t| t.id)
    }

    // --- Transfer Details operations ---

    pub fn save_transfer_details(
        &self,
        mut details: ExternalAssetOwnerTransferDetails,
    ) -> ExternalAssetOwnerTransferDetails {
        if details.id <= 0 {
            details.id = self.next_details_id.fetch_add(1, Ordering::SeqCst);
        }
        details.details_id = details.id;
        details.recalculate_total_outstanding();
        self.transfer_details
            .insert(details.asset_owner_transfer_id, details.clone());
        details
    }

    pub fn find_transfer_details_by_transfer_id(
        &self,
        transfer_id: i64,
    ) -> Option<ExternalAssetOwnerTransferDetails> {
        self.transfer_details.get(&transfer_id).map(|d| d.clone())
    }

    // --- Loan Mapping operations ---

    pub fn save_loan_mapping(
        &self,
        loan_id: i64,
        owner_transfer_id: i64,
    ) -> ExternalAssetOwnerTransferLoanMapping {
        let id = self.next_loan_mapping_id.fetch_add(1, Ordering::SeqCst);
        let mapping = ExternalAssetOwnerTransferLoanMapping::new(id, loan_id, owner_transfer_id);
        self.loan_mappings.insert(loan_id, mapping.clone());
        mapping
    }

    pub fn find_loan_mapping_by_loan_id(
        &self,
        loan_id: i64,
    ) -> Option<ExternalAssetOwnerTransferLoanMapping> {
        self.loan_mappings.get(&loan_id).map(|m| m.clone())
    }

    pub fn delete_loan_mapping_by_loan_id(
        &self,
        loan_id: i64,
    ) -> Option<ExternalAssetOwnerTransferLoanMapping> {
        self.loan_mappings.remove(&loan_id).map(|(_, v)| v)
    }

    // --- Journal Entry & Sub-Ledger Mapping operations ---

    pub fn save_journal_entry(&self, mut entry: InvestorJournalEntry) -> InvestorJournalEntry {
        if entry.id <= 0 {
            entry.id = self.next_journal_entry_id.fetch_add(1, Ordering::SeqCst);
        }
        self.journal_entries.insert(entry.id, entry.clone());
        entry
    }

    pub fn save_owner_journal_mapping(
        &self,
        journal_entry_id: i64,
        owner_id: i64,
    ) -> ExternalAssetOwnerJournalEntryMapping {
        let id = self
            .next_owner_je_mapping_id
            .fetch_add(1, Ordering::SeqCst);
        let mapping = ExternalAssetOwnerJournalEntryMapping::new(id, journal_entry_id, owner_id);
        self.owner_journal_mappings.insert(id, mapping.clone());
        mapping
    }

    pub fn save_transfer_journal_mapping(
        &self,
        journal_entry_id: i64,
        owner_transfer_id: i64,
    ) -> ExternalAssetOwnerTransferJournalEntryMapping {
        let id = self
            .next_transfer_je_mapping_id
            .fetch_add(1, Ordering::SeqCst);
        let mapping =
            ExternalAssetOwnerTransferJournalEntryMapping::new(id, journal_entry_id, owner_transfer_id);
        self.transfer_journal_mappings.insert(id, mapping.clone());
        mapping
    }

    pub fn list_journal_entries_for_transfer(&self, transfer_id: i64) -> Vec<InvestorJournalEntry> {
        let mut entries: Vec<InvestorJournalEntry> = self
            .journal_entries
            .iter()
            .filter(|e| e.value().transfer_id == transfer_id)
            .map(|e| e.value().clone())
            .collect();
        entries.sort_by_key(|e| e.id);
        entries
    }

    pub fn list_journal_entries_for_owner(&self, owner_id: i64) -> Vec<InvestorJournalEntry> {
        let mut entries: Vec<InvestorJournalEntry> = self
            .journal_entries
            .iter()
            .filter(|e| e.value().owner_id == Some(owner_id))
            .map(|e| e.value().clone())
            .collect();
        entries.sort_by_key(|e| e.id);
        entries
    }

    pub fn list_owner_journal_mappings(
        &self,
        owner_id: i64,
    ) -> Vec<ExternalAssetOwnerJournalEntryMapping> {
        let mut list: Vec<ExternalAssetOwnerJournalEntryMapping> = self
            .owner_journal_mappings
            .iter()
            .filter(|e| e.value().owner_id == owner_id)
            .map(|e| e.value().clone())
            .collect();
        list.sort_by_key(|m| m.id);
        list
    }

    pub fn list_transfer_journal_mappings(
        &self,
        transfer_id: i64,
    ) -> Vec<ExternalAssetOwnerTransferJournalEntryMapping> {
        let mut list: Vec<ExternalAssetOwnerTransferJournalEntryMapping> = self
            .transfer_journal_mappings
            .iter()
            .filter(|e| e.value().owner_transfer_id == transfer_id)
            .map(|e| e.value().clone())
            .collect();
        list.sort_by_key(|m| m.id);
        list
    }

    pub fn record_business_event(&self, event: LoanOwnershipTransferBusinessEvent) {
        self.business_events
            .entry(event.loan_id)
            .or_default()
            .push(event);
    }

    pub fn list_business_events_for_loan(
        &self,
        loan_id: i64,
    ) -> Vec<LoanOwnershipTransferBusinessEvent> {
        self.business_events
            .get(&loan_id)
            .map(|v| v.clone())
            .unwrap_or_default()
    }
}

// =============================================================================
// 6. External Asset Owner Transfer Engine (`ExternalAssetOwnerTransferEngine`)
// =============================================================================

/// Core state machine, COB settlement processor, sub-ledger GL journal generator, and Avro event
/// emitter for External Asset Owner (Investor) loan transfers.
///
/// Ported from:
/// - `ExternalAssetOwnersWriteServiceImpl`
/// - `LoanAccountOwnerTransferBusinessStep`
/// - `LoanAccountOwnerTransferServiceImpl`
/// - `LoanTransferabilityServiceImpl`
/// - `AccountingServiceImpl` & `InvestorAccountingHelper`
/// - `LoanAccountDataV1Enricher` & `InvestorBusinessEventSerializer`
#[derive(Debug, Clone)]
pub struct ExternalAssetOwnerTransferEngine {
    repository: Arc<DualDbInvestorRepository>,
    gl_config: InvestorGlAccountConfig,
}

impl Default for ExternalAssetOwnerTransferEngine {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ExternalAssetOwnerTransferEngine {
    pub fn new(repository: Arc<DualDbInvestorRepository>) -> Self {
        Self {
            repository,
            gl_config: InvestorGlAccountConfig::default(),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbInvestorRepository::in_memory()))
    }

    pub fn with_gl_config(mut self, gl_config: InvestorGlAccountConfig) -> Self {
        self.gl_config = gl_config;
        self
    }

    pub fn repository(&self) -> &Arc<DualDbInvestorRepository> {
        &self.repository
    }

    pub const fn gl_config(&self) -> &InvestorGlAccountConfig {
        &self.gl_config
    }

    /// Initiates a `Sale` transfer on `loan_id` (creating a `Pending` transfer record).
    ///
    /// Ported from `ExternalAssetOwnersWriteServiceImpl.saleLoanByLoanId`.
    #[allow(clippy::too_many_arguments)]
    pub fn initiate_sale(
        &self,
        loan_id: i64,
        loan_account_number: Option<String>,
        owner_external_id: impl Into<ExternalId>,
        transfer_external_id: Option<ExternalId>,
        purchase_price_ratio: Decimal,
        settlement_date: NaiveDate,
        business_date: NaiveDate,
    ) -> FineractResult<ExternalAssetOwnerTransfer> {
        if settlement_date < business_date {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "Settlement date cannot be in the past",
            )
            .into());
        }
        if purchase_price_ratio <= Decimal::ZERO {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "Purchase price ratio must be greater than zero",
            )
            .into());
        }

        let resolved_transfer_ext_id = match transfer_external_id {
            Some(ext) if !ext.is_empty() => ext,
            _ => ExternalId::generate(),
        };
        if self
            .repository
            .exists_transfer_with_external_id(&resolved_transfer_ext_id)
        {
            return Err(ExternalAssetOwnerInitiateTransferException::new(format!(
                "Already existing an asset transfer with the provided transfer external id: {}",
                resolved_transfer_ext_id
            ))
            .into());
        }

        let effective_transfers = self
            .repository
            .find_effective_transfers_order_by_id_desc(loan_id, business_date);
        if effective_transfers.len() >= 2 {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "This loan cannot be sold, there is already an in progress transfer",
            )
            .into());
        } else if let Some(existing) = effective_transfers.first() {
            if existing.status.is_pending() {
                return Err(ExternalAssetOwnerInitiateTransferException::new(
                    "External asset owner transfer is already in PENDING state for this loan",
                )
                .into());
            }
            if !existing.status.is_active() {
                return Err(ExternalAssetOwnerInitiateTransferException::new(format!(
                    "This loan cannot be sold, because it is incorrect state! (transferId = {})",
                    existing.id
                ))
                .into());
            }
        }

        let owner = self.repository.find_or_create_owner(owner_external_id)?;
        let transfer = ExternalAssetOwnerTransfer::new(
            0,
            owner.id,
            resolved_transfer_ext_id,
            ExternalTransferStatus::Pending,
            None,
            purchase_price_ratio,
            settlement_date,
            business_date,
            FUTURE_DATE_9999_12_31,
            loan_id,
            loan_account_number,
        );
        self.repository.save_transfer(transfer)
    }

    /// Initiates a `Buyback` transfer on `loan_id`, validating that an active or pending `Sale`
    /// exists on the loan and `settlement_date >= sale.settlement_date`.
    ///
    /// Ported from `ExternalAssetOwnersWriteServiceImpl.buybackLoanByLoanId`.
    pub fn initiate_buyback(
        &self,
        loan_id: i64,
        transfer_external_id: Option<ExternalId>,
        settlement_date: NaiveDate,
        business_date: NaiveDate,
    ) -> FineractResult<ExternalAssetOwnerTransfer> {
        if settlement_date < business_date {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "Settlement date cannot be in the past",
            )
            .into());
        }

        let resolved_transfer_ext_id = match transfer_external_id {
            Some(ext) if !ext.is_empty() => ext,
            _ => ExternalId::generate(),
        };
        if self
            .repository
            .exists_transfer_with_external_id(&resolved_transfer_ext_id)
        {
            return Err(ExternalAssetOwnerInitiateTransferException::new(format!(
                "Already existing an asset transfer with the provided transfer external id: {}",
                resolved_transfer_ext_id
            ))
            .into());
        }

        let effective_transfers = self
            .repository
            .find_effective_transfers_order_by_id_desc(loan_id, business_date);
        if effective_transfers.is_empty() {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "This loan cannot be bought back, it is not owned by an external asset owner",
            )
            .into());
        }
        if effective_transfers.len() >= 2 {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "This loan cannot be bought back, external asset owner buyback transfer is already in progress",
            )
            .into());
        }

        let sale_transfer = &effective_transfers[0];
        if !sale_transfer.status.is_active() && !sale_transfer.status.is_pending() {
            return Err(ExternalAssetOwnerInitiateTransferException::new(format!(
                "This loan cannot be bought back, effective transfer is not in right state: {}",
                sale_transfer.status
            ))
            .into());
        }
        if settlement_date < sale_transfer.settlement_date {
            return Err(ExternalAssetOwnerInitiateTransferException::new(format!(
                "This loan cannot be bought back, settlement date is earlier than effective transfer settlement date: {}",
                sale_transfer.settlement_date
            ))
            .into());
        }

        let buyback_status = match sale_transfer.status {
            ExternalTransferStatus::ActiveIntermediate => {
                ExternalTransferStatus::BuybackIntermediate
            }
            _ => ExternalTransferStatus::Buyback,
        };

        let buyback_transfer = ExternalAssetOwnerTransfer::new(
            0,
            sale_transfer.owner_id,
            resolved_transfer_ext_id,
            buyback_status,
            None,
            sale_transfer.purchase_price_ratio,
            settlement_date,
            business_date,
            FUTURE_DATE_9999_12_31,
            loan_id,
            sale_transfer.loan_account_number.clone(),
        );
        self.repository.save_transfer(buyback_transfer)
    }

    /// Cancels a pending `Sale` or `Buyback` transfer at user request (`ExternalTransferSubStatus::UserRequested`).
    ///
    /// Ported from `ExternalAssetOwnersWriteServiceImpl.cancelTransactionById`.
    pub fn cancel_transfer(
        &self,
        transfer_id: i64,
        business_date: NaiveDate,
    ) -> FineractResult<ExternalAssetOwnerTransfer> {
        let mut selected = self
            .repository
            .find_transfer_by_id(transfer_id)
            .ok_or_else(|| {
                ExternalAssetOwnerInitiateTransferException::new(format!(
                    "This loan cannot be cancelled, transfer with id {transfer_id} does not exist"
                ))
            })?;

        let effective = self
            .repository
            .find_effective_transfers_order_by_id_desc(selected.loan_id, business_date);
        if effective.is_empty() {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "This loan cannot be cancelled, there is no effective transfer for this loan",
            )
            .into());
        }
        if effective[0].id != selected.id {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "This loan cannot be cancelled, selected transfer is not the latest",
            )
            .into());
        }
        if !selected.status.is_pending() && !selected.status.is_buyback() {
            return Err(ExternalAssetOwnerInitiateTransferException::new(
                "This loan cannot be cancelled, the selected transfer status is not pending or buyback",
            )
            .into());
        }

        selected.status = ExternalTransferStatus::Cancelled;
        selected.sub_status = Some(ExternalTransferSubStatus::UserRequested);
        selected.effective_date_to = business_date;
        self.repository.save_transfer(selected)
    }

    /// Initiates a `Sale` transfer from a parsed [`JsonCommand`].
    pub fn initiate_sale_from_command(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> FineractResult<ExternalAssetOwnerTransfer> {
        ExternalAssetOwnerValidator::validate_for_sale(command)?;
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let owner_external_id = command
            .string_value_of_parameter_named(ExternalTransferRequestParameters::OWNER_EXTERNAL_ID)
            .unwrap_or_default();
        let transfer_external_id = command
            .string_value_of_parameter_named(
                ExternalTransferRequestParameters::TRANSFER_EXTERNAL_ID,
            )
            .map(ExternalId::from);
        let ratio = command
            .big_decimal_value_of_parameter_named(
                ExternalTransferRequestParameters::PURCHASE_PRICE_RATIO,
            )?
            .or_else(|| {
                command
                    .string_value_of_parameter_named(
                        ExternalTransferRequestParameters::PURCHASE_PRICE_RATIO,
                    )
                    .and_then(|s| s.trim().parse::<Decimal>().ok())
            })
            .unwrap_or(Decimal::ONE);
        let settlement_date = command
            .local_date_value_of_parameter_named(
                ExternalTransferRequestParameters::SETTLEMENT_DATE,
            )?
            .ok_or_else(|| {
                ExternalAssetOwnerInitiateTransferException::new("settlementDate is required")
            })?;

        self.initiate_sale(
            loan_id,
            Some(format!("{loan_id:09}")),
            ExternalId::new(owner_external_id),
            transfer_external_id,
            ratio,
            settlement_date,
            ctx.business_date(),
        )
    }

    /// Initiates a `Buyback` transfer from a parsed [`JsonCommand`].
    pub fn initiate_buyback_from_command(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> FineractResult<ExternalAssetOwnerTransfer> {
        ExternalAssetOwnerValidator::validate_for_buyback(command)?;
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let transfer_external_id = command
            .string_value_of_parameter_named(
                ExternalTransferRequestParameters::TRANSFER_EXTERNAL_ID,
            )
            .map(ExternalId::from);
        let settlement_date = command
            .local_date_value_of_parameter_named(
                ExternalTransferRequestParameters::SETTLEMENT_DATE,
            )?
            .ok_or_else(|| {
                ExternalAssetOwnerInitiateTransferException::new("settlementDate is required")
            })?;

        self.initiate_buyback(
            loan_id,
            transfer_external_id,
            settlement_date,
            ctx.business_date(),
        )
    }

    /// Executes COB ownership transfer settlement for `loan` on `business_date`:
    /// - If a `Buyback` is scheduled on the same `settlement_date == business_date` as a `Pending` `Sale`,
    ///   cancels both with `ExternalTransferStatus::Cancelled` and `ExternalTransferSubStatus::SamDayTransfer`.
    /// - If a `Pending` `Sale` matures on `settlement_date == business_date`:
    ///   - If `loan.total_overpaid > 0` -> declines with `ExternalTransferSubStatus::BalanceNegative`.
    ///   - If `loan.total_outstanding() <= 0` -> declines with `ExternalTransferSubStatus::BalanceZero`.
    ///   - Otherwise transitions `Sale` to `ExternalTransferStatus::Active`, records `ExternalAssetOwnerTransferDetails`,
    ///     creates `ExternalAssetOwnerTransferLoanMapping`, posts balanced Investor Sub-Ledger GL journal entries,
    ///     enriches `LoanAccountDataV1`, and emits `LoanOwnershipTransferBusinessEvent`.
    /// - If a `Buyback` matures on `settlement_date == business_date`:
    ///   - Expires the active `Sale` and `Buyback`, records `ExternalAssetOwnerTransferDetails`, removes
    ///     `ExternalAssetOwnerTransferLoanMapping`, posts balanced Investor Sub-Ledger GL journal entries
    ///     relieving the investor's sub-ledger balance, and emits `LoanOwnershipTransferBusinessEvent`.
    ///
    /// Ported from `LoanAccountOwnerTransferBusinessStep.execute`.
    pub fn execute_cob_settlement(
        &self,
        loan: &LoanTransferSnapshot,
        business_date: NaiveDate,
    ) -> FineractResult<CobTransferSettlementOutcome> {
        let mut due_transfers: Vec<ExternalAssetOwnerTransfer> = self
            .repository
            .list_transfers_by_loan_id(loan.loan_id)
            .into_iter()
            .filter(|t| {
                t.settlement_date == business_date
                    && t.effective_date_to >= FUTURE_DATE_9999_12_31
                    && (t.status.is_pending() || t.status.is_buyback())
            })
            .collect();
        due_transfers.sort_by_key(|t| t.id);

        let mut settled_transfers = Vec::new();
        let mut transfer_details = Vec::new();
        let mut journal_entries = Vec::new();
        let mut owner_journal_mappings = Vec::new();
        let mut transfer_journal_mappings = Vec::new();
        let mut business_events = Vec::new();

        if due_transfers.len() == 2 {
            let first = &due_transfers[0];
            let second = &due_transfers[1];
            if !first.status.is_pending() || !second.status.is_buyback() {
                return Err(ExternalAssetOwnerInitiateTransferException::new(format!(
                    "Illegal transfer found. Expected PENDING and BUYBACK, found: {} and {}",
                    first.status, second.status
                ))
                .into());
            }

            for mut t in due_transfers {
                let orig_req_type = t.request_type();
                t.status = ExternalTransferStatus::Cancelled;
                t.sub_status = Some(ExternalTransferSubStatus::SamDayTransfer);
                t.effective_date_from = business_date;
                t.effective_date_to = business_date;
                let saved = self.repository.save_transfer(t)?;
                let loan_data = self.build_enriched_loan_account_data(loan);
                let ev = self.build_business_event(
                    loan,
                    &saved,
                    orig_req_type,
                    None,
                    None,
                    loan_data,
                );
                self.repository.record_business_event(ev.clone());
                business_events.push(ev);
                settled_transfers.push(saved);
            }
        } else if due_transfers.len() == 1 {
            let mut transfer = due_transfers.remove(0);
            if transfer.status.is_pending() {
                if loan.total_overpaid > Decimal::ZERO {
                    transfer.status = ExternalTransferStatus::Declined;
                    transfer.sub_status = Some(ExternalTransferSubStatus::BalanceNegative);
                    transfer.effective_date_from = business_date;
                    transfer.effective_date_to = business_date;
                    let saved = self.repository.save_transfer(transfer)?;
                    let loan_data = self.build_enriched_loan_account_data(loan);
                    let ev = self.build_business_event(
                        loan,
                        &saved,
                        ExternalTransferRequestType::Sale,
                        None,
                        None,
                        loan_data,
                    );
                    self.repository.record_business_event(ev.clone());
                    business_events.push(ev);
                    settled_transfers.push(saved);
                } else if loan.total_outstanding() <= Decimal::ZERO {
                    transfer.status = ExternalTransferStatus::Declined;
                    transfer.sub_status = Some(ExternalTransferSubStatus::BalanceZero);
                    transfer.effective_date_from = business_date;
                    transfer.effective_date_to = business_date;
                    let saved = self.repository.save_transfer(transfer)?;
                    let loan_data = self.build_enriched_loan_account_data(loan);
                    let ev = self.build_business_event(
                        loan,
                        &saved,
                        ExternalTransferRequestType::Sale,
                        None,
                        None,
                        loan_data,
                    );
                    self.repository.record_business_event(ev.clone());
                    business_events.push(ev);
                    settled_transfers.push(saved);
                } else {
                    // Check if there was a previous active owner on the loan (owner-to-owner sale)
                    let previous_owner_id = self
                        .repository
                        .find_active_transfer_by_loan_id(loan.loan_id)
                        .and_then(|mut prev| {
                            if prev.id != transfer.id {
                                let prev_owner = prev.owner_id;
                                prev.effective_date_to = business_date;
                                let _ = self.repository.save_transfer(prev);
                                self.repository.delete_loan_mapping_by_loan_id(loan.loan_id);
                                Some(prev_owner)
                            } else {
                                None
                            }
                        });

                    let active_status = match transfer.status {
                        ExternalTransferStatus::PendingIntermediate => {
                            ExternalTransferStatus::ActiveIntermediate
                        }
                        _ => ExternalTransferStatus::Active,
                    };
                    transfer.status = active_status;
                    transfer.sub_status = None;
                    transfer.effective_date_from =
                        business_date.succ_opt().unwrap_or(business_date);
                    transfer.effective_date_to = FUTURE_DATE_9999_12_31;
                    if transfer.loan_account_number.is_none() {
                        transfer.loan_account_number = loan.loan_account_number.clone();
                    }

                    let saved = self.repository.save_transfer(transfer)?;
                    self.repository.save_loan_mapping(loan.loan_id, saved.id);

                    let details = self.repository.save_transfer_details(
                        ExternalAssetOwnerTransferDetails::new(
                            0,
                            saved.id,
                            loan.total_principal_outstanding,
                            loan.total_interest_outstanding,
                            loan.total_fee_charges_outstanding,
                            loan.total_penalty_charges_outstanding,
                            loan.total_overpaid,
                        ),
                    );

                    let (jes, owner_maps, transfer_maps) = self
                        .post_journal_entries_for_sale(
                            loan,
                            &saved,
                            previous_owner_id,
                            business_date,
                        );

                    let loan_data = self.build_enriched_loan_account_data(loan);
                    let ev = self.build_business_event(
                        loan,
                        &saved,
                        ExternalTransferRequestType::Sale,
                        Some(&details),
                        previous_owner_id,
                        loan_data,
                    );
                    self.repository.record_business_event(ev.clone());

                    settled_transfers.push(saved);
                    transfer_details.push(details);
                    journal_entries.extend(jes);
                    owner_journal_mappings.extend(owner_maps);
                    transfer_journal_mappings.extend(transfer_maps);
                    business_events.push(ev);
                }
            } else if transfer.status.is_buyback() {
                if let Some(mut active_sale) = self
                    .repository
                    .find_active_transfer_by_loan_id(loan.loan_id)
                {
                    active_sale.effective_date_to = business_date;
                    self.repository.save_transfer(active_sale)?;

                    transfer.effective_date_to = business_date;
                    let saved_buyback = self.repository.save_transfer(transfer)?;
                    self.repository.delete_loan_mapping_by_loan_id(loan.loan_id);

                    let details = self.repository.save_transfer_details(
                        ExternalAssetOwnerTransferDetails::new(
                            0,
                            saved_buyback.id,
                            loan.total_principal_outstanding,
                            loan.total_interest_outstanding,
                            loan.total_fee_charges_outstanding,
                            loan.total_penalty_charges_outstanding,
                            loan.total_overpaid,
                        ),
                    );

                    let (jes, owner_maps, transfer_maps) =
                        self.post_journal_entries_for_buyback(loan, &saved_buyback, business_date);

                    let loan_data = self.build_enriched_loan_account_data(loan);
                    let ev = self.build_business_event(
                        loan,
                        &saved_buyback,
                        ExternalTransferRequestType::Buyback,
                        Some(&details),
                        Some(saved_buyback.owner_id),
                        loan_data,
                    );
                    self.repository.record_business_event(ev.clone());

                    settled_transfers.push(saved_buyback);
                    transfer_details.push(details);
                    journal_entries.extend(jes);
                    owner_journal_mappings.extend(owner_maps);
                    transfer_journal_mappings.extend(transfer_maps);
                    business_events.push(ev);
                } else {
                    // Buyback matured without an active sale -> cancel with UNSOLD
                    transfer.status = ExternalTransferStatus::Cancelled;
                    transfer.sub_status = Some(ExternalTransferSubStatus::Unsold);
                    transfer.effective_date_from = business_date;
                    transfer.effective_date_to = business_date;
                    let saved = self.repository.save_transfer(transfer)?;
                    let loan_data = self.build_enriched_loan_account_data(loan);
                    let ev = self.build_business_event(
                        loan,
                        &saved,
                        ExternalTransferRequestType::Buyback,
                        None,
                        None,
                        loan_data,
                    );
                    self.repository.record_business_event(ev.clone());
                    business_events.push(ev);
                    settled_transfers.push(saved);
                }
            }
        }

        let active_loan_mapping = self.repository.find_loan_mapping_by_loan_id(loan.loan_id);
        let loan_account_data = self.build_enriched_loan_account_data(loan);

        Ok(CobTransferSettlementOutcome {
            loan_id: loan.loan_id,
            business_date,
            settled_transfers,
            transfer_details,
            active_loan_mapping,
            journal_entries,
            owner_journal_mappings,
            transfer_journal_mappings,
            business_events,
            loan_account_data,
        })
    }

    /// Generates balanced double-entry GL journal entries and sub-ledger mappings for an executed `Sale`.
    ///
    /// Ported from `AccountingServiceImpl.createJournalEntriesForSaleAssetTransfer`.
    fn post_journal_entries_for_sale(
        &self,
        loan: &LoanTransferSnapshot,
        transfer: &ExternalAssetOwnerTransfer,
        previous_owner_id: Option<i64>,
        transaction_date: NaiveDate,
    ) -> (
        Vec<InvestorJournalEntry>,
        Vec<ExternalAssetOwnerJournalEntryMapping>,
        Vec<ExternalAssetOwnerTransferJournalEntryMapping>,
    ) {
        let mut entries = Vec::new();
        let mut owner_mappings = Vec::new();
        let mut transfer_mappings = Vec::new();

        let component_legs = self.resolve_component_gl_legs(loan);
        let total_amount: Decimal = component_legs.iter().map(|(_, amt)| *amt).sum();
        if total_amount <= Decimal::ZERO {
            return (entries, owner_mappings, transfer_mappings);
        }

        let tx_id = format!("{}{}", INVESTOR_TRANSFER_IDENTIFIER, transfer.id);
        let is_overpaid = loan.total_overpaid > Decimal::ZERO && loan.total_outstanding() <= Decimal::ZERO;

        // Pass 1 (!isReversalOrder = false): Move money to AssetTransfer clearing account
        for &(gl_account_id, amount) in &component_legs {
            let credit_entry = self.repository.save_journal_entry(InvestorJournalEntry {
                id: 0,
                gl_account_id,
                office_id: loan.office_id,
                currency_code: loan.currency_code.clone(),
                transaction_id: tx_id.clone(),
                entry_date: transaction_date,
                entry_type: InvestorJournalEntryType::Credit,
                amount,
                loan_id: loan.loan_id,
                transfer_id: transfer.id,
                owner_id: if is_overpaid {
                    Some(transfer.owner_id)
                } else {
                    previous_owner_id
                },
                manual_entry: false,
            });
            transfer_mappings.push(
                self.repository
                    .save_transfer_journal_mapping(credit_entry.id, transfer.id),
            );
            if let Some(oid) = credit_entry.owner_id {
                owner_mappings.push(
                    self.repository
                        .save_owner_journal_mapping(credit_entry.id, oid),
                );
            }
            entries.push(credit_entry);
        }

        let clearing_debit = self.repository.save_journal_entry(InvestorJournalEntry {
            id: 0,
            gl_account_id: self.gl_config.asset_transfer_gl_account_id,
            office_id: loan.office_id,
            currency_code: loan.currency_code.clone(),
            transaction_id: tx_id.clone(),
            entry_date: transaction_date,
            entry_type: InvestorJournalEntryType::Debit,
            amount: total_amount,
            loan_id: loan.loan_id,
            transfer_id: transfer.id,
            owner_id: None,
            manual_entry: false,
        });
        transfer_mappings.push(
            self.repository
                .save_transfer_journal_mapping(clearing_debit.id, transfer.id),
        );
        entries.push(clearing_debit);

        // Pass 2 (isReversalOrder = true): Move money from AssetTransfer clearing account to new owner
        for &(gl_account_id, amount) in &component_legs {
            let owner_for_debit = if is_overpaid {
                previous_owner_id
            } else {
                Some(transfer.owner_id)
            };
            let debit_entry = self.repository.save_journal_entry(InvestorJournalEntry {
                id: 0,
                gl_account_id,
                office_id: loan.office_id,
                currency_code: loan.currency_code.clone(),
                transaction_id: tx_id.clone(),
                entry_date: transaction_date,
                entry_type: InvestorJournalEntryType::Debit,
                amount,
                loan_id: loan.loan_id,
                transfer_id: transfer.id,
                owner_id: owner_for_debit,
                manual_entry: false,
            });
            transfer_mappings.push(
                self.repository
                    .save_transfer_journal_mapping(debit_entry.id, transfer.id),
            );
            if let Some(oid) = debit_entry.owner_id {
                owner_mappings.push(
                    self.repository
                        .save_owner_journal_mapping(debit_entry.id, oid),
                );
            }
            entries.push(debit_entry);
        }

        let clearing_credit = self.repository.save_journal_entry(InvestorJournalEntry {
            id: 0,
            gl_account_id: self.gl_config.asset_transfer_gl_account_id,
            office_id: loan.office_id,
            currency_code: loan.currency_code.clone(),
            transaction_id: tx_id,
            entry_date: transaction_date,
            entry_type: InvestorJournalEntryType::Credit,
            amount: total_amount,
            loan_id: loan.loan_id,
            transfer_id: transfer.id,
            owner_id: None,
            manual_entry: false,
        });
        transfer_mappings.push(
            self.repository
                .save_transfer_journal_mapping(clearing_credit.id, transfer.id),
        );
        entries.push(clearing_credit);

        (entries, owner_mappings, transfer_mappings)
    }

    /// Generates balanced double-entry GL journal entries and sub-ledger mappings for an executed `Buyback`.
    ///
    /// Ported from `AccountingServiceImpl.createJournalEntriesForBuybackAssetTransfer`.
    fn post_journal_entries_for_buyback(
        &self,
        loan: &LoanTransferSnapshot,
        transfer: &ExternalAssetOwnerTransfer,
        transaction_date: NaiveDate,
    ) -> (
        Vec<InvestorJournalEntry>,
        Vec<ExternalAssetOwnerJournalEntryMapping>,
        Vec<ExternalAssetOwnerTransferJournalEntryMapping>,
    ) {
        let mut entries = Vec::new();
        let mut owner_mappings = Vec::new();
        let mut transfer_mappings = Vec::new();

        let component_legs = self.resolve_component_gl_legs(loan);
        let total_amount: Decimal = component_legs.iter().map(|(_, amt)| *amt).sum();
        if total_amount <= Decimal::ZERO {
            return (entries, owner_mappings, transfer_mappings);
        }

        let tx_id = format!("{}{}", INVESTOR_TRANSFER_IDENTIFIER, transfer.id);
        let is_overpaid = loan.total_overpaid > Decimal::ZERO && loan.total_outstanding() <= Decimal::ZERO;

        // Pass 1 (!isReversalOrder = true): Debit component GL accounts, Credit AssetTransfer clearing account
        for &(gl_account_id, amount) in &component_legs {
            let owner_for_debit = if is_overpaid {
                Some(transfer.owner_id)
            } else {
                None
            };
            let debit_entry = self.repository.save_journal_entry(InvestorJournalEntry {
                id: 0,
                gl_account_id,
                office_id: loan.office_id,
                currency_code: loan.currency_code.clone(),
                transaction_id: tx_id.clone(),
                entry_date: transaction_date,
                entry_type: InvestorJournalEntryType::Debit,
                amount,
                loan_id: loan.loan_id,
                transfer_id: transfer.id,
                owner_id: owner_for_debit,
                manual_entry: false,
            });
            transfer_mappings.push(
                self.repository
                    .save_transfer_journal_mapping(debit_entry.id, transfer.id),
            );
            if let Some(oid) = debit_entry.owner_id {
                owner_mappings.push(
                    self.repository
                        .save_owner_journal_mapping(debit_entry.id, oid),
                );
            }
            entries.push(debit_entry);
        }

        let clearing_credit = self.repository.save_journal_entry(InvestorJournalEntry {
            id: 0,
            gl_account_id: self.gl_config.asset_transfer_gl_account_id,
            office_id: loan.office_id,
            currency_code: loan.currency_code.clone(),
            transaction_id: tx_id.clone(),
            entry_date: transaction_date,
            entry_type: InvestorJournalEntryType::Credit,
            amount: total_amount,
            loan_id: loan.loan_id,
            transfer_id: transfer.id,
            owner_id: None,
            manual_entry: false,
        });
        transfer_mappings.push(
            self.repository
                .save_transfer_journal_mapping(clearing_credit.id, transfer.id),
        );
        entries.push(clearing_credit);

        // Pass 2 (isReversalOrder = false): Credit component GL accounts (tagged to previous investor), Debit AssetTransfer clearing
        for &(gl_account_id, amount) in &component_legs {
            let owner_for_credit = if !is_overpaid {
                Some(transfer.owner_id)
            } else {
                None
            };
            let credit_entry = self.repository.save_journal_entry(InvestorJournalEntry {
                id: 0,
                gl_account_id,
                office_id: loan.office_id,
                currency_code: loan.currency_code.clone(),
                transaction_id: tx_id.clone(),
                entry_date: transaction_date,
                entry_type: InvestorJournalEntryType::Credit,
                amount,
                loan_id: loan.loan_id,
                transfer_id: transfer.id,
                owner_id: owner_for_credit,
                manual_entry: false,
            });
            transfer_mappings.push(
                self.repository
                    .save_transfer_journal_mapping(credit_entry.id, transfer.id),
            );
            if let Some(oid) = credit_entry.owner_id {
                owner_mappings.push(
                    self.repository
                        .save_owner_journal_mapping(credit_entry.id, oid),
                );
            }
            entries.push(credit_entry);
        }

        let clearing_debit = self.repository.save_journal_entry(InvestorJournalEntry {
            id: 0,
            gl_account_id: self.gl_config.asset_transfer_gl_account_id,
            office_id: loan.office_id,
            currency_code: loan.currency_code.clone(),
            transaction_id: tx_id,
            entry_date: transaction_date,
            entry_type: InvestorJournalEntryType::Debit,
            amount: total_amount,
            loan_id: loan.loan_id,
            transfer_id: transfer.id,
            owner_id: None,
            manual_entry: false,
        });
        transfer_mappings.push(
            self.repository
                .save_transfer_journal_mapping(clearing_debit.id, transfer.id),
        );
        entries.push(clearing_debit);

        (entries, owner_mappings, transfer_mappings)
    }

    fn resolve_component_gl_legs(&self, loan: &LoanTransferSnapshot) -> Vec<(i64, Decimal)> {
        let mut legs: Vec<(i64, Decimal)> = Vec::new();
        let mut add_leg = |gl_id: i64, amount: Decimal| {
            if amount > Decimal::ZERO {
                if let Some(existing) = legs.iter_mut().find(|(id, _)| *id == gl_id) {
                    existing.1 += amount;
                } else {
                    legs.push((gl_id, amount));
                }
            }
        };

        if loan.total_principal_outstanding > Decimal::ZERO {
            let gl_id = if loan.charged_off {
                if loan.fraud {
                    self.gl_config.charge_off_fraud_expense_gl_account_id
                } else {
                    self.gl_config.charge_off_expense_gl_account_id
                }
            } else {
                self.gl_config.loan_portfolio_gl_account_id
            };
            add_leg(gl_id, loan.total_principal_outstanding);
        }

        if loan.total_interest_outstanding > Decimal::ZERO {
            let gl_id = if loan.charged_off {
                self.gl_config
                    .income_from_charge_off_interest_gl_account_id
            } else {
                self.gl_config.interest_receivable_gl_account_id
            };
            add_leg(gl_id, loan.total_interest_outstanding);
        }

        if loan.total_fee_charges_outstanding > Decimal::ZERO {
            let gl_id = if loan.charged_off {
                self.gl_config.income_from_charge_off_fees_gl_account_id
            } else {
                self.gl_config.fees_receivable_gl_account_id
            };
            add_leg(gl_id, loan.total_fee_charges_outstanding);
        }

        if loan.total_penalty_charges_outstanding > Decimal::ZERO {
            let gl_id = if loan.charged_off {
                self.gl_config.income_from_charge_off_penalty_gl_account_id
            } else {
                self.gl_config.penalties_receivable_gl_account_id
            };
            add_leg(gl_id, loan.total_penalty_charges_outstanding);
        }

        if loan.total_overpaid > Decimal::ZERO {
            add_leg(self.gl_config.overpayment_gl_account_id, loan.total_overpaid);
        }

        legs
    }

    /// Enriches a [`LoanAccountDataV1`] Avro payload with the active external asset owner transfer
    /// metadata (`externalOwnerId`, `settlementDate`, `purchasePriceRatio`).
    ///
    /// Ported from `org.apache.fineract.investor.enricher.LoanAccountDataV1Enricher`.
    pub fn enrich_loan_account_data(&self, data: &mut LoanAccountDataV1) {
        let Some(loan_id) = data.id else {
            return;
        };
        if let Some(active_transfer) = self.repository.find_active_transfer_by_loan_id(loan_id) {
            let owner_ext_id = self
                .repository
                .find_owner_by_id(active_transfer.owner_id)
                .and_then(|o| o.external_id.value().map(str::to_string));
            data.external_owner_id = owner_ext_id.clone();
            data.settlement_date = Some(active_transfer.settlement_date.format("%Y-%m-%d").to_string());
            data.purchase_price_ratio = Some(active_transfer.purchase_price_ratio.to_string());
            if let Some(ref mut charges) = data.charges {
                for charge in charges.iter_mut() {
                    charge.external_owner_id = owner_ext_id.clone();
                }
            }
        } else {
            data.external_owner_id = None;
            data.settlement_date = None;
            data.purchase_price_ratio = None;
        }
    }

    /// Builds a [`LoanAccountDataV1`] snapshot for `loan` and enriches it via [`Self::enrich_loan_account_data`].
    pub fn build_enriched_loan_account_data(
        &self,
        loan: &LoanTransferSnapshot,
    ) -> LoanAccountDataV1 {
        let mut data = LoanAccountDataV1 {
            id: Some(loan.loan_id),
            account_no: loan.loan_account_number.clone(),
            external_id: loan
                .loan_external_id
                .as_ref()
                .and_then(|e| e.value().map(str::to_string)),
            external_owner_id: None,
            settlement_date: None,
            purchase_price_ratio: None,
            status: None,
            sub_status: None,
            client_id: None,
            client_account_no: None,
            client_name: None,
            client_office_id: Some(loan.office_id),
            client_external_id: None,
            loan_product_id: Some(loan.loan_product_id),
            loan_product_name: None,
            loan_product_description: None,
            is_loan_product_linked_to_floating_rate: None,
            fund_id: None,
            fund_name: None,
            loan_purpose_id: None,
            loan_purpose_name: None,
            loan_officer_id: None,
            loan_officer_name: None,
            loan_type: None,
            principal: Some(loan.total_principal_outstanding),
            approved_principal: None,
            proposed_principal: None,
            net_disbursal_amount: None,
            term_frequency: None,
            term_period_frequency_type: None,
            number_of_repayments: None,
            repayment_every: None,
            repayment_frequency_type: None,
            repayment_frequency_nth_day_type: None,
            repayment_frequency_day_of_week_type: None,
            interest_rate_per_period: None,
            interest_rate_frequency_type: None,
            annual_interest_rate: None,
            is_floating_interest_rate: None,
            interest_rate_differential: None,
            amortization_type: None,
            interest_type: None,
            interest_calculation_period_type: None,
            allow_partial_period_interest_calculation: None,
            in_arrears_tolerance: None,
            transaction_processing_strategy_code: None,
            transaction_processing_strategy_name: None,
            grace_on_principal_payment: None,
            recurring_moratorium_on_principal_periods: None,
            grace_on_interest_payment: None,
            grace_on_interest_charged: None,
            grace_on_arrears_ageing: None,
            interest_charged_from_date: None,
            expected_first_repayment_on_date: None,
            sync_disbursement_with_meeting: None,
            timeline: None,
            summary: None,
            repayment_schedule: None,
            transactions: None,
            charges: None,
            meeting: None,
            disbursement_details: None,
            original_schedule: None,
            total_overpaid: Some(loan.total_overpaid),
            loan_counter: None,
            loan_product_counter: None,
            linked_account: None,
            multi_disburse_loan: None,
            disallow_expected_disbursements: None,
            can_define_installment_amount: None,
            fixed_emi_amount: None,
            max_outstanding_loan_balance: None,
            can_disburse: None,
            emi_amount_variations: None,
            loan_term_variations: None,
            client_active_loan_options: None,
            can_use_for_topup: None,
            is_topup: None,
            closure_loan_id: None,
            closure_loan_account_no: None,
            topup_amount: None,
            product: None,
            in_arrears: None,
            is_npa: None,
            overdue_charges: None,
            days_in_month_type: None,
            days_in_year_type: None,
            days_in_year_custom_strategy: None,
            month_end_due_date_strategy: None,
            is_interest_recalculation_enabled: None,
            interest_recalculation_data: None,
            create_standing_instruction_at_disbursement: None,
            delinquent: None,
            delinquency_range: None,
            overpaid_on_date: None,
            last_closed_business_date: None,
            charged_off: Some(loan.charged_off),
            enable_installment_level_delinquency: None,
            enable_down_payment: None,
            disbursed_amount_percentage_for_down_payment: None,
            enable_auto_repayment_for_down_payment: None,
            repayment_start_date_type: None,
            interest_recognition_on_disbursement_date: None,
            custom_data: None,
            originators: None,
            actual_no_term: None,
        };
        self.enrich_loan_account_data(&mut data);
        data
    }

    /// Builds a [`LoanOwnershipTransferBusinessEvent`] matching `InvestorBusinessEventSerializer.toAvroDTO`.
    fn build_business_event(
        &self,
        loan: &LoanTransferSnapshot,
        transfer: &ExternalAssetOwnerTransfer,
        request_type: ExternalTransferRequestType,
        details: Option<&ExternalAssetOwnerTransferDetails>,
        previous_owner_id: Option<i64>,
        loan_account_data: LoanAccountDataV1,
    ) -> LoanOwnershipTransferBusinessEvent {
        let asset_owner_external_id = self
            .repository
            .find_owner_by_id(transfer.owner_id)
            .and_then(|o| o.external_id.value().map(str::to_string));
        let previous_owner_external_id = previous_owner_id
            .and_then(|id| self.repository.find_owner_by_id(id))
            .and_then(|o| o.external_id.value().map(str::to_string));
        let settlement_str = transfer.settlement_date.format("%Y-%m-%d").to_string();

        let payload = LoanOwnershipTransferDataV1 {
            loan_id: loan.loan_id,
            loan_external_id: loan
                .loan_external_id
                .as_ref()
                .and_then(|e| e.value().map(str::to_string)),
            r#type: Some(request_type.as_str().to_string()),
            transfer_external_id: transfer.external_id.value().map(str::to_string),
            transfer_external_group_id: None,
            submitted_date: Some(settlement_str.clone()),
            asset_owner_external_id,
            previous_owner_external_id,
            currency: Some(CurrencyDataV1 {
                code: Some(loan.currency_code.clone()),
                name: None,
                decimal_places: Some(loan.currency_digits as i32),
                in_multiples_of: loan.currency_in_multiples_of,
                display_symbol: None,
                name_code: None,
                display_label: None,
            }),
            total_outstanding_balance_amount: details.map(|d| d.total_outstanding),
            outstanding_principal_portion: details.map(|d| d.total_principal_outstanding),
            outstanding_fee_portion: details.map(|d| d.total_fee_charges_outstanding),
            outstanding_penalty_portion: details.map(|d| d.total_penalty_charges_outstanding),
            outstanding_interest_portion: details.map(|d| d.total_interest_outstanding),
            over_payment_portion: details.map(|d| d.total_overpaid),
            unrecognized_income_portion: None,
            unpaid_charge_data: if details.is_some() {
                Some(loan.unpaid_charges.clone())
            } else {
                None
            },
            transfer_status: Some(transfer.status.avro_transfer_status().to_string()),
            transfer_status_reason: transfer.sub_status.map(|s| s.as_str().to_string()),
            settlement_date: Some(settlement_str),
            purchase_price_ratio: Some(transfer.purchase_price_ratio.to_string()),
            custom_data: None,
        };

        LoanOwnershipTransferBusinessEvent {
            event_type: LoanOwnershipTransferBusinessEvent::TYPE.to_string(),
            category: LoanOwnershipTransferBusinessEvent::CATEGORY.to_string(),
            schema_name: LoanOwnershipTransferBusinessEvent::SCHEMA_NAME.to_string(),
            loan_id: loan.loan_id,
            transfer_id: transfer.id,
            transfer_status: transfer.status,
            transfer_sub_status: transfer.sub_status,
            payload,
            loan_account_data,
        }
    }
}

// =============================================================================
// 7. CQRS Command Handlers
// =============================================================================

/// Handles `SALE` `LOAN` commands (`SaleLoanToExternalAssetOwnerHandler.java`).
#[derive(Debug, Clone)]
pub struct SaleLoanToExternalAssetOwnerCommandHandler {
    repository: Arc<DualDbInvestorRepository>,
}

/// Alias matching upstream Java class name `SaleLoanToExternalAssetOwnerHandler`.
pub type SaleLoanToExternalAssetOwnerHandler = SaleLoanToExternalAssetOwnerCommandHandler;

impl SaleLoanToExternalAssetOwnerCommandHandler {
    pub const ACTION_NAME: &'static str = "SALE";
    pub const ENTITY_NAME: &'static str = "LOAN";

    pub fn new(repository: Arc<DualDbInvestorRepository>) -> Self {
        Self { repository }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbInvestorRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbInvestorRepository> {
        &self.repository
    }
}

#[async_trait]
impl CommandHandler for SaleLoanToExternalAssetOwnerCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let engine = ExternalAssetOwnerTransferEngine::new(Arc::clone(&self.repository));
        let saved = engine.initiate_sale_from_command(ctx, command)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_entity_external_id(saved.external_id)
            .with_sub_entity_id(saved.loan_id)
            .with_loan_id(saved.loan_id)
            .build())
    }
}

/// Handles `BUYBACK` `LOAN` commands (`BuybackLoanFromExternalAssetOwnerHandler.java`).
#[derive(Debug, Clone)]
pub struct BuybackLoanFromExternalAssetOwnerCommandHandler {
    repository: Arc<DualDbInvestorRepository>,
}

/// Alias matching upstream Java class name `BuybackLoanFromExternalAssetOwnerHandler`.
pub type BuybackLoanFromExternalAssetOwnerHandler = BuybackLoanFromExternalAssetOwnerCommandHandler;

impl BuybackLoanFromExternalAssetOwnerCommandHandler {
    pub const ACTION_NAME: &'static str = "BUYBACK";
    pub const ENTITY_NAME: &'static str = "LOAN";

    pub fn new(repository: Arc<DualDbInvestorRepository>) -> Self {
        Self { repository }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbInvestorRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbInvestorRepository> {
        &self.repository
    }
}

#[async_trait]
impl CommandHandler for BuybackLoanFromExternalAssetOwnerCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let engine = ExternalAssetOwnerTransferEngine::new(Arc::clone(&self.repository));
        let saved = engine.initiate_buyback_from_command(ctx, command)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_entity_external_id(saved.external_id)
            .with_sub_entity_id(saved.loan_id)
            .with_loan_id(saved.loan_id)
            .build())
    }
}

/// Handles `CANCEL` `ASSET_OWNER_TRANSACTION` / `LOAN` commands
/// (`CancelLoanFromExternalAssetOwnerHandler.java` & `CancelTransactionFromExternalAssetOwnerHandler.java`).
#[derive(Debug, Clone)]
pub struct CancelExternalAssetOwnerTransferCommandHandler {
    repository: Arc<DualDbInvestorRepository>,
}

/// Aliases matching upstream Java class names.
pub type CancelLoanFromExternalAssetOwnerHandler = CancelExternalAssetOwnerTransferCommandHandler;
pub type CancelTransactionFromExternalAssetOwnerHandler =
    CancelExternalAssetOwnerTransferCommandHandler;

impl CancelExternalAssetOwnerTransferCommandHandler {
    pub const ACTION_NAME: &'static str = "CANCEL";
    pub const ENTITY_NAME: &'static str = "ASSET_OWNER_TRANSACTION";

    pub fn new(repository: Arc<DualDbInvestorRepository>) -> Self {
        Self { repository }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbInvestorRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbInvestorRepository> {
        &self.repository
    }
}

#[async_trait]
impl CommandHandler for CancelExternalAssetOwnerTransferCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let transfer_id = command
            .entity_id
            .or(command.subresource_id)
            .unwrap_or_default();
        let engine = ExternalAssetOwnerTransferEngine::new(Arc::clone(&self.repository));
        let cancelled = engine.cancel_transfer(transfer_id, ctx.business_date())?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(cancelled.id)
            .with_entity_external_id(cancelled.external_id)
            .with_sub_entity_id(cancelled.loan_id)
            .with_loan_id(cancelled.loan_id)
            .build())
    }
}

// =============================================================================
// 8. Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    #[test]
    fn test_sale_to_active_to_buyback_settlement_and_sub_ledger_gl_entries() {
        let engine = ExternalAssetOwnerTransferEngine::in_memory();
        let day1 = ymd(2026, 3, 10);
        let day2 = ymd(2026, 3, 25);

        // 1. Initiate Sale on day1
        let sale = engine
            .initiate_sale(
                101,
                Some("000000101".to_string()),
                ExternalId::new("INV-BLACKROCK-01"),
                Some(ExternalId::new("TX-SALE-101")),
                dec!(1.025),
                day1,
                day1,
            )
            .expect("initiate sale should succeed");
        assert_eq!(sale.status, ExternalTransferStatus::Pending);
        assert_eq!(sale.sub_status, None);
        assert_eq!(sale.effective_date_to, FUTURE_DATE_9999_12_31);

        // 2. Execute COB settlement on day1 with Principal=1000, Interest=50, Fees=10, Penalties=5
        let loan_snapshot = LoanTransferSnapshot::new(
            101,
            dec!(1000.00),
            dec!(50.00),
            dec!(10.00),
            dec!(5.00),
            dec!(0.00),
        );
        assert_eq!(loan_snapshot.total_outstanding(), dec!(1065.00));

        let sale_outcome = engine
            .execute_cob_settlement(&loan_snapshot, day1)
            .expect("COB sale settlement should succeed");

        assert_eq!(sale_outcome.settled_transfers.len(), 1);
        let active_sale = &sale_outcome.settled_transfers[0];
        assert_eq!(active_sale.status, ExternalTransferStatus::Active);
        assert_eq!(active_sale.sub_status, None);
        assert_eq!(active_sale.effective_date_to, FUTURE_DATE_9999_12_31);

        // Verify active loan mapping was created
        let loan_mapping = sale_outcome
            .active_loan_mapping
            .as_ref()
            .expect("active loan mapping must exist after Sale settlement");
        assert_eq!(loan_mapping.loan_id, 101);
        assert_eq!(loan_mapping.owner_transfer_id, active_sale.id);

        // Verify transfer details
        assert_eq!(sale_outcome.transfer_details.len(), 1);
        let details = &sale_outcome.transfer_details[0];
        assert_eq!(details.total_outstanding, dec!(1065.00));
        assert_eq!(details.total_principal_outstanding, dec!(1000.00));
        assert_eq!(details.total_interest_outstanding, dec!(50.00));
        assert_eq!(details.total_fee_charges_outstanding, dec!(10.00));
        assert_eq!(details.total_penalty_charges_outstanding, dec!(5.00));

        // Verify balanced GL journal entries & Investor Sub-Ledger mappings
        assert!(sale_outcome.is_balanced());
        assert_eq!(sale_outcome.total_debits(), dec!(2130.00));
        assert_eq!(sale_outcome.total_credits(), dec!(2130.00));
        // 4 component debit legs attributed to the new investor owner
        assert_eq!(sale_outcome.owner_journal_mappings.len(), 4);
        let owner_entries = engine
            .repository()
            .list_journal_entries_for_owner(active_sale.owner_id);
        assert_eq!(owner_entries.len(), 4);
        assert!(owner_entries.iter().all(|e| e.is_debit()));
        let owner_debit_sum: Decimal = owner_entries.iter().map(|e| e.amount).sum();
        assert_eq!(owner_debit_sum, dec!(1065.00));

        // Verify LoanAccountDataV1 & LoanOwnershipTransferBusinessEvent Avro payload
        assert_eq!(
            sale_outcome.loan_account_data.external_owner_id.as_deref(),
            Some("INV-BLACKROCK-01")
        );
        assert_eq!(
            sale_outcome.loan_account_data.settlement_date.as_deref(),
            Some("2026-03-10")
        );
        assert_eq!(sale_outcome.business_events.len(), 1);
        let sale_event = &sale_outcome.business_events[0];
        assert_eq!(sale_event.payload.r#type.as_deref(), Some("SALE"));
        assert_eq!(
            sale_event.payload.transfer_status.as_deref(),
            Some("EXECUTED")
        );
        assert_eq!(
            sale_event.payload.total_outstanding_balance_amount,
            Some(dec!(1065.00))
        );

        // 3. Validate Buyback rejects settlement_date earlier than sale.settlement_date
        let early_buyback_err = engine
            .initiate_buyback(101, None, ymd(2026, 3, 9), ymd(2026, 3, 9))
            .unwrap_err();
        assert!(early_buyback_err
            .to_string()
            .contains("settlement date is earlier than effective transfer settlement date"));

        // 4. Initiate Buyback on day2 and settle via COB
        let buyback = engine
            .initiate_buyback(
                101,
                Some(ExternalId::new("TX-BUYBACK-101")),
                day2,
                day2,
            )
            .expect("initiate buyback should succeed");
        assert_eq!(buyback.status, ExternalTransferStatus::Buyback);

        let buyback_outcome = engine
            .execute_cob_settlement(&loan_snapshot, day2)
            .expect("COB buyback settlement should succeed");
        assert!(buyback_outcome.active_loan_mapping.is_none());
        assert!(buyback_outcome
            .loan_account_data
            .external_owner_id
            .is_none());
        assert!(buyback_outcome.is_balanced());
        assert_eq!(buyback_outcome.total_debits(), dec!(2130.00));
        assert_eq!(buyback_outcome.total_credits(), dec!(2130.00));
        // Investor sub-ledger now has 4 debits (from sale) + 4 credits (from buyback) -> net 0
        let all_owner_entries = engine
            .repository()
            .list_journal_entries_for_owner(active_sale.owner_id);
        assert_eq!(all_owner_entries.len(), 8);
        let owner_net: Decimal = all_owner_entries
            .iter()
            .map(|e| if e.is_debit() { e.amount } else { -e.amount })
            .sum();
        assert_eq!(owner_net, Decimal::ZERO);
    }

    #[test]
    fn test_same_day_sale_and_buyback_cancelled_with_sam_day_transfer() {
        let engine = ExternalAssetOwnerTransferEngine::in_memory();
        let cob_date = ymd(2026, 4, 15);

        let sale = engine
            .initiate_sale(
                202,
                Some("000000202".to_string()),
                ExternalId::new("INV-VANGUARD-02"),
                Some(ExternalId::new("TX-SAMEDAY-SALE")),
                dec!(1.00),
                cob_date,
                cob_date,
            )
            .unwrap();
        let buyback = engine
            .initiate_buyback(
                202,
                Some(ExternalId::new("TX-SAMEDAY-BUYBACK")),
                cob_date,
                cob_date,
            )
            .unwrap();

        let loan = LoanTransferSnapshot::new(
            202,
            dec!(500.00),
            dec!(25.00),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        );
        let outcome = engine.execute_cob_settlement(&loan, cob_date).unwrap();

        assert_eq!(outcome.settled_transfers.len(), 2);
        for settled in &outcome.settled_transfers {
            assert_eq!(settled.status, ExternalTransferStatus::Cancelled);
            assert_eq!(
                settled.sub_status,
                Some(ExternalTransferSubStatus::SamDayTransfer)
            );
            assert_eq!(settled.effective_date_to, cob_date);
        }
        assert!(outcome.active_loan_mapping.is_none());
        assert!(outcome.journal_entries.is_empty());
        assert_eq!(outcome.business_events.len(), 2);
        assert_eq!(
            engine
                .repository()
                .find_transfer_by_id(sale.id)
                .unwrap()
                .sub_status,
            Some(ExternalTransferSubStatus::SamDayTransfer)
        );
        assert_eq!(
            engine
                .repository()
                .find_transfer_by_id(buyback.id)
                .unwrap()
                .sub_status,
            Some(ExternalTransferSubStatus::SamDayTransfer)
        );
    }

    #[test]
    fn test_negative_and_zero_balance_decline_on_cob_settlement() {
        let engine = ExternalAssetOwnerTransferEngine::in_memory();
        let cob_date = ymd(2026, 5, 1);

        // Case 1: Overpaid loan (negative balance -> BalanceNegative)
        engine
            .initiate_sale(
                301,
                None,
                ExternalId::new("INV-FIDELITY-01"),
                Some(ExternalId::new("TX-NEG-301")),
                dec!(1.00),
                cob_date,
                cob_date,
            )
            .unwrap();
        let overpaid_loan = LoanTransferSnapshot::new(
            301,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(45.00),
        );
        let neg_outcome = engine
            .execute_cob_settlement(&overpaid_loan, cob_date)
            .unwrap();
        assert_eq!(neg_outcome.settled_transfers.len(), 1);
        assert_eq!(
            neg_outcome.settled_transfers[0].status,
            ExternalTransferStatus::Declined
        );
        assert_eq!(
            neg_outcome.settled_transfers[0].sub_status,
            Some(ExternalTransferSubStatus::BalanceNegative)
        );
        assert!(neg_outcome.active_loan_mapping.is_none());
        assert!(neg_outcome.journal_entries.is_empty());

        // Case 2: Zero balance loan (total_outstanding == 0 -> BalanceZero)
        engine
            .initiate_sale(
                302,
                None,
                ExternalId::new("INV-FIDELITY-01"),
                Some(ExternalId::new("TX-ZERO-302")),
                dec!(1.00),
                cob_date,
                cob_date,
            )
            .unwrap();
        let zero_loan = LoanTransferSnapshot::new(
            302,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        );
        let zero_outcome = engine.execute_cob_settlement(&zero_loan, cob_date).unwrap();
        assert_eq!(zero_outcome.settled_transfers.len(), 1);
        assert_eq!(
            zero_outcome.settled_transfers[0].status,
            ExternalTransferStatus::Declined
        );
        assert_eq!(
            zero_outcome.settled_transfers[0].sub_status,
            Some(ExternalTransferSubStatus::BalanceZero)
        );
        assert!(zero_outcome.active_loan_mapping.is_none());
        assert!(zero_outcome.journal_entries.is_empty());
    }

    #[tokio::test]
    async fn test_cqrs_command_handlers_sale_buyback_and_cancel() {
        let repo = Arc::new(DualDbInvestorRepository::in_memory());
        let sale_handler = SaleLoanToExternalAssetOwnerCommandHandler::new(Arc::clone(&repo));
        let buyback_handler =
            BuybackLoanFromExternalAssetOwnerCommandHandler::new(Arc::clone(&repo));
        let cancel_handler =
            CancelExternalAssetOwnerTransferCommandHandler::new(Arc::clone(&repo));

        let ctx = ExecutionContext::new("default", ymd(2026, 6, 1));

        let sale_cmd = JsonCommand::from_json(
            r#"{
                "settlementDate": "2026-06-10",
                "ownerExternalId": "INV-APOLLO-99",
                "transferExternalId": "TX-CMD-SALE-1",
                "purchasePriceRatio": "1.015",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_loan_id(555);

        let sale_res = sale_handler.handle(&ctx, &sale_cmd).await.unwrap();
        let sale_transfer_id = sale_res.resource_id.unwrap();
        assert_eq!(sale_res.sub_resource_id, Some(555));

        let buyback_cmd = JsonCommand::from_json(
            r#"{
                "settlementDate": "2026-06-20",
                "transferExternalId": "TX-CMD-BUYBACK-1",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_loan_id(555);

        // Settle the sale first so only 1 effective transfer is active when buyback is requested,
        // or cancel the pending sale via CancelExternalAssetOwnerTransferCommandHandler:
        let cancel_cmd = JsonCommand::from_json("{}")
            .unwrap()
            .with_entity_id(sale_transfer_id);
        let cancel_res = cancel_handler.handle(&ctx, &cancel_cmd).await.unwrap();
        assert_eq!(cancel_res.resource_id, Some(sale_transfer_id));
        assert_eq!(
            repo.find_transfer_by_id(sale_transfer_id).unwrap().status,
            ExternalTransferStatus::Cancelled
        );

        // Re-initiate sale and then initiate buyback on the pending sale
        let sale_cmd_2 = JsonCommand::from_json(
            r#"{
                "settlementDate": "2026-06-10",
                "ownerExternalId": "INV-APOLLO-99",
                "transferExternalId": "TX-CMD-SALE-2",
                "purchasePriceRatio": "1.00",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_loan_id(555);
        sale_handler.handle(&ctx, &sale_cmd_2).await.unwrap();

        let buyback_res = buyback_handler.handle(&ctx, &buyback_cmd).await.unwrap();
        assert!(buyback_res.resource_id.is_some());
        assert_eq!(buyback_res.sub_resource_id, Some(555));
    }
}
