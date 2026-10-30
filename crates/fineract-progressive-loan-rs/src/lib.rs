//! Apache Fineract Progressive Loan Engine, Dynamic Interest Period Model,
//! and Advanced Payment Allocation Rules (`fineract-progressive-loan-rs`).
//!
//! Ported from `fineract-progressive-loan` and `fineract-loan` payment allocation
//! modules:
//! - [`PaymentAllocationTransactionType`]
//! - [`PaymentAllocationType`], [`DueType`], [`AllocationType`]
//! - [`FutureInstallmentAllocationRule`]
//! - [`LoanPaymentAllocationRule`] (`m_loan_payment_allocation_rule`)
//! - [`CreditAllocationType`], [`CreditAllocationTransactionType`], [`LoanCreditAllocationRule`] (`m_loan_credit_allocation_rule`)
//! - [`InterestPeriod`] & [`RepaymentPeriod`]
//! - [`ProgressiveLoanInterestScheduleModel`] & [`ProgressiveLoanModel`] (`m_loan_progressive_model`)
//! - [`AdvancedPaymentScheduleTransactionProcessor`]

use std::fmt;
use std::str::FromStr;

use chrono::NaiveDate;
use fineract_money::{
    FineractDecimal, FineractRoundingMode, MonetaryCurrency, Money, MoneyError, dec,
};
use fineract_validation_rs::{FineractError, FineractResult};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ============================================================================
// 1. Payment Allocation Enums & Rules (`m_loan_payment_allocation_rule`)
// ============================================================================

/// Transaction types targeted by a [`LoanPaymentAllocationRule`].
///
/// Mirrors `org.apache.fineract.portfolio.loanproduct.domain.PaymentAllocationTransactionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentAllocationTransactionType {
    Default,
    Repayment,
    DownPayment,
    MerchantIssuedRefund,
    PayoutRefund,
    GoodwillCredit,
    ChargeRefund,
    ChargeAdjustment,
    WaiveInterest,
    ChargePayment,
    RefundForActiveLoan,
    InterestPaymentWaiver,
    InterestRefund,
    CapitalizedIncomeAdjustment,
}

impl PaymentAllocationTransactionType {
    /// Canonical uppercase persistence code stored in `m_loan_payment_allocation_rule.transaction_type`.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Repayment => "REPAYMENT",
            Self::DownPayment => "DOWN_PAYMENT",
            Self::MerchantIssuedRefund => "MERCHANT_ISSUED_REFUND",
            Self::PayoutRefund => "PAYOUT_REFUND",
            Self::GoodwillCredit => "GOODWILL_CREDIT",
            Self::ChargeRefund => "CHARGE_REFUND",
            Self::ChargeAdjustment => "CHARGE_ADJUSTMENT",
            Self::WaiveInterest => "WAIVE_INTEREST",
            Self::ChargePayment => "CHARGE_PAYMENT",
            Self::RefundForActiveLoan => "REFUND_FOR_ACTIVE_LOAN",
            Self::InterestPaymentWaiver => "INTEREST_PAYMENT_WAIVER",
            Self::InterestRefund => "INTEREST_REFUND",
            Self::CapitalizedIncomeAdjustment => "CAPITALIZED_INCOME_ADJUSTMENT",
        }
    }

    /// Human-readable description matching upstream Fineract.
    pub const fn human_readable_name(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Repayment => "Repayment",
            Self::DownPayment => "Down Payment",
            Self::MerchantIssuedRefund => "Merchant Issued Refund",
            Self::PayoutRefund => "Payout Refund",
            Self::GoodwillCredit => "Goodwill Credit",
            Self::ChargeRefund => "Charge Refund",
            Self::ChargeAdjustment => "Charge Adjustment",
            Self::WaiveInterest => "Waive Interest",
            Self::ChargePayment => "Charge Payment",
            Self::RefundForActiveLoan => "Refund for Active Loan",
            Self::InterestPaymentWaiver => "Interest Payment Waiver",
            Self::InterestRefund => "Interest Refund",
            Self::CapitalizedIncomeAdjustment => "Capitalized Income Adjustment",
        }
    }

    /// Parse from uppercase/case-insensitive database or API code.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_uppercase().as_str() {
            "DEFAULT" => Some(Self::Default),
            "REPAYMENT" => Some(Self::Repayment),
            "DOWN_PAYMENT" => Some(Self::DownPayment),
            "MERCHANT_ISSUED_REFUND" => Some(Self::MerchantIssuedRefund),
            "PAYOUT_REFUND" => Some(Self::PayoutRefund),
            "GOODWILL_CREDIT" => Some(Self::GoodwillCredit),
            "CHARGE_REFUND" => Some(Self::ChargeRefund),
            "CHARGE_ADJUSTMENT" => Some(Self::ChargeAdjustment),
            "WAIVE_INTEREST" => Some(Self::WaiveInterest),
            "CHARGE_PAYMENT" => Some(Self::ChargePayment),
            "REFUND_FOR_ACTIVE_LOAN" => Some(Self::RefundForActiveLoan),
            "INTEREST_PAYMENT_WAIVER" => Some(Self::InterestPaymentWaiver),
            "INTEREST_REFUND" => Some(Self::InterestRefund),
            "CAPITALIZED_INCOME_ADJUSTMENT" => Some(Self::CapitalizedIncomeAdjustment),
            _ => None,
        }
    }
}

impl fmt::Display for PaymentAllocationTransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PaymentAllocationTransactionType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_code(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.loan.payment.allocation.transaction.type.invalid",
                format!("Invalid PaymentAllocationTransactionType: {s}"),
            )
        })
    }
}

/// Due-status bucket of a [`PaymentAllocationType`].
///
/// Mirrors `org.apache.fineract.portfolio.loanproduct.domain.DueType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DueType {
    PastDue,
    Due,
    InAdvance,
}

impl DueType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PastDue => "PAST_DUE",
            Self::Due => "DUE",
            Self::InAdvance => "IN_ADVANCE",
        }
    }
}

/// Component bucket of a [`PaymentAllocationType`].
///
/// Mirrors `org.apache.fineract.portfolio.loanproduct.domain.AllocationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AllocationType {
    Penalty,
    Fee,
    Principal,
    Interest,
}

impl AllocationType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Penalty => "PENALTY",
            Self::Fee => "FEE",
            Self::Principal => "PRINCIPAL",
            Self::Interest => "INTEREST",
        }
    }
}

/// Ordered 12-bucket payment allocation targets combining [`DueType`] and [`AllocationType`].
///
/// Mirrors `org.apache.fineract.portfolio.loanproduct.domain.PaymentAllocationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentAllocationType {
    PastDuePenalty,
    PastDueFee,
    PastDuePrincipal,
    PastDueInterest,
    DuePenalty,
    DueFee,
    DuePrincipal,
    DueInterest,
    InAdvancePenalty,
    InAdvanceFee,
    InAdvancePrincipal,
    InAdvanceInterest,
}

impl PaymentAllocationType {
    /// Returns the [`DueType`] (`PastDue`, `Due`, or `InAdvance`) of this allocation target.
    pub const fn due_type(&self) -> DueType {
        match self {
            Self::PastDuePenalty
            | Self::PastDueFee
            | Self::PastDuePrincipal
            | Self::PastDueInterest => DueType::PastDue,
            Self::DuePenalty | Self::DueFee | Self::DuePrincipal | Self::DueInterest => {
                DueType::Due
            }
            Self::InAdvancePenalty
            | Self::InAdvanceFee
            | Self::InAdvancePrincipal
            | Self::InAdvanceInterest => DueType::InAdvance,
        }
    }

    /// Returns the [`AllocationType`] (`Penalty`, `Fee`, `Principal`, or `Interest`) of this target.
    pub const fn allocation_type(&self) -> AllocationType {
        match self {
            Self::PastDuePenalty | Self::DuePenalty | Self::InAdvancePenalty => {
                AllocationType::Penalty
            }
            Self::PastDueFee | Self::DueFee | Self::InAdvanceFee => AllocationType::Fee,
            Self::PastDuePrincipal | Self::DuePrincipal | Self::InAdvancePrincipal => {
                AllocationType::Principal
            }
            Self::PastDueInterest | Self::DueInterest | Self::InAdvanceInterest => {
                AllocationType::Interest
            }
        }
    }

    /// Constructs a [`PaymentAllocationType`] from its [`DueType`] and [`AllocationType`] components.
    pub const fn from_parts(due_type: DueType, allocation_type: AllocationType) -> Self {
        match (due_type, allocation_type) {
            (DueType::PastDue, AllocationType::Penalty) => Self::PastDuePenalty,
            (DueType::PastDue, AllocationType::Fee) => Self::PastDueFee,
            (DueType::PastDue, AllocationType::Principal) => Self::PastDuePrincipal,
            (DueType::PastDue, AllocationType::Interest) => Self::PastDueInterest,
            (DueType::Due, AllocationType::Penalty) => Self::DuePenalty,
            (DueType::Due, AllocationType::Fee) => Self::DueFee,
            (DueType::Due, AllocationType::Principal) => Self::DuePrincipal,
            (DueType::Due, AllocationType::Interest) => Self::DueInterest,
            (DueType::InAdvance, AllocationType::Penalty) => Self::InAdvancePenalty,
            (DueType::InAdvance, AllocationType::Fee) => Self::InAdvanceFee,
            (DueType::InAdvance, AllocationType::Principal) => Self::InAdvancePrincipal,
            (DueType::InAdvance, AllocationType::Interest) => Self::InAdvanceInterest,
        }
    }

    /// Canonical uppercase string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PastDuePenalty => "PAST_DUE_PENALTY",
            Self::PastDueFee => "PAST_DUE_FEE",
            Self::PastDuePrincipal => "PAST_DUE_PRINCIPAL",
            Self::PastDueInterest => "PAST_DUE_INTEREST",
            Self::DuePenalty => "DUE_PENALTY",
            Self::DueFee => "DUE_FEE",
            Self::DuePrincipal => "DUE_PRINCIPAL",
            Self::DueInterest => "DUE_INTEREST",
            Self::InAdvancePenalty => "IN_ADVANCE_PENALTY",
            Self::InAdvanceFee => "IN_ADVANCE_FEE",
            Self::InAdvancePrincipal => "IN_ADVANCE_PRINCIPAL",
            Self::InAdvanceInterest => "IN_ADVANCE_INTEREST",
        }
    }

    /// Parses a [`PaymentAllocationType`] from its uppercase string code.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_uppercase().as_str() {
            "PAST_DUE_PENALTY" => Some(Self::PastDuePenalty),
            "PAST_DUE_FEE" => Some(Self::PastDueFee),
            "PAST_DUE_PRINCIPAL" => Some(Self::PastDuePrincipal),
            "PAST_DUE_INTEREST" => Some(Self::PastDueInterest),
            "DUE_PENALTY" => Some(Self::DuePenalty),
            "DUE_FEE" => Some(Self::DueFee),
            "DUE_PRINCIPAL" => Some(Self::DuePrincipal),
            "DUE_INTEREST" => Some(Self::DueInterest),
            "IN_ADVANCE_PENALTY" => Some(Self::InAdvancePenalty),
            "IN_ADVANCE_FEE" => Some(Self::InAdvanceFee),
            "IN_ADVANCE_PRINCIPAL" => Some(Self::InAdvancePrincipal),
            "IN_ADVANCE_INTEREST" => Some(Self::InAdvanceInterest),
            _ => None,
        }
    }

    /// All 12 [`PaymentAllocationType`] variants in canonical `PastDue -> Due -> InAdvance` order.
    pub const fn all() -> [Self; 12] {
        [
            Self::PastDuePenalty,
            Self::PastDueFee,
            Self::PastDuePrincipal,
            Self::PastDueInterest,
            Self::DuePenalty,
            Self::DueFee,
            Self::DuePrincipal,
            Self::DueInterest,
            Self::InAdvancePenalty,
            Self::InAdvanceFee,
            Self::InAdvancePrincipal,
            Self::InAdvanceInterest,
        ]
    }

    /// Standard progressive loan allocation order:
    /// `PastDue (Penalty, Fee, Interest, Principal) -> Due (Penalty, Fee, Interest, Principal) -> InAdvance (Penalty, Fee, Principal, Interest)`.
    pub fn default_progressive_order() -> Vec<Self> {
        vec![
            Self::PastDuePenalty,
            Self::PastDueFee,
            Self::PastDueInterest,
            Self::PastDuePrincipal,
            Self::DuePenalty,
            Self::DueFee,
            Self::DueInterest,
            Self::DuePrincipal,
            Self::InAdvancePenalty,
            Self::InAdvanceFee,
            Self::InAdvancePrincipal,
            Self::InAdvanceInterest,
        ]
    }
}

