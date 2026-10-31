//! Multi-tenant execution context, business date types, and task-local state propagation.
//!
//! Ported from:
//! - `org.apache.fineract.infrastructure.businessdate.domain.BusinessDateType`
//! - `org.apache.fineract.infrastructure.core.domain.ActionContext`
//! - `org.apache.fineract.infrastructure.core.domain.FineractPlatformTenantConnection`
//! - `org.apache.fineract.infrastructure.core.domain.FineractPlatformTenant`
//! - `org.apache.fineract.infrastructure.core.domain.FineractContext`
//! - `org.apache.fineract.infrastructure.core.service.ThreadLocalContextUtil`

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Type of logical business date in Fineract.
///
/// Ported from `org.apache.fineract.infrastructure.businessdate.domain.BusinessDateType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum BusinessDateType {
    #[default]
    #[serde(rename = "BUSINESS_DATE", alias = "BusinessDate", alias = "business_date")]
    BusinessDate,
    #[serde(rename = "COB_DATE", alias = "CobDate", alias = "cob_date")]
    CobDate,
}

impl BusinessDateType {
    /// Java-compatible constant alias for `BusinessDateType::BusinessDate`.
    pub const BUSINESS_DATE: Self = Self::BusinessDate;
    /// Java-compatible constant alias for `BusinessDateType::CobDate`.
    pub const COB_DATE: Self = Self::CobDate;

    /// Numeric identifier matching `BusinessDateType.getId()` (`1` or `2`).
    pub const fn id(&self) -> i32 {
        match self {
            Self::BusinessDate => 1,
            Self::CobDate => 2,
        }
    }

    /// Human-readable description matching `BusinessDateType.getDescription()`.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::BusinessDate => "Business Date",
            Self::CobDate => "Close of Business Date",
        }
    }

    /// Canonical enum name matching `BusinessDateType.getName()`.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::BusinessDate => "BUSINESS_DATE",
            Self::CobDate => "COB_DATE",
        }
    }
}

impl fmt::Display for BusinessDateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for BusinessDateType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "BUSINESS_DATE" | "BusinessDate" | "business_date" => Ok(Self::BusinessDate),
            "COB_DATE" | "CobDate" | "cob_date" => Ok(Self::CobDate),
            other => Err(format!("Unknown BusinessDateType: `{other}`")),
        }
    }
}

/// Execution action context determining which [`BusinessDateType`] is active.
///
/// Ported from `org.apache.fineract.infrastructure.core.domain.ActionContext`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum ActionContext {
    #[default]
    #[serde(rename = "DEFAULT", alias = "Default", alias = "default")]
    Default,
    #[serde(rename = "COB", alias = "Cob", alias = "cob")]
    Cob,
}

impl ActionContext {
    /// Java-compatible constant alias for `ActionContext::Default`.
    pub const DEFAULT: Self = Self::Default;
    /// Java-compatible constant alias for `ActionContext::Cob`.
    pub const COB: Self = Self::Cob;

    /// Precedence order matching `ActionContext.getOrder()`.
    pub const fn order(&self) -> i32 {
        match self {
            Self::Default => 0,
            Self::Cob => 1,
        }
    }

    /// Human-readable description matching `ActionContext.getDescription()`.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Default => "Default context",
            Self::Cob => "Close of Business context",
        }
    }

    /// Associated [`BusinessDateType`] matching `ActionContext.getBusinessDateType()`.
    pub const fn business_date_type(&self) -> BusinessDateType {
        match self {
            Self::Default => BusinessDateType::BusinessDate,
            Self::Cob => BusinessDateType::CobDate,
        }
    }
}

impl fmt::Display for ActionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("DEFAULT"),
            Self::Cob => f.write_str("COB"),
        }
    }
}

