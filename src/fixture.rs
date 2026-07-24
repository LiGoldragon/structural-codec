//! The proof-of-concept fixture universe — the acceptance gate of slice one. Builds
//! a concrete `AddressedStructuralTable` in the explicit `FIXTURE_UNIVERSE` covering:
//! the `CommitSequence`/`StateDigest` newtypes, a `DatabaseMarker` struct whose body
//! is a repeat of `Field`, whose sole constructor is the bare elided-name `Type`,
//! the `Documentation → Summary → Text` string-rejoin delegate chain, and standalone
//! `Integer`/`Float`/`Text` leaf types. The builder is data-bearing (it carries the
//! block delimiter), so law 4 can mint two revisions that differ only in textual
//! form.

use std::collections::BTreeMap;

use raw_discovery::{Delimiter, RawProfile, SealedTokenProfile, TriggerIdentifier, TriggerSet};

use crate::authoring::{AuthoringForm, ObjectSymbolPrefixedBlock};
use crate::codec::{ConstructorCodec, StructuralEntry};
use crate::error::TableError;
use crate::form::{
    AtomForm, CarrierLeaf, LeafCodec, LeafForm, ScalarLeaf, SequenceForm, StructuralForm,
};
use crate::ids::{EncodedConstructorId, PositionalSignature, ScopedEncodedTypeId};
use crate::table::{
    AddressedStructuralTable, EncodedLayoutIdentity, RawProfileIdentity, TableIdentityPayload,
};
use raw_discovery::AtomCase;

// Fixture type ids (local numbers echo the design's worked examples).
pub const INTEGER: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(10);
pub const FLOAT: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(9);
pub const TEXT: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(33);
pub const CARRIED_TEXT: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(34);
pub const SUMMARY: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(32);
pub const DOCUMENTATION: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(31);
pub const COMMIT_SEQUENCE: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(1);
pub const STATE_DIGEST: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(2);
pub const DATABASE_MARKER: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(3);
pub const FIELD: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(23);

pub const PARENTHESIS_BOUNDARY: TriggerIdentifier = TriggerIdentifier::new(0);
pub const SQUARE_BOUNDARY: TriggerIdentifier = TriggerIdentifier::new(1);
pub const BRACE_BOUNDARY: TriggerIdentifier = TriggerIdentifier::new(2);
pub const APPLICATION_OPERATOR: TriggerIdentifier = TriggerIdentifier::new(3);
pub const PIPE_TEXT_CARRIER: TriggerIdentifier = TriggerIdentifier::new(4);
pub const WHITESPACE_TRIVIA: TriggerIdentifier = TriggerIdentifier::new(5);
pub const COMMENT_TRIVIA: TriggerIdentifier = TriggerIdentifier::new(6);

/// Builds the fixture table. Carries the varying textual surface so two tables
/// can differ only in form.
#[derive(Clone, Debug)]
pub struct FixtureBuilder {
    newtype_delimiter: Delimiter,
}

impl Default for FixtureBuilder {
    fn default() -> Self {
        Self {
            newtype_delimiter: Delimiter::Brace,
        }
    }
}

