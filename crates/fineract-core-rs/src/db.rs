//! Dual-Database (`PostgreSQL 16+` & `MySQL / MariaDB`) SQL dialect layer and multi-tenant connection registry.
//!
//! Adheres to Invariant 2 of the Fineract-RS Architectural Constitution:
//! - Supports both PostgreSQL and MySQL/MariaDB via `sqlx::AnyPool` and [`SqlDialect`].
//! - Translates positional `?` placeholders to `$1, $2, ...` for PostgreSQL while ignoring `?` inside quotes.
//! - Supports `SELECT ... FOR UPDATE SKIP LOCKED` for Close of Business (COB) locking (`m_loan_account_locks`).
//! - Provides [`TenantPoolRegistry`] backed by `DashMap<String, (SqlDialect, sqlx::AnyPool)>`.

use std::fmt;
use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::any::AnyPoolOptions;
use sqlx::AnyPool;
use thiserror::Error;

use crate::context::current_execution_context;

/// Supported relational database dialects in Fineract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SqlDialect {
    #[default]
    Postgres,
    MySql,
}

impl SqlDialect {
    /// Detects the [`SqlDialect`] from a database connection URL or JDBC URL.
    pub fn from_url(url: &str) -> Result<Self, DbError> {
        let normalized = url.trim().to_ascii_lowercase();
        if normalized.starts_with("postgres://")
            || normalized.starts_with("postgresql://")
            || normalized.starts_with("jdbc:postgresql://")
        {
            Ok(Self::Postgres)
        } else if normalized.starts_with("mysql://")
            || normalized.starts_with("mariadb://")
            || normalized.starts_with("jdbc:mysql://")
            || normalized.starts_with("jdbc:mariadb://")
        {
            Ok(Self::MySql)
        } else {
            Err(DbError::UnsupportedDialectUrl(url.to_string()))
        }
    }

    /// Returns `true` if the dialect is PostgreSQL.
    pub const fn is_postgres(&self) -> bool {
        matches!(self, Self::Postgres)
    }

    /// Returns `true` if the dialect is MySQL / MariaDB.
    pub const fn is_mysql(&self) -> bool {
        matches!(self, Self::MySql)
    }
}

impl fmt::Display for SqlDialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres => f.write_str("PostgreSQL"),
            Self::MySql => f.write_str("MySQL"),
        }
    }
}

/// Database routing and dialect error type.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("Tenant database pool not registered for tenant `{0}`")]
    TenantNotFound(String),

    #[error("Unsupported database URL scheme for SQL dialect detection: `{0}`")]
    UnsupportedDialectUrl(String),

    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// SQL dialect query rewriter for dual-database compatibility (`PostgreSQL` vs `MySQL/MariaDB`).
pub struct DialectSql;

impl DialectSql {
    /// Rewrites positional `?` placeholders to `$1, $2, ...` when `dialect` is [`SqlDialect::Postgres`],
    /// while ignoring `?` characters inside single quotes (`'...'`) and double quotes (`"..."`),
    /// and preserves `?` placeholders unchanged for [`SqlDialect::MySql`].
    pub fn rewrite_placeholders(sql: &str, dialect: SqlDialect) -> String {
        match dialect {
            SqlDialect::MySql => sql.to_string(),
            SqlDialect::Postgres => Self::rewrite_postgres_placeholders(sql),
        }
    }

    /// Alias for [`DialectSql::rewrite_placeholders`] matching `rust_constitution.md`.
    pub fn bind_rewrite(sql: &str, dialect: SqlDialect) -> String {
        Self::rewrite_placeholders(sql, dialect)
    }

    /// Rewrites placeholders for `dialect` and appends `FOR UPDATE SKIP LOCKED` for Close of Business (COB)
    /// non-blocking row lock acquisition (`m_loan_account_locks`).
    pub fn for_update_skip_locked(sql: &str, dialect: SqlDialect) -> String {
        let rewritten = Self::rewrite_placeholders(sql, dialect);
        let trimmed = rewritten.trim_end().trim_end_matches(';').trim_end();
        let upper = trimmed.to_ascii_uppercase();

        if upper.ends_with("FOR UPDATE SKIP LOCKED") {
            trimmed.to_string()
        } else if upper.ends_with("FOR UPDATE") {
            format!("{trimmed} SKIP LOCKED")
        } else {
            format!("{trimmed} FOR UPDATE SKIP LOCKED")
        }
    }

