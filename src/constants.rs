//! Constants for CKB Virtual Tx.

/// CKB syscall numbers (matching ckb-std/ckb_constants.rs).
pub mod syscall_numbers {
    pub const SYS_EXIT: u64 = 93;
    pub const SYS_LOAD_TRANSACTION: u64 = 2051;
    pub const SYS_LOAD_SCRIPT: u64 = 2052;
    pub const SYS_LOAD_TX_HASH: u64 = 2061;
    pub const SYS_LOAD_SCRIPT_HASH: u64 = 2062;
    pub const SYS_LOAD_CELL: u64 = 2071;
    pub const SYS_LOAD_HEADER: u64 = 2072;
    pub const SYS_LOAD_INPUT: u64 = 2073;
    pub const SYS_LOAD_WITNESS: u64 = 2074;
    pub const SYS_LOAD_CELL_BY_FIELD: u64 = 2081;
    pub const SYS_LOAD_HEADER_BY_FIELD: u64 = 2082;
    pub const SYS_LOAD_INPUT_BY_FIELD: u64 = 2083;
    pub const SYS_LOAD_CELL_DATA: u64 = 2092;
    pub const SYS_DEBUG: u64 = 2177;
}

/// CKB source types for syscall queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum Source {
    Input = 1,
    Output = 2,
    CellDep = 3,
    HeaderDep = 4,
    GroupInput = 0x0100_0000_0000_0001,
    GroupOutput = 0x0100_0000_0000_0002,
}

impl Source {
    pub fn from_u64(value: u64) -> Option<Self> {
        match value {
            1 => Some(Source::Input),
            2 => Some(Source::Output),
            3 => Some(Source::CellDep),
            4 => Some(Source::HeaderDep),
            0x0100_0000_0000_0001 => Some(Source::GroupInput),
            0x0100_0000_0000_0002 => Some(Source::GroupOutput),
            _ => None,
        }
    }

    /// Returns true if this is a group source (GroupInput/GroupOutput).
    pub fn is_group(&self) -> bool {
        matches!(self, Source::GroupInput | Source::GroupOutput)
    }

    /// Returns the base source for a group source.
    pub fn base_source(&self) -> Source {
        match self {
            Source::GroupInput => Source::Input,
            Source::GroupOutput => Source::Output,
            other => *other,
        }
    }
}

/// CKB cell fields for load_cell_by_field syscall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum CellField {
    Capacity = 0,
    DataHash = 1,
    Lock = 2,
    LockHash = 3,
    Type = 4,
    TypeHash = 5,
    OccupiedCapacity = 6,
}

impl CellField {
    pub fn from_u64(value: u64) -> Option<Self> {
        match value {
            0 => Some(CellField::Capacity),
            1 => Some(CellField::DataHash),
            2 => Some(CellField::Lock),
            3 => Some(CellField::LockHash),
            4 => Some(CellField::Type),
            5 => Some(CellField::TypeHash),
            6 => Some(CellField::OccupiedCapacity),
            _ => None,
        }
    }
}

/// CKB input fields for load_input_by_field syscall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum InputField {
    OutPoint = 0,
    Since = 1,
}

impl InputField {
    pub fn from_u64(value: u64) -> Option<Self> {
        match value {
            0 => Some(InputField::OutPoint),
            1 => Some(InputField::Since),
            _ => None,
        }
    }
}

/// CKB header fields for load_header_by_field syscall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum HeaderField {
    EpochNumber = 0,
    EpochStartBlockNumber = 1,
    EpochLength = 2,
}

impl HeaderField {
    pub fn from_u64(value: u64) -> Option<Self> {
        match value {
            0 => Some(HeaderField::EpochNumber),
            1 => Some(HeaderField::EpochStartBlockNumber),
            2 => Some(HeaderField::EpochLength),
            _ => None,
        }
    }
}

/// CKB standard return codes.
pub const CKB_SUCCESS: i32 = 0;
pub const CKB_INDEX_OUT_OF_BOUND: i32 = 1;
pub const CKB_ITEM_MISSING: i32 = 2;

/// Marker code hash prefix for VTx indicator inputs.
/// The first input with this code hash in its type script identifies the tx as a VTx.
pub const VTX_INDICATOR_CODE_HASH: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x76, 0x74, 0x78, 0x00, // "vtx\0" at bytes 16..20
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];
