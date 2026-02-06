//! Mock syscall layer for CKB Virtual Tx.
//!
//! This module provides a mock implementation of CKB syscalls that redirects
//! data access to a virtual transaction instead of the real current transaction.
//!
//! ## How it works
//!
//! Scripts running in CKB-VM access transaction data through syscalls like
//! `ckb_load_cell`, `ckb_load_input`, `ckb_load_witness`, etc. The
//! `VtxSyscallContext` intercepts these calls and serves data from the
//! VirtualTx, translating between the virtual tx's local indices and the
//! script's expected view.
//!
//! ### Overriding syscalls
//!
//! For C scripts: override functions in `ckb_raw_syscalls.h` using the weak
//! alias mechanism (the functions are declared with `weak_alias`).
//!
//! For Rust scripts: override functions in `ckb-std/src/syscalls/native.rs`.
//!
//! This module provides the logic layer; the actual FFI override must be done
//! in the target environment (C or Rust on-chain script).

use ckb_types::packed;
use ckb_types::prelude::*;

use crate::constants::*;
use crate::error::VtxResult;
use crate::types::VirtualTx;

/// Context for mocking CKB syscalls against a VirtualTx.
///
/// This replaces the real syscall context so that scripts see the virtual
/// transaction's data instead of the real transaction's data.
///
/// ## Script Group Handling
///
/// For lock scripts, the script group may span multiple VTxs. The context
/// supports multiple passes by tracking which VTx indices belong to the
/// current script group and providing the combined group view.
pub struct VtxSyscallContext {
    /// The virtual transaction being served.
    vtx: VirtualTx,

    /// The script currently being verified.
    current_script: packed::Script,

    /// Indices of inputs in this VTx that belong to the current script group.
    /// Computed by matching lock script hashes.
    group_input_indices: Vec<usize>,

    /// Indices of outputs in this VTx that belong to the current script group.
    group_output_indices: Vec<usize>,
}

impl VtxSyscallContext {
    /// Create a new syscall context for the given VTx and script.
    pub fn new(vtx: VirtualTx, current_script: packed::Script) -> Self {
        let script_hash: [u8; 32] = current_script.calc_script_hash().unpack();

        // Compute group input indices
        let group_input_indices: Vec<usize> = vtx
            .resolved_inputs
            .iter()
            .enumerate()
            .filter(|(_, cell)| {
                let lock_hash: [u8; 32] = cell.output.lock().calc_script_hash().unpack();
                lock_hash == script_hash
            })
            .map(|(i, _)| i)
            .collect();

        // Compute group output indices
        let group_output_indices: Vec<usize> = vtx
            .outputs
            .iter()
            .enumerate()
            .filter(|(_, cell)| {
                let lock_hash: [u8; 32] = cell.lock().calc_script_hash().unpack();
                lock_hash == script_hash
            })
            .map(|(i, _)| i)
            .collect();

        Self {
            vtx,
            current_script,
            group_input_indices,
            group_output_indices,
        }
    }

    /// Resolve a group-relative index to a VTx-local index.
    fn resolve_index(&self, index: usize, source: Source) -> Option<usize> {
        match source {
            Source::GroupInput => self.group_input_indices.get(index).copied(),
            Source::GroupOutput => self.group_output_indices.get(index).copied(),
            _ => Some(index),
        }
    }

    /// Mock implementation of `ckb_load_tx_hash`.
    ///
    /// Note: In a VTx context, this returns a hash derived from the VTx's
    /// own components rather than the composed transaction hash. The real
    /// composed transaction hash is only available after all VTxs are composed.
    /// Callers that need the real tx hash should compose first, then use the
    /// resulting `TransactionView::hash()`.
    pub fn load_tx_hash(&self) -> VtxResult<[u8; 32]> {
        // In a real scenario, this would be the hash of the composed tx.
        // For the VTx context, we compute a hash from the VTx's inputs.
        use ckb_types::core::TransactionBuilder;

        let mut builder = TransactionBuilder::default();
        for input in &self.vtx.inputs {
            builder = builder.input(input.clone());
        }
        for output in &self.vtx.outputs {
            builder = builder.output(output.clone());
        }
        for data in &self.vtx.outputs_data {
            builder = builder.output_data(data.raw_data());
        }
        let tx = builder.build();
        Ok(tx.hash().unpack())
    }

