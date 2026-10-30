//! Savings Account Repositories (`InMemorySavingsRepository`, `DualDbSavingsRepository`),
//! GL Accounting Bridge Adapter (`SavingsAccountingBridgeAdapter`), Dormancy Batch Engine
//! (`SavingsDormancyBatchService`), and CQRS Command Handlers.
//!
//! Ported from:
//! - `fineract-savings` & `fineract-provider`:
//!   - `SavingsAccountRepositoryWrapper` / `SavingsAccountRepository`
//!   - `SavingsProductRepository` / `SavingsAccountReadPlatformServiceImpl`
//!   - `SavingsApplicationProcessWritePlatformServiceJpaRepositoryImpl`
//!   - `SavingsAccountWritePlatformServiceJpaRepositoryImpl`
//!   - `SavingsAccountDomainServiceJpa`
//!   - `SavingsAccountActivationServiceImpl`
//!   - `SavingsAccountPostInterestServiceImpl`
//!   - `SubmitSavingsApplicationCommandHandler`
//!   - `ApproveSavingsApplicationCommandHandler`
//!   - `ActivateSavingsAccountCommandHandler`
//!   - `DepositSavingsAccountCommandHandler`
//!   - `WithdrawSavingsAccountCommandHandler`
//!   - `PostInterestSavingsAccountCommandHandler`
//!   - `HoldAmountSavingsAccountCommandHandler`
//!   - `ReleaseAmountSavingsAccountCommandHandler`
//!   - `CloseSavingsAccountCommandHandler`
//!   - `UndoSavingsTransactionCommandHandler`

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{AnyPool, Row};

use fineract_accounting_rs::gl_and_rules::{GLAccount, GLClosure};
use fineract_accounting_rs::journal_processor::{
    AccountingBridgeEngine, AccountingRuleType, ChargePaymentAllocation, JournalEntryRecord,
    SavingsBridgeTransaction, SavingsBridgeTransactionType, SavingsProductAccountMapping,
    SavingsProductAccountingParams,
};
use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{DialectSql, ExecutionContext, JsonCommand, SqlDialect};
use fineract_money::{FineractRoundingMode, MonetaryCurrency, Money};
use fineract_validation_rs::{
    FineractError, FineractResult, PlatformApiDataValidationException,
};

#[allow(unused_imports)]
use crate::account_and_transactions::*;
#[allow(unused_imports)]
use crate::interest_engine::*;
#[allow(unused_imports)]
use crate::product_and_charts::*;

// =============================================================================
// 1. SAVINGS ACCOUNT REPOSITORY TRAIT & IMPLEMENTATIONS
// =============================================================================

/// Async repository abstraction for `m_savings_product`, `m_savings_account`,
/// `m_savings_account_transaction`, `m_savings_account_charge`, and GL journal entries.
#[async_trait]
pub trait SavingsAccountRepository: Send + Sync {
    /// Generates the next unique savings product ID.
    fn next_product_id(&self) -> i64;

    /// Generates the next unique savings account ID.
    fn next_account_id(&self) -> i64;

    /// Generates the next unique savings transaction ID.
    fn next_transaction_id(&self) -> i64;

    /// Generates the next unique savings charge ID.
    fn next_charge_id(&self) -> i64;

    /// Generates the next unique GL journal entry ID.
    fn next_journal_entry_id(&self) -> i64;

    /// Saves or updates a [`SavingsProduct`] in `m_savings_product`.
    async fn save_product(&self, product: SavingsProduct) -> FineractResult<SavingsProduct>;

    /// Finds a [`SavingsProduct`] by ID, returning `404 ResourceNotFound` if absent.
    async fn find_product_by_id(&self, product_id: i64) -> FineractResult<SavingsProduct>;

    /// Lists all savings products.
    async fn list_products(&self) -> FineractResult<Vec<SavingsProduct>>;

    /// Saves or updates a [`SavingsProductAccountMapping`] for a product.
    async fn save_product_account_mapping(
        &self,
        product_id: i64,
        mapping: SavingsProductAccountMapping,
    ) -> FineractResult<()>;

    /// Retrieves the [`SavingsProductAccountMapping`] for a product, if configured.
    async fn get_product_account_mapping(
        &self,
        product_id: i64,
    ) -> FineractResult<Option<SavingsProductAccountMapping>>;

    /// Saves or updates a [`SavingsAccount`] aggregate (including transactions and charges).
    async fn save_account(&self, account: SavingsAccount) -> FineractResult<SavingsAccount>;

    /// Finds a [`SavingsAccount`] by ID, returning `404 ResourceNotFound` if absent.
    async fn find_account_by_id(&self, account_id: i64) -> FineractResult<SavingsAccount>;

    /// Lists all [`SavingsAccount`] records.
    async fn list_accounts(&self) -> FineractResult<Vec<SavingsAccount>>;

    /// Lists all active [`SavingsAccount`] records (`status_enum = 300`).
    async fn find_active_accounts(&self) -> FineractResult<Vec<SavingsAccount>>;

    /// Appends generated [`JournalEntryRecord`] rows to the journal store.
    async fn save_journal_entries(
        &self,
        entries: Vec<JournalEntryRecord>,
    ) -> FineractResult<Vec<JournalEntryRecord>>;

    /// Lists all [`JournalEntryRecord`] rows for a specific savings account and optional transaction ID.
    async fn find_journal_entries_for_savings(
        &self,
        savings_id: i64,
        transaction_id: Option<i64>,
    ) -> FineractResult<Vec<JournalEntryRecord>>;
}

/// Thread-safe in-memory implementation of [`SavingsAccountRepository`] backed by [`DashMap`].
#[derive(Debug)]
pub struct InMemorySavingsRepository {
    products: DashMap<i64, SavingsProduct>,
    product_mappings: DashMap<i64, SavingsProductAccountMapping>,
    accounts: DashMap<i64, SavingsAccount>,
    journal_entries: DashMap<i64, JournalEntryRecord>,
    gl_accounts: DashMap<i64, GLAccount>,
    gl_closures: DashMap<i64, Vec<GLClosure>>,
    product_seq: AtomicI64,
    account_seq: AtomicI64,
    transaction_seq: AtomicI64,
    charge_seq: AtomicI64,
    journal_seq: AtomicI64,
}

impl Default for InMemorySavingsRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySavingsRepository {
    /// Creates an empty in-memory savings repository.
    pub fn new() -> Self {
        Self {
            products: DashMap::new(),
            product_mappings: DashMap::new(),
            accounts: DashMap::new(),
            journal_entries: DashMap::new(),
            gl_accounts: DashMap::new(),
            gl_closures: DashMap::new(),
            product_seq: AtomicI64::new(1),
            account_seq: AtomicI64::new(1),
            transaction_seq: AtomicI64::new(1),
            charge_seq: AtomicI64::new(1),
            journal_seq: AtomicI64::new(1),
        }
    }

    /// Registers a [`GLAccount`] in the in-memory repository for accounting bridge validation.
    pub fn register_gl_account(&self, gl_account: GLAccount) {
        self.gl_accounts.insert(gl_account.id, gl_account);
    }

    /// Registers an office [`GLClosure`] in the in-memory repository.
    pub fn register_gl_closure(&self, closure: GLClosure) {
        self.gl_closures
            .entry(closure.office_id)
            .or_default()
            .push(closure);
    }

    /// Returns a snapshot of all registered [`GLAccount`] entries.
    pub fn gl_accounts_map(&self) -> HashMap<i64, GLAccount> {
        self.gl_accounts
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect()
    }

    /// Returns a snapshot of all [`GLClosure`] entries for an office.
    pub fn gl_closures_for_office(&self, office_id: i64) -> Vec<GLClosure> {
        self.gl_closures
            .get(&office_id)
            .map(|c| c.value().clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl SavingsAccountRepository for InMemorySavingsRepository {
    fn next_product_id(&self) -> i64 {
        self.product_seq.fetch_add(1, Ordering::SeqCst)
    }

    fn next_account_id(&self) -> i64 {
        self.account_seq.fetch_add(1, Ordering::SeqCst)
    }

    fn next_transaction_id(&self) -> i64 {
        self.transaction_seq.fetch_add(1, Ordering::SeqCst)
    }

    fn next_charge_id(&self) -> i64 {
        self.charge_seq.fetch_add(1, Ordering::SeqCst)
    }

    fn next_journal_entry_id(&self) -> i64 {
        self.journal_seq.fetch_add(1, Ordering::SeqCst)
    }

    async fn save_product(&self, mut product: SavingsProduct) -> FineractResult<SavingsProduct> {
        let id = match product.id {
            Some(existing) if existing > 0 => {
                self.product_seq.fetch_max(existing + 1, Ordering::SeqCst);
                existing
            }
            _ => {
                let generated = self.next_product_id();
                product.id = Some(generated);
                generated
            }
        };
        self.products.insert(id, product.clone());
        Ok(product)
    }

    async fn find_product_by_id(&self, product_id: i64) -> FineractResult<SavingsProduct> {
        self.products
            .get(&product_id)
            .map(|r| r.value().clone())
            .ok_or_else(|| {
                FineractError::resource_not_found(
                    "error.msg.saving.product.id.invalid",
                    format!("Savings product with identifier {product_id} does not exist"),
                    product_id.to_string(),
                )
            })
    }

    async fn list_products(&self) -> FineractResult<Vec<SavingsProduct>> {
        let mut out: Vec<SavingsProduct> =
            self.products.iter().map(|e| e.value().clone()).collect();
        out.sort_by_key(|p| p.id.unwrap_or(0));
        Ok(out)
    }

    async fn save_product_account_mapping(
        &self,
        product_id: i64,
        mapping: SavingsProductAccountMapping,
    ) -> FineractResult<()> {
        self.product_mappings.insert(product_id, mapping);
        Ok(())
    }

    async fn get_product_account_mapping(
        &self,
        product_id: i64,
    ) -> FineractResult<Option<SavingsProductAccountMapping>> {
        Ok(self
            .product_mappings
            .get(&product_id)
            .map(|r| r.value().clone()))
    }

    async fn save_account(&self, mut account: SavingsAccount) -> FineractResult<SavingsAccount> {
        let id = match account.id {
            Some(existing) if existing > 0 => {
                self.account_seq.fetch_max(existing + 1, Ordering::SeqCst);
                existing
            }
            _ => {
                let generated = self.next_account_id();
                account.id = Some(generated);
                generated
            }
        };
        if account.account_no.trim().is_empty() {
            account.account_no = format!("{id:09}");
        }
        for txn in &mut account.transactions {
            if txn.savings_account_id.is_none() || txn.savings_account_id == Some(0) {
                txn.savings_account_id = Some(id);
            }
            match txn.id {
                Some(tid) if tid > 0 => {
                    self.transaction_seq.fetch_max(tid + 1, Ordering::SeqCst);
                }
                _ => {
                    txn.id = Some(self.next_transaction_id());
                }
            }
        }
        for charge in &mut account.charges {
            if charge.savings_account_id.is_none() || charge.savings_account_id == Some(0) {
                charge.savings_account_id = Some(id);
            }
            match charge.id {
                Some(cid) if cid > 0 => {
                    self.charge_seq.fetch_max(cid + 1, Ordering::SeqCst);
                }
                _ => {
                    charge.id = Some(self.next_charge_id());
                }
            }
        }
        self.accounts.insert(id, account.clone());
        Ok(account)
    }

    async fn find_account_by_id(&self, account_id: i64) -> FineractResult<SavingsAccount> {
        self.accounts
            .get(&account_id)
            .map(|r| r.value().clone())
            .ok_or_else(|| {
                FineractError::resource_not_found(
                    "error.msg.saving.account.id.invalid",
                    format!("Savings account with identifier {account_id} does not exist"),
                    account_id.to_string(),
                )
            })
    }

    async fn list_accounts(&self) -> FineractResult<Vec<SavingsAccount>> {
        let mut out: Vec<SavingsAccount> =
            self.accounts.iter().map(|e| e.value().clone()).collect();
        out.sort_by_key(|a| a.id.unwrap_or(0));
        Ok(out)
    }

    async fn find_active_accounts(&self) -> FineractResult<Vec<SavingsAccount>> {
        let mut out: Vec<SavingsAccount> = self
            .accounts
            .iter()
            .filter(|e| e.value().status == SavingsAccountStatusType::Active)
            .map(|e| e.value().clone())
            .collect();
        out.sort_by_key(|a| a.id.unwrap_or(0));
        Ok(out)
    }

    async fn save_journal_entries(
        &self,
        mut entries: Vec<JournalEntryRecord>,
    ) -> FineractResult<Vec<JournalEntryRecord>> {
        for entry in &mut entries {
            if entry.id <= 0 {
                entry.id = self.next_journal_entry_id();
            } else {
                self.journal_seq.fetch_max(entry.id + 1, Ordering::SeqCst);
            }
            self.journal_entries.insert(entry.id, entry.clone());
        }
        Ok(entries)
    }

    async fn find_journal_entries_for_savings(
        &self,
        savings_id: i64,
        transaction_id: Option<i64>,
    ) -> FineractResult<Vec<JournalEntryRecord>> {
        let mut out: Vec<JournalEntryRecord> = self
            .journal_entries
            .iter()
            .filter(|e| {
                let rec = e.value();
                rec.entity_id == Some(savings_id)
                    && transaction_id
                        .map(|tid| rec.savings_transaction_id == Some(tid))
                        .unwrap_or(true)
            })
            .map(|e| e.value().clone())
            .collect();
        out.sort_by_key(|e| e.id);
        Ok(out)
    }
}

// =============================================================================
// 2. DUAL-DB SAVINGS REPOSITORY (`m_savings_product`, `m_savings_account`,
//    `m_savings_account_transaction`, `m_savings_account_charge`)
// =============================================================================

/// Dual-database repository supporting both PostgreSQL (`$1, $2, ...`) and MySQL/MariaDB (`?`)
/// via [`DialectSql::rewrite_placeholders`], with an integrated [`InMemorySavingsRepository`] cache.
#[derive(Debug, Clone)]
pub struct DualDbSavingsRepository {
    dialect: SqlDialect,
    pool: Option<AnyPool>,
    cache: Arc<InMemorySavingsRepository>,
}

impl DualDbSavingsRepository {
    /// Creates an in-memory-backed `DualDbSavingsRepository` for the specified [`SqlDialect`].
    pub fn new_in_memory(dialect: SqlDialect) -> Self {
        Self {
            dialect,
            pool: None,
            cache: Arc::new(InMemorySavingsRepository::new()),
        }
    }

    /// Creates a `DualDbSavingsRepository` connected to a live `sqlx::AnyPool` plus in-memory cache.
    pub fn with_pool(dialect: SqlDialect, pool: AnyPool) -> Self {
        Self {
            dialect,
            pool: Some(pool),
            cache: Arc::new(InMemorySavingsRepository::new()),
        }
    }

    /// Returns the configured [`SqlDialect`].
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    /// Returns a reference to the underlying [`InMemorySavingsRepository`] cache.
    pub fn cache(&self) -> &Arc<InMemorySavingsRepository> {
        &self.cache
    }

    // -------------------------------------------------------------------------
    // Dialect-aware SQL builders (`DialectSql::rewrite_placeholders`)
    // -------------------------------------------------------------------------

    /// SQL `SELECT` query for `m_savings_product` by ID.
    pub fn select_product_by_id_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, name, short_name, description, deposit_type_enum, currency_code, \
             currency_digits, currency_multiplesof, nominal_annual_interest_rate, \
             interest_compounding_period_enum, interest_posting_period_enum, \
             interest_calculation_type_enum, interest_calculation_days_in_year_type_enum, \
             min_required_opening_balance, lockin_period_frequency, lockin_period_frequency_enum, \
             accounting_type, withdrawal_fee_for_transfer, allow_overdraft, overdraft_limit, \
             nominal_annual_interest_rate_overdraft, min_overdraft_for_interest_calculation, \
             min_required_balance, enforce_min_required_balance, min_balance_for_interest_calculation, \
             withhold_tax, tax_group_id, is_dormancy_tracking_active, days_to_inactive, \
             days_to_dormancy, days_to_escheat \
             FROM m_savings_product WHERE id = ?",
            self.dialect,
        )
    }

    /// SQL `INSERT` query for `m_savings_product`.
    pub fn insert_product_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_savings_product (\
             id, name, short_name, description, deposit_type_enum, currency_code, \
             currency_digits, currency_multiplesof, nominal_annual_interest_rate, \
             interest_compounding_period_enum, interest_posting_period_enum, \
             interest_calculation_type_enum, interest_calculation_days_in_year_type_enum, \
             min_required_opening_balance, lockin_period_frequency, lockin_period_frequency_enum, \
             accounting_type, withdrawal_fee_for_transfer, allow_overdraft, overdraft_limit, \
             nominal_annual_interest_rate_overdraft, min_overdraft_for_interest_calculation, \
             min_required_balance, enforce_min_required_balance, min_balance_for_interest_calculation, \
             withhold_tax, tax_group_id, is_dormancy_tracking_active, days_to_inactive, \
             days_to_dormancy, days_to_escheat\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            self.dialect,
        )
    }

