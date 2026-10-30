//! Loan Repayment Schedule Installments, Loan Charges, Term Variations, Transactions, and Summary.
//!
//! Faithfully ported from `org.apache.fineract.portfolio.loanaccount.domain`:
//! - [`LoanTransactionType`] (`LoanTransactionType.java`)
//! - [`LoanTermVariationType`] (`LoanTermVariationType.java`)
//! - [`LoanRepaymentScheduleInstallment`] (`LoanRepaymentScheduleInstallment.java`, `m_loan_repayment_schedule`)
//! - [`LoanCharge`] (`LoanCharge.java`, `m_loan_charge`)
//! - [`LoanInstallmentCharge`] (`LoanInstallmentCharge.java`, `m_loan_installment_charge`)
//! - [`LoanTermVariations`] (`LoanTermVariations.java`, `m_loan_term_variations`)
//! - [`LoanTransactionToRepaymentScheduleMapping`] (`LoanTransactionToRepaymentScheduleMapping.java`, `m_loan_transaction_repayment_schedule_mapping`)
//! - [`LoanTransaction`] (`LoanTransaction.java`, `m_loan_transaction`)
//! - [`LoanSummary`] (`LoanSummary.java`)

use std::cmp::Ordering;

use chrono::NaiveDate;
use fineract_charge_rs::{
    ChargeCalculationType, ChargePaymentMode, ChargeTimeType, EnumOptionData,
};
use fineract_core_rs::primitives::ExternalId;
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// =============================================================================
// 1. LoanTransactionType (LoanTransactionType.java)
// =============================================================================

/// Monetary and non-monetary transaction types recorded against a loan account (`transaction_type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanTransactionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum LoanTransactionType {
    #[default]
    Invalid = 0,
    Disbursement = 1,
    Repayment = 2,
    Contra = 3,
    WaiveInterest = 4,
    RepaymentAtDisbursement = 5,
    WriteOff = 6,
    MarkedForRescheduling = 7,
    RecoveryRepayment = 8,
    WaiveCharges = 9,
    Accrual = 10,
    InitiateTransfer = 12,
    ApproveTransfer = 13,
    WithdrawTransfer = 14,
    RejectTransfer = 15,
    Refund = 16,
    ChargePayment = 17,
    RefundForActiveLoan = 18,
    IncomePosting = 19,
    CreditBalanceRefund = 20,
    MerchantIssuedRefund = 21,
    PayoutRefund = 22,
    GoodwillCredit = 23,
    ChargeRefund = 24,
    Chargeback = 25,
    ChargeAdjustment = 26,
    ChargeOff = 27,
    DownPayment = 28,
    ReAge = 29,
    ReAmortize = 30,
    AccrualActivity = 31,
    InterestPaymentWaiver = 32,
    CapitalizedIncome = 33,
    BuyDownFee = 34,
}

impl LoanTransactionType {
    pub const INVALID: Self = Self::Invalid;
    pub const DISBURSEMENT: Self = Self::Disbursement;
    pub const REPAYMENT: Self = Self::Repayment;
    pub const CONTRA: Self = Self::Contra;
    pub const WAIVE_INTEREST: Self = Self::WaiveInterest;
    pub const REPAYMENT_AT_DISBURSEMENT: Self = Self::RepaymentAtDisbursement;
    pub const WRITEOFF: Self = Self::WriteOff;
    pub const WRITE_OFF: Self = Self::WriteOff;
    pub const MARKED_FOR_RESCHEDULING: Self = Self::MarkedForRescheduling;
    pub const RECOVERY_REPAYMENT: Self = Self::RecoveryRepayment;
    pub const WAIVE_CHARGES: Self = Self::WaiveCharges;
    pub const ACCRUAL: Self = Self::Accrual;
    pub const INITIATE_TRANSFER: Self = Self::InitiateTransfer;
    pub const APPROVE_TRANSFER: Self = Self::ApproveTransfer;
    pub const WITHDRAW_TRANSFER: Self = Self::WithdrawTransfer;
    pub const REJECT_TRANSFER: Self = Self::RejectTransfer;
    pub const REFUND: Self = Self::Refund;
    pub const CHARGE_PAYMENT: Self = Self::ChargePayment;
    pub const REFUND_FOR_ACTIVE_LOAN: Self = Self::RefundForActiveLoan;
    pub const INCOME_POSTING: Self = Self::IncomePosting;
    pub const CREDIT_BALANCE_REFUND: Self = Self::CreditBalanceRefund;
    pub const MERCHANT_ISSUED_REFUND: Self = Self::MerchantIssuedRefund;
    pub const PAYOUT_REFUND: Self = Self::PayoutRefund;
    pub const GOODWILL_CREDIT: Self = Self::GoodwillCredit;
    pub const CHARGE_REFUND: Self = Self::ChargeRefund;
    pub const CHARGEBACK: Self = Self::Chargeback;
    pub const CHARGE_ADJUSTMENT: Self = Self::ChargeAdjustment;
    pub const CHARGE_OFF: Self = Self::ChargeOff;
    pub const DOWN_PAYMENT: Self = Self::DownPayment;
    pub const REAGE: Self = Self::ReAge;
    pub const RE_AGE: Self = Self::ReAge;
    pub const REAMORTIZE: Self = Self::ReAmortize;
    pub const RE_AMORTIZE: Self = Self::ReAmortize;
    pub const ACCRUAL_ACTIVITY: Self = Self::AccrualActivity;
    pub const INTEREST_PAYMENT_WAIVER: Self = Self::InterestPaymentWaiver;
    pub const CAPITALIZED_INCOME: Self = Self::CapitalizedIncome;
    pub const BUY_DOWN_FEE: Self = Self::BuyDownFee;

    pub const REPAYMENT_LIKE_TYPES: &'static [Self] = &[
        Self::Repayment,
        Self::MerchantIssuedRefund,
        Self::PayoutRefund,
        Self::GoodwillCredit,
        Self::ChargeRefund,
        Self::ChargeAdjustment,
        Self::DownPayment,
        Self::InterestPaymentWaiver,
    ];

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "loanTransactionType.invalid",
            Self::Disbursement => "loanTransactionType.disbursement",
            Self::Repayment => "loanTransactionType.repayment",
            Self::Contra => "loanTransactionType.contra",
            Self::WaiveInterest => "loanTransactionType.waiver",
            Self::RepaymentAtDisbursement => "loanTransactionType.repaymentAtDisbursement",
            Self::WriteOff => "loanTransactionType.writeOff",
            Self::MarkedForRescheduling => "loanTransactionType.marked.for.rescheduling",
            Self::RecoveryRepayment => "loanTransactionType.recoveryRepayment",
            Self::WaiveCharges => "loanTransactionType.waiveCharges",
            Self::Accrual => "loanTransactionType.accrual",
            Self::InitiateTransfer => "loanTransactionType.initiateTransfer",
            Self::ApproveTransfer => "loanTransactionType.approveTransfer",
            Self::WithdrawTransfer => "loanTransactionType.withdrawTransfer",
            Self::RejectTransfer => "loanTransactionType.rejectTransfer",
            Self::Refund => "loanTransactionType.refund",
            Self::ChargePayment => "loanTransactionType.chargePayment",
            Self::RefundForActiveLoan => "loanTransactionType.refund",
            Self::IncomePosting => "loanTransactionType.incomePosting",
            Self::CreditBalanceRefund => "loanTransactionType.creditBalanceRefund",
            Self::MerchantIssuedRefund => "loanTransactionType.merchantIssuedRefund",
            Self::PayoutRefund => "loanTransactionType.payoutRefund",
            Self::GoodwillCredit => "loanTransactionType.goodwillCredit",
            Self::ChargeRefund => "loanTransactionType.chargeRefund",
            Self::Chargeback => "loanTransactionType.chargeback",
            Self::ChargeAdjustment => "loanTransactionType.chargeAdjustment",
            Self::ChargeOff => "loanTransactionType.chargeOff",
            Self::DownPayment => "loanTransactionType.downPayment",
            Self::ReAge => "loanTransactionType.reAge",
            Self::ReAmortize => "loanTransactionType.reAmortize",
            Self::AccrualActivity => "loanTransactionType.accrualActivity",
            Self::InterestPaymentWaiver => "loanTransactionType.interestPaymentWaiver",
            Self::CapitalizedIncome => "loanTransactionType.capitalizedIncome",
            Self::BuyDownFee => "loanTransactionType.buyDownFee",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Disbursement => "Disbursement",
            Self::Repayment => "Repayment",
            Self::Contra => "Contra",
            Self::WaiveInterest => "Waive Interest",
            Self::RepaymentAtDisbursement => "Repayment (at time of disbursement)",
            Self::WriteOff => "Close (as written-off)",
            Self::MarkedForRescheduling => "Close (as rescheduled)",
            Self::RecoveryRepayment => "Recovery Payment",
            Self::WaiveCharges => "Waive Loan Charges",
            Self::Accrual => "Accrual",
            Self::InitiateTransfer => "Initiate Transfer",
            Self::ApproveTransfer => "Approve Transfer",
            Self::WithdrawTransfer => "Withdraw Transfer",
            Self::RejectTransfer => "Reject Transfer",
            Self::Refund => "Refund",
            Self::ChargePayment => "Charge Payment",
            Self::RefundForActiveLoan => "Refund for Active Loan",
            Self::IncomePosting => "Income Posting",
            Self::CreditBalanceRefund => "Credit Balance Refund",
            Self::MerchantIssuedRefund => "Merchant Issued Refund",
            Self::PayoutRefund => "Payout Refund",
            Self::GoodwillCredit => "Goodwill Credit",
            Self::ChargeRefund => "Charge Refund",
            Self::Chargeback => "Chargeback",
            Self::ChargeAdjustment => "Charge Adjustment",
            Self::ChargeOff => "Charge-off",
            Self::DownPayment => "Down Payment",
            Self::ReAge => "Re-age",
            Self::ReAmortize => "Re-amortize",
            Self::AccrualActivity => "Accrual Activity",
            Self::InterestPaymentWaiver => "Interest Payment Waiver",
            Self::CapitalizedIncome => "Capitalized Income",
            Self::BuyDownFee => "Buy Down Fee",
        }
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Disbursement,
            2 => Self::Repayment,
            3 => Self::Contra,
            4 => Self::WaiveInterest,
            5 => Self::RepaymentAtDisbursement,
            6 => Self::WriteOff,
            7 => Self::MarkedForRescheduling,
            8 => Self::RecoveryRepayment,
            9 => Self::WaiveCharges,
            10 => Self::Accrual,
            12 => Self::InitiateTransfer,
            13 => Self::ApproveTransfer,
            14 => Self::WithdrawTransfer,
            15 => Self::RejectTransfer,
            16 => Self::Refund,
            17 => Self::ChargePayment,
            18 => Self::RefundForActiveLoan,
            19 => Self::IncomePosting,
            20 => Self::CreditBalanceRefund,
            21 => Self::MerchantIssuedRefund,
            22 => Self::PayoutRefund,
            23 => Self::GoodwillCredit,
            24 => Self::ChargeRefund,
            25 => Self::Chargeback,
            26 => Self::ChargeAdjustment,
            27 => Self::ChargeOff,
            28 => Self::DownPayment,
            29 => Self::ReAge,
            30 => Self::ReAmortize,
            31 => Self::AccrualActivity,
            32 => Self::InterestPaymentWaiver,
            33 => Self::CapitalizedIncome,
            34 => Self::BuyDownFee,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(value: i32) -> Self {
        if value < i16::MIN as i32 || value > i16::MAX as i32 {
            Self::Invalid
        } else {
            Self::from_i16(value as i16)
        }
    }

    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_disbursement(self) -> bool {
        matches!(self, Self::Disbursement)
    }

    #[inline]
    pub const fn is_repayment_at_disbursement(self) -> bool {
        matches!(self, Self::RepaymentAtDisbursement)
    }

    #[inline]
    pub const fn is_repayment(self) -> bool {
        matches!(self, Self::Repayment)
    }

    #[inline]
    pub const fn is_interest_payment_waiver(self) -> bool {
        matches!(self, Self::InterestPaymentWaiver)
    }

    #[inline]
    pub const fn is_merchant_issued_refund(self) -> bool {
        matches!(self, Self::MerchantIssuedRefund)
    }

    #[inline]
    pub const fn is_payout_refund(self) -> bool {
        matches!(self, Self::PayoutRefund)
    }

    #[inline]
    pub const fn is_goodwill_credit(self) -> bool {
        matches!(self, Self::GoodwillCredit)
    }

    #[inline]
    pub const fn is_charge_refund(self) -> bool {
        matches!(self, Self::ChargeRefund)
    }

    #[inline]
    pub const fn is_down_payment(self) -> bool {
        matches!(self, Self::DownPayment)
    }

    #[inline]
    pub const fn is_charge_adjustment(self) -> bool {
        matches!(self, Self::ChargeAdjustment)
    }

    /// Mirrors `LoanTransactionType.isRepaymentType()` in Java.
    #[inline]
    pub const fn is_repayment_type(self) -> bool {
        self.is_repayment()
            || self.is_merchant_issued_refund()
            || self.is_payout_refund()
            || self.is_goodwill_credit()
            || self.is_charge_refund()
            || self.is_down_payment()
            || self.is_interest_payment_waiver()
            || self.is_charge_adjustment()
    }

    #[inline]
    pub const fn is_recovery_repayment(self) -> bool {
        matches!(self, Self::RecoveryRepayment)
    }

    #[inline]
    pub const fn is_waive_interest(self) -> bool {
        matches!(self, Self::WaiveInterest)
    }

    #[inline]
    pub const fn is_waive_charges(self) -> bool {
        matches!(self, Self::WaiveCharges)
    }

    #[inline]
    pub const fn is_accrual(self) -> bool {
        matches!(self, Self::Accrual)
    }

    #[inline]
    pub const fn is_write_off(self) -> bool {
        matches!(self, Self::WriteOff)
    }

    #[inline]
    pub const fn is_charge_payment(self) -> bool {
        matches!(self, Self::ChargePayment)
    }

    #[inline]
    pub const fn is_refund_for_active_loan(self) -> bool {
        matches!(self, Self::RefundForActiveLoan)
    }

    #[inline]
    pub const fn is_income_posting(self) -> bool {
        matches!(self, Self::IncomePosting)
    }

    #[inline]
    pub const fn is_credit_balance_refund(self) -> bool {
        matches!(self, Self::CreditBalanceRefund)
    }

    #[inline]
    pub const fn is_refund(self) -> bool {
        matches!(self, Self::Refund)
    }

    #[inline]
    pub const fn is_chargeback(self) -> bool {
        matches!(self, Self::Chargeback)
    }

    #[inline]
    pub const fn is_charge_off(self) -> bool {
        matches!(self, Self::ChargeOff)
    }

    #[inline]
    pub const fn is_re_age(self) -> bool {
        matches!(self, Self::ReAge)
    }

    #[inline]
    pub const fn is_re_amortize(self) -> bool {
        matches!(self, Self::ReAmortize)
    }

    #[inline]
    pub const fn is_accrual_activity(self) -> bool {
        matches!(self, Self::AccrualActivity)
    }

    #[inline]
    pub const fn is_capitalized_income(self) -> bool {
        matches!(self, Self::CapitalizedIncome)
    }

    #[inline]
    pub const fn is_buy_down_fee(self) -> bool {
        matches!(self, Self::BuyDownFee)
    }

    #[inline]
    pub const fn repayment_like_transaction_types() -> &'static [Self] {
        Self::REPAYMENT_LIKE_TYPES
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

