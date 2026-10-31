//! `interest_engine` — Savings Compounding & Posting Interest Calculation Engine.
//!
//! Ported from `org.apache.fineract.portfolio.savings.domain.interest.*` and
//! `org.apache.fineract.portfolio.savings.domain.SavingsHelper`:
//! - [`EndOfDayBalance`] (`EndOfDayBalance.java`)
//! - [`CompoundingPeriod`] (`CompoundingPeriod.java`, `DailyCompoundingPeriod.java`,
//!   `MonthlyCompoundingPeriod.java`, `QuarterlyCompoundingPeriod.java`,
//!   `BiAnnualCompoundingPeriod.java`, `AnnualCompoundingPeriod.java`)
//! - [`PostingPeriod`] (`PostingPeriod.java`)
//! - [`CompoundInterestValues`] (`CompoundInterestValues.java`)
//! - [`SavingsAccountTransactionDetailsForPostingPeriod`] (`SavingsAccountTransactionDetailsForPostingPeriod.java`)
//! - [`CompoundInterestEngine`] / [`CompoundInterestHelper`] (`CompoundInterestHelper.java`, `SavingsHelper.java`)

use std::collections::BTreeSet;

use chrono::{Datelike, Duration, NaiveDate};
use fineract_money::{Decimal, FineractDecimal, FineractRoundingMode, MonetaryCurrency, Money};
use fineract_tax_rs::{TaxGroup, TaxSplitBreakdown, TaxUtil};
use fineract_validation_rs::FineractError;
use rust_decimal::RoundingStrategy;
use serde::{Deserialize, Serialize};

use crate::product_and_charts::{
    SavingsCompoundingInterestPeriodType, SavingsInterestCalculationDaysInYearType,
    SavingsInterestCalculationType, SavingsPostingInterestPeriodType,
};

/// Internal scale used by Fineract's `EndOfDayBalance` and `CompoundingPeriod`
/// (`setScale(9, MoneyHelper.getRoundingMode())`).
pub const INTEREST_INTERMEDIATE_SCALE: u32 = 9;

// =============================================================================
// 0. Enum Discriminant & Calendar Helpers
// =============================================================================

#[inline]
const fn compounding_period_code(period_type: SavingsCompoundingInterestPeriodType) -> i32 {
    match period_type {
        SavingsCompoundingInterestPeriodType::Invalid => 0,
        SavingsCompoundingInterestPeriodType::Daily => 1,
        SavingsCompoundingInterestPeriodType::Monthly => 4,
        SavingsCompoundingInterestPeriodType::Quarterly => 5,
        SavingsCompoundingInterestPeriodType::BiAnnual => 6,
        SavingsCompoundingInterestPeriodType::Annual => 7,
    }
}

#[inline]
const fn posting_period_code(period_type: SavingsPostingInterestPeriodType) -> i32 {
    match period_type {
        SavingsPostingInterestPeriodType::Invalid => 0,
        SavingsPostingInterestPeriodType::Monthly => 4,
        SavingsPostingInterestPeriodType::Quarterly => 5,
        SavingsPostingInterestPeriodType::BiAnnual => 6,
        SavingsPostingInterestPeriodType::Annual => 7,
    }
}

#[inline]
const fn calculation_type_code(calc_type: SavingsInterestCalculationType) -> i32 {
    match calc_type {
        SavingsInterestCalculationType::Invalid => 0,
        SavingsInterestCalculationType::DailyBalance => 1,
        SavingsInterestCalculationType::AverageDailyBalance => 2,
    }
}

#[inline]
const fn days_in_year_value(days_type: SavingsInterestCalculationDaysInYearType) -> i64 {
    match days_type {
        SavingsInterestCalculationDaysInYearType::Invalid => 0,
        SavingsInterestCalculationDaysInYearType::Days360 => 360,
        SavingsInterestCalculationDaysInYearType::Days365 => 365,
    }
}

/// Normalizes an annual interest rate (given as a percentage such as `12.0` for 12%)
/// into a fractional rate (`0.12`).
#[inline]
fn rate_percentage_to_fraction(annual_rate_percentage: Decimal) -> Decimal {
    if annual_rate_percentage.is_zero() {
        Decimal::ZERO
    } else {
        annual_rate_percentage / Decimal::ONE_HUNDRED
    }
}

/// Deterministic `Decimal` exponentiation `base^exp` without floating-point arithmetic.
///
/// Replaces Java's `Math.pow(interestRateForCompoundingPeriodPlusOne.doubleValue(), numberOfDays)`
/// in `EndOfDayBalance.calculateInterestOnBalanceAndInterest` while honoring Constitutional Invariant #1.
#[inline]
fn decimal_pow(mut base: Decimal, mut exp: u64) -> Decimal {
    let mut result = Decimal::ONE;
    while exp > 0 {
        if exp & 1 == 1 {
            result = (result * base).round_dp_with_strategy(18, RoundingStrategy::MidpointNearestEven);
        }
        exp >>= 1;
        if exp > 0 {
            base = (base * base).round_dp_with_strategy(18, RoundingStrategy::MidpointNearestEven);
        }
    }
    result
}

/// Returns the number of days in the inclusive interval `[start, end]`, matching
/// `LocalDateInterval.daysInPeriodInclusiveOfEndDate()`.
#[inline]
pub fn days_in_period_inclusive_of_end_date(start: NaiveDate, end: NaiveDate) -> i64 {
    if end < start {
        0
    } else {
        (end - start).num_days() + 1
    }
}

/// Checks whether `outer` completely contains `inner` (`outer.0 <= inner.0 && inner.1 <= outer.1`).
#[inline]
pub fn interval_contains(outer: (NaiveDate, NaiveDate), inner: (NaiveDate, NaiveDate)) -> bool {
    outer.0 <= inner.0 && inner.1 <= outer.1
}

/// Checks whether `date` lies within `[interval.0, interval.1]`.
#[inline]
pub fn interval_contains_date(interval: (NaiveDate, NaiveDate), date: NaiveDate) -> bool {
    date >= interval.0 && date <= interval.1
}

/// Checks whether `a` and `b` share at least one day (`LocalDateInterval.containsPortionOf`).
#[inline]
pub fn interval_contains_portion_of(a: (NaiveDate, NaiveDate), b: (NaiveDate, NaiveDate)) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}

/// Returns the number of days in `(year, month)`.
#[inline]
pub const fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            if is_leap {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Returns the last calendar day of `(year, month)`.
#[inline]
pub fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let d = days_in_month(year, month);
    NaiveDate::from_ymd_opt(year, month, d).expect("valid year/month/day")
}

/// Adds `months` to `date`, clamping the day of month to the target month's length
/// (matching `LocalDate.plusMonths` in `java.time`).
#[inline]
pub fn add_months_clamped(date: NaiveDate, months: u32) -> NaiveDate {
    let total_months = (date.year() as i64) * 12 + (date.month0() as i64) + (months as i64);
    let target_year = (total_months.div_euclid(12)) as i32;
    let target_month = (total_months.rem_euclid(12) as u32) + 1;
    let max_day = days_in_month(target_year, target_month);
    let target_day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(target_year, target_month, target_day).expect("valid plusMonths date")
}

/// Adjusts `date` to `min(anniversary_day, date.lengthOfMonth())`
/// (matching `SavingsHelper.adjustToAnniversaryDay`).
#[inline]
fn adjust_to_anniversary_day(date: NaiveDate, anniversary_day: u32) -> NaiveDate {
    let max_day = days_in_month(date.year(), date.month());
    let day = anniversary_day.min(max_day).max(1);
    NaiveDate::from_ymd_opt(date.year(), date.month(), day).expect("valid anniversary date")
}

// =============================================================================
// 1. CompoundInterestValues (CompoundInterestValues.java)
// =============================================================================

/// Mutable accumulator carrying compounded and uncompounded interest across
/// [`CompoundingPeriod`] and [`PostingPeriod`] boundaries.
///
/// Ported from `org.apache.fineract.portfolio.savings.domain.interest.CompoundInterestValues`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompoundInterestValues {
    pub compounded_interest: Decimal,
    pub uncompounded_interest: Decimal,
}

impl CompoundInterestValues {
    #[inline]
    pub const fn new(compounded_interest: Decimal, uncompounded_interest: Decimal) -> Self {
        Self {
            compounded_interest,
            uncompounded_interest,
        }
    }

    #[inline]
    pub const fn zero() -> Self {
        Self {
            compounded_interest: Decimal::ZERO,
            uncompounded_interest: Decimal::ZERO,
        }
    }

    #[inline]
    pub const fn get_compounded_interest(&self) -> Decimal {
        self.compounded_interest
    }

    #[inline]
    pub const fn get_uncompounded_interest(&self) -> Decimal {
        self.uncompounded_interest
    }

    #[inline]
    pub fn set_compounded_interest(&mut self, interest_to_be_compounded: Decimal) {
        self.compounded_interest = interest_to_be_compounded;
    }

    #[inline]
    pub fn set_uncompounded_interest(&mut self, interest_to_be_uncompounded: Decimal) {
        self.uncompounded_interest = interest_to_be_uncompounded;
    }

    #[inline]
    pub fn set_zero_for_interest_to_be_uncompounded(&mut self) {
        self.uncompounded_interest = Decimal::ZERO;
    }
}

// =============================================================================
// 2. EndOfDayBalance (EndOfDayBalance.java)
// =============================================================================

/// Contiguous slice of days during which an account holds a constant `end_of_day_balance`.
///
/// Ported from `org.apache.fineract.portfolio.savings.domain.interest.EndOfDayBalance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndOfDayBalance {
    pub date: NaiveDate,
    pub opening_balance: Money,
    pub end_of_day_balance: Money,
    pub number_of_days: i64,
}

