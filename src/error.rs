//! Typed failures at the structural-codec boundary.

use content_identity::ArchiveError;
use name_table::NameTableError;
use raw_discovery::{RecognizeError, TokenProfileError, TriggerIdentifier};

use crate::form::DelegationPayload;
use crate::ids::{EncodedConstructorId, ScopedEncodedTypeId, StableRoleId};

/// A checked authoring operation refused to construct an invalid archived
/// record. Semantic consistency is then enforced when the table seals.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthoringError {
    #[error("field role zero is reserved")]
    ZeroRoleIdentity,
    #[error("field role {role:?} is present more than once")]
    DuplicateRoleIdentity { role: StableRoleId },
}

#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, thiserror::Error)]
pub enum DisjointnessError {
    #[error(
        "encoded type {core_type:?}: accepted forms under {first:?} and {second:?} are not provably disjoint ({reason})"
    )]
    NotProvablyDisjoint {
        core_type: ScopedEncodedTypeId,
        first: EncodedConstructorId,
        second: EncodedConstructorId,
        reason: DisjointnessReason,
    },
    #[error("encoded type {core_type:?}: delegate proof re-entered {reentered:?}")]
    DelegateExpansionCycle {
        core_type: ScopedEncodedTypeId,
        reentered: ScopedEncodedTypeId,
    },
}

#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, thiserror::Error)]
pub enum DisjointnessReason {
    #[error("delegate target {target:?} has no table entry")]
    MissingDelegateTarget { target: ScopedEncodedTypeId },
    #[error("a leaf or repeated form has no pinned outer kind")]
    OpaqueForm,
    #[error("both forms accept an overlapping atom case")]
    OverlappingAtomCase,
    #[error("both forms require the same literal")]
    SameLiteral,
    #[error("a literal may satisfy the atom form")]
    LiteralMayMatchNameAtom,
    #[error("neither application position is provably disjoint")]
    ApplicationPositionsNotDisjoint,
    #[error("both forms activate the same boundary")]
    SharedBoundary,
    #[error("a role link {role:?} is absent from the typed record")]
    MissingRole { role: StableRoleId },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DecodeError {
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
    #[error(transparent)]
    Recognition(#[from] RecognizeError),
    #[error("no structural entry for expected type {0:?}")]
    UnknownType(ScopedEncodedTypeId),
    #[error("expected {expected} block, found {found}")]
    BlockKindMismatch {
        expected: &'static str,
        found: &'static str,
    },
    #[error("atom case did not match the expected form")]
    CaseMismatch,
    /// EC18: absent literal data is materially distinct from a present but
    /// non-matching literal.
    #[error("literal decoding requires a configured lexicon")]
    MissingLexicon,
    #[error("literal atom did not match the expected interned keyword")]
    LiteralMismatch,
    #[error("delegated position did not satisfy {payload:?}")]
    DelegationPayloadMismatch { payload: DelegationPayload },
    #[error("repeated position held {found} objects outside its declared bounds")]
    RepetitionCardinality { found: u64 },
    #[error("could not flatten the block to a scalar leaf")]
    LeafNotFlattenable,
    #[error("scalar leaf failed to parse: {0}")]
    ScalarParse(String),
    #[error("transparent delegation cycle through type {0:?}")]
    DelegationCycle(ScopedEncodedTypeId),
    #[error("typed role {role:?} is absent from this record")]
    MissingRole { role: StableRoleId },
    #[error("the profile boundary {boundary:?} does not match this raw block")]
    BoundaryMismatch { boundary: TriggerIdentifier },
    #[error("no accepted decode form matched under expected type {core_type:?}")]
    NoAlternative { core_type: ScopedEncodedTypeId },
    #[error("source did not contain exactly one root object")]
    RootObjectCount,
    #[error(transparent)]
    Names(#[from] NameTableError),
}

impl DecodeError {
    /// Whether a failed alternative merely did not structurally match. Only
    /// these failures may advance alternative evaluation; table/profile/name
    /// failures retain their exact cause.
    pub(crate) fn is_structural_non_match(&self) -> bool {
        matches!(
            self,
            Self::BlockKindMismatch { .. }
                | Self::CaseMismatch
                | Self::LiteralMismatch
                | Self::DelegationPayloadMismatch { .. }
                | Self::RepetitionCardinality { .. }
                | Self::LeafNotFlattenable
                | Self::ScalarParse(_)
                | Self::BoundaryMismatch { .. }
                | Self::NoAlternative { .. }
        )
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum EncodeError {
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
    #[error("no structural entry for expected type {0:?}")]
    UnknownType(ScopedEncodedTypeId),
    #[error("value selected constructor {chosen:?}, which is not in this entry")]
    UnknownConstructor { chosen: EncodedConstructorId },
    #[error("value shape did not fit the canonical descriptor")]
    ShapeMismatch,
    #[error("the encoded atom did not match the canonical literal")]
    LiteralMismatch,
    #[error("delegated position did not satisfy {payload:?}")]
    DelegationPayloadMismatch { payload: DelegationPayload },
    #[error("typed role {role:?} is absent from this record or mirror")]
    MissingRole { role: StableRoleId },
    #[error("a value spelling would not decode canonically")]
    NonCanonicalSpelling,
    #[error(transparent)]
    Names(#[from] NameTableError),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum TableError {
    #[error(transparent)]
    Disjointness(#[from] DisjointnessError),
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error("table language {table:?} conflicts with encoded type {encoded:?}")]
    LanguageMismatch {
        table: crate::ids::EncodedLanguage,
        encoded: ScopedEncodedTypeId,
    },
    #[error("constructor {constructor:?} does not belong under entry {entry:?}")]
    ConstructorUnderWrongEntry {
        constructor: EncodedConstructorId,
        entry: ScopedEncodedTypeId,
    },
    #[error("entry key {key:?} does not equal its archived type {entry:?}")]
    EntryKeyMismatch {
        entry: ScopedEncodedTypeId,
        key: ScopedEncodedTypeId,
    },
    #[error("entry {entry:?} has no constructors")]
    EmptyEntry { entry: ScopedEncodedTypeId },
    #[error("entry {entry:?} repeats constructor identity {constructor:?}")]
    DuplicateConstructor {
        entry: ScopedEncodedTypeId,
        constructor: EncodedConstructorId,
    },
    #[error("constructor {constructor:?} repeats accepted decode-form identity {form:?}")]
    DuplicateDecodeForm {
        constructor: EncodedConstructorId,
        form: crate::ids::DecodeFormId,
    },
    #[error("a reserved fixture Schema id appeared in a non-fixture vocabulary")]
    ReservedFixtureIdInLanguageTable,
    #[error("a fixture vocabulary included a non-reserved encoded type")]
    FixtureVocabularyHasProductionId,
    #[error("a typed record repeats role {role:?}")]
    DuplicateRole { role: StableRoleId },
    #[error("an archived role identity was zero")]
    ZeroRoleIdentity,
    #[error(
        "an archived role identity {actual:?} does not match its typed field role {expected:?}"
    )]
    RoleIdentityMismatch {
        expected: StableRoleId,
        actual: StableRoleId,
    },
    #[error("descriptor refers to missing role {role:?}")]
    MissingRole { role: StableRoleId },
    #[error("trigger {identifier:?} does not provide the required role")]
    WrongTriggerKind { identifier: TriggerIdentifier },
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("the textual form carried {count} chunks; un-view requires exactly one")]
pub struct SingleChunkRequired {
    pub count: usize,
}
