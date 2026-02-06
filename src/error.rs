//! Error types for CKB Virtual Tx.

use thiserror::Error;

/// Errors that can occur during VTx operations.
#[derive(Error, Debug)]
pub enum VtxError {
    /// Index out of bound when accessing tx components.
    #[error("index out of bound: {context} index {index}")]
    IndexOutOfBound { context: String, index: usize },

    /// Requested item is missing (e.g., type script on a cell).
    #[error("item missing: {0}")]
    ItemMissing(String),

    /// Invalid source provided to a syscall.
    #[error("invalid source: {0}")]
    InvalidSource(u64),

    /// Invalid field provided to a syscall.
    #[error("invalid field: {0}")]
    InvalidField(u64),

    /// VTx specification is invalid.
    #[error("invalid vtx spec: {0}")]
    InvalidSpec(String),

    /// Component ranges overlap between VTxs.
    #[error("overlapping ranges in vtx spec: {0}")]
    OverlappingRanges(String),

    /// Script group mismatch across VTxs.
    #[error("script group error: {0}")]
    ScriptGroupError(String),

    /// The intent declaration is invalid or unsatisfied.
    #[error("intent error: {0}")]
    IntentError(String),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    SerializationError(String),
}

/// Result type alias for VTx operations.
pub type VtxResult<T> = Result<T, VtxError>;
