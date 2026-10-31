//! `fineract-rates-rs` — Floating Rates, Base Lending Rates, and Rate Period Engine.
//!
//! Ported from:
//! - `fineract-core` / `fineract-provider`: `org.apache.fineract.portfolio.rate.*`
//!   (`RateAppliesTo`, `Rate`, `RateData`, `RateDefinitionCommandFromApiJsonDeserializer`,
//!   `RateWriteServiceImpl`, `RateReadServiceImpl`, `CreateRateCommandHandler`, `UpdateRateCommandHandler`)
//! - `fineract-rates`: `org.apache.fineract.portfolio.floatingrates.*`
//!   (`FloatingRate`, `FloatingRatePeriod`, `FloatingRateDTO`, `FloatingRateData`,
//!   `FloatingRatePeriodData`, `FloatingRateCreateValidator`, `FloatingRateUpdateValidator`,
//!   `FloatingRateRatePeriodsChecker`, `FloatingRateWriteServiceImpl`, `FloatingRatesReadServiceImpl`,
//!   `FloatingRateCreateCommandHandler`, `FloatingRateUpdateCommandHandler`)

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AnyPool, Row};

use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{
    DialectSql, ExecutionContext, FineractDateParser, FineractNumberParser, JsonCommand, SqlDialect,
};
use fineract_validation_rs::{
    ApiErrorArg, ApiParameterError, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException,
};

// =============================================================================
// 1. RATE APPLIES TO ENUMERATION (`RateProductAppliesTo`)
// =============================================================================

/// Product category to which a [`Rate`] (`m_rate.product_apply`) applies.
///
/// Ported from `org.apache.fineract.portfolio.rate.domain.RateAppliesTo` and extended
/// for Loans (`1`), Savings (`2`), and Client (`3`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum RateProductAppliesTo {
    #[default]
    Loans = 1,
    Savings = 2,
    Client = 3,
}

/// Alias matching upstream Java `RateAppliesTo`.
pub type RateAppliesTo = RateProductAppliesTo;

impl RateProductAppliesTo {
    /// Constant alias for [`RateProductAppliesTo::Loans`].
    pub const LOANS: Self = Self::Loans;
    /// Java-compatible constant alias for [`RateProductAppliesTo::Loans`].
    pub const LOAN: Self = Self::Loans;
    /// Constant alias for [`RateProductAppliesTo::Savings`].
    pub const SAVINGS: Self = Self::Savings;
    /// Constant alias for [`RateProductAppliesTo::Client`].
    pub const CLIENT: Self = Self::Client;

    /// Returns the `SMALLINT` (`i16`) database representation stored in `m_rate.product_apply`.
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Returns the `i32` integer representation matching `RateAppliesTo.getValue()`.
    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    /// Alias matching `RateAppliesTo.getValue()`.
    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Returns the Fineract globalisation code matching `RateAppliesTo.getCode()`.
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Loans => "rateAppliesTo.loan",
            Self::Savings => "rateAppliesTo.savings",
            Self::Client => "rateAppliesTo.client",
        }
    }

    /// Returns the human-readable option label (`RateEnumerations.rateAppliesTo`).
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Loans => "Loan",
            Self::Savings => "Savings",
            Self::Client => "Client",
        }
    }

    /// Converts an `i16` discriminant (`1`, `2`, `3`) into [`RateProductAppliesTo`].
    #[inline]
    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::Loans),
            2 => Some(Self::Savings),
            3 => Some(Self::Client),
            _ => None,
        }
    }

    /// Converts an `i32` discriminant (`1`, `2`, `3`) into [`RateProductAppliesTo`].
    #[inline]
    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Loans),
            2 => Some(Self::Savings),
            3 => Some(Self::Client),
            _ => None,
        }
    }

    /// Alias for [`RateProductAppliesTo::from_i32`] matching `RateAppliesTo.fromInt`.
    #[inline]
    pub const fn from_int(value: i32) -> Option<Self> {
        Self::from_i32(value)
    }

    /// Returns `true` if `self == RateProductAppliesTo::Loans` (`RateAppliesTo.isLoanRate()`).
    #[inline]
    pub const fn is_loan_rate(self) -> bool {
        matches!(self, Self::Loans)
    }

    /// Returns `true` if `self == RateProductAppliesTo::Savings`.
    #[inline]
    pub const fn is_savings_rate(self) -> bool {
        matches!(self, Self::Savings)
    }

    /// Returns `true` if `self == RateProductAppliesTo::Client`.
    #[inline]
    pub const fn is_client_rate(self) -> bool {
        matches!(self, Self::Client)
    }
}

impl fmt::Display for RateProductAppliesTo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<RateProductAppliesTo> for i16 {
    fn from(value: RateProductAppliesTo) -> Self {
        value.as_i16()
    }
}

impl From<RateProductAppliesTo> for i32 {
    fn from(value: RateProductAppliesTo) -> Self {
        value.as_i32()
    }
}

impl TryFrom<i16> for RateProductAppliesTo {
    type Error = FineractError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::from_i16(value).ok_or_else(|| {
            FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.rate.productApply.is.not.within.expected.range",
                    format!("The parameter `productApply` has an invalid value `{value}`."),
                    "productApply",
                    vec![ApiErrorArg::from(i32::from(value))],
                ),
            )
        })
    }
}

impl TryFrom<i32> for RateProductAppliesTo {
    type Error = FineractError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_i32(value).ok_or_else(|| {
            FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.rate.productApply.is.not.within.expected.range",
                    format!("The parameter `productApply` has an invalid value `{value}`."),
                    "productApply",
                    vec![ApiErrorArg::from(value)],
                ),
            )
        })
    }
}

impl FromStr for RateProductAppliesTo {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if let Ok(code) = trimmed.parse::<i32>() {
            return Self::try_from(code);
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "loan" | "loans" | "rateappliesto.loan" => Ok(Self::Loans),
            "saving" | "savings" | "rateappliesto.savings" => Ok(Self::Savings),
            "client" | "clients" | "rateappliesto.client" => Ok(Self::Client),
            _ => Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.rate.productApply.is.not.within.expected.range",
                    format!("The parameter `productApply` has an invalid value `{trimmed}`."),
                    "productApply",
                    vec![ApiErrorArg::from(trimmed)],
                ),
            )),
        }
    }
}

// =============================================================================
// 2. RATE & RATEDATA (`m_rate`)
// =============================================================================

/// Supported JSON parameters for `CREATE_RATE` and `UPDATE_RATE` commands
/// (`RateDefinitionCommandFromApiJsonDeserializer.java` & `Rate.java`).
pub const RATE_SUPPORTED_PARAMETERS: &[&str] = &[
    "id",
    "name",
    "percentage",
    "productApply",
    "active",
    "approveUser",
    "approveUserId",
    "locale",
    "dateFormat",
];

/// Domain entity representing a Rate in table `m_rate`.
///
/// Ported from `org.apache.fineract.portfolio.rate.domain.Rate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rate {
    pub id: i64,
    pub name: String,
    pub percentage: Decimal,
    pub product_apply: RateProductAppliesTo,
    pub active: bool,
    pub approve_user_id: Option<i64>,
}

/// Read-model DTO representing a Rate from `m_rate`.
///
/// Ported from `org.apache.fineract.portfolio.rate.data.RateData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateData {
    pub id: i64,
    pub name: String,
    pub percentage: Decimal,
    pub product_apply: RateProductAppliesTo,
    pub active: bool,
    pub approve_user_id: Option<i64>,
}

impl RateData {
    /// Creates a new [`RateData`] instance.
    pub fn new(
        id: i64,
        name: impl Into<String>,
        percentage: Decimal,
        product_apply: RateProductAppliesTo,
        active: bool,
        approve_user_id: Option<i64>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            percentage,
            product_apply,
            active,
            approve_user_id,
        }
    }

    /// Factory constructor matching `RateData.instance(...)`.
    pub fn instance(
        id: i64,
        name: impl Into<String>,
        percentage: Decimal,
        product_apply: RateProductAppliesTo,
        active: bool,
    ) -> Self {
        Self::new(id, name, percentage, product_apply, active, None)
    }
}

impl From<Rate> for RateData {
    fn from(rate: Rate) -> Self {
        Self {
            id: rate.id,
            name: rate.name,
            percentage: rate.percentage,
            product_apply: rate.product_apply,
            active: rate.active,
            approve_user_id: rate.approve_user_id,
        }
    }
}

impl From<&Rate> for RateData {
    fn from(rate: &Rate) -> Self {
        rate.clone().into()
    }
}

impl From<RateData> for Rate {
    fn from(data: RateData) -> Self {
        Self {
            id: data.id,
            name: data.name,
            percentage: data.percentage,
            product_apply: data.product_apply,
            active: data.active,
            approve_user_id: data.approve_user_id,
        }
    }
}

impl From<&RateData> for Rate {
    fn from(data: &RateData) -> Self {
        data.clone().into()
    }
}

