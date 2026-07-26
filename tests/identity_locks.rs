use crate::fixture::{COMMIT_SEQUENCE, FixtureBuilder};
use crate::{StructuralEvaluator, StructuralRule, StructuralTableDomain};
use content_identity::PortableArchive;
use name_table::{IdentifierNamespace, NameTable};

#[test]
fn actual_typed_rule_records_archive_and_restore() {
    let table = FixtureBuilder::new().build().expect("table");
    let rule = table.entry(COMMIT_SEQUENCE).expect("entry").constructors()[0].encode_form();
    let bytes = rule.to_archive_bytes().expect("archive rule");
    let restored = StructuralRule::from_archive_bytes(&bytes).expect("restore rule");
    assert_eq!(&restored, rule);
}

#[test]
fn table_layout_bumps_for_sealed_discovery_rules() {
    assert_eq!(
        <StructuralTableDomain as content_identity::HashDomain>::layout_version().value(),
        8
    );
}

#[test]
fn final_combined_shape_digest_locks() {
    let table = FixtureBuilder::new().build().expect("table");
    assert_eq!(
        table.identity().to_hexadecimal(),
        "49d0cdc188467f7e1bc03fcfa3035fd2ad2c322a9c68a50b55dc2943a3dfef6e"
    );
    let profile = FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(COMMIT_SEQUENCE, "CommitSequence.{ Integer }", &mut names)
        .expect("decode");
    assert_eq!(
        value
            .content_identity()
            .expect("value identity")
            .to_hexadecimal(),
        "1d4d66ad10643647ea5014a1589922b90efc8b18f16d68778781e1f380a000ce"
    );
}