impl fmt::Display for PaymentAllocationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PaymentAllocationType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_code(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.loan.payment.allocation.type.invalid",
                format!("Invalid PaymentAllocationType: {s}"),
            )
        })
    }
}

/// Strategy for allocating in-advance (`IN_ADVANCE_*`) payment amounts across future installments.
///
/// Mirrors `org.apache.fineract.portfolio.loanproduct.domain.FutureInstallmentAllocationRule`
/// plus progressive loan `REAMORTIZATION` support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FutureInstallmentAllocationRule {
    NextInstallment,
    LastInstallment,
    NextLastInstallment,
    Reamortization,
}

impl FutureInstallmentAllocationRule {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NextInstallment => "NEXT_INSTALLMENT",
            Self::LastInstallment => "LAST_INSTALLMENT",
            Self::NextLastInstallment => "NEXT_LAST_INSTALLMENT",
            Self::Reamortization => "REAMORTIZATION",
        }
    }

    pub const fn human_readable_name(&self) -> &'static str {
        match self {
            Self::NextInstallment => "Next Installment",
            Self::LastInstallment => "Last Installment",
            Self::NextLastInstallment => "Next Last Installment",
            Self::Reamortization => "Reamortization",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_uppercase().as_str() {
            "NEXT_INSTALLMENT" => Some(Self::NextInstallment),
            "LAST_INSTALLMENT" => Some(Self::LastInstallment),
            "NEXT_LAST_INSTALLMENT" => Some(Self::NextLastInstallment),
            "REAMORTIZATION" | "RE_AMORTIZATION" => Some(Self::Reamortization),
            _ => None,
        }
    }
}

impl fmt::Display for FutureInstallmentAllocationRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FutureInstallmentAllocationRule {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_code(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.loan.future.installment.allocation.rule.invalid",
                format!("Invalid FutureInstallmentAllocationRule: {s}"),
            )
        })
    }
}

/// Payment allocation rule bound to a loan (`m_loan_payment_allocation_rule`).
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.domain.LoanPaymentAllocationRule`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanPaymentAllocationRule {
    pub id: Option<i64>,
    pub loan_id: Option<i64>,
    pub transaction_type: PaymentAllocationTransactionType,
    pub allocation_types: Vec<PaymentAllocationType>,
    pub future_installment_allocation_rule: FutureInstallmentAllocationRule,
}

impl LoanPaymentAllocationRule {
    /// Database table name in Fineract schema.
    pub const TABLE_NAME: &'static str = "m_loan_payment_allocation_rule";

    pub fn new(
        transaction_type: PaymentAllocationTransactionType,
        allocation_types: Vec<PaymentAllocationType>,
        future_installment_allocation_rule: FutureInstallmentAllocationRule,
    ) -> Self {
        Self {
            id: None,
            loan_id: None,
            transaction_type,
            allocation_types,
            future_installment_allocation_rule,
        }
    }

    pub fn with_loan_id(
        loan_id: i64,
        transaction_type: PaymentAllocationTransactionType,
        allocation_types: Vec<PaymentAllocationType>,
        future_installment_allocation_rule: FutureInstallmentAllocationRule,
    ) -> Self {
        Self {
            id: None,
            loan_id: Some(loan_id),
            transaction_type,
            allocation_types,
            future_installment_allocation_rule,
        }
    }

    /// Creates a standard `DEFAULT` progressive loan payment allocation rule.
    pub fn default_rule(
        future_installment_allocation_rule: FutureInstallmentAllocationRule,
    ) -> Self {
        Self::new(
            PaymentAllocationTransactionType::Default,
            PaymentAllocationType::default_progressive_order(),
            future_installment_allocation_rule,
        )
    }

    /// Formats `allocation_types` as a comma-separated string for `m_loan_payment_allocation_rule.allocation_types`.
    pub fn allocation_types_csv(&self) -> String {
        self.allocation_types
            .iter()
            .map(PaymentAllocationType::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Parses a [`LoanPaymentAllocationRule`] from database columns (`transaction_type`, `allocation_types`, `future_installment_allocation_rule`).
    pub fn from_csv(
        transaction_type: &str,
        allocation_types_csv: &str,
        future_installment_allocation_rule: &str,
    ) -> FineractResult<Self> {
        let tx_type = PaymentAllocationTransactionType::from_str(transaction_type)?;
        let future_rule =
            FutureInstallmentAllocationRule::from_str(future_installment_allocation_rule)?;
        let mut allocation_types = Vec::new();
        for token in allocation_types_csv
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            allocation_types.push(PaymentAllocationType::from_str(token)?);
        }
        Ok(Self::new(tx_type, allocation_types, future_rule))
    }
}

// ============================================================================
// 2. Credit Allocation Enums & Rules (`m_loan_credit_allocation_rule`)
// ============================================================================

/// Allocation component for credit/chargeback rules (`m_loan_credit_allocation_rule`).
///
/// Mirrors `org.apache.fineract.portfolio.loanproduct.domain.AllocationType` as used in
/// `LoanCreditAllocationRule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CreditAllocationType {
    Penalty,
    Fee,
    Principal,
    Interest,
}

impl CreditAllocationType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Penalty => "PENALTY",
            Self::Fee => "FEE",
            Self::Principal => "PRINCIPAL",
            Self::Interest => "INTEREST",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_uppercase().as_str() {
            "PENALTY" => Some(Self::Penalty),
            "FEE" => Some(Self::Fee),
            "PRINCIPAL" => Some(Self::Principal),
            "INTEREST" => Some(Self::Interest),
            _ => None,
        }
    }

    pub fn default_order() -> Vec<Self> {
        vec![Self::Principal, Self::Interest, Self::Fee, Self::Penalty]
    }
}

impl From<AllocationType> for CreditAllocationType {
    fn from(value: AllocationType) -> Self {
        match value {
            AllocationType::Penalty => Self::Penalty,
            AllocationType::Fee => Self::Fee,
            AllocationType::Principal => Self::Principal,
            AllocationType::Interest => Self::Interest,
        }
    }
}

impl From<CreditAllocationType> for AllocationType {
    fn from(value: CreditAllocationType) -> Self {
        match value {
            CreditAllocationType::Penalty => Self::Penalty,
            CreditAllocationType::Fee => Self::Fee,
            CreditAllocationType::Principal => Self::Principal,
            CreditAllocationType::Interest => Self::Interest,
        }
    }
}

impl fmt::Display for CreditAllocationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CreditAllocationType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_code(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.loan.credit.allocation.type.invalid",
                format!("Invalid CreditAllocationType: {s}"),
            )
        })
    }
}

/// Transaction types targeted by a [`LoanCreditAllocationRule`].
///
/// Mirrors `org.apache.fineract.portfolio.loanproduct.domain.CreditAllocationTransactionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CreditAllocationTransactionType {
    Chargeback,
}

impl CreditAllocationTransactionType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Chargeback => "CHARGEBACK",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_uppercase().as_str() {
            "CHARGEBACK" => Some(Self::Chargeback),
            _ => None,
        }
    }
}

impl fmt::Display for CreditAllocationTransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CreditAllocationTransactionType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_code(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.loan.credit.allocation.transaction.type.invalid",
                format!("Invalid CreditAllocationTransactionType: {s}"),
            )
        })
    }
}

/// Credit allocation rule bound to a loan (`m_loan_credit_allocation_rule`).
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.domain.LoanCreditAllocationRule`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanCreditAllocationRule {
    pub id: Option<i64>,
    pub loan_id: Option<i64>,
    pub transaction_type: CreditAllocationTransactionType,
    pub allocation_types: Vec<CreditAllocationType>,
}

impl LoanCreditAllocationRule {
    /// Database table name in Fineract schema.
    pub const TABLE_NAME: &'static str = "m_loan_credit_allocation_rule";

    pub fn new(
        transaction_type: CreditAllocationTransactionType,
        allocation_types: Vec<CreditAllocationType>,
    ) -> Self {
        Self {
            id: None,
            loan_id: None,
            transaction_type,
            allocation_types,
        }
    }

    pub fn chargeback(allocation_types: Vec<CreditAllocationType>) -> Self {
        Self::new(CreditAllocationTransactionType::Chargeback, allocation_types)
    }

    pub fn default_chargeback_rule() -> Self {
        Self::chargeback(CreditAllocationType::default_order())
    }

    pub fn allocation_types_csv(&self) -> String {
        self.allocation_types
            .iter()
            .map(CreditAllocationType::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn from_csv(transaction_type: &str, allocation_types_csv: &str) -> FineractResult<Self> {
        let tx_type = CreditAllocationTransactionType::from_str(transaction_type)?;
        let mut allocation_types = Vec::new();
        for token in allocation_types_csv
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            allocation_types.push(CreditAllocationType::from_str(token)?);
        }
        Ok(Self::new(tx_type, allocation_types))
    }
}

// ============================================================================
// 3. Progressive Loan Schedule Domain: `InterestPeriod` & `RepaymentPeriod`
// ============================================================================

/// Sub-period slice within a [`RepaymentPeriod`] over which the principal balance and interest
/// rate remain constant.
///
/// Dynamically split by [`ProgressiveLoanInterestScheduleModel`] whenever an early repayment,
/// late repayment, interest rate change, or multi-tranche disbursement occurs mid-period.
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.loanschedule.data.InterestPeriod`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterestPeriod {
    pub from_date: NaiveDate,
    pub due_date: NaiveDate,
    pub rate_factor: Decimal,
    pub rate_factor_till_period_due_date: Decimal,
    pub balance_correction_amount: Money,
    pub disbursement_amount: Money,
    pub capitalized_income_principal: Money,
    pub credited_principal: Money,
    pub credited_interest: Money,
    pub outstanding_loan_balance: Money,
    pub calculated_due_interest: Money,
    pub is_paused: bool,
}

impl InterestPeriod {
    /// Creates a new [`InterestPeriod`] for `[from_date, due_date]` initialized with zero monetary amounts.
    pub fn new(from_date: NaiveDate, due_date: NaiveDate, zero: Money) -> Self {
        Self {
            from_date,
            due_date,
            rate_factor: Decimal::ZERO,
            rate_factor_till_period_due_date: Decimal::ZERO,
            balance_correction_amount: zero.clone(),
            disbursement_amount: zero.clone(),
            capitalized_income_principal: zero.clone(),
            credited_principal: zero.clone(),
            credited_interest: zero.clone(),
            outstanding_loan_balance: zero.clone(),
            calculated_due_interest: zero,
            is_paused: false,
        }
    }

    /// Creates a new [`InterestPeriod`] initialized with zero amounts in `currency`.
    pub fn with_empty_amounts(
        from_date: NaiveDate,
        due_date: NaiveDate,
        currency: MonetaryCurrency,
    ) -> Self {
        Self::new(from_date, due_date, Money::zero(currency))
    }

    /// Number of calendar days in `[from_date, due_date]`.
    pub fn length_in_days(&self) -> i64 {
        (self.due_date - self.from_date).num_days()
    }

    /// Adds to `disbursement_amount`.
    pub fn add_disbursement_amount(&mut self, amount: &Money) -> Result<(), MoneyError> {
        self.disbursement_amount = self.disbursement_amount.plus(amount)?;
        Ok(())
    }

    /// Adds to `balance_correction_amount`.
    pub fn add_balance_correction_amount(&mut self, amount: &Money) -> Result<(), MoneyError> {
        self.balance_correction_amount = self.balance_correction_amount.plus(amount)?;
        Ok(())
    }

    /// Adds to `capitalized_income_principal`.
    pub fn add_capitalized_income_principal(&mut self, amount: &Money) -> Result<(), MoneyError> {
        self.capitalized_income_principal = self.capitalized_income_principal.plus(amount)?;
        Ok(())
    }

    /// Adds to `credited_principal`.
    pub fn add_credited_principal(&mut self, amount: &Money) -> Result<(), MoneyError> {
        self.credited_principal = self.credited_principal.plus(amount)?;
        Ok(())
    }

    /// Adds to `credited_interest`.
    pub fn add_credited_interest(&mut self, amount: &Money) -> Result<(), MoneyError> {
        self.credited_interest = self.credited_interest.plus(amount)?;
        Ok(())
    }

