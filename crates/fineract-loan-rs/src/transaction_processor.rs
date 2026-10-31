//! Loan Repayment Schedule Transaction Processor Strategies & Backdated Replay Engine.
//!
//! Faithfully ports Apache Fineract's `org.apache.fineract.portfolio.loanaccount.domain.transactionprocessor`:
//! - [`TransactionProcessorStrategyCode`] (`LoanRepaymentScheduleTransactionProcessorFactory`, strategy codes & names)
//! - [`PaymentComponent`] (`Penalty`, `Fee`, `Interest`, `Principal`)
//! - [`ChangedTransactionDetail`] and [`TransactionChangeData`] (`ChangedTransactionDetail.java`)
//! - [`LoanRepaymentScheduleTransactionProcessor`] strategy implementations:
//!   - [`MifosStandard`](TransactionProcessorStrategyCode::MifosStandard) (`FineractStyleLoanRepaymentScheduleTransactionProcessor`)
//!   - [`HeavensFamily`](TransactionProcessorStrategyCode::HeavensFamily) (`HeavensFamilyLoanRepaymentScheduleTransactionProcessor`)
//!   - [`Creocore`](TransactionProcessorStrategyCode::Creocore) (`CreocoreLoanRepaymentScheduleTransactionProcessor`)
//!   - [`RbiIndia`](TransactionProcessorStrategyCode::RbiIndia) (`RBILoanRepaymentScheduleTransactionProcessor`)
//!   - [`PrincipalInterestPenaltiesFees`](TransactionProcessorStrategyCode::PrincipalInterestPenaltiesFees) (`PrincipalInterestPenaltyFeesOrderLoanRepaymentScheduleTransactionProcessor`)
//!   - [`InterestPrincipalPenaltiesFees`](TransactionProcessorStrategyCode::InterestPrincipalPenaltiesFees) (`InterestPrincipalPenaltyFeesOrderLoanRepaymentScheduleTransactionProcessor`)
//!   - [`EarlyRepayment`](TransactionProcessorStrategyCode::EarlyRepayment) (`EarlyPaymentLoanRepaymentScheduleTransactionProcessor`)
//!   - [`DuePenaltyFeeInterestPrincipalInAdvancePrincipalPenaltyFeeInterest`](TransactionProcessorStrategyCode::DuePenaltyFeeInterestPrincipalInAdvancePrincipalPenaltyFeeInterest)
//!   - [`DuePenaltyInterestPrincipalFeeInAdvancePenaltyInterestPrincipalFee`](TransactionProcessorStrategyCode::DuePenaltyInterestPrincipalFeeInAdvancePenaltyInterestPrincipalFee)
//!   - [`AdvancedPaymentAllocation`](TransactionProcessorStrategyCode::AdvancedPaymentAllocation) (`AdvancedPaymentScheduleTransactionProcessor`)
//! - [`BackdatedTransactionReplayEngine`] (`AbstractLoanRepaymentScheduleTransactionProcessor` + `LoanRepaymentScheduleProcessingWrapper`)

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use chrono::NaiveDate;
use fineract_money::{Decimal, FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::FineractError;
use serde::{Deserialize, Serialize};

use crate::schedule_and_terms::{
    LoanCharge, LoanRepaymentScheduleInstallment, LoanTransaction,
    LoanTransactionToRepaymentScheduleMapping, LoanTransactionType,
};

// ============================================================================
// 1. TransactionProcessorStrategyCode
// ============================================================================

/// Identifies the repayment schedule transaction processing strategy.
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.domain.transactionprocessor.impl.*`
/// and `LoanProductTransactionProcessingStrategy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TransactionProcessorStrategyCode {
    /// `"mifos-standard-strategy"` — Penalties -> Fees -> Interest -> Principal
    /// (`FineractStyleLoanRepaymentScheduleTransactionProcessor`).
    MifosStandard,
    /// `"heavensfamily-strategy"` — Overdue/on-time installments: Penalties -> Fees -> Interest -> Principal;
    /// strictly in-advance installments: Principal first (waiving remaining installment interest if principal
    /// of that future installment is fully paid in advance), then Penalties -> Fees -> Interest
    /// (`HeavensFamilyLoanRepaymentScheduleTransactionProcessor`).
    HeavensFamily,
    /// `"creocore-strategy"` — Overdue Penalties -> Fees -> Interest across all overdue installments first,
    /// then standard Penalties -> Fees -> Interest -> Principal across installments
    /// (`CreocoreLoanRepaymentScheduleTransactionProcessor`).
    Creocore,
    /// `"rbi-india-strategy"` — All overdue and currently due Penalties -> Fees -> Interest across all
    /// overdue/due installments first before any Principal is allocated
    /// (`RBILoanRepaymentScheduleTransactionProcessor`).
    RbiIndia,
    /// `"principal-interest-penalties-fees-order-strategy"` — Principal -> Interest -> Penalties -> Fees
    /// (`PrincipalInterestPenaltyFeesOrderLoanRepaymentScheduleTransactionProcessor`).
    PrincipalInterestPenaltiesFees,
    /// `"interest-principal-penalties-fees-order-strategy"` — Interest -> Principal -> Penalties -> Fees
    /// (`InterestPrincipalPenaltyFeesOrderLoanRepaymentScheduleTransactionProcessor`).
    InterestPrincipalPenaltiesFees,
    /// `"early-repayment-strategy"` — Overdue/due installments: Interest -> Principal -> Penalties -> Fees;
    /// in-advance / excess payment allocated directly to Principal reduction across unpaid installments
    /// (`EarlyPaymentLoanRepaymentScheduleTransactionProcessor`).
    EarlyRepayment,
    /// `"due-penalty-fee-interest-principal-in-advance-principal-penalty-fee-interest-strategy"`
    /// (`DuePenFeeIntPriInAdvancePriPenFeeIntLoanRepaymentScheduleTransactionProcessor`).
    DuePenaltyFeeInterestPrincipalInAdvancePrincipalPenaltyFeeInterest,
    /// `"due-penalty-interest-principal-fee-in-advance-penalty-interest-principal-fee-strategy"`
    /// (`DuePenIntPriFeeInAdvancePenIntPriFeeLoanRepaymentScheduleTransactionProcessor`).
    DuePenaltyInterestPrincipalFeeInAdvancePenaltyInterestPrincipalFee,
    /// `"advanced-payment-allocation-strategy"` — Configurable horizon-aware allocation
    /// (`PastDue -> Due -> InAdvance`) (`AdvancedPaymentScheduleTransactionProcessor`).
    AdvancedPaymentAllocation,
}

impl TransactionProcessorStrategyCode {
    pub const MIFOS_STANDARD: Self = Self::MifosStandard;
    pub const HEAVENS_FAMILY: Self = Self::HeavensFamily;
    pub const CREOCORE: Self = Self::Creocore;
    pub const RBI_INDIA: Self = Self::RbiIndia;
    pub const PRINCIPAL_INTEREST_PENALTIES_FEES: Self = Self::PrincipalInterestPenaltiesFees;
    pub const INTEREST_PRINCIPAL_PENALTIES_FEES: Self = Self::InterestPrincipalPenaltiesFees;
    pub const EARLY_REPAYMENT: Self = Self::EarlyRepayment;
    pub const ADVANCED_PAYMENT_ALLOCATION: Self = Self::AdvancedPaymentAllocation;

    pub const ALL: &'static [Self] = &[
        Self::MifosStandard,
        Self::HeavensFamily,
        Self::Creocore,
        Self::RbiIndia,
        Self::PrincipalInterestPenaltiesFees,
        Self::InterestPrincipalPenaltiesFees,
        Self::EarlyRepayment,
        Self::DuePenaltyFeeInterestPrincipalInAdvancePrincipalPenaltyFeeInterest,
        Self::DuePenaltyInterestPrincipalFeeInAdvancePenaltyInterestPrincipalFee,
        Self::AdvancedPaymentAllocation,
    ];

    /// Upstream Fineract strategy code string.
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MifosStandard => "mifos-standard-strategy",
            Self::HeavensFamily => "heavensfamily-strategy",
            Self::Creocore => "creocore-strategy",
            Self::RbiIndia => "rbi-india-strategy",
            Self::PrincipalInterestPenaltiesFees => {
                "principal-interest-penalties-fees-order-strategy"
            }
            Self::InterestPrincipalPenaltiesFees => {
                "interest-principal-penalties-fees-order-strategy"
            }
            Self::EarlyRepayment => "early-repayment-strategy",
            Self::DuePenaltyFeeInterestPrincipalInAdvancePrincipalPenaltyFeeInterest => {
                "due-penalty-fee-interest-principal-in-advance-principal-penalty-fee-interest-strategy"
            }
            Self::DuePenaltyInterestPrincipalFeeInAdvancePenaltyInterestPrincipalFee => {
                "due-penalty-interest-principal-fee-in-advance-penalty-interest-principal-fee-strategy"
            }
            Self::AdvancedPaymentAllocation => "advanced-payment-allocation-strategy",
        }
    }

    /// Upstream Fineract human-readable strategy name.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MifosStandard => "Penalties, Fees, Interest, Principal order",
            Self::HeavensFamily => "HeavensFamily Unique",
            Self::Creocore => "Creocore Unique",
            Self::RbiIndia => "Overdue/Due Fee/Int,Principal",
            Self::PrincipalInterestPenaltiesFees => "Principal, Interest, Penalties, Fees Order",
            Self::InterestPrincipalPenaltiesFees => "Interest, Principal, Penalties, Fees Order",
            Self::EarlyRepayment => "Early Repayment Strategy",
            Self::DuePenaltyFeeInterestPrincipalInAdvancePrincipalPenaltyFeeInterest => {
                "Due Penalty, Fee, Interest, Principal, In Advance Principal, Penalty, Fee, Interest"
            }
            Self::DuePenaltyInterestPrincipalFeeInAdvancePenaltyInterestPrincipalFee => {
                "Due Penalty, Interest, Principal, Fee, In Advance Penalty, Interest, Principal, Fee"
            }
            Self::AdvancedPaymentAllocation => "Advanced payment allocation strategy",
        }
    }

    /// Resolves a strategy from its Fineract code or name (case-insensitive).
    pub fn from_code(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        for candidate in Self::ALL {
            if candidate.accept(trimmed) {
                return Some(*candidate);
            }
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "mifos" | "standard" | "fineract-style" | "default" => Some(Self::MifosStandard),
            "heavensfamily" | "heavens-family" => Some(Self::HeavensFamily),
            "creocore" => Some(Self::Creocore),
            "rbi" | "rbi-india" => Some(Self::RbiIndia),
            "pipf" | "principal-interest-penalties-fees" => {
                Some(Self::PrincipalInterestPenaltiesFees)
            }
            "ippf" | "interest-principal-penalties-fees" => {
                Some(Self::InterestPrincipalPenaltiesFees)
            }
            "early-repayment" | "early-payment" => Some(Self::EarlyRepayment),
            "advanced" | "advanced-payment-allocation" => Some(Self::AdvancedPaymentAllocation),
            _ => None,
        }
    }

    /// Mirrors `LoanRepaymentScheduleTransactionProcessor.accept(String)`.
    pub fn accept(self, s: &str) -> bool {
        let trimmed = s.trim();
        trimmed.eq_ignore_ascii_case(self.code()) || trimmed.eq_ignore_ascii_case(self.name())
    }
}

