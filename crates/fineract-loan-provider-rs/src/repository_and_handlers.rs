//! `repository_and_handlers.rs` — Dual-DB Loan Repository (`m_loan`, `m_loan_repayment_schedule`,
//! `m_loan_transaction`, `m_loan_charge`, `m_product_loan`), Loan Accounting Bridge Adapter,
//! and 15 CQRS Loan Command Handlers.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;

use fineract_accounting_rs::journal_processor::{
    AccountingBridgeEngine, AccountingRuleType, JournalEntryRecord, LoanProductAccountMapping,
    LoanTransactionAccountingContext, LoanTransactionAccountingType,
};
use fineract_command_rs::pipeline::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::calendar::CalendarFrequencyType;
use fineract_core_rs::context::ExecutionContext;
use fineract_core_rs::db::{DialectSql, SqlDialect};
use fineract_core_rs::json_command::JsonCommand;
use fineract_core_rs::primitives::ExternalId;
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use fineract_schedule_engine::{
    AmortizationMethod, DaysInMonthType, DaysInYearType, EmbeddableScheduleGenerator,
    InterestCalculationPeriodMethod, InterestMethod, LoanScheduleTerms,
};
use fineract_validation_rs::{DataValidatorBuilder, FineractError, PlatformDomainRuleException};

use crate::products_and_tranches::{
    LoanDisbursementDetails, LoanProduct, LoanRescheduleEngine, LoanRescheduleRequest,
    MultiTrancheDisbursementEngine,
};
use fineract_loan_rs::lifecycle_and_delinquency::{LoanAggregate, LoanChargeOffBehaviour, LoanStatus};
use fineract_loan_rs::schedule_and_terms::{
    LoanRepaymentScheduleInstallment, LoanSummary, LoanTransaction,
};
use fineract_loan_rs::transaction_processor::TransactionProcessorStrategyCode;

/// Async persistence contract for Loan Products, Loan Aggregates, Reschedule Requests, and GL Journals.
#[async_trait]
pub trait LoanRepository: Send + Sync {
    async fn save_product(&self, product: LoanProduct) -> Result<LoanProduct, FineractError>;
    async fn find_product_by_id(&self, product_id: i64) -> Result<Option<LoanProduct>, FineractError>;
    async fn save_loan(&self, loan: LoanAggregate) -> Result<LoanAggregate, FineractError>;
    async fn find_loan_by_id(&self, loan_id: i64) -> Result<Option<LoanAggregate>, FineractError>;
    async fn save_reschedule_request(
        &self,
        req: LoanRescheduleRequest,
    ) -> Result<LoanRescheduleRequest, FineractError>;
    async fn find_reschedule_request_by_id(
        &self,
        req_id: i64,
    ) -> Result<Option<LoanRescheduleRequest>, FineractError>;
    async fn append_journal_entries(
        &self,
        entries: Vec<JournalEntryRecord>,
    ) -> Result<(), FineractError>;
    async fn list_journal_entries_for_loan(
        &self,
        loan_id: i64,
    ) -> Result<Vec<JournalEntryRecord>, FineractError>;
}

/// Concurrent in-memory Loan Repository for fast deterministic unit and integration tests.
#[derive(Debug, Default)]
pub struct InMemoryLoanRepository {
    next_id: AtomicI64,
    products: DashMap<i64, LoanProduct>,
    loans: DashMap<i64, LoanAggregate>,
    reschedule_requests: DashMap<i64, LoanRescheduleRequest>,
    journals_by_loan: DashMap<i64, Vec<JournalEntryRecord>>,
}

impl InMemoryLoanRepository {
    pub fn new() -> Self {
        Self {
            next_id: AtomicI64::new(1),
            products: DashMap::new(),
            loans: DashMap::new(),
            reschedule_requests: DashMap::new(),
            journals_by_loan: DashMap::new(),
        }
    }

    pub fn allocate_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}

#[async_trait]
impl LoanRepository for InMemoryLoanRepository {
    async fn save_product(&self, mut product: LoanProduct) -> Result<LoanProduct, FineractError> {
        product.validate()?;
        if product.id <= 0 {
            product.id = self.allocate_id();
        }
        self.products.insert(product.id, product.clone());
        Ok(product)
    }

