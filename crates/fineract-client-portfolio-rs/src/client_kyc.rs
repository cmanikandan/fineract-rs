

//! Client KYC, Identifiers, Family Members, and Address domain models, state machine,
//! Dual-DB repository (`m_client`, `m_client_non_person`, `m_client_identifier`,
//! `m_address`, `m_client_address`, `m_family_members`), and command handlers.
//!
//! Ported from:
//! - `org.apache.fineract.portfolio.client.domain.{Client, ClientStatus, LegalForm, ClientIdentifier, ClientIdentifierStatus, ClientAddress, ClientNonPerson, ClientFamilyMembers}`
//! - `org.apache.fineract.portfolio.client.data.{ClientData, ClientDataValidator}`
//! - `org.apache.fineract.portfolio.client.service.{ClientWritePlatformServiceJpaRepositoryImpl, ClientIdentifierWritePlatformServiceJpaRepositoryImpl, ClientFamilyMembersWritePlatformServiceImpl}`
//! - `org.apache.fineract.portfolio.address.domain.Address`
//! - `org.apache.fineract.portfolio.address.service.AddressWritePlatformServiceImpl`
//! - `org.apache.fineract.portfolio.transfer.service.TransferWritePlatformServiceJpaRepositoryImpl`

fn parse_date_str(s: &str) -> Option<NaiveDate> {
    let part = s.split(|c: char| c == ' ' || c == 'T').next().unwrap_or(s);
    NaiveDate::parse_from_str(part, "%Y-%m-%d").ok()
}

use std::collections::BTreeMap;
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
    get_business_date, try_current_execution_context, DialectSql, ExecutionContext, ExternalId,
    JsonCommand, SqlDialect,
};
use fineract_validation_rs::{
    ApiParameterError, DataValidatorBuilder, FineractError, FineractResult,
    PlatformApiDataValidationException, PlatformDomainRuleException,
};

// =============================================================================
// 1. ENUMERATIONS (`ClientStatus`, `LegalForm`, `ClientIdentifierStatus`)
// =============================================================================

/// Client lifecycle status enumeration (`m_client.status_enum`).
///
/// Ported from `org.apache.fineract.portfolio.client.domain.ClientStatus`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum ClientStatus {
    #[default]
    Invalid = 0,
    Pending = 100,
    Active = 300,
    TransferInProgress = 303,
    TransferOnHold = 304,
    Withdrawn = 400,
    Rejected = 500,
    Closed = 600,
    Reopened = 700,
}

impl ClientStatus {
    pub const INVALID: Self = Self::Invalid;
    pub const PENDING: Self = Self::Pending;
    pub const ACTIVE: Self = Self::Active;
    pub const TRANSFER_IN_PROGRESS: Self = Self::TransferInProgress;
    pub const TRANSFER_ON_HOLD: Self = Self::TransferOnHold;
    pub const WITHDRAWN: Self = Self::Withdrawn;
    pub const REJECTED: Self = Self::Rejected;
    pub const CLOSED: Self = Self::Closed;
    pub const REOPENED: Self = Self::Reopened;

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn as_int(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "clientStatusType.invalid",
            Self::Pending => "clientStatusType.pending",
            Self::Active => "clientStatusType.active",
            Self::TransferInProgress => "clientStatusType.transfer.in.progress",
            Self::TransferOnHold => "clientStatusType.transfer.on.hold",
            Self::Withdrawn => "clientStatusType.withdraw",
            Self::Rejected => "clientStatusType.rejected",
            Self::Closed => "clientStatusType.closed",
            Self::Reopened => "clientStatusType.reopened",
        }
    }

    pub const fn from_i16(status_value: i16) -> Self {
        match status_value {
            100 => Self::Pending,
            300 => Self::Active,
            303 => Self::TransferInProgress,
            304 => Self::TransferOnHold,
            400 | 800 => Self::Withdrawn,
            500 => Self::Rejected,
            600 => Self::Closed,
            700 => Self::Reopened,
            _ => Self::Invalid,
        }
    }

    pub const fn from_i32(status_value: i32) -> Self {
        match status_value {
            100 => Self::Pending,
            300 => Self::Active,
            303 => Self::TransferInProgress,
            304 => Self::TransferOnHold,
            400 | 800 => Self::Withdrawn,
            500 => Self::Rejected,
            600 => Self::Closed,
            700 => Self::Reopened,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(status_value: Option<i32>) -> Self {
        match status_value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    pub fn from_string(client_string: &str) -> Self {
        let trimmed = client_string.trim();
        if trimmed.is_empty() {
            return Self::Invalid;
        }
        if trimmed.eq_ignore_ascii_case("PENDING") {
            Self::Pending
        } else if trimmed.eq_ignore_ascii_case("ACTIVE") {
            Self::Active
        } else if trimmed.eq_ignore_ascii_case("TRANSFER_IN_PROGRESS")
            || trimmed.eq_ignore_ascii_case("TRANSFERINPROGRESS")
        {
            Self::TransferInProgress
        } else if trimmed.eq_ignore_ascii_case("TRANSFER_ON_HOLD")
            || trimmed.eq_ignore_ascii_case("TRANSFERONHOLD")
        {
            Self::TransferOnHold
        } else if trimmed.eq_ignore_ascii_case("WITHDRAWN") {
            Self::Withdrawn
        } else if trimmed.eq_ignore_ascii_case("REJECTED") {
            Self::Rejected
        } else if trimmed.eq_ignore_ascii_case("CLOSED") {
            Self::Closed
        } else if trimmed.eq_ignore_ascii_case("REOPENED") {
            Self::Reopened
        } else {
            Self::Invalid
        }
    }

    pub const fn has_state_of(self, state: Self) -> bool {
        (self as i16) == (state as i16)
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
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

    pub const fn is_withdrawn(self) -> bool {
        matches!(self, Self::Withdrawn)
    }

    pub const fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }

    pub const fn is_reopened(self) -> bool {
        matches!(self, Self::Reopened)
    }
}

/// Client legal form enumeration (`m_client.legal_form_enum`).
///
/// Ported from `org.apache.fineract.portfolio.client.domain.LegalForm`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum LegalForm {
    #[default]
    Person = 1,
    Entity = 2,
}

impl LegalForm {
    pub const PERSON: Self = Self::Person;
    pub const ENTITY: Self = Self::Entity;

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn as_int(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Person => "legalFormType.person",
            Self::Entity => "legalFormType.entity",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Person => "Person",
            Self::Entity => "Entity",
        }
    }

    pub const fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::Person),
            2 => Some(Self::Entity),
            _ => None,
        }
    }

    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Person),
            2 => Some(Self::Entity),
            _ => None,
        }
    }

    pub const fn from_int(value: i32) -> Option<Self> {
        Self::from_i32(value)
    }

    pub const fn is_person(self) -> bool {
        matches!(self, Self::Person)
    }

    pub const fn is_entity(self) -> bool {
        matches!(self, Self::Entity)
    }
}

impl std::fmt::Display for LegalForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Client identifier status enumeration (`m_client_identifier.status`).
///
/// Ported from `org.apache.fineract.portfolio.client.domain.ClientIdentifierStatus`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[repr(i16)]
pub enum ClientIdentifierStatus {
    Invalid = 0,
    Inactive = 100,
    #[default]
    Active = 200,
}

impl ClientIdentifierStatus {
    pub const INVALID: Self = Self::Invalid;
    pub const INACTIVE: Self = Self::Inactive;
    pub const ACTIVE: Self = Self::Active;

    pub const fn as_i16(self) -> i16 {
        self as i16
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Inactive => "clientIdentifierStatusType.inactive",
            Self::Active => "clientIdentifierStatusType.active",
            Self::Invalid => "clientIdentifierStatusType.invalid",
        }
    }

    pub const fn from_i16(value: i16) -> Self {
        match value {
            100 => Self::Inactive,
            200 => Self::Active,
            _ => Self::Invalid,
        }
    }

    pub const fn from_i32(value: i32) -> Self {
        match value {
            100 => Self::Inactive,
            200 => Self::Active,
            _ => Self::Invalid,
        }
    }

    pub const fn from_int(value: Option<i32>) -> Self {
        match value {
            Some(v) => Self::from_i32(v),
            None => Self::Invalid,
        }
    }

    pub fn from_str_flexible(raw: &str) -> Self {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("ACTIVE") {
            Self::Active
        } else if trimmed.eq_ignore_ascii_case("INACTIVE") {
            Self::Inactive
        } else {
            Self::Invalid
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub const fn is_inactive(self) -> bool {
        matches!(self, Self::Inactive)
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    /// Returns `Some(200)` when active, `None` when inactive, matching `m_client_identifier.active`.
    pub const fn active_column_value(self) -> Option<i32> {
        if self.is_active() {
            Some(self as i32)
        } else {
            None
        }
    }
}

// =============================================================================
// 2. CLIENT DOMAIN & DATA (`m_client`)
// =============================================================================

/// Supported JSON parameters for Client create and update commands.
pub const CLIENT_SUPPORTED_PARAMETERS: &[&str] = &[
    "id",
    "groupId",
    "accountNo",
    "externalId",
    "mobileNo",
    "emailAddress",
    "firstname",
    "middlename",
    "lastname",
    "fullname",
    "displayname",
    "displayName",
    "officeId",
    "transferToOfficeId",
    "active",
    "activationDate",
    "reactivationDate",
    "reopenedDate",
    "closureDate",
    "closureReasonId",
    "rejectionDate",
    "rejectionReasonId",
    "withdrawalDate",
    "withdrawalReasonId",
    "staffId",
    "isStaff",
    "submittedOnDate",
    "savingsProductId",
    "savingsAccountId",
    "dateOfBirth",
    "genderId",
    "clientTypeId",
    "clientClassificationId",
    "legalFormId",
    "clientNonPersonDetails",
    "address",
    "familyMembers",
    "datatables",
    "locale",
    "dateFormat",
];

/// Domain aggregate representing a Client (`m_client`).
///
/// Ported from `org.apache.fineract.portfolio.client.domain.Client`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: i64,
    pub account_no: String,
    pub external_id: ExternalId,
    pub status: ClientStatus,
    pub sub_status: Option<i64>,
    pub activation_date: Option<NaiveDate>,
    pub office_joining_date: Option<NaiveDate>,
    pub office_id: i64,
    pub transfer_to_office_id: Option<i64>,
    pub staff_id: Option<i64>,
    pub firstname: Option<String>,
    pub middlename: Option<String>,
    pub lastname: Option<String>,
    pub fullname: Option<String>,
    pub display_name: String,
    pub mobile_no: Option<String>,
    pub email_address: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender_cv_id: Option<i64>,
    pub client_type_cv_id: Option<i64>,
    pub client_classification_cv_id: Option<i64>,
    pub legal_form: LegalForm,
    pub submitted_on_date: Option<NaiveDate>,
    pub closed_on_date: Option<NaiveDate>,
    pub closure_reason_cv_id: Option<i64>,
    pub rejected_on_date: Option<NaiveDate>,
    pub reject_reason_cv_id: Option<i64>,
    pub withdrawn_on_date: Option<NaiveDate>,
    pub withdraw_reason_cv_id: Option<i64>,
    pub reactivated_on_date: Option<NaiveDate>,
    pub reopened_on_date: Option<NaiveDate>,
    pub proposed_transfer_date: Option<NaiveDate>,
    pub savings_product_id: Option<i64>,
    pub savings_account_id: Option<i64>,
    pub is_staff: bool,
    pub image_id: Option<i64>,
}

/// Immutable DTO representation of a [`Client`] (`m_client`).
///
/// Ported from `org.apache.fineract.portfolio.client.data.ClientData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientData {
    pub id: i64,
    pub account_no: String,
    pub external_id: ExternalId,
    pub status: ClientStatus,
    pub sub_status: Option<i64>,
    pub activation_date: Option<NaiveDate>,
    pub office_joining_date: Option<NaiveDate>,
    pub office_id: i64,
    pub transfer_to_office_id: Option<i64>,
    pub staff_id: Option<i64>,
    pub firstname: Option<String>,
    pub middlename: Option<String>,
    pub lastname: Option<String>,
    pub fullname: Option<String>,
    pub display_name: String,
    pub mobile_no: Option<String>,
    pub email_address: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender_cv_id: Option<i64>,
    pub client_type_cv_id: Option<i64>,
    pub client_classification_cv_id: Option<i64>,
    pub legal_form: LegalForm,
    pub submitted_on_date: Option<NaiveDate>,
    pub closed_on_date: Option<NaiveDate>,
    pub closure_reason_cv_id: Option<i64>,
    pub rejected_on_date: Option<NaiveDate>,
    pub reject_reason_cv_id: Option<i64>,
    pub withdrawn_on_date: Option<NaiveDate>,
    pub withdraw_reason_cv_id: Option<i64>,
    pub reactivated_on_date: Option<NaiveDate>,
    pub reopened_on_date: Option<NaiveDate>,
    pub proposed_transfer_date: Option<NaiveDate>,
    pub savings_product_id: Option<i64>,
    pub savings_account_id: Option<i64>,
    pub is_staff: bool,
    pub image_id: Option<i64>,
}

impl From<&Client> for ClientData {
    fn from(client: &Client) -> Self {
        Self {
            id: client.id,
            account_no: client.account_no.clone(),
            external_id: client.external_id.clone(),
            status: client.status,
            sub_status: client.sub_status,
            activation_date: client.activation_date,
            office_joining_date: client.office_joining_date,
            office_id: client.office_id,
            transfer_to_office_id: client.transfer_to_office_id,
            staff_id: client.staff_id,
            firstname: client.firstname.clone(),
            middlename: client.middlename.clone(),
            lastname: client.lastname.clone(),
            fullname: client.fullname.clone(),
            display_name: client.display_name.clone(),
            mobile_no: client.mobile_no.clone(),
            email_address: client.email_address.clone(),
            date_of_birth: client.date_of_birth,
            gender_cv_id: client.gender_cv_id,
            client_type_cv_id: client.client_type_cv_id,
            client_classification_cv_id: client.client_classification_cv_id,
            legal_form: client.legal_form,
            submitted_on_date: client.submitted_on_date,
            closed_on_date: client.closed_on_date,
            closure_reason_cv_id: client.closure_reason_cv_id,
            rejected_on_date: client.rejected_on_date,
            reject_reason_cv_id: client.reject_reason_cv_id,
            withdrawn_on_date: client.withdrawn_on_date,
            withdraw_reason_cv_id: client.withdraw_reason_cv_id,
            reactivated_on_date: client.reactivated_on_date,
            reopened_on_date: client.reopened_on_date,
            proposed_transfer_date: client.proposed_transfer_date,
            savings_product_id: client.savings_product_id,
            savings_account_id: client.savings_account_id,
            is_staff: client.is_staff,
            image_id: client.image_id,
        }
    }
}

