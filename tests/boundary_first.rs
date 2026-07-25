//! Direct evaluator witnesses for outside-in boundary partitioning.

use std::collections::BTreeMap;

use name_table::{IdentifierNamespace, Name, NameTable};
use raw_discovery::{
    Delimiter, ProfileRevision, RawProfile, SealedTokenProfile, TokenProfileData,
    TokenProfileError, Trigger, TriggerDefinition, TriggerIdentifier, TriggerSet,
};
use structural_codec::{
    AddressedStructuralTable, CarrierLeaf, ConstructorCodec, DecodeError, EncodedConstructorId,
    EncodedLayoutIdentity, LeafCodec, LeafForm, PositionalSignature, RawProfileIdentity,
    ScalarLeaf, ScopedEncodedTypeId, SequenceForm, StructuralEntry, StructuralEvaluator,
    StructuralForm, TableIdentityPayload,
};

const PARENTHESIS: TriggerIdentifier = TriggerIdentifier::new(0);
const SQUARE: TriggerIdentifier = TriggerIdentifier::new(1);
const BRACE: TriggerIdentifier = TriggerIdentifier::new(2);
const APPLICATION: TriggerIdentifier = TriggerIdentifier::new(3);
const PIPE_TEXT: TriggerIdentifier = TriggerIdentifier::new(4);
const WHITESPACE: TriggerIdentifier = TriggerIdentifier::new(5);
const COMMENT: TriggerIdentifier = TriggerIdentifier::new(6);

const OUTER: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(500);
const INNER: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture(501);

fn delimited(
    boundary: TriggerIdentifier,
    delimiter: Delimiter,
    sequence: SequenceForm,
) -> StructuralForm {
    StructuralForm::Delimited {
        boundary,
        delimiter,
        sequence,
    }
}

fn entry(core_type: ScopedEncodedTypeId, form: StructuralForm) -> StructuralEntry {
    StructuralEntry::new(
        core_type,
        vec![ConstructorCodec::new(
            EncodedConstructorId::new(core_type, 0),
            vec![form.clone()],
            form,
            PositionalSignature::default(),
        )],
    )
}

fn table(
    profile: &SealedTokenProfile,
    trivia: TriggerSet,
    entries: impl IntoIterator<Item = StructuralEntry>,
) -> AddressedStructuralTable {
    AddressedStructuralTable::seal(
        TableIdentityPayload {
            core_universe: structural_codec::FIXTURE_UNIVERSE,
            core_layout_identity: EncodedLayoutIdentity([0x51; 32]),
            raw_profile_identity: RawProfileIdentity::from_profile(profile),
            trivia_triggers: trivia,
            leaf_codec_contracts: Vec::new(),
            entries: entries
                .into_iter()
                .map(|entry| (entry.core_type, entry))
                .collect::<BTreeMap<_, _>>(),
        },
        profile,
    )
    .expect("boundary fixture table seals")
}

fn standard_table(
    entries: impl IntoIterator<Item = StructuralEntry>,
) -> (AddressedStructuralTable, SealedTokenProfile) {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let table = table(
        &profile,
        TriggerSet::new(vec![WHITESPACE, COMMENT]),
        entries,
    );
    (table, profile)
}

fn pascal_group(boundary: TriggerIdentifier, delimiter: Delimiter) -> StructuralForm {
    delimited(
        boundary,
        delimiter,
        SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
    )
}

#[test]
fn missing_outer_close_wins_before_an_invalid_child_is_interpreted() {
    let (table, profile) = standard_table([entry(
        OUTER,
        delimited(
            BRACE,
            Delimiter::Brace,
            SequenceForm::Product(vec![pascal_group(SQUARE, Delimiter::SquareBracket)]),
        ),
    )]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);

    assert!(matches!(
        evaluator.decode_text(OUTER, "{[lower]", &mut names),
        Err(DecodeError::TokenProfile(
            TokenProfileError::UnclosedBoundary {
                identifier,
                byte_offset: 0,
            }
        )) if identifier == BRACE
    ));
}