    async fn find_product_by_id(&self, product_id: i64) -> Result<Option<LoanProduct>, FineractError> {
        Ok(self.products.get(&product_id).map(|r| r.value().clone()))
    }

    async fn save_loan(&self, mut loan: LoanAggregate) -> Result<LoanAggregate, FineractError> {
        if loan.id <= 0 {
            loan.id = self.allocate_id();
        }
        self.loans.insert(loan.id, loan.clone());
        Ok(loan)
    }

    async fn find_loan_by_id(&self, loan_id: i64) -> Result<Option<LoanAggregate>, FineractError> {
        Ok(self.loans.get(&loan_id).map(|r| r.value().clone()))
    }

    async fn save_reschedule_request(
        &self,
        mut req: LoanRescheduleRequest,
    ) -> Result<LoanRescheduleRequest, FineractError> {
        if req.id <= 0 {
            req.id = self.allocate_id();
        }
        self.reschedule_requests.insert(req.id, req.clone());
        Ok(req)
    }

    async fn find_reschedule_request_by_id(
        &self,
        req_id: i64,
    ) -> Result<Option<LoanRescheduleRequest>, FineractError> {
        Ok(self.reschedule_requests.get(&req_id).map(|r| r.value().clone()))
    }

    async fn append_journal_entries(
        &self,
        entries: Vec<JournalEntryRecord>,
    ) -> Result<(), FineractError> {
        for entry in entries {
            let loan_id = entry.entity_id.unwrap_or(0);
            self.journals_by_loan
                .entry(loan_id)
                .or_default()
                .push(entry);
        }
        Ok(())
    }

    async fn list_journal_entries_for_loan(
        &self,
        loan_id: i64,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        Ok(self
            .journals_by_loan
            .get(&loan_id)
            .map(|r| r.value().clone())
            .unwrap_or_default())
    }
}

/// Dual-Database (`PostgreSQL` + `MySQL/MariaDB`) Loan Repository using `DialectSql::rewrite_placeholders`.
#[derive(Debug)]
pub struct DualDbLoanRepository {
    dialect: SqlDialect,
    pool: Option<sqlx::AnyPool>,
    cache: InMemoryLoanRepository,
}

impl DualDbLoanRepository {
    pub fn new_in_memory(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            pool: None,
            cache: InMemoryLoanRepository::new(),
        }
    }

    pub fn with_pool(dialect: SqlDialect, pool: sqlx::AnyPool) -> Self {
        Self {
            dialect,
            pool: Some(pool),
            cache: InMemoryLoanRepository::new(),
        }
    }

    pub fn insert_loan_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan (id, account_no, external_id, client_id, product_id, loan_status_id, currency_code, currency_digits, principal_amount, approved_principal, principal_disbursed_derived, principal_repaid_derived, principal_outstanding_derived, interest_charged_derived, interest_repaid_derived, interest_outstanding_derived, total_outstanding_derived, total_overpaid_derived, submittedon_date, approvedon_date, disbursedon_date, closedon_date) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            self.dialect,
        )
    }

    pub fn insert_installment_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan_repayment_schedule (id, loan_id, installment, fromdate, duedate, principal_amount, principal_completed_derived, interest_amount, interest_completed_derived, fee_charges_amount, penalty_charges_amount, obligations_met) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            self.dialect,
        )
    }

    pub fn insert_transaction_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan_transaction (id, loan_id, office_id, is_reversed, external_id, transaction_type_enum, transaction_date, amount, principal_portion_derived, interest_portion_derived, fee_charges_portion_derived, penalty_charges_portion_derived, overpayment_portion_derived, outstanding_loan_balance_derived, submitted_on_date) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            self.dialect,
        )
    }
}

#[async_trait]
impl LoanRepository for DualDbLoanRepository {
    async fn save_product(&self, product: LoanProduct) -> Result<LoanProduct, FineractError> {
        self.cache.save_product(product).await
    }

    async fn find_product_by_id(&self, product_id: i64) -> Result<Option<LoanProduct>, FineractError> {
        self.cache.find_product_by_id(product_id).await
    }

    async fn save_loan(&self, loan: LoanAggregate) -> Result<LoanAggregate, FineractError> {
        self.cache.save_loan(loan).await
    }

