//! Core loan repayment schedule generation engine (`fineract-schedule-engine`).
//!
//! Ported from Apache Fineract's `fineract-loan` and `fineract-progressive-loan-embeddable-schedule-generator`:
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.EmbeddableProgressiveLoanScheduleGenerator`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.AbstractCumulativeLoanScheduleGenerator`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.CumulativeDecliningBalanceInterestLoanScheduleGenerator`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.CumulativeFlatInterestLoanScheduleGenerator`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.DefaultScheduledDateGenerator`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.DefaultPaymentPeriodsInOneYearCalculator`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.FinanicalFunctions`
//! - `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.AprCalculator`

use chrono::{Datelike, NaiveDate};
use fineract_charge_rs::EnumOptionData;
use fineract_core_rs::calendar::{CalendarFrequencyType, CalendarUtils};
use fineract_money::{
    Decimal, DecimalRoundingExt, FineractDecimal, FineractRoundingMode, MonetaryCurrency, Money, RoundingStrategy, dec,
};
use serde::{Deserialize, Serialize};

/// Precision scale used for intermediate high-precision decimal arithmetic
/// (equivalent to Java's `MathContext(12, RoundingMode.HALF_UP)`).
const HIGH_PRECISION_SCALE: u32 = 12;

/// Amortization method for loan principal and interest repayment.
///
/// Port of `org.apache.fineract.portfolio.loanproduct.domain.AmortizationMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum AmortizationMethod {
    /// Equal principal payments per installment (`EQUAL_PRINCIPAL = 0`).
    EqualPrincipal = 0,
    /// Equal total installment (EMI) payments (`EQUAL_INSTALLMENTS = 1`).
    #[default]
    EqualInstallments = 1,
}

impl AmortizationMethod {
    pub const EQUAL_PRINCIPAL: Self = Self::EqualPrincipal;
    pub const EQUAL_INSTALLMENTS: Self = Self::EqualInstallments;

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            0 => Some(Self::EqualPrincipal),
            1 => Some(Self::EqualInstallments),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::EqualPrincipal),
            1 => Some(Self::EqualInstallments),
            _ => None,
        }
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EqualPrincipal => "amortization.method.equal.principal",
            Self::EqualInstallments => "amortization.method.equal.installments",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::EqualPrincipal => "Equal principal payments",
            Self::EqualInstallments => "Equal installments",
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

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(self.value() as i64, self.code(), self.description())
    }
}

/// Interest calculation method for loan schedules.
///
/// Port of `org.apache.fineract.portfolio.loanproduct.domain.InterestMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum InterestMethod {
    /// Interest calculated on declining principal balance (`DECLINING_BALANCE = 0`).
    #[default]
    DecliningBalance = 0,
    /// Flat interest calculated on original principal (`FLAT = 1`).
    Flat = 1,
}

impl InterestMethod {
    pub const DECLINING_BALANCE: Self = Self::DecliningBalance;
    pub const FLAT: Self = Self::Flat;

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            0 => Some(Self::DecliningBalance),
            1 => Some(Self::Flat),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::DecliningBalance),
            1 => Some(Self::Flat),
            _ => None,
        }
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DecliningBalance => "interest.method.declining.balance",
            Self::Flat => "interest.method.flat",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::DecliningBalance => "Declining Balance",
            Self::Flat => "Flat",
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

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(self.value() as i64, self.code(), self.description())
    }
}

/// Interest calculation period method.
///
/// Port of `org.apache.fineract.portfolio.loanproduct.domain.InterestCalculationPeriodMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum InterestCalculationPeriodMethod {
    /// Calculate interest based on actual days in period (`DAILY = 0`).
    Daily = 0,
    /// Calculate interest using the repayment period frequency (`SAME_AS_REPAYMENT_PERIOD = 1`).
    #[default]
    SameAsRepaymentPeriod = 1,
}

impl InterestCalculationPeriodMethod {
    pub const DAILY: Self = Self::Daily;
    pub const SAME_AS_REPAYMENT_PERIOD: Self = Self::SameAsRepaymentPeriod;

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            0 => Some(Self::Daily),
            1 => Some(Self::SameAsRepaymentPeriod),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Daily),
            1 => Some(Self::SameAsRepaymentPeriod),
            _ => None,
        }
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Daily => "interestCalculationPeriodMethod.daily",
            Self::SameAsRepaymentPeriod => {
                "interestCalculationPeriodMethod.same.as.repayment.period"
            }
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::SameAsRepaymentPeriod => "Same as repayment period",
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

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(self.value() as i64, self.code(), self.description())
    }
}

/// Day-count convention for days in a month.
///
/// Port of `org.apache.fineract.portfolio.common.domain.DaysInMonthType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum DaysInMonthType {
    /// Actual number of calendar days in the month (`ACTUAL = 1`).
    #[default]
    Actual = 1,
    /// Fixed 30 days per month (`DAYS_30 = 30`).
    Days30 = 30,
}

impl DaysInMonthType {
    pub const ACTUAL: Self = Self::Actual;
    pub const DAYS_30: Self = Self::Days30;

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::Actual),
            30 => Some(Self::Days30),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Actual),
            30 => Some(Self::Days30),
            _ => None,
        }
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Actual => "DaysInMonthType.actual",
            Self::Days30 => "DaysInMonthType.days30",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Actual => "Actual",
            Self::Days30 => "30 Days",
        }
    }

    #[inline]
    pub const fn is_days_30(self) -> bool {
        matches!(self, Self::Days30)
    }

    #[inline]
    pub const fn is_actual(self) -> bool {
        matches!(self, Self::Actual)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(self.value() as i64, self.code(), self.description())
    }
}

/// Day-count convention for days in a year.
///
/// Port of `org.apache.fineract.portfolio.common.domain.DaysInYearType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum DaysInYearType {
    /// Actual number of days in the year (365 or 366 in leap years) (`ACTUAL = 1`).
    #[default]
    Actual = 1,
    /// 360 days per year (`DAYS_360 = 360`).
    Days360 = 360,
    /// 364 days per year (`DAYS_364 = 364`).
    Days364 = 364,
    /// 365 days per year (`DAYS_365 = 365`).
    Days365 = 365,
}

