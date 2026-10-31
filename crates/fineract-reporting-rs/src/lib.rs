//! `fineract-reporting-rs` — Read Reporting Engine (`stretchy_report`), SQL Injection Guard,
//! `GenericResultsetData` JSON/CSV Renderer, Dynamic User-Defined DataTables (`x_registered_table`),
//! and AdHoc Queries (`m_adhoc`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use fineract_command_rs::pipeline::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::context::ExecutionContext;
use fineract_core_rs::db::{DialectSql, SqlDialect};
use fineract_core_rs::json_command::JsonCommand;
use fineract_validation_rs::{FineractError, PlatformDomainRuleException};

/// Core Fineract entity tables that support dynamic DataTables (`x_registered_table`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityTables {
    Client,
    Group,
    Center,
    Loan,
    Savings,
    Office,
    LoanProduct,
    SavingsProduct,
    ShareProduct,
}

impl EntityTables {
    pub fn application_table_name(&self) -> &'static str {
        match self {
            Self::Client => "m_client",
            Self::Group => "m_group",
            Self::Center => "m_center",
            Self::Loan => "m_loan",
            Self::Savings => "m_savings_account",
            Self::Office => "m_office",
            Self::LoanProduct => "m_product_loan",
            Self::SavingsProduct => "m_savings_product",
            Self::ShareProduct => "m_share_product",
        }
    }

    pub fn foreign_key_column_name(&self) -> &'static str {
        match self {
            Self::Client => "client_id",
            Self::Group => "group_id",
            Self::Center => "center_id",
            Self::Loan => "loan_id",
            Self::Savings => "savings_account_id",
            Self::Office => "office_id",
            Self::LoanProduct => "loan_product_id",
            Self::SavingsProduct => "savings_product_id",
            Self::ShareProduct => "share_product_id",
        }
    }
}

/// Column metadata inside `GenericResultsetData` and `x_registered_table`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultsetColumnHeaderData {
    pub column_name: String,
    pub column_type: String,
    pub column_length: Option<u32>,
    pub column_display_type: String,
    pub is_column_nullable: bool,
    pub is_column_primary_key: bool,
    pub is_column_unique: bool,
    pub is_column_indexed: bool,
}

/// Single row inside `GenericResultsetData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultsetRowData {
    pub row: Vec<Option<String>>,
}

/// Wire-compatible Fineract tabular report & DataTable result set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericResultsetData {
    pub column_headers: Vec<ResultsetColumnHeaderData>,
    pub data: Vec<ResultsetRowData>,
}

impl GenericResultsetData {
    pub fn to_csv(&self) -> String {
        self.to_csv_string()
    }

    pub fn to_csv_string(&self) -> String {
        let mut lines = Vec::with_capacity(self.data.len() + 1);
        let header_line: Vec<String> = self
            .column_headers
            .iter()
            .map(|h| format!("\"{}\"", h.column_name.replace('"', "\"\"")))
            .collect();
        lines.push(header_line.join(","));
        for row_data in &self.data {
            let cells: Vec<String> = row_data
                .row
                .iter()
                .map(|cell| match cell {
                    Some(v) => format!("\"{}\"", v.replace('"', "\"\"")),
                    None => String::new(),
                })
                .collect();
            lines.push(cells.join(","));
        }
        lines.join("\n")
    }
}

/// Stretchy Report metadata (`stretchy_report` table).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StretchyReport {
    pub id: i64,
    pub report_name: String,
    pub report_type: String,
    pub report_subtype: Option<String>,
    pub report_category: Option<String>,
    pub report_sql: Option<String>,
    pub description: Option<String>,
    pub core_report: bool,
    pub use_report: bool,
}

/// Guards stretchy report parameter interpolation and DataTable SQL identifiers against SQL injection.
pub struct SqlInjectionAndReportValidator;

impl SqlInjectionAndReportValidator {
    pub fn validate_identifier(ident: &str) -> Result<(), FineractError> {
        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_]{0,62}$").unwrap();
        if !re.is_match(ident) {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.datatable.invalid.identifier",
                format!("Invalid or unsafe SQL identifier: {ident}"),
            )));
        }
        Ok(())
    }

    pub fn validate_and_interpolate_report_sql(
        report_sql: &str,
        params: &HashMap<String, String>,
    ) -> Result<String, FineractError> {
        let trimmed = report_sql.trim_start().to_uppercase();
        if !trimmed.starts_with("SELECT") {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.report.only.select.allowed",
                "Stretchy reports must begin with a SELECT statement",
            )));
        }
        let mut rendered = report_sql.to_string();
        for (k, v) in params {
            if v.contains(';')
                || v.contains("--")
                || v.contains("/*")
                || v.to_uppercase().contains("DROP ")
                || v.to_uppercase().contains("DELETE ")
                || v.to_uppercase().contains("UPDATE ")
            {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.report.sql.injection.detected",
                    format!("Unsafe report parameter value for {k}"),
                )));
            }
            let escaped = v.replace('\'', "''");
            rendered = rendered.replace(&format!("${{{k}}}"), &escaped);
            rendered = rendered.replace(&format!("R_${{{k}}}"), &escaped);
        }
        Ok(rendered)
    }
}