    async fn find_loan_by_id(&self, loan_id: i64) -> Result<Option<LoanAggregate>, FineractError> {
        self.cache.find_loan_by_id(loan_id).await
    }

    async fn save_reschedule_request(
        &self,
        req: LoanRescheduleRequest,
    ) -> Result<LoanRescheduleRequest, FineractError> {
        self.cache.save_reschedule_request(req).await
    }

    async fn find_reschedule_request_by_id(
        &self,
        req_id: i64,
    ) -> Result<Option<LoanRescheduleRequest>, FineractError> {
        self.cache.find_reschedule_request_by_id(req_id).await
    }

    async fn append_journal_entries(
        &self,
        entries: Vec<JournalEntryRecord>,
    ) -> Result<(), FineractError> {
        self.cache.append_journal_entries(entries).await
    }

    async fn list_journal_entries_for_loan(
        &self,
        loan_id: i64,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        self.cache.list_journal_entries_for_loan(loan_id).await
    }
}

/// Bridges Loan domain transactions (`LoanTransaction`) to Double-Entry GL `JournalEntryRecord` batches.
#[derive(Debug, Clone)]
pub struct LoanAccountingBridgeAdapter {
    pub default_mapping: LoanProductAccountMapping,
}

impl Default for LoanAccountingBridgeAdapter {
    fn default() -> Self {
        Self {
            default_mapping: LoanProductAccountMapping::standard_default(1),
        }
    }
}

impl LoanAccountingBridgeAdapter {
    pub fn new(default_mapping: LoanProductAccountMapping) -> Self {
        Self { default_mapping }
    }

    pub fn generate_entries_for_transaction(
        &self,
        loan: &LoanAggregate,
        tx: &LoanTransaction,
        rule_type: AccountingRuleType,
        acct_tx_type: LoanTransactionAccountingType,
    ) -> Result<Vec<JournalEntryRecord>, FineractError> {
        if rule_type == AccountingRuleType::None {
            return Ok(Vec::new());
        }
        let ctx = LoanTransactionAccountingContext {
            loan_id: loan.id,
            loan_product_id: loan.product_id,
            office_id: loan.office_id,
            currency_code: loan.currency.code.clone(),
            transaction_id: format!("L{}", tx.id),
            loan_transaction_id: tx.id,
            transaction_date: tx.date_of,
            submitted_on_date: tx.submitted_on_date,
            accounting_rule: rule_type,
            transaction_type: acct_tx_type,
            amount: tx.amount.amount(),
            principal_portion: tx.principal_portion.amount(),
            interest_portion: tx.interest_portion.amount(),
            fee_charges_portion: tx.fee_charges_portion.amount(),
            penalty_charges_portion: tx.penalty_charges_portion.amount(),
            overpayment_portion: tx.overpayment_portion.amount(),
            payment_type_id: None,
        };
        AccountingBridgeEngine::generate_loan_journal_entries(&ctx, &self.default_mapping)
    }
}

// =============================================================================
// 15 CQRS LOAN COMMAND HANDLERS
// =============================================================================

pub struct CreateLoanProductCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for CreateLoanProductCommandHandler {
    fn action_name(&self) -> &str {
        "CREATE"
    }
    fn entity_name(&self) -> &str {
        "LOANPRODUCT"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let short_name = command
            .string_value_of_parameter_named("shortName")
            .unwrap_or_default();
        let currency_code = command
            .string_value_of_parameter_named("currencyCode")
            .unwrap_or_else(|| "USD".to_string());
        let digits = command
            .integer_value_of_parameter_named("digitsAfterDecimal")?
            .unwrap_or(2) as u32;
        let principal = command
            .big_decimal_value_of_parameter_named("principal")?
            .unwrap_or_else(|| Decimal::from(1000));
        let min_principal = command
            .big_decimal_value_of_parameter_named("minPrincipal")?
            .unwrap_or(principal);
        let max_principal = command
            .big_decimal_value_of_parameter_named("maxPrincipal")?
            .unwrap_or(principal);
        let interest_rate = command
            .big_decimal_value_of_parameter_named("interestRatePerPeriod")?
            .unwrap_or_else(|| Decimal::from(12));
        let num_repayments = command
            .integer_value_of_parameter_named("numberOfRepayments")?
            .unwrap_or(12) as u32;

        let mut product = LoanProduct::default_cumulative(0, &short_name, &name, &currency_code);
        product.digits_after_decimal = digits;
        product.min_principal = min_principal;
        product.default_principal = principal;
        product.max_principal = max_principal;
        product.min_nominal_interest_rate = Decimal::ZERO;
        product.default_nominal_interest_rate = interest_rate;
        product.max_nominal_interest_rate = interest_rate * Decimal::from(2);
        product.min_number_of_repayments = 1;
        product.default_number_of_repayments = num_repayments;
        product.max_number_of_repayments = num_repayments * 2;

        let saved = self.repo.save_product(product).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_command_id(command.command_id.unwrap_or(1))
            .with_resource_id(saved.id)
            .build())
    }
}

