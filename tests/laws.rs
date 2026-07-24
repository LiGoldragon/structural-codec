//! The conformance laws — the acceptance gate of slice one, exercised over the
//! fixture universe. Each law is stated in the design (§4.6) and checked here against
//! the trusted evaluator.

use name_table::{IdentifierNamespace, Name, NameTable};
use raw_discovery::{Block, Delimiter, Recognizer};
use structural_codec::fixture::{
    COMMIT_SEQUENCE, DOCUMENTATION, FIELD, FLOAT, FixtureBuilder, TEXT,
};
use structural_codec::{
    AddressedStructuralTable, ScalarValue, ScopedEncodedTypeId, StructuralEvaluator,
    StructuralValue,
};

fn recognize_single(source: &str) -> Block {
    let document = Recognizer::standard()
        .recognize(source)
        .expect("valid fixture text");
    document.root_object_at(0).expect("one root").clone()
}

fn standard_table() -> AddressedStructuralTable {
    FixtureBuilder::new().build().expect("seal fixture table")
}

#[test]
fn law_one_round_trip_core() {
    let table = standard_table();
    let evaluator = StructuralEvaluator::new(&table);
    let cases: &[(ScopedEncodedTypeId, &str)] = &[
        (COMMIT_SEQUENCE, "CommitSequence.{ Integer }"),
        (FIELD, "Integer"),
        (DOCUMENTATION, "alpha.beta.gamma"),
        (FLOAT, "-122.3"),
    ];
    for (expected, source) in cases {
        let block = recognize_single(source);
        let mut names = NameTable::new(IdentifierNamespace::Fixture);
        let value = evaluator
            .decode(*expected, &block, &mut names)
            .unwrap_or_else(|error| panic!("decode {source}: {error}"));
        let re_encoded = evaluator
            .encode(*expected, &value, &names)
            .unwrap_or_else(|error| panic!("encode {source}: {error}"));
        let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
        let value_again = evaluator
            .decode(*expected, &re_encoded, &mut names_again)
            .unwrap_or_else(|error| panic!("re-decode {source}: {error}"));
        assert_eq!(value, value_again, "law 1 for {source}");
    }
}

#[test]
fn law_two_round_trip_canonical() {
    let table = standard_table();
    let evaluator = StructuralEvaluator::new(&table);
    let cases: &[(ScopedEncodedTypeId, &str)] = &[
        (COMMIT_SEQUENCE, "CommitSequence.{ Integer }"),
        (FIELD, "Integer"),
        (DOCUMENTATION, "alpha.beta.gamma"),
        (FLOAT, "-122.3"),
    ];
    for (expected, source) in cases {
        let block = recognize_single(source);
        let mut names = NameTable::new(IdentifierNamespace::Fixture);
        let value = evaluator
            .decode(*expected, &block, &mut names)
            .unwrap_or_else(|error| panic!("decode {source}: {error}"));
        let encoded = evaluator
            .encode(*expected, &value, &names)
            .unwrap_or_else(|error| panic!("encode {source}: {error}"));
        assert_eq!(encoded, block, "law 2 for {source}");
    }
}

/// A scalar text leaf carries ordinary multiword content through the canonical
/// parenthesized string form and returns to the same encoded value.
#[test]
fn scalar_text_leaf_round_trips_multiword_parenthesized_string() {
    let table = standard_table();
    let evaluator = StructuralEvaluator::new(&table);
    let source = "(alpha beta)";
    let block = recognize_single(source);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode(TEXT, &block, &mut names)
        .expect("decode multiword text");
    let StructuralValue::Chosen { payload, .. } = &value else {
        panic!("text value is constructor-tagged");
    };
    assert!(matches!(
        payload.as_ref(),
        StructuralValue::Scalar(ScalarValue::Text(text)) if text == "alpha beta"
    ));

    let encoded = evaluator
        .encode(TEXT, &value, &names)
        .expect("encode multiword text");
    assert_eq!(encoded, block);
    let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
    let decoded_again = evaluator
        .decode(TEXT, &encoded, &mut names_again)
        .expect("re-decode canonical multiword text");
    assert_eq!(decoded_again, value);
}

#[test]
fn law_three_interning_atomicity() {
    let table = standard_table();
    let evaluator = StructuralEvaluator::new(&table);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    names
        .intern(Name::new("PriorName"))
        .expect("fixture allocation");
    let bytes_before = names
        .to_archive_bytes()
        .expect("archive before")
        .as_ref()
        .to_vec();
    let identity_before = names.identity().expect("identity before");
    let block = recognize_single("notADeclaration");
    let outcome = evaluator.decode(COMMIT_SEQUENCE, &block, &mut names);
    assert!(outcome.is_err(), "the decode must fail");
    let bytes_after = names
        .to_archive_bytes()
        .expect("archive after")
        .as_ref()
        .to_vec();
    let identity_after = names.identity().expect("identity after");
    assert_eq!(bytes_before, bytes_after, "archived bytes unchanged");
    assert_eq!(
        identity_before, identity_after,
        "content identity unchanged"
    );
}

#[test]
fn law_four_identity_preserving_across_revisions() {
    let table_old = FixtureBuilder::new()
        .with_newtype_delimiter(Delimiter::Brace)
        .build()
        .expect("seal old table");
    let table_new = FixtureBuilder::new()
        .with_newtype_delimiter(Delimiter::Parenthesis)
        .build()
        .expect("seal new table");
    assert_ne!(
        table_old.identity(),
        table_new.identity(),
        "the two table revisions differ"
    );
    let evaluator_old = StructuralEvaluator::new(&table_old);
    let evaluator_new = StructuralEvaluator::new(&table_new);
    let block_old = recognize_single("CommitSequence.{ Integer }");
    let block_new = recognize_single("CommitSequence.( Integer )");
    let mut names_old = NameTable::new(IdentifierNamespace::Fixture);
    let value_old = evaluator_old
        .decode(COMMIT_SEQUENCE, &block_old, &mut names_old)
        .expect("decode old text with old table");
    let mut names_new = NameTable::new(IdentifierNamespace::Fixture);
    let value_new = evaluator_new
        .decode(COMMIT_SEQUENCE, &block_new, &mut names_new)
        .expect("decode new text with new table");
    assert_eq!(value_old, value_new, "the structural value never moved");
    assert_eq!(
        value_old.content_identity().expect("identity old"),
        value_new.content_identity().expect("identity new"),
        "the value's content identity never moved"
    );
    let re_encoded = evaluator_new
        .encode(COMMIT_SEQUENCE, &value_old, &names_old)
        .expect("encode old value with new table");
    assert_eq!(re_encoded, block_new);
}
