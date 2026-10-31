//! Loan Products, Multi-Tranche Disbursement, Loan Rescheduling, and Guarantor Hold Lock Engine.
//!
//! Faithfully ported from Apache Fineract upstream Java classes:
//! - `org.apache.fineract.portfolio.loanproduct.domain.LoanProduct` (`m_product_loan`)
//! - `org.apache.fineract.portfolio.loanproduct.domain.LoanProductRelatedDetail`
//! - `org.apache.fineract.portfolio.loanproduct.domain.LoanProductMinMaxConstraints`
//! - `org.apache.fineract.portfolio.loanproduct.domain.LoanProductTrancheDetails`
//! - `org.apache.fineract.portfolio.loanproduct.domain.LoanProductGuaranteeDetails` (`m_product_loan_guarantee_details`)
//! - `org.apache.fineract.portfolio.loanproduct.serialization.LoanProductDataValidator`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanScheduleType`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanScheduleProcessingType`
//! - `org.apache.fineract.portfolio.loanaccount.domain.LoanDisbursementDetails` (`m_loan_disbursement_detail`)
//! - `org.apache.fineract.portfolio.loanaccount.service.LoanDisbursementService`
//! - `org.apache.fineract.portfolio.loanaccount.serialization.LoanDisbursementValidator`
//! - `org.apache.fineract.portfolio.loanaccount.rescheduleloan.domain.LoanRescheduleRequest` (`m_loan_reschedule_request`)
//! - `org.apache.fineract.portfolio.loanaccount.rescheduleloan.data.LoanRescheduleRequestDataValidatorImpl`
//! - `org.apache.fineract.portfolio.loanaccount.rescheduleloan.service.LoanRescheduleRequestWritePlatformServiceImpl`
//! - `org.apache.fineract.portfolio.loanaccount.guarantor.domain.GuarantorType`
//! - `org.apache.fineract.portfolio.loanaccount.guarantor.domain.GuarantorFundStatusType`
//! - `org.apache.fineract.portfolio.loanaccount.guarantor.domain.Guarantor` (`m_guarantor`)
//! - `org.apache.fineract.portfolio.loanaccount.guarantor.domain.GuarantorFundingDetails` (`m_guarantor_funding_details`)
//! - `org.apache.fineract.portfolio.loanaccount.guarantor.domain.GuarantorFundingTransaction` (`m_guarantor_funding_transaction`)
//! - `org.apache.fineract.portfolio.loanaccount.guarantor.service.GuarantorDomainServiceImpl`

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use chrono::{Days, Months, NaiveDate};
pub use fineract_accounting_rs::AccountingRuleType;
use fineract_charge_rs::EnumOptionData;
use fineract_loan_rs::schedule_and_terms::LoanRepaymentScheduleInstallment;
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use fineract_savings_rs::account_and_transactions::SavingsAccount;
use fineract_validation_rs::{
    ApiErrorArg, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException, PlatformDomainRuleException,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// =============================================================================
// 1. LOAN SCHEDULE TYPE & LOAN SCHEDULE PROCESSING TYPE
// =============================================================================

/// Canonical payment allocation strategy code required for progressive loan schedules.
pub const ADVANCED_PAYMENT_ALLOCATION_STRATEGY: &str = "advanced-payment-allocation-strategy";

/// Default cumulative payment allocation strategy code (`mifos-standard-strategy`).
pub const MIFOS_STANDARD_STRATEGY: &str = "mifos-standard-strategy";

/// Loan schedule generation and interest calculation model (`loan_schedule_type`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanScheduleType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoanScheduleType {
    /// Traditional cumulative schedule calculator (`CUMULATIVE`).
    #[default]
    #[serde(alias = "Cumulative", alias = "cumulative")]
    Cumulative,
    /// Progressive EMI / advanced payment allocation schedule calculator (`PROGRESSIVE`).
    #[serde(alias = "Progressive", alias = "progressive")]
    Progressive,
}

impl LoanScheduleType {
    pub const CUMULATIVE: Self = Self::Cumulative;
    pub const PROGRESSIVE: Self = Self::Progressive;

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cumulative => "CUMULATIVE",
            Self::Progressive => "PROGRESSIVE",
        }
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Cumulative => "LoanScheduleType.cumulative",
            Self::Progressive => "LoanScheduleType.progressive",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Cumulative => "Cumulative",
            Self::Progressive => "Progressive",
        }
    }

    #[inline]
    pub const fn is_cumulative(self) -> bool {
        matches!(self, Self::Cumulative)
    }

    #[inline]
    pub const fn is_progressive(self) -> bool {
        matches!(self, Self::Progressive)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        let id = match self {
            Self::Cumulative => 1,
            Self::Progressive => 2,
        };
        EnumOptionData::new(id, self.code(), self.description())
    }
}

impl fmt::Display for LoanScheduleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LoanScheduleType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "CUMULATIVE" | "LOANSCHEDULETYPE.CUMULATIVE" => Ok(Self::Cumulative),
            "PROGRESSIVE" | "LOANSCHEDULETYPE.PROGRESSIVE" => Ok(Self::Progressive),
            other => Err(format!("Invalid LoanScheduleType: `{other}`")),
        }
    }
}

/// Installment allocation order for loan schedule processing (`loan_schedule_processing_type`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanScheduleProcessingType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoanScheduleProcessingType {
    /// Settles each installment's configured allocation buckets completely before advancing to the next installment (`HORIZONTAL`).
    #[default]
    #[serde(alias = "Horizontal", alias = "horizontal")]
    Horizontal,
    /// Settles each bucket across all due installments before moving to the next allocation rule (`VERTICAL`).
    #[serde(alias = "Vertical", alias = "vertical")]
    Vertical,
}

impl LoanScheduleProcessingType {
    pub const HORIZONTAL: Self = Self::Horizontal;
    pub const VERTICAL: Self = Self::Vertical;

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "HORIZONTAL",
            Self::Vertical => "VERTICAL",
        }
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Horizontal => "LoanScheduleProcessingType.horizontal",
            Self::Vertical => "LoanScheduleProcessingType.vertical",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Horizontal => "Horizontal",
            Self::Vertical => "Vertical",
        }
    }

    #[inline]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Horizontal)
    }

    #[inline]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Vertical)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        let id = match self {
            Self::Horizontal => 1,
            Self::Vertical => 2,
        };
        EnumOptionData::new(id, self.code(), self.description())
    }
}

impl fmt::Display for LoanScheduleProcessingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LoanScheduleProcessingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "HORIZONTAL" | "LOANSCHEDULEPROCESSINGTYPE.HORIZONTAL" => Ok(Self::Horizontal),
            "VERTICAL" | "LOANSCHEDULEPROCESSINGTYPE.VERTICAL" => Ok(Self::Vertical),
            other => Err(format!("Invalid LoanScheduleProcessingType: `{other}`")),
        }
    }
}

// =============================================================================
// 2. LOAN PRODUCT ENUMERATIONS (FREQUENCY, AMORTIZATION, INTEREST)
// =============================================================================

/// Repayment and interest rate period frequency type (`repayment_period_frequency_enum`).
///
/// Ported from `org.apache.fineract.portfolio.common.domain.PeriodFrequencyType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum PeriodFrequencyType {
    Days = 0,
    Weeks = 1,
    #[default]
    Months = 2,
    Years = 3,
    WholeTerm = 4,
    Invalid = 5,
}

/// Alias for [`PeriodFrequencyType`] used in loan repayment configurations.
pub type RepaymentFrequencyType = PeriodFrequencyType;

impl PeriodFrequencyType {
    pub const DAYS: Self = Self::Days;
    pub const WEEKS: Self = Self::Weeks;
    pub const MONTHS: Self = Self::Months;
    pub const YEARS: Self = Self::Years;
    pub const WHOLE_TERM: Self = Self::WholeTerm;
    pub const INVALID: Self = Self::Invalid;

    /// Valid repayment frequency values (`0 = Days`, `1 = Weeks`, `2 = Months`).
    pub const VALID_REPAYMENT_VALUES: &'static [i64] = &[0, 1, 2];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Days,
            1 => Self::Weeks,
            2 => Self::Months,
            3 => Self::Years,
            4 => Self::WholeTerm,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_int(v: Option<i32>) -> Self {
        match v {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Days => "periodFrequencyType.days",
            Self::Weeks => "periodFrequencyType.weeks",
            Self::Months => "periodFrequencyType.months",
            Self::Years => "periodFrequencyType.years",
            Self::WholeTerm => "periodFrequencyType.whole_term",
            Self::Invalid => "periodFrequencyType.invalid",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Days => "Days",
            Self::Weeks => "Weeks",
            Self::Months => "Months",
            Self::Years => "Years",
            Self::WholeTerm => "Whole term",
            Self::Invalid => "Invalid",
        }
    }

    /// Advances `date` by `every * step_multiplier` periods of this frequency.
    pub fn add_periods(self, date: NaiveDate, every: u32, periods: u32) -> NaiveDate {
        let total = u64::from(every).saturating_mul(u64::from(periods));
        if total == 0 {
            return date;
        }
        match self {
            Self::Days => date
                .checked_add_days(Days::new(total))
                .unwrap_or(date),
            Self::Weeks => date
                .checked_add_days(Days::new(total.saturating_mul(7)))
                .unwrap_or(date),
            Self::Months => date
                .checked_add_months(Months::new(u32::try_from(total).unwrap_or(u32::MAX)))
                .unwrap_or(date),
            Self::Years => date
                .checked_add_months(Months::new(
                    u32::try_from(total.saturating_mul(12)).unwrap_or(u32::MAX),
                ))
                .unwrap_or(date),
            Self::WholeTerm | Self::Invalid => date
                .checked_add_months(Months::new(u32::try_from(total).unwrap_or(u32::MAX)))
                .unwrap_or(date),
        }
    }

