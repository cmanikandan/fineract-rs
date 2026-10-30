#!/usr/bin/env python3
"""
argon_context_packer.py — Deterministic Context Compiler & Multi-File Unpacker for Gemini 4 Argon
Builds the 3-Layer Long-Context Prompt Bundle for any Wave Pod (Waves 1..7) and extracts
Layer 2 Upstream Rust Public API headers (`anchor/upstream_rust_api.rs`).
"""

import argparse
import json
import re
from pathlib import Path

ROOT = Path("/Users/cmanikandan/Downloads/test-proj")
JAVA_ROOT = ROOT / "fineract-java"
RUST_ROOT = ROOT / "fineract-rs"
ANCHOR_DIR = RUST_ROOT / "anchor"

WAVE_POD_MANIFEST = {
    "wave1": {
        "description": "Core Kernel, Money, Command Spine, Security & Validation",
        "crates": [
            "fineract-money",
            "fineract-core-rs",
            "fineract-validation-rs",
            "fineract-command-rs",
            "fineract-security-rs",
        ],
        "java_globs": [
            "fineract-validation/src/**/*.java",
            "fineract-command*/src/**/*.java",
            "fineract-security/src/**/*.java",
            "oauth2-tests/src/**/*.java",
            "twofactor-tests/src/**/*.java",
            "fineract-core/src/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/core/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/security/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/configuration/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/cache/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/entityaccess/**/*.java",
        ],
    },
    "wave2": {
        "description": "Reference Data, Organization, Accounting & Charges/Tax",
        "crates": [
            "fineract-rates-rs",
            "fineract-tax-rs",
            "fineract-charge-rs",
            "fineract-branch-rs",
            "fineract-document-rs",
            "fineract-organisation-rs",
            "fineract-accounting-rs",
        ],
        "java_globs": [
            "fineract-rates/src/**/*.java",
            "fineract-tax/src/**/*.java",
            "fineract-charge/src/**/*.java",
            "fineract-branch/src/**/*.java",
            "fineract-document/src/**/*.java",
            "fineract-accounting/src/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/accounting/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/organisation/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/useradministration/**/*.java",
        ],
    },
    "wave3a": {
        "description": "Client, Group, Collateral & Account Transfers",
        "crates": ["fineract-client-portfolio-rs", "fineract-collateral-rs"],
        "java_globs": [
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/client/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/group/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/address/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/meeting/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/calendar/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/collateral/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/collateralmanagement/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/account/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/accountdetails/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/paymentdetail/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/fund/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/note/**/*.java",
        ],
    },
    "wave3b": {
        "description": "Savings Engine & Interest Rate Charts",
        "crates": ["fineract-savings-rs"],
        "java_globs": [
            "fineract-savings/src/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/savings/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/interestratechart/**/*.java",
        ],
    },
    "wave4a": {
        "description": "Embeddable Schedule Generator, Progressive Loan & Loan Origination",
        "crates": ["fineract-schedule-engine", "fineract-progressive-loan-rs", "fineract-loan-origination-rs"],
        "java_globs": [
            "fineract-progressive-loan-embeddable-schedule-generator/src/**/*.java",
            "fineract-progressive-loan/src/**/*.java",
            "fineract-loan-origination/src/**/*.java",
        ],
    },
    "wave4b": {
        "description": "Core Loan Domain (fineract-loan)",
        "crates": ["fineract-loan-rs"],
        "java_globs": ["fineract-loan/src/**/*.java"],
    },
    "wave4c": {
        "description": "Provider Loan Orchestration & Products",
        "crates": ["fineract-loan-provider-rs"],
        "java_globs": [
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/loanaccount/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/loanproduct/**/*.java",
        ],
    },
    "wave5a": {
        "description": "Working Capital Loan Engine",
        "crates": ["fineract-working-capital-loan-rs"],
        "java_globs": ["fineract-working-capital-loan/src/**/*.java"],
    },
    "wave5b": {
        "description": "Investor Sub-Ledger, Share Accounts & Interoperation",
        "crates": ["fineract-investor-rs", "fineract-shares-rs", "fineract-interop-rs"],
        "java_globs": [
            "fineract-investor/src/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/shareaccounts/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/shareproducts/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/transfer/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/delinquency/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/rate/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/floatingrates/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/coapplicant/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/interoperation/**/*.java",
        ],
    },
    "wave6a": {
        "description": "Close of Business (COB), Batch API, Jobs & Event Outbox",
        "crates": ["fineract-cob-rs", "fineract-outbox-rs", "fineract-batch-api-rs"],
        "java_globs": [
            "fineract-cob/src/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/cob/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/batch/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/event/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/jobs/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/springbatch/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/hooks/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/sms/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/campaigns/**/*.java",
        ],
    },
    "wave6b": {
        "description": "Reporting, DataQueries, Bulk Import, Surveys & Custom Extensions",
        "crates": ["fineract-reporting-rs", "fineract-ops-rs"],
        "java_globs": [
            "fineract-report/src/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/dataqueries/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/bulkimport/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/reportmailingjob/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/creditbureau/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/survey/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/documentmanagement/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/codes/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/infrastructure/gcm/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/spm/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/template/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/notification/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/adhocquery/**/*.java",
            "fineract-provider/src/main/java/org/apache/fineract/portfolio/collectionsheet/**/*.java",
            "custom/**/*.java",
        ],
    },
}


