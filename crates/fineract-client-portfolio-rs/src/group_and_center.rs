//! Groups, Centers, Calendar Instances, Meetings, Client Attendance, Funds, Payment Types/Details, and Notes.
//!
//! Ported from:
//! - `org.apache.fineract.portfolio.group.domain.*` (`Group`, `GroupLevel`, `GroupRole`, `GroupTypes`, `GroupingTypeStatus`)
//! - `org.apache.fineract.portfolio.group.service.*` & `handler.*`
//! - `org.apache.fineract.portfolio.calendar.domain.*` (`Calendar`, `CalendarInstance`, `CalendarEntityType`, `CalendarType`)
//! - `org.apache.fineract.portfolio.meeting.domain.*` & `attendance.domain.*` (`Meeting`, `ClientAttendance`, `AttendanceType`)
//! - `org.apache.fineract.portfolio.fund.domain.Fund` & `handler.CreateFundCommandHandler`
//! - `org.apache.fineract.portfolio.paymenttype.domain.PaymentType` & `paymentdetail.domain.PaymentDetail`
//! - `org.apache.fineract.portfolio.note.domain.*` (`Note`, `NoteType`) & `handler.NoteCreateCommandHandler`

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{AnyPool, Row};

use fineract_command_rs::{CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder};
use fineract_core_rs::{
    CalendarUtils, DialectSql, ExecutionContext, ExternalId, JsonCommand, RecurrenceRule,
    SqlDialect,
};
use fineract_validation_rs::{
    ApiErrorArg, ApiParameterError, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException, PlatformDomainRuleException,
};

// =============================================================================
// 1. GROUPING TYPE STATUS & GROUP TYPES / LEVEL
// =============================================================================

/// Status of a Group or Center (`m_group.status_enum`).
///
/// Ported from `org.apache.fineract.portfolio.group.domain.GroupingTypeStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum GroupingTypeStatus {
    #[default]
    Invalid = 0,
    Pending = 100,
    Active = 300,
    TransferInProgress = 303,
    TransferOnHold = 304,
    Closed = 600,
}

impl GroupingTypeStatus {
    pub const INVALID: Self = Self::Invalid;
    pub const PENDING: Self = Self::Pending;
    pub const ACTIVE: Self = Self::Active;
    pub const TRANSFER_IN_PROGRESS: Self = Self::TransferInProgress;
    pub const TRANSFER_ON_HOLD: Self = Self::TransferOnHold;
    pub const CLOSED: Self = Self::Closed;

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn as_int(self) -> i32 {
        self as i32
    }

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "groupingStatusType.invalid",
            Self::Pending => "groupingStatusType.pending",
            Self::Active => "groupingStatusType.active",
            Self::TransferInProgress => "clientStatusType.transfer.in.progress",
            Self::TransferOnHold => "clientStatusType.transfer.on.hold",
            Self::Closed => "groupingStatusType.closed",
        }
    }

    pub const fn from_i32(status_value: i32) -> Self {
        match status_value {
            100 => Self::Pending,
            300 => Self::Active,
            303 => Self::TransferInProgress,
            304 => Self::TransferOnHold,
            600 => Self::Closed,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(status_value: Option<i32>) -> Self {
        match status_value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    pub const fn from_i16(status_value: i16) -> Self {
        Self::from_i32(status_value as i32)
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn is_submitted_and_pending_approval(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }

    pub const fn is_transfer_in_progress(self) -> bool {
        matches!(self, Self::TransferInProgress)
    }

    pub const fn is_transfer_on_hold(self) -> bool {
        matches!(self, Self::TransferOnHold)
    }

    pub const fn is_under_transfer(self) -> bool {
        matches!(self, Self::TransferInProgress | Self::TransferOnHold)
    }
}

/// Group type hierarchy level enum (`m_group_level.id` / `m_group.level_id`).
///
/// Ported from `org.apache.fineract.portfolio.group.domain.GroupTypes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum GroupTypes {
    #[default]
    Invalid = 0,
    Center = 1,
    Group = 2,
}

impl GroupTypes {
    pub const INVALID: Self = Self::Invalid;
    pub const CENTER: Self = Self::Center;
    pub const GROUP: Self = Self::Group;

    pub const fn id(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn parent_id(self) -> Option<i32> {
        match self {
            Self::Invalid | Self::Center => None,
            Self::Group => Some(1),
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "groupTypes.invalid",
            Self::Center => "Center",
            Self::Group => "Group",
        }
    }

    pub const fn from_int(level_id: i32) -> Self {
        match level_id {
            1 => Self::Center,
            2 => Self::Group,
            _ => Self::Invalid,
        }
    }

    pub const fn is_center(self) -> bool {
        matches!(self, Self::Center)
    }

    pub const fn is_group(self) -> bool {
        matches!(self, Self::Group)
    }

    pub const fn can_have_clients(self) -> bool {
        matches!(self, Self::Group)
    }

    pub const fn is_super_parent(self) -> bool {
        matches!(self, Self::Center)
    }

    pub const fn is_recursable(self) -> bool {
        matches!(self, Self::Center)
    }
}

/// Group hierarchy level metadata (`m_group_level` table).
///
/// Ported from `org.apache.fineract.portfolio.group.domain.GroupLevel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLevel {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub super_parent: bool,
    pub level_name: String,
    pub recursable: bool,
    pub can_have_clients: bool,
}

impl GroupLevel {
    pub const CENTER_LEVEL_ID: i32 = 1;
    pub const GROUP_LEVEL_ID: i32 = 2;
    pub const CENTER: i32 = 1;
    pub const GROUP: i32 = 2;

    pub fn center() -> Self {
        Self {
            id: Self::CENTER_LEVEL_ID,
            parent_id: None,
            super_parent: true,
            level_name: "Center".to_string(),
            recursable: true,
            can_have_clients: false,
        }
    }

    pub fn group() -> Self {
        Self {
            id: Self::GROUP_LEVEL_ID,
            parent_id: Some(Self::CENTER_LEVEL_ID),
            super_parent: false,
            level_name: "Group".to_string(),
            recursable: false,
            can_have_clients: true,
        }
    }

    pub fn from_id(level_id: i32) -> Option<Self> {
        match level_id {
            Self::CENTER_LEVEL_ID => Some(Self::center()),
            Self::GROUP_LEVEL_ID => Some(Self::group()),
            _ => None,
        }
    }

    pub fn group_type(&self) -> GroupTypes {
        GroupTypes::from_int(self.id)
    }

    pub fn is_center(&self) -> bool {
        self.level_name.eq_ignore_ascii_case("Center") || self.id == Self::CENTER_LEVEL_ID
    }

    pub fn is_group(&self) -> bool {
        self.level_name.eq_ignore_ascii_case("Group") || self.id == Self::GROUP_LEVEL_ID
    }
}

// =============================================================================
// 2. GROUP ROLE & GROUP (CENTERS AND GROUPS)
// =============================================================================

/// Client role assignment within a Group (`m_group_roles` table).
///
/// Ported from `org.apache.fineract.portfolio.group.domain.GroupRole`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRole {
    pub id: i64,
    pub group_id: i64,
    pub client_id: i64,
    pub role_cv_id: i32,
}

impl GroupRole {
    pub const fn new(id: i64, group_id: i64, client_id: i64, role_cv_id: i32) -> Self {
        Self {
            id,
            group_id,
            client_id,
            role_cv_id,
        }
    }
}

/// Supported JSON parameters for creating/updating a Group (`GroupingTypesApiConstants.GROUP_REQUEST_DATA_PARAMETERS`).
pub const GROUP_SUPPORTED_PARAMS: &[&str] = &[
    "locale",
    "dateFormat",
    "name",
    "externalId",
    "officeId",
    "staffId",
    "centerId",
    "active",
    "activationDate",
    "clientMembers",
    "submittedOnDate",
    "datatables",
    "accountNo",
];

/// Supported JSON parameters for creating/updating a Center (`GroupingTypesApiConstants.CENTER_REQUEST_DATA_PARAMETERS`).
pub const CENTER_SUPPORTED_PARAMS: &[&str] = &[
    "locale",
    "dateFormat",
    "name",
    "externalId",
    "officeId",
    "staffId",
    "active",
    "activationDate",
    "groupMembers",
    "submittedOnDate",
    "datatables",
    "accountNo",
];

/// Supported JSON parameters for activating a Group or Center (`GroupingTypesApiConstants.ACTIVATION_REQUEST_DATA_PARAMETERS`).
pub const GROUP_ACTIVATION_SUPPORTED_PARAMS: &[&str] =
    &["locale", "dateFormat", "activationDate"];

/// Supported JSON parameters for associating/disassociating clients (`GroupingTypesApiConstants.GROUP_CLIENTS_ASSOCIATION_DATA_PARAMETERS`).
pub const GROUP_CLIENT_ASSOCIATION_SUPPORTED_PARAMS: &[&str] = &["clientMembers"];

/// Unified Center and Group domain entity (`m_group` table).
///
/// Ported from `org.apache.fineract.portfolio.group.domain.Group`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: i64,
    pub account_no: Option<String>,
    pub external_id: ExternalId,
    pub status: GroupingTypeStatus,
    pub activation_date: Option<NaiveDate>,
    pub office_id: i64,
    pub staff_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub level_id: i32,
    pub name: String,
    pub hierarchy: String,
    pub closure_reason_cv_id: Option<i32>,
    pub closed_on_date: Option<NaiveDate>,
    pub submitted_on_date: NaiveDate,
    pub client_member_ids: Vec<i64>,
    pub roles: Vec<GroupRole>,
}

impl Group {
    /// Creates a new Center (`level_id == 1`).
    #[allow(clippy::too_many_arguments)]
    pub fn new_center(
        id: i64,
        office_id: i64,
        staff_id: Option<i64>,
        name: impl Into<String>,
        external_id: ExternalId,
        active: bool,
        activation_date: Option<NaiveDate>,
        submitted_on_date: NaiveDate,
        business_date: NaiveDate,
        office_opening_date: Option<NaiveDate>,
    ) -> FineractResult<Self> {
        let mut center = Self {
            id,
            account_no: if id > 0 {
                Some(format!("{id:09}"))
            } else {
                None
            },
            external_id,
            status: GroupingTypeStatus::Pending,
            activation_date: None,
            office_id,
            staff_id,
            parent_id: None,
            level_id: GroupTypes::Center.id(),
            name: name.into(),
            hierarchy: if id > 0 {
                format!(".{id}.")
            } else {
                String::new()
            },
            closure_reason_cv_id: None,
            closed_on_date: None,
            submitted_on_date,
            client_member_ids: Vec::new(),
            roles: Vec::new(),
        };

        center.validate_submitted_on_date(business_date, office_opening_date)?;

        if active {
            let act_date = activation_date.ok_or_else(|| {
                FineractError::Validation(PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.center.activationDate.cannot.be.blank",
                    "The parameter `activationDate` cannot be blank when `active` is true.",
                    "activationDate",
                ))
            })?;
            center.activate(act_date, business_date, office_opening_date, None, None)?;
        }

