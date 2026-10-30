//! `fineract-charge-rs` — Loan, Savings, Client & Share Fee and Penalty Charge Calculation Engine.
//!
//! Ported from `fineract-charge` and `org.apache.fineract.portfolio.charge`:
//! - [`ChargeAppliesTo`] (`ChargeAppliesTo.java`)
//! - [`ChargeTimeType`] (`ChargeTimeType.java`)
//! - [`ChargeCalculationType`] (`ChargeCalculationType.java`)
//! - [`ChargePaymentMode`] (`ChargePaymentMode.java`)
//! - [`FeeFrequencyType`] (`PeriodFrequencyType.java` / `ChargeEnumerations.java`)
//! - [`Charge`] and [`ChargeData`] (`Charge.java`, `ChargeData.java`, matching `m_charge`)
//! - [`ChargeDefinitionValidator`] (`ChargeDefinitionCommandFromApiJsonDeserializer.java`)
//! - [`ChargeCalculationEngine`] (`LoanCharge.java`, `LoanChargeService.java`, `ChargeCalculationType.java`)
//! - [`ChargeRepository`], [`InMemoryChargeRepository`], [`DualDbChargeRepository`] (`m_charge` persistence)
//! - [`CreateChargeDefinitionCommandHandler`], [`UpdateChargeDefinitionCommandHandler`],
//!   [`DeleteChargeDefinitionCommandHandler`] (`ChargeWritePlatformServiceJpaRepositoryImpl.java`)

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use fineract_command_rs::pipeline::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::context::ExecutionContext;
use fineract_core_rs::db::{DialectSql, SqlDialect};
use fineract_core_rs::json_command::JsonCommand;
use fineract_money::{CurrencyData, Decimal, FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{
    ApiErrorArg, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException, PlatformDomainRuleException,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{AnyPool, Row};

// =============================================================================
// 0. EnumOptionData (Wire-compatible Fineract enumeration representation)
// =============================================================================

/// Standard Fineract `EnumOptionData` payload `{ id, code, value }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumOptionData {
    pub id: i64,
    pub code: String,
    pub value: String,
}

impl EnumOptionData {
    pub fn new(id: i64, code: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            id,
            code: code.into(),
            value: value.into(),
        }
    }
}

// =============================================================================
// 1. ChargeAppliesTo (ChargeAppliesTo.java)
// =============================================================================

/// Target portfolio entity to which a [`Charge`] definition applies (`charge_applies_to_enum`).
///
/// Ported from `org.apache.fineract.portfolio.charge.domain.ChargeAppliesTo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum ChargeAppliesTo {
    #[default]
    Invalid = 0,
    Loan = 1,
    Savings = 2,
    Client = 3,
    Shares = 4,
    WorkingCapitalLoan = 5,
}

impl ChargeAppliesTo {
    pub const INVALID: Self = Self::Invalid;
    pub const LOAN: Self = Self::Loan;
    pub const SAVINGS: Self = Self::Savings;
    pub const CLIENT: Self = Self::Client;
    pub const SHARES: Self = Self::Shares;
    pub const WORKING_CAPITAL_LOAN: Self = Self::WorkingCapitalLoan;

    /// Valid values supported by `ChargeAppliesTo.validValues()` in `ChargeDefinitionCommandFromApiJsonDeserializer`.
    pub const VALID_VALUES: &'static [i64] = &[1, 2, 3, 4];

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
            Self::Invalid => "chargeAppliesTo.invalid",
            Self::Loan => "chargeAppliesTo.loan",
            Self::Savings => "chargeAppliesTo.savings",
            Self::Client => "chargeAppliesTo.client",
            Self::Shares => "chargeAppliesTo.shares",
            Self::WorkingCapitalLoan => "chargeAppliesTo.workingCapitalLoan",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Loan => "Loan",
            Self::Savings => "Savings",
            Self::Client => "Client",
            Self::Shares => "Shares",
            Self::WorkingCapitalLoan => "Working Capital Loan",
        }
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Loan,
            2 => Self::Savings,
            3 => Self::Client,
            4 => Self::Shares,
            5 => Self::WorkingCapitalLoan,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Loan,
            Some(2) => Self::Savings,
            Some(3) => Self::Client,
            Some(4) => Self::Shares,
            Some(5) => Self::WorkingCapitalLoan,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn valid_values() -> &'static [i64] {
        Self::VALID_VALUES
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_loan_charge(self) -> bool {
        matches!(self, Self::Loan)
    }

    #[inline]
    pub const fn is_savings_charge(self) -> bool {
        matches!(self, Self::Savings)
    }

    #[inline]
    pub const fn is_client_charge(self) -> bool {
        matches!(self, Self::Client)
    }

    #[inline]
    pub const fn is_shares_charge(self) -> bool {
        matches!(self, Self::Shares)
    }

    #[inline]
    pub const fn is_working_capital_loan_charge(self) -> bool {
        matches!(self, Self::WorkingCapitalLoan)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

// =============================================================================
// 2. ChargeTimeType (ChargeTimeType.java)
// =============================================================================

/// Lifecycle event or schedule trigger when a [`Charge`] becomes due (`charge_time_enum`).
///
/// Ported from `org.apache.fineract.portfolio.charge.domain.ChargeTimeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum ChargeTimeType {
    #[default]
    Invalid = 0,
    Disbursement = 1,
    SpecifiedDueDate = 2,
    SavingsActivation = 3,
    SavingsClosure = 4,
    WithdrawalFee = 5,
    AnnualFee = 6,
    MonthlyFee = 7,
    InstallmentFee = 8,
    OverdueInstallment = 9,
    OverdraftFee = 10,
    WeeklyFee = 11,
    TrancheDisbursement = 12,
    ShareAccountActivate = 13,
    SharePurchase = 14,
    ShareRedeem = 15,
    SavingsNoActivityFee = 16,
    LineOfCreditActivation = 17,
}

impl ChargeTimeType {
    pub const INVALID: Self = Self::Invalid;
    pub const DISBURSEMENT: Self = Self::Disbursement;
    pub const SPECIFIED_DUE_DATE: Self = Self::SpecifiedDueDate;
    pub const SAVINGS_ACTIVATION: Self = Self::SavingsActivation;
    pub const SAVINGS_CLOSURE: Self = Self::SavingsClosure;
    pub const WITHDRAWAL_FEE: Self = Self::WithdrawalFee;
    pub const ANNUAL_FEE: Self = Self::AnnualFee;
    pub const MONTHLY_FEE: Self = Self::MonthlyFee;
    pub const INSTALLMENT_FEE: Self = Self::InstallmentFee;
    pub const OVERDUE_INSTALLMENT: Self = Self::OverdueInstallment;
    pub const OVERDRAFT_FEE: Self = Self::OverdraftFee;
    pub const WEEKLY_FEE: Self = Self::WeeklyFee;
    pub const TRANCHE_DISBURSEMENT: Self = Self::TrancheDisbursement;
    pub const SHAREACCOUNT_ACTIVATE: Self = Self::ShareAccountActivate;
    pub const SHARE_PURCHASE: Self = Self::SharePurchase;
    pub const SHARE_REDEEM: Self = Self::ShareRedeem;
    pub const SAVINGS_NOACTIVITY_FEE: Self = Self::SavingsNoActivityFee;
    pub const LINE_OF_CREDIT_ACTIVATION: Self = Self::LineOfCreditActivation;

    pub const VALID_LOAN_VALUES: &'static [i64] = &[1, 2, 8, 9, 12, 17];
    pub const VALID_LOAN_CHARGE_TIME_TYPES: &'static [Self] = &[
        Self::Disbursement,
        Self::SpecifiedDueDate,
        Self::InstallmentFee,
        Self::OverdueInstallment,
        Self::TrancheDisbursement,
        Self::LineOfCreditActivation,
    ];

    pub const VALID_SAVINGS_VALUES: &'static [i64] = &[2, 3, 4, 5, 6, 7, 10, 11, 16];
    pub const VALID_SAVINGS_CHARGE_TIME_TYPES: &'static [Self] = &[
        Self::SpecifiedDueDate,
        Self::SavingsActivation,
        Self::SavingsClosure,
        Self::WithdrawalFee,
        Self::AnnualFee,
        Self::MonthlyFee,
        Self::OverdraftFee,
        Self::WeeklyFee,
        Self::SavingsNoActivityFee,
    ];

    pub const VALID_CLIENT_VALUES: &'static [i64] = &[2];
    pub const VALID_CLIENT_CHARGE_TIME_TYPES: &'static [Self] = &[Self::SpecifiedDueDate];

    pub const VALID_SHARE_VALUES: &'static [i64] = &[13, 14, 15];
    pub const VALID_SHARE_CHARGE_TIME_TYPES: &'static [Self] = &[
        Self::ShareAccountActivate,
        Self::SharePurchase,
        Self::ShareRedeem,
    ];

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
            Self::Invalid => "chargeTimeType.invalid",
            Self::Disbursement => "chargeTimeType.disbursement",
            Self::SpecifiedDueDate => "chargeTimeType.specifiedDueDate",
            Self::SavingsActivation => "chargeTimeType.savingsActivation",
            Self::SavingsClosure => "chargeTimeType.savingsClosure",
            Self::WithdrawalFee => "chargeTimeType.withdrawalFee",
            Self::AnnualFee => "chargeTimeType.annualFee",
            Self::MonthlyFee => "chargeTimeType.monthlyFee",
            Self::InstallmentFee => "chargeTimeType.instalmentFee",
            Self::OverdueInstallment => "chargeTimeType.overdueInstallment",
            Self::OverdraftFee => "chargeTimeType.overdraftFee",
            Self::WeeklyFee => "chargeTimeType.weeklyFee",
            Self::TrancheDisbursement => "chargeTimeType.trancheDisbursement",
            Self::ShareAccountActivate => "chargeTimeType.activation",
            Self::SharePurchase => "chargeTimeType.sharespurchase",
            Self::ShareRedeem => "chargeTimeType.sharesredeem",
            Self::SavingsNoActivityFee => "chargeTimeType.savingsNoActivityFee",
            Self::LineOfCreditActivation => "chargeTimeType.lineOfCreditActivation",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Disbursement => "Disbursement",
            Self::SpecifiedDueDate => "Specified due date",
            Self::SavingsActivation => "Savings Activation",
            Self::SavingsClosure => "Savings Closure",
            Self::WithdrawalFee => "Withdrawal Fee",
            Self::AnnualFee => "Annual Fee",
            Self::MonthlyFee => "Monthly Fee",
            Self::InstallmentFee => "Installment Fee",
            Self::OverdueInstallment => "Overdue Fees",
            Self::OverdraftFee => "Overdraft Fee",
            Self::WeeklyFee => "Weekly Fee",
            Self::TrancheDisbursement => "Tranche Disbursement",
            Self::ShareAccountActivate => "Share Account Activate",
            Self::SharePurchase => "Share Purchase",
            Self::ShareRedeem => "Share Redeem",
            Self::SavingsNoActivityFee => "Saving No Activity Fee",
            Self::LineOfCreditActivation => "Line of Credit Activation",
        }
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Disbursement,
            2 => Self::SpecifiedDueDate,
            3 => Self::SavingsActivation,
            4 => Self::SavingsClosure,
            5 => Self::WithdrawalFee,
            6 => Self::AnnualFee,
            7 => Self::MonthlyFee,
            8 => Self::InstallmentFee,
            9 => Self::OverdueInstallment,
            10 => Self::OverdraftFee,
            11 => Self::WeeklyFee,
            12 => Self::TrancheDisbursement,
            13 => Self::ShareAccountActivate,
            14 => Self::SharePurchase,
            15 => Self::ShareRedeem,
            16 => Self::SavingsNoActivityFee,
            17 => Self::LineOfCreditActivation,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Disbursement,
            Some(2) => Self::SpecifiedDueDate,
            Some(3) => Self::SavingsActivation,
            Some(4) => Self::SavingsClosure,
            Some(5) => Self::WithdrawalFee,
            Some(6) => Self::AnnualFee,
            Some(7) => Self::MonthlyFee,
            Some(8) => Self::InstallmentFee,
            Some(9) => Self::OverdueInstallment,
            Some(10) => Self::OverdraftFee,
            Some(11) => Self::WeeklyFee,
            Some(12) => Self::TrancheDisbursement,
            Some(13) => Self::ShareAccountActivate,
            Some(14) => Self::SharePurchase,
            Some(15) => Self::ShareRedeem,
            Some(16) => Self::SavingsNoActivityFee,
            Some(17) => Self::LineOfCreditActivation,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn valid_loan_values() -> &'static [i64] {
        Self::VALID_LOAN_VALUES
    }

    #[inline]
    pub const fn valid_loan_charge_time_types() -> &'static [Self] {
        Self::VALID_LOAN_CHARGE_TIME_TYPES
    }

    #[inline]
    pub const fn valid_savings_values() -> &'static [i64] {
        Self::VALID_SAVINGS_VALUES
    }

    #[inline]
    pub const fn valid_savings_charge_time_types() -> &'static [Self] {
        Self::VALID_SAVINGS_CHARGE_TIME_TYPES
    }

    #[inline]
    pub const fn valid_client_values() -> &'static [i64] {
        Self::VALID_CLIENT_VALUES
    }

    #[inline]
    pub const fn valid_client_charge_time_types() -> &'static [Self] {
        Self::VALID_CLIENT_CHARGE_TIME_TYPES
    }

    #[inline]
    pub const fn valid_share_values() -> &'static [i64] {
        Self::VALID_SHARE_VALUES
    }

    #[inline]
    pub const fn valid_share_charge_time_types() -> &'static [Self] {
        Self::VALID_SHARE_CHARGE_TIME_TYPES
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_on_disbursement(self) -> bool {
        matches!(self, Self::Disbursement)
    }

    #[inline]
    pub const fn is_on_specified_due_date(self) -> bool {
        matches!(self, Self::SpecifiedDueDate)
    }

    #[inline]
    pub const fn is_savings_activation(self) -> bool {
        matches!(self, Self::SavingsActivation)
    }

    #[inline]
    pub const fn is_savings_closure(self) -> bool {
        matches!(self, Self::SavingsClosure)
    }

    #[inline]
    pub const fn is_withdrawal_fee(self) -> bool {
        matches!(self, Self::WithdrawalFee)
    }

    #[inline]
    pub const fn is_annual_fee(self) -> bool {
        matches!(self, Self::AnnualFee)
    }

    #[inline]
    pub const fn is_monthly_fee(self) -> bool {
        matches!(self, Self::MonthlyFee)
    }

    #[inline]
    pub const fn is_weekly_fee(self) -> bool {
        matches!(self, Self::WeeklyFee)
    }

    #[inline]
    pub const fn is_installment_fee(self) -> bool {
        matches!(self, Self::InstallmentFee)
    }

    #[inline]
    pub const fn is_overdue_installment(self) -> bool {
        matches!(self, Self::OverdueInstallment)
    }

    #[inline]
    pub const fn is_overdraft_fee(self) -> bool {
        matches!(self, Self::OverdraftFee)
    }

    #[inline]
    pub const fn is_tranche_disbursement(self) -> bool {
        matches!(self, Self::TrancheDisbursement)
    }

    #[inline]
    pub const fn is_share_account_activate(self) -> bool {
        matches!(self, Self::ShareAccountActivate)
    }

    #[inline]
    pub const fn is_shares_purchase(self) -> bool {
        matches!(self, Self::SharePurchase)
    }

    #[inline]
    pub const fn is_shares_redeem(self) -> bool {
        matches!(self, Self::ShareRedeem)
    }

    #[inline]
    pub const fn is_savings_no_activity_fee(self) -> bool {
        matches!(self, Self::SavingsNoActivityFee)
    }

    #[inline]
    pub const fn is_line_of_credit_activation(self) -> bool {
        matches!(self, Self::LineOfCreditActivation)
    }

    #[inline]
    pub const fn is_allowed_loan_charge_time(self) -> bool {
        matches!(
            self,
            Self::Disbursement
                | Self::SpecifiedDueDate
                | Self::InstallmentFee
                | Self::OverdueInstallment
                | Self::TrancheDisbursement
                | Self::LineOfCreditActivation
        )
    }

    #[inline]
    pub const fn is_allowed_savings_charge_time(self) -> bool {
        matches!(
            self,
            Self::SpecifiedDueDate
                | Self::SavingsActivation
                | Self::SavingsClosure
                | Self::WithdrawalFee
                | Self::AnnualFee
                | Self::MonthlyFee
                | Self::WeeklyFee
                | Self::OverdraftFee
                | Self::SavingsNoActivityFee
        )
    }

    #[inline]
    pub const fn is_allowed_client_charge_time(self) -> bool {
        matches!(self, Self::SpecifiedDueDate)
    }

    #[inline]
    pub const fn is_allowed_share_charge_time(self) -> bool {
        matches!(
            self,
            Self::ShareAccountActivate | Self::SharePurchase | Self::ShareRedeem
        )
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

// =============================================================================
// 3. ChargeCalculationType (ChargeCalculationType.java)
// =============================================================================

/// Calculation formula type for a [`Charge`] (`charge_calculation_enum`).
///
/// Ported from `org.apache.fineract.portfolio.charge.domain.ChargeCalculationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum ChargeCalculationType {
    #[default]
    Invalid = 0,
    Flat = 1,
    PercentOfAmount = 2,
    PercentOfAmountAndInterest = 3,
    PercentOfInterest = 4,
    PercentOfDisbursementAmount = 5,
}

