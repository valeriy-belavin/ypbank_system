//! CSV format parser and serializer.
//!
//! This module provides parsing and writing capabilities for CSV bank statements.

use crate::error::{Error, Result};
use crate::types::{DebitCredit, Statement, Transaction};
use chrono::NaiveDate;
use csv::{Reader, Writer};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::str::FromStr;

/// Represents a CSV statement.
#[derive(Debug, Clone, PartialEq)]
pub struct CsvStatement {
    /// The underlying statement data.
    pub statement: Statement,
}

/// CSV transaction record structure.
#[derive(Debug, Serialize, Deserialize)]
struct CsvRecord {
    #[serde(rename = "Дата проводки", alias = "Date", alias = "date")]
    date: String,
    #[serde(rename = "Счет Дебет", alias = "Debit Account", alias = "debit_account", default)]
    debit_account: String,
    #[serde(rename = "Счет Кредит", alias = "Credit Account", alias = "credit_account", default)]
    credit_account: String,
    #[serde(rename = "Сумма по дебету", alias = "Debit Amount", alias = "debit_amount", default)]
    debit_amount: String,
    #[serde(rename = "Сумма по кредиту", alias = "Credit Amount", alias = "credit_amount", default)]
    credit_amount: String,
    #[serde(rename = "№ документа", alias = "Document No", alias = "reference", default)]
    reference: String,
    #[serde(rename = "Назначение платежа", alias = "Purpose", alias = "description", default)]
    description: String,
    #[serde(rename = "Банк (БИК и наименование)", alias = "Bank", alias = "bank", default)]
    bank: String,
}

impl CsvStatement {
    /// Parse a CSV statement from any source implementing `Read`.
    ///
    /// # Arguments
    ///
    /// * `reader` - A mutable reference to a type implementing `Read`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use ypbank_system::csv_format::CsvStatement;
    ///
    /// let mut file = File::open("statement.csv")?;
    /// let statement = CsvStatement::from_read(&mut file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut csv_reader = Reader::from_reader(reader);
        let mut transactions = Vec::new();

        let mut account = String::new();
        let currency = String::from("RUB"); // Default currency

        for result in csv_reader.deserialize() {
            let record: CsvRecord = result?;

            // Skip empty rows
            if record.date.trim().is_empty() {
                continue;
            }

            // Try to parse date
            let date = Self::parse_date(&record.date)?;

            // Determine debit or credit
            let (amount, debit_credit, counterparty_account) = if !record.debit_amount.is_empty() {
                let amount = Self::parse_amount(&record.debit_amount)?;
                let counterparty = if !record.credit_account.is_empty() {
                    Some(Self::extract_account(&record.credit_account))
                } else {
                    None
                };

                // Update account from debit_account if needed
                if !record.debit_account.is_empty() && account.is_empty() {
                    account = Self::extract_account(&record.debit_account);
                }

                (amount, DebitCredit::Debit, counterparty)
            } else if !record.credit_amount.is_empty() {
                let amount = Self::parse_amount(&record.credit_amount)?;
                let counterparty = if !record.debit_account.is_empty() {
                    Some(Self::extract_account(&record.debit_account))
                } else {
                    None
                };

                // Update account from credit_account if needed
                if !record.credit_account.is_empty() && account.is_empty() {
                    account = Self::extract_account(&record.credit_account);
                }

                (amount, DebitCredit::Credit, counterparty)
            } else {
                continue; // Skip if no amount
            };

            // Extract counterparty name from description or account field
            let counterparty_name = Self::extract_counterparty_name(&record.description, &record.debit_account, &record.credit_account);

            transactions.push(Transaction {
                reference: record.reference.trim().to_string(),
                date,
                value_date: Some(date),
                amount,
                currency: currency.clone(),
                debit_credit,
                account: None,
                counterparty_account,
                counterparty_name,
                bank_identifier: if !record.bank.is_empty() {
                    Some(Self::extract_bic(&record.bank))
                } else {
                    None
                },
                description: record.description.trim().to_string(),
                additional_info: None,
            });
        }

        if account.is_empty() {
            account = "UNKNOWN".to_string();
        }

        let statement_id = format!("CSV-{}", chrono::Utc::now().timestamp());
        let mut statement = Statement::new(statement_id, account, currency);
        statement.transactions = transactions;