impl Default for TransactionProcessorStrategyCode {
    fn default() -> Self {
        Self::MifosStandard
    }
}

impl fmt::Display for TransactionProcessorStrategyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for TransactionProcessorStrategyCode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_code(s).ok_or_else(|| format!("Unknown transaction processor strategy: '{s}'"))
    }
}

// ============================================================================
// 2. Payment Allocation Building Blocks & Change Tracking
// ============================================================================

/// Individual monetary obligation component on a repayment schedule installment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaymentComponent {
    Penalty,
    Fee,
    Interest,
    Principal,
}

impl PaymentComponent {
    pub const STANDARD_ORDER: [Self; 4] = [
        Self::Penalty,
        Self::Fee,
        Self::Interest,
        Self::Principal,
    ];

    pub const PRINCIPAL_INTEREST_PENALTIES_FEES_ORDER: [Self; 4] = [
        Self::Principal,
        Self::Interest,
        Self::Penalty,
        Self::Fee,
    ];

    pub const INTEREST_PRINCIPAL_PENALTIES_FEES_ORDER: [Self; 4] = [
        Self::Interest,
        Self::Principal,
        Self::Penalty,
        Self::Fee,
    ];

    pub const PRINCIPAL_PENALTY_FEE_INTEREST_ORDER: [Self; 4] = [
        Self::Principal,
        Self::Penalty,
        Self::Fee,
        Self::Interest,
    ];

    pub const PENALTY_INTEREST_PRINCIPAL_FEE_ORDER: [Self; 4] = [
        Self::Penalty,
        Self::Interest,
        Self::Principal,
        Self::Fee,
    ];
}

/// Captures a transaction whose derived allocation changed during backdated replay.
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.domain.TransactionChangeData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionChangeData {
    pub old_transaction: LoanTransaction,
    pub new_transaction: LoanTransaction,
}