def extract_upstream_rust_stubs() -> str:
    """Extract public API signatures (`pub struct`, `pub enum`, `pub trait`, `pub fn`, `pub type`) from crates/."""
    crates_dir = RUST_ROOT / "crates"
    if not crates_dir.exists():
        return "// No upstream crates generated yet.\n"

    stub_lines = [
        "// =============================================================================",
        "// LAYER 2: UPSTREAM RUST PUBLIC API STUBS (Auto-extracted across workspace)",
        "// =============================================================================",
    ]
    pub_pattern = re.compile(
        r"^\s*(pub\s+(?:async\s+)?(?:fn|struct|enum|trait|type|mod|const)\s+[^{;]+)"
    )

    for crate_path in sorted(crates_dir.iterdir()):
        if not crate_path.is_dir() or crate_path.name == "fineract-avro-rs":
            continue
        stub_lines.append(f"\n// --- CRATE: {crate_path.name} ---")
        for rs_file in sorted((crate_path / "src").rglob("*.rs")):
            rel = rs_file.relative_to(crate_path)
            stub_lines.append(f"// File: {crate_path.name}/{rel}")
            for line in rs_file.read_text(encoding="utf-8").splitlines():
                m = pub_pattern.match(line)
                if m:
                    stub_lines.append(m.group(1).strip() + ";")

    out = "\n".join(stub_lines)
    (ANCHOR_DIR / "upstream_rust_api.rs").write_text(out, encoding="utf-8")
    return out


def summarize_pods():
    print(f"{'Pod ID':10s} | {'Files':6s} | {'Est. Tokens':12s} | {'Target Crates'}")
    print("-" * 85)
    for pod_id, spec in WAVE_POD_MANIFEST.items():
        matched = set()
        for g in spec["java_globs"]:
            for p in JAVA_ROOT.glob(g):
                if p.is_file():
                    matched.add(p)
        total_bytes = sum(p.stat().st_size for p in matched)
        print(
            f"{pod_id:10s} | {len(matched):6d} | {total_bytes // 4:12,d} | {', '.join(spec['crates'])}"
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--summary", action="store_true", help="Print token summary of all Wave Pods")
    parser.add_argument("--extract-stubs", action="store_true", help="Refresh Layer 2 Rust API stubs")
    args = parser.parse_args()
    if args.extract_stubs:
        stubs = extract_upstream_rust_stubs()
        print(f"Extracted Layer 2 stubs ({len(stubs):,d} bytes)")
    if args.summary or not args.extract_stubs:
        summarize_pods()