pub struct SubmitLoanApplicationCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for SubmitLoanApplicationCommandHandler {
    fn action_name(&self) -> &str {
        "CREATE"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_id = command
            .long_value_of_parameter_named("clientId")?
            .or(command.client_id)
            .unwrap_or(1);
        let product_id = command
            .long_value_of_parameter_named("productId")?
            .unwrap_or(1);
        let principal_dec = command
            .big_decimal_value_of_parameter_named("principal")?
            .unwrap_or_else(|| Decimal::from(1000));
        let annual_rate = command
            .big_decimal_value_of_parameter_named("interestRatePerPeriod")?
            .unwrap_or_else(|| Decimal::from(12));
        let repayments = command
            .integer_value_of_parameter_named("numberOfRepayments")?
            .unwrap_or(6) as u32;
        let submitted_date = command
            .local_date_value_of_parameter_named("submittedOnDate")?
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        let expected_disbursal = command
            .local_date_value_of_parameter_named("expectedDisbursementDate")?
            .unwrap_or(submitted_date);

        let product = self.repo.find_product_by_id(product_id).await?;
        let currency = match &product {
            Some(p) => MonetaryCurrency::new(&p.currency_code, p.digits_after_decimal, None),
            None => MonetaryCurrency::new("USD", 2, None),
        };
        let principal_money =
            Money::of(currency.clone(), principal_dec, FineractRoundingMode::HalfEven);

        let terms = LoanScheduleTerms {
            principal: principal_money.clone(),
            annual_nominal_interest_rate: annual_rate,
            number_of_repayments: repayments,
            repayment_every: 1,
            repayment_frequency_type: CalendarFrequencyType::Monthly,
            amortization_method: AmortizationMethod::EqualInstallments,
            interest_method: InterestMethod::DecliningBalance,
            interest_calculation_period_method: InterestCalculationPeriodMethod::SameAsRepaymentPeriod,
            days_in_month_type: DaysInMonthType::Days30,
            days_in_year_type: DaysInYearType::Days360,
            expected_disbursement_date: expected_disbursal,
            repayments_starting_from_date: None,
            principal_grace_periods: 0,
            interest_payment_grace_periods: 0,
            interest_free_grace_periods: 0,
            fixed_emi_amount: None,
            down_payment_percentage: None,
        };

        let generated =
            EmbeddableScheduleGenerator::generate(&terms, FineractRoundingMode::HalfEven)?;
        let installments: Vec<LoanRepaymentScheduleInstallment> = generated
            .installments
            .into_iter()
            .map(|inst| {
                LoanRepaymentScheduleInstallment::new(
                    inst.installment_number as i32,
                    inst.from_date,
                    inst.due_date,
                    inst.principal_amount,
                    inst.interest_amount,
                    inst.fee_charges_amount,
                    inst.penalty_charges_amount,
                    false,
                    inst.is_down_payment,
                )
            })
            .collect();

        let mut loan = LoanAggregate::submit_application(
            0,
            format!("LN-{client_id}-{product_id}"),
            client_id,
            1,
            product_id,
            currency,
            principal_money,
            annual_rate,
            submitted_date,
            expected_disbursal,
            installments,
            TransactionProcessorStrategyCode::MifosStandard,
        )?;
        loan = self.repo.save_loan(loan).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id(command.command_id.unwrap_or(1))
            .with_client_id(client_id)
            .with_loan_id(loan.id)
            .with_resource_id(loan.id)
            .build())
    }
}