/// Dynamic user-defined DataTable (`x_registered_table`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredDataTable {
    pub registered_table_name: String,
    pub application_table_name: String,
    pub entity_subtype: Option<String>,
    pub category: i32,
    pub is_multi_row: bool,
    pub columns: Vec<ResultsetColumnHeaderData>,
}

impl RegisteredDataTable {
    pub fn generate_create_table_ddl(&self, dialect: SqlDialect) -> Result<String, FineractError> {
        SqlInjectionAndReportValidator::validate_identifier(&self.registered_table_name)?;
        SqlInjectionAndReportValidator::validate_identifier(&self.application_table_name)?;

        let mut col_defs = Vec::new();
        if self.is_multi_row {
            let pk_def = match dialect {
                SqlDialect::Postgres => "id BIGSERIAL PRIMARY KEY",
                SqlDialect::MySql => "id BIGINT AUTO_INCREMENT PRIMARY KEY",
            };
            col_defs.push(pk_def.to_string());
            col_defs.push("app_table_id BIGINT NOT NULL".to_string());
        } else {
            col_defs.push("app_table_id BIGINT PRIMARY KEY NOT NULL".to_string());
        }

        for col in &self.columns {
            SqlInjectionAndReportValidator::validate_identifier(&col.column_name)?;
            let sql_type = match col.column_display_type.to_uppercase().as_str() {
                "STRING" | "TEXT" => format!("VARCHAR({})", col.column_length.unwrap_or(255)),
                "INTEGER" | "NUMBER" => "BIGINT".to_string(),
                "DECIMAL" => "DECIMAL(19,6)".to_string(),
                "BOOLEAN" => "BOOLEAN".to_string(),
                "DATE" => "DATE".to_string(),
                "DATETIME" => "TIMESTAMP".to_string(),
                _ => "VARCHAR(255)".to_string(),
            };
            let null_clause = if col.is_column_nullable { "" } else { " NOT NULL" };
            let uniq_clause = if col.is_column_unique { " UNIQUE" } else { "" };
            col_defs.push(format!("{}{}{}", col.column_name, " ".to_string() + &sql_type, null_clause.to_string() + uniq_clause));
        }

        Ok(format!(
            "CREATE TABLE {} ({}, CONSTRAINT fk_{}_app FOREIGN KEY (app_table_id) REFERENCES {}(id))",
            self.registered_table_name,
            col_defs.join(", "),
            self.registered_table_name,
            self.application_table_name
        ))
    }
}

/// AdHoc Query definition (`m_adhoc`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdHocQuery {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub table_name: String,
    pub table_fields: String,
    pub email: Option<String>,
    pub report_run_frequency_code: Option<i32>,
    pub is_active: bool,
}

/// Dual-Database Repository for Stretchy Reports, Dynamic DataTables, and AdHoc Queries.
pub struct DualDbReportingAndDataTablesRepository {
    next_id: AtomicI64,
    pub dialect: SqlDialect,
    pub reports: DashMap<i64, StretchyReport>,
    pub datatables: DashMap<String, RegisteredDataTable>,
    pub datatable_rows: DashMap<(String, i64), Vec<HashMap<String, Value>>>,
    pub adhoc_queries: DashMap<i64, AdHocQuery>,
}

impl DualDbReportingAndDataTablesRepository {
    pub fn new(dialect: SqlDialect) -> Self {
        Self {
            next_id: AtomicI64::new(1),
            dialect,
            reports: DashMap::new(),
            datatables: DashMap::new(),
            datatable_rows: DashMap::new(),
            adhoc_queries: DashMap::new(),
        }
    }

    pub fn register_datatable(
        &self,
        dt: RegisteredDataTable,
    ) -> Result<String, FineractError> {
        let ddl = dt.generate_create_table_ddl(self.dialect)?;
        self.datatables
            .insert(dt.registered_table_name.clone(), dt);
        Ok(ddl)
    }

