//! `fineract-shares-rs` — Share Products, Share Accounts, Dividend Declarations, and Share Redemption Engine.
//!
//! Ported from:
//! - `org.apache.fineract.portfolio.shareproducts.domain.{ShareProduct, ShareProductMarketPrice, ShareProductDividendPayOutDetails, ShareProductDividendStatusType}`
//! - `org.apache.fineract.portfolio.shareproducts.service.{ShareProductWritePlatformServiceJpaRepositoryImpl, ShareProductDividendAssembler}`
//! - `org.apache.fineract.portfolio.shareproducts.serialization.ShareProductDataSerializer`
//! - `org.apache.fineract.portfolio.shareproducts.handler.{CreateShareProductCommandHandler, CreateShareProductDividendCommandHandler, ApproveShareProductDividendCommandHandler}`
//! - `org.apache.fineract.portfolio.shareaccounts.domain.{ShareAccount, ShareAccountTransaction, ShareAccountStatusType, PurchasedSharesStatusType, ShareAccountDividendDetails, ShareAccountDividendStatusType}`
//! - `org.apache.fineract.portfolio.shareaccounts.service.{ShareAccountWritePlatformServiceJpaRepositoryImpl, ShareAccountSchedularServiceImpl}`
//! - `org.apache.fineract.portfolio.shareaccounts.serialization.ShareAccountDataSerializer`
//! - `org.apache.fineract.portfolio.shareaccounts.handler.{CreateShareAccountCommandHandler, ApproveShareAccountCommandHandler, ActivateShareAccountCommandHandler, ApplyAddtionalSharesCommandHandler, ApproveAddtionalSharesCommandHandler, RejectAddtionalSharesCommandHandler, RedeemSharesCommandHandler, CloseShareAccountCommandHandler}`

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Days, NaiveDate};
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AnyPool, Row};

use fineract_charge_rs::EnumOptionData;
use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{
    FineractDateParser, FineractNumberParser, DialectSql, ExecutionContext, JsonCommand,
    SqlDialect,
};
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{
    DataValidatorBuilder, FineractError, FineractResult, PlatformDomainRuleException,
};

// =============================================================================
// 1. API CONSTANTS & SUPPORTED PARAMETERS
// =============================================================================

/// Supported JSON parameters for creating or updating a [`ShareProduct`] (`m_share_product`).
pub const SHARE_PRODUCT_SUPPORTED_PARAMETERS: &[&str] = &[
    "locale",
    "dateFormat",
    "id",
    "name",
    "shortName",
    "description",
    "externalId",
    "totalShares",
    "currencyCode",
    "digitsAfterDecimal",
    "inMultiplesOf",
    "sharesIssued",
    "unitPrice",
    "shareCapital",
    "minimumShares",
    "nominalShares",
    "maximumShares",
    "marketPricePeriods",
    "chargesSelected",
    "allowDividendCalculationForInactiveClients",
    "lockinPeriodFrequency",
    "lockinPeriodFrequencyType",
    "minimumActivePeriodForDividends",
    "minimumactiveperiodFrequencyType",
    "minimumActivePeriodFrequencyType",
    "accountingRule",
    "sharesReferenceId",
    "sharesSuspenseId",
    "shareEquityId",
    "incomeFromFeeAccountId",
];

/// Supported JSON parameters for declaring a share product dividend (`m_share_product_dividend_pay_out`).
pub const SHARE_DIVIDEND_SUPPORTED_PARAMETERS: &[&str] = &[
    "locale",
    "dateFormat",
    "dividendPeriodStartDate",
    "dividendPeriodEndDate",
    "dividendAmount",
];

/// Supported JSON parameters for creating or updating a [`ShareAccount`] (`m_share_account`).
pub const SHARE_ACCOUNT_SUPPORTED_PARAMETERS: &[&str] = &[
    "locale",
    "dateFormat",
    "id",
    "accountNo",
    "clientId",
    "productId",
    "submittedDate",
    "approvedDate",
    "activatedDate",
    "fieldOfficerId",
    "externalId",
    "currencyCode",
    "digitsAfterDecimal",
    "inMultiplesOf",
    "requestedShares",
    "savingsAccountId",
    "lockinPeriodFrequency",
    "lockinPeriodFrequencyType",
    "minimumActivePeriod",
    "minimumActivePeriodFrequencyType",
    "allowDividendCalculationForInactiveClients",
    "charges",
    "applicationDate",
    "purchasedDate",
    "numberOfShares",
    "unitPrice",
    "note",
];

/// Supported JSON parameters for share account approval (`APPROVE SHAREACCOUNT`).
pub const SHARE_ACCOUNT_APPROVAL_PARAMETERS: &[&str] =
    &["locale", "dateFormat", "approvedDate", "note"];

/// Supported JSON parameters for share account activation (`ACTIVATE SHAREACCOUNT`).
pub const SHARE_ACCOUNT_ACTIVATE_PARAMETERS: &[&str] =
    &["locale", "dateFormat", "activatedDate"];

/// Supported JSON parameters for additional shares and share redemption commands.
pub const SHARE_ACCOUNT_ADDITIONAL_SHARES_PARAMETERS: &[&str] = &[
    "locale",
    "dateFormat",
    "requestedDate",
    "requestedShares",
    "unitPrice",
];

/// Supported JSON parameters for closing a share account (`CLOSE SHAREACCOUNT`).
pub const SHARE_ACCOUNT_CLOSE_PARAMETERS: &[&str] =
    &["locale", "dateFormat", "closedDate", "note"];

// =============================================================================
// 2. SHARE DOMAIN ENUMERATIONS
// =============================================================================

/// Lifecycle status of a [`ShareAccount`] (`m_share_account.status_enum`).
///
/// Ported from `org.apache.fineract.portfolio.shareaccounts.domain.ShareAccountStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum ShareAccountStatusType {
    #[default]
    Invalid = 0,
    SubmittedAndPendingApproval = 100,
    Approved = 200,
    Active = 300,
    Rejected = 500,
    Closed = 600,
}

impl ShareAccountStatusType {
    pub const INVALID: Self = Self::Invalid;
    pub const SUBMITTED_AND_PENDING_APPROVAL: Self = Self::SubmittedAndPendingApproval;
    pub const APPROVED: Self = Self::Approved;
    pub const ACTIVE: Self = Self::Active;
    pub const REJECTED: Self = Self::Rejected;
    pub const CLOSED: Self = Self::Closed;

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
            Self::Invalid => "shareAccountStatusType.invalid",
            Self::SubmittedAndPendingApproval => {
                "shareAccountStatusType.submitted.and.pending.approval"
            }
            Self::Approved => "shareAccountStatusType.approved",
            Self::Active => "shareAccountStatusType.active",
            Self::Rejected => "shareAccountStatusType.rejected",
            Self::Closed => "shareAccountStatusType.closed",
        }
    }

    pub const fn from_i16(value: i16) -> Self {
        match value {
            100 => Self::SubmittedAndPendingApproval,
            200 => Self::Approved,
            300 => Self::Active,
            500 => Self::Rejected,
            600 => Self::Closed,
            _ => Self::Invalid,
        }
    }

    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::SubmittedAndPendingApproval,
            200 => Self::Approved,
            300 => Self::Active,
            500 => Self::Rejected,
            600 => Self::Closed,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    pub const fn has_state_of(self, state: Self) -> bool {
        (self as i16) == (state as i16)
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_submitted_and_pending_approval(self) -> bool {
        matches!(self, Self::SubmittedAndPendingApproval)
    }

    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed | Self::Rejected)
    }

    pub const fn is_active_or_awaiting_approval_or_disbursal(self) -> bool {
        self.is_approved() || self.is_submitted_and_pending_approval() || self.is_active()
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for ShareAccountStatusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Transaction type of a [`ShareAccountTransaction`] (`m_share_account_transactions.type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.shareaccounts.domain.PurchasedSharesStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum ShareAccountTransactionType {
    #[default]
    Invalid = 0,
    Issue = 100,
    Purchase = 300,
    Redeem = 400,
    ChargePayment = 500,
}

impl ShareAccountTransactionType {
    pub const INVALID: Self = Self::Invalid;
    pub const ISSUE: Self = Self::Issue;
    pub const PURCHASE: Self = Self::Purchase;
    pub const REDEEM: Self = Self::Redeem;
    pub const CHARGE_PAYMENT: Self = Self::ChargePayment;

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
            Self::Invalid => "purchasedSharesType.invalid",
            Self::Issue => "purchasedSharesType.issue",
            Self::Purchase => "purchasedSharesType.purchased",
            Self::Redeem => "purchasedSharesType.redeemed",
            Self::ChargePayment => "charge.payment",
        }
    }

    pub const fn from_i16(value: i16) -> Self {
        match value {
            100 => Self::Issue,
            300 => Self::Purchase,
            400 | 600 => Self::Redeem,
            500 | 700 => Self::ChargePayment,
            _ => Self::Invalid,
        }
    }

    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::Issue,
            300 => Self::Purchase,
            400 | 600 => Self::Redeem,
            500 | 700 => Self::ChargePayment,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_issue(self) -> bool {
        matches!(self, Self::Issue)
    }

    pub const fn is_purchase(self) -> bool {
        matches!(self, Self::Purchase)
    }

    pub const fn is_purchased(self) -> bool {
        matches!(self, Self::Purchase | Self::Issue)
    }

    pub const fn is_redeem(self) -> bool {
        matches!(self, Self::Redeem)
    }

    pub const fn is_redeemed(self) -> bool {
        matches!(self, Self::Redeem)
    }

    pub const fn is_charge_payment(self) -> bool {
        matches!(self, Self::ChargePayment)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for ShareAccountTransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Approval status of a [`ShareAccountTransaction`] (`m_share_account_transactions.status_enum`).
///
/// Ported from `org.apache.fineract.portfolio.shareaccounts.domain.PurchasedSharesStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum ShareAccountTransactionStatus {
    #[default]
    Invalid = 0,
    Pending = 100,
    Approved = 300,
    Rejected = 400,
}

/// Java-compatible alias for [`ShareAccountTransactionStatus`].
pub type PurchasedSharesStatusType = ShareAccountTransactionStatus;

impl ShareAccountTransactionStatus {
    pub const INVALID: Self = Self::Invalid;
    pub const PENDING: Self = Self::Pending;
    pub const APPLIED: Self = Self::Pending;
    pub const APPROVED: Self = Self::Approved;
    pub const REJECTED: Self = Self::Rejected;

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
            Self::Invalid => "purchasedSharesStatusType.invalid",
            Self::Pending => "purchasedSharesStatusType.applied",
            Self::Approved => "purchasedSharesStatusType.approved",
            Self::Rejected => "purchasedSharesStatusType.rejected",
        }
    }

    pub const fn from_i16(value: i16) -> Self {
        match value {
            100 => Self::Pending,
            300 => Self::Approved,
            400 => Self::Rejected,
            _ => Self::Invalid,
        }
    }

    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::Pending,
            300 => Self::Approved,
            400 => Self::Rejected,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn is_applied(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }

    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for ShareAccountTransactionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Lifecycle status of a share product dividend payout (`m_share_product_dividend_pay_out.status`
/// and `m_share_account_dividend_details.status`).
///
/// Ported from `org.apache.fineract.portfolio.shareproducts.domain.ShareProductDividendStatusType`
/// and `org.apache.fineract.portfolio.shareaccounts.domain.ShareAccountDividendStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum ShareProductDividendStatusType {
    #[default]
    Invalid = 0,
    Initiated = 100,
    Approved = 300,
    Posted = 400,
}

/// Alias matching `org.apache.fineract.portfolio.shareaccounts.domain.ShareAccountDividendStatusType`.
pub type ShareAccountDividendStatusType = ShareProductDividendStatusType;

impl ShareProductDividendStatusType {
    pub const INVALID: Self = Self::Invalid;
    pub const INITIATED: Self = Self::Initiated;
    pub const APPROVED: Self = Self::Approved;
    pub const POSTED: Self = Self::Posted;

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
            Self::Invalid => "shareAccountDividendStatusType.invalid",
            Self::Initiated => "shareAccountDividendStatusType.initiated",
            Self::Approved => "shareAccountDividendStatusType.approved",
            Self::Posted => "shareAccountDividendStatusType.posted",
        }
    }

    pub const fn from_i16(value: i16) -> Self {
        match value {
            100 => Self::Initiated,
            300 => Self::Approved,
            400 => Self::Posted,
            _ => Self::Invalid,
        }
    }

    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::Initiated,
            300 => Self::Approved,
            400 => Self::Posted,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_initiated(self) -> bool {
        matches!(self, Self::Initiated)
    }

    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }

    pub const fn is_posted(self) -> bool {
        matches!(self, Self::Posted)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for ShareProductDividendStatusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Period frequency type for share lock-in and minimum active periods.
///
/// Ported from `org.apache.fineract.portfolio.shareproducts.SharePeriodFrequencyType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum SharePeriodFrequencyType {
    Days = 0,
    Weeks = 1,
    Months = 2,
    Years = 3,
    #[default]
    Invalid = 4,
}

