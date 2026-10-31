//! `fineract-loan-origination-rs` — Loan Origination, Credit Scoring, Co-Applicants, and Underwriting Workflow.
//!
//! Ported from:
//! - `org.apache.fineract.portfolio.loanorigination.domain.{LoanOriginator, LoanOriginatorStatus, LoanOriginatorMapping, WorkingCapitalLoanOriginatorMapping}`
//! - `org.apache.fineract.portfolio.loanorigination.api.LoanOriginatorApiConstants`
//! - `org.apache.fineract.portfolio.loanorigination.data.{LoanOriginatorData, LoanOriginatorRequestData, LoanOriginatorTemplateData, LoanApplicationOriginatorData, LoanOriginatorMappingResponse, LoanOriginatorsResponse}`
//! - `org.apache.fineract.portfolio.loanorigination.serialization.{LoanOriginatorDataValidator, LoanApplicationOriginatorDataValidator}`
//! - `org.apache.fineract.portfolio.loanorigination.service.{LoanOriginatorWritePlatformServiceImpl, LoanOriginatorReadPlatformServiceImpl, LoanOriginatorLinkingServiceImpl, LoanOriginatorHelper, WorkingCapitalLoanOriginatorWritePlatformServiceImpl}`
//! - `org.apache.fineract.portfolio.loanorigination.handler.{CreateLoanOriginatorCommandHandler, UpdateLoanOriginatorCommandHandler, DeleteLoanOriginatorCommandHandler, AttachLoanOriginatorCommandHandler, DetachLoanOriginatorCommandHandler}`
//! - `org.apache.fineract.portfolio.loanorigination.exception.*`
//! - `org.apache.fineract.portfolio.coapplicant.*` & Underwriting Scorecard Engine (`DTI`, `LTV`, Bureau Score)

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use rust_decimal::{Decimal, RoundingStrategy};
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AnyPool, Row};

use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{DialectSql, ExecutionContext, ExternalId, JsonCommand, SqlDialect};
use fineract_validation_rs::{
    ApiParameterError, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException, PlatformDomainRuleException,
};

// =============================================================================
// 1. API CONSTANTS & SUPPORTED PARAMETERS (`LoanOriginatorApiConstants`)
// =============================================================================

pub const LOAN_ORIGINATOR_RESOURCE_NAME: &str = "LOAN_ORIGINATOR";
pub const LOAN_ORIGINATOR_RESOURCE_PATH: &str = "/loan-originators";
pub const CO_APPLICANT_RESOURCE_NAME: &str = "COAPPLICANT";

pub const ORIGINATOR_TYPE_CODE_NAME: &str = "LoanOriginatorType";
pub const CHANNEL_TYPE_CODE_NAME: &str = "LoanOriginationChannelType";
pub const ENABLE_ORIGINATOR_CREATION_DURING_LOAN_APPLICATION: &str =
    "enable-originator-creation-during-loan-application";

pub const EXTERNAL_ID_PARAM: &str = "externalId";
pub const NAME_PARAM: &str = "name";
pub const STATUS_PARAM: &str = "status";
pub const ORIGINATOR_TYPE_ID_PARAM: &str = "originatorTypeId";
pub const ORIGINATOR_TYPE_PARAM: &str = "originatorType";
pub const CHANNEL_TYPE_ID_PARAM: &str = "channelTypeId";
pub const CHANNEL_TYPE_PARAM: &str = "channelType";
pub const COMMISSION_RATE_PARAM: &str = "commissionRate";
pub const REVENUE_SHARE_PERCENTAGE_PARAM: &str = "revenueSharePercentage";

/// Supported request parameters for `CREATE LOAN_ORIGINATOR`.
pub const CREATE_LOAN_ORIGINATOR_PARAMS: &[&str] = &[
    EXTERNAL_ID_PARAM,
    NAME_PARAM,
    STATUS_PARAM,
    ORIGINATOR_TYPE_ID_PARAM,
    ORIGINATOR_TYPE_PARAM,
    CHANNEL_TYPE_ID_PARAM,
    CHANNEL_TYPE_PARAM,
    COMMISSION_RATE_PARAM,
    "locale",
    "dateFormat",
];

/// Supported request parameters for `UPDATE LOAN_ORIGINATOR`.
pub const UPDATE_LOAN_ORIGINATOR_PARAMS: &[&str] = &[
    NAME_PARAM,
    STATUS_PARAM,
    ORIGINATOR_TYPE_ID_PARAM,
    ORIGINATOR_TYPE_PARAM,
    CHANNEL_TYPE_ID_PARAM,
    CHANNEL_TYPE_PARAM,
    COMMISSION_RATE_PARAM,
    "locale",
    "dateFormat",
];

/// Supported request parameters for `ATTACH LOAN_ORIGINATOR`.
pub const ATTACH_LOAN_ORIGINATOR_PARAMS: &[&str] = &[
    "id",
    "loanId",
    "originatorId",
    EXTERNAL_ID_PARAM,
    NAME_PARAM,
    ORIGINATOR_TYPE_ID_PARAM,
    ORIGINATOR_TYPE_PARAM,
    CHANNEL_TYPE_ID_PARAM,
    CHANNEL_TYPE_PARAM,
    REVENUE_SHARE_PERCENTAGE_PARAM,
    "locale",
    "dateFormat",
];

/// Supported request parameters for `ADD COAPPLICANT`.
pub const CO_APPLICANT_SUPPORTED_PARAMS: &[&str] = &[
    "id",
    "loanId",
    "clientId",
    "relationshipCvId",
    "relationshipTypeId",
    "incomeSharePercentage",
    "liabilitySharePercentage",
    "locale",
    "dateFormat",
];

/// System-seeded `LoanOriginatorType` code values from `0001_initial_schema.xml`.
pub const STANDARD_ORIGINATOR_TYPES: &[(i64, &str)] = &[
    (1, "MERCHANT"),
    (2, "BROKER"),
    (3, "AFFILIATE"),
    (4, "PLATFORM"),
];

/// System-seeded `LoanOriginationChannelType` code values from `0001_initial_schema.xml`.
pub const STANDARD_CHANNEL_TYPES: &[(i64, &str)] = &[
    (1, "ONLINE"),
    (2, "IN_STORE"),
    (3, "API"),
    (4, "AGGREGATOR"),
];

/// Resolves a standard `LoanOriginatorType` name from its 1-based code value ID.
pub fn resolve_originator_type_by_id(type_id: i64) -> FineractResult<String> {
    STANDARD_ORIGINATOR_TYPES
        .iter()
        .find_map(|&(id, name)| if id == type_id { Some(name.to_string()) } else { None })
        .ok_or_else(|| {
            FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.loan.originator.originatorTypeId.invalid.code.value",
                    format!("Invalid code value id {type_id} for {ORIGINATOR_TYPE_CODE_NAME}"),
                    ORIGINATOR_TYPE_ID_PARAM,
                    vec![type_id.into()],
                ),
            )
        })
}

/// Resolves a standard `LoanOriginationChannelType` name from its 1-based code value ID.
pub fn resolve_channel_type_by_id(channel_type_id: i64) -> FineractResult<String> {
    STANDARD_CHANNEL_TYPES
        .iter()
        .find_map(|&(id, name)| {
            if id == channel_type_id {
                Some(name.to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.loan.originator.channelTypeId.invalid.code.value",
                    format!("Invalid code value id {channel_type_id} for {CHANNEL_TYPE_CODE_NAME}"),
                    CHANNEL_TYPE_ID_PARAM,
                    vec![channel_type_id.into()],
                ),
            )
        })
}

/// Resolves the numeric `originator_type_cv_id` for a given `LoanOriginatorType` code name.
pub fn originator_type_id_from_name(name: &str) -> Option<i64> {
    let trimmed = name.trim();
    if let Ok(parsed) = trimmed.parse::<i64>() {
        return Some(parsed);
    }
    STANDARD_ORIGINATOR_TYPES
        .iter()
        .find_map(|&(id, code)| trimmed.eq_ignore_ascii_case(code).then_some(id))
}

/// Resolves the numeric `channel_type_cv_id` for a given `LoanOriginationChannelType` code name.
pub fn channel_type_id_from_name(name: &str) -> Option<i64> {
    let trimmed = name.trim();
    if let Ok(parsed) = trimmed.parse::<i64>() {
        return Some(parsed);
    }
    STANDARD_CHANNEL_TYPES
        .iter()
        .find_map(|&(id, code)| trimmed.eq_ignore_ascii_case(code).then_some(id))
}

// =============================================================================
// 2. DOMAIN EXCEPTIONS (`org.apache.fineract.portfolio.loanorigination.exception.*`)
// =============================================================================

/// Raised when a loan originator cannot be found by ID or external ID.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanOriginatorNotFoundException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanOriginatorNotFoundException {
    pub identifier: String,
    pub by_external_id: bool,
}

impl LoanOriginatorNotFoundException {
    pub const ID_NOT_FOUND_CODE: &'static str = "error.msg.loan.originator.id.not.found";
    pub const EXTERNAL_ID_NOT_FOUND_CODE: &'static str =
        "error.msg.loan.originator.external.id.not.found";

    pub fn by_id(id: i64) -> Self {
        Self {
            identifier: id.to_string(),
            by_external_id: false,
        }
    }

    pub fn by_external_id(external_id: impl Into<String>) -> Self {
        Self {
            identifier: external_id.into(),
            by_external_id: true,
        }
    }

    pub fn into_fineract_error(self) -> FineractError {
        if self.by_external_id {
            FineractError::resource_not_found(
                Self::EXTERNAL_ID_NOT_FOUND_CODE,
                format!(
                    "Loan Originator with external id {} not found",
                    self.identifier
                ),
                self.identifier,
            )
        } else {
            FineractError::resource_not_found(
                Self::ID_NOT_FOUND_CODE,
                format!("Loan Originator with id {} not found", self.identifier),
                self.identifier,
            )
        }
    }
}

/// Raised when a loan originator with the same `externalId` already exists.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanOriginatorDuplicateExternalIdException`.
pub fn duplicate_originator_external_id_error(external_id: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.loan.originator.duplicate.external.id",
        format!("Loan Originator with external id '{external_id}' already exists"),
        vec![external_id.into()],
    ))
}

/// Raised when an invalid `LoanOriginatorStatus` string is provided.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanOriginatorInvalidStatusException`.
pub fn invalid_originator_status_error(status: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.loan.originator.invalid.status",
        format!(
            "Invalid loan originator status: {status}. Valid values are: ACTIVE, PENDING, INACTIVE"
        ),
        vec![status.into()],
    ))
}

/// Raised when attempting to attach a non-`ACTIVE` originator to a loan.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanOriginatorNotActiveException`.
pub fn originator_not_active_error(originator_id: i64, status: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.loan.originator.not.active",
        format!(
            "Originator with id {originator_id} has status {status}. Only ACTIVE originators can be attached to loans."
        ),
        vec![originator_id.into(), status.into()],
    ))
}

/// Raised when an originator is already mapped to the target loan.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanOriginatorMappingAlreadyExistsException`.
pub fn originator_mapping_already_exists_error(loan_id: i64, originator_id: i64) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.loan.originator.mapping.already.exists",
        format!(
            "Originator with id {originator_id} is already attached to loan with id {loan_id}"
        ),
        vec![loan_id.into(), originator_id.into()],
    ))
}

/// Raised when an originator mapping cannot be found on the target loan.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanOriginatorMappingNotFoundException`.
pub fn originator_mapping_not_found_error(loan_id: i64, originator_id: i64) -> FineractError {
    FineractError::resource_not_found(
        "error.msg.loan.originator.mapping.not.found",
        format!("Originator with id {originator_id} is not attached to loan with id {loan_id}"),
        format!("{loan_id}:{originator_id}"),
    )
}

/// Raised when attempting to delete a `LoanOriginator` that is still mapped to one or more loans.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanOriginatorCannotBeDeletedException`.
pub fn originator_cannot_be_deleted_error(id: i64) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.loan.originator.cannot.be.deleted.mapped.to.loan",
        format!(
            "Loan Originator with id {id} cannot be deleted as it is mapped to one or more loans"
        ),
        vec![id.into()],
    ))
}

/// Raised when on-the-fly originator creation during loan application is disabled via global configuration.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanOriginatorCreationNotAllowedException`.
pub fn originator_creation_not_allowed_error(external_id: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.loan.originator.creation.not.allowed",
        format!(
            "Cannot create originator with externalId '{external_id}' during loan application. Global configuration 'enable-originator-creation-during-loan-application' is disabled."
        ),
        vec![external_id.into()],
    ))
}

/// Raised when attaching or detaching an originator on a loan that is not in `Submitted and Pending Approval` status.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.exception.LoanNotInSubmittedStatusException`.
pub fn loan_not_in_submitted_status_error(loan_id: i64, current_status: &str) -> FineractError {
    FineractError::DomainRule(PlatformDomainRuleException::with_args(
        "error.msg.loan.not.in.submitted.status",
        format!(
            "Loan with id {loan_id} has status {current_status}. Originator can only be attached/detached while loan is in 'Submitted and Pending Approval' status."
        ),
        vec![loan_id.into(), current_status.into()],
    ))
}

