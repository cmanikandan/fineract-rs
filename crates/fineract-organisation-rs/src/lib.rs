//! `fineract-organisation-rs` — Office Hierarchy, Staff, Working Days, Holidays, and Loan Provisioning Criteria.
//!
//! Ported from:
//! - `org.apache.fineract.organisation.office.domain.Office`
//! - `org.apache.fineract.organisation.office.data.{OfficeData, OfficeTransactionData}`
//! - `org.apache.fineract.organisation.office.serialization.OfficeCommandFromApiJsonDeserializer`
//! - `org.apache.fineract.organisation.office.service.{OfficeReadPlatformServiceImpl, OfficeWritePlatformServiceJpaRepositoryImpl}`
//! - `org.apache.fineract.organisation.office.handler.{CreateOfficeCommandHandler, UpdateOfficeCommandHandler}`
//! - `org.apache.fineract.organisation.staff.domain.{Staff, StaffOrganisationalRoleType}`
//! - `org.apache.fineract.organisation.staff.data.StaffData`
//! - `org.apache.fineract.organisation.staff.service.{StaffReadServiceImpl, StaffWriteServiceImpl}`
//! - `org.apache.fineract.organisation.staff.handler.{StaffCreateCommandHandler, StaffUpdateCommandHandler}`
//! - `org.apache.fineract.organisation.provisioning.domain.{ProvisioningCategory, ProvisioningCriteria, ProvisioningCriteriaDefinition}`
//! - `org.apache.fineract.organisation.provisioning.service.{ProvisioningCriteriaAssembler, ProvisioningCriteriaWritePlatformServiceJpaRepositoryImpl}`
//! - `org.apache.fineract.organisation.provisioning.serialization.ProvisioningCriteriaDefinitionJsonDeserializer`
//! - `org.apache.fineract.organisation.provisioning.handler.CreateProvisioningCriteriaRequestCommandHandler`

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{AnyPool, Row};

use fineract_command_rs::{
    CommandHandler, CommandProcessingResult, CommandProcessingResultBuilder,
};
use fineract_core_rs::{
    DialectSql, ExecutionContext, ExternalId, FineractNumberParser, JsonCommand, SqlDialect,
};
use fineract_validation_rs::{
    DataValidatorBuilder, FineractError, FineractResult, PlatformApiDataValidationException,
    PlatformDomainRuleException,
};

// =============================================================================
// 1. OFFICE DOMAIN & DATA (`m_office`)
// =============================================================================

/// Supported JSON parameters for Office create and update commands
/// (`OfficeCommandFromApiJsonDeserializer.SUPPORTED_PARAMETERS`).
pub const OFFICE_SUPPORTED_PARAMETERS: &[&str] = &[
    "name",
    "parentId",
    "openingDate",
    "externalId",
    "locale",
    "dateFormat",
];

/// Domain aggregate representing an organisational branch or head office (`m_office`).
///
/// Ported from `org.apache.fineract.organisation.office.domain.Office`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Office {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub hierarchy: String,
    pub external_id: ExternalId,
    pub name: String,
    pub opening_date: NaiveDate,
}

/// Immutable DTO representation of an [`Office`] (`m_office`).
///
/// Ported from `org.apache.fineract.organisation.office.data.OfficeData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficeData {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub hierarchy: String,
    pub external_id: ExternalId,
    pub name: String,
    pub opening_date: NaiveDate,
}

impl Office {
    /// Creates a new [`Office`] and computes its initial materialized hierarchy path.
    pub fn new(
        id: i64,
        parent_id: Option<i64>,
        name: impl Into<String>,
        opening_date: NaiveDate,
        external_id: ExternalId,
    ) -> Self {
        let trimmed_name = name.into().trim().to_string();
        let mut office = Self {
            id,
            parent_id,
            hierarchy: String::new(),
            external_id,
            name: trimmed_name,
            opening_date,
        };
        office.generate_hierarchy(None);
        office
    }

    /// Creates a root Head Office (`parent_id = None`), matching `Office.headOffice(...)`.
    pub fn head_office(
        id: i64,
        name: impl Into<String>,
        opening_date: NaiveDate,
        external_id: ExternalId,
    ) -> Self {
        Self::new(id, None, name, opening_date, external_id)
    }

    /// Validates a `CREATE OFFICE` command payload matching `OfficeCommandFromApiJsonDeserializer.validateForCreate`.
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(OFFICE_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("office");

        let name = command.string_value_of_parameter_named("name");
        validator
            .reset()
            .parameter("name")
            .value(name)
            .not_blank()
            .not_exceeding_length_of(100);

        let opening_date = command.local_date_value_of_parameter_named("openingDate")?;
        validator
            .reset()
            .parameter("openingDate")
            .value(opening_date)
            .not_null();

        if command.parameter_exists("externalId") {
            let external_id = command.string_value_of_parameter_named("externalId");
            validator
                .reset()
                .parameter("externalId")
                .value(external_id)
                .not_exceeding_length_of(100);
        }

        if command.parameter_exists("parentId") {
            let parent_id = command.long_value_of_parameter_named("parentId")?;
            validator
                .reset()
                .parameter("parentId")
                .value(parent_id)
                .not_null()
                .integer_greater_than_zero();
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates an `UPDATE OFFICE` command payload matching `OfficeCommandFromApiJsonDeserializer.validateForUpdate`.
    pub fn validate_for_update(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(OFFICE_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("office");

        if command.parameter_exists("name") {
            let name = command.string_value_of_parameter_named("name");
            validator
                .reset()
                .parameter("name")
                .value(name)
                .not_blank()
                .not_exceeding_length_of(100);
        }

        if command.parameter_exists("openingDate") {
            let opening_date = command.local_date_value_of_parameter_named("openingDate")?;
            validator
                .reset()
                .parameter("openingDate")
                .value(opening_date)
                .not_null();
        }

        if command.parameter_exists("externalId") {
            let external_id = command.string_value_of_parameter_named("externalId");
            validator
                .reset()
                .parameter("externalId")
                .value(external_id)
                .not_exceeding_length_of(100);
        }

        if command.parameter_exists("parentId") {
            let parent_id = command.long_value_of_parameter_named("parentId")?;
            validator
                .reset()
                .parameter("parentId")
                .value(parent_id)
                .not_null()
                .integer_greater_than_zero();
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs an [`Office`] from a [`JsonCommand`] and optional parent [`Office`],
    /// matching `Office.fromJson(parentOffice, command)`.
    pub fn from_json(id: i64, parent: Option<&Office>, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;
        let name = command
            .string_value_of_parameter_named("name")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let opening_date = command
            .local_date_value_of_parameter_named("openingDate")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.office.openingDate.cannot.be.blank",
                        "The parameter `openingDate` is mandatory.",
                        "openingDate",
                        Vec::new(),
                    ),
                )
            })?;
        let external_id = ExternalId::from_opt(command.string_value_of_parameter_named("externalId"));
        let parent_id = match parent {
            Some(p) => Some(p.id),
            None => command.long_value_of_parameter_named("parentId")?,
        };

        let mut office = Self {
            id,
            parent_id,
            hierarchy: String::new(),
            external_id,
            name,
            opening_date,
        };
        office.generate_hierarchy(parent.map(|p| p.hierarchy.as_str()));
        Ok(office)
    }

    /// Generates Fineract's materialized hierarchy path (e.g., `".1."`, `".1.2."`, `".1.2.5."`).
    ///
    /// Ported from `Office.generateHierarchy()` and `Office.hierarchyOf(id)`.
    pub fn generate_hierarchy(&mut self, parent_hierarchy: Option<&str>) {
        match parent_hierarchy {
            Some(raw_parent) => {
                let trimmed = raw_parent.trim();
                if trimmed.is_empty() || trimmed == "." {
                    if let Some(pid) = self.parent_id {
                        if pid > 0 && pid != self.id && self.id > 0 {
                            self.hierarchy = format!(".{pid}.{}.", self.id);
                            return;
                        }
                    }
                    if self.id > 0 {
                        self.hierarchy = format!(".{}.", self.id);
                    } else {
                        self.hierarchy = ".".to_string();
                    }
                } else {
                    let mut prefix = if trimmed.starts_with('.') {
                        trimmed.to_string()
                    } else {
                        format!(".{trimmed}")
                    };
                    if !prefix.ends_with('.') {
                        prefix.push('.');
                    }
                    if self.id > 0 {
                        let self_seg = format!(".{}.", self.id);
                        if prefix.ends_with(&self_seg) {
                            self.hierarchy = prefix;
                        } else {
                            self.hierarchy = format!("{prefix}{}.", self.id);
                        }
                    } else {
                        self.hierarchy = prefix;
                    }
                }
            }
            None => {
                if let Some(pid) = self.parent_id {
                    if pid > 0 && pid != self.id && self.id > 0 {
                        self.hierarchy = format!(".{pid}.{}.", self.id);
                        return;
                    }
                }
                if self.id > 0 {
                    self.hierarchy = format!(".{}.", self.id);
                } else {
                    self.hierarchy = ".".to_string();
                }
            }
        }
    }

    /// Returns `true` if `self.id == id`, matching `Office.identifiedBy(id)`.
    pub const fn identified_by(&self, id: i64) -> bool {
        self.id == id
    }

    /// Returns the parsed office IDs along this office's materialized hierarchy path.
    pub fn hierarchy_ids(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self
            .hierarchy
            .split('.')
            .filter(|seg| !seg.trim().is_empty())
            .filter_map(|seg| seg.trim().parse::<i64>().ok())
            .collect();
        if self.id > 0 && !ids.contains(&self.id) {
            ids.push(self.id);
        }
        ids
    }

    /// Returns `true` if `office_id` appears in `self`'s hierarchy or equals `self.id`,
    /// matching `Office.hasAnOfficeInHierarchyWithId(officeId)`.
    pub fn has_an_office_in_hierarchy_with_id(&self, office_id: i64) -> bool {
        if self.id == office_id {
            return true;
        }
        self.hierarchy
            .split('.')
            .filter(|seg| !seg.trim().is_empty())
            .any(|seg| seg.trim().parse::<i64>().ok() == Some(office_id))
    }

    /// Returns `true` if `office_id` is NOT in `self`'s hierarchy,
    /// matching `Office.doesNotHaveAnOfficeInHierarchyWithId(officeId)`.
    pub fn does_not_have_an_office_in_hierarchy_with_id(&self, office_id: i64) -> bool {
        !self.has_an_office_in_hierarchy_with_id(office_id)
    }

    /// Returns `true` if `self` is the direct parent or materialized-path ancestor of `other`.
    pub fn is_parent_of(&self, other: &Office) -> bool {
        if self.id == other.id {
            return false;
        }
        if other.parent_id == Some(self.id) {
            return true;
        }
        let self_path = if self.hierarchy.is_empty() || self.hierarchy == "." {
            if self.id > 0 {
                format!(".{}.", self.id)
            } else {
                String::new()
            }
        } else {
            self.hierarchy.clone()
        };
        !self_path.is_empty()
            && other.hierarchy != self_path
            && other.hierarchy.starts_with(&self_path)
    }

    /// Returns `true` if `parent` is a parent/ancestor of `self`, matching `Office.hasParentOf(office)`.
    pub fn has_parent_of(&self, parent: &Office) -> bool {
        parent.is_parent_of(self)
    }

    /// Returns `true` if assigning `candidate_parent` as the parent of `self` would introduce a cycle
    /// (i.e. `candidate_parent` is `self` or a descendant of `self`).
    pub fn would_create_cycle(&self, candidate_parent: &Office) -> bool {
        self.id == candidate_parent.id
            || self.is_parent_of(candidate_parent)
            || !candidate_parent.does_not_have_an_office_in_hierarchy_with_id(self.id)
    }

    /// Validates and updates the parent of `self` to `new_parent`, preventing self-parenting,
    /// descendant cycle formation, and root-office parent reassignment.
    ///
    /// Ported from `Office.update(final Office newParent)`.
    pub fn update_parent(&mut self, new_parent: &Office) -> FineractResult<()> {
        if self.identified_by(new_parent.id) {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.office.parentId.same.as.id",
                    "Cannot update office with parent same as self.",
                    vec![self.id.into(), new_parent.id.into()],
                ),
            ));
        }

        if self.would_create_cycle(new_parent) {
            return Err(FineractError::DomainRule(
                PlatformDomainRuleException::with_args(
                    "error.msg.office.parentId.cannot.be.descendant",
                    "Cannot update office with a descendant office as parent.",
                    vec![self.id.into(), new_parent.id.into()],
                ),
            ));
        }

        if self.parent_id.is_none() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.office.cannot.update.parent.office.of.root.office",
                "The root office must not be set with a parent office.",
            )));
        }

        self.parent_id = Some(new_parent.id);
        self.generate_hierarchy(Some(&new_parent.hierarchy));
        Ok(())
    }

    /// Alias for [`Office::update_parent`].
    pub fn set_parent(&mut self, new_parent: &Office) -> FineractResult<()> {
        self.update_parent(new_parent)
    }

    /// Applies a [`JsonCommand`] update to `self`, returning the map of changed attributes
    /// matching `Office.update(final JsonCommand command)`.
    pub fn update(
        &mut self,
        command: &JsonCommand,
        new_parent: Option<&Office>,
    ) -> FineractResult<BTreeMap<String, Value>> {
        Self::validate_for_update(command)?;
        let mut actual_changes = BTreeMap::new();

        if command.parameter_exists("parentId") && self.parent_id.is_none() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.office.cannot.update.parent.office.of.root.office",
                "The root office must not be set with a parent office.",
            )));
        }

        if self.parent_id.is_some()
            && command.is_change_in_long_parameter_named("parentId", self.parent_id)?
        {
            if let Some(new_parent_id) = command.long_value_of_parameter_named("parentId")? {
                if new_parent_id == self.id {
                    return Err(FineractError::DomainRule(
                        PlatformDomainRuleException::with_args(
                            "error.msg.office.parentId.same.as.id",
                            "Cannot update office with parent same as self.",
                            vec![self.id.into(), new_parent_id.into()],
                        ),
                    ));
                }
                if let Some(parent_ref) = new_parent {
                    self.update_parent(parent_ref)?;
                } else {
                    self.parent_id = Some(new_parent_id);
                    self.generate_hierarchy(None);
                }
                actual_changes.insert("parentId".to_string(), Value::from(new_parent_id));
            }
        }

        if command.is_change_in_local_date_parameter_named("openingDate", Some(self.opening_date))?
        {
            if let Some(raw_val) = command.string_value_of_parameter_named("openingDate") {
                actual_changes.insert("openingDate".to_string(), Value::String(raw_val));
            }
            let date_format = command.date_format();
            if !date_format.is_empty() {
                actual_changes.insert("dateFormat".to_string(), Value::String(date_format));
            }
            let locale = command.locale();
            if !locale.is_empty() {
                actual_changes.insert("locale".to_string(), Value::String(locale));
            }
            if let Some(parsed_date) = command.local_date_value_of_parameter_named("openingDate")? {
                self.opening_date = parsed_date;
            }
        }

        if command.is_change_in_string_parameter_named("name", Some(&self.name)) {
            let new_name = command
                .string_value_of_parameter_named("name")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            actual_changes.insert("name".to_string(), Value::String(new_name.clone()));
            self.name = new_name;
        }

        if command.is_change_in_string_parameter_named("externalId", self.external_id.value()) {
            let new_ext = command.string_value_of_parameter_named("externalId");
            actual_changes.insert(
                "externalId".to_string(),
                new_ext
                    .clone()
                    .map(Value::String)
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            self.external_id = ExternalId::from_opt(new_ext);
        }

        Ok(actual_changes)
    }

    /// Returns `true` if `self.opening_date < base_date`, matching `Office.isOpeningDateBefore`.
    pub fn is_opening_date_before(&self, base_date: NaiveDate) -> bool {
        self.opening_date < base_date
    }

    /// Returns `true` if `self.opening_date > base_date`, matching `Office.isOpeningDateAfter`.
    pub fn is_opening_date_after(&self, base_date: NaiveDate) -> bool {
        self.opening_date > base_date
    }

    /// Converts this [`Office`] aggregate into an [`OfficeData`] DTO.
    pub fn to_data(&self) -> OfficeData {
        OfficeData::from(self)
    }
}

