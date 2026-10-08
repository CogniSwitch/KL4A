//! Error type whose `Display` output matches the exact Python exception messages
//! `validate_bundle` surfaces verbatim (docs/port/port-mapping-a-core-data.md §3.3,
//! §3.6): `str(exc)` for a caught exception becomes the message text callers see, so
//! these strings are load-bearing, not just diagnostics.

use std::fmt;

#[derive(Debug)]
pub enum SopkbError {
    Io(std::io::Error),
    /// "manifest.yaml must contain a mapping" / other ValueError-equivalent messages.
    Value(String),
    /// "Missing manifest: <path>" / "Source directory does not exist: <path>" (Python
    /// FileNotFoundError-equivalent).
    NotFound(String),
    /// A YAML or JSON parse failure; message mirrors PyYAML's/json's own text closely
    /// enough for the "does this equal Python's str(exc)" cases that matter today
    /// (the malformed-* fixtures assert shape, not exact text -- see DECISIONS.md).
    Parse(String),
    /// A request is refused because it conflicts with in-progress state -- e.g.
    /// `sopkb-workbench`'s `set_workbench_root` refusing a folder switch while a
    /// mutation (an ingest step, an upload-staging write) is still in flight for the
    /// current root (PORT_PLAN.md P-W2/P-W10). New in Phase 9: Python's HTTP layer has
    /// no equivalent, because a request could never outlive the server process that
    /// was serving it.
    Conflict(String),
}

impl fmt::Display for SopkbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SopkbError::Io(e) => write!(f, "{e}"),
            SopkbError::Value(s) => write!(f, "{s}"),
            SopkbError::NotFound(s) => write!(f, "{s}"),
            SopkbError::Parse(s) => write!(f, "{s}"),
            SopkbError::Conflict(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for SopkbError {}

impl From<std::io::Error> for SopkbError {
    fn from(e: std::io::Error) -> Self {
        SopkbError::Io(e)
    }
}

/// Both directions of `serde_json` failure land in [`SopkbError::Parse`]. Deserializing
/// is the case that actually happens (a corrupt state file); serializing one of this
/// crate's own record structs cannot fail in practice, but goes here rather than
/// through an `unwrap` so a future struct with a non-string map key surfaces as an
/// error instead of a panic.
impl From<serde_json::Error> for SopkbError {
    fn from(e: serde_json::Error) -> Self {
        SopkbError::Parse(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SopkbError>;