    /// Returns the number of repayment periods per year for annual-to-periodic interest conversion.
    pub fn periods_per_year(self, repayment_every: u32) -> Decimal {
        let every = Decimal::from(repayment_every.max(1));
        match self {
            Self::Days => Decimal::from(365) / every,
            Self::Weeks => Decimal::from(52) / every,
            Self::Months | Self::WholeTerm | Self::Invalid => Decimal::from(12) / every,
            Self::Years => Decimal::ONE / every,
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

/// Loan principal and interest amortization method (`amortization_method_enum`).
///
/// Ported from `org.apache.fineract.portfolio.loanproduct.domain.AmortizationMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum AmortizationMethod {
    /// Equal principal payments across installments (`0`).
    EqualPrincipal = 0,
    /// Equal total installment (EMI) payments across installments (`1`).
    #[default]
    EqualInstallments = 1,
    /// Invalid (`2`).
    Invalid = 2,
}

impl AmortizationMethod {
    pub const EQUAL_PRINCIPAL: Self = Self::EqualPrincipal;
    pub const EQUAL_INSTALLMENTS: Self = Self::EqualInstallments;
    pub const INVALID: Self = Self::Invalid;
    pub const VALID_VALUES: &'static [i64] = &[0, 1];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::EqualPrincipal,
            1 => Self::EqualInstallments,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_int(v: Option<i32>) -> Self {
        match v {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_equal_principal(self) -> bool {
        matches!(self, Self::EqualPrincipal)
    }

    #[inline]
    pub const fn is_equal_installments(self) -> bool {
        matches!(self, Self::EqualInstallments)
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EqualPrincipal => "amortizationType.equal.principal",
            Self::EqualInstallments => "amortizationType.equal.installments",
            Self::Invalid => "amortizationType.invalid",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::EqualPrincipal => "Equal principal payments",
            Self::EqualInstallments => "Equal installments",
            Self::Invalid => "Invalid",
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

/// Loan interest calculation method (`interest_method_enum`).
///
/// Ported from `org.apache.fineract.portfolio.loanproduct.domain.InterestMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum InterestMethod {
    /// Declining balance interest calculation (`0`).
    #[default]
    DecliningBalance = 0,
    /// Flat interest calculation across original principal (`1`).
    Flat = 1,
    /// Invalid (`2`).
    Invalid = 2,
}

impl InterestMethod {
    pub const DECLINING_BALANCE: Self = Self::DecliningBalance;
    pub const FLAT: Self = Self::Flat;
    pub const INVALID: Self = Self::Invalid;
    pub const VALID_VALUES: &'static [i64] = &[0, 1];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::DecliningBalance,
            1 => Self::Flat,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_int(v: Option<i32>) -> Self {
        match v {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_declining_balance(self) -> bool {
        matches!(self, Self::DecliningBalance)
    }

    #[inline]
    pub const fn is_flat(self) -> bool {
        matches!(self, Self::Flat)
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DecliningBalance => "interestType.declining.balance",
            Self::Flat => "interestType.flat",
            Self::Invalid => "interestType.invalid",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::DecliningBalance => "Declining Balance",
            Self::Flat => "Flat",
            Self::Invalid => "Invalid",
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

/// Interest calculation period method (`interest_calculated_in_period_enum`).
///
/// Ported from `org.apache.fineract.portfolio.loanproduct.domain.InterestCalculationPeriodMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum InterestCalculationPeriodMethod {
    /// Calculate exact daily interest (`0`).
    Daily = 0,
    /// Calculate interest using the repayment period frequency (`1`).
    #[default]
    SameAsRepaymentPeriod = 1,
    /// Invalid (`2`).
    Invalid = 2,
}

impl InterestCalculationPeriodMethod {
    pub const DAILY: Self = Self::Daily;
    pub const SAME_AS_REPAYMENT_PERIOD: Self = Self::SameAsRepaymentPeriod;
    pub const INVALID: Self = Self::Invalid;
    pub const VALID_VALUES: &'static [i64] = &[0, 1];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Daily,
            1 => Self::SameAsRepaymentPeriod,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_int(v: Option<i32>) -> Self {
        match v {
            Some(code) => Self::from_i32(code),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_daily(self) -> bool {
        matches!(self, Self::Daily)
    }

    #[inline]
    pub const fn is_same_as_repayment_period(self) -> bool {
        matches!(self, Self::SameAsRepaymentPeriod)
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Daily => "interestCalculationPeriodType.daily",
            Self::SameAsRepaymentPeriod => "interestCalculationPeriodType.same.as.repayment.period",
            Self::Invalid => "interestCalculationPeriodType.invalid",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::SameAsRepaymentPeriod => "Same as repayment period",
            Self::Invalid => "Invalid",
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

// =============================================================================
// 3. LOAN PRODUCT (`m_product_loan`)
// =============================================================================

/// Loan Product domain model (`m_product_loan`).
///
/// Ported from `org.apache.fineract.portfolio.loanproduct.domain.LoanProduct`,
/// `LoanProductRelatedDetail`, `LoanProductMinMaxConstraints`, `LoanProductTrancheDetails`,
/// and validated via `LoanProductDataValidator`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanProduct {
    pub id: i64,
    pub short_name: String,
    pub name: String,
    pub currency_code: String,
    pub digits_after_decimal: i32,
    pub min_principal: Decimal,
    pub default_principal: Decimal,
    pub max_principal: Decimal,
    pub min_nominal_interest_rate: Decimal,
    pub default_nominal_interest_rate: Decimal,
    pub max_nominal_interest_rate: Decimal,
    pub min_number_of_repayments: u32,
    pub default_number_of_repayments: u32,
    pub max_number_of_repayments: u32,
    pub repayment_every: u32,
    pub repayment_frequency_type: PeriodFrequencyType,
    pub amortization_method: AmortizationMethod,
    pub interest_method: InterestMethod,
    pub interest_calculation_period_method: InterestCalculationPeriodMethod,
    pub loan_schedule_type: LoanScheduleType,
    pub loan_schedule_processing_type: LoanScheduleProcessingType,
    pub transaction_processing_strategy_code: String,
    pub accounting_rule: AccountingRuleType,
    pub multi_disburse_loan: bool,
    pub max_tranche_count: Option<u32>,
    pub outstanding_loan_balance_limit: Option<Decimal>,
    pub enable_down_payment: bool,
    pub disbursed_amount_percentage_for_down_payment: Option<Decimal>,
    pub enable_auto_repayment_for_down_payment: bool,
    pub delinquency_bucket_id: Option<i64>,
}

impl Default for LoanProduct {
    fn default() -> Self {
        Self {
            id: 0,
            short_name: String::from("LP01"),
            name: String::from("Default Loan Product"),
            currency_code: String::from("USD"),
            digits_after_decimal: 2,
            min_principal: Decimal::from(1_000),
            default_principal: Decimal::from(10_000),
            max_principal: Decimal::from(100_000),
            min_nominal_interest_rate: Decimal::ONE,
            default_nominal_interest_rate: Decimal::from(12),
            max_nominal_interest_rate: Decimal::from(36),
            min_number_of_repayments: 1,
            default_number_of_repayments: 12,
            max_number_of_repayments: 60,
            repayment_every: 1,
            repayment_frequency_type: PeriodFrequencyType::Months,
            amortization_method: AmortizationMethod::EqualInstallments,
            interest_method: InterestMethod::DecliningBalance,
            interest_calculation_period_method:
                InterestCalculationPeriodMethod::SameAsRepaymentPeriod,
            loan_schedule_type: LoanScheduleType::Cumulative,
            loan_schedule_processing_type: LoanScheduleProcessingType::Horizontal,
            transaction_processing_strategy_code: String::from(MIFOS_STANDARD_STRATEGY),
            accounting_rule: AccountingRuleType::None,
            multi_disburse_loan: false,
            max_tranche_count: None,
            outstanding_loan_balance_limit: None,
            enable_down_payment: false,
            disbursed_amount_percentage_for_down_payment: None,
            enable_auto_repayment_for_down_payment: false,
            delinquency_bucket_id: None,
        }
    }
}

impl LoanProduct {
    /// Creates a new cumulative [`LoanProduct`] with standard defaults.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        short_name: impl Into<String>,
        name: impl Into<String>,
        currency_code: impl Into<String>,
        digits_after_decimal: i32,
        default_principal: Decimal,
        default_nominal_interest_rate: Decimal,
        default_number_of_repayments: u32,
    ) -> Self {
        Self {
            id,
            short_name: short_name.into(),
            name: name.into(),
            currency_code: currency_code.into(),
            digits_after_decimal,
            min_principal: default_principal,
            default_principal,
            max_principal: default_principal,
            min_nominal_interest_rate: default_nominal_interest_rate,
            default_nominal_interest_rate,
            max_nominal_interest_rate: default_nominal_interest_rate,
            min_number_of_repayments: default_number_of_repayments,
            default_number_of_repayments,
            max_number_of_repayments: default_number_of_repayments,
            ..Self::default()
        }
    }

    /// Configures a progressive loan product with the mandatory `advanced-payment-allocation-strategy`.
    pub fn new_progressive(
        id: i64,
        short_name: impl Into<String>,
        name: impl Into<String>,
        currency_code: impl Into<String>,
        digits_after_decimal: i32,
        default_principal: Decimal,
        default_nominal_interest_rate: Decimal,
        default_number_of_repayments: u32,
        processing_type: LoanScheduleProcessingType,
    ) -> Self {
        let mut product = Self::new(
            id,
            short_name,
            name,
            currency_code,
            digits_after_decimal,
            default_principal,
            default_nominal_interest_rate,
            default_number_of_repayments,
        );
        product.loan_schedule_type = LoanScheduleType::Progressive;
        product.loan_schedule_processing_type = processing_type;
        product.transaction_processing_strategy_code =
            String::from(ADVANCED_PAYMENT_ALLOCATION_STRATEGY);
        product
    }

    /// Returns the product's [`MonetaryCurrency`].
    #[inline]
    pub fn currency(&self) -> MonetaryCurrency {
        MonetaryCurrency::new(&self.currency_code, self.digits_after_decimal.max(0) as u32, None)
    }

    /// Sets `[min_principal, default_principal, max_principal]`.
    pub const fn with_principal_bounds(
        mut self,
        min_principal: Decimal,
        default_principal: Decimal,
        max_principal: Decimal,
    ) -> Self {
        self.min_principal = min_principal;
        self.default_principal = default_principal;
        self.max_principal = max_principal;
        self
    }

    /// Sets `[min_nominal_interest_rate, default_nominal_interest_rate, max_nominal_interest_rate]`.
    pub const fn with_interest_rate_bounds(
        mut self,
        min_rate: Decimal,
        default_rate: Decimal,
        max_rate: Decimal,
    ) -> Self {
        self.min_nominal_interest_rate = min_rate;
        self.default_nominal_interest_rate = default_rate;
        self.max_nominal_interest_rate = max_rate;
        self
    }

    /// Sets `[min_number_of_repayments, default_number_of_repayments, max_number_of_repayments]`.
    pub const fn with_repayment_count_bounds(
        mut self,
        min_repayments: u32,
        default_repayments: u32,
        max_repayments: u32,
    ) -> Self {
        self.min_number_of_repayments = min_repayments;
        self.default_number_of_repayments = default_repayments;
        self.max_number_of_repayments = max_repayments;
        self
    }

    /// Enables multi-tranche disbursement with `max_tranche_count` and optional `outstanding_loan_balance_limit`.
    pub const fn with_multi_disburse(
        mut self,
        max_tranche_count: Option<u32>,
        outstanding_loan_balance_limit: Option<Decimal>,
    ) -> Self {
        self.multi_disburse_loan = true;
        self.max_tranche_count = max_tranche_count;
        self.outstanding_loan_balance_limit = outstanding_loan_balance_limit;
        self
    }

    /// Enables down payment with percentage and optional automatic down-payment transaction creation.
    pub const fn with_down_payment(
        mut self,
        percentage: Decimal,
        auto_repayment: bool,
    ) -> Self {
        self.enable_down_payment = true;
        self.disbursed_amount_percentage_for_down_payment = Some(percentage);
        self.enable_auto_repayment_for_down_payment = auto_repayment;
        self
    }

    /// Validates all [`LoanProduct`] constraints matching `LoanProductDataValidator` and `LoanProduct.validateLoanProductPreSave`:
    /// - Identity & currency bounds (`name`, `shortName`, `currencyCode`, `digitsAfterDecimal` in `0..=6`)
    /// - `0 < min_principal <= default_principal <= max_principal`
    /// - `0 <= min_nominal_interest_rate <= default_nominal_interest_rate <= max_nominal_interest_rate`
    /// - `0 < min_number_of_repayments <= default_number_of_repayments <= max_number_of_repayments`
    /// - `repayment_every > 0` and valid `repayment_frequency_type`, `amortization_method`, `interest_method`, `interest_calculation_period_method`, `accounting_rule`
    /// - Progressive loan schedule rules:
    ///   - `LoanScheduleType::Progressive` requires `transaction_processing_strategy_code == "advanced-payment-allocation-strategy"`
    ///   - `transaction_processing_strategy_code == "advanced-payment-allocation-strategy"` requires `LoanScheduleType::Progressive`
    ///   - `LoanScheduleProcessingType::Vertical` is supported only when `loan_schedule_type == LoanScheduleType::Progressive` and strategy is `"advanced-payment-allocation-strategy"`
    /// - Multi-tranche rules:
    ///   - When `multi_disburse_loan == true`, `max_tranche_count` must be `Some(c)` with `c > 0`, and `outstanding_loan_balance_limit` (if `Some`) must be `> 0`
    ///   - When `multi_disburse_loan == false`, `max_tranche_count` and `outstanding_loan_balance_limit` must be `None`
    /// - Down-payment rules:
    ///   - When `enable_down_payment == true`, `disbursed_amount_percentage_for_down_payment` is mandatory, must be in `1..=100`, and have scale `<= 6`
    ///   - When `enable_down_payment == false`, `disbursed_amount_percentage_for_down_payment` must be `None` and `enable_auto_repayment_for_down_payment` must be `false`
    pub fn validate(&self) -> Result<(), PlatformApiDataValidationException> {
        let b = DataValidatorBuilder::for_resource("loanproduct");

        // 1. Identity & Currency
        let b = b
            .reset()
            .parameter("name")
            .value(self.name.as_str())
            .not_blank()
            .not_exceeding_length_of(100);

        let b = b
            .reset()
            .parameter("shortName")
            .value(self.short_name.as_str())
            .not_blank()
            .not_exceeding_length_of(4);

        let b = b
            .reset()
            .parameter("currencyCode")
            .value(self.currency_code.as_str())
            .not_blank()
            .not_exceeding_length_of(3);

        let b = b
            .reset()
            .parameter("digitsAfterDecimal")
            .value(i64::from(self.digits_after_decimal))
            .in_min_max_range(0, 6);

        // 2. Principal min <= default <= max
        let b = b
            .reset()
            .parameter("minPrincipal")
            .value(self.min_principal)
            .positive_amount();

        let b = b
            .reset()
            .parameter("principal")
            .value(self.default_principal)
            .positive_amount();

        let b = b
            .reset()
            .parameter("maxPrincipal")
            .value(self.max_principal)
            .positive_amount();

        let b = if self.min_principal > self.max_principal {
            b.reset()
                .parameter("minPrincipal")
                .value(self.min_principal)
                .fail_with_code(
                    "must.be.less.than.or.equal.to.maxPrincipal",
                    "minPrincipal must be less than or equal to maxPrincipal.",
                )
        } else {
            b
        };

        let b = if self.default_principal < self.min_principal
            || self.default_principal > self.max_principal
        {
            b.reset()
                .parameter("principal")
                .value(self.default_principal)
                .in_min_and_max_amount_range(self.min_principal, self.max_principal)
        } else {
            b
        };

        // 3. Nominal interest rate min <= default <= max
        let b = b
            .reset()
            .parameter("minInterestRatePerPeriod")
            .value(self.min_nominal_interest_rate)
            .zero_or_positive_amount();

        let b = b
            .reset()
            .parameter("interestRatePerPeriod")
            .value(self.default_nominal_interest_rate)
            .zero_or_positive_amount();

        let b = b
            .reset()
            .parameter("maxInterestRatePerPeriod")
            .value(self.max_nominal_interest_rate)
            .zero_or_positive_amount();

        let b = if self.min_nominal_interest_rate > self.max_nominal_interest_rate {
            b.reset()
                .parameter("minInterestRatePerPeriod")
                .value(self.min_nominal_interest_rate)
                .fail_with_code(
                    "must.be.less.than.or.equal.to.maxInterestRatePerPeriod",
                    "minInterestRatePerPeriod must be less than or equal to maxInterestRatePerPeriod.",
                )
        } else {
            b
        };

        let b = if self.default_nominal_interest_rate < self.min_nominal_interest_rate
            || self.default_nominal_interest_rate > self.max_nominal_interest_rate
        {
            b.reset()
                .parameter("interestRatePerPeriod")
                .value(self.default_nominal_interest_rate)
                .in_min_and_max_amount_range(
                    self.min_nominal_interest_rate,
                    self.max_nominal_interest_rate,
                )
        } else {
            b
        };

        // 4. Number of repayments min <= default <= max
        let b = b
            .reset()
            .parameter("minNumberOfRepayments")
            .value(i64::from(self.min_number_of_repayments))
            .integer_greater_than_zero();

        let b = b
            .reset()
            .parameter("numberOfRepayments")
            .value(i64::from(self.default_number_of_repayments))
            .integer_greater_than_zero();

        let b = b
            .reset()
            .parameter("maxNumberOfRepayments")
            .value(i64::from(self.max_number_of_repayments))
            .integer_greater_than_zero();

        let b = if self.min_number_of_repayments > self.max_number_of_repayments {
            b.reset()
                .parameter("minNumberOfRepayments")
                .value(i64::from(self.min_number_of_repayments))
                .fail_with_code(
                    "must.be.less.than.or.equal.to.maxNumberOfRepayments",
                    "minNumberOfRepayments must be less than or equal to maxNumberOfRepayments.",
                )
        } else {
            b
        };

        let b = if self.default_number_of_repayments < self.min_number_of_repayments
            || self.default_number_of_repayments > self.max_number_of_repayments
        {
            b.reset()
                .parameter("numberOfRepayments")
                .value(i64::from(self.default_number_of_repayments))
                .in_min_max_range(
                    i64::from(self.min_number_of_repayments),
                    i64::from(self.max_number_of_repayments),
                )
        } else {
            b
        };

        // 5. Repayment frequency & methods
        let b = b
            .reset()
            .parameter("repaymentEvery")
            .value(i64::from(self.repayment_every))
            .integer_greater_than_zero();

        let b = b
            .reset()
            .parameter("repaymentFrequencyType")
            .value(i64::from(self.repayment_frequency_type.value()))
            .is_one_of_these_values(PeriodFrequencyType::VALID_REPAYMENT_VALUES.iter().copied());

        let b = b
            .reset()
            .parameter("amortizationType")
            .value(i64::from(self.amortization_method.value()))
            .is_one_of_these_values(AmortizationMethod::VALID_VALUES.iter().copied());

        let b = b
            .reset()
            .parameter("interestType")
            .value(i64::from(self.interest_method.value()))
            .is_one_of_these_values(InterestMethod::VALID_VALUES.iter().copied());

        let b = b
            .reset()
            .parameter("interestCalculationPeriodType")
            .value(i64::from(self.interest_calculation_period_method.value()))
            .is_one_of_these_values(
                InterestCalculationPeriodMethod::VALID_VALUES
                    .iter()
                    .copied(),
            );

        let b = b
            .reset()
            .parameter("accountingRule")
            .value(i64::from(self.accounting_rule.as_i16()))
            .in_min_max_range(1, 4);

        // 6. Transaction processing strategy & Progressive loan schedule constraints
        let b = b
            .reset()
            .parameter("transactionProcessingStrategyCode")
            .value(self.transaction_processing_strategy_code.as_str())
            .not_blank();

        let is_advanced_strategy = self
            .transaction_processing_strategy_code
            .eq_ignore_ascii_case(ADVANCED_PAYMENT_ALLOCATION_STRATEGY);

        // LoanProductDataValidator: PROGRESSIVE requires advanced-payment-allocation-strategy
        let b = if self.loan_schedule_type.is_progressive() && !is_advanced_strategy {
            b.reset()
                .parameter("transactionProcessingStrategyCode")
                .value(self.transaction_processing_strategy_code.as_str())
                .fail_with_code_no_parameter_added_to_error_code(
                    "loanScheduleType.supported.only.with.advanced.payment.allocation.strategy",
                )
        } else {
            b
        };

        // LoanProductDataValidator: advanced-payment-allocation-strategy requires PROGRESSIVE schedule type
        let b = if is_advanced_strategy && !self.loan_schedule_type.is_progressive() {
            b.reset()
                .parameter("loanScheduleProcessingType")
                .value(self.loan_schedule_processing_type.as_str())
                .fail_with_code_no_parameter_added_to_error_code(
                    "loanScheduleProcessingType.supported.only.for.progressive.loan.schedule.type",
                )
        } else {
            b
        };

        // LoanProductDataValidator: VERTICAL schedule processing requires PROGRESSIVE + advanced-payment-allocation-strategy
        let b = if self.loan_schedule_processing_type.is_vertical()
            && (!self.loan_schedule_type.is_progressive() || !is_advanced_strategy)
        {
            b.reset()
                .parameter("loanScheduleProcessingType")
                .value(self.loan_schedule_processing_type.as_str())
                .fail_with_code_no_parameter_added_to_error_code(
                    "loanScheduleProcessingType.supported.only.for.progressive.loan.schedule.handling",
                )
        } else {
            b
        };

        // Progressive loan products require DecliningBalance + EqualInstallments in Fineract
        let b = if self.loan_schedule_type.is_progressive() && self.interest_method.is_flat() {
            b.reset()
                .parameter("interestType")
                .value(i64::from(self.interest_method.value()))
                .fail_with_code(
                    "progressive.not.supported.for.flat.interest",
                    "Progressive loan schedule type is only supported with Declining Balance interest.",
                )
        } else {
            b
        };

        // 7. Multi-disbursement tranche rules (`LoanProductDataValidator`)
        let b = if self.multi_disburse_loan {
            let b = match self.max_tranche_count {
                Some(count) => b
                    .reset()
                    .parameter("maxTrancheCount")
                    .value(i64::from(count))
                    .integer_greater_than_zero(),
                None => b
                    .reset()
                    .parameter("maxTrancheCount")
                    .value(Option::<i64>::None)
                    .not_null(),
            };
            if let Some(limit) = self.outstanding_loan_balance_limit {
                b.reset()
                    .parameter("outstandingLoanBalance")
                    .value(limit)
                    .positive_amount()
            } else {
                b
            }
        } else {
            let b = if self.max_tranche_count.is_some() {
                b.reset()
                    .parameter("maxTrancheCount")
                    .value(self.max_tranche_count.map(i64::from))
                    .fail_with_code(
                        "supported.only.for.multidisburse.loan",
                        "maxTrancheCount is supported only when multiDisburseLoan is enabled.",
                    )
            } else {
                b
            };
            if self.outstanding_loan_balance_limit.is_some() {
                b.reset()
                    .parameter("outstandingLoanBalance")
                    .value(self.outstanding_loan_balance_limit)
                    .fail_with_code(
                        "supported.only.for.multidisburse.loan",
                        "outstandingLoanBalance is supported only when multiDisburseLoan is enabled.",
                    )
            } else {
                b
            }
        };

        // 8. Down payment validation (`LoanProductDataValidator.validateDownPaymentPercentage`)
        let b = if self.enable_down_payment {
            match self.disbursed_amount_percentage_for_down_payment {
                None => b
                    .reset()
                    .parameter("disbursedAmountPercentageForDownPayment")
                    .value(Option::<Decimal>::None)
                    .fail_with_code(
                        "required.for.down.payment.enabled.loan",
                        "disbursedAmountPercentageForDownPayment is required when enableDownPayment is true.",
                    ),
                Some(pct) => {
                    let b = b
                        .reset()
                        .parameter("disbursedAmountPercentageForDownPayment")
                        .value(pct)
                        .in_min_and_max_amount_range(Decimal::ONE, Decimal::from(100));
                    if pct.scale() > 6 {
                        b.reset()
                            .parameter("disbursedAmountPercentageForDownPayment")
                            .value(pct)
                            .scale_not_greater_than(6)
                    } else {
                        b
                    }
                }
            }
        } else {
            let b = if let Some(pct) = self.disbursed_amount_percentage_for_down_payment {
                b.reset()
                    .parameter("disbursedAmountPercentageForDownPayment")
                    .value(pct)
                    .fail_with_code(
                        "supported.only.for.down.payment.enabled.loan",
                        "disbursedAmountPercentageForDownPayment is supported only when enableDownPayment is true.",
                    )
            } else {
                b
            };
            if self.enable_auto_repayment_for_down_payment {
                b.reset()
                    .parameter("enableAutoRepaymentForDownPayment")
                    .value(self.enable_auto_repayment_for_down_payment)
                    .fail_with_code(
                        "supported.only.for.down.payment.enabled.loan",
                        "enableAutoRepaymentForDownPayment is supported only when enableDownPayment is true.",
                    )
            } else {
                b
            }
        };

        b.finish()
    }

    /// Validates that a loan application's proposed `principal`, `nominal_interest_rate`, and
    /// `number_of_repayments` fall within this product's `[min, max]` bounds (`LoanApplicationValidator`).
    pub fn validate_loan_application_terms(
        &self,
        principal: Decimal,
        nominal_interest_rate: Decimal,
        number_of_repayments: u32,
    ) -> Result<(), PlatformApiDataValidationException> {
        let b = DataValidatorBuilder::for_resource("loan");

        let b = b
            .reset()
            .parameter("principal")
            .value(principal)
            .in_min_and_max_amount_range(self.min_principal, self.max_principal);

        let b = b
            .reset()
            .parameter("interestRatePerPeriod")
            .value(nominal_interest_rate)
            .in_min_and_max_amount_range(
                self.min_nominal_interest_rate,
                self.max_nominal_interest_rate,
            );

        let b = b
            .reset()
            .parameter("numberOfRepayments")
            .value(i64::from(number_of_repayments))
            .in_min_max_range(
                i64::from(self.min_number_of_repayments),
                i64::from(self.max_number_of_repayments),
            );

        b.finish()
    }

    /// Calculates the down-payment portion for a disbursed amount when `enable_down_payment` is active.
    pub fn calculate_down_payment_amount(&self, disbursed_amount: Decimal) -> Decimal {
        if !self.enable_down_payment {
            return Decimal::ZERO;
        }
        let Some(pct) = self.disbursed_amount_percentage_for_down_payment else {
            return Decimal::ZERO;
        };
        let raw = (disbursed_amount * pct) / Decimal::from(100);
        Money::of(self.currency(), raw, FineractRoundingMode::HalfEven).amount()
    }
}

// =============================================================================
// 4. MULTI-TRANCHE DISBURSEMENT (`m_loan_disbursement_detail`)
// =============================================================================

/// Single scheduled or actual disbursement tranche (`m_loan_disbursement_detail`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanDisbursementDetails`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanDisbursementDetails {
    pub id: i64,
    pub loan_id: i64,
    pub expected_disbursement_date: NaiveDate,
    pub actual_disbursement_date: Option<NaiveDate>,
    pub principal: Decimal,
    pub net_disbursal_amount: Option<Decimal>,
}

impl LoanDisbursementDetails {
    /// Creates a scheduled (undisbursed) tranche.
    pub fn new(
        id: i64,
        loan_id: i64,
        expected_disbursement_date: NaiveDate,
        principal: Decimal,
    ) -> Self {
        Self {
            id,
            loan_id,
            expected_disbursement_date,
            actual_disbursement_date: None,
            principal,
            net_disbursal_amount: None,
        }
    }

    /// Creates an already-disbursed tranche with `actual_disbursement_date` and `net_disbursal_amount`.
    pub fn new_disbursed(
        id: i64,
        loan_id: i64,
        expected_disbursement_date: NaiveDate,
        actual_disbursement_date: NaiveDate,
        principal: Decimal,
        net_disbursal_amount: Option<Decimal>,
    ) -> Self {
        Self {
            id,
            loan_id,
            expected_disbursement_date,
            actual_disbursement_date: Some(actual_disbursement_date),
            principal,
            net_disbursal_amount: Some(net_disbursal_amount.unwrap_or(principal)),
        }
    }

    /// Returns `true` if this tranche has been disbursed (`actual_disbursement_date.is_some()`).
    #[inline]
    pub const fn is_disbursed(&self) -> bool {
        self.actual_disbursement_date.is_some()
    }

    /// Returns `true` if this tranche is still pending disbursement.
    #[inline]
    pub const fn is_pending(&self) -> bool {
        self.actual_disbursement_date.is_none()
    }

    /// Returns the effective disbursement date (`actual_disbursement_date` if disbursed, otherwise `expected_disbursement_date`).
    #[inline]
    pub fn effective_disbursement_date(&self) -> NaiveDate {
        self.actual_disbursement_date
            .unwrap_or(self.expected_disbursement_date)
    }

    /// Marks this tranche as disbursed on `actual_date`.
    pub fn mark_disbursed(
        &mut self,
        actual_date: NaiveDate,
        net_disbursal_amount: Option<Decimal>,
    ) {
        self.actual_disbursement_date = Some(actual_date);
        self.net_disbursal_amount = Some(net_disbursal_amount.unwrap_or(self.principal));
    }
}

/// Summary of a multi-tranche disbursement validation or execution step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiTrancheSummary {
    pub tranche_count: usize,
    pub max_tranche_count: Option<u32>,
    pub sanctioned_principal: Decimal,
    pub total_tranche_principal: Decimal,
    pub total_disbursed_principal: Decimal,
    pub total_pending_principal: Decimal,
    pub unallocated_sanctioned_principal: Decimal,
    pub down_payment_amount: Decimal,
}

/// Stateless engine validating and executing multi-tranche loan disbursements.
///
/// Ported from `LoanDisbursementService.java` and `LoanDisbursementValidator.java`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MultiTrancheDisbursementEngine;

impl MultiTrancheDisbursementEngine {
    /// Validates a slice of [`LoanDisbursementDetails`] against a [`LoanProduct`] and `sanctioned_principal`:
    /// 1. Product must have `multi_disburse_loan == true` if `tranches.len() > 1` (or when validating multi-tranche data).
    /// 2. `tranches.len() <= max_tranche_count` (`validation.msg.loan.disbursementData.exceeding.max.tranche.count`).
    /// 3. Every tranche must have `principal > 0`.
    /// 4. Total tranche `principal` must not exceed `sanctioned_principal`
    ///    (`error.msg.loan.disbursal.disburse.amount.must.be.less.than.approved.principal`).
    /// 5. If the product defines `outstanding_loan_balance_limit`, total tranche principal (or resulting balance)
    ///    must not exceed `outstanding_loan_balance_limit`.
    pub fn validate_tranches(
        product: &LoanProduct,
        sanctioned_principal: Decimal,
        tranches: &[LoanDisbursementDetails],
    ) -> FineractResult<MultiTrancheSummary> {
        Self::validate_tranches_with_outstanding(
            product,
            sanctioned_principal,
            Decimal::ZERO,
            tranches,
        )
    }

    /// Validates tranches while also accounting for `current_principal_repaid` when checking
    /// `outstanding_loan_balance_limit`.
    pub fn validate_tranches_with_outstanding(
        product: &LoanProduct,
        sanctioned_principal: Decimal,
        current_principal_repaid: Decimal,
        tranches: &[LoanDisbursementDetails],
    ) -> FineractResult<MultiTrancheSummary> {
        let mut b = DataValidatorBuilder::for_resource("loan");

        if !product.multi_disburse_loan && !tranches.is_empty() {
            b = b
                .reset()
                .parameter("disbursementData")
                .fail_with_code(
                    "not.supported.for.single.disburse.loan",
                    "Multi-tranche disbursement data is only allowed for multi-disburse loan products.",
                );
            return Err(FineractError::DataValidation(b.finish().unwrap_err()));
        }

        if let Some(max_tranches) = product.max_tranche_count {
            if tranches.len() > usize::try_from(max_tranches).unwrap_or(usize::MAX) {
                b = b
                    .reset()
                    .parameter("disbursementData")
                    .value(i64::try_from(tranches.len()).unwrap_or(i64::MAX))
                    .fail_with_code(
                        "exceeding.max.tranche.count",
                        &format!(
                            "Number of disbursement tranches ({}) exceeds product maxTrancheCount ({max_tranches}).",
                            tranches.len()
                        ),
                    );
                return Err(FineractError::DataValidation(b.finish().unwrap_err()));
            }
        }

        let mut total_tranche_principal = Decimal::ZERO;
        let mut total_disbursed_principal = Decimal::ZERO;
        let mut total_pending_principal = Decimal::ZERO;

        for (idx, tranche) in tranches.iter().enumerate() {
            if tranche.principal <= Decimal::ZERO {
                b = b
                    .reset()
                    .parameter("principal")
                    .value(tranche.principal)
                    .positive_amount();
                return Err(FineractError::DataValidation(b.finish().unwrap_err()));
            }
            if idx > 0
                && tranche.expected_disbursement_date < tranches[idx - 1].expected_disbursement_date
            {
                b = b
                    .reset()
                    .parameter("expectedDisbursementDate")
                    .value(tranche.expected_disbursement_date.to_string())
                    .fail_with_code(
                        "cannot.be.before.previous.tranche.date",
                        "Tranche expected disbursement date cannot be before the previous tranche expected disbursement date.",
                    );
                return Err(FineractError::DataValidation(b.finish().unwrap_err()));
            }

            total_tranche_principal += tranche.principal;
            if tranche.is_disbursed() {
                total_disbursed_principal += tranche.principal;
            } else {
                total_pending_principal += tranche.principal;
            }
        }

        // Total tranche principal <= sanctioned_principal
        if total_tranche_principal > sanctioned_principal {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.loan.disbursal.disburse.amount.must.be.less.than.approved.principal",
                    format!(
                        "Sum of tranche principals ({total_tranche_principal}) cannot exceed sanctioned principal ({sanctioned_principal})."
                    ),
                    vec![
                        ApiErrorArg::from(total_tranche_principal),
                        ApiErrorArg::from(sanctioned_principal),
                    ],
                ),
            ));
        }

        // Check product outstanding loan balance limit if configured
        if let Some(balance_limit) = product.outstanding_loan_balance_limit {
            let projected_outstanding =
                (total_disbursed_principal - current_principal_repaid).max(Decimal::ZERO);
            if projected_outstanding > balance_limit || total_tranche_principal > balance_limit {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.loan.disbursal.amount.exceeds.max.outstanding.balance",
                        format!(
                            "Tranche principal ({total_tranche_principal}) exceeds product outstandingLoanBalanceLimit ({balance_limit})."
                        ),
                        vec![
                            ApiErrorArg::from(total_tranche_principal),
                            ApiErrorArg::from(balance_limit),
                        ],
                    ),
                ));
            }
        }

        let down_payment_amount = product.calculate_down_payment_amount(total_disbursed_principal);
        Ok(MultiTrancheSummary {
            tranche_count: tranches.len(),
            max_tranche_count: product.max_tranche_count,
            sanctioned_principal,
            total_tranche_principal,
            total_disbursed_principal,
            total_pending_principal,
            unallocated_sanctioned_principal: (sanctioned_principal - total_tranche_principal)
                .max(Decimal::ZERO),
            down_payment_amount,
        })
    }

    /// Adds a new scheduled tranche to `tranches` and validates all multi-tranche constraints.
    pub fn add_tranche(
        product: &LoanProduct,
        sanctioned_principal: Decimal,
        tranches: &mut Vec<LoanDisbursementDetails>,
        new_tranche: LoanDisbursementDetails,
    ) -> FineractResult<MultiTrancheSummary> {
        let mut candidate = tranches.clone();
        candidate.push(new_tranche);
        let summary = Self::validate_tranches(product, sanctioned_principal, &candidate)?;
        *tranches = candidate;
        Ok(summary)
    }

    /// Disburses the tranche identified by `tranche_id` on `actual_disbursement_date` (or the next
    /// pending tranche if `tranche_id` is `None`), returning the updated [`MultiTrancheSummary`]
    /// and the disbursed [`LoanDisbursementDetails`].
    pub fn disburse_tranche(
        product: &LoanProduct,
        sanctioned_principal: Decimal,
        current_principal_repaid: Decimal,
        tranches: &mut [LoanDisbursementDetails],
        tranche_id: Option<i64>,
        actual_disbursement_date: NaiveDate,
        net_disbursal_amount: Option<Decimal>,
    ) -> FineractResult<(LoanDisbursementDetails, MultiTrancheSummary)> {
        let pos = match tranche_id {
            Some(id) => tranches.iter().position(|t| t.id == id),
            None => tranches.iter().position(|t| t.is_pending()),
        };

        let Some(idx) = pos else {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.loan.tranche.not.found.for.disbursal",
                "No pending disbursement tranche found to disburse.",
            )));
        };