// =============================================================================
// 3. LOAN ORIGINATOR STATUS & DOMAIN MODELS (`m_loan_originator`, `m_loan_originator_mapping`)
// =============================================================================

/// Lifecycle status of a [`LoanOriginator`] (`m_loan_originator.status`).
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.domain.LoanOriginatorStatus`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub enum LoanOriginatorStatus {
    #[default]
    #[serde(rename = "ACTIVE", alias = "Active", alias = "active")]
    Active,
    #[serde(rename = "INACTIVE", alias = "Inactive", alias = "inactive")]
    Inactive,
    #[serde(rename = "PENDING", alias = "Pending", alias = "pending")]
    Pending,
}

impl LoanOriginatorStatus {
    pub const ACTIVE: Self = Self::Active;
    pub const INACTIVE: Self = Self::Inactive;
    pub const PENDING: Self = Self::Pending;

    pub const ALL_VALUES: &'static [&'static str] = &["ACTIVE", "PENDING", "INACTIVE"];

    /// Returns the canonical uppercase string value (`"ACTIVE"`, `"INACTIVE"`, `"PENDING"`).
    pub const fn value(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Inactive => "INACTIVE",
            Self::Pending => "PENDING",
        }
    }

    /// Alias for [`Self::value`].
    pub const fn as_str(self) -> &'static str {
        self.value()
    }

    /// Returns the Fineract enumeration code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Active => "loanOriginatorStatus.active",
            Self::Inactive => "loanOriginatorStatus.inactive",
            Self::Pending => "loanOriginatorStatus.pending",
        }
    }

    /// Returns all valid string representations (`["ACTIVE", "PENDING", "INACTIVE"]`).
    pub const fn get_all_values() -> &'static [&'static str] {
        Self::ALL_VALUES
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub const fn is_inactive(self) -> bool {
        matches!(self, Self::Inactive)
    }

    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Case-insensitive parser matching `LoanOriginatorStatus.fromString(String text)`.
    pub fn from_string(text: &str) -> FineractResult<Self> {
        let trimmed = text.trim();
        if trimmed.eq_ignore_ascii_case("ACTIVE") {
            Ok(Self::Active)
        } else if trimmed.eq_ignore_ascii_case("INACTIVE") {
            Ok(Self::Inactive)
        } else if trimmed.eq_ignore_ascii_case("PENDING") {
            Ok(Self::Pending)
        } else {
            Err(invalid_originator_status_error(text))
        }
    }
}

impl fmt::Display for LoanOriginatorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.value())
    }
}

impl FromStr for LoanOriginatorStatus {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_string(s)
    }
}

/// Domain entity representing a third-party or internal Loan Originator (`m_loan_originator`).
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.domain.LoanOriginator`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoanOriginator {
    pub id: i64,
    pub external_id: ExternalId,
    pub name: String,
    pub status: LoanOriginatorStatus,
    pub originator_type: Option<String>,
    pub channel_type: Option<String>,
    pub commission_rate: Option<Decimal>,
}

impl LoanOriginator {
    /// Creates a new [`LoanOriginator`] aggregate.
    pub fn new(
        id: i64,
        external_id: impl Into<ExternalId>,
        name: impl Into<String>,
        status: LoanOriginatorStatus,
        originator_type: Option<String>,
        channel_type: Option<String>,
        commission_rate: Option<Decimal>,
    ) -> Self {
        Self {
            id,
            external_id: external_id.into(),
            name: name.into(),
            status,
            originator_type,
            channel_type,
            commission_rate,
        }
    }

    /// Creates a new [`LoanOriginator`] matching Java `LoanOriginator.create(...)`.
    pub fn create(
        external_id: impl Into<ExternalId>,
        name: impl Into<String>,
        status: LoanOriginatorStatus,
        originator_type: Option<String>,
        channel_type: Option<String>,
    ) -> Self {
        Self::new(0, external_id, name, status, originator_type, channel_type, None)
    }

    /// Builder-style setter for `commission_rate`.
    pub fn with_commission_rate(mut self, commission_rate: Decimal) -> Self {
        self.commission_rate = Some(commission_rate);
        self
    }

    /// Resolves the numeric `originator_type_cv_id` if `originator_type` is set.
    pub fn originator_type_cv_id(&self) -> Option<i64> {
        self.originator_type
            .as_deref()
            .and_then(originator_type_id_from_name)
    }

    /// Resolves the numeric `channel_type_cv_id` if `channel_type` is set.
    pub fn channel_type_cv_id(&self) -> Option<i64> {
        self.channel_type
            .as_deref()
            .and_then(channel_type_id_from_name)
    }

    /// Validates a `CREATE LOAN_ORIGINATOR` command payload, ported from
    /// `LoanOriginatorDataValidator.validateForCreate`.
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        if command.raw_json.trim().is_empty() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.invalid.json",
                    "The request JSON body cannot be blank.",
                    "json",
                    Vec::new(),
                ),
            ));
        }

        command.check_for_unsupported_parameters(CREATE_LOAN_ORIGINATOR_PARAMS)?;
        let validator = DataValidatorBuilder::for_resource(LOAN_ORIGINATOR_RESOURCE_NAME);

        let external_id = command.string_value_of_parameter_named(EXTERNAL_ID_PARAM);
        validator
            .reset()
            .parameter(EXTERNAL_ID_PARAM)
            .value(external_id)
            .not_blank()
            .not_exceeding_length_of(100);

        let name = command.string_value_of_parameter_named(NAME_PARAM);
        validator
            .reset()
            .parameter(NAME_PARAM)
            .value(name)
            .ignore_if_null()
            .not_exceeding_length_of(255);

        if command.parameter_exists(STATUS_PARAM) {
            let status = command.string_value_of_parameter_named(STATUS_PARAM);
            validator
                .reset()
                .parameter(STATUS_PARAM)
                .value(status.clone())
                .not_blank();
            if let Some(ref s) = status {
                if !s.trim().is_empty() {
                    LoanOriginatorStatus::from_string(s)?;
                }
            }
        }

        if command.parameter_exists(COMMISSION_RATE_PARAM) {
            let rate = command.big_decimal_value_of_parameter_named(COMMISSION_RATE_PARAM)?;
            validator
                .reset()
                .parameter(COMMISSION_RATE_PARAM)
                .value(rate)
                .ignore_if_null()
                .zero_or_positive_amount()
                .not_greater_than_max(Decimal::ONE_HUNDRED);
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates an `UPDATE LOAN_ORIGINATOR` command payload, ported from
    /// `LoanOriginatorDataValidator.validateForUpdate`.
    pub fn validate_for_update(command: &JsonCommand) -> FineractResult<()> {
        if command.raw_json.trim().is_empty() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.invalid.json",
                    "The request JSON body cannot be blank.",
                    "json",
                    Vec::new(),
                ),
            ));
        }

        command.check_for_unsupported_parameters(UPDATE_LOAN_ORIGINATOR_PARAMS)?;
        let validator = DataValidatorBuilder::for_resource(LOAN_ORIGINATOR_RESOURCE_NAME);

        let name = command.string_value_of_parameter_named(NAME_PARAM);
        validator
            .reset()
            .parameter(NAME_PARAM)
            .value(name)
            .ignore_if_null()
            .not_exceeding_length_of(255);

        if command.parameter_exists(STATUS_PARAM) {
            let status = command.string_value_of_parameter_named(STATUS_PARAM);
            validator
                .reset()
                .parameter(STATUS_PARAM)
                .value(status.clone())
                .not_blank();
            if let Some(ref s) = status {
                if !s.trim().is_empty() {
                    LoanOriginatorStatus::from_string(s)?;
                }
            }
        }

        if command.parameter_exists(COMMISSION_RATE_PARAM) {
            let rate = command.big_decimal_value_of_parameter_named(COMMISSION_RATE_PARAM)?;
            validator
                .reset()
                .parameter(COMMISSION_RATE_PARAM)
                .value(rate)
                .ignore_if_null()
                .zero_or_positive_amount()
                .not_greater_than_max(Decimal::ONE_HUNDRED);
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a [`LoanOriginator`] from a validated [`JsonCommand`].
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;

        let external_id_str = command
            .string_value_of_parameter_named(EXTERNAL_ID_PARAM)
            .unwrap_or_default();
        let name = command
            .string_value_of_parameter_named(NAME_PARAM)
            .unwrap_or_default();
        let status = match command.string_value_of_parameter_named(STATUS_PARAM) {
            Some(ref s) if !s.trim().is_empty() => LoanOriginatorStatus::from_string(s)?,
            _ => LoanOriginatorStatus::Active,
        };

        let originator_type = if let Some(type_id) =
            command.long_value_of_parameter_named(ORIGINATOR_TYPE_ID_PARAM)?
        {
            Some(resolve_originator_type_by_id(type_id)?)
        } else {
            command
                .string_value_of_parameter_named(ORIGINATOR_TYPE_PARAM)
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty())
        };

        let channel_type =
            if let Some(chan_id) = command.long_value_of_parameter_named(CHANNEL_TYPE_ID_PARAM)? {
                Some(resolve_channel_type_by_id(chan_id)?)
            } else {
                command
                    .string_value_of_parameter_named(CHANNEL_TYPE_PARAM)
                    .map(|s| s.trim().to_uppercase())
                    .filter(|s| !s.is_empty())
            };

        let commission_rate =
            command.big_decimal_value_of_parameter_named(COMMISSION_RATE_PARAM)?;

        Ok(Self {
            id,
            external_id: ExternalId::from_opt(Some(external_id_str)),
            name,
            status,
            originator_type,
            channel_type,
            commission_rate,
        })
    }

    /// Applies an update [`JsonCommand`] to this [`LoanOriginator`] and returns the changed fields.
    ///
    /// Ported from `LoanOriginatorWritePlatformServiceImpl.update`.
    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        Self::validate_for_update(command)?;
        let mut changes = BTreeMap::new();

        if command.is_change_in_string_parameter_named(NAME_PARAM, Some(&self.name)) {
            let new_name = command
                .string_value_of_parameter_named(NAME_PARAM)
                .unwrap_or_default();
            self.name = new_name.clone();
            changes.insert(NAME_PARAM.to_string(), Value::String(new_name));
        }

        if command.is_change_in_string_parameter_named(STATUS_PARAM, Some(self.status.value())) {
            if let Some(new_status_str) = command.string_value_of_parameter_named(STATUS_PARAM) {
                let new_status = LoanOriginatorStatus::from_string(&new_status_str)?;
                self.status = new_status;
                changes.insert(STATUS_PARAM.to_string(), Value::String(new_status_str));
            }
        }

        if command.parameter_exists(ORIGINATOR_TYPE_ID_PARAM) {
            let current_id = self.originator_type_cv_id();
            if command.is_change_in_long_parameter_named(ORIGINATOR_TYPE_ID_PARAM, current_id)? {
                let new_type_id = command.long_value_of_parameter_named(ORIGINATOR_TYPE_ID_PARAM)?;
                self.originator_type = match new_type_id {
                    Some(tid) => Some(resolve_originator_type_by_id(tid)?),
                    None => None,
                };
                changes.insert(
                    ORIGINATOR_TYPE_ID_PARAM.to_string(),
                    new_type_id.map(Value::from).unwrap_or(Value::Null),
                );
            }
        } else if command.parameter_exists(ORIGINATOR_TYPE_PARAM) {
            if command.is_change_in_string_parameter_named(
                ORIGINATOR_TYPE_PARAM,
                self.originator_type.as_deref(),
            ) {
                let new_type = command
                    .string_value_of_parameter_named(ORIGINATOR_TYPE_PARAM)
                    .map(|s| s.trim().to_uppercase())
                    .filter(|s| !s.is_empty());
                changes.insert(
                    ORIGINATOR_TYPE_PARAM.to_string(),
                    new_type
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
                self.originator_type = new_type;
            }
        }

        if command.parameter_exists(CHANNEL_TYPE_ID_PARAM) {
            let current_id = self.channel_type_cv_id();
            if command.is_change_in_long_parameter_named(CHANNEL_TYPE_ID_PARAM, current_id)? {
                let new_chan_id = command.long_value_of_parameter_named(CHANNEL_TYPE_ID_PARAM)?;
                self.channel_type = match new_chan_id {
                    Some(cid) => Some(resolve_channel_type_by_id(cid)?),
                    None => None,
                };
                changes.insert(
                    CHANNEL_TYPE_ID_PARAM.to_string(),
                    new_chan_id.map(Value::from).unwrap_or(Value::Null),
                );
            }
        } else if command.parameter_exists(CHANNEL_TYPE_PARAM) {
            if command.is_change_in_string_parameter_named(
                CHANNEL_TYPE_PARAM,
                self.channel_type.as_deref(),
            ) {
                let new_chan = command
                    .string_value_of_parameter_named(CHANNEL_TYPE_PARAM)
                    .map(|s| s.trim().to_uppercase())
                    .filter(|s| !s.is_empty());
                changes.insert(
                    CHANNEL_TYPE_PARAM.to_string(),
                    new_chan
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
                self.channel_type = new_chan;
            }
        }

        if command.parameter_exists(COMMISSION_RATE_PARAM) {
            if command.is_change_in_big_decimal_parameter_named(
                COMMISSION_RATE_PARAM,
                self.commission_rate,
            )? {
                let new_rate =
                    command.big_decimal_value_of_parameter_named(COMMISSION_RATE_PARAM)?;
                self.commission_rate = new_rate;
                changes.insert(
                    COMMISSION_RATE_PARAM.to_string(),
                    new_rate
                        .map(|d| Value::String(d.to_string()))
                        .unwrap_or(Value::Null),
                );
            }
        }

        Ok(changes)
    }
}

