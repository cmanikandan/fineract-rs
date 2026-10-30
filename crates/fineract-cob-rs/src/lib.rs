//! `fineract-cob-rs` — Close of Business (COB) Partitioned Worker Pool, Account Lock Manager,
//! Inline COB & Catch-up Engine.
//!
//! Faithfully ported from Apache Fineract's `fineract-cob` and `fineract-provider/cob`:
//! - [`LockOwner`] (`org.apache.fineract.cob.domain.LockOwner`)
//! - [`LoanAccountLock`] (`org.apache.fineract.cob.domain.LoanAccountLock`, `m_loan_account_locks`)
//! - [`LoanCobParameter`] & [`CobPartition`] (`LoanCOBParameter.java`, `COBPartition.java`, `CommonPartitioner.java`)
//! - [`CobBusinessStep`] (`org.apache.fineract.cob.COBBusinessStep`)
//! - Built-in Loan COB Business Steps (`org.apache.fineract.cob.loan.*` & `org.apache.fineract.investor.cob.loan.*`):
//!   1. [`ApplyChargeToOverdueLoansBusinessStep`] (`APPLY_CHARGE_TO_OVERDUE_LOANS`)
//!   2. [`LoanDelinquencyClassificationBusinessStep`] (`LOAN_DELINQUENCY_CLASSIFICATION`)
//!   3. [`LoanInterestRecalculationBusinessStep`] (`INTEREST_RECALCULATION`)
//!   4. [`AccrualActivityPostingBusinessStep`] (`ACCRUAL_ACTIVITY_POSTING`)
//!   5. [`ExternalAssetOwnerTransferBusinessStep`] (`EXTERNAL_ASSET_OWNER_TRANSFER`)
//!   6. [`CheckLoanRepaymentDueBusinessStep`] (`CHECK_LOAN_REPAYMENT_DUE`)
//!   7. [`CheckLoanRepaymentOverdueBusinessStep`] (`CHECK_LOAN_REPAYMENT_OVERDUE`)
//! - [`LoanAccountLockManager`] (`LoanLockingServiceImpl.java`, `AbstractLoanItemListener.java`, `ChunkProcessingLoanItemListener.java`)
//! - [`InlineLoanCobExecutor`] (`InlineLoanCOBExecutorServiceImpl.java`, `InlineCommonLockableCOBExecutorService.java`)
//! - [`PartitionedCobWorkerPool`] (`LoanCOBPartitioner.java`, `LoanItemProcessor.java`, `LoanItemWriter.java`, `LoanCOBCatchUpServiceImpl.java`)

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use dashmap::DashMap;
use rust_decimal::{Decimal, RoundingStrategy};
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{AnyPool, Row};
use tokio::sync::Semaphore;

use fineract_core_rs::context::{
    with_execution_context, ActionContext, BusinessDateType, ExecutionContext,
};
use fineract_core_rs::db::{DialectSql, SqlDialect};
use fineract_core_rs::primitives::ExternalId;
use fineract_investor_rs::{
    DualDbInvestorRepository, ExternalAssetOwnerTransferEngine, LoanTransferSnapshot,
};
use fineract_validation_rs::{
    ApiGlobalErrorResponse, FineractError, FineractResult, PlatformDomainRuleException,
};

// =============================================================================
// 1. COB CONSTANTS (`COBConstant.java` & `LoanCOBConstant.java`)
// =============================================================================

/// Job name for the Loan Close of Business batch job (`LoanCOBConstant.JOB_NAME`).
pub const LOAN_COB_JOB_NAME: &str = "LOAN_COB";

/// Human-readable job name (`LoanCOBConstant.JOB_HUMAN_READABLE_NAME`).
pub const LOAN_COB_JOB_HUMAN_READABLE_NAME: &str = "Loan COB";

/// Business step category name for Loan COB (`LoanCOBConstant.LOAN_COB_JOB_NAME`).
pub const LOAN_COB_STEP_JOB_NAME: &str = "LOAN_CLOSE_OF_BUSINESS";

/// Partition name prefix (`LoanCOBPartitioner.PARTITION_PREFIX`).
pub const LOAN_COB_PARTITION_PREFIX: &str = "Partition_";

/// Maximum length of the `error` column in `m_loan_account_locks` (`VARCHAR(255)`).
pub const MAX_LOCK_ERROR_LENGTH: usize = 255;

// =============================================================================
// 2. LockOwner (`org.apache.fineract.cob.domain.LockOwner`)
// =============================================================================

/// Identifies the owner of a row lock in `m_loan_account_locks`.
///
/// Ported from `org.apache.fineract.cob.domain.LockOwner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LockOwner {
    /// Batch chunk worker lock (`LOAN_COB_CHUNK_PROCESSING`).
    #[default]
    #[serde(
        rename = "LOAN_COB_CHUNK_PROCESSING",
        alias = "LoanCobChunkProcessing",
        alias = "loan_cob_chunk_processing"
    )]
    LoanCobChunkProcessing,

    /// Synchronous inline COB interceptor lock (`LOAN_INLINE_COB_PROCESSING`).
    #[serde(
        rename = "LOAN_INLINE_COB_PROCESSING",
        alias = "LoanInlineCobProcessing",
        alias = "loan_inline_cob_processing"
    )]
    LoanInlineCobProcessing,
}

impl LockOwner {
    /// Java-compatible constant alias for [`LockOwner::LoanCobChunkProcessing`].
    pub const LOAN_COB_CHUNK_PROCESSING: Self = Self::LoanCobChunkProcessing;
    /// Java-compatible constant alias for [`LockOwner::LoanInlineCobProcessing`].
    pub const LOAN_INLINE_COB_PROCESSING: Self = Self::LoanInlineCobProcessing;

    /// Returns the database/Java enum string (`LOAN_COB_CHUNK_PROCESSING` or `LOAN_INLINE_COB_PROCESSING`).
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoanCobChunkProcessing => "LOAN_COB_CHUNK_PROCESSING",
            Self::LoanInlineCobProcessing => "LOAN_INLINE_COB_PROCESSING",
        }
    }

    /// Alias for [`LockOwner::as_str`].
    #[inline]
    pub const fn code(self) -> &'static str {
        self.as_str()
    }

    /// Alias for [`LockOwner::as_str`].
    #[inline]
    pub const fn java_name(self) -> &'static str {
        self.as_str()
    }

    /// Parses a string into [`LockOwner`].
    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "LOAN_COB_CHUNK_PROCESSING" | "LOANCOBCHUNKPROCESSING" => {
                Some(Self::LoanCobChunkProcessing)
            }
            "LOAN_INLINE_COB_PROCESSING" | "LOANINLINECOBPROCESSING" => {
                Some(Self::LoanInlineCobProcessing)
            }
            _ => None,
        }
    }

    /// Returns `true` if owned by `LOAN_COB_CHUNK_PROCESSING`.
    #[inline]
    pub const fn is_chunk_processing(self) -> bool {
        matches!(self, Self::LoanCobChunkProcessing)
    }

    /// Returns `true` if owned by `LOAN_INLINE_COB_PROCESSING`.
    #[inline]
    pub const fn is_inline_processing(self) -> bool {
        matches!(self, Self::LoanInlineCobProcessing)
    }
}

impl fmt::Display for LockOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LockOwner {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.loan.lock.owner.invalid",
                format!("Unknown LockOwner value: {s}"),
            )
        })
    }
}

// =============================================================================
// 3. LoanAccountLock (`org.apache.fineract.cob.domain.LoanAccountLock`)
// =============================================================================

/// Row-level loan COB lock stored in `m_loan_account_locks`.
///
/// Ported from `org.apache.fineract.cob.domain.AccountLock` and
/// `org.apache.fineract.cob.domain.LoanAccountLock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAccountLock {
    pub loan_id: i64,
    pub version: i64,
    pub lock_owner: LockOwner,
    pub lock_placed_on: DateTime<Utc>,
    pub error: Option<String>,
    pub stacktrace: Option<String>,
    pub lock_placed_on_cob_business_date: Option<NaiveDate>,
}

impl LoanAccountLock {
    /// Creates a new [`LoanAccountLock`] for `loan_id` with `version = 0`.
    pub fn new(
        loan_id: i64,
        lock_owner: LockOwner,
        lock_placed_on_cob_business_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            loan_id,
            version: 0,
            lock_owner,
            lock_placed_on: Utc::now(),
            error: None,
            stacktrace: None,
            lock_placed_on_cob_business_date,
        }
    }

    /// Creates a soft lock (`LockOwner::LoanCobChunkProcessing`, `version = 0`).
    pub fn new_soft_lock(loan_id: i64, cob_business_date: NaiveDate) -> Self {
        Self::new(
            loan_id,
            LockOwner::LoanCobChunkProcessing,
            Some(cob_business_date),
        )
    }

    /// Creates a hard lock (`version = 1`) for the given [`LockOwner`].
    pub fn new_hard_lock(
        loan_id: i64,
        lock_owner: LockOwner,
        cob_business_date: NaiveDate,
    ) -> Self {
        Self {
            loan_id,
            version: 1,
            lock_owner,
            lock_placed_on: Utc::now(),
            error: None,
            stacktrace: None,
            lock_placed_on_cob_business_date: Some(cob_business_date),
        }
    }

    /// Records a step failure error and stacktrace on the lock, matching `AccountLock.setError`.
    pub fn set_error(&mut self, error: impl Into<String>, stacktrace: impl Into<String>) {
        let raw_error = error.into();
        let truncated_error = if raw_error.chars().count() > MAX_LOCK_ERROR_LENGTH {
            raw_error.chars().take(MAX_LOCK_ERROR_LENGTH).collect()
        } else {
            raw_error
        };
        self.error = Some(truncated_error);
        self.stacktrace = Some(stacktrace.into());
        self.version += 1;
    }

    /// Clears any recorded error and stacktrace on the lock.
    pub fn clear_error(&mut self) {
        self.error = None;
        self.stacktrace = None;
    }

    /// Transitions the lock to `new_lock_owner`, refreshes `lock_placed_on`, clears any prior error,
    /// and increments `version` (matching `AccountLock.setNewLockOwner`).
    pub fn set_new_lock_owner(&mut self, new_lock_owner: LockOwner) {
        self.lock_owner = new_lock_owner;
        self.lock_placed_on = Utc::now();
        self.error = None;
        self.stacktrace = None;
        self.version += 1;
    }

    /// Returns `true` if this lock has a recorded COB step failure (`error.is_some()`).
    #[inline]
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// Returns `true` if this is a soft lock (`LockOwner::LoanCobChunkProcessing` at `version == 0` with no error).
    #[inline]
    pub fn is_soft_lock(&self) -> bool {
        self.lock_owner == LockOwner::LoanCobChunkProcessing
            && self.version == 0
            && self.error.is_none()
    }

    /// Returns `true` if this is an active hard lock (`version > 0` or owned by `LoanInlineCobProcessing`).
    #[inline]
    pub fn is_hard_lock(&self) -> bool {
        (self.version > 0 || self.lock_owner == LockOwner::LoanInlineCobProcessing)
            && self.error.is_none()
    }
}

// =============================================================================
// 4. LoanCobParameter & CobPartition (`LoanCOBParameter.java` & `COBPartition.java`)
// =============================================================================

/// Inclusive range of loan IDs `[min_loan_id..=max_loan_id]` to be processed by COB.
///
/// Ported from `org.apache.fineract.cob.data.LoanCOBParameter`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanCobParameter {
    pub min_loan_id: i64,
    pub max_loan_id: i64,
}

impl LoanCobParameter {
    /// Creates a new [`LoanCobParameter`] covering `[min_loan_id..=max_loan_id]`.
    pub const fn new(min_loan_id: i64, max_loan_id: i64) -> Self {
        Self {
            min_loan_id,
            max_loan_id,
        }
    }

    /// Returns `true` if this parameter range contains no loans
    /// (`(min_loan_id == 0 && max_loan_id == 0) || max_loan_id < min_loan_id`).
    ///
    /// Matches `LoanCOBParameter.isEmpty()`.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        (self.min_loan_id == 0 && self.max_loan_id == 0) || self.max_loan_id < self.min_loan_id
    }

    /// Returns the number of integer IDs in `[min_loan_id..=max_loan_id]`.
    #[inline]
    pub fn id_span(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            (self.max_loan_id - self.min_loan_id + 1) as usize
        }
    }

    /// Returns `true` if `loan_id` falls within `[min_loan_id..=max_loan_id]`.
    #[inline]
    pub const fn contains(&self, loan_id: i64) -> bool {
        !self.is_empty() && loan_id >= self.min_loan_id && loan_id <= self.max_loan_id
    }
}

/// Partition descriptor produced by partitioning a [`LoanCobParameter`] range across COB workers.
///
/// Ported from `org.apache.fineract.cob.data.COBPartition` and `CommonPartitioner.java`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobPartition {
    pub partition_index: usize,
    pub min_loan_id: i64,
    pub max_loan_id: i64,
    pub page_size: usize,
}

impl CobPartition {
    /// Creates a new [`CobPartition`].
    pub const fn new(
        partition_index: usize,
        min_loan_id: i64,
        max_loan_id: i64,
        page_size: usize,
    ) -> Self {
        Self {
            partition_index,
            min_loan_id,
            max_loan_id,
            page_size,
        }
    }

    /// Returns the Fineract partition key (`"Partition_{partition_index}"`).
    pub fn partition_name(&self) -> String {
        format!("{}{}", LOAN_COB_PARTITION_PREFIX, self.partition_index)
    }

    /// Converts this partition's bounds into a [`LoanCobParameter`].
    #[inline]
    pub const fn to_cob_parameter(&self) -> LoanCobParameter {
        LoanCobParameter::new(self.min_loan_id, self.max_loan_id)
    }