        if tranches[idx].is_disbursed() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.loan.tranche.already.disbursed",
                "Selected disbursement tranche has already been disbursed.",
            )));
        }

        let mut candidate = tranches.to_vec();
        candidate[idx].mark_disbursed(actual_disbursement_date, net_disbursal_amount);
        let summary = Self::validate_tranches_with_outstanding(
            product,
            sanctioned_principal,
            current_principal_repaid,
            &candidate,
        )?;
        tranches[idx] = candidate[idx].clone();
        Ok((tranches[idx].clone(), summary))
    }
}

// =============================================================================
// 5. LOAN RESCHEDULING (`m_loan_reschedule_request` & `LoanRescheduleEngine`)
// =============================================================================

/// Lifecycle status of a [`LoanRescheduleRequest`] (`status_enum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum LoanRescheduleRequestStatusEnum {
    Invalid = 0,
    #[default]
    SubmittedAndPendingApproval = 100,
    Approved = 200,
    Rejected = 300,
}

impl LoanRescheduleRequestStatusEnum {
    pub const INVALID: Self = Self::Invalid;
    pub const SUBMITTED_AND_PENDING_APPROVAL: Self = Self::SubmittedAndPendingApproval;
    pub const APPROVED: Self = Self::Approved;
    pub const REJECTED: Self = Self::Rejected;

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            100 => Self::SubmittedAndPendingApproval,
            200 => Self::Approved,
            300 => Self::Rejected,
            _ => Self::Invalid,
        }
    }
}

