use rust_xlsxwriter::Workbook;
use std::path::Path;
use xli_core::XliError;

pub fn create_blank(path: &Path, sheets: &[String]) -> Result<(), XliError> {
    let mut workbook = Workbook::new();

    if sheets.is_empty() {
        workbook.add_worksheet();
    } else {
        for sheet_name in sheets {
            let worksheet = workbook.add_worksheet();
            worksheet
                .set_name(sheet_name)
                .map_err(|error| XliError::WriteConflict {
                    target: sheet_name.clone(),
                    details: Some(error.to_string()),
                })?;
        }
    }

    workbook.save(path).map_err(|error| XliError::OoxmlCorrupt {
        details: error.to_string(),
    })
}

pub fn create_from_csv(csv_path: &Path, out_path: &Path, sheet_name: &str) -> Result<(), XliError> {
    // Use the csv crate for RFC 4180-compliant parsing. The previous
    // line.split(',') approach silently broke quoted fields containing commas
    // (e.g. "Smith, John" would be split into two cells). (Issue #24)
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(csv_path)
        .map_err(|error| match error.kind() {
            csv::ErrorKind::Io(io_err) if io_err.kind() == std::io::ErrorKind::NotFound => {
                XliError::FileNotFound {
                    path: csv_path.display().to_string(),
                }
            }
            _ => XliError::OoxmlCorrupt {
                details: error.to_string(),
            },
        })?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name(sheet_name)
        .map_err(|error| XliError::WriteConflict {
            target: sheet_name.to_string(),
            details: Some(error.to_string()),
        })?;

    for (row_idx, record) in reader.records().enumerate() {
        let record = record.map_err(|error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        })?;
        for (col_idx, field) in record.iter().enumerate() {
            worksheet
                .write_string(row_idx as u32, col_idx as u16, field)
                .map_err(|error| XliError::OoxmlCorrupt {
                    details: error.to_string(),
                })?;
        }
    }

    workbook
        .save(out_path)
        .map_err(|error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        })
}

pub fn create_from_markdown(
    md_path: &Path,
    out_path: &Path,
    sheet_name: &str,
) -> Result<(), XliError> {
    let content = std::fs::read_to_string(md_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            XliError::FileNotFound {
                path: md_path.display().to_string(),
            }
        } else {
            XliError::OoxmlCorrupt {
                details: error.to_string(),
            }
        }
    })?;

    let rows = parse_markdown_table(&content)?;
    if rows.is_empty() {
        return create_blank(out_path, &[sheet_name.to_string()]);
    }

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name(sheet_name)
        .map_err(|error| XliError::WriteConflict {
            target: sheet_name.to_string(),
            details: Some(error.to_string()),
        })?;

    for (row_idx, row) in rows.iter().enumerate() {
        for (col_idx, cell) in row.iter().enumerate() {
            // Try to write as number first, fall back to string
            if let Ok(num) = cell.parse::<f64>() {
                worksheet
                    .write_number(row_idx as u32, col_idx as u16, num)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            } else if cell.eq_ignore_ascii_case("true") {
                worksheet
                    .write_boolean(row_idx as u32, col_idx as u16, true)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            } else if cell.eq_ignore_ascii_case("false") {
                worksheet
                    .write_boolean(row_idx as u32, col_idx as u16, false)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            } else if !cell.is_empty() {
                worksheet
                    .write_string(row_idx as u32, col_idx as u16, cell)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            }
        }
    }

    workbook
        .save(out_path)
        .map_err(|error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        })
}

