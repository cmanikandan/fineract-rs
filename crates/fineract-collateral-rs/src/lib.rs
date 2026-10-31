//! `fineract-collateral-rs` — Collateral Management, Client Collateral Valuation, and Loan Collateral Pledging.
//!
//! Ported from:
//! - `org.apache.fineract.portfolio.collateralmanagement.domain.{CollateralManagementDomain, ClientCollateralManagement, CollateralManagementRepositoryWrapper, ClientCollateralManagementRepositoryWrapper}`
//! - `org.apache.fineract.portfolio.collateralmanagement.service.{CollateralManagementWriteServiceImpl, CollateralManagementReadServiceImpl, ClientCollateralManagementWriteServiceImpl, ClientCollateralManagementReadServiceImpl, LoanCollateralManagementWriteServiceImpl, LoanCollateralManagementReadServiceImpl, LoanCollateralAssembler}`
//! - `org.apache.fineract.portfolio.collateralmanagement.handler.{CollateralProductCreateCommandHandler, CollateralProductUpdateCommandHandler, CollateralProductDeleteCommandHandler, ClientCollateralCreateCommandHandler, ClientCollateralUpdateCommandHandler, ClientCollateralDeleteCommandHandler, LoanCollateralDeleteCommandHandler}`
//! - `org.apache.fineract.portfolio.collateralmanagement.data.{CollateralManagementData, ClientCollateralManagementData, LoanCollateralResponseData, LoanCollateralTemplateData}`
//! - `org.apache.fineract.portfolio.collateralmanagement.exception.{CollateralNotFoundException, ClientCollateralNotFoundException, LoanCollateralManagementNotFoundException, CollateralCannotBeDeletedException, ClientCollateralCannotBeDeletedException, LoanCollateralAmountNotSufficientException}`
//! - `org.apache.fineract.portfolio.loanaccount.domain.LoanCollateralManagement`
//! - `org.apache.fineract.portfolio.loanaccount.exception.{InvalidAmountOfCollateralQuantity, InvalidAmountOfCollaterals}`
//! - `org.apache.fineract.portfolio.collateral.domain.LoanCollateral`
//! - `org.apache.fineract.portfolio.collateral.command.CollateralCommand`
//! - `org.apache.fineract.portfolio.collateral.service.{CollateralWritePlatformServiceJpaRepositoryImpl, CollateralReadPlatformServiceImpl}`
//! - `org.apache.fineract.portfolio.collateral.handler.{CreateCollateralCommandHandler, UpdateCollateralCommandHandler, DeleteCollateralCommandHandler}`

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AnyPool, Row};

use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{DialectSql, ExecutionContext, JsonCommand, SqlDialect};
use fineract_money::{CurrencyData, FineractDecimal, FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{
    DataValidatorBuilder, FineractError, FineractResult, PlatformApiDataValidationException,
    PlatformDomainRuleException,
};

// =============================================================================
// 1. CONSTANTS & INPUT PARAMETERS
// =============================================================================

/// Code name for legacy loan collateral types (`CollateralApiConstants.COLLATERAL_CODE_NAME`).
pub const COLLATERAL_CODE_NAME: &str = "LoanCollateral";

/// Scale used for collateral management quantities, base prices, and percentages (`scale = 5, precision = 20`).
pub const COLLATERAL_DECIMAL_SCALE: u32 = 5;

/// Scale used for legacy loan collateral values (`scale = 6, precision = 19`).
pub const LEGACY_COLLATERAL_DECIMAL_SCALE: u32 = 6;

/// Supported JSON parameters for Collateral Management Product (`m_collateral_management`) commands.
pub const COLLATERAL_PRODUCT_SUPPORTED_PARAMETERS: &[&str] = &[
    "name",
    "quality",
    "basePrice",
    "unitType",
    "pctToBase",
    "currency",
    "currencyCode",
    "collateralId",
    "locale",
    "dateFormat",
];

/// Supported JSON parameters for Client Collateral (`m_client_collateral_management`) commands.
pub const CLIENT_COLLATERAL_SUPPORTED_PARAMETERS: &[&str] = &[
    "collateralId",
    "clientId",
    "quantity",
    "totalCollateralValue",
    "locale",
    "dateFormat",
];

/// Supported JSON parameters for Loan Collateral Management (`m_loan_collateral_management`) commands.
pub const LOAN_COLLATERAL_MANAGEMENT_SUPPORTED_PARAMETERS: &[&str] = &[
    "id",
    "clientCollateralId",
    "collateralId",
    "loanId",
    "quantity",
    "transactionId",
    "isReleased",
    "locale",
    "dateFormat",
];

/// Supported JSON parameters for Legacy Loan Collateral (`m_loan_collateral`) commands.
pub const LEGACY_COLLATERAL_SUPPORTED_PARAMETERS: &[&str] = &[
    "loanId",
    "collateralId",
    "collateralTypeId",
    "value",
    "description",
    "locale",
    "dateFormat",
];

// =============================================================================
// 2. DOMAIN EXCEPTIONS & ERROR HELPERS
// =============================================================================

/// Raised when a collateral management product (`m_collateral_management`) is not found.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.exception.CollateralNotFoundException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollateralNotFoundException {
    pub id: i64,
}

impl CollateralNotFoundException {
    pub const CODE: &'static str = "error.msg.collateral.id.invalid";

    pub const fn new(id: i64) -> Self {
        Self { id }
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::resource_not_found(
            Self::CODE,
            format!("Collateral with identifier {} does not exist", self.id),
            self.id.to_string(),
        )
    }
}

/// Raised when a client collateral (`m_client_collateral_management`) is not found.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.exception.ClientCollateralNotFoundException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCollateralNotFoundException {
    pub id: i64,
}

impl ClientCollateralNotFoundException {
    pub const CODE: &'static str = "error.msg.client.collateral.id.invalid";

    pub const fn new(id: i64) -> Self {
        Self { id }
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::resource_not_found(
            Self::CODE,
            format!("Collateral with identifier {} does not exist", self.id),
            self.id.to_string(),
        )
    }
}

/// Raised when a pledged loan collateral (`m_loan_collateral_management`) is not found.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.exception.LoanCollateralManagementNotFoundException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanCollateralManagementNotFoundException {
    pub id: i64,
}

impl LoanCollateralManagementNotFoundException {
    pub const CODE: &'static str = "error.msg.loan.collateral.id.invalid";

    pub const fn new(id: i64) -> Self {
        Self { id }
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::resource_not_found(
            Self::CODE,
            format!("Loan Collateral with identifier {} does not exist", self.id),
            self.id.to_string(),
        )
    }
}

/// Raised when a collateral product cannot be deleted because it is attached to client collateral with positive quantity.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.exception.CollateralCannotBeDeletedException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollateralCannotBeDeletedException {
    pub collateral_id: i64,
}

impl CollateralCannotBeDeletedException {
    pub const CODE: &'static str =
        "error.msg.collateral.is.already.associated.with.client.collateral";
    pub const MESSAGE: &'static str =
        "This collateral cannot be deleted as this is associated with one or more client collaterals";

    pub const fn new(collateral_id: i64) -> Self {
        Self { collateral_id }
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        PlatformDomainRuleException::with_args(
            Self::CODE,
            Self::MESSAGE,
            vec![self.collateral_id.into()],
        )
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

/// Raised when a client collateral cannot be deleted because it has unreleased loan collateral pledges.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.exception.ClientCollateralCannotBeDeletedException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCollateralCannotBeDeletedException {
    pub client_collateral_id: i64,
}

impl ClientCollateralCannotBeDeletedException {
    pub const CODE: &'static str =
        "error.msg.client.collateral.is.already.associated.with.loan.collateral";
    pub const MESSAGE: &'static str =
        "This client collateral cannot be deleted as this is associated with one or more loan collaterals";

    pub const fn new(client_collateral_id: i64) -> Self {
        Self {
            client_collateral_id,
        }
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        PlatformDomainRuleException::with_args(
            Self::CODE,
            Self::MESSAGE,
            vec![self.client_collateral_id.into()],
        )
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

/// Raised when a pledged loan collateral quantity is non-positive or exceeds the client's available collateral quantity.
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.exception.InvalidAmountOfCollateralQuantity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAmountOfCollateralQuantity {
    pub quantity: Decimal,
}

impl InvalidAmountOfCollateralQuantity {
    pub const CODE: &'static str = "error.msg.loan.collateral.failed";

    pub const fn new(quantity: Decimal) -> Self {
        Self { quantity }
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        let q_str = self.quantity.to_string();
        let msg = if self.quantity <= Decimal::ZERO {
            format!("The collateral quantity value`{q_str}`` must be greater than zero")
        } else {
            format!("The collateral quantity value`{q_str}`` cannot exceed the available quantity")
        };
        PlatformDomainRuleException::with_args(Self::CODE, msg, vec![q_str.into()])
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

/// Raised when the total collateral value is less than the loan principal amount.
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.exception.InvalidAmountOfCollaterals`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAmountOfCollaterals {
    pub amount: Decimal,
}

impl InvalidAmountOfCollaterals {
    pub const CODE: &'static str = "error.msg.loan.collateral.failed";

    pub const fn new(amount: Decimal) -> Self {
        Self { amount }
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        let a_str = self.amount.to_string();
        PlatformDomainRuleException::with_args(
            Self::CODE,
            format!(
                "The total collateral value`{a_str}`` should be greater than or equal to the loan amount"
            ),
            vec![a_str.into()],
        )
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

/// Raised during loan disbursement when the total disbursed amount exceeds total pledged collateral value.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.exception.LoanCollateralAmountNotSufficientException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanCollateralAmountNotSufficientException {
    pub amount: Decimal,
}

impl LoanCollateralAmountNotSufficientException {
    pub const CODE: &'static str = "error.msg.loan.disbursement.amount.not.sufficient";

    pub const fn new(amount: Decimal) -> Self {
        Self { amount }
    }

    pub fn into_domain_rule_exception(self) -> PlatformDomainRuleException {
        let a_str = self.amount.to_string();
        PlatformDomainRuleException::with_args(
            Self::CODE,
            format!(
                "The disbursement amount `{a_str}`` is invalid or insufficient to disburse the loan."
            ),
            vec![a_str.into()],
        )
    }

    pub fn into_fineract_error(self) -> FineractError {
        FineractError::DomainRule(self.into_domain_rule_exception())
    }
}

/// Helper returning a not-found [`FineractError`] for a [`CollateralManagementProduct`].
pub fn collateral_product_not_found_error(id: i64) -> FineractError {
    CollateralNotFoundException::new(id).into_fineract_error()
}

/// Helper returning a not-found [`FineractError`] for a [`ClientCollateralManagement`].
pub fn client_collateral_not_found_error(id: i64) -> FineractError {
    ClientCollateralNotFoundException::new(id).into_fineract_error()
}

/// Helper returning a not-found [`FineractError`] for a [`LoanCollateralManagement`].
pub fn loan_collateral_management_not_found_error(id: i64) -> FineractError {
    LoanCollateralManagementNotFoundException::new(id).into_fineract_error()
}

/// Helper returning a not-found [`FineractError`] for a legacy [`LoanCollateral`].
pub fn legacy_loan_collateral_not_found_error(loan_id: i64, collateral_id: i64) -> FineractError {
    FineractError::resource_not_found(
        "error.msg.loan.collateral.",
        format!("Collateral with Id {collateral_id} does not exist for loan with Id {loan_id}"),
        collateral_id.to_string(),
    )
}

/// Helper returning a domain rule violation when a loan is not in `SubmittedAndPendingApproval` stage
/// for legacy collateral create/update/delete operations.
pub fn legacy_collateral_loan_stage_error(action: &str, loan_id: i64) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.loan.collateral.associated.loan.not.in.submitted.and.pending.approval.stage",
        format!(
            "This collateral cannot be {action} as the loan it is associated with is not in submitted and pending approval stage"
        ),
        vec![loan_id.into()],
    ))
}

