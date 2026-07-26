use crate::fixture::{COMMIT_SEQUENCE, FixtureBuilder};
use crate::{ApplicationDelimitedHead, FieldValue, StructuralEvaluator};
use name_table::{IdentifierNamespace, NameTable};

#[test]
fn shared_evaluator_decodes_and_encodes_a_typed_record_without_position_counting() {
    let table = FixtureBuilder::new().build().expect("fixture seals");
    let evaluator = StructuralEvaluator::new(&table).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(COMMIT_SEQUENCE, "CommitSequence.{ Integer }", &mut names)
        .expect("shared decode");
    assert!(matches!(
        value.fields().value::<ApplicationDelimitedHead>(),
        Some(FieldValue::Atom(_))
    ));
    assert_eq!(
        evaluator
            .encode_text(COMMIT_SEQUENCE, &value, &names)
            .expect("shared encode"),
        "CommitSequence.{Integer}"
    );
}
