#![forbid(unsafe_code)]

mod commands;
mod output;

use clap::{Parser, Subcommand};
use commands::inspect::InspectArgs;
use serde::Serialize;
use xli_core::{CommitMode, CommitStats, ResponseEnvelope, Status, XliError};

#[derive(Debug, Parser)]
#[command(name = "xli", version, about = "Excel CLI for structured workbook operations")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, global = true)]
    human: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Inspect(InspectArgs),
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if !std::env::args().any(|arg| arg == "--human") {
                let envelope: ResponseEnvelope<serde_json::Value> =
                    make_error_envelope(XliError::CliParseError {
                    message: error.to_string(),
                });
                let _ = output::emit(&envelope, false);
                std::process::exit(2);
            }
            error.exit();
        }
    };

    let exit_code = match run(cli) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Inspect(args) => commands::inspect::run(args, cli.human)?,
    }
    Ok(())
}

fn make_error_envelope<T>(error: XliError) -> ResponseEnvelope<T>
where
    T: Serialize,
{
    ResponseEnvelope {
        status: Status::Error,
        command: "cli".to_string(),
        input: None,
        output: None,
        commit_mode: CommitMode::None,
        fingerprint_before: None,
        fingerprint_after: None,
        needs_recalc: false,
        stats: CommitStats::default(),
        warnings: Vec::new(),
        errors: vec![error],
        suggested_repairs: Vec::new(),
    }
}