    /// Principal added during this interest period (`disbursement_amount + capitalized_income_principal + credited_principal`).
    pub fn calculated_due_principal(&self) -> Result<Money, MoneyError> {
        self.disbursement_amount
            .plus(&self.capitalized_income_principal)?
            .plus(&self.credited_principal)
    }

    /// Recalculates `calculated_due_interest` from `outstanding_loan_balance * rate_factor + credited_interest`.
    pub fn recalculate_due_interest(
        &mut self,
        rounding_mode: FineractRoundingMode,
    ) -> Result<Money, MoneyError> {
        if self.is_paused || self.outstanding_loan_balance.is_less_than_or_equal_to_zero() {
            self.calculated_due_interest = self.credited_interest.negative_to_zero();
            return Ok(self.calculated_due_interest.clone());
        }
        let raw_interest = FineractDecimal::new(self.outstanding_loan_balance.amount())
            .multiply(FineractDecimal::new(self.rate_factor))
            .raw();
        let interest_money = Money::of(
            *self.outstanding_loan_balance.currency(),
            raw_interest,
            rounding_mode,
        );
        self.calculated_due_interest = interest_money
            .plus(&self.credited_interest)?
            .negative_to_zero();
        Ok(self.calculated_due_interest.clone())
    }
}

/// Progressive loan repayment installment containing one or more [`InterestPeriod`] slices.
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.loanschedule.data.RepaymentPeriod`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepaymentPeriod {
    pub installment_number: i32,
    pub from_date: NaiveDate,
    pub due_date: NaiveDate,
    pub interest_periods: Vec<InterestPeriod>,
    pub emi: Money,
    pub original_emi: Money,
    pub paid_principal: Money,
    pub paid_interest: Money,
    pub unrecognized_interest: Money,
    pub due_fee: Money,
    pub paid_fee: Money,
    pub waived_fee: Money,
    pub due_penalty: Money,
    pub paid_penalty: Money,
    pub waived_penalty: Money,
    pub credited_principal: Money,
    pub credited_interest: Money,
    pub credited_fee: Money,
    pub credited_penalty: Money,
    /// Principal paid in advance strictly before `due_date`.
    pub early_paid_principal: Money,
    pub is_down_payment: bool,
    pub is_recalculation_enabled: bool,
}

impl RepaymentPeriod {
    /// Creates a new [`RepaymentPeriod`] with a single initial [`InterestPeriod`] `[from_date, due_date]`.
    pub fn new(
        installment_number: i32,
        from_date: NaiveDate,
        due_date: NaiveDate,
        zero: Money,
    ) -> Self {
        let initial_ip = InterestPeriod::new(from_date, due_date, zero.clone());
        Self {
            installment_number,
            from_date,
            due_date,
            interest_periods: vec![initial_ip],
            emi: zero.clone(),
            original_emi: zero.clone(),
            paid_principal: zero.clone(),
            paid_interest: zero.clone(),
            unrecognized_interest: zero.clone(),
            due_fee: zero.clone(),
            paid_fee: zero.clone(),
            waived_fee: zero.clone(),
            due_penalty: zero.clone(),
            paid_penalty: zero.clone(),
            waived_penalty: zero.clone(),
            credited_principal: zero.clone(),
            credited_interest: zero.clone(),
            credited_fee: zero.clone(),
            credited_penalty: zero.clone(),
            early_paid_principal: zero,
            is_down_payment: false,
            is_recalculation_enabled: false,
        }
    }

    /// Number of calendar days in `[from_date, due_date]`.
    pub fn length_in_days(&self) -> i64 {
        (self.due_date - self.from_date).num_days()
    }

    /// Returns `1 + sum(ip.rate_factor)` across all [`InterestPeriod`]s in this installment.
    pub fn rate_factor_plus_1(&self) -> Decimal {
        self.interest_periods
            .iter()
            .fold(Decimal::ONE, |acc, ip| acc + ip.rate_factor)
    }

    /// Reference to the first [`InterestPeriod`].
    pub fn first_interest_period(&self) -> &InterestPeriod {
        &self.interest_periods[0]
    }

    /// Mutable reference to the first [`InterestPeriod`].
    pub fn first_interest_period_mut(&mut self) -> &mut InterestPeriod {
        &mut self.interest_periods[0]
    }

    /// Reference to the last [`InterestPeriod`].
    pub fn last_interest_period(&self) -> &InterestPeriod {
        self.interest_periods
            .last()
            .expect("RepaymentPeriod always has at least one InterestPeriod")
    }

    /// Mutable reference to the last [`InterestPeriod`].
    pub fn last_interest_period_mut(&mut self) -> &mut InterestPeriod {
        self.interest_periods
            .last_mut()
            .expect("RepaymentPeriod always has at least one InterestPeriod")
    }

    /// Sum of `calculated_due_interest` across all [`InterestPeriod`] slices in this installment,
    /// plus any period-level `credited_interest`.
    pub fn calculated_due_interest(&self) -> Result<Money, MoneyError> {
        let mut total = self.emi.zero_of();
        for ip in &self.interest_periods {
            total = total.plus(&ip.calculated_due_interest)?;
        }
        total = total.plus(&self.credited_interest)?;
        Ok(total.negative_to_zero())
    }

    /// Effective due interest for this installment (`max(calculated_due_interest - unrecognized_interest, paid_interest)`).
    pub fn due_interest(&self) -> Result<Money, MoneyError> {
        let calc = self
            .calculated_due_interest()?
            .minus_to_zero(&self.unrecognized_interest)?;
        Ok(Money::max(&calc, &self.paid_interest).clone())
    }

    /// Effective due principal for this installment (`max(emi - due_interest, paid_principal)`).
    pub fn due_principal(&self) -> Result<Money, MoneyError> {
        let due_int = self.due_interest()?;
        let from_emi = self.emi.minus_to_zero(&due_int)?;
        Ok(Money::max(&from_emi, &self.paid_principal).clone())
    }

    /// Unpaid principal remaining on this installment (`due_principal - paid_principal`).
    pub fn outstanding_principal(&self) -> Result<Money, MoneyError> {
        self.due_principal()?.minus_to_zero(&self.paid_principal)
    }

    /// Unpaid interest remaining on this installment (`due_interest - paid_interest`).
    pub fn outstanding_interest(&self) -> Result<Money, MoneyError> {
        self.due_interest()?.minus_to_zero(&self.paid_interest)
    }

    /// Unpaid fee remaining on this installment (`due_fee - paid_fee - waived_fee`).
    pub fn outstanding_fee(&self) -> Result<Money, MoneyError> {
        self.due_fee
            .minus_to_zero(&self.paid_fee)?
            .minus_to_zero(&self.waived_fee)
    }

    /// Unpaid penalty remaining on this installment (`due_penalty - paid_penalty - waived_penalty`).
    pub fn outstanding_penalty(&self) -> Result<Money, MoneyError> {
        self.due_penalty
            .minus_to_zero(&self.paid_penalty)?
            .minus_to_zero(&self.waived_penalty)
    }

    /// Total amount paid across principal, interest, fee, and penalty on this installment.
    pub fn total_paid(&self) -> Result<Money, MoneyError> {
        self.paid_principal
            .plus(&self.paid_interest)?
            .plus(&self.paid_fee)?
            .plus(&self.paid_penalty)
    }

    /// Total unpaid amount across principal, interest, fee, and penalty on this installment.
    pub fn total_outstanding(&self) -> Result<Money, MoneyError> {
        self.outstanding_principal()?
            .plus(&self.outstanding_interest()?)?
            .plus(&self.outstanding_fee()?)?
            .plus(&self.outstanding_penalty()?)
    }

    /// Whether all principal, interest, fees, and penalties on this installment are fully settled.
    pub fn is_fully_paid(&self) -> Result<bool, MoneyError> {
        Ok(self.total_outstanding()?.is_zero())
    }

    /// Total disbursement amount across all [`InterestPeriod`]s in this installment.
    pub fn total_disbursed_amount(&self) -> Result<Money, MoneyError> {
        let mut total = self.emi.zero_of();
        for ip in &self.interest_periods {
            total = total.plus(&ip.disbursement_amount)?;
        }
        Ok(total)
    }

    /// Total balance correction amount across all [`InterestPeriod`]s in this installment.
    pub fn total_balance_correction_amount(&self) -> Result<Money, MoneyError> {
        let mut total = self.emi.zero_of();
        for ip in &self.interest_periods {
            total = total.plus(&ip.balance_correction_amount)?;
        }
        Ok(total)
    }

    /// Scheduled ending principal balance at `self.due_date` after `due_principal` is accounted for.
    pub fn ending_scheduled_balance(&self) -> Result<Money, MoneyError> {
        let last_balance = &self.last_interest_period().outstanding_loan_balance;
        let remaining_scheduled_principal = self
            .due_principal()?
            .minus_to_zero(&self.early_paid_principal)?;
        last_balance.minus_to_zero(&remaining_scheduled_principal)
    }
}

// ============================================================================
// 4. Persisted Progressive Loan Model (`m_loan_progressive_model`)
// ============================================================================

/// Database entity representing a serialized [`ProgressiveLoanInterestScheduleModel`] stored in
/// `m_loan_progressive_model`.
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.domain.ProgressiveLoanModel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressiveLoanModel {
    pub id: Option<i64>,
    pub loan_id: i64,
    pub version: String,
    pub last_modified_date: NaiveDate,
    pub json_model: String,
}

impl ProgressiveLoanModel {
    pub const TABLE_NAME: &'static str = "m_loan_progressive_model";
    pub const CURRENT_VERSION: &'static str = "1";

    pub fn from_schedule_model(
        loan_id: i64,
        last_modified_date: NaiveDate,
        model: &ProgressiveLoanInterestScheduleModel,
    ) -> FineractResult<Self> {
        let json_model = serde_json::to_string(model).map_err(|e| {
            FineractError::domain_rule(
                "error.msg.loan.progressive.model.serialize.failed",
                e.to_string(),
            )
        })?;
        Ok(Self {
            id: None,
            loan_id,
            version: Self::CURRENT_VERSION.to_string(),
            last_modified_date,
            json_model,
        })
    }

    pub fn decode_schedule_model(&self) -> FineractResult<ProgressiveLoanInterestScheduleModel> {
        serde_json::from_str(&self.json_model).map_err(|e| {
            FineractError::domain_rule(
                "error.msg.loan.progressive.model.deserialize.failed",
                e.to_string(),
            )
        })
    }
}

// ============================================================================
// 5. Progressive Loan Interest Schedule Model & EMI Calculator
// ============================================================================

/// Day-count convention for days in a month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DaysInMonthStrategy {
    /// Actual calendar days in each sub-period.
    Actual,
    /// Fixed 30-day month convention.
    Days30,
    /// Partial periods are scaled proportionally (`ip_days / rp_days`) relative to an equal repayment period.
    SameAsRepaymentPeriod,
}

/// Day-count convention for days in a year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DaysInYearStrategy {
    Actual,
    Days360,
    Days364,
    Days365,
}

impl DaysInYearStrategy {
    pub fn days_in_year(&self, date: NaiveDate) -> i64 {
        match self {
            Self::Actual => {
                let year = date.format("%Y").to_string().parse::<i32>().unwrap_or(2024);
                let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
                if is_leap { 366 } else { 365 }
            }
            Self::Days360 => 360,
            Self::Days364 => 364,
            Self::Days365 => 365,
        }
    }
}

/// Effective interest rate change entry (`effective_from_date` -> `annual_nominal_interest_rate`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterestRateChange {
    pub effective_from_date: NaiveDate,
    pub annual_nominal_interest_rate: Decimal,
}

/// Configuration parameters for [`ProgressiveLoanInterestScheduleModel`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressiveLoanScheduleParams {
    pub currency: MonetaryCurrency,
    pub annual_nominal_interest_rate: Decimal,
    pub days_in_month_strategy: DaysInMonthStrategy,
    pub days_in_year_strategy: DaysInYearStrategy,
    /// Number of repayment periods in a full year (e.g. `12` for monthly, `52` for weekly, `26` for bi-weekly).
    pub periods_per_year: u32,
    pub installment_amount_in_multiples_of: Option<i32>,
    pub rounding_mode: FineractRoundingMode,
    pub allow_full_term_for_tranche: bool,
}

impl ProgressiveLoanScheduleParams {
    /// Creates standard monthly progressive schedule parameters (`12` periods/year, `SameAsRepaymentPeriod` / `Days360`).
    pub fn monthly(currency: MonetaryCurrency, annual_nominal_interest_rate: Decimal) -> Self {
        Self {
            currency,
            annual_nominal_interest_rate,
            days_in_month_strategy: DaysInMonthStrategy::SameAsRepaymentPeriod,
            days_in_year_strategy: DaysInYearStrategy::Days360,
            periods_per_year: 12,
            installment_amount_in_multiples_of: None,
            rounding_mode: FineractRoundingMode::HalfEven,
            allow_full_term_for_tranche: false,
        }
    }

    /// Creates daily `ACTUAL/365` progressive schedule parameters.
    pub fn actual_365(currency: MonetaryCurrency, annual_nominal_interest_rate: Decimal) -> Self {
        Self {
            currency,
            annual_nominal_interest_rate,
            days_in_month_strategy: DaysInMonthStrategy::Actual,
            days_in_year_strategy: DaysInYearStrategy::Days365,
            periods_per_year: 12,
            installment_amount_in_multiples_of: None,
            rounding_mode: FineractRoundingMode::HalfEven,
            allow_full_term_for_tranche: false,
        }
    }
}

/// Stateful progressive loan interest and EMI schedule model.
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.loanschedule.data.ProgressiveLoanInterestScheduleModel`
/// and `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.ProgressiveEMICalculator`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressiveLoanInterestScheduleModel {
    pub repayment_periods: Vec<RepaymentPeriod>,
    pub params: ProgressiveLoanScheduleParams,
    pub interest_rate_changes: Vec<InterestRateChange>,
    /// Tracks overdue principal corrections already injected per installment number so repeated
    /// recalculations stay idempotent.
    overdue_corrected_installments: Vec<(i32, NaiveDate, Money)>,
}