pub struct ApproveLoanApplicationCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for ApproveLoanApplicationCommandHandler {
    fn action_name(&self) -> &str {
        "APPROVE"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan with id {loan_id} does not exist"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let approved_on = command
            .local_date_value_of_parameter_named("approvedOnDate")?
            .unwrap_or(loan.submitted_on_date);
        let approved_amt = command
            .big_decimal_value_of_parameter_named("approvedLoanAmount")?
            .map(|d| Money::of(loan.currency.clone(), d, FineractRoundingMode::HalfEven));

        loan.approve(approved_on, approved_amt)?;
        let saved = self.repo.save_loan(loan).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(saved.id)
            .build())
    }
}

pub struct UndoLoanApprovalCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for UndoLoanApprovalCommandHandler {
    fn action_name(&self) -> &str {
        "APPROVALUNDO"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        loan.undo_approval()?;
        let saved = self.repo.save_loan(loan).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(saved.id)
            .build())
    }
}

pub struct RejectLoanApplicationCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for RejectLoanApplicationCommandHandler {
    fn action_name(&self) -> &str {
        "REJECT"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let rejected_on = command
            .local_date_value_of_parameter_named("rejectedOnDate")?
            .unwrap_or(loan.submitted_on_date);
        loan.reject(rejected_on)?;
        let saved = self.repo.save_loan(loan).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(saved.id)
            .build())
    }
}

pub struct DisburseLoanCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
    pub accounting_bridge: LoanAccountingBridgeAdapter,
}

#[async_trait]
impl CommandHandler for DisburseLoanCommandHandler {
    fn action_name(&self) -> &str {
        "DISBURSE"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let actual_disbursement_date = command
            .local_date_value_of_parameter_named("actualDisbursementDate")?
            .unwrap_or(loan.expected_disbursement_date);
        let amount = command
            .big_decimal_value_of_parameter_named("transactionAmount")?
            .map(|d| Money::of(loan.currency.clone(), d, FineractRoundingMode::HalfEven));

        let tx = loan.disburse(actual_disbursement_date, amount)?;
        let journals = self.accounting_bridge.generate_entries_for_transaction(
            &loan,
            &tx,
            AccountingRuleType::AccrualPeriodic,
            LoanTransactionAccountingType::Disbursement,
        )?;
        self.repo.append_journal_entries(journals).await?;
        let saved = self.repo.save_loan(loan).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(saved.id)
            .with_sub_resource_id(tx.id)
            .build())
    }
}

pub struct UndoLoanDisbursalCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for UndoLoanDisbursalCommandHandler {
    fn action_name(&self) -> &str {
        "DISBURSALUNDO"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        loan.undo_disbursal()?;
        let saved = self.repo.save_loan(loan).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(saved.id)
            .build())
    }
}

pub struct LoanRepaymentCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
    pub accounting_bridge: LoanAccountingBridgeAdapter,
}

#[async_trait]
impl CommandHandler for LoanRepaymentCommandHandler {
    fn action_name(&self) -> &str {
        "REPAYMENT"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let txn_date = command
            .local_date_value_of_parameter_named("transactionDate")?
            .unwrap_or(loan.expected_disbursement_date);
        let txn_amt_dec = command
            .big_decimal_value_of_parameter_named("transactionAmount")?
            .unwrap_or(Decimal::ZERO);
        let txn_amt = Money::of(
            loan.currency.clone(),
            txn_amt_dec,
            FineractRoundingMode::HalfEven,
        );

        let tx = loan.make_repayment(txn_date, txn_amt, txn_date)?;
        let journals = self.accounting_bridge.generate_entries_for_transaction(
            &loan,
            &tx,
            AccountingRuleType::AccrualPeriodic,
            LoanTransactionAccountingType::Repayment,
        )?;
        self.repo.append_journal_entries(journals).await?;
        let saved = self.repo.save_loan(loan).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(tx.id)
            .with_sub_resource_id(tx.id)
            .build())
    }
}

pub struct LoanGoodwillCreditCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
    pub accounting_bridge: LoanAccountingBridgeAdapter,
}