impl DaysInYearType {
    pub const ACTUAL: Self = Self::Actual;
    pub const DAYS_360: Self = Self::Days360;
    pub const DAYS_364: Self = Self::Days364;
    pub const DAYS_365: Self = Self::Days365;

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::Actual),
            360 => Some(Self::Days360),
            364 => Some(Self::Days364),
            365 => Some(Self::Days365),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_int(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Actual),
            360 => Some(Self::Days360),
            364 => Some(Self::Days364),
            365 => Some(Self::Days365),
            _ => None,
        }
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Actual => "DaysInYearType.actual",
            Self::Days360 => "DaysInYearType.days360",
            Self::Days364 => "DaysInYearType.days364",
            Self::Days365 => "DaysInYearType.days365",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Actual => "Actual",
            Self::Days360 => "360 Days",
            Self::Days364 => "364 Days",
            Self::Days365 => "365 Days",
        }
    }

    #[inline]
    pub const fn is_actual(self) -> bool {
        matches!(self, Self::Actual)
    }

    /// Returns the number of days in the specified `year` according to this convention.
    #[inline]
    pub fn days_in_year_for(self, year: i32) -> u32 {
        match self {
            Self::Actual => CalendarUtils::days_in_year(year) as u32,
            Self::Days360 => 360,
            Self::Days364 => 364,
            Self::Days365 => 365,
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(self.value() as i64, self.code(), self.description())
    }
}

/// Loan schedule type (`CUMULATIVE` vs `PROGRESSIVE`).
///
/// Port of `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanScheduleType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LoanScheduleType {
    #[default]
    Cumulative,
    Progressive,
}

impl LoanScheduleType {
    pub const CUMULATIVE: Self = Self::Cumulative;
    pub const PROGRESSIVE: Self = Self::Progressive;

    pub fn to_enum_option_data(self) -> EnumOptionData {
        match self {
            Self::Cumulative => {
                EnumOptionData::new(0, "LoanScheduleType.cumulative", "Cumulative")
            }
            Self::Progressive => {
                EnumOptionData::new(1, "LoanScheduleType.progressive", "Progressive")
            }
        }
    }
}

/// Loan schedule processing type (`HORIZONTAL` vs `VERTICAL`).
///
/// Port of `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanScheduleProcessingType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LoanScheduleProcessingType {
    #[default]
    Horizontal,
    Vertical,
}

impl LoanScheduleProcessingType {
    pub const HORIZONTAL: Self = Self::Horizontal;
    pub const VERTICAL: Self = Self::Vertical;

    pub fn to_enum_option_data(self) -> EnumOptionData {
        match self {
            Self::Horizontal => {
                EnumOptionData::new(0, "LoanScheduleProcessingType.horizontal", "Horizontal")
            }
            Self::Vertical => {
                EnumOptionData::new(1, "LoanScheduleProcessingType.vertical", "Vertical")
            }
        }
    }
}

/// Complete loan schedule terms used to generate a deterministic repayment schedule.
///
/// Port of `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanApplicationTerms`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanScheduleTerms {
    pub principal: Money,
    pub annual_nominal_interest_rate: Decimal,
    pub number_of_repayments: u32,
    pub repayment_every: u32,
    pub repayment_frequency_type: CalendarFrequencyType,
    pub amortization_method: AmortizationMethod,
    pub interest_method: InterestMethod,
    pub interest_calculation_period_method: InterestCalculationPeriodMethod,
    pub days_in_month_type: DaysInMonthType,
    pub days_in_year_type: DaysInYearType,
    pub expected_disbursement_date: NaiveDate,
    pub repayments_starting_from_date: Option<NaiveDate>,
    pub principal_grace_periods: u32,
    pub interest_payment_grace_periods: u32,
    pub interest_free_grace_periods: u32,
    pub fixed_emi_amount: Option<Decimal>,
    pub down_payment_percentage: Option<Decimal>,
}

impl LoanScheduleTerms {
    /// Creates standard monthly declining-balance equal-installments loan schedule terms.
    pub fn new(
        principal: Money,
        annual_nominal_interest_rate: Decimal,
        number_of_repayments: u32,
        repayment_every: u32,
        repayment_frequency_type: CalendarFrequencyType,
        expected_disbursement_date: NaiveDate,
    ) -> Self {
        Self {
            principal,
            annual_nominal_interest_rate,
            number_of_repayments,
            repayment_every,
            repayment_frequency_type,
            amortization_method: AmortizationMethod::EqualInstallments,
            interest_method: InterestMethod::DecliningBalance,
            interest_calculation_period_method:
                InterestCalculationPeriodMethod::SameAsRepaymentPeriod,
            days_in_month_type: DaysInMonthType::Actual,
            days_in_year_type: DaysInYearType::Actual,
            expected_disbursement_date,
            repayments_starting_from_date: None,
            principal_grace_periods: 0,
            interest_payment_grace_periods: 0,
            interest_free_grace_periods: 0,
            fixed_emi_amount: None,
            down_payment_percentage: None,
        }
    }

    /// Returns the currency of the loan principal.
    #[inline]
    pub fn currency(&self) -> MonetaryCurrency {
        self.principal.currency().clone()
    }
}

/// A single installment period in a generated loan repayment schedule.
///
/// Port of `LoanScheduleModelPeriod` / `LoanRepaymentScheduleInstallment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleInstallment {
    pub installment_number: u32,
    pub from_date: NaiveDate,
    pub due_date: NaiveDate,
    pub principal_amount: Money,
    pub interest_amount: Money,
    pub fee_charges_amount: Money,
    pub penalty_charges_amount: Money,
    pub total_installment_amount: Money,
    pub principal_balance_outstanding: Money,
    pub is_down_payment: bool,
}

impl ScheduleInstallment {
    /// Returns the number of calendar days between `from_date` and `due_date`.
    #[inline]
    pub fn days_in_period(&self) -> i64 {
        CalendarUtils::days_between(self.from_date, self.due_date)
    }
}

/// The complete generated loan repayment schedule model.
///
/// Port of `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanScheduleModel`
/// and `LoanRepaymentScheduleModelData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedLoanSchedule {
    pub currency: MonetaryCurrency,
    pub disbursement_date: NaiveDate,
    pub total_principal: Money,
    pub total_interest: Money,
    pub total_fees: Money,
    pub total_repayment: Money,
    pub installments: Vec<ScheduleInstallment>,
}

impl GeneratedLoanSchedule {
    /// Returns the total penalty charges across all installments.
    pub fn total_penalties(&self) -> Money {
        let mut total = Money::zero(self.currency.clone());
        for inst in &self.installments {
            total = total.plus_unchecked(&inst.penalty_charges_amount);
        }
        total
    }
}

/// Pure, deterministic loan repayment schedule generator.
///
/// Port of `EmbeddableProgressiveLoanScheduleGenerator`, `CumulativeFlatInterestLoanScheduleGenerator`,
/// and `CumulativeDecliningBalanceInterestLoanScheduleGenerator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmbeddableScheduleGenerator;

impl EmbeddableScheduleGenerator {
    /// Creates a new `EmbeddableScheduleGenerator`.
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Generates a complete loan repayment schedule from the provided [`LoanScheduleTerms`].
    pub fn generate(&self, terms: &LoanScheduleTerms) -> GeneratedLoanSchedule {
        Self::generate_schedule(terms)
    }

