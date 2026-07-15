//! The addressed structural table: the external sidecar keyed by `ScopedCoreTypeId`.
//! Its content identity is computed over `TableIdentityPayload` and STORED OUTSIDE
//! that payload (fixing the self-reference bug), and is EXCLUDED from Core value
//! identity by construction — Core hashing never sees the table. Old table decodes
//! old text, a new table encodes new text, and both reach the same Core value (§4.6).

use std::collections::BTreeMap;

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};

use crate::codec::StructuralEntry;
use crate::error::{DisjointnessError, TableError};
use crate::ids::{CoreUniverseId, ScopedCoreTypeId, StructuralRevision};

/// The identity of a Core layout the forms target (supplied by the Core side).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct CoreLayoutIdentity(pub [u8; 32]);

/// The identity of a raw profile (glyph set + revision).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct RawProfileIdentity(pub [u8; 32]);

/// The identity of a leaf codec's contract.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct LeafCodecContractId(pub u32);

/// The table-identity pre-image. The resulting hash is stored on
/// `AddressedStructuralTable`, NEVER inside here.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TableIdentityPayload {
    pub core_universe: CoreUniverseId,
    pub core_layout_identity: CoreLayoutIdentity,
    pub raw_profile_identity: RawProfileIdentity,
    /// The committed lexicon — the EXACT glyph bytes this table's text uses.
    pub committed_lexicon: Vec<u8>,
    pub leaf_codec_contracts: Vec<LeafCodecContractId>,
    pub entries: BTreeMap<ScopedCoreTypeId, StructuralEntry>,
}

/// The hash domain for structural tables, layout-version tagged.
pub struct StructuralTableDomain;

impl HashDomain for StructuralTableDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 addressed structural table",
            layout: LayoutVersion::new(1),
        }
    }
}

/// A revisioned structural table with its identity stored outside the hashed payload.
#[derive(Clone, Debug)]
pub struct AddressedStructuralTable {
    revision: StructuralRevision,
    payload: TableIdentityPayload,
    identity: ContentHash<StructuralTableDomain>,
}

impl AddressedStructuralTable {
    /// Compute the table's content identity over the payload and store it outside.
    pub fn seal(
        revision: StructuralRevision,
        payload: TableIdentityPayload,
    ) -> Result<Self, TableError> {
        let identity = ContentHash::of_core(&payload)?;
        Ok(Self {
            revision,
            payload,
            identity,
        })
    }

    pub fn revision(&self) -> StructuralRevision {
        self.revision
    }

    /// The table's content identity — co-versioned with the language package,
    /// EXCLUDED from Core value identity.
    pub fn identity(&self) -> ContentHash<StructuralTableDomain> {
        self.identity
    }

    /// Queried BY expected type, never globally searched; the input never selects its
    /// own type.
    pub fn entry(&self, expected: ScopedCoreTypeId) -> Option<&StructuralEntry> {
        self.payload.entries.get(&expected)
    }

    /// Validate conservative disjointness across every entry.
    pub fn validate_disjoint(&self) -> Result<(), DisjointnessError> {
        for entry in self.payload.entries.values() {
            entry.validate_disjoint()?;
        }
        Ok(())
    }
}