impl ProgressiveLoanInterestScheduleModel {
    /// Constructs a new schedule model from installment date boundaries `(from_date, due_date)`
    /// and an optional initial disbursement at the first period's `from_date`.
    pub fn generate_schedule(
        period_dates: &[(NaiveDate, NaiveDate)],
        initial_disbursement: Option<Money>,
        params: ProgressiveLoanScheduleParams,
    ) -> FineractResult<Self> {
        if period_dates.is_empty() {
            return Err(FineractError::domain_rule(
                "error.msg.loan.progressive.schedule.empty",
                "ProgressiveLoanInterestScheduleModel requires at least one repayment period",
            ));
        }
        let zero = Money::zero(params.currency);
        let mut repayment_periods = Vec::with_capacity(period_dates.len());
        for (idx, &(from_date, due_date)) in period_dates.iter().enumerate() {
            if due_date <= from_date {
                return Err(FineractError::domain_rule(
                    "error.msg.loan.progressive.schedule.invalid.period",
                    format!(
                        "Installment {} due_date ({due_date}) must be after from_date ({from_date})",
                        idx + 1
                    ),
                ));
            }
            repayment_periods.push(RepaymentPeriod::new(
                (idx + 1) as i32,
                from_date,
                due_date,
                zero.clone(),
            ));
        }

        let mut model = Self {
            repayment_periods,
            params,
            interest_rate_changes: Vec::new(),
            overdue_corrected_installments: Vec::new(),
        };

        model.recalculate_rate_factors();

        if let Some(disbursed) = initial_disbursement {
            if disbursed.is_greater_than_zero() {
                let start_date = model.repayment_periods[0].from_date;
                model.add_disbursement(start_date, disbursed)?;
            }
        }

        Ok(model)
    }

    /// Returns the zero [`Money`] value for the schedule's currency.
    pub fn zero(&self) -> Money {
        Money::zero(self.params.currency)
    }

    /// Returns the nominal annual interest rate effective on `date`.
    pub fn effective_annual_interest_rate(&self, date: NaiveDate) -> Decimal {
        let mut rate = self.params.annual_nominal_interest_rate;
        for change in &self.interest_rate_changes {
            if change.effective_from_date <= date {
                rate = change.annual_nominal_interest_rate;
            }
        }
        rate
    }

    /// Calculates the rate factor for an [`InterestPeriod`] `[ip_from, ip_due]` inside a
    /// [`RepaymentPeriod`] `[rp_from, rp_due]`.
    pub fn calculate_rate_factor(
        &self,
        rp_from: NaiveDate,
        rp_due: NaiveDate,
        ip_from: NaiveDate,
        ip_due: NaiveDate,
    ) -> Decimal {
        let ip_days = (ip_due - ip_from).num_days();
        let rp_days = (rp_due - rp_from).num_days();
        if ip_days <= 0 || rp_days <= 0 {
            return Decimal::ZERO;
        }

        let annual_rate = self.effective_annual_interest_rate(ip_from);
        if annual_rate <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let rate_per_unit = FineractDecimal::new(annual_rate).divide(
            FineractDecimal::new(dec!(100)),
            self.params.rounding_mode,
        );

        match self.params.days_in_month_strategy {
            DaysInMonthStrategy::SameAsRepaymentPeriod | DaysInMonthStrategy::Days30 => {
                let periods_per_year = Decimal::from(self.params.periods_per_year.max(1));
                let full_period_factor = rate_per_unit.divide(
                    FineractDecimal::new(periods_per_year),
                    self.params.rounding_mode,
                );
                let ratio = FineractDecimal::new(Decimal::from(ip_days)).divide(
                    FineractDecimal::new(Decimal::from(rp_days)),
                    self.params.rounding_mode,
                );
                full_period_factor.multiply(ratio).raw()
            }
            DaysInMonthStrategy::Actual => {
                let days_in_year =
                    Decimal::from(self.params.days_in_year_strategy.days_in_year(ip_from));
                let day_ratio = FineractDecimal::new(Decimal::from(ip_days)).divide(
                    FineractDecimal::new(days_in_year),
                    self.params.rounding_mode,
                );
                rate_per_unit.multiply(day_ratio).raw()
            }
        }
    }

    /// Recalculates `rate_factor` and `rate_factor_till_period_due_date` across every
    /// [`RepaymentPeriod`] and [`InterestPeriod`].
    pub fn recalculate_rate_factors(&mut self) {
        let num_periods = self.repayment_periods.len();
        for rp_idx in 0..num_periods {
            let rp_from = self.repayment_periods[rp_idx].from_date;
            let rp_due = self.repayment_periods[rp_idx].due_date;
            let num_ips = self.repayment_periods[rp_idx].interest_periods.len();
            for ip_idx in 0..num_ips {
                let ip_from = self.repayment_periods[rp_idx].interest_periods[ip_idx].from_date;
                let ip_due = self.repayment_periods[rp_idx].interest_periods[ip_idx].due_date;
                let is_paused = self.repayment_periods[rp_idx].interest_periods[ip_idx].is_paused;

                let rf = if is_paused {
                    Decimal::ZERO
                } else {
                    self.calculate_rate_factor(rp_from, rp_due, ip_from, rp_due_or_ip_due(ip_due))
                };
                let rf_till_end = if is_paused {
                    Decimal::ZERO
                } else {
                    self.calculate_rate_factor(rp_from, rp_due, ip_from, rp_due)
                };

                self.repayment_periods[rp_idx].interest_periods[ip_idx].rate_factor = rf;
                self.repayment_periods[rp_idx].interest_periods[ip_idx]
                    .rate_factor_till_period_due_date = rf_till_end;
            }
        }
    }

    /// Ensures an [`InterestPeriod`] boundary exists at `split_date` inside the [`RepaymentPeriod`]
    /// covering `split_date`, splitting the enclosing [`InterestPeriod`] into
    /// `[ip.from_date, split_date]` and `[split_date, ip.due_date]` when `ip.from_date < split_date < ip.due_date`.
    ///
    /// Returns `(repayment_period_index, interest_period_index_starting_at_split_date)`.
    pub fn ensure_interest_period_split(
        &mut self,
        split_date: NaiveDate,
    ) -> FineractResult<Option<(usize, usize)>> {
        if self.repayment_periods.is_empty() {
            return Ok(None);
        }

        // Case 1: split_date is on or before the first period's from_date
        if split_date <= self.repayment_periods[0].from_date {
            return Ok(Some((0, 0)));
        }

        // Find the repayment period where `from_date <= split_date <= due_date`
        // Prefer the period where `split_date == from_date` if it's the start of a period,
        // or the period where `from_date < split_date < due_date`.
        let mut target_rp_idx = None;
        for (idx, rp) in self.repayment_periods.iter().enumerate() {
            if split_date == rp.from_date {
                target_rp_idx = Some(idx);
                break;
            }
            if split_date > rp.from_date && split_date < rp.due_date {
                target_rp_idx = Some(idx);
                break;
            }
            if split_date == rp.due_date && idx + 1 == self.repayment_periods.len() {
                // On the final maturity date
                return Ok(None);
            }
        }

        let Some(rp_idx) = target_rp_idx else {
            return Ok(None);
        };

        // Check if an InterestPeriod inside `repayment_periods[rp_idx]` already has `from_date == split_date`
        if let Some(ip_idx) = self.repayment_periods[rp_idx]
            .interest_periods
            .iter()
            .position(|ip| ip.from_date == split_date)
        {
            return Ok(Some((rp_idx, ip_idx)));
        }

        // Otherwise find the InterestPeriod with `ip.from_date < split_date < ip.due_date` and split it
        let Some(enclosing_idx) = self.repayment_periods[rp_idx]
            .interest_periods
            .iter()
            .position(|ip| split_date > ip.from_date && split_date < ip.due_date)
        else {
            return Ok(None);
        };

        let zero = self.zero();
        let original_due_date =
            self.repayment_periods[rp_idx].interest_periods[enclosing_idx].due_date;
        let is_paused = self.repayment_periods[rp_idx].interest_periods[enclosing_idx].is_paused;

        // Shorten first slice to `[from_date, split_date]`
        self.repayment_periods[rp_idx].interest_periods[enclosing_idx].due_date = split_date;

        // Create second slice `[split_date, original_due_date]`
        let mut second_slice = InterestPeriod::new(split_date, original_due_date, zero);
        second_slice.is_paused = is_paused;

        let new_ip_idx = enclosing_idx + 1;
        self.repayment_periods[rp_idx]
            .interest_periods
            .insert(new_ip_idx, second_slice);

        self.recalculate_rate_factors();
        Ok(Some((rp_idx, new_ip_idx)))
    }

    /// Records a disbursement (initial or multi-tranche) on `disbursement_date`, dynamically
    /// splitting the enclosing [`InterestPeriod`] if `disbursement_date` falls mid-period,
    /// and recalculates EMIs and balances.
    pub fn add_disbursement(
        &mut self,
        disbursement_date: NaiveDate,
        disbursed_amount: Money,
    ) -> FineractResult<()> {
        if !disbursed_amount.is_greater_than_zero() {
            return Ok(());
        }

        let Some((rp_idx, ip_idx)) = self.ensure_interest_period_split(disbursement_date)? else {
            return Err(FineractError::domain_rule(
                "error.msg.loan.disbursement.date.outside.schedule",
                format!(
                    "Disbursement date {disbursement_date} is outside the loan schedule"
                ),
            ));
        };

        self.repayment_periods[rp_idx].interest_periods[ip_idx]
            .add_disbursement_amount(&disbursed_amount)
            .map_err(money_err)?;

        let start_emi_idx = if self.params.allow_full_term_for_tranche {
            0
        } else {
            rp_idx
        };
        self.recalculate_emi_from(start_emi_idx, true)?;
        Ok(())
    }

    /// Adds an interest rate change effective from `effective_date`, dynamically splitting
    /// the enclosing [`InterestPeriod`] if `effective_date` falls mid-period, and recalculating
    /// rate factors, EMIs, and interest.
    pub fn add_interest_rate_change(
        &mut self,
        effective_date: NaiveDate,
        new_annual_nominal_interest_rate: Decimal,
    ) -> FineractResult<()> {
        if let Some(existing) = self
            .interest_rate_changes
            .iter_mut()
            .find(|c| c.effective_from_date == effective_date)
        {
            existing.annual_nominal_interest_rate = new_annual_nominal_interest_rate;
        } else {
            self.interest_rate_changes.push(InterestRateChange {
                effective_from_date: effective_date,
                annual_nominal_interest_rate: new_annual_nominal_interest_rate,
            });
            self.interest_rate_changes
                .sort_by_key(|c| c.effective_from_date);
        }

        let split_pos = self.ensure_interest_period_split(effective_date)?;
        self.recalculate_rate_factors();

        let start_rp_idx = split_pos.map(|(rp_idx, _)| rp_idx).unwrap_or(0);
        self.recalculate_emi_from(start_rp_idx, true)?;
        Ok(())
    }