impl OfficeData {
    /// Creates a new [`OfficeData`] instance.
    pub fn new(
        id: i64,
        parent_id: Option<i64>,
        hierarchy: impl Into<String>,
        external_id: ExternalId,
        name: impl Into<String>,
        opening_date: NaiveDate,
    ) -> Self {
        Self {
            id,
            parent_id,
            hierarchy: hierarchy.into(),
            external_id,
            name: name.into(),
            opening_date,
        }
    }

    /// Returns `true` if `self.id == office_id`, matching `OfficeData.hasIdentifyOf(officeId)`.
    pub const fn has_identify_of(&self, office_id: i64) -> bool {
        self.id == office_id
    }

    /// Computes Fineract's hierarchy-indented `nameDecorated` string (`....Branch Name`).
    pub fn name_decorated(&self) -> String {
        let dot_count = self.hierarchy.chars().filter(|&c| c == '.').count();
        let level = dot_count.saturating_sub(2);
        let prefix = ".".repeat(level * 4);
        format!("{prefix}{}", self.name)
    }
}

impl From<&Office> for OfficeData {
    fn from(office: &Office) -> Self {
        Self {
            id: office.id,
            parent_id: office.parent_id,
            hierarchy: office.hierarchy.clone(),
            external_id: office.external_id.clone(),
            name: office.name.clone(),
            opening_date: office.opening_date,
        }
    }
}

impl From<Office> for OfficeData {
    fn from(office: Office) -> Self {
        Self::from(&office)
    }
}

impl From<OfficeData> for Office {
    fn from(data: OfficeData) -> Self {
        Self {
            id: data.id,
            parent_id: data.parent_id,
            hierarchy: data.hierarchy,
            external_id: data.external_id,
            name: data.name,
            opening_date: data.opening_date,
        }
    }
}

// =============================================================================
// 2. STAFF DOMAIN & DATA (`m_staff`)
// =============================================================================

/// Supported JSON parameters for Staff create and update commands.
pub const STAFF_SUPPORTED_PARAMETERS: &[&str] = &[
    "officeId",
    "firstname",
    "lastname",
    "isLoanOfficer",
    "externalId",
    "emailAddress",
    "mobileNo",
    "isActive",
    "joiningDate",
    "organisationalRoleType",
    "organisationalRoleParentStaffId",
    "forceStatus",
    "locale",
    "dateFormat",
];

/// Staff organisational role enumeration (`m_staff.organisational_role_enum`).
///
/// Ported from `org.apache.fineract.organisation.staff.domain.StaffOrganisationalRoleType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(i16)]
pub enum StaffOrganisationalRoleType {
    #[default]
    Invalid = 0,
    ProgramDirector = 100,
    BranchManager = 200,
    FieldOfficerCoordinator = 300,
    FieldOfficer = 400,
}

impl StaffOrganisationalRoleType {
    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "staffOrganisationalRoleType.invalid",
            Self::ProgramDirector => "staffOrganisationalRoleType.programDirector",
            Self::BranchManager => "staffOrganisationalRoleType.branchManager",
            Self::FieldOfficerCoordinator => "staffOrganisationalRoleType.coordinator",
            Self::FieldOfficer => "staffOrganisationalRoleType.fieldAgent",
        }
    }

    pub const fn from_i16(value: i16) -> Self {
        match value {
            100 => Self::ProgramDirector,
            200 => Self::BranchManager,
            300 => Self::FieldOfficerCoordinator,
            400 => Self::FieldOfficer,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(100) => Self::ProgramDirector,
            Some(200) => Self::BranchManager,
            Some(300) => Self::FieldOfficerCoordinator,
            Some(400) => Self::FieldOfficer,
            _ => Self::Invalid,
        }
    }
}

/// Domain aggregate representing an employee / loan officer (`m_staff`).
///
/// Ported from `org.apache.fineract.organisation.staff.domain.Staff` and `StaffWriteServiceImpl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Staff {
    pub id: i64,
    pub office_id: i64,
    pub firstname: String,
    pub lastname: String,
    pub display_name: String,
    pub is_loan_officer: bool,
    pub is_active: bool,
    pub joining_date: Option<NaiveDate>,
    pub mobile_no: Option<String>,
    pub external_id: ExternalId,
    pub organisational_role_type: Option<i16>,
    pub organisational_role_parent_staff_id: Option<i64>,
}

/// Immutable DTO representation of a [`Staff`] member (`m_staff`).
///
/// Ported from `org.apache.fineract.organisation.staff.data.StaffData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaffData {
    pub id: i64,
    pub office_id: i64,
    pub firstname: String,
    pub lastname: String,
    pub display_name: String,
    pub is_loan_officer: bool,
    pub is_active: bool,
    pub joining_date: Option<NaiveDate>,
    pub mobile_no: Option<String>,
    pub external_id: ExternalId,
    pub organisational_role_type: Option<i16>,
    pub organisational_role_parent_staff_id: Option<i64>,
}

