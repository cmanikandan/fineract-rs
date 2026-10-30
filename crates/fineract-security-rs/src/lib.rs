//! `fineract-security-rs` — Apache Fineract Security, Authentication & RBAC Layer
//!
//! Ported from:
//! - `org.apache.fineract.useradministration.domain` (`AppUser`, `Role`, `Permission`)
//! - `org.apache.fineract.infrastructure.security.filter` (`TenantAwareBasicAuthenticationFilter`,
//!   `TenantAwareAuthenticationFilter`, `TwoFactorAuthenticationFilter`)
//! - `org.apache.fineract.infrastructure.security.converter` (`FineractJwtAuthenticationTokenConverter`,
//!   `FineractOidcJwtAuthenticationConverter`)
//! - `org.apache.fineract.infrastructure.security.domain` (`TFAccessToken`, `TwoFactorConfiguration`)
//! - `org.apache.fineract.infrastructure.security.service` (`TwoFactorServiceImpl`, `RandomOTPGenerator`,
//!   `SpringSecurityPlatformSecurityContext`)
//! - `org.apache.fineract.infrastructure.core.config.SecurityConfig`

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use fineract_core_rs::{
    ActionContext, BusinessDateType, ExecutionContext, FineractPlatformTenant,
};
use fineract_validation_rs::FineractError;
use hmac::{Hmac, Mac};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

// =============================================================================
// SECURITY CONSTANTS (Ported from TenantConstants & TwoFactorConstants)
// =============================================================================

/// Required multi-tenant HTTP request header (`TenantAwareBasicAuthenticationFilter.java`).
pub const TENANT_ID_REQUEST_HEADER: &str = "Fineract-Platform-TenantId";

/// Two-factor authentication token HTTP request header (`TwoFactorAuthenticationFilter.java`).
pub const TFA_TOKEN_REQUEST_HEADER: &str = "Fineract-Platform-TFA-Token";

/// Permission code that allows a user to bypass two-factor authentication (`TwoFactorConstants.java`).
pub const BYPASS_TWO_FACTOR_PERMISSION: &str = "BYPASS_TWOFACTOR";

/// Authority added to the security context once 2FA succeeds (`TwoFactorAuthenticationFilter.java`).
pub const TWOFACTOR_AUTHENTICATED_AUTHORITY: &str = "TWOFACTOR_AUTHENTICATED";

/// Superuser permission granting all platform capabilities (`AppUser.java`).
pub const ALL_FUNCTIONS_PERMISSION: &str = "ALL_FUNCTIONS";

/// Read-only superuser permission granting all `READ_*` capabilities (`AppUser.java`).
pub const ALL_FUNCTIONS_READ_PERMISSION: &str = "ALL_FUNCTIONS_READ";

/// Write superuser permission granting `CREATE_*`, `UPDATE_*`, `DELETE_*` (`SecurityConfig.java`).
pub const ALL_FUNCTIONS_WRITE_PERMISSION: &str = "ALL_FUNCTIONS_WRITE";

/// Maker-checker superuser permission granting all `*_CHECKER` capabilities (`AppUser.java`).
pub const CHECKER_SUPER_USER_PERMISSION: &str = "CHECKER_SUPER_USER";

/// Allowed OTP characters matching `RandomOTPGenerator.java`.
const ALLOWED_OTP_CHARACTERS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVQXYZ";

// =============================================================================
// SECURITY ERROR & AXUM HTTP RESPONSE MAPPING
// =============================================================================

/// Security-layer error mapping directly to HTTP status codes (`400`, `401`, `403`)
/// and [`FineractError`].
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecurityError {
    /// `400 Bad Request` when `Fineract-Platform-TenantId` header is missing or blank.
    #[error("No tenant identifier found: Add request header of 'Fineract-Platform-TenantId' or add the parameter 'tenantIdentifier' to query string of request URL.")]
    MissingTenantIdentifier,

    /// `400 Bad Request` when the requested tenant identifier is unknown or inactive.
    #[error("Invalid tenant identifier: {0}")]
    InvalidTenantIdentifier(String),

    /// `401 Unauthorized` when credentials or tokens are missing, malformed, expired, or invalid.
    #[error("Unauthenticated: {0}")]
    Unauthenticated(String),

    /// `401 Unauthorized` when an invalid/expired `Fineract-Platform-TFA-Token` is provided.
    #[error("Invalid two-factor access token provided")]
    InvalidTwoFactorAccessToken,

    /// `403 Forbidden` when 2FA is required on the endpoint or RBAC permission check fails.
    #[error("Access denied: {0}")]
    AccessDenied(String),

    /// `403 Forbidden` when a user's password must be reset before executing commands.
    #[error("Password reset required for user ID {0}")]
    PasswordResetRequired(i64),
}

impl SecurityError {
    /// Returns the wire-compatible HTTP [`StatusCode`] for this security error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::MissingTenantIdentifier | Self::InvalidTenantIdentifier(_) => {
                StatusCode::BAD_REQUEST
            }
            Self::Unauthenticated(_) | Self::InvalidTwoFactorAccessToken => {
                StatusCode::UNAUTHORIZED
            }
            Self::AccessDenied(_) | Self::PasswordResetRequired(_) => StatusCode::FORBIDDEN,
        }
    }

    /// Converts this [`SecurityError`] into a [`FineractError`].
    pub fn into_fineract_error(self) -> FineractError {
        match self {
            Self::MissingTenantIdentifier
            | Self::InvalidTenantIdentifier(_)
            | Self::Unauthenticated(_)
            | Self::InvalidTwoFactorAccessToken => FineractError::Unauthenticated,
            Self::AccessDenied(_) | Self::PasswordResetRequired(_) => FineractError::AccessDenied,
        }
    }
}

impl From<SecurityError> for FineractError {
    fn from(err: SecurityError) -> Self {
        err.into_fineract_error()
    }
}

impl IntoResponse for SecurityError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let www_authenticate = match &self {
            Self::MissingTenantIdentifier
            | Self::InvalidTenantIdentifier(_)
            | Self::Unauthenticated(_) => Some("Basic realm=\"Fineract Platform API\""),
            Self::InvalidTwoFactorAccessToken => {
                Some("Basic realm=\"Fineract Platform API Two Factor\"")
            }
            _ => None,
        };

        let body = serde_json::json!({
            "developerMessage": self.to_string(),
            "httpStatusCode": status.as_u16().to_string(),
            "defaultUserMessage": self.to_string(),
            "userMessageGlobalisationCode": match &self {
                Self::MissingTenantIdentifier | Self::InvalidTenantIdentifier(_) => "error.msg.invalid.tenant.identifier",
                Self::Unauthenticated(_) => "error.msg.not.authenticated",
                Self::InvalidTwoFactorAccessToken => "error.msg.twofactor.access.token.invalid",
                Self::AccessDenied(_) => "error.msg.not.authorized",
                Self::PasswordResetRequired(_) => "error.msg.password.outdated",
            },
            "errors": []
        });

        let mut response = (status, Json(body)).into_response();
        if let Some(realm) = www_authenticate {
            if let Ok(val) = HeaderValue::from_str(realm) {
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, val);
            }
        }
        response
    }
}

// =============================================================================
// 1. DOMAIN MODELS: m_permission, m_role, m_appuser
// =============================================================================

/// Fine-grained platform permission corresponding to table `m_permission`.
///
/// Ported from `org.apache.fineract.useradministration.domain.Permission`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub id: i64,
    pub grouping: String,
    pub code: String,
    pub entity_name: Option<String>,
    pub action_name: Option<String>,
    pub can_maker_checker: bool,
}

impl Permission {
    /// Creates a new [`Permission`] matching `m_permission` columns.
    pub fn new(
        id: i64,
        grouping: impl Into<String>,
        code: impl Into<String>,
        entity_name: Option<String>,
        action_name: Option<String>,
        can_maker_checker: bool,
    ) -> Self {
        Self {
            id,
            grouping: grouping.into(),
            code: code.into(),
            entity_name,
            action_name,
            can_maker_checker,
        }
    }

    /// Creates an entity-scoped [`Permission`] with code `{action_name}_{entity_name}`
    /// matching `new Permission(grouping, entityName, actionName)` in `Permission.java`.
    pub fn for_entity(
        id: i64,
        grouping: impl Into<String>,
        entity_name: impl Into<String>,
        action_name: impl Into<String>,
        can_maker_checker: bool,
    ) -> Self {
        let entity = entity_name.into();
        let action = action_name.into();
        let code = format!("{action}_{entity}");
        Self {
            id,
            grouping: grouping.into(),
            code,
            entity_name: Some(entity),
            action_name: Some(action),
            can_maker_checker,
        }
    }