impl From<Client> for ClientData {
    fn from(client: Client) -> Self {
        Self::from(&client)
    }
}

impl From<ClientData> for Client {
    fn from(data: ClientData) -> Self {
        Self {
            id: data.id,
            account_no: data.account_no,
            external_id: data.external_id,
            status: data.status,
            sub_status: data.sub_status,
            activation_date: data.activation_date,
            office_joining_date: data.office_joining_date,
            office_id: data.office_id,
            transfer_to_office_id: data.transfer_to_office_id,
            staff_id: data.staff_id,
            firstname: data.firstname,
            middlename: data.middlename,
            lastname: data.lastname,
            fullname: data.fullname,
            display_name: data.display_name,
            mobile_no: data.mobile_no,
            email_address: data.email_address,
            date_of_birth: data.date_of_birth,
            gender_cv_id: data.gender_cv_id,
            client_type_cv_id: data.client_type_cv_id,
            client_classification_cv_id: data.client_classification_cv_id,
            legal_form: data.legal_form,
            submitted_on_date: data.submitted_on_date,
            closed_on_date: data.closed_on_date,
            closure_reason_cv_id: data.closure_reason_cv_id,
            rejected_on_date: data.rejected_on_date,
            reject_reason_cv_id: data.reject_reason_cv_id,
            withdrawn_on_date: data.withdrawn_on_date,
            withdraw_reason_cv_id: data.withdraw_reason_cv_id,
            reactivated_on_date: data.reactivated_on_date,
            reopened_on_date: data.reopened_on_date,
            proposed_transfer_date: data.proposed_transfer_date,
            savings_product_id: data.savings_product_id,
            savings_account_id: data.savings_account_id,
            is_staff: data.is_staff,
            image_id: data.image_id,
        }
    }
}

