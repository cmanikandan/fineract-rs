//! Savings Account and Transactions domain model (`org.apache.fineract.portfolio.savings.domain`).
//!
//! Ported from Apache Fineract's `SavingsAccount`, `SavingsAccountTransaction`,
//! `SavingsAccountCharge`, `SavingsAccountSummary`, `SavingsAccountStatusType`,
//! `SavingsAccountSubStatusEnum`, and `SavingsAccountTransactionType`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use fineract_charge_rs::{Charge, ChargeCalculationType, ChargeTimeType};
use fineract_core_rs::EnumOptionData;
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::PlatformDomainRuleException;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ============================================================================
// 1. SavingsAccountStatusType
// ============================================================================

/// Lifecycle status of a savings account (`org.apache.fineract.portfolio.savings.domain.SavingsAccountStatusType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum SavingsAccountStatusType {
    #[default]
    Invalid = 0,
    SubmittedAndPendingApproval = 100,
    Approved = 200,
    Active = 300,
    WithdrawnByApplicant = 400,
    Rejected = 500,
    Closed = 600,
    PrematureClosed = 700,
    TransferInProgress = 800,
    TransferOnHold = 900,
    Matured = 1000,
}

impl SavingsAccountStatusType {
    /// Alias matching Fineract's `PRE_MATURE_CLOSURE` enum constant.
    pub const PRE_MATURE_CLOSURE: Self = Self::PrematureClosed;

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn get_value(self) -> i32 {
        self as i32
    }

    pub fn from_int(value: i32) -> Self {
        match value {
            100 => Self::SubmittedAndPendingApproval,
            200 => Self::Approved,
            300 => Self::Active,
            303 | 800 => Self::TransferInProgress,
            304 | 900 => Self::TransferOnHold,
            400 => Self::WithdrawnByApplicant,
            500 => Self::Rejected,
            600 => Self::Closed,
            700 => Self::PrematureClosed,
            1000 => Self::Matured,
            _ => Self::Invalid,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "savingsAccountStatusType.invalid",
            Self::SubmittedAndPendingApproval => {
                "savingsAccountStatusType.submitted.and.pending.approval"
            }
            Self::Approved => "savingsAccountStatusType.approved",
            Self::Active => "savingsAccountStatusType.active",
            Self::TransferInProgress => "savingsAccountStatusType.transfer.in.progress",
            Self::TransferOnHold => "savingsAccountStatusType.transfer.on.hold",
            Self::WithdrawnByApplicant => "savingsAccountStatusType.withdrawn.by.applicant",
            Self::Rejected => "savingsAccountStatusType.rejected",
            Self::Closed => "savingsAccountStatusType.closed",
            Self::PrematureClosed => "savingsAccountStatusType.pre.mature.closure",
            Self::Matured => "savingsAccountStatusType.matured",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::SubmittedAndPendingApproval => "Submitted and pending approval",
            Self::Approved => "Approved",
            Self::Active => "Active",
            Self::TransferInProgress => "Transfer in progress",
            Self::TransferOnHold => "Transfer on hold",
            Self::WithdrawnByApplicant => "Withdrawn by applicant",
            Self::Rejected => "Rejected",
            Self::Closed => "Closed",
            Self::PrematureClosed => "Premature Closed",
            Self::Matured => "Matured",
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(self.as_i32() as i64, self.code(), self.description())
    }

    pub const fn is_submitted_and_pending_approval(self) -> bool {
        matches!(self, Self::SubmittedAndPendingApproval)
    }

    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub const fn is_active_or_awaiting_approval_or_disbursal(self) -> bool {
        self.is_approved() || self.is_submitted_and_pending_approval() || self.is_active()
    }

    pub const fn is_withdrawn_by_applicant(self) -> bool {
        matches!(self, Self::WithdrawnByApplicant)
    }

    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    pub const fn is_closed(self) -> bool {
        matches!(
            self,
            Self::Closed | Self::PrematureClosed | Self::Rejected | Self::WithdrawnByApplicant
        )
    }

    pub const fn is_pre_mature_closure(self) -> bool {
        matches!(self, Self::PrematureClosed)
    }

    pub const fn is_transfer_in_progress(self) -> bool {
        matches!(self, Self::TransferInProgress)
    }

    pub const fn is_transfer_on_hold(self) -> bool {
        matches!(self, Self::TransferOnHold)
    }

    pub const fn is_matured(self) -> bool {
        matches!(self, Self::Matured)
    }

    pub const fn is_under_transfer(self) -> bool {
        self.is_transfer_in_progress() || self.is_transfer_on_hold()
    }
}

// ============================================================================
// 2. SavingsAccountSubStatusEnum
// ============================================================================

/// Sub-status of a savings account (`org.apache.fineract.portfolio.savings.domain.SavingsAccountSubStatusEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum SavingsAccountSubStatusEnum {
    #[default]
    None = 0,
    Inactive = 100,
    Dormant = 200,
    Escheat = 300,
    Block = 400,
    BlockCredit = 500,
    BlockDebit = 600,
}

impl SavingsAccountSubStatusEnum {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn get_value(self) -> i32 {
        self as i32
    }

    pub fn from_int(value: i32) -> Self {
        match value {
            100 => Self::Inactive,
            200 => Self::Dormant,
            300 => Self::Escheat,
            400 => Self::Block,
            500 => Self::BlockCredit,
            600 => Self::BlockDebit,
            _ => Self::None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::None => "SavingsAccountSubStatusEnum.none",
            Self::Inactive => "SavingsAccountSubStatusEnum.inactive",
            Self::Dormant => "SavingsAccountSubStatusEnum.dormant",
            Self::Escheat => "SavingsAccountSubStatusEnum.escheat",
            Self::Block => "SavingsAccountSubStatusEnum.block",
            Self::BlockCredit => "SavingsAccountSubStatusEnum.blockCredit",
            Self::BlockDebit => "SavingsAccountSubStatusEnum.blockDebit",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Inactive => "Inactive",
            Self::Dormant => "Dormant",
            Self::Escheat => "Escheat",
            Self::Block => "Block",
            Self::BlockCredit => "BlockCredit",
            Self::BlockDebit => "BlockDebit",
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(self.as_i32() as i64, self.code(), self.description())
    }

    pub const fn is_sub_status_none(self) -> bool {
        matches!(self, Self::None)
    }

    pub const fn is_sub_status_inactive(self) -> bool {
        matches!(self, Self::Inactive)
    }

    pub const fn is_sub_status_dormant(self) -> bool {
        matches!(self, Self::Dormant)
    }

    pub const fn is_sub_status_escheat(self) -> bool {
        matches!(self, Self::Escheat)
    }

    pub const fn is_sub_status_account_blocked(self) -> bool {
        matches!(self, Self::Block)
    }

    pub const fn is_sub_status_credit_blocked(self) -> bool {
        matches!(self, Self::BlockCredit)
    }

    pub const fn is_sub_status_debit_blocked(self) -> bool {
        matches!(self, Self::BlockDebit)
    }
}

// ============================================================================
// 3. SavingsAccountTransactionType & TransactionEntryType
// ============================================================================

/// Accounting direction of a savings transaction (`TransactionEntryType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SavingsTransactionEntryType {
    Credit,
    Debit,
}

impl SavingsTransactionEntryType {
    pub const fn is_credit(self) -> bool {
        matches!(self, Self::Credit)
    }

    pub const fn is_debit(self) -> bool {
        matches!(self, Self::Debit)
    }
}

/// Savings account transaction type (`org.apache.fineract.portfolio.savings.SavingsAccountTransactionType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum SavingsAccountTransactionType {
    #[default]
    Invalid = 0,
    Deposit = 1,
    Withdrawal = 2,
    InterestPosting = 3,
    WithdrawalFee = 4,
    AnnualFee = 5,
    WaiveCharge = 6,
    PayCharge = 7,
    DividendPayout = 8,
    Accrual = 10,
    InitiateTransfer = 12,
    ApproveTransfer = 13,
    WithdrawTransfer = 14,
    RejectTransfer = 15,
    WrittenOff = 16,
    OverdraftInterest = 17,
    WithholdTax = 18,
    Escheat = 19,
    AmountHold = 20,
    AmountRelease = 21,
}

impl SavingsAccountTransactionType {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn get_value(self) -> i32 {
        self as i32
    }

    pub fn from_int(value: i32) -> Self {
        match value {
            1 => Self::Deposit,
            2 => Self::Withdrawal,
            3 => Self::InterestPosting,
            4 => Self::WithdrawalFee,
            5 => Self::AnnualFee,
            6 => Self::WaiveCharge,
            7 => Self::PayCharge,
            8 => Self::DividendPayout,
            10 => Self::Accrual,
            12 => Self::InitiateTransfer,
            13 => Self::ApproveTransfer,
            14 => Self::WithdrawTransfer,
            15 => Self::RejectTransfer,
            16 => Self::WrittenOff,
            17 => Self::OverdraftInterest,
            18 => Self::WithholdTax,
            19 => Self::Escheat,
            20 => Self::AmountHold,
            21 => Self::AmountRelease,
            _ => Self::Invalid,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "savingsAccountTransactionType.invalid",
            Self::Deposit => "savingsAccountTransactionType.deposit",
            Self::Withdrawal => "savingsAccountTransactionType.withdrawal",
            Self::InterestPosting => "savingsAccountTransactionType.interestPosting",
            Self::WithdrawalFee => "savingsAccountTransactionType.withdrawalFee",
            Self::AnnualFee => "savingsAccountTransactionType.annualFee",
            Self::WaiveCharge => "savingsAccountTransactionType.waiveCharge",
            Self::PayCharge => "savingsAccountTransactionType.payCharge",
            Self::DividendPayout => "savingsAccountTransactionType.dividendPayout",
            Self::Accrual => "savingsAccountTransactionType.accrual",
            Self::InitiateTransfer => "savingsAccountTransactionType.initiateTransfer",
            Self::ApproveTransfer => "savingsAccountTransactionType.approveTransfer",
            Self::WithdrawTransfer => "savingsAccountTransactionType.withdrawTransfer",
            Self::RejectTransfer => "savingsAccountTransactionType.rejectTransfer",
            Self::WrittenOff => "savingsAccountTransactionType.writtenoff",
            Self::OverdraftInterest => "savingsAccountTransactionType.overdraftInterest",
            Self::WithholdTax => "savingsAccountTransactionType.withholdTax",
            Self::Escheat => "savingsAccountTransactionType.escheat",
            Self::AmountHold => "savingsAccountTransactionType.amountHold",
            Self::AmountRelease => "savingsAccountTransactionType.amountRelease",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Deposit => "Deposit",
            Self::Withdrawal => "Withdrawal",
            Self::InterestPosting => "Interest Posting",
            Self::WithdrawalFee => "Withdrawal fee",
            Self::AnnualFee => "Annual fee",
            Self::WaiveCharge => "Waive Charge",
            Self::PayCharge => "Pay Charge",
            Self::DividendPayout => "Dividend Payout",
            Self::Accrual => "Accrual",
            Self::InitiateTransfer => "Transfer initiated",
            Self::ApproveTransfer => "Transfer approved",
            Self::WithdrawTransfer => "Transfer Withdrawn",
            Self::RejectTransfer => "Transfer Rejected",
            Self::WrittenOff => "Written Off",
            Self::OverdraftInterest => "Overdraft Interest",
            Self::WithholdTax => "Withhold Tax",
            Self::Escheat => "Escheat",
            Self::AmountHold => "Amount On Hold",
            Self::AmountRelease => "Amount Released",
        }
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(self.as_i32() as i64, self.code(), self.description())
    }

    pub const fn entry_type(self) -> Option<SavingsTransactionEntryType> {
        match self {
            Self::Deposit | Self::InterestPosting | Self::DividendPayout => {
                Some(SavingsTransactionEntryType::Credit)
            }
            Self::Withdrawal
            | Self::WithdrawalFee
            | Self::AnnualFee
            | Self::PayCharge
            | Self::OverdraftInterest
            | Self::WithholdTax
            | Self::Escheat => Some(SavingsTransactionEntryType::Debit),
            Self::Invalid
            | Self::WaiveCharge
            | Self::Accrual
            | Self::InitiateTransfer
            | Self::ApproveTransfer
            | Self::WithdrawTransfer
            | Self::RejectTransfer
            | Self::WrittenOff
            | Self::AmountHold
            | Self::AmountRelease => None,
        }
    }

    pub const fn is_credit_entry_type(self) -> bool {
        matches!(self.entry_type(), Some(SavingsTransactionEntryType::Credit))
    }

    pub const fn is_debit_entry_type(self) -> bool {
        matches!(self.entry_type(), Some(SavingsTransactionEntryType::Debit))
    }

    pub const fn is_deposit(self) -> bool {
        matches!(self, Self::Deposit)
    }

    pub const fn is_withdrawal(self) -> bool {
        matches!(self, Self::Withdrawal)
    }

    pub const fn is_interest_posting(self) -> bool {
        matches!(self, Self::InterestPosting)
    }

    pub const fn is_overdraft_interest(self) -> bool {
        matches!(self, Self::OverdraftInterest)
    }

    pub const fn is_withhold_tax(self) -> bool {
        matches!(self, Self::WithholdTax)
    }

    pub const fn is_withdrawal_fee(self) -> bool {
        matches!(self, Self::WithdrawalFee)
    }

    pub const fn is_annual_fee(self) -> bool {
        matches!(self, Self::AnnualFee)
    }

    pub const fn is_pay_charge(self) -> bool {
        matches!(self, Self::PayCharge)
    }

    pub const fn is_charge_transaction(self) -> bool {
        self.is_pay_charge() || self.is_withdrawal_fee() || self.is_annual_fee()
    }

    pub const fn is_waive_charge(self) -> bool {
        matches!(self, Self::WaiveCharge)
    }

    pub const fn is_transfer_initiation(self) -> bool {
        matches!(self, Self::InitiateTransfer)
    }

    pub const fn is_transfer_approval(self) -> bool {
        matches!(self, Self::ApproveTransfer)
    }

    pub const fn is_transfer_rejection(self) -> bool {
        matches!(self, Self::RejectTransfer)
    }

    pub const fn is_transfer_withdrawal(self) -> bool {
        matches!(self, Self::WithdrawTransfer)
    }

    pub const fn is_writtenoff(self) -> bool {
        matches!(self, Self::WrittenOff)
    }

    pub const fn is_dividend_payout(self) -> bool {
        matches!(self, Self::DividendPayout)
    }

    pub const fn is_income_from_interest(self) -> bool {
        matches!(self, Self::OverdraftInterest)
    }

    pub const fn is_escheat(self) -> bool {
        matches!(self, Self::Escheat)
    }

    pub const fn is_amount_on_hold(self) -> bool {
        matches!(self, Self::AmountHold)
    }

    pub const fn is_amount_release(self) -> bool {
        matches!(self, Self::AmountRelease)
    }

    pub const fn is_accrual(self) -> bool {
        matches!(self, Self::Accrual)
    }

    pub const fn is_credit(self) -> bool {
        self.is_credit_entry_type()
    }

    pub const fn is_debit(self) -> bool {
        self.is_debit_entry_type()
    }
}