    /// Creates a special grouping permission (e.g., `ALL_FUNCTIONS`, `ALL_FUNCTIONS_READ`,
    /// `CHECKER_SUPER_USER`, `BYPASS_TWOFACTOR`).
    pub fn special(id: i64, code: impl Into<String>) -> Self {
        Self {
            id,
            grouping: "special".to_string(),
            code: code.into(),
            entity_name: None,
            action_name: None,
            can_maker_checker: false,
        }
    }

    /// Case-insensitive code check matching `Permission.hasCode(checkCode)`.
    pub fn has_code(&self, check_code: &str) -> bool {
        self.code.eq_ignore_ascii_case(check_code.trim())
    }

    /// Returns whether maker-checker can be enabled on this permission (`hasMakerCheckerEnabled()`).
    pub const fn has_maker_checker_enabled(&self) -> bool {
        self.can_maker_checker
    }

    /// Toggles `can_maker_checker` and returns `true` if the value changed (`enableMakerChecker`).
    pub fn enable_maker_checker(&mut self, can_maker_checker: bool) -> bool {
        let changed = self.can_maker_checker != can_maker_checker;
        self.can_maker_checker = can_maker_checker;
        changed
    }
}

/// DTO projection of a [`Role`] matching `org.apache.fineract.useradministration.data.RoleData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleData {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub disabled: bool,
}

/// User role aggregate corresponding to table `m_role` and join table `m_role_permission`.
///
/// Ported from `org.apache.fineract.useradministration.domain.Role`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_disabled: bool,
    pub permissions: Vec<Permission>,
}

impl Role {
    /// Creates an enabled [`Role`] with the provided permissions.
    pub fn new(
        id: i64,
        name: impl Into<String>,
        description: impl Into<String>,
        permissions: Vec<Permission>,
    ) -> Self {
        Self {
            id,
            name: name.into().trim().to_string(),
            description: description.into().trim().to_string(),
            is_disabled: false,
            permissions,
        }
    }

    /// Checks whether this role is active (not disabled) and contains `permission_code`,
    /// matching `Role.hasPermissionTo(permissionCode)`.
    pub fn has_permission_to(&self, permission_code: &str) -> bool {
        if self.is_disabled {
            return false;
        }
        self.permissions
            .iter()
            .any(|p| p.has_code(permission_code))
    }

    /// Adds or removes a permission matching `Role.updatePermission(permission, isSelected)`.
    pub fn update_permission(&mut self, permission: Permission, is_selected: bool) -> bool {
        if is_selected {
            if self.permissions.iter().any(|p| p.has_code(&permission.code)) {
                false
            } else {
                self.permissions.push(permission);
                true
            }
        } else {
            let before = self.permissions.len();
            self.permissions
                .retain(|p| !p.has_code(&permission.code));
            self.permissions.len() != before
        }
    }

    pub fn disable_role(&mut self) {
        self.is_disabled = true;
    }

    pub fn enable_role(&mut self) {
        self.is_disabled = false;
    }

    pub fn to_data(&self) -> RoleData {
        RoleData {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            disabled: self.is_disabled,
        }
    }
}

/// Platform user aggregate corresponding to table `m_appuser` and join table `m_appuser_role`.
///
/// Ported from `org.apache.fineract.useradministration.domain.AppUser`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUser {
    pub id: i64,
    pub username: String,
    pub office_id: i64,
    pub staff_id: Option<i64>,
    pub enabled: bool,
    pub account_non_expired: bool,
    pub account_non_locked: bool,
    pub credentials_non_expired: bool,
    pub roles: Vec<Role>,
    pub is_self_service_user: bool,
    pub cannot_change_password: bool,
}

impl AppUser {
    /// Creates an active [`AppUser`] assigned to `office_id` with the given `roles`.
    pub fn new(
        id: i64,
        username: impl Into<String>,
        office_id: i64,
        staff_id: Option<i64>,
        roles: Vec<Role>,
    ) -> Self {
        Self {
            id,
            username: username.into().trim().to_string(),
            office_id,
            staff_id,
            enabled: true,
            account_non_expired: true,
            account_non_locked: true,
            credentials_non_expired: true,
            roles,
            is_self_service_user: false,
            cannot_change_password: false,
        }
    }

    /// Returns `true` if the user account is enabled, non-expired, non-locked, and has non-expired credentials.
    pub const fn is_active(&self) -> bool {
        self.enabled
            && self.account_non_expired
            && self.account_non_locked
            && self.credentials_non_expired
    }

    /// Returns `true` if any active role explicitly contains `permission_code`
    /// without considering `ALL_FUNCTIONS` wildcard expansion (`AppUser.hasSpecificPermissionTo`).
    pub fn has_specific_permission_to(&self, permission_code: &str) -> bool {
        let target = permission_code.trim();
        if target.is_empty() {
            return false;
        }
        self.roles
            .iter()
            .filter(|r| !r.is_disabled)
            .any(|role| role.has_permission_to(target))
    }

    /// Returns `true` if the user holds the `"ALL_FUNCTIONS"` superuser permission.
    pub fn is_super_user(&self) -> bool {
        self.has_specific_permission_to(ALL_FUNCTIONS_PERMISSION)
    }

    /// Returns `true` if the user holds `"CHECKER_SUPER_USER"` or `"ALL_FUNCTIONS"`,
    /// matching `AppUser.isCheckerSuperUser()`.
    pub fn is_checker_super_user(&self) -> bool {
        self.has_permission_to(CHECKER_SUPER_USER_PERMISSION)
    }

    /// Returns `true` if the user is authorized for `code`, supporting:
    /// - `"ALL_FUNCTIONS"` (superuser wildcard for all permissions)
    /// - `"ALL_FUNCTIONS_READ"` (wildcard for all `"READ_{ENTITY}"` permissions)
    /// - `"ALL_FUNCTIONS_WRITE"` (wildcard for `"CREATE_{ENTITY}"`, `"UPDATE_{ENTITY}"`, `"DELETE_{ENTITY}"`)
    /// - `"CHECKER_SUPER_USER"` (wildcard for all `"{ACTION}_{ENTITY}_CHECKER"` maker-checker permissions)
    /// - Direct `{ACTION}_{ENTITY}` and `{ACTION}_{ENTITY}_CHECKER` permission codes.
    pub fn has_permission_to(&self, code: &str) -> bool {
        let normalized = code.trim().to_ascii_uppercase();
        if normalized.is_empty() {
            return false;
        }

        // 1. ALL_FUNCTIONS grants every permission
        if self.has_specific_permission_to(ALL_FUNCTIONS_PERMISSION) {
            return true;
        }

        // 2. Direct permission match on any enabled role
        if self.has_specific_permission_to(&normalized) {
            return true;
        }

        // 3. ALL_FUNCTIONS_READ grants any READ_<ENTITY> permission
        if normalized.starts_with("READ_")
            && !normalized.ends_with("_CHECKER")
            && self.has_specific_permission_to(ALL_FUNCTIONS_READ_PERMISSION)
        {
            return true;
        }

        // 4. ALL_FUNCTIONS_WRITE grants CREATE_<ENTITY>, UPDATE_<ENTITY>, DELETE_<ENTITY>
        if (normalized.starts_with("CREATE_")
            || normalized.starts_with("UPDATE_")
            || normalized.starts_with("DELETE_"))
            && !normalized.ends_with("_CHECKER")
            && self.has_specific_permission_to(ALL_FUNCTIONS_WRITE_PERMISSION)
        {
            return true;
        }

        // 5. CHECKER_SUPER_USER grants any <ACTION>_<ENTITY>_CHECKER permission
        if normalized.ends_with("_CHECKER")
            && self.has_specific_permission_to(CHECKER_SUPER_USER_PERMISSION)
        {
            return true;
        }

        false
    }

    /// Validates that the user has permission to execute `code`, returning
    /// `Err(FineractError::AccessDenied)` if unauthorized (`AppUser.validateHasPermissionTo`).
    pub fn validate_has_permission_to(&self, code: &str) -> Result<(), FineractError> {
        if self.has_permission_to(code) {
            Ok(())
        } else {
            Err(FineractError::AccessDenied)
        }
    }

    /// Validates that the user has checker permission for `function` (`{FUNCTION}_CHECKER` or `CHECKER_SUPER_USER`),
    /// matching `AppUser.validateHasCheckerPermissionTo(function)`.
    pub fn validate_has_checker_permission_to(&self, function: &str) -> Result<(), FineractError> {
        let trimmed = function.trim().to_ascii_uppercase();
        let checker_code = if trimmed.ends_with("_CHECKER") {
            trimmed
        } else {
            format!("{trimmed}_CHECKER")
        };
        if self.has_permission_to(CHECKER_SUPER_USER_PERMISSION)
            || self.has_permission_to(&checker_code)
        {
            Ok(())
        } else {
            Err(FineractError::AccessDenied)
        }
    }