impl SharePeriodFrequencyType {
    pub const DAYS: Self = Self::Days;
    pub const WEEKS: Self = Self::Weeks;
    pub const MONTHS: Self = Self::Months;
    pub const YEARS: Self = Self::Years;
    pub const INVALID: Self = Self::Invalid;

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Days => "sharePeriodFrequencyType.days",
            Self::Weeks => "sharePeriodFrequencyType.weeks",
            Self::Months => "sharePeriodFrequencyType.months",
            Self::Years => "sharePeriodFrequencyType.years",
            Self::Invalid => "sharePeriodFrequencyType.invalid",
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(0) => Self::Days,
            Some(1) => Self::Weeks,
            Some(2) => Self::Months,
            Some(3) => Self::Years,
            _ => Self::Invalid,
        }
    }
}

// =============================================================================
// 3. SHARE PRODUCT & MARKET PRICE (`m_share_product`, `m_share_product_market_price`)
// =============================================================================

/// Dated market price entry for a [`ShareProduct`] (`m_share_product_market_price`).
///
/// Ported from `org.apache.fineract.portfolio.shareproducts.domain.ShareProductMarketPrice`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareMarketPrice {
    pub id: i64,
    pub product_id: i64,
    pub from_date: NaiveDate,
    pub share_value: Decimal,
}

/// Java-compatible alias for [`ShareMarketPrice`].
pub type ShareProductMarketPrice = ShareMarketPrice;

impl ShareMarketPrice {
    /// Creates a new [`ShareMarketPrice`] entry.
    pub const fn new(
        id: i64,
        product_id: i64,
        from_date: NaiveDate,
        share_value: Decimal,
    ) -> Self {
        Self {
            id,
            product_id,
            from_date,
            share_value,
        }
    }

    /// Returns `from_date` (`ShareProductMarketPrice.getStartDate()`).
    pub const fn start_date(&self) -> NaiveDate {
        self.from_date
    }

    /// Returns `share_value` (`ShareProductMarketPrice.getPrice()`).
    pub const fn price(&self) -> Decimal {
        self.share_value
    }
}

/// Share Product aggregate (`m_share_product`).
///
/// Ported from `org.apache.fineract.portfolio.shareproducts.domain.ShareProduct`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareProduct {
    pub id: i64,
    pub name: String,
    pub short_name: String,
    pub currency_code: String,
    pub total_shares: i64,
    pub total_shares_issued: i64,
    pub unit_price: Decimal,
    pub share_capital: Decimal,
    pub minimum_shares: Option<i64>,
    pub nominal_shares: Option<i64>,
    pub maximum_shares: Option<i64>,
    pub allow_dividend_calculation_for_inactive_clients: bool,
    pub lockin_period: Option<u32>,
    pub minimum_active_period: Option<u32>,
    pub market_prices: Vec<ShareMarketPrice>,
}

impl Default for ShareProduct {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            short_name: String::new(),
            currency_code: "USD".to_string(),
            total_shares: 0,
            total_shares_issued: 0,
            unit_price: Decimal::ONE,
            share_capital: Decimal::ZERO,
            minimum_shares: None,
            nominal_shares: None,
            maximum_shares: None,
            allow_dividend_calculation_for_inactive_clients: false,
            lockin_period: None,
            minimum_active_period: None,
            market_prices: Vec::new(),
        }
    }
}

impl ShareProduct {
    /// Creates a new [`ShareProduct`] aggregate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        name: impl Into<String>,
        short_name: impl Into<String>,
        currency_code: impl Into<String>,
        total_shares: i64,
        total_shares_issued: i64,
        unit_price: Decimal,
        share_capital: Decimal,
        minimum_shares: Option<i64>,
        nominal_shares: Option<i64>,
        maximum_shares: Option<i64>,
        allow_dividend_calculation_for_inactive_clients: bool,
        lockin_period: Option<u32>,
        minimum_active_period: Option<u32>,
        market_prices: Vec<ShareMarketPrice>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            short_name: short_name.into(),
            currency_code: currency_code.into(),
            total_shares,
            total_shares_issued,
            unit_price,
            share_capital,
            minimum_shares,
            nominal_shares,
            maximum_shares,
            allow_dividend_calculation_for_inactive_clients,
            lockin_period,
            minimum_active_period,
            market_prices,
        }
    }

    /// Derives the effective share unit price on `date` (`ShareProduct.deriveMarketPrice(LocalDate)`).
    ///
    /// Selects the market price entry with the latest `from_date <= date` (breaking ties by position/ID),
    /// or falls back to `self.unit_price` when no market price period has started on or before `date`.
    pub fn derive_unit_price_on(&self, date: NaiveDate) -> Decimal {
        let mut best: Option<(NaiveDate, usize, i64, Decimal)> = None;
        for (idx, mp) in self.market_prices.iter().enumerate() {
            if mp.from_date <= date {
                let candidate = (mp.from_date, idx, mp.id, mp.share_value);
                match best {
                    None => best = Some(candidate),
                    Some((best_date, best_idx, best_id, _)) => {
                        if mp.from_date > best_date
                            || (mp.from_date == best_date && (mp.id > best_id || idx > best_idx))
                        {
                            best = Some(candidate);
                        }
                    }
                }
            }
        }
        best.map(|(_, _, _, price)| price)
            .unwrap_or(self.unit_price)
    }

    /// Alias for [`ShareProduct::derive_unit_price_on`] matching Java `ShareProduct.deriveMarketPrice`.
    pub fn derive_market_price(&self, date: NaiveDate) -> Decimal {
        self.derive_unit_price_on(date)
    }

    /// Returns `true` if `requested_shares` satisfies `[minimum_shares, maximum_shares]` bounds.
    pub fn is_shares_allowed(&self, requested_shares: i64) -> bool {
        if requested_shares <= 0 {
            return false;
        }
        if let Some(min) = self.minimum_shares {
            if requested_shares < min {
                return false;
            }
        }
        if let Some(max) = self.maximum_shares {
            if requested_shares > max {
                return false;
            }
        }
        true
    }

    /// Validates that `requested_shares` falls within the product's `[minimum_shares, maximum_shares]` bounds.
    pub fn validate_requested_shares(&self, requested_shares: i64) -> FineractResult<()> {
        let validator = DataValidatorBuilder::for_resource("sharesaccount");
        validator
            .reset()
            .parameter("requestedShares")
            .value(requested_shares)
            .long_greater_than_zero();

        if let Some(min) = self.minimum_shares {
            if requested_shares < min {
                validator
                    .reset()
                    .parameter("requestedShares")
                    .value(requested_shares)
                    .fail_with_code(
                        "client.can.not.purchase.shares.lessthan.product.definition",
                        "Client can not purchase shares less than product definition",
                    );
            }
        }

        if let Some(max) = self.maximum_shares {
            if requested_shares > max {
                validator
                    .reset()
                    .parameter("requestedShares")
                    .value(requested_shares)
                    .fail_with_code(
                        "client.can.not.purchase.shares.morethan.product.definition",
                        "Client can not purchase shares more than product definition",
                    );
            }
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates a `CREATE SHAREPRODUCT` [`JsonCommand`] matching `ShareProductDataSerializer.validateAndCreate`.
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(SHARE_PRODUCT_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("sharesproduct");

        let name = command.string_value_of_parameter_named("name");
        validator
            .reset()
            .parameter("name")
            .value(name)
            .not_blank()
            .not_exceeding_length_of(200);

        let short_name = command.string_value_of_parameter_named("shortName");
        validator
            .reset()
            .parameter("shortName")
            .value(short_name)
            .not_blank()
            .not_exceeding_length_of(4);

        let total_shares = command.long_value_of_parameter_named("totalShares")?;
        validator
            .reset()
            .parameter("totalShares")
            .value(total_shares)
            .not_null()
            .long_greater_than_zero();

        let shares_issued = command.long_value_of_parameter_named("sharesIssued")?;
        if let (Some(issued), Some(total)) = (shares_issued, total_shares) {
            if issued > total {
                validator
                    .reset()
                    .parameter("sharesIssued")
                    .value(issued)
                    .fail_with_code_no_parameter_added_to_error_code(
                        "sharesIssued.cannot.be.greater.than.totalNumberOfShares",
                    );
            }
        }

        let currency_code = command.string_value_of_parameter_named("currencyCode");
        validator
            .reset()
            .parameter("currencyCode")
            .value(currency_code)
            .not_blank()
            .not_exceeding_length_of(3);

        let unit_price = command.big_decimal_value_of_parameter_named("unitPrice")?;
        validator
            .reset()
            .parameter("unitPrice")
            .value(unit_price)
            .not_null()
            .positive_amount();

        let min_shares = command.long_value_of_parameter_named("minimumShares")?;
        let nominal_shares = command.long_value_of_parameter_named("nominalShares")?;
        let max_shares = command.long_value_of_parameter_named("maximumShares")?;

        if let (Some(min), Some(nom)) = (min_shares, nominal_shares) {
            if nom < min {
                validator
                    .reset()
                    .parameter("nominalShares")
                    .value(nom)
                    .integer_greater_than_number(min);
            }
        }

        if let (Some(nom), Some(max)) = (nominal_shares, max_shares) {
            if max < nom {
                validator
                    .reset()
                    .parameter("maximumShares")
                    .value(max)
                    .integer_greater_than_number(nom);
            }
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a [`ShareProduct`] from a [`JsonCommand`], matching `ShareProductDataSerializer.validateAndCreate`.
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;

        let name = command
            .string_value_of_parameter_named("name")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let short_name = command
            .string_value_of_parameter_named("shortName")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let currency_code = command
            .string_value_of_parameter_named("currencyCode")
            .map(|s| s.trim().to_uppercase())
            .unwrap_or_else(|| "USD".to_string());
        let total_shares = command
            .long_value_of_parameter_named("totalShares")?
            .unwrap_or(0);
        let total_shares_issued = command
            .long_value_of_parameter_named("sharesIssued")?
            .unwrap_or(total_shares);
        let unit_price = command
            .big_decimal_value_of_parameter_named("unitPrice")?
            .unwrap_or(Decimal::ONE);
        let share_capital = command
            .big_decimal_value_of_parameter_named("shareCapital")?
            .unwrap_or_else(|| Decimal::from(total_shares_issued) * unit_price);
        let minimum_shares = command.long_value_of_parameter_named("minimumShares")?;
        let nominal_shares = command.long_value_of_parameter_named("nominalShares")?;
        let maximum_shares = command.long_value_of_parameter_named("maximumShares")?;
        let allow_dividend_calculation_for_inactive_clients = command
            .boolean_object_value_of_parameter_named("allowDividendCalculationForInactiveClients")
            .unwrap_or(false);
        let lockin_period = command
            .integer_value_of_parameter_named("lockinPeriodFrequency")?
            .and_then(|v| u32::try_from(v).ok());
        let minimum_active_period = command
            .integer_value_of_parameter_named("minimumActivePeriodForDividends")?
            .and_then(|v| u32::try_from(v).ok());

        let mut market_prices = Vec::new();
        if let Some(Value::Array(items)) = command.json_element("marketPricePeriods") {
            let locale = command.locale();
            let date_format = command.date_format();
            for (idx, item) in items.iter().enumerate() {
                let item_locale = item
                    .get("locale")
                    .and_then(Value::as_str)
                    .unwrap_or(&locale);
                let item_df = item
                    .get("dateFormat")
                    .and_then(Value::as_str)
                    .unwrap_or(&date_format);
                let from_date = match item.get("fromDate") {
                    Some(Value::String(raw)) => {
                        FineractDateParser::parse_date_for_param("fromDate", raw, item_df, item_locale)?
                    }
                    _ => continue,
                };
                let share_value = match item.get("shareValue") {
                    Some(raw_val) => FineractNumberParser::parse_decimal_for_param(
                        "shareValue",
                        raw_val,
                        item_locale,
                    )?
                    .unwrap_or(unit_price),
                    None => unit_price,
                };
                let mp_id = item
                    .get("id")
                    .and_then(Value::as_i64)
                    .unwrap_or((idx as i64) + 1);
                market_prices.push(ShareMarketPrice::new(mp_id, id, from_date, share_value));
            }
        }
        market_prices.sort_by(|a, b| a.from_date.cmp(&b.from_date).then_with(|| a.id.cmp(&b.id)));

        Ok(Self {
            id,
            name,
            short_name,
            currency_code,
            total_shares,
            total_shares_issued,
            unit_price,
            share_capital,
            minimum_shares,
            nominal_shares,
            maximum_shares,
            allow_dividend_calculation_for_inactive_clients,
            lockin_period,
            minimum_active_period,
            market_prices,
        })
    }
}

// =============================================================================
// 4. SHARE ACCOUNT TRANSACTION (`m_share_account_transactions`)
// =============================================================================

/// Share account transaction record (`m_share_account_transactions`).
///
/// Ported from `org.apache.fineract.portfolio.shareaccounts.domain.ShareAccountTransaction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccountTransaction {
    pub id: i64,
    pub account_id: i64,
    pub transaction_date: NaiveDate,
    pub total_shares: i64,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub charge_amount: Decimal,
    pub status_enum: ShareAccountTransactionStatus,
    pub type_enum: ShareAccountTransactionType,
}

impl ShareAccountTransaction {
    /// Creates a new [`ShareAccountTransaction`] with explicit fields.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: i64,
        account_id: i64,
        transaction_date: NaiveDate,
        total_shares: i64,
        unit_price: Decimal,
        amount: Decimal,
        charge_amount: Decimal,
        status_enum: ShareAccountTransactionStatus,
        type_enum: ShareAccountTransactionType,
    ) -> Self {
        Self {
            id,
            account_id,
            transaction_date,
            total_shares,
            unit_price,
            amount,
            charge_amount,
            status_enum,
            type_enum,
        }
    }

    /// Creates a pending share purchase transaction (`status_enum = Pending`, `type_enum = Purchase`).
    pub fn new_purchase(
        id: i64,
        account_id: i64,
        transaction_date: NaiveDate,
        total_shares: i64,
        unit_price: Decimal,
    ) -> Self {
        let amount = unit_price * Decimal::from(total_shares);
        Self {
            id,
            account_id,
            transaction_date,
            total_shares,
            unit_price,
            amount,
            charge_amount: Decimal::ZERO,
            status_enum: ShareAccountTransactionStatus::Pending,
            type_enum: ShareAccountTransactionType::Purchase,
        }
    }

    /// Creates an approved share redemption transaction (`status_enum = Approved`, `type_enum = Redeem`),
    /// matching `ShareAccountTransaction.createRedeemTransaction`.
    pub fn create_redeem_transaction(
        id: i64,
        account_id: i64,
        transaction_date: NaiveDate,
        total_shares: i64,
        unit_price: Decimal,
    ) -> Self {
        let amount = unit_price * Decimal::from(total_shares);
        Self {
            id,
            account_id,
            transaction_date,
            total_shares,
            unit_price,
            amount,
            charge_amount: Decimal::ZERO,
            status_enum: ShareAccountTransactionStatus::Approved,
            type_enum: ShareAccountTransactionType::Redeem,
        }
    }

    /// Creates an approved charge payment transaction (`status_enum = Approved`, `type_enum = ChargePayment`),
    /// matching `ShareAccountTransaction.createChargeTransaction`.
    pub const fn create_charge_transaction(
        id: i64,
        account_id: i64,
        transaction_date: NaiveDate,
        charge_amount: Decimal,
    ) -> Self {
        Self {
            id,
            account_id,
            transaction_date,
            total_shares: 0,
            unit_price: Decimal::ZERO,
            amount: charge_amount,
            charge_amount,
            status_enum: ShareAccountTransactionStatus::Approved,
            type_enum: ShareAccountTransactionType::ChargePayment,
        }
    }

    /// Marks this transaction as [`ShareAccountTransactionStatus::Approved`].
    pub fn approve(&mut self) {
        self.status_enum = ShareAccountTransactionStatus::Approved;
    }

    /// Reverts this transaction to [`ShareAccountTransactionStatus::Pending`].
    pub fn undo_approve(&mut self) {
        self.status_enum = ShareAccountTransactionStatus::Pending;
    }

    /// Marks this transaction as [`ShareAccountTransactionStatus::Rejected`] and subtracts any attached charge amount.
    pub fn reject(&mut self) {
        self.status_enum = ShareAccountTransactionStatus::Rejected;
        if self.charge_amount > Decimal::ZERO && self.amount >= self.charge_amount {
            self.amount -= self.charge_amount;
        }
    }

    /// Adds `total_charge_amount` to `self.amount` and records `self.charge_amount`.
    pub fn update_charge_amount(&mut self, total_charge_amount: Decimal) {
        self.amount += total_charge_amount;
        self.charge_amount = total_charge_amount;
    }

    /// Deducts `total_charge_amount` from `self.amount` and records `self.charge_amount` (used on share redemption).
    pub fn deduct_charges_from_total_amount(&mut self, total_charge_amount: Decimal) {
        self.amount -= total_charge_amount;
        self.charge_amount = total_charge_amount;
    }

    pub const fn is_pending_for_approval_transaction(&self) -> bool {
        self.status_enum.is_pending() && self.type_enum.is_purchased()
    }

    pub const fn is_purchase_transaction(&self) -> bool {
        self.status_enum.is_approved() && self.type_enum.is_purchased()
    }

    pub const fn is_redeem_transaction(&self) -> bool {
        self.status_enum.is_approved() && self.type_enum.is_redeem()
    }

    pub const fn is_charge_transaction(&self) -> bool {
        self.status_enum.is_approved() && self.type_enum.is_charge_payment()
    }

    pub const fn is_purchase_rejected_transaction(&self) -> bool {
        self.status_enum.is_rejected() && self.type_enum.is_purchased()
    }
}