/// Loan Reschedule Request entity (`m_loan_reschedule_request`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.rescheduleloan.domain.LoanRescheduleRequest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanRescheduleRequest {
    pub id: i64,
    pub loan_id: i64,
    pub status_enum: i32,
    pub reschedule_from_installment: u32,
    pub reschedule_from_date: NaiveDate,
    pub adjusted_due_date: Option<NaiveDate>,
    pub extra_terms: Option<u32>,
    pub grace_on_principal: Option<u32>,
    pub grace_on_interest: Option<u32>,
    pub new_interest_rate: Option<Decimal>,
}

impl LoanRescheduleRequest {
    /// Creates a new [`LoanRescheduleRequest`] in `SubmittedAndPendingApproval` (`100`) state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        loan_id: i64,
        reschedule_from_installment: u32,
        reschedule_from_date: NaiveDate,
        adjusted_due_date: Option<NaiveDate>,
        extra_terms: Option<u32>,
        grace_on_principal: Option<u32>,
        grace_on_interest: Option<u32>,
        new_interest_rate: Option<Decimal>,
    ) -> Self {
        Self {
            id,
            loan_id,
            status_enum: LoanRescheduleRequestStatusEnum::SubmittedAndPendingApproval.as_i32(),
            reschedule_from_installment,
            reschedule_from_date,
            adjusted_due_date,
            extra_terms,
            grace_on_principal,
            grace_on_interest,
            new_interest_rate,
        }
    }

    #[inline]
    pub const fn status(&self) -> LoanRescheduleRequestStatusEnum {
        LoanRescheduleRequestStatusEnum::from_i32(self.status_enum)
    }

    #[inline]
    pub const fn is_pending_approval(&self) -> bool {
        self.status_enum == 100
    }

    #[inline]
    pub const fn is_approved(&self) -> bool {
        self.status_enum == 200
    }

    #[inline]
    pub const fn is_rejected(&self) -> bool {
        self.status_enum == 300
    }

    /// Marks the request as approved (`status_enum = 200`).
    pub fn approve(&mut self) {
        self.status_enum = LoanRescheduleRequestStatusEnum::Approved.as_i32();
    }

    /// Marks the request as rejected (`status_enum = 300`).
    pub fn reject(&mut self) {
        self.status_enum = LoanRescheduleRequestStatusEnum::Rejected.as_i32();
    }

    /// Returns `true` if at least one rescheduling variation is configured.
    #[inline]
    pub fn has_variation(&self) -> bool {
        self.adjusted_due_date.is_some()
            || self.extra_terms.unwrap_or(0) > 0
            || self.grace_on_principal.unwrap_or(0) > 0
            || self.grace_on_interest.unwrap_or(0) > 0
            || self.new_interest_rate.is_some()
    }
}

