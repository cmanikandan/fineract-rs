#!/usr/bin/env python3
"""
Scaffolds all 28 crates across Waves 0-7 in `/Users/cmanikandan/Downloads/test-proj/fineract-rs`
so that every Wave's parallel subagent decode lanes have locked `Cargo.toml` manifests and module entrypoints.
"""

from pathlib import Path

RUST_ROOT = Path("/Users/cmanikandan/Downloads/test-proj/fineract-rs")
CRATES_DIR = RUST_ROOT / "crates"

CRATE_SPECS = [
    # Wave 0 & 1 (already created, kept in workspace list)
    ("fineract-avro-rs", [], "Auto-generated Apache Fineract Avro v1 External Event schemas (100 structs)"),
    ("fineract-money", [], "Deterministic financial arithmetic, MonetaryCurrency, Money, and rounding modes"),
    ("fineract-validation-rs", ["fineract-money"], "Wire-compatible DataValidatorBuilder, ApiParameterError, and globalisation error codes"),
    ("fineract-core-rs", ["fineract-money", "fineract-validation-rs"], "Core Kernel: Multi-tenant Context, Dual-DB SQL Dialect Layer, Locale JSON Parser, Calendar Engine"),
    ("fineract-command-rs", ["fineract-core-rs", "fineract-validation-rs", "fineract-money"], "CQRS Command Spine: Idempotency, Maker-Checker, Dual-DB Store, Disruptor Executor"),
    ("fineract-security-rs", ["fineract-core-rs", "fineract-validation-rs"], "Security Layer: Basic Auth, OAuth2/JWT, Two-Factor OTP, and Maker-Checker RBAC"),
    # Wave 2
    ("fineract-rates-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-command-rs"], "Floating Rates, Base Lending Rates, and Rate Period Engine"),
    ("fineract-tax-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-command-rs"], "Tax Components, Tax Groups, and Date-Effective Tax Splitting Engine"),
    ("fineract-charge-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-tax-rs", "fineract-command-rs"], "Loan & Savings Fee and Penalty Charge Calculation Engine"),
    ("fineract-branch-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-command-rs"], "Branch, Teller, and Cashier Cash Management & Vault Settlement Engine"),
    ("fineract-document-rs", ["fineract-core-rs", "fineract-validation-rs", "fineract-command-rs"], "Document Management, Content Repository (Filesystem/S3), and Entity Image Storage"),
    ("fineract-organisation-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-command-rs"], "Office Hierarchy, Staff, Working Days, Holidays, and Loan Provisioning Criteria"),
    ("fineract-accounting-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-organisation-rs", "fineract-command-rs"], "Double-Entry General Ledger, Chart of Accounts, GL Closures, and Cash/Accrual Accounting Bridge"),
    # Wave 3
    ("fineract-client-portfolio-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-organisation-rs", "fineract-command-rs"], "Client KYC, Groups, Centers, JLG Meetings, Standing Instructions & Account Transfers"),
    ("fineract-collateral-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-client-portfolio-rs", "fineract-command-rs"], "Collateral Management, Client Collateral Valuation, and Loan Collateral Pledging"),
    ("fineract-savings-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-tax-rs", "fineract-charge-rs", "fineract-accounting-rs", "fineract-client-portfolio-rs", "fineract-command-rs"], "Savings Accounts, Fixed & Recurring Deposits, Interest Rate Charts, Daily Compounding, Holds & Overdrafts"),
    # Wave 4
    ("fineract-schedule-engine", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-rates-rs", "fineract-charge-rs"], "Pure-Rust Embeddable EMI, Declining Balance, Flat, and Progressive Loan Schedule Calculator"),
    ("fineract-progressive-loan-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-schedule-engine", "fineract-charge-rs"], "Progressive Loan Interest Schedule Model, Advanced Payment Allocation, and Interest Recalculation"),
    ("fineract-loan-origination-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-client-portfolio-rs", "fineract-command-rs"], "Loan Origination, Credit Scoring, Co-Applicants, and Underwriting Workflow"),
    ("fineract-loan-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-schedule-engine", "fineract-progressive-loan-rs", "fineract-charge-rs", "fineract-tax-rs", "fineract-accounting-rs"], "Core Loan Lifecycle State Machine, Transaction Processors, Delinquency Buckets & Backdated Replay"),
    ("fineract-loan-provider-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-loan-rs", "fineract-loan-origination-rs", "fineract-collateral-rs", "fineract-accounting-rs", "fineract-command-rs"], "Loan Products, Multi-Tranche Disbursement, Rescheduling, Guarantor Locks & Dual-DB Loan Repositories"),
    # Wave 5
    ("fineract-working-capital-loan-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-schedule-engine", "fineract-loan-rs", "fineract-accounting-rs", "fineract-command-rs"], "Working Capital Loans, Revolving Credit Lines, Drawdowns, and Amortization Engine"),
    ("fineract-investor-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-loan-rs", "fineract-accounting-rs", "fineract-avro-rs", "fineract-command-rs"], "External Asset Owner (Investor) Loan Sale, Buyback, Settlement, and Sub-Ledger Accounting"),
    ("fineract-shares-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-charge-rs", "fineract-accounting-rs", "fineract-client-portfolio-rs", "fineract-command-rs"], "Share Products, Share Accounts, Dividend Declarations, and Share Redemption Engine"),
    ("fineract-interop-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-savings-rs", "fineract-loan-rs", "fineract-command-rs"], "Mojaloop / Open Payment Interoperation Engine (Party Lookup, Quotes, Transfers, Payer/Payee FSP)"),
    # Wave 6
    ("fineract-outbox-rs", ["fineract-core-rs", "fineract-avro-rs", "fineract-validation-rs"], "Transactional External Event Outbox (`m_external_event`), Avro Serializer, and Kafka/AMQP Relay"),
    ("fineract-cob-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-loan-rs", "fineract-loan-provider-rs", "fineract-investor-rs", "fineract-outbox-rs"], "Close of Business (COB) Partitioned Worker Pool, Account Lock Manager, Inline COB & Catch-up Engine"),
    ("fineract-batch-api-rs", ["fineract-core-rs", "fineract-validation-rs", "fineract-command-rs", "fineract-security-rs"], "Transactional Batch API Engine with JSONPath (`$.resourceId`) Dependency Resolution"),
    ("fineract-reporting-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-security-rs"], "Read Reporting Engine, Stretchy Reports, Dynamic DataTables (`x_registered_table`), and AdHoc Queries"),
    ("fineract-ops-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-command-rs"], "Bulk Import (Excel/CSV), Campaigns (SMS/Email), Webhooks, Surveys (SPM), Credit Bureau & Custom Wasm Hooks"),
    ("fineract-server-rs", [
        "fineract-avro-rs", "fineract-money", "fineract-validation-rs", "fineract-core-rs", "fineract-command-rs", "fineract-security-rs",
        "fineract-rates-rs", "fineract-tax-rs", "fineract-charge-rs", "fineract-branch-rs", "fineract-document-rs", "fineract-organisation-rs", "fineract-accounting-rs",
        "fineract-client-portfolio-rs", "fineract-collateral-rs", "fineract-savings-rs",
        "fineract-schedule-engine", "fineract-progressive-loan-rs", "fineract-loan-origination-rs", "fineract-loan-rs", "fineract-loan-provider-rs",
        "fineract-working-capital-loan-rs", "fineract-investor-rs", "fineract-shares-rs", "fineract-interop-rs",
        "fineract-outbox-rs", "fineract-cob-rs", "fineract-batch-api-rs", "fineract-reporting-rs", "fineract-ops-rs"
    ], "Unified Axum HTTP Server & OpenAPI 3.1 Gateway assembling all Fineract-RS domain routes"),
    # Wave 7
    ("fineract-client-rs", ["fineract-core-rs", "fineract-money", "fineract-validation-rs", "fineract-avro-rs"], "Typed Async Rust HTTP Client SDK for Apache Fineract (replacing fineract-client & fineract-client-feign)"),
    ("fineract-e2e-rs", ["fineract-server-rs", "fineract-client-rs"], "BDD / Cucumber End-to-End Test Runner & Step Definitions ported from fineract-e2e-tests-core"),
    ("fineract-integration-tests-rs", ["fineract-server-rs", "fineract-client-rs"], "Comprehensive Integration Test Suite ported from upstream integration-tests"),
]


