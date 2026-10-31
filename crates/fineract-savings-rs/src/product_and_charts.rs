//! `product_and_charts` — Savings Products (`m_savings_product`), Fixed & Recurring Deposit
//! Product Term/Pre-Closure Configurations (`m_deposit_product_term_and_preclosure`,
//! `m_deposit_product_recurring_detail`), Savings & Deposit Enumerations, and Interest Rate Charts
//! (`m_interest_rate_chart`, `m_interest_rate_slab`, `m_interest_incentives`).
//!
//! Ported from:
//! - `org.apache.fineract.portfolio.savings`:
//!   - `DepositAccountType`
//!   - `SavingsCompoundingInterestPeriodType`
//!   - `SavingsPostingInterestPeriodType`
//!   - `SavingsInterestCalculationType`
//!   - `SavingsInterestCalculationDaysInYearType`
//!   - `SavingsPeriodFrequencyType`
//!   - `PreClosurePenalInterestOnType`
//! - `org.apache.fineract.portfolio.savings.domain`:
//!   - `SavingsProduct`
//!   - `FixedDepositProduct`, `DepositProductTermAndPreClosure`, `DepositTermDetail`,
//!     `DepositPreClosureDetail`, `DepositProductAmountDetails`
//!   - `RecurringDepositProduct`, `DepositProductRecurringDetail`, `DepositRecurringDetail`
//!   - `DepositAccountInterestRateChart`, `DepositAccountInterestRateChartSlabs`,
//!     `DepositAccountInterestIncentives`
//! - `org.apache.fineract.portfolio.savings.data`:
//!   - `SavingsProductDataValidator`, `DepositProductDataValidator`
//! - `org.apache.fineract.portfolio.interestratechart`:
//!   - `InterestRateChart`, `InterestRateChartFields`
//!   - `InterestRateChartSlab`, `InterestRateChartSlabFields`, `InterestRateChartSlabComparator`
//!   - `InterestIncentives`, `InterestIncentivesFields`
//!   - `InterestIncentiveAttributeName`, `InterestIncentiveType`, `InterestIncentiveEntityType`,
//!     `ClientAttributeIncentiveCalculation`, `AttributeIncentiveCalculation`

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate};
use fineract_accounting_rs::journal_processor::AccountingRuleType;
use fineract_charge_rs::EnumOptionData;
use fineract_core_rs::json_command::{FineractDateParser, FineractNumberParser, JsonCommand};
use fineract_money::MonetaryCurrency;
use fineract_validation_rs::{
    ApiErrorArg, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// =============================================================================
// 0. HELPER TRAIT FOR FLEXIBLE INTEGER ENUM CONVERSIONS
// =============================================================================

/// Trait allowing `from_int` on Fineract enums to accept `i32`, `i16`, `i64`,
/// `Option<i32>`, `Option<i16>`, or `Option<i64>`.
pub trait IntoEnumInt {
    fn into_enum_int(self) -> Option<i32>;
}

impl IntoEnumInt for i32 {
    #[inline]
    fn into_enum_int(self) -> Option<i32> {
        Some(self)
    }
}

impl IntoEnumInt for i16 {
    #[inline]
    fn into_enum_int(self) -> Option<i32> {
        Some(i32::from(self))
    }
}

impl IntoEnumInt for i64 {
    #[inline]
    fn into_enum_int(self) -> Option<i32> {
        i32::try_from(self).ok()
    }
}

impl IntoEnumInt for Option<i32> {
    #[inline]
    fn into_enum_int(self) -> Option<i32> {
        self
    }
}

impl IntoEnumInt for Option<i16> {
    #[inline]
    fn into_enum_int(self) -> Option<i32> {
        self.map(i32::from)
    }
}

impl IntoEnumInt for Option<i64> {
    #[inline]
    fn into_enum_int(self) -> Option<i32> {
        self.and_then(|v| i32::try_from(v).ok())
    }
}

// =============================================================================
// 1. SAVINGS & DEPOSIT ENUMERATIONS
// =============================================================================

/// Deposit account and product discriminator (`m_savings_product.deposit_type_enum`,
/// `m_savings_account.deposit_type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.savings.DepositAccountType`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum DepositAccountType {
    #[default]
    Invalid = 0,
    SavingsDeposit = 100,
    FixedDeposit = 200,
    RecurringDeposit = 300,
    CurrentAccount = 400,
}

impl DepositAccountType {
    pub const INVALID: Self = Self::Invalid;
    pub const SAVINGS_DEPOSIT: Self = Self::SavingsDeposit;
    pub const FIXED_DEPOSIT: Self = Self::FixedDeposit;
    pub const RECURRING_DEPOSIT: Self = Self::RecurringDeposit;
    pub const CURRENT_ACCOUNT: Self = Self::CurrentAccount;
    pub const CURRENT_DEPOSIT: Self = Self::CurrentAccount;

