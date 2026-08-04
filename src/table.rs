//! Sealed, vocabulary-identified tables of archived typed rule records.
//!
//! A downstream vocabulary supplies its root enum and complete encoded-ID
//! chains, authors real typed records, and seals those records as one table.
//! The shared evaluator and prover operate only on `StructureRecord` field
//! data; records carry no language-specific grammar algorithm.

use std::collections::BTreeMap;

use content_identity::{ArchiveError, ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use raw_discovery::{
    BlockTreeDiscoveryConfiguration, BoundaryDiscoveryContextIdentifier,
    SealedBlockTreeDiscoveryConfiguration, SealedTokenProfile, TokenProfileDomain, Trigger,
    TriggerIdentifier,
};

use crate::codec::StructuralEntry;
use crate::error::TableError;
use crate::form::{
    BorrowedFieldView, FieldVisitor, SharedDescriptor, StructuralRule, StructureRecord,
};
use crate::ids::{EncodedTypeId, StableRoleId};

/// The target encoded-layout identity is a domain-typed content address. There
/// is no raw byte wrapper, default value, or zero placeholder.
pub struct TargetLayoutDomain;
impl HashDomain for TargetLayoutDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 target encoded layout",
            layout: LayoutVersion::new(1),
        }
    }
}
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetLayoutIdentity(ContentHash<TargetLayoutDomain>);

impl TargetLayoutIdentity {
    /// Derive the identity from canonical target-layout data. Raw digest bytes
    /// have no constructor at this boundary, so zero/default placeholders are
    /// unrepresentable.
    pub fn derive(layout_data: &[u8]) -> Self {
        Self(ContentHash::derive(layout_data))
    }
}

/// Domain for production structuretree vocabularies.
pub struct StructuralVocabularyDomain;
impl HashDomain for StructuralVocabularyDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 structuretree vocabulary",
            layout: LayoutVersion::new(1),
        }
    }
}

/// A separate optional domain for explicitly test-only sidecars.
pub struct FixtureVocabularyDomain;
impl HashDomain for FixtureVocabularyDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 test vocabulary",
            layout: LayoutVersion::new(1),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralVocabularyIdentity {
    Language(ContentHash<StructuralVocabularyDomain>),
    Fixture(ContentHash<FixtureVocabularyDomain>),
}

/// Canonical source spellings selected explicitly for one discovery context.
///
/// Trigger-set order has no rendering meaning. A form that needs a separator
/// or text carrier must name it here, and sealing proves it is active in this
/// exact recursive context.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ContextualTextualPolicy {
    context: BoundaryDiscoveryContextIdentifier,
    separator: Option<TriggerIdentifier>,
    carrier: Option<TriggerIdentifier>,
}

impl ContextualTextualPolicy {
    pub const fn new(
        context: BoundaryDiscoveryContextIdentifier,
        separator: Option<TriggerIdentifier>,
        carrier: Option<TriggerIdentifier>,
    ) -> Self {
        Self {
            context,
            separator,
            carrier,
        }
    }

    pub const fn context(&self) -> BoundaryDiscoveryContextIdentifier {
        self.context
    }

    pub const fn separator(&self) -> Option<TriggerIdentifier> {
        self.separator
    }

    pub const fn carrier(&self) -> Option<TriggerIdentifier> {
        self.carrier
    }
}

/// Identity-bearing canonical textual output policy for every source context.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TextualRenderingPolicy {
    contexts: Vec<ContextualTextualPolicy>,
}

impl TextualRenderingPolicy {
    pub fn new(mut contexts: Vec<ContextualTextualPolicy>) -> Self {
        contexts.sort_unstable_by_key(ContextualTextualPolicy::context);
        Self { contexts }
    }