/// Aggregates all transaction changes detected during [`BackdatedTransactionReplayEngine`] replay.
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.domain.ChangedTransactionDetail`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedTransactionDetail {
    pub transaction_changes: Vec<TransactionChangeData>,
    pub new_transaction_mappings: BTreeMap<i64, LoanTransaction>,
}

impl ChangedTransactionDetail {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_changes(&self) -> bool {
        !self.transaction_changes.is_empty()
    }
}

/// Snapshot of a transaction's derived allocation portions before reprocessing,
/// used to detect whether a backdated insertion altered an existing transaction's breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TransactionPortionSnapshot {
    id: Option<i64>,
    principal_portion: Decimal,
    interest_portion: Decimal,
    fee_charges_portion: Decimal,
    penalty_charges_portion: Decimal,
    overpayment_portion: Decimal,
    outstanding_loan_balance: Decimal,
}

impl TransactionPortionSnapshot {
    fn capture(tx: &LoanTransaction) -> Self {
        Self {
            id: tx.id,
            principal_portion: tx.principal_portion,
            interest_portion: tx.interest_portion,
            fee_charges_portion: tx.fee_charges_portion,
            penalty_charges_portion: tx.penalty_charges_portion,
            overpayment_portion: tx.overpayment_portion,
            outstanding_loan_balance: tx.outstanding_loan_balance,
        }
    }

    fn differs_from(&self, tx: &LoanTransaction) -> bool {
        self.principal_portion != tx.principal_portion
            || self.interest_portion != tx.interest_portion
            || self.fee_charges_portion != tx.fee_charges_portion
            || self.penalty_charges_portion != tx.penalty_charges_portion
            || self.overpayment_portion != tx.overpayment_portion
            || self.outstanding_loan_balance != tx.outstanding_loan_balance
    }
}

/// Accumulates paid/waived/written-off portions for a single transaction across installments.
#[derive(Debug, Clone, Copy, Default)]
struct TransactionAllocationTotals {
    principal: Decimal,
    interest: Decimal,
    fees: Decimal,
    penalties: Decimal,
}

impl TransactionAllocationTotals {
    #[inline]
    fn add_component(&mut self, component: PaymentComponent, amount: Decimal) {
        if amount <= Decimal::ZERO {
            return;
        }
        match component {
            PaymentComponent::Principal => self.principal += amount,
            PaymentComponent::Interest => self.interest += amount,
            PaymentComponent::Fee => self.fees += amount,
            PaymentComponent::Penalty => self.penalties += amount,
        }
    }
}

// ============================================================================
// 3. Installment Component Manipulation Helpers (Fineract-Exact Semantics)
// ============================================================================

/// Pays a single component on `installment` up to `remaining`, updating `total_paid_in_advance`,
/// `total_paid_late`, `obligations_met`, `obligations_met_on_date`, and `mapping`.
fn pay_installment_component(
    installment: &mut LoanRepaymentScheduleInstallment,
    component: PaymentComponent,
    transaction_date: NaiveDate,
    remaining: &mut Decimal,
    totals: &mut TransactionAllocationTotals,
    mapping: &mut InstallmentMappingAccumulator,
    currency: &MonetaryCurrency,
    mode: FineractRoundingMode,
) -> Decimal {
    if *remaining <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let available = Money::of(currency, *remaining, mode).amount();
    let paid = match component {
        PaymentComponent::Penalty => installment.pay_penalty_charges(transaction_date, available),
        PaymentComponent::Fee => installment.pay_fee_charges(transaction_date, available),
        PaymentComponent::Interest => installment.pay_interest(transaction_date, available),
        PaymentComponent::Principal => installment.pay_principal(transaction_date, available),
    };

    if paid > Decimal::ZERO {
        *remaining = (*remaining - paid).max(Decimal::ZERO);
        totals.add_component(component, paid);
        mapping.add_component(component, paid);
    }
    paid
}

/// Pays a list of components in order on a single `installment`.
fn pay_installment_in_order(
    installment: &mut LoanRepaymentScheduleInstallment,
    order: &[PaymentComponent],
    transaction_date: NaiveDate,
    remaining: &mut Decimal,
    totals: &mut TransactionAllocationTotals,
    mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
    transaction_id: Option<i64>,
    currency: &MonetaryCurrency,
    mode: FineractRoundingMode,
) {
    if *remaining <= Decimal::ZERO || installment.total_outstanding() <= Decimal::ZERO {
        return;
    }

    let mut acc = InstallmentMappingAccumulator::new(
        transaction_id,
        installment.id,
        installment.installment_number,
        installment.due_date,
    );
    for &component in order {
        if *remaining <= Decimal::ZERO {
            break;
        }
        pay_installment_component(
            installment,
            component,
            transaction_date,
            remaining,
            totals,
            &mut acc,
            currency,
            mode,
        );
    }
    acc.flush_into(mappings);
}

/// Unpays (reverses payment on) a single component on `installment` during a `Refund` /
/// `RefundForActiveLoan` transaction, returning the amount deducted from `*_completed`.
fn unpay_installment_component(
    installment: &mut LoanRepaymentScheduleInstallment,
    component: PaymentComponent,
    transaction_date: NaiveDate,
    remaining_refund: &mut Decimal,
    totals: &mut TransactionAllocationTotals,
    mapping: &mut InstallmentMappingAccumulator,
    currency: &MonetaryCurrency,
    mode: FineractRoundingMode,
) -> Decimal {
    if *remaining_refund <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let refund_available = Money::of(currency, *remaining_refund, mode).amount();
    let deduct = match component {
        PaymentComponent::Penalty => {
            installment.unpay_penalty_charges(transaction_date, refund_available)
        }
        PaymentComponent::Fee => installment.unpay_fee_charges(transaction_date, refund_available),
        PaymentComponent::Interest => {
            installment.unpay_interest(transaction_date, refund_available)
        }
        PaymentComponent::Principal => {
            installment.unpay_principal(transaction_date, refund_available)
        }
    };

    if deduct > Decimal::ZERO {
        *remaining_refund = (*remaining_refund - deduct).max(Decimal::ZERO);
        totals.add_component(component, deduct);
        mapping.add_component(component, deduct);
    }
    deduct
}

/// Accumulator for a single `(LoanTransaction, LoanRepaymentScheduleInstallment)` mapping.
#[derive(Debug, Clone)]
struct InstallmentMappingAccumulator {
    loan_transaction_id: Option<i64>,
    loan_repayment_schedule_id: Option<i64>,
    installment_number: i32,
    due_date: NaiveDate,
    principal: Decimal,
    interest: Decimal,
    fees: Decimal,
    penalties: Decimal,
}

impl InstallmentMappingAccumulator {
    fn new(
        loan_transaction_id: Option<i64>,
        loan_repayment_schedule_id: Option<i64>,
        installment_number: i32,
        due_date: NaiveDate,
    ) -> Self {
        Self {
            loan_transaction_id,
            loan_repayment_schedule_id,
            installment_number,
            due_date,
            principal: Decimal::ZERO,
            interest: Decimal::ZERO,
            fees: Decimal::ZERO,
            penalties: Decimal::ZERO,
        }
    }

    fn add_component(&mut self, component: PaymentComponent, amount: Decimal) {
        if amount <= Decimal::ZERO {
            return;
        }
        match component {
            PaymentComponent::Principal => self.principal += amount,
            PaymentComponent::Interest => self.interest += amount,
            PaymentComponent::Fee => self.fees += amount,
            PaymentComponent::Penalty => self.penalties += amount,
        }
    }

    fn total_amount(&self) -> Decimal {
        self.principal + self.interest + self.fees + self.penalties
    }

    fn flush_into(self, mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>) {
        let total = self.total_amount();
        if total <= Decimal::ZERO {
            return;
        }
        // Merge if a mapping for the same installment already exists on this transaction
        if let Some(existing) = mappings.iter_mut().find(|m| {
            (self.loan_repayment_schedule_id.is_some()
                && m.loan_repayment_schedule_id == self.loan_repayment_schedule_id)
                || m.installment_number == Some(self.installment_number)
        }) {
            existing.update_components(self.principal, self.interest, self.fees, self.penalties);
        } else {
            mappings.push(LoanTransactionToRepaymentScheduleMapping::new(
                self.loan_transaction_id,
                self.loan_repayment_schedule_id,
                Some(self.installment_number),
                Some(self.due_date),
                self.principal,
                self.interest,
                self.fees,
                self.penalties,
            ));
        }
    }
}

// ============================================================================
// 4. Strategy Processor Engine
// ============================================================================

/// Executes single-transaction and multi-transaction schedule allocation for any
/// [`TransactionProcessorStrategyCode`].
#[derive(Debug, Clone, Copy, Default)]
pub struct LoanRepaymentScheduleTransactionProcessor {
    pub strategy: TransactionProcessorStrategyCode,
}

impl LoanRepaymentScheduleTransactionProcessor {
    pub const fn new(strategy: TransactionProcessorStrategyCode) -> Self {
        Self { strategy }
    }

    /// Processes a single monetary repayment-like transaction (`Repayment`, `DownPayment`,
    /// `RecoveryRepayment`, `RepaymentAtDisbursement`, `GoodwillCredit`, `MerchantIssuedRefund`,
    /// `PayoutRefund`) against `installments` according to `self.strategy`.
    ///
    /// Returns the overpayment [`Money`] if the transaction amount exceeded total schedule obligations.
    pub fn process_repayment_transaction(
        &self,
        transaction: &mut LoanTransaction,
        installments: &mut [LoanRepaymentScheduleInstallment],
        charges: &mut [LoanCharge],
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) -> Money {
        transaction.reset_derived_components();

        let tx_date = transaction.transaction_date;
        let tx_id = transaction.id;
        let mut remaining = Money::of(currency, transaction.amount, mode).amount();
        let mut totals = TransactionAllocationTotals::default();
        let mut mappings = Vec::new();

        if transaction.transaction_type == LoanTransactionType::RepaymentAtDisbursement {
            // Repayment at disbursement only pays disbursement-time fee/penalty charges, not installments
            totals.fees = remaining;
            remaining = Decimal::ZERO;
        } else {
            match self.strategy {
                TransactionProcessorStrategyCode::MifosStandard => {
                    Self::allocate_mifos_standard(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::HeavensFamily => {
                    Self::allocate_heavens_family(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::Creocore => {
                    Self::allocate_creocore(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::RbiIndia => {
                    Self::allocate_rbi_india(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::PrincipalInterestPenaltiesFees => {
                    Self::allocate_principal_interest_penalties_fees(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::InterestPrincipalPenaltiesFees => {
                    Self::allocate_interest_principal_penalties_fees(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::EarlyRepayment => {
                    Self::allocate_early_repayment(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::DuePenaltyFeeInterestPrincipalInAdvancePrincipalPenaltyFeeInterest => {
                    Self::allocate_due_pen_fee_int_pri_in_advance_pri_pen_fee_int(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::DuePenaltyInterestPrincipalFeeInAdvancePenaltyInterestPrincipalFee => {
                    Self::allocate_due_pen_int_pri_fee_in_advance_pen_int_pri_fee(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
                TransactionProcessorStrategyCode::AdvancedPaymentAllocation => {
                    Self::allocate_advanced_payment_allocation(
                        installments,
                        tx_date,
                        tx_id,
                        &mut remaining,
                        &mut totals,
                        &mut mappings,
                        currency,
                        mode,
                    );
                }
            }
        }

        let overpayment = Money::of(currency, remaining.max(Decimal::ZERO), mode);
        transaction.update_components(
            totals.principal,
            totals.interest,
            totals.fees,
            totals.penalties,
        );
        transaction.overpayment_portion = overpayment.amount();
        transaction.repayment_schedule_mappings = mappings;

        // Update paid amounts on active charges proportionally
        Self::apply_paid_charges(charges, totals.fees, totals.penalties);

        overpayment
    }

    // ------------------------------------------------------------------------
    // Strategy 1: MifosStandard ("mifos-standard-strategy")
    // Penalties -> Fees -> Interest -> Principal (installment by installment)
    // ------------------------------------------------------------------------
    fn allocate_mifos_standard(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            pay_installment_in_order(
                installment,
                &PaymentComponent::STANDARD_ORDER,
                tx_date,
                remaining,
                totals,
                mappings,
                tx_id,
                currency,
                mode,
            );
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 2: HeavensFamily ("heavensfamily-strategy")
    // - For overdue or due installments (`tx_date >= due_date`, or `tx_date >= from_date`
    //   when not strictly in advance): Penalties -> Fees -> Interest -> Principal.
    // - For strictly in-advance payments (`tx_date < due_date` and all prior due installments
    //   are settled): allocates to Principal first; if that future installment's Principal
    //   is completely paid off in advance, waives the remaining Interest on that installment!
    //   Then allocates to Penalties -> Fees -> Interest.
    // ------------------------------------------------------------------------
    fn allocate_heavens_family(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        for idx in 0..installments.len() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            if installments[idx].total_outstanding() <= Decimal::ZERO {
                continue;
            }

            let prev_due_date = if idx > 0 {
                Some(installments[idx - 1].due_date)
            } else {
                installments[idx].from_date
            };

            let due_date = installments[idx].due_date;
            let is_in_advance = tx_date < due_date
                && prev_due_date.map_or(true, |prev_due| tx_date <= prev_due || idx == 0);

            let installment = &mut installments[idx];
            if is_in_advance {
                let mut acc = InstallmentMappingAccumulator::new(
                    tx_id,
                    installment.id,
                    installment.installment_number,
                    installment.due_date,
                );
                let had_principal_before = installment.principal_outstanding() > Decimal::ZERO;
                pay_installment_component(
                    installment,
                    PaymentComponent::Principal,
                    tx_date,
                    remaining,
                    totals,
                    &mut acc,
                    currency,
                    mode,
                );
                // HeavensFamily unique rule: if a future installment's principal is paid in full
                // in advance, the remaining interest on that installment is waived.
                if had_principal_before
                    && installment.principal_outstanding() <= Decimal::ZERO
                    && installment.interest_outstanding() > Decimal::ZERO
                {
                    let interest_to_waive = installment.interest_outstanding();
                    let _ = installment.waive_interest(tx_date, interest_to_waive);
                }

                for &component in &[
                    PaymentComponent::Penalty,
                    PaymentComponent::Fee,
                    PaymentComponent::Interest,
                ] {
                    if *remaining <= Decimal::ZERO {
                        break;
                    }
                    pay_installment_component(
                        installment,
                        component,
                        tx_date,
                        remaining,
                        totals,
                        &mut acc,
                        currency,
                        mode,
                    );
                }
                acc.flush_into(mappings);
            } else {
                pay_installment_in_order(
                    installment,
                    &PaymentComponent::STANDARD_ORDER,
                    tx_date,
                    remaining,
                    totals,
                    mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 3: Creocore ("creocore-strategy")
    // - Pass 1: Across all overdue installments (`due_date < tx_date`), pay
    //   Penalties -> Fees -> Interest first before any Principal is paid.
    // - Pass 2: Across all installments, pay Penalties -> Fees -> Interest -> Principal.
    // ------------------------------------------------------------------------
    fn allocate_creocore(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        let has_overdue = installments
            .iter()
            .any(|inst| inst.due_date < tx_date && inst.total_outstanding() > Decimal::ZERO);

        if has_overdue {
            // Pass 1: Overdue Penalties, Fees, Interest across all overdue installments
            for installment in installments.iter_mut() {
                if *remaining <= Decimal::ZERO {
                    break;
                }
                if installment.due_date < tx_date && installment.total_outstanding() > Decimal::ZERO
                {
                    pay_installment_in_order(
                        installment,
                        &[
                            PaymentComponent::Penalty,
                            PaymentComponent::Fee,
                            PaymentComponent::Interest,
                        ],
                        tx_date,
                        remaining,
                        totals,
                        mappings,
                        tx_id,
                        currency,
                        mode,
                    );
                }
            }
        }

        // Pass 2: Standard order across all installments (pays overdue Principal, then current/future installments)
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            pay_installment_in_order(
                installment,
                &PaymentComponent::STANDARD_ORDER,
                tx_date,
                remaining,
                totals,
                mappings,
                tx_id,
                currency,
                mode,
            );
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 4: RbiIndia ("rbi-india-strategy")
    // Reserve Bank of India (RBI) guideline:
    // - Pass 1: All overdue and currently due Penalties, Fees, and Interest across all
    //   overdue/due installments (`due_date <= tx_date`) must be paid first before ANY
    //   Principal is reduced.
    // - Pass 2: Allocate remaining amount to Principal (and any future installments in
    //   Penalties -> Fees -> Interest -> Principal order).
    // ------------------------------------------------------------------------
    fn allocate_rbi_india(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        // Pass 1: Pay Penalties, Fees, and Interest on all installments that are overdue or due on tx_date
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            if installment.due_date <= tx_date && installment.total_outstanding() > Decimal::ZERO {
                pay_installment_in_order(
                    installment,
                    &[
                        PaymentComponent::Penalty,
                        PaymentComponent::Fee,
                        PaymentComponent::Interest,
                    ],
                    tx_date,
                    remaining,
                    totals,
                    mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }

        // Pass 2: Now pay Principal on overdue/due installments, and Penalties -> Fees -> Interest -> Principal on future installments
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            pay_installment_in_order(
                installment,
                &PaymentComponent::STANDARD_ORDER,
                tx_date,
                remaining,
                totals,
                mappings,
                tx_id,
                currency,
                mode,
            );
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 5: PrincipalInterestPenaltiesFees
    // ("principal-interest-penalties-fees-order-strategy")
    // Principal -> Interest -> Penalties -> Fees
    // ------------------------------------------------------------------------
    fn allocate_principal_interest_penalties_fees(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            pay_installment_in_order(
                installment,
                &PaymentComponent::PRINCIPAL_INTEREST_PENALTIES_FEES_ORDER,
                tx_date,
                remaining,
                totals,
                mappings,
                tx_id,
                currency,
                mode,
            );
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 6: InterestPrincipalPenaltiesFees
    // ("interest-principal-penalties-fees-order-strategy")
    // Interest -> Principal -> Penalties -> Fees
    // ------------------------------------------------------------------------
    fn allocate_interest_principal_penalties_fees(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            pay_installment_in_order(
                installment,
                &PaymentComponent::INTEREST_PRINCIPAL_PENALTIES_FEES_ORDER,
                tx_date,
                remaining,
                totals,
                mappings,
                tx_id,
                currency,
                mode,
            );
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 7: EarlyRepayment ("early-repayment-strategy")
    // (`EarlyPaymentLoanRepaymentScheduleTransactionProcessor`)
    // - Pass 1: For all installments due on or before `tx_date` (`due_date <= tx_date`),
    //   allocate in `Interest -> Principal -> Penalties -> Fees` order.
    // - Pass 2: Any excess / in-advance payment (`due_date > tx_date`) is allocated
    //   exclusively to `Principal` across unpaid installments first.
    // - Pass 3: If all Principal across the entire loan is paid off and money still remains,
    //   allocate to remaining `Interest -> Penalties -> Fees` across unpaid installments.
    // ------------------------------------------------------------------------
    fn allocate_early_repayment(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        // Pass 1: Overdue and due-today installments: Interest -> Principal -> Penalties -> Fees
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            if installment.due_date <= tx_date && installment.total_outstanding() > Decimal::ZERO {
                pay_installment_in_order(
                    installment,
                    &PaymentComponent::INTEREST_PRINCIPAL_PENALTIES_FEES_ORDER,
                    tx_date,
                    remaining,
                    totals,
                    mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }

        // Pass 2: In-advance / excess payment allocated directly to Principal of unpaid installments
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            if installment.principal_outstanding() > Decimal::ZERO {
                pay_installment_in_order(
                    installment,
                    &[PaymentComponent::Principal],
                    tx_date,
                    remaining,
                    totals,
                    mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }

        // Pass 3: Any leftover after all principal is cleared pays remaining Interest -> Penalties -> Fees
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            if installment.total_outstanding() > Decimal::ZERO {
                pay_installment_in_order(
                    installment,
                    &[
                        PaymentComponent::Interest,
                        PaymentComponent::Penalty,
                        PaymentComponent::Fee,
                    ],
                    tx_date,
                    remaining,
                    totals,
                    mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 8: DuePenaltyFeeInterestPrincipalInAdvancePrincipalPenaltyFeeInterest
    // ------------------------------------------------------------------------
    fn allocate_due_pen_fee_int_pri_in_advance_pri_pen_fee_int(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            let order = if tx_date < installment.due_date {
                &PaymentComponent::PRINCIPAL_PENALTY_FEE_INTEREST_ORDER
            } else {
                &PaymentComponent::STANDARD_ORDER
            };
            pay_installment_in_order(
                installment,
                order,
                tx_date,
                remaining,
                totals,
                mappings,
                tx_id,
                currency,
                mode,
            );
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 9: DuePenaltyInterestPrincipalFeeInAdvancePenaltyInterestPrincipalFee
    // ------------------------------------------------------------------------
    fn allocate_due_pen_int_pri_fee_in_advance_pen_int_pri_fee(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            pay_installment_in_order(
                installment,
                &PaymentComponent::PENALTY_INTEREST_PRINCIPAL_FEE_ORDER,
                tx_date,
                remaining,
                totals,
                mappings,
                tx_id,
                currency,
                mode,
            );
        }
    }

    // ------------------------------------------------------------------------
    // Strategy 10: AdvancedPaymentAllocation ("advanced-payment-allocation-strategy")
    // Horizon-based allocation:
    //   1. PAST_DUE (`due_date < tx_date`): Penalties -> Fees -> Interest -> Principal
    //   2. DUE (`due_date == tx_date`): Penalties -> Fees -> Interest -> Principal
    //   3. IN_ADVANCE (`due_date > tx_date`): Principal -> Penalties -> Fees -> Interest
    // ------------------------------------------------------------------------
    fn allocate_advanced_payment_allocation(
        installments: &mut [LoanRepaymentScheduleInstallment],
        tx_date: NaiveDate,
        tx_id: Option<i64>,
        remaining: &mut Decimal,
        totals: &mut TransactionAllocationTotals,
        mappings: &mut Vec<LoanTransactionToRepaymentScheduleMapping>,
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        // Horizon 1: PAST_DUE (due_date < tx_date)
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            if installment.due_date < tx_date && installment.total_outstanding() > Decimal::ZERO {
                pay_installment_in_order(
                    installment,
                    &PaymentComponent::STANDARD_ORDER,
                    tx_date,
                    remaining,
                    totals,
                    mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }

        // Horizon 2: DUE (due_date == tx_date)
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            if installment.due_date == tx_date && installment.total_outstanding() > Decimal::ZERO {
                pay_installment_in_order(
                    installment,
                    &PaymentComponent::STANDARD_ORDER,
                    tx_date,
                    remaining,
                    totals,
                    mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }

        // Horizon 3: IN_ADVANCE (due_date > tx_date) — Principal first in progressive/advanced default
        for installment in installments.iter_mut() {
            if *remaining <= Decimal::ZERO {
                break;
            }
            if installment.due_date > tx_date && installment.total_outstanding() > Decimal::ZERO {
                pay_installment_in_order(
                    installment,
                    &PaymentComponent::PRINCIPAL_PENALTY_FEE_INTEREST_ORDER,
                    tx_date,
                    remaining,
                    totals,
                    mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }
    }

    // ------------------------------------------------------------------------
    // ChargePayment, Waiver, WriteOff, Chargeback, Refund Handlers
    // ------------------------------------------------------------------------

    /// Processes a `ChargePayment` transaction (pays fee/penalty charges first, then remaining via strategy).
    pub fn process_charge_payment(
        &self,
        transaction: &mut LoanTransaction,
        installments: &mut [LoanRepaymentScheduleInstallment],
        charges: &mut [LoanCharge],
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) -> Money {
        transaction.reset_derived_components();

        let tx_date = transaction.transaction_date;
        let tx_id = transaction.id;
        let mut remaining = Money::of(currency, transaction.amount, mode).amount();
        let mut totals = TransactionAllocationTotals::default();
        let mut mappings = Vec::new();

        // First allocate to Penalties and Fees on installments
        for installment in installments.iter_mut() {
            if remaining <= Decimal::ZERO {
                break;
            }
            pay_installment_in_order(
                installment,
                &[PaymentComponent::Penalty, PaymentComponent::Fee],
                tx_date,
                &mut remaining,
                &mut totals,
                &mut mappings,
                tx_id,
                currency,
                mode,
            );
        }

        // Any remaining amount is allocated via the configured strategy
        if remaining > Decimal::ZERO {
            for installment in installments.iter_mut() {
                if remaining <= Decimal::ZERO {
                    break;
                }
                pay_installment_in_order(
                    installment,
                    &PaymentComponent::STANDARD_ORDER,
                    tx_date,
                    &mut remaining,
                    &mut totals,
                    &mut mappings,
                    tx_id,
                    currency,
                    mode,
                );
            }
        }

        let overpayment = Money::of(currency, remaining.max(Decimal::ZERO), mode);
        transaction.update_components(
            totals.principal,
            totals.interest,
            totals.fees,
            totals.penalties,
        );
        transaction.overpayment_portion = overpayment.amount();
        transaction.repayment_schedule_mappings = mappings;

        Self::apply_paid_charges(charges, totals.fees, totals.penalties);
        overpayment
    }

    /// Processes an interest waiver (`WaiveInterest` or `InterestPaymentWaiver`).
    pub fn process_interest_waiver(
        &self,
        transaction: &mut LoanTransaction,
        installments: &mut [LoanRepaymentScheduleInstallment],
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        transaction.reset_derived_components();
        let tx_date = transaction.transaction_date;
        let tx_id = transaction.id;
        let mut remaining = Money::of(currency, transaction.amount, mode).amount();
        let mut total_waived = Decimal::ZERO;
        let mut mappings = Vec::new();

        for installment in installments.iter_mut() {
            if remaining <= Decimal::ZERO {
                break;
            }
            if installment.interest_outstanding() <= Decimal::ZERO {
                continue;
            }
            let waived = installment.waive_interest(tx_date, remaining);
            if waived > Decimal::ZERO {
                remaining = (remaining - waived).max(Decimal::ZERO);
                total_waived += waived;
                let mut acc = InstallmentMappingAccumulator::new(
                    tx_id,
                    installment.id,
                    installment.installment_number,
                    installment.due_date,
                );
                acc.add_component(PaymentComponent::Interest, waived);
                acc.flush_into(&mut mappings);
            }
        }

        transaction.interest_portion = total_waived;
        transaction.repayment_schedule_mappings = mappings;
    }

    /// Processes a charge waiver (`WaiveCharges`).
    pub fn process_charges_waiver(
        &self,
        transaction: &mut LoanTransaction,
        installments: &mut [LoanRepaymentScheduleInstallment],
        charges: &mut [LoanCharge],
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        transaction.reset_derived_components();
        let tx_date = transaction.transaction_date;
        let tx_id = transaction.id;
        let mut remaining = Money::of(currency, transaction.amount, mode).amount();
        let mut fees_waived = Decimal::ZERO;
        let mut penalties_waived = Decimal::ZERO;
        let mut mappings = Vec::new();

        for installment in installments.iter_mut() {
            if remaining <= Decimal::ZERO {
                break;
            }
            let mut acc = InstallmentMappingAccumulator::new(
                tx_id,
                installment.id,
                installment.installment_number,
                installment.due_date,
            );
            if installment.fee_charges_outstanding() > Decimal::ZERO {
                let waived = installment.waive_fee_charges(tx_date, remaining);
                if waived > Decimal::ZERO {
                    remaining = (remaining - waived).max(Decimal::ZERO);
                    fees_waived += waived;
                    acc.add_component(PaymentComponent::Fee, waived);
                }
            }
            if remaining > Decimal::ZERO && installment.penalty_charges_outstanding() > Decimal::ZERO
            {
                let waived = installment.waive_penalty_charges(tx_date, remaining);
                if waived > Decimal::ZERO {
                    remaining = (remaining - waived).max(Decimal::ZERO);
                    penalties_waived += waived;
                    acc.add_component(PaymentComponent::Penalty, waived);
                }
            }
            acc.flush_into(&mut mappings);
        }

        for charge in charges.iter_mut() {
            if !charge.is_active || charge.waived || charge.is_paid {
                continue;
            }
            if charge.is_penalty
                && penalties_waived > Decimal::ZERO
                && charge.amount_outstanding > Decimal::ZERO
            {
                charge.waive(None);
            } else if !charge.is_penalty
                && fees_waived > Decimal::ZERO
                && charge.amount_outstanding > Decimal::ZERO
            {
                charge.waive(None);
            }
        }

        transaction.fee_charges_portion = fees_waived;
        transaction.penalty_charges_portion = penalties_waived;
        transaction.repayment_schedule_mappings = mappings;
    }

    /// Processes a `WriteOff` transaction across all unpaid installments.
    pub fn process_write_off(
        &self,
        transaction: &mut LoanTransaction,
        installments: &mut [LoanRepaymentScheduleInstallment],
        _currency: &MonetaryCurrency,
        _mode: FineractRoundingMode,
    ) {
        transaction.reset_derived_components();
        let tx_date = transaction.transaction_date;
        let tx_id = transaction.id;

        let mut total_principal = Decimal::ZERO;
        let mut total_interest = Decimal::ZERO;
        let mut total_fees = Decimal::ZERO;
        let mut total_penalties = Decimal::ZERO;
        let mut mappings = Vec::new();

        for installment in installments.iter_mut() {
            if installment.total_outstanding() <= Decimal::ZERO {
                continue;
            }
            let p = installment.write_off_outstanding_principal(tx_date);
            let i = installment.write_off_outstanding_interest(tx_date);
            let f = installment.write_off_outstanding_fee_charges(tx_date);
            let pen = installment.write_off_outstanding_penalty_charges(tx_date);
            total_principal += p;
            total_interest += i;
            total_fees += f;
            total_penalties += pen;

            let mut acc = InstallmentMappingAccumulator::new(
                tx_id,
                installment.id,
                installment.installment_number,
                installment.due_date,
            );
            acc.add_component(PaymentComponent::Principal, p);
            acc.add_component(PaymentComponent::Interest, i);
            acc.add_component(PaymentComponent::Fee, f);
            acc.add_component(PaymentComponent::Penalty, pen);
            acc.flush_into(&mut mappings);
        }

        transaction.update_components(total_principal, total_interest, total_fees, total_penalties);
        let written_off_sum = total_principal + total_interest + total_fees + total_penalties;
        if written_off_sum > Decimal::ZERO {
            transaction.amount = written_off_sum;
        }
        transaction.repayment_schedule_mappings = mappings;
    }

    /// Processes a `Refund` or `RefundForActiveLoan` transaction by undoing paid components
    /// across installments in reverse chronological order (`handleRefund` in Fineract).
    pub fn process_refund(
        &self,
        transaction: &mut LoanTransaction,
        installments: &mut [LoanRepaymentScheduleInstallment],
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        transaction.reset_derived_components();
        let tx_date = transaction.transaction_date;
        let tx_id = transaction.id;
        let mut remaining = Money::of(currency, transaction.amount, mode).amount();
        let mut totals = TransactionAllocationTotals::default();
        let mut mappings = Vec::new();

        // Reverse of strategy order (Fineract refunds Principal -> Interest -> Fees -> Penalties from latest installment backwards)
        let reverse_order: [PaymentComponent; 4] = match self.strategy {
            TransactionProcessorStrategyCode::PrincipalInterestPenaltiesFees => [
                PaymentComponent::Fee,
                PaymentComponent::Penalty,
                PaymentComponent::Interest,
                PaymentComponent::Principal,
            ],
            TransactionProcessorStrategyCode::InterestPrincipalPenaltiesFees
            | TransactionProcessorStrategyCode::EarlyRepayment => [
                PaymentComponent::Fee,
                PaymentComponent::Penalty,
                PaymentComponent::Principal,
                PaymentComponent::Interest,
            ],
            _ => [
                PaymentComponent::Principal,
                PaymentComponent::Interest,
                PaymentComponent::Fee,
                PaymentComponent::Penalty,
            ],
        };

        for installment in installments.iter_mut().rev() {
            if remaining <= Decimal::ZERO {
                break;
            }
            let mut acc = InstallmentMappingAccumulator::new(
                tx_id,
                installment.id,
                installment.installment_number,
                installment.due_date,
            );
            for &component in &reverse_order {
                if remaining <= Decimal::ZERO {
                    break;
                }
                unpay_installment_component(
                    installment,
                    component,
                    tx_date,
                    &mut remaining,
                    &mut totals,
                    &mut acc,
                    currency,
                    mode,
                );
            }
            acc.flush_into(&mut mappings);
        }

        transaction.update_components(
            totals.principal,
            totals.interest,
            totals.fees,
            totals.penalties,
        );
        transaction.overpayment_portion = remaining.max(Decimal::ZERO);
        transaction.repayment_schedule_mappings = mappings;
    }

    /// Processes a `Chargeback` transaction (`AbstractLoanRepaymentScheduleTransactionProcessor.handleChargeback`).
    ///
    /// Credits back principal on the current or latest installment so that the borrower owes
    /// the charged-back amount again.
    pub fn process_chargeback(
        &self,
        transaction: &mut LoanTransaction,
        installments: &mut [LoanRepaymentScheduleInstallment],
        currency: &MonetaryCurrency,
        mode: FineractRoundingMode,
    ) {
        transaction.reset_derived_components();
        let tx_date = transaction.transaction_date;
        let amount = Money::of(currency, transaction.amount, mode).amount();
        if amount <= Decimal::ZERO {
            return;
        }

        // Find the first installment with due_date >= tx_date, or fallback to the last installment
        let target_idx = installments
            .iter()
            .position(|inst| inst.due_date >= tx_date)
            .unwrap_or_else(|| installments.len().saturating_sub(1));

        if let Some(inst) = installments.get_mut(target_idx) {
            inst.principal_amount += amount;
            inst.check_if_repayment_period_obligations_are_met(tx_date);
            transaction.principal_portion = amount;
        }
    }

    fn apply_paid_charges(
        charges: &mut [LoanCharge],
        mut fees_paid: Decimal,
        mut penalties_paid: Decimal,
    ) {
        for charge in charges.iter_mut() {
            if !charge.is_active
                || charge.waived
                || charge.is_paid
                || charge.is_due_at_disbursement()
            {
                continue;
            }
            if charge.is_penalty {
                if penalties_paid > Decimal::ZERO && charge.amount_outstanding > Decimal::ZERO {
                    let paid = charge.pay(penalties_paid, None);
                    penalties_paid = (penalties_paid - paid).max(Decimal::ZERO);
                }
            } else if fees_paid > Decimal::ZERO && charge.amount_outstanding > Decimal::ZERO {
                let paid = charge.pay(fees_paid, None);
                fees_paid = (fees_paid - paid).max(Decimal::ZERO);
            }
        }
    }
}

// ============================================================================
// 5. BackdatedTransactionReplayEngine
// ============================================================================

/// Stateful backdated transaction replay engine.
///
/// Mirrors `AbstractLoanRepaymentScheduleTransactionProcessor.reprocessLoanTransactions`
/// and `LoanRepaymentScheduleProcessingWrapper.reprocess`.
#[derive(Debug, Clone, Default)]
pub struct BackdatedTransactionReplayEngine;

impl BackdatedTransactionReplayEngine {
    /// Reprocesses all loan transactions from scratch when a backdated transaction is inserted,
    /// modified, or reversed.
    ///
    /// ### Steps Performed
    /// 1. Resets installment derived paid/waived/written-off components (`reset_derived_components()`).
    /// 2. Resets and re-applies non-disbursement charges to their corresponding installments.
    /// 3. Sorts transactions chronologically by `(transaction_date, submitted_on_date, id)`.
    /// 4. Replays each non-reversed transaction against `installments` using `strategy`.
    /// 5. Updates each transaction's `principal_portion`, `interest_portion`, `fee_charges_portion`,
    ///    `penalty_charges_portion`, `overpayment_portion`, and `outstanding_loan_balance`.
    /// 6. Returns the total remaining overpayment (`Money`).
    pub fn reprocess_transactions(
        currency: &MonetaryCurrency,
        disbursement_date: NaiveDate,
        installments: &mut [LoanRepaymentScheduleInstallment],
        charges: &mut [LoanCharge],
        transactions: &mut [LoanTransaction],
        strategy: TransactionProcessorStrategyCode,
        mode: FineractRoundingMode,
    ) -> Result<Money, FineractError> {
        let (overpayment, _changes) = Self::reprocess_transactions_with_details(
            currency,
            disbursement_date,
            installments,
            charges,
            transactions,
            strategy,
            mode,
        )?;
        Ok(overpayment)
    }

    /// Reprocesses all transactions and also returns [`ChangedTransactionDetail`] listing any
    /// existing transactions whose derived portions changed due to the backdated replay.
    pub fn reprocess_transactions_with_details(
        currency: &MonetaryCurrency,
        disbursement_date: NaiveDate,
        installments: &mut [LoanRepaymentScheduleInstallment],
        charges: &mut [LoanCharge],
        transactions: &mut [LoanTransaction],
        strategy: TransactionProcessorStrategyCode,
        mode: FineractRoundingMode,
    ) -> Result<(Money, ChangedTransactionDetail), FineractError> {
        // 1. Reset derived components on all installments
        for installment in installments.iter_mut() {
            installment.reset_derived_components();
        }

        // 2. Re-apply non-disbursement charges to installments (if explicit LoanCharge list is supplied)
        Self::reapply_charges_to_installments(
            currency,
            disbursement_date,
            installments,
            charges,
            mode,
        );

        // 3. Sort transactions chronologically by (transaction_date, submitted_on_date, id)
        //    Disbursement transactions on the same date always precede repayments.
        transactions.sort_by(|a, b| {
            a.transaction_date
                .cmp(&b.transaction_date)
                .then_with(|| {
                    Self::transaction_type_sort_rank(a.transaction_type)
                        .cmp(&Self::transaction_type_sort_rank(b.transaction_type))
                })
                .then_with(|| a.submitted_on_date.cmp(&b.submitted_on_date))
                .then_with(|| a.id.unwrap_or(i64::MAX).cmp(&b.id.unwrap_or(i64::MAX)))
        });

        let processor = LoanRepaymentScheduleTransactionProcessor::new(strategy);
        let mut changed_detail = ChangedTransactionDetail::new();
        let mut total_overpaid = Money::zero(currency);

        // Total scheduled principal across installments (used to track running principal balance)
        let initial_principal: Decimal = installments.iter().map(|i| i.principal_amount).sum();
        let mut cumulative_principal_disbursed = Decimal::ZERO;

        for tx in transactions.iter_mut() {
            if tx.is_reversed {
                continue;
            }

            let before_snapshot = TransactionPortionSnapshot::capture(tx);
            let old_tx_clone = tx.clone();

            match tx.transaction_type {
                LoanTransactionType::Disbursement => {
                    cumulative_principal_disbursed += tx.amount;
                    let remaining_principal: Decimal =
                        installments.iter().map(|i| i.principal_outstanding()).sum();
                    tx.outstanding_loan_balance = if remaining_principal > Decimal::ZERO {
                        remaining_principal
                    } else {
                        cumulative_principal_disbursed
                    };
                }
                LoanTransactionType::Repayment
                | LoanTransactionType::DownPayment
                | LoanTransactionType::RecoveryRepayment
                | LoanTransactionType::RepaymentAtDisbursement
                | LoanTransactionType::GoodwillCredit
                | LoanTransactionType::MerchantIssuedRefund
                | LoanTransactionType::PayoutRefund => {
                    let overpaid = processor.process_repayment_transaction(
                        tx,
                        installments,
                        charges,
                        currency,
                        mode,
                    );
                    if overpaid.is_greater_than_zero() {
                        total_overpaid = total_overpaid + overpaid;
                    }
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                LoanTransactionType::ChargePayment => {
                    let overpaid = processor.process_charge_payment(
                        tx,
                        installments,
                        charges,
                        currency,
                        mode,
                    );
                    if overpaid.is_greater_than_zero() {
                        total_overpaid = total_overpaid + overpaid;
                    }
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                LoanTransactionType::WaiveInterest
                | LoanTransactionType::InterestPaymentWaiver => {
                    processor.process_interest_waiver(tx, installments, currency, mode);
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                LoanTransactionType::WaiveCharges => {
                    processor.process_charges_waiver(tx, installments, charges, currency, mode);
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                LoanTransactionType::WriteOff => {
                    processor.process_write_off(tx, installments, currency, mode);
                    total_overpaid = Money::zero(currency);
                    tx.outstanding_loan_balance = Decimal::ZERO;
                }
                LoanTransactionType::ChargeOff => {
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                LoanTransactionType::Refund | LoanTransactionType::RefundForActiveLoan => {
                    processor.process_refund(tx, installments, currency, mode);
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                LoanTransactionType::CreditBalanceRefund => {
                    tx.reset_derived_components();
                    let refund_money = Money::of(currency, tx.amount, mode);
                    tx.overpayment_portion = refund_money.amount();
                    total_overpaid = (total_overpaid - refund_money).zero_if_negative();
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                LoanTransactionType::Chargeback => {
                    processor.process_chargeback(tx, installments, currency, mode);
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                LoanTransactionType::CapitalizedIncome => {
                    tx.principal_portion = Money::of(currency, tx.amount, mode).amount();
                    tx.outstanding_loan_balance =
                        Self::calculate_outstanding_principal(installments);
                }
                _ => {
                    // Non-allocating transactions (Accrual, IncomePosting, InitiateTransfer, etc.)
                    if tx.outstanding_loan_balance == Decimal::ZERO
                        && initial_principal > Decimal::ZERO
                    {
                        tx.outstanding_loan_balance =
                            Self::calculate_outstanding_principal(installments);
                    }
                }
            }

            if before_snapshot.id.is_some() && before_snapshot.differs_from(tx) {
                changed_detail.transaction_changes.push(TransactionChangeData {
                    old_transaction: old_tx_clone,
                    new_transaction: tx.clone(),
                });
                if let Some(id) = tx.id {
                    changed_detail.new_transaction_mappings.insert(id, tx.clone());
                }
            }
        }

        Ok((total_overpaid, changed_detail))
    }

    #[inline]
    fn calculate_outstanding_principal(installments: &[LoanRepaymentScheduleInstallment]) -> Decimal {
        installments
            .iter()
            .map(|inst| inst.principal_outstanding())
            .sum::<Decimal>()
            .max(Decimal::ZERO)
    }

    #[inline]
    const fn transaction_type_sort_rank(tx_type: LoanTransactionType) -> u8 {
        match tx_type {
            LoanTransactionType::Disbursement => 0,
            LoanTransactionType::RepaymentAtDisbursement | LoanTransactionType::DownPayment => 1,
            LoanTransactionType::Accrual | LoanTransactionType::AccrualActivity => 2,
            LoanTransactionType::WaiveCharges | LoanTransactionType::WaiveInterest => 3,
            _ => 4,
        }
    }

    /// Resets non-disbursement [`LoanCharge`] tracking and maps active charges onto `installments`
    /// (`LoanRepaymentScheduleProcessingWrapper.reprocess`).
    ///
    /// Note: When `charges` is empty (e.g., when a caller sets `fee_charges_amount` or
    /// `penalty_charges_amount` directly on installments), existing installment fee/penalty
    /// charged amounts are preserved.
    fn reapply_charges_to_installments(
        currency: &MonetaryCurrency,
        disbursement_date: NaiveDate,
        installments: &mut [LoanRepaymentScheduleInstallment],
        charges: &mut [LoanCharge],
        mode: FineractRoundingMode,
    ) {
        if charges.is_empty() {
            return;
        }

        for charge in charges.iter_mut() {
            if !charge.is_due_at_disbursement() {
                charge.reset_to_original();
            }
        }

        let num_installments = installments.len();
        let down_payment_flags: Vec<bool> = installments.iter().map(|i| i.is_down_payment).collect();
        for (idx, installment) in installments.iter_mut().enumerate() {
            if installment.is_down_payment {
                continue;
            }
            let is_first_normal = idx == 0
                || down_payment_flags[..idx].iter().all(|&prev| prev);
            let period_start = installment.from_date.unwrap_or(disbursement_date);
            let period_end = installment.due_date;

            let mut fee_due = Decimal::ZERO;
            let mut fee_waived = Decimal::ZERO;
            let mut penalty_due = Decimal::ZERO;
            let mut penalty_waived = Decimal::ZERO;

            for charge in charges.iter() {
                if !charge.is_active || charge.is_due_at_disbursement() {
                    continue;
                }
                let belongs_to_installment = if charge.is_instalment_fee() {
                    true
                } else if let Some(charge_due) = charge.due_date {
                    if is_first_normal {
                        charge_due >= period_start && charge_due <= period_end
                    } else if idx + 1 == num_installments && charge_due > period_end {
                        true
                    } else {
                        charge_due > period_start && charge_due <= period_end
                    }
                } else {
                    is_first_normal
                };

                if belongs_to_installment {
                    let amt = if charge.is_instalment_fee() {
                        charge
                            .installment_charges
                            .iter()
                            .find(|ic| ic.installment_number == installment.installment_number)
                            .map(|ic| ic.amount)
                            .unwrap_or(charge.amount)
                    } else {
                        charge.amount
                    };
                    let waived_amt = if charge.waived { amt } else { charge.amount_waived };
                    if charge.is_penalty {
                        penalty_due += amt;
                        penalty_waived += waived_amt;
                    } else {
                        fee_due += amt;
                        fee_waived += waived_amt;
                    }
                }
            }

            installment.fee_charges_amount = Money::of(currency, fee_due, mode).amount();
            installment.fee_charges_waived = Money::of(currency, fee_waived, mode).amount();
            installment.penalty_charges_amount = Money::of(currency, penalty_due, mode).amount();
            installment.penalty_charges_waived =
                Money::of(currency, penalty_waived, mode).amount();
        }
    }
}

// ============================================================================
// 6. Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use fineract_core_rs::primitives::ExternalId;
    use rust_decimal_macros::dec;

    fn usd() -> MonetaryCurrency {
        MonetaryCurrency::new("USD", 2, None)
    }

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn make_installment(
        num: i32,
        from_date: NaiveDate,
        due_date: NaiveDate,
        principal: Decimal,
        interest: Decimal,
        fees: Decimal,
        penalties: Decimal,
    ) -> LoanRepaymentScheduleInstallment {
        let mut inst = LoanRepaymentScheduleInstallment::new(
            Some(1),
            num,
            Some(from_date),
            due_date,
            principal,
            interest,
            fees,
            penalties,
        );
        inst.id = Some(i64::from(num));
        inst
    }

    fn make_repayment_tx(id: i64, tx_date: NaiveDate, amount: Decimal) -> LoanTransaction {
        let mut tx = LoanTransaction::repayment(
            Some(1),
            Some(1),
            tx_date,
            amount,
            ExternalId::empty(),
        );
        tx.id = Some(id);
        tx.submitted_on_date = tx_date;
        tx
    }

    #[test]
    fn strategy_code_lookup_and_display_roundtrip() {
        for &strategy in TransactionProcessorStrategyCode::ALL {
            assert_eq!(
                TransactionProcessorStrategyCode::from_code(strategy.code()),
                Some(strategy)
            );
            assert_eq!(
                TransactionProcessorStrategyCode::from_code(strategy.name()),
                Some(strategy)
            );
            assert_eq!(
                strategy.to_string().parse::<TransactionProcessorStrategyCode>(),
                Ok(strategy)
            );
        }
        assert_eq!(
            TransactionProcessorStrategyCode::MifosStandard.code(),
            "mifos-standard-strategy"
        );
        assert_eq!(
            TransactionProcessorStrategyCode::RbiIndia.code(),
            "rbi-india-strategy"
        );
        assert_eq!(
            TransactionProcessorStrategyCode::PrincipalInterestPenaltiesFees.code(),
            "principal-interest-penalties-fees-order-strategy"
        );
    }

    #[test]
    fn compare_mifos_standard_vs_pipf_on_single_installment_partial_payment() {
        let currency = usd();
        let mode = FineractRoundingMode::HalfEven;
        // Installment 1: Principal = 1000, Interest = 100, Fee = 50, Penalty = 25 (Total = 1175)
        // Partial payment of 200 on due date 2026-02-01:
        // - MifosStandard (Penalties -> Fees -> Interest -> Principal):
        //   Penalty = 25, Fee = 50, Interest = 100, Principal = 25
        // - PrincipalInterestPenaltiesFees (Principal -> Interest -> Penalties -> Fees):
        //   Principal = 200, Interest = 0, Penalty = 0, Fee = 0
        let mut mifos_installments = vec![make_installment(
            1,
            d(2026, 1, 1),
            d(2026, 2, 1),
            dec!(1000.00),
            dec!(100.00),
            dec!(50.00),
            dec!(25.00),
        )];
        let mut pipf_installments = mifos_installments.clone();

        let mut mifos_tx = make_repayment_tx(10, d(2026, 2, 1), dec!(200.00));
        let mut pipf_tx = make_repayment_tx(10, d(2026, 2, 1), dec!(200.00));
        let mut charges = Vec::new();

        let mifos_processor = LoanRepaymentScheduleTransactionProcessor::new(
            TransactionProcessorStrategyCode::MifosStandard,
        );
        let pipf_processor = LoanRepaymentScheduleTransactionProcessor::new(
            TransactionProcessorStrategyCode::PrincipalInterestPenaltiesFees,
        );

        let over_mifos = mifos_processor.process_repayment_transaction(
            &mut mifos_tx,
            &mut mifos_installments,
            &mut charges,
            &currency,
            mode,
        );
        let over_pipf = pipf_processor.process_repayment_transaction(
            &mut pipf_tx,
            &mut pipf_installments,
            &mut charges,
            &currency,
            mode,
        );

        assert!(over_mifos.is_zero());
        assert_eq!(mifos_tx.penalty_charges_portion, dec!(25.00));
        assert_eq!(mifos_tx.fee_charges_portion, dec!(50.00));
        assert_eq!(mifos_tx.interest_portion, dec!(100.00));
        assert_eq!(mifos_tx.principal_portion, dec!(25.00));
        assert_eq!(mifos_installments[0].principal_outstanding(), dec!(975.00));

        assert!(over_pipf.is_zero());
        assert_eq!(pipf_tx.principal_portion, dec!(200.00));
        assert_eq!(pipf_tx.interest_portion, dec!(0.00));
        assert_eq!(pipf_tx.penalty_charges_portion, dec!(0.00));
        assert_eq!(pipf_tx.fee_charges_portion, dec!(0.00));
        assert_eq!(pipf_installments[0].principal_outstanding(), dec!(800.00));
        assert_eq!(pipf_installments[0].interest_outstanding(), dec!(100.00));
    }

    #[test]
    fn compare_mifos_standard_vs_rbi_india_on_multi_installment_overdue_payment() {
        let currency = usd();
        let mode = FineractRoundingMode::HalfEven;
        // Two installments, both overdue/due on 2026-03-01:
        // Inst 1 (due 2026-02-01, overdue): Principal = 500, Interest = 50, Fee = 20, Penalty = 10
        // Inst 2 (due 2026-03-01, due today): Principal = 500, Interest = 50, Fee = 20, Penalty = 10
        // Borrower pays 200 on 2026-03-01:
        // - MifosStandard settles Inst 1 first:
        //   Inst 1: Penalty 10 + Fee 20 + Interest 50 + Principal 120 = 200
        //   Total: Penalty = 10, Fee = 20, Interest = 50, Principal = 120
        // - RbiIndia settles all overdue/due Penalties, Fees, and Interest across Inst 1 AND Inst 2 first:
        //   Inst 1: Penalty 10 + Fee 20 + Interest 50 = 80
        //   Inst 2: Penalty 10 + Fee 20 + Interest 50 = 80
        //   Remaining 40 goes to Inst 1 Principal = 40
        //   Total: Penalty = 20, Fee = 40, Interest = 100, Principal = 40
        let base_schedule = vec![
            make_installment(
                1,
                d(2026, 1, 1),
                d(2026, 2, 1),
                dec!(500.00),
                dec!(50.00),
                dec!(20.00),
                dec!(10.00),
            ),
            make_installment(
                2,
                d(2026, 2, 1),
                d(2026, 3, 1),
                dec!(500.00),
                dec!(50.00),
                dec!(20.00),
                dec!(10.00),
            ),
        ];

        let mut mifos_schedule = base_schedule.clone();
        let mut rbi_schedule = base_schedule;
        let mut charges = Vec::new();

        let mut mifos_tx = make_repayment_tx(1, d(2026, 3, 1), dec!(200.00));
        let mut rbi_tx = make_repayment_tx(1, d(2026, 3, 1), dec!(200.00));

        LoanRepaymentScheduleTransactionProcessor::new(
            TransactionProcessorStrategyCode::MifosStandard,
        )
        .process_repayment_transaction(
            &mut mifos_tx,
            &mut mifos_schedule,
            &mut charges,
            &currency,
            mode,
        );

        LoanRepaymentScheduleTransactionProcessor::new(TransactionProcessorStrategyCode::RbiIndia)
            .process_repayment_transaction(
                &mut rbi_tx,
                &mut rbi_schedule,
                &mut charges,
                &currency,
                mode,
            );

        // MifosStandard assertions
        assert_eq!(mifos_tx.penalty_charges_portion, dec!(10.00));
        assert_eq!(mifos_tx.fee_charges_portion, dec!(20.00));
        assert_eq!(mifos_tx.interest_portion, dec!(50.00));
        assert_eq!(mifos_tx.principal_portion, dec!(120.00));
        assert_eq!(mifos_schedule[0].principal_outstanding(), dec!(380.00));
        assert_eq!(mifos_schedule[1].interest_outstanding(), dec!(50.00));

        // RbiIndia assertions
        assert_eq!(rbi_tx.penalty_charges_portion, dec!(20.00));
        assert_eq!(rbi_tx.fee_charges_portion, dec!(40.00));
        assert_eq!(rbi_tx.interest_portion, dec!(100.00));
        assert_eq!(rbi_tx.principal_portion, dec!(40.00));
        assert_eq!(rbi_schedule[0].interest_outstanding(), dec!(0.00));
        assert_eq!(rbi_schedule[1].interest_outstanding(), dec!(0.00));
        assert_eq!(rbi_schedule[0].principal_outstanding(), dec!(460.00));
        assert_eq!(rbi_schedule[1].principal_outstanding(), dec!(500.00));
    }

    #[test]
    fn heavens_family_in_advance_principal_payment_waives_installment_interest() {
        let currency = usd();
        let mode = FineractRoundingMode::HalfEven;
        // Inst 1 due 2026-02-01: Principal = 500, Interest = 50
        // Payment of 500 made in advance on 2026-01-15:
        // HeavensFamily pays Principal (500) first and automatically waives the 50 Interest!
        let mut installments = vec![make_installment(
            1,
            d(2026, 1, 1),
            d(2026, 2, 1),
            dec!(500.00),
            dec!(50.00),
            dec!(0.00),
            dec!(0.00),
        )];
        let mut charges = Vec::new();
        let mut tx = make_repayment_tx(1, d(2026, 1, 15), dec!(500.00));

        let overpaid = LoanRepaymentScheduleTransactionProcessor::new(
            TransactionProcessorStrategyCode::HeavensFamily,
        )
        .process_repayment_transaction(&mut tx, &mut installments, &mut charges, &currency, mode);

        assert!(overpaid.is_zero());
        assert_eq!(tx.principal_portion, dec!(500.00));
        assert_eq!(tx.interest_portion, dec!(0.00));
        assert_eq!(installments[0].principal_outstanding(), dec!(0.00));
        assert_eq!(installments[0].interest_waived, dec!(50.00));
        assert_eq!(installments[0].interest_outstanding(), dec!(0.00));
        assert!(installments[0].obligations_met);
    }

    #[test]
    fn early_repayment_strategy_allocates_excess_directly_to_principal() {
        let currency = usd();
        let mode = FineractRoundingMode::HalfEven;
        // Inst 1 (due 2026-02-01): Principal = 500, Interest = 50
        // Inst 2 (due 2026-03-01): Principal = 500, Interest = 50
        // Payment of 750 on 2026-02-01 (Inst 1 due date):
        // - Inst 1 (due today): Interest (50) + Principal (500) = 550
        // - Excess 200 (in advance of Inst 2): allocated directly to Inst 2 Principal (200), NOT Inst 2 Interest!
        let mut installments = vec![
            make_installment(
                1,
                d(2026, 1, 1),
                d(2026, 2, 1),
                dec!(500.00),
                dec!(50.00),
                dec!(0.00),
                dec!(0.00),
            ),
            make_installment(
                2,
                d(2026, 2, 1),
                d(2026, 3, 1),
                dec!(500.00),
                dec!(50.00),
                dec!(0.00),
                dec!(0.00),
            ),
        ];
        let mut charges = Vec::new();
        let mut tx = make_repayment_tx(1, d(2026, 2, 1), dec!(750.00));

        LoanRepaymentScheduleTransactionProcessor::new(
            TransactionProcessorStrategyCode::EarlyRepayment,
        )
        .process_repayment_transaction(&mut tx, &mut installments, &mut charges, &currency, mode);

        assert_eq!(tx.interest_portion, dec!(50.00));
        assert_eq!(tx.principal_portion, dec!(700.00));
        assert_eq!(installments[1].principal_outstanding(), dec!(300.00));
        assert_eq!(installments[1].interest_outstanding(), dec!(50.00));
    }

    #[test]
    fn backdated_transaction_insertion_and_reversal_replay_updates_all_portions_and_balances() {
        let currency = usd();
        let mode = FineractRoundingMode::HalfEven;
        let disbursement_date = d(2026, 1, 1);

        let mut installments = vec![
            make_installment(
                1,
                d(2026, 1, 1),
                d(2026, 2, 1),
                dec!(500.00),
                dec!(50.00),
                dec!(20.00),
                dec!(10.00),
            ),
            make_installment(
                2,
                d(2026, 2, 1),
                d(2026, 3, 1),
                dec!(500.00),
                dec!(50.00),
                dec!(20.00),
                dec!(10.00),
            ),
        ];
        let mut charges = Vec::new();

        // Step 1: Initial transaction on 2026-03-01 for 300.00 (submitted 2026-03-01, id = 10)
        // Plus a backdated repayment on 2026-02-01 for 580.00 (submitted 2026-03-02, id = 11)
        // placed out-of-order in the slice to verify chronological sorting & replay!
        let mut disb = LoanTransaction::disbursement(
            Some(1),
            Some(1),
            disbursement_date,
            dec!(1000.00),
            ExternalId::empty(),
        );
        disb.id = Some(1);
        disb.submitted_on_date = disbursement_date;

        let mut tx_march = make_repayment_tx(10, d(2026, 3, 1), dec!(300.00));
        tx_march.submitted_on_date = d(2026, 3, 1);

        let mut tx_feb_backdated = make_repayment_tx(11, d(2026, 2, 1), dec!(580.00));
        tx_feb_backdated.submitted_on_date = d(2026, 3, 2);

        let mut transactions = vec![disb, tx_march, tx_feb_backdated];

        let overpaid = BackdatedTransactionReplayEngine::reprocess_transactions(
            &currency,
            disbursement_date,
            &mut installments,
            &mut charges,
            &mut transactions,
            TransactionProcessorStrategyCode::MifosStandard,
            mode,
        )
        .expect("replay should succeed");

        assert!(overpaid.is_zero());
        // Verify transactions were sorted chronologically: [id=1 (Jan 1), id=11 (Feb 1), id=10 (Mar 1)]
        assert_eq!(transactions[0].id, Some(1));
        assert_eq!(transactions[1].id, Some(11));
        assert_eq!(transactions[2].id, Some(10));

        // Backdated Feb 1 payment of 580 completely pays Installment 1 (10 pen + 20 fee + 50 int + 500 prin = 580)
        assert_eq!(transactions[1].penalty_charges_portion, dec!(10.00));
        assert_eq!(transactions[1].fee_charges_portion, dec!(20.00));
        assert_eq!(transactions[1].interest_portion, dec!(50.00));
        assert_eq!(transactions[1].principal_portion, dec!(500.00));
        assert_eq!(transactions[1].outstanding_loan_balance, dec!(500.00));
        assert!(installments[0].obligations_met);

        // Subsequent Mar 1 payment of 300 now allocates against Installment 2 (10 pen + 20 fee + 50 int + 220 prin = 300)
        assert_eq!(transactions[2].penalty_charges_portion, dec!(10.00));
        assert_eq!(transactions[2].fee_charges_portion, dec!(20.00));
        assert_eq!(transactions[2].interest_portion, dec!(50.00));
        assert_eq!(transactions[2].principal_portion, dec!(220.00));
        assert_eq!(transactions[2].outstanding_loan_balance, dec!(280.00));

        // Step 2: Now reverse the backdated Feb 1 transaction (id=11) and add a large repayment on Mar 5
        // that causes an overpayment of 150.00, then replay!
        transactions[1].reverse(d(2026, 3, 3));
        let mut tx_payoff = make_repayment_tx(12, d(2026, 3, 5), dec!(1010.00));
        tx_payoff.submitted_on_date = d(2026, 3, 5);
        transactions.push(tx_payoff);

        let overpaid_after_reversal = BackdatedTransactionReplayEngine::reprocess_transactions(
            &currency,
            disbursement_date,
            &mut installments,
            &mut charges,
            &mut transactions,
            TransactionProcessorStrategyCode::MifosStandard,
            mode,
        )
        .expect("replay after reversal should succeed");

        // Total schedule obligation across Inst 1 + Inst 2 = 580 + 580 = 1160.
        // Active repayments: Mar 1 (300) + Mar 5 (1010) = 1310 -> Overpayment = 150.00!
        assert_eq!(overpaid_after_reversal.amount(), dec!(150.00));
        assert!(installments[0].obligations_met);
        assert!(installments[1].obligations_met);
        assert_eq!(
            transactions.last().unwrap().overpayment_portion,
            dec!(150.00)
        );
        assert_eq!(
            transactions.last().unwrap().outstanding_loan_balance,
            dec!(0.00)
        );
    }
}
