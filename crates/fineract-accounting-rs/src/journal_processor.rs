//! Double-Entry Journal Entry Processor, Accounting Bridge (Cash & Accrual), and Running Balance Engine.
//!
//! Faithfully ports Apache Fineract's `fineract-accounting` and `fineract-provider` journal entry
//! write services, loan/savings accounting bridge processors, reversal workflows, and running
//! balance calculators:
//! - `org.apache.fineract.accounting.common.AccountingRuleType`
//! - `org.apache.fineract.accounting.common.AccountingConstants`
//! - `org.apache.fineract.accounting.journalentry.domain.JournalEntry`
//! - `org.apache.fineract.accounting.journalentry.domain.JournalEntryType`
//! - `org.apache.fineract.portfolio.PortfolioProductType`
//! - `org.apache.fineract.accounting.journalentry.service.JournalEntryWritePlatformServiceJpaRepositoryImpl`
//! - `org.apache.fineract.accounting.journalentry.service.AccountingProcessorHelper`
//! - `org.apache.fineract.accounting.journalentry.service.CashBasedAccountingProcessorForLoan`
//! - `org.apache.fineract.accounting.journalentry.service.AccrualBasedAccountingProcessorForLoan`
//! - `org.apache.fineract.accounting.journalentry.service.CashBasedAccountingProcessorForSavings`
//! - `org.apache.fineract.accounting.journalentry.service.AccrualBasedAccountingProcessorForSavings`
//! - `org.apache.fineract.accounting.journalentry.service.JournalEntryRunningBalanceUpdateServiceImpl`

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use chrono::NaiveDate;
use fineract_command_rs::{CommandEnvelope, CommandHandler, CommandProcessingResult};
use fineract_core_rs::error::{ApiParameterError, FineractError};
use fineract_persistence_rs::{DbRow, DualDbPool, SqlParam};
use fineract_validation_rs::DataValidatorBuilder;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::gl_and_rules::{GLAccount, GLAccountType, GLAccountUsage, GLClosure};

// ============================================================================
// 1. Core Domain Enumerations
// ============================================================================

/// Mirrors `org.apache.fineract.accounting.common.AccountingRuleType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum AccountingRuleType {
    None = 1,
    CashBased = 2,
    AccrualPeriodic = 3,
    AccrualUpfront = 4,
}

impl AccountingRuleType {
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::None),
            2 => Some(Self::CashBased),
            3 => Some(Self::AccrualPeriodic),
            4 => Some(Self::AccrualUpfront),
            _ => None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::None => "accountingRuleType.none",
            Self::CashBased => "accountingRuleType.cash",
            Self::AccrualPeriodic => "accountingRuleType.accrual.periodic",
            Self::AccrualUpfront => "accountingRuleType.accrual.upfront",
        }
    }

    #[inline]
    pub const fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    #[inline]
    pub const fn is_cash_based(self) -> bool {
        matches!(self, Self::CashBased)
    }

    #[inline]
    pub const fn is_accrual_based(self) -> bool {
        matches!(self, Self::AccrualPeriodic | Self::AccrualUpfront)
    }

    #[inline]
    pub const fn is_periodic_accrual(self) -> bool {
        matches!(self, Self::AccrualPeriodic)
    }

    #[inline]
    pub const fn is_upfront_accrual(self) -> bool {
        matches!(self, Self::AccrualUpfront)
    }
}

/// Mirrors `org.apache.fineract.accounting.journalentry.domain.JournalEntryType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum JournalEntryType {
    Credit = 1,
    Debit = 2,
}

impl JournalEntryType {
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::Credit),
            2 => Some(Self::Debit),
            _ => None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Credit => "journalEntryType.credit",
            Self::Debit => "journalEntryType.debit",
        }
    }

    #[inline]
    pub const fn is_debit(self) -> bool {
        matches!(self, Self::Debit)
    }

    #[inline]
    pub const fn is_credit(self) -> bool {
        matches!(self, Self::Credit)
    }

    #[inline]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Credit => Self::Debit,
            Self::Debit => Self::Credit,
        }
    }
}

/// Mirrors `org.apache.fineract.portfolio.PortfolioProductType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum PortfolioProductType {
    Loan = 1,
    Saving = 2,
    Client = 3,
    Provisioning = 4,
    Shares = 5,
    WorkingCapitalLoan = 6,
}

impl PortfolioProductType {
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::Loan),
            2 => Some(Self::Saving),
            3 => Some(Self::Client),
            4 => Some(Self::Provisioning),
            5 => Some(Self::Shares),
            6 => Some(Self::WorkingCapitalLoan),
            _ => None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Loan => "portfolioProductType.loan",
            Self::Saving => "portfolioProductType.saving",
            Self::Client => "portfolioProductType.client",
            Self::Provisioning => "portfolioProductType.provisioning",
            Self::Shares => "portfolioProductType.shares",
            Self::WorkingCapitalLoan => "portfolioProductType.workingCapitalLoan",
        }
    }
}

// ============================================================================
// 2. Product Accounting Parameter Slots & GL Mappings
// ============================================================================

/// Mirrors `AccountingConstants.CashAccountsForLoan` and `AccountingConstants.AccrualAccountsForLoan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum LoanProductAccountingParams {
    FundSource = 1,
    LoanPortfolio = 2,
    InterestOnLoans = 3,
    IncomeFromFees = 4,
    IncomeFromPenalties = 5,
    LossesWrittenOff = 6,
    InterestReceivable = 7,
    FeesReceivable = 8,
    PenaltiesReceivable = 9,
    TransfersInSuspense = 10,
    OverpaymentLiability = 11,
    IncomeFromRecovery = 12,
    GoodwillCredit = 13,
    IncomeFromChargeOffInterest = 14,
    IncomeFromChargeOffFees = 15,
    ChargeOffExpense = 16,
    ChargeOffFraudExpense = 17,
    IncomeFromChargeOffPenalty = 18,
    IncomeFromGoodwillCreditInterest = 19,
    IncomeFromGoodwillCreditFees = 20,
    IncomeFromGoodwillCreditPenalty = 21,
    DeferredIncomeLiability = 22,
    IncomeFromCapitalization = 23,
    BuyDownExpense = 24,
}

impl LoanProductAccountingParams {
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::FundSource),
            2 => Some(Self::LoanPortfolio),
            3 => Some(Self::InterestOnLoans),
            4 => Some(Self::IncomeFromFees),
            5 => Some(Self::IncomeFromPenalties),
            6 => Some(Self::LossesWrittenOff),
            7 => Some(Self::InterestReceivable),
            8 => Some(Self::FeesReceivable),
            9 => Some(Self::PenaltiesReceivable),
            10 => Some(Self::TransfersInSuspense),
            11 => Some(Self::OverpaymentLiability),
            12 => Some(Self::IncomeFromRecovery),
            13 => Some(Self::GoodwillCredit),
            14 => Some(Self::IncomeFromChargeOffInterest),
            15 => Some(Self::IncomeFromChargeOffFees),
            16 => Some(Self::ChargeOffExpense),
            17 => Some(Self::ChargeOffFraudExpense),
            18 => Some(Self::IncomeFromChargeOffPenalty),
            19 => Some(Self::IncomeFromGoodwillCreditInterest),
            20 => Some(Self::IncomeFromGoodwillCreditFees),
            21 => Some(Self::IncomeFromGoodwillCreditPenalty),
            22 => Some(Self::DeferredIncomeLiability),
            23 => Some(Self::IncomeFromCapitalization),
            24 => Some(Self::BuyDownExpense),
            _ => None,
        }
    }

    pub const fn param_name(self) -> &'static str {
        match self {
            Self::FundSource => "fundSourceAccountId",
            Self::LoanPortfolio => "loanPortfolioAccountId",
            Self::InterestOnLoans => "interestOnLoanAccountId",
            Self::IncomeFromFees => "incomeFromFeeAccountId",
            Self::IncomeFromPenalties => "incomeFromPenaltyAccountId",
            Self::LossesWrittenOff => "writeOffAccountId",
            Self::InterestReceivable => "receivableInterestAccountId",
            Self::FeesReceivable => "receivableFeeAccountId",
            Self::PenaltiesReceivable => "receivablePenaltyAccountId",
            Self::TransfersInSuspense => "transfersInSuspenseAccountId",
            Self::OverpaymentLiability => "overpaymentLiabilityAccountId",
            Self::IncomeFromRecovery => "incomeFromRecoveryAccountId",
            Self::GoodwillCredit => "goodwillCreditAccountId",
            Self::IncomeFromChargeOffInterest => "incomeFromChargeOffInterestAccountId",
            Self::IncomeFromChargeOffFees => "incomeFromChargeOffFeesAccountId",
            Self::ChargeOffExpense => "chargeOffExpenseAccountId",
            Self::ChargeOffFraudExpense => "chargeOffFraudExpenseAccountId",
            Self::IncomeFromChargeOffPenalty => "incomeFromChargeOffPenaltyAccountId",
            Self::IncomeFromGoodwillCreditInterest => "incomeFromGoodwillCreditInterestAccountId",
            Self::IncomeFromGoodwillCreditFees => "incomeFromGoodwillCreditFeesAccountId",
            Self::IncomeFromGoodwillCreditPenalty => "incomeFromGoodwillCreditPenaltyAccountId",
            Self::DeferredIncomeLiability => "deferredIncomeLiabilityAccountId",
            Self::IncomeFromCapitalization => "incomeFromCapitalizationAccountId",
            Self::BuyDownExpense => "buyDownExpenseAccountId",
        }
    }
}

/// Mirrors `AccountingConstants.CashAccountsForSavings` and `AccountingConstants.AccrualAccountsForSavings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum SavingsProductAccountingParams {
    SavingsReference = 1,
    SavingsControl = 2,
    InterestOnSavings = 3,
    IncomeFromFees = 4,
    IncomeFromPenalties = 5,
    TransfersInSuspense = 10,
    OverdraftPortfolioControl = 11,
    IncomeFromInterest = 12,
    LossesWrittenOff = 13,
    EscheatLiability = 14,
    InterestPayable = 15,
    PenaltiesReceivable = 16,
    FeesReceivable = 17,
}

impl SavingsProductAccountingParams {
    #[inline]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::SavingsReference),
            2 => Some(Self::SavingsControl),
            3 => Some(Self::InterestOnSavings),
            4 => Some(Self::IncomeFromFees),
            5 => Some(Self::IncomeFromPenalties),
            10 => Some(Self::TransfersInSuspense),
            11 => Some(Self::OverdraftPortfolioControl),
            12 => Some(Self::IncomeFromInterest),
            13 => Some(Self::LossesWrittenOff),
            14 => Some(Self::EscheatLiability),
            15 => Some(Self::InterestPayable),
            16 => Some(Self::PenaltiesReceivable),
            17 => Some(Self::FeesReceivable),
            _ => None,
        }
    }

    pub const fn param_name(self) -> &'static str {
        match self {
            Self::SavingsReference => "savingsReferenceAccountId",
            Self::SavingsControl => "savingsControlAccountId",
            Self::InterestOnSavings => "interestOnSavingsAccountId",
            Self::IncomeFromFees => "incomeFromFeeAccountId",
            Self::IncomeFromPenalties => "incomeFromPenaltyAccountId",
            Self::TransfersInSuspense => "transfersInSuspenseAccountId",
            Self::OverdraftPortfolioControl => "overdraftPortfolioControlId",
            Self::IncomeFromInterest => "incomeFromInterestId",
            Self::LossesWrittenOff => "writeOffAccountId",
            Self::EscheatLiability => "escheatLiabilityId",
            Self::InterestPayable => "interestPayableAccountId",
            Self::PenaltiesReceivable => "penaltiesReceivableAccountId",
            Self::FeesReceivable => "feesReceivableAccountId",
        }
    }
}

/// Mirrors `acc_product_mapping` row in `fineract_schema_dual.sql`
/// (`org.apache.fineract.accounting.producttoaccountmapping.domain.ProductToGLAccountMapping`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductToGLAccountMapping {
    pub id: i64,
    pub gl_account_id: i64,
    pub product_id: i64,
    pub product_type: PortfolioProductType,
    pub payment_type_id: Option<i64>,
    pub charge_id: Option<i64>,
    pub financial_account_type: i16,
}

/// Complete resolved GL account configuration for a Loan Product.
/// Supports default slot mappings, payment-channel fund-source overrides, and charge-specific income overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanProductAccountMapping {
    pub product_id: i64,
    pub accounting_rule: AccountingRuleType,
    pub accounts: HashMap<LoanProductAccountingParams, i64>,
    pub payment_channel_fund_sources: HashMap<i64, i64>,
    pub fee_to_income_accounts: HashMap<i64, i64>,
    pub penalty_to_income_accounts: HashMap<i64, i64>,
}

impl LoanProductAccountMapping {
    pub fn new(product_id: i64, accounting_rule: AccountingRuleType) -> Self {
        Self {
            product_id,
            accounting_rule,
            accounts: HashMap::new(),
            payment_channel_fund_sources: HashMap::new(),
            fee_to_income_accounts: HashMap::new(),
            penalty_to_income_accounts: HashMap::new(),
        }
    }