// =============================================================================
// 2. LoanTermVariationType (LoanTermVariationType.java)
// =============================================================================

/// Type of schedule or interest variation applied to a loan (`term_type`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanTermVariationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum LoanTermVariationType {
    #[default]
    Invalid = 0,
    EmiAmount = 1,
    InterestRate = 2,
    PrincipalAmount = 3,
    DueDate = 4,
    InsertInstallment = 5,
    DeleteInstallment = 6,
    GraceOnInterest = 7,
    GraceOnPrincipal = 8,
    ExtendRepaymentPeriod = 9,
    InterestRateFromInstallment = 10,
}

impl LoanTermVariationType {
    pub const INVALID: Self = Self::Invalid;
    pub const EMI_AMOUNT: Self = Self::EmiAmount;
    pub const INTEREST_RATE: Self = Self::InterestRate;
    pub const PRINCIPAL_AMOUNT: Self = Self::PrincipalAmount;
    pub const DUE_DATE: Self = Self::DueDate;
    pub const INSERT_INSTALLMENT: Self = Self::InsertInstallment;
    pub const DELETE_INSTALLMENT: Self = Self::DeleteInstallment;
    pub const GRACE_ON_INTEREST: Self = Self::GraceOnInterest;
    pub const GRACE_ON_PRINCIPAL: Self = Self::GraceOnPrincipal;
    pub const EXTEND_REPAYMENT_PERIOD: Self = Self::ExtendRepaymentPeriod;
    pub const INTEREST_RATE_FROM_INSTALLMENT: Self = Self::InterestRateFromInstallment;

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "loanTermType.invalid",
            Self::EmiAmount => "loanTermType.emiAmount",
            Self::InterestRate => "loanTermType.interestRate",
            Self::PrincipalAmount => "loanTermType.principalAmount",
            Self::DueDate => "loanTermType.dueDate",
            Self::InsertInstallment => "loanTermType.insertInstallment",
            Self::DeleteInstallment => "loanTermType.deleteInstallment",
            Self::GraceOnInterest => "loanTermType.graceOnInterest",
            Self::GraceOnPrincipal => "loanTermType.graceOnPrincipal",
            Self::ExtendRepaymentPeriod => "loanTermType.extendRepaymentPeriod",
            Self::InterestRateFromInstallment => "loanTermType.interestRateFromInstallment",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::EmiAmount => "EMI Amount",
            Self::InterestRate => "Interest Rate",
            Self::PrincipalAmount => "Principal Amount",
            Self::DueDate => "Due Date",
            Self::InsertInstallment => "Insert Installment",
            Self::DeleteInstallment => "Delete Installment",
            Self::GraceOnInterest => "Grace On Interest",
            Self::GraceOnPrincipal => "Grace On Principal",
            Self::ExtendRepaymentPeriod => "Extend Repayment Period",
            Self::InterestRateFromInstallment => "Interest Rate From Installment",
        }
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::EmiAmount,
            2 => Self::InterestRate,
            3 => Self::PrincipalAmount,
            4 => Self::DueDate,
            5 => Self::InsertInstallment,
            6 => Self::DeleteInstallment,
            7 => Self::GraceOnInterest,
            8 => Self::GraceOnPrincipal,
            9 => Self::ExtendRepaymentPeriod,
            10 => Self::InterestRateFromInstallment,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i32(value: i32) -> Self {
        if value < i16::MIN as i32 || value > i16::MAX as i32 {
            Self::Invalid
        } else {
            Self::from_i16(value as i16)
        }
    }

    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    #[inline]
    pub const fn is_emi_amount_variation(self) -> bool {
        matches!(self, Self::EmiAmount)
    }

    #[inline]
    pub const fn is_interest_rate_variation(self) -> bool {
        matches!(self, Self::InterestRate)
    }

    #[inline]
    pub const fn is_principal_amount_variation(self) -> bool {
        matches!(self, Self::PrincipalAmount)
    }

    #[inline]
    pub const fn is_due_date_variation(self) -> bool {
        matches!(self, Self::DueDate)
    }

    #[inline]
    pub const fn is_insert_installment(self) -> bool {
        matches!(self, Self::InsertInstallment)
    }

    #[inline]
    pub const fn is_delete_installment(self) -> bool {
        matches!(self, Self::DeleteInstallment)
    }

    #[inline]
    pub const fn is_grace_on_interest(self) -> bool {
        matches!(self, Self::GraceOnInterest)
    }

    #[inline]
    pub const fn is_grace_on_principal(self) -> bool {
        matches!(self, Self::GraceOnPrincipal)
    }

    #[inline]
    pub const fn is_extend_repayment_period(self) -> bool {
        matches!(self, Self::ExtendRepaymentPeriod)
    }

    #[inline]
    pub const fn is_interest_rate_from_installment(self) -> bool {
        matches!(self, Self::InterestRateFromInstallment)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

// =============================================================================
// 3. LoanRepaymentScheduleInstallment (LoanRepaymentScheduleInstallment.java)
// =============================================================================

/// Flexible argument adapter allowing `(NaiveDate, Decimal)` or `(Decimal, NaiveDate)`
/// when paying or waiving an installment component.
pub trait InstallmentPaymentArgs {
    fn into_date_and_amount(self) -> (NaiveDate, Decimal);
}

impl InstallmentPaymentArgs for (NaiveDate, Decimal) {
    #[inline]
    fn into_date_and_amount(self) -> (NaiveDate, Decimal) {
        self
    }
}

impl InstallmentPaymentArgs for (Decimal, NaiveDate) {
    #[inline]
    fn into_date_and_amount(self) -> (NaiveDate, Decimal) {
        (self.1, self.0)
    }
}

impl InstallmentPaymentArgs for (NaiveDate, Money) {
    #[inline]
    fn into_date_and_amount(self) -> (NaiveDate, Decimal) {
        (self.0, self.1.amount())
    }
}

impl InstallmentPaymentArgs for (Money, NaiveDate) {
    #[inline]
    fn into_date_and_amount(self) -> (NaiveDate, Decimal) {
        (self.1, self.0.amount())
    }
}

/// Single repayment period on a loan schedule (`m_loan_repayment_schedule`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanRepaymentScheduleInstallment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoanRepaymentScheduleInstallment {
    pub id: Option<i64>,
    pub loan_id: Option<i64>,
    pub installment_number: i32,
    pub from_date: Option<NaiveDate>,
    pub due_date: NaiveDate,
    pub principal_amount: Decimal,
    pub principal_completed: Decimal,
    pub principal_writtenoff: Decimal,
    pub interest_amount: Decimal,
    pub interest_completed: Decimal,
    pub interest_waived: Decimal,
    pub interest_writtenoff: Decimal,
    pub interest_accrued: Decimal,
    pub fee_charges_amount: Decimal,
    pub fee_charges_completed: Decimal,
    pub fee_charges_waived: Decimal,
    pub fee_charges_writtenoff: Decimal,
    pub penalty_charges_amount: Decimal,
    pub penalty_charges_completed: Decimal,
    pub penalty_charges_waived: Decimal,
    pub penalty_charges_writtenoff: Decimal,
    pub total_paid_in_advance: Decimal,
    pub total_paid_late: Decimal,
    pub obligations_met: bool,
    pub obligations_met_on_date: Option<NaiveDate>,
    pub is_additional: bool,
    pub is_down_payment: bool,
    pub recalculated_interest_component: bool,
}

impl LoanRepaymentScheduleInstallment {
    /// Creates a new unpaid installment period.
    pub fn new(
        loan_id: Option<i64>,
        installment_number: i32,
        from_date: Option<NaiveDate>,
        due_date: NaiveDate,
        principal_amount: Decimal,
        interest_amount: Decimal,
        fee_charges_amount: Decimal,
        penalty_charges_amount: Decimal,
    ) -> Self {
        let mut inst = Self {
            id: None,
            loan_id,
            installment_number,
            from_date,
            due_date,
            principal_amount,
            principal_completed: Decimal::ZERO,
            principal_writtenoff: Decimal::ZERO,
            interest_amount,
            interest_completed: Decimal::ZERO,
            interest_waived: Decimal::ZERO,
            interest_writtenoff: Decimal::ZERO,
            interest_accrued: Decimal::ZERO,
            fee_charges_amount,
            fee_charges_completed: Decimal::ZERO,
            fee_charges_waived: Decimal::ZERO,
            fee_charges_writtenoff: Decimal::ZERO,
            penalty_charges_amount,
            penalty_charges_completed: Decimal::ZERO,
            penalty_charges_waived: Decimal::ZERO,
            penalty_charges_writtenoff: Decimal::ZERO,
            total_paid_in_advance: Decimal::ZERO,
            total_paid_late: Decimal::ZERO,
            obligations_met: false,
            obligations_met_on_date: None,
            is_additional: false,
            is_down_payment: false,
            recalculated_interest_component: false,
        };
        if inst.total_outstanding() == Decimal::ZERO && inst.total_due() > Decimal::ZERO {
            inst.obligations_met = true;
        }
        inst
    }

