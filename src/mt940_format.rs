//! MT940 SWIFT format parser and serializer.
//!
//! MT940 is a SWIFT format for electronic account statements.
//! This module provides parsing and writing capabilities for MT940 format.

use crate::error::{Error, Result};
use crate::types::{Balance, BalanceType, DebitCredit, Statement, Transaction};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use std::io::{BufRead, Write};
use std::str::FromStr;

/// Represents an MT940 statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Mt940Statement {
    /// The underlying statement data.
    pub statement: Statement,
}

impl Mt940Statement {
    /// Parse an MT940 statement from any source implementing `Read`.
    ///
    /// # Arguments
    ///
    /// * `reader` - A mutable reference to a type implementing `Read`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use ypbank_system::mt940_format::Mt940Statement;
    ///
    /// let mut file = File::open("statement.mt940")?;
    /// let statement = Mt940Statement::from_read(&mut file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_read<R: std::io::Read>(reader: &mut R) -> Result<Self> {
        let buf_reader = std::io::BufReader::new(reader);
        Self::parse_mt940(buf_reader)
    }

    /// Write an MT940 statement to any destination implementing `Write`.
    ///
    /// # Arguments
    ///
    /// * `writer` - A mutable reference to a type implementing `Write`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use ypbank_system::mt940_format::Mt940Statement;
    /// use ypbank_system::types::Statement;
    ///
    /// let statement = Statement::new("123".into(), "ACC001".into(), "USD".into());
    /// let mt940 = Mt940Statement { statement };
    /// let mut file = File::create("output.mt940")?;
    /// mt940.write_to(&mut file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.serialize_mt940(writer)
    }

    fn parse_mt940<R: BufRead>(reader: R) -> Result<Self> {
        let mut lines: Vec<String> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            lines.push(line);
        }

        let mut statement_id = String::new();
        let mut account = String::new();
        let mut sequence_number = None;
        let mut currency = String::new();
        let mut opening_balance = None;
        let mut closing_balance = None;
        let mut transactions = Vec::new();

        let mut current_line = 0;
        let mut current_transaction: Option<Transaction> = None;
        let mut transaction_description = String::new();

        while current_line < lines.len() {
            let line = &lines[current_line];

            if line.starts_with(":20:") {
                // Transaction Reference Number
                statement_id = line.get(4..).unwrap_or("").trim().to_string();
            } else if line.starts_with(":25:") {
                // Account Identification
                account = line.get(4..).unwrap_or("").trim().to_string();
            } else if line.starts_with(":28C:") {
                // Statement Number/Sequence Number
                sequence_number = Some(line.get(5..).unwrap_or("").trim().to_string());
            } else if line.starts_with(":60") {
                // Opening Balance
                opening_balance = Some(Self::parse_balance(line, BalanceType::Opening)?);
                if currency.is_empty() {
                    if let Some(ref bal) = opening_balance {
                        currency = bal.currency.clone();
                    }
                }
            } else if line.starts_with(":61:") {
                // Save previous transaction if exists
                if let Some(mut trans) = current_transaction.take() {
                    trans.description = transaction_description.trim().to_string();
                    transactions.push(trans);
                    transaction_description.clear();
                }

                // Statement Line (Transaction)
                current_transaction = Some(Self::parse_transaction_line(line, &currency)?);
            } else if line.starts_with(":86:") {
                // Information to Account Owner
                transaction_description = line.get(4..).unwrap_or("").trim().to_string();

                // Check for continuation lines
                let mut next_line = current_line + 1;
                while next_line < lines.len() {
                    let next = &lines[next_line];
                    if next.starts_with(':') {
                        break;
                    }
                    transaction_description.push(' ');
                    transaction_description.push_str(next.trim());
                    current_line = next_line;
                    next_line += 1;
                }
            } else if line.starts_with(":62") {
                // Closing Balance
                closing_balance = Some(Self::parse_balance(line, BalanceType::Closing)?);
            }

            current_line += 1;
        }

        // Don't forget the last transaction
        if let Some(mut trans) = current_transaction.take() {
            trans.description = transaction_description.trim().to_string();
            transactions.push(trans);
        }

        if statement_id.is_empty() {
            return Err(Error::MissingField("statement reference :20:".to_string()));
        }
        if account.is_empty() {
            return Err(Error::MissingField("account identification :25:".to_string()));
        }

        let mut statement = Statement::new(statement_id, account, currency);
        statement.sequence_number = sequence_number;
        statement.opening_balance = opening_balance;
        statement.closing_balance = closing_balance;
        statement.transactions = transactions;

        Ok(Mt940Statement { statement })
    }

    fn parse_balance(line: &str, balance_type: BalanceType) -> Result<Balance> {
        // Format: :60F:C250218USD2732398848,02
        // Position 1: D/C indicator
        // Position 2-7: Date (YYMMDD)
        // Position 8-10: Currency
        // Position 11+: Amount

        let content = if line.starts_with(":60") {
            line.get(5..).ok_or_else(|| Error::ParseError(format!("Invalid balance line: {}", line)))?
        } else if line.starts_with(":62") {
            line.get(5..).ok_or_else(|| Error::ParseError(format!("Invalid balance line: {}", line)))?
        } else {
            return Err(Error::ParseError(format!("Invalid balance line: {}", line)));
        };

        if content.len() < 11 {
            return Err(Error::ParseError(format!("Balance line too short: {}", line)));
        }

        let dc = content.get(0..1).unwrap_or("")
            .parse::<DebitCredit>()
            .map_err(|_| Error::ParseError(format!("Invalid D/C indicator in: {}", line)))?;

        let date_str = content.get(1..7)
            .ok_or_else(|| Error::ParseError(format!("Invalid date in balance line: {}", line)))?;
        let date = parse_mt940_date(date_str)?;

        let currency = content.get(7..10)
            .ok_or_else(|| Error::ParseError(format!("Invalid currency in balance line: {}", line)))?
            .to_string();

        let amount_str = content.get(10..)
            .ok_or_else(|| Error::ParseError(format!("Missing amount in balance line: {}", line)))?
            .replace(',', ".");
        let amount = Decimal::from_str(&amount_str)
            .map_err(|_| Error::InvalidAmount(amount_str.to_string()))?;

        Ok(Balance {
            balance_type,
            amount,
            currency,
            debit_credit: dc,
            date,
        })
    }

    fn parse_transaction_line(line: &str, default_currency: &str) -> Result<Transaction> {
        // Format: :61:2502180218D12,01NTRFGSLNVSHSUTKWDR//GI2504900007841
        // Position 1-6: Value date (YYMMDD)
        // Position 7-10: Entry date (MMDD) - optional
        // Position 11: D/C indicator
        // Position 12+: Amount
        // Then transaction type code
        // Then reference

        let content = line.get(4..)
            .ok_or_else(|| Error::ParseError(format!("Transaction line too short: {}", line)))?;

        if content.len() < 6 {
            return Err(Error::ParseError(format!("Transaction line too short: {}", line)));
        }

        let value_date_str = content.get(0..6)
            .ok_or_else(|| Error::ParseError(format!("Invalid value date in: {}", line)))?;
        let value_date = parse_mt940_date(value_date_str)?;

        // Try to parse entry date (may not always be present)
        let mut pos = 6;
        let date = if content.len() > pos + 4 && content.chars().nth(pos + 2).unwrap_or('X').is_ascii_digit() {
            let entry_date_str = content.get(pos..pos + 4)
                .ok_or_else(|| Error::ParseError(format!("Invalid entry date in: {}", line)))?;
            pos += 4;
            parse_mt940_entry_date(entry_date_str, value_date.year())?
        } else {
            value_date
        };

        // D/C indicator
        let dc_char = content.chars().nth(pos).ok_or_else(|| {
            Error::Mt940ParseError {
                line: 0,
                message: "Missing D/C indicator".to_string(),
            }
        })?;
        let debit_credit = dc_char.to_string()
            .parse::<DebitCredit>()
            .map_err(|_| Error::ParseError(format!("Invalid D/C: {}", dc_char)))?;
        pos += 1;

        // Parse amount
        let rest_of_line = content.get(pos..)
            .ok_or_else(|| Error::ParseError(format!("Missing amount in: {}", line)))?;
        let amount_end = rest_of_line
            .find(|c: char| c.is_alphabetic())
            .unwrap_or(rest_of_line.len());

        let amount_str = rest_of_line.get(0..amount_end)
            .ok_or_else(|| Error::ParseError(format!("Invalid amount in: {}", line)))?
            .replace(',', ".");
        let amount = Decimal::from_str(&amount_str)
            .map_err(|_| Error::InvalidAmount(amount_str.to_string()))?;

        // Extract reference from the rest
        let rest = rest_of_line.get(amount_end..)
            .ok_or_else(|| Error::ParseError(format!("Invalid format in: {}", line)))?;
        let reference = rest
            .split("//")
            .last()
            .unwrap_or(rest)
            .trim()
            .to_string();

        Ok(Transaction {
            reference: if reference.is_empty() {
                format!("{}-{}", date, amount)
            } else {
                reference
            },
            date,
            value_date: Some(value_date),
            amount,
            currency: default_currency.to_string(),
            debit_credit,
            account: None,
            counterparty_account: None,
            counterparty_name: None,
            bank_identifier: None,
            description: String::new(),
            additional_info: None,
        })
    }

    fn serialize_mt940<W: Write>(&self, writer: &mut W) -> Result<()> {
        let stmt = &self.statement;

        // Header (simplified)
        writeln!(writer, "{{1:F01BANKXXXXAXXX0000000000}}{{2:I940BANKXXXXAXXXXN}}{{4:")?;

        // :20: Transaction Reference Number
        writeln!(writer, ":20:{}", stmt.statement_id)?;

        // :25: Account Identification
        writeln!(writer, ":25:{}", stmt.account)?;

        // :28C: Statement Number
        if let Some(ref seq) = stmt.sequence_number {
            writeln!(writer, ":28C:{}", seq)?;
        }

        // :60: Opening Balance
        if let Some(ref balance) = stmt.opening_balance {
            write!(writer, ":60{}:", if balance.balance_type == BalanceType::Opening { "F" } else { "M" })?;
            write!(writer, "{}", balance.debit_credit.to_string())?;
            write!(writer, "{}", format_mt940_date(&balance.date))?;
            write!(writer, "{}", balance.currency)?;
            writeln!(writer, "{}", balance.amount.to_string().replace('.', ","))?;
        }

        // :61: Statement Lines (Transactions)
        for transaction in &stmt.transactions {
            write!(writer, ":61:")?;
            if let Some(value_date) = transaction.value_date {
                write!(writer, "{}", format_mt940_date(&value_date))?;
            } else {
                write!(writer, "{}", format_mt940_date(&transaction.date))?;
            }
            // Entry date (same as value date for simplicity)
            write!(writer, "{:02}{:02}", transaction.date.month(), transaction.date.day())?;
            write!(writer, "{}", transaction.debit_credit.to_string())?;
            write!(writer, "{}", transaction.amount.to_string().replace('.', ","))?;
            writeln!(writer, "NTRF//{}", transaction.reference)?;

            // :86: Information to Account Owner
            if !transaction.description.is_empty() {
                writeln!(writer, ":86:{}", transaction.description)?;
            }
        }

        // :62: Closing Balance
        if let Some(ref balance) = stmt.closing_balance {
            write!(writer, ":62{}:", if balance.balance_type == BalanceType::Closing { "F" } else { "M" })?;
            write!(writer, "{}", balance.debit_credit.to_string())?;
            write!(writer, "{}", format_mt940_date(&balance.date))?;
            write!(writer, "{}", balance.currency)?;
            writeln!(writer, "{}", balance.amount.to_string().replace('.', ","))?;
        }

        writeln!(writer, "-}}")?;

        Ok(())
    }
}

