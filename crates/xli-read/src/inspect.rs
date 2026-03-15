use calamine::{Reader, SheetType, Xlsx, open_workbook};
use serde::Serialize;
use std::collections::HashMap;
use std::io::BufReader;
use std::path::Path;
use xli_core::{XliError, col_to_letter, parse_address, parse_range};
use xli_fs::fingerprint;

/// High-level workbook metadata returned by `xli inspect`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkbookInfo {
    pub file: String,
    pub size_bytes: u64,
    pub fingerprint: String,
    pub sheets: Vec<SheetInfo>,
    pub defined_names: HashMap<String, String>,
    pub has_macros: bool,
}

/// High-level per-sheet metadata returned by `xli inspect`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SheetInfo {
    pub name: String,
    pub index: u32,
    pub dimensions: Option<String>,
    pub rows: u32,
    pub cols: u32,
    pub formula_count: u32,
    pub tables: Vec<String>,
    pub named_ranges: Vec<String>,
    pub merged_regions: Vec<String>,
    pub has_charts: bool,
}

/// Inspect an OOXML workbook and summarize its structure.
pub fn inspect(path: &Path) -> Result<WorkbookInfo, XliError> {
    if !path.exists() {
        return Err(XliError::FileNotFound {
            path: path.display().to_string(),
        });
    }

    let size_bytes = path.metadata().map_err(io_error)?.len();
    let workbook_fingerprint = fingerprint(path)?;
    let mut workbook: Xlsx<BufReader<std::fs::File>> =
        open_workbook(path).map_err(calamine_error)?;

    let has_macros = workbook.vba_project().is_some();
    let tables_loaded = workbook.load_tables().is_ok();
    let merged_regions_loaded = workbook.load_merged_regions().is_ok();

    let defined_names = workbook
        .defined_names()
        .iter()
        .map(|(name, formula)| (name.clone(), formula.clone()))
        .collect::<HashMap<_, _>>();
    let metadata = workbook.sheets_metadata().to_vec();
    let sheet_names = workbook.sheet_names();

    let mut sheets = Vec::with_capacity(sheet_names.len());
    for (index, name) in sheet_names.iter().enumerate() {
        let range = workbook.worksheet_range(name).map_err(calamine_error)?;
        let formulas = workbook.worksheet_formula(name).map_err(calamine_error)?;
        let dimensions = range
            .start()
            .zip(range.end())
            .map(|(start, end)| format_dimension(start, end));
        let (rows, cols) = range.get_size();
        let formula_count = formulas
            .rows()
            .flat_map(|row| row.iter())
            .filter(|formula| !formula.is_empty())
            .count() as u32;

        let tables = if tables_loaded {
            workbook
                .table_names_in_sheet(name)
                .into_iter()
                .map(|table| table.to_string())
                .collect()
        } else {
            Vec::new()
        };
        let merged_regions = if merged_regions_loaded {
            workbook
                .merged_regions_by_sheet(name)
                .into_iter()
                .map(|(_, _, dims)| format_dimension(dims.start, dims.end))
                .collect()
        } else {
            Vec::new()
        };
        let named_ranges = defined_names
            .iter()
            .filter(|(_, formula)| formula_targets_sheet(formula, name))
            .map(|(defined_name, _)| defined_name.clone())
            .collect();
        let has_charts = metadata
            .get(index)
            .map(|sheet| sheet.typ == SheetType::ChartSheet)
            .unwrap_or(false);

        sheets.push(SheetInfo {
            name: name.clone(),
            index: index as u32,
            dimensions,
            rows: rows as u32,
            cols: cols as u32,
            formula_count,
            tables,
            named_ranges,
            merged_regions,
            has_charts,
        });
    }

    Ok(WorkbookInfo {
        file: path.display().to_string(),
        size_bytes,
        fingerprint: workbook_fingerprint,
        sheets,
        defined_names,
        has_macros,
    })
}

fn format_dimension(start: (u32, u32), end: (u32, u32)) -> String {
    let start_col = col_to_letter(start.1);
    let end_col = col_to_letter(end.1);
    format!("{start_col}{}:{end_col}{}", start.0 + 1, end.0 + 1)
}

fn formula_targets_sheet(formula: &str, sheet_name: &str) -> bool {
    if let Ok(range) = parse_range(formula) {
        return range.sheet.as_deref() == Some(sheet_name);
    }

    if let Ok(cell) = parse_address(formula) {
        return cell.sheet.as_deref() == Some(sheet_name);
    }

    false
}

fn io_error(error: std::io::Error) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

fn calamine_error<E: std::fmt::Display>(error: E) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::inspect;
    use rust_xlsxwriter::Workbook;
    use tempfile::tempdir;
    use xli_core::XliError;

    #[test]
    fn inspects_basic_workbook_metadata() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("simple.xlsx");
        let mut workbook = Workbook::new();

        let summary = workbook.add_worksheet();
        summary.set_name("Summary").expect("name");
        summary.write_string(0, 0, "Metric").expect("write");
        summary.write_number(0, 1, 42.0).expect("write");
        summary.write_formula(1, 1, "=SUM(B1:B1)").expect("write");

        let raw = workbook.add_worksheet();
        raw.set_name("Raw Data").expect("name");
        raw.write_string(0, 0, "Value").expect("write");

        workbook.save(&path).expect("save");

        let info = inspect(&path).expect("inspect");
        assert_eq!(info.sheets.len(), 2);
        assert_eq!(info.sheets[0].name, "Summary");
        assert_eq!(info.sheets[0].formula_count, 1);
        assert_eq!(info.sheets[1].name, "Raw Data");
        assert!(info.fingerprint.starts_with("sha256:"));
    }

    #[test]
    fn missing_workbook_returns_file_not_found() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("missing.xlsx");
        let error = inspect(&path).expect_err("missing");
        assert!(matches!(error, XliError::FileNotFound { .. }));
    }
}