    /// Creates a down-payment installment period.
    pub fn new_down_payment(
        loan_id: Option<i64>,
        installment_number: i32,
        from_date: Option<NaiveDate>,
        due_date: NaiveDate,
        principal_amount: Decimal,
    ) -> Self {
        let mut inst = Self::new(
            loan_id,
            installment_number,
            from_date,
            due_date,
            principal_amount,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        );
        inst.is_down_payment = true;
        inst
    }

    /// Creates an additional (`N+1`) installment period after maturity.
    pub fn new_additional(
        loan_id: Option<i64>,
        installment_number: i32,
        from_date: Option<NaiveDate>,
        due_date: NaiveDate,
    ) -> Self {
        let mut inst = Self::new(
            loan_id,
            installment_number,
            from_date,
            due_date,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        );
        inst.is_additional = true;
        inst
    }

    /// Unpaid principal remaining (`principal_amount - principal_completed - principal_writtenoff`).
    #[inline]
    pub fn principal_outstanding(&self) -> Decimal {
        (self.principal_amount - self.principal_completed - self.principal_writtenoff)
            .max(Decimal::ZERO)
    }

    /// Unpaid interest remaining (`interest_amount - interest_completed - interest_waived - interest_writtenoff`).
    #[inline]
    pub fn interest_outstanding(&self) -> Decimal {
        (self.interest_amount
            - self.interest_completed
            - self.interest_waived
            - self.interest_writtenoff)
            .max(Decimal::ZERO)
    }

    /// Unpaid fee charges remaining (`fee_charges_amount - fee_charges_completed - fee_charges_waived - fee_charges_writtenoff`).
    #[inline]
    pub fn fee_charges_outstanding(&self) -> Decimal {
        (self.fee_charges_amount
            - self.fee_charges_completed
            - self.fee_charges_waived
            - self.fee_charges_writtenoff)
            .max(Decimal::ZERO)
    }

    /// Unpaid penalty charges remaining (`penalty_charges_amount - penalty_charges_completed - penalty_charges_waived - penalty_charges_writtenoff`).
    #[inline]
    pub fn penalty_charges_outstanding(&self) -> Decimal {
        (self.penalty_charges_amount
            - self.penalty_charges_completed
            - self.penalty_charges_waived
            - self.penalty_charges_writtenoff)
            .max(Decimal::ZERO)
    }

    /// Total outstanding across principal, interest, fee charges, and penalty charges.
    #[inline]
    pub fn total_outstanding(&self) -> Decimal {
        self.principal_outstanding()
            + self.interest_outstanding()
            + self.fee_charges_outstanding()
            + self.penalty_charges_outstanding()
    }

    /// Total originally charged across all four components.
    #[inline]
    pub fn total_due(&self) -> Decimal {
        self.principal_amount
            + self.interest_amount
            + self.fee_charges_amount
            + self.penalty_charges_amount
    }

    /// Total cash paid across all four components.
    #[inline]
    pub fn total_paid(&self) -> Decimal {
        self.principal_completed
            + self.interest_completed
            + self.fee_charges_completed
            + self.penalty_charges_completed
    }

    #[inline]
    pub fn is_not_fully_paid_off(&self) -> bool {
        !self.obligations_met
    }

    #[inline]
    pub fn is_overdue_on(&self, date: NaiveDate) -> bool {
        self.is_not_fully_paid_off() && date > self.due_date
    }

    #[inline]
    pub fn is_in_advance(&self, transaction_date: NaiveDate) -> bool {
        transaction_date < self.due_date
    }

    #[inline]
    pub fn is_late_payment(&self, transaction_date: NaiveDate) -> bool {
        transaction_date > self.due_date
    }

    #[inline]
    pub fn is_transaction_date_within_period(&self, reference_date: NaiveDate) -> bool {
        match self.from_date {
            Some(from) => reference_date > from && reference_date <= self.due_date,
            None => reference_date <= self.due_date,
        }
    }

    /// Updates `obligations_met` and `obligations_met_on_date` based on `total_outstanding()`.
    pub fn check_if_repayment_period_obligations_are_met(&mut self, transaction_date: NaiveDate) {
        self.obligations_met = self.total_outstanding() == Decimal::ZERO;
        if self.obligations_met {
            self.obligations_met_on_date = Some(transaction_date);
        } else {
            self.obligations_met_on_date = None;
        }
    }

    fn track_advance_and_late_totals(
        &mut self,
        transaction_date: NaiveDate,
        amount_paid: Decimal,
    ) {
        if amount_paid <= Decimal::ZERO {
            return;
        }
        if self.is_in_advance(transaction_date) {
            self.total_paid_in_advance += amount_paid;
        } else if self.is_late_payment(transaction_date) {
            self.total_paid_late += amount_paid;
        }
    }

    fn reduce_advance_and_late_totals(
        &mut self,
        transaction_date: NaiveDate,
        amount_deducted: Decimal,
    ) {
        if amount_deducted <= Decimal::ZERO {
            return;
        }
        if self.is_in_advance(transaction_date) {
            self.total_paid_in_advance =
                (self.total_paid_in_advance - amount_deducted).max(Decimal::ZERO);
        } else if self.is_late_payment(transaction_date) {
            self.total_paid_late = (self.total_paid_late - amount_deducted).max(Decimal::ZERO);
        }
    }

