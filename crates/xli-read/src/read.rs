use calamine::{Data, Reader, Xlsx, open_workbook};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::io::BufReader;
use std::path::Path;
use xli_core::{XliError, col_to_letter, parse_address, parse_range};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CellValueType {
    Number,
    String,
    Bool,
    Blank,
    Formula,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CellData {
    pub address: String,
    pub value: Value,
    pub formula: Option<String>,
    pub value_type: CellValueType,
    pub format: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RangeData {
    pub range: String,
    pub headers: Option<Vec<String>>,
    pub rows: Vec<Map<String, Value>>,
    pub total_rows: usize,
    pub truncated: bool,
    pub next_offset: Option<usize>,
}

pub fn read_cell(path: &Path, address: &str) -> Result<CellData, XliError> {
    let cell_ref = parse_address(address).map_err(XliError::from)?;
    let mut workbook = open_xlsx(path)?;
    let sheet_name = resolve_sheet_name(&workbook, cell_ref.sheet.as_deref())?;
    let range = workbook.worksheet_range(&sheet_name).map_err(calamine_error)?;
    let formulas = workbook.worksheet_formula(&sheet_name).map_err(calamine_error)?;
    let absolute = (cell_ref.row - 1, cell_ref.col_idx);
    let value = range.get_value(absolute).cloned().unwrap_or(Data::Empty);
    let formula = formulas
        .get_value(absolute)
        .filter(|formula| !formula.is_empty())
        .cloned();

    Ok(CellData {
        address: format!("{}!{}{}", sheet_name, cell_ref.col, cell_ref.row),
        value: data_to_json(&value),
        formula,
        value_type: value_type(&value, formulas.get_value(absolute)),
        format: None,
    })
}

pub fn read_range(
    path: &Path,
    range: &str,
    limit: Option<usize>,
    offset: Option<usize>,
    headers: bool,
) -> Result<RangeData, XliError> {
    let range_ref = parse_range(range).map_err(XliError::from)?;
    let mut workbook = open_xlsx(path)?;
    let sheet_name = resolve_sheet_name(&workbook, range_ref.sheet.as_deref())?;
    let worksheet = workbook.worksheet_range(&sheet_name).map_err(calamine_error)?;

    let start_row = range_ref.start.row - 1;
    let end_row = range_ref.end.row - 1;
    let start_col = range_ref.start.col_idx;
    let end_col = range_ref.end.col_idx;

    let mut matrix = Vec::new();
    for row in start_row..=end_row {
        let mut values = Vec::new();
        for col in start_col..=end_col {
            values.push(data_to_json(
                worksheet.get_value((row, col)).unwrap_or(&Data::Empty),
            ));
        }
        matrix.push(values);
    }

    let header_values = if headers && !matrix.is_empty() {
        Some(
            matrix[0]
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| format!("col_{}", index + 1))
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    let data_rows = if header_values.is_some() {
        matrix.into_iter().skip(1).collect::<Vec<_>>()
    } else {
        matrix
    };

    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(data_rows.len());
    let total_rows = data_rows.len();
    let rows = data_rows
        .iter()
        .skip(offset)
        .take(limit)
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(index, value)| {
                    let key = header_values
                        .as_ref()
                        .and_then(|headers| headers.get(index).cloned())
                        .unwrap_or_else(|| col_to_letter(start_col + index as u32));
                    (key, value.clone())
                })
                .collect::<Map<_, _>>()
        })
        .collect::<Vec<_>>();
    let truncated = offset + rows.len() < total_rows;

    Ok(RangeData {
        range: format!(
            "{}!{}{}:{}{}",
            sheet_name,
            range_ref.start.col,
            range_ref.start.row,
            range_ref.end.col,
            range_ref.end.row
        ),
        headers: header_values,
        rows,
        total_rows,
        truncated,
        next_offset: truncated.then_some(offset + limit),
    })
}