impl FixtureBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The delimiter the newtype bodies use. Varying it yields a table that differs
    /// from another only in textual form.
    pub fn with_newtype_delimiter(mut self, delimiter: Delimiter) -> Self {
        self.newtype_delimiter = delimiter;
        self
    }

    /// Seal the fixture table (identity computed over the payload, stored outside).
    pub fn build(&self) -> Result<AddressedStructuralTable, TableError> {
        let profile = Self::token_profile();
        let mut entries: BTreeMap<ScopedEncodedTypeId, StructuralEntry> = BTreeMap::new();
        for entry in self.entries() {
            entries.insert(entry.core_type, entry);
        }
        let payload = TableIdentityPayload {
            core_universe: crate::ids::FIXTURE_UNIVERSE,
            core_layout_identity: EncodedLayoutIdentity([0u8; 32]),
            raw_profile_identity: RawProfileIdentity::from_profile(&profile),
            trivia_triggers: TriggerSet::new(vec![WHITESPACE_TRIVIA, COMMENT_TRIVIA]),
            leaf_codec_contracts: Vec::new(),
            entries,
        };
        AddressedStructuralTable::seal(payload, &profile)
    }

    pub fn token_profile() -> SealedTokenProfile {
        RawProfile::standard()
            .seal()
            .expect("the standard fixture token profile seals")
    }

    fn entries(&self) -> Vec<StructuralEntry> {
        vec![
            Self::leaf_entry(INTEGER, ScalarLeaf::Integer),
            Self::leaf_entry(FLOAT, ScalarLeaf::Float),
            Self::leaf_entry(TEXT, ScalarLeaf::Text),
            Self::carrier_entry(CARRIED_TEXT),
            Self::delegate_entry(DOCUMENTATION, SUMMARY),
            Self::delegate_entry(SUMMARY, TEXT),
            self.newtype_entry(COMMIT_SEQUENCE),
            self.newtype_entry(STATE_DIGEST),
            self.struct_entry(DATABASE_MARKER),
            Self::field_entry(),
        ]
    }

    /// A leaf type: one constructor whose sole form is a scalar leaf.
    fn leaf_entry(core_type: ScopedEncodedTypeId, scalar: ScalarLeaf) -> StructuralEntry {
        let form = StructuralForm::Leaf(LeafForm::scalar(scalar));
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    }

    /// A text leaf whose complete body is carried by the profile's pipe-text
    /// trigger. Delimiters and application glyphs inside the body are data, not
    /// recursive structure.
    fn carrier_entry(core_type: ScopedEncodedTypeId) -> StructuralEntry {
        let form = StructuralForm::Leaf(LeafForm::with_trigger(
            LeafCodec::Carrier(CarrierLeaf::PipeText),
            PIPE_TEXT_CARRIER,
        ));
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    }

    /// A transparent newtype wrapper: one constructor delegating to the inner type.
    fn delegate_entry(
        core_type: ScopedEncodedTypeId,
        inner: ScopedEncodedTypeId,
    ) -> StructuralEntry {
        let form = StructuralForm::delegate(inner);
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::new(vec![inner]),
            )],
        )
    }

    /// A newtype declaration `Object.{ Type }` built from the AUTHORING vocabulary and
    /// normalized to the kernel `Application` form before it enters the table.
    fn newtype_entry(&self, core_type: ScopedEncodedTypeId) -> StructuralEntry {
        let authoring = AuthoringForm::ObjectPrefixed(ObjectSymbolPrefixedBlock {
            object: AtomForm::with_case(AtomCase::PascalCase),
            operator: APPLICATION_OPERATOR,
            boundary: Self::boundary_trigger(self.newtype_delimiter),
            delimiter: self.newtype_delimiter,
            sequence: SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        });
        let form = authoring.normalize();
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::new(vec![INTEGER]),
            )],
        )
    }

    /// A struct declaration `Object.{ Field* }` — a repeat of delegated fields.
    fn struct_entry(&self, core_type: ScopedEncodedTypeId) -> StructuralEntry {
        let form = StructuralForm::application(
            APPLICATION_OPERATOR,
            StructuralForm::pascal_atom(),
            StructuralForm::Delimited {
                boundary: BRACE_BOUNDARY,
                delimiter: Delimiter::Brace,
                sequence: SequenceForm::zero_or_more(StructuralForm::delegate(FIELD)),
            },
        );
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    }

    /// The `Field` type's sole constructor: a bare `Type` with its name elided.
    /// Field names are derived at the boundary and never authored in the form.
    fn field_entry() -> StructuralEntry {
        let type_only = StructuralForm::pascal_atom();
        StructuralEntry::new(
            FIELD,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(FIELD, 0),
                vec![type_only.clone()],
                type_only,
                PositionalSignature::default(),
            )],
        )
    }

    fn boundary_trigger(delimiter: Delimiter) -> TriggerIdentifier {
        match delimiter {
            Delimiter::Parenthesis => PARENTHESIS_BOUNDARY,
            Delimiter::SquareBracket => SQUARE_BOUNDARY,
            Delimiter::Brace => BRACE_BOUNDARY,
        }
    }
}