#[inline]
fn round_collateral_decimal(value: Decimal, mode: FineractRoundingMode) -> Decimal {
    if value.is_zero() {
        return Decimal::ZERO;
    }
    FineractDecimal::new(value)
        .round_to(COLLATERAL_DECIMAL_SCALE, mode)
        .strip_trailing_zeros()
        .into_inner()
}

// =============================================================================
// 3. COLLATERAL MANAGEMENT PRODUCT (`m_collateral_management`)
// =============================================================================

/// Collateral Management Product definition (`m_collateral_management`).
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.domain.CollateralManagementDomain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollateralManagementProduct {
    pub id: i64,
    pub name: String,
    pub quality: String,
    pub base_price: Decimal,
    pub unit_type: String,
    pub pct_to_base: Decimal,
    pub currency_code: String,
}

/// Type alias matching the upstream Java class name `CollateralManagementDomain`.
pub type CollateralManagementDomain = CollateralManagementProduct;

/// Immutable DTO representation of a [`CollateralManagementProduct`].
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.data.CollateralManagementData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollateralManagementData {
    pub id: i64,
    pub name: String,
    pub quality: String,
    pub base_price: Decimal,
    pub unit_type: String,
    pub pct_to_base: Decimal,
    pub currency: String,
}

impl CollateralManagementProduct {
    /// Creates a new [`CollateralManagementProduct`].
    pub fn new(
        id: i64,
        name: impl Into<String>,
        quality: impl Into<String>,
        base_price: Decimal,
        unit_type: impl Into<String>,
        pct_to_base: Decimal,
        currency_code: impl Into<String>,
    ) -> Self {
        Self {
            id,
            name: name.into().trim().to_string(),
            quality: quality.into().trim().to_string(),
            base_price,
            unit_type: unit_type.into().trim().to_string(),
            pct_to_base,
            currency_code: currency_code.into().trim().to_uppercase(),
        }
    }

    /// Validates a `CREATE COLLATERAL_PRODUCT` command payload (`CollateralProductCreateRequest`).
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(COLLATERAL_PRODUCT_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("collateral-product");

        let name = command.string_value_of_parameter_named("name");
        validator.reset().parameter("name").value(name).not_blank();

        let quality = command.string_value_of_parameter_named("quality");
        validator
            .reset()
            .parameter("quality")
            .value(quality)
            .not_blank()
            .not_exceeding_length_of(40);

        let base_price = command.big_decimal_value_of_parameter_named("basePrice")?;
        validator
            .reset()
            .parameter("basePrice")
            .value(base_price)
            .not_null()
            .positive_amount();

        let pct_to_base = command.big_decimal_value_of_parameter_named("pctToBase")?;
        validator
            .reset()
            .parameter("pctToBase")
            .value(pct_to_base)
            .not_null()
            .positive_amount();

        let unit_type = command.string_value_of_parameter_named("unitType");
        validator
            .reset()
            .parameter("unitType")
            .value(unit_type)
            .not_blank()
            .not_exceeding_length_of(10);

        let currency = command
            .string_value_of_parameter_named("currency")
            .or_else(|| command.string_value_of_parameter_named("currencyCode"));
        validator
            .reset()
            .parameter("currency")
            .value(currency)
            .not_blank();

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates an `UPDATE COLLATERAL_PRODUCT` command payload (`CollateralProductUpdateRequest`).
    pub fn validate_for_update(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(COLLATERAL_PRODUCT_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("collateral-product");

        if command.parameter_exists("quality") {
            let quality = command.string_value_of_parameter_named("quality");
            validator
                .reset()
                .parameter("quality")
                .value(quality)
                .not_blank()
                .not_exceeding_length_of(40);
        }

        if command.parameter_exists("basePrice") {
            let base_price = command.big_decimal_value_of_parameter_named("basePrice")?;
            validator
                .reset()
                .parameter("basePrice")
                .value(base_price)
                .not_null()
                .positive_amount();
        }

        if command.parameter_exists("pctToBase") {
            let pct_to_base = command.big_decimal_value_of_parameter_named("pctToBase")?;
            validator
                .reset()
                .parameter("pctToBase")
                .value(pct_to_base)
                .not_null()
                .positive_amount();
        }

        if command.parameter_exists("unitType") {
            let unit_type = command.string_value_of_parameter_named("unitType");
            validator
                .reset()
                .parameter("unitType")
                .value(unit_type)
                .not_blank()
                .not_exceeding_length_of(10);
        }

        if command.parameter_exists("currency") || command.parameter_exists("currencyCode") {
            let currency = command
                .string_value_of_parameter_named("currency")
                .or_else(|| command.string_value_of_parameter_named("currencyCode"));
            validator
                .reset()
                .parameter("currency")
                .value(currency)
                .not_blank();
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a [`CollateralManagementProduct`] from a [`JsonCommand`].
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;
        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default()
            .trim()
            .to_string();
        let quality = command
            .string_value_of_parameter_named("quality")
            .unwrap_or_default()
            .trim()
            .to_string();
        let base_price = command
            .big_decimal_value_of_parameter_named("basePrice")?
            .unwrap_or(Decimal::ZERO);
        let pct_to_base = command
            .big_decimal_value_of_parameter_named("pctToBase")?
            .unwrap_or(Decimal::ZERO);
        let unit_type = command
            .string_value_of_parameter_named("unitType")
            .unwrap_or_default()
            .trim()
            .to_string();
        let currency_code = command
            .string_value_of_parameter_named("currency")
            .or_else(|| command.string_value_of_parameter_named("currencyCode"))
            .unwrap_or_default()
            .trim()
            .to_uppercase();

        Ok(Self {
            id,
            name,
            quality,
            base_price,
            unit_type,
            pct_to_base,
            currency_code,
        })
    }

    /// Applies an update command to `self`, returning a map of modified fields matching
    /// `CollateralManagementWriteServiceImpl.applyChanges`.
    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        Self::validate_for_update(command)?;
        let mut changes = BTreeMap::new();

        if command.parameter_exists("name") {
            let new_name = command
                .string_value_of_parameter_named("name")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if new_name != self.name {
                self.name = new_name.clone();
                if new_name.is_empty() {
                    changes.insert("name".to_string(), Value::Null);
                } else {
                    changes.insert("name".to_string(), Value::String(new_name));
                }
            }
        }

        if command.is_change_in_string_parameter_named("quality", Some(&self.quality)) {
            let new_quality = command
                .string_value_of_parameter_named("quality")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            self.quality = new_quality.clone();
            changes.insert("quality".to_string(), Value::String(new_quality));
        }

        if command.is_change_in_string_parameter_named("unitType", Some(&self.unit_type)) {
            let new_unit = command
                .string_value_of_parameter_named("unitType")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            self.unit_type = new_unit.clone();
            changes.insert("unitType".to_string(), Value::String(new_unit));
        }

        if command.parameter_exists("currency") || command.parameter_exists("currencyCode") {
            if let Some(ccy) = command
                .string_value_of_parameter_named("currency")
                .or_else(|| command.string_value_of_parameter_named("currencyCode"))
            {
                let normalized_ccy = ccy.trim().to_uppercase();
                if normalized_ccy != self.currency_code {
                    self.currency_code = normalized_ccy.clone();
                    changes.insert("currency".to_string(), Value::String(normalized_ccy));
                }
            }
        }

        if command.is_change_in_big_decimal_parameter_named("basePrice", Some(self.base_price))? {
            if let Some(new_price) = command.big_decimal_value_of_parameter_named("basePrice")? {
                self.base_price = new_price;
                changes.insert(
                    "basePrice".to_string(),
                    serde_json::to_value(new_price)
                        .unwrap_or_else(|_| Value::String(new_price.to_string())),
                );
            }
        }

        if command.is_change_in_big_decimal_parameter_named("pctToBase", Some(self.pct_to_base))? {
            if let Some(new_pct) = command.big_decimal_value_of_parameter_named("pctToBase")? {
                self.pct_to_base = new_pct;
                changes.insert(
                    "pctToBase".to_string(),
                    serde_json::to_value(new_pct)
                        .unwrap_or_else(|_| Value::String(new_pct.to_string())),
                );
            }
        }

        Ok(changes)
    }

    /// Returns the haircut multiplier (`pct_to_base / 100`).
    #[inline]
    pub fn haircut_ratio(&self) -> Decimal {
        self.pct_to_base / Decimal::ONE_HUNDRED
    }

    /// Computes gross market value (`quantity * base_price`) for a given `quantity`.
    #[inline]
    pub fn value_for_quantity(&self, quantity: Decimal, mode: FineractRoundingMode) -> Decimal {
        if quantity.is_zero() || self.base_price.is_zero() {
            return Decimal::ZERO;
        }
        round_collateral_decimal(quantity * self.base_price, mode)
    }

    /// Computes lendable collateral value after applying the `pct_to_base` haircut (`total_value * (pct_to_base / 100)`).
    #[inline]
    pub fn collateral_value_for_quantity(
        &self,
        quantity: Decimal,
        mode: FineractRoundingMode,
    ) -> Decimal {
        let total = self.value_for_quantity(quantity, mode);
        if total.is_zero() || self.pct_to_base.is_zero() {
            return Decimal::ZERO;
        }
        round_collateral_decimal(total * (self.pct_to_base / Decimal::ONE_HUNDRED), mode)
    }

    /// Converts this product into [`CollateralManagementData`].
    pub fn to_data(&self) -> CollateralManagementData {
        CollateralManagementData {
            id: self.id,
            name: self.name.clone(),
            quality: self.quality.clone(),
            base_price: self.base_price,
            unit_type: self.unit_type.clone(),
            pct_to_base: self.pct_to_base,
            currency: self.currency_code.clone(),
        }
    }
}

// =============================================================================
// 4. CLIENT COLLATERAL MANAGEMENT (`m_client_collateral_management`)
// =============================================================================

/// Client-owned collateral record (`m_client_collateral_management`).
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.domain.ClientCollateralManagement`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCollateralManagement {
    pub id: i64,
    pub client_id: i64,
    pub collateral_id: i64,
    pub quantity: Decimal,
}

/// Summary of a loan transaction linked to a client collateral item.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.data.LoanTransactionData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCollateralLoanTransactionData {
    pub loan_id: i64,
    pub transaction_date: Option<NaiveDate>,
    pub remaining_amount: Decimal,
    pub principal_portion: Decimal,
}

/// Read DTO for a client collateral record including computed `total` and `total_collateral` values.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.data.ClientCollateralManagementData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCollateralManagementData {
    pub id: i64,
    pub client_id: i64,
    pub name: String,
    pub quantity: Decimal,
    pub total: Decimal,
    pub total_collateral: Decimal,
    pub loan_transaction_data: Vec<ClientCollateralLoanTransactionData>,
}

/// Template DTO for pledging client collateral to a loan.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.data.LoanCollateralTemplateData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanCollateralTemplateData {
    pub collateral_id: i64,
    pub name: String,
    pub base_price: Decimal,
    pub pct_to_base: Decimal,
    pub quantity: Decimal,
}

impl ClientCollateralManagement {
    /// Creates a new [`ClientCollateralManagement`] record.
    pub const fn new(id: i64, client_id: i64, collateral_id: i64, quantity: Decimal) -> Self {
        Self {
            id,
            client_id,
            collateral_id,
            quantity,
        }
    }

    /// Computes the gross valuation of this client collateral (`quantity * base_price`),
    /// matching `ClientCollateralManagement.getTotal()`.
    #[inline]
    pub fn total_value(
        &self,
        product: &CollateralManagementProduct,
        mode: FineractRoundingMode,
    ) -> Decimal {
        if self.quantity.is_zero() || product.base_price.is_zero() {
            return Decimal::ZERO;
        }
        round_collateral_decimal(self.quantity * product.base_price, mode)
    }

    /// Computes the lendable collateral valuation after applying the product's `pct_to_base` haircut
    /// (`total_value * (pct_to_base / 100)`), matching `ClientCollateralManagement.getTotalCollateral(total)`.
    #[inline]
    pub fn total_collateral_value(
        &self,
        product: &CollateralManagementProduct,
        mode: FineractRoundingMode,
    ) -> Decimal {
        let total = self.total_value(product, mode);
        if total.is_zero() || product.pct_to_base.is_zero() {
            return Decimal::ZERO;
        }
        round_collateral_decimal(total * (product.pct_to_base / Decimal::ONE_HUNDRED), mode)
    }

    /// Converts [`Self::total_value`] into a [`Money`] instance using the product's currency code.
    pub fn total_value_money(
        &self,
        product: &CollateralManagementProduct,
        digits_after_decimal: u32,
        mode: FineractRoundingMode,
    ) -> Money {
        let currency = MonetaryCurrency::with_code(&product.currency_code, digits_after_decimal);
        Money::of(currency, self.total_value(product, mode), mode)
    }

    /// Converts [`Self::total_collateral_value`] into a [`Money`] instance using the product's currency code.
    pub fn total_collateral_value_money(
        &self,
        product: &CollateralManagementProduct,
        digits_after_decimal: u32,
        mode: FineractRoundingMode,
    ) -> Money {
        let currency = MonetaryCurrency::with_code(&product.currency_code, digits_after_decimal);
        Money::of(currency, self.total_collateral_value(product, mode), mode)
    }

    /// Computes the total currently pledged (unreleased) quantity across `pledged` loan collaterals
    /// associated with this client collateral.
    pub fn pledged_quantity(&self, pledged: &[LoanCollateralManagement]) -> Decimal {
        pledged
            .iter()
            .filter(|item| {
                !item.is_released
                    && (self.id <= 0
                        || item.client_collateral_id <= 0
                        || item.client_collateral_id == self.id)
            })
            .map(|item| item.quantity)
            .fold(Decimal::ZERO, |acc, q| acc + q)
    }

    /// Computes the remaining unpledged quantity available to pledge to new or updated loans
    /// (`quantity - sum(active pledged quantities)`).
    pub fn available_quantity(&self, pledged: &[LoanCollateralManagement]) -> Decimal {
        let active_pledged = self.pledged_quantity(pledged);
        (self.quantity - active_pledged).max(Decimal::ZERO)
    }

    /// Computes the available (unpledged) lendable collateral value after applying the `pct_to_base` haircut.
    pub fn available_collateral_value(
        &self,
        product: &CollateralManagementProduct,
        pledged: &[LoanCollateralManagement],
        mode: FineractRoundingMode,
    ) -> Decimal {
        let avail_qty = self.available_quantity(pledged);
        product.collateral_value_for_quantity(avail_qty, mode)
    }

    /// Updates `self.quantity` directly, matching `ClientCollateralManagement.updateQuantity`.
    pub fn update_quantity(&mut self, quantity: Decimal) {
        self.quantity = quantity;
    }

    /// Restores `released_quantity` to `self.quantity` after a loan is closed or written off,
    /// matching `ClientCollateralManagement.updateQuantityAfterLoanClosed`.
    pub fn update_quantity_after_loan_closed(&mut self, released_quantity: Decimal) {
        self.quantity += released_quantity;
    }

    /// Pledges `requested_quantity` of this client collateral to `loan_id`, validating that
    /// `requested_quantity > 0` and `requested_quantity <= self.available_quantity(pledged)`.
    pub fn pledge(
        &self,
        id: i64,
        loan_id: i64,
        requested_quantity: Decimal,
        pledged: &[LoanCollateralManagement],
        transaction_id: Option<i64>,
    ) -> FineractResult<LoanCollateralManagement> {
        LoanCollateralManagement::pledge(
            id,
            self,
            pledged,
            loan_id,
            requested_quantity,
            transaction_id,
        )
    }

    /// Validates a `CREATE CLIENT_COLLATERAL` command payload
    /// (`ClientCollateralManagementWriteServiceImpl.validateForCreation`).
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(CLIENT_COLLATERAL_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("client-collateral");

        let collateral_id = command.long_value_of_parameter_named("collateralId")?;
        if collateral_id.is_none() {
            validator
                .reset()
                .parameter("collateralId")
                .fail_with_code("parameter.collateralId.not.exists", "collateralId is required");
        } else {
            validator
                .reset()
                .parameter("collateralId")
                .value(collateral_id)
                .not_null()
                .integer_greater_than_zero();
        }

        let quantity = command.big_decimal_value_of_parameter_named("quantity")?;
        if quantity.is_none() {
            validator
                .reset()
                .parameter("quantity")
                .fail_with_code("parameter.quantity.not.exists", "quantity is required");
        } else {
            validator
                .reset()
                .parameter("quantity")
                .value(quantity)
                .not_null()
                .positive_amount();
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates an `UPDATE CLIENT_COLLATERAL` command payload against `pledged_quantity`
    /// (`ClientCollateralManagementWriteServiceImpl.validateForUpdate`).
    pub fn validate_for_update(
        command: &JsonCommand,
        pledged_quantity: Decimal,
    ) -> FineractResult<()> {
        command.check_for_unsupported_parameters(CLIENT_COLLATERAL_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("client-collateral");

        let quantity = command.big_decimal_value_of_parameter_named("quantity")?;
        match quantity {
            None => {
                validator
                    .reset()
                    .parameter("quantity")
                    .fail_with_code("parameter..quantity.not.exists", "quantity is required");
            }
            Some(q) => {
                validator
                    .reset()
                    .parameter("quantity")
                    .value(q)
                    .not_null()
                    .positive_amount();
                if pledged_quantity > Decimal::ZERO && q < pledged_quantity {
                    validator
                        .reset()
                        .parameter("quantity")
                        .value(q)
                        .not_less_than_min(pledged_quantity);
                }
            }
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a [`ClientCollateralManagement`] from a [`JsonCommand`].
    pub fn from_json(id: i64, client_id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;
        let collateral_id = command
            .long_value_of_parameter_named("collateralId")?
            .unwrap_or_default();
        let quantity = command
            .big_decimal_value_of_parameter_named("quantity")?
            .unwrap_or(Decimal::ZERO);
        Ok(Self {
            id,
            client_id,
            collateral_id,
            quantity,
        })
    }

    /// Updates `self.quantity` from a [`JsonCommand`], validating that the new quantity is positive
    /// and not less than the currently pledged quantity across `pledged`.
    pub fn update(
        &mut self,
        command: &JsonCommand,
        pledged: &[LoanCollateralManagement],
    ) -> FineractResult<BTreeMap<String, Value>> {
        let pledged_qty = self.pledged_quantity(pledged);
        Self::validate_for_update(command, pledged_qty)?;
        let mut changes = BTreeMap::new();

        if command.is_change_in_big_decimal_parameter_named("quantity", Some(self.quantity))? {
            if let Some(new_qty) = command.big_decimal_value_of_parameter_named("quantity")? {
                self.quantity = new_qty;
                changes.insert(
                    "quantity".to_string(),
                    serde_json::to_value(new_qty)
                        .unwrap_or_else(|_| Value::String(new_qty.to_string())),
                );
                let locale = command.locale();
                if !locale.is_empty() {
                    changes.insert("locale".to_string(), Value::String(locale));
                }
            }
        }

        Ok(changes)
    }

    /// Builds a [`ClientCollateralManagementData`] view for this client collateral.
    pub fn to_data(
        &self,
        product: &CollateralManagementProduct,
        loan_transaction_data: Vec<ClientCollateralLoanTransactionData>,
        mode: FineractRoundingMode,
    ) -> ClientCollateralManagementData {
        let total = self.total_value(product, mode);
        let total_collateral = self.total_collateral_value(product, mode);
        ClientCollateralManagementData {
            id: self.id,
            client_id: self.client_id,
            name: product.name.clone(),
            quantity: self.quantity,
            total,
            total_collateral,
            loan_transaction_data,
        }
    }

    /// Builds a [`LoanCollateralTemplateData`] view for this client collateral.
    pub fn to_template_data(
        &self,
        product: &CollateralManagementProduct,
    ) -> LoanCollateralTemplateData {
        LoanCollateralTemplateData {
            collateral_id: self.id,
            name: product.name.clone(),
            base_price: product.base_price,
            pct_to_base: product.pct_to_base,
            quantity: self.quantity,
        }
    }
}

// =============================================================================
// 5. LOAN COLLATERAL MANAGEMENT (`m_loan_collateral_management`)
// =============================================================================

/// Pledged loan collateral record linking a loan to a client collateral item (`m_loan_collateral_management`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanCollateralManagement` and
/// `org.apache.fineract.portfolio.collateralmanagement.service.LoanCollateralAssembler`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanCollateralManagement {
    pub id: i64,
    pub client_collateral_id: i64,
    pub loan_id: i64,
    pub quantity: Decimal,
    pub is_released: bool,
    pub transaction_id: Option<i64>,
}

/// Read DTO for a pledged loan collateral item with valuation details.
///
/// Ported from `org.apache.fineract.portfolio.collateralmanagement.data.LoanCollateralResponseData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanCollateralResponseData {
    pub collateral_id: i64,
    pub client_collateral_id: i64,
    pub quantity: Decimal,
    pub total: Decimal,
    pub total_collateral: Decimal,
    pub name: String,
    pub unit_price: Decimal,
    pub pct_to_base: Decimal,
}

