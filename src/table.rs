//! Sealed, vocabulary-identified tables of archived typed rule records.
//!
//! A downstream vocabulary can construct its typed records without access to
//! raw identity fields, combine several record shapes with
//! [`RuleCoproduct`](crate::form::RuleCoproduct),
//! then seal the whole table as the validation boundary. The shared evaluator
//! and prover operate on its `StructureRecord` field data; the record type does
//! not implement grammar behavior:
//!
//! ```
//! use std::collections::BTreeMap;
//!
//! use raw_discovery::{RawProfile, TriggerSet};
//! use structural_codec::{
//!     AcceptedDecodeForm, AddressedStructuralTable, ConstructorCodec, DecodeFormId,
//!     EncodedConstructorId, EncodedLanguage, LeafCodec, ScopedEncodedTypeId,
//!     SharedDescriptor, StructuralEntry, StructuralRule, StructuralVocabularyIdentity,
//!     TableIdentityPayload, TargetLayoutIdentity, UnaryRule,
//! };
//!
//! let profile = RawProfile::standard().seal()?;
//! let type_id = ScopedEncodedTypeId::schema(7);
//! let rule = StructuralRule::Unary(
//!     UnaryRule::new(SharedDescriptor::Leaf(LeafCodec::Integer)).expect("built-in role"),
//! );
//! let constructor = EncodedConstructorId::under(type_id, 1);
//! let entry = StructuralEntry::new(
//!     type_id,
//!     vec![ConstructorCodec::new(
//!         constructor,
//!         vec![AcceptedDecodeForm::new(DecodeFormId::new(1), rule.clone())],
//!         rule,
//!     )],
//! );
//! let payload = TableIdentityPayload::new(
//!     EncodedLanguage::Schema,
//!     TargetLayoutIdentity::derive(b"downstream encoded layout"),
//!     profile.identity(),
//!     StructuralVocabularyIdentity::language(b"downstream vocabulary"),
//!     TriggerSet::new(vec![]),
//!     BTreeMap::from([(type_id, entry)]),
//! );
//! let table = AddressedStructuralTable::seal(payload, &profile)?;
//! assert!(table.entry(type_id).is_some());
//! # Ok::<(), structural_codec::TableError>(())
//! ```

use std::collections::BTreeMap;

