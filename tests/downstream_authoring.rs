//! A real downstream vocabulary: its records and encoded value live here, not
//! in structural-codec. The only imported execution path is the shared
//! evaluator through `Textual`.

use std::collections::BTreeMap;

use name_table::{Identifier, IdentifierNamespace, NameTable, NameTableError, NameTransaction};
use raw_discovery::{
    BlockTreeDiscoveryConfiguration, BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
    BoundaryDiscoveryContextIdentifier, RawProfile, TokenProfileError, TriggerSet,
};
use structural_codec::{
    AcceptedDecodeForm, AddressedStructuralTable, AtomCase, AtomDescriptor, AuthoringError,
    ChunkName, ConstructorCodec, ContextualTextualPolicy, DecodeError, DecodeFormId, EncodeError,
    EncodedConstructorId, EncodedForm, EncodedLanguage, FieldEnd, FieldLink, FieldRole, FieldValue,
    Position, RuleCoproduct, ScopedEncodedTypeId, SharedDescriptor, SingleChunkRequired,
    StructuralEntry, StructuralValue, StructuralVocabularyIdentity, StructureRecord, TableError,
    TableIdentityPayload, TargetLayoutIdentity, TextChunk, Textual, TextualForm,
    TextualRenderingPolicy,
};

const VALUE: ScopedEncodedTypeId = ScopedEncodedTypeId::schema(72);

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
struct PascalRole;

impl FieldRole for PascalRole {
    const STABLE_ID: u16 = 901;
}

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
struct CamelRole;

impl FieldRole for CamelRole {
    const STABLE_ID: u16 = 902;
}

/// A downstream archived record with one real typed field.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct PascalRecord {
    root: Position<PascalRole>,
}

impl PascalRecord {
    fn new() -> Result<Self, AuthoringError> {
        Ok(Self {
            root: Position::try_new(SharedDescriptor::Atom(AtomDescriptor::with_case(
                AtomCase::PascalCase,
            )))?,
        })
    }
}

impl StructureRecord for PascalRecord {
    type View<'record>
        = FieldLink<'record, PascalRole, FieldEnd>
    where
        Self: 'record;

    fn root_role(&self) -> structural_codec::StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.root, FieldEnd)
    }
}

/// A second downstream archived record. `RuleCoproduct` combines it with the
/// first without a language grammar dispatch implementation.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct CamelRecord {
    root: Position<CamelRole>,
}

impl CamelRecord {
    fn new() -> Result<Self, AuthoringError> {
        Ok(Self {
            root: Position::try_new(SharedDescriptor::Atom(AtomDescriptor::with_case(
                AtomCase::CamelCase,
            )))?,
        })
    }
}

impl StructureRecord for CamelRecord {
    type View<'record>
        = FieldLink<'record, CamelRole, FieldEnd>
    where
        Self: 'record;

    fn root_role(&self) -> structural_codec::StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.root, FieldEnd)
    }
}

type ExternalRules = RuleCoproduct<PascalRecord, CamelRecord>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExternalEncoded {
    Pascal(Identifier),
    Camel(Identifier),
}

struct ExternalLanguage;

impl EncodedForm for ExternalEncoded {
    type Language = ExternalLanguage;
}

