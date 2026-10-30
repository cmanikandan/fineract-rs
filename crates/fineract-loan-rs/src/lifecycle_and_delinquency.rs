//! Loan Account Lifecycle State Machine, Delinquency Classification Engine, and `LoanAggregate`.
//!
//! Faithfully ported from `org.apache.fineract.portfolio.loanaccount.domain`,
//! `org.apache.fineract.portfolio.loanaccount.service`, and
//! `org.apache.fineract.portfolio.delinquency`:
//! - [`LoanStatus`] (`LoanStatus.java`, `m_loan.loan_status_id`)
//! - [`LoanSubStatus`] (`LoanSubStatus.java`, `m_loan.loan_sub_status_id`)
//! - [`LoanEvent`] and [`DefaultLoanLifecycleStateMachine`] (`LoanEvent.java`, `DefaultLoanLifecycleStateMachine.java`)
//! - [`LoanChargeOffBehaviour`] (`LoanChargeOffBehaviour.java`)
//! - [`DelinquencyRange`] (`DelinquencyRange.java`, `m_delinquency_range`)
//! - [`DelinquencyBucket`] and [`DelinquencyBucketType`] (`DelinquencyBucket.java`, `m_delinquency_bucket`)
//! - [`LoanDelinquencyTagHistory`] (`LoanDelinquencyTagHistory.java`, `m_loan_delinquency_tag_history`)
//! - [`LoanDelinquencyAction`] and [`DelinquencyPausePeriod`] (`LoanDelinquencyAction.java`, `m_loan_delinquency_action`)
//! - [`CollectionData`] and [`InstallmentLevelDelinquency`] (`CollectionData.java`)
//! - [`DelinquencyCalculationEngine`] (`LoanDelinquencyDomainServiceImpl.java`, `DelinquencyEffectivePauseHelperImpl.java`, `DelinquencyWritePlatformServiceImpl.java`)
//! - [`LoanAggregate`] (`Loan.java`, `m_loan` aggregate root and state machine)

use std::fmt;

use chrono::NaiveDate;
use fineract_charge_rs::EnumOptionData;
use fineract_core_rs::primitives::ExternalId;
use fineract_money::{Decimal, FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{
    FineractError, PlatformApiDataValidationException, PlatformDomainRuleException,
};
use serde::{Deserialize, Serialize};

use crate::schedule_and_terms::{
    LoanCharge, LoanRepaymentScheduleInstallment, LoanSummary, LoanTermVariations, LoanTransaction,
    LoanTransactionType,
};
use crate::transaction_processor::{
    BackdatedTransactionReplayEngine, TransactionProcessorStrategyCode,
};

// =============================================================================
// 1. LoanStatus (LoanStatus.java)
// =============================================================================

/// Representation of the lifecycle status of a loan account (`m_loan.loan_status_id`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum LoanStatus {
    #[default]
    Invalid = 0,
    SubmittedAndPendingApproval = 100,
    Approved = 200,
    Active = 300,
    TransferInProgress = 303,
    TransferOnHold = 304,
    WithdrawnByClient = 400,
    Rejected = 500,
    ClosedObligationsMet = 600,
    ClosedWrittenOff = 601,
    ClosedRescheduleOutstandingAmount = 602,
    Overpaid = 700,
}

impl LoanStatus {
    pub const INVALID: Self = Self::Invalid;
    pub const SUBMITTED_AND_PENDING_APPROVAL: Self = Self::SubmittedAndPendingApproval;
    pub const APPROVED: Self = Self::Approved;
    pub const ACTIVE: Self = Self::Active;
    pub const TRANSFER_IN_PROGRESS: Self = Self::TransferInProgress;
    pub const TRANSFER_ON_HOLD: Self = Self::TransferOnHold;
    pub const WITHDRAWN_BY_CLIENT: Self = Self::WithdrawnByClient;
    pub const REJECTED: Self = Self::Rejected;
    pub const CLOSED_OBLIGATIONS_MET: Self = Self::ClosedObligationsMet;
    pub const CLOSED_WRITTEN_OFF: Self = Self::ClosedWrittenOff;
    pub const CLOSED_RESCHEDULE_OUTSTANDING_AMOUNT: Self = Self::ClosedRescheduleOutstandingAmount;
    pub const OVERPAID: Self = Self::Overpaid;

    /// Numeric status code stored in `m_loan.loan_status_id`.
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Numeric status code as `i32` (`getValue()` in Java).
    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Upstream Fineract enum code (`getCode()` in Java).
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "loanStatusType.invalid",
            Self::SubmittedAndPendingApproval => "loanStatusType.submitted.and.pending.approval",
            Self::Approved => "loanStatusType.approved",
            Self::Active => "loanStatusType.active",
            Self::TransferInProgress => "loanStatusType.transfer.in.progress",
            Self::TransferOnHold => "loanStatusType.transfer.on.hold",
            Self::WithdrawnByClient => "loanStatusType.withdrawn.by.client",
            Self::Rejected => "loanStatusType.rejected",
            Self::ClosedObligationsMet => "loanStatusType.closed.obligations.met",
            Self::ClosedWrittenOff => "loanStatusType.closed.written.off",
            Self::ClosedRescheduleOutstandingAmount => {
                "loanStatusType.closed.reschedule.outstanding.amount"
            }
            Self::Overpaid => "loanStatusType.overpaid",
        }
    }

    /// Human-readable description of the loan status.
    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::SubmittedAndPendingApproval => "Submitted and pending approval",
            Self::Approved => "Approved",
            Self::Active => "Active",
            Self::TransferInProgress => "Transfer in progress",
            Self::TransferOnHold => "Transfer on hold",
            Self::WithdrawnByClient => "Withdrawn by applicant",
            Self::Rejected => "Rejected",
            Self::ClosedObligationsMet => "Closed (obligations met)",
            Self::ClosedWrittenOff => "Closed (written off)",
            Self::ClosedRescheduleOutstandingAmount => "Closed (rescheduled)",
            Self::Overpaid => "Overpaid",
        }
    }

    /// Resolves a `LoanStatus` from its database integer code (`LoanStatus.fromInt`).
    #[inline]
    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::SubmittedAndPendingApproval,
            200 => Self::Approved,
            300 => Self::Active,
            303 => Self::TransferInProgress,
            304 => Self::TransferOnHold,
            400 => Self::WithdrawnByClient,
            500 => Self::Rejected,
            600 => Self::ClosedObligationsMet,
            601 => Self::ClosedWrittenOff,
            602 => Self::ClosedRescheduleOutstandingAmount,
            700 => Self::Overpaid,
            _ => Self::Invalid,
        }
    }

    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        Self::from_i32(value as i32)
    }

    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    #[inline]
    pub const fn has_state_of(self, state: Self) -> bool {
        self as i16 == state as i16
    }

    #[inline]
    pub const fn is_submitted_and_pending_approval(self) -> bool {
        matches!(self, Self::SubmittedAndPendingApproval)
    }

    #[inline]
    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }

    #[inline]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    #[inline]
    pub const fn is_closed(self) -> bool {
        self.is_closed_obligations_met()
            || self.is_closed_written_off()
            || self.is_closed_with_outsanding_amount_marked_for_reschedule()
    }

    #[inline]
    pub const fn is_closed_obligations_met(self) -> bool {
        matches!(self, Self::ClosedObligationsMet)
    }

    #[inline]
    pub const fn is_closed_written_off(self) -> bool {
        matches!(self, Self::ClosedWrittenOff)
    }

    #[inline]
    pub const fn is_closed_with_outsanding_amount_marked_for_reschedule(self) -> bool {
        matches!(self, Self::ClosedRescheduleOutstandingAmount)
    }

    #[inline]
    pub const fn is_withdrawn_by_client(self) -> bool {
        matches!(self, Self::WithdrawnByClient)
    }

    #[inline]
    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    #[inline]
    pub const fn is_active_or_awaiting_approval_or_disbursal(self) -> bool {
        self.is_approved() || self.is_submitted_and_pending_approval() || self.is_active()
    }

    #[inline]
    pub const fn is_transfer_in_progress(self) -> bool {
        matches!(self, Self::TransferInProgress)
    }

    #[inline]
    pub const fn is_transfer_on_hold(self) -> bool {
        matches!(self, Self::TransferOnHold)
    }

    #[inline]
    pub const fn is_under_transfer(self) -> bool {
        self.is_transfer_in_progress() || self.is_transfer_on_hold()
    }

    #[inline]
    pub const fn is_overpaid(self) -> bool {
        matches!(self, Self::Overpaid)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

impl fmt::Display for LoanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

// =============================================================================
// 2. LoanSubStatus (LoanSubStatus.java)
// =============================================================================

/// Sub-status refinement on a loan account (`m_loan.loan_sub_status_id`).
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanSubStatus`.
/// Includes `Restructured = 300` as well as upstream `Foreclosed = 100`, `Claimed = 200`,
/// `ContractTermination = 900`, and `LoanWithdrawal = 901`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum LoanSubStatus {
    #[default]
    Invalid = 0,
    Foreclosed = 100,
    Claimed = 200,
    Restructured = 300,
    ContractTermination = 900,
    LoanWithdrawal = 901,
}

impl LoanSubStatus {
    pub const INVALID: Self = Self::Invalid;
    pub const FORECLOSED: Self = Self::Foreclosed;
    pub const CLAIMED: Self = Self::Claimed;
    pub const RESTRUCTURED: Self = Self::Restructured;
    pub const CONTRACT_TERMINATION: Self = Self::ContractTermination;
    pub const LOAN_WITHDRAWAL: Self = Self::LoanWithdrawal;

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
            Self::Invalid => "loanSubStatus.invalid",
            Self::Foreclosed => "loanSubStatus.foreclosed",
            Self::Claimed => "loanSubStatus.claimed",
            Self::Restructured => "loanSubStatus.restructured",
            Self::ContractTermination => "loanSubStatus.contractTermination",
            Self::LoanWithdrawal => "loanSubStatus.loanWithdrawal",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Foreclosed => "Foreclosed",
            Self::Claimed => "Claimed",
            Self::Restructured => "Restructured",
            Self::ContractTermination => "Contract termination",
            Self::LoanWithdrawal => "Loan withdrawal",
        }
    }

    #[inline]
    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::Foreclosed,
            200 => Self::Claimed,
            300 => Self::Restructured,
            900 => Self::ContractTermination,
            901 => Self::LoanWithdrawal,
            _ => Self::Invalid,
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
    pub const fn is_foreclosed(self) -> bool {
        matches!(self, Self::Foreclosed)
    }

    #[inline]
    pub const fn is_claimed(self) -> bool {
        matches!(self, Self::Claimed)
    }

    #[inline]
    pub const fn is_restructured(self) -> bool {
        matches!(self, Self::Restructured)
    }

    #[inline]
    pub const fn is_contract_termination(self) -> bool {
        matches!(self, Self::ContractTermination)
    }

    #[inline]
    pub const fn is_loan_withdrawal(self) -> bool {
        matches!(self, Self::LoanWithdrawal)
    }

    pub fn to_enum_option_data(self) -> EnumOptionData {
        EnumOptionData::new(i64::from(self.value()), self.code(), self.description())
    }
}

impl fmt::Display for LoanSubStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

// =============================================================================
// 3. LoanEvent, DefaultLoanLifecycleStateMachine, and LoanChargeOffBehaviour
// =============================================================================

/// Domain events that drive transitions in [`DefaultLoanLifecycleStateMachine`].
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.LoanEvent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoanEvent {
    LoanCreated,
    LoanRejected,
    LoanWithdrawn,
    LoanApproved,
    LoanApprovalUndo,
    LoanDisbursed,
    LoanDisbursalUndo,
    LoanRepaymentOrWaiver,
    RepaidInFull,
    WriteOffOutstanding,
    WriteOffOutstandingUndo,
    RescheduleOutstandingAmount,
    LoanOverpayment,
    LoanClosed,
    LoanInitiateTransfer,
    LoanApproveTransfer,
    LoanWithdrawTransfer,
    LoanRejectTransfer,
    LoanRefund,
    LoanCreditBalanceRefund,
    LoanChargeback,
}

/// Pure transition table implementing `DefaultLoanLifecycleStateMachine.java`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultLoanLifecycleStateMachine;

impl DefaultLoanLifecycleStateMachine {
    /// Computes the next [`LoanStatus`] when `loan_event` occurs from `from`.
    ///
    /// Mirrors `DefaultLoanLifecycleStateMachine.dryTransition(LoanEvent, LoanStatus)`.
    #[inline]
    pub const fn dry_transition(loan_event: LoanEvent, from: LoanStatus) -> LoanStatus {
        match loan_event {
            LoanEvent::LoanCreated => match from {
                LoanStatus::Invalid => LoanStatus::SubmittedAndPendingApproval,
                other => other,
            },
            LoanEvent::LoanRejected => match from {
                LoanStatus::SubmittedAndPendingApproval => LoanStatus::Rejected,
                other => other,
            },
            LoanEvent::LoanApproved => match from {
                LoanStatus::SubmittedAndPendingApproval => LoanStatus::Approved,
                other => other,
            },
            LoanEvent::LoanWithdrawn => match from {
                LoanStatus::SubmittedAndPendingApproval => LoanStatus::WithdrawnByClient,
                other => other,
            },
            LoanEvent::LoanDisbursed => match from {
                LoanStatus::Approved | LoanStatus::ClosedObligationsMet | LoanStatus::Overpaid => {
                    LoanStatus::Active
                }
                other => other,
            },
            LoanEvent::LoanApprovalUndo => match from {
                LoanStatus::Approved => LoanStatus::SubmittedAndPendingApproval,
                other => other,
            },
            LoanEvent::LoanDisbursalUndo => match from {
                LoanStatus::Active
                | LoanStatus::ClosedObligationsMet
                | LoanStatus::Overpaid => LoanStatus::Approved,
                other => other,
            },
            LoanEvent::LoanRepaymentOrWaiver
            | LoanEvent::LoanRefund
            | LoanEvent::LoanChargeback => match from {
                LoanStatus::ClosedObligationsMet | LoanStatus::Overpaid => LoanStatus::Active,
                other => other,
            },
            LoanEvent::RepaidInFull => match from {
                LoanStatus::Active | LoanStatus::Overpaid => LoanStatus::ClosedObligationsMet,
                other => other,
            },
            LoanEvent::WriteOffOutstanding => match from {
                LoanStatus::Active => LoanStatus::ClosedWrittenOff,
                other => other,
            },
            LoanEvent::WriteOffOutstandingUndo => match from {
                LoanStatus::ClosedWrittenOff => LoanStatus::Active,
                other => other,
            },
            LoanEvent::RescheduleOutstandingAmount => match from {
                LoanStatus::Active => LoanStatus::ClosedRescheduleOutstandingAmount,
                other => other,
            },
            LoanEvent::LoanOverpayment => match from {
                LoanStatus::ClosedObligationsMet | LoanStatus::Active => LoanStatus::Overpaid,
                other => other,
            },
            LoanEvent::LoanClosed => LoanStatus::ClosedObligationsMet,
            LoanEvent::LoanInitiateTransfer => LoanStatus::TransferInProgress,
            LoanEvent::LoanApproveTransfer => match from {
                LoanStatus::TransferInProgress => LoanStatus::Active,
                other => other,
            },
            LoanEvent::LoanWithdrawTransfer => match from {
                LoanStatus::TransferInProgress => LoanStatus::Active,
                other => other,
            },
            LoanEvent::LoanRejectTransfer => match from {
                LoanStatus::TransferInProgress => LoanStatus::TransferOnHold,
                other => other,
            },
            LoanEvent::LoanCreditBalanceRefund => from,
        }
    }
}

/// Charge-off behaviour configured for a loan (`LoanChargeOffBehaviour.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LoanChargeOffBehaviour {
    /// Standard charge-off behaviour (`REGULAR`).
    #[default]
    Regular,
    /// Zeroes interest and stops further interest accrual after charge-off (`ZERO_INTEREST`).
    ZeroInterest,
    /// Accelerates maturity to the charge-off date (`ACCELERATE_MATURITY`).
    AccelerateMaturity,
}

impl LoanChargeOffBehaviour {
    #[inline]
    pub const fn is_zero_interest(self) -> bool {
        matches!(self, Self::ZeroInterest)
    }

    #[inline]
    pub const fn is_accelerate_maturity(self) -> bool {
        matches!(self, Self::AccelerateMaturity)
    }
}

// =============================================================================
// 4. Delinquency & Collection Domain Types
// =============================================================================

/// Age range within a [`DelinquencyBucket`] (`m_delinquency_range`).
///
/// Ported from `org.apache.fineract.portfolio.delinquency.domain.DelinquencyRange`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelinquencyRange {
    pub id: i64,
    pub classification: String,
    pub minimum_age_days: i32,
    pub maximum_age_days: Option<i32>,
}

