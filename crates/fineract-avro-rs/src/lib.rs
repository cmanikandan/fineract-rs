//! Auto-generated from `fineract-avro-schemas` (101 `.avsc` files).
//! Provides strongly-typed Rust structs and raw Avro JSON schema constants for the Fineract Transactional Outbox (`m_external_event`).
#![allow(clippy::all, non_camel_case_types)]

use serde::{Deserialize, Serialize};

/// Avro schema `org.apache.fineract.avro.BulkMessageItemV1` (Namespace: `org.apache.fineract.avro`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkMessageItemV1 {
    /// The ID of the message to be sent
    pub id: i64,
    /// The type of event the payload refers to. For example LoanApprovedBusinessEvent
    pub r#type: String,
    /// The category of event the payload refers to. For example LOAN
    pub category: String,
    /// The fully qualified name of the schema of the event payload. For example org.apache.fineract.avro.loan.v1.LoanAccountDataV1
    pub dataschema: String,
    /// The payload data serialized into Avro bytes
    pub data: Vec<u8>,
}

pub const BULK_MESSAGE_ITEM_V1_AVSC: &str = r#"{"name":"BulkMessageItemV1","namespace":"org.apache.fineract.avro","type":"record","fields":[{"name":"id","doc":"The ID of the message to be sent","type":"long"},{"name":"type","doc":"The type of event the payload refers to. For example LoanApprovedBusinessEvent","type":"string"},{"name":"category","doc":"The category of event the payload refers to. For example LOAN","type":"string"},{"name":"dataschema","doc":"The fully qualified name of the schema of the event payload. For example org.apache.fineract.avro.loan.v1.LoanAccountDataV1","type":"string"},{"name":"data","doc":"The payload data serialized into Avro bytes","type":"bytes"}]}"#;

/// Avro schema `org.apache.fineract.avro.BulkMessagePayloadV1` (Namespace: `org.apache.fineract.avro`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkMessagePayloadV1 {
    /// The individual messages within this bulk message
    pub datas: Vec<BulkMessageItemV1>,
}

pub const BULK_MESSAGE_PAYLOAD_V1_AVSC: &str = r#"{"name":"BulkMessagePayloadV1","namespace":"org.apache.fineract.avro","type":"record","fields":[{"name":"datas","doc":"The individual messages within this bulk message","type":{"type":"array","items":"org.apache.fineract.avro.BulkMessageItemV1"}}]}"#;

/// Avro schema `org.apache.fineract.avro.MessageV1` (Namespace: `org.apache.fineract.avro`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageV1 {
    /// The ID of the message to be sent
    pub id: i64,
    /// A unique identifier of the source service
    pub source: String,
    /// The type of event the payload refers to. For example LoanApprovedBusinessEvent
    pub r#type: String,
    /// The category of event the payload refers to. For example LOAN
    pub category: String,
    /// The UTC time of when the event has been raised; in ISO_LOCAL_DATE_TIME format. For example 2011-12-03T10:15:30
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// The business date when the event has been raised; in ISO_LOCAL_DATE format. For example 2011-12-03
    #[serde(rename = "businessDate")]
    pub business_date: String,
    /// The tenantId that the event has been sent from. For example default
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    /// The idempotency key for this particular event for consumer de-duplication
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
    /// The fully qualified name of the schema of the event payload. For example org.apache.fineract.avro.loan.v1.LoanAccountDataV1
    pub dataschema: String,
    /// The payload data serialized into Avro bytes
    pub data: Vec<u8>,
}

pub const MESSAGE_V1_AVSC: &str = r#"{"name":"MessageV1","namespace":"org.apache.fineract.avro","type":"record","fields":[{"name":"id","doc":"The ID of the message to be sent","type":"long"},{"name":"source","doc":"A unique identifier of the source service","type":"string"},{"name":"type","doc":"The type of event the payload refers to. For example LoanApprovedBusinessEvent","type":"string"},{"name":"category","doc":"The category of event the payload refers to. For example LOAN","type":"string"},{"name":"createdAt","doc":"The UTC time of when the event has been raised; in ISO_LOCAL_DATE_TIME format. For example 2011-12-03T10:15:30","type":"string"},{"name":"businessDate","doc":"The business date when the event has been raised; in ISO_LOCAL_DATE format. For example 2011-12-03","type":"string"},{"name":"tenantId","doc":"The tenantId that the event has been sent from. For example default","type":"string"},{"name":"idempotencyKey","doc":"The idempotency key for this particular event for consumer de-duplication","type":"string"},{"name":"dataschema","doc":"The fully qualified name of the schema of the event payload. For example org.apache.fineract.avro.loan.v1.LoanAccountDataV1","type":"string"},{"name":"data","doc":"The payload data serialized into Avro bytes","type":"bytes"}]}"#;

/// Avro schema `org.apache.fineract.avro.client.v1.ClientCollateralManagementDataV1` (Namespace: `org.apache.fineract.avro.client.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCollateralManagementDataV1 {
    pub quantity: Option<rust_decimal::Decimal>,
    pub id: Option<i64>,
    #[serde(rename = "pctToBase")]
    pub pct_to_base: Option<rust_decimal::Decimal>,
    #[serde(rename = "unitPrice")]
    pub unit_price: Option<rust_decimal::Decimal>,
    pub total: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCollateral")]
    pub total_collateral: Option<rust_decimal::Decimal>,
    pub name: Option<String>,
}

pub const CLIENT_COLLATERAL_MANAGEMENT_DATA_V1_AVSC: &str = r#"{"name":"ClientCollateralManagementDataV1","namespace":"org.apache.fineract.avro.client.v1","type":"record","fields":[{"default":null,"name":"quantity","type":["null","bigdecimal"]},{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"pctToBase","type":["null","bigdecimal"]},{"default":null,"name":"unitPrice","type":["null","bigdecimal"]},{"default":null,"name":"total","type":["null","bigdecimal"]},{"default":null,"name":"totalCollateral","type":["null","bigdecimal"]},{"default":null,"name":"name","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.client.v1.ClientDataV1` (Namespace: `org.apache.fineract.avro.client.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    pub status: Option<EnumOptionDataV1>,
    #[serde(rename = "subStatus")]
    pub sub_status: Option<CodeValueDataV1>,
    pub active: Option<bool>,
    #[serde(rename = "activationDate")]
    pub activation_date: Option<String>,
    pub firstname: Option<String>,
    pub middlename: Option<String>,
    pub lastname: Option<String>,
    pub fullname: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "mobileNo")]
    pub mobile_no: Option<String>,
    #[serde(rename = "emailAddress")]
    pub email_address: Option<String>,
    #[serde(rename = "dateOfBirth")]
    pub date_of_birth: Option<String>,
    pub gender: Option<CodeValueDataV1>,
    #[serde(rename = "clientType")]
    pub client_type: Option<CodeValueDataV1>,
    #[serde(rename = "clientClassification")]
    pub client_classification: Option<CodeValueDataV1>,
    #[serde(rename = "isStaff")]
    pub is_staff: Option<bool>,
    #[serde(rename = "officeId")]
    pub office_id: Option<i64>,
    #[serde(rename = "officeName")]
    pub office_name: Option<String>,
    #[serde(rename = "transferToOfficeId")]
    pub transfer_to_office_id: Option<i64>,
    #[serde(rename = "transferToOfficeName")]
    pub transfer_to_office_name: Option<String>,
    #[serde(rename = "imageId")]
    pub image_id: Option<i64>,
    #[serde(rename = "imagePresent")]
    pub image_present: Option<bool>,
    #[serde(rename = "staffId")]
    pub staff_id: Option<i64>,
    #[serde(rename = "staffName")]
    pub staff_name: Option<String>,
    pub timeline: Option<ClientTimelineDataV1>,
    #[serde(rename = "savingsProductId")]
    pub savings_product_id: Option<i64>,
    #[serde(rename = "savingsProductName")]
    pub savings_product_name: Option<String>,
    #[serde(rename = "savingsAccountId")]
    pub savings_account_id: Option<i64>,
    #[serde(rename = "legalForm")]
    pub legal_form: Option<EnumOptionDataV1>,
    #[serde(rename = "clientCollateralManagements")]
    pub client_collateral_managements: Option<Vec<ClientCollateralManagementDataV1>>,
    pub groups: Option<Vec<GroupGeneralDataV1>>,
}

pub const CLIENT_DATA_V1_AVSC: &str = r#"{"name":"ClientDataV1","namespace":"org.apache.fineract.avro.client.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"subStatus","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"active","type":["null","boolean"]},{"default":null,"name":"activationDate","type":["null","string"]},{"default":null,"name":"firstname","type":["null","string"]},{"default":null,"name":"middlename","type":["null","string"]},{"default":null,"name":"lastname","type":["null","string"]},{"default":null,"name":"fullname","type":["null","string"]},{"default":null,"name":"displayName","type":["null","string"]},{"default":null,"name":"mobileNo","type":["null","string"]},{"default":null,"name":"emailAddress","type":["null","string"]},{"default":null,"name":"dateOfBirth","type":["null","string"]},{"default":null,"name":"gender","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"clientType","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"clientClassification","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"isStaff","type":["null","boolean"]},{"default":null,"name":"officeId","type":["null","long"]},{"default":null,"name":"officeName","type":["null","string"]},{"default":null,"name":"transferToOfficeId","type":["null","long"]},{"default":null,"name":"transferToOfficeName","type":["null","string"]},{"default":null,"name":"imageId","type":["null","long"]},{"default":null,"name":"imagePresent","type":["null","boolean"]},{"default":null,"name":"staffId","type":["null","long"]},{"default":null,"name":"staffName","type":["null","string"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.client.v1.ClientTimelineDataV1"]},{"default":null,"name":"savingsProductId","type":["null","long"]},{"default":null,"name":"savingsProductName","type":["null","string"]},{"default":null,"name":"savingsAccountId","type":["null","long"]},{"default":null,"name":"legalForm","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"clientCollateralManagements","type":["null",{"type":"array","items":"org.apache.fineract.avro.client.v1.ClientCollateralManagementDataV1"}]},{"default":null,"name":"groups","type":["null",{"type":"array","items":"org.apache.fineract.avro.group.v1.GroupGeneralDataV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.client.v1.ClientTimelineDataV1` (Namespace: `org.apache.fineract.avro.client.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTimelineDataV1 {
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    #[serde(rename = "submittedByUsername")]
    pub submitted_by_username: Option<String>,
    #[serde(rename = "submittedByFirstname")]
    pub submitted_by_firstname: Option<String>,
    #[serde(rename = "submittedByLastname")]
    pub submitted_by_lastname: Option<String>,
    #[serde(rename = "activatedOnDate")]
    pub activated_on_date: Option<String>,
    #[serde(rename = "activatedByUsername")]
    pub activated_by_username: Option<String>,
    #[serde(rename = "activatedByFirstname")]
    pub activated_by_firstname: Option<String>,
    #[serde(rename = "activatedByLastname")]
    pub activated_by_lastname: Option<String>,
    #[serde(rename = "closedOnDate")]
    pub closed_on_date: Option<String>,
    #[serde(rename = "closedByUsername")]
    pub closed_by_username: Option<String>,
    #[serde(rename = "closedByFirstname")]
    pub closed_by_firstname: Option<String>,
    #[serde(rename = "closedByLastname")]
    pub closed_by_lastname: Option<String>,
}

pub const CLIENT_TIMELINE_DATA_V1_AVSC: &str = r#"{"name":"ClientTimelineDataV1","namespace":"org.apache.fineract.avro.client.v1","type":"record","fields":[{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"submittedByUsername","type":["null","string"]},{"default":null,"name":"submittedByFirstname","type":["null","string"]},{"default":null,"name":"submittedByLastname","type":["null","string"]},{"default":null,"name":"activatedOnDate","type":["null","string"]},{"default":null,"name":"activatedByUsername","type":["null","string"]},{"default":null,"name":"activatedByFirstname","type":["null","string"]},{"default":null,"name":"activatedByLastname","type":["null","string"]},{"default":null,"name":"closedOnDate","type":["null","string"]},{"default":null,"name":"closedByUsername","type":["null","string"]},{"default":null,"name":"closedByFirstname","type":["null","string"]},{"default":null,"name":"closedByLastname","type":["null","string"]}]}"#;

/// Metadata emitted by DocumentCreatedBusinessEvent when a file is stored in Fineract. (Namespace: `org.apache.fineract.avro.document.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "parentEntityType")]
    pub parent_entity_type: Option<String>,
    #[serde(rename = "parentEntityId")]
    pub parent_entity_id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "fileName")]
    pub file_name: Option<String>,
    pub size: Option<i64>,
    pub r#type: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "storageType")]
    pub storage_type: Option<i32>,
}

pub const DOCUMENT_DATA_V1_AVSC: &str = r#"{"name":"DocumentDataV1","namespace":"org.apache.fineract.avro.document.v1","doc":"Metadata emitted by DocumentCreatedBusinessEvent when a file is stored in Fineract.","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"parentEntityType","type":["null","string"]},{"default":null,"name":"parentEntityId","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"fileName","type":["null","string"]},{"default":null,"name":"size","type":["null","long"]},{"default":null,"name":"type","type":["null","string"]},{"default":null,"name":"description","type":["null","string"]},{"default":null,"name":"storageType","type":["null","int"]}]}"#;

/// Avro schema `org.apache.fineract.avro.fixeddeposit.v1.FixedDepositAccountDataV1` (Namespace: `org.apache.fineract.avro.fixeddeposit.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixedDepositAccountDataV1 {
    pub id: Option<i32>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "groupId")]
    pub group_id: Option<i32>,
    #[serde(rename = "groupName")]
    pub group_name: Option<String>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i32>,
    #[serde(rename = "clientName")]
    pub client_name: Option<String>,
    #[serde(rename = "depositProductId")]
    pub deposit_product_id: Option<i32>,
    #[serde(rename = "depositProductName")]
    pub deposit_product_name: Option<String>,
    #[serde(rename = "fieldOfficerId")]
    pub field_officer_id: Option<i32>,
    #[serde(rename = "fieldOfficerName")]
    pub field_officer_name: Option<String>,
    pub status: Option<SavingsAccountStatusEnumDataV1>,
    pub timeline: Option<SavingsAccountApplicationTimelineDataV1>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "nominalAnnualInterestRate")]
    pub nominal_annual_interest_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestCompoundingPeriodType")]
    pub interest_compounding_period_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestPostingPeriodType")]
    pub interest_posting_period_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestCalculationType")]
    pub interest_calculation_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestCalculationDaysInYearType")]
    pub interest_calculation_days_in_year_type: Option<EnumOptionDataV1>,
    #[serde(rename = "minRequiredOpeningBalance")]
    pub min_required_opening_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "lockinPeriodFrequency")]
    pub lockin_period_frequency: Option<i32>,
    #[serde(rename = "lockinPeriodFrequencyType")]
    pub lockin_period_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "withdrawalFeeForTransfers")]
    pub withdrawal_fee_for_transfers: Option<bool>,
    #[serde(rename = "depositType")]
    pub deposit_type: Option<EnumOptionDataV1>,
    #[serde(rename = "minBalanceForInterestCalculation")]
    pub min_balance_for_interest_calculation: Option<rust_decimal::Decimal>,
    #[serde(rename = "withHoldTax")]
    pub with_hold_tax: Option<bool>,
    pub summary: Option<SavingsAccountSummaryDataV1>,
    pub transactions: Option<Vec<SavingsAccountTransactionDataV1>>,
    pub charges: Option<Vec<SavingsAccountChargeDataV1>>,
    #[serde(rename = "preClosurePenalApplicable")]
    pub pre_closure_penal_applicable: Option<bool>,
    #[serde(rename = "preClosurePenalInterest")]
    pub pre_closure_penal_interest: Option<rust_decimal::Decimal>,
    #[serde(rename = "preClosurePenalInterestOnType")]
    pub pre_closure_penal_interest_on_type: Option<EnumOptionDataV1>,
    #[serde(rename = "minDepositTerm")]
    pub min_deposit_term: Option<i32>,
    #[serde(rename = "maxDepositTerm")]
    pub max_deposit_term: Option<i32>,
    #[serde(rename = "minDepositTermType")]
    pub min_deposit_term_type: Option<EnumOptionDataV1>,
    #[serde(rename = "maxDepositTermType")]
    pub max_deposit_term_type: Option<EnumOptionDataV1>,
    #[serde(rename = "inMultiplesOfDepositTerm")]
    pub in_multiples_of_deposit_term: Option<i32>,
    #[serde(rename = "inMultiplesOfDepositTermType")]
    pub in_multiples_of_deposit_term_type: Option<EnumOptionDataV1>,
    #[serde(rename = "depositAmount")]
    pub deposit_amount: Option<f32>,
    #[serde(rename = "maturityAmount")]
    pub maturity_amount: Option<f32>,
    #[serde(rename = "maturityDate")]
    pub maturity_date: Option<String>,
    #[serde(rename = "depositPeriod")]
    pub deposit_period: Option<i32>,
    #[serde(rename = "depositPeriodFrequency")]
    pub deposit_period_frequency: Option<EnumOptionDataV1>,
    #[serde(rename = "activationCharge")]
    pub activation_charge: Option<rust_decimal::Decimal>,
    #[serde(rename = "transferToSavingsId")]
    pub transfer_to_savings_id: Option<i64>,
    #[serde(rename = "onAccountClosure")]
    pub on_account_closure: Option<EnumOptionDataV1>,
    #[serde(rename = "linkedAccount")]
    pub linked_account: Option<PortfolioAccountDataV1>,
    #[serde(rename = "transferInterestToSavings")]
    pub transfer_interest_to_savings: Option<bool>,
    #[serde(rename = "transferToSavingsAccount")]
    pub transfer_to_savings_account: Option<PortfolioAccountDataV1>,
}

pub const FIXED_DEPOSIT_ACCOUNT_DATA_V1_AVSC: &str = r#"{"name":"FixedDepositAccountDataV1","namespace":"org.apache.fineract.avro.fixeddeposit.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"groupId","type":["null","int"]},{"default":null,"name":"groupName","type":["null","string"]},{"default":null,"name":"clientId","type":["null","int"]},{"default":null,"name":"clientName","type":["null","string"]},{"default":null,"name":"depositProductId","type":["null","int"]},{"default":null,"name":"depositProductName","type":["null","string"]},{"default":null,"name":"fieldOfficerId","type":["null","int"]},{"default":null,"name":"fieldOfficerName","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountStatusEnumDataV1"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountApplicationTimelineDataV1"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"nominalAnnualInterestRate","type":["null","bigdecimal"]},{"default":null,"name":"interestCompoundingPeriodType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestPostingPeriodType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestCalculationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestCalculationDaysInYearType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minRequiredOpeningBalance","type":["null","bigdecimal"]},{"default":null,"name":"lockinPeriodFrequency","type":["null","int"]},{"default":null,"name":"lockinPeriodFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"withdrawalFeeForTransfers","type":["null","boolean"]},{"default":null,"name":"depositType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minBalanceForInterestCalculation","type":["null","bigdecimal"]},{"default":null,"name":"withHoldTax","type":["null","boolean"]},{"default":null,"name":"summary","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountSummaryDataV1"]},{"default":null,"name":"transactions","type":["null",{"type":"array","items":"org.apache.fineract.avro.savings.v1.SavingsAccountTransactionDataV1"}]},{"default":null,"name":"charges","type":["null",{"type":"array","items":"org.apache.fineract.avro.savings.v1.SavingsAccountChargeDataV1"}]},{"default":null,"name":"preClosurePenalApplicable","type":["null","boolean"]},{"default":null,"name":"preClosurePenalInterest","type":["null","bigdecimal"]},{"default":null,"name":"preClosurePenalInterestOnType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minDepositTerm","type":["null","int"]},{"default":null,"name":"maxDepositTerm","type":["null","int"]},{"default":null,"name":"minDepositTermType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"maxDepositTermType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"inMultiplesOfDepositTerm","type":["null","int"]},{"default":null,"name":"inMultiplesOfDepositTermType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"depositAmount","type":["null","float"]},{"default":null,"name":"maturityAmount","type":["null","float"]},{"default":null,"name":"maturityDate","type":["null","string"]},{"default":null,"name":"depositPeriod","type":["null","int"]},{"default":null,"name":"depositPeriodFrequency","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"activationCharge","type":["null","bigdecimal"]},{"default":null,"name":"transferToSavingsId","type":["null","long"]},{"default":null,"name":"onAccountClosure","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"linkedAccount","type":["null","org.apache.fineract.avro.portfolio.v1.PortfolioAccountDataV1"]},{"default":null,"name":"transferInterestToSavings","type":["null","boolean"]},{"default":null,"name":"transferToSavingsAccount","type":["null","org.apache.fineract.avro.portfolio.v1.PortfolioAccountDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.generic.v1.CalendarDataV1` (Namespace: `org.apache.fineract.avro.generic.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "calendarInstanceId")]
    pub calendar_instance_id: Option<i64>,
    #[serde(rename = "entityId")]
    pub entity_id: Option<i64>,
    #[serde(rename = "entityType")]
    pub entity_type: Option<EnumOptionDataV1>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    #[serde(rename = "meetingTime")]
    pub meeting_time: Option<String>,
    pub duration: Option<i32>,
    pub r#type: Option<EnumOptionDataV1>,
    pub repeating: Option<bool>,
    pub recurrence: Option<String>,
    pub frequency: Option<EnumOptionDataV1>,
    pub interval: Option<i32>,
    #[serde(rename = "repeatsOnDay")]
    pub repeats_on_day: Option<EnumOptionDataV1>,
    #[serde(rename = "repeatsOnNthDayOfMonth")]
    pub repeats_on_nth_day_of_month: Option<EnumOptionDataV1>,
    #[serde(rename = "remindBy")]
    pub remind_by: Option<EnumOptionDataV1>,
    #[serde(rename = "firstReminder")]
    pub first_reminder: Option<i32>,
    #[serde(rename = "secondReminder")]
    pub second_reminder: Option<i32>,
    #[serde(rename = "recurringDates")]
    pub recurring_dates: Option<Vec<String>>,
    #[serde(rename = "nextTenRecurringDates")]
    pub next_ten_recurring_dates: Option<Vec<String>>,
    #[serde(rename = "humanReadable")]
    pub human_readable: Option<String>,
    #[serde(rename = "recentEligibleMeetingDate")]
    pub recent_eligible_meeting_date: Option<String>,
    #[serde(rename = "createdDate")]
    pub created_date: Option<String>,
    #[serde(rename = "lastUpdatedDate")]
    pub last_updated_date: Option<String>,
    #[serde(rename = "createdByUserId")]
    pub created_by_user_id: Option<i64>,
    #[serde(rename = "createdByUsername")]
    pub created_by_username: Option<String>,
    #[serde(rename = "lastUpdatedByUserId")]
    pub last_updated_by_user_id: Option<i64>,
    #[serde(rename = "lastUpdatedByUsername")]
    pub last_updated_by_username: Option<String>,
    #[serde(rename = "repeatsOnDayOfMonth")]
    pub repeats_on_day_of_month: Option<i32>,
}

pub const CALENDAR_DATA_V1_AVSC: &str = r#"{"name":"CalendarDataV1","namespace":"org.apache.fineract.avro.generic.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"calendarInstanceId","type":["null","long"]},{"default":null,"name":"entityId","type":["null","long"]},{"default":null,"name":"entityType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"title","type":["null","string"]},{"default":null,"name":"description","type":["null","string"]},{"default":null,"name":"location","type":["null","string"]},{"default":null,"name":"startDate","type":["null","string"]},{"default":null,"name":"endDate","type":["null","string"]},{"default":null,"name":"meetingTime","type":["null","string"]},{"default":null,"name":"duration","type":["null","int"]},{"default":null,"name":"type","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"repeating","type":["null","boolean"]},{"default":null,"name":"recurrence","type":["null","string"]},{"default":null,"name":"frequency","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interval","type":["null","int"]},{"default":null,"name":"repeatsOnDay","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"repeatsOnNthDayOfMonth","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"remindBy","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"firstReminder","type":["null","int"]},{"default":null,"name":"secondReminder","type":["null","int"]},{"default":null,"name":"recurringDates","type":["null",{"type":"array","items":"string"}]},{"default":null,"name":"nextTenRecurringDates","type":["null",{"type":"array","items":"string"}]},{"default":null,"name":"humanReadable","type":["null","string"]},{"default":null,"name":"recentEligibleMeetingDate","type":["null","string"]},{"default":null,"name":"createdDate","type":["null","string"]},{"default":null,"name":"lastUpdatedDate","type":["null","string"]},{"default":null,"name":"createdByUserId","type":["null","long"]},{"default":null,"name":"createdByUsername","type":["null","string"]},{"default":null,"name":"lastUpdatedByUserId","type":["null","long"]},{"default":null,"name":"lastUpdatedByUsername","type":["null","string"]},{"default":null,"name":"repeatsOnDayOfMonth","type":["null","int"]}]}"#;

/// Avro schema `org.apache.fineract.avro.generic.v1.CodeValueDataV1` (Namespace: `org.apache.fineract.avro.generic.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeValueDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub position: Option<i32>,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub mandatory: Option<bool>,
}

pub const CODE_VALUE_DATA_V1_AVSC: &str = r#"{"name":"CodeValueDataV1","namespace":"org.apache.fineract.avro.generic.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"position","type":["null","int"]},{"default":null,"name":"description","type":["null","string"]},{"default":null,"name":"active","type":["null","boolean"]},{"default":null,"name":"mandatory","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.generic.v1.CommandProcessingResultV1` (Namespace: `org.apache.fineract.avro.generic.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandProcessingResultV1 {
    #[serde(rename = "commandId")]
    pub command_id: Option<i64>,
    #[serde(rename = "officeId")]
    pub office_id: Option<i64>,
    #[serde(rename = "groupId")]
    pub group_id: Option<i64>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i64>,
    #[serde(rename = "loanId")]
    pub loan_id: Option<i64>,
    #[serde(rename = "savingsId")]
    pub savings_id: Option<i64>,
    #[serde(rename = "resourceId")]
    pub resource_id: Option<i64>,
    #[serde(rename = "subResourceId")]
    pub sub_resource_id: Option<i64>,
    #[serde(rename = "transactionId")]
    pub transaction_id: Option<i64>,
    #[serde(rename = "resourceIdentifier")]
    pub resource_identifier: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<i64>,
    #[serde(rename = "gsimId")]
    pub gsim_id: Option<i64>,
    #[serde(rename = "glimId")]
    pub glim_id: Option<i64>,
}

pub const COMMAND_PROCESSING_RESULT_V1_AVSC: &str = r#"{"name":"CommandProcessingResultV1","namespace":"org.apache.fineract.avro.generic.v1","type":"record","fields":[{"default":null,"name":"commandId","type":["null","long"]},{"default":null,"name":"officeId","type":["null","long"]},{"default":null,"name":"groupId","type":["null","long"]},{"default":null,"name":"clientId","type":["null","long"]},{"default":null,"name":"loanId","type":["null","long"]},{"default":null,"name":"savingsId","type":["null","long"]},{"default":null,"name":"resourceId","type":["null","long"]},{"default":null,"name":"subResourceId","type":["null","long"]},{"default":null,"name":"transactionId","type":["null","long"]},{"default":null,"name":"resourceIdentifier","type":["null","string"]},{"default":null,"name":"productId","type":["null","long"]},{"default":null,"name":"gsimId","type":["null","long"]},{"default":null,"name":"glimId","type":["null","long"]}]}"#;

/// Avro schema `org.apache.fineract.avro.generic.v1.CurrencyDataV1` (Namespace: `org.apache.fineract.avro.generic.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyDataV1 {
    pub code: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "decimalPlaces")]
    pub decimal_places: Option<i32>,
    #[serde(rename = "inMultiplesOf")]
    pub in_multiples_of: Option<i32>,
    #[serde(rename = "displaySymbol")]
    pub display_symbol: Option<String>,
    #[serde(rename = "nameCode")]
    pub name_code: Option<String>,
    #[serde(rename = "displayLabel")]
    pub display_label: Option<String>,
}

pub const CURRENCY_DATA_V1_AVSC: &str = r#"{"name":"CurrencyDataV1","namespace":"org.apache.fineract.avro.generic.v1","type":"record","fields":[{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"decimalPlaces","type":["null","int"]},{"default":null,"name":"inMultiplesOf","type":["null","int"]},{"default":null,"name":"displaySymbol","type":["null","string"]},{"default":null,"name":"nameCode","type":["null","string"]},{"default":null,"name":"displayLabel","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.generic.v1.EnumOptionDataV1` (Namespace: `org.apache.fineract.avro.generic.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumOptionDataV1 {
    pub id: Option<i32>,
    pub code: Option<String>,
    pub value: Option<String>,
}

pub const ENUM_OPTION_DATA_V1_AVSC: &str = r#"{"name":"EnumOptionDataV1","namespace":"org.apache.fineract.avro.generic.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"value","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1` (Namespace: `org.apache.fineract.avro.generic.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringEnumOptionDataV1 {
    pub id: Option<String>,
    pub code: Option<String>,
    pub value: Option<String>,
}

pub const STRING_ENUM_OPTION_DATA_V1_AVSC: &str = r#"{"name":"StringEnumOptionDataV1","namespace":"org.apache.fineract.avro.generic.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","string"]},{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"value","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.gl.v1.GLAccountDataV1` (Namespace: `org.apache.fineract.avro.gl.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GLAccountDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<i64>,
    #[serde(rename = "glCode")]
    pub gl_code: Option<String>,
    pub disabled: Option<bool>,
    #[serde(rename = "manualEntriesAllowed")]
    pub manual_entries_allowed: Option<bool>,
    pub r#type: Option<EnumOptionDataV1>,
    pub usage: Option<EnumOptionDataV1>,
    pub description: Option<String>,
    #[serde(rename = "nameDecorated")]
    pub name_decorated: Option<String>,
    #[serde(rename = "tagId")]
    pub tag_id: Option<CodeValueDataV1>,
    #[serde(rename = "organizationRunningBalance")]
    pub organization_running_balance: Option<i64>,
}

