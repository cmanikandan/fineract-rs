# Fineract-RS Architectural Constitution (Layer 1 Frozen Anchor)

All Rust code decoded across Waves 1–7 MUST adhere to these 6 non-negotiable architectural invariants:

1. **Deterministic Financial Arithmetic (`fineract-money`)**:
   - NEVER use `f32` or `f64` for monetary amounts, interest rates, fees, taxes, or exchange rates.
   - Always use `rust_decimal::Decimal` wrapped in `FineractDecimal` or `Money` (`MonetaryCurrency` + `FineractDecimal`), enforcing explicit `RoundingMode` (`HalfUp`, `HalfEven`, `Ceiling`, `Floor`, `Down`, `Up`) matching `org.apache.fineract.organisation.monetary.domain.MoneyHelper`.

2. **Dual-Database Runtime Compatibility (`PostgreSQL 16+` & `MySQL / MariaDB`)**:
   - Persistence repositories MUST support both PostgreSQL and MySQL/MariaDB via `sqlx::AnyPool` and `SqlDialect` (`Postgres` vs `MySql`).
   - Use standard ANSI SQL types compatible with `anchor/fineract_schema_dual.sql` and pass queries through `DialectSql::bind_rewrite(sql, dialect)` to translate positional `?` placeholders to `$1, $2, ...` when executing on PostgreSQL.
   - For Close of Business (COB) locking (`m_loan_account_locks`), support both `SELECT ... FOR UPDATE SKIP LOCKED` (supported by PostgreSQL 9.5+ and MySQL 8.0+) and optimistic versioned lock acquisition.

3. **Two-Pass Locale & DateFormat JSON Deserialization (`FromFineractJson`)**:
   - Fineract API payloads include sibling fields `"locale"` (e.g., `"en_US"`, `"de_DE"`, `"fr_FR"`) and `"dateFormat"` (e.g., `"dd MMMM yyyy"`, `"yyyy-MM-dd"`).
   - Command payloads MUST extract `(locale, date_format)` in Pass 1 (`JsonCommandEnvelope`) before parsing formatted numbers (`"10.000,50"`) and dates (`"30 October 2026"`) in Pass 2.

4. **Pure Domain Aggregates + Explicit `DomainDelta` Persistence (No ORM Magic)**:
   - Do NOT replicate EclipseLink's mutable bidirectional entity graphs or static bytecode weaving.
   - Domain transitions are pure functions: `fn apply_command(state: &Aggregate, cmd: &Command, ctx: &BusinessContext) -> Result<(Aggregate, DomainDelta), DomainError>`.
   - Repositories apply `DomainDelta` inside a single ACID `sqlx::Transaction<'_, sqlx::Any>`.

5. **Thread-Safe Async Context (`TenantContext` + `BusinessDateContext`)**:
   - Replace Java's `ThreadLocalContextUtil` (`FineractContext`, `FineractPlatformTenant`, `BusinessDateType::BUSINESS_DATE`, `BusinessDateType::COB_DATE`, `ActionContext`) with an immutable `Arc<ExecutionContext>` propagated via `tokio::task_local!` and Axum request extensions.

6. **Wire-Compatible Error Globalisation Codes (`PlatformApiDataValidationException` & `PlatformDomainRuleException`)**:
   - Every validation and domain error MUST preserve Fineract's exact `globalisation_message_code` (e.g., `"error.msg.loan.disbursal.date.cannot.be.before.submitted.date"`), `default_user_message`, `parameter_name`, and `errors: Vec<ApiParameterError>` JSON structure so upstream `:integration-tests` and `:fineract-e2e-tests-runner` pass without modification.