impl DelinquencyRange {
    pub fn new(
        id: i64,
        classification: impl Into<String>,
        minimum_age_days: i32,
        maximum_age_days: Option<i32>,
    ) -> Result<Self, FineractError> {
        let classification = classification.into();
        if classification.trim().is_empty() {
            return Err(PlatformApiDataValidationException::single_error(
                "validation.msg.delinquency.range.classification.cannot.be.blank",
                "Delinquency range classification cannot be blank",
                "classification",
            )
            .into());
        }
        if minimum_age_days < 0 {
            return Err(PlatformApiDataValidationException::single_error(
                "validation.msg.delinquency.range.minimumAgeDays.must.be.non.negative",
                "Delinquency range minimum_age_days must be non-negative",
                "minimumAgeDays",
            )
            .into());
        }
        if let Some(max_days) = maximum_age_days {
            if max_days < minimum_age_days {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.delinquency.range.invalid.age.days",
                    format!(
                        "Delinquency range maximum_age_days ({max_days}) cannot be less than minimum_age_days ({minimum_age_days})"
                    ),
                )
                .into());
            }
        }
        Ok(Self {
            id,
            classification,
            minimum_age_days,
            maximum_age_days,
        })
    }

    /// Unchecked constructor for tests and static definitions.
    pub fn of(
        id: i64,
        classification: impl Into<String>,
        minimum_age_days: i32,
        maximum_age_days: Option<i32>,
    ) -> Self {
        Self {
            id,
            classification: classification.into(),
            minimum_age_days,
            maximum_age_days,
        }
    }

    /// Returns `true` if `delinquent_days` falls within `[minimum_age_days, maximum_age_days]`.
    #[inline]
    pub fn matches_age_days(&self, delinquent_days: i64) -> bool {
        if delinquent_days <= 0 {
            return false;
        }
        let min = i64::from(self.minimum_age_days);
        if delinquent_days < min {
            return false;
        }
        match self.maximum_age_days {
            Some(max) => delinquent_days <= i64::from(max),
            None => true,
        }
    }
}

/// Bucket type (`REGULAR` or `WORKING_CAPITAL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DelinquencyBucketType {
    #[default]
    Regular,
    WorkingCapital,
}

/// Ordered collection of [`DelinquencyRange`] rules (`m_delinquency_bucket`).
///
/// Ported from `org.apache.fineract.portfolio.delinquency.domain.DelinquencyBucket`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelinquencyBucket {
    pub id: i64,
    pub name: String,
    pub bucket_type: DelinquencyBucketType,
    pub ranges: Vec<DelinquencyRange>,
}

impl DelinquencyBucket {
    /// Creates a new [`DelinquencyBucket`], sorting ranges by `minimum_age_days` and validating
    /// that there are no overlapping age windows (`DelinquencyWritePlatformServiceImpl.validateDelinquencyRanges`).
    pub fn new(
        id: i64,
        name: impl Into<String>,
        mut ranges: Vec<DelinquencyRange>,
    ) -> Result<Self, FineractError> {
        ranges.sort_by_key(|r| (r.minimum_age_days, r.maximum_age_days.unwrap_or(i32::MAX)));
        for window in ranges.windows(2) {
            let prev = &window[0];
            let next = &window[1];
            match prev.maximum_age_days {
                None => {
                    return Err(PlatformDomainRuleException::new(
                        "error.msg.delinquency.bucket.ranges.overlap",
                        format!(
                            "Delinquency range '{}' has no maximum age and overlaps with '{}'",
                            prev.classification, next.classification
                        ),
                    )
                    .into());
                }
                Some(prev_max) if prev_max >= next.minimum_age_days => {
                    return Err(PlatformDomainRuleException::new(
                        "error.msg.delinquency.bucket.ranges.overlap",
                        format!(
                            "Delinquency range '{}' ({}..={}) overlaps with '{}' ({}..)",
                            prev.classification,
                            prev.minimum_age_days,
                            prev_max,
                            next.classification,
                            next.minimum_age_days
                        ),
                    )
                    .into());
                }
                _ => {}
            }
        }
        Ok(Self {
            id,
            name: name.into(),
            bucket_type: DelinquencyBucketType::Regular,
            ranges,
        })
    }

    /// Unchecked constructor sorting ranges by `minimum_age_days`.
    pub fn of(id: i64, name: impl Into<String>, mut ranges: Vec<DelinquencyRange>) -> Self {
        ranges.sort_by_key(|r| (r.minimum_age_days, r.maximum_age_days.unwrap_or(i32::MAX)));
        Self {
            id,
            name: name.into(),
            bucket_type: DelinquencyBucketType::Regular,
            ranges,
        }
    }

    /// Resolves the matching [`DelinquencyRange`] for `delinquent_days` (`DelinquencyWritePlatformServiceImpl`).
    pub fn resolve_range(&self, delinquent_days: i64) -> Option<&DelinquencyRange> {
        if delinquent_days <= 0 {
            return None;
        }
        let mut matched: Option<&DelinquencyRange> = None;
        for range in &self.ranges {
            if range.matches_age_days(delinquent_days) {
                matched = Some(range);
                break;
            }
        }
        matched
    }
}

/// Audit trail of delinquency range assignments on a loan (`m_loan_delinquency_tag_history`).
///
/// Ported from `org.apache.fineract.portfolio.delinquency.domain.LoanDelinquencyTagHistory`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanDelinquencyTagHistory {
    pub id: Option<i64>,
    pub loan_id: i64,
    pub delinquency_range_id: i64,
    pub delinquency_range_classification: String,
    pub added_on_date: NaiveDate,
    pub lifted_on_date: Option<NaiveDate>,
}

impl LoanDelinquencyTagHistory {
    pub fn new(
        id: Option<i64>,
        loan_id: i64,
        range: &DelinquencyRange,
        added_on_date: NaiveDate,
    ) -> Self {
        Self {
            id,
            loan_id,
            delinquency_range_id: range.id,
            delinquency_range_classification: range.classification.clone(),
            added_on_date,
            lifted_on_date: None,
        }
    }

    #[inline]
    pub const fn is_active(&self) -> bool {
        self.lifted_on_date.is_none()
    }

    pub fn lift(&mut self, lifted_on_date: NaiveDate) {
        self.lifted_on_date = Some(lifted_on_date);
    }
}

/// Delinquency pause/resume action recorded in `m_loan_delinquency_action`.
///
/// Ported from `org.apache.fineract.portfolio.delinquency.domain.LoanDelinquencyAction`
/// and `DelinquencyAction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoanDelinquencyAction {
    /// Pauses delinquency calculation over `[start_date, end_date)`.
    Pause {
        id: Option<i64>,
        start_date: NaiveDate,
        end_date: NaiveDate,
    },
    /// Resumes delinquency calculation starting at `start_date`, truncating any active pause.
    Resume {
        id: Option<i64>,
        start_date: NaiveDate,
    },
}

impl LoanDelinquencyAction {
    pub const fn pause(id: Option<i64>, start_date: NaiveDate, end_date: NaiveDate) -> Self {
        Self::Pause {
            id,
            start_date,
            end_date,
        }
    }

    pub const fn resume(id: Option<i64>, start_date: NaiveDate) -> Self {
        Self::Resume { id, start_date }
    }

    #[inline]
    pub const fn start_date(&self) -> NaiveDate {
        match self {
            Self::Pause { start_date, .. } | Self::Resume { start_date, .. } => *start_date,
        }
    }

    #[inline]
    pub const fn end_date(&self) -> Option<NaiveDate> {
        match self {
            Self::Pause { end_date, .. } => Some(*end_date),
            Self::Resume { .. } => None,
        }
    }

    #[inline]
    pub const fn is_pause(&self) -> bool {
        matches!(self, Self::Pause { .. })
    }

    #[inline]
    pub const fn is_resume(&self) -> bool {
        matches!(self, Self::Resume { .. })
    }
}

/// Effective resolved delinquency pause period (`LoanDelinquencyActionData`).
///
/// Mirrors the effective pause intervals computed by `DelinquencyEffectivePauseHelperImpl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelinquencyPausePeriod {
    pub active: bool,
    pub pause_period_start: NaiveDate,
    pub pause_period_end: NaiveDate,
}

impl DelinquencyPausePeriod {
    /// Counts how many days in `[overdue_since_date, business_date)` overlap with
    /// `[pause_period_start, pause_period_end)`.
    ///
    /// Mirrors `LoanDelinquencyDomainServiceImpl.getPausedDaysWithinRange`.
    pub fn overlapped_paused_days(
        &self,
        overdue_since_date: NaiveDate,
        business_date: NaiveDate,
    ) -> i64 {
        let start = overdue_since_date.max(self.pause_period_start);
        let end = business_date.min(self.pause_period_end);
        let diff = (end - start).num_days();
        diff.max(0)
    }
}

/// Per-installment delinquency breakdown (`InstallmentLevelDelinquency.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallmentLevelDelinquency {
    pub installment_number: i32,
    pub due_date: NaiveDate,
    pub past_due_days: i64,
    pub delinquent_days: i64,
    pub range_id: Option<i64>,
    pub classification: Option<String>,
    pub minimum_age_days: Option<i32>,
    pub maximum_age_days: Option<i32>,
    pub delinquent_amount: Money,
    pub delinquent_principal: Money,
    pub delinquent_interest: Money,
    pub delinquent_fee: Money,
    pub delinquent_penalty: Money,
}