    /// Pays towards the principal component of this installment, returning the principal portion paid.
    ///
    /// Accepts either `(transaction_date, amount)` or `(amount, transaction_date)`.
    pub fn pay_principal<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        if transaction_amount <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let principal_due = self.principal_outstanding();
        let paid_portion = transaction_amount.min(principal_due);
        self.principal_completed += paid_portion;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        self.track_advance_and_late_totals(transaction_date, paid_portion);
        paid_portion
    }

    /// Pays towards the interest component of this installment, returning the interest portion paid.
    ///
    /// Accepts either `(transaction_date, amount)` or `(amount, transaction_date)`.
    pub fn pay_interest<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        if transaction_amount <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let interest_due = self.interest_outstanding();
        let paid_portion = transaction_amount.min(interest_due);
        self.interest_completed += paid_portion;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        self.track_advance_and_late_totals(transaction_date, paid_portion);
        paid_portion
    }

    /// Pays towards the fee charges component of this installment, returning the fee portion paid.
    ///
    /// Accepts either `(transaction_date, amount)` or `(amount, transaction_date)`.
    pub fn pay_fee_charges<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        if transaction_amount <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let fee_due = self.fee_charges_outstanding();
        let paid_portion = transaction_amount.min(fee_due);
        self.fee_charges_completed += paid_portion;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        self.track_advance_and_late_totals(transaction_date, paid_portion);
        paid_portion
    }

    /// Pays towards the penalty charges component of this installment, returning the penalty portion paid.
    ///
    /// Accepts either `(transaction_date, amount)` or `(amount, transaction_date)`.
    pub fn pay_penalty_charges<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        if transaction_amount <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let penalty_due = self.penalty_charges_outstanding();
        let paid_portion = transaction_amount.min(penalty_due);
        self.penalty_charges_completed += paid_portion;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        self.track_advance_and_late_totals(transaction_date, paid_portion);
        paid_portion
    }

    /// Waives up to `amount` of outstanding interest on this installment, returning the waived portion.
    ///
    /// Accepts either `(transaction_date, amount)` or `(amount, transaction_date)`.
    pub fn waive_interest<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        if transaction_amount <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let interest_due = self.interest_outstanding();
        let waived_portion = transaction_amount.min(interest_due);
        self.interest_waived += waived_portion;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        waived_portion
    }

    /// Waives up to `amount` of outstanding fee charges on this installment, returning the waived portion.
    pub fn waive_fee_charges<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        if transaction_amount <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let fee_due = self.fee_charges_outstanding();
        let waived_portion = transaction_amount.min(fee_due);
        self.fee_charges_waived += waived_portion;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        waived_portion
    }

    /// Waives up to `amount` of outstanding penalty charges on this installment, returning the waived portion.
    pub fn waive_penalty_charges<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        if transaction_amount <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let penalty_due = self.penalty_charges_outstanding();
        let waived_portion = transaction_amount.min(penalty_due);
        self.penalty_charges_waived += waived_portion;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        waived_portion
    }

    /// Reverses (`UNPAY`) up to `amount` of paid principal on this installment.
    pub fn unpay_principal<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        let deducted = transaction_amount
            .max(Decimal::ZERO)
            .min(self.principal_completed);
        self.principal_completed -= deducted;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        self.reduce_advance_and_late_totals(transaction_date, deducted);
        deducted
    }

    /// Reverses (`UNPAY`) up to `amount` of paid interest on this installment.
    pub fn unpay_interest<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        let deducted = transaction_amount
            .max(Decimal::ZERO)
            .min(self.interest_completed);
        self.interest_completed -= deducted;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        self.reduce_advance_and_late_totals(transaction_date, deducted);
        deducted
    }

    /// Reverses (`UNPAY`) up to `amount` of paid fee charges on this installment.
    pub fn unpay_fee_charges<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        let deducted = transaction_amount
            .max(Decimal::ZERO)
            .min(self.fee_charges_completed);
        self.fee_charges_completed -= deducted;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        self.reduce_advance_and_late_totals(transaction_date, deducted);
        deducted
    }

    /// Reverses (`UNPAY`) up to `amount` of paid penalty charges on this installment.
    pub fn unpay_penalty_charges<A, B>(&mut self, arg1: A, arg2: B) -> Decimal
    where
        (A, B): InstallmentPaymentArgs,
    {
        let (transaction_date, transaction_amount) = (arg1, arg2).into_date_and_amount();
        let deducted = transaction_amount
            .max(Decimal::ZERO)
            .min(self.penalty_charges_completed);
        self.penalty_charges_completed -= deducted;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        deducted
    }

    /// Writes off all remaining outstanding balances (principal, interest, fees, penalties) on this
    /// installment as of `transaction_date`, returning the total amount written off.
    pub fn write_off(&mut self, transaction_date: NaiveDate) -> Decimal {
        let principal_wo = self.write_off_outstanding_principal(transaction_date);
        let interest_wo = self.write_off_outstanding_interest(transaction_date);
        let fee_wo = self.write_off_outstanding_fee_charges(transaction_date);
        let penalty_wo = self.write_off_outstanding_penalty_charges(transaction_date);
        principal_wo + interest_wo + fee_wo + penalty_wo
    }

    pub fn write_off_outstanding_principal(&mut self, transaction_date: NaiveDate) -> Decimal {
        let principal_wo = self.principal_outstanding();
        self.principal_writtenoff += principal_wo;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        principal_wo
    }

    pub fn write_off_outstanding_interest(&mut self, transaction_date: NaiveDate) -> Decimal {
        let interest_wo = self.interest_outstanding();
        self.interest_writtenoff += interest_wo;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        interest_wo
    }

    pub fn write_off_outstanding_fee_charges(&mut self, transaction_date: NaiveDate) -> Decimal {
        let fee_wo = self.fee_charges_outstanding();
        self.fee_charges_writtenoff += fee_wo;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        fee_wo
    }

    pub fn write_off_outstanding_penalty_charges(
        &mut self,
        transaction_date: NaiveDate,
    ) -> Decimal {
        let penalty_wo = self.penalty_charges_outstanding();
        self.penalty_charges_writtenoff += penalty_wo;
        self.check_if_repayment_period_obligations_are_met(transaction_date);
        penalty_wo
    }

    #[inline]
    pub fn pay_principal_component(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Decimal {
        self.pay_principal(transaction_date, amount)
    }

    #[inline]
    pub fn pay_interest_component(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Decimal {
        self.pay_interest(transaction_date, amount)
    }

    #[inline]
    pub fn pay_fee_charges_component(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Decimal {
        self.pay_fee_charges(transaction_date, amount)
    }

    #[inline]
    pub fn pay_penalty_charges_component(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Decimal {
        self.pay_penalty_charges(transaction_date, amount)
    }

    #[inline]
    pub fn waive_interest_component(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Decimal {
        self.waive_interest(transaction_date, amount)
    }

    #[inline]
    pub fn waive_fee_charges_component(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Decimal {
        self.waive_fee_charges(transaction_date, amount)
    }

    #[inline]
    pub fn waive_penalty_charges_component(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Decimal {
        self.waive_penalty_charges(transaction_date, amount)
    }

    /// Resets all derived completion, waiver, write-off, advance/late, and obligation flags
    /// prior to replaying transactions against the schedule.
    ///
    /// Mirrors `LoanRepaymentScheduleInstallment.resetDerivedComponents()`.
    pub fn reset_derived_components(&mut self) {
        self.principal_completed = Decimal::ZERO;
        self.principal_writtenoff = Decimal::ZERO;
        self.interest_completed = Decimal::ZERO;
        self.interest_waived = Decimal::ZERO;
        self.interest_writtenoff = Decimal::ZERO;
        self.fee_charges_completed = Decimal::ZERO;
        self.fee_charges_waived = Decimal::ZERO;
        self.fee_charges_writtenoff = Decimal::ZERO;
        self.penalty_charges_completed = Decimal::ZERO;
        self.penalty_charges_waived = Decimal::ZERO;
        self.penalty_charges_writtenoff = Decimal::ZERO;
        self.total_paid_in_advance = Decimal::ZERO;
        self.total_paid_late = Decimal::ZERO;
        self.obligations_met = false;
        self.obligations_met_on_date = None;
    }

    /// Resets accrual-tracking fields (`interest_accrued`).
    pub fn reset_accrual_components(&mut self) {
        self.interest_accrued = Decimal::ZERO;
    }
}

impl PartialOrd for LoanRepaymentScheduleInstallment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LoanRepaymentScheduleInstallment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.installment_number
            .cmp(&other.installment_number)
            .then_with(|| self.due_date.cmp(&other.due_date))
    }
}

// =============================================================================
// 4. LoanInstallmentCharge (LoanInstallmentCharge.java)
// =============================================================================

/// Per-installment breakdown of an installment fee charge (`m_loan_installment_charge`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanInstallmentCharge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoanInstallmentCharge {
    pub id: Option<i64>,
    pub loan_charge_id: Option<i64>,
    pub loan_schedule_id: Option<i64>,
    pub installment_number: i32,
    pub due_date: Option<NaiveDate>,
    pub amount: Decimal,
    pub amount_paid: Decimal,
    pub amount_waived: Decimal,
    pub amount_writtenoff: Decimal,
    pub amount_outstanding: Decimal,
    pub amount_through_charge_payment: Decimal,
    pub is_paid: bool,
    pub waived: bool,
}

impl LoanInstallmentCharge {
    pub fn new(
        installment_number: i32,
        due_date: Option<NaiveDate>,
        amount: Decimal,
    ) -> Self {
        Self {
            id: None,
            loan_charge_id: None,
            loan_schedule_id: None,
            installment_number,
            due_date,
            amount,
            amount_paid: Decimal::ZERO,
            amount_waived: Decimal::ZERO,
            amount_writtenoff: Decimal::ZERO,
            amount_outstanding: amount,
            amount_through_charge_payment: Decimal::ZERO,
            is_paid: amount == Decimal::ZERO,
            waived: false,
        }
    }

    #[inline]
    pub fn calculate_outstanding(&self) -> Decimal {
        (self.amount - self.amount_paid - self.amount_waived - self.amount_writtenoff)
            .max(Decimal::ZERO)
    }

    #[inline]
    pub fn determine_if_fully_paid(&self) -> bool {
        self.calculate_outstanding() == Decimal::ZERO
    }

    #[inline]
    pub fn is_pending(&self) -> bool {
        self.amount_outstanding > Decimal::ZERO
    }

    #[inline]
    pub fn is_charge_amount_paid(&self) -> bool {
        self.amount_through_charge_payment == self.amount_paid
    }

    pub fn waive(&mut self) -> Decimal {
        let waived_amount = self.amount_outstanding;
        self.amount_waived += waived_amount;
        self.amount_outstanding = Decimal::ZERO;
        self.is_paid = false;
        self.waived = true;
        waived_amount
    }

    pub fn undo_waive(&mut self) {
        self.amount_outstanding += self.amount_waived;
        self.amount_waived = Decimal::ZERO;
        self.is_paid = false;
        self.waived = false;
    }

    pub fn update_paid_amount_by(
        &mut self,
        increment_by: Decimal,
        fee_amount: Decimal,
    ) -> Decimal {
        let amount_paid_previously = self.amount_paid;
        let paid_on_this_charge = if increment_by >= self.amount_outstanding {
            let paid = self.amount_outstanding;
            self.amount_paid += paid;
            self.amount_outstanding = Decimal::ZERO;
            paid
        } else {
            self.amount_paid += increment_by;
            self.amount_outstanding = self.calculate_outstanding();
            increment_by
        };

        if amount_paid_previously > Decimal::ZERO {
            self.amount_through_charge_payment += fee_amount;
        } else {
            self.amount_through_charge_payment = fee_amount;
        }

        if self.determine_if_fully_paid() {
            if self.amount_waived > Decimal::ZERO {
                self.waived = true;
            } else {
                self.is_paid = true;
            }
        }
        paid_on_this_charge
    }

    pub fn undo_paid_amount_by(
        &mut self,
        increment_by: Decimal,
        fee_amount: Decimal,
    ) -> Decimal {
        let deducted = if increment_by >= self.amount_paid {
            let d = self.amount_paid;
            self.amount_paid = Decimal::ZERO;
            self.amount_outstanding = self.amount;
            d
        } else {
            self.amount_paid -= increment_by;
            self.amount_outstanding = self.calculate_outstanding();
            increment_by
        };
        self.amount_through_charge_payment = fee_amount;
        self.is_paid = self.determine_if_fully_paid();
        deducted
    }

    pub fn write_off(&mut self) -> Decimal {
        let written_off = self.amount_outstanding;
        self.amount_writtenoff += written_off;
        self.amount_outstanding = Decimal::ZERO;
        written_off
    }

    pub fn reset_paid_amount(&mut self) {
        self.amount_paid = Decimal::ZERO;
        self.amount_outstanding = self.calculate_outstanding();
        self.is_paid = false;
    }

    pub fn reset_to_original(&mut self) {
        self.amount_paid = Decimal::ZERO;
        self.amount_waived = Decimal::ZERO;
        self.amount_writtenoff = Decimal::ZERO;
        self.amount_through_charge_payment = Decimal::ZERO;
        self.amount_outstanding = self.amount;
        self.is_paid = false;
        self.waived = false;
    }
}

impl PartialOrd for LoanInstallmentCharge {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LoanInstallmentCharge {
    fn cmp(&self, other: &Self) -> Ordering {
        self.installment_number.cmp(&other.installment_number)
    }
}

// =============================================================================
// 5. LoanCharge (LoanCharge.java)
// =============================================================================

/// Fee or penalty charge attached to a loan account (`m_loan_charge`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanCharge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoanCharge {
    pub id: Option<i64>,
    pub loan_id: Option<i64>,
    pub charge_id: i64,
    pub name: Option<String>,
    pub charge_time: ChargeTimeType,
    pub submitted_on_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub charge_calculation: ChargeCalculationType,
    pub charge_payment_mode: ChargePaymentMode,
    pub percentage: Option<Decimal>,
    pub amount_percentage_applied_to: Option<Decimal>,
    pub amount_or_percentage: Decimal,
    pub amount: Decimal,
    pub amount_paid: Decimal,
    pub amount_waived: Decimal,
    pub amount_writtenoff: Decimal,
    pub amount_outstanding: Decimal,
    pub tax_amount: Decimal,
    pub is_penalty: bool,
    pub is_paid: bool,
    pub waived: bool,
    pub min_cap: Option<Decimal>,
    pub max_cap: Option<Decimal>,
    pub is_active: bool,
    pub external_id: ExternalId,
    pub installment_charges: Vec<LoanInstallmentCharge>,
}

impl LoanCharge {
    /// Creates a new `LoanCharge`, computing the initial charge amount from `charge_calculation`
    /// and applying `min_cap` / `max_cap` bounds when present.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        loan_id: Option<i64>,
        charge_id: i64,
        charge_time: ChargeTimeType,
        due_date: Option<NaiveDate>,
        charge_calculation: ChargeCalculationType,
        charge_payment_mode: ChargePaymentMode,
        amount_or_percentage: Decimal,
        base_amount: Decimal,
        is_penalty: bool,
        min_cap: Option<Decimal>,
        max_cap: Option<Decimal>,
    ) -> Self {
        let (percentage, amount_percentage_applied_to, raw_amount) =
            if charge_calculation.is_percentage_based() {
                let pct = amount_or_percentage;
                let calc = Self::percentage_of(base_amount, pct);
                (Some(pct), Some(base_amount), calc)
            } else {
                (None, None, amount_or_percentage)
            };

        let mut charge = Self {
            id: None,
            loan_id,
            charge_id,
            name: None,
            charge_time,
            submitted_on_date: due_date,
            due_date,
            charge_calculation,
            charge_payment_mode,
            percentage,
            amount_percentage_applied_to,
            amount_or_percentage,
            amount: raw_amount,
            amount_paid: Decimal::ZERO,
            amount_waived: Decimal::ZERO,
            amount_writtenoff: Decimal::ZERO,
            amount_outstanding: raw_amount,
            tax_amount: Decimal::ZERO,
            is_penalty,
            is_paid: false,
            waived: false,
            min_cap,
            max_cap,
            is_active: true,
            external_id: ExternalId::empty(),
            installment_charges: Vec::new(),
        };
        let capped = charge.minimum_and_maximum_cap(raw_amount);
        charge.amount = capped;
        charge.amount_outstanding = capped;
        charge
    }

    /// Computes `value * percentage / 100` if `value > 0`, else `0`.
    pub fn percentage_of(value: Decimal, percentage: Decimal) -> Decimal {
        if value > Decimal::ZERO {
            (value * percentage) / Decimal::ONE_HUNDRED
        } else {
            Decimal::ZERO
        }
    }

    /// Clamps `percentage_of` between `min_cap` and `max_cap` if configured.
    pub fn minimum_and_maximum_cap(&self, raw_amount: Decimal) -> Decimal {
        if let Some(min_cap) = self.min_cap {
            if raw_amount < min_cap {
                return min_cap;
            }
        }
        if let Some(max_cap) = self.max_cap {
            if raw_amount > max_cap {
                return max_cap;
            }
        }
        raw_amount
    }

    #[inline]
    pub fn calculate_outstanding(&self) -> Decimal {
        (self.amount - self.amount_paid - self.amount_waived - self.amount_writtenoff)
            .max(Decimal::ZERO)
    }

    #[inline]
    pub fn determine_if_fully_paid(&self) -> bool {
        self.calculate_outstanding() == Decimal::ZERO
    }

    #[inline]
    pub fn is_fully_paid(&self) -> bool {
        self.is_paid
    }

    #[inline]
    pub fn is_fee_charge(&self) -> bool {
        !self.is_penalty
    }

    #[inline]
    pub fn is_penalty_charge(&self) -> bool {
        self.is_penalty
    }

    #[inline]
    pub fn is_due_at_disbursement(&self) -> bool {
        self.charge_time.is_on_disbursement() || self.charge_time.is_tranche_disbursement()
    }

    #[inline]
    pub fn is_disbursement_charge(&self) -> bool {
        self.charge_time.is_on_disbursement()
    }

    #[inline]
    pub fn is_tranche_disbursement_charge(&self) -> bool {
        self.charge_time.is_tranche_disbursement()
    }

    #[inline]
    pub fn is_specified_due_date(&self) -> bool {
        self.charge_time.is_on_specified_due_date()
    }

    #[inline]
    pub fn is_instalment_fee(&self) -> bool {
        self.charge_time.is_installment_fee()
    }

    #[inline]
    pub fn is_installment_fee(&self) -> bool {
        self.charge_time.is_installment_fee()
    }

    #[inline]
    pub fn is_overdue_installment_charge(&self) -> bool {
        self.charge_time.is_overdue_installment()
    }

    #[inline]
    pub fn is_charge_pending(&self) -> bool {
        !self.is_paid && !self.waived && self.amount_outstanding > Decimal::ZERO
    }

    #[inline]
    pub fn is_paid_or_partially_paid(&self) -> bool {
        (self.amount_paid + self.amount_waived + self.amount_writtenoff) > Decimal::ZERO
    }

    /// Checks whether this charge's `due_date` falls within `(from_date, to_date]`
    /// (or `[from_date, to_date]` when `is_first_period` is `true`).
    pub fn is_due_in_period(
        &self,
        from_date: Option<NaiveDate>,
        to_date: NaiveDate,
        is_first_period: bool,
    ) -> bool {
        let Some(due) = self.due_date else {
            return false;
        };
        match from_date {
            Some(from) if is_first_period => due >= from && due <= to_date,
            Some(from) => due > from && due <= to_date,
            None => due <= to_date,
        }
    }

    pub fn mark_as_fully_paid(&mut self) {
        self.amount_paid = self.amount;
        self.amount_outstanding = Decimal::ZERO;
        self.is_paid = true;
    }

    pub fn reconcile_fully_paid(&mut self) {
        self.amount_paid =
            (self.amount - self.amount_waived - self.amount_writtenoff).max(Decimal::ZERO);
        self.amount_outstanding = Decimal::ZERO;
        if self.amount_waived > Decimal::ZERO {
            self.waived = true;
        } else {
            self.is_paid = true;
        }
    }

    pub fn reset_to_original(&mut self) {
        self.amount_paid = Decimal::ZERO;
        self.amount_waived = Decimal::ZERO;
        self.amount_writtenoff = Decimal::ZERO;
        self.amount_outstanding = self.amount;
        self.is_paid = false;
        self.waived = false;
        for ic in &mut self.installment_charges {
            ic.reset_to_original();
        }
    }

    pub fn reset_paid_amount(&mut self) {
        self.amount_paid = Decimal::ZERO;
        self.amount_outstanding = self.calculate_outstanding();
        self.is_paid = false;
        for ic in &mut self.installment_charges {
            ic.reset_paid_amount();
        }
    }

    /// Waives this loan charge (or a specific installment charge if `is_instalment_fee()`),
    /// returning the waived amount.
    pub fn waive(&mut self, installment_number: Option<i32>) -> Decimal {
        if self.is_instalment_fee() {
            if let Some(inst_num) = installment_number {
                if let Some(ic) = self
                    .installment_charges
                    .iter_mut()
                    .find(|c| c.installment_number == inst_num)
                {
                    let waived_amt = ic.waive();
                    self.amount_waived += waived_amt;
                    self.amount_outstanding =
                        (self.amount_outstanding - waived_amt).max(Decimal::ZERO);
                    if self.determine_if_fully_paid() {
                        self.is_paid = false;
                        self.waived = true;
                    }
                    return waived_amt;
                }
            }
        }
        let waived_amt = self.amount_outstanding;
        self.amount_waived += waived_amt;
        self.amount_outstanding = Decimal::ZERO;
        self.is_paid = false;
        self.waived = true;
        waived_amt
    }

    pub fn undo_waive(&mut self, installment_number: Option<i32>) {
        if self.is_instalment_fee() {
            if let Some(inst_num) = installment_number {
                if let Some(ic) = self
                    .installment_charges
                    .iter_mut()
                    .find(|c| c.installment_number == inst_num)
                {
                    ic.undo_waive();
                    let reversed = ic.amount_outstanding;
                    self.amount_waived = (self.amount_waived - reversed).max(Decimal::ZERO);
                    self.amount_outstanding += reversed;
                    if !self.determine_if_fully_paid() {
                        self.is_paid = false;
                        self.waived = false;
                    }
                    return;
                }
            }
        }
        self.amount_outstanding += self.amount_waived;
        self.amount_waived = Decimal::ZERO;
        self.is_paid = false;
        self.waived = false;
    }

    /// Pays up to `increment_by` towards this loan charge, returning the amount applied.
    pub fn update_paid_amount_by(
        &mut self,
        increment_by: Decimal,
        installment_number: Option<i32>,
        fee_amount: Decimal,
    ) -> Decimal {
        if increment_by <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let process_amount = if self.is_instalment_fee() && !self.installment_charges.is_empty() {
            let target = match installment_number {
                Some(inst_num) => self
                    .installment_charges
                    .iter_mut()
                    .find(|ic| ic.installment_number == inst_num),
                None => self
                    .installment_charges
                    .iter_mut()
                    .filter(|ic| ic.is_pending())
                    .min_by_key(|ic| (ic.due_date, ic.installment_number)),
            };
            match target {
                Some(ic) => ic.update_paid_amount_by(increment_by, fee_amount),
                None => Decimal::ZERO,
            }
        } else {
            increment_by
        };

        let paid_on_this_charge = if process_amount >= self.amount_outstanding {
            let paid = self.amount_outstanding;
            self.amount_paid += paid;
            self.amount_outstanding = Decimal::ZERO;
            if self.amount_waived > Decimal::ZERO {
                self.waived = true;
            } else {
                self.is_paid = true;
            }
            paid
        } else {
            self.amount_paid += process_amount;
            self.amount_outstanding = self.calculate_outstanding();
            process_amount
        };

        paid_on_this_charge
    }

    /// Convenience alias for paying towards a `LoanCharge`.
    pub fn pay(&mut self, amount: Decimal, installment_number: Option<i32>) -> Decimal {
        self.update_paid_amount_by(amount, installment_number, amount)
    }

    /// Writes off any remaining outstanding balance on this charge.
    pub fn write_off(&mut self) -> Decimal {
        let written_off = self.amount_outstanding;
        self.amount_writtenoff += written_off;
        self.amount_outstanding = Decimal::ZERO;
        for ic in &mut self.installment_charges {
            ic.write_off();
        }
        written_off
    }

    pub fn effective_due_date(&self) -> Option<NaiveDate> {
        if self.is_instalment_fee() {
            self.installment_charges
                .iter()
                .filter(|ic| ic.is_pending())
                .min_by_key(|ic| (ic.due_date, ic.installment_number))
                .and_then(|ic| ic.due_date)
        } else {
            self.due_date
        }
    }
}

// =============================================================================
// 6. LoanTermVariations (LoanTermVariations.java)
// =============================================================================

/// Scheduled or ad-hoc variation of loan terms (`m_loan_term_variations`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanTermVariations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoanTermVariations {
    pub id: Option<i64>,
    pub loan_id: Option<i64>,
    pub term_type: LoanTermVariationType,
    pub term_applicable_from: NaiveDate,
    pub decimal_value: Option<Decimal>,
    pub date_value: Option<NaiveDate>,
    pub is_specific_to_installment: bool,
    pub applied_on_loan_status: i32,
    pub is_active: bool,
    pub parent_id: Option<i64>,
}

impl LoanTermVariations {
    pub fn new(
        loan_id: Option<i64>,
        term_type: LoanTermVariationType,
        term_applicable_from: NaiveDate,
        decimal_value: Option<Decimal>,
        date_value: Option<NaiveDate>,
        is_specific_to_installment: bool,
        applied_on_loan_status: i32,
    ) -> Self {
        Self {
            id: None,
            loan_id,
            term_type,
            term_applicable_from,
            decimal_value,
            date_value,
            is_specific_to_installment,
            applied_on_loan_status,
            is_active: true,
            parent_id: None,
        }
    }

    #[inline]
    pub const fn get_term_type(&self) -> LoanTermVariationType {
        self.term_type
    }

    #[inline]
    pub const fn get_term_applicable_from(&self) -> NaiveDate {
        self.term_applicable_from
    }

    #[inline]
    pub const fn fetch_term_applica_date(&self) -> NaiveDate {
        self.term_applicable_from
    }

    #[inline]
    pub const fn get_term_value(&self) -> Option<Decimal> {
        self.decimal_value
    }

    #[inline]
    pub const fn get_date_value(&self) -> Option<NaiveDate> {
        self.date_value
    }

    #[inline]
    pub const fn fetch_date_value(&self) -> Option<NaiveDate> {
        self.date_value
    }

    #[inline]
    pub const fn get_on_loan_status(&self) -> i32 {
        self.applied_on_loan_status
    }

    #[inline]
    pub const fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn update_is_active(&mut self, is_active: bool) {
        self.is_active = is_active;
    }

    pub fn mark_as_inactive(&mut self) {
        self.is_active = false;
    }
}

// =============================================================================
// 7. LoanTransactionToRepaymentScheduleMapping (LoanTransactionToRepaymentScheduleMapping.java)
// =============================================================================

/// Allocation mapping between a [`LoanTransaction`] and a [`LoanRepaymentScheduleInstallment`]
/// (`m_loan_transaction_repayment_schedule_mapping`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanTransactionToRepaymentScheduleMapping`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoanTransactionToRepaymentScheduleMapping {
    pub id: Option<i64>,
    pub loan_transaction_id: Option<i64>,
    pub loan_repayment_schedule_id: Option<i64>,
    pub installment_number: Option<i32>,
    pub due_date: Option<NaiveDate>,
    pub principal_portion: Decimal,
    pub interest_portion: Decimal,
    pub fee_charges_portion: Decimal,
    pub penalty_charges_portion: Decimal,
    pub amount: Decimal,
}