// =============================================================================
// 5. TRAITS FOR FLEXIBLE SHARE ACCOUNT OPERATIONS
// =============================================================================

/// Trait abstracting a share unit price and product policy source (allowing callers to pass
/// a [`Decimal`] unit price, a [`ShareProduct`] reference, `(Decimal, Option<u32>)` with lock-in days, or `()`).
pub trait SharePriceSource {
    fn unit_price_on(&self, date: NaiveDate) -> Option<Decimal>;
    fn min_shares(&self) -> Option<i64> {
        None
    }
    fn max_shares(&self) -> Option<i64> {
        None
    }
    fn lockin_days(&self) -> u32 {
        0
    }
}

impl SharePriceSource for Decimal {
    fn unit_price_on(&self, _date: NaiveDate) -> Option<Decimal> {
        Some(*self)
    }
}

impl SharePriceSource for Option<Decimal> {
    fn unit_price_on(&self, _date: NaiveDate) -> Option<Decimal> {
        *self
    }
}

impl SharePriceSource for (Decimal, Option<u32>) {
    fn unit_price_on(&self, _date: NaiveDate) -> Option<Decimal> {
        Some(self.0)
    }
    fn lockin_days(&self) -> u32 {
        self.1.unwrap_or(0)
    }
}

impl SharePriceSource for (Decimal, u32) {
    fn unit_price_on(&self, _date: NaiveDate) -> Option<Decimal> {
        Some(self.0)
    }
    fn lockin_days(&self) -> u32 {
        self.1
    }
}

impl SharePriceSource for ShareProduct {
    fn unit_price_on(&self, date: NaiveDate) -> Option<Decimal> {
        Some(self.derive_unit_price_on(date))
    }
    fn min_shares(&self) -> Option<i64> {
        self.minimum_shares
    }
    fn max_shares(&self) -> Option<i64> {
        self.maximum_shares
    }
    fn lockin_days(&self) -> u32 {
        self.lockin_period.unwrap_or(0)
    }
}

impl SharePriceSource for &ShareProduct {
    fn unit_price_on(&self, date: NaiveDate) -> Option<Decimal> {
        Some(self.derive_unit_price_on(date))
    }
    fn min_shares(&self) -> Option<i64> {
        self.minimum_shares
    }
    fn max_shares(&self) -> Option<i64> {
        self.maximum_shares
    }
    fn lockin_days(&self) -> u32 {
        self.lockin_period.unwrap_or(0)
    }
}

impl SharePriceSource for () {
    fn unit_price_on(&self, _date: NaiveDate) -> Option<Decimal> {
        None
    }
}

/// Trait allowing `approve_additional_shares` and `reject_additional_shares` to accept a single
/// transaction ID (`i64`), a slice/vector of IDs (`&[i64]`, `Vec<i64>`), `Option<i64>`, or `()` (all pending).
pub trait TransactionSelector {
    fn select_ids(&self, transactions: &[ShareAccountTransaction]) -> Vec<i64>;
}

impl TransactionSelector for i64 {
    fn select_ids(&self, _transactions: &[ShareAccountTransaction]) -> Vec<i64> {
        vec![*self]
    }
}

impl TransactionSelector for &[i64] {
    fn select_ids(&self, _transactions: &[ShareAccountTransaction]) -> Vec<i64> {
        self.to_vec()
    }
}

impl TransactionSelector for Vec<i64> {
    fn select_ids(&self, _transactions: &[ShareAccountTransaction]) -> Vec<i64> {
        self.clone()
    }
}

impl<const N: usize> TransactionSelector for [i64; N] {
    fn select_ids(&self, _transactions: &[ShareAccountTransaction]) -> Vec<i64> {
        self.to_vec()
    }
}

impl<const N: usize> TransactionSelector for &[i64; N] {
    fn select_ids(&self, _transactions: &[ShareAccountTransaction]) -> Vec<i64> {
        self.to_vec()
    }
}

impl TransactionSelector for Option<i64> {
    fn select_ids(&self, transactions: &[ShareAccountTransaction]) -> Vec<i64> {
        match self {
            Some(id) => vec![*id],
            None => transactions
                .iter()
                .filter(|t| t.is_pending_for_approval_transaction())
                .map(|t| t.id)
                .collect(),
        }
    }
}

impl TransactionSelector for () {
    fn select_ids(&self, transactions: &[ShareAccountTransaction]) -> Vec<i64> {
        transactions
            .iter()
            .filter(|t| t.is_pending_for_approval_transaction())
            .map(|t| t.id)
            .collect()
    }
}

// =============================================================================
// 6. SHARE ACCOUNT AGGREGATE (`m_share_account`)
// =============================================================================

/// Share Account aggregate (`m_share_account`).
///
/// Ported from `org.apache.fineract.portfolio.shareaccounts.domain.ShareAccount`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccount {
    pub id: i64,
    pub account_no: String,
    pub client_id: i64,
    pub product_id: i64,
    pub savings_account_id: i64,
    pub status: ShareAccountStatusType,
    pub total_approved_shares: i64,
    pub total_pending_shares: i64,
    pub transactions: Vec<ShareAccountTransaction>,
}

impl Default for ShareAccount {
    fn default() -> Self {
        Self {
            id: 0,
            account_no: String::new(),
            client_id: 0,
            product_id: 0,
            savings_account_id: 0,
            status: ShareAccountStatusType::SubmittedAndPendingApproval,
            total_approved_shares: 0,
            total_pending_shares: 0,
            transactions: Vec::new(),
        }
    }
}