    pub const VALID_VALUES: &'static [i64] = &[100, 200, 300, 400];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "depositAccountType.invalid",
            Self::SavingsDeposit => "depositAccountType.savingsDeposit",
            Self::FixedDeposit => "depositAccountType.fixedDeposit",
            Self::RecurringDeposit => "depositAccountType.recurringDeposit",
            Self::CurrentAccount => "depositAccountType.currentDeposit",
        }
    }

    #[inline]
    pub const fn resource_name(self) -> &'static str {
        match self {
            Self::SavingsDeposit => "savingsaccount",
            Self::FixedDeposit => "fixeddepositaccount",
            Self::RecurringDeposit => "recurringdepositaccount",
            Self::CurrentAccount => "currentaccount",
            Self::Invalid => "INVALID",
        }
    }

    #[inline]
    pub const fn product_resource_name(self) -> &'static str {
        match self {
            Self::SavingsDeposit => "savingsproduct",
            Self::FixedDeposit => "fixeddepositproduct",
            Self::RecurringDeposit => "recurringdepositproduct",
            Self::CurrentAccount => "savingsproduct",
            Self::Invalid => "INVALID",
        }
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Self {
        match v {
            100 => Self::SavingsDeposit,
            200 => Self::FixedDeposit,
            300 => Self::RecurringDeposit,
            400 => Self::CurrentAccount,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            100 => Self::SavingsDeposit,
            200 => Self::FixedDeposit,
            300 => Self::RecurringDeposit,
            400 => Self::CurrentAccount,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub fn from_int<V: IntoEnumInt>(v: V) -> Self {
        match v.into_enum_int() {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_savings_deposit(self) -> bool {
        matches!(self, Self::SavingsDeposit)
    }

    #[inline]
    pub const fn is_fixed_deposit(self) -> bool {
        matches!(self, Self::FixedDeposit)
    }

    #[inline]
    pub const fn is_recurring_deposit(self) -> bool {
        matches!(self, Self::RecurringDeposit)
    }

    #[inline]
    pub const fn is_current_account(self) -> bool {
        matches!(self, Self::CurrentAccount)
    }

    #[inline]
    pub const fn is_current_deposit(self) -> bool {
        matches!(self, Self::CurrentAccount)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.to_string())
    }
}

impl fmt::Display for DepositAccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<DepositAccountType> for i16 {
    fn from(v: DepositAccountType) -> Self {
        v.as_i16()
    }
}

impl From<DepositAccountType> for i32 {
    fn from(v: DepositAccountType) -> Self {
        v.as_i32()
    }
}

/// Compounding interest period frequency (`interest_compounding_period_enum`).
///
/// Ported from `org.apache.fineract.portfolio.savings.SavingsCompoundingInterestPeriodType`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum SavingsCompoundingInterestPeriodType {
    #[default]
    Invalid = 0,
    Daily = 1,
    Monthly = 4,
    Quarterly = 5,
    BiAnnual = 6,
    Annual = 7,
}

impl SavingsCompoundingInterestPeriodType {
    pub const INVALID: Self = Self::Invalid;
    pub const DAILY: Self = Self::Daily;
    pub const MONTHLY: Self = Self::Monthly;
    pub const QUARTERLY: Self = Self::Quarterly;
    pub const QUATERLY: Self = Self::Quarterly;
    pub const BI_ANNUAL: Self = Self::BiAnnual;
    pub const BIANNUAL: Self = Self::BiAnnual;
    pub const ANNUAL: Self = Self::Annual;

    pub const VALID_VALUES: &'static [i64] = &[1, 4, 5, 6, 7];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "savingsCompoundingInterestPeriodType.invalid",
            Self::Daily => "savingsCompoundingInterestPeriodType.daily",
            Self::Monthly => "savingsCompoundingInterestPeriodType.monthly",
            Self::Quarterly => "savingsCompoundingInterestPeriodType.quarterly",
            Self::BiAnnual => "savingsCompoundingInterestPeriodType.biannual",
            Self::Annual => "savingsCompoundingInterestPeriodType.annual",
        }
    }

    #[inline]
    pub const fn integer_values() -> &'static [i64] {
        Self::VALID_VALUES
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Self {
        match v {
            1 => Self::Daily,
            4 => Self::Monthly,
            5 => Self::Quarterly,
            6 => Self::BiAnnual,
            7 => Self::Annual,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Daily,
            4 => Self::Monthly,
            5 => Self::Quarterly,
            6 => Self::BiAnnual,
            7 => Self::Annual,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub fn from_int<V: IntoEnumInt>(v: V) -> Self {
        match v.into_enum_int() {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_daily(self) -> bool {
        matches!(self, Self::Daily)
    }

    #[inline]
    pub const fn is_monthly(self) -> bool {
        matches!(self, Self::Monthly)
    }

    #[inline]
    pub const fn is_quarterly(self) -> bool {
        matches!(self, Self::Quarterly)
    }

    #[inline]
    pub const fn is_biannual(self) -> bool {
        matches!(self, Self::BiAnnual)
    }

    #[inline]
    pub const fn is_annual(self) -> bool {
        matches!(self, Self::Annual)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for SavingsCompoundingInterestPeriodType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<SavingsCompoundingInterestPeriodType> for i16 {
    fn from(v: SavingsCompoundingInterestPeriodType) -> Self {
        v.as_i16()
    }
}

impl From<SavingsCompoundingInterestPeriodType> for i32 {
    fn from(v: SavingsCompoundingInterestPeriodType) -> Self {
        v.as_i32()
    }
}

/// Interest posting period frequency (`interest_posting_period_enum`).
///
/// Ported from `org.apache.fineract.portfolio.savings.SavingsPostingInterestPeriodType`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum SavingsPostingInterestPeriodType {
    #[default]
    Invalid = 0,
    Monthly = 4,
    Quarterly = 5,
    BiAnnual = 6,
    Annual = 7,
}

impl SavingsPostingInterestPeriodType {
    pub const INVALID: Self = Self::Invalid;
    pub const MONTHLY: Self = Self::Monthly;
    pub const QUARTERLY: Self = Self::Quarterly;
    pub const QUATERLY: Self = Self::Quarterly;
    pub const BI_ANNUAL: Self = Self::BiAnnual;
    pub const BIANNUAL: Self = Self::BiAnnual;
    pub const ANNUAL: Self = Self::Annual;

    pub const VALID_VALUES: &'static [i64] = &[4, 5, 6, 7];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "savingsPostingInterestPeriodType.invalid",
            Self::Monthly => "savingsPostingInterestPeriodType.monthly",
            Self::Quarterly => "savingsPostingInterestPeriodType.quarterly",
            Self::BiAnnual => "savingsPostingInterestPeriodType.biannual",
            Self::Annual => "savingsPostingInterestPeriodType.annual",
        }
    }

    #[inline]
    pub const fn integer_values() -> &'static [i64] {
        Self::VALID_VALUES
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Self {
        match v {
            4 => Self::Monthly,
            5 => Self::Quarterly,
            6 => Self::BiAnnual,
            7 => Self::Annual,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            4 => Self::Monthly,
            5 => Self::Quarterly,
            6 => Self::BiAnnual,
            7 => Self::Annual,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub fn from_int<V: IntoEnumInt>(v: V) -> Self {
        match v.into_enum_int() {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_monthly(self) -> bool {
        matches!(self, Self::Monthly)
    }

    #[inline]
    pub const fn is_quarterly(self) -> bool {
        matches!(self, Self::Quarterly)
    }

    #[inline]
    pub const fn is_biannual(self) -> bool {
        matches!(self, Self::BiAnnual)
    }

    #[inline]
    pub const fn is_annual(self) -> bool {
        matches!(self, Self::Annual)
    }

    /// Number of calendar months in this posting period (`1`, `3`, `6`, `12`).
    #[inline]
    pub const fn months_in_period(self) -> Option<u32> {
        match self {
            Self::Monthly => Some(1),
            Self::Quarterly => Some(3),
            Self::BiAnnual => Some(6),
            Self::Annual => Some(12),
            Self::Invalid => None,
        }
    }

    /// Returns `true` if `compounding` is valid and `compounding <= self`.
    #[inline]
    pub const fn allows_compounding_period(
        self,
        compounding: SavingsCompoundingInterestPeriodType,
    ) -> bool {
        !self.is_invalid() && !compounding.is_invalid() && (compounding as i16) <= (self as i16)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for SavingsPostingInterestPeriodType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<SavingsPostingInterestPeriodType> for i16 {
    fn from(v: SavingsPostingInterestPeriodType) -> Self {
        v.as_i16()
    }
}

impl From<SavingsPostingInterestPeriodType> for i32 {
    fn from(v: SavingsPostingInterestPeriodType) -> Self {
        v.as_i32()
    }
}

impl PartialEq<SavingsPostingInterestPeriodType> for SavingsCompoundingInterestPeriodType {
    #[inline]
    fn eq(&self, other: &SavingsPostingInterestPeriodType) -> bool {
        (*self as i16) == (*other as i16)
    }
}

impl PartialOrd<SavingsPostingInterestPeriodType> for SavingsCompoundingInterestPeriodType {
    #[inline]
    fn partial_cmp(&self, other: &SavingsPostingInterestPeriodType) -> Option<Ordering> {
        (*self as i16).partial_cmp(&(*other as i16))
    }
}

impl PartialEq<SavingsCompoundingInterestPeriodType> for SavingsPostingInterestPeriodType {
    #[inline]
    fn eq(&self, other: &SavingsCompoundingInterestPeriodType) -> bool {
        (*self as i16) == (*other as i16)
    }
}

impl PartialOrd<SavingsCompoundingInterestPeriodType> for SavingsPostingInterestPeriodType {
    #[inline]
    fn partial_cmp(&self, other: &SavingsCompoundingInterestPeriodType) -> Option<Ordering> {
        (*self as i16).partial_cmp(&(*other as i16))
    }
}

/// Savings interest balance calculation method (`interest_calculation_type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.savings.SavingsInterestCalculationType`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum SavingsInterestCalculationType {
    #[default]
    Invalid = 0,
    DailyBalance = 1,
    AverageDailyBalance = 2,
}

impl SavingsInterestCalculationType {
    pub const INVALID: Self = Self::Invalid;
    pub const DAILY_BALANCE: Self = Self::DailyBalance;
    pub const AVERAGE_DAILY_BALANCE: Self = Self::AverageDailyBalance;

    pub const VALID_VALUES: &'static [i64] = &[1, 2];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "savingsInterestCalculationType.invalid",
            Self::DailyBalance => "savingsInterestCalculationType.dailybalance",
            Self::AverageDailyBalance => "savingsInterestCalculationType.averagedailybalance",
        }
    }

    #[inline]
    pub const fn integer_values() -> &'static [i64] {
        Self::VALID_VALUES
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Self {
        match v {
            1 => Self::DailyBalance,
            2 => Self::AverageDailyBalance,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::DailyBalance,
            2 => Self::AverageDailyBalance,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub fn from_int<V: IntoEnumInt>(v: V) -> Self {
        match v.into_enum_int() {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_daily_balance(self) -> bool {
        matches!(self, Self::DailyBalance)
    }

    #[inline]
    pub const fn is_average_daily_balance(self) -> bool {
        matches!(self, Self::AverageDailyBalance)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for SavingsInterestCalculationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<SavingsInterestCalculationType> for i16 {
    fn from(v: SavingsInterestCalculationType) -> Self {
        v.as_i16()
    }
}

impl From<SavingsInterestCalculationType> for i32 {
    fn from(v: SavingsInterestCalculationType) -> Self {
        v.as_i32()
    }
}

/// Days-in-year denominator for savings interest calculation (`interest_calculation_days_in_year_type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.savings.SavingsInterestCalculationDaysInYearType`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum SavingsInterestCalculationDaysInYearType {
    #[default]
    Invalid = 0,
    Days360 = 360,
    Days365 = 365,
}

impl SavingsInterestCalculationDaysInYearType {
    pub const INVALID: Self = Self::Invalid;
    pub const DAYS_360: Self = Self::Days360;
    pub const DAYS_365: Self = Self::Days365;

    pub const VALID_VALUES: &'static [i64] = &[360, 365];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "savingsInterestCalculationDaysInYearType.invalid",
            Self::Days360 => "savingsInterestCalculationDaysInYearType.days360",
            Self::Days365 => "savingsInterestCalculationDaysInYearType.days365",
        }
    }

    #[inline]
    pub const fn integer_values() -> &'static [i64] {
        Self::VALID_VALUES
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Self {
        match v {
            360 => Self::Days360,
            365 => Self::Days365,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            360 => Self::Days360,
            365 => Self::Days365,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub fn from_int<V: IntoEnumInt>(v: V) -> Self {
        match v.into_enum_int() {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_days_360(self) -> bool {
        matches!(self, Self::Days360)
    }

    #[inline]
    pub const fn is_days_365(self) -> bool {
        matches!(self, Self::Days365)
    }

    #[inline]
    pub const fn days_in_year(self) -> Option<u32> {
        match self {
            Self::Days360 => Some(360),
            Self::Days365 => Some(365),
            Self::Invalid => None,
        }
    }

    #[inline]
    pub fn as_decimal(self) -> Option<Decimal> {
        match self {
            Self::Days360 => Some(Decimal::from(360)),
            Self::Days365 => Some(Decimal::from(365)),
            Self::Invalid => None,
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for SavingsInterestCalculationDaysInYearType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<SavingsInterestCalculationDaysInYearType> for i16 {
    fn from(v: SavingsInterestCalculationDaysInYearType) -> Self {
        v.as_i16()
    }
}

impl From<SavingsInterestCalculationDaysInYearType> for i32 {
    fn from(v: SavingsInterestCalculationDaysInYearType) -> Self {
        v.as_i32()
    }
}

/// Supported calendar period frequencies for savings lock-in, deposit terms, and rate chart slabs.
///
/// Ported from `org.apache.fineract.portfolio.savings.SavingsPeriodFrequencyType`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum SavingsPeriodFrequencyType {
    Days = 0,
    Weeks = 1,
    Months = 2,
    Years = 3,
    #[default]
    Invalid = 4,
}

impl SavingsPeriodFrequencyType {
    pub const DAYS: Self = Self::Days;
    pub const WEEKS: Self = Self::Weeks;
    pub const MONTHS: Self = Self::Months;
    pub const YEARS: Self = Self::Years;
    pub const INVALID: Self = Self::Invalid;

    pub const VALID_VALUES: &'static [i64] = &[0, 1, 2, 3];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Days => "savingsPeriodFrequencyType.days",
            Self::Weeks => "savingsPeriodFrequencyType.weeks",
            Self::Months => "savingsPeriodFrequencyType.months",
            Self::Years => "savingsPeriodFrequencyType.years",
            Self::Invalid => "savingsPeriodFrequencyType.invalid",
        }
    }

    #[inline]
    pub const fn integer_values() -> &'static [i64] {
        Self::VALID_VALUES
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Self {
        match v {
            0 => Self::Days,
            1 => Self::Weeks,
            2 => Self::Months,
            3 => Self::Years,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Days,
            1 => Self::Weeks,
            2 => Self::Months,
            3 => Self::Years,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub fn from_int<V: IntoEnumInt>(v: V) -> Self {
        match v.into_enum_int() {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_days(self) -> bool {
        matches!(self, Self::Days)
    }

    #[inline]
    pub const fn is_weeks(self) -> bool {
        matches!(self, Self::Weeks)
    }

    #[inline]
    pub const fn is_months(self) -> bool {
        matches!(self, Self::Months)
    }

    #[inline]
    pub const fn is_years(self) -> bool {
        matches!(self, Self::Years)
    }

    /// Converts a period length into approximate days matching `DepositTermDetail.convertToSafeDays`.
    #[inline]
    pub const fn to_safe_days(self, period: i32) -> i32 {
        match self {
            Self::Days => period,
            Self::Weeks => period.saturating_mul(7),
            Self::Months => period.saturating_mul(30),
            Self::Years => period.saturating_mul(365),
            Self::Invalid => 0,
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for SavingsPeriodFrequencyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<SavingsPeriodFrequencyType> for i16 {
    fn from(v: SavingsPeriodFrequencyType) -> Self {
        v.as_i16()
    }
}

impl From<SavingsPeriodFrequencyType> for i32 {
    fn from(v: SavingsPeriodFrequencyType) -> Self {
        v.as_i32()
    }
}

/// Pre-closure penal interest application rule (`pre_closure_penal_interest_on_enum`).
///
/// Ported from `org.apache.fineract.portfolio.savings.PreClosurePenalInterestOnType`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum PreClosurePenalInterestOnType {
    #[default]
    Invalid = 0,
    WholeTerm = 1,
    TillPrematureWithdrawal = 2,
}

impl PreClosurePenalInterestOnType {
    pub const INVALID: Self = Self::Invalid;
    pub const WHOLE_TERM: Self = Self::WholeTerm;
    pub const TILL_PREMATURE_WITHDRAWAL: Self = Self::TillPrematureWithdrawal;

    pub const VALID_VALUES: &'static [i64] = &[1, 2];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "preClosurePenalInterestOnType.invalid",
            Self::WholeTerm => "preClosurePenalInterestOnType.wholeTerm",
            Self::TillPrematureWithdrawal => {
                "preClosurePenalInterestOnType.tillPrematureWithdrawal"
            }
        }
    }

    #[inline]
    pub const fn integer_values() -> &'static [i64] {
        Self::VALID_VALUES
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Self {
        match v {
            1 => Self::WholeTerm,
            2 => Self::TillPrematureWithdrawal,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::WholeTerm,
            2 => Self::TillPrematureWithdrawal,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub fn from_int<V: IntoEnumInt>(v: V) -> Self {
        match v.into_enum_int() {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_whole_term(self) -> bool {
        matches!(self, Self::WholeTerm)
    }

    #[inline]
    pub const fn is_till_premature_withdrawal(self) -> bool {
        matches!(self, Self::TillPrematureWithdrawal)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.code())
    }
}

impl fmt::Display for PreClosurePenalInterestOnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<PreClosurePenalInterestOnType> for i16 {
    fn from(v: PreClosurePenalInterestOnType) -> Self {
        v.as_i16()
    }
}

impl From<PreClosurePenalInterestOnType> for i32 {
    fn from(v: PreClosurePenalInterestOnType) -> Self {
        v.as_i32()
    }
}

// =============================================================================
// 2. INTEREST RATE CHART & INCENTIVE ENUMERATIONS AND ENTITIES
// =============================================================================

/// Client attribute targeted by an [`InterestRateChartIncentive`] (`m_interest_incentives.attribute_name`).
///
/// Ported from `org.apache.fineract.portfolio.interestratechart.incentive.InterestIncentiveAttributeName`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum IncentiveAttributeName {
    Gender = 2,
    Age = 3,
    ClientType = 4,
    ClientClassification = 5,
}

/// Alias matching upstream Java `InterestIncentiveAttributeName`.
pub type InterestIncentiveAttributeName = IncentiveAttributeName;

impl IncentiveAttributeName {
    pub const GENDER: Self = Self::Gender;
    pub const AGE: Self = Self::Age;
    pub const CLIENT_TYPE: Self = Self::ClientType;
    pub const CLIENT_CLASSIFICATION: Self = Self::ClientClassification;

    pub const VALID_VALUES: &'static [i64] = &[2, 3, 4, 5];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Gender => "InterestIncentiveAttributeName.gender",
            Self::Age => "InterestIncentiveAttributeName.age",
            Self::ClientType => "InterestIncentiveAttributeName.clientType",
            Self::ClientClassification => "InterestIncentiveAttributeName.clientClassification",
        }
    }

    #[inline]
    pub const fn is_code_value_attribute(self) -> bool {
        matches!(
            self,
            Self::Gender | Self::ClientType | Self::ClientClassification
        )
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Option<Self> {
        match v {
            2 => Some(Self::Gender),
            3 => Some(Self::Age),
            4 => Some(Self::ClientType),
            5 => Some(Self::ClientClassification),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Option<Self> {
        match v {
            2 => Some(Self::Gender),
            3 => Some(Self::Age),
            4 => Some(Self::ClientType),
            5 => Some(Self::ClientClassification),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(v: i32) -> Option<Self> {
        Self::from_i32(v)
    }
}

impl fmt::Display for IncentiveAttributeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Comparison condition evaluated by an [`InterestRateChartIncentive`] (`m_interest_incentives.condition_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum IncentiveConditionType {
    Equal = 2,
    NotEqual = 3,
    LessThan = 4,
    GreaterThan = 5,
}

impl IncentiveConditionType {
    pub const EQUAL: Self = Self::Equal;
    pub const NOT_EQUAL: Self = Self::NotEqual;
    pub const LESS_THAN: Self = Self::LessThan;
    pub const LESSTHAN: Self = Self::LessThan;
    pub const GREATER_THAN: Self = Self::GreaterThan;
    pub const GREATERTHAN: Self = Self::GreaterThan;

    pub const VALID_VALUES: &'static [i64] = &[2, 3, 4, 5];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Equal => "ConditionType.equal",
            Self::NotEqual => "ConditionType.notequal",
            Self::LessThan => "ConditionType.lessthan",
            Self::GreaterThan => "ConditionType.greaterthan",
        }
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Option<Self> {
        match v {
            2 => Some(Self::Equal),
            3 => Some(Self::NotEqual),
            4 => Some(Self::LessThan),
            5 => Some(Self::GreaterThan),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Option<Self> {
        match v {
            2 => Some(Self::Equal),
            3 => Some(Self::NotEqual),
            4 => Some(Self::LessThan),
            5 => Some(Self::GreaterThan),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(v: i32) -> Option<Self> {
        Self::from_i32(v)
    }

    /// Evaluates `actual <op> target` matching `AttributeIncentiveCalculation.applyIncentive`.
    #[inline]
    pub const fn evaluate(self, actual: i64, target: i64) -> bool {
        match self {
            Self::Equal => actual == target,
            Self::NotEqual => actual != target,
            Self::LessThan => actual < target,
            Self::GreaterThan => actual > target,
        }
    }
}

impl fmt::Display for IncentiveConditionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Rate modification mode of an [`InterestRateChartIncentive`] (`m_interest_incentives.incentive_type`).
///
/// - [`IncentiveType::Entity`] (`1`, Java `FIXED`): overrides the slab's annual interest rate with `amount`.
/// - [`IncentiveType::Incentive`] (`2`, Java `INCENTIVE`): adds `amount` (positive or negative delta) to the rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i16)]
pub enum IncentiveType {
    Entity = 1,
    Incentive = 2,
}

/// Alias matching upstream Java `InterestIncentiveType`.
pub type InterestIncentiveType = IncentiveType;

impl IncentiveType {
    pub const ENTITY: Self = Self::Entity;
    pub const FIXED: Self = Self::Entity;
    pub const INCENTIVE: Self = Self::Incentive;

    pub const VALID_VALUES: &'static [i64] = &[1, 2];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Entity => "InterestIncentiveType.fixed",
            Self::Incentive => "InterestIncentiveType.incentive",
        }
    }

    #[inline]
    pub const fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Entity),
            2 => Some(Self::Incentive),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Entity),
            2 => Some(Self::Incentive),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(v: i32) -> Option<Self> {
        Self::from_i32(v)
    }
}

impl fmt::Display for IncentiveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Client KYC / demographic attributes evaluated when computing interest rate chart incentives.
///
/// Ported from `IncentiveDTO` and `ClientAttributeIncentiveCalculation.java`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientIncentiveContext {
    pub gender_id: Option<i64>,
    pub age: Option<i32>,
    pub client_type_id: Option<i64>,
    pub client_classification_id: Option<i64>,
}

impl ClientIncentiveContext {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            gender_id: None,
            age: None,
            client_type_id: None,
            client_classification_id: None,
        }
    }

    #[inline]
    pub const fn new(
        gender_id: Option<i64>,
        age: Option<i32>,
        client_type_id: Option<i64>,
        client_classification_id: Option<i64>,
    ) -> Self {
        Self {
            gender_id,
            age,
            client_type_id,
            client_classification_id,
        }
    }

    #[inline]
    pub const fn with_gender_id(mut self, gender_id: i64) -> Self {
        self.gender_id = Some(gender_id);
        self
    }

    #[inline]
    pub const fn with_age(mut self, age: i32) -> Self {
        self.age = Some(age);
        self
    }

    #[inline]
    pub const fn with_client_type_id(mut self, client_type_id: i64) -> Self {
        self.client_type_id = Some(client_type_id);
        self
    }

    #[inline]
    pub const fn with_client_classification_id(mut self, client_classification_id: i64) -> Self {
        self.client_classification_id = Some(client_classification_id);
        self
    }

    /// Computes complete years between `date_of_birth` and `business_date` matching
    /// `ChronoUnit.YEARS.between(dobLocalDate, DateUtils.getBusinessLocalDate())`.
    pub fn age_from_dob(date_of_birth: NaiveDate, business_date: NaiveDate) -> i32 {
        if business_date <= date_of_birth {
            return 0;
        }
        let mut years = business_date.year() - date_of_birth.year();
        if (business_date.month(), business_date.day())
            < (date_of_birth.month(), date_of_birth.day())
        {
            years -= 1;
        }
        years.max(0)
    }

    /// Returns the numeric attribute value corresponding to `attr`.
    #[inline]
    pub fn attribute_value(&self, attr: IncentiveAttributeName) -> Option<i64> {
        match attr {
            IncentiveAttributeName::Gender => self.gender_id,
            IncentiveAttributeName::Age => self.age.map(i64::from),
            IncentiveAttributeName::ClientType => self.client_type_id,
            IncentiveAttributeName::ClientClassification => self.client_classification_id,
        }
    }
}

/// Interest rate incentive rule attached to an [`InterestRateChartSlab`]
/// (`m_interest_incentives` / `m_savings_account_interest_incentives`).
///
/// Ported from `InterestIncentives.java` and `InterestIncentivesFields.java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterestRateChartIncentive {
    pub id: Option<i64>,
    pub slab_id: Option<i64>,
    pub entity_type: i16,
    pub attribute_name: IncentiveAttributeName,
    pub condition_type: IncentiveConditionType,
    pub attribute_value: String,
    pub incentive_type: IncentiveType,
    pub amount: Decimal,
}

/// Alias matching upstream Java `InterestIncentives`.
pub type InterestIncentives = InterestRateChartIncentive;

impl InterestRateChartIncentive {
    pub const CUSTOMER_ENTITY_TYPE: i16 = 2;

    pub fn new(
        attribute_name: IncentiveAttributeName,
        condition_type: IncentiveConditionType,
        attribute_value: impl Into<String>,
        incentive_type: IncentiveType,
        amount: Decimal,
    ) -> Self {
        Self {
            id: None,
            slab_id: None,
            entity_type: Self::CUSTOMER_ENTITY_TYPE,
            attribute_name,
            condition_type,
            attribute_value: attribute_value.into(),
            incentive_type,
            amount,
        }
    }

    pub const fn with_id(mut self, id: i64) -> Self {
        self.id = Some(id);
        self
    }

    pub const fn with_slab_id(mut self, slab_id: i64) -> Self {
        self.slab_id = Some(slab_id);
        self
    }

    /// Returns `true` if `client_attrs` satisfies this incentive's attribute condition.
    pub fn matches(&self, client_attrs: &ClientIncentiveContext) -> bool {
        let Some(actual) = client_attrs.attribute_value(self.attribute_name) else {
            return false;
        };
        let Ok(target) = self.attribute_value.trim().parse::<i64>() else {
            return false;
        };
        self.condition_type.evaluate(actual, target)
    }

    /// Applies this incentive to `current_rate` if `client_attrs` matches.
    pub fn apply(&self, current_rate: Decimal, client_attrs: &ClientIncentiveContext) -> Decimal {
        if !self.matches(client_attrs) {
            return current_rate;
        }
        match self.incentive_type {
            IncentiveType::Entity => self.amount,
            IncentiveType::Incentive => current_rate + self.amount,
        }
    }

    /// Validates incentive configuration matching `InterestIncentivesFields.validateIncentiveData`.
    pub fn validate(&self) -> FineractResult<()> {
        let builder = DataValidatorBuilder::for_resource("interest.rate.incentives");
        let parsed_val = self.attribute_value.trim().parse::<i64>().ok();
        let builder = builder
            .reset()
            .parameter("attributeValue")
            .value(parsed_val)
            .not_null()
            .long_greater_than_zero();

        let builder = if self.attribute_name.is_code_value_attribute() {
            builder
                .reset()
                .parameter("conditionType")
                .value(self.condition_type.value())
                .is_one_of_these_values([
                    i64::from(IncentiveConditionType::Equal.value()),
                    i64::from(IncentiveConditionType::NotEqual.value()),
                ])
        } else {
            builder
        };

        builder.throw_validation_errors().map_err(FineractError::from)
    }
}

/// Single slab within an [`InterestRateChart`] (`m_interest_rate_slab` /
/// `m_savings_account_interest_rate_slab`).
///
/// Ported from `InterestRateChartSlab.java` and `InterestRateChartSlabFields.java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterestRateChartSlab {
    pub id: Option<i64>,
    pub interest_rate_chart_id: Option<i64>,
    pub description: Option<String>,
    pub period_type: SavingsPeriodFrequencyType,
    pub from_period: Option<i32>,
    pub to_period: Option<i32>,
    pub amount_range_from: Option<Decimal>,
    pub amount_range_to: Option<Decimal>,
    pub annual_interest_rate: Decimal,
    pub currency_code: String,
    pub incentives: Vec<InterestRateChartIncentive>,
}

impl InterestRateChartSlab {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        description: Option<String>,
        period_type: SavingsPeriodFrequencyType,
        from_period: Option<i32>,
        to_period: Option<i32>,
        amount_range_from: Option<Decimal>,
        amount_range_to: Option<Decimal>,
        annual_interest_rate: Decimal,
        currency_code: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            interest_rate_chart_id: None,
            description,
            period_type,
            from_period,
            to_period,
            amount_range_from,
            amount_range_to,
            annual_interest_rate,
            currency_code: currency_code.into(),
            incentives: Vec::new(),
        }
    }

    /// Creates a period-based slab (`from_period..=to_period`).
    pub fn for_period_range(
        period_type: SavingsPeriodFrequencyType,
        from_period: i32,
        to_period: Option<i32>,
        annual_interest_rate: Decimal,
        currency_code: impl Into<String>,
    ) -> Self {
        Self::new(
            None,
            period_type,
            Some(from_period),
            to_period,
            None,
            None,
            annual_interest_rate,
            currency_code,
        )
    }

    /// Creates an amount-based slab (`amount_range_from..=amount_range_to`).
    pub fn for_amount_range(
        amount_range_from: Decimal,
        amount_range_to: Option<Decimal>,
        annual_interest_rate: Decimal,
        currency_code: impl Into<String>,
    ) -> Self {
        Self::new(
            None,
            SavingsPeriodFrequencyType::Invalid,
            None,
            None,
            Some(amount_range_from),
            amount_range_to,
            annual_interest_rate,
            currency_code,
        )
    }

    pub const fn with_id(mut self, id: i64) -> Self {
        self.id = Some(id);
        self
    }

    pub const fn with_chart_id(mut self, chart_id: i64) -> Self {
        self.interest_rate_chart_id = Some(chart_id);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub const fn with_amount_range(mut self, from: Option<Decimal>, to: Option<Decimal>) -> Self {
        self.amount_range_from = from;
        self.amount_range_to = to;
        self
    }

    pub const fn with_period_range(
        mut self,
        period_type: SavingsPeriodFrequencyType,
        from_period: Option<i32>,
        to_period: Option<i32>,
    ) -> Self {
        self.period_type = period_type;
        self.from_period = from_period;
        self.to_period = to_period;
        self
    }

    pub fn with_incentive(mut self, incentive: InterestRateChartIncentive) -> Self {
        self.incentives.push(incentive);
        self
    }

    pub fn with_incentives(mut self, incentives: Vec<InterestRateChartIncentive>) -> Self {
        self.incentives = incentives;
        self
    }

    /// Checks whether `deposit_amount` falls within `[amount_range_from, amount_range_to]`
    /// matching `InterestRateChartSlabFields.isAmountBetween`.
    #[inline]
    pub fn is_amount_between(&self, deposit_amount: Decimal) -> bool {
        match (self.amount_range_from, self.amount_range_to) {
            (Some(from), Some(to)) => deposit_amount >= from && deposit_amount <= to,
            (Some(from), None) => deposit_amount >= from,
            (None, Some(to)) => deposit_amount <= to,
            (None, None) => true,
        }
    }

    /// Checks whether `(period, query_period_type)` falls within `[from_period, to_period]`
    /// matching `InterestRateChartSlabFields.isPeriodBetween`.
    #[inline]
    pub fn is_period_between(
        &self,
        period: i32,
        query_period_type: SavingsPeriodFrequencyType,
    ) -> bool {
        let Some(from) = self.from_period else {
            return true;
        };

        // Convert `period` into the slab's frequency if both frequencies are valid and differ.
        let effective_period = if !self.period_type.is_invalid()
            && !query_period_type.is_invalid()
            && self.period_type != query_period_type
        {
            let query_days = query_period_type.to_safe_days(period);
            let unit_days = self.period_type.to_safe_days(1).max(1);
            query_days / unit_days
        } else {
            period
        };

        match self.to_period {
            Some(to) => effective_period >= from && effective_period <= to,
            None => effective_period >= from,
        }
    }

    /// Returns `true` if both `amount` and `(period, period_type)` match this slab.
    #[inline]
    pub fn matches(
        &self,
        amount: Decimal,
        period: i32,
        period_type: SavingsPeriodFrequencyType,
    ) -> bool {
        self.is_period_between(period, period_type) && self.is_amount_between(amount)
    }

    /// Computes the effective annual interest rate for this slab after applying all matching
    /// client incentives (`DepositAccountInterestRateChart.getApplicableInterestRate`).
    pub fn effective_interest_rate(&self, client_attrs: &ClientIncentiveContext) -> Decimal {
        let mut effective_rate = self.annual_interest_rate;
        for incentive in &self.incentives {
            effective_rate = incentive.apply(effective_rate, client_attrs);
        }
        // In upstream Fineract, if incentives reduce effectiveInterestRate to zero, reset to slab default.
        if effective_rate == Decimal::ZERO && self.annual_interest_rate != Decimal::ZERO {
            effective_rate = self.annual_interest_rate;
        }
        effective_rate
    }

    #[inline]
    pub const fn is_from_period_greater_than_to_period(&self) -> bool {
        match (self.from_period, self.to_period) {
            (Some(from), Some(to)) => from > to,
            _ => false,
        }
    }

    #[inline]
    pub fn is_amount_range_from_greater_than_to(&self) -> bool {
        match (self.amount_range_from, self.amount_range_to) {
            (Some(from), Some(to)) => from > to,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_valid_chart(&self, is_primary_grouping_by_amount: bool) -> bool {
        (!is_primary_grouping_by_amount && self.from_period.is_some())
            || (is_primary_grouping_by_amount && self.amount_range_from.is_some())
    }

    #[inline]
    pub fn is_periods_same(&self, that: &Self) -> bool {
        self.from_period == that.from_period && self.to_period == that.to_period
    }

    #[inline]
    pub fn is_amount_same(&self, that: &Self) -> bool {
        self.amount_range_from == that.amount_range_from
            && self.amount_range_to == that.amount_range_to
    }

    fn is_period_overlapping(&self, that: &Self) -> bool {
        if self.to_period == that.to_period || self.from_period == that.from_period {
            return true;
        }
        let (Some(this_from), Some(that_from)) = (self.from_period, that.from_period) else {
            return false;
        };
        let Some(this_to) = self.to_period else {
            return true;
        };
        let Some(that_to) = that.to_period else {
            return that_from <= this_to;
        };
        this_from <= that_to && that_from <= this_to
    }

    fn is_amount_overlapping(&self, that: &Self) -> bool {
        if self.amount_range_from == that.amount_range_from
            || self.amount_range_to == that.amount_range_to
        {
            return true;
        }
        let (Some(this_from), Some(that_from)) = (self.amount_range_from, that.amount_range_from)
        else {
            return false;
        };
        let Some(this_to) = self.amount_range_to else {
            return true;
        };
        let Some(that_to) = that.amount_range_to else {
            return that_from <= this_to;
        };
        this_from <= that_to && that_from <= this_to
    }

    /// Port of `InterestRateChartSlabFields.isRateChartOverlapping`.
    pub fn is_overlapping(&self, that: &Self, is_primary_grouping_by_amount: bool) -> bool {
        let period_overlap = self.is_period_overlapping(that);
        let amount_overlap = self.is_amount_overlapping(that);
        let period_same = self.is_periods_same(that);
        let amount_same = self.is_amount_same(that);
        if is_primary_grouping_by_amount {
            (amount_overlap && !amount_same) || (period_overlap && amount_same)
        } else {
            (period_overlap && !period_same) || (amount_overlap && period_same)
        }
    }

    #[inline]
    pub fn is_not_proper_period_start(slab: &Self) -> bool {
        matches!(slab.from_period, Some(p) if p != 0 && p != 1)
    }

    #[inline]
    pub fn is_not_proper_amount_start(slab: &Self) -> bool {
        matches!(slab.amount_range_from, Some(a) if a != Decimal::ZERO && a != Decimal::ONE)
    }

    #[inline]
    pub fn is_not_proper_chart_start(&self) -> bool {
        Self::is_not_proper_period_start(self) || Self::is_not_proper_amount_start(self)
    }

    fn is_not_proper_next_period(&self, next_from_period: Option<i32>) -> bool {
        let Some(this_to) = self.to_period else {
            return true;
        };
        matches!(next_from_period, Some(next_p) if next_p != this_to + 1)
    }

    fn is_not_proper_next_amount(&self, next_from_amount: Option<Decimal>) -> bool {
        let Some(this_to) = self.amount_range_to else {
            return true;
        };
        matches!(next_from_amount, Some(next_a) if next_a != this_to + Decimal::ONE && next_a != this_to)
    }

    /// Port of `InterestRateChartSlabFields.isRateChartHasGap`.
    pub fn has_gap(&self, that: &Self, is_primary_grouping_by_amount: bool) -> bool {
        let is_period_same = self.is_periods_same(that);
        let is_amount_same = self.is_amount_same(that);
        let has_periods = self.from_period.is_some() || that.from_period.is_some();
        let has_amounts = self.amount_range_from.is_some() || that.amount_range_from.is_some();

        if is_primary_grouping_by_amount {
            if is_amount_same {
                if has_periods {
                    return self.is_not_proper_next_period(that.from_period);
                }
            } else {
                return self.is_not_proper_next_amount(that.amount_range_from)
                    || Self::is_not_proper_period_start(that);
            }
        } else if is_period_same {
            if has_amounts {
                return self.is_not_proper_next_amount(that.amount_range_from);
            }
        } else {
            return self.is_not_proper_next_period(that.from_period)
                || Self::is_not_proper_amount_start(that);
        }
        false
    }

    /// Comparator ordering two slabs matching `InterestRateChartSlabComparator.java`.
    pub fn compare_slabs(
        a: &Self,
        b: &Self,
        is_primary_grouping_by_amount: bool,
    ) -> Ordering {
        if a.is_periods_same(b) && a.is_amount_same(b) {
            return Ordering::Equal;
        }
        let cmp_periods = || match (a.from_period, b.from_period) {
            (Some(p1), Some(p2)) => p1.cmp(&p2),
            _ => Ordering::Equal,
        };
        let cmp_amounts = || match (a.amount_range_from, b.amount_range_from) {
            (Some(a1), Some(a2)) => a1.cmp(&a2),
            _ => Ordering::Equal,
        };
        if is_primary_grouping_by_amount {
            if a.is_amount_same(b) {
                cmp_periods()
            } else {
                cmp_amounts()
            }
        } else if a.is_periods_same(b) {
            cmp_amounts()
        } else {
            cmp_periods()
        }
    }

    /// Validates slab-level constraints matching `InterestRateChartSlabDataValidator`.
    pub fn validate(&self, is_primary_grouping_by_amount: bool) -> FineractResult<()> {
        let builder = DataValidatorBuilder::for_resource("chartslab");
        let builder = builder
            .reset()
            .parameter("currencyCode")
            .value(self.currency_code.as_str())
            .not_blank()
            .not_exceeding_length_of(3);

        let builder = builder
            .reset()
            .parameter("annualInterestRate")
            .value(self.annual_interest_rate)
            .zero_or_positive_amount();

        let builder = if self.amount_range_from.is_none() && self.from_period.is_none() {
            builder.fail_with_code_no_parameter_added_to_error_code(
                "fromperiod.or.amountRangeFrom.required",
            )
        } else {
            builder
        };

        let builder = if !is_primary_grouping_by_amount || self.from_period.is_some() {
            let b = builder
                .reset()
                .parameter("fromPeriod")
                .value(self.from_period)
                .not_null()
                .integer_zero_or_greater();
            b.reset()
                .parameter("periodType")
                .value(self.period_type.value())
                .is_one_of_these_values(SavingsPeriodFrequencyType::VALID_VALUES.iter().copied())
        } else {
            builder
        };

        let builder = if let Some(to_p) = self.to_period {
            builder
                .reset()
                .parameter("toPeriod")
                .value(to_p)
                .integer_greater_than_zero()
        } else {
            builder
        };

        let builder = if self.is_from_period_greater_than_to_period() {
            builder
                .reset()
                .parameter("fromPeriod")
                .value(self.from_period)
                .fail_with_code(
                    "from.period.is.greater.than.to.period",
                    "From period cannot be greater than to period.",
                )
        } else {
            builder
        };

        let builder = if is_primary_grouping_by_amount || self.amount_range_from.is_some() {
            builder
                .reset()
                .parameter("amountRangeFrom")
                .value(self.amount_range_from)
                .not_null()
                .zero_or_positive_amount()
        } else {
            builder
        };

        let builder = if let Some(to_amt) = self.amount_range_to {
            builder
                .reset()
                .parameter("amountRangeTo")
                .value(to_amt)
                .positive_amount()
        } else {
            builder
        };

        let builder = if self.is_amount_range_from_greater_than_to() {
            builder
                .reset()
                .parameter("amountRangeFrom")
                .value(self.amount_range_from)
                .fail_with_code(
                    "amount.range.from.is.greater.than.amount.range.to",
                    "Amount range from cannot be greater than amount range to.",
                )
        } else {
            builder
        };

        builder
            .throw_validation_errors()
            .map_err(FineractError::from)?;

        for incentive in &self.incentives {
            incentive.validate()?;
        }

        Ok(())
    }
}

/// Interest Rate Chart entity (`m_interest_rate_chart` / `m_savings_account_interest_rate_chart`).
///
/// Ported from `InterestRateChart.java`, `InterestRateChartFields.java`, and
/// `DepositAccountInterestRateChart.java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterestRateChart {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub from_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub is_primary_grouping_by_amount: bool,
    pub chart_slabs: Vec<InterestRateChartSlab>,
}

/// Alias matching upstream Java `DepositAccountInterestRateChart`.
pub type DepositAccountInterestRateChart = InterestRateChart;

impl InterestRateChart {
    pub fn new(
        name: impl Into<String>,
        from_date: NaiveDate,
        end_date: Option<NaiveDate>,
        is_primary_grouping_by_amount: bool,
        chart_slabs: Vec<InterestRateChartSlab>,
    ) -> Self {
        Self {
            id: None,
            name: name.into(),
            description: None,
            from_date,
            end_date,
            is_primary_grouping_by_amount,
            chart_slabs,
        }
    }

    pub const fn with_id(mut self, id: i64) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_slab(mut self, slab: InterestRateChartSlab) -> Self {
        self.chart_slabs.push(slab);
        self
    }

    #[inline]
    pub fn slabs(&self) -> &[InterestRateChartSlab] {
        &self.chart_slabs
    }

    /// Finds the applicable annual interest rate for the given deposit `amount`, `period`,
    /// `period_type`, and `client_attrs` (including any matching client incentives).
    ///
    /// Ported from `DepositAccountInterestRateChart.getApplicableInterestRate`.
    pub fn find_applicable_interest_rate(
        &self,
        amount: Decimal,
        period: i32,
        period_type: SavingsPeriodFrequencyType,
        client_attrs: &ClientIncentiveContext,
    ) -> Option<Decimal> {
        let mut matched_rate: Option<Decimal> = None;
        for slab in &self.chart_slabs {
            if slab.matches(amount, period, period_type) {
                matched_rate = Some(slab.effective_interest_rate(client_attrs));
            }
        }
        matched_rate
    }

    /// Checks whether `target` falls within `[from_date, end_date]` (or `>= from_date` if `end_date` is `None`).
    #[inline]
    pub fn is_applicable_for_date(&self, target: NaiveDate) -> bool {
        if target < self.from_date {
            return false;
        }
        match self.end_date {
            Some(end) => target <= end,
            None => true,
        }
    }

    /// Checks whether two charts' date intervals overlap (`InterestRateChartFields.isOverlapping`).
    pub fn is_overlapping(&self, other: &Self, default_end_date: NaiveDate) -> bool {
        let this_end = self.end_date.unwrap_or(default_end_date).max(self.from_date);
        let other_end = other
            .end_date
            .unwrap_or(default_end_date)
            .max(other.from_date);
        self.from_date <= other_end && other.from_date <= this_end
    }

    /// Validates chart fields, individual slabs, and cross-slab ordering/contiguity/non-overlap
    /// matching `InterestRateChart.validateChartSlabs`.
    pub fn validate(&self) -> FineractResult<()> {
        let builder = DataValidatorBuilder::for_resource("interestchart");
        let builder = builder
            .reset()
            .parameter("name")
            .value(self.name.as_str())
            .not_blank()
            .not_exceeding_length_of(100);

        let builder = if let Some(end_date) = self.end_date {
            if self.from_date > end_date {
                builder
                    .reset()
                    .parameter("fromDate")
                    .value(self.from_date)
                    .fail_with_code(
                        "from.date.is.after.to.date",
                        "From date cannot be after end date.",
                    )
            } else {
                builder
            }
        } else {
            builder
        };

        let builder = if self.chart_slabs.is_empty() {
            builder
                .reset()
                .parameter("chartSlabs")
                .fail_with_code("cannot.be.empty", "Chart slabs cannot be empty.")
        } else {
            builder
        };

        builder
            .throw_validation_errors()
            .map_err(FineractError::from)?;

        for slab in &self.chart_slabs {
            slab.validate(self.is_primary_grouping_by_amount)?;
        }

        let mut sorted = self.chart_slabs.clone();
        sorted.sort_by(|a, b| {
            InterestRateChartSlab::compare_slabs(a, b, self.is_primary_grouping_by_amount)
        });

        let mut builder = DataValidatorBuilder::for_resource("interestchart");
        let mut expected_period_type: Option<SavingsPeriodFrequencyType> = None;

        for (i, slab) in sorted.iter().enumerate() {
            if i == 0 {
                if !slab.period_type.is_invalid() {
                    expected_period_type = Some(slab.period_type);
                }
                if slab.is_not_proper_chart_start() {
                    builder = builder.fail_with_code_no_parameter_added_to_error_code(
                        "chart.slabs.range.start.incorrect",
                    );
                }
            } else if !slab.period_type.is_invalid() {
                if let Some(exp_pt) = expected_period_type {
                    if slab.period_type != exp_pt {
                        builder = builder
                            .reset()
                            .parameter("periodType")
                            .value(slab.period_type.value())
                            .fail_with_code(
                                "period.type.is.not.same",
                                "Period type must be the same across all chart slabs.",
                            );
                    }
                } else {
                    expected_period_type = Some(slab.period_type);
                }
            }

            if let Some(next_slab) = sorted.get(i + 1) {
                if slab.is_overlapping(next_slab, self.is_primary_grouping_by_amount) {
                    builder = builder
                        .resource("savings.interestRateChart")
                        .parameter("slabs")
                        .fail_with_code("overlap", "Interest rate chart slabs cannot overlap.");
                } else if slab.has_gap(next_slab, self.is_primary_grouping_by_amount) {
                    builder = builder
                        .resource("savings.interestRateChart")
                        .parameter("slabs")
                        .fail_with_code("gap", "Interest rate chart slabs cannot have a gap.");
                }
            }
        }

        builder
            .throw_validation_errors()
            .map_err(FineractError::from)
    }
}

// =============================================================================
// 3. FIXED & RECURRING DEPOSIT PRODUCT TERM AND PRE-CLOSURE CONFIGURATIONS
// =============================================================================

/// Fixed & Recurring Deposit Product term, pre-closure penalty, and deposit amount configuration
/// (`m_deposit_product_term_and_preclosure`).
///
/// Ported from `DepositProductTermAndPreClosure.java`, `DepositTermDetail.java`,
/// `DepositPreClosureDetail.java`, and `DepositProductAmountDetails.java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixedDepositProductTermAndPreClosure {
    pub id: Option<i64>,
    pub savings_product_id: Option<i64>,
    pub min_deposit_term: Option<i32>,
    pub max_deposit_term: Option<i32>,
    pub min_deposit_term_type: SavingsPeriodFrequencyType,
    pub max_deposit_term_type: SavingsPeriodFrequencyType,
    pub in_multiples_of_deposit_term: Option<i32>,
    pub in_multiples_of_deposit_term_type: SavingsPeriodFrequencyType,
    pub pre_closure_penal_applicable: bool,
    pub pre_closure_penal_interest: Option<Decimal>,
    pub pre_closure_penal_interest_on_type: PreClosurePenalInterestOnType,
    pub min_deposit_amount: Option<Decimal>,
    pub deposit_amount: Option<Decimal>,
    pub max_deposit_amount: Option<Decimal>,
}

/// Alias matching upstream Java `DepositProductTermAndPreClosure`.
pub type DepositProductTermAndPreClosure = FixedDepositProductTermAndPreClosure;

impl Default for FixedDepositProductTermAndPreClosure {
    fn default() -> Self {
        Self {
            id: None,
            savings_product_id: None,
            min_deposit_term: Some(1),
            max_deposit_term: None,
            min_deposit_term_type: SavingsPeriodFrequencyType::Months,
            max_deposit_term_type: SavingsPeriodFrequencyType::Invalid,
            in_multiples_of_deposit_term: None,
            in_multiples_of_deposit_term_type: SavingsPeriodFrequencyType::Invalid,
            pre_closure_penal_applicable: false,
            pre_closure_penal_interest: None,
            pre_closure_penal_interest_on_type: PreClosurePenalInterestOnType::Invalid,
            min_deposit_amount: None,
            deposit_amount: None,
            max_deposit_amount: None,
        }
    }
}

impl FixedDepositProductTermAndPreClosure {
    pub fn new(
        min_deposit_term: i32,
        min_deposit_term_type: SavingsPeriodFrequencyType,
        max_deposit_term: Option<i32>,
        max_deposit_term_type: SavingsPeriodFrequencyType,
    ) -> Self {
        Self {
            min_deposit_term: Some(min_deposit_term),
            min_deposit_term_type,
            max_deposit_term,
            max_deposit_term_type,
            ..Self::default()
        }
    }

    pub const fn with_in_multiples_of(
        mut self,
        multiples: i32,
        frequency_type: SavingsPeriodFrequencyType,
    ) -> Self {
        self.in_multiples_of_deposit_term = Some(multiples);
        self.in_multiples_of_deposit_term_type = frequency_type;
        self
    }

    pub const fn with_pre_closure_penalty(
        mut self,
        applicable: bool,
        penal_interest: Option<Decimal>,
        on_type: PreClosurePenalInterestOnType,
    ) -> Self {
        self.pre_closure_penal_applicable = applicable;
        if applicable {
            self.pre_closure_penal_interest = penal_interest;
            self.pre_closure_penal_interest_on_type = on_type;
        } else {
            self.pre_closure_penal_interest = None;
            self.pre_closure_penal_interest_on_type = PreClosurePenalInterestOnType::Invalid;
        }
        self
    }

    pub const fn with_deposit_amounts(
        mut self,
        min_amount: Option<Decimal>,
        default_amount: Option<Decimal>,
        max_amount: Option<Decimal>,
    ) -> Self {
        self.min_deposit_amount = min_amount;
        self.deposit_amount = default_amount;
        self.max_deposit_amount = max_amount;
        self
    }

    /// Checks whether `min_deposit_term` converted to safe days exceeds `max_deposit_term`
    /// (`DepositTermDetail.isMinDepositTermGreaterThanMaxDepositTerm`).
    pub const fn is_min_deposit_term_greater_than_max(&self) -> bool {
        match (self.min_deposit_term, self.max_deposit_term) {
            (Some(min_t), Some(max_t)) => {
                let min_days = self.min_deposit_term_type.to_safe_days(min_t);
                let max_type = if self.max_deposit_term_type.is_invalid() {
                    self.min_deposit_term_type
                } else {
                    self.max_deposit_term_type
                };
                let max_days = max_type.to_safe_days(max_t);
                min_days > max_days
            }
            _ => false,
        }
    }

    /// Checks whether a deposit term `(period, period_type)` is within `[min_deposit_term, max_deposit_term]`.
    pub const fn is_term_within_limits(
        &self,
        period: i32,
        period_type: SavingsPeriodFrequencyType,
    ) -> bool {
        let days = period_type.to_safe_days(period);
        if let Some(min_t) = self.min_deposit_term {
            let min_days = self.min_deposit_term_type.to_safe_days(min_t);
            if days < min_days {
                return false;
            }
        }
        if let Some(max_t) = self.max_deposit_term {
            let max_type = if self.max_deposit_term_type.is_invalid() {
                self.min_deposit_term_type
            } else {
                self.max_deposit_term_type
            };
            let max_days = max_type.to_safe_days(max_t);
            if days > max_days {
                return false;
            }
        }
        true
    }

    /// Port of `DepositTermDetail.isValidInMultiplesOfPeriod`.
    pub const fn is_valid_in_multiples_of_period(
        &self,
        deposit_period: i32,
        deposit_period_frequency_type: SavingsPeriodFrequencyType,
    ) -> bool {
        let Some(multiples) = self.in_multiples_of_deposit_term else {
            return true;
        };
        let deposit_days = deposit_period_frequency_type.to_safe_days(deposit_period);
        let mult_days = self.in_multiples_of_deposit_term_type.to_safe_days(multiples);
        let min_days = match self.min_deposit_term {
            Some(min_t) => self.min_deposit_term_type.to_safe_days(min_t),
            None => 0,
        };
        if mult_days == 0 {
            return true;
        }
        (deposit_days - min_days) % mult_days == 0
    }

    /// Validates term, pre-closure penalty, and deposit amount rules matching
    /// `DepositProductDataValidator` and `FixedDepositProduct.validateDomainRules`.
    pub fn validate(&self) -> FineractResult<()> {
        let builder = DataValidatorBuilder::for_resource("fixeddepositproduct");

        let builder = builder
            .reset()
            .parameter("minDepositTerm")
            .value(self.min_deposit_term)
            .not_null()
            .integer_greater_than_zero();

        let builder = builder
            .reset()
            .parameter("minDepositTermTypeId")
            .value(self.min_deposit_term_type.value())
            .is_one_of_these_values(SavingsPeriodFrequencyType::VALID_VALUES.iter().copied());

        let builder = if let Some(max_t) = self.max_deposit_term {
            let b = builder
                .reset()
                .parameter("maxDepositTerm")
                .value(max_t)
                .integer_greater_than_zero();
            b.reset()
                .parameter("maxDepositTermTypeId")
                .value(self.max_deposit_term_type.value())
                .is_one_of_these_values(SavingsPeriodFrequencyType::VALID_VALUES.iter().copied())
        } else {
            builder
        };

        let builder = if self.is_min_deposit_term_greater_than_max() {
            builder
                .reset()
                .parameter("maxDepositTerm")
                .value(self.max_deposit_term)
                .fail_with_code_no_parameter_added_to_error_code("max.term.lessthan.min.term")
        } else {
            builder
        };

        let builder = if let Some(mult) = self.in_multiples_of_deposit_term {
            let b = builder
                .reset()
                .parameter("inMultiplesOfDepositTerm")
                .value(mult)
                .integer_greater_than_zero();
            b.reset()
                .parameter("inMultiplesOfDepositTermTypeId")
                .value(self.in_multiples_of_deposit_term_type.value())
                .is_one_of_these_values(SavingsPeriodFrequencyType::VALID_VALUES.iter().copied())
        } else {
            builder
        };

        let builder = if self.pre_closure_penal_applicable {
            let b = builder
                .reset()
                .parameter("preClosurePenalInterest")
                .value(self.pre_closure_penal_interest)
                .not_null()
                .zero_or_positive_amount();
            b.reset()
                .parameter("preClosurePenalInterestOnTypeId")
                .value(self.pre_closure_penal_interest_on_type.value())
                .is_one_of_these_values(
                    PreClosurePenalInterestOnType::VALID_VALUES.iter().copied(),
                )
        } else {
            builder
        };

        let builder = if let Some(min_amt) = self.min_deposit_amount {
            builder
                .reset()
                .parameter("depositMinAmount")
                .value(min_amt)
                .positive_amount()
        } else {
            builder
        };

        let builder = if let Some(max_amt) = self.max_deposit_amount {
            let b = builder
                .reset()
                .parameter("depositMaxAmount")
                .value(max_amt)
                .positive_amount();
            if let Some(min_amt) = self.min_deposit_amount {
                b.reset()
                    .parameter("depositMaxAmount")
                    .value(max_amt)
                    .not_less_than_min(min_amt)
            } else {
                b
            }
        } else {
            builder
        };

        let builder = if let Some(dep_amt) = self.deposit_amount {
            let b = builder
                .reset()
                .parameter("depositAmount")
                .value(dep_amt)
                .positive_amount();
            match (self.min_deposit_amount, self.max_deposit_amount) {
                (Some(min_a), Some(max_a)) if min_a <= max_a => b
                    .reset()
                    .parameter("depositAmount")
                    .value(dep_amt)
                    .in_min_and_max_amount_range(min_a, max_a),
                (Some(min_a), None) => b
                    .reset()
                    .parameter("depositAmount")
                    .value(dep_amt)
                    .not_less_than_min(min_a),
                (None, Some(max_a)) => b
                    .reset()
                    .parameter("depositAmount")
                    .value(dep_amt)
                    .not_greater_than_max(max_a),
                _ => b,
            }
        } else {
            builder
        };

        builder
            .throw_validation_errors()
            .map_err(FineractError::from)
    }
}

/// Recurring Deposit Product recurring deposit rules (`m_deposit_product_recurring_detail`).
///
/// Ported from `DepositProductRecurringDetail.java` and `DepositRecurringDetail.java`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringDepositProductTermConfig {
    pub id: Option<i64>,
    pub savings_product_id: Option<i64>,
    pub is_mandatory: bool,
    pub allow_withdrawal: bool,
    pub adjust_advance_towards_future_payments: bool,
    pub term_and_preclosure: Option<FixedDepositProductTermAndPreClosure>,
}

/// Alias matching upstream Java `DepositProductRecurringDetail`.
pub type DepositProductRecurringDetail = RecurringDepositProductTermConfig;
/// Alias matching upstream Java `DepositRecurringDetail`.
pub type DepositRecurringDetail = RecurringDepositProductTermConfig;

impl RecurringDepositProductTermConfig {
    #[inline]
    pub const fn new(
        is_mandatory: bool,
        allow_withdrawal: bool,
        adjust_advance_towards_future_payments: bool,
    ) -> Self {
        Self {
            id: None,
            savings_product_id: None,
            is_mandatory,
            allow_withdrawal,
            adjust_advance_towards_future_payments,
            term_and_preclosure: None,
        }
    }

    pub fn with_term_and_preclosure(
        mut self,
        term_and_preclosure: FixedDepositProductTermAndPreClosure,
    ) -> Self {
        self.term_and_preclosure = Some(term_and_preclosure);
        self
    }

    #[inline]
    pub const fn is_mandatory_deposit(&self) -> bool {
        self.is_mandatory
    }

    #[inline]
    pub const fn allow_withdrawal(&self) -> bool {
        self.allow_withdrawal
    }

    #[inline]
    pub const fn adjust_advance_towards_future_payments(&self) -> bool {
        self.adjust_advance_towards_future_payments
    }

    /// Validates any attached [`FixedDepositProductTermAndPreClosure`] configuration.
    pub fn validate(&self) -> FineractResult<()> {
        if let Some(ref term) = self.term_and_preclosure {
            term.validate()?;
        }
        Ok(())
    }
}

// =============================================================================
// 4. SAVINGS PRODUCT (`m_savings_product`)
// =============================================================================

/// Savings, Fixed Deposit, Recurring Deposit, or Current Account product definition
/// (`m_savings_product`).
///
/// Ported from `SavingsProduct.java`, `FixedDepositProduct.java`, `RecurringDepositProduct.java`,
/// `SavingsProductDataValidator.java`, and `DepositProductDataValidator.java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsProduct {
    pub id: i64,
    pub name: String,
    pub short_name: String,
    pub description: Option<String>,
    pub deposit_type: DepositAccountType,
    pub currency: MonetaryCurrency,
    pub nominal_annual_interest_rate: Decimal,
    pub compounding_period_type: SavingsCompoundingInterestPeriodType,
    pub interest_compounding_period_type: SavingsCompoundingInterestPeriodType,
    pub posting_period_type: SavingsPostingInterestPeriodType,
    pub interest_posting_period_type: SavingsPostingInterestPeriodType,
    pub interest_calculation_type: SavingsInterestCalculationType,
    pub interest_calculation_days_in_year_type: SavingsInterestCalculationDaysInYearType,
    pub min_required_opening_balance: Option<Decimal>,
    pub lockin_period_frequency: Option<i32>,
    pub lockin_period_frequency_type: Option<SavingsPeriodFrequencyType>,
    pub withdrawal_fee_for_transfer: bool,
    pub allow_overdraft: bool,
    pub overdraft_limit: Option<Decimal>,
    pub nominal_annual_interest_rate_overdraft: Option<Decimal>,
    pub min_overdraft_for_interest_calculation: Option<Decimal>,
    pub enforce_min_required_balance: bool,
    pub min_required_balance: Option<Decimal>,
    pub lien_allowed: bool,
    pub max_allowed_lien_limit: Option<Decimal>,
    pub min_balance_for_interest_calculation: Option<Decimal>,
    pub withhold_tax: bool,
    pub tax_group_id: Option<i64>,
    pub accounting_rule: AccountingRuleType,
    pub is_dormancy_tracking_active: bool,
    pub days_to_inactive: Option<i32>,
    pub days_to_dormancy: Option<i32>,
    pub days_to_escheat: Option<i32>,
    pub start_date: Option<NaiveDate>,
    pub close_date: Option<NaiveDate>,
    pub charge_ids: Vec<i64>,
    pub term_and_preclosure: Option<FixedDepositProductTermAndPreClosure>,
    pub recurring_config: Option<RecurringDepositProductTermConfig>,
    pub charts: Vec<InterestRateChart>,
}

impl Default for SavingsProduct {
    fn default() -> Self {
        Self::new(
            0,
            String::new(),
            String::new(),
            MonetaryCurrency::usd(),
            Decimal::ZERO,
        )
    }
}

impl SavingsProduct {
    /// Creates a standard [`DepositAccountType::SavingsDeposit`] product with sensible defaults
    /// (`Daily` compounding, `Monthly` posting, `DailyBalance` calculation, `Days365` year).
    pub fn new(
        id: i64,
        name: impl Into<String>,
        short_name: impl Into<String>,
        currency: MonetaryCurrency,
        nominal_annual_interest_rate: Decimal,
    ) -> Self {
        Self::with_terms(
            id,
            name,
            short_name,
            currency,
            nominal_annual_interest_rate,
            SavingsCompoundingInterestPeriodType::Daily,
            SavingsPostingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
        )
    }

    /// Creates a [`SavingsProduct`] with explicit compounding, posting, calculation, and days-in-year types.
    #[allow(clippy::too_many_arguments)]
    pub fn with_terms(
        id: i64,
        name: impl Into<String>,
        short_name: impl Into<String>,
        currency: MonetaryCurrency,
        nominal_annual_interest_rate: Decimal,
        compounding_period_type: SavingsCompoundingInterestPeriodType,
        posting_period_type: SavingsPostingInterestPeriodType,
        interest_calculation_type: SavingsInterestCalculationType,
        interest_calculation_days_in_year_type: SavingsInterestCalculationDaysInYearType,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            short_name: short_name.into(),
            description: None,
            deposit_type: DepositAccountType::SavingsDeposit,
            currency,
            nominal_annual_interest_rate,
            compounding_period_type,
            interest_compounding_period_type: compounding_period_type,
            posting_period_type,
            interest_posting_period_type: posting_period_type,
            interest_calculation_type,
            interest_calculation_days_in_year_type,
            min_required_opening_balance: None,
            lockin_period_frequency: None,
            lockin_period_frequency_type: None,
            withdrawal_fee_for_transfer: false,
            allow_overdraft: false,
            overdraft_limit: None,
            nominal_annual_interest_rate_overdraft: None,
            min_overdraft_for_interest_calculation: None,
            enforce_min_required_balance: false,
            min_required_balance: None,
            lien_allowed: false,
            max_allowed_lien_limit: None,
            min_balance_for_interest_calculation: None,
            withhold_tax: false,
            tax_group_id: None,
            accounting_rule: AccountingRuleType::None,
            is_dormancy_tracking_active: false,
            days_to_inactive: None,
            days_to_dormancy: None,
            days_to_escheat: None,
            start_date: None,
            close_date: None,
            charge_ids: Vec::new(),
            term_and_preclosure: None,
            recurring_config: None,
            charts: Vec::new(),
        }
    }

    pub const fn with_id(mut self, id: i64) -> Self {
        self.id = id;
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub const fn with_deposit_type(mut self, deposit_type: DepositAccountType) -> Self {
        self.deposit_type = deposit_type;
        self
    }

    pub const fn with_compounding_and_posting(
        mut self,
        compounding: SavingsCompoundingInterestPeriodType,
        posting: SavingsPostingInterestPeriodType,
    ) -> Self {
        self.compounding_period_type = compounding;
        self.interest_compounding_period_type = compounding;
        self.posting_period_type = posting;
        self.interest_posting_period_type = posting;
        self
    }

    pub const fn with_overdraft(
        mut self,
        allow_overdraft: bool,
        overdraft_limit: Option<Decimal>,
        nominal_annual_interest_rate_overdraft: Option<Decimal>,
        min_overdraft_for_interest_calculation: Option<Decimal>,
    ) -> Self {
        self.allow_overdraft = allow_overdraft;
        self.overdraft_limit = overdraft_limit;
        self.nominal_annual_interest_rate_overdraft = nominal_annual_interest_rate_overdraft;
        self.min_overdraft_for_interest_calculation = min_overdraft_for_interest_calculation;
        self
    }

    pub const fn with_lien(
        mut self,
        lien_allowed: bool,
        max_allowed_lien_limit: Option<Decimal>,
    ) -> Self {
        self.lien_allowed = lien_allowed;
        self.max_allowed_lien_limit = max_allowed_lien_limit;
        self
    }

    pub const fn with_withholding_tax(
        mut self,
        withhold_tax: bool,
        tax_group_id: Option<i64>,
    ) -> Self {
        self.withhold_tax = withhold_tax;
        self.tax_group_id = tax_group_id;
        self
    }

    pub const fn with_min_required_balance(
        mut self,
        enforce: bool,
        min_required_balance: Option<Decimal>,
    ) -> Self {
        self.enforce_min_required_balance = enforce;
        self.min_required_balance = min_required_balance;
        self
    }

    pub const fn with_lockin_period(
        mut self,
        frequency: i32,
        frequency_type: SavingsPeriodFrequencyType,
    ) -> Self {
        self.lockin_period_frequency = Some(frequency);
        self.lockin_period_frequency_type = Some(frequency_type);
        self
    }

    pub const fn with_dormancy_tracking(
        mut self,
        active: bool,
        days_to_inactive: Option<i32>,
        days_to_dormancy: Option<i32>,
        days_to_escheat: Option<i32>,
    ) -> Self {
        self.is_dormancy_tracking_active = active;
        self.days_to_inactive = days_to_inactive;
        self.days_to_dormancy = days_to_dormancy;
        self.days_to_escheat = days_to_escheat;
        self
    }

    pub fn with_term_and_preclosure(
        mut self,
        term_and_preclosure: FixedDepositProductTermAndPreClosure,
    ) -> Self {
        self.term_and_preclosure = Some(term_and_preclosure);
        self
    }

    pub fn with_recurring_config(
        mut self,
        recurring_config: RecurringDepositProductTermConfig,
    ) -> Self {
        self.recurring_config = Some(recurring_config);
        self
    }

    pub fn with_chart(mut self, chart: InterestRateChart) -> Self {
        self.charts.push(chart);
        self
    }

    /// Resolves the effective compounding period type whether mutated via `compounding_period_type`
    /// or `interest_compounding_period_type`.
    #[inline]
    pub const fn effective_compounding_period_type(&self) -> SavingsCompoundingInterestPeriodType {
        if !matches!(
            self.compounding_period_type,
            SavingsCompoundingInterestPeriodType::Daily
        ) {
            self.compounding_period_type
        } else {
            self.interest_compounding_period_type
        }
    }

    /// Resolves the effective posting period type whether mutated via `posting_period_type`
    /// or `interest_posting_period_type`.
    #[inline]
    pub const fn effective_posting_period_type(&self) -> SavingsPostingInterestPeriodType {
        if !matches!(
            self.posting_period_type,
            SavingsPostingInterestPeriodType::Monthly
        ) {
            self.posting_period_type
        } else {
            self.interest_posting_period_type
        }
    }

    /// Normalizes overdraft limits when `allow_overdraft` is enabled (`SavingsProduct.esnureOverdraftLimitsSetForOverdraftAccounts`).
    pub fn ensure_overdraft_limits_defaulted(&mut self) {
        if self.allow_overdraft {
            if self.overdraft_limit.is_none() {
                self.overdraft_limit = Some(Decimal::ZERO);
            }
            if self.nominal_annual_interest_rate_overdraft.is_none() {
                self.nominal_annual_interest_rate_overdraft = Some(Decimal::ZERO);
            }
            if self.min_overdraft_for_interest_calculation.is_none() {
                self.min_overdraft_for_interest_calculation = Some(Decimal::ZERO);
            }
        } else {
            self.overdraft_limit = None;
            self.nominal_annual_interest_rate_overdraft = None;
            self.min_overdraft_for_interest_calculation = None;
        }
    }

    /// Finds the [`InterestRateChart`] applicable on `target_date` (`FixedDepositProduct.applicableChart`).
    pub fn applicable_chart(&self, target_date: NaiveDate) -> Option<&InterestRateChart> {
        self.charts
            .iter()
            .find(|chart| chart.is_applicable_for_date(target_date))
    }

    /// Validates all product domain and parameter rules:
    /// - `compounding_period_type <= posting_period_type`
    /// - valid overdraft and lien configuration
    /// - valid withholding tax configuration (`withhold_tax` requires `tax_group_id: Some(id > 0)`,
    ///   and `!withhold_tax` forbids `tax_group_id`)
    /// - valid lock-in period, dormancy tracking, deposit terms, and interest rate charts.
    pub fn validate(&self) -> Result<(), PlatformApiDataValidationException> {
        let resource = self.deposit_type.product_resource_name();
        let builder = DataValidatorBuilder::for_resource(resource);

        // 1. Basic identity & currency
        let builder = builder
            .reset()
            .parameter("name")
            .value(self.name.as_str())
            .not_blank()
            .not_exceeding_length_of(100);

        let builder = builder
            .reset()
            .parameter("shortName")
            .value(self.short_name.as_str())
            .not_blank()
            .not_exceeding_length_of(4);

        let builder = if let Some(ref desc) = self.description {
            builder
                .reset()
                .parameter("description")
                .value(desc.as_str())
                .not_exceeding_length_of(500)
        } else {
            builder
        };

        let builder = builder
            .reset()
            .parameter("currencyCode")
            .value(self.currency.code())
            .not_blank();

        let builder = builder
            .reset()
            .parameter("digitsAfterDecimal")
            .value(i64::from(self.currency.digits_after_decimal()))
            .in_min_max_range(0, 6);

        let builder = builder
            .reset()
            .parameter("nominalAnnualInterestRate")
            .value(self.nominal_annual_interest_rate)
            .zero_or_positive_amount();

        // 2. Compounding & Posting Period Types (`compounding_period_type <= posting_period_type`)
        let comp_period = self.effective_compounding_period_type();
        let post_period = self.effective_posting_period_type();

        let builder = builder
            .reset()
            .parameter("interestCompoundingPeriodType")
            .value(comp_period.value())
            .is_one_of_these_values(
                SavingsCompoundingInterestPeriodType::VALID_VALUES
                    .iter()
                    .copied(),
            );

        let builder = builder
            .reset()
            .parameter("interestPostingPeriodType")
            .value(post_period.value())
            .is_one_of_these_values(
                SavingsPostingInterestPeriodType::VALID_VALUES
                    .iter()
                    .copied(),
            );

        let builder = if !comp_period.is_invalid()
            && !post_period.is_invalid()
            && comp_period > post_period
        {
            builder.fail_with_code_no_parameter_added_to_error_code(
                "posting.period.type.is.less.than.compound.period.type",
            )
        } else {
            builder
        };

        let builder = builder
            .reset()
            .parameter("interestCalculationType")
            .value(self.interest_calculation_type.value())
            .is_one_of_these_values(
                SavingsInterestCalculationType::VALID_VALUES.iter().copied(),
            );

        let builder = builder
            .reset()
            .parameter("interestCalculationDaysInYearType")
            .value(self.interest_calculation_days_in_year_type.value())
            .is_one_of_these_values(
                SavingsInterestCalculationDaysInYearType::VALID_VALUES
                    .iter()
                    .copied(),
            );

        // 3. Overdraft & Lien validation
        let builder = if self.allow_overdraft {
            let b = if matches!(
                self.deposit_type,
                DepositAccountType::FixedDeposit | DepositAccountType::RecurringDeposit
            ) {
                builder.reset().parameter("allowOverdraft").fail_with_code(
                    "not.allowed.for.deposit.products",
                    "Overdraft is not allowed for Fixed or Recurring Deposit products.",
                )
            } else {
                builder
            };

            let b = if let Some(limit) = self.overdraft_limit {
                b.reset()
                    .parameter("overdraftLimit")
                    .value(limit)
                    .zero_or_positive_amount()
            } else {
                b
            };

            let b = if let Some(od_rate) = self.nominal_annual_interest_rate_overdraft {
                b.reset()
                    .parameter("nominalAnnualInterestRateOverdraft")
                    .value(od_rate)
                    .zero_or_positive_amount()
            } else {
                b
            };

            let b = if let Some(min_od) = self.min_overdraft_for_interest_calculation {
                b.reset()
                    .parameter("minOverdraftForInterestCalculation")
                    .value(min_od)
                    .zero_or_positive_amount()
            } else {
                b
            };

            if self.lien_allowed {
                let od_limit = self.overdraft_limit.unwrap_or(Decimal::ZERO);
                let lien_limit = self.max_allowed_lien_limit.unwrap_or(Decimal::ZERO);
                if od_limit > lien_limit {
                    b.fail_with_code_no_parameter_added_to_error_code(
                        "Overdraft.limit.can.not.be.greater.than.lien.limit",
                    )
                } else {
                    b
                }
            } else {
                b
            }
        } else {
            let b = if self.overdraft_limit.is_some() {
                builder.reset().parameter("overdraftLimit").fail_with_code(
                    "not.allowed.when.overdraft.disabled",
                    "Overdraft limit cannot be set when allowOverdraft is false.",
                )
            } else {
                builder
            };
            let b = if self.nominal_annual_interest_rate_overdraft.is_some() {
                b.reset()
                    .parameter("nominalAnnualInterestRateOverdraft")
                    .fail_with_code(
                        "not.allowed.when.overdraft.disabled",
                        "Overdraft interest rate cannot be set when allowOverdraft is false.",
                    )
            } else {
                b
            };
            if self.min_overdraft_for_interest_calculation.is_some() {
                b.reset()
                    .parameter("minOverdraftForInterestCalculation")
                    .fail_with_code(
                        "not.allowed.when.overdraft.disabled",
                        "Minimum overdraft for interest calculation cannot be set when allowOverdraft is false.",
                    )
            } else {
                b
            }
        };

        let builder = if let Some(max_lien) = self.max_allowed_lien_limit {
            builder
                .reset()
                .parameter("maxAllowedLienLimit")
                .value(max_lien)
                .zero_or_positive_amount()
        } else {
            builder
        };

        // 4. Withholding Tax configuration (`withhold_tax: bool`, `tax_group_id: Option<i64>`)
        let builder = if self.withhold_tax {
            builder
                .reset()
                .parameter("taxGroupId")
                .value(self.tax_group_id)
                .not_null()
                .long_greater_than_zero()
        } else if self.tax_group_id.is_some() {
            builder.reset().parameter("taxGroupId").fail_with_code(
                "not.allowed.when.withhold.tax.disabled",
                "taxGroupId cannot be provided when withHoldTax is false.",
            )
        } else {
            builder
        };

        // 5. Lock-in period validation (`SavingsProduct.validateLockinDetails`)
        let builder = match (self.lockin_period_frequency, self.lockin_period_frequency_type) {
            (Some(freq), Some(freq_type)) => {
                let b = builder
                    .reset()
                    .parameter("lockinPeriodFrequency")
                    .value(freq)
                    .integer_zero_or_greater();
                b.reset()
                    .parameter("lockinPeriodFrequencyType")
                    .value(i64::from(freq_type.value()))
                    .in_min_max_range(0, 3)
            }
            (Some(freq), None) => {
                let b = builder
                    .reset()
                    .parameter("lockinPeriodFrequency")
                    .value(freq)
                    .integer_zero_or_greater();
                b.reset()
                    .parameter("lockinPeriodFrequencyType")
                    .value(None::<i32>)
                    .not_null()
            }
            (None, Some(freq_type)) if !freq_type.is_invalid() => builder
                .reset()
                .parameter("lockinPeriodFrequency")
                .value(None::<i32>)
                .not_null(),
            _ => builder,
        };

        // 6. Dormancy tracking validation
        let builder = if self.is_dormancy_tracking_active {
            let b = builder
                .reset()
                .parameter("daysToInactive")
                .value(self.days_to_inactive)
                .not_null()
                .integer_greater_than_zero();
            let b = b
                .reset()
                .parameter("daysToDormancy")
                .value(self.days_to_dormancy)
                .not_null()
                .integer_greater_than_zero();
            let b = b
                .reset()
                .parameter("daysToEscheat")
                .value(self.days_to_escheat)
                .not_null()
                .integer_greater_than_zero();
            match (
                self.days_to_inactive,
                self.days_to_dormancy,
                self.days_to_escheat,
            ) {
                (Some(inact), Some(dorm), Some(esch)) => {
                    let b = b
                        .reset()
                        .parameter("daysToDormancy")
                        .value(dorm)
                        .integer_greater_than_number(i64::from(inact));
                    b.reset()
                        .parameter("daysToEscheat")
                        .value(esch)
                        .integer_greater_than_number(i64::from(dorm))
                }
                _ => b,
            }
        } else {
            builder
        };

        // 7. Start and close date ordering
        let builder = match (self.start_date, self.close_date) {
            (Some(start), Some(close)) if close < start => builder
                .reset()
                .parameter("closeDate")
                .value(close)
                .fail_with_code(
                    "must.be.after.startDate",
                    "Close date must be on or after start date.",
                ),
            _ => builder,
        };

        builder.throw_validation_errors()?;

        // 8. Validate Fixed / Recurring Deposit sub-configurations and interest rate charts
        if let Some(ref term) = self.term_and_preclosure {
            term.validate().map_err(|e| match e {
                FineractError::DataValidation(dv) => dv,
                other => PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.fixeddepositproduct.term.invalid",
                    other.to_string(),
                    "termAndPreClosure",
                    vec![],
                ),
            })?;
        }
        if let Some(ref rec) = self.recurring_config {
            rec.validate().map_err(|e| match e {
                FineractError::DataValidation(dv) => dv,
                other => PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.recurringdepositproduct.term.invalid",
                    other.to_string(),
                    "recurringConfig",
                    vec![],
                ),
            })?;
        }

        for (i, chart) in self.charts.iter().enumerate() {
            chart.validate().map_err(|e| match e {
                FineractError::DataValidation(dv) => dv,
                other => PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.interest.rate.chart.invalid",
                    other.to_string(),
                    "charts",
                    vec![],
                ),
            })?;
            for other in self.charts.iter().skip(i + 1) {
                let ref_date = self
                    .close_date
                    .or(chart.end_date)
                    .or(other.end_date)
                    .unwrap_or(chart.from_date.max(other.from_date));
                if chart.is_overlapping(other, ref_date) {
                    let b = DataValidatorBuilder::for_resource(resource)
                        .fail_with_code_no_parameter_added_to_error_code(
                            "chart.overlapping.from.and.end.dates",
                        );
                    return b.throw_validation_errors();
                }
            }
        }

        Ok(())
    }

    /// Alias for [`SavingsProduct::validate`].
    #[inline]
    pub fn validate_domain_rules(&self) -> FineractResult<()> {
        self.validate().map_err(FineractError::from)
    }

    /// Parses a [`SavingsProduct`] from a Fineract [`JsonCommand`].
    pub fn from_json(id: Option<i64>, command: &JsonCommand) -> FineractResult<Self> {
        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let short_name = command
            .string_value_of_parameter_named("shortName")
            .unwrap_or_default();
        let description = command.string_value_of_parameter_named("description");
        let currency_code = command
            .string_value_of_parameter_named("currencyCode")
            .unwrap_or_else(|| "USD".to_string());
        let digits_after_decimal = command
            .integer_value_sans_locale_of_parameter_named("digitsAfterDecimal")?
            .unwrap_or(2)
            .clamp(0, 6) as u32;
        let in_multiples_of = command.integer_value_of_parameter_named("inMultiplesOf")?;
        let currency =
            MonetaryCurrency::new(currency_code, digits_after_decimal, in_multiples_of);

        let nominal_annual_interest_rate = command
            .big_decimal_value_of_parameter_named("nominalAnnualInterestRate")?
            .unwrap_or(Decimal::ZERO);

        let compounding_period_type = SavingsCompoundingInterestPeriodType::from_int(
            command.integer_value_sans_locale_of_parameter_named("interestCompoundingPeriodType")?,
        );
        let posting_period_type = SavingsPostingInterestPeriodType::from_int(
            command.integer_value_sans_locale_of_parameter_named("interestPostingPeriodType")?,
        );
        let interest_calculation_type = SavingsInterestCalculationType::from_int(
            command.integer_value_sans_locale_of_parameter_named("interestCalculationType")?,
        );
        let interest_calculation_days_in_year_type =
            SavingsInterestCalculationDaysInYearType::from_int(
                command.integer_value_sans_locale_of_parameter_named(
                    "interestCalculationDaysInYearType",
                )?,
            );

        let mut product = Self::with_terms(
            id.unwrap_or(0),
            name,
            short_name,
            currency,
            nominal_annual_interest_rate,
            compounding_period_type,
            posting_period_type,
            interest_calculation_type,
            interest_calculation_days_in_year_type,
        );
        product.description = description;
        product.min_required_opening_balance =
            command.big_decimal_value_of_parameter_named("minRequiredOpeningBalance")?;
        product.lockin_period_frequency =
            command.integer_value_of_parameter_named("lockinPeriodFrequency")?;
        product.lockin_period_frequency_type = command
            .integer_value_sans_locale_of_parameter_named("lockinPeriodFrequencyType")?
            .map(SavingsPeriodFrequencyType::from_i32);
        product.withdrawal_fee_for_transfer =
            command.boolean_primitive_value_of_parameter_named("withdrawalFeeForTransfers");
        product.allow_overdraft =
            command.boolean_primitive_value_of_parameter_named("allowOverdraft");
        product.overdraft_limit = command.big_decimal_value_of_parameter_named("overdraftLimit")?;
        product.nominal_annual_interest_rate_overdraft = command
            .big_decimal_value_of_parameter_named("nominalAnnualInterestRateOverdraft")?;
        product.min_overdraft_for_interest_calculation = command
            .big_decimal_value_of_parameter_named("minOverdraftForInterestCalculation")?;
        product.enforce_min_required_balance =
            command.boolean_primitive_value_of_parameter_named("enforceMinRequiredBalance");
        product.min_required_balance =
            command.big_decimal_value_of_parameter_named("minRequiredBalance")?;
        product.lien_allowed = command.boolean_primitive_value_of_parameter_named("lienAllowed");
        product.max_allowed_lien_limit =
            command.big_decimal_value_of_parameter_named("maxAllowedLienLimit")?;
        product.min_balance_for_interest_calculation =
            command.big_decimal_value_of_parameter_named("minBalanceForInterestCalculation")?;
        product.withhold_tax = command.boolean_primitive_value_of_parameter_named("withHoldTax");
        product.tax_group_id = command.long_value_of_parameter_named("taxGroupId")?;
        product.accounting_rule = command
            .integer_value_of_parameter_named("accountingRule")?
            .and_then(|v| AccountingRuleType::from_i16(v as i16))
            .unwrap_or(AccountingRuleType::None);
        product.is_dormancy_tracking_active =
            command.boolean_primitive_value_of_parameter_named("isDormancyTrackingActive");
        product.days_to_inactive = command.integer_value_of_parameter_named("daysToInactive")?;
        product.days_to_dormancy = command.integer_value_of_parameter_named("daysToDormancy")?;
        product.days_to_escheat = command.integer_value_of_parameter_named("daysToEscheat")?;
        product.start_date = command.local_date_value_of_parameter_named("startDate")?;
        product.close_date = command.local_date_value_of_parameter_named("closeDate")?;

        product.validate()?;
        product.ensure_overdraft_limits_defaulted();
        Ok(product)
    }

    /// Applies an update [`JsonCommand`] to this [`SavingsProduct`], returning changed fields.
    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        let mut changes_map = std::collections::HashMap::new();
        if let Some(new_name) = command.update_string_if_changed("name", Some(&self.name), &mut changes_map) {
            self.name = new_name;
        }
        if let Some(new_short_name) = command.update_string_if_changed("shortName", Some(&self.short_name), &mut changes_map) {
            self.short_name = new_short_name;
        }
        if let Some(new_rate) = command.update_decimal_if_changed(
            "nominalAnnualInterestRate",
            Some(self.nominal_annual_interest_rate),
            &mut changes_map,
        )? {
            self.nominal_annual_interest_rate = new_rate;
        }
        let mut changes: BTreeMap<String, Value> = changes_map.into_iter().collect();
        if command.parameter_exists("description") {
            let new_desc = command.string_value_of_parameter_named("description");
            if new_desc != self.description {
                changes.insert(
                    "description".to_string(),
                    new_desc.clone().map(Value::String).unwrap_or(Value::Null),
                );
                self.description = new_desc;
            }
        }
        if let Some(comp) =
            command.integer_value_sans_locale_of_parameter_named("interestCompoundingPeriodType")?
        {
            let new_type = SavingsCompoundingInterestPeriodType::from_i32(comp);
            if new_type != self.effective_compounding_period_type() {
                changes.insert("interestCompoundingPeriodType".to_string(), json!(comp));
                self.compounding_period_type = new_type;
                self.interest_compounding_period_type = new_type;
            }
        }
        if let Some(post) =
            command.integer_value_sans_locale_of_parameter_named("interestPostingPeriodType")?
        {
            let new_type = SavingsPostingInterestPeriodType::from_i32(post);
            if new_type != self.effective_posting_period_type() {
                changes.insert("interestPostingPeriodType".to_string(), json!(post));
                self.posting_period_type = new_type;
                self.interest_posting_period_type = new_type;
            }
        }
        if command.parameter_exists("allowOverdraft") {
            let new_val = command.boolean_primitive_value_of_parameter_named("allowOverdraft");
            if new_val != self.allow_overdraft {
                changes.insert("allowOverdraft".to_string(), json!(new_val));
                self.allow_overdraft = new_val;
            }
        }
        if command.parameter_exists("overdraftLimit") {
            let new_val = command
                .big_decimal_value_of_parameter_named_default_to_null_if_zero("overdraftLimit")?;
            if new_val != self.overdraft_limit {
                changes.insert("overdraftLimit".to_string(), json!(new_val));
                self.overdraft_limit = new_val;
            }
        }
        if command.parameter_exists("withHoldTax") {
            let new_val = command.boolean_primitive_value_of_parameter_named("withHoldTax");
            if new_val != self.withhold_tax {
                changes.insert("withHoldTax".to_string(), json!(new_val));
                self.withhold_tax = new_val;
            }
        }
        if command.parameter_exists("taxGroupId") {
            let new_val = command.long_value_of_parameter_named("taxGroupId")?;
            if new_val != self.tax_group_id {
                changes.insert("taxGroupId".to_string(), json!(new_val));
                self.tax_group_id = new_val;
            }
        }

        self.validate()?;
        self.ensure_overdraft_limits_defaulted();
        Ok(changes)
    }
}

/// Helper error constructor for missing `SavingsProduct` (`SavingsProductNotFoundException.java`).
pub fn savings_product_not_found_error(id: i64) -> FineractError {
    FineractError::resource_not_found(
        "error.msg.savingproduct.id.invalid",
        format!("Savings product with identifier {id} does not exist"),
        id.to_string(),
    )
}

/// Helper error constructor for missing `FixedDepositProduct` (`FixedDepositProductNotFoundException.java`).
pub fn fixed_deposit_product_not_found_error(id: i64) -> FineractError {
    FineractError::resource_not_found(
        "error.msg.fixeddepositproduct.id.invalid",
        format!("Fixed deposit product with identifier {id} does not exist"),
        id.to_string(),
    )
}

/// Helper error constructor for missing `RecurringDepositProduct` (`RecurringDepositProductNotFoundException.java`).
pub fn recurring_deposit_product_not_found_error(id: i64) -> FineractError {
    FineractError::resource_not_found(
        "error.msg.recurringdepositproduct.id.invalid",
        format!("Recurring deposit product with identifier {id} does not exist"),
        id.to_string(),
    )
}

/// Helper error constructor for missing `InterestRateChart` (`InterestRateChartNotFoundException.java`).
pub fn interest_rate_chart_not_found_error(id: i64) -> FineractError {
    FineractError::resource_not_found(
        "error.msg.interest.rate.chart.id.invalid",
        format!("Interest rate chart with identifier {id} does not exist"),
        id.to_string(),
    )
}

// =============================================================================
// 5. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[test]
    fn savings_and_deposit_enum_discriminants_match_java() {
        assert_eq!(DepositAccountType::Invalid as i16, 0);
        assert_eq!(DepositAccountType::SavingsDeposit as i16, 100);
        assert_eq!(DepositAccountType::FixedDeposit as i16, 200);
        assert_eq!(DepositAccountType::RecurringDeposit as i16, 300);
        assert_eq!(DepositAccountType::CurrentAccount as i16, 400);
        assert_eq!(
            DepositAccountType::from_int(200),
            DepositAccountType::FixedDeposit
        );
        assert_eq!(
            DepositAccountType::from_int(999),
            DepositAccountType::Invalid
        );

        assert_eq!(SavingsCompoundingInterestPeriodType::Invalid as i16, 0);
        assert_eq!(SavingsCompoundingInterestPeriodType::Daily as i16, 1);
        assert_eq!(SavingsCompoundingInterestPeriodType::Monthly as i16, 4);
        assert_eq!(SavingsCompoundingInterestPeriodType::Quarterly as i16, 5);
        assert_eq!(SavingsCompoundingInterestPeriodType::BiAnnual as i16, 6);
        assert_eq!(SavingsCompoundingInterestPeriodType::Annual as i16, 7);

        assert_eq!(SavingsPostingInterestPeriodType::Invalid as i16, 0);
        assert_eq!(SavingsPostingInterestPeriodType::Monthly as i16, 4);
        assert_eq!(SavingsPostingInterestPeriodType::Quarterly as i16, 5);
        assert_eq!(SavingsPostingInterestPeriodType::BiAnnual as i16, 6);
        assert_eq!(SavingsPostingInterestPeriodType::Annual as i16, 7);

        assert_eq!(SavingsInterestCalculationType::Invalid as i16, 0);
        assert_eq!(SavingsInterestCalculationType::DailyBalance as i16, 1);
        assert_eq!(
            SavingsInterestCalculationType::AverageDailyBalance as i16,
            2
        );

        assert_eq!(SavingsInterestCalculationDaysInYearType::Invalid as i16, 0);
        assert_eq!(
            SavingsInterestCalculationDaysInYearType::Days360 as i16,
            360
        );
        assert_eq!(
            SavingsInterestCalculationDaysInYearType::Days365 as i16,
            365
        );

        assert_eq!(SavingsPeriodFrequencyType::Days as i16, 0);
        assert_eq!(SavingsPeriodFrequencyType::Weeks as i16, 1);
        assert_eq!(SavingsPeriodFrequencyType::Months as i16, 2);
        assert_eq!(SavingsPeriodFrequencyType::Years as i16, 3);
        assert_eq!(SavingsPeriodFrequencyType::Invalid as i16, 4);

        assert_eq!(PreClosurePenalInterestOnType::Invalid as i16, 0);
        assert_eq!(PreClosurePenalInterestOnType::WholeTerm as i16, 1);
        assert_eq!(
            PreClosurePenalInterestOnType::TillPrematureWithdrawal as i16,
            2
        );

        assert_eq!(IncentiveAttributeName::Gender as i16, 2);
        assert_eq!(IncentiveAttributeName::Age as i16, 3);
        assert_eq!(IncentiveAttributeName::ClientType as i16, 4);
        assert_eq!(IncentiveAttributeName::ClientClassification as i16, 5);

        assert_eq!(IncentiveConditionType::Equal as i16, 2);
        assert_eq!(IncentiveConditionType::NotEqual as i16, 3);
        assert_eq!(IncentiveConditionType::LessThan as i16, 4);
        assert_eq!(IncentiveConditionType::GreaterThan as i16, 5);

        assert_eq!(IncentiveType::Entity as i16, 1);
        assert_eq!(IncentiveType::Incentive as i16, 2);
    }

    #[test]
    fn interest_rate_chart_slab_lookup_and_client_incentives() {
        let senior_citizen_incentive = InterestRateChartIncentive::new(
            IncentiveAttributeName::Age,
            IncentiveConditionType::GreaterThan,
            "60",
            IncentiveType::Incentive,
            dec!(0.50),
        );
        let female_incentive = InterestRateChartIncentive::new(
            IncentiveAttributeName::Gender,
            IncentiveConditionType::Equal,
            "2",
            IncentiveType::Incentive,
            dec!(0.25),
        );
        let vip_fixed_override = InterestRateChartIncentive::new(
            IncentiveAttributeName::ClientClassification,
            IncentiveConditionType::Equal,
            "10",
            IncentiveType::Entity,
            dec!(9.00),
        );

        let slab_1_to_6_months = InterestRateChartSlab::for_period_range(
            SavingsPeriodFrequencyType::Months,
            1,
            Some(6),
            dec!(5.00),
            "USD",
        )
        .with_amount_range(Some(dec!(0)), Some(dec!(10000)))
        .with_incentive(senior_citizen_incentive.clone())
        .with_incentive(female_incentive.clone());

        let slab_7_to_12_months = InterestRateChartSlab::for_period_range(
            SavingsPeriodFrequencyType::Months,
            7,
            Some(12),
            dec!(6.50),
            "USD",
        )
        .with_amount_range(Some(dec!(0)), Some(dec!(10000)))
        .with_incentive(senior_citizen_incentive)
        .with_incentive(vip_fixed_override);

        let chart = InterestRateChart::new(
            "FD Chart 2026",
            sample_date(2026, 1, 1),
            Some(sample_date(2026, 12, 31)),
            false,
            vec![slab_1_to_6_months, slab_7_to_12_months],
        );
        assert!(chart.validate().is_ok());

        // 1. Standard client (no matching incentives)
        let regular_client = ClientIncentiveContext::new(Some(1), Some(35), None, None);
        assert_eq!(
            chart.find_applicable_interest_rate(
                dec!(5000),
                3,
                SavingsPeriodFrequencyType::Months,
                &regular_client,
            ),
            Some(dec!(5.00))
        );

        // 2. Senior female client (both +0.50% and +0.25% incentives apply)
        let senior_female = ClientIncentiveContext::new(Some(2), Some(65), None, None);
        assert_eq!(
            chart.find_applicable_interest_rate(
                dec!(5000),
                6,
                SavingsPeriodFrequencyType::Months,
                &senior_female,
            ),
            Some(dec!(5.75))
        );

        // 3. VIP client on 12-month deposit (Entity/fixed override sets rate to 9.00%)
        let vip_client = ClientIncentiveContext::new(Some(1), Some(40), None, Some(10));
        assert_eq!(
            chart.find_applicable_interest_rate(
                dec!(8000),
                12,
                SavingsPeriodFrequencyType::Months,
                &vip_client,
            ),
            Some(dec!(9.00))
        );

        // 4. Out-of-range period or amount returns None
        assert_eq!(
            chart.find_applicable_interest_rate(
                dec!(5000),
                24,
                SavingsPeriodFrequencyType::Months,
                &regular_client,
            ),
            None
        );
        assert_eq!(
            chart.find_applicable_interest_rate(
                dec!(25000),
                6,
                SavingsPeriodFrequencyType::Months,
                &regular_client,
            ),
            None
        );
    }

    #[test]
    fn interest_rate_chart_detects_overlapping_and_gapped_slabs() {
        let s1 = InterestRateChartSlab::for_period_range(
            SavingsPeriodFrequencyType::Months,
            1,
            Some(6),
            dec!(5.0),
            "USD",
        );
        let s2_overlap = InterestRateChartSlab::for_period_range(
            SavingsPeriodFrequencyType::Months,
            5,
            Some(12),
            dec!(6.0),
            "USD",
        );
        let overlapping_chart = InterestRateChart::new(
            "Overlapping Chart",
            sample_date(2026, 1, 1),
            None,
            false,
            vec![s1.clone(), s2_overlap],
        );
        assert!(overlapping_chart.validate().is_err());

        let s2_gap = InterestRateChartSlab::for_period_range(
            SavingsPeriodFrequencyType::Months,
            8,
            Some(12),
            dec!(6.0),
            "USD",
        );
        let gapped_chart = InterestRateChart::new(
            "Gapped Chart",
            sample_date(2026, 1, 1),
            None,
            false,
            vec![s1, s2_gap],
        );
        assert!(gapped_chart.validate().is_err());
    }

    #[test]
    fn savings_product_validates_compounding_le_posting_overdraft_and_withhold_tax() {
        let mut product = SavingsProduct::with_terms(
            1,
            "Regular Savings",
            "RS01",
            MonetaryCurrency::usd(),
            dec!(4.50),
            SavingsCompoundingInterestPeriodType::Daily,
            SavingsPostingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
        );
        assert!(product.validate().is_ok());

        // Verify 5-arg SavingsProduct::new constructor as well
        let default_product = SavingsProduct::new(
            2,
            "Standard Savings",
            "SS01",
            MonetaryCurrency::usd(),
            dec!(5.00),
        );
        assert!(default_product.validate().is_ok());

        // 1. Compounding period > Posting period must fail (e.g., Annual compounding with Monthly posting)
        product.compounding_period_type = SavingsCompoundingInterestPeriodType::Annual;
        product.posting_period_type = SavingsPostingInterestPeriodType::Monthly;
        assert!(product.compounding_period_type > product.posting_period_type);
        let err = product.validate().expect_err("compounding > posting must fail");
        assert!(err
            .errors
            .iter()
            .any(|e| e
                .user_message_globalisation_code
                .contains("posting.period.type.is.less.than.compound.period.type")));

        // Restore valid compounding <= posting
        product.compounding_period_type = SavingsCompoundingInterestPeriodType::Monthly;
        assert!(product.compounding_period_type <= product.posting_period_type);
        assert!(product.validate().is_ok());

        // 2. Invalid overdraft configuration: overdraft_limit set while allow_overdraft is false
        let invalid_od_disabled = product
            .clone()
            .with_overdraft(false, Some(dec!(1000)), None, None);
        assert!(invalid_od_disabled.validate().is_err());

        // Negative overdraft limit when allow_overdraft is true must fail
        let invalid_od_negative = product
            .clone()
            .with_overdraft(true, Some(dec!(-100)), Some(dec!(12.0)), None);
        assert!(invalid_od_negative.validate().is_err());

        // Overdraft limit > max_allowed_lien_limit when both overdraft and lien are enabled must fail
        let invalid_od_vs_lien = product
            .clone()
            .with_overdraft(true, Some(dec!(5000)), Some(dec!(10.0)), Some(dec!(100)))
            .with_lien(true, Some(dec!(2000)));
        assert!(invalid_od_vs_lien.validate().is_err());

        // Valid overdraft configuration succeeds
        let valid_od = product
            .clone()
            .with_overdraft(true, Some(dec!(2000)), Some(dec!(10.0)), Some(dec!(100)))
            .with_lien(true, Some(dec!(5000)));
        assert!(valid_od.validate().is_ok());

        // 3. Withholding tax validation: withhold_tax = true requires tax_group_id
        let missing_tax_group = product.clone().with_withholding_tax(true, None);
        assert!(missing_tax_group.validate().is_err());

        // withhold_tax = false with Some(tax_group_id) must fail
        let unexpected_tax_group = product.clone().with_withholding_tax(false, Some(10));
        assert!(unexpected_tax_group.validate().is_err());

        // Valid withholding tax configuration succeeds
        let valid_tax = product.with_withholding_tax(true, Some(10));
        assert!(valid_tax.validate().is_ok());
    }

    #[test]
    fn fixed_and_recurring_deposit_term_and_preclosure_validation() {
        let valid_term = FixedDepositProductTermAndPreClosure::new(
            3,
            SavingsPeriodFrequencyType::Months,
            Some(12),
            SavingsPeriodFrequencyType::Months,
        )
        .with_in_multiples_of(3, SavingsPeriodFrequencyType::Months)
        .with_pre_closure_penalty(
            true,
            Some(dec!(1.50)),
            PreClosurePenalInterestOnType::WholeTerm,
        )
        .with_deposit_amounts(Some(dec!(1000)), Some(dec!(5000)), Some(dec!(100000)));

        assert!(valid_term.validate().is_ok());
        assert!(valid_term.is_term_within_limits(6, SavingsPeriodFrequencyType::Months));
        assert!(!valid_term.is_term_within_limits(2, SavingsPeriodFrequencyType::Months));
        assert!(valid_term.is_valid_in_multiples_of_period(9, SavingsPeriodFrequencyType::Months));
        assert!(!valid_term.is_valid_in_multiples_of_period(7, SavingsPeriodFrequencyType::Months));

        // Min deposit term > Max deposit term (e.g., 2 Years > 12 Months) must fail
        let invalid_min_gt_max = FixedDepositProductTermAndPreClosure::new(
            2,
            SavingsPeriodFrequencyType::Years,
            Some(12),
            SavingsPeriodFrequencyType::Months,
        );
        assert!(invalid_min_gt_max.validate().is_err());

        // Pre-closure penal applicable without penal interest rate must fail
        let invalid_preclosure = FixedDepositProductTermAndPreClosure::new(
            3,
            SavingsPeriodFrequencyType::Months,
            Some(12),
            SavingsPeriodFrequencyType::Months,
        )
        .with_pre_closure_penalty(true, None, PreClosurePenalInterestOnType::WholeTerm);
        assert!(invalid_preclosure.validate().is_err());

        // Recurring deposit config validation
        let rd_config =
            RecurringDepositProductTermConfig::new(true, false, true).with_term_and_preclosure(valid_term);
        assert!(rd_config.validate().is_ok());
        assert!(rd_config.is_mandatory_deposit());
        assert!(!rd_config.allow_withdrawal());
        assert!(rd_config.adjust_advance_towards_future_payments());
    }
}