pub fn read_table(
    path: &Path,
    table_name: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<RangeData, XliError> {
    let mut workbook = open_xlsx(path)?;
    workbook.load_tables().map_err(calamine_error)?;
    let table = workbook.table_by_name(table_name).map_err(calamine_error)?;
    let total_rows = table.data().height();
    let headers = Some(table.columns().to_vec());
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(total_rows);
    let rows = table
        .data()
        .rows()
        .skip(offset)
        .take(limit)
        .map(|row: &[Data]| {
            row.iter()
                .enumerate()
                .map(|(index, value)| {
                    (
                        table
                            .columns()
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| format!("col_{}", index + 1)),
                        data_to_json(value),
                    )
                })
                .collect::<Map<_, _>>()
        })
        .collect::<Vec<_>>();
    let truncated = offset + rows.len() < total_rows;

    Ok(RangeData {
        range: table.name().to_string(),
        headers,
        rows,
        total_rows,
        truncated,
        next_offset: truncated.then_some(offset + limit),
    })
}

fn open_xlsx(path: &Path) -> Result<Xlsx<BufReader<std::fs::File>>, XliError> {
    if !path.exists() {
        return Err(XliError::FileNotFound {
            path: path.display().to_string(),
        });
    }
    open_workbook(path).map_err(calamine_error)
}

fn resolve_sheet_name(
    workbook: &Xlsx<BufReader<std::fs::File>>,
    explicit: Option<&str>,
) -> Result<String, XliError> {
    if let Some(sheet_name) = explicit {
        return Ok(sheet_name.to_string());
    }

    workbook
        .sheet_names()
        .into_iter()
        .next()
        .ok_or_else(|| XliError::SheetNotFound {
            sheet: "Sheet1".to_string(),
        })
}

fn data_to_json(data: &Data) -> Value {
    match data {
        Data::Empty => Value::Null,
        Data::String(value) => Value::String(value.clone()),
        Data::Float(value) => json!(value),
        Data::Int(value) => json!(value),
        Data::Bool(value) => json!(value),
        Data::Error(error) => Value::String(error.to_string()),
        Data::DateTime(value) => json!(value.as_f64()),
        Data::DateTimeIso(value) => Value::String(value.clone()),
        Data::DurationIso(value) => Value::String(value.clone()),
    }
}

fn value_type(data: &Data, formula: Option<&String>) -> CellValueType {
    if formula.is_some_and(|formula| !formula.is_empty()) {
        return CellValueType::Formula;
    }

    match data {
        Data::Empty => CellValueType::Blank,
        Data::String(_) => CellValueType::String,
        Data::Float(_) | Data::Int(_) | Data::DateTime(_) => CellValueType::Number,
        Data::Bool(_) => CellValueType::Bool,
        Data::Error(_) => CellValueType::Error,
        Data::DateTimeIso(_) | Data::DurationIso(_) => CellValueType::String,
    }
}

fn calamine_error<E: std::fmt::Display>(error: E) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{CellValueType, read_cell, read_range};
    use rust_xlsxwriter::Workbook;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn reads_single_cell() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("simple.xlsx");
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name("Summary").expect("name");
        sheet.write_number(0, 0, 42.0).expect("write");
        workbook.save(&path).expect("save");

        let cell = read_cell(&path, "Summary!A1").expect("read");
        assert_eq!(cell.value, json!(42.0));
        assert_eq!(cell.value_type, CellValueType::Number);
    }

    #[test]
    fn reads_range_with_headers_and_pagination() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("simple.xlsx");
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name("Summary").expect("name");
        sheet.write_string(0, 0, "name").expect("write");
        sheet.write_string(0, 1, "value").expect("write");
        sheet.write_string(1, 0, "foo").expect("write");
        sheet.write_number(1, 1, 1.0).expect("write");
        sheet.write_string(2, 0, "bar").expect("write");
        sheet.write_number(2, 1, 2.0).expect("write");
        workbook.save(&path).expect("save");

        let range = read_range(&path, "Summary!A1:B3", Some(1), Some(0), true).expect("read");
        assert_eq!(range.total_rows, 2);
        assert!(range.truncated);
        assert_eq!(range.rows.len(), 1);
        assert_eq!(range.rows[0]["name"], json!("foo"));
    }
}