        Ok(CsvStatement { statement })
    }

    /// Write a CSV statement to any destination implementing `Write`.
    ///
    /// # Arguments
    ///
    /// * `writer` - A mutable reference to a type implementing `Write`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use ypbank_system::csv_format::CsvStatement;
    /// use ypbank_system::types::Statement;
    ///
    /// let statement = Statement::new("123".into(), "ACC001".into(), "USD".into());
    /// let csv = CsvStatement { statement };
    /// let mut file = File::create("output.csv")?;
    /// csv.write_to(&mut file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut csv_writer = Writer::from_writer(writer);

        for transaction in &self.statement.transactions {
            let (debit_account, credit_account, debit_amount, credit_amount) = match transaction.debit_credit {
                DebitCredit::Debit => (
                    self.statement.account.clone(),
                    transaction.counterparty_account.clone().unwrap_or_default(),
                    transaction.amount.to_string(),
                    String::new(),
                ),
                DebitCredit::Credit => (
                    transaction.counterparty_account.clone().unwrap_or_default(),
                    self.statement.account.clone(),
                    String::new(),
                    transaction.amount.to_string(),
                ),
            };

            let record = CsvRecord {
                date: transaction.date.format("%d.%m.%Y").to_string(),
                debit_account,
                credit_account,
                debit_amount,
                credit_amount,
                reference: transaction.reference.clone(),
                description: transaction.description.clone(),
                bank: transaction.bank_identifier.clone().unwrap_or_default(),
            };

            csv_writer.serialize(record)?;
        }

        csv_writer.flush()?;
        Ok(())
    }

    fn parse_date(date_str: &str) -> Result<NaiveDate> {
        // Try various date formats
        let formats = vec![
            "%d.%m.%Y",     // 20.02.2024
            "%Y-%m-%d",     // 2024-02-20
            "%d/%m/%Y",     // 20/02/2024
            "%m/%d/%Y",     // 02/20/2024
        ];

        for format in formats {
            if let Ok(date) = NaiveDate::parse_from_str(date_str.trim(), format) {
                return Ok(date);
            }
        }

        Err(Error::InvalidDate(date_str.to_string()))
    }

    fn parse_amount(amount_str: &str) -> Result<Decimal> {
        // Remove spaces and replace comma with dot
        let cleaned = amount_str
            .trim()
            .replace(' ', "")
            .replace(',', ".");

        Decimal::from_str(&cleaned)
            .map_err(|_| Error::InvalidAmount(amount_str.to_string()))
    }

    fn extract_account(account_field: &str) -> String {
        // Extract account number from a field that may contain multiple lines
        // e.g., "40702810440000030888\n7735602068\nООО РОМАШКА"
        account_field
            .lines()
            .next()
            .unwrap_or(account_field)
            .trim()
            .to_string()
    }

    fn extract_bic(bank_field: &str) -> String {
        // Extract BIC from bank field like "БИК 044525545 АО ЮниКредит Банк, г.Москва"
        // БИК is Cyrillic, "BIC " is ASCII
        if let Some(bic_start) = bank_field.find("БИК ") {
            // БИК is 3 UTF-8 chars + space = need to find the space and skip past it
            let after_bic = &bank_field[bic_start + "БИК ".len()..];
            after_bic
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        } else if let Some(bic_start) = bank_field.find("BIC ") {
            let after_bic = &bank_field[bic_start + "BIC ".len()..];
            after_bic
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        } else {
            bank_field.trim().to_string()
        }
    }

    fn extract_counterparty_name(description: &str, debit_account: &str, credit_account: &str) -> Option<String> {
        // Try to extract name from account fields
        let account_lines: Vec<&str> = debit_account.lines().chain(credit_account.lines()).collect();

        // Third line often contains the name
        if account_lines.len() >= 3 {
            let name = account_lines[2].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        // Otherwise, try to extract from description
        if !description.is_empty() {
            Some(description.lines().next().unwrap_or(description).trim().to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_parse_date() {
        let date = CsvStatement::parse_date("20.02.2024").unwrap();
        assert_eq!(date.year(), 2024);
        assert_eq!(date.month(), 2);
        assert_eq!(date.day(), 20);
    }

    #[test]
    fn test_parse_amount() {
        let amount = CsvStatement::parse_amount("1 540,00").unwrap();
        assert_eq!(amount.to_string(), "1540.00");
    }

    #[test]
    fn test_extract_bic() {
        let bic = CsvStatement::extract_bic("БИК 044525545 АО ЮниКредит Банк, г.Москва");
        assert_eq!(bic, "044525545");
    }
}
