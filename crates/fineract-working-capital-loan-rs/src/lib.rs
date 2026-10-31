//! `fineract-working-capital-loan-rs` — Working Capital Loans, Revolving Credit Lines, Drawdowns,
//! Commitment Fee & Interest Accrual, Repayment Replenishment, and CQRS Command Handlers.
//!
//! Ported from `fineract-working-capital-loan` (`org.apache.fineract.portfolio.workingcapitalloan.*`
//! and `org.apache.fineract.portfolio.workingcapitalloanproduct.*`).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use dashmap::DashMap;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{AnyPool, Row};

use fineract_command_rs::pipeline::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::context::ExecutionContext;
use fineract_core_rs::db::{DialectSql, SqlDialect};
use fineract_core_rs::json_command::JsonCommand;
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{
    DataValidatorBuilder, FineractError, FineractResult, PlatformDomainRuleException,
};

// =============================================================================
// 1. ENUMS: WorkingCapitalLoanStatus & WorkingCapitalAmortizationType
// =============================================================================

/// Lifecycle status of a Working Capital Loan account (`m_wc_loan.loan_status_id`).
///
/// Mirrors `org.apache.fineract.portfolio.loanaccount.domain.LoanStatus` as used by
/// `org.apache.fineract.portfolio.workingcapitalloan.domain.WorkingCapitalLoan` and
/// `WorkingCapitalLoanLifecycleStateMachine`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[repr(i32)]
pub enum WorkingCapitalLoanStatus {
    #[default]
    Invalid = 0,
    SubmittedAndPendingApproval = 100,
    Approved = 200,
    Active = 300,
    WithdrawnByClient = 400,
    Rejected = 500,
    Closed = 600,
    WrittenOff = 601,
}

impl WorkingCapitalLoanStatus {
    pub const INVALID: Self = Self::Invalid;
    pub const SUBMITTED_AND_PENDING_APPROVAL: Self = Self::SubmittedAndPendingApproval;
    pub const APPROVED: Self = Self::Approved;
    pub const ACTIVE: Self = Self::Active;
    pub const WITHDRAWN_BY_CLIENT: Self = Self::WithdrawnByClient;
    pub const REJECTED: Self = Self::Rejected;
    pub const CLOSED: Self = Self::Closed;
    pub const WRITTEN_OFF: Self = Self::WrittenOff;

    /// Returns the integer status code stored in `m_wc_loan.loan_status_id`.
    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Alias for [`WorkingCapitalLoanStatus::value`].
    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    /// Returns the `SMALLINT` (`i16`) discriminant.
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Converts an `i32` status value into [`WorkingCapitalLoanStatus`].
    #[inline]
    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::SubmittedAndPendingApproval,
            200 => Self::Approved,
            300 => Self::Active,
            400 => Self::WithdrawnByClient,
            500 => Self::Rejected,
            600 => Self::Closed,
            601 => Self::WrittenOff,
            _ => Self::Invalid,
        }
    }

    /// Converts an `i16` status value into [`WorkingCapitalLoanStatus`].
    #[inline]
    pub const fn from_i16(value: i16) -> Self {
        Self::from_i32(value as i32)
    }

    /// Converts an optional `i32` status value into [`WorkingCapitalLoanStatus`].
    #[inline]
    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    /// Returns the Fineract globalisation message code.
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "loanStatusType.invalid",
            Self::SubmittedAndPendingApproval => "loanStatusType.submitted.and.pending.approval",
            Self::Approved => "loanStatusType.approved",
            Self::Active => "loanStatusType.active",
            Self::WithdrawnByClient => "loanStatusType.withdrawn.by.client",
            Self::Rejected => "loanStatusType.rejected",
            Self::Closed => "loanStatusType.closed.obligations.met",
            Self::WrittenOff => "loanStatusType.closed.written.off",
        }
    }

    /// Returns the human-readable description.
    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::SubmittedAndPendingApproval => "Submitted and pending approval",
            Self::Approved => "Approved",
            Self::Active => "Active",
            Self::WithdrawnByClient => "Withdrawn by applicant",
            Self::Rejected => "Rejected",
            Self::Closed => "Closed (obligations met)",
            Self::WrittenOff => "Closed (written off)",
        }
    }

    #[inline]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
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
    pub const fn is_withdrawn_by_client(self) -> bool {
        matches!(self, Self::WithdrawnByClient)
    }

    #[inline]
    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    #[inline]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }

    #[inline]
    pub const fn is_written_off(self) -> bool {
        matches!(self, Self::WrittenOff)
    }

    #[inline]
    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Rejected | Self::WithdrawnByClient)
    }
}

impl fmt::Display for WorkingCapitalLoanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Amortization strategy for a [`WorkingCapitalLoanProduct`] and its drawdowns.
///
/// Ported from `org.apache.fineract.portfolio.workingcapitalloanproduct.domain.WorkingCapitalAmortizationType`
/// and extended with working-capital drawdown schedule strategies.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[repr(i32)]
pub enum WorkingCapitalAmortizationType {
    #[default]
    EqualInstallments = 1,
    EqualPrincipal = 2,
    BulletAtMaturity = 3,
    RevenueSharePercentage = 4,
}

impl WorkingCapitalAmortizationType {
    /// Upstream Java `WorkingCapitalAmortizationType.EIR` alias.
    pub const EIR: Self = Self::EqualInstallments;
    /// Upstream Java `WorkingCapitalAmortizationType.FLAT` alias.
    pub const FLAT: Self = Self::EqualPrincipal;

    #[inline]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EqualInstallments => "workingCapitalAmortizationType.equalInstallments",
            Self::EqualPrincipal => "workingCapitalAmortizationType.equalPrincipal",
            Self::BulletAtMaturity => "workingCapitalAmortizationType.bulletAtMaturity",
            Self::RevenueSharePercentage => "workingCapitalAmortizationType.revenueSharePercentage",
        }
    }

    #[inline]
    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::EqualInstallments),
            2 => Some(Self::EqualPrincipal),
            3 => Some(Self::BulletAtMaturity),
            4 => Some(Self::RevenueSharePercentage),
            _ => None,
        }
    }

    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "EQUAL_INSTALLMENTS" | "EQUALINSTALLMENTS" | "EIR" | "1" => {
                Some(Self::EqualInstallments)
            }
            "EQUAL_PRINCIPAL" | "EQUALPRINCIPAL" | "FLAT" | "2" => Some(Self::EqualPrincipal),
            "BULLET_AT_MATURITY" | "BULLETATMATURITY" | "BULLET" | "3" => {
                Some(Self::BulletAtMaturity)
            }
            "REVENUE_SHARE_PERCENTAGE" | "REVENUESHAREPERCENTAGE" | "REVENUE_SHARE" | "TPV"
            | "4" => Some(Self::RevenueSharePercentage),
            _ => None,
        }
    }

    #[inline]
    pub const fn is_equal_installments(self) -> bool {
        matches!(self, Self::EqualInstallments)
    }

    #[inline]
    pub const fn is_equal_principal(self) -> bool {
        matches!(self, Self::EqualPrincipal)
    }

    #[inline]
    pub const fn is_bullet_at_maturity(self) -> bool {
        matches!(self, Self::BulletAtMaturity)
    }

    #[inline]
    pub const fn is_revenue_share_percentage(self) -> bool {
        matches!(self, Self::RevenueSharePercentage)
    }
}

impl FromStr for WorkingCapitalAmortizationType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or_else(|| {
            FineractError::DomainRule(PlatformDomainRuleException::new(
                "validation.msg.wc.product.amortizationType.invalid",
                format!("Unsupported working capital amortization type `{s}`"),
            ))
        })
    }
}

impl fmt::Display for WorkingCapitalAmortizationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

// =============================================================================
// 2. DOMAIN ENTITIES: Product, Drawdown, and WorkingCapitalLoanAccount
// =============================================================================

/// Working Capital Loan Product configuration (`m_wc_loan_product`).
///
/// Ported from `org.apache.fineract.portfolio.workingcapitalloanproduct.domain.WorkingCapitalLoanProduct`,
/// `WorkingCapitalLoanProductRelatedDetail`, and `WorkingCapitalLoanProductMinMaxConstraints`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanProduct {
    pub id: i64,
    pub short_name: String,
    pub name: String,
    pub currency_code: String,
    pub credit_limit_min: Decimal,
    pub credit_limit_default: Decimal,
    pub credit_limit_max: Decimal,
    pub nominal_interest_rate: Decimal,
    pub commitment_fee_rate: Option<Decimal>,
    pub revolving_line: bool,
    pub max_drawdowns: Option<u32>,
    pub amortization_type: WorkingCapitalAmortizationType,
}