impl Rate {
    /// Creates a new [`Rate`] domain entity.
    pub fn new(
        id: i64,
        name: impl Into<String>,
        percentage: Decimal,
        product_apply: RateProductAppliesTo,
        active: bool,
        approve_user_id: Option<i64>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            percentage,
            product_apply,
            active,
            approve_user_id,
        }
    }

    /// Converts this [`Rate`] into a [`RateData`] projection.
    pub fn to_data(&self) -> RateData {
        RateData::from(self)
    }

    /// Validates a [`JsonCommand`] for `CREATE_RATE`
    /// (`RateDefinitionCommandFromApiJsonDeserializer.validateForCreate`).
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(RATE_SUPPORTED_PARAMETERS)?;

        let validator = DataValidatorBuilder::for_resource("rate");

        let name = command.string_value_of_parameter_named("name");
        validator
            .reset()
            .parameter("name")
            .value(name.as_deref())
            .not_blank()
            .not_exceeding_length_of(250);

        let percentage = command.big_decimal_value_of_parameter_named("percentage")?;
        validator
            .reset()
            .parameter("percentage")
            .value(percentage)
            .not_null()
            .zero_or_positive_amount();

        let product_apply_str = command.string_value_of_parameter_named("productApply");
        validator
            .reset()
            .parameter("productApply")
            .value(product_apply_str.as_deref())
            .not_blank()
            .not_exceeding_length_of(100);

        validator.into_validation_result()?;

        if let Some(ref raw_apply) = product_apply_str {
            let _ = RateProductAppliesTo::from_str(raw_apply)?;
        }

        Ok(())
    }

    /// Validates a [`JsonCommand`] for `UPDATE_RATE`
    /// (`RateDefinitionCommandFromApiJsonDeserializer.validateForUpdate`).
    pub fn validate_for_update(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(RATE_SUPPORTED_PARAMETERS)?;

        let validator = DataValidatorBuilder::for_resource("rate");

        if command.parameter_exists("name") {
            let name = command.string_value_of_parameter_named("name");
            validator
                .reset()
                .parameter("name")
                .value(name.as_deref())
                .not_blank()
                .not_exceeding_length_of(250);
        }

        if command.parameter_exists("percentage") {
            let percentage = command.big_decimal_value_of_parameter_named("percentage")?;
            validator
                .reset()
                .parameter("percentage")
                .value(percentage)
                .not_null()
                .zero_or_positive_amount();
        }

        if command.parameter_exists("productApply") {
            let product_apply_str = command.string_value_of_parameter_named("productApply");
            validator
                .reset()
                .parameter("productApply")
                .value(product_apply_str.as_deref())
                .not_blank();
        }

        validator.into_validation_result()?;
        Ok(())
    }

    /// Constructs a [`Rate`] from a validated [`JsonCommand`], matching `Rate.fromJson`.
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;

        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let percentage = command
            .big_decimal_value_of_parameter_named("percentage")?
            .unwrap_or(Decimal::ZERO);
        let raw_apply = command
            .string_value_of_parameter_named("productApply")
            .unwrap_or_default();
        let product_apply = RateProductAppliesTo::from_str(&raw_apply)?;
        let active = command.boolean_primitive_value_of_parameter_named("active");
        let approve_user_id = match command.long_value_of_parameter_named("approveUserId")? {
            Some(uid) => Some(uid),
            None => command.long_value_of_parameter_named("approveUser")?,
        };

        Ok(Self {
            id,
            name,
            percentage,
            product_apply,
            active,
            approve_user_id,
        })
    }

    /// Applies an update command to this [`Rate`] and returns the map of changed attributes,
    /// matching `Rate.update(JsonCommand)` in `Rate.java`.
    ///
    /// Attempting to modify `productApply` raises a domain rule violation
    /// (`error.msg.charge.rate.applies.to.update.not.supported`).
    pub fn update(
        &mut self,
        command: &JsonCommand,
    ) -> FineractResult<BTreeMap<String, serde_json::Value>> {
        if command.parameter_exists("productApply") {
            let new_apply =
                if let Ok(Some(int_val)) = command.integer_value_sans_locale_of_parameter_named("productApply") {
                    RateProductAppliesTo::from_i32(int_val)
                } else if let Some(s) = command.string_value_of_parameter_named("productApply") {
                    RateProductAppliesTo::from_str(&s).ok()
                } else {
                    None
                };

            if new_apply != Some(self.product_apply) {
                return Err(FineractError::domain_rule(
                    "error.msg.charge.rate.applies.to.update.not.supported",
                    "Update of Rate applies to is not supported",
                ));
            }
        }

        Self::validate_for_update(command)?;

        let mut actual_changes = BTreeMap::new();

        if command.is_change_in_string_parameter_named("name", Some(&self.name)) {
            if let Some(new_name) = command.string_value_of_parameter_named("name") {
                actual_changes.insert("name".to_string(), Value::String(new_name.clone()));
                self.name = new_name;
            }
        }

        if command.is_change_in_big_decimal_parameter_named("percentage", Some(self.percentage))? {
            if let Some(new_pct) = command.big_decimal_value_of_parameter_named("percentage")? {
                actual_changes.insert("percentage".to_string(), serde_json::json!(new_pct));
                self.percentage = new_pct;
            }
        }

        if command.is_change_in_boolean_parameter_named("active", Some(self.active)) {
            let new_active = command.boolean_primitive_value_of_parameter_named("active");
            actual_changes.insert("active".to_string(), Value::Bool(new_active));
            self.active = new_active;
        }

        if command.is_change_in_long_parameter_named("approveUserId", self.approve_user_id)? {
            let new_user_id = command.long_value_of_parameter_named("approveUserId")?;
            actual_changes.insert(
                "approveUserId".to_string(),
                new_user_id.map_or(Value::Null, Value::from),
            );
            self.approve_user_id = new_user_id;
        } else if command.is_change_in_long_parameter_named("approveUser", self.approve_user_id)? {
            let new_user_id = command.long_value_of_parameter_named("approveUser")?;
            actual_changes.insert(
                "approveUserId".to_string(),
                new_user_id.map_or(Value::Null, Value::from),
            );
            self.approve_user_id = new_user_id;
        }

        Ok(actual_changes)
    }
}

// =============================================================================
// 3. FLOATING RATE PERIOD & FLOATING RATE (`m_floating_rates_periods`, `m_floating_rates`)
// =============================================================================

/// Supported root JSON parameters for `CREATE_FLOATINGRATE` and `UPDATE_FLOATINGRATE`.
pub const FLOATING_RATE_SUPPORTED_PARAMETERS: &[&str] = &[
    "id",
    "name",
    "isBaseLendingRate",
    "is_base_lending_rate",
    "isActive",
    "is_active",
    "ratePeriods",
    "periods",
    "locale",
    "dateFormat",
];

/// Domain entity and DTO representing a single rate period in `m_floating_rates_periods`.
///
/// Ported from `org.apache.fineract.portfolio.floatingrates.domain.FloatingRatePeriod`
/// and `FloatingRatePeriodData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingRatePeriod {
    pub id: i64,
    pub from_date: NaiveDate,
    pub interest_rate: Decimal,
    pub is_differential_to_base_lending_rate: bool,
    pub is_active: bool,
}

/// Alias matching upstream Java `FloatingRatePeriodData`.
pub type FloatingRatePeriodData = FloatingRatePeriod;

impl FloatingRatePeriod {
    /// Creates a new [`FloatingRatePeriod`].
    pub const fn new(
        id: i64,
        from_date: NaiveDate,
        interest_rate: Decimal,
        is_differential_to_base_lending_rate: bool,
        is_active: bool,
    ) -> Self {
        Self {
            id,
            from_date,
            interest_rate,
            is_differential_to_base_lending_rate,
            is_active,
        }
    }

    /// Convenience constructor for an active [`FloatingRatePeriod`] (`is_active = true`).
    pub const fn active(
        from_date: NaiveDate,
        interest_rate: Decimal,
        is_differential_to_base_lending_rate: bool,
    ) -> Self {
        Self {
            id: 0,
            from_date,
            interest_rate,
            is_differential_to_base_lending_rate,
            is_active: true,
        }
    }

    /// Returns `from_date` (`FloatingRatePeriod.fetchFromDate()`).
    #[inline]
    pub const fn fetch_from_date(&self) -> NaiveDate {
        self.from_date
    }

    /// Updates the `is_active` flag (`FloatingRatePeriod.setActive`).
    #[inline]
    pub fn set_active(&mut self, is_active: bool) {
        self.is_active = is_active;
    }

    /// Evaluates this period into an effective rate period by adding `interest_rate_diff` (`factor`)
    /// and, if `is_differential_to_base_lending_rate` is `true`, adding the Base Lending Rate
    /// applicable on `self.from_date` (matching `FloatingRatePeriod.toData(FloatingRateDTO)`).
    pub fn to_effective_period(
        &self,
        base_lending_rate: Option<&FloatingRate>,
        interest_rate_diff: Decimal,
    ) -> FineractResult<FloatingRatePeriod> {
        let mut effective_rate = self.interest_rate + interest_rate_diff;
        if self.is_differential_to_base_lending_rate {
            let blr = base_lending_rate.ok_or_else(|| {
                FineractError::domain_rule(
                    "error.msg.floatingrate.base.lending.rate.not.found",
                    "Base Lending Rate does not exist",
                )
            })?;
            let base_rate = blr.fetch_applicable_rate(self.from_date, None, Decimal::ZERO)?;
            effective_rate += base_rate;
        }
        Ok(FloatingRatePeriod {
            id: self.id,
            from_date: self.from_date,
            interest_rate: effective_rate,
            is_differential_to_base_lending_rate: self.is_differential_to_base_lending_rate,
            is_active: self.is_active,
        })
    }
}

/// Domain entity representing a Floating Rate schedule in `m_floating_rates`.
///
/// Ported from `org.apache.fineract.portfolio.floatingrates.domain.FloatingRate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingRate {
    pub id: i64,
    pub name: String,
    pub is_base_lending_rate: bool,
    pub is_active: bool,
    pub periods: Vec<FloatingRatePeriod>,
}

/// Alias matching upstream Java `FloatingRateData`.
pub type FloatingRateData = FloatingRate;

impl FloatingRate {
    /// Creates a new [`FloatingRate`] and sorts `periods` ascending by `(from_date, id)`
    /// (matching `@OrderBy("fromDate,id")` in `FloatingRate.java`).
    pub fn new(
        id: i64,
        name: impl Into<String>,
        is_base_lending_rate: bool,
        is_active: bool,
        mut periods: Vec<FloatingRatePeriod>,
    ) -> Self {
        periods.sort_by_key(|p| (p.from_date, p.id));
        Self {
            id,
            name: name.into(),
            is_base_lending_rate,
            is_active,
            periods,
        }
    }

    /// Finds the active [`FloatingRatePeriod`] applicable on `on_date`
    /// (the active period with the latest `from_date <= on_date`).
    pub fn find_applicable_period(&self, on_date: NaiveDate) -> Option<&FloatingRatePeriod> {
        self.periods
            .iter()
            .filter(|p| p.is_active && p.from_date <= on_date)
            .max_by_key(|p| (p.from_date, p.id))
    }

    /// Finds the raw `interest_rate` of the active period applicable on `on_date`
    /// (matching `FloatingRateDTO.fetchBaseRate` / `findInterestRate`).
    pub fn find_interest_rate(&self, on_date: NaiveDate) -> Option<Decimal> {
        self.find_applicable_period(on_date).map(|p| p.interest_rate)
    }