/// Holds a tenant's database server and connection pool settings.
///
/// Ported from `org.apache.fineract.infrastructure.core.domain.FineractPlatformTenantConnection`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FineractPlatformTenantConnection {
    pub connection_id: Option<i64>,
    pub schema_server: String,
    pub schema_server_port: String,
    pub schema_connection_parameters: Option<String>,
    pub schema_username: String,
    pub schema_password: String,
    pub schema_name: String,
    pub read_only_schema_server: Option<String>,
    pub read_only_schema_server_port: Option<String>,
    pub read_only_schema_name: Option<String>,
    pub read_only_schema_username: Option<String>,
    pub read_only_schema_password: Option<String>,
    pub read_only_schema_connection_parameters: Option<String>,
    pub auto_update_enabled: bool,
    pub initial_size: i32,
    pub validation_interval: i64,
    pub remove_abandoned: bool,
    pub remove_abandoned_timeout: i32,
    pub log_abandoned: bool,
    pub abandon_when_percentage_full: i32,
    pub max_active: i32,
    pub min_idle: i32,
    pub max_idle: i32,
    pub suspect_timeout: i32,
    pub time_between_eviction_runs_millis: i32,
    pub min_evictable_idle_time_millis: i32,
    pub test_on_borrow: bool,
    pub master_password_hash: Option<String>,
}

impl Default for FineractPlatformTenantConnection {
    fn default() -> Self {
        Self {
            connection_id: Some(1),
            schema_server: "localhost".to_string(),
            schema_server_port: "5432".to_string(),
            schema_connection_parameters: None,
            schema_username: "postgres".to_string(),
            schema_password: String::new(),
            schema_name: "fineract_default".to_string(),
            read_only_schema_server: None,
            read_only_schema_server_port: None,
            read_only_schema_name: None,
            read_only_schema_username: None,
            read_only_schema_password: None,
            read_only_schema_connection_parameters: None,
            auto_update_enabled: true,
            initial_size: 5,
            validation_interval: 30_000,
            remove_abandoned: true,
            remove_abandoned_timeout: 60,
            log_abandoned: true,
            abandon_when_percentage_full: 50,
            max_active: 40,
            min_idle: 20,
            max_idle: 10,
            suspect_timeout: 60,
            time_between_eviction_runs_millis: 34_000,
            min_evictable_idle_time_millis: 60_000,
            test_on_borrow: true,
            master_password_hash: None,
        }
    }
}

impl FineractPlatformTenantConnection {
    /// Formats a JDBC/connection URL matching `FineractPlatformTenantConnection.toJdbcUrl(...)`.
    pub fn to_jdbc_url(
        protocol: &str,
        host: &str,
        port: &str,
        db: &str,
        parameters: Option<&str>,
    ) -> String {
        let mut url = format!("{protocol}://{host}:{port}/{db}");
        if let Some(params) = parameters {
            if !params.trim().is_empty() {
                url.push('?');
                url.push_str(params);
            }
        }
        url
    }

    /// Resolves JDBC protocol prefix from a driver class name matching `resolveProtocol(driver)`.
    pub fn resolve_protocol(driver: &str) -> Result<&'static str, String> {
        match driver {
            "org.postgresql.Driver" => Ok("jdbc:postgresql"),
            "com.mysql.cj.jdbc.Driver" => Ok("jdbc:mysql"),
            "org.mariadb.jdbc.Driver" => Ok("jdbc:mariadb"),
            other => Err(format!("Unsupported JDBC driver: {other}")),
        }
    }
}

impl fmt::Display for FineractPlatformTenantConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.schema_name, self.schema_server, self.schema_server_port
        )?;
        if let Some(params) = &self.schema_connection_parameters {
            if !params.is_empty() {
                write!(f, "?{params}")?;
            }
        }
        Ok(())
    }
}

/// Platform tenant metadata and database connection descriptor.
///
/// Ported from `org.apache.fineract.infrastructure.core.domain.FineractPlatformTenant`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FineractPlatformTenant {
    pub id: i64,
    pub tenant_identifier: String,
    pub name: String,
    pub timezone_id: String,
    pub connection: Option<FineractPlatformTenantConnection>,
}

impl Default for FineractPlatformTenant {
    fn default() -> Self {
        Self {
            id: 1,
            tenant_identifier: "default".to_string(),
            name: "Default Demo Tenant".to_string(),
            timezone_id: "Asia/Kolkata".to_string(),
            connection: Some(FineractPlatformTenantConnection::default()),
        }
    }
}

