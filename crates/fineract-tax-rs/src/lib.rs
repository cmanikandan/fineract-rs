//! Tax components, date-effective histories, tax groups, and deterministic tax calculation utilities
//! ported from Apache Fineract `fineract-tax`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use chrono::NaiveDate;
use fineract_command_rs::pipeline::{CommandHandler, ExecutionContext};
use fineract_core_rs::db::{DialectSql, SqlDialect};
use fineract_core_rs::json_command::JsonCommand;
use fineract_core_rs::primitives::{
    CommandProcessingResult, CommandProcessingResultBuilder, EnumOptionData,
};
use fineract_money::{FineractDecimal, FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{DataValidatorBuilder, FineractError};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{AnyPool, Row};

const HUNDRED: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

/// Error globalisation code when a tax group's combined component percentage exceeds 100%.
pub const TAX_GROUP_TOTAL_PERCENTAGE_MORE_THAN_MAX_CODE: &str =
    "error.msg.tax.group.total.percentage.is.more.than.max";

/// Parameter constants matching upstream `TaxApiConstants.java`.
pub mod tax_api_constants {
    pub const NAME_PARAM_NAME: &str = "name";
    pub const PERCENTAGE_PARAM_NAME: &str = "percentage";
    pub const DEBIT_ACCOUNT_TYPE_PARAM_NAME: &str = "debitAccountType";
    pub const DEBIT_ACCOUNT_ID_PARAM_NAME: &str = "debitAccountId";
    pub const CREDIT_ACCOUNT_TYPE_PARAM_NAME: &str = "creditAccountType";
    pub const CREDIT_ACCOUNT_ID_PARAM_NAME: &str = "creditAccountId";
    pub const START_DATE_PARAM_NAME: &str = "startDate";
    pub const END_DATE_PARAM_NAME: &str = "endDate";
    pub const CHARGE_INCLUDES_TAX_PARAM_NAME: &str = "chargeIncludesTax";
    pub const TAX_COMPONENTS_PARAM_NAME: &str = "taxComponents";
    pub const ID_PARAM_NAME: &str = "id";
    pub const TAX_COMPONENT_ID_PARAM_NAME: &str = "taxComponentId";
    pub const LOCALE_PARAM_NAME: &str = "locale";
    pub const DATE_FORMAT_PARAM_NAME: &str = "dateFormat";
}

/// General Ledger / Tax Account classification matching `GLAccountType` in Fineract.
#[repr(i16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaxAccountType {
    Asset = 1,
    Liability = 2,
    Equity = 3,
    Income = 4,
    Expense = 5,
}

/// Alias matching upstream Java `GLAccountType`.
pub type GLAccountType = TaxAccountType;

impl TaxAccountType {
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Asset => "accountType.asset",
            Self::Liability => "accountType.liability",
            Self::Equity => "accountType.equity",
            Self::Income => "accountType.income",
            Self::Expense => "accountType.expense",
        }
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Asset),
            2 => Some(Self::Liability),
            3 => Some(Self::Equity),
            4 => Some(Self::Income),
            5 => Some(Self::Expense),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Asset),
            2 => Some(Self::Liability),
            3 => Some(Self::Equity),
            4 => Some(Self::Income),
            5 => Some(Self::Expense),
            _ => None,
        }
    }

    #[inline]
    pub const fn is_asset_type(self) -> bool {
        matches!(self, Self::Asset)
    }

    #[inline]
    pub const fn is_liability_type(self) -> bool {
        matches!(self, Self::Liability)
    }

    #[inline]
    pub const fn is_equity_type(self) -> bool {
        matches!(self, Self::Equity)
    }

    #[inline]
    pub const fn is_income_type(self) -> bool {
        matches!(self, Self::Income)
    }

    #[inline]
    pub const fn is_expense_type(self) -> bool {
        matches!(self, Self::Expense)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        let label = match self {
            Self::Asset => "ASSET",
            Self::Liability => "LIABILITY",
            Self::Equity => "EQUITY",
            Self::Income => "INCOME",
            Self::Expense => "EXPENSE",
        };
        EnumOptionData::new(self as i64, self.code(), label)
    }
}

impl From<TaxAccountType> for i16 {
    fn from(value: TaxAccountType) -> Self {
        value as i16
    }
}

impl From<TaxAccountType> for i32 {
    fn from(value: TaxAccountType) -> Self {
        value as i32
    }
}

impl TryFrom<i16> for TaxAccountType {
    type Error = FineractError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::from_i16(value).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.tax.account.type.invalid",
                format!("Invalid TaxAccountType value: {value}"),
            )
        })
    }
}

impl TryFrom<i32> for TaxAccountType {
    type Error = FineractError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_int(value).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.tax.account.type.invalid",
                format!("Invalid TaxAccountType value: {value}"),
            )
        })
    }
}

/// Historical tax rate period matching `m_tax_component_history` (`TaxComponentHistory.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxComponentHistory {
    pub id: i64,
    pub percentage: Decimal,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
}

impl TaxComponentHistory {
    pub fn new(
        id: i64,
        percentage: Decimal,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            id,
            percentage,
            start_date,
            end_date,
        }
    }

    #[inline]
    pub fn start_date(&self) -> NaiveDate {
        self.start_date
    }

    #[inline]
    pub fn end_date(&self) -> Option<NaiveDate> {
        self.end_date
    }

    #[inline]
    pub fn percentage(&self) -> Decimal {
        self.percentage
    }

    pub fn update_end_date(&mut self, end_date: NaiveDate) {
        self.end_date = Some(end_date);
    }

    /// Checks whether `target` falls within `[self.start_date, self.end_date]`.
    pub fn occurs_on_day_from_and_up_to_and_including(&self, target: NaiveDate) -> bool {
        target >= self.start_date && self.end_date.map_or(true, |end| target <= end)
    }
}

/// Tax component definition matching `m_tax_component` (`TaxComponent.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxComponent {
    pub id: i64,
    pub name: String,
    pub percentage: Decimal,
    pub debit_account_type: Option<i16>,
    pub debit_account_id: Option<i64>,
    pub credit_account_type: Option<i16>,
    pub credit_account_id: Option<i64>,
    pub start_date: NaiveDate,
    pub history: Vec<TaxComponentHistory>,
}

