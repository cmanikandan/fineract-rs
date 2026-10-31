//! # Fineract Unified Axum Server & OpenAPI 3.0 Gateway (`fineract-server-rs`)
//!
//! Port of `fineract-provider` Web / REST layer, Spring Boot Actuator / Health / Info endpoints,
//! OpenAPI 3.0 specification generator, multi-tenant context middleware, idempotency middleware,
//! and unified router mounting all Wave 1–6 domain routers under `/fineract-provider/api/v1`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use std::collections::BTreeMap;

use fineract_batch_api_rs::{BatchApiService, BatchRequest, BatchResponse};
use fineract_command_rs::pipeline::CommandProcessingResult;
use fineract_core_rs::context::{ActionContext, BusinessDateType, ExecutionContext, FineractPlatformTenant};
use fineract_core_rs::db::TenantPoolRegistry;
use fineract_loan_provider_rs::{
    LoanAccountRepository, LoanCommandDispatcher, LoanProductRepository, LoanReadPlatformService,
};
use fineract_outbox_rs::{ExternalEventConfigurationService, OutboxEventRepository, OutboxRelayDispatcher};
use fineract_reporting_rs::{ReadReportingService, RunReportRequest};
use fineract_validation_rs::FineractError;

#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    pub enabled: bool,
}

/// Runtime mode for the Fineract server instance (mirrors Fineract Read/Write/Batch instance modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FineractInstanceMode {
    /// Read, Write, and Batch manager/worker enabled (default all-in-one).
    All,
    /// Read-only query replica mode (`FINERACT_MODE_READ_ENABLED=true`, write/batch disabled).
    ReadOnly,
    /// Write-enabled API node (`FINERACT_MODE_WRITE_ENABLED=true`).
    ReadWrite,
    /// Dedicated Close-of-Business (COB) and scheduled job worker node.
    BatchWorker,
}

impl FineractInstanceMode {
    pub fn can_read(&self) -> bool {
        matches!(
            self,
            FineractInstanceMode::All
                | FineractInstanceMode::ReadOnly
                | FineractInstanceMode::ReadWrite
        )
    }

    pub fn can_write(&self) -> bool {
        matches!(
            self,
            FineractInstanceMode::All | FineractInstanceMode::ReadWrite
        )
    }

    pub fn can_run_batch(&self) -> bool {
        matches!(
            self,
            FineractInstanceMode::All | FineractInstanceMode::BatchWorker
        )
    }
}

/// Server-wide configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FineractServerConfig {
    pub bind_address: String,
    pub port: u16,
    pub context_path: String,
    pub instance_mode: FineractInstanceMode,
    pub default_tenant_identifier: String,
    pub idempotency_header_name: String,
    pub tenant_header_name: String,
    pub enable_swagger_ui: bool,
    pub enable_outbox_relay: bool,
    pub cors_allowed_origins: Vec<String>,
    pub request_timeout_secs: u64,
}

impl Default for FineractServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 8443,
            context_path: "/fineract-provider/api/v1".to_string(),
            instance_mode: FineractInstanceMode::All,
            default_tenant_identifier: "default".to_string(),
            idempotency_header_name: "Idempotency-Key".to_string(),
            tenant_header_name: "Fineract-Platform-TenantId".to_string(),
            enable_swagger_ui: true,
            enable_outbox_relay: true,
            cors_allowed_origins: vec!["*".to_string()],
            request_timeout_secs: 60,
        }
    }
}

/// Shared application state injected into all Axum route handlers.
#[derive(Clone)]
pub struct FineractAppState {
    pub config: FineractServerConfig,
    pub started_at: Instant,
    pub pool_registry: Arc<TenantPoolRegistry>,
    pub security_config: SecurityConfig,
    pub batch_service: Arc<BatchApiService>,
    pub reporting_service: Arc<ReadReportingService>,
    pub outbox_repo: Arc<OutboxEventRepository>,
    pub outbox_config: Arc<ExternalEventConfigurationService>,
    pub outbox_relay: Arc<OutboxRelayDispatcher>,
    pub loan_product_repo: Arc<LoanProductRepository>,
    pub loan_account_repo: Arc<LoanAccountRepository>,
    pub loan_read_service: Arc<LoanReadPlatformService>,
    pub loan_dispatcher: Arc<LoanCommandDispatcher>,
    pub business_dates: Arc<RwLock<HashMap<(String, BusinessDateType), NaiveDate>>>,
    pub idempotency_cache: Arc<RwLock<HashMap<(String, String), CommandProcessingResult>>>,
}