/// Join entity mapping a [`LoanOriginator`] to a Loan with a revenue share percentage (`m_loan_originator_mapping`).
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.domain.LoanOriginatorMapping`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoanOriginatorMapping {
    pub loan_id: i64,
    pub originator_id: i64,
    pub revenue_share_percentage: Decimal,
}

impl LoanOriginatorMapping {
    /// Creates a new [`LoanOriginatorMapping`] with an explicit `revenue_share_percentage`.
    pub const fn new(
        loan_id: i64,
        originator_id: i64,
        revenue_share_percentage: Decimal,
    ) -> Self {
        Self {
            loan_id,
            originator_id,
            revenue_share_percentage,
        }
    }

    /// Creates a [`LoanOriginatorMapping`] with a default `100%` revenue share, matching
    /// Java `LoanOriginatorMapping.create(loanId, originator)`.
    pub const fn create(loan_id: i64, originator_id: i64) -> Self {
        Self {
            loan_id,
            originator_id,
            revenue_share_percentage: Decimal::ONE_HUNDRED,
        }
    }

    /// Validates that `loan_id > 0`, `originator_id > 0`, and `revenue_share_percentage` is within `(0, 100]`.
    pub fn validate(&self) -> FineractResult<()> {
        if self.loan_id <= 0 {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.loan.originator.mapping.invalid.loan.id",
                "Loan ID for originator mapping must be greater than zero",
            )));
        }
        if self.originator_id <= 0 {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.loan.originator.mapping.invalid.originator.id",
                "Originator ID for originator mapping must be greater than zero",
            )));
        }
        if self.revenue_share_percentage <= Decimal::ZERO
            || self.revenue_share_percentage > Decimal::ONE_HUNDRED
        {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.loan.originator.mapping.invalid.revenue.share",
                    format!(
                        "Revenue share percentage must be between 0 (exclusive) and 100 (inclusive), got {}",
                        self.revenue_share_percentage
                    ),
                    vec![self.revenue_share_percentage.to_string().into()],
                ),
            ));
        }
        Ok(())
    }

    /// Computes this originator's commission amount for a given `disbursed_amount` and `commission_rate` percentage.
    pub fn calculate_commission(
        &self,
        disbursed_amount: Decimal,
        commission_rate_pct: Option<Decimal>,
    ) -> Decimal {
        let Some(rate) = commission_rate_pct else {
            return Decimal::ZERO;
        };
        if disbursed_amount <= Decimal::ZERO || rate <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let gross_commission = (disbursed_amount * rate) / Decimal::ONE_HUNDRED;
        ((gross_commission * self.revenue_share_percentage) / Decimal::ONE_HUNDRED)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
    }
}

/// Extracted inline originator payload from a loan application or disbursement JSON array.
///
/// Ported from `org.apache.fineract.portfolio.loanorigination.data.LoanApplicationOriginatorData`
/// and `LoanApplicationOriginatorDataValidator`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoanApplicationOriginatorData {
    pub id: Option<i64>,
    pub external_id: Option<String>,
    pub name: Option<String>,
    pub type_id: Option<i64>,
    pub channel_type_id: Option<i64>,
    pub revenue_share_percentage: Option<Decimal>,
}

impl LoanApplicationOriginatorData {
    /// Validates and extracts a [`LoanApplicationOriginatorData`] from a JSON object,
    /// ported from `LoanApplicationOriginatorDataValidator.validateAndExtract`.
    pub fn validate_and_extract(value: &Value) -> FineractResult<Self> {
        let Some(obj) = value.as_object() else {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(vec![ApiParameterError::parameter_error(
                    "validation.msg.loan.originator.invalid.element",
                    "Each originator entry must be a JSON object",
                    "originators",
                    Vec::new(),
                )]),
            ));
        };

        let validator = DataValidatorBuilder::for_resource("loan.originator");
        let id = obj.get("id").and_then(Value::as_i64);
        let external_id = obj
            .get(EXTERNAL_ID_PARAM)
            .and_then(Value::as_str)
            .map(str::to_string);

        if id.is_none()
            && external_id
                .as_deref()
                .map_or(true, |s| s.trim().is_empty())
        {
            validator.reset().parameter("id").fail_with_code(
                "or.externalId.required",
                "Either 'id' or 'externalId' must be provided for originator",
            );
        }

        let name = obj
            .get(NAME_PARAM)
            .and_then(Value::as_str)
            .map(str::to_string);
        validator
            .reset()
            .parameter(NAME_PARAM)
            .value(name.clone())
            .ignore_if_null()
            .not_exceeding_length_of(255);

        let type_id = obj.get(ORIGINATOR_TYPE_ID_PARAM).and_then(Value::as_i64);
        if let Some(tid) = type_id {
            if resolve_originator_type_by_id(tid).is_err() {
                validator
                    .reset()
                    .parameter(ORIGINATOR_TYPE_ID_PARAM)
                    .value(tid)
                    .fail_with_code(
                        "invalid.code.value",
                        &format!("Invalid code value id {tid} for {ORIGINATOR_TYPE_CODE_NAME}"),
                    );
            }
        }

        let channel_type_id = obj.get(CHANNEL_TYPE_ID_PARAM).and_then(Value::as_i64);
        if let Some(cid) = channel_type_id {
            if resolve_channel_type_by_id(cid).is_err() {
                validator
                    .reset()
                    .parameter(CHANNEL_TYPE_ID_PARAM)
                    .value(cid)
                    .fail_with_code(
                        "invalid.code.value",
                        &format!("Invalid code value id {cid} for {CHANNEL_TYPE_CODE_NAME}"),
                    );
            }
        }

        let revenue_share_percentage = obj
            .get(REVENUE_SHARE_PERCENTAGE_PARAM)
            .and_then(|v| v.as_str().and_then(|s| Decimal::from_str_exact(s).ok()).or_else(|| {
                v.as_f64().and_then(|n| Decimal::from_str_exact(&n.to_string()).ok())
            }));

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)?;

        Ok(Self {
            id,
            external_id,
            name,
            type_id,
            channel_type_id,
            revenue_share_percentage,
        })
    }
}

// =============================================================================
// 4. CO-APPLICANT DOMAIN (`m_loan_co_applicant`)
// =============================================================================

/// Domain entity representing a Co-Applicant / Co-Borrower attached to a Loan (`m_loan_co_applicant`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CoApplicant {
    pub id: i64,
    pub loan_id: i64,
    pub client_id: i64,
    pub relationship_cv_id: Option<i64>,
    pub income_share_percentage: Option<Decimal>,
    pub liability_share_percentage: Option<Decimal>,
}

impl CoApplicant {
    /// Creates a new [`CoApplicant`] entity.
    pub const fn new(
        id: i64,
        loan_id: i64,
        client_id: i64,
        relationship_cv_id: Option<i64>,
        income_share_percentage: Option<Decimal>,
        liability_share_percentage: Option<Decimal>,
    ) -> Self {
        Self {
            id,
            loan_id,
            client_id,
            relationship_cv_id,
            income_share_percentage,
            liability_share_percentage,
        }
    }

    /// Validates domain invariants on a [`CoApplicant`].
    pub fn validate(&self) -> FineractResult<()> {
        if self.loan_id <= 0 {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.coapplicant.loan.id.invalid",
                "Loan ID for co-applicant must be a positive integer",
            )));
        }
        if self.client_id <= 0 {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.coapplicant.client.id.invalid",
                "Client ID for co-applicant must be a positive integer",
            )));
        }
        if let Some(rel_id) = self.relationship_cv_id {
            if rel_id <= 0 {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.coapplicant.relationship.cv.id.invalid",
                    "Relationship code value ID must be greater than zero",
                )));
            }
        }
        if let Some(inc) = self.income_share_percentage {
            if inc < Decimal::ZERO || inc > Decimal::ONE_HUNDRED {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.coapplicant.income.share.out.of.bounds",
                        format!(
                            "Co-applicant income share percentage must be between 0 and 100, got {inc}"
                        ),
                        vec![inc.to_string().into()],
                    ),
                ));
            }
        }
        if let Some(liab) = self.liability_share_percentage {
            if liab < Decimal::ZERO || liab > Decimal::ONE_HUNDRED {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.coapplicant.liability.share.out.of.bounds",
                        format!(
                            "Co-applicant liability share percentage must be between 0 and 100, got {liab}"
                        ),
                        vec![liab.to_string().into()],
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Validates an `ADD COAPPLICANT` [`JsonCommand`] payload.
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(CO_APPLICANT_SUPPORTED_PARAMS)?;
        let validator = DataValidatorBuilder::for_resource(CO_APPLICANT_RESOURCE_NAME);

        let client_id = command.long_value_of_parameter_named("clientId")?;
        validator
            .reset()
            .parameter("clientId")
            .value(client_id)
            .not_null()
            .integer_greater_than_zero();

        let relationship_cv_id = if command.parameter_exists("relationshipCvId") {
            command.long_value_of_parameter_named("relationshipCvId")?
        } else {
            command.long_value_of_parameter_named("relationshipTypeId")?
        };
        if command.parameter_exists("relationshipCvId")
            || command.parameter_exists("relationshipTypeId")
        {
            validator
                .reset()
                .parameter("relationshipCvId")
                .value(relationship_cv_id)
                .ignore_if_null()
                .integer_greater_than_zero();
        }

        if command.parameter_exists("incomeSharePercentage") {
            let inc = command.big_decimal_value_of_parameter_named("incomeSharePercentage")?;
            validator
                .reset()
                .parameter("incomeSharePercentage")
                .value(inc)
                .ignore_if_null()
                .zero_or_positive_amount()
                .not_greater_than_max(Decimal::ONE_HUNDRED);
        }

        if command.parameter_exists("liabilitySharePercentage") {
            let liab = command.big_decimal_value_of_parameter_named("liabilitySharePercentage")?;
            validator
                .reset()
                .parameter("liabilitySharePercentage")
                .value(liab)
                .ignore_if_null()
                .zero_or_positive_amount()
                .not_greater_than_max(Decimal::ONE_HUNDRED);
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a [`CoApplicant`] from a validated [`JsonCommand`].
    pub fn from_json(id: i64, loan_id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;
        let client_id = command
            .long_value_of_parameter_named("clientId")?
            .unwrap_or_default();
        let relationship_cv_id = if command.parameter_exists("relationshipCvId") {
            command.long_value_of_parameter_named("relationshipCvId")?
        } else {
            command.long_value_of_parameter_named("relationshipTypeId")?
        };
        let income_share_percentage =
            command.big_decimal_value_of_parameter_named("incomeSharePercentage")?;
        let liability_share_percentage =
            command.big_decimal_value_of_parameter_named("liabilitySharePercentage")?;

        let co_applicant = Self {
            id,
            loan_id,
            client_id,
            relationship_cv_id,
            income_share_percentage,
            liability_share_percentage,
        };
        co_applicant.validate()?;
        Ok(co_applicant)
    }

    /// Computes the weighted monthly income contribution of this co-applicant.
    /// Defaults to `100%` of `gross_monthly_income` when `income_share_percentage` is `None`.
    pub fn weighted_monthly_income(&self, gross_monthly_income: Decimal) -> Decimal {
        let pct = self
            .income_share_percentage
            .unwrap_or(Decimal::ONE_HUNDRED);
        ((gross_monthly_income * pct) / Decimal::ONE_HUNDRED)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
    }

    /// Computes the weighted monthly debt obligation of this co-applicant.
    /// Defaults to `100%` of `monthly_debt_obligation` when `liability_share_percentage` is `None`.
    pub fn weighted_monthly_liability(&self, monthly_debt_obligation: Decimal) -> Decimal {
        let pct = self
            .liability_share_percentage
            .unwrap_or(Decimal::ONE_HUNDRED);
        ((monthly_debt_obligation * pct) / Decimal::ONE_HUNDRED)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
    }
}

// =============================================================================
// 5. UNDERWRITING SCORECARD ENGINE (`OriginationDecisionStatus`, `UnderwritingScorecardEngine`)
// =============================================================================

/// Decision outcome status produced by the [`UnderwritingScorecardEngine`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub enum OriginationDecisionStatus {
    #[default]
    #[serde(
        rename = "PENDING_REVIEW",
        alias = "PendingReview",
        alias = "pending_review"
    )]
    PendingReview,
    #[serde(rename = "APPROVED", alias = "Approved", alias = "approved")]
    Approved,
    #[serde(
        rename = "CONDITIONALLY_APPROVED",
        alias = "ConditionallyApproved",
        alias = "conditionally_approved"
    )]
    ConditionallyApproved,
    #[serde(rename = "DECLINED", alias = "Declined", alias = "declined")]
    Declined,
    #[serde(
        rename = "MANUAL_UNDERWRITING_REQUIRED",
        alias = "ManualUnderwritingRequired",
        alias = "manual_underwriting_required"
    )]
    ManualUnderwritingRequired,
}

impl OriginationDecisionStatus {
    pub const PENDING_REVIEW: Self = Self::PendingReview;
    pub const APPROVED: Self = Self::Approved;
    pub const CONDITIONALLY_APPROVED: Self = Self::ConditionallyApproved;
    pub const DECLINED: Self = Self::Declined;
    pub const MANUAL_UNDERWRITING_REQUIRED: Self = Self::ManualUnderwritingRequired;

    pub const fn code(self) -> &'static str {
        match self {
            Self::PendingReview => "originationDecisionStatus.pendingReview",
            Self::Approved => "originationDecisionStatus.approved",
            Self::ConditionallyApproved => "originationDecisionStatus.conditionallyApproved",
            Self::Declined => "originationDecisionStatus.declined",
            Self::ManualUnderwritingRequired => {
                "originationDecisionStatus.manualUnderwritingRequired"
            }
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PendingReview => "PENDING_REVIEW",
            Self::Approved => "APPROVED",
            Self::ConditionallyApproved => "CONDITIONALLY_APPROVED",
            Self::Declined => "DECLINED",
            Self::ManualUnderwritingRequired => "MANUAL_UNDERWRITING_REQUIRED",
        }
    }

    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved | Self::ConditionallyApproved)
    }

    pub const fn is_declined(self) -> bool {
        matches!(self, Self::Declined)
    }

    pub const fn requires_manual_underwriting(self) -> bool {
        matches!(self, Self::ManualUnderwritingRequired)
    }

    pub const fn is_pending_review(self) -> bool {
        matches!(self, Self::PendingReview)
    }
}

impl fmt::Display for OriginationDecisionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Financial profile of a co-applicant supplied to the [`UnderwritingScorecardEngine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoApplicantFinancialProfile {
    pub co_applicant: CoApplicant,
    pub gross_monthly_income: Decimal,
    pub monthly_debt_service: Decimal,
    pub bureau_score: Option<i32>,
}