impl TaxComponent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        name: impl Into<String>,
        percentage: Decimal,
        debit_account_type: Option<i16>,
        debit_account_id: Option<i64>,
        credit_account_type: Option<i16>,
        credit_account_id: Option<i64>,
        start_date: NaiveDate,
        history: Vec<TaxComponentHistory>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            percentage,
            debit_account_type,
            debit_account_id,
            credit_account_type,
            credit_account_id,
            start_date,
            history,
        }
    }

    /// Returns the applicable percentage on `date` by checking the current effective period
    /// and falling back to `self.history`.
    ///
    /// Note on boundary dates:
    /// - If a history record has `end_date == Some(date)` and `date == self.start_date`:
    ///   - In Fineract's `TaxComponent.occursOnDayFrom(target)` (`DateUtils.isAfter(target, startDate)`),
    ///     when a history entry ends on `self.start_date` and `date > self.start_date`, the current
    ///     `self.percentage` applies; when `date >= self.start_date` and no history entry has
    ///     `end_date > date`, `self.percentage` applies on `self.start_date` as well.
    pub fn applicable_percentage_on(&self, date: NaiveDate) -> Option<Decimal> {
        if date >= self.start_date {
            return Some(self.percentage);
        }

        // Prefer half-open `[start_date, end_date)` when contiguous histories share a boundary date,
        // then fall back to closed `[start_date, end_date]`.
        if let Some(hist) = self
            .history
            .iter()
            .find(|h| date >= h.start_date && h.end_date.map_or(true, |end| date < end))
        {
            return Some(hist.percentage);
        }

        for hist in &self.history {
            if hist.occurs_on_day_from_and_up_to_and_including(date) {
                return Some(hist.percentage);
            }
        }

        None
    }

    /// Alias for [`Self::applicable_percentage_on`] matching Java `getApplicablePercentage`.
    #[inline]
    pub fn get_applicable_percentage(&self, date: NaiveDate) -> Option<Decimal> {
        self.applicable_percentage_on(date)
    }

    /// Returns the most recent historical rate (`end_date` latest), matching `findPreviousRate()` in `TaxComponent.java`.
    pub fn find_previous_rate(&self) -> Option<&TaxComponentHistory> {
        self.history
            .iter()
            .filter(|h| h.end_date.is_some())
            .max_by_key(|h| h.end_date)
    }

    /// Mutable variant of [`Self::find_previous_rate`].
    pub fn find_previous_rate_mut(&mut self) -> Option<&mut TaxComponentHistory> {
        self.history
            .iter_mut()
            .filter(|h| h.end_date.is_some())
            .max_by_key(|h| h.end_date)
    }

    /// Collects the current `start_date` and all historical `start_date`s.
    pub fn all_start_dates(&self) -> Vec<NaiveDate> {
        let mut dates = Vec::with_capacity(1 + self.history.len());
        dates.push(self.start_date);
        for h in &self.history {
            dates.push(h.start_date);
        }
        dates
    }

    /// Applies an update command to this `TaxComponent`, archiving the previous rate into `self.history`
    /// when `percentage` changes or updating the previous history entry's `end_date` when only `startDate` moves.
    pub fn apply_update(
        &mut self,
        command: &JsonCommand,
        next_history_id: i64,
    ) -> Result<BTreeMap<String, Value>, FineractError> {
        use tax_api_constants::*;
        let mut changes = BTreeMap::new();

        if command.is_change_in_string_parameter_named(NAME_PARAM_NAME, Some(&self.name)) {
            if let Some(new_name) = command.string_value_of_parameter_named(NAME_PARAM_NAME) {
                let new_name = new_name.trim().to_string();
                changes.insert(NAME_PARAM_NAME.to_string(), json!(new_name));
                self.name = new_name;
            }
        }

        if command.is_change_in_big_decimal_parameter_named(
            PERCENTAGE_PARAM_NAME,
            Some(self.percentage),
        ) {
            if let Some(new_pct) =
                command.big_decimal_value_of_parameter_named(PERCENTAGE_PARAM_NAME)
            {
                changes.insert(PERCENTAGE_PARAM_NAME.to_string(), json!(new_pct));
                if let Some(locale) = command.locale() {
                    changes.insert(LOCALE_PARAM_NAME.to_string(), json!(locale));
                }

                let old_start = self.start_date;
                let new_start = command
                    .local_date_value_of_parameter_named(START_DATE_PARAM_NAME)
                    .unwrap_or(self.start_date);

                changes.insert(
                    START_DATE_PARAM_NAME.to_string(),
                    json!(new_start.format("%Y-%m-%d").to_string()),
                );
                if let Some(df) = command.date_format() {
                    changes.insert(DATE_FORMAT_PARAM_NAME.to_string(), json!(df));
                }
                if let Some(locale) = command.locale() {
                    changes.insert(LOCALE_PARAM_NAME.to_string(), json!(locale));
                }

                let history_entry = TaxComponentHistory::new(
                    next_history_id,
                    self.percentage,
                    old_start,
                    Some(new_start),
                );
                self.history.push(history_entry);
                self.percentage = new_pct;
                self.start_date = new_start;
            }
        } else if command
            .is_change_in_local_date_parameter_named(START_DATE_PARAM_NAME, Some(self.start_date))
        {
            if let Some(new_start) =
                command.local_date_value_of_parameter_named(START_DATE_PARAM_NAME)
            {
                changes.insert(
                    START_DATE_PARAM_NAME.to_string(),
                    json!(new_start.format("%Y-%m-%d").to_string()),
                );
                self.start_date = new_start;
                if let Some(prev) = self.find_previous_rate_mut() {
                    prev.update_end_date(new_start);
                }
            }
        }

        Ok(changes)
    }
}

/// Mapping of a `TaxComponent` into a `TaxGroup` with an effective date range,
/// matching `m_tax_group_mappings` (`TaxGroupMappings.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxGroupMapping {
    pub id: i64,
    pub tax_component: TaxComponent,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
}

/// Alias matching upstream Java `TaxGroupMappings`.
pub type TaxGroupMappings = TaxGroupMapping;

impl TaxGroupMapping {
    pub fn new(
        id: i64,
        tax_component: TaxComponent,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            id,
            tax_component,
            start_date,
            end_date,
        }
    }

    /// Returns `true` if `target` is on or after `self.start_date` and (if `self.end_date` is set)
    /// on or before `self.end_date`.
    pub fn occurs_on_day_from_and_up_to_and_including(&self, target: NaiveDate) -> bool {
        target >= self.start_date && self.end_date.map_or(true, |end| target <= end)
    }

    /// Returns the applicable percentage of the mapped component on `date` if this mapping is active on `date`.
    pub fn applicable_percentage_on(&self, date: NaiveDate) -> Option<Decimal> {
        if self.occurs_on_day_from_and_up_to_and_including(date) {
            self.tax_component.applicable_percentage_on(date)
        } else {
            None
        }
    }
}

/// A named group of date-effective `TaxGroupMapping` records matching `m_tax_group` (`TaxGroup.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxGroup {
    pub id: i64,
    pub name: String,
    pub mappings: Vec<TaxGroupMapping>,
}

impl TaxGroup {
    pub fn new(id: i64, name: impl Into<String>, mappings: Vec<TaxGroupMapping>) -> Self {
        Self {
            id,
            name: name.into(),
            mappings,
        }
    }

    /// Finds an existing mapping by ID or returns a `NotFound` (`error.msg.tax.mapping.id.invalid`) error.
    pub fn find_mapping_by_id(&self, id: i64) -> Result<&TaxGroupMapping, FineractError> {
        self.mappings
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| {
                FineractError::not_found(
                    "error.msg.tax.mapping.id.invalid",
                    format!("Tax mapping with identifier {id} does not exist"),
                )
            })
    }

    /// Mutable variant of [`Self::find_mapping_by_id`].
    pub fn find_mapping_by_id_mut(
        &mut self,
        id: i64,
    ) -> Result<&mut TaxGroupMapping, FineractError> {
        self.mappings
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or_else(|| {
                FineractError::not_found(
                    "error.msg.tax.mapping.id.invalid",
                    format!("Tax mapping with identifier {id} does not exist"),
                )
            })
    }

    /// Validates this tax group's mappings (overlapping same-component dates and total percentage <= 100%).
    pub fn validate(&self) -> Result<(), FineractError> {
        TaxUtil::validate_tax_group_mappings(&self.mappings)
    }
}

/// Result of splitting a gross or taxable amount across a `TaxGroup`'s active `TaxComponent`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxSplitBreakdown {
    pub tax_component_id: i64,
    pub percentage: Decimal,
    pub tax_amount: Money,
    pub credit_account_id: Option<i64>,
}

impl TaxSplitBreakdown {
    pub fn new(
        tax_component_id: i64,
        percentage: Decimal,
        tax_amount: Money,
        credit_account_id: Option<i64>,
    ) -> Self {
        Self {
            tax_component_id,
            percentage,
            tax_amount,
            credit_account_id,
        }
    }
}

/// Stateless tax calculation and validation utilities ported from `TaxUtils.java` and `TaxValidator.java`.
pub struct TaxUtil;

/// Alias matching upstream Java `TaxUtils`.
pub type TaxUtils = TaxUtil;