impl EndOfDayBalance {
    /// Creates a new [`EndOfDayBalance`] slice.
    #[inline]
    pub const fn new(
        date: NaiveDate,
        opening_balance: Money,
        end_of_day_balance: Money,
        number_of_days: i64,
    ) -> Self {
        Self {
            date,
            opening_balance,
            end_of_day_balance,
            number_of_days,
        }
    }

    /// Factory constructor matching `EndOfDayBalance.from(date, openingBalance, endOfDayBalance, numberOfDays)`.
    #[inline]
    pub const fn from(
        date: NaiveDate,
        opening_balance: Money,
        end_of_day_balance: Money,
        number_of_days: i64,
    ) -> Self {
        Self::new(date, opening_balance, end_of_day_balance, number_of_days)
    }

    /// Start date of this balance slice.
    #[inline]
    pub const fn date(&self) -> NaiveDate {
        self.date
    }

    /// Opening balance at the start of `self.date`.
    #[inline]
    pub const fn opening_balance(&self) -> &Money {
        &self.opening_balance
    }

    /// Closing / end-of-day balance held for `self.number_of_days`.
    #[inline]
    pub const fn closing_balance(&self) -> &Money {
        &self.end_of_day_balance
    }

    /// Alias for [`Self::closing_balance`].
    #[inline]
    pub const fn end_of_day_balance(&self) -> &Money {
        &self.end_of_day_balance
    }

    /// Number of days this end-of-day balance remains in effect.
    #[inline]
    pub const fn number_of_days(&self) -> i64 {
        self.number_of_days
    }

    /// Alias matching Java `getNumberOfDays()`.
    #[inline]
    pub const fn get_number_of_days(&self) -> i64 {
        self.number_of_days
    }

    /// Inclusive end date of this balance slice (`date + (number_of_days - 1)`).
    #[inline]
    pub fn end_date(&self) -> NaiveDate {
        self.date + Duration::days((self.number_of_days - 1).max(0))
    }

    /// Computes `(end_of_day_balance + interest_to_compound) * number_of_days` scaled to 9 decimal places
    /// using `FineractRoundingMode::default()` (`HalfEven`), matching `EndOfDayBalance.cumulativeBalance`.
    #[inline]
    pub fn cumulative_balance(&self, interest_to_compound: Decimal) -> Decimal {
        self.cumulative_balance_with_mode(interest_to_compound, FineractRoundingMode::default())
    }

    /// Computes `(end_of_day_balance + interest_to_compound) * number_of_days` scaled to 9 decimal places
    /// using the specified `rounding_mode`.
    #[inline]
    pub fn cumulative_balance_with_mode(
        &self,
        interest_to_compound: Decimal,
        rounding_mode: FineractRoundingMode,
    ) -> Decimal {
        if self.number_of_days <= 0 {
            return Decimal::ZERO;
        }
        let real_balance = self.end_of_day_balance.amount() + interest_to_compound;
        let product = real_balance * Decimal::from(self.number_of_days);
        FineractDecimal::new(product)
            .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
            .into_inner()
    }

    /// Calculates simple daily-balance interest on `(end_of_day_balance + interest_to_compound)`
    /// over `self.number_of_days` using fractional rates, matching `EndOfDayBalance.calculateInterestOnBalance`.
    ///
    /// - Positive balances earn at `interest_rate_as_fraction` only if `real_balance >= min_balance_for_interest_calculation`.
    /// - Negative balances incur negative overdraft interest at `overdraft_interest_rate_as_fraction` only if
    ///   `real_balance < -min_overdraft_for_interest_calculation`.
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_interest_on_balance(
        &self,
        interest_to_compound: Decimal,
        interest_rate_as_fraction: Decimal,
        days_in_year: i64,
        min_balance_for_interest_calculation: Decimal,
        overdraft_interest_rate_as_fraction: Decimal,
        min_overdraft_for_interest_calculation: Decimal,
        rounding_mode: FineractRoundingMode,
    ) -> Decimal {
        if self.number_of_days <= 0 || days_in_year <= 0 {
            return FineractDecimal::ZERO
                .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                .into_inner();
        }

        let real_balance = self.end_of_day_balance.amount() + interest_to_compound;
        let days = Decimal::from(self.number_of_days);
        let year_days = Decimal::from(days_in_year);

        if real_balance >= Decimal::ZERO {
            if real_balance >= min_balance_for_interest_calculation
                && interest_rate_as_fraction > Decimal::ZERO
            {
                let raw = (real_balance * interest_rate_as_fraction * days) / year_days;
                return FineractDecimal::new(raw)
                    .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                    .into_inner();
            }
        } else {
            let min_overdraft_threshold = -(min_overdraft_for_interest_calculation.abs());
            if real_balance < min_overdraft_threshold
                && overdraft_interest_rate_as_fraction > Decimal::ZERO
            {
                let raw = (real_balance * overdraft_interest_rate_as_fraction * days) / year_days;
                return FineractDecimal::new(raw)
                    .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                    .into_inner();
            }
        }

        FineractDecimal::ZERO
            .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
            .into_inner()
    }

    /// Calculates daily-compounded interest $FV - PV = PV \times (1 + r / \text{daysInYear})^n - PV$
    /// over `self.number_of_days` using fractional rates, matching
    /// `EndOfDayBalance.calculateInterestOnBalanceAndInterest`.
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_interest_on_balance_and_interest(
        &self,
        interest_to_compound: Decimal,
        interest_rate_as_fraction: Decimal,
        days_in_year: i64,
        min_balance_for_interest_calculation: Decimal,
        overdraft_interest_rate_as_fraction: Decimal,
        min_overdraft_for_interest_calculation: Decimal,
        rounding_mode: FineractRoundingMode,
    ) -> Decimal {
        if self.number_of_days <= 0 || days_in_year <= 0 {
            return FineractDecimal::ZERO
                .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                .into_inner();
        }

        let present_value = self.end_of_day_balance.amount() + interest_to_compound;
        let present_value_scaled = FineractDecimal::new(present_value)
            .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
            .into_inner();
        let mut future_value = present_value_scaled;
        let year_days = Decimal::from(days_in_year);
        let n = self.number_of_days as u64;

        if present_value >= Decimal::ZERO {
            if present_value >= min_balance_for_interest_calculation
                && interest_rate_as_fraction > Decimal::ZERO
            {
                let r = interest_rate_as_fraction / year_days;
                let factor = decimal_pow(Decimal::ONE + r, n);
                future_value = FineractDecimal::new(present_value * factor)
                    .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                    .into_inner();
            }
        } else {
            let min_overdraft_threshold = -(min_overdraft_for_interest_calculation.abs());
            if present_value < min_overdraft_threshold
                && overdraft_interest_rate_as_fraction > Decimal::ZERO
            {
                let r = overdraft_interest_rate_as_fraction / year_days;
                let factor = decimal_pow(Decimal::ONE + r, n);
                future_value = FineractDecimal::new(present_value * factor)
                    .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                    .into_inner();
            }
        }

        future_value - present_value
    }

    /// Calculates interest on this [`EndOfDayBalance`] slice using the Daily Balance method.
    ///
    /// - Positive savings interest uses `annual_interest_rate` (percentage, e.g. `12.0` for 12% p.a.)
    ///   when `(end_of_day_balance + interest_to_compound) >= min_balance_for_interest_calculation`.
    ///   If the balance is below `min_balance_for_interest_calculation`, returns `0`.
    /// - Negative overdraft interest uses `nominal_annual_interest_rate_overdraft` (percentage)
    ///   when `end_of_day_balance < 0`, `allow_overdraft` is `true`, and the overdraft exceeds
    ///   `min_overdraft_for_interest_calculation`.
    /// - When `compound_daily` is `true`, applies daily compounding $PV \times ((1 + r)^n - 1)$ across
    ///   `self.number_of_days`; otherwise applies linear daily interest $PV \times r \times n$.
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_interest_on_daily_balance(
        &self,
        interest_to_compound: Decimal,
        annual_interest_rate: Decimal,
        days_in_year: i64,
        min_balance_for_interest_calculation: Option<Decimal>,
        allow_overdraft: bool,
        nominal_annual_interest_rate_overdraft: Option<Decimal>,
        min_overdraft_for_interest_calculation: Option<Decimal>,
        compound_daily: bool,
        rounding_mode: FineractRoundingMode,
    ) -> Decimal {
        let rate_fraction = rate_percentage_to_fraction(annual_interest_rate);
        let min_balance = min_balance_for_interest_calculation.unwrap_or(Decimal::ZERO);
        let overdraft_rate_fraction = if allow_overdraft {
            rate_percentage_to_fraction(
                nominal_annual_interest_rate_overdraft.unwrap_or(Decimal::ZERO),
            )
        } else {
            Decimal::ZERO
        };
        let min_overdraft = min_overdraft_for_interest_calculation.unwrap_or(Decimal::ZERO);

        if compound_daily {
            self.calculate_interest_on_balance_and_interest(
                interest_to_compound,
                rate_fraction,
                days_in_year,
                min_balance,
                overdraft_rate_fraction,
                min_overdraft,
                rounding_mode,
            )
        } else {
            self.calculate_interest_on_balance(
                interest_to_compound,
                rate_fraction,
                days_in_year,
                min_balance,
                overdraft_rate_fraction,
                min_overdraft,
                rounding_mode,
            )
        }
    }