    /// Validates `READ_{RESOURCE_TYPE}` permission (`AppUser.validateHasReadPermission`).
    pub fn validate_has_read_permission(&self, resource_type: &str) -> Result<(), FineractError> {
        let code = format!("READ_{}", resource_type.trim().to_ascii_uppercase());
        self.validate_has_permission_to(&code)
    }

    /// Validates `CREATE_{RESOURCE_TYPE}` permission (`AppUser.validateHasCreatePermission`).
    pub fn validate_has_create_permission(&self, resource_type: &str) -> Result<(), FineractError> {
        let code = format!("CREATE_{}", resource_type.trim().to_ascii_uppercase());
        self.validate_has_permission_to(&code)
    }

    /// Validates `UPDATE_{RESOURCE_TYPE}` permission (`AppUser.validateHasUpdatePermission`).
    pub fn validate_has_update_permission(&self, resource_type: &str) -> Result<(), FineractError> {
        let code = format!("UPDATE_{}", resource_type.trim().to_ascii_uppercase());
        self.validate_has_permission_to(&code)
    }

    /// Validates `DELETE_{RESOURCE_TYPE}` permission (`AppUser.validateHasDeletePermission`).
    pub fn validate_has_delete_permission(&self, resource_type: &str) -> Result<(), FineractError> {
        let code = format!("DELETE_{}", resource_type.trim().to_ascii_uppercase());
        self.validate_has_permission_to(&code)
    }

    /// Returns `true` if the user has at least one of the specified permission codes (`AppUser.hasAnyPermission`).
    pub fn has_any_permission(&self, permission_codes: &[&str]) -> bool {
        permission_codes
            .iter()
            .any(|code| self.has_permission_to(code))
    }

    /// Returns `true` if the user has NONE of the specified permission codes (`AppUser.hasNotPermissionForAnyOf`).
    pub fn has_not_permission_for_any_of(&self, permission_codes: &[&str]) -> bool {
        !self.has_any_permission(permission_codes)
    }

    /// Returns all unique permission codes granted across all enabled roles (`AppUser.populateGrantedAuthorities`).
    pub fn granted_authorities(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for role in self.roles.iter().filter(|r| !r.is_disabled) {
            for perm in &role.permissions {
                if seen.insert(perm.code.clone()) {
                    out.push(perm.code.clone());
                }
            }
        }
        out
    }

    /// Returns `true` if the user has the `BYPASS_TWOFACTOR` permission (`TwoFactorAuthenticationFilter.java`).
    pub fn can_bypass_two_factor(&self) -> bool {
        self.has_specific_permission_to(BYPASS_TWO_FACTOR_PERMISSION)
    }
}

impl fmt::Display for AppUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AppUser [username={}, getId()={}]", self.username, self.id)
    }
}

// =============================================================================
// 2A. BASIC AUTHENTICATION VERIFIER
// =============================================================================

/// Stored user record pairing an [`AppUser`] with their password hash or plaintext secret.
#[derive(Debug, Clone)]
pub struct StoredUserEntry {
    pub user: AppUser,
    pub password_hash: String,
}

/// Hashes a password with SHA-256 salted by user ID or returns a `{sha256}` prefixed hex digest.
pub fn hash_password_sha256(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    format!("{{sha256}}{}", hex_encode(&digest))
}

fn verify_password_match(provided_password: &str, stored_secret: &str) -> bool {
    if let Some(stripped) = stored_secret.strip_prefix("{noop}") {
        return constant_time_eq(provided_password.as_bytes(), stripped.as_bytes());
    }
    if let Some(expected_hex) = stored_secret.strip_prefix("{sha256}") {
        let mut hasher = Sha256::new();
        hasher.update(provided_password.as_bytes());
        let actual_hex = hex_encode(&hasher.finalize());
        return constant_time_eq(
            actual_hex.to_ascii_lowercase().as_bytes(),
            expected_hex.to_ascii_lowercase().as_bytes(),
        );
    }
    // Support both raw SHA-256 hex digest (64 hex chars) and direct password match
    if stored_secret.len() == 64 && stored_secret.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut hasher = Sha256::new();
        hasher.update(provided_password.as_bytes());
        let actual_hex = hex_encode(&hasher.finalize());
        if constant_time_eq(
            actual_hex.to_ascii_lowercase().as_bytes(),
            stored_secret.to_ascii_lowercase().as_bytes(),
        ) {
            return true;
        }
    }
    constant_time_eq(provided_password.as_bytes(), stored_secret.as_bytes())
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

fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_CHARS[(b >> 4) as usize] as char);
        out.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    out
}

/// HTTP Basic Authentication verifier ported from `TenantAwareBasicAuthenticationFilter`
/// and `DaoAuthenticationProvider` / `PlatformUserDetailsChecker`.
#[derive(Debug, Clone, Default)]
pub struct BasicAuthVerifier {
    users: Arc<RwLock<HashMap<String, StoredUserEntry>>>,
}

impl BasicAuthVerifier {
    /// Creates an empty [`BasicAuthVerifier`].
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers an [`AppUser`] and password (plain, `{noop}...`, or `{sha256}...`).
    pub fn register_user(&self, user: AppUser, password: impl Into<String>) {
        let pwd = password.into();
        let password_hash = if pwd.starts_with("{noop}") || pwd.starts_with("{sha256}") {
            pwd
        } else {
            hash_password_sha256(&pwd)
        };
        let mut guard = self.users.write().expect("BasicAuthVerifier lock poisoned");
        guard.insert(
            user.username.clone(),
            StoredUserEntry {
                user,
                password_hash,
            },
        );
    }

    /// Looks up a registered [`AppUser`] by username.
    pub fn find_user(&self, username: &str) -> Option<AppUser> {
        let guard = self.users.read().expect("BasicAuthVerifier lock poisoned");
        guard.get(username.trim()).map(|entry| entry.user.clone())
    }

    /// Parses an `Authorization: Basic <base64>` header into `(username, password)`.
    pub fn parse_basic_header(authorization_header: &str) -> Result<(String, String), SecurityError> {
        let trimmed = authorization_header.trim();
        let encoded = trimmed
            .strip_prefix("Basic ")
            .or_else(|| trimmed.strip_prefix("basic "))
            .ok_or_else(|| {
                SecurityError::Unauthenticated(
                    "Missing or malformed 'Authorization: Basic' header".to_string(),
                )
            })?
            .trim();

        if encoded.is_empty() {
            return Err(SecurityError::Unauthenticated(
                "Empty Basic authentication token".to_string(),
            ));
        }

        let decoded_bytes = BASE64_STANDARD.decode(encoded).map_err(|_| {
            SecurityError::Unauthenticated("Invalid Base64 in Basic authentication header".to_string())
        })?;

        let decoded_str = String::from_utf8(decoded_bytes).map_err(|_| {
            SecurityError::Unauthenticated("Invalid UTF-8 in Basic authentication token".to_string())
        })?;

        let (username, password) = decoded_str.split_once(':').ok_or_else(|| {
            SecurityError::Unauthenticated(
                "Basic authentication token missing ':' delimiter".to_string(),
            )
        })?;

        if username.is_empty() {
            return Err(SecurityError::Unauthenticated(
                "Username cannot be empty in Basic authentication token".to_string(),
            ));
        }

        Ok((username.to_string(), password.to_string()))
    }

    /// Encodes `(username, password)` into a Base64 authentication key matching
    /// `AuthenticationApiResource.java` (`base64EncodedAuthenticationKey`).
    pub fn encode_basic_credentials(username: &str, password: &str) -> String {
        BASE64_STANDARD.encode(format!("{username}:{password}").as_bytes())
    }

    /// Verifies active user flags (`enabled`, `account_non_expired`, `account_non_locked`,
    /// `credentials_non_expired`) matching Spring Security's `UserDetailsChecker`.
    pub fn verify_user_status(user: &AppUser) -> Result<(), SecurityError> {
        if !user.enabled {
            return Err(SecurityError::Unauthenticated(format!(
                "User account '{}' is disabled",
                user.username
            )));
        }
        if !user.account_non_expired {
            return Err(SecurityError::Unauthenticated(format!(
                "User account '{}' has expired",
                user.username
            )));
        }
        if !user.account_non_locked {
            return Err(SecurityError::Unauthenticated(format!(
                "User account '{}' is locked",
                user.username
            )));
        }
        if !user.credentials_non_expired {
            return Err(SecurityError::Unauthenticated(format!(
                "User credentials for '{}' have expired",
                user.username
            )));
        }
        Ok(())
    }

