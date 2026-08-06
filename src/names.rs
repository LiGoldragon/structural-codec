//! Typed, read-only name inputs to structural evaluation.
//!
//! The translator is the sole allocator. This crate accepts already-issued
//! declaration assignments and already-resolved references; it cannot turn a
//! spelling into a new encoded name.

use name_table::{EncodedName, TextualName};
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
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
pub struct DeclarationAssignment<Language> {
    encoded_name: EncodedName,
    marker: std::marker::PhantomData<Language>,
}

impl<Language> DeclarationAssignment<Language> {
    pub const fn new(encoded_name: EncodedName) -> Self {
        Self {
            encoded_name,
            marker: std::marker::PhantomData,
        }
    }

    pub const fn encoded_name(&self) -> &EncodedName {
        &self.encoded_name
    }

    pub const fn into_encoded_name(self) -> EncodedName {
        self.encoded_name
    }
}

impl<Language> Clone for DeclarationAssignment<Language> {
    fn clone(&self) -> Self {
        Self::new(self.encoded_name)
    }
}

/// An identity returned by lookup-only reference resolution.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
pub struct ResolvedReference<Language> {
    encoded_name: EncodedName,
    marker: std::marker::PhantomData<Language>,
}

impl<Language> ResolvedReference<Language> {
    pub const fn new(encoded_name: EncodedName) -> Self {
        Self {
            encoded_name,
            marker: std::marker::PhantomData,
        }
    }

    pub const fn encoded_name(&self) -> &EncodedName {
        &self.encoded_name
    }

    pub const fn into_encoded_name(self) -> EncodedName {
        self.encoded_name
    }
}

impl<Language> Clone for ResolvedReference<Language> {
    fn clone(&self) -> Self {
        Self::new(self.encoded_name)
    }
}

/// Read-only spelling projection for already-issued encoded names.
///
/// A consumer may back this with any verified current or historical snapshot.
/// Capsule pin composition is intentionally outside this interface.
pub trait EncodedNameResolver<Root> {
    fn resolve(&self, encoded_name: &EncodedName) -> Option<&TextualName>;
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