    /// Computes the effective interest rate applicable on `on_date` by:
    /// 1. Verifying that this floating rate is active.
    /// 2. Selecting the latest active [`FloatingRatePeriod`] whose `from_date <= on_date`.
    /// 3. Adding `factor` (loan/product differential offset).
    /// 4. When `period.is_differential_to_base_lending_rate` is `true`, resolving the active
    ///    `base_lending_rate` on `on_date` and adding its rate to the result.
    ///
    /// Ported from `FloatingRate.java`, `FloatingRatePeriod.java`, and `FloatingRateDTO.java`.
    pub fn fetch_applicable_rate(
        &self,
        on_date: NaiveDate,
        base_lending_rate: Option<&FloatingRate>,
        factor: Decimal,
    ) -> Result<Decimal, FineractError> {
        if !self.is_active {
            return Err(FineractError::domain_rule(
                "error.msg.floatingrate.inactive",
                format!("Floating Rate `{}` is not active.", self.name),
            ));
        }

        let period = self.find_applicable_period(on_date).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.floatingrate.period.not.found",
                format!(
                    "No active floating rate period found for `{}` on or before {on_date}.",
                    self.name
                ),
            )
        })?;

        let mut effective_rate = period.interest_rate + factor;

        if period.is_differential_to_base_lending_rate {
            if self.is_base_lending_rate {
                return Err(FineractError::domain_rule(
                    "org.apache.fineract.portfolio.floatingrate.differential-to-base-lending-rate.invalid-for-base-lending-rate",
                    "Base Lending Rate cannot have differential rate periods.",
                ));
            }

            let blr = base_lending_rate.ok_or_else(|| {
                FineractError::domain_rule(
                    "error.msg.floatingrate.base.lending.rate.not.found",
                    "Base Lending Rate is required for differential floating rate calculation.",
                )
            })?;

            if !blr.is_base_lending_rate {
                return Err(FineractError::domain_rule(
                    "error.msg.floatingrate.invalid.base.lending.rate",
                    format!(
                        "Floating Rate `{}` is not configured as a Base Lending Rate.",
                        blr.name
                    ),
                ));
            }

            if !blr.is_active {
                return Err(FineractError::domain_rule(
                    "error.msg.floatingrate.base.lending.rate.inactive",
                    format!("Base Lending Rate `{}` is not active.", blr.name),
                ));
            }

            let base_rate = blr.fetch_applicable_rate(on_date, None, Decimal::ZERO)?;
            effective_rate += base_rate;
        }

        Ok(effective_rate)
    }

    /// Generates the sequence of effective [`FloatingRatePeriod`] items starting from `start_date`,
    /// matching `FloatingRate.fetchInterestRates(FloatingRateDTO)` in `FloatingRate.java`.
    pub fn fetch_interest_rates(
        &self,
        start_date: NaiveDate,
        is_floating_interest_rate: bool,
        interest_rate_diff: Decimal,
        base_lending_rate: Option<&FloatingRate>,
    ) -> FineractResult<Vec<FloatingRatePeriod>> {
        let mut sorted_periods = self.periods.clone();
        sorted_periods.sort_by_key(|p| (p.from_date, p.id));

        let mut applicable_rates = Vec::new();
        let mut previous_period: Option<&FloatingRatePeriod> = None;
        let mut add_period_data = false;

        for period in &sorted_periods {
            if period.is_active {
                if applicable_rates.is_empty() && start_date < period.fetch_from_date() {
                    if is_floating_interest_rate {
                        add_period_data = true;
                    }
                    if let Some(prev) = previous_period {
                        applicable_rates
                            .push(prev.to_effective_period(base_lending_rate, interest_rate_diff)?);
                    } else if !add_period_data {
                        applicable_rates.push(
                            period.to_effective_period(base_lending_rate, interest_rate_diff)?,
                        );
                    }
                }
                if add_period_data {
                    applicable_rates
                        .push(period.to_effective_period(base_lending_rate, interest_rate_diff)?);
                }
                previous_period = Some(period);
            }
        }

        if applicable_rates.is_empty() {
            if let Some(prev) = previous_period {
                applicable_rates
                    .push(prev.to_effective_period(base_lending_rate, interest_rate_diff)?);
            }
        }

        Ok(applicable_rates)
    }

    /// Validates this [`FloatingRate`] and its [`FloatingRatePeriod`] items:
    /// - `name` must be non-blank and at most 200 characters.
    /// - `periods` must not be empty.
    /// - No period may have a negative `interest_rate` (`< 0`).
    /// - No two periods (with the same active status) may have duplicate `from_date` values.
    /// - A Base Lending Rate (`is_base_lending_rate == true`) cannot have any period with
    ///   `is_differential_to_base_lending_rate == true`.
    ///
    /// Ported from `FloatingRateCreateValidator`, `FloatingRateUpdateValidator`, and
    /// `FloatingRateRatePeriodsChecker`.
    pub fn validate_floating_rate(&self) -> Result<(), FineractError> {
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            errors.push(ApiParameterError::parameter_error(
                "org.apache.fineract.portfolio.floatingrate.name.not-blank",
                "The parameter `name` is mandatory and cannot be blank.",
                "name",
                vec![],
            ));
        } else if self.name.chars().count() > 200 {
            errors.push(ApiParameterError::parameter_error(
                "org.apache.fineract.portfolio.floatingrate.name.max",
                "The parameter `name` exceeds max length of 200 characters.",
                "name",
                vec![ApiErrorArg::from(200_i32)],
            ));
        }

        if self.periods.is_empty() {
            errors.push(ApiParameterError::parameter_error(
                "org.apache.fineract.portfolio.floatingrate.rate-periods.not-empty",
                "The parameter `ratePeriods` cannot be empty.",
                "ratePeriods",
                vec![],
            ));
        }

        let mut seen_dates: HashSet<(NaiveDate, bool)> = HashSet::with_capacity(self.periods.len());
        let mut duplicate_from_date = false;

        for period in &self.periods {
            if period.interest_rate < Decimal::ZERO {
                errors.push(ApiParameterError::parameter_error(
                    "org.apache.fineract.portfolio.floatingrate.interest-rate.positive-or-zero",
                    "The parameter `interestRate` must be greater than or equal to 0.",
                    "interestRate",
                    vec![ApiErrorArg::from(period.interest_rate)],
                ));
            }

            if self.is_base_lending_rate && period.is_differential_to_base_lending_rate {
                errors.push(ApiParameterError::parameter_error(
                    "org.apache.fineract.portfolio.floatingrate.differential-to-base-lending-rate.invalid-for-base-lending-rate",
                    "Base Lending Rate cannot have periods with `isDifferentialToBaseLendingRate` set to true.",
                    "isDifferentialToBaseLendingRate",
                    vec![],
                ));
            }

            if !seen_dates.insert((period.from_date, period.is_active)) {
                duplicate_from_date = true;
            }
        }

        if duplicate_from_date {
            errors.push(ApiParameterError::parameter_error(
                "org.apache.fineract.portfolio.floatingrate.from-date.duplicate",
                "Attempt to add multiple floating rate periods with same fromDate.",
                "ratePeriods",
                vec![],
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(errors),
            ))
        }
    }

    /// Validates rate periods against `business_date` (`from_date > business_date`) and
    /// `base_lending_rate_available`, matching `FloatingRateRatePeriodsChecker.checkRatePeriods`.
    pub fn validate_for_business_date(
        &self,
        business_date: NaiveDate,
        base_lending_rate_available: bool,
    ) -> Result<(), FineractError> {
        self.validate_floating_rate()?;

        let mut errors = Vec::new();
        for period in &self.periods {
            if period.is_active && period.from_date <= business_date {
                errors.push(ApiParameterError::parameter_error(
                    "org.apache.fineract.portfolio.floatingrate.from-date.future",
                    "The parameter `fromDate` must be in the future relative to business date.",
                    "fromDate",
                    vec![ApiErrorArg::from_date(period.from_date)],
                ));
            }
            if period.is_differential_to_base_lending_rate && !base_lending_rate_available {
                errors.push(ApiParameterError::parameter_error(
                    "org.apache.fineract.portfolio.floatingrate.differential-to-base-lending-rate.not-available",
                    "Base Lending Rate is not available for differential rate period.",
                    "isDifferentialToBaseLendingRate",
                    vec![],
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(errors),
            ))
        }
    }

    /// Replaces future rate periods (`from_date > business_date`) by marking them inactive
    /// and appending `new_rate_periods`, matching `FloatingRate.replaceRatePeriods` in `FloatingRate.java`.
    pub fn replace_rate_periods(
        &mut self,
        business_date: NaiveDate,
        new_rate_periods: Vec<FloatingRatePeriod>,
    ) {
        let incoming_dates: HashSet<NaiveDate> =
            new_rate_periods.iter().map(|p| p.from_date).collect();
        for existing in &mut self.periods {
            if existing.from_date > business_date || incoming_dates.contains(&existing.from_date) {
                existing.set_active(false);
            }
        }
        for mut new_period in new_rate_periods {
            new_period.set_active(true);
            self.periods.push(new_period);
        }
        self.periods.sort_by_key(|p| (p.from_date, p.id));
    }

    /// Returns `true` if this floating rate is active and has at least one active period
    /// marked `is_differential_to_base_lending_rate == true`.
    pub fn is_linked_to_base_lending_rate(&self) -> bool {
        self.is_active
            && !self.is_base_lending_rate
            && self
                .periods
                .iter()
                .any(|p| p.is_active && p.is_differential_to_base_lending_rate)
    }

    /// Parses a [`FloatingRate`] from a [`JsonCommand`] (`FloatingRateCreateRequest`).
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        command.check_for_unsupported_parameters(FLOATING_RATE_SUPPORTED_PARAMETERS)?;

        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let is_base_lending_rate = command
            .boolean_object_value_of_parameter_named("isBaseLendingRate")
            .or_else(|| command.boolean_object_value_of_parameter_named("is_base_lending_rate"))
            .unwrap_or(false);
        let is_active = command
            .boolean_object_value_of_parameter_named("isActive")
            .or_else(|| command.boolean_object_value_of_parameter_named("is_active"))
            .unwrap_or(true);

        let periods = Self::parse_rate_periods_from_command(command)?;
        let rate = Self::new(id, name, is_base_lending_rate, is_active, periods);
        rate.validate_floating_rate()?;
        Ok(rate)
    }

    /// Parses the `"ratePeriods"` (or `"periods"`) JSON array from a [`JsonCommand`],
    /// supporting per-period and command-level `"locale"` and `"dateFormat"` attributes
    /// as well as ISO-8601 `yyyy-MM-dd` dates (`FloatingRatePeriodRequest.fromDateAsLocalDate`).
    pub fn parse_rate_periods_from_command(
        command: &JsonCommand,
    ) -> FineractResult<Vec<FloatingRatePeriod>> {
        let periods_val = command
            .json_element("ratePeriods")
            .or_else(|| command.json_element("periods"));

        let Some(Value::Array(arr)) = periods_val else {
            return Ok(Vec::new());
        };

        let root_locale = command.locale();
        let root_date_format = command.date_format();

        let mut periods = Vec::with_capacity(arr.len());
        for (idx, item) in arr.iter().enumerate() {
            let Some(obj) = item.as_object() else {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "org.apache.fineract.portfolio.floatingrate.rate-periods.invalid",
                        format!("ratePeriods[{idx}] must be a JSON object."),
                        "ratePeriods",
                        vec![],
                    ),
                ));
            };

            let item_locale = obj
                .get("locale")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    let trimmed = root_locale.trim();
                    if trimmed.is_empty() {
                        "en_US"
                    } else {
                        trimmed
                    }
                });

            let item_date_format = obj
                .get("dateFormat")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| root_date_format.trim());

            let from_date_val = obj
                .get("fromDate")
                .or_else(|| obj.get("from_date"))
                .ok_or_else(|| {
                    FineractError::DataValidation(
                        PlatformApiDataValidationException::single_parameter_error(
                            "org.apache.fineract.portfolio.floatingrate.from-date.not-blank",
                            "The parameter `fromDate` cannot be blank.",
                            "fromDate",
                            vec![],
                        ),
                    )
                })?;

            let from_date = match from_date_val {
                Value::Array(date_arr) => FineractDateParser::parse_date_array(date_arr, "fromDate")?,
                Value::String(raw_date) => {
                    let trimmed = raw_date.trim();
                    if trimmed.is_empty() {
                        return Err(FineractError::DataValidation(
                            PlatformApiDataValidationException::single_parameter_error(
                                "org.apache.fineract.portfolio.floatingrate.from-date.not-blank",
                                "The parameter `fromDate` cannot be blank.",
                                "fromDate",
                                vec![],
                            ),
                        ));
                    }
                    if item_date_format.is_empty() {
                        NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|_| {
                            FineractError::DataValidation(
                                PlatformApiDataValidationException::single_parameter_error(
                                    "org.apache.fineract.portfolio.floatingrate.from-date.invalid",
                                    format!("The parameter `fromDate` value `{trimmed}` is not a valid ISO date."),
                                    "fromDate",
                                    vec![ApiErrorArg::from(trimmed)],
                                ),
                            )
                        })?
                    } else {
                        FineractDateParser::parse_date_for_param(
                            "fromDate",
                            trimmed,
                            item_date_format,
                            item_locale,
                        )?
                    }
                }
                _ => {
                    return Err(FineractError::DataValidation(
                        PlatformApiDataValidationException::single_parameter_error(
                            "org.apache.fineract.portfolio.floatingrate.from-date.invalid",
                            "The parameter `fromDate` is invalid.",
                            "fromDate",
                            vec![],
                        ),
                    ));
                }
            };

            let interest_rate_val = obj
                .get("interestRate")
                .or_else(|| obj.get("interest_rate"))
                .ok_or_else(|| {
                    FineractError::DataValidation(
                        PlatformApiDataValidationException::single_parameter_error(
                            "org.apache.fineract.portfolio.floatingrate.interest-rate.not-null",
                            "The parameter `interestRate` cannot be null.",
                            "interestRate",
                            vec![],
                        ),
                    )
                })?;

            let interest_rate = FineractNumberParser::parse_decimal_for_param(
                "interestRate",
                interest_rate_val,
                item_locale,
            )?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "org.apache.fineract.portfolio.floatingrate.interest-rate.not-null",
                        "The parameter `interestRate` cannot be null.",
                        "interestRate",
                        vec![],
                    ),
                )
            })?;

            let is_differential = obj
                .get("isDifferentialToBaseLendingRate")
                .or_else(|| obj.get("is_differential_to_base_lending_rate"))
                .and_then(|v| match v {
                    Value::Bool(b) => Some(*b),
                    Value::String(s) => s.trim().parse::<bool>().ok(),
                    _ => None,
                })
                .unwrap_or(false);

            let is_active = obj
                .get("isActive")
                .or_else(|| obj.get("is_active"))
                .and_then(|v| match v {
                    Value::Bool(b) => Some(*b),
                    Value::String(s) => s.trim().parse::<bool>().ok(),
                    _ => None,
                })
                .unwrap_or(true);

            let period_id = obj
                .get("id")
                .and_then(Value::as_i64)
                .unwrap_or((idx as i64) + 1);

            periods.push(FloatingRatePeriod::new(
                period_id,
                from_date,
                interest_rate,
                is_differential,
                is_active,
            ));
        }

        Ok(periods)
    }
}

