//! General Ledger Chart of Accounts (`GLAccount`), Accounting Closures (`GLClosure`),
//! Financial Activity Account Mappings (`FinancialActivityAccount`), Accounting Rules (`AccountingRule`),
//! Dual-Database Metadata Repository (`DualDbGlMetadataRepository`), and CQRS Command Handlers.
//!
//! Ported from:
//! - `org.apache.fineract.accounting.glaccount.domain.{GLAccount, GLAccountType, GLAccountUsage}`
//! - `org.apache.fineract.accounting.glaccount.command.GLAccountCommand`
//! - `org.apache.fineract.accounting.glaccount.service.GLAccountWritePlatformServiceJpaRepositoryImpl`
//! - `org.apache.fineract.accounting.glaccount.handler.CreateGLAccountCommandHandler`
//! - `org.apache.fineract.accounting.closure.domain.GLClosure`
//! - `org.apache.fineract.accounting.closure.command.GLClosureCommand`
//! - `org.apache.fineract.accounting.closure.service.GLClosureWritePlatformServiceJpaRepositoryImpl`
//! - `org.apache.fineract.accounting.closure.handler.CreateGLClosureCommandHandler`
//! - `org.apache.fineract.accounting.common.AccountingConstants.FinancialActivity`
//! - `org.apache.fineract.accounting.financialactivityaccount.domain.FinancialActivityAccount`
//! - `org.apache.fineract.accounting.financialactivityaccount.serialization.FinancialActivityAccountDataValidator`
//! - `org.apache.fineract.accounting.financialactivityaccount.service.FinancialActivityAccountWritePlatformServiceImpl`
//! - `org.apache.fineract.accounting.financialactivityaccount.handler.CreateFinancialActivityAccountHandler`
//! - `org.apache.fineract.accounting.rule.domain.{AccountingRule, AccountingTagRule}`
//! - `org.apache.fineract.accounting.rule.serialization.AccountingRuleCommandFromApiJsonDeserializer`
//! - `org.apache.fineract.accounting.rule.service.AccountingRuleWritePlatformServiceJpaRepositoryImpl`
//! - `org.apache.fineract.accounting.rule.handler.CreateAccountingRuleCommandHandler`

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::{AnyPool, Row};

use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{DialectSql, ExecutionContext, JsonCommand, SqlDialect};
use fineract_validation_rs::{
    ApiErrorArg, ApiParameterError, DataValidatorBuilder, FineractError,
    PlatformApiDataValidationException, PlatformDomainRuleException,
};

// =============================================================================
// 1. GLAccountType (org.apache.fineract.accounting.glaccount.domain.GLAccountType)
// =============================================================================

/// General Ledger account classification (`acc_gl_account.classification_enum`).
///
/// Ported from `org.apache.fineract.accounting.glaccount.domain.GLAccountType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum GLAccountType {
    Asset = 1,
    Liability = 2,
    Equity = 3,
    Income = 4,
    Expense = 5,
}

impl GLAccountType {
    /// Java-compatible constant alias for [`GLAccountType::Asset`].
    pub const ASSET: Self = Self::Asset;
    /// Java-compatible constant alias for [`GLAccountType::Liability`].
    pub const LIABILITY: Self = Self::Liability;
    /// Java-compatible constant alias for [`GLAccountType::Equity`].
    pub const EQUITY: Self = Self::Equity;
    /// Java-compatible constant alias for [`GLAccountType::Income`].
    pub const INCOME: Self = Self::Income;
    /// Java-compatible constant alias for [`GLAccountType::Expense`].
    pub const EXPENSE: Self = Self::Expense;

    /// Minimum valid discriminant (`GLAccountType.getMinValue()`).
    pub const MIN_VALUE: i32 = 1;
    /// Maximum valid discriminant (`GLAccountType.getMaxValue()`).
    pub const MAX_VALUE: i32 = 5;

    /// Returns the integer discriminant matching `GLAccountType.getValue()`.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Returns the integer discriminant as `i32`.
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    /// Returns the `SMALLINT` (`i16`) database representation stored in `acc_gl_account.classification_enum`.
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Returns `GLAccountType.getMinValue()`.
    pub const fn min_value() -> i32 {
        Self::MIN_VALUE
    }

    /// Returns `GLAccountType.getMaxValue()`.
    pub const fn max_value() -> i32 {
        Self::MAX_VALUE
    }

    /// Returns the globalisation message code matching `GLAccountType.getCode()`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Asset => "accountType.asset",
            Self::Liability => "accountType.liability",
            Self::Equity => "accountType.equity",
            Self::Income => "accountType.income",
            Self::Expense => "accountType.expense",
        }
    }

    /// Returns the uppercase Java enum name (`GLAccountType.name()`).
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::Asset => "ASSET",
            Self::Liability => "LIABILITY",
            Self::Equity => "EQUITY",
            Self::Income => "INCOME",
            Self::Expense => "EXPENSE",
        }
    }

    /// Returns the `m_code` option name for tags associated with this account type
    /// (`AccountingConstants.*_TAG_OPTION_CODE_NAME`).
    pub const fn tag_option_code_name(self) -> &'static str {
        match self {
            Self::Asset => "AssetAccountTags",
            Self::Liability => "LiabilityAccountTags",
            Self::Equity => "EquityAccountTags",
            Self::Income => "IncomeAccountTags",
            Self::Expense => "ExpenseAccountTags",
        }
    }

    /// Converts an `i32` discriminant into [`GLAccountType`], matching `GLAccountType.fromInt(Integer)`.
    pub const fn from_int(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Asset),
            2 => Some(Self::Liability),
            3 => Some(Self::Equity),
            4 => Some(Self::Income),
            5 => Some(Self::Expense),
            _ => None,
        }
    }

    /// Converts an `i16` discriminant into [`GLAccountType`].
    pub const fn from_i16(v: i16) -> Option<Self> {
        Self::from_int(v as i32)
    }

    /// Parses a string name or code into [`GLAccountType`] (`GLAccountType.fromString`).
    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "ASSET" | "ACCOUNTTYPE.ASSET" => Some(Self::Asset),
            "LIABILITY" | "ACCOUNTTYPE.LIABILITY" => Some(Self::Liability),
            "EQUITY" | "ACCOUNTTYPE.EQUITY" => Some(Self::Equity),
            "INCOME" | "ACCOUNTTYPE.INCOME" => Some(Self::Income),
            "EXPENSE" | "ACCOUNTTYPE.EXPENSE" => Some(Self::Expense),
            _ => None,
        }
    }

    /// Returns `true` if `self == GLAccountType::Asset` (`GLAccountType.isAssetType()`).
    pub const fn is_asset_type(self) -> bool {
        matches!(self, Self::Asset)
    }

    /// Alias for [`GLAccountType::is_asset_type`].
    pub const fn is_asset(self) -> bool {
        self.is_asset_type()
    }

    /// Returns `true` if `self == GLAccountType::Liability` (`GLAccountType.isLiabilityType()`).
    pub const fn is_liability_type(self) -> bool {
        matches!(self, Self::Liability)
    }

    /// Alias for [`GLAccountType::is_liability_type`].
    pub const fn is_liability(self) -> bool {
        self.is_liability_type()
    }

    /// Returns `true` if `self == GLAccountType::Equity` (`GLAccountType.isEquityType()`).
    pub const fn is_equity_type(self) -> bool {
        matches!(self, Self::Equity)
    }

    /// Alias for [`GLAccountType::is_equity_type`].
    pub const fn is_equity(self) -> bool {
        self.is_equity_type()
    }

    /// Returns `true` if `self == GLAccountType::Income` (`GLAccountType.isIncomeType()`).
    pub const fn is_income_type(self) -> bool {
        matches!(self, Self::Income)
    }

    /// Alias for [`GLAccountType::is_income_type`].
    pub const fn is_income(self) -> bool {
        self.is_income_type()
    }

    /// Returns `true` if `self == GLAccountType::Expense` (`GLAccountType.isExpenseType()`).
    pub const fn is_expense_type(self) -> bool {
        matches!(self, Self::Expense)
    }

    /// Alias for [`GLAccountType::is_expense_type`].
    pub const fn is_expense(self) -> bool {
        self.is_expense_type()
    }

    /// Returns whether a debit entry increases the normal balance of this account classification
    /// (`ASSET` and `EXPENSE` are debit-normal; `LIABILITY`, `EQUITY`, and `INCOME` are credit-normal).
    pub const fn is_debit_normal(self) -> bool {
        matches!(self, Self::Asset | Self::Expense)
    }
}

impl fmt::Display for GLAccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

impl FromStr for GLAccountType {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.glaccount.type.invalid",
                format!("Invalid GL account type: {s}"),
            )
        })
    }
}

impl TryFrom<i32> for GLAccountType {
    type Error = FineractError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_int(value).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.glaccount.type.invalid",
                format!("Invalid GL account type value: {value}"),
            )
        })
    }
}

impl TryFrom<i16> for GLAccountType {
    type Error = FineractError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::try_from(i32::from(value))
    }
}

impl From<GLAccountType> for i32 {
    fn from(value: GLAccountType) -> Self {
        value.value()
    }
}

impl From<GLAccountType> for i16 {
    fn from(value: GLAccountType) -> Self {
        value.as_i16()
    }
}

// =============================================================================
// 2. GLAccountUsage (org.apache.fineract.accounting.glaccount.domain.GLAccountUsage)
// =============================================================================

/// General Ledger account usage (`acc_gl_account.account_usage`).
///
/// - `Detail = 1`: Leaf account eligible for journal entry postings.
/// - `Header = 2`: Grouping/summary account eligible to have child accounts.
///
/// Ported from `org.apache.fineract.accounting.glaccount.domain.GLAccountUsage`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i32)]
pub enum GLAccountUsage {
    #[default]
    Detail = 1,
    Header = 2,
}

impl GLAccountUsage {
    /// Java-compatible constant alias for [`GLAccountUsage::Detail`].
    pub const DETAIL: Self = Self::Detail;
    /// Java-compatible constant alias for [`GLAccountUsage::Header`].
    pub const HEADER: Self = Self::Header;

    /// Minimum valid discriminant (`GLAccountUsage.getMinValue()`).
    pub const MIN_VALUE: i32 = 1;
    /// Maximum valid discriminant (`GLAccountUsage.getMaxValue()`).
    pub const MAX_VALUE: i32 = 2;

    /// Returns the integer discriminant matching `GLAccountUsage.getValue()`.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Returns the integer discriminant as `i32`.
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    /// Returns the `SMALLINT` (`i16`) database representation stored in `acc_gl_account.account_usage`.
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Returns `GLAccountUsage.getMinValue()`.
    pub const fn min_value() -> i32 {
        Self::MIN_VALUE
    }

    /// Returns `GLAccountUsage.getMaxValue()`.
    pub const fn max_value() -> i32 {
        Self::MAX_VALUE
    }

    /// Returns the globalisation message code matching `GLAccountUsage.getCode()`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Detail => "accountUsage.detail",
            Self::Header => "accountUsage.header",
        }
    }

    /// Returns the uppercase Java enum name (`GLAccountUsage.name()`).
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::Detail => "DETAIL",
            Self::Header => "HEADER",
        }
    }

    /// Converts an `i32` discriminant into [`GLAccountUsage`], matching `GLAccountUsage.fromInt(int)`.
    pub const fn from_int(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Detail),
            2 => Some(Self::Header),
            _ => None,
        }
    }

    /// Converts an `i16` discriminant into [`GLAccountUsage`].
    pub const fn from_i16(v: i16) -> Option<Self> {
        Self::from_int(v as i32)
    }

    /// Returns `true` if `self == GLAccountUsage::Detail`.
    pub const fn is_detail(self) -> bool {
        matches!(self, Self::Detail)
    }

    /// Returns `true` if `self == GLAccountUsage::Header`.
    pub const fn is_header(self) -> bool {
        matches!(self, Self::Header)
    }
}

impl fmt::Display for GLAccountUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

impl FromStr for GLAccountUsage {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "DETAIL" | "ACCOUNTUSAGE.DETAIL" => Ok(Self::Detail),
            "HEADER" | "ACCOUNTUSAGE.HEADER" => Ok(Self::Header),
            _ => Err(FineractError::domain_rule(
                "error.msg.glaccount.usage.invalid",
                format!("Invalid GL account usage: {s}"),
            )),
        }
    }
}

impl TryFrom<i32> for GLAccountUsage {
    type Error = FineractError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_int(value).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.glaccount.usage.invalid",
                format!("Invalid GL account usage value: {value}"),
            )
        })
    }
}

impl TryFrom<i16> for GLAccountUsage {
    type Error = FineractError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::try_from(i32::from(value))
    }
}

impl From<GLAccountUsage> for i32 {
    fn from(value: GLAccountUsage) -> Self {
        value.value()
    }
}

impl From<GLAccountUsage> for i16 {
    fn from(value: GLAccountUsage) -> Self {
        value.as_i16()
    }
}

// =============================================================================
// 3. FinancialActivity (org.apache.fineract.accounting.common.AccountingConstants.FinancialActivity)
// =============================================================================