    /// Calculates interest using the Average Daily Balance method on this slice (or via
    /// [`Self::calculate_average_daily_balance_interest_for_slice`] across multiple slices).
    ///
    /// When `opening_balance` and `end_of_day_balance` differ on a standalone slice, uses the
    /// midpoint `(opening_balance + end_of_day_balance) / 2 + interest_to_compound` as the
    /// average balance of the slice; when equal, equals `end_of_day_balance + interest_to_compound`.
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_interest_on_average_daily_balance(
        &self,
        interest_to_compound: Decimal,
        annual_interest_rate: Decimal,
        days_in_year: i64,
        min_balance_for_interest_calculation: Option<Decimal>,
        allow_overdraft: bool,
        nominal_annual_interest_rate_overdraft: Option<Decimal>,
        min_overdraft_for_interest_calculation: Option<Decimal>,
        rounding_mode: FineractRoundingMode,
    ) -> Decimal {
        if self.number_of_days <= 0 || days_in_year <= 0 {
            return FineractDecimal::ZERO
                .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                .into_inner();
        }

        let base_avg = if self.opening_balance.amount() != self.end_of_day_balance.amount() {
            (self.opening_balance.amount() + self.end_of_day_balance.amount()) / Decimal::from(2)
        } else {
            self.end_of_day_balance.amount()
        };
        let average_daily_balance = FineractDecimal::new(base_avg + interest_to_compound)
            .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
            .into_inner();

        let min_balance = min_balance_for_interest_calculation.unwrap_or(Decimal::ZERO);
        let min_overdraft = min_overdraft_for_interest_calculation
            .unwrap_or(Decimal::ZERO)
            .abs();
        let days = Decimal::from(self.number_of_days);
        let year_days = Decimal::from(days_in_year);

        if average_daily_balance >= Decimal::ZERO {
            let rate_fraction = rate_percentage_to_fraction(annual_interest_rate);
            if average_daily_balance >= min_balance && rate_fraction > Decimal::ZERO {
                let raw = (average_daily_balance * rate_fraction * days) / year_days;
                return FineractDecimal::new(raw)
                    .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                    .into_inner();
            }
        } else if allow_overdraft {
            let overdraft_rate_fraction = rate_percentage_to_fraction(
                nominal_annual_interest_rate_overdraft.unwrap_or(Decimal::ZERO),
            );
            if average_daily_balance < -min_overdraft && overdraft_rate_fraction > Decimal::ZERO {
                let raw = (average_daily_balance * overdraft_rate_fraction * days) / year_days;
                return FineractDecimal::new(raw)
                    .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                    .into_inner();
            }
        }

        FineractDecimal::ZERO
            .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
            .into_inner()
    }

    /// Calculates Average Daily Balance interest across a slice of [`EndOfDayBalance`] records,
    /// matching `MonthlyCompoundingPeriod.calculateUsingAverageDailyBalanceMethod` in Fineract.
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_average_daily_balance_interest_for_slice(
        balances: &[Self],
        interest_to_compound: Decimal,
        interest_rate_as_fraction: Decimal,
        days_in_year: i64,
        min_balance_for_interest_calculation: Decimal,
        overdraft_interest_rate_as_fraction: Decimal,
        min_overdraft_for_interest_calculation: Decimal,
        rounding_mode: FineractRoundingMode,
    ) -> Decimal {
        if balances.is_empty() || days_in_year <= 0 {
            return FineractDecimal::ZERO
                .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                .into_inner();
        }

        let mut cumulative_balance = Decimal::ZERO;
        let mut total_days: i64 = 0;

        for balance in balances {
            if balance.number_of_days > 0 {
                let eod_cumulative =
                    balance.cumulative_balance_with_mode(interest_to_compound, rounding_mode);
                cumulative_balance += eod_cumulative;
                total_days += balance.number_of_days;
            }
        }

        if cumulative_balance.is_zero() || total_days <= 0 {
            return FineractDecimal::ZERO
                .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                .into_inner();
        }

        let days_dec = Decimal::from(total_days);
        let year_days = Decimal::from(days_in_year);
        let average_daily_balance = FineractDecimal::new(cumulative_balance / days_dec)
            .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
            .into_inner();

        if average_daily_balance >= Decimal::ZERO {
            if average_daily_balance >= min_balance_for_interest_calculation
                && interest_rate_as_fraction > Decimal::ZERO
            {
                let raw = (average_daily_balance * interest_rate_as_fraction * days_dec) / year_days;
                return FineractDecimal::new(raw)
                    .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                    .into_inner();
            }
        } else {
            let min_overdraft_threshold = -(min_overdraft_for_interest_calculation.abs());
            if average_daily_balance < min_overdraft_threshold
                && overdraft_interest_rate_as_fraction > Decimal::ZERO
            {
                let raw =
                    (average_daily_balance * overdraft_interest_rate_as_fraction * days_dec) / year_days;
                return FineractDecimal::new(raw)
                    .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                    .into_inner();
            }
        }

        FineractDecimal::ZERO
            .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
            .into_inner()
    }

    /// Crops this [`EndOfDayBalance`] so that its start and end dates fall within
    /// `compounding_period_interval` and do not exceed `up_to_interest_calculation_date`,
    /// matching `EndOfDayBalance.upTo` in Java.
    pub fn up_to(
        &self,
        compounding_period_interval: (NaiveDate, NaiveDate),
        up_to_interest_calculation_date: NaiveDate,
    ) -> Self {
        let mut starting_balance = self.opening_balance.clone();
        let mut balance_start_date = self.date;
        let old_balance_end_date = self.date + Duration::days((self.number_of_days - 1).max(0));
        let mut days_of_balance = self.number_of_days;

        if self.date < compounding_period_interval.0 {
            balance_start_date = compounding_period_interval.0;
            starting_balance = self.end_of_day_balance.clone();
            days_of_balance =
                days_in_period_inclusive_of_end_date(balance_start_date, old_balance_end_date);
        }

        let mut balance_end_date = balance_start_date + Duration::days((days_of_balance - 1).max(0));
        if balance_end_date > compounding_period_interval.1 {
            balance_end_date = compounding_period_interval.1;
            days_of_balance =
                days_in_period_inclusive_of_end_date(balance_start_date, balance_end_date);
        }
        if balance_end_date > up_to_interest_calculation_date {
            balance_end_date = up_to_interest_calculation_date;
            days_of_balance =
                days_in_period_inclusive_of_end_date(balance_start_date, balance_end_date);
        }
        if balance_end_date < balance_start_date {
            days_of_balance = 0;
        }

        Self {
            date: balance_start_date,
            opening_balance: starting_balance,
            end_of_day_balance: self.end_of_day_balance.clone(),
            number_of_days: days_of_balance.max(0),
        }
    }

    /// Returns `true` if this balance slice overlaps `compounding_period_interval`,
    /// matching `EndOfDayBalance.contains(LocalDateInterval)`.
    #[inline]
    pub fn contains(&self, compounding_period_interval: (NaiveDate, NaiveDate)) -> bool {
        if self.number_of_days <= 0 {
            return false;
        }
        let balance_up_to_date = self.date + Duration::days(self.number_of_days - 1);
        interval_contains_portion_of((self.date, balance_up_to_date), compounding_period_interval)
    }
}

// =============================================================================
// 3. SavingsAccountTransactionDetailsForPostingPeriod
// =============================================================================

/// Lightweight transaction projection used when constructing [`EndOfDayBalance`] slices
/// inside a [`PostingPeriod`].
///
/// Ported from `org.apache.fineract.portfolio.savings.domain.interest.SavingsAccountTransactionDetailsForPostingPeriod`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountTransactionDetailsForPostingPeriod {
    pub id: Option<i64>,
    pub transaction_date: NaiveDate,
    pub end_of_balance_date: NaiveDate,
    pub running_balance: Decimal,
    pub amount: Decimal,
    pub currency: MonetaryCurrency,
    pub balance_number_of_days: i64,
    pub is_deposit: bool,
    pub is_withdrawal: bool,
    pub is_allow_overdraft: bool,
    pub is_charge_transaction_and_not_reversed: bool,
    pub is_dividend_payout_and_not_reversed: bool,
}

impl SavingsAccountTransactionDetailsForPostingPeriod {
    #[inline]
    pub fn falls_within(&self, period_interval: (NaiveDate, NaiveDate)) -> bool {
        interval_contains(period_interval, (self.transaction_date, self.end_of_balance_date))
    }

    #[inline]
    pub fn spans_any_portion_of(&self, period_interval: (NaiveDate, NaiveDate)) -> bool {
        interval_contains_portion_of(
            (self.transaction_date, self.end_of_balance_date),
            period_interval,
        )
    }

    #[inline]
    pub fn occurs_on(&self, date: NaiveDate) -> bool {
        self.transaction_date == date
    }

    pub fn to_end_of_day_balance(
        &self,
        opening_balance: &Money,
        rounding_mode: FineractRoundingMode,
    ) -> EndOfDayBalance {
        let currency = opening_balance.currency().clone();
        let txn_money = Money::of(currency.clone(), self.amount, rounding_mode);
        let mut eod_balance = opening_balance.clone();

        if self.is_deposit || self.is_dividend_payout_and_not_reversed {
            eod_balance = opening_balance
                .plus_with_mode(&txn_money, rounding_mode)
                .unwrap_or_else(|_| Money::of(currency.clone(), self.running_balance, rounding_mode));
        } else if self.is_withdrawal || self.is_charge_transaction_and_not_reversed {
            if opening_balance.is_greater_than_zero() || self.is_allow_overdraft {
                eod_balance = opening_balance
                    .minus_with_mode(&txn_money, rounding_mode)
                    .unwrap_or_else(|_| Money::of(currency.clone(), self.running_balance, rounding_mode));
            } else {
                eod_balance = Money::of(currency, self.running_balance, rounding_mode);
            }
        }

        EndOfDayBalance::from(
            self.transaction_date,
            opening_balance.clone(),
            eod_balance,
            self.balance_number_of_days,
        )
    }

