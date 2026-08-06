//! Test-only concrete typed vocabulary.
//!
//! Fixture ids occupy the reserved Schema range and the table carries a separate
//! fixture vocabulary domain, so they cannot compose with production Schema data.

use std::collections::BTreeMap;

use raw_discovery::{
    BlockPrefixAttachment, BlockPrefixRule, BlockTreeDiscoveryConfiguration,
    BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext, BoundaryDiscoveryContextIdentifier,
    BoundaryDiscoveryTransition, CharacterClass, RawProfile, SealedTokenProfile, TriggerIdentifier,
    TriggerSet,
};

use crate::codec::{AcceptedDecodeForm, ConstructorCodec, StructuralEntry};
use crate::error::TableError;
use crate::form::{
    ApplicationDelimitedRule, ApplicationRule, AtomDescriptor, LeafCodec, SharedDescriptor,
    StructuralRule, UnaryRule,
};
use crate::ids::{DecodeFormId, EncodedConstructorId, ScopedEncodedTypeId};
use crate::table::{
    AddressedStructuralTable, ContextualTextualPolicy, StructuralVocabularyIdentity,
    TableIdentityPayload, TargetLayoutIdentity, TextualRenderingPolicy,
};

pub const INTEGER: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf010);
pub const FLOAT: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf011);
pub const TEXT: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf012);
pub const DOCUMENTATION: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf013);
pub const COMMIT_SEQUENCE: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf014);
pub const STATE_DIGEST: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf015);
pub const DATABASE_MARKER: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf016);
pub const FIELD: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf017);

pub const PARENTHESIS_BOUNDARY: TriggerIdentifier = TriggerIdentifier::new(0);
pub const SQUARE_BOUNDARY: TriggerIdentifier = TriggerIdentifier::new(1);
pub const BRACE_BOUNDARY: TriggerIdentifier = TriggerIdentifier::new(2);
pub const APPLICATION_OPERATOR: TriggerIdentifier = TriggerIdentifier::new(3);
pub const PIPE_CARRIER: TriggerIdentifier = TriggerIdentifier::new(4);
pub const WHITESPACE_TRIVIA: TriggerIdentifier = TriggerIdentifier::new(5);
pub const COMMENT_TRIVIA: TriggerIdentifier = TriggerIdentifier::new(6);

#[derive(Clone, Debug)]
pub struct FixtureBuilder {
    boundary: TriggerIdentifier,
}

impl Default for FixtureBuilder {
    fn default() -> Self {
        Self {
            boundary: BRACE_BOUNDARY,
        }
    }
}