/// Organization-wide financial activity placeholders mapped to specific GL accounts
/// (`acc_gl_financial_activity_account.financial_activity_type`).
///
/// Ported from `org.apache.fineract.accounting.common.AccountingConstants.FinancialActivity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum FinancialActivity {
    AssetTransfer = 100,
    CashAtMainVault = 101,
    CashAtTeller = 102,
    AssetFundSource = 103,
    LiabilityTransfer = 200,
    PayableDividends = 201,
    OpeningBalancesTransferContra = 300,
}

impl FinancialActivity {
    /// Java-compatible constant alias for [`FinancialActivity::AssetTransfer`].
    pub const ASSET_TRANSFER: Self = Self::AssetTransfer;
    /// Java-compatible constant alias for [`FinancialActivity::LiabilityTransfer`].
    pub const LIABILITY_TRANSFER: Self = Self::LiabilityTransfer;
    /// Java-compatible constant alias for [`FinancialActivity::CashAtMainVault`].
    pub const CASH_AT_MAINVAULT: Self = Self::CashAtMainVault;
    /// Alias for [`FinancialActivity::CashAtMainVault`].
    pub const CASH_AT_MAIN_VAULT: Self = Self::CashAtMainVault;
    /// Java-compatible constant alias for [`FinancialActivity::CashAtTeller`].
    pub const CASH_AT_TELLER: Self = Self::CashAtTeller;
    /// Java-compatible constant alias for [`FinancialActivity::OpeningBalancesTransferContra`].
    pub const OPENING_BALANCES_TRANSFER_CONTRA: Self = Self::OpeningBalancesTransferContra;
    /// Java-compatible constant alias for [`FinancialActivity::AssetFundSource`].
    pub const ASSET_FUND_SOURCE: Self = Self::AssetFundSource;
    /// Java-compatible constant alias for [`FinancialActivity::PayableDividends`].
    pub const PAYABLE_DIVIDENDS: Self = Self::PayableDividends;

    /// Returns the required [`GLAccountType`] for this financial activity,
    /// matching `FinancialActivity.getMappedGLAccountType()`.
    pub const fn mapped_gl_account_type(&self) -> GLAccountType {
        match self {
            Self::AssetTransfer => GLAccountType::Asset,
            Self::LiabilityTransfer => GLAccountType::Liability,
            Self::CashAtMainVault => GLAccountType::Asset,
            Self::CashAtTeller => GLAccountType::Asset,
            Self::OpeningBalancesTransferContra => GLAccountType::Equity,
            Self::AssetFundSource => GLAccountType::Asset,
            Self::PayableDividends => GLAccountType::Liability,
        }
    }

    /// Returns the integer activity ID matching `FinancialActivity.getValue()`.
    pub const fn value(&self) -> i32 {
        *self as i32
    }

    /// Returns the integer activity ID as `i32`.
    pub const fn as_int(self) -> i32 {
        self as i32
    }

    /// Returns the `SMALLINT` (`i16`) database representation stored in
    /// `acc_gl_financial_activity_account.financial_activity_type`.
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    /// Returns the camelCase code matching `FinancialActivity.getCode()`.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::AssetTransfer => "assetTransfer",
            Self::LiabilityTransfer => "liabilityTransfer",
            Self::CashAtMainVault => "cashAtMainVault",
            Self::CashAtTeller => "cashAtTeller",
            Self::OpeningBalancesTransferContra => "openingBalancesTransferContra",
            Self::AssetFundSource => "fundSource",
            Self::PayableDividends => "payableDividends",
        }
    }

    /// Converts an `i32` activity ID into [`FinancialActivity`], matching `FinancialActivity.fromInt(int)`.
    pub const fn from_int(financial_activity_id: i32) -> Option<Self> {
        match financial_activity_id {
            100 => Some(Self::AssetTransfer),
            200 => Some(Self::LiabilityTransfer),
            101 => Some(Self::CashAtMainVault),
            102 => Some(Self::CashAtTeller),
            300 => Some(Self::OpeningBalancesTransferContra),
            103 => Some(Self::AssetFundSource),
            201 => Some(Self::PayableDividends),
            _ => None,
        }
    }

    /// Converts an `i16` activity ID into [`FinancialActivity`].
    pub const fn from_i16(financial_activity_id: i16) -> Option<Self> {
        Self::from_int(financial_activity_id as i32)
    }

    /// Returns all defined [`FinancialActivity`] variants in upstream declaration order.
    pub const fn all() -> [Self; 7] {
        [
            Self::AssetTransfer,
            Self::LiabilityTransfer,
            Self::CashAtMainVault,
            Self::CashAtTeller,
            Self::OpeningBalancesTransferContra,
            Self::AssetFundSource,
            Self::PayableDividends,
        ]
    }

    /// Returns the valid integer discriminants for `DataValidatorBuilder::is_one_of_these_values`.
    pub const fn valid_values() -> [i64; 7] {
        [100, 200, 101, 102, 300, 103, 201]
    }
}

impl fmt::Display for FinancialActivity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::AssetTransfer => "ASSET TRANSFER",
            Self::LiabilityTransfer => "LIABILITY TRANSFER",
            Self::CashAtMainVault => "CASH AT MAINVAULT",
            Self::CashAtTeller => "CASH AT TELLER",
            Self::OpeningBalancesTransferContra => "OPENING BALANCES TRANSFER CONTRA",
            Self::AssetFundSource => "ASSET FUND SOURCE",
            Self::PayableDividends => "PAYABLE DIVIDENDS",
        };
        f.write_str(s)
    }
}

impl TryFrom<i32> for FinancialActivity {
    type Error = FineractError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_int(value).ok_or_else(|| {
            FineractError::domain_rule(
                "error.msg.financialActivityAccount.not.found",
                format!("Financial Activity account with for the financial Activity with Id {value} does not exist"),
            )
        })
    }
}

impl TryFrom<i16> for FinancialActivity {
    type Error = FineractError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::try_from(i32::from(value))
    }
}

// =============================================================================
// 4. GLAccount (acc_gl_account)
// =============================================================================

/// General Ledger account entity matching `acc_gl_account` in `anchor/fineract_schema_dual.sql`.
///
/// Ported from `org.apache.fineract.accounting.glaccount.domain.GLAccount`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GLAccount {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub hierarchy: Option<String>,
    pub name: String,
    pub gl_code: String,
    pub disabled: bool,
    pub manual_entries_allowed: bool,
    pub account_type: GLAccountType,
    pub account_usage: GLAccountUsage,
    pub description: Option<String>,
    pub tag_id: Option<i64>,
}