/// Schedule calculation parameters used by [`LoanRescheduleEngine::apply_reschedule`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanRescheduleConfig {
    pub currency: MonetaryCurrency,
    pub current_nominal_annual_interest_rate: Decimal,
    pub repayment_every: u32,
    pub repayment_frequency_type: PeriodFrequencyType,
    pub amortization_method: AmortizationMethod,
    pub interest_method: InterestMethod,
}

impl Default for LoanRescheduleConfig {
    fn default() -> Self {
        Self {
            currency: MonetaryCurrency::usd(),
            current_nominal_annual_interest_rate: Decimal::from(12),
            repayment_every: 1,
            repayment_frequency_type: PeriodFrequencyType::Months,
            amortization_method: AmortizationMethod::EqualInstallments,
            interest_method: InterestMethod::DecliningBalance,
        }
    }
}

impl LoanRescheduleConfig {
    /// Creates a [`LoanRescheduleConfig`] directly from a [`LoanProduct`].
    pub fn from_product(product: &LoanProduct) -> Self {
        Self {
            currency: product.currency(),
            current_nominal_annual_interest_rate: product.default_nominal_interest_rate,
            repayment_every: product.repayment_every,
            repayment_frequency_type: product.repayment_frequency_type,
            amortization_method: product.amortization_method,
            interest_method: product.interest_method,
        }
    }
}

/// Output produced by [`LoanRescheduleEngine::apply_reschedule`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanRescheduleResult {
    pub installments: Vec<LoanRepaymentScheduleInstallment>,
    pub rescheduled_from_installment: u32,
    pub original_installment_count: usize,
    pub new_installment_count: usize,
    pub remaining_principal_rescheduled: Decimal,
    pub new_nominal_annual_interest_rate: Decimal,
    pub recalculated_emi: Decimal,
    pub new_maturity_date: NaiveDate,
}

/// Engine validating and applying [`LoanRescheduleRequest`] modifications to a loan repayment schedule.
///
/// Ported from `LoanRescheduleRequestDataValidatorImpl.java`,
/// `LoanRescheduleRequestWritePlatformServiceImpl.java`, and `AbstractCumulativeLoanScheduleGenerator.java`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoanRescheduleEngine;

impl LoanRescheduleEngine {
    /// Validates a [`LoanRescheduleRequest`] against the current repayment schedule:
    /// 1. At least one variation (`adjusted_due_date`, `extra_terms`, `grace_on_principal`, `grace_on_interest`, `new_interest_rate`) must be specified.
    /// 2. `reschedule_from_installment` must exist in `installments`.
    /// 3. The target installment must not already be fully paid (`obligations_met == false`).
    /// 4. If `adjusted_due_date` is provided, it must not precede the installment's `from_date`.
    /// 5. If `new_interest_rate` is provided, it must be `>= 0`.
    pub fn validate_request(
        installments: &[LoanRepaymentScheduleInstallment],
        request: &LoanRescheduleRequest,
    ) -> FineractResult<usize> {
        let mut b = DataValidatorBuilder::for_resource("loanRescheduleRequest");

        if !request.has_variation() {
            b = b
                .reset()
                .parameter("rescheduleFromDate")
                .fail_with_code(
                    "variation.required",
                    "At least one reschedule variation (adjustedDueDate, extraTerms, graceOnPrincipal, graceOnInterest, or newInterestRate) must be provided.",
                );
            return Err(FineractError::DataValidation(b.finish().unwrap_err()));
        }

        if let Some(new_rate) = request.new_interest_rate {
            if new_rate < Decimal::ZERO {
                b = b
                    .reset()
                    .parameter("newInterestRate")
                    .value(new_rate)
                    .zero_or_positive_amount();
                return Err(FineractError::DataValidation(b.finish().unwrap_err()));
            }
        }

        let target_num = i32::try_from(request.reschedule_from_installment).unwrap_or(i32::MAX);
        let Some(start_idx) = installments
            .iter()
            .position(|inst| inst.installment_number == target_num)
        else {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.loan.reschedule.installment.does.not.exist",
                    format!(
                        "Installment number {} does not exist in the repayment schedule.",
                        request.reschedule_from_installment
                    ),
                    vec![ApiErrorArg::from(i64::from(
                        request.reschedule_from_installment,
                    ))],
                ),
            ));
        };

        let target_inst = &installments[start_idx];
        if target_inst.obligations_met
            || (target_inst.total_due() > Decimal::ZERO
                && target_inst.total_outstanding() == Decimal::ZERO)
        {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.loan.reschedule.installment.already.paid",
                    format!(
                        "Installment {} is already fully paid and cannot be rescheduled.",
                        request.reschedule_from_installment
                    ),
                    vec![ApiErrorArg::from(i64::from(
                        request.reschedule_from_installment,
                    ))],
                ),
            ));
        }

        if let Some(adjusted_due) = request.adjusted_due_date {
            if let Some(from_date) = target_inst.from_date {
                if adjusted_due <= from_date {
                    b = b
                        .reset()
                        .parameter("adjustedDueDate")
                        .value(adjusted_due.to_string())
                        .fail_with_code(
                            "cannot.be.on.or.before.period.from.date",
                            "Adjusted due date must be strictly after the installment period start date.",
                        );
                    return Err(FineractError::DataValidation(b.finish().unwrap_err()));
                }
            }
            if start_idx > 0 && adjusted_due <= installments[start_idx - 1].due_date {
                b = b
                    .reset()
                    .parameter("adjustedDueDate")
                    .value(adjusted_due.to_string())
                    .fail_with_code(
                        "cannot.be.on.or.before.previous.installment.due.date",
                        "Adjusted due date must be strictly after the previous installment's due date.",
                    );
                return Err(FineractError::DataValidation(b.finish().unwrap_err()));
            }
        }

        Ok(start_idx)
    }

    /// Applies a [`LoanRescheduleRequest`] to `installments`:
    /// - Preserves all installments prior to `reschedule_from_installment` untouched.
    /// - Shifts the due date of `reschedule_from_installment` to `adjusted_due_date` (if provided) and cascades
    ///   subsequent installment `from_date` and `due_date` using the loan's repayment frequency.
    /// - Extends the remaining schedule by `extra_terms` installments (plus any principal grace periods if needed so
    ///   there is always at least one principal-amortizing period).
    /// - Applies `grace_on_principal` (setting principal due to `0` during grace periods) and `grace_on_interest`
    ///   (setting interest due to `0` during interest grace periods).
    /// - Applies `new_interest_rate` (or keeps `config.current_nominal_annual_interest_rate`) and recalculates
    ///   the remaining EMI and principal/interest split across all remaining future installments so that the sum
    ///   of future principal exactly equals the remaining unpaid principal balance.
    pub fn apply_reschedule(
        installments: &[LoanRepaymentScheduleInstallment],
        request: &LoanRescheduleRequest,
        config: &LoanRescheduleConfig,
    ) -> FineractResult<LoanRescheduleResult> {
        let start_idx = Self::validate_request(installments, request)?;

        let original_count = installments.len();
        let preserved: Vec<LoanRepaymentScheduleInstallment> =
            installments[..start_idx].to_vec();
        let future_slice = &installments[start_idx..];

        // Total unpaid principal across future installments (plus any partial principal already paid on future_slice[0])
        let remaining_unpaid_principal: Decimal = future_slice
            .iter()
            .map(LoanRepaymentScheduleInstallment::principal_outstanding)
            .sum();

        let original_remaining_terms = u32::try_from(future_slice.len()).unwrap_or(1);
        let extra_terms = request.extra_terms.unwrap_or(0);
        let principal_grace = request.grace_on_principal.unwrap_or(0);
        let interest_grace = request.grace_on_interest.unwrap_or(0);

        // Total future installments: original remaining + extra_terms (and if principal_grace >= total_terms,
        // extend by principal_grace just like Fineract's ExtendRepaymentPeriod / GraceOnPrincipal handling)
        let mut total_future_terms = original_remaining_terms.saturating_add(extra_terms);
        if principal_grace >= total_future_terms {
            total_future_terms = total_future_terms.saturating_add(principal_grace);
        }
        total_future_terms = total_future_terms.max(1);

        let effective_annual_rate = request
            .new_interest_rate
            .unwrap_or(config.current_nominal_annual_interest_rate);
        let periods_per_year = config
            .repayment_frequency_type
            .periods_per_year(config.repayment_every);
        let periodic_rate = if periods_per_year > Decimal::ZERO {
            effective_annual_rate / (Decimal::from(100) * periods_per_year)
        } else {
            Decimal::ZERO
        };

        // Determine starting from_date and first rescheduled due_date
        let first_from_date = future_slice[0].from_date.or_else(|| {
            if start_idx > 0 {
                Some(installments[start_idx - 1].due_date)
            } else {
                None
            }
        });
        let first_due_date = request
            .adjusted_due_date
            .unwrap_or(future_slice[0].due_date);

        let amortizing_terms = total_future_terms.saturating_sub(principal_grace).max(1);

        // Calculate standard periodic EMI for EqualInstallments over `amortizing_terms`
        let recalculated_emi = Self::calculate_periodic_emi(
            remaining_unpaid_principal,
            periodic_rate,
            amortizing_terms,
            config.amortization_method,
            config.interest_method,
            config.currency.clone(),
        );

        let loan_id = future_slice[0].loan_id.or(Some(request.loan_id));
        let base_installment_number = future_slice[0].installment_number;

        let mut new_future_installments = Vec::with_capacity(
            usize::try_from(total_future_terms).unwrap_or(future_slice.len()),
        );
        let mut balance_remaining = remaining_unpaid_principal;
        let mut prev_due_date = first_due_date;

        for offset in 0..total_future_terms {
            let inst_number =
                base_installment_number + i32::try_from(offset).unwrap_or(0);
            let period_from = if offset == 0 {
                first_from_date
            } else {
                Some(prev_due_date)
            };
            let period_due = if offset == 0 {
                first_due_date
            } else {
                config.repayment_frequency_type.add_periods(
                    first_due_date,
                    config.repayment_every,
                    offset,
                )
            };
            prev_due_date = period_due;

            let is_principal_grace = offset < principal_grace;
            let is_interest_grace = offset < interest_grace;
            let is_last_installment = offset + 1 == total_future_terms;

            // Calculate interest for this period
            let raw_interest = if is_interest_grace || periodic_rate <= Decimal::ZERO {
                Decimal::ZERO
            } else {
                match config.interest_method {
                    InterestMethod::DecliningBalance | InterestMethod::Invalid => {
                        balance_remaining * periodic_rate
                    }
                    InterestMethod::Flat => {
                        remaining_unpaid_principal * periodic_rate
                    }
                }
            };
            let period_interest = Money::of(config.currency.clone(), raw_interest, FineractRoundingMode::HalfEven).amount();

            // Calculate principal for this period
            let period_principal = if is_principal_grace {
                Decimal::ZERO
            } else if is_last_installment {
                balance_remaining
            } else {
                match config.amortization_method {
                    AmortizationMethod::EqualPrincipal => {
                        let raw_p =
                            remaining_unpaid_principal / Decimal::from(amortizing_terms);
                        Money::of(config.currency.clone(), raw_p, FineractRoundingMode::HalfEven)
                            .amount()
                            .min(balance_remaining)
                    }
                    AmortizationMethod::EqualInstallments | AmortizationMethod::Invalid => {
                        let raw_p = (recalculated_emi - period_interest).max(Decimal::ZERO);
                        Money::of(config.currency.clone(), raw_p, FineractRoundingMode::HalfEven)
                            .amount()
                            .min(balance_remaining)
                    }
                }
            };

            balance_remaining = (balance_remaining - period_principal).max(Decimal::ZERO);

            // Preserve any existing partial payments / fees from the original first rescheduled installment
            let (existing_p_paid, existing_i_paid, fee_amt, pen_amt) =
                if offset == 0 {
                    (
                        future_slice[0].principal_completed,
                        future_slice[0].interest_completed,
                        future_slice[0].fee_charges_amount,
                        future_slice[0].penalty_charges_amount,
                    )
                } else if let Some(orig) = future_slice.get(usize::try_from(offset).unwrap_or(usize::MAX)) {
                    (
                        Decimal::ZERO,
                        Decimal::ZERO,
                        orig.fee_charges_amount,
                        orig.penalty_charges_amount,
                    )
                } else {
                    (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO)
                };

            let mut inst = LoanRepaymentScheduleInstallment::new(
                loan_id,
                inst_number,
                period_from,
                period_due,
                period_principal + existing_p_paid,
                period_interest + existing_i_paid,
                fee_amt,
                pen_amt,
            );
            inst.principal_completed = existing_p_paid;
            inst.interest_completed = existing_i_paid;
            inst.recalculated_interest_component = true;
            new_future_installments.push(inst);
        }

        let mut all_installments = preserved;
        all_installments.extend(new_future_installments);
        let new_maturity_date = all_installments
            .last()
            .map(|i| i.due_date)
            .unwrap_or(request.reschedule_from_date);

        Ok(LoanRescheduleResult {
            new_installment_count: all_installments.len(),
            installments: all_installments,
            rescheduled_from_installment: request.reschedule_from_installment,
            original_installment_count: original_count,
            remaining_principal_rescheduled: remaining_unpaid_principal,
            new_nominal_annual_interest_rate: effective_annual_rate,
            recalculated_emi,
            new_maturity_date,
        })
    }

    /// Computes the periodic Equal Monthly/Periodic Installment (EMI) using pure `Decimal` arithmetic:
    /// \[
    /// \text{EMI} = P \cdot \frac{r (1 + r)^n}{(1 + r)^n - 1}
    /// \]
    pub fn calculate_periodic_emi(
        principal: Decimal,
        periodic_rate: Decimal,
        number_of_payments: u32,
        amortization_method: AmortizationMethod,
        interest_method: InterestMethod,
        currency: MonetaryCurrency,
    ) -> Decimal {
        let n = number_of_payments.max(1);
        let n_dec = Decimal::from(n);
        if principal <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        if periodic_rate <= Decimal::ZERO {
            return Money::of(currency, principal / n_dec, FineractRoundingMode::HalfEven).amount();
        }
        if interest_method.is_flat() || amortization_method.is_equal_principal() {
            let p_part = principal / n_dec;
            let i_part = principal * periodic_rate;
            return Money::of(currency, p_part + i_part, FineractRoundingMode::HalfEven).amount();
        }

        // Declining-balance Equal Installments: P * r * (1 + r)^n / ((1 + r)^n - 1)
        let one_plus_r = Decimal::ONE + periodic_rate;
        let mut pow_factor = Decimal::ONE;
        for _ in 0..n {
            pow_factor = (pow_factor * one_plus_r).round_dp_with_strategy(
                12,
                FineractRoundingMode::HalfEven.to_rounding_strategy(),
            );
        }
        let denominator = pow_factor - Decimal::ONE;
        if denominator <= Decimal::ZERO {
            return Money::of(currency, principal / n_dec, FineractRoundingMode::HalfEven).amount();
        }
        let raw_emi = (principal * periodic_rate * pow_factor) / denominator;
        Money::of(currency, raw_emi, FineractRoundingMode::HalfEven).amount()
    }
}