// =============================================================================
// 4. REPOSITORY CONTRACT, IN-MEMORY REPOSITORY & DUAL-DB REPOSITORY
// =============================================================================

/// Persistence contract for `m_rate`, `m_floating_rates`, and `m_floating_rates_periods`.
#[async_trait]
pub trait RatesRepository: Send + Sync {
    /// Inserts a new [`Rate`] into `m_rate`.
    async fn create_rate(&self, rate: Rate) -> FineractResult<Rate>;

    /// Updates an existing [`Rate`] in `m_rate`.
    async fn update_rate(&self, rate: Rate) -> FineractResult<Rate>;

    /// Finds a [`Rate`] by primary key `id`.
    async fn find_rate_by_id(&self, id: i64) -> FineractResult<Option<Rate>>;

    /// Retrieves a [`Rate`] by `id` or returns `FineractError::ResourceNotFound`
    /// (`error.msg.rate.id.invalid`, matching `RateNotFoundException`).
    async fn get_rate_by_id(&self, id: i64) -> FineractResult<Rate> {
        self.find_rate_by_id(id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.rate.id.invalid",
                format!("Rate with identifier {id} does not exist"),
                id.to_string(),
            )
        })
    }

    /// Finds a [`Rate`] by unique `name`.
    async fn find_rate_by_name(&self, name: &str) -> FineractResult<Option<Rate>>;

    /// Lists all [`Rate`] records ordered by `id`.
    async fn list_rates(&self) -> FineractResult<Vec<Rate>>;

    /// Lists all active [`Rate`] records applicable to loans (`RateProductAppliesTo::Loans`).
    async fn list_loan_applicable_rates(&self) -> FineractResult<Vec<Rate>>;

    /// Inserts a new [`FloatingRate`] and its [`FloatingRatePeriod`] children into
    /// `m_floating_rates` and `m_floating_rates_periods`.
    async fn create_floating_rate(&self, floating_rate: FloatingRate) -> FineractResult<FloatingRate>;

    /// Updates an existing [`FloatingRate`] and its [`FloatingRatePeriod`] children.
    async fn update_floating_rate(&self, floating_rate: FloatingRate) -> FineractResult<FloatingRate>;

    /// Finds a [`FloatingRate`] (with its periods) by primary key `id`.
    async fn find_floating_rate_by_id(&self, id: i64) -> FineractResult<Option<FloatingRate>>;

    /// Retrieves a [`FloatingRate`] by `id` or returns `FineractError::ResourceNotFound`
    /// (`error.msg.floatingrate.id.invalid`, matching `FloatingRateNotFoundException`).
    async fn get_floating_rate_by_id(&self, id: i64) -> FineractResult<FloatingRate> {
        self.find_floating_rate_by_id(id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.floatingrate.id.invalid",
                format!("Floating Rate with identifier {id} does not exist"),
                id.to_string(),
            )
        })
    }

    /// Finds a [`FloatingRate`] by unique `name`.
    async fn find_floating_rate_by_name(&self, name: &str) -> FineractResult<Option<FloatingRate>>;

    /// Finds the active Base Lending Rate (`is_base_lending_rate = true AND is_active = true`),
    /// matching `FloatingRateRepository.retrieveBaseLendingRate()`.
    async fn find_base_lending_rate(&self) -> FineractResult<Option<FloatingRate>>;

    /// Retrieves the active Base Lending Rate or returns `FineractError::ResourceNotFound`
    /// (`error.msg.floatingrate.base.lending.rate.not.found`).
    async fn get_base_lending_rate(&self) -> FineractResult<FloatingRate> {
        self.find_base_lending_rate().await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.floatingrate.base.lending.rate.not.found",
                "Floating Rate does not exist",
                "baseLendingRate",
            )
        })
    }

    /// Lists all [`FloatingRate`] records ordered by `id`.
    async fn list_floating_rates(&self) -> FineractResult<Vec<FloatingRate>>;

    /// Lists all active [`FloatingRate`] records (`is_active = true`).
    async fn list_active_floating_rates(&self) -> FineractResult<Vec<FloatingRate>>;

    /// Lists all active [`FloatingRate`] records that have at least one active period with
    /// `is_differential_to_base_lending_rate = true`
    /// (`FloatingRateRepository.retrieveFloatingRatesLinkedToBLR()`).
    async fn list_floating_rates_linked_to_blr(&self) -> FineractResult<Vec<FloatingRate>>;
}

/// Thread-safe in-memory implementation of [`RatesRepository`] backed by [`DashMap`].
#[derive(Debug)]
pub struct InMemoryRatesRepository {
    rates: DashMap<i64, Rate>,
    floating_rates: DashMap<i64, FloatingRate>,
    next_rate_id: AtomicI64,
    next_floating_rate_id: AtomicI64,
    next_period_id: AtomicI64,
}

impl Default for InMemoryRatesRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryRatesRepository {
    /// Creates an empty [`InMemoryRatesRepository`].
    pub fn new() -> Self {
        Self {
            rates: DashMap::new(),
            floating_rates: DashMap::new(),
            next_rate_id: AtomicI64::new(1),
            next_floating_rate_id: AtomicI64::new(1),
            next_period_id: AtomicI64::new(1),
        }
    }

    /// Returns the number of stored [`Rate`] entities.
    pub fn rate_count(&self) -> usize {
        self.rates.len()
    }

    /// Returns the number of stored [`FloatingRate`] entities.
    pub fn floating_rate_count(&self) -> usize {
        self.floating_rates.len()
    }
}

#[async_trait]
impl RatesRepository for InMemoryRatesRepository {
    async fn create_rate(&self, mut rate: Rate) -> FineractResult<Rate> {
        if self
            .rates
            .iter()
            .any(|entry| entry.value().name.eq_ignore_ascii_case(&rate.name))
        {
            return Err(FineractError::domain_rule(
                "error.msg.fund.duplicate.externalId",
                format!("A rate with name '{}' already exists", rate.name),
            ));
        }

        if rate.id <= 0 {
            rate.id = self.next_rate_id.fetch_add(1, Ordering::SeqCst);
        } else {
            let _ = self
                .next_rate_id
                .fetch_max(rate.id.saturating_add(1), Ordering::SeqCst);
        }

        self.rates.insert(rate.id, rate.clone());
        Ok(rate)
    }

    async fn update_rate(&self, rate: Rate) -> FineractResult<Rate> {
        if !self.rates.contains_key(&rate.id) {
            return Err(FineractError::resource_not_found(
                "error.msg.rate.id.invalid",
                format!("Rate with identifier {} does not exist", rate.id),
                rate.id.to_string(),
            ));
        }

        if self.rates.iter().any(|entry| {
            entry.key() != &rate.id && entry.value().name.eq_ignore_ascii_case(&rate.name)
        }) {
            return Err(FineractError::domain_rule(
                "error.msg.fund.duplicate.externalId",
                format!("A rate with name '{}' already exists", rate.name),
            ));
        }

        self.rates.insert(rate.id, rate.clone());
        Ok(rate)
    }

    async fn find_rate_by_id(&self, id: i64) -> FineractResult<Option<Rate>> {
        Ok(self.rates.get(&id).map(|r| r.value().clone()))
    }

    async fn find_rate_by_name(&self, name: &str) -> FineractResult<Option<Rate>> {
        Ok(self
            .rates
            .iter()
            .find(|entry| entry.value().name == name)
            .map(|entry| entry.value().clone()))
    }