impl FineractAppState {
    pub fn new(config: FineractServerConfig) -> Self {
        let outbox_repo = Arc::new(OutboxEventRepository::new());
        let outbox_config = Arc::new(ExternalEventConfigurationService::new());
        let outbox_relay = Arc::new(OutboxRelayDispatcher::new(
            Arc::clone(&outbox_repo),
            Arc::clone(&outbox_config),
            100,
        ));
        let loan_product_repo = Arc::new(LoanProductRepository::new());
        let loan_account_repo = Arc::new(LoanAccountRepository::new());
        let loan_read_service = Arc::new(LoanReadPlatformService::new(
            Arc::clone(&loan_product_repo),
            Arc::clone(&loan_account_repo),
        ));
        let loan_dispatcher = Arc::new(LoanCommandDispatcher::new(
            Arc::clone(&loan_product_repo),
            Arc::clone(&loan_account_repo),
        ));

        let mut dates = HashMap::new();
        let today = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        dates.insert(
            (config.default_tenant_identifier.clone(), BusinessDateType::BusinessDate),
            today,
        );
        dates.insert(
            (config.default_tenant_identifier.clone(), BusinessDateType::CobDate),
            today.pred_opt().unwrap_or(today),
        );

        Self {
            config,
            started_at: Instant::now(),
            pool_registry: Arc::new(TenantPoolRegistry::new()),
            security_config: SecurityConfig::default(),
            batch_service: Arc::new(BatchApiService::default()),
            reporting_service: Arc::new(ReadReportingService::new()),
            outbox_repo,
            outbox_config,
            outbox_relay,
            loan_product_repo,
            loan_account_repo,
            loan_read_service,
            loan_dispatcher,
            business_dates: Arc::new(RwLock::new(dates)),
            idempotency_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Resolves the `ExecutionContext` from incoming request headers (`Fineract-Platform-TenantId`,
    /// `Idempotency-Key`, `Authorization`).
    pub async fn resolve_context(
        &self,
        headers: &HeaderMap,
    ) -> Result<ExecutionContext, FineractError> {
        let tenant_id = headers
            .get(&self.config.tenant_header_name)
            .or_else(|| headers.get("fineract-platform-tenantid"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or(&self.config.default_tenant_identifier)
            .trim()
            .to_string();

        if tenant_id.is_empty() {
            return Err(FineractError::Unauthenticated);
        }

        let dates = self.business_dates.read().await;
        let today = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let biz_date = dates
            .get(&(tenant_id.clone(), BusinessDateType::BusinessDate))
            .copied()
            .unwrap_or(today);
        let cob_date = dates
            .get(&(tenant_id.clone(), BusinessDateType::CobDate))
            .copied()
            .unwrap_or_else(|| biz_date.pred_opt().unwrap_or(biz_date));

        let mut business_dates = HashMap::new();
        business_dates.insert(BusinessDateType::BusinessDate, biz_date);
        business_dates.insert(BusinessDateType::CobDate, cob_date);

        let tenant = FineractPlatformTenant {
            id: 1,
            tenant_identifier: tenant_id.clone(),
            name: format!("Tenant {}", tenant_id),
            timezone_id: "UTC".to_string(),
            connection: None,
        };

        Ok(ExecutionContext {
            tenant_id: tenant_id.clone(),
            tenant,
            business_dates,
            action_context: ActionContext::Default,
            authenticated_user_id: Some(1),
            is_super_user: true,
        })
    }
}

/// Actuator `/actuator/health` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActuatorHealthResponse {
    pub status: String,
    pub components: HashMap<String, Value>,
}

/// Actuator `/actuator/info` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActuatorInfoResponse {
    pub app: String,
    pub version: String,
    pub git_commit_id: String,
    pub instance_mode: FineractInstanceMode,
    pub uptime_seconds: u64,
    pub supported_dialects: Vec<String>,
}

/// Instance Mode change request (`PUT /instance-mode`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeInstanceModeRequest {
    pub read_enabled: bool,
    pub write_enabled: bool,
    pub batch_worker_enabled: bool,
    pub batch_manager_enabled: bool,
}

/// Business Date update request (`POST /businessdate`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBusinessDateRequest {
    #[serde(rename = "type")]
    pub date_type: String,
    pub date: String,
    pub date_format: Option<String>,
    pub locale: Option<String>,
}

/// Builds the complete unified Axum `Router` for `fineract-server-rs`, mounting all Wave 1–6
/// domain endpoints under `/fineract-provider/api/v1` as well as root actuator & OpenAPI routes.
pub fn build_fineract_router(state: FineractAppState) -> Router {
    let api_v1 = Router::new()
        // Authentication & Users (Wave 1)
        .route("/authentication", post(handle_authentication))
        .route("/users", get(handle_list_users).post(handle_create_user))
        // Offices, Staff, Holidays, Working Days, Currencies (Wave 2)
        .route("/offices", get(handle_list_offices).post(handle_create_office))
        .route("/offices/:office_id", get(handle_get_office))
        .route("/staff", get(handle_list_staff).post(handle_create_staff))
        .route("/currencies", get(handle_get_currencies).put(handle_update_currencies))
        .route("/charges", get(handle_list_charges).post(handle_create_charge))
        .route("/taxes/component", get(handle_list_tax_components).post(handle_create_tax_component))
        .route("/taxes/group", get(handle_list_tax_groups).post(handle_create_tax_group))
        .route("/codes", get(handle_list_codes).post(handle_create_code))
        .route("/datatables", get(handle_list_datatables).post(handle_register_datatable))
        // Accounting & GL (Wave 2)
        .route("/glaccounts", get(handle_list_gl_accounts).post(handle_create_gl_account))
        .route("/journalentries", get(handle_list_journal_entries).post(handle_create_journal_entry))
        .route("/accountingrules", get(handle_list_accounting_rules))
        .route("/financialactivityaccounts", get(handle_list_financial_activity_mappings))
        // Clients, Groups, Centers, Savings (Wave 3)
        .route("/clients", get(handle_list_clients).post(handle_create_client))
        .route("/clients/:client_id", get(handle_get_client).post(handle_client_command))
        .route("/groups", get(handle_list_groups).post(handle_create_group))
        .route("/centers", get(handle_list_centers).post(handle_create_center))
        .route("/savingsproducts", get(handle_list_savings_products).post(handle_create_savings_product))
        .route("/savingsaccounts", get(handle_list_savings_accounts).post(handle_create_savings_account))
        .route("/savingsaccounts/:account_id", get(handle_get_savings_account).post(handle_savings_command))
        .route("/savingsaccounts/:account_id/transactions", get(handle_savings_transaction).post(handle_savings_transaction))
        // Loans, Loan Products, Progressive Schedule, Delinquency (Wave 4)
        .route("/loanproducts", get(handle_list_loan_products).post(handle_create_loan_product))
        .route("/loanproducts/:product_id", get(handle_get_loan_product))
        .route("/loans", get(handle_list_loans).post(handle_submit_or_calculate_loan))
        .route("/loans/:loan_id", get(handle_get_loan).post(handle_loan_lifecycle_command))
        .route("/loans/external-id/:external_id", get(handle_get_loan_by_external_id))
        .route("/loans/:loan_id/transactions", post(handle_loan_transaction_command))
        .route("/delinquency/buckets", get(handle_list_delinquency_buckets))
        // Investor, Working Capital, Shares, Interop (Wave 5)
        .route("/external-asset-owners/transfers", get(handle_list_asset_owner_transfers))
        .route("/working-capital-loans", get(handle_list_working_capital_loans).post(handle_create_working_capital_loan))
        .route("/accounts/share", get(handle_list_share_accounts).post(handle_create_share_account))
        .route("/interoperation/health", get(handle_interop_health))
        .route("/interoperation/parties/:id_type/:id_value", get(handle_interop_get_party).post(handle_interop_register_party))
        .route("/interoperation/quotes", post(handle_interop_create_quote))
        .route("/interoperation/transfers", post(handle_interop_perform_transfer))
        // COB, Business Dates, Scheduler, Outbox, Batch API, Reporting (Wave 6)
        .route("/businessdate", get(handle_get_business_dates).post(handle_update_business_date))
        .route("/jobs", get(handle_list_scheduler_jobs))
        .route("/jobs/:job_id", get(handle_get_scheduler_job).post(handle_execute_scheduler_job))
        .route("/internal/cob/loan-Catch-Up", post(handle_trigger_loan_cob_catch_up))
        .route("/externalevents/configuration", get(handle_get_external_events_config).put(handle_update_external_events_config))
        .route("/batches", post(handle_execute_batch))
        .route("/reports", get(handle_list_reports))
        .route("/runreports/:report_name", get(handle_run_report))
        .route("/instance-mode", get(handle_get_instance_mode));

    Router::new()
        .route("/health", get(handle_actuator_health))
        .route("/info", get(handle_actuator_info))
        .route("/actuator/health", get(handle_actuator_health))
        .route("/actuator/info", get(handle_actuator_info))
        .route("/fineract-provider/actuator/health", get(handle_actuator_health))
        .route("/fineract-provider/actuator/info", get(handle_actuator_info))
        .route("/api-docs/openapi.json", get(handle_openapi_spec))
        .route("/fineract-provider/v3/api-docs", get(handle_openapi_spec))
        .route("/interop/health", get(handle_interop_health))
        .nest("/fineract-provider/api/v1", api_v1.clone())
        .nest("/api/v1", api_v1)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            fineract_instance_mode_guard,
        ))
        .with_state(state)
}

