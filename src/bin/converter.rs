//! YP Bank Converter - CLI tool for converting between financial formats.

use clap::Parser;
use std::fs::File;
use std::io::{self, Read, Write};
use ypbank_system::{
    camt053_format::Camt053Statement,
    csv_format::CsvStatement,
    mt940_format::Mt940Statement,
    Format, Result, Statement,
};

#[derive(Parser)]
#[command(name = "ypbank_converter")]
#[command(about = "Convert between bank statement formats (MT940, CAMT.053, CSV)", long_about = None)]
struct Cli {
    /// Input file path (or stdin if not provided)
    #[arg(short, long)]
    input: Option<String>,

    /// Input format (mt940, camt053, csv)
    #[arg(long = "input-format")]
    input_format: String,

    /// Output format (mt940, camt053, csv)
    #[arg(long = "output-format")]
    output_format: String,

    /// Output file path (or stdout if not provided)
    #[arg(short, long)]
    output: Option<String>,
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
    let input_format = cli.input_format.parse::<Format>()?;
    let output_format = cli.output_format.parse::<Format>()?;

    // Process based on input file or stdin
    let statement = if let Some(ref input_path) = cli.input {
        let mut file = File::open(input_path)?;
        parse_input(&mut file, input_format)?
    } else {
        let mut stdin = io::stdin();
        parse_input(&mut stdin, input_format)?
    };

    // Output based on output file or stdout
    if let Some(ref output_path) = cli.output {
        let mut file = File::create(output_path)?;
        write_output(&mut file, &statement, output_format)?;
    } else {
        let mut stdout = io::stdout();
        write_output(&mut stdout, &statement, output_format)?;
    }

    Ok(())
}

fn parse_input<R: Read>(reader: &mut R, format: Format) -> Result<Statement> {
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

fn write_output<W: Write>(writer: &mut W, statement: &Statement, format: Format) -> Result<()> {
    match format {
        Format::Mt940 => {
            let mt940 = Mt940Statement {
                statement: statement.clone(),
            };
            mt940.write_to(writer)?;
        }
        Format::Camt053 => {
            let camt053 = Camt053Statement {
                statement: statement.clone(),
            };
            camt053.write_to(writer)?;
        }
        Format::Csv => {
            let csv = CsvStatement {
                statement: statement.clone(),
            };
            csv.write_to(writer)?;
        }
    }
    Ok(())
}