        Ok(center)
    }

    /// Creates a new Group (`level_id == 2`), optionally attached to a parent Center.
    #[allow(clippy::too_many_arguments)]
    pub fn new_group(
        id: i64,
        office_id: i64,
        staff_id: Option<i64>,
        parent_id: Option<i64>,
        parent_hierarchy: Option<&str>,
        name: impl Into<String>,
        external_id: ExternalId,
        active: bool,
        activation_date: Option<NaiveDate>,
        submitted_on_date: NaiveDate,
        client_member_ids: Vec<i64>,
        business_date: NaiveDate,
        office_opening_date: Option<NaiveDate>,
        min_clients: Option<usize>,
        max_clients: Option<usize>,
    ) -> FineractResult<Self> {
        let mut deduped_clients = Vec::with_capacity(client_member_ids.len());
        let mut seen = BTreeSet::new();
        for cid in client_member_ids {
            if !seen.insert(cid) {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.group.client.exist.in.group",
                    format!("Client with identifier {cid} is already exists in Group with identifier {id}"),
                    vec![ApiErrorArg::new(cid), ApiErrorArg::new(id)],
                )
                .into());
            }
            deduped_clients.push(cid);
        }

        let mut group = Self {
            id,
            account_no: if id > 0 {
                Some(format!("{id:09}"))
            } else {
                None
            },
            external_id,
            status: GroupingTypeStatus::Pending,
            activation_date: None,
            office_id,
            staff_id,
            parent_id,
            level_id: GroupTypes::Group.id(),
            name: name.into(),
            hierarchy: String::new(),
            closure_reason_cv_id: None,
            closed_on_date: None,
            submitted_on_date,
            client_member_ids: deduped_clients,
            roles: Vec::new(),
        };

        group.generate_hierarchy(parent_hierarchy);
        group.validate_submitted_on_date(business_date, office_opening_date)?;

        if active {
            let act_date = activation_date.ok_or_else(|| {
                FineractError::Validation(PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.group.activationDate.cannot.be.blank",
                    "The parameter `activationDate` cannot be blank when `active` is true.",
                    "activationDate",
                ))
            })?;
            group.activate(
                act_date,
                business_date,
                office_opening_date,
                min_clients,
                max_clients,
            )?;
        } else if let Some(max) = max_clients {
            if group.client_member_ids.len() > max {
                return Err(Self::member_count_out_of_range_error(min_clients, Some(max)));
            }
        }

        Ok(group)
    }

    /// Validates JSON payload for creating a Group (`GroupingTypesDataValidator.validateForCreateGroup`).
    pub fn validate_for_create_group(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(GROUP_SUPPORTED_PARAMS)?;
        let validator = DataValidatorBuilder::for_resource("group");

        let name = command.string_value_of_parameter_named("name");
        let validator = validator
            .parameter("name")
            .value(name.as_deref())
            .not_blank()
            .not_exceeding_length_of(100);

        let external_id = command.string_value_of_parameter_named("externalId");
        let validator = validator
            .parameter("externalId")
            .value(external_id.as_deref())
            .ignore_if_null()
            .not_exceeding_length_of(100);

        let center_id = command.long_value_of_parameter_named("centerId")?;
        let office_id = command.long_value_of_parameter_named("officeId")?;
        let validator = if center_id.is_none() {
            validator
                .parameter("officeId")
                .value(office_id)
                .not_null()
                .integer_greater_than_zero()
        } else {
            validator
                .parameter("officeId")
                .value(office_id)
                .ignore_if_null()
                .integer_greater_than_zero()
        };

        let validator = validator
            .parameter("centerId")
            .value(center_id)
            .ignore_if_null()
            .long_greater_than_zero();

        let staff_id = command.long_value_of_parameter_named("staffId")?;
        let validator = validator
            .parameter("staffId")
            .value(staff_id)
            .ignore_if_null()
            .long_greater_than_zero();

        let active = command.boolean_object_value_of_parameter_named("active");
        let validator = if let Some(is_active) = active {
            if is_active {
                let activation_date =
                    command.local_date_value_of_parameter_named("activationDate")?;
                validator
                    .parameter("activationDate")
                    .value(activation_date)
                    .not_null()
            } else {
                validator
            }
        } else {
            validator
        };

        validator.throw_validation_errors()?;
        Ok(())
    }

    /// Validates JSON payload for creating a Center (`GroupingTypesDataValidator.validateForCreateCenter`).
    pub fn validate_for_create_center(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(CENTER_SUPPORTED_PARAMS)?;
        let validator = DataValidatorBuilder::for_resource("center");

        let name = command.string_value_of_parameter_named("name");
        let validator = validator
            .parameter("name")
            .value(name.as_deref())
            .not_blank()
            .not_exceeding_length_of(100);

        let external_id = command.string_value_of_parameter_named("externalId");
        let validator = validator
            .parameter("externalId")
            .value(external_id.as_deref())
            .ignore_if_null()
            .not_exceeding_length_of(100);

        let office_id = command.long_value_of_parameter_named("officeId")?;
        let validator = validator
            .parameter("officeId")
            .value(office_id)
            .not_null()
            .integer_greater_than_zero();

        let staff_id = command.long_value_of_parameter_named("staffId")?;
        let validator = validator
            .parameter("staffId")
            .value(staff_id)
            .ignore_if_null()
            .long_greater_than_zero();

        let active = command.boolean_object_value_of_parameter_named("active");
        let validator = if let Some(is_active) = active {
            if is_active {
                let activation_date =
                    command.local_date_value_of_parameter_named("activationDate")?;
                validator
                    .parameter("activationDate")
                    .value(activation_date)
                    .not_null()
            } else {
                validator
            }
        } else {
            validator
        };

        validator.throw_validation_errors()?;
        Ok(())
    }

    /// Assembles a `Group` (`level_id = 2`) from a `JsonCommand`.
    pub fn from_json_for_group(
        id: i64,
        command: &JsonCommand,
        parent_center: Option<&Group>,
        business_date: NaiveDate,
        office_opening_date: Option<NaiveDate>,
        min_clients: Option<usize>,
        max_clients: Option<usize>,
    ) -> FineractResult<Self> {
        Self::validate_for_create_group(command)?;

        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let external_id =
            ExternalId::from_opt(command.string_value_of_parameter_named("externalId"));
        let center_id = command
            .long_value_of_parameter_named("centerId")?
            .or_else(|| parent_center.map(|c| c.id));
        let office_id = match command.long_value_of_parameter_named("officeId")? {
            Some(oid) => oid,
            None => parent_center.map(|c| c.office_id).ok_or_else(|| {
                FineractError::Validation(PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.group.officeId.cannot.be.blank",
                    "The parameter `officeId` cannot be blank.",
                    "officeId",
                ))
            })?,
        };
        let staff_id = command
            .long_value_of_parameter_named("staffId")?
            .or_else(|| parent_center.and_then(|c| c.staff_id));
        let active = command.boolean_primitive_value_of_parameter_named("active");
        let activation_date = command.local_date_value_of_parameter_named("activationDate")?;
        let submitted_on_date = command
            .local_date_value_of_parameter_named("submittedOnDate")?
            .or(if active { activation_date } else { None })
            .unwrap_or(business_date);

        let client_member_ids = Self::extract_id_array(command, "clientMembers")?;
        let parent_hierarchy = parent_center.map(|c| c.hierarchy.as_str());

        Self::new_group(
            id,
            office_id,
            staff_id,
            center_id,
            parent_hierarchy,
            name,
            external_id,
            active,
            activation_date,
            submitted_on_date,
            client_member_ids,
            business_date,
            office_opening_date,
            min_clients,
            max_clients,
        )
    }

    /// Assembles a Center (`level_id = 1`) from a `JsonCommand`.
    pub fn from_json_for_center(
        id: i64,
        command: &JsonCommand,
        business_date: NaiveDate,
        office_opening_date: Option<NaiveDate>,
    ) -> FineractResult<Self> {
        Self::validate_for_create_center(command)?;

        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default();
        let external_id =
            ExternalId::from_opt(command.string_value_of_parameter_named("externalId"));
        let office_id = command
            .long_value_of_parameter_named("officeId")?
            .unwrap_or_default();
        let staff_id = command.long_value_of_parameter_named("staffId")?;
        let active = command.boolean_primitive_value_of_parameter_named("active");
        let activation_date = command.local_date_value_of_parameter_named("activationDate")?;
        let submitted_on_date = command
            .local_date_value_of_parameter_named("submittedOnDate")?
            .or(if active { activation_date } else { None })
            .unwrap_or(business_date);

        Self::new_center(
            id,
            office_id,
            staff_id,
            name,
            external_id,
            active,
            activation_date,
            submitted_on_date,
            business_date,
            office_opening_date,
        )
    }

    pub fn extract_id_array(command: &JsonCommand, param_name: &str) -> FineractResult<Vec<i64>> {
        let Some(val) = command.json_element(param_name) else {
            return Ok(Vec::new());
        };
        if val.is_null() {
            return Ok(Vec::new());
        }
        let Some(arr) = val.as_array() else {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::single_parameter_error(
                    format!("validation.msg.group.{param_name}.must.be.array"),
                    format!("The parameter `{param_name}` must be an array."),
                    param_name,
                ),
            ));
        };
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            if let Some(id) = item.as_i64() {
                out.push(id);
            } else if let Some(s) = item.as_str() {
                let parsed = s.trim().parse::<i64>().map_err(|_| {
                    FineractError::Validation(
                        PlatformApiDataValidationException::single_parameter_error(
                            format!("validation.msg.group.{param_name}.invalid.integer"),
                            format!("Invalid ID `{s}` in `{param_name}`."),
                            param_name,
                        ),
                    )
                })?;
                out.push(parsed);
            }
        }
        Ok(out)
    }

    /// Generates materialized path hierarchy (`Group.generateHierarchy`):
    /// - Root Center or standalone Group: `".{id}."`
    /// - Child Group under Center: `"{parent_hierarchy}{id}."` (e.g. `".1.2."`)
    pub fn generate_hierarchy(&mut self, parent_hierarchy: Option<&str>) {
        if self.id <= 0 {
            return;
        }
        if let Some(parent_h) = parent_hierarchy {
            let trimmed = parent_h.trim();
            if trimmed.is_empty() {
                if let Some(pid) = self.parent_id {
                    self.hierarchy = format!(".{pid}.{}.", self.id);
                } else {
                    self.hierarchy = format!(".{}.", self.id);
                }
            } else if trimmed.ends_with('.') {
                self.hierarchy = format!("{trimmed}{}.", self.id);
            } else {
                self.hierarchy = format!("{trimmed}.{}.", self.id);
            }
        } else if let Some(pid) = self.parent_id {
            self.hierarchy = format!(".{pid}.{}.", self.id);
        } else {
            self.hierarchy = format!(".{}.", self.id);
        }
    }

    /// Parses materialized path `hierarchy` (e.g., `".1.2."`) into ordered ancestor/self IDs.
    pub fn hierarchy_ids(&self) -> Vec<i64> {
        self.hierarchy
            .split('.')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<i64>().ok())
            .collect()
    }

    pub const fn group_type(&self) -> GroupTypes {
        GroupTypes::from_int(self.level_id)
    }

    pub const fn is_center(&self) -> bool {
        self.level_id == GroupLevel::CENTER_LEVEL_ID
    }

    pub const fn is_group(&self) -> bool {
        self.level_id == GroupLevel::GROUP_LEVEL_ID
    }

    pub const fn is_child_group(&self) -> bool {
        self.parent_id.is_some()
    }

    pub const fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub const fn is_pending(&self) -> bool {
        self.status.is_pending()
    }

    pub const fn is_closed(&self) -> bool {
        self.status.is_closed()
    }

    pub const fn is_transfer_in_progress(&self) -> bool {
        self.status.is_transfer_in_progress()
    }

    pub const fn is_transfer_on_hold(&self) -> bool {
        self.status.is_transfer_on_hold()
    }

    pub fn has_client_with_id(&self, client_id: i64) -> bool {
        self.client_member_ids.contains(&client_id)
    }

    fn validate_submitted_on_date(
        &self,
        business_date: NaiveDate,
        office_opening_date: Option<NaiveDate>,
    ) -> FineractResult<()> {
        if self.submitted_on_date > business_date {
            let entity = if self.is_center() { "center" } else { "group" };
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::new(vec![ApiParameterError::parameter_error(
                    format!("error.msg.{entity}.submittedOnDate.in.the.future"),
                    "Submitted on date cannot be in the future.",
                    "submittedOnDate",
                    vec![ApiErrorArg::from_date(self.submitted_on_date)],
                )]),
            ));
        }
        if let Some(opening_date) = office_opening_date {
            if opening_date > self.submitted_on_date {
                let entity = if self.is_center() { "center" } else { "group" };
                return Err(FineractError::Validation(
                    PlatformApiDataValidationException::new(vec![
                        ApiParameterError::parameter_error(
                            format!(
                                "error.msg.{entity}.submittedOnDate.cannot.be.before.office.activation.date"
                            ),
                            "Submitted on date cannot be before the office opening date.",
                            "submittedOnDate",
                            vec![
                                ApiErrorArg::from_date(opening_date),
                                ApiErrorArg::from_date(self.submitted_on_date),
                            ],
                        ),
                    ]),
                ));
            }
        }
        Ok(())
    }

    /// Validates that `client_member_ids.len()` falls within `[min_clients, max_clients]`.
    ///
    /// Matches `GroupMemberCountNotInPermissibleRangeException` (`error.msg.group.members.count.must.be.in.permissible.range`).
    pub fn validate_member_count_range(
        &self,
        min_clients: Option<usize>,
        max_clients: Option<usize>,
    ) -> FineractResult<()> {
        if self.is_center() {
            return Ok(());
        }
        let count = self.client_member_ids.len();
        if let Some(min) = min_clients {
            if count < min {
                return Err(Self::member_count_out_of_range_error(min_clients, max_clients));
            }
        }
        if let Some(max) = max_clients {
            if count > max {
                return Err(Self::member_count_out_of_range_error(min_clients, max_clients));
            }
        }
        Ok(())
    }

    fn member_count_out_of_range_error(
        min_clients: Option<usize>,
        max_clients: Option<usize>,
    ) -> FineractError {
        let min_val = min_clients.unwrap_or(0) as i64;
        let max_val = max_clients.map(|m| m as i64).unwrap_or(i64::MAX);
        PlatformDomainRuleException::with_args(
            "error.msg.group.members.count.must.be.in.permissible.range",
            "The number of clients in the group is not within the permissible range",
            vec![
                ApiErrorArg::new(self_or_zero(min_val)),
                ApiErrorArg::new(max_val),
            ],
        )
        .into()
    }

    /// Activates a pending Group or Center (`Group.activate`).
    pub fn activate(
        &mut self,
        activation_date: NaiveDate,
        business_date: NaiveDate,
        office_opening_date: Option<NaiveDate>,
        min_clients: Option<usize>,
        max_clients: Option<usize>,
    ) -> FineractResult<BTreeMap<String, Value>> {
        let entity = if self.is_center() { "center" } else { "group" };

        if self.is_active() {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::new(vec![ApiParameterError::parameter_error(
                    format!("error.msg.{entity}.already.active"),
                    format!(
                        "Cannot activate {entity}. It is already active with name `{}`.",
                        self.name
                    ),
                    "name",
                    vec![ApiErrorArg::new(self.name.clone())],
                )]),
            ));
        }

        let mut errors = Vec::new();
        if self.submitted_on_date > business_date {
            errors.push(ApiParameterError::parameter_error(
                format!("error.msg.{entity}.submittedOnDate.in.the.future"),
                "Submitted on date cannot be in the future.",
                "submittedOnDate",
                vec![ApiErrorArg::from_date(self.submitted_on_date)],
            ));
        }
        if self.submitted_on_date > activation_date {
            errors.push(ApiParameterError::parameter_error(
                format!("error.msg.{entity}.submittedOnDate.after.activation.date"),
                "Submitted on date cannot be after the activation date.",
                "submittedOnDate",
                vec![
                    ApiErrorArg::from_date(self.submitted_on_date),
                    ApiErrorArg::from_date(activation_date),
                ],
            ));
        }
        if let Some(opening_date) = office_opening_date {
            if opening_date > activation_date {
                errors.push(ApiParameterError::parameter_error(
                    format!(
                        "error.msg.{entity}.activationDate.cannot.be.before.office.activation.date"
                    ),
                    "Activation date cannot be before the office opening date.",
                    "activationDate",
                    vec![
                        ApiErrorArg::from_date(opening_date),
                        ApiErrorArg::from_date(activation_date),
                    ],
                ));
            }
        }
        if activation_date > business_date {
            errors.push(ApiParameterError::parameter_error(
                format!("error.msg.{entity}.activationDate.in.the.future"),
                "Activation date cannot be in the future.",
                "activationDate",
                vec![ApiErrorArg::from_date(activation_date)],
            ));
        }

        if !errors.is_empty() {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::new(errors),
            ));
        }

        self.validate_member_count_range(min_clients, max_clients)?;

        self.status = GroupingTypeStatus::Active;
        self.activation_date = Some(activation_date);

        let mut changes = BTreeMap::new();
        changes.insert("active".to_string(), json!(true));
        changes.insert(
            "status".to_string(),
            json!(GroupingTypeStatus::Active.value()),
        );
        changes.insert(
            "activationDate".to_string(),
            json!(activation_date.format("%Y-%m-%d").to_string()),
        );
        Ok(changes)
    }

    /// Associates a slice of client IDs into this Group (`Group.associateClients`).
    ///
    /// RejectsCenters (`can_have_clients == false`), rejects duplicate clients (`ClientExistInGroupException`),
    /// and validates `max_clients` when provided.
    pub fn associate_clients(
        &mut self,
        client_ids: &[i64],
        max_clients: Option<usize>,
    ) -> FineractResult<Vec<i64>> {
        if self.is_center() {
            return Err(PlatformDomainRuleException::new(
                "error.msg.center.cannot.associate.clients",
                "Centers cannot directly have client members; associate clients with a Group instead.",
            )
            .into());
        }
        if client_ids.is_empty() {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.group.clientMembers.cannot.be.empty",
                    "The parameter `clientMembers` cannot be empty.",
                    "clientMembers",
                ),
            ));
        }

        let mut newly_added = Vec::with_capacity(client_ids.len());
        for &client_id in client_ids {
            if self.client_member_ids.contains(&client_id) || newly_added.contains(&client_id) {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.group.client.exist.in.group",
                    format!(
                        "Client with identifier {client_id} is already exists in Group with identifier {}",
                        self.id
                    ),
                    vec![ApiErrorArg::new(client_id), ApiErrorArg::new(self.id)],
                )
                .into());
            }
            newly_added.push(client_id);
        }

        if let Some(max) = max_clients {
            if self.client_member_ids.len() + newly_added.len() > max {
                return Err(Self::member_count_out_of_range_error(None, Some(max)));
            }
        }

        self.client_member_ids.extend_from_slice(&newly_added);
        Ok(newly_added)
    }

    /// Disassociates a slice of client IDs from this Group (`Group.disassociateClients`).
    ///
    /// Rejects clients not currently in the Group (`ClientNotInGroupException`) and checks `min_clients` if active.
    pub fn disassociate_clients(
        &mut self,
        client_ids: &[i64],
        min_clients: Option<usize>,
    ) -> FineractResult<Vec<i64>> {
        if client_ids.is_empty() {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.group.clientMembers.cannot.be.empty",
                    "The parameter `clientMembers` cannot be empty.",
                    "clientMembers",
                ),
            ));
        }

        for &client_id in client_ids {
            if !self.client_member_ids.contains(&client_id) {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.group.client.not.in.group",
                    format!(
                        "Client with identifier {client_id} is not in Group with identifier {}",
                        self.id
                    ),
                    vec![ApiErrorArg::new(client_id), ApiErrorArg::new(self.id)],
                )
                .into());
            }
        }

        if self.is_active() {
            if let Some(min) = min_clients {
                let remaining = self
                    .client_member_ids
                    .iter()
                    .filter(|cid| !client_ids.contains(cid))
                    .count();
                if remaining < min {
                    return Err(Self::member_count_out_of_range_error(Some(min), None));
                }
            }
        }

        let mut removed = Vec::with_capacity(client_ids.len());
        self.client_member_ids.retain(|cid| {
            if client_ids.contains(cid) {
                removed.push(*cid);
                false
            } else {
                true
            }
        });
        // Also remove any group roles held by disassociated clients
        self.roles.retain(|r| !client_ids.contains(&r.client_id));

        Ok(removed)
    }

    /// Assigns a role (`m_group_roles`) to a client member of this Group (`GroupRole.createGroupRole`).
    pub fn assign_role(
        &mut self,
        role_id: i64,
        client_id: i64,
        role_cv_id: i32,
    ) -> FineractResult<GroupRole> {
        if role_cv_id <= 0 {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.group.role.must.be.greater.than.zero",
                    "The parameter `role` must be greater than zero.",
                    "role",
                ),
            ));
        }
        if !self.has_client_with_id(client_id) {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.group.client.not.in.group",
                format!(
                    "Client with identifier {client_id} is not in Group with identifier {}",
                    self.id
                ),
                vec![ApiErrorArg::new(client_id), ApiErrorArg::new(self.id)],
            )
            .into());
        }

        let role = GroupRole::new(role_id, self.id, client_id, role_cv_id);
        self.roles.push(role.clone());
        Ok(role)
    }

    /// Unassigns a role by `role_id` from this Group.
    pub fn unassign_role(&mut self, role_id: i64) -> FineractResult<GroupRole> {
        if let Some(pos) = self.roles.iter().position(|r| r.id == role_id) {
            Ok(self.roles.remove(pos))
        } else {
            Err(FineractError::resource_not_found(
                "error.msg.group.role.id.invalid",
                format!("Group role with identifier {role_id} does not exist"),
                vec![ApiErrorArg::new(role_id)],
            ))
        }
    }

    /// Closes this Group or Center (`Group.close`).
    pub fn close(
        &mut self,
        closure_date: NaiveDate,
        closure_reason_cv_id: i32,
        business_date: NaiveDate,
    ) -> FineractResult<BTreeMap<String, Value>> {
        let entity = if self.is_center() { "center" } else { "group" };

        if self.is_closed() {
            return Err(PlatformDomainRuleException::with_args(
                format!("error.msg.{entity}.already.closed"),
                format!("Cannot close {entity}. It is already closed."),
                vec![ApiErrorArg::new(self.name.clone())],
            )
            .into());
        }
        if closure_date > business_date {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::new(vec![ApiParameterError::parameter_error(
                    format!("error.msg.{entity}.closureDate.cannot.be.in.the.future"),
                    "Closure date cannot be in the future.",
                    "closureDate",
                    vec![ApiErrorArg::from_date(closure_date)],
                )]),
            ));
        }
        if let Some(act_date) = self.activation_date {
            if closure_date < act_date {
                return Err(FineractError::Validation(
                    PlatformApiDataValidationException::new(vec![
                        ApiParameterError::parameter_error(
                            format!("error.msg.{entity}.closureDate.cannot.be.before.activationDate"),
                            "Closure date cannot be before the activation date.",
                            "closureDate",
                            vec![
                                ApiErrorArg::from_date(act_date),
                                ApiErrorArg::from_date(closure_date),
                            ],
                        ),
                    ]),
                ));
            }
        }

        self.status = GroupingTypeStatus::Closed;
        self.closed_on_date = Some(closure_date);
        self.closure_reason_cv_id = Some(closure_reason_cv_id);

        let mut changes = BTreeMap::new();
        changes.insert(
            "status".to_string(),
            json!(GroupingTypeStatus::Closed.value()),
        );
        changes.insert(
            "closureDate".to_string(),
            json!(closure_date.format("%Y-%m-%d").to_string()),
        );
        changes.insert("closureReasonId".to_string(), json!(closure_reason_cv_id));
        Ok(changes)
    }
}