/// Command/DTO representation of [`LoanCollateralManagement`].
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.data.LoanCollateralManagementData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanCollateralManagementData {
    pub id: i64,
    pub client_collateral_id: i64,
    pub quantity: Decimal,
    pub total: Option<Decimal>,
    pub total_collateral: Option<Decimal>,
    pub name: Option<String>,
    pub unit_price: Option<Decimal>,
    pub pct_to_base: Option<Decimal>,
}

impl LoanCollateralManagement {
    /// Creates a [`LoanCollateralManagement`] instance directly without validation.
    pub const fn new(
        id: i64,
        client_collateral_id: i64,
        loan_id: i64,
        quantity: Decimal,
        is_released: bool,
        transaction_id: Option<i64>,
    ) -> Self {
        Self {
            id,
            client_collateral_id,
            loan_id,
            quantity,
            is_released,
            transaction_id,
        }
    }

    /// Validates that `requested_quantity > 0` and `requested_quantity <= available_quantity`,
    /// rejecting invalid or over-pledged quantities with [`InvalidAmountOfCollateralQuantity`].
    pub fn validate_pledge_quantity(
        requested_quantity: Decimal,
        available_quantity: Decimal,
    ) -> FineractResult<()> {
        if requested_quantity <= Decimal::ZERO || requested_quantity > available_quantity {
            return Err(InvalidAmountOfCollateralQuantity::new(requested_quantity).into_fineract_error());
        }
        Ok(())
    }

    /// Validates a pledge request against a [`ClientCollateralManagement`] and its existing pledges.
    pub fn validate_pledge(
        client_collateral: &ClientCollateralManagement,
        existing_pledges: &[LoanCollateralManagement],
        requested_quantity: Decimal,
    ) -> FineractResult<()> {
        let available = client_collateral.available_quantity(existing_pledges);
        Self::validate_pledge_quantity(requested_quantity, available)
    }

