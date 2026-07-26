use crate::fixture::{COMMIT_SEQUENCE, FixtureBuilder};
use crate::{DecodeError, StructuralEvaluator};
use name_table::{IdentifierNamespace, NameTable};

#[test]
fn nested_boundary_is_discovered_before_the_typed_item_is_evaluated() {
    let table = FixtureBuilder::new().build().expect("table");
    let profile = FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(matches!(
        evaluator.decode_text(
            COMMIT_SEQUENCE,
            "CommitSequence.{ (not-an-integer) }",
            &mut names
        ),
        Err(DecodeError::NoAlternative { .. })
    ));
}