const fn self_or_zero(val: i64) -> i64 {
    val
}

// =============================================================================
// 3. CALENDAR, CALENDAR ENTITY TYPE & CALENDAR INSTANCE
// =============================================================================

/// Entity type to which a `CalendarInstance` is bound (`m_calendar_instance.entity_type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.calendar.domain.CalendarEntityType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum CalendarEntityType {
    #[default]
    Invalid = 0,
    Clients = 1,
    Groups = 2,
    Loans = 3,
    Centers = 4,
    Savings = 5,
    LoanRecalculationRestDetail = 6,
    LoanRecalculationCompoundingDetail = 7,
}

impl CalendarEntityType {
    pub const INVALID: Self = Self::Invalid;
    pub const CLIENTS: Self = Self::Clients;
    pub const GROUPS: Self = Self::Groups;
    pub const LOANS: Self = Self::Loans;
    pub const CENTERS: Self = Self::Centers;
    pub const SAVINGS: Self = Self::Savings;
    pub const LOAN_RECALCULATION_REST_DETAIL: Self = Self::LoanRecalculationRestDetail;
    pub const LOAN_RECALCULATION_COMPOUNDING_DETAIL: Self =
        Self::LoanRecalculationCompoundingDetail;

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "calendarEntityType.invalid",
            Self::Clients => "calendarEntityType.clients",
            Self::Groups => "calendarEntityType.groups",
            Self::Loans => "calendarEntityType.loans",
            Self::Centers => "calendarEntityType.centers",
            Self::Savings => "calendarEntityType.savings",
            Self::LoanRecalculationRestDetail => {
                "calendarEntityType.loan.recalculation.rest.detail"
            }
            Self::LoanRecalculationCompoundingDetail => {
                "calendarEntityType.loan.recalculation.compounding.detail"
            }
        }
    }

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Clients,
            2 => Self::Groups,
            3 => Self::Loans,
            4 => Self::Centers,
            5 => Self::Savings,
            6 => Self::LoanRecalculationRestDetail,
            7 => Self::LoanRecalculationCompoundingDetail,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: i32) -> Self {
        Self::from_i16(value as i16)
    }

    pub fn from_entity_name(name: &str) -> Self {
        match name.trim().to_ascii_uppercase().as_str() {
            "CLIENTS" | "CLIENT" => Self::Clients,
            "GROUPS" | "GROUP" => Self::Groups,
            "LOANS" | "LOAN" => Self::Loans,
            "CENTERS" | "CENTER" => Self::Centers,
            "SAVINGS" | "SAVING" => Self::Savings,
            _ => Self::Invalid,
        }
    }

    pub const fn is_center(self) -> bool {
        matches!(self, Self::Centers)
    }

    pub const fn is_group(self) -> bool {
        matches!(self, Self::Groups)
    }

    pub const fn is_loan(self) -> bool {
        matches!(self, Self::Loans)
    }

    pub const fn is_savings(self) -> bool {
        matches!(self, Self::Savings)
    }
}

/// Calendar category type (`m_calendar.calendar_type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.calendar.domain.CalendarType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum CalendarType {
    #[default]
    Collection = 1,
    Training = 2,
    Audit = 3,
    General = 4,
}

impl CalendarType {
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Collection),
            2 => Some(Self::Training),
            3 => Some(Self::Audit),
            4 => Some(Self::General),
            _ => None,
        }
    }
}

/// Calendar definition (`m_calendar` table).
///
/// Ported from `org.apache.fineract.portfolio.calendar.domain.Calendar`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub duration: Option<i16>,
    pub calendar_type: CalendarType,
    pub repeating: bool,
    pub recurrence: Option<RecurrenceRule>,
}

impl Calendar {
    pub fn new(
        id: i64,
        title: impl Into<String>,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
        calendar_type: CalendarType,
        repeating: bool,
        recurrence: Option<RecurrenceRule>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: None,
            location: None,
            start_date,
            end_date,
            duration: None,
            calendar_type,
            repeating,
            recurrence,
        }
    }

    pub fn is_valid_recurring_date(
        &self,
        target_date: NaiveDate,
        is_skip_repayment_on_first_month: bool,
        number_of_days: u32,
    ) -> bool {
        if !self.repeating {
            return target_date == self.start_date;
        }
        if let Some(end) = self.end_date {
            if target_date > end {
                return false;
            }
        }
        let Some(ref rule) = self.recurrence else {
            return target_date == self.start_date;
        };
        CalendarUtils::is_valid_recurring_date(
            rule,
            self.start_date,
            target_date,
            is_skip_repayment_on_first_month,
            number_of_days,
        )
    }
}

/// Binding between a `Calendar` (and its `RecurrenceRule`) and a domain entity such as a
/// Center, Group, Loan, or Savings account (`m_calendar_instance` table).
///
/// Ported from `org.apache.fineract.portfolio.calendar.domain.CalendarInstance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarInstance {
    pub id: i64,
    pub calendar_id: i64,
    pub entity_id: i64,
    pub entity_type: CalendarEntityType,
    pub title: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub repeating: bool,
    pub recurrence: Option<RecurrenceRule>,
}

