//! `fineract-ops-rs` — Operational Engines for Apache Fineract
//!
//! Decodes and ports the following upstream Apache Fineract operational modules into safe,
//! high-throughput Rust:
//! 1. **Bulk Import Engine (`m_import_document`)** — ported from
//!    `org.apache.fineract.infrastructure.bulkimport.*` (`GlobalEntityType`, `ImportDocument`,
//!    `CsvBulkImportProcessor` for Clients, Loans, Loan Repayments, Savings Deposits, and
//!    GL Journal Entries with `Status` and `Error` report column annotations).
//! 2. **Webhook & Event Hook Engine (`m_hook`, `m_hook_templates`, `m_hook_registered_events`,
//!    `m_hook_configuration`)** — ported from `org.apache.fineract.infrastructure.hooks.*`
//!    (`HookTemplate`, `HookRegisteredEvent`, `HookConfiguration`, `Hook`, and `WebhookDispatcher`
//!    with RFC 2104 `X-Fineract-Signature` HMAC-SHA256 signing and event routing).
//! 3. **SMS & Email Campaigns (`sms_campaign`, `sms_messages_outbound`,
//!    `scheduled_email_campaign`)** — ported from `org.apache.fineract.infrastructure.campaigns.*`
//!    and `org.apache.fineract.infrastructure.sms.*` (`CampaignTriggerType`,
//!    `SmsMessageStatusType`, `SmsCampaign`, Mustache-style `{{variable}}` template compilation,
//!    and `ScheduledEmailCampaign`).
//! 4. **SPM / Poverty Scorecard Surveys (`m_surveys`, `m_survey_questions`, `m_survey_responses`,
//!    `m_survey_scorecards`)** — ported from `org.apache.fineract.spm.*` (`Survey`,
//!    `SurveyComponent`, `SurveyQuestion`, `SurveyResponse`, `SurveyLookupTable`,
//!    `SurveyScorecardEntry`, and `SpmScorecardEngine`).
//! 5. **Credit Bureau Integration (`m_creditbureau`, `m_organisation_creditbureau`,
//!    `m_creditbureau_loanproduct_mapping`)** — ported from
//!    `org.apache.fineract.infrastructure.creditbureau.*`.
//! 6. **JLG & Individual Collection Sheet (`CollectionSheetData`, `IndividualCollectionSheetData`,
//!    `BulkRepaymentAndSavingsDepositResult`)** — ported from
//!    `org.apache.fineract.portfolio.collectionsheet.*`.
//! 7. **Custom Extension Plugin Registry (`:custom` replacement)** — `CustomModulePlugin` trait
//!    and `CustomPluginRegistry` allowing institutions to register custom schedule generators,
//!    command interceptors, COB steps, and accounting rules at runtime without modifying core crates.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use chrono::{NaiveDate, NaiveDateTime};
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Common Error Type
// ============================================================================

/// Operational error returned across Bulk Import, Webhooks, Campaigns, SPM,
/// Credit Bureau, Collection Sheet, and Custom Plugin subsystems.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum OpsError {
    #[error("validation error on field `{field}`: {message}")]
    Validation { field: String, message: String },

    #[error("invalid state transition: {0}")]
    InvalidState(String),

    #[error("entity not found: {0}")]
    NotFound(String),

    #[error("credit bureau check failed: {0}")]
    CreditBureauFailure(String),

    #[error("plugin error in `{plugin_id}`: {message}")]
    PluginError { plugin_id: String, message: String },
}

// ============================================================================
// 1. Bulk Import Engine (`m_import_document`)
// ============================================================================

/// Entity types supported by the Fineract Bulk Import engine (`m_import_document.entity_type`).
///
/// Ported from `org.apache.fineract.infrastructure.bulkimport.data.GlobalEntityType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum GlobalEntityType {
    ClientsPerson = 1,
    ClientsEntity = 2,
    Groups = 3,
    Centers = 4,
    Offices = 5,
    Staff = 6,
    Users = 7,
    ChartOfAccounts = 8,
    GlJournalEntries = 9,
    Loans = 10,
    LoanTransactions = 11,
    SavingsAccount = 12,
    SavingsTransactions = 13,
    FixedDepositAccounts = 14,
    RecurringDepositAccounts = 15,
    ShareAccounts = 16,
    Guarantors = 17,
}

impl GlobalEntityType {
    /// Returns the integer discriminant stored in `m_import_document.entity_type`.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Resolves a [`GlobalEntityType`] from its integer discriminant.
    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::ClientsPerson),
            2 => Some(Self::ClientsEntity),
            3 => Some(Self::Groups),
            4 => Some(Self::Centers),
            5 => Some(Self::Offices),
            6 => Some(Self::Staff),
            7 => Some(Self::Users),
            8 => Some(Self::ChartOfAccounts),
            9 => Some(Self::GlJournalEntries),
            10 => Some(Self::Loans),
            11 => Some(Self::LoanTransactions),
            12 => Some(Self::SavingsAccount),
            13 => Some(Self::SavingsTransactions),
            14 => Some(Self::FixedDepositAccounts),
            15 => Some(Self::RecurringDepositAccounts),
            16 => Some(Self::ShareAccounts),
            17 => Some(Self::Guarantors),
            _ => None,
        }
    }

    /// Returns the Fineract entity code string matching `GlobalEntityType.getCode()`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::ClientsPerson => "clients.person",
            Self::ClientsEntity => "clients.entity",
            Self::Groups => "groups",
            Self::Centers => "centers",
            Self::Offices => "offices",
            Self::Staff => "staff",
            Self::Users => "users",
            Self::ChartOfAccounts => "chartofaccounts",
            Self::GlJournalEntries => "gljournalentries",
            Self::Loans => "loans",
            Self::LoanTransactions => "loantransactions",
            Self::SavingsAccount => "savingsaccount",
            Self::SavingsTransactions => "savingstransactions",
            Self::FixedDepositAccounts => "fixeddepositaccounts",
            Self::RecurringDepositAccounts => "recurringdeposits",
            Self::ShareAccounts => "shareaccounts",
            Self::Guarantors => "guarantors",
        }
    }

    /// Returns the canonical workbook sheet name from `TemplatePopulateImportConstants`.
    pub const fn sheet_name(self) -> &'static str {
        match self {
            Self::ClientsPerson => "ClientPerson",
            Self::ClientsEntity => "ClientEntity",
            Self::Groups => "Groups",
            Self::Centers => "Centers",
            Self::Offices => "Offices",
            Self::Staff => "Staff",
            Self::Users => "Users",
            Self::ChartOfAccounts => "ChartOfAccounts",
            Self::GlJournalEntries => "AddJournalEntries",
            Self::Loans => "Loans",
            Self::LoanTransactions => "LoanRepayment",
            Self::SavingsAccount => "SavingsAccounts",
            Self::SavingsTransactions => "SavingsTransaction",
            Self::FixedDepositAccounts => "FixedDeposit",
            Self::RecurringDepositAccounts => "RecurringDeposit",
            Self::ShareAccounts => "SharedAccounts",
            Self::Guarantors => "guarantor",
        }
    }
}

impl fmt::Display for GlobalEntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Persistent tracking entity for a bulk import job (`m_import_document`).
///
/// Ported from `org.apache.fineract.infrastructure.bulkimport.domain.ImportDocument`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDocument {
    pub id: i64,
    pub document_id: i64,
    pub import_time: NaiveDateTime,
    pub end_time: Option<NaiveDateTime>,
    pub entity_type: GlobalEntityType,
    pub total_records: i32,
    pub success_count: i32,
    pub failure_count: i32,
    pub completed: bool,
}

impl ImportDocument {
    /// Creates a new in-progress [`ImportDocument`] matching `ImportDocument.instance(...)`.
    pub fn instance(
        id: i64,
        document_id: i64,
        import_time: NaiveDateTime,
        entity_type: GlobalEntityType,
        total_records: i32,
    ) -> Self {
        Self {
            id,
            document_id,
            import_time,
            end_time: None,
            entity_type,
            total_records,
            success_count: 0,
            failure_count: 0,
            completed: false,
        }
    }

    /// Marks the import document completed with final `end_time`, `success_count`, and `failure_count`.
    ///
    /// Ported from `ImportDocument.update(final LocalDateTime endTime, final Integer successCount, final Integer errorCount)`.
    pub fn update(&mut self, end_time: NaiveDateTime, success_count: i32, failure_count: i32) {
        self.end_time = Some(end_time);
        self.completed = true;
        self.success_count = success_count;
        self.failure_count = failure_count;
    }
}

/// Status cell values emitted by Fineract import handlers (`TemplatePopulateImportConstants`).
pub mod bulk_import_status {
    pub const STATUS_CELL_IMPORTED: &str = "Imported";
    pub const STATUS_CREATION_FAILED: &str = "Creation failed";
    pub const STATUS_APPROVAL_FAILED: &str = "Approval failed";
    pub const STATUS_ACTIVATION_FAILED: &str = "Activation failed";
    pub const STATUS_DISBURSAL_FAILED: &str = "Disbursal failed";
    pub const STATUS_DISBURSAL_REPAYMENT_FAILED: &str = "Repayment failed";
    pub const STATUS_VALIDATION_FAILED: &str = "Validation failed";
}

/// Parsed and validated row payload for the five primary bulk-import handlers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BulkImportRowPayload {
    ClientPerson {
        first_name: String,
        last_name: String,
        middle_name: Option<String>,
        office_name: String,
        staff_name: Option<String>,
        external_id: Option<String>,
        submitted_on: NaiveDate,
        activation_date: Option<NaiveDate>,
        active: bool,
        mobile_no: Option<String>,
    },
    Loan {
        office_name: String,
        loan_type: String,
        client_name: String,
        product_name: String,
        principal: Decimal,
        number_of_repayments: i32,
        repaid_every: i32,
        repaid_every_frequency: String,
        loan_term: i32,
        loan_term_frequency: String,
        nominal_interest_rate: Decimal,
        submitted_on_date: NaiveDate,
        approved_date: Option<NaiveDate>,
        disbursed_date: Option<NaiveDate>,
        total_amount_repaid: Option<Decimal>,
    },
    LoanRepayment {
        office_name: String,
        client_name: String,
        loan_account_no: String,
        amount: Decimal,
        repaid_on_date: NaiveDate,
        repayment_type: String,
    },
    SavingsDeposit {
        office_name: String,
        client_name: String,
        savings_account_no: String,
        transaction_type: String,
        amount: Decimal,
        transaction_date: NaiveDate,
        payment_type: String,
    },
    GlJournalEntry {
        office_name: String,
        transaction_date: NaiveDate,
        currency_code: String,
        transaction_id: String,
        gl_account_debit: Option<String>,
        amount_debit: Option<Decimal>,
        gl_account_credit: Option<String>,
        amount_credit: Option<Decimal>,
        comments: Option<String>,
    },
    Generic {
        columns: HashMap<String, String>,
    },
}

/// Dispatched command produced by [`CsvBulkImportProcessor`] for a valid import row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkImportCommandDispatch {
    pub row_index: usize,
    pub entity_name: String,
    pub action_name: String,
    pub payload: BulkImportRowPayload,
    pub created_resource_id: i64,
}

/// Per-row outcome emitted by [`CsvBulkImportProcessor`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkImportRowOutcome {
    pub row_index: usize,
    pub status: String,
    pub error: Option<String>,
    pub resource_id: Option<i64>,
}

/// Complete result of running [`CsvBulkImportProcessor`] on a CSV/tabular input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkImportExecutionResult {
    pub import_document: ImportDocument,
    pub row_outcomes: Vec<BulkImportRowOutcome>,
    pub dispatched_commands: Vec<BulkImportCommandDispatch>,
    pub annotated_csv_report: String,
}

/// Tabular CSV/Workbook Bulk Import processor ported from `BulkImportWorkbookServiceImpl`,
/// `ClientPersonImportHandler`, `LoanImportHandler`, `LoanRepaymentImportHandler`,
/// `SavingsTransactionImportHandler`, and `JournalEntriesImportHandler`.
#[derive(Debug, Clone, Default)]
pub struct CsvBulkImportProcessor {
    pub default_date_format: String,
}

impl CsvBulkImportProcessor {
    /// Creates a new [`CsvBulkImportProcessor`] with ISO `"%Y-%m-%d"` default date parsing.
    pub fn new() -> Self {
        Self {
            default_date_format: "%Y-%m-%d".to_string(),
        }
    }

    /// Processes a CSV document for the specified [`GlobalEntityType`], validating each row,
    /// checking GL Journal Entry batch balance when `entity_type == GlobalEntityType::GlJournalEntries`,
    /// dispatching commands via `dispatcher`, and returning an annotated CSV with `Status` and `Error` columns.
    pub fn process_csv_with_dispatcher<F>(
        &self,
        document_id: i64,
        entity_type: GlobalEntityType,
        import_time: NaiveDateTime,
        end_time: NaiveDateTime,
        csv_input: &str,
        mut dispatcher: F,
    ) -> BulkImportExecutionResult
    where
        F: FnMut(GlobalEntityType, usize, &BulkImportRowPayload) -> Result<i64, String>,
    {
        let lines: Vec<&str> = csv_input
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.trim().is_empty())
            .collect();

        if lines.is_empty() {
            let mut doc = ImportDocument::instance(1, document_id, import_time, entity_type, 0);
            doc.update(end_time, 0, 0);
            return BulkImportExecutionResult {
                import_document: doc,
                row_outcomes: Vec::new(),
                dispatched_commands: Vec::new(),
                annotated_csv_report: "Status,Error\n".to_string(),
            };
        }