// =============================================================================
// 6. GUARANTOR & GUARANTOR FUNDING HOLD LOCKS (`m_guarantor`, `m_guarantor_funding_details`)
// =============================================================================

/// Type of loan guarantor (`guarantor_type_enum` in `m_guarantor`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.guarantor.domain.GuarantorType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum GuarantorType {
    /// Existing client of the financial institution (`1`).
    #[default]
    Client = 1,
    /// Staff member of the financial institution (`2`).
    Staff = 2,
    /// External individual or entity not registered as a client/staff (`3`).
    External = 3,
}

impl GuarantorType {
    pub const CLIENT: Self = Self::Client;
    pub const CUSTOMER: Self = Self::Client;
    pub const STAFF: Self = Self::Staff;
    pub const EXTERNAL: Self = Self::External;

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Client),
            2 => Some(Self::Staff),
            3 => Some(Self::External),
            _ => None,
        }
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Client => "guarantor.existing.customer",
            Self::Staff => "guarantor.staff",
            Self::External => "guarantor.external",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Client => "Existing Customer",
            Self::Staff => "Staff",
            Self::External => "External",
        }
    }

    #[inline]
    pub const fn is_client(self) -> bool {
        matches!(self, Self::Client)
    }

    #[inline]
    pub const fn is_staff(self) -> bool {
        matches!(self, Self::Staff)
    }

    #[inline]
    pub const fn is_external(self) -> bool {
        matches!(self, Self::External)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

/// Lifecycle status of a [`GuarantorFundingDetails`] record (`status_enum` in `m_guarantor_funding_details`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.guarantor.domain.GuarantorFundStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum GuarantorFundStatusType {
    Invalid = 0,
    #[default]
    Active = 100,
    Completed = 200,
    Withdrawn = 300,
    Deleted = 400,
}

impl GuarantorFundStatusType {
    pub const INVALID: Self = Self::Invalid;
    pub const ACTIVE: Self = Self::Active;
    pub const COMPLETED: Self = Self::Completed;
    pub const WITHDRAWN: Self = Self::Withdrawn;
    pub const DELETED: Self = Self::Deleted;

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        match v {
            100 => Self::Active,
            200 => Self::Completed,
            300 => Self::Withdrawn,
            400 => Self::Deleted,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    #[inline]
    pub const fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }
}