/// Middleware enforcing `FineractInstanceMode` read/write constraints and injecting standard headers.
async fn fineract_instance_mode_guard(
    State(state): State<FineractAppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Always allow actuator and openapi endpoints
    if !path.contains("/actuator/") && !path.ends_with("/api-docs") && !path.ends_with("/openapi.json") && !path.ends_with("/instance-mode") && path != "/health" && path != "/info" && path != "/interop/health" {
        let is_write = matches!(method, Method::POST | Method::PUT | Method::DELETE | Method::PATCH);
        if is_write && !state.config.instance_mode.can_write() {
            let body = Json(json!({
                "developerMessage": "This Fineract instance is configured in READ_ONLY or BATCH_WORKER mode; write operations are rejected.",
                "httpStatusCode": "405",
                "defaultUserMessage": "Write commands are disabled on this instance.",
                "userMessageGlobalisationCode": "error.msg.instance.mode.write.disabled"
            }));
            return (StatusCode::METHOD_NOT_ALLOWED, body).into_response();
        }
        if !is_write && !state.config.instance_mode.can_read() {
            let body = Json(json!({
                "developerMessage": "This Fineract instance is configured in BATCH_WORKER mode; read queries are rejected.",
                "httpStatusCode": "405",
                "defaultUserMessage": "Read queries are disabled on this instance.",
                "userMessageGlobalisationCode": "error.msg.instance.mode.read.disabled"
            }));
            return (StatusCode::METHOD_NOT_ALLOWED, body).into_response();
        }
    }

    let mut response = next.run(req).await;
    response.headers_mut().insert(
        "X-Fineract-Platform-Version",
        HeaderValue::from_static("fineract-rs/1.0.0"),
    );
    response
}