        let headers = parse_csv_row(lines[0]);
        let status_col_idx = headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case("status"));
        let error_col_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("error"));

        let data_rows: Vec<(usize, Vec<String>)> = lines[1..]
            .iter()
            .enumerate()
            .map(|(idx, line)| (idx + 1, parse_csv_row(line)))
            .collect();

        // Pre-compute imbalanced transaction IDs for GL Journal Entries (debits != credits)
        let imbalanced_gl_txns = if entity_type == GlobalEntityType::GlJournalEntries {
            self.find_imbalanced_gl_transactions(&headers, &data_rows)
        } else {
            HashMap::new()
        };

        let mut import_doc = ImportDocument::instance(
            1,
            document_id,
            import_time,
            entity_type,
            data_rows.len() as i32,
        );

        let mut out_headers = headers.clone();
        if status_col_idx.is_none() {
            out_headers.push("Status".to_string());
        }
        if error_col_idx.is_none() {
            out_headers.push("Error".to_string());
        }

        let mut annotated_lines = Vec::with_capacity(data_rows.len() + 1);
        annotated_lines.push(format_csv_row(&out_headers));

        let mut success_count = 0i32;
        let mut failure_count = 0i32;
        let mut row_outcomes = Vec::with_capacity(data_rows.len());
        let mut dispatched_commands = Vec::new();

        for (row_index, cells) in data_rows {
            let row_map = build_row_map(&headers, &cells);

            // Idempotency guard matching `ImportHandlerUtils.isNotImported(row, STATUS_COL)`
            if let Some(existing_status) = row_map.get("status") {
                if existing_status.eq_ignore_ascii_case(bulk_import_status::STATUS_CELL_IMPORTED) {
                    let mut out_cells = cells.clone();
                    append_status_and_error(
                        &mut out_cells,
                        status_col_idx,
                        error_col_idx,
                        headers.len(),
                        bulk_import_status::STATUS_CELL_IMPORTED,
                        "",
                    );
                    annotated_lines.push(format_csv_row(&out_cells));
                    row_outcomes.push(BulkImportRowOutcome {
                        row_index,
                        status: bulk_import_status::STATUS_CELL_IMPORTED.to_string(),
                        error: None,
                        resource_id: None,
                    });
                    continue;
                }
            }

            let parse_result =
                self.parse_and_validate_row(entity_type, &row_map, &imbalanced_gl_txns);

            match parse_result {
                Ok((entity_name, action_name, payload)) => {
                    match dispatcher(entity_type, row_index, &payload) {
                        Ok(resource_id) => {
                            success_count += 1;
                            let mut out_cells = cells.clone();
                            append_status_and_error(
                                &mut out_cells,
                                status_col_idx,
                                error_col_idx,
                                headers.len(),
                                bulk_import_status::STATUS_CELL_IMPORTED,
                                "",
                            );
                            annotated_lines.push(format_csv_row(&out_cells));
                            row_outcomes.push(BulkImportRowOutcome {
                                row_index,
                                status: bulk_import_status::STATUS_CELL_IMPORTED.to_string(),
                                error: None,
                                resource_id: Some(resource_id),
                            });
                            dispatched_commands.push(BulkImportCommandDispatch {
                                row_index,
                                entity_name,
                                action_name,
                                payload,
                                created_resource_id: resource_id,
                            });
                        }
                        Err(err_msg) => {
                            failure_count += 1;
                            let failure_status = classify_failure_status(entity_type, &err_msg);
                            let mut out_cells = cells.clone();
                            append_status_and_error(
                                &mut out_cells,
                                status_col_idx,
                                error_col_idx,
                                headers.len(),
                                failure_status,
                                &err_msg,
                            );
                            annotated_lines.push(format_csv_row(&out_cells));
                            row_outcomes.push(BulkImportRowOutcome {
                                row_index,
                                status: failure_status.to_string(),
                                error: Some(err_msg),
                                resource_id: None,
                            });
                        }
                    }
                }
                Err(err_msg) => {
                    failure_count += 1;
                    let failure_status = classify_failure_status(entity_type, &err_msg);
                    let mut out_cells = cells.clone();
                    append_status_and_error(
                        &mut out_cells,
                        status_col_idx,
                        error_col_idx,
                        headers.len(),
                        failure_status,
                        &err_msg,
                    );
                    annotated_lines.push(format_csv_row(&out_cells));
                    row_outcomes.push(BulkImportRowOutcome {
                        row_index,
                        status: failure_status.to_string(),
                        error: Some(err_msg),
                        resource_id: None,
                    });
                }
            }
        }

        import_doc.update(end_time, success_count, failure_count);
        let mut annotated_csv_report = annotated_lines.join("\n");
        annotated_csv_report.push('\n');

        BulkImportExecutionResult {
            import_document: import_doc,
            row_outcomes,
            dispatched_commands,
            annotated_csv_report,
        }
    }

    /// Processes a CSV document with the default command dispatcher (assigning sequential resource IDs `1000 + row_index`).
    pub fn process_csv(
        &self,
        document_id: i64,
        entity_type: GlobalEntityType,
        import_time: NaiveDateTime,
        end_time: NaiveDateTime,
        csv_input: &str,
    ) -> BulkImportExecutionResult {
        self.process_csv_with_dispatcher(
            document_id,
            entity_type,
            import_time,
            end_time,
            csv_input,
            |_etype, row_idx, _payload| Ok(1000 + row_idx as i64),
        )
    }

    fn find_imbalanced_gl_transactions(
        &self,
        headers: &[String],
        data_rows: &[(usize, Vec<String>)],
    ) -> HashMap<String, String> {
        let mut totals: HashMap<String, (Decimal, Decimal)> = HashMap::new();
        for (_idx, cells) in data_rows {
            let map = build_row_map(headers, cells);
            let txn_id = get_field(&map, &["transaction_id", "transactionid"])
                .unwrap_or_default()
                .to_string();
            if txn_id.is_empty() {
                continue;
            }
            let debit = get_field(&map, &["amount_debit", "debit_amount", "debit"])
                .and_then(|s| s.parse::<Decimal>().ok())
                .unwrap_or(Decimal::ZERO);
            let credit = get_field(&map, &["amount_credit", "credit_amount", "credit"])
                .and_then(|s| s.parse::<Decimal>().ok())
                .unwrap_or(Decimal::ZERO);
            let entry = totals.entry(txn_id).or_insert((Decimal::ZERO, Decimal::ZERO));
            entry.0 += debit;
            entry.1 += credit;
        }

        let mut imbalanced = HashMap::new();
        for (txn_id, (total_debit, total_credit)) in totals {
            if total_debit != total_credit {
                imbalanced.insert(
                    txn_id.clone(),
                    format!(
                        "GL journal transaction `{txn_id}` is unbalanced: total debit {total_debit} != total credit {total_credit}"
                    ),
                );
            }
        }
        imbalanced
    }

    fn parse_and_validate_row(
        &self,
        entity_type: GlobalEntityType,
        row: &HashMap<String, String>,
        imbalanced_gl_txns: &HashMap<String, String>,
    ) -> Result<(String, String, BulkImportRowPayload), String> {
        match entity_type {
            GlobalEntityType::ClientsPerson => {
                let first_name = require_non_empty(row, &["first_name", "firstname"], "firstName")?;
                let last_name = require_non_empty(row, &["last_name", "lastname"], "lastName")?;
                let office_name =
                    require_non_empty(row, &["office_name", "officename", "office"], "officeName")?;
                let submitted_on = parse_required_date(
                    row,
                    &["submitted_on", "submittedon", "submitted_on_date"],
                    "submittedOnDate",
                    &self.default_date_format,
                )?;
                let active = get_field(row, &["active"])
                    .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
                    .unwrap_or(true);
                let activation_date = if active {
                    Some(
                        parse_optional_date(
                            row,
                            &["activation_date", "activationdate"],
                            &self.default_date_format,
                        )?
                        .unwrap_or(submitted_on),
                    )
                } else {
                    Some(submitted_on)
                };
                if let Some(act_date) = activation_date {
                    if act_date < submitted_on {
                        return Err(
                            "activationDate cannot be before submittedOnDate".to_string()
                        );
                    }
                }
                Ok((
                    "CLIENT".to_string(),
                    "CREATE".to_string(),
                    BulkImportRowPayload::ClientPerson {
                        first_name,
                        last_name,
                        middle_name: get_field(row, &["middle_name", "middlename"])
                            .map(str::to_string),
                        office_name,
                        staff_name: get_field(row, &["staff_name", "staffname"]).map(str::to_string),
                        external_id: get_field(row, &["external_id", "externalid"])
                            .map(str::to_string),
                        submitted_on,
                        activation_date,
                        active,
                        mobile_no: get_field(row, &["mobile_no", "mobileno"]).map(str::to_string),
                    },
                ))
            }
            GlobalEntityType::Loans => {
                let office_name =
                    require_non_empty(row, &["office_name", "officename", "office"], "officeName")?;
                let loan_type = get_field(row, &["loan_type", "loantype"])
                    .unwrap_or("individual")
                    .to_ascii_lowercase();
                if !matches!(loan_type.as_str(), "individual" | "jlg" | "group") {
                    return Err(format!("Unsupported loanType `{loan_type}`"));
                }
                let client_name = require_non_empty(
                    row,
                    &["client_name", "clientname", "group_name"],
                    "clientOrGroupName",
                )?;
                let product_name =
                    require_non_empty(row, &["product_name", "product", "productname"], "product")?;
                let principal = parse_positive_decimal(row, &["principal"], "principal")?;
                let number_of_repayments = parse_positive_i32(
                    row,
                    &["number_of_repayments", "no_of_repayments", "repayments"],
                    "numberOfRepayments",
                )?;
                let repaid_every =
                    parse_positive_i32(row, &["repaid_every", "repaidevery"], "repaidEvery")
                        .unwrap_or(1);
                let repaid_every_frequency =
                    get_field(row, &["repaid_every_frequency", "frequency"])
                        .unwrap_or("Months")
                        .to_string();
                let loan_term = parse_positive_i32(row, &["loan_term", "loanterm"], "loanTerm")
                    .unwrap_or(number_of_repayments * repaid_every);
                let loan_term_frequency = get_field(row, &["loan_term_frequency"])
                    .unwrap_or(&repaid_every_frequency)
                    .to_string();
                let nominal_interest_rate = parse_non_negative_decimal(
                    row,
                    &["nominal_interest_rate", "interest_rate"],
                    "nominalInterestRate",
                )?;
                let submitted_on_date = parse_required_date(
                    row,
                    &["submitted_on_date", "submitted_on", "submittedondate"],
                    "submittedOnDate",
                    &self.default_date_format,
                )?;
                let approved_date = parse_optional_date(
                    row,
                    &["approved_date", "approveddate"],
                    &self.default_date_format,
                )?;
                let disbursed_date = parse_optional_date(
                    row,
                    &["disbursed_date", "disburseddate"],
                    &self.default_date_format,
                )?;
                if let Some(app_date) = approved_date {
                    if app_date < submitted_on_date {
                        return Err(
                            "Approval failed: approvedDate cannot be before submittedOnDate"
                                .to_string(),
                        );
                    }
                }
                if let (Some(app_date), Some(disb_date)) = (approved_date, disbursed_date) {
                    if disb_date < app_date {
                        return Err(
                            "Disbursal failed: disbursedDate cannot be before approvedDate"
                                .to_string(),
                        );
                    }
                }
                let total_amount_repaid =
                    get_field(row, &["total_amount_repaid", "amount_repaid"])
                        .map(|s| {
                            s.parse::<Decimal>().map_err(|_| {
                                "Repayment failed: invalid totalAmountRepaid".to_string()
                            })
                        })
                        .transpose()?;

                Ok((
                    "LOAN".to_string(),
                    "CREATE".to_string(),
                    BulkImportRowPayload::Loan {
                        office_name,
                        loan_type,
                        client_name,
                        product_name,
                        principal,
                        number_of_repayments,
                        repaid_every,
                        repaid_every_frequency,
                        loan_term,
                        loan_term_frequency,
                        nominal_interest_rate,
                        submitted_on_date,
                        approved_date,
                        disbursed_date,
                        total_amount_repaid,
                    },
                ))
            }
            GlobalEntityType::LoanTransactions => {
                let office_name = get_field(row, &["office_name", "office"])
                    .unwrap_or("Head Office")
                    .to_string();
                let client_name = get_field(row, &["client_name", "client"])
                    .unwrap_or("Unknown")
                    .to_string();
                let loan_account_no = require_non_empty(
                    row,
                    &["loan_account_no", "loanaccountno", "account_no"],
                    "loanAccountNo",
                )?;
                let amount =
                    parse_positive_decimal(row, &["amount", "repayment_amount"], "transactionAmount")?;
                let repaid_on_date = parse_required_date(
                    row,
                    &["repaid_on_date", "transaction_date", "repayment_date"],
                    "transactionDate",
                    &self.default_date_format,
                )?;
                let repayment_type = get_field(row, &["repayment_type", "payment_type"])
                    .unwrap_or("Cash")
                    .to_string();

                Ok((
                    "LOAN".to_string(),
                    "REPAYMENT".to_string(),
                    BulkImportRowPayload::LoanRepayment {
                        office_name,
                        client_name,
                        loan_account_no,
                        amount,
                        repaid_on_date,
                        repayment_type,
                    },
                ))
            }
            GlobalEntityType::SavingsTransactions => {
                let office_name = get_field(row, &["office_name", "office"])
                    .unwrap_or("Head Office")
                    .to_string();
                let client_name = get_field(row, &["client_name", "client"])
                    .unwrap_or("Unknown")
                    .to_string();
                let savings_account_no = require_non_empty(
                    row,
                    &["savings_account_no", "savingsaccountno", "account_no"],
                    "savingsAccountNo",
                )?;
                let transaction_type = get_field(row, &["transaction_type", "type"])
                    .unwrap_or("Deposit")
                    .to_string();
                if !transaction_type.eq_ignore_ascii_case("Deposit")
                    && !transaction_type.eq_ignore_ascii_case("Withdrawal")
                {
                    return Err(format!(
                        "Unsupported savings transactionType `{transaction_type}`; expected Deposit or Withdrawal"
                    ));
                }
                let amount = parse_positive_decimal(row, &["amount"], "transactionAmount")?;
                let transaction_date = parse_required_date(
                    row,
                    &["transaction_date", "date"],
                    "transactionDate",
                    &self.default_date_format,
                )?;
                let payment_type = get_field(row, &["payment_type", "paymenttype"])
                    .unwrap_or("Cash")
                    .to_string();
                let action_name = if transaction_type.eq_ignore_ascii_case("Withdrawal") {
                    "WITHDRAWAL".to_string()
                } else {
                    "DEPOSIT".to_string()
                };

                Ok((
                    "SAVINGSACCOUNT".to_string(),
                    action_name,
                    BulkImportRowPayload::SavingsDeposit {
                        office_name,
                        client_name,
                        savings_account_no,
                        transaction_type,
                        amount,
                        transaction_date,
                        payment_type,
                    },
                ))
            }
            GlobalEntityType::GlJournalEntries => {
                let office_name =
                    require_non_empty(row, &["office_name", "office"], "officeName")?;
                let transaction_date = parse_required_date(
                    row,
                    &["transaction_date", "transaction_on_date", "date"],
                    "transactionDate",
                    &self.default_date_format,
                )?;
                let currency_code =
                    require_non_empty(row, &["currency_code", "currency"], "currencyCode")?;
                let transaction_id =
                    require_non_empty(row, &["transaction_id", "transactionid"], "transactionId")?;
                if let Some(imbalance_err) = imbalanced_gl_txns.get(&transaction_id) {
                    return Err(imbalance_err.clone());
                }
                let gl_account_debit = get_field(row, &["gl_account_debit", "debit_gl_account"])
                    .map(str::to_string);
                let amount_debit = get_field(row, &["amount_debit", "debit_amount", "debit"])
                    .map(|s| {
                        s.parse::<Decimal>()
                            .map_err(|_| "Invalid debit amount".to_string())
                    })
                    .transpose()?;
                let gl_account_credit = get_field(row, &["gl_account_credit", "credit_gl_account"])
                    .map(str::to_string);
                let amount_credit = get_field(row, &["amount_credit", "credit_amount", "credit"])
                    .map(|s| {
                        s.parse::<Decimal>()
                            .map_err(|_| "Invalid credit amount".to_string())
                    })
                    .transpose()?;

                if gl_account_debit.is_none() && gl_account_credit.is_none() {
                    return Err(
                        "Journal entry row must specify at least a debit or credit GL account"
                            .to_string(),
                    );
                }
                if let Some(d) = amount_debit {
                    if d <= Decimal::ZERO {
                        return Err("Debit amount must be strictly positive".to_string());
                    }
                }
                if let Some(c) = amount_credit {
                    if c <= Decimal::ZERO {
                        return Err("Credit amount must be strictly positive".to_string());
                    }
                }

                Ok((
                    "JOURNALENTRY".to_string(),
                    "CREATE".to_string(),
                    BulkImportRowPayload::GlJournalEntry {
                        office_name,
                        transaction_date,
                        currency_code,
                        transaction_id,
                        gl_account_debit,
                        amount_debit,
                        gl_account_credit,
                        amount_credit,
                        comments: get_field(row, &["comments", "note"]).map(str::to_string),
                    },
                ))
            }
            other => {
                if row.values().all(|v| v.trim().is_empty()) {
                    return Err("Row is completely empty".to_string());
                }
                Ok((
                    other.code().to_ascii_uppercase(),
                    "IMPORT".to_string(),
                    BulkImportRowPayload::Generic {
                        columns: row.clone(),
                    },
                ))
            }
        }
    }
}