impl CoApplicantFinancialProfile {
    pub const fn new(
        co_applicant: CoApplicant,
        gross_monthly_income: Decimal,
        monthly_debt_service: Decimal,
        bureau_score: Option<i32>,
    ) -> Self {
        Self {
            co_applicant,
            gross_monthly_income,
            monthly_debt_service,
            bureau_score,
        }
    }
}

/// Input parameters for evaluating a loan application in [`UnderwritingScorecardEngine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnderwritingApplicationInput {
    pub loan_id: Option<i64>,
    pub requested_principal: Decimal,
    pub proposed_monthly_installment: Decimal,
    pub primary_gross_monthly_income: Decimal,
    pub primary_existing_monthly_debt: Decimal,
    pub primary_bureau_score: i32,
    pub appraised_collateral_value: Option<Decimal>,
    pub co_applicants: Vec<CoApplicantFinancialProfile>,
}

impl UnderwritingApplicationInput {
    pub fn new(
        requested_principal: Decimal,
        proposed_monthly_installment: Decimal,
        primary_gross_monthly_income: Decimal,
        primary_existing_monthly_debt: Decimal,
        primary_bureau_score: i32,
        appraised_collateral_value: Option<Decimal>,
    ) -> Self {
        Self {
            loan_id: None,
            requested_principal,
            proposed_monthly_installment,
            primary_gross_monthly_income,
            primary_existing_monthly_debt,
            primary_bureau_score,
            appraised_collateral_value,
            co_applicants: Vec::new(),
        }
    }

    pub fn with_loan_id(mut self, loan_id: i64) -> Self {
        self.loan_id = Some(loan_id);
        self
    }

    pub fn with_co_applicants(mut self, co_applicants: Vec<CoApplicantFinancialProfile>) -> Self {
        self.co_applicants = co_applicants;
        self
    }
}

/// Policy thresholds for [`UnderwritingScorecardEngine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnderwritingScorecardConfig {
    /// Minimum credit bureau score for automatic approval (default: `700`).
    pub min_bureau_score_auto_approve: i32,
    /// Minimum credit bureau score for conditional approval (default: `620`).
    pub min_bureau_score_conditional: i32,
    /// Minimum credit bureau score for manual underwriting; scores below this are declined (default: `550`).
    pub min_bureau_score_manual_review: i32,
    /// Maximum DTI (%) for automatic approval (default: `36.00%`).
    pub max_dti_auto_approve: Decimal,
    /// Maximum DTI (%) for conditional approval (default: `43.00%`).
    pub max_dti_conditional: Decimal,
    /// Hard-cap DTI (%); ratios above this are declined (default: `50.00%`).
    pub max_dti_hard_cap: Decimal,
    /// Maximum LTV (%) for automatic approval (default: `80.00%`).
    pub max_ltv_auto_approve: Decimal,
    /// Maximum LTV (%) for conditional approval (default: `90.00%`).
    pub max_ltv_conditional: Decimal,
    /// Hard-cap LTV (%); ratios above this are declined (default: `100.00%`).
    pub max_ltv_hard_cap: Decimal,
}

impl Default for UnderwritingScorecardConfig {
    fn default() -> Self {
        Self {
            min_bureau_score_auto_approve: 700,
            min_bureau_score_conditional: 620,
            min_bureau_score_manual_review: 550,
            max_dti_auto_approve: dec!(36.00),
            max_dti_conditional: dec!(43.00),
            max_dti_hard_cap: dec!(50.00),
            max_ltv_auto_approve: dec!(80.00),
            max_ltv_conditional: dec!(90.00),
            max_ltv_hard_cap: dec!(100.00),
        }
    }
}

/// Structured underwriting decision output produced by [`UnderwritingScorecardEngine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginationDecision {
    pub loan_id: Option<i64>,
    pub status: OriginationDecisionStatus,
    pub dti_ratio: Decimal,
    pub ltv_ratio: Option<Decimal>,
    pub effective_bureau_score: i32,
    pub scorecard_points: Decimal,
    pub risk_grade: String,
    pub max_recommended_principal: Option<Decimal>,
    pub reasons: Vec<String>,
}

impl OriginationDecision {
    pub const fn is_approved(&self) -> bool {
        self.status.is_approved()
    }

    pub const fn is_declined(&self) -> bool {
        self.status.is_declined()
    }

    pub const fn requires_manual_underwriting(&self) -> bool {
        self.status.requires_manual_underwriting()
    }
}

/// Deterministic underwriting and credit-scoring engine computing Debt-to-Income (`DTI`),
/// Loan-to-Value (`LTV`), and Credit Bureau Score thresholds to produce an [`OriginationDecision`].
#[derive(Debug, Clone, Default)]
pub struct UnderwritingScorecardEngine {
    config: UnderwritingScorecardConfig,
}

impl UnderwritingScorecardEngine {
    /// Creates an [`UnderwritingScorecardEngine`] with standard policy thresholds.
    pub fn new() -> Self {
        Self {
            config: UnderwritingScorecardConfig::default(),
        }
    }

    /// Creates an [`UnderwritingScorecardEngine`] with custom policy thresholds.
    pub const fn with_config(config: UnderwritingScorecardConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the active [`UnderwritingScorecardConfig`].
    pub const fn config(&self) -> &UnderwritingScorecardConfig {
        &self.config
    }

    /// Computes the Debt-to-Income (`DTI`) percentage: `(total_monthly_debt / total_monthly_income) * 100`
    /// rounded to 2 decimal places (`HALF_EVEN`).
    pub fn calculate_dti(
        total_monthly_debt: Decimal,
        total_monthly_income: Decimal,
    ) -> FineractResult<Decimal> {
        if total_monthly_income <= Decimal::ZERO {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.underwriting.income.must.be.positive",
                "Total monthly income must be greater than zero to compute DTI",
            )));
        }
        if total_monthly_debt < Decimal::ZERO {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.underwriting.debt.cannot.be.negative",
                "Total monthly debt cannot be negative",
            )));
        }
        let dti = ((total_monthly_debt * Decimal::ONE_HUNDRED) / total_monthly_income)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
        Ok(dti)
    }

    /// Computes the Loan-to-Value (`LTV`) percentage: `(loan_amount / collateral_value) * 100`
    /// rounded to 2 decimal places (`HALF_EVEN`).
    pub fn calculate_ltv(
        loan_amount: Decimal,
        collateral_value: Decimal,
    ) -> FineractResult<Decimal> {
        if loan_amount <= Decimal::ZERO {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.underwriting.loan.amount.must.be.positive",
                "Requested loan amount must be greater than zero to compute LTV",
            )));
        }
        if collateral_value <= Decimal::ZERO {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.underwriting.collateral.value.must.be.positive",
                "Appraised collateral value must be greater than zero to compute LTV",
            )));
        }
        let ltv = ((loan_amount * Decimal::ONE_HUNDRED) / collateral_value)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
        Ok(ltv)
    }

    /// Evaluates pre-computed `dti`, optional `ltv`, and `bureau_score` against policy thresholds.
    pub fn evaluate_metrics(
        &self,
        dti_ratio: Decimal,
        ltv_ratio: Option<Decimal>,
        effective_bureau_score: i32,
    ) -> OriginationDecision {
        let mut reasons = Vec::new();
        let mut hard_decline = false;
        let mut manual_review = false;
        let mut conditional = false;

        // 1. Bureau Score threshold evaluation
        if effective_bureau_score < self.config.min_bureau_score_manual_review {
            hard_decline = true;
            reasons.push(format!(
                "Bureau score {} is below minimum threshold {}",
                effective_bureau_score, self.config.min_bureau_score_manual_review
            ));
        } else if effective_bureau_score < self.config.min_bureau_score_conditional {
            manual_review = true;
            reasons.push(format!(
                "Bureau score {} requires manual underwriting (< {})",
                effective_bureau_score, self.config.min_bureau_score_conditional
            ));
        } else if effective_bureau_score < self.config.min_bureau_score_auto_approve {
            conditional = true;
            reasons.push(format!(
                "Bureau score {} qualifies for conditional approval (< {})",
                effective_bureau_score, self.config.min_bureau_score_auto_approve
            ));
        }

        // 2. Debt-to-Income (DTI) threshold evaluation
        if dti_ratio > self.config.max_dti_hard_cap {
            hard_decline = true;
            reasons.push(format!(
                "DTI ratio {}% exceeds hard cap of {}%",
                dti_ratio, self.config.max_dti_hard_cap
            ));
        } else if dti_ratio > self.config.max_dti_conditional {
            manual_review = true;
            reasons.push(format!(
                "DTI ratio {}% exceeds conditional limit of {}% and requires manual underwriting",
                dti_ratio, self.config.max_dti_conditional
            ));
        } else if dti_ratio > self.config.max_dti_auto_approve {
            conditional = true;
            reasons.push(format!(
                "DTI ratio {}% exceeds auto-approve limit of {}%",
                dti_ratio, self.config.max_dti_auto_approve
            ));
        }

        // 3. Loan-to-Value (LTV) threshold evaluation (if secured)
        if let Some(ltv) = ltv_ratio {
            if ltv > self.config.max_ltv_hard_cap {
                hard_decline = true;
                reasons.push(format!(
                    "LTV ratio {}% exceeds hard cap of {}%",
                    ltv, self.config.max_ltv_hard_cap
                ));
            } else if ltv > self.config.max_ltv_conditional {
                manual_review = true;
                reasons.push(format!(
                    "LTV ratio {}% exceeds conditional limit of {}% and requires manual underwriting",
                    ltv, self.config.max_ltv_conditional
                ));
            } else if ltv > self.config.max_ltv_auto_approve {
                conditional = true;
                reasons.push(format!(
                    "LTV ratio {}% exceeds auto-approve limit of {}%",
                    ltv, self.config.max_ltv_auto_approve
                ));
            }
        }

        let status = if hard_decline {
            OriginationDecisionStatus::Declined
        } else if manual_review {
            OriginationDecisionStatus::ManualUnderwritingRequired
        } else if conditional {
            OriginationDecisionStatus::ConditionallyApproved
        } else {
            reasons.push("All DTI, LTV, and bureau score thresholds satisfied".to_string());
            OriginationDecisionStatus::Approved
        };

        let scorecard_points =
            Self::compute_scorecard_points(dti_ratio, ltv_ratio, effective_bureau_score);
        let risk_grade = Self::derive_risk_grade(scorecard_points).to_string();

        OriginationDecision {
            loan_id: None,
            status,
            dti_ratio,
            ltv_ratio,
            effective_bureau_score,
            scorecard_points,
            risk_grade,
            max_recommended_principal: None,
            reasons,
        }
    }

    /// Evaluates a complete [`UnderwritingApplicationInput`] (aggregating primary borrower and
    /// co-applicant income/liability shares) to produce an [`OriginationDecision`].
    pub fn evaluate(
        &self,
        input: &UnderwritingApplicationInput,
    ) -> FineractResult<OriginationDecision> {
        if input.requested_principal <= Decimal::ZERO {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.underwriting.loan.amount.must.be.positive",
                "Requested principal must be greater than zero",
            )));
        }
        if input.proposed_monthly_installment < Decimal::ZERO {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.underwriting.installment.cannot.be.negative",
                "Proposed monthly installment cannot be negative",
            )));
        }

        let mut total_monthly_income = input.primary_gross_monthly_income;
        let mut total_monthly_debt =
            input.primary_existing_monthly_debt + input.proposed_monthly_installment;

        // Representative bureau score: weighted or conservative lower-bound when co-applicants participate,
        // or income-weighted composite bureau score across applicants.
        let mut weighted_score_num =
            Decimal::from(input.primary_bureau_score) * input.primary_gross_monthly_income.max(Decimal::ONE);
        let mut weighted_score_den = input.primary_gross_monthly_income.max(Decimal::ONE);

        for profile in &input.co_applicants {
            profile.co_applicant.validate()?;
            let eff_inc = profile
                .co_applicant
                .weighted_monthly_income(profile.gross_monthly_income);
            let eff_debt = profile
                .co_applicant
                .weighted_monthly_liability(profile.monthly_debt_service);
            total_monthly_income += eff_inc;
            total_monthly_debt += eff_debt;

            if let Some(co_score) = profile.bureau_score {
                if eff_inc > Decimal::ZERO {
                    weighted_score_num += Decimal::from(co_score) * eff_inc;
                    weighted_score_den += eff_inc;
                }
            }
        }

        let dti_ratio = Self::calculate_dti(total_monthly_debt, total_monthly_income)?;
        let ltv_ratio = match input.appraised_collateral_value {
            Some(collateral_val) => {
                Some(Self::calculate_ltv(input.requested_principal, collateral_val)?)
            }
            None => None,
        };

        let effective_bureau_score = (weighted_score_num / weighted_score_den)
            .round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven)
            .to_string()
            .parse::<i32>()
            .unwrap_or(input.primary_bureau_score);

        let mut decision = self.evaluate_metrics(dti_ratio, ltv_ratio, effective_bureau_score);
        decision.loan_id = input.loan_id;
        decision.max_recommended_principal = match input.appraised_collateral_value {
            Some(collateral_val) if collateral_val > Decimal::ZERO => Some(
                ((collateral_val * self.config.max_ltv_conditional) / Decimal::ONE_HUNDRED)
                    .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven),
            ),
            _ => Some(input.requested_principal),
        };

        Ok(decision)
    }

    fn compute_scorecard_points(
        dti_ratio: Decimal,
        ltv_ratio: Option<Decimal>,
        bureau_score: i32,
    ) -> Decimal {
        // Bureau score component: up to 50 points (scaled 300..=850)
        let clamped_bureau = bureau_score.clamp(300, 850) - 300;
        let bureau_pts = (Decimal::from(clamped_bureau) * dec!(50)) / dec!(550);

        // DTI component: up to 30 points (0% DTI -> 30 pts, >= 60% DTI -> 0 pts)
        let dti_clamped = dti_ratio.max(Decimal::ZERO).min(dec!(60));
        let dti_pts = ((dec!(60) - dti_clamped) * dec!(30)) / dec!(60);

        // LTV component: up to 20 points (unsecured neutral 15 pts; secured 0% -> 20 pts, >= 120% -> 0 pts)
        let ltv_pts = match ltv_ratio {
            Some(ltv) => {
                let ltv_clamped = ltv.max(Decimal::ZERO).min(dec!(120));
                ((dec!(120) - ltv_clamped) * dec!(20)) / dec!(120)
            }
            None => dec!(15),
        };

        (bureau_pts + dti_pts + ltv_pts)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
    }

    fn derive_risk_grade(scorecard_points: Decimal) -> &'static str {
        if scorecard_points >= dec!(60) {
            "A"
        } else if scorecard_points >= dec!(45) {
            "B"
        } else if scorecard_points >= dec!(30) {
            "C"
        } else if scorecard_points >= dec!(15) {
            "D"
        } else {
            "E"
        }
    }
}