impl CalendarInstance {
    /// Creates a new `CalendarInstance` binding a `RecurrenceRule` to an entity.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        calendar_id: i64,
        entity_id: i64,
        entity_type: CalendarEntityType,
        title: impl Into<String>,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
        repeating: bool,
        recurrence: Option<RecurrenceRule>,
    ) -> Self {
        Self {
            id,
            calendar_id,
            entity_id,
            entity_type,
            title: title.into(),
            start_date,
            end_date,
            repeating,
            recurrence,
        }
    }

    /// Creates a `CalendarInstance` from a `Calendar` and target entity.
    pub fn from_calendar(
        id: i64,
        calendar: &Calendar,
        entity_id: i64,
        entity_type: CalendarEntityType,
    ) -> Self {
        Self {
            id,
            calendar_id: calendar.id,
            entity_id,
            entity_type,
            title: calendar.title.clone(),
            start_date: calendar.start_date,
            end_date: calendar.end_date,
            repeating: calendar.repeating,
            recurrence: calendar.recurrence.clone(),
        }
    }

    /// Checks whether `target_date` is a valid occurrence on this `CalendarInstance`.
    pub fn is_valid_recurring_date(
        &self,
        target_date: NaiveDate,
        is_skip_repayment_on_first_month: bool,
        number_of_days: u32,
    ) -> bool {
        if !self.repeating {
            return target_date == self.start_date;
        }
        if let Some(end) = self.end_date {
            if target_date > end {
                return false;
            }
        }
        let Some(ref rule) = self.recurrence else {
            return target_date == self.start_date;
        };
        CalendarUtils::is_valid_recurring_date(
            rule,
            self.start_date,
            target_date,
            is_skip_repayment_on_first_month,
            number_of_days,
        )
    }

    /// Validates that `target_date` falls on a valid recurring date of this `CalendarInstance`,
    /// returning `NotValidRecurringDateException` (`error.msg.calendar.meeting.not.valid.recurring.date`) if not.
    pub fn validate_recurring_date(&self, target_date: NaiveDate) -> FineractResult<()> {
        if !self.is_valid_recurring_date(target_date, false, 0) {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.calendar.meeting.not.valid.recurring.date",
                format!(
                    "The date `{target_date}` is not a valid meeting date for calendar `{}`.",
                    self.title
                ),
                vec![
                    ApiErrorArg::from_date(target_date),
                    ApiErrorArg::new(self.title.clone()),
                ],
            )
            .into());
        }
        Ok(())
    }
}

// =============================================================================
// 4. ATTENDANCE TYPE, CLIENT ATTENDANCE & MEETING
// =============================================================================

/// Client attendance status at a Center/Group meeting (`m_client_attendance.attendance_type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.meeting.attendance.AttendanceType` / `MeetingAttendanceType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum AttendanceType {
    #[default]
    Invalid = 0,
    Present = 1,
    Absent = 2,
    Approved = 3,
    Leave = 4,
    Late = 5,
}

pub type MeetingAttendanceType = AttendanceType;

impl AttendanceType {
    pub const INVALID: Self = Self::Invalid;
    pub const PRESENT: Self = Self::Present;
    pub const ABSENT: Self = Self::Absent;
    pub const APPROVED: Self = Self::Approved;
    pub const LEAVE: Self = Self::Leave;
    pub const LATE: Self = Self::Late;

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "attendanceType.invalid",
            Self::Present => "attendanceType.present",
            Self::Absent => "attendanceType.absent",
            Self::Approved => "attendanceType.approved",
            Self::Leave => "attendanceType.leave",
            Self::Late => "attendanceType.late",
        }
    }

    pub const fn from_i16(value: i16) -> Self {
        match value {
            1 => Self::Present,
            2 => Self::Absent,
            3 => Self::Approved,
            4 => Self::Leave,
            5 => Self::Late,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i16(v as i16),
            None => Self::Invalid,
        }
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }
}

/// Client attendance record for a specific `Meeting` (`m_client_attendance` table).
///
/// Ported from `org.apache.fineract.portfolio.meeting.attendance.domain.ClientAttendance`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAttendance {
    pub id: i64,
    pub client_id: i64,
    pub meeting_id: i64,
    pub attendance_type: AttendanceType,
}

pub type MeetingAttendance = ClientAttendance;

impl ClientAttendance {
    pub const fn new(
        id: i64,
        client_id: i64,
        meeting_id: i64,
        attendance_type: AttendanceType,
    ) -> Self {
        Self {
            id,
            client_id,
            meeting_id,
            attendance_type,
        }
    }

    pub fn update_attendance_type(&mut self, attendance_type: AttendanceType) {
        self.attendance_type = attendance_type;
    }
}

/// Supported JSON parameters for creating/updating a `Meeting` (`MeetingApiConstants.MEETING_REQUEST_DATA_PARAMETERS`).
pub const MEETING_SUPPORTED_PARAMS: &[&str] = &[
    "locale",
    "dateFormat",
    "meetingDate",
    "calendarId",
    "clientsAttendance",
];

/// Center or Group meeting instance (`m_meeting` table).
///
/// Ported from `org.apache.fineract.portfolio.meeting.domain.Meeting`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: i64,
    pub calendar_instance_id: i64,
    pub meeting_date: NaiveDate,
    pub clients_attendance: Vec<ClientAttendance>,
}

impl Meeting {
    /// Creates a new `Meeting` after validating `meeting_date` against `calendar_instance`
    /// and ensuring `meeting_date` is not in the future relative to `business_date`.
    pub fn new_validated(
        id: i64,
        calendar_instance: &CalendarInstance,
        meeting_date: NaiveDate,
        business_date: NaiveDate,
        is_skip_repayment_on_first_month: bool,
        number_of_days: u32,
        clients_attendance: Vec<ClientAttendance>,
    ) -> FineractResult<Self> {
        Self::validate_meeting_date(
            calendar_instance,
            meeting_date,
            business_date,
            is_skip_repayment_on_first_month,
            number_of_days,
        )?;

        let mut meeting = Self {
            id,
            calendar_instance_id: calendar_instance.id,
            meeting_date,
            clients_attendance: Vec::new(),
        };
        if !clients_attendance.is_empty() {
            meeting.associate_clients_attendance(clients_attendance)?;
        }
        Ok(meeting)
    }

    /// Validates that `meeting_date` matches the `CalendarInstance` recurrence rule and is not in the future.
    ///
    /// Matches `Meeting.createNew` -> `NotValidRecurringDateException` and `MeetingDateException`.
    pub fn validate_meeting_date(
        calendar_instance: &CalendarInstance,
        meeting_date: NaiveDate,
        business_date: NaiveDate,
        is_skip_repayment_on_first_month: bool,
        number_of_days: u32,
    ) -> FineractResult<()> {
        if !calendar_instance.is_valid_recurring_date(
            meeting_date,
            is_skip_repayment_on_first_month,
            number_of_days,
        ) {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.calendar.meeting.not.valid.recurring.date",
                format!(
                    "The date `{meeting_date}` is not a valid meeting date for calendar `{}`.",
                    calendar_instance.title
                ),
                vec![
                    ApiErrorArg::from_date(meeting_date),
                    ApiErrorArg::new(calendar_instance.title.clone()),
                ],
            )
            .into());
        }

        if meeting_date > business_date {
            return Err(PlatformDomainRuleException::with_args(
                "error.msg.meeting.date.cannot.be.a.future.date",
                "Meeting date cannot be a future date.",
                vec![ApiErrorArg::from_date(meeting_date)],
            )
            .into());
        }

        Ok(())
    }

    /// Validates a `JsonCommand` for creating a `Meeting` (`MeetingDataValidator.validateForCreate`).
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(MEETING_SUPPORTED_PARAMS)?;
        let validator = DataValidatorBuilder::for_resource("meeting");

        let meeting_date = command.local_date_value_of_parameter_named("meetingDate")?;
        let validator = validator
            .parameter("meetingDate")
            .value(meeting_date)
            .not_null();

        let calendar_id = command.long_value_of_parameter_named("calendarId")?;
        let validator = validator
            .parameter("calendarId")
            .value(calendar_id)
            .not_null()
            .long_greater_than_zero();

        if let Some(Value::Array(arr)) = command.json_element("clientsAttendance") {
            let mut v = validator;
            for (idx, elem) in arr.iter().enumerate() {
                let cid = elem.get("clientId").and_then(|val| val.as_i64());
                let att = elem.get("attendanceType").and_then(|val| val.as_i64());
                v = v
                    .parameter(&format!("clientsAttendance[{idx}].clientId"))
                    .value(cid)
                    .not_null()
                    .long_greater_than_zero()
                    .parameter(&format!("clientsAttendance[{idx}].attendanceType"))
                    .value(att)
                    .not_null()
                    .in_min_max_range(1, 5);
            }
            v.throw_validation_errors()?;
        } else {
            validator.throw_validation_errors()?;
        }

        Ok(())
    }

    /// Associates client attendance records with this `Meeting`, rejecting duplicate client entries
    /// (`error.msg.attendance.duplicate.client.entry`).
    pub fn associate_clients_attendance(
        &mut self,
        attendances: Vec<ClientAttendance>,
    ) -> FineractResult<()> {
        for mut att in attendances {
            if att.attendance_type.is_invalid() {
                return Err(FineractError::Validation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.meeting.attendanceType.invalid",
                        "Attendance type is invalid.",
                        "attendanceType",
                    ),
                ));
            }
            if self
                .clients_attendance
                .iter()
                .any(|existing| existing.client_id == att.client_id)
            {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.attendance.duplicate.client.entry",
                    format!(
                        "Duplicate attendance entry for client with identifier {}",
                        att.client_id
                    ),
                    vec![ApiErrorArg::new(att.client_id)],
                )
                .into());
            }
            att.meeting_id = self.id;
            self.clients_attendance.push(att);
        }
        Ok(())
    }

    /// Updates or inserts client attendance records on this `Meeting`.
    pub fn update_clients_attendance(&mut self, attendances: Vec<ClientAttendance>) {
        for mut att in attendances {
            if let Some(existing) = self
                .clients_attendance
                .iter_mut()
                .find(|a| a.client_id == att.client_id)
            {
                existing.update_attendance_type(att.attendance_type);
            } else {
                att.meeting_id = self.id;
                self.clients_attendance.push(att);
            }
        }
    }
}

// =============================================================================
// 5. FUND, PAYMENT TYPE, PAYMENT DETAIL & NOTE
// =============================================================================

/// Supported JSON parameters for creating/updating a `Fund` (`FundCommandFromApiJsonDeserializer`).
pub const FUND_SUPPORTED_PARAMS: &[&str] = &["name", "externalId"];

/// Fund source definition (`m_fund` table).
///
/// Ported from `org.apache.fineract.portfolio.fund.domain.Fund`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fund {
    pub id: i64,
    pub name: String,
    pub external_id: ExternalId,
}

impl Fund {
    pub fn new(id: i64, name: impl Into<String>, external_id: ExternalId) -> Self {
        Self {
            id,
            name: name.into(),
            external_id,
        }
    }

    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(FUND_SUPPORTED_PARAMS)?;
        let validator = DataValidatorBuilder::for_resource("fund");

        let name = command.string_value_of_parameter_named("name");
        let validator = validator
            .parameter("name")
            .value(name.as_deref())
            .not_blank()
            .not_exceeding_length_of(100);

        let external_id = command.string_value_of_parameter_named("externalId");
        let validator = validator
            .parameter("externalId")
            .value(external_id.as_deref())
            .ignore_if_null()
            .not_blank()
            .not_exceeding_length_of(100);

        validator.throw_validation_errors()?;
        Ok(())
    }

    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;
        let name = command
            .string_value_of_parameter_named("name")
            .unwrap_or_default()
            .trim()
            .to_string();
        let external_id = ExternalId::from_opt(
            command
                .string_value_of_parameter_named("externalId")
                .map(|s| s.trim().to_string()),
        );
        Ok(Self::new(id, name, external_id))
    }

    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        command.check_for_unsupported_parameters(FUND_SUPPORTED_PARAMS)?;
        let validator = DataValidatorBuilder::for_resource("fund");

        let mut validator = validator;
        if command.parameter_exists("name") {
            let name = command.string_value_of_parameter_named("name");
            validator = validator
                .parameter("name")
                .value(name.as_deref())
                .not_blank()
                .not_exceeding_length_of(100);
        }
        if command.parameter_exists("externalId") {
            let external_id = command.string_value_of_parameter_named("externalId");
            validator = validator
                .parameter("externalId")
                .value(external_id.as_deref())
                .not_blank()
                .not_exceeding_length_of(100);
        }
        validator.throw_validation_errors()?;

        let mut changes = BTreeMap::new();
        if command.is_change_in_string_parameter_named("name", Some(&self.name)) {
            let new_name = command
                .string_value_of_parameter_named("name")
                .unwrap_or_default()
                .trim()
                .to_string();
            changes.insert("name".to_string(), json!(new_name));
            self.name = new_name;
        }
        if command.is_change_in_string_parameter_named("externalId", self.external_id.value()) {
            let new_ext = command
                .string_value_of_parameter_named("externalId")
                .map(|s| s.trim().to_string());
            changes.insert("externalId".to_string(), json!(new_ext));
            self.external_id = ExternalId::from_opt(new_ext);
        }
        Ok(changes)
    }
}

/// Payment channel definition (`m_payment_type` table).
///
/// Ported from `org.apache.fineract.portfolio.paymenttype.domain.PaymentType`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentType {
    pub id: i32,
    pub value: String,
    pub description: Option<String>,
    pub is_cash_payment: bool,
    pub order_position: i32,
    pub code_name: Option<String>,
    pub is_system_defined: bool,
}

impl PaymentType {
    pub fn new(
        id: i32,
        value: impl Into<String>,
        description: Option<String>,
        is_cash_payment: bool,
        order_position: i32,
        code_name: Option<String>,
        is_system_defined: bool,
    ) -> Self {
        Self {
            id,
            value: value.into(),
            description,
            is_cash_payment,
            order_position,
            code_name,
            is_system_defined,
        }
    }
}

/// Supported JSON parameters for `PaymentDetail` (`PaymentDetailConstants`).
pub const PAYMENT_DETAIL_SUPPORTED_PARAMS: &[&str] = &[
    "paymentTypeId",
    "accountNumber",
    "checkNumber",
    "routingCode",
    "receiptNumber",
    "bankNumber",
];

/// Transaction payment instrument details (`m_payment_detail` table).
///
/// Ported from `org.apache.fineract.portfolio.paymentdetail.domain.PaymentDetail`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentDetail {
    pub id: i64,
    pub payment_type_id: i32,
    pub account_number: Option<String>,
    pub check_number: Option<String>,
    pub routing_code: Option<String>,
    pub receipt_number: Option<String>,
    pub bank_number: Option<String>,
}