/// Parse a markdown pipe table into a Vec of rows (each row is a Vec of cell strings).
/// Skips the separator row (contains only dashes/colons/pipes).
fn parse_markdown_table(content: &str) -> Result<Vec<Vec<String>>, XliError> {
    let mut rows = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains('|') {
            // Skip non-table lines. Stop if we already have rows and hit a blank.
            if !rows.is_empty() {
                break;
            }
            continue;
        }

        // Skip separator rows like |---|---|---|
        if is_separator_row(trimmed) {
            continue;
        }

        let cells: Vec<String> = trimmed
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect::<Vec<_>>();

        // Trim leading/trailing empty strings from leading/trailing pipes
        let cells: Vec<String> = cells
            .into_iter()
            .skip_while(|c| c.is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .skip_while(|c| c.is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    Ok(rows)
}

fn is_separator_row(line: &str) -> bool {
    line.chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
}

#[cfg(test)]
mod tests {
    use super::{create_blank, create_from_csv, create_from_markdown};
    use calamine::{open_workbook, Reader, Xlsx};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn creates_blank_workbook_with_named_sheets() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("blank.xlsx");
        create_blank(&path, &["Summary".to_string(), "Data".to_string()]).expect("create");

        let workbook: Xlsx<_> = open_workbook(&path).expect("open");
        assert_eq!(workbook.sheet_names(), vec!["Summary", "Data"]);
    }

    #[test]
    fn creates_workbook_from_csv() {
        let dir = tempdir().expect("tempdir");
        let csv = dir.path().join("data.csv");
        let out = dir.path().join("data.xlsx");
        fs::write(&csv, "name,value\nfoo,1\nbar,2\n").expect("write");

        create_from_csv(&csv, &out, "Import").expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Import").expect("range");
        assert_eq!(
            range
                .get_value((0, 0))
                .map(|cell: &calamine::Data| cell.to_string()),
            Some("name".to_string())
        );
        assert_eq!(
            range
                .get_value((1, 0))
                .map(|cell: &calamine::Data| cell.to_string()),
            Some("foo".to_string())
        );
    }

    #[test]
    fn csv_quoted_fields_with_commas_are_single_cells() {
        // Regression test for Issue #24: split(',') would break "Smith, John"
        // into two cells. The csv crate handles RFC 4180 quoting correctly.
        let dir = tempdir().expect("tempdir");
        let csv = dir.path().join("quoted.csv");
        let out = dir.path().join("quoted.xlsx");
        fs::write(&csv, "name,city\n\"Smith, John\",\"New York\"\n").expect("write");

        create_from_csv(&csv, &out, "Data").expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Data").expect("range");
        assert_eq!(
            range
                .get_value((1, 0))
                .map(|cell: &calamine::Data| cell.to_string()),
            Some("Smith, John".to_string()),
            "quoted field with comma should be a single cell"
        );
        assert_eq!(
            range
                .get_value((1, 1))
                .map(|cell: &calamine::Data| cell.to_string()),
            Some("New York".to_string())
        );
    }

    #[test]
    fn creates_workbook_from_markdown_table() {
        let dir = tempdir().expect("tempdir");
        let md = dir.path().join("table.md");
        let out = dir.path().join("table.xlsx");
        fs::write(
            &md,
            "| Name  | Score |\n| ----- | ----- |\n| Alice | 95    |\n| Bob   | 87    |\n",
        )
        .expect("write");

        create_from_markdown(&md, &out, "Data").expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Data").expect("range");
        // Row 0: headers
        assert_eq!(
            range.get_value((0, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("Name".to_string())
        );
        assert_eq!(
            range.get_value((0, 1)).map(|c: &calamine::Data| c.to_string()),
            Some("Score".to_string())
        );
        // Row 1: Alice, 95 (as number)
        assert_eq!(
            range.get_value((1, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("Alice".to_string())
        );
        assert_eq!(
            range.get_value((1, 1)).map(|c: &calamine::Data| c.to_string()),
            Some("95".to_string())
        );
    }

    #[test]
    fn markdown_missing_file_returns_error() {
        let dir = tempdir().expect("tempdir");
        let md = dir.path().join("nope.md");
        let out = dir.path().join("out.xlsx");
        let err = create_from_markdown(&md, &out, "Sheet1").expect_err("missing");
        assert!(matches!(err, xli_core::XliError::FileNotFound { .. }));
    }

    #[test]
    fn markdown_with_surrounding_text() {
        let dir = tempdir().expect("tempdir");
        let md = dir.path().join("doc.md");
        let out = dir.path().join("doc.xlsx");
        fs::write(
            &md,
            "# Report\n\nSome intro text.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nMore text.\n",
        )
        .expect("write");

        create_from_markdown(&md, &out, "Sheet1").expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Sheet1").expect("range");
        assert_eq!(
            range.get_value((0, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("A".to_string())
        );
        // "1" should be written as number
        assert_eq!(
            range.get_value((1, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("1".to_string())
        );
    }
}