// =============================================================================
// 6. DUAL-DB REPOSITORY (`DualDbLoanOriginationRepository`)
// =============================================================================

/// Dual-database (PostgreSQL & MySQL/MariaDB + in-memory cache) repository for
/// `m_loan_originator`, `m_loan_originator_mapping`, and `m_loan_co_applicant`.
#[derive(Debug)]
pub struct DualDbLoanOriginationRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    next_id: AtomicI64,
    enable_originator_creation_during_app: AtomicBool,
    originators: DashMap<i64, LoanOriginator>,
    mappings: DashMap<(i64, i64), LoanOriginatorMapping>,
    co_applicants: DashMap<i64, CoApplicant>,
    decisions: DashMap<i64, OriginationDecision>,
    loan_submitted_status: DashMap<i64, String>,
    loan_primary_client: DashMap<i64, i64>,
}

impl Default for DualDbLoanOriginationRepository {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl DualDbLoanOriginationRepository {
    pub const ORIGINATOR_COLUMNS: &'static str =
        "id, external_id, name, status, originator_type_cv_id, channel_type_cv_id";
    pub const ORIGINATOR_MAPPING_COLUMNS: &'static str = "id, loan_id, originator_id";
    pub const CO_APPLICANT_COLUMNS: &'static str =
        "id, loan_id, client_id, relationship_cv_id, income_share_percentage, liability_share_percentage";