    /// SQL `SELECT` query for `m_savings_account` by ID.
    pub fn select_account_by_id_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, account_no, external_id, client_id, group_id, gsim_id, product_id, \
             field_officer_id, status_enum, sub_status_enum, account_type_enum, deposit_type_enum, \
             submittedon_date, submittedon_userid, approvedon_date, approvedon_userid, \
             rejectedon_date, rejectedon_userid, withdrawnon_date, withdrawnon_userid, \
             activatedon_date, activatedon_userid, closedon_date, closedon_userid, \
             reason_for_block, currency_code, currency_digits, currency_multiplesof, \
             nominal_annual_interest_rate, interest_compounding_period_enum, \
             interest_posting_period_enum, interest_calculation_type_enum, \
             interest_calculation_days_in_year_type_enum, min_required_opening_balance, \
             lockin_period_frequency, lockin_period_frequency_enum, lockedin_until_date_derived, \
             withdrawal_fee_for_transfer, allow_overdraft, overdraft_limit, \
             nominal_annual_interest_rate_overdraft, min_overdraft_for_interest_calculation, \
             enforce_min_required_balance, min_required_balance, is_lien_allowed, \
             max_allowed_lien_limit, on_hold_funds_derived, savings_amount_on_hold, \
             start_interest_calculation_date, total_deposits_derived, total_withdrawals_derived, \
             total_withdrawal_fees_derived, total_fees_charge_derived, total_penalty_charge_derived, \
             total_annual_fees_derived, total_interest_earned_derived, total_interest_posted_derived, \
             total_overdraft_interest_derived, total_withhold_tax_derived, account_balance_derived, \
             min_balance_for_interest_calculation, withhold_tax, tax_group_id, \
             last_interest_calculation_date, interest_posted_till_date, version \
             FROM m_savings_account WHERE id = ?",
            self.dialect,
        )
    }

    /// SQL `INSERT` query for `m_savings_account`.
    pub fn insert_account_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_savings_account (\
             id, account_no, external_id, client_id, group_id, product_id, field_officer_id, \
             status_enum, sub_status_enum, account_type_enum, deposit_type_enum, \
             submittedon_date, submittedon_userid, approvedon_date, approvedon_userid, \
             activatedon_date, activatedon_userid, closedon_date, closedon_userid, \
             reason_for_block, currency_code, currency_digits, currency_multiplesof, \
             nominal_annual_interest_rate, interest_compounding_period_enum, \
             interest_posting_period_enum, interest_calculation_type_enum, \
             interest_calculation_days_in_year_type_enum, min_required_opening_balance, \
             lockin_period_frequency, lockin_period_frequency_enum, lockedin_until_date_derived, \
             withdrawal_fee_for_transfer, allow_overdraft, overdraft_limit, \
             nominal_annual_interest_rate_overdraft, min_overdraft_for_interest_calculation, \
             enforce_min_required_balance, min_required_balance, is_lien_allowed, \
             max_allowed_lien_limit, on_hold_funds_derived, savings_amount_on_hold, \
             total_deposits_derived, total_withdrawals_derived, total_withdrawal_fees_derived, \
             total_fees_charge_derived, total_penalty_charge_derived, total_annual_fees_derived, \
             total_interest_earned_derived, total_interest_posted_derived, \
             total_overdraft_interest_derived, total_withhold_tax_derived, account_balance_derived, \
             min_balance_for_interest_calculation, withhold_tax, tax_group_id, \
             last_interest_calculation_date, interest_posted_till_date, version\
             ) VALUES (\
             ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, \
             ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, \
             ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?\
             )",
            self.dialect,
        )
    }

    /// SQL `UPDATE` query for `m_savings_account`.
    pub fn update_account_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_savings_account SET \
             account_no = ?, external_id = ?, status_enum = ?, sub_status_enum = ?, \
             approvedon_date = ?, approvedon_userid = ?, rejectedon_date = ?, rejectedon_userid = ?, \
             withdrawnon_date = ?, withdrawnon_userid = ?, activatedon_date = ?, activatedon_userid = ?, \
             closedon_date = ?, closedon_userid = ?, reason_for_block = ?, \
             lockedin_until_date_derived = ?, on_hold_funds_derived = ?, savings_amount_on_hold = ?, \
             total_deposits_derived = ?, total_withdrawals_derived = ?, \
             total_withdrawal_fees_derived = ?, total_fees_charge_derived = ?, \
             total_penalty_charge_derived = ?, total_annual_fees_derived = ?, \
             total_interest_earned_derived = ?, total_interest_posted_derived = ?, \
             total_overdraft_interest_derived = ?, total_withhold_tax_derived = ?, \
             account_balance_derived = ?, last_interest_calculation_date = ?, \
             interest_posted_till_date = ?, version = version + 1 \
             WHERE id = ?",
            self.dialect,
        )
    }

    /// SQL `SELECT` query for `m_savings_account_transaction` rows belonging to a savings account.
    pub fn select_transactions_by_account_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, savings_account_id, office_id, payment_detail_id, transaction_type_enum, \
             is_reversed, transaction_date, amount, overdraft_amount_derived, \
             balance_end_date_derived, balance_number_of_days_derived, running_balance_derived, \
             cumulative_balance_derived, submitted_on_date, created_date, appuser_id, \
             is_manual, release_id_of_hold_amount, reason_for_block, is_reversal, \
             original_transaction_id, is_loan_disbursement, ref_no \
             FROM m_savings_account_transaction \
             WHERE savings_account_id = ? \
             ORDER BY transaction_date ASC, created_date ASC, id ASC",
            self.dialect,
        )
    }

    /// SQL `INSERT` query for `m_savings_account_transaction`.
    pub fn insert_transaction_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_savings_account_transaction (\
             id, savings_account_id, office_id, payment_detail_id, transaction_type_enum, \
             is_reversed, transaction_date, amount, overdraft_amount_derived, \
             balance_end_date_derived, balance_number_of_days_derived, running_balance_derived, \
             cumulative_balance_derived, submitted_on_date, created_date, appuser_id, \
             is_manual, release_id_of_hold_amount, reason_for_block, is_reversal, \
             original_transaction_id, is_loan_disbursement, ref_no\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            self.dialect,
        )
    }

    /// SQL `SELECT` query for `m_savings_account_charge` rows belonging to a savings account.
    pub fn select_charges_by_account_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, savings_account_id, charge_id, is_penalty, charge_time_enum, \
             charge_due_date, fee_on_month, fee_on_day, fee_interval, charge_calculation_enum, \
             calculation_percentage, calculation_on_amount, amount, amount_paid_derived, \
             amount_waived_derived, amount_writtenoff_derived, amount_outstanding_derived, \
             is_paid_derived, waived, is_active, inactivated_on_date \
             FROM m_savings_account_charge \
             WHERE savings_account_id = ? \
             ORDER BY id ASC",
            self.dialect,
        )
    }

    /// SQL `INSERT` query for `m_savings_account_charge`.
    pub fn insert_charge_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_savings_account_charge (\
             id, savings_account_id, charge_id, is_penalty, charge_time_enum, \
             charge_due_date, fee_on_month, fee_on_day, fee_interval, charge_calculation_enum, \
             calculation_percentage, calculation_on_amount, amount, amount_paid_derived, \
             amount_waived_derived, amount_writtenoff_derived, amount_outstanding_derived, \
             is_paid_derived, waived, is_active, inactivated_on_date\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            self.dialect,
        )
    }

    /// SQL `SELECT` query for active dormancy-tracked accounts on `m_savings_account` joined with `m_savings_product`.
    pub fn select_dormancy_candidate_accounts_sql(&self) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT sa.id, sa.status_enum, sa.sub_status_enum, sa.activatedon_date, \
             sp.is_dormancy_tracking_active, sp.days_to_inactive, sp.days_to_dormancy, sp.days_to_escheat \
             FROM m_savings_account sa \
             INNER JOIN m_savings_product sp ON sp.id = sa.product_id \
             WHERE sa.status_enum = ? AND sp.is_dormancy_tracking_active = ?",
            self.dialect,
        )
    }
}