    async fn list_rates(&self) -> FineractResult<Vec<Rate>> {
        let mut items: Vec<Rate> = self.rates.iter().map(|e| e.value().clone()).collect();
        items.sort_by_key(|r| r.id);
        Ok(items)
    }

    async fn list_loan_applicable_rates(&self) -> FineractResult<Vec<Rate>> {
        let mut items: Vec<Rate> = self
            .rates
            .iter()
            .filter(|e| e.value().active && e.value().product_apply == RateProductAppliesTo::Loans)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|r| r.id);
        Ok(items)
    }

    async fn create_floating_rate(
        &self,
        mut floating_rate: FloatingRate,
    ) -> FineractResult<FloatingRate> {
        floating_rate.validate_floating_rate()?;

        if self
            .floating_rates
            .iter()
            .any(|e| e.value().name.eq_ignore_ascii_case(&floating_rate.name))
        {
            return Err(FineractError::domain_rule(
                "error.msg.floatingrates.duplicate.name",
                format!(
                    "Floating Rate with name `{}` already exists",
                    floating_rate.name
                ),
            ));
        }

        if floating_rate.id <= 0 {
            floating_rate.id = self.next_floating_rate_id.fetch_add(1, Ordering::SeqCst);
        } else {
            let _ = self
                .next_floating_rate_id
                .fetch_max(floating_rate.id.saturating_add(1), Ordering::SeqCst);
        }

        for period in &mut floating_rate.periods {
            if period.id <= 0 {
                period.id = self.next_period_id.fetch_add(1, Ordering::SeqCst);
            } else {
                let _ = self
                    .next_period_id
                    .fetch_max(period.id.saturating_add(1), Ordering::SeqCst);
            }
        }
        floating_rate.periods.sort_by_key(|p| (p.from_date, p.id));

        self.floating_rates
            .insert(floating_rate.id, floating_rate.clone());
        Ok(floating_rate)
    }

    async fn update_floating_rate(
        &self,
        mut floating_rate: FloatingRate,
    ) -> FineractResult<FloatingRate> {
        floating_rate.validate_floating_rate()?;

        if !self.floating_rates.contains_key(&floating_rate.id) {
            return Err(FineractError::resource_not_found(
                "error.msg.floatingrate.id.invalid",
                format!(
                    "Floating Rate with identifier {} does not exist",
                    floating_rate.id
                ),
                floating_rate.id.to_string(),
            ));
        }

        if self.floating_rates.iter().any(|e| {
            e.key() != &floating_rate.id
                && e.value().name.eq_ignore_ascii_case(&floating_rate.name)
        }) {
            return Err(FineractError::domain_rule(
                "error.msg.floatingrates.duplicate.name",
                format!(
                    "Floating Rate with name `{}` already exists",
                    floating_rate.name
                ),
            ));
        }

        for period in &mut floating_rate.periods {
            if period.id <= 0 {
                period.id = self.next_period_id.fetch_add(1, Ordering::SeqCst);
            }
        }
        floating_rate.periods.sort_by_key(|p| (p.from_date, p.id));

        self.floating_rates
            .insert(floating_rate.id, floating_rate.clone());
        Ok(floating_rate)
    }

    async fn find_floating_rate_by_id(&self, id: i64) -> FineractResult<Option<FloatingRate>> {
        Ok(self.floating_rates.get(&id).map(|e| e.value().clone()))
    }

    async fn find_floating_rate_by_name(&self, name: &str) -> FineractResult<Option<FloatingRate>> {
        Ok(self
            .floating_rates
            .iter()
            .find(|e| e.value().name == name)
            .map(|e| e.value().clone()))
    }

    async fn find_base_lending_rate(&self) -> FineractResult<Option<FloatingRate>> {
        Ok(self
            .floating_rates
            .iter()
            .find(|e| e.value().is_base_lending_rate && e.value().is_active)
            .map(|e| e.value().clone()))
    }

    async fn list_floating_rates(&self) -> FineractResult<Vec<FloatingRate>> {
        let mut items: Vec<FloatingRate> =
            self.floating_rates.iter().map(|e| e.value().clone()).collect();
        items.sort_by_key(|r| r.id);
        Ok(items)
    }

    async fn list_active_floating_rates(&self) -> FineractResult<Vec<FloatingRate>> {
        let mut items: Vec<FloatingRate> = self
            .floating_rates
            .iter()
            .filter(|e| e.value().is_active)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|r| r.id);
        Ok(items)
    }

    async fn list_floating_rates_linked_to_blr(&self) -> FineractResult<Vec<FloatingRate>> {
        let mut items: Vec<FloatingRate> = self
            .floating_rates
            .iter()
            .filter(|e| e.value().is_linked_to_base_lending_rate())
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|r| r.id);
        Ok(items)
    }
}

/// Dual-Database (`PostgreSQL 16+` & `MySQL / MariaDB`) SQLx repository for
/// `m_rate`, `m_floating_rates`, and `m_floating_rates_periods`.
///
/// Uses [`DialectSql::rewrite_placeholders`] to translate positional `?` placeholders
/// into `$1, $2, ...` when running on PostgreSQL while preserving `?` on MySQL/MariaDB.
#[derive(Debug, Clone)]
pub struct DualDbRatesRepository {
    pool: AnyPool,
    dialect: SqlDialect,
}

impl DualDbRatesRepository {
    pub const RATE_SELECT_COLUMNS: &'static str =
        "id, name, CAST(percentage AS CHAR(64)) AS percentage_str, product_apply, active, approve_user";

    pub const FLOATING_RATE_SELECT_COLUMNS: &'static str =
        "id, name, is_base_lending_rate, is_active";

    pub const FLOATING_RATE_PERIOD_SELECT_COLUMNS: &'static str =
        "id, floating_rates_id, from_date, CAST(interest_rate AS CHAR(64)) AS interest_rate_str, is_differential_to_base_lending_rate, is_active";