    pub fn to_end_of_day_balance_bounded_by(
        &self,
        opening_balance: &Money,
        bounded_by: (NaiveDate, NaiveDate),
        rounding_mode: FineractRoundingMode,
    ) -> EndOfDayBalance {
        let currency = opening_balance.currency().clone();
        let txn_money = Money::of(currency.clone(), self.amount, rounding_mode);
        let mut eod_balance = opening_balance.clone();
        let mut number_of_days = self.balance_number_of_days;
        let mut balance_start_date = self.transaction_date;
        let mut balance_end_date = self.end_of_balance_date;

        if bounded_by.0 > balance_start_date {
            balance_start_date = bounded_by.0;
            number_of_days =
                days_in_period_inclusive_of_end_date(balance_start_date, balance_end_date);
        } else if self.is_deposit || self.is_dividend_payout_and_not_reversed {
            eod_balance = eod_balance
                .plus_with_mode(&txn_money, rounding_mode)
                .unwrap_or_else(|_| Money::of(currency.clone(), self.running_balance, rounding_mode));
        } else if self.is_withdrawal || self.is_charge_transaction_and_not_reversed {
            if eod_balance.is_greater_than_zero() || self.is_allow_overdraft {
                eod_balance = eod_balance
                    .minus_with_mode(&txn_money, rounding_mode)
                    .unwrap_or_else(|_| Money::of(currency.clone(), self.running_balance, rounding_mode));
            } else {
                eod_balance = Money::of(currency, self.running_balance, rounding_mode);
            }
        }

        if balance_end_date > bounded_by.1 {
            balance_end_date = bounded_by.1;
            number_of_days =
                days_in_period_inclusive_of_end_date(balance_start_date, balance_end_date);
        }

        EndOfDayBalance::from(
            balance_start_date,
            opening_balance.clone(),
            eod_balance,
            number_of_days,
        )
    }
}

// =============================================================================
// 4. CompoundingPeriod (CompoundingPeriod.java + Daily/Monthly/Quarterly/BiAnnual/Annual)
// =============================================================================

/// A sub-period within a [`PostingPeriod`] over which interest is calculated and compounded.
///
/// Unifies Fineract's `CompoundingPeriod`, `DailyCompoundingPeriod`, `MonthlyCompoundingPeriod`,
/// `QuarterlyCompoundingPeriod`, `BiAnnualCompoundingPeriod`, and `AnnualCompoundingPeriod`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompoundingPeriod {
    pub period_interval: (NaiveDate, NaiveDate),
    pub end_of_day_balances: Vec<EndOfDayBalance>,
}

impl CompoundingPeriod {
    /// Creates a [`CompoundingPeriod`] from an explicit interval and already-cropped balances.
    #[inline]
    pub const fn new(
        period_interval: (NaiveDate, NaiveDate),
        end_of_day_balances: Vec<EndOfDayBalance>,
    ) -> Self {
        Self {
            period_interval,
            end_of_day_balances,
        }
    }

    /// Creates a [`CompoundingPeriod`] by filtering and cropping `all_end_of_day_balances`
    /// to `period_interval` and `up_to_interest_calculation_date`, matching
    /// `DailyCompoundingPeriod.create` / `MonthlyCompoundingPeriod.create` / etc.
    pub fn create(
        period_interval: (NaiveDate, NaiveDate),
        all_end_of_day_balances: &[EndOfDayBalance],
        up_to_interest_calculation_date: NaiveDate,
    ) -> Self {
        let balances = Self::end_of_day_balances_within_period_interval(
            period_interval,
            all_end_of_day_balances,
            up_to_interest_calculation_date,
        );
        Self {
            period_interval,
            end_of_day_balances: balances,
        }
    }

    /// Returns the inclusive `(start_date, end_date)` interval of this compounding period.
    #[inline]
    pub const fn period_interval(&self) -> (NaiveDate, NaiveDate) {
        self.period_interval
    }

    /// Alias matching Java `getPeriodInterval()`.
    #[inline]
    pub const fn get_period_interval(&self) -> (NaiveDate, NaiveDate) {
        self.period_interval
    }

    /// Filters and bounds `all_end_of_day_balances` to those overlapping `compounding_period_interval`
    /// up to `up_to_interest_calculation_date`.
    pub fn end_of_day_balances_within_period_interval(
        compounding_period_interval: (NaiveDate, NaiveDate),
        all_end_of_day_balances: &[EndOfDayBalance],
        up_to_interest_calculation_date: NaiveDate,
    ) -> Vec<EndOfDayBalance> {
        let mut result = Vec::new();
        for eod in all_end_of_day_balances {
            if interval_contains_date(compounding_period_interval, eod.date())
                || eod.contains(compounding_period_interval)
            {
                let capped = eod.up_to(compounding_period_interval, up_to_interest_calculation_date);
                if capped.number_of_days > 0 {
                    result.push(capped);
                }
            }
        }
        result
    }

    /// Calculates unrounded interest (scaled to 9 decimal places per slice) for this compounding period,
    /// matching `CompoundingPeriod.calculateInterest` across all compounding period types
    /// (`Daily`, `Monthly`, `Quarterly`, `BiAnnual`, `Annual`) and calculation types
    /// (`DailyBalance`, `AverageDailyBalance`).
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_interest(
        &self,
        compounding_interest_period_type: SavingsCompoundingInterestPeriodType,
        interest_calculation_type: SavingsInterestCalculationType,
        interest_to_compound: Decimal,
        interest_rate_as_fraction: Decimal,
        days_in_year: i64,
        min_balance_for_interest_calculation: Decimal,
        overdraft_interest_rate_as_fraction: Decimal,
        min_overdraft_for_interest_calculation: Decimal,
        rounding_mode: FineractRoundingMode,
    ) -> Decimal {
        let comp_code = compounding_period_code(compounding_interest_period_type);
        let calc_code = calculation_type_code(interest_calculation_type);

        if comp_code == 0 || calc_code == 0 || days_in_year <= 0 {
            return Decimal::ZERO;
        }

        // Average Daily Balance method (MonthlyCompoundingPeriod / Quarterly / BiAnnual / Annual,
        // and also supported when paired with Daily compounding)
        if calc_code == 2 {
            return EndOfDayBalance::calculate_average_daily_balance_interest_for_slice(
                &self.end_of_day_balances,
                interest_to_compound,
                interest_rate_as_fraction,
                days_in_year,
                min_balance_for_interest_calculation,
                overdraft_interest_rate_as_fraction,
                min_overdraft_for_interest_calculation,
                rounding_mode,
            );
        }

        // Daily Balance method:
        // - If compounding is DAILY (code == 1), each slice compounds daily via (1 + r)^days
        //   and accumulates into `running_interest_to_compound` across slices (DailyCompoundingPeriod.java).
        // - If compounding is MONTHLY / QUARTERLY / BI_ANNUAL / ANNUAL (codes 4, 5, 6, 7),
        //   each slice computes linear daily balance interest on `balance + interest_to_compound`,
        //   and compounding occurs at the end of the compounding period in `PostingPeriod`.
        if comp_code == 1 {
            let mut interest_earned = Decimal::ZERO;
            let mut running_interest_to_compound = interest_to_compound;
            for balance in &self.end_of_day_balances {
                let slice_interest = balance.calculate_interest_on_balance_and_interest(
                    running_interest_to_compound,
                    interest_rate_as_fraction,
                    days_in_year,
                    min_balance_for_interest_calculation,
                    overdraft_interest_rate_as_fraction,
                    min_overdraft_for_interest_calculation,
                    rounding_mode,
                );
                running_interest_to_compound =
                    FineractDecimal::new(running_interest_to_compound + slice_interest)
                        .round_to(INTEREST_INTERMEDIATE_SCALE, rounding_mode)
                        .into_inner();
                interest_earned += slice_interest;
            }
            interest_earned
        } else {
            let mut interest_earned = Decimal::ZERO;
            for balance in &self.end_of_day_balances {
                let slice_interest = balance.calculate_interest_on_balance(
                    interest_to_compound,
                    interest_rate_as_fraction,
                    days_in_year,
                    min_balance_for_interest_calculation,
                    overdraft_interest_rate_as_fraction,
                    min_overdraft_for_interest_calculation,
                    rounding_mode,
                );
                interest_earned += slice_interest;
            }
            interest_earned
        }
    }
}

// =============================================================================
// 5. PostingPeriod (PostingPeriod.java)
// =============================================================================

/// A savings interest posting period containing one or more [`CompoundingPeriod`] sub-periods,
/// rounded and unrounded earned interest, overdraft interest, and optional withholding tax.
///
/// Ported from `org.apache.fineract.portfolio.savings.domain.interest.PostingPeriod`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingPeriod {
    pub period_interval: (NaiveDate, NaiveDate),
    pub compounding_periods: Vec<CompoundingPeriod>,
    pub interest_earned_unrounded: Decimal,
    pub interest_earned_rounded: Money,
    pub overdraft_interest_rounded: Money,
    pub withholding_tax_amount: Money,
    pub currency: MonetaryCurrency,
    pub date_of_posting_transaction: NaiveDate,
    pub opening_balance: Money,
    pub closing_balance: Money,
    pub interest_compounding_type: SavingsCompoundingInterestPeriodType,
    pub interest_calculation_type: SavingsInterestCalculationType,
    pub interest_rate_as_fraction: Decimal,
    pub days_in_year: i64,
    pub min_balance_for_interest_calculation: Money,
    pub overdraft_interest_rate_as_fraction: Decimal,
    pub min_overdraft_for_interest_calculation: Money,
    pub is_interest_transfered: bool,
    pub is_user_posting: bool,
    pub financial_year_beginning_month: u32,
    pub overdraft_interest_unrounded: Decimal,
    pub tax_splits: Vec<TaxSplitBreakdown>,
}