    /// Creates a new repository backed by a live `sqlx::AnyPool` and SQL dialect.
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            next_id: AtomicI64::new(1),
            enable_originator_creation_during_app: AtomicBool::new(false),
            originators: DashMap::new(),
            mappings: DashMap::new(),
            co_applicants: DashMap::new(),
            decisions: DashMap::new(),
            loan_submitted_status: DashMap::new(),
            loan_primary_client: DashMap::new(),
        }
    }

    /// Creates an in-memory repository (defaulting to PostgreSQL SQL generation dialect).
    pub fn in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    /// Alias for [`Self::in_memory`].
    pub fn new_in_memory() -> Self {
        Self::in_memory()
    }

    /// Creates an in-memory repository configured with a specific [`SqlDialect`].
    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            pool: None,
            dialect,
            next_id: AtomicI64::new(1),
            enable_originator_creation_during_app: AtomicBool::new(false),
            originators: DashMap::new(),
            mappings: DashMap::new(),
            co_applicants: DashMap::new(),
            decisions: DashMap::new(),
            loan_submitted_status: DashMap::new(),
            loan_primary_client: DashMap::new(),
        }
    }

    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub const fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    pub fn allocate_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Configures `enable-originator-creation-during-loan-application`.
    pub fn set_originator_creation_during_loan_application_enabled(&self, enabled: bool) {
        self.enable_originator_creation_during_app
            .store(enabled, Ordering::SeqCst);
    }

    pub fn is_originator_creation_during_loan_application_enabled(&self) -> bool {
        self.enable_originator_creation_during_app
            .load(Ordering::SeqCst)
    }

    /// Registers the lifecycle status code of a loan (e.g. `"loanStatusType.submitted.and.pending.approval"`
    /// vs `"loanStatusType.active"`) for enforcing `LoanNotInSubmittedStatusException`.
    pub fn register_loan_status(&self, loan_id: i64, status_code: impl Into<String>) {
        self.loan_submitted_status
            .insert(loan_id, status_code.into());
    }

    /// Registers the primary borrower `client_id` for a loan so co-applicant validation can prevent
    /// adding the primary borrower as their own co-applicant.
    pub fn register_loan_primary_client(&self, loan_id: i64, primary_client_id: i64) {
        self.loan_primary_client
            .insert(loan_id, primary_client_id);
    }

    fn ensure_loan_in_submitted_status(&self, loan_id: i64) -> FineractResult<()> {
        if let Some(status_ref) = self.loan_submitted_status.get(&loan_id) {
            let status = status_ref.value();
            if status != "loanStatusType.submitted.and.pending.approval"
                && !status.eq_ignore_ascii_case("SUBMITTED_AND_PENDING_APPROVAL")
            {
                return Err(loan_not_in_submitted_status_error(loan_id, status));
            }
        }
        Ok(())
    }

    // --- Dialect-Aware SQL Builders ---

    pub fn select_originator_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_originator WHERE id = ?",
                Self::ORIGINATOR_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_originator_by_external_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_originator WHERE external_id = ?",
                Self::ORIGINATOR_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_all_originators_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_originator ORDER BY id ASC",
                Self::ORIGINATOR_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_originator_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan_originator (id, external_id, name, status, originator_type_cv_id, channel_type_cv_id, created_on_utc, created_by, last_modified_on_utc, last_modified_by) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, 1, CURRENT_TIMESTAMP, 1)",
            dialect,
        )
    }

    pub fn update_originator_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_loan_originator SET name = ?, status = ?, originator_type_cv_id = ?, channel_type_cv_id = ?, last_modified_on_utc = CURRENT_TIMESTAMP WHERE id = ?",
            dialect,
        )
    }

    pub fn delete_originator_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders("DELETE FROM m_loan_originator WHERE id = ?", dialect)
    }

    pub fn select_mappings_by_loan_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_originator_mapping WHERE loan_id = ? ORDER BY id ASC",
                Self::ORIGINATOR_MAPPING_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_originator_mapping_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan_originator_mapping (id, loan_id, originator_id, created_on_utc, created_by, last_modified_on_utc, last_modified_by) VALUES (?, ?, ?, CURRENT_TIMESTAMP, 1, CURRENT_TIMESTAMP, 1)",
            dialect,
        )
    }

    pub fn delete_originator_mapping_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_loan_originator_mapping WHERE loan_id = ? AND originator_id = ?",
            dialect,
        )
    }

    pub fn select_co_applicants_by_loan_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_loan_co_applicant WHERE loan_id = ? ORDER BY id ASC",
                Self::CO_APPLICANT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_co_applicant_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loan_co_applicant (id, loan_id, client_id, relationship_cv_id, income_share_percentage, liability_share_percentage) VALUES (?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn delete_co_applicant_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_loan_co_applicant WHERE loan_id = ? AND id = ?",
            dialect,
        )
    }

    // --- Loan Originator Operations ---

    /// Creates and persists a [`LoanOriginator`], checking for duplicate `external_id`.
    pub async fn create_originator(
        &self,
        mut originator: LoanOriginator,
    ) -> FineractResult<LoanOriginator> {
        if let Some(ext_val) = originator.external_id.value() {
            if self
                .originators
                .iter()
                .any(|entry| entry.value().external_id.value() == Some(ext_val))
            {
                return Err(duplicate_originator_external_id_error(ext_val));
            }
        }

        if originator.id <= 0 {
            originator.id = self.allocate_id();
        } else {
            let _ = self
                .next_id
                .fetch_max(originator.id.saturating_add(1), Ordering::SeqCst);
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_originator_sql(self.dialect);
            sqlx::query(&sql)
                .bind(originator.id)
                .bind(originator.external_id.value())
                .bind(&originator.name)
                .bind(originator.status.value())
                .bind(originator.originator_type_cv_id())
                .bind(originator.channel_type_cv_id())
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.originators
            .insert(originator.id, originator.clone());
        Ok(originator)
    }

    /// Alias for [`Self::create_originator`].
    pub async fn save_originator(
        &self,
        originator: LoanOriginator,
    ) -> FineractResult<LoanOriginator> {
        self.create_originator(originator).await
    }

    /// Finds a [`LoanOriginator`] by internal ID, returning `Ok(None)` if not present.
    pub async fn find_originator_by_id(&self, id: i64) -> FineractResult<Option<LoanOriginator>> {
        if let Some(entry) = self.originators.get(&id) {
            return Ok(Some(entry.value().clone()));
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_originator_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let loaded = Self::map_originator_row(&row)?;
                self.originators.insert(loaded.id, loaded.clone());
                return Ok(Some(loaded));
            }
        }
        Ok(None)
    }

    /// Retrieves a [`LoanOriginator`] by ID or returns `LoanOriginatorNotFoundException`.
    pub async fn get_originator_by_id(&self, id: i64) -> FineractResult<LoanOriginator> {
        self.find_originator_by_id(id)
            .await?
            .ok_or_else(|| LoanOriginatorNotFoundException::by_id(id).into_fineract_error())
    }

    /// Finds a [`LoanOriginator`] by `external_id`.
    pub async fn find_originator_by_external_id(
        &self,
        external_id: &str,
    ) -> FineractResult<Option<LoanOriginator>> {
        if let Some(entry) = self
            .originators
            .iter()
            .find(|e| e.value().external_id.value() == Some(external_id))
        {
            return Ok(Some(entry.value().clone()));
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_originator_by_external_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(external_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let loaded = Self::map_originator_row(&row)?;
                self.originators.insert(loaded.id, loaded.clone());
                return Ok(Some(loaded));
            }
        }
        Ok(None)
    }

    /// Retrieves a [`LoanOriginator`] by `external_id` or returns `LoanOriginatorNotFoundException`.
    pub async fn get_originator_by_external_id(
        &self,
        external_id: &str,
    ) -> FineractResult<LoanOriginator> {
        self.find_originator_by_external_id(external_id)
            .await?
            .ok_or_else(|| {
                LoanOriginatorNotFoundException::by_external_id(external_id).into_fineract_error()
            })
    }

    /// Updates an existing [`LoanOriginator`].
    pub async fn update_originator(
        &self,
        originator: LoanOriginator,
    ) -> FineractResult<LoanOriginator> {
        let _existing = self.get_originator_by_id(originator.id).await?;
        if let Some(ref pool) = self.pool {
            let sql = Self::update_originator_sql(self.dialect);
            sqlx::query(&sql)
                .bind(&originator.name)
                .bind(originator.status.value())
                .bind(originator.originator_type_cv_id())
                .bind(originator.channel_type_cv_id())
                .bind(originator.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.originators
            .insert(originator.id, originator.clone());
        Ok(originator)
    }

    /// Deletes a [`LoanOriginator`] by ID if it is not mapped to any loan.
    ///
    /// Ported from `LoanOriginatorWritePlatformServiceImpl.delete`.
    pub async fn delete_originator(&self, originator_id: i64) -> FineractResult<LoanOriginator> {
        let originator = self.get_originator_by_id(originator_id).await?;
        let is_mapped = self
            .mappings
            .iter()
            .any(|m| m.value().originator_id == originator_id);
        if is_mapped {
            return Err(originator_cannot_be_deleted_error(originator_id));
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::delete_originator_sql(self.dialect);
            sqlx::query(&sql)
                .bind(originator_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.originators.remove(&originator_id);
        Ok(originator)
    }

    /// Lists all [`LoanOriginator`] records ordered by ID.
    pub fn list_originators(&self) -> Vec<LoanOriginator> {
        let mut items: Vec<LoanOriginator> =
            self.originators.iter().map(|e| e.value().clone()).collect();
        items.sort_by_key(|o| o.id);
        items
    }

    fn map_originator_row(row: &sqlx::any::AnyRow) -> FineractResult<LoanOriginator> {
        let id: i64 = row
            .try_get("id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let external_id_raw: Option<String> = row
            .try_get("external_id")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let name: Option<String> = row
            .try_get("name")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let status_raw: String = row
            .try_get("status")
            .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        let orig_type_id: Option<i64> = row.try_get("originator_type_cv_id").ok();
        let chan_type_id: Option<i64> = row.try_get("channel_type_cv_id").ok();

        let status = LoanOriginatorStatus::from_string(&status_raw)?;
        let originator_type = orig_type_id.and_then(|tid| resolve_originator_type_by_id(tid).ok());
        let channel_type = chan_type_id.and_then(|cid| resolve_channel_type_by_id(cid).ok());

        Ok(LoanOriginator {
            id,
            external_id: ExternalId::from_opt(external_id_raw),
            name: name.unwrap_or_default(),
            status,
            originator_type,
            channel_type,
            commission_rate: None,
        })
    }

    // --- Loan Originator Mapping Operations ---

    /// Attaches an active [`LoanOriginator`] to a loan (`m_loan_originator_mapping`),
    /// enforcing `LoanNotInSubmittedStatusException`, `LoanOriginatorNotActiveException`,
    /// `LoanOriginatorMappingAlreadyExistsException`, and revenue-share bounds (`<= 100%`).
    pub async fn attach_originator_to_loan(
        &self,
        loan_id: i64,
        originator_id: i64,
        revenue_share_percentage: Option<Decimal>,
    ) -> FineractResult<LoanOriginatorMapping> {
        self.ensure_loan_in_submitted_status(loan_id)?;
        let originator = self.get_originator_by_id(originator_id).await?;
        if !originator.status.is_active() {
            return Err(originator_not_active_error(
                originator_id,
                originator.status.value(),
            ));
        }

        if self.mappings.contains_key(&(loan_id, originator_id)) {
            return Err(originator_mapping_already_exists_error(
                loan_id,
                originator_id,
            ));
        }

        let share = revenue_share_percentage.unwrap_or(Decimal::ONE_HUNDRED);
        let mapping = LoanOriginatorMapping::new(loan_id, originator_id, share);
        mapping.validate()?;

        // If explicit < 100% shares are used, ensure cumulative explicit shares do not exceed 100%.
        if revenue_share_percentage.is_some() {
            let existing_sum: Decimal = self
                .list_mappings_for_loan(loan_id)
                .into_iter()
                .map(|m| m.revenue_share_percentage)
                .sum();
            if existing_sum + share > Decimal::ONE_HUNDRED {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.loan.originator.revenue.share.exceeds.100",
                        format!(
                            "Total originator revenue share percentage for loan {loan_id} would exceed 100% (existing: {existing_sum}%, requested: {share}%)"
                        ),
                        vec![loan_id.into(), (existing_sum + share).to_string().into()],
                    ),
                ));
            }
        }

        if let Some(ref pool) = self.pool {
            let map_id = self.allocate_id();
            let sql = Self::insert_originator_mapping_sql(self.dialect);
            sqlx::query(&sql)
                .bind(map_id)
                .bind(loan_id)
                .bind(originator_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.mappings
            .insert((loan_id, originator_id), mapping.clone());
        Ok(mapping)
    }

    /// Detaches a [`LoanOriginator`] from a loan (`m_loan_originator_mapping`).
    ///
    /// Ported from `LoanOriginatorWritePlatformServiceImpl.detachOriginatorFromLoan`.
    pub async fn detach_originator_from_loan(
        &self,
        loan_id: i64,
        originator_id: i64,
    ) -> FineractResult<LoanOriginatorMapping> {
        self.ensure_loan_in_submitted_status(loan_id)?;
        let _originator = self.get_originator_by_id(originator_id).await?;

        let Some((_, removed)) = self.mappings.remove(&(loan_id, originator_id)) else {
            return Err(originator_mapping_not_found_error(loan_id, originator_id));
        };

        if let Some(ref pool) = self.pool {
            let sql = Self::delete_originator_mapping_sql(self.dialect);
            sqlx::query(&sql)
                .bind(loan_id)
                .bind(originator_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        Ok(removed)
    }

    /// Lists all [`LoanOriginatorMapping`] records for a given `loan_id`.
    pub fn list_mappings_for_loan(&self, loan_id: i64) -> Vec<LoanOriginatorMapping> {
        let mut items: Vec<LoanOriginatorMapping> = self
            .mappings
            .iter()
            .filter(|e| e.value().loan_id == loan_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|m| m.originator_id);
        items
    }

    /// Lists all [`LoanOriginator`] entities attached to a given `loan_id`.
    pub fn list_originators_for_loan(&self, loan_id: i64) -> Vec<LoanOriginator> {
        self.list_mappings_for_loan(loan_id)
            .into_iter()
            .filter_map(|m| self.originators.get(&m.originator_id).map(|o| o.clone()))
            .collect()
    }

    /// Processes the `"originators"` array during loan application, resolving by ID or `externalId`
    /// and optionally creating new originators if `enable-originator-creation-during-loan-application` is enabled.
    ///
    /// Ported from `AbstractLoanOriginatorLinkingServiceImpl.processOriginatorsForLoanApplication`.
    pub async fn process_originators_for_loan_application(
        &self,
        loan_id: i64,
        originators_array: &[Value],
    ) -> FineractResult<Vec<LoanOriginatorMapping>> {
        let mut attached_ids = BTreeSet::new();
        let mut created_mappings = Vec::new();

        for element in originators_array {
            if !element.is_object() {
                continue;
            }
            let data = LoanApplicationOriginatorData::validate_and_extract(element)?;
            let originator_id = self
                .resolve_or_create_originator_id(&data, false)
                .await?;
            if !attached_ids.insert(originator_id) {
                continue;
            }
            if !self.mappings.contains_key(&(loan_id, originator_id)) {
                let mapping = LoanOriginatorMapping::new(
                    loan_id,
                    originator_id,
                    data.revenue_share_percentage.unwrap_or(Decimal::ONE_HUNDRED),
                );
                mapping.validate()?;
                self.mappings
                    .insert((loan_id, originator_id), mapping.clone());
                created_mappings.push(mapping);
            }
        }

        Ok(created_mappings)
    }

    /// Reconciles loan-originator mappings during loan disbursement so the loan's mappings
    /// match the supplied array (an empty slice detaches all current originators).
    ///
    /// Ported from `LoanOriginatorLinkingServiceImpl.processOriginatorsForLoanDisbursement`.
    pub async fn process_originators_for_loan_disbursement(
        &self,
        loan_id: i64,
        originators_array: &[Value],
    ) -> FineractResult<Vec<LoanOriginatorMapping>> {
        let mut requested_ids = BTreeMap::new();
        for element in originators_array {
            let data = LoanApplicationOriginatorData::validate_and_extract(element)?;
            let orig_id = self.resolve_or_create_originator_id(&data, true).await?;
            requested_ids.insert(
                orig_id,
                data.revenue_share_percentage.unwrap_or(Decimal::ONE_HUNDRED),
            );
        }

        let current = self.list_mappings_for_loan(loan_id);
        for mapping in &current {
            if !requested_ids.contains_key(&mapping.originator_id) {
                self.mappings.remove(&(loan_id, mapping.originator_id));
            }
        }

        for (orig_id, share) in requested_ids {
            if !self.mappings.contains_key(&(loan_id, orig_id)) {
                let mapping = LoanOriginatorMapping::new(loan_id, orig_id, share);
                mapping.validate()?;
                self.mappings.insert((loan_id, orig_id), mapping);
            }
        }

        Ok(self.list_mappings_for_loan(loan_id))
    }

    async fn resolve_or_create_originator_id(
        &self,
        data: &LoanApplicationOriginatorData,
        is_disbursement: bool,
    ) -> FineractResult<i64> {
        if let Some(id) = data.id {
            let originator = self.get_originator_by_id(id).await?;
            if !originator.status.is_active() {
                return Err(originator_not_active_error(
                    originator.id,
                    originator.status.value(),
                ));
            }
            return Ok(originator.id);
        }

        let ext_id = data.external_id.as_deref().unwrap_or_default();
        if let Some(existing) = self.find_originator_by_external_id(ext_id).await? {
            if !existing.status.is_active() {
                return Err(originator_not_active_error(
                    existing.id,
                    existing.status.value(),
                ));
            }
            return Ok(existing.id);
        }

        if !is_disbursement && !self.is_originator_creation_during_loan_application_enabled() {
            return Err(originator_creation_not_allowed_error(ext_id));
        }

        let originator_type = match data.type_id {
            Some(tid) => Some(resolve_originator_type_by_id(tid)?),
            None => None,
        };
        let channel_type = match data.channel_type_id {
            Some(cid) => Some(resolve_channel_type_by_id(cid)?),
            None => None,
        };

        let created = self
            .create_originator(LoanOriginator::create(
                ext_id.to_string(),
                data.name.clone().unwrap_or_default(),
                LoanOriginatorStatus::Active,
                originator_type,
                channel_type,
            ))
            .await?;
        Ok(created.id)
    }

    // --- Co-Applicant Operations ---

    /// Adds a [`CoApplicant`] to a loan, validating uniqueness of `(loan_id, client_id)`,
    /// ensuring the co-applicant is not the primary borrower, and checking cumulative
    /// income/liability share percentages (`<= 100%`).
    pub async fn add_co_applicant(
        &self,
        mut co_applicant: CoApplicant,
    ) -> FineractResult<CoApplicant> {
        co_applicant.validate()?;

        if let Some(primary_id) = self.loan_primary_client.get(&co_applicant.loan_id) {
            if *primary_id.value() == co_applicant.client_id {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.loan.coapplicant.cannot.be.primary.borrower",
                        format!(
                            "Client {} is already the primary borrower on loan {}",
                            co_applicant.client_id, co_applicant.loan_id
                        ),
                        vec![co_applicant.loan_id.into(), co_applicant.client_id.into()],
                    ),
                ));
            }
        }

        let existing_for_loan = self.list_co_applicants_for_loan(co_applicant.loan_id);
        if existing_for_loan
            .iter()
            .any(|c| c.client_id == co_applicant.client_id && c.id != co_applicant.id)
        {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.loan.coapplicant.already.exists",
                    format!(
                        "Client {} is already a co-applicant on loan {}",
                        co_applicant.client_id, co_applicant.loan_id
                    ),
                    vec![co_applicant.loan_id.into(), co_applicant.client_id.into()],
                ),
            ));
        }

        if let Some(inc_share) = co_applicant.income_share_percentage {
            let existing_inc: Decimal = existing_for_loan
                .iter()
                .filter_map(|c| c.income_share_percentage)
                .sum();
            if existing_inc + inc_share > Decimal::ONE_HUNDRED {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.loan.coapplicant.total.income.share.exceeds.100",
                        format!(
                            "Total co-applicant income share percentage for loan {} cannot exceed 100%",
                            co_applicant.loan_id
                        ),
                        vec![co_applicant.loan_id.into()],
                    ),
                ));
            }
        }

        if let Some(liab_share) = co_applicant.liability_share_percentage {
            let existing_liab: Decimal = existing_for_loan
                .iter()
                .filter_map(|c| c.liability_share_percentage)
                .sum();
            if existing_liab + liab_share > Decimal::ONE_HUNDRED {
                return Err(FineractError::DomainRule(
                    PlatformDomainRuleException::with_args(
                        "error.msg.loan.coapplicant.total.liability.share.exceeds.100",
                        format!(
                            "Total co-applicant liability share percentage for loan {} cannot exceed 100%",
                            co_applicant.loan_id
                        ),
                        vec![co_applicant.loan_id.into()],
                    ),
                ));
            }
        }

        if co_applicant.id <= 0 {
            co_applicant.id = self.allocate_id();
        } else {
            let _ = self
                .next_id
                .fetch_max(co_applicant.id.saturating_add(1), Ordering::SeqCst);
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_co_applicant_sql(self.dialect);
            sqlx::query(&sql)
                .bind(co_applicant.id)
                .bind(co_applicant.loan_id)
                .bind(co_applicant.client_id)
                .bind(co_applicant.relationship_cv_id)
                .bind(co_applicant.income_share_percentage.map(|d| d.to_string()))
                .bind(
                    co_applicant
                        .liability_share_percentage
                        .map(|d| d.to_string()),
                )
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.co_applicants
            .insert(co_applicant.id, co_applicant.clone());
        Ok(co_applicant)
    }

    /// Removes a [`CoApplicant`] from a loan by `(loan_id, co_applicant_id)`.
    pub async fn remove_co_applicant(
        &self,
        loan_id: i64,
        co_applicant_id: i64,
    ) -> FineractResult<CoApplicant> {
        let Some(existing) = self.co_applicants.get(&co_applicant_id).map(|e| e.clone()) else {
            return Err(FineractError::resource_not_found(
                "error.msg.loan.coapplicant.not.found",
                format!("Co-applicant with id {co_applicant_id} not found on loan {loan_id}"),
                co_applicant_id.to_string(),
            ));
        };

        if existing.loan_id != loan_id {
            return Err(FineractError::resource_not_found(
                "error.msg.loan.coapplicant.not.found",
                format!("Co-applicant with id {co_applicant_id} does not belong to loan {loan_id}"),
                co_applicant_id.to_string(),
            ));
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::delete_co_applicant_sql(self.dialect);
            sqlx::query(&sql)
                .bind(loan_id)
                .bind(co_applicant_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.co_applicants.remove(&co_applicant_id);
        Ok(existing)
    }

    /// Retrieves a [`CoApplicant`] by ID.
    pub fn get_co_applicant(&self, co_applicant_id: i64) -> FineractResult<CoApplicant> {
        self.co_applicants
            .get(&co_applicant_id)
            .map(|e| e.value().clone())
            .ok_or_else(|| {
                FineractError::resource_not_found(
                    "error.msg.loan.coapplicant.not.found",
                    format!("Co-applicant with id {co_applicant_id} not found"),
                    co_applicant_id.to_string(),
                )
            })
    }

    /// Lists all [`CoApplicant`] records for a given `loan_id` ordered by ID.
    pub fn list_co_applicants_for_loan(&self, loan_id: i64) -> Vec<CoApplicant> {
        let mut items: Vec<CoApplicant> = self
            .co_applicants
            .iter()
            .filter(|e| e.value().loan_id == loan_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|c| c.id);
        items
    }

    // --- Underwriting Decisions ---

    /// Stores an [`OriginationDecision`] for `loan_id`.
    pub fn save_origination_decision(
        &self,
        loan_id: i64,
        mut decision: OriginationDecision,
    ) -> OriginationDecision {
        decision.loan_id = Some(loan_id);
        self.decisions.insert(loan_id, decision.clone());
        decision
    }

    /// Retrieves the latest [`OriginationDecision`] for `loan_id`.
    pub fn get_origination_decision(&self, loan_id: i64) -> Option<OriginationDecision> {
        self.decisions.get(&loan_id).map(|d| d.value().clone())
    }
}

// =============================================================================
// 7. COMMAND HANDLERS
// =============================================================================

/// Command handler for `CREATE LOAN_ORIGINATOR`
/// (`org.apache.fineract.portfolio.loanorigination.handler.CreateLoanOriginatorCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateLoanOriginatorCommandHandler {
    repo: Arc<DualDbLoanOriginationRepository>,
}

