//! `fineract-command-rs` — Apache Fineract CQRS Command Spine
//!
//! Ported from `fineract-command`, `fineract-command-jdbc`, `fineract-command-audit`,
//! `fineract-command-async`, `fineract-command-disruptor`, and `fineract-core/commands`.
//!
//! - `pipeline`: Core `Command<T>`, `CommandWrapper`, `CommandProcessingResult`, `CommandHandler`,
//!   `MakerCheckerPolicy`, and `IdempotencyMiddleware`.
//! - `store_and_exec`: Dual-Database (`PostgreSQL` + `MySQL/MariaDB`) `CommandSourceJdbcStore`
//!   (`m_portfolio_command_source` table) and high-throughput `DisruptorCommandExecutor` & `AsyncCommandExecutor`.

pub mod pipeline;
pub mod store_and_exec;

pub use pipeline::*;
pub use store_and_exec::*;