impl LoanTransactionToRepaymentScheduleMapping {
    pub fn new(
        loan_transaction_id: Option<i64>,
        loan_repayment_schedule_id: Option<i64>,
        installment_number: Option<i32>,
        due_date: Option<NaiveDate>,
        principal_portion: Decimal,
        interest_portion: Decimal,
        fee_charges_portion: Decimal,
        penalty_charges_portion: Decimal,
    ) -> Self {
        let amount =
            principal_portion + interest_portion + fee_charges_portion + penalty_charges_portion;
        Self {
            id: None,
            loan_transaction_id,
            loan_repayment_schedule_id,
            installment_number,
            due_date,
            principal_portion,
            interest_portion,
            fee_charges_portion,
            penalty_charges_portion,
            amount,
        }
    }

    pub fn create_from(
        installment: &LoanRepaymentScheduleInstallment,
        principal_portion: Decimal,
        interest_portion: Decimal,
        fee_charges_portion: Decimal,
        penalty_charges_portion: Decimal,
    ) -> Self {
        Self::new(
            None,
            installment.id,
            Some(installment.installment_number),
            Some(installment.due_date),
            principal_portion,
            interest_portion,
            fee_charges_portion,
            penalty_charges_portion,
        )
    }

    pub fn set_components(
        &mut self,
        principal: Decimal,
        interest: Decimal,
        fee_charges: Decimal,
        penalty_charges: Decimal,
    ) {
        self.principal_portion = principal;
        self.interest_portion = interest;
        self.fee_charges_portion = fee_charges;
        self.penalty_charges_portion = penalty_charges;
        self.amount = principal + interest + fee_charges + penalty_charges;
    }