    /// Mock implementation of `ckb_load_script_hash`.
    pub fn load_script_hash(&self) -> [u8; 32] {
        self.current_script.calc_script_hash().unpack()
    }

    /// Mock implementation of `ckb_load_script`.
    pub fn load_script(&self) -> Vec<u8> {
        self.current_script.as_slice().to_vec()
    }

    /// Mock implementation of `ckb_load_cell`.
    ///
    /// Loads the full serialized cell at the given index and source.
    pub fn load_cell(&self, index: usize, source_val: u64) -> Result<Vec<u8>, i32> {
        let source = Source::from_u64(source_val).ok_or(-1i32)?;
        let resolved_index = self.resolve_index(index, source).ok_or(CKB_INDEX_OUT_OF_BOUND)?;

        let cell = match source.base_source() {
            Source::Input => self.vtx.resolved_inputs.get(resolved_index).map(|c| &c.output),
            Source::Output => self.vtx.outputs.get(resolved_index),
            Source::CellDep => self
                .vtx
                .resolved_dep_cells
                .get(resolved_index)
                .map(|c| &c.output),
            _ => None,
        };

        cell.map(|c| c.as_slice().to_vec())
            .ok_or(CKB_INDEX_OUT_OF_BOUND)
    }

    /// Mock implementation of `ckb_load_input`.
    pub fn load_input(&self, index: usize, source_val: u64) -> Result<Vec<u8>, i32> {
        let source = Source::from_u64(source_val).ok_or(-1i32)?;
        let resolved_index = self.resolve_index(index, source).ok_or(CKB_INDEX_OUT_OF_BOUND)?;

        match source.base_source() {
            Source::Input => self
                .vtx
                .inputs
                .get(resolved_index)
                .map(|i| i.as_slice().to_vec())
                .ok_or(CKB_INDEX_OUT_OF_BOUND),
            _ => Err(CKB_INDEX_OUT_OF_BOUND),
        }
    }

    /// Mock implementation of `ckb_load_witness`.
    pub fn load_witness(&self, index: usize, source_val: u64) -> Result<Vec<u8>, i32> {
        let source = Source::from_u64(source_val).ok_or(-1i32)?;
        let resolved_index = self.resolve_index(index, source).ok_or(CKB_INDEX_OUT_OF_BOUND)?;

        match source.base_source() {
            Source::Input | Source::Output => self
                .vtx
                .witnesses
                .get(resolved_index)
                .map(|w| w.raw_data().to_vec())
                .ok_or(CKB_INDEX_OUT_OF_BOUND),
            _ => Err(CKB_INDEX_OUT_OF_BOUND),
        }
    }

    /// Mock implementation of `ckb_load_header`.
    pub fn load_header(&self, index: usize, source_val: u64) -> Result<Vec<u8>, i32> {
        let source = Source::from_u64(source_val).ok_or(-1i32)?;

        match source {
            Source::HeaderDep => self
                .vtx
                .resolved_headers
                .get(index)
                .map(|h| h.data().as_slice().to_vec())
                .ok_or(CKB_INDEX_OUT_OF_BOUND),
            _ => Err(CKB_INDEX_OUT_OF_BOUND),
        }
    }