    /// Creates a new active (`is_released = false`) [`LoanCollateralManagement`] pledge after
    /// verifying that `requested_quantity > 0` and `requested_quantity <= client_collateral.available_quantity(existing_pledges)`.
    ///
    /// Ported from `LoanCollateralAssembler.fromParsedJson`.
    pub fn pledge(
        id: i64,
        client_collateral: &ClientCollateralManagement,
        existing_pledges: &[LoanCollateralManagement],
        loan_id: i64,
        requested_quantity: Decimal,
        transaction_id: Option<i64>,
    ) -> FineractResult<Self> {
        Self::validate_pledge(client_collateral, existing_pledges, requested_quantity)?;
        Ok(Self {
            id,
            client_collateral_id: client_collateral.id,
            loan_id,
            quantity: requested_quantity,
            is_released: false,
            transaction_id,
        })
    }

    /// Alias for [`LoanCollateralManagement::pledge`].
    pub fn from_client_collateral(
        id: i64,
        client_collateral: &ClientCollateralManagement,
        existing_pledges: &[LoanCollateralManagement],
        loan_id: i64,
        requested_quantity: Decimal,
        transaction_id: Option<i64>,
    ) -> FineractResult<Self> {
        Self::pledge(
            id,
            client_collateral,
            existing_pledges,
            loan_id,
            requested_quantity,
            transaction_id,
        )
    }

    /// Updates the pledged quantity of an existing [`LoanCollateralManagement`], accounting for
    /// `self.quantity` already pledged when checking `client_collateral.available_quantity(other_pledges)`.
    ///
    /// Ported from the `id != null` branch of `LoanCollateralAssembler.fromParsedJson`.
    pub fn update_quantity_with_validation(
        &mut self,
        client_collateral: &ClientCollateralManagement,
        existing_pledges: &[LoanCollateralManagement],
        new_quantity: Decimal,
    ) -> FineractResult<()> {
        if new_quantity <= Decimal::ZERO {
            return Err(InvalidAmountOfCollateralQuantity::new(new_quantity).into_fineract_error());
        }
        let other_pledged: Decimal = existing_pledges
            .iter()
            .filter(|p| {
                !p.is_released
                    && p.id != self.id
                    && (client_collateral.id <= 0
                        || p.client_collateral_id <= 0
                        || p.client_collateral_id == client_collateral.id)
            })
            .map(|p| p.quantity)
            .fold(Decimal::ZERO, |acc, q| acc + q);
        let available_for_this_pledge = (client_collateral.quantity - other_pledged).max(Decimal::ZERO);
        Self::validate_pledge_quantity(new_quantity, available_for_this_pledge)?;
        self.quantity = new_quantity;
        Ok(())
    }

    /// Marks this collateral pledge as released (`is_released = true`).
    /// Returns `true` if the pledge transitioned from unreleased to released, or `false` if already released.
    #[inline]
    pub fn release(&mut self) -> bool {
        if self.is_released {
            false
        } else {
            self.is_released = true;
            true
        }
    }

    /// Automatically releases this collateral pledge when the associated loan closes
    /// (obligation met / overpaid / closed) or is written off.
    ///
    /// Ported from `LoanAccountDomainServiceJpa.updateAndSaveLoanCollateralTransactionsForIndividualAccounts`
    /// and `LoanApplicationWritePlatformServiceJpaRepositoryImpl.releaseAttachedCollaterals`.
    #[inline]
    pub fn release_on_loan_closure(&mut self) -> bool {
        self.release()
    }

    /// Automatically releases this collateral pledge on loan write-off.
    #[inline]
    pub fn release_on_write_off(&mut self) -> bool {
        self.release()
    }

    /// Releases this collateral pledge and adds its `quantity` back to `client_collateral`
    /// (for workflows that deduct pledged quantity directly from `ClientCollateralManagement.quantity`).
    pub fn release_and_restore_client_quantity(
        &mut self,
        client_collateral: &mut ClientCollateralManagement,
    ) -> bool {
        if self.release() {
            client_collateral.update_quantity_after_loan_closed(self.quantity);
            true
        } else {
            false
        }
    }

    /// Releases all unreleased collateral pledges belonging to `loan_id` upon loan closure or write-off,
    /// returning the number of pledges that were newly released.
    pub fn release_all_for_loan(pledges: &mut [Self], loan_id: i64) -> usize {
        let mut count = 0;
        for pledge in pledges.iter_mut() {
            if pledge.loan_id == loan_id && pledge.release() {
                count += 1;
            }
        }
        count
    }

    /// Computes the gross valuation of this pledged collateral (`quantity * base_price`).
    #[inline]
    pub fn total_value(
        &self,
        product: &CollateralManagementProduct,
        mode: FineractRoundingMode,
    ) -> Decimal {
        product.value_for_quantity(self.quantity, mode)
    }

    /// Computes the lendable collateral valuation of this pledge after applying the `pct_to_base` haircut
    /// (`total_value * (pct_to_base / 100)`).
    #[inline]
    pub fn total_collateral_value(
        &self,
        product: &CollateralManagementProduct,
        mode: FineractRoundingMode,
    ) -> Decimal {
        product.collateral_value_for_quantity(self.quantity, mode)
    }

    /// Converts this pledge into [`LoanCollateralResponseData`].
    pub fn to_response_data(
        &self,
        product: &CollateralManagementProduct,
        mode: FineractRoundingMode,
    ) -> LoanCollateralResponseData {
        let total = self.total_value(product, mode);
        let total_collateral = self.total_collateral_value(product, mode);
        LoanCollateralResponseData {
            collateral_id: self.id,
            client_collateral_id: self.client_collateral_id,
            quantity: self.quantity,
            total,
            total_collateral,
            name: product.name.clone(),
            unit_price: product.base_price,
            pct_to_base: product.pct_to_base,
        }
    }

    /// Converts this pledge into [`LoanCollateralManagementData`], matching `LoanCollateralManagement.toCommand()`.
    pub fn to_command_data(
        &self,
        product: Option<&CollateralManagementProduct>,
        mode: FineractRoundingMode,
    ) -> LoanCollateralManagementData {
        match product {
            Some(prod) => LoanCollateralManagementData {
                id: self.id,
                client_collateral_id: self.client_collateral_id,
                quantity: self.quantity,
                total: Some(self.total_value(prod, mode)),
                total_collateral: Some(self.total_collateral_value(prod, mode)),
                name: Some(prod.name.clone()),
                unit_price: Some(prod.base_price),
                pct_to_base: Some(prod.pct_to_base),
            },
            None => LoanCollateralManagementData {
                id: self.id,
                client_collateral_id: self.client_collateral_id,
                quantity: self.quantity,
                total: None,
                total_collateral: None,
                name: None,
                unit_price: None,
                pct_to_base: None,
            },
        }
    }
}

// =============================================================================
// 6. LEGACY LOAN COLLATERAL (`m_loan_collateral`)
// =============================================================================

/// Legacy Loan Collateral record attached directly to a loan (`m_loan_collateral`).
///
/// Ported from `org.apache.fineract.portfolio.collateral.domain.LoanCollateral`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanCollateral {
    pub id: i64,
    pub loan_id: i64,
    pub type_cv_id: i64,
    pub value: Decimal,
    pub description: Option<String>,
}

/// Code value summary for a legacy [`LoanCollateral`] type (`m_code_value`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollateralCodeValueData {
    pub id: i64,
    pub name: String,
}

/// Read DTO for a legacy [`LoanCollateral`] (`org.apache.fineract.portfolio.collateral.data.CollateralData`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollateralData {
    pub id: i64,
    pub r#type: CollateralCodeValueData,
    pub value: Option<Decimal>,
    pub description: Option<String>,
    pub currency: Option<CurrencyData>,
}