impl FineractPlatformTenant {
    /// Creates a new [`FineractPlatformTenant`].
    pub fn new(
        id: i64,
        tenant_identifier: impl Into<String>,
        name: impl Into<String>,
        timezone_id: impl Into<String>,
        connection: Option<FineractPlatformTenantConnection>,
    ) -> Self {
        Self {
            id,
            tenant_identifier: tenant_identifier.into(),
            name: name.into(),
            timezone_id: timezone_id.into(),
            connection,
        }
    }

    pub fn id(&self) -> i64 {
        self.id
    }

    pub fn tenant_identifier(&self) -> &str {
        &self.tenant_identifier
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn timezone_id(&self) -> &str {
        &self.timezone_id
    }

    pub fn connection(&self) -> Option<&FineractPlatformTenantConnection> {
        self.connection.as_ref()
    }
}

/// Immutable async-safe execution context replacing `FineractContext` and `ThreadLocalContextUtil`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionContext {
    pub tenant_id: String,
    pub tenant: FineractPlatformTenant,
    pub business_dates: HashMap<BusinessDateType, NaiveDate>,
    pub action_context: ActionContext,
    pub authenticated_user_id: Option<i64>,
    #[serde(alias = "is_Super_user", alias = "isSuperUser")]
    pub is_super_user: bool,
}

/// Alias matching upstream `FineractContext`.
pub type FineractContext = ExecutionContext;

/// Alias matching `BusinessContext` in `rust_constitution.md`.
pub type BusinessContext = ExecutionContext;

impl Default for ExecutionContext {
    fn default() -> Self {
        let today = Utc::now().date_naive();
        let mut business_dates = HashMap::with_capacity(2);
        business_dates.insert(BusinessDateType::BusinessDate, today);
        business_dates.insert(BusinessDateType::CobDate, today);
        let tenant = FineractPlatformTenant::default();
        Self {
            tenant_id: tenant.tenant_identifier.clone(),
            tenant,
            business_dates,
            action_context: ActionContext::Default,
            authenticated_user_id: Some(1),
            is_super_user: false,
        }
    }
}

impl ExecutionContext {
    /// Creates a system execution context for a tenant.
    pub fn system(tenant_id: impl Into<String>) -> Self {
        Self::default_for_tenant(tenant_id)
    }