    /// Applies a balance correction (e.g., negative correction `-principal_paid` on early repayment
    /// or positive correction `+overdue_principal` on late payment) at `balance_change_date`,
    /// splitting the enclosing [`InterestPeriod`] if `balance_change_date` is mid-period.
    pub fn change_outstanding_balance_and_update_interest_periods(
        &mut self,
        balance_change_date: NaiveDate,
        disbursed_amount: Money,
        balance_correction_amount: Money,
        capitalized_income_principal: Money,
    ) -> FineractResult<Option<usize>> {
        if disbursed_amount.is_zero()
            && balance_correction_amount.is_zero()
            && capitalized_income_principal.is_zero()
        {
            return Ok(None);
        }

        let Some((rp_idx, ip_idx)) = self.ensure_interest_period_split(balance_change_date)? else {
            return Ok(None);
        };

        let ip = &mut self.repayment_periods[rp_idx].interest_periods[ip_idx];
        ip.add_disbursement_amount(&disbursed_amount)
            .map_err(money_err)?;
        ip.add_balance_correction_amount(&balance_correction_amount)
            .map_err(money_err)?;
        ip.add_capitalized_income_principal(&capitalized_income_principal)
            .map_err(money_err)?;

        self.recalculate_balances_and_interest()?;
        Ok(Some(rp_idx))
    }

    /// Re-amortizes the remaining unpaid principal balance evenly across all future unpaid
    /// [`RepaymentPeriod`]s with `due_date > effective_date`.
    pub fn reamortize(&mut self, effective_date: NaiveDate) -> FineractResult<()> {
        let _ = self.ensure_interest_period_split(effective_date)?;
        let Some(start_rp_idx) = self
            .repayment_periods
            .iter()
            .position(|rp| rp.due_date > effective_date && !rp.is_fully_paid().unwrap_or(false))
        else {
            return Ok(());
        };
        self.recalculate_emi_from(start_rp_idx, true)
    }

    /// Injects overdue balance corrections for any installment with `due_date < target_date`
    /// that still had unpaid principal after its `due_date`, ensuring interest accrues on the
    /// unpaid overdue principal up to `target_date`.
    pub fn recalculate_overdue_amounts_till_date(
        &mut self,
        target_date: NaiveDate,
    ) -> FineractResult<()> {
        let mut pending_corrections: Vec<(i32, NaiveDate, NaiveDate, Money)> = Vec::new();

        for rp in &self.repayment_periods {
            if rp.due_date < target_date {
                let overdue_principal = rp.outstanding_principal().map_err(money_err)?;
                let already_recorded = self
                    .overdue_corrected_installments
                    .iter()
                    .any(|(inst, _, _)| *inst == rp.installment_number);
                if overdue_principal.is_greater_than_zero() && !already_recorded {
                    pending_corrections.push((
                        rp.installment_number,
                        rp.due_date,
                        target_date,
                        overdue_principal,
                    ));
                }
            }
        }

        let zero = self.zero();
        for (inst_num, from_overdue_date, to_overdue_date, overdue_amount) in pending_corrections {
            // 1. Add `+overdue_amount` at `from_overdue_date` (start of next period)
            let _ = self.change_outstanding_balance_and_update_interest_periods(
                from_overdue_date,
                zero.clone(),
                overdue_amount.clone(),
                zero.clone(),
            )?;
            // 2. Add `-overdue_amount` at `to_overdue_date` (when settled / evaluated)
            let _ = self.change_outstanding_balance_and_update_interest_periods(
                to_overdue_date,
                zero.clone(),
                overdue_amount.negated(),
                zero.clone(),
            )?;
            self.overdue_corrected_installments
                .push((inst_num, to_overdue_date, overdue_amount));
        }

        self.recalculate_balances_and_interest()
    }

    /// Recalculates EMI values from `start_period_idx` through the end of the schedule using
    /// Fineract's progressive EMI formula (`ProgressiveEMICalculator::calculateEMIValue`),
    /// then updates all period balances and interest amounts.
    pub fn recalculate_emi_from(
        &mut self,
        start_period_idx: usize,
        recalculate_emi_value: bool,
    ) -> FineractResult<()> {
        if start_period_idx >= self.repayment_periods.len() {
            return self.recalculate_balances_and_interest();
        }

        if recalculate_emi_value {
            // First propagate balances up to `start_period_idx` so we know the exact starting principal
            self.propagate_balances_and_interest_internal(false)?;

            // Compute total principal to be amortized across `start_period_idx..len`:
            // initial balance entering `start_period_idx` + any disbursements/capitalized income in `start_period_idx..len`,
            // minus any early principal already paid on periods in `start_period_idx..len`.
            let mut principal_to_amortize = if start_period_idx == 0 {
                self.zero()
            } else {
                self.repayment_periods[start_period_idx - 1]
                    .ending_scheduled_balance()
                    .map_err(money_err)?
            };

            for rp in &self.repayment_periods[start_period_idx..] {
                for ip in &rp.interest_periods {
                    principal_to_amortize = principal_to_amortize
                        .plus(&ip.disbursement_amount)
                        .map_err(money_err)?
                        .plus(&ip.capitalized_income_principal)
                        .map_err(money_err)?
                        .plus(&ip.credited_principal)
                        .map_err(money_err)?;
                }
            }

            let rate_factors_plus_1: Vec<Decimal> = self.repayment_periods[start_period_idx..]
                .iter()
                .map(RepaymentPeriod::rate_factor_plus_1)
                .collect();

            let calculated_emi = self.calculate_emi_value(
                &principal_to_amortize,
                &rate_factors_plus_1,
            )?;

            let is_initial = self
                .repayment_periods
                .iter()
                .all(|rp| rp.original_emi.is_zero());

            for rp in &mut self.repayment_periods[start_period_idx..] {
                rp.emi = calculated_emi.clone();
                if is_initial {
                    rp.original_emi = calculated_emi.clone();
                }
            }

            // Evening-out pass (mirrors `checkAndAdjustEmiIfNeededOnRelatedRepaymentPeriods`):
            // If after propagating with `calculated_emi`, the last period's EMI exceeds the
            // regular EMI by more than `(N * 0.01)` due to rounding down of regular EMI, test `emi + 1 step`.
            self.propagate_balances_and_interest_internal(true)?;
            let remaining_count = self.repayment_periods.len() - start_period_idx;
            if remaining_count > 1 {
                let last_emi = self.repayment_periods.last().unwrap().emi.clone();
                let diff = last_emi.minus(&calculated_emi).map_err(money_err)?;
                let scale = self.params.currency.decimal_places() as u32;
                let one_cent = Decimal::new(1, scale);
                let threshold = Money::of(
                    self.params.currency,
                    one_cent * Decimal::from((remaining_count as i64) / 2 + 1),
                    self.params.rounding_mode,
                );
                if diff.is_greater_than(&threshold) {
                    let step = match self.params.installment_amount_in_multiples_of {
                        Some(m) if m > 0 => Decimal::from(m),
                        _ => one_cent,
                    };
                    let adjusted_emi = calculated_emi
                        .plus(&Money::of(
                            self.params.currency,
                            step,
                            self.params.rounding_mode,
                        ))
                        .map_err(money_err)?;
                    for rp in &mut self.repayment_periods[start_period_idx..] {
                        rp.emi = adjusted_emi.clone();
                        if is_initial {
                            rp.original_emi = adjusted_emi.clone();
                        }
                    }
                }
            }
        }

        self.recalculate_balances_and_interest()
    }

    /// Recomputes `outstanding_loan_balance` and `calculated_due_interest` across all
    /// [`InterestPeriod`]s and reconciles the final unpaid [`RepaymentPeriod`]'s EMI so that
    /// `sum(rp.due_principal())` equals total disbursed + capitalized + credited principal.
    pub fn recalculate_balances_and_interest(&mut self) -> FineractResult<()> {
        self.propagate_balances_and_interest_internal(true)
    }

    fn propagate_balances_and_interest_internal(
        &mut self,
        adjust_last_period_emi: bool,
    ) -> FineractResult<()> {
        let num_periods = self.repayment_periods.len();
        if num_periods == 0 {
            return Ok(());
        }

        let mut total_principal_inflow = self.zero();
        for rp in &self.repayment_periods {
            for ip in &rp.interest_periods {
                total_principal_inflow = total_principal_inflow
                    .plus(&ip.disbursement_amount)
                    .map_err(money_err)?
                    .plus(&ip.capitalized_income_principal)
                    .map_err(money_err)?
                    .plus(&ip.credited_principal)
                    .map_err(money_err)?;
            }
            total_principal_inflow = total_principal_inflow
                .plus(&rp.credited_principal)
                .map_err(money_err)?;
        }

        for rp_idx in 0..num_periods {
            let starting_balance = if rp_idx == 0 {
                self.zero()
            } else {
                self.repayment_periods[rp_idx - 1]
                    .ending_scheduled_balance()
                    .map_err(money_err)?
            };

            let num_ips = self.repayment_periods[rp_idx].interest_periods.len();
            for ip_idx in 0..num_ips {
                let prev_balance = if ip_idx == 0 {
                    starting_balance.clone()
                } else {
                    self.repayment_periods[rp_idx].interest_periods[ip_idx - 1]
                        .outstanding_loan_balance
                        .clone()
                };

                let ip = &mut self.repayment_periods[rp_idx].interest_periods[ip_idx];
                let new_balance = prev_balance
                    .plus(&ip.disbursement_amount)
                    .map_err(money_err)?
                    .plus(&ip.capitalized_income_principal)
                    .map_err(money_err)?
                    .plus(&ip.credited_principal)
                    .map_err(money_err)?
                    .plus(&ip.balance_correction_amount)
                    .map_err(money_err)?
                    .negative_to_zero();

                ip.outstanding_loan_balance = new_balance;
                ip.recalculate_due_interest(self.params.rounding_mode)
                    .map_err(money_err)?;
            }
        }

        if adjust_last_period_emi && total_principal_inflow.is_greater_than_zero() {
            // Reconcile the last installment's EMI so total due principal across all periods
            // equals `total_principal_inflow`.
            let last_idx = num_periods - 1;
            let mut prior_due_principal = self.zero();
            for rp in &self.repayment_periods[..last_idx] {
                prior_due_principal = prior_due_principal
                    .plus(&rp.due_principal().map_err(money_err)?)
                    .map_err(money_err)?;
            }

            let last_due_principal = total_principal_inflow
                .minus_to_zero(&prior_due_principal)
                .map_err(money_err)?;
            let last_due_interest = self.repayment_periods[last_idx]
                .due_interest()
                .map_err(money_err)?;
            self.repayment_periods[last_idx].emi = last_due_principal
                .plus(&last_due_interest)
                .map_err(money_err)?;
            if self.repayment_periods[last_idx].original_emi.is_zero() {
                self.repayment_periods[last_idx].original_emi =
                    self.repayment_periods[last_idx].emi.clone();
            }
        }

        Ok(())
    }

    /// Computes the Equal Monthly/Periodic Installment (EMI) using Fineract's exact polynomial
    /// recurrence from `ProgressiveEMICalculator::calculateEMIValue`:
    ///
    /// - `rateFactorPlus1N = prod_{i=0..k-1} r_i`
    /// - `fn_0 = 1`, `fn_i = 1 + fn_{i-1} * r_i` for `i = 1..k-1`
    /// - `EMI = (principal * rateFactorPlus1N) / fn_{k-1}`
    pub fn calculate_emi_value(
        &self,
        principal: &Money,
        rate_factors_plus_1: &[Decimal],
    ) -> FineractResult<Money> {
        if principal.is_less_than_or_equal_to_zero() || rate_factors_plus_1.is_empty() {
            return Ok(self.zero());
        }

        let mut rate_factor_n = FineractDecimal::new(Decimal::ONE);
        for &r in rate_factors_plus_1 {
            rate_factor_n = rate_factor_n.multiply(FineractDecimal::new(r));
        }

        let mut fn_result = FineractDecimal::new(Decimal::ONE);
        for &r in &rate_factors_plus_1[1..] {
            fn_result = fn_result
                .multiply(FineractDecimal::new(r))
                .add(FineractDecimal::new(Decimal::ONE));
        }

        if fn_result.is_zero() {
            return Ok(self.zero());
        }

        let numerator = FineractDecimal::new(principal.amount()).multiply(rate_factor_n);
        let raw_emi = numerator.divide(fn_result, self.params.rounding_mode).raw();

        let emi = Money::of(self.params.currency, raw_emi, self.params.rounding_mode);
        let rounded_emi = match self.params.installment_amount_in_multiples_of {
            Some(mult) if mult > 0 => emi.round_to_multiples_of(mult, self.params.rounding_mode),
            _ => emi,
        };
        Ok(rounded_emi)
    }

