//! Core types for CKB Virtual Tx.
//!
//! A `VirtualTx` represents a partial transaction that can be composed with
//! other VTxs to form a complete CKB transaction.

use ckb_types::core::{HeaderView, TransactionBuilder, TransactionView};
use ckb_types::packed::{self, Byte32, CellDep, CellInput, CellOutput};
use ckb_types::prelude::*;
use serde::{Deserialize, Serialize};

use crate::constants::{Source, VTX_INDICATOR_CODE_HASH};
use crate::error::{VtxError, VtxResult};
use crate::spec::VtxSpec;

/// A virtual transaction containing a partial set of CKB transaction components.
///
/// Each VTx owns an exclusive subset of the complete transaction's inputs,
/// outputs, witnesses, cell deps, and header deps as specified by its `VtxSpec`.
#[derive(Clone, Debug)]
pub struct VirtualTx {
    /// The inputs owned by this VTx.
    pub inputs: Vec<CellInput>,
    /// The outputs owned by this VTx.
    pub outputs: Vec<CellOutput>,
    /// The output data corresponding to each output.
    pub outputs_data: Vec<packed::Bytes>,
    /// The witnesses for inputs in this VTx.
    pub witnesses: Vec<packed::Bytes>,
    /// Cell dependencies referenced by this VTx.
    pub cell_deps: Vec<CellDep>,
    /// Header dependencies referenced by this VTx.
    pub header_deps: Vec<Byte32>,
    /// Resolved input cells (the actual cell data for each input).
    /// Used by the mock syscall layer to serve load_cell for Source::Input.
    pub resolved_inputs: Vec<ResolvedCell>,
    /// Resolved cell dep cells.
    pub resolved_dep_cells: Vec<ResolvedCell>,
    /// Resolved headers for header deps.
    pub resolved_headers: Vec<HeaderView>,
    /// The specification describing how this VTx maps to the complete tx.
    pub spec: VtxSpec,
}

/// A resolved cell with its output and data.
#[derive(Clone, Debug)]
pub struct ResolvedCell {
    /// The cell output (capacity, lock, type).
    pub output: CellOutput,
    /// The cell data.
    pub data: Vec<u8>,
}