impl GLAccount {
    /// Supported JSON input parameters from `GLAccountJsonInputParams.java`.
    pub const SUPPORTED_PARAMS: &'static [&'static str] = &[
        "id",
        "name",
        "parentId",
        "glCode",
        "disabled",
        "manualEntriesAllowed",
        "type",
        "usage",
        "description",
        "tagId",
    ];

    /// Creates a new [`GLAccount`] instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        parent_id: Option<i64>,
        name: impl Into<String>,
        gl_code: impl Into<String>,
        disabled: bool,
        manual_entries_allowed: bool,
        account_type: GLAccountType,
        account_usage: GLAccountUsage,
        description: Option<String>,
        tag_id: Option<i64>,
    ) -> Self {
        let hierarchy = if parent_id.is_none() && id > 0 {
            Some(".".to_string())
        } else {
            None
        };
        Self {
            id,
            parent_id,
            hierarchy,
            name: name.into(),
            gl_code: gl_code.into(),
            disabled,
            manual_entries_allowed,
            account_type,
            account_usage,
            description,
            tag_id,
        }
    }

    /// Creates a `Header` [`GLAccount`] convenience instance.
    pub fn new_header(
        id: i64,
        parent_id: Option<i64>,
        name: impl Into<String>,
        gl_code: impl Into<String>,
        account_type: GLAccountType,
    ) -> Self {
        Self::new(
            id,
            parent_id,
            name,
            gl_code,
            false,
            false,
            account_type,
            GLAccountUsage::Header,
            None,
            None,
        )
    }

    /// Creates a `Detail` [`GLAccount`] convenience instance.
    pub fn new_detail(
        id: i64,
        parent_id: Option<i64>,
        name: impl Into<String>,
        gl_code: impl Into<String>,
        account_type: GLAccountType,
        manual_entries_allowed: bool,
    ) -> Self {
        Self::new(
            id,
            parent_id,
            name,
            gl_code,
            false,
            manual_entries_allowed,
            account_type,
            GLAccountUsage::Detail,
            None,
            None,
        )
    }

    /// Returns `true` if `self.account_usage == GLAccountUsage::Header` (`GLAccount.isHeaderAccount()`).
    pub const fn is_header_account(&self) -> bool {
        matches!(self.account_usage, GLAccountUsage::Header)
    }

    /// Returns `true` if `self.account_usage == GLAccountUsage::Detail` (`GLAccount.isDetailAccount()`).
    pub const fn is_detail_account(&self) -> bool {
        matches!(self.account_usage, GLAccountUsage::Detail)
    }

    /// Computes the child hierarchy path string for `child_id` under this account,
    /// matching `GLAccount.hierarchyOf(Long id)`.
    pub fn hierarchy_of(&self, child_id: i64) -> String {
        let base = self.hierarchy.as_deref().unwrap_or(".");
        if base.ends_with('.') {
            format!("{base}{child_id}.")
        } else {
            format!("{base}.{child_id}.")
        }
    }

    /// Populates `self.hierarchy` from `parent`, matching `GLAccount.generateHierarchy()`.
    ///
    /// - Root accounts (`parent == None`) receive `"."`.
    /// - Child accounts receive `parent.hierarchyOf(self.id)` (e.g., `".12."`).
    pub fn generate_hierarchy(&mut self, parent: Option<&GLAccount>) -> Result<(), FineractError> {
        if let Some(p) = parent {
            p.validate_can_have_child(Some(self.id))?;
            self.parent_id = Some(p.id);
            self.hierarchy = Some(p.hierarchy_of(self.id));
        } else {
            self.parent_id = None;
            self.hierarchy = Some(".".to_string());
        }
        Ok(())
    }

    /// Enforces that only `Header` accounts can have children (`GLAccountWritePlatformServiceJpaRepositoryImpl.validateParentGLAccount`)
    /// and that an account cannot be its own parent (`InvalidParentGLAccountHeadException`).
    pub fn validate_can_have_child(&self, child_id: Option<i64>) -> Result<(), FineractError> {
        if let Some(cid) = child_id {
            if cid > 0 && cid == self.id {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.glaccount.id.and.parentid.must.not.same",
                        format!("parentId:{}, id{} should not be same", self.id, cid),
                        vec![ApiErrorArg::from(cid), ApiErrorArg::from(self.id)],
                    ),
                ));
            }
        }
        if self.is_detail_account() {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.glaccount.parent.invalid",
                    format!(
                        "The account with id {} is a 'Detail' account and cannot be used as a parent",
                        self.id
                    ),
                    vec![ApiErrorArg::from(self.id)],
                ),
            ));
        }
        Ok(())
    }

    /// Alias for [`GLAccount::validate_can_have_child`] without a child ID.
    pub fn validate_as_parent(&self) -> Result<(), FineractError> {
        self.validate_can_have_child(None)
    }

    /// Validates that this account can receive journal entry postings:
    /// 1. Must be a `Detail` account (`Header` accounts cannot have journal entries posted directly).
    /// 2. Must not be `disabled` (`GlJournalEntryInvalidReason.GL_ACCOUNT_DISABLED`).
    /// 3. If `is_manual_entry` is `true`, must have `manual_entries_allowed == true`
    ///    (`GlJournalEntryInvalidReason.GL_ACCOUNT_MANUAL_ENTRIES_NOT_PERMITTED`).
    pub fn validate_for_journal_posting(&self, is_manual_entry: bool) -> Result<(), FineractError> {
        if !self.is_detail_account() {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.glJournalEntry.invalid.debit.or.credit.accounts",
                    format!(
                        "Journal entry cannot be posted to Header GL account '{}' ({})",
                        self.name, self.gl_code
                    ),
                    vec![
                        ApiErrorArg::from(self.name.as_str()),
                        ApiErrorArg::from(self.gl_code.as_str()),
                    ],
                ),
            ));
        }
        if self.disabled {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.glJournalEntry.invalid.account.disabled",
                    "Target account has been disabled",
                    vec![
                        ApiErrorArg::from(self.name.as_str()),
                        ApiErrorArg::from(self.gl_code.as_str()),
                    ],
                ),
            ));
        }
        if is_manual_entry && !self.manual_entries_allowed {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.glJournalEntry.invalid.account.manual.adjustments.not.permitted",
                    "Target account does not allow manual adjustments",
                    vec![
                        ApiErrorArg::from(self.name.as_str()),
                        ApiErrorArg::from(self.gl_code.as_str()),
                    ],
                ),
            ));
        }
        Ok(())
    }

    /// Validates that this account is a `Detail` account and is not disabled (for system/product postings).
    pub fn validate_for_posting(&self) -> Result<(), FineractError> {
        self.validate_for_journal_posting(false)
    }

    /// Validates a `CREATE GLACCOUNT` [`JsonCommand`] matching `GLAccountCommand.validateForCreate()`.
    pub fn validate_for_create(command: &JsonCommand) -> Result<(), FineractError> {
        command.check_for_unsupported_parameters(Self::SUPPORTED_PARAMS)?;

        let name = command.string_value_of_parameter_named("name");
        let gl_code = command.string_value_of_parameter_named("glCode");
        let parent_id = command.long_value_of_parameter_named("parentId")?;
        let account_type = command.integer_value_sans_locale_of_parameter_named("type")?;
        let usage = command.integer_value_sans_locale_of_parameter_named("usage")?;
        let description = command.string_value_of_parameter_named("description");
        let manual_entries_allowed =
            command.boolean_object_value_of_parameter_named("manualEntriesAllowed");
        let tag_id = command.long_value_of_parameter_named("tagId")?;

        let validator = DataValidatorBuilder::new().resource("GLAccount");
        validator
            .reset()
            .parameter("name")
            .value(name)
            .not_blank()
            .not_exceeding_length_of(200);
        validator
            .reset()
            .parameter("glCode")
            .value(gl_code)
            .not_blank()
            .not_exceeding_length_of(45);
        validator
            .reset()
            .parameter("parentId")
            .value(parent_id)
            .ignore_if_null()
            .integer_greater_than_zero();
        validator
            .reset()
            .parameter("type")
            .value(account_type)
            .not_null()
            .in_min_max_range(
                i64::from(GLAccountType::MIN_VALUE),
                i64::from(GLAccountType::MAX_VALUE),
            );
        validator
            .reset()
            .parameter("usage")
            .value(usage)
            .in_min_max_range(
                i64::from(GLAccountUsage::MIN_VALUE),
                i64::from(GLAccountUsage::MAX_VALUE),
            );
        validator
            .reset()
            .parameter("description")
            .value(description)
            .ignore_if_null()
            .not_exceeding_length_of(500);
        validator
            .reset()
            .parameter("manualEntriesAllowed")
            .value(manual_entries_allowed)
            .not_blank();
        validator
            .reset()
            .parameter("tagId")
            .value(tag_id)
            .ignore_if_null()
            .long_greater_than_zero();

        validator.throw_validation_errors()?;
        Ok(())
    }

    /// Constructs a [`GLAccount`] from a validated [`JsonCommand`] and optional parent [`GLAccount`],
    /// matching `GLAccount.fromJson(...)`.
    pub fn from_json(
        id: i64,
        parent: Option<&GLAccount>,
        command: &JsonCommand,
    ) -> Result<Self, FineractError> {
        Self::validate_for_create(command)?;
        if let Some(p) = parent {
            p.validate_can_have_child(if id > 0 { Some(id) } else { None })?;
        }

        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let gl_code = command
            .string_value_of_parameter_named("glCode")
            .unwrap_or_default();
        let disabled = command.boolean_primitive_value_of_parameter_named("disabled");
        let manual_entries_allowed =
            command.boolean_primitive_value_of_parameter_named("manualEntriesAllowed");
        let type_int = command
            .integer_value_sans_locale_of_parameter_named("type")?
            .unwrap_or(0);
        let usage_int = command
            .integer_value_sans_locale_of_parameter_named("usage")?
            .unwrap_or(0);
        let account_type = GLAccountType::try_from(type_int)?;
        let account_usage = GLAccountUsage::try_from(usage_int)?;
        let description = command.string_value_of_parameter_named("description");
        let tag_id = command.long_value_of_parameter_named("tagId")?;
        let parent_id = parent
            .map(|p| p.id)
            .or(command.long_value_of_parameter_named("parentId")?);

        let mut account = Self::new(
            id,
            parent_id,
            name,
            gl_code,
            disabled,
            manual_entries_allowed,
            account_type,
            account_usage,
            description,
            tag_id,
        );
        if id > 0 {
            account.generate_hierarchy(parent)?;
        }
        Ok(account)
    }

    /// Updates mutable fields of this [`GLAccount`] from a [`JsonCommand`], returning the changed parameter map
    /// matching `GLAccount.update(JsonCommand)`.
    pub fn update(
        &mut self,
        command: &JsonCommand,
    ) -> Result<BTreeMap<String, serde_json::Value>, FineractError> {
        let mut actual_changes = BTreeMap::new();

        if command.is_change_in_string_parameter_named("description", self.description.as_deref()) {
            let new_val = command.string_value_of_parameter_named("description");
            actual_changes.insert(
                "description".to_string(),
                new_val
                    .clone()
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            );
            self.description = new_val;
        }

        if command.is_change_in_boolean_parameter_named("disabled", Some(self.disabled)) {
            if let Some(new_val) = command.boolean_object_value_of_parameter_named("disabled") {
                actual_changes.insert("disabled".to_string(), serde_json::Value::Bool(new_val));
                self.disabled = new_val;
            }
        }

        if command.is_change_in_string_parameter_named("glCode", Some(self.gl_code.as_str())) {
            if let Some(new_val) = command.string_value_of_parameter_named("glCode") {
                actual_changes.insert(
                    "glCode".to_string(),
                    serde_json::Value::String(new_val.clone()),
                );
                self.gl_code = new_val;
            }
        }

        if command.is_change_in_boolean_parameter_named(
            "manualEntriesAllowed",
            Some(self.manual_entries_allowed),
        ) {
            if let Some(new_val) =
                command.boolean_object_value_of_parameter_named("manualEntriesAllowed")
            {
                actual_changes.insert(
                    "manualEntriesAllowed".to_string(),
                    serde_json::Value::Bool(new_val),
                );
                self.manual_entries_allowed = new_val;
            }
        }

        if command.is_change_in_string_parameter_named("name", Some(self.name.as_str())) {
            if let Some(new_val) = command.string_value_of_parameter_named("name") {
                actual_changes.insert(
                    "name".to_string(),
                    serde_json::Value::String(new_val.clone()),
                );
                self.name = new_val;
            }
        }

        if command.parameter_exists("parentId") {
            let new_parent = command.long_value_of_parameter_named("parentId")?;
            if new_parent != self.parent_id {
                actual_changes.insert(
                    "parentId".to_string(),
                    new_parent
                        .map(serde_json::Value::from)
                        .unwrap_or(serde_json::Value::Null),
                );
                self.parent_id = new_parent;
            }
        }

        if command.parameter_exists("type") {
            if let Some(new_type_int) =
                command.integer_value_sans_locale_of_parameter_named("type")?
            {
                if new_type_int != self.account_type.value() {
                    let new_type = GLAccountType::try_from(new_type_int)?;
                    actual_changes
                        .insert("type".to_string(), serde_json::Value::from(new_type_int));
                    self.account_type = new_type;
                }
            }
        }

        if command.parameter_exists("usage") {
            if let Some(new_usage_int) =
                command.integer_value_sans_locale_of_parameter_named("usage")?
            {
                if new_usage_int != self.account_usage.value() {
                    let new_usage = GLAccountUsage::try_from(new_usage_int)?;
                    actual_changes
                        .insert("usage".to_string(), serde_json::Value::from(new_usage_int));
                    self.account_usage = new_usage;
                }
            }
        }

        if command.is_change_in_long_parameter_named("tagId", Some(self.tag_id.unwrap_or(0)))? {
            let new_tag = command.long_value_of_parameter_named("tagId")?;
            actual_changes.insert(
                "tagId".to_string(),
                new_tag
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
            );
            self.tag_id = new_tag;
        }

        Ok(actual_changes)
    }
}

// =============================================================================
// 5. GLClosure (acc_gl_closure)
// =============================================================================

/// Accounting branch closure entity matching `acc_gl_closure` in `anchor/fineract_schema_dual.sql`.
///
/// Ported from `org.apache.fineract.accounting.closure.domain.GLClosure`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GLClosure {
    pub id: i64,
    pub office_id: i64,
    pub closing_date: NaiveDate,
    pub is_deleted: bool,
    pub comments: Option<String>,
}

impl GLClosure {
    /// Supported JSON input parameters from `GLClosureJsonInputParams.java`.
    pub const SUPPORTED_PARAMS: &'static [&'static str] = &[
        "id",
        "officeId",
        "closingDate",
        "comments",
        "locale",
        "dateFormat",
    ];

    /// Creates a new active (`is_deleted = false`) [`GLClosure`], trimming and normalizing empty
    /// comments to `None` (matching `GLClosure.java`).
    pub fn new(
        id: i64,
        office_id: i64,
        closing_date: NaiveDate,
        comments: Option<String>,
    ) -> Self {
        let normalized_comments = comments
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty());
        Self {
            id,
            office_id,
            closing_date,
            is_deleted: false,
            comments: normalized_comments,
        }
    }

    /// Validates that a journal entry or financial transaction on `txn_date` occurs strictly after
    /// the branch's latest active [`GLClosure`], matching
    /// `AccountingProcessorHelperImpl.checkForBranchClosures` and
    /// `JournalEntryWritePlatformServiceImpl.validateBusinessRulesForJournalEntries`:
    ///
    /// ```java
    /// if (latestGLClosure != null) {
    ///     if (!DateUtils.isBefore(latestGLClosure.getClosingDate(), transactionDate)) {
    ///         throw new JournalEntryInvalidException(GlJournalEntryInvalidReason.ACCOUNTING_CLOSED,
    ///             latestGLClosure.getClosingDate(), null, null);
    ///     }
    /// }
    /// ```
    pub fn validate_transaction_date_after_closure(
        closure: Option<&GLClosure>,
        txn_date: NaiveDate,
    ) -> Result<(), FineractError> {
        if let Some(gl_closure) = closure {
            if !gl_closure.is_deleted && txn_date <= gl_closure.closing_date {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.glJournalEntry.invalid.accounting.closed",
                        "Journal entry cannot be made prior to last account closing date for the branch",
                        vec![ApiErrorArg::from_date(gl_closure.closing_date)],
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Instance method convenience wrapper around [`GLClosure::validate_transaction_date_after_closure`].
    pub fn validate_transaction_date(&self, txn_date: NaiveDate) -> Result<(), FineractError> {
        Self::validate_transaction_date_after_closure(Some(self), txn_date)
    }

    /// Validates a `CREATE GLCLOSURE` [`JsonCommand`] matching `GLClosureCommand.validateForCreate()`.
    pub fn validate_for_create(command: &JsonCommand) -> Result<(), FineractError> {
        command.check_for_unsupported_parameters(Self::SUPPORTED_PARAMS)?;

        let closing_date = command.local_date_value_of_parameter_named("closingDate")?;
        let office_id = command.long_value_of_parameter_named("officeId")?;
        let comments = command.string_value_of_parameter_named("comments");

        let validator = DataValidatorBuilder::new().resource("GLClosure");
        validator
            .reset()
            .parameter("closingDate")
            .value(closing_date)
            .not_blank();
        validator
            .reset()
            .parameter("officeId")
            .value(office_id)
            .not_null()
            .integer_greater_than_zero();
        validator
            .reset()
            .parameter("comments")
            .value(comments)
            .ignore_if_null()
            .not_exceeding_length_of(500);

        validator.throw_validation_errors()?;
        Ok(())
    }

    /// Constructs a [`GLClosure`] from a validated [`JsonCommand`], matching `GLClosure.fromJson(...)`.
    pub fn from_json(id: i64, command: &JsonCommand) -> Result<Self, FineractError> {
        Self::validate_for_create(command)?;
        let office_id = command
            .long_value_of_parameter_named("officeId")?
            .unwrap_or(0);
        let closing_date = command
            .local_date_value_of_parameter_named("closingDate")?
            .expect("closingDate validated as not_blank");
        let comments = command.string_value_of_parameter_named("comments");
        Ok(Self::new(id, office_id, closing_date, comments))
    }
}

/// Top-level helper validating that `txn_date` is strictly after `closure.closing_date` when an
/// active [`GLClosure`] exists for the branch.
///
/// Delegates directly to [`GLClosure::validate_transaction_date_after_closure`].
pub fn validate_transaction_date_after_closure(
    closure: Option<&GLClosure>,
    txn_date: NaiveDate,
) -> Result<(), FineractError> {
    GLClosure::validate_transaction_date_after_closure(closure, txn_date)
}

// =============================================================================
// 6. FinancialActivityAccount (acc_gl_financial_activity_account)
// =============================================================================

/// Mapping between a [`FinancialActivity`] and a [`GLAccount`]
/// (`acc_gl_financial_activity_account` in `anchor/fineract_schema_dual.sql`).
///
/// Ported from `org.apache.fineract.accounting.financialactivityaccount.domain.FinancialActivityAccount`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancialActivityAccount {
    pub id: i64,
    pub gl_account_id: i64,
    pub financial_activity_type: FinancialActivity,
}

impl FinancialActivityAccount {
    /// Supported JSON input parameters from `FinancialActivityAccountsJsonInputParams.java`.
    pub const SUPPORTED_PARAMS: &'static [&'static str] = &["financialActivityId", "glAccountId"];

    /// Creates a [`FinancialActivityAccount`] struct without bound account validation.
    pub const fn new(
        id: i64,
        gl_account_id: i64,
        financial_activity_type: FinancialActivity,
    ) -> Self {
        Self {
            id,
            gl_account_id,
            financial_activity_type,
        }
    }

    /// Creates a new [`FinancialActivityAccount`] and validates that `gl_account.account_type`
    /// matches `financial_activity_type.mapped_gl_account_type()`, matching
    /// `FinancialActivityAccountWritePlatformServiceImpl.validateFinancialActivityAndAccountMapping`.
    pub fn create_new(
        id: i64,
        gl_account: &GLAccount,
        financial_activity_type: FinancialActivity,
    ) -> Result<Self, FineractError> {
        Self::validate_gl_account_for_activity(financial_activity_type, gl_account)?;
        Ok(Self {
            id,
            gl_account_id: gl_account.id,
            financial_activity_type,
        })
    }

    /// Validates that the bound [`GLAccount`]'s `account_type` matches
    /// `self.financial_activity_type.mapped_gl_account_type()`.
    pub fn validate_mapping(&self, gl_account: &GLAccount) -> Result<(), FineractError> {
        Self::validate_gl_account_for_activity(self.financial_activity_type, gl_account)
    }

    /// Validates that `gl_account.account_type == financial_activity.mapped_gl_account_type()`,
    /// throwing `FinancialActivityAccountInvalidException` (`error.msg.financialActivityAccount.invalid`)
    /// on mismatch.
    pub fn validate_gl_account_for_activity(
        financial_activity: FinancialActivity,
        gl_account: &GLAccount,
    ) -> Result<(), FineractError> {
        let expected_type = financial_activity.mapped_gl_account_type();
        if gl_account.account_type != expected_type {
            let msg = format!(
                "Financial Activity '{}' with Id :'{}' can only be associated with a Ledger Account of Type {} the provided Ledger Account '{}({})'  does not of the required type",
                financial_activity.code(),
                financial_activity.value(),
                expected_type.code(),
                gl_account.name,
                gl_account.gl_code
            );
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.financialActivityAccount.invalid",
                    msg,
                    vec![
                        ApiErrorArg::from(financial_activity.code()),
                        ApiErrorArg::from(financial_activity.value()),
                        ApiErrorArg::from(expected_type.code()),
                        ApiErrorArg::from(gl_account.name.as_str()),
                        ApiErrorArg::from(gl_account.gl_code.as_str()),
                    ],
                ),
            ));
        }
        Ok(())
    }

    /// Validates a `CREATE FINANCIALACTIVITYACCOUNT` [`JsonCommand`] matching
    /// `FinancialActivityAccountDataValidator.validateForCreate`.
    pub fn validate_for_create(command: &JsonCommand) -> Result<(), FineractError> {
        command.check_for_unsupported_parameters(Self::SUPPORTED_PARAMS)?;

        let financial_activity_id =
            command.integer_value_sans_locale_of_parameter_named("financialActivityId")?;
        let gl_account_id = command.long_value_of_parameter_named("glAccountId")?;

        let validator = DataValidatorBuilder::new().resource("financialactivityaccount");
        validator
            .reset()
            .parameter("financialActivityId")
            .value(financial_activity_id)
            .not_null()
            .is_one_of_these_values(FinancialActivity::valid_values());
        validator
            .reset()
            .parameter("glAccountId")
            .value(gl_account_id)
            .not_null()
            .integer_greater_than_zero();

        validator.throw_validation_errors()?;
        Ok(())
    }
}