pub const GLACCOUNT_DATA_V1_AVSC: &str = r#"{"name":"GLAccountDataV1","namespace":"org.apache.fineract.avro.gl.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"parentId","type":["null","long"]},{"default":null,"name":"glCode","type":["null","string"]},{"default":null,"name":"disabled","type":["null","boolean"]},{"default":null,"name":"manualEntriesAllowed","type":["null","boolean"]},{"default":null,"name":"type","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"usage","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"description","type":["null","string"]},{"default":null,"name":"nameDecorated","type":["null","string"]},{"default":null,"name":"tagId","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"organizationRunningBalance","type":["null","long"]}]}"#;

/// Avro schema `org.apache.fineract.avro.group.v1.GroupGeneralDataV1` (Namespace: `org.apache.fineract.avro.group.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupGeneralDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    pub status: Option<EnumOptionDataV1>,
    pub active: Option<bool>,
    #[serde(rename = "activationDate")]
    pub activation_date: Option<String>,
    #[serde(rename = "officeId")]
    pub office_id: Option<i64>,
    #[serde(rename = "officeName")]
    pub office_name: Option<String>,
    #[serde(rename = "centerId")]
    pub center_id: Option<i64>,
    #[serde(rename = "centerName")]
    pub center_name: Option<String>,
    #[serde(rename = "staffId")]
    pub staff_id: Option<i64>,
    #[serde(rename = "staffName")]
    pub staff_name: Option<String>,
    pub hierarchy: Option<String>,
    #[serde(rename = "groupLevel")]
    pub group_level: Option<String>,
    #[serde(rename = "groupRoles")]
    pub group_roles: Option<Vec<GroupRoleDataV1>>,
    #[serde(rename = "calendarsData")]
    pub calendars_data: Option<Vec<CalendarDataV1>>,
    #[serde(rename = "collectionMeetingCalendar")]
    pub collection_meeting_calendar: Option<CalendarDataV1>,
}

pub const GROUP_GENERAL_DATA_V1_AVSC: &str = r#"{"name":"GroupGeneralDataV1","namespace":"org.apache.fineract.avro.group.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"active","type":["null","boolean"]},{"default":null,"name":"activationDate","type":["null","string"]},{"default":null,"name":"officeId","type":["null","long"]},{"default":null,"name":"officeName","type":["null","string"]},{"default":null,"name":"centerId","type":["null","long"]},{"default":null,"name":"centerName","type":["null","string"]},{"default":null,"name":"staffId","type":["null","long"]},{"default":null,"name":"staffName","type":["null","string"]},{"default":null,"name":"hierarchy","type":["null","string"]},{"default":null,"name":"groupLevel","type":["null","string"]},{"default":null,"name":"groupRoles","type":["null",{"type":"array","items":"org.apache.fineract.avro.group.v1.GroupRoleDataV1"}]},{"default":null,"name":"calendarsData","type":["null",{"type":"array","items":"org.apache.fineract.avro.generic.v1.CalendarDataV1"}]},{"default":null,"name":"collectionMeetingCalendar","type":["null","org.apache.fineract.avro.generic.v1.CalendarDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.group.v1.GroupRoleDataV1` (Namespace: `org.apache.fineract.avro.group.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRoleDataV1 {
    pub id: Option<i64>,
    pub role: Option<CodeValueDataV1>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i64>,
    #[serde(rename = "clientName")]
    pub client_name: Option<String>,
}

pub const GROUP_ROLE_DATA_V1_AVSC: &str = r#"{"name":"GroupRoleDataV1","namespace":"org.apache.fineract.avro.group.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"role","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"clientId","type":["null","long"]},{"default":null,"name":"clientName","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.CollectionDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDataV1 {
    #[serde(rename = "availableDisbursementAmount")]
    pub available_disbursement_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "availableDisbursementAmountWithOverApplied")]
    pub available_disbursement_amount_with_over_applied: Option<rust_decimal::Decimal>,
    #[serde(rename = "pastDueDays")]
    pub past_due_days: Option<i32>,
    #[serde(rename = "pastDueDate")]
    pub past_due_date: Option<String>,
    #[serde(rename = "nextPaymentDueDate")]
    pub next_payment_due_date: Option<String>,
    #[serde(rename = "nextPaymentAmount")]
    pub next_payment_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "delinquentDays")]
    pub delinquent_days: Option<i32>,
    #[serde(rename = "delinquentDate")]
    pub delinquent_date: Option<String>,
    #[serde(rename = "delinquentAmount")]
    pub delinquent_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "lastPaymentDate")]
    pub last_payment_date: Option<String>,
    #[serde(rename = "lastPaymentAmount")]
    pub last_payment_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "lastRepaymentDate")]
    pub last_repayment_date: Option<String>,
    #[serde(rename = "lastRepaymentAmount")]
    pub last_repayment_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "delinquencyPausePeriods")]
    pub delinquency_pause_periods: Option<Vec<DelinquencyPausePeriodV1>>,
    #[serde(rename = "installmentDelinquencyBuckets")]
    pub installment_delinquency_buckets: Option<Vec<LoanInstallmentDelinquencyBucketDataV1>>,
    #[serde(rename = "delinquentPrincipal")]
    pub delinquent_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "delinquentInterest")]
    pub delinquent_interest: Option<rust_decimal::Decimal>,
    #[serde(rename = "delinquentFee")]
    pub delinquent_fee: Option<rust_decimal::Decimal>,
    #[serde(rename = "delinquentPenalty")]
    pub delinquent_penalty: Option<rust_decimal::Decimal>,
}

pub const COLLECTION_DATA_V1_AVSC: &str = r#"{"name":"CollectionDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"availableDisbursementAmount","type":["null","bigdecimal"]},{"default":null,"name":"availableDisbursementAmountWithOverApplied","type":["null","bigdecimal"]},{"default":null,"name":"pastDueDays","type":["null","int"]},{"default":null,"name":"pastDueDate","type":["null","string"]},{"default":null,"name":"nextPaymentDueDate","type":["null","string"]},{"default":null,"name":"nextPaymentAmount","type":["null","bigdecimal"]},{"default":null,"name":"delinquentDays","type":["null","int"]},{"default":null,"name":"delinquentDate","type":["null","string"]},{"default":null,"name":"delinquentAmount","type":["null","bigdecimal"]},{"default":null,"name":"lastPaymentDate","type":["null","string"]},{"default":null,"name":"lastPaymentAmount","type":["null","bigdecimal"]},{"default":null,"name":"lastRepaymentDate","type":["null","string"]},{"default":null,"name":"lastRepaymentAmount","type":["null","bigdecimal"]},{"default":null,"name":"delinquencyPausePeriods","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.DelinquencyPausePeriodV1"}]},{"default":null,"name":"installmentDelinquencyBuckets","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanInstallmentDelinquencyBucketDataV1"}]},{"default":null,"name":"delinquentPrincipal","type":["null","bigdecimal"]},{"default":null,"name":"delinquentInterest","type":["null","bigdecimal"]},{"default":null,"name":"delinquentFee","type":["null","bigdecimal"]},{"default":null,"name":"delinquentPenalty","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.DelinquencyBucketDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelinquencyBucketDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub ranges: Option<Vec<DelinquencyRangeDataV1>>,
}

pub const DELINQUENCY_BUCKET_DATA_V1_AVSC: &str = r#"{"name":"DelinquencyBucketDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"ranges","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.DelinquencyRangeDataV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.DelinquencyPausePeriodV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelinquencyPausePeriodV1 {
    pub active: Option<bool>,
    #[serde(rename = "pausePeriodStart")]
    pub pause_period_start: Option<String>,
    #[serde(rename = "pausePeriodEnd")]
    pub pause_period_end: Option<String>,
}

pub const DELINQUENCY_PAUSE_PERIOD_V1_AVSC: &str = r#"{"name":"DelinquencyPausePeriodV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"active","type":["null","boolean"]},{"default":null,"name":"pausePeriodStart","type":["null","string"]},{"default":null,"name":"pausePeriodEnd","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.DelinquencyRangeDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelinquencyRangeDataV1 {
    pub id: Option<i64>,
    pub classification: Option<String>,
    #[serde(rename = "minimumAgeDays")]
    pub minimum_age_days: Option<i32>,
    #[serde(rename = "maximumAgeDays")]
    pub maximum_age_days: Option<i32>,
}

pub const DELINQUENCY_RANGE_DATA_V1_AVSC: &str = r#"{"name":"DelinquencyRangeDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"classification","type":["null","string"]},{"default":null,"name":"minimumAgeDays","type":["null","int"]},{"default":null,"name":"maximumAgeDays","type":["null","int"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.DisbursementDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisbursementDataV1 {
    pub id: Option<i32>,
    #[serde(rename = "expectedDisbursementDate")]
    pub expected_disbursement_date: Option<String>,
    #[serde(rename = "actualDisbursementDate")]
    pub actual_disbursement_date: Option<String>,
    pub principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "netDisbursalAmount")]
    pub net_disbursal_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "loanChargeId")]
    pub loan_charge_id: Option<String>,
    #[serde(rename = "chargeAmount")]
    pub charge_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "waivedChargeAmount")]
    pub waived_charge_amount: Option<rust_decimal::Decimal>,
}

pub const DISBURSEMENT_DATA_V1_AVSC: &str = r#"{"name":"DisbursementDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"expectedDisbursementDate","type":["null","string"]},{"default":null,"name":"actualDisbursementDate","type":["null","string"]},{"default":null,"name":"principal","type":["null","bigdecimal"]},{"default":null,"name":"netDisbursalAmount","type":["null","bigdecimal"]},{"default":null,"name":"loanChargeId","type":["null","string"]},{"default":null,"name":"chargeAmount","type":["null","bigdecimal"]},{"default":null,"name":"waivedChargeAmount","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanAccountDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAccountDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "externalOwnerId")]
    pub external_owner_id: Option<String>,
    #[serde(rename = "settlementDate")]
    pub settlement_date: Option<String>,
    #[serde(rename = "purchasePriceRatio")]
    pub purchase_price_ratio: Option<String>,
    pub status: Option<LoanStatusEnumDataV1>,
    #[serde(rename = "subStatus")]
    pub sub_status: Option<EnumOptionDataV1>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i64>,
    #[serde(rename = "clientAccountNo")]
    pub client_account_no: Option<String>,
    #[serde(rename = "clientName")]
    pub client_name: Option<String>,
    #[serde(rename = "clientOfficeId")]
    pub client_office_id: Option<i64>,
    #[serde(rename = "clientExternalId")]
    pub client_external_id: Option<String>,
    #[serde(rename = "loanProductId")]
    pub loan_product_id: Option<i64>,
    #[serde(rename = "loanProductName")]
    pub loan_product_name: Option<String>,
    #[serde(rename = "loanProductDescription")]
    pub loan_product_description: Option<String>,
    #[serde(rename = "isLoanProductLinkedToFloatingRate")]
    pub is_loan_product_linked_to_floating_rate: Option<bool>,
    #[serde(rename = "fundId")]
    pub fund_id: Option<i64>,
    #[serde(rename = "fundName")]
    pub fund_name: Option<String>,
    #[serde(rename = "loanPurposeId")]
    pub loan_purpose_id: Option<i64>,
    #[serde(rename = "loanPurposeName")]
    pub loan_purpose_name: Option<String>,
    #[serde(rename = "loanOfficerId")]
    pub loan_officer_id: Option<i64>,
    #[serde(rename = "loanOfficerName")]
    pub loan_officer_name: Option<String>,
    #[serde(rename = "loanType")]
    pub loan_type: Option<EnumOptionDataV1>,
    pub principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "approvedPrincipal")]
    pub approved_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "proposedPrincipal")]
    pub proposed_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "netDisbursalAmount")]
    pub net_disbursal_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "termFrequency")]
    pub term_frequency: Option<i32>,
    #[serde(rename = "termPeriodFrequencyType")]
    pub term_period_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "numberOfRepayments")]
    pub number_of_repayments: Option<i32>,
    #[serde(rename = "repaymentEvery")]
    pub repayment_every: Option<i32>,
    #[serde(rename = "repaymentFrequencyType")]
    pub repayment_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "repaymentFrequencyNthDayType")]
    pub repayment_frequency_nth_day_type: Option<EnumOptionDataV1>,
    #[serde(rename = "repaymentFrequencyDayOfWeekType")]
    pub repayment_frequency_day_of_week_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestRatePerPeriod")]
    pub interest_rate_per_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestRateFrequencyType")]
    pub interest_rate_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "annualInterestRate")]
    pub annual_interest_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "isFloatingInterestRate")]
    pub is_floating_interest_rate: Option<bool>,
    #[serde(rename = "interestRateDifferential")]
    pub interest_rate_differential: Option<rust_decimal::Decimal>,
    #[serde(rename = "amortizationType")]
    pub amortization_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestType")]
    pub interest_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestCalculationPeriodType")]
    pub interest_calculation_period_type: Option<EnumOptionDataV1>,
    #[serde(rename = "allowPartialPeriodInterestCalculation")]
    pub allow_partial_period_interest_calculation: Option<bool>,
    #[serde(rename = "inArrearsTolerance")]
    pub in_arrears_tolerance: Option<rust_decimal::Decimal>,
    #[serde(rename = "transactionProcessingStrategyCode")]
    pub transaction_processing_strategy_code: Option<String>,
    #[serde(rename = "transactionProcessingStrategyName")]
    pub transaction_processing_strategy_name: Option<String>,
    #[serde(rename = "graceOnPrincipalPayment")]
    pub grace_on_principal_payment: Option<i32>,
    #[serde(rename = "recurringMoratoriumOnPrincipalPeriods")]
    pub recurring_moratorium_on_principal_periods: Option<i32>,
    #[serde(rename = "graceOnInterestPayment")]
    pub grace_on_interest_payment: Option<i32>,
    #[serde(rename = "graceOnInterestCharged")]
    pub grace_on_interest_charged: Option<i32>,
    #[serde(rename = "graceOnArrearsAgeing")]
    pub grace_on_arrears_ageing: Option<i32>,
    #[serde(rename = "interestChargedFromDate")]
    pub interest_charged_from_date: Option<String>,
    #[serde(rename = "expectedFirstRepaymentOnDate")]
    pub expected_first_repayment_on_date: Option<String>,
    #[serde(rename = "syncDisbursementWithMeeting")]
    pub sync_disbursement_with_meeting: Option<bool>,
    pub timeline: Option<LoanApplicationTimelineDataV1>,
    pub summary: Option<LoanSummaryDataV1>,
    #[serde(rename = "repaymentSchedule")]
    pub repayment_schedule: Option<LoanScheduleDataV1>,
    pub transactions: Option<Vec<LoanTransactionDataV1>>,
    pub charges: Option<Vec<LoanChargeDataV1>>,
    pub meeting: Option<CalendarDataV1>,
    #[serde(rename = "disbursementDetails")]
    pub disbursement_details: Option<Vec<DisbursementDataV1>>,
    #[serde(rename = "originalSchedule")]
    pub original_schedule: Option<LoanScheduleDataV1>,
    #[serde(rename = "totalOverpaid")]
    pub total_overpaid: Option<rust_decimal::Decimal>,
    #[serde(rename = "loanCounter")]
    pub loan_counter: Option<i32>,
    #[serde(rename = "loanProductCounter")]
    pub loan_product_counter: Option<i32>,
    #[serde(rename = "linkedAccount")]
    pub linked_account: Option<PortfolioAccountDataV1>,
    #[serde(rename = "multiDisburseLoan")]
    pub multi_disburse_loan: Option<bool>,
    #[serde(rename = "disallowExpectedDisbursements")]
    pub disallow_expected_disbursements: Option<bool>,
    #[serde(rename = "canDefineInstallmentAmount")]
    pub can_define_installment_amount: Option<bool>,
    #[serde(rename = "fixedEmiAmount")]
    pub fixed_emi_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "maxOutstandingLoanBalance")]
    pub max_outstanding_loan_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "canDisburse")]
    pub can_disburse: Option<bool>,
    #[serde(rename = "emiAmountVariations")]
    pub emi_amount_variations: Option<Vec<LoanTermVariationsDataV1>>,
    #[serde(rename = "loanTermVariations")]
    pub loan_term_variations: Option<Vec<LoanTermVariationsDataV1>>,
    #[serde(rename = "clientActiveLoanOptions")]
    pub client_active_loan_options: Option<Vec<LoanAccountSummaryDataV1>>,
    #[serde(rename = "canUseForTopup")]
    pub can_use_for_topup: Option<bool>,
    #[serde(rename = "isTopup")]
    pub is_topup: Option<bool>,
    #[serde(rename = "closureLoanId")]
    pub closure_loan_id: Option<i64>,
    #[serde(rename = "closureLoanAccountNo")]
    pub closure_loan_account_no: Option<String>,
    #[serde(rename = "topupAmount")]
    pub topup_amount: Option<rust_decimal::Decimal>,
    pub product: Option<LoanProductDataV1>,
    #[serde(rename = "inArrears")]
    pub in_arrears: Option<bool>,
    #[serde(rename = "isNPA")]
    pub is_npa: Option<bool>,
    #[serde(rename = "overdueCharges")]
    pub overdue_charges: Option<Vec<ChargeDataV1>>,
    #[serde(rename = "daysInMonthType")]
    pub days_in_month_type: Option<EnumOptionDataV1>,
    #[serde(rename = "daysInYearType")]
    pub days_in_year_type: Option<EnumOptionDataV1>,
    #[serde(rename = "daysInYearCustomStrategy")]
    pub days_in_year_custom_strategy: Option<StringEnumOptionDataV1>,
    #[serde(rename = "monthEndDueDateStrategy")]
    pub month_end_due_date_strategy: Option<StringEnumOptionDataV1>,
    #[serde(rename = "isInterestRecalculationEnabled")]
    pub is_interest_recalculation_enabled: Option<bool>,
    #[serde(rename = "interestRecalculationData")]
    pub interest_recalculation_data: Option<LoanInterestRecalculationDataV1>,
    #[serde(rename = "createStandingInstructionAtDisbursement")]
    pub create_standing_instruction_at_disbursement: Option<bool>,
    pub delinquent: Option<CollectionDataV1>,
    #[serde(rename = "delinquencyRange")]
    pub delinquency_range: Option<DelinquencyRangeDataV1>,
    #[serde(rename = "overpaidOnDate")]
    pub overpaid_on_date: Option<String>,
    #[serde(rename = "lastClosedBusinessDate")]
    pub last_closed_business_date: Option<String>,
    #[serde(rename = "chargedOff")]
    pub charged_off: Option<bool>,
    #[serde(rename = "enableInstallmentLevelDelinquency")]
    pub enable_installment_level_delinquency: Option<bool>,
    #[serde(rename = "enableDownPayment")]
    pub enable_down_payment: Option<bool>,
    #[serde(rename = "disbursedAmountPercentageForDownPayment")]
    pub disbursed_amount_percentage_for_down_payment: Option<rust_decimal::Decimal>,
    #[serde(rename = "enableAutoRepaymentForDownPayment")]
    pub enable_auto_repayment_for_down_payment: Option<bool>,
    #[serde(rename = "repaymentStartDateType")]
    pub repayment_start_date_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestRecognitionOnDisbursementDate")]
    pub interest_recognition_on_disbursement_date: Option<bool>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
    pub originators: Option<Vec<OriginatorDetailsV1>>,
    #[serde(rename = "actualNoTerm")]
    pub actual_no_term: Option<i32>,
}

pub const LOAN_ACCOUNT_DATA_V1_AVSC: &str = r#"{"name":"LoanAccountDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"externalOwnerId","type":["null","string"]},{"default":null,"name":"settlementDate","type":["null","string"]},{"default":null,"name":"purchasePriceRatio","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.loan.v1.LoanStatusEnumDataV1"]},{"default":null,"name":"subStatus","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"clientId","type":["null","long"]},{"default":null,"name":"clientAccountNo","type":["null","string"]},{"default":null,"name":"clientName","type":["null","string"]},{"default":null,"name":"clientOfficeId","type":["null","long"]},{"default":null,"name":"clientExternalId","type":["null","string"]},{"default":null,"name":"loanProductId","type":["null","long"]},{"default":null,"name":"loanProductName","type":["null","string"]},{"default":null,"name":"loanProductDescription","type":["null","string"]},{"default":null,"name":"isLoanProductLinkedToFloatingRate","type":["null","boolean"]},{"default":null,"name":"fundId","type":["null","long"]},{"default":null,"name":"fundName","type":["null","string"]},{"default":null,"name":"loanPurposeId","type":["null","long"]},{"default":null,"name":"loanPurposeName","type":["null","string"]},{"default":null,"name":"loanOfficerId","type":["null","long"]},{"default":null,"name":"loanOfficerName","type":["null","string"]},{"default":null,"name":"loanType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"principal","type":["null","bigdecimal"]},{"default":null,"name":"approvedPrincipal","type":["null","bigdecimal"]},{"default":null,"name":"proposedPrincipal","type":["null","bigdecimal"]},{"default":null,"name":"netDisbursalAmount","type":["null","bigdecimal"]},{"default":null,"name":"termFrequency","type":["null","int"]},{"default":null,"name":"termPeriodFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"numberOfRepayments","type":["null","int"]},{"default":null,"name":"repaymentEvery","type":["null","int"]},{"default":null,"name":"repaymentFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"repaymentFrequencyNthDayType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"repaymentFrequencyDayOfWeekType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestRatePerPeriod","type":["null","bigdecimal"]},{"default":null,"name":"interestRateFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"annualInterestRate","type":["null","bigdecimal"]},{"default":null,"name":"isFloatingInterestRate","type":["null","boolean"]},{"default":null,"name":"interestRateDifferential","type":["null","bigdecimal"]},{"default":null,"name":"amortizationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestCalculationPeriodType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"allowPartialPeriodInterestCalculation","type":["null","boolean"]},{"default":null,"name":"inArrearsTolerance","type":["null","bigdecimal"]},{"default":null,"name":"transactionProcessingStrategyCode","type":["null","string"]},{"default":null,"name":"transactionProcessingStrategyName","type":["null","string"]},{"default":null,"name":"graceOnPrincipalPayment","type":["null","int"]},{"default":null,"name":"recurringMoratoriumOnPrincipalPeriods","type":["null","int"]},{"default":null,"name":"graceOnInterestPayment","type":["null","int"]},{"default":null,"name":"graceOnInterestCharged","type":["null","int"]},{"default":null,"name":"graceOnArrearsAgeing","type":["null","int"]},{"default":null,"name":"interestChargedFromDate","type":["null","string"]},{"default":null,"name":"expectedFirstRepaymentOnDate","type":["null","string"]},{"default":null,"name":"syncDisbursementWithMeeting","type":["null","boolean"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.loan.v1.LoanApplicationTimelineDataV1"]},{"default":null,"name":"summary","type":["null","org.apache.fineract.avro.loan.v1.LoanSummaryDataV1"]},{"default":null,"name":"repaymentSchedule","type":["null","org.apache.fineract.avro.loan.v1.LoanScheduleDataV1"]},{"default":null,"name":"transactions","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanTransactionDataV1"}]},{"default":null,"name":"charges","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanChargeDataV1"}]},{"default":null,"name":"meeting","type":["null","org.apache.fineract.avro.generic.v1.CalendarDataV1"]},{"default":null,"name":"disbursementDetails","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.DisbursementDataV1"}]},{"default":null,"name":"originalSchedule","type":["null","org.apache.fineract.avro.loan.v1.LoanScheduleDataV1"]},{"default":null,"name":"totalOverpaid","type":["null","bigdecimal"]},{"default":null,"name":"loanCounter","type":["null","int"]},{"default":null,"name":"loanProductCounter","type":["null","int"]},{"default":null,"name":"linkedAccount","type":["null","org.apache.fineract.avro.portfolio.v1.PortfolioAccountDataV1"]},{"default":null,"name":"multiDisburseLoan","type":["null","boolean"]},{"default":null,"name":"disallowExpectedDisbursements","type":["null","boolean"]},{"default":null,"name":"canDefineInstallmentAmount","type":["null","boolean"]},{"default":null,"name":"fixedEmiAmount","type":["null","bigdecimal"]},{"default":null,"name":"maxOutstandingLoanBalance","type":["null","bigdecimal"]},{"default":null,"name":"canDisburse","type":["null","boolean"]},{"default":null,"name":"emiAmountVariations","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanTermVariationsDataV1"}]},{"default":null,"name":"loanTermVariations","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanTermVariationsDataV1"}]},{"default":null,"name":"clientActiveLoanOptions","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanAccountSummaryDataV1"}]},{"default":null,"name":"canUseForTopup","type":["null","boolean"]},{"default":null,"name":"isTopup","type":["null","boolean"]},{"default":null,"name":"closureLoanId","type":["null","long"]},{"default":null,"name":"closureLoanAccountNo","type":["null","string"]},{"default":null,"name":"topupAmount","type":["null","bigdecimal"]},{"default":null,"name":"product","type":["null","org.apache.fineract.avro.loan.v1.LoanProductDataV1"]},{"default":null,"name":"inArrears","type":["null","boolean"]},{"default":null,"name":"isNPA","type":["null","boolean"]},{"default":null,"name":"overdueCharges","type":["null",{"type":"array","items":"org.apache.fineract.avro.portfolio.v1.ChargeDataV1"}]},{"default":null,"name":"daysInMonthType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"daysInYearType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"daysInYearCustomStrategy","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"monthEndDueDateStrategy","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"isInterestRecalculationEnabled","type":["null","boolean"]},{"default":null,"name":"interestRecalculationData","type":["null","org.apache.fineract.avro.loan.v1.LoanInterestRecalculationDataV1"]},{"default":null,"name":"createStandingInstructionAtDisbursement","type":["null","boolean"]},{"default":null,"name":"delinquent","type":["null","org.apache.fineract.avro.loan.v1.CollectionDataV1"]},{"default":null,"name":"delinquencyRange","type":["null","org.apache.fineract.avro.loan.v1.DelinquencyRangeDataV1"]},{"default":null,"name":"overpaidOnDate","type":["null","string"]},{"default":null,"name":"lastClosedBusinessDate","type":["null","string"]},{"default":null,"name":"chargedOff","type":["null","boolean"]},{"default":null,"name":"enableInstallmentLevelDelinquency","type":["null","boolean"]},{"default":null,"name":"enableDownPayment","type":["null","boolean"]},{"default":null,"name":"disbursedAmountPercentageForDownPayment","type":["null","bigdecimal"]},{"default":null,"name":"enableAutoRepaymentForDownPayment","type":["null","boolean"]},{"default":null,"name":"repaymentStartDateType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestRecognitionOnDisbursementDate","type":["null","boolean"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]},{"default":null,"name":"originators","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.OriginatorDetailsV1"}]},{"default":null,"name":"actualNoTerm","type":["null","int"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanAccountDelinquencyRangeDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAccountDelinquencyRangeDataV1 {
    #[serde(rename = "loanId")]
    pub loan_id: i64,
    #[serde(rename = "loanAccountNo")]
    pub loan_account_no: String,
    #[serde(rename = "loanExternalId")]
    pub loan_external_id: Option<String>,
    #[serde(rename = "delinquencyRange")]
    pub delinquency_range: Option<DelinquencyRangeDataV1>,
    pub charges: Vec<LoanChargeDataRangeViewV1>,
    pub currency: CurrencyDataV1,
    /// Contains installments  total, fee, interest, principal and penalty amount summaries
    pub amount: LoanAmountDataV1,
    #[serde(rename = "delinquentDate")]
    pub delinquent_date: Option<String>,
    #[serde(rename = "installmentDelinquencyBuckets")]
    pub installment_delinquency_buckets: Option<Vec<LoanInstallmentDelinquencyBucketDataV1>>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
    pub originators: Option<Vec<OriginatorDetailsV1>>,
}

pub const LOAN_ACCOUNT_DELINQUENCY_RANGE_DATA_V1_AVSC: &str = r#"{"name":"LoanAccountDelinquencyRangeDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"name":"loanId","type":"long"},{"name":"loanAccountNo","type":"string"},{"default":null,"name":"loanExternalId","type":["null","string"]},{"default":null,"name":"delinquencyRange","type":["null","org.apache.fineract.avro.loan.v1.DelinquencyRangeDataV1"]},{"name":"charges","type":{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanChargeDataRangeViewV1"}},{"name":"currency","type":"org.apache.fineract.avro.generic.v1.CurrencyDataV1"},{"name":"amount","doc":"Contains installments  total, fee, interest, principal and penalty amount summaries","type":"org.apache.fineract.avro.loan.v1.LoanAmountDataV1"},{"default":null,"name":"delinquentDate","type":["null","string"]},{"default":null,"name":"installmentDelinquencyBuckets","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanInstallmentDelinquencyBucketDataV1"}]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]},{"default":null,"name":"originators","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.OriginatorDetailsV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanAccountStayedLockedDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAccountStayedLockedDataV1 {
    pub id: i64,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
}

pub const LOAN_ACCOUNT_STAYED_LOCKED_DATA_V1_AVSC: &str = r#"{"name":"LoanAccountStayedLockedDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"name":"id","type":"long"},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"accountNo","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanAccountSummaryDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAccountSummaryDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "parentAccountNumber")]
    pub parent_account_number: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<i64>,
    #[serde(rename = "productName")]
    pub product_name: Option<String>,
    #[serde(rename = "shortProductName")]
    pub short_product_name: Option<String>,
    pub status: Option<LoanStatusEnumDataV1>,
    #[serde(rename = "loanType")]
    pub loan_type: Option<EnumOptionDataV1>,
    #[serde(rename = "loanCycle")]
    pub loan_cycle: Option<i32>,
    pub timeline: Option<LoanApplicationTimelineDataV1>,
    #[serde(rename = "inArrears")]
    pub in_arrears: Option<bool>,
    #[serde(rename = "originalLoan")]
    pub original_loan: Option<rust_decimal::Decimal>,
    #[serde(rename = "loanBalance")]
    pub loan_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountPaid")]
    pub amount_paid: Option<rust_decimal::Decimal>,
}