impl VirtualTx {
    /// Create a new empty VirtualTx with the given spec.
    pub fn new(spec: VtxSpec) -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            outputs_data: Vec::new(),
            witnesses: Vec::new(),
            cell_deps: Vec::new(),
            header_deps: Vec::new(),
            resolved_inputs: Vec::new(),
            resolved_dep_cells: Vec::new(),
            resolved_headers: Vec::new(),
            spec,
        }
    }

    /// Check if the first input has the VTx indicator type script.
    ///
    /// The first input's type script code_hash must match `VTX_INDICATOR_CODE_HASH`
    /// to identify this transaction as containing virtual transactions.
    pub fn is_vtx_indicator(input_cell: &CellOutput) -> bool {
        if let Some(type_script) = input_cell.type_().to_opt() {
            let code_hash: [u8; 32] = type_script.code_hash().unpack();
            code_hash == VTX_INDICATOR_CODE_HASH
        } else {
            false
        }
    }

    /// Get the number of inputs in this VTx.
    pub fn inputs_len(&self) -> usize {
        self.inputs.len()
    }

    /// Get the number of outputs in this VTx.
    pub fn outputs_len(&self) -> usize {
        self.outputs.len()
    }

    /// Get an input by local index.
    pub fn get_input(&self, index: usize) -> Option<&CellInput> {
        self.inputs.get(index)
    }

    /// Get an output by local index.
    pub fn get_output(&self, index: usize) -> Option<&CellOutput> {
        self.outputs.get(index)
    }

    /// Get output data by local index.
    pub fn get_output_data(&self, index: usize) -> Option<&packed::Bytes> {
        self.outputs_data.get(index)
    }

    /// Get a witness by local index.
    pub fn get_witness(&self, index: usize) -> Option<&packed::Bytes> {
        self.witnesses.get(index)
    }

    /// Get a resolved input cell by local index.
    pub fn get_resolved_input(&self, index: usize) -> Option<&ResolvedCell> {
        self.resolved_inputs.get(index)
    }

    /// Get cell for the given source and index.
    /// For Source::Input, returns the resolved input cell.
    /// For Source::Output, returns the output cell.
    /// For Source::CellDep, returns the resolved dep cell.
    pub fn get_cell(&self, source: Source, index: usize) -> Option<&CellOutput> {
        match source {
            Source::Input | Source::GroupInput => {
                self.resolved_inputs.get(index).map(|c| &c.output)
            }
            Source::Output | Source::GroupOutput => self.outputs.get(index),
            Source::CellDep => self.resolved_dep_cells.get(index).map(|c| &c.output),
            _ => None,
        }
    }

    /// Get cell data for the given source and index.
    pub fn get_cell_data(&self, source: Source, index: usize) -> Option<Vec<u8>> {
        match source {
            Source::Input | Source::GroupInput => {
                self.resolved_inputs.get(index).map(|c| c.data.clone())
            }
            Source::Output | Source::GroupOutput => {
                self.outputs_data.get(index).map(|d| d.raw_data().to_vec())
            }
            Source::CellDep => {
                self.resolved_dep_cells.get(index).map(|c| c.data.clone())
            }
            _ => None,
        }
    }

    /// Validate that this VTx's spec does not conflict with another VTx's spec.
    pub fn validate_no_overlap(&self, other: &VirtualTx) -> VtxResult<()> {
        self.spec.validate_no_overlap(&other.spec)
    }

    /// Compose multiple VTxs into a complete transaction.
    ///
    /// The VTxs must have non-overlapping specs. Components are placed in the
    /// complete transaction according to each VTx's spec ranges.
    pub fn compose(vtxs: &[VirtualTx]) -> VtxResult<TransactionView> {
        // Validate no overlaps
        for i in 0..vtxs.len() {
            for j in (i + 1)..vtxs.len() {
                vtxs[i].validate_no_overlap(&vtxs[j])?;
            }
        }

        // Calculate total sizes
        let total_inputs: usize = vtxs.iter().map(|v| v.spec.input_range.len()).sum();
        let total_outputs: usize = vtxs.iter().map(|v| v.spec.output_range.len()).sum();

        // Allocate vectors for the composed tx
        let mut all_inputs: Vec<Option<CellInput>> = vec![None; total_inputs];
        let mut all_outputs: Vec<Option<CellOutput>> = vec![None; total_outputs];
        let mut all_outputs_data: Vec<Option<packed::Bytes>> = vec![None; total_outputs];
        let mut all_witnesses: Vec<Option<packed::Bytes>> = vec![None; total_inputs];
        let mut all_cell_deps: Vec<CellDep> = Vec::new();
        let mut all_header_deps: Vec<Byte32> = Vec::new();

        for vtx in vtxs {
            // Place inputs at their spec positions
            for (local_idx, global_idx) in vtx.spec.input_range.clone().enumerate() {
                if global_idx >= total_inputs {
                    return Err(VtxError::InvalidSpec(format!(
                        "input index {} exceeds total {}",
                        global_idx, total_inputs
                    )));
                }
                if let Some(input) = vtx.inputs.get(local_idx) {
                    all_inputs[global_idx] = Some(input.clone());
                }
            }

            // Place outputs at their spec positions
            for (local_idx, global_idx) in vtx.spec.output_range.clone().enumerate() {
                if global_idx >= total_outputs {
                    return Err(VtxError::InvalidSpec(format!(
                        "output index {} exceeds total {}",
                        global_idx, total_outputs
                    )));
                }
                if let Some(output) = vtx.outputs.get(local_idx) {
                    all_outputs[global_idx] = Some(output.clone());
                }
                if let Some(data) = vtx.outputs_data.get(local_idx) {
                    all_outputs_data[global_idx] = Some(data.clone());
                }
            }

            // Place witnesses at their spec positions
            for (local_idx, global_idx) in vtx.spec.input_range.clone().enumerate() {
                if let Some(witness) = vtx.witnesses.get(local_idx) {
                    all_witnesses[global_idx] = Some(witness.clone());
                }
            }

            // Merge cell deps (dedup by out_point)
            for dep in &vtx.cell_deps {
                let already_exists = all_cell_deps
                    .iter()
                    .any(|d| d.out_point().as_slice() == dep.out_point().as_slice());
                if !already_exists {
                    all_cell_deps.push(dep.clone());
                }
            }

            // Merge header deps (dedup by hash)
            for hd in &vtx.header_deps {
                let already_exists = all_header_deps
                    .iter()
                    .any(|h| h.as_slice() == hd.as_slice());
                if !already_exists {
                    all_header_deps.push(hd.clone());
                }
            }
        }

        // Build the transaction
        let mut builder = TransactionBuilder::default();

        for input in all_inputs.into_iter().flatten() {
            builder = builder.input(input);
        }
        for output in all_outputs.into_iter().flatten() {
            builder = builder.output(output);
        }
        for data in all_outputs_data.into_iter() {
            builder = builder.output_data(
                data.unwrap_or_default().raw_data(),
            );
        }
        for witness in all_witnesses.into_iter() {
            builder = builder.witness(
                witness.unwrap_or_default().raw_data(),
            );
        }
        for dep in all_cell_deps {
            builder = builder.cell_dep(dep);
        }
        for hd in all_header_deps {
            builder = builder.header_dep(hd);
        }

        Ok(builder.build())
    }
}