impl TaxUtil {
    /// Validates that the sum of applicable component percentages on `date` does not exceed 100%,
    /// and that across all effective boundary dates in `mappings`, total percentage never exceeds 100%.
    pub fn validate_tax_group_mappings(mappings: &[TaxGroupMapping]) -> Result<(), FineractError> {
        // 1. Validate that the same tax component does not have overlapping date intervals.
        for i in 0..mappings.len() {
            for j in (i + 1)..mappings.len() {
                let a = &mappings[i];
                let b = &mappings[j];
                if a.tax_component.id == b.tax_component.id {
                    let overlaps = match (a.end_date, b.end_date) {
                        (None, None) => true,
                        (Some(a_end), None) => a_end >= b.start_date,
                        (None, Some(b_end)) => b_end >= a.start_date,
                        (Some(a_end), Some(b_end)) => {
                            !(a.start_date > b_end || b.start_date > a_end)
                        }
                    };
                    if overlaps {
                        return Err(FineractError::domain_rule(
                            "error.msg.tax.group.component.dates.overlap",
                            "Same tax component cannot have overlapping date ranges in a tax group.",
                        ));
                    }
                }
            }
        }

        // 2. Collect all candidate transition dates (mapping start dates + component start/history dates)
        let mut dates = Vec::new();
        for m in mappings {
            dates.push(m.start_date);
            if let Some(end) = m.end_date {
                dates.push(end);
            }
            for d in m.tax_component.all_start_dates() {
                dates.push(d);
            }
        }
        dates.sort_unstable();
        dates.dedup();

        for check_date in dates {
            Self::validate_total_percentage_on_date(mappings, check_date)?;
        }

        Ok(())
    }

    /// Validates that on `date`, the sum of applicable percentages across `mappings` is `<= 100`.
    /// Returns a `FineractError` with code `error.msg.tax.group.total.percentage.is.more.than.max`
    /// if the total exceeds 100%.
    pub fn validate_total_percentage_on_date(
        mappings: &[TaxGroupMapping],
        date: NaiveDate,
    ) -> Result<Decimal, FineractError> {
        let mut total = Decimal::ZERO;
        for mapping in mappings {
            if let Some(pct) = mapping.applicable_percentage_on(date) {
                total += pct;
            }
        }
        if total > HUNDRED {
            return Err(FineractError::domain_rule(
                TAX_GROUP_TOTAL_PERCENTAGE_MORE_THAN_MAX_CODE,
                format!(
                    "Total tax percentage ({total}) cannot be more than 100 ({TAX_GROUP_TOTAL_PERCENTAGE_MORE_THAN_MAX_CODE})"
                ),
            ));
        }
        Ok(total)
    }

    /// Calculates tax for a single component percentage at the given `scale` and `mode`.
    /// Equivalent to `TaxUtils.calculateTax` in Java: `(amount * percentage / 100).round_to(scale, mode)`.
    #[inline]
    pub fn calculate_tax_decimal(
        amount: Decimal,
        percentage: Decimal,
        scale: u32,
        mode: FineractRoundingMode,
    ) -> Decimal {
        if amount.is_zero() || percentage.is_zero() {
            return Decimal::ZERO;
        }
        let raw = (amount * percentage) / HUNDRED;
        FineractDecimal::new(raw).round_to(scale, mode).into_inner()
    }

    /// Splits the tax on `amount` as of `date` across all active mappings in `tax_group`.
    ///
    /// Validates that the total applicable percentage on `date` (and across the tax group)
    /// does not exceed 100% (`error.msg.tax.group.total.percentage.is.more.than.max`).
    pub fn split_tax(
        amount: &Money,
        date: NaiveDate,
        tax_group: &TaxGroup,
        mode: FineractRoundingMode,
    ) -> Result<Vec<TaxSplitBreakdown>, FineractError> {
        Self::split_tax_mappings(
            amount,
            date,
            &tax_group.mappings,
            amount.currency().digits_after_decimal(),
            mode,
        )
    }

    /// Splits the tax on `amount` as of `date` across `mappings` using the specified `scale` and `mode`.
    pub fn split_tax_mappings(
        amount: &Money,
        date: NaiveDate,
        mappings: &[TaxGroupMapping],
        scale: u32,
        mode: FineractRoundingMode,
    ) -> Result<Vec<TaxSplitBreakdown>, FineractError> {
        Self::validate_total_percentage_on_date(mappings, date)?;
        Self::validate_tax_group_mappings(mappings)?;

        let currency = if scale == amount.currency().digits_after_decimal() {
            amount.currency().clone()
        } else {
            MonetaryCurrency::new(amount.currency_code(), scale, amount.in_multiples_of())
        };

        let mut result = Vec::new();
        let base_amount = amount.amount();

        for mapping in mappings {
            if let Some(percentage) = mapping.applicable_percentage_on(date) {
                let tax_dec =
                    Self::calculate_tax_decimal(base_amount, percentage, scale, mode);
                let tax_money = Money::of(currency.clone(), tax_dec, mode);
                result.push(TaxSplitBreakdown {
                    tax_component_id: mapping.tax_component.id,
                    percentage,
                    tax_amount: tax_money,
                    credit_account_id: mapping.tax_component.credit_account_id,
                });
            }
        }

        Ok(result)
    }

    /// Computes `(net_income_amount, tax_split_breakdowns)` from `gross_amount` on `date`
    /// across `mappings` at the given `scale` and `mode`, ported from `TaxUtils.incomeAmount`.
    ///
    /// Validates that the total applicable tax percentage does not exceed 100%
    /// (`error.msg.tax.group.total.percentage.is.more.than.max`).
    pub fn income_amount(
        gross_amount: &Money,
        date: NaiveDate,
        mappings: &[TaxGroupMapping],
        scale: u32,
        mode: FineractRoundingMode,
    ) -> Result<(Money, Vec<TaxSplitBreakdown>), FineractError> {
        let splits = Self::split_tax_mappings(gross_amount, date, mappings, scale, mode)?;

        let currency = if scale == gross_amount.currency().digits_after_decimal() {
            gross_amount.currency().clone()
        } else {
            MonetaryCurrency::new(
                gross_amount.currency_code(),
                scale,
                gross_amount.in_multiples_of(),
            )
        };

        let total_tax_dec: Decimal = splits.iter().map(|s| s.tax_amount.amount()).sum();
        let net_dec = gross_amount.amount() - total_tax_dec;
        let net_money = Money::of(currency, net_dec, mode);

        Ok((net_money, splits))
    }

    /// Sums the `tax_amount` across a slice of [`TaxSplitBreakdown`] items.
    pub fn total_tax_amount(
        currency: &MonetaryCurrency,
        splits: &[TaxSplitBreakdown],
        mode: FineractRoundingMode,
    ) -> Money {
        let total: Decimal = splits.iter().map(|s| s.tax_amount.amount()).sum();
        Money::of(currency.clone(), total, mode)
    }

    /// Adds the calculated tax across `mappings` onto `base_amount` (or returns `base_amount` unchanged if `charge_includes_tax` is true).
    pub fn add_tax(
        base_amount: &Money,
        date: NaiveDate,
        mappings: &[TaxGroupMapping],
        scale: u32,
        mode: FineractRoundingMode,
        charge_includes_tax: bool,
    ) -> Result<(Money, Vec<TaxSplitBreakdown>), FineractError> {
        let splits = Self::split_tax_mappings(base_amount, date, mappings, scale, mode)?;
        if charge_includes_tax {
            return Ok((base_amount.clone(), splits));
        }
        let total_tax = Self::total_tax_amount(base_amount.currency(), &splits, mode);
        let gross = base_amount.plus(&total_tax, mode)?;
        Ok((gross, splits))
    }
}

/// Input validator ported from `TaxValidator.java`.
pub struct TaxValidator;