// =============================================================================
// 7. AccountingRule (acc_accounting_rule + acc_rule_tags)
// =============================================================================

/// Accounting rule entity matching `acc_accounting_rule` and `acc_rule_tags` in
/// `anchor/fineract_schema_dual.sql`.
///
/// Ported from `org.apache.fineract.accounting.rule.domain.AccountingRule` and `AccountingTagRule`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountingRule {
    pub id: i64,
    pub name: String,
    pub office_id: Option<i64>,
    pub debit_account_id: Option<i64>,
    pub allow_multiple_debits: bool,
    pub debit_tag_ids: Vec<i64>,
    pub credit_account_id: Option<i64>,
    pub allow_multiple_credits: bool,
    pub credit_tag_ids: Vec<i64>,
    pub system_defined: bool,
    pub description: Option<String>,
}

impl AccountingRule {
    /// Supported JSON input parameters from `AccountingRuleJsonInputParams.java`.
    pub const SUPPORTED_PARAMS: &'static [&'static str] = &[
        "id",
        "officeId",
        "accountToDebit",
        "accountToCredit",
        "name",
        "description",
        "systemDefined",
        "debitTags",
        "creditTags",
        "allowMultipleCreditEntries",
        "allowMultipleDebitEntries",
        "locale",
    ];

    /// `JournalEntryType.CREDIT.getValue()` (`1`) stored in `acc_rule_tags.acc_type_enum`.
    pub const CREDIT_TAG_TYPE_ENUM: i16 = 1;
    /// `JournalEntryType.DEBIT.getValue()` (`2`) stored in `acc_rule_tags.acc_type_enum`.
    pub const DEBIT_TAG_TYPE_ENUM: i16 = 2;

    /// Creates a new [`AccountingRule`] instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        name: impl Into<String>,
        office_id: Option<i64>,
        debit_account_id: Option<i64>,
        allow_multiple_debits: bool,
        debit_tag_ids: Vec<i64>,
        credit_account_id: Option<i64>,
        allow_multiple_credits: bool,
        credit_tag_ids: Vec<i64>,
        system_defined: bool,
        description: Option<String>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            office_id,
            debit_account_id,
            allow_multiple_debits,
            debit_tag_ids,
            credit_account_id,
            allow_multiple_credits,
            credit_tag_ids,
            system_defined,
            description: description
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty()),
        }
    }

    /// Validates domain invariants on [`AccountingRule`], matching
    /// `AccountingRuleWritePlatformServiceJpaRepositoryImpl.assembleAccountingRuleAndTags`:
    /// - Exactly one of `debit_account_id` or non-empty `debit_tag_ids` must be specified.
    /// - Exactly one of `credit_account_id` or non-empty `credit_tag_ids` must be specified.
    /// - No duplicate tag IDs within `debit_tag_ids` or within `credit_tag_ids` (`UNIQUE_ACCOUNT_RULE_TAGS`).
    pub fn validate(&self) -> Result<(), FineractError> {
        let has_debit_account = self.debit_account_id.is_some();
        let has_debit_tags = !self.debit_tag_ids.is_empty();
        if (has_debit_account && has_debit_tags) || (!has_debit_account && !has_debit_tags) {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.accounting.rule.accountToDebit.or.debitTags.required",
                    "accountToDebit or debitTags required",
                    vec![
                        ApiErrorArg::from("accountToDebit"),
                        ApiErrorArg::from("debitTags"),
                    ],
                ),
            ));
        }

        let has_credit_account = self.credit_account_id.is_some();
        let has_credit_tags = !self.credit_tag_ids.is_empty();
        if (has_credit_account && has_credit_tags) || (!has_credit_account && !has_credit_tags) {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.accounting.rule.accountToCredit.or.creditTags.required",
                    "accountToCredit or creditTags required",
                    vec![
                        ApiErrorArg::from("accountToCredit"),
                        ApiErrorArg::from("creditTags"),
                    ],
                ),
            ));
        }

        let mut seen_debit = HashSet::new();
        for &tag_id in &self.debit_tag_ids {
            if !seen_debit.insert(tag_id) {
                return Err(FineractError::domain_rule(
                    "error.msg.accounting.rule.tag.duplicate",
                    "The accounting rule already have the tags which you defined",
                ));
            }
        }

        let mut seen_credit = HashSet::new();
        for &tag_id in &self.credit_tag_ids {
            if !seen_credit.insert(tag_id) {
                return Err(FineractError::domain_rule(
                    "error.msg.accounting.rule.tag.duplicate",
                    "The accounting rule already have the tags which you defined",
                ));
            }
        }

        Ok(())
    }

    /// Validates a `CREATE ACCOUNTINGRULE` [`JsonCommand`] matching
    /// `AccountingRuleCommandFromApiJsonDeserializer.validateForCreate`.
    pub fn validate_for_create(command: &JsonCommand) -> Result<(), FineractError> {
        command.check_for_unsupported_parameters(Self::SUPPORTED_PARAMS)?;

        let account_to_debit = command.long_value_of_parameter_named("accountToDebit")?;
        let account_to_credit = command.long_value_of_parameter_named("accountToCredit")?;
        let office_id = command.long_value_of_parameter_named("officeId")?;
        let name = command.string_value_of_parameter_named("name");
        let description = command.string_value_of_parameter_named("description");
        let credit_tags = command.array_value_of_parameter_named("creditTags");
        let debit_tags = command.array_value_of_parameter_named("debitTags");

        let validator = DataValidatorBuilder::new().resource("AccountingRule");
        validator
            .reset()
            .parameter("accountToDebit")
            .value(account_to_debit)
            .ignore_if_null()
            .integer_greater_than_zero();
        validator
            .reset()
            .parameter("accountToCredit")
            .value(account_to_credit)
            .ignore_if_null()
            .integer_greater_than_zero();
        validator
            .reset()
            .parameter("officeId")
            .value(office_id)
            .not_null()
            .integer_greater_than_zero();
        validator
            .reset()
            .parameter("name")
            .value(name)
            .not_blank()
            .not_exceeding_length_of(100);
        validator
            .reset()
            .parameter("description")
            .value(description)
            .ignore_if_null()
            .not_exceeding_length_of(500);

        validator
            .reset()
            .parameter("creditTags")
            .value(credit_tags.clone())
            .ignore_if_null()
            .array_not_empty();
        if let Some(ref tags) = credit_tags {
            for tag in tags {
                validator
                    .reset()
                    .parameter("creditTags")
                    .value(tag.as_str())
                    .ignore_if_null()
                    .not_blank()
                    .long_greater_than_zero();
            }
        }

        validator
            .reset()
            .parameter("debitTags")
            .value(debit_tags.clone())
            .ignore_if_null()
            .array_not_empty();
        if let Some(ref tags) = debit_tags {
            for tag in tags {
                validator
                    .reset()
                    .parameter("debitTags")
                    .value(tag.as_str())
                    .ignore_if_null()
                    .not_blank()
                    .long_greater_than_zero();
            }
        }

        let mut errors = validator.data_validation_errors();

        if credit_tags.is_none() && account_to_credit.is_none() {
            errors.push(ApiParameterError::parameter_error(
                "validation.msg.accountToCredit.or.creditTags.required",
                "The parameter accountToCredit orcreditTags required",
                "accountToCredit,creditTags",
                vec![
                    ApiErrorArg::from("accountToCredit"),
                    ApiErrorArg::from("creditTags"),
                ],
            ));
        }

        if debit_tags.is_none() && account_to_debit.is_none() {
            errors.push(ApiParameterError::parameter_error(
                "validation.msg.accountToDebit.or.debitTags.required",
                "The parameter accountToDebit ordebitTags required",
                "accountToDebit,debitTags",
                vec![
                    ApiErrorArg::from("accountToDebit"),
                    ApiErrorArg::from("debitTags"),
                ],
            ));
        }

        if !errors.is_empty() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(errors),
            ));
        }

        Ok(())
    }

    /// Constructs an [`AccountingRule`] from a [`JsonCommand`], matching
    /// `AccountingRuleWritePlatformServiceJpaRepositoryImpl.assembleAccountingRuleAndTags`.
    pub fn from_json(id: i64, command: &JsonCommand) -> Result<Self, FineractError> {
        Self::validate_for_create(command)?;

        let office_id = command.long_value_of_parameter_named("officeId")?;
        let debit_account_id = command.long_value_of_parameter_named("accountToDebit")?;
        let credit_account_id = command.long_value_of_parameter_named("accountToCredit")?;
        let raw_debit_tags = command.array_value_of_parameter_named("debitTags");
        let raw_credit_tags = command.array_value_of_parameter_named("creditTags");

        if (debit_account_id.is_some() && raw_debit_tags.is_some())
            || (debit_account_id.is_none() && raw_debit_tags.is_none())
        {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.accounting.rule.accountToDebit.or.debitTags.required",
                    "accountToDebit or debitTags required",
                    vec![
                        ApiErrorArg::from("accountToDebit"),
                        ApiErrorArg::from("debitTags"),
                    ],
                ),
            ));
        }

        if (credit_account_id.is_some() && raw_credit_tags.is_some())
            || (credit_account_id.is_none() && raw_credit_tags.is_none())
        {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.accounting.rule.accountToCredit.or.creditTags.required",
                    "accountToCredit or creditTags required",
                    vec![
                        ApiErrorArg::from("accountToCredit"),
                        ApiErrorArg::from("creditTags"),
                    ],
                ),
            ));
        }

        let mut debit_tag_ids = Vec::new();
        if let Some(tags) = raw_debit_tags {
            let mut seen = HashSet::new();
            for t in tags {
                let parsed = t.trim().parse::<i64>().map_err(|_| {
                    FineractError::domain_rule(
                        "error.msg.accounting.rule.debitTags.invalid",
                        format!("Invalid debit tag id: {t}"),
                    )
                })?;
                if seen.insert(parsed) {
                    debit_tag_ids.push(parsed);
                }
            }
        }

        let mut credit_tag_ids = Vec::new();
        if let Some(tags) = raw_credit_tags {
            let mut seen = HashSet::new();
            for t in tags {
                let parsed = t.trim().parse::<i64>().map_err(|_| {
                    FineractError::domain_rule(
                        "error.msg.accounting.rule.creditTags.invalid",
                        format!("Invalid credit tag id: {t}"),
                    )
                })?;
                if seen.insert(parsed) {
                    credit_tag_ids.push(parsed);
                }
            }
        }

        let allow_multiple_debits = if !debit_tag_ids.is_empty() {
            command.boolean_primitive_value_of_parameter_named("allowMultipleDebitEntries")
        } else {
            false
        };
        let allow_multiple_credits = if !credit_tag_ids.is_empty() {
            command.boolean_primitive_value_of_parameter_named("allowMultipleCreditEntries")
        } else {
            false
        };

        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let description = command.string_value_of_parameter_named("description");

        let rule = Self::new(
            id,
            name,
            office_id,
            debit_account_id,
            allow_multiple_debits,
            debit_tag_ids,
            credit_account_id,
            allow_multiple_credits,
            credit_tag_ids,
            false,
            description,
        );
        rule.validate()?;
        Ok(rule)
    }
}

