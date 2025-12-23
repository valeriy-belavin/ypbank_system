//! YP Bank Compare - CLI tool for comparing bank statements from different formats.

use clap::Parser;
use std::fs::File;
use ypbank_system::{
    camt053_format::Camt053Statement,
    csv_format::CsvStatement,
    mt940_format::Mt940Statement,
    Format, Result, Statement,
};

#[derive(Parser)]
#[command(name = "ypbank_compare")]
#[command(about = "Compare bank statements from different formats", long_about = None)]
struct Cli {
    /// First file path
    #[arg(long = "file1")]
    file1: String,

    /// First file format (mt940, camt053, csv)
    #[arg(long = "format1")]
    format1: String,

    /// Second file path
    #[arg(long = "file2")]
    file2: String,

    /// Second file format (mt940, camt053, csv)
    #[arg(long = "format2")]
    format2: String,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Parse formats
    let format1 = cli.format1.parse::<Format>()?;
    let format2 = cli.format2.parse::<Format>()?;

    // Read and parse first file
    let mut file1 = File::open(&cli.file1)?;
    let statement1 = parse_statement(&mut file1, format1)?;

    // Read and parse second file
    let mut file2 = File::open(&cli.file2)?;
    let statement2 = parse_statement(&mut file2, format2)?;

    // Compare statements
    let result = compare_statements(&statement1, &statement2);

    println!("{}", result);

    Ok(())
}

fn parse_statement<R: std::io::Read>(reader: &mut R, format: Format) -> Result<Statement> {
    match format {
        Format::Mt940 => {
            let mt940 = Mt940Statement::from_read(reader)?;
            Ok(mt940.statement)
        }
        Format::Camt053 => {
            let camt053 = Camt053Statement::from_read(reader)?;
            Ok(camt053.statement)
        }
        Format::Csv => {
            let csv = CsvStatement::from_read(reader)?;
            Ok(csv.statement)
        }
    }
}

fn compare_statements(stmt1: &Statement, stmt2: &Statement) -> String {
    let mut differences = Vec::new();

    // Compare number of transactions
    if stmt1.transactions.len() != stmt2.transactions.len() {
        differences.push(format!(
            "Number of transactions differs: {} vs {}",
            stmt1.transactions.len(),
            stmt2.transactions.len()
        ));
    }

    // Compare transactions
    let min_len = std::cmp::min(stmt1.transactions.len(), stmt2.transactions.len());
    for i in 0..min_len {
        let tx1 = &stmt1.transactions[i];
        let tx2 = &stmt2.transactions[i];

        // Compare key fields
        if tx1.date != tx2.date {
            differences.push(format!(
                "Transaction {} date differs: {} vs {}",
                i + 1,
                tx1.date,
                tx2.date
            ));
        }

        if tx1.amount != tx2.amount {
            differences.push(format!(
                "Transaction {} amount differs: {} vs {}",
                i + 1,
                tx1.amount,
                tx2.amount
            ));
        }

        if tx1.debit_credit != tx2.debit_credit {
            differences.push(format!(
                "Transaction {} type differs: {:?} vs {:?}",
                i + 1,
                tx1.debit_credit,
                tx2.debit_credit
            ));
        }

        // Compare description (allowing for minor differences)
        let desc1 = normalize_string(&tx1.description);
        let desc2 = normalize_string(&tx2.description);
        if desc1 != desc2 && !desc1.is_empty() && !desc2.is_empty() {
            differences.push(format!(
                "Transaction {} description differs:\n  File 1: {}\n  File 2: {}",
                i + 1,
                tx1.description,
                tx2.description
            ));
        }
    }

    // Compare balances if present
    if let (Some(ref bal1), Some(ref bal2)) = (&stmt1.opening_balance, &stmt2.opening_balance) {
        if bal1.amount != bal2.amount {
            differences.push(format!(
                "Opening balance differs: {} vs {}",
                bal1.amount,
                bal2.amount
            ));
        }
    }

    if let (Some(ref bal1), Some(ref bal2)) = (&stmt1.closing_balance, &stmt2.closing_balance) {
        if bal1.amount != bal2.amount {
            differences.push(format!(
                "Closing balance differs: {} vs {}",
                bal1.amount,
                bal2.amount
            ));
        }
    }

    if differences.is_empty() {
        format!("The transaction records in '{}' and '{}' are identical.",
                "file1", "file2")
    } else {
        let mut result = String::from("Differences found:\n");
        for diff in differences {
            result.push_str("  - ");
            result.push_str(&diff);
            result.push('\n');
        }
        result
    }
}

fn normalize_string(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}