pub const LOAN_ACCOUNT_SUMMARY_DATA_V1_AVSC: &str = r#"{"name":"LoanAccountSummaryDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"parentAccountNumber","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"productId","type":["null","long"]},{"default":null,"name":"productName","type":["null","string"]},{"default":null,"name":"shortProductName","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.loan.v1.LoanStatusEnumDataV1"]},{"default":null,"name":"loanType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"loanCycle","type":["null","int"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.loan.v1.LoanApplicationTimelineDataV1"]},{"default":null,"name":"inArrears","type":["null","boolean"]},{"default":null,"name":"originalLoan","type":["null","bigdecimal"]},{"default":null,"name":"loanBalance","type":["null","bigdecimal"]},{"default":null,"name":"amountPaid","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanAccountsStayedLockedDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAccountsStayedLockedDataV1 {
    #[serde(rename = "loanAccounts")]
    pub loan_accounts: Vec<LoanAccountStayedLockedDataV1>,
}

pub const LOAN_ACCOUNTS_STAYED_LOCKED_DATA_V1_AVSC: &str = r#"{"name":"LoanAccountsStayedLockedDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"name":"loanAccounts","type":{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanAccountStayedLockedDataV1"}}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanAmountDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAmountDataV1 {
    #[serde(rename = "totalAmount")]
    pub total_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalAmount")]
    pub principal_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestAmount")]
    pub interest_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeAmount")]
    pub fee_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyAmount")]
    pub penalty_amount: Option<rust_decimal::Decimal>,
}

pub const LOAN_AMOUNT_DATA_V1_AVSC: &str = r#"{"name":"LoanAmountDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"totalAmount","type":["null","bigdecimal"]},{"default":null,"name":"principalAmount","type":["null","bigdecimal"]},{"default":null,"name":"interestAmount","type":["null","bigdecimal"]},{"default":null,"name":"feeAmount","type":["null","bigdecimal"]},{"default":null,"name":"penaltyAmount","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanApplicationTimelineDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanApplicationTimelineDataV1 {
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    #[serde(rename = "submittedByUsername")]
    pub submitted_by_username: Option<String>,
    #[serde(rename = "submittedByFirstname")]
    pub submitted_by_firstname: Option<String>,
    #[serde(rename = "submittedByLastname")]
    pub submitted_by_lastname: Option<String>,
    #[serde(rename = "rejectedOnDate")]
    pub rejected_on_date: Option<String>,
    #[serde(rename = "rejectedByUsername")]
    pub rejected_by_username: Option<String>,
    #[serde(rename = "rejectedByFirstname")]
    pub rejected_by_firstname: Option<String>,
    #[serde(rename = "rejectedByLastname")]
    pub rejected_by_lastname: Option<String>,
    #[serde(rename = "withdrawnOnDate")]
    pub withdrawn_on_date: Option<String>,
    #[serde(rename = "withdrawnByUsername")]
    pub withdrawn_by_username: Option<String>,
    #[serde(rename = "withdrawnByFirstname")]
    pub withdrawn_by_firstname: Option<String>,
    #[serde(rename = "withdrawnByLastname")]
    pub withdrawn_by_lastname: Option<String>,
    #[serde(rename = "approvedOnDate")]
    pub approved_on_date: Option<String>,
    #[serde(rename = "approvedByUsername")]
    pub approved_by_username: Option<String>,
    #[serde(rename = "approvedByFirstname")]
    pub approved_by_firstname: Option<String>,
    #[serde(rename = "approvedByLastname")]
    pub approved_by_lastname: Option<String>,
    #[serde(rename = "expectedDisbursementDate")]
    pub expected_disbursement_date: Option<String>,
    #[serde(rename = "actualDisbursementDate")]
    pub actual_disbursement_date: Option<String>,
    #[serde(rename = "disbursedByUsername")]
    pub disbursed_by_username: Option<String>,
    #[serde(rename = "disbursedByFirstname")]
    pub disbursed_by_firstname: Option<String>,
    #[serde(rename = "disbursedByLastname")]
    pub disbursed_by_lastname: Option<String>,
    #[serde(rename = "closedOnDate")]
    pub closed_on_date: Option<String>,
    #[serde(rename = "closedByUsername")]
    pub closed_by_username: Option<String>,
    #[serde(rename = "closedByFirstname")]
    pub closed_by_firstname: Option<String>,
    #[serde(rename = "closedByLastname")]
    pub closed_by_lastname: Option<String>,
    #[serde(rename = "expectedMaturityDate")]
    pub expected_maturity_date: Option<String>,
    #[serde(rename = "writeOffOnDate")]
    pub write_off_on_date: Option<String>,
    #[serde(rename = "writeOffByUsername")]
    pub write_off_by_username: Option<String>,
    #[serde(rename = "writeOffByFirstname")]
    pub write_off_by_firstname: Option<String>,
    #[serde(rename = "writeOffByLastname")]
    pub write_off_by_lastname: Option<String>,
    #[serde(rename = "chargedOffOnDate")]
    pub charged_off_on_date: Option<String>,
    #[serde(rename = "chargedOffByUsername")]
    pub charged_off_by_username: Option<String>,
    #[serde(rename = "chargedOffByFirstname")]
    pub charged_off_by_firstname: Option<String>,
    #[serde(rename = "chargedOffByLastname")]
    pub charged_off_by_lastname: Option<String>,
}

pub const LOAN_APPLICATION_TIMELINE_DATA_V1_AVSC: &str = r#"{"name":"LoanApplicationTimelineDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"submittedByUsername","type":["null","string"]},{"default":null,"name":"submittedByFirstname","type":["null","string"]},{"default":null,"name":"submittedByLastname","type":["null","string"]},{"default":null,"name":"rejectedOnDate","type":["null","string"]},{"default":null,"name":"rejectedByUsername","type":["null","string"]},{"default":null,"name":"rejectedByFirstname","type":["null","string"]},{"default":null,"name":"rejectedByLastname","type":["null","string"]},{"default":null,"name":"withdrawnOnDate","type":["null","string"]},{"default":null,"name":"withdrawnByUsername","type":["null","string"]},{"default":null,"name":"withdrawnByFirstname","type":["null","string"]},{"default":null,"name":"withdrawnByLastname","type":["null","string"]},{"default":null,"name":"approvedOnDate","type":["null","string"]},{"default":null,"name":"approvedByUsername","type":["null","string"]},{"default":null,"name":"approvedByFirstname","type":["null","string"]},{"default":null,"name":"approvedByLastname","type":["null","string"]},{"default":null,"name":"expectedDisbursementDate","type":["null","string"]},{"default":null,"name":"actualDisbursementDate","type":["null","string"]},{"default":null,"name":"disbursedByUsername","type":["null","string"]},{"default":null,"name":"disbursedByFirstname","type":["null","string"]},{"default":null,"name":"disbursedByLastname","type":["null","string"]},{"default":null,"name":"closedOnDate","type":["null","string"]},{"default":null,"name":"closedByUsername","type":["null","string"]},{"default":null,"name":"closedByFirstname","type":["null","string"]},{"default":null,"name":"closedByLastname","type":["null","string"]},{"default":null,"name":"expectedMaturityDate","type":["null","string"]},{"default":null,"name":"writeOffOnDate","type":["null","string"]},{"default":null,"name":"writeOffByUsername","type":["null","string"]},{"default":null,"name":"writeOffByFirstname","type":["null","string"]},{"default":null,"name":"writeOffByLastname","type":["null","string"]},{"default":null,"name":"chargedOffOnDate","type":["null","string"]},{"default":null,"name":"chargedOffByUsername","type":["null","string"]},{"default":null,"name":"chargedOffByFirstname","type":["null","string"]},{"default":null,"name":"chargedOffByLastname","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanChargeDataRangeViewV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanChargeDataRangeViewV1 {
    pub id: i64,
    pub name: String,
    pub currency: CurrencyDataV1,
    pub amount: rust_decimal::Decimal,
}

pub const LOAN_CHARGE_DATA_RANGE_VIEW_V1_AVSC: &str = r#"{"name":"LoanChargeDataRangeViewV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"name":"id","type":"long"},{"name":"name","type":"string"},{"name":"currency","type":"org.apache.fineract.avro.generic.v1.CurrencyDataV1"},{"name":"amount","type":"bigdecimal"}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanChargeDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanChargeDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "chargeId")]
    pub charge_id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "chargeTimeType")]
    pub charge_time_type: Option<EnumOptionDataV1>,
    #[serde(rename = "dueDate")]
    pub due_date: Option<String>,
    #[serde(rename = "chargeCalculationType")]
    pub charge_calculation_type: Option<EnumOptionDataV1>,
    pub percentage: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountPercentageAppliedTo")]
    pub amount_percentage_applied_to: Option<rust_decimal::Decimal>,
    pub currency: Option<CurrencyDataV1>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountPaid")]
    pub amount_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountWaived")]
    pub amount_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountWrittenOff")]
    pub amount_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountOutstanding")]
    pub amount_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountOrPercentage")]
    pub amount_or_percentage: Option<rust_decimal::Decimal>,
    #[serde(rename = "chargeOptions")]
    pub charge_options: Option<Vec<ChargeDataV1>>,
    pub penalty: Option<bool>,
    #[serde(rename = "chargePaymentMode")]
    pub charge_payment_mode: Option<EnumOptionDataV1>,
    pub paid: Option<bool>,
    pub waived: Option<bool>,
    #[serde(rename = "chargePayable")]
    pub charge_payable: Option<bool>,
    #[serde(rename = "loanId")]
    pub loan_id: Option<i64>,
    #[serde(rename = "minCap")]
    pub min_cap: Option<rust_decimal::Decimal>,
    #[serde(rename = "maxCap")]
    pub max_cap: Option<rust_decimal::Decimal>,
    #[serde(rename = "installmentChargeData")]
    pub installment_charge_data: Option<Vec<LoanInstallmentChargeDataV1>>,
    #[serde(rename = "amountAccrued")]
    pub amount_accrued: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountUnrecognized")]
    pub amount_unrecognized: Option<rust_decimal::Decimal>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "externalLoanId")]
    pub external_loan_id: Option<String>,
    #[serde(rename = "externalOwnerId")]
    pub external_owner_id: Option<String>,
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
    /// List of originators attached to the parent loan for revenue sharing
    pub originators: Option<Vec<OriginatorDetailsV1>>,
}

pub const LOAN_CHARGE_DATA_V1_AVSC: &str = r#"{"name":"LoanChargeDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"chargeId","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"chargeTimeType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"dueDate","type":["null","string"]},{"default":null,"name":"chargeCalculationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"percentage","type":["null","bigdecimal"]},{"default":null,"name":"amountPercentageAppliedTo","type":["null","bigdecimal"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"amountPaid","type":["null","bigdecimal"]},{"default":null,"name":"amountWaived","type":["null","bigdecimal"]},{"default":null,"name":"amountWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"amountOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"amountOrPercentage","type":["null","bigdecimal"]},{"default":null,"name":"chargeOptions","type":["null",{"type":"array","items":"org.apache.fineract.avro.portfolio.v1.ChargeDataV1"}]},{"default":null,"name":"penalty","type":["null","boolean"]},{"default":null,"name":"chargePaymentMode","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"paid","type":["null","boolean"]},{"default":null,"name":"waived","type":["null","boolean"]},{"default":null,"name":"chargePayable","type":["null","boolean"]},{"default":null,"name":"loanId","type":["null","long"]},{"default":null,"name":"minCap","type":["null","bigdecimal"]},{"default":null,"name":"maxCap","type":["null","bigdecimal"]},{"default":null,"name":"installmentChargeData","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanInstallmentChargeDataV1"}]},{"default":null,"name":"amountAccrued","type":["null","bigdecimal"]},{"default":null,"name":"amountUnrecognized","type":["null","bigdecimal"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"externalLoanId","type":["null","string"]},{"default":null,"name":"externalOwnerId","type":["null","string"]},{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]},{"default":null,"name":"originators","doc":"List of originators attached to the parent loan for revenue sharing","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.OriginatorDetailsV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanChargeDeletedV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanChargeDeletedV1 {
    pub id: Option<i64>,
    #[serde(rename = "chargeId")]
    pub charge_id: Option<i64>,
}

pub const LOAN_CHARGE_DELETED_V1_AVSC: &str = r#"{"name":"LoanChargeDeletedV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"chargeId","type":["null","long"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanChargePaidByDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanChargePaidByDataV1 {
    pub id: Option<i32>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "installmentNumber")]
    pub installment_number: Option<i32>,
    #[serde(rename = "chargeId")]
    pub charge_id: Option<i64>,
    #[serde(rename = "transactionId")]
    pub transaction_id: Option<i64>,
    pub name: Option<String>,
}

pub const LOAN_CHARGE_PAID_BY_DATA_V1_AVSC: &str = r#"{"name":"LoanChargePaidByDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"installmentNumber","type":["null","int"]},{"default":null,"name":"chargeId","type":["null","long"]},{"default":null,"name":"transactionId","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanInstallmentChargeDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanInstallmentChargeDataV1 {
    #[serde(rename = "installmentNumber")]
    pub installment_number: Option<i32>,
    #[serde(rename = "dueDate")]
    pub due_date: Option<String>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountOutstanding")]
    pub amount_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountWaived")]
    pub amount_waived: Option<rust_decimal::Decimal>,
    pub paid: Option<bool>,
    pub waived: Option<bool>,
    #[serde(rename = "amountAccrued")]
    pub amount_accrued: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountUnrecognized")]
    pub amount_unrecognized: Option<rust_decimal::Decimal>,
}

pub const LOAN_INSTALLMENT_CHARGE_DATA_V1_AVSC: &str = r#"{"name":"LoanInstallmentChargeDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"installmentNumber","type":["null","int"]},{"default":null,"name":"dueDate","type":["null","string"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"amountOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"amountWaived","type":["null","bigdecimal"]},{"default":null,"name":"paid","type":["null","boolean"]},{"default":null,"name":"waived","type":["null","boolean"]},{"default":null,"name":"amountAccrued","type":["null","bigdecimal"]},{"default":null,"name":"amountUnrecognized","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanInstallmentDelinquencyBucketDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanInstallmentDelinquencyBucketDataV1 {
    #[serde(rename = "delinquencyRange")]
    pub delinquency_range: Option<DelinquencyRangeDataV1>,
    /// Contains installments  total, fee, interest, principal and penalty amount summaries
    pub amount: LoanAmountDataV1,
    pub charges: Vec<LoanChargeDataRangeViewV1>,
    pub currency: CurrencyDataV1,
}

pub const LOAN_INSTALLMENT_DELINQUENCY_BUCKET_DATA_V1_AVSC: &str = r#"{"name":"LoanInstallmentDelinquencyBucketDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"delinquencyRange","type":["null","org.apache.fineract.avro.loan.v1.DelinquencyRangeDataV1"]},{"name":"amount","doc":"Contains installments  total, fee, interest, principal and penalty amount summaries","type":"org.apache.fineract.avro.loan.v1.LoanAmountDataV1"},{"name":"charges","type":{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanChargeDataRangeViewV1"}},{"name":"currency","type":"org.apache.fineract.avro.generic.v1.CurrencyDataV1"}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanInterestRecalculationDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanInterestRecalculationDataV1 {
    pub id: Option<i32>,
    #[serde(rename = "loanId")]
    pub loan_id: Option<i64>,
    #[serde(rename = "interestRecalculationCompoundingType")]
    pub interest_recalculation_compounding_type: Option<EnumOptionDataV1>,
    #[serde(rename = "rescheduleStrategyType")]
    pub reschedule_strategy_type: Option<EnumOptionDataV1>,
    #[serde(rename = "calendarData")]
    pub calendar_data: Option<CalendarDataV1>,
    #[serde(rename = "recalculationRestFrequencyType")]
    pub recalculation_rest_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationRestFrequencyInterval")]
    pub recalculation_rest_frequency_interval: Option<i32>,
    #[serde(rename = "recalculationRestFrequencyNthDay")]
    pub recalculation_rest_frequency_nth_day: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationRestFrequencyWeekday")]
    pub recalculation_rest_frequency_weekday: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationRestFrequencyOnDay")]
    pub recalculation_rest_frequency_on_day: Option<i32>,
    #[serde(rename = "recalculationCompoundingFrequencyType")]
    pub recalculation_compounding_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationCompoundingFrequencyInterval")]
    pub recalculation_compounding_frequency_interval: Option<i32>,
    #[serde(rename = "recalculationCompoundingFrequencyNthDay")]
    pub recalculation_compounding_frequency_nth_day: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationCompoundingFrequencyWeekday")]
    pub recalculation_compounding_frequency_weekday: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationCompoundingFrequencyOnDay")]
    pub recalculation_compounding_frequency_on_day: Option<i32>,
    #[serde(rename = "isCompoundingToBePostedAsTransaction")]
    pub is_compounding_to_be_posted_as_transaction: Option<bool>,
    #[serde(rename = "compoundingCalendarData")]
    pub compounding_calendar_data: Option<CalendarDataV1>,
    #[serde(rename = "allowCompoundingOnEod")]
    pub allow_compounding_on_eod: Option<bool>,
    #[serde(rename = "disallowInterestCalculationOnPastDue")]
    pub disallow_interest_calculation_on_past_due: Option<bool>,
}

pub const LOAN_INTEREST_RECALCULATION_DATA_V1_AVSC: &str = r#"{"name":"LoanInterestRecalculationDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"loanId","type":["null","long"]},{"default":null,"name":"interestRecalculationCompoundingType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"rescheduleStrategyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"calendarData","type":["null","org.apache.fineract.avro.generic.v1.CalendarDataV1"]},{"default":null,"name":"recalculationRestFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationRestFrequencyInterval","type":["null","int"]},{"default":null,"name":"recalculationRestFrequencyNthDay","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationRestFrequencyWeekday","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationRestFrequencyOnDay","type":["null","int"]},{"default":null,"name":"recalculationCompoundingFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationCompoundingFrequencyInterval","type":["null","int"]},{"default":null,"name":"recalculationCompoundingFrequencyNthDay","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationCompoundingFrequencyWeekday","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationCompoundingFrequencyOnDay","type":["null","int"]},{"default":null,"name":"isCompoundingToBePostedAsTransaction","type":["null","boolean"]},{"default":null,"name":"compoundingCalendarData","type":["null","org.apache.fineract.avro.generic.v1.CalendarDataV1"]},{"default":null,"name":"allowCompoundingOnEod","type":["null","boolean"]},{"default":null,"name":"disallowInterestCalculationOnPastDue","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanOwnershipTransferDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanOwnershipTransferDataV1 {
    #[serde(rename = "loanId")]
    pub loan_id: i64,
    #[serde(rename = "loanExternalId")]
    pub loan_external_id: Option<String>,
    pub r#type: Option<String>,
    #[serde(rename = "transferExternalId")]
    pub transfer_external_id: Option<String>,
    #[serde(rename = "transferExternalGroupId")]
    pub transfer_external_group_id: Option<String>,
    #[serde(rename = "submittedDate")]
    pub submitted_date: Option<String>,
    #[serde(rename = "assetOwnerExternalId")]
    pub asset_owner_external_id: Option<String>,
    #[serde(rename = "previousOwnerExternalId")]
    pub previous_owner_external_id: Option<String>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "totalOutstandingBalanceAmount")]
    pub total_outstanding_balance_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "outstandingPrincipalPortion")]
    pub outstanding_principal_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "outstandingFeePortion")]
    pub outstanding_fee_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "outstandingPenaltyPortion")]
    pub outstanding_penalty_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "outstandingInterestPortion")]
    pub outstanding_interest_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "overPaymentPortion")]
    pub over_payment_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "unrecognizedIncomePortion")]
    pub unrecognized_income_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "unpaidChargeData")]
    pub unpaid_charge_data: Option<Vec<UnpaidChargeDataV1>>,
    #[serde(rename = "transferStatus")]
    pub transfer_status: Option<String>,
    #[serde(rename = "transferStatusReason")]
    pub transfer_status_reason: Option<String>,
    #[serde(rename = "settlementDate")]
    pub settlement_date: Option<String>,
    #[serde(rename = "purchasePriceRatio")]
    pub purchase_price_ratio: Option<String>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
}

pub const LOAN_OWNERSHIP_TRANSFER_DATA_V1_AVSC: &str = r#"{"name":"LoanOwnershipTransferDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"name":"loanId","type":"long"},{"default":null,"name":"loanExternalId","type":["null","string"]},{"default":null,"name":"type","type":["null","string"]},{"default":null,"name":"transferExternalId","type":["null","string"]},{"default":null,"name":"transferExternalGroupId","type":["null","string"]},{"default":null,"name":"submittedDate","type":["null","string"]},{"default":null,"name":"assetOwnerExternalId","type":["null","string"]},{"default":null,"name":"previousOwnerExternalId","type":["null","string"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"totalOutstandingBalanceAmount","type":["null","bigdecimal"]},{"default":null,"name":"outstandingPrincipalPortion","type":["null","bigdecimal"]},{"default":null,"name":"outstandingFeePortion","type":["null","bigdecimal"]},{"default":null,"name":"outstandingPenaltyPortion","type":["null","bigdecimal"]},{"default":null,"name":"outstandingInterestPortion","type":["null","bigdecimal"]},{"default":null,"name":"overPaymentPortion","type":["null","bigdecimal"]},{"default":null,"name":"unrecognizedIncomePortion","type":["null","bigdecimal"]},{"default":null,"name":"unpaidChargeData","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.UnpaidChargeDataV1"}]},{"default":null,"name":"transferStatus","type":["null","string"]},{"default":null,"name":"transferStatusReason","type":["null","string"]},{"default":null,"name":"settlementDate","type":["null","string"]},{"default":null,"name":"purchasePriceRatio","type":["null","string"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanProductBorrowerCycleVariationDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanProductBorrowerCycleVariationDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "borrowerCycleNumber")]
    pub borrower_cycle_number: Option<i32>,
    #[serde(rename = "paramType")]
    pub param_type: Option<EnumOptionDataV1>,
    #[serde(rename = "valueConditionType")]
    pub value_condition_type: Option<EnumOptionDataV1>,
    #[serde(rename = "minValue")]
    pub min_value: Option<rust_decimal::Decimal>,
    #[serde(rename = "maxValue")]
    pub max_value: Option<rust_decimal::Decimal>,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<rust_decimal::Decimal>,
}

pub const LOAN_PRODUCT_BORROWER_CYCLE_VARIATION_DATA_V1_AVSC: &str = r#"{"name":"LoanProductBorrowerCycleVariationDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"borrowerCycleNumber","type":["null","int"]},{"default":null,"name":"paramType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"valueConditionType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minValue","type":["null","bigdecimal"]},{"default":null,"name":"maxValue","type":["null","bigdecimal"]},{"default":null,"name":"defaultValue","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanProductDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanProductDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "shortName")]
    pub short_name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "fundId")]
    pub fund_id: Option<i64>,
    #[serde(rename = "fundName")]
    pub fund_name: Option<String>,
    #[serde(rename = "includeInBorrowerCycle")]
    pub include_in_borrower_cycle: Option<bool>,
    #[serde(rename = "useBorrowerCycle")]
    pub use_borrower_cycle: Option<bool>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "closeDate")]
    pub close_date: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    pub currency: Option<CurrencyDataV1>,
    pub principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "minPrincipal")]
    pub min_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "maxPrincipal")]
    pub max_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "numberOfRepayments")]
    pub number_of_repayments: Option<i32>,
    #[serde(rename = "minNumberOfRepayments")]
    pub min_number_of_repayments: Option<i32>,
    #[serde(rename = "maxNumberOfRepayments")]
    pub max_number_of_repayments: Option<i32>,
    #[serde(rename = "repaymentEvery")]
    pub repayment_every: Option<i32>,
    #[serde(rename = "repaymentFrequencyType")]
    pub repayment_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestRatePerPeriod")]
    pub interest_rate_per_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "minInterestRatePerPeriod")]
    pub min_interest_rate_per_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "maxInterestRatePerPeriod")]
    pub max_interest_rate_per_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestRateFrequencyType")]
    pub interest_rate_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "annualInterestRate")]
    pub annual_interest_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "isLinkedToFloatingInterestRates")]
    pub is_linked_to_floating_interest_rates: Option<bool>,
    #[serde(rename = "floatingRateId")]
    pub floating_rate_id: Option<i64>,
    #[serde(rename = "floatingRateName")]
    pub floating_rate_name: Option<String>,
    #[serde(rename = "interestRateDifferential")]
    pub interest_rate_differential: Option<rust_decimal::Decimal>,
    #[serde(rename = "minDifferentialLendingRate")]
    pub min_differential_lending_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "defaultDifferentialLendingRate")]
    pub default_differential_lending_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "maxDifferentialLendingRate")]
    pub max_differential_lending_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "isFloatingInterestRateCalculationAllowed")]
    pub is_floating_interest_rate_calculation_allowed: Option<bool>,
    #[serde(rename = "allowVariableInstallments")]
    pub allow_variable_installments: Option<bool>,
    #[serde(rename = "minimumGap")]
    pub minimum_gap: Option<i32>,
    #[serde(rename = "maximumGap")]
    pub maximum_gap: Option<i32>,
    #[serde(rename = "amortizationType")]
    pub amortization_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestType")]
    pub interest_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestCalculationPeriodType")]
    pub interest_calculation_period_type: Option<EnumOptionDataV1>,
    #[serde(rename = "allowPartialPeriodInterestCalculation")]
    pub allow_partial_period_interest_calculation: Option<bool>,
    #[serde(rename = "inArrearsTolerance")]
    pub in_arrears_tolerance: Option<rust_decimal::Decimal>,
    #[serde(rename = "transactionProcessingStrategyCode")]
    pub transaction_processing_strategy_code: Option<String>,
    #[serde(rename = "transactionProcessingStrategyName")]
    pub transaction_processing_strategy_name: Option<String>,
    #[serde(rename = "graceOnPrincipalPayment")]
    pub grace_on_principal_payment: Option<i32>,
    #[serde(rename = "recurringMoratoriumOnPrincipalPeriods")]
    pub recurring_moratorium_on_principal_periods: Option<i32>,
    #[serde(rename = "graceOnInterestPayment")]
    pub grace_on_interest_payment: Option<i32>,
    #[serde(rename = "graceOnInterestCharged")]
    pub grace_on_interest_charged: Option<i32>,
    #[serde(rename = "graceOnArrearsAgeing")]
    pub grace_on_arrears_ageing: Option<i32>,
    #[serde(rename = "overdueDaysForNPA")]
    pub overdue_days_for_npa: Option<i32>,
    #[serde(rename = "daysInMonthType")]
    pub days_in_month_type: Option<EnumOptionDataV1>,
    #[serde(rename = "daysInYearType")]
    pub days_in_year_type: Option<EnumOptionDataV1>,
    #[serde(rename = "daysInYearCustomStrategy")]
    pub days_in_year_custom_strategy: Option<StringEnumOptionDataV1>,
    #[serde(rename = "monthEndDueDateStrategy")]
    pub month_end_due_date_strategy: Option<StringEnumOptionDataV1>,
    #[serde(rename = "isInterestRecalculationEnabled")]
    pub is_interest_recalculation_enabled: Option<bool>,
    #[serde(rename = "interestRecalculationData")]
    pub interest_recalculation_data: Option<LoanProductInterestRecalculationDataV1>,
    #[serde(rename = "minimumDaysBetweenDisbursalAndFirstRepayment")]
    pub minimum_days_between_disbursal_and_first_repayment: Option<i32>,
    #[serde(rename = "canDefineInstallmentAmount")]
    pub can_define_installment_amount: Option<bool>,
    #[serde(rename = "installmentAmountInMultiplesOf")]
    pub installment_amount_in_multiples_of: Option<i32>,
    pub charges: Option<Vec<ChargeDataV1>>,
    #[serde(rename = "principalVariationsForBorrowerCycle")]
    pub principal_variations_for_borrower_cycle: Option<Vec<LoanProductBorrowerCycleVariationDataV1>>,
    #[serde(rename = "interestRateVariationsForBorrowerCycle")]
    pub interest_rate_variations_for_borrower_cycle: Option<Vec<LoanProductBorrowerCycleVariationDataV1>>,
    #[serde(rename = "numberOfRepaymentVariationsForBorrowerCycle")]
    pub number_of_repayment_variations_for_borrower_cycle: Option<Vec<LoanProductBorrowerCycleVariationDataV1>>,
    #[serde(rename = "isRatesEnabled")]
    pub is_rates_enabled: Option<bool>,
    pub rates: Option<Vec<RateDataV1>>,
    #[serde(rename = "multiDisburseLoan")]
    pub multi_disburse_loan: Option<bool>,
    #[serde(rename = "maxTrancheCount")]
    pub max_tranche_count: Option<i32>,
    #[serde(rename = "outstandingLoanBalance")]
    pub outstanding_loan_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "disallowExpectedDisbursements")]
    pub disallow_expected_disbursements: Option<bool>,
    #[serde(rename = "allowApprovedDisbursedAmountsOverApplied")]
    pub allow_approved_disbursed_amounts_over_applied: Option<bool>,
    #[serde(rename = "overAppliedCalculationType")]
    pub over_applied_calculation_type: Option<String>,
    #[serde(rename = "overAppliedNumber")]
    pub over_applied_number: Option<i32>,
    #[serde(rename = "principalThresholdForLastInstallment")]
    pub principal_threshold_for_last_installment: Option<rust_decimal::Decimal>,
    #[serde(rename = "holdGuaranteeFunds")]
    pub hold_guarantee_funds: Option<bool>,
    #[serde(rename = "productGuaranteeData")]
    pub product_guarantee_data: Option<LoanProductGuaranteeDataV1>,
    #[serde(rename = "accountMovesOutOfNPAOnlyOnArrearsCompletion")]
    pub account_moves_out_of_npa_only_on_arrears_completion: Option<bool>,
    #[serde(rename = "syncExpectedWithDisbursementDate")]
    pub sync_expected_with_disbursement_date: Option<bool>,
    #[serde(rename = "isEqualAmortization")]
    pub is_equal_amortization: Option<bool>,
    #[serde(rename = "fixedPrincipalPercentagePerInstallment")]
    pub fixed_principal_percentage_per_installment: Option<rust_decimal::Decimal>,
    #[serde(rename = "delinquencyBucket")]
    pub delinquency_bucket: Option<DelinquencyBucketDataV1>,
    #[serde(rename = "interestRecognitionOnDisbursementDate")]
    pub interest_recognition_on_disbursement_date: Option<bool>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
}