impl WorkingCapitalLoanProduct {
    /// Creates a new [`WorkingCapitalLoanProduct`] and validates its constraints.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        short_name: impl Into<String>,
        name: impl Into<String>,
        currency_code: impl Into<String>,
        credit_limit_min: Decimal,
        credit_limit_default: Decimal,
        credit_limit_max: Decimal,
        nominal_interest_rate: Decimal,
        commitment_fee_rate: Option<Decimal>,
        revolving_line: bool,
        max_drawdowns: Option<u32>,
        amortization_type: WorkingCapitalAmortizationType,
    ) -> FineractResult<Self> {
        let product = Self {
            id,
            short_name: short_name.into().trim().to_string(),
            name: name.into().trim().to_string(),
            currency_code: currency_code.into().trim().to_ascii_uppercase(),
            credit_limit_min,
            credit_limit_default,
            credit_limit_max,
            nominal_interest_rate,
            commitment_fee_rate,
            revolving_line,
            max_drawdowns,
            amortization_type,
        };
        product.validate()?;
        Ok(product)
    }

    /// Returns a 2-decimal [`MonetaryCurrency`] for `self.currency_code`.
    #[inline]
    pub fn monetary_currency(&self) -> MonetaryCurrency {
        MonetaryCurrency::with_code(self.currency_code.clone(), 2)
    }

    /// Validates product configuration invariants (`min <= default <= max`, non-negative rates, etc.).
    pub fn validate(&self) -> FineractResult<()> {
        let validator = DataValidatorBuilder::for_resource("workingcapitalloanproduct");

        validator
            .reset()
            .parameter("name")
            .value(Some(self.name.as_str()))
            .not_blank()
            .not_exceeding_length_of(100);

        validator
            .reset()
            .parameter("shortName")
            .value(Some(self.short_name.as_str()))
            .not_blank()
            .not_exceeding_length_of(4);

        validator
            .reset()
            .parameter("currencyCode")
            .value(Some(self.currency_code.as_str()))
            .not_blank()
            .not_exceeding_length_of(3);

        validator
            .reset()
            .parameter("creditLimitMin")
            .value(self.credit_limit_min)
            .greater_than_zero();

        validator
            .reset()
            .parameter("creditLimitDefault")
            .value(self.credit_limit_default)
            .greater_than_zero()
            .in_min_and_max_amount_range(self.credit_limit_min, self.credit_limit_max);

        validator
            .reset()
            .parameter("creditLimitMax")
            .value(self.credit_limit_max)
            .greater_than_zero();

        validator
            .reset()
            .parameter("nominalInterestRate")
            .value(self.nominal_interest_rate)
            .zero_or_positive();

        if let Some(fee_rate) = self.commitment_fee_rate {
            validator
                .reset()
                .parameter("commitmentFeeRate")
                .value(fee_rate)
                .zero_or_positive();
        }

        if let Some(0) = self.max_drawdowns {
            validator.reset().parameter("maxDrawdowns").fail_with_code(
                "validation.msg.workingcapitalloanproduct.maxDrawdowns.must.be.greater.than.zero",
                "`maxDrawdowns` must be greater than zero when specified",
            );
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates that a requested or approved credit limit lies within `[credit_limit_min, credit_limit_max]`.
    pub fn validate_credit_limit(&self, credit_limit: Decimal) -> FineractResult<()> {
        if credit_limit < self.credit_limit_min || credit_limit > self.credit_limit_max {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.credit.limit.out.of.bounds",
                format!(
                    "Sanctioned credit limit {credit_limit} must be between product minimum {} and maximum {}",
                    self.credit_limit_min, self.credit_limit_max
                ),
            )));
        }
        Ok(())
    }

    /// Parses and validates a [`WorkingCapitalLoanProduct`] from a [`JsonCommand`].
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let short_name = command
            .string_value_of_parameter_named("shortName")
            .or_else(|| command.string_value_of_parameter_named("short_name"))
            .unwrap_or_default();
        let currency_code = command
            .string_value_of_parameter_named("currencyCode")
            .or_else(|| command.string_value_of_parameter_named("currency_code"))
            .unwrap_or_else(|| "USD".to_string());

        let credit_limit_default = command
            .big_decimal_value_of_parameter_named("creditLimitDefault")?
            .or(command.big_decimal_value_of_parameter_named("principal")?)
            .or(command.big_decimal_value_of_parameter_named("defaultPrincipal")?)
            .unwrap_or(Decimal::ZERO);

        let credit_limit_min = command
            .big_decimal_value_of_parameter_named("creditLimitMin")?
            .or(command.big_decimal_value_of_parameter_named("minPrincipal")?)
            .or(command.big_decimal_value_of_parameter_named("principalMin")?)
            .unwrap_or(credit_limit_default);

        let credit_limit_max = command
            .big_decimal_value_of_parameter_named("creditLimitMax")?
            .or(command.big_decimal_value_of_parameter_named("maxPrincipal")?)
            .or(command.big_decimal_value_of_parameter_named("principalMax")?)
            .unwrap_or(credit_limit_default);

        let nominal_interest_rate = command
            .big_decimal_value_of_parameter_named("nominalInterestRate")?
            .or(command.big_decimal_value_of_parameter_named("annualEir")?)
            .or(command.big_decimal_value_of_parameter_named("interestRatePerPeriod")?)
            .unwrap_or(Decimal::ZERO);

        let commitment_fee_rate =
            command.big_decimal_value_of_parameter_named("commitmentFeeRate")?;

        let revolving_line = command
            .boolean_object_value_of_parameter_named("revolvingLine")
            .unwrap_or(true);

        let max_drawdowns = command
            .integer_value_sans_locale_of_parameter_named("maxDrawdowns")?
            .map(|v| u32::try_from(v).unwrap_or(0));

        let amortization_type = if let Some(raw_amort) =
            command.string_value_of_parameter_named("amortizationType")
        {
            WorkingCapitalAmortizationType::from_str(&raw_amort)?
        } else if let Some(code) =
            command.integer_value_sans_locale_of_parameter_named("amortizationType")?
        {
            WorkingCapitalAmortizationType::from_i32(code).ok_or_else(|| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "validation.msg.wc.product.amortizationType.invalid",
                    format!("Unsupported working capital amortization type code `{code}`"),
                ))
            })?
        } else {
            WorkingCapitalAmortizationType::EqualInstallments
        };

        Self::new(
            id,
            short_name,
            name,
            currency_code,
            credit_limit_min,
            credit_limit_default,
            credit_limit_max,
            nominal_interest_rate,
            commitment_fee_rate,
            revolving_line,
            max_drawdowns,
            amortization_type,
        )
    }
}

/// Individual drawdown tranche under a [`WorkingCapitalLoanAccount`] (`m_wc_loan_disbursement_detail`).
///
/// Ported from `org.apache.fineract.portfolio.workingcapitalloan.domain.WorkingCapitalLoanDisbursementDetails`
/// and `WorkingCapitalLoanBalance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalDrawdown {
    pub id: i64,
    pub wc_loan_id: i64,
    pub drawdown_date: NaiveDate,
    pub maturity_date: NaiveDate,
    pub principal_amount: Money,
    pub outstanding_principal: Money,
    pub accrued_interest: Money,
    pub fee_amount: Money,
}

impl WorkingCapitalDrawdown {
    /// Creates a new [`WorkingCapitalDrawdown`] with `outstanding_principal = principal_amount`
    /// and zero `accrued_interest` and `fee_amount`.
    pub fn new(
        id: i64,
        wc_loan_id: i64,
        drawdown_date: NaiveDate,
        maturity_date: NaiveDate,
        principal_amount: Money,
    ) -> FineractResult<Self> {
        if !principal_amount.is_greater_than_zero() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.drawdown.amount.must.be.positive",
                "Drawdown principal amount must be greater than zero",
            )));
        }
        if maturity_date < drawdown_date {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.drawdown.maturity.before.drawdown.date",
                format!(
                    "Drawdown maturity date {maturity_date} cannot be before drawdown date {drawdown_date}"
                ),
            )));
        }
        let zero = principal_amount.zero_of();
        Ok(Self {
            id,
            wc_loan_id,
            drawdown_date,
            maturity_date,
            outstanding_principal: principal_amount.clone(),
            principal_amount,
            accrued_interest: zero.clone(),
            fee_amount: zero,
        })
    }

    /// Returns total outstanding obligations (`outstanding_principal + accrued_interest + fee_amount`) on this drawdown.
    pub fn total_outstanding(&self) -> FineractResult<Money> {
        let sum = self
            .outstanding_principal
            .plus(&self.accrued_interest)
            .and_then(|m| m.plus(&self.fee_amount))
            .map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.currency.mismatch",
                    e.to_string(),
                ))
            })?;
        Ok(sum)
    }

    /// Returns `true` if principal, accrued interest, and fees on this drawdown have all been fully paid.
    #[inline]
    pub fn is_fully_repaid(&self) -> bool {
        self.outstanding_principal.is_zero()
            && self.accrued_interest.is_zero()
            && self.fee_amount.is_zero()
    }
}