fn classify_failure_status(entity_type: GlobalEntityType, err_msg: &str) -> &'static str {
    if entity_type == GlobalEntityType::Loans {
        if err_msg.starts_with("Approval failed") {
            return bulk_import_status::STATUS_APPROVAL_FAILED;
        }
        if err_msg.starts_with("Disbursal failed") {
            return bulk_import_status::STATUS_DISBURSAL_FAILED;
        }
        if err_msg.starts_with("Repayment failed") {
            return bulk_import_status::STATUS_DISBURSAL_REPAYMENT_FAILED;
        }
    }
    bulk_import_status::STATUS_CREATION_FAILED
}

fn parse_csv_row(line: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                result.push(current.trim().to_string());
                current.clear();
            }
            other => current.push(other),
        }
    }
    result.push(current.trim().to_string());
    result
}

fn format_csv_row(cells: &[String]) -> String {
    cells
        .iter()
        .map(|cell| {
            if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                format!("\"{}\"", cell.replace('"', "\"\""))
            } else {
                cell.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn build_row_map(headers: &[String], cells: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::with_capacity(headers.len());
    for (idx, header) in headers.iter().enumerate() {
        let normalized = header
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-'], "_");
        let val = cells.get(idx).cloned().unwrap_or_default();
        map.insert(normalized, val);
    }
    map
}

fn append_status_and_error(
    cells: &mut Vec<String>,
    status_col_idx: Option<usize>,
    error_col_idx: Option<usize>,
    header_len: usize,
    status: &str,
    error: &str,
) {
    while cells.len() < header_len {
        cells.push(String::new());
    }
    match status_col_idx {
        Some(idx) => cells[idx] = status.to_string(),
        None => cells.push(status.to_string()),
    }
    match error_col_idx {
        Some(idx) => cells[idx] = error.to_string(),
        None => cells.push(error.to_string()),
    }
}

fn get_field<'a>(row: &'a HashMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(v) = row.get(*k) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn require_non_empty(
    row: &HashMap<String, String>,
    keys: &[&str],
    field_label: &str,
) -> Result<String, String> {
    get_field(row, keys)
        .map(str::to_string)
        .ok_or_else(|| format!("Missing mandatory field `{field_label}`"))
}

fn parse_required_date(
    row: &HashMap<String, String>,
    keys: &[&str],
    field_label: &str,
    fmt: &str,
) -> Result<NaiveDate, String> {
    let raw = require_non_empty(row, keys, field_label)?;
    NaiveDate::parse_from_str(&raw, fmt)
        .or_else(|_| NaiveDate::parse_from_str(&raw, "%d %B %Y"))
        .or_else(|_| NaiveDate::parse_from_str(&raw, "%d/%m/%Y"))
        .map_err(|_| format!("Invalid date `{raw}` for field `{field_label}`"))
}

fn parse_optional_date(
    row: &HashMap<String, String>,
    keys: &[&str],
    fmt: &str,
) -> Result<Option<NaiveDate>, String> {
    match get_field(row, keys) {
        Some(raw) => NaiveDate::parse_from_str(raw, fmt)
            .or_else(|_| NaiveDate::parse_from_str(raw, "%d %B %Y"))
            .or_else(|_| NaiveDate::parse_from_str(raw, "%d/%m/%Y"))
            .map(Some)
            .map_err(|_| format!("Invalid date `{raw}`")),
        None => Ok(None),
    }
}

fn parse_positive_decimal(
    row: &HashMap<String, String>,
    keys: &[&str],
    field_label: &str,
) -> Result<Decimal, String> {
    let raw = require_non_empty(row, keys, field_label)?;
    let val = raw
        .parse::<Decimal>()
        .map_err(|_| format!("Invalid decimal `{raw}` for field `{field_label}`"))?;
    if val <= Decimal::ZERO {
        return Err(format!("Field `{field_label}` must be greater than zero"));
    }
    Ok(val)
}

fn parse_non_negative_decimal(
    row: &HashMap<String, String>,
    keys: &[&str],
    field_label: &str,
) -> Result<Decimal, String> {
    let raw = require_non_empty(row, keys, field_label)?;
    let val = raw
        .parse::<Decimal>()
        .map_err(|_| format!("Invalid decimal `{raw}` for field `{field_label}`"))?;
    if val < Decimal::ZERO {
        return Err(format!("Field `{field_label}` cannot be negative"));
    }
    Ok(val)
}

fn parse_positive_i32(
    row: &HashMap<String, String>,
    keys: &[&str],
    field_label: &str,
) -> Result<i32, String> {
    let raw = require_non_empty(row, keys, field_label)?;
    let val = raw
        .parse::<i32>()
        .map_err(|_| format!("Invalid integer `{raw}` for field `{field_label}`"))?;
    if val <= 0 {
        return Err(format!("Field `{field_label}` must be greater than zero"));
    }
    Ok(val)
}

// ============================================================================
// 2. Webhook & Event Hook Engine (`m_hook`, `m_hook_templates`, `m_hook_registered_events`, `m_hook_configuration`)
// ============================================================================

/// Hook template type (`m_hook_templates`).
///
/// Ported from `org.apache.fineract.infrastructure.hooks.domain.HookTemplate` and `HookApiConstants`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i64)]
pub enum HookTemplate {
    Web = 1,
    Sms = 2,
}

impl HookTemplate {
    /// Returns the template ID in `m_hook_templates.id`.
    pub const fn id(self) -> i64 {
        self as i64
    }

    /// Returns the canonical template name (`"Web"` or `"SMS Bridge"`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Web => "Web",
            Self::Sms => "SMS Bridge",
        }
    }

    /// Resolves a [`HookTemplate`] from its `template_id`.
    pub const fn from_id(id: i64) -> Option<Self> {
        match id {
            1 => Some(Self::Web),
            2 => Some(Self::Sms),
            _ => None,
        }
    }
}

/// Event subscription attached to a [`Hook`] (`m_hook_registered_events`).
///
/// Ported from `org.apache.fineract.infrastructure.hooks.domain.HookResource`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HookRegisteredEvent {
    pub entity_name: String,
    pub action_name: String,
}

impl HookRegisteredEvent {
    /// Creates a new [`HookRegisteredEvent`].
    pub fn new(entity_name: impl Into<String>, action_name: impl Into<String>) -> Self {
        Self {
            entity_name: entity_name.into(),
            action_name: action_name.into(),
        }
    }

    /// Checks whether this registered event matches an incoming `(entity_name, action_name)` pair
    /// (supporting case-insensitive comparison and `"*"` wildcard).
    pub fn matches(&self, entity_name: &str, action_name: &str) -> bool {
        let entity_ok =
            self.entity_name == "*" || self.entity_name.eq_ignore_ascii_case(entity_name);
        let action_ok =
            self.action_name == "*" || self.action_name.eq_ignore_ascii_case(action_name);
        entity_ok && action_ok
    }
}

/// Key-value configuration entry for a [`Hook`] (`m_hook_configuration`).
///
/// Ported from `org.apache.fineract.infrastructure.hooks.domain.HookConfiguration`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookConfiguration {
    pub id: Option<i64>,
    pub hook_id: i64,
    pub field_type: String,
    pub field_name: String,
    pub field_value: String,
}

impl HookConfiguration {
    /// Creates a new [`HookConfiguration`] entry.
    pub fn new(
        hook_id: i64,
        field_type: impl Into<String>,
        field_name: impl Into<String>,
        field_value: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            hook_id,
            field_type: field_type.into(),
            field_name: field_name.into(),
            field_value: field_value.into(),
        }
    }
}

/// Configured webhook or SMS bridge hook (`m_hook`).
///
/// Ported from `org.apache.fineract.infrastructure.hooks.domain.Hook`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hook {
    pub id: i64,
    pub template_id: i64,
    pub name: String,
    pub is_active: bool,
    pub events: Vec<HookRegisteredEvent>,
    pub config: HashMap<String, String>,
}

impl Hook {
    /// Returns the resolved [`HookTemplate`] for `self.template_id`.
    pub fn template(&self) -> Option<HookTemplate> {
        HookTemplate::from_id(self.template_id)
    }

    /// Returns `true` if the hook is active and subscribes to `(entity_name, action_name)`.
    pub fn subscribes_to(&self, entity_name: &str, action_name: &str) -> bool {
        self.is_active
            && self
                .events
                .iter()
                .any(|ev| ev.matches(entity_name, action_name))
    }

    /// Looks up the configured `Payload URL` (`"Payload URL"` or `"payloadURL"`).
    pub fn payload_url(&self) -> Option<&str> {
        self.config
            .get("Payload URL")
            .or_else(|| self.config.get("payloadURL"))
            .map(String::as_str)
    }

    /// Looks up the configured `Content Type` (defaults to `"json"`).
    pub fn content_type(&self) -> &str {
        self.config
            .get("Content Type")
            .or_else(|| self.config.get("contentType"))
            .map(String::as_str)
            .unwrap_or("json")
    }

    /// Looks up an optional webhook signing secret (`"Secret"` or `"secret"`).
    pub fn secret(&self) -> Option<&str> {
        self.config
            .get("Secret")
            .or_else(|| self.config.get("secret"))
            .map(String::as_str)
    }
}

/// Outbound HTTP request envelope prepared by [`WebhookDispatcher`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchedWebhookRequest {
    pub hook_id: i64,
    pub hook_name: String,
    pub template: HookTemplate,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// Event hook router and HMAC-SHA256 signer ported from `FineractHookListener`, `WebHookProcessor`,
/// and `WebHookService`.
#[derive(Debug, Clone, Default)]
pub struct WebhookDispatcher {
    hooks: Vec<Hook>,
    default_signing_secret: String,
    fineract_endpoint_url: String,
}