// =============================================================================
// 8. DualDbGlMetadataRepository
// =============================================================================

/// Dual-Database (`PostgreSQL` + `MySQL/MariaDB`) and concurrent in-memory repository for
/// Fineract General Ledger metadata:
/// - `acc_gl_account` ([`GLAccount`])
/// - `acc_gl_closure` ([`GLClosure`])
/// - `acc_gl_financial_activity_account` ([`FinancialActivityAccount`])
/// - `acc_accounting_rule` & `acc_rule_tags` ([`AccountingRule`])
///
/// Uses [`DialectSql::rewrite_placeholders`] when backed by a live [`sqlx::AnyPool`], and
/// maintains lock-free [`DashMap`] stores for fast unit/integration testing without an external DB.
#[derive(Debug, Clone)]
pub struct DualDbGlMetadataRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    gl_accounts: Arc<DashMap<i64, GLAccount>>,
    gl_account_ids_by_code: Arc<DashMap<String, i64>>,
    gl_closures: Arc<DashMap<i64, GLClosure>>,
    financial_activity_accounts: Arc<DashMap<i64, FinancialActivityAccount>>,
    financial_activity_ids_by_type: Arc<DashMap<FinancialActivity, i64>>,
    accounting_rules: Arc<DashMap<i64, AccountingRule>>,
    accounting_rule_ids_by_name: Arc<DashMap<String, i64>>,
    next_id: Arc<AtomicI64>,
}

impl Default for DualDbGlMetadataRepository {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl DualDbGlMetadataRepository {
    pub const GL_ACCOUNT_COLUMNS: &'static str = "id, name, parent_id, hierarchy, gl_code, \
        disabled, manual_journal_entries_allowed, account_usage, classification_enum, \
        tag_id, description";

    pub const GL_CLOSURE_COLUMNS: &'static str =
        "id, office_id, closing_date, is_deleted, comments";

    pub const FINANCIAL_ACTIVITY_ACCOUNT_COLUMNS: &'static str =
        "id, gl_account_id, financial_activity_type";

    pub const ACCOUNTING_RULE_COLUMNS: &'static str = "id, name, office_id, debit_account_id, \
        allow_multiple_debits, credit_account_id, allow_multiple_credits, description, \
        system_defined";

