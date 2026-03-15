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
            csv::ErrorKind::Io(io_err)
                if io_err.kind() == std::io::ErrorKind::NotFound =>
            {
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

    workbook.save(out_path).map_err(|error| XliError::OoxmlCorrupt {
        details: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{create_blank, create_from_csv};
    use calamine::{Reader, Xlsx, open_workbook};
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
            range.get_value((0, 0)).map(|cell: &calamine::Data| cell.to_string()),
            Some("name".to_string())
        );
        assert_eq!(
            range.get_value((1, 0)).map(|cell: &calamine::Data| cell.to_string()),
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
            range.get_value((1, 0)).map(|cell: &calamine::Data| cell.to_string()),
            Some("Smith, John".to_string()),
            "quoted field with comma should be a single cell"
        );
        assert_eq!(
            range.get_value((1, 1)).map(|cell: &calamine::Data| cell.to_string()),
            Some("New York".to_string())
        );
    }
}
