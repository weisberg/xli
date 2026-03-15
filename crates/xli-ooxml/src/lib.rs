#![forbid(unsafe_code)]

//! Workbook mutation helpers.

mod editor;

pub use editor::{
    BatchSummary, UMYA_FALLBACK_WARNING, apply_batch, apply_format, apply_sheet_action,
    apply_write, write_workbook,
};