pub const LOAN_PRODUCT_DATA_V1_AVSC: &str = r#"{"name":"LoanProductDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"shortName","type":["null","string"]},{"default":null,"name":"description","type":["null","string"]},{"default":null,"name":"fundId","type":["null","long"]},{"default":null,"name":"fundName","type":["null","string"]},{"default":null,"name":"includeInBorrowerCycle","type":["null","boolean"]},{"default":null,"name":"useBorrowerCycle","type":["null","boolean"]},{"default":null,"name":"startDate","type":["null","string"]},{"default":null,"name":"closeDate","type":["null","string"]},{"default":null,"name":"status","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"principal","type":["null","bigdecimal"]},{"default":null,"name":"minPrincipal","type":["null","bigdecimal"]},{"default":null,"name":"maxPrincipal","type":["null","bigdecimal"]},{"default":null,"name":"numberOfRepayments","type":["null","int"]},{"default":null,"name":"minNumberOfRepayments","type":["null","int"]},{"default":null,"name":"maxNumberOfRepayments","type":["null","int"]},{"default":null,"name":"repaymentEvery","type":["null","int"]},{"default":null,"name":"repaymentFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestRatePerPeriod","type":["null","bigdecimal"]},{"default":null,"name":"minInterestRatePerPeriod","type":["null","bigdecimal"]},{"default":null,"name":"maxInterestRatePerPeriod","type":["null","bigdecimal"]},{"default":null,"name":"interestRateFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"annualInterestRate","type":["null","bigdecimal"]},{"default":null,"name":"isLinkedToFloatingInterestRates","type":["null","boolean"]},{"default":null,"name":"floatingRateId","type":["null","long"]},{"default":null,"name":"floatingRateName","type":["null","string"]},{"default":null,"name":"interestRateDifferential","type":["null","bigdecimal"]},{"default":null,"name":"minDifferentialLendingRate","type":["null","bigdecimal"]},{"default":null,"name":"defaultDifferentialLendingRate","type":["null","bigdecimal"]},{"default":null,"name":"maxDifferentialLendingRate","type":["null","bigdecimal"]},{"default":null,"name":"isFloatingInterestRateCalculationAllowed","type":["null","boolean"]},{"default":null,"name":"allowVariableInstallments","type":["null","boolean"]},{"default":null,"name":"minimumGap","type":["null","int"]},{"default":null,"name":"maximumGap","type":["null","int"]},{"default":null,"name":"amortizationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestCalculationPeriodType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"allowPartialPeriodInterestCalculation","type":["null","boolean"]},{"default":null,"name":"inArrearsTolerance","type":["null","bigdecimal"]},{"default":null,"name":"transactionProcessingStrategyCode","type":["null","string"]},{"default":null,"name":"transactionProcessingStrategyName","type":["null","string"]},{"default":null,"name":"graceOnPrincipalPayment","type":["null","int"]},{"default":null,"name":"recurringMoratoriumOnPrincipalPeriods","type":["null","int"]},{"default":null,"name":"graceOnInterestPayment","type":["null","int"]},{"default":null,"name":"graceOnInterestCharged","type":["null","int"]},{"default":null,"name":"graceOnArrearsAgeing","type":["null","int"]},{"default":null,"name":"overdueDaysForNPA","type":["null","int"]},{"default":null,"name":"daysInMonthType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"daysInYearType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"daysInYearCustomStrategy","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"monthEndDueDateStrategy","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"isInterestRecalculationEnabled","type":["null","boolean"]},{"default":null,"name":"interestRecalculationData","type":["null","org.apache.fineract.avro.loan.v1.LoanProductInterestRecalculationDataV1"]},{"default":null,"name":"minimumDaysBetweenDisbursalAndFirstRepayment","type":["null","int"]},{"default":null,"name":"canDefineInstallmentAmount","type":["null","boolean"]},{"default":null,"name":"installmentAmountInMultiplesOf","type":["null","int"]},{"default":null,"name":"charges","type":["null",{"type":"array","items":"org.apache.fineract.avro.portfolio.v1.ChargeDataV1"}]},{"default":null,"name":"principalVariationsForBorrowerCycle","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanProductBorrowerCycleVariationDataV1"}]},{"default":null,"name":"interestRateVariationsForBorrowerCycle","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanProductBorrowerCycleVariationDataV1"}]},{"default":null,"name":"numberOfRepaymentVariationsForBorrowerCycle","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanProductBorrowerCycleVariationDataV1"}]},{"default":null,"name":"isRatesEnabled","type":["null","boolean"]},{"default":null,"name":"rates","type":["null",{"type":"array","items":"org.apache.fineract.avro.portfolio.v1.RateDataV1"}]},{"default":null,"name":"multiDisburseLoan","type":["null","boolean"]},{"default":null,"name":"maxTrancheCount","type":["null","int"]},{"default":null,"name":"outstandingLoanBalance","type":["null","bigdecimal"]},{"default":null,"name":"disallowExpectedDisbursements","type":["null","boolean"]},{"default":null,"name":"allowApprovedDisbursedAmountsOverApplied","type":["null","boolean"]},{"default":null,"name":"overAppliedCalculationType","type":["null","string"]},{"default":null,"name":"overAppliedNumber","type":["null","int"]},{"default":null,"name":"principalThresholdForLastInstallment","type":["null","bigdecimal"]},{"default":null,"name":"holdGuaranteeFunds","type":["null","boolean"]},{"default":null,"name":"productGuaranteeData","type":["null","org.apache.fineract.avro.loan.v1.LoanProductGuaranteeDataV1"]},{"default":null,"name":"accountMovesOutOfNPAOnlyOnArrearsCompletion","type":["null","boolean"]},{"default":null,"name":"syncExpectedWithDisbursementDate","type":["null","boolean"]},{"default":null,"name":"isEqualAmortization","type":["null","boolean"]},{"default":null,"name":"fixedPrincipalPercentagePerInstallment","type":["null","bigdecimal"]},{"default":null,"name":"delinquencyBucket","type":["null","org.apache.fineract.avro.loan.v1.DelinquencyBucketDataV1"]},{"default":null,"name":"interestRecognitionOnDisbursementDate","type":["null","boolean"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanProductGuaranteeDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanProductGuaranteeDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "productId")]
    pub product_id: Option<i64>,
    #[serde(rename = "mandatoryGuarantee")]
    pub mandatory_guarantee: Option<rust_decimal::Decimal>,
    #[serde(rename = "minimumGuaranteeFromOwnFunds")]
    pub minimum_guarantee_from_own_funds: Option<rust_decimal::Decimal>,
    #[serde(rename = "minimumGuaranteeFromGuarantor")]
    pub minimum_guarantee_from_guarantor: Option<rust_decimal::Decimal>,
}

pub const LOAN_PRODUCT_GUARANTEE_DATA_V1_AVSC: &str = r#"{"name":"LoanProductGuaranteeDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"productId","type":["null","long"]},{"default":null,"name":"mandatoryGuarantee","type":["null","bigdecimal"]},{"default":null,"name":"minimumGuaranteeFromOwnFunds","type":["null","bigdecimal"]},{"default":null,"name":"minimumGuaranteeFromGuarantor","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanProductInterestRecalculationDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanProductInterestRecalculationDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "productId")]
    pub product_id: Option<i64>,
    #[serde(rename = "interestRecalculationCompoundingType")]
    pub interest_recalculation_compounding_type: Option<EnumOptionDataV1>,
    #[serde(rename = "rescheduleStrategyType")]
    pub reschedule_strategy_type: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationRestFrequencyType")]
    pub recalculation_rest_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationRestFrequencyInterval")]
    pub recalculation_rest_frequency_interval: Option<i32>,
    #[serde(rename = "recalculationRestFrequencyNthDay")]
    pub recalculation_rest_frequency_nth_day: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationRestFrequencyWeekday")]
    pub recalculation_rest_frequency_weekday: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationRestFrequencyOnDay")]
    pub recalculation_rest_frequency_on_day: Option<i32>,
    #[serde(rename = "recalculationCompoundingFrequencyType")]
    pub recalculation_compounding_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationCompoundingFrequencyInterval")]
    pub recalculation_compounding_frequency_interval: Option<i32>,
    #[serde(rename = "recalculationCompoundingFrequencyNthDay")]
    pub recalculation_compounding_frequency_nth_day: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationCompoundingFrequencyWeekday")]
    pub recalculation_compounding_frequency_weekday: Option<EnumOptionDataV1>,
    #[serde(rename = "recalculationCompoundingFrequencyOnDay")]
    pub recalculation_compounding_frequency_on_day: Option<i32>,
    #[serde(rename = "isArrearsBasedOnOriginalSchedule")]
    pub is_arrears_based_on_original_schedule: Option<bool>,
    #[serde(rename = "isCompoundingToBePostedAsTransaction")]
    pub is_compounding_to_be_posted_as_transaction: Option<bool>,
    #[serde(rename = "preClosureInterestCalculationStrategy")]
    pub pre_closure_interest_calculation_strategy: Option<EnumOptionDataV1>,
    #[serde(rename = "allowCompoundingOnEod")]
    pub allow_compounding_on_eod: Option<bool>,
    #[serde(rename = "disallowInterestCalculationOnPastDue")]
    pub disallow_interest_calculation_on_past_due: Option<bool>,
}

pub const LOAN_PRODUCT_INTEREST_RECALCULATION_DATA_V1_AVSC: &str = r#"{"name":"LoanProductInterestRecalculationDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"productId","type":["null","long"]},{"default":null,"name":"interestRecalculationCompoundingType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"rescheduleStrategyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationRestFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationRestFrequencyInterval","type":["null","int"]},{"default":null,"name":"recalculationRestFrequencyNthDay","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationRestFrequencyWeekday","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationRestFrequencyOnDay","type":["null","int"]},{"default":null,"name":"recalculationCompoundingFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationCompoundingFrequencyInterval","type":["null","int"]},{"default":null,"name":"recalculationCompoundingFrequencyNthDay","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationCompoundingFrequencyWeekday","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"recalculationCompoundingFrequencyOnDay","type":["null","int"]},{"default":null,"name":"isArrearsBasedOnOriginalSchedule","type":["null","boolean"]},{"default":null,"name":"isCompoundingToBePostedAsTransaction","type":["null","boolean"]},{"default":null,"name":"preClosureInterestCalculationStrategy","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"allowCompoundingOnEod","type":["null","boolean"]},{"default":null,"name":"disallowInterestCalculationOnPastDue","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanRepaymentDueDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanRepaymentDueDataV1 {
    #[serde(rename = "loanId")]
    pub loan_id: serde_json::Value,
    #[serde(rename = "loanAccountNo")]
    pub loan_account_no: serde_json::Value,
    #[serde(rename = "loanExternalId")]
    pub loan_external_id: Option<String>,
    pub currency: Option<CurrencyDataV1>,
    pub installment: serde_json::Value,
    #[serde(rename = "pastDueAmount")]
    pub past_due_amount: Option<RepaymentPastDueDataV1>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
    pub originators: Option<Vec<OriginatorDetailsV1>>,
}

pub const LOAN_REPAYMENT_DUE_DATA_V1_AVSC: &str = r#"{"name":"LoanRepaymentDueDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"name":"loanId","type":["long"]},{"name":"loanAccountNo","type":["string"]},{"default":null,"name":"loanExternalId","type":["null","string"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"name":"installment","type":["org.apache.fineract.avro.loan.v1.RepaymentDueDataV1"]},{"default":null,"name":"pastDueAmount","type":["null","org.apache.fineract.avro.loan.v1.RepaymentPastDueDataV1"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]},{"default":null,"name":"originators","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.OriginatorDetailsV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanScheduleDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanScheduleDataV1 {
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "loanTermInDays")]
    pub loan_term_in_days: Option<i32>,
    #[serde(rename = "totalPrincipalDisbursed")]
    pub total_principal_disbursed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPrincipalExpected")]
    pub total_principal_expected: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPrincipalPaid")]
    pub total_principal_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalInterestCharged")]
    pub total_interest_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalFeeChargesCharged")]
    pub total_fee_charges_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPenaltyChargesCharged")]
    pub total_penalty_charges_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWaived")]
    pub total_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWrittenOff")]
    pub total_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalRepaymentExpected")]
    pub total_repayment_expected: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalRepayment")]
    pub total_repayment: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPaidInAdvance")]
    pub total_paid_in_advance: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPaidLate")]
    pub total_paid_late: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalOutstanding")]
    pub total_outstanding: Option<rust_decimal::Decimal>,
    pub periods: Option<Vec<LoanSchedulePeriodDataV1>>,
    #[serde(rename = "futurePeriods")]
    pub future_periods: Option<Vec<LoanSchedulePeriodDataV1>>,
}

pub const LOAN_SCHEDULE_DATA_V1_AVSC: &str = r#"{"name":"LoanScheduleDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"loanTermInDays","type":["null","int"]},{"default":null,"name":"totalPrincipalDisbursed","type":["null","bigdecimal"]},{"default":null,"name":"totalPrincipalExpected","type":["null","bigdecimal"]},{"default":null,"name":"totalPrincipalPaid","type":["null","bigdecimal"]},{"default":null,"name":"totalInterestCharged","type":["null","bigdecimal"]},{"default":null,"name":"totalFeeChargesCharged","type":["null","bigdecimal"]},{"default":null,"name":"totalPenaltyChargesCharged","type":["null","bigdecimal"]},{"default":null,"name":"totalWaived","type":["null","bigdecimal"]},{"default":null,"name":"totalWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"totalRepaymentExpected","type":["null","bigdecimal"]},{"default":null,"name":"totalRepayment","type":["null","bigdecimal"]},{"default":null,"name":"totalPaidInAdvance","type":["null","bigdecimal"]},{"default":null,"name":"totalPaidLate","type":["null","bigdecimal"]},{"default":null,"name":"totalOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"periods","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanSchedulePeriodDataV1"}]},{"default":null,"name":"futurePeriods","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanSchedulePeriodDataV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanSchedulePeriodDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanSchedulePeriodDataV1 {
    pub period: Option<i32>,
    #[serde(rename = "fromDate")]
    pub from_date: Option<String>,
    #[serde(rename = "dueDate")]
    pub due_date: Option<String>,
    #[serde(rename = "obligationsMetOnDate")]
    pub obligations_met_on_date: Option<String>,
    pub complete: Option<bool>,
    #[serde(rename = "daysInPeriod")]
    pub days_in_period: Option<i32>,
    #[serde(rename = "principalDisbursed")]
    pub principal_disbursed: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalOriginalDue")]
    pub principal_original_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalDue")]
    pub principal_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalPaid")]
    pub principal_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalWrittenOff")]
    pub principal_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalOutstanding")]
    pub principal_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalLoanBalanceOutstanding")]
    pub principal_loan_balance_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestOriginalDue")]
    pub interest_original_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestDue")]
    pub interest_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestPaid")]
    pub interest_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestWaived")]
    pub interest_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestWrittenOff")]
    pub interest_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestOutstanding")]
    pub interest_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesDue")]
    pub fee_charges_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesPaid")]
    pub fee_charges_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesWaived")]
    pub fee_charges_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesWrittenOff")]
    pub fee_charges_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesOutstanding")]
    pub fee_charges_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesDue")]
    pub penalty_charges_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesPaid")]
    pub penalty_charges_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesWaived")]
    pub penalty_charges_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesWrittenOff")]
    pub penalty_charges_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesOutstanding")]
    pub penalty_charges_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalOriginalDueForPeriod")]
    pub total_original_due_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalDueForPeriod")]
    pub total_due_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPaidForPeriod")]
    pub total_paid_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPaidInAdvanceForPeriod")]
    pub total_paid_in_advance_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPaidLateForPeriod")]
    pub total_paid_late_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWaivedForPeriod")]
    pub total_waived_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWrittenOffForPeriod")]
    pub total_written_off_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalOutstandingForPeriod")]
    pub total_outstanding_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalOverdue")]
    pub total_overdue: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalActualCostOfLoanForPeriod")]
    pub total_actual_cost_of_loan_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalInstallmentAmountForPeriod")]
    pub total_installment_amount_for_period: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCredits")]
    pub total_credits: Option<rust_decimal::Decimal>,
    #[serde(rename = "downPaymentPeriod")]
    pub down_payment_period: Option<bool>,
}

pub const LOAN_SCHEDULE_PERIOD_DATA_V1_AVSC: &str = r#"{"name":"LoanSchedulePeriodDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"period","type":["null","int"]},{"default":null,"name":"fromDate","type":["null","string"]},{"default":null,"name":"dueDate","type":["null","string"]},{"default":null,"name":"obligationsMetOnDate","type":["null","string"]},{"default":null,"name":"complete","type":["null","boolean"]},{"default":null,"name":"daysInPeriod","type":["null","int"]},{"default":null,"name":"principalDisbursed","type":["null","bigdecimal"]},{"default":null,"name":"principalOriginalDue","type":["null","bigdecimal"]},{"default":null,"name":"principalDue","type":["null","bigdecimal"]},{"default":null,"name":"principalPaid","type":["null","bigdecimal"]},{"default":null,"name":"principalWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"principalOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"principalLoanBalanceOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"interestOriginalDue","type":["null","bigdecimal"]},{"default":null,"name":"interestDue","type":["null","bigdecimal"]},{"default":null,"name":"interestPaid","type":["null","bigdecimal"]},{"default":null,"name":"interestWaived","type":["null","bigdecimal"]},{"default":null,"name":"interestWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"interestOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesDue","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesPaid","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesWaived","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesDue","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesPaid","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesWaived","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"totalOriginalDueForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalDueForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalPaidForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalPaidInAdvanceForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalPaidLateForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalWaivedForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalWrittenOffForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalOutstandingForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalOverdue","type":["null","bigdecimal"]},{"default":null,"name":"totalActualCostOfLoanForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalInstallmentAmountForPeriod","type":["null","bigdecimal"]},{"default":null,"name":"totalCredits","type":["null","bigdecimal"]},{"default":null,"name":"downPaymentPeriod","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanStatusEnumDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanStatusEnumDataV1 {
    pub id: Option<i32>,
    pub code: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "pendingApproval")]
    pub pending_approval: Option<bool>,
    #[serde(rename = "waitingForDisbursal")]
    pub waiting_for_disbursal: Option<bool>,
    pub active: Option<bool>,
    #[serde(rename = "closedObligationsMet")]
    pub closed_obligations_met: Option<bool>,
    #[serde(rename = "closedWrittenOff")]
    pub closed_written_off: Option<bool>,
    #[serde(rename = "closedRescheduled")]
    pub closed_rescheduled: Option<bool>,
    pub closed: Option<bool>,
    pub overpaid: Option<bool>,
}

pub const LOAN_STATUS_ENUM_DATA_V1_AVSC: &str = r#"{"name":"LoanStatusEnumDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"value","type":["null","string"]},{"default":null,"name":"pendingApproval","type":["null","boolean"]},{"default":null,"name":"waitingForDisbursal","type":["null","boolean"]},{"default":null,"name":"active","type":["null","boolean"]},{"default":null,"name":"closedObligationsMet","type":["null","boolean"]},{"default":null,"name":"closedWrittenOff","type":["null","boolean"]},{"default":null,"name":"closedRescheduled","type":["null","boolean"]},{"default":null,"name":"closed","type":["null","boolean"]},{"default":null,"name":"overpaid","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanSummaryDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanSummaryDataV1 {
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "principalDisbursed")]
    pub principal_disbursed: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalAdjustments")]
    pub principal_adjustments: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalPaid")]
    pub principal_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalWrittenOff")]
    pub principal_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalOutstanding")]
    pub principal_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalOverdue")]
    pub principal_overdue: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestCharged")]
    pub interest_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestPaid")]
    pub interest_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestWaived")]
    pub interest_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestWrittenOff")]
    pub interest_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestOutstanding")]
    pub interest_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestOverdue")]
    pub interest_overdue: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesCharged")]
    pub fee_charges_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesDueAtDisbursementCharged")]
    pub fee_charges_due_at_disbursement_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesPaid")]
    pub fee_charges_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesWaived")]
    pub fee_charges_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesWrittenOff")]
    pub fee_charges_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesOutstanding")]
    pub fee_charges_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesOverdue")]
    pub fee_charges_overdue: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesCharged")]
    pub penalty_charges_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesPaid")]
    pub penalty_charges_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesWaived")]
    pub penalty_charges_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesWrittenOff")]
    pub penalty_charges_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesOutstanding")]
    pub penalty_charges_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesOverdue")]
    pub penalty_charges_overdue: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalExpectedRepayment")]
    pub total_expected_repayment: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalRepayment")]
    pub total_repayment: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalExpectedCostOfLoan")]
    pub total_expected_cost_of_loan: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCostOfLoan")]
    pub total_cost_of_loan: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWaived")]
    pub total_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWrittenOff")]
    pub total_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalOutstanding")]
    pub total_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalOverdue")]
    pub total_overdue: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalRecovered")]
    pub total_recovered: Option<rust_decimal::Decimal>,
    #[serde(rename = "overdueSinceDate")]
    pub overdue_since_date: Option<String>,
    #[serde(rename = "writeoffReasonId")]
    pub writeoff_reason_id: Option<i64>,
    #[serde(rename = "writeoffReason")]
    pub writeoff_reason: Option<String>,
    #[serde(rename = "totalMerchantRefund")]
    pub total_merchant_refund: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalMerchantRefundReversed")]
    pub total_merchant_refund_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPayoutRefund")]
    pub total_payout_refund: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPayoutRefundReversed")]
    pub total_payout_refund_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalGoodwillCredit")]
    pub total_goodwill_credit: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalGoodwillCreditReversed")]
    pub total_goodwill_credit_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalChargeAdjustment")]
    pub total_charge_adjustment: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalChargeAdjustmentReversed")]
    pub total_charge_adjustment_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalChargeback")]
    pub total_chargeback: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCreditBalanceRefund")]
    pub total_credit_balance_refund: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCreditBalanceRefundReversed")]
    pub total_credit_balance_refund_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalRepaymentTransaction")]
    pub total_repayment_transaction: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalRepaymentTransactionReversed")]
    pub total_repayment_transaction_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalInterestPaymentWaiver")]
    pub total_interest_payment_waiver: Option<rust_decimal::Decimal>,
    #[serde(rename = "chargeOffReasonId")]
    pub charge_off_reason_id: Option<i64>,
    #[serde(rename = "chargeOffReason")]
    pub charge_off_reason: Option<String>,
    #[serde(rename = "totalUnpaidPayableDueInterest")]
    pub total_unpaid_payable_due_interest: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalUnpaidPayableNotDueInterest")]
    pub total_unpaid_payable_not_due_interest: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalInterestRefund")]
    pub total_interest_refund: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPrincipal")]
    pub total_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCapitalizedIncome")]
    pub total_capitalized_income: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCapitalizedIncomeAdjustment")]
    pub total_capitalized_income_adjustment: Option<rust_decimal::Decimal>,
}

pub const LOAN_SUMMARY_DATA_V1_AVSC: &str = r#"{"name":"LoanSummaryDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"principalDisbursed","type":["null","bigdecimal"]},{"default":null,"name":"principalAdjustments","type":["null","bigdecimal"]},{"default":null,"name":"principalPaid","type":["null","bigdecimal"]},{"default":null,"name":"principalWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"principalOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"principalOverdue","type":["null","bigdecimal"]},{"default":null,"name":"interestCharged","type":["null","bigdecimal"]},{"default":null,"name":"interestPaid","type":["null","bigdecimal"]},{"default":null,"name":"interestWaived","type":["null","bigdecimal"]},{"default":null,"name":"interestWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"interestOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"interestOverdue","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesCharged","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesDueAtDisbursementCharged","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesPaid","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesWaived","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesOverdue","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesCharged","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesPaid","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesWaived","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesOverdue","type":["null","bigdecimal"]},{"default":null,"name":"totalExpectedRepayment","type":["null","bigdecimal"]},{"default":null,"name":"totalRepayment","type":["null","bigdecimal"]},{"default":null,"name":"totalExpectedCostOfLoan","type":["null","bigdecimal"]},{"default":null,"name":"totalCostOfLoan","type":["null","bigdecimal"]},{"default":null,"name":"totalWaived","type":["null","bigdecimal"]},{"default":null,"name":"totalWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"totalOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"totalOverdue","type":["null","bigdecimal"]},{"default":null,"name":"totalRecovered","type":["null","bigdecimal"]},{"default":null,"name":"overdueSinceDate","type":["null","string"]},{"default":null,"name":"writeoffReasonId","type":["null","long"]},{"default":null,"name":"writeoffReason","type":["null","string"]},{"default":null,"name":"totalMerchantRefund","type":["null","bigdecimal"]},{"default":null,"name":"totalMerchantRefundReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalPayoutRefund","type":["null","bigdecimal"]},{"default":null,"name":"totalPayoutRefundReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalGoodwillCredit","type":["null","bigdecimal"]},{"default":null,"name":"totalGoodwillCreditReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalChargeAdjustment","type":["null","bigdecimal"]},{"default":null,"name":"totalChargeAdjustmentReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalChargeback","type":["null","bigdecimal"]},{"default":null,"name":"totalCreditBalanceRefund","type":["null","bigdecimal"]},{"default":null,"name":"totalCreditBalanceRefundReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalRepaymentTransaction","type":["null","bigdecimal"]},{"default":null,"name":"totalRepaymentTransactionReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalInterestPaymentWaiver","type":["null","bigdecimal"]},{"default":null,"name":"chargeOffReasonId","type":["null","long"]},{"default":null,"name":"chargeOffReason","type":["null","string"]},{"default":null,"name":"totalUnpaidPayableDueInterest","type":["null","bigdecimal"]},{"default":null,"name":"totalUnpaidPayableNotDueInterest","type":["null","bigdecimal"]},{"default":null,"name":"totalInterestRefund","type":["null","bigdecimal"]},{"default":null,"name":"totalPrincipal","type":["null","bigdecimal"]},{"default":null,"name":"totalCapitalizedIncome","type":["null","bigdecimal"]},{"default":null,"name":"totalCapitalizedIncomeAdjustment","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanTermVariationsDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanTermVariationsDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "termType")]
    pub term_type: Option<EnumOptionDataV1>,
    #[serde(rename = "termVariationApplicableFrom")]
    pub term_variation_applicable_from: Option<String>,
    #[serde(rename = "decimalValue")]
    pub decimal_value: Option<rust_decimal::Decimal>,
    #[serde(rename = "dateValue")]
    pub date_value: Option<String>,
    #[serde(rename = "isSpecificToInstallment")]
    pub is_specific_to_installment: Option<bool>,
    #[serde(rename = "isProcessed")]
    pub is_processed: Option<bool>,
}

pub const LOAN_TERM_VARIATIONS_DATA_V1_AVSC: &str = r#"{"name":"LoanTermVariationsDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"termType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"termVariationApplicableFrom","type":["null","string"]},{"default":null,"name":"decimalValue","type":["null","bigdecimal"]},{"default":null,"name":"dateValue","type":["null","string"]},{"default":null,"name":"isSpecificToInstallment","type":["null","boolean"]},{"default":null,"name":"isProcessed","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanTransactionAdjustmentDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanTransactionAdjustmentDataV1 {
    #[serde(rename = "transactionToAdjust")]
    pub transaction_to_adjust: Option<LoanTransactionDataV1>,
    #[serde(rename = "newTransactionDetail")]
    pub new_transaction_detail: Option<LoanTransactionDataV1>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
}

pub const LOAN_TRANSACTION_ADJUSTMENT_DATA_V1_AVSC: &str = r#"{"name":"LoanTransactionAdjustmentDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"transactionToAdjust","type":["null","org.apache.fineract.avro.loan.v1.LoanTransactionDataV1"]},{"default":null,"name":"newTransactionDetail","type":["null","org.apache.fineract.avro.loan.v1.LoanTransactionDataV1"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanTransactionDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanTransactionDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "loanId")]
    pub loan_id: Option<i64>,
    #[serde(rename = "externalLoanId")]
    pub external_loan_id: Option<String>,
    #[serde(rename = "officeId")]
    pub office_id: Option<i64>,
    #[serde(rename = "officeName")]
    pub office_name: Option<String>,
    pub r#type: Option<LoanTransactionEnumDataV1>,
    pub date: Option<String>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "paymentDetailData")]
    pub payment_detail_data: Option<PaymentDetailDataV1>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "netDisbursalAmount")]
    pub net_disbursal_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalPortion")]
    pub principal_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestPortion")]
    pub interest_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesPortion")]
    pub fee_charges_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesPortion")]
    pub penalty_charges_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "overpaymentPortion")]
    pub overpayment_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "unrecognizedIncomePortion")]
    pub unrecognized_income_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "externalOwnerId")]
    pub external_owner_id: Option<String>,
    pub transfer: Option<AccountTransferDataV1>,
    #[serde(rename = "fixedEmiAmount")]
    pub fixed_emi_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "outstandingLoanBalance")]
    pub outstanding_loan_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    #[serde(rename = "manuallyReversed")]
    pub manually_reversed: Option<bool>,
    #[serde(rename = "possibleNextRepaymentDate")]
    pub possible_next_repayment_date: Option<String>,
    #[serde(rename = "loanChargePaidByList")]
    pub loan_charge_paid_by_list: Option<Vec<LoanChargePaidByDataV1>>,
    #[serde(rename = "transactionRelations")]
    pub transaction_relations: Option<Vec<LoanTransactionRelationDataV1>>,
    /// Only used for Charge-off transaction
    #[serde(rename = "unpaidCharges")]
    pub unpaid_charges: Option<Vec<UnpaidChargeDataV1>>,
    #[serde(rename = "reversedOnDate")]
    pub reversed_on_date: Option<String>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
    pub reversed: Option<bool>,
    pub classification: Option<CodeValueDataV1>,
    /// List of originators attached to the parent loan for revenue sharing
    pub originators: Option<Vec<OriginatorDetailsV1>>,
    /// Optional flags associated with this transaction event
    pub flags: Option<LoanTransactionFlagsDataV1>,
}

