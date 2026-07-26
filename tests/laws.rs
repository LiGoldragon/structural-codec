use crate::StructuralEvaluator;
use crate::fixture::{COMMIT_SEQUENCE, FixtureBuilder};
use name_table::{IdentifierNamespace, NameTable};

#[test]
fn typed_record_round_trip_is_identity_stable_across_a_boundary_respelling() {
    let left = FixtureBuilder::new().build().expect("brace table");
    let right = FixtureBuilder::new()
        .with_newtype_boundary(crate::fixture::PARENTHESIS_BOUNDARY)
        .build()
        .expect("parenthesis table");
    assert_ne!(left.identity(), right.identity());
    let profile = FixtureBuilder::token_profile();
    let left_evaluator = StructuralEvaluator::with_profile(&left, &profile).expect("left profile");
    let right_evaluator =
        StructuralEvaluator::with_profile(&right, &profile).expect("right profile");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = left_evaluator
        .decode_text(COMMIT_SEQUENCE, "CommitSequence.{ Integer }", &mut names)
        .expect("left decode");
    let content = value.content_identity().expect("mirror identity");
    let text = right_evaluator
        .encode_text(COMMIT_SEQUENCE, &value, &names)
        .expect("right encode");
    assert_eq!(text, "CommitSequence.(Integer)");
    let mut again_names = NameTable::new(IdentifierNamespace::Fixture);
    let again = right_evaluator
        .decode_text(COMMIT_SEQUENCE, &text, &mut again_names)
        .expect("right decode");
    assert_eq!(again.content_identity().expect("mirror identity"), content);
}
