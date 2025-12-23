//! YP Bank Converter Library
//!
//! A library for parsing, serializing, and converting financial data formats.
//!
//! # Supported Formats
//!
//! - **MT940**: SWIFT-like bank statements
//! - **CAMT.053**: ISO 20022 XML format
//! - **CSV**: Comma-separated values format
//!
//! # Features
//!
//! - Parse financial statements from various formats
//! - Convert between MT940 and CAMT.053 formats
//! - Export to CSV and other formats
//! - Use standard `Read` and `Write` traits for flexibility
//!
//! # Examples
//!
//! ## Parsing an MT940 file
//!
//! ```no_run
//! use std::fs::File;
//! use ypbank_system::mt940_format::Mt940Statement;
//!
//! let mut file = File::open("statement.mt940")?;
//! let statement = Mt940Statement::from_read(&mut file)?;
//! println!("Statement ID: {}", statement.statement.statement_id);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Converting MT940 to CAMT.053
//!
//! ```no_run
//! use std::fs::File;
//! use ypbank_system::mt940_format::Mt940Statement;
//! use ypbank_system::camt053_format::Camt053Statement;
//!
//! let mut input = File::open("input.mt940")?;
//! let mt940 = Mt940Statement::from_read(&mut input)?;
//!
//! // Convert using From trait
//! let camt053: Camt053Statement = mt940.into();
//!
//! let mut output = File::create("output.xml")?;
//! camt053.write_to(&mut output)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod error;
pub mod types;
pub mod mt940_format;
pub mod camt053_format;
pub mod csv_format;
pub mod conversion;

use std::str::FromStr;

// Re-export commonly used types
pub use error::{Error, Result};
pub use types::{Transaction, Statement, Balance, DebitCredit, BalanceType};

/// Supported financial data formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// MT940 SWIFT format
    Mt940,
    /// CAMT.053 ISO 20022 XML format
    Camt053,
    /// CSV format
    Csv,
}

impl FromStr for Format {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "mt940" | "mt-940" | "swift" => Ok(Format::Mt940),
            "camt053" | "camt.053" | "camt" | "xml" => Ok(Format::Camt053),
            "csv" => Ok(Format::Csv),
            _ => Err(Error::InvalidFormat(s.to_string())),
        }
    }
}

impl Format {
    /// Parse format from string representation (deprecated - use FromStr instead).
    #[deprecated(since = "0.1.0", note = "Use FromStr::from_str or str::parse instead")]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self> {
        s.parse()
    }

    /// Get file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Format::Mt940 => "mt940",
            Format::Camt053 => "xml",
            Format::Csv => "csv",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_str() {
        assert_eq!("mt940".parse::<Format>().unwrap(), Format::Mt940);
        assert_eq!("MT940".parse::<Format>().unwrap(), Format::Mt940);
        assert_eq!("camt053".parse::<Format>().unwrap(), Format::Camt053);
        assert_eq!("csv".parse::<Format>().unwrap(), Format::Csv);
        assert!("unknown".parse::<Format>().is_err());
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(Format::Mt940.extension(), "mt940");
        assert_eq!(Format::Camt053.extension(), "xml");
        assert_eq!(Format::Csv.extension(), "csv");
    }
}