impl ChargeCalculationType {
    pub const INVALID: Self = Self::Invalid;
    pub const FLAT: Self = Self::Flat;
    pub const PERCENT_OF_AMOUNT: Self = Self::PercentOfAmount;
    pub const PERCENT_OF_AMOUNT_AND_INTEREST: Self = Self::PercentOfAmountAndInterest;
    pub const PERCENT_OF_INTEREST: Self = Self::PercentOfInterest;
    pub const PERCENT_OF_DISBURSEMENT_AMOUNT: Self = Self::PercentOfDisbursementAmount;

    pub const VALID_VALUES_FOR_LOAN: &'static [i64] = &[1, 2, 3, 4, 5];
    pub const VALID_VALUES_FOR_SAVINGS: &'static [i64] = &[1, 2];
    pub const VALID_VALUES_FOR_CLIENTS: &'static [i64] = &[1];
    pub const VALID_VALUES_FOR_SHARES: &'static [i64] = &[1, 2];
    pub const VALID_VALUES_FOR_SHARE_ACCOUNT_ACTIVATION: &'static [i64] = &[1];
    pub const VALID_VALUES_FOR_TRANCHE_DISBURSEMENT: &'static [i64] = &[1, 5];

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
            Self::Invalid => "chargeCalculationType.invalid",
            Self::Flat => "chargeCalculationType.flat",
            Self::PercentOfAmount => "chargeCalculationType.percent.of.amount",
            Self::PercentOfAmountAndInterest => {
                "chargeCalculationType.percent.of.amount.and.interest"
            }
            Self::PercentOfInterest => "chargeCalculationType.percent.of.interest",
            Self::PercentOfDisbursementAmount => {
                "chargeCalculationType.percent.of.disbursement.amount"
            }
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Flat => "Flat",
            Self::PercentOfAmount => "% Amount",
            Self::PercentOfAmountAndInterest => "% Loan Amount + Interest",
            Self::PercentOfInterest => "% Interest",
            Self::PercentOfDisbursementAmount => "% Disbursement Amount",
        }
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Flat,
            2 => Self::PercentOfAmount,
            3 => Self::PercentOfAmountAndInterest,
            4 => Self::PercentOfInterest,
            5 => Self::PercentOfDisbursementAmount,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::Flat,
            Some(2) => Self::PercentOfAmount,
            Some(3) => Self::PercentOfAmountAndInterest,
            Some(4) => Self::PercentOfInterest,
            Some(5) => Self::PercentOfDisbursementAmount,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn valid_values_for_loan() -> &'static [i64] {
        Self::VALID_VALUES_FOR_LOAN
    }

    #[inline]
    pub const fn valid_values_for_savings() -> &'static [i64] {
        Self::VALID_VALUES_FOR_SAVINGS
    }

    #[inline]
    pub const fn valid_values_for_clients() -> &'static [i64] {
        Self::VALID_VALUES_FOR_CLIENTS
    }

    #[inline]
    pub const fn valid_values_for_shares() -> &'static [i64] {
        Self::VALID_VALUES_FOR_SHARES
    }

    #[inline]
    pub const fn valid_values_for_share_account_activation() -> &'static [i64] {
        Self::VALID_VALUES_FOR_SHARE_ACCOUNT_ACTIVATION
    }

    #[inline]
    pub const fn valid_values_for_tranche_disbursement() -> &'static [i64] {
        Self::VALID_VALUES_FOR_TRANCHE_DISBURSEMENT
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_flat(self) -> bool {
        matches!(self, Self::Flat)
    }

    #[inline]
    pub const fn is_percentage_of_amount(self) -> bool {
        matches!(self, Self::PercentOfAmount)
    }

    #[inline]
    pub const fn is_percentage_of_amount_and_interest(self) -> bool {
        matches!(self, Self::PercentOfAmountAndInterest)
    }

    #[inline]
    pub const fn is_percentage_of_interest(self) -> bool {
        matches!(self, Self::PercentOfInterest)
    }

    #[inline]
    pub const fn is_percentage_of_disbursement_amount(self) -> bool {
        matches!(self, Self::PercentOfDisbursementAmount)
    }

    #[inline]
    pub const fn is_percentage_based(self) -> bool {
        matches!(
            self,
            Self::PercentOfAmount
                | Self::PercentOfAmountAndInterest
                | Self::PercentOfInterest
                | Self::PercentOfDisbursementAmount
        )
    }

    #[inline]
    pub const fn has_interest(self) -> bool {
        matches!(
            self,
            Self::PercentOfInterest | Self::PercentOfAmountAndInterest
        )
    }

    #[inline]
    pub const fn is_allowed_savings_charge_calculation_type(self) -> bool {
        matches!(self, Self::Flat | Self::PercentOfAmount)
    }

    #[inline]
    pub const fn is_allowed_client_charge_calculation_type(self) -> bool {
        matches!(self, Self::Flat)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

// =============================================================================
// 4. ChargePaymentMode (ChargePaymentMode.java)
// =============================================================================

/// Payment mode for settling a loan [`Charge`] (`charge_payment_mode_enum`).
///
/// Ported from `org.apache.fineract.portfolio.charge.domain.ChargePaymentMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum ChargePaymentMode {
    #[default]
    Regular = 0,
    AccountTransfer = 1,
}

impl ChargePaymentMode {
    pub const REGULAR: Self = Self::Regular;
    pub const ACCOUNT_TRANSFER: Self = Self::AccountTransfer;

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
    pub const fn code(self) -> &'static str {
        match self {
            Self::Regular => "chargepaymentmode.regular",
            Self::AccountTransfer => "chargepaymentmode.accounttransfer",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Regular => "Regular",
            Self::AccountTransfer => "Account transfer",
        }
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::AccountTransfer,
            _ => Self::Regular,
        }
    }

    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(1) => Self::AccountTransfer,
            _ => Self::Regular,
        }
    }

    #[inline]
    pub const fn valid_values() -> &'static [i64] {
        Self::VALID_VALUES
    }

    #[inline]
    pub const fn is_payment_mode_account_transfer(self) -> bool {
        matches!(self, Self::AccountTransfer)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

// =============================================================================
// 5. FeeFrequencyType (PeriodFrequencyType.java / ChargeEnumerations.java)
// =============================================================================

/// Recurring fee frequency type (`fee_frequency` in `m_charge`).
///
/// Ported from `org.apache.fineract.portfolio.common.domain.PeriodFrequencyType`
/// and `ChargeEnumerations.feeFrequencyType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum FeeFrequencyType {
    Days = 0,
    Weeks = 1,
    Months = 2,
    Years = 3,
    WholeTerm = 4,
    #[default]
    Invalid = 5,
}

impl FeeFrequencyType {
    pub const DAYS: Self = Self::Days;
    pub const WEEKS: Self = Self::Weeks;
    pub const MONTHS: Self = Self::Months;
    pub const YEARS: Self = Self::Years;
    pub const WHOLE_TERM: Self = Self::WholeTerm;
    pub const INVALID: Self = Self::Invalid;