    /// Creates an execution context for a tenant with a default business date.
    pub fn default_for_tenant(tenant_id: impl Into<String>) -> Self {
        Self::new(tenant_id.into(), chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
    }

    /// Creates an [`ExecutionContext`] for a given `tenant_id` and active `business_date`.
    pub fn new(tenant_id: impl Into<String>, business_date: NaiveDate) -> Self {
        let tenant_id = tenant_id.into();
        let mut business_dates = HashMap::with_capacity(2);
        business_dates.insert(BusinessDateType::BusinessDate, business_date);
        business_dates.insert(BusinessDateType::CobDate, business_date);
        let tenant = FineractPlatformTenant {
            tenant_identifier: tenant_id.clone(),
            ..FineractPlatformTenant::default()
        };
        Self {
            tenant_id,
            tenant,
            business_dates,
            action_context: ActionContext::Default,
            authenticated_user_id: Some(1),
            is_super_user: false,
        }
    }

    /// Returns the business date corresponding to `self.action_context.business_date_type()`,
    /// matching `ThreadLocalContextUtil.getBusinessDate()`.
    pub fn get_business_date(&self) -> NaiveDate {
        let target_type = self.action_context.business_date_type();
        self.business_dates
            .get(&target_type)
            .or_else(|| self.business_dates.get(&BusinessDateType::BusinessDate))
            .copied()
            .unwrap_or_else(|| Utc::now().date_naive())
    }

    /// Ergonomic alias for [`ExecutionContext::get_business_date`].
    pub fn business_date(&self) -> NaiveDate {
        self.get_business_date()
    }

    /// Ergonomic alias for COB business date.
    pub fn cob_date(&self) -> NaiveDate {
        self.business_date_by_type(BusinessDateType::CobDate)
            .unwrap_or_else(|| self.business_date())
    }

    /// Returns the business date for a specific [`BusinessDateType`],
    /// matching `ThreadLocalContextUtil.getBusinessDateByType(businessDateType)`.
    pub fn get_business_date_by_type(&self, date_type: BusinessDateType) -> Option<NaiveDate> {
        self.business_dates.get(&date_type).copied()
    }

    /// Ergonomic alias for [`ExecutionContext::get_business_date_by_type`].
    pub fn business_date_by_type(&self, date_type: BusinessDateType) -> Option<NaiveDate> {
        self.get_business_date_by_type(date_type)
    }

    /// Returns a reference to the current [`FineractPlatformTenant`],
    /// matching `ThreadLocalContextUtil.getTenant()`.
    pub fn get_tenant(&self) -> &FineractPlatformTenant {
        &self.tenant
    }

    /// Returns the active [`ActionContext`],
    /// matching `ThreadLocalContextUtil.getActionContext()`.
    pub fn get_action_context(&self) -> ActionContext {
        self.action_context
    }

    /// Returns a cloned [`ExecutionContext`] with the updated [`ActionContext`].
    pub fn with_action_context(&self, action_context: ActionContext) -> Self {
        let mut next = self.clone();
        next.action_context = action_context;
        next
    }

    /// Returns a cloned [`ExecutionContext`] with a specific [`BusinessDateType`] updated.
    pub fn with_business_date(&self, date_type: BusinessDateType, date: NaiveDate) -> Self {
        let mut next = self.clone();
        next.business_dates.insert(date_type, date);
        next
    }

    /// Returns whether the authenticated user has superuser privileges.
    pub const fn is_super_user(&self) -> bool {
        self.is_super_user
    }

    /// Compatibility accessor matching `is_Super_user`.
    #[allow(non_snake_case)]
    pub const fn is_Super_user(&self) -> bool {
        self.is_super_user
    }
}

tokio::task_local! {
    static TASK_EXECUTION_CONTEXT: Arc<ExecutionContext>;
}

/// Runs an asynchronous future within the scope of the provided [`ExecutionContext`].
///
/// Accepts either an owned `ExecutionContext` or an `Arc<ExecutionContext>`.
pub async fn with_execution_context<C, F, R>(ctx: C, fut: F) -> R
where
    C: Into<Arc<ExecutionContext>>,
    F: Future<Output = R>,
{
    TASK_EXECUTION_CONTEXT.scope(ctx.into(), fut).await
}

/// Runs a synchronous closure within the scope of the provided [`ExecutionContext`].
pub fn with_execution_context_sync<C, F, R>(ctx: C, f: F) -> R
where
    C: Into<Arc<ExecutionContext>>,
    F: FnOnce() -> R,
{
    TASK_EXECUTION_CONTEXT.sync_scope(ctx.into(), f)
}

/// Returns the active task-local [`ExecutionContext`] if one is set, or `None` outside a task scope.
pub fn try_current_execution_context() -> Option<Arc<ExecutionContext>> {
    TASK_EXECUTION_CONTEXT.try_with(Arc::clone).ok()
}

/// Returns the active task-local [`ExecutionContext`], or a default context if called outside a scope.
pub fn current_execution_context() -> Arc<ExecutionContext> {
    try_current_execution_context().unwrap_or_else(|| Arc::new(ExecutionContext::default()))
}

/// Returns the active business date from the current task-local [`ExecutionContext`],
/// replacing `ThreadLocalContextUtil.getBusinessDate()`.
pub fn get_business_date() -> NaiveDate {
    current_execution_context().get_business_date()
}

/// Returns the business date for `date_type` from the current task-local [`ExecutionContext`],
/// replacing `ThreadLocalContextUtil.getBusinessDateByType(type)`.
pub fn get_business_date_by_type(date_type: BusinessDateType) -> Option<NaiveDate> {
    current_execution_context().get_business_date_by_type(date_type)
}

/// Returns all business dates from the current task-local [`ExecutionContext`],
/// replacing `ThreadLocalContextUtil.getBusinessDates()`.
pub fn get_business_dates() -> HashMap<BusinessDateType, NaiveDate> {
    current_execution_context().business_dates.clone()
}

/// Returns the active [`FineractPlatformTenant`] from the current task-local [`ExecutionContext`],
/// replacing `ThreadLocalContextUtil.getTenant()`.
pub fn get_tenant() -> FineractPlatformTenant {
    current_execution_context().tenant.clone()
}

/// Returns the active [`ActionContext`] from the current task-local [`ExecutionContext`],
/// replacing `ThreadLocalContextUtil.getActionContext()`.
pub fn get_action_context() -> ActionContext {
    current_execution_context().action_context
}

/// Compatibility facade matching upstream `org.apache.fineract.infrastructure.core.service.ThreadLocalContextUtil`.
pub struct ThreadLocalContextUtil;

impl ThreadLocalContextUtil {
    pub const CONTEXT_TENANTS: &'static str = "tenants";