    /// Static entry point to generate a complete loan repayment schedule.
    pub fn generate_schedule(terms: &LoanScheduleTerms) -> GeneratedLoanSchedule {
        let currency = terms.principal.currency().clone();
        let zero = Money::zero(currency.clone());

        if terms.number_of_repayments == 0 || !terms.principal.is_greater_than_zero() {
            return GeneratedLoanSchedule {
                currency,
                disbursement_date: terms.expected_disbursement_date,
                total_principal: zero.clone(),
                total_interest: zero.clone(),
                total_fees: zero.clone(),
                total_repayment: zero,
                installments: Vec::new(),
            };
        }

        let repayment_every = terms.repayment_every.max(1);
        let num_repayments = terms.number_of_repayments;

        let mut installments = Vec::with_capacity((num_repayments + 1) as usize);
        let mut installment_number: u32 = 1;
        let mut outstanding_balance = terms.principal.clone();

        // 1. Optional Down Payment Installment (#1 on expected_disbursement_date)
        if let Some(dp_pct) = terms.down_payment_percentage {
            if dp_pct > Decimal::ZERO {
                let clamped_pct = dp_pct.min(dec!(100));
                let dp_raw = terms.principal.amount() * clamped_pct / dec!(100);
                let mut dp_money = Money::of_with_mode(
                    currency.clone(),
                    dp_raw,
                    FineractRoundingMode::Down,
                );
                if dp_money.amount() > outstanding_balance.amount() {
                    dp_money = outstanding_balance.clone();
                }

                outstanding_balance = outstanding_balance.minus_unchecked(&dp_money);

                installments.push(ScheduleInstallment {
                    installment_number,
                    from_date: terms.expected_disbursement_date,
                    due_date: terms.expected_disbursement_date,
                    principal_amount: dp_money.clone(),
                    interest_amount: zero.clone(),
                    fee_charges_amount: zero.clone(),
                    penalty_charges_amount: zero.clone(),
                    total_installment_amount: dp_money,
                    principal_balance_outstanding: outstanding_balance.clone(),
                    is_down_payment: true,
                });

                installment_number += 1;
            }
        }

        // Principal remaining after down-payment to be amortized across `num_repayments`
        let amortizable_principal = outstanding_balance.clone();

        // 2. Generate repayment period date ranges (`from_date`, `due_date`)
        let period_dates = Self::generate_period_dates(
            terms.expected_disbursement_date,
            terms.repayments_starting_from_date,
            terms.repayment_frequency_type,
            repayment_every,
            num_repayments,
        );

        let principal_grace = terms.principal_grace_periods.min(num_repayments.saturating_sub(1));
        let interest_free_grace = terms.interest_free_grace_periods.min(num_repayments);
        let interest_payment_grace = terms
            .interest_payment_grace_periods
            .min(num_repayments.saturating_sub(1));

        match terms.interest_method {
            InterestMethod::Flat => Self::generate_flat_installments(
                terms,
                &currency,
                &zero,
                &amortizable_principal,
                &mut outstanding_balance,
                &period_dates,
                principal_grace,
                interest_free_grace,
                interest_payment_grace,
                &mut installment_number,
                &mut installments,
            ),
            InterestMethod::DecliningBalance => match terms.amortization_method {
                AmortizationMethod::EqualPrincipal => {
                    Self::generate_declining_equal_principal_installments(
                        terms,
                        &currency,
                        &zero,
                        &amortizable_principal,
                        &mut outstanding_balance,
                        &period_dates,
                        principal_grace,
                        interest_free_grace,
                        interest_payment_grace,
                        &mut installment_number,
                        &mut installments,
                    )
                }
                AmortizationMethod::EqualInstallments => {
                    Self::generate_declining_equal_installments(
                        terms,
                        &currency,
                        &zero,
                        &amortizable_principal,
                        &mut outstanding_balance,
                        &period_dates,
                        principal_grace,
                        interest_free_grace,
                        interest_payment_grace,
                        &mut installment_number,
                        &mut installments,
                    )
                }
            },
        }

        // Compute aggregate schedule totals
        let mut total_principal = zero.clone();
        let mut total_interest = zero.clone();
        let mut total_fees = zero.clone();
        let mut total_repayment = zero;

        for inst in &installments {
            total_principal = total_principal.plus_unchecked(&inst.principal_amount);
            total_interest = total_interest.plus_unchecked(&inst.interest_amount);
            total_fees = total_fees.plus_unchecked(&inst.fee_charges_amount);
            total_repayment = total_repayment.plus_unchecked(&inst.total_installment_amount);
        }

        GeneratedLoanSchedule {
            currency,
            disbursement_date: terms.expected_disbursement_date,
            total_principal,
            total_interest,
            total_fees,
            total_repayment,
            installments,
        }
    }

    /// Generates `(from_date, due_date)` pairs for `number_of_repayments` periods.
    ///
    /// Preserves the seed day-of-month so month-end dates (e.g., Jan 31 -> Feb 28 -> Mar 31)
    /// do not drift across short months.
    pub fn generate_period_dates(
        expected_disbursement_date: NaiveDate,
        repayments_starting_from_date: Option<NaiveDate>,
        frequency_type: CalendarFrequencyType,
        repayment_every: u32,
        number_of_repayments: u32,
    ) -> Vec<(NaiveDate, NaiveDate)> {
        let mut periods = Vec::with_capacity(number_of_repayments as usize);
        if number_of_repayments == 0 {
            return periods;
        }

        let interval = (repayment_every.max(1)) as i32;
        let first_due_date = match repayments_starting_from_date {
            Some(start_date) if start_date > expected_disbursement_date => start_date,
            _ => CalendarUtils::add_period(
                expected_disbursement_date,
                frequency_type,
                interval,
                Some(expected_disbursement_date.day()),
            ),
        };

        let seed_day = match repayments_starting_from_date {
            Some(start_date) if start_date > expected_disbursement_date => Some(start_date.day()),
            _ => Some(expected_disbursement_date.day()),
        };

        let mut from_date = expected_disbursement_date;
        let mut due_date = first_due_date;

        for period_idx in 0..number_of_repayments {
            periods.push((from_date, due_date));
            from_date = due_date;
            if period_idx + 1 < number_of_repayments {
                due_date =
                    CalendarUtils::add_period(from_date, frequency_type, interval, seed_day);
            }
        }

        periods
    }