    pub fn with_account(mut self, param: LoanProductAccountingParams, gl_account_id: i64) -> Self {
        self.accounts.insert(param, gl_account_id);
        self
    }

    pub fn with_payment_channel_fund_source(
        mut self,
        payment_type_id: i64,
        gl_account_id: i64,
    ) -> Self {
        self.payment_channel_fund_sources
            .insert(payment_type_id, gl_account_id);
        self
    }

    pub fn with_fee_income_account(mut self, charge_id: i64, gl_account_id: i64) -> Self {
        self.fee_to_income_accounts.insert(charge_id, gl_account_id);
        self
    }

    pub fn with_penalty_income_account(mut self, charge_id: i64, gl_account_id: i64) -> Self {
        self.penalty_to_income_accounts
            .insert(charge_id, gl_account_id);
        self
    }

    /// Resolves a GL account for the given `LoanProductAccountingParams`, honoring payment-type
    /// overrides on `FundSource` and charge overrides on `IncomeFromFees` / `IncomeFromPenalties`.
    pub fn resolve_account(
        &self,
        param: LoanProductAccountingParams,
        payment_type_id: Option<i64>,
        charge_id: Option<i64>,
    ) -> Result<i64, FineractError> {
        if param == LoanProductAccountingParams::FundSource {
            if let Some(pt_id) = payment_type_id {
                if let Some(&gl_id) = self.payment_channel_fund_sources.get(&pt_id) {
                    return Ok(gl_id);
                }
            }
        }
        if param == LoanProductAccountingParams::IncomeFromFees {
            if let Some(ch_id) = charge_id {
                if let Some(&gl_id) = self.fee_to_income_accounts.get(&ch_id) {
                    return Ok(gl_id);
                }
            }
        }
        if param == LoanProductAccountingParams::IncomeFromPenalties {
            if let Some(ch_id) = charge_id {
                if let Some(&gl_id) = self.penalty_to_income_accounts.get(&ch_id) {
                    return Ok(gl_id);
                }
            }
        }
        self.accounts.get(&param).copied().ok_or_else(|| {
            FineractError::DomainValidation(vec![ApiParameterError::parameter(
                param.param_name(),
                "error.msg.accounting.product.mapping.not.found",
                format!(
                    "GL account mapping `{}` ({:?}) not configured for loan product {}",
                    param.param_name(),
                    param,
                    self.product_id
                ),
            )])
        })
    }
}

/// Complete resolved GL account configuration for a Savings Product.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsProductAccountMapping {
    pub product_id: i64,
    pub accounting_rule: AccountingRuleType,
    pub accounts: HashMap<SavingsProductAccountingParams, i64>,
    pub payment_channel_fund_sources: HashMap<i64, i64>,
    pub fee_to_income_accounts: HashMap<i64, i64>,
    pub penalty_to_income_accounts: HashMap<i64, i64>,
}

impl SavingsProductAccountMapping {
    pub fn new(product_id: i64, accounting_rule: AccountingRuleType) -> Self {
        Self {
            product_id,
            accounting_rule,
            accounts: HashMap::new(),
            payment_channel_fund_sources: HashMap::new(),
            fee_to_income_accounts: HashMap::new(),
            penalty_to_income_accounts: HashMap::new(),
        }
    }

    pub fn with_account(
        mut self,
        param: SavingsProductAccountingParams,
        gl_account_id: i64,
    ) -> Self {
        self.accounts.insert(param, gl_account_id);
        self
    }

    pub fn with_payment_channel_fund_source(
        mut self,
        payment_type_id: i64,
        gl_account_id: i64,
    ) -> Self {
        self.payment_channel_fund_sources
            .insert(payment_type_id, gl_account_id);
        self
    }

    pub fn with_fee_income_account(mut self, charge_id: i64, gl_account_id: i64) -> Self {
        self.fee_to_income_accounts.insert(charge_id, gl_account_id);
        self
    }

    pub fn with_penalty_income_account(mut self, charge_id: i64, gl_account_id: i64) -> Self {
        self.penalty_to_income_accounts
            .insert(charge_id, gl_account_id);
        self
    }

    /// Resolves a GL account for the given `SavingsProductAccountingParams`, honoring payment-type
    /// overrides on `SavingsReference` and charge overrides on `IncomeFromFees` / `IncomeFromPenalties`.
    pub fn resolve_account(
        &self,
        param: SavingsProductAccountingParams,
        payment_type_id: Option<i64>,
        charge_id: Option<i64>,
    ) -> Result<i64, FineractError> {
        if param == SavingsProductAccountingParams::SavingsReference {
            if let Some(pt_id) = payment_type_id {
                if let Some(&gl_id) = self.payment_channel_fund_sources.get(&pt_id) {
                    return Ok(gl_id);
                }
            }
        }
        if param == SavingsProductAccountingParams::IncomeFromFees {
            if let Some(ch_id) = charge_id {
                if let Some(&gl_id) = self.fee_to_income_accounts.get(&ch_id) {
                    return Ok(gl_id);
                }
            }
        }
        if param == SavingsProductAccountingParams::IncomeFromPenalties {
            if let Some(ch_id) = charge_id {
                if let Some(&gl_id) = self.penalty_to_income_accounts.get(&ch_id) {
                    return Ok(gl_id);
                }
            }
        }
        self.accounts.get(&param).copied().ok_or_else(|| {
            FineractError::DomainValidation(vec![ApiParameterError::parameter(
                param.param_name(),
                "error.msg.accounting.product.mapping.not.found",
                format!(
                    "GL account mapping `{}` ({:?}) not configured for savings product {}",
                    param.param_name(),
                    param,
                    self.product_id
                ),
            )])
        })
    }
}

// ============================================================================
// 3. Journal Entry Lines & Persistent Record (`acc_gl_journal_entry`)
// ============================================================================

/// Single debit or credit leg in a journal entry batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntryLine {
    pub gl_account_id: i64,
    pub entry_type: JournalEntryType,
    pub amount: Decimal,
    pub comments: Option<String>,
}

impl JournalEntryLine {
    pub fn debit(gl_account_id: i64, amount: Decimal, comments: Option<String>) -> Self {
        Self {
            gl_account_id,
            entry_type: JournalEntryType::Debit,
            amount,
            comments,
        }
    }

    pub fn credit(gl_account_id: i64, amount: Decimal, comments: Option<String>) -> Self {
        Self {
            gl_account_id,
            entry_type: JournalEntryType::Credit,
            amount,
            comments,
        }
    }
}

/// Persistent representation of `acc_gl_journal_entry` in `fineract_schema_dual.sql`
/// (`org.apache.fineract.accounting.journalentry.domain.JournalEntry`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntryRecord {
    pub id: i64,
    pub account_id: i64,
    pub office_id: i64,
    pub reversal_id: Option<i64>,
    pub currency_code: String,
    pub transaction_id: String,
    pub loan_transaction_id: Option<i64>,
    pub savings_transaction_id: Option<i64>,
    pub client_transaction_id: Option<i64>,
    pub share_transaction_id: Option<i64>,
    pub reversed: bool,
    pub manual_entry: bool,
    pub entry_date: NaiveDate,
    pub type_enum: i16,
    pub amount: Decimal,
    pub description: Option<String>,
    pub entity_type_enum: Option<i16>,
    pub entity_id: Option<i64>,
    pub office_running_balance: Decimal,
    pub organization_running_balance: Decimal,
    pub is_running_balance_calculated: bool,
    pub payment_details_id: Option<i64>,
    pub ref_num: Option<String>,
    pub submitted_on_date: NaiveDate,
}

impl JournalEntryRecord {
    /// Returns the typed `JournalEntryType` (`Credit = 1`, `Debit = 2`) for this record.
    pub fn entry_type(&self) -> Option<JournalEntryType> {
        JournalEntryType::from_i16(self.type_enum)
    }

    /// Returns the typed `PortfolioProductType` if `entity_type_enum` is populated.
    pub fn portfolio_product_type(&self) -> Option<PortfolioProductType> {
        self.entity_type_enum.and_then(PortfolioProductType::from_i16)
    }

    /// Converts a database row from `acc_gl_journal_entry` into a `JournalEntryRecord`.
    pub fn from_db_row(row: &DbRow) -> Result<Self, FineractError> {
        Ok(Self {
            id: row.get_i64("id")?,
            account_id: row.get_i64("account_id")?,
            office_id: row.get_i64("office_id")?,
            reversal_id: row.get_opt_i64("reversal_id")?,
            currency_code: row.get_string("currency_code")?,
            transaction_id: row.get_string("transaction_id")?,
            loan_transaction_id: row.get_opt_i64("loan_transaction_id")?,
            savings_transaction_id: row.get_opt_i64("savings_transaction_id")?,
            client_transaction_id: row.get_opt_i64("client_transaction_id")?,
            share_transaction_id: row.get_opt_i64("share_transaction_id")?,
            reversed: row.get_bool("reversed")?,
            manual_entry: row.get_bool("manual_entry")?,
            entry_date: row.get_date("entry_date")?,
            type_enum: row.get_i16("type_enum")?,
            amount: row.get_decimal("amount")?,
            description: row.get_opt_string("description")?,
            entity_type_enum: row.get_opt_i16("entity_type_enum")?,
            entity_id: row.get_opt_i64("entity_id")?,
            office_running_balance: row.get_decimal("office_running_balance")?,
            organization_running_balance: row.get_decimal("organization_running_balance")?,
            is_running_balance_calculated: row.get_bool("is_running_balance_calculated")?,
            payment_details_id: row.get_opt_i64("payment_details_id")?,
            ref_num: row.get_opt_string("ref_num")?,
            submitted_on_date: row.get_date("submitted_on_date")?,
        })
    }
}

// ============================================================================
// 4. Double-Entry Journal Validator
// ============================================================================

/// Enforces Fineract's strict double-entry accounting invariants (`JournalEntryWritePlatformServiceJpaRepositoryImpl`).
pub struct DoubleEntryJournalValidator;

impl DoubleEntryJournalValidator {
    /// Validates a batch of `JournalEntryLine`s for a given office, date, and currency:
    /// 1. `currency_code` is non-blank.
    /// 2. At least one `Debit` and at least one `Credit` line are present.
    /// 3. Every line has `gl_account_id > 0` and `amount > Decimal::ZERO`.
    /// 4. Every referenced `GLAccount` exists in `accounts`, is not `disabled`, and has `GLAccountUsage::Detail`.
    /// 5. `sum(Debits) == sum(Credits) > Decimal::ZERO`.
    /// 6. `entry_date > latest_closure_date` for all non-deleted `GLClosure`s of `office_id`.
    pub fn validate_balanced_batch(
        office_id: i64,
        entry_date: NaiveDate,
        currency_code: &str,
        lines: &[JournalEntryLine],
        closures: &[GLClosure],
        accounts: &HashMap<i64, GLAccount>,
    ) -> Result<(), FineractError> {
        Self::validate_balanced_batch_with_options(
            office_id,
            entry_date,
            currency_code,
            lines,
            closures,
            accounts,
            false,
            None,
        )
    }