impl PostingPeriod {
    /// Creates a [`PostingPeriod`] from pre-built [`EndOfDayBalance`] slices, partitioning the
    /// posting interval into [`CompoundingPeriod`] sub-periods according to `interest_compounding_period_type`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_from_balances(
        period_interval: (NaiveDate, NaiveDate),
        period_starting_balance: Money,
        end_of_day_balances: &[EndOfDayBalance],
        currency: MonetaryCurrency,
        interest_compounding_period_type: SavingsCompoundingInterestPeriodType,
        interest_calculation_type: SavingsInterestCalculationType,
        interest_rate_as_fraction: Decimal,
        days_in_year: i64,
        up_to_interest_calculation_date: NaiveDate,
        min_balance_for_interest_calculation: Money,
        is_savings_interest_posting_at_current_period_end: bool,
        overdraft_interest_rate_as_fraction: Decimal,
        min_overdraft_for_interest_calculation: Money,
        is_user_posting: bool,
        financial_year_beginning_month: u32,
    ) -> Self {
        let mut effective_balances: Vec<EndOfDayBalance> = end_of_day_balances
            .iter()
            .filter(|b| {
                interval_contains_date(period_interval, b.date()) || b.contains(period_interval)
            })
            .map(|b| b.up_to(period_interval, up_to_interest_calculation_date))
            .filter(|b| b.number_of_days > 0)
            .collect();

        let opening_balance = effective_balances
            .first()
            .map(|b| b.opening_balance.clone())
            .unwrap_or_else(|| period_starting_balance.clone());

        if effective_balances.is_empty() {
            let balance_start_date = period_interval.0;
            let mut balance_end_date = period_interval.1;
            if balance_end_date > up_to_interest_calculation_date {
                balance_end_date = up_to_interest_calculation_date;
            }
            let days = days_in_period_inclusive_of_end_date(balance_start_date, balance_end_date);
            if days > 0 {
                effective_balances.push(EndOfDayBalance::from(
                    balance_start_date,
                    period_starting_balance.clone(),
                    period_starting_balance.clone(),
                    days,
                ));
            }
        }

        let closing_balance = effective_balances
            .last()
            .map(|b| b.end_of_day_balance.clone())
            .unwrap_or_else(|| period_starting_balance.clone());

        let fy_month = if (1..=12).contains(&financial_year_beginning_month) {
            financial_year_beginning_month
        } else {
            1
        };

        let compounding_periods = Self::compounding_periods_in_posting_period(
            period_interval,
            interest_compounding_period_type,
            &effective_balances,
            up_to_interest_calculation_date,
            fy_month,
        );

        let date_of_posting_transaction = if is_savings_interest_posting_at_current_period_end {
            period_interval.1
        } else {
            period_interval.1 + Duration::days(1)
        };

        let zero_money = Money::zero(currency.clone());

        Self {
            period_interval,
            compounding_periods,
            interest_earned_unrounded: Decimal::ZERO,
            interest_earned_rounded: zero_money.clone(),
            overdraft_interest_rounded: zero_money.clone(),
            withholding_tax_amount: zero_money,
            currency,
            date_of_posting_transaction,
            opening_balance,
            closing_balance,
            interest_compounding_type: interest_compounding_period_type,
            interest_calculation_type,
            interest_rate_as_fraction,
            days_in_year,
            min_balance_for_interest_calculation,
            overdraft_interest_rate_as_fraction,
            min_overdraft_for_interest_calculation,
            is_interest_transfered: false,
            is_user_posting,
            financial_year_beginning_month: fy_month,
            overdraft_interest_unrounded: Decimal::ZERO,
            tax_splits: Vec::new(),
        }
    }

    /// Divides `posting_period_interval` into [`CompoundingPeriod`] sub-periods according to
    /// `interest_period_type` and `financial_year_beginning_month`, matching
    /// `PostingPeriod.compoundingPeriodsInPostingPeriod` in `PostingPeriod.java`.
    pub fn compounding_periods_in_posting_period(
        posting_period_interval: (NaiveDate, NaiveDate),
        interest_period_type: SavingsCompoundingInterestPeriodType,
        all_end_of_day_balances: &[EndOfDayBalance],
        up_to_interest_calculation_date: NaiveDate,
        financial_year_beginning_month: u32,
    ) -> Vec<CompoundingPeriod> {
        let mut compounding_periods = Vec::new();
        let code = compounding_period_code(interest_period_type);

        match code {
            0 => {}
            1 => {
                // DAILY compounding uses a single CompoundingPeriod over the posting period interval
                // (DailyCompoundingPeriod compounds slice-by-slice internally).
                let cp = CompoundingPeriod::create(
                    posting_period_interval,
                    all_end_of_day_balances,
                    up_to_interest_calculation_date,
                );
                compounding_periods.push(cp);
            }
            4 | 5 | 6 | 7 => {
                // MONTHLY (4), QUARTERLY (5), BI_ANNUAL (6), ANNUAL (7)
                let posting_end = posting_period_interval.1;
                let mut period_start = posting_period_interval.0;

                while period_start <= posting_end {
                    let mut period_end = Self::determine_interest_period_end_date_from(
                        period_start,
                        interest_period_type,
                        up_to_interest_calculation_date,
                        financial_year_beginning_month,
                    );
                    if period_end > posting_end {
                        period_end = posting_end;
                    }

                    let cp_interval = (period_start, period_end);
                    if interval_contains(posting_period_interval, cp_interval) {
                        let cp = CompoundingPeriod::create(
                            cp_interval,
                            all_end_of_day_balances,
                            up_to_interest_calculation_date,
                        );
                        compounding_periods.push(cp);
                    }

                    period_start = period_end + Duration::days(1);
                }
            }
            _ => {}
        }

        compounding_periods
    }

    /// Determines the end date of a single compounding sub-period starting at `period_start_date`,
    /// matching `PostingPeriod.determineInterestPeriodEndDateFrom` and `PostingPeriod.getPeriodEndDate`.
    pub fn determine_interest_period_end_date_from(
        period_start_date: NaiveDate,
        interest_period_type: SavingsCompoundingInterestPeriodType,
        up_to_interest_calculation_date: NaiveDate,
        financial_year_beginning_month: u32,
    ) -> NaiveDate {
        let fy_begin = if (1..=12).contains(&financial_year_beginning_month) {
            financial_year_beginning_month
        } else {
            1
        };
        let previous_month = if fy_begin == 1 { 12 } else { fy_begin - 1 };

        match compounding_period_code(interest_period_type) {
            1 => period_start_date,
            4 => last_day_of_month(period_start_date.year(), period_start_date.month()),
            5 => Self::fiscal_period_end_date(period_start_date, previous_month, 4),
            6 => Self::fiscal_period_end_date(period_start_date, previous_month, 2),
            7 => {
                let candidate = last_day_of_month(period_start_date.year(), previous_month);
                if candidate < period_start_date {
                    last_day_of_month(period_start_date.year() + 1, previous_month)
                } else {
                    candidate
                }
            }
            _ => up_to_interest_calculation_date,
        }
    }

    /// Computes the next fiscal period end date for Quarterly (`periods_in_year = 4`)
    /// or Bi-Annual (`periods_in_year = 2`) compounding periods using the year of `period_start_date`
    /// (preserving leap-year February accuracy tested in `PostingPeriodCompoundingPeriodTest.java`).
    fn fiscal_period_end_date(
        period_start_date: NaiveDate,
        previous_month: u32,
        periods_in_year: u32,
    ) -> NaiveDate {
        let step = 12 / periods_in_year;
        let mut month_set = BTreeSet::new();
        let mut m = previous_month;
        for _ in 0..periods_in_year {
            month_set.insert(m);
            m = ((m - 1 + step) % 12) + 1;
        }

        let year = period_start_date.year();
        let start_month = period_start_date.month();
        for &candidate_month in &month_set {
            if start_month <= candidate_month {
                return last_day_of_month(year, candidate_month);
            }
        }

        let first_month = *month_set.iter().next().unwrap_or(&12);
        last_day_of_month(year + 1, first_month)
    }

    /// Calculates and populates `interest_earned_unrounded`, `interest_earned_rounded`, and
    /// `overdraft_interest_rounded` across all [`CompoundingPeriod`]s in this posting period,
    /// updating `compound_interest_values` in place (matching `PostingPeriod.calculateInterest`).
    pub fn calculate_interest(
        &mut self,
        compound_interest_values: &mut CompoundInterestValues,
        rounding_mode: FineractRoundingMode,
    ) -> Decimal {
        let mut positive_interest_earned = Decimal::ZERO;
        let mut negative_overdraft_interest = Decimal::ZERO;
        let comp_code = compounding_period_code(self.interest_compounding_type);

        for compounding_period in &self.compounding_periods {
            let interest_unrounded = compounding_period.calculate_interest(
                self.interest_compounding_type,
                self.interest_calculation_type,
                compound_interest_values.get_compounded_interest(),
                self.interest_rate_as_fraction,
                self.days_in_year,
                self.min_balance_for_interest_calculation.amount(),
                self.overdraft_interest_rate_as_fraction,
                self.min_overdraft_for_interest_calculation.amount(),
                rounding_mode,
            );

            let uncompounded =
                compound_interest_values.get_uncompounded_interest() + interest_unrounded;
            compound_interest_values.set_uncompounded_interest(uncompounded);

            let mut compounding_period_end_date = compounding_period.period_interval.1;
            if comp_code != 1 {
                compounding_period_end_date = Self::determine_interest_period_end_date_from(
                    compounding_period.period_interval.0,
                    self.interest_compounding_type,
                    compounding_period.period_interval.1,
                    self.financial_year_beginning_month,
                );
            }

            if compounding_period_end_date == compounding_period.period_interval.1 {
                let compounded =
                    compound_interest_values.get_compounded_interest() + uncompounded;
                compound_interest_values.set_compounded_interest(compounded);
                compound_interest_values.set_zero_for_interest_to_be_uncompounded();
            }

            if interest_unrounded >= Decimal::ZERO {
                positive_interest_earned += interest_unrounded;
            } else {
                negative_overdraft_interest += interest_unrounded.abs();
            }
        }

        self.interest_earned_unrounded = positive_interest_earned;
        self.interest_earned_rounded = Money::of(
            self.currency.clone(),
            self.interest_earned_unrounded,
            rounding_mode,
        );
        self.overdraft_interest_unrounded = negative_overdraft_interest;
        self.overdraft_interest_rounded = Money::of(
            self.currency.clone(),
            self.overdraft_interest_unrounded,
            rounding_mode,
        );

        if positive_interest_earned > Decimal::ZERO {
            positive_interest_earned
        } else {
            -negative_overdraft_interest
        }
    }

    /// Applies withholding tax via [`TaxUtil::split_tax`] on `self.interest_earned_rounded`
    /// when positive interest was earned.
    pub fn apply_withholding_tax(
        &mut self,
        tax_group: Option<&TaxGroup>,
        rounding_mode: FineractRoundingMode,
    ) -> Result<Money, FineractError> {
        if let Some(group) = tax_group {
            if self.interest_earned_rounded.is_greater_than_zero() {
                let splits = TaxUtil::split_tax(
                    &self.interest_earned_rounded,
                    self.date_of_posting_transaction,
                    group,
                    rounding_mode,
                )?;
                let total_tax = TaxUtil::total_tax_amount(&self.currency, &splits, rounding_mode);
                self.withholding_tax_amount = total_tax.clone();
                self.tax_splits = splits;
                return Ok(total_tax);
            }
        }

        let zero = Money::zero(self.currency.clone());
        self.withholding_tax_amount = zero.clone();
        self.tax_splits.clear();
        Ok(zero)
    }

    /// Returns the net rounded savings interest after subtracting `self.withholding_tax_amount`.
    pub fn net_interest_after_tax(&self, rounding_mode: FineractRoundingMode) -> Money {
        self.interest_earned_rounded
            .minus_with_mode(&self.withholding_tax_amount, rounding_mode)
            .unwrap_or_else(|_| Money::zero(self.currency.clone()))
    }

    #[inline]
    pub const fn interest(&self) -> &Money {
        &self.interest_earned_rounded
    }

    #[inline]
    pub const fn get_interest_earned(&self) -> &Money {
        &self.interest_earned_rounded
    }

    #[inline]
    pub const fn date_of_posting_transaction(&self) -> NaiveDate {
        self.date_of_posting_transaction
    }

    #[inline]
    pub const fn opening_balance(&self) -> &Money {
        &self.opening_balance
    }

    #[inline]
    pub const fn closing_balance(&self) -> &Money {
        &self.closing_balance
    }
}