    pub fn update_components(
        &mut self,
        principal: Decimal,
        interest: Decimal,
        fee_charges: Decimal,
        penalty_charges: Decimal,
    ) {
        self.principal_portion += principal;
        self.interest_portion += interest;
        self.fee_charges_portion += fee_charges;
        self.penalty_charges_portion += penalty_charges;
        self.amount = self.principal_portion
            + self.interest_portion
            + self.fee_charges_portion
            + self.penalty_charges_portion;
    }

    #[inline]
    pub fn is_zero_amount(&self) -> bool {
        self.amount == Decimal::ZERO
    }
}

// =============================================================================
// 8. LoanTransaction (LoanTransaction.java)
// =============================================================================

/// Monetary or non-monetary transaction recorded against a loan account (`m_loan_transaction`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanTransaction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoanTransaction {
    pub id: Option<i64>,
    pub loan_id: Option<i64>,
    pub office_id: Option<i64>,
    pub payment_detail_id: Option<i64>,
    pub transaction_type: LoanTransactionType,
    pub transaction_date: NaiveDate,
    pub submitted_on_date: NaiveDate,
    pub amount: Decimal,
    pub principal_portion: Decimal,
    pub interest_portion: Decimal,
    pub fee_charges_portion: Decimal,
    pub penalty_charges_portion: Decimal,
    pub overpayment_portion: Decimal,
    pub unrecognized_income_portion: Decimal,
    pub outstanding_loan_balance: Decimal,
    pub is_reversed: bool,
    pub reversed_on_date: Option<NaiveDate>,
    pub manually_adjusted_or_reversed: bool,
    pub external_id: ExternalId,
    pub reversal_external_id: ExternalId,
    pub charge_refund_charge_type: Option<String>,
    pub repayment_schedule_mappings: Vec<LoanTransactionToRepaymentScheduleMapping>,
}

impl LoanTransaction {
    pub fn new(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        transaction_type: LoanTransactionType,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Self {
        Self {
            id: None,
            loan_id,
            office_id,
            payment_detail_id: None,
            transaction_type,
            transaction_date,
            submitted_on_date: transaction_date,
            amount,
            principal_portion: Decimal::ZERO,
            interest_portion: Decimal::ZERO,
            fee_charges_portion: Decimal::ZERO,
            penalty_charges_portion: Decimal::ZERO,
            overpayment_portion: Decimal::ZERO,
            unrecognized_income_portion: Decimal::ZERO,
            outstanding_loan_balance: Decimal::ZERO,
            is_reversed: false,
            reversed_on_date: None,
            manually_adjusted_or_reversed: false,
            external_id: ExternalId::empty(),
            reversal_external_id: ExternalId::empty(),
            charge_refund_charge_type: None,
            repayment_schedule_mappings: Vec::new(),
        }
    }

    pub fn disbursement(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        disbursement_date: NaiveDate,
        amount: Decimal,
        external_id: ExternalId,
    ) -> Self {
        let mut txn = Self::new(
            loan_id,
            office_id,
            LoanTransactionType::Disbursement,
            disbursement_date,
            amount,
        );
        txn.external_id = external_id;
        txn
    }

    pub fn repayment(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        payment_date: NaiveDate,
        amount: Decimal,
        external_id: ExternalId,
    ) -> Self {
        let mut txn = Self::new(
            loan_id,
            office_id,
            LoanTransactionType::Repayment,
            payment_date,
            amount,
        );
        txn.external_id = external_id;
        txn
    }

    pub fn down_payment(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        payment_date: NaiveDate,
        amount: Decimal,
        external_id: ExternalId,
    ) -> Self {
        let mut txn = Self::new(
            loan_id,
            office_id,
            LoanTransactionType::DownPayment,
            payment_date,
            amount,
        );
        txn.external_id = external_id;
        txn
    }

    pub fn chargeback(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        payment_date: NaiveDate,
        amount: Decimal,
        external_id: ExternalId,
    ) -> Self {
        let mut txn = Self::new(
            loan_id,
            office_id,
            LoanTransactionType::Chargeback,
            payment_date,
            amount,
        );
        txn.principal_portion = amount;
        txn.external_id = external_id;
        txn
    }

    pub fn waiver(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        waive_date: NaiveDate,
        amount: Decimal,
        interest_waived: Decimal,
        unrecognized_portion: Decimal,
        external_id: ExternalId,
    ) -> Self {
        let mut txn = Self::new(
            loan_id,
            office_id,
            LoanTransactionType::WaiveInterest,
            waive_date,
            amount,
        );
        txn.interest_portion = interest_waived;
        txn.unrecognized_income_portion = unrecognized_portion;
        txn.external_id = external_id;
        txn
    }

    pub fn waive_loan_charge(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        waive_date: NaiveDate,
        waived_amount: Decimal,
        fee_charges_waived: Decimal,
        penalty_charges_waived: Decimal,
        external_id: ExternalId,
    ) -> Self {
        let mut txn = Self::new(
            loan_id,
            office_id,
            LoanTransactionType::WaiveCharges,
            waive_date,
            waived_amount,
        );
        txn.fee_charges_portion = fee_charges_waived;
        txn.penalty_charges_portion = penalty_charges_waived;
        txn.external_id = external_id;
        txn
    }

    pub fn accrual(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        date_of: NaiveDate,
        amount: Decimal,
        interest_portion: Decimal,
        fee_charges_portion: Decimal,
        penalty_charges_portion: Decimal,
        external_id: ExternalId,
    ) -> Self {
        let mut txn = Self::new(
            loan_id,
            office_id,
            LoanTransactionType::Accrual,
            date_of,
            amount,
        );
        txn.interest_portion = interest_portion;
        txn.fee_charges_portion = fee_charges_portion;
        txn.penalty_charges_portion = penalty_charges_portion;
        txn.external_id = external_id;
        txn
    }

    pub fn writeoff(
        loan_id: Option<i64>,
        office_id: Option<i64>,
        write_off_date: NaiveDate,
        external_id: ExternalId,
    ) -> Self {
        let mut txn = Self::new(
            loan_id,
            office_id,
            LoanTransactionType::WriteOff,
            write_off_date,
            Decimal::ZERO,
        );
        txn.external_id = external_id;
        txn
    }

    /// Marks this transaction as reversed and clears its schedule mappings.
    pub fn reverse(&mut self, reversed_on_date: NaiveDate) {
        self.is_reversed = true;
        self.reversed_on_date = Some(reversed_on_date);
        self.repayment_schedule_mappings.clear();
    }

    pub fn reverse_with_external_id(
        &mut self,
        reversed_on_date: NaiveDate,
        reversal_external_id: ExternalId,
    ) {
        self.reverse(reversed_on_date);
        self.reversal_external_id = reversal_external_id;
    }

    /// Resets derived allocation portions and running loan balance prior to schedule replay.
    ///
    /// Mirrors `LoanTransaction.resetDerivedComponents()`.
    pub fn reset_derived_components(&mut self) {
        self.principal_portion = Decimal::ZERO;
        self.interest_portion = Decimal::ZERO;
        self.fee_charges_portion = Decimal::ZERO;
        self.penalty_charges_portion = Decimal::ZERO;
        self.overpayment_portion = Decimal::ZERO;
        self.outstanding_loan_balance = Decimal::ZERO;
        self.repayment_schedule_mappings.clear();
    }

    /// Accumulates `principal`, `interest`, `fee_charges`, and `penalty_charges` into the
    /// derived portions of this transaction.
    pub fn update_components(
        &mut self,
        principal: Decimal,
        interest: Decimal,
        fee_charges: Decimal,
        penalty_charges: Decimal,
    ) {
        self.principal_portion += principal;
        self.interest_portion += interest;
        self.fee_charges_portion += fee_charges;
        self.penalty_charges_portion += penalty_charges;
    }

    /// Accumulates the four components and updates `self.amount` to their sum.
    pub fn update_components_and_total(
        &mut self,
        principal: Decimal,
        interest: Decimal,
        fee_charges: Decimal,
        penalty_charges: Decimal,
    ) {
        self.update_components(principal, interest, fee_charges, penalty_charges);
        self.amount = self.principal_portion
            + self.interest_portion
            + self.fee_charges_portion
            + self.penalty_charges_portion;
    }

    pub fn update_charges_components(&mut self, fee_charges: Decimal, penalty_charges: Decimal) {
        self.fee_charges_portion += fee_charges;
        self.penalty_charges_portion += penalty_charges;
    }

    pub fn set_over_payments(&mut self, over_payment: Decimal) {
        self.overpayment_portion = over_payment.max(Decimal::ZERO);
    }

    pub fn update_outstanding_loan_balance(&mut self, balance: Decimal) {
        self.outstanding_loan_balance = balance;
    }

    #[inline]
    pub const fn type_of(&self) -> LoanTransactionType {
        self.transaction_type
    }

    #[inline]
    pub const fn transaction_date(&self) -> NaiveDate {
        self.transaction_date
    }

    #[inline]
    pub const fn is_reversed(&self) -> bool {
        self.is_reversed
    }

    #[inline]
    pub const fn is_not_reversed(&self) -> bool {
        !self.is_reversed
    }

    #[inline]
    pub const fn is_disbursement(&self) -> bool {
        self.transaction_type.is_disbursement() && !self.is_reversed
    }

    #[inline]
    pub const fn is_repayment(&self) -> bool {
        self.transaction_type.is_repayment() && !self.is_reversed
    }

    #[inline]
    pub const fn is_repayment_at_disbursement(&self) -> bool {
        self.transaction_type.is_repayment_at_disbursement() && !self.is_reversed
    }

    #[inline]
    pub const fn is_down_payment(&self) -> bool {
        self.transaction_type.is_down_payment() && !self.is_reversed
    }

    #[inline]
    pub const fn is_merchant_issued_refund(&self) -> bool {
        self.transaction_type.is_merchant_issued_refund() && !self.is_reversed
    }

    #[inline]
    pub const fn is_payout_refund(&self) -> bool {
        self.transaction_type.is_payout_refund() && !self.is_reversed
    }

    #[inline]
    pub const fn is_goodwill_credit(&self) -> bool {
        self.transaction_type.is_goodwill_credit() && !self.is_reversed
    }

    #[inline]
    pub const fn is_interest_payment_waiver(&self) -> bool {
        self.transaction_type.is_interest_payment_waiver() && !self.is_reversed
    }

    #[inline]
    pub const fn is_charge_refund(&self) -> bool {
        self.transaction_type.is_charge_refund() && !self.is_reversed
    }

    #[inline]
    pub const fn is_charge_adjustment(&self) -> bool {
        self.transaction_type.is_charge_adjustment() && !self.is_reversed
    }

    #[inline]
    pub const fn is_repayment_like_type(&self) -> bool {
        self.transaction_type.is_repayment_type() && !self.is_reversed
    }

    #[inline]
    pub const fn is_type_allowed_for_chargeback(&self) -> bool {
        !self.is_reversed
            && (self.transaction_type.is_repayment()
                || self.transaction_type.is_merchant_issued_refund()
                || self.transaction_type.is_payout_refund()
                || self.transaction_type.is_goodwill_credit()
                || self.transaction_type.is_down_payment()
                || self.transaction_type.is_interest_payment_waiver())
    }

    #[inline]
    pub const fn is_recovery_repayment(&self) -> bool {
        self.transaction_type.is_recovery_repayment() && !self.is_reversed
    }

    #[inline]
    pub const fn is_interest_waiver(&self) -> bool {
        self.transaction_type.is_waive_interest() && !self.is_reversed
    }

    #[inline]
    pub const fn is_charges_waiver(&self) -> bool {
        self.transaction_type.is_waive_charges() && !self.is_reversed
    }

    #[inline]
    pub const fn is_waiver(&self) -> bool {
        self.is_interest_waiver() || self.is_charges_waiver()
    }

    #[inline]
    pub const fn is_write_off(&self) -> bool {
        self.transaction_type.is_write_off() && !self.is_reversed
    }

    #[inline]
    pub const fn is_charge_off(&self) -> bool {
        self.transaction_type.is_charge_off() && !self.is_reversed
    }

    #[inline]
    pub const fn is_charge_payment(&self) -> bool {
        self.transaction_type.is_charge_payment() && !self.is_reversed
    }

    #[inline]
    pub const fn is_chargeback(&self) -> bool {
        self.transaction_type.is_chargeback() && !self.is_reversed
    }

    #[inline]
    pub const fn is_editable(&self) -> bool {
        !self.is_chargeback()
    }

    #[inline]
    pub const fn is_accrual(&self) -> bool {
        self.transaction_type.is_accrual()
    }

    #[inline]
    pub const fn is_accrual_activity(&self) -> bool {
        self.transaction_type.is_accrual_activity()
    }

    #[inline]
    pub const fn is_accrual_related(&self) -> bool {
        self.is_accrual() || self.is_accrual_activity()
    }

    #[inline]
    pub const fn is_income_posting(&self) -> bool {
        self.transaction_type.is_income_posting() && !self.is_reversed
    }

    #[inline]
    pub const fn is_credit_balance_refund(&self) -> bool {
        self.transaction_type.is_credit_balance_refund() && !self.is_reversed
    }

    #[inline]
    pub const fn is_refund_for_active_loan(&self) -> bool {
        self.transaction_type.is_refund_for_active_loan() && !self.is_reversed
    }

    #[inline]
    pub const fn is_refund(&self) -> bool {
        self.transaction_type.is_refund() && !self.is_reversed
    }

    #[inline]
    pub const fn is_re_age(&self) -> bool {
        self.transaction_type.is_re_age() && !self.is_reversed
    }

    #[inline]
    pub const fn is_re_amortize(&self) -> bool {
        self.transaction_type.is_re_amortize() && !self.is_reversed
    }

    #[inline]
    pub const fn is_capitalized_income(&self) -> bool {
        self.transaction_type.is_capitalized_income() && !self.is_reversed
    }

    #[inline]
    pub const fn is_buy_down_fee(&self) -> bool {
        self.transaction_type.is_buy_down_fee() && !self.is_reversed
    }

    #[inline]
    pub const fn is_non_monetary_transaction(&self) -> bool {
        !self.is_reversed
            && matches!(
                self.transaction_type,
                LoanTransactionType::Contra
                    | LoanTransactionType::MarkedForRescheduling
                    | LoanTransactionType::Accrual
                    | LoanTransactionType::AccrualActivity
                    | LoanTransactionType::InitiateTransfer
                    | LoanTransactionType::ApproveTransfer
                    | LoanTransactionType::WithdrawTransfer
                    | LoanTransactionType::RejectTransfer
                    | LoanTransactionType::ChargeOff
                    | LoanTransactionType::ReAge
                    | LoanTransactionType::ReAmortize
                    | LoanTransactionType::BuyDownFee
            )
    }

    #[inline]
    pub const fn is_payment_transaction(&self) -> bool {
        !self.is_reversed
            && !(self.is_disbursement()
                || self.is_repayment_at_disbursement()
                || self.is_non_monetary_transaction()
                || self.is_income_posting())
    }

    #[inline]
    pub fn is_over_paid(&self) -> bool {
        self.overpayment_portion > Decimal::ZERO
    }

    #[inline]
    pub fn is_greater_than_zero(&self) -> bool {
        self.amount > Decimal::ZERO
    }

    #[inline]
    pub fn is_before(&self, date: NaiveDate) -> bool {
        self.transaction_date < date
    }

    #[inline]
    pub fn is_after(&self, date: NaiveDate) -> bool {
        self.transaction_date > date
    }

    #[inline]
    pub fn is_on(&self, date: NaiveDate) -> bool {
        self.transaction_date == date
    }

    /// Mirrors `LoanTransaction.transactionAmountsMatch(...)`.
    pub fn transaction_amounts_match(&self, other: &Self) -> bool {
        self.amount == other.amount
            && self.principal_portion == other.principal_portion
            && self.interest_portion == other.interest_portion
            && self.fee_charges_portion == other.fee_charges_portion
            && self.penalty_charges_portion == other.penalty_charges_portion
            && self.overpayment_portion == other.overpayment_portion
    }
}

// =============================================================================
// 9. LoanSummary (LoanSummary.java)
// =============================================================================

/// Derived monetary totals across all installments and charges of a loan (`LoanSummary.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoanSummary {
    pub total_principal_disbursed: Decimal,
    pub total_principal_repaid: Decimal,
    pub total_principal_writtenoff: Decimal,
    pub total_principal_outstanding: Decimal,
    pub total_interest_charged: Decimal,
    pub total_interest_repaid: Decimal,
    pub total_interest_waived: Decimal,
    pub total_interest_writtenoff: Decimal,
    pub total_interest_outstanding: Decimal,
    pub total_fee_charges_charged: Decimal,
    pub total_fee_charges_repaid: Decimal,
    pub total_fee_charges_outstanding: Decimal,
    pub total_penalty_charges_charged: Decimal,
    pub total_penalty_charges_repaid: Decimal,
    pub total_penalty_charges_outstanding: Decimal,
    pub total_expected_repayment: Decimal,
    pub total_repayment: Decimal,
    pub total_outstanding: Decimal,
    pub total_overpaid: Decimal,
}