// ============================================================================
// 4. SavingsAccountCharge & PaidBy
// ============================================================================

/// Join record linking a savings transaction to a savings charge (`m_savings_account_charge_paid_by`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsAccountChargePaidBy {
    pub id: Option<i64>,
    pub savings_account_transaction_id: Option<i64>,
    pub savings_account_charge_id: i64,
    pub amount: Decimal,
}

impl SavingsAccountChargePaidBy {
    pub fn new(
        savings_account_transaction_id: Option<i64>,
        savings_account_charge_id: i64,
        amount: Decimal,
    ) -> Self {
        Self {
            id: None,
            savings_account_transaction_id,
            savings_account_charge_id,
            amount,
        }
    }
}

/// Charge attached to a savings account (`m_savings_account_charge`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsAccountCharge {
    pub id: Option<i64>,
    pub savings_account_id: Option<i64>,
    pub charge_id: i64,
    pub name: String,
    pub charge_time_type: ChargeTimeType,
    pub charge_due_date: Option<NaiveDate>,
    pub fee_on_month: Option<u32>,
    pub fee_on_day: Option<u32>,
    pub fee_interval: Option<i32>,
    pub charge_calculation_type: ChargeCalculationType,
    pub percentage: Option<Decimal>,
    pub amount_percentage_applied_to: Option<Decimal>,
    pub currency: MonetaryCurrency,
    pub amount: Decimal,
    pub amount_paid: Decimal,
    pub amount_waived: Decimal,
    pub amount_written_off: Decimal,
    pub amount_outstanding: Decimal,
    pub is_penalty: bool,
    pub is_paid_derived: bool,
    pub waived: bool,
    pub status: bool,
    pub inactivation_date: Option<NaiveDate>,
    pub free_withdrawal_count: i32,
}

impl SavingsAccountCharge {
    pub fn new(
        id: Option<i64>,
        savings_account_id: Option<i64>,
        charge_id: i64,
        name: impl Into<String>,
        currency: MonetaryCurrency,
        charge_time_type: ChargeTimeType,
        charge_calculation_type: ChargeCalculationType,
        amount: Decimal,
        is_penalty: bool,
        charge_due_date: Option<NaiveDate>,
    ) -> Self {
        let (fee_amount, percentage, amount_percentage_applied_to) =
            if charge_calculation_type.is_percentage_of_amount() {
                (Decimal::ZERO, Some(amount), Some(Decimal::ZERO))
            } else {
                let rounded = Money::of(&currency, amount, FineractRoundingMode::HalfEven).amount();
                (rounded, None, None)
            };

        Self {
            id,
            savings_account_id,
            charge_id,
            name: name.into(),
            charge_time_type,
            charge_due_date,
            fee_on_month: None,
            fee_on_day: None,
            fee_interval: None,
            charge_calculation_type,
            percentage,
            amount_percentage_applied_to,
            currency,
            amount: fee_amount,
            amount_paid: Decimal::ZERO,
            amount_waived: Decimal::ZERO,
            amount_written_off: Decimal::ZERO,
            amount_outstanding: fee_amount,
            is_penalty,
            is_paid_derived: false,
            waived: false,
            status: true,
            inactivation_date: None,
            free_withdrawal_count: 0,
        }
    }

    /// Creates a new `SavingsAccountCharge` from a `Charge`.
    pub fn from_charge_definition(
        charge_def: &Charge,
        currency: MonetaryCurrency,
        amount_override: Option<Decimal>,
        due_date: Option<NaiveDate>,
    ) -> Self {
        let raw_amount = amount_override.unwrap_or(charge_def.amount);
        let (amount, percentage, amount_percentage_applied_to) =
            if charge_def.charge_calculation_type.is_percentage_of_amount() {
                (
                    Decimal::ZERO,
                    Some(raw_amount),
                    Some(Decimal::ZERO),
                )
            } else {
                let rounded = Money::of(&currency, raw_amount, FineractRoundingMode::HalfEven).amount();
                (rounded, None, None)
            };

        Self {
            id: None,
            savings_account_id: None,
            charge_id: charge_def.id,
            name: charge_def.name.clone(),
            charge_time_type: charge_def.charge_time_type,
            charge_due_date: due_date,
            fee_on_month: charge_def.fee_on_month.map(u32::from),
            fee_on_day: charge_def.fee_on_day.map(u32::from),
            fee_interval: charge_def.fee_interval,
            charge_calculation_type: charge_def.charge_calculation_type,
            percentage,
            amount_percentage_applied_to,
            currency,
            amount,
            amount_paid: Decimal::ZERO,
            amount_waived: Decimal::ZERO,
            amount_written_off: Decimal::ZERO,
            amount_outstanding: amount,
            is_penalty: charge_def.is_penalty,
            is_paid_derived: amount.is_zero()
                && !charge_def.charge_calculation_type.is_percentage_of_amount(),
            waived: false,
            status: true,
            inactivation_date: None,
            free_withdrawal_count: 0,
        }
    }

    /// Creates a flat or percentage withdrawal fee charge directly.
    pub fn new_withdrawal_fee(
        id: Option<i64>,
        charge_id: i64,
        name: impl Into<String>,
        currency: MonetaryCurrency,
        calculation_type: ChargeCalculationType,
        amount_or_rate: Decimal,
        is_penalty: bool,
    ) -> Self {
        let (amount, percentage) = if calculation_type.is_percentage_of_amount() {
            (Decimal::ZERO, Some(amount_or_rate))
        } else {
            let rounded = Money::of(&currency, amount_or_rate, FineractRoundingMode::HalfEven).amount();
            (rounded, None)
        };
        Self {
            id,
            savings_account_id: None,
            charge_id,
            name: name.into(),
            charge_time_type: ChargeTimeType::WithdrawalFee,
            charge_due_date: None,
            fee_on_month: None,
            fee_on_day: None,
            fee_interval: None,
            charge_calculation_type: calculation_type,
            percentage,
            amount_percentage_applied_to: None,
            currency,
            amount,
            amount_paid: Decimal::ZERO,
            amount_waived: Decimal::ZERO,
            amount_written_off: Decimal::ZERO,
            amount_outstanding: amount,
            is_penalty,
            is_paid_derived: false,
            waived: false,
            status: true,
            inactivation_date: None,
            free_withdrawal_count: 0,
        }
    }

    pub const fn is_active(&self) -> bool {
        self.status
    }

    pub const fn is_not_active(&self) -> bool {
        !self.status
    }

    pub const fn is_savings_activation(&self) -> bool {
        self.charge_time_type.is_savings_activation()
    }

    pub const fn is_withdrawal_fee(&self) -> bool {
        self.charge_time_type.is_withdrawal_fee()
    }

    pub const fn is_annual_fee(&self) -> bool {
        self.charge_time_type.is_annual_fee()
    }

    pub const fn is_monthly_fee(&self) -> bool {
        self.charge_time_type.is_monthly_fee()
    }

    pub const fn is_weekly_fee(&self) -> bool {
        self.charge_time_type.is_weekly_fee()
    }

    pub const fn is_savings_no_activity_fee(&self) -> bool {
        self.charge_time_type.is_savings_no_activity_fee()
    }

    pub const fn is_overdraft_fee(&self) -> bool {
        self.charge_time_type.is_overdraft_fee()
    }

    pub const fn is_recurring_fee(&self) -> bool {
        self.is_weekly_fee() || self.is_monthly_fee() || self.is_annual_fee()
    }

    pub const fn is_paid(&self) -> bool {
        self.is_paid_derived
    }

    pub const fn is_not_fully_paid(&self) -> bool {
        !self.is_paid()
    }

    pub const fn is_waived(&self) -> bool {
        self.waived
    }

    pub const fn is_fee_charge(&self) -> bool {
        !self.is_penalty
    }

    /// Calculates the fee amount for a given transaction amount (e.g. withdrawal amount).
    pub fn calculate_fee_for_transaction_amount(&self, transaction_amount: Decimal) -> Money {
        match self.charge_calculation_type {
            ChargeCalculationType::Flat => Money::of(self.currency.clone(), self.amount, FineractRoundingMode::HalfEven),
            ChargeCalculationType::PercentOfAmount
            | ChargeCalculationType::PercentOfAmountAndInterest
            | ChargeCalculationType::PercentOfInterest
            | ChargeCalculationType::PercentOfDisbursementAmount => {
                let pct = self.percentage.unwrap_or(self.amount);
                let raw = (transaction_amount * pct) / Decimal::from(100);
                Money::of(self.currency.clone(), raw, FineractRoundingMode::HalfEven)
            }
            ChargeCalculationType::Invalid => Money::zero(&self.currency),
        }
    }

    /// Updates the charge state for a newly triggered withdrawal fee or no-activity fee.
    pub fn update_with_dral_fee_amount(&mut self, transaction_amount: Decimal) -> Money {
        let fee_money = self.calculate_fee_for_transaction_amount(transaction_amount);
        if self.charge_calculation_type.is_percentage_of_amount() {
            self.amount_percentage_applied_to = Some(transaction_amount);
            self.amount = self.amount + fee_money.amount();
            self.amount_outstanding = self.calculate_outstanding();
            self.is_paid_derived = false;
        } else {
            self.amount = self.amount + fee_money.amount();
            self.amount_outstanding = self.calculate_outstanding();
            self.is_paid_derived = false;
        }
        fee_money
    }

    /// Pays up to `payment_amount` against this charge and returns the remaining unpaid money.
    pub fn mark_as_paid(&mut self, payment_amount: Money) -> Money {
        let amount_to_pay = if payment_amount.amount() > self.amount_outstanding {
            Money::of(&self.currency, self.amount_outstanding, FineractRoundingMode::HalfEven)
        } else {
            payment_amount.clone()
        };
        self.amount_paid += amount_to_pay.amount();
        self.amount_outstanding = self.calculate_outstanding();
        self.is_paid_derived = self.amount_outstanding.is_zero();
        if self.is_paid_derived && self.is_recurring_fee() {
            self.update_next_due_date();
        }
        payment_amount - amount_to_pay
    }

    /// Waives the outstanding balance of this charge and returns the waived amount.
    pub fn waive(&mut self) -> Money {
        let waived_money = Money::of(self.currency.clone(), self.amount_outstanding, FineractRoundingMode::HalfEven);
        self.amount_waived += waived_money.amount();
        self.amount_outstanding = Decimal::ZERO;
        self.waived = true;
        self.is_paid_derived = false;
        if self.is_recurring_fee() {
            self.update_next_due_date();
        }
        waived_money
    }

    /// Reverses a payment or waiver on this charge.
    pub fn undo_payment(&mut self, amount: Decimal) {
        self.amount_paid = (self.amount_paid - amount).max(Decimal::ZERO);
        if self.is_withdrawal_fee() {
            self.amount = (self.amount - amount).max(Decimal::ZERO);
        }
        self.amount_outstanding = self.calculate_outstanding();
        self.is_paid_derived = self.amount_outstanding.is_zero() && !self.is_withdrawal_fee();
        self.waived = false;
    }

    pub fn inactivate(&mut self, inactivation_date: NaiveDate) {
        self.status = false;
        self.inactivation_date = Some(inactivation_date);
    }

    /// Advances `charge_due_date` to the next recurrence interval for recurring fees.
    pub fn update_next_due_date(&mut self) {
        if let Some(due_date) = self.charge_due_date {
            let interval = self.fee_interval.unwrap_or(1).max(1);
            let next_date = if self.is_weekly_fee() {
                due_date + chrono::Duration::days(7 * i64::from(interval))
            } else if self.is_monthly_fee() {
                add_months_clamped(due_date, interval)
            } else if self.is_annual_fee() {
                add_months_clamped(due_date, 12 * interval)
            } else {
                due_date
            };
            self.charge_due_date = Some(next_date);
        }
    }

    fn calculate_outstanding(&self) -> Decimal {
        let accounted_for = self.amount_paid + self.amount_waived + self.amount_written_off;
        (self.amount - accounted_for).max(Decimal::ZERO)
    }
}

fn add_months_clamped(date: NaiveDate, months: i32) -> NaiveDate {
    let total_months = date.year() * 12 + (date.month0() as i32) + months;
    let target_year = total_months.div_euclid(12);
    let target_month = (total_months.rem_euclid(12) + 1) as u32;
    let max_day = last_day_of_month(target_year, target_month);
    let day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(target_year, target_month, day).unwrap_or(date)
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

// ============================================================================
// 5. SavingsAccountTransaction
// ============================================================================

/// Tax component split recorded on a savings transaction (`m_savings_account_transaction_tax_details`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsAccountTransactionTaxDetails {
    pub id: Option<i64>,
    pub tax_component_id: i64,
    pub amount: Decimal,
}

/// Savings account transaction (`m_savings_account_transaction`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsAccountTransaction {
    pub id: Option<i64>,
    pub savings_account_id: Option<i64>,
    pub office_id: Option<i64>,
    pub payment_detail_id: Option<i64>,
    pub transaction_type: SavingsAccountTransactionType,
    pub date_of: NaiveDate,
    pub amount: Decimal,
    pub reversed: bool,
    pub running_balance: Decimal,
    pub cumulative_balance: Decimal,
    pub balance_end_date: Option<NaiveDate>,
    pub balance_number_of_days: Option<i32>,
    pub overdraft_amount: Option<Decimal>,
    pub submitted_on_date: NaiveDate,
    pub is_manual: bool,
    pub is_loan_disbursement: bool,
    pub release_id_of_hold_amount: Option<i64>,
    pub reason_for_block: Option<String>,
    pub is_reversal: bool,
    pub original_transaction_id: Option<i64>,
    pub lien_transaction: bool,
    pub ref_no: Option<String>,
    pub charges_paid_by: Vec<SavingsAccountChargePaidBy>,
    pub tax_details: Vec<SavingsAccountTransactionTaxDetails>,
}

impl SavingsAccountTransaction {
    /// Creates a new deposit transaction.
    pub fn deposit(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        payment_detail_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
    ) -> Self {
        Self::new_base(
            savings_account_id,
            office_id,
            payment_detail_id,
            SavingsAccountTransactionType::Deposit,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        )
    }

    /// Creates a new dividend payout credit transaction.
    pub fn dividend_payout(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        payment_detail_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
    ) -> Self {
        Self::new_base(
            savings_account_id,
            office_id,
            payment_detail_id,
            SavingsAccountTransactionType::DividendPayout,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        )
    }

    /// Creates a new withdrawal transaction.
    pub fn withdrawal(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        payment_detail_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
    ) -> Self {
        Self::new_base(
            savings_account_id,
            office_id,
            payment_detail_id,
            SavingsAccountTransactionType::Withdrawal,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        )
    }

    /// Creates a new interest posting transaction.
    pub fn interest_posting(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
        is_manual: bool,
    ) -> Self {
        let mut tx = Self::new_base(
            savings_account_id,
            office_id,
            None,
            SavingsAccountTransactionType::InterestPosting,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        );
        tx.is_manual = is_manual;
        tx
    }

