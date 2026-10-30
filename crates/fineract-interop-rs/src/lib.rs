//! Fineract Interoperation & Mojaloop DFSP Adapter (`org.apache.fineract.interoperation`).
//!
//! Ported from Apache Fineract's `fineract-core`, `fineract-savings`, and `fineract-provider`
//! interoperation modules:
//! - [`InteropIdentifierType`] (`InteropIdentifierType.java`)
//! - [`InteropTransactionRole`] (`InteropTransactionRole.java`)
//! - [`InteropActionState`] (`InteropActionState.java`)
//! - [`InteropIdentifier`] (`InteropIdentifier.java`, table `interop_identifier`)
//! - [`InteropQuoteRequestData`] & [`InteropQuoteResponseData`]
//! - [`InteropTransferRequestData`] & [`InteropTransferResponseData`]
//! - [`DualDbInteropRepository`] (PostgreSQL & MySQL SQL dialect support + in-memory store)
//! - [`InteroperationService`] (`InteropServiceImpl.java`: party identifier registration/lookup/deletion,
//!   quote calculation, transfer prepare/hold, transfer commit/settlement, transfer release/abort)
//! - CQRS Command Handlers implementing [`fineract_command_rs::CommandHandler`]

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use dashmap::DashMap;
use fineract_command_rs::{CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder};
use fineract_core_rs::{
    DialectSql, ExecutionContext, ExternalId, JsonCommand, SqlDialect,
};
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use fineract_savings_rs::{
    SavingsAccount, SavingsAccountTransaction, SavingsAccountTransactionType,
};
use fineract_validation_rs::{
    DataValidatorBuilder, FineractError, FineractResult, PlatformDomainRuleException,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;

// ============================================================================
// 1. Interoperation Constants (`InteropUtil.java`)
// ============================================================================

/// Root API path for Fineract Interoperation (`"interoperation"`).
pub const INTEROP_ROOT_PATH: &str = "interoperation";

/// Default routing code recorded on interop payment details (`"INTEROPERATION"`).
pub const DEFAULT_ROUTING_CODE: &str = "INTEROPERATION";

/// Entity name for party identifiers (`"INTERID"`).
pub const ENTITY_NAME_IDENTIFIER: &str = "INTERID";
/// Entity name for interop payment requests (`"INTERREQUEST"`).
pub const ENTITY_NAME_REQUEST: &str = "INTERREQUEST";
/// Entity name for interop quotes (`"INTERQUOTE"`).
pub const ENTITY_NAME_QUOTE: &str = "INTERQUOTE";
/// Entity name for interop transfers (`"INTERTRANSFER"`).
pub const ENTITY_NAME_TRANSFER: &str = "INTERTRANSFER";

/// Action name for transfer preparation (`"PREPARE"`).
pub const ACTION_TRANSFER_PREPARE: &str = "PREPARE";
/// Action name for transfer settlement/commit (`"CREATE"`).
pub const ACTION_TRANSFER_COMMIT: &str = "CREATE";
/// Action name for transfer hold release (`"RELEASE"`).
pub const ACTION_TRANSFER_RELEASE: &str = "RELEASE";

// JSON parameter names matching `InteropUtil.java`
pub const PARAM_TRANSACTION_CODE: &str = "transactionCode";
pub const PARAM_REQUEST_CODE: &str = "requestCode";
pub const PARAM_QUOTE_CODE: &str = "quoteCode";
pub const PARAM_TRANSFER_CODE: &str = "transferCode";
pub const PARAM_ACCOUNT_ID: &str = "accountId";
pub const PARAM_AMOUNT: &str = "amount";
pub const PARAM_CURRENCY: &str = "currency";
pub const PARAM_AMOUNT_TYPE: &str = "amountType";
pub const PARAM_TRANSACTION_ROLE: &str = "transactionRole";
pub const PARAM_TRANSACTION_TYPE: &str = "transactionType";
pub const PARAM_NOTE: &str = "note";
pub const PARAM_EXPIRATION: &str = "expiration";
pub const PARAM_EXTENSION_LIST: &str = "extensionList";
pub const PARAM_FEES: &str = "fees";
pub const PARAM_FSP_FEE: &str = "fspFee";
pub const PARAM_FSP_COMMISSION: &str = "fspCommission";
pub const PARAM_IDENTIFIER_TYPE: &str = "idType";
pub const PARAM_IDENTIFIER_VALUE: &str = "idValue";
pub const PARAM_SUB_ID_OR_TYPE: &str = "subIdOrType";

// ============================================================================
// 2. Core Interoperation Enums
// ============================================================================

/// Party identifier type in the Mojaloop / Fineract Interoperation ALS directory
/// (`org.apache.fineract.interoperation.domain.InteropIdentifierType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(i32)]
pub enum InteropIdentifierType {
    Msisdn = 1,
    Email = 2,
    PersonalId = 3,
    Business = 4,
    Device = 5,
    AccountId = 6,
    Iban = 7,
    Alias = 8,
    Bban = 9,
}

impl InteropIdentifierType {
    /// Returns the integer database discriminant (`1..=9`) stored in `interop_identifier.type`.
    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    /// Resolves an [`InteropIdentifierType`] from its database integer discriminant.
    #[inline]
    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Msisdn),
            2 => Some(Self::Email),
            3 => Some(Self::PersonalId),
            4 => Some(Self::Business),
            5 => Some(Self::Device),
            6 => Some(Self::AccountId),
            7 => Some(Self::Iban),
            8 => Some(Self::Alias),
            9 => Some(Self::Bban),
            _ => None,
        }
    }

    /// Returns the canonical Mojaloop / Fineract uppercase identifier type code.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Msisdn => "MSISDN",
            Self::Email => "EMAIL",
            Self::PersonalId => "PERSONAL_ID",
            Self::Business => "BUSINESS",
            Self::Device => "DEVICE",
            Self::AccountId => "ACCOUNT_ID",
            Self::Iban => "IBAN",
            Self::Alias => "ALIAS",
            Self::Bban => "BBAN",
        }
    }

    /// Returns the localization code matching `InteropIdentifierType.getCode()`.
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Msisdn => "interopIdentifierType.msisdn",
            Self::Email => "interopIdentifierType.email",
            Self::PersonalId => "interopIdentifierType.personal_id",
            Self::Business => "interopIdentifierType.business",
            Self::Device => "interopIdentifierType.device",
            Self::AccountId => "interopIdentifierType.account_id",
            Self::Iban => "interopIdentifierType.iban",
            Self::Alias => "interopIdentifierType.alias",
            Self::Bban => "interopIdentifierType.bban",
        }
    }

    /// Resolves an identifier type by name (case-insensitive).
    pub fn resolve_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "MSISDN" => Some(Self::Msisdn),
            "EMAIL" => Some(Self::Email),
            "PERSONAL_ID" | "PERSONALID" => Some(Self::PersonalId),
            "BUSINESS" => Some(Self::Business),
            "DEVICE" => Some(Self::Device),
            "ACCOUNT_ID" | "ACCOUNTID" => Some(Self::AccountId),
            "IBAN" => Some(Self::Iban),
            "ALIAS" => Some(Self::Alias),
            "BBAN" => Some(Self::Bban),
            _ => None,
        }
    }
}

impl fmt::Display for InteropIdentifierType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InteropIdentifierType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::resolve_name(s).ok_or_else(|| {
            FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.interop.identifier.type.invalid",
                format!("Invalid interop identifier type: {s}"),
                vec![s.to_string().into()],
            ))
        })
    }
}

/// Role of the DFSP party in an interoperable transaction
/// (`org.apache.fineract.interoperation.domain.InteropTransactionRole`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InteropTransactionRole {
    /// Sending / debit side of the transfer (`Withdrawal` on savings account).
    Payer,
    /// Receiving / credit side of the transfer (`Deposit` on savings account).
    Payee,
}

impl InteropTransactionRole {
    /// Returns `true` if this role is [`InteropTransactionRole::Payer`].
    #[inline]
    pub const fn is_payer(self) -> bool {
        matches!(self, Self::Payer)
    }

    /// Returns `true` if this role is [`InteropTransactionRole::Payee`].
    #[inline]
    pub const fn is_payee(self) -> bool {
        matches!(self, Self::Payee)
    }

    /// Matches `InteropTransactionRole.isWithdraw()` in Fineract (`true` for `Payer`, `false` for `Payee`).
    #[inline]
    pub const fn is_withdraw(self) -> bool {
        matches!(self, Self::Payer)
    }

    /// Maps the interop role to the corresponding [`SavingsAccountTransactionType`]
    /// (`Payer -> Withdrawal`, `Payee -> Deposit`), matching `InteropTransactionRole.getTransactionType()`.
    #[inline]
    pub const fn transaction_type(self) -> SavingsAccountTransactionType {
        match self {
            Self::Payer => SavingsAccountTransactionType::Withdrawal,
            Self::Payee => SavingsAccountTransactionType::Deposit,
        }
    }

    /// Returns `"PAYER"` or `"PAYEE"`.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Payer => "PAYER",
            Self::Payee => "PAYEE",
        }
    }
}

impl fmt::Display for InteropTransactionRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InteropTransactionRole {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "PAYER" => Ok(Self::Payer),
            "PAYEE" => Ok(Self::Payee),
            _ => Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.interop.transaction.role.invalid",
                format!("Invalid interop transaction role: {s}"),
                vec![s.to_string().into()],
            ))),
        }
    }
}

/// Execution state of an interop request, quote, or transfer
/// (`org.apache.fineract.interoperation.domain.InteropActionState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InteropActionState {
    Accepted,
    Rejected,
}

impl InteropActionState {
    #[inline]
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }

    #[inline]
    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "ACCEPTED",
            Self::Rejected => "REJECTED",
        }
    }
}

impl fmt::Display for InteropActionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InteropActionState {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "ACCEPTED" => Ok(Self::Accepted),
            "REJECTED" => Ok(Self::Rejected),
            _ => Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.interop.action.state.invalid",
                format!("Invalid interop action state: {s}"),
                vec![s.to_string().into()],
            ))),
        }
    }
}

/// Mojaloop amount type (`SEND` or `RECEIVE`)
/// (`org.apache.fineract.interoperation.domain.InteropAmountType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InteropAmountType {
    #[default]
    Send,
    Receive,
}

impl InteropAmountType {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Send => "SEND",
            Self::Receive => "RECEIVE",
        }
    }
}

impl fmt::Display for InteropAmountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InteropAmountType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "SEND" => Ok(Self::Send),
            "RECEIVE" => Ok(Self::Receive),
            _ => Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.interop.amount.type.invalid",
                format!("Invalid interop amount type: {s}"),
                vec![s.to_string().into()],
            ))),
        }
    }
}

/// Transfer lifecycle action (`PREPARE`, `CREATE`, `RELEASE`)
/// (`org.apache.fineract.interoperation.domain.InteropTransferActionType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InteropTransferActionType {
    Prepare,
    Create,
    Release,
}

impl InteropTransferActionType {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prepare => ACTION_TRANSFER_PREPARE,
            Self::Create => ACTION_TRANSFER_COMMIT,
            Self::Release => ACTION_TRANSFER_RELEASE,
        }
    }
}

impl fmt::Display for InteropTransferActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InteropTransferActionType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "PREPARE" => Ok(Self::Prepare),
            "CREATE" | "COMMIT" => Ok(Self::Create),
            "RELEASE" => Ok(Self::Release),
            _ => Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.interop.transfer.action.invalid",
                format!("Invalid interop transfer action: {s}"),
                vec![s.to_string().into()],
            ))),
        }
    }
}

// ============================================================================
// 3. Domain Entities & DTOs
// ============================================================================

/// Key-value extension entry in Mojaloop payloads (`ExtensionData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InteropExtensionData {
    pub key: String,
    pub value: String,
}

impl InteropExtensionData {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// Geographic coordinate pair in Mojaloop payloads (`GeoCodeData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeoCodeData {
    pub latitude: String,
    pub longitude: String,
}

/// Mojaloop transaction scenario metadata (`InteropTransactionTypeData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropTransactionTypeData {
    pub scenario: String,
    pub sub_scenario: Option<String>,
    pub initiator: InteropTransactionRole,
    pub initiator_type: String,
}

/// Wire representation of a monetary amount + currency code (`MoneyData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MoneyData {
    pub amount: Decimal,
    pub currency: String,
}

impl MoneyData {
    pub fn new(amount: Decimal, currency: impl Into<String>) -> Self {
        Self {
            amount,
            currency: currency.into(),
        }
    }

