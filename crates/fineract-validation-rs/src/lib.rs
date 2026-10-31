//! `fineract-validation-rs` — Wire-compatible Apache Fineract validation and error framework.
//!
//! Ported from:
//! - `org.apache.fineract.infrastructure.core.data.DataValidatorBuilder`
//! - `org.apache.fineract.infrastructure.core.data.ApiParameterError`
//! - `org.apache.fineract.infrastructure.core.data.ApiErrorMessageArg`
//! - `org.apache.fineract.infrastructure.core.data.ApiGlobalErrorResponse`
//! - `org.apache.fineract.infrastructure.core.exception.PlatformApiDataValidationException`
//! - `org.apache.fineract.infrastructure.core.exception.AbstractPlatformDomainRuleException`
//! - `org.apache.fineract.validation.constraints.*`

use std::borrow::Cow;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use regex::Regex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Separator used by `DataValidatorBuilder::validate_string_for`.
pub const VALID_INPUT_SEPARATOR: &str = "_";

/// Single argument attached to an [`ApiParameterError`].
///
/// Wire-compatible with `org.apache.fineract.infrastructure.core.data.ApiErrorMessageArg`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorArg {
    pub value: serde_json::Value,
}

/// Alias matching the upstream Java class name `ApiErrorMessageArg`.
pub type ApiErrorMessageArg = ApiErrorArg;

impl ApiErrorArg {
    /// Creates an [`ApiErrorArg`] from any JSON value.
    pub fn new(value: impl Into<serde_json::Value>) -> Self {
        Self {
            value: value.into(),
        }
    }

    /// Creates an [`ApiErrorArg`] from a `NaiveDate`, formatted as `yyyy-MM-dd` matching Java `ApiParameterError`.
    pub fn from_date(date: NaiveDate) -> Self {
        Self {
            value: serde_json::Value::String(date.format("%Y-%m-%d").to_string()),
        }
    }

    /// Creates an [`ApiErrorArg`] from a `Decimal`.
    pub fn from_decimal(dec: Decimal) -> Self {
        Self {
            value: serde_json::json!(dec),
        }
    }
}

impl From<serde_json::Value> for ApiErrorArg {
    fn from(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl From<&str> for ApiErrorArg {
    fn from(value: &str) -> Self {
        Self {
            value: serde_json::Value::String(value.to_string()),
        }
    }
}

impl From<String> for ApiErrorArg {
    fn from(value: String) -> Self {
        Self {
            value: serde_json::Value::String(value),
        }
    }
}

impl From<i32> for ApiErrorArg {
    fn from(value: i32) -> Self {
        Self {
            value: serde_json::Value::Number(value.into()),
        }
    }
}

impl From<i64> for ApiErrorArg {
    fn from(value: i64) -> Self {
        Self {
            value: serde_json::Value::Number(value.into()),
        }
    }
}

impl From<usize> for ApiErrorArg {
    fn from(value: usize) -> Self {
        Self {
            value: serde_json::Value::Number(value.into()),
        }
    }
}

impl From<u32> for ApiErrorArg {
    fn from(value: u32) -> Self {
        Self {
            value: serde_json::Value::Number(value.into()),
        }
    }
}

impl From<bool> for ApiErrorArg {
    fn from(value: bool) -> Self {
        Self {
            value: serde_json::Value::Bool(value),
        }
    }
}

impl From<NaiveDate> for ApiErrorArg {
    fn from(date: NaiveDate) -> Self {
        Self::from_date(date)
    }
}

impl From<Decimal> for ApiErrorArg {
    fn from(dec: Decimal) -> Self {
        Self::from_decimal(dec)
    }
}

/// Wire-compatible parameter validation or domain error item.
///
/// Ported from `org.apache.fineract.infrastructure.core.data.ApiParameterError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiParameterError {
    pub developer_message: String,
    pub default_user_message: String,
    pub user_message_globalisation_code: String,
    pub parameter_name: String,
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub args: Vec<ApiErrorArg>,
}

impl ApiParameterError {
    /// Creates a new [`ApiParameterError`] with explicit fields.
    pub fn new(
        user_message_globalisation_code: impl Into<String>,
        developer_message: impl Into<String>,
        default_user_message: impl Into<String>,
        parameter_name: impl Into<String>,
        value: Option<serde_json::Value>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        Self {
            developer_message: developer_message.into(),
            default_user_message: default_user_message.into(),
            user_message_globalisation_code: user_message_globalisation_code.into(),
            parameter_name: parameter_name.into(),
            value,
            args,
        }
    }

    /// Port of `ApiParameterError.generalError(globalisationMessageCode, defaultUserMessage, args...)`.
    pub fn general_error(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let msg = default_user_message.into();
        Self {
            developer_message: msg.clone(),
            default_user_message: msg,
            user_message_globalisation_code: globalisation_message_code.into(),
            parameter_name: "id".to_string(),
            value: None,
            args,
        }
    }

    /// Port of `ApiParameterError.resourceIdentifierNotFound(globalisationMessageCode, defaultUserMessage, args...)`.
    pub fn resource_identifier_not_found(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let msg = default_user_message.into();
        Self {
            developer_message: msg.clone(),
            default_user_message: msg,
            user_message_globalisation_code: globalisation_message_code.into(),
            parameter_name: "id".to_string(),
            value: None,
            args,
        }
    }

    /// Port of `ApiParameterError.parameterError(globalisationMessageCode, defaultUserMessage, parameterName, args...)`.
    pub fn parameter_error(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        parameter_name: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let msg = default_user_message.into();
        Self {
            developer_message: msg.clone(),
            default_user_message: msg,
            user_message_globalisation_code: globalisation_message_code.into(),
            parameter_name: parameter_name.into(),
            value: None,
            args,
        }
    }

    /// Port of `ApiParameterError.parameterError(globalisationMessageCode, developerMessage, defaultUserMessage, parameterName, args...)`.
    pub fn parameter_error_with_developer_message(
        globalisation_message_code: impl Into<String>,
        developer_message: impl Into<String>,
        default_user_message: impl Into<String>,
        parameter_name: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        Self {
            developer_message: developer_message.into(),
            default_user_message: default_user_message.into(),
            user_message_globalisation_code: globalisation_message_code.into(),
            parameter_name: parameter_name.into(),
            value: None,
            args,
        }
    }

    /// Port of `ApiParameterError.parameterErrorWithValue(...)`.
    pub fn parameter_error_with_value(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        parameter_name: impl Into<String>,
        value: Option<serde_json::Value>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let msg = default_user_message.into();
        Self {
            developer_message: msg.clone(),
            default_user_message: msg,
            user_message_globalisation_code: globalisation_message_code.into(),
            parameter_name: parameter_name.into(),
            value,
            args,
        }
    }
}

impl std::fmt::Display for ApiParameterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.developer_message)
    }
}

/// Top-level HTTP error envelope returned by Fineract REST endpoints.
///
/// Ported from `org.apache.fineract.infrastructure.core.data.ApiGlobalErrorResponse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGlobalErrorResponse {
    pub developer_message: String,
    pub http_status_code: String,
    pub default_user_message: String,
    pub user_message_globalisation_code: String,
    #[serde(default)]
    pub errors: Vec<ApiParameterError>,
}

impl ApiGlobalErrorResponse {
    /// Creates an [`ApiGlobalErrorResponse`] with explicit values.
    pub fn create(
        status_code: u16,
        msg_code: impl Into<String>,
        developer_message: impl Into<String>,
        default_user_message: impl Into<String>,
        errors: Vec<ApiParameterError>,
    ) -> Self {
        Self {
            developer_message: developer_message.into(),
            http_status_code: status_code.to_string(),
            default_user_message: default_user_message.into(),
            user_message_globalisation_code: msg_code.into(),
            errors,
        }
    }

    /// Port of `ApiGlobalErrorResponse.unAuthenticated()`.
    pub fn un_authenticated() -> Self {
        Self::create(
            401,
            "error.msg.not.authenticated",
            "Invalid authentication details were passed in api request.",
            "Unauthenticated. Please login.",
            Vec::new(),
        )
    }

    /// Alias for [`ApiGlobalErrorResponse::un_authenticated`].
    pub fn unauthenticated() -> Self {
        Self::un_authenticated()
    }

    /// Port of `ApiGlobalErrorResponse.invalidTenantIdentifier()`.
    pub fn invalid_tenant_identifier() -> Self {
        Self::create(
            401,
            "error.msg.invalid.tenant.identifier",
            "Invalid tenant details were passed in api request.",
            "Invalid tenant identifier provided with request.",
            Vec::new(),
        )
    }

    /// Port of `ApiGlobalErrorResponse.invalidInstanceTypeMethod(method)`.
    pub fn invalid_instance_type_method(method: &str) -> Self {
        Self::create(
            405,
            "error.msg.invalid.instance.type",
            format!("Invalid instance type called in api request for the method {method}"),
            format!("Invalid method {method} used with request to this instance type."),
            Vec::new(),
        )
    }

    /// Port of `ApiGlobalErrorResponse.loanIsLocked(loanId)`.
    pub fn loan_is_locked(loan_id: i64) -> Self {
        let msg = format!("Loan is locked by the COB job. Loan ID: {loan_id}");
        Self::create(409, "error.msg.loan.locked", msg.clone(), msg, Vec::new())
    }

    /// Port of `ApiGlobalErrorResponse.conflict(type, identifier)`.
    pub fn conflict(conflict_type: Option<&str>, identifier: Option<&str>) -> Self {
        let resolved_type = conflict_type.unwrap_or("unknown");
        let mut details = match conflict_type {
            Some(t) => format!(" on {t}"),
            None => String::new(),
        };
        if let Some(id) = identifier {
            details.push_str(&format!(" [{id}]"));
        }
        let msg = format!(
            "The server is currently unable to handle the request due to concurrent modification {details}, please try again"
        );
        Self::create(
            409,
            format!("error.msg.platform.service.{resolved_type}.conflict"),
            msg.clone(),
            msg,
            Vec::new(),
        )
    }

    /// Port of `ApiGlobalErrorResponse.unAuthorized(defaultUserMessage)`.
    pub fn un_authorized(default_user_message: impl Into<String>) -> Self {
        let msg_code = "error.msg.not.authorized";
        let errors = vec![ApiParameterError::general_error(
            msg_code,
            default_user_message,
            Vec::new(),
        )];
        Self::create(
            403,
            msg_code,
            "The user associated with credentials passed on this request does not have sufficient privileges to perform this action.",
            "Insufficient privileges to perform this action.",
            errors,
        )
    }