#[derive(Debug, thiserror::Error)]
enum ExternalTextualError {
    #[error(transparent)]
    Authoring(#[from] AuthoringError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error(transparent)]
    Profile(#[from] TokenProfileError),
    #[error(transparent)]
    Names(#[from] NameTableError),
    #[error(transparent)]
    Chunks(#[from] SingleChunkRequired),
    #[error(transparent)]
    Table(#[from] TableError),
    #[error("expected the external value record")]
    WrongRecord,
}

struct ExternalTextual {
    table: AddressedStructuralTable<ExternalRules>,
}

impl ExternalTextual {
    fn new(vocabulary: &[u8]) -> Result<Self, ExternalTextualError> {
        let profile = RawProfile::standard().seal()?;
        let pascal = ExternalRules::Left(PascalRecord::new()?);
        let camel = ExternalRules::Right(CamelRecord::new()?);
        let entry = StructuralEntry::new(
            VALUE,
            vec![
                ConstructorCodec::new(
                    EncodedConstructorId::under(VALUE, 1),
                    vec![AcceptedDecodeForm::new(
                        DecodeFormId::new(1),
                        pascal.clone(),
                    )],
                    pascal,
                ),
                ConstructorCodec::new(
                    EncodedConstructorId::under(VALUE, 2),
                    vec![AcceptedDecodeForm::new(DecodeFormId::new(2), camel.clone())],
                    camel,
                ),
            ],
        );
        Ok(Self {
            table: AddressedStructuralTable::seal(
                TableIdentityPayload::new(
                    EncodedLanguage::Schema,
                    TargetLayoutIdentity::derive(b"external encoded layout"),
                    profile.identity(),
                    StructuralVocabularyIdentity::language(vocabulary),
                    BlockTreeDiscoveryConfiguration::new(
                        BoundaryDiscoveryConfiguration::new(
                            BoundaryDiscoveryContextIdentifier::new(1),
                            vec![BoundaryDiscoveryContext::new(
                                BoundaryDiscoveryContextIdentifier::new(1),
                                TriggerSet::new(vec![]),
                            )],
                            vec![],
                        ),
                        vec![],
                    ),
                    TextualRenderingPolicy::new(vec![ContextualTextualPolicy::new(
                        BoundaryDiscoveryContextIdentifier::new(1),
                        None,
                        None,
                    )]),
                    BTreeMap::from([(VALUE, entry)]),
                ),
                &profile,
            )?,
        })
    }
}

impl Textual<ExternalRules> for ExternalTextual {
    type Encoded = ExternalEncoded;
    type Language = ExternalLanguage;
    type Error = ExternalTextualError;

    fn structuretree(&self) -> &AddressedStructuralTable<ExternalRules> {
        &self.table
    }

    fn missing_root_object(&self) -> Self::Error {
        ExternalTextualError::WrongRecord
    }

    fn reify(
        &self,
        expected: ScopedEncodedTypeId,
        mirror: &StructuralValue,
        _names: &mut NameTransaction<'_>,
    ) -> Result<Self::Encoded, Self::Error> {
        if expected != VALUE {
            return Err(ExternalTextualError::WrongRecord);
        }
        match mirror.constructor().local() {
            1 => match mirror.field::<PascalRole>() {
                Some(FieldValue::Atom(identifier)) => Ok(ExternalEncoded::Pascal(*identifier)),
                _ => Err(ExternalTextualError::WrongRecord),
            },
            2 => match mirror.field::<CamelRole>() {
                Some(FieldValue::Atom(identifier)) => Ok(ExternalEncoded::Camel(*identifier)),
                _ => Err(ExternalTextualError::WrongRecord),
            },
            _ => Err(ExternalTextualError::WrongRecord),
        }
    }

    fn reflect(
        &self,
        expected: ScopedEncodedTypeId,
        encoded: &Self::Encoded,
        _names: &NameTable,
    ) -> Result<StructuralValue, Self::Error> {
        if expected != VALUE {
            return Err(ExternalTextualError::WrongRecord);
        }
        match encoded {
            ExternalEncoded::Pascal(identifier) => {
                let mut record = StructuralValue::record(EncodedConstructorId::under(VALUE, 1));
                record.insert::<PascalRole>(FieldValue::Atom(*identifier))?;
                Ok(record.finish())
            }
            ExternalEncoded::Camel(identifier) => {
                let mut record = StructuralValue::record(EncodedConstructorId::under(VALUE, 2));
                record.insert::<CamelRole>(FieldValue::Atom(*identifier))?;
                Ok(record.finish())
            }
        }
    }
}

#[test]
fn external_archived_records_round_trip_through_the_shared_textual_evaluator()
-> Result<(), ExternalTextualError> {
    let textual = ExternalTextual::new(b"external vocabulary sidecar")?;
    let other = ExternalTextual::new(b"other external vocabulary sidecar")?;
    assert_ne!(textual.table.identity(), other.table.identity());

    let mut names = NameTable::new(IdentifierNamespace::Schema);
    for source in ["Pascal", "camel"] {
        let encoded = textual.unview(
            VALUE,
            &TextualForm::from_chunks(vec![TextChunk {
                name: ChunkName::unit(),
                text: source.to_owned(),
            }]),
            &mut names,
        )?;
        assert!(matches!(
            (source, &encoded),
            ("Pascal", ExternalEncoded::Pascal(_)) | ("camel", ExternalEncoded::Camel(_))
        ));
        assert_eq!(textual.view(VALUE, &encoded, &names)?.sole_text()?, source);
    }
    Ok(())
}

#[test]
fn checked_external_manual_mirror_refuses_duplicate_typed_roles() -> Result<(), ExternalTextualError>
{
    let mut record = StructuralValue::record(EncodedConstructorId::under(VALUE, 1));
    record.insert::<PascalRole>(FieldValue::Atom(
        NameTable::new(IdentifierNamespace::Schema).intern(name_table::Name::new("Pascal"))?,
    ))?;
    assert!(matches!(
        record.insert::<PascalRole>(FieldValue::Atom(
            NameTable::new(IdentifierNamespace::Schema).intern(name_table::Name::new("Other"))?,
        )),
        Err(AuthoringError::DuplicateRoleIdentity { .. })
    ));
    Ok(())
}