    /// Authenticates `(username, password)` and verifies that the [`AppUser`] is active.
    pub fn authenticate_credentials(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AppUser, SecurityError> {
        let guard = self.users.read().expect("BasicAuthVerifier lock poisoned");
        let entry = guard.get(username.trim()).ok_or_else(|| {
            SecurityError::Unauthenticated(format!("Invalid username or password for '{username}'"))
        })?;

        if !verify_password_match(password, &entry.password_hash) {
            return Err(SecurityError::Unauthenticated(format!(
                "Invalid username or password for '{username}'"
            )));
        }

        Self::verify_user_status(&entry.user)?;
        Ok(entry.user.clone())
    }

    /// Parses `Authorization: Basic ...`, authenticates credentials, and verifies active user status.
    pub fn verify_authorization_header(
        &self,
        authorization_header: &str,
    ) -> Result<AppUser, SecurityError> {
        let (username, password) = Self::parse_basic_header(authorization_header)?;
        self.authenticate_credentials(&username, &password)
    }

    /// Convenience wrapper returning `Result<AppUser, FineractError>`.
    pub fn verify(&self, authorization_header: &str) -> Result<AppUser, FineractError> {
        self.verify_authorization_header(authorization_header)
            .map_err(Into::into)
    }
}

// =============================================================================
// 2B. OAUTH2 / OIDC JWT RESOURCE SERVER VERIFIER
// =============================================================================

/// Standard & Fineract OAuth2/OIDC JWT claims payload ported from
/// `FineractJwtAuthenticationTokenConverter`, `FineractOidcJwtAuthenticationConverter`,
/// and `TenantAwareAuthenticationFilter`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FineractJwtClaims {
    /// Subject (`sub`) — mapped to `AppUser.username` by default.
    pub sub: String,
    /// Expiration timestamp (seconds since Unix epoch).
    pub exp: usize,
    /// Issued-at timestamp (seconds since Unix epoch).
    #[serde(default)]
    pub iat: Option<usize>,
    /// Token issuer (`iss`).
    #[serde(default)]
    pub iss: Option<String>,
    /// Token audience (`aud`).
    #[serde(default)]
    pub aud: Option<String>,
    /// Optional Fineract tenant identifier claim (`tenant`) used by `TenantAwareAuthenticationFilter`.
    #[serde(default)]
    pub tenant: Option<String>,
    /// Space-delimited OAuth2 scopes (`"read write"`).
    #[serde(default)]
    pub scope: Option<String>,
    /// Array-formatted OAuth2 scopes (`["read", "write"]`).
    #[serde(default)]
    pub scp: Vec<String>,
    /// Explicit Fineract permission authorities embedded in the token.
    #[serde(default)]
    pub authorities: Vec<String>,
    /// Optional OIDC `preferred_username` claim for external IdP mapping.
    #[serde(default)]
    pub preferred_username: Option<String>,
}

impl FineractJwtClaims {
    /// Extracts all normalized OAuth2 scopes from `scope` (space-delimited) and `scp` (array).
    pub fn scopes(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        if let Some(scope_str) = &self.scope {
            for s in scope_str.split_whitespace() {
                if !s.is_empty() {
                    out.insert(s.to_string());
                }
            }
        }
        for s in &self.scp {
            if !s.trim().is_empty() {
                out.insert(s.trim().to_string());
            }
        }
        out
    }

    /// Checks whether this JWT grants the requested OAuth2 scope.
    pub fn has_scope(&self, required_scope: &str) -> bool {
        self.scopes().contains(required_scope.trim())
    }
}

/// Verified JWT authentication outcome pairing the resolved [`AppUser`] with parsed [`FineractJwtClaims`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedJwtAuthentication {
    pub user: AppUser,
    pub claims: FineractJwtClaims,
    pub raw_token: String,
}

/// OAuth2 / OIDC Bearer JWT verifier ported from `FineractJwtAuthenticationTokenConverter`
/// and `FineractOidcJwtAuthenticationConverter`.
#[derive(Clone)]
pub struct OAuth2JwtVerifier {
    secret_bytes: Vec<u8>,
    algorithm: Algorithm,
    expected_issuer: Option<String>,
    required_scopes: Vec<String>,
    username_claim: String,
    users: Arc<RwLock<HashMap<String, AppUser>>>,
}

impl fmt::Debug for OAuth2JwtVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuth2JwtVerifier")
            .field("algorithm", &self.algorithm)
            .field("expected_issuer", &self.expected_issuer)
            .field("required_scopes", &self.required_scopes)
            .field("username_claim", &self.username_claim)
            .finish()
    }
}

impl OAuth2JwtVerifier {
    /// Creates an HS256 [`OAuth2JwtVerifier`] using the provided symmetric secret.
    pub fn new_hs256(secret: impl AsRef<[u8]>) -> Self {
        Self {
            secret_bytes: secret.as_ref().to_vec(),
            algorithm: Algorithm::HS256,
            expected_issuer: None,
            required_scopes: Vec::new(),
            username_claim: "sub".to_string(),
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Configures an expected token issuer (`iss`).
    pub fn with_expected_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.expected_issuer = Some(issuer.into());
        self
    }

    /// Configures scopes that every verified Bearer token must include.
    pub fn with_required_scopes<I, S>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Configures which claim holds the Fineract username (`"sub"` by default, or `"preferred_username"` for OIDC).
    pub fn with_username_claim(mut self, claim_name: impl Into<String>) -> Self {
        self.username_claim = claim_name.into();
        self
    }

    /// Registers an [`AppUser`] so JWT subjects can be resolved to full Fineract domain users.
    pub fn register_user(&self, user: AppUser) {
        let mut guard = self.users.write().expect("OAuth2JwtVerifier lock poisoned");
        guard.insert(user.username.clone(), user);
    }

    /// Issues a signed HS256 JWT for testing and internal OAuth2 token generation.
    pub fn issue_token(&self, claims: &FineractJwtClaims) -> Result<String, SecurityError> {
        let header = Header::new(self.algorithm);
        encode(
            &header,
            claims,
            &EncodingKey::from_secret(&self.secret_bytes),
        )
        .map_err(|e| SecurityError::Unauthenticated(format!("Failed to sign JWT: {e}")))
    }

    /// Extracts the raw JWT string from an `Authorization: Bearer <token>` header.
    pub fn extract_bearer_token(authorization_header: &str) -> Result<&str, SecurityError> {
        let trimmed = authorization_header.trim();
        let token = trimmed
            .strip_prefix("Bearer ")
            .or_else(|| trimmed.strip_prefix("bearer "))
            .ok_or_else(|| {
                SecurityError::Unauthenticated(
                    "Missing or malformed 'Authorization: Bearer' header".to_string(),
                )
            })?
            .trim();

        if token.is_empty() {
            return Err(SecurityError::Unauthenticated(
                "Empty Bearer token".to_string(),
            ));
        }
        Ok(token)
    }

    /// Verifies a raw JWT string (signature, expiration, issuer, required scopes) and maps its subject to an [`AppUser`].
    pub fn verify_token(
        &self,
        raw_token: &str,
    ) -> Result<VerifiedJwtAuthentication, SecurityError> {
        let mut validation = Validation::new(self.algorithm);
        validation.validate_exp = true;
        validation.leeway = 0;
        if let Some(iss) = &self.expected_issuer {
            validation.set_issuer(&[iss]);
        }

        let token_data = decode::<FineractJwtClaims>(
            raw_token,
            &DecodingKey::from_secret(&self.secret_bytes),
            &validation,
        )
        .map_err(|e| SecurityError::Unauthenticated(format!("Invalid JWT token: {e}")))?;

        let claims = token_data.claims;

        // Explicit expiration check against current UTC timestamp
        let now_ts = Utc::now().timestamp().max(0) as usize;
        if claims.exp <= now_ts {
            return Err(SecurityError::Unauthenticated(
                "JWT token has expired".to_string(),
            ));
        }

        // Verify required OAuth2 scopes
        let granted_scopes = claims.scopes();
        for req_scope in &self.required_scopes {
            if !granted_scopes.contains(req_scope.as_str()) {
                return Err(SecurityError::AccessDenied(format!(
                    "JWT missing required scope '{req_scope}'"
                )));
            }
        }

        // Resolve username from configured claim (`preferred_username` or `sub`)
        let username = if self.username_claim == "preferred_username" {
            claims
                .preferred_username
                .as_deref()
                .unwrap_or(&claims.sub)
                .trim()
        } else {
            claims.sub.trim()
        };

        if username.is_empty() {
            return Err(SecurityError::Unauthenticated(
                "JWT subject claim is blank".to_string(),
            ));
        }

        let guard = self.users.read().expect("OAuth2JwtVerifier lock poisoned");
        let mut user = guard.get(username).cloned().ok_or_else(|| {
            SecurityError::Unauthenticated(format!(
                "OAuth2/OIDC user not found for subject '{username}'"
            ))
        })?;
        drop(guard);

        // Merge any explicit JWT authorities into a transient role if not already present on the user
        if !claims.authorities.is_empty() {
            let extra_perms: Vec<Permission> = claims
                .authorities
                .iter()
                .enumerate()
                .filter(|(_, code)| !user.has_specific_permission_to(code))
                .map(|(idx, code)| Permission::special(-(idx as i64 + 1), code.clone()))
                .collect();
            if !extra_perms.is_empty() {
                user.roles.push(Role::new(
                    -1,
                    "JWT_AUTHORITIES",
                    "Authorities granted by OAuth2 JWT",
                    extra_perms,
                ));
            }
        }

        BasicAuthVerifier::verify_user_status(&user)?;

        Ok(VerifiedJwtAuthentication {
            user,
            claims,
            raw_token: raw_token.to_string(),
        })
    }

    /// Parses an `Authorization: Bearer <token>` header, validates the JWT, and returns the resolved [`AppUser`].
    pub fn verify_authorization_header(
        &self,
        authorization_header: &str,
    ) -> Result<VerifiedJwtAuthentication, SecurityError> {
        let raw_token = Self::extract_bearer_token(authorization_header)?;
        self.verify_token(raw_token)
    }

    /// Convenience wrapper returning `Result<AppUser, FineractError>`.
    pub fn verify(&self, authorization_header: &str) -> Result<AppUser, FineractError> {
        self.verify_authorization_header(authorization_header)
            .map(|verified| verified.user)
            .map_err(Into::into)
    }
}

// =============================================================================
// 2C. TWO-FACTOR AUTHENTICATION (f_twofactor_access_token / TwoFactorAuthService)
// =============================================================================

/// Delivery channel descriptor for 2FA OTP tokens (`OTPDeliveryMethod.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OTPDeliveryMethod {
    pub name: String,
    pub target: String,
}