impl WebhookDispatcher {
    pub const HEADER_SIGNATURE: &'static str = "X-Fineract-Signature";
    pub const HEADER_ENTITY: &'static str = "X-Fineract-Entity";
    pub const HEADER_ACTION: &'static str = "X-Fineract-Action";
    pub const HEADER_TENANT: &'static str = "Fineract-Platform-TenantId";
    pub const HEADER_ENDPOINT: &'static str = "X-Fineract-Endpoint";
    pub const HEADER_API_KEY: &'static str = "X-Fineract-API-Key";

    /// Creates a new [`WebhookDispatcher`] with a default HMAC-SHA256 signing secret.
    pub fn new(default_signing_secret: impl Into<String>) -> Self {
        Self {
            hooks: Vec::new(),
            default_signing_secret: default_signing_secret.into(),
            fineract_endpoint_url: "https://localhost:8443/fineract-provider/api/v1/".to_string(),
        }
    }

    /// Registers a [`Hook`] with the dispatcher.
    pub fn register_hook(&mut self, hook: Hook) {
        self.hooks.push(hook);
    }

    /// Computes a lowercase hexadecimal RFC 2104 HMAC-SHA256 digest of `payload` under `secret`.
    pub fn compute_hmac_sha256_hex(secret: &[u8], payload: &[u8]) -> String {
        let mac = hmac_sha256(secret, payload);
        bytes_to_hex(&mac)
    }

    /// Formats the `X-Fineract-Signature` header value (`sha256=<hex>`).
    pub fn format_signature_header(secret: &str, payload: &str) -> String {
        format!(
            "sha256={}",
            Self::compute_hmac_sha256_hex(secret.as_bytes(), payload.as_bytes())
        )
    }

    /// Constant-time verification of an `X-Fineract-Signature` header value against `secret` and `payload`.
    pub fn verify_signature_header(secret: &str, payload: &str, header_value: &str) -> bool {
        let expected = Self::format_signature_header(secret, payload);
        constant_time_eq(expected.as_bytes(), header_value.as_bytes())
    }

    /// Dispatches a domain event `(entity_name, action_name)` for `tenant_id` to all active hooks
    /// subscribed to that event, producing signed [`DispatchedWebhookRequest`] envelopes.
    pub fn dispatch_event(
        &self,
        tenant_id: &str,
        entity_name: &str,
        action_name: &str,
        payload_json: &str,
    ) -> Vec<DispatchedWebhookRequest> {
        let mut dispatched = Vec::new();
        for hook in &self.hooks {
            if !hook.subscribes_to(entity_name, action_name) {
                continue;
            }
            let Some(url) = hook.payload_url() else {
                continue;
            };
            let template = hook.template().unwrap_or(HookTemplate::Web);
            let secret = hook.secret().unwrap_or(&self.default_signing_secret);
            let signature = Self::format_signature_header(secret, payload_json);

            let mut headers = HashMap::new();
            headers.insert(Self::HEADER_ENTITY.to_string(), entity_name.to_string());
            headers.insert(Self::HEADER_ACTION.to_string(), action_name.to_string());
            headers.insert(Self::HEADER_TENANT.to_string(), tenant_id.to_string());
            headers.insert(Self::HEADER_SIGNATURE.to_string(), signature);

            let content_type = if hook.content_type().eq_ignore_ascii_case("form") {
                "application/x-www-form-urlencoded"
            } else {
                "application/json"
            };
            headers.insert("Content-Type".to_string(), content_type.to_string());

            match template {
                HookTemplate::Web => {
                    headers.insert(
                        Self::HEADER_ENDPOINT.to_string(),
                        self.fineract_endpoint_url.clone(),
                    );
                }
                HookTemplate::Sms => {
                    if let Some(api_key) = hook
                        .config
                        .get("SMS Provider Token")
                        .or_else(|| hook.config.get("apiKey"))
                    {
                        headers.insert(Self::HEADER_API_KEY.to_string(), api_key.clone());
                    }
                }
            }

            dispatched.push(DispatchedWebhookRequest {
                hook_id: hook.id,
                hook_name: hook.name.clone(),
                template,
                url: url.to_string(),
                headers,
                body: payload_json.to_string(),
            });
        }
        dispatched
    }
}

// --- Pure-Rust FIPS 180-4 SHA-256 & RFC 2104 HMAC-SHA256 ---

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
    0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
    0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
    0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
    0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
    0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2,
];

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity((data.len() + 72) & !63);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k_block = [0u8; 64];
    if key.len() > 64 {
        let hashed = sha256(key);
        k_block[..32].copy_from_slice(&hashed);
    } else {
        k_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k_block[i];
        opad[i] ^= k_block[i];
    }

    let mut inner_input = Vec::with_capacity(64 + message.len());
    inner_input.extend_from_slice(&ipad);
    inner_input.extend_from_slice(message);
    let inner_hash = sha256(&inner_input);

    let mut outer_input = Vec::with_capacity(64 + 32);
    outer_input.extend_from_slice(&opad);
    outer_input.extend_from_slice(&inner_hash);
    sha256(&outer_input)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ============================================================================
// 3. SMS & Email Campaigns (`sms_campaign`, `sms_messages_outbound`, `scheduled_email_campaign`)
// ============================================================================

/// Trigger type of an SMS or Email campaign (`sms_campaign.campaign_trigger_type`).
///
/// Ported from `org.apache.fineract.infrastructure.campaigns.sms.constants.SmsCampaignTriggerType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum CampaignTriggerType {
    Direct = 1,
    Scheduled = 2,
    Triggered = 3,
}

impl CampaignTriggerType {
    /// Returns the integer discriminant stored in `sms_campaign.campaign_trigger_type`.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Resolves a [`CampaignTriggerType`] from its integer value.
    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Direct),
            2 => Some(Self::Scheduled),
            3 => Some(Self::Triggered),
            _ => None,
        }
    }

    /// Returns the Fineract enumeration code matching `SmsCampaignTriggerType.getCode()`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Direct => "triggerType.direct",
            Self::Scheduled => "triggerType.schedule",
            Self::Triggered => "triggerType.triggered",
        }
    }
}

/// Outbound SMS delivery status (`sms_messages_outbound.status_enum`).
///
/// Ported from `org.apache.fineract.infrastructure.sms.domain.SmsMessageStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum SmsMessageStatusType {
    Invalid = 0,
    Pending = 100,
    WaitingForDeliveryReport = 150,
    Sent = 200,
    Delivered = 300,
    Failed = 400,
}

impl SmsMessageStatusType {
    /// Returns the integer value stored in `sms_messages_outbound.status_enum`.
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// Resolves an [`SmsMessageStatusType`] from its integer discriminant.
    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::Pending,
            150 => Self::WaitingForDeliveryReport,
            200 => Self::Sent,
            300 => Self::Delivered,
            400 => Self::Failed,
            _ => Self::Invalid,
        }
    }

    /// Returns the Fineract message status code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "smsMessageStatusType.invalid",
            Self::Pending => "smsMessageStatusType.pending",
            Self::WaitingForDeliveryReport => "smsMessageStatusType.waitingForDeliveryReport",
            Self::Sent => "smsMessageStatusType.sent",
            Self::Delivered => "smsMessageStatusType.delivered",
            Self::Failed => "smsMessageStatusType.failed",
        }
    }
}

/// Lifecycle state of an SMS or Email campaign (`sms_campaign.status_enum`).
///
/// Ported from `org.apache.fineract.infrastructure.campaigns.sms.constants.SmsCampaignStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum SmsCampaignStatus {
    Pending = 100,
    Active = 300,
    Closed = 600,
}

/// Outbound SMS message row (`sms_messages_outbound`).
///
/// Ported from `org.apache.fineract.infrastructure.sms.domain.SmsMessage`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmsOutboundMessage {
    pub id: i64,
    pub campaign_id: Option<i64>,
    pub client_id: i64,
    pub mobile_no: Option<String>,
    pub message: String,
    pub status_enum: SmsMessageStatusType,
    pub is_notification: bool,
}

/// SMS Campaign entity (`sms_campaign`) with Mustache-style `{{variable}}` template compilation.
///
/// Ported from `org.apache.fineract.infrastructure.campaigns.sms.domain.SmsCampaign` and
/// `SmsCampaignWritePlatformServiceJpaImpl.compileSmsTemplate(...)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmsCampaign {
    pub id: i64,
    pub campaign_name: String,
    pub campaign_type: i32,
    pub trigger_type: CampaignTriggerType,
    pub provider_id: Option<i64>,
    pub report_id: i64,
    pub param_value: String,
    pub status: SmsCampaignStatus,
    pub message_template: String,
    pub submitted_on_date: NaiveDate,
    pub approved_on_date: Option<NaiveDate>,
    pub closure_date: Option<NaiveDate>,
    pub recurrence: Option<String>,
    pub is_visible: bool,
    pub is_notification: bool,
}

impl SmsCampaign {
    /// Creates a new [`SmsCampaign`] in `Pending` status.
    pub fn new(
        id: i64,
        campaign_name: impl Into<String>,
        trigger_type: CampaignTriggerType,
        provider_id: Option<i64>,
        report_id: i64,
        param_value: impl Into<String>,
        message_template: impl Into<String>,
        submitted_on_date: NaiveDate,
        is_notification: bool,
    ) -> Self {
        Self {
            id,
            campaign_name: campaign_name.into(),
            campaign_type: if is_notification { 3 } else { 1 },
            trigger_type,
            provider_id,
            report_id,
            param_value: param_value.into(),
            status: SmsCampaignStatus::Pending,
            message_template: message_template.into(),
            submitted_on_date,
            approved_on_date: None,
            closure_date: None,
            recurrence: None,
            is_visible: true,
            is_notification,
        }
    }

    /// Activates the campaign on `activation_date`, enforcing `SmsCampaign.activate(...)` rules.
    pub fn activate(&mut self, activation_date: NaiveDate, business_date: NaiveDate) -> Result<(), OpsError> {
        if self.status == SmsCampaignStatus::Active {
            return Err(OpsError::InvalidState(
                "Cannot activate campaign. Campaign is already active.".to_string(),
            ));
        }
        if activation_date < self.submitted_on_date {
            return Err(OpsError::Validation {
                field: "activationDate".to_string(),
                message: "submitted date cannot be after the activation date".to_string(),
            });
        }
        if activation_date > business_date {
            return Err(OpsError::Validation {
                field: "activationDate".to_string(),
                message: "Activation date cannot be in the future.".to_string(),
            });
        }
        self.approved_on_date = Some(activation_date);
        self.status = SmsCampaignStatus::Active;
        Ok(())
    }

    /// Closes the campaign on `closure_date`, enforcing `SmsCampaign.close(...)` rules.
    pub fn close(&mut self, closure_date: NaiveDate, business_date: NaiveDate) -> Result<(), OpsError> {
        if self.status == SmsCampaignStatus::Closed {
            return Err(OpsError::InvalidState(
                "Cannot close campaign. Campaign already in closed state.".to_string(),
            ));
        }
        if closure_date > business_date {
            return Err(OpsError::Validation {
                field: "closureDate".to_string(),
                message: "closure date cannot be in the future.".to_string(),
            });
        }
        self.closure_date = Some(closure_date);
        self.status = SmsCampaignStatus::Closed;
        Ok(())
    }

    /// Compiles the Mustache-style `{{key}}` template against `params`, supporting whitespace inside
    /// braces (e.g. `{{ clientName }}` or `{{loanAccountNo}}`).
    ///
    /// Ported from `SmsCampaignWritePlatformServiceJpaImpl.compileSmsTemplate`.
    pub fn render_message(&self, params: &HashMap<String, String>) -> String {
        compile_mustache_template(&self.message_template, params)
    }

    /// Generates [`SmsOutboundMessage`] rows for a list of recipient parameter maps.
    ///
    /// - For `CampaignTriggerType::Direct` and `CampaignTriggerType::Scheduled`, messages enter
    ///   `SmsMessageStatusType::Pending` (`100`).
    /// - For `CampaignTriggerType::Triggered`, messages enter
    ///   `SmsMessageStatusType::WaitingForDeliveryReport` (`150`) matching `SmsCampaignWritePlatformServiceJpaImpl`.
    pub fn generate_outbound_messages(
        &self,
        recipients: &[HashMap<String, String>],
    ) -> Vec<SmsOutboundMessage> {
        let initial_status = match self.trigger_type {
            CampaignTriggerType::Triggered => SmsMessageStatusType::WaitingForDeliveryReport,
            CampaignTriggerType::Direct | CampaignTriggerType::Scheduled => {
                SmsMessageStatusType::Pending
            }
        };

        recipients
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                let client_id = entry
                    .get("id")
                    .or_else(|| entry.get("clientId"))
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or((idx + 1) as i64);
                let mobile_no = entry.get("mobileNo").cloned();
                if !self.is_notification && mobile_no.as_deref().unwrap_or("").trim().is_empty() {
                    return None;
                }
                let message = self.render_message(entry);
                Some(SmsOutboundMessage {
                    id: (idx + 1) as i64,
                    campaign_id: Some(self.id),
                    client_id,
                    mobile_no,
                    message,
                    status_enum: initial_status,
                    is_notification: self.is_notification,
                })
            })
            .collect()
    }
}

/// Renders a Mustache-style `{{placeholder}}` template using the supplied `params` map.
pub fn compile_mustache_template(template: &str, params: &HashMap<String, String>) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open_idx) = rest.find("{{") {
        output.push_str(&rest[..open_idx]);
        let after_open = &rest[open_idx + 2..];
        if let Some(close_idx) = after_open.find("}}") {
            let key = after_open[..close_idx].trim();
            if let Some(val) = params.get(key) {
                output.push_str(val);
            }
            rest = &after_open[close_idx + 2..];
        } else {
            output.push_str(&rest[open_idx..]);
            rest = "";
        }
    }
    output.push_str(rest);
    output
}

