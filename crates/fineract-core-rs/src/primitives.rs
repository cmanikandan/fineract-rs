//! Core Fineract value objects, pagination parameters, audit metadata, and explicit `DomainDelta` persistence contracts.
//!
//! Ported from:
//! - `org.apache.fineract.infrastructure.core.domain.ExternalId`
//! - `org.apache.fineract.infrastructure.core.service.Page`
//! - `org.apache.fineract.infrastructure.core.service.SearchParameters`
//! - `org.apache.fineract.infrastructure.core.domain.AbstractAuditableWithUTCDateTimeCustom`

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::context::current_execution_context;

/// Error returned when an [`ExternalId`] operation violates Fineract invariants.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExternalIdError {
    #[error("error.external.id.cannot.be.blank")]
    BlankExternalId,

    #[error("error.external.id.is.not.set")]
    ExternalIdNotSet,
}

/// Value object representing an optional external identifier (`external_id`).
///
/// Ported from `org.apache.fineract.infrastructure.core.domain.ExternalId`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExternalId(pub Option<String>);

impl ExternalId {
    /// Creates an empty [`ExternalId`] (`None`), matching `ExternalId.empty()`.
    pub const fn empty() -> Self {
        Self(None)
    }

    /// Creates a new [`ExternalId`] from a string value.
    ///
    /// Panics with `"error.external.id.cannot.be.blank"` if `value` is blank, matching
    /// `new ExternalId(String value)` in Java. Use [`ExternalId::try_new`] or [`ExternalId::from_opt`]
    /// for fallible/nullable construction.
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        assert!(
            !value.trim().is_empty(),
            "error.external.id.cannot.be.blank"
        );
        Self(Some(value))
    }

    /// Fallible constructor returning [`ExternalIdError::BlankExternalId`] if `value` is blank.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ExternalIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            Err(ExternalIdError::BlankExternalId)
        } else {
            Ok(Self(Some(value)))
        }
    }

    /// Constructs an [`ExternalId`] from an optional string, normalizing empty/blank strings to `None`.
    pub fn from_opt(value: Option<String>) -> Self {
        match value {
            Some(s) if !s.trim().is_empty() => Self(Some(s)),
            _ => Self(None),
        }
    }

    /// Generates a new [`ExternalId`] backed by a random UUID v4, matching `ExternalId.generate()`.
    pub fn generate() -> Self {
        Self(Some(Uuid::new_v4().to_string()))
    }

    /// Returns `true` if the external ID is unset (`None`) or blank, matching `ExternalId.isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().map_or(true, |s| s.trim().is_empty())
    }

    /// Returns the underlying string slice if present, matching `ExternalId.getValue()`.
    pub fn value(&self) -> Option<&str> {
        self.0.as_deref()
    }

    /// Validates that the [`ExternalId`] is set, returning [`ExternalIdError::ExternalIdNotSet`]
    /// (`"error.external.id.is.not.set"`) if empty, matching `ExternalId.throwExceptionIfEmpty()`.
    pub fn throw_exception_if_empty(&self) -> Result<&str, ExternalIdError> {
        match self.value() {
            Some(v) if !v.trim().is_empty() => Ok(v),
            _ => Err(ExternalIdError::ExternalIdNotSet),
        }
    }
}

impl fmt::Display for ExternalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(v) = &self.0 {
            f.write_str(v)
        } else {
            Ok(())
        }
    }
}

impl From<Option<String>> for ExternalId {
    fn from(value: Option<String>) -> Self {
        Self::from_opt(value)
    }
}

impl From<String> for ExternalId {
    fn from(value: String) -> Self {
        Self::from_opt(Some(value))
    }
}

impl From<&str> for ExternalId {
    fn from(value: &str) -> Self {
        Self::from_opt(Some(value.to_string()))
    }
}

/// Paginated response envelope matching `org.apache.fineract.infrastructure.core.service.Page<E>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total_filtered_records: i64,
    pub page_items: Vec<T>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            total_filtered_records: 0,
            page_items: Vec::new(),
        }
    }
}

impl<T> Page<T> {
    /// Creates a new [`Page<T>`] matching `new Page<>(pageItems, totalFilteredRecords)`.
    pub fn new(page_items: Vec<T>, total_filtered_records: i64) -> Self {
        Self {
            total_filtered_records,
            page_items,
        }
    }

    /// Creates an empty [`Page<T>`].
    pub const fn empty() -> Self {
        Self {
            total_filtered_records: 0,
            page_items: Vec::new(),
        }
    }

    /// Returns the total number of filtered records across all pages.
    pub const fn total_filtered_records(&self) -> i64 {
        self.total_filtered_records
    }

