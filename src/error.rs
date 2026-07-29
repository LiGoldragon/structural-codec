//! Typed failures at the structural-codec boundary.

use content_identity::ArchiveError;
use raw_discovery::{
    BlockDiscoveryError, BoundaryDiscoveryContextIdentifier, RecognizeError, SourceBound,
    TokenProfileError, TriggerIdentifier,
};

use crate::form::DelegationPayload;
use crate::ids::{EncodedConstructorId, EncodedTypeId, StableRoleId};

#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthoringError {
    #[error("field role zero is reserved")]
    ZeroRoleIdentity,
    #[error("field role {role:?} is present more than once")]
    DuplicateRoleIdentity { role: StableRoleId },
}

#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, thiserror::Error)]
pub enum DisjointnessError<Root> {
    #[error(
        "encoded type {core_type:?}: accepted forms under {first:?} and {second:?} are not provably disjoint ({reason})"
    )]
    NotProvablyDisjoint {
        core_type: EncodedTypeId<Root>,
        first: EncodedConstructorId<Root>,
        second: EncodedConstructorId<Root>,
        reason: DisjointnessReason<Root>,
    },
    #[error("encoded type {core_type:?}: delegate proof re-entered {reentered:?}")]
    DelegateExpansionCycle {
        core_type: EncodedTypeId<Root>,
        reentered: EncodedTypeId<Root>,
    },
}

#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, thiserror::Error)]
pub enum DisjointnessReason<Root> {
    #[error("delegate target {target:?} has no table entry")]
    MissingDelegateTarget { target: EncodedTypeId<Root> },
    #[error("a leaf or repeated form has no pinned outer kind")]
    OpaqueForm,
    #[error("both forms accept an overlapping atom case")]
    OverlappingAtomCase,
    #[error("both forms require the same literal")]
    SameLiteral,
    #[error("a literal may satisfy the named position")]
    LiteralMayMatchNameAtom,
    #[error("neither application position is provably disjoint")]
    ApplicationPositionsNotDisjoint,
    #[error("both forms activate the same boundary")]
    SharedBoundary,
    #[error("a role link {role:?} is absent from the typed record")]
    MissingRole { role: StableRoleId },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DecodeError<Root> {
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
    #[error(transparent)]
    BlockDiscovery(#[from] BlockDiscoveryError),
    #[error(transparent)]
    Recognition(#[from] RecognizeError),
    #[error("no structural entry for expected type {0:?}")]
    UnknownType(EncodedTypeId<Root>),
    #[error("expected {expected} block, found {found}")]
    BlockKindMismatch {
        expected: &'static str,
        found: &'static str,
    },
    #[error("atom case did not match the expected form")]
    CaseMismatch,
    #[error("literal atom did not match the expected encoded vocabulary word")]
    LiteralMismatch,
    #[error("no translator-issued assignment was supplied for declaration {bound:?}")]
    MissingDeclarationAssignment { bound: SourceBound },
    #[error("a lookup-only reference did not resolve at {bound:?}")]
    UnresolvedReference { bound: SourceBound },
    #[error("the supplied encoded ID resolves to a different spelling at {bound:?}")]
    NameBindingMismatch { bound: SourceBound },
    #[error("the resolved encoded identity is reserved at {bound:?}")]
    ExcludedNameIdentity { bound: SourceBound },
    #[error("the encoded name has no spelling in the supplied resolver")]
    UnknownEncodedName {
        encoded_id: name_table::EncodedId<Root>,
    },
    #[error("ordered product expected {expected} sibling blocks, found {found}")]
    ProductArityMismatch { expected: usize, found: usize },
    #[error(
        "ordered product position {position} for role {role:?} did not match source block {bound:?}"
    )]
    ProductPositionMismatch {
        position: usize,
        role: StableRoleId,
        bound: SourceBound,
    },
    #[error("delegated position did not satisfy {payload:?}")]
    DelegationPayloadMismatch { payload: DelegationPayload },
    #[error("repeated position held {found} objects outside its declared bounds")]
    RepetitionCardinality { found: u64 },
    #[error("could not flatten the block to a scalar leaf")]
    LeafNotFlattenable,
    #[error("scalar leaf failed to parse: {0}")]
    ScalarParse(String),
    #[error("transparent delegation cycle through type {0:?}")]
    DelegationCycle(EncodedTypeId<Root>),
    #[error("typed role {role:?} is absent from this record")]
    MissingRole { role: StableRoleId },
    #[error("the profile boundary {boundary:?} does not match this raw block")]
    BoundaryMismatch { boundary: TriggerIdentifier },
    #[error("no accepted decode form matched under expected type {core_type:?}")]
    NoAlternative { core_type: EncodedTypeId<Root> },
    #[error("no branch of a descriptor alternation matched")]
    NoDescriptorAlternative,
    #[error(
        "ordered-sequence repetition at role {role:?} has no count that satisfies its typed tail"
    )]
    SequenceRepetitionBoundary {
        role: StableRoleId,
        refusal: Box<DecodeError<Root>>,
    },
    #[error("source did not contain exactly one root object")]
    RootObjectCount,
}