    pub fn insert_datatable_entry(
        &self,
        table_name: &str,
        app_table_id: i64,
        row: HashMap<String, Value>,
    ) -> Result<usize, FineractError> {
        let dt = self.datatables.get(table_name).ok_or_else(|| {
            FineractError::ResourceNotFound {
                globalisation_code: "error.msg.datatable.not.found".to_string(),
                default_message: format!("DataTable {table_name} not found"),
                resource_id: table_name.to_string(),
            }
        })?;
        let mut entry = self
            .datatable_rows
            .entry((table_name.to_string(), app_table_id))
            .or_default();
        if !dt.is_multi_row && !entry.is_empty() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.datatable.single.row.only",
                format!("DataTable {table_name} only allows a single row per entity"),
            )));
        }
        entry.push(row);
        Ok(entry.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stretchy_report_sql_injection_guard_and_csv_export() {
        let mut params = HashMap::new();
        params.insert("officeId".to_string(), "1".to_string());
        let safe_sql = SqlInjectionAndReportValidator::validate_and_interpolate_report_sql(
            "SELECT id, display_name FROM m_client WHERE office_id = ${officeId}",
            &params,
        )
        .unwrap();
        assert_eq!(
            safe_sql,
            "SELECT id, display_name FROM m_client WHERE office_id = 1"
        );

        params.insert("officeId".to_string(), "1; DROP TABLE m_client".to_string());
        assert!(SqlInjectionAndReportValidator::validate_and_interpolate_report_sql(
            "SELECT id FROM m_client WHERE office_id = ${officeId}",
            &params
        )
        .is_err());

        let resultset = GenericResultsetData {
            column_headers: vec![ResultsetColumnHeaderData {
                column_name: "client_id".to_string(),
                column_type: "BIGINT".to_string(),
                column_length: None,
                column_display_type: "INTEGER".to_string(),
                is_column_nullable: false,
                is_column_primary_key: true,
                is_column_unique: true,
                is_column_indexed: true,
            }],
            data: vec![ResultsetRowData {
                row: vec![Some("101".to_string())],
            }],
        };
        assert_eq!(resultset.to_csv_string(), "\"client_id\"\n\"101\"");
    }

    #[test]
    fn test_dynamic_datatable_ddl_and_single_row_enforcement() {
        let repo = DualDbReportingAndDataTablesRepository::new(SqlDialect::Postgres);
        let dt = RegisteredDataTable {
            registered_table_name: "dt_client_kyc_extra".to_string(),
            application_table_name: "m_client".to_string(),
            entity_subtype: Some("PERSON".to_string()),
            category: 100,
            is_multi_row: false,
            columns: vec![ResultsetColumnHeaderData {
                column_name: "tax_id_number".to_string(),
                column_type: "VARCHAR".to_string(),
                column_length: Some(64),
                column_display_type: "STRING".to_string(),
                is_column_nullable: false,
                is_column_primary_key: false,
                is_column_unique: true,
                is_column_indexed: true,
            }],
        };
        let ddl = repo.register_datatable(dt).unwrap();
        assert!(ddl.contains("CREATE TABLE dt_client_kyc_extra"));
        assert!(ddl.contains("tax_id_number VARCHAR(64) NOT NULL UNIQUE"));

        let mut row = HashMap::new();
        row.insert("tax_id_number".to_string(), Value::String("TIN-999".to_string()));
        assert_eq!(
            repo.insert_datatable_entry("dt_client_kyc_extra", 10, row.clone())
                .unwrap(),
            1
        );
        // Second insert on single-row datatable must fail
        assert!(repo
            .insert_datatable_entry("dt_client_kyc_extra", 10, row)
            .is_err());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReportRequest {
    pub report_name: String,
    pub query_params: HashMap<String, String>,
    pub export_csv: bool,
    pub parameter_type: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ReadReportingService;

impl ReadReportingService {
    pub fn new() -> Self {
        Self
    }

    pub fn list_reports(&self) -> Vec<StretchyReport> {
        vec![
            StretchyReport {
                id: 1,
                report_name: "Client Listing".to_string(),
                report_type: "Table".to_string(),
                report_subtype: None,
                report_category: Some("Client".to_string()),
                report_sql: Some("SELECT id, display_name FROM m_client".to_string()),
                description: Some("Listing of active clients".to_string()),
                core_report: true,
                use_report: true,
            }
        ]
    }

    pub fn execute_stretchy_report(
        &self,
        _req: &RunReportRequest,
        rows: Vec<Vec<Value>>,
    ) -> Result<GenericResultsetData, FineractError> {
        let col_count = rows.first().map(|r| r.len()).unwrap_or(2);
        let headers: Vec<ResultsetColumnHeaderData> = (0..col_count)
            .map(|i| {
                let (name, typ) = match i {
                    0 => ("id", "INTEGER"),
                    1 => ("officeName", "VARCHAR"),
                    2 => ("accountNo", "VARCHAR"),
                    3 => ("displayName", "VARCHAR"),
                    _ => ("col", "VARCHAR"),
                };
                ResultsetColumnHeaderData {
                    column_name: name.to_string(),
                    column_type: typ.to_string(),
                    column_length: Some(100),
                    column_display_type: typ.to_string(),
                    is_column_nullable: true,
                    is_column_primary_key: i == 0,
                    is_column_unique: i == 0,
                    is_column_indexed: i == 0,
                }
            })
            .collect();
        let row_data = rows
            .into_iter()
            .map(|r| ResultsetRowData {
                row: r.into_iter().map(|val| match val {
                    Value::String(s) => Some(s),
                    Value::Null => None,
                    other => Some(other.to_string()),
                }).collect(),
            })
            .collect();
        Ok(GenericResultsetData {
            column_headers: headers,
            data: row_data,
        })
    }
}