    pub fn for_context(
        &self,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Option<&ContextualTextualPolicy> {
        self.contexts
            .binary_search_by_key(&context, ContextualTextualPolicy::context)
            .ok()
            .map(|index| &self.contexts[index])
    }

    fn contexts(&self) -> &[ContextualTextualPolicy] {
        &self.contexts
    }
}

impl StructuralVocabularyIdentity {
    /// Derive a production vocabulary identity from its canonical archived
    /// vocabulary bytes.
    pub fn language(data: &[u8]) -> Self {
        Self::Language(ContentHash::derive(data))
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TableIdentityPayload<Root, Record = StructuralRule<Root>> {
    target_layout_identity: TargetLayoutIdentity,
    token_profile_identity: ContentHash<TokenProfileDomain>,
    vocabulary_identity: StructuralVocabularyIdentity,
    block_discovery: BlockTreeDiscoveryConfiguration,
    textual_rendering: TextualRenderingPolicy,
    entries: Vec<StructuralEntry<Root, Record>>,
}

impl<Root: Ord, Record> TableIdentityPayload<Root, Record> {
    /// Assemble the complete, typed identity pre-image for a structural table.
    /// Content hashes cross this boundary in their typed domains; raw digest
    /// bytes have no authoring path.
    pub fn new(
        target_layout_identity: TargetLayoutIdentity,
        token_profile_identity: ContentHash<TokenProfileDomain>,
        vocabulary_identity: StructuralVocabularyIdentity,
        block_discovery: BlockTreeDiscoveryConfiguration,
        textual_rendering: TextualRenderingPolicy,
        mut entries: Vec<StructuralEntry<Root, Record>>,
    ) -> Self {
        entries.sort_by(|left, right| left.encoded_type().cmp(right.encoded_type()));
        Self {
            target_layout_identity,
            token_profile_identity,
            vocabulary_identity,
            block_discovery,
            textual_rendering,
            entries,
        }
    }
}

/// The table archives pass-one boundary data and the descriptor vocabulary.
/// Its identity layout advances independently of the unchanged
/// structural-value layout.
pub struct StructuralTableDomain;
impl HashDomain for StructuralTableDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 addressed structural table",
            layout: LayoutVersion::new(13),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AddressedStructuralTable<Root, Record = StructuralRule<Root>> {
    payload: TableIdentityPayload<Root, Record>,
    identity: ContentHash<StructuralTableDomain>,
    profile: SealedTokenProfile,
    block_discovery: SealedBlockTreeDiscoveryConfiguration,
}

/// Archive capability for the complete static table payload. The blanket
/// implementation is available when its record type is rkyv-derived; it carries
/// no grammar behavior and only writes canonical identity bytes.
pub trait ArchivedTablePayload {
    #[doc(hidden)]
    fn table_identity(&self) -> Result<ContentHash<StructuralTableDomain>, ArchiveError>;
}

impl<Root, Record> ArchivedTablePayload for TableIdentityPayload<Root, Record>
where
    Self: rkyv::Archive
        + for<'serialize> rkyv::Serialize<
            rkyv::rancor::Strategy<
                rkyv::ser::Serializer<
                    rkyv::util::AlignedVec,
                    rkyv::ser::allocator::ArenaHandle<'serialize>,
                    rkyv::ser::sharing::Share,
                >,
                rkyv::rancor::Error,
            >,
        >,
{
    fn table_identity(&self) -> Result<ContentHash<StructuralTableDomain>, ArchiveError> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map_err(|error| ArchiveError::Serialize(error.to_string()))?;
        Ok(ContentHash::derive(bytes.as_ref()))
    }
}

impl<Root, Record> AddressedStructuralTable<Root, Record>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    pub fn seal(
        payload: TableIdentityPayload<Root, Record>,
        profile: &SealedTokenProfile,
    ) -> Result<Self, TableError<Root>>
    where
        TableIdentityPayload<Root, Record>: ArchivedTablePayload,
    {
        if payload.token_profile_identity != profile.identity() {
            return Err(TableError::TokenProfileIdentityMismatch);
        }
        let block_discovery = payload.block_discovery.seal(profile)?;
        Self::validate(&payload, profile)?;
        let identity = payload.table_identity()?;
        Ok(Self {
            payload,
            identity,
            profile: profile.clone(),
            block_discovery,
        })
    }

    pub fn identity(&self) -> ContentHash<StructuralTableDomain> {
        self.identity
    }
    pub fn entry(&self, expected: &EncodedTypeId<Root>) -> Option<&StructuralEntry<Root, Record>> {
        self.payload
            .entries
            .iter()
            .find(|entry| entry.encoded_type() == expected)
    }
    pub fn token_profile_identity(&self) -> ContentHash<TokenProfileDomain> {
        self.payload.token_profile_identity
    }

    /// The exact sealed profile validated when this table was sealed.
    pub fn token_profile(&self) -> &SealedTokenProfile {
        &self.profile
    }

    /// The sealed block-discovery rules archived in this table's identity
    /// payload and validated against [`Self::token_profile`].
    pub fn block_discovery(&self) -> &SealedBlockTreeDiscoveryConfiguration {
        &self.block_discovery
    }