/// Working Capital Loan Account aggregate (`m_wc_loan`).
///
/// Ported from `org.apache.fineract.portfolio.workingcapitalloan.domain.WorkingCapitalLoan`,
/// `WorkingCapitalLoanBalance`, and `WorkingCapitalLoanLifecycleStateMachine`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanAccount {
    pub id: i64,
    pub account_no: String,
    pub client_id: i64,
    pub product_id: i64,
    pub status: WorkingCapitalLoanStatus,
    pub sanctioned_credit_limit: Money,
    pub utilized_credit_limit: Money,
    pub available_credit_limit: Money,
    pub drawdowns: Vec<WorkingCapitalDrawdown>,
}

impl WorkingCapitalLoanAccount {
    /// Creates a new [`WorkingCapitalLoanAccount`] in [`WorkingCapitalLoanStatus::SubmittedAndPendingApproval`]
    /// state with zero utilized limit and `available_credit_limit = sanctioned_credit_limit`.
    pub fn new(
        id: i64,
        account_no: impl Into<String>,
        client_id: i64,
        product_id: i64,
        sanctioned_credit_limit: Money,
    ) -> Self {
        let zero = sanctioned_credit_limit.zero_of();
        Self {
            id,
            account_no: account_no.into(),
            client_id,
            product_id,
            status: WorkingCapitalLoanStatus::SubmittedAndPendingApproval,
            available_credit_limit: sanctioned_credit_limit.clone(),
            utilized_credit_limit: zero,
            sanctioned_credit_limit,
            drawdowns: Vec::new(),
        }
    }