impl PaymentDetail {
    pub fn new(
        id: i64,
        payment_type_id: i32,
        account_number: Option<String>,
        check_number: Option<String>,
        routing_code: Option<String>,
        receipt_number: Option<String>,
        bank_number: Option<String>,
    ) -> Self {
        Self {
            id,
            payment_type_id,
            account_number,
            check_number,
            routing_code,
            receipt_number,
            bank_number,
        }
    }

    /// Assembles an optional `PaymentDetail` from a `JsonCommand` if `paymentTypeId` is present
    /// (`PaymentDetailAssembler.fetchPaymentDetail`).
    pub fn from_json_opt(id: i64, command: &JsonCommand) -> FineractResult<Option<Self>> {
        let Some(payment_type_id) = command.integer_value_of_parameter_named("paymentTypeId")?
        else {
            return Ok(None);
        };
        if payment_type_id <= 0 {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.paymentdetail.paymentTypeId.must.be.greater.than.zero",
                    "The parameter `paymentTypeId` must be greater than zero.",
                    "paymentTypeId",
                ),
            ));
        }
        Ok(Some(Self {
            id,
            payment_type_id,
            account_number: command.string_value_of_parameter_named("accountNumber"),
            check_number: command.string_value_of_parameter_named("checkNumber"),
            routing_code: command.string_value_of_parameter_named("routingCode"),
            receipt_number: command.string_value_of_parameter_named("receiptNumber"),
            bank_number: command.string_value_of_parameter_named("bankNumber"),
        }))
    }
}

/// Entity target type of a `Note` (`m_note.note_type_enum`).
///
/// Ported from `org.apache.fineract.portfolio.note.domain.NoteType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum NoteType {
    #[default]
    Client = 100,
    Loan = 200,
    LoanTransaction = 300,
    SavingAccount = 500,
    Group = 600,
    Shares = 700,
    SavingsTransaction = 800,
}

impl NoteType {
    pub const CLIENT: Self = Self::Client;
    pub const LOAN: Self = Self::Loan;
    pub const LOAN_TRANSACTION: Self = Self::LoanTransaction;
    pub const SAVING_ACCOUNT: Self = Self::SavingAccount;
    pub const SAVINGS_ACCOUNT: Self = Self::SavingAccount;
    pub const GROUP: Self = Self::Group;
    pub const SHARES: Self = Self::Shares;
    pub const SHARE_ACCOUNT: Self = Self::Shares;
    pub const SAVINGS_TRANSACTION: Self = Self::SavingsTransaction;

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Client => "noteType.client",
            Self::Loan => "noteType.loan",
            Self::LoanTransaction => "noteType.loan.transaction",
            Self::SavingAccount => "noteType.saving.account",
            Self::Group => "noteType.group",
            Self::Shares => "noteType.shares",
            Self::SavingsTransaction => "noteType.savings.transaction",
        }
    }

    pub const fn api_url(self) -> &'static str {
        match self {
            Self::Client => "clients",
            Self::Loan => "loans",
            Self::LoanTransaction => "loanTransactions",
            Self::SavingAccount => "savings",
            Self::Group => "groups",
            Self::Shares => "accounts/share",
            Self::SavingsTransaction => "savingsTransactions",
        }
    }

    pub const fn from_i16(v: i16) -> Option<Self> {
        match v {
            100 => Some(Self::Client),
            200 => Some(Self::Loan),
            300 => Some(Self::LoanTransaction),
            500 => Some(Self::SavingAccount),
            600 => Some(Self::Group),
            700 => Some(Self::Shares),
            800 => Some(Self::SavingsTransaction),
            _ => None,
        }
    }

    pub const fn from_int(v: i32) -> Option<Self> {
        Self::from_i16(v as i16)
    }

    pub fn from_api_url_or_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "clients" | "client" => Some(Self::Client),
            "loans" | "loan" => Some(Self::Loan),
            "loantransactions" | "loantransaction" | "loan_transaction" => {
                Some(Self::LoanTransaction)
            }
            "savings" | "savingaccount" | "savingsaccount" | "saving_account" => {
                Some(Self::SavingAccount)
            }
            "groups" | "group" | "centers" | "center" => Some(Self::Group),
            "accounts/share" | "shares" | "share" | "shareaccount" => Some(Self::Shares),
            "savingstransactions" | "savingstransaction" | "savings_transaction" => {
                Some(Self::SavingsTransaction)
            }
            _ => None,
        }
    }
}

/// Supported JSON parameters for creating/updating a `Note`.
pub const NOTE_SUPPORTED_PARAMS: &[&str] = &["note", "resourceType", "noteTypeId"];

/// Polymorphic text note attached to a Client, Group/Center, Loan, LoanTransaction,
/// SavingsAccount, SavingsTransaction, or ShareAccount (`m_note` table).
///
/// Ported from `org.apache.fineract.portfolio.note.domain.Note`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: i64,
    pub client_id: Option<i64>,
    pub group_id: Option<i64>,
    pub loan_id: Option<i64>,
    pub loan_transaction_id: Option<i64>,
    pub savings_account_id: Option<i64>,
    pub savings_transaction_id: Option<i64>,
    pub share_account_id: Option<i64>,
    pub note_type: NoteType,
    pub note: String,
}

impl Note {
    pub fn for_client(id: i64, client_id: i64, note: impl Into<String>) -> Self {
        Self {
            id,
            client_id: Some(client_id),
            group_id: None,
            loan_id: None,
            loan_transaction_id: None,
            savings_account_id: None,
            savings_transaction_id: None,
            share_account_id: None,
            note_type: NoteType::Client,
            note: note.into(),
        }
    }

    pub fn for_group(id: i64, group_id: i64, note: impl Into<String>) -> Self {
        Self {
            id,
            client_id: None,
            group_id: Some(group_id),
            loan_id: None,
            loan_transaction_id: None,
            savings_account_id: None,
            savings_transaction_id: None,
            share_account_id: None,
            note_type: NoteType::Group,
            note: note.into(),
        }
    }

    pub fn for_loan(id: i64, loan_id: i64, note: impl Into<String>) -> Self {
        Self {
            id,
            client_id: None,
            group_id: None,
            loan_id: Some(loan_id),
            loan_transaction_id: None,
            savings_account_id: None,
            savings_transaction_id: None,
            share_account_id: None,
            note_type: NoteType::Loan,
            note: note.into(),
        }
    }

    pub fn for_loan_transaction(
        id: i64,
        loan_id: Option<i64>,
        loan_transaction_id: i64,
        note: impl Into<String>,
    ) -> Self {
        Self {
            id,
            client_id: None,
            group_id: None,
            loan_id,
            loan_transaction_id: Some(loan_transaction_id),
            savings_account_id: None,
            savings_transaction_id: None,
            share_account_id: None,
            note_type: NoteType::LoanTransaction,
            note: note.into(),
        }
    }

    pub fn for_saving_account(id: i64, savings_account_id: i64, note: impl Into<String>) -> Self {
        Self {
            id,
            client_id: None,
            group_id: None,
            loan_id: None,
            loan_transaction_id: None,
            savings_account_id: Some(savings_account_id),
            savings_transaction_id: None,
            share_account_id: None,
            note_type: NoteType::SavingAccount,
            note: note.into(),
        }
    }

    pub fn for_share_account(id: i64, share_account_id: i64, note: impl Into<String>) -> Self {
        Self {
            id,
            client_id: None,
            group_id: None,
            loan_id: None,
            loan_transaction_id: None,
            savings_account_id: None,
            savings_transaction_id: None,
            share_account_id: Some(share_account_id),
            note_type: NoteType::Shares,
            note: note.into(),
        }
    }

    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(NOTE_SUPPORTED_PARAMS)?;
        let note = command.string_value_of_parameter_named("note");
        DataValidatorBuilder::for_resource("note")
            .parameter("note")
            .value(note.as_deref())
            .not_blank()
            .not_exceeding_length_of(1000)
            .throw_validation_errors()?;
        Ok(())
    }

    pub fn from_entity_and_type(
        id: i64,
        note_type: NoteType,
        resource_id: i64,
        note_text: impl Into<String>,
    ) -> Self {
        match note_type {
            NoteType::Client => Self::for_client(id, resource_id, note_text),
            NoteType::Group => Self::for_group(id, resource_id, note_text),
            NoteType::Loan => Self::for_loan(id, resource_id, note_text),
            NoteType::LoanTransaction => {
                Self::for_loan_transaction(id, None, resource_id, note_text)
            }
            NoteType::SavingAccount => Self::for_saving_account(id, resource_id, note_text),
            NoteType::Shares => Self::for_share_account(id, resource_id, note_text),
            NoteType::SavingsTransaction => Self {
                id,
                client_id: None,
                group_id: None,
                loan_id: None,
                loan_transaction_id: None,
                savings_account_id: None,
                savings_transaction_id: Some(resource_id),
                share_account_id: None,
                note_type: NoteType::SavingsTransaction,
                note: note_text.into(),
            },
        }
    }
}

// =============================================================================
// 6. DUAL-DB GROUP AND PORTFOLIO REPOSITORY
// =============================================================================

/// Dual-database (`Postgres` / `MySql`) and in-memory repository for Groups, Centers,
/// Group Roles, Calendar Instances, Meetings, Client Attendance, Funds, Payment Types/Details, and Notes.
#[derive(Debug)]
pub struct DualDbGroupAndPortfolioRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    next_id: AtomicI64,
    groups: DashMap<i64, Group>,
    calendar_instances: DashMap<i64, CalendarInstance>,
    meetings: DashMap<i64, Meeting>,
    funds: DashMap<i64, Fund>,
    payment_types: DashMap<i32, PaymentType>,
    payment_details: DashMap<i64, PaymentDetail>,
    notes: DashMap<i64, Note>,
    min_clients: Option<usize>,
    max_clients: Option<usize>,
}

impl Default for DualDbGroupAndPortfolioRepository {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl DualDbGroupAndPortfolioRepository {
    pub const GROUP_COLUMNS: &'static str = "id, account_no, external_id, status_enum, \
        activation_date, office_id, staff_id, parent_id, level_id, display_name, hierarchy, \
        closure_reason_cv_id, closedon_date, submittedon_date";

    pub const GROUP_ROLE_COLUMNS: &'static str = "id, client_id, group_id, role_cv_id";

    pub const CALENDAR_INSTANCE_COLUMNS: &'static str =
        "ci.id, ci.calendar_id, ci.entity_id, ci.entity_type_enum, \
         c.title, c.start_date, c.end_date, c.repeating, c.recurrence";

    pub const MEETING_COLUMNS: &'static str = "id, calendar_instance_id, meeting_date";

    pub const CLIENT_ATTENDANCE_COLUMNS: &'static str =
        "id, client_id, meeting_id, attendance_type_enum";

    pub const FUND_COLUMNS: &'static str = "id, name, external_id";

    pub const PAYMENT_TYPE_COLUMNS: &'static str =
        "id, value, description, is_cash_payment, order_position, code_name, is_system_defined";

    pub const PAYMENT_DETAIL_COLUMNS: &'static str =
        "id, payment_type_id, account_number, check_number, routing_code, receipt_number, bank_number";