    /// The identity-bearing canonical source rendering policy.
    pub fn textual_rendering(&self) -> &TextualRenderingPolicy {
        &self.payload.textual_rendering
    }
    pub fn vocabulary_identity(&self) -> StructuralVocabularyIdentity {
        self.payload.vocabulary_identity
    }
    fn validate(
        payload: &TableIdentityPayload<Root, Record>,
        profile: &SealedTokenProfile,
    ) -> Result<(), TableError<Root>> {
        Self::validate_textual_rendering(
            &payload.textual_rendering,
            profile,
            &payload.block_discovery,
        )?;
        let mut encoded_types = std::collections::BTreeSet::new();
        for entry in &payload.entries {
            let type_id = entry.encoded_type();
            if !encoded_types.insert(type_id.clone()) {
                return Err(TableError::DuplicateEncodedType {
                    entry: type_id.clone(),
                });
            }
            if entry.constructors().is_empty() {
                return Err(TableError::EmptyEntry {
                    entry: type_id.clone(),
                });
            }
            let mut constructors = std::collections::BTreeSet::new();
            for codec in entry.constructors() {
                if !constructors.insert(codec.constructor().clone()) {
                    return Err(TableError::DuplicateConstructor {
                        entry: type_id.clone(),
                        constructor: codec.constructor().clone(),
                    });
                }
                if codec.constructor().type_id() != type_id {
                    return Err(TableError::ConstructorUnderWrongEntry {
                        constructor: codec.constructor().clone(),
                        entry: type_id.clone(),
                    });
                }
                let mut forms = std::collections::BTreeSet::new();
                for accepted in codec.decode_forms() {
                    if !forms.insert(accepted.identity()) {
                        return Err(TableError::DuplicateDecodeForm {
                            constructor: codec.constructor().clone(),
                            form: accepted.identity(),
                        });
                    }
                    Self::validate_rule(accepted.rule(), profile, &payload.block_discovery)?;
                }
                Self::validate_rule(codec.encode_form(), profile, &payload.block_discovery)?;
            }
            entry.validate_disjoint_with(&payload.entries)?;
        }
        Ok(())
    }