    /// Creates a new overdraft interest debit transaction.
    pub fn overdraft_interest(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
        is_manual: bool,
    ) -> Self {
        let mut tx = Self::new_base(
            savings_account_id,
            office_id,
            None,
            SavingsAccountTransactionType::OverdraftInterest,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        );
        tx.is_manual = is_manual;
        tx
    }

    /// Creates a new withhold tax debit transaction.
    pub fn withhold_tax(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
        tax_details: Vec<SavingsAccountTransactionTaxDetails>,
    ) -> Self {
        let mut tx = Self::new_base(
            savings_account_id,
            office_id,
            None,
            SavingsAccountTransactionType::WithholdTax,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        );
        tx.tax_details = tax_details;
        tx
    }

    /// Creates a new withdrawal fee transaction.
    pub fn withdrawal_fee(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
        charge_id: Option<i64>,
    ) -> Self {
        let mut tx = Self::new_base(
            savings_account_id,
            office_id,
            None,
            SavingsAccountTransactionType::WithdrawalFee,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        );
        if let Some(cid) = charge_id {
            tx.charges_paid_by.push(SavingsAccountChargePaidBy::new(
                None,
                cid,
                amount.amount(),
            ));
        }
        tx
    }

    /// Creates a new charge payment transaction (`WithdrawalFee`, `AnnualFee`, or `PayCharge`).
    pub fn pay_charge(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        payment_detail_id: Option<i64>,
        transaction_type: SavingsAccountTransactionType,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
        charge_id: i64,
    ) -> Self {
        let mut tx = Self::new_base(
            savings_account_id,
            office_id,
            payment_detail_id,
            transaction_type,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        );
        tx.charges_paid_by.push(SavingsAccountChargePaidBy::new(
            None,
            charge_id,
            amount.amount(),
        ));
        tx
    }

    /// Creates a new charge waiver transaction.
    pub fn waive_charge(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
        charge_id: i64,
    ) -> Self {
        let mut tx = Self::new_base(
            savings_account_id,
            office_id,
            None,
            SavingsAccountTransactionType::WaiveCharge,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        );
        tx.charges_paid_by.push(SavingsAccountChargePaidBy::new(
            None,
            charge_id,
            amount.amount(),
        ));
        tx
    }

    /// Creates an amount-on-hold transaction (`AmountHold`).
    pub fn hold_amount(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        payment_detail_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
        lien_transaction: bool,
        reason_for_block: Option<String>,
    ) -> Self {
        let mut tx = Self::new_base(
            savings_account_id,
            office_id,
            payment_detail_id,
            SavingsAccountTransactionType::AmountHold,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        );
        tx.lien_transaction = lien_transaction;
        tx.reason_for_block = reason_for_block;
        tx
    }

    /// Creates an amount-release transaction (`AmountRelease`) for a prior `AmountHold` transaction.
    pub fn release_amount(
        hold_transaction: &Self,
        transaction_date: NaiveDate,
        submitted_on_date: NaiveDate,
    ) -> Self {
        let mut tx = Self::new_base(
            hold_transaction.savings_account_id,
            hold_transaction.office_id,
            hold_transaction.payment_detail_id,
            SavingsAccountTransactionType::AmountRelease,
            transaction_date,
            hold_transaction.amount,
            submitted_on_date,
        );
        tx.running_balance = hold_transaction.running_balance;
        tx.lien_transaction = hold_transaction.lien_transaction;
        tx
    }

    /// Creates an escheat transaction when a dormant account escheats.
    pub fn escheat(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
    ) -> Self {
        Self::new_base(
            savings_account_id,
            office_id,
            None,
            SavingsAccountTransactionType::Escheat,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        )
    }

    /// Creates a write-off transaction for a negative overdraft balance.
    pub fn write_off(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
    ) -> Self {
        Self::new_base(
            savings_account_id,
            office_id,
            None,
            SavingsAccountTransactionType::WrittenOff,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        )
    }

    /// Creates an accrual transaction.
    pub fn accrual(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        transaction_date: NaiveDate,
        amount: Money,
        submitted_on_date: NaiveDate,
    ) -> Self {
        Self::new_base(
            savings_account_id,
            office_id,
            None,
            SavingsAccountTransactionType::Accrual,
            transaction_date,
            amount.amount(),
            submitted_on_date,
        )
    }

    fn new_base(
        savings_account_id: Option<i64>,
        office_id: Option<i64>,
        payment_detail_id: Option<i64>,
        transaction_type: SavingsAccountTransactionType,
        date_of: NaiveDate,
        amount: Decimal,
        submitted_on_date: NaiveDate,
    ) -> Self {
        Self {
            id: None,
            savings_account_id,
            office_id,
            payment_detail_id,
            transaction_type,
            date_of,
            amount,
            reversed: false,
            running_balance: Decimal::ZERO,
            cumulative_balance: Decimal::ZERO,
            balance_end_date: Some(date_of),
            balance_number_of_days: Some(0),
            overdraft_amount: None,
            submitted_on_date,
            is_manual: false,
            is_loan_disbursement: false,
            release_id_of_hold_amount: None,
            reason_for_block: None,
            is_reversal: false,
            original_transaction_id: None,
            lien_transaction: false,
            ref_no: None,
            charges_paid_by: Vec::new(),
            tax_details: Vec::new(),
        }
    }

    pub const fn transaction_date(&self) -> NaiveDate {
        self.date_of
    }

    pub const fn is_reversed(&self) -> bool {
        self.reversed
    }

    pub const fn is_not_reversed(&self) -> bool {
        !self.reversed
    }

    pub const fn is_reversal_transaction(&self) -> bool {
        self.is_reversal
    }

    pub const fn is_deposit(&self) -> bool {
        self.transaction_type.is_deposit()
    }

    pub const fn is_deposit_and_not_reversed(&self) -> bool {
        self.is_deposit() && self.is_not_reversed()
    }

    pub const fn is_dividend_payout(&self) -> bool {
        self.transaction_type.is_dividend_payout()
    }

    pub const fn is_dividend_payout_and_not_reversed(&self) -> bool {
        self.is_dividend_payout() && self.is_not_reversed()
    }

    pub const fn is_withdrawal(&self) -> bool {
        self.transaction_type.is_withdrawal()
    }

    pub const fn is_post_interest_calculation_required(&self) -> bool {
        self.is_interest_posting() && self.is_not_reversed()
    }

    pub const fn is_interest_posting(&self) -> bool {
        self.transaction_type.is_interest_posting()
    }

    pub const fn is_interest_posting_and_not_reversed(&self) -> bool {
        self.is_interest_posting() && self.is_not_reversed()
    }

    pub const fn is_overdraft_interest_and_not_reversed(&self) -> bool {
        self.transaction_type.is_overdraft_interest() && self.is_not_reversed()
    }

    pub const fn is_withhold_tax_and_not_reversed(&self) -> bool {
        self.transaction_type.is_withhold_tax() && self.is_not_reversed()
    }

    pub const fn is_withdrawal_fee_and_not_reversed(&self) -> bool {
        self.transaction_type.is_withdrawal_fee() && self.is_not_reversed()
    }

    pub const fn is_annual_fee_and_not_reversed(&self) -> bool {
        self.transaction_type.is_annual_fee() && self.is_not_reversed()
    }

    pub const fn is_pay_charge(&self) -> bool {
        self.transaction_type.is_pay_charge()
    }

    pub const fn is_charge_transaction(&self) -> bool {
        self.transaction_type.is_charge_transaction()
    }

    pub const fn is_charge_transaction_and_not_reversed(&self) -> bool {
        self.transaction_type.is_charge_transaction() && self.is_not_reversed()
    }

    pub const fn is_waive_charge(&self) -> bool {
        self.transaction_type.is_waive_charge()
    }

    pub const fn is_waive_charge_and_not_reversed(&self) -> bool {
        self.transaction_type.is_waive_charge() && self.is_not_reversed()
    }

    pub const fn is_amount_on_hold(&self) -> bool {
        self.transaction_type.is_amount_on_hold()
    }

    pub const fn is_amount_release(&self) -> bool {
        self.transaction_type.is_amount_release()
    }

    pub const fn is_amount_on_hold_not_released(&self) -> bool {
        self.is_amount_on_hold()
            && self.is_not_reversed()
            && self.release_id_of_hold_amount.is_none()
    }

    pub const fn is_credit(&self) -> bool {
        self.is_deposit()
            || self.is_interest_posting()
            || self.is_dividend_payout()
            || self.transaction_type.is_approve_transfer()
            || self.transaction_type.is_withdraw_transfer()
            || self.transaction_type.is_writtenoff()
    }

    pub const fn is_debit(&self) -> bool {
        self.is_withdrawal()
            || self.is_withdrawal_fee_and_not_reversed()
            || self.is_annual_fee_and_not_reversed()
            || self.is_pay_charge()
            || self.is_overdraft_interest_and_not_reversed()
            || self.is_withhold_tax_and_not_reversed()
            || self.transaction_type.is_initiate_transfer()
            || self.transaction_type.is_reject_transfer()
            || self.transaction_type.is_escheat()
    }

    /// Marks this transaction as reversed and zeroes its balance fields.
    pub fn reverse(&mut self) {
        self.reversed = true;
        self.zero_balance_fields();
    }

    /// Resets running balance, cumulative balance, end date, and days to zero/None.
    pub fn zero_balance_fields(&mut self) {
        self.running_balance = Decimal::ZERO;
        self.cumulative_balance = Decimal::ZERO;
        self.balance_end_date = None;
        self.balance_number_of_days = None;
    }

    /// Updates the running balance on this transaction.
    pub fn update_running_balance(&mut self, balance: Money) {
        self.running_balance = balance.amount();
    }

    /// Updates the end date, number of days, and cumulative balance for daily interest calculation.
    pub fn update_cumulative_balance_and_dates(
        &mut self,
        currency: MonetaryCurrency,
        end_date: Option<NaiveDate>,
    ) {
        self.balance_end_date = end_date;
        if let Some(end) = end_date {
            let days = (end - self.date_of).num_days().max(0) as i32;
            self.balance_number_of_days = Some(days);
            let cum = Money::of(currency, self.running_balance, FineractRoundingMode::HalfEven) * i64::from(days);
            self.cumulative_balance = cum.amount();
        } else {
            self.balance_number_of_days = Some(0);
            self.cumulative_balance = Decimal::ZERO;
        }
    }

    pub fn update_overdraft_amount(&mut self, overdraft_amount: Option<Decimal>) {
        self.overdraft_amount = overdraft_amount;
    }

    pub fn get_amount(&self, currency: impl Into<MonetaryCurrency>) -> Money {
        Money::of(currency, self.amount, FineractRoundingMode::HalfEven)
    }

    pub fn get_running_balance(&self, currency: impl Into<MonetaryCurrency>) -> Money {
        Money::of(currency, self.running_balance, FineractRoundingMode::HalfEven)
    }
}

impl SavingsAccountTransactionType {
    const fn is_approve_transfer(self) -> bool {
        self.is_transfer_approval()
    }
    const fn is_withdraw_transfer(self) -> bool {
        self.is_transfer_withdrawal()
    }
    const fn is_initiate_transfer(self) -> bool {
        self.is_transfer_initiation()
    }
    const fn is_reject_transfer(self) -> bool {
        self.is_transfer_rejection()
    }
}

/// Comparator ordering transactions chronologically by `(date_of, submitted_on_date, id)`.
/// Mirrors `SavingsAccountTransactionComparator` in Apache Fineract.
pub fn compare_savings_transactions(
    a: &SavingsAccountTransaction,
    b: &SavingsAccountTransaction,
) -> Ordering {
    match a.date_of.cmp(&b.date_of) {
        Ordering::Equal => match a.submitted_on_date.cmp(&b.submitted_on_date) {
            Ordering::Equal => match (a.id, b.id) {
                (Some(id_a), Some(id_b)) => id_a.cmp(&id_b),
                // New unsaved transactions (None) sort after persisted transactions on the same day
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (None, None) => Ordering::Equal,
            },
            other => other,
        },
        other => other,
    }
}

// ============================================================================
// 6. SavingsAccountSummary
// ============================================================================

/// Aggregated balance and transaction summary embedded in `SavingsAccount`
/// (`org.apache.fineract.portfolio.savings.domain.SavingsAccountSummary`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SavingsAccountSummary {
    pub total_deposits: Decimal,
    pub total_withdrawals: Decimal,
    pub total_withdrawal_fees: Decimal,
    pub total_annual_fees: Decimal,
    pub total_interest_earned: Decimal,
    pub total_interest_posted: Decimal,
    pub total_fee_charge: Decimal,
    pub total_penalty_charge: Decimal,
    pub total_fee_charges_waived: Decimal,
    pub total_penalty_charges_waived: Decimal,
    pub total_overdraft_interest_derived: Decimal,
    pub total_withhold_tax: Decimal,
    pub account_balance: Decimal,
    pub available_balance: Decimal,
    pub last_interest_calculation_date: Option<NaiveDate>,
    pub interest_posted_till_date: Option<NaiveDate>,
    pub running_balance_on_interest_posting_till_date: Decimal,
}