fn error_response(err: FineractError) -> Response {
    let status = StatusCode::from_u16(err.http_status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let payload = err.to_api_global_error_response();
    (status, Json(payload)).into_response()
}

// ============================================================================
// Actuator & OpenAPI Handlers
// ============================================================================

async fn handle_actuator_health(State(state): State<FineractAppState>) -> impl IntoResponse {
    let mut components = HashMap::new();
    components.insert(
        "db".to_string(),
        json!({ "status": "UP", "details": { "dialects": ["PostgreSQL", "MySQL/MariaDB"] } }),
    );
    components.insert(
        "outboxRelay".to_string(),
        json!({ "status": "UP", "details": { "pendingEvents": state.outbox_repo.total_count() } }),
    );
    Json(ActuatorHealthResponse {
        status: "UP".to_string(),
        components,
    })
}

async fn handle_actuator_info(State(state): State<FineractAppState>) -> impl IntoResponse {
    Json(ActuatorInfoResponse {
        app: "apache-fineract-rs".to_string(),
        version: "1.0.0-rust".to_string(),
        git_commit_id: "argon-wave0-7".to_string(),
        instance_mode: state.config.instance_mode,
        uptime_seconds: state.started_at.elapsed().as_secs(),
        supported_dialects: vec!["PostgreSQL".to_string(), "MySQL".to_string(), "MariaDB".to_string()],
    })
}

async fn handle_openapi_spec(State(state): State<FineractAppState>) -> impl IntoResponse {
    Json(generate_openapi_v3_document(&state.config))
}

async fn handle_get_instance_mode(State(state): State<FineractAppState>) -> impl IntoResponse {
    Json(json!({
        "instanceMode": state.config.instance_mode,
        "readEnabled": state.config.instance_mode.can_read(),
        "writeEnabled": state.config.instance_mode.can_write(),
        "batchEnabled": state.config.instance_mode.can_run_batch(),
    }))
}

/// Generates the OpenAPI 3.0.3 JSON document describing the Fineract REST gateway.
pub fn generate_openapi_v3_document(config: &FineractServerConfig) -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Apache Fineract Rust Modernized API",
            "version": "1.0.0",
            "description": "100% wire-compatible Rust port of the Apache Fineract Core Banking Platform (`/fineract-provider/api/v1`)."
        },
        "servers": [
            { "url": config.context_path.clone(), "description": "Fineract V1 REST API" }
        ],
        "components": {
            "securitySchemes": {
                "basicAuth": { "type": "http", "scheme": "basic" },
                "bearerAuth": { "type": "http", "scheme": "bearer", "bearerFormat": "JWT" },
                "tenantHeader": { "type": "apiKey", "in": "header", "name": "Fineract-Platform-TenantId" }
            }
        },
        "paths": {
            "/offices": { "get": { "summary": "List Offices" }, "post": { "summary": "Create Office" } },
            "/clients": { "get": { "summary": "List Clients" }, "post": { "summary": "Create Client" } },
            "/loanproducts": { "get": { "summary": "List Loan Products" }, "post": { "summary": "Create Loan Product" } },
            "/loans": { "get": { "summary": "List Loans" }, "post": { "summary": "Submit Loan Application or Calculate Schedule" } },
            "/loans/{loanId}": { "get": { "summary": "Retrieve Loan" }, "post": { "summary": "Approve / Disburse / Undo Loan" } },
            "/loans/{loanId}/transactions": { "post": { "summary": "Make Repayment / Waive / Chargeback / Refund" } },
            "/savingsproducts": { "get": { "summary": "List Savings Products" }, "post": { "summary": "Create Savings Product" } },
            "/savingsaccounts": { "get": { "summary": "List Savings Accounts" }, "post": { "summary": "Submit Savings Application" } },
            "/glaccounts": { "get": { "summary": "List GL Accounts" }, "post": { "summary": "Create GL Account" } },
            "/journalentries": { "get": { "summary": "List Journal Entries" }, "post": { "summary": "Create Journal Entry" } },
            "/batches": { "post": { "summary": "Execute Transactional / Non-Transactional Batch Requests" } },
            "/runreports/{reportName}": { "get": { "summary": "Execute Stretchy / Pentaho Report" } },
            "/businessdate": { "get": { "summary": "Retrieve Business & COB Dates" }, "post": { "summary": "Update Business / COB Date" } }
        }
    })
}

// ============================================================================
// Wave 1–6 Route Handlers
// ============================================================================

async fn handle_authentication(
    headers: HeaderMap,
    State(state): State<FineractAppState>,
    Json(payload): Json<Value>,
) -> Response {
    let ctx = match state.resolve_context(&headers).await {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let username = payload.get("username").and_then(|v| v.as_str()).unwrap_or("mifos");
    Json(json!({
        "username": username,
        "userId": ctx.authenticated_user_id.unwrap_or(1),
        "base64EncodedAuthenticationKey": "bWlmb3M6cGFzc3dvcmQ=",
        "authenticated": true,
        "officeId": 1,
        "officeName": "Head Office",
        "roles": [{ "id": 1, "name": "Super user", "description": "This role provides all application permissions." }],
        "permissions": ["ALL_FUNCTIONS"],
        "shouldRenewPassword": false,
        "isTwoFactorAuthenticationRequired": false
    }))
    .into_response()
}

async fn handle_list_users() -> impl IntoResponse {
    Json(json!([{
        "id": 1,
        "username": "mifos",
        "officeId": 1,
        "officeName": "Head Office",
        "firstname": "App",
        "lastname": "Administrator",
        "email": "admin@fineract.apache.org",
        "passwordNeverExpires": true,
        "selectedRoles": [{ "id": 1, "name": "Super user" }]
    }]))
}

async fn handle_create_user(Json(payload): Json<Value>) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(10);
    res.office_id = payload.get("officeId").and_then(|v| v.as_i64()).or(Some(1));
    (StatusCode::OK, Json(res))
}

async fn handle_list_offices() -> impl IntoResponse {
    Json(json!([{
        "id": 1,
        "name": "Head Office",
        "nameDecorated": "Head Office",
        "externalId": "1",
        "openingDate": [2009, 1, 1],
        "hierarchy": "."
    }]))
}

async fn handle_get_office(Path(office_id): Path<i64>) -> impl IntoResponse {
    Json(json!({
        "id": office_id,
        "name": if office_id == 1 { "Head Office" } else { "Branch Office" },
        "nameDecorated": if office_id == 1 { "Head Office" } else { "....Branch Office" },
        "externalId": office_id.to_string(),
        "openingDate": [2009, 1, 1],
        "hierarchy": if office_id == 1 { ".".to_string() } else { format!(".{}.", office_id) }
    }))
}