    fn validate_textual_rendering(
        policy: &TextualRenderingPolicy,
        profile: &SealedTokenProfile,
        block_discovery: &BlockTreeDiscoveryConfiguration,
    ) -> Result<(), TableError<Root>> {
        let contexts = block_discovery.boundaries().contexts();
        if policy.contexts().len() != contexts.len()
            || !policy
                .contexts()
                .iter()
                .zip(contexts)
                .all(|(policy, context)| policy.context() == context.identifier())
        {
            return Err(TableError::TextualPolicyContextMismatch);
        }
        for policy in policy.contexts() {
            let active = contexts
                .iter()
                .find(|context| context.identifier() == policy.context())
                .expect("the exact context set was checked above")
                .triggers()
                .triggers();
            for (identifier, required) in [
                (policy.separator(), "whitespace"),
                (policy.carrier(), "carrier"),
            ] {
                let Some(identifier) = identifier else {
                    continue;
                };
                if !active.contains(&identifier) {
                    return Err(TableError::InactiveTextualPolicyTrigger {
                        context: policy.context(),
                        identifier,
                    });
                }
                let valid = match required {
                    "whitespace" => matches!(
                        profile.definition(identifier)?.trigger,
                        Trigger::Whitespace { .. }
                    ),
                    "carrier" => matches!(
                        profile.definition(identifier)?.trigger,
                        Trigger::Carrier { .. } | Trigger::CurlyText
                    ),
                    _ => unreachable!("fixed policy trigger kinds"),
                };
                if !valid {
                    return Err(TableError::WrongTextualPolicyTrigger {
                        context: policy.context(),
                        identifier,
                        required,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_rule(
        rule: &Record,
        profile: &SealedTokenProfile,
        block_discovery: &BlockTreeDiscoveryConfiguration,
    ) -> Result<(), TableError<Root>> {
        struct Roles<Root> {
            values: BTreeMap<StableRoleId, SharedDescriptor<Root>>,
            duplicate: Option<StableRoleId>,
            zero: bool,
            mismatch: Option<(StableRoleId, StableRoleId)>,
        }
        impl<Root: Clone> FieldVisitor<Root> for Roles<Root> {
            fn field<Role: crate::ids::FieldRole>(
                &mut self,
                position: &crate::form::Position<Role, Root>,
            ) {
                let expected = StableRoleId::for_role::<Role>();
                let actual = position.role();
                self.zero |= actual.value() == 0;
                if actual != expected {
                    self.mismatch = Some((expected, actual));
                }
                if self
                    .values
                    .insert(actual, position.descriptor().clone())
                    .is_some()
                {
                    self.duplicate = Some(actual);
                }
            }
        }
        let mut roles = Roles {
            values: BTreeMap::new(),
            duplicate: None,
            zero: false,
            mismatch: None,
        };
        rule.fields().expose(&mut roles);
        if roles.zero {
            return Err(TableError::ZeroRoleIdentity);
        }
        if let Some((expected, actual)) = roles.mismatch {
            return Err(TableError::RoleIdentityMismatch { expected, actual });
        }
        if let Some(role) = roles.duplicate {
            return Err(TableError::DuplicateRole { role });
        }
        let root = rule.root_role();
        let descriptor = roles
            .values
            .get(&root)
            .ok_or(TableError::MissingRole { role: root })?;
        Self::validate_descriptor(descriptor, &roles.values, profile, block_discovery)
    }

    fn validate_descriptor(
        descriptor: &SharedDescriptor<Root>,
        roles: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
        profile: &SealedTokenProfile,
        block_discovery: &BlockTreeDiscoveryConfiguration,
    ) -> Result<(), TableError<Root>> {
        match descriptor {
            SharedDescriptor::Application {
                operator,
                head,
                payload,
            } => {
                if !matches!(
                    profile.definition(*operator)?.trigger,
                    Trigger::Application { .. } | Trigger::Punctuation { .. }
                ) {
                    return Err(TableError::WrongTriggerKind {
                        identifier: *operator,
                    });
                }
                for role in [head, payload] {
                    let child = roles
                        .get(role)
                        .ok_or(TableError::MissingRole { role: *role })?;
                    Self::validate_descriptor(child, roles, profile, block_discovery)?;
                }
            }
            SharedDescriptor::InlineApplication {
                operator,
                head,
                payload,
            } => {
                if !matches!(
                    profile.definition(*operator)?.trigger,
                    Trigger::Application { .. } | Trigger::Punctuation { .. }
                ) {
                    return Err(TableError::WrongTriggerKind {
                        identifier: *operator,
                    });
                }
                Self::validate_descriptor(head, roles, profile, block_discovery)?;
                Self::validate_descriptor(payload, roles, profile, block_discovery)?;
            }
            SharedDescriptor::Alternation(alternatives) => {
                if alternatives.is_empty() {
                    return Err(TableError::EmptyAlternation);
                }
                for alternative in alternatives {
                    Self::validate_descriptor(alternative, roles, profile, block_discovery)?;
                }
            }
            SharedDescriptor::Delimited { boundary, content }
            | SharedDescriptor::ItemBoundary { boundary, content } => {
                if !matches!(
                    profile.definition(*boundary)?.trigger,
                    Trigger::Boundary { .. }
                ) {
                    return Err(TableError::WrongTriggerKind {
                        identifier: *boundary,
                    });
                }
                if !block_discovery
                    .boundaries()
                    .contexts()
                    .iter()
                    .any(|context| context.triggers().triggers().contains(boundary))
                {
                    return Err(TableError::UnconfiguredDiscoveryBoundary {
                        boundary: *boundary,
                    });
                }
                let child = roles
                    .get(content)
                    .ok_or(TableError::MissingRole { role: *content })?;
                Self::validate_descriptor(child, roles, profile, block_discovery)?;
            }
            SharedDescriptor::Carrier { carrier, content } => {
                if !matches!(
                    profile.definition(*carrier)?.trigger,
                    Trigger::Carrier { .. } | Trigger::CurlyText
                ) {
                    return Err(TableError::WrongTriggerKind {
                        identifier: *carrier,
                    });
                }
                Self::validate_descriptor(content, roles, profile, block_discovery)?;
            }
            SharedDescriptor::Repeated { element, .. } => {
                Self::validate_descriptor(element, roles, profile, block_discovery)?
            }
            SharedDescriptor::Declaration(atom)
            | SharedDescriptor::Reference(atom)
            | SharedDescriptor::DeclarationExcluding { atom, .. }
            | SharedDescriptor::ReferenceExcluding { atom, .. } => {
                if let Some(trigger) = atom.trigger {
                    if !matches!(
                        profile.definition(trigger)?.trigger,
                        Trigger::LeadingCharacterClass { .. }
                    ) {
                        return Err(TableError::WrongTriggerKind {
                            identifier: trigger,
                        });
                    }
                }
            }
            SharedDescriptor::OrderedProduct(product) => {
                let mut members = std::collections::BTreeSet::new();
                for role in product.members() {
                    if !members.insert(*role) {
                        return Err(TableError::DuplicateProductRole { role: *role });
                    }
                    let child = roles
                        .get(role)
                        .ok_or(TableError::MissingRole { role: *role })?;
                    if !matches!(child, SharedDescriptor::Delegate { .. }) {
                        return Err(TableError::ProductMemberNotDelegate { role: *role });
                    }
                    Self::validate_descriptor(child, roles, profile, block_discovery)?;
                }
            }
            SharedDescriptor::OrderedSequence(sequence)
            | SharedDescriptor::AdjacentSequence(sequence) => {
                let mut members = std::collections::BTreeSet::new();
                for role in sequence.members() {
                    if !members.insert(*role) {
                        return Err(TableError::DuplicateSequenceRole { role: *role });
                    }
                    let child = roles
                        .get(role)
                        .ok_or(TableError::MissingRole { role: *role })?;
                    Self::validate_descriptor(child, roles, profile, block_discovery)?;
                }
            }
            SharedDescriptor::Literal(_)
            | SharedDescriptor::Leaf(_)
            | SharedDescriptor::Delegate { .. } => {}
        }
        Ok(())
    }
}