    /// Creates a new [`DualDbRatesRepository`] for `pool` and `dialect`.
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self { pool, dialect }
    }

    /// Returns the configured [`SqlDialect`].
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    /// Returns a reference to the underlying [`AnyPool`].
    pub const fn pool(&self) -> &AnyPool {
        &self.pool
    }

    // --- SQL Query Builders (`m_rate`) ---

    pub fn select_rate_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_rate WHERE id = ?",
            Self::RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_rate_by_name_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_rate WHERE name = ?",
            Self::RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_all_rates_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_rate ORDER BY id ASC",
            Self::RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_active_rates_by_product_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_rate WHERE active = ? AND product_apply = ? ORDER BY id ASC",
            Self::RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn insert_rate_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_rate (\
            id, name, percentage, active, product_apply, created_date, createdby_id, \
            lastmodifiedby_id, lastmodified_date, approve_user\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn update_rate_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_rate SET \
            name = ?, percentage = ?, active = ?, product_apply = ?, \
            lastmodifiedby_id = ?, lastmodified_date = ?, approve_user = ? \
            WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    // --- SQL Query Builders (`m_floating_rates` & `m_floating_rates_periods`) ---

    pub fn select_floating_rate_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_floating_rates WHERE id = ?",
            Self::FLOATING_RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_floating_rate_by_name_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_floating_rates WHERE name = ?",
            Self::FLOATING_RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_base_lending_rate_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_floating_rates WHERE is_base_lending_rate = ? AND is_active = ?",
            Self::FLOATING_RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_all_floating_rates_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_floating_rates ORDER BY id ASC",
            Self::FLOATING_RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_active_floating_rates_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_floating_rates WHERE is_active = ? ORDER BY id ASC",
            Self::FLOATING_RATE_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_floating_rates_linked_to_blr_sql(dialect: SqlDialect) -> String {
        let raw = "SELECT DISTINCT r.id, r.name, r.is_base_lending_rate, r.is_active \
            FROM m_floating_rates r \
            INNER JOIN m_floating_rates_periods p ON p.floating_rates_id = r.id \
            WHERE r.is_active = ? AND p.is_active = ? AND p.is_differential_to_base_lending_rate = ? \
            ORDER BY r.id ASC";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn insert_floating_rate_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_floating_rates (\
            id, name, is_base_lending_rate, is_active, created_date, lastmodified_date, \
            created_by, last_modified_by\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn update_floating_rate_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_floating_rates SET \
            name = ?, is_base_lending_rate = ?, is_active = ?, \
            lastmodified_date = ?, last_modified_by = ? \
            WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn select_periods_by_floating_rate_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_floating_rates_periods WHERE floating_rates_id = ? ORDER BY from_date ASC, id ASC",
            Self::FLOATING_RATE_PERIOD_SELECT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn insert_floating_rate_period_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_floating_rates_periods (\
            id, floating_rates_id, from_date, interest_rate, \
            is_differential_to_base_lending_rate, is_active, created_date, \
            lastmodified_date, created_by, last_modified_by\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn delete_periods_by_floating_rate_id_sql(dialect: SqlDialect) -> String {
        let raw = "DELETE FROM m_floating_rates_periods WHERE floating_rates_id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    // --- Row Mappers ---

    fn map_rate_row(row: &sqlx::any::AnyRow) -> FineractResult<Rate> {
        let id: i64 = row
            .try_get("id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let name: String = row
            .try_get("name")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let percentage_str: String = row
            .try_get("percentage_str")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let percentage = Decimal::from_str(percentage_str.trim())
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let product_apply_raw: i16 = row
            .try_get("product_apply")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let product_apply =
            RateProductAppliesTo::from_i16(product_apply_raw).unwrap_or(RateProductAppliesTo::Loans);
        let active: bool = row
            .try_get("active")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let approve_user_id: Option<i64> = row
            .try_get("approve_user")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(Rate {
            id,
            name,
            percentage,
            product_apply,
            active,
            approve_user_id,
        })
    }

    fn map_period_row(row: &sqlx::any::AnyRow) -> FineractResult<FloatingRatePeriod> {
        let id: i64 = row
            .try_get("id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let from_date: NaiveDate = {
            let s: String = row
                .try_get("from_date")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            let date_part = s.split(|c: char| c == ' ' || c == 'T').next().unwrap_or(s.as_str());
            NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?
        };
        let interest_rate_str: String = row
            .try_get("interest_rate_str")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let interest_rate = Decimal::from_str(interest_rate_str.trim())
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let is_differential_to_base_lending_rate: bool = row
            .try_get("is_differential_to_base_lending_rate")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let is_active: bool = row
            .try_get("is_active")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(FloatingRatePeriod {
            id,
            from_date,
            interest_rate,
            is_differential_to_base_lending_rate,
            is_active,
        })
    }

    async fn load_periods_for_rate(
        &self,
        floating_rate_id: i64,
    ) -> FineractResult<Vec<FloatingRatePeriod>> {
        let sql = Self::select_periods_by_floating_rate_id_sql(self.dialect);
        let rows = sqlx::query(&sql)
            .bind(floating_rate_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let mut periods = Vec::with_capacity(rows.len());
        for row in &rows {
            periods.push(Self::map_period_row(row)?);
        }
        Ok(periods)
    }

    async fn hydrate_floating_rate(
        &self,
        row: &sqlx::any::AnyRow,
    ) -> FineractResult<FloatingRate> {
        let id: i64 = row
            .try_get("id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let name: String = row
            .try_get("name")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let is_base_lending_rate: bool = row
            .try_get("is_base_lending_rate")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let is_active: bool = row
            .try_get("is_active")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let periods = self.load_periods_for_rate(id).await?;

        Ok(FloatingRate::new(
            id,
            name,
            is_base_lending_rate,
            is_active,
            periods,
        ))
    }
}

#[async_trait]
impl RatesRepository for DualDbRatesRepository {
    async fn create_rate(&self, rate: Rate) -> FineractResult<Rate> {
        let sql = Self::insert_rate_sql(self.dialect);
        let now = Utc::now().naive_utc().to_string();
        let user_id: i64 = 1;

        sqlx::query(&sql)
            .bind(rate.id)
            .bind(&rate.name)
            .bind(rate.percentage.to_string())
            .bind(rate.active)
            .bind(rate.product_apply.as_i16())
            .bind(&now)
            .bind(user_id)
            .bind(user_id)
            .bind(&now)
            .bind(rate.approve_user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(rate)
    }

    async fn update_rate(&self, rate: Rate) -> FineractResult<Rate> {
        let sql = Self::update_rate_sql(self.dialect);
        let now = Utc::now().naive_utc().to_string();
        let user_id: i64 = 1;

        sqlx::query(&sql)
            .bind(&rate.name)
            .bind(rate.percentage.to_string())
            .bind(rate.active)
            .bind(rate.product_apply.as_i16())
            .bind(user_id)
            .bind(&now)
            .bind(rate.approve_user_id)
            .bind(rate.id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(rate)
    }

    async fn find_rate_by_id(&self, id: i64) -> FineractResult<Option<Rate>> {
        let sql = Self::select_rate_by_id_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(Self::map_rate_row(row)?)),
            None => Ok(None),
        }
    }

    async fn find_rate_by_name(&self, name: &str) -> FineractResult<Option<Rate>> {
        let sql = Self::select_rate_by_name_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(Self::map_rate_row(row)?)),
            None => Ok(None),
        }
    }

    async fn list_rates(&self) -> FineractResult<Vec<Rate>> {
        let sql = Self::select_all_rates_sql(self.dialect);
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(Self::map_rate_row(row)?);
        }
        Ok(out)
    }

    async fn list_loan_applicable_rates(&self) -> FineractResult<Vec<Rate>> {
        let sql = Self::select_active_rates_by_product_sql(self.dialect);
        let rows = sqlx::query(&sql)
            .bind(true)
            .bind(RateProductAppliesTo::Loans.as_i16())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(Self::map_rate_row(row)?);
        }
        Ok(out)
    }

    async fn create_floating_rate(
        &self,
        floating_rate: FloatingRate,
    ) -> FineractResult<FloatingRate> {
        floating_rate.validate_floating_rate()?;

        let insert_rate_sql = Self::insert_floating_rate_sql(self.dialect);
        let insert_period_sql = Self::insert_floating_rate_period_sql(self.dialect);
        let now = Utc::now().naive_utc().to_string();
        let user_id: i64 = 1;

        sqlx::query(&insert_rate_sql)
            .bind(floating_rate.id)
            .bind(&floating_rate.name)
            .bind(floating_rate.is_base_lending_rate)
            .bind(floating_rate.is_active)
            .bind(&now)
            .bind(&now)
            .bind(user_id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        for period in &floating_rate.periods {
            let from_dt = period.from_date.and_hms_opt(0, 0, 0).unwrap_or_default().to_string();
            sqlx::query(&insert_period_sql)
                .bind(period.id)
                .bind(floating_rate.id)
                .bind(from_dt)
                .bind(period.interest_rate.to_string())
                .bind(period.is_differential_to_base_lending_rate)
                .bind(period.is_active)
                .bind(&now)
                .bind(&now)
                .bind(user_id)
                .bind(user_id)
                .execute(&self.pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        Ok(floating_rate)
    }

    async fn update_floating_rate(
        &self,
        floating_rate: FloatingRate,
    ) -> FineractResult<FloatingRate> {
        floating_rate.validate_floating_rate()?;

        let update_sql = Self::update_floating_rate_sql(self.dialect);
        let delete_periods_sql = Self::delete_periods_by_floating_rate_id_sql(self.dialect);
        let insert_period_sql = Self::insert_floating_rate_period_sql(self.dialect);
        let now = Utc::now().naive_utc().to_string();
        let user_id: i64 = 1;

        sqlx::query(&update_sql)
            .bind(&floating_rate.name)
            .bind(floating_rate.is_base_lending_rate)
            .bind(floating_rate.is_active)
            .bind(&now)
            .bind(user_id)
            .bind(floating_rate.id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        sqlx::query(&delete_periods_sql)
            .bind(floating_rate.id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        for period in &floating_rate.periods {
            let from_dt = period.from_date.and_hms_opt(0, 0, 0).unwrap_or_default().to_string();
            sqlx::query(&insert_period_sql)
                .bind(period.id)
                .bind(floating_rate.id)
                .bind(from_dt)
                .bind(period.interest_rate.to_string())
                .bind(period.is_differential_to_base_lending_rate)
                .bind(period.is_active)
                .bind(&now)
                .bind(&now)
                .bind(user_id)
                .bind(user_id)
                .execute(&self.pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        Ok(floating_rate)
    }

    async fn find_floating_rate_by_id(&self, id: i64) -> FineractResult<Option<FloatingRate>> {
        let sql = Self::select_floating_rate_by_id_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(self.hydrate_floating_rate(row).await?)),
            None => Ok(None),
        }
    }

    async fn find_floating_rate_by_name(&self, name: &str) -> FineractResult<Option<FloatingRate>> {
        let sql = Self::select_floating_rate_by_name_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(self.hydrate_floating_rate(row).await?)),
            None => Ok(None),
        }
    }

    async fn find_base_lending_rate(&self) -> FineractResult<Option<FloatingRate>> {
        let sql = Self::select_base_lending_rate_sql(self.dialect);
        let maybe_row = sqlx::query(&sql)
            .bind(true)
            .bind(true)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        match maybe_row {
            Some(ref row) => Ok(Some(self.hydrate_floating_rate(row).await?)),
            None => Ok(None),
        }
    }

    async fn list_floating_rates(&self) -> FineractResult<Vec<FloatingRate>> {
        let sql = Self::select_all_floating_rates_sql(self.dialect);
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(self.hydrate_floating_rate(row).await?);
        }
        Ok(out)
    }

    async fn list_active_floating_rates(&self) -> FineractResult<Vec<FloatingRate>> {
        let sql = Self::select_active_floating_rates_sql(self.dialect);
        let rows = sqlx::query(&sql)
            .bind(true)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(self.hydrate_floating_rate(row).await?);
        }
        Ok(out)
    }

    async fn list_floating_rates_linked_to_blr(&self) -> FineractResult<Vec<FloatingRate>> {
        let sql = Self::select_floating_rates_linked_to_blr_sql(self.dialect);
        let rows = sqlx::query(&sql)
            .bind(true)
            .bind(true)
            .bind(true)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(self.hydrate_floating_rate(row).await?);
        }
        Ok(out)
    }
}

// =============================================================================
// 5. CQRS COMMAND HANDLERS (`CommandHandler`)
// =============================================================================

/// Command handler for `CREATE_RATE` (`CreateRateCommandHandler.java` & `RateWriteServiceImpl.createRate`).
pub struct CreateRateCommandHandler {
    repo: Arc<dyn RatesRepository>,
}

impl Default for CreateRateCommandHandler {
    fn default() -> Self {
        Self::new(Arc::new(InMemoryRatesRepository::new()))
    }
}

impl CreateRateCommandHandler {
    pub fn new(repo: Arc<dyn RatesRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<dyn RatesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateRateCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let assigned_id = command
            .long_value_of_parameter_named("id")?
            .unwrap_or(0);
        let rate = Rate::from_json(assigned_id, command)?;
        let saved = self.repo.create_rate(rate).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for `UPDATE_RATE` (`UpdateRateCommandHandler.java` & `RateWriteServiceImpl.updateRate`).
pub struct UpdateRateCommandHandler {
    repo: Arc<dyn RatesRepository>,
}

impl Default for UpdateRateCommandHandler {
    fn default() -> Self {
        Self::new(Arc::new(InMemoryRatesRepository::new()))
    }
}

impl UpdateRateCommandHandler {
    pub fn new(repo: Arc<dyn RatesRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<dyn RatesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateRateCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let rate_id = match command.entity_id {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("id")?
                .ok_or_else(|| {
                    FineractError::resource_not_found(
                        "error.msg.rate.id.invalid",
                        "Rate identifier is required for update",
                        "null",
                    )
                })?,
        };

        let mut rate = self.repo.get_rate_by_id(rate_id).await?;
        let changes = rate.update(command)?;

        if !changes.is_empty() {
            self.repo.update_rate(rate).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(rate_id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `CREATE_FLOATINGRATE`
/// (`FloatingRateCreateCommandHandler.java` & `FloatingRateWriteServiceImpl.create`).
pub struct CreateFloatingRateCommandHandler {
    repo: Arc<dyn RatesRepository>,
}

/// Alias matching upstream Java `FloatingRateCreateCommandHandler`.
pub type FloatingRateCreateCommandHandler = CreateFloatingRateCommandHandler;

impl Default for CreateFloatingRateCommandHandler {
    fn default() -> Self {
        Self::new(Arc::new(InMemoryRatesRepository::new()))
    }
}

impl CreateFloatingRateCommandHandler {
    pub fn new(repo: Arc<dyn RatesRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<dyn RatesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateFloatingRateCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let assigned_id = command
            .long_value_of_parameter_named("id")?
            .unwrap_or(0);
        let floating_rate = FloatingRate::from_json(assigned_id, command)?;

        let existing_blr = self.repo.find_base_lending_rate().await?;
        if floating_rate.is_base_lending_rate && existing_blr.is_some() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "org.apache.fineract.portfolio.floatingrate.base-lending-rate.duplicate",
                    "Base Lending Rate already exists.",
                    "isBaseLendingRate",
                    vec![],
                ),
            ));
        }

        if floating_rate
            .periods
            .iter()
            .any(|p| p.is_differential_to_base_lending_rate)
            && existing_blr.is_none()
        {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "org.apache.fineract.portfolio.floatingrate.differential-to-base-lending-rate.not-available",
                    "Base Lending Rate is not available for differential floating rate period.",
                    "isDifferentialToBaseLendingRate",
                    vec![],
                ),
            ));
        }

        let saved = self.repo.create_floating_rate(floating_rate).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for `UPDATE_FLOATINGRATE`
/// (`FloatingRateUpdateCommandHandler.java` & `FloatingRateWriteServiceImpl.update`).
pub struct UpdateFloatingRateCommandHandler {
    repo: Arc<dyn RatesRepository>,
}

/// Alias matching upstream Java `FloatingRateUpdateCommandHandler`.
pub type FloatingRateUpdateCommandHandler = UpdateFloatingRateCommandHandler;

impl Default for UpdateFloatingRateCommandHandler {
    fn default() -> Self {
        Self::new(Arc::new(InMemoryRatesRepository::new()))
    }
}

impl UpdateFloatingRateCommandHandler {
    pub fn new(repo: Arc<dyn RatesRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<dyn RatesRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateFloatingRateCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(FLOATING_RATE_SUPPORTED_PARAMETERS)?;

        let floating_rate_id = match command.entity_id {
            Some(id) => id,
            None => command
                .long_value_of_parameter_named("id")?
                .ok_or_else(|| {
                    FineractError::resource_not_found(
                        "error.msg.floatingrate.id.invalid",
                        "Floating Rate identifier is required for update",
                        "null",
                    )
                })?,
        };

        let mut floating_rate = self.repo.get_floating_rate_by_id(floating_rate_id).await?;
        let existing_blr = self.repo.find_base_lending_rate().await?;

        let requested_blr = command
            .boolean_object_value_of_parameter_named("isBaseLendingRate")
            .or_else(|| command.boolean_object_value_of_parameter_named("is_base_lending_rate"));
        let requested_active = command
            .boolean_object_value_of_parameter_named("isActive")
            .or_else(|| command.boolean_object_value_of_parameter_named("is_active"));

        let new_is_blr = requested_blr.unwrap_or(floating_rate.is_base_lending_rate);
        let new_is_active = requested_active.unwrap_or(floating_rate.is_active);

        if requested_blr == Some(true) {
            if let Some(ref blr) = existing_blr {
                if blr.id != floating_rate.id {
                    return Err(FineractError::DataValidation(
                        PlatformApiDataValidationException::single_parameter_error(
                            "org.apache.fineract.portfolio.floatingrate.base-lending-rate.duplicate",
                            "Base Lending Rate already exists.",
                            "isBaseLendingRate",
                            vec![],
                        ),
                    ));
                }
            }
        }

        let is_blr_modified_as_non_blr = existing_blr
            .as_ref()
            .is_some_and(|blr| blr.id == floating_rate.id)
            && (!new_is_blr || !new_is_active);

        if is_blr_modified_as_non_blr {
            let linked = self.repo.list_floating_rates_linked_to_blr().await?;
            if !linked.is_empty() {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "org.apache.fineract.portfolio.floatingrate.base-lending-rate.linked",
                        "There are floating rates linked to this Base Lending Rate.",
                        "isBaseLendingRate",
                        vec![],
                    ),
                ));
            }
        }

        let mut changes = BTreeMap::new();

        if command.parameter_exists("name") {
            let new_name = command
                .string_value_of_parameter_named("name")
                .unwrap_or_default();
            if new_name.trim().is_empty() {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "org.apache.fineract.portfolio.floatingrate.name.not-blank",
                        "The parameter `name` cannot be blank.",
                        "name",
                        vec![],
                    ),
                ));
            }
            if new_name.chars().count() > 200 {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "org.apache.fineract.portfolio.floatingrate.name.max",
                        "The parameter `name` exceeds max length of 200 characters.",
                        "name",
                        vec![ApiErrorArg::from(200_i32)],
                    ),
                ));
            }
            if new_name != floating_rate.name {
                floating_rate.name = new_name.clone();
                changes.insert("name".to_string(), Value::String(new_name));
            }
        }

        if let Some(val) = requested_blr {
            if val != floating_rate.is_base_lending_rate {
                floating_rate.is_base_lending_rate = val;
                changes.insert("isBaseLendingRate".to_string(), Value::Bool(val));
            }
        }

        if let Some(val) = requested_active {
            if val != floating_rate.is_active {
                floating_rate.is_active = val;
                changes.insert("isActive".to_string(), Value::Bool(val));
            }
        }

        if command.parameter_exists("ratePeriods") || command.parameter_exists("periods") {
            let new_periods = FloatingRate::parse_rate_periods_from_command(command)?;
            if new_periods.is_empty() {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "org.apache.fineract.portfolio.floatingrate.rate-periods.not-empty",
                        "The parameter `ratePeriods` cannot be empty.",
                        "ratePeriods",
                        vec![],
                    ),
                ));
            }

            let blr_available = existing_blr.is_some() && !is_blr_modified_as_non_blr;
            if new_periods
                .iter()
                .any(|p| p.is_differential_to_base_lending_rate)
                && !blr_available
                && !floating_rate.is_base_lending_rate
            {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "org.apache.fineract.portfolio.floatingrate.differential-to-base-lending-rate.not-available",
                        "Base Lending Rate is not available for differential floating rate period.",
                        "isDifferentialToBaseLendingRate",
                        vec![],
                    ),
                ));
            }

            let temp_check = FloatingRate::new(
                floating_rate.id,
                floating_rate.name.clone(),
                floating_rate.is_base_lending_rate,
                floating_rate.is_active,
                new_periods.clone(),
            );
            temp_check.validate_floating_rate()?;

            floating_rate.replace_rate_periods(ctx.business_date(), new_periods.clone());
            changes.insert(
                "ratePeriods".to_string(),
                serde_json::to_value(&new_periods).unwrap_or(Value::Null),
            );
        }

        floating_rate.validate_floating_rate()?;

        if !changes.is_empty() {
            self.repo.update_floating_rate(floating_rate).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(floating_rate_id)
            .with_changes(changes)
            .build())
    }
}