/// Metadata associated with an [`OTPRequest`] (`OTPMetadata.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OTPMetadata {
    pub request_time: DateTime<Utc>,
    pub token_live_time_in_sec: i64,
    pub extended_access_token: bool,
    pub delivery_method: OTPDeliveryMethod,
}

/// In-flight one-time password request (`OTPRequest.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OTPRequest {
    pub token: String,
    pub metadata: OTPMetadata,
}

impl OTPRequest {
    /// Creates a new [`OTPRequest`] matching `OTPRequest.create(...)`.
    pub fn create(
        token: impl Into<String>,
        token_live_time_in_sec: i64,
        extended_access_token: bool,
        delivery_method: OTPDeliveryMethod,
    ) -> Self {
        Self {
            token: token.into(),
            metadata: OTPMetadata {
                request_time: Utc::now(),
                token_live_time_in_sec,
                extended_access_token,
                delivery_method,
            },
        }
    }

    /// Returns `true` if the OTP request has not yet expired (`OTPRequest.isValid()`).
    pub fn is_valid(&self) -> bool {
        self.is_valid_at(Utc::now())
    }

    /// Returns `true` if the OTP request is valid at the given `now` timestamp.
    pub fn is_valid_at(&self, now: DateTime<Utc>) -> bool {
        let expire_time =
            self.metadata.request_time + Duration::seconds(self.metadata.token_live_time_in_sec);
        now < expire_time
    }
}

/// Wire DTO returned when a 2FA access token is issued (`AccessTokenData.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessTokenData {
    pub token: String,
    pub valid_from: NaiveDateTime,
    pub valid_to: NaiveDateTime,
}

/// Two-factor access token entity matching table `twofactor_access_token` (`f_twofactor_access_token`)
/// and `org.apache.fineract.infrastructure.security.domain.TFAccessToken`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TFAccessToken {
    pub id: i64,
    pub token: String,
    pub appuser_id: i64,
    pub valid_from: NaiveDateTime,
    pub valid_to: NaiveDateTime,
    pub enabled: bool,
}

impl TFAccessToken {
    /// Creates a new enabled [`TFAccessToken`] valid for `token_live_time_in_sec` seconds,
    /// matching `TFAccessToken.create(token, user, tokenLiveTimeInSec)`.
    pub fn create(
        id: i64,
        token: impl Into<String>,
        appuser_id: i64,
        token_live_time_in_sec: i64,
    ) -> Self {
        let valid_from = Utc::now().naive_utc();
        let valid_to = valid_from + Duration::seconds(token_live_time_in_sec);
        Self {
            id,
            token: token.into(),
            appuser_id,
            valid_from,
            valid_to,
            enabled: true,
        }
    }

    /// Checks whether the token is enabled and within `[valid_from, valid_to)` at the current UTC time,
    /// matching `TFAccessToken.isValid()`.
    pub fn is_valid(&self) -> bool {
        self.is_valid_at(Utc::now().naive_utc())
    }

    /// Checks whether the token is enabled and `valid_from <= now < valid_to`.
    pub fn is_valid_at(&self, now: NaiveDateTime) -> bool {
        self.enabled && self.valid_from <= now && now < self.valid_to
    }

    /// Projects this entity to [`AccessTokenData`] matching `TFAccessToken.toTokenData()`.
    pub fn to_token_data(&self) -> AccessTokenData {
        AccessTokenData {
            token: self.token.clone(),
            valid_from: self.valid_from,
            valid_to: self.valid_to,
        }
    }
}

/// Configuration settings for two-factor authentication matching `TwoFactorConfigurationConstants.java`
/// and `TwoFactorConfigurationServiceImpl.java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TwoFactorConfig {
    pub otp_delivery_email_enable: bool,
    pub otp_delivery_sms_enable: bool,
    pub otp_token_length: usize,
    pub otp_token_live_time: i64,
    pub access_token_live_time: i64,
    pub access_token_live_time_extended: i64,
}

impl Default for TwoFactorConfig {
    fn default() -> Self {
        Self {
            otp_delivery_email_enable: true,
            otp_delivery_sms_enable: false,
            otp_token_length: 6,
            otp_token_live_time: 300,
            access_token_live_time: 86_400,
            access_token_live_time_extended: 604_800,
        }
    }
}

/// Generates alphanumeric OTP tokens matching `RandomOTPGenerator.java` and 32-char hex access tokens
/// matching `UUIDAccessTokenGenerationService.java`.
#[derive(Debug)]
pub struct TwoFactorAuthService {
    enabled: bool,
    config: RwLock<TwoFactorConfig>,
    otp_requests: RwLock<HashMap<i64, OTPRequest>>,
    access_tokens: RwLock<HashMap<(i64, String), TFAccessToken>>,
    counter: AtomicU64,
}

impl Default for TwoFactorAuthService {
    fn default() -> Self {
        Self::new(false)
    }
}