    /// Valid fee frequency values allowed on `Charge` (`0..=3`: Days, Weeks, Months, Years).
    pub const VALID_FEE_FREQUENCY_VALUES: &'static [i64] = &[0, 1, 2, 3];

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
            Self::Days => "feeFrequencyperiodFrequencyType.days",
            Self::Weeks => "feeFrequencyperiodFrequencyType.weeks",
            Self::Months => "feeFrequencyperiodFrequencyType.months",
            Self::Years => "feeFrequencyperiodFrequencyType.years",
            Self::WholeTerm => "feeFrequencyperiodFrequencyType.whole_term",
            Self::Invalid => "feeFrequencyperiodFrequencyType.invalid",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Days => "Days",
            Self::Weeks => "Weeks",
            Self::Months => "Months",
            Self::Years => "Years",
            Self::WholeTerm => "Whole Term",
            Self::Invalid => "Invalid",
        }
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        match value {
            0 => Self::Days,
            1 => Self::Weeks,
            2 => Self::Months,
            3 => Self::Years,
            4 => Self::WholeTerm,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(0) => Self::Days,
            Some(1) => Self::Weeks,
            Some(2) => Self::Months,
            Some(3) => Self::Years,
            Some(4) => Self::WholeTerm,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn valid_fee_frequency_values() -> &'static [i64] {
        Self::VALID_FEE_FREQUENCY_VALUES
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

/// Enumeration conversion helpers ported from `ChargeEnumerations.java`.
pub struct ChargeEnumerations;

impl ChargeEnumerations {
    pub fn charge_applies_to(id: i32) -> EnumOptionData {
        ChargeAppliesTo::from_int(Some(id)).to_enum_option_data()
    }

    pub fn charge_time_type(id: i32) -> EnumOptionData {
        ChargeTimeType::from_int(Some(id)).to_enum_option_data()
    }

    pub fn charge_calculation_type(id: i32) -> EnumOptionData {
        ChargeCalculationType::from_int(Some(id)).to_enum_option_data()
    }

    pub fn charge_payment_mode(id: i32) -> EnumOptionData {
        ChargePaymentMode::from_int(Some(id)).to_enum_option_data()
    }

    pub fn fee_frequency_type(id: i32) -> EnumOptionData {
        FeeFrequencyType::from_int(Some(id)).to_enum_option_data()
    }
}

// =============================================================================
// 6. Charge Domain Exceptions & Error Helpers
// =============================================================================

/// Port of `ChargeNotFoundException(id)`.
pub fn charge_not_found_error(id: i64) -> FineractError {
    FineractError::resource_not_found(
        "error.msg.charge.id.invalid",
        format!("Charge with identifier {id} does not exist"),
        id.to_string(),
    )
}

/// Port of `ChargeDueAtDisbursementCannotBePenaltyException(name)`.
pub fn charge_due_at_disbursement_cannot_be_penalty_error(name: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.charge.due.at.disbursement.cannot.be.penalty",
        format!("Charge `{name}` is due at disbursement and cannot be a penalty."),
        vec![ApiErrorArg::from(name)],
    ))
}

/// Port of `ChargeMustBePenaltyException(name)`.
pub fn charge_must_be_penalty_error(name: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.charge.must.be.penalty",
        format!("Charge `{name}` must be a penalty."),
        vec![ApiErrorArg::from(name)],
    ))
}

/// Port of duplicate charge name data integrity error (`error.msg.charge.name.already.exists`).
pub fn charge_name_already_exists_error(name: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.charge.name.already.exists",
        format!("Charge with name `{name}` already exists."),
        vec![ApiErrorArg::from("name"), ApiErrorArg::from(name)],
    ))
}

/// Port of `ChargeCannotBeUpdatedException`.
pub fn charge_cannot_be_updated_error(
    globalisation_code: impl Into<String>,
    default_user_message: impl Into<String>,
) -> FineractError {
    FineractError::domain_rule(globalisation_code, default_user_message)
}

/// Port of `ChargeCannotBeDeletedException`.
pub fn charge_cannot_be_deleted_error(
    globalisation_code: impl Into<String>,
    default_user_message: impl Into<String>,
) -> FineractError {
    FineractError::domain_rule(globalisation_code, default_user_message)
}

// =============================================================================
// 7. Charge & ChargeData (Charge.java, ChargeData.java, matching m_charge)
// =============================================================================

/// Domain entity representing a fee or penalty definition in `m_charge`.
///
/// Ported from `org.apache.fineract.portfolio.charge.domain.Charge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Charge {
    pub id: i64,
    pub name: String,
    pub currency_code: String,
    pub charge_applies_to: ChargeAppliesTo,
    pub charge_time_type: ChargeTimeType,
    pub charge_calculation_type: ChargeCalculationType,
    pub charge_payment_mode: ChargePaymentMode,
    pub amount: Decimal,
    #[serde(alias = "penalty", alias = "is_penalty")]
    pub is_penalty: bool,
    #[serde(alias = "active", alias = "is_active")]
    pub is_active: bool,
    #[serde(alias = "deleted", alias = "is_deleted", alias = "is_Deleted", alias = "isDeleted")]
    pub is_deleted: bool,
    pub min_cap: Option<Decimal>,
    pub max_cap: Option<Decimal>,
    pub fee_frequency: Option<FeeFrequencyType>,
    pub fee_interval: Option<i32>,
    pub fee_on_month: Option<u8>,
    pub fee_on_day: Option<u8>,
    pub income_or_liability_account_id: Option<i64>,
    pub tax_group_id: Option<i64>,
}

/// Read-model DTO representing a fee or penalty definition.
///
/// Ported from `org.apache.fineract.portfolio.charge.data.ChargeData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChargeData {
    pub id: i64,
    pub name: String,
    pub currency_code: String,
    pub charge_applies_to: ChargeAppliesTo,
    pub charge_time_type: ChargeTimeType,
    pub charge_calculation_type: ChargeCalculationType,
    pub charge_payment_mode: ChargePaymentMode,
    pub amount: Decimal,
    #[serde(alias = "penalty", alias = "is_penalty")]
    pub is_penalty: bool,
    #[serde(alias = "active", alias = "is_active")]
    pub is_active: bool,
    #[serde(alias = "deleted", alias = "is_deleted", alias = "is_Deleted", alias = "isDeleted")]
    pub is_deleted: bool,
    pub min_cap: Option<Decimal>,
    pub max_cap: Option<Decimal>,
    pub fee_frequency: Option<FeeFrequencyType>,
    pub fee_interval: Option<i32>,
    pub fee_on_month: Option<u8>,
    pub fee_on_day: Option<u8>,
    pub income_or_liability_account_id: Option<i64>,
    pub tax_group_id: Option<i64>,
}

impl Charge {
    /// Creates a new [`Charge`] domain instance and validates domain rules (`Charge.fromJson` in Java).
    pub fn from_json(id: i64, command: &JsonCommand) -> Result<Self, FineractError> {
        ChargeDefinitionValidator::validate_for_create(command)?;

        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let currency_code = command
            .string_value_of_parameter_named("currencyCode")
            .unwrap_or_default();
        let amount = command
            .big_decimal_value_of_parameter_named("amount")?
            .unwrap_or(Decimal::ZERO);

        let charge_applies_to = ChargeAppliesTo::from_int(
            command.integer_value_of_parameter_named("chargeAppliesTo")?,
        );
        let charge_time_type =
            ChargeTimeType::from_int(command.integer_value_of_parameter_named("chargeTimeType")?);
        let charge_calculation_type = ChargeCalculationType::from_int(
            command.integer_value_of_parameter_named("chargeCalculationType")?,
        );
        let charge_payment_mode = if command.parameter_exists("chargePaymentMode") {
            ChargePaymentMode::from_int(
                command.integer_value_of_parameter_named("chargePaymentMode")?,
            )
        } else {
            ChargePaymentMode::Regular
        };

        let is_penalty = command.boolean_primitive_value_of_parameter_named("penalty");
        let is_active = command.boolean_primitive_value_of_parameter_named("active");

        let min_cap = command
            .big_decimal_value_of_parameter_named_default_to_null_if_zero("minCap")?;
        let max_cap = command
            .big_decimal_value_of_parameter_named_default_to_null_if_zero("maxCap")?;

        let mut fee_frequency = command
            .integer_value_of_parameter_named("feeFrequency")?
            .map(|v| FeeFrequencyType::from_int(Some(v)));
        let mut fee_interval = command.integer_value_of_parameter_named("feeInterval")?;
        let mut fee_on_month: Option<u8> = None;
        let mut fee_on_day: Option<u8> = None;

        if matches!(
            charge_time_type,
            ChargeTimeType::AnnualFee | ChargeTimeType::MonthlyFee
        ) {
            if let Some(month_day) = command.local_date_value_of_parameter_named("feeOnMonthDay")? {
                use chrono::Datelike;
                fee_on_month = u8::try_from(month_day.month()).ok();
                fee_on_day = u8::try_from(month_day.day()).ok();
            } else {
                fee_on_month = command
                    .integer_value_sans_locale_of_parameter_named("feeOnMonth")?
                    .and_then(|m| u8::try_from(m).ok());
                fee_on_day = command
                    .integer_value_sans_locale_of_parameter_named("feeOnDay")?
                    .and_then(|d| u8::try_from(d).ok());
            }
        }

        if charge_time_type == ChargeTimeType::MonthlyFee {
            fee_frequency = Some(FeeFrequencyType::Months);
        } else if charge_time_type == ChargeTimeType::WeeklyFee {
            fee_frequency = Some(FeeFrequencyType::Weeks);
        }

        if matches!(
            charge_time_type,
            ChargeTimeType::MonthlyFee | ChargeTimeType::WeeklyFee
        ) && fee_interval.is_none()
        {
            fee_interval = Some(1);
        }

        let income_or_liability_account_id =
            command.long_value_of_parameter_named("incomeAccountId")?;
        let tax_group_id = command.long_value_of_parameter_named("taxGroupId")?;

        let charge = Self {
            id,
            name,
            currency_code,
            charge_applies_to,
            charge_time_type,
            charge_calculation_type,
            charge_payment_mode,
            amount,
            is_penalty,
            is_active,
            is_deleted: false,
            min_cap,
            max_cap,
            fee_frequency,
            fee_interval,
            fee_on_month,
            fee_on_day,
            income_or_liability_account_id,
            tax_group_id,
        };

        charge.validate_domain_rules()?;
        Ok(charge)
    }

    /// Validates cross-field domain rules from `Charge.java` constructor.
    pub fn validate_domain_rules(&self) -> Result<(), FineractError> {
        if self.charge_applies_to.is_loan_charge() {
            if (self.charge_time_type.is_on_disbursement()
                || self.charge_time_type.is_tranche_disbursement())
                && self.is_penalty
            {
                return Err(charge_due_at_disbursement_cannot_be_penalty_error(
                    &self.name,
                ));
            }
            if self.charge_time_type.is_overdue_installment() && !self.is_penalty {
                return Err(charge_must_be_penalty_error(&self.name));
            }
        }
        Ok(())
    }

    /// Applies partial updates from a [`JsonCommand`] (`Charge.update` in Java) and returns changed fields.
    pub fn update(
        &mut self,
        command: &JsonCommand,
    ) -> Result<BTreeMap<String, Value>, FineractError> {
        ChargeDefinitionValidator::validate_for_update(command)?;

        let mut changes = BTreeMap::new();

        if command.is_change_in_string_parameter_named("name", Some(&self.name)) {
            if let Some(new_val) = command.string_value_of_parameter_named("name") {
                changes.insert("name".to_string(), json!(new_val));
                self.name = new_val;
            }
        }

        if command.is_change_in_string_parameter_named("currencyCode", Some(&self.currency_code)) {
            if let Some(new_val) = command.string_value_of_parameter_named("currencyCode") {
                changes.insert("currencyCode".to_string(), json!(new_val));
                self.currency_code = new_val;
            }
        }

        if command.is_change_in_big_decimal_parameter_named("amount", Some(self.amount))? {
            if let Some(new_val) = command.big_decimal_value_of_parameter_named("amount")? {
                changes.insert("amount".to_string(), json!(new_val));
                self.amount = new_val;
            }
        }

        if command.is_change_in_integer_parameter_named(
            "chargeAppliesTo",
            Some(self.charge_applies_to.value()),
        )? {
            if let Some(new_val) = command.integer_value_of_parameter_named("chargeAppliesTo")? {
                changes.insert("chargeAppliesTo".to_string(), json!(new_val));
                self.charge_applies_to = ChargeAppliesTo::from_int(Some(new_val));
            }
        }

        if command.is_change_in_integer_parameter_named(
            "chargeTimeType",
            Some(self.charge_time_type.value()),
        )? {
            if let Some(new_val) = command.integer_value_of_parameter_named("chargeTimeType")? {
                changes.insert("chargeTimeType".to_string(), json!(new_val));
                self.charge_time_type = ChargeTimeType::from_int(Some(new_val));
            }
        }

        if command.is_change_in_integer_parameter_named(
            "chargeCalculationType",
            Some(self.charge_calculation_type.value()),
        )? {
            if let Some(new_val) =
                command.integer_value_of_parameter_named("chargeCalculationType")?
            {
                changes.insert("chargeCalculationType".to_string(), json!(new_val));
                self.charge_calculation_type = ChargeCalculationType::from_int(Some(new_val));
            }
        }

        if command.is_change_in_integer_parameter_named(
            "chargePaymentMode",
            Some(self.charge_payment_mode.value()),
        )? {
            if let Some(new_val) = command.integer_value_of_parameter_named("chargePaymentMode")? {
                changes.insert("chargePaymentMode".to_string(), json!(new_val));
                self.charge_payment_mode = ChargePaymentMode::from_int(Some(new_val));
            }
        }

        if command.is_change_in_boolean_parameter_named("penalty", Some(self.is_penalty)) {
            let new_val = command.boolean_primitive_value_of_parameter_named("penalty");
            changes.insert("penalty".to_string(), json!(new_val));
            self.is_penalty = new_val;
        }

        if command.is_change_in_boolean_parameter_named("active", Some(self.is_active)) {
            let new_val = command.boolean_primitive_value_of_parameter_named("active");
            changes.insert("active".to_string(), json!(new_val));
            self.is_active = new_val;
        }

        if command.is_change_in_big_decimal_parameter_named("minCap", self.min_cap)? {
            let new_val = command
                .big_decimal_value_of_parameter_named_default_to_null_if_zero("minCap")?;
            changes.insert("minCap".to_string(), json!(new_val));
            self.min_cap = new_val;
        }

        if command.is_change_in_big_decimal_parameter_named("maxCap", self.max_cap)? {
            let new_val = command
                .big_decimal_value_of_parameter_named_default_to_null_if_zero("maxCap")?;
            changes.insert("maxCap".to_string(), json!(new_val));
            self.max_cap = new_val;
        }

        if command.is_change_in_integer_parameter_named(
            "feeFrequency",
            self.fee_frequency.map(FeeFrequencyType::value),
        )? {
            let new_val = command.integer_value_of_parameter_named("feeFrequency")?;
            changes.insert("feeFrequency".to_string(), json!(new_val));
            self.fee_frequency = new_val.map(|v| FeeFrequencyType::from_int(Some(v)));
        }

        if command.is_change_in_integer_parameter_named("feeInterval", self.fee_interval)? {
            let new_val = command.integer_value_of_parameter_named("feeInterval")?;
            changes.insert("feeInterval".to_string(), json!(new_val));
            self.fee_interval = new_val;
        }

        if command.is_change_in_long_parameter_named(
            "incomeAccountId",
            self.income_or_liability_account_id,
        )? {
            let new_val = command.long_value_of_parameter_named("incomeAccountId")?;
            changes.insert("incomeAccountId".to_string(), json!(new_val));
            self.income_or_liability_account_id = new_val;
        }

        if command.is_change_in_long_parameter_named("taxGroupId", self.tax_group_id)? {
            let new_val = command.long_value_of_parameter_named("taxGroupId")?;
            changes.insert("taxGroupId".to_string(), json!(new_val));
            self.tax_group_id = new_val;
        }

        ChargeDefinitionValidator::validate_charge(self)?;
        self.validate_domain_rules()?;

        Ok(changes)
    }