impl TaxValidator {
    /// Validates a `CREATE TAXCOMPONENT` command.
    pub fn validate_for_tax_component_create(command: &JsonCommand) -> Result<(), FineractError> {
        use tax_api_constants::*;
        let mut errors = Vec::new();
        let mut base = DataValidatorBuilder::new(&mut errors).resource("tax.component");

        base.reset()
            .parameter(NAME_PARAM_NAME)
            .value(command.string_value_of_parameter_named(NAME_PARAM_NAME))
            .not_blank();

        let percentage = command.big_decimal_value_of_parameter_named(PERCENTAGE_PARAM_NAME);
        base.reset()
            .parameter(PERCENTAGE_PARAM_NAME)
            .value(percentage)
            .not_null()
            .positive_amount()
            .not_greater_than_max(HUNDRED);

        let debit_type = command.integer_value_of_parameter_named(DEBIT_ACCOUNT_TYPE_PARAM_NAME);
        base.reset()
            .parameter(DEBIT_ACCOUNT_TYPE_PARAM_NAME)
            .value(debit_type)
            .ignore_if_null()
            .is_one_of_these_values(&[1, 2, 3, 4, 5]);

        let credit_type = command.integer_value_of_parameter_named(CREDIT_ACCOUNT_TYPE_PARAM_NAME);
        base.reset()
            .parameter(CREDIT_ACCOUNT_TYPE_PARAM_NAME)
            .value(credit_type)
            .ignore_if_null()
            .is_one_of_these_values(&[1, 2, 3, 4, 5]);

        let debit_id = command.long_value_of_parameter_named(DEBIT_ACCOUNT_ID_PARAM_NAME);
        if debit_type.is_some() {
            base.reset()
                .parameter(DEBIT_ACCOUNT_ID_PARAM_NAME)
                .value(debit_id)
                .not_null();
        } else {
            base.reset()
                .parameter(DEBIT_ACCOUNT_ID_PARAM_NAME)
                .value(debit_id)
                .must_be_blank_when_parameter_provided_is(DEBIT_ACCOUNT_TYPE_PARAM_NAME, debit_type);
        }

        let credit_id = command.long_value_of_parameter_named(CREDIT_ACCOUNT_ID_PARAM_NAME);
        if credit_type.is_some() {
            base.reset()
                .parameter(CREDIT_ACCOUNT_ID_PARAM_NAME)
                .value(credit_id)
                .not_null();
        } else {
            base.reset()
                .parameter(CREDIT_ACCOUNT_ID_PARAM_NAME)
                .value(credit_id)
                .must_be_blank_when_parameter_provided_is(
                    CREDIT_ACCOUNT_TYPE_PARAM_NAME,
                    credit_type,
                );
        }

        DataValidatorBuilder::throw_if_errors(errors)
    }

    /// Validates an `UPDATE TAXCOMPONENT` command.
    pub fn validate_for_tax_component_update(command: &JsonCommand) -> Result<(), FineractError> {
        use tax_api_constants::*;
        let mut errors = Vec::new();
        let mut base = DataValidatorBuilder::new(&mut errors).resource("tax.component");

        if command.parameter_exists(NAME_PARAM_NAME) {
            base.reset()
                .parameter(NAME_PARAM_NAME)
                .value(command.string_value_of_parameter_named(NAME_PARAM_NAME))
                .not_blank();
        }

        if command.parameter_exists(PERCENTAGE_PARAM_NAME) {
            let percentage = command.big_decimal_value_of_parameter_named(PERCENTAGE_PARAM_NAME);
            base.reset()
                .parameter(PERCENTAGE_PARAM_NAME)
                .value(percentage)
                .not_null()
                .positive_amount()
                .not_greater_than_max(HUNDRED);
        }

        if command.parameter_exists(START_DATE_PARAM_NAME) {
            base.reset()
                .parameter(START_DATE_PARAM_NAME)
                .value(command.local_date_value_of_parameter_named(START_DATE_PARAM_NAME))
                .not_null();
        }

        DataValidatorBuilder::throw_if_errors(errors)
    }

    /// Validates a `CREATE TAXGROUP` command.
    pub fn validate_for_tax_group_create(command: &JsonCommand) -> Result<(), FineractError> {
        use tax_api_constants::*;
        let mut errors = Vec::new();
        let mut base = DataValidatorBuilder::new(&mut errors).resource("tax.group");

        base.reset()
            .parameter(NAME_PARAM_NAME)
            .value(command.string_value_of_parameter_named(NAME_PARAM_NAME))
            .not_blank();

        let components = command.array_of_parameter_named(TAX_COMPONENTS_PARAM_NAME);
        base.reset()
            .parameter(TAX_COMPONENTS_PARAM_NAME)
            .value(components)
            .not_null();

        if let Some(arr) = components {
            if arr.is_empty() {
                base.reset()
                    .parameter(TAX_COMPONENTS_PARAM_NAME)
                    .fail_with_code("cannot.be.empty", "Tax components array cannot be empty");
            }
            for item in arr {
                let comp_id = item.get(TAX_COMPONENT_ID_PARAM_NAME).and_then(|v| v.as_i64());
                base.reset()
                    .parameter(&format!(
                        "{TAX_COMPONENTS_PARAM_NAME}.{TAX_COMPONENT_ID_PARAM_NAME}"
                    ))
                    .value(comp_id)
                    .not_null()
                    .long_greater_than_zero();
            }
        }

        DataValidatorBuilder::throw_if_errors(errors)
    }

    /// Validates an `UPDATE TAXGROUP` command.
    pub fn validate_for_tax_group_update(command: &JsonCommand) -> Result<(), FineractError> {
        use tax_api_constants::*;
        let mut errors = Vec::new();
        let mut base = DataValidatorBuilder::new(&mut errors).resource("tax.group");

        if command.parameter_exists(NAME_PARAM_NAME) {
            base.reset()
                .parameter(NAME_PARAM_NAME)
                .value(command.string_value_of_parameter_named(NAME_PARAM_NAME))
                .not_blank();
        }

        if command.parameter_exists(TAX_COMPONENTS_PARAM_NAME) {
            if let Some(arr) = command.array_of_parameter_named(TAX_COMPONENTS_PARAM_NAME) {
                for item in arr {
                    let mapping_id = item.get(ID_PARAM_NAME).and_then(|v| v.as_i64());
                    let comp_id = item.get(TAX_COMPONENT_ID_PARAM_NAME).and_then(|v| v.as_i64());
                    if mapping_id.is_none() {
                        base.reset()
                            .parameter(&format!(
                                "{TAX_COMPONENTS_PARAM_NAME}.{TAX_COMPONENT_ID_PARAM_NAME}"
                            ))
                            .value(comp_id)
                            .not_null()
                            .long_greater_than_zero();
                    } else {
                        base.reset()
                            .parameter(&format!("{TAX_COMPONENTS_PARAM_NAME}.{ID_PARAM_NAME}"))
                            .value(mapping_id)
                            .long_greater_than_zero();
                    }
                }
            }
        }

        DataValidatorBuilder::throw_if_errors(errors)
    }
}

#[derive(Debug, Default)]
struct InMemoryTaxStore {
    next_component_id: i64,
    next_history_id: i64,
    next_group_id: i64,
    next_mapping_id: i64,
    components: HashMap<i64, TaxComponent>,
    groups: HashMap<i64, TaxGroup>,
}

impl InMemoryTaxStore {
    fn new() -> Self {
        Self {
            next_component_id: 1,
            next_history_id: 1,
            next_group_id: 1,
            next_mapping_id: 1,
            components: HashMap::new(),
            groups: HashMap::new(),
        }
    }
}

/// Dual-database repository for `m_tax_component`, `m_tax_component_history`, `m_tax_group`,
/// and `m_tax_group_mappings` supporting both PostgreSQL (`$1..$N`) and MySQL/MariaDB (`?`)
/// via [`DialectSql`] as well as deterministic in-memory storage for unit/command testing.
#[derive(Debug, Clone)]
pub struct DualDbTaxRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    store: Arc<RwLock<InMemoryTaxStore>>,
}

impl Default for DualDbTaxRepository {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl DualDbTaxRepository {
    /// Creates an in-memory repository (default dialect: PostgreSQL).
    pub fn new_in_memory() -> Self {
        Self {
            pool: None,
            dialect: SqlDialect::Postgres,
            store: Arc::new(RwLock::new(InMemoryTaxStore::new())),
        }
    }