/// Identifies which script group a script belongs to across VTxs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptGroupInfo {
    /// The script hash identifying this group.
    pub script_hash: [u8; 32],
    /// Indices of VTxs that contain inputs with this lock script.
    pub vtx_indices: Vec<usize>,
    /// For each VTx, the local input indices belonging to this script group.
    pub local_input_indices: Vec<Vec<usize>>,
}

/// Collect lock script groups across multiple VTxs.
///
/// Lock scripts are grouped by their script hash. A script group that spans
/// multiple VTxs requires multiple verification passes.
pub fn collect_script_groups(vtxs: &[VirtualTx]) -> Vec<ScriptGroupInfo> {
    let mut groups: Vec<ScriptGroupInfo> = Vec::new();

    for (vtx_idx, vtx) in vtxs.iter().enumerate() {
        for (input_idx, resolved) in vtx.resolved_inputs.iter().enumerate() {
            let lock_hash: [u8; 32] = resolved.output.lock().calc_script_hash().unpack();

            if let Some(group) = groups.iter_mut().find(|g| g.script_hash == lock_hash) {
                if let Some(pos) = group.vtx_indices.iter().position(|&v| v == vtx_idx) {
                    group.local_input_indices[pos].push(input_idx);
                } else {
                    group.vtx_indices.push(vtx_idx);
                    group.local_input_indices.push(vec![input_idx]);
                }
            } else {
                groups.push(ScriptGroupInfo {
                    script_hash: lock_hash,
                    vtx_indices: vec![vtx_idx],
                    local_input_indices: vec![vec![input_idx]],
                });
            }
        }
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::VtxSpec;
    use ckb_types::packed;
    use ckb_types::packed::Uint64;
    use ckb_types::prelude::Pack;

    fn dummy_cell_output(capacity: u64) -> CellOutput {
        let lock_script = packed::Script::new_builder()
            .code_hash(packed::Byte32::zero())
            .hash_type(packed::Byte::new(0))
            .build();
        let cap: Uint64 = capacity.pack();
        CellOutput::new_builder()
            .capacity(cap)
            .lock(lock_script)
            .build()
    }

    fn dummy_cell_input(index: u32) -> CellInput {
        let out_point = packed::OutPoint::new(packed::Byte32::zero(), index);
        CellInput::new(out_point, 0)
    }

    #[test]
    fn test_is_vtx_indicator() {
        let vtx_type_script = packed::Script::new_builder()
            .code_hash(VTX_INDICATOR_CODE_HASH.pack())
            .hash_type(packed::Byte::new(0))
            .build();
        let cap: Uint64 = 100u64.pack();
        let cell = CellOutput::new_builder()
            .capacity(cap)
            .lock(packed::Script::default())
            .type_(Some(vtx_type_script).pack())
            .build();
        assert!(VirtualTx::is_vtx_indicator(&cell));
    }

    #[test]
    fn test_is_not_vtx_indicator() {
        let cell = dummy_cell_output(100);
        assert!(!VirtualTx::is_vtx_indicator(&cell));
    }

    #[test]
    fn test_compose_two_vtxs() {
        let spec1 = VtxSpec {
            input_range: 0..2,
            output_range: 0..1,
            ..VtxSpec::default()
        };
        let spec2 = VtxSpec {
            input_range: 2..4,
            output_range: 1..3,
            ..VtxSpec::default()
        };

        let mut vtx1 = VirtualTx::new(spec1);
        vtx1.inputs.push(dummy_cell_input(0));
        vtx1.inputs.push(dummy_cell_input(1));
        vtx1.outputs.push(dummy_cell_output(100));
        vtx1.outputs_data.push(packed::Bytes::default());

        let mut vtx2 = VirtualTx::new(spec2);
        vtx2.inputs.push(dummy_cell_input(2));
        vtx2.inputs.push(dummy_cell_input(3));
        vtx2.outputs.push(dummy_cell_output(200));
        vtx2.outputs.push(dummy_cell_output(300));
        vtx2.outputs_data.push(packed::Bytes::default());
        vtx2.outputs_data.push(packed::Bytes::default());

        let tx = VirtualTx::compose(&[vtx1, vtx2]).unwrap();
        assert_eq!(tx.inputs().len(), 4);
        assert_eq!(tx.outputs().len(), 3);
    }

    #[test]
    fn test_compose_dedup_cell_deps() {
        let spec1 = VtxSpec {
            input_range: 0..1,
            output_range: 0..1,
            ..VtxSpec::default()
        };
        let spec2 = VtxSpec {
            input_range: 1..2,
            output_range: 1..2,
            ..VtxSpec::default()
        };

        let shared_dep = packed::CellDep::new_builder()
            .out_point(packed::OutPoint::new(packed::Byte32::zero(), 0))
            .build();

        let mut vtx1 = VirtualTx::new(spec1);
        vtx1.inputs.push(dummy_cell_input(0));
        vtx1.outputs.push(dummy_cell_output(100));
        vtx1.outputs_data.push(packed::Bytes::default());
        vtx1.cell_deps.push(shared_dep.clone());

        let mut vtx2 = VirtualTx::new(spec2);
        vtx2.inputs.push(dummy_cell_input(1));
        vtx2.outputs.push(dummy_cell_output(200));
        vtx2.outputs_data.push(packed::Bytes::default());
        vtx2.cell_deps.push(shared_dep);

        let tx = VirtualTx::compose(&[vtx1, vtx2]).unwrap();
        // Should deduplicate the shared cell dep
        assert_eq!(tx.cell_deps().len(), 1);
    }

    #[test]
    fn test_compose_overlapping_ranges_fails() {
        let spec1 = VtxSpec {
            input_range: 0..3,
            output_range: 0..1,
            ..VtxSpec::default()
        };
        let spec2 = VtxSpec {
            input_range: 2..4, // overlaps with spec1 at index 2
            output_range: 1..2,
            ..VtxSpec::default()
        };

        let vtx1 = VirtualTx::new(spec1);
        let vtx2 = VirtualTx::new(spec2);

        let result = VirtualTx::compose(&[vtx1, vtx2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_script_groups() {
        let lock1 = packed::Script::new_builder()
            .code_hash(packed::Byte32::zero())
            .hash_type(packed::Byte::new(0))
            .args(vec![1u8].pack())
            .build();
        let lock2 = packed::Script::new_builder()
            .code_hash(packed::Byte32::zero())
            .hash_type(packed::Byte::new(0))
            .args(vec![2u8].pack())
            .build();

        let cap100: Uint64 = 100u64.pack();
        let cap200: Uint64 = 200u64.pack();
        let cap300: Uint64 = 300u64.pack();
        let cell1 = CellOutput::new_builder()
            .capacity(cap100)
            .lock(lock1.clone())
            .build();
        let cell2 = CellOutput::new_builder()
            .capacity(cap200)
            .lock(lock2)
            .build();
        let cell3 = CellOutput::new_builder()
            .capacity(cap300)
            .lock(lock1)
            .build();

        let spec1 = VtxSpec {
            input_range: 0..1,
            output_range: 0..1,
            ..VtxSpec::default()
        };
        let spec2 = VtxSpec {
            input_range: 1..3,
            output_range: 1..2,
            ..VtxSpec::default()
        };

        let mut vtx1 = VirtualTx::new(spec1);
        vtx1.resolved_inputs.push(ResolvedCell {
            output: cell1,
            data: vec![],
        });

        let mut vtx2 = VirtualTx::new(spec2);
        vtx2.resolved_inputs.push(ResolvedCell {
            output: cell2,
            data: vec![],
        });
        vtx2.resolved_inputs.push(ResolvedCell {
            output: cell3,
            data: vec![],
        });

        let groups = collect_script_groups(&[vtx1, vtx2]);

        // lock1 appears in vtx1[0] and vtx2[1] - spans 2 VTxs
        // lock2 appears in vtx2[0] - single VTx
        assert_eq!(groups.len(), 2);

        let lock1_group = groups.iter().find(|g| g.vtx_indices.len() == 2).unwrap();
        assert_eq!(lock1_group.vtx_indices, vec![0, 1]);

        let lock2_group = groups.iter().find(|g| g.vtx_indices.len() == 1).unwrap();
        assert_eq!(lock2_group.vtx_indices, vec![1]);
    }
}