fn normalize_opt_string(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn current_business_date_opt() -> Option<NaiveDate> {
    if try_current_execution_context().is_some() {
        Some(get_business_date())
    } else {
        None
    }
}

impl Client {
    /// Formats a 9-digit zero-padded Fineract account number from a client ID.
    pub fn format_account_no(id: i64) -> String {
        format!("{:09}", id.max(0))
    }

    /// Creates a new [`Client`] for a natural person (`LegalForm::Person`) in `Pending` status.
    pub fn new_person(
        id: i64,
        office_id: i64,
        firstname: impl Into<String>,
        middlename: Option<String>,
        lastname: impl Into<String>,
        submitted_on_date: NaiveDate,
        external_id: ExternalId,
    ) -> Self {
        let mut client = Self {
            id,
            account_no: if id > 0 {
                Self::format_account_no(id)
            } else {
                String::new()
            },
            external_id,
            status: ClientStatus::Pending,
            office_id,
            firstname: normalize_opt_string(Some(firstname.into())),
            middlename: normalize_opt_string(middlename),
            lastname: normalize_opt_string(Some(lastname.into())),
            legal_form: LegalForm::Person,
            submitted_on_date: Some(submitted_on_date),
            ..Default::default()
        };
        client.derive_display_name();
        client
    }

    /// Creates a new [`Client`] for a corporate / non-person entity (`LegalForm::Entity`) in `Pending` status.
    pub fn new_entity(
        id: i64,
        office_id: i64,
        fullname: impl Into<String>,
        submitted_on_date: NaiveDate,
        external_id: ExternalId,
    ) -> Self {
        let mut client = Self {
            id,
            account_no: if id > 0 {
                Self::format_account_no(id)
            } else {
                String::new()
            },
            external_id,
            status: ClientStatus::Pending,
            office_id,
            fullname: normalize_opt_string(Some(fullname.into())),
            legal_form: LegalForm::Entity,
            submitted_on_date: Some(submitted_on_date),
            ..Default::default()
        };
        client.derive_display_name();
        client
    }

    /// Derives `self.display_name` from `fullname` or `(firstname, middlename, lastname)`,
    /// matching `Client.deriveDisplayName()` in Fineract.
    pub fn derive_display_name(&mut self) -> String {
        if let Some(ref full) = self.fullname {
            let trimmed = full.trim();
            if !trimmed.is_empty() {
                self.display_name = trimmed.to_string();
                return self.display_name.clone();
            }
        }

        let mut parts = Vec::with_capacity(3);
        if self.legal_form.is_person() {
            if let Some(ref first) = self.firstname {
                let t = first.trim();
                if !t.is_empty() {
                    parts.push(t);
                }
            }
            if let Some(ref middle) = self.middlename {
                let t = middle.trim();
                if !t.is_empty() {
                    parts.push(t);
                }
            }
            if let Some(ref last) = self.lastname {
                let t = last.trim();
                if !t.is_empty() {
                    parts.push(t);
                }
            }
        }
        self.display_name = parts.join(" ");
        self.display_name.clone()
    }

    /// Resets derived name components when switching `LegalForm`, matching `Client.resetDerivedNames`.
    pub fn reset_derived_names(&mut self, legal_form: LegalForm) {
        if legal_form.is_person() {
            self.fullname = None;
        } else if legal_form.is_entity() {
            self.firstname = None;
            self.middlename = None;
            self.lastname = None;
            self.display_name.clear();
        }
    }

    pub const fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub const fn is_not_active(&self) -> bool {
        !self.is_active()
    }

    pub const fn is_pending(&self) -> bool {
        matches!(self.status, ClientStatus::Pending | ClientStatus::Reopened)
    }

    pub const fn is_not_pending(&self) -> bool {
        !self.is_pending()
    }

    pub const fn is_closed(&self) -> bool {
        self.status.is_closed()
    }

    pub const fn is_rejected(&self) -> bool {
        self.status.is_rejected()
    }

    pub const fn is_withdrawn(&self) -> bool {
        self.status.is_withdrawn()
    }

    pub const fn is_transfer_in_progress(&self) -> bool {
        self.status.is_transfer_in_progress()
    }

    pub const fn is_transfer_on_hold(&self) -> bool {
        self.status.is_transfer_on_hold()
    }

    pub const fn is_transfer_in_progress_or_on_hold(&self) -> bool {
        self.status.is_under_transfer()
    }

    pub const fn identified_by(&self, client_id: i64) -> bool {
        self.id == client_id
    }

    pub fn update_account_no(&mut self, account_identifier: impl Into<String>) {
        self.account_no = account_identifier.into();
    }

    /// Validates mutually exclusive name parts (`fullname` vs `firstname`/`middlename`/`lastname`),
    /// matching `Client.validateNameParts`.
    pub fn validate_name_parts(&self) -> FineractResult<()> {
        let has_fullname = self
            .fullname
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_first = self
            .firstname
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_middle = self
            .middlename
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_last = self
            .lastname
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);

        let mut errors = Vec::new();
        if has_fullname {
            if has_first {
                errors.push(ApiParameterError::parameter_error(
                    "validation.msg.client.firstname.must.be.blank.when.fullname.is.provided",
                    "The parameter `firstname` must be blank when `fullname` is provided.",
                    "firstname",
                    vec![],
                ));
            }
            if has_middle {
                errors.push(ApiParameterError::parameter_error(
                    "validation.msg.client.middlename.must.be.blank.when.fullname.is.provided",
                    "The parameter `middlename` must be blank when `fullname` is provided.",
                    "middlename",
                    vec![],
                ));
            }
            if has_last {
                errors.push(ApiParameterError::parameter_error(
                    "validation.msg.client.lastname.must.be.blank.when.fullname.is.provided",
                    "The parameter `lastname` must be blank when `fullname` is provided.",
                    "lastname",
                    vec![],
                ));
            }
            if let Some(ref full) = self.fullname {
                if full.len() > 160 {
                    errors.push(ApiParameterError::parameter_error(
                        "validation.msg.client.fullname.exceeds.max.length",
                        "The parameter `fullname` exceeds max length of 160.",
                        "fullname",
                        vec![],
                    ));
                }
            }
        } else {
            let validator = DataValidatorBuilder::for_resource("client");
            validator
                .reset()
                .parameter("firstname")
                .value(self.firstname.clone())
                .not_blank()
                .not_exceeding_length_of(50);
            validator
                .reset()
                .parameter("middlename")
                .value(self.middlename.clone())
                .ignore_if_null()
                .not_exceeding_length_of(50);
            validator
                .reset()
                .parameter("lastname")
                .value(self.lastname.clone())
                .not_blank()
                .not_exceeding_length_of(50);
            errors.extend(validator.data_validation_errors());
        }

        if !errors.is_empty() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(errors),
            ));
        }
        Ok(())
    }

    /// Validates `submitted_on_date`, `reopened_on_date`, and `activation_date` ordering,
    /// matching `Client.validateActivationDate`.
    pub fn validate_activation_date(&self) -> FineractResult<()> {
        let mut errors = Vec::new();
        let biz_date = current_business_date_opt();

        if let (Some(submitted), Some(today)) = (self.submitted_on_date, biz_date) {
            if submitted > today {
                errors.push(ApiParameterError::parameter_error(
                    "error.msg.clients.submittedOnDate.in.the.future",
                    "submitted date cannot be in the future.",
                    "submittedOnDate",
                    vec![],
                ));
            }
        }

        if let Some(activation) = self.activation_date {
            if let Some(submitted) = self.submitted_on_date {
                if submitted > activation {
                    errors.push(ApiParameterError::parameter_error(
                        "error.msg.clients.submittedOnDate.after.activation.date",
                        "submitted date cannot be after the activation date",
                        "submittedOnDate",
                        vec![],
                    ));
                }
            }
            if let Some(reopened) = self.reopened_on_date {
                if reopened > activation {
                    errors.push(ApiParameterError::parameter_error(
                        "error.msg.clients.submittedOnDate.after.reopened.date",
                        "reopened date cannot be after the activation date",
                        "reopenedDate",
                        vec![],
                    ));
                }
            }
            if let Some(today) = biz_date {
                if activation > today {
                    errors.push(ApiParameterError::parameter_error(
                        "error.msg.clients.activationDate.in.the.future",
                        "Activation date cannot be in the future.",
                        "activationDate",
                        vec![],
                    ));
                }
            }
        }

        if !errors.is_empty() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(errors),
            ));
        }
        Ok(())
    }

    /// Validates both name parts and activation date constraints, matching `Client.validate()`.
    pub fn validate(&self) -> FineractResult<()> {
        self.validate_name_parts()?;
        self.validate_activation_date()?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // State Machine Methods
    // -------------------------------------------------------------------------

    /// Activates the client (`Pending`/`Reopened` -> `Active`), setting `activation_date`
    /// and `office_joining_date`, and clearing any prior closure metadata.
    ///
    /// Ported from `Client.activate(...)`.
    pub fn activate(&mut self, activation_date: NaiveDate) -> FineractResult<()> {
        if self.is_active() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "error.msg.clients.already.active",
                    "Cannot activate client. Client is already active.",
                    "activationDate",
                    vec![],
                ),
            ));
        }
        if !self.is_pending() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.activate.invalid.status",
                "Only pending or reopened clients can be activated.",
            )));
        }
        if let Some(submitted) = self.submitted_on_date {
            if submitted > activation_date {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "error.msg.clients.submittedOnDate.after.activation.date",
                        "submitted date cannot be after the activation date",
                        "submittedOnDate",
                        vec![],
                    ),
                ));
            }
        }
        if let Some(reopened) = self.reopened_on_date {
            if reopened > activation_date {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "error.msg.clients.submittedOnDate.after.reopened.date",
                        "reopened date cannot be after the activation date",
                        "reopenedDate",
                        vec![],
                    ),
                ));
            }
        }
        if let Some(today) = current_business_date_opt() {
            if activation_date > today {
                return Err(FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "error.msg.clients.activationDate.in.the.future",
                        "Activation date cannot be in the future.",
                        "activationDate",
                        vec![],
                    ),
                ));
            }
        }

        self.activation_date = Some(activation_date);
        self.office_joining_date = Some(activation_date);
        self.status = ClientStatus::Active;
        self.closed_on_date = None;
        self.closure_reason_cv_id = None;
        Ok(())
    }

    /// Rejects a pending client (`Pending`/`Reopened` -> `Rejected`), recording `rejected_on_date`
    /// and `reject_reason_cv_id`.
    ///
    /// Ported from `ClientWritePlatformServiceJpaRepositoryImpl.rejectClient` and `Client.reject`.
    pub fn reject(&mut self, rejection_date: NaiveDate, reason_cv_id: i64) -> FineractResult<()> {
        if self.is_not_pending() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.rejection.on.account.not.in.pending.activation.status",
                "Only clients pending activation may be rejected.",
            )));
        }
        if reason_cv_id <= 0 {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.client.rejectionReasonId.cannot.be.blank",
                    "The parameter `rejectionReasonId` is mandatory.",
                    "rejectionReasonId",
                    vec![],
                ),
            ));
        }
        if let Some(submitted) = self.submitted_on_date {
            if submitted > rejection_date {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.client.rejection.date.cannot.before.client.submitted.date",
                    "The client rejection date cannot be before the client submitted date.",
                )));
            }
        }

        self.rejected_on_date = Some(rejection_date);
        self.reject_reason_cv_id = Some(reason_cv_id);
        self.status = ClientStatus::Rejected;
        Ok(())
    }

    /// Withdraws a pending client (`Pending`/`Reopened` -> `Withdrawn`), recording `withdrawn_on_date`
    /// and `withdraw_reason_cv_id`.
    ///
    /// Ported from `ClientWritePlatformServiceJpaRepositoryImpl.withdrawClient` and `Client.withdraw`.
    pub fn withdraw(
        &mut self,
        withdrawal_date: NaiveDate,
        reason_cv_id: i64,
    ) -> FineractResult<()> {
        if self.is_not_pending() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.withdrawal.on.account.not.in.pending.activation.status",
                "Only clients pending activation may be withdrawn.",
            )));
        }
        if reason_cv_id <= 0 {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.client.withdrawalReasonId.cannot.be.blank",
                    "The parameter `withdrawalReasonId` is mandatory.",
                    "withdrawalReasonId",
                    vec![],
                ),
            ));
        }
        if let Some(submitted) = self.submitted_on_date {
            if submitted > withdrawal_date {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.client.withdrawal.date.cannot.before.client.submitted.date",
                    "The client withdrawal date cannot be before the client submitted date.",
                )));
            }
        }

        self.withdrawn_on_date = Some(withdrawal_date);
        self.withdraw_reason_cv_id = Some(reason_cv_id);
        self.status = ClientStatus::Withdrawn;
        Ok(())
    }

    /// Closes a client (`Active`/`Pending` -> `Closed`), recording `closed_on_date`
    /// and `closure_reason_cv_id`.
    ///
    /// Ported from `ClientWritePlatformServiceJpaRepositoryImpl.closeClient` and `Client.close`.
    pub fn close(&mut self, closure_date: NaiveDate, reason_cv_id: i64) -> FineractResult<()> {
        if self.status.is_closed() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.close.is.already.closed",
                "Client is already closed.",
            )));
        }
        if self.status.is_under_transfer() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.close.is.under.transfer",
                "Cannot Close a Client under Transfer",
            )));
        }
        if reason_cv_id <= 0 {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.client.closureReasonId.cannot.be.blank",
                    "The parameter `closureReasonId` is mandatory.",
                    "closureReasonId",
                    vec![],
                ),
            ));
        }
        if self.is_not_pending() {
            if let Some(activation) = self.activation_date {
                if activation > closure_date {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.client.close.date.cannot.before.client.actvation.date",
                        "The client closureDate cannot be before the client ActivationDate.",
                    )));
                }
            }
        }

        self.closed_on_date = Some(closure_date);
        self.closure_reason_cv_id = Some(reason_cv_id);
        self.status = ClientStatus::Closed;
        Ok(())
    }

    /// Reactivates a closed client (`Closed` -> `Pending`), clearing `closed_on_date` and
    /// `closure_reason_cv_id` and setting `reactivated_on_date`.
    ///
    /// Ported from `ClientWritePlatformServiceJpaRepositoryImpl.reActivateClient` and `Client.reActivate`.
    pub fn reactivate(&mut self, reactivate_date: NaiveDate) -> FineractResult<()> {
        if !self.is_closed() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.reactivation.on.nonclosed.account",
                "only closed clients may be reactivated.",
            )));
        }
        if let Some(closed_date) = self.closed_on_date {
            if closed_date > reactivate_date {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.client.reactivation.date.cannot.before.client.closed.date",
                    "The client reactivation date cannot be before the client closed date.",
                )));
            }
        }

        self.closed_on_date = None;
        self.closure_reason_cv_id = None;
        self.reactivated_on_date = Some(reactivate_date);
        self.status = ClientStatus::Pending;
        Ok(())
    }

    /// Reopens a rejected or withdrawn client (`Rejected`/`Withdrawn` -> `Reopened`).
    pub fn reopen(&mut self, reopened_date: NaiveDate) -> FineractResult<()> {
        if self.is_rejected() {
            if let Some(rejected_date) = self.rejected_on_date {
                if rejected_date > reopened_date {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.client.reopened.date.cannot.before.client.rejected.date",
                        "The client reactivation date cannot be before the client rejected date.",
                    )));
                }
            }
        } else if self.is_withdrawn() {
            if let Some(withdrawn_date) = self.withdrawn_on_date {
                if withdrawn_date > reopened_date {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.client.reopened.date.cannot.before.client.withdrawal.date",
                        "The client reactivation date cannot be before the client withdrawal date.",
                    )));
                }
            }
        } else {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.reopen.invalid.status",
                "Only rejected or withdrawn clients may be reopened.",
            )));
        }

        self.reopened_on_date = Some(reopened_date);
        self.status = ClientStatus::Reopened;
        Ok(())
    }

    /// Initiates a branch transfer proposal (`Active` -> `TransferInProgress`),
    /// setting `transfer_to_office_id`.
    ///
    /// Ported from `TransferWritePlatformServiceJpaRepositoryImpl.proposeClientTransfer`.
    pub fn propose_transfer(&mut self, destination_office_id: i64) -> FineractResult<()> {
        if destination_office_id <= 0 {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.client.destinationOfficeId.cannot.be.blank",
                    "The parameter `destinationOfficeId` must be greater than zero.",
                    "destinationOfficeId",
                    vec![],
                ),
            ));
        }
        if self.office_id == destination_office_id {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.transfers.cannot.be.initiated.to.same.branch",
                "Transfer cannot be initiated to the same branch.",
            )));
        }
        if !self.is_active() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.transfers.client.not.active",
                "Client must be active to propose a branch transfer.",
            )));
        }

        self.status = ClientStatus::TransferInProgress;
        self.transfer_to_office_id = Some(destination_office_id);
        Ok(())
    }

    /// Accepts a pending branch transfer (`TransferInProgress` -> `Active`),
    /// moving `office_id` to `transfer_to_office_id` and updating `office_joining_date`.
    ///
    /// Ported from `TransferWritePlatformServiceJpaRepositoryImpl.acceptClientTransfer`.
    pub fn accept_transfer(&mut self, transfer_date: NaiveDate) -> FineractResult<()> {
        if !self.is_transfer_in_progress() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.transfers.not.awaiting.transfer.approval",
                "Client is not awaiting transfer approval.",
            )));
        }
        let destination_office_id = self.transfer_to_office_id.ok_or_else(|| {
            FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.transfers.destination.office.not.set",
                "Transfer destination office is not set on client.",
            ))
        })?;

        self.status = ClientStatus::Active;
        self.office_id = destination_office_id;
        self.transfer_to_office_id = None;
        self.office_joining_date = Some(self.proposed_transfer_date.unwrap_or(transfer_date));
        self.proposed_transfer_date = None;
        Ok(())
    }

    /// Rejects a pending branch transfer (`TransferInProgress` -> `TransferOnHold`),
    /// clearing `transfer_to_office_id` and `proposed_transfer_date`.
    ///
    /// Ported from `TransferWritePlatformServiceJpaRepositoryImpl.rejectClientTransfer`.
    pub fn reject_transfer(&mut self) -> FineractResult<()> {
        if !self.is_transfer_in_progress() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.transfers.not.awaiting.transfer.approval",
                "Client is not awaiting transfer approval.",
            )));
        }

        self.status = ClientStatus::TransferOnHold;
        self.transfer_to_office_id = None;
        self.proposed_transfer_date = None;
        Ok(())
    }

    /// Withdraws a branch transfer (`TransferInProgress` or `TransferOnHold` -> `Active`).
    pub fn withdraw_transfer(&mut self) -> FineractResult<()> {
        if !self.is_transfer_in_progress_or_on_hold() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.transfers.not.awaiting.transfer.approval.or.on.hold",
                "Client is not awaiting transfer approval or on hold.",
            )));
        }

        self.status = ClientStatus::Active;
        self.transfer_to_office_id = None;
        self.proposed_transfer_date = None;
        Ok(())
    }

    /// Converts this [`Client`] into a [`ClientData`] DTO.
    pub fn to_data(&self) -> ClientData {
        ClientData::from(self)
    }

    /// Validates a `CREATE CLIENT` command payload matching `ClientDataValidator.validateForCreate`.
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(CLIENT_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("client");

        let office_id = command.long_value_of_parameter_named("officeId")?;
        validator
            .reset()
            .parameter("officeId")
            .value(office_id)
            .not_null()
            .integer_greater_than_zero();

        if command.parameter_exists("accountNo") {
            let account_no = command.string_value_of_parameter_named("accountNo");
            validator
                .reset()
                .parameter("accountNo")
                .value(account_no)
                .not_blank()
                .not_exceeding_length_of(20);
        }

        if command.parameter_exists("externalId") {
            let external_id = command.string_value_of_parameter_named("externalId");
            validator
                .reset()
                .parameter("externalId")
                .value(external_id)
                .ignore_if_null()
                .not_exceeding_length_of(100);
        }

        if command.parameter_exists("mobileNo") {
            let mobile_no = command.string_value_of_parameter_named("mobileNo");
            validator
                .reset()
                .parameter("mobileNo")
                .value(mobile_no)
                .ignore_if_null()
                .not_exceeding_length_of(50);
        }

        if command.parameter_exists("legalFormId") {
            let legal_form_id = command.integer_value_sans_locale_of_parameter_named("legalFormId")?;
            validator
                .reset()
                .parameter("legalFormId")
                .value(legal_form_id)
                .not_null()
                .in_min_max_range(1, 2);
        }

        let active = command.boolean_object_value_of_parameter_named("active");
        if active == Some(true) {
            let activation_date = command.local_date_value_of_parameter_named("activationDate")?;
            validator
                .reset()
                .parameter("activationDate")
                .value(activation_date)
                .not_null();
        }

        let fullname = normalize_opt_string(command.string_value_of_parameter_named("fullname"));
        let firstname = normalize_opt_string(command.string_value_of_parameter_named("firstname"));
        let middlename =
            normalize_opt_string(command.string_value_of_parameter_named("middlename"));
        let lastname = normalize_opt_string(command.string_value_of_parameter_named("lastname"));

        let has_fullname = fullname.is_some();
        let has_individual = firstname.is_some() || middlename.is_some() || lastname.is_some();

        if has_fullname && has_individual {
            validator
                .reset()
                .parameter("fullname")
                .fail_with_code(
                    "must.be.blank.when.individual.name.parts.provided",
                    "Fullname cannot be provided together with individual name parts.",
                );
        } else if has_fullname {
            validator
                .reset()
                .parameter("fullname")
                .value(fullname)
                .not_blank()
                .not_exceeding_length_of(160);
        } else {
            validator
                .reset()
                .parameter("firstname")
                .value(firstname)
                .not_blank()
                .not_exceeding_length_of(50);
            validator
                .reset()
                .parameter("middlename")
                .value(middlename)
                .ignore_if_null()
                .not_exceeding_length_of(50);
            validator
                .reset()
                .parameter("lastname")
                .value(lastname)
                .not_blank()
                .not_exceeding_length_of(50);
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a [`Client`] from a [`JsonCommand`], matching `ClientWritePlatformServiceJpaRepositoryImpl.createClient`.
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;

        let office_id = command
            .long_value_of_parameter_named("officeId")?
            .unwrap_or(1);
        let legal_form = command
            .integer_value_sans_locale_of_parameter_named("legalFormId")?
            .and_then(LegalForm::from_i32)
            .unwrap_or_else(|| {
                if normalize_opt_string(command.string_value_of_parameter_named("fullname"))
                    .is_some()
                {
                    LegalForm::Entity
                } else {
                    LegalForm::Person
                }
            });

        let account_no = normalize_opt_string(command.string_value_of_parameter_named("accountNo"))
            .unwrap_or_else(|| {
                if id > 0 {
                    Self::format_account_no(id)
                } else {
                    String::new()
                }
            });

        let external_id = ExternalId::from_opt(normalize_opt_string(
            command.string_value_of_parameter_named("externalId"),
        ));
        let mobile_no = normalize_opt_string(command.string_value_of_parameter_named("mobileNo"));
        let email_address =
            normalize_opt_string(command.string_value_of_parameter_named("emailAddress"));
        let firstname = normalize_opt_string(command.string_value_of_parameter_named("firstname"));
        let middlename =
            normalize_opt_string(command.string_value_of_parameter_named("middlename"));
        let lastname = normalize_opt_string(command.string_value_of_parameter_named("lastname"));
        let fullname = normalize_opt_string(command.string_value_of_parameter_named("fullname"));
        let is_staff = command.boolean_primitive_value_of_parameter_named("isStaff");
        let date_of_birth = command.local_date_value_of_parameter_named("dateOfBirth")?;
        let staff_id = command.long_value_of_parameter_named("staffId")?;
        let gender_cv_id = command.long_value_of_parameter_named("genderId")?;
        let client_type_cv_id = command.long_value_of_parameter_named("clientTypeId")?;
        let client_classification_cv_id =
            command.long_value_of_parameter_named("clientClassificationId")?;
        let savings_product_id = command.long_value_of_parameter_named("savingsProductId")?;
        let savings_account_id = command.long_value_of_parameter_named("savingsAccountId")?;

        let active = command.boolean_primitive_value_of_parameter_named("active");
        let activation_date = if active {
            command.local_date_value_of_parameter_named("activationDate")?
        } else {
            None
        };
        let office_joining_date = activation_date;
        let status = if active {
            ClientStatus::Active
        } else {
            ClientStatus::Pending
        };

        let default_submitted = current_business_date_opt()
            .or(activation_date)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"));
        let mut submitted_on_date = if command.parameter_exists("submittedOnDate") {
            command.local_date_value_of_parameter_named("submittedOnDate")?
        } else {
            Some(default_submitted)
        };
        if active {
            if let (Some(sub), Some(act)) = (submitted_on_date, activation_date) {
                if sub > act {
                    submitted_on_date = Some(act);
                }
            }
        }

        let mut client = Self {
            id,
            account_no,
            external_id,
            status,
            sub_status: None,
            activation_date,
            office_joining_date,
            office_id,
            transfer_to_office_id: None,
            staff_id,
            firstname,
            middlename,
            lastname,
            fullname,
            display_name: String::new(),
            mobile_no,
            email_address,
            date_of_birth,
            gender_cv_id,
            client_type_cv_id,
            client_classification_cv_id,
            legal_form,
            submitted_on_date,
            closed_on_date: None,
            closure_reason_cv_id: None,
            rejected_on_date: None,
            reject_reason_cv_id: None,
            withdrawn_on_date: None,
            withdraw_reason_cv_id: None,
            reactivated_on_date: None,
            reopened_on_date: None,
            proposed_transfer_date: None,
            savings_product_id,
            savings_account_id,
            is_staff,
            image_id: None,
        };
        client.derive_display_name();
        client.validate()?;
        Ok(client)
    }
}

// =============================================================================
// 3. CLIENT NON-PERSON (`m_client_non_person`)
// =============================================================================

/// Supported JSON parameters for `clientNonPersonDetails`.
pub const CLIENT_NON_PERSON_SUPPORTED_PARAMETERS: &[&str] = &[
    "incorpNumber",
    "remarks",
    "incorpValidityTillDate",
    "constitutionId",
    "mainBusinessLineId",
    "locale",
    "dateFormat",
];

/// Corporate / Non-Person details for an entity client (`m_client_non_person`).
///
/// Ported from `org.apache.fineract.portfolio.client.domain.ClientNonPerson`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientNonPerson {
    pub id: i64,
    pub client_id: i64,
    pub constitution_cv_id: i64,
    pub incorp_no: Option<String>,
    pub incorp_validity_till: Option<NaiveDate>,
    pub main_business_line_cv_id: Option<i64>,
    pub remarks: Option<String>,
}