    /// Creates an in-memory repository configured for a specific [`SqlDialect`].
    pub fn new_with_dialect(dialect: SqlDialect) -> Self {
        Self {
            pool: None,
            dialect,
            store: Arc::new(RwLock::new(InMemoryTaxStore::new())),
        }
    }

    /// Creates a repository backed by a `sqlx::AnyPool` and [`SqlDialect`].
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            store: Arc::new(RwLock::new(InMemoryTaxStore::new())),
        }
    }

    #[inline]
    pub fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    #[inline]
    pub fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    /// Returns the dialect-adjusted SQL for inserting into `m_tax_component`.
    pub fn insert_tax_component_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_tax_component (id, name, percentage, debit_account_type_enum, debit_account_id, credit_account_type_enum, credit_account_id, start_date, createdby_id, created_date, lastmodifiedby_id, lastmodified_date) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for updating `m_tax_component`.
    pub fn update_tax_component_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_tax_component SET name = $1, percentage = $2, start_date = $3, lastmodifiedby_id = $4, lastmodified_date = $5 WHERE id = $6",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for inserting into `m_tax_component_history`.
    pub fn insert_tax_component_history_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_tax_component_history (id, tax_component_id, percentage, start_date, end_date, createdby_id, created_date, lastmodifiedby_id, lastmodified_date) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for selecting a `TaxComponent` by ID.
    pub fn select_tax_component_by_id_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, name, percentage, debit_account_type_enum, debit_account_id, credit_account_type_enum, credit_account_id, start_date FROM m_tax_component WHERE id = $1",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for selecting `TaxComponentHistory` rows for a component.
    pub fn select_tax_component_history_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, percentage, start_date, end_date FROM m_tax_component_history WHERE tax_component_id = $1 ORDER BY start_date ASC, id ASC",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for inserting into `m_tax_group`.
    pub fn insert_tax_group_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_tax_group (id, name, createdby_id, created_date, lastmodifiedby_id, lastmodified_date) VALUES ($1, $2, $3, $4, $5, $6)",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for updating `m_tax_group`.
    pub fn update_tax_group_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_tax_group SET name = $1, lastmodifiedby_id = $2, lastmodified_date = $3 WHERE id = $4",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for inserting into `m_tax_group_mappings`.
    pub fn insert_tax_group_mapping_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_tax_group_mappings (id, tax_group_id, tax_component_id, start_date, end_date, createdby_id, created_date, lastmodifiedby_id, lastmodified_date) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for updating `m_tax_group_mappings`.
    pub fn update_tax_group_mapping_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_tax_group_mappings SET end_date = $1, lastmodifiedby_id = $2, lastmodified_date = $3 WHERE id = $4 AND tax_group_id = $5",
            self.dialect,
        )
    }

    /// Returns the dialect-adjusted SQL for selecting a `TaxGroup` by ID.
    pub fn select_tax_group_by_id_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, name FROM m_tax_group WHERE id = $1",
            self.dialect,
        )
    }

    /// Allocates the next history ID in the repository's backing store.
    pub fn allocate_history_id(&self) -> i64 {
        let mut guard = self.store.write().expect("tax store lock poisoned");
        let id = guard.next_history_id;
        guard.next_history_id += 1;
        id
    }

    /// Allocates the next mapping ID in the repository's backing store.
    pub fn allocate_mapping_id(&self) -> i64 {
        let mut guard = self.store.write().expect("tax store lock poisoned");
        let id = guard.next_mapping_id;
        guard.next_mapping_id += 1;
        id
    }

    /// Saves or updates a [`TaxComponent`] in the repository, assigning an ID if `component.id <= 0`.
    pub fn save_tax_component(&self, mut component: TaxComponent) -> Result<TaxComponent, FineractError> {
        let mut guard = self.store.write().expect("tax store lock poisoned");
        if component.id <= 0 {
            component.id = guard.next_component_id;
            guard.next_component_id += 1;
        } else if component.id >= guard.next_component_id {
            guard.next_component_id = component.id + 1;
        }
        for h in &mut component.history {
            if h.id <= 0 {
                h.id = guard.next_history_id;
                guard.next_history_id += 1;
            } else if h.id >= guard.next_history_id {
                guard.next_history_id = h.id + 1;
            }
        }
        guard.components.insert(component.id, component.clone());
        Ok(component)
    }

    /// Retrieves a [`TaxComponent`] by ID, or returns `NotFound` (`error.msg.tax.component.id.invalid`).
    pub fn find_tax_component_by_id(&self, id: i64) -> Result<TaxComponent, FineractError> {
        let guard = self.store.read().expect("tax store lock poisoned");
        guard.components.get(&id).cloned().ok_or_else(|| {
            FineractError::not_found(
                "error.msg.tax.component.id.invalid",
                format!("Tax component with identifier {id} does not exist"),
            )
        })
    }

    /// Lists all [`TaxComponent`] entities sorted by ID.
    pub fn list_tax_components(&self) -> Vec<TaxComponent> {
        let guard = self.store.read().expect("tax store lock poisoned");
        let mut items: Vec<TaxComponent> = guard.components.values().cloned().collect();
        items.sort_by_key(|c| c.id);
        items
    }

    /// Saves or updates a [`TaxGroup`] in the repository after validating its mappings.
    pub fn save_tax_group(&self, mut group: TaxGroup) -> Result<TaxGroup, FineractError> {
        group.validate()?;
        let mut guard = self.store.write().expect("tax store lock poisoned");
        if group.id <= 0 {
            group.id = guard.next_group_id;
            guard.next_group_id += 1;
        } else if group.id >= guard.next_group_id {
            guard.next_group_id = group.id + 1;
        }
        for m in &mut group.mappings {
            if m.id <= 0 {
                m.id = guard.next_mapping_id;
                guard.next_mapping_id += 1;
            } else if m.id >= guard.next_mapping_id {
                guard.next_mapping_id = m.id + 1;
            }
        }
        guard.groups.insert(group.id, group.clone());
        Ok(group)
    }

    /// Retrieves a [`TaxGroup`] by ID, or returns `NotFound` (`error.msg.tax.group.id.invalid`).
    pub fn find_tax_group_by_id(&self, id: i64) -> Result<TaxGroup, FineractError> {
        let guard = self.store.read().expect("tax store lock poisoned");
        guard.groups.get(&id).cloned().ok_or_else(|| {
            FineractError::not_found(
                "error.msg.tax.group.id.invalid",
                format!("Tax group with identifier {id} does not exist"),
            )
        })
    }

    /// Lists all [`TaxGroup`] entities sorted by ID.
    pub fn list_tax_groups(&self) -> Vec<TaxGroup> {
        let guard = self.store.read().expect("tax store lock poisoned");
        let mut items: Vec<TaxGroup> = guard.groups.values().cloned().collect();
        items.sort_by_key(|g| g.id);
        items
    }

    /// Async query to load a [`TaxComponent`] and its history from a live `sqlx::AnyPool`.
    pub async fn fetch_tax_component_from_db(
        &self,
        id: i64,
    ) -> Result<TaxComponent, FineractError> {
        let Some(pool) = &self.pool else {
            return self.find_tax_component_by_id(id);
        };

        let comp_sql = self.select_tax_component_by_id_sql();
        let row = sqlx::query(&comp_sql)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| FineractError::Internal(format!("DB query error: {e}")))?
            .ok_or_else(|| {
                FineractError::not_found(
                    "error.msg.tax.component.id.invalid",
                    format!("Tax component with identifier {id} does not exist"),
                )
            })?;

        let hist_sql = self.select_tax_component_history_sql();
        let hist_rows = sqlx::query(&hist_sql)
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(|e| FineractError::Internal(format!("DB history query error: {e}")))?;

        let mut history = Vec::with_capacity(hist_rows.len());
        for h_row in hist_rows {
            let h_id: i64 = h_row.try_get("id").unwrap_or_default();
            let pct_str: String = h_row
                .try_get::<String, _>("percentage")
                .unwrap_or_else(|_| "0".to_string());
            let pct = pct_str.parse::<Decimal>().unwrap_or(Decimal::ZERO);
            let start_str: String = h_row.try_get("start_date").unwrap_or_default();
            let start_date = NaiveDate::parse_from_str(&start_str, "%Y-%m-%d")
                .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
            let end_date = h_row
                .try_get::<Option<String>, _>("end_date")
                .ok()
                .flatten()
                .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
            history.push(TaxComponentHistory::new(h_id, pct, start_date, end_date));
        }

        let name: String = row.try_get("name").unwrap_or_default();
        let pct_str: String = row
            .try_get::<String, _>("percentage")
            .unwrap_or_else(|_| "0".to_string());
        let percentage = pct_str.parse::<Decimal>().unwrap_or(Decimal::ZERO);
        let debit_account_type: Option<i16> = row.try_get("debit_account_type_enum").ok();
        let debit_account_id: Option<i64> = row.try_get("debit_account_id").ok();
        let credit_account_type: Option<i16> = row.try_get("credit_account_type_enum").ok();
        let credit_account_id: Option<i64> = row.try_get("credit_account_id").ok();
        let start_str: String = row.try_get("start_date").unwrap_or_default();
        let start_date = NaiveDate::parse_from_str(&start_str, "%Y-%m-%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());

        Ok(TaxComponent::new(
            id,
            name,
            percentage,
            debit_account_type,
            debit_account_id,
            credit_account_type,
            credit_account_id,
            start_date,
            history,
        ))
    }
}

