//! Direct source-text laws for the one shared evaluator.

use name_table::{IdentifierNamespace, Name, NameTable, NameTableError, NameTransaction};
use raw_discovery::{RawProfile, SealedTokenProfile};
use structural_codec::error::SingleChunkRequired;
use structural_codec::fixture::{
    CARRIED_TEXT, COMMIT_SEQUENCE, DATABASE_MARKER, DOCUMENTATION, FLOAT, FixtureBuilder,
};
use structural_codec::{
    AddressedStructuralTable, DecodeError, EncodeError, ScalarValue, ScopedEncodedTypeId,
    StructuralEvaluator, StructuralValue, Textual, TextualForm,
};

fn fixture() -> (AddressedStructuralTable, raw_discovery::SealedTokenProfile) {
    (
        FixtureBuilder::new().build().expect("fixture table seals"),
        FixtureBuilder::token_profile(),
    )
}

#[test]
fn expected_forms_drive_recursive_text_without_a_token_stream() {
    let (table, profile) = fixture();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let cases = [
        (COMMIT_SEQUENCE, "CommitSequence.{ Integer }"),
        (DATABASE_MARKER, "DatabaseMarker.{ Integer Integer }"),
        (DOCUMENTATION, "alpha.beta.gamma"),
        (FLOAT, "-122.3"),
    ];

    for (expected, source) in cases {
        let mut names = NameTable::new(IdentifierNamespace::Fixture);
        let value = evaluator
            .decode_text(expected, source, &mut names)
            .unwrap_or_else(|error| panic!("decode {source}: {error}"));
        let canonical = evaluator
            .encode_text(expected, &value, &names)
            .unwrap_or_else(|error| panic!("encode {source}: {error}"));
        let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
        let value_again = evaluator
            .decode_text(expected, &canonical, &mut names_again)
            .unwrap_or_else(|error| panic!("re-decode {canonical}: {error}"));
        assert_eq!(value, value_again, "encoded value round-trip for {source}");
        assert_eq!(
            evaluator
                .encode_text(expected, &value_again, &names_again)
                .expect("second canonical encode"),
            canonical,
            "canonical text is idempotent for {source}"
        );
    }

    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(
            CARRIED_TEXT,
            r#"(|macro } . can contain \|) and \\ literally|)"#,
            &mut names,
        )
        .expect("carrier body decodes without recursively activating its glyphs");
    let canonical = evaluator
        .encode_text(CARRIED_TEXT, &value, &names)
        .expect("carrier body encodes through the profile");
    assert_eq!(
        canonical,
        r#"(|macro } . can contain \|) and \\ literally|)"#
    );
    let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
    assert_eq!(
        evaluator
            .decode_text(CARRIED_TEXT, &canonical, &mut names_again)
            .expect("canonical carrier re-decodes"),
        value
    );
}

#[test]
fn canonical_trivia_spelling_comes_from_the_profile() {
    let (table, profile) = fixture();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(
            DATABASE_MARKER,
            "DatabaseMarker. { Integer\n;; accepted trivia\nInteger }",
            &mut names,
        )
        .expect("profile trivia is accepted");
    assert_eq!(
        evaluator
            .encode_text(DATABASE_MARKER, &value, &names)
            .expect("canonical profile-driven emission"),
        "DatabaseMarker.{Integer Integer}"
    );
}

#[test]
fn operator_triggers_are_inactive_in_scalar_positions() {
    let (table, profile) = fixture();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(DOCUMENTATION, "alpha.beta.gamma", &mut names)
        .expect("text expected here, so periods stay scalar text");

    let StructuralValue::Chosen { payload, .. } = value else {
        panic!("documentation is constructor-tagged");
    };
    let StructuralValue::Delegated(summary) = payload.as_ref() else {
        panic!("documentation delegates to summary");
    };
    let StructuralValue::Chosen {
        payload: summary_payload,
        ..
    } = summary.as_ref()
    else {
        panic!("summary is constructor-tagged");
    };
    let StructuralValue::Delegated(text) = summary_payload.as_ref() else {
        panic!("summary delegates to text");
    };
    let StructuralValue::Chosen {
        payload: text_payload,
        ..
    } = text.as_ref()
    else {
        panic!("text is constructor-tagged");
    };
    assert!(matches!(
        text_payload.as_ref(),
        StructuralValue::Scalar(ScalarValue::Text(text)) if text == "alpha.beta.gamma"
    ));
}

#[test]
fn evaluator_refuses_a_profile_other_than_the_table_pin() {
    let (table, _) = fixture();
    let other = RawProfile::nomos_extended().seal().expect("Nomos profile");
    let evaluator = StructuralEvaluator::with_profile(&table, &other);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(matches!(
        evaluator.decode_text(FLOAT, "1.5", &mut names),
        Err(DecodeError::TokenProfileIdentityMismatch)
    ));
}

#[test]
fn failed_direct_decode_leaves_the_nametree_byte_identical() {
    let (table, profile) = fixture();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    names
        .intern(Name::new("PriorName"))
        .expect("fixture prior name");
    let before = names.to_archive_bytes().expect("archive before");
    let identity_before = names.identity().expect("identity before");

    assert!(
        evaluator
            .decode_text(COMMIT_SEQUENCE, "notADeclaration", &mut names)
            .is_err()
    );

    assert_eq!(
        before.as_ref(),
        names.to_archive_bytes().expect("archive after").as_ref()
    );
    assert_eq!(identity_before, names.identity().expect("identity after"));
}

struct FailingReifier {
    table: AddressedStructuralTable,
    profile: SealedTokenProfile,
}

#[derive(Debug, thiserror::Error)]
enum ReifyError {
    #[error("deliberate reify refusal")]
    Refused,
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error(transparent)]
    Names(#[from] NameTableError),
    #[error(transparent)]
    Chunks(#[from] SingleChunkRequired),
}

impl Textual for FailingReifier {
    type Encoded = ();
    type Language = ();
    type Error = ReifyError;

    fn structuretree(&self) -> &AddressedStructuralTable {
        &self.table
    }

    fn token_profile(&self) -> &SealedTokenProfile {
        &self.profile
    }

    fn missing_root_object(&self) -> Self::Error {
        ReifyError::Refused
    }

    fn reify(
        &self,
        _expected: ScopedEncodedTypeId,
        _mirror: &StructuralValue,
        names: &mut NameTransaction<'_>,
    ) -> Result<Self::Encoded, Self::Error> {
        names.intern(Name::new("MustRollback"))?;
        Err(ReifyError::Refused)
    }

    fn reflect(
        &self,
        _expected: ScopedEncodedTypeId,
        _encoded: &Self::Encoded,
        _names: &NameTable,
    ) -> Result<StructuralValue, Self::Error> {
        Ok(StructuralValue::Empty)
    }
}

#[test]
fn failed_reify_shares_the_decode_name_transaction() {
    let mouth = FailingReifier {
        table: FixtureBuilder::new().build().expect("fixture table"),
        profile: FixtureBuilder::token_profile(),
    };
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    names
        .intern(Name::new("PriorName"))
        .expect("fixture prior name");
    let before = names.to_archive_bytes().expect("archive before");
    let identity_before = names.identity().expect("identity before");

    assert!(matches!(
        mouth.unview(FLOAT, &TextualForm::single("1.5".to_owned()), &mut names),
        Err(ReifyError::Refused)
    ));

    assert_eq!(
        before.as_ref(),
        names.to_archive_bytes().expect("archive after").as_ref()
    );
    assert_eq!(identity_before, names.identity().expect("identity after"));
}