impl ClientNonPerson {
    /// Creates a new [`ClientNonPerson`] and validates domain rules against the parent client's incorporation date (`date_of_birth`).
    pub fn create_new(
        id: i64,
        client_id: i64,
        constitution_cv_id: i64,
        main_business_line_cv_id: Option<i64>,
        incorp_no: Option<String>,
        incorp_validity_till: Option<NaiveDate>,
        remarks: Option<String>,
        client_incorp_date: Option<NaiveDate>,
    ) -> FineractResult<Self> {
        let entity = Self {
            id,
            client_id,
            constitution_cv_id,
            incorp_no: normalize_opt_string(incorp_no),
            incorp_validity_till,
            main_business_line_cv_id,
            remarks: normalize_opt_string(remarks),
        };
        entity.validate(client_incorp_date)?;
        Ok(entity)
    }

    pub fn new(
        id: i64,
        client_id: i64,
        constitution_cv_id: i64,
        main_business_line_cv_id: Option<i64>,
        incorp_no: Option<String>,
        incorp_validity_till: Option<NaiveDate>,
        remarks: Option<String>,
    ) -> Self {
        Self {
            id,
            client_id,
            constitution_cv_id,
            incorp_no: normalize_opt_string(incorp_no),
            incorp_validity_till,
            main_business_line_cv_id,
            remarks: normalize_opt_string(remarks),
        }
    }

    pub fn incorp_number(&self) -> Option<&str> {
        self.incorp_no.as_deref()
    }

    pub const fn constitution_id(&self) -> i64 {
        self.constitution_cv_id
    }

    pub const fn main_business_line_id(&self) -> Option<i64> {
        self.main_business_line_cv_id
    }

    /// Validates `constitution_cv_id`, field lengths, and `incorp_validity_till` against `client_incorp_date`,
    /// matching `ClientNonPerson.validate(client)`.
    pub fn validate(&self, client_incorp_date: Option<NaiveDate>) -> FineractResult<()> {
        let mut errors = Vec::new();
        if self.constitution_cv_id <= 0 {
            errors.push(ApiParameterError::parameter_error(
                "error.msg.clients.constitutionid.is.null",
                "Constitution ID may not be null",
                "constitutionId",
                vec![],
            ));
        }
        if let (Some(incorp_date), Some(validity_till)) =
            (client_incorp_date, self.incorp_validity_till)
        {
            if incorp_date > validity_till {
                errors.push(ApiParameterError::parameter_error(
                    "error.msg.clients.incorpValidityTill.after.incorp.date",
                    "incorpvaliditytill date cannot be after the incorporation date",
                    "incorpValidityTillDate",
                    vec![],
                ));
            }
        }
        if let Some(ref incorp_no) = self.incorp_no {
            if incorp_no.len() > 50 {
                errors.push(ApiParameterError::parameter_error(
                    "validation.msg.client.incorpNumber.exceeds.max.length",
                    "incorpNumber cannot exceed 50 characters",
                    "incorpNumber",
                    vec![],
                ));
            }
        }
        if let Some(ref remarks) = self.remarks {
            if remarks.len() > 150 {
                errors.push(ApiParameterError::parameter_error(
                    "validation.msg.client.remarks.exceeds.max.length",
                    "remarks cannot exceed 150 characters",
                    "remarks",
                    vec![],
                ));
            }
        }

        if !errors.is_empty() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(errors),
            ));
        }
        Ok(())
    }
}

// =============================================================================
// 4. CLIENT IDENTIFIER (`m_client_identifier`)
// =============================================================================

/// Supported JSON parameters for ClientIdentifier create and update commands.
pub const CLIENT_IDENTIFIER_SUPPORTED_PARAMETERS: &[&str] = &[
    "documentTypeId",
    "documentKey",
    "status",
    "description",
    "documentDescription",
    "issuanceDate",
    "expiryDate",
    "locale",
    "dateFormat",
];

/// Client KYC document identifier (`m_client_identifier`).
///
/// Ported from `org.apache.fineract.portfolio.client.domain.ClientIdentifier`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientIdentifier {
    pub id: i64,
    pub client_id: i64,
    pub document_type_id: i64,
    pub document_key: String,
    pub status: ClientIdentifierStatus,
    pub description: Option<String>,
    pub issuance_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    /// Mirrors `m_client_identifier.active` (`Some(200)` when active, `None` when inactive).
    pub active: Option<i32>,
}

impl ClientIdentifier {
    /// Creates a new [`ClientIdentifier`] and synchronizes `self.active` with `status`.
    pub fn new(
        id: i64,
        client_id: i64,
        document_type_id: i64,
        document_key: impl Into<String>,
        status: ClientIdentifierStatus,
        description: Option<String>,
        issuance_date: Option<NaiveDate>,
        expiry_date: Option<NaiveDate>,
    ) -> FineractResult<Self> {
        let key = document_key.into().trim().to_string();
        let identifier = Self {
            id,
            client_id,
            document_type_id,
            document_key: key,
            status,
            description: normalize_opt_string(description),
            issuance_date,
            expiry_date,
            active: status.active_column_value(),
        };
        identifier.validate()?;
        Ok(identifier)
    }

    pub const fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub const fn is_inactive(&self) -> bool {
        self.status.is_inactive()
    }

    pub fn document_key(&self) -> &str {
        &self.document_key
    }

    pub const fn document_type_id(&self) -> i64 {
        self.document_type_id
    }

    /// Updates the status of this identifier and keeps `self.active` aligned (`Some(200)` vs `None`).
    pub fn set_status(&mut self, status: ClientIdentifierStatus) {
        self.status = status;
        self.active = status.active_column_value();
    }

    /// Validates mandatory fields (`document_type_id > 0`, `document_key` non-blank, valid `status`).
    pub fn validate(&self) -> FineractResult<()> {
        let validator = DataValidatorBuilder::for_resource("clientIdentifier");
        validator
            .reset()
            .parameter("documentTypeId")
            .value(Some(self.document_type_id))
            .not_null()
            .integer_greater_than_zero();
        validator
            .reset()
            .parameter("documentKey")
            .value(Some(self.document_key.clone()))
            .not_blank()
            .not_exceeding_length_of(1000);
        if self.status.is_invalid() {
            validator
                .reset()
                .parameter("status")
                .fail_with_code("invalid.status", "Status must be Active or Inactive.");
        }
        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Validates `CREATE CLIENTIDENTIFIER` command parameters matching `ClientIdentifierCommand.validateForCreate`.
    pub fn validate_for_create(command: &JsonCommand) -> FineractResult<()> {
        command.check_for_unsupported_parameters(CLIENT_IDENTIFIER_SUPPORTED_PARAMETERS)?;
        let validator = DataValidatorBuilder::for_resource("clientIdentifier");

        let document_type_id = command.long_value_of_parameter_named("documentTypeId")?;
        validator
            .reset()
            .parameter("documentTypeId")
            .value(document_type_id)
            .not_null()
            .integer_greater_than_zero();

        let document_key = command.string_value_of_parameter_named("documentKey");
        validator
            .reset()
            .parameter("documentKey")
            .value(document_key)
            .not_blank()
            .not_exceeding_length_of(1000);

        if let Some(status_raw) = command.string_value_of_parameter_named("status") {
            let parsed = ClientIdentifierStatus::from_str_flexible(&status_raw);
            if parsed.is_invalid() {
                validator
                    .reset()
                    .parameter("status")
                    .fail_with_code("invalid.status", "Status must be Active or Inactive.");
            }
        }

        validator
            .throw_validation_errors()
            .map_err(FineractError::from)
    }

    /// Constructs a [`ClientIdentifier`] from a [`JsonCommand`], matching `ClientIdentifier.fromJson`.
    pub fn from_json(id: i64, client_id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Self::validate_for_create(command)?;
        let document_type_id = command
            .long_value_of_parameter_named("documentTypeId")?
            .unwrap_or_default();
        let document_key = command
            .string_value_of_parameter_named("documentKey")
            .unwrap_or_default();
        let description = command
            .string_value_of_parameter_named("description")
            .or_else(|| command.string_value_of_parameter_named("documentDescription"));
        let status_raw = command
            .string_value_of_parameter_named("status")
            .unwrap_or_else(|| "Active".to_string());
        let status = ClientIdentifierStatus::from_str_flexible(&status_raw);
        let issuance_date = command.local_date_value_of_parameter_named("issuanceDate")?;
        let expiry_date = command.local_date_value_of_parameter_named("expiryDate")?;

        Self::new(
            id,
            client_id,
            document_type_id,
            document_key,
            status,
            description,
            issuance_date,
            expiry_date,
        )
    }

    /// Validates uniqueness against existing identifiers:
    /// 1. `(document_type_id, document_key)` must be globally unique (`unique_identifier_key` -> `error.msg.clientIdentifier.identityKey.duplicate`).
    /// 2. An active identifier (`active == Some(200)`) cannot duplicate an active identifier of the same `document_type_id`
    ///    or `document_key` for the same `client_id` (`unique_active_client_identifier` -> `error.msg.clientIdentifier.type.duplicate`).
    pub fn validate_uniqueness(&self, existing_identifiers: &[ClientIdentifier]) -> FineractResult<()> {
        for other in existing_identifiers {
            if other.id == self.id && self.id > 0 {
                continue;
            }
            if other.document_type_id == self.document_type_id
                && other
                    .document_key
                    .trim()
                    .eq_ignore_ascii_case(self.document_key.trim())
            {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.clientIdentifier.identityKey.duplicate",
                    format!(
                        "Client identifier of type {} with value of {} already exists.",
                        self.document_type_id, self.document_key
                    ),
                )));
            }
            if self.is_active()
                && other.is_active()
                && other.client_id == self.client_id
                && (other.document_type_id == self.document_type_id
                    || other
                        .document_key
                        .trim()
                        .eq_ignore_ascii_case(self.document_key.trim()))
            {
                return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                    "error.msg.clientIdentifier.type.duplicate",
                    format!(
                        "Active Client identifier of type {} is already present for this client",
                        self.document_type_id
                    ),
                )));
            }
        }
        Ok(())
    }

    /// Applies updates from a [`JsonCommand`], returning the changed field map matching `ClientIdentifier.update`.
    pub fn update(&mut self, command: &JsonCommand) -> FineractResult<BTreeMap<String, Value>> {
        command.check_for_unsupported_parameters(CLIENT_IDENTIFIER_SUPPORTED_PARAMETERS)?;
        let mut changes = BTreeMap::new();

        if command.is_change_in_long_parameter_named("documentTypeId", Some(self.document_type_id))? {
            if let Some(new_type_id) = command.long_value_of_parameter_named("documentTypeId")? {
                self.document_type_id = new_type_id;
                changes.insert("documentTypeId".to_string(), Value::from(new_type_id));
            }
        }

        if command.is_change_in_string_parameter_named("documentKey", Some(&self.document_key)) {
            let new_key = command
                .string_value_of_parameter_named("documentKey")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            changes.insert("documentKey".to_string(), Value::String(new_key.clone()));
            self.document_key = new_key;
        }

        if command.is_change_in_string_parameter_named("description", self.description.as_deref()) {
            let new_desc = normalize_opt_string(command.string_value_of_parameter_named("description"));
            changes.insert(
                "description".to_string(),
                new_desc
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            self.description = new_desc;
        }

        if let Some(status_str) = command.string_value_of_parameter_named("status") {
            let new_status = ClientIdentifierStatus::from_str_flexible(&status_str);
            if !new_status.is_invalid() && new_status != self.status {
                self.set_status(new_status);
                changes.insert("status".to_string(), Value::String(status_str));
            }
        }

        if command.is_change_in_local_date_parameter_named("issuanceDate", self.issuance_date)? {
            let new_date = command.local_date_value_of_parameter_named("issuanceDate")?;
            self.issuance_date = new_date;
            changes.insert(
                "issuanceDate".to_string(),
                new_date
                    .map(|d| Value::String(d.to_string()))
                    .unwrap_or(Value::Null),
            );
        }

        if command.is_change_in_local_date_parameter_named("expiryDate", self.expiry_date)? {
            let new_date = command.local_date_value_of_parameter_named("expiryDate")?;
            self.expiry_date = new_date;
            changes.insert(
                "expiryDate".to_string(),
                new_date
                    .map(|d| Value::String(d.to_string()))
                    .unwrap_or(Value::Null),
            );
        }

        self.validate()?;
        Ok(changes)
    }
}

// =============================================================================
// 5. ADDRESS (`m_address`) & CLIENT ADDRESS (`m_client_address`)
// =============================================================================

/// Physical or postal address record (`m_address`).
///
/// Ported from `org.apache.fineract.portfolio.address.domain.Address`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub id: i64,
    pub street: Option<String>,
    pub address_line_1: Option<String>,
    pub address_line_2: Option<String>,
    pub address_line_3: Option<String>,
    pub town_village: Option<String>,
    pub city: Option<String>,
    pub county_district: Option<String>,
    pub state_province_id: Option<i64>,
    pub country_id: Option<i64>,
    pub postal_code: Option<String>,
    pub latitude: Option<Decimal>,
    pub longitude: Option<Decimal>,
    pub created_by: Option<String>,
    pub created_on: Option<NaiveDate>,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
}