impl ShareAccount {
    /// Creates a new [`ShareAccount`] in [`ShareAccountStatusType::SubmittedAndPendingApproval`] status
    /// with an initial pending share purchase transaction.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        account_no: impl Into<String>,
        client_id: i64,
        product_id: i64,
        savings_account_id: i64,
        application_date: NaiveDate,
        requested_shares: i64,
        unit_price: Decimal,
    ) -> Self {
        let initial_txn = ShareAccountTransaction::new_purchase(
            1,
            id,
            application_date,
            requested_shares,
            unit_price,
        );
        Self {
            id,
            account_no: account_no.into(),
            client_id,
            product_id,
            savings_account_id,
            status: ShareAccountStatusType::SubmittedAndPendingApproval,
            total_approved_shares: 0,
            total_pending_shares: requested_shares,
            transactions: vec![initial_txn],
        }
    }

    /// Formats a 9-digit zero-padded account number from `id` (`format!("{id:09}")`).
    pub fn format_account_no(id: i64) -> String {
        format!("{id:09}")
    }

    fn next_transaction_id(&self) -> i64 {
        self.transactions
            .iter()
            .map(|t| t.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    fn latest_unit_price(&self) -> Decimal {
        self.transactions
            .iter()
            .rev()
            .find(|t| t.unit_price > Decimal::ZERO)
            .map(|t| t.unit_price)
            .unwrap_or(Decimal::ONE)
    }

    fn is_date_before_existing_transactions(&self, date: NaiveDate) -> bool {
        self.transactions.iter().any(|txn| {
            !txn.is_charge_transaction()
                && !txn.is_purchase_rejected_transaction()
                && date < txn.transaction_date
        })
    }

    /// Approves a share account that is in [`ShareAccountStatusType::SubmittedAndPendingApproval`],
    /// approving all pending purchase transactions and moving `total_pending_shares` to `total_approved_shares`.
    ///
    /// Ported from `ShareAccount.approve` and `ShareAccountDataSerializer.validateAndApprove`.
    pub fn approve(&mut self, approved_date: NaiveDate) -> FineractResult<()> {
        if self.status != ShareAccountStatusType::SubmittedAndPendingApproval {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.is.not.pending.for.approval",
                "Share account is not in submitted and pending approval status",
            )));
        }

        if let Some(earliest_date) = self
            .transactions
            .iter()
            .filter(|t| !t.is_charge_transaction())
            .map(|t| t.transaction_date)
            .min()
        {
            if approved_date < earliest_date {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.approved.date.cannot.be.before.submitted.date",
                    "Approved date cannot be before submitted date",
                )));
            }
        }

        let mut newly_approved_shares = 0_i64;
        for txn in &mut self.transactions {
            if txn.status_enum == ShareAccountTransactionStatus::Pending {
                txn.approve();
                newly_approved_shares += txn.total_shares;
            }
        }

        if self.total_pending_shares > 0 {
            self.total_approved_shares += self.total_pending_shares;
            self.total_pending_shares = 0;
        } else if newly_approved_shares > 0 {
            self.total_approved_shares += newly_approved_shares;
        }

        self.status = ShareAccountStatusType::Approved;
        Ok(())
    }

    /// Activates an approved [`ShareAccount`], transitioning `status` to [`ShareAccountStatusType::Active`].
    ///
    /// Ported from `ShareAccount.activate` and `ShareAccountDataSerializer.validateAndActivate`.
    pub fn activate(&mut self, activated_date: NaiveDate) -> FineractResult<()> {
        if self.status != ShareAccountStatusType::Approved {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.is.not.in.approved.status",
                "Share account is not in approved status",
            )));
        }

        if let Some(earliest_date) = self
            .transactions
            .iter()
            .filter(|t| !t.is_charge_transaction())
            .map(|t| t.transaction_date)
            .min()
        {
            if activated_date < earliest_date {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.cannot.be.before.approved.date",
                    "Activated date cannot be before approved date",
                )));
            }
        }

        for txn in &mut self.transactions {
            if txn.is_charge_transaction() {
                txn.transaction_date = activated_date;
            }
        }

        self.status = ShareAccountStatusType::Active;
        Ok(())
    }

    /// Submits a request for additional shares on an active [`ShareAccount`], creating a
    /// [`ShareAccountTransactionStatus::Pending`] [`ShareAccountTransactionType::Purchase`] transaction
    /// and incrementing `total_pending_shares`.
    ///
    /// Ported from `ShareAccount.addAdditionalPurchasedShares` and `ShareAccountDataSerializer.validateAndApplyAddtionalShares`.
    pub fn apply_additional_shares<P: SharePriceSource>(
        &mut self,
        requested_date: NaiveDate,
        requested_shares: i64,
        price_source: P,
    ) -> FineractResult<ShareAccountTransaction> {
        if self.status != ShareAccountStatusType::Active {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.is.not.in.active.state",
                "Share account is not in active state",
            )));
        }

        if requested_shares <= 0 {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.requestedShares.not.greater.than.zero",
                "Requested shares must be greater than zero",
            )));
        }

        if let Some(max_shares) = price_source.max_shares() {
            let total_after_approval =
                self.total_approved_shares + self.total_pending_shares + requested_shares;
            if total_after_approval > max_shares {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.exceeding.maximum.limit.defined.in.the.shareproduct",
                    "Existing and requested shares count is more than product definition",
                )));
            }
        }

        if self.is_date_before_existing_transactions(requested_date) {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.purchase.transaction.date.cannot.be.before.existing.transactions",
                "Purchase transaction date cannot be before existing transactions",
            )));
        }

        let unit_price = price_source
            .unit_price_on(requested_date)
            .unwrap_or_else(|| self.latest_unit_price());
        if unit_price <= Decimal::ZERO {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.unitPrice.not.greater.than.zero",
                "Share unit price must be greater than zero",
            )));
        }

        let txn_id = self.next_transaction_id();
        let txn = ShareAccountTransaction::new_purchase(
            txn_id,
            self.id,
            requested_date,
            requested_shares,
            unit_price,
        );
        self.total_pending_shares += requested_shares;
        self.transactions.push(txn.clone());
        Ok(txn)
    }

    /// Approves pending additional share purchase transaction(s) selected by `selector`
    /// (e.g. a single `transaction_id: i64`, `&[i64]`, or `()`), moving their shares from
    /// `total_pending_shares` to `total_approved_shares`.
    ///
    /// Ported from `ShareAccount.updateApprovedShares` and `ShareAccountDataSerializer.validateAndApproveAddtionalShares`.
    pub fn approve_additional_shares<S: TransactionSelector>(
        &mut self,
        selector: S,
    ) -> FineractResult<Vec<i64>> {
        if self.status != ShareAccountStatusType::Active {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.is.not.in.active.state",
                "Share account is not in active state",
            )));
        }

        let target_ids = selector.select_ids(&self.transactions);
        if target_ids.is_empty() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.pending.shares.not.found",
                "No pending share purchase transactions found to approve",
            )));
        }

        let mut approved_ids = Vec::with_capacity(target_ids.len());
        let mut total_shares_approved = 0_i64;

        for id in target_ids {
            let txn = self
                .transactions
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| {
                    FineractError::resource_not_found(
                        "error.msg.sharesaccount.transaction.id.invalid",
                        format!("Share account transaction with identifier {id} does not exist"),
                        id.to_string(),
                    )
                })?;

            if !txn.is_pending_for_approval_transaction() {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.transaction.not.pending.approval",
                    format!("Share account transaction {id} is not pending approval"),
                )));
            }

            txn.approve();
            total_shares_approved += txn.total_shares;
            approved_ids.push(id);
        }

        self.total_approved_shares += total_shares_approved;
        self.total_pending_shares = (self.total_pending_shares - total_shares_approved).max(0);

        Ok(approved_ids)
    }

    /// Rejects pending additional share purchase transaction(s) selected by `selector`
    /// (e.g. a single `transaction_id: i64`, `&[i64]`, or `()`), marking each transaction as
    /// [`ShareAccountTransactionStatus::Rejected`] and removing its shares from `total_pending_shares`.
    ///
    /// Ported from `ShareAccount.removePendingShares` and `ShareAccountDataSerializer.validateAndRejectAddtionalShares`.
    pub fn reject_additional_shares<S: TransactionSelector>(
        &mut self,
        selector: S,
    ) -> FineractResult<Vec<i64>> {
        if self.status != ShareAccountStatusType::Active {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.is.not.in.active.state",
                "Share account is not in active state",
            )));
        }

        let target_ids = selector.select_ids(&self.transactions);
        if target_ids.is_empty() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.pending.shares.not.found",
                "No pending share purchase transactions found to reject",
            )));
        }

        let mut rejected_ids = Vec::with_capacity(target_ids.len());
        let mut total_shares_rejected = 0_i64;

        for id in target_ids {
            let txn = self
                .transactions
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| {
                    FineractError::resource_not_found(
                        "error.msg.sharesaccount.transaction.id.invalid",
                        format!("Share account transaction with identifier {id} does not exist"),
                        id.to_string(),
                    )
                })?;

            if !txn.is_pending_for_approval_transaction() {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.transaction.not.pending.approval",
                    format!("Share account transaction {id} is not pending approval"),
                )));
            }

            txn.reject();
            total_shares_rejected += txn.total_shares;
            rejected_ids.push(id);
        }

        self.total_pending_shares = (self.total_pending_shares - total_shares_rejected).max(0);

        Ok(rejected_ids)
    }

    /// Redeems `requested_shares` on `requested_date`, enforcing:
    /// 1. Account status is [`ShareAccountStatusType::Active`],
    /// 2. `requested_shares > 0` and `requested_shares <= self.total_approved_shares`,
    /// 3. `requested_date` is not before any existing active transaction,
    /// 4. Lock-in period (`lockin_days` from `price_or_product`) has elapsed for at least `requested_shares` of purchased shares.
    ///
    /// Ported from `ShareAccountDataSerializer.validateAndRedeemShares` and `ShareAccountDataSerializer.validateRedeemRequest`.
    pub fn redeem_shares<P: SharePriceSource>(
        &mut self,
        requested_date: NaiveDate,
        requested_shares: i64,
        price_or_product: P,
    ) -> FineractResult<ShareAccountTransaction> {
        let unit_price = price_or_product
            .unit_price_on(requested_date)
            .unwrap_or_else(|| self.latest_unit_price());
        let lockin_days = price_or_product.lockin_days();
        self.redeem_shares_with_lockin(
            requested_date,
            requested_shares,
            unit_price,
            if lockin_days > 0 {
                Some(lockin_days)
            } else {
                None
            },
        )
    }

    /// Redeems `requested_shares` on `requested_date` at `unit_price` with an explicit `lockin_period_days`.
    pub fn redeem_shares_with_lockin(
        &mut self,
        requested_date: NaiveDate,
        requested_shares: i64,
        unit_price: Decimal,
        lockin_period_days: Option<u32>,
    ) -> FineractResult<ShareAccountTransaction> {
        if self.status != ShareAccountStatusType::Active {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.is.not.in.active.state",
                "Share account is not in active state",
            )));
        }

        if requested_shares <= 0 {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.requestedShares.not.greater.than.zero",
                "Requested shares to redeem must be greater than zero",
            )));
        }

        if requested_shares > self.total_approved_shares {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.cannot.be.redeemed.due.to.insufficient.shares",
                "Cannot redeem shares exceeding total approved shares",
            )));
        }

        if self.is_date_before_existing_transactions(requested_date) {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.redeem.transaction.date.cannot.be.before.existing.transactions",
                "Redeem transaction date cannot be before existing transactions",
            )));
        }

        if let Some(lockin_days) = lockin_period_days.filter(|&d| d > 0) {
            let mut total_shares_can_be_redeemed = 0_i64;
            let mut total_shares_purchased_before_redeem = 0_i64;
            let mut is_purchase_transaction_exist = false;

            for txn in &self.transactions {
                if !txn.is_charge_transaction() && !txn.is_purchase_rejected_transaction() {
                    let purchase_date = txn.transaction_date;
                    let lockin_date = purchase_date
                        .checked_add_days(Days::new(u64::from(lockin_days)))
                        .unwrap_or(purchase_date);

                    if lockin_date <= requested_date {
                        if txn.is_purchase_transaction() {
                            total_shares_can_be_redeemed += txn.total_shares;
                        } else if txn.is_redeem_transaction() {
                            total_shares_can_be_redeemed -= txn.total_shares;
                        }
                    }

                    if purchase_date <= requested_date {
                        if txn.is_purchase_transaction() {
                            is_purchase_transaction_exist = true;
                            total_shares_purchased_before_redeem += txn.total_shares;
                        } else if txn.is_redeem_transaction() {
                            total_shares_purchased_before_redeem -= txn.total_shares;
                        }
                    }
                }
            }

            if !is_purchase_transaction_exist {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.no.purchase.transaction.found.before.redeem.date",
                    "No purchase transaction found before redeem date",
                )));
            } else if total_shares_purchased_before_redeem < requested_shares {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.cannot.be.redeemed.due.to.insufficient.shares.for.this.redeem.date",
                    "Cannot redeem shares due to insufficient shares for this redeem date",
                )));
            } else if total_shares_can_be_redeemed < requested_shares {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.cannot.be.redeemed.due.to.lockinperiod",
                    "Cannot redeem shares before lock-in period has elapsed",
                )));
            }
        }

        let txn_id = self.next_transaction_id();
        let redeem_txn = ShareAccountTransaction::create_redeem_transaction(
            txn_id,
            self.id,
            requested_date,
            requested_shares,
            unit_price,
        );
        self.total_approved_shares -= requested_shares;
        self.transactions.push(redeem_txn.clone());

        Ok(redeem_txn)
    }

    /// Closes an active [`ShareAccount`] on `closed_date`, redeeming any remaining `total_approved_shares`
    /// and setting `status` to [`ShareAccountStatusType::Closed`].
    ///
    /// Ported from `ShareAccount.close` and `ShareAccountDataSerializer.validateAndClose`.
    pub fn close<P: SharePriceSource>(
        &mut self,
        closed_date: NaiveDate,
        price_source: P,
    ) -> FineractResult<Option<ShareAccountTransaction>> {
        if self.status != ShareAccountStatusType::Active {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.is.not.in.active.state",
                "Share account must be active to close",
            )));
        }

        if self.is_date_before_existing_transactions(closed_date) {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.sharesaccount.share.account.cannot.be.closed.before.existing.transactions",
                "Share account cannot be closed before existing transactions",
            )));
        }

        let redeem_txn = if self.total_approved_shares > 0 {
            let unit_price = price_source
                .unit_price_on(closed_date)
                .unwrap_or_else(|| self.latest_unit_price());
            let txn_id = self.next_transaction_id();
            let txn = ShareAccountTransaction::create_redeem_transaction(
                txn_id,
                self.id,
                closed_date,
                self.total_approved_shares,
                unit_price,
            );
            self.transactions.push(txn.clone());
            Some(txn)
        } else {
            None
        };

        self.total_approved_shares = 0;
        self.total_pending_shares = 0;
        self.status = ShareAccountStatusType::Closed;

        Ok(redeem_txn)
    }
}

// =============================================================================
// 7. DIVIDEND PAYOUT DETAILS & PRO-RATA DIVIDEND ENGINE
//    (`m_share_product_dividend_pay_out`, `m_share_account_dividend_details`)
// =============================================================================

/// Per-account dividend allocation record (`m_share_account_dividend_details`).
///
/// Ported from `org.apache.fineract.portfolio.shareaccounts.domain.ShareAccountDividendDetails`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccountDividendDetails {
    pub id: i64,
    pub dividend_pay_out_id: i64,
    pub account_id: i64,
    pub amount: Decimal,
    pub status: ShareProductDividendStatusType,
    pub savings_transaction_id: Option<i64>,
}

impl ShareAccountDividendDetails {
    /// Creates a new [`ShareAccountDividendDetails`] in [`ShareProductDividendStatusType::Initiated`] status.
    pub const fn new(
        id: i64,
        dividend_pay_out_id: i64,
        account_id: i64,
        amount: Decimal,
    ) -> Self {
        Self {
            id,
            dividend_pay_out_id,
            account_id,
            amount,
            status: ShareProductDividendStatusType::Initiated,
            savings_transaction_id: None,
        }
    }

    /// Marks this account dividend allocation as posted to a savings account transaction.
    pub fn mark_posted(&mut self, savings_transaction_id: i64) {
        self.status = ShareProductDividendStatusType::Posted;
        self.savings_transaction_id = Some(savings_transaction_id);
    }
}

/// Share Product Dividend Payout aggregate (`m_share_product_dividend_pay_out`).
///
/// Ported from `org.apache.fineract.portfolio.shareproducts.domain.ShareProductDividendPayOutDetails`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareProductDividendPayOutDetails {
    pub id: i64,
    pub product_id: i64,
    pub amount: Decimal,
    pub dividend_period_start_date: NaiveDate,
    pub dividend_period_end_date: NaiveDate,
    pub status: ShareProductDividendStatusType,
    pub account_dividend_details: Vec<ShareAccountDividendDetails>,
}