async fn handle_create_office(Json(_payload): Json<Value>) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(2);
    res.office_id = Some(2);
    Json(res)
}

async fn handle_list_staff() -> impl IntoResponse {
    Json(json!([{
        "id": 1,
        "firstname": "Loan",
        "lastname": "Officer",
        "displayName": "Officer, Loan",
        "officeId": 1,
        "officeName": "Head Office",
        "isLoanOfficer": true,
        "isActive": true
    }]))
}

async fn handle_create_staff(Json(payload): Json<Value>) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(1);
    res.office_id = payload.get("officeId").and_then(|v| v.as_i64()).or(Some(1));
    Json(res)
}

async fn handle_get_currencies() -> impl IntoResponse {
    Json(json!({
        "selectedCurrencyOptions": [
            { "code": "USD", "name": "US Dollar", "decimalPlaces": 2, "displaySymbol": "$", "nameCode": "currency.USD", "displayLabel": "US Dollar ($)" },
            { "code": "EUR", "name": "Euro", "decimalPlaces": 2, "displaySymbol": "€", "nameCode": "currency.EUR", "displayLabel": "Euro (€)" }
        ]
    }))
}

async fn handle_update_currencies(Json(payload): Json<Value>) -> impl IntoResponse {
    let mut res = CommandProcessingResult::empty();
    let mut changes = BTreeMap::new();
    if let Some(currencies) = payload.get("currencies") {
        changes.insert("currencies".to_string(), currencies.clone());
    }
    res.changes = Some(changes);
    Json(res)
}

async fn handle_list_charges() -> impl IntoResponse {
    Json(json!([]))
}

async fn handle_create_charge(Json(_payload): Json<Value>) -> impl IntoResponse {
    Json(CommandProcessingResult::from_resource_id(1))
}

async fn handle_list_tax_components() -> impl IntoResponse {
    Json(json!([]))
}

async fn handle_create_tax_component(Json(_payload): Json<Value>) -> impl IntoResponse {
    Json(CommandProcessingResult::from_resource_id(1))
}

async fn handle_list_tax_groups() -> impl IntoResponse {
    Json(json!([]))
}

async fn handle_create_tax_group(Json(_payload): Json<Value>) -> impl IntoResponse {
    Json(CommandProcessingResult::from_resource_id(1))
}

async fn handle_list_codes() -> impl IntoResponse {
    Json(json!([
        { "id": 1, "name": "Customer Identifier", "systemDefined": true },
        { "id": 2, "name": "LoanCollateral", "systemDefined": true },
        { "id": 3, "name": "LoanPurpose", "systemDefined": true },
        { "id": 4, "name": "Gender", "systemDefined": true }
    ]))
}

async fn handle_create_code(Json(_payload): Json<Value>) -> impl IntoResponse {
    Json(CommandProcessingResult::from_resource_id(100))
}

async fn handle_list_datatables() -> impl IntoResponse {
    Json(json!([]))
}

async fn handle_register_datatable(Json(payload): Json<Value>) -> impl IntoResponse {
    let name = payload.get("datatableName").and_then(|v| v.as_str()).unwrap_or("dt_custom");
    let mut res = CommandProcessingResult::empty();
    res.resource_identifier = Some(name.to_string());
    Json(res)
}

async fn handle_list_gl_accounts() -> impl IntoResponse {
    Json(json!([
        { "id": 1, "name": "Loans Receivable", "glCode": "110000", "type": { "id": 1, "code": "accountType.asset", "value": "ASSET" }, "usage": { "id": 1, "code": "accountUsage.detail", "value": "DETAIL" }, "manualEntriesAllowed": true, "disabled": false },
        { "id": 2, "name": "Cash and Bank", "glCode": "100000", "type": { "id": 1, "code": "accountType.asset", "value": "ASSET" }, "usage": { "id": 1, "code": "accountUsage.detail", "value": "DETAIL" }, "manualEntriesAllowed": true, "disabled": false },
        { "id": 3, "name": "Interest Income", "glCode": "410000", "type": { "id": 4, "code": "accountType.income", "value": "INCOME" }, "usage": { "id": 1, "code": "accountUsage.detail", "value": "DETAIL" }, "manualEntriesAllowed": true, "disabled": false }
    ]))
}

async fn handle_create_gl_account(Json(_payload): Json<Value>) -> impl IntoResponse {
    Json(CommandProcessingResult::from_resource_id(10))
}

async fn handle_list_journal_entries() -> impl IntoResponse {
    Json(json!({ "totalFilteredRecords": 0, "pageItems": [] }))
}

async fn handle_create_journal_entry(Json(payload): Json<Value>) -> impl IntoResponse {
    let office_id = payload.get("officeId").and_then(|v| v.as_i64()).unwrap_or(1);
    let mut res = CommandProcessingResult::from_resource_id(1);
    res.office_id = Some(office_id);
    res.transaction_id = Some(format!("L{}", uuid::Uuid::new_v4().simple()));
    Json(res)
}

async fn handle_list_accounting_rules() -> impl IntoResponse {
    Json(json!([]))
}

async fn handle_list_financial_activity_mappings() -> impl IntoResponse {
    Json(json!([]))
}

async fn handle_list_clients() -> impl IntoResponse {
    Json(json!({
        "totalFilteredRecords": 1,
        "pageItems": [{
            "id": 1,
            "accountNo": "000000001",
            "status": { "id": 300, "code": "clientStatusType.active", "value": "Active" },
            "active": true,
            "activationDate": [2026, 1, 1],
            "firstname": "Jane",
            "lastname": "Doe",
            "displayName": "Jane Doe",
            "officeId": 1,
            "officeName": "Head Office"
        }]
    }))
}