    /// Calculates the periodic interest rate factor for a single period `(from_date, due_date)`.
    ///
    /// Supports:
    /// - `Actual/365`, `Actual/360`, `Actual/364`, `Actual/Actual`
    /// - `30/360`, `30/365`, `30/364`, `30/Actual`
    /// - `InterestCalculationPeriodMethod::Daily` vs `SameAsRepaymentPeriod`
    pub fn calculate_period_interest_rate(
        terms: &LoanScheduleTerms,
        from_date: NaiveDate,
        due_date: NaiveDate,
    ) -> Decimal {
        if terms.annual_nominal_interest_rate <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let annual_rate_fraction = Self::round_high_precision(
            terms.annual_nominal_interest_rate / dec!(100),
        );

        let repayment_every = Decimal::from(terms.repayment_every.max(1));

        // Standard periodic rate when days_in_month == 30 and days_in_year == 360 for monthly
        // or when SameAsRepaymentPeriod is used with standard calendar intervals:
        let days_in_year = Decimal::from(terms.days_in_year_type.days_in_year_for(from_date.year()));

        match terms.interest_calculation_period_method {
            InterestCalculationPeriodMethod::Daily => {
                let days_in_period = Self::calculate_days_in_period(
                    terms.days_in_month_type,
                    from_date,
                    due_date,
                    terms.repayment_frequency_type,
                    terms.repayment_every,
                );
                Self::round_high_precision(annual_rate_fraction * days_in_period / days_in_year)
            }
            InterestCalculationPeriodMethod::SameAsRepaymentPeriod => {
                match terms.repayment_frequency_type {
                    CalendarFrequencyType::Monthly => {
                        match (terms.days_in_month_type, terms.days_in_year_type) {
                            (DaysInMonthType::Days30, DaysInYearType::Days360) => {
                                // 30/360: each month is 30/360 = 1/12 of a year
                                Self::round_high_precision(
                                    annual_rate_fraction * repayment_every / dec!(12),
                                )
                            }
                            (DaysInMonthType::Days30, _) => {
                                let days = Decimal::from(30u32) * repayment_every;
                                Self::round_high_precision(
                                    annual_rate_fraction * days / days_in_year,
                                )
                            }
                            (DaysInMonthType::Actual, DaysInYearType::Actual) => {
                                // In Fineract SameAsRepaymentPeriod with Actual/Actual on whole months,
                                // if the first period is an exact period, periodic rate is 1/12 * every;
                                // if daily/actual days are used across varying days-in-year:
                                let expected_from = CalendarUtils::add_period(
                                    due_date,
                                    CalendarFrequencyType::Monthly,
                                    -(terms.repayment_every.max(1) as i32),
                                    None,
                                );
                                if expected_from == from_date {
                                    Self::round_high_precision(
                                        annual_rate_fraction * repayment_every / dec!(12),
                                    )
                                } else {
                                    let actual_days = Decimal::from(
                                        CalendarUtils::days_between(from_date, due_date).max(0),
                                    );
                                    Self::round_high_precision(
                                        annual_rate_fraction * actual_days / days_in_year,
                                    )
                                }
                            }
                            (DaysInMonthType::Actual, _) => {
                                // Actual/365, Actual/360, Actual/364 with SameAsRepaymentPeriod:
                                // Use actual days in period divided by days_in_year (365/360/364)
                                let actual_days = Decimal::from(
                                    CalendarUtils::days_between(from_date, due_date).max(0),
                                );
                                Self::round_high_precision(
                                    annual_rate_fraction * actual_days / days_in_year,
                                )
                            }
                        }
                    }
                    CalendarFrequencyType::Weekly => {
                        let actual_days = Decimal::from(
                            CalendarUtils::days_between(from_date, due_date).max(0),
                        );
                        if matches!(terms.days_in_year_type, DaysInYearType::Actual)
                            && actual_days == Decimal::from(7u32) * repayment_every
                        {
                            Self::round_high_precision(
                                annual_rate_fraction * repayment_every / dec!(52),
                            )
                        } else {
                            Self::round_high_precision(
                                annual_rate_fraction * actual_days / days_in_year,
                            )
                        }
                    }
                    CalendarFrequencyType::Daily => {
                        let actual_days = Decimal::from(
                            CalendarUtils::days_between(from_date, due_date).max(0),
                        );
                        Self::round_high_precision(
                            annual_rate_fraction * actual_days / days_in_year,
                        )
                    }
                    CalendarFrequencyType::Yearly => {
                        Self::round_high_precision(annual_rate_fraction * repayment_every)
                    }
                    CalendarFrequencyType::Invalid => Decimal::ZERO,
                }
            }
        }
    }

    /// Calculates the nominal periodic rate used for constant-EMI PMT calculation.
    ///
    /// Matches Fineract's `DefaultPaymentPeriodsInOneYearCalculator` and `ProgressiveEMICalculator`.
    pub fn calculate_nominal_rate_for_pmt(terms: &LoanScheduleTerms) -> Decimal {
        if terms.annual_nominal_interest_rate <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let annual_rate_fraction = Self::round_high_precision(
            terms.annual_nominal_interest_rate / dec!(100),
        );
        let repayment_every = Decimal::from(terms.repayment_every.max(1));

        match terms.repayment_frequency_type {
            CalendarFrequencyType::Monthly => match (terms.days_in_month_type, terms.days_in_year_type) {
                (DaysInMonthType::Days30, DaysInYearType::Days360)
                | (DaysInMonthType::Actual, DaysInYearType::Actual) => {
                    Self::round_high_precision(annual_rate_fraction * repayment_every / dec!(12))
                }
                (DaysInMonthType::Days30, year_type) => {
                    let diy = Decimal::from(year_type.days_in_year_for(
                        terms.expected_disbursement_date.year(),
                    ));
                    Self::round_high_precision(
                        annual_rate_fraction * dec!(30) * repayment_every / diy,
                    )
                }
                (DaysInMonthType::Actual, year_type) => {
                    // For Actual/365 or Actual/360 monthly loans, Fineract's PaymentPeriodsInOneYearCalculator
                    // uses 12 periods per year for the initial PMT estimate (or 30.4167/diy)
                    let diy = Decimal::from(year_type.days_in_year_for(
                        terms.expected_disbursement_date.year(),
                    ));
                    if matches!(
                        terms.interest_calculation_period_method,
                        InterestCalculationPeriodMethod::SameAsRepaymentPeriod
                    ) && diy == dec!(365)
                    {
                        Self::round_high_precision(annual_rate_fraction * repayment_every / dec!(12))
                    } else {
                        Self::round_high_precision(annual_rate_fraction * repayment_every / dec!(12))
                    }
                }
            },
            CalendarFrequencyType::Weekly => {
                Self::round_high_precision(annual_rate_fraction * repayment_every / dec!(52))
            }
            CalendarFrequencyType::Daily => {
                let diy = Decimal::from(
                    terms
                        .days_in_year_type
                        .days_in_year_for(terms.expected_disbursement_date.year()),
                );
                Self::round_high_precision(annual_rate_fraction * repayment_every / diy)
            }
            CalendarFrequencyType::Yearly => {
                Self::round_high_precision(annual_rate_fraction * repayment_every)
            }
            CalendarFrequencyType::Invalid => Decimal::ZERO,
        }
    }