/// Guarantor funding details (`m_guarantor_funding_details`) tracking the savings account hold lock
/// backing a [`Guarantor`].
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.guarantor.domain.GuarantorFundingDetails`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuarantorFundingDetails {
    pub id: i64,
    pub guarantor_id: i64,
    pub savings_account_id: i64,
    pub status_enum: i32,
    pub amount: Decimal,
    pub amount_released_derived: Decimal,
    pub amount_remaining_derived: Decimal,
    pub amount_transfered_derived: Decimal,
    pub hold_transaction_id: Option<i64>,
    pub release_transaction_ids: Vec<i64>,
}

impl Default for GuarantorFundingDetails {
    fn default() -> Self {
        Self {
            id: 0,
            guarantor_id: 0,
            savings_account_id: 0,
            status_enum: GuarantorFundStatusType::Active.as_i32(),
            amount: Decimal::ZERO,
            amount_released_derived: Decimal::ZERO,
            amount_remaining_derived: Decimal::ZERO,
            amount_transfered_derived: Decimal::ZERO,
            hold_transaction_id: None,
            release_transaction_ids: Vec::new(),
        }
    }
}

impl GuarantorFundingDetails {
    /// Creates a new active [`GuarantorFundingDetails`] for `savings_account_id` and `amount`.
    pub fn new(id: i64, guarantor_id: i64, savings_account_id: i64, amount: Decimal) -> Self {
        Self {
            id,
            guarantor_id,
            savings_account_id,
            status_enum: GuarantorFundStatusType::Active.as_i32(),
            amount,
            amount_released_derived: Decimal::ZERO,
            amount_remaining_derived: amount,
            amount_transfered_derived: Decimal::ZERO,
            hold_transaction_id: None,
            release_transaction_ids: Vec::new(),
        }
    }

    #[inline]
    pub const fn status(&self) -> GuarantorFundStatusType {
        GuarantorFundStatusType::from_i32(self.status_enum)
    }

    #[inline]
    pub const fn is_active(&self) -> bool {
        self.status_enum == 100
    }

    #[inline]
    pub const fn amount_released(&self) -> Decimal {
        self.amount_released_derived
    }

    #[inline]
    pub const fn amount_remaining(&self) -> Decimal {
        self.amount_remaining_derived
    }

    #[inline]
    pub const fn amount_transferred(&self) -> Decimal {
        self.amount_transfered_derived
    }

    /// Releases up to `amount` from `amount_remaining_derived`, transitioning `status_enum` to
    /// [`GuarantorFundStatusType::Completed`] (`200`) once `amount_remaining_derived == 0`.
    /// Mirrors `GuarantorFundingDetails.releaseFunds` in Fineract.
    pub fn release_funds(&mut self, amount: Decimal) -> Decimal {
        if amount <= Decimal::ZERO || self.amount_remaining_derived <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let actual_released = amount.min(self.amount_remaining_derived);
        self.amount_released_derived += actual_released;
        self.amount_remaining_derived -= actual_released;
        if self.amount_remaining_derived == Decimal::ZERO {
            self.status_enum = GuarantorFundStatusType::Completed.as_i32();
        }
        actual_released
    }

    /// Reverses (`undoReleaseFunds`) a previously released amount on this funding detail.
    pub fn undo_release_funds(&mut self, amount: Decimal) {
        if amount <= Decimal::ZERO {
            return;
        }
        let restored = amount.min(self.amount_released_derived);
        self.amount_released_derived -= restored;
        self.amount_remaining_derived += restored;
        if self.amount_remaining_derived > Decimal::ZERO
            && self.status_enum == GuarantorFundStatusType::Completed.as_i32()
        {
            self.status_enum = GuarantorFundStatusType::Active.as_i32();
        }
    }

    /// Releases all remaining funds and sets `status_enum` (`Withdrawn`, `Deleted`, or `Completed`).
    pub fn withdraw_all_funds(&mut self, target_status: GuarantorFundStatusType) -> Decimal {
        let released = self.amount_remaining_derived;
        self.amount_released_derived += released;
        self.amount_remaining_derived = Decimal::ZERO;
        self.status_enum = target_status.as_i32();
        released
    }
}

/// Loan Guarantor entity (`m_guarantor`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.guarantor.domain.Guarantor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guarantor {
    pub id: i64,
    pub loan_id: i64,
    pub client_reln_cv_id: Option<i64>,
    pub guarantor_type: GuarantorType,
    pub entity_id: Option<i64>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub is_active: bool,
    pub funding_details: Vec<GuarantorFundingDetails>,
}

impl Default for Guarantor {
    fn default() -> Self {
        Self {
            id: 0,
            loan_id: 0,
            client_reln_cv_id: None,
            guarantor_type: GuarantorType::Client,
            entity_id: None,
            firstname: None,
            lastname: None,
            is_active: true,
            funding_details: Vec::new(),
        }
    }
}

impl Guarantor {
    /// Creates a new active [`Guarantor`] for a loan.
    pub fn new(
        id: i64,
        loan_id: i64,
        guarantor_type: GuarantorType,
        entity_id: Option<i64>,
        funding_details: Vec<GuarantorFundingDetails>,
    ) -> Self {
        Self {
            id,
            loan_id,
            client_reln_cv_id: None,
            guarantor_type,
            entity_id,
            firstname: None,
            lastname: None,
            is_active: true,
            funding_details,
        }
    }

    /// Returns `true` if this guarantor is a self-guarantee by `loan_client_id`.
    #[inline]
    pub fn is_self_guarantee(&self, loan_client_id: i64) -> bool {
        self.guarantor_type == GuarantorType::Client && self.entity_id == Some(loan_client_id)
    }

    /// Returns `true` if this guarantor is an external/third-party customer guarantee (not `loan_client_id`).
    #[inline]
    pub fn is_external_guarantee(&self, loan_client_id: i64) -> bool {
        !self.is_self_guarantee(loan_client_id)
    }

    /// Total originally guaranteed amount across active funding details.
    pub fn total_guaranteed_amount(&self) -> Decimal {
        if !self.is_active {
            return Decimal::ZERO;
        }
        self.funding_details
            .iter()
            .filter(|fd| {
                matches!(
                    fd.status(),
                    GuarantorFundStatusType::Active | GuarantorFundStatusType::Completed
                )
            })
            .map(|fd| fd.amount)
            .sum()
    }

    /// Total remaining on-hold amount across active funding details.
    pub fn total_remaining_on_hold(&self) -> Decimal {
        if !self.is_active {
            return Decimal::ZERO;
        }
        self.funding_details
            .iter()
            .filter(|fd| fd.is_active())
            .map(|fd| fd.amount_remaining_derived)
            .sum()
    }
}

/// Product-level guarantee requirements (`m_product_loan_guarantee_details`).
///
/// Ported from `org.apache.fineract.portfolio.loanproduct.domain.LoanProductGuaranteeDetails`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoanProductGuaranteeDetails {
    pub id: i64,
    pub product_id: i64,
    pub mandatory_guarantee: Decimal,
    pub minimum_guarantee_from_own_funds: Option<Decimal>,
    pub minimum_guarantee_from_guarantor_funds: Option<Decimal>,
}

/// Summary of guarantor hold lock placement or release operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuarantorHoldSummary {
    pub total_guaranteed_amount: Decimal,
    pub total_locked_amount: Decimal,
    pub total_released_amount: Decimal,
    pub total_remaining_on_hold: Decimal,
}

/// Engine managing savings account hold locks when a loan with guarantor funds is approved/disbursed
/// and releasing proportional guarantor holds upon loan repayment.
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.guarantor.service.GuarantorDomainServiceImpl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct GuarantorHoldEngine;

/// Alias for [`GuarantorHoldEngine`] matching Fineract's `GuarantorDomainService`.
pub type GuarantorDomainService = GuarantorHoldEngine;

/// Alias for [`GuarantorHoldEngine`].
pub type GuarantorLockEngine = GuarantorHoldEngine;

impl GuarantorHoldEngine {
    /// Validates guarantor coverage against [`LoanProductGuaranteeDetails`] (if provided) and places
    /// savings account hold locks on each guarantor's linked [`SavingsAccount`] when a loan is approved/disbursed.
    ///
    /// Mirrors `GuarantorDomainServiceImpl.validateGuarantorBusinessRules` and `assignGuarantor`:
    /// 1. Validates that each active [`GuarantorFundingDetails`] references an active [`SavingsAccount`]
    ///    with sufficient `withdrawable_balance()`.
    /// 2. Updates `savings_account.on_hold_funds` and creates a `SavingsAccountTransactionType::AmountHold`
    ///    lock transaction on the savings account via `hold_amount(...)`.
    pub fn lock_guarantor_funds_on_approval(
        loan_principal: Decimal,
        loan_client_id: i64,
        guarantee_requirements: Option<&LoanProductGuaranteeDetails>,
        guarantors: &mut [Guarantor],
        savings_accounts: &mut BTreeMap<i64, SavingsAccount>,
        lock_date: NaiveDate,
    ) -> FineractResult<GuarantorHoldSummary> {
        // 1. Validate mandatory guarantee percentages if configured on the product
        if let Some(req) = guarantee_requirements {
            let mut total_self_funds = Decimal::ZERO;
            let mut total_ext_funds = Decimal::ZERO;

            for g in guarantors.iter().filter(|g| g.is_active) {
                let g_amount: Decimal = g
                    .funding_details
                    .iter()
                    .filter(|fd| fd.is_active())
                    .map(|fd| fd.amount)
                    .sum();
                if g.is_self_guarantee(loan_client_id) {
                    total_self_funds += g_amount;
                } else {
                    total_ext_funds += g_amount;
                }
            }

            let mandatory_total =
                (loan_principal * req.mandatory_guarantee) / Decimal::from(100);
            if total_self_funds + total_ext_funds < mandatory_total {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.loan.guarantor.min.funds.not.met",
                        format!(
                            "Total guaranteed funds ({}) is less than mandatory guarantee requirement ({mandatory_total}).",
                            total_self_funds + total_ext_funds
                        ),
                        vec![
                            ApiErrorArg::from(total_self_funds + total_ext_funds),
                            ApiErrorArg::from(mandatory_total),
                        ],
                    ),
                ));
            }

            if let Some(min_own_pct) = req.minimum_guarantee_from_own_funds {
                let min_own_amt = (loan_principal * min_own_pct) / Decimal::from(100);
                if total_self_funds < min_own_amt {
                    return Err(FineractError::DomainRule(
                        PlatformDomainRuleException::with_args(
                            "error.msg.loan.guarantor.min.self.funds.not.met",
                            format!(
                                "Self-guaranteed funds ({total_self_funds}) is less than minimum required ({min_own_amt})."
                            ),
                            vec![
                                ApiErrorArg::from(total_self_funds),
                                ApiErrorArg::from(min_own_amt),
                            ],
                        ),
                    ));
                }
            }

            if let Some(min_ext_pct) = req.minimum_guarantee_from_guarantor_funds {
                let min_ext_amt = (loan_principal * min_ext_pct) / Decimal::from(100);
                if total_ext_funds < min_ext_amt {
                    return Err(FineractError::DomainRule(
                        PlatformDomainRuleException::with_args(
                            "error.msg.loan.guarantor.min.external.funds.not.met",
                            format!(
                                "External guarantor funds ({total_ext_funds}) is less than minimum required ({min_ext_amt})."
                            ),
                            vec![
                                ApiErrorArg::from(total_ext_funds),
                                ApiErrorArg::from(min_ext_amt),
                            ],
                        ),
                    ));
                }
            }
        }

        // 2. Lock funds on each referenced savings account
        let mut total_guaranteed_amount = Decimal::ZERO;
        let mut total_locked_amount = Decimal::ZERO;

        for guarantor in guarantors.iter_mut().filter(|g| g.is_active) {
            for fd in guarantor.funding_details.iter_mut().filter(|fd| fd.is_active()) {
                total_guaranteed_amount += fd.amount;
                if fd.hold_transaction_id.is_some() {
                    // Already locked
                    total_locked_amount += fd.amount_remaining_derived;
                    continue;
                }

                let Some(savings) = savings_accounts.get_mut(&fd.savings_account_id) else {
                    return Err(FineractError::resource_not_found(
                        "error.msg.saving.account.id.invalid",
                        format!(
                            "Savings account with identifier {} does not exist for guarantor funding lock.",
                            fd.savings_account_id
                        ),
                        fd.savings_account_id.to_string(),
                    ));
                };

                let hold_tx = savings
                    .hold_amount(
                        lock_date,
                        fd.amount,
                        None,
                        lock_date,
                        false,
                        Some(format!("Loan #{} Guarantor Hold", guarantor.loan_id)),
                    )
                    .map_err(FineractError::DomainRule)?;

                fd.hold_transaction_id = hold_tx.id;
                fd.amount_remaining_derived = fd.amount;
                fd.amount_released_derived = Decimal::ZERO;
                total_locked_amount += fd.amount;
            }
        }

        Ok(GuarantorHoldSummary {
            total_guaranteed_amount,
            total_locked_amount,
            total_released_amount: Decimal::ZERO,
            total_remaining_on_hold: total_locked_amount,
        })
    }

    /// Releases proportional guarantor hold locks upon loan principal repayment (`GuarantorDomainServiceImpl.relaseGuarantorFunds`):
    /// \[
    /// \text{releaseAmount} = \frac{\text{principalPaid} \times \text{totalGuaranteedAmount}}{\text{loanPrincipal}}
    /// \]
    /// If `remaining_loan_principal_after_repayment == 0`, releases 100% of all remaining active guarantor holds.
    /// External guarantor holds are released before self-guarantee holds, matching Fineract's ordering.
    pub fn release_proportional_holds_on_repayment(
        loan_principal: Decimal,
        principal_paid: Decimal,
        remaining_loan_principal_after_repayment: Decimal,
        loan_client_id: i64,
        currency: MonetaryCurrency,
        guarantors: &mut [Guarantor],
        savings_accounts: &mut BTreeMap<i64, SavingsAccount>,
        release_date: NaiveDate,
    ) -> FineractResult<GuarantorHoldSummary> {
        if principal_paid <= Decimal::ZERO || loan_principal <= Decimal::ZERO {
            return Ok(Self::summarize(guarantors));
        }

        let total_guaranteed: Decimal = guarantors
            .iter()
            .filter(|g| g.is_active)
            .map(Guarantor::total_guaranteed_amount)
            .sum();
        let total_remaining_before: Decimal = guarantors
            .iter()
            .filter(|g| g.is_active)
            .map(Guarantor::total_remaining_on_hold)
            .sum();

        if total_remaining_before <= Decimal::ZERO {
            return Ok(Self::summarize(guarantors));
        }

        let mut amount_to_release =
            if remaining_loan_principal_after_repayment <= Decimal::ZERO {
                total_remaining_before
            } else {
                let raw_share = (principal_paid * total_guaranteed) / loan_principal;
                Money::of(currency, raw_share, FineractRoundingMode::HalfEven)
                    .amount()
                    .min(total_remaining_before)
            };

        // Pass 1: Release external guarantor holds first, then Pass 2: self-guarantor holds
        for release_self in [false, true] {
            for guarantor in guarantors.iter_mut().filter(|g| g.is_active) {
                if guarantor.is_self_guarantee(loan_client_id) != release_self {
                    continue;
                }
                for fd in guarantor.funding_details.iter_mut().filter(|fd| fd.is_active()) {
                    if amount_to_release <= Decimal::ZERO {
                        break;
                    }
                    let released = fd.release_funds(amount_to_release);
                    if released <= Decimal::ZERO {
                        continue;
                    }
                    amount_to_release -= released;

                    if let Some(savings) = savings_accounts.get_mut(&fd.savings_account_id) {
                        // If the funding detail is now completely released and has an underlying hold tx,
                        // release it on the savings account; otherwise reduce savings_amount_on_hold proportionally.
                        if fd.amount_remaining_derived == Decimal::ZERO {
                            if let Some(hold_tx_id) = fd.hold_transaction_id {
                                // Restore any prior partial reductions before calling release_amount so the full tx amount is balanced
                                let prior_partial = fd.amount - released;
                                if prior_partial > Decimal::ZERO {
                                    savings.savings_amount_on_hold += prior_partial;
                                }
                                if let Ok(rel_tx) =
                                    savings.release_amount(hold_tx_id, release_date, release_date)
                                {
                                    if let Some(rel_id) = rel_tx.id {
                                        fd.release_transaction_ids.push(rel_id);
                                    }
                                }
                            } else {
                                savings.savings_amount_on_hold =
                                    (savings.savings_amount_on_hold - released).max(Decimal::ZERO);
                            }
                        } else {
                            savings.savings_amount_on_hold =
                                (savings.savings_amount_on_hold - released).max(Decimal::ZERO);
                        }
                    }
                }
            }
        }

        Ok(Self::summarize(guarantors))
    }

    /// Computes the current [`GuarantorHoldSummary`] across all guarantors on a loan.
    pub fn summarize(guarantors: &[Guarantor]) -> GuarantorHoldSummary {
        let mut total_guaranteed_amount = Decimal::ZERO;
        let mut total_released_amount = Decimal::ZERO;
        let mut total_remaining_on_hold = Decimal::ZERO;

        for g in guarantors.iter().filter(|g| g.is_active) {
            for fd in &g.funding_details {
                if matches!(
                    fd.status(),
                    GuarantorFundStatusType::Active | GuarantorFundStatusType::Completed
                ) {
                    total_guaranteed_amount += fd.amount;
                    total_released_amount += fd.amount_released_derived;
                    total_remaining_on_hold += fd.amount_remaining_derived;
                }
            }
        }

        GuarantorHoldSummary {
            total_guaranteed_amount,
            total_locked_amount: total_guaranteed_amount,
            total_released_amount,
            total_remaining_on_hold,
        }
    }
}

// =============================================================================
// 7. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[test]
    fn test_loan_product_min_default_max_bounds_and_progressive_validation() {
        // 1. Valid default cumulative product
        let cumulative = LoanProduct::default();
        assert!(cumulative.validate().is_ok());

        // 2. Invalid principal bounds (min > default, default > max)
        let bad_principal = LoanProduct::default()
            .with_principal_bounds(dec!(5000), dec!(2000), dec!(10000));
        let err = bad_principal.validate().unwrap_err();
        assert!(err
            .errors
            .iter()
            .any(|e| e.parameter_name == "principal"));

        // 3. Invalid interest bounds
        let bad_interest = LoanProduct::default()
            .with_interest_rate_bounds(dec!(15), dec!(12), dec!(10));
        let err = bad_interest.validate().unwrap_err();
        assert!(err
            .errors
            .iter()
            .any(|e| e.parameter_name == "minInterestRatePerPeriod"));

        // 4. Invalid repayment count bounds
        let bad_repayments = LoanProduct::default().with_repayment_count_bounds(12, 6, 24);
        let err = bad_repayments.validate().unwrap_err();
        assert!(err
            .errors
            .iter()
            .any(|e| e.parameter_name == "numberOfRepayments"));

        // 5. Progressive loan requires advanced-payment-allocation-strategy
        let mut bad_progressive = LoanProduct::default();
        bad_progressive.loan_schedule_type = LoanScheduleType::Progressive;
        bad_progressive.transaction_processing_strategy_code =
            String::from(MIFOS_STANDARD_STRATEGY);
        let err = bad_progressive.validate().unwrap_err();
        assert!(err.errors.iter().any(|e| {
            e.user_message_globalisation_code.contains(
                "loanScheduleType.supported.only.with.advanced.payment.allocation.strategy",
            )
        }));

        // 6. Vertical processing is rejected on Cumulative schedule
        let mut bad_vertical = LoanProduct::default();
        bad_vertical.loan_schedule_processing_type = LoanScheduleProcessingType::Vertical;
        let err = bad_vertical.validate().unwrap_err();
        assert!(err.errors.iter().any(|e| {
            e.user_message_globalisation_code.contains(
                "loanScheduleProcessingType.supported.only.for.progressive.loan.schedule.handling",
            )
        }));

        // 7. Valid Progressive + Vertical product with down payment
        let valid_progressive = LoanProduct::new_progressive(
            10,
            "PRG1",
            "Progressive Loan",
            "USD",
            2,
            dec!(10000),
            dec!(12),
            12,
            LoanScheduleProcessingType::Vertical,
        )
        .with_down_payment(dec!(20), true);
        assert!(valid_progressive.validate().is_ok());
        assert_eq!(
            valid_progressive.calculate_down_payment_amount(dec!(2500)),
            dec!(500.00)
        );
    }

    #[test]
    fn test_multi_tranche_disbursement_engine_validation_and_disbursal() {
        let product = LoanProduct::default()
            .with_multi_disburse(Some(2), Some(dec!(10000)));
        assert!(product.validate().is_ok());

        let t1 = LoanDisbursementDetails::new(1, 100, d(2026, 1, 1), dec!(4000));
        let t2 = LoanDisbursementDetails::new(2, 100, d(2026, 2, 1), dec!(6000));
        let t3 = LoanDisbursementDetails::new(3, 100, d(2026, 3, 1), dec!(1000));

        // Exceeding max_tranche_count (3 > 2) fails
        let err = MultiTrancheDisbursementEngine::validate_tranches(
            &product,
            dec!(10000),
            &[t1.clone(), t2.clone(), t3],
        )
        .unwrap_err();
        assert!(err.to_string().contains("maxTrancheCount"));

        // Exceeding sanctioned principal (4000 + 7000 > 10000) fails
        let t2_over = LoanDisbursementDetails::new(2, 100, d(2026, 2, 1), dec!(7000));
        let err = MultiTrancheDisbursementEngine::validate_tranches(
            &product,
            dec!(10000),
            &[t1.clone(), t2_over],
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("disburse.amount.must.be.less.than.approved.principal"));

        // Valid 2-tranche schedule and sequential disbursal
        let mut tranches = vec![t1, t2];
        let (disbursed, summary) = MultiTrancheDisbursementEngine::disburse_tranche(
            &product,
            dec!(10000),
            Decimal::ZERO,
            &mut tranches,
            Some(1),
            d(2026, 1, 1),
            None,
        )
        .expect("tranche 1 disbursal");
        assert!(disbursed.is_disbursed());
        assert_eq!(summary.total_disbursed_principal, dec!(4000));
        assert_eq!(summary.total_pending_principal, dec!(6000));
    }

    #[test]
    fn test_loan_reschedule_engine_modifies_future_installments_and_recalculates_emi() {
        let currency = MonetaryCurrency::usd();
        let mut schedule = vec![
            LoanRepaymentScheduleInstallment::new(
                Some(500),
                1,
                Some(d(2026, 1, 1)),
                d(2026, 2, 1),
                dec!(246.28),
                dec!(10.00),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            LoanRepaymentScheduleInstallment::new(
                Some(500),
                2,
                Some(d(2026, 2, 1)),
                d(2026, 3, 1),
                dec!(248.74),
                dec!(7.54),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            LoanRepaymentScheduleInstallment::new(
                Some(500),
                3,
                Some(d(2026, 3, 1)),
                d(2026, 4, 1),
                dec!(251.23),
                dec!(5.05),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            LoanRepaymentScheduleInstallment::new(
                Some(500),
                4,
                Some(d(2026, 4, 1)),
                d(2026, 5, 1),
                dec!(253.75),
                dec!(2.54),
                Decimal::ZERO,
                Decimal::ZERO,
            ),
        ];

        // Pay installment 1 completely
        schedule[0].pay_interest(d(2026, 2, 1), dec!(10.00));
        schedule[0].pay_principal(d(2026, 2, 1), dec!(246.28));
        assert!(schedule[0].obligations_met);

        // Reschedule from installment 2: shift due date to 2026-03-15, add 2 extra terms, 1 principal grace period, and lower interest rate to 6%
        let request = LoanRescheduleRequest::new(
            1,
            500,
            2,
            d(2026, 3, 1),
            Some(d(2026, 3, 15)),
            Some(2),
            Some(1),
            None,
            Some(dec!(6.0)),
        );
        let config = LoanRescheduleConfig {
            currency,
            current_nominal_annual_interest_rate: dec!(12.0),
            repayment_every: 1,
            repayment_frequency_type: PeriodFrequencyType::Months,
            amortization_method: AmortizationMethod::EqualInstallments,
            interest_method: InterestMethod::DecliningBalance,
        };

        let result = LoanRescheduleEngine::apply_reschedule(&schedule, &request, &config)
            .expect("reschedule should succeed");

        // Installment 1 is untouched + 3 remaining + 2 extra = 6 installments total
        assert_eq!(result.new_installment_count, 6);
        assert_eq!(result.installments[0].due_date, d(2026, 2, 1));
        assert_eq!(result.installments[1].due_date, d(2026, 3, 15));
        // Installment 2 has principal grace -> principal_amount == 0
        assert_eq!(result.installments[1].principal_amount, Decimal::ZERO);
        assert!(result.installments[1].interest_amount > Decimal::ZERO);

        // Sum of future principal across installments 2..=6 equals unpaid balance (753.72)
        let rescheduled_principal_sum: Decimal = result.installments[1..]
            .iter()
            .map(|i| i.principal_amount)
            .sum();
        assert_eq!(rescheduled_principal_sum, dec!(753.72));
        assert_eq!(result.new_maturity_date, d(2026, 7, 15));
    }

    #[test]
    fn test_guarantor_hold_locks_and_proportional_release_on_repayment() {
        let currency = MonetaryCurrency::usd();
        let mut sa_self = SavingsAccount::new_submitted(
            "SAV-SELF",
            Some(10),
            None,
            1,
            currency.clone(),
            dec!(5.0),
            d(2026, 1, 1),
        );
        sa_self.id = Some(101);
        sa_self.approve(d(2026, 1, 1), None).unwrap();
        sa_self.activate(d(2026, 1, 1), None).unwrap();
        sa_self
            .deposit(d(2026, 1, 1), dec!(500.00), None, d(2026, 1, 1))
            .unwrap();

        let mut sa_ext = SavingsAccount::new_submitted(
            "SAV-EXT",
            Some(20),
            None,
            1,
            currency.clone(),
            dec!(5.0),
            d(2026, 1, 1),
        );
        sa_ext.id = Some(102);
        sa_ext.approve(d(2026, 1, 1), None).unwrap();
        sa_ext.activate(d(2026, 1, 1), None).unwrap();
        sa_ext
            .deposit(d(2026, 1, 1), dec!(1000.00), None, d(2026, 1, 1))
            .unwrap();

        let mut savings_map = BTreeMap::new();
        savings_map.insert(101, sa_self);
        savings_map.insert(102, sa_ext);

        // Loan of 1000 with 200 self-guarantee (client 10) and 300 external guarantee (client 20) = 500 total
        let mut guarantors = vec![
            Guarantor::new(
                1,
                900,
                GuarantorType::Client,
                Some(10),
                vec![GuarantorFundingDetails::new(11, 1, 101, dec!(200.00))],
            ),
            Guarantor::new(
                2,
                900,
                GuarantorType::Client,
                Some(20),
                vec![GuarantorFundingDetails::new(12, 2, 102, dec!(300.00))],
            ),
        ];

        let req = LoanProductGuaranteeDetails {
            id: 1,
            product_id: 1,
            mandatory_guarantee: dec!(50),
            minimum_guarantee_from_own_funds: Some(dec!(20)),
            minimum_guarantee_from_guarantor_funds: Some(dec!(30)),
        };

        let lock_summary = GuarantorHoldEngine::lock_guarantor_funds_on_approval(
            dec!(1000.00),
            10,
            Some(&req),
            &mut guarantors,
            &mut savings_map,
            d(2026, 1, 5),
        )
        .expect("guarantor hold lock");

        assert_eq!(lock_summary.total_locked_amount, dec!(500.00));
        assert_eq!(savings_map[&101].withdrawable_balance(), dec!(300.00));
        assert_eq!(savings_map[&102].withdrawable_balance(), dec!(700.00));

        // 1. Repay 400 principal (40% of 1000 loan -> releases 40% of 500 guarantee = 200 from external guarantor first)
        let rel_1 = GuarantorHoldEngine::release_proportional_holds_on_repayment(
            dec!(1000.00),
            dec!(400.00),
            dec!(600.00),
            10,
            currency.clone(),
            &mut guarantors,
            &mut savings_map,
            d(2026, 2, 5),
        )
        .expect("proportional release 1");

        assert_eq!(rel_1.total_released_amount, dec!(200.00));
        assert_eq!(rel_1.total_remaining_on_hold, dec!(300.00));
        // External guarantor (102) had 200 released -> 100 remaining on hold
        assert_eq!(
            guarantors[1].funding_details[0].amount_remaining_derived,
            dec!(100.00)
        );
        // Self guarantor (101) still has 200 remaining on hold
        assert_eq!(
            guarantors[0].funding_details[0].amount_remaining_derived,
            dec!(200.00)
        );

        // 2. Repay remaining 600 principal -> releases all remaining 300 (100 external + 200 self) and marks Completed
        let rel_2 = GuarantorHoldEngine::release_proportional_holds_on_repayment(
            dec!(1000.00),
            dec!(600.00),
            Decimal::ZERO,
            10,
            currency.clone(),
            &mut guarantors,
            &mut savings_map,
            d(2026, 3, 5),
        )
        .expect("final release");

        assert_eq!(rel_2.total_released_amount, dec!(500.00));
        assert_eq!(rel_2.total_remaining_on_hold, Decimal::ZERO);
        assert_eq!(
            guarantors[0].funding_details[0].status(),
            GuarantorFundStatusType::Completed
        );
        assert_eq!(
            guarantors[1].funding_details[0].status(),
            GuarantorFundStatusType::Completed
        );
        assert_eq!(savings_map[&101].withdrawable_balance(), dec!(500.00));
        assert_eq!(savings_map[&102].withdrawable_balance(), dec!(1000.00));
    }
}