    pub fn get_context() -> Arc<ExecutionContext> {
        current_execution_context()
    }

    pub fn get_tenant() -> FineractPlatformTenant {
        get_tenant()
    }

    pub fn get_business_dates() -> HashMap<BusinessDateType, NaiveDate> {
        get_business_dates()
    }

    pub fn get_business_date_by_type(date_type: BusinessDateType) -> Option<NaiveDate> {
        get_business_date_by_type(date_type)
    }

    pub fn get_business_date() -> NaiveDate {
        get_business_date()
    }

    pub fn get_action_context() -> ActionContext {
        get_action_context()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_local_execution_context_propagation() {
        let business_date = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let cob_date = NaiveDate::from_ymd_opt(2026, 10, 29).unwrap();

        let ctx = ExecutionContext::new("acme-bank", business_date)
            .with_business_date(BusinessDateType::CobDate, cob_date)
            .with_action_context(ActionContext::Cob);

        with_execution_context(ctx, async {
            let active = current_execution_context();
            assert_eq!(active.tenant_id, "acme-bank");
            assert_eq!(active.get_tenant().tenant_identifier(), "acme-bank");
            assert_eq!(active.get_action_context(), ActionContext::Cob);
            assert_eq!(get_business_date(), cob_date);
            assert_eq!(
                get_business_date_by_type(BusinessDateType::BusinessDate),
                Some(business_date)
            );
            assert_eq!(
                get_business_date_by_type(BusinessDateType::CobDate),
                Some(cob_date)
            );
        })
        .await;
    }

    #[test]
    fn test_tenant_connection_display_and_jdbc_url() {
        let mut conn = FineractPlatformTenantConnection::default();
        conn.schema_name = "fineract_default".to_string();
        conn.schema_server = "db.internal".to_string();
        conn.schema_server_port = "5432".to_string();
        conn.schema_connection_parameters = Some("sslmode=disable".to_string());

        assert_eq!(
            conn.to_string(),
            "fineract_default:db.internal:5432?sslmode=disable"
        );
        assert_eq!(
            FineractPlatformTenantConnection::resolve_protocol("org.postgresql.Driver").unwrap(),
            "jdbc:postgresql"
        );
        assert_eq!(
            FineractPlatformTenantConnection::to_jdbc_url(
                "jdbc:postgresql",
                "db.internal",
                "5432",
                "fineract_default",
                Some("sslmode=disable")
            ),
            "jdbc:postgresql://db.internal:5432/fineract_default?sslmode=disable"
        );
    }
}

/// Type alias matching `FineractPlatformContext` to `ExecutionContext`.
pub type FineractPlatformContext = ExecutionContext;

/// Type alias matching `FineractTenant` to `FineractPlatformTenant`.
pub type FineractTenant = FineractPlatformTenant;

/// Idempotency key wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct IdempotencyKey(pub String);

impl IdempotencyKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for IdempotencyKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for IdempotencyKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Tenant database pool registry.
#[derive(Debug, Default, Clone)]
pub struct TenantDatabasePoolRegistry;

impl TenantDatabasePoolRegistry {
    pub fn new() -> Self {
        Self
    }
}