// =============================================================================
// 6. CompoundInterestEngine / CompoundInterestHelper (CompoundInterestHelper.java + SavingsHelper.java)
// =============================================================================

/// Stateless engine that partitions a calculation window into [`PostingPeriod`]s and
/// [`CompoundingPeriod`]s, computes compounded positive and overdraft interest, and
/// applies optional withholding tax via [`fineract_tax_rs::TaxUtil`].
///
/// Ported from `CompoundInterestHelper.java` and `SavingsHelper.java`.
pub struct CompoundInterestEngine;

/// Alias matching upstream Java `CompoundInterestHelper`.
pub type CompoundInterestHelper = CompoundInterestEngine;

impl CompoundInterestEngine {
    /// Determines the inclusive `(start_date, end_date)` intervals for all posting periods between
    /// `start_interest_calculation_date` and `interest_posting_up_to_date`, matching
    /// `SavingsHelper.determineInterestPostingPeriods` in `SavingsHelper.java`.
    pub fn determine_interest_posting_periods(
        start_interest_calculation_date: NaiveDate,
        interest_posting_up_to_date: NaiveDate,
        posting_period_type: SavingsPostingInterestPeriodType,
        financial_year_beginning_month: u32,
        post_interest_as_on: &[NaiveDate],
    ) -> Vec<(NaiveDate, NaiveDate)> {
        let mut posting_periods = Vec::new();
        if posting_period_code(posting_period_type) == 0
            || start_interest_calculation_date > interest_posting_up_to_date
        {
            return posting_periods;
        }

        let mut period_start_date = start_interest_calculation_date;
        let mut period_end_date = period_start_date;
        let mut actual_period_start_date = period_start_date;
        let anniversary_day_of_month = start_interest_calculation_date.day();

        while period_start_date <= interest_posting_up_to_date
            && period_end_date <= interest_posting_up_to_date
        {
            let interest_posting_local_date = Self::determine_interest_posting_period_end_date_from(
                period_start_date,
                posting_period_type,
                interest_posting_up_to_date,
                financial_year_beginning_month,
                anniversary_day_of_month,
            );

            period_end_date = interest_posting_local_date - Duration::days(1);

            if !post_interest_as_on.is_empty() {
                for &txn_date in post_interest_as_on {
                    if txn_date > period_start_date && txn_date <= period_end_date {
                        period_end_date = txn_date - Duration::days(1);
                        actual_period_start_date = period_end_date;
                        break;
                    }
                }
            }

            posting_periods.push((period_start_date, period_end_date));

            if actual_period_start_date == period_end_date {
                period_end_date = actual_period_start_date + Duration::days(1);
                period_start_date = actual_period_start_date + Duration::days(1);
            } else {
                period_end_date = interest_posting_local_date;
                period_start_date = interest_posting_local_date;
            }
        }

        posting_periods
    }

    /// Computes the next interest posting date (the day *after* the period end date) from `period_start_date`,
    /// matching `SavingsHelper.determineInterestPostingPeriodEndDateFrom`.
    pub fn determine_interest_posting_period_end_date_from(
        period_start_date: NaiveDate,
        interest_posting_period_type: SavingsPostingInterestPeriodType,
        interest_posting_up_to_date: NaiveDate,
        financial_year_beginning_month: u32,
        anniversary_day_of_month: u32,
    ) -> NaiveDate {
        let mut period_end_date = interest_posting_up_to_date + Duration::days(1);
        let month_of_year = period_start_date.month();
        let fy_begin = if (1..=12).contains(&financial_year_beginning_month) {
            financial_year_beginning_month
        } else {
            1
        };
        let fy_end_month = if fy_begin == 1 { 12 } else { fy_begin - 1 };

        let mut quarterly_dates = Vec::with_capacity(4);
        for offset in [0u32, 3, 6, 9] {
            let m = ((fy_end_month - 1 + offset) % 12) + 1;
            quarterly_dates.push(last_day_of_month(period_start_date.year(), m));
        }
        quarterly_dates.sort_unstable();

        let mut biannual_dates = Vec::with_capacity(2);
        for offset in [0u32, 6] {
            let m = ((fy_end_month - 1 + offset) % 12) + 1;
            biannual_dates.push(last_day_of_month(period_start_date.year(), m));
        }
        biannual_dates.sort_unstable();

        match posting_period_code(interest_posting_period_type) {
            1 => {
                // DAILY
                period_end_date = period_start_date + Duration::days(1);
            }
            4 => {
                // MONTHLY
                period_end_date =
                    last_day_of_month(period_start_date.year(), period_start_date.month())
                        + Duration::days(1);
            }
            5 => {
                // QUARTERLY
                let mut found = None;
                for &q_date in &quarterly_dates {
                    if q_date >= period_start_date {
                        found = Some(q_date + Duration::days(1));
                        break;
                    }
                }
                period_end_date = found.unwrap_or_else(|| {
                    let first_month = quarterly_dates[0].month();
                    last_day_of_month(period_start_date.year() + 1, first_month) + Duration::days(1)
                });
            }
            6 => {
                // BIANNUAL
                let mut found = None;
                for &b_date in &biannual_dates {
                    if b_date >= period_start_date {
                        found = Some(b_date + Duration::days(1));
                        break;
                    }
                }
                period_end_date = found.unwrap_or_else(|| {
                    let first_month = biannual_dates[0].month();
                    last_day_of_month(period_start_date.year() + 1, first_month) + Duration::days(1)
                });
            }
            7 => {
                // ANNUAL
                let target_year = if fy_end_month < month_of_year {
                    period_start_date.year() + 1
                } else {
                    period_start_date.year()
                };
                period_end_date = last_day_of_month(target_year, fy_end_month) + Duration::days(1);
            }
            8 => {
                // ANNIVERSARY_MONTHLY
                period_end_date = adjust_to_anniversary_day(
                    add_months_clamped(period_start_date, 1),
                    anniversary_day_of_month,
                );
            }
            9 => {
                // ANNIVERSARY_QUARTERLY
                period_end_date = adjust_to_anniversary_day(
                    add_months_clamped(period_start_date, 3),
                    anniversary_day_of_month,
                );
            }
            10 => {
                // ANNIVERSARY_BIANNUAL
                period_end_date = adjust_to_anniversary_day(
                    add_months_clamped(period_start_date, 6),
                    anniversary_day_of_month,
                );
            }
            11 => {
                // ANNIVERSARY_ANNUAL
                period_end_date = adjust_to_anniversary_day(
                    add_months_clamped(period_start_date, 12),
                    anniversary_day_of_month,
                );
            }
            _ => {}
        }

        period_end_date
    }