impl TwoFactorAuthService {
    /// Creates a [`TwoFactorAuthService`] with the `twofactor` profile active or inactive.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            config: RwLock::new(TwoFactorConfig::default()),
            otp_requests: RwLock::new(HashMap::new()),
            access_tokens: RwLock::new(HashMap::new()),
            counter: AtomicU64::new(1),
        }
    }

    /// Creates a [`TwoFactorAuthService`] with custom [`TwoFactorConfig`].
    pub fn with_config(enabled: bool, config: TwoFactorConfig) -> Self {
        Self {
            enabled,
            config: RwLock::new(config),
            otp_requests: RwLock::new(HashMap::new()),
            access_tokens: RwLock::new(HashMap::new()),
            counter: AtomicU64::new(1),
        }
    }

    /// Returns `true` if the `twofactor` security profile is active.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Updates the active [`TwoFactorConfig`] (e.g., `PUT /v1/twofactor/configure`).
    pub fn update_config(&self, config: TwoFactorConfig) {
        let mut guard = self.config.write().expect("TwoFactorAuthService config lock");
        *guard = config;
    }

    /// Returns a snapshot of the active [`TwoFactorConfig`].
    pub fn config(&self) -> TwoFactorConfig {
        self.config
            .read()
            .expect("TwoFactorAuthService config lock")
            .clone()
    }

    /// Generates an OTP token of `token_length` characters from `ALLOWED_OTP_CHARACTERS`
    /// matching `RandomOTPGenerator.java`.
    pub fn generate_otp_token(&self, token_length: usize, user_id: i64) -> String {
        let seq = self.counter.fetch_add(1, Ordering::Relaxed);
        let now_nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let mut mac = Hmac::<Sha256>::new_from_slice(b"fineract-twofactor-otp-seed")
            .expect("HMAC accepts any key size");
        mac.update(&seq.to_le_bytes());
        mac.update(&user_id.to_le_bytes());
        mac.update(&now_nanos.to_le_bytes());
        let digest = mac.finalize().into_bytes();

        let len = token_length.max(1);
        let mut out = String::with_capacity(len);
        for i in 0..len {
            let byte = digest[i % digest.len()] as usize;
            let idx = (byte + i * 7) % ALLOWED_OTP_CHARACTERS.len();
            out.push(ALLOWED_OTP_CHARACTERS[idx] as char);
        }
        out
    }

    /// Generates a 32-character lowercase hex access token matching `UUIDAccessTokenGenerationService.java`.
    pub fn generate_random_access_token(&self, user_id: i64) -> String {
        let seq = self.counter.fetch_add(1, Ordering::Relaxed);
        let now_nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let mut hasher = Sha256::new();
        hasher.update(b"fineract-tfa-access-token");
        hasher.update(seq.to_le_bytes());
        hasher.update(user_id.to_le_bytes());
        hasher.update(now_nanos.to_le_bytes());
        let hex = hex_encode(&hasher.finalize());
        hex[..32].to_string()
    }

    /// Creates and stores a new [`OTPRequest`] for `user` matching `TwoFactorServiceImpl.createNewOTPToken`.
    pub fn create_new_otp_token(
        &self,
        user: &AppUser,
        delivery_method_name: &str,
        extended_access_token: bool,
    ) -> Result<OTPRequest, SecurityError> {
        let cfg = self.config();
        let method_norm = delivery_method_name.trim().to_ascii_lowercase();
        let delivery_method = match method_norm.as_str() {
            "email" if cfg.otp_delivery_email_enable => OTPDeliveryMethod {
                name: "email".to_string(),
                target: format!("{}@fineract.apache.org", user.username),
            },
            "sms" if cfg.otp_delivery_sms_enable => OTPDeliveryMethod {
                name: "sms".to_string(),
                target: "+10000000000".to_string(),
            },
            _ => {
                return Err(SecurityError::AccessDenied(format!(
                    "Invalid or disabled 2FA OTP delivery method '{delivery_method_name}'"
                )))
            }
        };

        let token = self.generate_otp_token(cfg.otp_token_length, user.id);
        let request = OTPRequest::create(
            token,
            cfg.otp_token_live_time.max(1),
            extended_access_token,
            delivery_method,
        );

        let mut guard = self
            .otp_requests
            .write()
            .expect("TwoFactorAuthService otp_requests lock");
        guard.insert(user.id, request.clone());
        Ok(request)
    }

    /// Validates a submitted `otp_token` for `user` and issues a [`TFAccessToken`],
    /// matching `TwoFactorServiceImpl.createAccessTokenFromOTP`.
    pub fn create_access_token_from_otp(
        &self,
        user: &AppUser,
        otp_token: &str,
    ) -> Result<TFAccessToken, SecurityError> {
        let otp_request = {
            let guard = self
                .otp_requests
                .read()
                .expect("TwoFactorAuthService otp_requests lock");
            guard.get(&user.id).cloned()
        };

        let req = otp_request.ok_or_else(|| {
            SecurityError::AccessDenied("No active OTP request found for user".to_string())
        })?;

        if !req.is_valid() || !req.token.eq_ignore_ascii_case(otp_token.trim()) {
            return Err(SecurityError::AccessDenied(
                "Invalid or expired two-factor OTP token".to_string(),
            ));
        }

        // Consume one-time OTP request
        {
            let mut guard = self
                .otp_requests
                .write()
                .expect("TwoFactorAuthService otp_requests lock");
            guard.remove(&user.id);
        }

        let cfg = self.config();
        let live_time = if req.metadata.extended_access_token {
            cfg.access_token_live_time_extended.max(1)
        } else {
            cfg.access_token_live_time.max(1)
        };

        let id = self.counter.fetch_add(1, Ordering::Relaxed) as i64;
        let token_str = self.generate_random_access_token(user.id);
        let access_token = TFAccessToken::create(id, token_str.clone(), user.id, live_time);

        let mut guard = self
            .access_tokens
            .write()
            .expect("TwoFactorAuthService access_tokens lock");
        guard.insert((user.id, token_str), access_token.clone());

        Ok(access_token)
    }

    /// Fetches a [`TFAccessToken`] for `(user, token)` matching `TwoFactorServiceImpl.fetchAccessTokenForUser`.
    pub fn fetch_access_token_for_user(&self, user: &AppUser, token: &str) -> Option<TFAccessToken> {
        let guard = self
            .access_tokens
            .read()
            .expect("TwoFactorAuthService access_tokens lock");
        guard
            .get(&(user.id, token.trim().to_string()))
            .cloned()
    }

    /// Invalidates (disables) an existing [`TFAccessToken`] matching `TwoFactorServiceImpl.invalidateAccessToken`.
    pub fn invalidate_access_token(
        &self,
        user: &AppUser,
        token: &str,
    ) -> Result<TFAccessToken, SecurityError> {
        let mut guard = self
            .access_tokens
            .write()
            .expect("TwoFactorAuthService access_tokens lock");
        let entry = guard
            .get_mut(&(user.id, token.trim().to_string()))
            .ok_or(SecurityError::InvalidTwoFactorAccessToken)?;

        if !entry.is_valid() {
            return Err(SecurityError::InvalidTwoFactorAccessToken);
        }

        entry.enabled = false;
        Ok(entry.clone())
    }

    /// Returns `true` if `request_path` is a 2FA bootstrap endpoint (`/v1/twofactor` or `/v1/twofactor/validate`)
    /// that only requires `fullyAuthenticated()` without `TWOFACTOR_AUTHENTICATED`
    /// (`SecurityConfig.java` lines 406–407).
    pub fn is_twofactor_bootstrap_endpoint(request_path: &str) -> bool {
        let path_only = request_path.split('?').next().unwrap_or(request_path);
        let trimmed = path_only.trim_end_matches('/');
        trimmed.ends_with("/v1/twofactor") || trimmed.ends_with("/v1/twofactor/validate")
    }

    /// Enforces `TwoFactorAuthenticationFilter.java` and `SecurityConfig.java` rules:
    /// 1. If 2FA profile is disabled, returns `Ok(())`.
    /// 2. If `user` has `BYPASS_TWOFACTOR` permission, returns `Ok(())`.
    /// 3. If `Fineract-Platform-TFA-Token` header is present:
    ///    - Validates token via `fetch_access_token_for_user(user, token)` and `access_token.is_valid()`.
    ///    - If invalid/expired/disabled, returns `Err(SecurityError::InvalidTwoFactorAccessToken)` (`401 Unauthorized`).
    ///    - If valid, returns `Ok(())`.
    /// 4. If `Fineract-Platform-TFA-Token` header is absent:
    ///    - If `request_path` is `/v1/twofactor` or `/v1/twofactor/validate`, allows request to proceed (`Ok(())`).
    ///    - Otherwise returns `Err(SecurityError::AccessDenied)` (`403 Forbidden`, missing `TWOFACTOR_AUTHENTICATED`).
    pub fn verify_request_tfa(
        &self,
        user: &AppUser,
        request_path: &str,
        tfa_token_header: Option<&str>,
    ) -> Result<(), SecurityError> {
        if !self.enabled {
            return Ok(());
        }

        if user.can_bypass_two_factor() {
            return Ok(());
        }

        if let Some(raw_token) = tfa_token_header {
            let token = raw_token.trim();
            if !token.is_empty() {
                match self.fetch_access_token_for_user(user, token) {
                    Some(access_token) if access_token.is_valid() => return Ok(()),
                    _ => return Err(SecurityError::InvalidTwoFactorAccessToken),
                }
            }
        }

        if Self::is_twofactor_bootstrap_endpoint(request_path) {
            return Ok(());
        }

        Err(SecurityError::AccessDenied(
            "Two-factor authentication required: missing 'Fineract-Platform-TFA-Token' header"
                .to_string(),
        ))
    }
}

// =============================================================================
// 3. TENANT AND SECURITY STATE & AXUM CONTEXT BUILDER
// =============================================================================

/// Shared multi-tenant and security state for Axum middleware/extractors.
#[derive(Debug, Clone)]
pub struct TenantAndSecurityState {
    pub basic_verifier: Arc<BasicAuthVerifier>,
    pub oauth2_verifier: Option<Arc<OAuth2JwtVerifier>>,
    pub two_factor_service: Arc<TwoFactorAuthService>,
    pub tenants: Arc<RwLock<HashMap<String, FineractPlatformTenant>>>,
    pub tenant_business_dates:
        Arc<RwLock<HashMap<String, HashMap<BusinessDateType, NaiveDate>>>>,
}

impl Default for TenantAndSecurityState {
    fn default() -> Self {
        Self::new(
            Arc::new(BasicAuthVerifier::new()),
            None,
            Arc::new(TwoFactorAuthService::new(false)),
        )
    }
}

impl TenantAndSecurityState {
    /// Creates a new [`TenantAndSecurityState`] pre-populated with the `"default"` tenant.
    pub fn new(
        basic_verifier: Arc<BasicAuthVerifier>,
        oauth2_verifier: Option<Arc<OAuth2JwtVerifier>>,
        two_factor_service: Arc<TwoFactorAuthService>,
    ) -> Self {
        let mut tenants = HashMap::new();
        let default_tenant = FineractPlatformTenant::default();
        tenants.insert(default_tenant.tenant_identifier.clone(), default_tenant);

        let today = Utc::now().date_naive();
        let mut default_dates = HashMap::new();
        default_dates.insert(BusinessDateType::BusinessDate, today);
        default_dates.insert(BusinessDateType::CobDate, today);
        let mut tenant_business_dates = HashMap::new();
        tenant_business_dates.insert("default".to_string(), default_dates);

        Self {
            basic_verifier,
            oauth2_verifier,
            two_factor_service,
            tenants: Arc::new(RwLock::new(tenants)),
            tenant_business_dates: Arc::new(RwLock::new(tenant_business_dates)),
        }
    }