/// Delinquency and collection metrics for a loan on a given `business_date` (`CollectionData.java`).
///
/// Ported from `org.apache.fineract.portfolio.delinquency.data.CollectionData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionData {
    pub available_disbursement_amount: Money,
    pub available_disbursement_amount_with_over_applied: Money,
    pub past_due_days: i64,
    pub next_payment_due_date: Option<NaiveDate>,
    pub delinquent_date: Option<NaiveDate>,
    pub past_due_date: Option<NaiveDate>,
    pub delinquent_days: i64,
    pub delinquent_amount: Money,
    pub delinquent_principal: Money,
    pub delinquent_interest: Money,
    pub delinquent_fee: Money,
    pub delinquent_penalty: Money,
    pub last_payment_date: Option<NaiveDate>,
    pub last_payment_amount: Money,
    pub last_repayment_date: Option<NaiveDate>,
    pub last_repayment_amount: Money,
    pub delinquency_range_id: Option<i64>,
    pub delinquency_classification: Option<String>,
    pub delinquency_pause_periods: Vec<DelinquencyPausePeriod>,
    pub installment_level_delinquency: Vec<InstallmentLevelDelinquency>,
}

impl CollectionData {
    /// Creates an empty/zeroed [`CollectionData`] instance (`CollectionData.template()`).
    pub fn template(currency: MonetaryCurrency, mode: FineractRoundingMode) -> Self {
        let zero = Money::zero(currency, mode);
        Self {
            available_disbursement_amount: zero,
            available_disbursement_amount_with_over_applied: zero,
            past_due_days: 0,
            next_payment_due_date: None,
            delinquent_date: None,
            past_due_date: None,
            delinquent_days: 0,
            delinquent_amount: zero,
            delinquent_principal: zero,
            delinquent_interest: zero,
            delinquent_fee: zero,
            delinquent_penalty: zero,
            last_payment_date: None,
            last_payment_amount: zero,
            last_repayment_date: None,
            last_repayment_amount: zero,
            delinquency_range_id: None,
            delinquency_classification: None,
            delinquency_pause_periods: Vec::new(),
            installment_level_delinquency: Vec::new(),
        }
    }
}

// =============================================================================
// 5. DelinquencyCalculationEngine
// =============================================================================

/// Computes [`CollectionData`], effective pause periods, active [`DelinquencyRange`],
/// and [`LoanDelinquencyTagHistory`] transitions on a given `business_date`.
///
/// Ported from:
/// - `org.apache.fineract.portfolio.delinquency.service.LoanDelinquencyDomainServiceImpl`
/// - `org.apache.fineract.portfolio.delinquency.helper.DelinquencyEffectivePauseHelperImpl`
/// - `org.apache.fineract.portfolio.delinquency.service.DelinquencyWritePlatformServiceImpl`
#[derive(Debug, Clone, Default)]
pub struct DelinquencyCalculationEngine;

impl DelinquencyCalculationEngine {
    /// Resolves raw [`LoanDelinquencyAction`] items (`Pause` and `Resume`) into non-overlapping
    /// effective [`DelinquencyPausePeriod`] windows on `business_date`.
    ///
    /// Mirrors `DelinquencyEffectivePauseHelperImpl.calculateEffectiveDelinquencyList`.
    pub fn calculate_effective_pause_periods(
        actions: &[LoanDelinquencyAction],
        business_date: NaiveDate,
    ) -> Vec<DelinquencyPausePeriod> {
        let mut effective = Vec::new();
        let mut idx = 0;
        while idx < actions.len() {
            if let LoanDelinquencyAction::Pause {
                start_date,
                end_date,
                ..
            } = actions[idx]
            {
                let effective_end = match actions.get(idx + 1) {
                    Some(LoanDelinquencyAction::Resume {
                        start_date: resume_date,
                        ..
                    }) => {
                        idx += 1;
                        (*resume_date).min(end_date)
                    }
                    _ => end_date,
                };
                if effective_end > start_date {
                    let active = start_date <= business_date && business_date <= effective_end;
                    effective.push(DelinquencyPausePeriod {
                        active,
                        pause_period_start: start_date,
                        pause_period_end: effective_end,
                    });
                }
            }
            idx += 1;
        }
        effective
    }

