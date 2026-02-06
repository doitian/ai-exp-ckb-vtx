# CKB Virtual Tx (VTx)

Virtual Tx is a solution to implement partial transactions for CKB.

## Overview

A virtual transaction (VTx) represents a subset of a complete CKB transaction. Multiple VTxs can be composed together to form a complete transaction. This enables partial signing, collaborative transaction construction, and economically incentivized transaction matching.

## Key Concepts

### Virtual Transaction

A `VirtualTx` contains a partial set of transaction components:
- Inputs (exclusive ownership)
- Outputs (exclusive ownership)
- Witnesses
- Cell dependencies (merged with deduplication on compose)
- Header dependencies (merged with deduplication on compose)

### Syscall Mocking

CKB scripts access transaction data through syscalls (`ckb_load_cell`, `ckb_load_input`, `ckb_load_witness`, etc.). The `VtxSyscallContext` intercepts these calls and serves data from the virtual transaction instead of the real current transaction.

- Override [ckb-c-stdlib/ckb_raw_syscalls.h](https://github.com/nervosnetwork/ckb-c-stdlib/blob/master/ckb_raw_syscalls.h) using the `weak_alias` mechanism
- Or override [ckb-std/src/syscalls/native.rs](https://github.com/nervosnetwork/ckb-std/blob/master/src/syscalls/native.rs)

### VTx Indicator

The first input's type script code hash is used as the indicator that the transaction contains virtual transactions. This uses a well-known code hash (`VTX_INDICATOR_CODE_HASH`).

### VTx Specification

The first input's type script witness includes a `VtxSpec` that describes how each VTx's components map to positions in the complete transaction:
- Input ranges are exclusive (no two VTxs share the same input index)
- Output ranges are exclusive
- Cell deps and header deps are deduplicated when composing

### Script Group Handling

Lock script groups that span multiple VTxs require multiple verification passes. The `collect_script_groups` function identifies these cross-VTx groups.

### Intent Declaration

The first input can carry an `Intent` that declares the partial tx's requirements on the missing parts:
- Output constraints (capacity, lock/type script hash)
- Fee constraints (maximum allowed fee)
- Minimum input/output counts
- Application-specific metadata

Anyone is economically incentivized to match partial txs because fee can be provided as part of the matching.

## Architecture

```
src/
├── lib.rs           # Library root with public API
├── constants.rs     # CKB syscall numbers, source types, field types
├── error.rs         # Error types
├── types.rs         # VirtualTx, ResolvedCell, ScriptGroupInfo, compose()
├── spec.rs          # VtxSpec - mapping between VTx and composed tx
├── intent.rs        # Intent declaration for partial tx requirements
└── mock_syscalls.rs # VtxSyscallContext - mock CKB syscalls
```

## Usage

```rust
use ckb_vtx::{VirtualTx, VtxSpec, VtxSyscallContext, Intent};

// Create a VTx spec for the first partial tx (inputs 0..2, outputs 0..1)
let spec = VtxSpec::new(0..2, 0..1, 0, 2);
let mut vtx = VirtualTx::new(spec);

// Add inputs, outputs, witnesses, cell deps...
// vtx.inputs.push(...);
// vtx.outputs.push(...);

// Create a mock syscall context for script verification
// let ctx = VtxSyscallContext::new(vtx, current_script);

// Compose multiple VTxs into a complete transaction
// let tx = VirtualTx::compose(&[vtx1, vtx2]).unwrap();
```

## Building

```bash
cargo build
```

## Testing

```bash
cargo test
```