    /// Returns `true` if this partition represents an empty range (`min_loan_id == 0 && max_loan_id == 0`).
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.to_cob_parameter().is_empty()
    }

    /// Divides `param` into contiguous [`CobPartition`] chunks of `partition_size` IDs each,
    /// matching `CommonPartitioner.partition` and `LoanCOBPartitioner.partition`.
    pub fn partition_range(
        param: Option<LoanCobParameter>,
        partition_size: usize,
        page_size: usize,
    ) -> Vec<CobPartition> {
        let effective_partition_size = partition_size.max(1) as i64;
        let effective_page_size = page_size.max(1);
        let Some(range) = param.filter(|p| !p.is_empty()) else {
            return vec![CobPartition::new(1, 0, 0, effective_page_size)];
        };

        let mut partitions = Vec::new();
        let mut partition_index = 1usize;
        let mut start = range.min_loan_id;
        while start <= range.max_loan_id {
            let end = (start + effective_partition_size - 1).min(range.max_loan_id);
            partitions.push(CobPartition::new(
                partition_index,
                start,
                end,
                effective_page_size,
            ));
            partition_index += 1;
            start = end + 1;
        }
        partitions
    }
}

// =============================================================================
// 5. LoanAggregate & Supporting Domain Models for COB Business Steps
// =============================================================================

/// Lifecycle status of a Loan (`m_loan.loan_status_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum CobLoanStatus {
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

impl CobLoanStatus {
    pub const ACTIVE: Self = Self::Active;
    pub const CLOSED_OBLIGATIONS_MET: Self = Self::ClosedObligationsMet;
    pub const CLOSED_WRITTEN_OFF: Self = Self::ClosedWrittenOff;
    pub const OVERPAID: Self = Self::Overpaid;

    #[inline]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    #[inline]
    pub const fn is_closed_or_overpaid(self) -> bool {
        matches!(
            self,
            Self::ClosedObligationsMet
                | Self::ClosedWrittenOff
                | Self::ClosedRescheduleOutstandingAmount
                | Self::Overpaid
                | Self::WithdrawnByClient
                | Self::Rejected
        )
    }
}

/// Calculation mode for an overdue penalty charge applied during COB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum OverdueChargeCalculationMode {
    /// Fixed flat amount (`ChargeCalculationType::Flat`).
    #[default]
    Flat,
    /// Percentage of overdue installment principal (`ChargeCalculationType::PercentOfAmount`).
    PercentOfAmount,
    /// Percentage of overdue installment interest (`ChargeCalculationType::PercentOfInterest`).
    PercentOfInterest,
    /// Percentage of overdue installment principal + interest (`ChargeCalculationType::PercentOfAmountAndInterest`).
    PercentOfAmountAndInterest,
}

/// Specification of an overdue penalty product charge attached to a loan or product.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverdueChargeSpec {
    pub charge_definition_id: i64,
    pub name: String,
    pub calculation_mode: OverdueChargeCalculationMode,
    pub amount: Decimal,
    pub penalty: bool,
}

impl OverdueChargeSpec {
    pub fn new_flat_penalty(
        charge_definition_id: i64,
        name: impl Into<String>,
        amount: Decimal,
    ) -> Self {
        Self {
            charge_definition_id,
            name: name.into(),
            calculation_mode: OverdueChargeCalculationMode::Flat,
            amount,
            penalty: true,
        }
    }

    pub fn new_percent_of_principal_penalty(
        charge_definition_id: i64,
        name: impl Into<String>,
        percentage: Decimal,
    ) -> Self {
        Self {
            charge_definition_id,
            name: name.into(),
            calculation_mode: OverdueChargeCalculationMode::PercentOfAmount,
            amount: percentage,
            penalty: true,
        }
    }

    /// Computes the charge amount for `installment`, rounding with `HALF_EVEN` to `currency_digits`.
    pub fn compute_amount(
        &self,
        installment: &CobRepaymentInstallment,
        currency_digits: u32,
    ) -> Decimal {
        let raw = match self.calculation_mode {
            OverdueChargeCalculationMode::Flat => self.amount,
            OverdueChargeCalculationMode::PercentOfAmount => {
                (installment.principal_outstanding() * self.amount) / dec!(100)
            }
            OverdueChargeCalculationMode::PercentOfInterest => {
                (installment.interest_outstanding() * self.amount) / dec!(100)
            }
            OverdueChargeCalculationMode::PercentOfAmountAndInterest => {
                ((installment.principal_outstanding() + installment.interest_outstanding())
                    * self.amount)
                    / dec!(100)
            }
        };
        raw.round_dp_with_strategy(currency_digits, RoundingStrategy::MidpointEven)
    }
}

/// Applied charge on a [`LoanAggregate`] (`m_loan_charge`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobLoanCharge {
    pub id: i64,
    pub charge_definition_id: i64,
    pub installment_number: Option<i32>,
    pub due_date: NaiveDate,
    pub amount: Decimal,
    pub amount_paid: Decimal,
    pub amount_waived: Decimal,
    pub amount_outstanding: Decimal,
    pub penalty: bool,
    pub active: bool,
}

/// Repayment schedule installment on a [`LoanAggregate`] (`m_loan_repayment_schedule`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobRepaymentInstallment {
    pub installment_number: i32,
    pub from_date: NaiveDate,
    pub due_date: NaiveDate,
    pub principal_amount: Decimal,
    pub principal_completed: Decimal,
    pub principal_written_off: Decimal,
    pub interest_charged: Decimal,
    pub interest_completed: Decimal,
    pub interest_waived: Decimal,
    pub interest_written_off: Decimal,
    pub fee_charges_charged: Decimal,
    pub fee_charges_completed: Decimal,
    pub fee_charges_waived: Decimal,
    pub fee_charges_written_off: Decimal,
    pub penalty_charges_charged: Decimal,
    pub penalty_charges_completed: Decimal,
    pub penalty_charges_waived: Decimal,
    pub penalty_charges_written_off: Decimal,
    pub obligations_met: bool,
    pub obligations_met_on_date: Option<NaiveDate>,
    pub recalculated_interest_component: bool,
    pub delinquency_range_id: Option<i64>,
}

impl CobRepaymentInstallment {
    /// Creates a new unpaid installment with `principal_amount` and `interest_charged`.
    pub fn new(
        installment_number: i32,
        from_date: NaiveDate,
        due_date: NaiveDate,
        principal_amount: Decimal,
        interest_charged: Decimal,
    ) -> Self {
        Self {
            installment_number,
            from_date,
            due_date,
            principal_amount,
            principal_completed: Decimal::ZERO,
            principal_written_off: Decimal::ZERO,
            interest_charged,
            interest_completed: Decimal::ZERO,
            interest_waived: Decimal::ZERO,
            interest_written_off: Decimal::ZERO,
            fee_charges_charged: Decimal::ZERO,
            fee_charges_completed: Decimal::ZERO,
            fee_charges_waived: Decimal::ZERO,
            fee_charges_written_off: Decimal::ZERO,
            penalty_charges_charged: Decimal::ZERO,
            penalty_charges_completed: Decimal::ZERO,
            penalty_charges_waived: Decimal::ZERO,
            penalty_charges_written_off: Decimal::ZERO,
            obligations_met: false,
            obligations_met_on_date: None,
            recalculated_interest_component: false,
            delinquency_range_id: None,
        }
    }

    #[inline]
    pub fn principal_outstanding(&self) -> Decimal {
        (self.principal_amount - self.principal_completed - self.principal_written_off)
            .max(Decimal::ZERO)
    }

    #[inline]
    pub fn interest_outstanding(&self) -> Decimal {
        (self.interest_charged
            - self.interest_completed
            - self.interest_waived
            - self.interest_written_off)
            .max(Decimal::ZERO)
    }

    #[inline]
    pub fn fee_charges_outstanding(&self) -> Decimal {
        (self.fee_charges_charged
            - self.fee_charges_completed
            - self.fee_charges_waived
            - self.fee_charges_written_off)
            .max(Decimal::ZERO)
    }

    #[inline]
    pub fn penalty_charges_outstanding(&self) -> Decimal {
        (self.penalty_charges_charged
            - self.penalty_charges_completed
            - self.penalty_charges_waived
            - self.penalty_charges_written_off)
            .max(Decimal::ZERO)
    }

    #[inline]
    pub fn total_outstanding(&self) -> Decimal {
        self.principal_outstanding()
            + self.interest_outstanding()
            + self.fee_charges_outstanding()
            + self.penalty_charges_outstanding()
    }

    #[inline]
    pub fn net_accrual_interest(&self) -> Decimal {
        (self.interest_charged - self.interest_waived).max(Decimal::ZERO)
    }

    #[inline]
    pub fn net_accrual_fees(&self) -> Decimal {
        (self.fee_charges_charged - self.fee_charges_waived).max(Decimal::ZERO)
    }

    #[inline]
    pub fn net_accrual_penalties(&self) -> Decimal {
        (self.penalty_charges_charged - self.penalty_charges_waived).max(Decimal::ZERO)
    }
}

/// Delinquency aging range bucket (`m_delinquency_range`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobDelinquencyRange {
    pub id: i64,
    pub classification: String,
    pub minimum_age_days: i64,
    pub maximum_age_days: Option<i64>,
}

impl CobDelinquencyRange {
    pub fn new(
        id: i64,
        classification: impl Into<String>,
        minimum_age_days: i64,
        maximum_age_days: Option<i64>,
    ) -> Self {
        Self {
            id,
            classification: classification.into(),
            minimum_age_days,
            maximum_age_days,
        }
    }

    /// Returns `true` if `overdue_days` falls inside `[minimum_age_days..=maximum_age_days]`.
    #[inline]
    pub fn matches(&self, overdue_days: i64) -> bool {
        overdue_days >= self.minimum_age_days
            && self
                .maximum_age_days
                .map_or(true, |max| overdue_days <= max)
    }
}

/// Delinquency tag history entry (`m_loan_delinquency_tag_history`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobDelinquencyTagHistory {
    pub id: i64,
    pub delinquency_range_id: i64,
    pub loan_id: i64,
    pub added_on_date: NaiveDate,
    pub lifted_on_date: Option<NaiveDate>,
}

impl CobDelinquencyTagHistory {
    #[inline]
    pub const fn is_active(&self) -> bool {
        self.lifted_on_date.is_none()
    }
}

/// Transaction posted against a [`LoanAggregate`] during COB (e.g. `ACCRUAL_ACTIVITY = 31`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobLoanTransaction {
    pub id: i64,
    pub transaction_type_enum: i16,
    pub transaction_date: NaiveDate,
    pub installment_number: Option<i32>,
    pub amount: Decimal,
    pub principal_portion: Decimal,
    pub interest_portion: Decimal,
    pub fee_charges_portion: Decimal,
    pub penalty_charges_portion: Decimal,
    pub reversed: bool,
}

impl CobLoanTransaction {
    pub const TYPE_ACCRUAL: i16 = 10;
    pub const TYPE_ACCRUAL_ACTIVITY: i16 = 31;

    pub fn new_accrual_activity(
        id: i64,
        transaction_date: NaiveDate,
        installment_number: i32,
        interest_portion: Decimal,
        fee_charges_portion: Decimal,
        penalty_charges_portion: Decimal,
    ) -> Self {
        let amount = interest_portion + fee_charges_portion + penalty_charges_portion;
        Self {
            id,
            transaction_type_enum: Self::TYPE_ACCRUAL_ACTIVITY,
            transaction_date,
            installment_number: Some(installment_number),
            amount,
            principal_portion: Decimal::ZERO,
            interest_portion,
            fee_charges_portion,
            penalty_charges_portion,
            reversed: false,
        }
    }
}

/// Domain business event emitted during a COB business step (`LoanRepaymentDueBusinessEvent`,
/// `LoanRepaymentOverdueBusinessEvent`, `LoanDelinquencyRangeChangeBusinessEvent`,
/// `LoanAccrualActivityCreatedBusinessEvent`, `LoanOwnershipTransferBusinessEvent`, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobBusinessEvent {
    pub event_type: String,
    pub category: String,
    pub loan_id: i64,
    pub business_date: NaiveDate,
    pub installment_number: Option<i32>,
    pub amount: Option<Decimal>,
    pub details: String,
}

/// Pure in-memory Loan Aggregate operated on by [`CobBusinessStep::execute`], [`InlineLoanCobExecutor`],
/// and [`PartitionedCobWorkerPool`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAggregate {
    pub id: i64,
    pub account_number: String,
    pub external_id: Option<ExternalId>,
    pub office_id: i64,
    pub client_id: i64,
    pub product_id: i64,
    pub currency_code: String,
    pub currency_digits: u32,
    pub status: CobLoanStatus,
    pub submitted_on_date: Option<NaiveDate>,
    pub approved_on_date: Option<NaiveDate>,
    pub disbursed_on_date: Option<NaiveDate>,
    pub last_closed_business_date: Option<NaiveDate>,
    pub principal_disbursed: Decimal,
    pub annual_interest_rate: Decimal,
    pub days_in_year: u32,
    pub grace_on_arrears_ageing: i64,
    pub penalty_wait_period_days: i64,
    pub grace_on_penalty_posting_days: i64,
    pub npa_overdue_days_threshold: Option<i64>,
    pub is_npa: bool,
    pub charged_off: bool,
    pub fraud: bool,
    pub is_interest_recalculation_enabled: bool,
    pub enable_accrual_activity_posting: bool,
    pub enable_installment_level_delinquency: bool,
    pub interest_recalculated_on: Option<NaiveDate>,
    pub overdue_days: i64,
    pub overdue_since_date: Option<NaiveDate>,
    pub delinquency_range_id: Option<i64>,
    pub delinquency_classification: Option<String>,
    pub external_owner_id: Option<String>,
    pub total_overpaid: Decimal,
    pub installments: Vec<CobRepaymentInstallment>,
    pub overdue_charge_specs: Vec<OverdueChargeSpec>,
    pub charges: Vec<CobLoanCharge>,
    pub delinquency_ranges: Vec<CobDelinquencyRange>,
    pub delinquency_tag_history: Vec<CobDelinquencyTagHistory>,
    pub transactions: Vec<CobLoanTransaction>,
    pub executed_steps_log: Vec<(NaiveDate, String)>,
    pub emitted_events: Vec<CobBusinessEvent>,
}

