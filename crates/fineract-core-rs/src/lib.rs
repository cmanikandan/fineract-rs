//! `fineract-core-rs` — Apache Fineract Core Kernel
//!
//! Provides:
//! - `context`: Multi-tenant `FineractPlatformTenant`, `BusinessDateType` (`BUSINESS_DATE`, `COB_DATE`),
//!   `ActionContext`, and task-local `ExecutionContext` (replacing `ThreadLocalContextUtil`).
//! - `db`: Dual-database (`PostgreSQL` + `MySQL/MariaDB`) connection manager and SQL parameter placeholder
//!   dialect rewriter (`?` <-> `$1..$N`).
//! - `primitives`: `ExternalId`, `Page<T>`, `SearchParameters`, and `DomainDelta` persistence contracts.
//! - `json_command`: Two-pass locale-aware (`en_US`, `de_DE`, `fr_FR`) and `dateFormat`-aware JSON parser
//!   ported from `FromJsonHelper` and `JsonParserHelper`.
//! - `calendar`: Repayment schedule date generation, RFC 5545 recurrence rules, working-days, and holiday
//!   rescheduling ported from `CalendarUtils` and `WorkingDaysUtil`.

pub mod calendar;
pub mod context;
pub mod db;
pub mod json_command;
pub mod primitives;

pub use calendar::*;
pub use context::*;
pub use db::*;
pub use json_command::*;
pub use primitives::*;

pub use fineract_validation_rs::{
    ApiGlobalErrorResponse, FineractError, FineractResult, PlatformDomainRuleException,
};
