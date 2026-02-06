//! VTx Specification.
//!
//! The VtxSpec defines how a virtual transaction's components map to positions
//! in the complete (composed) transaction. It is stored in the first input's
//! type script witness.

use serde::{Deserialize, Serialize};
use std::ops::Range;

use crate::error::{VtxError, VtxResult};

/// Specification for how a VTx's components map to the complete transaction.
///
/// Each VTx owns non-overlapping (disjoint) ranges of input and output indices
/// in the composed transaction. Cell deps and header deps are merged with
/// deduplication.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VtxSpec {
    /// The range of input indices this VTx occupies in the composed tx.
    /// Must be disjoint with input ranges of all other VTxs.
    pub input_range: Range<usize>,

    /// The range of output indices this VTx occupies in the composed tx.
    /// Must be disjoint with output ranges of all other VTxs.
    pub output_range: Range<usize>,

    /// Index of this VTx within the set of VTxs being composed.
    pub vtx_index: usize,

    /// Total number of VTxs expected in the composition.
    pub total_vtxs: usize,
}

impl Default for VtxSpec {
    fn default() -> Self {
        Self {
            input_range: 0..0,
            output_range: 0..0,
            vtx_index: 0,
            total_vtxs: 1,
        }
    }
}

impl VtxSpec {
    /// Create a new VtxSpec.
    pub fn new(
        input_range: Range<usize>,
        output_range: Range<usize>,
        vtx_index: usize,
        total_vtxs: usize,
    ) -> Self {
        Self {
            input_range,
            output_range,
            vtx_index,
            total_vtxs,
        }
    }

    /// Validate that this spec does not overlap with another spec.
    pub fn validate_no_overlap(&self, other: &VtxSpec) -> VtxResult<()> {
        if ranges_overlap(&self.input_range, &other.input_range) {
            return Err(VtxError::OverlappingRanges(format!(
                "input ranges {:?} and {:?} overlap",
                self.input_range, other.input_range
            )));
        }
        if ranges_overlap(&self.output_range, &other.output_range) {
            return Err(VtxError::OverlappingRanges(format!(
                "output ranges {:?} and {:?} overlap",
                self.output_range, other.output_range
            )));
        }
        Ok(())
    }

    /// Serialize the spec to bytes for inclusion in witness data.
    pub fn to_bytes(&self) -> VtxResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| VtxError::SerializationError(e.to_string()))
    }

    /// Deserialize a spec from witness bytes.
    pub fn from_bytes(bytes: &[u8]) -> VtxResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| VtxError::SerializationError(e.to_string()))
    }

    /// Map a global input index to a local index within this VTx.
    /// Returns None if the global index is not within this VTx's range.
    pub fn global_to_local_input(&self, global_index: usize) -> Option<usize> {
        if self.input_range.contains(&global_index) {
            Some(global_index - self.input_range.start)
        } else {
            None
        }
    }

    /// Map a global output index to a local index within this VTx.
    /// Returns None if the global index is not within this VTx's range.
    pub fn global_to_local_output(&self, global_index: usize) -> Option<usize> {
        if self.output_range.contains(&global_index) {
            Some(global_index - self.output_range.start)
        } else {
            None
        }
    }

    /// Map a local input index to the global index in the composed tx.
    pub fn local_to_global_input(&self, local_index: usize) -> Option<usize> {
        let global = self.input_range.start + local_index;
        if self.input_range.contains(&global) {
            Some(global)
        } else {
            None
        }
    }

    /// Map a local output index to the global index in the composed tx.
    pub fn local_to_global_output(&self, local_index: usize) -> Option<usize> {
        let global = self.output_range.start + local_index;
        if self.output_range.contains(&global) {
            Some(global)
        } else {
            None
        }
    }
}

/// Check if two ranges overlap.
fn ranges_overlap(a: &Range<usize>, b: &Range<usize>) -> bool {
    !a.is_empty() && !b.is_empty() && a.start < b.end && b.start < a.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_overlapping_ranges() {
        let spec1 = VtxSpec::new(0..3, 0..2, 0, 2);
        let spec2 = VtxSpec::new(3..5, 2..4, 1, 2);
        assert!(spec1.validate_no_overlap(&spec2).is_ok());
    }

    #[test]
    fn test_overlapping_input_ranges() {
        let spec1 = VtxSpec::new(0..3, 0..2, 0, 2);
        let spec2 = VtxSpec::new(2..5, 2..4, 1, 2);
        assert!(spec1.validate_no_overlap(&spec2).is_err());
    }

    #[test]
    fn test_overlapping_output_ranges() {
        let spec1 = VtxSpec::new(0..3, 0..3, 0, 2);
        let spec2 = VtxSpec::new(3..5, 2..4, 1, 2);
        assert!(spec1.validate_no_overlap(&spec2).is_err());
    }

    #[test]
    fn test_empty_ranges_no_overlap() {
        let spec1 = VtxSpec::new(0..0, 0..0, 0, 2);
        let spec2 = VtxSpec::new(0..0, 0..0, 1, 2);
        assert!(spec1.validate_no_overlap(&spec2).is_ok());
    }

    #[test]
    fn test_global_to_local_mapping() {
        let spec = VtxSpec::new(3..6, 2..5, 1, 2);

        // Input mapping
        assert_eq!(spec.global_to_local_input(3), Some(0));
        assert_eq!(spec.global_to_local_input(5), Some(2));
        assert_eq!(spec.global_to_local_input(6), None);
        assert_eq!(spec.global_to_local_input(2), None);

        // Output mapping
        assert_eq!(spec.global_to_local_output(2), Some(0));
        assert_eq!(spec.global_to_local_output(4), Some(2));
        assert_eq!(spec.global_to_local_output(5), None);
    }

    #[test]
    fn test_local_to_global_mapping() {
        let spec = VtxSpec::new(3..6, 2..5, 1, 2);

        assert_eq!(spec.local_to_global_input(0), Some(3));
        assert_eq!(spec.local_to_global_input(2), Some(5));
        assert_eq!(spec.local_to_global_input(3), None);

        assert_eq!(spec.local_to_global_output(0), Some(2));
        assert_eq!(spec.local_to_global_output(2), Some(4));
        assert_eq!(spec.local_to_global_output(3), None);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let spec = VtxSpec::new(0..5, 0..3, 0, 2);
        let bytes = spec.to_bytes().unwrap();
        let deserialized = VtxSpec::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.input_range, spec.input_range);
        assert_eq!(deserialized.output_range, spec.output_range);
        assert_eq!(deserialized.vtx_index, spec.vtx_index);
        assert_eq!(deserialized.total_vtxs, spec.total_vtxs);
    }
}