#[test]
fn mismatched_nested_close_is_committed_and_never_masked_as_no_alternative() {
    let (table, profile) = standard_table([entry(
        OUTER,
        delimited(
            BRACE,
            Delimiter::Brace,
            SequenceForm::Product(vec![pascal_group(SQUARE, Delimiter::SquareBracket)]),
        ),
    )]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);

    assert!(matches!(
        evaluator.decode_text(OUTER, "{[Inner}", &mut names),
        Err(DecodeError::TokenProfile(
            TokenProfileError::MismatchedBoundary {
                expected,
                found,
                ..
            }
        )) if expected == SQUARE && found == BRACE
    ));
}

#[test]
fn failed_child_is_tied_to_its_discovered_bound_and_cannot_read_trailing_text() {
    let (table, profile) = standard_table([entry(OUTER, pascal_group(BRACE, Delimiter::Brace))]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    names.intern(Name::new("Prior")).expect("prior name");
    let before = names.to_archive_bytes().expect("before");
    let identity_before = names.identity().expect("identity before");

    let error = evaluator
        .decode_text(OUTER, "{lower}TrailingValid", &mut names)
        .expect_err("camel child is rejected inside the completed brace bound");
    assert!(matches!(
        error,
        DecodeError::BoundedInterior {
            boundary,
            start: 1,
            end: 6,
            source,
        } if boundary == BRACE && matches!(*source, DecodeError::CaseMismatch)
    ));
    assert_eq!(
        before.as_ref(),
        names.to_archive_bytes().expect("after").as_ref()
    );
    assert_eq!(identity_before, names.identity().expect("identity after"));
}

#[test]
fn malformed_boundary_result_is_independent_of_decode_alternative_order() {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let brace = pascal_group(BRACE, Delimiter::Brace);
    let square = pascal_group(SQUARE, Delimiter::SquareBracket);
    let build = |forms: Vec<StructuralForm>| {
        let alternative_entry = StructuralEntry::new(
            OUTER,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(OUTER, 0),
                forms,
                brace.clone(),
                PositionalSignature::default(),
            )],
        );
        table(
            &profile,
            TriggerSet::new(vec![WHITESPACE, COMMENT]),
            [alternative_entry],
        )
    };

    for candidate in [
        build(vec![brace.clone(), square.clone()]),
        build(vec![square, brace.clone()]),
    ] {
        let evaluator = StructuralEvaluator::with_profile(&candidate, &profile);
        let mut names = NameTable::new(IdentifierNamespace::Fixture);
        assert!(matches!(
            evaluator.decode_text(OUTER, "{Inner", &mut names),
            Err(DecodeError::TokenProfile(
                TokenProfileError::UnclosedBoundary { identifier, .. }
            )) if identifier == BRACE
        ));
    }
}

#[test]
fn carriers_hide_closers_and_escaped_carrier_closes_during_partitioning() {
    let carrier = StructuralForm::Leaf(LeafForm::with_trigger(
        LeafCodec::Carrier(CarrierLeaf::PipeText),
        PIPE_TEXT,
    ));
    let (table, profile) = standard_table([entry(
        OUTER,
        delimited(
            BRACE,
            Delimiter::Brace,
            SequenceForm::Product(vec![carrier]),
        ),
    )]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let source = r#"{(| body } ] \|) remains carried |)}"#;
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, source, &mut names)
        .expect("carrier shields every apparent boundary");
    let canonical = evaluator
        .encode_text(OUTER, &value, &names)
        .expect("canonical carrier emission");
    let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
    let again = evaluator
        .decode_text(OUTER, &canonical, &mut names_again)
        .expect("canonical text re-decodes");
    assert_eq!(again, value);
    assert_eq!(
        evaluator
            .encode_text(OUTER, &again, &names_again)
            .expect("canonical idempotence"),
        canonical
    );
}

