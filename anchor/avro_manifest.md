# Apache Fineract Avro Event Schemas Manifest (101 Schemas)

| Rust Struct | Namespace | Fields |
| :--- | :--- | :--- |
| `BulkMessageItemV1` | `org.apache.fineract.avro` | `id, type, category, dataschema, data` |
| `BulkMessagePayloadV1` | `org.apache.fineract.avro` | `datas` |
| `MessageV1` | `org.apache.fineract.avro` | `id, source, type, category, createdAt, businessDate, tenantId, idempotencyKey...` |
| `ClientCollateralManagementDataV1` | `org.apache.fineract.avro.client.v1` | `quantity, id, pctToBase, unitPrice, total, totalCollateral, name` |
| `ClientDataV1` | `org.apache.fineract.avro.client.v1` | `id, accountNo, externalId, status, subStatus, active, activationDate, firstname...` |
| `ClientTimelineDataV1` | `org.apache.fineract.avro.client.v1` | `submittedOnDate, submittedByUsername, submittedByFirstname, submittedByLastname, activatedOnDate, activatedByUsername, activatedByFirstname, activatedByLastname...` |
| `DocumentDataV1` | `org.apache.fineract.avro.document.v1` | `id, parentEntityType, parentEntityId, name, fileName, size, type, description...` |
| `FixedDepositAccountDataV1` | `org.apache.fineract.avro.fixeddeposit.v1` | `id, accountNo, externalId, groupId, groupName, clientId, clientName, depositProductId...` |
| `CalendarDataV1` | `org.apache.fineract.avro.generic.v1` | `id, calendarInstanceId, entityId, entityType, title, description, location, startDate...` |
| `CodeValueDataV1` | `org.apache.fineract.avro.generic.v1` | `id, name, position, description, active, mandatory` |
| `CommandProcessingResultV1` | `org.apache.fineract.avro.generic.v1` | `commandId, officeId, groupId, clientId, loanId, savingsId, resourceId, subResourceId...` |
| `CurrencyDataV1` | `org.apache.fineract.avro.generic.v1` | `code, name, decimalPlaces, inMultiplesOf, displaySymbol, nameCode, displayLabel` |
| `EnumOptionDataV1` | `org.apache.fineract.avro.generic.v1` | `id, code, value` |
| `StringEnumOptionDataV1` | `org.apache.fineract.avro.generic.v1` | `id, code, value` |
| `GLAccountDataV1` | `org.apache.fineract.avro.gl.v1` | `id, name, parentId, glCode, disabled, manualEntriesAllowed, type, usage...` |
| `GroupGeneralDataV1` | `org.apache.fineract.avro.group.v1` | `id, accountNo, name, externalId, status, active, activationDate, officeId...` |
| `GroupRoleDataV1` | `org.apache.fineract.avro.group.v1` | `id, role, clientId, clientName` |
| `CollectionDataV1` | `org.apache.fineract.avro.loan.v1` | `availableDisbursementAmount, availableDisbursementAmountWithOverApplied, pastDueDays, pastDueDate, nextPaymentDueDate, nextPaymentAmount, delinquentDays, delinquentDate...` |
| `DelinquencyBucketDataV1` | `org.apache.fineract.avro.loan.v1` | `id, name, ranges` |
| `DelinquencyPausePeriodV1` | `org.apache.fineract.avro.loan.v1` | `active, pausePeriodStart, pausePeriodEnd` |
| `DelinquencyRangeDataV1` | `org.apache.fineract.avro.loan.v1` | `id, classification, minimumAgeDays, maximumAgeDays` |
| `DisbursementDataV1` | `org.apache.fineract.avro.loan.v1` | `id, expectedDisbursementDate, actualDisbursementDate, principal, netDisbursalAmount, loanChargeId, chargeAmount, waivedChargeAmount` |
| `LoanAccountDataV1` | `org.apache.fineract.avro.loan.v1` | `id, accountNo, externalId, externalOwnerId, settlementDate, purchasePriceRatio, status, subStatus...` |
| `LoanAccountDelinquencyRangeDataV1` | `org.apache.fineract.avro.loan.v1` | `loanId, loanAccountNo, loanExternalId, delinquencyRange, charges, currency, amount, delinquentDate...` |
| `LoanAccountStayedLockedDataV1` | `org.apache.fineract.avro.loan.v1` | `id, externalId, accountNo` |
| `LoanAccountSummaryDataV1` | `org.apache.fineract.avro.loan.v1` | `id, accountNo, parentAccountNumber, externalId, productId, productName, shortProductName, status...` |
| `LoanAccountsStayedLockedDataV1` | `org.apache.fineract.avro.loan.v1` | `loanAccounts` |
| `LoanAmountDataV1` | `org.apache.fineract.avro.loan.v1` | `totalAmount, principalAmount, interestAmount, feeAmount, penaltyAmount` |
| `LoanApplicationTimelineDataV1` | `org.apache.fineract.avro.loan.v1` | `submittedOnDate, submittedByUsername, submittedByFirstname, submittedByLastname, rejectedOnDate, rejectedByUsername, rejectedByFirstname, rejectedByLastname...` |
| `LoanChargeDataRangeViewV1` | `org.apache.fineract.avro.loan.v1` | `id, name, currency, amount` |
| `LoanChargeDataV1` | `org.apache.fineract.avro.loan.v1` | `id, chargeId, name, chargeTimeType, dueDate, chargeCalculationType, percentage, amountPercentageAppliedTo...` |
| `LoanChargeDeletedV1` | `org.apache.fineract.avro.loan.v1` | `id, chargeId` |
| `LoanChargePaidByDataV1` | `org.apache.fineract.avro.loan.v1` | `id, amount, installmentNumber, chargeId, transactionId, name` |
| `LoanInstallmentChargeDataV1` | `org.apache.fineract.avro.loan.v1` | `installmentNumber, dueDate, amount, amountOutstanding, amountWaived, paid, waived, amountAccrued...` |
| `LoanInstallmentDelinquencyBucketDataV1` | `org.apache.fineract.avro.loan.v1` | `delinquencyRange, amount, charges, currency` |
| `LoanInterestRecalculationDataV1` | `org.apache.fineract.avro.loan.v1` | `id, loanId, interestRecalculationCompoundingType, rescheduleStrategyType, calendarData, recalculationRestFrequencyType, recalculationRestFrequencyInterval, recalculationRestFrequencyNthDay...` |
| `LoanOwnershipTransferDataV1` | `org.apache.fineract.avro.loan.v1` | `loanId, loanExternalId, type, transferExternalId, transferExternalGroupId, submittedDate, assetOwnerExternalId, previousOwnerExternalId...` |
| `LoanProductBorrowerCycleVariationDataV1` | `org.apache.fineract.avro.loan.v1` | `id, borrowerCycleNumber, paramType, valueConditionType, minValue, maxValue, defaultValue` |
| `LoanProductDataV1` | `org.apache.fineract.avro.loan.v1` | `id, name, shortName, description, fundId, fundName, includeInBorrowerCycle, useBorrowerCycle...` |
| `LoanProductGuaranteeDataV1` | `org.apache.fineract.avro.loan.v1` | `id, productId, mandatoryGuarantee, minimumGuaranteeFromOwnFunds, minimumGuaranteeFromGuarantor` |
| `LoanProductInterestRecalculationDataV1` | `org.apache.fineract.avro.loan.v1` | `id, productId, interestRecalculationCompoundingType, rescheduleStrategyType, recalculationRestFrequencyType, recalculationRestFrequencyInterval, recalculationRestFrequencyNthDay, recalculationRestFrequencyWeekday...` |
| `LoanRepaymentDueDataV1` | `org.apache.fineract.avro.loan.v1` | `loanId, loanAccountNo, loanExternalId, currency, installment, pastDueAmount, customData, originators` |
| `LoanScheduleDataV1` | `org.apache.fineract.avro.loan.v1` | `currency, loanTermInDays, totalPrincipalDisbursed, totalPrincipalExpected, totalPrincipalPaid, totalInterestCharged, totalFeeChargesCharged, totalPenaltyChargesCharged...` |
| `LoanSchedulePeriodDataV1` | `org.apache.fineract.avro.loan.v1` | `period, fromDate, dueDate, obligationsMetOnDate, complete, daysInPeriod, principalDisbursed, principalOriginalDue...` |
| `LoanStatusEnumDataV1` | `org.apache.fineract.avro.loan.v1` | `id, code, value, pendingApproval, waitingForDisbursal, active, closedObligationsMet, closedWrittenOff...` |
| `LoanSummaryDataV1` | `org.apache.fineract.avro.loan.v1` | `currency, principalDisbursed, principalAdjustments, principalPaid, principalWrittenOff, principalOutstanding, principalOverdue, interestCharged...` |
| `LoanTermVariationsDataV1` | `org.apache.fineract.avro.loan.v1` | `id, termType, termVariationApplicableFrom, decimalValue, dateValue, isSpecificToInstallment, isProcessed` |
| `LoanTransactionAdjustmentDataV1` | `org.apache.fineract.avro.loan.v1` | `transactionToAdjust, newTransactionDetail, customData` |
| `LoanTransactionDataV1` | `org.apache.fineract.avro.loan.v1` | `id, loanId, externalLoanId, officeId, officeName, type, date, currency...` |
| `LoanTransactionEnumDataV1` | `org.apache.fineract.avro.loan.v1` | `id, code, value, disbursement, repaymentAtDisbursement, repayment, merchantIssuedRefund, payoutRefund...` |
| `LoanTransactionFlagsDataV1` | `org.apache.fineract.avro.loan.v1` | `changedTerms` |
| `LoanTransactionRelationDataV1` | `org.apache.fineract.avro.loan.v1` | `fromLoanTransaction, toLoanTransaction, toLoanCharge, amount, relationType, paymentType` |
| `OriginatorDetailsV1` | `org.apache.fineract.avro.loan.v1` | `id, externalId, name, status, originatorType, channelType` |
| `RepaymentDueDataV1` | `org.apache.fineract.avro.loan.v1` | `installmentNumber, installmentDueDate, principalAmountDue, interestAmountDue, feeChargeAmountDue, penaltyChargeAmountDue, totalAmountDue` |
| `RepaymentPastDueDataV1` | `org.apache.fineract.avro.loan.v1` | `totalAmount, principalAmount, interestAmount, feeAmount, penaltyAmount` |
| `UnpaidChargeDataV1` | `org.apache.fineract.avro.loan.v1` | `chargeId, chargeName, outstandingAmount` |
| `OfficeDataV1` | `org.apache.fineract.avro.office.v1` | `id, name, nameDecorated, externalId, openingDate, hierarchy, parentId, parentName...` |
| `PaymentDetailDataV1` | `org.apache.fineract.avro.payment.v1` | `id, paymentType, accountNumber, checkNumber, routingCode, receiptNumber, bankNumber` |
| `PaymentTypeDataV1` | `org.apache.fineract.avro.payment.v1` | `id, name, description, isCashPayment, position, codeName, isSystemDefined` |
| `ChargeDataV1` | `org.apache.fineract.avro.portfolio.v1` | `id, name, active, penalty, freeWithdrawal, freeWithdrawalChargeFrequency, restartFrequency, restartFrequencyEnum...` |
| `PortfolioAccountDataV1` | `org.apache.fineract.avro.portfolio.v1` | `id, accountNo, externalId, groupId, groupName, clientId, clientName, productId...` |
| `RateDataV1` | `org.apache.fineract.avro.portfolio.v1` | `id, name, percentage, productApply, active` |
| `RecurringDepositAccountDataV1` | `org.apache.fineract.avro.recurringdeposit.v1` | `id, accountNo, externalId, groupId, groupName, clientId, clientName, depositProductId...` |
| `AccountTransferDataV1` | `org.apache.fineract.avro.savings.v1` | `id, reversed, currency, transferAmount, transferDate, transferDescription, fromOffice, fromClient...` |
| `SavingsAccountApplicationTimelineDataV1` | `org.apache.fineract.avro.savings.v1` | `submittedOnDate, submittedByUsername, submittedByFirstname, submittedByLastname, rejectedOnDate, rejectedByUsername, rejectedByFirstname, rejectedByLastname...` |
| `SavingsAccountChargeDataV1` | `org.apache.fineract.avro.savings.v1` | `id, chargeId, accountId, name, chargeTimeType, dueDate, feeOnMonthDay, feeInterval...` |
| `SavingsAccountChargesPaidByDataV1` | `org.apache.fineract.avro.savings.v1` | `chargeId, amount, savingsAccountChargeData` |
| `SavingsAccountDataV1` | `org.apache.fineract.avro.savings.v1` | `id, accountNo, depositType, externalId, groupId, groupName, clientId, clientName...` |
| `SavingsAccountStatusEnumDataV1` | `org.apache.fineract.avro.savings.v1` | `id, code, value, submittedAndPendingApproval, approved, rejected, withdrawnByApplicant, active...` |
| `SavingsAccountStayedLockedDataV1` | `org.apache.fineract.avro.savings.v1` | `id, externalId, accountNo` |
| `SavingsAccountSubStatusEnumDataV1` | `org.apache.fineract.avro.savings.v1` | `id, code, value, none, inactive, dormant, escheat, block...` |
| `SavingsAccountSummaryDataV1` | `org.apache.fineract.avro.savings.v1` | `currency, totalDeposits, totalWithdrawals, totalWithdrawalFees, totalAnnualFees, totalInterestEarned, totalInterestPosted, accountBalance...` |
| `SavingsAccountTransactionDataV1` | `org.apache.fineract.avro.savings.v1` | `id, transactionType, accountId, accountNo, date, currency, paymentDetailData, amount...` |
| `SavingsAccountTransactionEnumDataV1` | `org.apache.fineract.avro.savings.v1` | `id, code, value, deposit, dividendPayout, withdrawal, interestPosting, feeDeduction...` |
| `SavingsAccountsStayedLockedDataV1` | `org.apache.fineract.avro.savings.v1` | `savingsAccounts` |
| `ShareAccountApplicationTimelineDataV1` | `org.apache.fineract.avro.share.v1` | `submittedOnDate, submittedByUsername, submittedByFirstname, submittedByLastname, rejectedDate, rejectedByUsername, rejectedByFirstname, rejectedByLastname...` |
| `ShareAccountDataV1` | `org.apache.fineract.avro.share.v1` | `id, accountNo, externalId, savingsAccountNumber, clientId, clientName, defaultShares, productId...` |
| `ShareAccountStatusEnumDataV1` | `org.apache.fineract.avro.share.v1` | `id, code, value, submittedAndPendingApproval, approved, rejected, active, closed` |
| `ShareAccountSummaryDataV1` | `org.apache.fineract.avro.share.v1` | `id, accountNo, totalApprovedShares, totalPendingForApprovalShares, externalId, productId, productName, shortProductName...` |
| `ShareAccountTransactionDataV1` | `org.apache.fineract.avro.share.v1` | `id, accountId, purchasedDate, numberOfShares, purchasedPrice, status, type, amount...` |
| `ShareProductDataV1` | `org.apache.fineract.avro.share.v1` | `id, name, shortName, description, externalId, currency, totalShares, totalSharesIssued...` |
| `ShareProductMarketPriceDataV1` | `org.apache.fineract.avro.share.v1` | `id, fromDate, shareValue` |
| `WorkingCapitalBreachDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, name, breachFrequency, breachFrequencyType, breachAmountCalculationType, breachAmount, breachGraceDays, breachStartDate...` |
| `WorkingCapitalLoanAccountDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, accountNo, externalId, status, clientId, clientAccountNo, clientName, clientOfficeId...` |
| `WorkingCapitalLoanAccountStayedLockedDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, externalId, accountNo` |
| `WorkingCapitalLoanAccountsStayedLockedDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `workingCapitalLoanAccounts` |
| `WorkingCapitalLoanBreachSchedulePeriodDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `periodNumber, fromDate, toDate, numberOfDays, minPaymentAmount, outstandingAmount, paidAmount, nearBreach...` |
| `WorkingCapitalLoanChargeDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, chargeId, name, chargeTimeType, dueDate, chargeCalculationType, currency, amount...` |
| `WorkingCapitalLoanChargePaidByDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, amount, chargeId, transactionId, name` |
| `WorkingCapitalLoanCollectionDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `delinquentDays, delinquentDate, delinquentAmount, lastPaymentDate, lastPaymentAmount, lastRepaymentDate, lastRepaymentAmount, delinquencyPausePeriods...` |
| `WorkingCapitalLoanDelinquencyDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `delinquencyStartType, delinquencyStartDate, delinquencyEffectiveStartDate` |
| `WorkingCapitalLoanDelinquencySchedulePeriodDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `periodNumber, fromDate, toDate, expectedAmount, paidAmount, outstandingAmount, minPaymentCriteriaMet, delinquentDays...` |
| `WorkingCapitalLoanDelinquencyScheduleTagDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `rangeId, classification, minimumAgeDays, maximumAgeDays, delinquentAmount` |
| `WorkingCapitalLoanDisbursementDetailDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, expectedDisbursementDate, actualDisbursementDate, principal, netDisbursalAmount` |
| `WorkingCapitalLoanJournalEntryDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, loanId, transactionId, wcLoanTransactionId, glAccount, type, amount, currencyCode...` |
| `WorkingCapitalLoanPeriodPaymentRateChangeDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `effectiveDate, previousRate, newRate, reversed, reversedOnDate, calculatedAnnualEir, dailyPaymentAmount, segmentTerm` |
| `WorkingCapitalLoanSummaryDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `currency, principalDisbursed, principalAdjustments, principalPaid, principalWrittenOff, principalOutstanding, feeChargesCharged, feeChargesPaid...` |
| `WorkingCapitalLoanTransactionAdjustmentDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `transactionToAdjust, newTransactionDetail` |
| `WorkingCapitalLoanTransactionDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, wcLoanId, externalLoanId, type, transactionDate, submittedOnDate, currency, transactionAmount...` |
| `WorkingCapitalNearBreachDataV1` | `org.apache.fineract.avro.workingcapitalloan.v1` | `id, name, frequency, frequencyType, threshold` |