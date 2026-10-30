//! `fineract-savings-rs` — Savings Products, Fixed & Recurring Deposits, Interest Rate Charts,
//! Daily Compounding Interest Engine, Savings Account Lifecycle, Holds, Overdrafts, and Repositories.

pub mod account_and_transactions;
pub mod interest_engine;
pub mod product_and_charts;
pub mod repository_and_handlers;

pub use account_and_transactions::*;
pub use interest_engine::*;
pub use product_and_charts::*;
pub use repository_and_handlers::*;