    /// Transitions the loan from [`WorkingCapitalLoanStatus::SubmittedAndPendingApproval`] to
    /// [`WorkingCapitalLoanStatus::Approved`] and synchronizes `available_credit_limit`.
    pub fn approve(&mut self) -> FineractResult<()> {
        if self.status != WorkingCapitalLoanStatus::SubmittedAndPendingApproval {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "validation.msg.wc.loan.transition.not.allowed",
                format!(
                    "Approval is not allowed from current status {:?}",
                    self.status
                ),
            )));
        }
        self.status = WorkingCapitalLoanStatus::Approved;
        self.available_credit_limit = self
            .sanctioned_credit_limit
            .minus(&self.utilized_credit_limit)
            .map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.currency.mismatch",
                    e.to_string(),
                ))
            })?;
        Ok(())
    }

    /// Transitions the loan from [`WorkingCapitalLoanStatus::Approved`] to
    /// [`WorkingCapitalLoanStatus::Active`].
    pub fn activate(&mut self) -> FineractResult<()> {
        if self.status != WorkingCapitalLoanStatus::Approved {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "validation.msg.wc.loan.transition.not.allowed",
                format!(
                    "Activation is not allowed from current status {:?}",
                    self.status
                ),
            )));
        }
        self.status = WorkingCapitalLoanStatus::Active;
        self.available_credit_limit = self
            .sanctioned_credit_limit
            .minus(&self.utilized_credit_limit)
            .map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.currency.mismatch",
                    e.to_string(),
                ))
            })?;
        Ok(())
    }

    /// Executes a drawdown against the working capital facility:
    /// - Requires the account to be `Active` (or `Approved`, which automatically transitions to `Active`).
    /// - Enforces `amount > 0` and `amount <= available_credit_limit`.
    /// - Enforces `drawdowns.len() < max_drawdowns` when `product.max_drawdowns` is `Some(max)`.
    /// - Increases `utilized_credit_limit` by `amount` and decreases `available_credit_limit` by `amount`.
    /// - Appends and returns the created [`WorkingCapitalDrawdown`].
    pub fn execute_drawdown(
        &mut self,
        amount: Money,
        drawdown_date: NaiveDate,
        maturity_date: NaiveDate,
        product: &WorkingCapitalLoanProduct,
    ) -> FineractResult<WorkingCapitalDrawdown> {
        if self.status == WorkingCapitalLoanStatus::Approved {
            self.activate()?;
        }
        if self.status != WorkingCapitalLoanStatus::Active {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "validation.msg.wc.loan.transition.not.allowed",
                format!(
                    "Drawdown is not allowed when loan status is {:?}",
                    self.status
                ),
            )));
        }

        if !self.sanctioned_credit_limit.is_same_currency(&amount) {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.currency.mismatch",
                format!(
                    "Drawdown currency {} does not match loan currency {}",
                    amount.currency_code(),
                    self.sanctioned_credit_limit.currency_code()
                ),
            )));
        }

        if !amount.is_greater_than_zero() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.drawdown.amount.must.be.positive",
                "Drawdown amount must be greater than zero",
            )));
        }

        if let Some(max_drawdowns) = product.max_drawdowns {
            if self.drawdowns.len() >= max_drawdowns as usize {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.max.drawdowns.exceeded",
                    format!(
                        "Cannot execute drawdown: maximum allowed drawdowns ({max_drawdowns}) already reached"
                    ),
                )));
            }
        }

        if amount.is_greater_than(&self.available_credit_limit) {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.drawdown.exceeds.available.credit.limit",
                format!(
                    "Requested drawdown amount {} exceeds available credit limit {}",
                    amount.amount(),
                    self.available_credit_limit.amount()
                ),
            )));
        }

        let drawdown_id = (self.drawdowns.len() as i64) + 1;
        let drawdown = WorkingCapitalDrawdown::new(
            drawdown_id,
            self.id,
            drawdown_date,
            maturity_date,
            amount.clone(),
        )?;

        self.utilized_credit_limit =
            self.utilized_credit_limit.plus(&amount).map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.arithmetic.error",
                    e.to_string(),
                ))
            })?;
        self.available_credit_limit =
            self.available_credit_limit.minus(&amount).map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.arithmetic.error",
                    e.to_string(),
                ))
            })?;

        self.drawdowns.push(drawdown.clone());
        Ok(drawdown)
    }

    /// Processes a repayment across active drawdowns in order `Fees -> Accrued Interest -> Principal`:
    /// - Reduces `utilized_credit_limit` by the principal portion repaid.
    /// - When `product.revolving_line == true`, replenishes `available_credit_limit` by the principal
    ///   portion repaid (up to `sanctioned_credit_limit - utilized_credit_limit`).
    /// - When `product.revolving_line == false`, `available_credit_limit` is not replenished.
    /// - Returns the principal portion repaid as [`Money`].
    pub fn make_repayment(
        &mut self,
        amount: Money,
        product: &WorkingCapitalLoanProduct,
    ) -> FineractResult<Money> {
        if self.status != WorkingCapitalLoanStatus::Active {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "validation.msg.wc.loan.repayment.not.allowed",
                format!(
                    "Repayment is only allowed on ACTIVE working capital loans, found {:?}",
                    self.status
                ),
            )));
        }

        if !self.sanctioned_credit_limit.is_same_currency(&amount) {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.currency.mismatch",
                format!(
                    "Repayment currency {} does not match loan currency {}",
                    amount.currency_code(),
                    self.sanctioned_credit_limit.currency_code()
                ),
            )));
        }

        if !amount.is_greater_than_zero() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.repayment.amount.must.be.positive",
                "Repayment amount must be greater than zero",
            )));
        }

        let mut remaining = amount;
        let mut principal_repaid = self.sanctioned_credit_limit.zero_of();

        // 1. Allocate to Fees first (chronological across drawdowns)
        for drawdown in &mut self.drawdowns {
            if remaining.is_zero() {
                break;
            }
            if drawdown.fee_amount.is_greater_than_zero() {
                let pay_fee = if remaining.is_greater_than_or_equal_to(&drawdown.fee_amount) {
                    drawdown.fee_amount.clone()
                } else {
                    remaining.clone()
                };
                drawdown.fee_amount = drawdown.fee_amount.minus(&pay_fee).map_err(|e| {
                    FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.loan.arithmetic.error",
                        e.to_string(),
                    ))
                })?;
                remaining = remaining.minus(&pay_fee).map_err(|e| {
                    FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.loan.arithmetic.error",
                        e.to_string(),
                    ))
                })?;
            }
        }

        // 2. Allocate to Accrued Interest next
        for drawdown in &mut self.drawdowns {
            if remaining.is_zero() {
                break;
            }
            if drawdown.accrued_interest.is_greater_than_zero() {
                let pay_interest =
                    if remaining.is_greater_than_or_equal_to(&drawdown.accrued_interest) {
                        drawdown.accrued_interest.clone()
                    } else {
                        remaining.clone()
                    };
                drawdown.accrued_interest =
                    drawdown.accrued_interest.minus(&pay_interest).map_err(|e| {
                        FineractError::DomainRule(PlatformDomainRuleException::new(
                            "error.msg.wc.loan.arithmetic.error",
                            e.to_string(),
                        ))
                    })?;
                remaining = remaining.minus(&pay_interest).map_err(|e| {
                    FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.loan.arithmetic.error",
                        e.to_string(),
                    ))
                })?;
            }
        }

        // 3. Allocate to Outstanding Principal
        for drawdown in &mut self.drawdowns {
            if remaining.is_zero() {
                break;
            }
            if drawdown.outstanding_principal.is_greater_than_zero() {
                let pay_principal =
                    if remaining.is_greater_than_or_equal_to(&drawdown.outstanding_principal) {
                        drawdown.outstanding_principal.clone()
                    } else {
                        remaining.clone()
                    };
                drawdown.outstanding_principal = drawdown
                    .outstanding_principal
                    .minus(&pay_principal)
                    .map_err(|e| {
                        FineractError::DomainRule(PlatformDomainRuleException::new(
                            "error.msg.wc.loan.arithmetic.error",
                            e.to_string(),
                        ))
                    })?;
                remaining = remaining.minus(&pay_principal).map_err(|e| {
                    FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.loan.arithmetic.error",
                        e.to_string(),
                    ))
                })?;
                principal_repaid = principal_repaid.plus(&pay_principal).map_err(|e| {
                    FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.loan.arithmetic.error",
                        e.to_string(),
                    ))
                })?;
            }
        }

        // Update utilized credit limit
        self.utilized_credit_limit = self
            .utilized_credit_limit
            .minus_to_zero(&principal_repaid)
            .map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.arithmetic.error",
                    e.to_string(),
                ))
            })?;

        // Replenish available credit limit only when product.revolving_line == true
        if product.revolving_line {
            let replenished = self
                .available_credit_limit
                .plus(&principal_repaid)
                .map_err(|e| {
                    FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.loan.arithmetic.error",
                        e.to_string(),
                    ))
                })?;
            let max_available = self
                .sanctioned_credit_limit
                .minus(&self.utilized_credit_limit)
                .map_err(|e| {
                    FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.loan.arithmetic.error",
                        e.to_string(),
                    ))
                })?;
            self.available_credit_limit = if replenished.is_greater_than(&max_available) {
                max_available
            } else {
                replenished
            };
        }

        Ok(principal_repaid)
    }

    /// Accrues one day of interest on `utilized_credit_limit` (using `product.nominal_interest_rate`)
    /// and one day of commitment fee on `available_credit_limit` (using `product.commitment_fee_rate`)
    /// under an Actual/365 Fixed day-count convention.
    ///
    /// Returns `(daily_interest, daily_commitment_fee)`.
    pub fn accrue_daily_interest_and_commitment_fee(
        &mut self,
        product: &WorkingCapitalLoanProduct,
    ) -> FineractResult<(Money, Money)> {
        if self.status != WorkingCapitalLoanStatus::Active {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "validation.msg.wc.loan.accrual.not.allowed",
                format!(
                    "Daily accrual is only allowed on ACTIVE working capital loans, found {:?}",
                    self.status
                ),
            )));
        }

        let currency = self.sanctioned_credit_limit.currency().clone();
        let days_in_year_pct = dec!(36500);

        let daily_interest_raw = if self.utilized_credit_limit.is_greater_than_zero()
            && product.nominal_interest_rate > Decimal::ZERO
        {
            (self.utilized_credit_limit.amount() * product.nominal_interest_rate) / days_in_year_pct
        } else {
            Decimal::ZERO
        };
        let daily_interest = Money::of(
            currency.clone(),
            daily_interest_raw,
            FineractRoundingMode::HalfEven,
        );

        let daily_commitment_fee_raw = match product.commitment_fee_rate {
            Some(rate)
                if rate > Decimal::ZERO && self.available_credit_limit.is_greater_than_zero() =>
            {
                (self.available_credit_limit.amount() * rate) / days_in_year_pct
            }
            _ => Decimal::ZERO,
        };
        let daily_commitment_fee = Money::of(
            currency,
            daily_commitment_fee_raw,
            FineractRoundingMode::HalfEven,
        );

        // Attach accrued interest and commitment fee to active drawdown(s) if present
        if let Some(target_idx) = self
            .drawdowns
            .iter()
            .position(|d| d.outstanding_principal.is_greater_than_zero())
            .or_else(|| {
                if self.drawdowns.is_empty() {
                    None
                } else {
                    Some(self.drawdowns.len() - 1)
                }
            })
        {
            if daily_interest.is_greater_than_zero() {
                self.drawdowns[target_idx].accrued_interest = self.drawdowns[target_idx]
                    .accrued_interest
                    .plus(&daily_interest)
                    .map_err(|e| {
                        FineractError::DomainRule(PlatformDomainRuleException::new(
                            "error.msg.wc.loan.arithmetic.error",
                            e.to_string(),
                        ))
                    })?;
            }
            if daily_commitment_fee.is_greater_than_zero() {
                self.drawdowns[target_idx].fee_amount = self.drawdowns[target_idx]
                    .fee_amount
                    .plus(&daily_commitment_fee)
                    .map_err(|e| {
                        FineractError::DomainRule(PlatformDomainRuleException::new(
                            "error.msg.wc.loan.arithmetic.error",
                            e.to_string(),
                        ))
                    })?;
            }
        }

        Ok((daily_interest, daily_commitment_fee))
    }

    /// Closes the working capital loan account once all utilized principal, accrued interest, and fees
    /// have been settled (`status -> WorkingCapitalLoanStatus::Closed`).
    pub fn close(&mut self) -> FineractResult<()> {
        if self.status != WorkingCapitalLoanStatus::Active
            && self.status != WorkingCapitalLoanStatus::Approved
        {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "validation.msg.wc.loan.transition.not.allowed",
                format!(
                    "Cannot close working capital loan from status {:?}",
                    self.status
                ),
            )));
        }

        if self.utilized_credit_limit.is_greater_than_zero() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.wc.loan.cannot.close.with.outstanding.balance",
                format!(
                    "Cannot close working capital loan {}: utilized credit limit {} is still outstanding",
                    self.id,
                    self.utilized_credit_limit.amount()
                ),
            )));
        }

        for drawdown in &self.drawdowns {
            if !drawdown.is_fully_repaid() {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.cannot.close.with.outstanding.balance",
                    format!(
                        "Cannot close working capital loan {}: drawdown {} still has unpaid obligations",
                        self.id, drawdown.id
                    ),
                )));
            }
        }

        self.status = WorkingCapitalLoanStatus::Closed;
        self.available_credit_limit = self.sanctioned_credit_limit.zero_of();
        Ok(())
    }

    /// Sums `accrued_interest` across all drawdowns.
    pub fn total_accrued_interest(&self) -> FineractResult<Money> {
        let mut total = self.sanctioned_credit_limit.zero_of();
        for d in &self.drawdowns {
            total = total.plus(&d.accrued_interest).map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.arithmetic.error",
                    e.to_string(),
                ))
            })?;
        }
        Ok(total)
    }

    /// Sums `fee_amount` across all drawdowns.
    pub fn total_fee_amount(&self) -> FineractResult<Money> {
        let mut total = self.sanctioned_credit_limit.zero_of();
        for d in &self.drawdowns {
            total = total.plus(&d.fee_amount).map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.arithmetic.error",
                    e.to_string(),
                ))
            })?;
        }
        Ok(total)
    }

    /// Sums total outstanding (`outstanding_principal + accrued_interest + fee_amount`) across all drawdowns.
    pub fn total_outstanding(&self) -> FineractResult<Money> {
        let mut total = self.sanctioned_credit_limit.zero_of();
        for d in &self.drawdowns {
            let d_out = d.total_outstanding()?;
            total = total.plus(&d_out).map_err(|e| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.wc.loan.arithmetic.error",
                    e.to_string(),
                ))
            })?;
        }
        Ok(total)
    }
}

// =============================================================================
// 3. DUAL-DB REPOSITORY (`DualDbWorkingCapitalLoanRepository`)
// =============================================================================

