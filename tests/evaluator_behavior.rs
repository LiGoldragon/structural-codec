//! Evaluator behaviours: delegation constructs every wrapper level, transparent
//! delegation cycles are rejected, the string-rejoin and float leaves share one
//! control path, and a struct body decodes positional elided-name Fields.

use std::collections::BTreeMap;

use name_table::{IdentifierNamespace, NameTable};
use raw_discovery::{Block, Recognizer, TriggerSet};
use structural_codec::fixture::{
    COMMENT_TRIVIA, DATABASE_MARKER, DOCUMENTATION, FLOAT, FixtureBuilder, WHITESPACE_TRIVIA,
};
use structural_codec::{
    AddressedStructuralTable, ConstructorCodec, EncodedConstructorId, PositionalSignature,
    ScalarValue, ScopedEncodedTypeId, StructuralEntry, StructuralEvaluator, StructuralForm,
    StructuralValue, TableIdentityPayload,
};
use structural_codec::{EncodedLayoutIdentity, RawProfileIdentity};

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
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
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
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
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

/// A struct body decodes each bare type as the one elided-name Field constructor,
/// preserving same-typed fields by position rather than stored field names.
#[test]
fn struct_body_decodes_positional_elided_fields() {
    let table = FixtureBuilder::new().build().expect("seal");
    let evaluator = StructuralEvaluator::new(&table);
    let block = recognize_single("DatabaseMarker.{ Integer Integer }");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
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
    assert_eq!(fields.len(), 2, "two positional fields");

    for field in &fields {
        let StructuralValue::Delegated(inner) = field else {
            panic!("each field is a delegate wrapper");
        };
        let StructuralValue::Chosen { constructor, .. } = inner.as_ref() else {
            panic!("the delegate resolves to a chosen Field constructor");
        };
        assert_eq!(*constructor, 0, "each bare field uses the sole constructor");
    }
}

/// The fixture Field entry admits one elided-name constructor and rejects the banned
/// `name.Type` surface through the evaluator's ordinary typed no-alternative path.
#[test]
fn field_entry_rejects_the_banned_named_surface() {
    let table = FixtureBuilder::new().build().expect("seal");
    let evaluator = StructuralEvaluator::new(&table);
    let field = table
        .entry(structural_codec::fixture::FIELD)
        .expect("field entry");
    assert_eq!(
        field.constructors.len(),
        1,
        "only the elided constructor remains"
    );

    let named = recognize_single("stateDigest.Integer");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(
        evaluator
            .decode(structural_codec::fixture::FIELD, &named, &mut names)
            .is_err(),
        "the banned named-field spelling has no accepted constructor"
    );
}

/// A transparent delegation cycle (A delegates to B, B delegates back to A, both on
/// the same block) is rejected by the left-recursion guard.
#[test]
fn transparent_delegation_cycle_is_rejected() {
    let type_a = ScopedEncodedTypeId::fixture(300);
    let type_b = ScopedEncodedTypeId::fixture(301);

    let single = |core_type: ScopedEncodedTypeId, target: ScopedEncodedTypeId| {
        let form = StructuralForm::delegate(target);
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )],
        )
    };

    let mut entries: BTreeMap<ScopedEncodedTypeId, StructuralEntry> = BTreeMap::new();
    entries.insert(type_a, single(type_a, type_b));
    entries.insert(type_b, single(type_b, type_a));
    let profile = FixtureBuilder::token_profile();
    let payload = TableIdentityPayload {
        core_universe: structural_codec::FIXTURE_UNIVERSE,
        core_layout_identity: EncodedLayoutIdentity([0u8; 32]),
        raw_profile_identity: RawProfileIdentity::from_profile(&profile),
        trivia_triggers: TriggerSet::new(vec![WHITESPACE_TRIVIA, COMMENT_TRIVIA]),
        leaf_codec_contracts: Vec::new(),
        entries,
    };
    let table: AddressedStructuralTable =
        AddressedStructuralTable::seal(payload, &profile).expect("seal");
    let evaluator = StructuralEvaluator::new(&table);

    let block = recognize_single("Whatever");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let outcome = evaluator.decode(type_a, &block, &mut names);
    assert!(outcome.is_err(), "a transparent cycle must be rejected");
}
