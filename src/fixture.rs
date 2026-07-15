//! The proof-of-concept fixture universe — the acceptance gate of slice one. Builds
//! a concrete `AddressedStructuralTable` in the explicit `FIXTURE_UNIVERSE` covering:
//! the `CommitSequence`/`StateDigest` newtypes, a `DatabaseMarker` struct whose body
//! is a repeat of `Field`, the `Field` type with its TWO structurally-disjoint
//! constructors (bare `Type` with elided name versus `name.Type`), the
//! `Documentation → Summary → Text` string-rejoin delegate chain, and standalone
//! `Integer`/`Float`/`Text` leaf types. The builder is data-bearing (it carries the
//! block delimiter and committed lexicon), so law 4 can mint two revisions that
//! differ only in textual form.

use std::collections::BTreeMap;

use raw_discovery::Delimiter;

use crate::authoring::{AuthoringForm, ObjectSymbolPrefixedBlock};
use crate::codec::{ConstructorCodec, StructuralEntry};
use crate::error::TableError;
use crate::form::{AtomForm, CaseExpectation, LeafForm, ScalarLeaf, SequenceForm, StructuralForm};
use crate::ids::{CoreConstructorId, PositionalSignature, ScopedCoreTypeId, StructuralRevision};
use crate::table::{
    AddressedStructuralTable, CoreLayoutIdentity, RawProfileIdentity, TableIdentityPayload,
};

// Fixture type ids (local numbers echo the design's worked examples).
pub const INTEGER: ScopedCoreTypeId = ScopedCoreTypeId::fixture(10);
pub const FLOAT: ScopedCoreTypeId = ScopedCoreTypeId::fixture(9);
pub const TEXT: ScopedCoreTypeId = ScopedCoreTypeId::fixture(33);
pub const SUMMARY: ScopedCoreTypeId = ScopedCoreTypeId::fixture(32);
pub const DOCUMENTATION: ScopedCoreTypeId = ScopedCoreTypeId::fixture(31);
pub const COMMIT_SEQUENCE: ScopedCoreTypeId = ScopedCoreTypeId::fixture(1);
pub const STATE_DIGEST: ScopedCoreTypeId = ScopedCoreTypeId::fixture(2);
pub const DATABASE_MARKER: ScopedCoreTypeId = ScopedCoreTypeId::fixture(3);
pub const FIELD: ScopedCoreTypeId = ScopedCoreTypeId::fixture(23);

/// Builds the fixture table. Carries the varying textual surface so two revisions
/// can differ only in form.
#[derive(Clone, Debug)]
pub struct FixtureBuilder {
    newtype_delimiter: Delimiter,
    committed_lexicon: Vec<u8>,
    revision: StructuralRevision,
}

impl Default for FixtureBuilder {
    fn default() -> Self {
        Self {
            newtype_delimiter: Delimiter::Brace,
            committed_lexicon: b"fixture-lexicon-standard".to_vec(),
            revision: StructuralRevision::new(1),
        }
    }
}

impl FixtureBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The delimiter the newtype bodies use. Varying it (with a matching lexicon and
    /// revision) yields a table that differs from another only in textual form.
    pub fn with_newtype_delimiter(mut self, delimiter: Delimiter) -> Self {
        self.newtype_delimiter = delimiter;
        self
    }

    pub fn with_lexicon(mut self, lexicon: Vec<u8>) -> Self {
        self.committed_lexicon = lexicon;
        self
    }

    pub fn with_revision(mut self, revision: StructuralRevision) -> Self {
        self.revision = revision;
        self
    }

    /// Seal the fixture table (identity computed over the payload, stored outside).
    pub fn build(&self) -> Result<AddressedStructuralTable, TableError> {
        let mut entries: BTreeMap<ScopedCoreTypeId, StructuralEntry> = BTreeMap::new();
        for entry in self.entries() {
            entries.insert(entry.core_type, entry);
        }
        let payload = TableIdentityPayload {
            core_universe: crate::ids::FIXTURE_UNIVERSE,
            core_layout_identity: CoreLayoutIdentity([0u8; 32]),
            raw_profile_identity: RawProfileIdentity([1u8; 32]),
            committed_lexicon: self.committed_lexicon.clone(),
            leaf_codec_contracts: Vec::new(),
            entries,
        };
        AddressedStructuralTable::seal(self.revision, payload)
    }

    fn entries(&self) -> Vec<StructuralEntry> {
        vec![
            Self::leaf_entry(INTEGER, ScalarLeaf::Integer),
            Self::leaf_entry(FLOAT, ScalarLeaf::Float),
            Self::leaf_entry(TEXT, ScalarLeaf::Text),
            Self::delegate_entry(DOCUMENTATION, SUMMARY),
            Self::delegate_entry(SUMMARY, TEXT),
            self.newtype_entry(COMMIT_SEQUENCE),
            self.newtype_entry(STATE_DIGEST),
            self.struct_entry(DATABASE_MARKER),
            Self::field_entry(),
        ]
    }

    /// A leaf type: one constructor whose sole form is a scalar leaf.
    fn leaf_entry(core_type: ScopedCoreTypeId, scalar: ScalarLeaf) -> StructuralEntry {
        let form = StructuralForm::Leaf(LeafForm::scalar(scalar));
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    }

    /// A transparent newtype wrapper: one constructor delegating to the inner type.
    fn delegate_entry(core_type: ScopedCoreTypeId, inner: ScopedCoreTypeId) -> StructuralEntry {
        let form = StructuralForm::Delegate(inner);
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::new(vec![inner]),
            )],
        )
    }

    /// A newtype declaration `Object.{ Type }` built from the AUTHORING vocabulary and
    /// normalized to the kernel `Application` form before it enters the table.
    fn newtype_entry(&self, core_type: ScopedCoreTypeId) -> StructuralEntry {
        let authoring = AuthoringForm::ObjectPrefixed(ObjectSymbolPrefixedBlock {
            object: AtomForm::with_case(CaseExpectation::PascalCase),
            delimiter: self.newtype_delimiter,
            sequence: SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        });
        let form = authoring.normalize();
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::new(vec![INTEGER]),
            )],
        )
    }

    /// A struct declaration `Object.{ Field* }` — a repeat of delegated fields.
    fn struct_entry(&self, core_type: ScopedCoreTypeId) -> StructuralEntry {
        let form = StructuralForm::application(
            StructuralForm::pascal_atom(),
            StructuralForm::Delimited {
                delimiter: Delimiter::Brace,
                sequence: SequenceForm::zero_or_more(StructuralForm::Delegate(FIELD)),
            },
        );
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    }

    /// The `Field` type with its two structurally-disjoint constructors: a bare
    /// `Type` (name elided, derived via name-table) versus `name.Type`.
    fn field_entry() -> StructuralEntry {
        let type_only = StructuralForm::pascal_atom();
        let named = StructuralForm::application(
            StructuralForm::camel_atom(),
            StructuralForm::pascal_atom(),
        );
        StructuralEntry::new(
            FIELD,
            vec![
                ConstructorCodec::new(
                    CoreConstructorId::new(FIELD, 0),
                    vec![type_only.clone()],
                    type_only,
                    PositionalSignature::default(),
                ),
                ConstructorCodec::new(
                    CoreConstructorId::new(FIELD, 1),
                    vec![named.clone()],
                    named,
                    PositionalSignature::default(),
                ),
            ],
        )
    }
}