    pub const NOTE_COLUMNS: &'static str =
        "id, client_id, group_id, loan_id, loan_transaction_id, savings_account_id, \
         savings_account_transaction_id, share_account_id, note_type_enum, note";

    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            next_id: AtomicI64::new(1),
            groups: DashMap::new(),
            calendar_instances: DashMap::new(),
            meetings: DashMap::new(),
            funds: DashMap::new(),
            payment_types: DashMap::new(),
            payment_details: DashMap::new(),
            notes: DashMap::new(),
            min_clients: None,
            max_clients: None,
        }
    }

    pub fn in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    pub fn new_in_memory() -> Self {
        Self::in_memory()
    }

    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            pool: None,
            dialect,
            next_id: AtomicI64::new(1),
            groups: DashMap::new(),
            calendar_instances: DashMap::new(),
            meetings: DashMap::new(),
            funds: DashMap::new(),
            payment_types: DashMap::new(),
            payment_details: DashMap::new(),
            notes: DashMap::new(),
            min_clients: None,
            max_clients: None,
        }
    }

    pub fn with_client_count_bounds(
        mut self,
        min_clients: Option<usize>,
        max_clients: Option<usize>,
    ) -> Self {
        self.min_clients = min_clients;
        self.max_clients = max_clients;
        self
    }

    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub const fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    pub const fn min_clients(&self) -> Option<usize> {
        self.min_clients
    }

    pub const fn max_clients(&self) -> Option<usize> {
        self.max_clients
    }

    pub fn allocate_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    // -------------------------------------------------------------------------
    // Dual-dialect SQL Builders
    // -------------------------------------------------------------------------

    pub fn select_group_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            &format!("SELECT {} FROM m_group WHERE id = $1", Self::GROUP_COLUMNS),
            dialect,
        )
    }

    pub fn insert_group_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_group (id, account_no, external_id, status_enum, activation_date, \
             office_id, staff_id, parent_id, level_id, display_name, hierarchy, \
             closure_reason_cv_id, closedon_date, submittedon_date) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
            dialect,
        )
    }

    pub fn update_group_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "UPDATE m_group SET status_enum = $1, activation_date = $2, hierarchy = $3, \
             display_name = $4, staff_id = $5, closure_reason_cv_id = $6, closedon_date = $7 \
             WHERE id = $8",
            dialect,
        )
    }

    pub fn insert_group_client_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_group_client (group_id, client_id) VALUES ($1, $2)",
            dialect,
        )
    }

    pub fn delete_group_client_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "DELETE FROM m_group_client WHERE group_id = $1 AND client_id = $2",
            dialect,
        )
    }

    pub fn insert_group_role_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_group_roles (id, client_id, group_id, role_cv_id) VALUES ($1, $2, $3, $4)",
            dialect,
        )
    }

    pub fn select_calendar_instance_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            &format!(
                "SELECT {} FROM m_calendar_instance ci \
                 JOIN m_calendar c ON c.id = ci.calendar_id WHERE ci.id = $1",
                Self::CALENDAR_INSTANCE_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_calendar_instance_by_calendar_and_entity_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            &format!(
                "SELECT {} FROM m_calendar_instance ci \
                 JOIN m_calendar c ON c.id = ci.calendar_id \
                 WHERE ci.calendar_id = $1 AND ci.entity_id = $2 AND ci.entity_type_enum = $3",
                Self::CALENDAR_INSTANCE_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_meeting_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_meeting (id, calendar_instance_id, meeting_date) VALUES ($1, $2, $3)",
            dialect,
        )
    }

    pub fn insert_client_attendance_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_client_attendance (id, client_id, meeting_id, attendance_type_enum) \
             VALUES ($1, $2, $3, $4)",
            dialect,
        )
    }

    pub fn select_fund_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            &format!("SELECT {} FROM m_fund WHERE id = $1", Self::FUND_COLUMNS),
            dialect,
        )
    }

    pub fn insert_fund_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_fund (id, name, external_id) VALUES ($1, $2, $3)",
            dialect,
        )
    }

    pub fn insert_payment_type_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_payment_type (id, value, description, is_cash_payment, order_position, code_name, is_system_defined) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            dialect,
        )
    }

    pub fn insert_payment_detail_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_payment_detail (id, payment_type_id, account_number, check_number, routing_code, receipt_number, bank_number) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            dialect,
        )
    }

    pub fn insert_note_sql(dialect: SqlDialect) -> String {
        DialectSql::bind_rewrite(
            "INSERT INTO m_note (id, client_id, group_id, loan_id, loan_transaction_id, \
             savings_account_id, savings_account_transaction_id, share_account_id, note_type_enum, note) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            dialect,
        )
    }

    // -------------------------------------------------------------------------
    // Group & Center Operations
    // -------------------------------------------------------------------------

    pub fn upsert_group_in_memory(&self, mut group: Group) -> Group {
        if group.id <= 0 {
            group.id = self.allocate_id();
        } else {
            let _ = self.next_id.fetch_max(group.id + 1, Ordering::SeqCst);
        }
        if group.account_no.is_none() {
            group.account_no = Some(format!("{:09}", group.id));
        }
        if group.hierarchy.is_empty() {
            let parent_hierarchy = group
                .parent_id
                .and_then(|pid| self.groups.get(&pid).map(|p| p.hierarchy.clone()));
            group.generate_hierarchy(parent_hierarchy.as_deref());
        }
        self.groups.insert(group.id, group.clone());
        group
    }

    pub fn get_group_in_memory(&self, id: i64) -> Option<Group> {
        self.groups.get(&id).map(|g| g.clone())
    }

    pub async fn save_group(&self, mut group: Group) -> FineractResult<Group> {
        if group.id <= 0 {
            group.id = self.allocate_id();
        } else {
            let _ = self.next_id.fetch_max(group.id + 1, Ordering::SeqCst);
        }
        if group.account_no.is_none() {
            group.account_no = Some(format!("{:09}", group.id));
        }
        if group.hierarchy.is_empty() {
            let parent_hierarchy = group
                .parent_id
                .and_then(|pid| self.groups.get(&pid).map(|p| p.hierarchy.clone()));
            group.generate_hierarchy(parent_hierarchy.as_deref());
        }

        // Check duplicate name within same office and level
        for existing in self.groups.iter() {
            if existing.id != group.id
                && existing.office_id == group.office_id
                && existing.level_id == group.level_id
                && existing.name.eq_ignore_ascii_case(&group.name)
            {
                let entity = if group.is_center() { "center" } else { "group" };
                return Err(PlatformDomainRuleException::with_args(
                    format!("error.msg.{entity}.duplicate.name"),
                    format!(
                        "{} with name `{}` already exists.",
                        if group.is_center() { "Center" } else { "Group" },
                        group.name
                    ),
                    vec![ApiErrorArg::new(group.name.clone())],
                )
                .into());
            }
            if existing.id != group.id {
                if let (Some(ext_a), Some(ext_b)) =
                    (existing.external_id.value(), group.external_id.value())
                {
                    if ext_a == ext_b {
                        let entity = if group.is_center() { "center" } else { "group" };
                        return Err(PlatformDomainRuleException::with_args(
                            format!("error.msg.{entity}.duplicate.externalId"),
                            format!(
                                "{} with externalId `{ext_b}` already exists.",
                                if group.is_center() { "Center" } else { "Group" }
                            ),
                            vec![ApiErrorArg::new(ext_b.to_string())],
                        )
                        .into());
                    }
                }
            }
        }

        if let Some(pool) = &self.pool {
            let sql = Self::insert_group_sql(self.dialect);
            sqlx::query(&sql)
                .bind(group.id)
                .bind(group.account_no.as_deref())
                .bind(group.external_id.value())
                .bind(group.status.value())
                .bind(group.activation_date.map(|d| d.to_string()))
                .bind(group.office_id)
                .bind(group.staff_id)
                .bind(group.parent_id)
                .bind(group.level_id)
                .bind(&group.name)
                .bind(&group.hierarchy)
                .bind(group.closure_reason_cv_id)
                .bind(group.closed_on_date.map(|d| d.to_string()))
                .bind(group.submitted_on_date.to_string())
                .execute(pool)
                .await
                .map_err(|e| {
                    PlatformDomainRuleException::new(
                        "error.msg.group.database.error",
                        format!("Failed to persist group {}: {e}", group.id),
                    )
                })?;

            let member_sql = Self::insert_group_client_sql(self.dialect);
            for &client_id in &group.client_member_ids {
                sqlx::query(&member_sql)
                    .bind(group.id)
                    .bind(client_id)
                    .execute(pool)
                    .await
                    .map_err(|e| {
                        PlatformDomainRuleException::new(
                            "error.msg.group.client.database.error",
                            format!("Failed to persist group client association: {e}"),
                        )
                    })?;
            }
        }

        self.groups.insert(group.id, group.clone());
        Ok(group)
    }

    pub async fn update_group(&self, group: Group) -> FineractResult<Group> {
        if let Some(pool) = &self.pool {
            let sql = Self::update_group_sql(self.dialect);
            sqlx::query(&sql)
                .bind(group.status.value())
                .bind(group.activation_date.map(|d| d.to_string()))
                .bind(&group.hierarchy)
                .bind(&group.name)
                .bind(group.staff_id)
                .bind(group.closure_reason_cv_id)
                .bind(group.closed_on_date.map(|d| d.to_string()))
                .bind(group.id)
                .execute(pool)
                .await
                .map_err(|e| {
                    PlatformDomainRuleException::new(
                        "error.msg.group.database.error",
                        format!("Failed to update group {}: {e}", group.id),
                    )
                })?;
        }
        self.groups.insert(group.id, group.clone());
        Ok(group)
    }

    pub async fn find_group_by_id(&self, id: i64) -> FineractResult<Option<Group>> {
        if let Some(g) = self.groups.get(&id) {
            return Ok(Some(g.clone()));
        }
        if let Some(pool) = &self.pool {
            let sql = Self::select_group_by_id_sql(self.dialect);
            let row_opt = sqlx::query(&sql).bind(id).fetch_optional(pool).await.map_err(|e| {
                PlatformDomainRuleException::new(
                    "error.msg.group.database.error",
                    format!("Failed to query group {id}: {e}"),
                )
            })?;
            if let Some(row) = row_opt {
                let status_val: i32 = row.try_get("status_enum").unwrap_or(0);
                let act_str: Option<String> = row.try_get("activation_date").ok().flatten();
                let closed_str: Option<String> = row.try_get("closedon_date").ok().flatten();
                let sub_str: String = row
                    .try_get("submittedon_date")
                    .unwrap_or_else(|_| "1970-01-01".to_string());
                let group = Group {
                    id: row.try_get("id").unwrap_or(id),
                    account_no: row.try_get("account_no").ok().flatten(),
                    external_id: ExternalId::from_opt(row.try_get("external_id").ok().flatten()),
                    status: GroupingTypeStatus::from_i32(status_val),
                    activation_date: act_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    office_id: row.try_get("office_id").unwrap_or(1),
                    staff_id: row.try_get("staff_id").ok().flatten(),
                    parent_id: row.try_get("parent_id").ok().flatten(),
                    level_id: row.try_get("level_id").unwrap_or(2),
                    name: row.try_get("display_name").unwrap_or_default(),
                    hierarchy: row.try_get("hierarchy").unwrap_or_default(),
                    closure_reason_cv_id: row.try_get("closure_reason_cv_id").ok().flatten(),
                    closed_on_date: closed_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    submitted_on_date: NaiveDate::parse_from_str(&sub_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    client_member_ids: Vec::new(),
                    roles: Vec::new(),
                };
                self.groups.insert(group.id, group.clone());
                return Ok(Some(group));
            }
        }
        Ok(None)
    }

    pub async fn get_group_by_id(&self, id: i64) -> FineractResult<Group> {
        self.find_group_by_id(id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.group.id.invalid",
                format!("Group with identifier {id} does not exist"),
                vec![ApiErrorArg::new(id)],
            )
        })
    }

    pub async fn get_center_by_id(&self, id: i64) -> FineractResult<Group> {
        let group = self.find_group_by_id(id).await?.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.center.id.invalid",
                format!("Center with identifier {id} does not exist"),
                vec![ApiErrorArg::new(id)],
            )
        })?;
        if !group.is_center() {
            return Err(FineractError::resource_not_found(
                "error.msg.center.id.invalid",
                format!("Center with identifier {id} does not exist"),
                vec![ApiErrorArg::new(id)],
            ));
        }
        Ok(group)
    }

    pub fn list_child_groups_of_center(&self, center_id: i64) -> Vec<Group> {
        let mut children: Vec<Group> = self
            .groups
            .iter()
            .filter(|g| g.parent_id == Some(center_id) && g.is_group())
            .map(|g| g.clone())
            .collect();
        children.sort_by_key(|g| g.id);
        children
    }

    // -------------------------------------------------------------------------
    // CalendarInstance & Meeting Operations
    // -------------------------------------------------------------------------

    pub fn upsert_calendar_instance_in_memory(
        &self,
        mut instance: CalendarInstance,
    ) -> CalendarInstance {
        if instance.id <= 0 {
            instance.id = self.allocate_id();
        } else {
            let _ = self.next_id.fetch_max(instance.id + 1, Ordering::SeqCst);
        }
        self.calendar_instances
            .insert(instance.id, instance.clone());
        instance
    }

    pub fn find_calendar_instance_by_id(&self, id: i64) -> Option<CalendarInstance> {
        self.calendar_instances.get(&id).map(|ci| ci.clone())
    }

    pub fn find_calendar_instance_by_calendar_and_entity(
        &self,
        calendar_id: i64,
        entity_id: i64,
        entity_type: CalendarEntityType,
    ) -> Option<CalendarInstance> {
        self.calendar_instances
            .iter()
            .find(|ci| {
                ci.calendar_id == calendar_id
                    && ci.entity_id == entity_id
                    && ci.entity_type == entity_type
            })
            .map(|ci| ci.clone())
            .or_else(|| {
                // Fallback: match by calendar_id and entity_id if entity_type was not specified on lookup
                self.calendar_instances
                    .iter()
                    .find(|ci| ci.calendar_id == calendar_id && ci.entity_id == entity_id)
                    .map(|ci| ci.clone())
            })
            .or_else(|| {
                // Fallback: match by calendar_id directly
                self.calendar_instances
                    .iter()
                    .find(|ci| ci.calendar_id == calendar_id || ci.id == calendar_id)
                    .map(|ci| ci.clone())
            })
    }

    pub async fn save_meeting(&self, mut meeting: Meeting) -> FineractResult<Meeting> {
        if meeting.id <= 0 {
            meeting.id = self.allocate_id();
        } else {
            let _ = self.next_id.fetch_max(meeting.id + 1, Ordering::SeqCst);
        }

        // Check duplicate meeting for same calendar_instance_id and meeting_date
        for existing in self.meetings.iter() {
            if existing.id != meeting.id
                && existing.calendar_instance_id == meeting.calendar_instance_id
                && existing.meeting_date == meeting.meeting_date
            {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.meeting.duplicate",
                    format!(
                        "A meeting on date `{}` already exists for calendar instance `{}`.",
                        meeting.meeting_date, meeting.calendar_instance_id
                    ),
                    vec![
                        ApiErrorArg::new(meeting.calendar_instance_id),
                        ApiErrorArg::from_date(meeting.meeting_date),
                    ],
                )
                .into());
            }
        }

        for att in &mut meeting.clients_attendance {
            if att.id <= 0 {
                att.id = self.allocate_id();
            }
            att.meeting_id = meeting.id;
        }

        if let Some(pool) = &self.pool {
            let sql = Self::insert_meeting_sql(self.dialect);
            sqlx::query(&sql)
                .bind(meeting.id)
                .bind(meeting.calendar_instance_id)
                .bind(meeting.meeting_date.to_string())
                .execute(pool)
                .await
                .map_err(|e| {
                    PlatformDomainRuleException::new(
                        "error.msg.meeting.database.error",
                        format!("Failed to persist meeting {}: {e}", meeting.id),
                    )
                })?;

            let att_sql = Self::insert_client_attendance_sql(self.dialect);
            for att in &meeting.clients_attendance {
                sqlx::query(&att_sql)
                    .bind(att.id)
                    .bind(att.client_id)
                    .bind(att.meeting_id)
                    .bind(att.attendance_type.as_i16())
                    .execute(pool)
                    .await
                    .map_err(|e| {
                        PlatformDomainRuleException::new(
                            "error.msg.meeting.attendance.database.error",
                            format!("Failed to persist client attendance: {e}"),
                        )
                    })?;
            }
        }

        self.meetings.insert(meeting.id, meeting.clone());
        Ok(meeting)
    }

    pub fn get_meeting_in_memory(&self, id: i64) -> Option<Meeting> {
        self.meetings.get(&id).map(|m| m.clone())
    }

    // -------------------------------------------------------------------------
    // Fund, PaymentType, PaymentDetail & Note Operations
    // -------------------------------------------------------------------------

    pub async fn save_fund(&self, mut fund: Fund) -> FineractResult<Fund> {
        if fund.id <= 0 {
            fund.id = self.allocate_id();
        } else {
            let _ = self.next_id.fetch_max(fund.id + 1, Ordering::SeqCst);
        }

        for existing in self.funds.iter() {
            if existing.id != fund.id && existing.name.eq_ignore_ascii_case(&fund.name) {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.fund.duplicate.name",
                    format!("Fund with name `{}` already exists.", fund.name),
                    vec![ApiErrorArg::new(fund.name.clone())],
                )
                .into());
            }
            if existing.id != fund.id {
                if let (Some(a), Some(b)) = (existing.external_id.value(), fund.external_id.value())
                {
                    if a == b {
                        return Err(PlatformDomainRuleException::with_args(
                            "error.msg.fund.duplicate.externalId",
                            format!("Fund with externalId `{b}` already exists."),
                            vec![ApiErrorArg::new(b.to_string())],
                        )
                        .into());
                    }
                }
            }
        }

        if let Some(pool) = &self.pool {
            let sql = Self::insert_fund_sql(self.dialect);
            sqlx::query(&sql)
                .bind(fund.id)
                .bind(&fund.name)
                .bind(fund.external_id.value())
                .execute(pool)
                .await
                .map_err(|e| {
                    PlatformDomainRuleException::new(
                        "error.msg.fund.database.error",
                        format!("Failed to persist fund {}: {e}", fund.id),
                    )
                })?;
        }

        self.funds.insert(fund.id, fund.clone());
        Ok(fund)
    }

    pub fn get_fund_in_memory(&self, id: i64) -> Option<Fund> {
        self.funds.get(&id).map(|f| f.clone())
    }

    pub fn upsert_payment_type_in_memory(&self, payment_type: PaymentType) -> PaymentType {
        self.payment_types
            .insert(payment_type.id, payment_type.clone());
        payment_type
    }

    pub fn get_payment_type_in_memory(&self, id: i32) -> Option<PaymentType> {
        self.payment_types.get(&id).map(|pt| pt.clone())
    }

    pub async fn save_payment_detail(
        &self,
        mut detail: PaymentDetail,
    ) -> FineractResult<PaymentDetail> {
        if detail.id <= 0 {
            detail.id = self.allocate_id();
        } else {
            let _ = self.next_id.fetch_max(detail.id + 1, Ordering::SeqCst);
        }

        if let Some(pool) = &self.pool {
            let sql = Self::insert_payment_detail_sql(self.dialect);
            sqlx::query(&sql)
                .bind(detail.id)
                .bind(detail.payment_type_id)
                .bind(detail.account_number.as_deref())
                .bind(detail.check_number.as_deref())
                .bind(detail.routing_code.as_deref())
                .bind(detail.receipt_number.as_deref())
                .bind(detail.bank_number.as_deref())
                .execute(pool)
                .await
                .map_err(|e| {
                    PlatformDomainRuleException::new(
                        "error.msg.paymentdetail.database.error",
                        format!("Failed to persist payment detail {}: {e}", detail.id),
                    )
                })?;
        }

        self.payment_details.insert(detail.id, detail.clone());
        Ok(detail)
    }

    pub fn get_payment_detail_in_memory(&self, id: i64) -> Option<PaymentDetail> {
        self.payment_details.get(&id).map(|pd| pd.clone())
    }

    pub async fn save_note(&self, mut note: Note) -> FineractResult<Note> {
        if note.id <= 0 {
            note.id = self.allocate_id();
        } else {
            let _ = self.next_id.fetch_max(note.id + 1, Ordering::SeqCst);
        }

        if let Some(pool) = &self.pool {
            let sql = Self::insert_note_sql(self.dialect);
            sqlx::query(&sql)
                .bind(note.id)
                .bind(note.client_id)
                .bind(note.group_id)
                .bind(note.loan_id)
                .bind(note.loan_transaction_id)
                .bind(note.savings_account_id)
                .bind(note.savings_transaction_id)
                .bind(note.share_account_id)
                .bind(note.note_type.as_i16())
                .bind(&note.note)
                .execute(pool)
                .await
                .map_err(|e| {
                    PlatformDomainRuleException::new(
                        "error.msg.note.database.error",
                        format!("Failed to persist note {}: {e}", note.id),
                    )
                })?;
        }

        self.notes.insert(note.id, note.clone());
        Ok(note)
    }

    pub fn get_note_in_memory(&self, id: i64) -> Option<Note> {
        self.notes.get(&id).map(|n| n.clone())
    }

    pub fn list_notes_for_entity(&self, note_type: NoteType, entity_id: i64) -> Vec<Note> {
        let mut result: Vec<Note> = self
            .notes
            .iter()
            .filter(|n| {
                n.note_type == note_type
                    && match note_type {
                        NoteType::Client => n.client_id == Some(entity_id),
                        NoteType::Group => n.group_id == Some(entity_id),
                        NoteType::Loan => n.loan_id == Some(entity_id),
                        NoteType::LoanTransaction => n.loan_transaction_id == Some(entity_id),
                        NoteType::SavingAccount => n.savings_account_id == Some(entity_id),
                        NoteType::Shares => n.share_account_id == Some(entity_id),
                        NoteType::SavingsTransaction => n.savings_transaction_id == Some(entity_id),
                    }
            })
            .map(|n| n.clone())
            .collect();
        result.sort_by_key(|n| n.id);
        result
    }
}