impl Default for LoanAggregate {
    fn default() -> Self {
        Self::new(1, None)
    }
}

impl LoanAggregate {
    /// Creates an active [`LoanAggregate`] with `id` and optional `last_closed_business_date`.
    pub fn new(id: i64, last_closed_business_date: Option<NaiveDate>) -> Self {
        Self {
            id,
            account_number: format!("{id:09}"),
            external_id: None,
            office_id: 1,
            client_id: 1,
            product_id: 1,
            currency_code: "USD".to_string(),
            currency_digits: 2,
            status: CobLoanStatus::Active,
            submitted_on_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            approved_on_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            disbursed_on_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            last_closed_business_date,
            principal_disbursed: dec!(1000.00),
            annual_interest_rate: dec!(12.00),
            days_in_year: 365,
            grace_on_arrears_ageing: 0,
            penalty_wait_period_days: 0,
            grace_on_penalty_posting_days: 0,
            npa_overdue_days_threshold: Some(90),
            is_npa: false,
            charged_off: false,
            fraud: false,
            is_interest_recalculation_enabled: false,
            enable_accrual_activity_posting: true,
            enable_installment_level_delinquency: false,
            interest_recalculated_on: None,
            overdue_days: 0,
            overdue_since_date: None,
            delinquency_range_id: None,
            delinquency_classification: None,
            external_owner_id: None,
            total_overpaid: Decimal::ZERO,
            installments: Vec::new(),
            overdue_charge_specs: Vec::new(),
            charges: Vec::new(),
            delinquency_ranges: Self::default_delinquency_ranges(),
            delinquency_tag_history: Vec::new(),
            transactions: Vec::new(),
            executed_steps_log: Vec::new(),
            emitted_events: Vec::new(),
        }
    }

    /// Standard 4-bucket delinquency aging schedule (`PAR_1_30`, `PAR_31_60`, `PAR_61_90`, `PAR_90_PLUS`).
    pub fn default_delinquency_ranges() -> Vec<CobDelinquencyRange> {
        vec![
            CobDelinquencyRange::new(1, "PAR_1_30", 1, Some(30)),
            CobDelinquencyRange::new(2, "PAR_31_60", 31, Some(60)),
            CobDelinquencyRange::new(3, "PAR_61_90", 61, Some(90)),
            CobDelinquencyRange::new(4, "PAR_90_PLUS", 91, None),
        ]
    }

    /// Builder helper to add a repayment schedule installment.
    pub fn with_installment(mut self, installment: CobRepaymentInstallment) -> Self {
        self.installments.push(installment);
        self
    }

    /// Builder helper to add an overdue penalty charge specification.
    pub fn with_overdue_charge_spec(mut self, spec: OverdueChargeSpec) -> Self {
        self.overdue_charge_specs.push(spec);
        self
    }

    /// Builder helper to enable interest recalculation.
    pub fn with_interest_recalculation(mut self, enabled: bool) -> Self {
        self.is_interest_recalculation_enabled = enabled;
        self
    }

    /// Computes total principal outstanding across all installments.
    pub fn total_principal_outstanding(&self) -> Decimal {
        if self.installments.is_empty() {
            self.principal_disbursed
        } else {
            self.installments
                .iter()
                .map(CobRepaymentInstallment::principal_outstanding)
                .sum()
        }
    }

    /// Computes total interest outstanding across all installments.
    pub fn total_interest_outstanding(&self) -> Decimal {
        self.installments
            .iter()
            .map(CobRepaymentInstallment::interest_outstanding)
            .sum()
    }

    /// Computes total fee charges outstanding across all installments.
    pub fn total_fee_charges_outstanding(&self) -> Decimal {
        self.installments
            .iter()
            .map(CobRepaymentInstallment::fee_charges_outstanding)
            .sum()
    }

    /// Computes total penalty charges outstanding across all installments.
    pub fn total_penalty_charges_outstanding(&self) -> Decimal {
        self.installments
            .iter()
            .map(CobRepaymentInstallment::penalty_charges_outstanding)
            .sum()
    }

    /// Computes total outstanding balance across all installments.
    pub fn total_outstanding(&self) -> Decimal {
        self.total_principal_outstanding()
            + self.total_interest_outstanding()
            + self.total_fee_charges_outstanding()
            + self.total_penalty_charges_outstanding()
    }

    /// Converts this loan's current balances into a [`LoanTransferSnapshot`] for [`ExternalAssetOwnerTransferEngine`].
    pub fn to_transfer_snapshot(&self) -> LoanTransferSnapshot {
        let mut snap = LoanTransferSnapshot::new(
            self.id,
            self.total_principal_outstanding(),
            self.total_interest_outstanding(),
            self.total_fee_charges_outstanding(),
            self.total_penalty_charges_outstanding(),
            self.total_overpaid,
        );
        snap.loan_account_number = Some(self.account_number.clone());
        snap.loan_external_id = self.external_id.clone();
        snap.loan_product_id = self.product_id;
        snap.office_id = self.office_id;
        snap.currency_code = self.currency_code.clone();
        snap.currency_digits = self.currency_digits;
        snap.charged_off = self.charged_off;
        snap.fraud = self.fraud;
        snap
    }

    /// Returns `true` if the loan requires COB processing for `target_cob_date`.
    pub fn needs_cob(&self, target_cob_date: NaiveDate) -> bool {
        if !self.status.is_active() {
            return false;
        }
        match self.last_closed_business_date {
            Some(last_closed) => last_closed < target_cob_date,
            None => {
                let anchor = self
                    .disbursed_on_date
                    .or(self.submitted_on_date)
                    .unwrap_or(target_cob_date);
                anchor <= target_cob_date
            }
        }
    }
}

// =============================================================================
// 6. CobBusinessStep Trait & 7 Built-in Loan COB Business Steps
// =============================================================================

/// Asynchronous Close of Business (COB) step executed sequentially on a [`LoanAggregate`].
///
/// Ported from `org.apache.fineract.cob.COBBusinessStep`.
#[async_trait]
pub trait CobBusinessStep: Send + Sync {
    /// Human-readable step name (`COBBusinessStep.getHumanReadableStepName()`).
    fn step_name(&self) -> &'static str;

    /// Canonical `SCREAMING_SNAKE_CASE` step identifier stored in `m_batch_business_steps.step_name`
    /// (`COBBusinessStep.getEnumStyledName()`).
    fn enum_styled_name(&self) -> &'static str;

    /// Executes this COB business step on `loan` for `cob_business_date`.
    async fn execute(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Result<(), FineractError>;
}

// -----------------------------------------------------------------------------
// Step 1: ApplyChargeToOverdueLoansBusinessStep (`APPLY_CHARGE_TO_OVERDUE_LOANS`)
// -----------------------------------------------------------------------------

/// Applies configured overdue penalty/fee charges to installments that are past due as of `cob_business_date`.
///
/// Ported from `org.apache.fineract.cob.loan.ApplyChargeToOverdueLoansBusinessStep`.
#[derive(Debug, Clone, Default)]
pub struct ApplyChargeToOverdueLoansBusinessStep {
    pub default_overdue_specs: Vec<OverdueChargeSpec>,
}

impl ApplyChargeToOverdueLoansBusinessStep {
    pub const ENUM_STYLED_NAME: &'static str = "APPLY_CHARGE_TO_OVERDUE_LOANS";
    pub const STEP_NAME: &'static str = "Apply charge to overdue loans";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_specs(specs: Vec<OverdueChargeSpec>) -> Self {
        Self {
            default_overdue_specs: specs,
        }
    }
}