impl LoanSummary {
    pub const fn new() -> Self {
        Self {
            total_principal_disbursed: Decimal::ZERO,
            total_principal_repaid: Decimal::ZERO,
            total_principal_writtenoff: Decimal::ZERO,
            total_principal_outstanding: Decimal::ZERO,
            total_interest_charged: Decimal::ZERO,
            total_interest_repaid: Decimal::ZERO,
            total_interest_waived: Decimal::ZERO,
            total_interest_writtenoff: Decimal::ZERO,
            total_interest_outstanding: Decimal::ZERO,
            total_fee_charges_charged: Decimal::ZERO,
            total_fee_charges_repaid: Decimal::ZERO,
            total_fee_charges_outstanding: Decimal::ZERO,
            total_penalty_charges_charged: Decimal::ZERO,
            total_penalty_charges_repaid: Decimal::ZERO,
            total_penalty_charges_outstanding: Decimal::ZERO,
            total_expected_repayment: Decimal::ZERO,
            total_repayment: Decimal::ZERO,
            total_outstanding: Decimal::ZERO,
            total_overpaid: Decimal::ZERO,
        }
    }

    /// Resets all summary fields to `Decimal::ZERO`.
    pub fn zero_fields(&mut self) {
        *self = Self::new();
    }

    /// Builds a `LoanSummary` from `principal_disbursed` and `installments`.
    pub fn from_installments(
        principal_disbursed: Decimal,
        installments: &[LoanRepaymentScheduleInstallment],
    ) -> Self {
        let mut summary = Self::new();
        summary.update_summary(principal_disbursed, installments, &[], Decimal::ZERO);
        summary
    }

    /// Recomputes all derived loan summary fields from `principal_disbursed`, `installments`,
    /// `charges`, and `total_overpaid`, mirroring `LoanSummary.updateSummary(...)` in Java.
    pub fn update_summary(
        &mut self,
        principal_disbursed: Decimal,
        installments: &[LoanRepaymentScheduleInstallment],
        charges: &[LoanCharge],
        total_overpaid: Decimal,
    ) {
        let schedule_principal: Decimal = installments.iter().map(|i| i.principal_amount).sum();
        self.total_principal_disbursed = if principal_disbursed > Decimal::ZERO {
            principal_disbursed
        } else {
            schedule_principal
        };

        self.total_principal_repaid = installments.iter().map(|i| i.principal_completed).sum();
        self.total_principal_writtenoff =
            installments.iter().map(|i| i.principal_writtenoff).sum();
        self.total_principal_outstanding = (self.total_principal_disbursed
            - self.total_principal_repaid
            - self.total_principal_writtenoff)
            .max(Decimal::ZERO);

        self.total_interest_charged = installments.iter().map(|i| i.interest_amount).sum();
        self.total_interest_repaid = installments.iter().map(|i| i.interest_completed).sum();
        self.total_interest_waived = installments.iter().map(|i| i.interest_waived).sum();
        self.total_interest_writtenoff = installments.iter().map(|i| i.interest_writtenoff).sum();
        self.total_interest_outstanding = (self.total_interest_charged
            - self.total_interest_repaid
            - self.total_interest_waived
            - self.total_interest_writtenoff)
            .max(Decimal::ZERO);

        let disbursement_fee_charged: Decimal = charges
            .iter()
            .filter(|c| c.is_active && !c.is_penalty && c.is_due_at_disbursement())
            .map(|c| c.amount)
            .sum();
        let disbursement_fee_repaid: Decimal = charges
            .iter()
            .filter(|c| c.is_active && !c.is_penalty && c.is_due_at_disbursement())
            .map(|c| c.amount_paid)
            .sum();
        let disbursement_fee_waived: Decimal = charges
            .iter()
            .filter(|c| c.is_active && !c.is_penalty && c.is_due_at_disbursement())
            .map(|c| c.amount_waived)
            .sum();
        let disbursement_fee_writtenoff: Decimal = charges
            .iter()
            .filter(|c| c.is_active && !c.is_penalty && c.is_due_at_disbursement())
            .map(|c| c.amount_writtenoff)
            .sum();

        let schedule_fee_charged: Decimal =
            installments.iter().map(|i| i.fee_charges_amount).sum();
        let schedule_fee_repaid: Decimal =
            installments.iter().map(|i| i.fee_charges_completed).sum();
        let schedule_fee_waived: Decimal =
            installments.iter().map(|i| i.fee_charges_waived).sum();
        let schedule_fee_writtenoff: Decimal =
            installments.iter().map(|i| i.fee_charges_writtenoff).sum();

        self.total_fee_charges_charged = schedule_fee_charged + disbursement_fee_charged;
        self.total_fee_charges_repaid = schedule_fee_repaid + disbursement_fee_repaid;
        let total_fee_waived = schedule_fee_waived + disbursement_fee_waived;
        let total_fee_writtenoff = schedule_fee_writtenoff + disbursement_fee_writtenoff;
        self.total_fee_charges_outstanding = (self.total_fee_charges_charged
            - self.total_fee_charges_repaid
            - total_fee_waived
            - total_fee_writtenoff)
            .max(Decimal::ZERO);

        self.total_penalty_charges_charged =
            installments.iter().map(|i| i.penalty_charges_amount).sum();
        self.total_penalty_charges_repaid = installments
            .iter()
            .map(|i| i.penalty_charges_completed)
            .sum();
        let total_penalty_waived: Decimal =
            installments.iter().map(|i| i.penalty_charges_waived).sum();
        let total_penalty_writtenoff: Decimal = installments
            .iter()
            .map(|i| i.penalty_charges_writtenoff)
            .sum();
        self.total_penalty_charges_outstanding = (self.total_penalty_charges_charged
            - self.total_penalty_charges_repaid
            - total_penalty_waived
            - total_penalty_writtenoff)
            .max(Decimal::ZERO);

        self.total_expected_repayment = self.total_principal_disbursed
            + self.total_interest_charged
            + self.total_fee_charges_charged
            + self.total_penalty_charges_charged;

        self.total_repayment = self.total_principal_repaid
            + self.total_interest_repaid
            + self.total_fee_charges_repaid
            + self.total_penalty_charges_repaid;

        self.total_outstanding = self.total_principal_outstanding
            + self.total_interest_outstanding
            + self.total_fee_charges_outstanding
            + self.total_penalty_charges_outstanding;

        self.total_overpaid = total_overpaid.max(Decimal::ZERO);
    }

    #[inline]
    pub fn is_repaid_in_full(&self) -> bool {
        self.total_outstanding == Decimal::ZERO
    }

    #[inline]
    pub fn total_expected_cost_of_loan(&self) -> Decimal {
        self.total_interest_charged
            + self.total_fee_charges_charged
            + self.total_penalty_charges_charged
    }