// =============================================================================
// 7. COMMAND HANDLERS
// =============================================================================

/// Command handler for `CREATE_GROUP` (`CreateGroupCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct CreateGroupCommandHandler {
    repo: Arc<DualDbGroupAndPortfolioRepository>,
}

impl CreateGroupCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "GROUP";

    pub fn new(repo: Arc<DualDbGroupAndPortfolioRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGroupAndPortfolioRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGroupAndPortfolioRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateGroupCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let center_id = command
            .long_value_of_parameter_named("centerId")?
            .or(command.entity_id);
        let parent_center = if let Some(cid) = center_id {
            Some(self.repo.get_center_by_id(cid).await?)
        } else {
            None
        };

        let id = self.repo.allocate_id();
        let group = Group::from_json_for_group(
            id,
            command,
            parent_center.as_ref(),
            ctx.business_date(),
            None,
            self.repo.min_clients(),
            self.repo.max_clients(),
        )?;

        let saved = self.repo.save_group(group).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_group_id(saved.id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `CREATE_CENTER` (`CreateCenterCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct CreateCenterCommandHandler {
    repo: Arc<DualDbGroupAndPortfolioRepository>,
}

impl CreateCenterCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "CENTER";

    pub fn new(repo: Arc<DualDbGroupAndPortfolioRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGroupAndPortfolioRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGroupAndPortfolioRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateCenterCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let id = self.repo.allocate_id();
        let center = Group::from_json_for_center(id, command, ctx.business_date(), None)?;
        let saved = self.repo.save_group(center).await?;

        // If `groupMembers` was supplied when creating the center, link those groups to this center
        let group_member_ids = Group::extract_id_array(command, "groupMembers")?;
        for gid in group_member_ids {
            let mut child_group = self.repo.get_group_by_id(gid).await?;
            if child_group.parent_id == Some(saved.id) {
                return Err(PlatformDomainRuleException::with_args(
                    "error.msg.center.group.exists.in.center",
                    format!(
                        "Group with identifier {gid} already exists in Center with identifier {}",
                        saved.id
                    ),
                    vec![ApiErrorArg::new(gid), ApiErrorArg::new(saved.id)],
                )
                .into());
            }
            child_group.parent_id = Some(saved.id);
            child_group.office_id = saved.office_id;
            child_group.generate_hierarchy(Some(&saved.hierarchy));
            self.repo.update_group(child_group).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_group_id(saved.id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `ACTIVATE_GROUP` and `ACTIVATE_CENTER` (`ActivateGroupCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct ActivateGroupCommandHandler {
    repo: Arc<DualDbGroupAndPortfolioRepository>,
}

impl ActivateGroupCommandHandler {
    pub const ACTION_NAME: &'static str = "ACTIVATE";
    pub const ENTITY_NAME: &'static str = "GROUP";

    pub fn new(repo: Arc<DualDbGroupAndPortfolioRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGroupAndPortfolioRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGroupAndPortfolioRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for ActivateGroupCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(GROUP_ACTIVATION_SUPPORTED_PARAMS)?;
        let group_id = command.entity_id.ok_or_else(|| {
            FineractError::Validation(PlatformApiDataValidationException::single_parameter_error(
                "validation.msg.group.id.cannot.be.blank",
                "The group `entity_id` is required for activation.",
                "groupId",
            ))
        })?;

        let activation_date = command.local_date_value_of_parameter_named("activationDate")?;
        DataValidatorBuilder::for_resource("group")
            .parameter("activationDate")
            .value(activation_date)
            .not_null()
            .throw_validation_errors()?;
        let activation_date = activation_date.unwrap();

        let mut group = self.repo.get_group_by_id(group_id).await?;
        let changes = group.activate(
            activation_date,
            ctx.business_date(),
            None,
            self.repo.min_clients(),
            self.repo.max_clients(),
        )?;
        let saved = self.repo.update_group(group).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_group_id(saved.id)
            .with_entity_id(saved.id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `ASSOCIATECLIENTS_GROUP` (`AssociateClientsToGroupCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct AssociateClientsToGroupCommandHandler {
    repo: Arc<DualDbGroupAndPortfolioRepository>,
}

impl AssociateClientsToGroupCommandHandler {
    pub const ACTION_NAME: &'static str = "ASSOCIATECLIENTS";
    pub const ENTITY_NAME: &'static str = "GROUP";

    pub fn new(repo: Arc<DualDbGroupAndPortfolioRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGroupAndPortfolioRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGroupAndPortfolioRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for AssociateClientsToGroupCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        command.check_for_unsupported_parameters(GROUP_CLIENT_ASSOCIATION_SUPPORTED_PARAMS)?;
        let group_id = command.entity_id.ok_or_else(|| {
            FineractError::Validation(PlatformApiDataValidationException::single_parameter_error(
                "validation.msg.group.id.cannot.be.blank",
                "The group `entity_id` is required for associating clients.",
                "groupId",
            ))
        })?;

        let client_member_ids = Group::extract_id_array(command, "clientMembers")?;
        if client_member_ids.is_empty() {
            return Err(FineractError::Validation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.group.clientMembers.cannot.be.empty",
                    "The parameter `clientMembers` cannot be empty.",
                    "clientMembers",
                ),
            ));
        }

        let mut group = self.repo.get_group_by_id(group_id).await?;
        let added = group.associate_clients(&client_member_ids, self.repo.max_clients())?;
        let saved = self.repo.update_group(group).await?;

        let mut changes = BTreeMap::new();
        changes.insert("clientMembers".to_string(), json!(added));

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_group_id(saved.id)
            .with_entity_id(saved.id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `CREATE_MEETING` (`MeetingCreateCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct CreateMeetingCommandHandler {
    repo: Arc<DualDbGroupAndPortfolioRepository>,
}

pub type MeetingCreateCommandHandler = CreateMeetingCommandHandler;

impl CreateMeetingCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "MEETING";

    pub fn new(repo: Arc<DualDbGroupAndPortfolioRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGroupAndPortfolioRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGroupAndPortfolioRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateMeetingCommandHandler {
    async fn handle(
        &self,
        ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        Meeting::validate_for_create(command)?;

        let calendar_id = command
            .long_value_of_parameter_named("calendarId")?
            .unwrap();
        let meeting_date = command
            .local_date_value_of_parameter_named("meetingDate")?
            .unwrap();

        let entity_id = command.entity_id.unwrap_or(0);
        let calendar_instance = self
            .repo
            .find_calendar_instance_by_calendar_and_entity(
                calendar_id,
                entity_id,
                CalendarEntityType::Invalid,
            )
            .ok_or_else(|| {
                FineractError::resource_not_found(
                    "error.msg.calendar.instance.not.found",
                    format!("Calendar instance for calendar {calendar_id} does not exist"),
                    vec![ApiErrorArg::new(calendar_id)],
                )
            })?;

        let meeting_id = self.repo.allocate_id();
        let mut attendances = Vec::new();
        if let Some(Value::Array(arr)) = command.json_element("clientsAttendance") {
            for elem in arr {
                let cid = elem
                    .get("clientId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or_default();
                let att_val = elem
                    .get("attendanceType")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i16;
                let att_id = self.repo.allocate_id();
                attendances.push(ClientAttendance::new(
                    att_id,
                    cid,
                    meeting_id,
                    AttendanceType::from_i16(att_val),
                ));
            }
        }

        let meeting = Meeting::new_validated(
            meeting_id,
            &calendar_instance,
            meeting_date,
            ctx.business_date(),
            false,
            0,
            attendances,
        )?;
        let saved = self.repo.save_meeting(meeting).await?;

        let group_id = if matches!(
            calendar_instance.entity_type,
            CalendarEntityType::Groups | CalendarEntityType::Centers
        ) {
            Some(calendar_instance.entity_id)
        } else {
            None
        };

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_group_id_opt(group_id)
            .with_entity_id(saved.id)
            .build())
    }
}

/// Command handler for `CREATE_FUND` (`CreateFundCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct CreateFundCommandHandler {
    repo: Arc<DualDbGroupAndPortfolioRepository>,
}

impl CreateFundCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "FUND";

    pub fn new(repo: Arc<DualDbGroupAndPortfolioRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGroupAndPortfolioRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGroupAndPortfolioRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateFundCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let id = self.repo.allocate_id();
        let fund = Fund::from_json(id, command)?;
        let saved = self.repo.save_fund(fund).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `CREATE_NOTE` (`NoteCreateCommandHandler.java`).
#[derive(Debug, Clone)]
pub struct CreateNoteCommandHandler {
    repo: Arc<DualDbGroupAndPortfolioRepository>,
}

pub type NoteCreateCommandHandler = CreateNoteCommandHandler;

impl CreateNoteCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "NOTE";

    pub fn new(repo: Arc<DualDbGroupAndPortfolioRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbGroupAndPortfolioRepository::in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbGroupAndPortfolioRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateNoteCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        Note::validate_for_create(command)?;
        let note_text = command
            .string_value_of_parameter_named("note")
            .unwrap_or_default();

        // Determine NoteType from command fields or explicit `resourceType` / `noteTypeId`
        let note_type = if let Some(rt) = command.string_value_of_parameter_named("resourceType") {
            NoteType::from_api_url_or_name(&rt).ok_or_else(|| {
                FineractError::Validation(PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.note.resourceType.invalid",
                    format!("Unsupported note resourceType `{rt}`."),
                    "resourceType",
                ))
            })?
        } else if let Some(type_id) = command.integer_value_of_parameter_named("noteTypeId")? {
            NoteType::from_int(type_id).ok_or_else(|| {
                FineractError::Validation(PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.note.noteTypeId.invalid",
                    format!("Unsupported noteTypeId `{type_id}`."),
                    "noteTypeId",
                ))
            })?
        } else if command.client_id.is_some() {
            NoteType::Client
        } else if command.loan_id.is_some() && command.subresource_id.is_some() {
            NoteType::LoanTransaction
        } else if command.loan_id.is_some() {
            NoteType::Loan
        } else if command.savings_id.is_some() {
            NoteType::SavingAccount
        } else {
            NoteType::Group
        };

        let resource_id = command
            .entity_id
            .or(command.client_id)
            .or(command.loan_id)
            .or(command.savings_id)
            .ok_or_else(|| {
                FineractError::Validation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.note.resourceId.cannot.be.blank",
                        "Target entity resource ID is required for creating a note.",
                        "resourceId",
                    ),
                )
            })?;

        let id = self.repo.allocate_id();
        let note = Note::from_entity_and_type(id, note_type, resource_id, note_text);
        let saved = self.repo.save_note(note).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_client_id_opt(saved.client_id)
            .with_group_id_opt(saved.group_id)
            .with_loan_id_opt(saved.loan_id)
            .with_savings_id_opt(saved.savings_account_id)
            .with_entity_id(saved.id)
            .build())
    }
}

// =============================================================================
// 8. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use fineract_core_rs::{CalendarFrequencyType, CalendarWeekDaysType};

    fn test_ctx() -> ExecutionContext {
        ExecutionContext::new("default", NaiveDate::from_ymd_opt(2025, 6, 15).unwrap())
    }

    #[test]
    fn grouping_type_status_and_group_level_mappings_match_fineract() {
        assert_eq!(GroupingTypeStatus::from_i32(0), GroupingTypeStatus::Invalid);
        assert_eq!(
            GroupingTypeStatus::from_i32(100),
            GroupingTypeStatus::Pending
        );
        assert_eq!(
            GroupingTypeStatus::from_i32(300),
            GroupingTypeStatus::Active
        );
        assert_eq!(
            GroupingTypeStatus::from_i32(303),
            GroupingTypeStatus::TransferInProgress
        );
        assert_eq!(
            GroupingTypeStatus::from_i32(304),
            GroupingTypeStatus::TransferOnHold
        );
        assert_eq!(
            GroupingTypeStatus::from_i32(600),
            GroupingTypeStatus::Closed
        );
        assert!(GroupingTypeStatus::TransferInProgress.is_under_transfer());
        assert!(GroupingTypeStatus::TransferOnHold.is_under_transfer());

        let center_lvl = GroupLevel::center();
        assert_eq!(center_lvl.id, 1);
        assert!(center_lvl.super_parent);
        assert!(!center_lvl.can_have_clients);

        let group_lvl = GroupLevel::group();
        assert_eq!(group_lvl.id, 2);
        assert_eq!(group_lvl.parent_id, Some(1));
        assert!(group_lvl.can_have_clients);
    }

    #[tokio::test]
    async fn center_and_group_hierarchy_activation_and_member_bounds_work() {
        let ctx = test_ctx();
        let repo = Arc::new(
            DualDbGroupAndPortfolioRepository::in_memory()
                .with_client_count_bounds(Some(2), Some(4)),
        );
        let center_handler = CreateCenterCommandHandler::new(repo.clone());
        let group_handler = CreateGroupCommandHandler::new(repo.clone());
        let activate_handler = ActivateGroupCommandHandler::new(repo.clone());
        let associate_handler = AssociateClientsToGroupCommandHandler::new(repo.clone());

        // 1. Create active Center
        let center_cmd = JsonCommand::from_json(
            r#"{
                "name": "Sunrise Center",
                "officeId": 1,
                "active": true,
                "activationDate": "2025-06-01",
                "submittedOnDate": "2025-06-01",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap();
        let center_res = center_handler.handle(&ctx, &center_cmd).await.unwrap();
        let center_id = center_res.resource_id.unwrap();
        let center = repo.get_center_by_id(center_id).await.unwrap();
        assert!(center.is_center());
        assert!(center.is_active());
        assert_eq!(center.hierarchy, format!(".{center_id}."));

        // 2. Create pending Group under Center with 1 client (below min_clients = 2)
        let group_cmd = JsonCommand::from_json(format!(
            r#"{{
                "name": "Alpha Solidarity Group",
                "centerId": {center_id},
                "active": false,
                "submittedOnDate": "2025-06-02",
                "clientMembers": [101],
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }}"#
        ))
        .unwrap();
        let group_res = group_handler.handle(&ctx, &group_cmd).await.unwrap();
        let group_id = group_res.resource_id.unwrap();
        let group = repo.get_group_by_id(group_id).await.unwrap();
        assert!(group.is_group());
        assert!(group.is_pending());
        assert_eq!(group.hierarchy, format!(".{center_id}.{group_id}."));
        assert_eq!(group.hierarchy_ids(), vec![center_id, group_id]);

        // 3. Activating with only 1 client should fail min_clients = 2 check
        let act_cmd = JsonCommand::from_json(
            r#"{
                "activationDate": "2025-06-10",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_entity_id(group_id);
        let err = activate_handler.handle(&ctx, &act_cmd).await.unwrap_err();
        assert!(err
            .to_string()
            .contains("error.msg.group.members.count.must.be.in.permissible.range"));

        // 4. Associate second client (102), then activate succeeds
        let assoc_cmd = JsonCommand::from_json(r#"{"clientMembers": [102]}"#)
            .unwrap()
            .with_entity_id(group_id);
        associate_handler.handle(&ctx, &assoc_cmd).await.unwrap();

        activate_handler.handle(&ctx, &act_cmd).await.unwrap();
        let mut active_group = repo.get_group_by_id(group_id).await.unwrap();
        assert!(active_group.is_active());
        assert_eq!(active_group.client_member_ids, vec![101, 102]);

        // 5. Duplicate client association fails
        let dup_err = active_group.associate_clients(&[102], Some(4)).unwrap_err();
        assert!(dup_err
            .to_string()
            .contains("error.msg.group.client.exist.in.group"));

        // 6. Role assignment requires client membership
        let role = active_group.assign_role(1, 101, 10).unwrap();
        assert_eq!(role.client_id, 101);
        assert_eq!(role.role_cv_id, 10);
        let non_member_role_err = active_group.assign_role(2, 999, 10).unwrap_err();
        assert!(non_member_role_err
            .to_string()
            .contains("error.msg.group.client.not.in.group"));
    }

    #[tokio::test]
    async fn meeting_validates_calendar_instance_recurrence_and_attendance() {
        let ctx = test_ctx(); // business_date = 2025-06-15
        let repo = Arc::new(DualDbGroupAndPortfolioRepository::in_memory());

        // Weekly every Monday starting 2025-06-02 (Mondays: June 2, 9, 16)
        let rule = RecurrenceRule::new(CalendarFrequencyType::Weekly, 1)
            .with_by_day(vec![CalendarWeekDaysType::MO]);
        let cal_instance = CalendarInstance::new(
            10,
            50,
            1,
            CalendarEntityType::Centers,
            "Weekly Monday Center Meeting",
            NaiveDate::from_ymd_opt(2025, 6, 2).unwrap(),
            None,
            true,
            Some(rule),
        );
        repo.upsert_calendar_instance_in_memory(cal_instance);

        let meeting_handler = CreateMeetingCommandHandler::new(repo.clone());

        // Valid Monday meeting on 2025-06-09 with attendance
        let valid_cmd = JsonCommand::from_json(
            r#"{
                "calendarId": 50,
                "meetingDate": "2025-06-09",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en",
                "clientsAttendance": [
                    {"clientId": 101, "attendanceType": 1},
                    {"clientId": 102, "attendanceType": 5}
                ]
            }"#,
        )
        .unwrap()
        .with_entity_id(1);
        let res = meeting_handler.handle(&ctx, &valid_cmd).await.unwrap();
        let meeting = repo
            .get_meeting_in_memory(res.resource_id.unwrap())
            .unwrap();
        assert_eq!(meeting.clients_attendance.len(), 2);
        assert_eq!(
            meeting.clients_attendance[0].attendance_type,
            AttendanceType::Present
        );
        assert_eq!(
            meeting.clients_attendance[1].attendance_type,
            AttendanceType::Late
        );

        // Non-Monday date (2025-06-10 Tuesday) fails recurrence validation
        let invalid_day_cmd = JsonCommand::from_json(
            r#"{
                "calendarId": 50,
                "meetingDate": "2025-06-10",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_entity_id(1);
        let err = meeting_handler
            .handle(&ctx, &invalid_day_cmd)
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("error.msg.calendar.meeting.not.valid.recurring.date"));

        // Future Monday (2025-06-16 > business_date 2025-06-15) fails future check
        let future_cmd = JsonCommand::from_json(
            r#"{
                "calendarId": 50,
                "meetingDate": "2025-06-16",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en"
            }"#,
        )
        .unwrap()
        .with_entity_id(1);
        let future_err = meeting_handler.handle(&ctx, &future_cmd).await.unwrap_err();
        assert!(future_err
            .to_string()
            .contains("error.msg.meeting.date.cannot.be.a.future.date"));
    }

    #[tokio::test]
    async fn fund_payment_detail_and_note_handlers_work() {
        let ctx = test_ctx();
        let repo = Arc::new(DualDbGroupAndPortfolioRepository::in_memory());
        let fund_handler = CreateFundCommandHandler::new(repo.clone());
        let note_handler = CreateNoteCommandHandler::new(repo.clone());

        // Create Fund
        let fund_cmd =
            JsonCommand::from_json(r#"{"name": "Microfinance Revolving Fund", "externalId": "FUND-01"}"#)
                .unwrap();
        let fund_res = fund_handler.handle(&ctx, &fund_cmd).await.unwrap();
        let fund = repo
            .get_fund_in_memory(fund_res.resource_id.unwrap())
            .unwrap();
        assert_eq!(fund.name, "Microfinance Revolving Fund");
        assert_eq!(fund.external_id.value(), Some("FUND-01"));

        // Duplicate Fund name fails
        let dup_fund_cmd =
            JsonCommand::from_json(r#"{"name": "microfinance revolving fund"}"#).unwrap();
        assert!(fund_handler.handle(&ctx, &dup_fund_cmd).await.is_err());

        // PaymentType & PaymentDetail assembly
        let pt = PaymentType::new(
            1,
            "Wire Transfer",
            Some("ACH / Wire".to_string()),
            false,
            1,
            Some("WIRE".to_string()),
            false,
        );
        repo.upsert_payment_type_in_memory(pt);

        let pd_cmd = JsonCommand::from_json(
            r#"{
                "paymentTypeId": 1,
                "accountNumber": "ACC-9988",
                "checkNumber": "CHK-100",
                "routingCode": "RT-01",
                "receiptNumber": "RCPT-77",
                "bankNumber": "BNK-55",
                "locale": "en"
            }"#,
        )
        .unwrap();
        let detail = PaymentDetail::from_json_opt(1, &pd_cmd).unwrap().unwrap();
        let saved_detail = repo.save_payment_detail(detail).await.unwrap();
        assert_eq!(saved_detail.account_number.as_deref(), Some("ACC-9988"));
        assert_eq!(saved_detail.receipt_number.as_deref(), Some("RCPT-77"));

        // Create Note for Group
        let note_cmd = JsonCommand::from_json(
            r#"{"note": "Center leader verified attendance ledger.", "resourceType": "groups"}"#,
        )
        .unwrap()
        .with_entity_id(42);
        let note_res = note_handler.handle(&ctx, &note_cmd).await.unwrap();
        assert_eq!(note_res.group_id, Some(42));
        let notes = repo.list_notes_for_entity(NoteType::Group, 42);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].note, "Center leader verified attendance ledger.");
    }
}