impl SavingsAccountSummary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recomputes summary totals from the account's transactions, charges, holds, and min balance settings.
    pub fn update_summary(
        &mut self,
        currency: MonetaryCurrency,
        transactions: &[SavingsAccountTransaction],
        charges: &[SavingsAccountCharge],
        savings_amount_on_hold: Decimal,
        on_hold_funds: Decimal,
        enforce_min_required_balance: bool,
        min_required_balance: Decimal,
    ) {
        let mut total_deposits = Money::zero(&currency);
        let mut total_withdrawals = Money::zero(&currency);
        let mut total_interest_posted = Money::zero(&currency);
        let mut total_withdrawal_fees = Money::zero(&currency);
        let mut total_annual_fees = Money::zero(&currency);
        let mut total_fee_charge = Money::zero(&currency);
        let mut total_penalty_charge = Money::zero(&currency);
        let mut total_fee_charges_waived = Money::zero(&currency);
        let mut total_penalty_charges_waived = Money::zero(&currency);
        let mut total_overdraft_interest = Money::zero(&currency);
        let mut total_withhold_tax = Money::zero(&currency);
        let mut total_written_off = Money::zero(&currency);

        let charge_penalty_lookup: BTreeMap<i64, bool> = charges
            .iter()
            .filter_map(|c| c.id.map(|id| (id, c.is_penalty)))
            .collect();

        for tx in transactions {
            if tx.is_reversed() || tx.is_reversal_transaction() {
                continue;
            }
            let tx_money = tx.get_amount(&currency);
            match tx.transaction_type {
                SavingsAccountTransactionType::Deposit
                | SavingsAccountTransactionType::DividendPayout
                | SavingsAccountTransactionType::ApproveTransfer
                | SavingsAccountTransactionType::WithdrawTransfer => {
                    total_deposits = total_deposits + tx_money;
                }
                SavingsAccountTransactionType::Withdrawal
                | SavingsAccountTransactionType::InitiateTransfer
                | SavingsAccountTransactionType::RejectTransfer
                | SavingsAccountTransactionType::Escheat => {
                    total_withdrawals = total_withdrawals + tx_money;
                }
                SavingsAccountTransactionType::InterestPosting => {
                    total_interest_posted = total_interest_posted + tx_money;
                }
                SavingsAccountTransactionType::WithdrawalFee => {
                    if Self::is_penalty_tx(tx, &charge_penalty_lookup) {
                        total_penalty_charge = total_penalty_charge + tx_money;
                    } else {
                        total_withdrawal_fees = total_withdrawal_fees + tx_money;
                    }
                }
                SavingsAccountTransactionType::AnnualFee => {
                    if Self::is_penalty_tx(tx, &charge_penalty_lookup) {
                        total_penalty_charge = total_penalty_charge + tx_money;
                    } else {
                        total_annual_fees = total_annual_fees + tx_money;
                    }
                }
                SavingsAccountTransactionType::PayCharge => {
                    if Self::is_penalty_tx(tx, &charge_penalty_lookup) {
                        total_penalty_charge = total_penalty_charge + tx_money;
                    } else {
                        total_fee_charge = total_fee_charge + tx_money;
                    }
                }
                SavingsAccountTransactionType::WaiveCharge => {
                    if Self::is_penalty_tx(tx, &charge_penalty_lookup) {
                        total_penalty_charges_waived = total_penalty_charges_waived + tx_money;
                    } else {
                        total_fee_charges_waived = total_fee_charges_waived + tx_money;
                    }
                }
                SavingsAccountTransactionType::OverdraftInterest => {
                    total_overdraft_interest = total_overdraft_interest + tx_money;
                }
                SavingsAccountTransactionType::WithholdTax => {
                    total_withhold_tax = total_withhold_tax + tx_money;
                }
                SavingsAccountTransactionType::WrittenOff => {
                    total_written_off = total_written_off + tx_money;
                }
                SavingsAccountTransactionType::Invalid
                | SavingsAccountTransactionType::Accrual
                | SavingsAccountTransactionType::AmountHold
                | SavingsAccountTransactionType::AmountRelease => {}
            }
        }

        self.total_deposits = total_deposits.amount();
        self.total_withdrawals = total_withdrawals.amount();
        self.total_interest_posted = total_interest_posted.amount();
        self.total_withdrawal_fees = total_withdrawal_fees.amount();
        self.total_annual_fees = total_annual_fees.amount();
        self.total_fee_charge = total_fee_charge.amount();
        self.total_penalty_charge = total_penalty_charge.amount();
        self.total_fee_charges_waived = total_fee_charges_waived.amount();
        self.total_penalty_charges_waived = total_penalty_charges_waived.amount();
        self.total_overdraft_interest_derived = total_overdraft_interest.amount();
        self.total_withhold_tax = total_withhold_tax.amount();

        let credits = total_deposits + total_interest_posted + total_written_off;
        let debits = total_withdrawals
            + total_withdrawal_fees
            + total_annual_fees
            + total_fee_charge
            + total_penalty_charge
            + total_overdraft_interest
            + total_withhold_tax;
        let account_balance = credits - debits;
        self.account_balance = account_balance.amount();

        let effective_min_balance = if enforce_min_required_balance {
            min_required_balance
        } else {
            Decimal::ZERO
        };

        let locked = Money::of(
            &currency,
            savings_amount_on_hold + on_hold_funds + effective_min_balance,
            FineractRoundingMode::HalfEven,
        );
        let available = account_balance - locked;
        self.available_balance = available.amount();
    }

    fn is_penalty_tx(
        tx: &SavingsAccountTransaction,
        charge_penalty_lookup: &BTreeMap<i64, bool>,
    ) -> bool {
        tx.charges_paid_by
            .first()
            .and_then(|cpb| charge_penalty_lookup.get(&cpb.savings_account_charge_id))
            .copied()
            .unwrap_or(false)
    }
}

// ============================================================================
// 7. SavingsAccount
// ============================================================================

/// Core Savings Account entity (`m_savings_account`, `org.apache.fineract.portfolio.savings.domain.SavingsAccount`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsAccount {
    pub id: Option<i64>,
    pub account_no: String,
    pub external_id: Option<String>,
    pub client_id: Option<i64>,
    pub group_id: Option<i64>,
    pub gsim_id: Option<i64>,
    pub product_id: i64,
    pub field_officer_id: Option<i64>,
    pub office_id: Option<i64>,
    pub status: SavingsAccountStatusType,
    pub sub_status: SavingsAccountSubStatusEnum,
    pub reason_for_block: Option<String>,
    pub account_type: i32,
    pub currency: MonetaryCurrency,
    pub nominal_annual_interest_rate: Decimal,
    pub interest_compounding_period_type: i32,
    pub interest_posting_period_type: i32,
    pub interest_calculation_type: i32,
    pub interest_calculation_days_in_year_type: i32,
    pub min_required_opening_balance: Option<Decimal>,
    pub lockin_period_frequency: Option<i32>,
    pub lockin_period_frequency_type: Option<i32>,
    pub lockedin_until_date_derived: Option<NaiveDate>,
    pub withdrawal_fee_for_transfer: bool,
    pub allow_overdraft: bool,
    pub overdraft_limit: Decimal,
    pub nominal_annual_interest_rate_overdraft: Decimal,
    pub min_overdraft_for_interest_calculation: Decimal,
    pub enforce_min_required_balance: bool,
    pub min_required_balance: Decimal,
    pub lien_allowed: bool,
    pub max_allowed_lien_limit: Decimal,
    pub on_hold_funds: Decimal,
    pub savings_amount_on_hold: Decimal,
    pub min_balance_for_interest_calculation: Option<Decimal>,
    pub withhold_tax: bool,
    pub tax_group_id: Option<i64>,
    pub start_interest_calculation_date: Option<NaiveDate>,
    // Dormancy tracking configuration
    pub is_dormancy_tracking_active: bool,
    pub days_to_inactive: Option<i32>,
    pub days_to_dormancy: Option<i32>,
    pub days_to_escheat: Option<i32>,
    // Lifecycle dates
    pub submitted_on_date: NaiveDate,
    pub submitted_by_user_id: Option<i64>,
    pub approved_on_date: Option<NaiveDate>,
    pub approved_by_user_id: Option<i64>,
    pub rejected_on_date: Option<NaiveDate>,
    pub rejected_by_user_id: Option<i64>,
    pub withdrawn_on_date: Option<NaiveDate>,
    pub withdrawn_by_user_id: Option<i64>,
    pub activated_on_date: Option<NaiveDate>,
    pub activated_by_user_id: Option<i64>,
    pub closed_on_date: Option<NaiveDate>,
    pub closed_by_user_id: Option<i64>,
    // Embedded summary & child collections
    pub summary: SavingsAccountSummary,
    pub transactions: Vec<SavingsAccountTransaction>,
    pub charges: Vec<SavingsAccountCharge>,
    next_local_tx_id: i64,
}

impl SavingsAccount {
    /// Creates a newly submitted savings account in `SubmittedAndPendingApproval` status.
    pub fn new_submitted(
        account_no: impl Into<String>,
        client_id: Option<i64>,
        group_id: Option<i64>,
        product_id: i64,
        currency: MonetaryCurrency,
        nominal_annual_interest_rate: Decimal,
        submitted_on_date: NaiveDate,
    ) -> Self {
        Self {
            id: None,
            account_no: account_no.into(),
            external_id: None,
            client_id,
            group_id,
            gsim_id: None,
            product_id,
            field_officer_id: None,
            office_id: None,
            status: SavingsAccountStatusType::SubmittedAndPendingApproval,
            sub_status: SavingsAccountSubStatusEnum::None,
            reason_for_block: None,
            account_type: 1,
            currency,
            nominal_annual_interest_rate,
            interest_compounding_period_type: 1, // Daily
            interest_posting_period_type: 4,     // Monthly
            interest_calculation_type: 1,        // Daily Balance
            interest_calculation_days_in_year_type: 365,
            min_required_opening_balance: None,
            lockin_period_frequency: None,
            lockin_period_frequency_type: None,
            lockedin_until_date_derived: None,
            withdrawal_fee_for_transfer: false,
            allow_overdraft: false,
            overdraft_limit: Decimal::ZERO,
            nominal_annual_interest_rate_overdraft: Decimal::ZERO,
            min_overdraft_for_interest_calculation: Decimal::ZERO,
            enforce_min_required_balance: false,
            min_required_balance: Decimal::ZERO,
            lien_allowed: false,
            max_allowed_lien_limit: Decimal::ZERO,
            on_hold_funds: Decimal::ZERO,
            savings_amount_on_hold: Decimal::ZERO,
            min_balance_for_interest_calculation: None,
            withhold_tax: false,
            tax_group_id: None,
            start_interest_calculation_date: None,
            is_dormancy_tracking_active: false,
            days_to_inactive: None,
            days_to_dormancy: None,
            days_to_escheat: None,
            submitted_on_date,
            submitted_by_user_id: None,
            approved_on_date: None,
            approved_by_user_id: None,
            rejected_on_date: None,
            rejected_by_user_id: None,
            withdrawn_on_date: None,
            withdrawn_by_user_id: None,
            activated_on_date: None,
            activated_by_user_id: None,
            closed_on_date: None,
            closed_by_user_id: None,
            summary: SavingsAccountSummary::new(),
            transactions: Vec::new(),
            charges: Vec::new(),
            next_local_tx_id: 1,
        }
    }

    // ------------------------------------------------------------------------
    // Status & Balance Query Helpers
    // ------------------------------------------------------------------------

    pub const fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub const fn is_not_active(&self) -> bool {
        !self.is_active()
    }

    pub const fn is_approved(&self) -> bool {
        self.status.is_approved()
    }

    pub const fn is_submitted_and_pending_approval(&self) -> bool {
        self.status.is_submitted_and_pending_approval()
    }

    pub const fn is_closed(&self) -> bool {
        self.status.is_closed()
    }

    /// Returns the account's current ledger balance (`summary.account_balance`).
    pub fn account_balance(&self) -> Decimal {
        self.summary.account_balance
    }

    /// Returns the account's current available balance (`account_balance - savings_amount_on_hold - on_hold_funds - min_required_balance`).
    pub fn available_balance(&self) -> Decimal {
        self.summary.available_balance
    }

    /// Effective minimum required balance enforced on the account.
    pub fn effective_min_required_balance(&self) -> Decimal {
        if self.enforce_min_required_balance {
            self.min_required_balance
        } else {
            Decimal::ZERO
        }
    }

    /// Effective overdraft limit available on the account.
    pub fn effective_overdraft_limit(&self) -> Decimal {
        if self.allow_overdraft || self.overdraft_limit > Decimal::ZERO {
            self.overdraft_limit.max(Decimal::ZERO)
        } else {
            Decimal::ZERO
        }
    }

    /// Returns the maximum amount that can currently be withdrawn or placed on hold:
    /// `account_balance - savings_amount_on_hold - on_hold_funds - effective_min_required_balance + effective_overdraft_limit`.
    pub fn withdrawable_balance(&self) -> Decimal {
        self.summary.account_balance
            - self.savings_amount_on_hold
            - self.on_hold_funds
            - self.effective_min_required_balance()
            + self.effective_overdraft_limit()
    }

    /// Adds a charge to the savings account and assigns it a local ID if none is set.
    pub fn add_charge(&mut self, mut charge: SavingsAccountCharge) -> i64 {
        let charge_id = charge.id.unwrap_or_else(|| {
            let next_id = self
                .charges
                .iter()
                .filter_map(|c| c.id)
                .max()
                .unwrap_or(0)
                + 1;
            charge.id = Some(next_id);
            next_id
        });
        charge.savings_account_id = self.id;
        self.charges.push(charge);
        charge_id
    }

    // ------------------------------------------------------------------------
    // Lifecycle State Transitions
    // ------------------------------------------------------------------------