    /// Extended double-entry validator that also enforces manual-entry permission (`manual_entries_allowed`)
    /// when `is_manual == true` and rejects future dates when `business_date` is supplied.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_balanced_batch_with_options(
        office_id: i64,
        entry_date: NaiveDate,
        currency_code: &str,
        lines: &[JournalEntryLine],
        closures: &[GLClosure],
        accounts: &HashMap<i64, GLAccount>,
        is_manual: bool,
        business_date: Option<NaiveDate>,
    ) -> Result<(), FineractError> {
        let mut dvb = DataValidatorBuilder::new("glJournalEntry");
        dvb.reset()
            .parameter("officeId")
            .value(Some(office_id))
            .not_null()
            .integer_greater_than_zero();
        dvb.reset()
            .parameter("currencyCode")
            .value(Some(currency_code))
            .not_blank()
            .not_exceeding_length_of(3);
        dvb.finish()?;

        // Check future date if business_date is provided
        if let Some(max_date) = business_date {
            if entry_date > max_date {
                return Err(FineractError::DomainRule {
                    globalisation_code: "error.msg.glJournalEntry.invalid.future.date".to_string(),
                    default_user_message: format!(
                        "The journal entry cannot be made for a future date `{}` (business date `{}`)",
                        entry_date, max_date
                    ),
                });
            }
        }

        // Check office accounting closure date: entry_date must be strictly after latest closing_date
        let latest_closure_date = closures
            .iter()
            .filter(|c| c.office_id == office_id && !c.is_deleted)
            .map(|c| c.closing_date)
            .max();

        if let Some(closing_date) = latest_closure_date {
            if entry_date <= closing_date {
                return Err(FineractError::DomainRule {
                    globalisation_code: "error.msg.glJournalEntry.invalid.accounting.closed"
                        .to_string(),
                    default_user_message: format!(
                        "Journal entry on `{}` cannot be made prior to or on last account closing date `{}` for office {}",
                        entry_date, closing_date, office_id
                    ),
                });
            }
        }

        if lines.is_empty() {
            return Err(FineractError::DomainRule {
                globalisation_code: "error.msg.glJournalEntry.invalid.no.debits.or.credits"
                    .to_string(),
                default_user_message:
                    "Journal Entry must have at least one Debit and one Credit".to_string(),
            });
        }

        let mut debit_count = 0usize;
        let mut credit_count = 0usize;
        let mut total_debits = Decimal::ZERO;
        let mut total_credits = Decimal::ZERO;

        for line in lines {
            if line.gl_account_id <= 0 || line.amount <= Decimal::ZERO {
                return Err(FineractError::DomainRule {
                    globalisation_code:
                        "error.msg.glJournalEntry.invalid.empty.account.or.amount".to_string(),
                    default_user_message:
                        "Both account and positive amount must be specified for all Debits and Credits"
                            .to_string(),
                });
            }

            let account = accounts.get(&line.gl_account_id).ok_or_else(|| {
                FineractError::DomainRule {
                    globalisation_code:
                        "error.msg.glJournalEntry.invalid.debit.or.credit.accounts".to_string(),
                    default_user_message: format!(
                        "GL account with id {} does not exist",
                        line.gl_account_id
                    ),
                }
            })?;

            if account.disabled {
                return Err(FineractError::DomainRule {
                    globalisation_code: "error.msg.glJournalEntry.invalid.account.disabled"
                        .to_string(),
                    default_user_message: format!(
                        "Target GL account `{}` ({}) has been disabled",
                        account.name, account.gl_code
                    ),
                });
            }

            if account.account_usage != GLAccountUsage::Detail {
                return Err(FineractError::DomainRule {
                    globalisation_code:
                        "error.msg.glJournalEntry.invalid.debit.or.credit.accounts".to_string(),
                    default_user_message: format!(
                        "Journal entries can only be posted to DETAIL accounts; account `{}` ({}) is a HEADER account",
                        account.name, account.gl_code
                    ),
                });
            }

            if is_manual && !account.manual_entries_allowed {
                return Err(FineractError::DomainRule {
                    globalisation_code:
                        "error.msg.glJournalEntry.invalid.account.manual.adjustments.not.permitted"
                            .to_string(),
                    default_user_message: format!(
                        "Target account `{}` ({}) does not allow manual adjustments",
                        account.name, account.gl_code
                    ),
                });
            }

            match line.entry_type {
                JournalEntryType::Debit => {
                    debit_count += 1;
                    total_debits += line.amount;
                }
                JournalEntryType::Credit => {
                    credit_count += 1;
                    total_credits += line.amount;
                }
            }
        }

        if debit_count == 0 || credit_count == 0 {
            return Err(FineractError::DomainRule {
                globalisation_code: "error.msg.glJournalEntry.invalid.no.debits.or.credits"
                    .to_string(),
                default_user_message:
                    "Journal Entry must have at least one Debit and one Credit".to_string(),
            });
        }

        if total_debits != total_credits || total_debits <= Decimal::ZERO {
            return Err(FineractError::DomainRule {
                globalisation_code: "error.msg.glJournalEntry.invalid.mismatch.debits.credits"
                    .to_string(),
                default_user_message: format!(
                    "Sum of All Debits ({}) must equal the sum of all Credits ({}) for a Journal Entry",
                    total_debits, total_credits
                ),
            });
        }

        Ok(())
    }
}

// ============================================================================
// 5. Accounting Bridge DTOs & Engine (Loans & Savings, Cash & Accrual)
// ============================================================================

/// Supported Loan portfolio transaction types bridged into GL journal entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoanBridgeTransactionType {
    Disbursement,
    Repayment,
    Accrual,
    WriteOff,
    Recovery,
    ChargePayment,
    GoodwillCredit,
}

/// Breakdown of a single charge (fee or penalty) paid/accrued in a transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChargePaymentAllocation {
    pub charge_id: i64,
    pub amount: Decimal,
    pub is_penalty: bool,
}

/// Input event representing a Loan transaction to be converted into balanced GL journal entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanBridgeTransaction {
    pub loan_id: i64,
    pub loan_Product_id: i64,
    pub loan_transaction_id: i64,
    pub office_id: i64,
    pub transaction_date: NaiveDate,
    pub submitted_on_date: NaiveDate,
    pub currency_code: String,
    pub transaction_type: LoanBridgeTransactionType,
    pub amount: Decimal,
    pub principal_portion: Decimal,
    pub interest_portion: Decimal,
    pub fee_charges_portion: Decimal,
    pub penalty_charges_portion: Decimal,
    pub overpayment_portion: Decimal,
    pub payment_type_id: Option<i64>,
    pub payment_details_id: Option<i64>,
    pub is_account_transfer: bool,
    pub charge_allocations: Vec<ChargePaymentAllocation>,
    pub description: Option<String>,
    pub ref_num: Option<String>,
}

impl LoanBridgeTransaction {
    /// Convenience constructor for a simple loan transaction with zeroed components.
    pub fn new(
        loan_id: i64,
        loan_product_id: i64,
        loan_transaction_id: i64,
        office_id: i64,
        transaction_date: NaiveDate,
        currency_code: impl Into<String>,
        transaction_type: LoanBridgeTransactionType,
        amount: Decimal,
    ) -> Self {
        Self {
            loan_id,
            loan_Product_id: loan_product_id,
            loan_transaction_id,
            office_id,
            transaction_date,
            submitted_on_date: transaction_date,
            currency_code: currency_code.into(),
            transaction_type,
            amount,
            principal_portion: Decimal::ZERO,
            interest_portion: Decimal::ZERO,
            fee_charges_portion: Decimal::ZERO,
            penalty_charges_portion: Decimal::ZERO,
            overpayment_portion: Decimal::ZERO,
            payment_type_id: None,
            payment_details_id: None,
            is_account_transfer: false,
            charge_allocations: Vec::new(),
            description: None,
            ref_num: None,
        }
    }
}

/// Supported Savings portfolio transaction types bridged into GL journal entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SavingsBridgeTransactionType {
    Deposit,
    Withdrawal,
    InterestPosting,
    FeeDeduction,
    PenaltyDeduction,
    Accrual,
    WriteOff,
    OverdraftInterest,
}

/// Input event representing a Savings transaction to be converted into balanced GL journal entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsBridgeTransaction {
    pub savings_id: i64,
    pub savings_product_id: i64,
    pub savings_transaction_id: i64,
    pub office_id: i64,
    pub transaction_date: NaiveDate,
    pub submitted_on_date: NaiveDate,
    pub currency_code: String,
    pub transaction_type: SavingsBridgeTransactionType,
    pub amount: Decimal,
    pub overdraft_amount: Decimal,
    pub payment_type_id: Option<i64>,
    pub payment_details_id: Option<i64>,
    pub is_account_transfer: bool,
    pub charge_allocations: Vec<ChargePaymentAllocation>,
    pub description: Option<String>,
    pub ref_num: Option<String>,
}

impl SavingsBridgeTransaction {
    pub fn new(
        savings_id: i64,
        savings_product_id: i64,
        savings_transaction_id: i64,
        office_id: i64,
        transaction_date: NaiveDate,
        currency_code: impl Into<String>,
        transaction_type: SavingsBridgeTransactionType,
        amount: Decimal,
    ) -> Self {
        Self {
            savings_id,
            savings_product_id,
            savings_transaction_id,
            office_id,
            transaction_date,
            submitted_on_date: transaction_date,
            currency_code: currency_code.into(),
            transaction_type,
            amount,
            overdraft_amount: Decimal::ZERO,
            payment_type_id: None,
            payment_details_id: None,
            is_account_transfer: false,
            charge_allocations: Vec::new(),
            description: None,
            ref_num: None,
        }
    }
}

/// Accounting Bridge Engine that translates Loan and Savings transactions into balanced
/// double-entry `JournalEntryRecord` batches under Cash-Based, Periodic Accrual, and Upfront Accrual
/// rules, and handles journal reversals.
pub struct AccountingBridgeEngine;

impl AccountingBridgeEngine {
    /// Generates manual `JournalEntryRecord`s after running full double-entry and manual-entry validations.
    #[allow(clippy::too_many_arguments)]
    pub fn create_manual_journal_entries(
        next_id_start: i64,
        office_id: i64,
        entry_date: NaiveDate,
        submitted_on_date: NaiveDate,
        currency_code: &str,
        transaction_id: Option<String>,
        payment_details_id: Option<i64>,
        ref_num: Option<String>,
        description: Option<String>,
        lines: &[JournalEntryLine],
        closures: &[GLClosure],
        accounts: &HashMap<i64, GLAccount>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        DoubleEntryJournalValidator::validate_balanced_batch_with_options(
            office_id,
            entry_date,
            currency_code,
            lines,
            closures,
            accounts,
            true,
            None,
        )?;

        let tx_id = transaction_id.unwrap_or_else(|| format!("M{}", Uuid::new_v4().simple()));
        let mut records = Vec::with_capacity(lines.len());
        for (idx, line) in lines.iter().enumerate() {
            records.push(JournalEntryRecord {
                id: next_id_start + (idx as i64),
                account_id: line.gl_account_id,
                office_id,
                reversal_id: None,
                currency_code: currency_code.to_string(),
                transaction_id: tx_id.clone(),
                loan_transaction_id: None,
                savings_transaction_id: None,
                client_transaction_id: None,
                share_transaction_id: None,
                reversed: false,
                manual_entry: true,
                entry_date,
                type_enum: line.entry_type.as_i16(),
                amount: line.amount,
                description: line.comments.clone().or_else(|| description.clone()),
                entity_type_enum: None,
                entity_id: None,
                office_running_balance: Decimal::ZERO,
                organization_running_balance: Decimal::ZERO,
                is_running_balance_calculated: false,
                payment_details_id,
                ref_num: ref_num.clone(),
                submitted_on_date,
            });
        }

        Ok(records)
    }

    /// Translates a `LoanBridgeTransaction` into a balanced `Vec<JournalEntryRecord>` according to
    /// the product's `AccountingRuleType` (`CashBased`, `AccrualPeriodic`, or `AccrualUpfront`).
    ///
    /// Mirrors `CashBasedAccountingProcessorForLoan` and `AccrualBasedAccountingProcessorForLoan`.
    pub fn generate_loan_journal_entries(
        next_id_start: i64,
        tx: &LoanBridgeTransaction,
        mapping: &LoanProductAccountMapping,
        closures: &[GLClosure],
        accounts: &HashMap<i64, GLAccount>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        if mapping.accounting_rule == AccountingRuleType::None {
            return Ok(Vec::new());
        }

        let mut lines: Vec<JournalEntryLine> = Vec::new();
        let rule = mapping.accounting_rule;

        match tx.transaction_type {
            LoanBridgeTransactionType::Disbursement => {
                // DR LoanPortfolio, CR FundSource (or TransfersInSuspense if account transfer)
                let loan_portfolio_gl = mapping.resolve_account(
                    LoanProductAccountingParams::LoanPortfolio,
                    tx.payment_type_id,
                    None,
                )?;
                let credit_param = if tx.is_account_transfer {
                    LoanProductAccountingParams::TransfersInSuspense
                } else {
                    LoanProductAccountingParams::FundSource
                };
                let fund_source_gl =
                    mapping.resolve_account(credit_param, tx.payment_type_id, None)?;

                lines.push(JournalEntryLine::debit(
                    loan_portfolio_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
                lines.push(JournalEntryLine::credit(
                    fund_source_gl,
                    tx.amount,
                    tx.description.clone(),
                ));

                // Under Upfront Accrual, if upfront interest/fees/penalties are specified on disbursement,
                // recognise them immediately (DR Receivable, CR Income).
                if rule == AccountingRuleType::AccrualUpfront {
                    Self::append_loan_accrual_lines(&mut lines, tx, mapping)?;
                }
            }

            LoanBridgeTransactionType::Repayment => {
                // DR FundSource (or TransfersInSuspense) for total amount repaid
                // Credits depend on Cash vs Accrual:
                // - Principal -> LoanPortfolio
                // - Interest -> InterestOnLoans (Cash) OR InterestReceivable (Accrual)
                // - Fees -> IncomeFromFees (Cash) OR FeesReceivable (Accrual)
                // - Penalties -> IncomeFromPenalties (Cash) OR PenaltiesReceivable (Accrual)
                // - Overpayment -> OverpaymentLiability
                let mut total_credited = Decimal::ZERO;

                if tx.principal_portion > Decimal::ZERO {
                    let portfolio_gl = mapping.resolve_account(
                        LoanProductAccountingParams::LoanPortfolio,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        portfolio_gl,
                        tx.principal_portion,
                        tx.description.clone(),
                    ));
                    total_credited += tx.principal_portion;
                }

                if tx.interest_portion > Decimal::ZERO {
                    let interest_param = if rule.is_accrual_based() {
                        LoanProductAccountingParams::InterestReceivable
                    } else {
                        LoanProductAccountingParams::InterestOnLoans
                    };
                    let interest_gl =
                        mapping.resolve_account(interest_param, tx.payment_type_id, None)?;
                    lines.push(JournalEntryLine::credit(
                        interest_gl,
                        tx.interest_portion,
                        tx.description.clone(),
                    ));
                    total_credited += tx.interest_portion;
                }

                if tx.fee_charges_portion > Decimal::ZERO {
                    if rule.is_accrual_based() {
                        let fees_receivable_gl = mapping.resolve_account(
                            LoanProductAccountingParams::FeesReceivable,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            fees_receivable_gl,
                            tx.fee_charges_portion,
                            tx.description.clone(),
                        ));
                    } else {
                        Self::append_fee_credits(
                            &mut lines,
                            tx.fee_charges_portion,
                            &tx.charge_allocations,
                            false,
                            mapping,
                            tx.payment_type_id,
                            tx.description.clone(),
                        )?;
                    }
                    total_credited += tx.fee_charges_portion;
                }

                if tx.penalty_charges_portion > Decimal::ZERO {
                    if rule.is_accrual_based() {
                        let penalties_receivable_gl = mapping.resolve_account(
                            LoanProductAccountingParams::PenaltiesReceivable,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            penalties_receivable_gl,
                            tx.penalty_charges_portion,
                            tx.description.clone(),
                        ));
                    } else {
                        Self::append_fee_credits(
                            &mut lines,
                            tx.penalty_charges_portion,
                            &tx.charge_allocations,
                            true,
                            mapping,
                            tx.payment_type_id,
                            tx.description.clone(),
                        )?;
                    }
                    total_credited += tx.penalty_charges_portion;
                }

                if tx.overpayment_portion > Decimal::ZERO {
                    let overpayment_gl = mapping.resolve_account(
                        LoanProductAccountingParams::OverpaymentLiability,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        overpayment_gl,
                        tx.overpayment_portion,
                        tx.description.clone(),
                    ));
                    total_credited += tx.overpayment_portion;
                }

                let debit_amount = if total_credited > Decimal::ZERO {
                    total_credited
                } else {
                    tx.amount
                };
                let debit_param = if tx.is_account_transfer {
                    LoanProductAccountingParams::TransfersInSuspense
                } else {
                    LoanProductAccountingParams::FundSource
                };
                let fund_source_gl =
                    mapping.resolve_account(debit_param, tx.payment_type_id, None)?;
                lines.insert(
                    0,
                    JournalEntryLine::debit(fund_source_gl, debit_amount, tx.description.clone()),
                );
            }