    pub fn from_money(money: &Money) -> Self {
        Self {
            amount: money.amount(),
            currency: money.currency_code().to_string(),
        }
    }

    pub fn to_money(&self, default_digits: u32) -> Money {
        let currency = MonetaryCurrency::new(self.currency.clone(), default_digits, None);
        Money::of(currency, self.amount, FineractRoundingMode::HalfEven)
    }

    pub fn to_money_with_currency(&self, currency: &MonetaryCurrency) -> FineractResult<Money> {
        if !currency.code.eq_ignore_ascii_case(self.currency.trim()) {
            return Err(InteropDomainException::currency_mismatch(
                &currency.code,
                &self.currency,
            ));
        }
        Ok(Money::of(
            currency.clone(),
            self.amount,
            FineractRoundingMode::HalfEven,
        ))
    }
}

/// Party identifier mapping an external Mojaloop party alias (`MSISDN`, `IBAN`, `EMAIL`, etc.)
/// to a Fineract [`SavingsAccount`] (`org.apache.fineract.interoperation.domain.InteropIdentifier`,
/// table `interop_identifier`).
///
/// Database schema (`interop_identifier`):
/// - `id` (`BIGINT PRIMARY KEY AUTO_INCREMENT`)
/// - `account_id` (`BIGINT NOT NULL`, FK to `m_savings_account.id`)
/// - `type` (`SMALLINT NOT NULL`)
/// - `a_value` (`VARCHAR(128) NOT NULL`)
/// - `sub_value_or_type` (`VARCHAR(128) NULL`)
/// - `created_by` (`VARCHAR(32) NOT NULL`)
/// - `created_on` (`TIMESTAMP NOT NULL`)
/// - `modified_by` (`VARCHAR(32) NULL`)
/// - `modified_on` (`TIMESTAMP NULL`)
/// - Unique constraints:
///   - `uk_hathor_identifier_account`: `(account_id, type)`
///   - `uk_hathor_identifier_value`: `(type, a_value, sub_value_or_type)`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropIdentifier {
    pub id: i64,
    pub account_id: i64,
    pub identifier_type: InteropIdentifierType,
    pub a_value: String,
    pub sub_value_or_type: Option<String>,
    pub created_by: String,
    pub created_on: DateTime<Utc>,
    pub modified_by: Option<String>,
    pub modified_on: Option<DateTime<Utc>>,
}

impl InteropIdentifier {
    /// Creates a new [`InteropIdentifier`] with default audit metadata.
    pub fn new(
        account_id: i64,
        identifier_type: InteropIdentifierType,
        a_value: impl Into<String>,
        sub_value_or_type: Option<String>,
    ) -> Self {
        Self::with_audit(
            0,
            account_id,
            identifier_type,
            a_value,
            sub_value_or_type,
            "system",
            Utc::now(),
        )
    }

    /// Creates a new [`InteropIdentifier`] with explicit ID and audit metadata.
    pub fn with_audit(
        id: i64,
        account_id: i64,
        identifier_type: InteropIdentifierType,
        a_value: impl Into<String>,
        sub_value_or_type: Option<String>,
        created_by: impl Into<String>,
        created_on: DateTime<Utc>,
    ) -> Self {
        let normalized_sub = sub_value_or_type.and_then(|s| {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        Self {
            id,
            account_id,
            identifier_type,
            a_value: a_value.into().trim().to_string(),
            sub_value_or_type: normalized_sub,
            created_by: created_by.into(),
            created_on,
            modified_by: None,
            modified_on: None,
        }
    }

    /// Sets the database identifier on this record.
    pub const fn with_id(mut self, id: i64) -> Self {
        self.id = id;
        self
    }
}

/// Request payload for registering or deleting a party identifier (`InteropIdentifierRequestData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropIdentifierRequestData {
    pub id_type: InteropIdentifierType,
    pub id_value: String,
    pub sub_id_or_type: Option<String>,
    pub account_id: String,
}

/// Response payload returning the account identifier associated with a party identifier
/// (`InteropIdentifierAccountResponseData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropIdentifierAccountResponseData {
    pub resource_id: Option<i64>,
    pub office_id: Option<i64>,
    pub account_id: String,
}

/// Request payload for calculating an interoperable quote (`InteropQuoteRequestData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropQuoteRequestData {
    pub transaction_code: String,
    pub request_code: Option<String>,
    pub account_id: i64,
    pub amount: Money,
    pub amount_type: InteropAmountType,
    pub transaction_role: InteropTransactionRole,
    pub transaction_type: Option<InteropTransactionTypeData>,
    pub note: Option<String>,
    pub geo_code: Option<GeoCodeData>,
    pub expiration: Option<DateTime<Utc>>,
    pub extension_list: Vec<InteropExtensionData>,
    pub quote_code: String,
    pub fees: Option<Money>,
    pub fsp_commission: Option<Money>,
    pub transaction_date: Option<NaiveDate>,
}

impl InteropQuoteRequestData {
    /// Constructs a new [`InteropQuoteRequestData`] with required Mojaloop quote fields.
    pub fn new(
        transaction_code: impl Into<String>,
        quote_code: impl Into<String>,
        account_id: i64,
        amount: Money,
        transaction_role: InteropTransactionRole,
    ) -> Self {
        Self {
            transaction_code: transaction_code.into(),
            request_code: None,
            account_id,
            amount,
            amount_type: InteropAmountType::Send,
            transaction_role,
            transaction_type: None,
            note: None,
            geo_code: None,
            expiration: None,
            extension_list: Vec::new(),
            quote_code: quote_code.into(),
            fees: None,
            fsp_commission: None,
            transaction_date: None,
        }
    }

    pub fn with_fees(mut self, fees: Money) -> Self {
        self.fees = Some(fees);
        self
    }

    pub fn with_fsp_commission(mut self, commission: Money) -> Self {
        self.fsp_commission = Some(commission);
        self
    }

    pub fn with_expiration(mut self, expiration: DateTime<Utc>) -> Self {
        self.expiration = Some(expiration);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// Response payload returned from quote calculation (`InteropQuoteResponseData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropQuoteResponseData {
    pub office_id: Option<i64>,
    pub client_id: Option<i64>,
    pub savings_id: Option<i64>,
    pub resource_id: Option<i64>,
    pub transaction_code: String,
    pub quote_code: String,
    pub amount: Money,
    pub transaction_role: InteropTransactionRole,
    pub fsp_fee: Option<Money>,
    pub fsp_commission: Option<Money>,
    pub expiration: Option<DateTime<Utc>>,
    pub state: InteropActionState,
    pub extension_list: Vec<InteropExtensionData>,
}

/// Request payload for preparing, committing, or releasing an interoperable transfer
/// (`InteropTransferRequestData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropTransferRequestData {
    pub transaction_code: String,
    pub request_code: Option<String>,
    pub transfer_code: String,
    pub account_id: i64,
    pub amount: Money,
    pub transaction_role: InteropTransactionRole,
    pub transaction_type: Option<InteropTransactionTypeData>,
    pub fsp_fee: Option<Money>,
    pub fsp_commission: Option<Money>,
    pub note: Option<String>,
    pub geo_code: Option<GeoCodeData>,
    pub expiration: Option<DateTime<Utc>>,
    pub extension_list: Vec<InteropExtensionData>,
    pub transaction_date: Option<NaiveDate>,
}

impl InteropTransferRequestData {
    /// Constructs a new [`InteropTransferRequestData`] with required Mojaloop transfer fields.
    pub fn new(
        transaction_code: impl Into<String>,
        transfer_code: impl Into<String>,
        account_id: i64,
        amount: Money,
        transaction_role: InteropTransactionRole,
    ) -> Self {
        Self {
            transaction_code: transaction_code.into(),
            request_code: None,
            transfer_code: transfer_code.into(),
            account_id,
            amount,
            transaction_role,
            transaction_type: None,
            fsp_fee: None,
            fsp_commission: None,
            note: None,
            geo_code: None,
            expiration: None,
            extension_list: Vec::new(),
            transaction_date: None,
        }
    }

    pub fn with_fsp_fee(mut self, fsp_fee: Money) -> Self {
        self.fsp_fee = Some(fsp_fee);
        self
    }

    pub fn with_fsp_commission(mut self, fsp_commission: Money) -> Self {
        self.fsp_commission = Some(fsp_commission);
        self
    }

    pub fn with_expiration(mut self, expiration: DateTime<Utc>) -> Self {
        self.expiration = Some(expiration);
        self
    }

    pub fn with_transaction_date(mut self, date: NaiveDate) -> Self {
        self.transaction_date = Some(date);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// Response payload returned from preparing, committing, or releasing an interoperable transfer
/// (`InteropTransferResponseData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropTransferResponseData {
    pub office_id: Option<i64>,
    pub client_id: Option<i64>,
    pub savings_id: Option<i64>,
    pub resource_id: Option<i64>,
    pub transaction_code: String,
    pub transfer_code: String,
    pub account_id: i64,
    pub amount: Money,
    pub transaction_role: InteropTransactionRole,
    pub fsp_fee: Option<Money>,
    pub fsp_commission: Option<Money>,
    pub expiration: Option<DateTime<Utc>>,
    pub state: InteropActionState,
    pub completed_timestamp: Option<DateTime<Utc>>,
    pub extension_list: Vec<InteropExtensionData>,
    pub hold_transaction_id: Option<i64>,
    pub release_transaction_id: Option<i64>,
    pub settlement_transaction_id: Option<i64>,
}

// ============================================================================
// 4. Interoperation Domain Exceptions (`org.apache.fineract.interoperation.exception`)
// ============================================================================

/// Helper factory for Fineract Interoperation domain exceptions matching Java's
/// `InteropAccountNotFoundException`, `InteropTransferAlreadyOnHoldException`,
/// `InteropTransferAlreadyCommittedException`, and `InteropTransferMissingException`.
pub struct InteropDomainException;

impl InteropDomainException {
    /// Account not found for the given ID or party identifier (`InteropAccountNotFoundException`).
    pub fn account_not_found(identifier_desc: impl Into<String>) -> FineractError {
        let desc = identifier_desc.into();
        FineractError::NotFound {
            globalisation_message_code: "error.msg.interop.account.not.found".to_string(),
            default_user_message: format!("Savings account not found for {desc}"),
        }
    }

    /// Party identifier not found in `interop_identifier`.
    pub fn identifier_not_found(
        id_type: InteropIdentifierType,
        a_value: &str,
        sub_value_or_type: Option<&str>,
    ) -> FineractError {
        let detail = match sub_value_or_type {
            Some(sub) => format!("{id_type}:{a_value}:{sub}"),
            None => format!("{id_type}:{a_value}"),
        };
        FineractError::NotFound {
            globalisation_message_code: "error.msg.interop.identifier.not.found".to_string(),
            default_user_message: format!("Interop party identifier not found for {detail}"),
        }
    }

    /// Duplicate `(account_id, type)` or `(type, a_value, sub_value_or_type)` registration.
    pub fn duplicate_identifier(reason: impl Into<String>) -> FineractError {
        let msg = reason.into();
        FineractError::DomainRule(PlatformDomainRuleException::new(
            "error.msg.interop.identifier.duplicate",
            msg.clone(),
            vec![msg.into()],
        ))
    }

    /// Transfer amount is already placed on hold for `transfer_code` (`InteropTransferAlreadyOnHoldException`).
    pub fn transfer_already_on_hold(transfer_code: &str) -> FineractError {
        FineractError::DomainRule(PlatformDomainRuleException::new(
            "error.msg.interop.transfer.already.on.hold",
            format!("Transfer amount was already put on hold for transfer: {transfer_code}"),
            vec![transfer_code.to_string().into()],
        ))
    }

    /// Transfer has already been committed/settled for `transfer_code` (`InteropTransferAlreadyCommittedException`).
    pub fn transfer_already_committed(transfer_code: &str) -> FineractError {
        FineractError::DomainRule(PlatformDomainRuleException::new(
            "error.msg.interop.transfer.already.committed",
            format!("Transfer was already committed for transfer: {transfer_code}"),
            vec![transfer_code.to_string().into()],
        ))
    }

    /// Expected active hold transaction is missing or does not match `transfer_code` (`InteropTransferMissingException`).
    pub fn transfer_hold_missing(transfer_code: &str) -> FineractError {
        FineractError::DomainRule(PlatformDomainRuleException::new(
            "error.msg.interop.onhold.transfer.missing",
            format!("No active on-hold transaction found matching transfer: {transfer_code}"),
            vec![transfer_code.to_string().into()],
        ))
    }