def main():
    members = [f'    "crates/{name}",' for name, _, _ in CRATE_SPECS]
    ws_deps = [f'{name} = {{ path = "crates/{name}" }}' for name, _, _ in CRATE_SPECS]

    root_cargo = f"""[workspace]
resolver = "2"
members = [
{chr(10).join(members)}
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
repository = "https://github.com/apache/fineract"

[workspace.dependencies]
{chr(10).join(ws_deps)}

# Async & Web Runtime
tokio = {{ version = "1.40", features = ["full"] }}
axum = {{ version = "0.7", features = ["macros", "json"] }}
tower = {{ version = "0.5", features = ["util", "timeout", "limit"] }}
tower-http = {{ version = "0.6", features = ["trace", "cors", "auth"] }}
async-trait = "0.1"
futures = "0.3"

# Serialization & Avro
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = {{ version = "1.0", features = ["preserve_order"] }}
apache-avro = {{ version = "0.17", features = ["snappy"] }}

# Financial Math, Dates, UUIDs, Regex
rust_decimal = {{ version = "1.36", features = ["serde", "serde-with-str", "maths"] }}
rust_decimal_macros = "1.36"
chrono = {{ version = "0.4", features = ["serde"] }}
uuid = {{ version = "1.10", features = ["v4", "v7", "serde"] }}
regex = "1.11"
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"

# Dual-Database Persistence (PostgreSQL + MySQL/MariaDB via sqlx::Any)
sqlx = {{ version = "0.8", features = [
    "runtime-tokio",
    "tls-rustls",
    "any",
    "postgres",
    "mysql",
    "chrono",
    "rust_decimal",
    "uuid",
    "json",
] }}

# Security & Crypto
jsonwebtoken = "9.3"
sha2 = "0.10"
hmac = "0.12"
base64 = "0.22"
dashmap = "6.1"

# Testing
proptest = "1.5"
"""
    (RUST_ROOT / "Cargo.toml").write_text(root_cargo, encoding="utf-8")

    already_built = {
        "fineract-avro-rs",
        "fineract-money",
        "fineract-validation-rs",
        "fineract-core-rs",
        "fineract-command-rs",
        "fineract-security-rs",
    }

    for name, internal_deps, desc in CRATE_SPECS:
        if name in already_built:
            continue
        cdir = CRATES_DIR / name
        sdir = cdir / "src"
        sdir.mkdir(parents=True, exist_ok=True)
        dep_lines = [f"{d} = {{ workspace = true }}" for d in internal_deps]
        cargo_toml = f"""[package]
name = "{name}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "{desc}"

[dependencies]
{chr(10).join(dep_lines)}
tokio = {{ workspace = true }}
axum = {{ workspace = true }}
sqlx = {{ workspace = true }}
serde = {{ workspace = true }}
serde_json = {{ workspace = true }}
rust_decimal = {{ workspace = true }}
rust_decimal_macros = {{ workspace = true }}
chrono = {{ workspace = true }}
uuid = {{ workspace = true }}
regex = {{ workspace = true }}
dashmap = {{ workspace = true }}
thiserror = {{ workspace = true }}
anyhow = {{ workspace = true }}
tracing = {{ workspace = true }}
async-trait = {{ workspace = true }}

[dev-dependencies]
proptest = {{ workspace = true }}
"""
        (cdir / "Cargo.toml").write_text(cargo_toml, encoding="utf-8")
        lib_rs = sdir / "lib.rs"
        if not lib_rs.exists():
            lib_rs.write_text(f"//! `{name}` — {desc}\n", encoding="utf-8")

    print(f"Scaffolded all {len(CRATE_SPECS)} workspace crates in {CRATES_DIR}")


if __name__ == "__main__":
    main()