            LoanBridgeTransactionType::Accrual => {
                // Only valid for Accrual-based accounting rules
                if !rule.is_accrual_based() {
                    return Ok(Vec::new());
                }
                Self::append_loan_accrual_lines(&mut lines, tx, mapping)?;
            }

            LoanBridgeTransactionType::WriteOff => {
                // Under Cash-based: only principal is written off (DR LossesWrittenOff, CR LoanPortfolio)
                // Under Accrual-based: principal, interest receivable, fees receivable, penalties receivable
                // are credited and total is debited to LossesWrittenOff.
                let write_off_gl = mapping.resolve_account(
                    LoanProductAccountingParams::LossesWrittenOff,
                    tx.payment_type_id,
                    None,
                )?;

                if rule.is_cash_based() {
                    let principal_amt = if tx.principal_portion > Decimal::ZERO {
                        tx.principal_portion
                    } else {
                        tx.amount
                    };
                    let portfolio_gl = mapping.resolve_account(
                        LoanProductAccountingParams::LoanPortfolio,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::debit(
                        write_off_gl,
                        principal_amt,
                        tx.description.clone(),
                    ));
                    lines.push(JournalEntryLine::credit(
                        portfolio_gl,
                        principal_amt,
                        tx.description.clone(),
                    ));
                } else {
                    let mut total_write_off = Decimal::ZERO;
                    if tx.principal_portion > Decimal::ZERO {
                        let portfolio_gl = mapping.resolve_account(
                            LoanProductAccountingParams::LoanPortfolio,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            portfolio_gl,
                            tx.principal_portion,
                            tx.description.clone(),
                        ));
                        total_write_off += tx.principal_portion;
                    }
                    if tx.interest_portion > Decimal::ZERO {
                        let interest_rec_gl = mapping.resolve_account(
                            LoanProductAccountingParams::InterestReceivable,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            interest_rec_gl,
                            tx.interest_portion,
                            tx.description.clone(),
                        ));
                        total_write_off += tx.interest_portion;
                    }
                    if tx.fee_charges_portion > Decimal::ZERO {
                        let fee_rec_gl = mapping.resolve_account(
                            LoanProductAccountingParams::FeesReceivable,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            fee_rec_gl,
                            tx.fee_charges_portion,
                            tx.description.clone(),
                        ));
                        total_write_off += tx.fee_charges_portion;
                    }
                    if tx.penalty_charges_portion > Decimal::ZERO {
                        let pen_rec_gl = mapping.resolve_account(
                            LoanProductAccountingParams::PenaltiesReceivable,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            pen_rec_gl,
                            tx.penalty_charges_portion,
                            tx.description.clone(),
                        ));
                        total_write_off += tx.penalty_charges_portion;
                    }
                    if total_write_off == Decimal::ZERO && tx.amount > Decimal::ZERO {
                        let portfolio_gl = mapping.resolve_account(
                            LoanProductAccountingParams::LoanPortfolio,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            portfolio_gl,
                            tx.amount,
                            tx.description.clone(),
                        ));
                        total_write_off = tx.amount;
                    }
                    lines.insert(
                        0,
                        JournalEntryLine::debit(
                            write_off_gl,
                            total_write_off,
                            tx.description.clone(),
                        ),
                    );
                }
            }

            LoanBridgeTransactionType::Recovery => {
                // DR FundSource (or TransfersInSuspense), CR IncomeFromRecovery
                let debit_param = if tx.is_account_transfer {
                    LoanProductAccountingParams::TransfersInSuspense
                } else {
                    LoanProductAccountingParams::FundSource
                };
                let fund_source_gl =
                    mapping.resolve_account(debit_param, tx.payment_type_id, None)?;
                let recovery_gl = mapping.resolve_account(
                    LoanProductAccountingParams::IncomeFromRecovery,
                    tx.payment_type_id,
                    None,
                )?;
                lines.push(JournalEntryLine::debit(
                    fund_source_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
                lines.push(JournalEntryLine::credit(
                    recovery_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
            }

            LoanBridgeTransactionType::ChargePayment => {
                // DR FundSource (or TransfersInSuspense)
                // CR Fees/Penalties (Income under Cash, Receivable under Accrual)
                let debit_param = if tx.is_account_transfer {
                    LoanProductAccountingParams::TransfersInSuspense
                } else {
                    LoanProductAccountingParams::FundSource
                };
                let fund_source_gl =
                    mapping.resolve_account(debit_param, tx.payment_type_id, None)?;

                let mut total_charges = Decimal::ZERO;
                if tx.fee_charges_portion > Decimal::ZERO {
                    if rule.is_accrual_based() {
                        let fee_rec_gl = mapping.resolve_account(
                            LoanProductAccountingParams::FeesReceivable,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            fee_rec_gl,
                            tx.fee_charges_portion,
                            tx.description.clone(),
                        ));
                    } else {
                        Self::append_fee_credits(
                            &mut lines,
                            tx.fee_charges_portion,
                            &tx.charge_allocations,
                            false,
                            mapping,
                            tx.payment_type_id,
                            tx.description.clone(),
                        )?;
                    }
                    total_charges += tx.fee_charges_portion;
                }

                if tx.penalty_charges_portion > Decimal::ZERO {
                    if rule.is_accrual_based() {
                        let pen_rec_gl = mapping.resolve_account(
                            LoanProductAccountingParams::PenaltiesReceivable,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            pen_rec_gl,
                            tx.penalty_charges_portion,
                            tx.description.clone(),
                        ));
                    } else {
                        Self::append_fee_credits(
                            &mut lines,
                            tx.penalty_charges_portion,
                            &tx.charge_allocations,
                            true,
                            mapping,
                            tx.payment_type_id,
                            tx.description.clone(),
                        )?;
                    }
                    total_charges += tx.penalty_charges_portion;
                }

                if total_charges == Decimal::ZERO && tx.amount > Decimal::ZERO {
                    let fee_gl = if rule.is_accrual_based() {
                        mapping.resolve_account(
                            LoanProductAccountingParams::FeesReceivable,
                            tx.payment_type_id,
                            None,
                        )?
                    } else {
                        mapping.resolve_account(
                            LoanProductAccountingParams::IncomeFromFees,
                            tx.payment_type_id,
                            None,
                        )?
                    };
                    lines.push(JournalEntryLine::credit(
                        fee_gl,
                        tx.amount,
                        tx.description.clone(),
                    ));
                    total_charges = tx.amount;
                }

                lines.insert(
                    0,
                    JournalEntryLine::debit(fund_source_gl, total_charges, tx.description.clone()),
                );
            }

            LoanBridgeTransactionType::GoodwillCredit => {
                // DR GoodwillCredit (Expense), CR LoanPortfolio / Receivables / Overpayment
                let goodwill_gl = mapping.resolve_account(
                    LoanProductAccountingParams::GoodwillCredit,
                    tx.payment_type_id,
                    None,
                )?;
                let mut total_credited = Decimal::ZERO;
                if tx.principal_portion > Decimal::ZERO {
                    let portfolio_gl = mapping.resolve_account(
                        LoanProductAccountingParams::LoanPortfolio,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        portfolio_gl,
                        tx.principal_portion,
                        tx.description.clone(),
                    ));
                    total_credited += tx.principal_portion;
                }
                if tx.interest_portion > Decimal::ZERO {
                    let interest_param = if rule.is_accrual_based() {
                        LoanProductAccountingParams::InterestReceivable
                    } else {
                        LoanProductAccountingParams::InterestOnLoans
                    };
                    let interest_gl =
                        mapping.resolve_account(interest_param, tx.payment_type_id, None)?;
                    lines.push(JournalEntryLine::credit(
                        interest_gl,
                        tx.interest_portion,
                        tx.description.clone(),
                    ));
                    total_credited += tx.interest_portion;
                }
                if total_credited == Decimal::ZERO && tx.amount > Decimal::ZERO {
                    let portfolio_gl = mapping.resolve_account(
                        LoanProductAccountingParams::LoanPortfolio,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        portfolio_gl,
                        tx.amount,
                        tx.description.clone(),
                    ));
                    total_credited = tx.amount;
                }
                lines.insert(
                    0,
                    JournalEntryLine::debit(goodwill_gl, total_credited, tx.description.clone()),
                );
            }
        }

        if lines.is_empty() {
            return Ok(Vec::new());
        }

        // Consolidate identical (gl_account_id, entry_type) lines in the same manner as
        // Fineract's AccountingProcessorHelper.createCreditJournalEntryOrReversalForLoan
        let consolidated = Self::consolidate_lines(lines);

        DoubleEntryJournalValidator::validate_balanced_batch(
            tx.office_id,
            tx.transaction_date,
            &tx.currency_code,
            &consolidated,
            closures,
            accounts,
        )?;

        let tx_key = format!("L{}", tx.loan_transaction_id);
        let mut records = Vec::with_capacity(consolidated.len());
        for (idx, line) in consolidated.into_iter().enumerate() {
            records.push(JournalEntryRecord {
                id: next_id_start + (idx as i64),
                account_id: line.gl_account_id,
                office_id: tx.office_id,
                reversal_id: None,
                currency_code: tx.currency_code.clone(),
                transaction_id: tx_key.clone(),
                loan_transaction_id: Some(tx.loan_transaction_id),
                savings_transaction_id: None,
                client_transaction_id: None,
                share_transaction_id: None,
                reversed: false,
                manual_entry: false,
                entry_date: tx.transaction_date,
                type_enum: line.entry_type.as_i16(),
                amount: line.amount,
                description: line.comments.or_else(|| tx.description.clone()),
                entity_type_enum: Some(PortfolioProductType::Loan.as_i16()),
                entity_id: Some(tx.loan_id),
                office_running_balance: Decimal::ZERO,
                organization_running_balance: Decimal::ZERO,
                is_running_balance_calculated: false,
                payment_details_id: tx.payment_details_id,
                ref_num: tx.ref_num.clone(),
                submitted_on_date: tx.submitted_on_date,
            });
        }

        Ok(records)
    }

    /// Translates a `SavingsBridgeTransaction` into a balanced `Vec<JournalEntryRecord>` according
    /// to the savings product's `AccountingRuleType`.
    ///
    /// Mirrors `CashBasedAccountingProcessorForSavings` and `AccrualBasedAccountingProcessorForSavings`.
    pub fn generate_savings_journal_entries(
        next_id_start: i64,
        tx: &SavingsBridgeTransaction,
        mapping: &SavingsProductAccountMapping,
        closures: &[GLClosure],
        accounts: &HashMap<i64, GLAccount>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        if mapping.accounting_rule == AccountingRuleType::None {
            return Ok(Vec::new());
        }

        let mut lines: Vec<JournalEntryLine> = Vec::new();
        let rule = mapping.accounting_rule;

        match tx.transaction_type {
            SavingsBridgeTransactionType::Deposit => {
                // DR SavingsReference (or TransfersInSuspense), CR SavingsControl (and OverdraftPortfolioControl if overdraft recovery)
                let debit_param = if tx.is_account_transfer {
                    SavingsProductAccountingParams::TransfersInSuspense
                } else {
                    SavingsProductAccountingParams::SavingsReference
                };
                let savings_ref_gl =
                    mapping.resolve_account(debit_param, tx.payment_type_id, None)?;
                lines.push(JournalEntryLine::debit(
                    savings_ref_gl,
                    tx.amount,
                    tx.description.clone(),
                ));

                if tx.overdraft_amount > Decimal::ZERO {
                    let overdraft_gl = mapping.resolve_account(
                        SavingsProductAccountingParams::OverdraftPortfolioControl,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        overdraft_gl,
                        tx.overdraft_amount,
                        tx.description.clone(),
                    ));
                    let normal_portion = tx.amount - tx.overdraft_amount;
                    if normal_portion > Decimal::ZERO {
                        let control_gl = mapping.resolve_account(
                            SavingsProductAccountingParams::SavingsControl,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::credit(
                            control_gl,
                            normal_portion,
                            tx.description.clone(),
                        ));
                    }
                } else {
                    let control_gl = mapping.resolve_account(
                        SavingsProductAccountingParams::SavingsControl,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        control_gl,
                        tx.amount,
                        tx.description.clone(),
                    ));
                }
            }

            SavingsBridgeTransactionType::Withdrawal => {
                // DR SavingsControl (and OverdraftPortfolioControl if overdraft), CR SavingsReference (or TransfersInSuspense)
                if tx.overdraft_amount > Decimal::ZERO {
                    let overdraft_gl = mapping.resolve_account(
                        SavingsProductAccountingParams::OverdraftPortfolioControl,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::debit(
                        overdraft_gl,
                        tx.overdraft_amount,
                        tx.description.clone(),
                    ));
                    let normal_portion = tx.amount - tx.overdraft_amount;
                    if normal_portion > Decimal::ZERO {
                        let control_gl = mapping.resolve_account(
                            SavingsProductAccountingParams::SavingsControl,
                            tx.payment_type_id,
                            None,
                        )?;
                        lines.push(JournalEntryLine::debit(
                            control_gl,
                            normal_portion,
                            tx.description.clone(),
                        ));
                    }
                } else {
                    let control_gl = mapping.resolve_account(
                        SavingsProductAccountingParams::SavingsControl,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::debit(
                        control_gl,
                        tx.amount,
                        tx.description.clone(),
                    ));
                }

                let credit_param = if tx.is_account_transfer {
                    SavingsProductAccountingParams::TransfersInSuspense
                } else {
                    SavingsProductAccountingParams::SavingsReference
                };
                let savings_ref_gl =
                    mapping.resolve_account(credit_param, tx.payment_type_id, None)?;
                lines.push(JournalEntryLine::credit(
                    savings_ref_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
            }

            SavingsBridgeTransactionType::InterestPosting => {
                // Under Cash-based: DR InterestOnSavings, CR SavingsControl
                // Under Periodic Accrual: DR InterestPayable (if configured, else InterestOnSavings), CR SavingsControl
                let debit_gl = if rule.is_accrual_based() {
                    mapping
                        .resolve_account(
                            SavingsProductAccountingParams::InterestPayable,
                            tx.payment_type_id,
                            None,
                        )
                        .or_else(|_| {
                            mapping.resolve_account(
                                SavingsProductAccountingParams::InterestOnSavings,
                                tx.payment_type_id,
                                None,
                            )
                        })?
                } else {
                    mapping.resolve_account(
                        SavingsProductAccountingParams::InterestOnSavings,
                        tx.payment_type_id,
                        None,
                    )?
                };
                let control_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::SavingsControl,
                    tx.payment_type_id,
                    None,
                )?;
                lines.push(JournalEntryLine::debit(
                    debit_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
                lines.push(JournalEntryLine::credit(
                    control_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
            }

            SavingsBridgeTransactionType::FeeDeduction => {
                // DR SavingsControl, CR IncomeFromFees (Cash) or FeesReceivable (Accrual if configured)
                let control_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::SavingsControl,
                    tx.payment_type_id,
                    None,
                )?;
                lines.push(JournalEntryLine::debit(
                    control_gl,
                    tx.amount,
                    tx.description.clone(),
                ));

                if rule.is_accrual_based()
                    && mapping
                        .accounts
                        .contains_key(&SavingsProductAccountingParams::FeesReceivable)
                {
                    let fee_rec_gl = mapping.resolve_account(
                        SavingsProductAccountingParams::FeesReceivable,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        fee_rec_gl,
                        tx.amount,
                        tx.description.clone(),
                    ));
                } else if !tx.charge_allocations.is_empty() {
                    for alloc in &tx.charge_allocations {
                        if alloc.amount > Decimal::ZERO {
                            let gl_id = mapping.resolve_account(
                                SavingsProductAccountingParams::IncomeFromFees,
                                tx.payment_type_id,
                                Some(alloc.charge_id),
                            )?;
                            lines.push(JournalEntryLine::credit(
                                gl_id,
                                alloc.amount,
                                tx.description.clone(),
                            ));
                        }
                    }
                } else {
                    let fee_gl = mapping.resolve_account(
                        SavingsProductAccountingParams::IncomeFromFees,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        fee_gl,
                        tx.amount,
                        tx.description.clone(),
                    ));
                }
            }

            SavingsBridgeTransactionType::PenaltyDeduction => {
                // DR SavingsControl, CR IncomeFromPenalties (or PenaltiesReceivable under Accrual)
                let control_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::SavingsControl,
                    tx.payment_type_id,
                    None,
                )?;
                lines.push(JournalEntryLine::debit(
                    control_gl,
                    tx.amount,
                    tx.description.clone(),
                ));

                if rule.is_accrual_based()
                    && mapping
                        .accounts
                        .contains_key(&SavingsProductAccountingParams::PenaltiesReceivable)
                {
                    let pen_rec_gl = mapping.resolve_account(
                        SavingsProductAccountingParams::PenaltiesReceivable,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        pen_rec_gl,
                        tx.amount,
                        tx.description.clone(),
                    ));
                } else {
                    let pen_gl = mapping.resolve_account(
                        SavingsProductAccountingParams::IncomeFromPenalties,
                        tx.payment_type_id,
                        None,
                    )?;
                    lines.push(JournalEntryLine::credit(
                        pen_gl,
                        tx.amount,
                        tx.description.clone(),
                    ));
                }
            }

            SavingsBridgeTransactionType::Accrual => {
                if !rule.is_accrual_based() {
                    return Ok(Vec::new());
                }
                // DR InterestOnSavings, CR InterestPayable
                let expense_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::InterestOnSavings,
                    tx.payment_type_id,
                    None,
                )?;
                let payable_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::InterestPayable,
                    tx.payment_type_id,
                    None,
                )?;
                lines.push(JournalEntryLine::debit(
                    expense_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
                lines.push(JournalEntryLine::credit(
                    payable_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
            }

            SavingsBridgeTransactionType::WriteOff => {
                // DR LossesWrittenOff, CR OverdraftPortfolioControl
                let loss_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::LossesWrittenOff,
                    tx.payment_type_id,
                    None,
                )?;
                let overdraft_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::OverdraftPortfolioControl,
                    tx.payment_type_id,
                    None,
                )?;
                lines.push(JournalEntryLine::debit(
                    loss_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
                lines.push(JournalEntryLine::credit(
                    overdraft_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
            }

            SavingsBridgeTransactionType::OverdraftInterest => {
                // DR OverdraftPortfolioControl, CR IncomeFromInterest
                let overdraft_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::OverdraftPortfolioControl,
                    tx.payment_type_id,
                    None,
                )?;
                let income_gl = mapping.resolve_account(
                    SavingsProductAccountingParams::IncomeFromInterest,
                    tx.payment_type_id,
                    None,
                )?;
                lines.push(JournalEntryLine::debit(
                    overdraft_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
                lines.push(JournalEntryLine::credit(
                    income_gl,
                    tx.amount,
                    tx.description.clone(),
                ));
            }
        }

        if lines.is_empty() {
            return Ok(Vec::new());
        }

        let consolidated = Self::consolidate_lines(lines);

        DoubleEntryJournalValidator::validate_balanced_batch(
            tx.office_id,
            tx.transaction_date,
            &tx.currency_code,
            &consolidated,
            closures,
            accounts,
        )?;

        let tx_key = format!("S{}", tx.savings_transaction_id);
        let mut records = Vec::with_capacity(consolidated.len());
        for (idx, line) in consolidated.into_iter().enumerate() {
            records.push(JournalEntryRecord {
                id: next_id_start + (idx as i64),
                account_id: line.gl_account_id,
                office_id: tx.office_id,
                reversal_id: None,
                currency_code: tx.currency_code.clone(),
                transaction_id: tx_key.clone(),
                loan_transaction_id: None,
                savings_transaction_id: Some(tx.savings_transaction_id),
                client_transaction_id: None,
                share_transaction_id: None,
                reversed: false,
                manual_entry: false,
                entry_date: tx.transaction_date,
                type_enum: line.entry_type.as_i16(),
                amount: line.amount,
                description: line.comments.or_else(|| tx.description.clone()),
                entity_type_enum: Some(PortfolioProductType::Saving.as_i16()),
                entity_id: Some(tx.savings_id),
                office_running_balance: Decimal::ZERO,
                organization_running_balance: Decimal::ZERO,
                is_running_balance_calculated: false,
                payment_details_id: tx.payment_details_id,
                ref_num: tx.ref_num.clone(),
                submitted_on_date: tx.submitted_on_date,
            });
        }

        Ok(records)
    }

    /// Reverses an existing batch of `JournalEntryRecord`s by:
    /// 1. Validating that none of the entries are already reversed and that none of their office
    ///    entry dates fall on or before an active `GLClosure`.
    /// 2. Creating opposite-type (`Debit <-> Credit`) reversal `JournalEntryRecord`s with `reversed = true`.
    /// 3. Mutating each original entry to set `reversed = true` and `reversal_id = Some(reversal_entry.id)`.
    ///
    /// Mirrors `JournalEntryWritePlatformServiceJpaRepositoryImpl.revertJournalEntry`.
    pub fn reverse_journal_entries(
        next_id_start: i64,
        original_entries: &mut [JournalEntryRecord],
        reversal_date: NaiveDate,
        reversal_comment: Option<String>,
        closures: &[GLClosure],
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        if original_entries.is_empty() {
            return Err(FineractError::DomainRule {
                globalisation_code: "error.msg.glJournalEntry.transaction.id.not.found"
                    .to_string(),
                default_user_message: "No journal entries found to reverse".to_string(),
            });
        }

        for orig in original_entries.iter() {
            if orig.reversed {
                return Err(FineractError::DomainRule {
                    globalisation_code: "error.msg.glJournalEntry.already.reversed".to_string(),
                    default_user_message: format!(
                        "Journal entries for transaction `{}` have already been reversed",
                        orig.transaction_id
                    ),
                });
            }

            let latest_closure = closures
                .iter()
                .filter(|c| c.office_id == orig.office_id && !c.is_deleted)
                .map(|c| c.closing_date)
                .max();
            if let Some(closing_date) = latest_closure {
                if orig.entry_date <= closing_date || reversal_date <= closing_date {
                    return Err(FineractError::DomainRule {
                        globalisation_code: "error.msg.glJournalEntry.invalid.accounting.closed"
                            .to_string(),
                        default_user_message: format!(
                            "Cannot reverse journal entry on `{}` prior to or on office closing date `{}`",
                            orig.entry_date, closing_date
                        ),
                    });
                }
            }
        }

        let mut reversals = Vec::with_capacity(original_entries.len());
        for (idx, orig) in original_entries.iter_mut().enumerate() {
            let rev_id = next_id_start + (idx as i64);
            let orig_type = JournalEntryType::from_i16(orig.type_enum).ok_or_else(|| {
                FineractError::DomainRule {
                    globalisation_code: "error.msg.glJournalEntry.invalid.type".to_string(),
                    default_user_message: format!("Invalid journal entry type {}", orig.type_enum),
                }
            })?;
            let opposite_type = orig_type.opposite();

            let description = reversal_comment.clone().or_else(|| {
                Some(format!(
                    "Reversal entry for Journal Entry with Entry Id :{} and transaction Id {}",
                    orig.id, orig.transaction_id
                ))
            });

            let rev_record = JournalEntryRecord {
                id: rev_id,
                account_id: orig.account_id,
                office_id: orig.office_id,
                reversal_id: None,
                currency_code: orig.currency_code.clone(),
                transaction_id: orig.transaction_id.clone(),
                loan_transaction_id: orig.loan_transaction_id,
                savings_transaction_id: orig.savings_transaction_id,
                client_transaction_id: orig.client_transaction_id,
                share_transaction_id: orig.share_transaction_id,
                reversed: true,
                manual_entry: orig.manual_entry,
                entry_date: reversal_date,
                type_enum: opposite_type.as_i16(),
                amount: orig.amount,
                description,
                entity_type_enum: orig.entity_type_enum,
                entity_id: orig.entity_id,
                office_running_balance: Decimal::ZERO,
                organization_running_balance: Decimal::ZERO,
                is_running_balance_calculated: false,
                payment_details_id: orig.payment_details_id,
                ref_num: orig.ref_num.clone(),
                submitted_on_date: reversal_date,
            };

            orig.reversed = true;
            orig.reversal_id = Some(rev_id);
            reversals.push(rev_record);
        }

        Ok(reversals)
    }

    // ---- Internal Helpers ----

    fn append_loan_accrual_lines(
        lines: &mut Vec<JournalEntryLine>,
        tx: &LoanBridgeTransaction,
        mapping: &LoanProductAccountMapping,
    ) -> Result<(), FineractError> {
        if tx.interest_portion > Decimal::ZERO {
            let rec_gl = mapping.resolve_account(
                LoanProductAccountingParams::InterestReceivable,
                tx.payment_type_id,
                None,
            )?;
            let inc_gl = mapping.resolve_account(
                LoanProductAccountingParams::InterestOnLoans,
                tx.payment_type_id,
                None,
            )?;
            lines.push(JournalEntryLine::debit(
                rec_gl,
                tx.interest_portion,
                tx.description.clone(),
            ));
            lines.push(JournalEntryLine::credit(
                inc_gl,
                tx.interest_portion,
                tx.description.clone(),
            ));
        }

        if tx.fee_charges_portion > Decimal::ZERO {
            let rec_gl = mapping.resolve_account(
                LoanProductAccountingParams::FeesReceivable,
                tx.payment_type_id,
                None,
            )?;
            lines.push(JournalEntryLine::debit(
                rec_gl,
                tx.fee_charges_portion,
                tx.description.clone(),
            ));
            Self::append_fee_credits(
                lines,
                tx.fee_charges_portion,
                &tx.charge_allocations,
                false,
                mapping,
                tx.payment_type_id,
                tx.description.clone(),
            )?;
        }

        if tx.penalty_charges_portion > Decimal::ZERO {
            let rec_gl = mapping.resolve_account(
                LoanProductAccountingParams::PenaltiesReceivable,
                tx.payment_type_id,
                None,
            )?;
            lines.push(JournalEntryLine::debit(
                rec_gl,
                tx.penalty_charges_portion,
                tx.description.clone(),
            ));
            Self::append_fee_credits(
                lines,
                tx.penalty_charges_portion,
                &tx.charge_allocations,
                true,
                mapping,
                tx.payment_type_id,
                tx.description.clone(),
            )?;
        }

        Ok(())
    }

    fn append_fee_credits(
        lines: &mut Vec<JournalEntryLine>,
        total_portion: Decimal,
        allocations: &[ChargePaymentAllocation],
        is_penalty: bool,
        mapping: &LoanProductAccountMapping,
        payment_type_id: Option<i64>,
        description: Option<String>,
    ) -> Result<(), FineractError> {
        let param = if is_penalty {
            LoanProductAccountingParams::IncomeFromPenalties
        } else {
            LoanProductAccountingParams::IncomeFromFees
        };

        let matching_allocs: Vec<&ChargePaymentAllocation> = allocations
            .iter()
            .filter(|a| a.is_penalty == is_penalty && a.amount > Decimal::ZERO)
            .collect();

        let mut allocated_sum = Decimal::ZERO;
        for alloc in matching_allocs {
            let gl_id = mapping.resolve_account(param, payment_type_id, Some(alloc.charge_id))?;
            lines.push(JournalEntryLine::credit(
                gl_id,
                alloc.amount,
                description.clone(),
            ));
            allocated_sum += alloc.amount;
        }

        let remainder = total_portion - allocated_sum;
        if remainder > Decimal::ZERO {
            let default_gl = mapping.resolve_account(param, payment_type_id, None)?;
            lines.push(JournalEntryLine::credit(
                default_gl,
                remainder,
                description,
            ));
        }

        Ok(())
    }

    fn consolidate_lines(lines: Vec<JournalEntryLine>) -> Vec<JournalEntryLine> {
        let mut merged: BTreeMap<(i64, i16), JournalEntryLine> = BTreeMap::new();
        let mut order: Vec<(i64, i16)> = Vec::new();
        for line in lines {
            let key = (line.gl_account_id, line.entry_type.as_i16());
            if let Some(existing) = merged.get_mut(&key) {
                existing.amount += line.amount;
            } else {
                order.push(key);
                merged.insert(key, line);
            }
        }
        order
            .into_iter()
            .filter_map(|k| merged.remove(&k))
            .collect()
    }
}

// ============================================================================
// 6. Running Balance Calculator (`JournalEntryRunningBalanceUpdateServiceImpl`)
// ============================================================================

/// Computes `office_running_balance` and `organization_running_balance` across journal entries
/// ordered by `(entry_date, id)`.
///
/// Mirrors `org.apache.fineract.accounting.journalentry.service.JournalEntryRunningBalanceUpdateServiceImpl`:
/// - `GLAccountType::Asset` and `GLAccountType::Expense` increase on `Debit` (`+amount`) and decrease on `Credit` (`-amount`).
/// - `GLAccountType::Liability`, `GLAccountType::Equity`, and `GLAccountType::Income` increase on `Credit` (`+amount`) and decrease on `Debit` (`-amount`).
pub struct RunningBalanceCalculator;

impl RunningBalanceCalculator {
    /// Returns the signed delta for a journal entry given its `GLAccountType` and `JournalEntryType`.
    #[inline]
    pub const fn signed_balance_delta(
        account_type: GLAccountType,
        entry_type: JournalEntryType,
        amount: Decimal,
    ) -> Decimal {
        match (account_type, entry_type) {
            (GLAccountType::Asset | GLAccountType::Expense, JournalEntryType::Debit) => amount,
            (GLAccountType::Asset | GLAccountType::Expense, JournalEntryType::Credit) => -amount,
            (
                GLAccountType::Liability | GLAccountType::Equity | GLAccountType::Income,
                JournalEntryType::Credit,
            ) => amount,
            (
                GLAccountType::Liability | GLAccountType::Equity | GLAccountType::Income,
                JournalEntryType::Debit,
            ) => -amount,
        }
    }

    /// Updates `office_running_balance`, `organization_running_balance`, and
    /// `is_running_balance_calculated = true` on a slice of `JournalEntryRecord`s in chronological
    /// `(entry_date, id)` order.
    pub fn calculate_running_balances(
        entries: &mut [JournalEntryRecord],
        accounts: &HashMap<i64, GLAccount>,
    ) -> Result<(), FineractError> {
        // Build index permutation sorted by (entry_date, id)
        let mut indices: Vec<usize> = (0..entries.len()).collect();
        indices.sort_by_key(|&i| (entries[i].entry_date, entries[i].id));

        // Track (office_id, account_id) -> Decimal and account_id -> Decimal
        let mut office_balances: HashMap<(i64, i64), Decimal> = HashMap::new();
        let mut org_balances: HashMap<i64, Decimal> = HashMap::new();

        for idx in indices {
            let entry = &mut entries[idx];
            let account = accounts.get(&entry.account_id).ok_or_else(|| {
                FineractError::NotFound {
                    globalisation_code: "error.msg.glaccount.id.invalid".to_string(),
                    default_user_message: format!(
                        "GL Account with id {} does not exist",
                        entry.account_id
                    ),
                }
            })?;
            let entry_type = JournalEntryType::from_i16(entry.type_enum).ok_or_else(|| {
                FineractError::DomainRule {
                    globalisation_code: "error.msg.glJournalEntry.invalid.type".to_string(),
                    default_user_message: format!(
                        "Invalid journal entry type {}",
                        entry.type_enum
                    ),
                }
            })?;

            // Note: In Fineract's running balance job (`JournalEntryRunningBalanceUpdateServiceImpl`),
            // unreversed entries (`reversed == false`) contribute to running balances, OR if both
            // original and reversal entries are present, their opposite signs naturally cancel out.
            // When `reversed == true` and `reversal_id.is_some()` paired with its reversal entry,
            // applying signed_balance_delta across all entries nets to zero at the reversal date while
            // preserving historical running balance on original entry_date.
            let delta =
                Self::signed_balance_delta(account.account_type, entry_type, entry.amount);

            let office_bal = office_balances
                .entry((entry.office_id, entry.account_id))
                .or_insert(Decimal::ZERO);
            *office_bal += delta;
            entry.office_running_balance = *office_bal;

            let org_bal = org_balances
                .entry(entry.account_id)
                .or_insert(Decimal::ZERO);
            *org_bal += delta;
            entry.organization_running_balance = *org_bal;
            entry.is_running_balance_calculated = true;
        }

        Ok(())
    }
}

// ============================================================================
// 7. Repository & Command Handlers (Dual-DB Postgres + MySQL & In-Memory)
// ============================================================================

/// Repository abstraction for `acc_gl_journal_entry`.
#[async_trait]
pub trait JournalEntryRepository: Send + Sync {
    async fn save_batch(
        &self,
        entries: Vec<JournalEntryRecord>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError>;

    async fn find_by_transaction_id(
        &self,
        transaction_id: &str,
    ) -> Result<Vec<JournalEntryRecord>, FineractError>;

    async fn find_by_office(
        &self,
        office_id: Option<i64>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError>;

    async fn update_batch(
        &self,
        entries: &[JournalEntryRecord],
    ) -> Result<(), FineractError>;
}

/// Dual-DB (`PostgreSQL` + `MySQL`) SQL repository for `acc_gl_journal_entry`.
pub struct DualDbJournalEntryRepository {
    pool: Arc<DualDbPool>,
}

impl DualDbJournalEntryRepository {
    pub fn new(pool: Arc<DualDbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl JournalEntryRepository for DualDbJournalEntryRepository {
    async fn save_batch(
        &self,
        mut entries: Vec<JournalEntryRecord>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        let sql = "INSERT INTO acc_gl_journal_entry (\
            account_id, office_id, reversal_id, currency_code, transaction_id, \
            loan_transaction_id, savings_transaction_id, client_transaction_id, share_transaction_id, \
            reversed, manual_entry, entry_date, type_enum, amount, description, \
            entity_type_enum, entity_id, office_running_balance, organization_running_balance, \
            is_running_balance_calculated, payment_details_id, ref_num, submitted_on_date\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

        for entry in &mut entries {
            let id = self
                .pool
                .insert_returning_id(
                    sql,
                    &[
                        SqlParam::I64(entry.account_id),
                        SqlParam::I64(entry.office_id),
                        SqlParam::OptI64(entry.reversal_id),
                        SqlParam::Str(entry.currency_code.clone()),
                        SqlParam::Str(entry.transaction_id.clone()),
                        SqlParam::OptI64(entry.loan_transaction_id),
                        SqlParam::OptI64(entry.savings_transaction_id),
                        SqlParam::OptI64(entry.client_transaction_id),
                        SqlParam::OptI64(entry.share_transaction_id),
                        SqlParam::Bool(entry.reversed),
                        SqlParam::Bool(entry.manual_entry),
                        SqlParam::Date(entry.entry_date),
                        SqlParam::I16(entry.type_enum),
                        SqlParam::Decimal(entry.amount),
                        SqlParam::OptStr(entry.description.clone()),
                        SqlParam::OptI16(entry.entity_type_enum),
                        SqlParam::OptI64(entry.entity_id),
                        SqlParam::Decimal(entry.office_running_balance),
                        SqlParam::Decimal(entry.organization_running_balance),
                        SqlParam::Bool(entry.is_running_balance_calculated),
                        SqlParam::OptI64(entry.payment_details_id),
                        SqlParam::OptStr(entry.ref_num.clone()),
                        SqlParam::Date(entry.submitted_on_date),
                    ],
                )
                .await?;
            entry.id = id;
        }

        Ok(entries)
    }

    async fn find_by_transaction_id(
        &self,
        transaction_id: &str,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        let sql = "SELECT id, account_id, office_id, reversal_id, currency_code, transaction_id, \
            loan_transaction_id, savings_transaction_id, client_transaction_id, share_transaction_id, \
            reversed, manual_entry, entry_date, type_enum, amount, description, \
            entity_type_enum, entity_id, office_running_balance, organization_running_balance, \
            is_running_balance_calculated, payment_details_id, ref_num, submitted_on_date \
            FROM acc_gl_journal_entry WHERE transaction_id = ? ORDER BY id ASC";
        let rows = self
            .pool
            .query_all(sql, &[SqlParam::Str(transaction_id.to_string())])
            .await?;
        rows.iter().map(JournalEntryRecord::from_db_row).collect()
    }

    async fn find_by_office(
        &self,
        office_id: Option<i64>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        let (sql, params) = if let Some(oid) = office_id {
            (
                "SELECT id, account_id, office_id, reversal_id, currency_code, transaction_id, \
                loan_transaction_id, savings_transaction_id, client_transaction_id, share_transaction_id, \
                reversed, manual_entry, entry_date, type_enum, amount, description, \
                entity_type_enum, entity_id, office_running_balance, organization_running_balance, \
                is_running_balance_calculated, payment_details_id, ref_num, submitted_on_date \
                FROM acc_gl_journal_entry WHERE office_id = ? ORDER BY entry_date ASC, id ASC",
                vec![SqlParam::I64(oid)],
            )
        } else {
            (
                "SELECT id, account_id, office_id, reversal_id, currency_code, transaction_id, \
                loan_transaction_id, savings_transaction_id, client_transaction_id, share_transaction_id, \
                reversed, manual_entry, entry_date, type_enum, amount, description, \
                entity_type_enum, entity_id, office_running_balance, organization_running_balance, \
                is_running_balance_calculated, payment_details_id, ref_num, submitted_on_date \
                FROM acc_gl_journal_entry ORDER BY entry_date ASC, id ASC",
                vec![],
            )
        };
        let rows = self.pool.query_all(sql, &params).await?;
        rows.iter().map(JournalEntryRecord::from_db_row).collect()
    }

    async fn update_batch(
        &self,
        entries: &[JournalEntryRecord],
    ) -> Result<(), FineractError> {
        let sql = "UPDATE acc_gl_journal_entry SET \
            reversal_id = ?, reversed = ?, office_running_balance = ?, \
            organization_running_balance = ?, is_running_balance_calculated = ? \
            WHERE id = ?";
        for entry in entries {
            self.pool
                .execute(
                    sql,
                    &[
                        SqlParam::OptI64(entry.reversal_id),
                        SqlParam::Bool(entry.reversed),
                        SqlParam::Decimal(entry.office_running_balance),
                        SqlParam::Decimal(entry.organization_running_balance),
                        SqlParam::Bool(entry.is_running_balance_calculated),
                        SqlParam::I64(entry.id),
                    ],
                )
                .await?;
        }
        Ok(())
    }
}

/// Thread-safe in-memory repository for unit testing and standalone command execution.
#[derive(Default)]
pub struct InMemoryJournalEntryRepository {
    entries: RwLock<Vec<JournalEntryRecord>>,
    next_id: RwLock<i64>,
}

impl InMemoryJournalEntryRepository {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            next_id: RwLock::new(1),
        }
    }
}

#[async_trait]
impl JournalEntryRepository for InMemoryJournalEntryRepository {
    async fn save_batch(
        &self,
        mut batch: Vec<JournalEntryRecord>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        let mut next_id = self.next_id.write().expect("poisoned lock");
        let mut store = self.entries.write().expect("poisoned lock");
        for entry in &mut batch {
            if entry.id <= 0 {
                entry.id = *next_id;
                *next_id += 1;
            } else if entry.id >= *next_id {
                *next_id = entry.id + 1;
            }
            store.push(entry.clone());
        }
        Ok(batch)
    }

    async fn find_by_transaction_id(
        &self,
        transaction_id: &str,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        let store = self.entries.read().expect("poisoned lock");
        Ok(store
            .iter()
            .filter(|e| e.transaction_id == transaction_id)
            .cloned()
            .collect())
    }

    async fn find_by_office(
        &self,
        office_id: Option<i64>,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        let store = self.entries.read().expect("poisoned lock");
        let mut result: Vec<JournalEntryRecord> = store
            .iter()
            .filter(|e| office_id.map_or(true, |oid| e.office_id == oid))
            .cloned()
            .collect();
        result.sort_by_key(|e| (e.entry_date, e.id));
        Ok(result)
    }

    async fn update_batch(
        &self,
        updated: &[JournalEntryRecord],
    ) -> Result<(), FineractError> {
        let mut store = self.entries.write().expect("poisoned lock");
        for item in updated {
            if let Some(existing) = store.iter_mut().find(|e| e.id == item.id) {
                *existing = item.clone();
            }
        }
        Ok(())
    }
}

/// JSON payload for `CREATE_JOURNALENTRY` command (`JournalEntryCommand.java`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJournalEntryCommandPayload {
    pub office_id: i64,
    pub transaction_date: NaiveDate,
    pub currency_code: String,
    pub comments: Option<String>,
    pub reference_number: Option<String>,
    pub payment_type_id: Option<i64>,
    pub credits: Vec<SingleDebitOrCreditEntryCommand>,
    pub debits: Vec<SingleDebitOrCreditEntryCommand>,
}

/// Single leg inside `CreateJournalEntryCommandPayload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleDebitOrCreditEntryCommand {
    pub gl_account_id: i64,
    pub amount: Decimal,
    pub comments: Option<String>,
}

/// Command handler for `CREATE_JOURNALENTRY`.
pub struct CreateJournalEntryCommandHandler {
    repo: Arc<dyn JournalEntryRepository>,
    accounts: Arc<RwLock<HashMap<i64, GLAccount>>>,
    closures: Arc<RwLock<Vec<GLClosure>>>,
}

impl CreateJournalEntryCommandHandler {
    pub fn new(
        repo: Arc<dyn JournalEntryRepository>,
        accounts: Arc<RwLock<HashMap<i64, GLAccount>>>,
        closures: Arc<RwLock<Vec<GLClosure>>>,
    ) -> Self {
        Self {
            repo,
            accounts,
            closures,
        }
    }
}

#[async_trait]
impl CommandHandler for CreateJournalEntryCommandHandler {
    fn action_name(&self) -> &'static str {
        "CREATE"
    }

    fn entity_name(&self) -> &'static str {
        "JOURNALENTRY"
    }

    async fn handle(
        &self,
        envelope: &CommandEnvelope,
    ) -> Result<CommandProcessingResult, FineractError> {
        let payload: CreateJournalEntryCommandPayload =
            serde_json::from_str(&envelope.json_payload).map_err(|e| {
                FineractError::DomainValidation(vec![ApiParameterError::parameter(
                    "json",
                    "error.msg.invalid.request.body",
                    format!("Invalid CREATE_JOURNALENTRY payload: {e}"),
                )])
            })?;

        let mut lines = Vec::with_capacity(payload.debits.len() + payload.credits.len());
        for d in &payload.debits {
            lines.push(JournalEntryLine::debit(
                d.gl_account_id,
                d.amount,
                d.comments.clone(),
            ));
        }
        for c in &payload.credits {
            lines.push(JournalEntryLine::credit(
                c.gl_account_id,
                c.amount,
                c.comments.clone(),
            ));
        }

        let records = {
            let accounts_guard = self.accounts.read().expect("poisoned lock");
            let closures_guard = self.closures.read().expect("poisoned lock");
            AccountingBridgeEngine::create_manual_journal_entries(
                0,
                payload.office_id,
                payload.transaction_date,
                payload.transaction_date,
                &payload.currency_code,
                None,
                payload.payment_type_id,
                payload.reference_number,
                payload.comments,
                &lines,
                &closures_guard,
                &accounts_guard,
            )?
        };

        let saved = self.repo.save_batch(records).await?;
        let tx_id = saved
            .first()
            .map(|r| r.transaction_id.clone())
            .unwrap_or_default();

        Ok(CommandProcessingResult::from_office_and_tx(
            payload.office_id,
            tx_id,
        ))
    }
}

// ============================================================================
// 8. Comprehensive Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_chart_of_accounts() -> HashMap<i64, GLAccount> {
        let mut map = HashMap::new();
        let make = |id: i64,
                    name: &str,
                    code: &str,
                    account_type: GLAccountType,
                    account_usage: GLAccountUsage,
                    disabled: bool,
                    manual: bool| GLAccount {
            id,
            parent_id: None,
            hierarchy: Some(format!(".{id}.")),
            name: name.to_string(),
            gl_code: code.to_string(),
            disabled,
            manual_entries_allowed: manual,
            account_type,
            account_usage,
            description: None,
            tag_id: None,
        };

        // Assets (1xx)
        map.insert(
            101,
            make(
                101,
                "Fund Source Cash",
                "10100",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            102,
            make(
                102,
                "Loan Portfolio",
                "10200",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            103,
            make(
                103,
                "Interest Receivable",
                "10300",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            104,
            make(
                104,
                "Fees Receivable",
                "10400",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            105,
            make(
                105,
                "Penalties Receivable",
                "10500",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            106,
            make(
                106,
                "Transfers In Suspense",
                "10600",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            107,
            make(
                107,
                "Overdraft Portfolio Control",
                "10700",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );

        // Liabilities (2xx)
        map.insert(
            201,
            make(
                201,
                "Overpayment Liability",
                "20100",
                GLAccountType::Liability,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            202,
            make(
                202,
                "Savings Control",
                "20200",
                GLAccountType::Liability,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            203,
            make(
                203,
                "Interest Payable",
                "20300",
                GLAccountType::Liability,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );

        // Income (4xx)
        map.insert(
            401,
            make(
                401,
                "Interest On Loans",
                "40100",
                GLAccountType::Income,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            402,
            make(
                402,
                "Income From Fees",
                "40200",
                GLAccountType::Income,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            403,
            make(
                403,
                "Income From Penalties",
                "40300",
                GLAccountType::Income,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            404,
            make(
                404,
                "Income From Recovery",
                "40400",
                GLAccountType::Income,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );

        // Expense (5xx)
        map.insert(
            501,
            make(
                501,
                "Losses Written Off",
                "50100",
                GLAccountType::Expense,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            502,
            make(
                502,
                "Interest On Savings",
                "50200",
                GLAccountType::Expense,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );
        map.insert(
            503,
            make(
                503,
                "Goodwill Credit Expense",
                "50300",
                GLAccountType::Expense,
                GLAccountUsage::Detail,
                false,
                true,
            ),
        );

        // Special test accounts (disabled, header, no-manual)
        map.insert(
            901,
            make(
                901,
                "Disabled Account",
                "90100",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                true,
                true,
            ),
        );
        map.insert(
            902,
            make(
                902,
                "Header Account",
                "90200",
                GLAccountType::Asset,
                GLAccountUsage::Header,
                false,
                true,
            ),
        );
        map.insert(
            903,
            make(
                903,
                "System Only Account",
                "90300",
                GLAccountType::Asset,
                GLAccountUsage::Detail,
                false,
                false,
            ),
        );

        map
    }

    fn sample_loan_mapping(rule: AccountingRuleType) -> LoanProductAccountMapping {
        LoanProductAccountMapping::new(1, rule)
            .with_account(LoanProductAccountingParams::FundSource, 101)
            .with_account(LoanProductAccountingParams::LoanPortfolio, 102)
            .with_account(LoanProductAccountingParams::InterestReceivable, 103)
            .with_account(LoanProductAccountingParams::FeesReceivable, 104)
            .with_account(LoanProductAccountingParams::PenaltiesReceivable, 105)
            .with_account(LoanProductAccountingParams::TransfersInSuspense, 106)
            .with_account(LoanProductAccountingParams::OverpaymentLiability, 201)
            .with_account(LoanProductAccountingParams::InterestOnLoans, 401)
            .with_account(LoanProductAccountingParams::IncomeFromFees, 402)
            .with_account(LoanProductAccountingParams::IncomeFromPenalties, 403)
            .with_account(LoanProductAccountingParams::IncomeFromRecovery, 404)
            .with_account(LoanProductAccountingParams::LossesWrittenOff, 501)
            .with_account(LoanProductAccountingParams::GoodwillCredit, 503)
    }

    fn sample_savings_mapping(rule: AccountingRuleType) -> SavingsProductAccountMapping {
        SavingsProductAccountMapping::new(10, rule)
            .with_account(SavingsProductAccountingParams::SavingsReference, 101)
            .with_account(SavingsProductAccountingParams::TransfersInSuspense, 106)
            .with_account(
                SavingsProductAccountingParams::OverdraftPortfolioControl,
                107,
            )
            .with_account(SavingsProductAccountingParams::SavingsControl, 202)
            .with_account(SavingsProductAccountingParams::InterestPayable, 203)
            .with_account(SavingsProductAccountingParams::IncomeFromInterest, 401)
            .with_account(SavingsProductAccountingParams::IncomeFromFees, 402)
            .with_account(SavingsProductAccountingParams::IncomeFromPenalties, 403)
            .with_account(SavingsProductAccountingParams::LossesWrittenOff, 501)
            .with_account(SavingsProductAccountingParams::InterestOnSavings, 502)
    }

    #[test]
    fn test_double_entry_validator_accepts_balanced_and_rejects_unbalanced() {
        let accounts = sample_chart_of_accounts();
        let closures = vec![GLClosure {
            id: 1,
            office_id: 1,
            closing_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
            is_deleted: false,
            comments: None,
        }];
        let valid_date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();

        // 1. Balanced multi-leg entry succeeds
        let balanced = vec![
            JournalEntryLine::debit(101, dec!(150.00), None),
            JournalEntryLine::credit(102, dec!(100.00), None),
            JournalEntryLine::credit(401, dec!(50.00), None),
        ];
        assert!(DoubleEntryJournalValidator::validate_balanced_batch(
            1,
            valid_date,
            "USD",
            &balanced,
            &closures,
            &accounts
        )
        .is_ok());

        // 2. Mismatched sum fails with mismatch code
        let unbalanced = vec![
            JournalEntryLine::debit(101, dec!(150.00), None),
            JournalEntryLine::credit(102, dec!(149.99), None),
        ];
        let err = DoubleEntryJournalValidator::validate_balanced_batch(
            1,
            valid_date,
            "USD",
            &unbalanced,
            &closures,
            &accounts,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FineractError::DomainRule {
                ref globalisation_code,
                ..
            } if globalisation_code == "error.msg.glJournalEntry.invalid.mismatch.debits.credits"
        ));

        // 3. Only debits (no credits) fails
        let only_debits = vec![JournalEntryLine::debit(101, dec!(100.00), None)];
        let err = DoubleEntryJournalValidator::validate_balanced_batch(
            1,
            valid_date,
            "USD",
            &only_debits,
            &closures,
            &accounts,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FineractError::DomainRule {
                ref globalisation_code,
                ..
            } if globalisation_code == "error.msg.glJournalEntry.invalid.no.debits.or.credits"
        ));

        // 4. Entry on or before closure date fails
        let closed_date = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let err = DoubleEntryJournalValidator::validate_balanced_batch(
            1,
            closed_date,
            "USD",
            &balanced,
            &closures,
            &accounts,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FineractError::DomainRule {
                ref globalisation_code,
                ..
            } if globalisation_code == "error.msg.glJournalEntry.invalid.accounting.closed"
        ));

        // 5. Disabled, Header, and Manual-disallowed accounts fail appropriately
        let disabled_lines = vec![
            JournalEntryLine::debit(901, dec!(50.00), None),
            JournalEntryLine::credit(101, dec!(50.00), None),
        ];
        assert!(DoubleEntryJournalValidator::validate_balanced_batch(
            1,
            valid_date,
            "USD",
            &disabled_lines,
            &closures,
            &accounts
        )
        .is_err());

        let header_lines = vec![
            JournalEntryLine::debit(902, dec!(50.00), None),
            JournalEntryLine::credit(101, dec!(50.00), None),
        ];
        assert!(DoubleEntryJournalValidator::validate_balanced_batch(
            1,
            valid_date,
            "USD",
            &header_lines,
            &closures,
            &accounts
        )
        .is_err());

        let no_manual_lines = vec![
            JournalEntryLine::debit(903, dec!(50.00), None),
            JournalEntryLine::credit(101, dec!(50.00), None),
        ];
        assert!(
            DoubleEntryJournalValidator::validate_balanced_batch_with_options(
                1,
                valid_date,
                "USD",
                &no_manual_lines,
                &closures,
                &accounts,
                true,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn test_loan_cash_vs_accrual_disbursement_repayment_and_writeoff() {
        let accounts = sample_chart_of_accounts();
        let closures = Vec::new();
        let date = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();

        let cash_mapping = sample_loan_mapping(AccountingRuleType::CashBased);
        let accrual_mapping = sample_loan_mapping(AccountingRuleType::AccrualPeriodic);

        // 1. Disbursement of 1,000.00: DR LoanPortfolio (102), CR FundSource (101)
        let disb_tx = LoanBridgeTransaction::new(
            77,
            1,
            5001,
            1,
            date,
            "USD",
            LoanBridgeTransactionType::Disbursement,
            dec!(1000.00),
        );
        let disb_entries = AccountingBridgeEngine::generate_loan_journal_entries(
            1,
            &disb_tx,
            &cash_mapping,
            &closures,
            &accounts,
        )
        .unwrap();
        assert_eq!(disb_entries.len(), 2);
        assert_eq!(disb_entries[0].account_id, 102);
        assert_eq!(disb_entries[0].type_enum, JournalEntryType::Debit.as_i16());
        assert_eq!(disb_entries[0].amount, dec!(1000.00));
        assert_eq!(disb_entries[1].account_id, 101);
        assert_eq!(disb_entries[1].type_enum, JournalEntryType::Credit.as_i16());
        assert_eq!(disb_entries[1].amount, dec!(1000.00));

        // 2. Periodic Accrual of 50 interest + 10 fee + 5 penalty:
        //    DR InterestReceivable(103) 50, CR InterestOnLoans(401) 50
        //    DR FeesReceivable(104) 10, CR IncomeFromFees(402) 10
        //    DR PenaltiesReceivable(105) 5, CR IncomeFromPenalties(403) 5
        let mut accrual_tx = LoanBridgeTransaction::new(
            77,
            1,
            5002,
            1,
            date,
            "USD",
            LoanBridgeTransactionType::Accrual,
            dec!(65.00),
        );
        accrual_tx.interest_portion = dec!(50.00);
        accrual_tx.fee_charges_portion = dec!(10.00);
        accrual_tx.penalty_charges_portion = dec!(5.00);

        let accrual_entries = AccountingBridgeEngine::generate_loan_journal_entries(
            10,
            &accrual_tx,
            &accrual_mapping,
            &closures,
            &accounts,
        )
        .unwrap();
        assert_eq!(accrual_entries.len(), 6);

        // 3. Repayment of 200 Principal + 50 Interest + 10 Fee + 5 Penalty (Total 265):
        // Under CashBased -> Credits go to LoanPortfolio(102), InterestOnLoans(401), IncomeFromFees(402), IncomeFromPenalties(403)
        // Under AccrualPeriodic -> Credits go to LoanPortfolio(102), InterestReceivable(103), FeesReceivable(104), PenaltiesReceivable(105)
        let mut repay_tx = LoanBridgeTransaction::new(
            77,
            1,
            5003,
            1,
            date,
            "USD",
            LoanBridgeTransactionType::Repayment,
            dec!(265.00),
        );
        repay_tx.principal_portion = dec!(200.00);
        repay_tx.interest_portion = dec!(50.00);
        repay_tx.fee_charges_portion = dec!(10.00);
        repay_tx.penalty_charges_portion = dec!(5.00);

        let cash_repay = AccountingBridgeEngine::generate_loan_journal_entries(
            20,
            &repay_tx,
            &cash_mapping,
            &closures,
            &accounts,
        )
        .unwrap();
        assert_eq!(cash_repay.len(), 5);
        assert!(cash_repay
            .iter()
            .any(|e| e.account_id == 401 && e.type_enum == JournalEntryType::Credit.as_i16()));
        assert!(cash_repay
            .iter()
            .any(|e| e.account_id == 402 && e.type_enum == JournalEntryType::Credit.as_i16()));

        let accrual_repay = AccountingBridgeEngine::generate_loan_journal_entries(
            30,
            &repay_tx,
            &accrual_mapping,
            &closures,
            &accounts,
        )
        .unwrap();
        assert_eq!(accrual_repay.len(), 5);
        assert!(accrual_repay
            .iter()
            .any(|e| e.account_id == 103 && e.type_enum == JournalEntryType::Credit.as_i16()));
        assert!(accrual_repay
            .iter()
            .any(|e| e.account_id == 104 && e.type_enum == JournalEntryType::Credit.as_i16()));
        assert!(accrual_repay
            .iter()
            .any(|e| e.account_id == 105 && e.type_enum == JournalEntryType::Credit.as_i16()));
    }

    #[test]
    fn test_savings_deposit_withdrawal_interest_posting_and_fee_deduction() {
        let accounts = sample_chart_of_accounts();
        let closures = Vec::new();
        let date = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        let mapping = sample_savings_mapping(AccountingRuleType::CashBased);

        // Deposit 500 -> DR SavingsReference(101) 500, CR SavingsControl(202) 500
        let dep_tx = SavingsBridgeTransaction::new(
            88,
            10,
            7001,
            1,
            date,
            "USD",
            SavingsBridgeTransactionType::Deposit,
            dec!(500.00),
        );
        let dep_entries = AccountingBridgeEngine::generate_savings_journal_entries(
            1,
            &dep_tx,
            &mapping,
            &closures,
            &accounts,
        )
        .unwrap();
        assert_eq!(dep_entries.len(), 2);
        assert_eq!(dep_entries[0].account_id, 101);
        assert_eq!(dep_entries[0].type_enum, JournalEntryType::Debit.as_i16());
        assert_eq!(dep_entries[1].account_id, 202);
        assert_eq!(dep_entries[1].type_enum, JournalEntryType::Credit.as_i16());

        // Interest Posting 12.50 -> DR InterestOnSavings(502) 12.50, CR SavingsControl(202) 12.50
        let int_tx = SavingsBridgeTransaction::new(
            88,
            10,
            7002,
            1,
            date,
            "USD",
            SavingsBridgeTransactionType::InterestPosting,
            dec!(12.50),
        );
        let int_entries = AccountingBridgeEngine::generate_savings_journal_entries(
            10,
            &int_tx,
            &mapping,
            &closures,
            &accounts,
        )
        .unwrap();
        assert_eq!(int_entries.len(), 2);
        assert_eq!(int_entries[0].account_id, 502);
        assert_eq!(int_entries[1].account_id, 202);
    }

    #[test]
    fn test_reverse_journal_entries_and_running_balance_calculation() {
        let accounts = sample_chart_of_accounts();
        let closures = Vec::new();
        let day1 = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 3, 2).unwrap();
        let cash_mapping = sample_loan_mapping(AccountingRuleType::CashBased);

        // Day 1: Disbursement of 1,000.00 (DR LoanPortfolio 102, CR FundSource 101)
        let disb_tx = LoanBridgeTransaction::new(
            77,
            1,
            9001,
            1,
            day1,
            "USD",
            LoanBridgeTransactionType::Disbursement,
            dec!(1000.00),
        );
        let mut entries = AccountingBridgeEngine::generate_loan_journal_entries(
            1,
            &disb_tx,
            &cash_mapping,
            &closures,
            &accounts,
        )
        .unwrap();

        // Day 2: Repayment of 200 Principal + 40 Interest (DR FundSource 101 = 240, CR LoanPortfolio 102 = 200, CR InterestOnLoans 401 = 40)
        let mut repay_tx = LoanBridgeTransaction::new(
            77,
            1,
            9002,
            1,
            day2,
            "USD",
            LoanBridgeTransactionType::Repayment,
            dec!(240.00),
        );
        repay_tx.principal_portion = dec!(200.00);
        repay_tx.interest_portion = dec!(40.00);
        let mut repay_entries = AccountingBridgeEngine::generate_loan_journal_entries(
            10,
            &repay_tx,
            &cash_mapping,
            &closures,
            &accounts,
        )
        .unwrap();

        entries.append(&mut repay_entries);

        // Calculate running balances across Day 1 + Day 2
        RunningBalanceCalculator::calculate_running_balances(&mut entries, &accounts).unwrap();

        // Verify Asset account 102 (LoanPortfolio): +1000 on Day 1 (Debit), +800 on Day 2 (after 200 Credit)
        let lp_day1 = entries
            .iter()
            .find(|e| e.id == 1 && e.account_id == 102)
            .unwrap();
        assert_eq!(lp_day1.office_running_balance, dec!(1000.00));
        let lp_day2 = entries
            .iter()
            .find(|e| e.account_id == 102 && e.entry_date == day2)
            .unwrap();
        assert_eq!(lp_day2.office_running_balance, dec!(800.00));

        // Verify Income account 401 (InterestOnLoans): increases on Credit (+40.00)
        let inc_day2 = entries
            .iter()
            .find(|e| e.account_id == 401 && e.entry_date == day2)
            .unwrap();
        assert_eq!(inc_day2.office_running_balance, dec!(40.00));
        assert!(inc_day2.is_running_balance_calculated);

        // Reverse Day 2 repayment entries and verify reversal linkage
        let mut to_reverse: Vec<JournalEntryRecord> = entries
            .iter()
            .filter(|e| e.transaction_id == "L9002")
            .cloned()
            .collect();
        let reversals = AccountingBridgeEngine::reverse_journal_entries(
            100,
            &mut to_reverse,
            day2,
            Some("Bounce check".to_string()),
            &closures,
        )
        .unwrap();

        assert_eq!(reversals.len(), 3);
        for (orig, rev) in to_reverse.iter().zip(reversals.iter()) {
            assert!(orig.reversed);
            assert_eq!(orig.reversal_id, Some(rev.id));
            assert!(rev.reversed);
            assert_eq!(
                JournalEntryType::from_i16(orig.type_enum)
                    .unwrap()
                    .opposite(),
                JournalEntryType::from_i16(rev.type_enum).unwrap()
            );
        }
    }
}