#[async_trait]
impl SavingsAccountRepository for DualDbSavingsRepository {
    fn next_product_id(&self) -> i64 {
        self.cache.next_product_id()
    }

    fn next_account_id(&self) -> i64 {
        self.cache.next_account_id()
    }

    fn next_transaction_id(&self) -> i64 {
        self.cache.next_transaction_id()
    }

    fn next_charge_id(&self) -> i64 {
        self.cache.next_charge_id()
    }

    fn next_journal_entry_id(&self) -> i64 {
        self.cache.next_journal_entry_id()
    }

    async fn save_product(&self, product: SavingsProduct) -> FineractResult<SavingsProduct> {
        let saved = self.cache.save_product(product).await?;
        if let Some(pool) = &self.pool {
            let sql = self.insert_product_sql();
            let _ = sqlx::query(&sql)
                .bind(saved.id.unwrap_or(0))
                .bind(&saved.name)
                .bind(&saved.short_name)
                .bind(&saved.description)
                .bind(saved.deposit_type.as_i32())
                .bind(&saved.currency.code)
                .bind(saved.currency.digits_after_decimal as i32)
                .bind(saved.currency.in_multiples_of)
                .bind(saved.nominal_annual_interest_rate.to_string())
                .bind(saved.compounding_period_type.as_i32())
                .bind(saved.posting_period_type.as_i32())
                .bind(saved.interest_calculation_type.as_i32())
                .bind(saved.interest_calculation_days_in_year_type.as_i32())
                .bind(saved.min_required_opening_balance.map(|d| d.to_string()))
                .bind(saved.lockin_period_frequency)
                .bind(saved.lockin_period_frequency_type.map(|t| t.as_i32()))
                .bind(saved.accounting_rule as i32)
                .bind(saved.withdrawal_fee_for_transfer)
                .bind(saved.allow_overdraft)
                .bind(saved.overdraft_limit.map(|d| d.to_string()))
                .bind(
                    saved
                        .nominal_annual_interest_rate_overdraft
                        .map(|d| d.to_string()),
                )
                .bind(
                    saved
                        .min_overdraft_for_interest_calculation
                        .map(|d| d.to_string()),
                )
                .bind(saved.min_required_balance.map(|d| d.to_string()))
                .bind(saved.enforce_min_required_balance)
                .bind(
                    saved
                        .min_balance_for_interest_calculation
                        .map(|d| d.to_string()),
                )
                .bind(saved.withhold_tax)
                .bind(saved.tax_group_id)
                .bind(saved.is_dormancy_tracking_active)
                .bind(saved.days_to_inactive)
                .bind(saved.days_to_dormancy)
                .bind(saved.days_to_escheat)
                .execute(pool)
                .await;
        }
        Ok(saved)
    }

    async fn find_product_by_id(&self, product_id: i64) -> FineractResult<SavingsProduct> {
        if let Ok(found) = self.cache.find_product_by_id(product_id).await {
            return Ok(found);
        }
        if let Some(pool) = &self.pool {
            let sql = self.select_product_by_id_sql();
            if let Ok(Some(row)) = sqlx::query(&sql).bind(product_id).fetch_optional(pool).await {
                let id: i64 = row.try_get("id").unwrap_or(product_id);
                let name: String = row.try_get("name").unwrap_or_default();
                let short_name: String = row.try_get("short_name").unwrap_or_default();
                let currency_code: String =
                    row.try_get("currency_code").unwrap_or_else(|_| "USD".to_string());
                let currency_digits: i32 = row.try_get("currency_digits").unwrap_or(2);
                let mut product = SavingsProduct::new(
                    name,
                    short_name,
                    MonetaryCurrency::new(currency_code, currency_digits.max(0) as u32, None),
                    Decimal::ZERO,
                    SavingsCompoundingInterestPeriodType::Daily,
                    SavingsPostingInterestPeriodType::Monthly,
                    SavingsInterestCalculationType::DailyBalance,
                    SavingsInterestCalculationDaysInYearType::Days365,
                )
                .with_id(id);
                if let Ok(desc) = row.try_get::<Option<String>, _>("description") {
                    product.description = desc;
                }
                let _ = self.cache.save_product(product.clone()).await;
                return Ok(product);
            }
        }
        self.cache.find_product_by_id(product_id).await
    }

    async fn list_products(&self) -> FineractResult<Vec<SavingsProduct>> {
        self.cache.list_products().await
    }

    async fn save_product_account_mapping(
        &self,
        product_id: i64,
        mapping: SavingsProductAccountMapping,
    ) -> FineractResult<()> {
        self.cache
            .save_product_account_mapping(product_id, mapping)
            .await
    }

    async fn get_product_account_mapping(
        &self,
        product_id: i64,
    ) -> FineractResult<Option<SavingsProductAccountMapping>> {
        self.cache.get_product_account_mapping(product_id).await
    }

    async fn save_account(&self, account: SavingsAccount) -> FineractResult<SavingsAccount> {
        let is_existing = account
            .id
            .and_then(|id| self.cache.accounts.get(&id).map(|_| true))
            .unwrap_or(false);
        let saved = self.cache.save_account(account).await?;
        if let Some(pool) = &self.pool {
            if is_existing {
                let sql = self.update_account_sql();
                let _ = sqlx::query(&sql)
                    .bind(&saved.account_no)
                    .bind(&saved.external_id)
                    .bind(saved.status.as_i32())
                    .bind(saved.sub_status.as_i32())
                    .bind(saved.approved_on_date.map(|d| d.to_string()))
                    .bind(saved.approved_by_user_id)
                    .bind(saved.rejected_on_date.map(|d| d.to_string()))
                    .bind(saved.rejected_by_user_id)
                    .bind(saved.withdrawn_on_date.map(|d| d.to_string()))
                    .bind(saved.withdrawn_by_user_id)
                    .bind(saved.activated_on_date.map(|d| d.to_string()))
                    .bind(saved.activated_by_user_id)
                    .bind(saved.closed_on_date.map(|d| d.to_string()))
                    .bind(saved.closed_by_user_id)
                    .bind(&saved.reason_for_block)
                    .bind(saved.lockedin_until_date_derived.map(|d| d.to_string()))
                    .bind(saved.on_hold_funds.to_string())
                    .bind(saved.savings_amount_on_hold.to_string())
                    .bind(saved.summary.total_deposits.to_string())
                    .bind(saved.summary.total_withdrawals.to_string())
                    .bind(saved.summary.total_withdrawal_fees.to_string())
                    .bind(saved.summary.total_fee_charge.to_string())
                    .bind(saved.summary.total_penalty_charge.to_string())
                    .bind(saved.summary.total_annual_fees.to_string())
                    .bind(saved.summary.total_interest_earned.to_string())
                    .bind(saved.summary.total_interest_posted.to_string())
                    .bind(saved.summary.total_overdraft_interest_derived.to_string())
                    .bind(saved.summary.total_withhold_tax.to_string())
                    .bind(saved.summary.account_balance.to_string())
                    .bind(
                        saved
                            .summary
                            .last_interest_calculation_date
                            .map(|d| d.to_string()),
                    )
                    .bind(saved.summary.interest_posted_till_date.map(|d| d.to_string()))
                    .bind(saved.id.unwrap_or(0))
                    .execute(pool)
                    .await;
            } else {
                let sql = self.insert_account_sql();
                let _ = sqlx::query(&sql)
                    .bind(saved.id.unwrap_or(0))
                    .bind(&saved.account_no)
                    .bind(&saved.external_id)
                    .bind(saved.client_id)
                    .bind(saved.group_id)
                    .bind(saved.product_id)
                    .bind(saved.field_officer_id)
                    .bind(saved.status.as_i32())
                    .bind(saved.sub_status.as_i32())
                    .bind(saved.account_type)
                    .bind(SavingsDepositAccountType::SavingsDeposit.as_i32())
                    .bind(Some(saved.submitted_on_date.to_string()))
                    .bind(saved.submitted_by_user_id)
                    .bind(saved.approved_on_date.map(|d| d.to_string()))
                    .bind(saved.approved_by_user_id)
                    .bind(saved.activated_on_date.map(|d| d.to_string()))
                    .bind(saved.activated_by_user_id)
                    .bind(saved.closed_on_date.map(|d| d.to_string()))
                    .bind(saved.closed_by_user_id)
                    .bind(&saved.reason_for_block)
                    .bind(&saved.currency.code)
                    .bind(saved.currency.digits_after_decimal as i32)
                    .bind(saved.currency.in_multiples_of)
                    .bind(saved.nominal_annual_interest_rate.to_string())
                    .bind(saved.interest_compounding_period_type)
                    .bind(saved.interest_posting_period_type)
                    .bind(saved.interest_calculation_type)
                    .bind(saved.interest_calculation_days_in_year_type)
                    .bind(saved.min_required_opening_balance.map(|d| d.to_string()))
                    .bind(saved.lockin_period_frequency)
                    .bind(saved.lockin_period_frequency_type)
                    .bind(saved.lockedin_until_date_derived.map(|d| d.to_string()))
                    .bind(saved.withdrawal_fee_for_transfer)
                    .bind(saved.allow_overdraft)
                    .bind(saved.overdraft_limit.to_string())
                    .bind(saved.nominal_annual_interest_rate_overdraft.to_string())
                    .bind(saved.min_overdraft_for_interest_calculation.to_string())
                    .bind(saved.enforce_min_required_balance)
                    .bind(saved.min_required_balance.to_string())
                    .bind(saved.lien_allowed)
                    .bind(saved.max_allowed_lien_limit.to_string())
                    .bind(saved.on_hold_funds.to_string())
                    .bind(saved.savings_amount_on_hold.to_string())
                    .bind(saved.summary.total_deposits.to_string())
                    .bind(saved.summary.total_withdrawals.to_string())
                    .bind(saved.summary.total_withdrawal_fees.to_string())
                    .bind(saved.summary.total_fee_charge.to_string())
                    .bind(saved.summary.total_penalty_charge.to_string())
                    .bind(saved.summary.total_annual_fees.to_string())
                    .bind(saved.summary.total_interest_earned.to_string())
                    .bind(saved.summary.total_interest_posted.to_string())
                    .bind(saved.summary.total_overdraft_interest_derived.to_string())
                    .bind(saved.summary.total_withhold_tax.to_string())
                    .bind(saved.summary.account_balance.to_string())
                    .bind(
                        saved
                            .min_balance_for_interest_calculation
                            .map(|d| d.to_string()),
                    )
                    .bind(saved.withhold_tax)
                    .bind(saved.tax_group_id)
                    .bind(
                        saved
                            .summary
                            .last_interest_calculation_date
                            .map(|d| d.to_string()),
                    )
                    .bind(saved.summary.interest_posted_till_date.map(|d| d.to_string()))
                    .bind(1_i32)
                    .execute(pool)
                    .await;
            }
        }
        Ok(saved)
    }

    async fn find_account_by_id(&self, account_id: i64) -> FineractResult<SavingsAccount> {
        self.cache.find_account_by_id(account_id).await
    }

    async fn list_accounts(&self) -> FineractResult<Vec<SavingsAccount>> {
        self.cache.list_accounts().await
    }

    async fn find_active_accounts(&self) -> FineractResult<Vec<SavingsAccount>> {
        self.cache.find_active_accounts().await
    }

    async fn save_journal_entries(
        &self,
        entries: Vec<JournalEntryRecord>,
    ) -> FineractResult<Vec<JournalEntryRecord>> {
        self.cache.save_journal_entries(entries).await
    }

    async fn find_journal_entries_for_savings(
        &self,
        savings_id: i64,
        transaction_id: Option<i64>,
    ) -> FineractResult<Vec<JournalEntryRecord>> {
        self.cache
            .find_journal_entries_for_savings(savings_id, transaction_id)
            .await
    }
}

// =============================================================================
// 3. SAVINGS ACCOUNTING BRIDGE ADAPTER (`SavingsAccountingBridgeAdapter`)
// =============================================================================

/// Classification of savings accounting bridge events for GL journal generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavingsAccountingEventType {
    Deposit,
    Withdrawal,
    InterestPosting,
    OverdraftInterest,
    FeeDeduction,
    PenaltyDeduction,
    Escheat,
    Reversal,
}