    #[inline]
    pub fn total_cost_of_loan(&self) -> Decimal {
        self.total_interest_repaid
            + self.total_fee_charges_repaid
            + self.total_penalty_charges_repaid
    }

    pub fn total_outstanding_money(
        &self,
        currency: MonetaryCurrency,
        mode: FineractRoundingMode,
    ) -> Money {
        Money::of(currency, self.total_outstanding, mode)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[test]
    fn loan_transaction_type_discriminants_and_predicates_match_upstream() {
        assert_eq!(LoanTransactionType::Invalid as i16, 0);
        assert_eq!(LoanTransactionType::Disbursement as i16, 1);
        assert_eq!(LoanTransactionType::Repayment as i16, 2);
        assert_eq!(LoanTransactionType::Contra as i16, 3);
        assert_eq!(LoanTransactionType::WaiveInterest as i16, 4);
        assert_eq!(LoanTransactionType::RepaymentAtDisbursement as i16, 5);
        assert_eq!(LoanTransactionType::WriteOff as i16, 6);
        assert_eq!(LoanTransactionType::MarkedForRescheduling as i16, 7);
        assert_eq!(LoanTransactionType::RecoveryRepayment as i16, 8);
        assert_eq!(LoanTransactionType::WaiveCharges as i16, 9);
        assert_eq!(LoanTransactionType::Accrual as i16, 10);
        assert_eq!(LoanTransactionType::InitiateTransfer as i16, 12);
        assert_eq!(LoanTransactionType::ApproveTransfer as i16, 13);
        assert_eq!(LoanTransactionType::WithdrawTransfer as i16, 14);
        assert_eq!(LoanTransactionType::RejectTransfer as i16, 15);
        assert_eq!(LoanTransactionType::Refund as i16, 16);
        assert_eq!(LoanTransactionType::ChargePayment as i16, 17);
        assert_eq!(LoanTransactionType::RefundForActiveLoan as i16, 18);
        assert_eq!(LoanTransactionType::IncomePosting as i16, 19);
        assert_eq!(LoanTransactionType::CreditBalanceRefund as i16, 20);
        assert_eq!(LoanTransactionType::MerchantIssuedRefund as i16, 21);
        assert_eq!(LoanTransactionType::PayoutRefund as i16, 22);
        assert_eq!(LoanTransactionType::GoodwillCredit as i16, 23);
        assert_eq!(LoanTransactionType::ChargeRefund as i16, 24);
        assert_eq!(LoanTransactionType::Chargeback as i16, 25);
        assert_eq!(LoanTransactionType::ChargeAdjustment as i16, 26);
        assert_eq!(LoanTransactionType::ChargeOff as i16, 27);
        assert_eq!(LoanTransactionType::DownPayment as i16, 28);
        assert_eq!(LoanTransactionType::ReAge as i16, 29);
        assert_eq!(LoanTransactionType::ReAmortize as i16, 30);
        assert_eq!(LoanTransactionType::AccrualActivity as i16, 31);
        assert_eq!(LoanTransactionType::InterestPaymentWaiver as i16, 32);
        assert_eq!(LoanTransactionType::CapitalizedIncome as i16, 33);
        assert_eq!(LoanTransactionType::BuyDownFee as i16, 34);

        assert_eq!(LoanTransactionType::from_i16(11), LoanTransactionType::Invalid);
        assert_eq!(
            LoanTransactionType::from_int(Some(28)),
            LoanTransactionType::DownPayment
        );
        assert!(LoanTransactionType::Repayment.is_repayment_type());
        assert!(LoanTransactionType::GoodwillCredit.is_repayment_type());
        assert!(LoanTransactionType::DownPayment.is_repayment_type());
        assert!(!LoanTransactionType::Disbursement.is_repayment_type());
    }

    #[test]
    fn loan_term_variation_type_discriminants_and_predicates_match_upstream() {
        assert_eq!(LoanTermVariationType::Invalid as i16, 0);
        assert_eq!(LoanTermVariationType::EmiAmount as i16, 1);
        assert_eq!(LoanTermVariationType::InterestRate as i16, 2);
        assert_eq!(LoanTermVariationType::PrincipalAmount as i16, 3);
        assert_eq!(LoanTermVariationType::DueDate as i16, 4);
        assert_eq!(LoanTermVariationType::InsertInstallment as i16, 5);
        assert_eq!(LoanTermVariationType::DeleteInstallment as i16, 6);
        assert_eq!(LoanTermVariationType::GraceOnInterest as i16, 7);
        assert_eq!(LoanTermVariationType::GraceOnPrincipal as i16, 8);
        assert_eq!(LoanTermVariationType::ExtendRepaymentPeriod as i16, 9);
        assert_eq!(LoanTermVariationType::InterestRateFromInstallment as i16, 10);

        assert!(LoanTermVariationType::from_i16(2).is_interest_rate_variation());
        assert!(LoanTermVariationType::from_i16(10).is_interest_rate_from_installment());
        assert!(LoanTermVariationType::from_i16(99).is_invalid());
    }

    #[test]
    fn installment_payment_waiver_writeoff_and_reset_lifecycle() {
        let mut inst = LoanRepaymentScheduleInstallment::new(
            Some(101),
            1,
            Some(d(2026, 1, 1)),
            d(2026, 2, 1),
            dec!(1000.00),
            dec!(50.00),
            dec!(15.00),
            dec!(10.00),
        );

        assert_eq!(inst.principal_outstanding(), dec!(1000.00));
        assert_eq!(inst.interest_outstanding(), dec!(50.00));
        assert_eq!(inst.fee_charges_outstanding(), dec!(15.00));
        assert_eq!(inst.penalty_charges_outstanding(), dec!(10.00));
        assert_eq!(inst.total_outstanding(), dec!(1075.00));
        assert!(!inst.obligations_met);

        // Pay penalty in advance (2026-01-15 < 2026-02-01)
        let paid_penalty = inst.pay_penalty_charges(d(2026, 1, 15), dec!(25.00));
        assert_eq!(paid_penalty, dec!(10.00));
        assert_eq!(inst.penalty_charges_outstanding(), dec!(0.00));
        assert_eq!(inst.total_paid_in_advance, dec!(10.00));

        // Pay fee on due date
        let paid_fee = inst.pay_fee_charges(d(2026, 2, 1), dec!(15.00));
        assert_eq!(paid_fee, dec!(15.00));
        assert_eq!(inst.fee_charges_outstanding(), dec!(0.00));
        assert_eq!(inst.total_paid_in_advance, dec!(10.00));
        assert_eq!(inst.total_paid_late, dec!(0.00));

        // Waive $20 interest, pay $30 interest late (2026-02-05 > 2026-02-01)
        let waived_interest = inst.waive_interest(d(2026, 2, 2), dec!(20.00));
        assert_eq!(waived_interest, dec!(20.00));
        assert_eq!(inst.interest_outstanding(), dec!(30.00));

        let paid_interest = inst.pay_interest(d(2026, 2, 5), dec!(30.00));
        assert_eq!(paid_interest, dec!(30.00));
        assert_eq!(inst.interest_outstanding(), dec!(0.00));
        assert_eq!(inst.total_paid_late, dec!(30.00));

        // Partially pay principal ($600), then write off remaining ($400)
        let paid_principal = inst.pay_principal(d(2026, 2, 5), dec!(600.00));
        assert_eq!(paid_principal, dec!(600.00));
        assert_eq!(inst.principal_outstanding(), dec!(400.00));
        assert!(!inst.obligations_met);

        let written_off = inst.write_off(d(2026, 2, 10));
        assert_eq!(written_off, dec!(400.00));
        assert_eq!(inst.principal_writtenoff, dec!(400.00));
        assert_eq!(inst.total_outstanding(), dec!(0.00));
        assert!(inst.obligations_met);
        assert_eq!(inst.obligations_met_on_date, Some(d(2026, 2, 10)));

        // Reset derived components restores original schedule state
        inst.reset_derived_components();
        assert_eq!(inst.total_outstanding(), dec!(1075.00));
        assert!(!inst.obligations_met);
        assert_eq!(inst.obligations_met_on_date, None);
        assert_eq!(inst.total_paid_in_advance, dec!(0.00));
        assert_eq!(inst.total_paid_late, dec!(0.00));
    }

    #[test]
    fn loan_charge_and_installment_charge_calculations() {
        let mut charge = LoanCharge::new(
            Some(10),
            1,
            ChargeTimeType::InstallmentFee,
            None,
            ChargeCalculationType::PercentOfAmount,
            ChargePaymentMode::Regular,
            dec!(2.5),
            dec!(1000.00),
            false,
            Some(dec!(30.00)),
            Some(dec!(100.00)),
        );
        // 2.5% of 1000 = 25, clamped up to min_cap = 30.00
        assert_eq!(charge.amount, dec!(30.00));
        assert_eq!(charge.amount_outstanding, dec!(30.00));

        charge.installment_charges = vec![
            LoanInstallmentCharge::new(1, Some(d(2026, 2, 1)), dec!(15.00)),
            LoanInstallmentCharge::new(2, Some(d(2026, 3, 1)), dec!(15.00)),
        ];

        let paid_first = charge.pay(dec!(15.00), Some(1));
        assert_eq!(paid_first, dec!(15.00));
        assert_eq!(charge.amount_outstanding, dec!(15.00));
        assert!(charge.installment_charges[0].is_paid);

        let waived_second = charge.waive(Some(2));
        assert_eq!(waived_second, dec!(15.00));
        assert_eq!(charge.amount_outstanding, dec!(0.00));
        assert!(charge.waived);
    }

    #[test]
    fn loan_summary_aggregates_installments_and_disbursement_charges() {
        let mut i1 = LoanRepaymentScheduleInstallment::new(
            Some(1),
            1,
            Some(d(2026, 1, 1)),
            d(2026, 2, 1),
            dec!(500.00),
            dec!(25.00),
            dec!(10.00),
            dec!(5.00),
        );
        i1.pay_penalty_charges(d(2026, 2, 1), dec!(5.00));
        i1.pay_fee_charges(d(2026, 2, 1), dec!(10.00));
        i1.pay_interest(d(2026, 2, 1), dec!(25.00));
        i1.pay_principal(d(2026, 2, 1), dec!(500.00));

        let mut i2 = LoanRepaymentScheduleInstallment::new(
            Some(1),
            2,
            Some(d(2026, 2, 1)),
            d(2026, 3, 1),
            dec!(500.00),
            dec!(25.00),
            dec!(10.00),
            dec!(0.00),
        );
        i2.waive_interest(d(2026, 2, 15), dec!(5.00));

        let mut summary = LoanSummary::new();
        summary.update_summary(dec!(1000.00), &[i1, i2], &[], dec!(0.00));

        assert_eq!(summary.total_principal_disbursed, dec!(1000.00));
        assert_eq!(summary.total_principal_repaid, dec!(500.00));
        assert_eq!(summary.total_principal_outstanding, dec!(500.00));
        assert_eq!(summary.total_interest_charged, dec!(50.00));
        assert_eq!(summary.total_interest_repaid, dec!(25.00));
        assert_eq!(summary.total_interest_waived, dec!(5.00));
        assert_eq!(summary.total_interest_outstanding, dec!(20.00));
        assert_eq!(summary.total_fee_charges_charged, dec!(20.00));
        assert_eq!(summary.total_fee_charges_repaid, dec!(10.00));
        assert_eq!(summary.total_fee_charges_outstanding, dec!(10.00));
        assert_eq!(summary.total_penalty_charges_charged, dec!(5.00));
        assert_eq!(summary.total_penalty_charges_repaid, dec!(5.00));
        assert_eq!(summary.total_penalty_charges_outstanding, dec!(0.00));
        assert_eq!(summary.total_expected_repayment, dec!(1075.00));
        assert_eq!(summary.total_repayment, dec!(540.00));
        assert_eq!(summary.total_outstanding, dec!(530.00));
        assert!(!summary.is_repaid_in_full());
    }
}