    /// Currency mismatch between account and request payload (`validateCurrencies` in `InteropServiceImpl`).
    pub fn currency_mismatch(account_currency: &str, request_currency: &str) -> FineractError {
        FineractError::DomainRule(PlatformDomainRuleException::new(
            "error.msg.interop.currency.not.match",
            format!(
                "Account currency '{account_currency}' does not match request currency '{request_currency}'"
            ),
            vec![
                account_currency.to_string().into(),
                request_currency.to_string().into(),
            ],
        ))
    }

    /// Account status or sub-status disallows the requested debit/credit (`validateTransaction` in `InteropServiceImpl`).
    pub fn transaction_not_allowed(account_id: i64, reason: impl Into<String>) -> FineractError {
        let reason_str = reason.into();
        FineractError::DomainRule(PlatformDomainRuleException::new(
            "error.msg.interop.transaction.not.allowed",
            format!("Transaction is not allowed on savings account {account_id}: {reason_str}"),
            vec![account_id.into(), reason_str.into()],
        ))
    }

    /// Insufficient withdrawable balance for quote or transfer (`InsufficientAccountBalanceException`).
    pub fn insufficient_balance(
        transaction_amount: Decimal,
        fee_amount: Decimal,
        withdrawable_balance: Decimal,
    ) -> FineractError {
        FineractError::DomainRule(PlatformDomainRuleException::new(
            "error.msg.savingsaccount.transaction.insufficient.account.balance",
            format!(
                "Insufficient account balance: required {} (amount {} + fee {}), withdrawable {}",
                transaction_amount + fee_amount,
                transaction_amount,
                fee_amount,
                withdrawable_balance
            ),
            vec![
                transaction_amount.to_string().into(),
                fee_amount.to_string().into(),
                withdrawable_balance.to_string().into(),
            ],
        ))
    }

    /// Quote or transfer request has already passed its `expiration` timestamp.
    pub fn request_expired(code: &str, expiration: DateTime<Utc>) -> FineractError {
        FineractError::DomainRule(PlatformDomainRuleException::new(
            "error.msg.interop.request.expired",
            format!("Interoperation request '{code}' expired at {expiration}"),
            vec![code.to_string().into(), expiration.to_rfc3339().into()],
        ))
    }
}

// ============================================================================
// 5. DualDbInteropRepository (PostgreSQL & MySQL Dialect SQL + In-Memory Store)
// ============================================================================

/// Lifecycle state of an interop transfer tracked by [`DualDbInteropRepository`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteropTransferLifecycleStatus {
    Prepared,
    Committed,
    Released,
}

/// Internal tracking record for a prepared, committed, or released Mojaloop transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteropTransferRecord {
    pub transfer_code: String,
    pub transaction_code: String,
    pub account_id: i64,
    pub transaction_role: InteropTransactionRole,
    pub amount: Money,
    pub fsp_fee: Option<Money>,
    pub fsp_commission: Option<Money>,
    pub total_hold_amount: Decimal,
    pub hold_transaction_id: Option<i64>,
    pub release_transaction_id: Option<i64>,
    pub settlement_transaction_id: Option<i64>,
    pub status: InteropTransferLifecycleStatus,
    pub completed_timestamp: Option<DateTime<Utc>>,
}

/// Composite key `(InteropIdentifierType, a_value, sub_value_or_type)` for `uk_hathor_identifier_value`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdentifierValueKey {
    identifier_type: InteropIdentifierType,
    a_value: String,
    sub_value_or_type: Option<String>,
}

impl IdentifierValueKey {
    fn new(
        identifier_type: InteropIdentifierType,
        a_value: &str,
        sub_value_or_type: Option<&str>,
    ) -> Self {
        let normalized_sub = sub_value_or_type.and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        Self {
            identifier_type,
            a_value: a_value.trim().to_string(),
            sub_value_or_type: normalized_sub,
        }
    }
}

/// Dual-database repository for `interop_identifier`, savings accounts, quotes, and transfers.
///
/// Generates dialect-aware SQL (`SqlDialect::Postgres` and `SqlDialect::MySql`) via
/// [`DialectSql::rewrite_placeholders`] and maintains a thread-safe in-memory store
/// enforcing both `uk_hathor_identifier_account` `(account_id, type)` and
/// `uk_hathor_identifier_value` `(type, a_value, sub_value_or_type)`.
#[derive(Debug)]
pub struct DualDbInteropRepository {
    dialect: SqlDialect,
    next_identifier_id: AtomicI64,
    identifiers_by_id: DashMap<i64, InteropIdentifier>,
    identifiers_by_value: DashMap<IdentifierValueKey, i64>,
    identifiers_by_account_type: DashMap<(i64, InteropIdentifierType), i64>,
    savings_accounts: DashMap<i64, SavingsAccount>,
    account_lookup_by_no_or_external: DashMap<String, i64>,
    quotes: DashMap<String, InteropQuoteResponseData>,
    transfers: DashMap<String, InteropTransferRecord>,
}

impl Default for DualDbInteropRepository {
    fn default() -> Self {
        Self::new(SqlDialect::Postgres)
    }
}

impl DualDbInteropRepository {
    /// Creates a new [`DualDbInteropRepository`] configured for the given SQL dialect.
    pub fn new(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            next_identifier_id: AtomicI64::new(1),
            identifiers_by_id: DashMap::new(),
            identifiers_by_value: DashMap::new(),
            identifiers_by_account_type: DashMap::new(),
            savings_accounts: DashMap::new(),
            account_lookup_by_no_or_external: DashMap::new(),
            quotes: DashMap::new(),
            transfers: DashMap::new(),
        }
    }

    /// Returns the configured [`SqlDialect`].
    #[inline]
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    // ------------------------------------------------------------------------
    // Dialect-Aware SQL Builders (`interop_identifier`)
    // ------------------------------------------------------------------------

    /// Builds the `INSERT INTO interop_identifier` statement for `dialect`.
    pub fn insert_identifier_sql(dialect: SqlDialect) -> String {
        let base = "INSERT INTO interop_identifier \
            (account_id, type, a_value, sub_value_or_type, created_by, created_on, modified_by, modified_on) \
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)";
        DialectSql::rewrite_placeholders(base, dialect)
    }

    /// Builds the `SELECT` query to resolve an [`InteropIdentifier`] by `(type, a_value, sub_value_or_type)`.
    pub fn select_identifier_by_type_and_value_sql(
        dialect: SqlDialect,
        has_sub_value: bool,
    ) -> String {
        let base = if has_sub_value {
            "SELECT id, account_id, type, a_value, sub_value_or_type, created_by, created_on, modified_by, modified_on \
             FROM interop_identifier WHERE type = $1 AND a_value = $2 AND sub_value_or_type = $3"
        } else {
            "SELECT id, account_id, type, a_value, sub_value_or_type, created_by, created_on, modified_by, modified_on \
             FROM interop_identifier WHERE type = $1 AND a_value = $2 AND sub_value_or_type IS NULL"
        };
        DialectSql::rewrite_placeholders(base, dialect)
    }

    /// Builds the `SELECT` query to list all party identifiers registered for an `account_id`.
    pub fn select_identifiers_by_account_sql(dialect: SqlDialect) -> String {
        let base = "SELECT id, account_id, type, a_value, sub_value_or_type, created_by, created_on, modified_by, modified_on \
                    FROM interop_identifier WHERE account_id = $1 ORDER BY id ASC";
        DialectSql::rewrite_placeholders(base, dialect)
    }

    /// Builds the `DELETE FROM interop_identifier` statement for `(type, a_value, sub_value_or_type)`.
    pub fn delete_identifier_by_type_and_value_sql(
        dialect: SqlDialect,
        has_sub_value: bool,
    ) -> String {
        let base = if has_sub_value {
            "DELETE FROM interop_identifier WHERE type = $1 AND a_value = $2 AND sub_value_or_type = $3"
        } else {
            "DELETE FROM interop_identifier WHERE type = $1 AND a_value = $2 AND sub_value_or_type IS NULL"
        };
        DialectSql::rewrite_placeholders(base, dialect)
    }

    // ------------------------------------------------------------------------
    // Savings Account Store Operations
    // ------------------------------------------------------------------------

    /// Saves or updates a [`SavingsAccount`] in the repository, indexing its `id`, `account_no`, and `external_id`.
    pub fn save_savings_account(&self, mut account: SavingsAccount) -> i64 {
        let id = account.id.unwrap_or_else(|| {
            let next_id = self
                .savings_accounts
                .iter()
                .map(|entry| *entry.key())
                .max()
                .unwrap_or(0)
                + 1;
            account.id = Some(next_id);
            next_id
        });
        if !account.account_no.trim().is_empty() {
            self.account_lookup_by_no_or_external
                .insert(account.account_no.trim().to_string(), id);
        }
        if let Some(ref ext) = account.external_id {
            if !ext.trim().is_empty() {
                self.account_lookup_by_no_or_external
                    .insert(ext.trim().to_string(), id);
            }
        }
        self.savings_accounts.insert(id, account);
        id
    }

    /// Retrieves a [`SavingsAccount`] by numeric `account_id`.
    pub fn find_savings_account(&self, account_id: i64) -> Option<SavingsAccount> {
        self.savings_accounts.get(&account_id).map(|r| r.clone())
    }

    /// Resolves a numeric `account_id` from either a numeric string, `account_no`, or `external_id`.
    pub fn resolve_account_id(&self, raw_account_id: &str) -> Option<i64> {
        let trimmed = raw_account_id.trim();
        if let Some(mapped) = self.account_lookup_by_no_or_external.get(trimmed) {
            return Some(*mapped);
        }
        if let Ok(parsed) = trimmed.parse::<i64>() {
            if self.savings_accounts.contains_key(&parsed) {
                return Some(parsed);
            }
            return Some(parsed);
        }
        None
    }

    // ------------------------------------------------------------------------
    // Party Identifier Store Operations (`interop_identifier`)
    // ------------------------------------------------------------------------

    /// Inserts or updates a party identifier while enforcing unique constraints:
    /// - `(type, a_value, sub_value_or_type)` cannot belong to another identifier
    /// - `(account_id, type)` has a 1:1 constraint per identifier type per account
    pub fn insert_identifier(
        &self,
        mut identifier: InteropIdentifier,
    ) -> FineractResult<InteropIdentifier> {
        let value_key = IdentifierValueKey::new(
            identifier.identifier_type,
            &identifier.a_value,
            identifier.sub_value_or_type.as_deref(),
        );
        if let Some(existing_id) = self.identifiers_by_value.get(&value_key) {
            let existing = self.identifiers_by_id.get(&existing_id);
            let existing_acc = existing.map(|e| e.account_id).unwrap_or_default();
            return Err(InteropDomainException::duplicate_identifier(format!(
                "Identifier {}:{} is already registered to account {}",
                identifier.identifier_type, identifier.a_value, existing_acc
            )));
        }

        let account_type_key = (identifier.account_id, identifier.identifier_type);
        if let Some(existing_id) = self.identifiers_by_account_type.get(&account_type_key) {
            return Err(InteropDomainException::duplicate_identifier(format!(
                "Account {} already has an identifier of type {} (id={})",
                identifier.account_id, identifier.identifier_type, *existing_id
            )));
        }

        let id = if identifier.id > 0 {
            identifier.id
        } else {
            self.next_identifier_id.fetch_add(1, Ordering::SeqCst)
        };
        identifier.id = id;

        self.identifiers_by_value.insert(value_key, id);
        self.identifiers_by_account_type
            .insert(account_type_key, id);
        self.identifiers_by_id.insert(id, identifier.clone());

        Ok(identifier)
    }

    /// Finds an [`InteropIdentifier`] by `(identifier_type, a_value, sub_value_or_type)`.
    pub fn find_identifier(
        &self,
        identifier_type: InteropIdentifierType,
        a_value: &str,
        sub_value_or_type: Option<&str>,
    ) -> Option<InteropIdentifier> {
        let key = IdentifierValueKey::new(identifier_type, a_value, sub_value_or_type);
        let id = *self.identifiers_by_value.get(&key)?;
        self.identifiers_by_id.get(&id).map(|e| e.clone())
    }

    /// Lists all [`InteropIdentifier`] entries bound to `account_id`.
    pub fn find_identifiers_by_account(&self, account_id: i64) -> Vec<InteropIdentifier> {
        let mut list: Vec<InteropIdentifier> = self
            .identifiers_by_id
            .iter()
            .filter(|entry| entry.value().account_id == account_id)
            .map(|entry| entry.value().clone())
            .collect();
        list.sort_by_key(|i| i.id);
        list
    }

    /// Deletes an [`InteropIdentifier`] by `(identifier_type, a_value, sub_value_or_type)`.
    pub fn delete_identifier(
        &self,
        identifier_type: InteropIdentifierType,
        a_value: &str,
        sub_value_or_type: Option<&str>,
    ) -> Option<InteropIdentifier> {
        let key = IdentifierValueKey::new(identifier_type, a_value, sub_value_or_type);
        let (_, id) = self.identifiers_by_value.remove(&key)?;
        let (_, removed) = self.identifiers_by_id.remove(&id)?;
        self.identifiers_by_account_type
            .remove(&(removed.account_id, removed.identifier_type));
        Some(removed)
    }

    // ------------------------------------------------------------------------
    // Quotes & Transfers Store Operations
    // ------------------------------------------------------------------------

    pub fn save_quote(&self, quote: InteropQuoteResponseData) {
        self.quotes.insert(quote.quote_code.clone(), quote);
    }

    pub fn find_quote(&self, quote_code: &str) -> Option<InteropQuoteResponseData> {
        self.quotes.get(quote_code).map(|q| q.clone())
    }

    pub fn save_transfer(&self, record: InteropTransferRecord) {
        self.transfers.insert(record.transfer_code.clone(), record);
    }

    pub fn find_transfer(&self, transfer_code: &str) -> Option<InteropTransferRecord> {
        self.transfers.get(transfer_code).map(|t| t.clone())
    }
}