    /// Soft-deletes this charge (`Charge.delete()` in Java), prefixing the name with `_deleted_<id>_`.
    pub fn delete(&mut self) {
        self.is_deleted = true;
        let prefix = format!("_deleted_{}_", self.id);
        if !self.name.starts_with(&prefix) {
            self.name = format!("{prefix}{}", self.name);
        }
    }

    #[inline]
    pub const fn is_deleted(&self) -> bool {
        self.is_deleted
    }

    /// Compatibility accessor matching prompt capitalization `is_Deleted`.
    #[allow(non_snake_case)]
    #[inline]
    pub const fn is_Deleted(&self) -> bool {
        self.is_deleted
    }

    #[inline]
    pub const fn is_active(&self) -> bool {
        self.is_active
    }

    #[inline]
    pub const fn is_penalty(&self) -> bool {
        self.is_penalty
    }

    #[inline]
    pub const fn is_loan_charge(&self) -> bool {
        self.charge_applies_to.is_loan_charge()
    }

    #[inline]
    pub const fn is_savings_charge(&self) -> bool {
        self.charge_applies_to.is_savings_charge()
    }

    #[inline]
    pub const fn is_client_charge(&self) -> bool {
        self.charge_applies_to.is_client_charge()
    }

    #[inline]
    pub const fn is_shares_charge(&self) -> bool {
        self.charge_applies_to.is_shares_charge()
    }

    /// Converts this domain [`Charge`] into a [`ChargeData`] DTO.
    pub fn to_data(&self) -> ChargeData {
        ChargeData {
            id: self.id,
            name: self.name.clone(),
            currency_code: self.currency_code.clone(),
            charge_applies_to: self.charge_applies_to,
            charge_time_type: self.charge_time_type,
            charge_calculation_type: self.charge_calculation_type,
            charge_payment_mode: self.charge_payment_mode,
            amount: self.amount,
            is_penalty: self.is_penalty,
            is_active: self.is_active,
            is_deleted: self.is_deleted,
            min_cap: self.min_cap,
            max_cap: self.max_cap,
            fee_frequency: self.fee_frequency,
            fee_interval: self.fee_interval,
            fee_on_month: self.fee_on_month,
            fee_on_day: self.fee_on_day,
            income_or_liability_account_id: self.income_or_liability_account_id,
            tax_group_id: self.tax_group_id,
        }
    }
}

impl ChargeData {
    #[inline]
    pub const fn is_deleted(&self) -> bool {
        self.is_deleted
    }

    /// Compatibility accessor matching prompt capitalization `is_Deleted`.
    #[allow(non_snake_case)]
    #[inline]
    pub const fn is_Deleted(&self) -> bool {
        self.is_deleted
    }

    pub fn currency_data(&self) -> CurrencyData {
        CurrencyData::from_code(self.currency_code.clone())
    }
}

impl From<&Charge> for ChargeData {
    fn from(charge: &Charge) -> Self {
        charge.to_data()
    }
}

impl From<Charge> for ChargeData {
    fn from(charge: Charge) -> Self {
        charge.to_data()
    }
}

// =============================================================================
// 8. ChargeDefinitionValidator (ChargeDefinitionCommandFromApiJsonDeserializer.java)
// =============================================================================

/// Supported JSON parameters for `ChargeDefinitionCommandFromApiJsonDeserializer`.
pub const SUPPORTED_CHARGE_PARAMETERS: &[&str] = &[
    "name",
    "amount",
    "locale",
    "currencyCode",
    "months",
    "monthDayFormat",
    "chargeAppliesTo",
    "chargeTimeType",
    "chargeCalculationType",
    "chargePaymentMode",
    "penalty",
    "active",
    "minCap",
    "maxCap",
    "feeOnMonthDay",
    "feeInterval",
    "feeFrequency",
    "incomeAccountId",
    "taxGroupId",
    "isFreeWithdrawal",
    "freeWithdrawalFrequency",
    "restartFrequency",
    "restartFrequencyEnum",
    "isPaymentType",
    "paymentTypeId",
];

/// Validator for charge creation, update, and calculation compatibility rules.
///
/// Ported from `ChargeDefinitionCommandFromApiJsonDeserializer.java`.
pub struct ChargeDefinitionValidator;

impl ChargeDefinitionValidator {
    /// Validates a `CREATE` charge [`JsonCommand`].
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(SUPPORTED_CHARGE_PARAMETERS)?;

        let base = DataValidatorBuilder::for_resource("charge");

        let charge_applies_to_raw = command.integer_value_of_parameter_named("chargeAppliesTo")?;
        base.reset()
            .parameter("chargeAppliesTo")
            .value(charge_applies_to_raw)
            .not_null()
            .is_one_of_these_values(ChargeAppliesTo::valid_values().iter().copied());

        let charge_time_type_raw = command.integer_value_of_parameter_named("chargeTimeType")?;
        base.reset()
            .parameter("chargeTimeType")
            .value(charge_time_type_raw)
            .not_null();

        let charge_calculation_type_raw =
            command.integer_value_of_parameter_named("chargeCalculationType")?;
        base.reset()
            .parameter("chargeCalculationType")
            .value(charge_calculation_type_raw)
            .not_null();

        let name = command.string_value_of_parameter_named("name");
        base.reset()
            .parameter("name")
            .value(name)
            .not_blank()
            .not_exceeding_length_of(100);

        let currency_code = command.string_value_of_parameter_named("currencyCode");
        base.reset()
            .parameter("currencyCode")
            .value(currency_code)
            .not_blank()
            .not_exceeding_length_of(3);

        let amount = command.big_decimal_value_of_parameter_named("amount")?;
        base.reset()
            .parameter("amount")
            .value(amount)
            .not_null()
            .positive_amount();

        if command.parameter_exists("penalty") {
            let penalty = command.boolean_object_value_of_parameter_named("penalty");
            base.reset()
                .parameter("penalty")
                .value(penalty)
                .not_null();
        }

        if command.parameter_exists("active") {
            let active = command.boolean_object_value_of_parameter_named("active");
            base.reset().parameter("active").value(active).not_null();
        }

        let min_cap = command.big_decimal_value_of_parameter_named("minCap")?;
        let max_cap = command.big_decimal_value_of_parameter_named("maxCap")?;

        let charge_applies_to = ChargeAppliesTo::from_int(charge_applies_to_raw);
        let charge_time_type = ChargeTimeType::from_int(charge_time_type_raw);
        let charge_calculation_type = ChargeCalculationType::from_int(charge_calculation_type_raw);

        Self::validate_compatibility_with_builder(
            &base,
            charge_applies_to,
            charge_time_type,
            charge_calculation_type,
            min_cap,
            max_cap,
            command.parameter_exists("minCap"),
            command.parameter_exists("maxCap"),
        );

        if command.parameter_exists("chargePaymentMode") {
            let payment_mode_raw =
                command.integer_value_of_parameter_named("chargePaymentMode")?;
            base.reset()
                .parameter("chargePaymentMode")
                .value(payment_mode_raw)
                .ignore_if_null()
                .is_one_of_these_values(ChargePaymentMode::valid_values().iter().copied());
        }

        if command.parameter_exists("feeFrequency") {
            let fee_frequency_raw = command.integer_value_of_parameter_named("feeFrequency")?;
            base.reset()
                .parameter("feeFrequency")
                .value(fee_frequency_raw)
                .ignore_if_null()
                .is_one_of_these_values(
                    FeeFrequencyType::valid_fee_frequency_values()
                        .iter()
                        .copied(),
                );
            if fee_frequency_raw.is_some() {
                let fee_interval = command.integer_value_of_parameter_named("feeInterval")?;
                base.reset()
                    .parameter("feeInterval")
                    .value(fee_interval)
                    .not_null()
                    .integer_greater_than_zero();
            }
        }

        if command.parameter_exists("incomeAccountId") {
            let income_account_id = command.long_value_of_parameter_named("incomeAccountId")?;
            base.reset()
                .parameter("incomeAccountId")
                .value(income_account_id)
                .ignore_if_null()
                .long_greater_than_zero();
        }

        if command.parameter_exists("taxGroupId") {
            let tax_group_id = command.long_value_of_parameter_named("taxGroupId")?;
            base.reset()
                .parameter("taxGroupId")
                .value(tax_group_id)
                .ignore_if_null()
                .long_greater_than_zero();
        }