impl Staff {
    /// Formats a staff member's display name (`"{lastname}, {firstname}"`, or `"{lastname}"` if `firstname` is blank),
    /// matching `StaffWriteServiceImpl.createStaff` and `StaffWriteServiceImpl.updateStaff`.
    pub fn format_display_name(firstname: &str, lastname: &str) -> String {
        let f = firstname.trim();
        let l = lastname.trim();
        if f.is_empty() {
            l.to_string()
        } else {
            format!("{l}, {f}")
        }
    }

    /// Derives and updates `self.display_name` as `"{lastname}, {firstname}"` (or `"{lastname}"` when `firstname` is empty),
    /// returning the formatted `display_name`.
    pub fn derive_display_name(&mut self) -> String {
        self.display_name = Self::format_display_name(&self.firstname, &self.lastname);
        self.display_name.clone()
    }

    /// Creates a new [`Staff`] instance and derives `display_name` automatically.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        office_id: i64,
        firstname: impl Into<String>,
        lastname: impl Into<String>,
        is_loan_officer: bool,
        is_active: bool,
        joining_date: Option<NaiveDate>,
        mobile_no: Option<String>,
        external_id: ExternalId,
        organisational_role_type: Option<i16>,
        organisational_role_parent_staff_id: Option<i64>,
    ) -> Self {
        let firstname = firstname.into().trim().to_string();
        let lastname = lastname.into().trim().to_string();
        let display_name = Self::format_display_name(&firstname, &lastname);
        Self {
            id,
            office_id,
            firstname,
            lastname,
            display_name,
            is_loan_officer,
            is_active,
            joining_date,
            mobile_no,
            external_id,
            organisational_role_type,
            organisational_role_parent_staff_id,
        }
    }

    /// Validates a `CREATE STAFF` command payload matching `StaffCreateRequest` validation rules.
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(STAFF_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("staff");

        let office_id = command.long_value_of_parameter_named("officeId")?;
        validator
            .reset()
            .parameter("officeId")
            .value(office_id)
            .not_null()
            .integer_greater_than_zero();

        let firstname = command.string_value_of_parameter_named("firstname");
        validator
            .reset()
            .parameter("firstname")
            .value(firstname)
            .not_blank()
            .not_exceeding_length_of(50);

        let lastname = command.string_value_of_parameter_named("lastname");
        validator
            .reset()
            .parameter("lastname")
            .value(lastname)
            .not_blank()
            .not_exceeding_length_of(50);

        if command.parameter_exists("externalId") {
            let external_id = command.string_value_of_parameter_named("externalId");
            validator
                .reset()
                .parameter("externalId")
                .value(external_id)
                .not_exceeding_length_of(100);
        }

        if command.parameter_exists("mobileNo") {
            let mobile_no = command.string_value_of_parameter_named("mobileNo");
            validator
                .reset()
                .parameter("mobileNo")
                .value(mobile_no)
                .not_exceeding_length_of(50);
        }

        if command.parameter_exists("joiningDate") {
            let joining_date = command.local_date_value_of_parameter_named("joiningDate")?;
            validator
                .reset()
                .parameter("joiningDate")
                .value(joining_date)
                .not_null();
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates an `UPDATE STAFF` command payload matching `StaffUpdateRequest` validation rules.
    pub fn validate_for_update(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(STAFF_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("staff");

        if command.parameter_exists("officeId") {
            let office_id = command.long_value_of_parameter_named("officeId")?;
            validator
                .reset()
                .parameter("officeId")
                .value(office_id)
                .not_null()
                .integer_greater_than_zero();
        }

        if command.parameter_exists("firstname") {
            let firstname = command.string_value_of_parameter_named("firstname");
            validator
                .reset()
                .parameter("firstname")
                .value(firstname)
                .not_blank()
                .not_exceeding_length_of(50);
        }

        if command.parameter_exists("lastname") {
            let lastname = command.string_value_of_parameter_named("lastname");
            validator
                .reset()
                .parameter("lastname")
                .value(lastname)
                .not_blank()
                .not_exceeding_length_of(50);
        }

        if command.parameter_exists("externalId") {
            let external_id = command.string_value_of_parameter_named("externalId");
            validator
                .reset()
                .parameter("externalId")
                .value(external_id)
                .not_exceeding_length_of(100);
        }

        if command.parameter_exists("mobileNo") {
            let mobile_no = command.string_value_of_parameter_named("mobileNo");
            validator
                .reset()
                .parameter("mobileNo")
                .value(mobile_no)
                .not_exceeding_length_of(50);
        }

        if command.parameter_exists("joiningDate") {
            let joining_date = command.local_date_value_of_parameter_named("joiningDate")?;
            validator
                .reset()
                .parameter("joiningDate")
                .value(joining_date)
                .not_null();
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a [`Staff`] aggregate from a [`JsonCommand`], matching `StaffWriteServiceImpl.createStaff`.
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;
        let office_id = command
            .long_value_of_parameter_named("officeId")?
            .unwrap_or(1);
        let firstname = command
            .string_value_of_parameter_named("firstname")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let lastname = command
            .string_value_of_parameter_named("lastname")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let is_loan_officer = command
            .boolean_object_value_of_parameter_named("isLoanOfficer")
            .unwrap_or(false);
        let is_active = command
            .boolean_object_value_of_parameter_named("isActive")
            .unwrap_or(true);
        let joining_date = command.local_date_value_of_parameter_named("joiningDate")?;
        let mobile_no = command.string_value_of_parameter_named("mobileNo");
        let external_id = ExternalId::from_opt(command.string_value_of_parameter_named("externalId"));
        let organisational_role_type = command
            .integer_value_sans_locale_of_parameter_named("organisationalRoleType")?
            .and_then(|v| i16::try_from(v).ok());
        let organisational_role_parent_staff_id =
            command.long_value_of_parameter_named("organisationalRoleParentStaffId")?;

        Ok(Self::new(
            id,
            office_id,
            firstname,
            lastname,
            is_loan_officer,
            is_active,
            joining_date,
            mobile_no,
            external_id,
            organisational_role_type,
            organisational_role_parent_staff_id,
        ))
    }

    /// Applies a [`JsonCommand`] update to `self` and re-derives `display_name` when `firstname` or `lastname` changes,
    /// matching `StaffWriteServiceImpl.updateStaff`.
    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        Self::validate_for_update(command)?;
        let mut changes = BTreeMap::new();

        if command.is_change_in_long_parameter_named("officeId", Some(self.office_id))? {
            if let Some(new_office_id) = command.long_value_of_parameter_named("officeId")? {
                self.office_id = new_office_id;
                changes.insert("officeId".to_string(), Value::from(new_office_id));
            }
        }

        if command.is_change_in_string_parameter_named("firstname", Some(&self.firstname)) {
            if let Some(new_firstname) = command.string_value_of_parameter_named("firstname") {
                let trimmed = new_firstname.trim().to_string();
                self.firstname = trimmed.clone();
                changes.insert("firstname".to_string(), Value::String(trimmed));
            }
        }

        if command.is_change_in_string_parameter_named("lastname", Some(&self.lastname)) {
            if let Some(new_lastname) = command.string_value_of_parameter_named("lastname") {
                let trimmed = new_lastname.trim().to_string();
                self.lastname = trimmed.clone();
                changes.insert("lastname".to_string(), Value::String(trimmed));
            }
        }

        if command.is_change_in_string_parameter_named("externalId", self.external_id.value()) {
            let new_ext = command.string_value_of_parameter_named("externalId");
            changes.insert(
                "externalId".to_string(),
                new_ext
                    .clone()
                    .map(Value::String)
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            self.external_id = ExternalId::from_opt(new_ext);
        }

        if command.is_change_in_string_parameter_named("mobileNo", self.mobile_no.as_deref()) {
            let new_mobile = command.string_value_of_parameter_named("mobileNo");
            changes.insert(
                "mobileNo".to_string(),
                new_mobile
                    .clone()
                    .map(Value::String)
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            self.mobile_no = new_mobile;
        }

        if command.is_change_in_boolean_parameter_named("isLoanOfficer", Some(self.is_loan_officer))
        {
            if let Some(flag) = command.boolean_object_value_of_parameter_named("isLoanOfficer") {
                self.is_loan_officer = flag;
                changes.insert("isLoanOfficer".to_string(), Value::Bool(flag));
            }
        }

        if command.is_change_in_boolean_parameter_named("isActive", Some(self.is_active)) {
            if let Some(flag) = command.boolean_object_value_of_parameter_named("isActive") {
                self.is_active = flag;
                changes.insert("isActive".to_string(), Value::Bool(flag));
            }
        }

        if command.is_change_in_local_date_parameter_named("joiningDate", self.joining_date)? {
            if let Some(raw_joining) = command.string_value_of_parameter_named("joiningDate") {
                changes.insert("joiningDate".to_string(), Value::String(raw_joining));
            }
            self.joining_date = command.local_date_value_of_parameter_named("joiningDate")?;
        }

        if command.parameter_exists("organisationalRoleType") {
            let new_role = command
                .integer_value_sans_locale_of_parameter_named("organisationalRoleType")?
                .and_then(|v| i16::try_from(v).ok());
            if new_role != self.organisational_role_type {
                self.organisational_role_type = new_role;
                changes.insert(
                    "organisationalRoleType".to_string(),
                    new_role.map(Value::from).unwrap_or(Value::Null),
                );
            }
        }

        if command.is_change_in_long_parameter_named(
            "organisationalRoleParentStaffId",
            self.organisational_role_parent_staff_id,
        )? {
            let new_parent_staff =
                command.long_value_of_parameter_named("organisationalRoleParentStaffId")?;
            self.organisational_role_parent_staff_id = new_parent_staff;
            changes.insert(
                "organisationalRoleParentStaffId".to_string(),
                new_parent_staff.map(Value::from).unwrap_or(Value::Null),
            );
        }

        if !changes.is_empty() {
            self.derive_display_name();
        }

        Ok(changes)
    }

    /// Converts this [`Staff`] aggregate into a [`StaffData`] DTO.
    pub fn to_data(&self) -> StaffData {
        StaffData::from(self)
    }
}

impl StaffData {
    /// Derives and updates `self.display_name` as `"{lastname}, {firstname}"`.
    pub fn derive_display_name(&mut self) -> String {
        self.display_name = Staff::format_display_name(&self.firstname, &self.lastname);
        self.display_name.clone()
    }
}

impl From<&Staff> for StaffData {
    fn from(staff: &Staff) -> Self {
        Self {
            id: staff.id,
            office_id: staff.office_id,
            firstname: staff.firstname.clone(),
            lastname: staff.lastname.clone(),
            display_name: staff.display_name.clone(),
            is_loan_officer: staff.is_loan_officer,
            is_active: staff.is_active,
            joining_date: staff.joining_date,
            mobile_no: staff.mobile_no.clone(),
            external_id: staff.external_id.clone(),
            organisational_role_type: staff.organisational_role_type,
            organisational_role_parent_staff_id: staff.organisational_role_parent_staff_id,
        }
    }
}

impl From<Staff> for StaffData {
    fn from(staff: Staff) -> Self {
        Self::from(&staff)
    }
}

impl From<StaffData> for Staff {
    fn from(data: StaffData) -> Self {
        Self {
            id: data.id,
            office_id: data.office_id,
            firstname: data.firstname,
            lastname: data.lastname,
            display_name: data.display_name,
            is_loan_officer: data.is_loan_officer,
            is_active: data.is_active,
            joining_date: data.joining_date,
            mobile_no: data.mobile_no,
            external_id: data.external_id,
            organisational_role_type: data.organisational_role_type,
            organisational_role_parent_staff_id: data.organisational_role_parent_staff_id,
        }
    }
}

// =============================================================================
// 3. PROVISIONING CATEGORY & CRITERIA (`m_provision_category`, `m_provisioning_criteria`)
// =============================================================================

/// Loan provisioning category (`m_provision_category` / `m_provisioning_category`),
/// e.g. `STANDARD`, `SUB-STANDARD`, `DOUBTFUL`, `LOSS`.
///
/// Ported from `org.apache.fineract.organisation.provisioning.domain.ProvisioningCategory`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisioningCategory {
    pub id: i64,
    pub category_name: String,
    pub category_description: Option<String>,
}

impl ProvisioningCategory {
    /// Creates a new [`ProvisioningCategory`].
    pub fn new(
        id: i64,
        category_name: impl Into<String>,
        category_description: Option<String>,
    ) -> Self {
        Self {
            id,
            category_name: category_name.into().trim().to_string(),
            category_description: category_description
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }

    /// Constructs a [`ProvisioningCategory`] from a [`JsonCommand`], matching `ProvisioningCategory.fromJson`.
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        let name = command
            .string_value_of_parameter_named("categoryname")
            .or_else(|| command.string_value_of_parameter_named("categoryName"));
        let validator = DataValidatorBuilder::for_resource("provisioningcategory");
        validator
            .reset()
            .parameter("categoryname")
            .value(name.clone())
            .not_blank()
            .not_exceeding_length_of(100);
        validator.throw_validation_errors()?;

        let description = command
            .string_value_of_parameter_named("description")
            .or_else(|| command.string_value_of_parameter_named("categorydescription"))
            .or_else(|| command.string_value_of_parameter_named("categoryDescription"));

        Ok(Self::new(id, name.unwrap_or_default(), description))
    }

    /// Updates this [`ProvisioningCategory`] from a [`JsonCommand`], matching `ProvisioningCategory.update`.
    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        let mut actual_changes = BTreeMap::new();
        if command.is_change_in_string_parameter_named("categoryname", Some(&self.category_name)) {
            if let Some(new_val) = command.string_value_of_parameter_named("categoryname") {
                actual_changes.insert("categoryname".to_string(), Value::String(new_val.clone()));
                self.category_name = new_val;
            }
        }
        if command.is_change_in_string_parameter_named(
            "categorydescription",
            self.category_description.as_deref(),
        ) {
            let new_val = command.string_value_of_parameter_named("categorydescription");
            actual_changes.insert(
                "categorydescription".to_string(),
                new_val
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            self.category_description = new_val;
        }
        Ok(actual_changes)
    }
}

/// Delinquency age-bucket definition within a [`ProvisioningCriteria`] (`m_provisioning_criteria_definition`).
///
/// Ported from `org.apache.fineract.organisation.provisioning.domain.ProvisioningCriteriaDefinition`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisioningCriteriaDefinition {
    pub id: i64,
    pub category_id: i64,
    pub min_age: i64,
    pub max_age: i64,
    pub provisioning_percentage: Decimal,
    pub liability_account_id: i64,
    pub expense_account_id: i64,
}

impl ProvisioningCriteriaDefinition {
    /// Creates a new [`ProvisioningCriteriaDefinition`].
    pub const fn new(
        id: i64,
        category_id: i64,
        min_age: i64,
        max_age: i64,
        provisioning_percentage: Decimal,
        liability_account_id: i64,
        expense_account_id: i64,
    ) -> Self {
        Self {
            id,
            category_id,
            min_age,
            max_age,
            provisioning_percentage,
            liability_account_id,
            expense_account_id,
        }
    }

    /// Returns the inclusive delinquency day range (`min_age..=max_age`).
    pub const fn age_range(&self) -> RangeInclusive<i64> {
        self.min_age..=self.max_age
    }

    /// Returns `true` if `delinquency_days` falls within `self.min_age..=self.max_age`.
    pub fn contains_age(&self, delinquency_days: i64) -> bool {
        self.age_range().contains(&delinquency_days)
    }

    /// Returns `true` if the inclusive age bucket `self.min_age..=self.max_age` overlaps with
    /// `other.min_age..=other.max_age`.
    ///
    /// Ported directly from `ProvisioningCriteriaDefinition.isOverlapping`:
    /// `this.minimumAge <= def.maximumAge && def.minimumAge <= this.maximumAge`.
    pub const fn is_overlapping(&self, other: &Self) -> bool {
        self.min_age <= other.max_age && other.min_age <= self.max_age
    }

    /// Updates the age bucket, provisioning percentage, and GL account bindings in place,
    /// matching `ProvisioningCriteriaDefinition.update(...)`.
    pub fn update(
        &mut self,
        min_age: i64,
        max_age: i64,
        provisioning_percentage: Decimal,
        liability_account_id: i64,
        expense_account_id: i64,
    ) {
        self.min_age = min_age;
        self.max_age = max_age;
        self.provisioning_percentage = provisioning_percentage;
        self.liability_account_id = liability_account_id;
        self.expense_account_id = expense_account_id;
    }
}

/// Loan provisioning criteria aggregate (`m_provisioning_criteria`), binding a named set of
/// non-overlapping [`ProvisioningCriteriaDefinition`] delinquency buckets to loan products
/// (`m_loanproduct_provisioning_mapping`).
///
/// Ported from `org.apache.fineract.organisation.provisioning.domain.ProvisioningCriteria`
/// and `org.apache.fineract.organisation.provisioning.service.ProvisioningCriteriaAssembler`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisioningCriteria {
    pub id: i64,
    pub criteria_name: String,
    pub definitions: Vec<ProvisioningCriteriaDefinition>,
    pub loan_product_ids: Vec<i64>,
}

impl ProvisioningCriteria {
    /// Creates and validates a new [`ProvisioningCriteria`], ensuring non-overlapping age buckets.
    pub fn new(
        id: i64,
        criteria_name: impl Into<String>,
        definitions: Vec<ProvisioningCriteriaDefinition>,
        loan_product_ids: Vec<i64>,
    ) -> FineractResult<Self> {
        let criteria = Self {
            id,
            criteria_name: criteria_name.into().trim().to_string(),
            definitions,
            loan_product_ids,
        };
        criteria.validate()?;
        Ok(criteria)
    }

    /// Creates a [`ProvisioningCriteria`] without running validation.
    pub fn new_unchecked(
        id: i64,
        criteria_name: impl Into<String>,
        definitions: Vec<ProvisioningCriteriaDefinition>,
        loan_product_ids: Vec<i64>,
    ) -> Self {
        Self {
            id,
            criteria_name: criteria_name.into().trim().to_string(),
            definitions,
            loan_product_ids,
        }
    }

    /// Validates that each [`ProvisioningCriteriaDefinition`] has a valid range (`0 <= min_age < max_age`,
    /// `provisioning_percentage >= 0`, positive `category_id` and GL accounts) and that no two definitions
    /// have overlapping delinquency day ranges (`min_age..=max_age`).
    ///
    /// Ported from `ProvisioningCriteriaDefinitionJsonDeserializer.validateForCreate` and
    /// `ProvisioningCriteriaAssembler.validateRange`.
    pub fn validate_definitions(definitions: &[ProvisioningCriteriaDefinition]) -> FineractResult<()> {
        let validator = DataValidatorBuilder::for_resource("provisioningcriteria");
        if definitions.is_empty() {
            validator
                .reset()
                .parameter("definitions")
                .value(Vec::<i64>::new())
                .list_not_empty();
            return validator
                .throw_validation_errors()
                .map_err(FineractError::from);
        }

        for (i, def) in definitions.iter().enumerate() {
            let idx = i + 1;
            validator
                .reset()
                .parameter("definitions")
                .parameter_at_index_array("categoryId", idx)
                .value(def.category_id)
                .not_null()
                .long_greater_than_zero();

            validator
                .reset()
                .parameter("minAge")
                .parameter_at_index_array("minAge", idx)
                .value(def.min_age)
                .not_null()
                .long_zero_or_greater();

            validator
                .reset()
                .parameter("maxAge")
                .parameter_at_index_array("maxAge", idx)
                .value(def.max_age)
                .not_null()
                .integer_greater_than_number(def.min_age);

            validator
                .reset()
                .parameter("provisioningPercentage")
                .parameter_at_index_array("provisioningPercentage", idx)
                .value(def.provisioning_percentage)
                .not_null()
                .zero_or_positive_amount();

            validator
                .reset()
                .parameter("liabilityAccount")
                .parameter_at_index_array("liabilityAccount", idx)
                .value(def.liability_account_id)
                .not_null()
                .long_greater_than_zero();

            validator
                .reset()
                .parameter("expenseAccount")
                .parameter_at_index_array("expenseAccount", idx)
                .value(def.expense_account_id)
                .not_null()
                .long_greater_than_zero();
        }

        validator.throw_validation_errors()?;

        for i in 0..definitions.len() {
            for j in (i + 1)..definitions.len() {
                if definitions[i].is_overlapping(&definitions[j]) {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.provisioning.criteria.definitions.are.overlapping",
                        "Provisioning Criteria definitions are overlapping ",
                    )));
                }
            }
        }

        Ok(())
    }

    /// Validates that no two definitions in `self.definitions` overlap (`min_age..=max_age`).
    pub fn validate_non_overlapping(&self) -> FineractResult<()> {
        Self::validate_definitions(&self.definitions)
    }

    /// Validates `criteria_name`, `loan_product_ids`, and `definitions` (including non-overlapping buckets).
    pub fn validate(&self) -> FineractResult<()> {
        let validator = DataValidatorBuilder::for_resource("provisioningcriteria");
        validator
            .reset()
            .parameter("criteriaName")
            .value(self.criteria_name.as_str())
            .not_blank()
            .not_exceeding_length_of(200);

        for (i, &product_id) in self.loan_product_ids.iter().enumerate() {
            validator
                .reset()
                .parameter("loanProducts")
                .parameter_at_index_array("id", i + 1)
                .value(product_id)
                .not_null()
                .long_greater_than_zero();
        }

        validator.throw_validation_errors()?;
        Self::validate_definitions(&self.definitions)
    }

    /// Parses and validates a [`ProvisioningCriteria`] from a [`JsonCommand`],
    /// matching `ProvisioningCriteriaDefinitionJsonDeserializer` and `ProvisioningCriteriaAssembler.fromParsedJson`.
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        let criteria_name = command
            .string_value_of_parameter_named("criteriaName")
            .or_else(|| command.string_value_of_parameter_named("criterianame"))
            .unwrap_or_default();

        let validator = DataValidatorBuilder::for_resource("provisioningcriteria");
        validator
            .reset()
            .parameter("criteriaName")
            .value(criteria_name.as_str())
            .not_blank()
            .not_exceeding_length_of(200);

        let locale_raw = command.locale();
        let locale = if locale_raw.trim().is_empty() {
            "en".to_string()
        } else {
            locale_raw
        };

        let mut loan_product_ids = Vec::new();
        if let Some(products_val) = command.json_element("loanProducts") {
            let Some(arr) = products_val.as_array() else {
                validator
                    .reset()
                    .parameter("loanProducts")
                    .value(Vec::<i64>::new())
                    .list_not_empty();
                validator.throw_validation_errors()?;
                unreachable!()
            };
            if arr.is_empty() {
                validator
                    .reset()
                    .parameter("loanProducts")
                    .value(Vec::<i64>::new())
                    .list_not_empty();
                validator.throw_validation_errors()?;
            }
            for (idx, item) in arr.iter().enumerate() {
                let prod_id = match item {
                    Value::Object(map) => match map.get("id") {
                        Some(v) => FineractNumberParser::parse_i64_for_param("id", v, &locale)?,
                        None => None,
                    },
                    other => FineractNumberParser::parse_i64_for_param("id", other, &locale)?,
                };
                validator
                    .reset()
                    .parameter("loanProducts")
                    .parameter_at_index_array("id", idx + 1)
                    .value(prod_id)
                    .not_null()
                    .long_greater_than_zero();
                if let Some(pid) = prod_id {
                    loan_product_ids.push(pid);
                }
            }
        }

        let defs_val = command
            .json_element("definitions")
            .or_else(|| command.json_element("provisioningcriteria"));
        let Some(defs_arr) = defs_val.and_then(Value::as_array) else {
            validator
                .reset()
                .parameter("definitions")
                .value(Vec::<i64>::new())
                .list_not_empty();
            validator.throw_validation_errors()?;
            unreachable!()
        };
        if defs_arr.is_empty() {
            validator
                .reset()
                .parameter("definitions")
                .value(Vec::<i64>::new())
                .list_not_empty();
            validator.throw_validation_errors()?;
        }

        let mut definitions = Vec::with_capacity(defs_arr.len());
        for (idx, item) in defs_arr.iter().enumerate() {
            let Some(obj) = item.as_object() else {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.provisioningcriteria.definitions.invalid",
                        "Each provisioning definition must be a JSON object.",
                        "definitions",
                        Vec::new(),
                    ),
                ));
            };

            let category_id = obj
                .get("categoryId")
                .and_then(|v| FineractNumberParser::parse_i64_for_param("categoryId", v, &locale).ok().flatten())
                .unwrap_or(0);
            let min_age = obj
                .get("minAge")
                .and_then(|v| FineractNumberParser::parse_i64_for_param("minAge", v, &locale).ok().flatten())
                .unwrap_or(-1);
            let max_age = obj
                .get("maxAge")
                .and_then(|v| FineractNumberParser::parse_i64_for_param("maxAge", v, &locale).ok().flatten())
                .unwrap_or(-1);
            let provisioning_percentage = match obj.get("provisioningPercentage") {
                Some(v) => FineractNumberParser::parse_decimal_for_param(
                    "provisioningPercentage",
                    v,
                    &locale,
                )?
                .unwrap_or(Decimal::NEGATIVE_ONE),
                None => Decimal::NEGATIVE_ONE,
            };
            let liability_account_id = obj
                .get("liabilityAccount")
                .or_else(|| obj.get("liabilityAccountId"))
                .and_then(|v| {
                    FineractNumberParser::parse_i64_for_param("liabilityAccount", v, &locale)
                        .ok()
                        .flatten()
                })
                .unwrap_or(0);
            let expense_account_id = obj
                .get("expenseAccount")
                .or_else(|| obj.get("expenseAccountId"))
                .and_then(|v| {
                    FineractNumberParser::parse_i64_for_param("expenseAccount", v, &locale)
                        .ok()
                        .flatten()
                })
                .unwrap_or(0);

            let def_id = obj
                .get("id")
                .and_then(|v| FineractNumberParser::parse_i64_for_param("id", v, &locale).ok().flatten())
                .unwrap_or((idx + 1) as i64);

            definitions.push(ProvisioningCriteriaDefinition::new(
                def_id,
                category_id,
                min_age,
                max_age,
                provisioning_percentage,
                liability_account_id,
                expense_account_id,
            ));
        }

        validator.throw_validation_errors()?;
        Self::new(id, criteria_name, definitions, loan_product_ids)
    }

    /// Finds the [`ProvisioningCriteriaDefinition`] matching `delinquency_days` (`min_age <= delinquency_days <= max_age`).
    pub fn definition_for_age(&self, delinquency_days: i64) -> Option<&ProvisioningCriteriaDefinition> {
        self.definitions
            .iter()
            .find(|def| def.contains_age(delinquency_days))
    }

    /// Indexes definitions by `category_id`, keeping the first definition if duplicate category keys exist,
    /// matching `ProvisioningCriteria.getDefinitionsByCategoryId()` in `ProvisioningCriteria.java`.
    pub fn definitions_by_category_id(&self) -> BTreeMap<i64, &ProvisioningCriteriaDefinition> {
        let mut map = BTreeMap::new();
        for def in &self.definitions {
            map.entry(def.category_id).or_insert(def);
        }
        map
    }
}

