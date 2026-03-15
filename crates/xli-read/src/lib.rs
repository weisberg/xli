#![forbid(unsafe_code)]

//! Read-only workbook inspection helpers.

pub mod inspect;
pub mod read;

pub use inspect::{SheetInfo, WorkbookInfo, inspect};
pub use read::{CellData, CellValueType, RangeData, read_cell, read_range, read_table};