/// Dual-database (PostgreSQL + MySQL/MariaDB via `sqlx::AnyPool` + concurrent `DashMap` in-memory store)
/// repository for [`WorkingCapitalLoanProduct`], [`WorkingCapitalLoanAccount`], and [`WorkingCapitalDrawdown`].
#[derive(Debug, Clone)]
pub struct DualDbWorkingCapitalLoanRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    products: Arc<DashMap<i64, WorkingCapitalLoanProduct>>,
    loans: Arc<DashMap<i64, WorkingCapitalLoanAccount>>,
    next_product_id: Arc<AtomicI64>,
    next_loan_id: Arc<AtomicI64>,
    next_drawdown_id: Arc<AtomicI64>,
}

impl Default for DualDbWorkingCapitalLoanRepository {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl DualDbWorkingCapitalLoanRepository {
    pub const PRODUCT_COLUMNS: &'static str = "id, short_name, name, currency_code, \
        credit_limit_min, credit_limit_default, credit_limit_max, nominal_interest_rate, \
        commitment_fee_rate, revolving_line, max_drawdowns, amortization_type";

    pub const LOAN_COLUMNS: &'static str = "id, account_no, client_id, product_id, \
        loan_status_id, currency_code, sanctioned_credit_limit, utilized_credit_limit, \
        available_credit_limit";

    pub const DRAWDOWN_COLUMNS: &'static str = "id, wc_loan_id, drawdown_date, maturity_date, \
        currency_code, principal_amount, outstanding_principal, accrued_interest, fee_amount";

    /// Creates a new [`DualDbWorkingCapitalLoanRepository`] backed by a SQLx [`AnyPool`] and [`SqlDialect`].
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            products: Arc::new(DashMap::new()),
            loans: Arc::new(DashMap::new()),
            next_product_id: Arc::new(AtomicI64::new(1)),
            next_loan_id: Arc::new(AtomicI64::new(1)),
            next_drawdown_id: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Creates an in-memory [`DualDbWorkingCapitalLoanRepository`].
    pub fn in_memory() -> Self {
        Self::new_in_memory()
    }

    /// Creates an in-memory [`DualDbWorkingCapitalLoanRepository`].
    pub fn new_in_memory() -> Self {
        Self {
            pool: None,
            dialect: SqlDialect::Postgres,
            products: Arc::new(DashMap::new()),
            loans: Arc::new(DashMap::new()),
            next_product_id: Arc::new(AtomicI64::new(1)),
            next_loan_id: Arc::new(AtomicI64::new(1)),
            next_drawdown_id: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Creates an in-memory [`DualDbWorkingCapitalLoanRepository`] configured for a specific [`SqlDialect`].
    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            ..Self::new_in_memory()
        }
    }

    #[inline]
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    #[inline]
    pub fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    pub fn allocate_product_id(&self) -> i64 {
        self.next_product_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn allocate_loan_id(&self) -> i64 {
        self.next_loan_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn allocate_drawdown_id(&self) -> i64 {
        self.next_drawdown_id.fetch_add(1, Ordering::SeqCst)
    }

    // --- Dialect-aware SQL statements ---

    pub fn select_product_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_wc_loan_product WHERE id = ?",
                Self::PRODUCT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_product_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_wc_loan_product ({}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                Self::PRODUCT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_loan_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!("SELECT {} FROM m_wc_loan WHERE id = ?", Self::LOAN_COLUMNS),
            dialect,
        )
    }

    pub fn insert_loan_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_wc_loan ({}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                Self::LOAN_COLUMNS
            ),
            dialect,
        )
    }

    pub fn update_loan_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_wc_loan SET account_no = ?, client_id = ?, product_id = ?, \
             loan_status_id = ?, currency_code = ?, sanctioned_credit_limit = ?, \
             utilized_credit_limit = ?, available_credit_limit = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn select_drawdowns_by_loan_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_wc_loan_disbursement_detail WHERE wc_loan_id = ? ORDER BY id ASC",
                Self::DRAWDOWN_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_drawdown_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_wc_loan_disbursement_detail ({}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                Self::DRAWDOWN_COLUMNS
            ),
            dialect,
        )
    }

    // --- Product CRUD ---

    /// Saves or updates a [`WorkingCapitalLoanProduct`] after checking unique `name` and `short_name`.
    pub async fn save_product(
        &self,
        mut product: WorkingCapitalLoanProduct,
    ) -> FineractResult<WorkingCapitalLoanProduct> {
        product.validate()?;
        if product.id <= 0 {
            product.id = self.allocate_product_id();
        } else {
            self.next_product_id
                .fetch_max(product.id + 1, Ordering::SeqCst);
        }

        for entry in self.products.iter() {
            let existing = entry.value();
            if existing.id != product.id {
                if existing.name.eq_ignore_ascii_case(&product.name) {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.product.duplicate.name",
                        format!(
                            "Working capital loan product with name `{}` already exists",
                            product.name
                        ),
                    )));
                }
                if existing
                    .short_name
                    .eq_ignore_ascii_case(&product.short_name)
                {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.wc.product.duplicate.shortName",
                        format!(
                            "Working capital loan product with shortName `{}` already exists",
                            product.short_name
                        ),
                    )));
                }
            }
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_product_sql(self.dialect);
            sqlx::query(&sql)
                .bind(product.id)
                .bind(&product.short_name)
                .bind(&product.name)
                .bind(&product.currency_code)
                .bind(product.credit_limit_min.to_string())
                .bind(product.credit_limit_default.to_string())
                .bind(product.credit_limit_max.to_string())
                .bind(product.nominal_interest_rate.to_string())
                .bind(product.commitment_fee_rate.map(|d| d.to_string()))
                .bind(product.revolving_line)
                .bind(product.max_drawdowns.map(|m| m as i64))
                .bind(product.amortization_type.value())
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.products.insert(product.id, product.clone());
        Ok(product)
    }

    /// Finds a [`WorkingCapitalLoanProduct`] by ID, returning `Ok(None)` if not found.
    pub async fn find_product_by_id(
        &self,
        product_id: i64,
    ) -> FineractResult<Option<WorkingCapitalLoanProduct>> {
        if let Some(entry) = self.products.get(&product_id) {
            return Ok(Some(entry.value().clone()));
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_product_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(product_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let short_name: String = row
                    .try_get("short_name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let name: String = row
                    .try_get("name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let currency_code: String = row
                    .try_get("currency_code")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let min_raw: String = row
                    .try_get("credit_limit_min")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let def_raw: String = row
                    .try_get("credit_limit_default")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let max_raw: String = row
                    .try_get("credit_limit_max")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let rate_raw: String = row
                    .try_get("nominal_interest_rate")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let fee_raw: Option<String> = row
                    .try_get("commitment_fee_rate")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let revolving_line: bool = row
                    .try_get("revolving_line")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let max_drawdowns_raw: Option<i64> = row
                    .try_get("max_drawdowns")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let amort_code: i32 = row
                    .try_get("amortization_type")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

                let product = WorkingCapitalLoanProduct {
                    id,
                    short_name,
                    name,
                    currency_code,
                    credit_limit_min: Decimal::from_str(&min_raw).unwrap_or(Decimal::ZERO),
                    credit_limit_default: Decimal::from_str(&def_raw).unwrap_or(Decimal::ZERO),
                    credit_limit_max: Decimal::from_str(&max_raw).unwrap_or(Decimal::ZERO),
                    nominal_interest_rate: Decimal::from_str(&rate_raw).unwrap_or(Decimal::ZERO),
                    commitment_fee_rate: fee_raw.and_then(|s| Decimal::from_str(&s).ok()),
                    revolving_line,
                    max_drawdowns: max_drawdowns_raw.and_then(|v| u32::try_from(v).ok()),
                    amortization_type: WorkingCapitalAmortizationType::from_i32(amort_code)
                        .unwrap_or_default(),
                };
                self.products.insert(id, product.clone());
                return Ok(Some(product));
            }
        }
        Ok(None)
    }

    /// Retrieves a [`WorkingCapitalLoanProduct`] by ID, returning `404 ResourceNotFound` if absent.
    pub async fn get_product(&self, product_id: i64) -> FineractResult<WorkingCapitalLoanProduct> {
        self.find_product_by_id(product_id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.wc.product.id.invalid",
                format!("Working capital loan product with identifier {product_id} does not exist"),
                product_id.to_string(),
            )
        })
    }

    /// Lists all [`WorkingCapitalLoanProduct`] records ordered by ID.
    pub async fn list_products(&self) -> FineractResult<Vec<WorkingCapitalLoanProduct>> {
        let mut items: Vec<WorkingCapitalLoanProduct> =
            self.products.iter().map(|e| e.value().clone()).collect();
        items.sort_by_key(|p| p.id);
        Ok(items)
    }

    // --- Loan Account CRUD ---

    /// Saves or updates a [`WorkingCapitalLoanAccount`].
    pub async fn save_loan(
        &self,
        mut loan: WorkingCapitalLoanAccount,
    ) -> FineractResult<WorkingCapitalLoanAccount> {
        let is_existing = loan.id > 0 && self.loans.contains_key(&loan.id);
        if loan.id <= 0 {
            loan.id = self.allocate_loan_id();
        } else {
            self.next_loan_id.fetch_max(loan.id + 1, Ordering::SeqCst);
        }
        if loan.account_no.trim().is_empty() {
            loan.account_no = format!("WCL-{:09}", loan.id);
        }

        for d in &mut loan.drawdowns {
            if d.wc_loan_id <= 0 {
                d.wc_loan_id = loan.id;
            }
            if d.id <= 0 {
                d.id = self.allocate_drawdown_id();
            }
        }

        if let Some(ref pool) = self.pool {
            if is_existing {
                let sql = Self::update_loan_sql(self.dialect);
                sqlx::query(&sql)
                    .bind(&loan.account_no)
                    .bind(loan.client_id)
                    .bind(loan.product_id)
                    .bind(loan.status.value())
                    .bind(loan.sanctioned_credit_limit.currency_code())
                    .bind(loan.sanctioned_credit_limit.amount().to_string())
                    .bind(loan.utilized_credit_limit.amount().to_string())
                    .bind(loan.available_credit_limit.amount().to_string())
                    .bind(loan.id)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            } else {
                let sql = Self::insert_loan_sql(self.dialect);
                sqlx::query(&sql)
                    .bind(loan.id)
                    .bind(&loan.account_no)
                    .bind(loan.client_id)
                    .bind(loan.product_id)
                    .bind(loan.status.value())
                    .bind(loan.sanctioned_credit_limit.currency_code())
                    .bind(loan.sanctioned_credit_limit.amount().to_string())
                    .bind(loan.utilized_credit_limit.amount().to_string())
                    .bind(loan.available_credit_limit.amount().to_string())
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }
        }

        self.loans.insert(loan.id, loan.clone());
        Ok(loan)
    }

    /// Finds a [`WorkingCapitalLoanAccount`] by ID, returning `Ok(None)` if not found.
    pub async fn find_loan_by_id(
        &self,
        loan_id: i64,
    ) -> FineractResult<Option<WorkingCapitalLoanAccount>> {
        if let Some(entry) = self.loans.get(&loan_id) {
            return Ok(Some(entry.value().clone()));
        }
        Ok(None)
    }

    /// Retrieves a [`WorkingCapitalLoanAccount`] by ID, returning `404 ResourceNotFound` if absent.
    pub async fn get_loan(&self, loan_id: i64) -> FineractResult<WorkingCapitalLoanAccount> {
        self.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.wc.loan.id.invalid",
                format!("Working capital loan with identifier {loan_id} does not exist"),
                loan_id.to_string(),
            )
        })
    }

    /// Lists all [`WorkingCapitalLoanAccount`] records ordered by ID.
    pub async fn list_loans(&self) -> FineractResult<Vec<WorkingCapitalLoanAccount>> {
        let mut items: Vec<WorkingCapitalLoanAccount> =
            self.loans.iter().map(|e| e.value().clone()).collect();
        items.sort_by_key(|l| l.id);
        Ok(items)
    }
}