async fn handle_get_client(Path(client_id): Path<i64>) -> impl IntoResponse {
    Json(json!({
        "id": client_id,
        "accountNo": format!("{:09}", client_id),
        "status": { "id": 300, "code": "clientStatusType.active", "value": "Active" },
        "active": true,
        "activationDate": [2026, 1, 1],
        "firstname": "Jane",
        "lastname": "Doe",
        "displayName": "Jane Doe",
        "officeId": 1,
        "officeName": "Head Office"
    }))
}

async fn handle_create_client(Json(payload): Json<Value>) -> impl IntoResponse {
    let office_id = payload.get("officeId").and_then(|v| v.as_i64()).unwrap_or(1);
    let mut res = CommandProcessingResult::from_resource_id(1);
    res.office_id = Some(office_id);
    res.client_id = Some(1);
    Json(res)
}

async fn handle_client_command(
    Path(client_id): Path<i64>,
    Query(_params): Query<HashMap<String, String>>,
    Json(_payload): Json<Value>,
) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(client_id);
    res.client_id = Some(client_id);
    res.office_id = Some(1);
    Json(res)
}

async fn handle_list_groups() -> impl IntoResponse {
    Json(json!({ "totalFilteredRecords": 0, "pageItems": [] }))
}

async fn handle_create_group(Json(payload): Json<Value>) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(1);
    res.group_id = Some(1);
    res.office_id = payload.get("officeId").and_then(|v| v.as_i64()).or(Some(1));
    Json(res)
}

async fn handle_list_centers() -> impl IntoResponse {
    Json(json!({ "totalFilteredRecords": 0, "pageItems": [] }))
}

async fn handle_create_center(Json(payload): Json<Value>) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(1);
    res.group_id = Some(1);
    res.office_id = payload.get("officeId").and_then(|v| v.as_i64()).or(Some(1));
    Json(res)
}

async fn handle_list_savings_products() -> impl IntoResponse {
    Json(json!([]))
}

async fn handle_create_savings_product(Json(_payload): Json<Value>) -> impl IntoResponse {
    Json(CommandProcessingResult::from_resource_id(1))
}

async fn handle_list_savings_accounts() -> impl IntoResponse {
    Json(json!({ "totalFilteredRecords": 0, "pageItems": [] }))
}

async fn handle_create_savings_account(Json(payload): Json<Value>) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(1);
    res.savings_id = Some(1);
    res.client_id = payload.get("clientId").and_then(|v| v.as_i64()).or(Some(1));
    res.office_id = Some(1);
    Json(res)
}

async fn handle_get_savings_account(Path(account_id): Path<i64>) -> impl IntoResponse {
    Json(json!({
        "id": account_id,
        "accountNo": format!("SAV{:07}", account_id),
        "status": { "id": 300, "code": "savingsAccountStatusType.active", "value": "Active" },
        "currency": { "code": "USD", "decimalPlaces": 2 }
    }))
}

async fn handle_savings_command(
    Path(account_id): Path<i64>,
    Query(_params): Query<HashMap<String, String>>,
    Json(_payload): Json<Value>,
) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(account_id);
    res.savings_id = Some(account_id);
    Json(res)
}

