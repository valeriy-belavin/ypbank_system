//! Format conversion traits.
//!
//! This module provides conversion between different financial formats
//! using Rust's `From` trait.

use crate::camt053_format::Camt053Statement;
use crate::mt940_format::Mt940Statement;

/// Convert from MT940 to CAMT.053 format.
impl From<Mt940Statement> for Camt053Statement {
    fn from(mt940: Mt940Statement) -> Self {
        // The conversion is straightforward since both formats
        // use the same underlying Statement structure.
        // Missing information in MT940 is represented with placeholders or None.
        let mut statement = mt940.statement.clone();

        // Ensure statement has creation date
        if statement.creation_date.is_none() {
            statement.creation_date = Some(chrono::Utc::now().date_naive());
        }

        Camt053Statement { statement }
    }
}

/// Convert from CAMT.053 to MT940 format.
impl From<Camt053Statement> for Mt940Statement {
    fn from(camt053: Camt053Statement) -> Self {
        // When converting from CAMT.053 to MT940, some information
        // that doesn't fit in MT940 can be placed in the :86: field
        // (Information to Account Owner).
        let mut statement = camt053.statement.clone();

        // Combine additional info into transaction descriptions for MT940
        for transaction in &mut statement.transactions {
            if let Some(ref addtl) = transaction.additional_info {
                if !transaction.description.is_empty() {
                    transaction.description.push_str(" | ");
                }
                transaction.description.push_str(addtl);
            }

            // Add counterparty info to description if present
            if let Some(ref name) = transaction.counterparty_name {
                if !transaction.description.is_empty() {
                    transaction.description.push_str(" | ");
                }
                transaction.description.push_str("Counterparty: ");
                transaction.description.push_str(name);
            }
        }

        Mt940Statement { statement }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Statement, Transaction, DebitCredit};
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use chrono::NaiveDate;

    #[test]
    fn test_mt940_to_camt053() {
        let mut statement = Statement::new("TEST001".into(), "ACC123".into(), "USD".into());
        statement.transactions.push(Transaction {
            reference: "REF001".into(),
            date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            value_date: Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            amount: Decimal::from_str("100.50").unwrap(),
            currency: "USD".into(),
            debit_credit: DebitCredit::Credit,
            account: None,
            counterparty_account: Some("ACC456".into()),
            counterparty_name: Some("Test Company".into()),
            bank_identifier: Some("TESTUS33".into()),
            description: "Test transaction".into(),
            additional_info: None,
        });

        let mt940 = Mt940Statement { statement };
        let camt053: Camt053Statement = mt940.into();

        assert_eq!(camt053.statement.statement_id, "TEST001");
        assert_eq!(camt053.statement.transactions.len(), 1);
    }

    #[test]
    fn test_camt053_to_mt940() {
        let mut statement = Statement::new("TEST002".into(), "ACC789".into(), "EUR".into());
        statement.transactions.push(Transaction {
            reference: "REF002".into(),
            date: NaiveDate::from_ymd_opt(2024, 2, 20).unwrap(),
            value_date: Some(NaiveDate::from_ymd_opt(2024, 2, 20).unwrap()),
            amount: Decimal::from_str("250.75").unwrap(),
            currency: "EUR".into(),
            debit_credit: DebitCredit::Debit,
            account: None,
            counterparty_account: Some("ACC999".into()),
            counterparty_name: Some("Another Company".into()),
            bank_identifier: Some("TESTDE33".into()),
            description: "Another test".into(),
            additional_info: Some("Extra info".into()),
        });

        let camt053 = Camt053Statement { statement };
        let mt940: Mt940Statement = camt053.into();

        assert_eq!(mt940.statement.statement_id, "TEST002");
        assert_eq!(mt940.statement.transactions.len(), 1);
        assert!(mt940.statement.transactions[0].description.contains("Extra info"));
    }
}