#[async_trait]
impl CobBusinessStep for ApplyChargeToOverdueLoansBusinessStep {
    fn step_name(&self) -> &'static str {
        Self::STEP_NAME
    }

    fn enum_styled_name(&self) -> &'static str {
        Self::ENUM_STYLED_NAME
    }

    async fn execute(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Result<(), FineractError> {
        loan.executed_steps_log
            .push((cob_business_date, Self::ENUM_STYLED_NAME.to_string()));

        if !loan.status.is_active() || loan.charged_off {
            return Ok(());
        }

        let specs: Vec<OverdueChargeSpec> = if !loan.overdue_charge_specs.is_empty() {
            loan.overdue_charge_specs.clone()
        } else {
            self.default_overdue_specs.clone()
        };
        if specs.is_empty() {
            return Ok(());
        }

        let wait_cutoff = cob_business_date - Duration::days(loan.penalty_wait_period_days.max(0));
        let grace_cutoff =
            cob_business_date - Duration::days(loan.grace_on_penalty_posting_days.max(0));
        let currency_digits = loan.currency_digits;
        let mut next_charge_id = loan.charges.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let mut new_charges = Vec::new();
        let mut new_events = Vec::new();

        for inst in &mut loan.installments {
            if inst.obligations_met || inst.total_outstanding() <= Decimal::ZERO {
                continue;
            }
            // Installment must be strictly overdue (due_date < cob_business_date) and past wait/grace cutoffs
            if inst.due_date >= cob_business_date
                || inst.due_date > wait_cutoff
                || inst.due_date > grace_cutoff
            {
                continue;
            }

            for spec in &specs {
                let already_applied = loan.charges.iter().any(|c| {
                    c.active
                        && c.charge_definition_id == spec.charge_definition_id
                        && c.installment_number == Some(inst.installment_number)
                });
                if already_applied {
                    continue;
                }

                let charge_amount = spec.compute_amount(inst, currency_digits);
                if charge_amount <= Decimal::ZERO {
                    continue;
                }

                if spec.penalty {
                    inst.penalty_charges_charged += charge_amount;
                } else {
                    inst.fee_charges_charged += charge_amount;
                }

                new_charges.push(CobLoanCharge {
                    id: next_charge_id,
                    charge_definition_id: spec.charge_definition_id,
                    installment_number: Some(inst.installment_number),
                    due_date: inst.due_date,
                    amount: charge_amount,
                    amount_paid: Decimal::ZERO,
                    amount_waived: Decimal::ZERO,
                    amount_outstanding: charge_amount,
                    penalty: spec.penalty,
                    active: true,
                });
                new_events.push(CobBusinessEvent {
                    event_type: "LoanApplyOverdueChargeBusinessEvent".to_string(),
                    category: "LOAN".to_string(),
                    loan_id: loan.id,
                    business_date: cob_business_date,
                    installment_number: Some(inst.installment_number),
                    amount: Some(charge_amount),
                    details: format!(
                        "Applied overdue charge '{}' ({}) to installment #{}",
                        spec.name, charge_amount, inst.installment_number
                    ),
                });
                next_charge_id += 1;
            }
        }

        loan.charges.extend(new_charges);
        loan.emitted_events.extend(new_events);
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Step 2: LoanDelinquencyClassificationBusinessStep (`LOAN_DELINQUENCY_CLASSIFICATION`)
// -----------------------------------------------------------------------------

/// Evaluates loan and installment delinquency aging buckets (`m_loan_delinquency_tag_history`),
/// updates `delinquency_range_id` and `is_npa`, and emits `LoanDelinquencyRangeChangeBusinessEvent`.
///
/// Ported from `org.apache.fineract.cob.loan.SetLoanDelinquencyTagsBusinessStep`.
#[derive(Debug, Clone, Default)]
pub struct LoanDelinquencyClassificationBusinessStep;

/// Alias matching the upstream Java class name `SetLoanDelinquencyTagsBusinessStep`.
pub type SetLoanDelinquencyTagsBusinessStep = LoanDelinquencyClassificationBusinessStep;

impl LoanDelinquencyClassificationBusinessStep {
    pub const ENUM_STYLED_NAME: &'static str = "LOAN_DELINQUENCY_CLASSIFICATION";
    pub const STEP_NAME: &'static str = "Loan delinquency classification";

    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CobBusinessStep for LoanDelinquencyClassificationBusinessStep {
    fn step_name(&self) -> &'static str {
        Self::STEP_NAME
    }

    fn enum_styled_name(&self) -> &'static str {
        Self::ENUM_STYLED_NAME
    }

    async fn execute(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Result<(), FineractError> {
        loan.executed_steps_log
            .push((cob_business_date, Self::ENUM_STYLED_NAME.to_string()));

        // If loan is closed/overpaid/inactive, lift any active delinquency tags
        if !loan.status.is_active() {
            let mut lifted = false;
            for tag in &mut loan.delinquency_tag_history {
                if tag.lifted_on_date.is_none() {
                    tag.lifted_on_date = Some(cob_business_date);
                    lifted = true;
                }
            }
            loan.overdue_days = 0;
            loan.overdue_since_date = None;
            loan.delinquency_range_id = None;
            loan.delinquency_classification = None;
            if lifted {
                loan.emitted_events.push(CobBusinessEvent {
                    event_type: "LoanDelinquencyRangeChangeBusinessEvent".to_string(),
                    category: "LOAN".to_string(),
                    loan_id: loan.id,
                    business_date: cob_business_date,
                    installment_number: None,
                    amount: None,
                    details: "Delinquency range cleared for non-active loan".to_string(),
                });
            }
            return Ok(());
        }

        // Find the earliest unpaid installment due before cob_business_date
        let earliest_overdue_due_date = loan
            .installments
            .iter()
            .filter(|inst| {
                !inst.obligations_met
                    && inst.total_outstanding() > Decimal::ZERO
                    && inst.due_date < cob_business_date
            })
            .map(|inst| inst.due_date)
            .min();

        let raw_overdue_days = earliest_overdue_due_date
            .map(|due| (cob_business_date - due).num_days().max(0))
            .unwrap_or(0);

        let effective_overdue_days = if raw_overdue_days > loan.grace_on_arrears_ageing.max(0) {
            raw_overdue_days
        } else {
            0
        };

        loan.overdue_days = effective_overdue_days;
        loan.overdue_since_date = if effective_overdue_days > 0 {
            earliest_overdue_due_date
        } else {
            None
        };

        // Update NPA flag if threshold is configured
        if let Some(npa_threshold) = loan.npa_overdue_days_threshold {
            loan.is_npa = effective_overdue_days >= npa_threshold;
        }

        // Update installment-level delinquency ranges if enabled
        if loan.enable_installment_level_delinquency {
            let ranges = loan.delinquency_ranges.clone();
            for inst in &mut loan.installments {
                if !inst.obligations_met
                    && inst.total_outstanding() > Decimal::ZERO
                    && inst.due_date < cob_business_date
                {
                    let inst_days = (cob_business_date - inst.due_date).num_days().max(0);
                    inst.delinquency_range_id = ranges
                        .iter()
                        .find(|r| r.matches(inst_days))
                        .map(|r| r.id);
                } else {
                    inst.delinquency_range_id = None;
                }
            }
        }

        let matched_range = if effective_overdue_days > 0 {
            loan.delinquency_ranges
                .iter()
                .find(|r| r.matches(effective_overdue_days))
                .cloned()
        } else {
            None
        };

        let previous_range_id = loan.delinquency_range_id;
        let new_range_id = matched_range.as_ref().map(|r| r.id);

        if previous_range_id != new_range_id {
            // Lift any active tag
            for tag in &mut loan.delinquency_tag_history {
                if tag.lifted_on_date.is_none() {
                    tag.lifted_on_date = Some(cob_business_date);
                }
            }

            if let Some(ref range) = matched_range {
                let next_tag_id = loan
                    .delinquency_tag_history
                    .iter()
                    .map(|t| t.id)
                    .max()
                    .unwrap_or(0)
                    + 1;
                loan.delinquency_tag_history.push(CobDelinquencyTagHistory {
                    id: next_tag_id,
                    delinquency_range_id: range.id,
                    loan_id: loan.id,
                    added_on_date: cob_business_date,
                    lifted_on_date: None,
                });
                loan.delinquency_range_id = Some(range.id);
                loan.delinquency_classification = Some(range.classification.clone());
            } else {
                loan.delinquency_range_id = None;
                loan.delinquency_classification = None;
            }

            loan.emitted_events.push(CobBusinessEvent {
                event_type: "LoanDelinquencyRangeChangeBusinessEvent".to_string(),
                category: "LOAN".to_string(),
                loan_id: loan.id,
                business_date: cob_business_date,
                installment_number: None,
                amount: None,
                details: format!(
                    "Delinquency range changed from {:?} to {:?} (overdue_days={})",
                    previous_range_id, new_range_id, effective_overdue_days
                ),
            });
        }

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Step 3: LoanInterestRecalculationBusinessStep (`INTEREST_RECALCULATION`)
// -----------------------------------------------------------------------------

/// Recalculates interest on active loans with interest recalculation enabled as of `cob_business_date`.
///
/// Ported from `org.apache.fineract.cob.loan.LoanInterestRecalculationCOBBusinessStep`.
#[derive(Debug, Clone, Default)]
pub struct LoanInterestRecalculationBusinessStep;

/// Alias matching the upstream Java class name `LoanInterestRecalculationCOBBusinessStep`.
pub type LoanInterestRecalculationCobBusinessStep = LoanInterestRecalculationBusinessStep;

impl LoanInterestRecalculationBusinessStep {
    pub const ENUM_STYLED_NAME: &'static str = "INTEREST_RECALCULATION";
    pub const STEP_NAME: &'static str = "Loan interest recalculation";

    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CobBusinessStep for LoanInterestRecalculationBusinessStep {
    fn step_name(&self) -> &'static str {
        Self::STEP_NAME
    }

    fn enum_styled_name(&self) -> &'static str {
        Self::ENUM_STYLED_NAME
    }

    async fn execute(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Result<(), FineractError> {
        loan.executed_steps_log
            .push((cob_business_date, Self::ENUM_STYLED_NAME.to_string()));

        if !loan.status.is_active()
            || !loan.is_interest_recalculation_enabled
            || loan.charged_off
            || loan.is_npa
        {
            return Ok(());
        }

        let days_in_year = Decimal::from(loan.days_in_year.max(360));
        let daily_rate = (loan.annual_interest_rate / dec!(100)) / days_in_year;
        let currency_digits = loan.currency_digits;
        let mut modified = false;

        for inst in &mut loan.installments {
            if inst.obligations_met || inst.principal_outstanding() <= Decimal::ZERO {
                continue;
            }
            // If an installment is overdue as of cob_business_date, accrue 1 day of additional interest on overdue principal
            if inst.due_date < cob_business_date {
                let daily_overdue_interest = (inst.principal_outstanding() * daily_rate)
                    .round_dp_with_strategy(currency_digits, RoundingStrategy::MidpointEven);
                if daily_overdue_interest > Decimal::ZERO {
                    inst.interest_charged += daily_overdue_interest;
                    inst.recalculated_interest_component = true;
                    modified = true;
                }
            }
        }

        loan.interest_recalculated_on = Some(cob_business_date);
        if modified {
            loan.emitted_events.push(CobBusinessEvent {
                event_type: "LoanScheduleVariationsAddedBusinessEvent".to_string(),
                category: "LOAN".to_string(),
                loan_id: loan.id,
                business_date: cob_business_date,
                installment_number: None,
                amount: Some(loan.total_interest_outstanding()),
                details: format!(
                    "Recalculated loan schedule interest as of {}",
                    cob_business_date
                ),
            });
        }

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Step 4: AccrualActivityPostingBusinessStep (`ACCRUAL_ACTIVITY_POSTING`)
// -----------------------------------------------------------------------------

/// Posts `ACCRUAL_ACTIVITY` (`transaction_type_enum = 31`) transactions on installment due dates.
///
/// Ported from `org.apache.fineract.cob.loan.AccrualActivityPostingBusinessStep`.
#[derive(Debug, Clone, Default)]
pub struct AccrualActivityPostingBusinessStep;

impl AccrualActivityPostingBusinessStep {
    pub const ENUM_STYLED_NAME: &'static str = "ACCRUAL_ACTIVITY_POSTING";
    pub const STEP_NAME: &'static str = "Accrual Activity Posting";

    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CobBusinessStep for AccrualActivityPostingBusinessStep {
    fn step_name(&self) -> &'static str {
        Self::STEP_NAME
    }

    fn enum_styled_name(&self) -> &'static str {
        Self::ENUM_STYLED_NAME
    }

    async fn execute(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Result<(), FineractError> {
        loan.executed_steps_log
            .push((cob_business_date, Self::ENUM_STYLED_NAME.to_string()));

        if !loan.status.is_active() || !loan.enable_accrual_activity_posting {
            return Ok(());
        }

        let mut next_tx_id = loan.transactions.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let mut new_transactions = Vec::new();
        let mut new_events = Vec::new();

        for inst in &loan.installments {
            if inst.due_date != cob_business_date {
                continue;
            }

            let interest = inst.net_accrual_interest();
            let fees = inst.net_accrual_fees();
            let penalties = inst.net_accrual_penalties();
            let total = interest + fees + penalties;
            if total <= Decimal::ZERO {
                continue;
            }

            // Check if an identical non-reversed ACCRUAL_ACTIVITY already exists on this date
            let mut identical_exists = false;
            for existing in &mut loan.transactions {
                if !existing.reversed
                    && existing.transaction_type_enum == CobLoanTransaction::TYPE_ACCRUAL_ACTIVITY
                    && existing.transaction_date == cob_business_date
                    && existing.installment_number == Some(inst.installment_number)
                {
                    if existing.interest_portion == interest
                        && existing.fee_charges_portion == fees
                        && existing.penalty_charges_portion == penalties
                    {
                        identical_exists = true;
                    } else {
                        // Reverse stale accrual activity so updated portions can be posted
                        existing.reversed = true;
                    }
                }
            }

            if identical_exists {
                continue;
            }

            let tx = CobLoanTransaction::new_accrual_activity(
                next_tx_id,
                cob_business_date,
                inst.installment_number,
                interest,
                fees,
                penalties,
            );
            next_tx_id += 1;

            new_events.push(CobBusinessEvent {
                event_type: "LoanAccrualActivityCreatedBusinessEvent".to_string(),
                category: "LOAN".to_string(),
                loan_id: loan.id,
                business_date: cob_business_date,
                installment_number: Some(inst.installment_number),
                amount: Some(tx.amount),
                details: format!(
                    "Posted ACCRUAL_ACTIVITY for installment #{} on {}",
                    inst.installment_number, cob_business_date
                ),
            });
            new_transactions.push(tx);
        }

        loan.transactions.extend(new_transactions);
        loan.emitted_events.extend(new_events);
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Step 5: ExternalAssetOwnerTransferBusinessStep (`EXTERNAL_ASSET_OWNER_TRANSFER`)
// -----------------------------------------------------------------------------

/// Settles pending External Asset Owner (Investor) loan Sale and Buyback transfers on `cob_business_date`.
///
/// Ported from `org.apache.fineract.investor.cob.loan.LoanAccountOwnerTransferBusinessStep`.
#[derive(Debug, Clone)]
pub struct ExternalAssetOwnerTransferBusinessStep {
    engine: ExternalAssetOwnerTransferEngine,
}

/// Alias matching the upstream Java class name `LoanAccountOwnerTransferBusinessStep`.
pub type LoanAccountOwnerTransferBusinessStep = ExternalAssetOwnerTransferBusinessStep;

impl Default for ExternalAssetOwnerTransferBusinessStep {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ExternalAssetOwnerTransferBusinessStep {
    pub const ENUM_STYLED_NAME: &'static str = "EXTERNAL_ASSET_OWNER_TRANSFER";
    pub const STEP_NAME: &'static str =
        "Iterates through loan external asset owner transfers and executes them";

    pub fn new(engine: ExternalAssetOwnerTransferEngine) -> Self {
        Self { engine }
    }

    pub fn from_repository(repository: Arc<DualDbInvestorRepository>) -> Self {
        Self {
            engine: ExternalAssetOwnerTransferEngine::new(repository),
        }
    }

    pub fn in_memory() -> Self {
        Self {
            engine: ExternalAssetOwnerTransferEngine::in_memory(),
        }
    }

    pub fn engine(&self) -> &ExternalAssetOwnerTransferEngine {
        &self.engine
    }
}

#[async_trait]
impl CobBusinessStep for ExternalAssetOwnerTransferBusinessStep {
    fn step_name(&self) -> &'static str {
        Self::STEP_NAME
    }

    fn enum_styled_name(&self) -> &'static str {
        Self::ENUM_STYLED_NAME
    }

    async fn execute(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Result<(), FineractError> {
        loan.executed_steps_log
            .push((cob_business_date, Self::ENUM_STYLED_NAME.to_string()));

        let snapshot = loan.to_transfer_snapshot();
        let outcome = self
            .engine
            .execute_cob_settlement(&snapshot, cob_business_date)?;

        loan.external_owner_id = outcome.loan_account_data.external_owner_id.clone();

        for ev in &outcome.business_events {
            loan.emitted_events.push(CobBusinessEvent {
                event_type: ev.event_type.clone(),
                category: ev.category.clone(),
                loan_id: loan.id,
                business_date: cob_business_date,
                installment_number: None,
                amount: ev.payload.total_outstanding_balance_amount,
                details: format!(
                    "External asset owner transfer #{} settled with status {}",
                    ev.transfer_id, ev.transfer_status
                ),
            });
        }

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Step 6: CheckLoanRepaymentDueBusinessStep (`CHECK_LOAN_REPAYMENT_DUE`)
// -----------------------------------------------------------------------------

/// Checks whether any installment repayment becomes due in `days_before_repayment_is_due` days
/// from `cob_business_date` and emits `LoanRepaymentDueBusinessEvent`.
///
/// Ported from `org.apache.fineract.cob.loan.CheckLoanRepaymentDueBusinessStep`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckLoanRepaymentDueBusinessStep {
    pub days_before_repayment_is_due: i64,
}

impl CheckLoanRepaymentDueBusinessStep {
    pub const ENUM_STYLED_NAME: &'static str = "CHECK_LOAN_REPAYMENT_DUE";
    pub const STEP_NAME: &'static str = "Check loan repayment due";

    pub const fn new(days_before_repayment_is_due: i64) -> Self {
        Self {
            days_before_repayment_is_due,
        }
    }
}

#[async_trait]
impl CobBusinessStep for CheckLoanRepaymentDueBusinessStep {
    fn step_name(&self) -> &'static str {
        Self::STEP_NAME
    }

    fn enum_styled_name(&self) -> &'static str {
        Self::ENUM_STYLED_NAME
    }

    async fn execute(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Result<(), FineractError> {
        loan.executed_steps_log
            .push((cob_business_date, Self::ENUM_STYLED_NAME.to_string()));

        if !loan.status.is_active() {
            return Ok(());
        }

        let lookahead = Duration::days(self.days_before_repayment_is_due.max(0));
        for inst in &loan.installments {
            if !inst.obligations_met
                && inst.total_outstanding() > Decimal::ZERO
                && (inst.due_date - lookahead) == cob_business_date
            {
                loan.emitted_events.push(CobBusinessEvent {
                    event_type: "LoanRepaymentDueBusinessEvent".to_string(),
                    category: "LOAN".to_string(),
                    loan_id: loan.id,
                    business_date: cob_business_date,
                    installment_number: Some(inst.installment_number),
                    amount: Some(inst.total_outstanding()),
                    details: format!(
                        "Installment #{} due on {} (outstanding={})",
                        inst.installment_number,
                        inst.due_date,
                        inst.total_outstanding()
                    ),
                });
                break;
            }
        }

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Step 7: CheckLoanRepaymentOverdueBusinessStep (`CHECK_LOAN_REPAYMENT_OVERDUE`)
// -----------------------------------------------------------------------------

/// Checks whether any unpaid installment reached `due_date + days_after_repayment_is_overdue == cob_business_date`
/// and emits `LoanRepaymentOverdueBusinessEvent`.
///
/// Ported from `org.apache.fineract.cob.loan.CheckLoanRepaymentOverdueBusinessStep`.
#[derive(Debug, Clone, Copy)]
pub struct CheckLoanRepaymentOverdueBusinessStep {
    pub days_after_repayment_is_overdue: i64,
}

impl Default for CheckLoanRepaymentOverdueBusinessStep {
    fn default() -> Self {
        Self {
            days_after_repayment_is_overdue: 1,
        }
    }
}

impl CheckLoanRepaymentOverdueBusinessStep {
    pub const ENUM_STYLED_NAME: &'static str = "CHECK_LOAN_REPAYMENT_OVERDUE";
    pub const STEP_NAME: &'static str = "Check loan repayment overdue";

    pub const fn new(days_after_repayment_is_overdue: i64) -> Self {
        Self {
            days_after_repayment_is_overdue,
        }
    }
}

#[async_trait]
impl CobBusinessStep for CheckLoanRepaymentOverdueBusinessStep {
    fn step_name(&self) -> &'static str {
        Self::STEP_NAME
    }

    fn enum_styled_name(&self) -> &'static str {
        Self::ENUM_STYLED_NAME
    }

    async fn execute(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Result<(), FineractError> {
        loan.executed_steps_log
            .push((cob_business_date, Self::ENUM_STYLED_NAME.to_string()));

        if !loan.status.is_active() || loan.status.is_closed_or_overpaid() {
            return Ok(());
        }

        let offset = Duration::days(self.days_after_repayment_is_overdue.max(0));
        for inst in &loan.installments {
            if !inst.obligations_met
                && inst.total_outstanding() > Decimal::ZERO
                && (inst.due_date + offset) == cob_business_date
            {
                loan.emitted_events.push(CobBusinessEvent {
                    event_type: "LoanRepaymentOverdueBusinessEvent".to_string(),
                    category: "LOAN".to_string(),
                    loan_id: loan.id,
                    business_date: cob_business_date,
                    installment_number: Some(inst.installment_number),
                    amount: Some(inst.total_outstanding()),
                    details: format!(
                        "Installment #{} overdue since {} (outstanding={})",
                        inst.installment_number,
                        inst.due_date,
                        inst.total_outstanding()
                    ),
                });
                break;
            }
        }

        Ok(())
    }
}

/// Returns the standard 7 built-in Loan COB Business Steps in canonical execution order.
pub fn default_loan_cob_business_steps() -> Vec<Arc<dyn CobBusinessStep>> {
    vec![
        Arc::new(ApplyChargeToOverdueLoansBusinessStep::new()),
        Arc::new(LoanDelinquencyClassificationBusinessStep::new()),
        Arc::new(LoanInterestRecalculationBusinessStep::new()),
        Arc::new(AccrualActivityPostingBusinessStep::new()),
        Arc::new(ExternalAssetOwnerTransferBusinessStep::in_memory()),
        Arc::new(CheckLoanRepaymentDueBusinessStep::default()),
        Arc::new(CheckLoanRepaymentOverdueBusinessStep::default()),
    ]
}

// =============================================================================
// 7. LoanAccountLockManager (`m_loan_account_locks`)
// =============================================================================

/// Dual-dialect (`PostgreSQL` + `MySQL 8.0+`) and in-memory manager for `m_loan_account_locks`.
///
/// Ported from:
/// - `org.apache.fineract.cob.loan.LoanLockingServiceImpl`
/// - `org.apache.fineract.cob.listener.AbstractLoanItemListener`
/// - `org.apache.fineract.cob.listener.ChunkProcessingLoanItemListener`
#[derive(Debug)]
pub struct LoanAccountLockManager {
    dialect: SqlDialect,
    pool: Option<AnyPool>,
    locks: DashMap<i64, LoanAccountLock>,
}

impl Default for LoanAccountLockManager {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl LoanAccountLockManager {
    pub const LOCK_COLUMNS: &'static str =
        "loan_id, version, lock_owner, lock_placed_on, error, stacktrace, lock_placed_on_cob_business_date";

    /// Creates a new [`LoanAccountLockManager`] backed by `sqlx::AnyPool` and [`SqlDialect`].
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            dialect,
            pool: Some(pool),
            locks: DashMap::new(),
        }
    }

    /// Creates an in-memory [`LoanAccountLockManager`] with `SqlDialect::Postgres` SQL generation.
    pub fn in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    /// Creates an in-memory [`LoanAccountLockManager`] configured for `dialect`.
    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            pool: None,
            locks: DashMap::new(),
        }
    }

    #[inline]
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    #[inline]
    pub const fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    // --- Dual-Dialect SQL Builders (PostgreSQL 16+ & MySQL 8.0+) ---

    /// Builds `SELECT ... FROM m_loan_account_locks WHERE loan_id = ? FOR UPDATE SKIP LOCKED`
    /// via [`DialectSql::for_update_skip_locked`] for both PostgreSQL and MySQL 8.0+.
    pub fn select_lock_for_update_skip_locked_sql(dialect: SqlDialect) -> String {
        let base = format!(
            "SELECT {} FROM m_loan_account_locks WHERE loan_id = ?",
            Self::LOCK_COLUMNS
        );
        DialectSql::for_update_skip_locked(&base, dialect)
    }

    /// Builds `SELECT ... FROM m_loan_account_locks WHERE loan_id >= ? AND loan_id <= ? FOR UPDATE SKIP LOCKED`
    /// via [`DialectSql::for_update_skip_locked`].
    pub fn select_range_locks_for_update_skip_locked_sql(dialect: SqlDialect) -> String {
        let base = format!(
            "SELECT {} FROM m_loan_account_locks WHERE loan_id >= ? AND loan_id <= ?",
            Self::LOCK_COLUMNS
        );
        DialectSql::for_update_skip_locked(&base, dialect)
    }

    /// Builds `INSERT INTO m_loan_account_locks (...) VALUES (?, ?, ?, ?, ?, ?, ?)`.
    pub fn insert_lock_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan_account_locks \
             (loan_id, version, lock_owner, lock_placed_on, error, stacktrace, lock_placed_on_cob_business_date) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    /// Builds `UPDATE m_loan_account_locks SET version = ?, lock_owner = ?, lock_placed_on = ?, error = NULL, stacktrace = NULL, lock_placed_on_cob_business_date = ? WHERE loan_id = ?`.
    pub fn update_lock_owner_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_loan_account_locks \
             SET version = ?, lock_owner = ?, lock_placed_on = ?, error = NULL, stacktrace = NULL, \
                 lock_placed_on_cob_business_date = ? \
             WHERE loan_id = ?",
            dialect,
        )
    }

    /// Builds `UPDATE m_loan_account_locks SET version = ?, error = ?, stacktrace = ? WHERE loan_id = ? AND lock_owner = ?`.
    pub fn update_lock_error_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_loan_account_locks \
             SET version = ?, error = ?, stacktrace = ? \
             WHERE loan_id = ? AND lock_owner = ?",
            dialect,
        )
    }

    /// Builds `DELETE FROM m_loan_account_locks WHERE loan_id = ? AND error IS NULL`.
    pub fn delete_successful_lock_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_loan_account_locks WHERE loan_id = ? AND error IS NULL",
            dialect,
        )
    }

    /// Builds `UPDATE m_loan SET last_closed_business_date = ? WHERE id = ?`.
    pub fn update_loan_last_closed_business_date_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_loan SET last_closed_business_date = ? WHERE id = ?",
            dialect,
        )
    }

    // --- Lock Acquisition, Upgrade, Error Recording & Release ---

    /// Applies a soft lock (`LockOwner::LoanCobChunkProcessing`, `version = 0`) on `loan_id`
    /// if no active lock currently exists.
    ///
    /// Ported from `LoanLockingServiceImpl.applySoftLock`.
    pub fn apply_soft_lock(
        &self,
        loan_id: i64,
        cob_business_date: NaiveDate,
    ) -> Result<LoanAccountLock, FineractError> {
        if let Some(existing) = self.locks.get(&loan_id) {
            if existing.lock_owner == LockOwner::LoanInlineCobProcessing && !existing.has_error() {
                return Err(FineractError::Conflict(
                    ApiGlobalErrorResponse::loan_is_locked(loan_id),
                ));
            }
            return Ok(existing.clone());
        }
        let soft = LoanAccountLock::new_soft_lock(loan_id, cob_business_date);
        self.locks.insert(loan_id, soft.clone());
        Ok(soft)
    }

    /// Upgrades a soft lock (or acquires a new chunk lock) to a hard lock (`LockOwner::LoanCobChunkProcessing`, `version >= 1`).
    ///
    /// Ported from `LoanLockingServiceImpl.upgradeToHardLock`.
    pub fn upgrade_to_hard_lock(
        &self,
        loan_id: i64,
        cob_business_date: NaiveDate,
    ) -> Result<LoanAccountLock, FineractError> {
        if let Some(mut existing) = self.locks.get_mut(&loan_id) {
            if existing.lock_owner == LockOwner::LoanInlineCobProcessing && !existing.has_error() {
                return Err(FineractError::Conflict(
                    ApiGlobalErrorResponse::loan_is_locked(loan_id),
                ));
            }
            existing.set_new_lock_owner(LockOwner::LoanCobChunkProcessing);
            existing.lock_placed_on_cob_business_date = Some(cob_business_date);
            return Ok(existing.clone());
        }

        let hard = LoanAccountLock::new_hard_lock(
            loan_id,
            LockOwner::LoanCobChunkProcessing,
            cob_business_date,
        );
        self.locks.insert(loan_id, hard.clone());
        Ok(hard)
    }

    /// Acquires a hard lock for `LockOwner::LoanCobChunkProcessing` on `loan_id`.
    pub fn acquire_chunk_lock(
        &self,
        loan_id: i64,
        cob_business_date: NaiveDate,
    ) -> Result<LoanAccountLock, FineractError> {
        self.upgrade_to_hard_lock(loan_id, cob_business_date)
    }

    /// Acquires a hard lock for `LockOwner::LoanInlineCobProcessing` on `loan_id`.
    ///
    /// - If `loan_id` is already locked by `LockOwner::LoanCobChunkProcessing` (without a failed step error),
    ///   returns `Err(FineractError::Conflict(ApiGlobalErrorResponse::loan_is_locked(loan_id)))` (`HTTP 409 Conflict`).
    /// - If `loan_id` is unlocked or has a prior failed lock (`error.is_some()`), acquires the lock
    ///   under `LockOwner::LoanInlineCobProcessing`.
    pub fn acquire_inline_lock(
        &self,
        loan_id: i64,
        cob_business_date: NaiveDate,
    ) -> Result<LoanAccountLock, FineractError> {
        if let Some(mut existing) = self.locks.get_mut(&loan_id) {
            if existing.lock_owner == LockOwner::LoanCobChunkProcessing && !existing.has_error() {
                return Err(FineractError::Conflict(
                    ApiGlobalErrorResponse::loan_is_locked(loan_id),
                ));
            }
            existing.set_new_lock_owner(LockOwner::LoanInlineCobProcessing);
            existing.lock_placed_on_cob_business_date = Some(cob_business_date);
            return Ok(existing.clone());
        }

        let lock = LoanAccountLock::new_hard_lock(
            loan_id,
            LockOwner::LoanInlineCobProcessing,
            cob_business_date,
        );
        self.locks.insert(loan_id, lock.clone());
        Ok(lock)
    }

    /// Records a COB business step failure (`error` and `stacktrace`) on the `LoanAccountLock`
    /// in `m_loan_account_locks` without aborting sibling loans in the chunk.
    ///
    /// Ported from `AbstractLoanItemListener.updateAccountLockWithError` and
    /// `ChunkProcessingLoanItemListener`.
    pub fn record_lock_error(
        &self,
        loan_id: i64,
        lock_owner: LockOwner,
        cob_business_date: NaiveDate,
        error: impl Into<String>,
        stacktrace: impl Into<String>,
    ) -> LoanAccountLock {
        let error_str = error.into();
        let stack_str = stacktrace.into();

        if let Some(mut existing) = self.locks.get_mut(&loan_id) {
            existing.lock_owner = lock_owner;
            existing.lock_placed_on_cob_business_date = Some(cob_business_date);
            existing.set_error(error_str, stack_str);
            return existing.clone();
        }

        let mut lock = LoanAccountLock::new_hard_lock(loan_id, lock_owner, cob_business_date);
        lock.set_error(error_str, stack_str);
        self.locks.insert(loan_id, lock.clone());
        lock
    }

    /// Releases the lock on `loan.id` upon successful COB completion and updates
    /// `loan.last_closed_business_date = Some(cob_business_date)`.
    ///
    /// Ported from `AbstractLoanItemProcessor.setLastClosedBusinessDate` and
    /// `AbstractLoanItemListener.afterWrite`.
    pub fn release_lock_and_close_loan(
        &self,
        loan: &mut LoanAggregate,
        cob_business_date: NaiveDate,
    ) -> Option<LoanAccountLock> {
        loan.last_closed_business_date = Some(cob_business_date);
        // Only remove lock if it does not hold an unrecovered error
        let should_remove = self
            .locks
            .get(&loan.id)
            .map(|l| !l.has_error())
            .unwrap_or(false);
        if should_remove {
            self.locks.remove(&loan.id).map(|(_, lock)| lock)
        } else {
            None
        }
    }

    /// Force-releases a lock on `loan_id` regardless of error state.
    pub fn release_lock(&self, loan_id: i64) -> Option<LoanAccountLock> {
        self.locks.remove(&loan_id).map(|(_, lock)| lock)
    }

    /// Inserts or overwrites a [`LoanAccountLock`] directly (useful for test setup or administrative recovery).
    pub fn put_lock(&self, lock: LoanAccountLock) {
        self.locks.insert(lock.loan_id, lock);
    }

    /// Returns the current [`LoanAccountLock`] for `loan_id`, if any.
    pub fn get_lock(&self, loan_id: i64) -> Option<LoanAccountLock> {
        self.locks.get(&loan_id).map(|l| l.clone())
    }

    /// Returns `true` if `loan_id` is currently locked by `LockOwner::LoanCobChunkProcessing` without error.
    pub fn is_locked_by_chunk_processing(&self, loan_id: i64) -> bool {
        self.locks
            .get(&loan_id)
            .map(|l| l.lock_owner == LockOwner::LoanCobChunkProcessing && !l.has_error())
            .unwrap_or(false)
    }

    /// Returns `true` if `loan_id` is currently locked by any owner without error.
    pub fn is_actively_locked(&self, loan_id: i64) -> bool {
        self.locks
            .get(&loan_id)
            .map(|l| !l.has_error())
            .unwrap_or(false)
    }

    /// Lists all locks ordered by `loan_id ASC`.
    pub fn list_locks(&self) -> Vec<LoanAccountLock> {
        let mut list: Vec<LoanAccountLock> = self.locks.iter().map(|e| e.value().clone()).collect();
        list.sort_by_key(|l| l.loan_id);
        list
    }

    /// Lists all locks that recorded a step failure (`error.is_some()`), ordered by `loan_id ASC`.
    pub fn list_failed_locks(&self) -> Vec<LoanAccountLock> {
        let mut list: Vec<LoanAccountLock> = self
            .locks
            .iter()
            .filter(|e| e.value().has_error())
            .map(|e| e.value().clone())
            .collect();
        list.sort_by_key(|l| l.loan_id);
        list
    }
}