pub const LOAN_TRANSACTION_DATA_V1_AVSC: &str = r#"{"name":"LoanTransactionDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"loanId","type":["null","long"]},{"default":null,"name":"externalLoanId","type":["null","string"]},{"default":null,"name":"officeId","type":["null","long"]},{"default":null,"name":"officeName","type":["null","string"]},{"default":null,"name":"type","type":["null","org.apache.fineract.avro.loan.v1.LoanTransactionEnumDataV1"]},{"default":null,"name":"date","type":["null","string"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"paymentDetailData","type":["null","org.apache.fineract.avro.payment.v1.PaymentDetailDataV1"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"netDisbursalAmount","type":["null","bigdecimal"]},{"default":null,"name":"principalPortion","type":["null","bigdecimal"]},{"default":null,"name":"interestPortion","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesPortion","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesPortion","type":["null","bigdecimal"]},{"default":null,"name":"overpaymentPortion","type":["null","bigdecimal"]},{"default":null,"name":"unrecognizedIncomePortion","type":["null","bigdecimal"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"externalOwnerId","type":["null","string"]},{"default":null,"name":"transfer","type":["null","org.apache.fineract.avro.savings.v1.AccountTransferDataV1"]},{"default":null,"name":"fixedEmiAmount","type":["null","bigdecimal"]},{"default":null,"name":"outstandingLoanBalance","type":["null","bigdecimal"]},{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"manuallyReversed","type":["null","boolean"]},{"default":null,"name":"possibleNextRepaymentDate","type":["null","string"]},{"default":null,"name":"loanChargePaidByList","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanChargePaidByDataV1"}]},{"default":null,"name":"transactionRelations","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.LoanTransactionRelationDataV1"}]},{"default":null,"name":"unpaidCharges","doc":"Only used for Charge-off transaction","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.UnpaidChargeDataV1"}]},{"default":null,"name":"reversedOnDate","type":["null","string"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]},{"default":null,"name":"reversed","type":["null","boolean"]},{"default":null,"name":"classification","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"originators","doc":"List of originators attached to the parent loan for revenue sharing","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.OriginatorDetailsV1"}]},{"default":null,"name":"flags","doc":"Optional flags associated with this transaction event","type":["null","org.apache.fineract.avro.loan.v1.LoanTransactionFlagsDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanTransactionEnumDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanTransactionEnumDataV1 {
    pub id: Option<i32>,
    pub code: Option<String>,
    pub value: Option<String>,
    pub disbursement: Option<bool>,
    #[serde(rename = "repaymentAtDisbursement")]
    pub repayment_at_disbursement: Option<bool>,
    pub repayment: Option<bool>,
    #[serde(rename = "merchantIssuedRefund")]
    pub merchant_issued_refund: Option<bool>,
    #[serde(rename = "payoutRefund")]
    pub payout_refund: Option<bool>,
    #[serde(rename = "goodwillCredit")]
    pub goodwill_credit: Option<bool>,
    #[serde(rename = "chargeRefund")]
    pub charge_refund: Option<bool>,
    pub contra: Option<bool>,
    #[serde(rename = "waiveInterest")]
    pub waive_interest: Option<bool>,
    #[serde(rename = "waiveCharges")]
    pub waive_charges: Option<bool>,
    pub accrual: Option<bool>,
    #[serde(rename = "writeOff")]
    pub write_off: Option<bool>,
    #[serde(rename = "recoveryRepayment")]
    pub recovery_repayment: Option<bool>,
    #[serde(rename = "initiateTransfer")]
    pub initiate_transfer: Option<bool>,
    #[serde(rename = "approveTransfer")]
    pub approve_transfer: Option<bool>,
    #[serde(rename = "withdrawTransfer")]
    pub withdraw_transfer: Option<bool>,
    #[serde(rename = "rejectTransfer")]
    pub reject_transfer: Option<bool>,
    #[serde(rename = "chargePayment")]
    pub charge_payment: Option<bool>,
    pub refund: Option<bool>,
    #[serde(rename = "refundForActiveLoans")]
    pub refund_for_active_loans: Option<bool>,
    #[serde(rename = "creditBalanceRefund")]
    pub credit_balance_refund: Option<bool>,
    pub chargeback: Option<bool>,
    #[serde(rename = "interestPaymentWaiver")]
    pub interest_payment_waiver: Option<bool>,
    #[serde(rename = "chargeAdjustment")]
    pub charge_adjustment: Option<bool>,
    pub chargeoff: Option<bool>,
    #[serde(rename = "downPayment")]
    pub down_payment: Option<bool>,
    #[serde(rename = "reAge")]
    pub re_age: Option<bool>,
    #[serde(rename = "reAmortize")]
    pub re_amortize: Option<bool>,
    #[serde(rename = "accrualActivity")]
    pub accrual_activity: Option<bool>,
    #[serde(rename = "interestRefund")]
    pub interest_refund: Option<bool>,
    #[serde(rename = "accrualAdjustment")]
    pub accrual_adjustment: Option<bool>,
    #[serde(rename = "capitalizedIncome")]
    pub capitalized_income: Option<bool>,
    #[serde(rename = "capitalizedIncomeAmortization")]
    pub capitalized_income_amortization: Option<bool>,
    #[serde(rename = "capitalizedIncomeAdjustment")]
    pub capitalized_income_adjustment: Option<bool>,
    #[serde(rename = "capitalizedIncomeAmortizationAdjustment")]
    pub capitalized_income_amortization_adjustment: Option<bool>,
    #[serde(rename = "contractTermination")]
    pub contract_termination: Option<bool>,
    #[serde(rename = "buyDownFee")]
    pub buy_down_fee: Option<bool>,
    #[serde(rename = "buyDownFeeAdjustment")]
    pub buy_down_fee_adjustment: Option<bool>,
    #[serde(rename = "buyDownFeeAmortization")]
    pub buy_down_fee_amortization: Option<bool>,
    #[serde(rename = "buyDownFeeAmortizationAdjustment")]
    pub buy_down_fee_amortization_adjustment: Option<bool>,
    #[serde(rename = "loanWithdrawal")]
    pub loan_withdrawal: Option<bool>,
}

pub const LOAN_TRANSACTION_ENUM_DATA_V1_AVSC: &str = r#"{"name":"LoanTransactionEnumDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"value","type":["null","string"]},{"default":null,"name":"disbursement","type":["null","boolean"]},{"default":null,"name":"repaymentAtDisbursement","type":["null","boolean"]},{"default":null,"name":"repayment","type":["null","boolean"]},{"default":null,"name":"merchantIssuedRefund","type":["null","boolean"]},{"default":null,"name":"payoutRefund","type":["null","boolean"]},{"default":null,"name":"goodwillCredit","type":["null","boolean"]},{"default":null,"name":"chargeRefund","type":["null","boolean"]},{"default":null,"name":"contra","type":["null","boolean"]},{"default":null,"name":"waiveInterest","type":["null","boolean"]},{"default":null,"name":"waiveCharges","type":["null","boolean"]},{"default":null,"name":"accrual","type":["null","boolean"]},{"default":null,"name":"writeOff","type":["null","boolean"]},{"default":null,"name":"recoveryRepayment","type":["null","boolean"]},{"default":null,"name":"initiateTransfer","type":["null","boolean"]},{"default":null,"name":"approveTransfer","type":["null","boolean"]},{"default":null,"name":"withdrawTransfer","type":["null","boolean"]},{"default":null,"name":"rejectTransfer","type":["null","boolean"]},{"default":null,"name":"chargePayment","type":["null","boolean"]},{"default":null,"name":"refund","type":["null","boolean"]},{"default":null,"name":"refundForActiveLoans","type":["null","boolean"]},{"default":null,"name":"creditBalanceRefund","type":["null","boolean"]},{"default":null,"name":"chargeback","type":["null","boolean"]},{"default":null,"name":"interestPaymentWaiver","type":["null","boolean"]},{"default":null,"name":"chargeAdjustment","type":["null","boolean"]},{"default":null,"name":"chargeoff","type":["null","boolean"]},{"default":null,"name":"downPayment","type":["null","boolean"]},{"default":null,"name":"reAge","type":["null","boolean"]},{"default":null,"name":"reAmortize","type":["null","boolean"]},{"default":null,"name":"accrualActivity","type":["null","boolean"]},{"default":null,"name":"interestRefund","type":["null","boolean"]},{"default":null,"name":"accrualAdjustment","type":["null","boolean"]},{"default":null,"name":"capitalizedIncome","type":["null","boolean"]},{"default":null,"name":"capitalizedIncomeAmortization","type":["null","boolean"]},{"default":null,"name":"capitalizedIncomeAdjustment","type":["null","boolean"]},{"default":null,"name":"capitalizedIncomeAmortizationAdjustment","type":["null","boolean"]},{"default":null,"name":"contractTermination","type":["null","boolean"]},{"default":null,"name":"buyDownFee","type":["null","boolean"]},{"default":null,"name":"buyDownFeeAdjustment","type":["null","boolean"]},{"default":null,"name":"buyDownFeeAmortization","type":["null","boolean"]},{"default":null,"name":"buyDownFeeAmortizationAdjustment","type":["null","boolean"]},{"default":null,"name":"loanWithdrawal","type":["null","boolean"]}]}"#;

/// Optional flags associated with a loan transaction event (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanTransactionFlagsDataV1 {
    /// True if the transaction changed the number of loan repayment terms (installments excluding downpayment and additional)
    #[serde(rename = "changedTerms")]
    pub changed_terms: Option<bool>,
}

pub const LOAN_TRANSACTION_FLAGS_DATA_V1_AVSC: &str = r#"{"name":"LoanTransactionFlagsDataV1","namespace":"org.apache.fineract.avro.loan.v1","doc":"Optional flags associated with a loan transaction event","type":"record","fields":[{"default":null,"name":"changedTerms","doc":"True if the transaction changed the number of loan repayment terms (installments excluding downpayment and additional)","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.LoanTransactionRelationDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanTransactionRelationDataV1 {
    #[serde(rename = "fromLoanTransaction")]
    pub from_loan_transaction: i64,
    #[serde(rename = "toLoanTransaction")]
    pub to_loan_transaction: Option<i64>,
    #[serde(rename = "toLoanCharge")]
    pub to_loan_charge: Option<i64>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "relationType")]
    pub relation_type: String,
    #[serde(rename = "paymentType")]
    pub payment_type: Option<String>,
}

pub const LOAN_TRANSACTION_RELATION_DATA_V1_AVSC: &str = r#"{"name":"LoanTransactionRelationDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"name":"fromLoanTransaction","type":"long"},{"default":null,"name":"toLoanTransaction","type":["null","long"]},{"default":null,"name":"toLoanCharge","type":["null","long"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"name":"relationType","type":"string"},{"default":null,"name":"paymentType","type":["null","string"]}]}"#;

/// Loan originator details for revenue sharing and reporting (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginatorDetailsV1 {
    /// Internal originator ID
    pub id: Option<i64>,
    /// Unique external identifier (Revenue Share ID)
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    /// Originator name
    pub name: Option<String>,
    /// Originator status: ACTIVE, PENDING, or INACTIVE
    pub status: Option<String>,
    /// Code value for originator type (MERCHANT, BROKER, AFFILIATE, PLATFORM)
    #[serde(rename = "originatorType")]
    pub originator_type: Option<CodeValueDataV1>,
    /// Code value for channel type (ONLINE, IN_STORE, API, AGGREGATOR)
    #[serde(rename = "channelType")]
    pub channel_type: Option<CodeValueDataV1>,
}

pub const ORIGINATOR_DETAILS_V1_AVSC: &str = r#"{"name":"OriginatorDetailsV1","namespace":"org.apache.fineract.avro.loan.v1","doc":"Loan originator details for revenue sharing and reporting","type":"record","fields":[{"default":null,"name":"id","doc":"Internal originator ID","type":["null","long"]},{"default":null,"name":"externalId","doc":"Unique external identifier (Revenue Share ID)","type":["null","string"]},{"default":null,"name":"name","doc":"Originator name","type":["null","string"]},{"default":null,"name":"status","doc":"Originator status: ACTIVE, PENDING, or INACTIVE","type":["null","string"]},{"default":null,"name":"originatorType","doc":"Code value for originator type (MERCHANT, BROKER, AFFILIATE, PLATFORM)","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"channelType","doc":"Code value for channel type (ONLINE, IN_STORE, API, AGGREGATOR)","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.RepaymentDueDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepaymentDueDataV1 {
    #[serde(rename = "installmentNumber")]
    pub installment_number: Option<i32>,
    #[serde(rename = "installmentDueDate")]
    pub installment_due_date: Option<String>,
    #[serde(rename = "principalAmountDue")]
    pub principal_amount_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestAmountDue")]
    pub interest_amount_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargeAmountDue")]
    pub fee_charge_amount_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargeAmountDue")]
    pub penalty_charge_amount_due: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalAmountDue")]
    pub total_amount_due: Option<rust_decimal::Decimal>,
}

pub const REPAYMENT_DUE_DATA_V1_AVSC: &str = r#"{"name":"RepaymentDueDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"installmentNumber","type":["null","int"]},{"default":null,"name":"installmentDueDate","type":["null","string"]},{"default":null,"name":"principalAmountDue","type":["null","bigdecimal"]},{"default":null,"name":"interestAmountDue","type":["null","bigdecimal"]},{"default":null,"name":"feeChargeAmountDue","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargeAmountDue","type":["null","bigdecimal"]},{"default":null,"name":"totalAmountDue","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.RepaymentPastDueDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepaymentPastDueDataV1 {
    #[serde(rename = "totalAmount")]
    pub total_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalAmount")]
    pub principal_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestAmount")]
    pub interest_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeAmount")]
    pub fee_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyAmount")]
    pub penalty_amount: Option<rust_decimal::Decimal>,
}

pub const REPAYMENT_PAST_DUE_DATA_V1_AVSC: &str = r#"{"name":"RepaymentPastDueDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"default":null,"name":"totalAmount","type":["null","bigdecimal"]},{"default":null,"name":"principalAmount","type":["null","bigdecimal"]},{"default":null,"name":"interestAmount","type":["null","bigdecimal"]},{"default":null,"name":"feeAmount","type":["null","bigdecimal"]},{"default":null,"name":"penaltyAmount","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.loan.v1.UnpaidChargeDataV1` (Namespace: `org.apache.fineract.avro.loan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnpaidChargeDataV1 {
    #[serde(rename = "chargeId")]
    pub charge_id: i64,
    #[serde(rename = "chargeName")]
    pub charge_name: String,
    #[serde(rename = "outstandingAmount")]
    pub outstanding_amount: rust_decimal::Decimal,
}

pub const UNPAID_CHARGE_DATA_V1_AVSC: &str = r#"{"name":"UnpaidChargeDataV1","namespace":"org.apache.fineract.avro.loan.v1","type":"record","fields":[{"name":"chargeId","type":"long"},{"name":"chargeName","type":"string"},{"name":"outstandingAmount","type":"bigdecimal"}]}"#;

/// Avro schema `org.apache.fineract.avro.office.v1.OfficeDataV1` (Namespace: `org.apache.fineract.avro.office.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficeDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "nameDecorated")]
    pub name_decorated: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "openingDate")]
    pub opening_date: Option<String>,
    pub hierarchy: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<i64>,
    #[serde(rename = "parentName")]
    pub parent_name: Option<String>,
    #[serde(rename = "allowedParents")]
    pub allowed_parents: Option<Vec<OfficeDataV1>>,
}

pub const OFFICE_DATA_V1_AVSC: &str = r#"{"name":"OfficeDataV1","namespace":"org.apache.fineract.avro.office.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"nameDecorated","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"openingDate","type":["null","string"]},{"default":null,"name":"hierarchy","type":["null","string"]},{"default":null,"name":"parentId","type":["null","long"]},{"default":null,"name":"parentName","type":["null","string"]},{"default":null,"name":"allowedParents","type":["null",{"type":"array","items":"org.apache.fineract.avro.office.v1.OfficeDataV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.payment.v1.PaymentDetailDataV1` (Namespace: `org.apache.fineract.avro.payment.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentDetailDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "paymentType")]
    pub payment_type: Option<PaymentTypeDataV1>,
    #[serde(rename = "accountNumber")]
    pub account_number: Option<String>,
    #[serde(rename = "checkNumber")]
    pub check_number: Option<String>,
    #[serde(rename = "routingCode")]
    pub routing_code: Option<String>,
    #[serde(rename = "receiptNumber")]
    pub receipt_number: Option<String>,
    #[serde(rename = "bankNumber")]
    pub bank_number: Option<String>,
}

pub const PAYMENT_DETAIL_DATA_V1_AVSC: &str = r#"{"name":"PaymentDetailDataV1","namespace":"org.apache.fineract.avro.payment.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"paymentType","type":["null","org.apache.fineract.avro.payment.v1.PaymentTypeDataV1"]},{"default":null,"name":"accountNumber","type":["null","string"]},{"default":null,"name":"checkNumber","type":["null","string"]},{"default":null,"name":"routingCode","type":["null","string"]},{"default":null,"name":"receiptNumber","type":["null","string"]},{"default":null,"name":"bankNumber","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.payment.v1.PaymentTypeDataV1` (Namespace: `org.apache.fineract.avro.payment.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentTypeDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "isCashPayment")]
    pub is_cash_payment: Option<bool>,
    pub position: Option<i64>,
    #[serde(rename = "codeName")]
    pub code_name: Option<String>,
    #[serde(rename = "isSystemDefined")]
    pub is_system_defined: Option<bool>,
}

pub const PAYMENT_TYPE_DATA_V1_AVSC: &str = r#"{"name":"PaymentTypeDataV1","namespace":"org.apache.fineract.avro.payment.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"description","type":["null","string"]},{"default":null,"name":"isCashPayment","type":["null","boolean"]},{"default":null,"name":"position","type":["null","long"]},{"default":null,"name":"codeName","type":["null","string"]},{"default":null,"name":"isSystemDefined","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.portfolio.v1.ChargeDataV1` (Namespace: `org.apache.fineract.avro.portfolio.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargeDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub active: Option<bool>,
    pub penalty: Option<bool>,
    #[serde(rename = "freeWithdrawal")]
    pub free_withdrawal: Option<bool>,
    #[serde(rename = "freeWithdrawalChargeFrequency")]
    pub free_withdrawal_charge_frequency: Option<i32>,
    #[serde(rename = "restartFrequency")]
    pub restart_frequency: Option<i32>,
    #[serde(rename = "restartFrequencyEnum")]
    pub restart_frequency_enum: Option<i32>,
    #[serde(rename = "isPaymentType")]
    pub is_payment_type: Option<bool>,
    #[serde(rename = "paymentTypeOptions")]
    pub payment_type_options: Option<PaymentTypeDataV1>,
    pub currency: Option<CurrencyDataV1>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "chargeTimeType")]
    pub charge_time_type: Option<EnumOptionDataV1>,
    #[serde(rename = "chargeAppliesTo")]
    pub charge_applies_to: Option<EnumOptionDataV1>,
    #[serde(rename = "chargeCalculationType")]
    pub charge_calculation_type: Option<EnumOptionDataV1>,
    #[serde(rename = "chargePaymentMode")]
    pub charge_payment_mode: Option<EnumOptionDataV1>,
    #[serde(rename = "feeOnMonthDay")]
    pub fee_on_month_day: Option<String>,
    #[serde(rename = "feeInterval")]
    pub fee_interval: Option<i32>,
    #[serde(rename = "minCap")]
    pub min_cap: Option<rust_decimal::Decimal>,
    #[serde(rename = "maxCap")]
    pub max_cap: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeFrequency")]
    pub fee_frequency: Option<EnumOptionDataV1>,
    #[serde(rename = "incomeOrLiabilityAccount")]
    pub income_or_liability_account: Option<GLAccountDataV1>,
}

pub const CHARGE_DATA_V1_AVSC: &str = r#"{"name":"ChargeDataV1","namespace":"org.apache.fineract.avro.portfolio.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"active","type":["null","boolean"]},{"default":null,"name":"penalty","type":["null","boolean"]},{"default":null,"name":"freeWithdrawal","type":["null","boolean"]},{"default":null,"name":"freeWithdrawalChargeFrequency","type":["null","int"]},{"default":null,"name":"restartFrequency","type":["null","int"]},{"default":null,"name":"restartFrequencyEnum","type":["null","int"]},{"default":null,"name":"isPaymentType","type":["null","boolean"]},{"default":null,"name":"paymentTypeOptions","type":["null","org.apache.fineract.avro.payment.v1.PaymentTypeDataV1"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"chargeTimeType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"chargeAppliesTo","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"chargeCalculationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"chargePaymentMode","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"feeOnMonthDay","type":["null","string"]},{"default":null,"name":"feeInterval","type":["null","int"]},{"default":null,"name":"minCap","type":["null","bigdecimal"]},{"default":null,"name":"maxCap","type":["null","bigdecimal"]},{"default":null,"name":"feeFrequency","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"incomeOrLiabilityAccount","type":["null","org.apache.fineract.avro.gl.v1.GLAccountDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.portfolio.v1.PortfolioAccountDataV1` (Namespace: `org.apache.fineract.avro.portfolio.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioAccountDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "groupId")]
    pub group_id: Option<i64>,
    #[serde(rename = "groupName")]
    pub group_name: Option<String>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i64>,
    #[serde(rename = "clientName")]
    pub client_name: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<i64>,
    #[serde(rename = "productName")]
    pub product_name: Option<String>,
    #[serde(rename = "fieldOfficerId")]
    pub field_officer_id: Option<i64>,
    #[serde(rename = "fieldOfficerName")]
    pub field_officer_name: Option<String>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "amtForTransfer")]
    pub amt_for_transfer: Option<rust_decimal::Decimal>,
}

pub const PORTFOLIO_ACCOUNT_DATA_V1_AVSC: &str = r#"{"name":"PortfolioAccountDataV1","namespace":"org.apache.fineract.avro.portfolio.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"groupId","type":["null","long"]},{"default":null,"name":"groupName","type":["null","string"]},{"default":null,"name":"clientId","type":["null","long"]},{"default":null,"name":"clientName","type":["null","string"]},{"default":null,"name":"productId","type":["null","long"]},{"default":null,"name":"productName","type":["null","string"]},{"default":null,"name":"fieldOfficerId","type":["null","long"]},{"default":null,"name":"fieldOfficerName","type":["null","string"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"amtForTransfer","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.portfolio.v1.RateDataV1` (Namespace: `org.apache.fineract.avro.portfolio.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub percentage: Option<rust_decimal::Decimal>,
    #[serde(rename = "productApply")]
    pub product_apply: Option<EnumOptionDataV1>,
    pub active: Option<bool>,
}

pub const RATE_DATA_V1_AVSC: &str = r#"{"name":"RateDataV1","namespace":"org.apache.fineract.avro.portfolio.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"percentage","type":["null","bigdecimal"]},{"default":null,"name":"productApply","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"active","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.recurringdeposit.v1.RecurringDepositAccountDataV1` (Namespace: `org.apache.fineract.avro.recurringdeposit.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringDepositAccountDataV1 {
    pub id: Option<i32>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "groupId")]
    pub group_id: Option<i32>,
    #[serde(rename = "groupName")]
    pub group_name: Option<String>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i32>,
    #[serde(rename = "clientName")]
    pub client_name: Option<String>,
    #[serde(rename = "depositProductId")]
    pub deposit_product_id: Option<i32>,
    #[serde(rename = "depositProductName")]
    pub deposit_product_name: Option<String>,
    #[serde(rename = "fieldOfficerId")]
    pub field_officer_id: Option<i32>,
    #[serde(rename = "fieldOfficerName")]
    pub field_officer_name: Option<String>,
    pub status: Option<SavingsAccountStatusEnumDataV1>,
    pub timeline: Option<SavingsAccountApplicationTimelineDataV1>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "nominalAnnualInterestRate")]
    pub nominal_annual_interest_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestCompoundingPeriodType")]
    pub interest_compounding_period_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestPostingPeriodType")]
    pub interest_posting_period_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestCalculationType")]
    pub interest_calculation_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestCalculationDaysInYearType")]
    pub interest_calculation_days_in_year_type: Option<EnumOptionDataV1>,
    #[serde(rename = "minRequiredOpeningBalance")]
    pub min_required_opening_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "lockinPeriodFrequency")]
    pub lockin_period_frequency: Option<i32>,
    #[serde(rename = "lockinPeriodFrequencyType")]
    pub lockin_period_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "withdrawalFeeForTransfers")]
    pub withdrawal_fee_for_transfers: Option<bool>,
    #[serde(rename = "depositType")]
    pub deposit_type: Option<EnumOptionDataV1>,
    #[serde(rename = "minBalanceForInterestCalculation")]
    pub min_balance_for_interest_calculation: Option<rust_decimal::Decimal>,
    #[serde(rename = "withHoldTax")]
    pub with_hold_tax: Option<bool>,
    pub summary: Option<SavingsAccountSummaryDataV1>,
    pub transactions: Option<Vec<SavingsAccountTransactionDataV1>>,
    pub charges: Option<Vec<SavingsAccountChargeDataV1>>,
    #[serde(rename = "preClosurePenalApplicable")]
    pub pre_closure_penal_applicable: Option<bool>,
    #[serde(rename = "preClosurePenalInterest")]
    pub pre_closure_penal_interest: Option<rust_decimal::Decimal>,
    #[serde(rename = "preClosurePenalInterestOnType")]
    pub pre_closure_penal_interest_on_type: Option<EnumOptionDataV1>,
    #[serde(rename = "minDepositTerm")]
    pub min_deposit_term: Option<i32>,
    #[serde(rename = "maxDepositTerm")]
    pub max_deposit_term: Option<i32>,
    #[serde(rename = "minDepositTermType")]
    pub min_deposit_term_type: Option<EnumOptionDataV1>,
    #[serde(rename = "maxDepositTermType")]
    pub max_deposit_term_type: Option<EnumOptionDataV1>,
    #[serde(rename = "inMultiplesOfDepositTerm")]
    pub in_multiples_of_deposit_term: Option<i32>,
    #[serde(rename = "inMultiplesOfDepositTermType")]
    pub in_multiples_of_deposit_term_type: Option<EnumOptionDataV1>,
    #[serde(rename = "depositAmount")]
    pub deposit_amount: Option<f32>,
    #[serde(rename = "maturityAmount")]
    pub maturity_amount: Option<f32>,
    #[serde(rename = "maturityDate")]
    pub maturity_date: Option<String>,
    #[serde(rename = "depositPeriod")]
    pub deposit_period: Option<i32>,
    #[serde(rename = "depositPeriodFrequency")]
    pub deposit_period_frequency: Option<EnumOptionDataV1>,
    #[serde(rename = "mandatoryRecommendedDepositAmount")]
    pub mandatory_recommended_deposit_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalOverdueAmount")]
    pub total_overdue_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "noOfOverdueInstallments")]
    pub no_of_overdue_installments: Option<i32>,
    #[serde(rename = "isMandatoryDeposit")]
    pub is_mandatory_deposit: Option<bool>,
    #[serde(rename = "allowWithdrawal")]
    pub allow_withdrawal: Option<bool>,
    #[serde(rename = "adjustAdvanceTowardsFuturePayments")]
    pub adjust_advance_towards_future_payments: Option<bool>,
    #[serde(rename = "expectedFirstDepositOnDate")]
    pub expected_first_deposit_on_date: Option<String>,
    #[serde(rename = "isCalendarInherited")]
    pub is_calendar_inherited: Option<bool>,
    #[serde(rename = "recurringFrequency")]
    pub recurring_frequency: Option<i32>,
    #[serde(rename = "recurringFrequencyType")]
    pub recurring_frequency_type: Option<EnumOptionDataV1>,
}