// ============================================================================
// 6. InteroperationService (`InteropServiceImpl.java`)
// ============================================================================

/// Core domain service orchestrating Mojaloop / Fineract Interoperation workflows
/// (`org.apache.fineract.interoperation.service.InteropServiceImpl`):
///
/// 1. **Party Identifier Lookup & Registration** (`interop_identifier`):
///    - [`InteroperationService::register_party_identifier`]
///    - [`InteroperationService::lookup_account_by_identifier`]
///    - [`InteroperationService::delete_party_identifier`]
/// 2. **Quotes Engine**:
///    - [`InteroperationService::calculate_quote`]
/// 3. **Two-Phase Transfers (Prepare / Commit / Release)**:
///    - [`InteroperationService::prepare_transfer`]
///    - [`InteroperationService::commit_transfer`]
///    - [`InteroperationService::release_transfer`]
#[derive(Debug, Clone)]
pub struct InteroperationService {
    repository: Arc<DualDbInteropRepository>,
}

impl Default for InteroperationService {
    fn default() -> Self {
        Self::new(Arc::new(DualDbInteropRepository::default()))
    }
}

impl InteroperationService {
    /// Creates a new [`InteroperationService`] backed by `repository`.
    pub fn new(repository: Arc<DualDbInteropRepository>) -> Self {
        Self { repository }
    }

    /// Returns a reference to the underlying [`DualDbInteropRepository`].
    #[inline]
    pub fn repository(&self) -> &Arc<DualDbInteropRepository> {
        &self.repository
    }

    /// Registers a [`SavingsAccount`] in the underlying repository for party resolution and transfers.
    pub fn register_savings_account(&self, account: SavingsAccount) -> i64 {
        self.repository.save_savings_account(account)
    }

    /// Retrieves the current state of a [`SavingsAccount`] by ID.
    pub fn get_savings_account(&self, account_id: i64) -> FineractResult<SavingsAccount> {
        self.repository
            .find_savings_account(account_id)
            .ok_or_else(|| InteropDomainException::account_not_found(format!("id={account_id}")))
    }

    // ------------------------------------------------------------------------
    // 6.1 Party Identifier Registration, Lookup, and Deletion
    // ------------------------------------------------------------------------

    /// Registers a party identifier (`MSISDN`, `IBAN`, `EMAIL`, `ACCOUNT_ID`, `ALIAS`, etc.)
    /// in `interop_identifier` linking it to an active [`SavingsAccount`].
    pub fn register_party_identifier(
        &self,
        account_id: i64,
        identifier_type: InteropIdentifierType,
        a_value: &str,
        sub_value_or_type: Option<&str>,
        created_by: Option<&str>,
    ) -> FineractResult<InteropIdentifier> {
        let mut validator = DataValidatorBuilder::new(ENTITY_NAME_IDENTIFIER);
        validator
            .reset()
            .parameter(PARAM_IDENTIFIER_VALUE)
            .value(Some(a_value))
            .not_blank()
            .not_exceeding_length_of(128);
        if let Some(sub) = sub_value_or_type {
            validator
                .reset()
                .parameter(PARAM_SUB_ID_OR_TYPE)
                .value(Some(sub))
                .not_exceeding_length_of(128);
        }
        validator.finish()?;

        // Verify the target savings account exists
        let _account = self.get_savings_account(account_id)?;

        let identifier = InteropIdentifier::with_audit(
            0,
            account_id,
            identifier_type,
            a_value,
            sub_value_or_type.map(ToOwned::to_owned),
            created_by.unwrap_or("system"),
            Utc::now(),
        );
        self.repository.insert_identifier(identifier)
    }

    /// Looks up a registered [`InteropIdentifier`] by `(identifier_type, a_value, sub_value_or_type)`.
    pub fn lookup_account_by_identifier(
        &self,
        identifier_type: InteropIdentifierType,
        a_value: &str,
        sub_value_or_type: Option<&str>,
    ) -> FineractResult<InteropIdentifier> {
        let identifier = self
            .repository
            .find_identifier(identifier_type, a_value, sub_value_or_type)
            .ok_or_else(|| {
                InteropDomainException::identifier_not_found(
                    identifier_type,
                    a_value,
                    sub_value_or_type,
                )
            })?;

        // Ensure the linked savings account still exists
        let _ = self.get_savings_account(identifier.account_id)?;
        Ok(identifier)
    }

    /// Looks up the [`InteropIdentifierAccountResponseData`] (returning the account's `external_id`
    /// or `account_no`) for a Mojaloop `GET /interoperation/participants/{idType}/{idValue}` query.
    pub fn get_account_by_identifier(
        &self,
        identifier_type: InteropIdentifierType,
        a_value: &str,
        sub_value_or_type: Option<&str>,
    ) -> FineractResult<InteropIdentifierAccountResponseData> {
        let identifier =
            self.lookup_account_by_identifier(identifier_type, a_value, sub_value_or_type)?;
        let account = self.get_savings_account(identifier.account_id)?;
        let external_or_no = account
            .external_id
            .clone()
            .unwrap_or_else(|| account.account_no.clone());
        Ok(InteropIdentifierAccountResponseData {
            resource_id: Some(identifier.id),
            office_id: account.office_id,
            account_id: external_or_no,
        })
    }

    /// Deletes a registered [`InteropIdentifier`] from `interop_identifier`.
    pub fn delete_party_identifier(
        &self,
        identifier_type: InteropIdentifierType,
        a_value: &str,
        sub_value_or_type: Option<&str>,
    ) -> FineractResult<InteropIdentifier> {
        self.repository
            .delete_identifier(identifier_type, a_value, sub_value_or_type)
            .ok_or_else(|| {
                InteropDomainException::identifier_not_found(
                    identifier_type,
                    a_value,
                    sub_value_or_type,
                )
            })
    }

    // ------------------------------------------------------------------------
    // 6.2 Quotes Engine (`createQuote` / `calculate_quote`)
    // ------------------------------------------------------------------------

    /// Calculates an interoperable quote (`InteropServiceImpl.createQuote`):
    /// - For [`InteropTransactionRole::Payer`] (debit): validates account debit eligibility, calculates
    ///   applicable withdrawal fees + request fees, and verifies `withdrawable_balance >= amount + fsp_fee - fsp_commission`.
    /// - For [`InteropTransactionRole::Payee`] (credit): validates account credit eligibility.
    pub fn calculate_quote(
        &self,
        request: &InteropQuoteRequestData,
    ) -> FineractResult<InteropQuoteResponseData> {
        Self::validate_quote_request(request)?;

        let savings_account = self.get_savings_account(request.account_id)?;
        let tx_date = request
            .transaction_date
            .unwrap_or_else(|| Utc::now().date_naive());

        Self::validate_account_for_transaction(
            &savings_account,
            request.transaction_role,
            &request.amount,
            request.fees.as_ref(),
            request.fsp_commission.as_ref(),
            tx_date,
        )?;

        let fee_money = match request.transaction_role {
            InteropTransactionRole::Payer => {
                // Sum active withdrawal fee charges on the savings account + explicit request fees
                let account_fee_decimal = Self::calculate_account_withdrawal_fee(
                    &savings_account,
                    request.amount.amount(),
                );
                let request_fee_decimal =
                    request.fees.as_ref().map(|f| f.amount()).unwrap_or(Decimal::ZERO);
                let total_fee_decimal = account_fee_decimal + request_fee_decimal;

                let commission_decimal = request
                    .fsp_commission
                    .as_ref()
                    .map(|c| c.amount())
                    .unwrap_or(Decimal::ZERO);

                let net_additional = (total_fee_decimal - commission_decimal).max(Decimal::ZERO);
                let required_balance = request.amount.amount() + net_additional;
                let withdrawable = savings_account.withdrawable_balance();
                if withdrawable < required_balance {
                    return Err(InteropDomainException::insufficient_balance(
                        request.amount.amount(),
                        net_additional,
                        withdrawable,
                    ));
                }

                Money::of(
                    savings_account.currency.clone(),
                    total_fee_decimal,
                    FineractRoundingMode::HalfEven,
                )
            }
            InteropTransactionRole::Payee => {
                let request_fee_decimal =
                    request.fees.as_ref().map(|f| f.amount()).unwrap_or(Decimal::ZERO);
                Money::of(
                    savings_account.currency.clone(),
                    request_fee_decimal,
                    FineractRoundingMode::HalfEven,
                )
            }
        };

        let response = InteropQuoteResponseData {
            office_id: savings_account.office_id,
            client_id: savings_account.client_id,
            savings_id: savings_account.id,
            resource_id: savings_account.id,
            transaction_code: request.transaction_code.clone(),
            quote_code: request.quote_code.clone(),
            amount: request.amount.clone(),
            transaction_role: request.transaction_role,
            fsp_fee: Some(fee_money),
            fsp_commission: request.fsp_commission.clone(),
            expiration: request.expiration,
            state: InteropActionState::Accepted,
            extension_list: request.extension_list.clone(),
        };

        self.repository.save_quote(response.clone());
        Ok(response)
    }

    // ------------------------------------------------------------------------
    // 6.3 Transfer Prepare (`prepareTransfer` / `prepare_transfer`)
    // ------------------------------------------------------------------------