// =============================================================================
// 4. CQRS COMMAND HANDLERS
// =============================================================================

fn resolve_loan_id(command: &JsonCommand) -> FineractResult<i64> {
    if let Some(id) = command.entity_id.or(command.loan_id) {
        if id > 0 {
            return Ok(id);
        }
    }
    if let Some(id) = command.long_value_of_parameter_named("loanId")? {
        if id > 0 {
            return Ok(id);
        }
    }
    Err(FineractError::resource_not_found(
        "error.msg.wc.loan.id.invalid",
        "Working capital loan identifier is required",
        "null",
    ))
}

/// CQRS command handler for `CREATE WORKINGCAPITALLOANPRODUCT`
/// (`CreateWorkingCapitalLoanProductCommandHandler.java`).
#[derive(Debug, Clone, Default)]
pub struct CreateWorkingCapitalProductCommandHandler {
    repo: Arc<DualDbWorkingCapitalLoanRepository>,
}

impl CreateWorkingCapitalProductCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "WORKINGCAPITALLOANPRODUCT";

    pub fn new(repo: Arc<DualDbWorkingCapitalLoanRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbWorkingCapitalLoanRepository::new_in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbWorkingCapitalLoanRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateWorkingCapitalProductCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let product_id = self.repo.allocate_product_id();
        let product = WorkingCapitalLoanProduct::from_json(product_id, command)?;
        let saved = self.repo.save_product(product).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// CQRS command handler for `CREATE WORKINGCAPITALLOAN`
/// (`WorkingCapitalLoanApplicationSubmittalCommandHandler.java`).
#[derive(Debug, Clone, Default)]
pub struct SubmitWorkingCapitalLoanCommandHandler {
    repo: Arc<DualDbWorkingCapitalLoanRepository>,
}

impl SubmitWorkingCapitalLoanCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "WORKINGCAPITALLOAN";

    pub fn new(repo: Arc<DualDbWorkingCapitalLoanRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbWorkingCapitalLoanRepository::new_in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbWorkingCapitalLoanRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for SubmitWorkingCapitalLoanCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let product_id = command
            .long_value_of_parameter_named("productId")?
            .ok_or_else(|| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "validation.msg.wc.loan.productId.required",
                    "Parameter `productId` is required to submit a working capital loan",
                ))
            })?;
        let client_id = command
            .long_value_of_parameter_named("clientId")?
            .or(command.client_id)
            .ok_or_else(|| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "validation.msg.wc.loan.clientId.required",
                    "Parameter `clientId` is required to submit a working capital loan",
                ))
            })?;

        let product = self.repo.get_product(product_id).await?;
        let requested_limit = command
            .big_decimal_value_of_parameter_named("sanctionedCreditLimit")?
            .or(command.big_decimal_value_of_parameter_named("principal")?)
            .or(command.big_decimal_value_of_parameter_named("creditLimit")?)
            .unwrap_or(product.credit_limit_default);

        product.validate_credit_limit(requested_limit)?;

        let loan_id = self.repo.allocate_loan_id();
        let account_no = command
            .string_value_of_parameter_named("accountNo")
            .unwrap_or_else(|| format!("WCL-{loan_id:09}"));
        let sanctioned_money = Money::of(
            product.monetary_currency(),
            requested_limit,
            FineractRoundingMode::HalfEven,
        );

        let account = WorkingCapitalLoanAccount::new(
            loan_id,
            account_no,
            client_id,
            product_id,
            sanctioned_money,
        );
        let saved = self.repo.save_loan(account).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_loan_id(saved.id)
            .with_client_id(saved.client_id)
            .build())
    }
}

/// CQRS command handler for `APPROVE WORKINGCAPITALLOAN`
/// (`ApproveWorkingCapitalLoanCommandHandler.java`).
#[derive(Debug, Clone, Default)]
pub struct ApproveWorkingCapitalLoanCommandHandler {
    repo: Arc<DualDbWorkingCapitalLoanRepository>,
}

impl ApproveWorkingCapitalLoanCommandHandler {
    pub const ACTION_NAME: &'static str = "APPROVE";
    pub const ENTITY_NAME: &'static str = "WORKINGCAPITALLOAN";

    pub fn new(repo: Arc<DualDbWorkingCapitalLoanRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbWorkingCapitalLoanRepository::new_in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbWorkingCapitalLoanRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for ApproveWorkingCapitalLoanCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = resolve_loan_id(command)?;
        let mut loan = self.repo.get_loan(loan_id).await?;
        let product = self.repo.get_product(loan.product_id).await?;

        if let Some(approved_amount) = command
            .big_decimal_value_of_parameter_named("approvedLoanAmount")?
            .or(command.big_decimal_value_of_parameter_named("approvedCreditLimit")?)
            .or(command.big_decimal_value_of_parameter_named("sanctionedCreditLimit")?)
        {
            product.validate_credit_limit(approved_amount)?;
            loan.sanctioned_credit_limit = Money::of(
                product.monetary_currency(),
                approved_amount,
                FineractRoundingMode::HalfEven,
            );
        }

        loan.approve()?;
        let saved = self.repo.save_loan(loan).await?;

        let mut changes = BTreeMap::new();
        changes.insert("status".to_string(), json!(saved.status.value()));
        changes.insert(
            "sanctionedCreditLimit".to_string(),
            json!(saved.sanctioned_credit_limit.amount()),
        );

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_loan_id(saved.id)
            .with_client_id(saved.client_id)
            .with_changes(changes)
            .build())
    }
}