/// Adapter connecting `SavingsAccount` transactions (`m_savings_account_transaction`)
/// to `fineract_accounting_rs::journal_processor::{AccountingBridgeEngine, SavingsProductAccountMapping}`
/// so balanced DR/CR [`JournalEntryRecord`] batches are emitted and persisted on
/// `Deposit`, `Withdrawal`, `InterestPosting`, `OverdraftInterest`, `FeeDeduction`, and `Reversal`.
#[derive(Debug, Clone, Default)]
pub struct SavingsAccountingBridgeAdapter {
    gl_accounts: HashMap<i64, GLAccount>,
    office_closures: BTreeMap<i64, Vec<GLClosure>>,
}

impl SavingsAccountingBridgeAdapter {
    /// Creates a new [`SavingsAccountingBridgeAdapter`].
    pub fn new() -> Self {
        Self {
            gl_accounts: HashMap::new(),
            office_closures: BTreeMap::new(),
        }
    }

    /// Creates a [`SavingsAccountingBridgeAdapter`] pre-populated with GL accounts and closures.
    pub fn with_gl_context(
        gl_accounts: HashMap<i64, GLAccount>,
        office_closures: BTreeMap<i64, Vec<GLClosure>>,
    ) -> Self {
        Self {
            gl_accounts,
            office_closures,
        }
    }

    /// Registers a [`GLAccount`] for validation when generating journal entries.
    pub fn register_gl_account(&mut self, gl_account: GLAccount) {
        self.gl_accounts.insert(gl_account.id, gl_account);
    }

    /// Registers an office [`GLClosure`] for accounting closure validation.
    pub fn register_gl_closure(&mut self, closure: GLClosure) {
        self.office_closures
            .entry(closure.office_id)
            .or_default()
            .push(closure);
    }

    /// Builds a default Cash-Based [`SavingsProductAccountMapping`] for standard savings products.
    #[allow(clippy::too_many_arguments)]
    pub fn default_cash_mapping(
        product_id: i64,
        savings_reference_gl_id: i64,
        savings_control_gl_id: i64,
        interest_on_savings_gl_id: i64,
        income_from_fees_gl_id: i64,
        income_from_penalties_gl_id: i64,
        overdraft_portfolio_control_gl_id: i64,
        income_from_interest_gl_id: i64,
        losses_written_off_gl_id: i64,
        escheat_liability_gl_id: i64,
    ) -> SavingsProductAccountMapping {
        SavingsProductAccountMapping::new(product_id, AccountingRuleType::CashBased)
            .with_account(
                SavingsProductAccountingParams::SavingsReference,
                savings_reference_gl_id,
            )
            .with_account(
                SavingsProductAccountingParams::SavingsControl,
                savings_control_gl_id,
            )
            .with_account(
                SavingsProductAccountingParams::InterestOnSavings,
                interest_on_savings_gl_id,
            )
            .with_account(
                SavingsProductAccountingParams::IncomeFromFees,
                income_from_fees_gl_id,
            )
            .with_account(
                SavingsProductAccountingParams::IncomeFromPenalties,
                income_from_penalties_gl_id,
            )
            .with_account(
                SavingsProductAccountingParams::OverdraftPortfolioControl,
                overdraft_portfolio_control_gl_id,
            )
            .with_account(
                SavingsProductAccountingParams::IncomeFromInterest,
                income_from_interest_gl_id,
            )
            .with_account(
                SavingsProductAccountingParams::LossesWrittenOff,
                losses_written_off_gl_id,
            )
            .with_account(
                SavingsProductAccountingParams::EscheatLiability,
                escheat_liability_gl_id,
            )
    }

    /// Maps a [`SavingsAccountTransaction`] to its corresponding [`SavingsBridgeTransactionType`].
    /// Returns `None` for non-accounting transactions such as `AmountHold` and `AmountRelease`.
    pub fn map_transaction_type(
        txn: &SavingsAccountTransaction,
        is_penalty_charge: bool,
    ) -> Option<SavingsBridgeTransactionType> {
        match txn.transaction_type {
            SavingsAccountTransactionType::Deposit
            | SavingsAccountTransactionType::DividendPayout => {
                Some(SavingsBridgeTransactionType::Deposit)
            }
            SavingsAccountTransactionType::Withdrawal => {
                Some(SavingsBridgeTransactionType::Withdrawal)
            }
            SavingsAccountTransactionType::InterestPosting => {
                Some(SavingsBridgeTransactionType::InterestPosting)
            }
            SavingsAccountTransactionType::OverdraftInterest => {
                Some(SavingsBridgeTransactionType::OverdraftInterest)
            }
            SavingsAccountTransactionType::WithdrawalFee
            | SavingsAccountTransactionType::AnnualFee
            | SavingsAccountTransactionType::PayCharge => {
                if is_penalty_charge {
                    Some(SavingsBridgeTransactionType::PenaltyDeduction)
                } else {
                    Some(SavingsBridgeTransactionType::FeeDeduction)
                }
            }
            SavingsAccountTransactionType::WithholdTax => {
                Some(SavingsBridgeTransactionType::Withdrawal)
            }
            SavingsAccountTransactionType::WrittenOff
            | SavingsAccountTransactionType::Escheat => {
                Some(SavingsBridgeTransactionType::WriteOff)
            }
            SavingsAccountTransactionType::Accrual => Some(SavingsBridgeTransactionType::Accrual),
            _ => None,
        }
    }

    /// Generates balanced [`JournalEntryRecord`] rows for a single [`SavingsAccountTransaction`].
    pub fn generate_entries_for_transaction(
        &self,
        next_id_start: i64,
        account: &SavingsAccount,
        txn: &SavingsAccountTransaction,
        mapping: &SavingsProductAccountMapping,
    ) -> FineractResult<Vec<JournalEntryRecord>> {
        if mapping.accounting_rule == AccountingRuleType::None {
            return Ok(Vec::new());
        }

        let is_penalty = txn.charges_paid_by.iter().any(|cpb| {
            account
                .charges
                .iter()
                .any(|c| c.id == Some(cpb.savings_account_charge_id) && c.is_penalty)
        });

        let Some(bridge_type) = Self::map_transaction_type(txn, is_penalty) else {
            return Ok(Vec::new());
        };

        if txn.amount <= Decimal::ZERO {
            return Ok(Vec::new());
        }

        let savings_id = account.id.or(txn.savings_account_id).unwrap_or(1);
        let txn_id = txn.id.unwrap_or(1);
        let office_id = account.office_id.or(txn.office_id).unwrap_or(1).max(1);

        let mut bridge_tx = SavingsBridgeTransaction::new(
            savings_id,
            account.product_id,
            txn_id,
            office_id,
            txn.transaction_date(),
            account.currency.code.clone(),
            bridge_type,
            txn.amount,
        );
        bridge_tx.overdraft_amount = txn.overdraft_amount.unwrap_or(Decimal::ZERO);
        bridge_tx.payment_type_id = txn.payment_detail_id;
        bridge_tx.payment_details_id = txn.payment_detail_id;
        bridge_tx.submitted_on_date = txn.submitted_on_date;
        bridge_tx.ref_num = txn.ref_no.clone();
        bridge_tx.description = Some(format!(
            "Savings {} transaction #{}",
            txn.transaction_type.code(),
            txn_id
        ));

        for cpb in &txn.charges_paid_by {
            if let Some(charge) = account
                .charges
                .iter()
                .find(|c| c.id == Some(cpb.savings_account_charge_id))
            {
                bridge_tx.charge_allocations.push(ChargePaymentAllocation {
                    charge_id: charge.charge_id,
                    amount: cpb.amount,
                    is_penalty: charge.is_penalty,
                });
            }
        }

        let closures = self
            .office_closures
            .get(&office_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        AccountingBridgeEngine::generate_savings_journal_entries(
            next_id_start,
            &bridge_tx,
            mapping,
            closures,
            &self.gl_accounts,
        )
        .map_err(|e| FineractError::domain_rule("error.msg.accounting.bridge.failed", e.to_string()))
    }

    /// Generates and persists balanced [`JournalEntryRecord`] rows for one or more new
    /// [`SavingsAccountTransaction`] items using the given [`SavingsAccountRepository`].
    pub async fn post_journal_entries_for_transactions<R: SavingsAccountRepository + ?Sized>(
        &self,
        repo: &R,
        account: &SavingsAccount,
        transactions: &[SavingsAccountTransaction],
    ) -> FineractResult<Vec<JournalEntryRecord>> {
        let Some(mapping) = repo
            .get_product_account_mapping(account.product_id)
            .await?
        else {
            return Ok(Vec::new());
        };

        let mut all_entries = Vec::new();
        for txn in transactions {
            if txn.reversed || txn.is_reversal {
                continue;
            }
            let next_id = repo.next_journal_entry_id();
            let generated =
                self.generate_entries_for_transaction(next_id, account, txn, &mapping)?;
            if !generated.is_empty() {
                let saved = repo.save_journal_entries(generated).await?;
                all_entries.extend(saved);
            }
        }
        Ok(all_entries)
    }

    /// Generates and persists contra (reversal) [`JournalEntryRecord`] rows when a
    /// [`SavingsAccountTransaction`] is undone / reversed.
    pub async fn reverse_journal_entries_for_transaction<R: SavingsAccountRepository + ?Sized>(
        &self,
        repo: &R,
        account: &SavingsAccount,
        original_transaction_id: i64,
        reversal_date: NaiveDate,
        reversal_comment: Option<String>,
    ) -> FineractResult<Vec<JournalEntryRecord>> {
        let savings_id = account.id.unwrap_or(0);
        let mut original_entries = repo
            .find_journal_entries_for_savings(savings_id, Some(original_transaction_id))
            .await?;
        if original_entries.is_empty() {
            return Ok(Vec::new());
        }

        let office_id = account.office_id.unwrap_or(1);
        let closures = self
            .office_closures
            .get(&office_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let next_id = repo.next_journal_entry_id();
        let reversal_entries = AccountingBridgeEngine::reverse_journal_entries(
            next_id,
            &mut original_entries,
            reversal_date,
            reversal_comment.or_else(|| {
                Some(format!(
                    "Reversal of savings transaction #{original_transaction_id}"
                ))
            }),
            closures,
        )
        .map_err(|e| {
            FineractError::domain_rule("error.msg.accounting.reversal.failed", e.to_string())
        })?;

        // Update original entries (marked `reversed = true`) and persist new contra entries
        repo.save_journal_entries(original_entries).await?;
        repo.save_journal_entries(reversal_entries).await
    }
}

// =============================================================================
// 4. SAVINGS DORMANCY BATCH SERVICE (`SavingsDormancyBatchService`)
// =============================================================================

/// Record of a single account dormancy state transition executed by [`SavingsDormancyBatchService`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DormancyTransitionRecord {
    pub savings_account_id: i64,
    pub previous_sub_status: SavingsAccountSubStatusEnum,
    pub new_sub_status: SavingsAccountSubStatusEnum,
    pub new_status: SavingsAccountStatusType,
    pub escheat_transaction_id: Option<i64>,
    pub escheat_amount: Decimal,
}

/// Summary of a batch dormancy scan run on a given `business_date`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DormancyBatchReport {
    pub business_date: Option<NaiveDate>,
    pub transitioned_to_inactive: Vec<i64>,
    pub transitioned_to_dormant: Vec<i64>,
    pub transitioned_to_escheat: Vec<i64>,
    pub transitions: Vec<DormancyTransitionRecord>,
}

/// Service ported from `SavingsAccountWritePlatformServiceJpaRepositoryImpl`
/// (`setSubStatusInactive`, `setSubStatusDormant`, `escheat`) and COB/Scheduler
/// `UpdateSavingsDormantAccountsTasklet`.
///
/// Scans active savings accounts on `business_date` against their product's
/// `days_to_inactive`, `days_to_dormancy`, and `days_to_escheat` thresholds and
/// transitions `SavingsAccountSubStatusEnum` (`None -> Inactive -> Dormant -> Escheat`).
#[derive(Debug, Clone)]
pub struct SavingsDormancyBatchService<R: SavingsAccountRepository> {
    repo: Arc<R>,
    accounting_bridge: SavingsAccountingBridgeAdapter,
}

impl<R: SavingsAccountRepository> SavingsDormancyBatchService<R> {
    /// Creates a new [`SavingsDormancyBatchService`].
    pub fn new(repo: Arc<R>, accounting_bridge: SavingsAccountingBridgeAdapter) -> Self {
        Self {
            repo,
            accounting_bridge,
        }
    }

