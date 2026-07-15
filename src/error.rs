//! Typed errors at the crate boundary (thiserror; no anyhow). Each operation owns a
//! focused error enum: disjointness validation, decoding, encoding, and table
//! identity.

use content_identity::ArchiveError;
use name_table::NameTableError;

use crate::ids::ScopedCoreTypeId;

/// A structural table failed conservative disjointness validation: two accepted
/// decode forms could not be PROVEN structurally distinct, so one might silently
/// shadow the other. Conservative-safe: unprovable disjointness is an error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DisjointnessError {
    #[error(
        "core type {core_type:?}: decode forms {first} and {second} are not provably disjoint ({reason})"
    )]
    NotProvablyDisjoint {
        core_type: ScopedCoreTypeId,
        first: usize,
        second: usize,
        reason: &'static str,
    },
}

/// Decoding a raw block under an expected type failed. A failed decode leaves the
/// NameTable byte-for-byte unchanged (interning atomicity, law 3).
#[derive(Debug, Clone, thiserror::Error)]
pub enum DecodeError {
    #[error("no structural entry for expected type {0:?}")]
    UnknownType(ScopedCoreTypeId),
    #[error("expected {expected} block, found {found}")]
    BlockKindMismatch {
        expected: &'static str,
        found: &'static str,
    },
    #[error("atom case did not match the expected form")]
    CaseMismatch,
    #[error("literal atom did not match the expected interned keyword")]
    LiteralMismatch,
    #[error("delimited sequence held {found} objects, outside the form's bounds")]
    SequenceCardinality { found: u64 },
    #[error("could not flatten the block to a scalar leaf")]
    LeafNotFlattenable,
    #[error("scalar leaf failed to parse: {0}")]
    ScalarParse(String),
    #[error("transparent delegation cycle through type {0:?}")]
    DelegationCycle(ScopedCoreTypeId),
    #[error("product form arity {form} did not match the {blocks} sibling blocks")]
    ProductArity { form: usize, blocks: usize },
    #[error("no accepted decode form matched under expected type {core_type:?}")]
    NoAlternative { core_type: ScopedCoreTypeId },
    #[error(transparent)]
    Names(#[from] NameTableError),
}

/// Encoding a structural value to a raw block failed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncodeError {
    #[error("no structural entry for expected type {0:?}")]
    UnknownType(ScopedCoreTypeId),
    #[error("value chose constructor {chosen}, but the entry has {available} constructors")]
    ConstructorOutOfRange { chosen: u32, available: usize },
    #[error("value shape did not fit the canonical encode form: {0}")]
    ShapeMismatch(&'static str),
    #[error(transparent)]
    Names(#[from] NameTableError),
}

/// Computing a table's content identity failed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TableError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
}