impl FixtureBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Vary the authoritative boundary trigger, never a copied delimiter enum.
    pub fn with_newtype_boundary(mut self, boundary: TriggerIdentifier) -> Self {
        self.boundary = boundary;
        self
    }

    pub fn token_profile() -> SealedTokenProfile {
        RawProfile::standard()
            .seal()
            .expect("the standard fixture profile seals")
    }

    pub fn block_discovery() -> BlockTreeDiscoveryConfiguration {
        let root = BoundaryDiscoveryContextIdentifier::new(1);
        BlockTreeDiscoveryConfiguration::new(
            BoundaryDiscoveryConfiguration::new(
                root,
                vec![BoundaryDiscoveryContext::new(
                    root,
                    TriggerSet::new(vec![
                        PARENTHESIS_BOUNDARY,
                        SQUARE_BOUNDARY,
                        BRACE_BOUNDARY,
                        TriggerIdentifier::new(4),
                        WHITESPACE_TRIVIA,
                        COMMENT_TRIVIA,
                    ]),
                )],
                vec![
                    BoundaryDiscoveryTransition::new(root, PARENTHESIS_BOUNDARY, root),
                    BoundaryDiscoveryTransition::new(root, SQUARE_BOUNDARY, root),
                    BoundaryDiscoveryTransition::new(root, BRACE_BOUNDARY, root),
                ],
            ),
            vec![
                BlockPrefixAttachment::new(
                    PARENTHESIS_BOUNDARY,
                    BlockPrefixRule::new(".", CharacterClass::AsciiAlphabetic),
                ),
                BlockPrefixAttachment::new(
                    SQUARE_BOUNDARY,
                    BlockPrefixRule::new(".", CharacterClass::AsciiAlphabetic),
                ),
                BlockPrefixAttachment::new(
                    BRACE_BOUNDARY,
                    BlockPrefixRule::new(".", CharacterClass::AsciiAlphabetic),
                ),
            ],
        )
    }

    pub fn textual_rendering() -> TextualRenderingPolicy {
        TextualRenderingPolicy::new(vec![ContextualTextualPolicy::new(
            BoundaryDiscoveryContextIdentifier::new(1),
            Some(WHITESPACE_TRIVIA),
            Some(PIPE_CARRIER),
        )])
    }

    pub fn build(&self) -> Result<AddressedStructuralTable, TableError> {
        let profile = Self::token_profile();
        let entries = self
            .entries()
            .into_iter()
            .map(|entry| (entry.encoded_type(), entry))
            .collect::<BTreeMap<_, _>>();
        AddressedStructuralTable::seal(
            TableIdentityPayload::new(
                crate::ids::EncodedLanguage::Schema,
                TargetLayoutIdentity::derive(b"structural-codec fixture encoded layout R3/R4"),
                profile.identity(),
                StructuralVocabularyIdentity::fixture(
                    b"structural-codec fixture typed vocabulary R3/R4",
                ),
                Self::block_discovery(),
                Self::textual_rendering(),
                entries,
            ),
            &profile,
        )
    }

    fn entries(&self) -> Vec<StructuralEntry> {
        vec![
            Self::unary(INTEGER, SharedDescriptor::Leaf(LeafCodec::Integer)),
            Self::unary(FLOAT, SharedDescriptor::Leaf(LeafCodec::Float)),
            Self::unary(TEXT, SharedDescriptor::Leaf(LeafCodec::Text)),
            Self::unary(
                FIELD,
                SharedDescriptor::Atom(AtomDescriptor::with_case(
                    crate::AtomCase::PascalCase,
                )),
            ),
            Self::unary(
                DOCUMENTATION,
                SharedDescriptor::Delegate {
                    target: TEXT,
                    payload: None,
                },
            ),
            self.newtype(COMMIT_SEQUENCE),
            self.newtype(STATE_DIGEST),
            self.database_marker(),
        ]
    }

    fn unary(type_id: ScopedEncodedTypeId, descriptor: SharedDescriptor) -> StructuralEntry {
        let rule = StructuralRule::Unary(UnaryRule::new(descriptor).expect("fixture role"));
        Self::entry(type_id, rule)
    }

    fn newtype(&self, type_id: ScopedEncodedTypeId) -> StructuralEntry {
        let rule = StructuralRule::ApplicationDelimited(
            ApplicationDelimitedRule::new(
                APPLICATION_OPERATOR,
                self.boundary,
                SharedDescriptor::Atom(AtomDescriptor::with_case(
                    crate::AtomCase::PascalCase,
                )),
                SharedDescriptor::Atom(AtomDescriptor::with_case(
                    crate::AtomCase::PascalCase,
                )),
                1,
                Some(1),
            )
            .expect("fixture roles"),
        );
        Self::entry(type_id, rule)
    }

    fn database_marker(&self) -> StructuralEntry {
        let rule = StructuralRule::Application(
            ApplicationRule::new(
                APPLICATION_OPERATOR,
                SharedDescriptor::Atom(AtomDescriptor::with_case(
                    crate::AtomCase::PascalCase,
                )),
                SharedDescriptor::Delegate {
                    target: FIELD,
                    payload: None,
                },
            )
            .expect("fixture roles"),
        );
        Self::entry(DATABASE_MARKER, rule)
    }

    fn entry(type_id: ScopedEncodedTypeId, rule: StructuralRule) -> StructuralEntry {
        let constructor = EncodedConstructorId::fixture_schema(type_id, 1);
        StructuralEntry::new(
            type_id,
            vec![ConstructorCodec::new(
                constructor,
                vec![AcceptedDecodeForm::new(DecodeFormId::new(1), rule.clone())],
                rule,
            )],
        )
    }
}
