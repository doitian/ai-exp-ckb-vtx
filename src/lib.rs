//! # CKB Virtual Tx (VTx)
//!
//! Virtual Tx is a solution to implement partial transactions for CKB.
//!
//! ## Overview
//!
//! A virtual transaction (VTx) represents a subset of a complete CKB transaction.
//! Multiple VTxs can be composed together to form a complete transaction. This
//! enables partial signing, collaborative transaction construction, and
//! economically incentivized transaction matching.
//!
//! ## Key Concepts
//!
//! - **Virtual Tx**: A partial transaction containing a subset of inputs, outputs,
//!   cell deps, header deps, and witnesses.
//! - **VTx Specification**: Metadata in the first input's type script witness that
//!   describes how VTx components map to the complete transaction.
//! - **Syscall Mocking**: Override CKB syscalls so scripts access virtual tx data
//!   instead of the real current transaction.
//! - **Script Group Handling**: Lock script groups across multiple VTxs run in
//!   multiple passes.
//! - **Intent Declaration**: The first input carries the intent of the partial tx,
//!   declaring requirements on missing parts.

pub mod constants;
pub mod error;
pub mod intent;
pub mod mock_syscalls;
pub mod spec;
pub mod types;

pub use error::VtxError;
pub use intent::Intent;
pub use mock_syscalls::VtxSyscallContext;
pub use spec::VtxSpec;
pub use types::VirtualTx;