    /// Transitions the savings account from `SubmittedAndPendingApproval` to `Approved`.
    pub fn approve(
        &mut self,
        approved_on_date: NaiveDate,
        approved_by_user_id: Option<i64>,
    ) -> Result<(), PlatformDomainRuleException> {
        if !self.status.is_submitted_and_pending_approval() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.approve.not.in.submittedandpendingapproval.state",
                "Savings account cannot be approved as it is not in submitted and pending approval state.",
                vec![self.status.code().into()],
            ));
        }
        if approved_on_date < self.submitted_on_date {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.approve.cannot.be.before.submitted.on.date",
                "The date on which a savings account is approved cannot be before its submission date.",
                vec![approved_on_date.to_string().into(), self.submitted_on_date.to_string().into()],
            ));
        }
        self.status = SavingsAccountStatusType::Approved;
        self.approved_on_date = Some(approved_on_date);
        self.approved_by_user_id = approved_by_user_id;
        Ok(())
    }

    /// Undoes approval, returning the account from `Approved` to `SubmittedAndPendingApproval`.
    pub fn undo_approval(&mut self) -> Result<(), PlatformDomainRuleException> {
        if !self.status.is_approved() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.undoapproval.not.in.approved.state",
                "Savings account undo approval is not allowed as the account is not in approved state.",
                vec![self.status.code().into()],
            ));
        }
        self.status = SavingsAccountStatusType::SubmittedAndPendingApproval;
        self.approved_on_date = None;
        self.approved_by_user_id = None;
        Ok(())
    }

    /// Rejects a savings account application (`SubmittedAndPendingApproval` -> `Rejected`).
    pub fn reject(
        &mut self,
        rejected_on_date: NaiveDate,
        rejected_by_user_id: Option<i64>,
    ) -> Result<(), PlatformDomainRuleException> {
        if !self.status.is_submitted_and_pending_approval() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.reject.not.in.submittedandpendingapproval.state",
                "Savings account cannot be rejected as it is not in submitted and pending approval state.",
                vec![self.status.code().into()],
            ));
        }
        if rejected_on_date < self.submitted_on_date {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.reject.cannot.be.before.submitted.on.date",
                "The date on which a savings account is rejected cannot be before its submission date.",
                vec![rejected_on_date.to_string().into(), self.submitted_on_date.to_string().into()],
            ));
        }
        self.status = SavingsAccountStatusType::Rejected;
        self.rejected_on_date = Some(rejected_on_date);
        self.rejected_by_user_id = rejected_by_user_id;
        self.closed_on_date = Some(rejected_on_date);
        self.closed_by_user_id = rejected_by_user_id;
        Ok(())
    }

    /// Withdraws a savings account application (`SubmittedAndPendingApproval` or `Approved` -> `WithdrawnByApplicant`).
    pub fn withdraw_application(
        &mut self,
        withdrawn_on_date: NaiveDate,
        withdrawn_by_user_id: Option<i64>,
    ) -> Result<(), PlatformDomainRuleException> {
        if !self.status.is_submitted_and_pending_approval() && !self.status.is_approved() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.withdraw.not.in.submittedandpendingapproval.or.approved.state",
                "Savings account application cannot be withdrawn as it is not in submitted and pending approval or approved state.",
                vec![self.status.code().into()],
            ));
        }
        if withdrawn_on_date < self.submitted_on_date {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.withdraw.cannot.be.before.submitted.on.date",
                "The date on which a savings account application is withdrawn cannot be before its submission date.",
                vec![withdrawn_on_date.to_string().into(), self.submitted_on_date.to_string().into()],
            ));
        }
        self.status = SavingsAccountStatusType::WithdrawnByApplicant;
        self.withdrawn_on_date = Some(withdrawn_on_date);
        self.withdrawn_by_user_id = withdrawn_by_user_id;
        self.closed_on_date = Some(withdrawn_on_date);
        self.closed_by_user_id = withdrawn_by_user_id;
        Ok(())
    }

    /// Activates an approved savings account (`Approved` -> `Active`).
    /// Processes any `SavingsActivation` charges and an optional `min_required_opening_balance` deposit.
    pub fn activate(
        &mut self,
        activated_on_date: NaiveDate,
        activated_by_user_id: Option<i64>,
    ) -> Result<(), PlatformDomainRuleException> {
        if !self.status.is_approved() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.activate.not.in.approved.state",
                "Savings account cannot be activated as it is not in approved state.",
                vec![self.status.code().into()],
            ));
        }
        if let Some(approved_date) = self.approved_on_date {
            if activated_on_date < approved_date {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.activate.cannot.be.before.approved.on.date",
                    "The date on which a savings account is activated cannot be before its approval date.",
                    vec![activated_on_date.to_string().into(), approved_date.to_string().into()],
                ));
            }
        }
        if activated_on_date < self.submitted_on_date {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.activate.cannot.be.before.submitted.on.date",
                "The date on which a savings account is activated cannot be before its submission date.",
                vec![activated_on_date.to_string().into(), self.submitted_on_date.to_string().into()],
            ));
        }

        self.status = SavingsAccountStatusType::Active;
        self.activated_on_date = Some(activated_on_date);
        self.activated_by_user_id = activated_by_user_id;
        if self.start_interest_calculation_date.is_none() {
            self.start_interest_calculation_date = Some(activated_on_date);
        }

        // If min_required_opening_balance > 0 and no deposit exists yet, create the opening deposit
        if let Some(opening_balance) = self.min_required_opening_balance {
            if opening_balance > Decimal::ZERO && self.transactions.is_empty() {
                let money = Money::of(self.currency.clone(), opening_balance, FineractRoundingMode::HalfEven);
                let mut tx = SavingsAccountTransaction::deposit(
                    self.id,
                    self.office_id,
                    None,
                    activated_on_date,
                    money,
                    activated_on_date,
                );
                tx.id = Some(self.allocate_tx_id());
                self.transactions.push(tx);
            }
        }

        // Settle any active SavingsActivation charges
        let activation_charges: Vec<(i64, Decimal)> = self
            .charges
            .iter_mut()
            .filter(|c| c.is_active() && c.is_savings_activation() && c.is_not_fully_paid())
            .filter_map(|c| {
                let cid = c.id?;
                let amt = c.amount_outstanding;
                if amt > Decimal::ZERO {
                    let money = Money::of(c.currency.clone(), amt, FineractRoundingMode::HalfEven);
                    c.mark_as_paid(money);
                    Some((cid, amt))
                } else {
                    None
                }
            })
            .collect();

        for (cid, amt) in activation_charges {
            let money = Money::of(self.currency.clone(), amt, FineractRoundingMode::HalfEven);
            let mut tx = SavingsAccountTransaction::pay_charge(
                self.id,
                self.office_id,
                None,
                SavingsAccountTransactionType::PayCharge,
                activated_on_date,
                money,
                activated_on_date,
                cid,
            );
            tx.id = Some(self.allocate_tx_id());
            self.transactions.push(tx);
        }

        self.recalculate_daily_balances(activated_on_date);
        Ok(())
    }

    /// Closes an active savings account (`Active` -> `Closed` or `PrematureClosed`).
    pub fn close(
        &mut self,
        closed_on_date: NaiveDate,
        closed_by_user_id: Option<i64>,
        premature_closure: bool,
    ) -> Result<(), PlatformDomainRuleException> {
        if !self.status.is_active() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.close.not.in.active.state",
                "Savings account cannot be closed as it is not in active state.",
                vec![self.status.code().into()],
            ));
        }
        if self.sub_status.is_sub_status_account_blocked()
            || self.sub_status.is_sub_status_credit_blocked()
            || self.sub_status.is_sub_status_debit_blocked()
        {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.close.account.is.blocked",
                "Savings account cannot be closed while blocked.",
                vec![self.sub_status.code().into()],
            ));
        }
        if !self.savings_amount_on_hold.is_zero() || !self.on_hold_funds.is_zero() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.close.amount.on.hold",
                "Savings account cannot be closed while funds are on hold.",
                vec![self.savings_amount_on_hold.to_string().into()],
            ));
        }
        if let Some(activated_date) = self.activated_on_date {
            if closed_on_date < activated_date {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.close.cannot.be.before.activated.on.date",
                    "The date on which a savings account is closed cannot be before its activation date.",
                    vec![closed_on_date.to_string().into(), activated_date.to_string().into()],
                ));
            }
        }
        if let Some(last_tx_date) = self.retrieve_last_transaction_date() {
            if closed_on_date < last_tx_date {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.close.cannot.be.before.last.transaction.date",
                    "The closing date cannot be earlier than the last transaction date.",
                    vec![closed_on_date.to_string().into(), last_tx_date.to_string().into()],
                ));
            }
        }

        // Inactivate any recurring/outstanding charges upon closure
        for charge in &mut self.charges {
            if charge.is_active() {
                charge.inactivate(closed_on_date);
            }
        }

        self.status = if premature_closure {
            SavingsAccountStatusType::PrematureClosed
        } else {
            SavingsAccountStatusType::Closed
        };
        self.closed_on_date = Some(closed_on_date);
        self.closed_by_user_id = closed_by_user_id;
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Block / Unblock / Dormancy Sub-Status Transitions
    // ------------------------------------------------------------------------

    /// Blocks both credits and debits on an active savings account (`sub_status = Block`).
    pub fn block(
        &mut self,
        reason_for_block: Option<String>,
    ) -> Result<(), PlatformDomainRuleException> {
        self.ensure_active_for_sub_status_change("block")?;
        if self.sub_status.is_sub_status_account_blocked() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.block.already.blocked",
                "Savings account is already blocked.",
                vec![],
            ));
        }
        if let Some(ref reason) = reason_for_block {
            if reason.trim().is_empty() {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.block.reason.cannot.be.blank",
                    "Reason for blocking savings account cannot be blank.",
                    vec![],
                ));
            }
        }
        self.sub_status = SavingsAccountSubStatusEnum::Block;
        self.reason_for_block = reason_for_block;
        Ok(())
    }

    /// Unblocks an account that is in `Block` sub-status (`sub_status = None`).
    pub fn unblock(&mut self) -> Result<(), PlatformDomainRuleException> {
        self.ensure_active_for_sub_status_change("unblock")?;
        if !self.sub_status.is_sub_status_account_blocked() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.unblock.not.in.block.state",
                "Savings account cannot be unblocked as it is not in blocked state.",
                vec![self.sub_status.code().into()],
            ));
        }
        self.sub_status = SavingsAccountSubStatusEnum::None;
        self.reason_for_block = None;
        Ok(())
    }

    /// Blocks credit transactions on an active savings account.
    /// If debits are already blocked (`BlockDebit`), transitions to full `Block`.
    pub fn block_credit(
        &mut self,
        reason_for_block: Option<String>,
    ) -> Result<(), PlatformDomainRuleException> {
        self.ensure_active_for_sub_status_change("blockCredit")?;
        if self.sub_status.is_sub_status_account_blocked()
            || self.sub_status.is_sub_status_credit_blocked()
        {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.blockCredit.already.blocked",
                "Credit transactions on this savings account are already blocked.",
                vec![self.sub_status.code().into()],
            ));
        }
        self.sub_status = if self.sub_status.is_sub_status_debit_blocked() {
            SavingsAccountSubStatusEnum::Block
        } else {
            SavingsAccountSubStatusEnum::BlockCredit
        };
        if reason_for_block.is_some() {
            self.reason_for_block = reason_for_block;
        }
        Ok(())
    }

    /// Unblocks credit transactions on an active savings account.
    /// If full `Block` is active, transitions to `BlockDebit`; if `BlockCredit`, transitions to `None`.
    pub fn unblock_credit(&mut self) -> Result<(), PlatformDomainRuleException> {
        self.ensure_active_for_sub_status_change("unblockCredit")?;
        if self.sub_status.is_sub_status_account_blocked() {
            self.sub_status = SavingsAccountSubStatusEnum::BlockDebit;
            Ok(())
        } else if self.sub_status.is_sub_status_credit_blocked() {
            self.sub_status = SavingsAccountSubStatusEnum::None;
            self.reason_for_block = None;
            Ok(())
        } else {
            Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.unblockCredit.not.blocked",
                "Savings account credits are not currently blocked.",
                vec![self.sub_status.code().into()],
            ))
        }
    }

    /// Blocks debit transactions on an active savings account.
    /// If credits are already blocked (`BlockCredit`), transitions to full `Block`.
    pub fn block_debit(
        &mut self,
        reason_for_block: Option<String>,
    ) -> Result<(), PlatformDomainRuleException> {
        self.ensure_active_for_sub_status_change("blockDebit")?;
        if self.sub_status.is_sub_status_account_blocked()
            || self.sub_status.is_sub_status_debit_blocked()
        {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.blockDebit.already.blocked",
                "Debit transactions on this savings account are already blocked.",
                vec![self.sub_status.code().into()],
            ));
        }
        self.sub_status = if self.sub_status.is_sub_status_credit_blocked() {
            SavingsAccountSubStatusEnum::Block
        } else {
            SavingsAccountSubStatusEnum::BlockDebit
        };
        if reason_for_block.is_some() {
            self.reason_for_block = reason_for_block;
        }
        Ok(())
    }

    /// Unblocks debit transactions on an active savings account.
    /// If full `Block` is active, transitions to `BlockCredit`; if `BlockDebit`, transitions to `None`.
    pub fn unblock_debit(&mut self) -> Result<(), PlatformDomainRuleException> {
        self.ensure_active_for_sub_status_change("unblockDebit")?;
        if self.sub_status.is_sub_status_account_blocked() {
            self.sub_status = SavingsAccountSubStatusEnum::BlockCredit;
            Ok(())
        } else if self.sub_status.is_sub_status_debit_blocked() {
            self.sub_status = SavingsAccountSubStatusEnum::None;
            self.reason_for_block = None;
            Ok(())
        } else {
            Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.unblockDebit.not.blocked",
                "Savings account debits are not currently blocked.",
                vec![self.sub_status.code().into()],
            ))
        }
    }

    /// Transitions dormancy sub-status (`None` -> `Inactive` -> `Dormant` -> `Escheat`).
    /// When transitioning to `Inactive`, applies any active `SavingsNoActivityFee` charges.
    /// When transitioning to `Escheat`, posts an `Escheat` debit transaction for any positive balance and closes the account.
    pub fn transition_dormancy_state(
        &mut self,
        target_sub_status: SavingsAccountSubStatusEnum,
        transition_date: NaiveDate,
    ) -> Result<Option<SavingsAccountTransaction>, PlatformDomainRuleException> {
        self.ensure_active_for_sub_status_change("dormancy")?;
        match target_sub_status {
            SavingsAccountSubStatusEnum::None => {
                self.sub_status = SavingsAccountSubStatusEnum::None;
                Ok(None)
            }
            SavingsAccountSubStatusEnum::Inactive => {
                if !self.sub_status.is_sub_status_none() {
                    return Err(PlatformDomainRuleException::with_args(
                        "error.msg.savingsaccount.dormancy.invalid.transition.to.inactive",
                        "Savings account can only transition to Inactive from None sub-status.",
                        vec![self.sub_status.code().into()],
                    ));
                }
                self.sub_status = SavingsAccountSubStatusEnum::Inactive;
                self.apply_no_activity_fee_charges(transition_date);
                Ok(None)
            }
            SavingsAccountSubStatusEnum::Dormant => {
                if !self.sub_status.is_sub_status_inactive() && !self.sub_status.is_sub_status_none()
                {
                    return Err(PlatformDomainRuleException::with_args(
                        "error.msg.savingsaccount.dormancy.invalid.transition.to.dormant",
                        "Savings account can only transition to Dormant from Inactive or None sub-status.",
                        vec![self.sub_status.code().into()],
                    ));
                }
                self.sub_status = SavingsAccountSubStatusEnum::Dormant;
                Ok(None)
            }
            SavingsAccountSubStatusEnum::Escheat => {
                if !self.sub_status.is_sub_status_dormant() {
                    return Err(PlatformDomainRuleException::with_args(
                        "error.msg.savingsaccount.dormancy.invalid.transition.to.escheat",
                        "Savings account can only transition to Escheat from Dormant sub-status.",
                        vec![self.sub_status.code().into()],
                    ));
                }
                self.sub_status = SavingsAccountSubStatusEnum::Escheat;
                self.status = SavingsAccountStatusType::Closed;
                self.closed_on_date = Some(transition_date);

                for charge in &mut self.charges {
                    if charge.is_active() {
                        charge.inactivate(transition_date);
                    }
                }

                let mut escheat_tx = None;
                if self.summary.account_balance > Decimal::ZERO {
                    let money = Money::of(self.currency.clone(), self.summary.account_balance, FineractRoundingMode::HalfEven);
                    let mut tx = SavingsAccountTransaction::escheat(
                        self.id,
                        self.office_id,
                        transition_date,
                        money,
                        transition_date,
                    );
                    tx.id = Some(self.allocate_tx_id());
                    self.transactions.push(tx.clone());
                    self.recalculate_daily_balances(transition_date);
                    if let Some(updated) = self.transactions.iter().find(|t| t.id == tx.id) {
                        escheat_tx = Some(updated.clone());
                    }
                }
                Ok(escheat_tx)
            }
            _ => Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.dormancy.unsupported.substatus",
                "Unsupported dormancy target sub-status.",
                vec![target_sub_status.code().into()],
            )),
        }
    }

    fn apply_no_activity_fee_charges(&mut self, transaction_date: NaiveDate) {
        let no_activity_charges: Vec<(i64, Decimal)> = self
            .charges
            .iter_mut()
            .filter(|c| c.is_active() && c.is_savings_no_activity_fee())
            .filter_map(|c| {
                let cid = c.id?;
                let amt = c.amount;
                if amt > Decimal::ZERO && self.summary.account_balance >= amt {
                    let money = Money::of(c.currency.clone(), amt, FineractRoundingMode::HalfEven);
                    c.update_with_dral_fee_amount(amt);
                    c.mark_as_paid(money);
                    Some((cid, amt))
                } else {
                    None
                }
            })
            .collect();

        if !no_activity_charges.is_empty() {
            for (cid, amt) in no_activity_charges {
                let money = Money::of(self.currency.clone(), amt, FineractRoundingMode::HalfEven);
                let mut tx = SavingsAccountTransaction::pay_charge(
                    self.id,
                    self.office_id,
                    None,
                    SavingsAccountTransactionType::PayCharge,
                    transaction_date,
                    money,
                    transaction_date,
                    cid,
                );
                tx.id = Some(self.allocate_tx_id());
                self.transactions.push(tx);
            }
            self.recalculate_daily_balances(transaction_date);
        }
    }

    // ------------------------------------------------------------------------
    // Transaction Mutations: Deposit, Withdraw, Hold, Release, Reverse
    // ------------------------------------------------------------------------

    /// Credits the savings account with a deposit (`SavingsAccountTransactionType::Deposit`).
    /// Validates active status, credit-block rules, and activation date bounds, resets dormancy if inactive/dormant,
    /// and recalculates chronological running balances.
    pub fn deposit(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
        payment_detail_id: Option<i64>,
        submitted_on_date: NaiveDate,
    ) -> Result<SavingsAccountTransaction, PlatformDomainRuleException> {
        self.validate_for_credit_transaction(transaction_date, amount)?;

        let money = Money::of(self.currency.clone(), amount, FineractRoundingMode::HalfEven);
        let tx_id = self.allocate_tx_id();
        let mut tx = SavingsAccountTransaction::deposit(
            self.id,
            self.office_id,
            payment_detail_id,
            transaction_date,
            money,
            submitted_on_date,
        );
        tx.id = Some(tx_id);

        if self.sub_status.is_sub_status_inactive() || self.sub_status.is_sub_status_dormant() {
            self.sub_status = SavingsAccountSubStatusEnum::None;
        }

        self.transactions.push(tx);
        self.recalculate_daily_balances(submitted_on_date);

        // Ensure backdated insertion didn't cause a historical minimum-balance violation
        self.validate_chronological_balances_after_mutation(transaction_date, tx_id)?;

        Ok(self.find_transaction_by_id(tx_id).unwrap().clone())
    }

    /// Debits the savings account with a withdrawal (`SavingsAccountTransactionType::Withdrawal`)
    /// and automatically applies any active `WithdrawalFee` charges (`SavingsAccountTransactionType::WithdrawalFee`).
    /// Enforces `withdrawable_balance = account_balance - savings_amount_on_hold - min_required_balance + overdraft_limit`.
    pub fn withdraw(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
        payment_detail_id: Option<i64>,
        submitted_on_date: NaiveDate,
        is_account_transfer: bool,
    ) -> Result<SavingsAccountTransaction, PlatformDomainRuleException> {
        self.validate_for_debit_transaction(transaction_date, amount)?;
        self.validate_lockin_period(transaction_date)?;

        let withdrawal_money = Money::of(self.currency.clone(), amount, FineractRoundingMode::HalfEven);
        let should_collect_withdrawal_fee =
            !is_account_transfer || self.withdrawal_fee_for_transfer;

        let pending_withdrawal_fees: Vec<(usize, Option<i64>, Money)> =
            if should_collect_withdrawal_fee {
                self.charges
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.is_active() && c.is_withdrawal_fee())
                    .map(|(idx, c)| {
                        let fee_money =
                            c.calculate_fee_for_transaction_amount(withdrawal_money.amount());
                        (idx, c.id, fee_money)
                    })
                    .filter(|(_, _, fee)| fee.is_greater_than_zero())
                    .collect()
            } else {
                Vec::new()
            };

        let total_fee_amount: Decimal = pending_withdrawal_fees
            .iter()
            .map(|(_, _, fee)| fee.amount())
            .sum();
        let total_debit = withdrawal_money.amount() + total_fee_amount;

        // Validate against current withdrawable balance (takes into account holds, min required balance, and overdraft limit)
        let current_withdrawable = self.withdrawable_balance();
        if total_debit > current_withdrawable {
            return Err(Self::insufficient_balance_exception(
                withdrawal_money.amount(),
                total_fee_amount,
                current_withdrawable,
            ));
        }

        let snapshot_transactions = self.transactions.clone();
        let snapshot_charges = self.charges.clone();
        let snapshot_summary = self.summary.clone();
        let snapshot_sub_status = self.sub_status;

        if self.sub_status.is_sub_status_inactive() || self.sub_status.is_sub_status_dormant() {
            self.sub_status = SavingsAccountSubStatusEnum::None;
        }

        let withdrawal_tx_id = self.allocate_tx_id();
        let mut withdrawal_tx = SavingsAccountTransaction::withdrawal(
            self.id,
            self.office_id,
            payment_detail_id,
            transaction_date,
            withdrawal_money.clone(),
            submitted_on_date,
        );
        withdrawal_tx.id = Some(withdrawal_tx_id);
        self.transactions.push(withdrawal_tx);

        for (charge_idx, charge_id, fee_money) in pending_withdrawal_fees {
            if let Some(charge) = self.charges.get_mut(charge_idx) {
                charge.update_with_dral_fee_amount(withdrawal_money.amount());
                charge.mark_as_paid(fee_money.clone());
            }
            let fee_tx_id = self.allocate_tx_id();
            let mut fee_tx = SavingsAccountTransaction::withdrawal_fee(
                self.id,
                self.office_id,
                transaction_date,
                fee_money,
                submitted_on_date,
                charge_id,
            );
            fee_tx.id = Some(fee_tx_id);
            for cpb in &mut fee_tx.charges_paid_by {
                cpb.savings_account_transaction_id = Some(fee_tx_id);
            }
            self.transactions.push(fee_tx);
        }

        self.recalculate_daily_balances(submitted_on_date);

        // Verify that chronological running balances from transaction_date onward never violate
        // minimum balance / overdraft rules (important for backdated withdrawals).
        if let Err(err) =
            self.validate_chronological_balances_from_date(transaction_date, withdrawal_money.amount(), total_fee_amount)
        {
            self.transactions = snapshot_transactions;
            self.charges = snapshot_charges;
            self.summary = snapshot_summary;
            self.sub_status = snapshot_sub_status;
            return Err(err);
        }

        Ok(self
            .find_transaction_by_id(withdrawal_tx_id)
            .unwrap()
            .clone())
    }

    /// Places an amount on hold (`SavingsAccountTransactionType::AmountHold`), locking it from
    /// `available_balance` and `withdrawable_balance` without changing `account_balance`.
    pub fn hold_amount(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
        payment_detail_id: Option<i64>,
        submitted_on_date: NaiveDate,
        lien_transaction: bool,
        reason_for_block: Option<String>,
    ) -> Result<SavingsAccountTransaction, PlatformDomainRuleException> {
        self.validate_for_debit_transaction(transaction_date, amount)?;

        let hold_money = Money::of(self.currency.clone(), amount, FineractRoundingMode::HalfEven);
        if lien_transaction {
            if !self.lien_allowed {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.lien.is.not.allowed",
                    "Lien is not allowed on this savings account.",
                    vec![],
                ));
            }
            let max_lien_available = self.withdrawable_balance() + self.max_allowed_lien_limit;
            if hold_money.amount() > max_lien_available {
                return Err(Self::insufficient_balance_exception(
                    hold_money.amount(),
                    Decimal::ZERO,
                    max_lien_available,
                ));
            }
        } else {
            let current_withdrawable = self.withdrawable_balance();
            if hold_money.amount() > current_withdrawable {
                return Err(Self::insufficient_balance_exception(
                    hold_money.amount(),
                    Decimal::ZERO,
                    current_withdrawable,
                ));
            }
        }

        self.savings_amount_on_hold += hold_money.amount();
        let tx_id = self.allocate_tx_id();
        let mut hold_tx = SavingsAccountTransaction::hold_amount(
            self.id,
            self.office_id,
            payment_detail_id,
            transaction_date,
            hold_money,
            submitted_on_date,
            lien_transaction,
            reason_for_block,
        );
        hold_tx.id = Some(tx_id);
        hold_tx.running_balance = self.summary.account_balance;

        self.transactions.push(hold_tx);
        self.recalculate_daily_balances(submitted_on_date);

        Ok(self.find_transaction_by_id(tx_id).unwrap().clone())
    }

    /// Releases a previously held amount (`SavingsAccountTransactionType::AmountRelease`),
    /// linking `release_id_of_hold_amount` on the original `AmountHold` transaction and restoring
    /// `savings_amount_on_hold` and `available_balance`.
    pub fn release_amount(
        &mut self,
        hold_transaction_id: i64,
        release_date: NaiveDate,
        submitted_on_date: NaiveDate,
    ) -> Result<SavingsAccountTransaction, PlatformDomainRuleException> {
        if !self.is_active() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.releaseAmount.not.active",
                "Transaction is not allowed. Account is not active.",
                vec![self.status.code().into()],
            ));
        }

        let hold_idx = self
            .transactions
            .iter()
            .position(|t| t.id == Some(hold_transaction_id))
            .ok_or_else(|| {
                PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.transaction.not.found",
                    "Savings account hold transaction was not found.",
                    vec![hold_transaction_id.into()],
                )
            })?;

        let hold_tx = self.transactions[hold_idx].clone();
        if !hold_tx.is_amount_on_hold() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.release.transaction.is.not.hold",
                "Cannot release a transaction that is not an amount-on-hold transaction.",
                vec![hold_transaction_id.into()],
            ));
        }
        if hold_tx.is_reversed() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.release.transaction.is.reversed",
                "Cannot release an amount-on-hold transaction that has been reversed.",
                vec![hold_transaction_id.into()],
            ));
        }
        if hold_tx.release_id_of_hold_amount.is_some() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.hold.transaction.already.released",
                "Hold amount on this transaction has already been released.",
                vec![hold_transaction_id.into()],
            ));
        }
        if release_date < hold_tx.date_of {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.release.date.cannot.be.before.hold.date",
                "Release date cannot be before the hold transaction date.",
                vec![release_date.to_string().into(), hold_tx.date_of.to_string().into()],
            ));
        }

        let release_tx_id = self.allocate_tx_id();
        let mut release_tx =
            SavingsAccountTransaction::release_amount(&hold_tx, release_date, submitted_on_date);
        release_tx.id = Some(release_tx_id);

        let held_amount = hold_tx.amount;
        self.transactions[hold_idx].release_id_of_hold_amount = Some(release_tx_id);
        self.savings_amount_on_hold = (self.savings_amount_on_hold - held_amount).max(Decimal::ZERO);

        self.transactions.push(release_tx);
        self.recalculate_daily_balances(submitted_on_date);

        Ok(self.find_transaction_by_id(release_tx_id).unwrap().clone())
    }

    /// Reverses an existing transaction by ID, undoing any associated charge payments or holds,
    /// recalculating chronological running balances, and verifying that reversing a credit
    /// does not violate minimum balance or overdraft constraints.
    pub fn reverse_transaction(
        &mut self,
        transaction_id: i64,
        reversal_date: NaiveDate,
    ) -> Result<SavingsAccountTransaction, PlatformDomainRuleException> {
        if !self.is_active() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.reverse.not.active",
                "Transaction reversal is not allowed when the savings account is not active.",
                vec![self.status.code().into()],
            ));
        }

        let tx_idx = self
            .transactions
            .iter()
            .position(|t| t.id == Some(transaction_id))
            .ok_or_else(|| {
                PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.transaction.not.found",
                    "Savings account transaction with the given identifier does not exist.",
                    vec![transaction_id.into()],
                )
            })?;

        if self.transactions[tx_idx].is_reversed() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.already.reversed",
                "Savings account transaction is already reversed.",
                vec![transaction_id.into()],
            ));
        }

        let snapshot_transactions = self.transactions.clone();
        let snapshot_charges = self.charges.clone();
        let snapshot_summary = self.summary.clone();
        let snapshot_hold = self.savings_amount_on_hold;

        let tx_date = self.transactions[tx_idx].date_of;
        let tx_type = self.transactions[tx_idx].transaction_type;
        let tx_amount = self.transactions[tx_idx].amount;
        let release_id = self.transactions[tx_idx].release_id_of_hold_amount;
        let charges_paid = self.transactions[tx_idx].charges_paid_by.clone();

        self.transactions[tx_idx].reverse();

        // If reversing an unreleased AmountHold, reduce savings_amount_on_hold
        if tx_type.is_amount_on_hold() && release_id.is_none() {
            self.savings_amount_on_hold =
                (self.savings_amount_on_hold - tx_amount).max(Decimal::ZERO);
        } else if tx_type.is_amount_release() {
            // If reversing an AmountRelease, re-lock the linked AmountHold
            for hold_tx in &mut self.transactions {
                if hold_tx.release_id_of_hold_amount == Some(transaction_id) {
                    hold_tx.release_id_of_hold_amount = None;
                    self.savings_amount_on_hold += tx_amount;
                    break;
                }
            }
        }

        // If the reversed transaction paid or waived a charge, undo the charge payment
        for cpb in charges_paid {
            if let Some(charge) = self
                .charges
                .iter_mut()
                .find(|c| c.id == Some(cpb.savings_account_charge_id))
            {
                charge.undo_payment(cpb.amount);
            }
        }

        self.recalculate_daily_balances(reversal_date);

        // Verify that reversing a credit did not cause running balances to drop below allowed floor
        if let Err(err) =
            self.validate_chronological_balances_from_date(tx_date, tx_amount, Decimal::ZERO)
        {
            self.transactions = snapshot_transactions;
            self.charges = snapshot_charges;
            self.summary = snapshot_summary;
            self.savings_amount_on_hold = snapshot_hold;
            return Err(err);
        }

        Ok(self.transactions[tx_idx].clone())
    }

    // ------------------------------------------------------------------------
    // Running Balance & Summary Recalculation Engine
    // ------------------------------------------------------------------------

    /// Sorts transactions chronologically, updates every transaction's `running_balance`,
    /// `overdraft_amount`, `balance_end_date`, `balance_number_of_days`, and `cumulative_balance`,
    /// and refreshes `self.summary`.
    /// Mirrors `SavingsAccount.recalculateDailyBalances` in Apache Fineract.

    pub fn post_interest_amount(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
        submitted_on_date: NaiveDate,
        _is_post_interest_as_on: bool,
        is_manual: bool,
    ) -> Result<SavingsAccountTransaction, PlatformDomainRuleException> {
        let money = Money::of(self.currency.clone(), amount, FineractRoundingMode::HalfEven);
        let mut tx = SavingsAccountTransaction::interest_posting(
            self.id,
            self.office_id,
            transaction_date,
            money,
            submitted_on_date,
            is_manual,
        );
        tx.id = Some(self.allocate_tx_id());
        self.transactions.push(tx.clone());
        Ok(tx)
    }

    pub fn post_overdraft_interest_amount(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
        submitted_on_date: NaiveDate,
        _is_post_interest_as_on: bool,
        is_manual: bool,
    ) -> Result<SavingsAccountTransaction, PlatformDomainRuleException> {
        let money = Money::of(self.currency.clone(), amount, FineractRoundingMode::HalfEven);
        let mut tx = SavingsAccountTransaction::overdraft_interest(
            self.id,
            self.office_id,
            transaction_date,
            money,
            submitted_on_date,
            is_manual,
        );
        tx.id = Some(self.allocate_tx_id());
        self.transactions.push(tx.clone());
        Ok(tx)
    }

    pub fn recalculate_daily_balances(&mut self, reference_date: NaiveDate) {
        self.transactions.sort_by(compare_savings_transactions);

        let currency = self.currency.clone();
        let mut running_balance = Money::zero(currency.clone());

        // Pass 1: Zero reversed transactions and compute running_balance & overdraft_amount
        for tx in &mut self.transactions {
            if tx.is_reversed() || tx.is_reversal_transaction() {
                tx.zero_balance_fields();
                tx.update_overdraft_amount(None);
                continue;
            }

            let tx_amount = tx.get_amount(currency.clone());
            let mut overdraft_amount: Option<Money> = None;

            if tx.is_credit() {
                if running_balance.is_less_than_zero() {
                    let diff = running_balance.clone() + tx_amount.clone();
                    if diff.is_less_than_zero() {
                        overdraft_amount = Some(tx_amount.clone());
                    } else {
                        overdraft_amount = Some(running_balance.negated());
                    }
                }
                running_balance = running_balance + tx_amount;
            } else if tx.is_debit() {
                if running_balance.is_less_than_zero() {
                    overdraft_amount = Some(tx_amount.clone());
                }
                running_balance = running_balance - tx_amount;
                if overdraft_amount.is_none() && running_balance.is_less_than_zero() {
                    overdraft_amount = Some(running_balance.negated());
                }
            }

            tx.update_running_balance(running_balance.clone());
            tx.update_overdraft_amount(overdraft_amount.map(|m| m.amount()));
        }

        // Pass 2: Compute balance_end_date, balance_number_of_days, and cumulative_balance
        // In Fineract, the last active transaction on a given date holds the end-of-day balance
        // until the next active transaction's date (or reference_date for the final transaction).
        let active_indices: Vec<usize> = self
            .transactions
            .iter()
            .enumerate()
            .filter(|(_, tx)| !tx.is_reversed() && !tx.is_reversal_transaction())
            .map(|(i, _)| i)
            .collect();

        for (pos, &tx_idx) in active_indices.iter().enumerate() {
            let tx_date = self.transactions[tx_idx].date_of;
            let next_tx_date = active_indices
                .get(pos + 1)
                .map(|&next_idx| self.transactions[next_idx].date_of);

            let end_date = match next_tx_date {
                Some(next_date) => Some(next_date),
                None => Some(reference_date.max(tx_date)),
            };

            self.transactions[tx_idx].update_cumulative_balance_and_dates(currency.clone(), end_date);
        }

        self.summary.update_summary(
            self.currency.clone(),
            &self.transactions,
            &self.charges,
            self.savings_amount_on_hold,
            self.on_hold_funds,
            self.enforce_min_required_balance,
            self.min_required_balance,
        );
    }

    /// Alias for `recalculate_daily_balances` for callers using the `recalculate_running_balances` name.
    pub fn recalculate_running_balances(&mut self, reference_date: NaiveDate) {
        self.recalculate_daily_balances(reference_date);
    }

    // ------------------------------------------------------------------------
    // Internal Validation Helpers
    // ------------------------------------------------------------------------

    fn ensure_active_for_sub_status_change(
        &self,
        action: &str,
    ) -> Result<(), PlatformDomainRuleException> {
        if !self.is_active() {
            return Err(PlatformDomainRuleException::with_args(
                format!("error.msg.savingsaccount.{action}.not.in.active.state"),
                format!("Savings account {action} is not allowed because the account is not active."),
                vec![self.status.code().into()],
            ));
        }
        Ok(())
    }

    fn validate_for_credit_transaction(
        &self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<(), PlatformDomainRuleException> {
        if !self.is_active() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.account.is.not.active",
                "Transaction is not allowed. Account is not active.",
                vec![self.status.code().into()],
            ));
        }
        if self.sub_status.is_sub_status_account_blocked() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.account.is.blocked",
                "Transaction is not allowed. Account is blocked.",
                vec![self.id.unwrap_or(0).into()],
            ));
        }
        if self.sub_status.is_sub_status_credit_blocked() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.credits.are.blocked",
                "Credit transactions are blocked on this savings account.",
                vec![self.id.unwrap_or(0).into()],
            ));
        }
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.amount.must.be.greater.than.zero",
                "Transaction amount must be greater than zero.",
                vec![amount.to_string().into()],
            ));
        }
        if let Some(activated_date) = self.activated_on_date {
            if transaction_date < activated_date {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.transaction.before.activation.date",
                    "Transaction date cannot be before the account activation date.",
                    vec![
                        transaction_date.to_string().into(),
                        activated_date.to_string().into(),
                    ],
                ));
            }
        }
        Ok(())
    }

    fn validate_for_debit_transaction(
        &self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<(), PlatformDomainRuleException> {
        if !self.is_active() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.account.is.not.active",
                "Transaction is not allowed. Account is not active.",
                vec![self.status.code().into()],
            ));
        }
        if self.sub_status.is_sub_status_account_blocked() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.account.is.blocked",
                "Transaction is not allowed. Account is blocked.",
                vec![self.id.unwrap_or(0).into()],
            ));
        }
        if self.sub_status.is_sub_status_debit_blocked() {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.debits.are.blocked",
                "Debit transactions are blocked on this savings account.",
                vec![self.id.unwrap_or(0).into()],
            ));
        }
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.savingsaccount.transaction.amount.must.be.greater.than.zero",
                "Transaction amount must be greater than zero.",
                vec![amount.to_string().into()],
            ));
        }
        if let Some(activated_date) = self.activated_on_date {
            if transaction_date < activated_date {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.transaction.before.activation.date",
                    "Transaction date cannot be before the account activation date.",
                    vec![
                        transaction_date.to_string().into(),
                        activated_date.to_string().into(),
                    ],
                ));
            }
        }
        Ok(())
    }

    fn validate_lockin_period(
        &self,
        transaction_date: NaiveDate,
    ) -> Result<(), PlatformDomainRuleException> {
        if let Some(locked_until) = self.lockedin_until_date_derived {
            if transaction_date <= locked_until {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.savingsaccount.withdrawal.locked.in.until.date",
                    "Withdrawals are not allowed before the lock-in period expires.",
                    vec![
                        transaction_date.to_string().into(),
                        locked_until.to_string().into(),
                    ],
                ));
            }
        }
        Ok(())
    }

    fn minimum_allowed_running_balance(&self) -> Decimal {
        let min_req = self.effective_min_required_balance();
        let overdraft = self.effective_overdraft_limit();
        let hold = self.savings_amount_on_hold + self.on_hold_funds;
        min_req + hold - overdraft
    }

    fn validate_chronological_balances_from_date(
        &self,
        from_date: NaiveDate,
        tx_amount: Decimal,
        fee_amount: Decimal,
    ) -> Result<(), PlatformDomainRuleException> {
        let min_allowed = self.minimum_allowed_running_balance();
        // Check end-of-day running balances on or after from_date
        let active_txs: Vec<&SavingsAccountTransaction> = self
            .transactions
            .iter()
            .filter(|t| !t.is_reversed() && !t.is_reversal_transaction() && t.date_of >= from_date)
            .collect();

        for (i, tx) in active_txs.iter().enumerate() {
            let is_last_of_day = active_txs
                .get(i + 1)
                .map(|next| next.date_of != tx.date_of)
                .unwrap_or(true);
            if is_last_of_day && tx.running_balance < min_allowed {
                return Err(Self::insufficient_balance_exception(
                    tx_amount,
                    fee_amount,
                    self.withdrawable_balance(),
                ));
            }
        }
        Ok(())
    }

    fn validate_chronological_balances_after_mutation(
        &mut self,
        from_date: NaiveDate,
        inserted_tx_id: i64,
    ) -> Result<(), PlatformDomainRuleException> {
        if let Err(err) =
            self.validate_chronological_balances_from_date(from_date, Decimal::ZERO, Decimal::ZERO)
        {
            self.transactions.retain(|t| t.id != Some(inserted_tx_id));
            self.recalculate_daily_balances(from_date);
            return Err(err);
        }
        Ok(())
    }

    fn insufficient_balance_exception(
        transaction_amount: Decimal,
        fee_amount: Decimal,
        available_withdrawable: Decimal,
    ) -> PlatformDomainRuleException {
        PlatformDomainRuleException::with_args(
            "error.msg.savingsaccount.transaction.insufficient.account.balance",
            "Insufficient account balance for withdrawal transaction.",
            vec![
                transaction_amount.to_string().into(),
                fee_amount.to_string().into(),
                available_withdrawable.to_string().into(),
            ],
        )
    }

    pub fn retrieve_last_transaction_date(&self) -> Option<NaiveDate> {
        self.transactions
            .iter()
            .filter(|t| !t.is_reversed() && !t.is_reversal_transaction())
            .map(|t| t.date_of)
            .max()
    }

    pub fn find_transaction_by_id(&self, transaction_id: i64) -> Option<&SavingsAccountTransaction> {
        self.transactions
            .iter()
            .find(|t| t.id == Some(transaction_id))
    }

    fn allocate_tx_id(&mut self) -> i64 {
        let max_existing = self
            .transactions
            .iter()
            .filter_map(|t| t.id)
            .max()
            .unwrap_or(0);
        if self.next_local_tx_id <= max_existing {
            self.next_local_tx_id = max_existing + 1;
        }
        let id = self.next_local_tx_id;
        self.next_local_tx_id += 1;
        id
    }
}