impl Address {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        street: Option<String>,
        address_line_1: Option<String>,
        address_line_2: Option<String>,
        address_line_3: Option<String>,
        town_village: Option<String>,
        city: Option<String>,
        county_district: Option<String>,
        state_province_id: Option<i64>,
        country_id: Option<i64>,
        postal_code: Option<String>,
        latitude: Option<Decimal>,
        longitude: Option<Decimal>,
    ) -> Self {
        Self {
            id,
            street: normalize_opt_string(street),
            address_line_1: normalize_opt_string(address_line_1),
            address_line_2: normalize_opt_string(address_line_2),
            address_line_3: normalize_opt_string(address_line_3),
            town_village: normalize_opt_string(town_village),
            city: normalize_opt_string(city),
            county_district: normalize_opt_string(county_district),
            state_province_id,
            country_id,
            postal_code: normalize_opt_string(postal_code),
            latitude,
            longitude,
            created_by: None,
            created_on: current_business_date_opt(),
            updated_by: None,
            updated_on: current_business_date_opt(),
        }
    }

    /// Constructs an [`Address`] from a [`JsonCommand`], matching `Address.fromJson`.
    pub fn from_json(id: i64, command: &JsonCommand) -> FineractResult<Self> {
        Ok(Self {
            id,
            street: normalize_opt_string(command.string_value_of_parameter_named("street")),
            address_line_1: normalize_opt_string(
                command.string_value_of_parameter_named("addressLine1"),
            ),
            address_line_2: normalize_opt_string(
                command.string_value_of_parameter_named("addressLine2"),
            ),
            address_line_3: normalize_opt_string(
                command.string_value_of_parameter_named("addressLine3"),
            ),
            town_village: normalize_opt_string(
                command.string_value_of_parameter_named("townVillage"),
            ),
            city: normalize_opt_string(command.string_value_of_parameter_named("city")),
            county_district: normalize_opt_string(
                command.string_value_of_parameter_named("countyDistrict"),
            ),
            state_province_id: command.long_value_of_parameter_named("stateProvinceId")?,
            country_id: command.long_value_of_parameter_named("countryId")?,
            postal_code: normalize_opt_string(
                command.string_value_of_parameter_named("postalCode"),
            ),
            latitude: command.big_decimal_value_of_parameter_named("latitude")?,
            longitude: command.big_decimal_value_of_parameter_named("longitude")?,
            created_by: normalize_opt_string(command.string_value_of_parameter_named("createdBy")),
            created_on: command
                .local_date_value_of_parameter_named("createdOn")?
                .or_else(current_business_date_opt),
            updated_by: normalize_opt_string(command.string_value_of_parameter_named("updatedBy")),
            updated_on: command
                .local_date_value_of_parameter_named("updatedOn")?
                .or_else(current_business_date_opt),
        })
    }
}

/// Join entity linking a [`Client`] to an [`Address`] by address type (`m_client_address`).
///
/// Ported from `org.apache.fineract.portfolio.client.domain.ClientAddress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddress {
    pub id: i64,
    pub client_id: i64,
    pub address_id: i64,
    pub address_type_id: i64,
    pub is_active: bool,
}

impl ClientAddress {
    pub const fn new(
        id: i64,
        client_id: i64,
        address_id: i64,
        address_type_id: i64,
        is_active: bool,
    ) -> Self {
        Self {
            id,
            client_id,
            address_id,
            address_type_id,
            is_active,
        }
    }

    pub const fn from_json(
        id: i64,
        is_active: bool,
        client_id: i64,
        address_id: i64,
        address_type_id: i64,
    ) -> Self {
        Self::new(id, client_id, address_id, address_type_id, is_active)
    }
}

// =============================================================================
// 6. CLIENT FAMILY MEMBER (`m_family_members`)
// =============================================================================

/// Family member KYC record linked to a [`Client`] (`m_family_members`).
///
/// Ported from `org.apache.fineract.portfolio.client.domain.ClientFamilyMembers`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientFamilyMember {
    pub id: i64,
    pub client_id: i64,
    pub firstname: String,
    pub middlename: Option<String>,
    pub lastname: Option<String>,
    pub qualification: Option<String>,
    pub mobile_number: Option<String>,
    pub age: Option<i64>,
    pub is_dependent: Option<bool>,
    pub relationship_cv_id: Option<i64>,
    pub marital_status_cv_id: Option<i64>,
    pub gender_cv_id: Option<i64>,
    pub profession_cv_id: Option<i64>,
    pub date_of_birth: Option<NaiveDate>,
}

/// Alias matching the plural upstream Java class name `ClientFamilyMembers`.
pub type ClientFamilyMembers = ClientFamilyMember;