impl ShareProductDividendPayOutDetails {
    /// Creates a new [`ShareProductDividendPayOutDetails`] in [`ShareProductDividendStatusType::Initiated`] status.
    pub const fn new(
        id: i64,
        product_id: i64,
        amount: Decimal,
        dividend_period_start_date: NaiveDate,
        dividend_period_end_date: NaiveDate,
    ) -> Self {
        Self {
            id,
            product_id,
            amount,
            dividend_period_start_date,
            dividend_period_end_date,
            status: ShareProductDividendStatusType::Initiated,
            account_dividend_details: Vec::new(),
        }
    }

    /// Approves the dividend payout (`ShareProductDividendPayOutDetails.approveDividendPayout`).
    pub fn approve_dividend_payout(&mut self) -> FineractResult<()> {
        if self.status.is_approved() || self.status.is_posted() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.shareproduct.dividend.already.approved",
                "Cannot approve already approved dividends",
            )));
        }
        self.status = ShareProductDividendStatusType::Approved;
        for detail in &mut self.account_dividend_details {
            detail.status = ShareProductDividendStatusType::Approved;
        }
        Ok(())
    }

    /// Marks the dividend payout and all child account details as [`ShareProductDividendStatusType::Posted`].
    pub fn mark_posted(&mut self) {
        self.status = ShareProductDividendStatusType::Posted;
        for detail in &mut self.account_dividend_details {
            detail.status = ShareProductDividendStatusType::Posted;
        }
    }
}

/// Share Dividend Engine calculating pro-rata dividend distributions across active share accounts
/// weighted by share-holding days (`shares * days_held`).
///
/// Ported from `org.apache.fineract.portfolio.shareproducts.service.ShareProductDividendAssembler`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShareDividendEngine;

impl ShareDividendEngine {
    /// Calculates the number of share-days (`shares * days_held`) held by a single [`ShareAccount`]
    /// during `[dividend_period_start_date, dividend_period_end_date]`, enforcing `minimum_active_period_days`.
    ///
    /// Ported from `ShareProductDividendAssembler.calculateNumberOfShareDays`.
    pub fn calculate_account_share_days(
        account: &ShareAccount,
        dividend_period_start_date: NaiveDate,
        dividend_period_end_date: NaiveDate,
        minimum_active_period_days: u32,
    ) -> i64 {
        if dividend_period_end_date <= dividend_period_start_date {
            return 0;
        }

        let mut eligible_txns: Vec<&ShareAccountTransaction> = account
            .transactions
            .iter()
            .filter(|t| {
                t.status_enum.is_approved()
                    && !t.type_enum.is_charge_payment()
                    && t.transaction_date < dividend_period_end_date
            })
            .collect();

        // Fallback if an account was constructed directly with `total_approved_shares > 0` and no transactions
        if eligible_txns.is_empty() && account.total_approved_shares > 0 {
            let period_days = (dividend_period_end_date - dividend_period_start_date).num_days();
            if period_days >= i64::from(minimum_active_period_days) {
                return account.total_approved_shares * period_days;
            }
            return 0;
        }

        eligible_txns
            .sort_by(|a, b| a.transaction_date.cmp(&b.transaction_date).then_with(|| a.id.cmp(&b.id)));

        let mut number_of_share_days_per_account = 0_i64;
        let mut number_of_shares = 0_i64;
        let mut last_dividend_applied_date: Option<NaiveDate> = None;

        for txn in eligible_txns {
            let mut share_start_date = txn.transaction_date;
            if share_start_date < dividend_period_start_date {
                share_start_date = dividend_period_start_date;
            }

            let number_of_purchase_days =
                (dividend_period_end_date - share_start_date).num_days();
            if txn.type_enum.is_purchased()
                && number_of_purchase_days < i64::from(minimum_active_period_days)
            {
                continue;
            }

            if let Some(last_date) = last_dividend_applied_date {
                let delta_days = (share_start_date - last_date).num_days().max(0);
                number_of_share_days_per_account += delta_days * number_of_shares;
            }

            last_dividend_applied_date = Some(share_start_date);
            if txn.type_enum.is_purchased() {
                number_of_shares += txn.total_shares;
            } else if txn.type_enum.is_redeem() {
                number_of_shares = (number_of_shares - txn.total_shares).max(0);
            }
        }

        if let Some(last_date) = last_dividend_applied_date {
            let remaining_days = (dividend_period_end_date - last_date).num_days().max(0);
            number_of_share_days_per_account += remaining_days * number_of_shares;
        }

        number_of_share_days_per_account.max(0)
    }

    /// Pro-rates `dividend_amount` across eligible [`ShareAccount`]s for `product` over
    /// `[dividend_period_start_date, dividend_period_end_date]` weighted by share-holding days (`shares * days_held`).
    ///
    /// Ported from `ShareProductDividendAssembler.calculateDividends`.
    pub fn calculate_dividends(
        product: &ShareProduct,
        accounts: &[ShareAccount],
        dividend_amount: Decimal,
        dividend_period_start_date: NaiveDate,
        dividend_period_end_date: NaiveDate,
    ) -> FineractResult<ShareProductDividendPayOutDetails> {
        let validator = DataValidatorBuilder::for_resource("sharesproduct.dividend.processing");
        validator
            .reset()
            .parameter("dividendAmount")
            .value(dividend_amount)
            .positive_amount();
        validator
            .reset()
            .parameter("dividendPeriodStartDate")
            .value(dividend_period_end_date)
            .validate_date_after(dividend_period_start_date);
        validator.throw_validation_errors()?;

        let eligible_accounts: Vec<&ShareAccount> = accounts
            .iter()
            .filter(|acc| {
                (acc.product_id == product.id || acc.product_id == 0 || product.id == 0)
                    && (acc.status.is_active()
                        || (product.allow_dividend_calculation_for_inactive_clients
                            && acc.status == ShareAccountStatusType::Closed))
            })
            .collect();

        if eligible_accounts.is_empty() {
            return Err(FineractError::resource_not_found(
                "error.msg.shareaccount.id.invalid",
                format!("Share Accounts for product {} do not exist", product.id),
                product.id.to_string(),
            ));
        }

        let min_active_days = product.minimum_active_period.unwrap_or(0);
        let mut share_days_per_account: Vec<(i64, i64)> =
            Vec::with_capacity(eligible_accounts.len());
        let mut total_share_days = 0_i64;

        for account in &eligible_accounts {
            let account_share_days = Self::calculate_account_share_days(
                account,
                dividend_period_start_date,
                dividend_period_end_date,
                min_active_days,
            );
            total_share_days += account_share_days;
            share_days_per_account.push((account.id, account_share_days));
        }

        if total_share_days <= 0 {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.shareproduct.dividend.eligible.shares.not.found",
                "No eligible shares for creating dividends",
            )));
        }

        let currency = MonetaryCurrency::with_code(&product.currency_code, 2);
        let rounding_mode = FineractRoundingMode::HalfEven;
        let normalized_pool = Money::of(currency.clone(), dividend_amount, rounding_mode).amount();

        let mut payout = ShareProductDividendPayOutDetails::new(
            1,
            product.id,
            normalized_pool,
            dividend_period_start_date,
            dividend_period_end_date,
        );

        let total_share_days_dec = Decimal::from(total_share_days);
        for (idx, (account_id, account_share_days)) in share_days_per_account.into_iter().enumerate()
        {
            let raw_share =
                (dividend_amount * Decimal::from(account_share_days)) / total_share_days_dec;
            let rounded_share = Money::of(currency.clone(), raw_share, rounding_mode).amount();
            payout
                .account_dividend_details
                .push(ShareAccountDividendDetails::new(
                    (idx as i64) + 1,
                    payout.id,
                    account_id,
                    rounded_share,
                ));
        }

        Ok(payout)
    }
}

// =============================================================================
// 8. DUAL-DATABASE REPOSITORY (`DualDbSharesRepository`)
// =============================================================================

/// Dual-database repository for [`ShareProduct`], [`ShareAccount`], and [`ShareProductDividendPayOutDetails`]
/// supporting both PostgreSQL and MySQL/MariaDB via `sqlx::AnyPool` as well as lock-free in-memory `DashMap` storage.
#[derive(Debug, Clone)]
pub struct DualDbSharesRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    products: Arc<DashMap<i64, ShareProduct>>,
    accounts: Arc<DashMap<i64, ShareAccount>>,
    dividends: Arc<DashMap<i64, ShareProductDividendPayOutDetails>>,
    next_id: Arc<AtomicI64>,
}

impl Default for DualDbSharesRepository {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl DualDbSharesRepository {
    pub const SHARE_PRODUCT_COLUMNS: &'static str = "id, name, short_name, currency_code, \
        total_shares, issued_shares, unit_price, capital_amount, minimum_client_shares, \
        nominal_client_shares, maximum_client_shares, allow_dividends_inactive_clients, \
        lockin_period_frequency, minimum_active_period_frequency";

    pub const SHARE_MARKET_PRICE_COLUMNS: &'static str =
        "id, product_id, from_date, share_value";

    pub const SHARE_ACCOUNT_COLUMNS: &'static str = "id, account_no, client_id, product_id, \
        savings_account_id, status_enum, total_approved_shares, total_pending_shares";

    pub const SHARE_TRANSACTION_COLUMNS: &'static str = "id, account_id, transaction_date, \
        total_shares, unit_price, amount, charge_amount, status_enum, type_enum";

    pub const SHARE_DIVIDEND_PAYOUT_COLUMNS: &'static str = "id, product_id, amount, \
        dividend_period_start_date, dividend_period_end_date, status";

    /// Creates a new [`DualDbSharesRepository`] backed by a SQLx [`AnyPool`] and [`SqlDialect`].
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            products: Arc::new(DashMap::new()),
            accounts: Arc::new(DashMap::new()),
            dividends: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Creates an in-memory [`DualDbSharesRepository`].
    pub fn in_memory() -> Self {
        Self {
            pool: None,
            dialect: SqlDialect::Postgres,
            products: Arc::new(DashMap::new()),
            accounts: Arc::new(DashMap::new()),
            dividends: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Alias for [`DualDbSharesRepository::in_memory`].
    pub fn new_in_memory() -> Self {
        Self::in_memory()
    }

    /// Creates an in-memory [`DualDbSharesRepository`] configured with `dialect`.
    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            ..Self::in_memory()
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

    // --- Dialect-aware SQL statements ---

    pub fn select_share_product_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_share_product WHERE id = ?",
                Self::SHARE_PRODUCT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_share_product_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_share_product (id, name, short_name, currency_code, total_shares, \
             issued_shares, unit_price, capital_amount, minimum_client_shares, \
             nominal_client_shares, maximum_client_shares, allow_dividends_inactive_clients, \
             lockin_period_frequency, minimum_active_period_frequency, accounting_type) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn select_market_prices_by_product_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_share_product_market_price WHERE product_id = ? ORDER BY from_date ASC, id ASC",
                Self::SHARE_MARKET_PRICE_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_market_price_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_share_product_market_price (id, product_id, from_date, share_value) VALUES (?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn select_share_account_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_share_account WHERE id = ?",
                Self::SHARE_ACCOUNT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_share_accounts_by_product_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_share_account WHERE product_id = ? ORDER BY id ASC",
                Self::SHARE_ACCOUNT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_share_account_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_share_account (id, account_no, client_id, product_id, \
             savings_account_id, status_enum, total_approved_shares, total_pending_shares) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn update_share_account_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_share_account SET account_no = ?, status_enum = ?, \
             total_approved_shares = ?, total_pending_shares = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn select_transactions_by_account_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_share_account_transactions WHERE account_id = ? AND is_active = TRUE ORDER BY transaction_date ASC, id ASC",
                Self::SHARE_TRANSACTION_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_transaction_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_share_account_transactions (id, account_id, transaction_date, \
             total_shares, unit_price, amount, charge_amount, status_enum, type_enum, is_active) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn insert_dividend_payout_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_share_product_dividend_pay_out (id, product_id, amount, \
             dividend_period_start_date, dividend_period_end_date, status) VALUES (?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn insert_account_dividend_detail_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_share_account_dividend_details (id, dividend_pay_out_id, account_id, \
             amount, status, savings_transaction_id) VALUES (?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    // --- ShareProduct Repository Operations ---

    /// Saves or updates a [`ShareProduct`].
    pub async fn save_product(&self, mut product: ShareProduct) -> FineractResult<ShareProduct> {
        if product.id <= 0 {
            product.id = self.allocate_id();
        }
        for mp in &mut product.market_prices {
            if mp.id <= 0 {
                mp.id = self.allocate_id();
            }
            mp.product_id = product.id;
        }

        for entry in self.products.iter() {
            let existing = entry.value();
            if existing.id != product.id {
                if existing.name.eq_ignore_ascii_case(&product.name) {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.shareproduct.duplicate.name",
                        format!("Share Product with name `{}` already exists", product.name),
                    )));
                }
                if existing.short_name.eq_ignore_ascii_case(&product.short_name) {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.shareproduct.duplicate.shortName",
                        format!(
                            "Share Product with shortName `{}` already exists",
                            product.short_name
                        ),
                    )));
                }
            }
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_share_product_sql(self.dialect);
            sqlx::query(&sql)
                .bind(product.id)
                .bind(&product.name)
                .bind(&product.short_name)
                .bind(&product.currency_code)
                .bind(product.total_shares)
                .bind(product.total_shares_issued)
                .bind(product.unit_price.to_string())
                .bind(product.share_capital.to_string())
                .bind(product.minimum_shares)
                .bind(product.nominal_shares.unwrap_or(product.total_shares))
                .bind(product.maximum_shares)
                .bind(product.allow_dividend_calculation_for_inactive_clients)
                .bind(product.lockin_period.map(|v| v as i32))
                .bind(product.minimum_active_period.map(|v| v as i32))
                .bind(1_i32)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

            let mp_sql = Self::insert_market_price_sql(self.dialect);
            for mp in &product.market_prices {
                sqlx::query(&mp_sql)
                    .bind(mp.id)
                    .bind(product.id)
                    .bind(mp.from_date)
                    .bind(mp.share_value.to_string())
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }
        }