    /// Registers a [`FineractPlatformTenant`] and its active [`BusinessDateType`] map.
    pub fn register_tenant(
        &self,
        tenant: FineractPlatformTenant,
        business_dates: HashMap<BusinessDateType, NaiveDate>,
    ) {
        let tenant_id = tenant.tenant_identifier.clone();
        self.tenants
            .write()
            .expect("tenants lock")
            .insert(tenant_id.clone(), tenant);
        self.tenant_business_dates
            .write()
            .expect("tenant_business_dates lock")
            .insert(tenant_id, business_dates);
    }

    /// Resolves a tenant by identifier, returning `Err(SecurityError::InvalidTenantIdentifier)` if unknown.
    pub fn resolve_tenant(
        &self,
        tenant_identifier: &str,
    ) -> Result<(FineractPlatformTenant, HashMap<BusinessDateType, NaiveDate>), SecurityError> {
        let id = tenant_identifier.trim();
        if id.is_empty() {
            return Err(SecurityError::MissingTenantIdentifier);
        }

        let tenant = self
            .tenants
            .read()
            .expect("tenants lock")
            .get(id)
            .cloned()
            .ok_or_else(|| SecurityError::InvalidTenantIdentifier(id.to_string()))?;

        let dates = self
            .tenant_business_dates
            .read()
            .expect("tenant_business_dates lock")
            .get(id)
            .cloned()
            .unwrap_or_else(|| {
                let today = Utc::now().date_naive();
                let mut m = HashMap::with_capacity(2);
                m.insert(BusinessDateType::BusinessDate, today);
                m.insert(BusinessDateType::CobDate, today);
                m
            });

        Ok((tenant, dates))
    }
}

/// Validates the `Fineract-Platform-TenantId` header (returning `400 Bad Request` via
/// [`SecurityError::MissingTenantIdentifier`] if missing), authenticates the caller via
/// Basic Auth or OAuth2 Bearer JWT, enforces 2FA token validation when the `twofactor`
/// profile is active (with bypass for `/v1/twofactor` endpoints and `BYPASS_TWOFACTOR` permission),
/// and constructs a [`fineract_core_rs::ExecutionContext`].
pub fn authenticate_and_build_context(
    state: &TenantAndSecurityState,
    headers: &HeaderMap,
    request_path: &str,
) -> Result<(ExecutionContext, AppUser), SecurityError> {
    // 1. Extract and validate `Fineract-Platform-TenantId` header (400 Bad Request if missing/blank)
    let tenant_header = headers
        .get(TENANT_ID_REQUEST_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(SecurityError::MissingTenantIdentifier)?;

    let (tenant, business_dates) = state.resolve_tenant(tenant_header)?;

    // 2. Extract and verify `Authorization` header (Basic or Bearer)
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            SecurityError::Unauthenticated("Missing 'Authorization' request header".to_string())
        })?;

    let user = if auth_header.starts_with("Basic ") || auth_header.starts_with("basic ") {
        state
            .basic_verifier
            .verify_authorization_header(auth_header)?
    } else if auth_header.starts_with("Bearer ") || auth_header.starts_with("bearer ") {
        let jwt_verifier = state.oauth2_verifier.as_ref().ok_or_else(|| {
            SecurityError::Unauthenticated(
                "OAuth2/JWT authentication is not enabled on this server".to_string(),
            )
        })?;
        let verified = jwt_verifier.verify_authorization_header(auth_header)?;
        if let Some(jwt_tenant) = &verified.claims.tenant {
            if !jwt_tenant.trim().is_empty() && jwt_tenant.trim() != tenant.tenant_identifier {
                return Err(SecurityError::InvalidTenantIdentifier(format!(
                    "JWT tenant '{}' does not match header tenant '{}'",
                    jwt_tenant, tenant.tenant_identifier
                )));
            }
        }
        verified.user
    } else {
        return Err(SecurityError::Unauthenticated(
            "Unsupported 'Authorization' scheme; expected 'Basic' or 'Bearer'".to_string(),
        ));
    };

    // 3. Enforce Two-Factor Authentication (`Fineract-Platform-TFA-Token`) if enabled
    let tfa_header = headers
        .get(TFA_TOKEN_REQUEST_HEADER)
        .and_then(|v| v.to_str().ok());
    state
        .two_factor_service
        .verify_request_tfa(&user, request_path, tfa_header)?;

    // 4. Build immutable `ExecutionContext`
    let ctx = ExecutionContext {
        tenant_id: tenant.tenant_identifier.clone(),
        tenant,
        business_dates,
        action_context: ActionContext::Default,
        authenticated_user_id: Some(user.id),
        is_super_user: user.is_super_user(),
    };

    Ok((ctx, user))
}