    /// Alias for [`ApiGlobalErrorResponse::un_authorized`].
    pub fn unauthorized(default_user_message: impl Into<String>) -> Self {
        Self::un_authorized(default_user_message)
    }

    /// Port of `ApiGlobalErrorResponse.domainRuleViolation(...)`.
    pub fn domain_rule_violation(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let errors = vec![ApiParameterError::general_error(
            globalisation_message_code,
            default_user_message,
            args,
        )];
        Self::create(
            403,
            "validation.msg.domain.rule.violation",
            "Request was understood but caused a domain rule violation.",
            "Errors contain reason for domain rule violation.",
            errors,
        )
    }

    /// Port of `ApiGlobalErrorResponse.notImplemented(...)`.
    pub fn not_implemented(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let errors = vec![ApiParameterError::general_error(
            globalisation_message_code,
            default_user_message,
            args,
        )];
        Self::create(
            501,
            "validation.msg.not.implemented",
            "Request was understood but not implemented.",
            "Errors contain reason for not implemented violation.",
            errors,
        )
    }

    /// Port of `ApiGlobalErrorResponse.dataIntegrityError(...)`.
    pub fn data_integrity_error(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        parameter_name: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let code = globalisation_message_code.into();
        let user_msg = default_user_message.into();
        let dev_msg = "The request caused a data integrity issue to be fired by the database.";
        let errors = vec![ApiParameterError::parameter_error_with_developer_message(
            code.clone(),
            dev_msg,
            user_msg.clone(),
            parameter_name,
            args,
        )];
        Self::create(403, code, dev_msg, user_msg, errors)
    }

    /// Port of `ApiGlobalErrorResponse.notFound(...)`.
    pub fn not_found(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let msg = "The requested resource is not available.";
        let errors = vec![ApiParameterError::resource_identifier_not_found(
            globalisation_message_code,
            default_user_message,
            args,
        )];
        Self::create(404, "error.msg.resource.not.found", msg, msg, errors)
    }

    /// Port of `ApiGlobalErrorResponse.badClientRequest(...)`.
    pub fn bad_client_request(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        errors: Vec<ApiParameterError>,
    ) -> Self {
        Self::create(
            400,
            globalisation_message_code,
            "The request was invalid. This typically will happen due to validation errors which are provided.",
            default_user_message,
            errors,
        )
    }

    /// Port of `ApiGlobalErrorResponse.jobIsDisabled(...)`.
    pub fn job_is_disabled(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
    ) -> Self {
        let msg = default_user_message.into();
        Self::create(403, globalisation_message_code, msg.clone(), msg, Vec::new())
    }

    /// Port of `ApiGlobalErrorResponse.serverSideError(...)`.
    pub fn server_side_error(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let msg = "An unexpected error occurred on the platform server.";
        let errors = vec![ApiParameterError::general_error(
            globalisation_message_code,
            default_user_message,
            args,
        )];
        Self::create(500, "error.msg.platform.server.side.error", msg, msg, errors)
    }

    /// Port of `ApiGlobalErrorResponse.serviceUnavailable(...)`.
    pub fn service_unavailable(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        let msg = "The server is currently unable to handle the request , please try after some time.";
        let errors = vec![ApiParameterError::general_error(
            globalisation_message_code,
            default_user_message,
            args,
        )];
        Self::create(
            503,
            "error.msg.platform.service.unavailable",
            msg,
            msg,
            errors,
        )
    }

    /// Serializes this response to a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Exception thrown when API request data validation fails.
///
/// Ported from `org.apache.fineract.infrastructure.core.exception.PlatformApiDataValidationException`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformApiDataValidationException {
    pub globalisation_message_code: String,
    pub default_user_message: String,
    pub errors: Vec<ApiParameterError>,
}

impl std::fmt::Display for PlatformApiDataValidationException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.default_user_message, self.globalisation_message_code)?;
        for err in &self.errors {
            write!(f, ": {} ({})", err.default_user_message, err.user_message_globalisation_code)?;
        }
        Ok(())
    }
}

impl std::error::Error for PlatformApiDataValidationException {}

impl PlatformApiDataValidationException {
    /// Creates a [`PlatformApiDataValidationException`] with the standard Fineract validation header.
    pub fn new(errors: Vec<ApiParameterError>) -> Self {
        Self {
            globalisation_message_code: "validation.msg.validation.errors.exist".to_string(),
            default_user_message: "Validation errors exist.".to_string(),
            errors,
        }
    }

    /// Creates a [`PlatformApiDataValidationException`] with custom top-level globalisation code and message.
    pub fn with_message(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        errors: Vec<ApiParameterError>,
    ) -> Self {
        Self {
            globalisation_message_code: globalisation_message_code.into(),
            default_user_message: default_user_message.into(),
            errors,
        }
    }

    /// Creates a [`PlatformApiDataValidationException`] containing a single parameter error.
        /// Convenience constructor for a single parameter error with no args.
    pub fn single_error(
        message_code: impl Into<String>,
        user_message: impl Into<String>,
        parameter_name: impl Into<String>,
    ) -> Self {
        Self::single_parameter_error(message_code, user_message, parameter_name, Vec::new())
    }

    pub fn single_parameter_error(
        message_code: impl Into<String>,
        user_message: impl Into<String>,
        parameter_name: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        Self::new(vec![ApiParameterError::parameter_error(
            message_code,
            user_message,
            parameter_name,
            args,
        )])
    }

    /// Returns the list of parameter validation errors.
    pub fn errors(&self) -> &[ApiParameterError] {
        &self.errors
    }

    /// Converts this validation exception into a wire-compatible [`ApiGlobalErrorResponse`] (HTTP 400).
    pub fn to_global_error_response(&self) -> ApiGlobalErrorResponse {
        ApiGlobalErrorResponse::bad_client_request(
            self.globalisation_message_code.clone(),
            self.default_user_message.clone(),
            self.errors.clone(),
        )
    }
}

/// Exception thrown when a valid API request violates a domain business rule.
///
/// Ported from `org.apache.fineract.infrastructure.core.exception.AbstractPlatformDomainRuleException`
/// and `GeneralPlatformDomainRuleException`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(rename_all = "camelCase")]
#[error("{default_user_message} ({globalisation_message_code})")]
pub struct PlatformDomainRuleException {
    pub globalisation_message_code: String,
    pub default_user_message: String,
    #[serde(default)]
    pub default_user_message_args: Vec<ApiErrorArg>,
}

/// Alias matching `GeneralPlatformDomainRuleException`.
pub type GeneralPlatformDomainRuleException = PlatformDomainRuleException;

impl PlatformDomainRuleException {
    /// Appends a single argument to the exception message arguments.
    pub fn with_arg(mut self, arg: impl std::fmt::Display) -> Self {
        self.default_user_message_args.push(ApiErrorArg {
            value: serde_json::Value::String(arg.to_string()),
        });
        self
    }

    /// Creates a new domain rule exception without message arguments.
    pub fn new(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
    ) -> Self {
        Self {
            globalisation_message_code: globalisation_message_code.into(),
            default_user_message: default_user_message.into(),
            default_user_message_args: Vec::new(),
        }
    }

    /// Creates a new domain rule exception with message arguments.
    pub fn with_args(
        globalisation_message_code: impl Into<String>,
        default_user_message: impl Into<String>,
        args: Vec<ApiErrorArg>,
    ) -> Self {
        Self {
            globalisation_message_code: globalisation_message_code.into(),
            default_user_message: default_user_message.into(),
            default_user_message_args: args,
        }
    }

    /// Converts this domain rule exception into a wire-compatible [`ApiGlobalErrorResponse`] (HTTP 403).
    pub fn to_global_error_response(&self) -> ApiGlobalErrorResponse {
        ApiGlobalErrorResponse::domain_rule_violation(
            self.globalisation_message_code.clone(),
            self.default_user_message.clone(),
            self.default_user_message_args.clone(),
        )
    }
}

/// Unified platform error enum across Fineract Rust crates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum FineractError {
    #[error(transparent)]
    DataValidation(#[from] PlatformApiDataValidationException),

    #[error(transparent)]
    DomainRule(#[from] PlatformDomainRuleException),

    #[error("{default_message} ({globalisation_code}): {resource_id}")]
    ResourceNotFound {
        globalisation_code: String,
        default_message: String,
        resource_id: String,
    },

    #[error("Unauthenticated. Please login. (error.msg.not.authenticated)")]
    Unauthenticated,

    #[error("Insufficient privileges to perform this action. (error.msg.not.authorized)")]
    AccessDenied,

    #[error("Idempotency conflict detected")]
    IdempotencyConflict,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Conflict: {0:?}")]
    Conflict(ApiGlobalErrorResponse),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Standard `Result` alias using [`FineractError`].
pub type FineractResult<T> = Result<T, FineractError>;

impl FineractError {
    /// Helper to construct a [`FineractError::DomainRule`].
    pub fn domain_rule(
        globalisation_code: impl Into<String>,
        default_message: impl Into<String>,
    ) -> Self {
        Self::DomainRule(PlatformDomainRuleException::new(
            globalisation_code,
            default_message,
        ))
    }

    /// Helper to construct a [`FineractError::ResourceNotFound`].
    pub fn resource_not_found(
        globalisation_code: impl Into<String>,
        default_message: impl Into<String>,
        resource_id: impl Into<String>,
    ) -> Self {
        Self::ResourceNotFound {
            globalisation_code: globalisation_code.into(),
            default_message: default_message.into(),
            resource_id: resource_id.into(),
        }
    }

    /// Helper to construct a not-found error with default empty resource ID.
    pub fn not_found(
        globalisation_code: impl Into<String>,
        default_message: impl Into<String>,
    ) -> Self {
        Self::ResourceNotFound {
            globalisation_code: globalisation_code.into(),
            default_message: default_message.into(),
            resource_id: String::new(),
        }
    }

    /// Returns the HTTP status code corresponding to this error.
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::DataValidation(_) => 400,
            Self::DomainRule(_) => 403,
            Self::ResourceNotFound { .. } => 404,
            Self::Unauthenticated => 401,
            Self::AccessDenied => 403,
            Self::IdempotencyConflict => 409,
            Self::DatabaseError(_) => 500,
            Self::Conflict(ref r) => r.http_status_code.parse::<u16>().unwrap_or(409),
            Self::Internal(_) => 500,
        }
    }

    /// Alias for http_status_code.
    pub fn http_status(&self) -> u16 {
        self.http_status_code()
    }

    /// Alias for to_global_error_response.
    pub fn to_api_global_error_response(&self) -> ApiGlobalErrorResponse {
        self.to_global_error_response()
    }

    /// Converts this [`FineractError`] into a wire-compatible [`ApiGlobalErrorResponse`].
    pub fn to_global_error_response(&self) -> ApiGlobalErrorResponse {
        match self {
            Self::DataValidation(ex) => ex.to_global_error_response(),
            Self::DomainRule(ex) => ex.to_global_error_response(),
            Self::ResourceNotFound {
                globalisation_code,
                default_message,
                resource_id,
            } => ApiGlobalErrorResponse::not_found(
                globalisation_code.clone(),
                default_message.clone(),
                vec![ApiErrorArg::from(resource_id.as_str())],
            ),
            Self::Unauthenticated => ApiGlobalErrorResponse::un_authenticated(),
            Self::AccessDenied => ApiGlobalErrorResponse::un_authorized(
                "Insufficient privileges to perform this action.",
            ),
            Self::IdempotencyConflict => ApiGlobalErrorResponse::conflict(Some("command"), None),
            Self::DatabaseError(msg) => ApiGlobalErrorResponse::server_side_error(
                "error.msg.platform.server.side.error",
                msg.clone(),
                Vec::new(),
            ),
            Self::Conflict(ref r) => r.clone(),
            Self::Internal(ref msg) => ApiGlobalErrorResponse::server_side_error(
                "error.msg.platform.server.side.error",
                msg.clone(),
                Vec::new(),
            ),
        }
    }
}

