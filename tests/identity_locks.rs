//! Absolute digest locks for every contextual hash domain owned here.

use std::collections::BTreeMap;

use content_identity::ContentHash;
use name_table::{IdentifierNamespace, Name, NameTable};
use raw_discovery::{AtomCase, Delimiter, TriggerIdentifier, TriggerSet};
use structural_codec::fixture::FixtureBuilder;
use structural_codec::{
    AtomForm, CarrierLeaf, ConstructorCodec, DelegationPayload, EncodedConstructorId,
    EncodedLayoutIdentity, FIXTURE_UNIVERSE, ForeignLeafId, LeafCodec, LeafCodecContractId,
    LeafForm, PositionalSignature, RawProfileIdentity, ScalarLeaf, ScalarValue,
    ScopedEncodedTypeId, SequenceForm, StructuralEntry, StructuralForm, StructuralTableDomain,
    StructuralValue, TableIdentityPayload,
};

const STRUCTURAL_TABLE_LAYOUT_SIX: [u8; 32] = [
    0xb9, 0x09, 0xe0, 0x17, 0x72, 0x28, 0x3b, 0x94, 0x64, 0x47, 0x1d, 0x42, 0xb1, 0x77, 0x31, 0x50,
    0xe6, 0xfc, 0xe8, 0x89, 0x98, 0x38, 0xa0, 0x38, 0xc4, 0x07, 0x95, 0xce, 0x39, 0x1d, 0x18, 0x1d,
];
const STRUCTURAL_VALUE_LAYOUT_ONE: [u8; 32] = [
    0x26, 0x2c, 0x67, 0xb6, 0xef, 0x12, 0xb6, 0x31, 0x6f, 0x1d, 0x72, 0xaf, 0x20, 0x1d, 0xb1, 0x35,
    0x62, 0x1e, 0xd4, 0x8a, 0xcd, 0x5d, 0x96, 0xf4, 0xee, 0x04, 0x1f, 0x10, 0x21, 0xb4, 0x4c, 0x98,
];
const COMPOSITE_STRUCTURAL_TABLE_LAYOUT_SIX: [u8; 32] = [
    0x28, 0x14, 0x26, 0x2c, 0x7d, 0x02, 0xb5, 0x68, 0xcb, 0xfd, 0x68, 0x13, 0x3d, 0xcc, 0x18, 0x77,
    0x5f, 0xbf, 0x4e, 0x78, 0x32, 0xeb, 0x36, 0xcb, 0xae, 0x64, 0xc4, 0xc9, 0xce, 0x57, 0x64, 0x6b,
];

#[test]
fn fixture_table_identity_is_an_absolute_layout_six_lock() {
    let table = FixtureBuilder::new().build().expect("fixture table");
    assert_eq!(
        table.identity().bytes(),
        &STRUCTURAL_TABLE_LAYOUT_SIX,
        "structural table data or its archived layout moved"
    );
}

#[test]
fn every_structural_form_variant_is_in_the_layout_six_lock() {
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let literal = names
        .intern(Name::new("LiteralWitness"))
        .expect("intern literal witness");
    let target = ScopedEncodedTypeId::fixture(901);
    let witnessed_forms = vec![
        StructuralForm::Atom(AtomForm::any_case()),
        StructuralForm::Atom(AtomForm::with_case(AtomCase::Symbol)),
        StructuralForm::Atom(AtomForm::with_case(AtomCase::PascalCase)),
        StructuralForm::Atom(AtomForm::with_case(AtomCase::CamelCase)),
        StructuralForm::Atom(AtomForm::with_case(AtomCase::KebabCase)),
        StructuralForm::Atom(AtomForm::with_trigger(
            Some(AtomCase::PascalCase),
            TriggerIdentifier::new(20),
        )),
        StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Integer)),
        StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Float)),
        StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Text)),
        StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Boolean)),
        StructuralForm::Leaf(LeafForm::with_trigger(
            LeafCodec::Carrier(CarrierLeaf::PipeText),
            TriggerIdentifier::new(21),
        )),
        StructuralForm::Leaf(LeafForm {
            codec: LeafCodec::Foreign(ForeignLeafId(22)),
            trigger: None,
        }),
        StructuralForm::Literal(literal),
        StructuralForm::application(
            TriggerIdentifier::new(23),
            StructuralForm::pascal_atom(),
            StructuralForm::camel_atom(),
        ),
        StructuralForm::Delimited {
            boundary: TriggerIdentifier::new(24),
            delimiter: Delimiter::Parenthesis,
            sequence: SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        },
        StructuralForm::Delimited {
            boundary: TriggerIdentifier::new(25),
            delimiter: Delimiter::SquareBracket,
            sequence: SequenceForm::Repeat {
                minimum: 1,
                maximum: Some(3),
                element: Box::new(StructuralForm::camel_atom()),
            },
        },
        StructuralForm::Delimited {
            boundary: TriggerIdentifier::new(26),
            delimiter: Delimiter::Brace,
            sequence: SequenceForm::Product(Vec::new()),
        },
        StructuralForm::delegate(target),
        StructuralForm::delegate_with_payload(
            target,
            DelegationPayload::AtomCase(AtomCase::KebabCase),
        ),
    ];
    let core_type = ScopedEncodedTypeId::fixture(900);
    let payload = TableIdentityPayload {
        core_universe: FIXTURE_UNIVERSE,
        core_layout_identity: EncodedLayoutIdentity([0x33; 32]),
        raw_profile_identity: RawProfileIdentity([0x44; 32]),
        trivia_triggers: TriggerSet::new(vec![
            TriggerIdentifier::new(27),
            TriggerIdentifier::new(28),
        ]),
        leaf_codec_contracts: vec![LeafCodecContractId(29), LeafCodecContractId(30)],
        entries: BTreeMap::from([(
            core_type,
            StructuralEntry::new(
                core_type,
                vec![ConstructorCodec::new(
                    EncodedConstructorId::new(core_type, 31),
                    witnessed_forms.clone(),
                    witnessed_forms[0].clone(),
                    PositionalSignature::new(vec![target]),
                )],
            ),
        )]),
    };
    assert_eq!(
        ContentHash::<StructuralTableDomain>::of_core(&payload)
            .expect("composite table identity")
            .bytes(),
        &COMPOSITE_STRUCTURAL_TABLE_LAYOUT_SIX,
        "a structural-form variant or the contextual table layout moved"
    );
}

#[test]
fn composite_structural_value_identity_is_an_absolute_layout_one_lock() {
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let identifier = names.intern(Name::new("Witness")).expect("intern witness");
    let value = StructuralValue::chosen(
        7,
        StructuralValue::Application(
            Box::new(StructuralValue::Atom(identifier)),
            Box::new(StructuralValue::Delimited(vec![
                StructuralValue::Scalar(ScalarValue::Integer(-7)),
                StructuralValue::Scalar(ScalarValue::Float(3.5)),
                StructuralValue::Scalar(ScalarValue::Text("text".to_owned())),
                StructuralValue::Scalar(ScalarValue::Boolean(true)),
                StructuralValue::Delegated(Box::new(StructuralValue::Empty)),
            ])),
        ),
    );
    assert_eq!(
        value.content_identity().expect("value identity").bytes(),
        &STRUCTURAL_VALUE_LAYOUT_ONE,
        "structural value data or its archived layout moved"
    );
}