pub const RECURRING_DEPOSIT_ACCOUNT_DATA_V1_AVSC: &str = r#"{"name":"RecurringDepositAccountDataV1","namespace":"org.apache.fineract.avro.recurringdeposit.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"groupId","type":["null","int"]},{"default":null,"name":"groupName","type":["null","string"]},{"default":null,"name":"clientId","type":["null","int"]},{"default":null,"name":"clientName","type":["null","string"]},{"default":null,"name":"depositProductId","type":["null","int"]},{"default":null,"name":"depositProductName","type":["null","string"]},{"default":null,"name":"fieldOfficerId","type":["null","int"]},{"default":null,"name":"fieldOfficerName","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountStatusEnumDataV1"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountApplicationTimelineDataV1"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"nominalAnnualInterestRate","type":["null","bigdecimal"]},{"default":null,"name":"interestCompoundingPeriodType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestPostingPeriodType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestCalculationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestCalculationDaysInYearType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minRequiredOpeningBalance","type":["null","bigdecimal"]},{"default":null,"name":"lockinPeriodFrequency","type":["null","int"]},{"default":null,"name":"lockinPeriodFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"withdrawalFeeForTransfers","type":["null","boolean"]},{"default":null,"name":"depositType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minBalanceForInterestCalculation","type":["null","bigdecimal"]},{"default":null,"name":"withHoldTax","type":["null","boolean"]},{"default":null,"name":"summary","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountSummaryDataV1"]},{"default":null,"name":"transactions","type":["null",{"type":"array","items":"org.apache.fineract.avro.savings.v1.SavingsAccountTransactionDataV1"}]},{"default":null,"name":"charges","type":["null",{"type":"array","items":"org.apache.fineract.avro.savings.v1.SavingsAccountChargeDataV1"}]},{"default":null,"name":"preClosurePenalApplicable","type":["null","boolean"]},{"default":null,"name":"preClosurePenalInterest","type":["null","bigdecimal"]},{"default":null,"name":"preClosurePenalInterestOnType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minDepositTerm","type":["null","int"]},{"default":null,"name":"maxDepositTerm","type":["null","int"]},{"default":null,"name":"minDepositTermType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"maxDepositTermType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"inMultiplesOfDepositTerm","type":["null","int"]},{"default":null,"name":"inMultiplesOfDepositTermType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"depositAmount","type":["null","float"]},{"default":null,"name":"maturityAmount","type":["null","float"]},{"default":null,"name":"maturityDate","type":["null","string"]},{"default":null,"name":"depositPeriod","type":["null","int"]},{"default":null,"name":"depositPeriodFrequency","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"mandatoryRecommendedDepositAmount","type":["null","bigdecimal"]},{"default":null,"name":"totalOverdueAmount","type":["null","bigdecimal"]},{"default":null,"name":"noOfOverdueInstallments","type":["null","int"]},{"default":null,"name":"isMandatoryDeposit","type":["null","boolean"]},{"default":null,"name":"allowWithdrawal","type":["null","boolean"]},{"default":null,"name":"adjustAdvanceTowardsFuturePayments","type":["null","boolean"]},{"default":null,"name":"expectedFirstDepositOnDate","type":["null","string"]},{"default":null,"name":"isCalendarInherited","type":["null","boolean"]},{"default":null,"name":"recurringFrequency","type":["null","int"]},{"default":null,"name":"recurringFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.AccountTransferDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTransferDataV1 {
    pub id: Option<i32>,
    pub reversed: Option<bool>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "transferAmount")]
    pub transfer_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "transferDate")]
    pub transfer_date: Option<String>,
    #[serde(rename = "transferDescription")]
    pub transfer_description: Option<String>,
    #[serde(rename = "fromOffice")]
    pub from_office: Option<OfficeDataV1>,
    #[serde(rename = "fromClient")]
    pub from_client: Option<ClientDataV1>,
    #[serde(rename = "fromAccountType")]
    pub from_account_type: Option<EnumOptionDataV1>,
    #[serde(rename = "fromAccount")]
    pub from_account: Option<PortfolioAccountDataV1>,
    #[serde(rename = "toOffice")]
    pub to_office: Option<OfficeDataV1>,
    #[serde(rename = "toClient")]
    pub to_client: Option<ClientDataV1>,
    #[serde(rename = "toAccountType")]
    pub to_account_type: Option<EnumOptionDataV1>,
    #[serde(rename = "toAccount")]
    pub to_account: Option<PortfolioAccountDataV1>,
}

pub const ACCOUNT_TRANSFER_DATA_V1_AVSC: &str = r#"{"name":"AccountTransferDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"reversed","type":["null","boolean"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"transferAmount","type":["null","bigdecimal"]},{"default":null,"name":"transferDate","type":["null","string"]},{"default":null,"name":"transferDescription","type":["null","string"]},{"default":null,"name":"fromOffice","type":["null","org.apache.fineract.avro.office.v1.OfficeDataV1"]},{"default":null,"name":"fromClient","type":["null","org.apache.fineract.avro.client.v1.ClientDataV1"]},{"default":null,"name":"fromAccountType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"fromAccount","type":["null","org.apache.fineract.avro.portfolio.v1.PortfolioAccountDataV1"]},{"default":null,"name":"toOffice","type":["null","org.apache.fineract.avro.office.v1.OfficeDataV1"]},{"default":null,"name":"toClient","type":["null","org.apache.fineract.avro.client.v1.ClientDataV1"]},{"default":null,"name":"toAccountType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"toAccount","type":["null","org.apache.fineract.avro.portfolio.v1.PortfolioAccountDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountApplicationTimelineDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountApplicationTimelineDataV1 {
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    #[serde(rename = "submittedByUsername")]
    pub submitted_by_username: Option<String>,
    #[serde(rename = "submittedByFirstname")]
    pub submitted_by_firstname: Option<String>,
    #[serde(rename = "submittedByLastname")]
    pub submitted_by_lastname: Option<String>,
    #[serde(rename = "rejectedOnDate")]
    pub rejected_on_date: Option<String>,
    #[serde(rename = "rejectedByUsername")]
    pub rejected_by_username: Option<String>,
    #[serde(rename = "rejectedByFirstname")]
    pub rejected_by_firstname: Option<String>,
    #[serde(rename = "rejectedByLastname")]
    pub rejected_by_lastname: Option<String>,
    #[serde(rename = "withdrawnOnDate")]
    pub withdrawn_on_date: Option<String>,
    #[serde(rename = "withdrawnByUsername")]
    pub withdrawn_by_username: Option<String>,
    #[serde(rename = "withdrawnByFirstname")]
    pub withdrawn_by_firstname: Option<String>,
    #[serde(rename = "withdrawnByLastname")]
    pub withdrawn_by_lastname: Option<String>,
    #[serde(rename = "approvedOnDate")]
    pub approved_on_date: Option<String>,
    #[serde(rename = "approvedByUsername")]
    pub approved_by_username: Option<String>,
    #[serde(rename = "approvedByFirstname")]
    pub approved_by_firstname: Option<String>,
    #[serde(rename = "approvedByLastname")]
    pub approved_by_lastname: Option<String>,
    #[serde(rename = "activatedOnDate")]
    pub activated_on_date: Option<String>,
    #[serde(rename = "activatedByUsername")]
    pub activated_by_username: Option<String>,
    #[serde(rename = "activatedByFirstname")]
    pub activated_by_firstname: Option<String>,
    #[serde(rename = "activatedByLastname")]
    pub activated_by_lastname: Option<String>,
    #[serde(rename = "closedOnDate")]
    pub closed_on_date: Option<String>,
    #[serde(rename = "closedByUsername")]
    pub closed_by_username: Option<String>,
    #[serde(rename = "closedByFirstname")]
    pub closed_by_firstname: Option<String>,
    #[serde(rename = "closedByLastname")]
    pub closed_by_lastname: Option<String>,
}

pub const SAVINGS_ACCOUNT_APPLICATION_TIMELINE_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountApplicationTimelineDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"submittedByUsername","type":["null","string"]},{"default":null,"name":"submittedByFirstname","type":["null","string"]},{"default":null,"name":"submittedByLastname","type":["null","string"]},{"default":null,"name":"rejectedOnDate","type":["null","string"]},{"default":null,"name":"rejectedByUsername","type":["null","string"]},{"default":null,"name":"rejectedByFirstname","type":["null","string"]},{"default":null,"name":"rejectedByLastname","type":["null","string"]},{"default":null,"name":"withdrawnOnDate","type":["null","string"]},{"default":null,"name":"withdrawnByUsername","type":["null","string"]},{"default":null,"name":"withdrawnByFirstname","type":["null","string"]},{"default":null,"name":"withdrawnByLastname","type":["null","string"]},{"default":null,"name":"approvedOnDate","type":["null","string"]},{"default":null,"name":"approvedByUsername","type":["null","string"]},{"default":null,"name":"approvedByFirstname","type":["null","string"]},{"default":null,"name":"approvedByLastname","type":["null","string"]},{"default":null,"name":"activatedOnDate","type":["null","string"]},{"default":null,"name":"activatedByUsername","type":["null","string"]},{"default":null,"name":"activatedByFirstname","type":["null","string"]},{"default":null,"name":"activatedByLastname","type":["null","string"]},{"default":null,"name":"closedOnDate","type":["null","string"]},{"default":null,"name":"closedByUsername","type":["null","string"]},{"default":null,"name":"closedByFirstname","type":["null","string"]},{"default":null,"name":"closedByLastname","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountChargeDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountChargeDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "chargeId")]
    pub charge_id: Option<i64>,
    #[serde(rename = "accountId")]
    pub account_id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "chargeTimeType")]
    pub charge_time_type: Option<EnumOptionDataV1>,
    #[serde(rename = "dueDate")]
    pub due_date: Option<String>,
    #[serde(rename = "feeOnMonthDay")]
    pub fee_on_month_day: Option<String>,
    #[serde(rename = "feeInterval")]
    pub fee_interval: Option<i32>,
    #[serde(rename = "chargeCalculationType")]
    pub charge_calculation_type: Option<EnumOptionDataV1>,
    pub percentage: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountPercentageAppliedTo")]
    pub amount_percentage_applied_to: Option<rust_decimal::Decimal>,
    pub currency: Option<CurrencyDataV1>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountPaid")]
    pub amount_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountWaived")]
    pub amount_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountWrittenOff")]
    pub amount_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountOutstanding")]
    pub amount_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountOrPercentage")]
    pub amount_or_percentage: Option<rust_decimal::Decimal>,
    pub penalty: Option<bool>,
    #[serde(rename = "isActive")]
    pub is_active: Option<bool>,
    #[serde(rename = "isFreeWithdrawal")]
    pub is_free_withdrawal: Option<bool>,
    #[serde(rename = "freeWithdrawalChargeFrequency")]
    pub free_withdrawal_charge_frequency: Option<i32>,
    #[serde(rename = "restartFrequency")]
    pub restart_frequency: Option<i32>,
    #[serde(rename = "restartFrequencyEnum")]
    pub restart_frequency_enum: Option<i32>,
    #[serde(rename = "inactivationDate")]
    pub inactivation_date: Option<String>,
    #[serde(rename = "chargeData")]
    pub charge_data: Option<ChargeDataV1>,
}

pub const SAVINGS_ACCOUNT_CHARGE_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountChargeDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"chargeId","type":["null","long"]},{"default":null,"name":"accountId","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"chargeTimeType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"dueDate","type":["null","string"]},{"default":null,"name":"feeOnMonthDay","type":["null","string"]},{"default":null,"name":"feeInterval","type":["null","int"]},{"default":null,"name":"chargeCalculationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"percentage","type":["null","bigdecimal"]},{"default":null,"name":"amountPercentageAppliedTo","type":["null","bigdecimal"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"amountPaid","type":["null","bigdecimal"]},{"default":null,"name":"amountWaived","type":["null","bigdecimal"]},{"default":null,"name":"amountWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"amountOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"amountOrPercentage","type":["null","bigdecimal"]},{"default":null,"name":"penalty","type":["null","boolean"]},{"default":null,"name":"isActive","type":["null","boolean"]},{"default":null,"name":"isFreeWithdrawal","type":["null","boolean"]},{"default":null,"name":"freeWithdrawalChargeFrequency","type":["null","int"]},{"default":null,"name":"restartFrequency","type":["null","int"]},{"default":null,"name":"restartFrequencyEnum","type":["null","int"]},{"default":null,"name":"inactivationDate","type":["null","string"]},{"default":null,"name":"chargeData","type":["null","org.apache.fineract.avro.portfolio.v1.ChargeDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountChargesPaidByDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountChargesPaidByDataV1 {
    #[serde(rename = "chargeId")]
    pub charge_id: Option<i64>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "savingsAccountChargeData")]
    pub savings_account_charge_data: Option<SavingsAccountChargeDataV1>,
}

pub const SAVINGS_ACCOUNT_CHARGES_PAID_BY_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountChargesPaidByDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"chargeId","type":["null","long"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"savingsAccountChargeData","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountChargeDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountDataV1 {
    pub id: Option<i32>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "depositType")]
    pub deposit_type: Option<EnumOptionDataV1>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "groupId")]
    pub group_id: Option<i32>,
    #[serde(rename = "groupName")]
    pub group_name: Option<String>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i32>,
    #[serde(rename = "clientName")]
    pub client_name: Option<String>,
    #[serde(rename = "savingsProductId")]
    pub savings_product_id: Option<i32>,
    #[serde(rename = "savingsProductName")]
    pub savings_product_name: Option<String>,
    #[serde(rename = "fieldOfficerId")]
    pub field_officer_id: Option<i32>,
    #[serde(rename = "fieldOfficerName")]
    pub field_officer_name: Option<String>,
    pub status: Option<SavingsAccountStatusEnumDataV1>,
    #[serde(rename = "subStatus")]
    pub sub_status: Option<SavingsAccountSubStatusEnumDataV1>,
    #[serde(rename = "reasonForBlock")]
    pub reason_for_block: Option<String>,
    pub timeline: Option<SavingsAccountApplicationTimelineDataV1>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "nominalAnnualInterestRate")]
    pub nominal_annual_interest_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestCompoundingPeriodType")]
    pub interest_compounding_period_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestPostingPeriodType")]
    pub interest_posting_period_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestCalculationType")]
    pub interest_calculation_type: Option<EnumOptionDataV1>,
    #[serde(rename = "interestCalculationDaysInYearType")]
    pub interest_calculation_days_in_year_type: Option<EnumOptionDataV1>,
    #[serde(rename = "minRequiredOpeningBalance")]
    pub min_required_opening_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "lockinPeriodFrequency")]
    pub lockin_period_frequency: Option<i32>,
    #[serde(rename = "lockinPeriodFrequencyType")]
    pub lockin_period_frequency_type: Option<EnumOptionDataV1>,
    #[serde(rename = "withdrawalFeeForTransfers")]
    pub withdrawal_fee_for_transfers: Option<bool>,
    #[serde(rename = "allowOverdraft")]
    pub allow_overdraft: Option<bool>,
    #[serde(rename = "overdraftLimit")]
    pub overdraft_limit: Option<rust_decimal::Decimal>,
    #[serde(rename = "minRequiredBalance")]
    pub min_required_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "enforceMinRequiredBalance")]
    pub enforce_min_required_balance: Option<bool>,
    #[serde(rename = "maxAllowedLienLimit")]
    pub max_allowed_lien_limit: Option<rust_decimal::Decimal>,
    #[serde(rename = "lienAllowed")]
    pub lien_allowed: Option<bool>,
    #[serde(rename = "minBalanceForInterestCalculation")]
    pub min_balance_for_interest_calculation: Option<rust_decimal::Decimal>,
    #[serde(rename = "onHoldFunds")]
    pub on_hold_funds: Option<rust_decimal::Decimal>,
    #[serde(rename = "withHoldTax")]
    pub with_hold_tax: Option<bool>,
    #[serde(rename = "lastActiveTransactionDate")]
    pub last_active_transaction_date: Option<String>,
    #[serde(rename = "isDormancyTrackingActive")]
    pub is_dormancy_tracking_active: Option<bool>,
    #[serde(rename = "daysToInactive")]
    pub days_to_inactive: Option<i32>,
    #[serde(rename = "daysToDormancy")]
    pub days_to_dormancy: Option<i32>,
    #[serde(rename = "daysToEscheat")]
    pub days_to_escheat: Option<i32>,
    #[serde(rename = "savingsAmountOnHold")]
    pub savings_amount_on_hold: Option<rust_decimal::Decimal>,
    pub summary: Option<SavingsAccountSummaryDataV1>,
    pub transactions: Option<Vec<SavingsAccountTransactionDataV1>>,
    pub charges: Option<Vec<SavingsAccountChargeDataV1>>,
    #[serde(rename = "withdrawalFee")]
    pub withdrawal_fee: Option<SavingsAccountChargeDataV1>,
    #[serde(rename = "annualFee")]
    pub annual_fee: Option<SavingsAccountChargeDataV1>,
    #[serde(rename = "nominalAnnualInterestRateOverdraft")]
    pub nominal_annual_interest_rate_overdraft: Option<rust_decimal::Decimal>,
    #[serde(rename = "minOverdraftForInterestCalculation")]
    pub min_overdraft_for_interest_calculation: Option<rust_decimal::Decimal>,
}

pub const SAVINGS_ACCOUNT_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"depositType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"groupId","type":["null","int"]},{"default":null,"name":"groupName","type":["null","string"]},{"default":null,"name":"clientId","type":["null","int"]},{"default":null,"name":"clientName","type":["null","string"]},{"default":null,"name":"savingsProductId","type":["null","int"]},{"default":null,"name":"savingsProductName","type":["null","string"]},{"default":null,"name":"fieldOfficerId","type":["null","int"]},{"default":null,"name":"fieldOfficerName","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountStatusEnumDataV1"]},{"default":null,"name":"subStatus","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountSubStatusEnumDataV1"]},{"default":null,"name":"reasonForBlock","type":["null","string"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountApplicationTimelineDataV1"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"nominalAnnualInterestRate","type":["null","bigdecimal"]},{"default":null,"name":"interestCompoundingPeriodType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestPostingPeriodType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestCalculationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"interestCalculationDaysInYearType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minRequiredOpeningBalance","type":["null","bigdecimal"]},{"default":null,"name":"lockinPeriodFrequency","type":["null","int"]},{"default":null,"name":"lockinPeriodFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"withdrawalFeeForTransfers","type":["null","boolean"]},{"default":null,"name":"allowOverdraft","type":["null","boolean"]},{"default":null,"name":"overdraftLimit","type":["null","bigdecimal"]},{"default":null,"name":"minRequiredBalance","type":["null","bigdecimal"]},{"default":null,"name":"enforceMinRequiredBalance","type":["null","boolean"]},{"default":null,"name":"maxAllowedLienLimit","type":["null","bigdecimal"]},{"default":null,"name":"lienAllowed","type":["null","boolean"]},{"default":null,"name":"minBalanceForInterestCalculation","type":["null","bigdecimal"]},{"default":null,"name":"onHoldFunds","type":["null","bigdecimal"]},{"default":null,"name":"withHoldTax","type":["null","boolean"]},{"default":null,"name":"lastActiveTransactionDate","type":["null","string"]},{"default":null,"name":"isDormancyTrackingActive","type":["null","boolean"]},{"default":null,"name":"daysToInactive","type":["null","int"]},{"default":null,"name":"daysToDormancy","type":["null","int"]},{"default":null,"name":"daysToEscheat","type":["null","int"]},{"default":null,"name":"savingsAmountOnHold","type":["null","bigdecimal"]},{"default":null,"name":"summary","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountSummaryDataV1"]},{"default":null,"name":"transactions","type":["null",{"type":"array","items":"org.apache.fineract.avro.savings.v1.SavingsAccountTransactionDataV1"}]},{"default":null,"name":"charges","type":["null",{"type":"array","items":"org.apache.fineract.avro.savings.v1.SavingsAccountChargeDataV1"}]},{"default":null,"name":"withdrawalFee","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountChargeDataV1"]},{"default":null,"name":"annualFee","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountChargeDataV1"]},{"default":null,"name":"nominalAnnualInterestRateOverdraft","type":["null","bigdecimal"]},{"default":null,"name":"minOverdraftForInterestCalculation","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountStatusEnumDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountStatusEnumDataV1 {
    pub id: Option<i32>,
    pub code: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "submittedAndPendingApproval")]
    pub submitted_and_pending_approval: Option<bool>,
    pub approved: Option<bool>,
    pub rejected: Option<bool>,
    #[serde(rename = "withdrawnByApplicant")]
    pub withdrawn_by_applicant: Option<bool>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    #[serde(rename = "prematureClosed")]
    pub premature_closed: Option<bool>,
    #[serde(rename = "transferInProgress")]
    pub transfer_in_progress: Option<bool>,
    #[serde(rename = "transferOnHold")]
    pub transfer_on_hold: Option<bool>,
    pub matured: Option<bool>,
}

pub const SAVINGS_ACCOUNT_STATUS_ENUM_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountStatusEnumDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"value","type":["null","string"]},{"default":null,"name":"submittedAndPendingApproval","type":["null","boolean"]},{"default":null,"name":"approved","type":["null","boolean"]},{"default":null,"name":"rejected","type":["null","boolean"]},{"default":null,"name":"withdrawnByApplicant","type":["null","boolean"]},{"default":null,"name":"active","type":["null","boolean"]},{"default":null,"name":"closed","type":["null","boolean"]},{"default":null,"name":"prematureClosed","type":["null","boolean"]},{"default":null,"name":"transferInProgress","type":["null","boolean"]},{"default":null,"name":"transferOnHold","type":["null","boolean"]},{"default":null,"name":"matured","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountStayedLockedDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountStayedLockedDataV1 {
    pub id: i64,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
}

pub const SAVINGS_ACCOUNT_STAYED_LOCKED_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountStayedLockedDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"name":"id","type":"long"},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"accountNo","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountSubStatusEnumDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountSubStatusEnumDataV1 {
    pub id: Option<i32>,
    pub code: Option<String>,
    pub value: Option<String>,
    pub none: Option<bool>,
    pub inactive: Option<bool>,
    pub dormant: Option<bool>,
    pub escheat: Option<bool>,
    pub block: Option<bool>,
    #[serde(rename = "blockCredit")]
    pub block_credit: Option<bool>,
    #[serde(rename = "blockDebit")]
    pub block_debit: Option<bool>,
}

pub const SAVINGS_ACCOUNT_SUB_STATUS_ENUM_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountSubStatusEnumDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"value","type":["null","string"]},{"default":null,"name":"none","type":["null","boolean"]},{"default":null,"name":"inactive","type":["null","boolean"]},{"default":null,"name":"dormant","type":["null","boolean"]},{"default":null,"name":"escheat","type":["null","boolean"]},{"default":null,"name":"block","type":["null","boolean"]},{"default":null,"name":"blockCredit","type":["null","boolean"]},{"default":null,"name":"blockDebit","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountSummaryDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountSummaryDataV1 {
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "totalDeposits")]
    pub total_deposits: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWithdrawals")]
    pub total_withdrawals: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWithdrawalFees")]
    pub total_withdrawal_fees: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalAnnualFees")]
    pub total_annual_fees: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalInterestEarned")]
    pub total_interest_earned: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalInterestPosted")]
    pub total_interest_posted: Option<rust_decimal::Decimal>,
    #[serde(rename = "accountBalance")]
    pub account_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalFeeCharge")]
    pub total_fee_charge: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPenaltyCharge")]
    pub total_penalty_charge: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalOverdraftInterestDerived")]
    pub total_overdraft_interest_derived: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalWithholdTax")]
    pub total_withhold_tax: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestNotPosted")]
    pub interest_not_posted: Option<rust_decimal::Decimal>,
    #[serde(rename = "lastInterestCalculationDate")]
    pub last_interest_calculation_date: Option<String>,
    #[serde(rename = "availableBalance")]
    pub available_balance: Option<rust_decimal::Decimal>,
    #[serde(rename = "interestPostedTillDate")]
    pub interest_posted_till_date: Option<String>,
    #[serde(rename = "prevInterestPostedTillDate")]
    pub prev_interest_posted_till_date: Option<String>,
}

pub const SAVINGS_ACCOUNT_SUMMARY_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountSummaryDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"totalDeposits","type":["null","bigdecimal"]},{"default":null,"name":"totalWithdrawals","type":["null","bigdecimal"]},{"default":null,"name":"totalWithdrawalFees","type":["null","bigdecimal"]},{"default":null,"name":"totalAnnualFees","type":["null","bigdecimal"]},{"default":null,"name":"totalInterestEarned","type":["null","bigdecimal"]},{"default":null,"name":"totalInterestPosted","type":["null","bigdecimal"]},{"default":null,"name":"accountBalance","type":["null","bigdecimal"]},{"default":null,"name":"totalFeeCharge","type":["null","bigdecimal"]},{"default":null,"name":"totalPenaltyCharge","type":["null","bigdecimal"]},{"default":null,"name":"totalOverdraftInterestDerived","type":["null","bigdecimal"]},{"default":null,"name":"totalWithholdTax","type":["null","bigdecimal"]},{"default":null,"name":"interestNotPosted","type":["null","bigdecimal"]},{"default":null,"name":"lastInterestCalculationDate","type":["null","string"]},{"default":null,"name":"availableBalance","type":["null","bigdecimal"]},{"default":null,"name":"interestPostedTillDate","type":["null","string"]},{"default":null,"name":"prevInterestPostedTillDate","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountTransactionDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountTransactionDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "transactionType")]
    pub transaction_type: Option<SavingsAccountTransactionEnumDataV1>,
    #[serde(rename = "accountId")]
    pub account_id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    pub date: Option<String>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "paymentDetailData")]
    pub payment_detail_data: Option<PaymentDetailDataV1>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "outstandingChargeAmount")]
    pub outstanding_charge_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "runningBalance")]
    pub running_balance: Option<rust_decimal::Decimal>,
    pub reversed: Option<bool>,
    pub transfer: Option<AccountTransferDataV1>,
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    #[serde(rename = "interestedPostedAsOn")]
    pub interested_posted_as_on: Option<bool>,
    #[serde(rename = "submittedByUsername")]
    pub submitted_by_username: Option<String>,
    pub note: Option<String>,
    #[serde(rename = "isManualTransaction")]
    pub is_manual_transaction: Option<bool>,
    #[serde(rename = "isReversal")]
    pub is_reversal: Option<bool>,
    #[serde(rename = "originalTransactionId")]
    pub original_transaction_id: Option<String>,
    #[serde(rename = "lienTransaction")]
    pub lien_transaction: Option<bool>,
    #[serde(rename = "releaseTransactionId")]
    pub release_transaction_id: Option<i64>,
    #[serde(rename = "reasonForBlock")]
    pub reason_for_block: Option<String>,
    #[serde(rename = "chargesPaidByData")]
    pub charges_paid_by_data: Option<Vec<SavingsAccountChargesPaidByDataV1>>,
}

pub const SAVINGS_ACCOUNT_TRANSACTION_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountTransactionDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"transactionType","type":["null","org.apache.fineract.avro.savings.v1.SavingsAccountTransactionEnumDataV1"]},{"default":null,"name":"accountId","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"date","type":["null","string"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"paymentDetailData","type":["null","org.apache.fineract.avro.payment.v1.PaymentDetailDataV1"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"outstandingChargeAmount","type":["null","bigdecimal"]},{"default":null,"name":"runningBalance","type":["null","bigdecimal"]},{"default":null,"name":"reversed","type":["null","boolean"]},{"default":null,"name":"transfer","type":["null","org.apache.fineract.avro.savings.v1.AccountTransferDataV1"]},{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"interestedPostedAsOn","type":["null","boolean"]},{"default":null,"name":"submittedByUsername","type":["null","string"]},{"default":null,"name":"note","type":["null","string"]},{"default":null,"name":"isManualTransaction","type":["null","boolean"]},{"default":null,"name":"isReversal","type":["null","boolean"]},{"default":null,"name":"originalTransactionId","type":["null","string"]},{"default":null,"name":"lienTransaction","type":["null","boolean"]},{"default":null,"name":"releaseTransactionId","type":["null","long"]},{"default":null,"name":"reasonForBlock","type":["null","string"]},{"default":null,"name":"chargesPaidByData","type":["null",{"type":"array","items":"org.apache.fineract.avro.savings.v1.SavingsAccountChargesPaidByDataV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountTransactionEnumDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountTransactionEnumDataV1 {
    pub id: Option<i64>,
    pub code: Option<String>,
    pub value: Option<String>,
    pub deposit: Option<bool>,
    #[serde(rename = "dividendPayout")]
    pub dividend_payout: Option<bool>,
    pub withdrawal: Option<bool>,
    #[serde(rename = "interestPosting")]
    pub interest_posting: Option<bool>,
    #[serde(rename = "feeDeduction")]
    pub fee_deduction: Option<bool>,
    #[serde(rename = "initiateTransfer")]
    pub initiate_transfer: Option<bool>,
    #[serde(rename = "approveTransfer")]
    pub approve_transfer: Option<bool>,
    #[serde(rename = "withdrawTransfer")]
    pub withdraw_transfer: Option<bool>,
    #[serde(rename = "rejectTransfer")]
    pub reject_transfer: Option<bool>,
    #[serde(rename = "overdraftInterest")]
    pub overdraft_interest: Option<bool>,
    pub writtenoff: Option<bool>,
    #[serde(rename = "overdraftFee")]
    pub overdraft_fee: Option<bool>,
    #[serde(rename = "withholdTax")]
    pub withhold_tax: Option<bool>,
    pub escheat: Option<bool>,
    #[serde(rename = "amountHold")]
    pub amount_hold: Option<bool>,
    #[serde(rename = "amountRelease")]
    pub amount_release: Option<bool>,
}

pub const SAVINGS_ACCOUNT_TRANSACTION_ENUM_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountTransactionEnumDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"value","type":["null","string"]},{"default":null,"name":"deposit","type":["null","boolean"]},{"default":null,"name":"dividendPayout","type":["null","boolean"]},{"default":null,"name":"withdrawal","type":["null","boolean"]},{"default":null,"name":"interestPosting","type":["null","boolean"]},{"default":null,"name":"feeDeduction","type":["null","boolean"]},{"default":null,"name":"initiateTransfer","type":["null","boolean"]},{"default":null,"name":"approveTransfer","type":["null","boolean"]},{"default":null,"name":"withdrawTransfer","type":["null","boolean"]},{"default":null,"name":"rejectTransfer","type":["null","boolean"]},{"default":null,"name":"overdraftInterest","type":["null","boolean"]},{"default":null,"name":"writtenoff","type":["null","boolean"]},{"default":null,"name":"overdraftFee","type":["null","boolean"]},{"default":null,"name":"withholdTax","type":["null","boolean"]},{"default":null,"name":"escheat","type":["null","boolean"]},{"default":null,"name":"amountHold","type":["null","boolean"]},{"default":null,"name":"amountRelease","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.savings.v1.SavingsAccountsStayedLockedDataV1` (Namespace: `org.apache.fineract.avro.savings.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsAccountsStayedLockedDataV1 {
    #[serde(rename = "savingsAccounts")]
    pub savings_accounts: Vec<SavingsAccountStayedLockedDataV1>,
}

pub const SAVINGS_ACCOUNTS_STAYED_LOCKED_DATA_V1_AVSC: &str = r#"{"name":"SavingsAccountsStayedLockedDataV1","namespace":"org.apache.fineract.avro.savings.v1","type":"record","fields":[{"name":"savingsAccounts","type":{"type":"array","items":"org.apache.fineract.avro.savings.v1.SavingsAccountStayedLockedDataV1"}}]}"#;