// ============================================================================
// 8. Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("valid decimal")
    }

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn usd() -> MonetaryCurrency {
        MonetaryCurrency::new("USD", 2, None)
    }

    fn active_account() -> SavingsAccount {
        let mut account = SavingsAccount::new_submitted(
            "SAV-00001",
            Some(101),
            None,
            1,
            usd(),
            d("5.0"),
            ymd(2025, 1, 1),
        );
        account.id = Some(1);
        account.office_id = Some(1);
        account
            .approve(ymd(2025, 1, 2), Some(1))
            .expect("approve should succeed");
        account
            .activate(ymd(2025, 1, 3), Some(1))
            .expect("activate should succeed");
        account
    }

    #[test]
    fn test_enum_discriminants_and_roundtrips() {
        assert_eq!(SavingsAccountStatusType::Invalid.as_i32(), 0);
        assert_eq!(
            SavingsAccountStatusType::SubmittedAndPendingApproval.as_i32(),
            100
        );
        assert_eq!(SavingsAccountStatusType::Approved.as_i32(), 200);
        assert_eq!(SavingsAccountStatusType::Active.as_i32(), 300);
        assert_eq!(SavingsAccountStatusType::WithdrawnByApplicant.as_i32(), 400);
        assert_eq!(SavingsAccountStatusType::Rejected.as_i32(), 500);
        assert_eq!(SavingsAccountStatusType::Closed.as_i32(), 600);
        assert_eq!(SavingsAccountStatusType::PrematureClosed.as_i32(), 700);
        assert_eq!(SavingsAccountStatusType::TransferInProgress.as_i32(), 800);
        assert_eq!(SavingsAccountStatusType::TransferOnHold.as_i32(), 900);
        assert_eq!(SavingsAccountStatusType::Matured.as_i32(), 1000);

        assert_eq!(SavingsAccountSubStatusEnum::None.as_i32(), 0);
        assert_eq!(SavingsAccountSubStatusEnum::Inactive.as_i32(), 100);
        assert_eq!(SavingsAccountSubStatusEnum::Dormant.as_i32(), 200);
        assert_eq!(SavingsAccountSubStatusEnum::Escheat.as_i32(), 300);
        assert_eq!(SavingsAccountSubStatusEnum::Block.as_i32(), 400);
        assert_eq!(SavingsAccountSubStatusEnum::BlockCredit.as_i32(), 500);
        assert_eq!(SavingsAccountSubStatusEnum::BlockDebit.as_i32(), 600);

        assert_eq!(SavingsAccountTransactionType::Invalid.as_i32(), 0);
        assert_eq!(SavingsAccountTransactionType::Deposit.as_i32(), 1);
        assert_eq!(SavingsAccountTransactionType::Withdrawal.as_i32(), 2);
        assert_eq!(SavingsAccountTransactionType::InterestPosting.as_i32(), 3);
        assert_eq!(SavingsAccountTransactionType::WithdrawalFee.as_i32(), 4);
        assert_eq!(SavingsAccountTransactionType::AnnualFee.as_i32(), 5);
        assert_eq!(SavingsAccountTransactionType::WaiveCharge.as_i32(), 6);
        assert_eq!(SavingsAccountTransactionType::PayCharge.as_i32(), 7);
        assert_eq!(SavingsAccountTransactionType::DividendPayout.as_i32(), 8);
        assert_eq!(SavingsAccountTransactionType::InitiateTransfer.as_i32(), 12);
        assert_eq!(SavingsAccountTransactionType::ApproveTransfer.as_i32(), 13);
        assert_eq!(SavingsAccountTransactionType::WithdrawTransfer.as_i32(), 14);
        assert_eq!(SavingsAccountTransactionType::RejectTransfer.as_i32(), 15);
        assert_eq!(SavingsAccountTransactionType::WrittenOff.as_i32(), 16);
        assert_eq!(SavingsAccountTransactionType::OverdraftInterest.as_i32(), 17);
        assert_eq!(SavingsAccountTransactionType::WithholdTax.as_i32(), 18);
        assert_eq!(SavingsAccountTransactionType::Escheat.as_i32(), 19);
        assert_eq!(SavingsAccountTransactionType::AmountHold.as_i32(), 20);
        assert_eq!(SavingsAccountTransactionType::AmountRelease.as_i32(), 21);
    }

    #[test]
    fn test_lifecycle_transitions_and_dormancy() {
        let mut account = SavingsAccount::new_submitted(
            "SAV-LIFE-1",
            Some(10),
            None,
            1,
            usd(),
            d("4.5"),
            ymd(2025, 2, 1),
        );
        assert!(account.is_submitted_and_pending_approval());

        // Cannot activate before approval
        assert!(account.activate(ymd(2025, 2, 2), Some(1)).is_err());

        // Approve then activate
        account.approve(ymd(2025, 2, 2), Some(1)).unwrap();
        assert!(account.is_approved());
        account.activate(ymd(2025, 2, 3), Some(1)).unwrap();
        assert!(account.is_active());

        // Deposit 250.00
        account
            .deposit(ymd(2025, 2, 4), d("250.00"), None, ymd(2025, 2, 4))
            .unwrap();
        assert_eq!(account.account_balance(), d("250.00"));

        // Dormancy progression: None -> Inactive -> Dormant -> reset on deposit -> Inactive -> Dormant -> Escheat
        account
            .transition_dormancy_state(SavingsAccountSubStatusEnum::Inactive, ymd(2025, 3, 1))
            .unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::Inactive);

        account
            .transition_dormancy_state(SavingsAccountSubStatusEnum::Dormant, ymd(2025, 4, 1))
            .unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::Dormant);

        // Activity resets dormancy back to None
        account
            .deposit(ymd(2025, 4, 2), d("50.00"), None, ymd(2025, 4, 2))
            .unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::None);
        assert_eq!(account.account_balance(), d("300.00"));

        // Transition all the way to Escheat
        account
            .transition_dormancy_state(SavingsAccountSubStatusEnum::Inactive, ymd(2025, 5, 1))
            .unwrap();
        account
            .transition_dormancy_state(SavingsAccountSubStatusEnum::Dormant, ymd(2025, 6, 1))
            .unwrap();
        let escheat_tx = account
            .transition_dormancy_state(SavingsAccountSubStatusEnum::Escheat, ymd(2025, 7, 1))
            .unwrap()
            .expect("escheat transaction should be created");
        assert_eq!(
            escheat_tx.transaction_type,
            SavingsAccountTransactionType::Escheat
        );
        assert_eq!(escheat_tx.amount, d("300.00"));
        assert_eq!(account.account_balance(), d("0.00"));
        assert_eq!(account.status, SavingsAccountStatusType::Closed);
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::Escheat);
    }

    #[test]
    fn test_deposit_and_withdrawal_with_flat_and_percentage_withdrawal_fees() {
        let mut account = active_account();

        // Attach a $2.50 flat withdrawal fee and a 1% percentage withdrawal fee
        account.add_charge(SavingsAccountCharge::new_withdrawal_fee(
            Some(10),
            100,
            "Flat ATM Fee",
            usd(),
            ChargeCalculationType::Flat,
            d("2.50"),
            false,
        ));
        account.add_charge(SavingsAccountCharge::new_withdrawal_fee(
            Some(11),
            101,
            "Pct Withdrawal Fee",
            usd(),
            ChargeCalculationType::PercentOfAmount,
            d("1.0"),
            false,
        ));

        // Deposit $1,000.00
        let dep = account
            .deposit(ymd(2025, 1, 5), d("1000.00"), None, ymd(2025, 1, 5))
            .unwrap();
        assert_eq!(dep.running_balance, d("1000.00"));
        assert_eq!(account.summary.total_deposits, d("1000.00"));
        assert_eq!(account.account_balance(), d("1000.00"));
        assert_eq!(account.available_balance(), d("1000.00"));

        // Withdraw $200.00 -> triggers $2.50 flat + $2.00 (1% of $200) = $4.50 in withdrawal fees
        let wd = account
            .withdraw(ymd(2025, 1, 6), d("200.00"), None, ymd(2025, 1, 6), false)
            .unwrap();
        assert_eq!(
            wd.transaction_type,
            SavingsAccountTransactionType::Withdrawal
        );
        assert_eq!(wd.amount, d("200.00"));
        assert_eq!(account.summary.total_withdrawals, d("200.00"));
        assert_eq!(account.summary.total_withdrawal_fees, d("4.50"));
        assert_eq!(account.account_balance(), d("795.50"));
        assert_eq!(account.available_balance(), d("795.50"));
        assert_eq!(account.transactions.len(), 4); // 1 deposit + 1 withdrawal + 2 fee txs

        // Attempting to withdraw $793.00 should fail because $793.00 + $2.50 + $7.93 = $803.43 > $795.50
        let err = account
            .withdraw(ymd(2025, 1, 7), d("793.00"), None, ymd(2025, 1, 7), false)
            .unwrap_err();
        assert_eq!(
            err.globalisation_message_code,
            "error.msg.savingsaccount.transaction.insufficient.account.balance"
        );
        assert_eq!(account.account_balance(), d("795.50"));
    }

    #[test]
    fn test_hold_and_release_balance_locks() {
        let mut account = active_account();
        account.enforce_min_required_balance = true;
        account.min_required_balance = d("100.00");

        // Deposit $500.00 -> account_balance = $500.00, available_balance = $400.00
        account
            .deposit(ymd(2025, 1, 5), d("500.00"), None, ymd(2025, 1, 5))
            .unwrap();
        assert_eq!(account.account_balance(), d("500.00"));
        assert_eq!(account.available_balance(), d("400.00"));
        assert_eq!(account.withdrawable_balance(), d("400.00"));

        // Hold $150.00 -> account_balance stays $500.00, available_balance drops to $250.00
        let hold_tx = account
            .hold_amount(
                ymd(2025, 1, 6),
                d("150.00"),
                None,
                ymd(2025, 1, 6),
                false,
                Some("Card authorization hold".to_string()),
            )
            .unwrap();
        let hold_id = hold_tx.id.unwrap();
        assert_eq!(
            hold_tx.transaction_type,
            SavingsAccountTransactionType::AmountHold
        );
        assert_eq!(account.savings_amount_on_hold, d("150.00"));
        assert_eq!(account.account_balance(), d("500.00"));
        assert_eq!(account.available_balance(), d("250.00"));
        assert_eq!(account.withdrawable_balance(), d("250.00"));

        // Withdrawing $300.00 must fail (exceeds $250.00 withdrawable balance)
        assert!(account
            .withdraw(ymd(2025, 1, 7), d("300.00"), None, ymd(2025, 1, 7), false)
            .is_err());

        // Withdrawing $200.00 succeeds -> account_balance = $300.00, available_balance = $50.00
        account
            .withdraw(ymd(2025, 1, 7), d("200.00"), None, ymd(2025, 1, 7), false)
            .unwrap();
        assert_eq!(account.account_balance(), d("300.00"));
        assert_eq!(account.available_balance(), d("50.00"));

        // Release the $150.00 hold -> available_balance restores to $200.00
        let release_tx = account
            .release_amount(hold_id, ymd(2025, 1, 8), ymd(2025, 1, 8))
            .unwrap();
        assert_eq!(
            release_tx.transaction_type,
            SavingsAccountTransactionType::AmountRelease
        );
        assert_eq!(account.savings_amount_on_hold, d("0.00"));
        assert_eq!(account.account_balance(), d("300.00"));
        assert_eq!(account.available_balance(), d("200.00"));

        let updated_hold = account.find_transaction_by_id(hold_id).unwrap();
        assert_eq!(updated_hold.release_id_of_hold_amount, release_tx.id);

        // Releasing the same hold a second time must fail
        assert!(account
            .release_amount(hold_id, ymd(2025, 1, 9), ymd(2025, 1, 9))
            .is_err());
    }

    #[test]
    fn test_overdraft_limit_and_overdraft_amount_tracking() {
        let mut account = active_account();
        account.allow_overdraft = true;
        account.overdraft_limit = d("300.00");

        // Deposit $100.00 -> account_balance = $100.00, withdrawable_balance = $400.00
        account
            .deposit(ymd(2025, 1, 5), d("100.00"), None, ymd(2025, 1, 5))
            .unwrap();
        assert_eq!(account.withdrawable_balance(), d("400.00"));

        // Withdraw $250.00 -> drives balance to -$150.00, overdraft_amount on tx = $150.00
        let wd1 = account
            .withdraw(ymd(2025, 1, 6), d("250.00"), None, ymd(2025, 1, 6), false)
            .unwrap();
        assert_eq!(wd1.running_balance, d("-150.00"));
        assert_eq!(wd1.overdraft_amount, Some(d("150.00")));
        assert_eq!(account.account_balance(), d("-150.00"));
        assert_eq!(account.withdrawable_balance(), d("150.00"));

        // Withdraw another $100.00 -> drives balance to -$250.00, overdraft_amount on tx = $100.00
        let wd2 = account
            .withdraw(ymd(2025, 1, 7), d("100.00"), None, ymd(2025, 1, 7), false)
            .unwrap();
        assert_eq!(wd2.running_balance, d("-250.00"));
        assert_eq!(wd2.overdraft_amount, Some(d("100.00")));
        assert_eq!(account.account_balance(), d("-250.00"));
        assert_eq!(account.withdrawable_balance(), d("50.00"));

        // Attempting to withdraw $60.00 exceeds the $300.00 overdraft limit
        assert!(account
            .withdraw(ymd(2025, 1, 8), d("60.00"), None, ymd(2025, 1, 8), false)
            .is_err());

        // Deposit $400.00 recovers from overdraft -> running_balance = +$150.00, overdraft_amount = $250.00
        let dep2 = account
            .deposit(ymd(2025, 1, 9), d("400.00"), None, ymd(2025, 1, 9))
            .unwrap();
        assert_eq!(dep2.running_balance, d("150.00"));
        assert_eq!(dep2.overdraft_amount, Some(d("250.00")));
        assert_eq!(account.account_balance(), d("150.00"));
        assert_eq!(account.withdrawable_balance(), d("450.00"));
    }

    #[test]
    fn test_block_debit_credit_and_full_block_enforcement() {
        let mut account = active_account();
        account
            .deposit(ymd(2025, 1, 5), d("500.00"), None, ymd(2025, 1, 5))
            .unwrap();

        // 1. Block debit -> withdrawals fail, deposits still succeed
        account
            .block_debit(Some("Compliance review on outgoing wires".to_string()))
            .unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::BlockDebit);

        let debit_err = account
            .withdraw(ymd(2025, 1, 6), d("50.00"), None, ymd(2025, 1, 6), false)
            .unwrap_err();
        assert_eq!(
            debit_err.globalisation_message_code,
            "error.msg.savingsaccount.transaction.debits.are.blocked"
        );

        account
            .deposit(ymd(2025, 1, 6), d("100.00"), None, ymd(2025, 1, 6))
            .unwrap();
        assert_eq!(account.account_balance(), d("600.00"));

        // 2. Block credit while debit is already blocked -> upgrades sub_status to full Block
        account
            .block_credit(Some("Freeze all activity".to_string()))
            .unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::Block);

        let full_block_err = account
            .deposit(ymd(2025, 1, 7), d("50.00"), None, ymd(2025, 1, 7))
            .unwrap_err();
        assert_eq!(
            full_block_err.globalisation_message_code,
            "error.msg.savingsaccount.transaction.account.is.blocked"
        );

        // 3. Unblock debit while in full Block -> downgrades sub_status to BlockCredit
        account.unblock_debit().unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::BlockCredit);

        // Now withdrawals succeed, but deposits fail with credits.are.blocked
        account
            .withdraw(ymd(2025, 1, 7), d("100.00"), None, ymd(2025, 1, 7), false)
            .unwrap();
        assert_eq!(account.account_balance(), d("500.00"));

        let credit_err = account
            .deposit(ymd(2025, 1, 7), d("50.00"), None, ymd(2025, 1, 7))
            .unwrap_err();
        assert_eq!(
            credit_err.globalisation_message_code,
            "error.msg.savingsaccount.transaction.credits.are.blocked"
        );

        // 4. Unblock credit -> returns to None
        account.unblock_credit().unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::None);

        // 5. Explicit full block() and unblock()
        account.block(Some("Court order".to_string())).unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::Block);
        assert!(account
            .withdraw(ymd(2025, 1, 8), d("10.00"), None, ymd(2025, 1, 8), false)
            .is_err());
        account.unblock().unwrap();
        assert_eq!(account.sub_status, SavingsAccountSubStatusEnum::None);
    }

    #[test]
    fn test_backdated_transaction_and_reversal_running_balance_recalculation() {
        let mut account = active_account();
        account.enforce_min_required_balance = true;
        account.min_required_balance = d("100.00");

        // Jan 5: Deposit $1,000.00
        let tx_jan5 = account
            .deposit(ymd(2025, 1, 5), d("1000.00"), None, ymd(2025, 1, 5))
            .unwrap();
        // Jan 15: Withdraw $400.00 -> running balance = $600.00
        let tx_jan15 = account
            .withdraw(ymd(2025, 1, 15), d("400.00"), None, ymd(2025, 1, 15), false)
            .unwrap();
        // Jan 20: Deposit $200.00 -> running balance = $800.00
        let tx_jan20 = account
            .deposit(ymd(2025, 1, 20), d("200.00"), None, ymd(2025, 1, 20))
            .unwrap();

        assert_eq!(tx_jan5.running_balance, d("1000.00"));
        assert_eq!(tx_jan15.running_balance, d("600.00"));
        assert_eq!(tx_jan20.running_balance, d("800.00"));

        // Insert a backdated withdrawal of $300.00 on Jan 10 (submitted Jan 22):
        // Chronological order becomes:
        //   Jan 5:  +$1,000.00 -> running = $1,000.00 (5 days: Jan 5..Jan 10, cum = 5,000)
        //   Jan 10: -$300.00   -> running = $700.00   (5 days: Jan 10..Jan 15, cum = 3,500)
        //   Jan 15: -$400.00   -> running = $300.00   (5 days: Jan 15..Jan 20, cum = 1,500)
        //   Jan 20: +$200.00   -> running = $500.00   (2 days: Jan 20..Jan 22, cum = 1,000)
        let tx_jan10 = account
            .withdraw(ymd(2025, 1, 10), d("300.00"), None, ymd(2025, 1, 22), false)
            .unwrap();
        assert_eq!(tx_jan10.running_balance, d("700.00"));

        let t_jan5 = account.find_transaction_by_id(tx_jan5.id.unwrap()).unwrap();
        let t_jan10 = account
            .find_transaction_by_id(tx_jan10.id.unwrap())
            .unwrap();
        let t_jan15 = account
            .find_transaction_by_id(tx_jan15.id.unwrap())
            .unwrap();
        let t_jan20 = account
            .find_transaction_by_id(tx_jan20.id.unwrap())
            .unwrap();

        assert_eq!(t_jan5.running_balance, d("1000.00"));
        assert_eq!(t_jan5.balance_number_of_days, Some(5));
        assert_eq!(t_jan5.cumulative_balance, d("5000.00"));

        assert_eq!(t_jan10.running_balance, d("700.00"));
        assert_eq!(t_jan10.balance_number_of_days, Some(5));
        assert_eq!(t_jan10.cumulative_balance, d("3500.00"));

        assert_eq!(t_jan15.running_balance, d("300.00"));
        assert_eq!(t_jan15.balance_number_of_days, Some(5));
        assert_eq!(t_jan15.cumulative_balance, d("1500.00"));

        assert_eq!(t_jan20.running_balance, d("500.00"));
        assert_eq!(t_jan20.balance_number_of_days, Some(2));
        assert_eq!(t_jan20.cumulative_balance, d("1000.00"));

        assert_eq!(account.account_balance(), d("500.00"));
        assert_eq!(account.available_balance(), d("400.00"));

        // A backdated withdrawal of $250.00 on Jan 12 must be rejected because on Jan 15
        // the running balance would drop to $50.00, violating the $100.00 min_required_balance!
        let backdated_err = account
            .withdraw(ymd(2025, 1, 12), d("250.00"), None, ymd(2025, 1, 22), false)
            .unwrap_err();
        assert_eq!(
            backdated_err.globalisation_message_code,
            "error.msg.savingsaccount.transaction.insufficient.account.balance"
        );
        assert_eq!(account.account_balance(), d("500.00"));

        // Reverse the Jan 10 withdrawal -> subsequent running balances restore to $600.00 and $800.00
        let reversed = account
            .reverse_transaction(tx_jan10.id.unwrap(), ymd(2025, 1, 22))
            .unwrap();
        assert!(reversed.is_reversed());
        assert_eq!(reversed.running_balance, d("0.00"));

        let t_jan15_after = account
            .find_transaction_by_id(tx_jan15.id.unwrap())
            .unwrap();
        let t_jan20_after = account
            .find_transaction_by_id(tx_jan20.id.unwrap())
            .unwrap();
        assert_eq!(t_jan15_after.running_balance, d("600.00"));
        assert_eq!(t_jan20_after.running_balance, d("800.00"));
        assert_eq!(account.account_balance(), d("800.00"));
        assert_eq!(account.available_balance(), d("700.00"));
    }
}
