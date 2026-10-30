//! `fineract-accounting-rs` — Double-Entry General Ledger, Chart of Accounts, GL Closures,
//! Financial Activity Accounts, Provisioning Entries, and Cash/Accrual Accounting Bridge.
//!
//! Ported from `fineract-accounting` and `fineract-provider/accounting`.

pub mod gl_and_rules;
pub mod journal_processor;

pub use gl_and_rules::*;
pub use journal_processor::*;