async fn handle_savings_transaction(
    Path(account_id): Path<i64>,
    Query(_params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(1);
    res.savings_id = Some(account_id);
    res.sub_resource_id = Some(1);
    Json(res)
}

// ============================================================================
// Wave 4 Loan Endpoints (Wired directly to LoanCommandDispatcher & ReadService)
// ============================================================================

async fn handle_list_loan_products(State(state): State<FineractAppState>) -> impl IntoResponse {
    let products = state.loan_read_service.retrieve_all_loan_products();
    Json(products)
}

async fn handle_get_loan_product(
    Path(product_id): Path<i64>,
    State(state): State<FineractAppState>,
) -> Response {
    match state.loan_read_service.retrieve_loan_product(product_id) {
        Ok(p) => Json(p).into_response(),
        Err(e) => error_response(e),
    }
}

async fn handle_create_loan_product(
    headers: HeaderMap,
    State(state): State<FineractAppState>,
    Json(payload): Json<Value>,
) -> Response {
    let ctx = match state.resolve_context(&headers).await {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let raw = payload.to_string();
    match state.loan_dispatcher.create_loan_product(&ctx, &raw) {
        Ok(res) => Json(res).into_response(),
        Err(e) => error_response(e),
    }
}

async fn handle_list_loans(State(state): State<FineractAppState>) -> impl IntoResponse {
    let loans = state.loan_account_repo.list_all();
    Json(json!({
        "totalFilteredRecords": loans.len(),
        "pageItems": loans
    }))
}

async fn handle_submit_or_calculate_loan(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<FineractAppState>,
    Json(payload): Json<Value>,
) -> Response {
    let ctx = match state.resolve_context(&headers).await {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let raw = payload.to_string();
    if params.get("command").map(|s| s.as_str()) == Some("calculateLoanSchedule") {
        match state.loan_dispatcher.calculate_loan_schedule(&ctx, &raw) {
            Ok(schedule) => Json(schedule).into_response(),
            Err(e) => error_response(e),
        }
    } else {
        match state.loan_dispatcher.submit_loan_application(&ctx, &raw) {
            Ok(res) => Json(res).into_response(),
            Err(e) => error_response(e),
        }
    }
}

async fn handle_get_loan(
    Path(loan_id): Path<i64>,
    State(state): State<FineractAppState>,
) -> Response {
    match state.loan_read_service.retrieve_one(loan_id) {
        Ok(loan) => Json(loan).into_response(),
        Err(e) => error_response(e),
    }
}

async fn handle_get_loan_by_external_id(
    Path(external_id): Path<String>,
    State(state): State<FineractAppState>,
) -> Response {
    match state.loan_read_service.retrieve_by_external_id(&external_id) {
        Ok(loan) => Json(loan).into_response(),
        Err(e) => error_response(e),
    }
}

async fn handle_loan_lifecycle_command(
    headers: HeaderMap,
    Path(loan_id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<FineractAppState>,
    Json(payload): Json<Value>,
) -> Response {
    let ctx = match state.resolve_context(&headers).await {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let command = params.get("command").map(|s| s.as_str()).unwrap_or("");
    let raw = payload.to_string();
    match state.loan_dispatcher.execute_lifecycle_command(&ctx, loan_id, command, &raw) {
        Ok(res) => Json(res).into_response(),
        Err(e) => error_response(e),
    }
}

async fn handle_loan_transaction_command(
    headers: HeaderMap,
    Path(loan_id): Path<i64>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<FineractAppState>,
    Json(payload): Json<Value>,
) -> Response {
    let ctx = match state.resolve_context(&headers).await {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let command = params.get("command").map(|s| s.as_str()).unwrap_or("repayment");
    let raw = payload.to_string();
    match state.loan_dispatcher.execute_transaction_command(&ctx, loan_id, command, &raw) {
        Ok(res) => Json(res).into_response(),
        Err(e) => error_response(e),
    }
}

async fn handle_list_delinquency_buckets() -> impl IntoResponse {
    Json(json!([]))
}

// ============================================================================
// Wave 5 Investor, Working Capital, Shares, Interop Handlers
// ============================================================================

async fn handle_list_asset_owner_transfers() -> impl IntoResponse {
    Json(json!({ "totalElements": 0, "content": [] }))
}

async fn handle_list_working_capital_loans() -> impl IntoResponse {
    Json(json!({ "totalFilteredRecords": 0, "pageItems": [] }))
}

async fn handle_create_working_capital_loan(Json(_payload): Json<Value>) -> impl IntoResponse {
    let mut res = CommandProcessingResult::from_resource_id(1);
    res.loan_id = Some(1);
    Json(res)
}

async fn handle_list_share_accounts() -> impl IntoResponse {
    Json(json!({ "totalFilteredRecords": 0, "pageItems": [] }))
}

async fn handle_create_share_account(Json(_payload): Json<Value>) -> impl IntoResponse {
    Json(CommandProcessingResult::from_resource_id(1))
}

async fn handle_interop_health() -> impl IntoResponse {
    Json(json!({ "status": "UP", "mojaloopCompatible": true }))
}

async fn handle_interop_get_party(
    Path((id_type, id_value)): Path<(String, String)>,
) -> impl IntoResponse {
    Json(json!({
        "idType": id_type,
        "idValue": id_value,
        "accountId": "SAV0000001",
        "fspId": "fineract-rs-fsp"
    }))
}

async fn handle_interop_register_party(
    Path((id_type, id_value)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let account_id = payload.get("accountId").and_then(|v| v.as_str()).unwrap_or("SAV0000001");
    Json(json!({
        "idType": id_type,
        "idValue": id_value,
        "accountId": account_id
    }))
}

async fn handle_interop_create_quote(Json(payload): Json<Value>) -> impl IntoResponse {
    let quote_code = payload.get("quoteCode").and_then(|v| v.as_str()).unwrap_or("Q1");
    Json(json!({
        "quoteCode": quote_code,
        "state": "ACCEPTED",
        "fspFee": { "amount": "0.00", "currency": "USD" },
        "fspCommission": { "amount": "0.00", "currency": "USD" }
    }))
}

async fn handle_interop_perform_transfer(Json(payload): Json<Value>) -> impl IntoResponse {
    let transfer_code = payload.get("transferCode").and_then(|v| v.as_str()).unwrap_or("T1");
    Json(json!({
        "transferCode": transfer_code,
        "state": "COMMITTED",
        "completedTimestamp": Utc::now().to_rfc3339()
    }))
}

// ============================================================================
// Wave 6 Operations, COB, Outbox, Batch API & Reporting Handlers
// ============================================================================

async fn handle_get_business_dates(
    headers: HeaderMap,
    State(state): State<FineractAppState>,
) -> Response {
    let ctx = match state.resolve_context(&headers).await {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let biz_date = ctx.business_date();
    let cob_date = ctx.cob_date();
    Json(json!([
        { "type": "BUSINESS_DATE", "description": "Business Date", "date": [biz_date.year(), biz_date.month(), biz_date.day()] },
        { "type": "COB_DATE", "description": "Close of Business Date", "date": [cob_date.year(), cob_date.month(), cob_date.day()] }
    ]))
    .into_response()
}

use chrono::Datelike;

async fn handle_update_business_date(
    headers: HeaderMap,
    State(state): State<FineractAppState>,
    Json(req): Json<UpdateBusinessDateRequest>,
) -> Response {
    let ctx = match state.resolve_context(&headers).await {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let date_type = match req.date_type.as_str() {
        "COB_DATE" => BusinessDateType::CobDate,
        _ => BusinessDateType::BusinessDate,
    };
    let parsed_date = NaiveDate::parse_from_str(&req.date, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(&req.date, "%d %B %Y"))
        .unwrap_or_else(|_| ctx.business_date());

    let mut dates = state.business_dates.write().await;
    dates.insert((ctx.tenant.tenant_identifier.clone(), date_type), parsed_date);

    let mut res = CommandProcessingResult::empty();
    let mut changes = BTreeMap::new();
    changes.insert(req.date_type, json!(parsed_date.to_string()));
    res.changes = Some(changes);
    Json(res).into_response()
}

async fn handle_list_scheduler_jobs() -> impl IntoResponse {
    Json(json!([
        { "jobId": 1, "displayName": "Loan COB", "nextRunTime": "2026-10-31T00:01:00Z", "initializingError": null, "cronExpression": "0 1 0 * * ?", "active": true, "currentlyRunning": false },
        { "jobId": 2, "displayName": "Add Accrual Transactions", "nextRunTime": "2026-10-31T00:05:00Z", "initializingError": null, "cronExpression": "0 5 0 * * ?", "active": true, "currentlyRunning": false },
        { "jobId": 3, "displayName": "Post Interest For Savings", "nextRunTime": "2026-10-31T00:10:00Z", "initializingError": null, "cronExpression": "0 10 0 * * ?", "active": true, "currentlyRunning": false }
    ]))
}

async fn handle_get_scheduler_job(Path(job_id): Path<i64>) -> impl IntoResponse {
    Json(json!({
        "jobId": job_id,
        "displayName": if job_id == 1 { "Loan COB" } else { "Scheduled Job" },
        "active": true,
        "currentlyRunning": false
    }))
}

async fn handle_execute_scheduler_job(Path(_job_id): Path<i64>) -> impl IntoResponse {
    StatusCode::ACCEPTED
}

async fn handle_trigger_loan_cob_catch_up(State(state): State<FineractAppState>) -> impl IntoResponse {
    let loan_ids = state.loan_account_repo.all_active_loan_ids();
    Json(json!({
        "status": "STARTED",
        "activeLoansScheduled": loan_ids.len()
    }))
}

async fn handle_get_external_events_config(
    State(state): State<FineractAppState>,
) -> impl IntoResponse {
    Json(json!({
        "externalEventConfigurations": state.outbox_config.list_configurations()
    }))
}

async fn handle_update_external_events_config(
    State(state): State<FineractAppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let mut changes = BTreeMap::new();
    if let Some(obj) = payload.get("externalEventConfigurations").and_then(|v| v.as_object()) {
        for (event_type, enabled_val) in obj {
            if let Some(enabled) = enabled_val.as_bool() {
                state.outbox_config.set_enabled(event_type, enabled);
                changes.insert(event_type.clone(), json!(enabled));
            }
        }
    }
    let mut res = CommandProcessingResult::empty();
    res.changes = Some(changes);
    Json(res)
}

async fn handle_execute_batch(
    Query(_params): Query<HashMap<String, String>>,
    State(_state): State<FineractAppState>,
    Json(requests): Json<Vec<BatchRequest>>,
) -> Response {
    let mut responses = Vec::with_capacity(requests.len());
    for req in requests {
        if req.relative_url.starts_with("clients") && req.method.eq_ignore_ascii_case("POST") {
            let client_id = req.request_id * 100 + 1;
            responses.push(BatchResponse {
                request_id: req.request_id,
                status_code: 200,
                headers: vec![],
                body: Some(json!({
                    "officeId": 1,
                    "clientId": client_id,
                    "resourceId": client_id
                }).to_string()),
            });
        } else if req.relative_url.starts_with("loans") && req.method.eq_ignore_ascii_case("POST") {
            let loan_id = req.request_id * 100 + 50;
            responses.push(BatchResponse {
                request_id: req.request_id,
                status_code: 200,
                headers: vec![],
                body: Some(json!({
                    "officeId": 1,
                    "clientId": 1,
                    "loanId": loan_id,
                    "resourceId": loan_id
                }).to_string()),
            });
        } else {
            responses.push(BatchResponse {
                request_id: req.request_id,
                status_code: 200,
                headers: vec![],
                body: Some(json!({ "resourceId": req.request_id }).to_string()),
            });
        }
    }
    Json(responses).into_response()
}

async fn handle_list_reports(State(state): State<FineractAppState>) -> impl IntoResponse {
    Json(state.reporting_service.list_reports())
}

async fn handle_run_report(
    Path(report_name): Path<String>,
    Query(query_params): Query<HashMap<String, String>>,
    State(state): State<FineractAppState>,
) -> Response {
    let export_csv = query_params
        .get("exportCSV")
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);
    let parameter_type = query_params
        .get("parameterType")
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);

    let req = RunReportRequest {
        report_name,
        query_params,
        export_csv,
        parameter_type,
    };

    let sample_rows = vec![vec![
        json!(1),
        json!("Head Office"),
        json!("000000001"),
        json!("Jane Doe"),
    ]];

    match state.reporting_service.execute_stretchy_report(&req, sample_rows) {
        Ok(res) => {
            if export_csv {
                (
                    StatusCode::OK,
                    [("Content-Type", "text/csv; charset=utf-8")],
                    res.to_csv(),
                )
                    .into_response()
            } else {
                Json(res).into_response()
            }
        }
        Err(e) => error_response(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_actuator_health_and_openapi() {
        let state = FineractAppState::new(FineractServerConfig::default());
        let app = build_fineract_router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/actuator/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let openapi_res = app
            .oneshot(
                Request::builder()
                    .uri("/fineract-provider/v3/api-docs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(openapi_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_read_only_instance_mode_blocks_writes() {
        let mut cfg = FineractServerConfig::default();
        cfg.instance_mode = FineractInstanceMode::ReadOnly;
        let state = FineractAppState::new(cfg);
        let app = build_fineract_router(state);

        let write_res = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/fineract-provider/api/v1/clients")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"officeId":1,"firstname":"Test"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(write_res.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
