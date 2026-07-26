use crate::fixture::{COMMIT_SEQUENCE, FixtureBuilder};
use crate::{ApplicationDelimitedHead, FieldValue, StructuralEvaluator};
use name_table::{IdentifierNamespace, NameTable};
use raw_discovery::Recognizer;

#[test]
fn shared_evaluator_decodes_and_encodes_a_typed_record_without_position_counting() {
    let table = FixtureBuilder::new().build().expect("fixture seals");
    let profile = FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile is pinned");
    let block = Recognizer::with_profile(profile.clone())
        .recognize("CommitSequence.{ Integer }")
        .expect("raw boundary discovery")
        .root_object_at(0)
        .expect("one root")
        .clone();
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode(COMMIT_SEQUENCE, &block, &mut names)
        .expect("shared decode");
    assert!(matches!(
        value.fields().value::<ApplicationDelimitedHead>(),
        Some(FieldValue::Atom(_))
    ));
    assert_eq!(
        evaluator
            .encode(COMMIT_SEQUENCE, &value, &names)
            .expect("shared encode"),
        block
    );
}