impl<Root> DecodeError<Root> {
    pub(crate) fn is_structural_non_match(&self) -> bool {
        if let Self::SequenceRepetitionBoundary { refusal, .. } = self {
            return refusal.is_structural_non_match();
        }
        matches!(
            self,
            Self::BlockKindMismatch { .. }
                | Self::CaseMismatch
                | Self::LiteralMismatch
                | Self::ExcludedNameIdentity { .. }
                | Self::DelegationPayloadMismatch { .. }
                | Self::RepetitionCardinality { .. }
                | Self::LeafNotFlattenable
                | Self::ScalarParse(_)
                | Self::BoundaryMismatch { .. }
                | Self::ProductArityMismatch { .. }
                | Self::ProductPositionMismatch { .. }
                | Self::NoAlternative { .. }
                | Self::NoDescriptorAlternative
        )
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum EncodeError<Root> {
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
    #[error("no structural entry for expected type {0:?}")]
    UnknownType(EncodedTypeId<Root>),
    #[error("value selected constructor {chosen:?}, which is not in this entry")]
    UnknownConstructor { chosen: EncodedConstructorId<Root> },
    #[error("value shape did not fit the canonical descriptor")]
    ShapeMismatch,
    #[error("the encoded atom did not match the canonical literal")]
    LiteralMismatch,
    #[error("the encoded identity is reserved at this name position")]
    ExcludedNameIdentity,
    #[error("the encoded name has no spelling in the supplied resolver")]
    UnknownEncodedName {
        encoded_id: name_table::EncodedId<Root>,
    },
    #[error("delegated position did not satisfy {payload:?}")]
    DelegationPayloadMismatch { payload: DelegationPayload },
    #[error("repeated position held {found} objects outside its declared bounds")]
    RepetitionCardinality { found: u64 },
    #[error("typed role {role:?} is absent from this record or mirror")]
    MissingRole { role: StableRoleId },
    #[error("a value spelling would not decode canonically")]
    NonCanonicalSpelling,
    #[error("no branch of a descriptor alternation accepted the value")]
    NoDescriptorAlternative,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum TableError<Root> {
    #[error(transparent)]
    Disjointness(#[from] DisjointnessError<Root>),
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("the structural table and supplied token profile have different identities")]
    TokenProfileIdentityMismatch,
    #[error("constructor {constructor:?} does not belong under entry {entry:?}")]
    ConstructorUnderWrongEntry {
        constructor: EncodedConstructorId<Root>,
        entry: EncodedTypeId<Root>,
    },
    #[error("the table contains the same encoded type more than once")]
    DuplicateEncodedType { entry: EncodedTypeId<Root> },
    #[error("a descriptor alternation must contain at least one branch")]
    EmptyAlternation,
    #[error("entry {entry:?} has no constructors")]
    EmptyEntry { entry: EncodedTypeId<Root> },
    #[error("entry {entry:?} repeats constructor identity {constructor:?}")]
    DuplicateConstructor {
        entry: EncodedTypeId<Root>,
        constructor: EncodedConstructorId<Root>,
    },
    #[error("constructor {constructor:?} repeats accepted decode-form identity {form:?}")]
    DuplicateDecodeForm {
        constructor: EncodedConstructorId<Root>,
        form: crate::ids::DecodeFormId,
    },
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
    #[error("ordered product repeats member role {role:?}")]
    DuplicateProductRole { role: StableRoleId },
    #[error("ordered product member role {role:?} is not a delegated expected type")]
    ProductMemberNotDelegate { role: StableRoleId },
    #[error("ordered sequence repeats member role {role:?}")]
    DuplicateSequenceRole { role: StableRoleId },
    #[error("trigger {identifier:?} does not provide the required role")]
    WrongTriggerKind { identifier: TriggerIdentifier },
    #[error(
        "boundary {boundary:?} is used by a structural form but absent from the table discovery rules"
    )]
    UnconfiguredDiscoveryBoundary { boundary: TriggerIdentifier },
    #[error("textual rendering policy does not name exactly the table discovery contexts")]
    TextualPolicyContextMismatch,
    #[error("textual policy trigger {identifier:?} is inactive in context {context:?}")]
    InactiveTextualPolicyTrigger {
        context: BoundaryDiscoveryContextIdentifier,
        identifier: TriggerIdentifier,
    },
    #[error(
        "textual policy trigger {identifier:?} is not a {required} trigger in context {context:?}"
    )]
    WrongTextualPolicyTrigger {
        context: BoundaryDiscoveryContextIdentifier,
        identifier: TriggerIdentifier,
        required: &'static str,
    },
    #[error(transparent)]
    BlockDiscovery(#[from] BlockDiscoveryError),
    #[error(transparent)]
    TokenProfile(#[from] TokenProfileError),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("the textual form carried {count} chunks; un-view requires exactly one")]
pub struct SingleChunkRequired {
    pub count: usize,
}
