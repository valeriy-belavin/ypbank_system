//! Error types for the yp-converter library.

use std::io;
use thiserror::Error;

/// Result type alias for library operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error types that can occur during parsing and serialization operations.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error occurred during read or write operations.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Error parsing CSV format.
    #[error("CSV parsing error: {0}")]
    CsvError(#[from] csv::Error),

    /// Error parsing XML format.
    #[error("XML parsing error: {0}")]
    XmlError(String),

    /// Error parsing MT940 format.
    #[error("MT940 parsing error at line {line}: {message}")]
    Mt940ParseError { line: usize, message: String },

    /// Invalid date format.
    #[error("Invalid date format: {0}")]
    InvalidDate(String),

    /// Invalid amount format.
    #[error("Invalid amount format: {0}")]
    InvalidAmount(String),

    /// Missing required field.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Invalid format specified.
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// General parsing error.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Conversion error between formats.
    #[error("Conversion error: {0}")]
    ConversionError(String),
}

impl From<quick_xml::Error> for Error {
    fn from(err: quick_xml::Error) -> Self {
        Error::XmlError(err.to_string())
    }
}

impl From<serde_xml_rs::Error> for Error {
    fn from(err: serde_xml_rs::Error) -> Self {
        Error::XmlError(err.to_string())
    }
}