/// Avro schema `org.apache.fineract.avro.share.v1.ShareAccountApplicationTimelineDataV1` (Namespace: `org.apache.fineract.avro.share.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccountApplicationTimelineDataV1 {
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    #[serde(rename = "submittedByUsername")]
    pub submitted_by_username: Option<String>,
    #[serde(rename = "submittedByFirstname")]
    pub submitted_by_firstname: Option<String>,
    #[serde(rename = "submittedByLastname")]
    pub submitted_by_lastname: Option<String>,
    #[serde(rename = "rejectedDate")]
    pub rejected_date: Option<String>,
    #[serde(rename = "rejectedByUsername")]
    pub rejected_by_username: Option<String>,
    #[serde(rename = "rejectedByFirstname")]
    pub rejected_by_firstname: Option<String>,
    #[serde(rename = "rejectedByLastname")]
    pub rejected_by_lastname: Option<String>,
    #[serde(rename = "approvedDate")]
    pub approved_date: Option<String>,
    #[serde(rename = "approvedByUsername")]
    pub approved_by_username: Option<String>,
    #[serde(rename = "approvedByFirstname")]
    pub approved_by_firstname: Option<String>,
    #[serde(rename = "approvedByLastname")]
    pub approved_by_lastname: Option<String>,
    #[serde(rename = "activatedDate")]
    pub activated_date: Option<String>,
    #[serde(rename = "activatedByUsername")]
    pub activated_by_username: Option<String>,
    #[serde(rename = "activatedByFirstname")]
    pub activated_by_firstname: Option<String>,
    #[serde(rename = "activatedByLastname")]
    pub activated_by_lastname: Option<String>,
    #[serde(rename = "closedDate")]
    pub closed_date: Option<String>,
    #[serde(rename = "closedByUsername")]
    pub closed_by_username: Option<String>,
    #[serde(rename = "closedByFirstname")]
    pub closed_by_firstname: Option<String>,
    #[serde(rename = "closedByLastname")]
    pub closed_by_lastname: Option<String>,
}

pub const SHARE_ACCOUNT_APPLICATION_TIMELINE_DATA_V1_AVSC: &str = r#"{"name":"ShareAccountApplicationTimelineDataV1","namespace":"org.apache.fineract.avro.share.v1","type":"record","fields":[{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"submittedByUsername","type":["null","string"]},{"default":null,"name":"submittedByFirstname","type":["null","string"]},{"default":null,"name":"submittedByLastname","type":["null","string"]},{"default":null,"name":"rejectedDate","type":["null","string"]},{"default":null,"name":"rejectedByUsername","type":["null","string"]},{"default":null,"name":"rejectedByFirstname","type":["null","string"]},{"default":null,"name":"rejectedByLastname","type":["null","string"]},{"default":null,"name":"approvedDate","type":["null","string"]},{"default":null,"name":"approvedByUsername","type":["null","string"]},{"default":null,"name":"approvedByFirstname","type":["null","string"]},{"default":null,"name":"approvedByLastname","type":["null","string"]},{"default":null,"name":"activatedDate","type":["null","string"]},{"default":null,"name":"activatedByUsername","type":["null","string"]},{"default":null,"name":"activatedByFirstname","type":["null","string"]},{"default":null,"name":"activatedByLastname","type":["null","string"]},{"default":null,"name":"closedDate","type":["null","string"]},{"default":null,"name":"closedByUsername","type":["null","string"]},{"default":null,"name":"closedByFirstname","type":["null","string"]},{"default":null,"name":"closedByLastname","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.share.v1.ShareAccountDataV1` (Namespace: `org.apache.fineract.avro.share.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccountDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "savingsAccountNumber")]
    pub savings_account_number: Option<String>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i64>,
    #[serde(rename = "clientName")]
    pub client_name: Option<String>,
    #[serde(rename = "defaultShares")]
    pub default_shares: Option<i64>,
    #[serde(rename = "productId")]
    pub product_id: Option<i64>,
    #[serde(rename = "productName")]
    pub product_name: Option<String>,
    pub status: Option<ShareAccountStatusEnumDataV1>,
    pub timeline: Option<ShareAccountApplicationTimelineDataV1>,
    pub currency: Option<CurrencyDataV1>,
    pub summary: Option<ShareAccountSummaryDataV1>,
    #[serde(rename = "purchasedShares")]
    pub purchased_shares: Option<Vec<ShareAccountTransactionDataV1>>,
    #[serde(rename = "savingsAccountId")]
    pub savings_account_id: Option<i64>,
    #[serde(rename = "currentMarketPrice")]
    pub current_market_price: Option<rust_decimal::Decimal>,
    #[serde(rename = "lockinPeriod")]
    pub lockin_period: Option<i32>,
    #[serde(rename = "lockPeriodTypeEnum")]
    pub lock_period_type_enum: Option<EnumOptionDataV1>,
    #[serde(rename = "minimumActivePeriod")]
    pub minimum_active_period: Option<i32>,
    #[serde(rename = "minimumActivePeriodTypeEnum")]
    pub minimum_active_period_type_enum: Option<EnumOptionDataV1>,
    #[serde(rename = "allowDividendCalculationForInactiveClients")]
    pub allow_dividend_calculation_for_inactive_clients: Option<bool>,
}

pub const SHARE_ACCOUNT_DATA_V1_AVSC: &str = r#"{"name":"ShareAccountDataV1","namespace":"org.apache.fineract.avro.share.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"savingsAccountNumber","type":["null","string"]},{"default":null,"name":"clientId","type":["null","long"]},{"default":null,"name":"clientName","type":["null","string"]},{"default":null,"name":"defaultShares","type":["null","long"]},{"default":null,"name":"productId","type":["null","long"]},{"default":null,"name":"productName","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.share.v1.ShareAccountStatusEnumDataV1"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.share.v1.ShareAccountApplicationTimelineDataV1"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"summary","type":["null","org.apache.fineract.avro.share.v1.ShareAccountSummaryDataV1"]},{"default":null,"name":"purchasedShares","type":["null",{"type":"array","items":"org.apache.fineract.avro.share.v1.ShareAccountTransactionDataV1"}]},{"default":null,"name":"savingsAccountId","type":["null","long"]},{"default":null,"name":"currentMarketPrice","type":["null","bigdecimal"]},{"default":null,"name":"lockinPeriod","type":["null","int"]},{"default":null,"name":"lockPeriodTypeEnum","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minimumActivePeriod","type":["null","int"]},{"default":null,"name":"minimumActivePeriodTypeEnum","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"allowDividendCalculationForInactiveClients","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.share.v1.ShareAccountStatusEnumDataV1` (Namespace: `org.apache.fineract.avro.share.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccountStatusEnumDataV1 {
    pub id: Option<i64>,
    pub code: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "submittedAndPendingApproval")]
    pub submitted_and_pending_approval: Option<bool>,
    pub approved: Option<bool>,
    pub rejected: Option<bool>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
}

pub const SHARE_ACCOUNT_STATUS_ENUM_DATA_V1_AVSC: &str = r#"{"name":"ShareAccountStatusEnumDataV1","namespace":"org.apache.fineract.avro.share.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"code","type":["null","string"]},{"default":null,"name":"value","type":["null","string"]},{"default":null,"name":"submittedAndPendingApproval","type":["null","boolean"]},{"default":null,"name":"approved","type":["null","boolean"]},{"default":null,"name":"rejected","type":["null","boolean"]},{"default":null,"name":"active","type":["null","boolean"]},{"default":null,"name":"closed","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.share.v1.ShareAccountSummaryDataV1` (Namespace: `org.apache.fineract.avro.share.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccountSummaryDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "totalApprovedShares")]
    pub total_approved_shares: Option<i64>,
    #[serde(rename = "totalPendingForApprovalShares")]
    pub total_pending_for_approval_shares: Option<i64>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "productId")]
    pub product_id: Option<i64>,
    #[serde(rename = "productName")]
    pub product_name: Option<String>,
    #[serde(rename = "shortProductName")]
    pub short_product_name: Option<String>,
    pub status: Option<ShareAccountStatusEnumDataV1>,
    pub currency: Option<CurrencyDataV1>,
    pub timeline: Option<ShareAccountApplicationTimelineDataV1>,
}

pub const SHARE_ACCOUNT_SUMMARY_DATA_V1_AVSC: &str = r#"{"name":"ShareAccountSummaryDataV1","namespace":"org.apache.fineract.avro.share.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"totalApprovedShares","type":["null","long"]},{"default":null,"name":"totalPendingForApprovalShares","type":["null","long"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"productId","type":["null","long"]},{"default":null,"name":"productName","type":["null","string"]},{"default":null,"name":"shortProductName","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.share.v1.ShareAccountStatusEnumDataV1"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.share.v1.ShareAccountApplicationTimelineDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.share.v1.ShareAccountTransactionDataV1` (Namespace: `org.apache.fineract.avro.share.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccountTransactionDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountId")]
    pub account_id: Option<i64>,
    #[serde(rename = "purchasedDate")]
    pub purchased_date: Option<String>,
    #[serde(rename = "numberOfShares")]
    pub number_of_shares: Option<i64>,
    #[serde(rename = "purchasedPrice")]
    pub purchased_price: Option<rust_decimal::Decimal>,
    pub status: Option<EnumOptionDataV1>,
    pub r#type: Option<EnumOptionDataV1>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "chargeAmount")]
    pub charge_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountPaid")]
    pub amount_paid: Option<rust_decimal::Decimal>,
}

pub const SHARE_ACCOUNT_TRANSACTION_DATA_V1_AVSC: &str = r#"{"name":"ShareAccountTransactionDataV1","namespace":"org.apache.fineract.avro.share.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountId","type":["null","long"]},{"default":null,"name":"purchasedDate","type":["null","string"]},{"default":null,"name":"numberOfShares","type":["null","long"]},{"default":null,"name":"purchasedPrice","type":["null","bigdecimal"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"type","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"chargeAmount","type":["null","bigdecimal"]},{"default":null,"name":"amountPaid","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.share.v1.ShareProductDataV1` (Namespace: `org.apache.fineract.avro.share.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareProductDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "shortName")]
    pub short_name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "totalShares")]
    pub total_shares: Option<i64>,
    #[serde(rename = "totalSharesIssued")]
    pub total_shares_issued: Option<i64>,
    #[serde(rename = "unitPrice")]
    pub unit_price: Option<rust_decimal::Decimal>,
    #[serde(rename = "shareCapital")]
    pub share_capital: Option<rust_decimal::Decimal>,
    #[serde(rename = "minimumShares")]
    pub minimum_shares: Option<i64>,
    #[serde(rename = "nominalShares")]
    pub nominal_shares: Option<i64>,
    #[serde(rename = "maximumShares")]
    pub maximum_shares: Option<i64>,
    #[serde(rename = "marketPrice")]
    pub market_price: Option<Vec<ShareProductMarketPriceDataV1>>,
    pub charges: Option<Vec<ChargeDataV1>>,
    #[serde(rename = "allowDividendCalculationForInactiveClients")]
    pub allow_dividend_calculation_for_inactive_clients: Option<bool>,
    #[serde(rename = "lockinPeriod")]
    pub lockin_period: Option<i32>,
    #[serde(rename = "lockPeriodTypeEnum")]
    pub lock_period_type_enum: Option<EnumOptionDataV1>,
    #[serde(rename = "minimumActivePeriod")]
    pub minimum_active_period: Option<i32>,
    #[serde(rename = "minimumActivePeriodForDividendsTypeEnum")]
    pub minimum_active_period_for_dividends_type_enum: Option<EnumOptionDataV1>,
}

pub const SHARE_PRODUCT_DATA_V1_AVSC: &str = r#"{"name":"ShareProductDataV1","namespace":"org.apache.fineract.avro.share.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"shortName","type":["null","string"]},{"default":null,"name":"description","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"totalShares","type":["null","long"]},{"default":null,"name":"totalSharesIssued","type":["null","long"]},{"default":null,"name":"unitPrice","type":["null","bigdecimal"]},{"default":null,"name":"shareCapital","type":["null","bigdecimal"]},{"default":null,"name":"minimumShares","type":["null","long"]},{"default":null,"name":"nominalShares","type":["null","long"]},{"default":null,"name":"maximumShares","type":["null","long"]},{"default":null,"name":"marketPrice","type":["null",{"type":"array","items":"org.apache.fineract.avro.share.v1.ShareProductMarketPriceDataV1"}]},{"default":null,"name":"charges","type":["null",{"type":"array","items":"org.apache.fineract.avro.portfolio.v1.ChargeDataV1"}]},{"default":null,"name":"allowDividendCalculationForInactiveClients","type":["null","boolean"]},{"default":null,"name":"lockinPeriod","type":["null","int"]},{"default":null,"name":"lockPeriodTypeEnum","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"minimumActivePeriod","type":["null","int"]},{"default":null,"name":"minimumActivePeriodForDividendsTypeEnum","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.share.v1.ShareProductMarketPriceDataV1` (Namespace: `org.apache.fineract.avro.share.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareProductMarketPriceDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "fromDate")]
    pub from_date: Option<String>,
    #[serde(rename = "shareValue")]
    pub share_value: Option<rust_decimal::Decimal>,
}

pub const SHARE_PRODUCT_MARKET_PRICE_DATA_V1_AVSC: &str = r#"{"name":"ShareProductMarketPriceDataV1","namespace":"org.apache.fineract.avro.share.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"fromDate","type":["null","string"]},{"default":null,"name":"shareValue","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalBreachDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalBreachDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "breachFrequency")]
    pub breach_frequency: Option<i32>,
    #[serde(rename = "breachFrequencyType")]
    pub breach_frequency_type: Option<StringEnumOptionDataV1>,
    #[serde(rename = "breachAmountCalculationType")]
    pub breach_amount_calculation_type: Option<StringEnumOptionDataV1>,
    #[serde(rename = "breachAmount")]
    pub breach_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "breachGraceDays")]
    pub breach_grace_days: Option<i32>,
    #[serde(rename = "breachStartDate")]
    pub breach_start_date: Option<String>,
    /// breachStartDate shifted by the breach grace days; only set for the first breach period.
    #[serde(rename = "breachEffectiveStartDate")]
    pub breach_effective_start_date: Option<String>,
    #[serde(rename = "nearBreach")]
    pub near_breach: Option<WorkingCapitalNearBreachDataV1>,
    /// Current breach schedule periods.
    #[serde(rename = "breachSchedule")]
    pub breach_schedule: Option<Vec<WorkingCapitalLoanBreachSchedulePeriodDataV1>>,
    /// Minimum payment amounts of the already breached periods that are still unpaid.
    #[serde(rename = "breachPastDueAmount")]
    pub breach_past_due_amount: Option<rust_decimal::Decimal>,
}

pub const WORKING_CAPITAL_BREACH_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalBreachDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"breachFrequency","type":["null","int"]},{"default":null,"name":"breachFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"breachAmountCalculationType","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"breachAmount","type":["null","bigdecimal"]},{"default":null,"name":"breachGraceDays","type":["null","int"]},{"default":null,"name":"breachStartDate","type":["null","string"]},{"default":null,"name":"breachEffectiveStartDate","doc":"breachStartDate shifted by the breach grace days; only set for the first breach period.","type":["null","string"]},{"default":null,"name":"nearBreach","type":["null","org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalNearBreachDataV1"]},{"default":null,"name":"breachSchedule","doc":"Current breach schedule periods.","type":["null",{"type":"array","items":"org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanBreachSchedulePeriodDataV1"}]},{"default":null,"name":"breachPastDueAmount","doc":"Minimum payment amounts of the already breached periods that are still unpaid.","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanAccountDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanAccountDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    pub status: Option<LoanStatusEnumDataV1>,
    #[serde(rename = "clientId")]
    pub client_id: Option<i64>,
    #[serde(rename = "clientAccountNo")]
    pub client_account_no: Option<String>,
    #[serde(rename = "clientName")]
    pub client_name: Option<String>,
    #[serde(rename = "clientOfficeId")]
    pub client_office_id: Option<i64>,
    #[serde(rename = "clientExternalId")]
    pub client_external_id: Option<String>,
    #[serde(rename = "loanProductId")]
    pub loan_product_id: Option<i64>,
    #[serde(rename = "loanProductName")]
    pub loan_product_name: Option<String>,
    #[serde(rename = "loanProductDescription")]
    pub loan_product_description: Option<String>,
    #[serde(rename = "fundId")]
    pub fund_id: Option<i64>,
    #[serde(rename = "fundName")]
    pub fund_name: Option<String>,
    pub principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "approvedPrincipal")]
    pub approved_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "proposedPrincipal")]
    pub proposed_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "netDisbursalAmount")]
    pub net_disbursal_amount: Option<rust_decimal::Decimal>,
    /// Loan term in days, not a count of repayments. Null until the amortization schedule is generated.
    #[serde(rename = "numberOfRepayments")]
    pub number_of_repayments: Option<i32>,
    #[serde(rename = "repaymentEvery")]
    pub repayment_every: Option<i32>,
    #[serde(rename = "repaymentFrequencyType")]
    pub repayment_frequency_type: Option<StringEnumOptionDataV1>,
    #[serde(rename = "amortizationType")]
    pub amortization_type: Option<StringEnumOptionDataV1>,
    /// Grace days before a period counts as delinquent.
    #[serde(rename = "graceOnArrearsAgeing")]
    pub grace_on_arrears_ageing: Option<i32>,
    pub timeline: Option<LoanApplicationTimelineDataV1>,
    pub summary: Option<WorkingCapitalLoanSummaryDataV1>,
    pub charges: Option<Vec<WorkingCapitalLoanChargeDataV1>>,
    #[serde(rename = "disbursementDetails")]
    pub disbursement_details: Option<Vec<WorkingCapitalLoanDisbursementDetailDataV1>>,
    #[serde(rename = "totalOverpaid")]
    pub total_overpaid: Option<rust_decimal::Decimal>,
    #[serde(rename = "loanProductCounter")]
    pub loan_product_counter: Option<i32>,
    pub delinquent: Option<WorkingCapitalLoanCollectionDataV1>,
    #[serde(rename = "lastClosedBusinessDate")]
    pub last_closed_business_date: Option<String>,
    /// True once the loan has been charged off.
    #[serde(rename = "chargedOff")]
    pub charged_off: Option<bool>,
    /// True when the loan has a delinquency bucket configured. Working capital classifies delinquency per period, not per installment.
    #[serde(rename = "enableInstallmentLevelDelinquency")]
    pub enable_installment_level_delinquency: Option<bool>,
    pub originators: Option<Vec<OriginatorDetailsV1>>,
    pub breach: Option<WorkingCapitalBreachDataV1>,
    /// The loan's own period payment rate. A rate change does not move it - the rate in force on a given date comes from the rate-change history.
    #[serde(rename = "paymentRate")]
    pub payment_rate: Option<rust_decimal::Decimal>,
    /// Effective discount fee, set at disbursement.
    #[serde(rename = "discountFee")]
    pub discount_fee: Option<rust_decimal::Decimal>,
    #[serde(rename = "proposedDiscountFee")]
    pub proposed_discount_fee: Option<rust_decimal::Decimal>,
    #[serde(rename = "approvedDiscountFee")]
    pub approved_discount_fee: Option<rust_decimal::Decimal>,
    /// Daily payment amount the loan was priced at, following paymentAmountCalculationStrategy: totalPaymentVolume x paymentRate / 100 / npvDayCount rounded to the currency under TPV, solved from annualEir under ANNUAL_EIR, and paymentAmount itself under PAYMENT_AMOUNT. A rate change does not restate it - what is billed from the day a change takes effect follows the rate then in force.
    #[serde(rename = "periodPaymentAmount")]
    pub period_payment_amount: Option<rust_decimal::Decimal>,
    /// Annual effective rate the loan was priced at, as a percentage: compounded over the product's NPV day count, not a calendar year, and rounded to six decimals. The base schedule's daily discounting derives from it. A rate change does not restate it - the schedule re-solves its own rate from the day the change takes effect. Comes from discount-fee pricing, not a lending interest rate.
    #[serde(rename = "calculatedAnnualEir")]
    pub calculated_annual_eir: Option<rust_decimal::Decimal>,
    /// Day count used by the amortization schedule.
    #[serde(rename = "npvDayCount")]
    pub npv_day_count: Option<i32>,
    /// Total payment volume the daily payment is sized from. Only for the TPV payment amount calculation strategy.
    #[serde(rename = "totalPaymentVolume")]
    pub total_payment_volume: Option<rust_decimal::Decimal>,
    /// How the daily payment is calculated, copied from the product when the loan is created: TPV (from totalPaymentVolume and paymentRate), ANNUAL_EIR (from annualEir) or PAYMENT_AMOUNT (paymentAmount as given).
    #[serde(rename = "paymentAmountCalculationStrategy")]
    pub payment_amount_calculation_strategy: Option<StringEnumOptionDataV1>,
    /// Contractual annual EIR percentage the daily payment is solved from. Only for the ANNUAL_EIR payment amount calculation strategy.
    #[serde(rename = "annualEir")]
    pub annual_eir: Option<rust_decimal::Decimal>,
    /// Fixed daily payment amount the schedule is built from. Only for the PAYMENT_AMOUNT payment amount calculation strategy.
    #[serde(rename = "paymentAmount")]
    pub payment_amount: Option<rust_decimal::Decimal>,
    pub delinquency: Option<WorkingCapitalLoanDelinquencyDataV1>,
    #[serde(rename = "overpaidOnDate")]
    pub overpaid_on_date: Option<String>,
    #[serde(rename = "actualNoTerm")]
    pub actual_no_term: Option<i32>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
    /// Period payment rate changes with the annual EIR, daily payment amount and segment term snapshotted when each change was booked, most recently booked first - which for a backdated change is not the same as effective-date order. Appended at the end of the record so consumers decoding positionally keep working.
    #[serde(rename = "periodPaymentRateHistory")]
    pub period_payment_rate_history: Option<Vec<WorkingCapitalLoanPeriodPaymentRateChangeDataV1>>,
    /// Currently effective period payment rate for TPV strategy loans: the rate in force on the business date from rate-change history, or paymentRate when none has taken effect yet. Null when the loan uses a non-TPV payment amount calculation strategy.
    #[serde(rename = "effectivePaymentRate")]
    pub effective_payment_rate: Option<rust_decimal::Decimal>,
}

pub const WORKING_CAPITAL_LOAN_ACCOUNT_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanAccountDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"accountNo","type":["null","string"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"status","type":["null","org.apache.fineract.avro.loan.v1.LoanStatusEnumDataV1"]},{"default":null,"name":"clientId","type":["null","long"]},{"default":null,"name":"clientAccountNo","type":["null","string"]},{"default":null,"name":"clientName","type":["null","string"]},{"default":null,"name":"clientOfficeId","type":["null","long"]},{"default":null,"name":"clientExternalId","type":["null","string"]},{"default":null,"name":"loanProductId","type":["null","long"]},{"default":null,"name":"loanProductName","type":["null","string"]},{"default":null,"name":"loanProductDescription","type":["null","string"]},{"default":null,"name":"fundId","type":["null","long"]},{"default":null,"name":"fundName","type":["null","string"]},{"default":null,"name":"principal","type":["null","bigdecimal"]},{"default":null,"name":"approvedPrincipal","type":["null","bigdecimal"]},{"default":null,"name":"proposedPrincipal","type":["null","bigdecimal"]},{"default":null,"name":"netDisbursalAmount","type":["null","bigdecimal"]},{"default":null,"name":"numberOfRepayments","doc":"Loan term in days, not a count of repayments. Null until the amortization schedule is generated.","type":["null","int"]},{"default":null,"name":"repaymentEvery","type":["null","int"]},{"default":null,"name":"repaymentFrequencyType","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"amortizationType","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"graceOnArrearsAgeing","doc":"Grace days before a period counts as delinquent.","type":["null","int"]},{"default":null,"name":"timeline","type":["null","org.apache.fineract.avro.loan.v1.LoanApplicationTimelineDataV1"]},{"default":null,"name":"summary","type":["null","org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanSummaryDataV1"]},{"default":null,"name":"charges","type":["null",{"type":"array","items":"org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanChargeDataV1"}]},{"default":null,"name":"disbursementDetails","type":["null",{"type":"array","items":"org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanDisbursementDetailDataV1"}]},{"default":null,"name":"totalOverpaid","type":["null","bigdecimal"]},{"default":null,"name":"loanProductCounter","type":["null","int"]},{"default":null,"name":"delinquent","type":["null","org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanCollectionDataV1"]},{"default":null,"name":"lastClosedBusinessDate","type":["null","string"]},{"default":null,"name":"chargedOff","doc":"True once the loan has been charged off.","type":["null","boolean"]},{"default":null,"name":"enableInstallmentLevelDelinquency","doc":"True when the loan has a delinquency bucket configured. Working capital classifies delinquency per period, not per installment.","type":["null","boolean"]},{"default":null,"name":"originators","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.OriginatorDetailsV1"}]},{"default":null,"name":"breach","type":["null","org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalBreachDataV1"]},{"default":null,"name":"paymentRate","doc":"The loan's own period payment rate. A rate change does not move it - the rate in force on a given date comes from the rate-change history.","type":["null","bigdecimal"]},{"default":null,"name":"discountFee","doc":"Effective discount fee, set at disbursement.","type":["null","bigdecimal"]},{"default":null,"name":"proposedDiscountFee","type":["null","bigdecimal"]},{"default":null,"name":"approvedDiscountFee","type":["null","bigdecimal"]},{"default":null,"name":"periodPaymentAmount","doc":"Daily payment amount the loan was priced at, following paymentAmountCalculationStrategy: totalPaymentVolume x paymentRate / 100 / npvDayCount rounded to the currency under TPV, solved from annualEir under ANNUAL_EIR, and paymentAmount itself under PAYMENT_AMOUNT. A rate change does not restate it - what is billed from the day a change takes effect follows the rate then in force.","type":["null","bigdecimal"]},{"default":null,"name":"calculatedAnnualEir","doc":"Annual effective rate the loan was priced at, as a percentage: compounded over the product's NPV day count, not a calendar year, and rounded to six decimals. The base schedule's daily discounting derives from it. A rate change does not restate it - the schedule re-solves its own rate from the day the change takes effect. Comes from discount-fee pricing, not a lending interest rate.","type":["null","bigdecimal"]},{"default":null,"name":"npvDayCount","doc":"Day count used by the amortization schedule.","type":["null","int"]},{"default":null,"name":"totalPaymentVolume","doc":"Total payment volume the daily payment is sized from. Only for the TPV payment amount calculation strategy.","type":["null","bigdecimal"]},{"default":null,"name":"paymentAmountCalculationStrategy","doc":"How the daily payment is calculated, copied from the product when the loan is created: TPV (from totalPaymentVolume and paymentRate), ANNUAL_EIR (from annualEir) or PAYMENT_AMOUNT (paymentAmount as given).","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"annualEir","doc":"Contractual annual EIR percentage the daily payment is solved from. Only for the ANNUAL_EIR payment amount calculation strategy.","type":["null","bigdecimal"]},{"default":null,"name":"paymentAmount","doc":"Fixed daily payment amount the schedule is built from. Only for the PAYMENT_AMOUNT payment amount calculation strategy.","type":["null","bigdecimal"]},{"default":null,"name":"delinquency","type":["null","org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanDelinquencyDataV1"]},{"default":null,"name":"overpaidOnDate","type":["null","string"]},{"default":null,"name":"actualNoTerm","type":["null","int"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]},{"default":null,"name":"periodPaymentRateHistory","doc":"Period payment rate changes with the annual EIR, daily payment amount and segment term snapshotted when each change was booked, most recently booked first - which for a backdated change is not the same as effective-date order. Appended at the end of the record so consumers decoding positionally keep working.","type":["null",{"type":"array","items":"org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanPeriodPaymentRateChangeDataV1"}]},{"default":null,"name":"effectivePaymentRate","doc":"Currently effective period payment rate for TPV strategy loans: the rate in force on the business date from rate-change history, or paymentRate when none has taken effect yet. Null when the loan uses a non-TPV payment amount calculation strategy.","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanAccountStayedLockedDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanAccountStayedLockedDataV1 {
    pub id: i64,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "accountNo")]
    pub account_no: Option<String>,
}

pub const WORKING_CAPITAL_LOAN_ACCOUNT_STAYED_LOCKED_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanAccountStayedLockedDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"name":"id","type":"long"},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"accountNo","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanAccountsStayedLockedDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanAccountsStayedLockedDataV1 {
    #[serde(rename = "workingCapitalLoanAccounts")]
    pub working_capital_loan_accounts: Vec<WorkingCapitalLoanAccountStayedLockedDataV1>,
}

pub const WORKING_CAPITAL_LOAN_ACCOUNTS_STAYED_LOCKED_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanAccountsStayedLockedDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"name":"workingCapitalLoanAccounts","type":{"type":"array","items":"org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanAccountStayedLockedDataV1"}}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanBreachSchedulePeriodDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanBreachSchedulePeriodDataV1 {
    #[serde(rename = "periodNumber")]
    pub period_number: Option<i32>,
    #[serde(rename = "fromDate")]
    pub from_date: Option<String>,
    #[serde(rename = "toDate")]
    pub to_date: Option<String>,
    #[serde(rename = "numberOfDays")]
    pub number_of_days: Option<i32>,
    #[serde(rename = "minPaymentAmount")]
    pub min_payment_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "outstandingAmount")]
    pub outstanding_amount: Option<rust_decimal::Decimal>,
    /// Amount already paid towards the minimum payment of this breach period.
    #[serde(rename = "paidAmount")]
    pub paid_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "nearBreach")]
    pub near_breach: Option<bool>,
    pub breach: Option<bool>,
    pub reset: Option<bool>,
}