// =============================================================================
// 4. DUAL-DB ORGANISATION REPOSITORY (`m_office`, `m_staff`, `m_provision_category`, `m_provisioning_criteria`)
// =============================================================================

/// Dual-Database (`PostgreSQL` + `MySQL/MariaDB`) and concurrent in-memory repository for
/// `m_office`, `m_staff`, `m_provision_category`, `m_provisioning_criteria`,
/// `m_provisioning_criteria_definition`, and `m_loanproduct_provisioning_mapping`.
#[derive(Debug, Clone)]
pub struct DualDbOrganisationRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    offices: Arc<DashMap<i64, Office>>,
    staff: Arc<DashMap<i64, Staff>>,
    categories: Arc<DashMap<i64, ProvisioningCategory>>,
    criteria: Arc<DashMap<i64, ProvisioningCriteria>>,
    next_id: Arc<AtomicI64>,
}

impl Default for DualDbOrganisationRepository {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl DualDbOrganisationRepository {
    /// Creates a new [`DualDbOrganisationRepository`] backed by a SQLx [`AnyPool`] and [`SqlDialect`].
    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            offices: Arc::new(DashMap::new()),
            staff: Arc::new(DashMap::new()),
            categories: Arc::new(DashMap::new()),
            criteria: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(2)),
        }
    }

    /// Creates a fast concurrent in-memory [`DualDbOrganisationRepository`] pre-seeded with
    /// Fineract's default Head Office (`id = 1`, `hierarchy = ".1."`) and standard provisioning categories.
    pub fn new_in_memory() -> Self {
        let offices = DashMap::new();
        let default_opening =
            NaiveDate::from_ymd_opt(2009, 1, 1).expect("2009-01-01 is a valid date");
        offices.insert(
            1,
            Office::head_office(1, "Head Office", default_opening, ExternalId::empty()),
        );

        let categories = DashMap::new();
        categories.insert(
            1,
            ProvisioningCategory::new(
                1,
                "STANDARD",
                Some("Promptly paid loans".to_string()),
            ),
        );
        categories.insert(
            2,
            ProvisioningCategory::new(
                2,
                "SUB-STANDARD",
                Some("Principal and/or Interest overdue by x days".to_string()),
            ),
        );
        categories.insert(
            3,
            ProvisioningCategory::new(
                3,
                "DOUBTFUL",
                Some("Principal and/or Interest overdue by x days and less than y".to_string()),
            ),
        );
        categories.insert(
            4,
            ProvisioningCategory::new(
                4,
                "LOSS",
                Some("Principal and/or Interest overdue by y days".to_string()),
            ),
        );

        Self {
            pool: None,
            dialect: SqlDialect::Postgres,
            offices: Arc::new(offices),
            staff: Arc::new(DashMap::new()),
            categories: Arc::new(categories),
            criteria: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(2)),
        }
    }

    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    fn allocate_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    // --- Dialect-aware SQL statements ---

    pub fn select_office_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, parent_id, hierarchy, external_id, name, opening_date FROM m_office WHERE id = ?",
            dialect,
        )
    }

    pub fn select_offices_by_hierarchy_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, parent_id, hierarchy, external_id, name, opening_date FROM m_office WHERE hierarchy LIKE ? ORDER BY hierarchy ASC",
            dialect,
        )
    }

    pub fn insert_office_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_office (id, parent_id, hierarchy, external_id, name, opening_date) VALUES (?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn update_office_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_office SET parent_id = ?, hierarchy = ?, external_id = ?, name = ?, opening_date = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn select_staff_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, office_id, firstname, lastname, display_name, is_loan_officer, is_active, joining_date, mobile_no, external_id, organisational_role_enum, organisational_role_parent_staff_id FROM m_staff WHERE id = ?",
            dialect,
        )
    }

    pub fn insert_staff_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_staff (id, office_id, firstname, lastname, display_name, is_loan_officer, is_active, joining_date, mobile_no, external_id, organisational_role_enum, organisational_role_parent_staff_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn update_staff_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_staff SET office_id = ?, firstname = ?, lastname = ?, display_name = ?, is_loan_officer = ?, is_active = ?, joining_date = ?, mobile_no = ?, external_id = ?, organisational_role_enum = ?, organisational_role_parent_staff_id = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn select_provisioning_category_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, category_name, description FROM m_provision_category WHERE id = ?",
            dialect,
        )
    }

    pub fn insert_provisioning_category_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_provision_category (id, category_name, description) VALUES (?, ?, ?)",
            dialect,
        )
    }

    pub fn insert_provisioning_criteria_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_provisioning_criteria (id, criteria_name) VALUES (?, ?)",
            dialect,
        )
    }

    pub fn insert_provisioning_criteria_definition_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_provisioning_criteria_definition (id, criteria_id, category_id, min_age, max_age, provision_percentage, liability_account, expense_account) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            dialect,
        )
    }

    pub fn insert_loanproduct_provisioning_mapping_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "INSERT INTO m_loanproduct_provisioning_mapping (id, product_id, criteria_id) VALUES (?, ?, ?)",
            dialect,
        )
    }

    pub fn select_provisioning_criteria_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "SELECT id, criteria_name FROM m_provisioning_criteria WHERE id = ?",
            dialect,
        )
    }

    // --- Office Operations ---

    /// Retrieves an [`Office`] by ID, returning `OfficeNotFoundException` (`error.msg.office.id.invalid`) if not found.
    pub async fn get_office(&self, office_id: i64) -> FineractResult<Office> {
        if let Some(entry) = self.offices.get(&office_id) {
            return Ok(entry.value().clone());
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_office_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(office_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let parent_id: Option<i64> = row
                    .try_get("parent_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let hierarchy: Option<String> = row
                    .try_get("hierarchy")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let external_id_raw: Option<String> = row
                    .try_get("external_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let name: String = row
                    .try_get("name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let opening_date_str: String = row
                    .try_get("opening_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let opening_date = NaiveDate::parse_from_str(&opening_date_str, "%Y-%m-%d")
                    .or_else(|_| NaiveDate::parse_from_str(opening_date_str.split('T').next().unwrap_or(&opening_date_str), "%Y-%m-%d"))
                    .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
                let office = Office {
                    id,
                    parent_id,
                    hierarchy: hierarchy.unwrap_or_else(|| format!(".{id}.")),
                    external_id: ExternalId::from_opt(external_id_raw),
                    name,
                    opening_date,
                };
                self.offices.insert(id, office.clone());
                return Ok(office);
            }
        }
        Err(FineractError::resource_not_found(
            "error.msg.office.id.invalid",
            format!("Office with identifier {office_id} does not exist"),
            office_id.to_string(),
        ))
    }

    /// Saves a new [`Office`] after checking unique `name` and `external_id` constraints.
    pub async fn save_office(&self, mut office: Office) -> FineractResult<Office> {
        if office.id <= 0 {
            office.id = self.allocate_id();
        }
        let parent_hierarchy = if let Some(pid) = office.parent_id {
            let parent = self.get_office(pid).await?;
            Some(parent.hierarchy)
        } else {
            None
        };
        office.generate_hierarchy(parent_hierarchy.as_deref());

        for item in self.offices.iter() {
            let existing = item.value();
            if existing.id != office.id {
                if existing.name.eq_ignore_ascii_case(&office.name) {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.office.duplicate.name",
                        format!("Office with name `{}` already exists", office.name),
                    )));
                }
                if let (Some(a), Some(b)) = (existing.external_id.value(), office.external_id.value())
                {
                    if a == b {
                        return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                            "error.msg.office.duplicate.externalId",
                            format!("Office with externalId `{b}` already exists"),
                        )));
                    }
                }
            }
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_office_sql(self.dialect);
            sqlx::query(&sql)
                .bind(office.id)
                .bind(office.parent_id)
                .bind(&office.hierarchy)
                .bind(office.external_id.value())
                .bind(&office.name)
                .bind(office.opening_date.to_string())
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.offices.insert(office.id, office.clone());
        Ok(office)
    }

    /// Updates an existing [`Office`] and cascades materialized hierarchy changes to any descendant offices.
    pub async fn update_office(&self, office: Office) -> FineractResult<Office> {
        let old_hierarchy = self
            .offices
            .get(&office.id)
            .map(|o| o.hierarchy.clone());

        if let Some(ref pool) = self.pool {
            let sql = Self::update_office_sql(self.dialect);
            sqlx::query(&sql)
                .bind(office.parent_id)
                .bind(&office.hierarchy)
                .bind(office.external_id.value())
                .bind(&office.name)
                .bind(office.opening_date.to_string())
                .bind(office.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.offices.insert(office.id, office.clone());

        // Cascade hierarchy prefix rewrite to descendants if this office's hierarchy path changed
        if let Some(old_prefix) = old_hierarchy {
            if !old_prefix.is_empty() && old_prefix != office.hierarchy {
                let mut descendants_to_update = Vec::new();
                for entry in self.offices.iter() {
                    let child = entry.value();
                    if child.id != office.id && child.hierarchy.starts_with(&old_prefix) {
                        let suffix = &child.hierarchy[old_prefix.len()..];
                        let mut updated_child = child.clone();
                        updated_child.hierarchy = format!("{}{suffix}", office.hierarchy);
                        descendants_to_update.push(updated_child);
                    }
                }
                for child in descendants_to_update {
                    self.offices.insert(child.id, child);
                }
            }
        }

        Ok(office)
    }

    /// Lists all offices ordered by hierarchy path.
    pub fn list_offices(&self) -> Vec<OfficeData> {
        let mut items: Vec<OfficeData> = self
            .offices
            .iter()
            .map(|e| OfficeData::from(e.value()))
            .collect();
        items.sort_by(|a, b| a.hierarchy.cmp(&b.hierarchy).then_with(|| a.id.cmp(&b.id)));
        items
    }

    // --- Staff Operations ---

    /// Retrieves a [`Staff`] member by ID, returning `StaffNotFoundException` (`error.msg.staff.id.invalid`) if not found.
    pub async fn get_staff(&self, staff_id: i64) -> FineractResult<Staff> {
        if let Some(entry) = self.staff.get(&staff_id) {
            return Ok(entry.value().clone());
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_staff_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(staff_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let office_id: i64 = row
                    .try_get("office_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let firstname: Option<String> = row
                    .try_get("firstname")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let lastname: Option<String> = row
                    .try_get("lastname")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let display_name: String = row
                    .try_get("display_name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let is_loan_officer: bool = row
                    .try_get("is_loan_officer")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let is_active: bool = row
                    .try_get("is_active")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let joining_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("joining_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                        .or_else(|_| NaiveDate::parse_from_str(s.split('T').next().unwrap_or(&s), "%Y-%m-%d"))
                        .ok());
                let mobile_no: Option<String> = row
                    .try_get("mobile_no")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let external_id_raw: Option<String> = row
                    .try_get("external_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let organisational_role_type: Option<i16> = row
                    .try_get("organisational_role_enum")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let organisational_role_parent_staff_id: Option<i64> = row
                    .try_get("organisational_role_parent_staff_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

                let staff = Staff {
                    id,
                    office_id,
                    firstname: firstname.unwrap_or_default(),
                    lastname: lastname.unwrap_or_default(),
                    display_name,
                    is_loan_officer,
                    is_active,
                    joining_date,
                    mobile_no,
                    external_id: ExternalId::from_opt(external_id_raw),
                    organisational_role_type,
                    organisational_role_parent_staff_id,
                };
                self.staff.insert(id, staff.clone());
                return Ok(staff);
            }
        }
        Err(FineractError::resource_not_found(
            "error.msg.staff.id.invalid",
            format!("Staff with identifier {staff_id} does not exist"),
            staff_id.to_string(),
        ))
    }

    /// Saves a new [`Staff`] record after verifying that its `office_id` exists and `display_name` / `external_id` are unique.
    pub async fn save_staff(&self, mut staff: Staff) -> FineractResult<Staff> {
        let _office = self.get_office(staff.office_id).await?;
        if staff.id <= 0 {
            staff.id = self.allocate_id();
        }
        staff.derive_display_name();

        for item in self.staff.iter() {
            let existing = item.value();
            if existing.id != staff.id {
                if existing.display_name.eq_ignore_ascii_case(&staff.display_name) {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.staff.duplicate.displayName",
                        format!(
                            "A staff with the given display name '{}' already exists",
                            staff.display_name
                        ),
                    )));
                }
                if let (Some(a), Some(b)) = (existing.external_id.value(), staff.external_id.value())
                {
                    if a == b {
                        return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                            "error.msg.staff.duplicate.externalId",
                            format!("Staff with externalId `{b}` already exists"),
                        )));
                    }
                }
            }
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_staff_sql(self.dialect);
            sqlx::query(&sql)
                .bind(staff.id)
                .bind(staff.office_id)
                .bind(&staff.firstname)
                .bind(&staff.lastname)
                .bind(&staff.display_name)
                .bind(staff.is_loan_officer)
                .bind(staff.is_active)
                .bind(staff.joining_date.map(|d| d.to_string()))
                .bind(&staff.mobile_no)
                .bind(staff.external_id.value())
                .bind(staff.organisational_role_type)
                .bind(staff.organisational_role_parent_staff_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.staff.insert(staff.id, staff.clone());
        Ok(staff)
    }

    /// Updates an existing [`Staff`] record.
    pub async fn update_staff(&self, mut staff: Staff) -> FineractResult<Staff> {
        let _office = self.get_office(staff.office_id).await?;
        staff.derive_display_name();

        if let Some(ref pool) = self.pool {
            let sql = Self::update_staff_sql(self.dialect);
            sqlx::query(&sql)
                .bind(staff.office_id)
                .bind(&staff.firstname)
                .bind(&staff.lastname)
                .bind(&staff.display_name)
                .bind(staff.is_loan_officer)
                .bind(staff.is_active)
                .bind(staff.joining_date.map(|d| d.to_string()))
                .bind(&staff.mobile_no)
                .bind(staff.external_id.value())
                .bind(staff.organisational_role_type)
                .bind(staff.organisational_role_parent_staff_id)
                .bind(staff.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.staff.insert(staff.id, staff.clone());
        Ok(staff)
    }

    // --- Provisioning Category & Criteria Operations ---

    /// Saves a [`ProvisioningCategory`].
    pub async fn save_provisioning_category(
        &self,
        mut category: ProvisioningCategory,
    ) -> FineractResult<ProvisioningCategory> {
        if category.id <= 0 {
            category.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_provisioning_category_sql(self.dialect);
            sqlx::query(&sql)
                .bind(category.id)
                .bind(&category.category_name)
                .bind(&category.category_description)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.categories.insert(category.id, category.clone());
        Ok(category)
    }

    /// Retrieves a [`ProvisioningCategory`] by ID.
    pub async fn get_provisioning_category(
        &self,
        category_id: i64,
    ) -> FineractResult<ProvisioningCategory> {
        if let Some(entry) = self.categories.get(&category_id) {
            return Ok(entry.value().clone());
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_provisioning_category_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(category_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let category_name: String = row
                    .try_get("category_name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let category_description: Option<String> = row
                    .try_get("description")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let cat = ProvisioningCategory::new(id, category_name, category_description);
                self.categories.insert(id, cat.clone());
                return Ok(cat);
            }
        }
        Err(FineractError::resource_not_found(
            "error.msg.provisioning.category.id.invalid",
            format!("Provisioning category with identifier {category_id} does not exist"),
            category_id.to_string(),
        ))
    }

    /// Saves a [`ProvisioningCriteria`] after validating non-overlapping definitions and unique criteria/product mappings.
    pub async fn save_provisioning_criteria(
        &self,
        mut criteria: ProvisioningCriteria,
    ) -> FineractResult<ProvisioningCriteria> {
        criteria.validate()?;
        if criteria.id <= 0 {
            criteria.id = self.allocate_id();
        }

        for item in self.criteria.iter() {
            let existing = item.value();
            if existing.id != criteria.id {
                if existing
                    .criteria_name
                    .eq_ignore_ascii_case(&criteria.criteria_name)
                {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.provisioning.duplicate.criterianame",
                        format!(
                            "Provisioning Criteria with name `{}` already exists",
                            criteria.criteria_name
                        ),
                    )));
                }
                if criteria
                    .loan_product_ids
                    .iter()
                    .any(|pid| existing.loan_product_ids.contains(pid))
                {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.provisioning.product.id(s).already.associated.existing.criteria",
                        "The selected products already associated with another Provisioning Criteria",
                    )));
                }
            }
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_provisioning_criteria_sql(self.dialect);
            sqlx::query(&sql)
                .bind(criteria.id)
                .bind(&criteria.criteria_name)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

            let def_sql = Self::insert_provisioning_criteria_definition_sql(self.dialect);
            for def in &criteria.definitions {
                let def_id = if def.id > 0 {
                    def.id
                } else {
                    self.allocate_id()
                };
                sqlx::query(&def_sql)
                    .bind(def_id)
                    .bind(criteria.id)
                    .bind(def.category_id)
                    .bind(def.min_age)
                    .bind(def.max_age)
                    .bind(def.provisioning_percentage.to_string())
                    .bind(def.liability_account_id)
                    .bind(def.expense_account_id)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }

            let map_sql = Self::insert_loanproduct_provisioning_mapping_sql(self.dialect);
            for &product_id in &criteria.loan_product_ids {
                let map_id = self.allocate_id();
                sqlx::query(&map_sql)
                    .bind(map_id)
                    .bind(product_id)
                    .bind(criteria.id)
                    .execute(pool)
                    .await
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            }
        }

        self.criteria.insert(criteria.id, criteria.clone());
        Ok(criteria)
    }

    /// Retrieves a [`ProvisioningCriteria`] by ID.
    pub async fn get_provisioning_criteria(
        &self,
        criteria_id: i64,
    ) -> FineractResult<ProvisioningCriteria> {
        if let Some(entry) = self.criteria.get(&criteria_id) {
            return Ok(entry.value().clone());
        }
        Err(FineractError::resource_not_found(
            "error.msg.provisioning.criteria.id.invalid",
            format!("Provisioning criteria with identifier {criteria_id} does not exist"),
            criteria_id.to_string(),
        ))
    }
}