fn parse_date_from_value(
    item: &Value,
    param_name: &str,
    fallback: NaiveDate,
) -> NaiveDate {
    if let Some(s) = item.get(param_name).and_then(|v| v.as_str()) {
        for fmt in ["%Y-%m-%d", "%d %B %Y", "%d %b %Y", "%m/%d/%Y", "%d/%m/%Y"] {
            if let Ok(d) = NaiveDate::parse_from_str(s.trim(), fmt) {
                return d;
            }
        }
    }
    fallback
}

/// Command handler for `CREATE TAXCOMPONENT` ported from `CreateTaxComponentCommandHandler.java`.
#[derive(Debug, Clone)]
pub struct CreateTaxComponentCommandHandler {
    repo: Arc<DualDbTaxRepository>,
}

impl CreateTaxComponentCommandHandler {
    pub fn new(repo: Arc<DualDbTaxRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler for CreateTaxComponentCommandHandler {
    fn action_name(&self) -> &'static str {
        "CREATE"
    }

    fn entity_name(&self) -> &'static str {
        "TAXCOMPONENT"
    }

    fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        use tax_api_constants::*;
        TaxValidator::validate_for_tax_component_create(command)?;

        let name = command
            .string_value_of_parameter_named(NAME_PARAM_NAME)
            .unwrap_or_default()
            .trim()
            .to_string();
        let percentage = command
            .big_decimal_value_of_parameter_named(PERCENTAGE_PARAM_NAME)
            .unwrap_or(Decimal::ZERO);
        let debit_account_type = command
            .integer_value_of_parameter_named(DEBIT_ACCOUNT_TYPE_PARAM_NAME)
            .map(|v| v as i16);
        let debit_account_id = command.long_value_of_parameter_named(DEBIT_ACCOUNT_ID_PARAM_NAME);
        let credit_account_type = command
            .integer_value_of_parameter_named(CREDIT_ACCOUNT_TYPE_PARAM_NAME)
            .map(|v| v as i16);
        let credit_account_id = command.long_value_of_parameter_named(CREDIT_ACCOUNT_ID_PARAM_NAME);
        let start_date = command
            .local_date_value_of_parameter_named(START_DATE_PARAM_NAME)
            .unwrap_or(ctx.business_date);

        let component = TaxComponent::new(
            0,
            name,
            percentage,
            debit_account_type,
            debit_account_id,
            credit_account_type,
            credit_account_id,
            start_date,
            Vec::new(),
        );
        let saved = self.repo.save_tax_component(component)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id(command.command_id())
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for `UPDATE TAXCOMPONENT` ported from `UpdateTaxComponentCommandHandler.java`.
#[derive(Debug, Clone)]
pub struct UpdateTaxComponentCommandHandler {
    repo: Arc<DualDbTaxRepository>,
}

impl UpdateTaxComponentCommandHandler {
    pub fn new(repo: Arc<DualDbTaxRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler for UpdateTaxComponentCommandHandler {
    fn action_name(&self) -> &'static str {
        "UPDATE"
    }

    fn entity_name(&self) -> &'static str {
        "TAXCOMPONENT"
    }

    fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        TaxValidator::validate_for_tax_component_update(command)?;

        let entity_id = command.entity_id().ok_or_else(|| {
            FineractError::not_found(
                "error.msg.tax.component.id.invalid",
                "Tax component identifier is required for update",
            )
        })?;