/// Scheduled Email Campaign (`scheduled_email_campaign`).
///
/// Ported from `org.apache.fineract.infrastructure.campaigns.email.domain.EmailCampaign`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledEmailCampaign {
    pub id: i64,
    pub campaign_name: String,
    pub campaign_type: i32,
    pub business_rule_id: i32,
    pub param_value: String,
    pub status_enum: SmsCampaignStatus,
    pub email_subject: String,
    pub email_message: String,
    pub email_attachment_file_format: Option<String>,
    pub stretchy_report_id: Option<i64>,
    pub recurrence: Option<String>,
    pub next_trigger_date: Option<NaiveDateTime>,
}

impl ScheduledEmailCampaign {
    /// Renders both the `email_subject` and `email_message` templates for a recipient parameter map.
    pub fn render_email(&self, params: &HashMap<String, String>) -> (String, String) {
        (
            compile_mustache_template(&self.email_subject, params),
            compile_mustache_template(&self.email_message, params),
        )
    }
}

// ============================================================================
// 4. SPM / Poverty Scorecard Surveys (`m_surveys`, `m_survey_questions`, `m_survey_responses`, `m_survey_scorecards`)
// ============================================================================

/// Survey component grouping (`m_survey_components`).
///
/// Ported from `org.apache.fineract.spm.domain.Component`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurveyComponent {
    pub id: i64,
    pub survey_id: i64,
    pub key: String,
    pub text: String,
    pub description: Option<String>,
    pub sequence_no: i32,
}

/// Selectable answer option for a [`SurveyQuestion`] (`m_survey_responses`).
///
/// Ported from `org.apache.fineract.spm.domain.Response`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurveyResponse {
    pub id: i64,
    pub question_id: i64,
    pub text: String,
    pub value: i32,
    pub sequence_no: i32,
}

/// Question inside an SPM [`Survey`] (`m_survey_questions`).
///
/// Ported from `org.apache.fineract.spm.domain.Question`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurveyQuestion {
    pub id: i64,
    pub survey_id: i64,
    pub component_key: Option<String>,
    pub key: String,
    pub text: String,
    pub description: Option<String>,
    pub sequence_no: i32,
    pub responses: Vec<SurveyResponse>,
}

/// Score range to PPI poverty-likelihood mapping (`m_survey_lookup_tables`).
///
/// Ported from `org.apache.fineract.spm.domain.LookupTable`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurveyLookupTable {
    pub id: i64,
    pub survey_id: i64,
    pub key: String,
    pub description: Option<String>,
    pub value_from: i32,
    pub value_to: i32,
    pub score: Decimal,
}

/// SPM Poverty Scorecard Survey (`m_surveys`).
///
/// Ported from `org.apache.fineract.spm.domain.Survey`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Survey {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub country_code: String,
    pub valid_from: NaiveDate,
    pub valid_to: Option<NaiveDate>,
    pub components: Vec<SurveyComponent>,
    pub questions: Vec<SurveyQuestion>,
    pub lookup_tables: Vec<SurveyLookupTable>,
}

impl Survey {
    /// Checks whether the survey is active on `on_date` (`valid_from <= on_date <= valid_to`).
    pub fn is_active_on(&self, on_date: NaiveDate) -> bool {
        if on_date < self.valid_from {
            return false;
        }
        if let Some(valid_to) = self.valid_to {
            if on_date > valid_to {
                return false;
            }
        }
        true
    }
}

/// Individual answered question row in a client scorecard (`m_survey_scorecards`).
///
/// Ported from `org.apache.fineract.spm.domain.Scorecard`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurveyScorecardEntry {
    pub id: i64,
    pub survey_id: i64,
    pub question_id: i64,
    pub response_id: i64,
    pub user_id: i64,
    pub client_id: i64,
    pub created_on: NaiveDateTime,
    pub value: i32,
}

/// Evaluated SPM scorecard summary for a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScorecardEvaluationResult {
    pub survey_id: i64,
    pub client_id: i64,
    pub entries: Vec<SurveyScorecardEntry>,
    pub total_score: i32,
    pub component_scores: HashMap<String, i32>,
    pub lookup_score: Option<Decimal>,
}

/// SPM Scorecard Evaluator ported from `ScorecardService` and `ScorecardReadPlatformServiceImpl`.
#[derive(Debug, Clone, Default)]
pub struct SpmScorecardEngine;

impl SpmScorecardEngine {
    /// Validates a client's `(question_id, response_id)` selections against [`Survey`], resolves each
    /// response's point `value`, computes per-component and total scores, and resolves the PPI
    /// lookup score if a matching [`SurveyLookupTable`] band (`value_from..=value_to`) exists.
    pub fn evaluate_scorecard(
        survey: &Survey,
        client_id: i64,
        user_id: i64,
        created_on: NaiveDateTime,
        answers: &[(i64, i64)],
    ) -> Result<ScorecardEvaluationResult, OpsError> {
        if !survey.is_active_on(created_on.date()) {
            return Err(OpsError::InvalidState(format!(
                "Survey `{}` is not active on {}",
                survey.key,
                created_on.date()
            )));
        }
        if answers.is_empty() {
            return Err(OpsError::Validation {
                field: "scorecardValues".to_string(),
                message: "At least one question response is required".to_string(),
            });
        }

        let mut entries = Vec::with_capacity(answers.len());
        let mut total_score = 0i32;
        let mut component_scores: HashMap<String, i32> = HashMap::new();

        for (idx, &(question_id, response_id)) in answers.iter().enumerate() {
            let question = survey
                .questions
                .iter()
                .find(|q| q.id == question_id)
                .ok_or_else(|| {
                    OpsError::NotFound(format!(
                        "Question {question_id} not found in survey {}",
                        survey.id
                    ))
                })?;

            let response = question
                .responses
                .iter()
                .find(|r| r.id == response_id)
                .ok_or_else(|| {
                    OpsError::NotFound(format!(
                        "Response {response_id} not found for question {question_id}"
                    ))
                })?;

            total_score += response.value;
            let comp_key = question
                .component_key
                .clone()
                .unwrap_or_else(|| "default".to_string());
            *component_scores.entry(comp_key).or_insert(0) += response.value;

            entries.push(SurveyScorecardEntry {
                id: (idx + 1) as i64,
                survey_id: survey.id,
                question_id,
                response_id,
                user_id,
                client_id,
                created_on,
                value: response.value,
            });
        }

        let lookup_score = survey
            .lookup_tables
            .iter()
            .find(|lt| total_score >= lt.value_from && total_score <= lt.value_to)
            .map(|lt| lt.score);

        Ok(ScorecardEvaluationResult {
            survey_id: survey.id,
            client_id,
            entries,
            total_score,
            component_scores,
            lookup_score,
        })
    }
}

// ============================================================================
// 5. Credit Bureau Integration (`m_creditbureau`, `m_organisation_creditbureau`, `m_creditbureau_loanproduct_mapping`)
// ============================================================================

/// Master credit bureau catalog record (`m_creditbureau`).
///
/// Ported from `org.apache.fineract.infrastructure.creditbureau.domain.CreditBureau`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditBureau {
    pub id: i64,
    pub name: String,
    pub product: String,
    pub country: String,
    pub implementation_key: String,
}

/// Tenant-scoped organisation credit bureau binding (`m_organisation_creditbureau`).
///
/// Ported from `org.apache.fineract.infrastructure.creditbureau.domain.OrganisationCreditBureau`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganisationCreditBureau {
    pub id: i64,
    pub alias: String,
    pub creditbureau_id: i64,
    pub is_active: bool,
    pub configuration: HashMap<String, String>,
}

/// Loan product to organisation credit bureau mapping (`m_creditbureau_loanproduct_mapping`).
///
/// Ported from `org.apache.fineract.infrastructure.creditbureau.domain.CreditBureauLoanProductMapping`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditBureauLoanProductMapping {
    pub id: i64,
    pub organisation_creditbureau_id: i64,
    pub loan_product_id: i64,
    pub is_credit_check_mandatory: bool,
    pub skip_credit_check_in_failure: bool,
    pub stale_period: i32,
    pub is_active: bool,
}

/// Stored credit report record (`m_creditreport`).
///
/// Ported from `org.apache.fineract.infrastructure.creditbureau.domain.CreditReport`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditReport {
    pub id: i64,
    pub bureau_id: i64,
    pub national_id: String,
    pub credit_score: i32,
    pub active_defaults: i32,
    pub total_arrears: Decimal,
    pub report_date: NaiveDate,
}

impl CreditReport {
    /// Checks whether this cached credit report is still fresh within `stale_period_days` as of `as_of_date`.
    pub fn is_fresh(&self, as_of_date: NaiveDate, stale_period_days: i32) -> bool {
        if stale_period_days <= 0 {
            return false;
        }
        let age_days = (as_of_date - self.report_date).num_days();
        age_days >= 0 && age_days <= i64::from(stale_period_days)
    }
}

/// Decision returned by [`CreditBureauIntegrationService::evaluate_loan_application`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreditCheckDecision {
    /// Credit check is not required or mapping is inactive.
    NotRequired,
    /// Credit check passed using either a fresh cached report or live bureau response.
    Passed {
        report: CreditReport,
        used_cached_report: bool,
    },
    /// Live bureau call failed, but `skip_credit_check_in_failure` allowed proceeding.
    SkippedOnBureauFailure { reason: String },
}

/// Credit Bureau orchestration service ported from `ExternalCreditBureauIntegrationWritePlatformServiceImpl`.
#[derive(Debug, Clone, Default)]
pub struct CreditBureauIntegrationService {
    pub bureaus: HashMap<i64, CreditBureau>,
    pub org_bureaus: HashMap<i64, OrganisationCreditBureau>,
    pub product_mappings: Vec<CreditBureauLoanProductMapping>,
    pub cached_reports: Vec<CreditReport>,
}

impl CreditBureauIntegrationService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluates credit bureau policy for a loan application on `loan_product_id` and `national_id`.
    pub fn evaluate_loan_application<F>(
        &mut self,
        loan_product_id: i64,
        national_id: &str,
        as_of_date: NaiveDate,
        live_bureau_fetcher: F,
    ) -> Result<CreditCheckDecision, OpsError>
    where
        F: FnOnce(&OrganisationCreditBureau, &str) -> Result<CreditReport, String>,
    {
        let Some(mapping) = self
            .product_mappings
            .iter()
            .find(|m| m.loan_product_id == loan_product_id && m.is_active)
            .cloned()
        else {
            return Ok(CreditCheckDecision::NotRequired);
        };

        let Some(org_bureau) = self
            .org_bureaus
            .get(&mapping.organisation_creditbureau_id)
            .filter(|ob| ob.is_active)
            .cloned()
        else {
            if mapping.is_credit_check_mandatory {
                return Err(OpsError::CreditBureauFailure(format!(
                    "Organisation credit bureau {} is not active",
                    mapping.organisation_creditbureau_id
                )));
            }
            return Ok(CreditCheckDecision::NotRequired);
        };

        // Check cached report freshness against `stale_period`
        if let Some(cached) = self
            .cached_reports
            .iter()
            .find(|r| {
                r.bureau_id == org_bureau.creditbureau_id
                    && r.national_id == national_id
                    && r.is_fresh(as_of_date, mapping.stale_period)
            })
            .cloned()
        {
            return Ok(CreditCheckDecision::Passed {
                report: cached,
                used_cached_report: true,
            });
        }

        // Fetch live report
        match live_bureau_fetcher(&org_bureau, national_id) {
            Ok(report) => {
                self.cached_reports.push(report.clone());
                Ok(CreditCheckDecision::Passed {
                    report,
                    used_cached_report: false,
                })
            }
            Err(err_reason) => {
                if mapping.skip_credit_check_in_failure {
                    Ok(CreditCheckDecision::SkippedOnBureauFailure { reason: err_reason })
                } else if mapping.is_credit_check_mandatory {
                    Err(OpsError::CreditBureauFailure(err_reason))
                } else {
                    Ok(CreditCheckDecision::SkippedOnBureauFailure { reason: err_reason })
                }
            }
        }
    }
}

// ============================================================================
// 6. JLG Collection Sheet (`CollectionSheetData`, `IndividualCollectionSheetData`, `BulkRepaymentAndSavingsDepositResult`)
// ============================================================================

/// Loan installment due entry inside a JLG or Individual Collection Sheet (`LoanDueData`).
///
/// Ported from `org.apache.fineract.portfolio.collectionsheet.data.LoanDueData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanDueData {
    pub loan_id: i64,
    pub account_id: String,
    pub account_status_id: i32,
    pub product_short_name: String,
    pub product_id: i64,
    pub currency_code: String,
    pub disbursement_amount: Decimal,
    pub principal_due: Decimal,
    pub principal_paid: Decimal,
    pub interest_due: Decimal,
    pub interest_paid: Decimal,
    pub charges_due: Decimal,
    pub fee_due: Decimal,
    pub fee_paid: Decimal,
}

impl LoanDueData {
    /// Returns the total amount due (`principal_due + interest_due + fee_due + charges_due`).
    pub fn total_due(&self) -> Decimal {
        self.principal_due + self.interest_due + self.fee_due + self.charges_due
    }
}

