//! Structural capability laws for the table-owned two-pass route.

use crate::StructuralEvaluator;
use crate::fixture::{COMMIT_SEQUENCE, FixtureBuilder};
use legacy_name_table::{IdentifierNamespace, NameTable};

#[test]
fn law_one_value_round_trip_uses_the_source_tree_route() {
    let table = FixtureBuilder::new().build().expect("brace table");
    let evaluator = StructuralEvaluator::new(&table).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(COMMIT_SEQUENCE, "CommitSequence.{ Integer }", &mut names)
        .expect("source decode");
    let text = evaluator
        .encode_text(COMMIT_SEQUENCE, &value, &names)
        .expect("source encode");
    let mut round_trip_names = NameTable::new(IdentifierNamespace::Fixture);
    let round_trip = evaluator
        .decode_text(COMMIT_SEQUENCE, &text, &mut round_trip_names)
        .expect("canonical source decode");

    assert_eq!(round_trip, value);
}

#[test]
fn law_two_decode_then_encode_is_canonical_text() {
    let table = FixtureBuilder::new().build().expect("brace table");
    let evaluator = StructuralEvaluator::new(&table).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(COMMIT_SEQUENCE, " CommitSequence.{ Integer } ", &mut names)
        .expect("source decode");

    assert_eq!(
        evaluator
            .encode_text(COMMIT_SEQUENCE, &value, &names)
            .expect("canonical source encode"),
        "CommitSequence.{Integer}"
    );
}

#[test]
fn law_three_discovery_and_typed_failures_preserve_name_table_bytes_and_identity() {
    let table = FixtureBuilder::new().build().expect("brace table");
    let evaluator = StructuralEvaluator::new(&table).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let before_bytes = names.to_archive_bytes().expect("archive before");
    let before_identity = names.identity().expect("identity before");

    for source in [
        "CommitSequence.{ (broken ] }",
        "CommitSequence.{ lower_case_item }",
    ] {
        assert!(
            evaluator
                .decode_text(COMMIT_SEQUENCE, source, &mut names)
                .is_err()
        );
        assert_eq!(
            names.to_archive_bytes().expect("archive after").as_ref(),
            before_bytes.as_ref()
        );
        assert_eq!(names.identity().expect("identity after"), before_identity);
    }
}

#[test]
fn law_four_profile_boundary_respelling_moves_table_not_value_identity() {
    let left = FixtureBuilder::new().build().expect("brace table");
    let right = FixtureBuilder::new()
        .with_newtype_boundary(crate::fixture::PARENTHESIS_BOUNDARY)
        .build()
        .expect("parenthesis table");
    assert_ne!(left.identity(), right.identity());

    let left_evaluator = StructuralEvaluator::new(&left).expect("left evaluator");
    let right_evaluator = StructuralEvaluator::new(&right).expect("right evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = left_evaluator
        .decode_text(COMMIT_SEQUENCE, "CommitSequence.{ Integer }", &mut names)
        .expect("left decode");
    let content = value.content_identity().expect("value identity");
    let text = right_evaluator
        .encode_text(COMMIT_SEQUENCE, &value, &names)
        .expect("right encode");
    assert_eq!(text, "CommitSequence.(Integer)");
    let mut again_names = NameTable::new(IdentifierNamespace::Fixture);
    let again = right_evaluator
        .decode_text(COMMIT_SEQUENCE, &text, &mut again_names)
        .expect("right decode");

    assert_eq!(
        again.content_identity().expect("round-trip identity"),
        content
    );
}
