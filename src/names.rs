//! Typed, read-only name inputs to structural evaluation.
//!
//! The translator is the sole allocator. This crate accepts already-issued
//! declaration assignments and already-resolved references; it cannot turn a
//! spelling into a new encoded ID.

use name_table::{EncodedId, Name};
use raw_discovery::SourceBound;

/// One name occurrence in the source currently being evaluated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NameOccurrence<'source> {
    spelling: &'source str,
    bound: SourceBound,
}

impl<'source> NameOccurrence<'source> {
    pub fn new(spelling: &'source str, bound: SourceBound) -> Self {
        Self { spelling, bound }
    }

    pub fn spelling(self) -> &'source str {
        self.spelling
    }

    pub fn bound(self) -> SourceBound {
        self.bound
    }
}

/// A translator-issued identity assigned to a declaration occurrence.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct DeclarationAssignment<Root>(EncodedId<Root>);

impl<Root> DeclarationAssignment<Root> {
    pub fn new(encoded_id: EncodedId<Root>) -> Self {
        Self(encoded_id)
    }

    pub fn encoded_id(&self) -> &EncodedId<Root> {
        &self.0
    }

    pub fn into_encoded_id(self) -> EncodedId<Root> {
        self.0
    }
}

/// An identity returned by lookup-only reference resolution.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct ResolvedReference<Root>(EncodedId<Root>);

impl<Root> ResolvedReference<Root> {
    pub fn new(encoded_id: EncodedId<Root>) -> Self {
        Self(encoded_id)
    }

    pub fn encoded_id(&self) -> &EncodedId<Root> {
        &self.0
    }

    pub fn into_encoded_id(self) -> EncodedId<Root> {
        self.0
    }
}

/// Read-only spelling projection for already-issued encoded IDs.
///
/// A consumer may back this with any verified current or historical snapshot.
/// Capsule pin composition is intentionally outside this interface.
pub trait EncodedNameResolver<Root> {
    fn resolve(&self, encoded_id: &EncodedId<Root>) -> Option<&Name>;
}

/// Typed inputs used while decoding declaration and reference positions.
///
/// The two methods cannot be substituted for one another. Both receive the
/// exact source bound so a caller can distinguish equal spellings in different
/// modules without this crate inventing module or Capsule context.
pub trait DecodeNameBindings<Root>: EncodedNameResolver<Root> {
    fn declaration_assignment(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Option<DeclarationAssignment<Root>>;

    fn reference_resolution(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Option<ResolvedReference<Root>>;
}