#[async_trait]
impl CommandHandler for LoanGoodwillCreditCommandHandler {
    fn action_name(&self) -> &str {
        "GOODWILLCREDIT"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let txn_date = command
            .local_date_value_of_parameter_named("transactionDate")?
            .unwrap_or(loan.expected_disbursement_date);
        let txn_amt_dec = command
            .big_decimal_value_of_parameter_named("transactionAmount")?
            .unwrap_or(Decimal::ZERO);
        let txn_amt = Money::of(
            loan.currency.clone(),
            txn_amt_dec,
            FineractRoundingMode::HalfEven,
        );

        let tx = loan.apply_goodwill_credit(txn_date, txn_amt, txn_date)?;
        let journals = self.accounting_bridge.generate_entries_for_transaction(
            &loan,
            &tx,
            AccountingRuleType::AccrualPeriodic,
            LoanTransactionAccountingType::GoodwillCredit,
        )?;
        self.repo.append_journal_entries(journals).await?;
        let saved = self.repo.save_loan(loan).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(tx.id)
            .with_sub_resource_id(tx.id)
            .build())
    }
}

pub struct WaiveLoanInterestCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for WaiveLoanInterestCommandHandler {
    fn action_name(&self) -> &str {
        "WAIVEINTERESTPORTION"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let txn_date = command
            .local_date_value_of_parameter_named("transactionDate")?
            .unwrap_or(loan.expected_disbursement_date);
        let amt_dec = command
            .big_decimal_value_of_parameter_named("transactionAmount")?
            .unwrap_or(Decimal::ZERO);
        let amt = Money::of(loan.currency.clone(), amt_dec, FineractRoundingMode::HalfEven);
        let tx = loan.waive_interest(txn_date, amt, txn_date)?;
        let saved = self.repo.save_loan(loan).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(tx.id)
            .build())
    }
}

pub struct WriteOffLoanCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
    pub accounting_bridge: LoanAccountingBridgeAdapter,
}

#[async_trait]
impl CommandHandler for WriteOffLoanCommandHandler {
    fn action_name(&self) -> &str {
        "WRITEOFF"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let txn_date = command
            .local_date_value_of_parameter_named("transactionDate")?
            .unwrap_or(loan.expected_disbursement_date);
        let tx = loan.write_off(txn_date, txn_date)?;
        let journals = self.accounting_bridge.generate_entries_for_transaction(
            &loan,
            &tx,
            AccountingRuleType::AccrualPeriodic,
            LoanTransactionAccountingType::WriteOff,
        )?;
        self.repo.append_journal_entries(journals).await?;
        let saved = self.repo.save_loan(loan).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(tx.id)
            .build())
    }
}

pub struct ChargeOffLoanCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for ChargeOffLoanCommandHandler {
    fn action_name(&self) -> &str {
        "CHARGEOFF"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let txn_date = command
            .local_date_value_of_parameter_named("transactionDate")?
            .unwrap_or(loan.expected_disbursement_date);
        let tx = loan.charge_off(txn_date, txn_date, LoanChargeOffBehaviour::Regular)?;
        let saved = self.repo.save_loan(loan).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(tx.id)
            .build())
    }
}

pub struct LoanRecoveryPaymentCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
    pub accounting_bridge: LoanAccountingBridgeAdapter,
}

#[async_trait]
impl CommandHandler for LoanRecoveryPaymentCommandHandler {
    fn action_name(&self) -> &str {
        "RECOVERYPAYMENT"
    }
    fn entity_name(&self) -> &str {
        "LOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command.loan_id.or(command.entity_id).unwrap_or(1);
        let mut loan = self.repo.find_loan_by_id(loan_id).await?.ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {loan_id} not found"),
                resource_id: loan_id.to_string(),
            }
        })?;
        let txn_date = command
            .local_date_value_of_parameter_named("transactionDate")?
            .unwrap_or(loan.expected_disbursement_date);
        let amt_dec = command
            .big_decimal_value_of_parameter_named("transactionAmount")?
            .unwrap_or(Decimal::ZERO);
        let amt = Money::of(loan.currency.clone(), amt_dec, FineractRoundingMode::HalfEven);
        let tx = loan.recover_written_off_balance(txn_date, amt, txn_date)?;
        let journals = self.accounting_bridge.generate_entries_for_transaction(
            &loan,
            &tx,
            AccountingRuleType::AccrualPeriodic,
            LoanTransactionAccountingType::Recovery,
        )?;
        self.repo.append_journal_entries(journals).await?;
        let saved = self.repo.save_loan(loan).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(saved.id)
            .with_resource_id(tx.id)
            .build())
    }
}

pub struct CreateLoanRescheduleRequestCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for CreateLoanRescheduleRequestCommandHandler {
    fn action_name(&self) -> &str {
        "CREATE"
    }
    fn entity_name(&self) -> &str {
        "RESCHEDULELOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command
            .long_value_of_parameter_named("loanId")?
            .or(command.loan_id)
            .unwrap_or(1);
        let reschedule_from_date = command
            .local_date_value_of_parameter_named("rescheduleFromDate")?
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
        let adjusted_due_date =
            command.local_date_value_of_parameter_named("adjustedDueDate")?;
        let extra_terms = command
            .integer_value_of_parameter_named("extraTerms")?
            .map(|v| v as u32);
        let grace_on_principal = command
            .integer_value_of_parameter_named("graceOnPrincipal")?
            .map(|v| v as u32);
        let grace_on_interest = command
            .integer_value_of_parameter_named("graceOnInterest")?
            .map(|v| v as u32);
        let new_interest_rate =
            command.big_decimal_value_of_parameter_named("newInterestRate")?;

        let req = LoanRescheduleRequest {
            id: 0,
            loan_id,
            status_enum: 100, // Submitted
            reschedule_from_installment: command
                .integer_value_of_parameter_named("rescheduleFromInstallment")?
                .map(|v| v as u32),
            reschedule_from_date,
            adjusted_due_date,
            extra_terms,
            grace_on_principal,
            grace_on_interest,
            new_interest_rate,
            recalculate_interest: true,
            submitted_on_date: reschedule_from_date,
            approved_on_date: None,
            rejected_on_date: None,
        };
        let saved = self.repo.save_reschedule_request(req).await?;
        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(loan_id)
            .with_resource_id(saved.id)
            .build())
    }
}

pub struct ApproveLoanRescheduleRequestCommandHandler {
    pub repo: Arc<dyn LoanRepository>,
}