        base.throw_validation_errors()?;
        Ok(())
    }

    /// Validates an `UPDATE` charge [`JsonCommand`].
    pub fn validate_for_update(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(SUPPORTED_CHARGE_PARAMETERS)?;

        let base = DataValidatorBuilder::for_resource("charge");

        if command.parameter_exists("name") {
            let name = command.string_value_of_parameter_named("name");
            base.reset()
                .parameter("name")
                .value(name)
                .not_blank()
                .not_exceeding_length_of(100);
        }

        if command.parameter_exists("currencyCode") {
            let currency_code = command.string_value_of_parameter_named("currencyCode");
            base.reset()
                .parameter("currencyCode")
                .value(currency_code)
                .not_blank()
                .not_exceeding_length_of(3);
        }

        if command.parameter_exists("amount") {
            let amount = command.big_decimal_value_of_parameter_named("amount")?;
            base.reset()
                .parameter("amount")
                .value(amount)
                .not_null()
                .positive_amount();
        }

        if command.parameter_exists("chargeAppliesTo") {
            let charge_applies_to_raw =
                command.integer_value_of_parameter_named("chargeAppliesTo")?;
            base.reset()
                .parameter("chargeAppliesTo")
                .value(charge_applies_to_raw)
                .not_null()
                .is_one_of_these_values(ChargeAppliesTo::valid_values().iter().copied());
        }

        if command.parameter_exists("chargePaymentMode") {
            let payment_mode_raw =
                command.integer_value_of_parameter_named("chargePaymentMode")?;
            base.reset()
                .parameter("chargePaymentMode")
                .value(payment_mode_raw)
                .not_null()
                .is_one_of_these_values(ChargePaymentMode::valid_values().iter().copied());
        }

        if command.parameter_exists("minCap") {
            let min_cap = command.big_decimal_value_of_parameter_named("minCap")?;
            base.reset()
                .parameter("minCap")
                .value(min_cap)
                .ignore_if_null()
                .positive_amount();
        }

        if command.parameter_exists("maxCap") {
            let max_cap = command.big_decimal_value_of_parameter_named("maxCap")?;
            base.reset()
                .parameter("maxCap")
                .value(max_cap)
                .ignore_if_null()
                .positive_amount();
        }

        if command.parameter_exists("feeFrequency") {
            let fee_frequency_raw = command.integer_value_of_parameter_named("feeFrequency")?;
            base.reset()
                .parameter("feeFrequency")
                .value(fee_frequency_raw)
                .ignore_if_null()
                .is_one_of_these_values(
                    FeeFrequencyType::valid_fee_frequency_values()
                        .iter()
                        .copied(),
                );
        }

        if command.parameter_exists("feeInterval") {
            let fee_interval = command.integer_value_of_parameter_named("feeInterval")?;
            base.reset()
                .parameter("feeInterval")
                .value(fee_interval)
                .ignore_if_null()
                .integer_greater_than_zero();
        }

        if command.parameter_exists("incomeAccountId") {
            let income_account_id = command.long_value_of_parameter_named("incomeAccountId")?;
            base.reset()
                .parameter("incomeAccountId")
                .value(income_account_id)
                .ignore_if_null()
                .long_greater_than_zero();
        }

        if command.parameter_exists("taxGroupId") {
            let tax_group_id = command.long_value_of_parameter_named("taxGroupId")?;
            base.reset()
                .parameter("taxGroupId")
                .value(tax_group_id)
                .ignore_if_null()
                .long_greater_than_zero();
        }

        base.throw_validation_errors()?;
        Ok(())
    }

    /// Validates a [`Charge`] struct against the `ChargeDefinitionCommandFromApiJsonDeserializer`
    /// compatibility rules for `ChargeAppliesTo`, `ChargeTimeType`, `ChargeCalculationType`,
    /// `min_cap`, and `max_cap`.
    pub fn validate_charge(charge: &Charge) -> FineractResult<()> {
        let base = DataValidatorBuilder::for_resource("charge");

        base.reset()
            .parameter("chargeAppliesTo")
            .value(i64::from(charge.charge_applies_to.value()))
            .is_one_of_these_values(ChargeAppliesTo::valid_values().iter().copied());

        base.reset()
            .parameter("amount")
            .value(charge.amount)
            .positive_amount();

        Self::validate_compatibility_with_builder(
            &base,
            charge.charge_applies_to,
            charge.charge_time_type,
            charge.charge_calculation_type,
            charge.min_cap,
            charge.max_cap,
            charge.min_cap.is_some(),
            charge.max_cap.is_some(),
        );

        base.throw_validation_errors()?;
        Ok(())
    }

    /// Core compatibility matrix from `ChargeDefinitionCommandFromApiJsonDeserializer.java`.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_compatibility_with_builder(
        base: &DataValidatorBuilder<'_>,
        charge_applies_to: ChargeAppliesTo,
        charge_time_type: ChargeTimeType,
        charge_calculation_type: ChargeCalculationType,
        min_cap: Option<Decimal>,
        max_cap: Option<Decimal>,
        has_min_cap: bool,
        has_max_cap: bool,
    ) {
        let time_val = i64::from(charge_time_type.value());
        let calc_val = i64::from(charge_calculation_type.value());

        match charge_applies_to {
            ChargeAppliesTo::Loan => {
                // Loan applicable validation (`ChargeTimeType.validLoanValues()`)
                base.reset()
                    .parameter("chargeTimeType")
                    .value(time_val)
                    .is_one_of_these_values(ChargeTimeType::valid_loan_values().iter().copied());

                // Loan calculation type (`ChargeCalculationType.validValuesForLoan()`)
                base.reset()
                    .parameter("chargeCalculationType")
                    .value(calc_val)
                    .is_one_of_these_values(
                        ChargeCalculationType::valid_values_for_loan()
                            .iter()
                            .copied(),
                    );

                // Tranche disbursement allows only FLAT (1) or PERCENT_OF_DISBURSEMENT_AMOUNT (5)
                if charge_time_type.is_tranche_disbursement() {
                    base.reset()
                        .parameter("chargeCalculationType")
                        .value(calc_val)
                        .is_one_of_these_values(
                            ChargeCalculationType::valid_values_for_tranche_disbursement()
                                .iter()
                                .copied(),
                        );
                }

                // PERCENT_OF_DISBURSEMENT_AMOUNT (5) is NOT allowed on non-disbursement loan charge times
                if !charge_time_type.is_on_disbursement()
                    && !charge_time_type.is_tranche_disbursement()
                {
                    base.reset()
                        .parameter("chargeCalculationType")
                        .value(calc_val)
                        .is_not_one_of_these_values([i64::from(
                            ChargeCalculationType::PercentOfDisbursementAmount.value(),
                        )]);
                }

                // minCap / maxCap are only allowed on PERCENT_OF_AMOUNT, PERCENT_OF_AMOUNT_AND_INTEREST,
                // or PERCENT_OF_DISBURSEMENT_AMOUNT for Loan charges
                let allows_cap = charge_calculation_type.is_percentage_of_amount()
                    || charge_calculation_type.is_percentage_of_amount_and_interest()
                    || charge_calculation_type.is_percentage_of_disbursement_amount();

                if (has_min_cap || has_max_cap) && !allows_cap {
                    base.reset()
                        .parameter("chargeCalculationType")
                        .value(calc_val)
                        .fail_with_code(
                            "charge.calculation.type.min.max.cap.not.applicable",
                            "minCap and maxCap are only applicable for percentage of amount, amount+interest, or disbursement amount loan charges.",
                        );
                }

                if let Some(min_val) = min_cap {
                    base.reset()
                        .parameter("minCap")
                        .value(min_val)
                        .positive_amount();
                }
                if let Some(max_val) = max_cap {
                    base.reset()
                        .parameter("maxCap")
                        .value(max_val)
                        .positive_amount();
                }
                if let (Some(min_val), Some(max_val)) = (min_cap, max_cap) {
                    if min_val > max_val {
                        base.reset()
                            .parameter("minCap")
                            .value(min_val)
                            .fail_with_code(
                                "min.cap.cannot.be.greater.than.max.cap",
                                "minCap cannot be greater than maxCap.",
                            );
                    }
                }
            }
            ChargeAppliesTo::Savings => {
                // Savings applicable validation (`ChargeTimeType.validSavingsValues()`)
                base.reset()
                    .parameter("chargeTimeType")
                    .value(time_val)
                    .is_one_of_these_values(
                        ChargeTimeType::valid_savings_values().iter().copied(),
                    );

                // Savings calculation type (`ChargeCalculationType.validValuesForSavings()`: 1, 2)
                base.reset()
                    .parameter("chargeCalculationType")
                    .value(calc_val)
                    .is_one_of_these_values(
                        ChargeCalculationType::valid_values_for_savings()
                            .iter()
                            .copied(),
                    );

                //Withdrawal fee allows Flat or PercentOfAmount; other savings charges verify validValuesForSavings
                if charge_time_type.is_savings_no_activity_fee()
                    && !charge_calculation_type.is_flat()
                {
                    base.reset()
                        .parameter("chargeCalculationType")
                        .value(calc_val)
                        .is_one_of_these_values([i64::from(ChargeCalculationType::Flat.value())]);
                }
            }
            ChargeAppliesTo::Client => {
                // Client applicable validation (`ChargeTimeType.validClientValues()`: 2)
                base.reset()
                    .parameter("chargeTimeType")
                    .value(time_val)
                    .is_one_of_these_values(ChargeTimeType::valid_client_values().iter().copied());

                // Client calculation type (`ChargeCalculationType.validValuesForClients()`: 1)
                base.reset()
                    .parameter("chargeCalculationType")
                    .value(calc_val)
                    .is_one_of_these_values(
                        ChargeCalculationType::valid_values_for_clients()
                            .iter()
                            .copied(),
                    );
            }
            ChargeAppliesTo::Shares => {
                // Shares applicable validation (`ChargeTimeType.validShareValues()`: 13, 14, 15)
                base.reset()
                    .parameter("chargeTimeType")
                    .value(time_val)
                    .is_one_of_these_values(ChargeTimeType::valid_share_values().iter().copied());

                // Shares calculation type (`ChargeCalculationType.validValuesForShares()`: 1, 2)
                base.reset()
                    .parameter("chargeCalculationType")
                    .value(calc_val)
                    .is_one_of_these_values(
                        ChargeCalculationType::valid_values_for_shares()
                            .iter()
                            .copied(),
                    );

                // ShareAccountActivate (13) only allows FLAT (1)
                if charge_time_type.is_share_account_activate() {
                    base.reset()
                        .parameter("chargeCalculationType")
                        .value(calc_val)
                        .is_one_of_these_values(
                            ChargeCalculationType::valid_values_for_share_account_activation()
                                .iter()
                                .copied(),
                        );
                }
            }
            ChargeAppliesTo::WorkingCapitalLoan | ChargeAppliesTo::Invalid => {
                base.reset()
                    .parameter("chargeAppliesTo")
                    .value(i64::from(charge_applies_to.value()))
                    .is_one_of_these_values(ChargeAppliesTo::valid_values().iter().copied());
            }
        }
    }
}

// =============================================================================
// 9. ChargeCalculationEngine (LoanCharge.java / LoanChargeService.java)
// =============================================================================

/// Engine for validating [`Charge`] definitions and calculating rounded [`Money`] charge amounts.
///
/// Ported from `LoanCharge.java`, `LoanChargeService.java`, and
/// `ChargeDefinitionCommandFromApiJsonDeserializer.java`.
pub struct ChargeCalculationEngine;

impl ChargeCalculationEngine {
    /// Validates [`Charge`] compatibility (`ChargeAppliesTo` / `ChargeTimeType` /
    /// `ChargeCalculationType`, `min_cap`/`max_cap`, and penalty domain rules) and calculates the
    /// charge [`Money`] amount rounded with `mode`.
    ///
    /// ### Calculation Rules
    /// - [`ChargeCalculationType::Flat`]:
    ///   - For [`ChargeTimeType::InstallmentFee`]: `charge.amount * max(installment_count, 1)`
    ///   - Otherwise: `charge.amount`
    /// - [`ChargeCalculationType::PercentOfAmount`]:
    ///   - `(principal * charge.amount) / 100`
    /// - [`ChargeCalculationType::PercentOfAmountAndInterest`]:
    ///   - `((principal + interest) * charge.amount) / 100`
    /// - [`ChargeCalculationType::PercentOfInterest`]:
    ///   - `(interest * charge.amount) / 100`
    /// - [`ChargeCalculationType::PercentOfDisbursementAmount`]:
    ///   - `(base_disbursement * charge.amount) / 100` (where `base_disbursement` is
    ///     `disbursement_amount` if `> 0`, falling back to `principal`)
    /// - **Min / Max Cap Clamping** (`LoanCharge.populateDerivedFields` in Java):
    ///   - For percentage-based calculations, if `charge.min_cap` is set and the computed amount
    ///     is `< min_cap`, it is clamped up to `min_cap`.
    ///   - If `charge.max_cap` is set and the computed amount is `> max_cap`, it is clamped down
    ///     to `max_cap`.
    pub fn calculate_charge_amount(
        charge: &Charge,
        currency: &MonetaryCurrency,
        principal: Decimal,
        interest: Decimal,
        disbursement_amount: Decimal,
        installment_count: usize,
        mode: FineractRoundingMode,
    ) -> Result<Money, FineractError> {
        // 1. Validate ChargeAppliesTo / ChargeTimeType / ChargeCalculationType compatibility
        ChargeDefinitionValidator::validate_charge(charge)?;
        charge.validate_domain_rules()?;

        // 2. Calculate raw unrounded charge amount using exact Decimal arithmetic (no f32/f64)
        let mut raw_amount = match charge.charge_calculation_type {
            ChargeCalculationType::Flat => {
                if charge.charge_time_type.is_installment_fee() {
                    let count = installment_count.max(1);
                    charge.amount * Decimal::from(count)
                } else {
                    charge.amount
                }
            }
            ChargeCalculationType::PercentOfAmount => {
                let base = if charge.charge_time_type.is_on_disbursement()
                    && disbursement_amount > Decimal::ZERO
                    && principal == Decimal::ZERO
                {
                    disbursement_amount
                } else {
                    principal
                };
                Self::percentage_of(base, charge.amount)
            }
            ChargeCalculationType::PercentOfAmountAndInterest => {
                let total_base = principal + interest;
                Self::percentage_of(total_base, charge.amount)
            }
            ChargeCalculationType::PercentOfInterest => {
                Self::percentage_of(interest, charge.amount)
            }
            ChargeCalculationType::PercentOfDisbursementAmount => {
                let base = if disbursement_amount > Decimal::ZERO {
                    disbursement_amount
                } else {
                    principal
                };
                Self::percentage_of(base, charge.amount)
            }
            ChargeCalculationType::Invalid => {
                return Err(PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.charge.chargeCalculationType.is.not.one.of.expected.enumerations",
                    "The parameter `chargeCalculationType` is not a valid charge calculation type.",
                    "chargeCalculationType",
                    vec![ApiErrorArg::from(charge.charge_calculation_type.value())],
                )
                .into());
            }
        };