impl From<&FineractError> for ApiGlobalErrorResponse {
    fn from(err: &FineractError) -> Self {
        err.to_global_error_response()
    }
}

impl From<FineractError> for ApiGlobalErrorResponse {
    fn from(err: FineractError) -> Self {
        err.to_global_error_response()
    }
}

/// Value representation stored inside [`DataValidatorBuilder`].
#[derive(Debug, Clone, PartialEq)]
pub enum ValidatorValue {
    Null,
    String(String),
    Integer(i64),
    Decimal(Decimal),
    Bool(bool),
    Date(NaiveDate),
    List(Vec<ValidatorValue>),
    Json(serde_json::Value),
}

impl ValidatorValue {
    /// Returns `true` if this value represents `null` / `None`.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null | Self::Json(serde_json::Value::Null))
    }

    /// Formats the value as a Java-compatible string (`Object::toString()`).
    pub fn as_java_string(&self) -> String {
        match self {
            Self::Null | Self::Json(serde_json::Value::Null) => String::new(),
            Self::String(s) => s.clone(),
            Self::Integer(n) => n.to_string(),
            Self::Decimal(d) => d.to_string(),
            Self::Bool(b) => b.to_string(),
            Self::Date(d) => d.format("%Y-%m-%d").to_string(),
            Self::List(items) => {
                let joined: Vec<String> = items.iter().map(Self::as_java_string).collect();
                format!("[{}]", joined.join(", "))
            }
            Self::Json(v) => match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                other => other.to_string(),
            },
        }
    }

    /// Attempts to parse or extract a [`Decimal`] from this value.
    pub fn to_decimal(&self) -> Option<Decimal> {
        match self {
            Self::Decimal(d) => Some(*d),
            Self::Integer(n) => Some(Decimal::from(*n)),
            Self::String(s) => Decimal::from_str(s.trim()).ok(),
            Self::Json(serde_json::Value::Number(n)) => Decimal::from_str(&n.to_string()).ok(),
            Self::Json(serde_json::Value::String(s)) => Decimal::from_str(s.trim()).ok(),
            _ => None,
        }
    }

    /// Attempts to parse or extract an `i64` from this value.
    pub fn to_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(n) => Some(*n),
            Self::Decimal(d) => (*d).try_into().ok(),
            Self::String(s) => s.trim().parse::<i64>().ok(),
            Self::Json(serde_json::Value::Number(n)) => n.as_i64(),
            Self::Json(serde_json::Value::String(s)) => s.trim().parse::<i64>().ok(),
            _ => None,
        }
    }

    /// Attempts to extract a [`NaiveDate`] from this value.
    pub fn to_naive_date(&self) -> Option<NaiveDate> {
        match self {
            Self::Date(d) => Some(*d),
            Self::String(s) => NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok(),
            Self::Json(serde_json::Value::String(s)) => {
                NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
            }
            _ => None,
        }
    }

    /// Converts this value into an [`ApiErrorArg`].
    pub fn to_error_arg(&self) -> ApiErrorArg {
        match self {
            Self::Null | Self::Json(serde_json::Value::Null) => {
                ApiErrorArg::new(serde_json::Value::Null)
            }
            Self::String(s) => ApiErrorArg::from(s.as_str()),
            Self::Integer(n) => ApiErrorArg::from(*n),
            Self::Decimal(d) => ApiErrorArg::from_decimal(*d),
            Self::Bool(b) => ApiErrorArg::from(*b),
            Self::Date(d) => ApiErrorArg::from_date(*d),
            Self::List(items) => {
                let arr: Vec<serde_json::Value> =
                    items.iter().map(|i| i.to_error_arg().value).collect();
                ApiErrorArg::new(serde_json::Value::Array(arr))
            }
            Self::Json(v) => ApiErrorArg::new(v.clone()),
        }
    }
}

/// Conversion trait allowing ergonomic `.value(...)` calls on [`DataValidatorBuilder`].
pub trait IntoValidatorValue {
    fn into_validator_value(self) -> ValidatorValue;
}

impl IntoValidatorValue for ValidatorValue {
    fn into_validator_value(self) -> ValidatorValue {
        self
    }
}

impl IntoValidatorValue for &ValidatorValue {
    fn into_validator_value(self) -> ValidatorValue {
        self.clone()
    }
}

impl IntoValidatorValue for () {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Null
    }
}

impl IntoValidatorValue for &str {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::String(self.to_string())
    }
}

impl IntoValidatorValue for &&str {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::String((*self).to_string())
    }
}

impl IntoValidatorValue for String {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::String(self)
    }
}

impl IntoValidatorValue for &String {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::String(self.clone())
    }
}

impl IntoValidatorValue for Cow<'_, str> {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::String(self.into_owned())
    }
}

impl IntoValidatorValue for bool {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Bool(self)
    }
}

impl IntoValidatorValue for &bool {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Bool(*self)
    }
}

impl IntoValidatorValue for i8 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(i64::from(self))
    }
}

impl IntoValidatorValue for i16 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(i64::from(self))
    }
}

impl IntoValidatorValue for i32 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(i64::from(self))
    }
}

impl IntoValidatorValue for &i32 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(i64::from(*self))
    }
}

impl IntoValidatorValue for i64 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(self)
    }
}

impl IntoValidatorValue for &i64 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(*self)
    }
}

impl IntoValidatorValue for isize {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(self as i64)
    }
}

impl IntoValidatorValue for u8 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(i64::from(self))
    }
}

impl IntoValidatorValue for u16 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(i64::from(self))
    }
}

impl IntoValidatorValue for u32 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(i64::from(self))
    }
}

impl IntoValidatorValue for u64 {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(self as i64)
    }
}

impl IntoValidatorValue for usize {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Integer(self as i64)
    }
}

impl IntoValidatorValue for f32 {
    fn into_validator_value(self) -> ValidatorValue {
        Decimal::from_str(&self.to_string())
            .map(ValidatorValue::Decimal)
            .unwrap_or(ValidatorValue::Null)
    }
}

impl IntoValidatorValue for f64 {
    fn into_validator_value(self) -> ValidatorValue {
        Decimal::from_str(&self.to_string())
            .map(ValidatorValue::Decimal)
            .unwrap_or(ValidatorValue::Null)
    }
}

impl IntoValidatorValue for Decimal {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Decimal(self)
    }
}

impl IntoValidatorValue for &Decimal {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Decimal(*self)
    }
}

impl IntoValidatorValue for NaiveDate {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Date(self)
    }
}

impl IntoValidatorValue for &NaiveDate {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::Date(*self)
    }
}

impl IntoValidatorValue for serde_json::Value {
    fn into_validator_value(self) -> ValidatorValue {
        match self {
            serde_json::Value::Null => ValidatorValue::Null,
            serde_json::Value::String(s) => ValidatorValue::String(s),
            serde_json::Value::Bool(b) => ValidatorValue::Bool(b),
            serde_json::Value::Number(ref n) => {
                if let Some(i) = n.as_i64() {
                    ValidatorValue::Integer(i)
                } else if let Ok(d) = Decimal::from_str(&n.to_string()) {
                    ValidatorValue::Decimal(d)
                } else {
                    ValidatorValue::Json(self)
                }
            }
            serde_json::Value::Array(arr) => ValidatorValue::List(
                arr.into_iter()
                    .map(IntoValidatorValue::into_validator_value)
                    .collect(),
            ),
            other => ValidatorValue::Json(other),
        }
    }
}

impl IntoValidatorValue for &serde_json::Value {
    fn into_validator_value(self) -> ValidatorValue {
        self.clone().into_validator_value()
    }
}

impl<T: IntoValidatorValue> IntoValidatorValue for Option<T> {
    fn into_validator_value(self) -> ValidatorValue {
        match self {
            Some(v) => v.into_validator_value(),
            None => ValidatorValue::Null,
        }
    }
}

impl<T: Clone + IntoValidatorValue> IntoValidatorValue for &Option<T> {
    fn into_validator_value(self) -> ValidatorValue {
        match self {
            Some(v) => v.clone().into_validator_value(),
            None => ValidatorValue::Null,
        }
    }
}

impl<T: IntoValidatorValue> IntoValidatorValue for Vec<T> {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::List(
            self.into_iter()
                .map(IntoValidatorValue::into_validator_value)
                .collect(),
        )
    }
}