    /// Scans all active savings accounts on `business_date` and applies eligible
    /// `None -> Inactive -> Dormant -> Escheat` transitions.
    pub async fn process_dormancy_transitions(
        &self,
        business_date: NaiveDate,
    ) -> FineractResult<DormancyBatchReport> {
        let mut report = DormancyBatchReport {
            business_date: Some(business_date),
            ..Default::default()
        };

        let active_accounts = self.repo.find_active_accounts().await?;
        for mut account in active_accounts {
            let Ok(product) = self.repo.find_product_by_id(account.product_id).await else {
                continue;
            };
            if !product.is_dormancy_tracking_active && !account.is_dormancy_tracking_active {
                continue;
            }

            let last_activity_date = account
                .retrieve_last_transaction_date()
                .or(account.activated_on_date)
                .unwrap_or(account.submitted_on_date);
            let days_inactive = (business_date - last_activity_date).num_days();
            if days_inactive < 0 {
                continue;
            }

            let days_to_inactive = product
                .days_to_inactive
                .or_else(|| account.days_to_inactive.map(i64::from));
            let days_to_dormancy = product
                .days_to_dormancy
                .or_else(|| account.days_to_dormancy.map(i64::from));
            let days_to_escheat = product
                .days_to_escheat
                .or_else(|| account.days_to_escheat.map(i64::from));

            let target_sub_status = match account.sub_status {
                SavingsAccountSubStatusEnum::None => {
                    if days_to_inactive.is_some_and(|d| days_inactive >= d) {
                        Some(SavingsAccountSubStatusEnum::Inactive)
                    } else {
                        None
                    }
                }
                SavingsAccountSubStatusEnum::Inactive => {
                    if days_to_dormancy.is_some_and(|d| days_inactive >= d) {
                        Some(SavingsAccountSubStatusEnum::Dormant)
                    } else {
                        None
                    }
                }
                SavingsAccountSubStatusEnum::Dormant => {
                    if days_to_escheat.is_some_and(|d| days_inactive >= d) {
                        Some(SavingsAccountSubStatusEnum::Escheat)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(new_sub_status) = target_sub_status {
                let previous_sub_status = account.sub_status;
                let pre_txn_count = account.transactions.len();
                let maybe_escheat_tx =
                    account.transition_dormancy_state(new_sub_status, business_date)?;

                let mut escheat_txn_id = maybe_escheat_tx.as_ref().and_then(|t| t.id);
                let mut escheat_amount = maybe_escheat_tx
                    .as_ref()
                    .map(|t| t.amount)
                    .unwrap_or(Decimal::ZERO);

                let saved = self.repo.save_account(account).await?;
                let account_id = saved.id.unwrap_or(0);

                if saved.transactions.len() > pre_txn_count {
                    let new_txns = &saved.transactions[pre_txn_count..];
                    for txn in new_txns {
                        if txn.transaction_type == SavingsAccountTransactionType::Escheat {
                            escheat_txn_id = txn.id;
                            escheat_amount = txn.amount;
                        }
                    }
                    let _ = self
                        .accounting_bridge
                        .post_journal_entries_for_transactions(&*self.repo, &saved, new_txns)
                        .await?;
                }

                match new_sub_status {
                    SavingsAccountSubStatusEnum::Inactive => {
                        report.transitioned_to_inactive.push(account_id);
                    }
                    SavingsAccountSubStatusEnum::Dormant => {
                        report.transitioned_to_dormant.push(account_id);
                    }
                    SavingsAccountSubStatusEnum::Escheat => {
                        report.transitioned_to_escheat.push(account_id);
                    }
                    _ => {}
                }

                report.transitions.push(DormancyTransitionRecord {
                    savings_account_id: account_id,
                    previous_sub_status,
                    new_sub_status,
                    new_status: saved.status,
                    escheat_transaction_id: escheat_txn_id,
                    escheat_amount,
                });
            }
        }

        Ok(report)
    }
}

// =============================================================================
// 5. JSON COMMAND EXTRACTION HELPERS
// =============================================================================

fn extract_required_date(command: &JsonCommand, param: &str) -> FineractResult<NaiveDate> {
    command
        .local_date_value_of_parameter_named(param)?
        .ok_or_else(|| {
            PlatformApiDataValidationException::single_parameter_error(
                format!("validation.msg.savingsaccount.{param}.cannot.be.blank"),
                format!("The parameter `{param}` is mandatory."),
                param,
                vec![],
            )
            .into()
        })
}

fn extract_optional_date(command: &JsonCommand, param: &str) -> FineractResult<Option<NaiveDate>> {
    if !command.parameter_exists(param) {
        return Ok(None);
    }
    command.local_date_value_of_parameter_named(param)
}

fn extract_required_decimal(command: &JsonCommand, param: &str) -> FineractResult<Decimal> {
    command
        .big_decimal_value_of_parameter_named(param)?
        .ok_or_else(|| {
            PlatformApiDataValidationException::single_parameter_error(
                format!("validation.msg.savingsaccount.{param}.cannot.be.blank"),
                format!("The parameter `{param}` is mandatory."),
                param,
                vec![],
            )
            .into()
        })
}

fn extract_optional_decimal(
    command: &JsonCommand,
    param: &str,
) -> FineractResult<Option<Decimal>> {
    if !command.parameter_exists(param) {
        return Ok(None);
    }
    command.big_decimal_value_of_parameter_named(param)
}

fn resolve_savings_id(command: &JsonCommand) -> FineractResult<i64> {
    if let Some(id) = command.savings_id.or(command.entity_id) {
        return Ok(id);
    }
    if let Some(id) = command.long_value_of_parameter_named("savingsId")? {
        return Ok(id);
    }
    if let Some(id) = command.long_value_of_parameter_named("accountId")? {
        return Ok(id);
    }
    Err(PlatformApiDataValidationException::single_parameter_error(
        "validation.msg.savingsaccount.id.cannot.be.blank",
        "The parameter `savingsId` is mandatory.",
        "savingsId",
        vec![],
    )
    .into())
}

fn resolve_subentity_id(command: &JsonCommand, param_name: &str) -> FineractResult<i64> {
    if let Some(id) = command.subresource_id {
        return Ok(id);
    }
    if let Some(id) = command.long_value_of_parameter_named(param_name)? {
        return Ok(id);
    }
    if let Some(id) = command.long_value_of_parameter_named("transactionId")? {
        return Ok(id);
    }
    Err(PlatformApiDataValidationException::single_parameter_error(
        format!("validation.msg.savingsaccount.{param_name}.cannot.be.blank"),
        format!("The parameter `{param_name}` is mandatory."),
        param_name,
        vec![],
    )
    .into())
}

// =============================================================================
// 6. CQRS COMMAND HANDLERS (`CommandHandler`)
// =============================================================================

// -----------------------------------------------------------------------------
// 6.1 SubmitSavingsApplicationCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `SUBMIT_SAVINGSACCOUNT` (`SubmitSavingsApplicationCommandHandler.java`).
pub struct SubmitSavingsApplicationCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
}

impl<R: SavingsAccountRepository> SubmitSavingsApplicationCommandHandler<R> {
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let product_id = command
            .long_value_of_parameter_named("productId")?
            .ok_or_else(|| {
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.savingsaccount.productId.cannot.be.blank",
                    "The parameter `productId` is mandatory.",
                    "productId",
                    vec![],
                )
            })?;

        let client_id = match command.client_id {
            Some(cid) => Some(cid),
            None => command.long_value_of_parameter_named("clientId")?,
        };
        let group_id = command.long_value_of_parameter_named("groupId")?;

        if client_id.is_none() && group_id.is_none() {
            return Err(PlatformApiDataValidationException::single_parameter_error(
                "validation.msg.savingsaccount.clientId.or.groupId.required",
                "Either `clientId` or `groupId` must be provided when submitting a savings application.",
                "clientId",
                vec![],
            )
            .into());
        }

        let submitted_on_date = extract_required_date(command, "submittedOnDate")?;
        let product = self.repo.find_product_by_id(product_id).await?;

        let account_id = self.repo.next_account_id();
        let account_no = command
            .string_value_of_parameter_named("accountNo")
            .unwrap_or_else(|| format!("{account_id:09}"));

        let nominal_rate = extract_optional_decimal(command, "nominalAnnualInterestRate")?
            .unwrap_or(product.nominal_annual_interest_rate);

        let mut account = SavingsAccount::new_submitted(
            account_no,
            client_id,
            group_id,
            product.id.unwrap_or(product_id),
            product.currency.clone(),
            nominal_rate,
            submitted_on_date,
        );
        account.id = Some(account_id);
        account.submitted_by_user_id = ctx.authenticated_user_id;
        account.office_id = command.long_value_of_parameter_named("officeId")?.or(Some(1));
        account.external_id = command.string_value_of_parameter_named("externalId");
        account.field_officer_id = command.long_value_of_parameter_named("fieldOfficerId")?;

        account.interest_compounding_period_type = command
            .integer_value_sans_locale_of_parameter_named("interestCompoundingPeriodType")?
            .unwrap_or_else(|| product.compounding_period_type.as_i32());

        account.interest_posting_period_type = command
            .integer_value_sans_locale_of_parameter_named("interestPostingPeriodType")?
            .unwrap_or_else(|| product.posting_period_type.as_i32());

        account.interest_calculation_type = command
            .integer_value_sans_locale_of_parameter_named("interestCalculationType")?
            .unwrap_or_else(|| product.interest_calculation_type.as_i32());

        account.interest_calculation_days_in_year_type = command
            .integer_value_sans_locale_of_parameter_named("interestCalculationDaysInYearType")?
            .unwrap_or_else(|| product.interest_calculation_days_in_year_type.as_i32());

        account.min_required_opening_balance =
            extract_optional_decimal(command, "minRequiredOpeningBalance")?
                .or(product.min_required_opening_balance);

        account.lockin_period_frequency = command
            .integer_value_sans_locale_of_parameter_named("lockinPeriodFrequency")?
            .or(product.lockin_period_frequency);

        account.lockin_period_frequency_type = command
            .integer_value_sans_locale_of_parameter_named("lockinPeriodFrequencyType")?
            .or_else(|| product.lockin_period_frequency_type.map(|t| t.as_i32()));

        account.withdrawal_fee_for_transfer = command
            .boolean_object_value_of_parameter_named("withdrawalFeeForTransfers")
            .unwrap_or(product.withdrawal_fee_for_transfer);

        account.allow_overdraft = command
            .boolean_object_value_of_parameter_named("allowOverdraft")
            .unwrap_or(product.allow_overdraft);

        account.overdraft_limit = extract_optional_decimal(command, "overdraftLimit")?
            .or(product.overdraft_limit)
            .unwrap_or(Decimal::ZERO);

        account.nominal_annual_interest_rate_overdraft =
            extract_optional_decimal(command, "nominalAnnualInterestRateOverdraft")?
                .or(product.nominal_annual_interest_rate_overdraft)
                .unwrap_or(Decimal::ZERO);

        account.min_overdraft_for_interest_calculation =
            extract_optional_decimal(command, "minOverdraftForInterestCalculation")?
                .or(product.min_overdraft_for_interest_calculation)
                .unwrap_or(Decimal::ZERO);

        account.enforce_min_required_balance = command
            .boolean_object_value_of_parameter_named("enforceMinRequiredBalance")
            .unwrap_or(product.enforce_min_required_balance);

        account.min_required_balance = extract_optional_decimal(command, "minRequiredBalance")?
            .or(product.min_required_balance)
            .unwrap_or(Decimal::ZERO);

        account.lien_allowed = command
            .boolean_object_value_of_parameter_named("lienAllowed")
            .unwrap_or(product.lien_allowed);

        account.max_allowed_lien_limit =
            extract_optional_decimal(command, "maxAllowedLienLimit")?
                .or(product.max_allowed_lien_limit)
                .unwrap_or(Decimal::ZERO);

        account.min_balance_for_interest_calculation =
            extract_optional_decimal(command, "minBalanceForInterestCalculation")?
                .or(product.min_balance_for_interest_calculation);

        account.withhold_tax = command
            .boolean_object_value_of_parameter_named("withHoldTax")
            .unwrap_or(product.withhold_tax);
        account.tax_group_id = product.tax_group_id;
        account.is_dormancy_tracking_active = product.is_dormancy_tracking_active;
        account.days_to_inactive = product.days_to_inactive.map(|d| d as i32);
        account.days_to_dormancy = product.days_to_dormancy.map(|d| d as i32);
        account.days_to_escheat = product.days_to_escheat.map(|d| d as i32);

        // Attach optional charges array if provided in JSON payload
        if let Some(charges_arr) = command.json_element("charges").and_then(|v| v.as_array()) {
            for item in charges_arr {
                if let Some(obj) = item.as_object() {
                    let charge_id = obj.get("chargeId").and_then(|v| v.as_i64()).unwrap_or(1);
                    let amount = obj
                        .get("amount")
                        .and_then(|v| {
                            v.as_str()
                                .and_then(|s| s.parse::<Decimal>().ok())
                                .or_else(|| v.as_f64().and_then(Decimal::from_f64_retain))
                                .or_else(|| v.as_i64().map(Decimal::from))
                        })
                        .unwrap_or(Decimal::ZERO);
                    let charge_time_enum = obj
                        .get("chargeTimeType")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(5) as i32;
                    let charge_calculation_enum = obj
                        .get("chargeCalculationType")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1) as i32;
                    let is_penalty = obj
                        .get("isPenalty")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let charge_time = fin_charge_time_from_i32(charge_time_enum);
                    let charge_calc = fin_charge_calc_from_i32(charge_calculation_enum);
                    let mut charge = SavingsAccountCharge::from_charge_definition(
                        Some(self.repo.next_charge_id()),
                        Some(account_id),
                        charge_id,
                        format!("Charge #{charge_id}"),
                        product.currency.clone(),
                        charge_time,
                        charge_calc,
                        amount,
                        is_penalty,
                        None,
                    );
                    if charge_calculation_enum == 2 {
                        charge.percentage = Some(amount);
                    }
                    account.charges.push(charge);
                }
            }
        }