    /// Returns a slice of the items on the current page.
    pub fn page_items(&self) -> &[T] {
        &self.page_items
    }

    /// Transforms the items of this page while preserving `total_filtered_records`.
    pub fn map<U, F>(self, f: F) -> Page<U>
    where
        F: FnMut(T) -> U,
    {
        Page {
            total_filtered_records: self.total_filtered_records,
            page_items: self.page_items.into_iter().map(f).collect(),
        }
    }
}

/// Pagination and sorting query parameters matching `org.apache.fineract.infrastructure.core.service.SearchParameters`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParameters {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub order_by: Option<String>,
    pub sort_order: Option<String>,
}

impl SearchParameters {
    /// Default maximum page limit matching `SearchParameters.DEFAULT_MAX_LIMIT`.
    pub const DEFAULT_MAX_LIMIT: i64 = 200;

    /// Creates a new [`SearchParameters`].
    pub fn new(
        offset: Option<i64>,
        limit: Option<i64>,
        order_by: Option<String>,
        sort_order: Option<String>,
    ) -> Self {
        Self {
            offset,
            limit,
            order_by,
            sort_order,
        }
    }

    /// Computes the effective limit matching `SearchParameters.getLimit()`:
    /// - `None` -> `Some(DEFAULT_MAX_LIMIT)` (`200`)
    /// - `Some(n)` where `n > 0` -> `Some(n)`
    /// - `Some(n)` where `n <= 0` -> `None` (unlimited)
    pub fn get_limit(&self) -> Option<i64> {
        match self.limit {
            None => Some(Self::DEFAULT_MAX_LIMIT),
            Some(n) if n > 0 => Some(n),
            Some(_) => None,
        }
    }

    /// Returns `true` if a positive `limit` was explicitly supplied.
    pub fn has_limit(&self) -> bool {
        matches!(self.limit, Some(n) if n > 0)
    }

    /// Returns `true` if `offset` was supplied.
    pub fn has_offset(&self) -> bool {
        self.offset.is_some()
    }

    /// Returns `true` if `order_by` is present and non-blank.
    pub fn has_order_by(&self) -> bool {
        self.order_by.as_ref().is_some_and(|s| !s.trim().is_empty())
    }

    /// Returns `true` if `sort_order` is present and non-blank.
    pub fn has_sort_order(&self) -> bool {
        self.sort_order
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
    }
}

/// Standard UTC audit columns (`created_by`, `created_on_utc`, `last_modified_by`, `last_modified_on_utc`)
/// matching `AbstractAuditableWithUTCDateTimeCustom`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditableFields {
    pub created_by: Option<i64>,
    pub created_on_utc: Option<DateTime<Utc>>,
    pub last_modified_by: Option<i64>,
    pub last_modified_on_utc: Option<DateTime<Utc>>,
}

impl AuditableFields {
    /// Initializes [`AuditableFields`] for a newly created entity.
    pub const fn new(user_id: Option<i64>, now: DateTime<Utc>) -> Self {
        Self {
            created_by: user_id,
            created_on_utc: Some(now),
            last_modified_by: user_id,
            last_modified_on_utc: Some(now),
        }
    }

    /// Initializes [`AuditableFields`] from the active task-local [`ExecutionContext`] and `Utc::now()`.
    pub fn from_current_context() -> Self {
        let ctx = current_execution_context();
        Self::new(ctx.authenticated_user_id, Utc::now())
    }

    /// Updates `last_modified_by` and `last_modified_on_utc`.
    pub fn touch(&mut self, user_id: Option<i64>, now: DateTime<Utc>) {
        self.last_modified_by = user_id;
        self.last_modified_on_utc = Some(now);
    }

    /// Updates `last_modified_by` and `last_modified_on_utc` from the active task-local [`ExecutionContext`].
    pub fn touch_from_current_context(&mut self) {
        let ctx = current_execution_context();
        self.touch(ctx.authenticated_user_id, Utc::now());
    }
}

/// Explicit non-ORM persistence operation applied inside an ACID transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationOp<T> {
    Insert(T),
    Update(T),
    Delete(i64),
}

impl<T> MutationOp<T> {
    pub const fn is_insert(&self) -> bool {
        matches!(self, Self::Insert(_))
    }

    pub const fn is_update(&self) -> bool {
        matches!(self, Self::Update(_))
    }

    pub const fn is_delete(&self) -> bool {
        matches!(self, Self::Delete(_))
    }

    pub fn map<U, F>(self, f: F) -> MutationOp<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Insert(v) => MutationOp::Insert(f(v)),
            Self::Update(v) => MutationOp::Update(f(v)),
            Self::Delete(id) => MutationOp::Delete(id),
        }
    }
}