        self.products.insert(product.id, product.clone());
        Ok(product)
    }

    /// Finds a [`ShareProduct`] by ID.
    pub async fn find_product_by_id(
        &self,
        product_id: i64,
    ) -> FineractResult<Option<ShareProduct>> {
        if let Some(entry) = self.products.get(&product_id) {
            return Ok(Some(entry.value().clone()));
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_share_product_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(product_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let name: String = row
                    .try_get("name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let short_name: String = row
                    .try_get("short_name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let currency_code: String = row
                    .try_get("currency_code")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let total_shares: i64 = row
                    .try_get("total_shares")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let total_shares_issued: i64 = row
                    .try_get("issued_shares")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let unit_price_str: String = row
                    .try_get("unit_price")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let share_capital_str: String = row
                    .try_get("capital_amount")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let minimum_shares: Option<i64> = row
                    .try_get("minimum_client_shares")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let nominal_shares: Option<i64> = row
                    .try_get("nominal_client_shares")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let maximum_shares: Option<i64> = row
                    .try_get("maximum_client_shares")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let allow_dividends: Option<bool> = row
                    .try_get("allow_dividends_inactive_clients")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let lockin_period: Option<i32> = row
                    .try_get("lockin_period_frequency")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let minimum_active_period: Option<i32> = row
                    .try_get("minimum_active_period_frequency")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

                let mp_sql = Self::select_market_prices_by_product_sql(self.dialect);
                let mp_rows = sqlx::query(&mp_sql)
                    .bind(id)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let mut market_prices = Vec::with_capacity(mp_rows.len());
                for mp_row in mp_rows {
                    let mp_id: i64 = mp_row
                        .try_get("id")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let from_date: NaiveDate = mp_row
                        .try_get("from_date")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let share_val_str: String = mp_row
                        .try_get("share_value")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let share_value = Decimal::from_str(&share_val_str)
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    market_prices.push(ShareMarketPrice::new(mp_id, id, from_date, share_value));
                }

                let product = ShareProduct {
                    id,
                    name,
                    short_name,
                    currency_code,
                    total_shares,
                    total_shares_issued,
                    unit_price: Decimal::from_str(&unit_price_str)
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                    share_capital: Decimal::from_str(&share_capital_str)
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                    minimum_shares,
                    nominal_shares,
                    maximum_shares,
                    allow_dividend_calculation_for_inactive_clients: allow_dividends
                        .unwrap_or(false),
                    lockin_period: lockin_period.and_then(|v| u32::try_from(v).ok()),
                    minimum_active_period: minimum_active_period.and_then(|v| u32::try_from(v).ok()),
                    market_prices,
                };
                self.products.insert(id, product.clone());
                return Ok(Some(product));
            }
        }
        Ok(None)
    }

    /// Retrieves a [`ShareProduct`] by ID or returns `ResourceNotFound` (`error.msg.shareproduct.id.invalid`).
    pub async fn get_product(&self, product_id: i64) -> FineractResult<ShareProduct> {
        self.find_product_by_id(product_id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.shareproduct.id.invalid",
                format!("Share product with identifier {product_id} does not exist"),
                product_id.to_string(),
            )
        })
    }

    /// Lists all [`ShareProduct`]s ordered by ID.
    pub fn list_products(&self) -> Vec<ShareProduct> {
        let mut items: Vec<ShareProduct> =
            self.products.iter().map(|e| e.value().clone()).collect();
        items.sort_by_key(|p| p.id);
        items
    }

    // --- ShareAccount Repository Operations ---

    /// Saves or updates a [`ShareAccount`] and its [`ShareAccountTransaction`]s.
    pub async fn save_account(&self, mut account: ShareAccount) -> FineractResult<ShareAccount> {
        let is_new = account.id <= 0 || !self.accounts.contains_key(&account.id);
        if account.id <= 0 {
            account.id = self.allocate_id();
        }
        if account.account_no.trim().is_empty() {
            account.account_no = ShareAccount::format_account_no(account.id);
        }
        for txn in &mut account.transactions {
            if txn.id <= 0 {
                txn.id = self.allocate_id();
            }
            txn.account_id = account.id;
        }

        if let Some(ref pool) = self.pool {
            if is_new {
                let sql = Self::insert_share_account_sql(self.dialect);
                sqlx::query(&sql)
                    .bind(account.id)
                    .bind(&account.account_no)
                    .bind(account.client_id)
                    .bind(account.product_id)
                    .bind(account.savings_account_id)
                    .bind(account.status.value())
                    .bind(account.total_approved_shares)
                    .bind(account.total_pending_shares)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            } else {
                let sql = Self::update_share_account_sql(self.dialect);
                sqlx::query(&sql)
                    .bind(&account.account_no)
                    .bind(account.status.value())
                    .bind(account.total_approved_shares)
                    .bind(account.total_pending_shares)
                    .bind(account.id)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }
        }

        self.accounts.insert(account.id, account.clone());
        Ok(account)
    }

    /// Finds a [`ShareAccount`] by ID.
    pub async fn find_account_by_id(
        &self,
        account_id: i64,
    ) -> FineractResult<Option<ShareAccount>> {
        if let Some(entry) = self.accounts.get(&account_id) {
            return Ok(Some(entry.value().clone()));
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_share_account_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(account_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let account_no: String = row
                    .try_get("account_no")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let client_id: i64 = row
                    .try_get("client_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let product_id: i64 = row
                    .try_get("product_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let savings_account_id: i64 = row
                    .try_get("savings_account_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let status_val: i32 = row
                    .try_get("status_enum")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let total_approved_shares: Option<i64> = row
                    .try_get("total_approved_shares")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let total_pending_shares: Option<i64> = row
                    .try_get("total_pending_shares")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

                let txn_sql = Self::select_transactions_by_account_sql(self.dialect);
                let txn_rows = sqlx::query(&txn_sql)
                    .bind(id)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let mut transactions = Vec::with_capacity(txn_rows.len());
                for tr in txn_rows {
                    let t_id: i64 = tr
                        .try_get("id")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let t_date: NaiveDate = tr
                        .try_get("transaction_date")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let t_shares: Option<i64> = tr
                        .try_get("total_shares")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let t_unit_price: Option<String> = tr
                        .try_get("unit_price")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let t_amount: Option<String> = tr
                        .try_get("amount")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let t_charge: Option<String> = tr
                        .try_get("charge_amount")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let t_status: Option<i32> = tr
                        .try_get("status_enum")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                    let t_type: Option<i32> = tr
                        .try_get("type_enum")
                        .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

                    transactions.push(ShareAccountTransaction {
                        id: t_id,
                        account_id: id,
                        transaction_date: t_date,
                        total_shares: t_shares.unwrap_or(0),
                        unit_price: t_unit_price
                            .and_then(|s| Decimal::from_str(&s).ok())
                            .unwrap_or(Decimal::ZERO),
                        amount: t_amount
                            .and_then(|s| Decimal::from_str(&s).ok())
                            .unwrap_or(Decimal::ZERO),
                        charge_amount: t_charge
                            .and_then(|s| Decimal::from_str(&s).ok())
                            .unwrap_or(Decimal::ZERO),
                        status_enum: ShareAccountTransactionStatus::from_int(t_status),
                        type_enum: ShareAccountTransactionType::from_int(t_type),
                    });
                }

                let account = ShareAccount {
                    id,
                    account_no,
                    client_id,
                    product_id,
                    savings_account_id,
                    status: ShareAccountStatusType::from_i32(status_val),
                    total_approved_shares: total_approved_shares.unwrap_or(0),
                    total_pending_shares: total_pending_shares.unwrap_or(0),
                    transactions,
                };
                self.accounts.insert(id, account.clone());
                return Ok(Some(account));
            }
        }
        Ok(None)
    }

    /// Retrieves a [`ShareAccount`] by ID or returns `ResourceNotFound` (`error.msg.shareaccount.id.invalid`).
    pub async fn get_account(&self, account_id: i64) -> FineractResult<ShareAccount> {
        self.find_account_by_id(account_id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.shareaccount.id.invalid",
                format!("Share account with identifier {account_id} does not exist"),
                account_id.to_string(),
            )
        })
    }

    /// Lists all [`ShareAccount`]s belonging to `product_id`, ordered by ID.
    pub fn list_accounts_by_product(&self, product_id: i64) -> Vec<ShareAccount> {
        let mut items: Vec<ShareAccount> = self
            .accounts
            .iter()
            .filter(|e| e.value().product_id == product_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|a| a.id);
        items
    }

    // --- Dividend Payout Repository Operations ---

    /// Saves a [`ShareProductDividendPayOutDetails`] aggregate.
    pub async fn save_dividend_payout(
        &self,
        mut payout: ShareProductDividendPayOutDetails,
    ) -> FineractResult<ShareProductDividendPayOutDetails> {
        if payout.id <= 0 || self.dividends.contains_key(&payout.id) {
            payout.id = self.allocate_id();
        }
        for detail in &mut payout.account_dividend_details {
            if detail.id <= 0 {
                detail.id = self.allocate_id();
            }
            detail.dividend_pay_out_id = payout.id;
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_dividend_payout_sql(self.dialect);
            sqlx::query(&sql)
                .bind(payout.id)
                .bind(payout.product_id)
                .bind(payout.amount.to_string())
                .bind(payout.dividend_period_start_date)
                .bind(payout.dividend_period_end_date)
                .bind(payout.status.value())
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

            let detail_sql = Self::insert_account_dividend_detail_sql(self.dialect);
            for detail in &payout.account_dividend_details {
                sqlx::query(&detail_sql)
                    .bind(detail.id)
                    .bind(payout.id)
                    .bind(detail.account_id)
                    .bind(detail.amount.to_string())
                    .bind(detail.status.value())
                    .bind(detail.savings_transaction_id)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }
        }

        self.dividends.insert(payout.id, payout.clone());
        Ok(payout)
    }

    /// Retrieves a [`ShareProductDividendPayOutDetails`] by ID.
    pub async fn get_dividend_payout(
        &self,
        payout_id: i64,
    ) -> FineractResult<ShareProductDividendPayOutDetails> {
        if let Some(entry) = self.dividends.get(&payout_id) {
            return Ok(entry.value().clone());
        }
        Err(FineractError::resource_not_found(
            "error.msg.shareproduct.dividend.id.invalid",
            format!("Share product dividend with identifier {payout_id} does not exist"),
            payout_id.to_string(),
        ))
    }

    /// Lists all [`ShareProductDividendPayOutDetails`] for `product_id`.
    pub fn list_dividend_payouts_by_product(
        &self,
        product_id: i64,
    ) -> Vec<ShareProductDividendPayOutDetails> {
        let mut items: Vec<ShareProductDividendPayOutDetails> = self
            .dividends
            .iter()
            .filter(|e| e.value().product_id == product_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|d| d.id);
        items
    }
}

// =============================================================================
// 9. CQRS COMMAND HANDLERS
// =============================================================================

/// CQRS command handler for `CREATE SHAREPRODUCT`
/// (`org.apache.fineract.portfolio.shareproducts.handler.CreateShareProductCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateShareProductCommandHandler {
    repo: Arc<DualDbSharesRepository>,
}

impl CreateShareProductCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "SHAREPRODUCT";

    pub fn new(repo: Arc<DualDbSharesRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbSharesRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbSharesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateShareProductCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let id = self.repo.allocate_id();
        let product = ShareProduct::from_json(id, command)?;
        let saved = self.repo.save_product(product).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// CQRS command handler for `CREATE SHAREACCOUNT`
/// (`org.apache.fineract.portfolio.shareaccounts.handler.CreateShareAccountCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateShareAccountCommandHandler {
    repo: Arc<DualDbSharesRepository>,
}