    /// Validates a new [`LoanDelinquencyAction`] before appending it to a loan (`DelinquencyActionParseAndValidator`).
    pub fn validate_delinquency_action(
        loan_status: LoanStatus,
        disbursement_date: Option<NaiveDate>,
        business_date: NaiveDate,
        existing_actions: &[LoanDelinquencyAction],
        new_action: &LoanDelinquencyAction,
    ) -> Result<(), FineractError> {
        if !loan_status.is_active() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.delinquency.action.invalid.loan.state",
                "Delinquency actions can only be performed on an Active loan",
            )
            .into());
        }
        if let Some(disb_date) = disbursement_date {
            if new_action.start_date() < disb_date {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.delinquency.action.start.date.before.disbursement",
                    "Delinquency action start date cannot be before first disbursement date",
                )
                .into());
            }
        }
        let effective_pauses =
            Self::calculate_effective_pause_periods(existing_actions, business_date);

        match new_action {
            LoanDelinquencyAction::Pause {
                start_date,
                end_date,
                ..
            } => {
                if end_date <= start_date {
                    return Err(PlatformDomainRuleException::new(
                        "error.msg.delinquency.action.invalid.end.date",
                        "Delinquency pause end date must be after start date",
                    )
                    .into());
                }
                for p in &effective_pauses {
                    if *start_date < p.pause_period_end && *end_date > p.pause_period_start {
                        return Err(PlatformDomainRuleException::new(
                            "error.msg.delinquency.action.overlapping.pause.period",
                            "Delinquency pause period cannot overlap with an existing pause period",
                        )
                        .into());
                    }
                }
            }
            LoanDelinquencyAction::Resume { start_date, .. } => {
                if *start_date != business_date {
                    return Err(PlatformDomainRuleException::new(
                        "error.msg.delinquency.action.invalid.start.date",
                        "Delinquency resume start date must match the current business date",
                    )
                    .into());
                }
                let has_active_pause = effective_pauses.iter().any(|p| p.active);
                if !has_active_pause {
                    return Err(PlatformDomainRuleException::new(
                        "error.msg.delinquency.action.resume.without.active.pause",
                        "Cannot resume delinquency when there is no active pause period",
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    /// Computes [`CollectionData`] and resolves the active [`DelinquencyRange`] for `loan`
    /// as of `business_date`.
    ///
    /// Mirrors `LoanDelinquencyDomainServiceImpl.getOverdueCollectionData`.
    pub fn calculate_collection_data<'a>(
        loan: &LoanAggregate,
        business_date: NaiveDate,
        bucket: Option<&'a DelinquencyBucket>,
    ) -> (CollectionData, Option<&'a DelinquencyRange>) {
        let currency = loan.currency;
        let mode = loan.rounding_mode;
        let zero = Money::zero(currency, mode);
        let mut data = CollectionData::template(currency, mode);

        let effective_pauses =
            Self::calculate_effective_pause_periods(&loan.delinquency_actions, business_date);
        data.delinquency_pause_periods = effective_pauses.clone();

        let available_disb =
            (loan.approved_principal - loan.summary.total_principal_disbursed).max(Decimal::ZERO);
        data.available_disbursement_amount = Money::of(currency, available_disb, mode);
        data.available_disbursement_amount_with_over_applied =
            data.available_disbursement_amount;

        // Populate last payment / repayment info from non-reversed transactions
        for tx in &loan.transactions {
            if tx.is_reversed {
                continue;
            }
            if tx.is_repayment_like_type() {
                let is_later = data
                    .last_payment_date
                    .map_or(true, |d| tx.transaction_date >= d);
                if is_later {
                    data.last_payment_date = Some(tx.transaction_date);
                    data.last_payment_amount = Money::of(currency, tx.amount, mode);
                }
            }
            if tx.is_repayment() {
                let is_later = data
                    .last_repayment_date
                    .map_or(true, |d| tx.transaction_date >= d);
                if is_later {
                    data.last_repayment_date = Some(tx.transaction_date);
                    data.last_repayment_amount = Money::of(currency, tx.amount, mode);
                }
            }
        }

        // Next payment due date: first unpaid installment with due_date >= business_date (or earliest unpaid)
        data.next_payment_due_date = loan
            .installments
            .iter()
            .filter(|i| !i.is_down_payment && i.is_not_fully_paid_off())
            .find(|i| i.due_date >= business_date)
            .or_else(|| {
                loan.installments
                    .iter()
                    .find(|i| !i.is_down_payment && i.is_not_fully_paid_off())
            })
            .map(|i| i.due_date);

        // Only Active (non-charged-off) loans accumulate overdue collection data
        if !loan.status.is_active() || loan.is_charged_off {
            return (data, None);
        }

        // Check if there is a chargeback on a previously met installment that is past due
        let mut oldest_overdue_date: Option<NaiveDate> = None;

        for inst in &loan.installments {
            if !inst.is_not_fully_paid_off() {
                continue;
            }
            if inst.due_date < business_date {
                // Determine effective overdue start date for this installment
                // (Wait: if a chargeback occurred after inst.due_date, Fineract uses the chargeback date if obligations were previously met)
                let inst_overdue_date =
                    Self::effective_installment_overdue_date(inst, &loan.transactions);
                if inst_overdue_date < business_date {
                    oldest_overdue_date = Some(match oldest_overdue_date {
                        Some(prev) => prev.min(inst_overdue_date),
                        None => inst_overdue_date,
                    });

                    let inst_principal = Money::of(currency, inst.principal_outstanding(), mode);
                    let inst_interest = Money::of(currency, inst.interest_outstanding(), mode);
                    let inst_fee = Money::of(currency, inst.fee_charges_outstanding(), mode);
                    let inst_penalty =
                        Money::of(currency, inst.penalty_charges_outstanding(), mode);
                    let inst_total = inst_principal
                        .plus(inst_interest, mode)
                        .plus(inst_fee, mode)
                        .plus(inst_penalty, mode);

                    data.delinquent_principal =
                        data.delinquent_principal.plus(inst_principal, mode);
                    data.delinquent_interest = data.delinquent_interest.plus(inst_interest, mode);
                    data.delinquent_fee = data.delinquent_fee.plus(inst_fee, mode);
                    data.delinquent_penalty = data.delinquent_penalty.plus(inst_penalty, mode);
                    data.delinquent_amount = data.delinquent_amount.plus(inst_total, mode);

                    let inst_past_due_days = (business_date - inst_overdue_date).num_days().max(0);
                    let inst_paused_days: i64 = effective_pauses
                        .iter()
                        .map(|p| p.overlapped_paused_days(inst_overdue_date, business_date))
                        .sum();
                    let inst_delinquent_days = (inst_past_due_days - inst_paused_days).max(0);
                    let inst_range = bucket.and_then(|b| b.resolve_range(inst_delinquent_days));

                    if inst_total > zero {
                        data.installment_level_delinquency
                            .push(InstallmentLevelDelinquency {
                                installment_number: inst.installment_number,
                                due_date: inst.due_date,
                                past_due_days: inst_past_due_days,
                                delinquent_days: inst_delinquent_days,
                                range_id: inst_range.map(|r| r.id),
                                classification: inst_range.map(|r| r.classification.clone()),
                                minimum_age_days: inst_range.map(|r| r.minimum_age_days),
                                maximum_age_days: inst_range.and_then(|r| r.maximum_age_days),
                                delinquent_amount: inst_total,
                                delinquent_principal: inst_principal,
                                delinquent_interest: inst_interest,
                                delinquent_fee: inst_fee,
                                delinquent_penalty: inst_penalty,
                            });
                    }
                }
            }
        }

        if let Some(overdue_since) = oldest_overdue_date {
            let past_due_days = (business_date - overdue_since).num_days().max(0);
            let paused_days: i64 = effective_pauses
                .iter()
                .map(|p| p.overlapped_paused_days(overdue_since, business_date))
                .sum();
            let effective_delinquent_days = (past_due_days - paused_days).max(0);
            let grace_days = i64::from(loan.grace_on_arrears_ageing.max(0));
            let net_delinquent_days = (effective_delinquent_days - grace_days).max(0);

            data.past_due_date = Some(overdue_since);
            data.past_due_days = past_due_days;
            data.delinquent_date =
                overdue_since.checked_add_signed(chrono::Duration::days(paused_days + grace_days));
            data.delinquent_days = net_delinquent_days;
        }

        let active_range = bucket.and_then(|b| b.resolve_range(data.delinquent_days));
        if let Some(range) = active_range {
            data.delinquency_range_id = Some(range.id);
            data.delinquency_classification = Some(range.classification.clone());
        }

        (data, active_range)
    }

    /// Evaluates delinquency for `loan` on `business_date` and updates `loan.delinquency_tag_history`
    /// (`DelinquencyWritePlatformServiceImpl.applyDelinquencyTagToLoan`).
    ///
    /// Note: When an active pause period covers `business_date`, tag transitions are frozen
    /// just as in Fineract's `DelinquencyWritePlatformServiceImpl`.
    pub fn evaluate_and_tag_loan<'a>(
        loan: &mut LoanAggregate,
        business_date: NaiveDate,
        bucket: Option<&'a DelinquencyBucket>,
    ) -> (CollectionData, Option<&'a DelinquencyRange>) {
        let (collection_data, active_range) =
            Self::calculate_collection_data(loan, business_date, bucket);

        let is_paused_today = collection_data
            .delinquency_pause_periods
            .iter()
            .any(|p| p.active);

        if is_paused_today {
            return (collection_data, active_range);
        }

        let loan_id = loan.id.unwrap_or(0);
        match active_range {
            Some(new_range) => {
                // Check if the loan already has this exact range active
                let current_active_range_id = loan
                    .delinquency_tag_history
                    .iter()
                    .find(|t| t.is_active())
                    .map(|t| t.delinquency_range_id);

                if current_active_range_id != Some(new_range.id) {
                    // Lift any previous active tag
                    for tag in &mut loan.delinquency_tag_history {
                        if tag.is_active() {
                            tag.lift(business_date);
                        }
                    }
                    let next_id = (loan.delinquency_tag_history.len() as i64) + 1;
                    loan.delinquency_tag_history
                        .push(LoanDelinquencyTagHistory::new(
                            Some(next_id),
                            loan_id,
                            new_range,
                            business_date,
                        ));
                }
            }
            None => {
                // Loan is out of delinquency: lift any active tag
                for tag in &mut loan.delinquency_tag_history {
                    if tag.is_active() {
                        tag.lift(business_date);
                    }
                }
            }
        }

        (collection_data, active_range)
    }

    fn effective_installment_overdue_date(
        inst: &LoanRepaymentScheduleInstallment,
        transactions: &[LoanTransaction],
    ) -> NaiveDate {
        // If there was a Chargeback after inst.due_date, and obligations were previously met before it,
        // the overdue clock starts from the earliest chargeback after inst.due_date.
        if let Some(met_date) = inst.obligations_met_on_date {
            if met_date <= inst.due_date {
                if let Some(cb_date) = transactions
                    .iter()
                    .filter(|t| t.is_chargeback() && t.transaction_date > inst.due_date)
                    .map(|t| t.transaction_date)
                    .min()
                {
                    return cb_date;
                }
            }
        }
        inst.due_date
    }
}

// =============================================================================
// 6. LoanAggregate (Loan.java — m_loan Domain Aggregate & State Machine)
// =============================================================================

/// Domain aggregate representing a Fineract loan account (`m_loan`) and its full
/// lifecycle state machine.
///
/// Ported from `org.apache.fineract.portfolio.loanaccount.domain.Loan` and associated
/// write platform / domain services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanAggregate {
    pub id: Option<i64>,
    pub account_no: String,
    pub external_id: ExternalId,
    pub client_id: Option<i64>,
    pub group_id: Option<i64>,
    pub office_id: Option<i64>,
    pub product_id: i64,
    pub status: LoanStatus,
    pub sub_status: Option<LoanSubStatus>,
    pub currency: MonetaryCurrency,
    pub rounding_mode: FineractRoundingMode,
    pub transaction_strategy: TransactionProcessorStrategyCode,
    pub charge_off_behaviour: LoanChargeOffBehaviour,

    // Principal & Term amounts
    pub proposed_principal: Decimal,
    pub approved_principal: Decimal,
    pub net_disbursal_amount: Decimal,
    pub grace_on_arrears_ageing: i32,

    // Lifecycle dates (`m_loan`)
    pub submitted_on_date: Option<NaiveDate>,
    pub approved_on_date: Option<NaiveDate>,
    pub expected_disbursed_on_date: Option<NaiveDate>,
    pub actual_disbursement_date: Option<NaiveDate>,
    pub expected_maturity_date: Option<NaiveDate>,
    pub actual_maturity_date: Option<NaiveDate>,
    pub closed_on_date: Option<NaiveDate>,
    pub written_off_on_date: Option<NaiveDate>,
    pub rescheduled_on_date: Option<NaiveDate>,
    pub withdrawn_on_date: Option<NaiveDate>,
    pub rejected_on_date: Option<NaiveDate>,
    pub overpaid_on_date: Option<NaiveDate>,
    pub charged_off_on_date: Option<NaiveDate>,
    pub is_charged_off: bool,
    pub is_fraud: bool,
    pub enable_down_payment: bool,
    pub disbursed_amount_percentage_for_down_payment: Option<Decimal>,
    pub enable_auto_repayment_for_down_payment: bool,
    pub accrued_till: Option<NaiveDate>,

    // Child collections & derived summary
    pub installments: Vec<LoanRepaymentScheduleInstallment>,
    pub charges: Vec<LoanCharge>,
    pub transactions: Vec<LoanTransaction>,
    pub term_variations: Vec<LoanTermVariations>,
    pub delinquency_actions: Vec<LoanDelinquencyAction>,
    pub delinquency_tag_history: Vec<LoanDelinquencyTagHistory>,
    pub summary: LoanSummary,
    next_transaction_id: i64,
}

impl LoanAggregate {
    /// Creates a new unsubmitted [`LoanAggregate`] (`LoanStatus::Invalid`).
    /// Call [`LoanAggregate::submit_application`] to transition to `SubmittedAndPendingApproval`.
    pub fn new(
        id: Option<i64>,
        account_no: impl Into<String>,
        product_id: i64,
        currency: MonetaryCurrency,
        proposed_principal: Decimal,
        installments: Vec<LoanRepaymentScheduleInstallment>,
        transaction_strategy: TransactionProcessorStrategyCode,
        rounding_mode: FineractRoundingMode,
    ) -> Self {
        let expected_maturity_date = installments.last().map(|i| i.due_date);
        let mut summary = LoanSummary::new();
        summary.update_summary(Decimal::ZERO, &installments, &[], Decimal::ZERO);

        Self {
            id,
            account_no: account_no.into(),
            external_id: ExternalId::empty(),
            client_id: None,
            group_id: None,
            office_id: Some(1),
            product_id,
            status: LoanStatus::Invalid,
            sub_status: None,
            currency,
            rounding_mode,
            transaction_strategy,
            charge_off_behaviour: LoanChargeOffBehaviour::Regular,
            proposed_principal,
            approved_principal: Decimal::ZERO,
            net_disbursal_amount: Decimal::ZERO,
            grace_on_arrears_ageing: 0,
            submitted_on_date: None,
            approved_on_date: None,
            expected_disbursed_on_date: None,
            actual_disbursement_date: None,
            expected_maturity_date,
            actual_maturity_date: expected_maturity_date,
            closed_on_date: None,
            written_off_on_date: None,
            rescheduled_on_date: None,
            withdrawn_on_date: None,
            rejected_on_date: None,
            overpaid_on_date: None,
            charged_off_on_date: None,
            is_charged_off: false,
            is_fraud: false,
            enable_down_payment: false,
            disbursed_amount_percentage_for_down_payment: None,
            enable_auto_repayment_for_down_payment: false,
            accrued_till: None,
            installments,
            charges: Vec::new(),
            transactions: Vec::new(),
            term_variations: Vec::new(),
            delinquency_actions: Vec::new(),
            delinquency_tag_history: Vec::new(),
            summary,
            next_transaction_id: 1,
        }
    }

    // =========================================================================
    // Application & Approval Lifecycle Transitions
    // =========================================================================

    /// Submits a loan application (`Invalid -> SubmittedAndPendingApproval`).
    ///
    /// Mirrors `Loan.loanApplicationSubmittal` and `LoanApplicationWritePlatformServiceJpaRepositoryImpl.submitApplication`.
    pub fn submit_application(
        &mut self,
        submitted_on_date: NaiveDate,
        expected_disbursement_date: Option<NaiveDate>,
    ) -> Result<(), FineractError> {
        if self.status != LoanStatus::Invalid {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.submit.application.not.allowed",
                format!(
                    "Cannot submit loan application when loan is in state {}",
                    self.status
                ),
            )
            .into());
        }
        if self.proposed_principal <= Decimal::ZERO {
            return Err(PlatformApiDataValidationException::single_error(
                "validation.msg.loan.principal.must.be.greater.than.zero",
                "Loan proposed principal must be greater than zero",
                "principal",
            )
            .into());
        }
        if let Some(exp_disb) = expected_disbursement_date {
            if exp_disb < submitted_on_date {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.loan.expected.disbursement.date.cannot.be.before.submitted.date",
                    "Expected disbursement date cannot be before submitted on date",
                )
                .into());
            }
        }
        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::LoanCreated,
            self.status,
        );
        self.submitted_on_date = Some(submitted_on_date);
        self.expected_disbursed_on_date = expected_disbursement_date;
        self.approved_principal = self.proposed_principal;
        self.summary
            .update_summary(Decimal::ZERO, &self.installments, &self.charges, Decimal::ZERO);
        Ok(())
    }

    /// Approves a submitted loan application (`SubmittedAndPendingApproval -> Approved`).
    ///
    /// Mirrors `Loan.loanApplicationApproval`.
    pub fn approve(
        &mut self,
        approved_on_date: NaiveDate,
        approved_amount: Option<Decimal>,
    ) -> Result<(), FineractError> {
        if !self.status.is_submitted_and_pending_approval() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.approve.not.in.submitted.and.pending.approval.state",
                "Loan can only be approved from Submitted and pending approval state",
            )
            .into());
        }
        if let Some(submitted_on) = self.submitted_on_date {
            if approved_on_date < submitted_on {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.loan.approval.date.cannot.be.before.submitted.date",
                    "The date on which a loan is approved cannot be earlier than its submittal date",
                )
                .into());
            }
        }
        let amount = approved_amount.unwrap_or(self.proposed_principal);
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.approved.amount.must.be.greater.than.zero",
                "Approved principal amount must be greater than zero",
            )
            .into());
        }
        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::LoanApproved,
            self.status,
        );
        self.approved_on_date = Some(approved_on_date);
        self.approved_principal = amount;
        Ok(())
    }

    /// Reverts loan approval (`Approved -> SubmittedAndPendingApproval`).
    ///
    /// Mirrors `Loan.undoApproval`.
    pub fn undo_approval(&mut self) -> Result<(), FineractError> {
        if !self.status.is_approved() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.undo.approval.not.in.approved.state",
                "Undo approval is only allowed when the loan is in Approved state",
            )
            .into());
        }
        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::LoanApprovalUndo,
            self.status,
        );
        self.approved_on_date = None;
        self.approved_principal = self.proposed_principal;
        Ok(())
    }

    /// Rejects a submitted loan application (`SubmittedAndPendingApproval -> Rejected`).
    ///
    /// Mirrors `Loan.loanApplicationRejection`.
    pub fn reject(&mut self, rejected_on_date: NaiveDate) -> Result<(), FineractError> {
        if !self.status.is_submitted_and_pending_approval() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.reject.not.in.submitted.and.pending.approval.state",
                "Only a loan in Submitted and pending approval state can be rejected",
            )
            .into());
        }
        if let Some(submitted_on) = self.submitted_on_date {
            if rejected_on_date < submitted_on {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.loan.rejection.date.cannot.be.before.submitted.date",
                    "Loan rejection date cannot be earlier than its submittal date",
                )
                .into());
            }
        }
        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::LoanRejected,
            self.status,
        );
        self.rejected_on_date = Some(rejected_on_date);
        self.closed_on_date = Some(rejected_on_date);
        Ok(())
    }

    /// Withdraws a submitted loan application (`SubmittedAndPendingApproval -> WithdrawnByClient`).
    ///
    /// Mirrors `Loan.loanApplicationWithdrawnByApplicant`.
    pub fn withdraw_by_client(
        &mut self,
        withdrawn_on_date: NaiveDate,
    ) -> Result<(), FineractError> {
        if !self.status.is_submitted_and_pending_approval() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.withdraw.not.in.submitted.and.pending.approval.state",
                "Only a loan in Submitted and pending approval state can be withdrawn by client",
            )
            .into());
        }
        if let Some(submitted_on) = self.submitted_on_date {
            if withdrawn_on_date < submitted_on {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.loan.withdrawn.date.cannot.be.before.submitted.date",
                    "Loan withdrawal date cannot be earlier than its submittal date",
                )
                .into());
            }
        }
        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::LoanWithdrawn,
            self.status,
        );
        self.withdrawn_on_date = Some(withdrawn_on_date);
        self.closed_on_date = Some(withdrawn_on_date);
        Ok(())
    }

    // =========================================================================
    // Disbursal & Undo Disbursal
    // =========================================================================

    /// Disburses the loan (`Approved -> Active`, or multi-tranche disbursal on `Active` / `ClosedObligationsMet` / `Overpaid`).
    ///
    /// Mirrors `Loan.disburse` and `LoanWritePlatformServiceJpaRepositoryImpl.disburseLoan`.
    /// Also creates an automatic `DownPayment` transaction if `enable_down_payment` and
    /// `enable_auto_repayment_for_down_payment` are configured.
    pub fn disburse(
        &mut self,
        disbursement_date: NaiveDate,
        disbursed_amount: Option<Decimal>,
    ) -> Result<LoanTransaction, FineractError> {
        if !(self.status.is_approved()
            || self.status.is_active()
            || self.status.is_closed_obligations_met()
            || self.status.is_overpaid())
        {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.disburse.not.in.approved.state",
                format!("Loan cannot be disbursed when in state {}", self.status),
            )
            .into());
        }
        if self.is_charged_off {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.disburse.not.allowed.on.charged.off.loan",
                "Disbursement is not allowed on a charged-off loan",
            )
            .into());
        }
        if let Some(approved_on) = self.approved_on_date {
            if disbursement_date < approved_on {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.loan.disbursal.date.should.be.after.approved.date",
                    "The date on which a loan is disbursed cannot be earlier than its approval date",
                )
                .into());
            }
        }
        if self.installments.is_empty() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.disburse.repayment.schedule.empty",
                "Cannot disburse a loan without repayment schedule installments",
            )
            .into());
        }

        let amount = disbursed_amount.unwrap_or_else(|| {
            if self.approved_principal > Decimal::ZERO {
                self.approved_principal
            } else {
                self.installments.iter().map(|i| i.principal_amount).sum()
            }
        });
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.disbursal.amount.must.be.greater.than.zero",
                "Disbursement amount must be greater than zero",
            )
            .into());
        }

        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::LoanDisbursed,
            self.status,
        );
        if self.actual_disbursement_date.is_none() {
            self.actual_disbursement_date = Some(disbursement_date);
        }
        self.net_disbursal_amount += amount;
        self.closed_on_date = None;

        let mut disb_tx = LoanTransaction::disbursement(
            self.id,
            self.office_id,
            disbursement_date,
            amount,
            ExternalId::empty(),
        );
        self.assign_tx_id(&mut disb_tx);
        self.transactions.push(disb_tx.clone());

        // Handle due-at-disbursement charges (`RepaymentAtDisbursement`)
        let mut disb_fees = Decimal::ZERO;
        let mut disb_penalties = Decimal::ZERO;
        for charge in &mut self.charges {
            if charge.is_active && charge.is_due_at_disbursement() && !charge.is_paid {
                let charge_amt = charge.amount_outstanding;
                if charge_amt > Decimal::ZERO {
                    charge.pay(charge_amt, None);
                    if charge.is_penalty {
                        disb_penalties += charge_amt;
                    } else {
                        disb_fees += charge_amt;
                    }
                }
            }
        }
        if disb_fees + disb_penalties > Decimal::ZERO {
            let mut disb_charge_tx = LoanTransaction::new(
                self.id,
                self.office_id,
                LoanTransactionType::RepaymentAtDisbursement,
                disbursement_date,
                disb_fees + disb_penalties,
            );
            disb_charge_tx.fee_charges_portion = disb_fees;
            disb_charge_tx.penalty_charges_portion = disb_penalties;
            self.assign_tx_id(&mut disb_charge_tx);
            self.transactions.push(disb_charge_tx);
        }

        // Auto down-payment support (`LoanDownPaymentHandlerServiceImpl`)
        if self.enable_down_payment && self.enable_auto_repayment_for_down_payment {
            let down_payment_amt = self
                .installments
                .iter()
                .find(|i| i.is_down_payment && i.is_not_fully_paid_off())
                .map(|i| i.principal_outstanding())
                .or_else(|| {
                    self.disbursed_amount_percentage_for_down_payment.map(|pct| {
                        Money::of(
                            self.currency,
                            amount * pct / Decimal::from(100),
                            self.rounding_mode,
                        )
                        .amount()
                    })
                })
                .unwrap_or(Decimal::ZERO);

            if down_payment_amt > Decimal::ZERO {
                let mut dp_tx = LoanTransaction::down_payment(
                    self.id,
                    self.office_id,
                    disbursement_date,
                    down_payment_amt,
                    ExternalId::empty(),
                );
                self.assign_tx_id(&mut dp_tx);
                self.transactions.push(dp_tx);
            }
        }

        self.reprocess_and_update_summary(disbursement_date)?;
        Ok(self.find_tx_or_default(disb_tx.id, disb_tx))
    }

    /// Reverts loan disbursal (`Active -> Approved`), clearing disbursement transactions and
    /// resetting installment derived components.
    ///
    /// Mirrors `Loan.undoDisbursal`.
    pub fn undo_disbursal(&mut self) -> Result<(), FineractError> {
        if !self.status.is_active() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.undo.disbursal.not.in.active.state",
                "Undo disbursal is only allowed when the loan is in Active state",
            )
            .into());
        }
        let has_user_activity = self.transactions.iter().any(|t| {
            !t.is_reversed
                && !matches!(
                    t.transaction_type,
                    LoanTransactionType::Disbursement
                        | LoanTransactionType::RepaymentAtDisbursement
                        | LoanTransactionType::Accrual
                        | LoanTransactionType::AccrualActivity
                )
        });
        if has_user_activity {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.undo.disbursal.not.allowed.transactions.exist",
                "Disbursal cannot be undone when repayments or other monetary transactions exist on the loan",
            )
            .into());
        }

        self.transactions.clear();
        for inst in &mut self.installments {
            inst.reset_derived_components();
            inst.reset_accrual_components();
        }
        for charge in &mut self.charges {
            charge.reset_paid_amount();
        }
        self.actual_disbursement_date = None;
        self.net_disbursal_amount = Decimal::ZERO;
        self.accrued_till = None;
        self.closed_on_date = None;
        self.overpaid_on_date = None;
        self.delinquency_actions.clear();
        self.delinquency_tag_history.clear();
        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::LoanDisbursalUndo,
            self.status,
        );
        self.summary
            .update_summary(Decimal::ZERO, &self.installments, &self.charges, Decimal::ZERO);
        Ok(())
    }

    // =========================================================================
    // Repayments, Refunds, Credits, and Waivers
    // =========================================================================

    /// Records a borrower repayment (`LoanTransactionType::Repayment`) and reprocesses the
    /// schedule, automatically transitioning `Active -> ClosedObligationsMet` or `Active -> Overpaid`.
    pub fn make_repayment(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        self.apply_repayment_like_transaction(
            LoanTransactionType::Repayment,
            transaction_date,
            amount,
        )
    }

    /// Applies a `GoodwillCredit` transaction (`LoanTransactionType::GoodwillCredit`).
    pub fn apply_goodwill_credit(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        self.apply_repayment_like_transaction(
            LoanTransactionType::GoodwillCredit,
            transaction_date,
            amount,
        )
    }

    /// Applies a `MerchantIssuedRefund` transaction (`LoanTransactionType::MerchantIssuedRefund`).
    pub fn apply_merchant_issued_refund(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        self.apply_repayment_like_transaction(
            LoanTransactionType::MerchantIssuedRefund,
            transaction_date,
            amount,
        )
    }

    /// Applies a `PayoutRefund` transaction (`LoanTransactionType::PayoutRefund`).
    pub fn apply_payout_refund(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        self.apply_repayment_like_transaction(
            LoanTransactionType::PayoutRefund,
            transaction_date,
            amount,
        )
    }

    /// Applies a refund (`RefundForActiveLoan` / `Refund`), unpaying previously completed
    /// installment portions or reducing overpayment and transitioning `Overpaid -> Active` or
    /// `ClosedObligationsMet -> Active` when `total_outstanding > 0`.
    ///
    /// Mirrors `LoanRefundService.makeRefundForActiveLoan` and `LoanRefundService.makeRefund`.
    pub fn make_refund(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        if !(self.status.is_active()
            || self.status.is_overpaid()
            || self.status.is_closed_obligations_met())
        {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.refund.invalid.loan.state",
                format!("Cannot make refund when loan is in state {}", self.status),
            )
            .into());
        }
        if self.is_charged_off {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.refund.not.allowed.on.charged.off.loan",
                "Refund is not allowed on a charged-off loan",
            )
            .into());
        }
        self.validate_not_before_disbursement(transaction_date, "refund")?;
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.refund.amount.must.be.greater.than.zero",
                "Refund amount must be greater than zero",
            )
            .into());
        }
        let max_refundable = self.summary.total_repayment;
        if amount > max_refundable {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.refund.amount.cannot.exceed.total.paid",
                format!(
                    "Refund amount ({amount}) cannot exceed total repaid ({max_refundable})"
                ),
            )
            .into());
        }

        let tx_type = if self.status.is_active() {
            LoanTransactionType::RefundForActiveLoan
        } else {
            LoanTransactionType::Refund
        };
        let mut tx = LoanTransaction::new(
            self.id,
            self.office_id,
            tx_type,
            transaction_date,
            amount,
        );
        self.assign_tx_id(&mut tx);
        self.transactions.push(tx.clone());

        self.reprocess_and_update_summary(transaction_date)?;
        Ok(self.find_tx_or_default(tx.id, tx))
    }

    /// Refunds overpaid credit balance (`LoanTransactionType::CreditBalanceRefund`) on an
    /// `Overpaid` loan, transitioning `Overpaid -> ClosedObligationsMet` when `total_outstanding == 0`
    /// and `total_overpaid == 0` (or `Overpaid -> Active` if `total_outstanding > 0`).
    ///
    /// Mirrors `LoanRefundService.creditBalanceRefund` and `Loan.handleCreditBalanceRefund`.
    pub fn credit_balance_refund(
        &mut self,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        if !self.status.is_overpaid() || self.summary.total_overpaid <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.credit.balance.refund.loan.not.overpaid",
                "Credit balance refund is only allowed when the loan is Overpaid",
            )
            .into());
        }
        if self.is_charged_off {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.credit.balance.refund.not.allowed.on.charged.off.loan",
                "Credit balance refund is not allowed on a charged-off loan",
            )
            .into());
        }
        self.validate_not_before_disbursement(transaction_date, "credit.balance.refund")?;
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.credit.balance.refund.amount.must.be.greater.than.zero",
                "Credit balance refund amount must be greater than zero",
            )
            .into());
        }
        if amount > self.summary.total_overpaid {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.credit.balance.refund.amount.must.be.less.than.or.equal.to.overpaid.amount",
                format!(
                    "Credit balance refund amount ({amount}) cannot exceed total overpaid ({})",
                    self.summary.total_overpaid
                ),
            )
            .into());
        }

        let mut tx = LoanTransaction::new(
            self.id,
            self.office_id,
            LoanTransactionType::CreditBalanceRefund,
            transaction_date,
            amount,
        );
        tx.overpayment_portion = amount;
        self.assign_tx_id(&mut tx);
        self.transactions.push(tx.clone());

        self.reprocess_and_update_summary(transaction_date)?;
        Ok(self.find_tx_or_default(tx.id, tx))
    }

    /// Waives interest (`LoanTransactionType::WaiveInterest`) on an `Active` loan.
    ///
    /// Mirrors `Loan.waiveInterest`.
    pub fn waive_interest(
        &mut self,
        waive_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        if !self.status.is_active() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.waive.interest.not.in.active.state",
                "Interest waiver is only allowed when the loan is in Active state",
            )
            .into());
        }
        if self.is_charged_off {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.waive.interest.not.allowed.on.charged.off.loan",
                "Interest waiver is not allowed on a charged-off loan",
            )
            .into());
        }
        self.validate_not_before_disbursement(waive_date, "waive.interest")?;
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.waive.interest.amount.must.be.greater.than.zero",
                "Waive interest amount must be greater than zero",
            )
            .into());
        }
        if amount > self.summary.total_interest_outstanding {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.waive.interest.amount.exceeds.outstanding.interest",
                format!(
                    "Waive interest amount ({amount}) exceeds total interest outstanding ({})",
                    self.summary.total_interest_outstanding
                ),
            )
            .into());
        }

        let mut tx = LoanTransaction::waiver(
            self.id,
            self.office_id,
            waive_date,
            amount,
            amount,
            Decimal::ZERO,
            ExternalId::empty(),
        );
        self.assign_tx_id(&mut tx);
        self.transactions.push(tx.clone());

        self.reprocess_and_update_summary(waive_date)?;
        Ok(self.find_tx_or_default(tx.id, tx))
    }

    // =========================================================================
    // Write-Off, Undo Write-Off, and Recovery Repayment
    // =========================================================================

    /// Writes off all remaining outstanding principal, interest, fees, and penalties on an
    /// `Active` loan (`Active -> ClosedWrittenOff`).
    ///
    /// Mirrors `Loan.closeAsWrittenOff`.
    pub fn write_off(
        &mut self,
        write_off_date: NaiveDate,
    ) -> Result<LoanTransaction, FineractError> {
        if !self.status.is_active() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.writeoff.not.in.active.state",
                "Write-off is only allowed when the loan is in Active state",
            )
            .into());
        }
        self.validate_not_before_disbursement(write_off_date, "writeoff")?;
        if let Some(last_tx_date) = self.latest_monetary_transaction_date() {
            if write_off_date < last_tx_date {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.loan.writeoff.date.cannot.be.before.last.transaction.date",
                    "Write-off date cannot be earlier than the latest monetary transaction date",
                )
                .into());
            }
        }
        if self.summary.total_outstanding <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.writeoff.no.outstanding.balance",
                "Cannot write off a loan that has no outstanding balance",
            )
            .into());
        }

        let mut tx = LoanTransaction::writeoff(
            self.id,
            self.office_id,
            write_off_date,
            ExternalId::empty(),
        );
        self.assign_tx_id(&mut tx);
        self.transactions.push(tx.clone());

        for charge in &mut self.charges {
            if charge.is_active && !charge.is_paid && !charge.waived {
                charge.write_off();
            }
        }

        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::WriteOffOutstanding,
            self.status,
        );
        self.written_off_on_date = Some(write_off_date);
        self.closed_on_date = Some(write_off_date);

        self.reprocess_and_update_summary(write_off_date)?;
        Ok(self.find_tx_or_default(tx.id, tx))
    }

    /// Reverses a write-off (`ClosedWrittenOff -> Active`), reversing the `WriteOff` transaction
    /// and any `RecoveryRepayment` transactions if none exist (or failing if recovery repayments
    /// have already been collected).
    ///
    /// Mirrors `Loan.undoWrittenOff`.
    pub fn undo_write_off(&mut self, undo_date: NaiveDate) -> Result<(), FineractError> {
        if !self.status.is_closed_written_off() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.undo.writeoff.not.written.off",
                "Undo write-off is only allowed when the loan is in Closed (written off) state",
            )
            .into());
        }
        let has_recoveries = self
            .transactions
            .iter()
            .any(|t| !t.is_reversed && t.is_recovery_repayment());
        if has_recoveries {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.undo.writeoff.not.allowed.recovery.repayments.exist",
                "Cannot undo write-off when recovery repayments exist on the loan",
            )
            .into());
        }

        for tx in &mut self.transactions {
            if !tx.is_reversed && tx.is_write_off() {
                tx.reverse(undo_date);
            }
        }
        for charge in &mut self.charges {
            if charge.amount_writtenoff > Decimal::ZERO {
                charge.amount_outstanding += charge.amount_writtenoff;
                charge.amount_writtenoff = Decimal::ZERO;
            }
        }

        self.written_off_on_date = None;
        self.closed_on_date = None;
        self.status = DefaultLoanLifecycleStateMachine::dry_transition(
            LoanEvent::WriteOffOutstandingUndo,
            self.status,
        );
        self.reprocess_and_update_summary(undo_date)?;
        Ok(())
    }

    /// Records a recovery payment (`LoanTransactionType::RecoveryRepayment`) against a
    /// written-off loan (`LoanStatus::ClosedWrittenOff`).
    ///
    /// Mirrors `LoanWritePlatformServiceJpaRepositoryImpl.makeLoanRepaymentWithChargeRefundChargeType`
    /// for `RECOVERY_REPAYMENT`.
    pub fn recover_written_off_balance(
        &mut self,
        recovery_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        if !self.status.is_closed_written_off() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.recovery.repayment.only.allowed.on.written.off.loan",
                "Recovery repayment is only allowed on a Closed (written off) loan",
            )
            .into());
        }
        if let Some(wo_date) = self.written_off_on_date {
            if recovery_date < wo_date {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.loan.recovery.repayment.date.cannot.be.before.writeoff.date",
                    "Recovery repayment date cannot be earlier than the write-off date",
                )
                .into());
            }
        }
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.recovery.repayment.amount.must.be.greater.than.zero",
                "Recovery repayment amount must be greater than zero",
            )
            .into());
        }

        let total_written_off = self.summary.total_principal_writtenoff
            + self.summary.total_interest_writtenoff
            + self
                .installments
                .iter()
                .map(|i| i.fee_charges_writtenoff + i.penalty_charges_writtenoff)
                .sum::<Decimal>();
        let already_recovered: Decimal = self
            .transactions
            .iter()
            .filter(|t| !t.is_reversed && t.is_recovery_repayment())
            .map(|t| t.amount)
            .sum();
        let remaining_recoverable = (total_written_off - already_recovered).max(Decimal::ZERO);
        if amount > remaining_recoverable {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.recovery.repayment.exceeds.written.off.balance",
                format!(
                    "Recovery repayment amount ({amount}) cannot exceed remaining written-off balance ({remaining_recoverable})"
                ),
            )
            .into());
        }

        // Allocate recovery across written-off Principal -> Interest -> Fee -> Penalty
        let mut rem = amount;
        let rem_principal = (self.summary.total_principal_writtenoff
            - self
                .transactions
                .iter()
                .filter(|t| !t.is_reversed && t.is_recovery_repayment())
                .map(|t| t.principal_portion)
                .sum::<Decimal>())
        .max(Decimal::ZERO);
        let principal_part = rem.min(rem_principal);
        rem -= principal_part;

        let rem_interest = (self.summary.total_interest_writtenoff
            - self
                .transactions
                .iter()
                .filter(|t| !t.is_reversed && t.is_recovery_repayment())
                .map(|t| t.interest_portion)
                .sum::<Decimal>())
        .max(Decimal::ZERO);
        let interest_part = rem.min(rem_interest);
        rem -= interest_part;

        let fee_written_off: Decimal = self
            .installments
            .iter()
            .map(|i| i.fee_charges_writtenoff)
            .sum();
        let rem_fee = (fee_written_off
            - self
                .transactions
                .iter()
                .filter(|t| !t.is_reversed && t.is_recovery_repayment())
                .map(|t| t.fee_charges_portion)
                .sum::<Decimal>())
        .max(Decimal::ZERO);
        let fee_part = rem.min(rem_fee);
        rem -= fee_part;

        let penalty_part = rem;

        let mut tx = LoanTransaction::new(
            self.id,
            self.office_id,
            LoanTransactionType::RecoveryRepayment,
            recovery_date,
            amount,
        );
        tx.principal_portion = principal_part;
        tx.interest_portion = interest_part;
        tx.fee_charges_portion = fee_part;
        tx.penalty_charges_portion = penalty_part;
        self.assign_tx_id(&mut tx);
        self.transactions.push(tx.clone());
        Ok(tx)
    }

    // =========================================================================
    // Charge-Off & Undo Charge-Off
    // =========================================================================

    /// Marks an `Active` loan as charged off (`LoanTransactionType::ChargeOff`),
    /// enforcing [`LoanChargeOffBehaviour`] and lifting any active delinquency tags.
    ///
    /// Mirrors `LoanWritePlatformServiceJpaRepositoryImpl.chargeOff`.
    pub fn charge_off(
        &mut self,
        charge_off_date: NaiveDate,
    ) -> Result<LoanTransaction, FineractError> {
        if !self.status.is_active() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.chargeoff.not.in.active.state",
                "Charge-off is only allowed when the loan is in Active state",
            )
            .into());
        }
        if self.is_charged_off {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.chargeoff.already.charged.off",
                "Loan is already charged off",
            )
            .into());
        }
        self.validate_not_before_disbursement(charge_off_date, "chargeoff")?;
        if let Some(last_tx_date) = self.latest_monetary_transaction_date() {
            if charge_off_date < last_tx_date {
                return Err(PlatformDomainRuleException::new(
                    "error.msg.loan.chargeoff.date.cannot.be.before.last.transaction.date",
                    "Charge-off date cannot be earlier than the latest monetary transaction date",
                )
                .into());
            }
        }

        // Apply LoanChargeOffBehaviour::ZeroInterest or AccelerateMaturity if configured
        match self.charge_off_behaviour {
            LoanChargeOffBehaviour::Regular => {}
            LoanChargeOffBehaviour::ZeroInterest => {
                for inst in &mut self.installments {
                    if inst.due_date > charge_off_date && inst.interest_outstanding() > Decimal::ZERO
                    {
                        inst.interest_amount = inst.interest_completed
                            + inst.interest_waived
                            + inst.interest_writtenoff;
                    }
                }
            }
            LoanChargeOffBehaviour::AccelerateMaturity => {
                for inst in &mut self.installments {
                    if inst.due_date > charge_off_date && inst.is_not_fully_paid_off() {
                        inst.due_date = charge_off_date;
                    }
                }
                self.actual_maturity_date = Some(charge_off_date);
            }
        }

        let mut tx = LoanTransaction::new(
            self.id,
            self.office_id,
            LoanTransactionType::ChargeOff,
            charge_off_date,
            self.summary.total_outstanding,
        );
        tx.principal_portion = self.summary.total_principal_outstanding;
        tx.interest_portion = self.summary.total_interest_outstanding;
        tx.fee_charges_portion = self.summary.total_fee_charges_outstanding;
        tx.penalty_charges_portion = self.summary.total_penalty_charges_outstanding;
        tx.outstanding_loan_balance = self.summary.total_principal_outstanding;
        self.assign_tx_id(&mut tx);
        self.transactions.push(tx.clone());

        self.is_charged_off = true;
        self.charged_off_on_date = Some(charge_off_date);

        // Lift any active delinquency tags when a loan is charged off
        for tag in &mut self.delinquency_tag_history {
            if tag.is_active() {
                tag.lift(charge_off_date);
            }
        }

        self.reprocess_and_update_summary(charge_off_date)?;
        Ok(self.find_tx_or_default(tx.id, tx))
    }

    /// Reverses a charge-off (`undoChargeOff`), restoring `is_charged_off = false`.
    ///
    /// Mirrors `LoanWritePlatformServiceJpaRepositoryImpl.undoChargeOff`.
    pub fn undo_charge_off(&mut self, undo_date: NaiveDate) -> Result<(), FineractError> {
        if !self.is_charged_off {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.undo.chargeoff.loan.not.charged.off",
                "Cannot undo charge-off on a loan that is not charged off",
            )
            .into());
        }
        for tx in &mut self.transactions {
            if !tx.is_reversed && tx.is_charge_off() {
                tx.reverse(undo_date);
            }
        }
        self.is_charged_off = false;
        self.charged_off_on_date = None;
        self.reprocess_and_update_summary(undo_date)?;
        Ok(())
    }

    // =========================================================================
    // Accrual Calculation Engine
    // =========================================================================

    /// Recalculates and posts periodic [`LoanTransactionType::Accrual`] transactions up to
    /// `business_date` (`LoanAccrualsProcessingServiceImpl.addPeriodicAccruals`).
    ///
    /// For each installment:
    /// - If `due_date <= business_date`, accrues the full installment interest, fee, and penalty.
    /// - If `from_date < business_date < due_date`, linearly pro-rates installment interest by
    ///   elapsed days `(business_date - from_date) / (due_date - from_date)` using `rounding_mode`.
    /// - Emits a single `LoanTransactionType::Accrual` for any net positive delta over
    ///   `installment.interest_accrued` (and unaccrued fees/penalties).
    pub fn recalculate_accruals_up_to(
        &mut self,
        business_date: NaiveDate,
    ) -> Result<Vec<LoanTransaction>, FineractError> {
        if !(self.status.is_active()
            || self.status.is_closed_obligations_met()
            || self.status.is_overpaid())
        {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.accrual.invalid.loan.state",
                format!("Cannot calculate accruals when loan is in state {}", self.status),
            )
            .into());
        }
        if self.is_charged_off {
            return Ok(Vec::new());
        }

        let disb_date = self
            .actual_disbursement_date
            .or(self.expected_disbursed_on_date)
            .unwrap_or(business_date);
        if business_date < disb_date {
            return Ok(Vec::new());
        }

        let already_accrued_fees: Decimal = self
            .transactions
            .iter()
            .filter(|t| !t.is_reversed && t.is_accrual())
            .map(|t| t.fee_charges_portion)
            .sum();
        let already_accrued_penalties: Decimal = self
            .transactions
            .iter()
            .filter(|t| !t.is_reversed && t.is_accrual())
            .map(|t| t.penalty_charges_portion)
            .sum();

        let mut new_interest_accrual = Decimal::ZERO;
        let mut cumulative_due_fees = Decimal::ZERO;
        let mut cumulative_due_penalties = Decimal::ZERO;

        for inst in &mut self.installments {
            if inst.is_down_payment {
                continue;
            }
            let from_date = inst.from_date.unwrap_or(disb_date);
            if business_date <= from_date {
                continue;
            }

            let recognizabale_interest =
                (inst.interest_amount - inst.interest_waived).max(Decimal::ZERO);
            let target_interest = if business_date >= inst.due_date {
                recognizabale_interest
            } else {
                let total_days = (inst.due_date - from_date).num_days().max(1);
                let elapsed_days = (business_date - from_date)
                    .num_days()
                    .clamp(0, total_days);
                let raw = recognizabale_interest * Decimal::from(elapsed_days)
                    / Decimal::from(total_days);
                Money::of(self.currency, raw, self.rounding_mode).amount()
            };

            if target_interest > inst.interest_accrued {
                let delta = target_interest - inst.interest_accrued;
                inst.interest_accrued = target_interest;
                new_interest_accrual += delta;
            }

            if business_date >= inst.due_date {
                cumulative_due_fees +=
                    (inst.fee_charges_amount - inst.fee_charges_waived).max(Decimal::ZERO);
                cumulative_due_penalties +=
                    (inst.penalty_charges_amount - inst.penalty_charges_waived).max(Decimal::ZERO);
            }
        }

        let new_fee_accrual = (cumulative_due_fees - already_accrued_fees).max(Decimal::ZERO);
        let new_penalty_accrual =
            (cumulative_due_penalties - already_accrued_penalties).max(Decimal::ZERO);
        let total_accrual = new_interest_accrual + new_fee_accrual + new_penalty_accrual;

        let mut created = Vec::new();
        if total_accrual > Decimal::ZERO {
            let mut accrual_tx = LoanTransaction::accrual(
                self.id,
                self.office_id,
                business_date,
                total_accrual,
                new_interest_accrual,
                new_fee_accrual,
                new_penalty_accrual,
                ExternalId::empty(),
            );
            self.assign_tx_id(&mut accrual_tx);
            self.transactions.push(accrual_tx.clone());
            created.push(accrual_tx);
        }
        self.accrued_till = Some(business_date);
        Ok(created)
    }

    // =========================================================================
    // Delinquency Action Management
    // =========================================================================

    /// Records a [`LoanDelinquencyAction`] (`Pause` or `Resume`) after validating it against
    /// existing pause periods and loan state.
    pub fn record_delinquency_action(
        &mut self,
        business_date: NaiveDate,
        action: LoanDelinquencyAction,
    ) -> Result<(), FineractError> {
        DelinquencyCalculationEngine::validate_delinquency_action(
            self.status,
            self.actual_disbursement_date,
            business_date,
            &self.delinquency_actions,
            &action,
        )?;
        self.delinquency_actions.push(action);
        Ok(())
    }

    // =========================================================================
    // Internal Replay & Lifecycle Status Reconciliation
    // =========================================================================

    fn apply_repayment_like_transaction(
        &mut self,
        tx_type: LoanTransactionType,
        transaction_date: NaiveDate,
        amount: Decimal,
    ) -> Result<LoanTransaction, FineractError> {
        if !(self.status.is_active()
            || self.status.is_overpaid()
            || self.status.is_closed_obligations_met())
        {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.repayment.or.waiver.account.is.not.active",
                format!(
                    "Transaction {tx_type:?} is not allowed when loan is in state {}",
                    self.status
                ),
            )
            .into());
        }
        self.validate_not_before_disbursement(transaction_date, "repayment")?;
        if amount <= Decimal::ZERO {
            return Err(PlatformDomainRuleException::new(
                "error.msg.loan.transaction.amount.must.be.greater.than.zero",
                "Transaction amount must be greater than zero",
            )
            .into());
        }

        let mut tx = LoanTransaction::new(
            self.id,
            self.office_id,
            tx_type,
            transaction_date,
            amount,
        );
        self.assign_tx_id(&mut tx);
        self.transactions.push(tx.clone());

        self.reprocess_and_update_summary(transaction_date)?;
        Ok(self.find_tx_or_default(tx.id, tx))
    }

    /// Replays all non-reversed transactions via [`BackdatedTransactionReplayEngine`], updates
    /// [`LoanSummary`], and reconciles [`LoanStatus`] (`Active <-> Overpaid <-> ClosedObligationsMet`).
    pub fn reprocess_and_update_summary(
        &mut self,
        reference_date: NaiveDate,
    ) -> Result<(), FineractError> {
        let disb_date = self
            .actual_disbursement_date
            .or(self.expected_disbursed_on_date)
            .unwrap_or(reference_date);

        // Remember recovery repayments on written-off loans so replay does not treat them as normal schedule repayments
        if self.status.is_closed_written_off() {
            let mut non_recovery_txs: Vec<LoanTransaction> = self
                .transactions
                .iter()
                .filter(|t| !t.is_recovery_repayment())
                .cloned()
                .collect();
            let recovery_txs: Vec<LoanTransaction> = self
                .transactions
                .iter()
                .filter(|t| t.is_recovery_repayment())
                .cloned()
                .collect();

            let overpaid = BackdatedTransactionReplayEngine::reprocess_transactions(
                &self.currency,
                disb_date,
                &mut self.installments,
                &mut self.charges,
                &mut non_recovery_txs,
                self.transaction_strategy,
                self.rounding_mode,
            )?;
            non_recovery_txs.extend(recovery_txs);
            self.transactions = non_recovery_txs;
            let disbursed_principal = self.total_disbursed_principal();
            self.summary.update_summary(
                disbursed_principal,
                &self.installments,
                &self.charges,
                overpaid.amount(),
            );
            return Ok(());
        }

        let overpaid_from_replay = BackdatedTransactionReplayEngine::reprocess_transactions(
            &self.currency,
            disb_date,
            &mut self.installments,
            &mut self.charges,
            &mut self.transactions,
            self.transaction_strategy,
            self.rounding_mode,
        )?;

        // Also account for any Refund / RefundForActiveLoan reducing previous overpayment
        let refunded_from_active_or_overpaid: Decimal = self
            .transactions
            .iter()
            .filter(|t| {
                !t.is_reversed
                    && matches!(
                        t.transaction_type,
                        LoanTransactionType::Refund | LoanTransactionType::RefundForActiveLoan
                    )
            })
            .map(|t| t.amount)
            .sum();

        let net_overpaid =
            (overpaid_from_replay.amount() - refunded_from_active_or_overpaid).max(Decimal::ZERO);

        let disbursed_principal = self.total_disbursed_principal();
        self.summary.update_summary(
            disbursed_principal,
            &self.installments,
            &self.charges,
            net_overpaid,
        );

        // Reconcile automatic lifecycle transitions between Active, Overpaid, and ClosedObligationsMet
        if matches!(
            self.status,
            LoanStatus::Active | LoanStatus::Overpaid | LoanStatus::ClosedObligationsMet
        ) {
            if self.summary.total_overpaid > Decimal::ZERO {
                self.status = DefaultLoanLifecycleStateMachine::dry_transition(
                    LoanEvent::LoanOverpayment,
                    self.status,
                );
                self.overpaid_on_date = Some(reference_date);
                self.closed_on_date = None;
            } else if self.summary.total_outstanding == Decimal::ZERO {
                self.status = DefaultLoanLifecycleStateMachine::dry_transition(
                    LoanEvent::RepaidInFull,
                    self.status,
                );
                self.closed_on_date = Some(reference_date);
                self.overpaid_on_date = None;
                for tag in &mut self.delinquency_tag_history {
                    if tag.is_active() {
                        tag.lift(reference_date);
                    }
                }
            } else {
                self.status = DefaultLoanLifecycleStateMachine::dry_transition(
                    LoanEvent::LoanRepaymentOrWaiver,
                    self.status,
                );
                self.closed_on_date = None;
                self.overpaid_on_date = None;
            }
        }

        Ok(())
    }

    fn total_disbursed_principal(&self) -> Decimal {
        let sum: Decimal = self
            .transactions
            .iter()
            .filter(|t| !t.is_reversed && t.is_disbursement())
            .map(|t| t.amount)
            .sum();
        if sum > Decimal::ZERO {
            sum
        } else {
            self.net_disbursal_amount
        }
    }

    fn latest_monetary_transaction_date(&self) -> Option<NaiveDate> {
        self.transactions
            .iter()
            .filter(|t| !t.is_reversed && !t.is_non_monetary_transaction())
            .map(|t| t.transaction_date)
            .max()
    }

    fn validate_not_before_disbursement(
        &self,
        tx_date: NaiveDate,
        action_key: &str,
    ) -> Result<(), FineractError> {
        if let Some(disb_date) = self.actual_disbursement_date {
            if tx_date < disb_date {
                return Err(PlatformDomainRuleException::new(
                    format!("error.msg.loan.{action_key}.date.cannot.be.before.disbursement.date"),
                    format!("Transaction date ({tx_date}) cannot be earlier than disbursement date ({disb_date})"),
                )
                .into());
            }
        }
        Ok(())
    }

    fn assign_tx_id(&mut self, tx: &mut LoanTransaction) {
        if tx.id.is_none() {
            tx.id = Some(self.next_transaction_id);
            self.next_transaction_id += 1;
        }
    }

    fn find_tx_or_default(
        &self,
        tx_id: Option<i64>,
        fallback: LoanTransaction,
    ) -> LoanTransaction {
        tx_id
            .and_then(|id| self.transactions.iter().find(|t| t.id == Some(id)).cloned())
            .unwrap_or(fallback)
    }
}