        account.recalculate_daily_balances(ctx.business_date());
        let saved = self.repo.save_account(account).await?;
        let saved_id = saved.id.unwrap_or(account_id);

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved_id)
            .with_savings_id(saved_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "SUBMIT"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

fn fin_charge_time_from_i32(code: i32) -> fineract_charge_rs::ChargeTimeType {
    fineract_charge_rs::ChargeTimeType::from_int(code)
        .unwrap_or(fineract_charge_rs::ChargeTimeType::WithdrawalFee)
}

fn fin_charge_calc_from_i32(code: i32) -> fineract_charge_rs::ChargeCalculationType {
    fineract_charge_rs::ChargeCalculationType::from_int(code)
        .unwrap_or(fineract_charge_rs::ChargeCalculationType::Flat)
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for SubmitSavingsApplicationCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.2 ApproveSavingsApplicationCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `APPROVE_SAVINGSACCOUNT` (`ApproveSavingsApplicationCommandHandler.java`).
pub struct ApproveSavingsApplicationCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
}

impl<R: SavingsAccountRepository> ApproveSavingsApplicationCommandHandler<R> {
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let approved_on_date = extract_required_date(command, "approvedOnDate")?;

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        account.approve(approved_on_date, ctx.authenticated_user_id)?;
        let saved = self.repo.save_account(account).await?;

        let mut changes = BTreeMap::new();
        changes.insert(
            "approvedOnDate".to_string(),
            json!(approved_on_date.to_string()),
        );
        changes.insert("status".to_string(), json!(saved.status.code()));

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(savings_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .with_changes(changes)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "APPROVE"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for ApproveSavingsApplicationCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.3 ActivateSavingsAccountCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `ACTIVATE_SAVINGSACCOUNT` (`ActivateSavingsAccountCommandHandler.java` &
/// `SavingsAccountActivationServiceImpl.java`).
pub struct ActivateSavingsAccountCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
    accounting_bridge: SavingsAccountingBridgeAdapter,
}

impl<R: SavingsAccountRepository> ActivateSavingsAccountCommandHandler<R> {
    pub fn new(repo: Arc<R>, accounting_bridge: SavingsAccountingBridgeAdapter) -> Self {
        Self {
            repo,
            accounting_bridge,
        }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let activated_on_date = extract_required_date(command, "activatedOnDate")?;

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        let pre_txn_count = account.transactions.len();
        account.activate(activated_on_date, ctx.authenticated_user_id)?;

        let saved = self.repo.save_account(account).await?;
        if saved.transactions.len() > pre_txn_count {
            let new_txns = &saved.transactions[pre_txn_count..];
            let _ = self
                .accounting_bridge
                .post_journal_entries_for_transactions(&*self.repo, &saved, new_txns)
                .await?;
        }

        let mut changes = BTreeMap::new();
        changes.insert(
            "activatedOnDate".to_string(),
            json!(activated_on_date.to_string()),
        );
        changes.insert("status".to_string(), json!(saved.status.code()));

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(savings_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .with_changes(changes)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "ACTIVATE"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for ActivateSavingsAccountCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.4 DepositSavingsAccountCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `DEPOSIT_SAVINGSACCOUNT` (`DepositSavingsAccountCommandHandler.java` &
/// `SavingsAccountDomainServiceJpa::handleDeposit`).
pub struct DepositSavingsAccountCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
    accounting_bridge: SavingsAccountingBridgeAdapter,
}

impl<R: SavingsAccountRepository> DepositSavingsAccountCommandHandler<R> {
    pub fn new(repo: Arc<R>, accounting_bridge: SavingsAccountingBridgeAdapter) -> Self {
        Self {
            repo,
            accounting_bridge,
        }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let transaction_date = extract_required_date(command, "transactionDate")?;
        let transaction_amount = extract_required_decimal(command, "transactionAmount")?;
        let payment_type_id = command.long_value_of_parameter_named("paymentTypeId")?;

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        let pre_txn_ids: Vec<Option<i64>> = account.transactions.iter().map(|t| t.id).collect();

        let deposit_txn = account.deposit(
            transaction_date,
            transaction_amount,
            payment_type_id,
            ctx.business_date(),
        )?;

        let saved = self.repo.save_account(account).await?;
        let new_txns: Vec<SavingsAccountTransaction> = saved
            .transactions
            .iter()
            .filter(|t| !pre_txn_ids.contains(&t.id))
            .cloned()
            .collect();

        let _ = self
            .accounting_bridge
            .post_journal_entries_for_transactions(&*self.repo, &saved, &new_txns)
            .await?;

        let txn_id = new_txns
            .first()
            .and_then(|t| t.id)
            .or(deposit_txn.id)
            .unwrap_or(0);

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(txn_id)
            .with_sub_entity_id(txn_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "DEPOSIT"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for DepositSavingsAccountCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.5 WithdrawSavingsAccountCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `WITHDRAW_SAVINGSACCOUNT` (`WithdrawSavingsAccountCommandHandler.java` &
/// `SavingsAccountDomainServiceJpa::handleWithdrawal`).
pub struct WithdrawSavingsAccountCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
    accounting_bridge: SavingsAccountingBridgeAdapter,
}

impl<R: SavingsAccountRepository> WithdrawSavingsAccountCommandHandler<R> {
    pub fn new(repo: Arc<R>, accounting_bridge: SavingsAccountingBridgeAdapter) -> Self {
        Self {
            repo,
            accounting_bridge,
        }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let transaction_date = extract_required_date(command, "transactionDate")?;
        let transaction_amount = extract_required_decimal(command, "transactionAmount")?;
        let payment_type_id = command.long_value_of_parameter_named("paymentTypeId")?;
        let is_account_transfer = command
            .boolean_object_value_of_parameter_named("isAccountTransfer")
            .unwrap_or(false);

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        let pre_txn_ids: Vec<Option<i64>> = account.transactions.iter().map(|t| t.id).collect();

        let withdrawal_txn = account.withdraw(
            transaction_date,
            transaction_amount,
            payment_type_id,
            ctx.business_date(),
            is_account_transfer,
        )?;

        let saved = self.repo.save_account(account).await?;
        let new_txns: Vec<SavingsAccountTransaction> = saved
            .transactions
            .iter()
            .filter(|t| !pre_txn_ids.contains(&t.id))
            .cloned()
            .collect();

        // Post GL journal entries for both the withdrawal and any automatic withdrawal fee charge
        let _ = self
            .accounting_bridge
            .post_journal_entries_for_transactions(&*self.repo, &saved, &new_txns)
            .await?;

        let txn_id = new_txns
            .iter()
            .find(|t| t.transaction_type == SavingsAccountTransactionType::Withdrawal)
            .and_then(|t| t.id)
            .or(withdrawal_txn.id)
            .unwrap_or(0);

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(txn_id)
            .with_sub_entity_id(txn_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "WITHDRAW"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for WithdrawSavingsAccountCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.6 PostInterestSavingsAccountCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `POSTINTEREST_SAVINGSACCOUNT` (`PostInterestSavingsAccountCommandHandler.java` &
/// `SavingsAccountPostInterestServiceImpl.java`).
pub struct PostInterestSavingsAccountCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
    accounting_bridge: SavingsAccountingBridgeAdapter,
}

impl<R: SavingsAccountRepository> PostInterestSavingsAccountCommandHandler<R> {
    pub fn new(repo: Arc<R>, accounting_bridge: SavingsAccountingBridgeAdapter) -> Self {
        Self {
            repo,
            accounting_bridge,
        }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let is_post_interest_as_on = command
            .boolean_object_value_of_parameter_named("isPostInterestAsOn")
            .unwrap_or(false);
        let posting_date = if is_post_interest_as_on {
            extract_required_date(command, "transactionDate")?
        } else {
            extract_optional_date(command, "transactionDate")?
                .unwrap_or_else(|| ctx.business_date())
        };

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        if !account.is_active() {
            return Err(FineractError::domain_rule(
                "error.msg.savingsaccount.postInterest.not.in.active.state",
                "Savings account postInterest is not allowed because the account is not active.",
            ));
        }

        let start_date = account
            .summary
            .interest_posted_till_date
            .or(account.start_interest_calculation_date)
            .or(account.activated_on_date)
            .unwrap_or(posting_date);

        let pre_txn_ids: Vec<Option<i64>> = account.transactions.iter().map(|t| t.id).collect();

        if posting_date > start_date {
            // Build daily balance slices (`EndOfDayBalance` from `crate::interest_engine`)
            let currency = account.currency.clone();
            let mut slices = Vec::new();
            let ordered_txns: Vec<&SavingsAccountTransaction> = account
                .transactions
                .iter()
                .filter(|t| {
                    !t.is_reversed()
                        && !t.is_reversal_transaction()
                        && (t.is_credit() || t.is_debit())
                })
                .collect();

            if ordered_txns.is_empty() {
                let days = (posting_date - start_date).num_days().max(1);
                let bal = Money::of(
                    currency.clone(),
                    account.summary.account_balance,
                    FineractRoundingMode::HalfEven,
                );
                slices.push(EndOfDayBalance::new(start_date, bal.clone(), bal, days));
            } else {
                let mut cursor_date = start_date;
                let mut current_bal = Decimal::ZERO;
                for txn in &ordered_txns {
                    let tx_date = txn.transaction_date();
                    if tx_date <= start_date {
                        current_bal = txn.running_balance;
                        continue;
                    }
                    if tx_date > posting_date {
                        break;
                    }
                    let days = (tx_date - cursor_date).num_days();
                    if days > 0 {
                        let m = Money::of(
                            currency.clone(),
                            current_bal,
                            FineractRoundingMode::HalfEven,
                        );
                        slices.push(EndOfDayBalance::new(cursor_date, m.clone(), m, days));
                    }
                    current_bal = txn.running_balance;
                    cursor_date = tx_date;
                }
                let tail_days = (posting_date - cursor_date).num_days();
                if tail_days > 0 {
                    let m = Money::of(
                        currency.clone(),
                        current_bal,
                        FineractRoundingMode::HalfEven,
                    );
                    slices.push(EndOfDayBalance::new(cursor_date, m.clone(), m, tail_days));
                }
            }

            let days_in_year = i64::from(account.interest_calculation_days_in_year_type).max(360);
            let annual_rate_fraction = account.nominal_annual_interest_rate / Decimal::from(100);
            let overdraft_rate_fraction =
                account.nominal_annual_interest_rate_overdraft / Decimal::from(100);
            let min_bal = account
                .min_balance_for_interest_calculation
                .unwrap_or(Decimal::ZERO);
            let min_od = account.min_overdraft_for_interest_calculation;

            let mut total_positive_interest_unrounded = Decimal::ZERO;
            let mut total_overdraft_interest_unrounded = Decimal::ZERO;

            for slice in &slices {
                let slice_interest = slice.calculate_interest_on_balance(
                    Decimal::ZERO,
                    annual_rate_fraction,
                    days_in_year,
                    min_bal,
                    overdraft_rate_fraction,
                    min_od,
                    FineractRoundingMode::HalfEven,
                );
                if slice_interest > Decimal::ZERO {
                    total_positive_interest_unrounded += slice_interest;
                } else if slice_interest < Decimal::ZERO && account.allow_overdraft {
                    total_overdraft_interest_unrounded += slice_interest.abs();
                }
            }

            let positive_interest_money = Money::of(
                currency.clone(),
                total_positive_interest_unrounded,
                FineractRoundingMode::HalfEven,
            );
            let overdraft_interest_money = Money::of(
                currency,
                total_overdraft_interest_unrounded,
                FineractRoundingMode::HalfEven,
            );

            if positive_interest_money.amount() > Decimal::ZERO {
                let _ = account.post_interest_amount(
                    posting_date,
                    positive_interest_money.amount(),
                    ctx.business_date(),
                    is_post_interest_as_on,
                    false,
                )?;
                account.summary.total_interest_earned += positive_interest_money.amount();
            }

            if overdraft_interest_money.amount() > Decimal::ZERO {
                let _ = account.post_overdraft_interest_amount(
                    posting_date,
                    overdraft_interest_money.amount(),
                    ctx.business_date(),
                    is_post_interest_as_on,
                    false,
                )?;
            }

            account.summary.last_interest_calculation_date = Some(posting_date);
            account.summary.interest_posted_till_date = Some(posting_date);
            account.recalculate_daily_balances(ctx.business_date());
        }

        let saved = self.repo.save_account(account).await?;
        let new_txns: Vec<SavingsAccountTransaction> = saved
            .transactions
            .iter()
            .filter(|t| !pre_txn_ids.contains(&t.id))
            .cloned()
            .collect();

        let _ = self
            .accounting_bridge
            .post_journal_entries_for_transactions(&*self.repo, &saved, &new_txns)
            .await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(savings_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "POSTINTEREST"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for PostInterestSavingsAccountCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.7 HoldAmountSavingsAccountCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `HOLDAMOUNT_SAVINGSACCOUNT` (`HoldAmountSavingsAccountCommandHandler.java` &
/// `SavingsAccountWritePlatformServiceJpaRepositoryImpl::holdAmount`).
pub struct HoldAmountSavingsAccountCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
}

impl<R: SavingsAccountRepository> HoldAmountSavingsAccountCommandHandler<R> {
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let transaction_date = extract_required_date(command, "transactionDate")?;
        let transaction_amount = extract_required_decimal(command, "transactionAmount")?;
        let reason_for_block = command.string_value_of_parameter_named("reasonForBlock");
        let lien_allowed = command
            .boolean_object_value_of_parameter_named("lien")
            .unwrap_or(false);
        let payment_type_id = command.long_value_of_parameter_named("paymentTypeId")?;

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        let hold_txn = account.hold_amount(
            transaction_date,
            transaction_amount,
            payment_type_id,
            ctx.business_date(),
            lien_allowed,
            reason_for_block,
        )?;
        let hold_txn_id = hold_txn.id.unwrap_or(0);

        let saved = self.repo.save_account(account).await?;
        let resolved_txn_id = saved
            .transactions
            .last()
            .and_then(|t| t.id)
            .unwrap_or(hold_txn_id);

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(resolved_txn_id)
            .with_sub_entity_id(resolved_txn_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "HOLDAMOUNT"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for HoldAmountSavingsAccountCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.8 ReleaseAmountSavingsAccountCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `RELEASEAMOUNT_SAVINGSACCOUNT` (`ReleaseAmountSavingsAccountCommandHandler.java` &
/// `SavingsAccountWritePlatformServiceJpaRepositoryImpl::releaseAmount`).
pub struct ReleaseAmountSavingsAccountCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
}

impl<R: SavingsAccountRepository> ReleaseAmountSavingsAccountCommandHandler<R> {
    pub fn new(repo: Arc<R>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let hold_transaction_id = resolve_subentity_id(command, "holdTransactionId")?;
        let release_date = extract_optional_date(command, "transactionDate")?
            .unwrap_or_else(|| ctx.business_date());

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        let release_txn =
            account.release_amount(hold_transaction_id, release_date, ctx.business_date())?;
        let release_txn_id = release_txn.id.unwrap_or(0);

        let saved = self.repo.save_account(account).await?;
        let resolved_txn_id = saved
            .transactions
            .last()
            .and_then(|t| t.id)
            .unwrap_or(release_txn_id);

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(resolved_txn_id)
            .with_sub_entity_id(resolved_txn_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "RELEASEAMOUNT"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for ReleaseAmountSavingsAccountCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.9 CloseSavingsAccountCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `CLOSE_SAVINGSACCOUNT` (`CloseSavingsAccountCommandHandler.java` &
/// `SavingsAccountWritePlatformServiceJpaRepositoryImpl::close`).
pub struct CloseSavingsAccountCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
    accounting_bridge: SavingsAccountingBridgeAdapter,
}

impl<R: SavingsAccountRepository> CloseSavingsAccountCommandHandler<R> {
    pub fn new(repo: Arc<R>, accounting_bridge: SavingsAccountingBridgeAdapter) -> Self {
        Self {
            repo,
            accounting_bridge,
        }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let closed_on_date = extract_required_date(command, "closedOnDate")?;
        let withdraw_balance = command
            .boolean_object_value_of_parameter_named("withdrawBalance")
            .unwrap_or(false);
        let payment_type_id = command.long_value_of_parameter_named("paymentTypeId")?;

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        let pre_txn_ids: Vec<Option<i64>> = account.transactions.iter().map(|t| t.id).collect();

        if withdraw_balance && account.summary.account_balance > Decimal::ZERO {
            let bal = account.summary.account_balance;
            let _ = account.withdraw(
                closed_on_date,
                bal,
                payment_type_id,
                ctx.business_date(),
                false,
            )?;
        }

        account.close(closed_on_date, ctx.authenticated_user_id, false)?;
        let saved = self.repo.save_account(account).await?;

        let new_txns: Vec<SavingsAccountTransaction> = saved
            .transactions
            .iter()
            .filter(|t| !pre_txn_ids.contains(&t.id))
            .cloned()
            .collect();
        if !new_txns.is_empty() {
            let _ = self
                .accounting_bridge
                .post_journal_entries_for_transactions(&*self.repo, &saved, &new_txns)
                .await?;
        }

        let mut changes = BTreeMap::new();
        changes.insert(
            "closedOnDate".to_string(),
            json!(closed_on_date.to_string()),
        );
        changes.insert("status".to_string(), json!(saved.status.code()));

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(savings_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .with_changes(changes)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "CLOSE"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for CloseSavingsAccountCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// -----------------------------------------------------------------------------
// 6.10 UndoSavingsTransactionCommandHandler
// -----------------------------------------------------------------------------

/// Command handler for `UNDOTRANSACTION_SAVINGSACCOUNT` (`UndoSavingsTransactionCommandHandler.java` &
/// `SavingsAccountWritePlatformServiceJpaRepositoryImpl::undoTransaction`).
pub struct UndoSavingsTransactionCommandHandler<R: SavingsAccountRepository> {
    repo: Arc<R>,
    accounting_bridge: SavingsAccountingBridgeAdapter,
}

impl<R: SavingsAccountRepository> UndoSavingsTransactionCommandHandler<R> {
    pub fn new(repo: Arc<R>, accounting_bridge: SavingsAccountingBridgeAdapter) -> Self {
        Self {
            repo,
            accounting_bridge,
        }
    }

    pub fn repository(&self) -> &Arc<R> {
        &self.repo
    }

    pub async fn process_command(
        &self,
        command: &JsonCommand,
        ctx: &ExecutionContext,
    ) -> FineractResult<CommandProcessingResult> {
        let savings_id = resolve_savings_id(command)?;
        let transaction_id = resolve_subentity_id(command, "transactionId")?;

        let mut account = self.repo.find_account_by_id(savings_id).await?;
        let reversed_txn = account.reverse_transaction(transaction_id, ctx.business_date())?;
        let saved = self.repo.save_account(account).await?;

        let _ = self
            .accounting_bridge
            .reverse_journal_entries_for_transaction(
                &*self.repo,
                &saved,
                transaction_id,
                ctx.business_date(),
                Some(format!(
                    "Undo savings transaction #{transaction_id} on account #{savings_id}"
                )),
            )
            .await?;

        let mut changes = BTreeMap::new();
        changes.insert("reversedTransactionId".to_string(), json!(transaction_id));
        if let Some(rev_id) = reversed_txn.id {
            changes.insert("reversalTransactionId".to_string(), json!(rev_id));
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(transaction_id)
            .with_sub_entity_id(transaction_id)
            .with_savings_id(savings_id)
            .with_office_id_opt(saved.office_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .with_changes(changes)
            .build())
    }

    pub fn action_name(&self) -> &str {
        "UNDOTRANSACTION"
    }

    pub fn entity_name(&self) -> &str {
        "SAVINGSACCOUNT"
    }
}

#[async_trait]
impl<R: SavingsAccountRepository + 'static> CommandHandler
    for UndoSavingsTransactionCommandHandler<R>
{
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        self.process_command(command, ctx).await
    }
}

// =============================================================================
// 7. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use fineract_accounting_rs::gl_and_rules::{GLAccountType, GLAccountUsage};
    use fineract_accounting_rs::journal_processor::JournalEntryType;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("valid date")
    }

    fn detail_gl(id: i64, name: &str, code: &str, account_type: GLAccountType) -> GLAccount {
        GLAccount::new(
            id,
            None,
            name,
            code,
            false,
            true,
            account_type,
            GLAccountUsage::Detail,
            None,
            None,
        )
    }

    fn build_gl_adapter_and_mapping(
        product_id: i64,
    ) -> (
        SavingsAccountingBridgeAdapter,
        SavingsProductAccountMapping,
    ) {
        let mut adapter = SavingsAccountingBridgeAdapter::new();
        adapter.register_gl_account(detail_gl(
            101,
            "Vault Cash / Savings Reference",
            "1010",
            GLAccountType::Asset,
        ));
        adapter.register_gl_account(detail_gl(
            201,
            "Savings Control Liability",
            "2010",
            GLAccountType::Liability,
        ));
        adapter.register_gl_account(detail_gl(
            301,
            "Interest Expense on Savings",
            "5010",
            GLAccountType::Expense,
        ));
        adapter.register_gl_account(detail_gl(
            401,
            "Income from Savings Fees",
            "4010",
            GLAccountType::Income,
        ));
        adapter.register_gl_account(detail_gl(
            402,
            "Income from Penalties",
            "4020",
            GLAccountType::Income,
        ));
        adapter.register_gl_account(detail_gl(
            102,
            "Overdraft Portfolio Control",
            "1020",
            GLAccountType::Asset,
        ));
        adapter.register_gl_account(detail_gl(
            403,
            "Income from Overdraft Interest",
            "4030",
            GLAccountType::Income,
        ));
        adapter.register_gl_account(detail_gl(
            502,
            "Losses Written Off",
            "5020",
            GLAccountType::Expense,
        ));
        adapter.register_gl_account(detail_gl(
            202,
            "Escheat Liability",
            "2020",
            GLAccountType::Liability,
        ));

        let mapping = SavingsAccountingBridgeAdapter::default_cash_mapping(
            product_id, 101, 201, 301, 401, 402, 102, 403, 502, 202,
        );
        (adapter, mapping)
    }

    #[tokio::test]
    async fn test_full_savings_lifecycle_handlers_and_gl_journal_entries() {
        let repo = Arc::new(InMemorySavingsRepository::new());
        let (adapter, mapping) = build_gl_adapter_and_mapping(1);

        // 1. Seed a savings product with 12% nominal annual interest rate and Cash-Based GL mapping
        let mut product = SavingsProduct::new(
            "Premier Checking & Savings",
            "PCS1",
            MonetaryCurrency::new("USD", 2, None),
            dec!(12.0),
            SavingsCompoundingInterestPeriodType::Daily,
            SavingsPostingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
        )
        .with_id(1);
        product.allow_overdraft = true;
        product.overdraft_limit = Some(dec!(500.00));
        product.nominal_annual_interest_rate_overdraft = Some(dec!(18.0));
        product.lien_allowed = true;
        product.max_allowed_lien_limit = Some(dec!(1000.00));
        repo.save_product(product).await.unwrap();
        repo.save_product_account_mapping(1, mapping).await.unwrap();

        let submit_handler = SubmitSavingsApplicationCommandHandler::new(repo.clone());
        let approve_handler = ApproveSavingsApplicationCommandHandler::new(repo.clone());
        let activate_handler =
            ActivateSavingsAccountCommandHandler::new(repo.clone(), adapter.clone());
        let deposit_handler =
            DepositSavingsAccountCommandHandler::new(repo.clone(), adapter.clone());
        let hold_handler = HoldAmountSavingsAccountCommandHandler::new(repo.clone());
        let release_handler = ReleaseAmountSavingsAccountCommandHandler::new(repo.clone());
        let withdraw_handler =
            WithdrawSavingsAccountCommandHandler::new(repo.clone(), adapter.clone());
        let post_interest_handler =
            PostInterestSavingsAccountCommandHandler::new(repo.clone(), adapter.clone());
        let undo_handler =
            UndoSavingsTransactionCommandHandler::new(repo.clone(), adapter.clone());
        let close_handler = CloseSavingsAccountCommandHandler::new(repo.clone(), adapter.clone());

        let ctx = ExecutionContext::new("default", d(2026, 3, 1));

        // 2. Submit Savings Application (with a flat $5.00 withdrawal fee charge)
        let submit_cmd = JsonCommand::from_value(
            None,
            json!({
                "clientId": 10,
                "productId": 1,
                "submittedOnDate": "2026-01-01",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en",
                "charges": [
                    {
                        "chargeId": 50,
                        "amount": 5.00,
                        "chargeTimeType": 5,
                        "chargeCalculationType": 1,
                        "isPenalty": false
                    }
                ]
            }),
        );
        let submit_res = submit_handler.handle(&ctx, &submit_cmd).await.unwrap();
        let savings_id = submit_res.savings_id.unwrap();
        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        assert_eq!(
            acct.status,
            SavingsAccountStatusType::SubmittedAndPendingApproval
        );

        // 3. Approve Application
        let approve_cmd = JsonCommand::from_value(
            None,
            json!({
                "approvedOnDate": "2026-01-01",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        )
        .with_savings_id(savings_id);
        approve_handler.handle(&ctx, &approve_cmd).await.unwrap();
        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        assert_eq!(acct.status, SavingsAccountStatusType::Approved);

        // 4. Activate Account
        let activate_cmd = JsonCommand::from_value(
            None,
            json!({
                "activatedOnDate": "2026-01-01",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        )
        .with_savings_id(savings_id);
        activate_handler.handle(&ctx, &activate_cmd).await.unwrap();
        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        assert_eq!(acct.status, SavingsAccountStatusType::Active);

        // 5. Deposit $1,000.00 on 2026-01-01
        let deposit_cmd = JsonCommand::from_value(
            None,
            json!({
                "transactionDate": "2026-01-01",
                "transactionAmount": 1000.00,
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        )
        .with_savings_id(savings_id);
        let dep_res = deposit_handler.handle(&ctx, &deposit_cmd).await.unwrap();
        let deposit_txn_id = dep_res.resource_id.unwrap();
        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        assert_eq!(acct.summary.account_balance, dec!(1000.00));

        // Check GL journal entries for Deposit (DR 101 SavingsReference $1000, CR 201 SavingsControl $1000)
        let dep_journals = repo
            .find_journal_entries_for_savings(savings_id, Some(deposit_txn_id))
            .await
            .unwrap();
        assert_eq!(dep_journals.len(), 2);
        assert!(dep_journals.iter().any(|j| j.account_id == 101
            && j.entry_type() == Some(JournalEntryType::Debit)
            && j.amount == dec!(1000.00)));
        assert!(dep_journals.iter().any(|j| j.account_id == 201
            && j.entry_type() == Some(JournalEntryType::Credit)
            && j.amount == dec!(1000.00)));

        // 6. Hold $300.00 on 2026-01-10
        let hold_cmd = JsonCommand::from_value(
            None,
            json!({
                "transactionDate": "2026-01-10",
                "transactionAmount": 300.00,
                "reasonForBlock": "Guarantee hold",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        )
        .with_savings_id(savings_id);
        let hold_res = hold_handler.handle(&ctx, &hold_cmd).await.unwrap();
        let hold_txn_id = hold_res.resource_id.unwrap();
        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        assert_eq!(acct.savings_amount_on_hold, dec!(300.00));
        assert_eq!(acct.summary.available_balance, dec!(700.00));

        // 7. Release Hold on 2026-01-15
        let release_cmd = JsonCommand::from_value(
            None,
            json!({
                "transactionDate": "2026-01-15",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        )
        .with_savings_id(savings_id)
        .with_subresource_id(hold_txn_id);
        release_handler.handle(&ctx, &release_cmd).await.unwrap();
        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        assert_eq!(acct.savings_amount_on_hold, dec!(0.00));
        assert_eq!(acct.summary.available_balance, dec!(1000.00));

        // 8. Withdraw $200.00 on 2026-01-20 (automatically triggers $5.00 withdrawal fee)
        let withdraw_cmd = JsonCommand::from_value(
            None,
            json!({
                "transactionDate": "2026-01-20",
                "transactionAmount": 200.00,
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        )
        .with_savings_id(savings_id);
        let withdraw_res = withdraw_handler.handle(&ctx, &withdraw_cmd).await.unwrap();
        let withdraw_txn_id = withdraw_res.resource_id.unwrap();
        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        // $1000.00 - $200.00 - $5.00 fee = $795.00
        assert_eq!(acct.summary.account_balance, dec!(795.00));
        assert_eq!(acct.summary.total_withdrawal_fees, dec!(5.00));

        // Verify GL entries include Withdrawal ($200 DR 201 / CR 101) AND FeeDeduction ($5 DR 201 / CR 401)
        let all_journals = repo
            .find_journal_entries_for_savings(savings_id, None)
            .await
            .unwrap();
        assert!(all_journals.iter().any(|j| j.account_id == 401
            && j.entry_type() == Some(JournalEntryType::Credit)
            && j.amount == dec!(5.00)));

        // 9. Post Interest as on 2026-02-01
        let post_int_cmd = JsonCommand::from_value(
            None,
            json!({
                "transactionDate": "2026-02-01",
                "isPostInterestAsOn": true,
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        )
        .with_savings_id(savings_id);
        post_interest_handler
            .handle(&ctx, &post_int_cmd)
            .await
            .unwrap();
        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        assert!(acct.summary.total_interest_posted > Decimal::ZERO);
        let posted_interest = acct.summary.total_interest_posted;

        // Verify InterestPosting GL journal entry (DR 301 InterestOnSavings, CR 201 SavingsControl)
        let all_journals = repo
            .find_journal_entries_for_savings(savings_id, None)
            .await
            .unwrap();
        assert!(all_journals.iter().any(|j| {
            j.account_id == 301
                && j.entry_type() == Some(JournalEntryType::Debit)
                && j.amount == posted_interest
        }));

        // 10. Undo the $200.00 Withdrawal transaction
        let undo_cmd = JsonCommand::from_value(None, json!({}))
            .with_savings_id(savings_id)
            .with_subresource_id(withdraw_txn_id);
        undo_handler.handle(&ctx, &undo_cmd).await.unwrap();

        let acct = repo.find_account_by_id(savings_id).await.unwrap();
        // Balance restored by +$200.00
        assert_eq!(
            acct.summary.account_balance,
            dec!(995.00) + posted_interest
        );

        // Check that the original withdrawal journal entries were reversed and contra entries created
        let w_journals = repo
            .find_journal_entries_for_savings(savings_id, Some(withdraw_txn_id))
            .await
            .unwrap();
        assert_eq!(w_journals.len(), 4);
        assert!(w_journals.iter().all(|j| j.reversed));

        // 11. Close the Savings Account on 2026-02-15 with withdrawBalance = true
        // First disable the withdrawal fee charge so the full closing withdrawal drains balance to 0
        let mut acct_for_close = repo.find_account_by_id(savings_id).await.unwrap();
        for ch in &mut acct_for_close.charges {
            ch.inactivate(d(2026, 2, 15));
        }
        repo.save_account(acct_for_close).await.unwrap();

        let close_cmd = JsonCommand::from_value(
            None,
            json!({
                "closedOnDate": "2026-02-15",
                "withdrawBalance": true,
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }),
        )
        .with_savings_id(savings_id);
        close_handler.handle(&ctx, &close_cmd).await.unwrap();

        let closed_acct = repo.find_account_by_id(savings_id).await.unwrap();
        assert_eq!(closed_acct.status, SavingsAccountStatusType::Closed);
        assert_eq!(closed_acct.summary.account_balance, dec!(0.00));
    }

    #[tokio::test]
    async fn test_dormancy_batch_service_transitions_inactive_dormant_escheat() {
        let repo = Arc::new(InMemorySavingsRepository::new());
        let (adapter, mapping) = build_gl_adapter_and_mapping(10);

        let mut product = SavingsProduct::new(
            "Dormancy Tracked Savings",
            "DTS1",
            MonetaryCurrency::new("USD", 2, None),
            dec!(5.0),
            SavingsCompoundingInterestPeriodType::Daily,
            SavingsPostingInterestPeriodType::Monthly,
            SavingsInterestCalculationType::DailyBalance,
            SavingsInterestCalculationDaysInYearType::Days365,
        )
        .with_id(10);
        product.is_dormancy_tracking_active = true;
        product.days_to_inactive = Some(30);
        product.days_to_dormancy = Some(60);
        product.days_to_escheat = Some(90);
        repo.save_product(product.clone()).await.unwrap();
        repo.save_product_account_mapping(10, mapping).await.unwrap();

        // Create and activate an account on 2026-01-01 with a $250.00 deposit
        let mut acct = SavingsAccount::new_submitted(
            "000000100",
            Some(1),
            None,
            10,
            product.currency.clone(),
            dec!(5.0),
            d(2026, 1, 1),
        );
        acct.id = Some(100);
        acct.office_id = Some(1);
        acct.is_dormancy_tracking_active = true;
        acct.days_to_inactive = Some(30);
        acct.days_to_dormancy = Some(60);
        acct.days_to_escheat = Some(90);
        acct.approve(d(2026, 1, 1), Some(1)).unwrap();
        acct.activate(d(2026, 1, 1), Some(1)).unwrap();
        acct.deposit(d(2026, 1, 1), dec!(250.00), None, d(2026, 1, 1))
            .unwrap();
        repo.save_account(acct).await.unwrap();

        let batch = SavingsDormancyBatchService::new(repo.clone(), adapter);

        // Day 31 (2026-02-01): None -> Inactive
        let rep1 = batch
            .process_dormancy_transitions(d(2026, 2, 1))
            .await
            .unwrap();
        assert_eq!(rep1.transitioned_to_inactive, vec![100]);
        let a1 = repo.find_account_by_id(100).await.unwrap();
        assert_eq!(a1.sub_status, SavingsAccountSubStatusEnum::Inactive);

        // Day 62 (2026-03-04): Inactive -> Dormant
        let rep2 = batch
            .process_dormancy_transitions(d(2026, 3, 4))
            .await
            .unwrap();
        assert_eq!(rep2.transitioned_to_dormant, vec![100]);
        let a2 = repo.find_account_by_id(100).await.unwrap();
        assert_eq!(a2.sub_status, SavingsAccountSubStatusEnum::Dormant);

        // Day 95 (2026-04-06): Dormant -> Escheat (closes account and posts Escheat debit of $250)
        let rep3 = batch
            .process_dormancy_transitions(d(2026, 4, 6))
            .await
            .unwrap();
        assert_eq!(rep3.transitioned_to_escheat, vec![100]);
        assert_eq!(rep3.transitions[0].escheat_amount, dec!(250.00));

        let a3 = repo.find_account_by_id(100).await.unwrap();
        assert_eq!(a3.sub_status, SavingsAccountSubStatusEnum::Escheat);
        assert_eq!(a3.status, SavingsAccountStatusType::Closed);
        assert_eq!(a3.summary.account_balance, dec!(0.00));
    }

    #[test]
    fn test_dual_db_savings_repository_sql_dialect_rewriting() {
        let pg_repo = DualDbSavingsRepository::new_in_memory(SqlDialect::Postgres);
        let mysql_repo = DualDbSavingsRepository::new_in_memory(SqlDialect::MySql);

        assert!(pg_repo.select_account_by_id_sql().contains("$1"));
        assert!(!pg_repo.select_account_by_id_sql().contains('?'));
        assert!(mysql_repo.select_account_by_id_sql().contains('?'));
        assert!(!mysql_repo.select_account_by_id_sql().contains("$1"));

        assert!(pg_repo.insert_transaction_sql().contains("$23"));
        assert!(pg_repo.insert_charge_sql().contains("$21"));
        assert!(pg_repo.select_product_by_id_sql().contains("$1"));
    }
}