    /// Pure deterministic PMT (Equal Monthly/Periodic Installment) solver using `Decimal` arithmetic:
    ///
    /// $$\text{PMT} = \frac{P \cdot r \cdot (1 + r)^n}{(1 + r)^n - 1}$$
    ///
    /// When $r = 0$, returns $P / n$.
    /// Port of `FinanicalFunctions.pmt` and `ProgressiveEMICalculator.calculateEMIValue`.
    pub fn calculate_pmt(
        principal: Decimal,
        periodic_rate: Decimal,
        number_of_periods: u32,
    ) -> Decimal {
        if number_of_periods == 0 || principal <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let n_dec = Decimal::from(number_of_periods);
        if periodic_rate <= Decimal::ZERO {
            return Self::round_high_precision(principal / n_dec);
        }

        let one_plus_r = Decimal::ONE + periodic_rate;
        let factor = Self::decimal_pow(one_plus_r, number_of_periods);
        let denominator = factor - Decimal::ONE;
        if denominator <= Decimal::ZERO {
            return Self::round_high_precision(principal / n_dec);
        }

        let r_round = Self::round_high_precision(periodic_rate);
        let factor_round = Self::round_high_precision(factor);
        let numerator = Self::round_high_precision(principal * r_round) * factor_round;
        Self::round_high_precision(numerator / denominator)
    }

    /// Computes `base^exp` using binary exponentiation with high-precision rounding at each step
    /// to prevent 96-bit `Decimal` mantissa overflow for large `exp`.
    pub fn decimal_pow(base: Decimal, exp: u32) -> Decimal {
        if exp == 0 {
            return Decimal::ONE;
        }
        let mut result = Decimal::ONE;
        let mut current_base = Self::round_high_precision(base);
        let mut remaining = exp;

        while remaining > 0 {
            if remaining & 1 == 1 {
                result = Self::round_high_precision(result * current_base);
            }
            remaining >>= 1;
            if remaining > 0 {
                current_base = Self::round_high_precision(current_base * current_base);
            }
        }
        result
    }

    #[inline]
    fn round_high_precision(val: Decimal) -> Decimal {
        val.round_dp_with_strategy(HIGH_PRECISION_SCALE, RoundingStrategy::MidpointAwayFromZero)
    }