impl ClientFamilyMember {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        client_id: i64,
        firstname: impl Into<String>,
        middlename: Option<String>,
        lastname: Option<String>,
        qualification: Option<String>,
        mobile_number: Option<String>,
        age: Option<i64>,
        is_dependent: Option<bool>,
        relationship_cv_id: Option<i64>,
        marital_status_cv_id: Option<i64>,
        gender_cv_id: Option<i64>,
        date_of_birth: Option<NaiveDate>,
        profession_cv_id: Option<i64>,
    ) -> Self {
        Self {
            id,
            client_id,
            firstname: firstname.into().trim().to_string(),
            middlename: normalize_opt_string(middlename),
            lastname: normalize_opt_string(lastname),
            qualification: normalize_opt_string(qualification),
            mobile_number: normalize_opt_string(mobile_number),
            age,
            is_dependent,
            relationship_cv_id,
            marital_status_cv_id,
            gender_cv_id,
            profession_cv_id,
            date_of_birth,
        }
    }

    /// Constructs a [`ClientFamilyMember`] from a [`JsonCommand`], matching `ClientFamilyMembersWritePlatformServiceImpl.addFamilyMember`.
    pub fn from_json(id: i64, client_id: i64, command: &JsonCommand) -> FineractResult<Self> {
        let firstname = command
            .string_value_of_parameter_named("firstName")
            .or_else(|| command.string_value_of_parameter_named("firstname"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if firstname.is_empty() {
            return Err(FineractError::DataValidation(
                PlatformApiDataValidationException::single_parameter_error(
                    "validation.msg.familymember.firstName.cannot.be.blank",
                    "The parameter `firstName` is mandatory.",
                    "firstName",
                    vec![],
                ),
            ));
        }

        let middlename = command
            .string_value_of_parameter_named("middleName")
            .or_else(|| command.string_value_of_parameter_named("middlename"));
        let lastname = command
            .string_value_of_parameter_named("lastName")
            .or_else(|| command.string_value_of_parameter_named("lastname"));
        let qualification = command.string_value_of_parameter_named("qualification");
        let mobile_number = command.string_value_of_parameter_named("mobileNumber");
        let age = command.long_value_of_parameter_named("age")?;
        let is_dependent = command.boolean_object_value_of_parameter_named("isDependent");
        let relationship_cv_id = command.long_value_of_parameter_named("relationshipId")?;
        let marital_status_cv_id = command.long_value_of_parameter_named("maritalStatusId")?;
        let gender_cv_id = command.long_value_of_parameter_named("genderId")?;
        let profession_cv_id = command.long_value_of_parameter_named("professionId")?;
        let date_of_birth = command.local_date_value_of_parameter_named("dateOfBirth")?;

        Ok(Self::new(
            id,
            client_id,
            firstname,
            middlename,
            lastname,
            qualification,
            mobile_number,
            age,
            is_dependent,
            relationship_cv_id,
            marital_status_cv_id,
            gender_cv_id,
            date_of_birth,
            profession_cv_id,
        ))
    }

    pub fn first_name(&self) -> &str {
        &self.firstname
    }

    pub fn middle_name(&self) -> Option<&str> {
        self.middlename.as_deref()
    }

    pub fn last_name(&self) -> Option<&str> {
        self.lastname.as_deref()
    }
}

// =============================================================================
// 7. DUAL-DB CLIENT REPOSITORY (`DualDbClientRepository`)
// =============================================================================

/// Dual-database (PostgreSQL / MySQL + in-memory cache) repository for `Client`,
/// `ClientNonPerson`, `ClientIdentifier`, `Address`, `ClientAddress`, and `ClientFamilyMember`.
#[derive(Debug, Clone)]
pub struct DualDbClientRepository {
    pool: Option<AnyPool>,
    dialect: SqlDialect,
    clients: Arc<DashMap<i64, Client>>,
    non_persons: Arc<DashMap<i64, ClientNonPerson>>,
    identifiers: Arc<DashMap<i64, ClientIdentifier>>,
    addresses: Arc<DashMap<i64, Address>>,
    client_addresses: Arc<DashMap<i64, ClientAddress>>,
    family_members: Arc<DashMap<i64, ClientFamilyMember>>,
    next_id: Arc<AtomicI64>,
}

impl Default for DualDbClientRepository {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl DualDbClientRepository {
    pub const CLIENT_COLUMNS: &'static str = "id, account_no, external_id, status_enum, sub_status, activation_date, office_joining_date, office_id, transfer_to_office_id, staff_id, firstname, middlename, lastname, fullname, display_name, mobile_no, email_address, date_of_birth, gender_cv_id, client_type_cv_id, client_classification_cv_id, legal_form_enum, submittedon_date, closedon_date, closure_reason_cv_id, rejectedon_date, reject_reason_cv_id, withdrawn_on_date, withdraw_reason_cv_id, reactivated_on_date, reopened_on_date, proposed_transfer_date, default_savings_product, default_savings_account, is_staff, image_id";

    pub const CLIENT_NON_PERSON_COLUMNS: &'static str = "id, client_id, constitution_cv_id, incorp_no, incorp_validity_till, main_business_line_cv_id, remarks";

    pub const CLIENT_IDENTIFIER_COLUMNS: &'static str = "id, client_id, document_type_id, document_key, status, description, issuance_date, expiry_date, active";

    pub const ADDRESS_COLUMNS: &'static str = "id, street, address_line_1, address_line_2, address_line_3, town_village, city, county_district, state_province_id, country_id, postal_code, latitude, longitude, created_by, created_on, updated_by, updated_on";

    pub const CLIENT_ADDRESS_COLUMNS: &'static str =
        "id, client_id, address_id, address_type_id, is_active";

    pub const FAMILY_MEMBER_COLUMNS: &'static str = "id, client_id, firstname, middlename, lastname, qualification, mobile_number, age, is_dependent, relationship_cv_id, marital_status_cv_id, gender_cv_id, profession_cv_id, date_of_birth";

    pub fn new(pool: AnyPool, dialect: SqlDialect) -> Self {
        Self {
            pool: Some(pool),
            dialect,
            clients: Arc::new(DashMap::new()),
            non_persons: Arc::new(DashMap::new()),
            identifiers: Arc::new(DashMap::new()),
            addresses: Arc::new(DashMap::new()),
            client_addresses: Arc::new(DashMap::new()),
            family_members: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(1)),
        }
    }

    pub fn new_in_memory() -> Self {
        Self::with_dialect(SqlDialect::Postgres)
    }

    pub fn in_memory() -> Self {
        Self::new_in_memory()
    }

    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self {
            pool: None,
            dialect,
            clients: Arc::new(DashMap::new()),
            non_persons: Arc::new(DashMap::new()),
            identifiers: Arc::new(DashMap::new()),
            addresses: Arc::new(DashMap::new()),
            client_addresses: Arc::new(DashMap::new()),
            family_members: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(1)),
        }
    }

    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub fn pool(&self) -> Option<&AnyPool> {
        self.pool.as_ref()
    }

    pub fn allocate_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    // -------------------------------------------------------------------------
    // Dialect-Aware SQL Builders
    // -------------------------------------------------------------------------

    pub fn select_client_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!("SELECT {} FROM m_client WHERE id = ?", Self::CLIENT_COLUMNS),
            dialect,
        )
    }

    pub fn select_client_by_account_no_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_client WHERE account_no = ?",
                Self::CLIENT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_clients_by_office_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_client WHERE office_id = ? ORDER BY id ASC",
                Self::CLIENT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_client_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_client ({}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                Self::CLIENT_COLUMNS
            ),
            dialect,
        )
    }

    pub fn update_client_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_client SET account_no = ?, external_id = ?, status_enum = ?, sub_status = ?, activation_date = ?, office_joining_date = ?, office_id = ?, transfer_to_office_id = ?, staff_id = ?, firstname = ?, middlename = ?, lastname = ?, fullname = ?, display_name = ?, mobile_no = ?, email_address = ?, date_of_birth = ?, gender_cv_id = ?, client_type_cv_id = ?, client_classification_cv_id = ?, legal_form_enum = ?, submittedon_date = ?, closedon_date = ?, closure_reason_cv_id = ?, rejectedon_date = ?, reject_reason_cv_id = ?, withdrawn_on_date = ?, withdraw_reason_cv_id = ?, reactivated_on_date = ?, reopened_on_date = ?, proposed_transfer_date = ?, default_savings_product = ?, default_savings_account = ?, is_staff = ?, image_id = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn delete_client_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders("DELETE FROM m_client WHERE id = ?", dialect)
    }

    pub fn select_client_non_person_by_client_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_client_non_person WHERE client_id = ?",
                Self::CLIENT_NON_PERSON_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_client_non_person_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_client_non_person ({}) VALUES (?, ?, ?, ?, ?, ?, ?)",
                Self::CLIENT_NON_PERSON_COLUMNS
            ),
            dialect,
        )
    }

    pub fn update_client_non_person_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_client_non_person SET constitution_cv_id = ?, incorp_no = ?, incorp_validity_till = ?, main_business_line_cv_id = ?, remarks = ? WHERE client_id = ?",
            dialect,
        )
    }

    pub fn delete_client_non_person_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "DELETE FROM m_client_non_person WHERE client_id = ?",
            dialect,
        )
    }

    pub fn select_client_identifier_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_client_identifier WHERE id = ?",
                Self::CLIENT_IDENTIFIER_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_client_identifiers_by_client_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_client_identifier WHERE client_id = ? ORDER BY id ASC",
                Self::CLIENT_IDENTIFIER_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_client_identifier_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_client_identifier ({}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                Self::CLIENT_IDENTIFIER_COLUMNS
            ),
            dialect,
        )
    }

    pub fn update_client_identifier_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_client_identifier SET document_type_id = ?, document_key = ?, status = ?, description = ?, issuance_date = ?, expiry_date = ?, active = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn delete_client_identifier_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders("DELETE FROM m_client_identifier WHERE id = ?", dialect)
    }

    pub fn insert_address_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_address ({}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                Self::ADDRESS_COLUMNS
            ),
            dialect,
        )
    }

    pub fn update_address_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_address SET street = ?, address_line_1 = ?, address_line_2 = ?, address_line_3 = ?, town_village = ?, city = ?, county_district = ?, state_province_id = ?, country_id = ?, postal_code = ?, latitude = ?, longitude = ?, updated_by = ?, updated_on = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn select_address_by_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!("SELECT {} FROM m_address WHERE id = ?", Self::ADDRESS_COLUMNS),
            dialect,
        )
    }

    pub fn insert_client_address_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_client_address ({}) VALUES (?, ?, ?, ?, ?)",
                Self::CLIENT_ADDRESS_COLUMNS
            ),
            dialect,
        )
    }

    pub fn select_client_addresses_by_client_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_client_address WHERE client_id = ? ORDER BY id ASC",
                Self::CLIENT_ADDRESS_COLUMNS
            ),
            dialect,
        )
    }

    pub fn insert_family_member_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "INSERT INTO m_family_members ({}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                Self::FAMILY_MEMBER_COLUMNS
            ),
            dialect,
        )
    }

    pub fn update_family_member_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            "UPDATE m_family_members SET firstname = ?, middlename = ?, lastname = ?, qualification = ?, mobile_number = ?, age = ?, is_dependent = ?, relationship_cv_id = ?, marital_status_cv_id = ?, gender_cv_id = ?, profession_cv_id = ?, date_of_birth = ? WHERE id = ?",
            dialect,
        )
    }

    pub fn delete_family_member_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders("DELETE FROM m_family_members WHERE id = ?", dialect)
    }

    pub fn select_family_members_by_client_id_sql(dialect: SqlDialect) -> String {
        DialectSql::rewrite_placeholders(
            &format!(
                "SELECT {} FROM m_family_members WHERE client_id = ? ORDER BY id ASC",
                Self::FAMILY_MEMBER_COLUMNS
            ),
            dialect,
        )
    }

    // -------------------------------------------------------------------------
    // Client Operations
    // -------------------------------------------------------------------------

    fn check_client_uniqueness(&self, client: &Client) -> FineractResult<()> {
        for entry in self.clients.iter() {
            let existing = entry.value();
            if existing.id != client.id {
                if !client.account_no.is_empty() && existing.account_no == client.account_no {
                    return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                        "error.msg.client.duplicate.accountNo",
                        format!("Client with accountNo `{}` already exists", client.account_no),
                    )));
                }
                if let (Some(a), Some(b)) =
                    (existing.external_id.value(), client.external_id.value())
                {
                    if a == b {
                        return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                            "error.msg.client.duplicate.externalId",
                            format!("Client with externalId `{b}` already exists"),
                        )));
                    }
                }
                if let (Some(ref a), Some(ref b)) = (&existing.mobile_no, &client.mobile_no) {
                    if a == b {
                        return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                            "error.msg.client.duplicate.mobileNo",
                            format!("Client with mobileNo `{b}` already exists"),
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Retrieves a [`Client`] by ID, returning `ClientNotFoundException` (`error.msg.client.id.invalid`) if not found.
    pub async fn get_client(&self, client_id: i64) -> FineractResult<Client> {
        if let Some(entry) = self.clients.get(&client_id) {
            return Ok(entry.value().clone());
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::select_client_by_id_sql(self.dialect);
            let maybe_row = sqlx::query(&sql)
                .bind(client_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            if let Some(row) = maybe_row {
                let id: i64 = row
                    .try_get("id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let account_no: String = row
                    .try_get("account_no")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let external_id_raw: Option<String> = row
                    .try_get("external_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let status_enum: i32 = row
                    .try_get("status_enum")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let sub_status: Option<i64> = row
                    .try_get("sub_status")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let activation_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("activation_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let office_joining_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("office_joining_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let office_id: i64 = row
                    .try_get("office_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let transfer_to_office_id: Option<i64> = row
                    .try_get("transfer_to_office_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let staff_id: Option<i64> = row
                    .try_get("staff_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let firstname: Option<String> = row
                    .try_get("firstname")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let middlename: Option<String> = row
                    .try_get("middlename")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let lastname: Option<String> = row
                    .try_get("lastname")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let fullname: Option<String> = row
                    .try_get("fullname")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let display_name: String = row
                    .try_get("display_name")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let mobile_no: Option<String> = row
                    .try_get("mobile_no")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let email_address: Option<String> = row
                    .try_get("email_address")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let date_of_birth: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("date_of_birth")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let gender_cv_id: Option<i64> = row
                    .try_get("gender_cv_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let client_type_cv_id: Option<i64> = row
                    .try_get("client_type_cv_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let client_classification_cv_id: Option<i64> = row
                    .try_get("client_classification_cv_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let legal_form_raw: Option<i32> = row
                    .try_get("legal_form_enum")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let submitted_on_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("submittedon_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let closed_on_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("closedon_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let closure_reason_cv_id: Option<i64> = row
                    .try_get("closure_reason_cv_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let rejected_on_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("rejectedon_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let reject_reason_cv_id: Option<i64> = row
                    .try_get("reject_reason_cv_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let withdrawn_on_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("withdrawn_on_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let withdraw_reason_cv_id: Option<i64> = row
                    .try_get("withdraw_reason_cv_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let reactivated_on_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("reactivated_on_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let reopened_on_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("reopened_on_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let proposed_transfer_date: Option<NaiveDate> = row
                    .try_get::<Option<String>, _>("proposed_transfer_date")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?
                    .and_then(|s| parse_date_str(&s));
                let savings_product_id: Option<i64> = row
                    .try_get("default_savings_product")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let savings_account_id: Option<i64> = row
                    .try_get("default_savings_account")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let is_staff: bool = row
                    .try_get("is_staff")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
                let image_id: Option<i64> = row
                    .try_get("image_id")
                    .map_err(|e| FineractError::DatabaseError(e.to_string()))?;

                let client = Client {
                    id,
                    account_no,
                    external_id: ExternalId::from_opt(external_id_raw),
                    status: ClientStatus::from_i32(status_enum),
                    sub_status,
                    activation_date,
                    office_joining_date,
                    office_id,
                    transfer_to_office_id,
                    staff_id,
                    firstname,
                    middlename,
                    lastname,
                    fullname,
                    display_name,
                    mobile_no,
                    email_address,
                    date_of_birth,
                    gender_cv_id,
                    client_type_cv_id,
                    client_classification_cv_id,
                    legal_form: legal_form_raw
                        .and_then(LegalForm::from_i32)
                        .unwrap_or(LegalForm::Person),
                    submitted_on_date,
                    closed_on_date,
                    closure_reason_cv_id,
                    rejected_on_date,
                    reject_reason_cv_id,
                    withdrawn_on_date,
                    withdraw_reason_cv_id,
                    reactivated_on_date,
                    reopened_on_date,
                    proposed_transfer_date,
                    savings_product_id,
                    savings_account_id,
                    is_staff,
                    image_id,
                };
                self.clients.insert(id, client.clone());
                return Ok(client);
            }
        }
        Err(FineractError::resource_not_found(
            "error.msg.client.id.invalid",
            format!("Client with identifier {client_id} does not exist"),
            client_id.to_string(),
        ))
    }

    pub async fn find_client_with_not_found_detection(
        &self,
        client_id: i64,
    ) -> FineractResult<Client> {
        self.get_client(client_id).await
    }

    pub async fn find_client_by_id(&self, client_id: i64) -> FineractResult<Option<Client>> {
        match self.get_client(client_id).await {
            Ok(c) => Ok(Some(c)),
            Err(FineractError::ResourceNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Saves a new [`Client`] after allocating an ID, generating `account_no` if blank,
    /// deriving `display_name`, and checking unique constraints.
    pub async fn save_client(&self, mut client: Client) -> FineractResult<Client> {
        if client.id <= 0 {
            client.id = self.allocate_id();
        }
        if client.account_no.trim().is_empty() {
            client.account_no = Client::format_account_no(client.id);
        }
        client.derive_display_name();
        client.validate()?;
        self.check_client_uniqueness(&client)?;

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_client_sql(self.dialect);
            sqlx::query(&sql)
                .bind(client.id)
                .bind(&client.account_no)
                .bind(client.external_id.value())
                .bind(client.status.as_i32())
                .bind(client.sub_status)
                .bind(client.activation_date.map(|d| d.to_string()))
                .bind(client.office_joining_date.map(|d| d.to_string()))
                .bind(client.office_id)
                .bind(client.transfer_to_office_id)
                .bind(client.staff_id)
                .bind(&client.firstname)
                .bind(&client.middlename)
                .bind(&client.lastname)
                .bind(&client.fullname)
                .bind(&client.display_name)
                .bind(&client.mobile_no)
                .bind(&client.email_address)
                .bind(client.date_of_birth.map(|d| d.to_string()))
                .bind(client.gender_cv_id)
                .bind(client.client_type_cv_id)
                .bind(client.client_classification_cv_id)
                .bind(client.legal_form.as_i32())
                .bind(client.submitted_on_date.map(|d| d.to_string()))
                .bind(client.closed_on_date.map(|d| d.to_string()))
                .bind(client.closure_reason_cv_id)
                .bind(client.rejected_on_date.map(|d| d.to_string()))
                .bind(client.reject_reason_cv_id)
                .bind(client.withdrawn_on_date.map(|d| d.to_string()))
                .bind(client.withdraw_reason_cv_id)
                .bind(client.reactivated_on_date.map(|d| d.to_string()))
                .bind(client.reopened_on_date.map(|d| d.to_string()))
                .bind(client.proposed_transfer_date.map(|d| d.to_string()))
                .bind(client.savings_product_id)
                .bind(client.savings_account_id)
                .bind(client.is_staff)
                .bind(client.image_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.clients.insert(client.id, client.clone());
        Ok(client)
    }

    pub async fn create_client(&self, client: Client) -> FineractResult<Client> {
        self.save_client(client).await
    }

    /// Updates an existing [`Client`] in the repository.
    pub async fn update_client(&self, mut client: Client) -> FineractResult<Client> {
        client.derive_display_name();
        client.validate_activation_date()?;
        self.check_client_uniqueness(&client)?;

        if let Some(ref pool) = self.pool {
            let sql = Self::update_client_sql(self.dialect);
            sqlx::query(&sql)
                .bind(&client.account_no)
                .bind(client.external_id.value())
                .bind(client.status.as_i32())
                .bind(client.sub_status)
                .bind(client.activation_date.map(|d| d.to_string()))
                .bind(client.office_joining_date.map(|d| d.to_string()))
                .bind(client.office_id)
                .bind(client.transfer_to_office_id)
                .bind(client.staff_id)
                .bind(&client.firstname)
                .bind(&client.middlename)
                .bind(&client.lastname)
                .bind(&client.fullname)
                .bind(&client.display_name)
                .bind(&client.mobile_no)
                .bind(&client.email_address)
                .bind(client.date_of_birth.map(|d| d.to_string()))
                .bind(client.gender_cv_id)
                .bind(client.client_type_cv_id)
                .bind(client.client_classification_cv_id)
                .bind(client.legal_form.as_i32())
                .bind(client.submitted_on_date.map(|d| d.to_string()))
                .bind(client.closed_on_date.map(|d| d.to_string()))
                .bind(client.closure_reason_cv_id)
                .bind(client.rejected_on_date.map(|d| d.to_string()))
                .bind(client.reject_reason_cv_id)
                .bind(client.withdrawn_on_date.map(|d| d.to_string()))
                .bind(client.withdraw_reason_cv_id)
                .bind(client.reactivated_on_date.map(|d| d.to_string()))
                .bind(client.reopened_on_date.map(|d| d.to_string()))
                .bind(client.proposed_transfer_date.map(|d| d.to_string()))
                .bind(client.savings_product_id)
                .bind(client.savings_account_id)
                .bind(client.is_staff)
                .bind(client.image_id)
                .bind(client.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.clients.insert(client.id, client.clone());
        Ok(client)
    }

    /// Deletes a pending [`Client`] (`ClientMustBePendingToBeDeletedException` if not pending).
    pub async fn delete_client(&self, client_id: i64) -> FineractResult<()> {
        let client = self.get_client(client_id).await?;
        if client.is_not_pending() {
            return Err(FineractError::DomainRule(PlatformDomainRuleException::new(
                "error.msg.client.cannot.be.deleted",
                "Cannot delete client that is not in pending state.",
            )));
        }
        if let Some(ref pool) = self.pool {
            let del_np = Self::delete_client_non_person_sql(self.dialect);
            sqlx::query(&del_np)
                .bind(client_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
            let del_c = Self::delete_client_sql(self.dialect);
            sqlx::query(&del_c)
                .bind(client_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.non_persons.remove(&client_id);
        self.clients.remove(&client_id);
        Ok(())
    }

    pub fn list_clients(&self) -> Vec<ClientData> {
        let mut items: Vec<ClientData> = self
            .clients
            .iter()
            .map(|e| ClientData::from(e.value()))
            .collect();
        items.sort_by_key(|c| c.id);
        items
    }

    pub fn list_clients_by_office(&self, office_id: i64) -> Vec<ClientData> {
        let mut items: Vec<ClientData> = self
            .clients
            .iter()
            .filter(|e| e.value().office_id == office_id)
            .map(|e| ClientData::from(e.value()))
            .collect();
        items.sort_by_key(|c| c.id);
        items
    }

    // -------------------------------------------------------------------------
    // ClientNonPerson Operations
    // -------------------------------------------------------------------------

    pub async fn save_client_non_person(
        &self,
        mut non_person: ClientNonPerson,
    ) -> FineractResult<ClientNonPerson> {
        let client = self.get_client(non_person.client_id).await?;
        non_person.validate(client.date_of_birth)?;
        if non_person.id <= 0 {
            non_person.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_client_non_person_sql(self.dialect);
            sqlx::query(&sql)
                .bind(non_person.id)
                .bind(non_person.client_id)
                .bind(non_person.constitution_cv_id)
                .bind(&non_person.incorp_no)
                .bind(non_person.incorp_validity_till.map(|d| d.to_string()))
                .bind(non_person.main_business_line_cv_id)
                .bind(&non_person.remarks)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.non_persons
            .insert(non_person.client_id, non_person.clone());
        Ok(non_person)
    }

    pub async fn get_client_non_person_by_client_id(
        &self,
        client_id: i64,
    ) -> FineractResult<Option<ClientNonPerson>> {
        if let Some(entry) = self.non_persons.get(&client_id) {
            return Ok(Some(entry.value().clone()));
        }
        Ok(None)
    }

    pub async fn delete_client_non_person_by_client_id(
        &self,
        client_id: i64,
    ) -> FineractResult<()> {
        if let Some(ref pool) = self.pool {
            let sql = Self::delete_client_non_person_sql(self.dialect);
            sqlx::query(&sql)
                .bind(client_id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.non_persons.remove(&client_id);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // ClientIdentifier Operations
    // -------------------------------------------------------------------------

    pub async fn save_client_identifier(
        &self,
        mut identifier: ClientIdentifier,
    ) -> FineractResult<ClientIdentifier> {
        let _client = self.get_client(identifier.client_id).await?;
        identifier.active = identifier.status.active_column_value();
        identifier.validate()?;

        let all_existing: Vec<ClientIdentifier> =
            self.identifiers.iter().map(|e| e.value().clone()).collect();
        identifier.validate_uniqueness(&all_existing)?;

        if identifier.id <= 0 {
            identifier.id = self.allocate_id();
        }

        if let Some(ref pool) = self.pool {
            let sql = Self::insert_client_identifier_sql(self.dialect);
            sqlx::query(&sql)
                .bind(identifier.id)
                .bind(identifier.client_id)
                .bind(identifier.document_type_id)
                .bind(&identifier.document_key)
                .bind(identifier.status.as_i32())
                .bind(&identifier.description)
                .bind(identifier.issuance_date.map(|d| d.to_string()))
                .bind(identifier.expiry_date.map(|d| d.to_string()))
                .bind(identifier.active)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.identifiers.insert(identifier.id, identifier.clone());
        Ok(identifier)
    }

    pub async fn create_client_identifier(
        &self,
        identifier: ClientIdentifier,
    ) -> FineractResult<ClientIdentifier> {
        self.save_client_identifier(identifier).await
    }

    pub async fn update_client_identifier(
        &self,
        mut identifier: ClientIdentifier,
    ) -> FineractResult<ClientIdentifier> {
        identifier.active = identifier.status.active_column_value();
        identifier.validate()?;

        let all_existing: Vec<ClientIdentifier> =
            self.identifiers.iter().map(|e| e.value().clone()).collect();
        identifier.validate_uniqueness(&all_existing)?;

        if let Some(ref pool) = self.pool {
            let sql = Self::update_client_identifier_sql(self.dialect);
            sqlx::query(&sql)
                .bind(identifier.document_type_id)
                .bind(&identifier.document_key)
                .bind(identifier.status.as_i32())
                .bind(&identifier.description)
                .bind(identifier.issuance_date.map(|d| d.to_string()))
                .bind(identifier.expiry_date.map(|d| d.to_string()))
                .bind(identifier.active)
                .bind(identifier.id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }

        self.identifiers.insert(identifier.id, identifier.clone());
        Ok(identifier)
    }

    pub async fn get_client_identifier(&self, id: i64) -> FineractResult<ClientIdentifier> {
        if let Some(entry) = self.identifiers.get(&id) {
            return Ok(entry.value().clone());
        }
        Err(FineractError::resource_not_found(
            "error.msg.clientIdentifier.id.invalid",
            format!("Client Identifier with the primary key {id} does not exist"),
            id.to_string(),
        ))
    }

    pub async fn list_client_identifiers(
        &self,
        client_id: i64,
    ) -> FineractResult<Vec<ClientIdentifier>> {
        let mut items: Vec<ClientIdentifier> = self
            .identifiers
            .iter()
            .filter(|e| e.value().client_id == client_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|i| i.id);
        Ok(items)
    }

    pub async fn delete_client_identifier(&self, id: i64) -> FineractResult<()> {
        let _existing = self.get_client_identifier(id).await?;
        if let Some(ref pool) = self.pool {
            let sql = Self::delete_client_identifier_sql(self.dialect);
            sqlx::query(&sql)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.identifiers.remove(&id);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Address & ClientAddress Operations
    // -------------------------------------------------------------------------

    pub async fn save_address(&self, mut address: Address) -> FineractResult<Address> {
        if address.id <= 0 {
            address.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_address_sql(self.dialect);
            sqlx::query(&sql)
                .bind(address.id)
                .bind(&address.street)
                .bind(&address.address_line_1)
                .bind(&address.address_line_2)
                .bind(&address.address_line_3)
                .bind(&address.town_village)
                .bind(&address.city)
                .bind(&address.county_district)
                .bind(address.state_province_id)
                .bind(address.country_id)
                .bind(&address.postal_code)
                .bind(address.latitude.map(|d| d.to_string()))
                .bind(address.longitude.map(|d| d.to_string()))
                .bind(&address.created_by)
                .bind(address.created_on.map(|d| d.to_string()))
                .bind(&address.updated_by)
                .bind(address.updated_on.map(|d| d.to_string()))
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.addresses.insert(address.id, address.clone());
        Ok(address)
    }

    pub async fn get_address(&self, id: i64) -> FineractResult<Address> {
        if let Some(entry) = self.addresses.get(&id) {
            return Ok(entry.value().clone());
        }
        Err(FineractError::resource_not_found(
            "error.msg.address.id.invalid",
            format!("Address with identifier {id} does not exist"),
            id.to_string(),
        ))
    }

    pub async fn save_client_address(
        &self,
        mut client_address: ClientAddress,
    ) -> FineractResult<ClientAddress> {
        let _client = self.get_client(client_address.client_id).await?;
        if client_address.id <= 0 {
            client_address.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_client_address_sql(self.dialect);
            sqlx::query(&sql)
                .bind(client_address.id)
                .bind(client_address.client_id)
                .bind(client_address.address_id)
                .bind(client_address.address_type_id)
                .bind(client_address.is_active)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.client_addresses
            .insert(client_address.id, client_address.clone());
        Ok(client_address)
    }

    pub async fn list_client_addresses(
        &self,
        client_id: i64,
    ) -> FineractResult<Vec<ClientAddress>> {
        let mut items: Vec<ClientAddress> = self
            .client_addresses
            .iter()
            .filter(|e| e.value().client_id == client_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|ca| ca.id);
        Ok(items)
    }

    // -------------------------------------------------------------------------
    // ClientFamilyMember Operations
    // -------------------------------------------------------------------------

    pub async fn save_family_member(
        &self,
        mut member: ClientFamilyMember,
    ) -> FineractResult<ClientFamilyMember> {
        let _client = self.get_client(member.client_id).await?;
        if member.id <= 0 {
            member.id = self.allocate_id();
        }
        if let Some(ref pool) = self.pool {
            let sql = Self::insert_family_member_sql(self.dialect);
            sqlx::query(&sql)
                .bind(member.id)
                .bind(member.client_id)
                .bind(&member.firstname)
                .bind(&member.middlename)
                .bind(&member.lastname)
                .bind(&member.qualification)
                .bind(&member.mobile_number)
                .bind(member.age)
                .bind(member.is_dependent)
                .bind(member.relationship_cv_id)
                .bind(member.marital_status_cv_id)
                .bind(member.gender_cv_id)
                .bind(member.profession_cv_id)
                .bind(member.date_of_birth.map(|d| d.to_string()))
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.family_members.insert(member.id, member.clone());
        Ok(member)
    }

    pub async fn get_family_member(&self, id: i64) -> FineractResult<ClientFamilyMember> {
        if let Some(entry) = self.family_members.get(&id) {
            return Ok(entry.value().clone());
        }
        Err(FineractError::resource_not_found(
            "error.msg.familymember.id.invalid",
            format!("Family member with identifier {id} does not exist"),
            id.to_string(),
        ))
    }

    pub async fn list_family_members(
        &self,
        client_id: i64,
    ) -> FineractResult<Vec<ClientFamilyMember>> {
        let mut items: Vec<ClientFamilyMember> = self
            .family_members
            .iter()
            .filter(|e| e.value().client_id == client_id)
            .map(|e| e.value().clone())
            .collect();
        items.sort_by_key(|m| m.id);
        Ok(items)
    }

    pub async fn delete_family_member(&self, id: i64) -> FineractResult<()> {
        let _existing = self.get_family_member(id).await?;
        if let Some(ref pool) = self.pool {
            let sql = Self::delete_family_member_sql(self.dialect);
            sqlx::query(&sql)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| FineractError::DatabaseError(e.to_string()))?;
        }
        self.family_members.remove(&id);
        Ok(())
    }
}

// =============================================================================
// 8. COMMAND HANDLERS
// =============================================================================

fn resolve_target_client_id(command: &JsonCommand) -> FineractResult<i64> {
    command
        .client_id
        .or(command.entity_id)
        .ok_or_else(|| {
            FineractError::resource_not_found(
                "error.msg.client.id.invalid",
                "Client identifier is required for command execution",
                "null",
            )
        })
}

/// Command handler for `CREATE CLIENT` (`org.apache.fineract.portfolio.client.handler.CreateClientCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateClientCommandHandler {
    repo: Arc<DualDbClientRepository>,
}

impl CreateClientCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "CLIENT";

    pub fn new(repo: Arc<DualDbClientRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbClientRepository::new_in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbClientRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateClientCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let id = self.repo.allocate_id();
        let client = Client::from_json(id, command)?;
        let group_id = command.long_value_of_parameter_named("groupId")?;
        let saved = self.repo.save_client(client).await?;

        // Extract and persist ClientNonPerson details if present
        if saved.legal_form.is_entity() {
            if let Some(np_val) = command.json_element("clientNonPersonDetails") {
                if np_val.is_object() {
                    let np_cmd = JsonCommand::from_value(Some(saved.id), np_val.clone());
                    if let Some(constitution_id) =
                        np_cmd.long_value_of_parameter_named("constitutionId")?
                    {
                        let main_business_line_id =
                            np_cmd.long_value_of_parameter_named("mainBusinessLineId")?;
                        let incorp_no = np_cmd.string_value_of_parameter_named("incorpNumber");
                        let remarks = np_cmd.string_value_of_parameter_named("remarks");
                        let incorp_validity_till =
                            np_cmd.local_date_value_of_parameter_named("incorpValidityTillDate")?;
                        let np = ClientNonPerson::create_new(
                            self.repo.allocate_id(),
                            saved.id,
                            constitution_id,
                            main_business_line_id,
                            incorp_no,
                            incorp_validity_till,
                            remarks,
                            saved.date_of_birth,
                        )?;
                        self.repo.save_client_non_person(np).await?;
                    }
                }
            }
        }

        // Persist inline addresses if provided
        if let Some(Value::Array(addr_items)) = command.json_element("address") {
            for item in addr_items {
                if item.is_object() {
                    let addr_cmd = JsonCommand::from_value(None, item.clone());
                    let address = Address::from_json(self.repo.allocate_id(), &addr_cmd)?;
                    let saved_addr = self.repo.save_address(address).await?;
                    let addr_type_id = addr_cmd
                        .long_value_of_parameter_named("addressTypeId")?
                        .unwrap_or(1);
                    let is_active = addr_cmd.boolean_primitive_value_of_parameter_named("isActive");
                    let client_addr = ClientAddress::new(
                        self.repo.allocate_id(),
                        saved.id,
                        saved_addr.id,
                        addr_type_id,
                        is_active,
                    );
                    self.repo.save_client_address(client_addr).await?;
                }
            }
        }

        // Persist inline family members if provided
        if let Some(Value::Array(fm_items)) = command.json_element("familyMembers") {
            for item in fm_items {
                if item.is_object() {
                    let fm_cmd = JsonCommand::from_value(None, item.clone());
                    let fm =
                        ClientFamilyMember::from_json(self.repo.allocate_id(), saved.id, &fm_cmd)?;
                    self.repo.save_family_member(fm).await?;
                }
            }
        }

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_group_id_opt(group_id)
            .with_client_id(saved.id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `ACTIVATE CLIENT` (`org.apache.fineract.portfolio.client.handler.ActivateClientCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct ActivateClientCommandHandler {
    repo: Arc<DualDbClientRepository>,
}

impl ActivateClientCommandHandler {
    pub const ACTION_NAME: &'static str = "ACTIVATE";
    pub const ENTITY_NAME: &'static str = "CLIENT";

    pub fn new(repo: Arc<DualDbClientRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbClientRepository::new_in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbClientRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for ActivateClientCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_id = resolve_target_client_id(command)?;
        let activation_date = command
            .local_date_value_of_parameter_named("activationDate")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.client.activationDate.cannot.be.blank",
                        "The parameter `activationDate` is mandatory.",
                        "activationDate",
                        vec![],
                    ),
                )
            })?;

        let mut client = self.repo.get_client(client_id).await?;
        client.activate(activation_date)?;
        let saved = self.repo.update_client(client).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_client_id(saved.id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `CLOSE CLIENT` (`org.apache.fineract.portfolio.client.handler.CloseClientCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CloseClientCommandHandler {
    repo: Arc<DualDbClientRepository>,
}

impl CloseClientCommandHandler {
    pub const ACTION_NAME: &'static str = "CLOSE";
    pub const ENTITY_NAME: &'static str = "CLIENT";

    pub fn new(repo: Arc<DualDbClientRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbClientRepository::new_in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbClientRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CloseClientCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_id = resolve_target_client_id(command)?;
        let closure_date = command
            .local_date_value_of_parameter_named("closureDate")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.client.closureDate.cannot.be.blank",
                        "The parameter `closureDate` is mandatory.",
                        "closureDate",
                        vec![],
                    ),
                )
            })?;
        let closure_reason_id = command
            .long_value_of_parameter_named("closureReasonId")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.client.closureReasonId.cannot.be.blank",
                        "The parameter `closureReasonId` is mandatory.",
                        "closureReasonId",
                        vec![],
                    ),
                )
            })?;

        let mut client = self.repo.get_client(client_id).await?;
        client.close(closure_date, closure_reason_id)?;
        let saved = self.repo.update_client(client).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_client_id(saved.id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `REJECT CLIENT` (`org.apache.fineract.portfolio.client.handler.RejectClientCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct RejectClientCommandHandler {
    repo: Arc<DualDbClientRepository>,
}

impl RejectClientCommandHandler {
    pub const ACTION_NAME: &'static str = "REJECT";
    pub const ENTITY_NAME: &'static str = "CLIENT";

    pub fn new(repo: Arc<DualDbClientRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbClientRepository::new_in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbClientRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for RejectClientCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_id = resolve_target_client_id(command)?;
        let rejection_date = command
            .local_date_value_of_parameter_named("rejectionDate")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.client.rejectionDate.cannot.be.blank",
                        "The parameter `rejectionDate` is mandatory.",
                        "rejectionDate",
                        vec![],
                    ),
                )
            })?;
        let rejection_reason_id = command
            .long_value_of_parameter_named("rejectionReasonId")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.client.rejectionReasonId.cannot.be.blank",
                        "The parameter `rejectionReasonId` is mandatory.",
                        "rejectionReasonId",
                        vec![],
                    ),
                )
            })?;

        let mut client = self.repo.get_client(client_id).await?;
        client.reject(rejection_date, rejection_reason_id)?;
        let saved = self.repo.update_client(client).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_client_id(saved.id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `WITHDRAW CLIENT` (`org.apache.fineract.portfolio.client.handler.WithdrawClientCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct WithdrawClientCommandHandler {
    repo: Arc<DualDbClientRepository>,
}

impl WithdrawClientCommandHandler {
    pub const ACTION_NAME: &'static str = "WITHDRAW";
    pub const ENTITY_NAME: &'static str = "CLIENT";

    pub fn new(repo: Arc<DualDbClientRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbClientRepository::new_in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbClientRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for WithdrawClientCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_id = resolve_target_client_id(command)?;
        let withdrawal_date = command
            .local_date_value_of_parameter_named("withdrawalDate")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.client.withdrawalDate.cannot.be.blank",
                        "The parameter `withdrawalDate` is mandatory.",
                        "withdrawalDate",
                        vec![],
                    ),
                )
            })?;
        let withdrawal_reason_id = command
            .long_value_of_parameter_named("withdrawalReasonId")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.client.withdrawalReasonId.cannot.be.blank",
                        "The parameter `withdrawalReasonId` is mandatory.",
                        "withdrawalReasonId",
                        vec![],
                    ),
                )
            })?;

        let mut client = self.repo.get_client(client_id).await?;
        client.withdraw(withdrawal_date, withdrawal_reason_id)?;
        let saved = self.repo.update_client(client).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_client_id(saved.id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `REACTIVATE CLIENT` (`org.apache.fineract.portfolio.client.handler.ReactivateClientCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct ReactivateClientCommandHandler {
    repo: Arc<DualDbClientRepository>,
}

impl ReactivateClientCommandHandler {
    pub const ACTION_NAME: &'static str = "REACTIVATE";
    pub const ENTITY_NAME: &'static str = "CLIENT";

    pub fn new(repo: Arc<DualDbClientRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbClientRepository::new_in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbClientRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for ReactivateClientCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_id = resolve_target_client_id(command)?;
        let reactivate_date = command
            .local_date_value_of_parameter_named("reactivationDate")?
            .ok_or_else(|| {
                FineractError::DataValidation(
                    PlatformApiDataValidationException::single_parameter_error(
                        "validation.msg.client.reactivationDate.cannot.be.blank",
                        "The parameter `reactivationDate` is mandatory.",
                        "reactivationDate",
                        vec![],
                    ),
                )
            })?;

        let mut client = self.repo.get_client(client_id).await?;
        client.reactivate(reactivate_date)?;
        let saved = self.repo.update_client(client).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(saved.office_id)
            .with_client_id(saved.id)
            .with_entity_id(saved.id)
            .with_resource_external_id(saved.external_id)
            .build())
    }
}

/// Command handler for `CREATE CLIENTIDENTIFIER` (`org.apache.fineract.portfolio.client.handler.CreateClientIdentifierCommandHandler`).
#[derive(Debug, Clone, Default)]
pub struct CreateClientIdentifierCommandHandler {
    repo: Arc<DualDbClientRepository>,
}

impl CreateClientIdentifierCommandHandler {
    pub const ACTION_NAME: &'static str = "CREATE";
    pub const ENTITY_NAME: &'static str = "CLIENTIDENTIFIER";

    pub fn new(repo: Arc<DualDbClientRepository>) -> Self {
        Self { repo }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(DualDbClientRepository::new_in_memory()))
    }

    pub fn repository(&self) -> &Arc<DualDbClientRepository> {
        &self.repo
    }
}

#[async_trait]
impl CommandHandler for CreateClientIdentifierCommandHandler {
    async fn handle(
        &self,
        _ctx: &ExecutionContext,
        command: &JsonCommand,
    ) -> Result<CommandProcessingResult, FineractError> {
        let client_id = resolve_target_client_id(command)?;
        let client = self.repo.get_client(client_id).await?;
        let id = self.repo.allocate_id();
        let identifier = ClientIdentifier::from_json(id, client_id, command)?;
        let saved = self.repo.save_client_identifier(identifier).await?;

        Ok(CommandProcessingResultBuilder::new()
            .with_command_id_opt(command.command_id)
            .with_office_id(client.office_id)
            .with_client_id(client_id)
            .with_entity_id(saved.id)
            .build())
    }
}

// =============================================================================
// 9. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid test date")
    }

    #[test]
    fn client_status_discriminants_and_predicates_match_spec() {
        assert_eq!(ClientStatus::Invalid as i16, 0);
        assert_eq!(ClientStatus::Pending as i16, 100);
        assert_eq!(ClientStatus::Active as i16, 300);
        assert_eq!(ClientStatus::TransferInProgress as i16, 303);
        assert_eq!(ClientStatus::TransferOnHold as i16, 304);
        assert_eq!(ClientStatus::Withdrawn as i16, 400);
        assert_eq!(ClientStatus::Rejected as i16, 500);
        assert_eq!(ClientStatus::Closed as i16, 600);
        assert_eq!(ClientStatus::Reopened as i16, 700);

        assert!(ClientStatus::from_i16(300).is_active());
        assert!(ClientStatus::from_i16(303).is_under_transfer());
        assert!(ClientStatus::from_i16(304).is_under_transfer());
        assert!(ClientStatus::from_i16(400).is_withdrawn());
        assert!(ClientStatus::from_i16(500).is_rejected());
        assert!(ClientStatus::from_i16(600).is_closed());
        assert!(ClientStatus::from_i16(700).is_reopened());
        assert_eq!(LegalForm::Person as i16, 1);
        assert_eq!(LegalForm::Entity as i16, 2);
    }

    #[test]
    fn derive_display_name_for_person_and_entity() {
        let mut person = Client::new_person(
            1,
            1,
            "Ada",
            Some("Augusta".to_string()),
            "Lovelace",
            ymd(2025, 1, 10),
            ExternalId::new("EXT-ADA"),
        );
        assert_eq!(person.derive_display_name(), "Ada Augusta Lovelace");
        person.middlename = None;
        assert_eq!(person.derive_display_name(), "Ada Lovelace");

        let entity = Client::new_entity(
            2,
            1,
            "Analytical Engine Corp",
            ymd(2025, 1, 10),
            ExternalId::new("EXT-CORP"),
        );
        assert_eq!(entity.display_name, "Analytical Engine Corp");
    }

    #[test]
    fn client_lifecycle_activate_close_reactivate_and_transfers() {
        let mut client = Client::new_person(
            10,
            1,
            "Grace",
            None,
            "Hopper",
            ymd(2025, 1, 5),
            ExternalId::empty(),
        );
        assert_eq!(client.status, ClientStatus::Pending);

        // Activation before submitted_on_date fails
        assert!(client.activate(ymd(2025, 1, 1)).is_err());

        // Valid activation
        client.activate(ymd(2025, 1, 10)).expect("activate");
        assert_eq!(client.status, ClientStatus::Active);
        assert_eq!(client.activation_date, Some(ymd(2025, 1, 10)));
        assert_eq!(client.office_joining_date, Some(ymd(2025, 1, 10)));

        // Propose transfer to same office fails
        assert!(client.propose_transfer(1).is_err());

        // Propose transfer to branch 2
        client.propose_transfer(2).expect("propose transfer");
        assert_eq!(client.status, ClientStatus::TransferInProgress);
        assert_eq!(client.transfer_to_office_id, Some(2));

        // Closing while under transfer fails
        assert!(client.close(ymd(2025, 2, 1), 11).is_err());

        // Reject transfer -> TransferOnHold
        client.reject_transfer().expect("reject transfer");
        assert_eq!(client.status, ClientStatus::TransferOnHold);
        assert_eq!(client.transfer_to_office_id, None);

        // Withdraw transfer -> Active, then propose & accept
        client.withdraw_transfer().expect("withdraw transfer");
        client.propose_transfer(3).expect("propose transfer to 3");
        client
            .accept_transfer(ymd(2025, 2, 15))
            .expect("accept transfer");
        assert_eq!(client.status, ClientStatus::Active);
        assert_eq!(client.office_id, 3);
        assert_eq!(client.office_joining_date, Some(ymd(2025, 2, 15)));

        // Close before activation date fails
        assert!(client.close(ymd(2025, 1, 9), 15).is_err());

        // Valid close
        client.close(ymd(2025, 3, 1), 15).expect("close client");
        assert_eq!(client.status, ClientStatus::Closed);
        assert_eq!(client.closed_on_date, Some(ymd(2025, 3, 1)));

        // Reactivate before closure date fails
        assert!(client.reactivate(ymd(2025, 2, 20)).is_err());

        // Valid reactivate returns to Pending and clears closed_on_date
        client
            .reactivate(ymd(2025, 3, 5))
            .expect("reactivate client");
        assert_eq!(client.status, ClientStatus::Pending);
        assert_eq!(client.closed_on_date, None);
        assert_eq!(client.reactivated_on_date, Some(ymd(2025, 3, 5)));
    }

    #[test]
    fn client_reject_and_withdraw_guards() {
        let mut c1 = Client::new_person(
            1,
            1,
            "Alan",
            None,
            "Turing",
            ymd(2025, 2, 10),
            ExternalId::empty(),
        );
        // Reject before submitted date fails
        assert!(c1.reject(ymd(2025, 2, 1), 100).is_err());
        c1.reject(ymd(2025, 2, 12), 100).expect("reject");
        assert_eq!(c1.status, ClientStatus::Rejected);
        assert_eq!(c1.rejected_on_date, Some(ymd(2025, 2, 12)));

        let mut c2 = Client::new_person(
            2,
            1,
            "Joan",
            None,
            "Clarke",
            ymd(2025, 2, 10),
            ExternalId::empty(),
        );
        c2.withdraw(ymd(2025, 2, 15), 200).expect("withdraw");
        assert_eq!(c2.status, ClientStatus::Withdrawn);
        assert_eq!(c2.withdrawn_on_date, Some(ymd(2025, 2, 15)));
    }

    #[test]
    fn client_identifier_uniqueness_and_active_status() {
        let id1 = ClientIdentifier::new(
            1,
            10,
            5,
            "PASS-12345",
            ClientIdentifierStatus::Active,
            Some("Passport".to_string()),
            Some(ymd(2020, 1, 1)),
            Some(ymd(2030, 1, 1)),
        )
        .expect("id1");
        assert_eq!(id1.active, Some(200));

        // Duplicate (document_type_id, document_key) across any client fails
        let dup_key = ClientIdentifier::new(
            2,
            99,
            5,
            "pass-12345",
            ClientIdentifierStatus::Inactive,
            None,
            None,
            None,
        )
        .expect("dup_key");
        assert!(dup_key
            .validate_uniqueness(std::slice::from_ref(&id1))
            .is_err());

        // Duplicate active document_type_id for the same client fails
        let dup_active_type = ClientIdentifier::new(
            3,
            10,
            5,
            "PASS-99999",
            ClientIdentifierStatus::Active,
            None,
            None,
            None,
        )
        .expect("dup_active_type");
        assert!(dup_active_type
            .validate_uniqueness(std::slice::from_ref(&id1))
            .is_err());

        // Inactive identifier of same document_type_id with different key for same client succeeds
        let inactive_same_type = ClientIdentifier::new(
            4,
            10,
            5,
            "PASS-OLD-001",
            ClientIdentifierStatus::Inactive,
            None,
            None,
            None,
        )
        .expect("inactive");
        assert_eq!(inactive_same_type.active, None);
        inactive_same_type
            .validate_uniqueness(std::slice::from_ref(&id1))
            .expect("inactive allowed");
    }

    #[tokio::test]
    async fn command_handlers_end_to_end_in_memory() {
        let repo = Arc::new(DualDbClientRepository::new_in_memory());
        let ctx = ExecutionContext::new("default", ymd(2025, 4, 1));

        let create_handler = CreateClientCommandHandler::new(repo.clone());
        let create_cmd = JsonCommand::from_json(
            r#"{
                "officeId": 1,
                "firstname": "Katherine",
                "lastname": "Johnson",
                "externalId": "KJ-001",
                "dateFormat": "yyyy-MM-dd",
                "locale": "en",
                "submittedOnDate": "2025-03-01",
                "active": false,
                "address": [
                    {
                        "addressTypeId": 1,
                        "street": "2100 NASA Pkwy",
                        "city": "Hampton",
                        "isActive": true
                    }
                ],
                "familyMembers": [
                    {
                        "firstName": "James",
                        "lastName": "Goble",
                        "age": 42,
                        "isDependent": false
                    }
                ]
            }"#,
        )
        .expect("valid json");

        let res = create_handler
            .handle(&ctx, &create_cmd)
            .await
            .expect("create client");
        let client_id = res.client_id.expect("client_id");

        let created = repo.get_client(client_id).await.expect("get client");
        assert_eq!(created.display_name, "Katherine Johnson");
        assert_eq!(created.status, ClientStatus::Pending);
        assert_eq!(
            repo.list_client_addresses(client_id)
                .await
                .expect("addresses")
                .len(),
            1
        );
        assert_eq!(
            repo.list_family_members(client_id)
                .await
                .expect("family")
                .len(),
            1
        );

        // Activate Client
        let activate_handler = ActivateClientCommandHandler::new(repo.clone());
        let activate_cmd = JsonCommand::from_json(
            r#"{"activationDate": "2025-03-10", "dateFormat": "yyyy-MM-dd", "locale": "en"}"#,
        )
        .expect("valid json")
        .with_client_id(client_id);
        activate_handler
            .handle(&ctx, &activate_cmd)
            .await
            .expect("activate client");
        assert_eq!(
            repo.get_client(client_id).await.expect("active").status,
            ClientStatus::Active
        );

        // Create ClientIdentifier
        let id_handler = CreateClientIdentifierCommandHandler::new(repo.clone());
        let id_cmd = JsonCommand::from_json(
            r#"{"documentTypeId": 1, "documentKey": "NASA-BADGE-01", "status": "Active"}"#,
        )
        .expect("valid json")
        .with_client_id(client_id);
        id_handler
            .handle(&ctx, &id_cmd)
            .await
            .expect("create identifier");
        // Duplicate key fails
        assert!(id_handler.handle(&ctx, &id_cmd).await.is_err());

        // Close & Reactivate Client
        let close_handler = CloseClientCommandHandler::new(repo.clone());
        let close_cmd = JsonCommand::from_json(
            r#"{"closureDate": "2025-03-20", "closureReasonId": 10, "dateFormat": "yyyy-MM-dd", "locale": "en"}"#,
        )
        .expect("valid json")
        .with_client_id(client_id);
        close_handler
            .handle(&ctx, &close_cmd)
            .await
            .expect("close client");
        assert_eq!(
            repo.get_client(client_id).await.expect("closed").status,
            ClientStatus::Closed
        );

        let reactivate_handler = ReactivateClientCommandHandler::new(repo.clone());
        let reactivate_cmd = JsonCommand::from_json(
            r#"{"reactivationDate": "2025-03-25", "dateFormat": "yyyy-MM-dd", "locale": "en"}"#,
        )
        .expect("valid json")
        .with_client_id(client_id);
        reactivate_handler
            .handle(&ctx, &reactivate_cmd)
            .await
            .expect("reactivate client");
        assert_eq!(
            repo.get_client(client_id)
                .await
                .expect("reactivated")
                .status,
            ClientStatus::Pending
        );
    }
}