    /// Prepares an interoperable transfer (`InteropServiceImpl.prepareTransfer`):
    /// - For [`InteropTransactionRole::Payer`]: places an `AmountHold` on the Payer's savings account
    ///   for `amount + fsp_fee - fsp_commission`, tags the hold transaction with `ref_no = transfer_code`,
    ///   and records the prepared transfer in the repository.
    /// - For [`InteropTransactionRole::Payee`]: validates credit eligibility and records the prepared transfer.
    pub fn prepare_transfer(
        &self,
        request: &InteropTransferRequestData,
    ) -> FineractResult<InteropTransferResponseData> {
        Self::validate_transfer_request(request)?;

        let mut savings_account = self.get_savings_account(request.account_id)?;
        let tx_date = request
            .transaction_date
            .unwrap_or_else(|| Utc::now().date_naive());

        Self::validate_account_for_transaction(
            &savings_account,
            request.transaction_role,
            &request.amount,
            request.fsp_fee.as_ref(),
            request.fsp_commission.as_ref(),
            tx_date,
        )?;

        // Check if transfer_code was already prepared or committed
        if let Some(existing) = self.repository.find_transfer(&request.transfer_code) {
            match existing.status {
                InteropTransferLifecycleStatus::Prepared => {
                    return Err(InteropDomainException::transfer_already_on_hold(
                        &request.transfer_code,
                    ));
                }
                InteropTransferLifecycleStatus::Committed => {
                    return Err(InteropDomainException::transfer_already_committed(
                        &request.transfer_code,
                    ));
                }
                InteropTransferLifecycleStatus::Released => {}
            }
        }

        if Self::has_active_hold_for_transfer(&savings_account, &request.transfer_code) {
            return Err(InteropDomainException::transfer_already_on_hold(
                &request.transfer_code,
            ));
        }
        if Self::has_committed_tx_for_transfer(
            &savings_account,
            &request.transfer_code,
            request.transaction_role.transaction_type(),
        ) {
            return Err(InteropDomainException::transfer_already_committed(
                &request.transfer_code,
            ));
        }

        let total_transfer_amount = Self::calculate_total_transfer_amount(request);
        let mut hold_tx_id = None;

        if request.transaction_role.is_withdraw() && total_transfer_amount > Decimal::ZERO {
            if savings_account.withdrawable_balance() < total_transfer_amount {
                let fee_part =
                    (total_transfer_amount - request.amount.amount()).max(Decimal::ZERO);
                return Err(InteropDomainException::insufficient_balance(
                    request.amount.amount(),
                    fee_part,
                    savings_account.withdrawable_balance(),
                ));
            }

            let hold_tx = savings_account.hold_amount(
                tx_date,
                total_transfer_amount,
                None,
                tx_date,
                false,
                Some(format!(
                    "{DEFAULT_ROUTING_CODE}:{}",
                    request.transfer_code
                )),
            )?;
            let allocated_id = hold_tx.id.unwrap_or(1);
            hold_tx_id = Some(allocated_id);

            if let Some(tx_mut) = savings_account
                .transactions
                .iter_mut()
                .find(|t| t.id == Some(allocated_id))
            {
                tx_mut.ref_no = Some(request.transfer_code.clone());
            }

            self.repository.save_savings_account(savings_account.clone());
        }

        let record = InteropTransferRecord {
            transfer_code: request.transfer_code.clone(),
            transaction_code: request.transaction_code.clone(),
            account_id: request.account_id,
            transaction_role: request.transaction_role,
            amount: request.amount.clone(),
            fsp_fee: request.fsp_fee.clone(),
            fsp_commission: request.fsp_commission.clone(),
            total_hold_amount: total_transfer_amount,
            hold_transaction_id: hold_tx_id,
            release_transaction_id: None,
            settlement_transaction_id: None,
            status: InteropTransferLifecycleStatus::Prepared,
            completed_timestamp: None,
        };
        self.repository.save_transfer(record);

        Ok(InteropTransferResponseData {
            office_id: savings_account.office_id,
            client_id: savings_account.client_id,
            savings_id: savings_account.id,
            resource_id: hold_tx_id.or(savings_account.id),
            transaction_code: request.transaction_code.clone(),
            transfer_code: request.transfer_code.clone(),
            account_id: request.account_id,
            amount: request.amount.clone(),
            transaction_role: request.transaction_role,
            fsp_fee: request.fsp_fee.clone(),
            fsp_commission: request.fsp_commission.clone(),
            expiration: request.expiration,
            state: InteropActionState::Accepted,
            completed_timestamp: None,
            extension_list: request.extension_list.clone(),
            hold_transaction_id: hold_tx_id,
            release_transaction_id: None,
            settlement_transaction_id: None,
        })
    }

    // ------------------------------------------------------------------------
    // 6.4 Transfer Commit (`commitTransfer` / `commit_transfer`)
    // ------------------------------------------------------------------------

    /// Commits (settles) an interoperable transfer (`InteropServiceImpl.commitTransfer`):
    /// - For [`InteropTransactionRole::Payer`]: verifies the active `AmountHold` transaction for
    ///   `transfer_code` matches `amount + fsp_fee - fsp_commission`, releases the hold via
    ///   [`SavingsAccount::release_amount`], and executes the debit via [`SavingsAccount::withdraw`].
    /// - For [`InteropTransactionRole::Payee`]: executes the credit via [`SavingsAccount::deposit`].
    pub fn commit_transfer(
        &self,
        request: &InteropTransferRequestData,
    ) -> FineractResult<InteropTransferResponseData> {
        Self::validate_transfer_request(request)?;

        let is_debit = request.transaction_role.is_withdraw();
        let mut savings_account = self.get_savings_account(request.account_id)?;
        let tx_date = request
            .transaction_date
            .unwrap_or_else(|| Utc::now().date_naive());

        Self::validate_account_for_transaction(
            &savings_account,
            request.transaction_role,
            &request.amount,
            request.fsp_fee.as_ref(),
            request.fsp_commission.as_ref(),
            tx_date,
        )?;

        if let Some(existing) = self.repository.find_transfer(&request.transfer_code) {
            if existing.status == InteropTransferLifecycleStatus::Committed {
                return Err(InteropDomainException::transfer_already_committed(
                    &request.transfer_code,
                ));
            }
        }

        let expected_tx_type = if is_debit {
            SavingsAccountTransactionType::Withdrawal
        } else {
            SavingsAccountTransactionType::Deposit
        };
        if Self::has_committed_tx_for_transfer(
            &savings_account,
            &request.transfer_code,
            expected_tx_type,
        ) {
            return Err(InteropDomainException::transfer_already_committed(
                &request.transfer_code,
            ));
        }

        let total_transfer_amount = Self::calculate_total_transfer_amount(request);
        let mut hold_tx_id = None;
        let mut release_tx_id = None;
        let settlement_tx_id: Option<i64>;

        if is_debit {
            let hold_tx = Self::find_active_hold_for_transfer(
                &savings_account,
                &request.transfer_code,
            )
            .ok_or_else(|| {
                InteropDomainException::transfer_hold_missing(&request.transfer_code)
            })?;

            if hold_tx.amount != total_transfer_amount {
                return Err(InteropDomainException::transfer_hold_missing(
                    &request.transfer_code,
                ));
            }

            if savings_account.withdrawable_balance() + hold_tx.amount < total_transfer_amount {
                let fee_part =
                    (total_transfer_amount - request.amount.amount()).max(Decimal::ZERO);
                return Err(InteropDomainException::insufficient_balance(
                    request.amount.amount(),
                    fee_part,
                    savings_account.withdrawable_balance(),
                ));
            }

            let found_hold_id = hold_tx.id.ok_or_else(|| {
                InteropDomainException::transfer_hold_missing(&request.transfer_code)
            })?;
            hold_tx_id = Some(found_hold_id);

            let release_date = tx_date.max(hold_tx.date_of);
            let release_tx =
                savings_account.release_amount(found_hold_id, release_date, release_date)?;
            let rel_id = release_tx.id.unwrap_or(0);
            release_tx_id = Some(rel_id);
            if let Some(tx_mut) = savings_account
                .transactions
                .iter_mut()
                .find(|t| t.id == Some(rel_id))
            {
                tx_mut.ref_no = Some(request.transfer_code.clone());
            }

            // Determine whether the account already has an active WithdrawalFee charge matching fsp_fee
            // so we never double-charge when `SavingsAccount::withdraw` runs.
            let account_withdrawal_fee = Self::calculate_account_withdrawal_fee(
                &savings_account,
                request.amount.amount(),
            );
            let request_fee = request
                .fsp_fee
                .as_ref()
                .map(|f| f.amount())
                .unwrap_or(Decimal::ZERO);
            let request_commission = request
                .fsp_commission
                .as_ref()
                .map(|c| c.amount())
                .unwrap_or(Decimal::ZERO);

            let withdrawal_tx = if account_withdrawal_fee > Decimal::ZERO
                && account_withdrawal_fee == request_fee
                && request_commission == Decimal::ZERO
            {
                // `withdraw(..., is_account_transfer = false)` will debit `request.amount` AND
                // automatically post the `WithdrawalFee` transaction for `account_withdrawal_fee`.
                savings_account.withdraw(
                    release_date,
                    request.amount.amount(),
                    None,
                    release_date,
                    false,
                )?
            } else {
                // Pass `is_account_transfer = true` (when `withdrawal_fee_for_transfer == false`)
                // so `total_transfer_amount` (`amount + fsp_fee - fsp_commission`) is debited cleanly.
                savings_account.withdraw(
                    release_date,
                    total_transfer_amount,
                    None,
                    release_date,
                    true,
                )?
            };

            let w_id = withdrawal_tx.id.unwrap_or(0);
            settlement_tx_id = Some(w_id);
            if let Some(tx_mut) = savings_account
                .transactions
                .iter_mut()
                .find(|t| t.id == Some(w_id))
            {
                tx_mut.ref_no = Some(request.transfer_code.clone());
            }
        } else {
            // Payee credit: net credit = amount - fsp_fee + fsp_commission
            let fee = request
                .fsp_fee
                .as_ref()
                .map(|f| f.amount())
                .unwrap_or(Decimal::ZERO);
            let commission = request
                .fsp_commission
                .as_ref()
                .map(|c| c.amount())
                .unwrap_or(Decimal::ZERO);
            let credit_amount =
                (request.amount.amount() - fee + commission).max(Decimal::ZERO);

            let deposit_tx =
                savings_account.deposit(tx_date, credit_amount, None, tx_date)?;
            let d_id = deposit_tx.id.unwrap_or(0);
            settlement_tx_id = Some(d_id);
            if let Some(tx_mut) = savings_account
                .transactions
                .iter_mut()
                .find(|t| t.id == Some(d_id))
            {
                tx_mut.ref_no = Some(request.transfer_code.clone());
            }
        }

        self.repository.save_savings_account(savings_account.clone());

        let completed_at = Utc::now();
        let record = InteropTransferRecord {
            transfer_code: request.transfer_code.clone(),
            transaction_code: request.transaction_code.clone(),
            account_id: request.account_id,
            transaction_role: request.transaction_role,
            amount: request.amount.clone(),
            fsp_fee: request.fsp_fee.clone(),
            fsp_commission: request.fsp_commission.clone(),
            total_hold_amount: total_transfer_amount,
            hold_transaction_id: hold_tx_id,
            release_transaction_id: release_tx_id,
            settlement_transaction_id: settlement_tx_id,
            status: InteropTransferLifecycleStatus::Committed,
            completed_timestamp: Some(completed_at),
        };
        self.repository.save_transfer(record);

        Ok(InteropTransferResponseData {
            office_id: savings_account.office_id,
            client_id: savings_account.client_id,
            savings_id: savings_account.id,
            resource_id: settlement_tx_id.or(savings_account.id),
            transaction_code: request.transaction_code.clone(),
            transfer_code: request.transfer_code.clone(),
            account_id: request.account_id,
            amount: request.amount.clone(),
            transaction_role: request.transaction_role,
            fsp_fee: request.fsp_fee.clone(),
            fsp_commission: request.fsp_commission.clone(),
            expiration: request.expiration,
            state: InteropActionState::Accepted,
            completed_timestamp: Some(completed_at),
            extension_list: request.extension_list.clone(),
            hold_transaction_id: hold_tx_id,
            release_transaction_id: release_tx_id,
            settlement_transaction_id: settlement_tx_id,
        })
    }

    // ------------------------------------------------------------------------
    // 6.5 Transfer Release (`releaseTransfer` / `release_transfer`)
    // ------------------------------------------------------------------------

    /// Releases an active hold on the Payer's savings account when a Mojaloop transfer is
    /// aborted, rejected, or timed out (`InteropServiceImpl.releaseTransfer`).
    pub fn release_transfer(
        &self,
        request: &InteropTransferRequestData,
    ) -> FineractResult<InteropTransferResponseData> {
        Self::validate_transfer_request(request)?;

        let mut savings_account = self.get_savings_account(request.account_id)?;
        let tx_date = request
            .transaction_date
            .unwrap_or_else(|| Utc::now().date_naive());

        let hold_tx = Self::find_active_hold_for_transfer(
            &savings_account,
            &request.transfer_code,
        )
        .ok_or_else(|| InteropDomainException::transfer_hold_missing(&request.transfer_code))?;

        let hold_tx_id = hold_tx
            .id
            .ok_or_else(|| InteropDomainException::transfer_hold_missing(&request.transfer_code))?;
        let held_amount = hold_tx.amount;
        let release_date = tx_date.max(hold_tx.date_of);

        let release_tx =
            savings_account.release_amount(hold_tx_id, release_date, release_date)?;
        let release_tx_id = release_tx.id.unwrap_or(0);
        if let Some(tx_mut) = savings_account
            .transactions
            .iter_mut()
            .find(|t| t.id == Some(release_tx_id))
        {
            tx_mut.ref_no = Some(request.transfer_code.clone());
        }

        self.repository.save_savings_account(savings_account.clone());

        let completed_at = Utc::now();
        let record = InteropTransferRecord {
            transfer_code: request.transfer_code.clone(),
            transaction_code: request.transaction_code.clone(),
            account_id: request.account_id,
            transaction_role: request.transaction_role,
            amount: request.amount.clone(),
            fsp_fee: request.fsp_fee.clone(),
            fsp_commission: request.fsp_commission.clone(),
            total_hold_amount: held_amount,
            hold_transaction_id: Some(hold_tx_id),
            release_transaction_id: Some(release_tx_id),
            settlement_transaction_id: None,
            status: InteropTransferLifecycleStatus::Released,
            completed_timestamp: Some(completed_at),
        };
        self.repository.save_transfer(record);

        Ok(InteropTransferResponseData {
            office_id: savings_account.office_id,
            client_id: savings_account.client_id,
            savings_id: savings_account.id,
            resource_id: Some(release_tx_id),
            transaction_code: request.transaction_code.clone(),
            transfer_code: request.transfer_code.clone(),
            account_id: request.account_id,
            amount: request.amount.clone(),
            transaction_role: request.transaction_role,
            fsp_fee: request.fsp_fee.clone(),
            fsp_commission: request.fsp_commission.clone(),
            expiration: request.expiration,
            state: InteropActionState::Accepted,
            completed_timestamp: Some(completed_at),
            extension_list: request.extension_list.clone(),
            hold_transaction_id: Some(hold_tx_id),
            release_transaction_id: Some(release_tx_id),
            settlement_transaction_id: None,
        })
    }