        // 3. Enforce min_cap and max_cap clamping
        if let Some(min_cap) = charge.min_cap {
            if raw_amount < min_cap {
                raw_amount = min_cap;
            }
        }
        if let Some(max_cap) = charge.max_cap {
            if raw_amount > max_cap {
                raw_amount = max_cap;
            }
        }

        // 4. Construct rounded Money in target currency
        Ok(Money::of(currency.clone(), raw_amount, mode))
    }

    /// Computes `(value * percentage) / 100` using exact `Decimal` math (`LoanCharge.percentageOf`).
    #[inline]
    pub fn percentage_of(value: Decimal, percentage: Decimal) -> Decimal {
        if value > Decimal::ZERO && percentage > Decimal::ZERO {
            (value * percentage) / Decimal::ONE_HUNDRED
        } else {
            Decimal::ZERO
        }
    }
}

// =============================================================================
// 10. ChargeRepository, InMemoryChargeRepository & DualDbChargeRepository
// =============================================================================

/// Async persistence contract for `m_charge`.
#[async_trait]
pub trait ChargeRepository: Send + Sync {
    /// Looks up a non-deleted [`Charge`] by primary key `id`.
    async fn find_by_id(&self, id: i64) -> FineractResult<Option<Charge>>;

    /// Returns all non-deleted [`Charge`] definitions ordered by `id`.
    async fn find_all(&self) -> FineractResult<Vec<Charge>>;

    /// Returns `true` if a non-deleted [`Charge`] with `name` already exists (excluding `exclude_id`).
    async fn exists_by_name(&self, name: &str, exclude_id: Option<i64>) -> FineractResult<bool>;

    /// Inserts a new [`Charge`] into `m_charge`, assigning a new `id` if `charge.id == 0`.
    async fn insert(&self, charge: Charge) -> FineractResult<Charge>;

    /// Updates an existing [`Charge`] in `m_charge`.
    async fn update(&self, charge: Charge) -> FineractResult<Charge>;

    /// Soft-deletes a [`Charge`] in `m_charge` (`is_deleted = true`, prefixed name).
    async fn soft_delete(&self, id: i64) -> FineractResult<Charge>;
}

/// Fast concurrent in-memory implementation of [`ChargeRepository`] backed by [`DashMap`].
#[derive(Debug)]
pub struct InMemoryChargeRepository {
    charges: DashMap<i64, Charge>,
    next_id: AtomicI64,
}

impl Default for InMemoryChargeRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryChargeRepository {
    pub fn new() -> Self {
        Self {
            charges: DashMap::new(),
            next_id: AtomicI64::new(1),
        }
    }

    pub fn len(&self) -> usize {
        self.charges.iter().filter(|e| !e.value().is_deleted).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl ChargeRepository for InMemoryChargeRepository {
    async fn find_by_id(&self, id: i64) -> FineractResult<Option<Charge>> {
        Ok(self
            .charges
            .get(&id)
            .map(|e| e.value().clone())
            .filter(|c| !c.is_deleted))
    }

    async fn find_all(&self) -> FineractResult<Vec<Charge>> {
        let mut items: Vec<Charge> = self
            .charges
            .iter()
            .map(|e| e.value().clone())
            .filter(|c| !c.is_deleted)
            .collect();
        items.sort_by_key(|c| c.id);
        Ok(items)
    }

    async fn exists_by_name(&self, name: &str, exclude_id: Option<i64>) -> FineractResult<bool> {
        Ok(self.charges.iter().any(|e| {
            let c = e.value();
            !c.is_deleted
                && c.name.eq_ignore_ascii_case(name)
                && exclude_id.map_or(true, |ex| c.id != ex)
        }))
    }

    async fn insert(&self, mut charge: Charge) -> FineractResult<Charge> {
        if self.exists_by_name(&charge.name, None).await? {
            return Err(charge_name_already_exists_error(&charge.name));
        }
        if charge.id <= 0 {
            charge.id = self.next_id.fetch_add(1, Ordering::SeqCst);
        } else {
            let _ = self
                .next_id
                .fetch_max(charge.id.saturating_add(1), Ordering::SeqCst);
        }
        self.charges.insert(charge.id, charge.clone());
        Ok(charge)
    }

    async fn update(&self, charge: Charge) -> FineractResult<Charge> {
        if !self.charges.contains_key(&charge.id) {
            return Err(charge_not_found_error(charge.id));
        }
        if self.exists_by_name(&charge.name, Some(charge.id)).await? {
            return Err(charge_name_already_exists_error(&charge.name));
        }
        self.charges.insert(charge.id, charge.clone());
        Ok(charge)
    }

    async fn soft_delete(&self, id: i64) -> FineractResult<Charge> {
        let mut entry = self
            .charges
            .get_mut(&id)
            .ok_or_else(|| charge_not_found_error(id))?;
        if entry.is_deleted {
            return Err(charge_not_found_error(id));
        }
        entry.delete();
        Ok(entry.clone())
    }
}

/// Dual-Database (`PostgreSQL` + `MySQL/MariaDB`) repository for `m_charge` backed by [`AnyPool`].
#[derive(Debug, Clone)]
pub struct DualDbChargeRepository {
    pool: AnyPool,
    dialect: SqlDialect,
}

impl DualDbChargeRepository {
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self { pool, dialect }
    }

    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub const fn pool(&self) -> &AnyPool {
        &self.pool
    }

    /// Generates the dialect-specific projection list for `m_charge`, casting `DECIMAL(19,6)`
    /// columns to text so `sqlx::AnyRow` decodes exact [`Decimal`] strings without float loss.
    pub fn select_columns(dialect: SqlDialect) -> &'static str {
        match dialect {
            SqlDialect::Postgres => {
                "id, name, currency_code, charge_applies_to_enum, charge_time_enum, \
                 charge_calculation_enum, charge_payment_mode_enum, \
                 CAST(amount AS TEXT) AS amount_str, fee_on_day, fee_interval, fee_on_month, \
                 is_penalty, is_active, is_deleted, \
                 CAST(min_cap AS TEXT) AS min_cap_str, CAST(max_cap AS TEXT) AS max_cap_str, \
                 fee_frequency, income_or_liability_account_id, tax_group_id"
            }
            SqlDialect::MySql => {
                "id, name, currency_code, charge_applies_to_enum, charge_time_enum, \
                 charge_calculation_enum, charge_payment_mode_enum, \
                 CAST(amount AS CHAR) AS amount_str, fee_on_day, fee_interval, fee_on_month, \
                 is_penalty, is_active, is_deleted, \
                 CAST(min_cap AS CHAR) AS min_cap_str, CAST(max_cap AS CHAR) AS max_cap_str, \
                 fee_frequency, income_or_liability_account_id, tax_group_id"
            }
        }
    }

    /// Dialect-aware SQL for `find_by_id`.
    pub fn select_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM m_charge WHERE id = ? AND is_deleted = FALSE",
            Self::select_columns(dialect)
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    /// Dialect-aware SQL for `find_all`.
    pub fn select_all_sql(dialect: SqlDialect) -> String {
        format!(
            "SELECT {} FROM m_charge WHERE is_deleted = FALSE ORDER BY id ASC",
            Self::select_columns(dialect)
        )
    }

    /// Dialect-aware SQL for `insert`.
    pub fn insert_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO m_charge (\
            id, name, currency_code, charge_applies_to_enum, charge_time_enum, \
            charge_calculation_enum, charge_payment_mode_enum, amount, \
            fee_on_day, fee_interval, fee_on_month, is_penalty, is_active, is_deleted, \
            min_cap, max_cap, fee_frequency, income_or_liability_account_id, tax_group_id\
        ) VALUES (\
            ?, ?, ?, ?, ?, ?, ?, CAST(? AS DECIMAL(19, 6)), \
            ?, ?, ?, ?, ?, ?, \
            CAST(? AS DECIMAL(19, 6)), CAST(? AS DECIMAL(19, 6)), ?, ?, ?\
        )";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Dialect-aware SQL for `update`.
    pub fn update_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_charge SET \
            name = ?, currency_code = ?, charge_applies_to_enum = ?, charge_time_enum = ?, \
            charge_calculation_enum = ?, charge_payment_mode_enum = ?, \
            amount = CAST(? AS DECIMAL(19, 6)), fee_on_day = ?, fee_interval = ?, \
            fee_on_month = ?, is_penalty = ?, is_active = ?, is_deleted = ?, \
            min_cap = CAST(? AS DECIMAL(19, 6)), max_cap = CAST(? AS DECIMAL(19, 6)), \
            fee_frequency = ?, income_or_liability_account_id = ?, tax_group_id = ? \
            WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    /// Dialect-aware SQL for `soft_delete`.
    pub fn soft_delete_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE m_charge SET is_deleted = TRUE, name = ? WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    fn parse_decimal_col(raw: Option<String>, col_name: &str) -> FineractResult<Option<Decimal>> {
        match raw {
            Some(s) if !s.trim().is_empty() => Decimal::from_str(s.trim()).map(Some).map_err(|e| {
                FineractError::DatabaseError(format!(
                    "Invalid DECIMAL in column `{col_name}` (`{s}`): {e}"
                ))
            }),
            _ => Ok(None),
        }
    }

    fn map_row(row: &sqlx::any::AnyRow) -> FineractResult<Charge> {
        let id: i64 = row
            .try_get("id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let name: Option<String> = row
            .try_get("name")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let currency_code: String = row
            .try_get("currency_code")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let charge_applies_to_enum: i16 = row
            .try_get("charge_applies_to_enum")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let charge_time_enum: i16 = row
            .try_get("charge_time_enum")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let charge_calculation_enum: i16 = row
            .try_get("charge_calculation_enum")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let charge_payment_mode_enum: Option<i16> = row
            .try_get("charge_payment_mode_enum")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let amount_str: Option<String> = row
            .try_get("amount_str")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let amount = Self::parse_decimal_col(amount_str, "amount")?.unwrap_or(Decimal::ZERO);

        let fee_on_day_raw: Option<i16> = row
            .try_get("fee_on_day")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let fee_interval_raw: Option<i16> = row
            .try_get("fee_interval")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let fee_on_month_raw: Option<i16> = row
            .try_get("fee_on_month")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let is_penalty: bool = row
            .try_get("is_penalty")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let is_active: bool = row
            .try_get("is_active")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let is_deleted: bool = row
            .try_get("is_deleted")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let min_cap_str: Option<String> = row
            .try_get("min_cap_str")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let max_cap_str: Option<String> = row
            .try_get("max_cap_str")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        let min_cap = Self::parse_decimal_col(min_cap_str, "min_cap")?;
        let max_cap = Self::parse_decimal_col(max_cap_str, "max_cap")?;

        let fee_frequency_raw: Option<i16> = row
            .try_get("fee_frequency")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let income_or_liability_account_id: Option<i64> = row
            .try_get("income_or_liability_account_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let tax_group_id: Option<i64> = row
            .try_get("tax_group_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(Charge {
            id,
            name: name.unwrap_or_default(),
            currency_code,
            charge_applies_to: ChargeAppliesTo::from_i16(charge_applies_to_enum),
            charge_time_type: ChargeTimeType::from_i16(charge_time_enum),
            charge_calculation_type: ChargeCalculationType::from_i16(charge_calculation_enum),
            charge_payment_mode: ChargePaymentMode::from_i16(
                charge_payment_mode_enum.unwrap_or(0),
            ),
            amount,
            is_penalty,
            is_active,
            is_deleted,
            min_cap,
            max_cap,
            fee_frequency: fee_frequency_raw.map(FeeFrequencyType::from_i16),
            fee_interval: fee_interval_raw.map(i32::from),
            fee_on_month: fee_on_month_raw.and_then(|v| u8::try_from(v).ok()),
            fee_on_day: fee_on_day_raw.and_then(|v| u8::try_from(v).ok()),
            income_or_liability_account_id,
            tax_group_id,
        })
    }
}

#[async_trait]
impl ChargeRepository for DualDbChargeRepository {
    async fn find_by_id(&self, id: i64) -> FineractResult<Option<Charge>> {
        let sql = Self::select_by_id_sql(self.dialect);
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        row.as_ref().map(Self::map_row).transpose()
    }