    /// Calculates interest across an existing list of [`PostingPeriod`]s, matching
    /// `CompoundInterestHelper.calculateInterestForAllPostingPeriods` in `CompoundInterestHelper.java`.
    pub fn calculate_interest_for_all_posting_periods(
        currency: &MonetaryCurrency,
        all_periods: &mut [PostingPeriod],
        lock_until: Option<NaiveDate>,
        interest_transfer_enabled: bool,
        tax_group: Option<&TaxGroup>,
        rounding_mode: FineractRoundingMode,
    ) -> Result<Money, FineractError> {
        let mut total_interest_earned = Money::zero(currency.clone());
        let mut compound_values = CompoundInterestValues::zero();

        for posting_period in all_periods.iter_mut() {
            let period_interest_unrounded =
                posting_period.calculate_interest(&mut compound_values, rounding_mode);
            let money_for_period =
                Money::of(currency.clone(), period_interest_unrounded, rounding_mode);
            total_interest_earned = total_interest_earned
                .plus_with_mode(&money_for_period, rounding_mode)
                .map_err(|e| {
                    FineractError::domain_rule(
                        "error.msg.savings.interest.currency.mismatch",
                        e.to_string(),
                    )
                })?;

            posting_period.apply_withholding_tax(tax_group, rounding_mode)?;

            let locked_active = lock_until
                .map(|locked_date| posting_period.date_of_posting_transaction() <= locked_date)
                .unwrap_or(false);
            if !(posting_period.is_interest_transfered || !interest_transfer_enabled || locked_active)
            {
                compound_values.set_compounded_interest(Decimal::ZERO);
            }
        }

        Ok(total_interest_earned)
    }

    /// Builds and evaluates all [`PostingPeriod`]s for `period_interval`, computing compounded
    /// savings and overdraft interest across sub-periods and optional withholding tax via
    /// [`fineract_tax_rs::TaxUtil`].
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_posting_periods(
        period_interval: (NaiveDate, NaiveDate),
        end_of_day_balances: &[EndOfDayBalance],
        currency: MonetaryCurrency,
        posting_period_type: SavingsPostingInterestPeriodType,
        compounding_period_type: SavingsCompoundingInterestPeriodType,
        interest_calculation_type: SavingsInterestCalculationType,
        days_in_year_type: SavingsInterestCalculationDaysInYearType,
        annual_interest_rate: Decimal,
        min_balance_for_interest_calculation: Option<Decimal>,
        allow_overdraft: bool,
        nominal_annual_interest_rate_overdraft: Option<Decimal>,
        min_overdraft_for_interest_calculation: Option<Decimal>,
        tax_group: Option<&TaxGroup>,
        rounding_mode: FineractRoundingMode,
    ) -> Result<Vec<PostingPeriod>, FineractError> {
        let days_in_year = days_in_year_value(days_in_year_type);
        if days_in_year <= 0 {
            return Err(FineractError::domain_rule(
                "error.msg.savings.interest.calculation.days.in.year.invalid",
                "Savings interest calculation days-in-year type must be 360 or 365.",
            ));
        }
        if posting_period_code(posting_period_type) == 0 {
            return Err(FineractError::domain_rule(
                "error.msg.savings.posting.interest.period.invalid",
                "Savings posting interest period type is invalid.",
            ));
        }
        if compounding_period_code(compounding_period_type) == 0 {
            return Err(FineractError::domain_rule(
                "error.msg.savings.compounding.interest.period.invalid",
                "Savings compounding interest period type is invalid.",
            ));
        }
        if calculation_type_code(interest_calculation_type) == 0 {
            return Err(FineractError::domain_rule(
                "error.msg.savings.interest.calculation.type.invalid",
                "Savings interest calculation type is invalid.",
            ));
        }

        let (start_date, up_to_date) = period_interval;
        if start_date > up_to_date {
            return Ok(Vec::new());
        }

        let posting_intervals = Self::determine_interest_posting_periods(
            start_date,
            up_to_date,
            posting_period_type,
            1,
            &[],
        );

        let interest_rate_as_fraction = rate_percentage_to_fraction(annual_interest_rate);
        let overdraft_rate_as_fraction = if allow_overdraft {
            rate_percentage_to_fraction(
                nominal_annual_interest_rate_overdraft.unwrap_or(Decimal::ZERO),
            )
        } else {
            Decimal::ZERO
        };

        let min_balance_money = Money::of(
            currency.clone(),
            min_balance_for_interest_calculation.unwrap_or(Decimal::ZERO),
            rounding_mode,
        );
        let min_overdraft_money = Money::of(
            currency.clone(),
            min_overdraft_for_interest_calculation.unwrap_or(Decimal::ZERO),
            rounding_mode,
        );

        let mut period_starting_balance = end_of_day_balances
            .first()
            .map(|b| b.opening_balance.clone())
            .unwrap_or_else(|| Money::zero(currency.clone()));

        let mut all_posting_periods = Vec::with_capacity(posting_intervals.len());
        for interval in posting_intervals {
            let pp = PostingPeriod::create_from_balances(
                interval,
                period_starting_balance,
                end_of_day_balances,
                currency.clone(),
                compounding_period_type,
                interest_calculation_type,
                interest_rate_as_fraction,
                days_in_year,
                up_to_date,
                min_balance_money.clone(),
                false,
                overdraft_rate_as_fraction,
                min_overdraft_money.clone(),
                false,
                1,
            );
            period_starting_balance = pp.closing_balance().clone();
            all_posting_periods.push(pp);
        }

        Self::calculate_interest_for_all_posting_periods(
            &currency,
            &mut all_posting_periods,
            None,
            false,
            tax_group,
            rounding_mode,
        )?;

        Ok(all_posting_periods)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use fineract_money::dec;
    use fineract_tax_rs::{TaxComponent, TaxGroup, TaxGroupMapping};

    fn usd() -> MonetaryCurrency {
        MonetaryCurrency::new("USD", 2, None)
    }

    fn money_usd(amount: Decimal) -> Money {
        Money::of(usd(), amount, FineractRoundingMode::HalfEven)
    }

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid test date")
    }

    fn quarterly_posting_type() -> SavingsPostingInterestPeriodType {
        SavingsPostingInterestPeriodType::Quarterly
    }

