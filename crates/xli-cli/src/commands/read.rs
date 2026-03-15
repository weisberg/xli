use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use xli_read::{CellData, RangeData};

use crate::output;

#[derive(Debug, Args)]
pub struct ReadArgs {
    pub file: PathBuf,
    pub address: Option<String>,
    #[arg(long)]
    pub sheet: Option<String>,
    #[arg(long)]
    pub range: Option<String>,
    #[arg(long)]
    pub table: Option<String>,
    #[arg(long, default_value = "1000")]
    pub limit: usize,
    #[arg(long, default_value = "0")]
    pub offset: usize,
    #[arg(long)]
    pub formulas: bool,
    #[arg(long)]
    pub headers: bool,
}

pub fn run(args: ReadArgs, human: bool) -> Result<bool> {
    let input = serde_json::json!({
        "file": args.file,
        "address": args.address,
        "sheet": args.sheet,
        "range": args.range,
        "table": args.table,
        "limit": args.limit,
        "offset": args.offset,
        "headers": args.headers,
        "formulas": args.formulas,
    });

    if let Some(table_name) = args.table.as_deref() {
        return match xli_read::read_table(&args.file, table_name, Some(args.limit), Some(args.offset))
        {
            Ok(output_data) => output::emit(
                &output::ok_envelope(
                    "read",
                    input,
                    output_data,
                    Vec::new(),
                    false,
                    xli_core::CommitMode::None,
                    None,
                    None,
                    xli_core::CommitStats::default(),
                ),
                human,
            ),
            Err(error) => output::emit(&output::error_envelope::<RangeData>("read", Some(input), error), human),
        };
    }

    if let Some(range) = args.range.as_deref().or(args.address.as_deref()) {
        let reference = qualify_reference(range, args.sheet.as_deref());
        if reference.contains(':') {
            return match xli_read::read_range(
                &args.file,
                &reference,
                Some(args.limit),
                Some(args.offset),
                args.headers,
            ) {
                Ok(output_data) => output::emit(
                    &output::ok_envelope(
                        "read",
                        input,
                        output_data,
                        Vec::new(),
                        false,
                        xli_core::CommitMode::None,
                        None,
                        None,
                        xli_core::CommitStats::default(),
                    ),
                    human,
                ),
                Err(error) => output::emit(&output::error_envelope::<RangeData>("read", Some(input), error), human),
            };
        }

        return match xli_read::read_cell(&args.file, &reference) {
            Ok(output_data) => {
                let output_data = if args.formulas {
                    formula_view(output_data)
                } else {
                    output_data
                };
                output::emit(
                    &output::ok_envelope(
                        "read",
                        input,
                        output_data,
                        Vec::new(),
                        false,
                        xli_core::CommitMode::None,
                        None,
                        None,
                        xli_core::CommitStats::default(),
                    ),
                    human,
                )
            }
            Err(error) => output::emit(&output::error_envelope::<CellData>("read", Some(input), error), human),
        };
    }

    output::emit(
        &output::error_envelope::<serde_json::Value>(
            "read",
            Some(input),
            xli_core::XliError::CliParseError {
                message: "read requires an address, --range, or --table".to_string(),
            },
        ),
        human,
    )
}

fn qualify_reference(reference: &str, sheet: Option<&str>) -> String {
    if reference.contains('!') || sheet.is_none() {
        reference.to_string()
    } else {
        format!("{}!{}", sheet.expect("sheet checked"), reference)
    }
}

fn formula_view(mut cell: CellData) -> CellData {
    if let Some(formula) = cell.formula.clone() {
        cell.value = serde_json::Value::String(formula);
        cell.value_type = xli_read::CellValueType::Formula;
    }
    cell
}