    // ------------------------------------------------------------------------
    // Internal Validation & Calculation Helpers
    // ------------------------------------------------------------------------

    fn validate_quote_request(request: &InteropQuoteRequestData) -> FineractResult<()> {
        let mut validator = DataValidatorBuilder::new(ENTITY_NAME_QUOTE);
        validator
            .reset()
            .parameter(PARAM_TRANSACTION_CODE)
            .value(Some(request.transaction_code.as_str()))
            .not_blank();
        validator
            .reset()
            .parameter(PARAM_QUOTE_CODE)
            .value(Some(request.quote_code.as_str()))
            .not_blank();
        validator
            .reset()
            .parameter(PARAM_AMOUNT)
            .value(Some(request.amount.amount()))
            .positive_amount();
        validator.finish()?;

        if let Some(exp) = request.expiration {
            if exp < Utc::now() {
                return Err(InteropDomainException::request_expired(
                    &request.quote_code,
                    exp,
                ));
            }
        }
        Ok(())
    }

    fn validate_transfer_request(request: &InteropTransferRequestData) -> FineractResult<()> {
        let mut validator = DataValidatorBuilder::new(ENTITY_NAME_TRANSFER);
        validator
            .reset()
            .parameter(PARAM_TRANSACTION_CODE)
            .value(Some(request.transaction_code.as_str()))
            .not_blank();
        validator
            .reset()
            .parameter(PARAM_TRANSFER_CODE)
            .value(Some(request.transfer_code.as_str()))
            .not_blank();
        validator
            .reset()
            .parameter(PARAM_AMOUNT)
            .value(Some(request.amount.amount()))
            .positive_amount();
        if let Some(ref fee) = request.fsp_fee {
            validator
                .reset()
                .parameter(PARAM_FSP_FEE)
                .value(Some(fee.amount()))
                .zero_or_positive_amount();
        }
        if let Some(ref commission) = request.fsp_commission {
            validator
                .reset()
                .parameter(PARAM_FSP_COMMISSION)
                .value(Some(commission.amount()))
                .zero_or_positive_amount();
        }
        validator.finish()?;

        if let Some(exp) = request.expiration {
            if exp < Utc::now() {
                return Err(InteropDomainException::request_expired(
                    &request.transfer_code,
                    exp,
                ));
            }
        }
        Ok(())
    }

    fn validate_account_for_transaction(
        savings_account: &SavingsAccount,
        role: InteropTransactionRole,
        amount: &Money,
        fee: Option<&Money>,
        commission: Option<&Money>,
        tx_date: NaiveDate,
    ) -> FineractResult<()> {
        let account_id = savings_account.id.unwrap_or(0);

        if !savings_account.is_active() {
            return Err(InteropDomainException::transaction_not_allowed(
                account_id,
                "Savings account is not active",
            ));
        }

        if savings_account.sub_status.is_sub_status_account_blocked() {
            return Err(InteropDomainException::transaction_not_allowed(
                account_id,
                "Savings account is blocked",
            ));
        }

        if !savings_account
            .currency
            .code
            .eq_ignore_ascii_case(amount.currency_code())
        {
            return Err(InteropDomainException::currency_mismatch(
                &savings_account.currency.code,
                amount.currency_code(),
            ));
        }

        if let Some(f) = fee {
            if !savings_account
                .currency
                .code
                .eq_ignore_ascii_case(f.currency_code())
            {
                return Err(InteropDomainException::currency_mismatch(
                    &savings_account.currency.code,
                    f.currency_code(),
                ));
            }
        }

        if let Some(c) = commission {
            if !savings_account
                .currency
                .code
                .eq_ignore_ascii_case(c.currency_code())
            {
                return Err(InteropDomainException::currency_mismatch(
                    &savings_account.currency.code,
                    c.currency_code(),
                ));
            }
        }

        match role {
            InteropTransactionRole::Payer => {
                if savings_account.sub_status.is_sub_status_debit_blocked() {
                    return Err(InteropDomainException::transaction_not_allowed(
                        account_id,
                        "Savings account is blocked for debit transactions",
                    ));
                }
                if let Some(locked_until) = savings_account.lockedin_until_date_derived {
                    if tx_date <= locked_until {
                        return Err(InteropDomainException::transaction_not_allowed(
                            account_id,
                            format!("Savings account is locked in until {locked_until}"),
                        ));
                    }
                }
            }
            InteropTransactionRole::Payee => {
                if savings_account.sub_status.is_sub_status_credit_blocked() {
                    return Err(InteropDomainException::transaction_not_allowed(
                        account_id,
                        "Savings account is blocked for credit transactions",
                    ));
                }
            }
        }

        Ok(())
    }

    /// Calculates `amount + fsp_fee - fsp_commission` clamped to zero (`InteropServiceImpl.calculateTotalTransferAmount`).
    pub fn calculate_total_transfer_amount(request: &InteropTransferRequestData) -> Decimal {
        let mut total = request.amount.amount();
        if let Some(ref fee) = request.fsp_fee {
            total += fee.amount();
        }
        if let Some(ref commission) = request.fsp_commission {
            total = (total - commission.amount()).max(Decimal::ZERO);
        }
        total
    }

    fn calculate_account_withdrawal_fee(
        savings_account: &SavingsAccount,
        transaction_amount: Decimal,
    ) -> Decimal {
        savings_account
            .charges
            .iter()
            .filter(|c| c.is_active() && c.is_withdrawal_fee())
            .map(|c| c.calculate_fee_for_transaction_amount(transaction_amount).amount())
            .sum()
    }

    fn find_active_hold_for_transfer<'a>(
        savings_account: &'a SavingsAccount,
        transfer_code: &str,
    ) -> Option<&'a SavingsAccountTransaction> {
        savings_account.transactions.iter().find(|tx| {
            tx.is_amount_on_hold_not_released()
                && tx.ref_no.as_deref() == Some(transfer_code)
        })
    }

    fn has_active_hold_for_transfer(
        savings_account: &SavingsAccount,
        transfer_code: &str,
    ) -> bool {
        Self::find_active_hold_for_transfer(savings_account, transfer_code).is_some()
    }

    fn has_committed_tx_for_transfer(
        savings_account: &SavingsAccount,
        transfer_code: &str,
        tx_type: SavingsAccountTransactionType,
    ) -> bool {
        savings_account.transactions.iter().any(|tx| {
            tx.is_not_reversed()
                && tx.transaction_type == tx_type
                && tx.ref_no.as_deref() == Some(transfer_code)
        })
    }
}

// ============================================================================
// 7. CQRS Command Handlers (`org.apache.fineract.interoperation.handler`)
// ============================================================================

fn parse_money_from_command(
    command: &JsonCommand,
    param_name: &str,
    fallback_currency: &MonetaryCurrency,
) -> FineractResult<Option<Money>> {
    let Some(elem) = command.json_element(param_name) else {
        return Ok(None);
    };
    if elem.is_null() {
        return Ok(None);
    }

    if let Some(obj) = elem.as_object() {
        let Some(raw_amt) = obj.get(PARAM_AMOUNT) else {
            return Ok(None);
        };
        let amount = match raw_amt {
            serde_json::Value::Number(n) => Decimal::from_str(&n.to_string()).map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.interop.amount.invalid",
                    format!("Invalid decimal amount in '{param_name}': {e}"),
                    vec![],
                ))
            })?,
            serde_json::Value::String(s) => Decimal::from_str(s.trim()).map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.interop.amount.invalid",
                    format!("Invalid decimal amount in '{param_name}': {e}"),
                    vec![],
                ))
            })?,
            _ => return Ok(None),
        };

        let currency_code = obj
            .get(PARAM_CURRENCY)
            .and_then(|v| v.as_str())
            .unwrap_or(&fallback_currency.code);
        let currency = if currency_code.eq_ignore_ascii_case(&fallback_currency.code) {
            fallback_currency.clone()
        } else {
            MonetaryCurrency::new(currency_code, fallback_currency.digits_after_decimal, None)
        };
        return Ok(Some(Money::of(
            currency,
            amount,
            FineractRoundingMode::HalfEven,
        )));
    }

    if let Some(amount) = command.big_decimal_value_of_parameter_named(param_name)? {
        let currency_code = command
            .string_value_of_parameter_named(PARAM_CURRENCY)
            .unwrap_or_else(|| fallback_currency.code.clone());
        let currency = if currency_code.eq_ignore_ascii_case(&fallback_currency.code) {
            fallback_currency.clone()
        } else {
            MonetaryCurrency::new(currency_code, fallback_currency.digits_after_decimal, None)
        };
        return Ok(Some(Money::of(
            currency,
            amount,
            FineractRoundingMode::HalfEven,
        )));
    }

    Ok(None)
}

fn resolve_command_account_id(
    service: &InteroperationService,
    command: &JsonCommand,
) -> FineractResult<i64> {
    if let Some(id) = command.long_value_of_parameter_named(PARAM_ACCOUNT_ID)? {
        return Ok(id);
    }
    if let Some(raw) = command.string_value_of_parameter_named(PARAM_ACCOUNT_ID) {
        if let Some(resolved) = service.repository().resolve_account_id(&raw) {
            return Ok(resolved);
        }
    }
    if let Some(entity_id) = command.entity_id {
        return Ok(entity_id);
    }
    Err(InteropDomainException::account_not_found(
        "missing accountId in command",
    ))
}

fn parse_transfer_request_from_command(
    service: &InteroperationService,
    command: &JsonCommand,
) -> FineractResult<InteropTransferRequestData> {
    let account_id = resolve_command_account_id(service, command)?;
    let account = service.get_savings_account(account_id)?;

    let transaction_code = command
        .string_value_of_parameter_named(PARAM_TRANSACTION_CODE)
        .unwrap_or_default();
    let transfer_code = command
        .string_value_of_parameter_named(PARAM_TRANSFER_CODE)
        .or_else(|| command.resource_external_id.as_ref().map(|e| e.as_str().to_string()))
        .unwrap_or_default();
    let role_str = command
        .string_value_of_parameter_named(PARAM_TRANSACTION_ROLE)
        .unwrap_or_else(|| "PAYER".to_string());
    let transaction_role = InteropTransactionRole::from_str(&role_str)?;

    let amount = parse_money_from_command(command, PARAM_AMOUNT, &account.currency)?
        .ok_or_else(|| {
            FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.interop.amount.required",
                "Parameter 'amount' is required for interop transfer",
                vec![],
            ))
        })?;
    let fsp_fee = parse_money_from_command(command, PARAM_FSP_FEE, &account.currency)?;
    let fsp_commission =
        parse_money_from_command(command, PARAM_FSP_COMMISSION, &account.currency)?;

    Ok(InteropTransferRequestData {
        transaction_code,
        request_code: command.string_value_of_parameter_named(PARAM_REQUEST_CODE),
        transfer_code,
        account_id,
        amount,
        transaction_role,
        transaction_type: None,
        fsp_fee,
        fsp_commission,
        note: command.string_value_of_parameter_named(PARAM_NOTE),
        geo_code: None,
        expiration: None,
        extension_list: Vec::new(),
        transaction_date: None,
    })
}

/// Command handler for registering a party identifier (`CreateInteropIdentifierHandler.java`).
#[derive(Debug, Clone)]
pub struct RegisterInteropIdentifierCommandHandler {
    service: Arc<InteroperationService>,
}