#[test]
fn an_unclosed_active_carrier_is_a_committed_typed_failure() {
    let carrier = StructuralForm::Leaf(LeafForm::with_trigger(
        LeafCodec::Carrier(CarrierLeaf::PipeText),
        PIPE_TEXT,
    ));
    let (table, profile) = standard_table([entry(
        OUTER,
        delimited(
            BRACE,
            Delimiter::Brace,
            SequenceForm::Product(vec![carrier]),
        ),
    )]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);

    assert!(matches!(
        evaluator.decode_text(OUTER, "{(|never closes}", &mut names),
        Err(DecodeError::TokenProfile(
            TokenProfileError::UnclosedCarrier {
                identifier,
                byte_offset: 1,
            }
        )) if identifier == PIPE_TEXT
    ));
}

#[test]
fn mixed_nested_groups_round_trip_canonically_through_bounded_recursion() {
    let form = delimited(
        BRACE,
        Delimiter::Brace,
        SequenceForm::Product(vec![delimited(
            SQUARE,
            Delimiter::SquareBracket,
            SequenceForm::Product(vec![pascal_group(PARENTHESIS, Delimiter::Parenthesis)]),
        )]),
    );
    let (table, profile) = standard_table([entry(OUTER, form)]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, "{ [ ( Inner ) ] }", &mut names)
        .expect("mixed nesting");
    let canonical = evaluator
        .encode_text(OUTER, &value, &names)
        .expect("canonical mixed nesting");
    assert_eq!(canonical, "{[(Inner)]}");
    let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
    assert_eq!(
        evaluator
            .decode_text(OUTER, &canonical, &mut names_again)
            .expect("canonical mixed nesting re-decodes"),
        value
    );
}

#[test]
fn adjacent_angle_closes_are_two_recursive_boundaries_not_a_shift_token() {
    let angle = TriggerIdentifier::new(10);
    let shift = TriggerIdentifier::new(11);
    let whitespace = TriggerIdentifier::new(12);
    let profile = TokenProfileData::new(
        ProfileRevision::new(17),
        vec![
            TriggerDefinition {
                identifier: angle,
                trigger: Trigger::Boundary {
                    opening: "<".to_owned(),
                    closing: ">".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: shift,
                trigger: Trigger::Punctuation {
                    glyph: ">>".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: whitespace,
                trigger: Trigger::Whitespace {
                    canonical_spelling: " ".to_owned(),
                },
            },
        ],
        TriggerSet::new(vec![angle, shift, whitespace]),
        String::new(),
    )
    .seal()
    .expect("angle profile");
    let inner = delimited(
        angle,
        Delimiter::Parenthesis,
        SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
    );
    let outer = delimited(
        angle,
        Delimiter::Parenthesis,
        SequenceForm::Product(vec![StructuralForm::delegate(INNER)]),
    );
    let table = table(
        &profile,
        TriggerSet::new(vec![whitespace]),
        [entry(OUTER, outer), entry(INNER, inner)],
    );
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, "< < T > >", &mut names)
        .expect("adjacent recursive closes");
    let canonical = evaluator
        .encode_text(OUTER, &value, &names)
        .expect("canonical adjacent closes");
    assert_eq!(canonical, "<<T>>");
    let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
    assert_eq!(
        evaluator
            .decode_text(OUTER, &canonical, &mut names_again)
            .expect("adjacent closes re-decode"),
        value
    );
}

#[test]
fn application_operator_is_sought_only_after_its_bounded_head_completes() {
    let head = delimited(
        PARENTHESIS,
        Delimiter::Parenthesis,
        SequenceForm::Product(vec![StructuralForm::Leaf(LeafForm::scalar(
            ScalarLeaf::Text,
        ))]),
    );
    let form = StructuralForm::application(APPLICATION, head, StructuralForm::pascal_atom());
    let (table, profile) = standard_table([entry(OUTER, form)]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, "(inside.period).Outside", &mut names)
        .expect("period inside the bounded head remains scalar text");
    let canonical = evaluator
        .encode_text(OUTER, &value, &names)
        .expect("application canonical text");
    assert_eq!(canonical, "(inside.period).Outside");
}