// =============================================================================
// 8. Dual-DB / In-Memory Loan Store for COB Execution
// =============================================================================

/// Shared repository of [`LoanAggregate`]s and business dates consumed by [`InlineLoanCobExecutor`]
/// and [`PartitionedCobWorkerPool`].
#[derive(Debug)]
pub struct CobLoanStore {
    dialect: SqlDialect,
    pool: Option<AnyPool>,
    loans: DashMap<i64, LoanAggregate>,
    business_dates: DashMap<BusinessDateType, NaiveDate>,
}

impl Default for CobLoanStore {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl CobLoanStore {
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            dialect,
            pool: Some(pool),
            loans: DashMap::new(),
            business_dates: DashMap::new(),
        }
    }

    pub fn in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            pool: None,
            loans: DashMap::new(),
            business_dates: DashMap::new(),
        }
    }

    #[inline]
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    /// SQL query matching `LoanRepository.findAllLoansBehindByLoanIds`:
    /// selects loan IDs in `[min_loan_id..=max_loan_id]` whose `last_closed_business_date` is null or `< ?`.
    pub fn select_loans_behind_in_range_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id FROM m_loan \
             WHERE id >= ? AND id <= ? \
               AND loan_status_id = 300 \
               AND (last_closed_business_date IS NULL OR last_closed_business_date < ?) \
             ORDER BY id ASC",
            dialect,
        )
    }

    /// SQL query matching `LoanRepository.findMinAndMaxNonClosedLoanIdsByLastClosedBusinessDate`.
    pub fn select_min_max_loan_ids_behind_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT MIN(id) AS min_loan_id, MAX(id) AS max_loan_id FROM m_loan \
             WHERE loan_status_id = 300 \
               AND (last_closed_business_date IS NULL OR last_closed_business_date < ?)",
            dialect,
        )
    }

    /// SQL query matching `m_business_date` lookup.
    pub fn select_business_date_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT type, date, version FROM m_business_date WHERE type = ?",
            dialect,
        )
    }

    pub fn upsert_loan(&self, loan: LoanAggregate) {
        self.loans.insert(loan.id, loan);
    }

    pub fn get_loan(&self, loan_id: i64) -> Option<LoanAggregate> {
        self.loans.get(&loan_id).map(|l| l.clone())
    }

    pub fn set_business_date(&self, date_type: BusinessDateType, date: NaiveDate) {
        self.business_dates.insert(date_type, date);
    }

    pub fn get_business_date(&self, date_type: BusinessDateType) -> Option<NaiveDate> {
        self.business_dates.get(&date_type).map(|d| *d)
    }

    /// Returns all loans in `[min_loan_id..=max_loan_id]` that need COB for `cob_business_date`,
    /// ordered by `id ASC`.
    pub fn find_loans_behind_in_range(
        &self,
        min_loan_id: i64,
        max_loan_id: i64,
        cob_business_date: NaiveDate,
    ) -> Vec<LoanAggregate> {
        let mut list: Vec<LoanAggregate> = self
            .loans
            .iter()
            .filter(|e| {
                let l = e.value();
                l.id >= min_loan_id && l.id <= max_loan_id && l.needs_cob(cob_business_date)
            })
            .map(|e| e.value().clone())
            .collect();
        list.sort_by_key(|l| l.id);
        list
    }

    /// Computes [`LoanCobParameter`] (`min_loan_id`, `max_loan_id`) across all active loans behind `cob_business_date`.
    pub fn find_min_max_loan_ids_behind(&self, cob_business_date: NaiveDate) -> LoanCobParameter {
        let mut min_id: Option<i64> = None;
        let mut max_id: Option<i64> = None;
        for entry in self.loans.iter() {
            let loan = entry.value();
            if loan.needs_cob(cob_business_date) {
                min_id = Some(min_id.map_or(loan.id, |m| m.min(loan.id)));
                max_id = Some(max_id.map_or(loan.id, |m| m.max(loan.id)));
            }
        }
        match (min_id, max_id) {
            (Some(min), Some(max)) => LoanCobParameter::new(min, max),
            _ => LoanCobParameter::new(0, 0),
        }
    }

    /// Finds the oldest `last_closed_business_date` among active loans behind `cob_business_date`,
    /// matching `LoanCOBCatchUpServiceImpl.getOldestCOBProcessedLoan`.
    pub fn find_oldest_cob_processed_date(&self, cob_business_date: NaiveDate) -> Option<NaiveDate> {
        self.loans
            .iter()
            .filter(|e| e.value().needs_cob(cob_business_date))
            .filter_map(|e| {
                let l = e.value();
                l.last_closed_business_date
                    .or_else(|| l.disbursed_on_date.and_then(|d| d.pred_opt()))
            })
            .min()
    }
}