    /// Total scheduled due principal across all [`RepaymentPeriod`]s.
    pub fn total_due_principal(&self) -> FineractResult<Money> {
        let mut total = self.zero();
        for rp in &self.repayment_periods {
            total = total
                .plus(&rp.due_principal().map_err(money_err)?)
                .map_err(money_err)?;
        }
        Ok(total)
    }

    /// Total paid principal across all [`RepaymentPeriod`]s.
    pub fn total_paid_principal(&self) -> FineractResult<Money> {
        let mut total = self.zero();
        for rp in &self.repayment_periods {
            total = total.plus(&rp.paid_principal).map_err(money_err)?;
        }
        Ok(total)
    }

    /// Total scheduled due interest across all [`RepaymentPeriod`]s.
    pub fn total_due_interest(&self) -> FineractResult<Money> {
        let mut total = self.zero();
        for rp in &self.repayment_periods {
            total = total
                .plus(&rp.due_interest().map_err(money_err)?)
                .map_err(money_err)?;
        }
        Ok(total)
    }

    /// Total paid interest across all [`RepaymentPeriod`]s.
    pub fn total_paid_interest(&self) -> FineractResult<Money> {
        let mut total = self.zero();
        for rp in &self.repayment_periods {
            total = total.plus(&rp.paid_interest).map_err(money_err)?;
        }
        Ok(total)
    }

    /// Total outstanding principal across all [`RepaymentPeriod`]s.
    pub fn total_outstanding_principal(&self) -> FineractResult<Money> {
        let mut total = self.zero();
        for rp in &self.repayment_periods {
            total = total
                .plus(&rp.outstanding_principal().map_err(money_err)?)
                .map_err(money_err)?;
        }
        Ok(total)
    }
}

#[inline]
const fn rp_due_or_ip_due(ip_due: NaiveDate) -> NaiveDate {
    ip_due
}

#[inline]
fn money_err(e: MoneyError) -> FineractError {
    FineractError::domain_rule("error.msg.loan.monetary.calculation.error", e.to_string())
}

// ============================================================================
// 6. Advanced Payment Schedule Transaction Processor
// ============================================================================

/// Installment-level breakdown of a processed transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallmentAllocationDetail {
    pub installment_number: i32,
    pub principal_paid: Money,
    pub interest_paid: Money,
    pub fee_paid: Money,
    pub penalty_paid: Money,
}

/// Summary result of processing a transaction through [`AdvancedPaymentScheduleTransactionProcessor`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionProcessingResult {
    pub transaction_date: NaiveDate,
    pub transaction_amount: Money,
    pub principal_portion: Money,
    pub interest_portion: Money,
    pub fee_charges_portion: Money,
    pub penalty_charges_portion: Money,
    pub overpayment_portion: Money,
    pub installment_allocations: Vec<InstallmentAllocationDetail>,
}

/// High-level transaction input for [`AdvancedPaymentScheduleTransactionProcessor::reprocess_transactions`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressiveLoanTransaction {
    Disbursement {
        date: NaiveDate,
        amount: Money,
    },
    InterestRateChange {
        effective_date: NaiveDate,
        annual_nominal_interest_rate: Decimal,
    },
    Repayment {
        date: NaiveDate,
        amount: Money,
        transaction_type: PaymentAllocationTransactionType,
    },
    Chargeback {
        date: NaiveDate,
        amount: Money,
        original_principal: Money,
        original_interest: Money,
        original_fee: Money,
        original_penalty: Money,
    },
    Reamortization {
        date: NaiveDate,
    },
}

