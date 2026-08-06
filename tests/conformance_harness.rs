use crate::fixture::{COMMIT_SEQUENCE, FIELD, FixtureBuilder};
use crate::{
    ConformanceHarness, DecodeError, EncodeError, EncodedConstructorId, FieldValue, GeneratedCodec,
    StructuralValue, UnaryRoot,
};
use legacy_name_table::{Identifier, IdentifierNamespace, Name, NameResolver, NameTable};
use crate::{Atom, AtomCase};

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndependentlyAuthoredPascal {
    name: Identifier,
}

impl GeneratedCodec for IndependentlyAuthoredPascal {
    const CORE_TYPE: crate::ScopedEncodedTypeId = FIELD;

    fn decode(source: &str, names: &mut NameTable) -> Result<Self, DecodeError> {
        let atom = Atom::new(source);
        if AtomCase::of(&atom) != AtomCase::PascalCase {
            return Err(DecodeError::CaseMismatch);
        }
        Ok(Self {
            name: names.intern(Name::new(source))?,
        })
    }

    fn encode<Resolver: NameResolver>(&self, resolver: &Resolver) -> Result<String, EncodeError> {
        Ok(resolver.resolve(self.name)?.as_str().to_owned())
    }

    fn to_structural(&self) -> StructuralValue {
        let mut record = StructuralValue::record(EncodedConstructorId::under(FIELD, 1));
        record
            .insert::<UnaryRoot>(FieldValue::Atom(self.name))
            .expect("the fixture unary role is valid");
        record.finish()
    }
}

#[test]
fn conformance_harness_requires_the_same_sealed_profile_as_direct_evaluation() {
    let table = FixtureBuilder::new().build().expect("table");
    let profile = FixtureBuilder::token_profile();
    let _harness =
        ConformanceHarness::new(&table, &profile, COMMIT_SEQUENCE).expect("profile bound harness");
}

#[test]
fn independently_authored_codec_matches_table_evaluation() {
    let table = FixtureBuilder::new().build().expect("table");
    let profile = FixtureBuilder::token_profile();
    let harness = ConformanceHarness::new(&table, &profile, FIELD).expect("conformance harness");
    let fixtures = [
        "Integer",
        "StateDigest",
        "commitSequence",
        "Foo.Bar",
        "{ Name }",
        "",
    ]
    .map(str::to_owned);

    harness
        .check::<IndependentlyAuthoredPascal>(&fixtures)
        .expect("independent codec and table evaluator agree");

    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(
        IndependentlyAuthoredPascal::decode("commitSequence", &mut names).is_err(),
        "the refusal fixture exercises the independent error path"
    );
}