// =============================================================================
// 9. InlineLoanCobExecutor (`InlineLoanCOBExecutorServiceImpl.java`)
// =============================================================================

/// Summary of an Inline COB catch-up execution for a single loan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineCobExecutionReport {
    pub loan_id: i64,
    pub previous_closed_business_date: Option<NaiveDate>,
    pub new_closed_business_date: Option<NaiveDate>,
    pub dates_processed: Vec<NaiveDate>,
    pub steps_executed_count: usize,
}

/// Intercepts write commands targeting a `loan_id` and synchronously executes any missing
/// Close of Business (COB) days from `last_closed_business_date + 1 day` through `current_cob_date`.
///
/// Ported from `org.apache.fineract.cob.service.InlineLoanCOBExecutorServiceImpl` and
/// `org.apache.fineract.cob.service.InlineCommonLockableCOBExecutorService`.
#[derive(Clone)]
pub struct InlineLoanCobExecutor {
    lock_manager: Arc<LoanAccountLockManager>,
    loan_store: Arc<CobLoanStore>,
    steps: Vec<Arc<dyn CobBusinessStep>>,
    tenant_id: String,
}

impl fmt::Debug for InlineLoanCobExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineLoanCobExecutor")
            .field("tenant_id", &self.tenant_id)
            .field("steps_count", &self.steps.len())
            .finish()
    }
}

impl Default for InlineLoanCobExecutor {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl InlineLoanCobExecutor {
    /// Creates a new [`InlineLoanCobExecutor`] with the given [`LoanAccountLockManager`],
    /// [`CobLoanStore`], and ordered [`CobBusinessStep`] pipeline.
    pub fn new(
        lock_manager: Arc<LoanAccountLockManager>,
        loan_store: Arc<CobLoanStore>,
        steps: Vec<Arc<dyn CobBusinessStep>>,
    ) -> Self {
        Self {
            lock_manager,
            loan_store,
            steps,
            tenant_id: "default".to_string(),
        }
    }

    /// Creates an in-memory [`InlineLoanCobExecutor`] with the default 7 Loan COB Business Steps.
    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(LoanAccountLockManager::in_memory()),
            Arc::new(CobLoanStore::in_memory()),
            default_loan_cob_business_steps(),
        )
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = tenant_id.into();
        self
    }

    pub fn lock_manager(&self) -> &Arc<LoanAccountLockManager> {
        &self.lock_manager
    }

    pub fn loan_store(&self) -> &Arc<CobLoanStore> {
        &self.loan_store
    }

    pub fn steps(&self) -> &[Arc<dyn CobBusinessStep>] {
        &self.steps
    }

    /// Intercepts an incoming write command for `loan`:
    /// 1. If `loan.id` is locked by `LockOwner::LoanCobChunkProcessing` (without error), returns
    ///    `Err(FineractError::Conflict(ApiGlobalErrorResponse::loan_is_locked(loan.id)))` (`HTTP 409 Conflict`).
    /// 2. If `loan.last_closed_business_date < Some(current_cob_date)` (or `None`), acquires
    ///    `LockOwner::LoanInlineCobProcessing` and iteratively executes all configured [`CobBusinessStep`]s
    ///    day-by-day from `last_closed_business_date + 1 day` through `current_cob_date`.
    /// 3. Releases the `LoanInlineCobProcessing` lock upon completion and updates
    ///    `loan.last_closed_business_date = Some(current_cob_date)`.
    pub async fn execute_inline_cob(
        &self,
        loan: &mut LoanAggregate,
        current_cob_date: NaiveDate,
    ) -> Result<InlineCobExecutionReport, FineractError> {
        // 1. Check for active chunk processing lock -> 409 Conflict
        if self.lock_manager.is_locked_by_chunk_processing(loan.id) {
            return Err(FineractError::Conflict(
                ApiGlobalErrorResponse::loan_is_locked(loan.id),
            ));
        }

        let previous_closed = loan.last_closed_business_date;

        // 2. Determine start date for catch-up + inline COB
        let start_date = match loan.last_closed_business_date {
            Some(last_closed) => {
                if last_closed >= current_cob_date {
                    return Ok(InlineCobExecutionReport {
                        loan_id: loan.id,
                        previous_closed_business_date: previous_closed,
                        new_closed_business_date: loan.last_closed_business_date,
                        dates_processed: Vec::new(),
                        steps_executed_count: 0,
                    });
                }
                last_closed.succ_opt().unwrap_or(current_cob_date)
            }
            None => {
                let anchor = loan
                    .disbursed_on_date
                    .or(loan.submitted_on_date)
                    .unwrap_or(current_cob_date);
                if anchor > current_cob_date {
                    return Ok(InlineCobExecutionReport {
                        loan_id: loan.id,
                        previous_closed_business_date: previous_closed,
                        new_closed_business_date: loan.last_closed_business_date,
                        dates_processed: Vec::new(),
                        steps_executed_count: 0,
                    });
                }
                anchor
            }
        };

        // 3. Acquire LoanInlineCobProcessing lock
        self.lock_manager
            .acquire_inline_lock(loan.id, start_date)?;

        let mut dates_processed = Vec::new();
        let mut steps_executed_count = 0usize;
        let mut cursor_date = start_date;

        // 4. Iteratively execute all configured CobBusinessSteps day-by-day through current_cob_date
        while cursor_date <= current_cob_date {
            let business_date = cursor_date.succ_opt().unwrap_or(cursor_date);
            let ctx = ExecutionContext::new(&self.tenant_id, business_date)
                .with_business_date(BusinessDateType::BusinessDate, business_date)
                .with_business_date(BusinessDateType::CobDate, cursor_date)
                .with_action_context(ActionContext::Cob);

            let day_result: Result<usize, FineractError> = with_execution_context(ctx, async {
                let mut count = 0usize;
                for step in &self.steps {
                    if let Err(err) = step.execute(loan, cursor_date).await {
                        let stack = format!(
                            "InlineLoanCobExecutor failed at step '{}' ({}) for loan_id={} on cob_date={}: {}",
                            step.enum_styled_name(),
                            step.step_name(),
                            loan.id,
                            cursor_date,
                            err
                        );
                        self.lock_manager.record_lock_error(
                            loan.id,
                            LockOwner::LoanInlineCobProcessing,
                            cursor_date,
                            err.to_string(),
                            stack,
                        );
                        return Err(err);
                    }
                    count += 1;
                }
                Ok(count)
            })
            .await;

            match day_result {
                Ok(day_steps) => {
                    steps_executed_count += day_steps;
                    loan.last_closed_business_date = Some(cursor_date);
                    dates_processed.push(cursor_date);
                }
                Err(err) => {
                    return Err(err);
                }
            }

            let Some(next_date) = cursor_date.succ_opt() else {
                break;
            };
            cursor_date = next_date;
        }

        // 5. Release lock and persist updated loan in store
        self.lock_manager
            .release_lock_and_close_loan(loan, current_cob_date);
        self.loan_store.upsert_loan(loan.clone());

        Ok(InlineCobExecutionReport {
            loan_id: loan.id,
            previous_closed_business_date: previous_closed,
            new_closed_business_date: loan.last_closed_business_date,
            dates_processed,
            steps_executed_count,
        })
    }

    /// Intercepts an incoming write command targeting `loan_id` stored in [`CobLoanStore`],
    /// runs day-by-day Inline COB catch-up through `current_cob_date`, and then executes `command_fn`.
    pub async fn intercept_write_command<F, Fut, R>(
        &self,
        loan_id: i64,
        current_cob_date: NaiveDate,
        command_fn: F,
    ) -> Result<(InlineCobExecutionReport, R), FineractError>
    where
        F: FnOnce(&mut LoanAggregate) -> Fut,
        Fut: Future<Output = Result<R, FineractError>>,
    {
        if self.lock_manager.is_locked_by_chunk_processing(loan_id) {
            return Err(FineractError::Conflict(
                ApiGlobalErrorResponse::loan_is_locked(loan_id),
            ));
        }

        let mut loan = self.loan_store.get_loan(loan_id).ok_or_else(|| {
            FineractError::not_found(
                "error.msg.loan.id.invalid",
                format!("Loan with identifier {loan_id} does not exist"),
            )
        })?;

        let report = self.execute_inline_cob(&mut loan, current_cob_date).await?;
        let output = command_fn(&mut loan).await?;
        self.loan_store.upsert_loan(loan);
        Ok((report, output))
    }
}