// =============================================================================
// 5. COMMAND HANDLERS (`CREATE/UPDATE OFFICE`, `CREATE/UPDATE STAFF`, `CREATE PROVISIONCRITERIA`)
// =============================================================================

/// Command handler for `CREATE OFFICE` (`org.apache.fineract.organisation.office.handler.CreateOfficeCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateOfficeCommandHandler {
    repo: Arc<DualDbOrganisationRepository>,
}

impl CreateOfficeCommandHandler {
    pub fn new(repo: Arc<DualDbOrganisationRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbOrganisationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateOfficeCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        Office::validate_for_create(command)?;
        let parent_id = if command.parameter_exists("parentId") {
            command.long_value_of_parameter_named("parentId")?
        } else {
            Some(1)
        };
        let parent = match parent_id {
            Some(pid) => Some(self.repo.get_office(pid).await?),
            None => None,
        };
        let office_id = self.repo.allocate_id();
        let office = Office::from_json(office_id, parent.as_ref(), command)?;
        let saved = self.repo.save_office(office).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_office_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `UPDATE OFFICE` (`org.apache.fineract.organisation.office.handler.UpdateOfficeCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct UpdateOfficeCommandHandler {
    repo: Arc<DualDbOrganisationRepository>,
}

impl UpdateOfficeCommandHandler {
    pub fn new(repo: Arc<DualDbOrganisationRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbOrganisationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateOfficeCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        Office::validate_for_update(command)?;
        let office_id = command.entity_id.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.office.id.invalid",
                "Office identifier is required for update",
                "null",
            )
        })?;
        let mut office = self.repo.get_office(office_id).await?;

        let new_parent = if command.parameter_exists("parentId") {
            if let Some(pid) = command.long_value_of_parameter_named("parentId")? {
                if pid == office.id {
                    return Err(FineractError::DomainRule(
                        PlatformDomainRuleException::with_args(
                            "error.msg.office.parentId.same.as.id",
                            "Cannot update office with parent same as self.",
                            vec![office.id.into(), pid.into()],
                        ),
                    ));
                }
                Some(self.repo.get_office(pid).await?)
            } else {
                None
            }
        } else {
            None
        };

        let changes = office.update(command, new_parent.as_ref())?;
        if !changes.is_empty() {
            self.repo.update_office(office.clone()).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(office.id)
            .with_office_id(office.id)
            .with_resource_external_id(office.external_id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `CREATE STAFF` (`org.apache.fineract.organisation.staff.handler.StaffCreateCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateStaffCommandHandler {
    repo: Arc<DualDbOrganisationRepository>,
}

/// Alias matching upstream Java `StaffCreateCommandHandler`.
pub type StaffCreateCommandHandler = CreateStaffCommandHandler;

impl CreateStaffCommandHandler {
    pub fn new(repo: Arc<DualDbOrganisationRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbOrganisationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateStaffCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let staff_id = self.repo.allocate_id();
        let staff = Staff::from_json(staff_id, command)?;
        let saved = self.repo.save_staff(staff).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .with_office_id(saved.office_id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `UPDATE STAFF` (`org.apache.fineract.organisation.staff.handler.StaffUpdateCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct UpdateStaffCommandHandler {
    repo: Arc<DualDbOrganisationRepository>,
}

/// Alias matching upstream Java `StaffUpdateCommandHandler`.
pub type StaffUpdateCommandHandler = UpdateStaffCommandHandler;

impl UpdateStaffCommandHandler {
    pub fn new(repo: Arc<DualDbOrganisationRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbOrganisationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for UpdateStaffCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let staff_id = command.entity_id.ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.staff.id.invalid",
                "Staff identifier is required for update",
                "null",
            )
        })?;
        let mut staff = self.repo.get_staff(staff_id).await?;
        let changes = staff.update(command)?;
        if !changes.is_empty() {
            staff = self.repo.update_staff(staff).await?;
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(staff.id)
            .with_office_id(staff.office_id)
            .with_resource_external_id(staff.external_id)
            .with_changes(changes)
            .build())
    }
}

/// Command handler for `CREATE PROVISIONCRITERIA`
/// (`org.apache.fineract.organisation.provisioning.handler.CreateProvisioningCriteriaRequestCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateProvisioningCriteriaCommandHandler {
    repo: Arc<DualDbOrganisationRepository>,
}

/// Alias matching upstream Java `CreateProvisioningCriteriaRequestCommandHandler`.
pub type CreateProvisioningCriteriaRequestCommandHandler = CreateProvisioningCriteriaCommandHandler;

impl CreateProvisioningCriteriaCommandHandler {
    pub fn new(repo: Arc<DualDbOrganisationRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> &Arc<DualDbOrganisationRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateProvisioningCriteriaCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let criteria_id = self.repo.allocate_id();
        let criteria = ProvisioningCriteria::from_json(criteria_id, command)?;
        let saved = self.repo.save_provisioning_criteria(criteria).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_entity_id(saved.id)
            .build())
    }
}

// =============================================================================
// 6. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use serde_json::json;

    fn sample_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2020, 1, 1).expect("valid date")
    }

    #[test]
    fn test_office_materialized_hierarchy_paths() {
        let mut head = Office::head_office(1, "Head Office", sample_date(), ExternalId::empty());
        head.generate_hierarchy(None);
        assert_eq!(head.hierarchy, ".1.");
        assert!(!head.does_not_have_an_office_in_hierarchy_with_id(1));
        assert!(head.does_not_have_an_office_in_hierarchy_with_id(2));

        let mut regional = Office::new(
            2,
            Some(head.id),
            "Regional Office",
            sample_date(),
            ExternalId::new("REG-01"),
        );
        regional.generate_hierarchy(Some(&head.hierarchy));
        assert_eq!(regional.hierarchy, ".1.2.");
        assert!(head.is_parent_of(&regional));
        assert!(!regional.is_parent_of(&head));
        assert!(!regional.does_not_have_an_office_in_hierarchy_with_id(1));
        assert!(!regional.does_not_have_an_office_in_hierarchy_with_id(2));
        assert!(regional.does_not_have_an_office_in_hierarchy_with_id(5));

        let mut branch = Office::new(
            5,
            Some(regional.id),
            "Sub-Branch 5",
            sample_date(),
            ExternalId::new("BR-05"),
        );
        branch.generate_hierarchy(Some(&regional.hierarchy));
        assert_eq!(branch.hierarchy, ".1.2.5.");
        assert!(head.is_parent_of(&branch));
        assert!(regional.is_parent_of(&branch));
        assert!(!branch.is_parent_of(&regional));
        assert!(!branch.does_not_have_an_office_in_hierarchy_with_id(1));
        assert!(!branch.does_not_have_an_office_in_hierarchy_with_id(2));
        assert!(!branch.does_not_have_an_office_in_hierarchy_with_id(5));
        assert!(branch.does_not_have_an_office_in_hierarchy_with_id(99));
    }

    #[test]
    fn test_office_parent_cycle_prevention() {
        let mut head = Office::head_office(1, "Head Office", sample_date(), ExternalId::empty());
        head.generate_hierarchy(None);

        let mut regional = Office::new(
            2,
            Some(1),
            "Regional Office",
            sample_date(),
            ExternalId::empty(),
        );
        regional.generate_hierarchy(Some(&head.hierarchy));

        let mut branch = Office::new(
            5,
            Some(2),
            "District Branch",
            sample_date(),
            ExternalId::empty(),
        );
        branch.generate_hierarchy(Some(&regional.hierarchy));

        // 1. Self-parenting is rejected
        let self_err = regional
            .update_parent(&regional.clone())
            .expect_err("self-parenting must fail");
        assert!(matches!(self_err, FineractError::DomainRule(_)));
        assert_eq!(self_err.http_status_code(), 403);

        // 2. Assigning direct child or deeper descendant as parent is rejected (cycle prevention)
        assert!(regional.would_create_cycle(&branch));
        let cycle_err = regional
            .update_parent(&branch)
            .expect_err("assigning descendant as parent must fail");
        assert!(matches!(cycle_err, FineractError::DomainRule(_)));

        // 3. Root office parent cannot be updated
        let mut peer_root = Office::head_office(3, "Other Office", sample_date(), ExternalId::empty());
        peer_root.generate_hierarchy(None);
        let root_err = head
            .update_parent(&peer_root)
            .expect_err("root office parent cannot be updated");
        assert!(matches!(root_err, FineractError::DomainRule(_)));

        // 4. Valid re-parenting to a non-descendant sibling office succeeds
        let mut sibling_regional = Office::new(
            3,
            Some(1),
            "Sibling Regional",
            sample_date(),
            ExternalId::empty(),
        );
        sibling_regional.generate_hierarchy(Some(&head.hierarchy));
        assert_eq!(sibling_regional.hierarchy, ".1.3.");

        branch
            .update_parent(&sibling_regional)
            .expect("re-parenting to non-descendant sibling should succeed");
        assert_eq!(branch.parent_id, Some(3));
        assert_eq!(branch.hierarchy, ".1.3.5.");
    }

    #[test]
    fn test_staff_derive_display_name_formatting() {
        let mut staff = Staff {
            id: 10,
            office_id: 1,
            firstname: "John".to_string(),
            lastname: "Doe".to_string(),
            display_name: String::new(),
            is_loan_officer: true,
            is_active: true,
            joining_date: Some(sample_date()),
            mobile_no: Some("+15550100".to_string()),
            external_id: ExternalId::new("STF-10"),
            organisational_role_type: Some(StaffOrganisationalRoleType::FieldOfficer.as_i16()),
            organisational_role_parent_staff_id: None,
        };

        let formatted = staff.derive_display_name();
        assert_eq!(formatted, "Doe, John");
        assert_eq!(staff.display_name, "Doe, John");

        // Updating firstname/lastname via JsonCommand updates display_name automatically
        let update_cmd = JsonCommand::from_value(
            Some(10),
            json!({
                "firstname": "Jane",
                "lastname": "Smith"
            }),
        );
        let changes = staff.update(&update_cmd).expect("staff update should succeed");
        assert!(changes.contains_key("firstname"));
        assert!(changes.contains_key("lastname"));
        assert_eq!(staff.display_name, "Smith, Jane");
    }

    #[test]
    fn test_provisioning_criteria_non_overlapping_age_buckets() {
        let valid_defs = vec![
            ProvisioningCriteriaDefinition::new(1, 1, 0, 30, dec!(1.00), 101, 201),
            ProvisioningCriteriaDefinition::new(2, 2, 31, 60, dec!(10.00), 101, 201),
            ProvisioningCriteriaDefinition::new(3, 3, 61, 90, dec!(50.00), 101, 201),
            ProvisioningCriteriaDefinition::new(4, 4, 91, 9999, dec!(100.00), 101, 201),
        ];

        let criteria = ProvisioningCriteria::new(
            1,
            "Standard NBFC Criteria",
            valid_defs,
            vec![1, 2],
        )
        .expect("disjoint contiguous buckets 0..=30, 31..=60, 61..=90, 91..=9999 must pass");
        assert_eq!(
            criteria
                .definition_for_age(45)
                .map(|d| d.provisioning_percentage),
            Some(dec!(10.00))
        );

        // Touching endpoint (0..=30 and 30..=60 overlap at day 30!)
        let touching_overlap_defs = vec![
            ProvisioningCriteriaDefinition::new(1, 1, 0, 30, dec!(1.00), 101, 201),
            ProvisioningCriteriaDefinition::new(2, 2, 30, 60, dec!(10.00), 101, 201),
        ];
        let err = ProvisioningCriteria::new(
            2,
            "Overlapping Boundary Criteria",
            touching_overlap_defs,
            vec![1],
        )
        .expect_err("buckets 0..=30 and 30..=60 overlap on day 30 and must fail");
        match err {
            FineractError::DomainRule(ex) => {
                assert_eq!(
                    ex.globalisation_message_code,
                    "error.msg.provisioning.criteria.definitions.are.overlapping"
                );
            }
            other => panic!("expected DomainRule error, got {other:?}"),
        }

        // Unordered interior overlap (61..=120 and 1..=90)
        let interior_overlap_defs = vec![
            ProvisioningCriteriaDefinition::new(1, 3, 61, 120, dec!(50.00), 101, 201),
            ProvisioningCriteriaDefinition::new(2, 1, 0, 30, dec!(1.00), 101, 201),
            ProvisioningCriteriaDefinition::new(3, 2, 25, 70, dec!(25.00), 101, 201),
        ];
        assert!(ProvisioningCriteria::validate_definitions(&interior_overlap_defs).is_err());

        // Invalid bucket where max_age <= min_age
        let inverted_defs = vec![ProvisioningCriteriaDefinition::new(
            1,
            1,
            30,
            30,
            dec!(5.00),
            101,
            201,
        )];
        let inv_err = ProvisioningCriteria::validate_definitions(&inverted_defs)
            .expect_err("max_age <= min_age must fail data validation");
        assert!(matches!(inv_err, FineractError::DataValidation(_)));
    }

    #[tokio::test]
    async fn test_command_handlers_and_dual_db_repository() {
        let repo = Arc::new(DualDbOrganisationRepository::new_in_memory());
        let ctx = ExecutionContext::new("default", sample_date());

        // 1. CreateOfficeCommandHandler creates child office under Head Office (1) -> ".1.2."
        let create_office_handler = CreateOfficeCommandHandler::new(Arc::clone(&repo));
        let create_office_cmd = JsonCommand::from_value(
            None,
            json!({
                "name": "Northern Region",
                "openingDate": "01 January 2020",
                "dateFormat": "dd MMMM yyyy",
                "locale": "en",
                "parentId": 1,
                "externalId": "NORTH-01"
            }),
        );
        let office_res = create_office_handler
            .handle(&ctx, &create_office_cmd)
            .await
            .expect("create office should succeed");
        let north_id = office_res.resource_id.expect("office resource_id");
        let north = repo.get_office(north_id).await.expect("fetch created office");
        assert_eq!(north.hierarchy, format!(".1.{north_id}."));

        // 2. Create sub-branch under Northern Region -> ".1.2.3."
        let create_sub_cmd = JsonCommand::from_value(
            None,
            json!({
                "name": "Winterfell Branch",
                "openingDate": "15 February 2020",
                "dateFormat": "dd MMMM yyyy",
                "locale": "en",
                "parentId": north_id,
                "externalId": "WF-01"
            }),
        );
        let sub_res = create_office_handler
            .handle(&ctx, &create_sub_cmd)
            .await
            .expect("create sub-branch should succeed");
        let sub_id = sub_res.resource_id.expect("sub-branch resource_id");
        let sub = repo.get_office(sub_id).await.expect("fetch sub-branch");
        assert_eq!(sub.hierarchy, format!(".1.{north_id}.{sub_id}."));

        // 3. UpdateOfficeCommandHandler rejects cycle when trying to set Northern Region's parent to Winterfell Branch
        let update_office_handler = UpdateOfficeCommandHandler::new(Arc::clone(&repo));
        let cycle_cmd = JsonCommand::from_value(
            Some(north_id),
            json!({
                "parentId": sub_id
            }),
        );
        let cycle_res = update_office_handler.handle(&ctx, &cycle_cmd).await;
        assert!(cycle_res.is_err(), "setting descendant as parent must fail");

        // 4. CreateStaffCommandHandler & UpdateStaffCommandHandler
        let create_staff_handler = CreateStaffCommandHandler::new(Arc::clone(&repo));
        let create_staff_cmd = JsonCommand::from_value(
            None,
            json!({
                "officeId": sub_id,
                "firstname": "Eddard",
                "lastname": "Stark",
                "isLoanOfficer": true,
                "isActive": true,
                "joiningDate": "01 March 2020",
                "dateFormat": "dd MMMM yyyy",
                "locale": "en",
                "externalId": "STF-NED"
            }),
        );
        let staff_res = create_staff_handler
            .handle(&ctx, &create_staff_cmd)
            .await
            .expect("create staff should succeed");
        let staff_id = staff_res.resource_id.expect("staff resource_id");
        let staff = repo.get_staff(staff_id).await.expect("fetch created staff");
        assert_eq!(staff.display_name, "Stark, Eddard");

        let update_staff_handler = UpdateStaffCommandHandler::new(Arc::clone(&repo));
        let update_staff_cmd = JsonCommand::from_value(
            Some(staff_id),
            json!({
                "firstname": "Robb"
            }),
        );
        update_staff_handler
            .handle(&ctx, &update_staff_cmd)
            .await
            .expect("update staff should succeed");
        let updated_staff = repo.get_staff(staff_id).await.expect("fetch updated staff");
        assert_eq!(updated_staff.display_name, "Stark, Robb");

        // 5. CreateProvisioningCriteriaCommandHandler
        let create_criteria_handler = CreateProvisioningCriteriaCommandHandler::new(Arc::clone(&repo));
        let create_criteria_cmd = JsonCommand::from_value(
            None,
            json!({
                "criteriaName": "Microfinance Standard Criteria",
                "locale": "en",
                "loanProducts": [{"id": 10}, {"id": 20}],
                "definitions": [
                    {
                        "categoryId": 1,
                        "minAge": 0,
                        "maxAge": 30,
                        "provisioningPercentage": "1.0",
                        "liabilityAccount": 100,
                        "expenseAccount": 200
                    },
                    {
                        "categoryId": 2,
                        "minAge": 31,
                        "maxAge": 90,
                        "provisioningPercentage": "25.5",
                        "liabilityAccount": 100,
                        "expenseAccount": 200
                    }
                ]
            }),
        );
        let criteria_res = create_criteria_handler
            .handle(&ctx, &create_criteria_cmd)
            .await
            .expect("create provisioning criteria should succeed");
        let criteria_id = criteria_res.resource_id.expect("criteria resource_id");
        let saved_criteria = repo
            .get_provisioning_criteria(criteria_id)
            .await
            .expect("fetch saved criteria");
        assert_eq!(saved_criteria.definitions.len(), 2);
        assert_eq!(saved_criteria.loan_product_ids, vec![10, 20]);

        // Dialect SQL placeholder verification
        assert!(
            DualDbOrganisationRepository::insert_office_sql(SqlDialect::Postgres).contains("$1")
        );
        assert!(
            DualDbOrganisationRepository::insert_office_sql(SqlDialect::MySql).contains('?')
        );
    }
}
