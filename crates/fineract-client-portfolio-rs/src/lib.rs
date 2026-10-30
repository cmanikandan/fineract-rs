//! `fineract-client-portfolio-rs` — Client KYC, Groups, Centers, JLG Meetings,
//! Funds, Payment Details, Notes, Standing Instructions, and Account Transfers.

pub mod account_transfer;
pub mod client_kyc;
pub mod group_and_center;

pub use account_transfer::*;
pub use client_kyc::*;
pub use group_and_center::*;