impl CreateShareAccountCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "SHAREACCOUNT";

    pub fn new(repo: Arc<DualDbSharesRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbSharesRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbSharesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateShareAccountCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(SHARE_ACCOUNT_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("sharesaccount");

        let client_id = command.long_value_of_parameter_named("clientId")?;
        validator
            .reset()
            .parameter("clientId")
            .value(client_id)
            .not_null()
            .long_greater_than_zero();

        let product_id = command.long_value_of_parameter_named("productId")?;
        validator
            .reset()
            .parameter("productId")
            .value(product_id)
            .not_null()
            .long_greater_than_zero();

        let savings_account_id = command.long_value_of_parameter_named("savingsAccountId")?;
        validator
            .reset()
            .parameter("savingsAccountId")
            .value(savings_account_id)
            .not_null()
            .long_greater_than_zero();

        let requested_shares = command.long_value_of_parameter_named("requestedShares")?;
        validator
            .reset()
            .parameter("requestedShares")
            .value(requested_shares)
            .not_null()
            .long_greater_than_zero();

        let application_date = command
            .local_date_value_of_parameter_named("applicationDate")?
            .or_else(|| {
                command
                    .local_date_value_of_parameter_named("submittedDate")
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(|| ctx.business_date());

        validator.throw_validation_errors()?;

        let product = self.repo.get_product(product_id.unwrap_or(0)).await?;
        let req_shares = requested_shares.unwrap_or(0);
        product.validate_requested_shares(req_shares)?;

        let unit_price = product.derive_unit_price_on(application_date);
        let account_id = self.repo.allocate_id();
        let account_no = command
            .string_value_of_parameter_named("accountNo")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| ShareAccount::format_account_no(account_id));

        let account = ShareAccount::new(
            account_id,
            account_no,
            client_id.unwrap_or(0),
            product.id,
            savings_account_id.unwrap_or(0),
            application_date,
            req_shares,
            unit_price,
        );
        let saved = self.repo.save_account(account).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id(saved.client_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// CQRS command handler for `APPROVE SHAREACCOUNT`
/// (`org.apache.fineract.portfolio.shareaccounts.handler.ApproveShareAccountCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct ApproveShareAccountCommandHandler {
    repo: Arc<DualDbSharesRepository>,
}

impl ApproveShareAccountCommandHandler {
    pub const ACTION_NAME: &'static str = "APPROVE";
    pub const ENTITY_NAME: &'static str = "SHAREACCOUNT";

    pub fn new(repo: Arc<DualDbSharesRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbSharesRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbSharesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for ApproveShareAccountCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(SHARE_ACCOUNT_APPROVAL_PARAMETERS)?;
        let account_id = command.entity_id.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.shareaccount.id.invalid",
                "Share account identifier is required for approval",
                "null",
            )
        })?;

        let approved_date = command
            .local_date_value_of_parameter_named("approvedDate")?
            .unwrap_or_else(|| ctx.business_date());

        let mut account = self.repo.get_account(account_id).await?;
        if let Some(product) = self.repo.find_product_by_id(account.product_id).await? {
            let max_issuable = if product.total_shares_issued > 0 {
                product.total_shares_issued
            } else {
                product.total_shares
            };
            let existing_subscribed: i64 = self
                .repo
                .list_accounts_by_product(product.id)
                .iter()
                .filter(|a| a.id != account.id)
                .map(|a| a.total_approved_shares)
                .sum();
            if existing_subscribed + account.total_pending_shares > max_issuable {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.sharesaccount.shares.requested.can.not.be.approved.exceeding.totalshares.issuable",
                    "Requested shares cannot be approved as they exceed total issuable shares",
                )));
            }
        }

        account.approve(approved_date)?;
        let saved = self.repo.save_account(account).await?;

        let mut changes = BTreeMap::new();
        changes.insert("id".to_string(), Value::from(saved.id));
        changes.insert(
            "approvedDate".to_string(),
            Value::String(approved_date.to_string()),
        );

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id(saved.client_id)
            .with_entity_id(saved.id)
            .with_changes(changes)
            .build())
    }
}

/// CQRS command handler for `ACTIVATE SHAREACCOUNT`
/// (`org.apache.fineract.portfolio.shareaccounts.handler.ActivateShareAccountCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct ActivateShareAccountCommandHandler {
    repo: Arc<DualDbSharesRepository>,
}

impl ActivateShareAccountCommandHandler {
    pub const ACTION_NAME: &'static str = "ACTIVATE";
    pub const ENTITY_NAME: &'static str = "SHAREACCOUNT";

    pub fn new(repo: Arc<DualDbSharesRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbSharesRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbSharesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for ActivateShareAccountCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(SHARE_ACCOUNT_ACTIVATE_PARAMETERS)?;
        let account_id = command.entity_id.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.shareaccount.id.invalid",
                "Share account identifier is required for activation",
                "null",
            )
        })?;

        let activated_date = command
            .local_date_value_of_parameter_named("activatedDate")?
            .unwrap_or_else(|| ctx.business_date());

        let mut account = self.repo.get_account(account_id).await?;
        account.activate(activated_date)?;
        let saved = self.repo.save_account(account).await?;

        let mut changes = BTreeMap::new();
        changes.insert(
            "activatedDate".to_string(),
            Value::String(activated_date.to_string()),
        );

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id(saved.client_id)
            .with_entity_id(saved.id)
            .with_changes(changes)
            .build())
    }
}

/// CQRS command handler for `APPLYADDITIONALSHARES SHAREACCOUNT`
/// (`org.apache.fineract.portfolio.shareaccounts.handler.ApplyAddtionalSharesCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct ApplyAdditionalSharesCommandHandler {
    repo: Arc<DualDbSharesRepository>,
}

impl ApplyAdditionalSharesCommandHandler {
    pub const ACTION_NAME: &'static str = "APPLYADDITIONALSHARES";
    pub const ENTITY_NAME: &'static str = "SHAREACCOUNT";

    pub fn new(repo: Arc<DualDbSharesRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbSharesRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbSharesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for ApplyAdditionalSharesCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(SHARE_ACCOUNT_ADDITIONAL_SHARES_PARAMETERS)?;
        let account_id = command.entity_id.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.shareaccount.id.invalid",
                "Share account identifier is required for applying additional shares",
                "null",
            )
        })?;

        let validator = DataValidatorBuilder::for_resource("sharesaccount");
        let requested_date = command
            .local_date_value_of_parameter_named("requestedDate")?
            .unwrap_or_else(|| ctx.business_date());
        let requested_shares = command.long_value_of_parameter_named("requestedShares")?;
        validator
            .reset()
            .parameter("requestedShares")
            .value(requested_shares)
            .not_null()
            .long_greater_than_zero();
        validator.throw_validation_errors()?;

        let mut account = self.repo.get_account(account_id).await?;
        let txn = if let Some(product) = self.repo.find_product_by_id(account.product_id).await? {
            account.apply_additional_shares(
                requested_date,
                requested_shares.unwrap_or(0),
                &product,
            )?
        } else {
            let explicit_price = command.big_decimal_value_of_parameter_named("unitPrice")?;
            account.apply_additional_shares(
                requested_date,
                requested_shares.unwrap_or(0),
                explicit_price,
            )?
        };

        let saved = self.repo.save_account(account).await?;
        let mut changes = BTreeMap::new();
        changes.insert("additionalshares".to_string(), Value::from(txn.id));

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id(saved.client_id)
            .with_entity_id(saved.id)
            .with_sub_entity_id(txn.id)
            .with_changes(changes)
            .build())
    }
}

/// CQRS command handler for `REDEEMSHARES SHAREACCOUNT`
/// (`org.apache.fineract.portfolio.shareaccounts.handler.RedeemSharesCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct RedeemSharesCommandHandler {
    repo: Arc<DualDbSharesRepository>,
}

impl RedeemSharesCommandHandler {
    pub const ACTION_NAME: &'static str = "REDEEMSHARES";
    pub const ENTITY_NAME: &'static str = "SHAREACCOUNT";

    pub fn new(repo: Arc<DualDbSharesRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbSharesRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbSharesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for RedeemSharesCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(SHARE_ACCOUNT_ADDITIONAL_SHARES_PARAMETERS)?;
        let account_id = command.entity_id.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.shareaccount.id.invalid",
                "Share account identifier is required for redeeming shares",
                "null",
            )
        })?;

        let validator = DataValidatorBuilder::for_resource("sharesaccount");
        let requested_date = command
            .local_date_value_of_parameter_named("requestedDate")?
            .unwrap_or_else(|| ctx.business_date());
        let requested_shares = command.long_value_of_parameter_named("requestedShares")?;
        validator
            .reset()
            .parameter("requestedShares")
            .value(requested_shares)
            .not_null()
            .long_greater_than_zero();
        validator.throw_validation_errors()?;

        let mut account = self.repo.get_account(account_id).await?;
        let txn = if let Some(product) = self.repo.find_product_by_id(account.product_id).await? {
            account.redeem_shares(
                requested_date,
                requested_shares.unwrap_or(0),
                &product,
            )?
        } else {
            let explicit_price = command.big_decimal_value_of_parameter_named("unitPrice")?;
            account.redeem_shares(
                requested_date,
                requested_shares.unwrap_or(0),
                explicit_price,
            )?
        };

        let saved = self.repo.save_account(account).await?;
        let mut changes = BTreeMap::new();
        changes.insert("requestedShares".to_string(), Value::from(txn.id));

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id(saved.client_id)
            .with_entity_id(saved.id)
            .with_sub_entity_id(txn.id)
            .with_changes(changes)
            .build())
    }
}

/// CQRS command handler for `CREATE_DIVIDEND SHAREPRODUCT`
/// (`org.apache.fineract.portfolio.shareproducts.handler.CreateShareProductDividendCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct DeclareShareDividendCommandHandler {
    repo: Arc<DualDbSharesRepository>,
}

/// Java-compatible alias for [`DeclareShareDividendCommandHandler`].
pub type CreateShareProductDividendCommandHandler = DeclareShareDividendCommandHandler;

impl DeclareShareDividendCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE_DIVIDEND";
    pub const ENTITY_NAME: &'static str = "SHAREPRODUCT";

    pub fn new(repo: Arc<DualDbSharesRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbSharesRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbSharesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DeclareShareDividendCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(SHARE_DIVIDEND_SUPPORTED_PARAMETERS)?;
        let product_id = command.entity_id.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.shareproduct.id.invalid",
                "Share product identifier is required for declaring dividends",
                "null",
            )
        })?;

        let validator = DataValidatorBuilder::for_resource("sharesproduct.dividend.processing");
        let start_date = command.local_date_value_of_parameter_named("dividendPeriodStartDate")?;
        validator
            .reset()
            .parameter("dividendPeriodStartDate")
            .value(start_date)
            .not_null();

        let end_date = command.local_date_value_of_parameter_named("dividendPeriodEndDate")?;
        validator
            .reset()
            .parameter("dividendPeriodEndDate")
            .value(end_date)
            .not_null();

        if let (Some(s), Some(e)) = (start_date, end_date) {
            validator
                .reset()
                .parameter("dividendPeriodStartDate")
                .value(e)
                .validate_date_after(s);
        }

        let dividend_amount = command.big_decimal_value_of_parameter_named("dividendAmount")?;
        validator
            .reset()
            .parameter("dividendAmount")
            .value(dividend_amount)
            .not_null()
            .positive_amount();
        validator.throw_validation_errors()?;

        let product = self.repo.get_product(product_id).await?;
        let accounts = self.repo.list_accounts_by_product(product_id);
        let payout = ShareDividendEngine::calculate_dividends(
            &product,
            &accounts,
            dividend_amount.unwrap_or(Decimal::ZERO),
            start_date.expect("validated start date"),
            end_date.expect("validated end date"),
        )?;
        let saved = self.repo.save_dividend_payout(payout).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(product_id)
            .with_sub_entity_id(saved.id)
            .build())
    }
}