impl CreateLoanOriginatorCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "LOAN_ORIGINATOR";

    pub fn new(repo: Arc<DualDbLoanOriginationRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbLoanOriginationRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbLoanOriginationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateLoanOriginatorCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let candidate = LoanOriginator::from_json(0, command)?;
        let saved = self.repo.create_originator(candidate).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_entity_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `UPDATE LOAN_ORIGINATOR`
/// (`org.apache.fineract.portfolio.loanorigination.handler.UpdateLoanOriginatorCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct UpdateLoanOriginatorCommandHandler {
    repo: Arc<DualDbLoanOriginationRepository>,
}

impl UpdateLoanOriginatorCommandHandler {
    pub const ACTION_NAME: &'static str = "UPDATE";
    pub const ENTITY_NAME: &'static str = "LOAN_ORIGINATOR";

    pub fn new(repo: Arc<DualDbLoanOriginationRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbLoanOriginationRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbLoanOriginationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateLoanOriginatorCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let id = command
            .entity_id
            .or_else(|| command.long_value_of_parameter_named("id").ok().flatten())
            .unwrap_or(0);
        let mut originator = self.repo.get_originator_by_id(id).await?;
        let changes = originator.update(command)?;
        if !changes.is_empty() {
            originator = self.repo.update_originator(originator).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(originator.id)
            .with_entity_external_id(originator.external_id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `DELETE LOAN_ORIGINATOR`
/// (`org.apache.fineract.portfolio.loanorigination.handler.DeleteLoanOriginatorCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct DeleteLoanOriginatorCommandHandler {
    repo: Arc<DualDbLoanOriginationRepository>,
}

impl DeleteLoanOriginatorCommandHandler {
    pub const ACTION_NAME: &'static str = "DELETE";
    pub const ENTITY_NAME: &'static str = "LOAN_ORIGINATOR";

    pub fn new(repo: Arc<DualDbLoanOriginationRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbLoanOriginationRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbLoanOriginationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DeleteLoanOriginatorCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let id = command
            .entity_id
            .or_else(|| command.long_value_of_parameter_named("id").ok().flatten())
            .unwrap_or(0);
        let deleted = self.repo.delete_originator(id).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(deleted.id)
            .with_entity_external_id(deleted.external_id)
            .build())
    }
}

/// Command handler for `ATTACH LOAN_ORIGINATOR`
/// (`org.apache.fineract.portfolio.loanorigination.handler.AttachLoanOriginatorCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct AttachLoanOriginatorCommandHandler {
    repo: Arc<DualDbLoanOriginationRepository>,
}

impl AttachLoanOriginatorCommandHandler {
    pub const ACTION_NAME: &'static str = "ATTACH";
    pub const ENTITY_NAME: &'static str = "LOAN_ORIGINATOR";

    pub fn new(repo: Arc<DualDbLoanOriginationRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbLoanOriginationRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbLoanOriginationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for AttachLoanOriginatorCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command
            .loan_id
            .or(command.entity_id)
            .or_else(|| command.long_value_of_parameter_named("loanId").ok().flatten())
            .unwrap_or(0);

        let originator_id = if let Some(sub_id) = command.subresource_id {
            sub_id
        } else if let Some(oid) = command.long_value_of_parameter_named("originatorId")? {
            oid
        } else if let Some(oid) = command.long_value_of_parameter_named("id")? {
            oid
        } else if let Some(ext_id) = command.string_value_of_parameter_named(EXTERNAL_ID_PARAM) {
            self.repo.get_originator_by_external_id(&ext_id).await?.id
        } else {
            0
        };

        let revenue_share =
            command.big_decimal_value_of_parameter_named(REVENUE_SHARE_PERCENTAGE_PARAM)?;
        let originator = self.repo.get_originator_by_id(originator_id).await?;
        let mapping = self
            .repo
            .attach_originator_to_loan(loan_id, originator_id, revenue_share)
            .await?;

        let mut changes = BTreeMap::new();
        changes.insert(
            REVENUE_SHARE_PERCENTAGE_PARAM.to_string(),
            Value::String(mapping.revenue_share_percentage.to_string()),
        );

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_loan_id(loan_id)
            .with_entity_id(loan_id)
            .with_sub_entity_id(originator_id)
            .with_sub_entity_external_id(originator.external_id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `DETACH LOAN_ORIGINATOR`
/// (`org.apache.fineract.portfolio.loanorigination.handler.DetachLoanOriginatorCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct DetachLoanOriginatorCommandHandler {
    repo: Arc<DualDbLoanOriginationRepository>,
}

impl DetachLoanOriginatorCommandHandler {
    pub const ACTION_NAME: &'static str = "DETACH";
    pub const ENTITY_NAME: &'static str = "LOAN_ORIGINATOR";

    pub fn new(repo: Arc<DualDbLoanOriginationRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbLoanOriginationRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbLoanOriginationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for DetachLoanOriginatorCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command
            .loan_id
            .or(command.entity_id)
            .or_else(|| command.long_value_of_parameter_named("loanId").ok().flatten())
            .unwrap_or(0);
        let originator_id = command
            .subresource_id
            .or_else(|| {
                command
                    .long_value_of_parameter_named("originatorId")
                    .ok()
                    .flatten()
            })
            .unwrap_or(0);

        let originator = self.repo.get_originator_by_id(originator_id).await?;
        let _removed = self
            .repo
            .detach_originator_from_loan(loan_id, originator_id)
            .await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_loan_id(loan_id)
            .with_entity_id(loan_id)
            .with_sub_entity_id(originator_id)
            .with_sub_entity_external_id(originator.external_id)
            .build())
    }
}

/// Command handler for adding a [`CoApplicant`] to a loan (`ADD COAPPLICANT` / `CREATE COAPPLICANT`).
#[derive(Debug, Clone, Default)]
pub struct AddCoApplicantCommandHandler {
    repo: Arc<DualDbLoanOriginationRepository>,
}

impl AddCoApplicantCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "COAPPLICANT";

    pub fn new(repo: Arc<DualDbLoanOriginationRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbLoanOriginationRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbLoanOriginationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for AddCoApplicantCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command
            .loan_id
            .or(command.entity_id)
            .or_else(|| command.long_value_of_parameter_named("loanId").ok().flatten())
            .unwrap_or(0);

        let candidate = CoApplicant::from_json(0, loan_id, command)?;
        let saved = self.repo.add_co_applicant(candidate).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_loan_id(saved.loan_id)
            .with_client_id(saved.client_id)
            .with_entity_id(saved.id)
            .with_sub_entity_id(saved.id)
            .build())
    }
}

/// Command handler for removing a [`CoApplicant`] from a loan (`DELETE COAPPLICANT` / `REMOVE COAPPLICANT`).
#[derive(Debug, Clone, Default)]
pub struct RemoveCoApplicantCommandHandler {
    repo: Arc<DualDbLoanOriginationRepository>,
}

impl RemoveCoApplicantCommandHandler {
    pub const ACTION_NAME: &'static str = "DELETE";
    pub const ENTITY_NAME: &'static str = "COAPPLICANT";

    pub fn new(repo: Arc<DualDbLoanOriginationRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self {
            repo: Arc::new(DualDbLoanOriginationRepository::in_memory()),
        }
    }

    pub fn repository(&self) -> &Arc<DualDbLoanOriginationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for RemoveCoApplicantCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let loan_id = command
            .loan_id
            .or_else(|| command.long_value_of_parameter_named("loanId").ok().flatten())
            .or(command.entity_id)
            .unwrap_or(0);

        let co_applicant_id = command
            .subresource_id
            .or_else(|| {
                command
                    .long_value_of_parameter_named("coApplicantId")
                    .ok()
                    .flatten()
            })
            .or_else(|| command.long_value_of_parameter_named("id").ok().flatten())
            .or(command.entity_id)
            .unwrap_or(0);

        let removed = self.repo.remove_co_applicant(loan_id, co_applicant_id).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_loan_id(removed.loan_id)
            .with_client_id(removed.client_id)
            .with_entity_id(removed.id)
            .with_sub_entity_id(removed.id)
            .build())
    }
}

