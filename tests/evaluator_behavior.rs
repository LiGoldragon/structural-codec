//! Evaluator behaviours: delegation constructs every wrapper level, transparent
//! delegation cycles are rejected, the string-rejoin and float leaves share one
//! control path, and a struct body decodes its disjoint Field alternatives.

use std::collections::BTreeMap;

use name_table::NameTable;
use raw_discovery::{Block, Recognizer};
use structural_codec::fixture::{DATABASE_MARKER, DOCUMENTATION, FLOAT, FixtureBuilder};
use structural_codec::{
    AddressedStructuralTable, ConstructorCodec, CoreConstructorId, PositionalSignature,
    ScalarValue, ScopedCoreTypeId, StructuralEntry, StructuralEvaluator, StructuralForm,
    StructuralRevision, StructuralValue, TableIdentityPayload,
};
use structural_codec::{CoreLayoutIdentity, RawProfileIdentity};

fn recognize_single(source: &str) -> Block {
    let document = Recognizer::standard().recognize(source).expect("recognize");
    document.root_object_at(0).expect("root").clone()
}

/// A three-deep newtype chain constructs THREE wrapper levels, terminating in the
/// rejoined string leaf — the same control path a float would take.
#[test]
fn delegation_constructs_every_wrapper_level() {
    let table = FixtureBuilder::new().build().expect("seal");
    let evaluator = StructuralEvaluator::new(&table);
    let block = recognize_single("alpha.beta.gamma");
    let mut names = NameTable::new();
    let value = evaluator
        .decode(DOCUMENTATION, &block, &mut names)
        .expect("decode Documentation");

    // Documentation → Summary → Text: Chosen wraps Delegated wraps Chosen …
    let mut depth = 0;
    let mut cursor = &value;
    loop {
        match cursor {
            StructuralValue::Chosen { payload, .. } => cursor = payload,
            StructuralValue::Delegated(inner) => {
                depth += 1;
                cursor = inner;
            }
            StructuralValue::Scalar(ScalarValue::Text(text)) => {
                assert_eq!(text, "alpha.beta.gamma", "rejoined dotted text");
                break;
            }
            other => panic!("unexpected mirror node: {other:?}"),
        }
    }
    assert_eq!(
        depth, 2,
        "two transparent delegate wrappers were constructed"
    );
}

/// The float leaf flattens `-122.3` and parses it — the same rejoin the string leaf
/// uses, differing only in the terminal parse.
#[test]
fn float_leaf_flattens_and_parses() {
    let table = FixtureBuilder::new().build().expect("seal");
    let evaluator = StructuralEvaluator::new(&table);
    let block = recognize_single("-122.3");
    let mut names = NameTable::new();
    let value = evaluator
        .decode(FLOAT, &block, &mut names)
        .expect("decode Float");
    match value {
        StructuralValue::Chosen { payload, .. } => match *payload {
            StructuralValue::Scalar(ScalarValue::Float(number)) => {
                assert!((number - (-122.3)).abs() < f64::EPSILON)
            }
            other => panic!("expected a float scalar, got {other:?}"),
        },
        other => panic!("expected a chosen constructor, got {other:?}"),
    }
}

/// A struct body decodes its two Field alternatives to their distinct constructors:
/// the bare `Integer` chooses `Field::TypeOnly` (0), and `commitSequence.Integer`
/// chooses `Field::Named` (1).
#[test]
fn struct_body_decodes_disjoint_field_alternatives() {
    let table = FixtureBuilder::new().build().expect("seal");
    let evaluator = StructuralEvaluator::new(&table);
    let block = recognize_single("DatabaseMarker.{ Integer commitSequence.Integer }");
    let mut names = NameTable::new();
    let value = evaluator
        .decode(DATABASE_MARKER, &block, &mut names)
        .expect("decode DatabaseMarker");

    // Chosen(struct) → Application(name, Delimited([field0, field1]))
    let StructuralValue::Chosen { payload, .. } = value else {
        panic!("expected a chosen struct constructor");
    };
    let StructuralValue::Application(_, body) = *payload else {
        panic!("expected the struct application");
    };
    let StructuralValue::Delimited(fields) = *body else {
        panic!("expected the delimited field body");
    };
    assert_eq!(fields.len(), 2, "two fields");

    let chosen_constructor = |field: &StructuralValue| -> u32 {
        let StructuralValue::Delegated(inner) = field else {
            panic!("each field is a delegate wrapper");
        };
        let StructuralValue::Chosen { constructor, .. } = inner.as_ref() else {
            panic!("the delegate resolves to a chosen Field constructor");
        };
        *constructor
    };
    assert_eq!(chosen_constructor(&fields[0]), 0, "bare type → TypeOnly");
    assert_eq!(chosen_constructor(&fields[1]), 1, "name.Type → Named");
}

/// A transparent delegation cycle (A delegates to B, B delegates back to A, both on
/// the same block) is rejected by the left-recursion guard.
#[test]
fn transparent_delegation_cycle_is_rejected() {
    let type_a = ScopedCoreTypeId::fixture(300);
    let type_b = ScopedCoreTypeId::fixture(301);

    let single = |core_type: ScopedCoreTypeId, target: ScopedCoreTypeId| {
        let form = StructuralForm::Delegate(target);
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                CoreConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    };

    let mut entries: BTreeMap<ScopedCoreTypeId, StructuralEntry> = BTreeMap::new();
    entries.insert(type_a, single(type_a, type_b));
    entries.insert(type_b, single(type_b, type_a));
    let payload = TableIdentityPayload {
        core_universe: structural_codec::FIXTURE_UNIVERSE,
        core_layout_identity: CoreLayoutIdentity([0u8; 32]),
        raw_profile_identity: RawProfileIdentity([1u8; 32]),
        committed_lexicon: b"cycle".to_vec(),
        leaf_codec_contracts: Vec::new(),
        entries,
    };
    let table: AddressedStructuralTable =
        AddressedStructuralTable::seal(StructuralRevision::new(1), payload).expect("seal");
    let evaluator = StructuralEvaluator::new(&table);

    let block = recognize_single("Whatever");
    let mut names = NameTable::new();
    let outcome = evaluator.decode(type_a, &block, &mut names);
    assert!(outcome.is_err(), "a transparent cycle must be rejected");
}
