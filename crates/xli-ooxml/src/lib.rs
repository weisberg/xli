#![forbid(unsafe_code)]

//! Workbook mutation helpers.

mod editor;
pub mod package;
pub mod shared_strings;

pub use editor::{
    BatchSummary, UMYA_FALLBACK_WARNING, apply_batch, apply_format, apply_sheet_action,
    apply_write, write_workbook,
};
pub use package::{WorkbookPatcher, XmlReader, XmlWriter};
pub use shared_strings::SharedStringTable;