use content_identity::{ArchiveError, ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use raw_discovery::{SealedTokenProfile, TokenProfileDomain, Trigger, TriggerSet};

use crate::codec::StructuralEntry;
use crate::error::TableError;
use crate::form::{
    BorrowedFieldView, FieldVisitor, SharedDescriptor, StructuralRule, StructureRecord,
};
use crate::ids::{EncodedLanguage, ScopedEncodedTypeId, StableRoleId};

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

/// A separate domain makes test-only sidecars unable to masquerade as a
/// production vocabulary even when their language dimension is Schema.
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

impl StructuralVocabularyIdentity {
    /// Derive a production vocabulary identity from its canonical archived
    /// vocabulary bytes.
    pub fn language(data: &[u8]) -> Self {
        Self::Language(ContentHash::derive(data))
    }

    #[cfg(test)]
    pub(crate) fn fixture(label: &[u8]) -> Self {
        Self::Fixture(ContentHash::derive(label))
    }

    fn is_fixture(self) -> bool {
        matches!(self, Self::Fixture(_))
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TableIdentityPayload<Record = StructuralRule> {
    language: EncodedLanguage,
    target_layout_identity: TargetLayoutIdentity,
    token_profile_identity: ContentHash<TokenProfileDomain>,
    vocabulary_identity: StructuralVocabularyIdentity,
    trivia_triggers: TriggerSet,
    entries: BTreeMap<ScopedEncodedTypeId, StructuralEntry<Record>>,
}

impl<Record> TableIdentityPayload<Record> {
    /// Assemble the complete, typed identity pre-image for a structural table.
    /// Content hashes cross this boundary in their typed domains; raw digest
    /// bytes have no authoring path.
    pub fn new(
        language: EncodedLanguage,
        target_layout_identity: TargetLayoutIdentity,
        token_profile_identity: ContentHash<TokenProfileDomain>,
        vocabulary_identity: StructuralVocabularyIdentity,
        trivia_triggers: TriggerSet,
        entries: BTreeMap<ScopedEncodedTypeId, StructuralEntry<Record>>,
    ) -> Self {
        Self {
            language,
            target_layout_identity,
            token_profile_identity,
            vocabulary_identity,
            trivia_triggers,
            entries,
        }
    }
}

/// One truthful R3/R4 layout bump: layout 6 to layout 7.
pub struct StructuralTableDomain;
impl HashDomain for StructuralTableDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 addressed structural table",
            layout: LayoutVersion::new(7),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AddressedStructuralTable<Record = StructuralRule> {
    payload: TableIdentityPayload<Record>,
    identity: ContentHash<StructuralTableDomain>,
}

/// Archive capability for the complete static table payload. The blanket
/// implementation is available when its record type is rkyv-derived; it carries
/// no grammar behavior and only writes canonical identity bytes.
pub trait ArchivedTablePayload {
    #[doc(hidden)]
    fn table_identity(&self) -> Result<ContentHash<StructuralTableDomain>, ArchiveError>;
}

impl<Record> ArchivedTablePayload for TableIdentityPayload<Record>
where
    Record: rkyv::Archive
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

impl<Record: StructureRecord> AddressedStructuralTable<Record> {
    pub fn seal(
        payload: TableIdentityPayload<Record>,
        profile: &SealedTokenProfile,
    ) -> Result<Self, TableError>
    where
        TableIdentityPayload<Record>: ArchivedTablePayload,
    {
        if payload.token_profile_identity != profile.identity() {
            return Err(TableError::TokenProfileIdentityMismatch);
        }
        Self::validate(&payload, profile)?;
        let identity = payload.table_identity()?;
        Ok(Self { payload, identity })
    }

    pub fn identity(&self) -> ContentHash<StructuralTableDomain> {
        self.identity
    }
    pub fn entry(&self, expected: ScopedEncodedTypeId) -> Option<&StructuralEntry<Record>> {
        self.payload.entries.get(&expected)
    }
    pub fn token_profile_identity(&self) -> ContentHash<TokenProfileDomain> {
        self.payload.token_profile_identity
    }
    pub fn vocabulary_identity(&self) -> StructuralVocabularyIdentity {
        self.payload.vocabulary_identity
    }
    pub fn language(&self) -> EncodedLanguage {
        self.payload.language
    }

    fn validate(
        payload: &TableIdentityPayload<Record>,
        profile: &SealedTokenProfile,
    ) -> Result<(), TableError> {
        profile.seal_trigger_set(payload.trivia_triggers.clone())?;
        for type_id in payload.entries.keys() {
            if type_id.language() != payload.language {
                return Err(TableError::LanguageMismatch {
                    table: payload.language,
                    encoded: *type_id,
                });
            }
            if payload.vocabulary_identity.is_fixture() != type_id.is_reserved_fixture_schema() {
                return Err(if payload.vocabulary_identity.is_fixture() {
                    TableError::FixtureVocabularyHasProductionId
                } else {
                    TableError::ReservedFixtureIdInLanguageTable
                });
            }
        }
        for (type_id, entry) in &payload.entries {
            if entry.encoded_type() != *type_id {
                return Err(TableError::EntryKeyMismatch {
                    entry: entry.encoded_type(),
                    key: *type_id,
                });
            }
            if entry.constructors().is_empty() {
                return Err(TableError::EmptyEntry { entry: *type_id });
            }
            let mut constructors = std::collections::BTreeSet::new();
            for codec in entry.constructors() {
                if !constructors.insert(codec.constructor()) {
                    return Err(TableError::DuplicateConstructor {
                        entry: *type_id,
                        constructor: codec.constructor(),
                    });
                }
                if codec.constructor().type_id() != *type_id {
                    return Err(TableError::ConstructorUnderWrongEntry {
                        constructor: codec.constructor(),
                        entry: *type_id,
                    });
                }
                let mut forms = std::collections::BTreeSet::new();
                for accepted in codec.decode_forms() {
                    if !forms.insert(accepted.identity()) {
                        return Err(TableError::DuplicateDecodeForm {
                            constructor: codec.constructor(),
                            form: accepted.identity(),
                        });
                    }
                    Self::validate_rule(accepted.rule(), profile)?;
                }
                Self::validate_rule(codec.encode_form(), profile)?;
            }
            entry.validate_disjoint_with(&payload.entries)?;
        }
        Ok(())
    }

    fn validate_rule(rule: &Record, profile: &SealedTokenProfile) -> Result<(), TableError> {
        struct Roles {
            values: BTreeMap<StableRoleId, SharedDescriptor>,
            duplicate: Option<StableRoleId>,
            zero: bool,
            mismatch: Option<(StableRoleId, StableRoleId)>,
        }
        impl FieldVisitor for Roles {
            fn field<Role: crate::ids::FieldRole>(
                &mut self,
                position: &crate::form::Position<Role>,
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
        Self::validate_descriptor(descriptor, &roles.values, profile)
    }

    fn validate_descriptor(
        descriptor: &SharedDescriptor,
        roles: &BTreeMap<StableRoleId, SharedDescriptor>,
        profile: &SealedTokenProfile,
    ) -> Result<(), TableError> {
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
                    Self::validate_descriptor(child, roles, profile)?;
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
                let child = roles
                    .get(content)
                    .ok_or(TableError::MissingRole { role: *content })?;
                Self::validate_descriptor(child, roles, profile)?;
            }
            SharedDescriptor::Repeated { element, .. } => {
                Self::validate_descriptor(element, roles, profile)?
            }
            SharedDescriptor::Atom(atom) => {
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
            SharedDescriptor::Literal(_)
            | SharedDescriptor::Leaf(_)
            | SharedDescriptor::Delegate { .. } => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use raw_discovery::TriggerSet;

    use super::*;
    use crate::codec::{AcceptedDecodeForm, ConstructorCodec};
    use crate::form::{AtomDescriptor, SharedDescriptor, StructuralRule, UnaryRule};
    use crate::ids::{DecodeFormId, EncodedConstructorId, ScopedEncodedTypeId};

    fn logos_entry() -> StructuralEntry {
        let type_id = ScopedEncodedTypeId::logos(3);
        let rule = StructuralRule::Unary(
            UnaryRule::new(SharedDescriptor::Atom(AtomDescriptor::any_case())).expect("role"),
        );
        StructuralEntry::new(
            type_id,
            vec![ConstructorCodec::new(
                EncodedConstructorId::under(type_id, 1),
                vec![AcceptedDecodeForm::new(DecodeFormId::new(1), rule.clone())],
                rule,
            )],
        )
    }

    #[test]
    fn wrong_language_and_profile_identity_are_refused_at_seal() {
        let profile = crate::fixture::FixtureBuilder::token_profile();
        let base = TableIdentityPayload::new(
            EncodedLanguage::Schema,
            TargetLayoutIdentity::derive(b"test target layout"),
            profile.identity(),
            StructuralVocabularyIdentity::fixture(b"test fixture vocabulary"),
            TriggerSet::new(vec![]),
            BTreeMap::from([(logos_entry().encoded_type(), logos_entry())]),
        );
        assert!(matches!(
            AddressedStructuralTable::seal(base, &profile),
            Err(TableError::LanguageMismatch { .. })
        ));

        let other_profile = raw_discovery::RawProfile::nomos_extended()
            .seal()
            .expect("valid alternate profile");
        let zero: TableIdentityPayload = TableIdentityPayload::new(
            EncodedLanguage::Schema,
            TargetLayoutIdentity::derive(b"test target layout"),
            other_profile.identity(),
            StructuralVocabularyIdentity::fixture(b"test fixture vocabulary"),
            TriggerSet::new(vec![]),
            BTreeMap::new(),
        );
        assert!(matches!(
            AddressedStructuralTable::seal(zero, &profile),
            Err(TableError::TokenProfileIdentityMismatch)
        ));
    }
}
