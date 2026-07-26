use crate::fixture::{DOCUMENTATION, FixtureBuilder};
use crate::{
    DecodeError, EncodeError, EncodedForm, SingleChunkRequired, StructuralEvaluator,
    StructuralValue, Textual,
};
use name_table::{IdentifierNamespace, NameTable, NameTableError, NameTransaction};

struct FixtureLanguage;
struct FixtureEncoded;

impl EncodedForm for FixtureEncoded {
    type Language = FixtureLanguage;
}

struct ProfileBoundTextual {
    table: crate::AddressedStructuralTable,
}

#[derive(Debug, thiserror::Error)]
enum TextualTestError {
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error(transparent)]
    Names(#[from] NameTableError),
    #[error(transparent)]
    Chunks(#[from] SingleChunkRequired),
    #[error("test-only reify/reflect refusal")]
    Refused,
}

impl Textual for ProfileBoundTextual {
    type Encoded = FixtureEncoded;
    type Language = FixtureLanguage;
    type Error = TextualTestError;

    fn structuretree(&self) -> &crate::AddressedStructuralTable {
        &self.table
    }

    fn missing_root_object(&self) -> Self::Error {
        TextualTestError::Refused
    }

    fn reify(
        &self,
        _expected: crate::ScopedEncodedTypeId,
        _mirror: &StructuralValue,
        _names: &mut NameTransaction<'_>,
    ) -> Result<Self::Encoded, Self::Error> {
        Err(TextualTestError::Refused)
    }

    fn reflect(
        &self,
        _expected: crate::ScopedEncodedTypeId,
        _encoded: &Self::Encoded,
        _names: &NameTable,
    ) -> Result<StructuralValue, Self::Error> {
        Err(TextualTestError::Refused)
    }
}

#[test]
fn text_entry_uses_the_same_profile_bound_record_evaluator() {
    let table = FixtureBuilder::new().build().expect("table");
    let profile = FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(DOCUMENTATION, "alpha.beta", &mut names)
        .expect("decode");
    assert_eq!(
        evaluator
            .encode_text(DOCUMENTATION, &value, &names)
            .expect("encode"),
        "alpha.beta"
    );
    assert!(matches!(
        evaluator.decode_text(DOCUMENTATION, "", &mut names),
        Err(DecodeError::RootObjectCount)
    ));
}

#[test]
fn carrier_text_is_opaque_to_boundary_discovery_and_round_trips_directly() {
    let table = FixtureBuilder::new().build().expect("table");
    let evaluator = StructuralEvaluator::new(&table).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let source = "(| } ] ) . ;; carrier content |)";
    let value = evaluator
        .decode_text(DOCUMENTATION, source, &mut names)
        .expect("opaque carrier decode");

    assert_eq!(
        evaluator
            .encode_text(DOCUMENTATION, &value, &names)
            .expect("direct carrier encode"),
        source
    );
}

#[test]
fn supplied_profile_mismatch_is_refused_before_textual_evaluation() {
    let mouth = ProfileBoundTextual {
        table: FixtureBuilder::new().build().expect("table"),
    };
    assert!(matches!(
        StructuralEvaluator::with_profile(
            &mouth.table,
            &raw_discovery::RawProfile::nomos_extended()
                .seal()
                .expect("alternate profile"),
        ),
        Err(DecodeError::TokenProfileIdentityMismatch)
    ));
}