/// CQRS command handler for `DRAWDOWN` / `DISBURSE WORKINGCAPITALLOAN`
/// (`DisburseWorkingCapitalLoanCommandHandler.java`).
#[derive(Debug, Clone, Default)]
pub struct DrawdownWorkingCapitalLoanCommandHandler {
    repo: Arc<DualDbWorkingCapitalLoanRepository>,
}

impl DrawdownWorkingCapitalLoanCommandHandler {
    pub const ACTION_NAME: &'static str = "DRAWDOWN";
    pub const ENTITY_NAME: &'static str = "WORKINGCAPITALLOAN";

    pub fn new(repo: Arc<DualDbWorkingCapitalLoanRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbWorkingCapitalLoanRepository::new_in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbWorkingCapitalLoanRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DrawdownWorkingCapitalLoanCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = resolve_loan_id(command)?;
        let mut loan = self.repo.get_loan(loan_id).await?;
        let product = self.repo.get_product(loan.product_id).await?;

        let raw_amount = command
            .big_decimal_value_of_parameter_named("transactionAmount")?
            .or(command.big_decimal_value_of_parameter_named("amount")?)
            .or(command.big_decimal_value_of_parameter_named("principalAmount")?)
            .ok_or_else(|| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "validation.msg.wc.loan.drawdown.amount.required",
                    "Parameter `transactionAmount` or `amount` is required for drawdown",
                ))
            })?;

        let drawdown_date = command
            .local_date_value_of_parameter_named("drawdownDate")?
            .or(command.local_date_value_of_parameter_named("actualDisbursementDate")?)
            .or(command.local_date_value_of_parameter_named("transactionDate")?)
            .unwrap_or_else(|| ctx.business_date());

        let maturity_date = command
            .local_date_value_of_parameter_named("maturityDate")?
            .or(command.local_date_value_of_parameter_named("expectedMaturityDate")?)
            .unwrap_or_else(|| drawdown_date + Duration::days(90));

        let amount_money = Money::of(
            product.monetary_currency(),
            raw_amount,
            FineractRoundingMode::HalfEven,
        );
        let drawdown =
            loan.execute_drawdown(amount_money, drawdown_date, maturity_date, &product)?;
        let saved = self.repo.save_loan(loan).await?;

        let mut changes: BTreeMap<String, Value> = BTreeMap::new();
        changes.insert("status".to_string(), json!(saved.status.value()));
        changes.insert(
            "utilizedCreditLimit".to_string(),
            json!(saved.utilized_credit_limit.amount()),
        );
        changes.insert(
            "availableCreditLimit".to_string(),
            json!(saved.available_credit_limit.amount()),
        );

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_sub_entity_id(drawdown.id)
            .with_loan_id(saved.id)
            .with_client_id(saved.client_id)
            .with_changes(changes)
            .build())
    }
}

/// CQRS command handler for `REPAYMENT WORKINGCAPITALLOAN`
/// (`RepaymentWorkingCapitalLoanCommandHandler.java`).
#[derive(Debug, Clone, Default)]
pub struct RepayWorkingCapitalLoanCommandHandler {
    repo: Arc<DualDbWorkingCapitalLoanRepository>,
}

impl RepayWorkingCapitalLoanCommandHandler {
    pub const ACTION_NAME: &'static str = "REPAYMENT";
    pub const ENTITY_NAME: &'static str = "WORKINGCAPITALLOAN";

    pub fn new(repo: Arc<DualDbWorkingCapitalLoanRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbWorkingCapitalLoanRepository::new_in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbWorkingCapitalLoanRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for RepayWorkingCapitalLoanCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = resolve_loan_id(command)?;
        let mut loan = self.repo.get_loan(loan_id).await?;
        let product = self.repo.get_product(loan.product_id).await?;

        let raw_amount = command
            .big_decimal_value_of_parameter_named("transactionAmount")?
            .or(command.big_decimal_value_of_parameter_named("amount")?)
            .ok_or_else(|| {
                FineractError::DomainRule(PlatformDomainRuleException::new(
                    "validation.msg.wc.loan.repayment.amount.required",
                    "Parameter `transactionAmount` or `amount` is required for repayment",
                ))
            })?;

        let amount_money = Money::of(
            product.monetary_currency(),
            raw_amount,
            FineractRoundingMode::HalfEven,
        );
        let principal_repaid = loan.make_repayment(amount_money, &product)?;
        let saved = self.repo.save_loan(loan).await?;

        let mut changes: BTreeMap<String, Value> = BTreeMap::new();
        changes.insert(
            "principalRepaid".to_string(),
            json!(principal_repaid.amount()),
        );
        changes.insert(
            "utilizedCreditLimit".to_string(),
            json!(saved.utilized_credit_limit.amount()),
        );
        changes.insert(
            "availableCreditLimit".to_string(),
            json!(saved.available_credit_limit.amount()),
        );

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_loan_id(saved.id)
            .with_client_id(saved.client_id)
            .with_changes(changes)
            .build())
    }
}