impl<T: Clone + IntoValidatorValue> IntoValidatorValue for &[T] {
    fn into_validator_value(self) -> ValidatorValue {
        ValidatorValue::List(
            self.iter()
                .cloned()
                .map(IntoValidatorValue::into_validator_value)
                .collect(),
        )
    }
}

#[derive(Debug, Clone)]
struct ValidatorState<'a> {
    data_validation_errors: Vec<ApiParameterError>,
    resource: Cow<'a, str>,
    parameter: Option<Cow<'a, str>>,
    array_part: Option<Cow<'a, str>>,
    array_index: Option<usize>,
    value: ValidatorValue,
    ignore_null_value: bool,
}

impl<'a> Default for ValidatorState<'a> {
    fn default() -> Self {
        Self {
            data_validation_errors: Vec::new(),
            resource: Cow::Borrowed(""),
            parameter: None,
            array_part: None,
            array_index: None,
            value: ValidatorValue::Null,
            ignore_null_value: false,
        }
    }
}

impl<'a> ValidatorState<'a> {
    fn param_name(&self) -> &str {
        self.parameter.as_deref().unwrap_or("")
    }

    fn push_error(&mut self, error: ApiParameterError) {
        self.data_validation_errors.push(error);
    }
}

/// Fluent data validation builder ported from `org.apache.fineract.infrastructure.core.data.DataValidatorBuilder`.
///
/// Supports both single-expression method chaining and Java-style `base_validator.reset().parameter(...)`
/// multi-statement chains via shared interior state.
#[derive(Debug, Clone)]
pub struct DataValidatorBuilder<'a> {
    state: Arc<Mutex<ValidatorState<'a>>>,
}