impl RegisterInteropIdentifierCommandHandler {
    pub fn new(service: Arc<InteroperationService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for RegisterInteropIdentifierCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let account_id = resolve_command_account_id(&self.service, command)?;
        let id_type_str = command
            .string_value_of_parameter_named(PARAM_IDENTIFIER_TYPE)
            .unwrap_or_else(|| "MSISDN".to_string());
        let id_type = InteropIdentifierType::from_str(&id_type_str)?;
        let id_value = command
            .string_value_of_parameter_named(PARAM_IDENTIFIER_VALUE)
            .or_else(|| command.resource_external_id.as_ref().map(|e| e.as_str().to_string()))
            .unwrap_or_default();
        let sub_id_or_type = command.string_value_of_parameter_named(PARAM_SUB_ID_OR_TYPE);

        let created_by = ctx.user_id.map(|u| u.to_string());
        let registered = self.service.register_party_identifier(
            account_id,
            id_type,
            &id_value,
            sub_id_or_type.as_deref(),
            created_by.as_deref(),
        )?;

        let account = self.service.get_savings_account(account_id)?;
        let external_or_no = account
            .external_id
            .unwrap_or_else(|| account.account_no.clone());

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(registered.id)
            .with_office_id_opt(account.office_id)
            .with_client_id_opt(account.client_id)
            .with_savings_id(account_id)
            .with_resource_external_id(ExternalId::new(registered.a_value.clone()))
            .with_change("accountId", json!(external_or_no))
            .with_change("idType", json!(registered.identifier_type.as_str()))
            .with_change("idValue", json!(registered.a_value))
            .build())
    }
}

/// Alias matching Fineract Java class name `CreateInteropIdentifierHandler`.
pub type CreateInteropIdentifierHandler = RegisterInteropIdentifierCommandHandler;

/// Command handler for deleting a party identifier (`DeleteInteropIdentifierHandler.java`).
#[derive(Debug, Clone)]
pub struct DeleteInteropIdentifierCommandHandler {
    service: Arc<InteroperationService>,
}

impl DeleteInteropIdentifierCommandHandler {
    pub fn new(service: Arc<InteroperationService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for DeleteInteropIdentifierCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let id_type_str = command
            .string_value_of_parameter_named(PARAM_IDENTIFIER_TYPE)
            .unwrap_or_else(|| "MSISDN".to_string());
        let id_type = InteropIdentifierType::from_str(&id_type_str)?;
        let id_value = command
            .string_value_of_parameter_named(PARAM_IDENTIFIER_VALUE)
            .or_else(|| command.resource_external_id.as_ref().map(|e| e.as_str().to_string()))
            .unwrap_or_default();
        let sub_id_or_type = command.string_value_of_parameter_named(PARAM_SUB_ID_OR_TYPE);

        let deleted = self.service.delete_party_identifier(
            id_type,
            &id_value,
            sub_id_or_type.as_deref(),
        )?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(deleted.id)
            .with_savings_id(deleted.account_id)
            .with_resource_external_id(ExternalId::new(deleted.a_value.clone()))
            .with_change("accountId", json!(deleted.account_id))
            .with_change("idType", json!(deleted.identifier_type.as_str()))
            .with_change("idValue", json!(deleted.a_value))
            .build())
    }
}

/// Alias matching Fineract Java class name `DeleteInteropIdentifierHandler`.
pub type DeleteInteropIdentifierHandler = DeleteInteropIdentifierCommandHandler;

/// Command handler for calculating an interoperable quote (`CreateInteropQuoteHandler.java`).
#[derive(Debug, Clone)]
pub struct CreateInteropQuoteCommandHandler {
    service: Arc<InteroperationService>,
}

impl CreateInteropQuoteCommandHandler {
    pub fn new(service: Arc<InteroperationService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for CreateInteropQuoteCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let account_id = resolve_command_account_id(&self.service, command)?;
        let account = self.service.get_savings_account(account_id)?;

        let transaction_code = command
            .string_value_of_parameter_named(PARAM_TRANSACTION_CODE)
            .unwrap_or_default();
        let quote_code = command
            .string_value_of_parameter_named(PARAM_QUOTE_CODE)
            .unwrap_or_default();
        let role_str = command
            .string_value_of_parameter_named(PARAM_TRANSACTION_ROLE)
            .unwrap_or_else(|| "PAYER".to_string());
        let transaction_role = InteropTransactionRole::from_str(&role_str)?;

        let amount = parse_money_from_command(command, PARAM_AMOUNT, &account.currency)?
            .ok_or_else(|| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.interop.amount.required",
                    "Parameter 'amount' is required for interop quote",
                    vec![],
                ))
            })?;
        let fees = parse_money_from_command(command, PARAM_FEES, &account.currency)?;
        let fsp_commission =
            parse_money_from_command(command, PARAM_FSP_COMMISSION, &account.currency)?;

        let mut req = InteropQuoteRequestData::new(
            transaction_code,
            quote_code,
            account_id,
            amount,
            transaction_role,
        );
        req.fees = fees;
        req.fsp_commission = fsp_commission;

        let response = self.service.calculate_quote(&req)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id_opt(response.resource_id)
            .with_office_id_opt(response.office_id)
            .with_client_id_opt(response.client_id)
            .with_savings_id_opt(response.savings_id)
            .with_resource_external_id(ExternalId::new(response.quote_code.clone()))
            .with_change("quoteCode", json!(response.quote_code))
            .with_change("transactionCode", json!(response.transaction_code))
            .with_change("state", json!(response.state.as_str()))
            .with_change(
                "fspFee",
                json!(response.fsp_fee.as_ref().map(|m| m.amount().to_string())),
            )
            .build())
    }
}

/// Alias matching Fineract Java class name `CreateInteropQuoteHandler`.
pub type CreateInteropQuoteHandler = CreateInteropQuoteCommandHandler;

/// Command handler for preparing an interoperable transfer (`PrepareInteropTransferHandler.java`).
#[derive(Debug, Clone)]
pub struct PrepareInteropTransferCommandHandler {
    service: Arc<InteroperationService>,
}

impl PrepareInteropTransferCommandHandler {
    pub fn new(service: Arc<InteroperationService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for PrepareInteropTransferCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let req = parse_transfer_request_from_command(&self.service, command)?;
        let response = self.service.prepare_transfer(&req)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id_opt(response.resource_id)
            .with_office_id_opt(response.office_id)
            .with_client_id_opt(response.client_id)
            .with_savings_id_opt(response.savings_id)
            .with_resource_external_id(ExternalId::new(response.transfer_code.clone()))
            .with_transaction_id_opt(response.hold_transaction_id.map(|id| id.to_string()))
            .with_change("transferCode", json!(response.transfer_code))
            .with_change("transactionCode", json!(response.transaction_code))
            .with_change("state", json!(response.state.as_str()))
            .build())
    }
}

/// Alias matching Fineract Java class name `PrepareInteropTransferHandler`.
pub type PrepareInteropTransferHandler = PrepareInteropTransferCommandHandler;

/// Command handler for committing/settling an interoperable transfer (`CommitInteropTransferHandler.java`).
#[derive(Debug, Clone)]
pub struct CommitInteropTransferCommandHandler {
    service: Arc<InteroperationService>,
}

impl CommitInteropTransferCommandHandler {
    pub fn new(service: Arc<InteroperationService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for CommitInteropTransferCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let req = parse_transfer_request_from_command(&self.service, command)?;
        let response = self.service.commit_transfer(&req)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id_opt(response.resource_id)
            .with_office_id_opt(response.office_id)
            .with_client_id_opt(response.client_id)
            .with_savings_id_opt(response.savings_id)
            .with_resource_external_id(ExternalId::new(response.transfer_code.clone()))
            .with_transaction_id_opt(response.settlement_transaction_id.map(|id| id.to_string()))
            .with_change("transferCode", json!(response.transfer_code))
            .with_change("transactionCode", json!(response.transaction_code))
            .with_change("state", json!(response.state.as_str()))
            .with_change(
                "completedTimestamp",
                json!(response.completed_timestamp.map(|ts| ts.to_rfc3339())),
            )
            .build())
    }
}

/// Alias matching Fineract Java class name `CommitInteropTransferHandler`.
pub type CommitInteropTransferHandler = CommitInteropTransferCommandHandler;

/// Command handler for releasing an interoperable transfer hold (`ReleaseInteropTransferHandler.java`).
#[derive(Debug, Clone)]
pub struct ReleaseInteropTransferCommandHandler {
    service: Arc<InteroperationService>,
}

impl ReleaseInteropTransferCommandHandler {
    pub fn new(service: Arc<InteroperationService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl CommandHandler for ReleaseInteropTransferCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let req = parse_transfer_request_from_command(&self.service, command)?;
        let response = self.service.release_transfer(&req)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id_opt(response.resource_id)
            .with_office_id_opt(response.office_id)
            .with_client_id_opt(response.client_id)
            .with_savings_id_opt(response.savings_id)
            .with_resource_external_id(ExternalId::new(response.transfer_code.clone()))
            .with_transaction_id_opt(response.release_transaction_id.map(|id| id.to_string()))
            .with_change("transferCode", json!(response.transfer_code))
            .with_change("transactionCode", json!(response.transaction_code))
            .with_change("state", json!(response.state.as_str()))
            .build())
    }
}

/// Alias matching Fineract Java class name `ReleaseInteropTransferHandler`.
pub type ReleaseInteropTransferHandler = ReleaseInteropTransferCommandHandler;

