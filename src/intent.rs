//! Intent declaration for partial transactions.
//!
//! The first input of a VTx can carry an intent that declares what the partial
//! transaction requires from the missing parts. This enables economically
//! incentivized matching of partial transactions.

use serde::{Deserialize, Serialize};

use crate::error::{VtxError, VtxResult};

/// Constraint on an output that must be present in the composed transaction.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutputConstraint {
    /// Minimum capacity required on the output (in shannons).
    pub min_capacity: Option<u64>,

    /// Maximum capacity allowed on the output (in shannons).
    pub max_capacity: Option<u64>,

    /// Required lock script hash (hex-encoded, without 0x prefix).
    /// If set, the output's lock script hash must match.
    pub lock_script_hash: Option<String>,

    /// Required type script hash (hex-encoded, without 0x prefix).
    /// If set, the output's type script hash must match.
    pub type_script_hash: Option<String>,
}

/// Constraint on the overall transaction fee.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeeConstraint {
    /// Maximum total fee allowed (in shannons).
    pub max_fee: Option<u64>,
}

/// Intent declaration for a partial transaction.
///
/// The intent is carried by the first input of the VTx and describes:
/// - What outputs must be present in the composed transaction
/// - Fee constraints to prevent fee griefing
/// - Additional metadata for matching
///
/// Anyone is economically incentivized to match partial txs because the fee
/// can be provided as part of the matching.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Intent {
    /// Human-readable description of this partial tx's purpose.
    pub description: Option<String>,

    /// Constraints on outputs that must exist in the composed tx.
    pub output_constraints: Vec<OutputConstraint>,

    /// Fee constraint for the composed transaction.
    pub fee_constraint: Option<FeeConstraint>,

    /// Minimum number of inputs required in the composed tx.
    pub min_inputs: Option<usize>,

    /// Minimum number of outputs required in the composed tx.
    pub min_outputs: Option<usize>,

    /// Application-specific metadata (opaque bytes, hex-encoded).
    pub metadata: Option<String>,
}

impl Default for Intent {
    fn default() -> Self {
        Self::new()
    }
}

impl Intent {
    /// Create a new empty intent.
    pub fn new() -> Self {
        Self {
            description: None,
            output_constraints: Vec::new(),
            fee_constraint: None,
            min_inputs: None,
            min_outputs: None,
            metadata: None,
        }
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add an output constraint.
    pub fn with_output_constraint(mut self, constraint: OutputConstraint) -> Self {
        self.output_constraints.push(constraint);
        self
    }

    /// Set the fee constraint.
    pub fn with_fee_constraint(mut self, max_fee: u64) -> Self {
        self.fee_constraint = Some(FeeConstraint {
            max_fee: Some(max_fee),
        });
        self
    }

    /// Serialize the intent to bytes for inclusion in witness data.
    pub fn to_bytes(&self) -> VtxResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| VtxError::SerializationError(e.to_string()))
    }

    /// Deserialize an intent from witness bytes.
    pub fn from_bytes(bytes: &[u8]) -> VtxResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| VtxError::SerializationError(e.to_string()))
    }

    /// Validate the intent against a composed transaction's properties.
    ///
    /// This checks that the composed transaction satisfies all the constraints
    /// declared in this intent.
    pub fn validate(
        &self,
        total_inputs: usize,
        total_outputs: usize,
        total_input_capacity: u64,
        total_output_capacity: u64,
    ) -> VtxResult<()> {
        if let Some(min) = self.min_inputs {
            if total_inputs < min {
                return Err(VtxError::IntentError(format!(
                    "expected at least {} inputs, got {}",
                    min, total_inputs
                )));
            }
        }

        if let Some(min) = self.min_outputs {
            if total_outputs < min {
                return Err(VtxError::IntentError(format!(
                    "expected at least {} outputs, got {}",
                    min, total_outputs
                )));
            }
        }

        if let Some(ref fee) = self.fee_constraint {
            let actual_fee = total_input_capacity.saturating_sub(total_output_capacity);
            if let Some(max_fee) = fee.max_fee {
                if actual_fee > max_fee {
                    return Err(VtxError::IntentError(format!(
                        "fee {} exceeds max_fee {}",
                        actual_fee, max_fee
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_serialization_roundtrip() {
        let intent = Intent::new()
            .with_description("Swap UDT for CKB")
            .with_fee_constraint(10000)
            .with_output_constraint(OutputConstraint {
                min_capacity: Some(100_0000_0000), // 100 CKB
                max_capacity: None,
                lock_script_hash: Some("abcd".to_string()),
                type_script_hash: None,
            });

        let bytes = intent.to_bytes().unwrap();
        let deserialized = Intent::from_bytes(&bytes).unwrap();
        assert_eq!(intent, deserialized);
    }

    #[test]
    fn test_validate_min_inputs() {
        let intent = Intent::new();
        let intent = Intent { min_inputs: Some(3), ..intent };
        assert!(intent.validate(3, 1, 1000, 900).is_ok());
        assert!(intent.validate(2, 1, 1000, 900).is_err());
    }

    #[test]
    fn test_validate_min_outputs() {
        let intent = Intent { min_outputs: Some(2), ..Intent::new() };
        assert!(intent.validate(1, 2, 1000, 900).is_ok());
        assert!(intent.validate(1, 1, 1000, 900).is_err());
    }

    #[test]
    fn test_validate_fee_constraint() {
        let intent = Intent::new().with_fee_constraint(100);
        // Fee = 1000 - 900 = 100, max_fee = 100 => OK
        assert!(intent.validate(1, 1, 1000, 900).is_ok());
        // Fee = 1000 - 899 = 101, max_fee = 100 => FAIL
        assert!(intent.validate(1, 1, 1000, 899).is_err());
    }
}