    /// Mock implementation of `ckb_load_cell_by_field`.
    pub fn load_cell_by_field(
        &self,
        index: usize,
        source_val: u64,
        field_val: u64,
    ) -> Result<Vec<u8>, i32> {
        let source = Source::from_u64(source_val).ok_or(-1i32)?;
        let field = CellField::from_u64(field_val).ok_or(-1i32)?;
        let resolved_index = self.resolve_index(index, source).ok_or(CKB_INDEX_OUT_OF_BOUND)?;

        let cell = match source.base_source() {
            Source::Input => self.vtx.resolved_inputs.get(resolved_index).map(|c| &c.output),
            Source::Output => self.vtx.outputs.get(resolved_index),
            Source::CellDep => self
                .vtx
                .resolved_dep_cells
                .get(resolved_index)
                .map(|c| &c.output),
            _ => None,
        }
        .ok_or(CKB_INDEX_OUT_OF_BOUND)?;

        match field {
            CellField::Capacity => {
                let cap: u64 = cell.capacity().unpack();
                Ok(cap.to_le_bytes().to_vec())
            }
            CellField::DataHash => {
                let data = match source.base_source() {
                    Source::Input => self
                        .vtx
                        .resolved_inputs
                        .get(resolved_index)
                        .map(|c| c.data.clone()),
                    Source::CellDep => self
                        .vtx
                        .resolved_dep_cells
                        .get(resolved_index)
                        .map(|c| c.data.clone()),
                    Source::Output => self
                        .vtx
                        .outputs_data
                        .get(resolved_index)
                        .map(|d| d.raw_data().to_vec()),
                    _ => None,
                }
                .ok_or(CKB_INDEX_OUT_OF_BOUND)?;
                let hash = ckb_types::packed::CellOutput::calc_data_hash(&data);
                Ok(hash.as_slice().to_vec())
            }
            CellField::Lock => Ok(cell.lock().as_slice().to_vec()),
            CellField::LockHash => {
                let hash = cell.lock().calc_script_hash();
                Ok(hash.as_slice().to_vec())
            }
            CellField::Type => cell
                .type_()
                .to_opt()
                .map(|t| t.as_slice().to_vec())
                .ok_or(CKB_ITEM_MISSING),
            CellField::TypeHash => cell
                .type_()
                .to_opt()
                .map(|t| t.calc_script_hash().as_slice().to_vec())
                .ok_or(CKB_ITEM_MISSING),
            CellField::OccupiedCapacity => {
                // Approximate: output size determines occupied capacity
                let occupied = cell.as_slice().len() as u64;
                Ok(occupied.to_le_bytes().to_vec())
            }
        }
    }

    /// Mock implementation of `ckb_load_input_by_field`.
    pub fn load_input_by_field(
        &self,
        index: usize,
        source_val: u64,
        field_val: u64,
    ) -> Result<Vec<u8>, i32> {
        let source = Source::from_u64(source_val).ok_or(-1i32)?;
        let field = InputField::from_u64(field_val).ok_or(-1i32)?;
        let resolved_index = self.resolve_index(index, source).ok_or(CKB_INDEX_OUT_OF_BOUND)?;

        let input = self
            .vtx
            .inputs
            .get(resolved_index)
            .ok_or(CKB_INDEX_OUT_OF_BOUND)?;

        match field {
            InputField::OutPoint => Ok(input.previous_output().as_slice().to_vec()),
            InputField::Since => {
                let since: u64 = input.since().unpack();
                Ok(since.to_le_bytes().to_vec())
            }
        }
    }

    /// Mock implementation of `ckb_load_cell_data`.
    pub fn load_cell_data(&self, index: usize, source_val: u64) -> Result<Vec<u8>, i32> {
        let source = Source::from_u64(source_val).ok_or(-1i32)?;
        let resolved_index = self.resolve_index(index, source).ok_or(CKB_INDEX_OUT_OF_BOUND)?;

        match source.base_source() {
            Source::Input => self
                .vtx
                .resolved_inputs
                .get(resolved_index)
                .map(|c| c.data.clone())
                .ok_or(CKB_INDEX_OUT_OF_BOUND),
            Source::Output => self
                .vtx
                .outputs_data
                .get(resolved_index)
                .map(|d| d.raw_data().to_vec())
                .ok_or(CKB_INDEX_OUT_OF_BOUND),
            Source::CellDep => self
                .vtx
                .resolved_dep_cells
                .get(resolved_index)
                .map(|c| c.data.clone())
                .ok_or(CKB_INDEX_OUT_OF_BOUND),
            _ => Err(CKB_INDEX_OUT_OF_BOUND),
        }
    }

    /// Get the count of items available for the given source.
    pub fn get_count(&self, source_val: u64) -> Result<usize, i32> {
        let source = Source::from_u64(source_val).ok_or(-1i32)?;
        match source {
            Source::Input => Ok(self.vtx.inputs.len()),
            Source::Output => Ok(self.vtx.outputs.len()),
            Source::CellDep => Ok(self.vtx.cell_deps.len()),
            Source::HeaderDep => Ok(self.vtx.header_deps.len()),
            Source::GroupInput => Ok(self.group_input_indices.len()),
            Source::GroupOutput => Ok(self.group_output_indices.len()),
        }
    }