// ============================================================================
// 8. Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn usd() -> MonetaryCurrency {
        MonetaryCurrency::new("USD", 2, None)
    }

    fn money_usd(amount: Decimal) -> Money {
        Money::of(usd(), amount, FineractRoundingMode::HalfEven)
    }

    fn test_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 15).expect("valid date")
    }

    fn create_active_savings_account(
        account_id: i64,
        account_no: &str,
        external_id: &str,
        opening_deposit: Decimal,
    ) -> SavingsAccount {
        let date = test_date();
        let mut account = SavingsAccount::new_submitted(
            account_no,
            Some(100 + account_id),
            None,
            1,
            usd(),
            dec!(0.0),
            date,
        );
        account.id = Some(account_id);
        account.office_id = Some(1);
        account.external_id = Some(external_id.to_string());
        account
            .approve(date, Some(1))
            .expect("approve savings account");
        account
            .activate(date, Some(1))
            .expect("activate savings account");
        if opening_deposit > Decimal::ZERO {
            account
                .deposit(date, opening_deposit, None, date)
                .expect("initial deposit");
        }
        account
    }

    #[test]
    fn test_interop_identifier_type_discriminants_and_parsing() {
        assert_eq!(InteropIdentifierType::Msisdn.as_i32(), 1);
        assert_eq!(InteropIdentifierType::Email.as_i32(), 2);
        assert_eq!(InteropIdentifierType::PersonalId.as_i32(), 3);
        assert_eq!(InteropIdentifierType::Business.as_i32(), 4);
        assert_eq!(InteropIdentifierType::Device.as_i32(), 5);
        assert_eq!(InteropIdentifierType::AccountId.as_i32(), 6);
        assert_eq!(InteropIdentifierType::Iban.as_i32(), 7);
        assert_eq!(InteropIdentifierType::Alias.as_i32(), 8);

        for (code, expected) in [
            (1, InteropIdentifierType::Msisdn),
            (2, InteropIdentifierType::Email),
            (3, InteropIdentifierType::PersonalId),
            (4, InteropIdentifierType::Business),
            (5, InteropIdentifierType::Device),
            (6, InteropIdentifierType::AccountId),
            (7, InteropIdentifierType::Iban),
            (8, InteropIdentifierType::Alias),
        ] {
            assert_eq!(InteropIdentifierType::from_i32(code), Some(expected));
            assert_eq!(
                InteropIdentifierType::from_str(expected.as_str()).unwrap(),
                expected
            );
        }

        assert_eq!(
            InteropTransactionRole::Payer.transaction_type(),
            SavingsAccountTransactionType::Withdrawal
        );
        assert_eq!(
            InteropTransactionRole::Payee.transaction_type(),
            SavingsAccountTransactionType::Deposit
        );
        assert!(InteropTransactionRole::Payer.is_withdraw());
        assert!(!InteropTransactionRole::Payee.is_withdraw());
    }

    #[test]
    fn test_party_lookup_by_msisdn_and_iban() {
        let service = InteroperationService::default();
        let payer_account =
            create_active_savings_account(10, "SAV-00010", "ext-payer-10", dec!(1000.00));
        let payee_account =
            create_active_savings_account(20, "SAV-00020", "ext-payee-20", dec!(250.00));
        service.register_savings_account(payer_account);
        service.register_savings_account(payee_account);

        // Register MSISDN for Payer and IBAN (with sub_value_or_type BIC) for Payee
        let msisdn_id = service
            .register_party_identifier(
                10,
                InteropIdentifierType::Msisdn,
                "+254700112233",
                None,
                Some("admin"),
            )
            .expect("register MSISDN");
        assert_eq!(msisdn_id.account_id, 10);
        assert_eq!(msisdn_id.identifier_type, InteropIdentifierType::Msisdn);
        assert_eq!(msisdn_id.a_value, "+254700112233");

        let iban_id = service
            .register_party_identifier(
                20,
                InteropIdentifierType::Iban,
                "DE89370400440532013000",
                Some("COBADEFFXXX"),
                Some("admin"),
            )
            .expect("register IBAN");
        assert_eq!(iban_id.account_id, 20);
        assert_eq!(iban_id.identifier_type, InteropIdentifierType::Iban);
        assert_eq!(
            iban_id.sub_value_or_type.as_deref(),
            Some("COBADEFFXXX")
        );

        // Lookup by MSISDN
        let resolved_msisdn = service
            .lookup_account_by_identifier(InteropIdentifierType::Msisdn, "+254700112233", None)
            .expect("lookup MSISDN");
        assert_eq!(resolved_msisdn.account_id, 10);

        let account_resp = service
            .get_account_by_identifier(InteropIdentifierType::Msisdn, "+254700112233", None)
            .expect("get account response by MSISDN");
        assert_eq!(account_resp.account_id, "ext-payer-10");

        // Lookup by IBAN + sub_value_or_type
        let resolved_iban = service
            .lookup_account_by_identifier(
                InteropIdentifierType::Iban,
                "DE89370400440532013000",
                Some("COBADEFFXXX"),
            )
            .expect("lookup IBAN");
        assert_eq!(resolved_iban.account_id, 20);

        // Duplicate registration of same MSISDN must fail
        let dup_err = service.register_party_identifier(
            20,
            InteropIdentifierType::Msisdn,
            "+254700112233",
            None,
            None,
        );
        assert!(dup_err.is_err());

        // Delete MSISDN and confirm subsequent lookup fails
        let deleted = service
            .delete_party_identifier(InteropIdentifierType::Msisdn, "+254700112233", None)
            .expect("delete MSISDN");
        assert_eq!(deleted.id, msisdn_id.id);
        assert!(service
            .lookup_account_by_identifier(InteropIdentifierType::Msisdn, "+254700112233", None)
            .is_err());
    }

    #[test]
    fn test_payer_quote_prepare_hold_and_commit_debit_flow() {
        let service = InteroperationService::default();
        let payer_account =
            create_active_savings_account(101, "SAV-00101", "ext-payer-101", dec!(500.00));
        let payee_account =
            create_active_savings_account(202, "SAV-00202", "ext-payee-202", dec!(100.00));
        service.register_savings_account(payer_account);
        service.register_savings_account(payee_account);

        // 1. Payer Quote: amount = 120.00 USD, fsp_fee = 2.50 USD, fsp_commission = 0.50 USD
        // Net required = 120.00 + 2.50 - 0.50 = 122.00 USD (<= 500.00 withdrawable)
        let quote_req = InteropQuoteRequestData::new(
            "tx-code-001",
            "quote-code-001",
            101,
            money_usd(dec!(120.00)),
            InteropTransactionRole::Payer,
        )
        .with_fees(money_usd(dec!(2.50)))
        .with_fsp_commission(money_usd(dec!(0.50)));

        let quote_resp = service
            .calculate_quote(&quote_req)
            .expect("calculate payer quote");
        assert_eq!(quote_resp.state, InteropActionState::Accepted);
        assert_eq!(quote_resp.fsp_fee, Some(money_usd(dec!(2.50))));
        assert_eq!(quote_resp.fsp_commission, Some(money_usd(dec!(0.50))));

        // Quote exceeding withdrawable balance must fail
        let excessive_quote = InteropQuoteRequestData::new(
            "tx-code-excess",
            "quote-code-excess",
            101,
            money_usd(dec!(600.00)),
            InteropTransactionRole::Payer,
        );
        assert!(service.calculate_quote(&excessive_quote).is_err());

        // 2. Payer Prepare Transfer: places hold of 122.00 USD (120.00 + 2.50 - 0.50)
        let mut transfer_req = InteropTransferRequestData::new(
            "tx-code-001",
            "transfer-code-001",
            101,
            money_usd(dec!(120.00)),
            InteropTransactionRole::Payer,
        )
        .with_fsp_fee(money_usd(dec!(2.50)))
        .with_fsp_commission(money_usd(dec!(0.50)))
        .with_transaction_date(test_date());

        let prep_resp = service
            .prepare_transfer(&transfer_req)
            .expect("prepare payer transfer");
        assert_eq!(prep_resp.state, InteropActionState::Accepted);
        assert!(prep_resp.hold_transaction_id.is_some());

        // Verify savings account hold & withdrawable balance after prepare
        let acct_after_prepare = service
            .get_savings_account(101)
            .expect("get payer account after prepare");
        assert_eq!(acct_after_prepare.account_balance(), dec!(500.00));
        assert_eq!(acct_after_prepare.savings_amount_on_hold, dec!(122.00));
        assert_eq!(acct_after_prepare.withdrawable_balance(), dec!(378.00));

        // Duplicate prepare for the same transfer_code must fail with already-on-hold error
        assert!(service.prepare_transfer(&transfer_req).is_err());

        // 3. Payer Commit Transfer: releases hold of 122.00 and debits 122.00 from Payer's account
        let commit_resp = service
            .commit_transfer(&transfer_req)
            .expect("commit payer transfer");
        assert_eq!(commit_resp.state, InteropActionState::Accepted);
        assert!(commit_resp.completed_timestamp.is_some());
        assert!(commit_resp.settlement_transaction_id.is_some());

        let acct_after_commit = service
            .get_savings_account(101)
            .expect("get payer account after commit");
        assert_eq!(acct_after_commit.savings_amount_on_hold, dec!(0.00));
        assert_eq!(acct_after_commit.account_balance(), dec!(378.00));
        assert_eq!(acct_after_commit.withdrawable_balance(), dec!(378.00));

        // Duplicate commit for the same transfer_code must fail with already-committed error
        assert!(service.commit_transfer(&transfer_req).is_err());

        // 4. Payee Commit Transfer: credits Payee's savings account
        transfer_req.account_id = 202;
        transfer_req.transfer_code = "transfer-code-001-payee".to_string();
        transfer_req.transaction_role = InteropTransactionRole::Payee;
        transfer_req.fsp_fee = None;
        transfer_req.fsp_commission = None;

        let payee_commit = service
            .commit_transfer(&transfer_req)
            .expect("commit payee transfer");
        assert_eq!(payee_commit.state, InteropActionState::Accepted);

        let payee_after_commit = service
            .get_savings_account(202)
            .expect("get payee account");
        assert_eq!(payee_after_commit.account_balance(), dec!(220.00));
    }

    #[test]
    fn test_payer_prepare_and_release_abort_flow() {
        let service = InteroperationService::default();
        let payer_account =
            create_active_savings_account(303, "SAV-00303", "ext-payer-303", dec!(400.00));
        service.register_savings_account(payer_account);

        let transfer_req = InteropTransferRequestData::new(
            "tx-abort-001",
            "transfer-abort-001",
            303,
            money_usd(dec!(150.00)),
            InteropTransactionRole::Payer,
        )
        .with_fsp_fee(money_usd(dec!(5.00)))
        .with_transaction_date(test_date());

        // 1. Prepare places 155.00 USD on hold
        let prep_resp = service
            .prepare_transfer(&transfer_req)
            .expect("prepare transfer before abort");
        assert_eq!(prep_resp.state, InteropActionState::Accepted);

        let acct_on_hold = service.get_savings_account(303).unwrap();
        assert_eq!(acct_on_hold.account_balance(), dec!(400.00));
        assert_eq!(acct_on_hold.savings_amount_on_hold, dec!(155.00));
        assert_eq!(acct_on_hold.withdrawable_balance(), dec!(245.00));

        // 2. Release aborts the Mojaloop transfer and restores the held funds
        let release_resp = service
            .release_transfer(&transfer_req)
            .expect("release transfer hold");
        assert_eq!(release_resp.state, InteropActionState::Accepted);
        assert!(release_resp.release_transaction_id.is_some());

        let acct_after_release = service.get_savings_account(303).unwrap();
        assert_eq!(acct_after_release.account_balance(), dec!(400.00));
        assert_eq!(acct_after_release.savings_amount_on_hold, dec!(0.00));
        assert_eq!(acct_after_release.withdrawable_balance(), dec!(400.00));

        // Releasing again or committing after release must fail because no active hold remains
        assert!(service.release_transfer(&transfer_req).is_err());
        assert!(service.commit_transfer(&transfer_req).is_err());
    }

    #[tokio::test]
    async fn test_cqrs_command_handlers_and_dual_db_sql() {
        assert_eq!(
            DualDbInteropRepository::select_identifier_by_type_and_value_sql(
                SqlDialect::Postgres,
                false
            ),
            "SELECT id, account_id, type, a_value, sub_value_or_type, created_by, created_on, modified_by, modified_on \
             FROM interop_identifier WHERE type = $1 AND a_value = $2 AND sub_value_or_type IS NULL"
        );
        assert_eq!(
            DualDbInteropRepository::select_identifier_by_type_and_value_sql(
                SqlDialect::MySql,
                true
            ),
            "SELECT id, account_id, type, a_value, sub_value_or_type, created_by, created_on, modified_by, modified_on \
             FROM interop_identifier WHERE type = ? AND a_value = ? AND sub_value_or_type = ?"
        );

        let service = Arc::new(InteroperationService::default());
        let payer = create_active_savings_account(505, "SAV-00505", "ext-505", dec!(800.00));
        service.register_savings_account(payer);

        let ctx = ExecutionContext::system("default");

        // 1. Register identifier via CommandHandler
        let register_handler = RegisterInteropIdentifierCommandHandler::new(Arc::clone(&service));
        let reg_cmd = JsonCommand::new(
            1,
            json!({
                "accountId": "ext-505",
                "idType": "MSISDN",
                "idValue": "+14155550199"
            }),
        );
        let reg_res = register_handler
            .handle(&ctx, &reg_cmd)
            .await
            .expect("register command handler");
        assert_eq!(reg_res.savings_id, Some(505));

        // 2. Quote via CommandHandler
        let quote_handler = CreateInteropQuoteCommandHandler::new(Arc::clone(&service));
        let quote_cmd = JsonCommand::new(
            2,
            json!({
                "transactionCode": "tx-cmd-1",
                "quoteCode": "quote-cmd-1",
                "accountId": "ext-505",
                "transactionRole": "PAYER",
                "amount": { "amount": "200.00", "currency": "USD" },
                "fees": { "amount": "1.50", "currency": "USD" }
            }),
        );
        let quote_res = quote_handler
            .handle(&ctx, &quote_cmd)
            .await
            .expect("quote command handler");
        assert_eq!(
            quote_res.resource_external_id.as_ref().map(|e| e.as_str()),
            Some("quote-cmd-1")
        );

        // 3. Prepare & Commit via CommandHandlers
        let prepare_handler = PrepareInteropTransferCommandHandler::new(Arc::clone(&service));
        let transfer_payload = json!({
            "transactionCode": "tx-cmd-1",
            "transferCode": "tr-cmd-1",
            "accountId": "ext-505",
            "transactionRole": "PAYER",
            "amount": { "amount": "200.00", "currency": "USD" },
            "fspFee": { "amount": "1.50", "currency": "USD" }
        });
        let prep_cmd = JsonCommand::new(3, transfer_payload.clone());
        let prep_res = prepare_handler
            .handle(&ctx, &prep_cmd)
            .await
            .expect("prepare command handler");
        assert!(prep_res.transaction_id.is_some());

        let commit_handler = CommitInteropTransferCommandHandler::new(Arc::clone(&service));
        let commit_cmd = JsonCommand::new(4, transfer_payload);
        let commit_res = commit_handler
            .handle(&ctx, &commit_cmd)
            .await
            .expect("commit command handler");
        assert!(commit_res.transaction_id.is_some());

        let final_acct = service.get_savings_account(505).unwrap();
        assert_eq!(final_acct.account_balance(), dec!(598.50));
    }
}