    async fn find_all(&self) -> FineractResult<Vec<Charge>> {
        let sql = Self::select_all_sql(self.dialect);
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        rows.iter().map(Self::map_row).collect()
    }

    async fn exists_by_name(&self, name: &str, exclude_id: Option<i64>) -> FineractResult<bool> {
        let raw = if exclude_id.is_some() {
            "SELECT COUNT(1) AS cnt FROM m_charge WHERE LOWER(name) = LOWER(?) AND is_deleted = FALSE AND id <> ?"
        } else {
            "SELECT COUNT(1) AS cnt FROM m_charge WHERE LOWER(name) = LOWER(?) AND is_deleted = FALSE"
        };
        let sql = DialectSql::rewrite_placeholders(raw, self.dialect);
        let mut q = sqlx::query(&sql).bind(name);
        if let Some(ex) = exclude_id {
            q = q.bind(ex);
        }
        let row = q
            .fetch_one(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let cnt: i64 = row
            .try_get("cnt")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        Ok(cnt > 0)
    }

    async fn insert(&self, mut charge: Charge) -> FineractResult<Charge> {
        if self.exists_by_name(&charge.name, None).await? {
            return Err(charge_name_already_exists_error(&charge.name));
        }

        if charge.id <= 0 {
            let next_row = sqlx::query("SELECT COALESCE(MAX(id), 0) + 1 AS next_id FROM m_charge")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            charge.id = next_row
                .try_get("next_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        let sql = Self::insert_sql(self.dialect);
        sqlx::query(&sql)
            .bind(charge.id)
            .bind(&charge.name)
            .bind(&charge.currency_code)
            .bind(charge.charge_applies_to.as_i16())
            .bind(charge.charge_time_type.as_i16())
            .bind(charge.charge_calculation_type.as_i16())
            .bind(charge.charge_payment_mode.as_i16())
            .bind(charge.amount.to_string())
            .bind(charge.fee_on_day.map(i16::from))
            .bind(charge.fee_interval.and_then(|v| i16::try_from(v).ok()))
            .bind(charge.fee_on_month.map(i16::from))
            .bind(charge.is_penalty)
            .bind(charge.is_active)
            .bind(charge.is_deleted)
            .bind(charge.min_cap.map(|d| d.to_string()))
            .bind(charge.max_cap.map(|d| d.to_string()))
            .bind(charge.fee_frequency.map(FeeFrequencyType::as_i16))
            .bind(charge.income_or_liability_account_id)
            .bind(charge.tax_group_id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(charge)
    }

    async fn update(&self, charge: Charge) -> FineractResult<Charge> {
        if self.exists_by_name(&charge.name, Some(charge.id)).await? {
            return Err(charge_name_already_exists_error(&charge.name));
        }

        let sql = Self::update_sql(self.dialect);
        let result = sqlx::query(&sql)
            .bind(&charge.name)
            .bind(&charge.currency_code)
            .bind(charge.charge_applies_to.as_i16())
            .bind(charge.charge_time_type.as_i16())
            .bind(charge.charge_calculation_type.as_i16())
            .bind(charge.charge_payment_mode.as_i16())
            .bind(charge.amount.to_string())
            .bind(charge.fee_on_day.map(i16::from))
            .bind(charge.fee_interval.and_then(|v| i16::try_from(v).ok()))
            .bind(charge.fee_on_month.map(i16::from))
            .bind(charge.is_penalty)
            .bind(charge.is_active)
            .bind(charge.is_deleted)
            .bind(charge.min_cap.map(|d| d.to_string()))
            .bind(charge.max_cap.map(|d| d.to_string()))
            .bind(charge.fee_frequency.map(FeeFrequencyType::as_i16))
            .bind(charge.income_or_liability_account_id)
            .bind(charge.tax_group_id)
            .bind(charge.id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(charge_not_found_error(charge.id));
        }

        Ok(charge)
    }

    async fn soft_delete(&self, id: i64) -> FineractResult<Charge> {
        let mut charge = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| charge_not_found_error(id))?;
        charge.delete();

        let sql = Self::soft_delete_sql(self.dialect);
        sqlx::query(&sql)
            .bind(&charge.name)
            .bind(charge.id)
            .execute(&self.pool)
            .await
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(charge)
    }
}

// =============================================================================
// 11. Command Handlers (Create / Update / Delete Charge Definition)
// =============================================================================

/// Command handler for `CREATE` `CHARGE` (`CreateChargeDefinitionCommandHandler.java`).
pub struct CreateChargeDefinitionCommandHandler {
    repository: Arc<dyn ChargeRepository>,
}

impl CreateChargeDefinitionCommandHandler {
    pub fn new(repository: Arc<dyn ChargeRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl CommandHandler for CreateChargeDefinitionCommandHandler {
    fn action_name(&self) -> &str {
        "CREATE"
    }

    fn entity_name(&self) -> &str {
        "CHARGE"
    }

    async fn process_command(
        &self,
        command: &JsonCommand,
        _context: &ExecutionContext,
    ) -> Result<CommandProcessingResult, FineractError> {
        let charge = Charge::from_json(0, command)?;
        let saved = self.repository.insert(charge).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for `UPDATE` `CHARGE` (`UpdateChargeDefinitionCommandHandler.java`).
pub struct UpdateChargeDefinitionCommandHandler {
    repository: Arc<dyn ChargeRepository>,
}

impl UpdateChargeDefinitionCommandHandler {
    pub fn new(repository: Arc<dyn ChargeRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl CommandHandler for UpdateChargeDefinitionCommandHandler {
    fn action_name(&self) -> &str {
        "UPDATE"
    }

    fn entity_name(&self) -> &str {
        "CHARGE"
    }

    async fn process_command(
        &self,
        command: &JsonCommand,
        _context: &ExecutionContext,
    ) -> Result<CommandProcessingResult, FineractError> {
        let charge_id = command
            .entity_id
            .ok_or_else(|| charge_not_found_error(0))?;
        let mut charge = self
            .repository
            .find_by_id(charge_id)
            .await?
            .ok_or_else(|| charge_not_found_error(charge_id))?;

        let changes = charge.update(command)?;
        if !changes.is_empty() {
            self.repository.update(charge).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(charge_id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `DELETE` `CHARGE` (`DeleteChargeDefinitionCommandHandler.java`).
pub struct DeleteChargeDefinitionCommandHandler {
    repository: Arc<dyn ChargeRepository>,
}

impl DeleteChargeDefinitionCommandHandler {
    pub fn new(repository: Arc<dyn ChargeRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl CommandHandler for DeleteChargeDefinitionCommandHandler {
    fn action_name(&self) -> &str {
        "DELETE"
    }

    fn entity_name(&self) -> &str {
        "CHARGE"
    }

    async fn process_command(
        &self,
        command: &JsonCommand,
        _context: &ExecutionContext,
    ) -> Result<CommandProcessingResult, FineractError> {
        let charge_id = command
            .entity_id
            .ok_or_else(|| charge_not_found_error(0))?;
        let deleted = self.repository.soft_delete(charge_id).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(deleted.id)
            .build())
    }
}

// =============================================================================
// 12. Comprehensive Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use fineract_money::dec;

    fn sample_loan_charge(
        time_type: ChargeTimeType,
        calc_type: ChargeCalculationType,
        amount: Decimal,
        min_cap: Option<Decimal>,
        max_cap: Option<Decimal>,
    ) -> Charge {
        Charge {
            id: 1,
            name: "Processing Fee".to_string(),
            currency_code: "USD".to_string(),
            charge_applies_to: ChargeAppliesTo::Loan,
            charge_time_type: time_type,
            charge_calculation_type: calc_type,
            charge_payment_mode: ChargePaymentMode::Regular,
            amount,
            is_penalty: time_type == ChargeTimeType::OverdueInstallment,
            is_active: true,
            is_deleted: false,
            min_cap,
            max_cap,
            fee_frequency: None,
            fee_interval: None,
            fee_on_month: None,
            fee_on_day: None,
            income_or_liability_account_id: None,
            tax_group_id: None,
        }
    }

    #[test]
    fn test_enums_wire_codes_and_values() {
        assert_eq!(ChargeAppliesTo::Loan.value(), 1);
        assert_eq!(ChargeAppliesTo::Savings.value(), 2);
        assert_eq!(ChargeAppliesTo::Client.value(), 3);
        assert_eq!(ChargeAppliesTo::Shares.value(), 4);
        assert_eq!(ChargeAppliesTo::from_i16(1), ChargeAppliesTo::Loan);
        assert_eq!(ChargeAppliesTo::from_i16(99), ChargeAppliesTo::Invalid);

        assert_eq!(ChargeTimeType::Disbursement.value(), 1);
        assert_eq!(ChargeTimeType::SpecifiedDueDate.value(), 2);
        assert_eq!(ChargeTimeType::SavingsActivation.value(), 3);
        assert_eq!(ChargeTimeType::SavingsClosure.value(), 4);
        assert_eq!(ChargeTimeType::WithdrawalFee.value(), 5);
        assert_eq!(ChargeTimeType::AnnualFee.value(), 6);
        assert_eq!(ChargeTimeType::MonthlyFee.value(), 7);
        assert_eq!(ChargeTimeType::InstallmentFee.value(), 8);
        assert_eq!(ChargeTimeType::OverdueInstallment.value(), 9);
        assert_eq!(ChargeTimeType::OverdraftFee.value(), 10);
        assert_eq!(ChargeTimeType::WeeklyFee.value(), 11);
        assert_eq!(ChargeTimeType::TrancheDisbursement.value(), 12);
        assert_eq!(ChargeTimeType::ShareAccountActivate.value(), 13);
        assert_eq!(ChargeTimeType::SharePurchase.value(), 14);
        assert_eq!(ChargeTimeType::ShareRedeem.value(), 15);
        assert_eq!(ChargeTimeType::SavingsNoActivityFee.value(), 16);
        assert_eq!(ChargeTimeType::LineOfCreditActivation.value(), 17);

        assert_eq!(ChargeCalculationType::Flat.value(), 1);
        assert_eq!(ChargeCalculationType::PercentOfAmount.value(), 2);
        assert_eq!(ChargeCalculationType::PercentOfAmountAndInterest.value(), 3);
        assert_eq!(ChargeCalculationType::PercentOfInterest.value(), 4);
        assert_eq!(ChargeCalculationType::PercentOfDisbursementAmount.value(), 5);
        assert!(ChargeCalculationType::PercentOfAmountAndInterest.has_interest());
        assert!(ChargeCalculationType::PercentOfInterest.has_interest());
        assert!(!ChargeCalculationType::PercentOfAmount.has_interest());

        assert_eq!(ChargePaymentMode::Regular.value(), 0);
        assert_eq!(ChargePaymentMode::AccountTransfer.value(), 1);
        assert!(ChargePaymentMode::AccountTransfer.is_payment_mode_account_transfer());

        assert_eq!(FeeFrequencyType::Days.value(), 0);
        assert_eq!(FeeFrequencyType::Weeks.value(), 1);
        assert_eq!(FeeFrequencyType::Months.value(), 2);
        assert_eq!(FeeFrequencyType::Years.value(), 3);
        assert_eq!(FeeFrequencyType::WholeTerm.value(), 4);
        assert_eq!(FeeFrequencyType::Invalid.value(), 5);
    }

    #[test]
    fn test_flat_charge_calculation_disbursement_and_installment() {
        let usd = MonetaryCurrency::usd();
        let mode = FineractRoundingMode::HalfEven;

        // Flat fee at disbursement: fixed 50.00 regardless of principal/installments
        let disb_charge = sample_loan_charge(
            ChargeTimeType::Disbursement,
            ChargeCalculationType::Flat,
            dec!(50.00),
            None,
            None,
        );
        let result = ChargeCalculationEngine::calculate_charge_amount(
            &disb_charge,
            &usd,
            dec!(10000.00),
            dec!(1200.00),
            dec!(10000.00),
            12,
            mode,
        )
        .expect("flat disbursement charge should succeed");
        assert_eq!(result.amount(), dec!(50.00));

        // Flat installment fee across 12 installments: 15.00 * 12 = 180.00
        let inst_charge = sample_loan_charge(
            ChargeTimeType::InstallmentFee,
            ChargeCalculationType::Flat,
            dec!(15.00),
            None,
            None,
        );
        let inst_result = ChargeCalculationEngine::calculate_charge_amount(
            &inst_charge,
            &usd,
            dec!(10000.00),
            dec!(1200.00),
            dec!(10000.00),
            12,
            mode,
        )
        .expect("flat installment charge should succeed");
        assert_eq!(inst_result.amount(), dec!(180.00));
    }

    #[test]
    fn test_percent_of_amount_and_percent_of_disbursement_calculation() {
        let usd = MonetaryCurrency::usd();
        let mode = FineractRoundingMode::HalfEven;

        // 2.5% of principal 10,000.00 = 250.00
        let pct_amount_charge = sample_loan_charge(
            ChargeTimeType::SpecifiedDueDate,
            ChargeCalculationType::PercentOfAmount,
            dec!(2.5),
            None,
            None,
        );
        let res = ChargeCalculationEngine::calculate_charge_amount(
            &pct_amount_charge,
            &usd,
            dec!(10000.00),
            dec!(1500.00),
            dec!(10000.00),
            10,
            mode,
        )
        .expect("percent of amount should succeed");
        assert_eq!(res.amount(), dec!(250.00));

        // 1.75% of tranche disbursement 4,000.00 = 70.00
        let tranche_charge = sample_loan_charge(
            ChargeTimeType::TrancheDisbursement,
            ChargeCalculationType::PercentOfDisbursementAmount,
            dec!(1.75),
            None,
            None,
        );
        let tranche_res = ChargeCalculationEngine::calculate_charge_amount(
            &tranche_charge,
            &usd,
            dec!(10000.00),
            dec!(1500.00),
            dec!(4000.00),
            10,
            mode,
        )
        .expect("percent of disbursement amount should succeed");
        assert_eq!(tranche_res.amount(), dec!(70.00));
    }

    #[test]
    fn test_percent_of_amount_and_interest_and_percent_of_interest_calculation() {
        let usd = MonetaryCurrency::usd();
        let mode = FineractRoundingMode::HalfUp;

        // 3% of (principal 10,000 + interest 2,000 = 12,000) = 360.00
        let pct_both = sample_loan_charge(
            ChargeTimeType::InstallmentFee,
            ChargeCalculationType::PercentOfAmountAndInterest,
            dec!(3.0),
            None,
            None,
        );
        let both_res = ChargeCalculationEngine::calculate_charge_amount(
            &pct_both,
            &usd,
            dec!(10000.00),
            dec!(2000.00),
            dec!(10000.00),
            12,
            mode,
        )
        .expect("percent of amount and interest should succeed");
        assert_eq!(both_res.amount(), dec!(360.00));

        // 5% of interest 2,000 = 100.00
        let pct_interest = sample_loan_charge(
            ChargeTimeType::OverdueInstallment,
            ChargeCalculationType::PercentOfInterest,
            dec!(5.0),
            None,
            None,
        );
        let int_res = ChargeCalculationEngine::calculate_charge_amount(
            &pct_interest,
            &usd,
            dec!(10000.00),
            dec!(2000.00),
            dec!(10000.00),
            12,
            mode,
        )
        .expect("percent of interest should succeed");
        assert_eq!(int_res.amount(), dec!(100.00));
    }

    #[test]
    fn test_min_cap_and_max_cap_clamping() {
        let usd = MonetaryCurrency::usd();
        let mode = FineractRoundingMode::HalfEven;

        // 2% with min_cap = 50.00, max_cap = 200.00
        let capped_charge = sample_loan_charge(
            ChargeTimeType::Disbursement,
            ChargeCalculationType::PercentOfAmount,
            dec!(2.0),
            Some(dec!(50.00)),
            Some(dec!(200.00)),
        );

        // Case 1: Principal = 1,000 -> 2% = 20.00 -> clamped up to min_cap (50.00)
        let low = ChargeCalculationEngine::calculate_charge_amount(
            &capped_charge,
            &usd,
            dec!(1000.00),
            dec!(100.00),
            dec!(1000.00),
            12,
            mode,
        )
        .expect("min_cap clamping should succeed");
        assert_eq!(low.amount(), dec!(50.00));

        // Case 2: Principal = 5,000 -> 2% = 100.00 -> within [50.00, 200.00]
        let mid = ChargeCalculationEngine::calculate_charge_amount(
            &capped_charge,
            &usd,
            dec!(5000.00),
            dec!(500.00),
            dec!(5000.00),
            12,
            mode,
        )
        .expect("in-range calculation should succeed");
        assert_eq!(mid.amount(), dec!(100.00));

        // Case 3: Principal = 25,000 -> 2% = 500.00 -> clamped down to max_cap (200.00)
        let high = ChargeCalculationEngine::calculate_charge_amount(
            &capped_charge,
            &usd,
            dec!(25000.00),
            dec!(2500.00),
            dec!(25000.00),
            12,
            mode,
        )
        .expect("max_cap clamping should succeed");
        assert_eq!(high.amount(), dec!(200.00));
    }

    #[test]
    fn test_invalid_loan_savings_client_share_charge_time_and_calculation_validation_errors() {
        let usd = MonetaryCurrency::usd();
        let mode = FineractRoundingMode::HalfEven;

        // 1. Loan charge with Savings time-type (WithdrawalFee) -> DataValidation error
        let mut invalid_loan_time = sample_loan_charge(
            ChargeTimeType::WithdrawalFee,
            ChargeCalculationType::Flat,
            dec!(10.0),
            None,
            None,
        );
        invalid_loan_time.is_penalty = false;
        let err = ChargeCalculationEngine::calculate_charge_amount(
            &invalid_loan_time,
            &usd,
            dec!(1000.0),
            dec!(100.0),
            dec!(1000.0),
            1,
            mode,
        )
        .expect_err("loan charge with withdrawal fee time type must fail");
        assert!(matches!(err, FineractError::DataValidation(_)));

        // 2. Loan SpecifiedDueDate with PercentOfDisbursementAmount -> DataValidation error
        let invalid_loan_calc = sample_loan_charge(
            ChargeTimeType::SpecifiedDueDate,
            ChargeCalculationType::PercentOfDisbursementAmount,
            dec!(2.0),
            None,
            None,
        );
        let err = ChargeCalculationEngine::calculate_charge_amount(
            &invalid_loan_calc,
            &usd,
            dec!(1000.0),
            dec!(100.0),
            dec!(1000.0),
            1,
            mode,
        )
        .expect_err("specified due date with percent of disbursement must fail");
        assert!(matches!(err, FineractError::DataValidation(_)));

        // 3. Savings charge with Loan time-type (Disbursement) or PercentOfInterest -> DataValidation error
        let mut invalid_savings = sample_loan_charge(
            ChargeTimeType::Disbursement,
            ChargeCalculationType::Flat,
            dec!(10.0),
            None,
            None,
        );
        invalid_savings.charge_applies_to = ChargeAppliesTo::Savings;
        let err = ChargeCalculationEngine::calculate_charge_amount(
            &invalid_savings,
            &usd,
            dec!(1000.0),
            dec!(0.0),
            dec!(0.0),
            1,
            mode,
        )
        .expect_err("savings charge with disbursement time type must fail");
        assert!(matches!(err, FineractError::DataValidation(_)));

        let mut invalid_savings_calc = sample_loan_charge(
            ChargeTimeType::WithdrawalFee,
            ChargeCalculationType::PercentOfInterest,
            dec!(2.0),
            None,
            None,
        );
        invalid_savings_calc.charge_applies_to = ChargeAppliesTo::Savings;
        let err = ChargeCalculationEngine::calculate_charge_amount(
            &invalid_savings_calc,
            &usd,
            dec!(1000.0),
            dec!(50.0),
            dec!(0.0),
            1,
            mode,
        )
        .expect_err("savings charge with percent of interest must fail");
        assert!(matches!(err, FineractError::DataValidation(_)));

        // 4. Client charge with MonthlyFee or PercentOfAmount -> DataValidation error
        let mut invalid_client_time = sample_loan_charge(
            ChargeTimeType::MonthlyFee,
            ChargeCalculationType::Flat,
            dec!(15.0),
            None,
            None,
        );
        invalid_client_time.charge_applies_to = ChargeAppliesTo::Client;
        let err = ChargeCalculationEngine::calculate_charge_amount(
            &invalid_client_time,
            &usd,
            dec!(500.0),
            dec!(0.0),
            dec!(0.0),
            1,
            mode,
        )
        .expect_err("client charge with monthly fee time type must fail");
        assert!(matches!(err, FineractError::DataValidation(_)));

        let mut invalid_client_calc = sample_loan_charge(
            ChargeTimeType::SpecifiedDueDate,
            ChargeCalculationType::PercentOfAmount,
            dec!(5.0),
            None,
            None,
        );
        invalid_client_calc.charge_applies_to = ChargeAppliesTo::Client;
        let err = ChargeCalculationEngine::calculate_charge_amount(
            &invalid_client_calc,
            &usd,
            dec!(500.0),
            dec!(0.0),
            dec!(0.0),
            1,
            mode,
        )
        .expect_err("client charge with percent of amount must fail");
        assert!(matches!(err, FineractError::DataValidation(_)));

        // 5. Share charge with ShareAccountActivate + PercentOfAmount -> DataValidation error
        let mut invalid_share_calc = sample_loan_charge(
            ChargeTimeType::ShareAccountActivate,
            ChargeCalculationType::PercentOfAmount,
            dec!(2.0),
            None,
            None,
        );
        invalid_share_calc.charge_applies_to = ChargeAppliesTo::Shares;
        let err = ChargeCalculationEngine::calculate_charge_amount(
            &invalid_share_calc,
            &usd,
            dec!(1000.0),
            dec!(0.0),
            dec!(0.0),
            1,
            mode,
        )
        .expect_err("share account activate charge with percent of amount must fail");
        assert!(matches!(err, FineractError::DataValidation(_)));

        // 6. Loan penalty at disbursement -> DomainRule error
        let mut penalty_at_disb = sample_loan_charge(
            ChargeTimeType::Disbursement,
            ChargeCalculationType::Flat,
            dec!(25.0),
            None,
            None,
        );
        penalty_at_disb.is_penalty = true;
        let err = ChargeCalculationEngine::calculate_charge_amount(
            &penalty_at_disb,
            &usd,
            dec!(1000.0),
            dec!(0.0),
            dec!(1000.0),
            1,
            mode,
        )
        .expect_err("penalty at disbursement must fail domain rule");
        assert!(matches!(err, FineractError::DomainRule(_)));
    }

    #[tokio::test]
    async fn test_charge_command_handlers_create_update_delete() {
        let repo = Arc::new(InMemoryChargeRepository::new());
        let create_handler = CreateChargeDefinitionCommandHandler::new(repo.clone());
        let update_handler = UpdateChargeDefinitionCommandHandler::new(repo.clone());
        let delete_handler = DeleteChargeDefinitionCommandHandler::new(repo.clone());
        let ctx = ExecutionContext::new(
            "default",
            NaiveDate::from_ymd_opt(2026, 10, 30).expect("valid date"),
        );

        let create_cmd = JsonCommand::from_value(
            None,
            json!({
                "name": "Loan Origination Fee",
                "currencyCode": "USD",
                "chargeAppliesTo": 1,
                "chargeTimeType": 1,
                "chargeCalculationType": 2,
                "chargePaymentMode": 0,
                "amount": 2.5,
                "minCap": 25.0,
                "maxCap": 250.0,
                "penalty": false,
                "active": true,
                "locale": "en"
            }),
        )
        .with_command_id(101);

        let create_res = create_handler
            .process_command(&create_cmd, &ctx)
            .await
            .expect("create charge should succeed");
        let charge_id = create_res.resource_id.expect("resource_id present");

        let stored = repo
            .find_by_id(charge_id)
            .await
            .expect("repo lookup")
            .expect("charge exists");
        assert_eq!(stored.name, "Loan Origination Fee");
        assert_eq!(stored.amount, dec!(2.5));
        assert_eq!(stored.min_cap, Some(dec!(25.0)));
        assert_eq!(stored.max_cap, Some(dec!(250.0)));
        assert!(!stored.is_Deleted());

        // Update amount and maxCap
        let update_cmd = JsonCommand::from_value(
            Some(charge_id),
            json!({
                "amount": 3.0,
                "maxCap": 300.0,
                "locale": "en"
            }),
        )
        .with_command_id(102);

        let update_res = update_handler
            .process_command(&update_cmd, &ctx)
            .await
            .expect("update charge should succeed");
        assert!(update_res.has_changes());

        let updated = repo
            .find_by_id(charge_id)
            .await
            .expect("repo lookup")
            .expect("charge exists");
        assert_eq!(updated.amount, dec!(3.0));
        assert_eq!(updated.max_cap, Some(dec!(300.0)));

        // Soft-delete charge
        let delete_cmd = JsonCommand::from_value(Some(charge_id), json!({})).with_command_id(103);
        let delete_res = delete_handler
            .process_command(&delete_cmd, &ctx)
            .await
            .expect("delete charge should succeed");
        assert_eq!(delete_res.resource_id, Some(charge_id));
        assert!(repo.find_by_id(charge_id).await.expect("lookup").is_none());
    }
}
