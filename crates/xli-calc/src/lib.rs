#![forbid(unsafe_code)]

//! Formula recalculation helpers.

mod libreoffice;

pub use libreoffice::{RecalcResult, recalc};