pub const WORKING_CAPITAL_LOAN_BREACH_SCHEDULE_PERIOD_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanBreachSchedulePeriodDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"periodNumber","type":["null","int"]},{"default":null,"name":"fromDate","type":["null","string"]},{"default":null,"name":"toDate","type":["null","string"]},{"default":null,"name":"numberOfDays","type":["null","int"]},{"default":null,"name":"minPaymentAmount","type":["null","bigdecimal"]},{"default":null,"name":"outstandingAmount","type":["null","bigdecimal"]},{"default":null,"name":"paidAmount","doc":"Amount already paid towards the minimum payment of this breach period.","type":["null","bigdecimal"]},{"default":null,"name":"nearBreach","type":["null","boolean"]},{"default":null,"name":"breach","type":["null","boolean"]},{"default":null,"name":"reset","type":["null","boolean"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanChargeDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanChargeDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "chargeId")]
    pub charge_id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "chargeTimeType")]
    pub charge_time_type: Option<EnumOptionDataV1>,
    #[serde(rename = "dueDate")]
    pub due_date: Option<String>,
    #[serde(rename = "chargeCalculationType")]
    pub charge_calculation_type: Option<EnumOptionDataV1>,
    pub currency: Option<CurrencyDataV1>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountPaid")]
    pub amount_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountWaived")]
    pub amount_waived: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountWrittenOff")]
    pub amount_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountOutstanding")]
    pub amount_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountAccrued")]
    pub amount_accrued: Option<rust_decimal::Decimal>,
    #[serde(rename = "amountUnrecognized")]
    pub amount_unrecognized: Option<rust_decimal::Decimal>,
    pub penalty: Option<bool>,
    #[serde(rename = "chargePaymentMode")]
    pub charge_payment_mode: Option<EnumOptionDataV1>,
    pub paid: Option<bool>,
    #[serde(rename = "loanId")]
    pub loan_id: Option<i64>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "externalLoanId")]
    pub external_loan_id: Option<String>,
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    #[serde(rename = "customData")]
    pub custom_data: Option<std::collections::HashMap<String, Vec<u8>>>,
}

pub const WORKING_CAPITAL_LOAN_CHARGE_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanChargeDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"chargeId","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"chargeTimeType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"dueDate","type":["null","string"]},{"default":null,"name":"chargeCalculationType","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"amountPaid","type":["null","bigdecimal"]},{"default":null,"name":"amountWaived","type":["null","bigdecimal"]},{"default":null,"name":"amountWrittenOff","type":["null","bigdecimal"]},{"default":null,"name":"amountOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"amountAccrued","type":["null","bigdecimal"]},{"default":null,"name":"amountUnrecognized","type":["null","bigdecimal"]},{"default":null,"name":"penalty","type":["null","boolean"]},{"default":null,"name":"chargePaymentMode","type":["null","org.apache.fineract.avro.generic.v1.EnumOptionDataV1"]},{"default":null,"name":"paid","type":["null","boolean"]},{"default":null,"name":"loanId","type":["null","long"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"externalLoanId","type":["null","string"]},{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"customData","type":["null",{"values":"bytes","type":"map"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanChargePaidByDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanChargePaidByDataV1 {
    pub id: Option<i64>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "chargeId")]
    pub charge_id: Option<i64>,
    #[serde(rename = "transactionId")]
    pub transaction_id: Option<i64>,
    pub name: Option<String>,
}

pub const WORKING_CAPITAL_LOAN_CHARGE_PAID_BY_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanChargePaidByDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"chargeId","type":["null","long"]},{"default":null,"name":"transactionId","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanCollectionDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanCollectionDataV1 {
    /// Days since the oldest active delinquency tag.
    #[serde(rename = "delinquentDays")]
    pub delinquent_days: Option<i32>,
    #[serde(rename = "delinquentDate")]
    pub delinquent_date: Option<String>,
    #[serde(rename = "delinquentAmount")]
    pub delinquent_amount: Option<rust_decimal::Decimal>,
    /// Date of the last non-reversed repayment-like transaction (repayment, payout refund, goodwill credit, charge adjustment).
    #[serde(rename = "lastPaymentDate")]
    pub last_payment_date: Option<String>,
    #[serde(rename = "lastPaymentAmount")]
    pub last_payment_amount: Option<rust_decimal::Decimal>,
    /// Date of the last non-reversed repayment transaction.
    #[serde(rename = "lastRepaymentDate")]
    pub last_repayment_date: Option<String>,
    #[serde(rename = "lastRepaymentAmount")]
    pub last_repayment_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "delinquencyPausePeriods")]
    pub delinquency_pause_periods: Option<Vec<DelinquencyPausePeriodV1>>,
    /// Current delinquency schedule periods.
    #[serde(rename = "delinquencySchedule")]
    pub delinquency_schedule: Option<Vec<WorkingCapitalLoanDelinquencySchedulePeriodDataV1>>,
    /// Aggregated outstanding of the delinquency schedule periods; always equal to delinquentAmount.
    #[serde(rename = "totalDelinquentAmount")]
    pub total_delinquent_amount: Option<rust_decimal::Decimal>,
}

pub const WORKING_CAPITAL_LOAN_COLLECTION_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanCollectionDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"delinquentDays","doc":"Days since the oldest active delinquency tag.","type":["null","int"]},{"default":null,"name":"delinquentDate","type":["null","string"]},{"default":null,"name":"delinquentAmount","type":["null","bigdecimal"]},{"default":null,"name":"lastPaymentDate","doc":"Date of the last non-reversed repayment-like transaction (repayment, payout refund, goodwill credit, charge adjustment).","type":["null","string"]},{"default":null,"name":"lastPaymentAmount","type":["null","bigdecimal"]},{"default":null,"name":"lastRepaymentDate","doc":"Date of the last non-reversed repayment transaction.","type":["null","string"]},{"default":null,"name":"lastRepaymentAmount","type":["null","bigdecimal"]},{"default":null,"name":"delinquencyPausePeriods","type":["null",{"type":"array","items":"org.apache.fineract.avro.loan.v1.DelinquencyPausePeriodV1"}]},{"default":null,"name":"delinquencySchedule","doc":"Current delinquency schedule periods.","type":["null",{"type":"array","items":"org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanDelinquencySchedulePeriodDataV1"}]},{"default":null,"name":"totalDelinquentAmount","doc":"Aggregated outstanding of the delinquency schedule periods; always equal to delinquentAmount.","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanDelinquencyDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanDelinquencyDataV1 {
    /// What starts the delinquency clock.
    #[serde(rename = "delinquencyStartType")]
    pub delinquency_start_type: Option<StringEnumOptionDataV1>,
    #[serde(rename = "delinquencyStartDate")]
    pub delinquency_start_date: Option<String>,
    /// delinquencyStartDate shifted by the delinquency grace days; only set for the first delinquency period.
    #[serde(rename = "delinquencyEffectiveStartDate")]
    pub delinquency_effective_start_date: Option<String>,
}

pub const WORKING_CAPITAL_LOAN_DELINQUENCY_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanDelinquencyDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"delinquencyStartType","doc":"What starts the delinquency clock.","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"delinquencyStartDate","type":["null","string"]},{"default":null,"name":"delinquencyEffectiveStartDate","doc":"delinquencyStartDate shifted by the delinquency grace days; only set for the first delinquency period.","type":["null","string"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanDelinquencySchedulePeriodDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanDelinquencySchedulePeriodDataV1 {
    #[serde(rename = "periodNumber")]
    pub period_number: Option<i32>,
    #[serde(rename = "fromDate")]
    pub from_date: Option<String>,
    #[serde(rename = "toDate")]
    pub to_date: Option<String>,
    #[serde(rename = "expectedAmount")]
    pub expected_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "paidAmount")]
    pub paid_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "outstandingAmount")]
    pub outstanding_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "minPaymentCriteriaMet")]
    pub min_payment_criteria_met: Option<bool>,
    #[serde(rename = "delinquentDays")]
    pub delinquent_days: Option<i32>,
    #[serde(rename = "delinquentAmount")]
    pub delinquent_amount: Option<rust_decimal::Decimal>,
    /// Active delinquency range tags currently applied to this schedule period.
    pub tags: Option<Vec<WorkingCapitalLoanDelinquencyScheduleTagDataV1>>,
}

pub const WORKING_CAPITAL_LOAN_DELINQUENCY_SCHEDULE_PERIOD_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanDelinquencySchedulePeriodDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"periodNumber","type":["null","int"]},{"default":null,"name":"fromDate","type":["null","string"]},{"default":null,"name":"toDate","type":["null","string"]},{"default":null,"name":"expectedAmount","type":["null","bigdecimal"]},{"default":null,"name":"paidAmount","type":["null","bigdecimal"]},{"default":null,"name":"outstandingAmount","type":["null","bigdecimal"]},{"default":null,"name":"minPaymentCriteriaMet","type":["null","boolean"]},{"default":null,"name":"delinquentDays","type":["null","int"]},{"default":null,"name":"delinquentAmount","type":["null","bigdecimal"]},{"default":null,"name":"tags","doc":"Active delinquency range tags currently applied to this schedule period.","type":["null",{"type":"array","items":"org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanDelinquencyScheduleTagDataV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanDelinquencyScheduleTagDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanDelinquencyScheduleTagDataV1 {
    #[serde(rename = "rangeId")]
    pub range_id: Option<i64>,
    /// Name of the delinquency range this schedule period is currently classified into.
    pub classification: Option<String>,
    #[serde(rename = "minimumAgeDays")]
    pub minimum_age_days: Option<i32>,
    #[serde(rename = "maximumAgeDays")]
    pub maximum_age_days: Option<i32>,
    #[serde(rename = "delinquentAmount")]
    pub delinquent_amount: Option<rust_decimal::Decimal>,
}

pub const WORKING_CAPITAL_LOAN_DELINQUENCY_SCHEDULE_TAG_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanDelinquencyScheduleTagDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"rangeId","type":["null","long"]},{"default":null,"name":"classification","doc":"Name of the delinquency range this schedule period is currently classified into.","type":["null","string"]},{"default":null,"name":"minimumAgeDays","type":["null","int"]},{"default":null,"name":"maximumAgeDays","type":["null","int"]},{"default":null,"name":"delinquentAmount","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanDisbursementDetailDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanDisbursementDetailDataV1 {
    pub id: Option<i32>,
    #[serde(rename = "expectedDisbursementDate")]
    pub expected_disbursement_date: Option<String>,
    #[serde(rename = "actualDisbursementDate")]
    pub actual_disbursement_date: Option<String>,
    pub principal: Option<rust_decimal::Decimal>,
    /// Net amount disbursed to the client after up-front deductions (from the amortization model).
    #[serde(rename = "netDisbursalAmount")]
    pub net_disbursal_amount: Option<rust_decimal::Decimal>,
}

pub const WORKING_CAPITAL_LOAN_DISBURSEMENT_DETAIL_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanDisbursementDetailDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","int"]},{"default":null,"name":"expectedDisbursementDate","type":["null","string"]},{"default":null,"name":"actualDisbursementDate","type":["null","string"]},{"default":null,"name":"principal","type":["null","bigdecimal"]},{"default":null,"name":"netDisbursalAmount","doc":"Net amount disbursed to the client after up-front deductions (from the amortization model).","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanJournalEntryDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanJournalEntryDataV1 {
    pub id: Option<i64>,
    /// Identifier of the working capital loan the journal entry belongs to.
    #[serde(rename = "loanId")]
    pub loan_id: Option<i64>,
    /// Ledger transaction identifier shared by every entry of the same working capital loan transaction.
    #[serde(rename = "transactionId")]
    pub transaction_id: Option<String>,
    /// Numeric identifier of the working capital loan transaction the entry was posted for.
    #[serde(rename = "wcLoanTransactionId")]
    pub wc_loan_transaction_id: Option<i64>,
    #[serde(rename = "glAccount")]
    pub gl_account: Option<GLAccountDataV1>,
    /// DEBIT or CREDIT.
    pub r#type: Option<StringEnumOptionDataV1>,
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "currencyCode")]
    pub currency_code: Option<String>,
    #[serde(rename = "officeId")]
    pub office_id: Option<i64>,
    #[serde(rename = "transactionDate")]
    pub transaction_date: Option<String>,
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    /// True when the entry is a reversal. Entries can arrive with this flag already set.
    pub reversed: Option<bool>,
    /// Identifier of the mirror entry that reversed this one, when it was reversed.
    #[serde(rename = "reversalId")]
    pub reversal_id: Option<i64>,
    #[serde(rename = "paymentDetailData")]
    pub payment_detail_data: Option<PaymentDetailDataV1>,
}

pub const WORKING_CAPITAL_LOAN_JOURNAL_ENTRY_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanJournalEntryDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"loanId","doc":"Identifier of the working capital loan the journal entry belongs to.","type":["null","long"]},{"default":null,"name":"transactionId","doc":"Ledger transaction identifier shared by every entry of the same working capital loan transaction.","type":["null","string"]},{"default":null,"name":"wcLoanTransactionId","doc":"Numeric identifier of the working capital loan transaction the entry was posted for.","type":["null","long"]},{"default":null,"name":"glAccount","type":["null","org.apache.fineract.avro.gl.v1.GLAccountDataV1"]},{"default":null,"name":"type","doc":"DEBIT or CREDIT.","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"amount","type":["null","bigdecimal"]},{"default":null,"name":"currencyCode","type":["null","string"]},{"default":null,"name":"officeId","type":["null","long"]},{"default":null,"name":"transactionDate","type":["null","string"]},{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"reversed","doc":"True when the entry is a reversal. Entries can arrive with this flag already set.","type":["null","boolean"]},{"default":null,"name":"reversalId","doc":"Identifier of the mirror entry that reversed this one, when it was reversed.","type":["null","long"]},{"default":null,"name":"paymentDetailData","type":["null","org.apache.fineract.avro.payment.v1.PaymentDetailDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanPeriodPaymentRateChangeDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanPeriodPaymentRateChangeDataV1 {
    /// Date the new period payment rate takes effect.
    #[serde(rename = "effectiveDate")]
    pub effective_date: Option<String>,
    /// Period payment rate in force before this change.
    #[serde(rename = "previousRate")]
    pub previous_rate: Option<rust_decimal::Decimal>,
    #[serde(rename = "newRate")]
    pub new_rate: Option<rust_decimal::Decimal>,
    /// True when the change was superseded by another change on the same effective date.
    pub reversed: Option<bool>,
    #[serde(rename = "reversedOnDate")]
    pub reversed_on_date: Option<String>,
    /// Annual effective interest rate, as a percentage (e.g. 43.756245), of the schedule segment this change opened, snapshotted when the change was booked and never restated.
    #[serde(rename = "calculatedAnnualEir")]
    pub calculated_annual_eir: Option<rust_decimal::Decimal>,
    /// Payment amount per repayment period of the segment this change opened (per day on a daily schedule)
    #[serde(rename = "dailyPaymentAmount")]
    pub daily_payment_amount: Option<rust_decimal::Decimal>,
    /// Payment periods the segment this change opened runs for; not the loan's term, which spans every segment.
    #[serde(rename = "segmentTerm")]
    pub segment_term: Option<i32>,
}

pub const WORKING_CAPITAL_LOAN_PERIOD_PAYMENT_RATE_CHANGE_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanPeriodPaymentRateChangeDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"effectiveDate","doc":"Date the new period payment rate takes effect.","type":["null","string"]},{"default":null,"name":"previousRate","doc":"Period payment rate in force before this change.","type":["null","bigdecimal"]},{"default":null,"name":"newRate","type":["null","bigdecimal"]},{"default":null,"name":"reversed","doc":"True when the change was superseded by another change on the same effective date.","type":["null","boolean"]},{"default":null,"name":"reversedOnDate","type":["null","string"]},{"default":null,"name":"calculatedAnnualEir","doc":"Annual effective interest rate, as a percentage (e.g. 43.756245), of the schedule segment this change opened, snapshotted when the change was booked and never restated.","type":["null","bigdecimal"]},{"default":null,"name":"dailyPaymentAmount","doc":"Payment amount per repayment period of the segment this change opened (per day on a daily schedule)","type":["null","bigdecimal"]},{"default":null,"name":"segmentTerm","doc":"Payment periods the segment this change opened runs for; not the loan's term, which spans every segment.","type":["null","int"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanSummaryDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanSummaryDataV1 {
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "principalDisbursed")]
    pub principal_disbursed: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalAdjustments")]
    pub principal_adjustments: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalPaid")]
    pub principal_paid: Option<rust_decimal::Decimal>,
    /// Always null: working capital does not yet implement write-off.
    #[serde(rename = "principalWrittenOff")]
    pub principal_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "principalOutstanding")]
    pub principal_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesCharged")]
    pub fee_charges_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesPaid")]
    pub fee_charges_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesWaived")]
    pub fee_charges_waived: Option<rust_decimal::Decimal>,
    /// Always null: working capital does not yet implement write-off.
    #[serde(rename = "feeChargesWrittenOff")]
    pub fee_charges_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesOutstanding")]
    pub fee_charges_outstanding: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesCharged")]
    pub penalty_charges_charged: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesPaid")]
    pub penalty_charges_paid: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesWaived")]
    pub penalty_charges_waived: Option<rust_decimal::Decimal>,
    /// Always null: working capital does not yet implement write-off.
    #[serde(rename = "penaltyChargesWrittenOff")]
    pub penalty_charges_written_off: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesOutstanding")]
    pub penalty_charges_outstanding: Option<rust_decimal::Decimal>,
    /// Sum of feeChargesCharged and penaltyChargesCharged.
    #[serde(rename = "totalChargeAmount")]
    pub total_charge_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalExpectedRepayment")]
    pub total_expected_repayment: Option<rust_decimal::Decimal>,
    /// Principal, fee and penalty actually settled. Not a sum of transaction amounts.
    #[serde(rename = "totalRepayment")]
    pub total_repayment: Option<rust_decimal::Decimal>,
    /// Sum of non-reversed repayment-like transaction amounts: REPAYMENT, PAYOUT_REFUND, GOODWILL_CREDIT, CHARGE_ADJUSTMENT.
    #[serde(rename = "totalPayment")]
    pub total_payment: Option<rust_decimal::Decimal>,
    /// Sum of reversed repayment-like transaction amounts: REPAYMENT, PAYOUT_REFUND, GOODWILL_CREDIT, CHARGE_ADJUSTMENT.
    #[serde(rename = "totalPaymentReversed")]
    pub total_payment_reversed: Option<rust_decimal::Decimal>,
    /// End date of the earliest delinquency period whose minimum payment criteria was not met.
    #[serde(rename = "overdueSinceDate")]
    pub overdue_since_date: Option<String>,
    #[serde(rename = "totalOutstanding")]
    pub total_outstanding: Option<rust_decimal::Decimal>,
    /// Always null: working capital does not yet implement write-off.
    #[serde(rename = "totalWrittenOff")]
    pub total_written_off: Option<rust_decimal::Decimal>,
    /// Always null: working capital does not yet implement recovery.
    #[serde(rename = "totalRecovered")]
    pub total_recovered: Option<rust_decimal::Decimal>,
    /// Always null: working capital does not yet implement write-off.
    #[serde(rename = "writeoffReasonId")]
    pub writeoff_reason_id: Option<i64>,
    /// Always null: working capital does not yet implement write-off.
    #[serde(rename = "writeoffReason")]
    pub writeoff_reason: Option<String>,
    /// Code value id of the reason the loan was charged off, when one was recorded.
    #[serde(rename = "chargeOffReasonId")]
    pub charge_off_reason_id: Option<i64>,
    /// Name of the reason the loan was charged off, when one was recorded.
    #[serde(rename = "chargeOffReason")]
    pub charge_off_reason: Option<String>,
    #[serde(rename = "totalPayoutRefund")]
    pub total_payout_refund: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalPayoutRefundReversed")]
    pub total_payout_refund_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalGoodwillCredit")]
    pub total_goodwill_credit: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalGoodwillCreditReversed")]
    pub total_goodwill_credit_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalChargeAdjustment")]
    pub total_charge_adjustment: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalChargeAdjustmentReversed")]
    pub total_charge_adjustment_reversed: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCreditBalanceRefund")]
    pub total_credit_balance_refund: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalCreditBalanceRefundReversed")]
    pub total_credit_balance_refund_reversed: Option<rust_decimal::Decimal>,
    /// Sum of non-reversed REPAYMENT transaction amounts.
    #[serde(rename = "totalRepaymentTransaction")]
    pub total_repayment_transaction: Option<rust_decimal::Decimal>,
    /// Sum of reversed REPAYMENT transaction amounts.
    #[serde(rename = "totalRepaymentTransactionReversed")]
    pub total_repayment_transaction_reversed: Option<rust_decimal::Decimal>,
    /// Disbursed amount plus the discount fee.
    #[serde(rename = "totalPrincipal")]
    pub total_principal: Option<rust_decimal::Decimal>,
    #[serde(rename = "realizedIncomeFromDiscountFee")]
    pub realized_income_from_discount_fee: Option<rust_decimal::Decimal>,
    #[serde(rename = "unrealizedIncomeFromDiscountFee")]
    pub unrealized_income_from_discount_fee: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalDiscountFee")]
    pub total_discount_fee: Option<rust_decimal::Decimal>,
    #[serde(rename = "totalDiscountFeeAdjustment")]
    pub total_discount_fee_adjustment: Option<rust_decimal::Decimal>,
}

pub const WORKING_CAPITAL_LOAN_SUMMARY_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanSummaryDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"principalDisbursed","type":["null","bigdecimal"]},{"default":null,"name":"principalAdjustments","type":["null","bigdecimal"]},{"default":null,"name":"principalPaid","type":["null","bigdecimal"]},{"default":null,"name":"principalWrittenOff","doc":"Always null: working capital does not yet implement write-off.","type":["null","bigdecimal"]},{"default":null,"name":"principalOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesCharged","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesPaid","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesWaived","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesWrittenOff","doc":"Always null: working capital does not yet implement write-off.","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesCharged","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesPaid","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesWaived","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesWrittenOff","doc":"Always null: working capital does not yet implement write-off.","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"totalChargeAmount","doc":"Sum of feeChargesCharged and penaltyChargesCharged.","type":["null","bigdecimal"]},{"default":null,"name":"totalExpectedRepayment","type":["null","bigdecimal"]},{"default":null,"name":"totalRepayment","doc":"Principal, fee and penalty actually settled. Not a sum of transaction amounts.","type":["null","bigdecimal"]},{"default":null,"name":"totalPayment","doc":"Sum of non-reversed repayment-like transaction amounts: REPAYMENT, PAYOUT_REFUND, GOODWILL_CREDIT, CHARGE_ADJUSTMENT.","type":["null","bigdecimal"]},{"default":null,"name":"totalPaymentReversed","doc":"Sum of reversed repayment-like transaction amounts: REPAYMENT, PAYOUT_REFUND, GOODWILL_CREDIT, CHARGE_ADJUSTMENT.","type":["null","bigdecimal"]},{"default":null,"name":"overdueSinceDate","doc":"End date of the earliest delinquency period whose minimum payment criteria was not met.","type":["null","string"]},{"default":null,"name":"totalOutstanding","type":["null","bigdecimal"]},{"default":null,"name":"totalWrittenOff","doc":"Always null: working capital does not yet implement write-off.","type":["null","bigdecimal"]},{"default":null,"name":"totalRecovered","doc":"Always null: working capital does not yet implement recovery.","type":["null","bigdecimal"]},{"default":null,"name":"writeoffReasonId","doc":"Always null: working capital does not yet implement write-off.","type":["null","long"]},{"default":null,"name":"writeoffReason","doc":"Always null: working capital does not yet implement write-off.","type":["null","string"]},{"default":null,"name":"chargeOffReasonId","doc":"Code value id of the reason the loan was charged off, when one was recorded.","type":["null","long"]},{"default":null,"name":"chargeOffReason","doc":"Name of the reason the loan was charged off, when one was recorded.","type":["null","string"]},{"default":null,"name":"totalPayoutRefund","type":["null","bigdecimal"]},{"default":null,"name":"totalPayoutRefundReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalGoodwillCredit","type":["null","bigdecimal"]},{"default":null,"name":"totalGoodwillCreditReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalChargeAdjustment","type":["null","bigdecimal"]},{"default":null,"name":"totalChargeAdjustmentReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalCreditBalanceRefund","type":["null","bigdecimal"]},{"default":null,"name":"totalCreditBalanceRefundReversed","type":["null","bigdecimal"]},{"default":null,"name":"totalRepaymentTransaction","doc":"Sum of non-reversed REPAYMENT transaction amounts.","type":["null","bigdecimal"]},{"default":null,"name":"totalRepaymentTransactionReversed","doc":"Sum of reversed REPAYMENT transaction amounts.","type":["null","bigdecimal"]},{"default":null,"name":"totalPrincipal","doc":"Disbursed amount plus the discount fee.","type":["null","bigdecimal"]},{"default":null,"name":"realizedIncomeFromDiscountFee","type":["null","bigdecimal"]},{"default":null,"name":"unrealizedIncomeFromDiscountFee","type":["null","bigdecimal"]},{"default":null,"name":"totalDiscountFee","type":["null","bigdecimal"]},{"default":null,"name":"totalDiscountFeeAdjustment","type":["null","bigdecimal"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanTransactionAdjustmentDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanTransactionAdjustmentDataV1 {
    #[serde(rename = "transactionToAdjust")]
    pub transaction_to_adjust: Option<WorkingCapitalLoanTransactionDataV1>,
    #[serde(rename = "newTransactionDetail")]
    pub new_transaction_detail: Option<WorkingCapitalLoanTransactionDataV1>,
}

pub const WORKING_CAPITAL_LOAN_TRANSACTION_ADJUSTMENT_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanTransactionAdjustmentDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"transactionToAdjust","type":["null","org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanTransactionDataV1"]},{"default":null,"name":"newTransactionDetail","type":["null","org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanTransactionDataV1"]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanTransactionDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalLoanTransactionDataV1 {
    pub id: Option<i64>,
    #[serde(rename = "wcLoanId")]
    pub wc_loan_id: Option<i64>,
    #[serde(rename = "externalLoanId")]
    pub external_loan_id: Option<String>,
    pub r#type: Option<LoanTransactionEnumDataV1>,
    #[serde(rename = "transactionDate")]
    pub transaction_date: Option<String>,
    #[serde(rename = "submittedOnDate")]
    pub submitted_on_date: Option<String>,
    pub currency: Option<CurrencyDataV1>,
    #[serde(rename = "transactionAmount")]
    pub transaction_amount: Option<rust_decimal::Decimal>,
    #[serde(rename = "paymentDetailData")]
    pub payment_detail_data: Option<PaymentDetailDataV1>,
    pub classification: Option<CodeValueDataV1>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    pub reversed: Option<bool>,
    #[serde(rename = "reversalExternalId")]
    pub reversal_external_id: Option<String>,
    #[serde(rename = "reversedOnDate")]
    pub reversed_on_date: Option<String>,
    #[serde(rename = "principalPortion")]
    pub principal_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "feeChargesPortion")]
    pub fee_charges_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "penaltyChargesPortion")]
    pub penalty_charges_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "overpaymentPortion")]
    pub overpayment_portion: Option<rust_decimal::Decimal>,
    #[serde(rename = "chargePaidByList")]
    pub charge_paid_by_list: Option<Vec<WorkingCapitalLoanChargePaidByDataV1>>,
}

pub const WORKING_CAPITAL_LOAN_TRANSACTION_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalLoanTransactionDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"wcLoanId","type":["null","long"]},{"default":null,"name":"externalLoanId","type":["null","string"]},{"default":null,"name":"type","type":["null","org.apache.fineract.avro.loan.v1.LoanTransactionEnumDataV1"]},{"default":null,"name":"transactionDate","type":["null","string"]},{"default":null,"name":"submittedOnDate","type":["null","string"]},{"default":null,"name":"currency","type":["null","org.apache.fineract.avro.generic.v1.CurrencyDataV1"]},{"default":null,"name":"transactionAmount","type":["null","bigdecimal"]},{"default":null,"name":"paymentDetailData","type":["null","org.apache.fineract.avro.payment.v1.PaymentDetailDataV1"]},{"default":null,"name":"classification","type":["null","org.apache.fineract.avro.generic.v1.CodeValueDataV1"]},{"default":null,"name":"externalId","type":["null","string"]},{"default":null,"name":"reversed","type":["null","boolean"]},{"default":null,"name":"reversalExternalId","type":["null","string"]},{"default":null,"name":"reversedOnDate","type":["null","string"]},{"default":null,"name":"principalPortion","type":["null","bigdecimal"]},{"default":null,"name":"feeChargesPortion","type":["null","bigdecimal"]},{"default":null,"name":"penaltyChargesPortion","type":["null","bigdecimal"]},{"default":null,"name":"overpaymentPortion","type":["null","bigdecimal"]},{"default":null,"name":"chargePaidByList","type":["null",{"type":"array","items":"org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalLoanChargePaidByDataV1"}]}]}"#;

/// Avro schema `org.apache.fineract.avro.workingcapitalloan.v1.WorkingCapitalNearBreachDataV1` (Namespace: `org.apache.fineract.avro.workingcapitalloan.v1`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCapitalNearBreachDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub frequency: Option<i32>,
    #[serde(rename = "frequencyType")]
    pub frequency_type: Option<StringEnumOptionDataV1>,
    pub threshold: Option<rust_decimal::Decimal>,
}

pub const WORKING_CAPITAL_NEAR_BREACH_DATA_V1_AVSC: &str = r#"{"name":"WorkingCapitalNearBreachDataV1","namespace":"org.apache.fineract.avro.workingcapitalloan.v1","type":"record","fields":[{"default":null,"name":"id","type":["null","long"]},{"default":null,"name":"name","type":["null","string"]},{"default":null,"name":"frequency","type":["null","int"]},{"default":null,"name":"frequencyType","type":["null","org.apache.fineract.avro.generic.v1.StringEnumOptionDataV1"]},{"default":null,"name":"threshold","type":["null","bigdecimal"]}]}"#;