#[async_trait]
impl CommandHandler for ApproveLoanRescheduleRequestCommandHandler {
    fn action_name(&self) -> &str {
        "APPROVE"
    }
    fn entity_name(&self) -> &str {
        "RESCHEDULELOAN"
    }
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let req_id = command.entity_id.unwrap_or(1);
        let mut req = self
            .repo
            .find_reschedule_request_by_id(req_id)
            .await?
            .ok_or_else(|| FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.reschedule.id.invalid".to_string(),
                default_message: format!("Reschedule request {req_id} not found"),
                resource_id: req_id.to_string(),
            })?;
        let mut loan = self
            .repo
            .find_loan_by_id(req.loan_id)
            .await?
            .ok_or_else(|| FineractError::ResourceNotFound {
                globalisation_code: "error.msg.loan.id.invalid".to_string(),
                default_message: format!("Loan {} not found", req.loan_id),
                resource_id: req.loan_id.to_string(),
            })?;

        LoanRescheduleEngine::apply_reschedule(
            &mut loan.installments,
            &req,
            &loan.currency,
            loan.annual_nominal_interest_rate,
            FineractRoundingMode::HalfEven,
        )?;
        loan.summary = LoanSummary::from_installments(&loan.currency, &loan.installments);
        req.status_enum = 200; // Approved
        req.approved_on_date = Some(req.reschedule_from_date);
        self.repo.save_reschedule_request(req).await?;
        self.repo.save_loan(loan.clone()).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_loan_id(loan.id)
            .with_resource_id(req_id)
            .build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_end_to_end_loan_lifecycle_and_balanced_gl_journals() {
        let repo = Arc::new(DualDbLoanRepository::new_in_memory(SqlDialect::Postgres));
        let bridge = LoanAccountingBridgeAdapter::default();
        let ctx = ExecutionContext::default_for_tenant("default");

        // 1. Create Loan Product
        let create_prod = CreateLoanProductCommandHandler { repo: repo.clone() };
        let prod_cmd = JsonCommand::from_json(
            Some(1),
            r#"{"name":"Micro Loan","shortName":"ML01","currencyCode":"USD","digitsAfterDecimal":2,"principal":1200,"interestRatePerPeriod":12,"numberOfRepayments":6,"locale":"en"}"#,
        )
        .unwrap();
        let prod_res = create_prod.handle(&ctx, &prod_cmd).await.unwrap();
        let prod_id = prod_res.resource_id.unwrap();

        // 2. Submit Loan Application
        let submit_loan = SubmitLoanApplicationCommandHandler { repo: repo.clone() };
        let submit_cmd = JsonCommand::from_json(
            Some(2),
            &format!(
                r#"{{"clientId":10,"productId":{prod_id},"principal":1200,"interestRatePerPeriod":12,"numberOfRepayments":6,"submittedOnDate":"2026-01-01","expectedDisbursementDate":"2026-01-01","dateFormat":"yyyy-MM-dd","locale":"en"}}"#
            ),
        )
        .unwrap();
        let submit_res = submit_loan.handle(&ctx, &submit_cmd).await.unwrap();
        let loan_id = submit_res.loan_id.unwrap();

        // 3. Approve Loan
        let approve_loan = ApproveLoanApplicationCommandHandler { repo: repo.clone() };
        let mut approve_cmd = JsonCommand::from_json(
            Some(3),
            r#"{"approvedOnDate":"2026-01-01","dateFormat":"yyyy-MM-dd","locale":"en"}"#,
        )
        .unwrap();
        approve_cmd.loan_id = Some(loan_id);
        approve_loan.handle(&ctx, &approve_cmd).await.unwrap();

        // 4. Disburse Loan
        let disburse_loan = DisburseLoanCommandHandler {
            repo: repo.clone(),
            accounting_bridge: bridge.clone(),
        };
        let mut disburse_cmd = JsonCommand::from_json(
            Some(4),
            r#"{"actualDisbursementDate":"2026-01-01","dateFormat":"yyyy-MM-dd","locale":"en"}"#,
        )
        .unwrap();
        disburse_cmd.loan_id = Some(loan_id);
        disburse_loan.handle(&ctx, &disburse_cmd).await.unwrap();

        let loan_after_disb = repo.find_loan_by_id(loan_id).await.unwrap().unwrap();
        assert_eq!(loan_after_disb.status, LoanStatus::Active);

        // 5. Make Repayment + Goodwill Credit to pay off full balance
        let total_due = loan_after_disb.summary.total_outstanding.amount();
        let repay_handler = LoanRepaymentCommandHandler {
            repo: repo.clone(),
            accounting_bridge: bridge.clone(),
        };
        let mut repay_cmd = JsonCommand::from_json(
            Some(5),
            &format!(
                r#"{{"transactionDate":"2026-02-01","transactionAmount":500,"dateFormat":"yyyy-MM-dd","locale":"en"}}"#
            ),
        )
        .unwrap();
        repay_cmd.loan_id = Some(loan_id);
        repay_handler.handle(&ctx, &repay_cmd).await.unwrap();

        let remaining = total_due - dec!(500);
        let goodwill_handler = LoanGoodwillCreditCommandHandler {
            repo: repo.clone(),
            accounting_bridge: bridge.clone(),
        };
        let mut gw_cmd = JsonCommand::from_json(
            Some(6),
            &format!(
                r#"{{"transactionDate":"2026-02-15","transactionAmount":{remaining},"dateFormat":"yyyy-MM-dd","locale":"en"}}"#
            ),
        )
        .unwrap();
        gw_cmd.loan_id = Some(loan_id);
        goodwill_handler.handle(&ctx, &gw_cmd).await.unwrap();

        let final_loan = repo.find_loan_by_id(loan_id).await.unwrap().unwrap();
        assert_eq!(final_loan.status, LoanStatus::ClosedObligationsMet);
        assert!(final_loan.summary.total_outstanding.is_zero());

        // Verify balanced GL journal entries
        let journals = repo.list_journal_entries_for_loan(loan_id).await.unwrap();
        assert!(!journals.is_empty());
        let total_debits: Decimal = journals
            .iter()
            .filter(|j| j.type_enum == 2)
            .map(|j| j.amount)
            .sum();
        let total_credits: Decimal = journals
            .iter()
            .filter(|j| j.type_enum == 1)
            .map(|j| j.amount)
            .sum();
        assert_eq!(total_debits, total_credits);
    }
}