/// Mandatory or voluntary savings deposit due entry (`SavingsDueData`).
///
/// Ported from `org.apache.fineract.portfolio.collectionsheet.data.SavingsDueData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsDueData {
    pub savings_id: i64,
    pub account_id: String,
    pub account_status_id: i32,
    pub product_name: String,
    pub product_id: i64,
    pub currency_code: String,
    pub due_amount: Decimal,
    pub deposit_account_type: String,
}

/// Client entry within a JLG Group Collection Sheet (`JLGClientData`).
///
/// Ported from `org.apache.fineract.portfolio.collectionsheet.data.JLGClientData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JlgClientCollectionData {
    pub client_id: i64,
    pub client_name: String,
    pub attendance_type_id: Option<i32>,
    pub loans: Vec<LoanDueData>,
    pub savings: Vec<SavingsDueData>,
}

/// Group entry within a JLG Collection Sheet (`JLGGroupData`).
///
/// Ported from `org.apache.fineract.portfolio.collectionsheet.data.JLGGroupData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JlgGroupCollectionData {
    pub group_id: i64,
    pub group_name: String,
    pub staff_id: Option<i64>,
    pub staff_name: Option<String>,
    pub level_id: i64,
    pub level_name: String,
    pub clients: Vec<JlgClientCollectionData>,
}

/// Joint Liability Group (JLG) Collection Sheet read model (`JLGCollectionSheetData`).
///
/// Ported from `org.apache.fineract.portfolio.collectionsheet.data.JLGCollectionSheetData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionSheetData {
    pub due_date: NaiveDate,
    pub groups: Vec<JlgGroupCollectionData>,
    pub attendance_type_options: Vec<(i32, String)>,
    pub payment_type_options: Vec<(i64, String)>,
}

impl CollectionSheetData {
    /// Computes the aggregate loan amount due across all groups and clients on the sheet.
    pub fn total_loan_due(&self) -> Decimal {
        self.groups
            .iter()
            .flat_map(|g| &g.clients)
            .flat_map(|c| &c.loans)
            .map(LoanDueData::total_due)
            .fold(Decimal::ZERO, |acc, x| acc + x)
    }

    /// Computes the aggregate savings deposit due across all groups and clients on the sheet.
    pub fn total_savings_due(&self) -> Decimal {
        self.groups
            .iter()
            .flat_map(|g| &g.clients)
            .flat_map(|c| &c.savings)
            .map(|s| s.due_amount)
            .fold(Decimal::ZERO, |acc, x| acc + x)
    }
}

/// Client entry within an [`IndividualCollectionSheetData`] (`IndividualClientData`).
///
/// Ported from `org.apache.fineract.portfolio.collectionsheet.data.IndividualClientData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndividualClientCollectionData {
    pub client_id: i64,
    pub client_name: String,
    pub loans: Vec<LoanDueData>,
    pub savings: Vec<SavingsDueData>,
}

/// Individual Collection Sheet read model (`IndividualCollectionSheetData`).
///
/// Ported from `org.apache.fineract.portfolio.collectionsheet.data.IndividualCollectionSheetData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndividualCollectionSheetData {
    pub due_date: NaiveDate,
    pub clients: Vec<IndividualClientCollectionData>,
    pub payment_type_options: Vec<(i64, String)>,
}

/// Single loan repayment line item in a bulk collection sheet submission (`RepaymentTransactionRequest`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkRepaymentTransaction {
    pub loan_id: i64,
    pub transaction_amount: Decimal,
    pub payment_type_id: Option<i64>,
}

/// Single savings deposit line item in a bulk collection sheet submission (`SavingDueTransactionRequest`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkSavingsDepositTransaction {
    pub savings_id: i64,
    pub transaction_amount: Decimal,
    pub deposit_account_type: i32,
}

/// Single loan disbursal line item in a bulk collection sheet submission (`DisbursementTransactionsRequest`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkDisbursalTransaction {
    pub loan_id: i64,
    pub transaction_amount: Decimal,
}

/// Result of executing `updateCollectionSheet` or `saveIndividualCollectionSheet`.
///
/// Ported from `CollectionSheetWritePlatformServiceJpaRepositoryImpl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkRepaymentAndSavingsDepositResult {
    pub entity_id: i64,
    pub group_id: Option<i64>,
    pub transaction_date: NaiveDate,
    pub loan_transactions: Vec<i64>,
    pub savings_transactions: Vec<i64>,
    pub disbursed_loan_ids: Vec<i64>,
    pub total_loan_repaid: Decimal,
    pub total_savings_deposited: Decimal,
    pub total_loan_disbursed: Decimal,
    pub updated_client_attendance: HashMap<i64, i32>,
    pub note: Option<String>,
}

/// Processor for JLG and Individual Collection Sheet bulk repayments, savings deposits, and disbursals.
///
/// Ported from `CollectionSheetWritePlatformServiceJpaRepositoryImpl`.
#[derive(Debug, Clone, Default)]
pub struct CollectionSheetProcessor;

impl CollectionSheetProcessor {
    /// Executes a bulk JLG or Individual Collection Sheet submission, validating all amounts and
    /// producing [`BulkRepaymentAndSavingsDepositResult`].
    pub fn process_bulk_collections(
        entity_id: i64,
        group_id: Option<i64>,
        transaction_date: NaiveDate,
        bulk_repayments: &[BulkRepaymentTransaction],
        bulk_savings_deposits: &[BulkSavingsDepositTransaction],
        bulk_disbursals: &[BulkDisbursalTransaction],
        clients_attendance: &[(i64, i32)],
        note: Option<String>,
    ) -> Result<BulkRepaymentAndSavingsDepositResult, OpsError> {
        let mut loan_transactions = Vec::new();
        let mut savings_transactions = Vec::new();
        let mut disbursed_loan_ids = Vec::new();
        let mut total_loan_repaid = Decimal::ZERO;
        let mut total_savings_deposited = Decimal::ZERO;
        let mut total_loan_disbursed = Decimal::ZERO;

        for (idx, rep) in bulk_repayments.iter().enumerate() {
            if rep.transaction_amount < Decimal::ZERO {
                return Err(OpsError::Validation {
                    field: "bulkRepaymentTransactions.transactionAmount".to_string(),
                    message: format!(
                        "Repayment amount for loan {} cannot be negative",
                        rep.loan_id
                    ),
                });
            }
            if rep.transaction_amount > Decimal::ZERO {
                total_loan_repaid += rep.transaction_amount;
                loan_transactions.push(5000 + idx as i64 + 1);
            }
        }

        for (idx, dep) in bulk_savings_deposits.iter().enumerate() {
            if dep.transaction_amount < Decimal::ZERO {
                return Err(OpsError::Validation {
                    field: "bulkSavingsDueTransactions.transactionAmount".to_string(),
                    message: format!(
                        "Savings deposit amount for account {} cannot be negative",
                        dep.savings_id
                    ),
                });
            }
            if dep.transaction_amount > Decimal::ZERO {
                total_savings_deposited += dep.transaction_amount;
                savings_transactions.push(7000 + idx as i64 + 1);
            }
        }

        for disb in bulk_disbursals {
            if disb.transaction_amount <= Decimal::ZERO {
                return Err(OpsError::Validation {
                    field: "bulkDisbursementTransactions.transactionAmount".to_string(),
                    message: format!(
                        "Disbursement amount for loan {} must be positive",
                        disb.loan_id
                    ),
                });
            }
            total_loan_disbursed += disb.transaction_amount;
            disbursed_loan_ids.push(disb.loan_id);
        }

        let mut updated_client_attendance = HashMap::with_capacity(clients_attendance.len());
        for &(client_id, attendance_type) in clients_attendance {
            if !(1..=5).contains(&attendance_type) {
                return Err(OpsError::Validation {
                    field: "clientsAttendance.attendanceType".to_string(),
                    message: format!("Invalid attendanceType `{attendance_type}` for client {client_id}"),
                });
            }
            updated_client_attendance.insert(client_id, attendance_type);
        }

        Ok(BulkRepaymentAndSavingsDepositResult {
            entity_id,
            group_id,
            transaction_date,
            loan_transactions,
            savings_transactions,
            disbursed_loan_ids,
            total_loan_repaid,
            total_savings_deposited,
            total_loan_disbursed,
            updated_client_attendance,
            note: note.filter(|n| !n.trim().is_empty()),
        })
    }
}

// ============================================================================
// 7. Custom Extension Plugin Registry (`:custom` replacement)
// ============================================================================

/// Custom loan schedule generation request passed to [`CustomModulePlugin::generate_custom_schedule`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomScheduleRequest {
    pub strategy_code: String,
    pub principal: Decimal,
    pub annual_interest_rate: Decimal,
    pub number_of_installments: u32,
    pub disbursement_date: NaiveDate,
}

/// Custom installment produced by a [`CustomModulePlugin`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomScheduleInstallment {
    pub period_number: u32,
    pub due_date: NaiveDate,
    pub principal_due: Decimal,
    pub interest_due: Decimal,
    pub fee_due: Decimal,
}

/// Command interception context passed to [`CustomModulePlugin::intercept_command`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomCommandContext {
    pub tenant_id: String,
    pub entity_name: String,
    pub action_name: String,
    pub payload_json: String,
}

/// Custom GL journal line produced by [`CustomModulePlugin::generate_custom_accounting_entries`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomAccountingLine {
    pub gl_account_code: String,
    pub is_debit: bool,
    pub amount: Decimal,
    pub description: String,
}

/// Runtime extension trait replacing Fineract's `:custom` Spring Boot auto-configuration modules
/// (`custom/acme/loan/processor`, `custom/acme/loan/cob`, `custom/acme/loan/job`,
/// `custom/acme/event/externalevent`, `custom/acme/note`).
///
/// Institutions implement [`CustomModulePlugin`] to register custom repayment schedule generators,
/// command pre/post interceptors, or custom accounting rules dynamically at runtime without
/// modifying core crates.
pub trait CustomModulePlugin: Send + Sync + fmt::Debug {
    /// Unique plugin identifier (e.g. `"acme-loan-plugin"`).
    fn plugin_id(&self) -> &str;

    /// Human-readable plugin description (e.g. `"ACME Corp.: standard loan transaction processing strategy"`).
    fn plugin_name(&self) -> &str;

    /// Optional custom repayment schedule / transaction strategy code (e.g. `"acme-standard-strategy"`).
    fn schedule_strategy_code(&self) -> Option<&str> {
        None
    }

    /// Generates a custom repayment schedule when `schedule_strategy_code()` is invoked.
    fn generate_custom_schedule(
        &self,
        _request: &CustomScheduleRequest,
    ) -> Result<Option<Vec<CustomScheduleInstallment>>, OpsError> {
        Ok(None)
    }

    /// Pre-execution command interceptor that can validate, enrich, or reject a command payload.
    fn intercept_command(
        &self,
        context: &mut CustomCommandContext,
    ) -> Result<(), OpsError> {
        let _ = context;
        Ok(())
    }

    /// Generates custom double-entry accounting lines for a transaction event.
    fn generate_custom_accounting_entries(
        &self,
        _entity_name: &str,
        _action_name: &str,
        _amount: Decimal,
    ) -> Result<Option<Vec<CustomAccountingLine>>, OpsError> {
        Ok(None)
    }
}

/// Thread-safe runtime registry for [`CustomModulePlugin`] extensions (`:custom` replacement).
#[derive(Debug, Default, Clone)]
pub struct CustomPluginRegistry {
    plugins: Arc<DashMap<String, Arc<dyn CustomModulePlugin>>>,
}