// =============================================================================
// 5. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn usd(amount: Decimal) -> Money {
        Money::of(
            MonetaryCurrency::usd(),
            amount,
            FineractRoundingMode::HalfEven,
        )
    }

    fn sample_revolving_product() -> WorkingCapitalLoanProduct {
        WorkingCapitalLoanProduct {
            id: 1,
            short_name: "WCRL".to_string(),
            name: "Revolving Working Capital Line".to_string(),
            currency_code: "USD".to_string(),
            credit_limit_min: dec!(10_000.00),
            credit_limit_default: dec!(100_000.00),
            credit_limit_max: dec!(500_000.00),
            nominal_interest_rate: dec!(12.00),
            commitment_fee_rate: Some(dec!(2.00)),
            revolving_line: true,
            max_drawdowns: Some(3),
            amortization_type: WorkingCapitalAmortizationType::BulletAtMaturity,
        }
    }

    #[test]
    fn test_revolving_credit_line_drawdowns_and_max_drawdowns_limit() {
        let product = sample_revolving_product();
        let mut account = WorkingCapitalLoanAccount::new(
            101,
            "WCL-000000101",
            55,
            product.id,
            usd(dec!(100_000.00)),
        );
        assert_eq!(
            account.status,
            WorkingCapitalLoanStatus::SubmittedAndPendingApproval
        );

        account.approve().expect("approve should succeed");
        assert_eq!(account.status, WorkingCapitalLoanStatus::Approved);

        account.activate().expect("activate should succeed");
        assert_eq!(account.status, WorkingCapitalLoanStatus::Active);
        assert_eq!(account.available_credit_limit, usd(dec!(100_000.00)));
        assert_eq!(account.utilized_credit_limit, usd(dec!(0.00)));

        let d1_date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
        let d1_mat = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        let d1 = account
            .execute_drawdown(usd(dec!(30_000.00)), d1_date, d1_mat, &product)
            .expect("first drawdown should succeed");
        assert_eq!(d1.id, 1);
        assert_eq!(d1.principal_amount, usd(dec!(30_000.00)));
        assert_eq!(account.utilized_credit_limit, usd(dec!(30_000.00)));
        assert_eq!(account.available_credit_limit, usd(dec!(70_000.00)));

        let d2 = account
            .execute_drawdown(usd(dec!(25_000.00)), d1_date, d1_mat, &product)
            .expect("second drawdown should succeed");
        assert_eq!(d2.id, 2);
        assert_eq!(account.utilized_credit_limit, usd(dec!(55_000.00)));
        assert_eq!(account.available_credit_limit, usd(dec!(45_000.00)));

        let d3 = account
            .execute_drawdown(usd(dec!(15_000.00)), d1_date, d1_mat, &product)
            .expect("third drawdown should succeed");
        assert_eq!(d3.id, 3);
        assert_eq!(account.utilized_credit_limit, usd(dec!(70_000.00)));
        assert_eq!(account.available_credit_limit, usd(dec!(30_000.00)));

        // 4th drawdown must be rejected because max_drawdowns = Some(3)
        let fourth = account.execute_drawdown(usd(dec!(5_000.00)), d1_date, d1_mat, &product);
        assert!(
            fourth.is_err(),
            "fourth drawdown must fail max_drawdowns = 3"
        );
    }

    #[test]
    fn test_over_limit_drawdown_rejection() {
        let product = sample_revolving_product();
        let mut account = WorkingCapitalLoanAccount::new(
            102,
            "WCL-000000102",
            55,
            product.id,
            usd(dec!(50_000.00)),
        );
        account.approve().unwrap();
        account.activate().unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        let maturity = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();

        account
            .execute_drawdown(usd(dec!(40_000.00)), date, maturity, &product)
            .expect("40k drawdown within 50k limit should succeed");
        assert_eq!(account.available_credit_limit, usd(dec!(10_000.00)));

        // Attempting to draw $15,000 when only $10,000 is available must fail
        let err = account
            .execute_drawdown(usd(dec!(15_000.00)), date, maturity, &product)
            .expect_err("drawdown exceeding available credit limit must be rejected");
        assert!(err
            .to_string()
            .contains("exceeds available credit limit"));
        assert_eq!(account.utilized_credit_limit, usd(dec!(40_000.00)));
        assert_eq!(account.available_credit_limit, usd(dec!(10_000.00)));
    }

    #[test]
    fn test_daily_interest_on_utilized_and_commitment_fee_on_undrawn_limit() {
        let mut product = sample_revolving_product();
        // Use 36.50% nominal interest and 3.65% commitment fee so 1-day accrual is exact ($10/day per $10k)
        product.nominal_interest_rate = dec!(36.50);
        product.commitment_fee_rate = Some(dec!(3.65));

        let mut account = WorkingCapitalLoanAccount::new(
            103,
            "WCL-000000103",
            55,
            product.id,
            usd(dec!(100_000.00)),
        );
        account.approve().unwrap();
        account.activate().unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let maturity = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();

        // Draw $40,000 -> utilized = $40,000, available (undrawn) = $60,000
        account
            .execute_drawdown(usd(dec!(40_000.00)), date, maturity, &product)
            .unwrap();

        let (daily_interest, daily_fee) = account
            .accrue_daily_interest_and_commitment_fee(&product)
            .expect("daily accrual should succeed");

        // Interest: 40,000 * 36.50% / 365 = $40.00
        assert_eq!(daily_interest, usd(dec!(40.00)));
        // Commitment fee: 60,000 * 3.65% / 365 = $6.00
        assert_eq!(daily_fee, usd(dec!(6.00)));

        assert_eq!(account.drawdowns[0].accrued_interest, usd(dec!(40.00)));
        assert_eq!(account.drawdowns[0].fee_amount, usd(dec!(6.00)));
    }

    #[test]
    fn test_repayment_credit_limit_replenishment_revolving_vs_non_revolving() {
        let mut revolving_product = sample_revolving_product();
        revolving_product.nominal_interest_rate = dec!(36.50);
        revolving_product.commitment_fee_rate = Some(dec!(3.65));

        let mut account = WorkingCapitalLoanAccount::new(
            104,
            "WCL-000000104",
            55,
            revolving_product.id,
            usd(dec!(100_000.00)),
        );
        account.approve().unwrap();
        account.activate().unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let maturity = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();

        account
            .execute_drawdown(usd(dec!(40_000.00)), date, maturity, &revolving_product)
            .unwrap();
        account
            .accrue_daily_interest_and_commitment_fee(&revolving_product)
            .unwrap();
        // Now drawdown has: fee = $6.00, interest = $40.00, principal = $40,000.00

        // Repay $10,046.00 -> pays $6.00 fee, $40.00 interest, and $10,000.00 principal
        let principal_repaid = account
            .make_repayment(usd(dec!(10_046.00)), &revolving_product)
            .expect("repayment should succeed");
        assert_eq!(principal_repaid, usd(dec!(10_000.00)));
        assert_eq!(account.drawdowns[0].fee_amount, usd(dec!(0.00)));
        assert_eq!(account.drawdowns[0].accrued_interest, usd(dec!(0.00)));
        assert_eq!(
            account.drawdowns[0].outstanding_principal,
            usd(dec!(30_000.00))
        );
        assert_eq!(account.utilized_credit_limit, usd(dec!(30_000.00)));
        // Revolving line replenishes available credit limit from $60,000 -> $70,000!
        assert_eq!(account.available_credit_limit, usd(dec!(70_000.00)));

        // Pay remaining $30,000 principal and close
        account
            .make_repayment(usd(dec!(30_000.00)), &revolving_product)
            .unwrap();
        assert_eq!(account.utilized_credit_limit, usd(dec!(0.00)));
        assert_eq!(account.available_credit_limit, usd(dec!(100_000.00)));
        account.close().expect("closing settled loan should succeed");
        assert_eq!(account.status, WorkingCapitalLoanStatus::Closed);

        // Now verify non-revolving line does NOT replenish available credit limit
        let mut non_revolving_product = sample_revolving_product();
        non_revolving_product.revolving_line = false;

        let mut non_rev_account = WorkingCapitalLoanAccount::new(
            105,
            "WCL-000000105",
            55,
            non_revolving_product.id,
            usd(dec!(100_000.00)),
        );
        non_rev_account.approve().unwrap();
        non_rev_account.activate().unwrap();
        non_rev_account
            .execute_drawdown(usd(dec!(40_000.00)), date, maturity, &non_revolving_product)
            .unwrap();
        assert_eq!(non_rev_account.available_credit_limit, usd(dec!(60_000.00)));

        non_rev_account
            .make_repayment(usd(dec!(15_000.00)), &non_revolving_product)
            .unwrap();
        assert_eq!(non_rev_account.utilized_credit_limit, usd(dec!(25_000.00)));
        // Non-revolving available credit limit stays at $60,000.00
        assert_eq!(non_rev_account.available_credit_limit, usd(dec!(60_000.00)));
    }

    #[tokio::test]
    async fn test_cqrs_command_handlers_end_to_end() {
        let repo = Arc::new(DualDbWorkingCapitalLoanRepository::new_in_memory());
        let ctx = ExecutionContext::new("default", NaiveDate::from_ymd_opt(2026, 3, 15).unwrap());

        let create_product_handler = CreateWorkingCapitalProductCommandHandler::new(repo.clone());
        let submit_loan_handler = SubmitWorkingCapitalLoanCommandHandler::new(repo.clone());
        let approve_loan_handler = ApproveWorkingCapitalLoanCommandHandler::new(repo.clone());
        let drawdown_handler = DrawdownWorkingCapitalLoanCommandHandler::new(repo.clone());
        let repay_handler = RepayWorkingCapitalLoanCommandHandler::new(repo.clone());

        let prod_cmd = JsonCommand::from_value(
            None,
            json!({
                "name": "SME Working Capital Line",
                "shortName": "SMEW",
                "currencyCode": "USD",
                "creditLimitMin": "5000.00",
                "creditLimitDefault": "50000.00",
                "creditLimitMax": "200000.00",
                "nominalInterestRate": "10.00",
                "commitmentFeeRate": "1.50",
                "revolvingLine": true,
                "maxDrawdowns": 5,
                "amortizationType": "EQUAL_INSTALLMENTS",
                "locale": "en"
            }),
        );
        let prod_res = create_product_handler.handle(&ctx, &prod_cmd).await.unwrap();
        let product_id = prod_res.resource_id.unwrap();

        let submit_cmd = JsonCommand::from_value(
            None,
            json!({
                "productId": product_id,
                "clientId": 77,
                "sanctionedCreditLimit": "80000.00",
                "locale": "en"
            }),
        );
        let submit_res = submit_loan_handler.handle(&ctx, &submit_cmd).await.unwrap();
        let loan_id = submit_res.resource_id.unwrap();

        let approve_cmd = JsonCommand::from_value(Some(loan_id), json!({}));
        approve_loan_handler
            .handle(&ctx, &approve_cmd)
            .await
            .unwrap();

        let drawdown_cmd = JsonCommand::from_value(
            Some(loan_id),
            json!({
                "transactionAmount": "30000.00",
                "drawdownDate": "2026-03-15",
                "maturityDate": "2026-06-15",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        );
        drawdown_handler.handle(&ctx, &drawdown_cmd).await.unwrap();

        let loan_after_draw = repo.get_loan(loan_id).await.unwrap();
        assert_eq!(loan_after_draw.status, WorkingCapitalLoanStatus::Active);
        assert_eq!(loan_after_draw.utilized_credit_limit, usd(dec!(30_000.00)));
        assert_eq!(loan_after_draw.available_credit_limit, usd(dec!(50_000.00)));

        let repay_cmd = JsonCommand::from_value(
            Some(loan_id),
            json!({
                "transactionAmount": "12000.00",
                "locale": "en"
            }),
        );
        repay_handler.handle(&ctx, &repay_cmd).await.unwrap();

        let loan_after_repay = repo.get_loan(loan_id).await.unwrap();
        assert_eq!(loan_after_repay.utilized_credit_limit, usd(dec!(18_000.00)));
        assert_eq!(
            loan_after_repay.available_credit_limit,
            usd(dec!(62_000.00))
        );
    }
}
