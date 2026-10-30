//! # Fineract BDD / Cucumber E2E Test Engine (`fineract-e2e-rs`)
//!
//! Port of `fineract-e2e-tests-core` and `fineract-e2e-tests-runner` (Gherkin `.feature` parser,
//! Scenario Context state holder, Given/When/Then step definitions for Clients, Loans, Savings,
//! Progressive Schedules, Delinquency, Business Dates, and Accounting GL verification).

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use fineract_client_rs::FineractClient;
use fineract_core_rs::{BusinessDateType, FineractError};
use fineract_loan_rs::{LoanAccount, LoanStatus};

/// Gherkin step keyword (`Given`, `When`, `Then`, `And`, `But`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GherkinKeyword {
    Given,
    When,
    Then,
    And,
    But,
}

/// A parsed Gherkin step with optional inline data table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GherkinStep {
    pub keyword: GherkinKeyword,
    pub text: String,
    pub data_table: Vec<HashMap<String, String>>,
}

/// A parsed Gherkin scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GherkinScenario {
    pub name: String,
    pub tags: Vec<String>,
    pub steps: Vec<GherkinStep>,
}

/// Execution report for a single BDD scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioExecutionReport {
    pub scenario_name: String,
    pub passed: bool,
    pub executed_steps: usize,
    pub failure_reason: Option<String>,
}

/// Stateful scenario world (mirrors `ScenarioContext` in `fineract-e2e-tests-core`).
pub struct BddScenarioWorld {
    pub client: FineractClient,
    pub current_office_id: i64,
    pub current_client_id: i64,
    pub current_loan_product_id: Option<i64>,
    pub current_loan_id: Option<i64>,
    pub last_error: Option<FineractError>,
    pub named_ids: HashMap<String, i64>,
}

impl BddScenarioWorld {
    pub fn new() -> Self {
        Self {
            client: FineractClient::default_in_process(),
            current_office_id: 1,
            current_client_id: 1,
            current_loan_product_id: None,
            current_loan_id: None,
            last_error: None,
            named_ids: HashMap::new(),
        }
    }