    /// Creates a new [`DualDbGlMetadataRepository`] backed by a SQLx [`AnyPool`] and [`SqlDialect`].
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            ..Self::with_dialect(dialect)
        }
    }

    /// Creates an in-memory [`DualDbGlMetadataRepository`] defaulting to [`SqlDialect::Postgres`].
    pub fn in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    /// Alias for [`DualDbGlMetadataRepository::in_memory`].
    pub fn new_in_memory() -> Self {
        Self::in_memory()
    }

    /// Creates an in-memory [`DualDbGlMetadataRepository`] configured with `dialect` for SQL generation.
    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            pool: None,
            dialect,
            gl_accounts: Arc::new(DashMap::new()),
            gl_account_ids_by_code: Arc::new(DashMap::new()),
            gl_closures: Arc::new(DashMap::new()),
            financial_activity_accounts: Arc::new(DashMap::new()),
            financial_activity_ids_by_type: Arc::new(DashMap::new()),
            accounting_rules: Arc::new(DashMap::new()),
            accounting_rule_ids_by_name: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(1)),
        }
    }

    /// Returns the configured [`SqlDialect`].
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    /// Returns a reference to the optional SQLx [`AnyPool`].
    pub const fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    fn allocate_id(&self, candidate: i64) -> i64 {
        if candidate > 0 {
            let _ = self.next_id.fetch_max(candidate + 1, Ordering::SeqCst);
            candidate
        } else {
            self.next_id.fetch_add(1, Ordering::SeqCst)
        }
    }

    // -------------------------------------------------------------------------
    // Dialect-Specific SQL Generators
    // -------------------------------------------------------------------------

    pub fn select_gl_account_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM acc_gl_account WHERE id = ?",
            Self::GL_ACCOUNT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_gl_account_by_code_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM acc_gl_account WHERE gl_code = ?",
            Self::GL_ACCOUNT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_gl_accounts_by_tag_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM acc_gl_account WHERE tag_id = ? ORDER BY id ASC",
            Self::GL_ACCOUNT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn insert_gl_account_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO acc_gl_account (\
            id, name, parent_id, hierarchy, gl_code, disabled, \
            manual_journal_entries_allowed, account_usage, classification_enum, \
            tag_id, description\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn update_gl_account_hierarchy_sql(dialect: SqlDialect) -> String {
        let raw = "UPDATE acc_gl_account SET hierarchy = ? WHERE id = ?";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn select_gl_closure_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM acc_gl_closure WHERE id = ?",
            Self::GL_CLOSURE_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_latest_gl_closure_by_office_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM acc_gl_closure WHERE office_id = ? AND is_deleted = false ORDER BY closing_date DESC LIMIT 1",
            Self::GL_CLOSURE_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn insert_gl_closure_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO acc_gl_closure (\
            id, office_id, closing_date, is_deleted, comments\
        ) VALUES (?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn select_financial_activity_account_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM acc_gl_financial_activity_account WHERE id = ?",
            Self::FINANCIAL_ACTIVITY_ACCOUNT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_financial_activity_account_by_type_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM acc_gl_financial_activity_account WHERE financial_activity_type = ?",
            Self::FINANCIAL_ACTIVITY_ACCOUNT_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn insert_financial_activity_account_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO acc_gl_financial_activity_account (\
            id, gl_account_id, financial_activity_type\
        ) VALUES (?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn select_accounting_rule_by_id_sql(dialect: SqlDialect) -> String {
        let raw = format!(
            "SELECT {} FROM acc_accounting_rule WHERE id = ?",
            Self::ACCOUNTING_RULE_COLUMNS
        );
        DialectSql::rewrite_placeholders(&raw, dialect)
    }

    pub fn select_accounting_rule_tags_sql(dialect: SqlDialect) -> String {
        let raw = "SELECT tag_id, acc_type_enum FROM acc_rule_tags WHERE acc_rule_id = ? ORDER BY id ASC";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn insert_accounting_rule_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO acc_accounting_rule (\
            id, name, office_id, debit_account_id, allow_multiple_debits, \
            credit_account_id, allow_multiple_credits, description, system_defined\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    pub fn insert_accounting_rule_tag_sql(dialect: SqlDialect) -> String {
        let raw = "INSERT INTO acc_rule_tags (id, acc_rule_id, tag_id, acc_type_enum) VALUES (?, ?, ?, ?)";
        DialectSql::rewrite_placeholders(raw, dialect)
    }

    // -------------------------------------------------------------------------
    // Row Mappers
    // -------------------------------------------------------------------------

    fn map_gl_account_row(row: &sqlx::any::AnyRow) -> Result<GLAccount, FineractError> {
        let usage_raw: i16 = row
            .try_get("account_usage")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let type_raw: i16 = row
            .try_get("classification_enum")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let tag_id_opt: Option<i64> = row
            .try_get::<Option<i64>, _>("tag_id")
            .or_else(|_| {
                row.try_get::<Option<i32>, _>("tag_id")
                    .map(|opt| opt.map(i64::from))
            })
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

        Ok(GLAccount {
            id: row
                .try_get("id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            name: row
                .try_get("name")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            parent_id: row
                .try_get("parent_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            hierarchy: row
                .try_get("hierarchy")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            gl_code: row
                .try_get("gl_code")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            disabled: row
                .try_get("disabled")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            manual_entries_allowed: row
                .try_get("manual_journal_entries_allowed")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            account_usage: GLAccountUsage::try_from(usage_raw)?,
            account_type: GLAccountType::try_from(type_raw)?,
            tag_id: tag_id_opt,
            description: row
                .try_get("description")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
        })
    }

    fn map_gl_closure_row(row: &sqlx::any::AnyRow) -> Result<GLClosure, FineractError> {
        Ok(GLClosure {
            id: row
                .try_get("id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            office_id: row
                .try_get("office_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            closing_date: {
                let s: String = row
                    .try_get("closing_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let part = s.split(|c: char| c == ' ' || c == 'T').next().unwrap_or(&s);
                NaiveDate::parse_from_str(part, "%Y-%m-%d")
                    .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
            },
            is_deleted: row
                .try_get("is_deleted")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            comments: row
                .try_get("comments")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
        })
    }

    fn map_financial_activity_account_row(
        row: &sqlx::any::AnyRow,
    ) -> Result<FinancialActivityAccount, FineractError> {
        let activity_raw: i16 = row
            .try_get("financial_activity_type")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        Ok(FinancialActivityAccount {
            id: row
                .try_get("id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            gl_account_id: row
                .try_get("gl_account_id")
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            financial_activity_type: FinancialActivity::try_from(activity_raw)?,
        })
    }

    // -------------------------------------------------------------------------
    // GLAccount Persistence
    // -------------------------------------------------------------------------

    /// Inserts or updates a [`GLAccount`] in the in-memory store.
    pub fn upsert_gl_account_in_memory(&self, mut account: GLAccount) -> GLAccount {
        account.id = self.allocate_id(account.id);
        if account.hierarchy.is_none() {
            let parent = account
                .parent_id
                .and_then(|pid| self.gl_accounts.get(&pid).map(|r| r.value().clone()));
            let _ = account.generate_hierarchy(parent.as_ref());
        }
        self.gl_account_ids_by_code
            .insert(account.gl_code.clone(), account.id);
        self.gl_accounts.insert(account.id, account.clone());
        account
    }

    /// Retrieves a [`GLAccount`] by ID from the in-memory store.
    pub fn get_gl_account_in_memory(&self, id: i64) -> Option<GLAccount> {
        self.gl_accounts.get(&id).map(|r| r.value().clone())
    }

    /// Creates and persists a new [`GLAccount`], enforcing parent `Header` usage, unique `gl_code`,
    /// and hierarchical path generation (`"."` or `".<parent_id>.<id>."`).
    pub async fn create_gl_account(&self, mut account: GLAccount) -> Result<GLAccount, FineractError> {
        let parent = if let Some(parent_id) = account.parent_id {
            let p = self.get_gl_account_by_id(parent_id).await?;
            p.validate_can_have_child(if account.id > 0 {
                Some(account.id)
            } else {
                None
            })?;
            Some(p)
        } else {
            None
        };

        account.id = self.allocate_id(account.id);
        account.generate_hierarchy(parent.as_ref())?;

        if let Some(ref pool) = self.pool {
            let insert_sql = Self::insert_gl_account_sql(self.dialect);
            let res = sqlx::query(&insert_sql)
                .bind(account.id)
                .bind(&account.name)
                .bind(account.parent_id)
                .bind(&account.hierarchy)
                .bind(&account.gl_code)
                .bind(account.disabled)
                .bind(account.manual_entries_allowed)
                .bind(account.account_usage.as_i16())
                .bind(account.account_type.as_i16())
                .bind(account.tag_id)
                .bind(&account.description)
                .execute(pool)
                .await;

            if let Err(e) = res {
                let msg = e.to_string();
                if msg.contains("acc_gl_code")
                    || msg.to_ascii_uppercase().contains("UNIQUE")
                    || msg.to_ascii_uppercase().contains("DUPLICATE")
                {
                    return Err(FineractError::DomainRule(
                        PlatformDomainRuleException::with_args(
                            "error.msg.glaccount.glcode.duplicate",
                            format!(
                                "General Ledger Account with GL code {} is already present",
                                account.gl_code
                            ),
                            vec![ApiErrorArg::from(account.gl_code.as_str())],
                        ),
                    ));
                }
                return Err(FineractError::DatabaseError(msg));
            }
        } else if self.gl_account_ids_by_code.contains_key(&account.gl_code) {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.glaccount.glcode.duplicate",
                    format!(
                        "General Ledger Account with GL code {} is already present",
                        account.gl_code
                    ),
                    vec![ApiErrorArg::from(account.gl_code.as_str())],
                ),
            ));
        }

        self.gl_account_ids_by_code
            .insert(account.gl_code.clone(), account.id);
        self.gl_accounts.insert(account.id, account.clone());
        Ok(account)
    }

    /// Alias for [`DualDbGlMetadataRepository::create_gl_account`].
    pub async fn save_gl_account(&self, account: GLAccount) -> Result<GLAccount, FineractError> {
        self.create_gl_account(account).await
    }

    /// Looks up a [`GLAccount`] by primary key `id`.
    pub async fn find_gl_account_by_id(&self, id: i64) -> Result<Option<GLAccount>, FineractError> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_gl_account_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(ref row) = maybe_row {
                let acc = Self::map_gl_account_row(row)?;
                self.gl_accounts.insert(acc.id, acc.clone());
                return Ok(Some(acc));
            }
            return Ok(None);
        }
        Ok(self.gl_accounts.get(&id).map(|r| r.value().clone()))
    }

    /// Retrieves a [`GLAccount`] by `id` or returns `GLAccountNotFoundException` (`error.msg.glaccount.id.invalid`).
    pub async fn get_gl_account_by_id(&self, id: i64) -> Result<GLAccount, FineractError> {
        self.find_gl_account_by_id(id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.glaccount.id.invalid",
                format!("General Ledger account with identifier {id} does not exist "),
                id.to_string(),
            )
        })
    }

    /// Looks up a [`GLAccount`] by `gl_code`.
    pub async fn find_gl_account_by_code(
        &self,
        gl_code: &str,
    ) -> Result<Option<GLAccount>, FineractError> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_gl_account_by_code_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(gl_code)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(ref row) = maybe_row {
                let acc = Self::map_gl_account_row(row)?;
                self.gl_accounts.insert(acc.id, acc.clone());
                return Ok(Some(acc));
            }
            return Ok(None);
        }
        if let Some(id_ref) = self.gl_account_ids_by_code.get(gl_code) {
            let id = *id_ref.value();
            return Ok(self.gl_accounts.get(&id).map(|r| r.value().clone()));
        }
        Ok(None)
    }

    /// Finds all [`GLAccount`] records tagged with `tag_id`.
    pub async fn find_gl_accounts_by_tag_id(
        &self,
        tag_id: i64,
    ) -> Result<Vec<GLAccount>, FineractError> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_gl_accounts_by_tag_id_sql(self.dialect);
            let rows = sqlx::query(&sql)
                .bind(tag_id)
                .fetch_all(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            let mut out = Vec::with_capacity(rows.len());
            for row in &rows {
                out.push(Self::map_gl_account_row(row)?);
            }
            return Ok(out);
        }
        let mut out: Vec<GLAccount> = self
            .gl_accounts
            .iter()
            .filter(|entry| entry.value().tag_id == Some(tag_id))
            .map(|entry| entry.value().clone())
            .collect();
        out.sort_by_key(|a| a.id);
        Ok(out)
    }

    /// Lists all [`GLAccount`] records ordered by `id`.
    pub async fn list_gl_accounts(&self) -> Result<Vec<GLAccount>, FineractError> {
        let mut out: Vec<GLAccount> = self
            .gl_accounts
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        out.sort_by_key(|a| a.id);
        Ok(out)
    }

    // -------------------------------------------------------------------------
    // GLClosure Persistence
    // -------------------------------------------------------------------------

    /// Inserts or updates a [`GLClosure`] in the in-memory store.
    pub fn upsert_gl_closure_in_memory(&self, mut closure: GLClosure) -> GLClosure {
        closure.id = self.allocate_id(closure.id);
        self.gl_closures.insert(closure.id, closure.clone());
        closure
    }

    /// Returns the latest non-deleted [`GLClosure`] for `office_id` from the in-memory store.
    pub fn get_latest_closure_in_memory(&self, office_id: i64) -> Option<GLClosure> {
        self.gl_closures
            .iter()
            .filter(|entry| {
                let c = entry.value();
                c.office_id == office_id && !c.is_deleted
            })
            .map(|entry| entry.value().clone())
            .max_by_key(|c| c.closing_date)
    }

    /// Creates and persists a new [`GLClosure`], enforcing unique `(office_id, closing_date)`
    /// (`GLClosureDuplicateException`).
    pub async fn create_gl_closure(&self, mut closure: GLClosure) -> Result<GLClosure, FineractError> {
        closure.id = self.allocate_id(closure.id);

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_gl_closure_sql(self.dialect);
            let res = sqlx::query(&sql)
                .bind(closure.id)
                .bind(closure.office_id)
                .bind(closure.closing_date.to_string())
                .bind(closure.is_deleted)
                .bind(&closure.comments)
                .execute(pool)
                .await;

            if let Err(e) = res {
                let msg = e.to_string();
                if msg.contains("office_id_closing_date")
                    || msg.to_ascii_uppercase().contains("UNIQUE")
                    || msg.to_ascii_uppercase().contains("DUPLICATE")
                {
                    return Err(FineractError::DomainRule(
                        PlatformDomainRuleException::with_args(
                            "error.msg.glclosure.glcode.duplicate",
                            format!(
                                "An accounting closure for branch with Id {} already exists for the date {}",
                                closure.office_id, closure.closing_date
                            ),
                            vec![
                                ApiErrorArg::from(closure.office_id),
                                ApiErrorArg::from_date(closure.closing_date),
                            ],
                        ),
                    ));
                }
                return Err(FineractError::DatabaseError(msg));
            }
        } else {
            let duplicate_exists = self.gl_closures.iter().any(|entry| {
                let c = entry.value();
                !c.is_deleted
                    && c.office_id == closure.office_id
                    && c.closing_date == closure.closing_date
            });
            if duplicate_exists {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.glclosure.glcode.duplicate",
                        format!(
                            "An accounting closure for branch with Id {} already exists for the date {}",
                            closure.office_id, closure.closing_date
                        ),
                        vec![
                            ApiErrorArg::from(closure.office_id),
                            ApiErrorArg::from_date(closure.closing_date),
                        ],
                    ),
                ));
            }
        }

        self.gl_closures.insert(closure.id, closure.clone());
        Ok(closure)
    }

    /// Alias for [`DualDbGlMetadataRepository::create_gl_closure`].
    pub async fn save_gl_closure(&self, closure: GLClosure) -> Result<GLClosure, FineractError> {
        self.create_gl_closure(closure).await
    }

    /// Looks up a [`GLClosure`] by primary key `id`.
    pub async fn find_gl_closure_by_id(&self, id: i64) -> Result<Option<GLClosure>, FineractError> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_gl_closure_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(ref row) = maybe_row {
                let closure = Self::map_gl_closure_row(row)?;
                self.gl_closures.insert(closure.id, closure.clone());
                return Ok(Some(closure));
            }
            return Ok(None);
        }
        Ok(self.gl_closures.get(&id).map(|r| r.value().clone()))
    }

    /// Retrieves the latest active (`is_deleted == false`) [`GLClosure`] for `office_id`,
    /// matching `GLClosureRepository.getLatestGLClosureByBranch(officeId)`.
    pub async fn get_latest_gl_closure_by_office(
        &self,
        office_id: i64,
    ) -> Result<Option<GLClosure>, FineractError> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_latest_gl_closure_by_office_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(office_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(ref row) = maybe_row {
                let closure = Self::map_gl_closure_row(row)?;
                self.gl_closures.insert(closure.id, closure.clone());
                return Ok(Some(closure));
            }
            return Ok(None);
        }
        Ok(self.get_latest_closure_in_memory(office_id))
    }

    /// Alias for [`DualDbGlMetadataRepository::get_latest_gl_closure_by_office`].
    pub async fn find_latest_closure_by_branch(
        &self,
        office_id: i64,
    ) -> Result<Option<GLClosure>, FineractError> {
        self.get_latest_gl_closure_by_office(office_id).await
    }

    // -------------------------------------------------------------------------
    // FinancialActivityAccount Persistence
    // -------------------------------------------------------------------------

    /// Inserts or updates a [`FinancialActivityAccount`] in the in-memory store.
    pub fn upsert_financial_activity_account_in_memory(
        &self,
        mut mapping: FinancialActivityAccount,
    ) -> FinancialActivityAccount {
        mapping.id = self.allocate_id(mapping.id);
        self.financial_activity_ids_by_type
            .insert(mapping.financial_activity_type, mapping.id);
        self.financial_activity_accounts
            .insert(mapping.id, mapping.clone());
        mapping
    }

    /// Creates and persists a new [`FinancialActivityAccount`], validating that the bound
    /// [`GLAccount`]'s `account_type` matches `mapping.financial_activity_type.mapped_gl_account_type()`
    /// and enforcing uniqueness per `financial_activity_type` (`DuplicateFinancialActivityAccountFoundException`).
    pub async fn create_financial_activity_account(
        &self,
        mut mapping: FinancialActivityAccount,
    ) -> Result<FinancialActivityAccount, FineractError> {
        let gl_account = self.get_gl_account_by_id(mapping.gl_account_id).await?;
        mapping.validate_mapping(&gl_account)?;
        mapping.id = self.allocate_id(mapping.id);

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_financial_activity_account_sql(self.dialect);
            let res = sqlx::query(&sql)
                .bind(mapping.id)
                .bind(mapping.gl_account_id)
                .bind(mapping.financial_activity_type.as_i16())
                .execute(pool)
                .await;

            if let Err(e) = res {
                let msg = e.to_string();
                if msg.contains("financial_activity_type")
                    || msg.to_ascii_uppercase().contains("UNIQUE")
                    || msg.to_ascii_uppercase().contains("DUPLICATE")
                {
                    let act_val = mapping.financial_activity_type.value();
                    return Err(FineractError::DomainRule(
                        PlatformDomainRuleException::with_args(
                            "error.msg.financialActivityAccount.exists",
                            format!("Mapping for activity already exists {act_val}"),
                            vec![ApiErrorArg::from(act_val)],
                        ),
                    ));
                }
                return Err(FineractError::DatabaseError(msg));
            }
        } else if self
            .financial_activity_ids_by_type
            .contains_key(&mapping.financial_activity_type)
        {
            let act_val = mapping.financial_activity_type.value();
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.financialActivityAccount.exists",
                    format!("Mapping for activity already exists {act_val}"),
                    vec![ApiErrorArg::from(act_val)],
                ),
            ));
        }

        self.financial_activity_ids_by_type
            .insert(mapping.financial_activity_type, mapping.id);
        self.financial_activity_accounts
            .insert(mapping.id, mapping.clone());
        Ok(mapping)
    }

    /// Alias for [`DualDbGlMetadataRepository::create_financial_activity_account`].
    pub async fn save_financial_activity_account(
        &self,
        mapping: FinancialActivityAccount,
    ) -> Result<FinancialActivityAccount, FineractError> {
        self.create_financial_activity_account(mapping).await
    }

    /// Looks up a [`FinancialActivityAccount`] by primary key `id`.
    pub async fn find_financial_activity_account_by_id(
        &self,
        id: i64,
    ) -> Result<Option<FinancialActivityAccount>, FineractError> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_financial_activity_account_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(ref row) = maybe_row {
                let item = Self::map_financial_activity_account_row(row)?;
                self.financial_activity_accounts
                    .insert(item.id, item.clone());
                return Ok(Some(item));
            }
            return Ok(None);
        }
        Ok(self
            .financial_activity_accounts
            .get(&id)
            .map(|r| r.value().clone()))
    }

    /// Looks up a [`FinancialActivityAccount`] by [`FinancialActivity`] type.
    pub async fn find_financial_activity_account_by_type(
        &self,
        activity: FinancialActivity,
    ) -> Result<Option<FinancialActivityAccount>, FineractError> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_financial_activity_account_by_type_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(activity.as_i16())
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(ref row) = maybe_row {
                let item = Self::map_financial_activity_account_row(row)?;
                self.financial_activity_ids_by_type
                    .insert(activity, item.id);
                self.financial_activity_accounts
                    .insert(item.id, item.clone());
                return Ok(Some(item));
            }
            return Ok(None);
        }
        if let Some(id_ref) = self.financial_activity_ids_by_type.get(&activity) {
            let id = *id_ref.value();
            return Ok(self
                .financial_activity_accounts
                .get(&id)
                .map(|r| r.value().clone()));
        }
        Ok(None)
    }

    /// Retrieves a [`FinancialActivityAccount`] by [`FinancialActivity`] type or returns
    /// `FinancialActivityAccountNotFoundException` (`error.msg.financialActivityAccount.not.found`).
    pub async fn get_financial_activity_account_by_type(
        &self,
        activity: FinancialActivity,
    ) -> Result<FinancialActivityAccount, FineractError> {
        self.find_financial_activity_account_by_type(activity)
            .await?
            .ok_or_else(|| {
                let act_id = activity.value();
                FineractError::resource_not_found(
                    "error.msg.financialActivityAccount.not.found",
                    format!(
                        "Financial Activity account with for the financial Activity with Id {act_id} does not exist"
                    ),
                    act_id.to_string(),
                )
            })
    }

    // -------------------------------------------------------------------------
    // AccountingRule Persistence
    // -------------------------------------------------------------------------

    /// Inserts or updates an [`AccountingRule`] in the in-memory store.
    pub fn upsert_accounting_rule_in_memory(&self, mut rule: AccountingRule) -> AccountingRule {
        rule.id = self.allocate_id(rule.id);
        self.accounting_rule_ids_by_name
            .insert(rule.name.clone(), rule.id);
        self.accounting_rules.insert(rule.id, rule.clone());
        rule
    }

    /// Creates and persists a new [`AccountingRule`] (and its tag rules in `acc_rule_tags`),
    /// validating referenced GL accounts and enforcing unique rule names (`AccountingRuleDuplicateException`).
    pub async fn create_accounting_rule(
        &self,
        mut rule: AccountingRule,
    ) -> Result<AccountingRule, FineractError> {
        rule.validate()?;

        if let Some(debit_id) = rule.debit_account_id {
            let _ = self.get_gl_account_by_id(debit_id).await?;
        }
        if let Some(credit_id) = rule.credit_account_id {
            let _ = self.get_gl_account_by_id(credit_id).await?;
        }

        rule.id = self.allocate_id(rule.id);

        if let Some(ref pool) = self.pool {
            let insert_sql = Self::insert_accounting_rule_sql(self.dialect);
            let res = sqlx::query(&insert_sql)
                .bind(rule.id)
                .bind(&rule.name)
                .bind(rule.office_id)
                .bind(rule.debit_account_id)
                .bind(rule.allow_multiple_debits)
                .bind(rule.credit_account_id)
                .bind(rule.allow_multiple_credits)
                .bind(&rule.description)
                .bind(rule.system_defined)
                .execute(pool)
                .await;

            if let Err(e) = res {
                let msg = e.to_string();
                if msg.contains("accounting_rule_name_unique")
                    || msg.to_ascii_uppercase().contains("UNIQUE")
                    || msg.to_ascii_uppercase().contains("DUPLICATE")
                {
                    return Err(FineractError::domain_rule(
                        "error.msg.accounting.rule.duplicate",
                        format!(
                            "An accounting rule with the name {} already exists{}",
                            rule.name, rule.name
                        ),
                    ));
                }
                return Err(FineractError::DatabaseError(msg));
            }

            let tag_sql = Self::insert_accounting_rule_tag_sql(self.dialect);
            for &tag_id in &rule.debit_tag_ids {
                let tag_row_id = self.allocate_id(0);
                sqlx::query(&tag_sql)
                    .bind(tag_row_id)
                    .bind(rule.id)
                    .bind(tag_id)
                    .bind(AccountingRule::DEBIT_TAG_TYPE_ENUM)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }
            for &tag_id in &rule.credit_tag_ids {
                let tag_row_id = self.allocate_id(0);
                sqlx::query(&tag_sql)
                    .bind(tag_row_id)
                    .bind(rule.id)
                    .bind(tag_id)
                    .bind(AccountingRule::CREDIT_TAG_TYPE_ENUM)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }
        } else if self.accounting_rule_ids_by_name.contains_key(&rule.name) {
            return Err(FineractError::domain_rule(
                "error.msg.accounting.rule.duplicate",
                format!(
                    "An accounting rule with the name {} already exists{}",
                    rule.name, rule.name
                ),
            ));
        }

        self.accounting_rule_ids_by_name
            .insert(rule.name.clone(), rule.id);
        self.accounting_rules.insert(rule.id, rule.clone());
        Ok(rule)
    }

    /// Alias for [`DualDbGlMetadataRepository::create_accounting_rule`].
    pub async fn save_accounting_rule(
        &self,
        rule: AccountingRule,
    ) -> Result<AccountingRule, FineractError> {
        self.create_accounting_rule(rule).await
    }

    /// Looks up an [`AccountingRule`] by primary key `id`.
    pub async fn find_accounting_rule_by_id(
        &self,
        id: i64,
    ) -> Result<Option<AccountingRule>, FineractError> {
        if let Some(ref pool) = self.pool {
            let sql = Self::select_accounting_rule_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            let Some(ref row) = maybe_row else {
                return Ok(None);
            };

            let tag_sql = Self::select_accounting_rule_tags_sql(self.dialect);
            let tag_rows = sqlx::query(&tag_sql)
                .bind(id)
                .fetch_all(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

            let mut debit_tag_ids = Vec::new();
            let mut credit_tag_ids = Vec::new();
            for tag_row in &tag_rows {
                let tag_id: i64 = tag_row
                    .try_get::<i64, _>("tag_id")
                    .or_else(|_| tag_row.try_get::<i32, _>("tag_id").map(i64::from))
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let acc_type_enum: i16 = tag_row
                    .try_get("acc_type_enum")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                if acc_type_enum == AccountingRule::CREDIT_TAG_TYPE_ENUM {
                    credit_tag_ids.push(tag_id);
                } else if acc_type_enum == AccountingRule::DEBIT_TAG_TYPE_ENUM {
                    debit_tag_ids.push(tag_id);
                }
            }

            let rule = AccountingRule {
                id: row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                name: row
                    .try_get("name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                office_id: row
                    .try_get("office_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                debit_account_id: row
                    .try_get("debit_account_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                allow_multiple_debits: row
                    .try_get("allow_multiple_debits")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                debit_tag_ids,
                credit_account_id: row
                    .try_get("credit_account_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                allow_multiple_credits: row
                    .try_get("allow_multiple_credits")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                credit_tag_ids,
                description: row
                    .try_get("description")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
                system_defined: row
                    .try_get("system_defined")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?,
            };
            self.accounting_rules.insert(rule.id, rule.clone());
            return Ok(Some(rule));
        }

        Ok(self.accounting_rules.get(&id).map(|r| r.value().clone()))
    }

    /// Retrieves an [`AccountingRule`] by `id` or returns `AccountingRuleNotFoundException`
    /// (`error.msg.accounting.rule.id.invalid`).
    pub async fn get_accounting_rule_by_id(&self, id: i64) -> Result<AccountingRule, FineractError> {
        self.find_accounting_rule_by_id(id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.accounting.rule.id.invalid",
                format!("Accounting Rule with identifier {id} does not exist"),
                id.to_string(),
            )
        })
    }

    /// Looks up an [`AccountingRule`] by `name` in the in-memory store.
    pub async fn find_accounting_rule_by_name(
        &self,
        name: &str,
    ) -> Result<Option<AccountingRule>, FineractError> {
        if let Some(id_ref) = self.accounting_rule_ids_by_name.get(name) {
            let id = *id_ref.value();
            return self.find_accounting_rule_by_id(id).await;
        }
        Ok(None)
    }
}

// =============================================================================
// 9. CQRS Command Handlers
// =============================================================================

/// CQRS command handler for `CREATE GLACCOUNT` (`@CommandType(entity = "GLACCOUNT", action = "CREATE")`).
///
/// Ported from `org.apache.fineract.accounting.glaccount.handler.CreateGLAccountCommandHandler`
/// and `GLAccountWritePlatformServiceJpaRepositoryImpl.createGLAccount`.
#[derive(Debug, Clone, Default)]
pub struct CreateGLAccountCommandHandler {
    repo: Arc<DualDbGlMetadataRepository>,
}

impl CreateGLAccountCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "GLACCOUNT";

    pub fn new(repo: Arc<DualDbGlMetadataRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGlMetadataRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGlMetadataRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateGLAccountCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        GLAccount::validate_for_create(command)?;

        let parent_id = command.long_value_of_parameter_named("parentId")?;
        let parent_account = if let Some(pid) = parent_id {
            let p = self.repo.get_gl_account_by_id(pid).await?;
            p.validate_as_parent()?;
            Some(p)
        } else {
            None
        };

        let gl_account = GLAccount::from_json(0, parent_account.as_ref(), command)?;
        let saved = self.repo.create_gl_account(gl_account).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// CQRS command handler for `CREATE GLCLOSURE` (`@CommandType(entity = "GLCLOSURE", action = "CREATE")`).
///
/// Ported from `org.apache.fineract.accounting.closure.handler.CreateGLClosureCommandHandler`
/// and `GLClosureWritePlatformServiceJpaRepositoryImpl.createGLClosure`.
#[derive(Debug, Clone, Default)]
pub struct CreateGLClosureCommandHandler {
    repo: Arc<DualDbGlMetadataRepository>,
}

impl CreateGLClosureCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "GLCLOSURE";

    pub fn new(repo: Arc<DualDbGlMetadataRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGlMetadataRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGlMetadataRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateGLClosureCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let closure = GLClosure::from_json(0, command)?;

        // Ensure closure date is not in the future relative to tenant business date
        let business_date = ctx.get_business_date();
        if closure.closing_date > business_date {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.glclosure.invalid.future.date",
                    "Accounting closures cannot be made for a future date",
                    vec![ApiErrorArg::from_date(closure.closing_date)],
                ),
            ));
        }

        // Ensure closure date is not before an existing accounting closure for the branch
        if let Some(latest) = self
            .repo
            .get_latest_gl_closure_by_office(closure.office_id)
            .await?
        {
            if latest.closing_date > closure.closing_date {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.glclosure.invalid.accounting.closed",
                        "Accounting Closure for this branch has already been defined for a greater date",
                        vec![ApiErrorArg::from_date(latest.closing_date)],
                    ),
                ));
            }
        }

        let office_id = closure.office_id;
        let saved = self.repo.create_gl_closure(closure).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(office_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// CQRS command handler for `CREATE FINANCIALACTIVITYACCOUNT`
/// (`@CommandType(entity = "FINANCIALACTIVITYACCOUNT", action = "CREATE")`).
///
/// Ported from `org.apache.fineract.accounting.financialactivityaccount.handler.CreateFinancialActivityAccountHandler`
/// and `FinancialActivityAccountWritePlatformServiceImpl.createFinancialActivityAccountMapping`.
#[derive(Debug, Clone, Default)]
pub struct CreateFinancialActivityAccountCommandHandler {
    repo: Arc<DualDbGlMetadataRepository>,
}

/// Type alias matching the upstream Java class name `CreateFinancialActivityAccountHandler`.
pub type CreateFinancialActivityAccountHandler = CreateFinancialActivityAccountCommandHandler;

impl CreateFinancialActivityAccountCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "FINANCIALACTIVITYACCOUNT";

    pub fn new(repo: Arc<DualDbGlMetadataRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGlMetadataRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGlMetadataRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateFinancialActivityAccountCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        FinancialActivityAccount::validate_for_create(command)?;

        let financial_activity_id = command
            .integer_value_sans_locale_of_parameter_named("financialActivityId")?
            .unwrap_or(0);
        let gl_account_id = command
            .long_value_of_parameter_named("glAccountId")?
            .unwrap_or(0);

        let activity = FinancialActivity::try_from(financial_activity_id)?;
        let gl_account = self.repo.get_gl_account_by_id(gl_account_id).await?;
        let mapping = FinancialActivityAccount::create_new(0, &gl_account, activity)?;
        let saved = self.repo.create_financial_activity_account(mapping).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// CQRS command handler for `CREATE ACCOUNTINGRULE`
/// (`@CommandType(entity = "ACCOUNTINGRULE", action = "CREATE")`).
///
/// Ported from `org.apache.fineract.accounting.rule.handler.CreateAccountingRuleCommandHandler`
/// and `AccountingRuleWritePlatformServiceJpaRepositoryImpl.createAccountingRule`.
#[derive(Debug, Clone, Default)]
pub struct CreateAccountingRuleCommandHandler {
    repo: Arc<DualDbGlMetadataRepository>,
}

impl CreateAccountingRuleCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "ACCOUNTINGRULE";

    pub fn new(repo: Arc<DualDbGlMetadataRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGlMetadataRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGlMetadataRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateAccountingRuleCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let rule = AccountingRule::from_json(0, command)?;
        let office_id = rule.office_id;
        let saved = self.repo.create_accounting_rule(rule).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id_opt(office_id)
            .with_entity_id(saved.id)
            .build())
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx(year: i32, month: u32, day: u32) -> ExecutionContext {
        ExecutionContext::new("default", NaiveDate::from_ymd_opt(year, month, day).unwrap())
    }

    #[test]
    fn test_gl_account_type_discriminants_and_codes() {
        assert_eq!(GLAccountType::Asset as i32, 1);
        assert_eq!(GLAccountType::Liability as i32, 2);
        assert_eq!(GLAccountType::Equity as i32, 3);
        assert_eq!(GLAccountType::Income as i32, 4);
        assert_eq!(GLAccountType::Expense as i32, 5);

        assert_eq!(GLAccountType::from_int(1), Some(GLAccountType::Asset));
        assert_eq!(GLAccountType::from_int(2), Some(GLAccountType::Liability));
        assert_eq!(GLAccountType::from_int(3), Some(GLAccountType::Equity));
        assert_eq!(GLAccountType::from_int(4), Some(GLAccountType::Income));
        assert_eq!(GLAccountType::from_int(5), Some(GLAccountType::Expense));
        assert_eq!(GLAccountType::from_int(0), None);
        assert_eq!(GLAccountType::from_int(6), None);

        assert_eq!(GLAccountType::Asset.code(), "accountType.asset");
        assert_eq!(GLAccountType::Liability.code(), "accountType.liability");
        assert_eq!(GLAccountType::Equity.code(), "accountType.equity");
        assert_eq!(GLAccountType::Income.code(), "accountType.income");
        assert_eq!(GLAccountType::Expense.code(), "accountType.expense");

        assert!(GLAccountType::Asset.is_asset_type());
        assert!(GLAccountType::Liability.is_liability_type());
        assert!(GLAccountType::Equity.is_equity_type());
        assert!(GLAccountType::Income.is_income_type());
        assert!(GLAccountType::Expense.is_expense_type());
    }

    #[test]
    fn test_gl_account_usage_discriminants_and_codes() {
        assert_eq!(GLAccountUsage::Detail as i32, 1);
        assert_eq!(GLAccountUsage::Header as i32, 2);
        assert_eq!(GLAccountUsage::from_int(1), Some(GLAccountUsage::Detail));
        assert_eq!(GLAccountUsage::from_int(2), Some(GLAccountUsage::Header));
        assert_eq!(GLAccountUsage::from_int(3), None);
        assert_eq!(GLAccountUsage::Detail.code(), "accountUsage.detail");
        assert_eq!(GLAccountUsage::Header.code(), "accountUsage.header");
        assert!(GLAccountUsage::Detail.is_detail());
        assert!(GLAccountUsage::Header.is_header());
    }

    #[test]
    fn test_financial_activity_mappings() {
        assert_eq!(FinancialActivity::AssetTransfer as i32, 100);
        assert_eq!(FinancialActivity::LiabilityTransfer as i32, 200);
        assert_eq!(FinancialActivity::CashAtMainVault as i32, 101);
        assert_eq!(FinancialActivity::CashAtTeller as i32, 102);
        assert_eq!(FinancialActivity::OpeningBalancesTransferContra as i32, 300);
        assert_eq!(FinancialActivity::AssetFundSource as i32, 103);
        assert_eq!(FinancialActivity::PayableDividends as i32, 201);

        assert_eq!(
            FinancialActivity::AssetTransfer.mapped_gl_account_type(),
            GLAccountType::Asset
        );
        assert_eq!(
            FinancialActivity::CashAtMainVault.mapped_gl_account_type(),
            GLAccountType::Asset
        );
        assert_eq!(
            FinancialActivity::CashAtTeller.mapped_gl_account_type(),
            GLAccountType::Asset
        );
        assert_eq!(
            FinancialActivity::AssetFundSource.mapped_gl_account_type(),
            GLAccountType::Asset
        );
        assert_eq!(
            FinancialActivity::LiabilityTransfer.mapped_gl_account_type(),
            GLAccountType::Liability
        );
        assert_eq!(
            FinancialActivity::PayableDividends.mapped_gl_account_type(),
            GLAccountType::Liability
        );
        assert_eq!(
            FinancialActivity::OpeningBalancesTransferContra.mapped_gl_account_type(),
            GLAccountType::Equity
        );
    }

    #[test]
    fn test_gl_account_hierarchy_and_journal_posting_guards() {
        let mut header = GLAccount::new_header(1, None, "Cash and Bank", "1000", GLAccountType::Asset);
        header.generate_hierarchy(None).unwrap();
        assert_eq!(header.hierarchy.as_deref(), Some("."));
        assert!(header.validate_as_parent().is_ok());
        // Header accounts cannot receive journal postings
        assert!(header.validate_for_journal_posting(false).is_err());

        let mut detail = GLAccount::new_detail(
            2,
            Some(1),
            "Main Vault Cash",
            "1001",
            GLAccountType::Asset,
            true,
        );
        detail.generate_hierarchy(Some(&header)).unwrap();
        assert_eq!(detail.hierarchy.as_deref(), Some(".2."));
        // Detail accounts can receive journal postings
        assert!(detail.validate_for_journal_posting(true).is_ok());
        // Detail accounts cannot have children
        let err = detail.validate_as_parent().unwrap_err();
        assert_eq!(err.http_status_code(), 403);
        assert_eq!(
            err.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.glaccount.parent.invalid"
        );

        // Disabled detail account rejects posting
        let mut disabled_detail = detail.clone();
        disabled_detail.disabled = true;
        let disabled_err = disabled_detail.validate_for_journal_posting(false).unwrap_err();
        assert_eq!(
            disabled_err.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.glJournalEntry.invalid.account.disabled"
        );

        // Manual entries disallowed rejects manual entry but allows system entry
        let mut system_only_detail = detail;
        system_only_detail.manual_entries_allowed = false;
        assert!(system_only_detail.validate_for_journal_posting(false).is_ok());
        let manual_err = system_only_detail.validate_for_journal_posting(true).unwrap_err();
        assert_eq!(
            manual_err.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.glJournalEntry.invalid.account.manual.adjustments.not.permitted"
        );
    }

    #[test]
    fn test_gl_closure_transaction_date_validation() {
        let closing_date = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();
        let closure = GLClosure::new(10, 1, closing_date, Some("  Q2 close  ".to_string()));
        assert_eq!(closure.comments.as_deref(), Some("Q2 close"));
        assert!(!closure.is_deleted);

        // No closure -> any date allowed
        assert!(GLClosure::validate_transaction_date_after_closure(
            None,
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
        )
        .is_ok());

        // Strictly after closing_date -> allowed
        assert!(GLClosure::validate_transaction_date_after_closure(
            Some(&closure),
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        )
        .is_ok());

        // Equal to closing_date -> rejected
        let eq_err = GLClosure::validate_transaction_date_after_closure(Some(&closure), closing_date)
            .unwrap_err();
        assert_eq!(
            eq_err.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.glJournalEntry.invalid.accounting.closed"
        );

        // Prior to closing_date -> rejected
        let before_err = GLClosure::validate_transaction_date_after_closure(
            Some(&closure),
            NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            before_err.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.glJournalEntry.invalid.accounting.closed"
        );

        // Soft-deleted closure -> allowed
        let mut deleted_closure = closure;
        deleted_closure.is_deleted = true;
        assert!(validate_transaction_date_after_closure(
            Some(&deleted_closure),
            NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()
        )
        .is_ok());
    }

    #[test]
    fn test_financial_activity_account_type_validation() {
        let asset_account = GLAccount::new_detail(
            10,
            None,
            "Vault Asset",
            "1100",
            GLAccountType::Asset,
            true,
        );
        let equity_account = GLAccount::new_detail(
            20,
            None,
            "Opening Balances Equity",
            "3100",
            GLAccountType::Equity,
            true,
        );

        let valid = FinancialActivityAccount::create_new(
            1,
            &asset_account,
            FinancialActivity::CashAtMainVault,
        )
        .unwrap();
        assert_eq!(valid.gl_account_id, 10);
        assert_eq!(valid.financial_activity_type, FinancialActivity::CashAtMainVault);

        let valid_contra = FinancialActivityAccount::create_new(
            2,
            &equity_account,
            FinancialActivity::OpeningBalancesTransferContra,
        )
        .unwrap();
        assert_eq!(valid_contra.gl_account_id, 20);

        // Binding an Equity account to CashAtMainVault (expects Asset) fails
        let mismatch = FinancialActivityAccount::create_new(
            3,
            &equity_account,
            FinancialActivity::CashAtMainVault,
        )
        .unwrap_err();
        assert_eq!(
            mismatch.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.financialActivityAccount.invalid"
        );
    }

    #[test]
    fn test_dual_db_sql_rewriting_postgres_and_mysql() {
        let pg_insert = DualDbGlMetadataRepository::insert_gl_account_sql(SqlDialect::Postgres);
        assert!(pg_insert.contains("$1, $2, $3"));
        assert!(!pg_insert.contains('?'));

        let my_insert = DualDbGlMetadataRepository::insert_gl_account_sql(SqlDialect::MySql);
        assert!(my_insert.contains("VALUES (?, ?, ?"));
        assert!(!my_insert.contains("$1"));

        let pg_closure =
            DualDbGlMetadataRepository::select_latest_gl_closure_by_office_sql(SqlDialect::Postgres);
        assert!(pg_closure.contains("WHERE office_id = $1 AND is_deleted = false"));
    }

    #[tokio::test]
    async fn test_command_handlers_end_to_end_in_memory() {
        let repo = Arc::new(DualDbGlMetadataRepository::in_memory());
        let gl_handler = CreateGLAccountCommandHandler::new(Arc::clone(&repo));
        let closure_handler = CreateGLClosureCommandHandler::new(Arc::clone(&repo));
        let fa_handler = CreateFinancialActivityAccountCommandHandler::new(Arc::clone(&repo));
        let rule_handler = CreateAccountingRuleCommandHandler::new(Arc::clone(&repo));
        let ctx = test_ctx(2026, 10, 30);

        // 1. Create Header Asset Account
        let create_header_cmd = JsonCommand::from_json(
            r#"{
                "name": "Assets Header",
                "glCode": "1000",
                "manualEntriesAllowed": false,
                "type": 1,
                "usage": 2,
                "description": "Top-level asset header"
            }"#,
        )
        .unwrap()
        .with_command_id(101);
        let header_res = gl_handler.handle(&ctx, &create_header_cmd).await.unwrap();
        let header_id = header_res.resource_id.unwrap();
        assert_eq!(header_id, 1);

        // 2. Create Detail Asset Account under Header
        let create_detail_asset_cmd = JsonCommand::from_json(
            r#"{
                "name": "Teller Cash",
                "glCode": "1001",
                "parentId": 1,
                "manualEntriesAllowed": true,
                "type": 1,
                "usage": 1
            }"#,
        )
        .unwrap();
        let asset_res = gl_handler.handle(&ctx, &create_detail_asset_cmd).await.unwrap();
        let asset_id = asset_res.resource_id.unwrap();
        let stored_asset = repo.get_gl_account_by_id(asset_id).await.unwrap();
        assert_eq!(stored_asset.hierarchy.as_deref(), Some(".2."));

        // Attempting to create a child under a Detail account fails with GLAccountInvalidParentException
        let invalid_child_cmd = JsonCommand::from_json(
            r#"{
                "name": "Invalid Sub-Detail",
                "glCode": "1002",
                "parentId": 2,
                "manualEntriesAllowed": true,
                "type": 1,
                "usage": 1
            }"#,
        )
        .unwrap();
        let invalid_parent_err = gl_handler.handle(&ctx, &invalid_child_cmd).await.unwrap_err();
        assert_eq!(
            invalid_parent_err.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.glaccount.parent.invalid"
        );

        // 3. Create Detail Liability Account
        let create_liability_cmd = JsonCommand::from_json(
            r#"{
                "name": "Suspense Liability",
                "glCode": "2001",
                "manualEntriesAllowed": true,
                "type": 2,
                "usage": 1
            }"#,
        )
        .unwrap();
        let liability_id = gl_handler
            .handle(&ctx, &create_liability_cmd)
            .await
            .unwrap()
            .resource_id
            .unwrap();

        // 4. Create Financial Activity Account Mapping (CashAtTeller = 102 -> Asset account)
        let create_fa_cmd = JsonCommand::from_json(format!(
            r#"{{"financialActivityId": 102, "glAccountId": {asset_id}}}"#
        ))
        .unwrap();
        let fa_res = fa_handler.handle(&ctx, &create_fa_cmd).await.unwrap();
        assert!(fa_res.resource_id.is_some());

        // Mapping CashAtMainVault (101, Asset) to Liability account fails
        let invalid_fa_cmd = JsonCommand::from_json(format!(
            r#"{{"financialActivityId": 101, "glAccountId": {liability_id}}}"#
        ))
        .unwrap();
        assert!(fa_handler.handle(&ctx, &invalid_fa_cmd).await.is_err());

        // 5. Create GL Closure on 2026-09-30
        let create_closure_cmd = JsonCommand::from_json(
            r#"{
                "officeId": 1,
                "closingDate": "2026-09-30",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en",
                "comments": "September close"
            }"#,
        )
        .unwrap();
        let closure_res = closure_handler.handle(&ctx, &create_closure_cmd).await.unwrap();
        assert_eq!(closure_res.office_id, Some(1));

        // Future closure date (2026-11-01 > 2026-10-30) is rejected
        let future_closure_cmd = JsonCommand::from_json(
            r#"{
                "officeId": 1,
                "closingDate": "2026-11-01",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap();
        let future_err = closure_handler.handle(&ctx, &future_closure_cmd).await.unwrap_err();
        assert_eq!(
            future_err.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.glclosure.invalid.future.date"
        );

        // Closure date prior to existing 2026-09-30 is rejected
        let backdated_closure_cmd = JsonCommand::from_json(
            r#"{
                "officeId": 1,
                "closingDate": "2026-08-31",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap();
        let backdated_err = closure_handler
            .handle(&ctx, &backdated_closure_cmd)
            .await
            .unwrap_err();
        assert_eq!(
            backdated_err.to_global_error_response().errors[0].user_message_globalisation_code,
            "error.msg.glclosure.invalid.accounting.closed"
        );

        // 6. Create Accounting Rule (Fixed Debit Account + Credit Tags)
        let create_rule_cmd = JsonCommand::from_json(format!(
            r#"{{
                "name": "Teller Cash Receipt Rule",
                "officeId": 1,
                "accountToDebit": {asset_id},
                "creditTags": ["10", "11"],
                "allowMultipleCreditEntries": true,
                "description": "Debit teller cash, credit tagged accounts"
            }}"#
        ))
        .unwrap();
        let rule_res = rule_handler.handle(&ctx, &create_rule_cmd).await.unwrap();
        let rule_id = rule_res.resource_id.unwrap();
        let saved_rule = repo.get_accounting_rule_by_id(rule_id).await.unwrap();
        assert_eq!(saved_rule.debit_account_id, Some(asset_id));
        assert_eq!(saved_rule.credit_tag_ids, vec![10, 11]);
        assert!(saved_rule.allow_multiple_credits);
    }
}