/// Advanced payment schedule transaction processor for Progressive Loans.
///
/// Allocates payments strictly according to [`LoanPaymentAllocationRule`] (`PastDue -> Due -> InAdvance`),
/// handles [`FutureInstallmentAllocationRule`] (`NextInstallment`, `LastInstallment`, `NextLastInstallment`,
/// `Reamortization`), dynamically splits [`InterestPeriod`]s on early or late repayments, and processes
/// [`LoanCreditAllocationRule`] chargebacks.
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.domain.transactionprocessor.impl.AdvancedPaymentScheduleTransactionProcessor`.
#[derive(Debug, Clone, Default)]
pub struct AdvancedPaymentScheduleTransactionProcessor {
    pub payment_allocation_rules: Vec<LoanPaymentAllocationRule>,
    pub credit_allocation_rules: Vec<LoanCreditAllocationRule>,
}

impl AdvancedPaymentScheduleTransactionProcessor {
    /// Creates a processor with the given payment and credit allocation rules.
    pub fn new(
        payment_allocation_rules: Vec<LoanPaymentAllocationRule>,
        credit_allocation_rules: Vec<LoanCreditAllocationRule>,
    ) -> Self {
        Self {
            payment_allocation_rules,
            credit_allocation_rules,
        }
    }

    /// Creates a processor configured with the default progressive payment allocation rule and
    /// the specified [`FutureInstallmentAllocationRule`].
    pub fn with_default_rule(
        future_installment_allocation_rule: FutureInstallmentAllocationRule,
    ) -> Self {
        Self {
            payment_allocation_rules: vec![LoanPaymentAllocationRule::default_rule(
                future_installment_allocation_rule,
            )],
            credit_allocation_rules: vec![LoanCreditAllocationRule::default_chargeback_rule()],
        }
    }

    /// Resolves the [`LoanPaymentAllocationRule`] for `transaction_type`, falling back to
    /// [`PaymentAllocationTransactionType::Default`].
    pub fn resolve_payment_rule(
        &self,
        transaction_type: PaymentAllocationTransactionType,
    ) -> LoanPaymentAllocationRule {
        self.payment_allocation_rules
            .iter()
            .find(|r| r.transaction_type == transaction_type)
            .or_else(|| {
                self.payment_allocation_rules
                    .iter()
                    .find(|r| r.transaction_type == PaymentAllocationTransactionType::Default)
            })
            .cloned()
            .unwrap_or_else(|| {
                LoanPaymentAllocationRule::default_rule(
                    FutureInstallmentAllocationRule::NextInstallment,
                )
            })
    }

    /// Resolves the [`LoanCreditAllocationRule`] for `transaction_type`.
    pub fn resolve_credit_rule(
        &self,
        transaction_type: CreditAllocationTransactionType,
    ) -> LoanCreditAllocationRule {
        self.credit_allocation_rules
            .iter()
            .find(|r| r.transaction_type == transaction_type)
            .cloned()
            .unwrap_or_else(LoanCreditAllocationRule::default_chargeback_rule)
    }

    /// Processes a repayment/payment transaction on `transaction_date` against `model`,
    /// dynamically updating `InterestPeriod` slices and allocating across installments in the
    /// exact order dictated by the resolved [`LoanPaymentAllocationRule`].
    pub fn process_repayment(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        transaction_date: NaiveDate,
        transaction_amount: Money,
        transaction_type: PaymentAllocationTransactionType,
    ) -> FineractResult<TransactionProcessingResult> {
        let rule = self.resolve_payment_rule(transaction_type);
        let zero = model.zero();

        // 1. If any past-due installments exist before `transaction_date`, inject overdue
        //    balance corrections so late interest is accurately accrued up to `transaction_date`.
        model.recalculate_overdue_amounts_till_date(transaction_date)?;

        // 2. If `transaction_date` falls strictly inside an active `RepaymentPeriod`, ensure the
        //    `InterestPeriod` is split at `transaction_date` so interest up to `transaction_date`
        //    is locked in before any early principal reduction.
        let _ = model.ensure_interest_period_split(transaction_date)?;
        model.recalculate_balances_and_interest()?;

        let mut remaining = transaction_amount.clone();
        let mut total_principal = zero.clone();
        let mut total_interest = zero.clone();
        let mut total_fee = zero.clone();
        let mut total_penalty = zero.clone();

        let num_periods = model.repayment_periods.len();
        let mut per_installment: Vec<InstallmentAllocationDetail> = model
            .repayment_periods
            .iter()
            .map(|rp| InstallmentAllocationDetail {
                installment_number: rp.installment_number,
                principal_paid: zero.clone(),
                interest_paid: zero.clone(),
                fee_paid: zero.clone(),
                penalty_paid: zero.clone(),
            })
            .collect();

        // Track how much early principal (`due_date > transaction_date`) was paid during this transaction
        let mut early_principal_paid_total = zero.clone();

        // 3. Allocate strictly in the order of `rule.allocation_types`
        for &alloc_type in &rule.allocation_types {
            if remaining.is_less_than_or_equal_to_zero() {
                break;
            }

            let candidate_indices = self.select_installment_indices(
                model,
                transaction_date,
                alloc_type.due_type(),
                rule.future_installment_allocation_rule,
            );

            for rp_idx in candidate_indices {
                if remaining.is_less_than_or_equal_to_zero() {
                    break;
                }
                if rp_idx >= num_periods {
                    continue;
                }

                let is_in_advance = model.repayment_periods[rp_idx].due_date > transaction_date;
                let paid_now = Self::allocate_component_on_period(
                    &mut model.repayment_periods[rp_idx],
                    alloc_type.allocation_type(),
                    &remaining,
                    is_in_advance,
                )?;

                if paid_now.is_greater_than_zero() {
                    remaining = remaining.minus_to_zero(&paid_now).map_err(money_err)?;
                    match alloc_type.allocation_type() {
                        AllocationType::Penalty => {
                            total_penalty = total_penalty.plus(&paid_now).map_err(money_err)?;
                            per_installment[rp_idx].penalty_paid = per_installment[rp_idx]
                                .penalty_paid
                                .plus(&paid_now)
                                .map_err(money_err)?;
                        }
                        AllocationType::Fee => {
                            total_fee = total_fee.plus(&paid_now).map_err(money_err)?;
                            per_installment[rp_idx].fee_paid = per_installment[rp_idx]
                                .fee_paid
                                .plus(&paid_now)
                                .map_err(money_err)?;
                        }
                        AllocationType::Interest => {
                            total_interest = total_interest.plus(&paid_now).map_err(money_err)?;
                            per_installment[rp_idx].interest_paid = per_installment[rp_idx]
                                .interest_paid
                                .plus(&paid_now)
                                .map_err(money_err)?;
                        }
                        AllocationType::Principal => {
                            total_principal =
                                total_principal.plus(&paid_now).map_err(money_err)?;
                            per_installment[rp_idx].principal_paid = per_installment[rp_idx]
                                .principal_paid
                                .plus(&paid_now)
                                .map_err(money_err)?;
                            if is_in_advance {
                                early_principal_paid_total = early_principal_paid_total
                                    .plus(&paid_now)
                                    .map_err(money_err)?;
                            }
                        }
                    }
                }
            }
        }

        // 4. If principal was paid in advance (`due_date > transaction_date`), record the negative
        //    balance correction `-early_principal_paid_total` at `transaction_date` so subsequent
        //    `InterestPeriod` slices immediately accrue interest on the reduced principal balance!
        if early_principal_paid_total.is_greater_than_zero() {
            model.change_outstanding_balance_and_update_interest_periods(
                transaction_date,
                zero.clone(),
                early_principal_paid_total.negated(),
                zero.clone(),
            )?;

            // If the future installment allocation rule is `Reamortization`, re-amortize remaining
            // unpaid installments over the reduced principal balance.
            if rule.future_installment_allocation_rule
                == FutureInstallmentAllocationRule::Reamortization
            {
                model.reamortize(transaction_date)?;
            }
        } else {
            model.recalculate_balances_and_interest()?;
        }

        let installment_allocations = per_installment
            .into_iter()
            .filter(|d| {
                d.principal_paid.is_greater_than_zero()
                    || d.interest_paid.is_greater_than_zero()
                    || d.fee_paid.is_greater_than_zero()
                    || d.penalty_paid.is_greater_than_zero()
            })
            .collect();

        Ok(TransactionProcessingResult {
            transaction_date,
            transaction_amount,
            principal_portion: total_principal,
            interest_portion: total_interest,
            fee_charges_portion: total_fee,
            penalty_charges_portion: total_penalty,
            overpayment_portion: remaining,
            installment_allocations,
        })
    }

    /// Processes a credit/chargeback transaction using [`LoanCreditAllocationRule`].
    #[allow(clippy::too_many_arguments)]
    pub fn process_credit_transaction(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        transaction_date: NaiveDate,
        chargeback_amount: Money,
        original_principal: Money,
        original_interest: Money,
        original_fee: Money,
        original_penalty: Money,
    ) -> FineractResult<TransactionProcessingResult> {
        let rule = self.resolve_credit_rule(CreditAllocationTransactionType::Chargeback);
        let zero = model.zero();
        let mut remaining = chargeback_amount.clone();

        let mut principal_portion = zero.clone();
        let mut interest_portion = zero.clone();
        let mut fee_portion = zero.clone();
        let mut penalty_portion = zero.clone();

        for &credit_type in &rule.allocation_types {
            if remaining.is_less_than_or_equal_to_zero() {
                break;
            }
            match credit_type {
                CreditAllocationType::Principal => {
                    let alloc = Money::min(&remaining, &original_principal).clone();
                    if alloc.is_greater_than_zero() {
                        principal_portion = principal_portion.plus(&alloc).map_err(money_err)?;
                        remaining = remaining.minus_to_zero(&alloc).map_err(money_err)?;
                    }
                }
                CreditAllocationType::Interest => {
                    let alloc = Money::min(&remaining, &original_interest).clone();
                    if alloc.is_greater_than_zero() {
                        interest_portion = interest_portion.plus(&alloc).map_err(money_err)?;
                        remaining = remaining.minus_to_zero(&alloc).map_err(money_err)?;
                    }
                }
                CreditAllocationType::Fee => {
                    let alloc = Money::min(&remaining, &original_fee).clone();
                    if alloc.is_greater_than_zero() {
                        fee_portion = fee_portion.plus(&alloc).map_err(money_err)?;
                        remaining = remaining.minus_to_zero(&alloc).map_err(money_err)?;
                    }
                }
                CreditAllocationType::Penalty => {
                    let alloc = Money::min(&remaining, &original_penalty).clone();
                    if alloc.is_greater_than_zero() {
                        penalty_portion = penalty_portion.plus(&alloc).map_err(money_err)?;
                        remaining = remaining.minus_to_zero(&alloc).map_err(money_err)?;
                    }
                }
            }
        }

        // Apply credited principal/interest to the active or upcoming installment on `transaction_date`
        let target_rp_idx = model
            .repayment_periods
            .iter()
            .position(|rp| rp.due_date >= transaction_date)
            .unwrap_or(model.repayment_periods.len().saturating_sub(1));

        let mut installment_allocations = Vec::new();
        if let Some((rp_idx, ip_idx)) = model.ensure_interest_period_split(transaction_date)? {
            if principal_portion.is_greater_than_zero() {
                model.repayment_periods[rp_idx].interest_periods[ip_idx]
                    .add_credited_principal(&principal_portion)
                    .map_err(money_err)?;
            }
            if interest_portion.is_greater_than_zero() {
                model.repayment_periods[rp_idx].interest_periods[ip_idx]
                    .add_credited_interest(&interest_portion)
                    .map_err(money_err)?;
            }
        } else if !model.repayment_periods.is_empty() {
            let rp = &mut model.repayment_periods[target_rp_idx];
            rp.credited_principal = rp
                .credited_principal
                .plus(&principal_portion)
                .map_err(money_err)?;
            rp.credited_interest = rp
                .credited_interest
                .plus(&interest_portion)
                .map_err(money_err)?;
        }

        if !model.repayment_periods.is_empty() {
            let rp = &mut model.repayment_periods[target_rp_idx];
            rp.due_fee = rp.due_fee.plus(&fee_portion).map_err(money_err)?;
            rp.credited_fee = rp.credited_fee.plus(&fee_portion).map_err(money_err)?;
            rp.due_penalty = rp.due_penalty.plus(&penalty_portion).map_err(money_err)?;
            rp.credited_penalty = rp
                .credited_penalty
                .plus(&penalty_portion)
                .map_err(money_err)?;

            installment_allocations.push(InstallmentAllocationDetail {
                installment_number: rp.installment_number,
                principal_paid: principal_portion.clone(),
                interest_paid: interest_portion.clone(),
                fee_paid: fee_portion.clone(),
                penalty_paid: penalty_portion.clone(),
            });
        }

        model.recalculate_balances_and_interest()?;

        Ok(TransactionProcessingResult {
            transaction_date,
            transaction_amount: chargeback_amount,
            principal_portion,
            interest_portion,
            fee_charges_portion: fee_portion,
            penalty_charges_portion: penalty_portion,
            overpayment_portion: remaining,
            installment_allocations,
        })
    }

    /// Reprocesses an ordered list of [`ProgressiveLoanTransaction`]s from scratch against `model`.
    pub fn reprocess_transactions(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        transactions: &[ProgressiveLoanTransaction],
    ) -> FineractResult<Vec<TransactionProcessingResult>> {
        let mut results = Vec::new();
        for txn in transactions {
            match txn {
                ProgressiveLoanTransaction::Disbursement { date, amount } => {
                    model.add_disbursement(*date, amount.clone())?;
                }
                ProgressiveLoanTransaction::InterestRateChange {
                    effective_date,
                    annual_nominal_interest_rate,
                } => {
                    model.add_interest_rate_change(*effective_date, *annual_nominal_interest_rate)?;
                }
                ProgressiveLoanTransaction::Repayment {
                    date,
                    amount,
                    transaction_type,
                } => {
                    let res = self.process_repayment(
                        model,
                        *date,
                        amount.clone(),
                        *transaction_type,
                    )?;
                    results.push(res);
                }
                ProgressiveLoanTransaction::Chargeback {
                    date,
                    amount,
                    original_principal,
                    original_interest,
                    original_fee,
                    original_penalty,
                } => {
                    let res = self.process_credit_transaction(
                        model,
                        *date,
                        amount.clone(),
                        original_principal.clone(),
                        original_interest.clone(),
                        original_fee.clone(),
                        original_penalty.clone(),
                    )?;
                    results.push(res);
                }
                ProgressiveLoanTransaction::Reamortization { date } => {
                    model.reamortize(*date)?;
                }
            }
        }
        Ok(results)
    }

    fn select_installment_indices(
        &self,
        model: &ProgressiveLoanInterestScheduleModel,
        transaction_date: NaiveDate,
        due_type: DueType,
        future_rule: FutureInstallmentAllocationRule,
    ) -> Vec<usize> {
        match due_type {
            DueType::PastDue => model
                .repayment_periods
                .iter()
                .enumerate()
                .filter(|(_, rp)| rp.due_date < transaction_date)
                .map(|(idx, _)| idx)
                .collect(),
            DueType::Due => model
                .repayment_periods
                .iter()
                .enumerate()
                .filter(|(_, rp)| rp.due_date == transaction_date)
                .map(|(idx, _)| idx)
                .collect(),
            DueType::InAdvance => {
                let mut future_indices: Vec<usize> = model
                    .repayment_periods
                    .iter()
                    .enumerate()
                    .filter(|(_, rp)| rp.due_date > transaction_date)
                    .map(|(idx, _)| idx)
                    .collect();

                match future_rule {
                    FutureInstallmentAllocationRule::NextInstallment
                    | FutureInstallmentAllocationRule::Reamortization => future_indices,
                    FutureInstallmentAllocationRule::LastInstallment => {
                        future_indices.reverse();
                        future_indices
                    }
                    FutureInstallmentAllocationRule::NextLastInstallment => {
                        // Current active period first (if `from_date <= transaction_date < due_date`),
                        // followed by remaining installments in reverse order (`LastInstallment`).
                        let current_idx = future_indices.iter().copied().find(|&idx| {
                            let rp = &model.repayment_periods[idx];
                            rp.from_date <= transaction_date && transaction_date < rp.due_date
                        });
                        future_indices.reverse();
                        if let Some(curr) = current_idx {
                            future_indices.retain(|&idx| idx != curr);
                            future_indices.insert(0, curr);
                        }
                        future_indices
                    }
                }
            }
        }
    }

    fn allocate_component_on_period(
        rp: &mut RepaymentPeriod,
        allocation_type: AllocationType,
        available: &Money,
        is_in_advance: bool,
    ) -> FineractResult<Money> {
        if available.is_less_than_or_equal_to_zero() {
            return Ok(available.zero_of());
        }

        match allocation_type {
            AllocationType::Penalty => {
                let outstanding = rp.outstanding_penalty().map_err(money_err)?;
                let alloc = Money::min(available, &outstanding).clone();
                if alloc.is_greater_than_zero() {
                    rp.paid_penalty = rp.paid_penalty.plus(&alloc).map_err(money_err)?;
                }
                Ok(alloc)
            }
            AllocationType::Fee => {
                let outstanding = rp.outstanding_fee().map_err(money_err)?;
                let alloc = Money::min(available, &outstanding).clone();
                if alloc.is_greater_than_zero() {
                    rp.paid_fee = rp.paid_fee.plus(&alloc).map_err(money_err)?;
                }
                Ok(alloc)
            }
            AllocationType::Interest => {
                let outstanding = rp.outstanding_interest().map_err(money_err)?;
                let alloc = Money::min(available, &outstanding).clone();
                if alloc.is_greater_than_zero() {
                    rp.paid_interest = rp.paid_interest.plus(&alloc).map_err(money_err)?;
                }
                Ok(alloc)
            }
            AllocationType::Principal => {
                let outstanding = rp.outstanding_principal().map_err(money_err)?;
                let alloc = Money::min(available, &outstanding).clone();
                if alloc.is_greater_than_zero() {
                    rp.paid_principal = rp.paid_principal.plus(&alloc).map_err(money_err)?;
                    if is_in_advance {
                        rp.early_paid_principal =
                            rp.early_paid_principal.plus(&alloc).map_err(money_err)?;
                    }
                }
                Ok(alloc)
            }
        }
    }
}

// ============================================================================
// 7. Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn usd() -> MonetaryCurrency {
        MonetaryCurrency::new("USD", 2, None)
    }

    fn money(amount: Decimal) -> Money {
        Money::of(usd(), amount, FineractRoundingMode::HalfEven)
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn sample_three_month_dates() -> Vec<(NaiveDate, NaiveDate)> {
        vec![
            (date(2024, 1, 1), date(2024, 1, 31)),
            (date(2024, 1, 31), date(2024, 3, 1)),
            (date(2024, 3, 1), date(2024, 3, 31)),
        ]
    }

    #[test]
    fn test_payment_and_credit_allocation_rule_roundtrip() {
        let rule = LoanPaymentAllocationRule::from_csv(
            "DEFAULT",
            "PAST_DUE_PENALTY,PAST_DUE_FEE,PAST_DUE_INTEREST,PAST_DUE_PRINCIPAL,DUE_PENALTY,DUE_FEE,DUE_INTEREST,DUE_PRINCIPAL,IN_ADVANCE_PENALTY,IN_ADVANCE_FEE,IN_ADVANCE_PRINCIPAL,IN_ADVANCE_INTEREST",
            "LAST_INSTALLMENT",
        )
        .expect("valid payment allocation rule");

        assert_eq!(
            rule.transaction_type,
            PaymentAllocationTransactionType::Default
        );
        assert_eq!(rule.allocation_types.len(), 12);
        assert_eq!(
            rule.allocation_types[0].due_type(),
            DueType::PastDue
        );
        assert_eq!(
            rule.allocation_types[0].allocation_type(),
            AllocationType::Penalty
        );
        assert_eq!(
            rule.future_installment_allocation_rule,
            FutureInstallmentAllocationRule::LastInstallment
        );
        assert_eq!(LoanPaymentAllocationRule::TABLE_NAME, "m_loan_payment_allocation_rule");

        let credit_rule =
            LoanCreditAllocationRule::from_csv("CHARGEBACK", "PRINCIPAL,INTEREST,FEE,PENALTY")
                .expect("valid credit allocation rule");
        assert_eq!(
            credit_rule.transaction_type,
            CreditAllocationTransactionType::Chargeback
        );
        assert_eq!(
            credit_rule.allocation_types,
            vec![
                CreditAllocationType::Principal,
                CreditAllocationType::Interest,
                CreditAllocationType::Fee,
                CreditAllocationType::Penalty,
            ]
        );
        assert_eq!(LoanCreditAllocationRule::TABLE_NAME, "m_loan_credit_allocation_rule");
    }

    #[test]
    fn test_on_time_progressive_repayments() {
        // $1,000.00 disbursed on 2024-01-01 across 3 equal 30-day monthly installments at 12% nominal annual rate (1% per month)
        let params = ProgressiveLoanScheduleParams::monthly(usd(), dec!(12.0));
        let mut model = ProgressiveLoanInterestScheduleModel::generate_schedule(
            &sample_three_month_dates(),
            Some(money(dec!(1000.00))),
            params,
        )
        .expect("schedule generated");

        // Regular EMI = $340.02, last installment EMI adjusted by +$0.01 to $340.03
        assert_eq!(model.repayment_periods[0].emi, money(dec!(340.02)));
        assert_eq!(
            model.repayment_periods[0].due_interest().unwrap(),
            money(dec!(10.00))
        );
        assert_eq!(
            model.repayment_periods[0].due_principal().unwrap(),
            money(dec!(330.02))
        );

        assert_eq!(model.repayment_periods[1].emi, money(dec!(340.02)));
        assert_eq!(
            model.repayment_periods[1].due_interest().unwrap(),
            money(dec!(6.70))
        );
        assert_eq!(
            model.repayment_periods[1].due_principal().unwrap(),
            money(dec!(333.32))
        );

        assert_eq!(model.repayment_periods[2].emi, money(dec!(340.03)));
        assert_eq!(
            model.repayment_periods[2].due_interest().unwrap(),
            money(dec!(3.37))
        );
        assert_eq!(
            model.repayment_periods[2].due_principal().unwrap(),
            money(dec!(336.66))
        );
        assert_eq!(model.total_due_principal().unwrap(), money(dec!(1000.00)));

        let processor = AdvancedPaymentScheduleTransactionProcessor::with_default_rule(
            FutureInstallmentAllocationRule::NextInstallment,
        );

        // Pay Installment 1 on time (2024-01-31)
        let r1 = processor
            .process_repayment(
                &mut model,
                date(2024, 1, 31),
                money(dec!(340.02)),
                PaymentAllocationTransactionType::Repayment,
            )
            .unwrap();
        assert_eq!(r1.interest_portion, money(dec!(10.00)));
        assert_eq!(r1.principal_portion, money(dec!(330.02)));
        assert!(model.repayment_periods[0].is_fully_paid().unwrap());

        // Pay Installment 2 on time (2024-03-01)
        let r2 = processor
            .process_repayment(
                &mut model,
                date(2024, 3, 1),
                money(dec!(340.02)),
                PaymentAllocationTransactionType::Repayment,
            )
            .unwrap();
        assert_eq!(r2.interest_portion, money(dec!(6.70)));
        assert_eq!(r2.principal_portion, money(dec!(333.32)));
        assert!(model.repayment_periods[1].is_fully_paid().unwrap());

        // Pay Installment 3 on time (2024-03-31)
        let r3 = processor
            .process_repayment(
                &mut model,
                date(2024, 3, 31),
                money(dec!(340.03)),
                PaymentAllocationTransactionType::Repayment,
            )
            .unwrap();
        assert_eq!(r3.interest_portion, money(dec!(3.37)));
        assert_eq!(r3.principal_portion, money(dec!(336.66)));
        assert!(model.repayment_periods[2].is_fully_paid().unwrap());
        assert_eq!(model.total_paid_principal().unwrap(), money(dec!(1000.00)));
        assert_eq!(model.total_paid_interest().unwrap(), money(dec!(20.07)));
    }

    #[test]
    fn test_early_mid_period_repayment_splits_interest_period_and_reduces_interest() {
        let params = ProgressiveLoanScheduleParams::monthly(usd(), dec!(12.0));
        let mut model = ProgressiveLoanInterestScheduleModel::generate_schedule(
            &sample_three_month_dates(),
            Some(money(dec!(1000.00))),
            params,
        )
        .expect("schedule generated");

        let processor = AdvancedPaymentScheduleTransactionProcessor::with_default_rule(
            FutureInstallmentAllocationRule::NextInstallment,
        );

        // Early repayment of $200.00 on 2024-01-16 (exactly 15 days into the 30-day first period)
        let early_res = processor
            .process_repayment(
                &mut model,
                date(2024, 1, 16),
                money(dec!(200.00)),
                PaymentAllocationTransactionType::Repayment,
            )
            .unwrap();

        // Under IN_ADVANCE allocation order, IN_ADVANCE_PRINCIPAL comes before IN_ADVANCE_INTEREST
        assert_eq!(early_res.principal_portion, money(dec!(200.00)));
        assert_eq!(early_res.interest_portion, money(dec!(0.00)));

        // Verify Period 1's InterestPeriod was dynamically split into 2 slices:
        // [2024-01-01..2024-01-16] ($1,000 balance -> $5.00 interest)
        // [2024-01-16..2024-01-31] ($800 balance -> $4.00 interest)
        let rp1 = &model.repayment_periods[0];
        assert_eq!(rp1.interest_periods.len(), 2);
        assert_eq!(rp1.interest_periods[0].from_date, date(2024, 1, 1));
        assert_eq!(rp1.interest_periods[0].due_date, date(2024, 1, 16));
        assert_eq!(
            rp1.interest_periods[0].outstanding_loan_balance,
            money(dec!(1000.00))
        );
        assert_eq!(
            rp1.interest_periods[0].calculated_due_interest,
            money(dec!(5.00))
        );

        assert_eq!(rp1.interest_periods[1].from_date, date(2024, 1, 16));
        assert_eq!(rp1.interest_periods[1].due_date, date(2024, 1, 31));
        assert_eq!(
            rp1.interest_periods[1].balance_correction_amount,
            money(dec!(-200.00))
        );
        assert_eq!(
            rp1.interest_periods[1].outstanding_loan_balance,
            money(dec!(800.00))
        );
        assert_eq!(
            rp1.interest_periods[1].calculated_due_interest,
            money(dec!(4.00))
        );

        // Total due interest on Period 1 dropped from $10.00 to $9.00!
        assert_eq!(rp1.due_interest().unwrap(), money(dec!(9.00)));
        assert_eq!(rp1.due_principal().unwrap(), money(dec!(331.02)));
        assert_eq!(rp1.outstanding_principal().unwrap(), money(dec!(131.02)));
        assert_eq!(model.total_due_principal().unwrap(), money(dec!(1000.00)));
    }

    #[test]
    fn test_early_repayment_with_last_installment_rule_and_reamortization() {
        // 1. Test `LastInstallment` rule: early $200 payment on 2024-01-16 reduces the 3rd (last) installment
        let params = ProgressiveLoanScheduleParams::monthly(usd(), dec!(12.0));
        let mut model_last = ProgressiveLoanInterestScheduleModel::generate_schedule(
            &sample_three_month_dates(),
            Some(money(dec!(1000.00))),
            params.clone(),
        )
        .unwrap();

        let proc_last = AdvancedPaymentScheduleTransactionProcessor::with_default_rule(
            FutureInstallmentAllocationRule::LastInstallment,
        );
        let res_last = proc_last
            .process_repayment(
                &mut model_last,
                date(2024, 1, 16),
                money(dec!(200.00)),
                PaymentAllocationTransactionType::Repayment,
            )
            .unwrap();
        assert_eq!(res_last.installment_allocations[0].installment_number, 3);
        assert_eq!(
            model_last.repayment_periods[2].paid_principal,
            money(dec!(200.00))
        );
        // Period 2 starts at $800.00 - $331.02 = $468.98
        assert_eq!(
            model_last.repayment_periods[1].interest_periods[0].outstanding_loan_balance,
            money(dec!(468.98))
        );

        // 2. Test `Reamortization` rule: early $200 payment on 2024-01-16 re-amortizes remaining installments
        let mut model_reamort = ProgressiveLoanInterestScheduleModel::generate_schedule(
            &sample_three_month_dates(),
            Some(money(dec!(1000.00))),
            params,
        )
        .unwrap();

        let proc_reamort = AdvancedPaymentScheduleTransactionProcessor::with_default_rule(
            FutureInstallmentAllocationRule::Reamortization,
        );
        proc_reamort
            .process_repayment(
                &mut model_reamort,
                date(2024, 1, 16),
                money(dec!(200.00)),
                PaymentAllocationTransactionType::Repayment,
            )
            .unwrap();

        // After $200 early principal payment and re-amortization, total scheduled due principal is still $1,000.00
        assert_eq!(
            model_reamort.total_due_principal().unwrap(),
            money(dec!(1000.00))
        );
        // And Period 2's recalculated EMI is lower than the original $340.02 EMI
        assert!(model_reamort.repayment_periods[1]
            .emi
            .is_less_than(&money(dec!(340.02))));
    }

    #[test]
    fn test_late_repayment_splits_subsequent_period_and_accrues_extra_overdue_interest() {
        let params = ProgressiveLoanScheduleParams::monthly(usd(), dec!(12.0));
        let mut model = ProgressiveLoanInterestScheduleModel::generate_schedule(
            &sample_three_month_dates(),
            Some(money(dec!(1000.00))),
            params,
        )
        .unwrap();

        let processor = AdvancedPaymentScheduleTransactionProcessor::with_default_rule(
            FutureInstallmentAllocationRule::NextInstallment,
        );

        // Installment 1 (due 2024-01-31) is NOT paid on 2024-01-31.
        // Instead, borrower pays $340.02 10 days late on 2024-02-10 (inside Period 2: 2024-01-31..2024-03-01).
        let late_res = processor
            .process_repayment(
                &mut model,
                date(2024, 2, 10),
                money(dec!(340.02)),
                PaymentAllocationTransactionType::Repayment,
            )
            .unwrap();

        // Allocated to PastDue Installment 1
        assert_eq!(late_res.installment_allocations[0].installment_number, 1);
        assert_eq!(late_res.interest_portion, money(dec!(10.00)));
        assert_eq!(late_res.principal_portion, money(dec!(330.02)));
        assert!(model.repayment_periods[0].is_fully_paid().unwrap());

        // Period 2 (2024-01-31..2024-03-01) was split at 2024-02-10:
        // - Slice 0 [2024-01-31..2024-02-10]: had +$330.02 overdue balance correction -> $1,000.00 balance!
        // - Slice 1 [2024-02-10..2024-03-01]: had -$330.02 correction when paid -> $669.98 balance!
        let rp2 = &model.repayment_periods[1];
        assert_eq!(rp2.interest_periods.len(), 2);
        assert_eq!(
            rp2.interest_periods[0].outstanding_loan_balance,
            money(dec!(1000.00))
        );
        assert_eq!(
            rp2.interest_periods[1].outstanding_loan_balance,
            money(dec!(669.98))
        );

        // Period 2's due interest ($3.33 + $4.47 = $7.80) is higher than the on-time $6.70!
        assert_eq!(rp2.due_interest().unwrap(), money(dec!(7.80)));
    }

    #[test]
    fn test_mid_period_rate_change_tranche_disbursement_and_chargeback() {
        let params = ProgressiveLoanScheduleParams::monthly(usd(), dec!(12.0));
        let mut model = ProgressiveLoanInterestScheduleModel::generate_schedule(
            &sample_three_month_dates(),
            Some(money(dec!(1000.00))),
            params,
        )
        .unwrap();

        // 1. Mid-period interest rate change on 2024-01-16 from 12% to 18%
        model
            .add_interest_rate_change(date(2024, 1, 16), dec!(18.0))
            .unwrap();
        assert_eq!(model.repayment_periods[0].interest_periods.len(), 2);
        assert_eq!(
            model.repayment_periods[0].interest_periods[0].rate_factor,
            dec!(0.005)
        );
        assert_eq!(
            model.repayment_periods[0].interest_periods[1].rate_factor,
            dec!(0.0075)
        );

        // 2. Second disbursement tranche of $500.00 on 2024-01-31 (start of Period 2)
        model
            .add_disbursement(date(2024, 1, 31), money(dec!(500.00)))
            .unwrap();
        assert_eq!(model.total_due_principal().unwrap(), money(dec!(1500.00)));

        // 3. Chargeback allocation using LoanCreditAllocationRule
        let processor = AdvancedPaymentScheduleTransactionProcessor::with_default_rule(
            FutureInstallmentAllocationRule::NextInstallment,
        );
        let cb_res = processor
            .process_credit_transaction(
                &mut model,
                date(2024, 2, 15),
                money(dec!(120.00)),
                money(dec!(100.00)),
                money(dec!(15.00)),
                money(dec!(5.00)),
                money(dec!(0.00)),
            )
            .unwrap();
        assert_eq!(cb_res.principal_portion, money(dec!(100.00)));
        assert_eq!(cb_res.interest_portion, money(dec!(15.00)));
        assert_eq!(cb_res.fee_charges_portion, money(dec!(5.00)));
        assert_eq!(model.total_due_principal().unwrap(), money(dec!(1600.00)));
    }
}