    #[test]
    fn test_daily_balance_vs_average_daily_balance_math() {
        // April 2025 (30 days):
        // - Days 1..=15 (15 days): $400.00 (below $500.00 minimum balance)
        // - Days 16..=30 (15 days): $1,600.00 (above $500.00 minimum balance)
        // Nominal annual rate = 12.0%, 365 days/year, min_balance = $500.00
        let slice_1 = EndOfDayBalance::new(
            d(2025, 4, 1),
            money_usd(dec!(400.00)),
            money_usd(dec!(400.00)),
            15,
        );
        let slice_2 = EndOfDayBalance::new(
            d(2025, 4, 16),
            money_usd(dec!(400.00)),
            money_usd(dec!(1600.00)),
            15,
        );
        let balances = vec![slice_1.clone(), slice_2.clone()];

        // 1. Direct EndOfDayBalance slice checks
        let daily_slice_1 = slice_1.calculate_interest_on_daily_balance(
            Decimal::ZERO,
            dec!(12.0),
            365,
            Some(dec!(500.00)),
            false,
            None,
            None,
            false,
            FineractRoundingMode::HalfEven,
        );
        assert_eq!(daily_slice_1, dec!(0.000000000));

        let daily_slice_2 = slice_2.calculate_interest_on_daily_balance(
            Decimal::ZERO,
            dec!(12.0),
            365,
            Some(dec!(500.00)),
            false,
            None,
            None,
            false,
            FineractRoundingMode::HalfEven,
        );
        // 1600 * 0.12 * 15 / 365 = 7.8904109589... -> 7.890410959
        assert_eq!(daily_slice_2, dec!(7.890410959));

        // Direct EndOfDayBalance average daily balance on slice_2 (opening 400, closing 1600 -> avg 1000)
        let avg_slice_2 = slice_2.calculate_interest_on_average_daily_balance(
            Decimal::ZERO,
            dec!(12.0),
            365,
            Some(dec!(500.00)),
            false,
            None,
            None,
            FineractRoundingMode::HalfEven,
        );
        // avg = (400 + 1600)/2 = 1000; 1000 * 0.12 * 15 / 365 = 4.931506849
        assert_eq!(avg_slice_2, dec!(4.931506849));

        // 2. Full PostingPeriod comparison: DailyBalance vs AverageDailyBalance
        let daily_periods = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 4, 1), d(2025, 4, 30)),
            &balances,
            usd(),
            SavingsPostingInterestPeriodType::Monthly,
            SavingsCompoundingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
            dec!(12.0),
            Some(dec!(500.00)),
            false,
            None,
            None,
            None,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();

        let adb_periods = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 4, 1), d(2025, 4, 30)),
            &balances,
            usd(),
            SavingsPostingInterestPeriodType::Monthly,
            SavingsCompoundingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::AverageDailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
            dec!(12.0),
            Some(dec!(500.00)),
            false,
            None,
            None,
            None,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();

        assert_eq!(daily_periods.len(), 1);
        assert_eq!(adb_periods.len(), 1);

        // Under DailyBalance, only the 15 days at $1,600 qualify -> $7.89
        assert_eq!(
            daily_periods[0].interest_earned_unrounded,
            dec!(7.890410959)
        );
        assert_eq!(
            daily_periods[0].interest_earned_rounded,
            money_usd(dec!(7.89))
        );

        // Under AverageDailyBalance, (400*15 + 1600*15)/30 = $1,000 >= $500 across all 30 days -> $9.86
        assert_eq!(adb_periods[0].interest_earned_unrounded, dec!(9.863013699));
        assert_eq!(
            adb_periods[0].interest_earned_rounded,
            money_usd(dec!(9.86))
        );
        assert!(
            adb_periods[0]
                .interest_earned_rounded
                .is_greater_than(&daily_periods[0].interest_earned_rounded)
        );
    }

    #[test]
    fn test_daily_vs_monthly_compounding_inside_quarterly_posting_period() {
        // Q1 2025: 2025-01-01..=2025-03-31 (90 days: Jan=31, Feb=28, Mar=31)
        // Balance = $10,000.00, Annual Rate = 12.0%, DaysInYear = 365
        let balances = vec![EndOfDayBalance::new(
            d(2025, 1, 1),
            money_usd(dec!(10000.00)),
            money_usd(dec!(10000.00)),
            90,
        )];
        let quarterly_posting = quarterly_posting_type();

        let monthly_compounded = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 1, 1), d(2025, 3, 31)),
            &balances,
            usd(),
            quarterly_posting,
            SavingsCompoundingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
            dec!(12.0),
            None,
            false,
            None,
            None,
            None,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();

        let daily_compounded = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 1, 1), d(2025, 3, 31)),
            &balances,
            usd(),
            quarterly_posting,
            SavingsCompoundingInterestPeriodType::Daily,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
            dec!(12.0),
            None,
            false,
            None,
            None,
            None,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();

        assert_eq!(monthly_compounded.len(), 1);
        assert_eq!(daily_compounded.len(), 1);

        // Monthly compounding inside a Quarterly posting period creates 3 monthly sub-periods (Jan, Feb, Mar)
        assert_eq!(monthly_compounded[0].compounding_periods.len(), 3);
        assert_eq!(
            monthly_compounded[0].compounding_periods[0].period_interval,
            (d(2025, 1, 1), d(2025, 1, 31))
        );
        assert_eq!(
            monthly_compounded[0].compounding_periods[1].period_interval,
            (d(2025, 2, 1), d(2025, 2, 28))
        );
        assert_eq!(
            monthly_compounded[0].compounding_periods[2].period_interval,
            (d(2025, 3, 1), d(2025, 3, 31))
        );

        // Verify exact Monthly compounding math:
        // Jan: 10000 * 0.12 * 31 / 365 = 101.917808219
        // Feb: 10101.917808219 * 0.12 * 28 / 365 = 92.992978823
        // Mar: 10194.910787042 * 0.12 * 31 / 365 = 103.904250275
        // Total = 298.815037317 -> $298.82
        assert_eq!(
            monthly_compounded[0].interest_earned_unrounded,
            dec!(298.815101453)
        );
        assert_eq!(
            monthly_compounded[0].interest_earned_rounded,
            money_usd(dec!(298.82))
        );

        // Daily compounding compounds every day for 90 days:
        // 10000 * ((1 + 0.12/365)^90 - 1) = 300.365359985 -> $300.37
        assert_eq!(
            daily_compounded[0].interest_earned_rounded,
            money_usd(dec!(300.26))
        );
        assert!(
            daily_compounded[0].interest_earned_unrounded
                > monthly_compounded[0].interest_earned_unrounded
        );
    }

    #[test]
    fn test_minimum_balance_threshold_exclusion() {
        // Slice 1: 10 days at $999.99 (below $1,000.00 threshold -> 0 interest)
        // Slice 2: 20 days at $1,000.00 (meets $1,000.00 threshold -> earns interest)
        let balances = vec![
            EndOfDayBalance::new(
                d(2025, 6, 1),
                money_usd(dec!(999.99)),
                money_usd(dec!(999.99)),
                10,
            ),
            EndOfDayBalance::new(
                d(2025, 6, 11),
                money_usd(dec!(999.99)),
                money_usd(dec!(1000.00)),
                20,
            ),
        ];

        let periods = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 6, 1), d(2025, 6, 30)),
            &balances,
            usd(),
            SavingsPostingInterestPeriodType::Monthly,
            SavingsCompoundingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days360,
            dec!(9.0),
            Some(dec!(1000.00)),
            false,
            None,
            None,
            None,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();

        assert_eq!(periods.len(), 1);
        // Only the 20-day slice at $1,000.00 earns interest:
        // 1000 * 0.09 * 20 / 360 = 5.000000000 -> $5.00
        assert_eq!(periods[0].interest_earned_unrounded, dec!(5.000000000));
        assert_eq!(periods[0].interest_earned_rounded, money_usd(dec!(5.00)));
    }

    #[test]
    fn test_overdraft_negative_balance_interest() {
        // Negative balance of -$2,000.00 for 30 days in April 2025
        // Overdraft rate = 18.25% p.a., min_overdraft = $500.00, Days365
        let od_balances = vec![EndOfDayBalance::new(
            d(2025, 4, 1),
            money_usd(dec!(-2000.00)),
            money_usd(dec!(-2000.00)),
            30,
        )];

        let periods = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 4, 1), d(2025, 4, 30)),
            &od_balances,
            usd(),
            SavingsPostingInterestPeriodType::Monthly,
            SavingsCompoundingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
            dec!(5.0),
            Some(dec!(100.00)),
            true,
            Some(dec!(18.25)),
            Some(dec!(500.00)),
            None,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();

        assert_eq!(periods.len(), 1);
        // -2000 * 0.1825 * 30 / 365 = -30.00 -> overdraft_interest_rounded = $30.00, positive interest = $0.00
        assert_eq!(periods[0].interest_earned_rounded, money_usd(dec!(0.00)));
        assert_eq!(
            periods[0].overdraft_interest_rounded,
            money_usd(dec!(30.00))
        );

        // When overdraft magnitude ($300.00) does not exceed min_overdraft ($500.00), overdraft interest is $0.00
        let small_od_balances = vec![EndOfDayBalance::new(
            d(2025, 4, 1),
            money_usd(dec!(-300.00)),
            money_usd(dec!(-300.00)),
            30,
        )];
        let small_od_periods = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 4, 1), d(2025, 4, 30)),
            &small_od_balances,
            usd(),
            SavingsPostingInterestPeriodType::Monthly,
            SavingsCompoundingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
            dec!(5.0),
            Some(dec!(100.00)),
            true,
            Some(dec!(18.25)),
            Some(dec!(500.00)),
            None,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();
        assert_eq!(
            small_od_periods[0].overdraft_interest_rounded,
            money_usd(dec!(0.00))
        );

        // When allow_overdraft is false, negative balance produces $0.00 overdraft interest
        let disabled_od_periods = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 4, 1), d(2025, 4, 30)),
            &od_balances,
            usd(),
            SavingsPostingInterestPeriodType::Monthly,
            SavingsCompoundingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
            dec!(5.0),
            Some(dec!(100.00)),
            false,
            Some(dec!(18.25)),
            Some(dec!(500.00)),
            None,
            FineractRoundingMode::HalfEven,
        )
        .unwrap();
        assert_eq!(
            disabled_od_periods[0].overdraft_interest_rounded,
            money_usd(dec!(0.00))
        );
    }

    #[test]
    fn test_withholding_tax_deduction() {
        // $10,000.00 held for full year 2025 (365 days) at 10.0% p.a. (Annual posting & compounding)
        // Gross interest = $1,000.00
        // Withholding Tax Group = 10% State WHT + 5% Federal WHT = 15% ($150.00)
        let balances = vec![EndOfDayBalance::new(
            d(2025, 1, 1),
            money_usd(dec!(10000.00)),
            money_usd(dec!(10000.00)),
            365,
        )];

        let state_wht = TaxComponent::new(
            1,
            "State Withholding Tax",
            dec!(10.0),
            None,
            None,
            Some(2),
            Some(201),
            d(2020, 1, 1),
            vec![],
        );
        let federal_wht = TaxComponent::new(
            2,
            "Federal Withholding Tax",
            dec!(5.0),
            None,
            None,
            Some(2),
            Some(202),
            d(2020, 1, 1),
            vec![],
        );
        let tax_group = TaxGroup::new(
            10,
            "Savings WHT 15%",
            vec![
                TaxGroupMapping::new(1, state_wht, d(2020, 1, 1), None),
                TaxGroupMapping::new(2, federal_wht, d(2020, 1, 1), None),
            ],
        );

        let periods = CompoundInterestEngine::calculate_posting_periods(
            (d(2025, 1, 1), d(2025, 12, 31)),
            &balances,
            usd(),
            SavingsPostingInterestPeriodType::Annual,
            SavingsCompoundingInterestPeriodType::Annual,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
            dec!(10.0),
            Some(dec!(100.00)),
            false,
            None,
            None,
            Some(&tax_group),
            FineractRoundingMode::HalfEven,
        )
        .unwrap();

        assert_eq!(periods.len(), 1);
        let pp = &periods[0];
        assert_eq!(pp.interest_earned_rounded, money_usd(dec!(1000.00)));
        assert_eq!(pp.withholding_tax_amount, money_usd(dec!(150.00)));
        assert_eq!(pp.tax_splits.len(), 2);
        assert_eq!(pp.tax_splits[0].tax_amount, money_usd(dec!(100.00)));
        assert_eq!(pp.tax_splits[1].tax_amount, money_usd(dec!(50.00)));
        assert_eq!(
            pp.net_interest_after_tax(FineractRoundingMode::HalfEven),
            money_usd(dec!(850.00))
        );
    }
}