// =============================================================================
// 8. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use serde_json::json;

    fn test_ctx() -> ExecutionContext {
        ExecutionContext::new(
            "default",
            NaiveDate::from_ymd_opt(2026, 10, 30).expect("valid date"),
        )
    }

    #[test]
    fn loan_originator_status_parsing_and_serialization() {
        assert_eq!(
            LoanOriginatorStatus::from_string("ACTIVE").unwrap(),
            LoanOriginatorStatus::Active
        );
        assert_eq!(
            LoanOriginatorStatus::from_string("pending").unwrap(),
            LoanOriginatorStatus::Pending
        );
        assert_eq!(
            LoanOriginatorStatus::from_string("InActive").unwrap(),
            LoanOriginatorStatus::Inactive
        );
        assert!(LoanOriginatorStatus::from_string("UNKNOWN").is_err());

        let json_str = serde_json::to_string(&LoanOriginatorStatus::Active).unwrap();
        assert_eq!(json_str, "\"ACTIVE\"");
        let parsed: LoanOriginatorStatus = serde_json::from_str("\"PENDING\"").unwrap();
        assert_eq!(parsed, LoanOriginatorStatus::Pending);
    }

    #[tokio::test]
    async fn create_update_and_delete_loan_originator_lifecycle() {
        let repo = Arc::new(DualDbLoanOriginationRepository::in_memory());
        let create_handler = CreateLoanOriginatorCommandHandler::new(Arc::clone(&repo));
        let update_handler = UpdateLoanOriginatorCommandHandler::new(Arc::clone(&repo));
        let delete_handler = DeleteLoanOriginatorCommandHandler::new(Arc::clone(&repo));
        let ctx = test_ctx();

        let create_cmd = JsonCommand::from_value(
            None,
            json!({
                "externalId": "ORIG-MERCH-001",
                "name": "Acme Solar Partner",
                "status": "ACTIVE",
                "originatorTypeId": 1,
                "channelTypeId": 3,
                "commissionRate": 2.5,
                "locale": "en"
            }),
        )
        .with_command_id(101);

        let create_res = create_handler.handle(&ctx, &create_cmd).await.unwrap();
        let orig_id = create_res.resource_id.expect("created originator id");
        assert_eq!(
            create_res
                .resource_external_id
                .as_ref()
                .and_then(|e| e.value()),
            Some("ORIG-MERCH-001")
        );

        let stored = repo.get_originator_by_id(orig_id).await.unwrap();
        assert_eq!(stored.name, "Acme Solar Partner");
        assert_eq!(stored.status, LoanOriginatorStatus::Active);
        assert_eq!(stored.originator_type.as_deref(), Some("MERCHANT"));
        assert_eq!(stored.channel_type.as_deref(), Some("API"));
        assert_eq!(stored.commission_rate, Some(dec!(2.5)));

        // Duplicate externalId must be rejected
        let dup_err = create_handler.handle(&ctx, &create_cmd).await.unwrap_err();
        assert_eq!(dup_err.http_status_code(), 403);

        // Update name, status, channelTypeId, and commissionRate
        let update_cmd = JsonCommand::from_value(
            Some(orig_id),
            json!({
                "name": "Acme Retail Partner",
                "status": "INACTIVE",
                "channelTypeId": 2,
                "commissionRate": 3.25,
                "locale": "en"
            }),
        );
        let update_res = update_handler.handle(&ctx, &update_cmd).await.unwrap();
        assert!(update_res.has_changes());

        let updated = repo.get_originator_by_id(orig_id).await.unwrap();
        assert_eq!(updated.name, "Acme Retail Partner");
        assert_eq!(updated.status, LoanOriginatorStatus::Inactive);
        assert_eq!(updated.channel_type.as_deref(), Some("IN_STORE"));
        assert_eq!(updated.commission_rate, Some(dec!(3.25)));

        // Delete unmapped originator succeeds
        let delete_cmd = JsonCommand::from_value(Some(orig_id), json!({}));
        delete_handler.handle(&ctx, &delete_cmd).await.unwrap();
        assert!(repo.find_originator_by_id(orig_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn attach_and_detach_loan_originator_enforces_domain_invariants() {
        let repo = Arc::new(DualDbLoanOriginationRepository::in_memory());
        let attach_handler = AttachLoanOriginatorCommandHandler::new(Arc::clone(&repo));
        let detach_handler = DetachLoanOriginatorCommandHandler::new(Arc::clone(&repo));
        let delete_handler = DeleteLoanOriginatorCommandHandler::new(Arc::clone(&repo));
        let ctx = test_ctx();

        let active_orig = repo
            .create_originator(LoanOriginator::new(
                0,
                "ORIG-ACTIVE-1",
                "Prime Broker",
                LoanOriginatorStatus::Active,
                Some("BROKER".to_string()),
                Some("ONLINE".to_string()),
                Some(dec!(1.50)),
            ))
            .await
            .unwrap();

        let pending_orig = repo
            .create_originator(LoanOriginator::new(
                0,
                "ORIG-PENDING-2",
                "Pending Affiliate",
                LoanOriginatorStatus::Pending,
                Some("AFFILIATE".to_string()),
                Some("AGGREGATOR".to_string()),
                None,
            ))
            .await
            .unwrap();

        let loan_id = 500;
        repo.register_loan_status(loan_id, "loanStatusType.submitted.and.pending.approval");

        // Attaching PENDING originator fails with LoanOriginatorNotActiveException
        let bad_attach = JsonCommand::from_value(None, json!({}))
            .with_loan_id(loan_id)
            .with_subresource_id(pending_orig.id);
        assert!(attach_handler.handle(&ctx, &bad_attach).await.is_err());

        // Attaching ACTIVE originator with 60% revenue share succeeds
        let good_attach = JsonCommand::from_value(
            None,
            json!({
                "revenueSharePercentage": 60.0,
                "locale": "en"
            }),
        )
        .with_loan_id(loan_id)
        .with_subresource_id(active_orig.id);

        let attach_res = attach_handler.handle(&ctx, &good_attach).await.unwrap();
        assert_eq!(attach_res.loan_id, Some(loan_id));
        assert_eq!(attach_res.sub_resource_id, Some(active_orig.id));

        // Duplicate attachment to same loan fails
        assert!(attach_handler.handle(&ctx, &good_attach).await.is_err());

        // Deleting mapped originator fails with LoanOriginatorCannotBeDeletedException
        let del_cmd = JsonCommand::from_value(Some(active_orig.id), json!({}));
        assert!(delete_handler.handle(&ctx, &del_cmd).await.is_err());

        // Commission calculation verifies 1.50% * 60% share on 10,000 = 90.00
        let mapping = &repo.list_mappings_for_loan(loan_id)[0];
        assert_eq!(
            mapping.calculate_commission(dec!(10000.00), active_orig.commission_rate),
            dec!(90.00)
        );

        // If loan moves to active status, detaching is blocked
        repo.register_loan_status(loan_id, "loanStatusType.active");
        let detach_cmd = JsonCommand::from_value(None, json!({}))
            .with_loan_id(loan_id)
            .with_subresource_id(active_orig.id);
        assert!(detach_handler.handle(&ctx, &detach_cmd).await.is_err());

        // While loan is in submitted status, detaching succeeds
        repo.register_loan_status(loan_id, "loanStatusType.submitted.and.pending.approval");
        detach_handler.handle(&ctx, &detach_cmd).await.unwrap();
        assert!(repo.list_mappings_for_loan(loan_id).is_empty());
    }

    #[tokio::test]
    async fn co_applicant_command_handlers_and_share_validation() {
        let repo = Arc::new(DualDbLoanOriginationRepository::in_memory());
        let add_handler = AddCoApplicantCommandHandler::new(Arc::clone(&repo));
        let remove_handler = RemoveCoApplicantCommandHandler::new(Arc::clone(&repo));
        let ctx = test_ctx();

        let loan_id = 700;
        let primary_client_id = 10;
        repo.register_loan_primary_client(loan_id, primary_client_id);

        // Primary borrower cannot be added as co-applicant
        let primary_as_co = JsonCommand::from_value(
            Some(loan_id),
            json!({
                "clientId": primary_client_id,
                "incomeSharePercentage": 50.0,
                "locale": "en"
            }),
        )
        .with_loan_id(loan_id);
        assert!(add_handler.handle(&ctx, &primary_as_co).await.is_err());

        // Add first co-applicant with 60% income share and 50% liability share
        let co1_cmd = JsonCommand::from_value(
            Some(loan_id),
            json!({
                "clientId": 11,
                "relationshipCvId": 2,
                "incomeSharePercentage": 60.0,
                "liabilitySharePercentage": 50.0,
                "locale": "en"
            }),
        )
        .with_loan_id(loan_id);
        let co1_res = add_handler.handle(&ctx, &co1_cmd).await.unwrap();
        let co1_id = co1_res.resource_id.unwrap();

        // Duplicate client on same loan is rejected
        assert!(add_handler.handle(&ctx, &co1_cmd).await.is_err());

        // Second co-applicant with 50% income share would push total to 110% -> rejected
        let co2_excess_cmd = JsonCommand::from_value(
            Some(loan_id),
            json!({
                "clientId": 12,
                "incomeSharePercentage": 50.0,
                "liabilitySharePercentage": 30.0,
                "locale": "en"
            }),
        )
        .with_loan_id(loan_id);
        assert!(add_handler.handle(&ctx, &co2_excess_cmd).await.is_err());

        // Second co-applicant with 40% income share (total 100%) succeeds
        let co2_valid_cmd = JsonCommand::from_value(
            Some(loan_id),
            json!({
                "clientId": 12,
                "relationshipCvId": 3,
                "incomeSharePercentage": 40.0,
                "liabilitySharePercentage": 50.0,
                "locale": "en"
            }),
        )
        .with_loan_id(loan_id);
        let co2_res = add_handler.handle(&ctx, &co2_valid_cmd).await.unwrap();
        let co2_id = co2_res.resource_id.unwrap();

        let list = repo.list_co_applicants_for_loan(loan_id);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, co1_id);
        assert_eq!(list[1].id, co2_id);

        // Remove first co-applicant
        let rem_cmd = JsonCommand::from_value(None, json!({}))
            .with_loan_id(loan_id)
            .with_subresource_id(co1_id);
        remove_handler.handle(&ctx, &rem_cmd).await.unwrap();
        assert_eq!(repo.list_co_applicants_for_loan(loan_id).len(), 1);
    }

    #[test]
    fn underwriting_scorecard_engine_dti_ltv_and_decision_tiers() {
        let engine = UnderwritingScorecardEngine::new();

        // 1. Prime auto-approval: DTI = (1000 + 800) / 6000 = 30.00% (<= 36%), LTV = 75.00% (<= 80%), Bureau = 740 (>= 700)
        let prime_input = UnderwritingApplicationInput::new(
            dec!(150000.00),
            dec!(800.00),
            dec!(6000.00),
            dec!(1000.00),
            740,
            Some(dec!(200000.00)),
        )
        .with_loan_id(1);
        let prime_decision = engine.evaluate(&prime_input).unwrap();
        assert_eq!(prime_decision.status, OriginationDecisionStatus::Approved);
        assert_eq!(prime_decision.dti_ratio, dec!(30.00));
        assert_eq!(prime_decision.ltv_ratio, Some(dec!(75.00)));
        assert_eq!(prime_decision.risk_grade, "A");

        // 2. Conditional approval: DTI = 40.00% (36%..=43%), LTV = 85.00% (80%..=90%), Bureau = 660 (620..700)
        let cond_decision =
            engine.evaluate_metrics(dec!(40.00), Some(dec!(85.00)), 660);
        assert_eq!(
            cond_decision.status,
            OriginationDecisionStatus::ConditionallyApproved
        );
        assert!(cond_decision.is_approved());

        // 3. Manual underwriting required: DTI = 46.00% (43%..=50%) or Bureau = 580 (550..620)
        let manual_decision =
            engine.evaluate_metrics(dec!(46.00), Some(dec!(75.00)), 680);
        assert_eq!(
            manual_decision.status,
            OriginationDecisionStatus::ManualUnderwritingRequired
        );
        assert!(manual_decision.requires_manual_underwriting());

        // 4. Hard decline: DTI = 55.00% (> 50%) or LTV = 105.00% (> 100%) or Bureau = 510 (< 550)
        let declined_decision =
            engine.evaluate_metrics(dec!(55.00), Some(dec!(105.00)), 510);
        assert_eq!(declined_decision.status, OriginationDecisionStatus::Declined);
        assert!(declined_decision.is_declined());
        assert_eq!(declined_decision.reasons.len(), 3);
    }

    #[test]
    fn underwriting_scorecard_with_co_applicant_improves_dti_to_approval() {
        let engine = UnderwritingScorecardEngine::new();

        // Primary borrower alone: debt = 1200 + 1000 = 2200, income = 4000 -> DTI = 55.00% (Declined)
        let solo_input = UnderwritingApplicationInput::new(
            dec!(80000.00),
            dec!(1000.00),
            dec!(4000.00),
            dec!(1200.00),
            720,
            Some(dec!(100000.00)),
        );
        let solo_decision = engine.evaluate(&solo_input).unwrap();
        assert_eq!(solo_decision.status, OriginationDecisionStatus::Declined);
        assert_eq!(solo_decision.dti_ratio, dec!(55.00));

        // With co-applicant contributing 80% of 5000 income (+4000) and 50% of 400 debt (+200):
        // Combined debt = 2200 + 200 = 2400, Combined income = 4000 + 4000 = 8000 -> DTI = 30.00% (Approved!)
        let co_app = CoApplicant::new(1, 99, 202, Some(1), Some(dec!(80.0)), Some(dec!(50.0)));
        let joint_input = solo_input.with_co_applicants(vec![CoApplicantFinancialProfile::new(
            co_app,
            dec!(5000.00),
            dec!(400.00),
            Some(760),
        )]);
        let joint_decision = engine.evaluate(&joint_input).unwrap();
        assert_eq!(joint_decision.status, OriginationDecisionStatus::Approved);
        assert_eq!(joint_decision.dti_ratio, dec!(30.00));
        assert_eq!(joint_decision.ltv_ratio, Some(dec!(80.00)));
    }

    #[test]
    fn dual_db_sql_queries_rewrite_for_postgres_and_mysql() {
        let pg_insert =
            DualDbLoanOriginationRepository::insert_originator_sql(SqlDialect::Postgres);
        assert!(pg_insert.contains("$1") && pg_insert.contains("$6"));

        let my_insert = DualDbLoanOriginationRepository::insert_originator_sql(SqlDialect::MySql);
        assert!(my_insert.contains('?') && !my_insert.contains("$1"));

        let pg_co = DualDbLoanOriginationRepository::insert_co_applicant_sql(SqlDialect::Postgres);
        assert!(pg_co.contains("$1") && pg_co.contains("$6"));
    }
}