// =============================================================================
// 6. COMPREHENSIVE UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid calendar date")
    }

    fn sample_base_lending_rate() -> FloatingRate {
        FloatingRate::new(
            1,
            "Base Lending Rate",
            true,
            true,
            vec![
                FloatingRatePeriod::new(1, ymd(2026, 1, 1), dec!(5.50), false, true),
                FloatingRatePeriod::new(2, ymd(2026, 6, 1), dec!(6.25), false, true),
                FloatingRatePeriod::new(3, ymd(2026, 10, 1), dec!(7.00), false, true),
                // Inactive period on 2026-08-01 should be ignored
                FloatingRatePeriod::new(4, ymd(2026, 8, 1), dec!(9.99), false, false),
            ],
        )
    }

    #[test]
    fn test_rate_product_applies_to_discriminants_and_parsing() {
        assert_eq!(RateProductAppliesTo::Loans as i16, 1);
        assert_eq!(RateProductAppliesTo::Savings as i16, 2);
        assert_eq!(RateProductAppliesTo::Client as i16, 3);

        assert_eq!(
            RateProductAppliesTo::from_i16(1),
            Some(RateProductAppliesTo::Loans)
        );
        assert_eq!(
            RateProductAppliesTo::from_i16(2),
            Some(RateProductAppliesTo::Savings)
        );
        assert_eq!(
            RateProductAppliesTo::from_i16(3),
            Some(RateProductAppliesTo::Client)
        );
        assert_eq!(RateProductAppliesTo::from_i16(99), None);

        assert_eq!(
            "1".parse::<RateProductAppliesTo>().unwrap(),
            RateProductAppliesTo::Loans
        );
        assert_eq!(
            "SAVINGS".parse::<RateProductAppliesTo>().unwrap(),
            RateProductAppliesTo::Savings
        );
        assert_eq!(
            "client".parse::<RateProductAppliesTo>().unwrap(),
            RateProductAppliesTo::Client
        );
    }

    #[test]
    fn test_base_lending_rate_lookup_across_dates() {
        let blr = sample_base_lending_rate();
        assert!(blr.validate_floating_rate().is_ok());

        // Before first period (2025-12-31) -> error
        let before_err = blr.fetch_applicable_rate(ymd(2025, 12, 31), None, dec!(0));
        assert!(before_err.is_err());

        // Exact boundary 2026-01-01 -> 5.50
        assert_eq!(
            blr.fetch_applicable_rate(ymd(2026, 1, 1), None, dec!(0))
                .unwrap(),
            dec!(5.50)
        );

        // Mid-period 2026-05-31 -> 5.50 + factor 0.50 = 6.00
        assert_eq!(
            blr.fetch_applicable_rate(ymd(2026, 5, 31), None, dec!(0.50))
                .unwrap(),
            dec!(6.00)
        );

        // Second period boundary 2026-06-01 -> 6.25
        assert_eq!(
            blr.fetch_applicable_rate(ymd(2026, 6, 1), None, dec!(0))
                .unwrap(),
            dec!(6.25)
        );

        // Inactive period on 2026-08-01 is skipped -> still 6.25 on 2026-08-15
        assert_eq!(
            blr.fetch_applicable_rate(ymd(2026, 8, 15), None, dec!(0))
                .unwrap(),
            dec!(6.25)
        );

        // Third period boundary 2026-10-01 and beyond -> 7.00
        assert_eq!(
            blr.fetch_applicable_rate(ymd(2026, 10, 1), None, dec!(0))
                .unwrap(),
            dec!(7.00)
        );
        assert_eq!(
            blr.fetch_applicable_rate(ymd(2027, 3, 15), None, dec!(1.25))
                .unwrap(),
            dec!(8.25)
        );
    }

    #[test]
    fn test_differential_floating_rate_calculation_across_dates() {
        let blr = sample_base_lending_rate();
        let product_rate = FloatingRate::new(
            2,
            "SME Floating Rate",
            false,
            true,
            vec![
                // Non-differential period from Jan 1: flat 8.00%
                FloatingRatePeriod::new(10, ymd(2026, 1, 1), dec!(8.00), false, true),
                // Differential period from Mar 1: BLR + 2.00%
                FloatingRatePeriod::new(11, ymd(2026, 3, 1), dec!(2.00), true, true),
                // Differential period from Sep 1: BLR + 1.50%
                FloatingRatePeriod::new(12, ymd(2026, 9, 1), dec!(1.50), true, true),
            ],
        );

        assert!(product_rate.validate_floating_rate().is_ok());

        // 1. On 2026-02-15: non-differential period (8.00%) + loan factor (0.25%) = 8.25%
        assert_eq!(
            product_rate
                .fetch_applicable_rate(ymd(2026, 2, 15), Some(&blr), dec!(0.25))
                .unwrap(),
            dec!(8.25)
        );

        // 2. On 2026-04-01: differential period (2.00%) + BLR on Apr 1 (5.50%) + factor (0.50%) = 8.00%
        assert_eq!(
            product_rate
                .fetch_applicable_rate(ymd(2026, 4, 1), Some(&blr), dec!(0.50))
                .unwrap(),
            dec!(8.00)
        );

        // 3. On 2026-07-01: BLR stepped up on Jun 1 to 6.25%, differential period still 2.00%, factor 0.50% -> 8.75%
        assert_eq!(
            product_rate
                .fetch_applicable_rate(ymd(2026, 7, 1), Some(&blr), dec!(0.50))
                .unwrap(),
            dec!(8.75)
        );

        // 4. On 2026-09-15: differential period changed to 1.50% + BLR (6.25%) + factor (0.00%) = 7.75%
        assert_eq!(
            product_rate
                .fetch_applicable_rate(ymd(2026, 9, 15), Some(&blr), dec!(0.00))
                .unwrap(),
            dec!(7.75)
        );

        // 5. On 2026-11-01: BLR stepped up on Oct 1 to 7.00% + differential (1.50%) + factor (-0.25%) = 8.25%
        assert_eq!(
            product_rate
                .fetch_applicable_rate(ymd(2026, 11, 1), Some(&blr), dec!(-0.25))
                .unwrap(),
            dec!(8.25)
        );

        // 6. Missing BLR when period is differential -> error
        assert!(product_rate
            .fetch_applicable_rate(ymd(2026, 4, 1), None, dec!(0))
            .is_err());
    }

    #[test]
    fn test_floating_rate_validation_rules() {
        // 1. Duplicate from_date fails validation
        let duplicate_dates_rate = FloatingRate::new(
            10,
            "Duplicate Date Rate",
            false,
            true,
            vec![
                FloatingRatePeriod::new(1, ymd(2026, 5, 1), dec!(5.0), false, true),
                FloatingRatePeriod::new(2, ymd(2026, 5, 1), dec!(6.0), false, true),
            ],
        );
        let err = duplicate_dates_rate.validate_floating_rate().unwrap_err();
        assert_eq!(err.http_status_code(), 400);
        assert!(err.to_string().contains("duplicate") || format!("{err:?}").contains("from-date.duplicate"));

        // 2. Base lending rate cannot have differential periods
        let invalid_blr = FloatingRate::new(
            11,
            "Invalid Base Lending Rate",
            true,
            true,
            vec![FloatingRatePeriod::new(
                1,
                ymd(2026, 5, 1),
                dec!(4.0),
                true,
                true,
            )],
        );
        let err = invalid_blr.validate_floating_rate().unwrap_err();
        assert_eq!(err.http_status_code(), 400);
        assert!(format!("{err:?}").contains("invalid-for-base-lending-rate"));

        // 3. Negative interest rate fails validation
        let negative_rate = FloatingRate::new(
            12,
            "Negative Period Rate",
            false,
            true,
            vec![FloatingRatePeriod::new(
                1,
                ymd(2026, 5, 1),
                dec!(-0.01),
                false,
                true,
            )],
        );
        assert!(negative_rate.validate_floating_rate().is_err());

        // 4. Blank name or empty periods fails validation
        let blank_name = FloatingRate::new(
            13,
            "   ",
            false,
            true,
            vec![FloatingRatePeriod::new(
                1,
                ymd(2026, 5, 1),
                dec!(5.0),
                false,
                true,
            )],
        );
        assert!(blank_name.validate_floating_rate().is_err());

        let empty_periods = FloatingRate::new(14, "Empty Periods", false, true, vec![]);
        assert!(empty_periods.validate_floating_rate().is_err());
    }

    #[test]
    fn test_dual_db_sql_placeholder_rewriting() {
        assert_eq!(
            DualDbRatesRepository::select_rate_by_id_sql(SqlDialect::Postgres),
            "SELECT id, name, CAST(percentage AS CHAR(64)) AS percentage_str, product_apply, active, approve_user FROM m_rate WHERE id = $1"
        );
        assert_eq!(
            DualDbRatesRepository::select_rate_by_id_sql(SqlDialect::MySql),
            "SELECT id, name, CAST(percentage AS CHAR(64)) AS percentage_str, product_apply, active, approve_user FROM m_rate WHERE id = ?"
        );
        assert_eq!(
            DualDbRatesRepository::select_base_lending_rate_sql(SqlDialect::Postgres),
            "SELECT id, name, is_base_lending_rate, is_active FROM m_floating_rates WHERE is_base_lending_rate = $1 AND is_active = $2"
        );
        assert_eq!(
            DualDbRatesRepository::select_periods_by_floating_rate_id_sql(SqlDialect::Postgres),
            "SELECT id, floating_rates_id, from_date, CAST(interest_rate AS CHAR(64)) AS interest_rate_str, is_differential_to_base_lending_rate, is_active FROM m_floating_rates_periods WHERE floating_rates_id = $1 ORDER BY from_date ASC, id ASC"
        );
    }

    #[tokio::test]
    async fn test_rate_and_floating_rate_command_handlers() {
        let repo = Arc::new(InMemoryRatesRepository::new());
        let ctx = ExecutionContext::new("default", ymd(2026, 1, 1));

        // 1. Create Rate via CreateRateCommandHandler
        let create_rate_handler = CreateRateCommandHandler::new(repo.clone());
        let create_rate_cmd = JsonCommand::from_json(
            r#"{
                "name": "Standard Loan VAT Rate",
                "percentage": "12.50",
                "productApply": 1,
                "active": true,
                "locale": "en_US"
            }"#,
        )
        .unwrap()
        .with_command_id(100);

        let create_res = create_rate_handler
            .handle(&ctx, &create_rate_cmd)
            .await
            .unwrap();
        let rate_id = create_res.resource_id.unwrap();
        assert_eq!(rate_id, 1);

        let stored_rate = repo.get_rate_by_id(rate_id).await.unwrap();
        assert_eq!(stored_rate.name, "Standard Loan VAT Rate");
        assert_eq!(stored_rate.percentage, dec!(12.50));
        assert_eq!(stored_rate.product_apply, RateProductAppliesTo::Loans);
        assert!(stored_rate.active);

        // 2. Update Rate via UpdateRateCommandHandler
        let update_rate_handler = UpdateRateCommandHandler::new(repo.clone());
        let update_rate_cmd = JsonCommand::from_json(
            r#"{
                "percentage": "14.75",
                "active": false,
                "locale": "en_US"
            }"#,
        )
        .unwrap()
        .with_entity_id(rate_id);

        let update_res = update_rate_handler
            .handle(&ctx, &update_rate_cmd)
            .await
            .unwrap();
        assert!(update_res.has_changes());
        let updated_rate = repo.get_rate_by_id(rate_id).await.unwrap();
        assert_eq!(updated_rate.percentage, dec!(14.75));
        assert!(!updated_rate.active);

        // Attempting to change productApply on update fails with domain rule violation
        let invalid_update_cmd = JsonCommand::from_json(r#"{"productApply": 2}"#)
            .unwrap()
            .with_entity_id(rate_id);
        let err = update_rate_handler
            .handle(&ctx, &invalid_update_cmd)
            .await
            .unwrap_err();
        assert_eq!(err.http_status_code(), 403);

        // 3. Create Base Lending Rate via CreateFloatingRateCommandHandler
        let create_fr_handler = CreateFloatingRateCommandHandler::new(repo.clone());
        let create_blr_cmd = JsonCommand::from_json(
            r#"{
                "name": "Base Lending Rate",
                "isBaseLendingRate": true,
                "isActive": true,
                "ratePeriods": [
                    {
                        "fromDate": "01 February 2026",
                        "interestRate": 5.0,
                        "isDifferentialToBaseLendingRate": false,
                        "locale": "en",
                        "dateFormat": "dd MMMM yyyy"
                    },
                    {
                        "fromDate": "01 July 2026",
                        "interestRate": 6.0,
                        "isDifferentialToBaseLendingRate": false,
                        "locale": "en",
                        "dateFormat": "dd MMMM yyyy"
                    }
                ]
            }"#,
        )
        .unwrap();

        let blr_res = create_fr_handler
            .handle(&ctx, &create_blr_cmd)
            .await
            .unwrap();
        let blr_id = blr_res.resource_id.unwrap();

        // Creating a second Base Lending Rate fails
        let duplicate_blr_cmd = JsonCommand::from_json(
            r#"{
                "name": "Second BLR",
                "isBaseLendingRate": true,
                "isActive": true,
                "ratePeriods": [
                    {
                        "fromDate": "2026-03-01",
                        "interestRate": 4.0
                    }
                ]
            }"#,
        )
        .unwrap();
        assert!(create_fr_handler
            .handle(&ctx, &duplicate_blr_cmd)
            .await
            .is_err());

        // 4. Create Differential Floating Rate linked to BLR
        let create_diff_cmd = JsonCommand::from_json(
            r#"{
                "name": "Differential Prime Rate",
                "isBaseLendingRate": false,
                "isActive": true,
                "ratePeriods": [
                    {
                        "fromDate": "2026-02-01",
                        "interestRate": 2.5,
                        "isDifferentialToBaseLendingRate": true
                    }
                ]
            }"#,
        )
        .unwrap();
        let diff_res = create_fr_handler
            .handle(&ctx, &create_diff_cmd)
            .await
            .unwrap();
        let diff_id = diff_res.resource_id.unwrap();

        // Deactivating BLR while Differential Prime Rate is linked to it fails
        let update_fr_handler = UpdateFloatingRateCommandHandler::new(repo.clone());
        let deactivate_blr_cmd = JsonCommand::from_json(r#"{"isActive": false}"#)
            .unwrap()
            .with_entity_id(blr_id);
        assert!(update_fr_handler
            .handle(&ctx, &deactivate_blr_cmd)
            .await
            .is_err());

        // 5. Update Differential Prime Rate with a new future period and verify calculation
        let update_diff_cmd = JsonCommand::from_json(
            r#"{
                "name": "Differential Prime Rate v2",
                "ratePeriods": [
                    {
                        "fromDate": "2026-09-01",
                        "interestRate": 3.0,
                        "isDifferentialToBaseLendingRate": true
                    }
                ]
            }"#,
        )
        .unwrap()
        .with_entity_id(diff_id);
        let update_diff_res = update_fr_handler
            .handle(&ctx, &update_diff_cmd)
            .await
            .unwrap();
        assert!(update_diff_res.has_changes());

        let stored_blr = repo.get_base_lending_rate().await.unwrap();
        let stored_diff = repo.get_floating_rate_by_id(diff_id).await.unwrap();
        assert_eq!(stored_diff.name, "Differential Prime Rate v2");
        assert_eq!(
            stored_diff
                .fetch_applicable_rate(ymd(2026, 9, 15), Some(&stored_blr), dec!(0.50))
                .unwrap(),
            dec!(9.50) // BLR 6.0 (from Jul 1) + diff 3.0 (from Sep 1) + factor 0.50
        );
    }
}