        let mut component = self.repo.find_tax_component_by_id(entity_id)?;
        let next_history_id = self.repo.allocate_history_id();
        let changes = component.apply_update(command, next_history_id)?;
        let saved = self.repo.save_tax_component(component)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id(command.command_id())
            .with_entity_id(saved.id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `CREATE TAXGROUP` ported from `CreateTaxGroupCommandHandler.java`.
#[derive(Debug, Clone)]
pub struct CreateTaxGroupCommandHandler {
    repo: Arc<DualDbTaxRepository>,
}

impl CreateTaxGroupCommandHandler {
    pub fn new(repo: Arc<DualDbTaxRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler for CreateTaxGroupCommandHandler {
    fn action_name(&self) -> &'static str {
        "CREATE"
    }

    fn entity_name(&self) -> &'static str {
        "TAXGROUP"
    }

    fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        use tax_api_constants::*;
        TaxValidator::validate_for_tax_group_create(command)?;

        let name = command
            .string_value_of_parameter_named(NAME_PARAM_NAME)
            .unwrap_or_default()
            .trim()
            .to_string();

        let arr = command
            .array_of_parameter_named(TAX_COMPONENTS_PARAM_NAME)
            .cloned()
            .unwrap_or_default();

        let mut mappings = Vec::with_capacity(arr.len());
        for item in &arr {
            let comp_id = item
                .get(TAX_COMPONENT_ID_PARAM_NAME)
                .and_then(|v| v.as_i64())
                .ok_or_else(|| {
                    FineractError::not_found(
                        "error.msg.tax.component.id.invalid",
                        "Tax component ID is required",
                    )
                })?;
            let tax_component = self.repo.find_tax_component_by_id(comp_id)?;
            let start_date = parse_date_from_value(item, START_DATE_PARAM_NAME, ctx.business_date);
            let end_date = item
                .get(END_DATE_PARAM_NAME)
                .filter(|v| !v.is_null())
                .map(|_| parse_date_from_value(item, END_DATE_PARAM_NAME, ctx.business_date));
            let mapping_id = self.repo.allocate_mapping_id();
            mappings.push(TaxGroupMapping::new(
                mapping_id,
                tax_component,
                start_date,
                end_date,
            ));
        }

        let group = TaxGroup::new(0, name, mappings);
        let saved = self.repo.save_tax_group(group)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id(command.command_id())
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for `UPDATE TAXGROUP` ported from `UpdateTaxGroupCommandHandler.java`.
#[derive(Debug, Clone)]
pub struct UpdateTaxGroupCommandHandler {
    repo: Arc<DualDbTaxRepository>,
}

impl UpdateTaxGroupCommandHandler {
    pub fn new(repo: Arc<DualDbTaxRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler for UpdateTaxGroupCommandHandler {
    fn action_name(&self) -> &'static str {
        "UPDATE"
    }

    fn entity_name(&self) -> &'static str {
        "TAXGROUP"
    }

    fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        use tax_api_constants::*;
        TaxValidator::validate_for_tax_group_update(command)?;

        let entity_id = command.entity_id().ok_or_else(|| {
            FineractError::not_found(
                "error.msg.tax.group.id.invalid",
                "Tax group identifier is required for update",
            )
        })?;

        let mut group = self.repo.find_tax_group_by_id(entity_id)?;
        let mut changes = BTreeMap::new();

        if command.is_change_in_string_parameter_named(NAME_PARAM_NAME, Some(&group.name)) {
            if let Some(new_name) = command.string_value_of_parameter_named(NAME_PARAM_NAME) {
                let new_name = new_name.trim().to_string();
                changes.insert(NAME_PARAM_NAME.to_string(), json!(new_name));
                group.name = new_name;
            }
        }

        if let Some(arr) = command
            .array_of_parameter_named(TAX_COMPONENTS_PARAM_NAME)
            .cloned()
        {
            let mut modified_mappings = Vec::new();
            for item in &arr {
                if let Some(mapping_id) = item.get(ID_PARAM_NAME).and_then(|v| v.as_i64()) {
                    let mapping = group.find_mapping_by_id_mut(mapping_id)?;
                    if item
                        .get(END_DATE_PARAM_NAME)
                        .filter(|v| !v.is_null())
                        .is_some()
                    {
                        let new_end =
                            parse_date_from_value(item, END_DATE_PARAM_NAME, ctx.business_date);
                        if mapping.end_date != Some(new_end) {
                            mapping.end_date = Some(new_end);
                            modified_mappings.push(json!({
                                ID_PARAM_NAME: mapping_id,
                                END_DATE_PARAM_NAME: new_end.format("%Y-%m-%d").to_string(),
                            }));
                        }
                    }
                } else {
                    let comp_id = item
                        .get(TAX_COMPONENT_ID_PARAM_NAME)
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| {
                            FineractError::not_found(
                                "error.msg.tax.component.id.invalid",
                                "Tax component ID is required for new mapping",
                            )
                        })?;
                    let tax_component = self.repo.find_tax_component_by_id(comp_id)?;
                    let start_date =
                        parse_date_from_value(item, START_DATE_PARAM_NAME, ctx.business_date);
                    let end_date = item
                        .get(END_DATE_PARAM_NAME)
                        .filter(|v| !v.is_null())
                        .map(|_| {
                            parse_date_from_value(item, END_DATE_PARAM_NAME, ctx.business_date)
                        });
                    let mapping_id = self.repo.allocate_mapping_id();
                    group.mappings.push(TaxGroupMapping::new(
                        mapping_id,
                        tax_component,
                        start_date,
                        end_date,
                    ));
                    modified_mappings.push(json!({
                        ID_PARAM_NAME: mapping_id,
                        TAX_COMPONENT_ID_PARAM_NAME: comp_id,
                        START_DATE_PARAM_NAME: start_date.format("%Y-%m-%d").to_string(),
                    }));
                }
            }

            if !modified_mappings.is_empty() {
                changes.insert(
                    TAX_COMPONENTS_PARAM_NAME.to_string(),
                    Value::Array(modified_mappings),
                );
            }
        }

        let saved = self.repo.save_tax_group(group)?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id(command.command_id())
            .with_entity_id(saved.id)
            .with_changes(changes)
            .build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).expect("valid decimal")
    }

    fn usd() -> MonetaryCurrency {
        MonetaryCurrency::new("USD", 2, None)
    }

    fn money_usd(s: &str) -> Money {
        Money::of(usd(), dec(s), FineractRoundingMode::HalfEven)
    }

    #[test]
    fn test_tax_account_type_conversions() {
        assert_eq!(TaxAccountType::Asset as i16, 1);
        assert_eq!(TaxAccountType::Liability as i16, 2);
        assert_eq!(TaxAccountType::Equity as i16, 3);
        assert_eq!(TaxAccountType::Income as i16, 4);
        assert_eq!(TaxAccountType::Expense as i16, 5);

        assert_eq!(TaxAccountType::from_i16(1), Some(TaxAccountType::Asset));
        assert_eq!(TaxAccountType::from_i16(2), Some(TaxAccountType::Liability));
        assert_eq!(TaxAccountType::from_i16(6), None);
        assert!(TaxAccountType::Liability.is_liability_type());
        assert_eq!(
            TaxAccountType::Liability.code(),
            "accountType.liability"
        );
    }

    #[test]
    fn test_tax_component_date_effective_history() {
        // Ported from TaxComponentTest.java:
        // Initial 5% starting 2023-01-01, updated to 7% starting 2024-03-01,
        // then startDate moved to 2024-04-01.
        let mut comp = TaxComponent::new(
            1,
            "VAT",
            dec("5.0"),
            None,
            None,
            Some(TaxAccountType::Liability.as_i16()),
            Some(101),
            d(2023, 1, 1),
            vec![],
        );

        // Before start date -> None
        assert_eq!(comp.applicable_percentage_on(d(2022, 12, 31)), None);
        // On and after start date -> 5.0
        assert_eq!(comp.applicable_percentage_on(d(2023, 1, 1)), Some(dec("5.0")));
        assert_eq!(comp.applicable_percentage_on(d(2023, 6, 15)), Some(dec("5.0")));

        // Step 1: Update percentage to 7% with startDate 2024-03-01
        let cmd1 = JsonCommand::from_raw(
            None,
            r#"{"percentage": 7.0, "startDate": "2024-03-01", "dateFormat": "yyyy-MM-dd", "locale": "en"}"#,
        )
        .unwrap();
        let changes1 = comp.apply_update(&cmd1, 10).unwrap();
        assert!(changes1.contains_key("percentage"));
        assert_eq!(comp.history.len(), 1);
        assert_eq!(comp.history[0].percentage, dec("5.0"));
        assert_eq!(comp.history[0].start_date, d(2023, 1, 1));
        assert_eq!(comp.history[0].end_date, Some(d(2024, 3, 1)));

        // Step 2: Move startDate to 2024-04-01 without changing percentage
        let cmd2 = JsonCommand::from_raw(
            None,
            r#"{"startDate": "2024-04-01", "dateFormat": "yyyy-MM-dd", "locale": "en"}"#,
        )
        .unwrap();
        comp.apply_update(&cmd2, 11).unwrap();
        assert_eq!(comp.start_date, d(2024, 4, 1));
        assert_eq!(comp.history[0].end_date, Some(d(2024, 4, 1)));

        // Query on 2024-03-15 -> falls in history [2023-01-01, 2024-04-01] -> 5.0%
        assert_eq!(
            comp.applicable_percentage_on(d(2024, 3, 15)),
            Some(dec("5.0"))
        );
        // Query on 2024-04-01 and 2024-04-02 -> current rate 7.0%
        assert_eq!(
            comp.applicable_percentage_on(d(2024, 4, 1)),
            Some(dec("7.0"))
        );
        assert_eq!(
            comp.applicable_percentage_on(d(2024, 4, 2)),
            Some(dec("7.0"))
        );
    }

    #[test]
    fn test_multi_component_tax_group_split_and_income_amount() {
        // Component 1: CGST 9% (credit account 201)
        let cgst = TaxComponent::new(
            1,
            "CGST",
            dec("9.0"),
            None,
            None,
            Some(TaxAccountType::Liability.as_i16()),
            Some(201),
            d(2024, 1, 1),
            vec![],
        );
        // Component 2: SGST 9% (credit account 202)
        let sgst = TaxComponent::new(
            2,
            "SGST",
            dec("9.0"),
            None,
            None,
            Some(TaxAccountType::Liability.as_i16()),
            Some(202),
            d(2024, 1, 1),
            vec![],
        );
        // Component 3: Expired surcharge 2% (ended 2024-05-31)
        let expired = TaxComponent::new(
            3,
            "Expired Surcharge",
            dec("2.0"),
            None,
            None,
            Some(TaxAccountType::Liability.as_i16()),
            Some(203),
            d(2024, 1, 1),
            vec![],
        );

        let mappings = vec![
            TaxGroupMapping::new(10, cgst, d(2024, 1, 1), None),
            TaxGroupMapping::new(11, sgst, d(2024, 1, 1), None),
            TaxGroupMapping::new(12, expired, d(2024, 1, 1), Some(d(2024, 5, 31))),
        ];
        let tax_group = TaxGroup::new(100, "GST 18%", mappings.clone());

        let gross = money_usd("250.00");

        // On 2024-06-15, only CGST (9%) and SGST (9%) are active -> 22.50 each, total tax 45.00, net income 205.00
        let splits = TaxUtil::split_tax(
            &gross,
            d(2024, 6, 15),
            &tax_group,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();
        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].tax_component_id, 1);
        assert_eq!(splits[0].percentage, dec("9.0"));
        assert_eq!(splits[0].tax_amount, money_usd("22.50"));
        assert_eq!(splits[0].credit_account_id, Some(201));