impl CustomPluginRegistry {
    /// Creates an empty [`CustomPluginRegistry`].
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(DashMap::new()),
        }
    }

    /// Registers a [`CustomModulePlugin`] implementation by its `plugin_id()`.
    pub fn register(&self, plugin: Arc<dyn CustomModulePlugin>) {
        self.plugins
            .insert(plugin.plugin_id().to_string(), plugin);
    }

    /// Unregisters a plugin by ID, returning `true` if it was present.
    pub fn unregister(&self, plugin_id: &str) -> bool {
        self.plugins.remove(plugin_id).is_some()
    }

    /// Looks up a registered plugin by `plugin_id`.
    pub fn get(&self, plugin_id: &str) -> Option<Arc<dyn CustomModulePlugin>> {
        self.plugins.get(plugin_id).map(|entry| Arc::clone(entry.value()))
    }

    /// Returns the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Returns `true` if no plugins are registered.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Finds a plugin matching `strategy_code` and invokes [`CustomModulePlugin::generate_custom_schedule`].
    pub fn generate_schedule(
        &self,
        request: &CustomScheduleRequest,
    ) -> Result<Option<Vec<CustomScheduleInstallment>>, OpsError> {
        for entry in self.plugins.iter() {
            if entry.value().schedule_strategy_code() == Some(request.strategy_code.as_str()) {
                return entry.value().generate_custom_schedule(request);
            }
        }
        Ok(None)
    }

    /// Runs all registered command interceptors in deterministic `plugin_id` order.
    pub fn run_command_interceptors(
        &self,
        context: &mut CustomCommandContext,
    ) -> Result<(), OpsError> {
        let mut keys: Vec<String> = self.plugins.iter().map(|e| e.key().clone()).collect();
        keys.sort();
        for key in keys {
            if let Some(plugin) = self.get(&key) {
                plugin.intercept_command(context)?;
            }
        }
        Ok(())
    }

    /// Invokes custom accounting rule plugins for `(entity_name, action_name, amount)` and validates
    /// that any returned journal lines are strictly balanced (`sum(debits) == sum(credits)`).
    pub fn evaluate_custom_accounting_rules(
        &self,
        entity_name: &str,
        action_name: &str,
        amount: Decimal,
    ) -> Result<Vec<CustomAccountingLine>, OpsError> {
        let mut keys: Vec<String> = self.plugins.iter().map(|e| e.key().clone()).collect();
        keys.sort();
        let mut all_lines = Vec::new();
        for key in keys {
            if let Some(plugin) = self.get(&key) {
                if let Some(lines) =
                    plugin.generate_custom_accounting_entries(entity_name, action_name, amount)?
                {
                    let total_debit: Decimal = lines
                        .iter()
                        .filter(|l| l.is_debit)
                        .map(|l| l.amount)
                        .sum();
                    let total_credit: Decimal = lines
                        .iter()
                        .filter(|l| !l.is_debit)
                        .map(|l| l.amount)
                        .sum();
                    if total_debit != total_credit {
                        return Err(OpsError::PluginError {
                            plugin_id: plugin.plugin_id().to_string(),
                            message: format!(
                                "Unbalanced custom accounting lines: debit {total_debit} != credit {total_credit}"
                            ),
                        });
                    }
                    all_lines.extend(lines);
                }
            }
        }
        Ok(all_lines)
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn dt(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn dtm(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> NaiveDateTime {
        dt(y, m, d)
            .and_hms_opt(h, min, s)
            .expect("valid datetime")
    }

    #[test]
    fn test_global_entity_type_discriminants_and_roundtrip() {
        assert_eq!(GlobalEntityType::ClientsPerson as i32, 1);
        assert_eq!(GlobalEntityType::ClientsEntity as i32, 2);
        assert_eq!(GlobalEntityType::Groups as i32, 3);
        assert_eq!(GlobalEntityType::Centers as i32, 4);
        assert_eq!(GlobalEntityType::Offices as i32, 5);
        assert_eq!(GlobalEntityType::Staff as i32, 6);
        assert_eq!(GlobalEntityType::Users as i32, 7);
        assert_eq!(GlobalEntityType::ChartOfAccounts as i32, 8);
        assert_eq!(GlobalEntityType::GlJournalEntries as i32, 9);
        assert_eq!(GlobalEntityType::Loans as i32, 10);
        assert_eq!(GlobalEntityType::LoanTransactions as i32, 11);
        assert_eq!(GlobalEntityType::SavingsAccount as i32, 12);
        assert_eq!(GlobalEntityType::SavingsTransactions as i32, 13);
        assert_eq!(GlobalEntityType::FixedDepositAccounts as i32, 14);
        assert_eq!(GlobalEntityType::RecurringDepositAccounts as i32, 15);
        assert_eq!(GlobalEntityType::ShareAccounts as i32, 16);
        assert_eq!(GlobalEntityType::Guarantors as i32, 17);

        for code in 1..=17 {
            let entity = GlobalEntityType::from_i32(code).expect("valid entity type");
            assert_eq!(entity.value(), code);
            assert!(!entity.code().is_empty());
            assert!(!entity.sheet_name().is_empty());
        }
        assert_eq!(GlobalEntityType::from_i32(99), None);
    }

    #[test]
    fn test_csv_bulk_import_clients_and_error_annotation() {
        let processor = CsvBulkImportProcessor::new();
        let csv = "\
first_name,last_name,office_name,submitted_on,activation_date,active,mobile_no
Ada,Lovelace,Head Office,2026-01-10,2026-01-12,true,9990001111
,MissingFirst,Head Office,2026-01-10,2026-01-12,true,9990002222
Grace,Hopper,Head Office,2026-01-15,2026-01-10,true,9990003333
";

        let res = processor.process_csv(
            42,
            GlobalEntityType::ClientsPerson,
            dtm(2026, 1, 20, 10, 0, 0),
            dtm(2026, 1, 20, 10, 0, 5),
            csv,
        );

        assert!(res.import_document.completed);
        assert_eq!(res.import_document.total_records, 3);
        assert_eq!(res.import_document.success_count, 1);
        assert_eq!(res.import_document.failure_count, 2);
        assert_eq!(res.dispatched_commands.len(), 1);
        assert_eq!(res.dispatched_commands[0].entity_name, "CLIENT");
        assert_eq!(res.dispatched_commands[0].action_name, "CREATE");

        assert_eq!(res.row_outcomes[0].status, "Imported");
        assert_eq!(res.row_outcomes[0].error, None);
        assert_eq!(res.row_outcomes[1].status, "Creation failed");
        assert!(res.row_outcomes[1]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("firstName"));
        assert_eq!(res.row_outcomes[2].status, "Creation failed");
        assert!(res.row_outcomes[2]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("activationDate cannot be before submittedOnDate"));

        assert!(res.annotated_csv_report.contains("Status,Error"));
        assert!(res.annotated_csv_report.contains("Imported,"));
        assert!(res.annotated_csv_report.contains("Creation failed,"));
    }

    #[test]
    fn test_csv_bulk_import_loans_repayments_savings_and_journal_entries() {
        let processor = CsvBulkImportProcessor::new();

        // 1. Loans (1 valid, 1 disbursal date before approval date -> Disbursal failed)
        let loans_csv = "\
office_name,loan_type,client_name,product_name,principal,number_of_repayments,nominal_interest_rate,submitted_on_date,approved_date,disbursed_date
Head Office,individual,Ada Lovelace,Micro Loan,5000.00,12,12.5,2026-01-01,2026-01-02,2026-01-05
Head Office,individual,Alan Turing,Micro Loan,5000.00,12,12.5,2026-01-01,2026-01-05,2026-01-03
";
        let loan_res = processor.process_csv(
            50,
            GlobalEntityType::Loans,
            dtm(2026, 1, 20, 10, 0, 0),
            dtm(2026, 1, 20, 10, 0, 1),
            loans_csv,
        );
        assert_eq!(loan_res.import_document.success_count, 1);
        assert_eq!(loan_res.import_document.failure_count, 1);
        assert_eq!(
            loan_res.row_outcomes[1].status,
            bulk_import_status::STATUS_DISBURSAL_FAILED
        );

        // 2. Loan Repayments
        let repayments_csv = "\
office_name,client_name,loan_account_no,amount,repaid_on_date,repayment_type
Head Office,Ada Lovelace,LN-0001,450.25,2026-02-01,Cash
Head Office,Ada Lovelace,LN-0001,-10.00,2026-02-01,Cash
";
        let rep_res = processor.process_csv(
            51,
            GlobalEntityType::LoanTransactions,
            dtm(2026, 2, 1, 12, 0, 0),
            dtm(2026, 2, 1, 12, 0, 1),
            repayments_csv,
        );
        assert_eq!(rep_res.import_document.success_count, 1);
        assert_eq!(rep_res.import_document.failure_count, 1);

        // 3. Savings Deposits
        let savings_csv = "\
office_name,client_name,savings_account_no,transaction_type,amount,transaction_date,payment_type
Head Office,Ada Lovelace,SAV-101,Deposit,1200.00,2026-02-01,Cash
";
        let sav_res = processor.process_csv(
            52,
            GlobalEntityType::SavingsTransactions,
            dtm(2026, 2, 1, 12, 0, 0),
            dtm(2026, 2, 1, 12, 0, 1),
            savings_csv,
        );
        assert_eq!(sav_res.import_document.success_count, 1);
        assert_eq!(sav_res.dispatched_commands[0].action_name, "DEPOSIT");

        // 4. GL Journal Entries (balanced TX-1 succeeds, unbalanced TX-2 fails)
        let gl_csv = "\
office_name,transaction_date,currency_code,transaction_id,gl_account_debit,amount_debit,gl_account_credit,amount_credit
Head Office,2026-02-01,USD,TX-1,1001,250.00,2001,250.00
Head Office,2026-02-01,USD,TX-2,1001,300.00,2001,100.00
";
        let gl_res = processor.process_csv(
            53,
            GlobalEntityType::GlJournalEntries,
            dtm(2026, 2, 1, 12, 0, 0),
            dtm(2026, 2, 1, 12, 0, 1),
            gl_csv,
        );
        assert_eq!(gl_res.import_document.success_count, 1);
        assert_eq!(gl_res.import_document.failure_count, 1);
        assert!(gl_res.row_outcomes[1]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unbalanced"));
    }

    #[test]
    fn test_webhook_hmac_sha256_signature_and_event_dispatch() {
        // Verify RFC 4231 Test Case 2 vector ("Jefe", "what do ya want for nothing?")
        let rfc_digest = WebhookDispatcher::compute_hmac_sha256_hex(
            b"Jefe",
            b"what do ya want for nothing?",
        );
        assert_eq!(
            rfc_digest,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );

        let mut dispatcher = WebhookDispatcher::new("tenant-default-secret");

        let mut web_config = HashMap::new();
        web_config.insert(
            "Payload URL".to_string(),
            "https://hooks.example.org/fineract".to_string(),
        );
        web_config.insert("Content Type".to_string(), "json".to_string());
        web_config.insert("Secret".to_string(), "super-secret-key".to_string());

        dispatcher.register_hook(Hook {
            id: 10,
            template_id: HookTemplate::Web.id(),
            name: "Loan Approval Webhook".to_string(),
            is_active: true,
            events: vec![
                HookRegisteredEvent::new("LOAN", "APPROVE"),
                HookRegisteredEvent::new("CLIENT", "CREATE"),
            ],
            config: web_config,
        });

        // Inactive hook should be ignored
        let mut inactive_config = HashMap::new();
        inactive_config.insert(
            "Payload URL".to_string(),
            "https://inactive.example.org".to_string(),
        );
        dispatcher.register_hook(Hook {
            id: 11,
            template_id: HookTemplate::Web.id(),
            name: "Inactive Hook".to_string(),
            is_active: false,
            events: vec![HookRegisteredEvent::new("LOAN", "APPROVE")],
            config: inactive_config,
        });

        let payload = r#"{"loanId":101,"resourceId":101,"changes":{"status":"APPROVED"}}"#;
        let requests = dispatcher.dispatch_event("default", "LOAN", "APPROVE", payload);
        assert_eq!(requests.len(), 1);

        let req = &requests[0];
        assert_eq!(req.hook_id, 10);
        assert_eq!(req.template, HookTemplate::Web);
        assert_eq!(
            req.headers.get(WebhookDispatcher::HEADER_ENTITY).map(String::as_str),
            Some("LOAN")
        );
        assert_eq!(
            req.headers.get(WebhookDispatcher::HEADER_ACTION).map(String::as_str),
            Some("APPROVE")
        );
        assert_eq!(
            req.headers.get(WebhookDispatcher::HEADER_TENANT).map(String::as_str),
            Some("default")
        );

        let sig = req
            .headers
            .get(WebhookDispatcher::HEADER_SIGNATURE)
            .expect("signature header present");
        assert!(sig.starts_with("sha256="));
        assert!(WebhookDispatcher::verify_signature_header(
            "super-secret-key",
            payload,
            sig
        ));
        assert!(!WebhookDispatcher::verify_signature_header(
            "wrong-secret",
            payload,
            sig
        ));

        // Unregistered event produces zero dispatches
        let unsubscribed = dispatcher.dispatch_event("default", "SAVINGS", "CLOSE", payload);
        assert!(unsubscribed.is_empty());
    }

    #[test]
    fn test_sms_campaign_template_interpolation_and_lifecycle() {
        assert_eq!(CampaignTriggerType::Direct.value(), 1);
        assert_eq!(CampaignTriggerType::Scheduled.value(), 2);
        assert_eq!(CampaignTriggerType::Triggered.value(), 3);

        assert_eq!(SmsMessageStatusType::Invalid.value(), 0);
        assert_eq!(SmsMessageStatusType::Pending.value(), 100);
        assert_eq!(SmsMessageStatusType::WaitingForDeliveryReport.value(), 150);
        assert_eq!(SmsMessageStatusType::Sent.value(), 200);
        assert_eq!(SmsMessageStatusType::Delivered.value(), 300);
        assert_eq!(SmsMessageStatusType::Failed.value(), 400);

        let mut campaign = SmsCampaign::new(
            7,
            "Repayment Reminder",
            CampaignTriggerType::Triggered,
            Some(1),
            12,
            r#"{"reportName":"LoanRepaymentReminder"}"#,
            "Dear {{clientName}}, your loan {{loanAccountNo}} repayment of {{amountDue}} is due on {{ dueDate }}.",
            dt(2026, 3, 1),
            false,
        );

        // Activate campaign
        campaign
            .activate(dt(2026, 3, 2), dt(2026, 3, 5))
            .expect("valid activation");
        assert_eq!(campaign.status, SmsCampaignStatus::Active);

        let mut recipient = HashMap::new();
        recipient.insert("id".to_string(), "501".to_string());
        recipient.insert("mobileNo".to_string(), "+15550199".to_string());
        recipient.insert("clientName".to_string(), "Chitra Devi".to_string());
        recipient.insert("loanAccountNo".to_string(), "000000501".to_string());
        recipient.insert("amountDue".to_string(), "1250.00".to_string());
        recipient.insert("dueDate".to_string(), "2026-03-15".to_string());

        let outbound = campaign.generate_outbound_messages(&[recipient]);
        assert_eq!(outbound.len(), 1);
        assert_eq!(
            outbound[0].message,
            "Dear Chitra Devi, your loan 000000501 repayment of 1250.00 is due on 2026-03-15."
        );
        assert_eq!(
            outbound[0].status_enum,
            SmsMessageStatusType::WaitingForDeliveryReport
        );

        campaign
            .close(dt(2026, 3, 5), dt(2026, 3, 5))
            .expect("valid close");
        assert_eq!(campaign.status, SmsCampaignStatus::Closed);
    }

    #[test]
    fn test_spm_scorecard_point_calculation_and_lookup_band() {
        let survey = Survey {
            id: 1,
            key: "PPI-IN-2026".to_string(),
            name: "India Poverty Probability Index".to_string(),
            description: Some("Standard SPM Poverty Scorecard".to_string()),
            country_code: "IN".to_string(),
            valid_from: dt(2025, 1, 1),
            valid_to: Some(dt(2027, 12, 31)),
            components: vec![
                SurveyComponent {
                    id: 1,
                    survey_id: 1,
                    key: "housing".to_string(),
                    text: "Housing Quality".to_string(),
                    description: None,
                    sequence_no: 1,
                },
                SurveyComponent {
                    id: 2,
                    survey_id: 1,
                    key: "assets".to_string(),
                    text: "Household Assets".to_string(),
                    description: None,
                    sequence_no: 2,
                },
            ],
            questions: vec![
                SurveyQuestion {
                    id: 101,
                    survey_id: 1,
                    component_key: Some("housing".to_string()),
                    key: "roof_material".to_string(),
                    text: "What is the primary roof material?".to_string(),
                    description: None,
                    sequence_no: 1,
                    responses: vec![
                        SurveyResponse {
                            id: 1001,
                            question_id: 101,
                            text: "Thatch / Mud".to_string(),
                            value: 5,
                            sequence_no: 1,
                        },
                        SurveyResponse {
                            id: 1002,
                            question_id: 101,
                            text: "Reinforced Concrete".to_string(),
                            value: 20,
                            sequence_no: 2,
                        },
                    ],
                },
                SurveyQuestion {
                    id: 102,
                    survey_id: 1,
                    component_key: Some("assets".to_string()),
                    key: "cooking_fuel".to_string(),
                    text: "What is the primary cooking fuel?".to_string(),
                    description: None,
                    sequence_no: 2,
                    responses: vec![
                        SurveyResponse {
                            id: 2001,
                            question_id: 102,
                            text: "Firewood".to_string(),
                            value: 4,
                            sequence_no: 1,
                        },
                        SurveyResponse {
                            id: 2002,
                            question_id: 102,
                            text: "LPG / Electricity".to_string(),
                            value: 15,
                            sequence_no: 2,
                        },
                    ],
                },
            ],
            lookup_tables: vec![
                SurveyLookupTable {
                    id: 1,
                    survey_id: 1,
                    key: "ppi_band_low".to_string(),
                    description: None,
                    value_from: 0,
                    value_to: 19,
                    score: dec!(64.5),
                },
                SurveyLookupTable {
                    id: 2,
                    survey_id: 1,
                    key: "ppi_band_high".to_string(),
                    description: None,
                    value_from: 20,
                    value_to: 100,
                    score: dec!(11.2),
                },
            ],
        };

        let result = SpmScorecardEngine::evaluate_scorecard(
            &survey,
            900,
            1,
            dtm(2026, 4, 10, 9, 30, 0),
            &[(101, 1002), (102, 2002)],
        )
        .expect("valid scorecard evaluation");

        assert_eq!(result.total_score, 35);
        assert_eq!(result.component_scores.get("housing"), Some(&20));
        assert_eq!(result.component_scores.get("assets"), Some(&15));
        assert_eq!(result.lookup_score, Some(dec!(11.2)));
        assert_eq!(result.entries.len(), 2);
    }

    #[test]
    fn test_credit_bureau_staleness_and_failure_fallback() {
        let mut service = CreditBureauIntegrationService::new();
        service.bureaus.insert(
            1,
            CreditBureau {
                id: 1,
                name: "ThitsaWorks".to_string(),
                product: "Microfinance Credit Check".to_string(),
                country: "MM".to_string(),
                implementation_key: "THITSAWORKS".to_string(),
            },
        );
        service.org_bureaus.insert(
            10,
            OrganisationCreditBureau {
                id: 10,
                alias: "Primary Bureau".to_string(),
                creditbureau_id: 1,
                is_active: true,
                configuration: HashMap::new(),
            },
        );
        service.product_mappings.push(CreditBureauLoanProductMapping {
            id: 100,
            organisation_creditbureau_id: 10,
            loan_product_id: 77,
            is_credit_check_mandatory: true,
            skip_credit_check_in_failure: false,
            stale_period: 30,
            is_active: true,
        });

        // 1. Live fetch populates cache
        let decision1 = service
            .evaluate_loan_application(77, "NID-9988", dt(2026, 5, 1), |_ob, nid| {
                Ok(CreditReport {
                    id: 1,
                    bureau_id: 1,
                    national_id: nid.to_string(),
                    credit_score: 745,
                    active_defaults: 0,
                    total_arrears: Decimal::ZERO,
                    report_date: dt(2026, 5, 1),
                })
            })
            .expect("live fetch succeeds");
        assert!(matches!(
            decision1,
            CreditCheckDecision::Passed {
                used_cached_report: false,
                ..
            }
        ));

        // 2. Within 30-day stale_period -> reuses cached report
        let decision2 = service
            .evaluate_loan_application(77, "NID-9988", dt(2026, 5, 20), |_ob, _nid| {
                Err("should not call live bureau when cache is fresh".to_string())
            })
            .expect("cached report reused");
        assert!(matches!(
            decision2,
            CreditCheckDecision::Passed {
                used_cached_report: true,
                ..
            }
        ));

        // 3. After stale_period (45 days later), live failure with skip_credit_check_in_failure = false -> error
        let err = service
            .evaluate_loan_application(77, "NID-9988", dt(2026, 6, 15), |_ob, _nid| {
                Err("Bureau timeout".to_string())
            })
            .expect_err("mandatory check without skip should fail");
        assert!(matches!(err, OpsError::CreditBureauFailure(_)));
    }

    #[test]
    fn test_jlg_collection_sheet_bulk_repayment_and_savings_deposit() {
        let sheet = CollectionSheetData {
            due_date: dt(2026, 6, 1),
            groups: vec![JlgGroupCollectionData {
                group_id: 20,
                group_name: "Sunrise JLG".to_string(),
                staff_id: Some(5),
                staff_name: Some("Officer Meera".to_string()),
                level_id: 2,
                level_name: "Group".to_string(),
                clients: vec![JlgClientCollectionData {
                    client_id: 301,
                    client_name: "Kavitha R".to_string(),
                    attendance_type_id: Some(1),
                    loans: vec![LoanDueData {
                        loan_id: 401,
                        account_id: "LN-401".to_string(),
                        account_status_id: 300,
                        product_short_name: "JLG".to_string(),
                        product_id: 1,
                        currency_code: "INR".to_string(),
                        disbursement_amount: dec!(10000.00),
                        principal_due: dec!(800.00),
                        principal_paid: Decimal::ZERO,
                        interest_due: dec!(120.00),
                        interest_paid: Decimal::ZERO,
                        charges_due: dec!(30.00),
                        fee_due: dec!(20.00),
                        fee_paid: Decimal::ZERO,
                    }],
                    savings: vec![SavingsDueData {
                        savings_id: 601,
                        account_id: "SAV-601".to_string(),
                        account_status_id: 300,
                        product_name: "Mandatory JLG Savings".to_string(),
                        product_id: 2,
                        currency_code: "INR".to_string(),
                        due_amount: dec!(200.00),
                        deposit_account_type: "SAVINGS".to_string(),
                    }],
                }],
            }],
            attendance_type_options: vec![(1, "Present".to_string()), (2, "Absent".to_string())],
            payment_type_options: vec![(1, "Cash".to_string())],
        };

        assert_eq!(sheet.total_loan_due(), dec!(970.00));
        assert_eq!(sheet.total_savings_due(), dec!(200.00));

        let individual_sheet = IndividualCollectionSheetData {
            due_date: dt(2026, 6, 1),
            clients: vec![IndividualClientCollectionData {
                client_id: 301,
                client_name: "Kavitha R".to_string(),
                loans: sheet.groups[0].clients[0].loans.clone(),
                savings: sheet.groups[0].clients[0].savings.clone(),
            }],
            payment_type_options: vec![(1, "Cash".to_string())],
        };
        assert_eq!(individual_sheet.clients.len(), 1);

        let result = CollectionSheetProcessor::process_bulk_collections(
            20,
            Some(20),
            dt(2026, 6, 1),
            &[BulkRepaymentTransaction {
                loan_id: 401,
                transaction_amount: dec!(970.00),
                payment_type_id: Some(1),
            }],
            &[BulkSavingsDepositTransaction {
                savings_id: 601,
                transaction_amount: dec!(200.00),
                deposit_account_type: 100,
            }],
            &[BulkDisbursalTransaction {
                loan_id: 402,
                transaction_amount: dec!(15000.00),
            }],
            &[(301, 1)],
            Some("Weekly center collection".to_string()),
        )
        .expect("bulk collection sheet execution succeeds");

        assert_eq!(result.total_loan_repaid, dec!(970.00));
        assert_eq!(result.total_savings_deposited, dec!(200.00));
        assert_eq!(result.total_loan_disbursed, dec!(15000.00));
        assert_eq!(result.loan_transactions.len(), 1);
        assert_eq!(result.savings_transactions.len(), 1);
        assert_eq!(result.disbursed_loan_ids, vec![402]);
        assert_eq!(result.updated_client_attendance.get(&301), Some(&1));
    }

    #[derive(Debug)]
    struct AcmeCustomPlugin;

    impl CustomModulePlugin for AcmeCustomPlugin {
        fn plugin_id(&self) -> &str {
            "acme-loan-plugin"
        }

        fn plugin_name(&self) -> &str {
            "ACME Corp.: standard loan transaction processing strategy"
        }

        fn schedule_strategy_code(&self) -> Option<&str> {
            Some("acme-standard-strategy")
        }

        fn generate_custom_schedule(
            &self,
            request: &CustomScheduleRequest,
        ) -> Result<Option<Vec<CustomScheduleInstallment>>, OpsError> {
            let n = Decimal::from(request.number_of_installments.max(1));
            let principal_per_period = (request.principal / n).round_dp(2);
            let mut installments = Vec::with_capacity(request.number_of_installments as usize);
            for period in 1..=request.number_of_installments {
                installments.push(CustomScheduleInstallment {
                    period_number: period,
                    due_date: request.disbursement_date
                        + chrono::Days::new(u64::from(period) * 30),
                    principal_due: principal_per_period,
                    interest_due: dec!(10.00),
                    fee_due: Decimal::ZERO,
                });
            }
            Ok(Some(installments))
        }

        fn intercept_command(
            &self,
            context: &mut CustomCommandContext,
        ) -> Result<(), OpsError> {
            if context.entity_name == "NOTE" && context.payload_json.contains("FORBIDDEN") {
                return Err(OpsError::PluginError {
                    plugin_id: self.plugin_id().to_string(),
                    message: "ACME policy forbids FORBIDDEN keyword in notes".to_string(),
                });
            }
            context.payload_json = context
                .payload_json
                .replace("\"acmeEnriched\":false", "\"acmeEnriched\":true");
            Ok(())
        }

        fn generate_custom_accounting_entries(
            &self,
            entity_name: &str,
            action_name: &str,
            amount: Decimal,
        ) -> Result<Option<Vec<CustomAccountingLine>>, OpsError> {
            if entity_name == "LOAN" && action_name == "REPAYMENT" {
                Ok(Some(vec![
                    CustomAccountingLine {
                        gl_account_code: "ACME-CASH-101".to_string(),
                        is_debit: true,
                        amount,
                        description: "ACME Cash Receipt".to_string(),
                    },
                    CustomAccountingLine {
                        gl_account_code: "ACME-PORTFOLIO-201".to_string(),
                        is_debit: false,
                        amount,
                        description: "ACME Loan Portfolio Reduction".to_string(),
                    },
                ]))
            } else {
                Ok(None)
            }
        }
    }

    #[test]
    fn test_custom_module_plugin_registry_schedule_interceptor_and_accounting() {
        let registry = CustomPluginRegistry::new();
        assert!(registry.is_empty());

        registry.register(Arc::new(AcmeCustomPlugin));
        assert_eq!(registry.len(), 1);

        // 1. Custom Schedule Generation
        let schedule = registry
            .generate_schedule(&CustomScheduleRequest {
                strategy_code: "acme-standard-strategy".to_string(),
                principal: dec!(1200.00),
                annual_interest_rate: dec!(12.0),
                number_of_installments: 3,
                disbursement_date: dt(2026, 1, 1),
            })
            .expect("custom schedule succeeds")
            .expect("plugin matched strategy_code");
        assert_eq!(schedule.len(), 3);
        assert_eq!(schedule[0].principal_due, dec!(400.00));

        // 2. Command Interceptor enrichment & rejection
        let mut ctx = CustomCommandContext {
            tenant_id: "default".to_string(),
            entity_name: "NOTE".to_string(),
            action_name: "CREATE".to_string(),
            payload_json: r#"{"note":"Hello ACME","acmeEnriched":false}"#.to_string(),
        };
        registry
            .run_command_interceptors(&mut ctx)
            .expect("valid note passes interceptor");
        assert!(ctx.payload_json.contains("\"acmeEnriched\":true"));

        let mut bad_ctx = CustomCommandContext {
            tenant_id: "default".to_string(),
            entity_name: "NOTE".to_string(),
            action_name: "CREATE".to_string(),
            payload_json: r#"{"note":"FORBIDDEN content"}"#.to_string(),
        };
        assert!(registry.run_command_interceptors(&mut bad_ctx).is_err());

        // 3. Balanced custom accounting rules
        let lines = registry
            .evaluate_custom_accounting_rules("LOAN", "REPAYMENT", dec!(500.00))
            .expect("balanced lines succeed");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].is_debit);
        assert!(!lines[1].is_debit);

        assert!(registry.unregister("acme-loan-plugin"));
        assert!(registry.is_empty());
    }
}
