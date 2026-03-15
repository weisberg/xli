#![forbid(unsafe_code)]

//! File locking, fingerprinting, staging, and atomic commit helpers.

mod commit;
mod fingerprint;
mod lock;
mod staging;

pub use commit::{
    AtomicCommitOptions, CommitResult, atomic_commit, atomic_commit_with_options,
    validate_ooxml_file,
};
pub use fingerprint::fingerprint;
pub use lock::WorkbookLock;
pub use staging::stage_temp_file;