        assert_eq!(splits[1].tax_component_id, 2);
        assert_eq!(splits[1].percentage, dec("9.0"));
        assert_eq!(splits[1].tax_amount, money_usd("22.50"));
        assert_eq!(splits[1].credit_account_id, Some(202));

        let (net_income, income_splits) = TaxUtil::income_amount(
            &gross,
            d(2024, 6, 15),
            &mappings,
            2,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();
        assert_eq!(income_splits.len(), 2);
        assert_eq!(net_income, money_usd("205.00"));

        // On 2024-03-15, all 3 components are active (9% + 9% + 2% = 20%) -> total tax 50.00, net income 200.00
        let (net_march, splits_march) = TaxUtil::income_amount(
            &gross,
            d(2024, 3, 15),
            &mappings,
            2,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();
        assert_eq!(splits_march.len(), 3);
        assert_eq!(net_march, money_usd("200.00"));
    }

    #[test]
    fn test_total_percentage_greater_than_100_validation_error() {
        let comp_a = TaxComponent::new(
            1,
            "Tax A",
            dec("60.0"),
            None,
            None,
            Some(TaxAccountType::Liability.as_i16()),
            Some(10),
            d(2024, 1, 1),
            vec![],
        );
        let comp_b = TaxComponent::new(
            2,
            "Tax B",
            dec("55.0"),
            None,
            None,
            Some(TaxAccountType::Liability.as_i16()),
            Some(11),
            d(2024, 1, 1),
            vec![],
        );

        let mappings = vec![
            TaxGroupMapping::new(1, comp_a, d(2024, 1, 1), None),
            TaxGroupMapping::new(2, comp_b, d(2024, 1, 1), None),
        ];
        let group = TaxGroup::new(1, "Invalid >100% Group", mappings.clone());
        let amount = money_usd("100.00");

        let err = TaxUtil::split_tax(
            &amount,
            d(2024, 6, 1),
            &group,
            FineractRoundingMode::HalfEven,
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains(TAX_GROUP_TOTAL_PERCENTAGE_MORE_THAN_MAX_CODE),
            "expected error containing {TAX_GROUP_TOTAL_PERCENTAGE_MORE_THAN_MAX_CODE}, got: {err}"
        );

        let err_income = TaxUtil::income_amount(
            &amount,
            d(2024, 6, 1),
            &mappings,
            2,
            FineractRoundingMode::HalfEven,
        )
        .unwrap_err();
        assert!(
            err_income
                .to_string()
                .contains(TAX_GROUP_TOTAL_PERCENTAGE_MORE_THAN_MAX_CODE)
        );
    }

    #[test]
    fn test_command_handlers_and_dual_db_repository() {
        let repo = Arc::new(DualDbTaxRepository::new_in_memory());
        let create_comp_handler = CreateTaxComponentCommandHandler::new(repo.clone());
        let update_comp_handler = UpdateTaxComponentCommandHandler::new(repo.clone());
        let create_group_handler = CreateTaxGroupCommandHandler::new(repo.clone());
        let update_group_handler = UpdateTaxGroupCommandHandler::new(repo.clone());

        let ctx = ExecutionContext::new("default", 1, d(2025, 1, 1));

        // 1. Create Tax Component 1 (60%)
        let cmd_c1 = JsonCommand::from_raw(
            Some(1),
            r#"{"name": "Comp 1", "percentage": 60.0, "creditAccountType": 2, "creditAccountId": 501, "startDate": "2025-01-01"}"#,
        )
        .unwrap();
        let res_c1 = create_comp_handler.handle(&ctx, &cmd_c1).unwrap();
        assert_eq!(res_c1.resource_id, Some(1));

        // 2. Create Tax Component 2 (50%)
        let cmd_c2 = JsonCommand::from_raw(
            Some(2),
            r#"{"name": "Comp 2", "percentage": 50.0, "creditAccountType": 2, "creditAccountId": 502, "startDate": "2025-01-01"}"#,
        )
        .unwrap();
        let res_c2 = create_comp_handler.handle(&ctx, &cmd_c2).unwrap();
        assert_eq!(res_c2.resource_id, Some(2));

        // 3. Attempt to create a Tax Group with Comp 1 (60%) + Comp 2 (50%) = 110% -> must fail with >100% error
        let cmd_bad_group = JsonCommand::from_raw(
            Some(3),
            r#"{"name": "Over 100 Group", "taxComponents": [{"taxComponentId": 1, "startDate": "2025-01-01"}, {"taxComponentId": 2, "startDate": "2025-01-01"}]}"#,
        )
        .unwrap();
        let group_err = create_group_handler.handle(&ctx, &cmd_bad_group).unwrap_err();
        assert!(
            group_err
                .to_string()
                .contains(TAX_GROUP_TOTAL_PERCENTAGE_MORE_THAN_MAX_CODE)
        );

        // 4. Update Comp 1 to 10% starting 2025-02-01
        let cmd_u1 = JsonCommand::from_raw(
            Some(4),
            r#"{"percentage": 10.0, "startDate": "2025-02-01"}"#,
        )
        .unwrap()
        .with_entity_id(1);
        update_comp_handler.handle(&ctx, &cmd_u1).unwrap();

        // 5. Create valid Tax Group with Comp 1 starting 2025-02-01 (10%) and Comp 2 starting 2025-02-01 (50%) = 60%
        let cmd_good_group = JsonCommand::from_raw(
            Some(5),
            r#"{"name": "Valid Group", "taxComponents": [{"taxComponentId": 1, "startDate": "2025-02-01"}, {"taxComponentId": 2, "startDate": "2025-02-01"}]}"#,
        )
        .unwrap();
        let res_group = create_group_handler.handle(&ctx, &cmd_good_group).unwrap();
        let group_id = res_group.resource_id.unwrap();

        // 6. Update Tax Group name and mapping endDate
        let cmd_update_group = JsonCommand::from_raw(
            Some(6),
            r#"{"name": "Updated Group", "taxComponents": [{"id": 1, "endDate": "2025-12-31"}]}"#,
        )
        .unwrap()
        .with_entity_id(group_id);
        let res_ug = update_group_handler.handle(&ctx, &cmd_update_group).unwrap();
        assert_eq!(res_ug.resource_id, Some(group_id));

        let saved_group = repo.find_tax_group_by_id(group_id).unwrap();
        assert_eq!(saved_group.name, "Updated Group");
        assert_eq!(saved_group.mappings[0].end_date, Some(d(2025, 12, 31)));

        // Verify dual-DB SQL placeholder rewriting for both Postgres and MySQL
        let pg_repo = DualDbTaxRepository::new_with_dialect(SqlDialect::Postgres);
        let mysql_repo = DualDbTaxRepository::new_with_dialect(SqlDialect::MySql);
        assert!(pg_repo.insert_tax_component_sql().contains("$1"));
        assert!(!mysql_repo.insert_tax_component_sql().contains("$1"));
        assert!(mysql_repo.insert_tax_component_sql().contains('?'));
    }
}