/// Pure domain transition delta capturing explicit persistence mutations and attribute changes
/// without ORM dirty-checking or bytecode weaving (Invariant 4 of `rust_constitution.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDelta<Change = serde_json::Value> {
    pub mutations: Vec<MutationOp<Change>>,
    pub changes: Vec<Change>,
    pub modified_fields: serde_json::Map<String, serde_json::Value>,
}

impl<Change> Default for DomainDelta<Change> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Change> DomainDelta<Change> {
    /// Creates an empty [`DomainDelta`].
    pub const fn new() -> Self {
        Self {
            mutations: Vec::new(),
            changes: Vec::new(),
            modified_fields: serde_json::Map::new(),
        }
    }

    /// Creates an empty [`DomainDelta`].
    pub const fn empty() -> Self {
        Self::new()
    }

    /// Creates a [`DomainDelta`] from a list of [`MutationOp<Change>`].
    pub fn from_mutations(mutations: Vec<MutationOp<Change>>) -> Self {
        Self {
            mutations,
            changes: Vec::new(),
            modified_fields: serde_json::Map::new(),
        }
    }

    /// Creates a [`DomainDelta`] from a list of domain `Change` records.
    pub fn from_changes(changes: Vec<Change>) -> Self {
        Self {
            mutations: Vec::new(),
            changes,
            modified_fields: serde_json::Map::new(),
        }
    }

    /// Appends a [`MutationOp`] to this delta.
    pub fn push_mutation(&mut self, op: MutationOp<Change>) {
        self.mutations.push(op);
    }

    /// Appends a [`MutationOp::Insert`] to this delta.
    pub fn insert(&mut self, entity: Change) {
        self.mutations.push(MutationOp::Insert(entity));
    }

    /// Appends a [`MutationOp::Update`] to this delta.
    pub fn update(&mut self, entity: Change) {
        self.mutations.push(MutationOp::Update(entity));
    }

    /// Appends a [`MutationOp::Delete`] by primary key `id` to this delta.
    pub fn delete(&mut self, id: i64) {
        self.mutations.push(MutationOp::Delete(id));
    }

    /// Appends a domain change event/entry to `changes`.
    pub fn push_change(&mut self, change: Change) {
        self.changes.push(change);
    }

    /// Records a modified field for `CommandProcessingResult.changes`.
    pub fn record_field_change(
        &mut self,
        field: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) {
        self.modified_fields.insert(field.into(), value.into());
    }

    /// Returns `true` if no mutations, domain changes, or modified fields were recorded.
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty() && self.changes.is_empty() && self.modified_fields.is_empty()
    }

    /// Merges another [`DomainDelta`] into `self` in order.
    pub fn extend(&mut self, other: Self) {
        self.mutations.extend(other.mutations);
        self.changes.extend(other.changes);
        self.modified_fields.extend(other.modified_fields);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_id_semantics() {
        let empty = ExternalId::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.value(), None);
        assert_eq!(
            empty.throw_exception_if_empty(),
            Err(ExternalIdError::ExternalIdNotSet)
        );

        let generated = ExternalId::generate();
        assert!(!generated.is_empty());
        assert!(generated.value().unwrap().len() >= 32);
        assert!(generated.throw_exception_if_empty().is_ok());

        assert_eq!(
            ExternalId::try_new("   "),
            Err(ExternalIdError::BlankExternalId)
        );
    }

    #[test]
    fn test_page_and_search_parameters() {
        let page = Page::new(vec!["loan-1", "loan-2"], 42);
        assert_eq!(page.total_filtered_records(), 42);
        assert_eq!(page.page_items().len(), 2);

        let default_params = SearchParameters::default();
        assert_eq!(
            default_params.get_limit(),
            Some(SearchParameters::DEFAULT_MAX_LIMIT)
        );
        assert!(!default_params.has_limit());

        let unlimited_params = SearchParameters::new(Some(0), Some(0), None, None);
        assert_eq!(unlimited_params.get_limit(), None);
    }

    #[test]
    fn test_domain_delta_and_mutation_ops() {
        let mut delta: DomainDelta<String> = DomainDelta::new();
        assert!(delta.is_empty());

        delta.insert("row_1".to_string());
        delta.update("row_1_v2".to_string());
        delta.delete(99);
        delta.record_field_change("principal", serde_json::json!(1000));

        assert!(!delta.is_empty());
        assert_eq!(delta.mutations.len(), 3);
        assert!(delta.mutations[0].is_insert());
        assert!(delta.mutations[1].is_update());
        assert!(delta.mutations[2].is_delete());
    }
}