impl<'a> Default for DataValidatorBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> DataValidatorBuilder<'a> {
    /// Creates a new [`DataValidatorBuilder`] starting an empty validation chain.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ValidatorState::default())),
        }
    }

    /// Creates a new [`DataValidatorBuilder`] pre-configured with a resource name.
    pub fn for_resource(resource_name: impl Into<Cow<'a, str>>) -> Self {
        let builder = Self::new();
        builder.resource(resource_name)
    }

    /// Creates a [`DataValidatorBuilder`] initialized with an existing vector of [`ApiParameterError`]s.
    pub fn with_errors(errors: Vec<ApiParameterError>) -> Self {
        let state = ValidatorState {
            data_validation_errors: errors,
            ..ValidatorState::default()
        };
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    /// Resets per-parameter state (`parameter`, `array_part`, `array_index`, `value`, `ignore_null_value`)
    /// while preserving `resource` and accumulated `data_validation_errors`.
    pub fn reset(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            s.parameter = None;
            s.array_part = None;
            s.array_index = None;
            s.value = ValidatorValue::Null;
            s.ignore_null_value = false;
        }
        self.clone()
    }

    /// Merges all validation errors from `other` into this builder.
    pub fn merge(&self, other: &DataValidatorBuilder<'_>) -> Self {
        let other_errors = other.data_validation_errors();
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            s.data_validation_errors.extend(other_errors);
        }
        self.clone()
    }

    /// Returns `true` if at least one validation error has been recorded.
    pub fn has_error(&self) -> bool {
        let s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
        !s.data_validation_errors.is_empty()
    }

    /// Returns a clone of the accumulated [`ApiParameterError`]s.
    pub fn data_validation_errors(&self) -> Vec<ApiParameterError> {
        let s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
        s.data_validation_errors.clone()
    }

    /// Sets the resource name used in globalisation error codes (`validation.msg.{resource}...`).
    pub fn resource(&self, resource_name: impl Into<Cow<'a, str>>) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            s.resource = resource_name.into();
        }
        self.clone()
    }

    /// Sets the parameter name currently being validated.
    pub fn parameter(&self, name: impl Into<Cow<'a, str>>) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            s.parameter = Some(name.into());
        }
        self.clone()
    }

    /// Port of `parameterAtIndexArray(arrayPart, arrayIndex)`.
    pub fn parameter_at_index_array(
        &self,
        array_part: impl Into<Cow<'a, str>>,
        array_index: usize,
    ) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            s.array_part = Some(array_part.into());
            s.array_index = Some(array_index);
        }
        self.clone()
    }

    /// Sets the parameter value to validate.
    pub fn value<V: IntoValidatorValue>(&self, val: V) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            s.value = val.into_validator_value();
        }
        self.clone()
    }

    /// Sets the current value to `Null`.
    pub fn null_value(&self) -> Self {
        self.value(ValidatorValue::Null)
    }

    /// Instructs subsequent validators in the current parameter chain to ignore `null`/`None` values.
    pub fn ignore_if_null(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            s.ignore_null_value = true;
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.notNull()`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.cannot.be.blank` if the value is null and `!ignore_if_null`.
    pub fn not_null(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && !s.ignore_null_value {
                let param = s.param_name().to_string();
                let mut real_parameter_name = param.clone();
                let mut code = format!("validation.msg.{}.{}", s.resource, param);

                let mut args = Vec::new();
                if let (Some(idx), Some(part)) = (s.array_index, s.array_part.as_deref()) {
                    if !part.trim().is_empty() {
                        code.push('.');
                        code.push_str(part);
                        real_parameter_name = format!("{param}[{idx}][{part}]");
                    }
                    args.push(ApiErrorArg::from(idx));
                }

                code.push_str(".cannot.be.blank");
                let msg = format!("The parameter `{real_parameter_name}` is mandatory.");
                let error = ApiParameterError::parameter_error(code, msg, real_parameter_name, args);
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.notBlank()`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.cannot.be.blank` if the value is null or whitespace-only.
    pub fn not_blank(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            let is_blank = s.value.is_null() || s.value.as_java_string().trim().is_empty();
            if is_blank {
                let param = s.param_name().to_string();
                let mut real_parameter_name = param.clone();
                let mut code = format!("validation.msg.{}.{}", s.resource, param);

                let mut args = Vec::new();
                if let (Some(idx), Some(part)) = (s.array_index, s.array_part.as_deref()) {
                    if !part.trim().is_empty() {
                        code.push('.');
                        code.push_str(part);
                        real_parameter_name = format!("{param}[{idx}][{part}]");
                    }
                    args.push(ApiErrorArg::from(idx));
                }

                code.push_str(".cannot.be.blank");
                let msg = format!("The parameter `{real_parameter_name}` is mandatory.");
                let error = ApiParameterError::parameter_error(code, msg, real_parameter_name, args);
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.notExceedingLengthOf(maxLength)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.exceeds.max.length`.
    pub fn not_exceeding_length_of(&self, max: usize) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                let raw_str = s.value.as_java_string();
                if raw_str.trim().chars().count() > max {
                    let param = s.param_name().to_string();
                    let code = format!("validation.msg.{}.{}.exceeds.max.length", s.resource, param);
                    let msg = format!("The parameter `{param}` exceeds max length of {max}.");
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![ApiErrorArg::from(max), ApiErrorArg::from(raw_str)],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.notExceedingListLengthOf(maxLength)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.exceeds.max.length.allowed`.
    pub fn not_exceeding_list_length_of(&self, max: usize) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let ValidatorValue::List(ref items) = s.value {
                if items.len() > max {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.exceeds.max.length.allowed",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` exceeds allowed max length of {max}."
                    );
                    let error = ApiParameterError::parameter_error(code, msg, param, Vec::new());
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Validates that the numeric value is strictly greater than zero (`> 0`).
    ///
    /// Port of `positiveAmount()`, `integerGreaterThanZero()`, and `longGreaterThanZero()`.
    /// Emits `validation.msg.{resource}.{parameter}.not.greater.than.zero`.
    pub fn greater_than_zero(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                let param = s.param_name().to_string();
                if let Some(dec) = s.value.to_decimal() {
                    if dec <= Decimal::ZERO {
                        let code = format!(
                            "validation.msg.{}.{}.not.greater.than.zero",
                            s.resource, param
                        );
                        let msg = format!("The parameter `{param}` must be greater than 0.");
                        let error = ApiParameterError::parameter_error(
                            code,
                            msg,
                            param,
                            vec![s.value.to_error_arg(), ApiErrorArg::from(0)],
                        );
                        s.push_error(error);
                    }
                } else {
                    let code = format!("validation.msg.{}.{}.not.a.number", s.resource, param);
                    let msg = format!("The parameter `{param}` must be a number.");
                    let error = ApiParameterError::parameter_error(code, msg, param, Vec::new());
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Alias for [`DataValidatorBuilder::greater_than_zero`] matching `DataValidatorBuilder.positiveAmount()`.
    pub fn positive_amount(&self) -> Self {
        self.greater_than_zero()
    }

    /// Alias for [`DataValidatorBuilder::greater_than_zero`] matching `DataValidatorBuilder.integerGreaterThanZero()`.
    pub fn integer_greater_than_zero(&self) -> Self {
        self.greater_than_zero()
    }

    /// Alias for [`DataValidatorBuilder::greater_than_zero`] matching `DataValidatorBuilder.longGreaterThanZero()`.
    pub fn long_greater_than_zero(&self) -> Self {
        self.greater_than_zero()
    }

    /// Validates that the numeric value is zero or positive (`>= 0`).
    ///
    /// Port of `zeroOrPositiveAmount()` and `integerZeroOrGreater()`.
    /// Emits `validation.msg.{resource}.{parameter}.not.zero.or.greater`.
    pub fn zero_or_positive(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                if let Some(dec) = s.value.to_decimal() {
                    if dec < Decimal::ZERO {
                        let param = s.param_name().to_string();
                        let code = format!(
                            "validation.msg.{}.{}.not.zero.or.greater",
                            s.resource, param
                        );
                        let msg = format!(
                            "The parameter `{param}` must be greater than or equal to 0."
                        );
                        let error = ApiParameterError::parameter_error(
                            code,
                            msg,
                            param,
                            vec![s.value.to_error_arg(), ApiErrorArg::from(0)],
                        );
                        s.push_error(error);
                    }
                }
            }
        }
        self.clone()
    }

    /// Alias for [`DataValidatorBuilder::zero_or_positive`] matching `DataValidatorBuilder.zeroOrPositiveAmount()`.
    pub fn zero_or_positive_amount(&self) -> Self {
        self.zero_or_positive()
    }

    /// Port of `DataValidatorBuilder.integerZeroOrGreater()`.
    pub fn integer_zero_or_greater(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                if let Some(n) = s.value.to_i64() {
                    if n < 0 {
                        let param = s.param_name().to_string();
                        let code = format!(
                            "validation.msg.{}.{}.not.zero.or.greater",
                            s.resource, param
                        );
                        let msg = format!("The parameter `{param}` must be zero or greater.");
                        let error = ApiParameterError::parameter_error(
                            code,
                            msg,
                            param,
                            vec![ApiErrorArg::from(n), ApiErrorArg::from(0)],
                        );
                        s.push_error(error);
                    }
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.longZeroOrGreater()`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.not.equal.or.greater.than.zero`.
    pub fn long_zero_or_greater(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                if let Some(n) = s.value.to_i64() {
                    if n < 0 {
                        let param = s.param_name().to_string();
                        let code = format!(
                            "validation.msg.{}.{}.not.equal.or.greater.than.zero",
                            s.resource, param
                        );
                        let msg =
                            format!("The parameter `{param}` must be equal or greater than 0.");
                        let error = ApiParameterError::parameter_error(
                            code,
                            msg,
                            param,
                            vec![ApiErrorArg::from(n), ApiErrorArg::from(0)],
                        );
                        s.push_error(error);
                    }
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.percentage()`.
    ///
    /// Checks `> 0` (`not.greater.than.zero`) and `<= 100` (`greater.than.one.hundred`).
    pub fn percentage(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                if let Some(dec) = s.value.to_decimal() {
                    let param = s.param_name().to_string();
                    if dec <= Decimal::ZERO {
                        let code = format!(
                            "validation.msg.{}.{}.not.greater.than.zero",
                            s.resource, param
                        );
                        let msg = format!("The parameter `{param}` must be greater than 0.");
                        let error = ApiParameterError::parameter_error(
                            code,
                            msg,
                            param.clone(),
                            vec![ApiErrorArg::from_decimal(dec), ApiErrorArg::from(0)],
                        );
                        s.push_error(error);
                    }
                    if dec > Decimal::from(100) {
                        let code = format!(
                            "validation.msg.{}.{}.greater.than.one.hundred",
                            s.resource, param
                        );
                        let msg =
                            format!("The parameter `{param}` must be not greater than 100.");
                        let error = ApiParameterError::parameter_error(
                            code,
                            msg,
                            param,
                            vec![ApiErrorArg::from_decimal(dec), ApiErrorArg::from(0)],
                        );
                        s.push_error(error);
                    }
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.inMinMaxRange(min, max)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.is.not.within.expected.range`.
    pub fn in_min_max_range(&self, min: i64, max: i64) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                if let Some(number) = s.value.to_i64() {
                    if number < min || number > max {
                        let param = s.param_name().to_string();
                        let code = format!(
                            "validation.msg.{}.{}.is.not.within.expected.range",
                            s.resource, param
                        );
                        let msg =
                            format!("The parameter `{param}` must be between {min} and {max}.");
                        let error = ApiParameterError::parameter_error(
                            code,
                            msg,
                            param,
                            vec![
                                ApiErrorArg::from(number),
                                ApiErrorArg::from(min),
                                ApiErrorArg::from(max),
                            ],
                        );
                        s.push_error(error);
                    }
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.inMinAndMaxAmountRange(minimumAmount, maximumAmount)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.amount.is.not.within.min.max.range`.
    pub fn in_min_and_max_amount_range(&self, min: Decimal, max: Decimal) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if !s.value.is_null() {
                if let Some(amount) = s.value.to_decimal() {
                    if amount < min || amount > max {
                        let param = s.param_name().to_string();
                        let code = format!(
                            "validation.msg.{}.{}.amount.is.not.within.min.max.range",
                            s.resource, param
                        );
                        let msg = format!(
                            "The parameter `{param}` amount {amount} must be between {min} and {max} ."
                        );
                        let error = ApiParameterError::parameter_error(
                            code,
                            msg,
                            param,
                            vec![
                                ApiErrorArg::from_decimal(amount),
                                ApiErrorArg::from_decimal(min),
                                ApiErrorArg::from_decimal(max),
                            ],
                        );
                        s.push_error(error);
                    }
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.isOneOfTheseValues(values...)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.is.not.one.of.expected.enumerations`.
    pub fn is_one_of_these_values<I, T>(&self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoValidatorValue,
    {
        let allowed: Vec<ValidatorValue> = values
            .into_iter()
            .map(IntoValidatorValue::into_validator_value)
            .collect();
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            let matches = !s.value.is_null()
                && allowed.iter().any(|candidate| {
                    candidate == &s.value
                        || candidate.as_java_string() == s.value.as_java_string()
                });

            if !matches {
                let values_list_str = allowed
                    .iter()
                    .map(ValidatorValue::as_java_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                let param = s.param_name().to_string();
                let code = format!(
                    "validation.msg.{}.{}.is.not.one.of.expected.enumerations",
                    s.resource, param
                );
                let msg =
                    format!("The parameter `{param}` must be one of [ {values_list_str} ] .");
                let mut args = vec![s.value.to_error_arg()];
                args.extend(allowed.iter().map(ValidatorValue::to_error_arg));
                let error = ApiParameterError::parameter_error(code, msg, param, args);
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.isOneOfTheseStringValues(valuesList)` (case-insensitive).
    ///
    /// Emits `validation.msg.{resource}.{parameter}.is.not.one.of.expected.enumerations`.
    pub fn is_one_of_these_string_values<I, S>(&self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let allowed: Vec<String> = values.into_iter().map(|s| s.as_ref().to_string()).collect();
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            let val_lower = s.value.as_java_string().to_lowercase();
            let matches = !s.value.is_null()
                && allowed
                    .iter()
                    .any(|candidate| candidate.to_lowercase() == val_lower);

            if !matches {
                let values_list_str = allowed.join(", ");
                let param = s.param_name().to_string();
                let code = format!(
                    "validation.msg.{}.{}.is.not.one.of.expected.enumerations",
                    s.resource, param
                );
                let msg =
                    format!("The parameter `{param}` must be one of [ {values_list_str} ] .");
                let mut args = vec![s.value.to_error_arg()];
                args.extend(allowed.iter().map(|v| ApiErrorArg::from(v.as_str())));
                let error = ApiParameterError::parameter_error(code, msg, param, args);
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.isNotOneOfTheseValues(values...)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.is.one.of.unwanted.enumerations`.
    pub fn is_not_one_of_these_values<I, T>(&self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoValidatorValue,
    {
        let unwanted: Vec<ValidatorValue> = values
            .into_iter()
            .map(IntoValidatorValue::into_validator_value)
            .collect();
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                let matches = unwanted.iter().any(|candidate| {
                    candidate == &s.value
                        || candidate.as_java_string() == s.value.as_java_string()
                });
                if matches {
                    let values_list_str = unwanted
                        .iter()
                        .map(ValidatorValue::as_java_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.is.one.of.unwanted.enumerations",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` must not be any of [ {values_list_str} ] ."
                    );
                    let mut args = vec![s.value.to_error_arg()];
                    args.extend(unwanted.iter().map(ValidatorValue::to_error_arg));
                    let error = ApiParameterError::parameter_error(code, msg, param, args);
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.validateForBooleanValue()`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.value.should.true.or.false`.
    pub fn validate_for_boolean_value(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            let valid = match s.value {
                ValidatorValue::Bool(_) => true,
                ValidatorValue::Null | ValidatorValue::Json(serde_json::Value::Null) => false,
                _ => {
                    let text = s.value.as_java_string();
                    let trimmed = text.trim();
                    trimmed.eq_ignore_ascii_case("TRUE") || trimmed.eq_ignore_ascii_case("FALSE")
                }
            };

            if !valid {
                let param = s.param_name().to_string();
                let code = format!(
                    "validation.msg.{}.{}.value.should.true.or.false",
                    s.resource, param
                );
                let msg = format!("The parameter `{param}` value should true or false ");
                let error = ApiParameterError::parameter_error(
                    code,
                    msg,
                    param,
                    vec![s.value.to_error_arg()],
                );
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.trueOrFalseRequired(trueOfFalseField)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.must.be.true.or.false`.
    pub fn true_or_false_required<V: IntoValidatorValue>(&self, true_or_false_field: V) -> Self {
        let field_val = true_or_false_field.into_validator_value();
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if !field_val.is_null() {
                let text = field_val.as_java_string();
                if !text.eq_ignore_ascii_case("true") && !text.eq_ignore_ascii_case("false") {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.must.be.true.or.false",
                        s.resource, param
                    );
                    let msg = format!("The parameter `{param}` must be set as true or false.");
                    let error = ApiParameterError::parameter_error(code, msg, param, Vec::new());
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.validateDateAfter(date)`.
    ///
    /// Checks `DateUtils.isAfter(date, dateVal)` (`other > date_val`, i.e. `date_val < other`)
    /// and emits `validation.msg.{resource}.{parameter}.is.less.than.date`.
    pub fn validate_date_after(&self, other: NaiveDate) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(date_val) = s.value.to_naive_date() {
                if other > date_val {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.is.less.than.date",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` must be greater than the provided date {}",
                        other.format("%Y-%m-%d")
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![
                            ApiErrorArg::from_date(date_val),
                            ApiErrorArg::from_date(other),
                        ],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.validateDateBefore(date)`.
    ///
    /// Checks `DateUtils.isBefore(date, dateVal)` (`other < date_val`, i.e. `date_val > other`)
    /// and emits `validation.msg.{resource}.{parameter}.is.greater.than.date`.
    pub fn validate_date_before(&self, other: NaiveDate) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(date_val) = s.value.to_naive_date() {
                if other < date_val {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.is.greater.than.date",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` must be less than the provided date{}",
                        other.format("%Y-%m-%d")
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![
                            ApiErrorArg::from_date(date_val),
                            ApiErrorArg::from_date(other),
                        ],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.validateDateAfterOrEqual(date)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.is.before.than.date` when `date_val < other`.
    pub fn validate_date_after_or_equal(&self, other: NaiveDate) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(date_val) = s.value.to_naive_date() {
                if date_val < other {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.is.before.than.date",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` must be greater than or equal to the provided date: {}",
                        other.format("%Y-%m-%d")
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![
                            ApiErrorArg::from_date(date_val),
                            ApiErrorArg::from_date(other),
                        ],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.validateDateBeforeOrEqual(date)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.is.greater.than.date` when `date_val > other`.
    pub fn validate_date_before_or_equal(&self, other: NaiveDate) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(date_val) = s.value.to_naive_date() {
                if date_val > other {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.is.greater.than.date",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` must be less than or equal to the provided date: {}",
                        other.format("%Y-%m-%d")
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![
                            ApiErrorArg::from_date(date_val),
                            ApiErrorArg::from_date(other),
                        ],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.validateDateForEqual(date)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.is.not.equal.to.date` when `date_val != other`.
    pub fn validate_date_for_equal(&self, other: NaiveDate) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(date_val) = s.value.to_naive_date() {
                if date_val != other {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.is.not.equal.to.date",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` must be equal to the provided date{}",
                        other.format("%Y-%m-%d")
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![
                            ApiErrorArg::from_date(date_val),
                            ApiErrorArg::from_date(other),
                        ],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.failWithCode(errorCode)` / `failWithCodeAndMessage(errorCode, defaultEnglishMessage)`.
    ///
    /// Builds `validation.msg.{resource}.{parameter}.{code}` (unless `code` is already a full `validation.msg.*`
    /// or `error.msg.*` code) with `default_msg`.
    pub fn fail_with_code(&self, code: &str, default_msg: &str) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            let param = s.param_name().to_string();
            let validation_error_code =
                if code.starts_with("validation.msg.") || code.starts_with("error.msg.") {
                    code.to_string()
                } else if !param.is_empty() {
                    format!("validation.msg.{}.{}.{}", s.resource, param, code)
                } else if !s.resource.is_empty() {
                    format!("validation.msg.{}.{}", s.resource, code)
                } else {
                    format!("validation.msg.{code}")
                };

            let resolved_msg = if default_msg.is_empty() {
                format!("Failed data validation due to: {code}.")
            } else {
                default_msg.to_string()
            };

            let mut args = Vec::new();
            if !s.value.is_null() {
                args.push(s.value.to_error_arg());
            }
            let error = ApiParameterError::parameter_error(
                validation_error_code,
                resolved_msg,
                param,
                args,
            );
            s.push_error(error);
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.failWithCodeNoParameterAddedToErrorCode(errorCode)`.
    pub fn fail_with_code_no_parameter_added_to_error_code(&self, code: &str) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            let param = s.param_name().to_string();
            let validation_error_code = format!("validation.msg.{}.{}", s.resource, code);
            let default_msg = format!("Failed data validation due to: {code}.");
            let mut args = Vec::new();
            if !s.value.is_null() {
                args.push(s.value.to_error_arg());
            }
            let error =
                ApiParameterError::parameter_error(validation_error_code, default_msg, param, args);
            s.push_error(error);
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.andNotBlank(linkedParameterName, linkedValue)`.
    pub fn and_not_blank(&self, linked_parameter_name: &str, linked_value: Option<&str>) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && linked_value.is_none() && s.ignore_null_value {
                return self.clone();
            }

            if linked_value.map(|v| v.trim().is_empty()).unwrap_or(true) {
                let param = s.param_name().to_string();
                let code = format!(
                    "validation.msg.{}.{}.cannot.be.empty.when.{}.is.populated",
                    s.resource, linked_parameter_name, param
                );
                let msg = format!(
                    "The parameter `{linked_parameter_name}` cannot be empty when {param} is populated."
                );
                let error = ApiParameterError::parameter_error(
                    code,
                    msg,
                    linked_parameter_name,
                    vec![
                        linked_value.into_validator_value().to_error_arg(),
                        s.value.to_error_arg(),
                    ],
                );
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.equalToParameter(linkedParameterName, linkedValue)`.
    pub fn equal_to_parameter<V: IntoValidatorValue>(
        &self,
        linked_parameter_name: &str,
        linked_value: V,
    ) -> Self {
        let linked = linked_value.into_validator_value();
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && linked.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() && s.value != linked {
                let param = s.param_name().to_string();
                let code = format!(
                    "validation.msg.{}.{}.not.equal.to.{}",
                    s.resource, linked_parameter_name, param
                );
                let msg = format!(
                    "The parameter `{linked_parameter_name}` is not equal to {param}."
                );
                let error = ApiParameterError::parameter_error(
                    code,
                    msg,
                    linked_parameter_name,
                    vec![linked.to_error_arg(), s.value.to_error_arg()],
                );
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.notSameAsParameter(linkedParameterName, linkedValue)`.
    pub fn not_same_as_parameter<V: IntoValidatorValue>(
        &self,
        linked_parameter_name: &str,
        linked_value: V,
    ) -> Self {
        let linked = linked_value.into_validator_value();
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && linked.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() && s.value == linked {
                let param = s.param_name().to_string();
                let code = format!(
                    "validation.msg.{}.{}.same.as.{}",
                    s.resource, linked_parameter_name, param
                );
                let msg = format!("The parameter `{linked_parameter_name}` is same as {param}.");
                let error = ApiParameterError::parameter_error(
                    code,
                    msg,
                    linked_parameter_name,
                    vec![linked.to_error_arg(), s.value.to_error_arg()],
                );
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.integerGreaterThanNumber(number)`.
    pub fn integer_greater_than_number(&self, number: i64) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(int_val) = s.value.to_i64() {
                if int_val < number + 1 {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.not.greater.than.specified.number",
                        s.resource, param
                    );
                    let msg = format!("The parameter `{param}` must be greater than {number}");
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![ApiErrorArg::from(int_val), ApiErrorArg::from(number)],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.integerEqualToOrGreaterThanNumber(number)`.
    pub fn integer_equal_to_or_greater_than_number(&self, number: i64) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(int_val) = s.value.to_i64() {
                if int_val < number {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.not.equal.to.or.greater.than.specified.number",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` must be equal to or greater than{number}"
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![ApiErrorArg::from(int_val), ApiErrorArg::from(number)],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.integerSameAsNumber(number)`.
    pub fn integer_same_as_number(&self, number: i64) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(int_val) = s.value.to_i64() {
                if int_val != number {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.not.equal.to.specified.number",
                        s.resource, param
                    );
                    let msg = format!("The parameter `{param}` must be same as {number}");
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![ApiErrorArg::from(int_val), ApiErrorArg::from(number)],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.integerInMultiplesOfNumber(number)`.
    pub fn integer_in_multiples_of_number(&self, number: i64) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(int_val) = s.value.to_i64() {
                if number == 0 || int_val < number || int_val % number != 0 {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.not.in.multiples.of.specified.number",
                        s.resource, param
                    );
                    let msg = format!("The parameter `{param}` must be multiples of {number}");
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![ApiErrorArg::from(int_val), ApiErrorArg::from(number)],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.arrayNotEmpty()`.
    pub fn array_not_empty(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            let is_empty = match s.value {
                ValidatorValue::List(ref items) => items.is_empty(),
                ValidatorValue::Null => true,
                _ => false,
            };
            if is_empty {
                let param = s.param_name().to_string();
                let code = format!("validation.msg.{}.{}.cannot.be.empty", s.resource, param);
                let msg = format!(
                    "The parameter `{param}` cannot be empty. You must select at least one."
                );
                let error = ApiParameterError::parameter_error(code, msg, param, Vec::new());
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.listNotEmpty()`.
    pub fn list_not_empty(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            let param = s.param_name().to_string();
            match s.value {
                ValidatorValue::List(ref items) => {
                    if items.is_empty() {
                        let code =
                            format!("validation.msg.{}.{}.cannot.be.empty", s.resource, param);
                        let msg = format!(
                            "The parameter `{param}` cannot be empty. You must select at least one."
                        );
                        let error =
                            ApiParameterError::parameter_error(code, msg, param, Vec::new());
                        s.push_error(error);
                    }
                }
                _ => {
                    let code = format!("validation.msg.{}.{}.is.not.list", s.resource, param);
                    let msg = format!("The parameter `{param}` should be a list.");
                    let error = ApiParameterError::parameter_error(code, msg, param, Vec::new());
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.anyOfNotNull(objects...)`.
    pub fn any_of_not_null<I, T>(&self, objects: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoValidatorValue,
    {
        let has_data = objects
            .into_iter()
            .any(|obj| !obj.into_validator_value().is_null());
        if !has_data {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            let code = format!("validation.msg.{}.no.parameters.for.update", s.resource);
            let error = ApiParameterError::parameter_error(
                code,
                "No parameters passed for update.",
                "id",
                Vec::new(),
            );
            s.push_error(error);
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.notLessThanMin(BigDecimal min)`.
    pub fn not_less_than_min(&self, min: Decimal) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(amount) = s.value.to_decimal() {
                if amount < min {
                    let param = s.param_name().to_string();
                    let code = format!("validation.msg.{}.{}.is.less.than.min", s.resource, param);
                    let msg = format!(
                        "The parameter `{param}` value {amount} must not be less than the minimum value {min}"
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![
                            ApiErrorArg::from_decimal(amount),
                            ApiErrorArg::from_decimal(min),
                        ],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.notGreaterThanMax(BigDecimal max)`.
    pub fn not_greater_than_max(&self, max: Decimal) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(amount) = s.value.to_decimal() {
                if amount > max {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.is.greater.than.max",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` value {amount} must not be more than maximum value {max}"
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![
                            ApiErrorArg::from_decimal(amount),
                            ApiErrorArg::from_decimal(max),
                        ],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.scaleNotGreaterThan(scale)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.scale.is.greater.than.{scale}`.
    pub fn scale_not_greater_than(&self, scale: u32) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if let Some(dec) = s.value.to_decimal() {
                let normalized = dec.normalize();
                if normalized.scale() > scale {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.scale.is.greater.than.{}",
                        s.resource, param, scale
                    );
                    let msg = format!(
                        "The parameter `{param}` value {dec} decimal place must not be more than {scale} places"
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![ApiErrorArg::from_decimal(dec), ApiErrorArg::from(scale)],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.matchesRegularExpression(expression)`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.does.not.match.regexp`.
    pub fn matches_regular_expression(&self, expression: &str) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                let text = s.value.as_java_string();
                let full_pattern = if expression.starts_with('^') && expression.ends_with('$') {
                    expression.to_string()
                } else {
                    format!("^(?:{expression})$")
                };
                let matched = Regex::new(&full_pattern)
                    .map(|re| re.is_match(&text))
                    .unwrap_or(false);
                if !matched {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.does.not.match.regexp",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` must match the provided regular expression [ {expression} ] ."
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![s.value.to_error_arg(), ApiErrorArg::from(expression)],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.validatePhoneNumber()`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.format.is.invalid`.
    pub fn validate_phone_number(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            let text = s.value.as_java_string();
            let re = Regex::new(r"^\+?[0-9. ()-]{0,25}$").expect("valid phone regex");
            if s.value.is_null() || !re.is_match(&text) {
                let param = s.param_name().to_string();
                let code = format!("validation.msg.{}.{}.format.is.invalid", s.resource, param);
                let msg = format!(
                    "The parameter `{param}` is in invalid format, should contain '-','+','()' and numbers only."
                );
                let error = ApiParameterError::parameter_error(
                    code,
                    msg,
                    param,
                    vec![s.value.to_error_arg()],
                );
                s.push_error(error);
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.validDateTimeFormatPattern()`.
    ///
    /// Emits `validation.msg.{resource}.{parameter}.invalid.dateFormat.pattern`.
    pub fn valid_date_time_format_pattern(&self) -> Self {
        {
            let mut s = self.state.lock().expect("DataValidatorBuilder lock poisoned");
            if s.value.is_null() && s.ignore_null_value {
                return self.clone();
            }

            if !s.value.is_null() {
                let pattern = s.value.as_java_string();
                if !pattern.trim().is_empty() && !DateFormatValidator::is_valid_pattern(&pattern) {
                    let param = s.param_name().to_string();
                    let code = format!(
                        "validation.msg.{}.{}.invalid.dateFormat.pattern",
                        s.resource, param
                    );
                    let msg = format!(
                        "The parameter `{param}` has an invalid date/time pattern: `{pattern}`."
                    );
                    let error = ApiParameterError::parameter_error(
                        code,
                        msg,
                        param,
                        vec![ApiErrorArg::from(pattern)],
                    );
                    s.push_error(error);
                }
            }
        }
        self.clone()
    }

    /// Port of `DataValidatorBuilder.throwValidationErrors()`.
    pub fn throw_validation_errors(&self) -> Result<(), PlatformApiDataValidationException> {
        self.into_validation_result()
    }

    /// Finishes validation and returns `Ok(())` if no errors were recorded, or
    /// `Err(PlatformApiDataValidationException)` containing all recorded [`ApiParameterError`]s.
        /// Finishes validation and returns `Ok(())` or `Err(PlatformApiDataValidationException)`.
    #[inline]
    pub fn finish(&self) -> Result<(), PlatformApiDataValidationException> {
        self.into_validation_result()
    }

    pub fn into_validation_result(&self) -> Result<(), PlatformApiDataValidationException> {
        let errors = self.data_validation_errors();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(PlatformApiDataValidationException::new(errors))
        }
    }

    /// Port of `DataValidatorBuilder.buildValidationParameterApiError(...)`.
    pub fn build_validation_parameter_api_error(
        resource: &str,
        parameter_name: &str,
        error_code: &str,
        error_message: &str,
        args: Vec<ApiErrorArg>,
    ) -> ApiParameterError {
        let validation_error_code = format!("validation.msg.{resource}.{parameter_name}{error_code}");
        let default_english_message = format!("The parameter `{parameter_name}` {error_message}");
        ApiParameterError::parameter_error(
            validation_error_code,
            default_english_message,
            parameter_name,
            args,
        )
    }
}

/// Validator for Java `DateTimeFormatter` patterns, ported from
/// `org.apache.fineract.validation.constraints.DateFormatValidator`.
pub struct DateFormatValidator;

impl DateFormatValidator {
    /// Returns `true` if `pattern` is blank or is a valid Java `DateTimeFormatter` pattern.
    pub fn is_valid_pattern(pattern: &str) -> bool {
        if pattern.trim().is_empty() {
            return true;
        }
        // Valid Java DateTimeFormatter pattern letters:
        // G, u, y, Y, D, M, L, d, g, Q, q, w, W, E, e, c, F, a, h, K, k, H, m, s, S, A, n, N, V, v, z, O, X, x, Z
        const VALID_PATTERN_LETTERS: &[char] = &[
            'G', 'u', 'y', 'Y', 'D', 'M', 'L', 'd', 'g', 'Q', 'q', 'w', 'W', 'E', 'e', 'c', 'F',
            'a', 'h', 'K', 'k', 'H', 'm', 's', 'S', 'A', 'n', 'N', 'V', 'v', 'z', 'O', 'X', 'x',
            'Z',
        ];

        let mut in_quotes = false;
        let mut has_pattern_letter = false;
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];
            if ch == '\'' {
                if i + 1 < chars.len() && chars[i + 1] == '\'' {
                    i += 2;
                    continue;
                }
                in_quotes = !in_quotes;
                i += 1;
                continue;
            }
            if !in_quotes {
                if ch.is_ascii_alphabetic() {
                    if !VALID_PATTERN_LETTERS.contains(&ch) {
                        return false;
                    }
                    // Check run length of identical pattern letters
                    let mut run_len = 1;
                    while i + run_len < chars.len() && chars[i + run_len] == ch {
                        run_len += 1;
                    }
                    let max_run = match ch {
                        'd' | 'H' | 'm' | 's' | 'h' | 'k' | 'K' | 'w' => 2,
                        'W' | 'F' => 1,
                        'D' => 3,
                        'M' | 'L' | 'E' | 'e' | 'c' | 'Q' | 'q' | 'G' | 'a' | 'z' | 'O' | 'v' => 5,
                        'y' | 'u' | 'Y' | 'S' | 'n' | 'N' | 'A' => 9,
                        _ => 5,
                    };
                    if run_len > max_run {
                        return false;
                    }
                    has_pattern_letter = true;
                    i += run_len;
                    continue;
                } else if ch.is_ascii_digit() || matches!(ch, '{' | '}' | '#') {
                    // Unquoted digits or reserved characters are invalid in Java DateTimeFormatter patterns
                    return false;
                }
            }
            i += 1;
        }

        !in_quotes && has_pattern_letter
    }
}

/// Validator for locale strings, ported from
/// `org.apache.fineract.validation.constraints.LocaleValidator`.
pub struct LocaleValidator;

impl LocaleValidator {
    /// Supported ISO 639-1 language codes recognized by Fineract's JVM locale registry.
    const KNOWN_LANGUAGES: &'static [&'static str] = &[
        "af", "am", "ar", "az", "be", "bg", "bn", "bs", "ca", "cs", "cy", "da", "de", "el", "en",
        "es", "et", "eu", "fa", "fi", "fil", "fr", "ga", "gl", "gu", "he", "hi", "hr", "hu", "hy",
        "id", "is", "it", "ja", "jv", "ka", "kk", "km", "kn", "ko", "ky", "lo", "lt", "lv", "mk",
        "ml", "mn", "mr", "ms", "mt", "my", "nb", "ne", "nl", "nn", "no", "or", "pa", "pl", "ps",
        "pt", "ro", "ru", "sd", "si", "sk", "sl", "so", "sq", "sr", "sv", "sw", "ta", "te", "th",
        "tk", "tr", "uk", "ur", "uz", "vi", "xh", "yi", "yo", "zh", "zu",
    ];

    /// Supported ISO 3166-1 alpha-2 country codes.
    const KNOWN_COUNTRIES: &'static [&'static str] = &[
        "AE", "AR", "AT", "AU", "BD", "BE", "BG", "BO", "BR", "BY", "CA", "CH", "CL", "CN", "CO",
        "CR", "CZ", "DE", "DK", "DO", "DZ", "EC", "EE", "EG", "ES", "ET", "FI", "FR", "GB", "GE",
        "GH", "GR", "GT", "HK", "HN", "HR", "HU", "ID", "IE", "IL", "IN", "IQ", "IR", "IS", "IT",
        "JM", "JO", "JP", "KE", "KH", "KR", "KW", "KZ", "LB", "LK", "LT", "LU", "LV", "LY", "MA",
        "MM", "MX", "MY", "NG", "NI", "NL", "NO", "NP", "NZ", "OM", "PA", "PE", "PH", "PK", "PL",
        "PR", "PT", "PY", "QA", "RO", "RS", "RU", "RW", "SA", "SD", "SE", "SG", "SI", "SK", "SN",
        "SV", "SY", "TH", "TN", "TR", "TW", "TZ", "UA", "UG", "US", "UY", "UZ", "VE", "VN", "YE",
        "ZA", "ZM", "ZW",
    ];

    /// Returns `true` if `value` is a valid BCP-47 / Java locale string (e.g., `"en"`, `"en-US"`, `"en_US"`).
    pub fn is_valid(value: &str) -> bool {
        if value.trim().is_empty() {
            return false;
        }
        let normalized = value.replace('_', "-");
        let segments: Vec<&str> = normalized.split('-').collect();
        match segments.as_slice() {
            [lang] => {
                let l = lang.to_ascii_lowercase();
                Self::KNOWN_LANGUAGES.contains(&l.as_str())
            }
            [lang, country] => {
                let l = lang.to_ascii_lowercase();
                let c = country.to_ascii_uppercase();
                Self::KNOWN_LANGUAGES.contains(&l.as_str())
                    && Self::KNOWN_COUNTRIES.contains(&c.as_str())
            }
            _ => false,
        }
    }
}

/// Validator for enum string values, ported from
/// `org.apache.fineract.validation.constraints.EnumValueValidator`.
pub struct EnumValueValidator;

impl EnumValueValidator {
    /// Returns `true` if `value` matches one of `accepted_values` case-insensitively.
    pub fn is_valid(value: Option<&str>, accepted_values: &[&str]) -> bool {
        match value {
            Some(v) => accepted_values
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(v)),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_null_and_not_blank_globalisation_codes() {
        let res = DataValidatorBuilder::new()
            .resource("client")
            .parameter("firstname")
            .value(None::<&str>)
            .not_null()
            .reset()
            .parameter("lastname")
            .value("   ")
            .not_blank()
            .into_validation_result();

        let err = res.expect_err("expected validation errors");
        assert_eq!(
            err.globalisation_message_code,
            "validation.msg.validation.errors.exist"
        );
        assert_eq!(err.default_user_message, "Validation errors exist.");
        assert_eq!(err.errors.len(), 2);

        assert_eq!(
            err.errors[0].user_message_globalisation_code,
            "validation.msg.client.firstname.cannot.be.blank"
        );
        assert_eq!(
            err.errors[0].default_user_message,
            "The parameter `firstname` is mandatory."
        );
        assert_eq!(err.errors[0].parameter_name, "firstname");

        assert_eq!(
            err.errors[1].user_message_globalisation_code,
            "validation.msg.client.lastname.cannot.be.blank"
        );
        assert_eq!(
            err.errors[1].default_user_message,
            "The parameter `lastname` is mandatory."
        );
        assert_eq!(err.errors[1].parameter_name, "lastname");
    }

    #[test]
    fn test_parameter_at_index_array_not_blank() {
        let res = DataValidatorBuilder::new()
            .resource("loan")
            .parameter("charges")
            .parameter_at_index_array("amount", 2)
            .value(None::<Decimal>)
            .not_null()
            .into_validation_result();

        let err = res.expect_err("expected array parameter validation error");
        assert_eq!(err.errors.len(), 1);
        assert_eq!(
            err.errors[0].user_message_globalisation_code,
            "validation.msg.loan.charges.amount.cannot.be.blank"
        );
        assert_eq!(err.errors[0].parameter_name, "charges[2][amount]");
        assert_eq!(
            err.errors[0].default_user_message,
            "The parameter `charges[2][amount]` is mandatory."
        );
    }

    #[test]
    fn test_not_exceeding_length_of() {
        let res = DataValidatorBuilder::new()
            .resource("staff")
            .parameter("externalId")
            .value("EXT-123456789")
            .not_exceeding_length_of(5)
            .into_validation_result();

        let err = res.expect_err("expected max length error");
        assert_eq!(
            err.errors[0].user_message_globalisation_code,
            "validation.msg.staff.externalId.exceeds.max.length"
        );
        assert_eq!(
            err.errors[0].default_user_message,
            "The parameter `externalId` exceeds max length of 5."
        );
        assert_eq!(err.errors[0].args.len(), 2);
        assert_eq!(err.errors[0].args[0], ApiErrorArg::from(5usize));
        assert_eq!(err.errors[0].args[1], ApiErrorArg::from("EXT-123456789"));
    }

    #[test]
    fn test_greater_than_zero_and_zero_or_positive() {
        let validator = DataValidatorBuilder::new().resource("loanproduct");
        validator
            .reset()
            .parameter("principal")
            .value(Decimal::ZERO)
            .greater_than_zero();
        validator
            .reset()
            .parameter("interestRatePerPeriod")
            .value(Decimal::new(-150, 2))
            .zero_or_positive();

        let err = validator
            .into_validation_result()
            .expect_err("expected numeric range errors");
        assert_eq!(err.errors.len(), 2);
        assert_eq!(
            err.errors[0].user_message_globalisation_code,
            "validation.msg.loanproduct.principal.not.greater.than.zero"
        );
        assert_eq!(
            err.errors[0].default_user_message,
            "The parameter `principal` must be greater than 0."
        );
        assert_eq!(
            err.errors[1].user_message_globalisation_code,
            "validation.msg.loanproduct.interestRatePerPeriod.not.zero.or.greater"
        );
        assert_eq!(
            err.errors[1].default_user_message,
            "The parameter `interestRatePerPeriod` must be greater than or equal to 0."
        );
    }

    #[test]
    fn test_in_min_max_range_and_enumerations() {
        let res = DataValidatorBuilder::new()
            .resource("savingsproduct")
            .parameter("interestPostingPeriodType")
            .value(99)
            .in_min_max_range(1, 12)
            .reset()
            .parameter("amortizationType")
            .value(5)
            .is_one_of_these_values([1, 2])
            .into_validation_result();

        let err = res.expect_err("expected range and enumeration errors");
        assert_eq!(err.errors.len(), 2);
        assert_eq!(
            err.errors[0].user_message_globalisation_code,
            "validation.msg.savingsproduct.interestPostingPeriodType.is.not.within.expected.range"
        );
        assert_eq!(
            err.errors[0].default_user_message,
            "The parameter `interestPostingPeriodType` must be between 1 and 12."
        );

        assert_eq!(
            err.errors[1].user_message_globalisation_code,
            "validation.msg.savingsproduct.amortizationType.is.not.one.of.expected.enumerations"
        );
        assert_eq!(
            err.errors[1].default_user_message,
            "The parameter `amortizationType` must be one of [ 1, 2 ] ."
        );
    }

    #[test]
    fn test_validate_for_boolean_value() {
        let ok = DataValidatorBuilder::new()
            .resource("client")
            .parameter("active")
            .value("true")
            .validate_for_boolean_value()
            .into_validation_result();
        assert!(ok.is_ok());

        let err = DataValidatorBuilder::new()
            .resource("client")
            .parameter("active")
            .value("yes")
            .validate_for_boolean_value()
            .into_validation_result()
            .expect_err("expected boolean error");
        assert_eq!(
            err.errors[0].user_message_globalisation_code,
            "validation.msg.client.active.value.should.true.or.false"
        );
        assert_eq!(
            err.errors[0].default_user_message,
            "The parameter `active` value should true or false "
        );
    }

    #[test]
    fn test_validate_date_after_and_before() {
        let disbursed = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        let early_due = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let late_dob = NaiveDate::from_ymd_opt(2026, 3, 20).unwrap();

        let err = DataValidatorBuilder::new()
            .resource("loan")
            .parameter("expectedFirstRepaymentOnDate")
            .value(early_due)
            .validate_date_after(disbursed)
            .reset()
            .parameter("dateOfBirth")
            .value(late_dob)
            .validate_date_before(disbursed)
            .into_validation_result()
            .expect_err("expected date comparison errors");

        assert_eq!(err.errors.len(), 2);
        assert_eq!(
            err.errors[0].user_message_globalisation_code,
            "validation.msg.loan.expectedFirstRepaymentOnDate.is.less.than.date"
        );
        assert_eq!(
            err.errors[0].default_user_message,
            "The parameter `expectedFirstRepaymentOnDate` must be greater than the provided date 2026-03-15"
        );
        assert_eq!(
            err.errors[0].args,
            vec![
                ApiErrorArg::from("2026-03-10"),
                ApiErrorArg::from("2026-03-15")
            ]
        );

        assert_eq!(
            err.errors[1].user_message_globalisation_code,
            "validation.msg.loan.dateOfBirth.is.greater.than.date"
        );
        assert_eq!(
            err.errors[1].default_user_message,
            "The parameter `dateOfBirth` must be less than the provided date2026-03-15"
        );
    }

    #[test]
    fn test_fail_with_code_and_ignore_if_null() {
        let err = DataValidatorBuilder::new()
            .resource("loan")
            .parameter("externalId")
            .value(None::<&str>)
            .ignore_if_null()
            .not_blank()
            .reset()
            .parameter("disbursementDate")
            .fail_with_code(
                "cannot.be.before.submitted.date",
                "Disbursement date cannot be before submitted date.",
            )
            .into_validation_result()
            .expect_err("expected fail_with_code error");

        assert_eq!(err.errors.len(), 1);
        assert_eq!(
            err.errors[0].user_message_globalisation_code,
            "validation.msg.loan.disbursementDate.cannot.be.before.submitted.date"
        );
        assert_eq!(
            err.errors[0].default_user_message,
            "Disbursement date cannot be before submitted date."
        );
    }

    #[test]
    fn test_camel_case_wire_serialization_and_fineract_error_mapping() {
        let validation_err = DataValidatorBuilder::new()
            .resource("office")
            .parameter("name")
            .value(None::<&str>)
            .not_blank()
            .into_validation_result()
            .unwrap_err();

        let fin_err = FineractError::from(validation_err);
        assert_eq!(fin_err.http_status_code(), 400);

        let global_resp = fin_err.to_global_error_response();
        let json = serde_json::to_value(&global_resp).expect("serialize ApiGlobalErrorResponse");

        assert_eq!(json["httpStatusCode"], "400");
        assert_eq!(
            json["userMessageGlobalisationCode"],
            "validation.msg.validation.errors.exist"
        );
        assert_eq!(json["defaultUserMessage"], "Validation errors exist.");
        assert_eq!(
            json["errors"][0]["userMessageGlobalisationCode"],
            "validation.msg.office.name.cannot.be.blank"
        );
        assert_eq!(json["errors"][0]["parameterName"], "name");

        let domain_err = FineractError::domain_rule(
            "error.msg.loan.disbursal.amount.exceeds.approved.amount",
            "Loan disbursal amount cannot exceed approved amount.",
        );
        assert_eq!(domain_err.http_status_code(), 403);
        let domain_resp = domain_err.to_global_error_response();
        assert_eq!(domain_resp.http_status_code, "403");
        assert_eq!(
            domain_resp.user_message_globalisation_code,
            "validation.msg.domain.rule.violation"
        );
        assert_eq!(
            domain_resp.errors[0].user_message_globalisation_code,
            "error.msg.loan.disbursal.amount.exceeds.approved.amount"
        );
    }

    #[test]
    fn test_date_format_and_locale_validators() {
        assert!(DateFormatValidator::is_valid_pattern(""));
        assert!(DateFormatValidator::is_valid_pattern("dd MMMM yyyy"));
        assert!(DateFormatValidator::is_valid_pattern("yyyy-MM-dd"));
        assert!(DateFormatValidator::is_valid_pattern("dd/MM/yyyy"));
        assert!(DateFormatValidator::is_valid_pattern("MMM dd, yyyy"));
        assert!(!DateFormatValidator::is_valid_pattern("02 February 2026"));
        assert!(!DateFormatValidator::is_valid_pattern("invalid"));
        assert!(!DateFormatValidator::is_valid_pattern("dd bbb yyyy"));

        assert!(LocaleValidator::is_valid("en"));
        assert!(LocaleValidator::is_valid("EN"));
        assert!(LocaleValidator::is_valid("en-US"));
        assert!(LocaleValidator::is_valid("en_US"));
        assert!(!LocaleValidator::is_valid(""));
        assert!(!LocaleValidator::is_valid("invalid-locale"));
        assert!(!LocaleValidator::is_valid("xx-YY"));
        assert!(!LocaleValidator::is_valid("123"));
        assert!(!LocaleValidator::is_valid("en-US-extra"));
    }
}
