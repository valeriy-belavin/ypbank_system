//! Common types used across different financial formats.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Represents a financial transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction reference.
    pub reference: String,

    /// Date of the transaction.
    pub date: NaiveDate,

    /// Valuation date (value date).
    pub value_date: Option<NaiveDate>,

    /// Transaction amount.
    pub amount: Decimal,

    /// Currency code (e.g., USD, EUR, RUB).
    pub currency: String,

    /// Debit (D) or Credit (C) indicator.
    pub debit_credit: DebitCredit,

    /// Account identification.
    pub account: Option<String>,

    /// Counterparty account.
    pub counterparty_account: Option<String>,

    /// Counterparty name.
    pub counterparty_name: Option<String>,

    /// Bank identifier (BIC).
    pub bank_identifier: Option<String>,

    /// Transaction description/purpose.
    pub description: String,

    /// Additional information.
    pub additional_info: Option<String>,
}

/// Debit/Credit indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebitCredit {
    /// Debit transaction (outgoing).
    Debit,
    /// Credit transaction (incoming).
    Credit,
}

impl FromStr for DebitCredit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "D" | "DBIT" | "DEBIT" => Ok(DebitCredit::Debit),
            "C" | "CRDT" | "CREDIT" => Ok(DebitCredit::Credit),
            _ => Err(format!("Invalid debit/credit indicator: {}", s)),
        }
    }
}

impl DebitCredit {
    /// Parse from string representation (deprecated - use FromStr instead).
    #[deprecated(since = "0.1.0", note = "Use FromStr::from_str instead")]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// Convert to string representation.
    pub fn to_string(&self) -> &'static str {
        match self {
            DebitCredit::Debit => "D",
            DebitCredit::Credit => "C",
        }
    }

    /// Convert to ISO 20022 format.
    pub fn to_iso_format(&self) -> &'static str {
        match self {
            DebitCredit::Debit => "DBIT",
            DebitCredit::Credit => "CRDT",
        }
    }
}

/// Account statement balance information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    /// Balance type (opening, closing, etc.).
    pub balance_type: BalanceType,

    /// Balance amount.
    pub amount: Decimal,

    /// Currency code.
    pub currency: String,

    /// Debit/Credit indicator.
    pub debit_credit: DebitCredit,

    /// Date of the balance.
    pub date: NaiveDate,
}

/// Types of balance in a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceType {
    /// Opening balance.
    Opening,
    /// Closing balance.
    Closing,
    /// Intermediate balance.
    Intermediate,
    /// Forward available balance.
    ForwardAvailable,
}

/// Account statement containing transactions and balances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Statement {
    /// Statement identification.
    pub statement_id: String,

    /// Account identification.
    pub account: String,

    /// Statement sequence number.
    pub sequence_number: Option<String>,

    /// Account owner/holder name.
    pub account_holder: Option<String>,

    /// Opening balance.
    pub opening_balance: Option<Balance>,

    /// Closing balance.
    pub closing_balance: Option<Balance>,

    /// List of transactions.
    pub transactions: Vec<Transaction>,

    /// Currency code for the account.
    pub currency: String,

    /// Statement creation date.
    pub creation_date: Option<NaiveDate>,

    /// From date for the statement period.
    pub from_date: Option<NaiveDate>,

    /// To date for the statement period.
    pub to_date: Option<NaiveDate>,
}

impl Statement {
    /// Create a new statement with basic information.
    pub fn new(statement_id: String, account: String, currency: String) -> Self {
        Self {
            statement_id,
            account,
            currency,
            sequence_number: None,
            account_holder: None,
            opening_balance: None,
            closing_balance: None,
            transactions: Vec::new(),
            creation_date: None,
            from_date: None,
            to_date: None,
        }
    }

    /// Add a transaction to the statement.
    pub fn add_transaction(&mut self, transaction: Transaction) {
        self.transactions.push(transaction);
    }
}