// =============================================================================
// 10. PartitionedCobWorkerPool & Catch-Up Engine
// =============================================================================

/// Result of processing a single [`CobPartition`] worker task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobPartitionResult {
    pub partition: CobPartition,
    pub processed_loan_ids: Vec<i64>,
    pub failed_loan_ids: Vec<i64>,
    pub skipped_locked_loan_ids: Vec<i64>,
}

/// Aggregate summary returned by [`PartitionedCobWorkerPool::execute_range`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobBatchExecutionSummary {
    pub cob_business_date: NaiveDate,
    pub parameter: LoanCobParameter,
    pub partitions: Vec<CobPartitionResult>,
    pub processed_loan_ids: Vec<i64>,
    pub failed_loan_ids: Vec<i64>,
    pub skipped_locked_loan_ids: Vec<i64>,
}

/// Status DTO matching `OldestCOBProcessedLoanDTO` from `LoanCOBCatchUpApiResource.java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OldestCobProcessedLoanDto {
    pub loan_ids: Vec<i64>,
    pub cob_processed_date: NaiveDate,
    pub cob_business_date: NaiveDate,
}

/// Status DTO matching `IsCatchUpRunningDTO` from `LoanCOBCatchUpApiResource.java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsCatchUpRunningDto {
    pub is_catch_up_running: bool,
    pub processing_date: Option<NaiveDate>,
}

/// Divides a [`LoanCobParameter`] range `[min_loan_id..=max_loan_id]` into [`CobPartition`] chunks
/// and processes them concurrently across `tokio` workers with per-loan fault isolation.
///
/// Ported from:
/// - `org.apache.fineract.cob.loan.LoanCOBPartitioner`
/// - `org.apache.fineract.cob.loan.LoanItemProcessor`
/// - `org.apache.fineract.cob.loan.LoanItemWriter`
/// - `org.apache.fineract.cob.listener.ChunkProcessingLoanItemListener`
/// - `org.apache.fineract.cob.service.LoanCOBCatchUpServiceImpl`
#[derive(Clone)]
pub struct PartitionedCobWorkerPool {
    lock_manager: Arc<LoanAccountLockManager>,
    loan_store: Arc<CobLoanStore>,
    steps: Vec<Arc<dyn CobBusinessStep>>,
    partition_size: usize,
    page_size: usize,
    max_concurrency: usize,
    tenant_id: String,
    is_catch_up_running: Arc<AtomicBool>,
}

impl fmt::Debug for PartitionedCobWorkerPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartitionedCobWorkerPool")
            .field("partition_size", &self.partition_size)
            .field("page_size", &self.page_size)
            .field("max_concurrency", &self.max_concurrency)
            .field("steps_count", &self.steps.len())
            .finish()
    }
}

impl Default for PartitionedCobWorkerPool {
    fn default() -> Self {
        Self::in_memory(100, 4)
    }
}

impl PartitionedCobWorkerPool {
    /// Creates a new [`PartitionedCobWorkerPool`].
    pub fn new(
        lock_manager: Arc<LoanAccountLockManager>,
        loan_store: Arc<CobLoanStore>,
        steps: Vec<Arc<dyn CobBusinessStep>>,
        partition_size: usize,
        page_size: usize,
        max_concurrency: usize,
    ) -> Self {
        Self {
            lock_manager,
            loan_store,
            steps,
            partition_size: partition_size.max(1),
            page_size: page_size.max(1),
            max_concurrency: max_concurrency.max(1),
            tenant_id: "default".to_string(),
            is_catch_up_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Creates an in-memory [`PartitionedCobWorkerPool`] with the default 7 Loan COB steps.
    pub fn in_memory(partition_size: usize, max_concurrency: usize) -> Self {
        let page_size = (partition_size / 2).max(1);
        Self::new(
            Arc::new(LoanAccountLockManager::in_memory()),
            Arc::new(CobLoanStore::in_memory()),
            default_loan_cob_business_steps(),
            partition_size,
            page_size,
            max_concurrency,
        )
    }

    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size.max(1);
        self
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = tenant_id.into();
        self
    }

    pub fn lock_manager(&self) -> &Arc<LoanAccountLockManager> {
        &self.lock_manager
    }

    pub fn loan_store(&self) -> &Arc<CobLoanStore> {
        &self.loan_store
    }

    pub fn steps(&self) -> &[Arc<dyn CobBusinessStep>] {
        &self.steps
    }

    /// Divides `param` into [`CobPartition`] chunks of `self.partition_size`.
    pub fn create_partitions(&self, param: LoanCobParameter) -> Vec<CobPartition> {
        CobPartition::partition_range(Some(param), self.partition_size, self.page_size)
    }

    /// Processes all eligible loans in `param` (`[min_loan_id..=max_loan_id]`) across concurrent
    /// `tokio` partition workers for `cob_business_date`.
    ///
    /// - Acquires soft locks per chunk and upgrades to hard locks per loan.
    /// - If a `CobBusinessStep` fails on a loan, records `error` and `stacktrace` on that loan's
    ///   [`LoanAccountLock`] in `m_loan_account_locks` **without aborting sibling loans in the chunk**.
    /// - Releases locks and sets `loan.last_closed_business_date = Some(cob_business_date)` for all
    ///   loans that succeed.
    pub async fn execute_range(
        &self,
        param: LoanCobParameter,
        cob_business_date: NaiveDate,
    ) -> Result<CobBatchExecutionSummary, FineractError> {
        let partitions = self.create_partitions(param);
        let semaphore = Arc::new(Semaphore::new(self.max_concurrency));
        let mut join_handles = Vec::with_capacity(partitions.len());

        for partition in partitions {
            let permit_sem = Arc::clone(&semaphore);
            let pool_clone = self.clone();
            let handle = tokio::spawn(async move {
                let _permit = permit_sem
                    .acquire_owned()
                    .await
                    .expect("worker pool semaphore should not be closed");
                pool_clone
                    .process_partition(partition, cob_business_date)
                    .await
            });
            join_handles.push(handle);
        }

        let mut partition_results = Vec::with_capacity(join_handles.len());
        let mut all_processed = Vec::new();
        let mut all_failed = Vec::new();
        let mut all_skipped = Vec::new();

        for handle in join_handles {
            let part_res = handle.await.map_err(|join_err| {
                FineractError::Internal(format!("COB partition worker task panicked: {join_err}"))
            })?;
            all_processed.extend(part_res.processed_loan_ids.iter().copied());
            all_failed.extend(part_res.failed_loan_ids.iter().copied());
            all_skipped.extend(part_res.skipped_locked_loan_ids.iter().copied());
            partition_results.push(part_res);
        }

        partition_results.sort_by_key(|r| r.partition.partition_index);
        all_processed.sort_unstable();
        all_failed.sort_unstable();
        all_skipped.sort_unstable();

        Ok(CobBatchExecutionSummary {
            cob_business_date,
            parameter: param,
            partitions: partition_results,
            processed_loan_ids: all_processed,
            failed_loan_ids: all_failed,
            skipped_locked_loan_ids: all_skipped,
        })
    }

    async fn process_partition(
        &self,
        partition: CobPartition,
        cob_business_date: NaiveDate,
    ) -> CobPartitionResult {
        if partition.is_empty() {
            return CobPartitionResult {
                partition,
                processed_loan_ids: Vec::new(),
                failed_loan_ids: Vec::new(),
                skipped_locked_loan_ids: Vec::new(),
            };
        }

        let loans = self.loan_store.find_loans_behind_in_range(
            partition.min_loan_id,
            partition.max_loan_id,
            cob_business_date,
        );

        let mut processed_loan_ids = Vec::new();
        let mut failed_loan_ids = Vec::new();
        let mut skipped_locked_loan_ids = Vec::new();

        let business_date = cob_business_date.succ_opt().unwrap_or(cob_business_date);
        let ctx = ExecutionContext::new(&self.tenant_id, business_date)
            .with_business_date(BusinessDateType::BusinessDate, business_date)
            .with_business_date(BusinessDateType::CobDate, cob_business_date)
            .with_action_context(ActionContext::Cob);

        with_execution_context(ctx, async {
            for chunk in loans.chunks(partition.page_size.max(1)) {
                // Phase 1: Apply soft locks for loans in this chunk
                for loan in chunk {
                    let _ = self
                        .lock_manager
                        .apply_soft_lock(loan.id, cob_business_date);
                }

                // Phase 2: Upgrade each loan to hard lock and run CobBusinessSteps with per-loan fault isolation
                for loan_snapshot in chunk {
                    let mut loan = loan_snapshot.clone();

                    // Upgrade to hard lock (skip if locked by Inline COB)
                    if self
                        .lock_manager
                        .upgrade_to_hard_lock(loan.id, cob_business_date)
                        .is_err()
                    {
                        skipped_locked_loan_ids.push(loan.id);
                        continue;
                    }

                    let mut loan_failed = false;
                    for step in &self.steps {
                        if let Err(err) = step.execute(&mut loan, cob_business_date).await {
                            let stack = format!(
                                "at org.apache.fineract.cob.loan.{} ({})\n  loan_id: {}\n  partition: {}\n  cob_business_date: {}\n  caused by: {}",
                                step.enum_styled_name(),
                                step.step_name(),
                                loan.id,
                                partition.partition_name(),
                                cob_business_date,
                                err
                            );
                            self.lock_manager.record_lock_error(
                                loan.id,
                                LockOwner::LoanCobChunkProcessing,
                                cob_business_date,
                                err.to_string(),
                                stack,
                            );
                            failed_loan_ids.push(loan.id);
                            loan_failed = true;
                            break;
                        }
                    }

                    if !loan_failed {
                        self.lock_manager
                            .release_lock_and_close_loan(&mut loan, cob_business_date);
                        self.loan_store.upsert_loan(loan.clone());
                        processed_loan_ids.push(loan.id);
                    }
                }
            }
        })
        .await;

        CobPartitionResult {
            partition,
            processed_loan_ids,
            failed_loan_ids,
            skipped_locked_loan_ids,
        }
    }

    /// Executes multi-day catch-up across all active loans behind `target_cob_date`, advancing
    /// day-by-day from the oldest unprocessed COB date through `target_cob_date`.
    ///
    /// Ported from `LoanCOBCatchUpServiceImpl.unlockHardLockedLoans` & `triggerAccountLockCatchUp`.
    pub async fn execute_catch_up(
        &self,
        target_cob_date: NaiveDate,
    ) -> Result<Vec<CobBatchExecutionSummary>, FineractError> {
        self.is_catch_up_running.store(true, Ordering::SeqCst);
        let mut summaries = Vec::new();

        let Some(oldest_closed) = self
            .loan_store
            .find_oldest_cob_processed_date(target_cob_date)
        else {
            self.is_catch_up_running.store(false, Ordering::SeqCst);
            return Ok(summaries);
        };

        let mut cursor_date = oldest_closed.succ_opt().unwrap_or(target_cob_date);
        while cursor_date <= target_cob_date {
            let param = self.loan_store.find_min_max_loan_ids_behind(cursor_date);
            if !param.is_empty() {
                let summary = self.execute_range(param, cursor_date).await?;
                summaries.push(summary);
            }
            let Some(next_date) = cursor_date.succ_opt() else {
                break;
            };
            cursor_date = next_date;
        }

        self.is_catch_up_running.store(false, Ordering::SeqCst);
        Ok(summaries)
    }

    /// Returns whether the catch-up engine is currently running (`IsCatchUpRunningDTO`).
    pub fn is_catch_up_running(&self, cob_date: Option<NaiveDate>) -> IsCatchUpRunningDto {
        let running = self.is_catch_up_running.load(Ordering::SeqCst);
        IsCatchUpRunningDto {
            is_catch_up_running: running,
            processing_date: if running { cob_date } else { None },
        }
    }
}

// =============================================================================
// 11. Comprehensive Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    /// Custom fault-injecting step used to verify that a step failure on a specific loan
    /// records `error` and `stacktrace` on `LoanAccountLock` without failing sibling loans in the chunk.
    struct FaultInjectingCobStep {
        fail_loan_id: i64,
    }

    #[async_trait]
    impl CobBusinessStep for FaultInjectingCobStep {
        fn step_name(&self) -> &'static str {
            "Fault Injecting Test Step"
        }