impl LoanCollateral {
    /// Creates a new legacy [`LoanCollateral`] record, normalizing empty descriptions to `None`
    /// matching `StringUtils.defaultIfEmpty(description, null)` in Java.
    pub fn new(
        id: i64,
        loan_id: i64,
        type_cv_id: i64,
        value: Decimal,
        description: Option<String>,
    ) -> Self {
        let normalized_desc = description
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty());
        Self {
            id,
            loan_id,
            type_cv_id,
            value,
            description: normalized_desc,
        }
    }

    /// Validates a `CREATE COLLATERAL` command payload (`CollateralCommand.validateForCreate`).
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(LEGACY_COLLATERAL_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("collateral");

        let collateral_type_id = command.long_value_of_parameter_named("collateralTypeId")?;
        validator
            .reset()
            .parameter("collateralTypeId")
            .value(collateral_type_id)
            .not_null()
            .integer_greater_than_zero();

        if command.parameter_exists("value") {
            let value = command.big_decimal_value_of_parameter_named("value")?;
            validator
                .reset()
                .parameter("value")
                .value(value)
                .ignore_if_null()
                .positive_amount();
        }

        if command.parameter_exists("description") {
            let description = command.string_value_of_parameter_named("description");
            validator
                .reset()
                .parameter("description")
                .value(description)
                .ignore_if_null()
                .not_exceeding_length_of(500);
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates an `UPDATE COLLATERAL` command payload (`CollateralCommand.validateForUpdate`).
    pub fn validate_for_update(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(LEGACY_COLLATERAL_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("collateral");

        if command.parameter_exists("collateralTypeId") {
            let collateral_type_id = command.long_value_of_parameter_named("collateralTypeId")?;
            validator
                .reset()
                .parameter("collateralTypeId")
                .value(collateral_type_id)
                .ignore_if_null()
                .integer_greater_than_zero();
        }

        if command.parameter_exists("value") {
            let value = command.big_decimal_value_of_parameter_named("value")?;
            validator
                .reset()
                .parameter("value")
                .value(value)
                .ignore_if_null()
                .positive_amount();
        }

        if command.parameter_exists("description") {
            let description = command.string_value_of_parameter_named("description");
            validator
                .reset()
                .parameter("description")
                .value(description)
                .ignore_if_null()
                .not_exceeding_length_of(500);
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a legacy [`LoanCollateral`] from a [`JsonCommand`], matching `LoanCollateral.fromJson`.
    pub fn from_json(id: i64, loan_id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;
        let type_cv_id = command
            .long_value_of_parameter_named("collateralTypeId")?
            .unwrap_or_default();
        let value = command
            .big_decimal_value_of_parameter_named("value")?
            .unwrap_or(Decimal::ZERO);
        let description = command.string_value_of_parameter_named("description");
        Ok(Self::new(id, loan_id, type_cv_id, value, description))
    }

    /// Applies an update command to a legacy [`LoanCollateral`], returning the map of changed fields
    /// matching `LoanCollateral.update(final JsonCommand command)`.
    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        Self::validate_for_update(command)?;
        let mut actual_changes = BTreeMap::new();

        if command
            .is_change_in_long_parameter_named("collateralTypeId", Some(self.type_cv_id))?
        {
            if let Some(new_type_id) = command.long_value_of_parameter_named("collateralTypeId")? {
                self.type_cv_id = new_type_id;
                actual_changes.insert("collateralTypeId".to_string(), Value::from(new_type_id));
            }
        }

        if command
            .is_change_in_string_parameter_named("description", self.description.as_deref())
        {
            let new_desc = command.string_value_of_parameter_named("description");
            actual_changes.insert(
                "description".to_string(),
                new_desc
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            self.description = new_desc
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }

        if command.is_change_in_big_decimal_parameter_named("value", Some(self.value))? {
            let new_val = command
                .big_decimal_value_of_parameter_named("value")?
                .unwrap_or(Decimal::ZERO);
            self.value = new_val;
            actual_changes.insert(
                "value".to_string(),
                serde_json::to_value(new_val)
                    .unwrap_or_else(|_| Value::String(new_val.to_string())),
            );
        }

        Ok(actual_changes)
    }

    /// Converts this legacy [`LoanCollateral`] into [`CollateralData`].
    pub fn to_data(
        &self,
        type_name: Option<String>,
        currency: Option<CurrencyData>,
    ) -> CollateralData {
        CollateralData {
            id: self.id,
            r#type: CollateralCodeValueData {
                id: self.type_cv_id,
                name: type_name.unwrap_or_else(|| format!("Type {}", self.type_cv_id)),
            },
            value: if self.value.is_zero() {
                None
            } else {
                Some(self.value)
            },
            description: self.description.clone(),
            currency,
        }
    }
}

// =============================================================================
// 7. DUAL-DB COLLATERAL REPOSITORY (`DualDbCollateralRepository`)
// =============================================================================

/// Dual-database (PostgreSQL & MySQL/MariaDB) and in-memory repository for:
/// - [`CollateralManagementProduct`] (`m_collateral_management`)
/// - [`ClientCollateralManagement`] (`m_client_collateral_management`)
/// - [`LoanCollateralManagement`] (`m_loan_collateral_management`)
/// - Legacy [`LoanCollateral`] (`m_loan_collateral`)
#[derive(Debug, Clone)]
pub struct DualDbCollateralRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    products: Arc<DashMap<i64, CollateralManagementProduct>>,
    client_collaterals: Arc<DashMap<i64, ClientCollateralManagement>>,
    loan_collateral_pledges: Arc<DashMap<i64, LoanCollateralManagement>>,
    legacy_loan_collaterals: Arc<DashMap<i64, LoanCollateral>>,
    next_id: Arc<AtomicI64>,
}

impl Default for DualDbCollateralRepository {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl DualDbCollateralRepository {
    pub const COLLATERAL_PRODUCT_COLUMNS: &'static str =
        "id, name, quality, base_price, unit_type, pct_to_base, currency";
    pub const CLIENT_COLLATERAL_COLUMNS: &'static str =
        "id, quantity, client_id, collateral_id";
    pub const LOAN_COLLATERAL_MANAGEMENT_COLUMNS: &'static str =
        "id, quantity, transaction_id, loan_id, is_released, client_collateral_id";
    pub const LEGACY_LOAN_COLLATERAL_COLUMNS: &'static str =
        "id, loan_id, type_cv_id, value, description";

    /// Creates a new [`DualDbCollateralRepository`] backed by a SQLx [`AnyPool`] and [`SqlDialect`].
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            products: Arc::new(DashMap::new()),
            client_collaterals: Arc::new(DashMap::new()),
            loan_collateral_pledges: Arc::new(DashMap::new()),
            legacy_loan_collaterals: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Creates an in-memory [`DualDbCollateralRepository`] (defaulting to PostgreSQL SQL dialect).
    pub fn in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    /// Alias for [`DualDbCollateralRepository::in_memory`].
    pub fn new_in_memory() -> Self {
        Self::in_memory()
    }

    /// Creates an in-memory [`DualDbCollateralRepository`] configured with `dialect`.
    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            pool: None,
            dialect,
            products: Arc::new(DashMap::new()),
            client_collaterals: Arc::new(DashMap::new()),
            loan_collateral_pledges: Arc::new(DashMap::new()),
            legacy_loan_collaterals: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(1)),
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

    pub fn select_collateral_product_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_collateral_management WHERE id = ?",
                Self::COLLATERAL_PRODUCT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_all_collateral_products_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_collateral_management ORDER BY id ASC",
                Self::COLLATERAL_PRODUCT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_collateral_product_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_collateral_management (id, name, quality, base_price, unit_type, pct_to_base, currency) VALUES (?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn update_collateral_product_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_collateral_management SET name = ?, quality = ?, base_price = ?, unit_type = ?, pct_to_base = ?, currency = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn delete_collateral_product_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_collateral_management WHERE id = ?",
            dialect,
        )
    }

    pub fn select_client_collateral_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_client_collateral_management WHERE id = ?",
                Self::CLIENT_COLLATERAL_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_client_collaterals_by_client_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_client_collateral_management WHERE client_id = ? ORDER BY id ASC",
                Self::CLIENT_COLLATERAL_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_client_collateral_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_client_collateral_management (id, quantity, client_id, collateral_id) VALUES (?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn update_client_collateral_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_client_collateral_management SET quantity = ?, client_id = ?, collateral_id = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn delete_client_collateral_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_client_collateral_management WHERE id = ?",
            dialect,
        )
    }

    pub fn select_loan_collateral_management_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_collateral_management WHERE id = ?",
                Self::LOAN_COLLATERAL_MANAGEMENT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_loan_collateral_managements_by_loan_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_collateral_management WHERE loan_id = ? ORDER BY id ASC",
                Self::LOAN_COLLATERAL_MANAGEMENT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_loan_collateral_managements_by_client_collateral_sql(
        dialect: SqlDialect,
    ) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_collateral_management WHERE client_collateral_id = ? ORDER BY id ASC",
                Self::LOAN_COLLATERAL_MANAGEMENT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_loan_collateral_management_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan_collateral_management (id, quantity, transaction_id, loan_id, is_released, client_collateral_id) VALUES (?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn update_loan_collateral_management_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_loan_collateral_management SET quantity = ?, transaction_id = ?, loan_id = ?, is_released = ?, client_collateral_id = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn release_loan_collateral_managements_by_loan_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_loan_collateral_management SET is_released = ? WHERE loan_id = ?",
            dialect,
        )
    }

    pub fn delete_loan_collateral_management_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_loan_collateral_management WHERE id = ?",
            dialect,
        )
    }

    pub fn select_legacy_collateral_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_collateral WHERE loan_id = ? AND id = ?",
                Self::LEGACY_LOAN_COLLATERAL_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_legacy_collaterals_by_loan_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_collateral WHERE loan_id = ? ORDER BY id ASC",
                Self::LEGACY_LOAN_COLLATERAL_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_legacy_collateral_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan_collateral (id, loan_id, type_cv_id, value, description) VALUES (?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn update_legacy_collateral_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_loan_collateral SET type_cv_id = ?, value = ?, description = ? WHERE loan_id = ? AND id = ?",
            dialect,
        )
    }

    pub fn delete_legacy_collateral_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_loan_collateral WHERE loan_id = ? AND id = ?",
            dialect,
        )
    }

    // --- Collateral Product Operations (`m_collateral_management`) ---

    /// Saves a new [`CollateralManagementProduct`].
    pub async fn save_collateral_product(
        &self,
        mut product: CollateralManagementProduct,
    ) -> FineractResult<CollateralManagementProduct> {
        if product.id <= 0 {
            product.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_collateral_product_sql(self.dialect);
            sqlx::query(&sql)
                .bind(product.id)
                .bind(&product.name)
                .bind(&product.quality)
                .bind(product.base_price.to_string())
                .bind(&product.unit_type)
                .bind(product.pct_to_base.to_string())
                .bind(&product.currency_code)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.products.insert(product.id, product.clone());
        Ok(product)
    }

    /// Alias for [`DualDbCollateralRepository::save_collateral_product`].
    pub async fn create_collateral_product(
        &self,
        product: CollateralManagementProduct,
    ) -> FineractResult<CollateralManagementProduct> {
        self.save_collateral_product(product).await
    }

    /// Updates an existing [`CollateralManagementProduct`].
    pub async fn update_collateral_product(
        &self,
        product: CollateralManagementProduct,
    ) -> FineractResult<CollateralManagementProduct> {
        if let Some(ref pool) = self.pool {
            let sql = Self::update_collateral_product_sql(self.dialect);
            sqlx::query(&sql)
                .bind(&product.name)
                .bind(&product.quality)
                .bind(product.base_price.to_string())
                .bind(&product.unit_type)
                .bind(product.pct_to_base.to_string())
                .bind(&product.currency_code)
                .bind(product.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.products.insert(product.id, product.clone());
        Ok(product)
    }

    /// Finds a [`CollateralManagementProduct`] by ID.
    pub async fn find_collateral_product_by_id(
        &self,
        id: i64,
    ) -> FineractResult<Option<CollateralManagementProduct>> {
        if let Some(entry) = self.products.get(&id) {
            return Ok(Some(entry.value().clone()));
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_collateral_product_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let row_id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let name: Option<String> = row
                    .try_get("name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let quality: String = row
                    .try_get("quality")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let base_price_raw: String = row
                    .try_get("base_price")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let unit_type: String = row
                    .try_get("unit_type")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let pct_to_base_raw: String = row
                    .try_get("pct_to_base")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let currency_code: String = row
                    .try_get("currency")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

                let base_price = Decimal::from_str(&base_price_raw).unwrap_or(Decimal::ZERO);
                let pct_to_base = Decimal::from_str(&pct_to_base_raw).unwrap_or(Decimal::ZERO);

                let product = CollateralManagementProduct {
                    id: row_id,
                    name: name.unwrap_or_default(),
                    quality,
                    base_price,
                    unit_type,
                    pct_to_base,
                    currency_code,
                };
                self.products.insert(row_id, product.clone());
                return Ok(Some(product));
            }
        }
        Ok(None)
    }

    /// Retrieves a [`CollateralManagementProduct`] by ID, returning [`CollateralNotFoundException`] if absent.
    pub async fn get_collateral_product(
        &self,
        id: i64,
    ) -> FineractResult<CollateralManagementProduct> {
        self.find_collateral_product_by_id(id)
            .await?
            .ok_or_else(|| collateral_product_not_found_error(id))
    }

    /// Lists all [`CollateralManagementProduct`] records ordered by ID.
    pub async fn list_collateral_products(&self) -> FineractResult<Vec<CollateralManagementProduct>> {
        let mut items: Vec<CollateralManagementProduct> =
            self.products.iter().map(|e| e.value().clone()).collect();
        items.sort_by_key(|p| p.id);
        Ok(items)
    }

    /// Deletes a [`CollateralManagementProduct`] by ID after verifying that no client collateral
    /// with `quantity > 0` is attached (`CollateralManagementWriteServiceImpl.validateForDeletion`).
    pub async fn delete_collateral_product(&self, id: i64) -> FineractResult<()> {
        let _product = self.get_collateral_product(id).await?;
        for entry in self.client_collaterals.iter() {
            let cc = entry.value();
            if cc.collateral_id == id && cc.quantity > Decimal::ZERO {
                return Err(CollateralCannotBeDeletedException::new(id).into_fineract_error());
            }
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::delete_collateral_product_sql(self.dialect);
            sqlx::query(&sql)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.products.remove(&id);
        Ok(())
    }

    // --- Client Collateral Operations (`m_client_collateral_management`) ---

    /// Saves a new [`ClientCollateralManagement`] record after verifying that its [`CollateralManagementProduct`] exists.
    pub async fn save_client_collateral(
        &self,
        mut client_collateral: ClientCollateralManagement,
    ) -> FineractResult<ClientCollateralManagement> {
        let _product = self
            .get_collateral_product(client_collateral.collateral_id)
            .await?;
        if client_collateral.id <= 0 {
            client_collateral.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_client_collateral_sql(self.dialect);
            sqlx::query(&sql)
                .bind(client_collateral.id)
                .bind(client_collateral.quantity.to_string())
                .bind(client_collateral.client_id)
                .bind(client_collateral.collateral_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.client_collaterals
            .insert(client_collateral.id, client_collateral.clone());
        Ok(client_collateral)
    }

    /// Updates an existing [`ClientCollateralManagement`] record.
    pub async fn update_client_collateral(
        &self,
        client_collateral: ClientCollateralManagement,
    ) -> FineractResult<ClientCollateralManagement> {
        if let Some(ref pool) = self.pool {
            let sql = Self::update_client_collateral_sql(self.dialect);
            sqlx::query(&sql)
                .bind(client_collateral.quantity.to_string())
                .bind(client_collateral.client_id)
                .bind(client_collateral.collateral_id)
                .bind(client_collateral.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.client_collaterals
            .insert(client_collateral.id, client_collateral.clone());
        Ok(client_collateral)
    }

    /// Finds a [`ClientCollateralManagement`] by ID.
    pub async fn find_client_collateral_by_id(
        &self,
        id: i64,
    ) -> FineractResult<Option<ClientCollateralManagement>> {
        if let Some(entry) = self.client_collaterals.get(&id) {
            return Ok(Some(entry.value().clone()));
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_client_collateral_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let row_id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let qty_raw: String = row
                    .try_get("quantity")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let client_id: i64 = row
                    .try_get("client_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let collateral_id: i64 = row
                    .try_get("collateral_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let quantity = Decimal::from_str(&qty_raw).unwrap_or(Decimal::ZERO);
                let record =
                    ClientCollateralManagement::new(row_id, client_id, collateral_id, quantity);
                self.client_collaterals.insert(row_id, record.clone());
                return Ok(Some(record));
            }
        }
        Ok(None)
    }

    /// Retrieves a [`ClientCollateralManagement`] by ID, returning [`ClientCollateralNotFoundException`] if absent.
    pub async fn get_client_collateral(&self, id: i64) -> FineractResult<ClientCollateralManagement> {
        self.find_client_collateral_by_id(id)
            .await?
            .ok_or_else(|| client_collateral_not_found_error(id))
    }

    /// Lists all [`ClientCollateralManagement`] records for `client_id`, optionally filtered by `currency_code`
    /// (`ClientCollateralManagementRepositoryWrapper.getClientCollateralData`).
    pub async fn list_client_collaterals(
        &self,
        client_id: i64,
        currency_filter: Option<&str>,
    ) -> FineractResult<Vec<ClientCollateralManagement>> {
        let mut items: Vec<ClientCollateralManagement> = self
            .client_collaterals
            .iter()
            .filter(|e| e.value().client_id == client_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|c| c.id);

        if let Some(ccy) = currency_filter {
            let mut filtered = Vec::new();
            for item in items {
                if let Some(prod) = self.find_collateral_product_by_id(item.collateral_id).await? {
                    if prod.currency_code.eq_ignore_ascii_case(ccy.trim()) {
                        filtered.push(item);
                    }
                }
            }
            return Ok(filtered);
        }
        Ok(items)
    }

    /// Returns enriched [`ClientCollateralManagementData`] items for `client_id`, optionally filtered by currency.
    pub async fn get_client_collateral_data(
        &self,
        client_id: i64,
        currency_filter: Option<&str>,
        mode: FineractRoundingMode,
    ) -> FineractResult<Vec<ClientCollateralManagementData>> {
        let items = self.list_client_collaterals(client_id, currency_filter).await?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let product = self.get_collateral_product(item.collateral_id).await?;
            out.push(item.to_data(&product, Vec::new(), mode));
        }
        Ok(out)
    }

    /// Deletes a [`ClientCollateralManagement`] by ID after verifying that no unreleased
    /// [`LoanCollateralManagement`] pledges exist (`ClientCollateralManagementWriteServiceImpl.validateForDeletion`).
    pub async fn delete_client_collateral(&self, id: i64) -> FineractResult<()> {
        let _cc = self.get_client_collateral(id).await?;
        for entry in self.loan_collateral_pledges.iter() {
            let pledge = entry.value();
            if pledge.client_collateral_id == id && !pledge.is_released {
                return Err(ClientCollateralCannotBeDeletedException::new(id).into_fineract_error());
            }
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::delete_client_collateral_sql(self.dialect);
            sqlx::query(&sql)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.client_collaterals.remove(&id);
        Ok(())
    }

    // --- Loan Collateral Management Operations (`m_loan_collateral_management`) ---

    /// Lists all [`LoanCollateralManagement`] pledges associated with `client_collateral_id`.
    pub async fn list_loan_collateral_managements_by_client_collateral(
        &self,
        client_collateral_id: i64,
    ) -> FineractResult<Vec<LoanCollateralManagement>> {
        let mut items: Vec<LoanCollateralManagement> = self
            .loan_collateral_pledges
            .iter()
            .filter(|e| e.value().client_collateral_id == client_collateral_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|p| p.id);
        Ok(items)
    }

    /// Lists all [`LoanCollateralManagement`] pledges associated with `loan_id`.
    pub async fn list_loan_collateral_managements_by_loan(
        &self,
        loan_id: i64,
    ) -> FineractResult<Vec<LoanCollateralManagement>> {
        let mut items: Vec<LoanCollateralManagement> = self
            .loan_collateral_pledges
            .iter()
            .filter(|e| e.value().loan_id == loan_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|p| p.id);
        Ok(items)
    }

    /// Pledges `requested_quantity` from `client_collateral_id` to `loan_id`, enforcing that
    /// `requested_quantity > 0` and `requested_quantity <= available_quantity`.
    pub async fn pledge_loan_collateral(
        &self,
        client_collateral_id: i64,
        loan_id: i64,
        requested_quantity: Decimal,
        transaction_id: Option<i64>,
    ) -> FineractResult<LoanCollateralManagement> {
        let client_collateral = self.get_client_collateral(client_collateral_id).await?;
        let existing_pledges = self
            .list_loan_collateral_managements_by_client_collateral(client_collateral_id)
            .await?;
        let id = self.allocate_id();
        let pledge = LoanCollateralManagement::pledge(
            id,
            &client_collateral,
            &existing_pledges,
            loan_id,
            requested_quantity,
            transaction_id,
        )?;
        self.save_loan_collateral_management(pledge).await
    }

    /// Persists a [`LoanCollateralManagement`] record.
    pub async fn save_loan_collateral_management(
        &self,
        mut pledge: LoanCollateralManagement,
    ) -> FineractResult<LoanCollateralManagement> {
        if pledge.id <= 0 {
            pledge.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_loan_collateral_management_sql(self.dialect);
            sqlx::query(&sql)
                .bind(pledge.id)
                .bind(pledge.quantity.to_string())
                .bind(pledge.transaction_id)
                .bind(pledge.loan_id)
                .bind(pledge.is_released)
                .bind(pledge.client_collateral_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.loan_collateral_pledges
            .insert(pledge.id, pledge.clone());
        Ok(pledge)
    }

    /// Retrieves a [`LoanCollateralManagement`] by ID, returning [`LoanCollateralManagementNotFoundException`] if absent.
    pub async fn get_loan_collateral_management(
        &self,
        id: i64,
    ) -> FineractResult<LoanCollateralManagement> {
        if let Some(entry) = self.loan_collateral_pledges.get(&id) {
            return Ok(entry.value().clone());
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_loan_collateral_management_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let row_id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let qty_raw: String = row
                    .try_get("quantity")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let transaction_id: Option<i64> = row
                    .try_get("transaction_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let loan_id: i64 = row
                    .try_get("loan_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let is_released: bool = row
                    .try_get("is_released")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let client_collateral_id: i64 = row
                    .try_get("client_collateral_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let quantity = Decimal::from_str(&qty_raw).unwrap_or(Decimal::ZERO);
                let pledge = LoanCollateralManagement::new(
                    row_id,
                    client_collateral_id,
                    loan_id,
                    quantity,
                    is_released,
                    transaction_id,
                );
                self.loan_collateral_pledges.insert(row_id, pledge.clone());
                return Ok(pledge);
            }
        }
        Err(loan_collateral_management_not_found_error(id))
    }

    /// Automatically releases all unreleased [`LoanCollateralManagement`] pledges for `loan_id`
    /// upon loan closure or write-off, returning the updated records.
    pub async fn release_collaterals_for_loan(
        &self,
        loan_id: i64,
    ) -> FineractResult<Vec<LoanCollateralManagement>> {
        if let Some(ref pool) = self.pool {
            let sql = Self::release_loan_collateral_managements_by_loan_sql(self.dialect);
            sqlx::query(&sql)
                .bind(true)
                .bind(loan_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        let mut updated = Vec::new();
        for mut entry in self.loan_collateral_pledges.iter_mut() {
            let pledge = entry.value_mut();
            if pledge.loan_id == loan_id {
                pledge.release_on_loan_closure();
                updated.push(pledge.clone());
            }
        }
        updated.sort_by_key(|p| p.id);
        Ok(updated)
    }

    /// Deletes a [`LoanCollateralManagement`] pledge by ID (`LoanCollateralManagementWriteServiceImpl.deleteLoanCollateral`).
    pub async fn delete_loan_collateral_management(
        &self,
        id: i64,
    ) -> FineractResult<LoanCollateralManagement> {
        let existing = self.get_loan_collateral_management(id).await?;
        if let Some(ref pool) = self.pool {
            let sql = Self::delete_loan_collateral_management_sql(self.dialect);
            sqlx::query(&sql)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.loan_collateral_pledges.remove(&id);
        Ok(existing)
    }

    /// Validates that the total lendable collateral value across active pledges for `loan_id`
    /// is at least `disbursed_amount`, returning [`LoanCollateralAmountNotSufficientException`] if insufficient
    /// (`LoanTransactionValidatorImpl.validateDisbursement`).
    pub async fn validate_loan_collateral_coverage(
        &self,
        loan_id: i64,
        disbursed_amount: Decimal,
        mode: FineractRoundingMode,
    ) -> FineractResult<Decimal> {
        let pledges = self.list_loan_collateral_managements_by_loan(loan_id).await?;
        let mut total_collateral = Decimal::ZERO;
        for pledge in &pledges {
            if !pledge.is_released {
                let cc = self.get_client_collateral(pledge.client_collateral_id).await?;
                let prod = self.get_collateral_product(cc.collateral_id).await?;
                total_collateral += pledge.total_collateral_value(&prod, mode);
            }
        }
        if !pledges.is_empty() && disbursed_amount > total_collateral {
            return Err(
                LoanCollateralAmountNotSufficientException::new(disbursed_amount)
                    .into_fineract_error(),
            );
        }
        Ok(total_collateral)
    }

    // --- Legacy Loan Collateral Operations (`m_loan_collateral`) ---

    /// Saves a new legacy [`LoanCollateral`] record (`m_loan_collateral`).
    pub async fn save_legacy_loan_collateral(
        &self,
        mut collateral: LoanCollateral,
    ) -> FineractResult<LoanCollateral> {
        if collateral.id <= 0 {
            collateral.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_legacy_collateral_sql(self.dialect);
            sqlx::query(&sql)
                .bind(collateral.id)
                .bind(collateral.loan_id)
                .bind(collateral.type_cv_id)
                .bind(collateral.value.to_string())
                .bind(&collateral.description)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.legacy_loan_collaterals
            .insert(collateral.id, collateral.clone());
        Ok(collateral)
    }

    /// Updates an existing legacy [`LoanCollateral`] record.
    pub async fn update_legacy_loan_collateral(
        &self,
        collateral: LoanCollateral,
    ) -> FineractResult<LoanCollateral> {
        if let Some(ref pool) = self.pool {
            let sql = Self::update_legacy_collateral_sql(self.dialect);
            sqlx::query(&sql)
                .bind(collateral.type_cv_id)
                .bind(collateral.value.to_string())
                .bind(&collateral.description)
                .bind(collateral.loan_id)
                .bind(collateral.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.legacy_loan_collaterals
            .insert(collateral.id, collateral.clone());
        Ok(collateral)
    }

    /// Retrieves a legacy [`LoanCollateral`] by `(loan_id, collateral_id)`.
    pub async fn get_legacy_loan_collateral(
        &self,
        loan_id: i64,
        collateral_id: i64,
    ) -> FineractResult<LoanCollateral> {
        if let Some(entry) = self.legacy_loan_collaterals.get(&collateral_id) {
            if entry.value().loan_id == loan_id {
                return Ok(entry.value().clone());
            }
        }
        Err(legacy_loan_collateral_not_found_error(
            loan_id,
            collateral_id,
        ))
    }

    /// Lists all legacy [`LoanCollateral`] records for `loan_id`.
    pub async fn list_legacy_loan_collaterals(
        &self,
        loan_id: i64,
    ) -> FineractResult<Vec<LoanCollateral>> {
        let mut items: Vec<LoanCollateral> = self
            .legacy_loan_collaterals
            .iter()
            .filter(|e| e.value().loan_id == loan_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|c| c.id);
        Ok(items)
    }

    /// Deletes a legacy [`LoanCollateral`] by `(loan_id, collateral_id)`.
    pub async fn delete_legacy_loan_collateral(
        &self,
        loan_id: i64,
        collateral_id: i64,
    ) -> FineractResult<()> {
        let _existing = self.get_legacy_loan_collateral(loan_id, collateral_id).await?;
        if let Some(ref pool) = self.pool {
            let sql = Self::delete_legacy_collateral_sql(self.dialect);
            sqlx::query(&sql)
                .bind(loan_id)
                .bind(collateral_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.legacy_loan_collaterals.remove(&collateral_id);
        Ok(())
    }
}

// =============================================================================
// 8. COMMAND HANDLERS
// =============================================================================

/// Command handler for `CREATE COLLATERAL_PRODUCT`
/// (`org.apache.fineract.portfolio.collateralmanagement.handler.CollateralProductCreateCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateCollateralProductCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

/// Alias matching upstream Java `CollateralProductCreateCommandHandler`.
pub type CollateralProductCreateCommandHandler = CreateCollateralProductCommandHandler;

impl CreateCollateralProductCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "COLLATERAL_PRODUCT";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateCollateralProductCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let id = self.repo.allocate_id();
        let product = CollateralManagementProduct::from_json(id, command)?;
        let saved = self.repo.save_collateral_product(product).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for `UPDATE COLLATERAL_PRODUCT`
/// (`org.apache.fineract.portfolio.collateralmanagement.handler.CollateralProductUpdateCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct UpdateCollateralProductCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

/// Alias matching upstream Java `CollateralProductUpdateCommandHandler`.
pub type CollateralProductUpdateCommandHandler = UpdateCollateralProductCommandHandler;

impl UpdateCollateralProductCommandHandler {
    pub const ACTION_NAME: &'static str = "UPDATE";
    pub const ENTITY_NAME: &'static str = "COLLATERAL_PRODUCT";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateCollateralProductCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let collateral_id = match command.entity_id {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("collateralId")?
                .ok_or_else(|| collateral_product_not_found_error(0))?,
        };
        let mut product = self.repo.get_collateral_product(collateral_id).await?;
        let changes = product.update(command)?;
        if !changes.is_empty() {
            self.repo.update_collateral_product(product.clone()).await?;
        }
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(product.id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `DELETE COLLATERAL_PRODUCT`
/// (`org.apache.fineract.portfolio.collateralmanagement.handler.CollateralProductDeleteCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct DeleteCollateralProductCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

/// Alias matching upstream Java `CollateralProductDeleteCommandHandler`.
pub type CollateralProductDeleteCommandHandler = DeleteCollateralProductCommandHandler;

impl DeleteCollateralProductCommandHandler {
    pub const ACTION_NAME: &'static str = "DELETE";
    pub const ENTITY_NAME: &'static str = "COLLATERAL_PRODUCT";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DeleteCollateralProductCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let collateral_id = match command.entity_id {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("collateralId")?
                .ok_or_else(|| collateral_product_not_found_error(0))?,
        };
        self.repo.delete_collateral_product(collateral_id).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(collateral_id)
            .build())
    }
}

/// Command handler for `CREATE CLIENT_COLLATERAL_PRODUCT`
/// (`org.apache.fineract.portfolio.collateralmanagement.handler.ClientCollateralCreateCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateClientCollateralCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

/// Alias matching upstream Java `ClientCollateralCreateCommandHandler`.
pub type ClientCollateralCreateCommandHandler = CreateClientCollateralCommandHandler;

impl CreateClientCollateralCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "CLIENT_COLLATERAL_PRODUCT";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateClientCollateralCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_id = match command.client_id {
            Some(cid) => cid,
            None => command
                .long_value_of_parameter_named("clientId")?
                .or(command.entity_id)
                .unwrap_or(1),
        };
        let id = self.repo.allocate_id();
        let client_collateral = ClientCollateralManagement::from_json(id, client_id, command)?;
        let saved = self.repo.save_client_collateral(client_collateral).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id(saved.client_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for `UPDATE CLIENT_COLLATERAL_PRODUCT`
/// (`org.apache.fineract.portfolio.collateralmanagement.handler.ClientCollateralUpdateCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct UpdateClientCollateralCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

/// Alias matching upstream Java `ClientCollateralUpdateCommandHandler`.
pub type ClientCollateralUpdateCommandHandler = UpdateClientCollateralCommandHandler;

impl UpdateClientCollateralCommandHandler {
    pub const ACTION_NAME: &'static str = "UPDATE";
    pub const ENTITY_NAME: &'static str = "CLIENT_COLLATERAL_PRODUCT";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateClientCollateralCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_collateral_id = match command.entity_id.or(command.subresource_id) {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("collateralId")?
                .ok_or_else(|| client_collateral_not_found_error(0))?,
        };
        let mut client_collateral = self
            .repo
            .get_client_collateral(client_collateral_id)
            .await?;
        let pledged = self
            .repo
            .list_loan_collateral_managements_by_client_collateral(client_collateral_id)
            .await?;
        let changes = client_collateral.update(command, &pledged)?;
        if !changes.is_empty() {
            self.repo
                .update_client_collateral(client_collateral.clone())
                .await?;
        }
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id(client_collateral.client_id)
            .with_entity_id(client_collateral.id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `DELETE CLIENT_COLLATERAL_PRODUCT`
/// (`org.apache.fineract.portfolio.collateralmanagement.handler.ClientCollateralDeleteCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct DeleteClientCollateralCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

/// Alias matching upstream Java `ClientCollateralDeleteCommandHandler`.
pub type ClientCollateralDeleteCommandHandler = DeleteClientCollateralCommandHandler;

impl DeleteClientCollateralCommandHandler {
    pub const ACTION_NAME: &'static str = "DELETE";
    pub const ENTITY_NAME: &'static str = "CLIENT_COLLATERAL_PRODUCT";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DeleteClientCollateralCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_collateral_id = match command.entity_id.or(command.subresource_id) {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("collateralId")?
                .ok_or_else(|| client_collateral_not_found_error(0))?,
        };
        let existing = self
            .repo
            .get_client_collateral(client_collateral_id)
            .await?;
        self.repo
            .delete_client_collateral(client_collateral_id)
            .await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id(existing.client_id)
            .with_entity_id(client_collateral_id)
            .build())
    }
}

/// Command handler for `DELETE LOAN_COLLATERAL_PRODUCT`
/// (`org.apache.fineract.portfolio.collateralmanagement.handler.LoanCollateralDeleteCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct DeleteLoanCollateralCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

/// Alias matching upstream Java `LoanCollateralDeleteCommandHandler`.
pub type LoanCollateralDeleteCommandHandler = DeleteLoanCollateralCommandHandler;

impl DeleteLoanCollateralCommandHandler {
    pub const ACTION_NAME: &'static str = "DELETE";
    pub const ENTITY_NAME: &'static str = "LOAN_COLLATERAL_PRODUCT";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DeleteLoanCollateralCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let pledge_id = match command.entity_id.or(command.subresource_id) {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("id")?
                .or(command.long_value_of_parameter_named("collateralId")?)
                .ok_or_else(|| loan_collateral_management_not_found_error(0))?,
        };
        let deleted = self
            .repo
            .delete_loan_collateral_management(pledge_id)
            .await?;
        let loan_id = command.loan_id.unwrap_or(deleted.loan_id);
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_loan_id(loan_id)
            .with_entity_id(deleted.id)
            .build())
    }
}

/// Command handler for legacy `CREATE COLLATERAL` (`org.apache.fineract.portfolio.collateral.handler.CreateCollateralCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateCollateralCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

impl CreateCollateralCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "COLLATERAL";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateCollateralCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = match command.loan_id {
            Some(lid) => lid,
            None => command
                .long_value_of_parameter_named("loanId")?
                .unwrap_or(1),
        };
        let id = self.repo.allocate_id();
        let collateral = LoanCollateral::from_json(id, loan_id, command)?;
        let saved = self.repo.save_legacy_loan_collateral(collateral).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_loan_id(saved.loan_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for legacy `UPDATE COLLATERAL` (`org.apache.fineract.portfolio.collateral.handler.UpdateCollateralCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct UpdateCollateralCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

impl UpdateCollateralCommandHandler {
    pub const ACTION_NAME: &'static str = "UPDATE";
    pub const ENTITY_NAME: &'static str = "COLLATERAL";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateCollateralCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = match command.loan_id {
            Some(lid) => lid,
            None => command
                .long_value_of_parameter_named("loanId")?
                .unwrap_or(1),
        };
        let collateral_id = match command.entity_id.or(command.subresource_id) {
            Some(cid) => cid,
            None => command
                .long_value_of_parameter_named("collateralId")?
                .ok_or_else(|| legacy_loan_collateral_not_found_error(loan_id, 0))?,
        };
        let mut collateral = self
            .repo
            .get_legacy_loan_collateral(loan_id, collateral_id)
            .await?;
        let changes = collateral.update(command)?;
        if !changes.is_empty() {
            self.repo
                .update_legacy_loan_collateral(collateral.clone())
                .await?;
        }
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_loan_id(loan_id)
            .with_entity_id(collateral.id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for legacy `DELETE COLLATERAL` (`org.apache.fineract.portfolio.collateral.handler.DeleteCollateralCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct DeleteCollateralCommandHandler {
    repo: Arc<DualDbCollateralRepository>,
}

impl DeleteCollateralCommandHandler {
    pub const ACTION_NAME: &'static str = "DELETE";
    pub const ENTITY_NAME: &'static str = "COLLATERAL";

    pub fn new(repo: Arc<DualDbCollateralRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbCollateralRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbCollateralRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DeleteCollateralCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = match command.loan_id {
            Some(lid) => lid,
            None => command
                .long_value_of_parameter_named("loanId")?
                .unwrap_or(1),
        };
        let collateral_id = match command.entity_id.or(command.subresource_id) {
            Some(cid) => cid,
            None => command
                .long_value_of_parameter_named("collateralId")?
                .ok_or_else(|| legacy_loan_collateral_not_found_error(loan_id, 0))?,
        };
        self.repo
            .delete_legacy_loan_collateral(loan_id, collateral_id)
            .await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_loan_id(loan_id)
            .with_entity_id(collateral_id)
            .build())
    }
}

// =============================================================================
// 9. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_gold_product() -> CollateralManagementProduct {
        CollateralManagementProduct::new(
            1,
            "Gold 24K",
            "999.9 Pure",
            dec!(2500.00),
            "gram",
            dec!(80.00), // 80% LTV / 20% haircut
            "USD",
        )
    }

    fn test_ctx() -> ExecutionContext {
        ExecutionContext::new(
            "default",
            NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid date"),
        )
    }

    #[test]
    fn client_collateral_valuation_formulas_and_haircut_pct_to_base() {
        let product = sample_gold_product();
        let client_collateral = ClientCollateralManagement::new(10, 100, product.id, dec!(12.5));

        // total_value = quantity (12.5) * base_price (2500.00) = 31,250
        let total = client_collateral.total_value(&product, FineractRoundingMode::HalfEven);
        assert_eq!(total, dec!(31250));

        // total_collateral_value = total_value (31,250) * (pct_to_base 80 / 100) = 25,000
        let collateral_val =
            client_collateral.total_collateral_value(&product, FineractRoundingMode::HalfEven);
        assert_eq!(collateral_val, dec!(25000));

        // Zero quantity yields zero valuation
        let empty_collateral =
            ClientCollateralManagement::new(11, 100, product.id, Decimal::ZERO);
        assert_eq!(
            empty_collateral.total_value(&product, FineractRoundingMode::HalfEven),
            Decimal::ZERO
        );
        assert_eq!(
            empty_collateral.total_collateral_value(&product, FineractRoundingMode::HalfEven),
            Decimal::ZERO
        );
    }

    #[test]
    fn client_collateral_valuation_respects_rounding_mode_at_scale_5() {
        let product = CollateralManagementProduct::new(
            2,
            "Agricultural Land",
            "Grade A",
            dec!(10.12345),
            "acre",
            dec!(33.33333),
            "USD",
        );
        let client_collateral = ClientCollateralManagement::new(20, 100, product.id, dec!(3));

        // 3 * 10.12345 = 30.37035
        assert_eq!(
            client_collateral.total_value(&product, FineractRoundingMode::HalfUp),
            dec!(30.37035)
        );

        // 30.37035 * 0.3333333 = 10.12344898855
        // HalfUp -> 10.12345, Down -> 10.12344
        assert_eq!(
            client_collateral.total_collateral_value(&product, FineractRoundingMode::HalfUp),
            dec!(10.12345)
        );
        assert_eq!(
            client_collateral.total_collateral_value(&product, FineractRoundingMode::Down),
            dec!(10.12344)
        );
    }

    #[test]
    fn pledge_validation_rejects_over_pledging_and_non_positive_quantities() {
        let product = sample_gold_product();
        let client_collateral = ClientCollateralManagement::new(10, 100, product.id, dec!(50));
        let mut pledges = Vec::new();

        // Reject 0 or negative quantity
        assert!(LoanCollateralManagement::pledge(
            1,
            &client_collateral,
            &pledges,
            500,
            Decimal::ZERO,
            None,
        )
        .is_err());
        assert!(LoanCollateralManagement::pledge(
            1,
            &client_collateral,
            &pledges,
            500,
            dec!(-5),
            None,
        )
        .is_err());

        // Pledge 30 out of 50 -> succeeds
        let pledge1 = LoanCollateralManagement::pledge(
            1,
            &client_collateral,
            &pledges,
            500,
            dec!(30),
            Some(9001),
        )
        .expect("first pledge of 30 should succeed");
        pledges.push(pledge1);
        assert_eq!(client_collateral.available_quantity(&pledges), dec!(20));

        // Attempt to pledge 25 when only 20 is available -> rejected!
        let over_pledge = LoanCollateralManagement::pledge(
            2,
            &client_collateral,
            &pledges,
            501,
            dec!(25),
            None,
        );
        assert!(over_pledge.is_err());
        let err = over_pledge.unwrap_err();
        assert_eq!(
            err.to_global_error_response().errors[0].user_message_globalisation_code,
            InvalidAmountOfCollateralQuantity::CODE
        );

        // Pledge exact remaining 20 -> succeeds, leaving 0 available
        let pledge2 = LoanCollateralManagement::pledge(
            2,
            &client_collateral,
            &pledges,
            501,
            dec!(20),
            None,
        )
        .expect("second pledge of 20 should succeed");
        pledges.push(pledge2);
        assert_eq!(
            client_collateral.available_quantity(&pledges),
            Decimal::ZERO
        );
    }

    #[test]
    fn automatic_collateral_release_on_loan_closure_and_write_off_restores_available_quantity() {
        let product = sample_gold_product();
        let client_collateral = ClientCollateralManagement::new(10, 100, product.id, dec!(100));
        let mut pledges = Vec::new();

        let p1 = LoanCollateralManagement::pledge(
            1,
            &client_collateral,
            &pledges,
            701,
            dec!(60),
            None,
        )
        .unwrap();
        pledges.push(p1);

        let p2 = LoanCollateralManagement::pledge(
            2,
            &client_collateral,
            &pledges,
            702,
            dec!(35),
            None,
        )
        .unwrap();
        pledges.push(p2);

        // 100 - (60 + 35) = 5 available
        assert_eq!(client_collateral.available_quantity(&pledges), dec!(5));

        // Loan 701 closes -> automatic release of all pledges for loan 701
        let released_count = LoanCollateralManagement::release_all_for_loan(&mut pledges, 701);
        assert_eq!(released_count, 1);
        assert!(pledges[0].is_released);
        assert!(!pledges[1].is_released);

        // Available quantity is restored from 5 to 65
        assert_eq!(client_collateral.available_quantity(&pledges), dec!(65));

        // Loan 702 is written off -> release on write-off
        assert!(pledges[1].release_on_write_off());
        assert!(pledges[1].is_released);
        assert_eq!(client_collateral.available_quantity(&pledges), dec!(100));

        // Re-releasing an already released pledge is idempotent
        assert!(!pledges[1].release_on_loan_closure());
    }

    #[tokio::test]
    async fn repository_and_command_handlers_end_to_end_lifecycle() {
        let repo = Arc::new(DualDbCollateralRepository::in_memory());
        let ctx = test_ctx();

        let create_prod_handler = CreateCollateralProductCommandHandler::new(repo.clone());
        let update_prod_handler = UpdateCollateralProductCommandHandler::new(repo.clone());
        let delete_prod_handler = DeleteCollateralProductCommandHandler::new(repo.clone());
        let create_client_coll_handler = CreateClientCollateralCommandHandler::new(repo.clone());
        let update_client_coll_handler = UpdateClientCollateralCommandHandler::new(repo.clone());
        let delete_loan_coll_handler = DeleteLoanCollateralCommandHandler::new(repo.clone());

        // 1. Create Collateral Product
        let create_prod_cmd = JsonCommand::from_json(
            r#"{
                "name": "COLLATERAL_PRODUCT_GOLD",
                "quality": "24K",
                "basePrice": 1000.0,
                "unitType": "gram",
                "pctToBase": 75.0,
                "currency": "USD",
                "locale": "en"
            }"#,
        )
        .unwrap();
        let prod_res = create_prod_handler
            .handle(&ctx, &create_prod_cmd)
            .await
            .expect("create collateral product should succeed");
        let prod_id = prod_res.resource_id.expect("resource_id present");

        // 2. Update Collateral Product
        let update_prod_cmd = JsonCommand::from_json(
            r#"{
                "basePrice": 1200.0,
                "pctToBase": 80.0,
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_entity_id(prod_id);
        let upd_res = update_prod_handler
            .handle(&ctx, &update_prod_cmd)
            .await
            .expect("update collateral product should succeed");
        assert!(upd_res.has_changes());
        let updated_prod = repo.get_collateral_product(prod_id).await.unwrap();
        assert_eq!(updated_prod.base_price, dec!(1200.0));
        assert_eq!(updated_prod.pct_to_base, dec!(80.0));

        // 3. Create Client Collateral
        let create_cc_cmd = JsonCommand::from_json(&format!(
            r#"{{
                "collateralId": {prod_id},
                "quantity": 50.0,
                "locale": "en"
            }}"#
        ))
        .unwrap()
        .with_client_id(42);
        let cc_res = create_client_coll_handler
            .handle(&ctx, &create_cc_cmd)
            .await
            .expect("create client collateral should succeed");
        let cc_id = cc_res.resource_id.expect("client collateral id");

        // Deleting product while attached to client collateral with quantity > 0 must fail
        let del_prod_cmd = JsonCommand::from_json("{}")
            .unwrap()
            .with_entity_id(prod_id);
        assert!(delete_prod_handler.handle(&ctx, &del_prod_cmd).await.is_err());

        // 4. Pledge 20 units to Loan 999
        let pledge = repo
            .pledge_loan_collateral(cc_id, 999, dec!(20), Some(5001))
            .await
            .expect("pledge 20 units should succeed");
        assert_eq!(pledge.quantity, dec!(20));
        assert!(!pledge.is_released);

        // Attempt to update client collateral quantity below pledged quantity (15 < 20) -> fails validation
        let invalid_upd_cc_cmd = JsonCommand::from_json(r#"{"quantity": 15.0, "locale": "en"}"#)
            .unwrap()
            .with_client_id(42)
            .with_entity_id(cc_id);
        assert!(update_client_coll_handler
            .handle(&ctx, &invalid_upd_cc_cmd)
            .await
            .is_err());

        // Update client collateral quantity to 40 (>= 20) -> succeeds
        let valid_upd_cc_cmd = JsonCommand::from_json(r#"{"quantity": 40.0, "locale": "en"}"#)
            .unwrap()
            .with_client_id(42)
            .with_entity_id(cc_id);
        update_client_coll_handler
            .handle(&ctx, &valid_upd_cc_cmd)
            .await
            .expect("updating client collateral to 40 should succeed");

        // 5. Release collateral on loan closure via repository
        let released = repo
            .release_collaterals_for_loan(999)
            .await
            .expect("release collaterals for loan");
        assert_eq!(released.len(), 1);
        assert!(released[0].is_released);

        // 6. Delete Loan Collateral via command handler
        let del_loan_coll_cmd = JsonCommand::from_json("{}")
            .unwrap()
            .with_loan_id(999)
            .with_entity_id(pledge.id);
        let del_pledge_res = delete_loan_coll_handler
            .handle(&ctx, &del_loan_coll_cmd)
            .await
            .expect("delete loan collateral should succeed");
        assert_eq!(del_pledge_res.resource_id, Some(pledge.id));
        assert_eq!(del_pledge_res.loan_id, Some(999));

        // 7. Delete Client Collateral and then Delete Collateral Product
        repo.delete_client_collateral(cc_id)
            .await
            .expect("delete client collateral should succeed once pledges are removed");
        delete_prod_handler
            .handle(&ctx, &del_prod_cmd)
            .await
            .expect("delete product should succeed once client collateral is removed");
    }

    #[tokio::test]
    async fn legacy_loan_collateral_crud_and_sql_dialect_rewriting() {
        let repo = Arc::new(DualDbCollateralRepository::in_memory());
        let ctx = test_ctx();
        let create_handler = CreateCollateralCommandHandler::new(repo.clone());
        let update_handler = UpdateCollateralCommandHandler::new(repo.clone());
        let delete_handler = DeleteCollateralCommandHandler::new(repo.clone());

        let create_cmd = JsonCommand::from_json(
            r#"{
                "collateralTypeId": 14,
                "value": 15000.50,
                "description": "Toyota Hilux Pickup",
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_loan_id(300);
        let created = create_handler.handle(&ctx, &create_cmd).await.unwrap();
        let cid = created.resource_id.unwrap();

        let update_cmd = JsonCommand::from_json(
            r#"{
                "value": 16500.00,
                "description": "Toyota Hilux 4x4 Pickup",
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_loan_id(300)
        .with_entity_id(cid);
        let updated = update_handler.handle(&ctx, &update_cmd).await.unwrap();
        assert!(updated.has_changes());

        let loaded = repo.get_legacy_loan_collateral(300, cid).await.unwrap();
        assert_eq!(loaded.value, dec!(16500.00));
        assert_eq!(
            loaded.description.as_deref(),
            Some("Toyota Hilux 4x4 Pickup")
        );

        let delete_cmd = JsonCommand::from_json("{}")
            .unwrap()
            .with_loan_id(300)
            .with_entity_id(cid);
        delete_handler.handle(&ctx, &delete_cmd).await.unwrap();
        assert!(repo.get_legacy_loan_collateral(300, cid).await.is_err());

        // Verify PostgreSQL vs MySQL placeholder rewriting
        let pg_sql =
            DualDbCollateralRepository::insert_collateral_product_sql(SqlDialect::Postgres);
        assert!(pg_sql.contains("$1") && pg_sql.contains("$7"));
        let mysql_sql =
            DualDbCollateralRepository::insert_collateral_product_sql(SqlDialect::MySql);
        assert!(mysql_sql.contains("VALUES (?, ?, ?, ?, ?, ?, ?)"));
    }
}