// =============================================================================
// 4. UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn sample_user_with_permissions(username: &str, perms: Vec<Permission>) -> AppUser {
        let role = Role::new(1, "Test Role", "Role for unit testing", perms);
        AppUser::new(10, username, 1, Some(100), vec![role])
    }

    #[test]
    fn test_rbac_all_functions_and_all_functions_read_and_maker_checker() {
        // 1. Superuser with ALL_FUNCTIONS
        let super_user = sample_user_with_permissions(
            "mifos",
            vec![Permission::special(1, "ALL_FUNCTIONS")],
        );
        assert!(super_user.is_super_user());
        assert!(super_user.has_permission_to("CREATE_LOAN"));
        assert!(super_user.has_permission_to("APPROVE_LOAN_CHECKER"));
        assert!(super_user.has_permission_to("READ_CLIENT"));
        assert!(super_user.validate_has_permission_to("DELETE_SAVINGSACCOUNT").is_ok());

        // 2. Read-only user with ALL_FUNCTIONS_READ
        let auditor = sample_user_with_permissions(
            "auditor",
            vec![Permission::special(2, "ALL_FUNCTIONS_READ")],
        );
        assert!(!auditor.is_super_user());
        assert!(auditor.has_permission_to("READ_LOAN"));
        assert!(auditor.has_permission_to("READ_CLIENT"));
        assert!(!auditor.has_permission_to("CREATE_LOAN"));
        assert!(!auditor.has_permission_to("READ_LOAN_CHECKER"));
        assert_eq!(
            auditor.validate_has_permission_to("CREATE_LOAN"),
            Err(FineractError::AccessDenied)
        );

        // 3. Maker-Checker permissions: specific vs CHECKER_SUPER_USER
        let maker_only = sample_user_with_permissions(
            "maker",
            vec![
                Permission::for_entity(3, "portfolio", "LOAN", "CREATE", true),
                Permission::for_entity(4, "portfolio", "LOAN", "APPROVE", true),
            ],
        );
        assert!(maker_only.has_permission_to("CREATE_LOAN"));
        assert!(maker_only.has_permission_to("APPROVE_LOAN"));
        assert!(!maker_only.has_permission_to("CREATE_LOAN_CHECKER"));
        assert_eq!(
            maker_only.validate_has_checker_permission_to("CREATE_LOAN"),
            Err(FineractError::AccessDenied)
        );

        let specific_checker = sample_user_with_permissions(
            "loan_checker",
            vec![Permission::new(
                5,
                "portfolio",
                "CREATE_LOAN_CHECKER",
                Some("LOAN".to_string()),
                Some("CREATE_CHECKER".to_string()),
                false,
            )],
        );
        assert!(specific_checker.has_permission_to("CREATE_LOAN_CHECKER"));
        assert!(specific_checker
            .validate_has_checker_permission_to("CREATE_LOAN")
            .is_ok());
        assert!(!specific_checker.has_permission_to("DISBURSE_LOAN_CHECKER"));

        let checker_super = sample_user_with_permissions(
            "checker_boss",
            vec![Permission::special(6, "CHECKER_SUPER_USER")],
        );
        assert!(checker_super.is_checker_super_user());
        assert!(checker_super.has_permission_to("CREATE_LOAN_CHECKER"));
        assert!(checker_super.has_permission_to("DISBURSE_LOAN_CHECKER"));
        assert!(!checker_super.has_permission_to("CREATE_LOAN"));
    }

    #[test]
    fn test_disabled_role_does_not_grant_permissions() {
        let mut role = Role::new(
            1,
            "Disabled Admin",
            "Disabled role",
            vec![Permission::special(1, "ALL_FUNCTIONS")],
        );
        role.disable_role();
        let user = AppUser::new(11, "temp_admin", 1, None, vec![role]);
        assert!(!user.is_super_user());
        assert!(!user.has_permission_to("READ_CLIENT"));
    }

    #[test]
    fn test_basic_auth_header_parsing_and_verification() {
        let verifier = BasicAuthVerifier::new();
        let user = sample_user_with_permissions(
            "mifos",
            vec![Permission::special(1, "ALL_FUNCTIONS")],
        );
        verifier.register_user(user.clone(), "password");

        let encoded = BasicAuthVerifier::encode_basic_credentials("mifos", "password");
        let header = format!("Basic {encoded}");

        let (parsed_user, parsed_pass) = BasicAuthVerifier::parse_basic_header(&header).unwrap();
        assert_eq!(parsed_user, "mifos");
        assert_eq!(parsed_pass, "password");

        let authenticated = verifier.verify_authorization_header(&header).unwrap();
        assert_eq!(authenticated.id, user.id);
        assert_eq!(authenticated.username, "mifos");

        // Wrong password fails with 401 Unauthenticated
        let bad_header = format!(
            "Basic {}",
            BasicAuthVerifier::encode_basic_credentials("mifos", "wrong")
        );
        assert_eq!(
            verifier
                .verify_authorization_header(&bad_header)
                .unwrap_err()
                .status_code(),
            StatusCode::UNAUTHORIZED
        );

        // Locked user fails verification
        let mut locked_user = user;
        locked_user.username = "locked_user".to_string();
        locked_user.account_non_locked = false;
        verifier.register_user(locked_user, "secret");
        let locked_header = format!(
            "Basic {}",
            BasicAuthVerifier::encode_basic_credentials("locked_user", "secret")
        );
        assert!(verifier.verify_authorization_header(&locked_header).is_err());
    }

    #[test]
    fn test_oauth2_jwt_verification_scopes_and_expiration() {
        let jwt_verifier = OAuth2JwtVerifier::new_hs256(b"fineract-oauth2-test-secret-key-256")
            .with_expected_issuer("https://fineract.apache.org")
            .with_required_scopes(["read"]);

        let user = sample_user_with_permissions(
            "mifos",
            vec![Permission::for_entity(10, "organisation", "OFFICE", "READ", false)],
        );
        jwt_verifier.register_user(user);

        let now = Utc::now().timestamp() as usize;
        let valid_claims = FineractJwtClaims {
            sub: "mifos".to_string(),
            exp: now + 3600,
            iat: Some(now),
            iss: Some("https://fineract.apache.org".to_string()),
            aud: None,
            tenant: Some("default".to_string()),
            scope: Some("read write".to_string()),
            scp: vec![],
            authorities: vec!["CREATE_CLIENT".to_string()],
            preferred_username: None,
        };

        let token = jwt_verifier.issue_token(&valid_claims).unwrap();
        let auth_header = format!("Bearer {token}");
        let verified = jwt_verifier
            .verify_authorization_header(&auth_header)
            .unwrap();
        assert_eq!(verified.user.username, "mifos");
        assert!(verified.user.has_permission_to("READ_OFFICE"));
        assert!(verified.user.has_permission_to("CREATE_CLIENT"));

        // Missing required scope 'read' -> 403 Forbidden
        let no_scope_claims = FineractJwtClaims {
            scope: Some("profile".to_string()),
            ..valid_claims.clone()
        };
        let no_scope_token = jwt_verifier.issue_token(&no_scope_claims).unwrap();
        assert_eq!(
            jwt_verifier
                .verify_token(&no_scope_token)
                .unwrap_err()
                .status_code(),
            StatusCode::FORBIDDEN
        );

        // Expired token -> 401 Unauthorized
        let expired_claims = FineractJwtClaims {
            exp: now.saturating_sub(120),
            ..valid_claims
        };
        let expired_token = jwt_verifier.issue_token(&expired_claims).unwrap();
        assert_eq!(
            jwt_verifier
                .verify_token(&expired_token)
                .unwrap_err()
                .status_code(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn test_twofactor_otp_and_access_token_lifecycle() {
        let tfa = TwoFactorAuthService::new(true);
        let user = sample_user_with_permissions(
            "mifos",
            vec![Permission::for_entity(1, "organisation", "OFFICE", "READ", false)],
        );

        // 1. Without TFA token, bootstrap endpoint `/api/v1/twofactor` is allowed,
        //    but `/api/v1/offices/1` is blocked with 403 Forbidden (matching TwoFactorAuthenticationTest.java).
        assert!(tfa
            .verify_request_tfa(&user, "/fineract-provider/api/v1/twofactor", None)
            .is_ok());
        assert!(tfa
            .verify_request_tfa(&user, "/fineract-provider/api/v1/twofactor/validate", None)
            .is_ok());
        assert_eq!(
            tfa.verify_request_tfa(&user, "/fineract-provider/api/v1/offices/1", None)
                .unwrap_err()
                .status_code(),
            StatusCode::FORBIDDEN
        );

        // 2. Request OTP & validate to obtain TFAccessToken
        let otp_req = tfa.create_new_otp_token(&user, "email", false).unwrap();
        assert_eq!(otp_req.token.len(), 6);
        assert!(otp_req.is_valid());

        let access_token = tfa
            .create_access_token_from_otp(&user, &otp_req.token)
            .unwrap();
        assert_eq!(access_token.token.len(), 32);
        assert!(access_token.is_valid());

        // 3. Access protected endpoint with valid Fineract-Platform-TFA-Token
        assert!(tfa
            .verify_request_tfa(
                &user,
                "/fineract-provider/api/v1/offices/1",
                Some(&access_token.token)
            )
            .is_ok());

        // 4. Invalidate token -> subsequent access with invalidated token returns 401 Unauthorized
        let invalidated = tfa
            .invalidate_access_token(&user, &access_token.token)
            .unwrap();
        assert!(!invalidated.enabled);
        assert!(!invalidated.is_valid());

        let err = tfa
            .verify_request_tfa(
                &user,
                "/fineract-provider/api/v1/offices/1",
                Some(&access_token.token),
            )
            .unwrap_err();
        assert_eq!(err, SecurityError::InvalidTwoFactorAccessToken);
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);

        // 5. User with BYPASS_TWOFACTOR bypasses 2FA enforcement even without header
        let bypass_user = sample_user_with_permissions(
            "system_bot",
            vec![Permission::special(99, BYPASS_TWO_FACTOR_PERMISSION)],
        );
        assert!(tfa
            .verify_request_tfa(&bypass_user, "/fineract-provider/api/v1/offices/1", None)
            .is_ok());
    }

    #[test]
    fn test_authenticate_and_build_context_end_to_end() {
        let basic = Arc::new(BasicAuthVerifier::new());
        let user = sample_user_with_permissions(
            "mifos",
            vec![Permission::special(1, "ALL_FUNCTIONS")],
        );
        basic.register_user(user, "password");

        let tfa = Arc::new(TwoFactorAuthService::new(true));
        let state = TenantAndSecurityState::new(basic, None, Arc::clone(&tfa));

        // 1. Missing `Fineract-Platform-TenantId` returns 400 Bad Request
        let empty_headers = HeaderMap::new();
        let err = authenticate_and_build_context(&state, &empty_headers, "/api/v1/offices")
            .unwrap_err();
        assert_eq!(err, SecurityError::MissingTenantIdentifier);
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);

        // 2. Authenticate on `/api/v1/twofactor` bootstrap path without TFA token
        let mut headers = HeaderMap::new();
        headers.insert(TENANT_ID_REQUEST_HEADER, HeaderValue::from_static("default"));
        let auth_val = format!(
            "Basic {}",
            BasicAuthVerifier::encode_basic_credentials("mifos", "password")
        );
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&auth_val).unwrap(),
        );

        let (ctx, authed_user) =
            authenticate_and_build_context(&state, &headers, "/api/v1/twofactor").unwrap();
        assert_eq!(ctx.tenant_id, "default");
        assert_eq!(ctx.authenticated_user_id, Some(10));
        assert!(ctx.is_super_user());

        // 3. Issue 2FA token and access `/api/v1/offices/1`
        let otp = tfa.create_new_otp_token(&authed_user, "email", false).unwrap();
        let tfa_tok = tfa
            .create_access_token_from_otp(&authed_user, &otp.token)
            .unwrap();
        headers.insert(
            TFA_TOKEN_REQUEST_HEADER,
            HeaderValue::from_str(&tfa_tok.token).unwrap(),
        );

        let (ctx2, _) =
            authenticate_and_build_context(&state, &headers, "/api/v1/offices/1").unwrap();
        assert_eq!(ctx2.tenant_id, "default");
        assert_eq!(ctx2.authenticated_user_id, Some(10));
        assert!(ctx2.is_super_user);
    }
}