    fn rewrite_postgres_placeholders(sql: &str) -> String {
        let mut out = String::with_capacity(sql.len() + 16);
        let mut chars = sql.chars().peekable();
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut param_index: usize = 0;

        while let Some(ch) = chars.next() {
            if in_single_quote {
                out.push(ch);
                if ch == '\\' {
                    if let Some(escaped) = chars.next() {
                        out.push(escaped);
                    }
                } else if ch == '\'' {
                    if chars.peek() == Some(&'\'') {
                        if let Some(escaped_quote) = chars.next() {
                            out.push(escaped_quote);
                        }
                    } else {
                        in_single_quote = false;
                    }
                }
            } else if in_double_quote {
                out.push(ch);
                if ch == '\\' {
                    if let Some(escaped) = chars.next() {
                        out.push(escaped);
                    }
                } else if ch == '"' {
                    if chars.peek() == Some(&'"') {
                        if let Some(escaped_quote) = chars.next() {
                            out.push(escaped_quote);
                        }
                    } else {
                        in_double_quote = false;
                    }
                }
            } else {
                match ch {
                    '\'' => {
                        in_single_quote = true;
                        out.push(ch);
                    }
                    '"' => {
                        in_double_quote = true;
                        out.push(ch);
                    }
                    '?' => {
                        param_index += 1;
                        out.push('$');
                        out.push_str(&param_index.to_string());
                    }
                    _ => out.push(ch),
                }
            }
        }

        out
    }
}

/// Multi-tenant runtime database pool registry mapping tenant identifiers (and `"fineract_tenants"`)
/// to `(SqlDialect, sqlx::AnyPool)`.
#[derive(Debug, Clone, Default)]
pub struct TenantPoolRegistry {
    pub pools: Arc<DashMap<String, (SqlDialect, AnyPool)>>,
}

impl TenantPoolRegistry {
    /// Canonical registry key for the master tenant directory database (`fineract_tenants`).
    pub const FINERACT_TENANTS_DB: &'static str = "fineract_tenants";
    /// Canonical tenant identifier for the default Fineract tenant (`default`).
    pub const DEFAULT_TENANT_ID: &'static str = "default";

    /// Creates an empty [`TenantPoolRegistry`].
    pub fn new() -> Self {
        Self {
            pools: Arc::new(DashMap::new()),
        }
    }

    /// Creates a [`TenantPoolRegistry`] pre-populated with the `"fineract_tenants"` catalog pool.
    pub fn with_tenants_pool(dialect: SqlDialect, pool: AnyPool) -> Self {
        let registry = Self::new();
        registry.register(Self::FINERACT_TENANTS_DB, dialect, pool);
        registry
    }

    /// Returns a reference to the underlying `DashMap<String, (SqlDialect, AnyPool)>`.
    pub fn map(&self) -> &DashMap<String, (SqlDialect, AnyPool)> {
        &self.pools
    }

    /// Registers or replaces a `(SqlDialect, AnyPool)` for `tenant_id`.
    pub fn register(&self, tenant_id: impl Into<String>, dialect: SqlDialect, pool: AnyPool) {
        self.pools.insert(tenant_id.into(), (dialect, pool));
    }

    /// Alias for [`TenantPoolRegistry::register`].
    pub fn insert(&self, tenant_id: impl Into<String>, dialect: SqlDialect, pool: AnyPool) {
        self.register(tenant_id, dialect, pool);
    }

    /// Connects a new [`sqlx::AnyPool`] from `database_url`, detects its [`SqlDialect`],
    /// and registers it under `tenant_id`.
    pub async fn connect_and_register(
        &self,
        tenant_id: impl Into<String>,
        database_url: &str,
        max_connections: u32,
    ) -> Result<(SqlDialect, AnyPool), DbError> {
        let dialect = SqlDialect::from_url(database_url)?;
        let pool = AnyPoolOptions::new()
            .max_connections(max_connections.max(1))
            .connect(database_url)
            .await?;
        let tenant_id = tenant_id.into();
        self.pools.insert(tenant_id, (dialect, pool.clone()));
        Ok((dialect, pool))
    }

    /// Retrieves the `(SqlDialect, AnyPool)` for `tenant_id`.
    pub fn get(&self, tenant_id: &str) -> Option<(SqlDialect, AnyPool)> {
        self.pools.get(tenant_id).map(|entry| entry.value().clone())
    }