// =============================================================================
// 10. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid test date")
    }

    fn sample_product() -> ShareProduct {
        ShareProduct {
            id: 1,
            name: "Member Equity Shares".to_string(),
            short_name: "MES".to_string(),
            currency_code: "USD".to_string(),
            total_shares: 100_000,
            total_shares_issued: 50_000,
            unit_price: dec!(10.00),
            share_capital: dec!(500000.00),
            minimum_shares: Some(10),
            nominal_shares: Some(100),
            maximum_shares: Some(1_000),
            allow_dividend_calculation_for_inactive_clients: false,
            lockin_period: Some(30),
            minimum_active_period: Some(10),
            market_prices: vec![
                ShareMarketPrice::new(1, 1, d(2025, 1, 1), dec!(12.50)),
                ShareMarketPrice::new(2, 1, d(2025, 6, 1), dec!(15.00)),
                ShareMarketPrice::new(3, 1, d(2025, 10, 1), dec!(18.75)),
            ],
        }
    }

    #[test]
    fn enum_discriminants_match_fineract_schema() {
        assert_eq!(ShareAccountStatusType::Invalid as i16, 0);
        assert_eq!(ShareAccountStatusType::SubmittedAndPendingApproval as i16, 100);
        assert_eq!(ShareAccountStatusType::Approved as i16, 200);
        assert_eq!(ShareAccountStatusType::Active as i16, 300);
        assert_eq!(ShareAccountStatusType::Rejected as i16, 500);
        assert_eq!(ShareAccountStatusType::Closed as i16, 600);

        assert_eq!(ShareAccountTransactionType::Invalid as i16, 0);
        assert_eq!(ShareAccountTransactionType::Issue as i16, 100);
        assert_eq!(ShareAccountTransactionType::Purchase as i16, 300);
        assert_eq!(ShareAccountTransactionType::Redeem as i16, 400);
        assert_eq!(ShareAccountTransactionType::ChargePayment as i16, 500);

        assert_eq!(ShareAccountTransactionStatus::Invalid as i16, 0);
        assert_eq!(ShareAccountTransactionStatus::Pending as i16, 100);
        assert_eq!(ShareAccountTransactionStatus::Approved as i16, 300);
        assert_eq!(ShareAccountTransactionStatus::Rejected as i16, 400);

        assert_eq!(ShareProductDividendStatusType::Invalid as i16, 0);
        assert_eq!(ShareProductDividendStatusType::Initiated as i16, 100);
        assert_eq!(ShareProductDividendStatusType::Approved as i16, 300);
        assert_eq!(ShareProductDividendStatusType::Posted as i16, 400);
    }

    #[test]
    fn market_price_lookup_on_date_resolves_effective_period_or_base_unit_price() {
        let product = sample_product();

        // Before first market price period -> base unit_price (10.00)
        assert_eq!(product.derive_unit_price_on(d(2024, 12, 31)), dec!(10.00));

        // On exact start date of first period -> 12.50
        assert_eq!(product.derive_unit_price_on(d(2025, 1, 1)), dec!(12.50));
        assert_eq!(product.derive_unit_price_on(d(2025, 3, 15)), dec!(12.50));

        // On and after second period -> 15.00
        assert_eq!(product.derive_unit_price_on(d(2025, 6, 1)), dec!(15.00));
        assert_eq!(product.derive_unit_price_on(d(2025, 9, 30)), dec!(15.00));

        // On and after third period -> 18.75
        assert_eq!(product.derive_unit_price_on(d(2025, 10, 1)), dec!(18.75));
        assert_eq!(product.derive_unit_price_on(d(2026, 2, 1)), dec!(18.75));
    }

    #[test]
    fn share_purchase_approval_and_rejection_update_account_balances_and_transactions() {
        let product = sample_product();
        let mut account = ShareAccount::new(
            10,
            "000000010",
            100,
            product.id,
            200,
            d(2025, 1, 5),
            100,
            product.derive_unit_price_on(d(2025, 1, 5)),
        );

        assert_eq!(
            account.status,
            ShareAccountStatusType::SubmittedAndPendingApproval
        );
        assert_eq!(account.total_approved_shares, 0);
        assert_eq!(account.total_pending_shares, 100);
        assert_eq!(account.transactions[0].amount, dec!(1250.00));

        // Approve & activate initial application
        account.approve(d(2025, 1, 6)).expect("approve account");
        assert_eq!(account.status, ShareAccountStatusType::Approved);
        assert_eq!(account.total_approved_shares, 100);
        assert_eq!(account.total_pending_shares, 0);
        assert_eq!(
            account.transactions[0].status_enum,
            ShareAccountTransactionStatus::Approved
        );

        account.activate(d(2025, 1, 7)).expect("activate account");
        assert_eq!(account.status, ShareAccountStatusType::Active);

        // Apply two additional share purchase requests
        let add_txn_1 = account
            .apply_additional_shares(d(2025, 6, 10), 50, &product)
            .expect("apply additional shares 1");
        assert_eq!(add_txn_1.unit_price, dec!(15.00));
        assert_eq!(add_txn_1.amount, dec!(750.00));
        assert_eq!(add_txn_1.status_enum, ShareAccountTransactionStatus::Pending);
        assert_eq!(account.total_approved_shares, 100);
        assert_eq!(account.total_pending_shares, 50);

        let add_txn_2 = account
            .apply_additional_shares(d(2025, 6, 15), 80, &product)
            .expect("apply additional shares 2");
        assert_eq!(account.total_pending_shares, 130);

        // Approve first additional purchase -> moves 50 shares from pending to approved
        account
            .approve_additional_shares(add_txn_1.id)
            .expect("approve additional shares 1");
        assert_eq!(account.total_approved_shares, 150);
        assert_eq!(account.total_pending_shares, 80);

        // Reject second additional purchase -> removes 80 shares from pending without adding to approved
        account
            .reject_additional_shares(add_txn_2.id)
            .expect("reject additional shares 2");
        assert_eq!(account.total_approved_shares, 150);
        assert_eq!(account.total_pending_shares, 0);
        assert_eq!(
            account
                .transactions
                .iter()
                .find(|t| t.id == add_txn_2.id)
                .unwrap()
                .status_enum,
            ShareAccountTransactionStatus::Rejected
        );
    }

    #[test]
    fn share_redemption_enforces_lockin_period_and_approved_balance() {
        let product = sample_product(); // lockin_period = Some(30) days
        let mut account = ShareAccount::new(
            11,
            "000000011",
            101,
            product.id,
            201,
            d(2025, 1, 1),
            100,
            product.derive_unit_price_on(d(2025, 1, 1)),
        );
        account.approve(d(2025, 1, 2)).unwrap();
        account.activate(d(2025, 1, 2)).unwrap();

        // 1. Attempting to redeem more than total_approved_shares (150 > 100) fails
        let over_redeem = account.redeem_shares(d(2025, 3, 1), 150, &product);
        assert!(over_redeem.is_err());

        // 2. Attempting to redeem within the 30-day lock-in period (2025-01-15 < 2025-01-31) fails
        let locked_redeem = account.redeem_shares(d(2025, 1, 15), 40, &product);
        assert!(locked_redeem.is_err());

        // 3. Redeeming on or after lock-in expiry (2025-01-31 is 30 days after 2025-01-01) succeeds
        let redeem_txn = account
            .redeem_shares(d(2025, 1, 31), 40, &product)
            .expect("redeem after lockin");
        assert_eq!(redeem_txn.type_enum, ShareAccountTransactionType::Redeem);
        assert_eq!(
            redeem_txn.status_enum,
            ShareAccountTransactionStatus::Approved
        );
        assert_eq!(redeem_txn.total_shares, 40);
        assert_eq!(redeem_txn.unit_price, dec!(12.50));
        assert_eq!(redeem_txn.amount, dec!(500.00));
        assert_eq!(account.total_approved_shares, 60);

        // 4. Close account redeems remaining 60 shares and transitions to Closed
        let close_txn = account
            .close(d(2025, 6, 10), &product)
            .expect("close account")
            .expect("remaining shares redeemed on close");
        assert_eq!(close_txn.total_shares, 60);
        assert_eq!(close_txn.unit_price, dec!(15.00));
        assert_eq!(close_txn.amount, dec!(900.00));
        assert_eq!(account.total_approved_shares, 0);
        assert_eq!(account.status, ShareAccountStatusType::Closed);
    }

    #[test]
    fn pro_rata_dividend_calculation_weights_by_share_holding_days() {
        let product = sample_product(); // minimum_active_period = Some(10)

        // Account 1: 100 shares purchased on 2025-01-01 -> held for full 30 days of [2025-01-01, 2025-01-31]
        // Share-days = 100 * 30 = 3,000
        let mut acc1 = ShareAccount::new(
            1,
            "000000001",
            101,
            product.id,
            301,
            d(2025, 1, 1),
            100,
            dec!(12.50),
        );
        acc1.approve(d(2025, 1, 1)).unwrap();
        acc1.activate(d(2025, 1, 1)).unwrap();

        // Account 2: 100 shares purchased on 2025-01-16 -> held for 15 days of [2025-01-01, 2025-01-31]
        // Share-days = 100 * 15 = 1,500
        let mut acc2 = ShareAccount::new(
            2,
            "000000002",
            102,
            product.id,
            302,
            d(2025, 1, 16),
            100,
            dec!(12.50),
        );
        acc2.approve(d(2025, 1, 16)).unwrap();
        acc2.activate(d(2025, 1, 16)).unwrap();

        let payout = ShareDividendEngine::calculate_dividends(
            &product,
            &[acc1, acc2],
            dec!(900.00),
            d(2025, 1, 1),
            d(2025, 1, 31),
        )
        .expect("calculate pro-rata dividends");

        assert_eq!(payout.product_id, product.id);
        assert_eq!(payout.amount, dec!(900.00));
        assert_eq!(payout.status, ShareProductDividendStatusType::Initiated);
        assert_eq!(payout.account_dividend_details.len(), 2);

        // Total share-days = 3,000 + 1,500 = 4,500
        // Account 1 receives 3000/4500 * 900 = 600.00
        // Account 2 receives 1500/4500 * 900 = 300.00
        let d1 = payout
            .account_dividend_details
            .iter()
            .find(|d| d.account_id == 1)
            .unwrap();
        let d2 = payout
            .account_dividend_details
            .iter()
            .find(|d| d.account_id == 2)
            .unwrap();
        assert_eq!(d1.amount, dec!(600.00));
        assert_eq!(d2.amount, dec!(300.00));
    }

    #[tokio::test]
    async fn cqrs_handlers_execute_end_to_end_lifecycle() {
        let repo = Arc::new(DualDbSharesRepository::in_memory());
        let ctx = ExecutionContext::new("default", d(2025, 3, 1));

        // 1. Create Share Product
        let create_prod_handler = CreateShareProductCommandHandler::new(Arc::clone(&repo));
        let prod_cmd = JsonCommand::from_json(
            r#"{
                "name": "Cooperative Equity",
                "shortName": "CEQ",
                "currencyCode": "USD",
                "totalShares": 10000,
                "sharesIssued": 5000,
                "unitPrice": 20.0,
                "minimumShares": 5,
                "nominalShares": 50,
                "maximumShares": 500,
                "allowDividendCalculationForInactiveClients": true,
                "lockinPeriodFrequency": 10,
                "minimumActivePeriodForDividends": 5,
                "locale": "en",
                "dateFormat": "yyyy-MM-dd",
                "marketPricePeriods": [
                    {"fromDate": "2025-01-01", "shareValue": 25.0, "locale": "en", "dateFormat": "yyyy-MM-dd"}
                ]
            }"#,
        )
        .unwrap();
        let prod_res = create_prod_handler.handle(&ctx, &prod_cmd).await.unwrap();
        let product_id = prod_res.resource_id.unwrap();

        // 2. Create Share Account
        let create_acc_handler = CreateShareAccountCommandHandler::new(Arc::clone(&repo));
        let acc_cmd = JsonCommand::from_json(format!(
            r#"{{
                "clientId": 77,
                "productId": {product_id},
                "savingsAccountId": 88,
                "requestedShares": 100,
                "applicationDate": "2025-01-01",
                "submittedDate": "2025-01-01",
                "locale": "en",
                "dateFormat": "yyyy-MM-dd"
            }}"#
        ))
        .unwrap();
        let acc_res = create_acc_handler.handle(&ctx, &acc_cmd).await.unwrap();
        let account_id = acc_res.resource_id.unwrap();

        // 3. Approve & Activate Share Account
        let approve_handler = ApproveShareAccountCommandHandler::new(Arc::clone(&repo));
        let approve_cmd = JsonCommand::from_json(
            r#"{"approvedDate": "2025-01-02", "locale": "en", "dateFormat": "yyyy-MM-dd"}"#,
        )
        .unwrap()
        .with_entity_id(account_id);
        approve_handler.handle(&ctx, &approve_cmd).await.unwrap();

        let activate_handler = ActivateShareAccountCommandHandler::new(Arc::clone(&repo));
        let activate_cmd = JsonCommand::from_json(
            r#"{"activatedDate": "2025-01-02", "locale": "en", "dateFormat": "yyyy-MM-dd"}"#,
        )
        .unwrap()
        .with_entity_id(account_id);
        activate_handler.handle(&ctx, &activate_cmd).await.unwrap();

        // 4. Apply Additional Shares & Redeem Shares
        let add_handler = ApplyAdditionalSharesCommandHandler::new(Arc::clone(&repo));
        let add_cmd = JsonCommand::from_json(
            r#"{"requestedDate": "2025-01-15", "requestedShares": 20, "locale": "en", "dateFormat": "yyyy-MM-dd"}"#,
        )
        .unwrap()
        .with_entity_id(account_id);
        add_handler.handle(&ctx, &add_cmd).await.unwrap();

        let redeem_handler = RedeemSharesCommandHandler::new(Arc::clone(&repo));
        let redeem_cmd = JsonCommand::from_json(
            r#"{"requestedDate": "2025-01-20", "requestedShares": 30, "locale": "en", "dateFormat": "yyyy-MM-dd"}"#,
        )
        .unwrap()
        .with_entity_id(account_id);
        redeem_handler.handle(&ctx, &redeem_cmd).await.unwrap();

        let updated_acc = repo.get_account(account_id).await.unwrap();
        assert_eq!(updated_acc.total_approved_shares, 70);
        assert_eq!(updated_acc.total_pending_shares, 20);

        // 5. Declare Share Dividend
        let div_handler = DeclareShareDividendCommandHandler::new(Arc::clone(&repo));
        let div_cmd = JsonCommand::from_json(
            r#"{
                "dividendPeriodStartDate": "2025-01-01",
                "dividendPeriodEndDate": "2025-01-31",
                "dividendAmount": 500.00,
                "locale": "en",
                "dateFormat": "yyyy-MM-dd"
            }"#,
        )
        .unwrap()
        .with_entity_id(product_id);
        let div_res = div_handler.handle(&ctx, &div_cmd).await.unwrap();
        let payout_id = div_res.sub_resource_id.unwrap();
        let payout = repo.get_dividend_payout(payout_id).await.unwrap();
        assert_eq!(payout.amount, dec!(500.00));
        assert_eq!(payout.account_dividend_details.len(), 1);
        assert_eq!(payout.account_dividend_details[0].amount, dec!(500.00));
    }
}