// =============================================================================
// 7. Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn usd() -> MonetaryCurrency {
        MonetaryCurrency::new("USD", 2, None)
    }

    fn sample_three_month_schedule() -> Vec<LoanRepaymentScheduleInstallment> {
        vec![
            LoanRepaymentScheduleInstallment::new(
                Some(1),
                1,
                Some(d(2026, 1, 1)),
                d(2026, 2, 1),
                dec!(300.00),
                dec!(30.00),
                dec!(0.00),
                dec!(0.00),
            ),
            LoanRepaymentScheduleInstallment::new(
                Some(1),
                2,
                Some(d(2026, 2, 1)),
                d(2026, 3, 1),
                dec!(300.00),
                dec!(20.00),
                dec!(0.00),
                dec!(0.00),
            ),
            LoanRepaymentScheduleInstallment::new(
                Some(1),
                3,
                Some(d(2026, 3, 1)),
                d(2026, 4, 1),
                dec!(400.00),
                dec!(10.00),
                dec!(0.00),
                dec!(0.00),
            ),
        ]
    }

    fn build_active_loan() -> LoanAggregate {
        let mut loan = LoanAggregate::new(
            Some(1),
            "LN-0001",
            10,
            usd(),
            dec!(1000.00),
            sample_three_month_schedule(),
            TransactionProcessorStrategyCode::MifosStandard,
            FineractRoundingMode::HalfEven,
        );
        loan.submit_application(d(2026, 1, 1), Some(d(2026, 1, 1)))
            .expect("submit");
        loan.approve(d(2026, 1, 1), None).expect("approve");
        loan.disburse(d(2026, 1, 1), None).expect("disburse");
        loan
    }

    #[test]
    fn test_loan_status_and_sub_status_discriminants() {
        assert_eq!(LoanStatus::Invalid as i16, 0);
        assert_eq!(LoanStatus::SubmittedAndPendingApproval as i16, 100);
        assert_eq!(LoanStatus::Approved as i16, 200);
        assert_eq!(LoanStatus::Active as i16, 300);
        assert_eq!(LoanStatus::TransferInProgress as i16, 303);
        assert_eq!(LoanStatus::TransferOnHold as i16, 304);
        assert_eq!(LoanStatus::WithdrawnByClient as i16, 400);
        assert_eq!(LoanStatus::Rejected as i16, 500);
        assert_eq!(LoanStatus::ClosedObligationsMet as i16, 600);
        assert_eq!(LoanStatus::ClosedWrittenOff as i16, 601);
        assert_eq!(LoanStatus::ClosedRescheduleOutstandingAmount as i16, 602);
        assert_eq!(LoanStatus::Overpaid as i16, 700);

        assert_eq!(LoanSubStatus::Invalid as i16, 0);
        assert_eq!(LoanSubStatus::Foreclosed as i16, 100);
        assert_eq!(LoanSubStatus::Claimed as i16, 200);
        assert_eq!(LoanSubStatus::Restructured as i16, 300);
    }

    #[test]
    fn test_full_application_approval_reject_withdraw_and_undo_lifecycle() {
        // 1. Submit -> Reject
        let mut rejected_loan = LoanAggregate::new(
            Some(10),
            "LN-REJ",
            1,
            usd(),
            dec!(1000.00),
            sample_three_month_schedule(),
            TransactionProcessorStrategyCode::MifosStandard,
            FineractRoundingMode::HalfEven,
        );
        rejected_loan
            .submit_application(d(2026, 1, 1), Some(d(2026, 1, 5)))
            .unwrap();
        assert_eq!(
            rejected_loan.status,
            LoanStatus::SubmittedAndPendingApproval
        );
        rejected_loan.reject(d(2026, 1, 3)).unwrap();
        assert_eq!(rejected_loan.status, LoanStatus::Rejected);
        assert_eq!(rejected_loan.rejected_on_date, Some(d(2026, 1, 3)));

        // 2. Submit -> Withdraw by client
        let mut withdrawn_loan = LoanAggregate::new(
            Some(11),
            "LN-WTH",
            1,
            usd(),
            dec!(1000.00),
            sample_three_month_schedule(),
            TransactionProcessorStrategyCode::MifosStandard,
            FineractRoundingMode::HalfEven,
        );
        withdrawn_loan
            .submit_application(d(2026, 1, 1), Some(d(2026, 1, 5)))
            .unwrap();
        withdrawn_loan.withdraw_by_client(d(2026, 1, 4)).unwrap();
        assert_eq!(withdrawn_loan.status, LoanStatus::WithdrawnByClient);
        assert_eq!(withdrawn_loan.withdrawn_on_date, Some(d(2026, 1, 4)));

        // 3. Submit -> Approve -> Undo Approval -> Approve -> Disburse -> Undo Disbursal -> Disburse
        let mut loan = LoanAggregate::new(
            Some(12),
            "LN-FLOW",
            1,
            usd(),
            dec!(1000.00),
            sample_three_month_schedule(),
            TransactionProcessorStrategyCode::MifosStandard,
            FineractRoundingMode::HalfEven,
        );
        loan.submit_application(d(2026, 1, 1), Some(d(2026, 1, 10)))
            .unwrap();
        loan.approve(d(2026, 1, 2), Some(dec!(1000.00))).unwrap();
        assert_eq!(loan.status, LoanStatus::Approved);

        loan.undo_approval().unwrap();
        assert_eq!(loan.status, LoanStatus::SubmittedAndPendingApproval);
        assert_eq!(loan.approved_on_date, None);

        loan.approve(d(2026, 1, 5), Some(dec!(1000.00))).unwrap();
        loan.disburse(d(2026, 1, 10), None).unwrap();
        assert_eq!(loan.status, LoanStatus::Active);
        assert_eq!(loan.summary.total_principal_disbursed, dec!(1000.00));
        assert_eq!(loan.summary.total_outstanding, dec!(1060.00));

        loan.undo_disbursal().unwrap();
        assert_eq!(loan.status, LoanStatus::Approved);
        assert!(loan.transactions.is_empty());

        loan.disburse(d(2026, 1, 10), None).unwrap();
        assert_eq!(loan.status, LoanStatus::Active);
    }

    #[test]
    fn test_overpayment_refund_and_credit_balance_refund_closure() {
        let mut loan = build_active_loan();
        assert_eq!(loan.summary.total_outstanding, dec!(1060.00));

        // Pay first two installments (330 + 320 = 650)
        loan.make_repayment(d(2026, 2, 1), dec!(330.00)).unwrap();
        loan.make_repayment(d(2026, 3, 1), dec!(320.00)).unwrap();
        assert_eq!(loan.status, LoanStatus::Active);
        assert_eq!(loan.summary.total_outstanding, dec!(410.00));

        // Overpay final installment by 90.00 (pay 500.00 when 410.00 is remaining)
        let overpay_tx = loan.make_repayment(d(2026, 4, 1), dec!(500.00)).unwrap();
        assert_eq!(overpay_tx.overpayment_portion, dec!(90.00));
        assert_eq!(loan.status, LoanStatus::Overpaid);
        assert_eq!(loan.summary.total_outstanding, dec!(0.00));
        assert_eq!(loan.summary.total_overpaid, dec!(90.00));

        // Case A: Refund via make_refund(150.00) transitions Overpaid -> Active
        let mut active_again = loan.clone();
        active_again
            .make_refund(d(2026, 4, 2), dec!(150.00))
            .unwrap();
        assert_eq!(active_again.status, LoanStatus::Active);
        assert!(active_again.summary.total_outstanding > Decimal::ZERO);
        assert_eq!(active_again.summary.total_overpaid, dec!(0.00));

        // Case B: Credit balance refund of 90.00 transitions Overpaid -> ClosedObligationsMet
        let cbr_tx = loan
            .credit_balance_refund(d(2026, 4, 2), dec!(90.00))
            .unwrap();
        assert_eq!(
            cbr_tx.transaction_type,
            LoanTransactionType::CreditBalanceRefund
        );
        assert_eq!(cbr_tx.overpayment_portion, dec!(90.00));
        assert_eq!(loan.summary.total_overpaid, dec!(0.00));
        assert_eq!(loan.summary.total_outstanding, dec!(0.00));
        assert_eq!(loan.status, LoanStatus::ClosedObligationsMet);
        assert_eq!(loan.closed_on_date, Some(d(2026, 4, 2)));
    }

    #[test]
    fn test_goodwill_credit_merchant_refund_payout_refund_and_waiver() {
        let mut loan = build_active_loan();
        // Waive 10.00 of interest on installment 1
        let waiver_tx = loan.waive_interest(d(2026, 1, 20), dec!(10.00)).unwrap();
        assert_eq!(waiver_tx.interest_portion, dec!(10.00));
        assert_eq!(loan.summary.total_interest_waived, dec!(10.00));
        assert_eq!(loan.summary.total_outstanding, dec!(1050.00));

        // Apply GoodwillCredit (320.00 covers installment 1 remaining 20 interest + 300 principal)
        loan.apply_goodwill_credit(d(2026, 2, 1), dec!(320.00))
            .unwrap();
        assert_eq!(loan.summary.total_outstanding, dec!(730.00));

        // Apply MerchantIssuedRefund (320.00 covers installment 2)
        loan.apply_merchant_issued_refund(d(2026, 3, 1), dec!(320.00))
            .unwrap();
        assert_eq!(loan.summary.total_outstanding, dec!(410.00));

        // Apply PayoutRefund (410.00 covers installment 3 and closes the loan!)
        loan.apply_payout_refund(d(2026, 4, 1), dec!(410.00))
            .unwrap();
        assert_eq!(loan.status, LoanStatus::ClosedObligationsMet);
        assert_eq!(loan.summary.total_outstanding, dec!(0.00));
    }

    #[test]
    fn test_write_off_undo_write_off_and_recovery_repayment() {
        let mut loan = build_active_loan();
        loan.make_repayment(d(2026, 2, 1), dec!(330.00)).unwrap();
        assert_eq!(loan.summary.total_outstanding, dec!(730.00));

        // Write off remaining 730.00 (700 principal + 30 interest)
        let wo_tx = loan.write_off(d(2026, 3, 15)).unwrap();
        assert_eq!(wo_tx.transaction_type, LoanTransactionType::WriteOff);
        assert_eq!(wo_tx.amount, dec!(730.00));
        assert_eq!(wo_tx.principal_portion, dec!(700.00));
        assert_eq!(wo_tx.interest_portion, dec!(30.00));
        assert_eq!(loan.status, LoanStatus::ClosedWrittenOff);
        assert_eq!(loan.summary.total_outstanding, dec!(0.00));
        assert_eq!(loan.summary.total_principal_writtenoff, dec!(700.00));
        assert_eq!(loan.summary.total_interest_writtenoff, dec!(30.00));

        // Undo write-off restores Active state and 730.00 balance
        loan.undo_write_off(d(2026, 3, 16)).unwrap();
        assert_eq!(loan.status, LoanStatus::Active);
        assert_eq!(loan.summary.total_outstanding, dec!(730.00));

        // Re-apply write-off and collect recovery repayments
        loan.write_off(d(2026, 3, 20)).unwrap();
        assert_eq!(loan.status, LoanStatus::ClosedWrittenOff);

        let rec_tx = loan
            .recover_written_off_balance(d(2026, 4, 1), dec!(500.00))
            .unwrap();
        assert_eq!(
            rec_tx.transaction_type,
            LoanTransactionType::RecoveryRepayment
        );
        assert_eq!(rec_tx.principal_portion, dec!(500.00));

        // Undo write-off is blocked once recovery repayments exist
        assert!(loan.undo_write_off(d(2026, 4, 2)).is_err());
    }

    #[test]
    fn test_charge_off_and_undo_charge_off() {
        let mut loan = build_active_loan();
        loan.charge_off_behaviour = LoanChargeOffBehaviour::ZeroInterest;

        // First put loan into delinquency tag before charge-off
        let bucket = DelinquencyBucket::new(
            1,
            "Standard Bucket",
            vec![
                DelinquencyRange::new(1, "RANGE_1_30", 1, Some(30)).unwrap(),
                DelinquencyRange::new(2, "RANGE_31_PLUS", 31, None).unwrap(),
            ],
        )
        .unwrap();

        let (col, range) =
            DelinquencyCalculationEngine::evaluate_and_tag_loan(&mut loan, d(2026, 2, 15), Some(&bucket));
        assert_eq!(col.delinquent_days, 14);
        assert_eq!(range.map(|r| r.id), Some(1));
        assert_eq!(loan.delinquency_tag_history.len(), 1);
        assert!(loan.delinquency_tag_history[0].is_active());

        // Charge off on 2026-02-20
        let co_tx = loan.charge_off(d(2026, 2, 20)).unwrap();
        assert_eq!(co_tx.transaction_type, LoanTransactionType::ChargeOff);
        assert!(loan.is_charged_off);
        assert_eq!(loan.charged_off_on_date, Some(d(2026, 2, 20)));
        // Active delinquency tag is lifted upon charge-off
        assert_eq!(
            loan.delinquency_tag_history[0].lifted_on_date,
            Some(d(2026, 2, 20))
        );
        // ZeroInterest behaviour zeroed future installments' unpaid interest (installments 2 and 3 had 20 + 10 = 30 interest)
        assert_eq!(loan.summary.total_interest_outstanding, dec!(30.00));

        // Undo charge-off restores is_charged_off = false
        loan.undo_charge_off(d(2026, 2, 21)).unwrap();
        assert!(!loan.is_charged_off);
        assert_eq!(loan.charged_off_on_date, None);
    }

    #[test]
    fn test_delinquency_calculation_with_pause_and_resume_windows() {
        let mut loan = build_active_loan();
        let bucket = DelinquencyBucket::new(
            100,
            "Retail Delinquency Bucket",
            vec![
                DelinquencyRange::new(1, "1-10 Days", 1, Some(10)).unwrap(),
                DelinquencyRange::new(2, "11-30 Days", 11, Some(30)).unwrap(),
                DelinquencyRange::new(3, "31+ Days", 31, None).unwrap(),
            ],
        )
        .unwrap();

        // Installment 1 is due on 2026-02-01 (330.00).
        // On 2026-02-10, past_due_days = 9, delinquent_days = 9 -> Range 1 ("1-10 Days")
        let (col1, range1) = DelinquencyCalculationEngine::evaluate_and_tag_loan(
            &mut loan,
            d(2026, 2, 10),
            Some(&bucket),
        );
        assert_eq!(col1.past_due_days, 9);
        assert_eq!(col1.delinquent_days, 9);
        assert_eq!(col1.delinquent_amount.amount(), dec!(330.00));
        assert_eq!(range1.map(|r| r.id), Some(1));
        assert_eq!(loan.delinquency_tag_history.len(), 1);
        assert!(loan.delinquency_tag_history[0].is_active());

        // Record a Pause from 2026-02-11 to 2026-02-25 (14 days max), then Resume early on 2026-02-18 (7 paused days: Feb 11..18)
        loan.record_delinquency_action(
            d(2026, 2, 11),
            LoanDelinquencyAction::pause(Some(1), d(2026, 2, 11), d(2026, 2, 25)),
        )
        .unwrap();
        loan.record_delinquency_action(
            d(2026, 2, 18),
            LoanDelinquencyAction::resume(Some(2), d(2026, 2, 18)),
        )
        .unwrap();

        // Evaluate on 2026-02-25:
        // Total calendar past_due_days since 2026-02-01 = 24 days.
        // Effective pause window is [2026-02-11, 2026-02-18) = 7 days.
        // Effective delinquent_days = 24 - 7 = 17 days -> Range 2 ("11-30 Days")!
        let (col2, range2) = DelinquencyCalculationEngine::evaluate_and_tag_loan(
            &mut loan,
            d(2026, 2, 25),
            Some(&bucket),
        );
        assert_eq!(col2.past_due_days, 24);
        assert_eq!(col2.delinquent_days, 17);
        assert_eq!(col2.delinquent_date, Some(d(2026, 2, 8)));
        assert_eq!(range2.map(|r| r.id), Some(2));
        assert_eq!(loan.delinquency_tag_history.len(), 2);
        assert_eq!(
            loan.delinquency_tag_history[0].lifted_on_date,
            Some(d(2026, 2, 25))
        );
        assert!(loan.delinquency_tag_history[1].is_active());
        assert_eq!(loan.delinquency_tag_history[1].delinquency_range_id, 2);

        // Pay off overdue installment 1 on 2026-02-26 -> loan exits delinquency and lifts tag 2
        loan.make_repayment(d(2026, 2, 26), dec!(330.00)).unwrap();
        let (col3, range3) = DelinquencyCalculationEngine::evaluate_and_tag_loan(
            &mut loan,
            d(2026, 2, 26),
            Some(&bucket),
        );
        assert_eq!(col3.delinquent_days, 0);
        assert_eq!(col3.delinquent_amount.amount(), dec!(0.00));
        assert!(range3.is_none());
        assert!(loan.delinquency_tag_history.iter().all(|t| !t.is_active()));
    }

    #[test]
    fn test_recalculate_accruals_up_to_business_date() {
        let mut loan = build_active_loan();
        // Installment 1 due 2026-02-01 has 30.00 interest.
        // Installment 2 (2026-02-01..2026-03-01, 28 days) has 20.00 interest.
        // At 2026-02-15 (14 days into installment 2, i.e. 50% of 28 days):
        // Expected accrued interest = 30.00 (inst 1) + 10.00 (half of inst 2) = 40.00
        let created = loan.recalculate_accruals_up_to(d(2026, 2, 15)).unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].transaction_type, LoanTransactionType::Accrual);
        assert_eq!(created[0].interest_portion, dec!(40.00));
        assert_eq!(loan.installments[0].interest_accrued, dec!(30.00));
        assert_eq!(loan.installments[1].interest_accrued, dec!(10.00));

        // Calling again on the same date is idempotent (no new accrual transactions)
        let second = loan.recalculate_accruals_up_to(d(2026, 2, 15)).unwrap();
        assert!(second.is_empty());
    }
}