/// Parse MT940 date format (YYMMDD) to NaiveDate.
fn parse_mt940_date(date_str: &str) -> Result<NaiveDate> {
    if date_str.len() != 6 {
        return Err(Error::InvalidDate(format!("Invalid MT940 date length: {}", date_str)));
    }

    let year = date_str.get(0..2)
        .ok_or_else(|| Error::InvalidDate(date_str.to_string()))?
        .parse::<i32>()
        .map_err(|_| Error::InvalidDate(date_str.to_string()))?;
    let month = date_str.get(2..4)
        .ok_or_else(|| Error::InvalidDate(date_str.to_string()))?
        .parse::<u32>()
        .map_err(|_| Error::InvalidDate(date_str.to_string()))?;
    let day = date_str.get(4..6)
        .ok_or_else(|| Error::InvalidDate(date_str.to_string()))?
        .parse::<u32>()
        .map_err(|_| Error::InvalidDate(date_str.to_string()))?;

    // Assume 2000+ for years < 50, otherwise 1900+
    let full_year = if year < 50 { 2000 + year } else { 1900 + year };

    NaiveDate::from_ymd_opt(full_year, month, day)
        .ok_or_else(|| Error::InvalidDate(format!("{}-{}-{}", full_year, month, day)))
}

/// Parse MT940 entry date (MMDD) using year from value date.
fn parse_mt940_entry_date(date_str: &str, year: i32) -> Result<NaiveDate> {
    if date_str.len() != 4 {
        return Err(Error::InvalidDate(format!("Invalid entry date length: {}", date_str)));
    }

    let month = date_str.get(0..2)
        .ok_or_else(|| Error::InvalidDate(date_str.to_string()))?
        .parse::<u32>()
        .map_err(|_| Error::InvalidDate(date_str.to_string()))?;
    let day = date_str.get(2..4)
        .ok_or_else(|| Error::InvalidDate(date_str.to_string()))?
        .parse::<u32>()
        .map_err(|_| Error::InvalidDate(date_str.to_string()))?;

    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| Error::InvalidDate(format!("{}-{}-{}", year, month, day)))
}

/// Format NaiveDate to MT940 format (YYMMDD).
fn format_mt940_date(date: &NaiveDate) -> String {
    format!("{:02}{:02}{:02}", date.year() % 100, date.month(), date.day())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mt940_date() {
        let date = parse_mt940_date("250218").unwrap();
        assert_eq!(date.year(), 2025);
        assert_eq!(date.month(), 2);
        assert_eq!(date.day(), 18);
    }

    #[test]
    fn test_debit_credit() {
        assert_eq!("D".parse::<DebitCredit>().ok(), Some(DebitCredit::Debit));
        assert_eq!("C".parse::<DebitCredit>().ok(), Some(DebitCredit::Credit));
        assert!("X".parse::<DebitCredit>().is_err());
    }
}