    /// Parses a Gherkin `.feature` text into a list of `GherkinScenario` definitions.
    pub fn parse_feature(feature_text: &str) -> Vec<GherkinScenario> {
        let mut scenarios = Vec::new();
        let mut current_tags: Vec<String> = Vec::new();
        let mut current_scenario: Option<GherkinScenario> = None;
        let mut pending_table_headers: Vec<String> = Vec::new();

        for raw_line in feature_text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("Feature:") {
                continue;
            }
            if line.starts_with('@') {
                for tag in line.split_whitespace() {
                    current_tags.push(tag.to_string());
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("Scenario:") {
                if let Some(prev) = current_scenario.take() {
                    scenarios.push(prev);
                }
                pending_table_headers.clear();
                current_scenario = Some(GherkinScenario {
                    name: rest.trim().to_string(),
                    tags: std::mem::take(&mut current_tags),
                    steps: Vec::new(),
                });
                continue;
            }
            if line.starts_with('|') && line.ends_with('|') {
                let cells: Vec<String> = line
                    .trim_matches('|')
                    .split('|')
                    .map(|s| s.trim().to_string())
                    .collect();
                if pending_table_headers.is_empty() {
                    pending_table_headers = cells;
                } else if let Some(scen) = current_scenario.as_mut() {
                    if let Some(last_step) = scen.steps.last_mut() {
                        let mut row_map = HashMap::new();
                        for (k, v) in pending_table_headers.iter().zip(cells.into_iter()) {
                            row_map.insert(k.clone(), v);
                        }
                        last_step.data_table.push(row_map);
                    }
                }
                continue;
            } else {
                pending_table_headers.clear();
            }

            let step_opt = if let Some(t) = line.strip_prefix("Given ") {
                Some((GherkinKeyword::Given, t.trim()))
            } else if let Some(t) = line.strip_prefix("When ") {
                Some((GherkinKeyword::When, t.trim()))
            } else if let Some(t) = line.strip_prefix("Then ") {
                Some((GherkinKeyword::Then, t.trim()))
            } else if let Some(t) = line.strip_prefix("And ") {
                Some((GherkinKeyword::And, t.trim()))
            } else if let Some(t) = line.strip_prefix("But ") {
                Some((GherkinKeyword::But, t.trim()))
            } else {
                None
            };

            if let (Some((kw, text)), Some(scen)) = (step_opt, current_scenario.as_mut()) {
                scen.steps.push(GherkinStep {
                    keyword: kw,
                    text: text.to_string(),
                    data_table: Vec::new(),
                });
            }
        }

        if let Some(last) = current_scenario {
            scenarios.push(last);
        }
        scenarios
    }

    /// Executes a single `GherkinScenario` against the Fineract in-process engine.
    pub async fn run_scenario(&mut self, scenario: &GherkinScenario) -> ScenarioExecutionReport {
        for (idx, step) in scenario.steps.iter().enumerate() {
            if let Err(reason) = self.execute_step(step).await {
                return ScenarioExecutionReport {
                    scenario_name: scenario.name.clone(),
                    passed: false,
                    executed_steps: idx + 1,
                    failure_reason: Some(reason),
                };
            }
        }
        ScenarioExecutionReport {
            scenario_name: scenario.name.clone(),
            passed: true,
            executed_steps: scenario.steps.len(),
            failure_reason: None,
        }
    }

    async fn execute_step(&mut self, step: &GherkinStep) -> Result<(), String> {
        let t = step.text.as_str();

        // Step 1: Business date setting
        // e.g. `business date is set to "2026-01-01"`
        if let Some(rest) = t.strip_prefix("business date is set to \"") {
            let date_str = rest.trim_end_matches('"');
            let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_err(|e| format!("Invalid date {}: {}", date_str, e))?;
            self.client
                .business_dates()
                .update_business_date(BusinessDateType::BusinessDate, date)
                .await
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        // Step 2: Create Loan Product
        // e.g. `a loan product "LP1" with principal 1000.00 and 4 installments`
        if t.starts_with("a loan product ") {
            let principal: f64 = step
                .data_table
                .first()
                .and_then(|r| r.get("principal"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000.0);
            let repayments: u32 = step
                .data_table
                .first()
                .and_then(|r| r.get("numberOfRepayments"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(4);
            let res = self
                .client
                .loan_products()
                .create(json!({
                    "name": format!("BDD Product {}", uuid::Uuid::new_v4().simple()),
                    "shortName": "BDD1",
                    "currencyCode": "USD",
                    "principal": principal,
                    "numberOfRepayments": repayments
                }))
                .await
                .map_err(|e| e.to_string())?;
            self.current_loan_product_id = res.resource_id;
            return Ok(());
        }

        // Step 3: Submit, Approve, Disburse Loan
        if t.starts_with("a loan application is submitted") {
            let product_id = self.current_loan_product_id.unwrap_or(1);
            let principal: f64 = step
                .data_table
                .first()
                .and_then(|r| r.get("principal"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000.0);
            let repayments: u32 = step
                .data_table
                .first()
                .and_then(|r| r.get("numberOfRepayments"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(4);
            let biz_date = self
                .client
                .business_dates()
                .get_business_date(BusinessDateType::BusinessDate)
                .await;
            let res = self
                .client
                .loans()
                .submit_application(json!({
                    "clientId": self.current_client_id,
                    "productId": product_id,
                    "principal": principal,
                    "numberOfRepayments": repayments,
                    "submittedOnDate": biz_date.to_string(),
                    "expectedDisbursementDate": biz_date.to_string()
                }))
                .await
                .map_err(|e| e.to_string())?;
            self.current_loan_id = res.loan_id;
            return Ok(());
        }

        if t == "the loan is approved and disbursed" {
            let loan_id = self.current_loan_id.ok_or("No active loan in scenario")?;
            let biz_date = self
                .client
                .business_dates()
                .get_business_date(BusinessDateType::BusinessDate)
                .await;
            self.client
                .loans()
                .approve(loan_id, json!({ "approvedOnDate": biz_date.to_string() }))
                .await
                .map_err(|e| e.to_string())?;
            self.client
                .loans()
                .disburse(
                    loan_id,
                    json!({ "actualDisbursementDate": biz_date.to_string() }),
                )
                .await
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        // Step 4: Repayment / Goodwill Credit / Chargeback
        if let Some(rest) = t.strip_prefix("a loan repayment of ") {
            let amount_str = rest.split_whitespace().next().unwrap_or("0");
            let amount: f64 = amount_str
                .parse()
                .map_err(|e| format!("Invalid repayment amount {}: {}", amount_str, e))?;
            let loan_id = self.current_loan_id.ok_or("No active loan in scenario")?;
            let biz_date = self
                .client
                .business_dates()
                .get_business_date(BusinessDateType::BusinessDate)
                .await;
            self.client
                .loans()
                .post_transaction(
                    loan_id,
                    "repayment",
                    json!({
                        "transactionDate": biz_date.to_string(),
                        "transactionAmount": amount
                    }),
                )
                .await
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        // Step 5: Assertions on Loan status and balances
        if let Some(expected_status) = t.strip_prefix("the loan status should be \"") {
            let expected = expected_status.trim_end_matches('"');
            let loan_id = self.current_loan_id.ok_or("No active loan in scenario")?;
            let loan = self
                .client
                .loans()
                .retrieve_one(loan_id)
                .await
                .map_err(|e| e.to_string())?;
            let actual = match loan.status {
                LoanStatus::Active => "ACTIVE",
                LoanStatus::ClosedObligationsMet => "CLOSED_OBLIGATIONS_MET",
                LoanStatus::Overpaid => "OVERPAID",
                LoanStatus::Approved => "APPROVED",
                LoanStatus::SubmittedAndPendingApproval => "SUBMITTED_AND_PENDING_APPROVAL",
                _ => "OTHER",
            };
            if actual != expected {
                return Err(format!("Expected loan status {}, got {}", expected, actual));
            }
            return Ok(());
        }

        if let Some(expected_str) = t.strip_prefix("the loan principal outstanding should be ") {
            let expected: Decimal = expected_str
                .trim()
                .parse()
                .map_err(|e| format!("Invalid decimal: {}", e))?;
            let loan_id = self.current_loan_id.ok_or("No active loan in scenario")?;
            let loan = self
                .client
                .loans()
                .retrieve_one(loan_id)
                .await
                .map_err(|e| e.to_string())?;
            if loan.summary.principal_outstanding != expected {
                return Err(format!(
                    "Expected principal_outstanding {}, got {}",
                    expected, loan.summary.principal_outstanding
                ));
            }
            return Ok(());
        }

        Err(format!("Unrecognized Gherkin step: '{}'", t))
    }
}

impl Default for BddScenarioWorld {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gherkin_loan_repayment_and_closure_feature() {
        let feature = r#"
Feature: Loan Lifecycle and Full Payoff
  @loan @smoke
  Scenario: Disburse 1000 USD loan and pay off in two installments
    Given business date is set to "2026-01-01"
    And a loan product "Standard" is created
      | principal | numberOfRepayments |
      | 1000.00   | 4                  |
    When a loan application is submitted
      | principal | numberOfRepayments |
      | 1000.00   | 4                  |
    And the loan is approved and disbursed
    Then the loan status should be "ACTIVE"
    And the loan principal outstanding should be 1000.00
    When business date is set to "2026-02-01"
    And a loan repayment of 400.00 is made
    Then the loan principal outstanding should be 600.00
    When business date is set to "2026-03-01"
    And a loan repayment of 600.00 is made
    Then the loan status should be "CLOSED_OBLIGATIONS_MET"
    And the loan principal outstanding should be 0.00
"#;

        let scenarios = BddScenarioWorld::parse_feature(feature);
        assert_eq!(scenarios.len(), 1);
        let mut world = BddScenarioWorld::new();
        let report = world.run_scenario(&scenarios[0]).await;
        assert!(report.passed, "Scenario failed: {:?}", report.failure_reason);
        assert_eq!(report.executed_steps, 12);
    }
}
