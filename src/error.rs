//! Typed errors at the crate boundary (thiserror; no anyhow). Each operation owns a
//! focused error enum: disjointness validation, decoding, encoding, and table
//! identity.

use content_identity::ArchiveError;
use name_table::NameTableError;

use crate::form::DelegationPayload;
use crate::ids::ScopedEncodedTypeId;
use raw_discovery::{TokenProfileError, TriggerIdentifier};

/// A structural table failed conservative disjointness validation: two accepted
/// decode forms could not be PROVEN structurally distinct, so one might silently
/// shadow the other. Conservative-safe: unprovable disjointness is an error.
///
/// This error is archiveable so callers can retain a typed seal refusal.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, thiserror::Error)]
pub enum DisjointnessError {
    #[error(
        "core type {core_type:?}: decode forms {first} and {second} are not provably disjoint ({reason})"
    )]
    NotProvablyDisjoint {
        core_type: ScopedEncodedTypeId,
        first: usize,
        second: usize,
        reason: DisjointnessReason,
    },
    #[error(
        "core type {core_type:?}: decode forms {first} and {second} contain an unresolved delegate expansion cycle through {reentered:?}"
    )]
    DelegateExpansionCycle {
        core_type: ScopedEncodedTypeId,
        first: usize,
        second: usize,
        reentered: ScopedEncodedTypeId,
    },
}

/// The typed reason a pair of forms could not be proven disjoint.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, thiserror::Error)]
pub enum DisjointnessReason {
    #[error("delegate target {target:?} has no table entry available for proof")]
    MissingDelegateTarget { target: ScopedEncodedTypeId },
    #[error("a leaf form has no pinned block kind")]
    OpaqueForm,
    #[error("both forms accept an overlapping atom case")]
    OverlappingAtomCase,
    #[error("both forms require the same interned literal")]
    SameLiteral,
    #[error("a literal atom might satisfy the name atom's case constraint")]
    LiteralMayMatchNameAtom,
    #[error("neither the application head nor payload is provably disjoint")]
    ApplicationPositionsNotDisjoint,
    #[error("both forms use the same delimiter")]
    SharedDelimiter,
}

/// Decoding source text or a structural block under an expected type failed. A
/// failed decode leaves the NameTable byte-for-byte unchanged (interning
/// atomicity, law 3).
#[derive(Debug, Clone, thiserror::Error)]
pub enum DecodeError {
    #[error("direct textual decoding requires a sealed token profile")]
    MissingTokenProfile,
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
    #[error("no structural entry for expected type {0:?}")]
    UnknownType(ScopedEncodedTypeId),
    #[error("expected {expected} block, found {found}")]
    BlockKindMismatch {
        expected: &'static str,
        found: &'static str,
    },
    #[error("atom case did not match the expected form")]
    CaseMismatch,
    #[error("literal decoding requires a configured lexicon")]
    MissingLexicon,
    #[error("literal atom did not match the expected interned keyword")]
    LiteralMismatch,
    #[error("the delegated position did not satisfy its typed direction {payload:?}")]
    DelegationPayloadMismatch { payload: DelegationPayload },
    #[error("delimited sequence held {found} objects, outside the form's bounds")]
    SequenceCardinality { found: u64 },
    #[error("could not flatten the block to a scalar leaf")]
    LeafNotFlattenable,
    #[error("scalar leaf failed to parse: {0}")]
    ScalarParse(String),
    #[error("transparent delegation cycle through type {0:?}")]
    DelegationCycle(ScopedEncodedTypeId),
    #[error("product form arity {form} did not match the {blocks} sibling blocks")]
    ProductArity { form: usize, blocks: usize },
    #[error("no accepted decode form matched under expected type {core_type:?}")]
    NoAlternative { core_type: ScopedEncodedTypeId },
    #[error("expected text at byte {byte_offset}")]
    ExpectedText { byte_offset: usize },
    #[error("expected trigger {expected:?}, found {found:?}")]
    ExpectedTrigger {
        expected: TriggerIdentifier,
        found: Option<TriggerIdentifier>,
    },
    #[error("trigger {identifier:?} did not provide the required {expected} role")]
    ExpectedTriggerRole {
        identifier: TriggerIdentifier,
        expected: &'static str,
    },
    #[error("text remained after the expected root form at byte {byte_offset}")]
    TrailingText { byte_offset: usize },
    #[error(
        "child decoding failed inside boundary {boundary:?} interior [{start}, {end}): {source}"
    )]
    BoundedInterior {
        boundary: TriggerIdentifier,
        start: usize,
        end: usize,
        #[source]
        source: Box<DecodeError>,
    },
    #[error("a repeated textual form succeeded without advancing the reader")]
    TextReaderDidNotAdvance,
    #[error(transparent)]
    Names(#[from] NameTableError),
}

/// Encoding a structural value to canonical text or a structural block failed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncodeError {
    #[error("direct textual encoding requires a sealed token profile")]
    MissingTokenProfile,
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
    #[error("no structural entry for expected type {0:?}")]
    UnknownType(ScopedEncodedTypeId),
    #[error("value chose constructor {chosen}, but the entry has {available} constructors")]
    ConstructorOutOfRange { chosen: u32, available: usize },
    #[error("value shape did not fit the canonical encode form: {0}")]
    ShapeMismatch(&'static str),
    #[error("the encoded atom did not match the canonical literal")]
    LiteralMismatch,
    #[error("the encoded delegated position did not satisfy its typed direction {payload:?}")]
    DelegationPayloadMismatch { payload: DelegationPayload },
    #[error("trigger {identifier:?} did not provide the required {expected} role")]
    ExpectedTriggerRole {
        identifier: TriggerIdentifier,
        expected: &'static str,
    },
    #[error("a value spelling would not decode canonically at its structural position")]
    NonCanonicalSpelling,
    #[error("the table has no canonical separator trigger")]
    MissingCanonicalSeparator,
    #[error(transparent)]
    Names(#[from] NameTableError),
}

/// Computing a table's content identity failed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TableError {
    #[error(transparent)]
    Disjointness(#[from] DisjointnessError),
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error("trigger {identifier:?} does not provide the required {expected} role")]
    WrongTriggerKind {
        identifier: TriggerIdentifier,
        expected: &'static str,
    },
    #[error("delegate target {target:?} is missing while validating lexical positions")]
    MissingLexicalDelegateTarget { target: ScopedEncodedTypeId },
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
}

/// A [`TextualForm`](crate::TextualForm) value did not carry the single text chunk the
/// provided un-view path requires. The trivial single-document case is one chunk; the
/// multi-chunk (filename→text) index is a deferred packaging future, so an un-view of a
/// zero- or many-chunk view is a loud, typed error rather than a silent pick.
#[derive(Debug, Clone, thiserror::Error)]
#[error("the textual form carried {count} chunks; un-view requires exactly one")]
pub struct SingleChunkRequired {
    /// How many chunks the view actually carried.
    pub count: usize,
}