    /// Retrieves the `(SqlDialect, AnyPool)` for `tenant_id` or returns [`DbError::TenantNotFound`].
    pub fn get_or_err(&self, tenant_id: &str) -> Result<(SqlDialect, AnyPool), DbError> {
        self.get(tenant_id)
            .ok_or_else(|| DbError::TenantNotFound(tenant_id.to_string()))
    }

    /// Retrieves the [`AnyPool`] for `tenant_id`.
    pub fn get_pool(&self, tenant_id: &str) -> Option<AnyPool> {
        self.pools
            .get(tenant_id)
            .map(|entry| entry.value().1.clone())
    }

    /// Retrieves the [`SqlDialect`] for `tenant_id`.
    pub fn get_dialect(&self, tenant_id: &str) -> Option<SqlDialect> {
        self.pools.get(tenant_id).map(|entry| entry.value().0)
    }

    /// Retrieves the `"fineract_tenants"` catalog `(SqlDialect, AnyPool)`.
    pub fn tenants_catalog_pool(&self) -> Option<(SqlDialect, AnyPool)> {
        self.get(Self::FINERACT_TENANTS_DB)
    }

    /// Resolves the `(SqlDialect, AnyPool)` for the active task-local [`ExecutionContext::tenant_id`].
    pub fn current_tenant_pool(&self) -> Result<(SqlDialect, AnyPool), DbError> {
        let ctx = current_execution_context();
        self.get_or_err(&ctx.tenant_id)
    }

    /// Removes a tenant pool from the registry.
    pub fn remove(&self, tenant_id: &str) -> Option<(SqlDialect, AnyPool)> {
        self.pools.remove(tenant_id).map(|(_, v)| v)
    }

    /// Checks whether `tenant_id` is registered.
    pub fn contains_tenant(&self, tenant_id: &str) -> bool {
        self.pools.contains_key(tenant_id)
    }

    /// Returns the number of registered pools.
    pub fn len(&self) -> usize {
        self.pools.len()
    }

    /// Returns `true` if no pools are registered.
    pub fn is_empty(&self) -> bool {
        self.pools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_placeholders_postgres_and_mysql() {
        let sql = "SELECT id, name FROM m_client WHERE office_id = ? AND status_enum = ? AND external_id = ?";
        assert_eq!(
            DialectSql::rewrite_placeholders(sql, SqlDialect::Postgres),
            "SELECT id, name FROM m_client WHERE office_id = $1 AND status_enum = $2 AND external_id = $3"
        );
        assert_eq!(DialectSql::rewrite_placeholders(sql, SqlDialect::MySql), sql);
    }

    #[test]
    fn test_rewrite_placeholders_ignores_quoted_question_marks() {
        let sql = r#"INSERT INTO m_note (note, "col?name", loan_id, tag) VALUES ('Why? Isn''t it?', ?, ?, "double""?""quote")"#;
        assert_eq!(
            DialectSql::rewrite_placeholders(sql, SqlDialect::Postgres),
            r#"INSERT INTO m_note (note, "col?name", loan_id, tag) VALUES ('Why? Isn''t it?', $1, $2, "double""?""quote")"#
        );
    }

    #[test]
    fn test_for_update_skip_locked() {
        let sql = "SELECT loan_id FROM m_loan_account_locks WHERE loan_id = ? AND lock_owner = ?;";
        assert_eq!(
            DialectSql::for_update_skip_locked(sql, SqlDialect::Postgres),
            "SELECT loan_id FROM m_loan_account_locks WHERE loan_id = $1 AND lock_owner = $2 FOR UPDATE SKIP LOCKED"
        );
        assert_eq!(
            DialectSql::for_update_skip_locked(sql, SqlDialect::MySql),
            "SELECT loan_id FROM m_loan_account_locks WHERE loan_id = ? AND lock_owner = ? FOR UPDATE SKIP LOCKED"
        );
    }

    #[test]
    fn test_dialect_from_url() {
        assert_eq!(
            SqlDialect::from_url("postgres://postgres:postgres@localhost:5432/fineract_default").unwrap(),
            SqlDialect::Postgres
        );
        assert_eq!(
            SqlDialect::from_url("mysql://root:mysql@localhost:3306/fineract_default").unwrap(),
            SqlDialect::MySql
        );
    }
}
