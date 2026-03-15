#![forbid(unsafe_code)]

//! Read-only workbook inspection helpers.

pub mod inspect;

pub use inspect::{SheetInfo, WorkbookInfo, inspect};