    fn calculate_days_in_period(
        days_in_month_type: DaysInMonthType,
        from_date: NaiveDate,
        due_date: NaiveDate,
        frequency_type: CalendarFrequencyType,
        repayment_every: u32,
    ) -> Decimal {
        match days_in_month_type {
            DaysInMonthType::Actual => {
                Decimal::from(CalendarUtils::days_between(from_date, due_date).max(0))
            }
            DaysInMonthType::Days30 => {
                if matches!(frequency_type, CalendarFrequencyType::Monthly) {
                    Decimal::from(30u32 * repayment_every.max(1))
                } else {
                    // 30/360 US/NASD day count between two dates
                    let y1 = from_date.year();
                    let m1 = from_date.month() as i32;
                    let d1 = from_date.day().min(30) as i32;
                    let y2 = due_date.year();
                    let m2 = due_date.month() as i32;
                    let d2 = if d1 == 30 && due_date.day() == 31 {
                        30
                    } else {
                        due_date.day() as i32
                    };
                    let days = 360 * (y2 - y1) + 30 * (m2 - m1) + (d2 - d1);
                    Decimal::from(days.max(0))
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_flat_installments(
        terms: &LoanScheduleTerms,
        currency: &MonetaryCurrency,
        zero: &Money,
        amortizable_principal: &Money,
        outstanding_balance: &mut Money,
        period_dates: &[(NaiveDate, NaiveDate)],
        principal_grace: u32,
        interest_free_grace: u32,
        interest_payment_grace: u32,
        installment_number: &mut u32,
        installments: &mut Vec<ScheduleInstallment>,
    ) {
        let num_repayments = period_dates.len() as u32;
        let principal_periods = (num_repayments - principal_grace).max(1);
        let equal_principal = Money::of(
            currency.clone(),
            amortizable_principal.amount() / Decimal::from(principal_periods),
            FineractRoundingMode::HalfUp,
        );

        let mut deferred_interest = zero.clone();

        for (idx, &(from_date, due_date)) in period_dates.iter().enumerate() {
            let period_num = (idx as u32) + 1;
            let is_last = period_num == num_repayments;

            // Principal for this installment
            let principal_amount = if is_last {
                outstanding_balance.clone()
            } else if period_num <= principal_grace {
                zero.clone()
            } else {
                equal_principal.clone().min(outstanding_balance.clone())
            };

            // Flat interest is calculated on the initial amortizable principal
            let period_interest = if period_num <= interest_free_grace {
                zero.clone()
            } else {
                let rate = Self::calculate_period_interest_rate(terms, from_date, due_date);
                Money::of(currency.clone(), amortizable_principal.amount() * rate, FineractRoundingMode::HalfUp)
            };

            let interest_amount = if period_num <= interest_payment_grace && !is_last {
                deferred_interest = deferred_interest.plus_unchecked(&period_interest);
                zero.clone()
            } else {
                let total_int = period_interest.plus_unchecked(&deferred_interest);
                deferred_interest = zero.clone();
                total_int
            };

            *outstanding_balance = outstanding_balance.minus_unchecked(&principal_amount);
            let total_installment_amount = principal_amount.plus_unchecked(&interest_amount);

            installments.push(ScheduleInstallment {
                installment_number: *installment_number,
                from_date,
                due_date,
                principal_amount,
                interest_amount,
                fee_charges_amount: zero.clone(),
                penalty_charges_amount: zero.clone(),
                total_installment_amount,
                principal_balance_outstanding: outstanding_balance.clone(),
                is_down_payment: false,
            });

            *installment_number += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_declining_equal_principal_installments(
        terms: &LoanScheduleTerms,
        currency: &MonetaryCurrency,
        zero: &Money,
        amortizable_principal: &Money,
        outstanding_balance: &mut Money,
        period_dates: &[(NaiveDate, NaiveDate)],
        principal_grace: u32,
        interest_free_grace: u32,
        interest_payment_grace: u32,
        installment_number: &mut u32,
        installments: &mut Vec<ScheduleInstallment>,
    ) {
        let num_repayments = period_dates.len() as u32;
        let principal_periods = (num_repayments - principal_grace).max(1);
        let equal_principal = Money::of(
            currency.clone(),
            amortizable_principal.amount() / Decimal::from(principal_periods),
            FineractRoundingMode::HalfUp,
        );

        let mut deferred_interest = zero.clone();

        for (idx, &(from_date, due_date)) in period_dates.iter().enumerate() {
            let period_num = (idx as u32) + 1;
            let is_last = period_num == num_repayments;

            // Interest is calculated on current declining outstanding balance before principal deduction
            let period_interest = if period_num <= interest_free_grace {
                zero.clone()
            } else {
                let rate = Self::calculate_period_interest_rate(terms, from_date, due_date);
                Money::of(currency.clone(), outstanding_balance.amount() * rate, FineractRoundingMode::HalfUp)
            };

            let interest_amount = if period_num <= interest_payment_grace && !is_last {
                deferred_interest = deferred_interest.plus_unchecked(&period_interest);
                zero.clone()
            } else {
                let total_int = period_interest.plus_unchecked(&deferred_interest);
                deferred_interest = zero.clone();
                total_int
            };

            let principal_amount = if is_last {
                outstanding_balance.clone()
            } else if period_num <= principal_grace {
                zero.clone()
            } else {
                equal_principal.clone().min(outstanding_balance.clone())
            };

            *outstanding_balance = outstanding_balance.minus_unchecked(&principal_amount);
            let total_installment_amount = principal_amount.plus_unchecked(&interest_amount);

            installments.push(ScheduleInstallment {
                installment_number: *installment_number,
                from_date,
                due_date,
                principal_amount,
                interest_amount,
                fee_charges_amount: zero.clone(),
                penalty_charges_amount: zero.clone(),
                total_installment_amount,
                principal_balance_outstanding: outstanding_balance.clone(),
                is_down_payment: false,
            });

            *installment_number += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_declining_equal_installments(
        terms: &LoanScheduleTerms,
        currency: &MonetaryCurrency,
        zero: &Money,
        amortizable_principal: &Money,
        outstanding_balance: &mut Money,
        period_dates: &[(NaiveDate, NaiveDate)],
        principal_grace: u32,
        interest_free_grace: u32,
        interest_payment_grace: u32,
        installment_number: &mut u32,
        installments: &mut Vec<ScheduleInstallment>,
    ) {
        let num_repayments = period_dates.len() as u32;
        let amortizing_periods = (num_repayments - principal_grace).max(1);

        let pmt_rate = Self::calculate_nominal_rate_for_pmt(terms);
        let raw_emi = match terms.fixed_emi_amount {
            Some(fixed_emi) if fixed_emi > Decimal::ZERO => fixed_emi,
            _ => Self::calculate_pmt(amortizable_principal.amount(), pmt_rate, amortizing_periods),
        };
        let fixed_emi = Money::of(currency.clone(), raw_emi, FineractRoundingMode::HalfUp);

        let mut deferred_interest = zero.clone();

        for (idx, &(from_date, due_date)) in period_dates.iter().enumerate() {
            let period_num = (idx as u32) + 1;
            let is_last = period_num == num_repayments;

            // Calculate interest on declining outstanding principal balance
            let period_interest = if period_num <= interest_free_grace {
                zero.clone()
            } else {
                let rate = Self::calculate_period_interest_rate(terms, from_date, due_date);
                Money::of(currency.clone(), outstanding_balance.amount() * rate, FineractRoundingMode::HalfUp)
            };

            let interest_amount = if period_num <= interest_payment_grace && !is_last {
                deferred_interest = deferred_interest.plus_unchecked(&period_interest);
                zero.clone()
            } else {
                let total_int = period_interest.plus_unchecked(&deferred_interest);
                deferred_interest = zero.clone();
                total_int
            };

            let principal_amount = if is_last {
                // Final installment absorbs any rounding discrepancy so sum(principal) == loan_principal
                outstanding_balance.clone()
            } else if period_num <= principal_grace {
                // Principal moratorium: only interest is paid during grace periods
                zero.clone()
            } else {
                // Standard EMI period: principal = max(0, min(outstanding, emi - interest))
                if fixed_emi.amount() > interest_amount.amount() {
                    let candidate = fixed_emi.minus_unchecked(&interest_amount);
                    if candidate.amount() > outstanding_balance.amount() {
                        outstanding_balance.clone()
                    } else {
                        candidate
                    }
                } else {
                    zero.clone()
                }
            };

            *outstanding_balance = outstanding_balance.minus_unchecked(&principal_amount);
            let total_installment_amount = principal_amount.plus_unchecked(&interest_amount);

            installments.push(ScheduleInstallment {
                installment_number: *installment_number,
                from_date,
                due_date,
                principal_amount,
                interest_amount,
                fee_charges_amount: zero.clone(),
                penalty_charges_amount: zero.clone(),
                total_installment_amount,
                principal_balance_outstanding: outstanding_balance.clone(),
                is_down_payment: false,
            });

            *installment_number += 1;
        }
    }
}

/// Annual Percentage Rate (APR) calculator helper.
///
/// Port of `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.AprCalculator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AprCalculator;

impl AprCalculator {
    /// Calculates the nominal annual interest rate from a periodic rate and frequency.
    pub fn calculate_from(
        frequency_type: CalendarFrequencyType,
        interest_rate_per_period: Decimal,
    ) -> Decimal {
        match frequency_type {
            CalendarFrequencyType::Daily => (interest_rate_per_period * dec!(365))
                .round_with_mode(8, FineractRoundingMode::HalfUp),
            CalendarFrequencyType::Weekly => (interest_rate_per_period * dec!(52))
                .round_with_mode(8, FineractRoundingMode::HalfUp),
            CalendarFrequencyType::Monthly => (interest_rate_per_period * dec!(12))
                .round_with_mode(8, FineractRoundingMode::HalfUp),
            CalendarFrequencyType::Yearly => {
                interest_rate_per_period.round_with_mode(8, FineractRoundingMode::HalfUp)
            }
            CalendarFrequencyType::Invalid => Decimal::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[test]
    fn test_upstream_progressive_schedule_no_downpayment() {
        // Exact test vector from Fineract's EmbeddableProgressiveLoanScheduleGeneratorTest:
        // $100.00 principal, 6 monthly repayments, 7.0% nominal annual interest, 30/360,
        // disbursed on 2024-01-01.
        let usd = MonetaryCurrency::usd();
        let terms = LoanScheduleTerms {
            principal: Money::of(usd.clone(), dec!(100.00), FineractRoundingMode::HalfUp),
            annual_nominal_interest_rate: dec!(7.0),
            number_of_repayments: 6,
            repayment_every: 1,
            repayment_frequency_type: CalendarFrequencyType::Monthly,
            amortization_method: AmortizationMethod::EqualInstallments,
            interest_method: InterestMethod::DecliningBalance,
            interest_calculation_period_method:
                InterestCalculationPeriodMethod::SameAsRepaymentPeriod,
            days_in_month_type: DaysInMonthType::Days30,
            days_in_year_type: DaysInYearType::Days360,
            expected_disbursement_date: date(2024, 1, 1),
            repayments_starting_from_date: None,
            principal_grace_periods: 0,
            interest_payment_grace_periods: 0,
            interest_free_grace_periods: 0,
            fixed_emi_amount: None,
            down_payment_percentage: None,
        };

        let generator = EmbeddableScheduleGenerator::new();
        let schedule = generator.generate(&terms);

        assert_eq!(schedule.installments.len(), 6);
        assert_eq!(schedule.total_principal.amount(), dec!(100.00));
        assert_eq!(schedule.total_interest.amount(), dec!(2.05));
        assert_eq!(schedule.total_repayment.amount(), dec!(102.05));

        let expected = [
            (1, date(2024, 1, 1), date(2024, 2, 1), dec!(16.43), dec!(0.58), dec!(17.01), dec!(83.57)),
            (2, date(2024, 2, 1), date(2024, 3, 1), dec!(16.52), dec!(0.49), dec!(17.01), dec!(67.05)),
            (3, date(2024, 3, 1), date(2024, 4, 1), dec!(16.62), dec!(0.39), dec!(17.01), dec!(50.43)),
            (4, date(2024, 4, 1), date(2024, 5, 1), dec!(16.72), dec!(0.29), dec!(17.01), dec!(33.71)),
            (5, date(2024, 5, 1), date(2024, 6, 1), dec!(16.81), dec!(0.20), dec!(17.01), dec!(16.90)),
            (6, date(2024, 6, 1), date(2024, 7, 1), dec!(16.90), dec!(0.10), dec!(17.00), dec!(0.00)),
        ];

        for (i, &(num, from, due, prin, int, total, out)) in expected.iter().enumerate() {
            let inst = &schedule.installments[i];
            assert_eq!(inst.installment_number, num);
            assert_eq!(inst.from_date, from);
            assert_eq!(inst.due_date, due);
            assert_eq!(inst.principal_amount.amount(), prin);
            assert_eq!(inst.interest_amount.amount(), int);
            assert_eq!(inst.total_installment_amount.amount(), total);
            assert_eq!(inst.principal_balance_outstanding.amount(), out);
            assert!(!inst.is_down_payment);
        }
    }

    #[test]
    fn test_upstream_progressive_schedule_with_25_percent_downpayment() {
        // Exact test vector from EmbeddableProgressiveLoanScheduleGeneratorTest:
        // $100.00 principal, 25% down payment on 2024-01-01 ($25.00), remaining $75.00 over 6 months at 7.0%
        let usd = MonetaryCurrency::usd();
        let terms = LoanScheduleTerms {
            principal: Money::of(usd, dec!(100.00), FineractRoundingMode::HalfUp),
            annual_nominal_interest_rate: dec!(7.0),
            number_of_repayments: 6,
            repayment_every: 1,
            repayment_frequency_type: CalendarFrequencyType::Monthly,
            amortization_method: AmortizationMethod::EqualInstallments,
            interest_method: InterestMethod::DecliningBalance,
            interest_calculation_period_method:
                InterestCalculationPeriodMethod::SameAsRepaymentPeriod,
            days_in_month_type: DaysInMonthType::Days30,
            days_in_year_type: DaysInYearType::Days360,
            expected_disbursement_date: date(2024, 1, 1),
            repayments_starting_from_date: None,
            principal_grace_periods: 0,
            interest_payment_grace_periods: 0,
            interest_free_grace_periods: 0,
            fixed_emi_amount: None,
            down_payment_percentage: Some(dec!(25.0)),
        };

        let schedule = EmbeddableScheduleGenerator::generate_schedule(&terms);
        assert_eq!(schedule.installments.len(), 7);
        assert_eq!(schedule.total_principal.amount(), dec!(100.00));
        assert_eq!(schedule.total_interest.amount(), dec!(1.54));
        assert_eq!(schedule.total_repayment.amount(), dec!(101.54));

        // Installment #1 is down payment on disbursement date
        let dp = &schedule.installments[0];
        assert_eq!(dp.installment_number, 1);
        assert!(dp.is_down_payment);
        assert_eq!(dp.from_date, date(2024, 1, 1));
        assert_eq!(dp.due_date, date(2024, 1, 1));
        assert_eq!(dp.principal_amount.amount(), dec!(25.00));
        assert_eq!(dp.interest_amount.amount(), dec!(0.00));
        assert_eq!(dp.principal_balance_outstanding.amount(), dec!(75.00));

        // Final installment #7 brings outstanding balance to 0.00
        let last = schedule.installments.last().unwrap();
        assert_eq!(last.installment_number, 7);
        assert_eq!(last.principal_balance_outstanding.amount(), dec!(0.00));
    }

    #[test]
    fn test_flat_interest_schedule() {
        let usd = MonetaryCurrency::usd();
        let terms = LoanScheduleTerms {
            principal: Money::of(usd, dec!(1200.00), FineractRoundingMode::HalfUp),
            annual_nominal_interest_rate: dec!(12.0),
            number_of_repayments: 12,
            repayment_every: 1,
            repayment_frequency_type: CalendarFrequencyType::Monthly,
            amortization_method: AmortizationMethod::EqualPrincipal,
            interest_method: InterestMethod::Flat,
            interest_calculation_period_method:
                InterestCalculationPeriodMethod::SameAsRepaymentPeriod,
            days_in_month_type: DaysInMonthType::Days30,
            days_in_year_type: DaysInYearType::Days360,
            expected_disbursement_date: date(2024, 1, 1),
            repayments_starting_from_date: None,
            principal_grace_periods: 0,
            interest_payment_grace_periods: 0,
            interest_free_grace_periods: 0,
            fixed_emi_amount: None,
            down_payment_percentage: None,
        };

        let schedule = EmbeddableScheduleGenerator::generate_schedule(&terms);
        assert_eq!(schedule.installments.len(), 12);
        assert_eq!(schedule.total_principal.amount(), dec!(1200.00));
        // 12% of $1200 for 1 year = $144.00 ($12.00 per month)
        assert_eq!(schedule.total_interest.amount(), dec!(144.00));
        assert_eq!(schedule.total_repayment.amount(), dec!(1344.00));

        for inst in &schedule.installments {
            assert_eq!(inst.principal_amount.amount(), dec!(100.00));
            assert_eq!(inst.interest_amount.amount(), dec!(12.00));
            assert_eq!(inst.total_installment_amount.amount(), dec!(112.00));
        }
    }

    #[test]
    fn test_declining_balance_equal_principal_and_moratorium() {
        let usd = MonetaryCurrency::usd();
        let terms = LoanScheduleTerms {
            principal: Money::of(usd, dec!(1000.00), FineractRoundingMode::HalfUp),
            annual_nominal_interest_rate: dec!(12.0),
            number_of_repayments: 5,
            repayment_every: 1,
            repayment_frequency_type: CalendarFrequencyType::Monthly,
            amortization_method: AmortizationMethod::EqualPrincipal,
            interest_method: InterestMethod::DecliningBalance,
            interest_calculation_period_method:
                InterestCalculationPeriodMethod::SameAsRepaymentPeriod,
            days_in_month_type: DaysInMonthType::Days30,
            days_in_year_type: DaysInYearType::Days360,
            expected_disbursement_date: date(2024, 1, 1),
            repayments_starting_from_date: None,
            principal_grace_periods: 1,
            interest_payment_grace_periods: 0,
            interest_free_grace_periods: 1,
            fixed_emi_amount: None,
            down_payment_percentage: None,
        };

        let schedule = EmbeddableScheduleGenerator::generate_schedule(&terms);
        assert_eq!(schedule.installments.len(), 5);
        assert_eq!(schedule.total_principal.amount(), dec!(1000.00));

        // Period 1 has both principal grace (1) and interest-free grace (1)
        assert_eq!(schedule.installments[0].principal_amount.amount(), dec!(0.00));
        assert_eq!(schedule.installments[0].interest_amount.amount(), dec!(0.00));
        assert_eq!(
            schedule.installments[0].principal_balance_outstanding.amount(),
            dec!(1000.00)
        );

        // Remaining 4 periods each amortize $250.00 principal
        for i in 1..5 {
            assert_eq!(schedule.installments[i].principal_amount.amount(), dec!(250.00));
        }
        assert_eq!(
            schedule.installments[4].principal_balance_outstanding.amount(),
            dec!(0.00)
        );
    }

    #[test]
    fn test_day_count_conventions_actual_365_and_actual_360() {
        let usd = MonetaryCurrency::usd();
        // Jan 1, 2024 to Feb 1, 2024 is 31 days
        let mut terms = LoanScheduleTerms::new(
            Money::of(usd, dec!(10000.00), FineractRoundingMode::HalfUp),
            dec!(10.0),
            3,
            1,
            CalendarFrequencyType::Monthly,
            date(2024, 1, 1),
        );
        terms.interest_calculation_period_method = InterestCalculationPeriodMethod::Daily;
        terms.days_in_month_type = DaysInMonthType::Actual;
        terms.days_in_year_type = DaysInYearType::Days365;

        let schedule_365 = EmbeddableScheduleGenerator::generate_schedule(&terms);
        // Period 1 interest: 10000 * 0.10 * 31 / 365 = 84.9315... -> $84.93
        assert_eq!(schedule_365.installments[0].interest_amount.amount(), dec!(84.93));
        assert_eq!(schedule_365.total_principal.amount(), dec!(10000.00));

        terms.days_in_year_type = DaysInYearType::Days360;
        let schedule_360 = EmbeddableScheduleGenerator::generate_schedule(&terms);
        // Period 1 interest: 10000 * 0.10 * 31 / 360 = 86.1111... -> $86.11
        assert_eq!(schedule_360.installments[0].interest_amount.amount(), dec!(86.11));
        assert_eq!(schedule_360.total_principal.amount(), dec!(10000.00));
    }

    proptest! {
        #[test]
        fn prop_sum_of_installment_principals_equals_loan_principal(
            principal_cents in 100i64..100_000_000i64,
            annual_rate_bps in 0i64..5000i64,
            num_repayments in 1u32..60u32,
            repayment_every in 1u32..4u32,
            amort_idx in 0usize..2usize,
            interest_idx in 0usize..2usize,
            calc_period_idx in 0usize..2usize,
            dim_idx in 0usize..2usize,
            diy_idx in 0usize..4usize,
            principal_grace in 0u32..5u32,
            interest_free_grace in 0u32..5u32,
            interest_payment_grace in 0u32..5u32,
            has_down_payment in any::<bool>(),
            down_payment_bps in 100i64..5000i64,
        ) {
            let usd = MonetaryCurrency::usd();
            let principal = Money::of(usd, Decimal::new(principal_cents, 2), FineractRoundingMode::HalfUp);
            let annual_rate = Decimal::new(annual_rate_bps, 2);

            let amortization_method = match amort_idx {
                0 => AmortizationMethod::EqualPrincipal,
                _ => AmortizationMethod::EqualInstallments,
            };
            let interest_method = match interest_idx {
                0 => InterestMethod::DecliningBalance,
                _ => InterestMethod::Flat,
            };
            let interest_calculation_period_method = match calc_period_idx {
                0 => InterestCalculationPeriodMethod::Daily,
                _ => InterestCalculationPeriodMethod::SameAsRepaymentPeriod,
            };
            let days_in_month_type = match dim_idx {
                0 => DaysInMonthType::Actual,
                _ => DaysInMonthType::Days30,
            };
            let days_in_year_type = match diy_idx {
                0 => DaysInYearType::Actual,
                1 => DaysInYearType::Days360,
                2 => DaysInYearType::Days364,
                _ => DaysInYearType::Days365,
            };
            let down_payment_percentage = if has_down_payment {
                Some(Decimal::new(down_payment_bps, 2))
            } else {
                None
            };

            let terms = LoanScheduleTerms {
                principal: principal.clone(),
                annual_nominal_interest_rate: annual_rate,
                number_of_repayments: num_repayments,
                repayment_every,
                repayment_frequency_type: CalendarFrequencyType::Monthly,
                amortization_method,
                interest_method,
                interest_calculation_period_method,
                days_in_month_type,
                days_in_year_type,
                expected_disbursement_date: date(2024, 1, 15),
                repayments_starting_from_date: None,
                principal_grace_periods: principal_grace,
                interest_payment_grace_periods: interest_payment_grace,
                interest_free_grace_periods: interest_free_grace,
                fixed_emi_amount: None,
                down_payment_percentage,
            };

            let schedule = EmbeddableScheduleGenerator::generate_schedule(&terms);

            let sum_principal: Decimal = schedule
                .installments
                .iter()
                .map(|i| i.principal_amount.amount())
                .sum();

            prop_assert_eq!(sum_principal, principal.amount());
            prop_assert_eq!(schedule.total_principal.amount(), principal.amount());

            let last_outstanding = schedule
                .installments
                .last()
                .map(|i| i.principal_balance_outstanding.amount())
                .unwrap_or(Decimal::ZERO);
            prop_assert_eq!(last_outstanding, Decimal::ZERO);
        }
    }
}