    /// Get a reference to the underlying VirtualTx.
    pub fn vtx(&self) -> &VirtualTx {
        &self.vtx
    }

    /// Get a reference to the current script.
    pub fn current_script(&self) -> &packed::Script {
        &self.current_script
    }

    /// Get the group input indices.
    pub fn group_input_indices(&self) -> &[usize] {
        &self.group_input_indices
    }

    /// Get the group output indices.
    pub fn group_output_indices(&self) -> &[usize] {
        &self.group_output_indices
    }

    /// Copy data from `src` into a buffer, respecting offset and length.
    /// Returns the total data length (useful for the CKB convention where
    /// the actual length is returned even if the buffer is too small).
    pub fn copy_with_offset(src: &[u8], buf: &mut [u8], offset: usize) -> usize {
        let total_len = src.len();
        if offset < total_len {
            let copy_len = std::cmp::min(buf.len(), total_len - offset);
            buf[..copy_len].copy_from_slice(&src[offset..offset + copy_len]);
        }
        total_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::VtxSpec;
    use crate::types::{ResolvedCell, VirtualTx};
    use ckb_types::packed;
    use ckb_types::packed::Uint64;
    use ckb_types::prelude::Pack;

    fn make_lock_script(args: &[u8]) -> packed::Script {
        packed::Script::new_builder()
            .code_hash(packed::Byte32::zero())
            .hash_type(packed::Byte::new(0))
            .args(args.to_vec().pack())
            .build()
    }

    fn make_cell_output(capacity: u64, lock: packed::Script) -> packed::CellOutput {
        let cap: Uint64 = capacity.pack();
        packed::CellOutput::new_builder()
            .capacity(cap)
            .lock(lock)
            .build()
    }

    fn setup_context() -> VtxSyscallContext {
        let lock1 = make_lock_script(&[1]);
        let lock2 = make_lock_script(&[2]);

        let spec = VtxSpec::new(0..3, 0..2, 0, 1);
        let mut vtx = VirtualTx::new(spec);

        // 3 inputs: 2 with lock1, 1 with lock2
        vtx.inputs.push(packed::CellInput::new(
            packed::OutPoint::new(packed::Byte32::zero(), 0),
            0,
        ));
        vtx.inputs.push(packed::CellInput::new(
            packed::OutPoint::new(packed::Byte32::zero(), 1),
            100,
        ));
        vtx.inputs.push(packed::CellInput::new(
            packed::OutPoint::new(packed::Byte32::zero(), 2),
            0,
        ));

        vtx.resolved_inputs.push(ResolvedCell {
            output: make_cell_output(1000, lock1.clone()),
            data: vec![0xaa, 0xbb],
        });
        vtx.resolved_inputs.push(ResolvedCell {
            output: make_cell_output(2000, lock2),
            data: vec![0xcc],
        });
        vtx.resolved_inputs.push(ResolvedCell {
            output: make_cell_output(3000, lock1.clone()),
            data: vec![],
        });

        // 2 outputs
        vtx.outputs.push(make_cell_output(500, lock1.clone()));
        vtx.outputs.push(make_cell_output(4500, lock1.clone()));
        vtx.outputs_data.push(packed::Bytes::default());
        vtx.outputs_data.push(packed::Bytes::default());

        // Witnesses
        vtx.witnesses.push(vec![0x01].pack());
        vtx.witnesses.push(vec![0x02].pack());
        vtx.witnesses.push(vec![0x03].pack());

        VtxSyscallContext::new(vtx, lock1)
    }

    #[test]
    fn test_load_cell_input_source() {
        let ctx = setup_context();

        // Load first input cell
        let cell_data = ctx.load_cell(0, Source::Input as u64).unwrap();
        assert!(!cell_data.is_empty());

        // Out of bounds
        let result = ctx.load_cell(5, Source::Input as u64);
        assert_eq!(result.unwrap_err(), CKB_INDEX_OUT_OF_BOUND);
    }

    #[test]
    fn test_load_cell_output_source() {
        let ctx = setup_context();
        let cell_data = ctx.load_cell(0, Source::Output as u64).unwrap();
        assert!(!cell_data.is_empty());
    }

    #[test]
    fn test_load_input() {
        let ctx = setup_context();
        let input_data = ctx.load_input(0, Source::Input as u64).unwrap();
        assert!(!input_data.is_empty());
    }

    #[test]
    fn test_load_witness() {
        let ctx = setup_context();
        let witness = ctx.load_witness(0, Source::Input as u64).unwrap();
        assert_eq!(witness, vec![0x01]);
    }

    #[test]
    fn test_group_input_indices() {
        let ctx = setup_context();
        // lock1 is used by inputs 0 and 2
        assert_eq!(ctx.group_input_indices(), &[0, 2]);
    }

    #[test]
    fn test_load_cell_group_input() {
        let ctx = setup_context();
        // GroupInput index 0 maps to local index 0 (lock1)
        let cell = ctx.load_cell(0, Source::GroupInput as u64).unwrap();
        assert!(!cell.is_empty());
        // GroupInput index 1 maps to local index 2 (lock1)
        let cell = ctx.load_cell(1, Source::GroupInput as u64).unwrap();
        assert!(!cell.is_empty());
        // GroupInput index 2 should be out of bounds
        let result = ctx.load_cell(2, Source::GroupInput as u64);
        assert_eq!(result.unwrap_err(), CKB_INDEX_OUT_OF_BOUND);
    }

    #[test]
    fn test_load_cell_by_field_capacity() {
        let ctx = setup_context();
        let data = ctx
            .load_cell_by_field(0, Source::Input as u64, CellField::Capacity as u64)
            .unwrap();
        let capacity = u64::from_le_bytes(data.try_into().unwrap());
        assert_eq!(capacity, 1000);
    }

    #[test]
    fn test_load_cell_by_field_lock() {
        let ctx = setup_context();
        let data = ctx
            .load_cell_by_field(0, Source::Input as u64, CellField::Lock as u64)
            .unwrap();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_load_cell_by_field_type_missing() {
        let ctx = setup_context();
        // Our test cells have no type script
        let result =
            ctx.load_cell_by_field(0, Source::Input as u64, CellField::Type as u64);
        assert_eq!(result.unwrap_err(), CKB_ITEM_MISSING);
    }

    #[test]
    fn test_load_input_by_field_since() {
        let ctx = setup_context();
        let data = ctx
            .load_input_by_field(1, Source::Input as u64, InputField::Since as u64)
            .unwrap();
        let since = u64::from_le_bytes(data.try_into().unwrap());
        assert_eq!(since, 100);
    }

    #[test]
    fn test_load_cell_data_input() {
        let ctx = setup_context();
        let data = ctx.load_cell_data(0, Source::Input as u64).unwrap();
        assert_eq!(data, vec![0xaa, 0xbb]);
    }

    #[test]
    fn test_load_script() {
        let ctx = setup_context();
        let script_bytes = ctx.load_script();
        assert!(!script_bytes.is_empty());
    }

    #[test]
    fn test_load_script_hash() {
        let ctx = setup_context();
        let hash = ctx.load_script_hash();
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn test_get_count() {
        let ctx = setup_context();
        assert_eq!(ctx.get_count(Source::Input as u64).unwrap(), 3);
        assert_eq!(ctx.get_count(Source::Output as u64).unwrap(), 2);
        assert_eq!(ctx.get_count(Source::GroupInput as u64).unwrap(), 2); // lock1 has 2 inputs
    }

    #[test]
    fn test_copy_with_offset() {
        let src = [1, 2, 3, 4, 5];
        let mut buf = [0u8; 3];

        let total = VtxSyscallContext::copy_with_offset(&src, &mut buf, 0);
        assert_eq!(total, 5);
        assert_eq!(buf, [1, 2, 3]);

        let total = VtxSyscallContext::copy_with_offset(&src, &mut buf, 2);
        assert_eq!(total, 5);
        assert_eq!(buf, [3, 4, 5]);

        let total = VtxSyscallContext::copy_with_offset(&src, &mut buf, 10);
        assert_eq!(total, 5);
        // buf unchanged since offset is past end
    }

    #[test]
    fn test_load_tx_hash() {
        let ctx = setup_context();
        let hash = ctx.load_tx_hash().unwrap();
        // Should return a non-zero hash
        assert_ne!(hash, [0u8; 32]);
    }
}