        fn enum_styled_name(&self) -> &'static str {
            "FAULT_INJECTING_STEP"
        }

        async fn execute(
            &self,
            loan: &mut LoanAggregate,
            cob_business_date: NaiveDate,
        ) -> Result<(), FineractError> {
            loan.executed_steps_log
                .push((cob_business_date, self.enum_styled_name().to_string()));
            if loan.id == self.fail_loan_id {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.cob.step.simulated.failure",
                    format!(
                        "Simulated COB step failure on loan {} at {}",
                        loan.id, cob_business_date
                    ),
                )));
            }
            Ok(())
        }
    }

    #[test]
    fn test_lock_owner_and_dialect_sql_for_update_skip_locked() {
        assert_eq!(
            LockOwner::LoanCobChunkProcessing.as_str(),
            "LOAN_COB_CHUNK_PROCESSING"
        );
        assert_eq!(
            LockOwner::LoanInlineCobProcessing.as_str(),
            "LOAN_INLINE_COB_PROCESSING"
        );
        assert_eq!(
            LockOwner::from_str("LOAN_COB_CHUNK_PROCESSING").unwrap(),
            LockOwner::LoanCobChunkProcessing
        );
        assert_eq!(
            LockOwner::from_str("LOAN_INLINE_COB_PROCESSING").unwrap(),
            LockOwner::LoanInlineCobProcessing
        );

        let pg_sql =
            LoanAccountLockManager::select_lock_for_update_skip_locked_sql(SqlDialect::Postgres);
        assert_eq!(
            pg_sql,
            "SELECT loan_id, version, lock_owner, lock_placed_on, error, stacktrace, lock_placed_on_cob_business_date FROM m_loan_account_locks WHERE loan_id = $1 FOR UPDATE SKIP LOCKED"
        );

        let mysql_sql =
            LoanAccountLockManager::select_lock_for_update_skip_locked_sql(SqlDialect::MySql);
        assert_eq!(
            mysql_sql,
            "SELECT loan_id, version, lock_owner, lock_placed_on, error, stacktrace, lock_placed_on_cob_business_date FROM m_loan_account_locks WHERE loan_id = ? FOR UPDATE SKIP LOCKED"
        );
    }

    #[tokio::test]
    async fn test_multi_day_inline_cob_catch_up_and_all_seven_built_in_steps() {
        let investor_repo = Arc::new(DualDbInvestorRepository::in_memory());
        let investor_engine = ExternalAssetOwnerTransferEngine::new(Arc::clone(&investor_repo));

        // Schedule a pending External Asset Owner Sale to settle on 2026-03-03
        investor_engine
            .initiate_sale(
                1001,
                Some("000001001".to_string()),
                ExternalId::new("INV-BLACKROCK-COB"),
                Some(ExternalId::new("TX-COB-1001")),
                dec!(1.01),
                ymd(2026, 3, 3),
                ymd(2026, 3, 1),
            )
            .expect("initiate sale should succeed");

        let steps: Vec<Arc<dyn CobBusinessStep>> = vec![
            Arc::new(ApplyChargeToOverdueLoansBusinessStep::new()),
            Arc::new(LoanDelinquencyClassificationBusinessStep::new()),
            Arc::new(LoanInterestRecalculationBusinessStep::new()),
            Arc::new(AccrualActivityPostingBusinessStep::new()),
            Arc::new(ExternalAssetOwnerTransferBusinessStep::new(investor_engine)),
            Arc::new(CheckLoanRepaymentDueBusinessStep::new(0)),
            Arc::new(CheckLoanRepaymentOverdueBusinessStep::new(1)),
        ];

        let lock_manager = Arc::new(LoanAccountLockManager::in_memory());
        let loan_store = Arc::new(CobLoanStore::in_memory());
        let inline_executor = InlineLoanCobExecutor::new(
            Arc::clone(&lock_manager),
            Arc::clone(&loan_store),
            steps,
        );

        // Loan #1001 was last closed on 2026-03-01, with:
        // - Installment #1 due on 2026-03-02 (becomes due on 2026-03-02, overdue on 2026-03-03 -> 2026-03-05)
        let mut loan = LoanAggregate::new(1001, Some(ymd(2026, 3, 1)))
            .with_interest_recalculation(true)
            .with_overdue_charge_spec(OverdueChargeSpec::new_flat_penalty(
                50,
                "Late Payment Penalty",
                dec!(15.00),
            ))
            .with_installment(CobRepaymentInstallment::new(
                1,
                ymd(2026, 2, 2),
                ymd(2026, 3, 2),
                dec!(500.00),
                dec!(25.00),
            ))
            .with_installment(CobRepaymentInstallment::new(
                2,
                ymd(2026, 3, 2),
                ymd(2026, 4, 2),
                dec!(500.00),
                dec!(25.00),
            ));
        loan.enable_installment_level_delinquency = true;
        loan_store.upsert_loan(loan);

        // Incoming write command arrives when current_cob_date is 2026-03-05 (4 days behind: Mar 2, 3, 4, 5)
        let target_cob_date = ymd(2026, 3, 5);
        let (report, command_result) = inline_executor
            .intercept_write_command(1001, target_cob_date, |updated_loan| async move {
                Ok(updated_loan.total_outstanding())
            })
            .await
            .expect("multi-day inline COB catch-up should succeed");

        assert_eq!(report.loan_id, 1001);
        assert_eq!(
            report.previous_closed_business_date,
            Some(ymd(2026, 3, 1))
        );
        assert_eq!(report.new_closed_business_date, Some(target_cob_date));
        assert_eq!(
            report.dates_processed,
            vec![
                ymd(2026, 3, 2),
                ymd(2026, 3, 3),
                ymd(2026, 3, 4),
                ymd(2026, 3, 5),
            ]
        );
        // 4 days * 7 steps = 28 step executions
        assert_eq!(report.steps_executed_count, 28);
        assert!(command_result > dec!(1050.00));

        // Verify lock was released after inline COB completed
        assert!(lock_manager.get_lock(1001).is_none());

        // Inspect updated loan state after the 4-day catch-up
        let updated = loan_store.get_loan(1001).unwrap();
        assert_eq!(updated.last_closed_business_date, Some(target_cob_date));
        // 1. Overdue penalty ($15.00) applied once to installment #1
        assert_eq!(updated.charges.len(), 1);
        assert_eq!(updated.charges[0].amount, dec!(15.00));
        // 2. Delinquency classification: on 2026-03-05, installment #1 (due 2026-03-02) is 3 days overdue -> PAR_1_30 (range 1)
        assert_eq!(updated.overdue_days, 3);
        assert_eq!(updated.delinquency_range_id, Some(1));
        assert_eq!(
            updated.delinquency_classification.as_deref(),
            Some("PAR_1_30")
        );
        assert_eq!(updated.installments[0].delinquency_range_id, Some(1));
        // 3. Interest recalculation ran and accrued overdue interest on installment #1
        assert_eq!(updated.interest_recalculated_on, Some(target_cob_date));
        assert!(updated.installments[0].recalculated_interest_component);
        assert!(updated.installments[0].interest_charged > dec!(25.00));
        // 4. Accrual activity transaction posted on installment #1's due date (2026-03-02)
        assert_eq!(updated.transactions.len(), 1);
        assert_eq!(
            updated.transactions[0].transaction_type_enum,
            CobLoanTransaction::TYPE_ACCRUAL_ACTIVITY
        );
        assert_eq!(updated.transactions[0].transaction_date, ymd(2026, 3, 2));
        // 5. External asset owner transfer settled on 2026-03-03
        assert_eq!(
            updated.external_owner_id.as_deref(),
            Some("INV-BLACKROCK-COB")
        );
        // 6 & 7. Repayment due (2026-03-02) and Repayment overdue (2026-03-03) events emitted
        assert!(updated
            .emitted_events
            .iter()
            .any(|e| e.event_type == "LoanRepaymentDueBusinessEvent"
                && e.business_date == ymd(2026, 3, 2)));
        assert!(updated
            .emitted_events
            .iter()
            .any(|e| e.event_type == "LoanRepaymentOverdueBusinessEvent"
                && e.business_date == ymd(2026, 3, 3)));
    }

    #[tokio::test]
    async fn test_inline_cob_lock_contention_returns_409_conflict_when_locked_by_chunk_worker() {
        let lock_manager = Arc::new(LoanAccountLockManager::in_memory());
        let loan_store = Arc::new(CobLoanStore::in_memory());
        let inline_executor = InlineLoanCobExecutor::new(
            Arc::clone(&lock_manager),
            Arc::clone(&loan_store),
            default_loan_cob_business_steps(),
        );

        let cob_date = ymd(2026, 4, 10);
        let loan = LoanAggregate::new(42, Some(ymd(2026, 4, 9)));
        loan_store.upsert_loan(loan.clone());

        // Simulate a batch chunk worker holding LoanCobChunkProcessing lock on loan #42
        lock_manager
            .acquire_chunk_lock(42, cob_date)
            .expect("chunk worker lock should succeed");

        // Incoming write command intercepted by InlineLoanCobExecutor must return 409 Conflict
        let err = inline_executor
            .intercept_write_command(42, cob_date, |_l| async move { Ok(()) })
            .await
            .expect_err("must fail with 409 Conflict when locked by chunk processing");

        assert_eq!(err.http_status_code(), 409);
        let global = err.to_global_error_response();
        assert_eq!(global.http_status_code, "409");
        assert_eq!(global.user_message_globalisation_code, "error.msg.loan.locked");
        assert!(global.default_user_message.contains("42"));
    }

    #[tokio::test]
    async fn test_step_failure_recorded_on_loan_account_lock_without_failing_sibling_loans_in_chunk()
    {
        let lock_manager = Arc::new(LoanAccountLockManager::in_memory());
        let loan_store = Arc::new(CobLoanStore::in_memory());
        let cob_date = ymd(2026, 5, 15);

        // Insert 5 loans (IDs 1..=5) in the same chunk; loan #3 will fail during FaultInjectingCobStep
        for loan_id in 1..=5 {
            loan_store.upsert_loan(LoanAggregate::new(loan_id, Some(ymd(2026, 5, 14))));
        }

        let steps: Vec<Arc<dyn CobBusinessStep>> = vec![
            Arc::new(ApplyChargeToOverdueLoansBusinessStep::new()),
            Arc::new(FaultInjectingCobStep { fail_loan_id: 3 }),
            Arc::new(LoanDelinquencyClassificationBusinessStep::new()),
        ];

        // Single partition & single chunk of size 10 covering [1..=5]
        let pool = PartitionedCobWorkerPool::new(
            Arc::clone(&lock_manager),
            Arc::clone(&loan_store),
            steps,
            10,
            10,
            2,
        );

        let summary = pool
            .execute_range(LoanCobParameter::new(1, 5), cob_date)
            .await
            .expect("chunk execution must not abort when a single loan fails");

        assert_eq!(summary.processed_loan_ids, vec![1, 2, 4, 5]);
        assert_eq!(summary.failed_loan_ids, vec![3]);
        assert!(summary.skipped_locked_loan_ids.is_empty());

        // Verify sibling loans (1, 2, 4, 5) had their locks released and last_closed_business_date updated
        for ok_id in [1, 2, 4, 5] {
            assert!(
                lock_manager.get_lock(ok_id).is_none(),
                "lock for successful loan {ok_id} must be released"
            );
            let closed_loan = loan_store.get_loan(ok_id).unwrap();
            assert_eq!(closed_loan.last_closed_business_date, Some(cob_date));
        }

        // Verify loan #3 kept its LoanAccountLock with error & stacktrace recorded, and its last_closed_business_date was NOT advanced
        let failed_lock = lock_manager
            .get_lock(3)
            .expect("failed loan #3 must retain its LoanAccountLock with error");
        assert_eq!(failed_lock.lock_owner, LockOwner::LoanCobChunkProcessing);
        assert!(failed_lock.has_error());
        assert!(failed_lock
            .error
            .as_deref()
            .unwrap()
            .contains("Simulated COB step failure on loan 3"));
        assert!(failed_lock
            .stacktrace
            .as_deref()
            .unwrap()
            .contains("FAULT_INJECTING_STEP"));
        assert_eq!(
            failed_lock.lock_placed_on_cob_business_date,
            Some(cob_date)
        );

        let failed_loan = loan_store.get_loan(3).unwrap();
        assert_eq!(
            failed_loan.last_closed_business_date,
            Some(ymd(2026, 5, 14))
        );
    }

    #[tokio::test]
    async fn test_partitioned_worker_pool_concurrent_execution_and_catch_up() {
        let lock_manager = Arc::new(LoanAccountLockManager::in_memory());
        let loan_store = Arc::new(CobLoanStore::in_memory());

        // Seed 25 active loans (IDs 101..=125) last closed on 2026-06-01
        for id in 101..=125 {
            let loan = LoanAggregate::new(id, Some(ymd(2026, 6, 1))).with_installment(
                CobRepaymentInstallment::new(
                    1,
                    ymd(2026, 5, 2),
                    ymd(2026, 6, 2),
                    dec!(250.00),
                    dec!(10.00),
                ),
            );
            loan_store.upsert_loan(loan);
        }

        // Partition size = 6 (creates 5 partitions: 101..=106, 107..=112, 113..=118, 119..=124, 125..=125),
        // page_size = 3, max_concurrency = 4
        let pool = PartitionedCobWorkerPool::new(
            Arc::clone(&lock_manager),
            Arc::clone(&loan_store),
            default_loan_cob_business_steps(),
            6,
            3,
            4,
        );

        let partitions = pool.create_partitions(LoanCobParameter::new(101, 125));
        assert_eq!(partitions.len(), 5);
        assert_eq!(partitions[0], CobPartition::new(1, 101, 106, 3));
        assert_eq!(partitions[1], CobPartition::new(2, 107, 112, 3));
        assert_eq!(partitions[2], CobPartition::new(3, 113, 118, 3));
        assert_eq!(partitions[3], CobPartition::new(4, 119, 124, 3));
        assert_eq!(partitions[4], CobPartition::new(5, 125, 125, 3));

        // Execute multi-day catch-up from 2026-06-02 through 2026-06-03 across the worker pool
        let summaries = pool
            .execute_catch_up(ymd(2026, 6, 3))
            .await
            .expect("partitioned catch-up should succeed");

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].cob_business_date, ymd(2026, 6, 2));
        assert_eq!(summaries[0].processed_loan_ids.len(), 25);
        assert_eq!(summaries[1].cob_business_date, ymd(2026, 6, 3));
        assert_eq!(summaries[1].processed_loan_ids.len(), 25);

        // Verify all 25 loans reached 2026-06-03, posted ACCRUAL_ACTIVITY on 2026-06-02,
        // were classified as PAR_1_30 on 2026-06-03, and have no lingering locks
        assert!(lock_manager.list_locks().is_empty());
        for id in 101..=125 {
            let loan = loan_store.get_loan(id).unwrap();
            assert_eq!(loan.last_closed_business_date, Some(ymd(2026, 6, 3)));
            assert_eq!(loan.overdue_days, 1);
            assert_eq!(loan.delinquency_range_id, Some(1));
            assert_eq!(loan.transactions.len(), 1);
            assert_eq!(
                loan.transactions[0].transaction_type_enum,
                CobLoanTransaction::TYPE_ACCRUAL_ACTIVITY
            );
        }
    }
}
