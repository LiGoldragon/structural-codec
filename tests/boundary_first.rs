//! Direct evaluator witnesses for outside-in boundary partitioning.

use std::collections::BTreeMap;

use name_table::{Identifier, IdentifierNamespace, Name, NameTable};
use raw_discovery::{
    Delimiter, ProfileRevision, RawProfile, SealedTokenProfile, TokenProfileData,
    TokenProfileError, Trigger, TriggerDefinition, TriggerIdentifier, TriggerSet,
};
use structural_codec::{
    AddressedStructuralTable, CarrierLeaf, ConstructorCodec, DecodeError, EncodedConstructorId,
    EncodedLayoutIdentity, LeafCodec, LeafForm, PositionalSignature, RawProfileIdentity,
    ScalarLeaf, ScopedEncodedTypeId, SequenceForm, StructuralEntry, StructuralEvaluator,
    StructuralForm, StructuralValue, TableIdentityPayload,
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

fn entry_with_alternatives(
    core_type: ScopedEncodedTypeId,
    decode_forms: Vec<StructuralForm>,
    encode_form: StructuralForm,
) -> StructuralEntry {
    StructuralEntry::new(
        core_type,
        vec![ConstructorCodec::new(
            EncodedConstructorId::new(core_type, 0),
            decode_forms,
            encode_form,
            PositionalSignature::default(),
        )],
    )
}

fn literal_application(literal: Identifier, payload: StructuralForm) -> StructuralForm {
    StructuralForm::application(APPLICATION, StructuralForm::Literal(literal), payload)
}

fn one_name(names: &mut NameTable, spelling: &str) -> Identifier {
    names.intern(Name::new(spelling)).expect("fixture literal")
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

#[test]
fn sibling_triggers_activate_only_when_the_sequence_reaches_that_position() {
    let form = delimited(
        BRACE,
        Delimiter::Brace,
        SequenceForm::Product(vec![
            StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Text)),
            pascal_group(SQUARE, Delimiter::SquareBracket),
        ]),
    );
    let (table, profile) = standard_table([entry(OUTER, form)]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let source = "{alpha[beta [Inner]}";
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, source, &mut names)
        .expect("the later square trigger is inactive in the first text position");
    let canonical = evaluator
        .encode_text(OUTER, &value, &names)
        .expect("canonical sibling-state emission");
    assert_eq!(canonical, source);
    let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
    let again = evaluator
        .decode_text(OUTER, &canonical, &mut names_again)
        .expect("canonical sibling-state text re-decodes");
    assert_eq!(
        evaluator
            .encode_text(OUTER, &again, &names_again)
            .expect("canonical sibling-state text is idempotent"),
        canonical
    );
}

#[test]
fn sibling_state_is_independent_of_top_level_alternative_order() {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let sibling_form = delimited(
        BRACE,
        Delimiter::Brace,
        SequenceForm::Product(vec![
            StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Text)),
            pascal_group(SQUARE, Delimiter::SquareBracket),
        ]),
    );
    let other = pascal_group(PARENTHESIS, Delimiter::Parenthesis);
    let build = |decode_forms: Vec<StructuralForm>| {
        table(
            &profile,
            TriggerSet::new(vec![WHITESPACE, COMMENT]),
            [StructuralEntry::new(
                OUTER,
                vec![ConstructorCodec::new(
                    EncodedConstructorId::new(OUTER, 0),
                    decode_forms,
                    sibling_form.clone(),
                    PositionalSignature::default(),
                )],
            )],
        )
    };

    for candidate in [
        build(vec![sibling_form.clone(), other.clone()]),
        build(vec![other, sibling_form.clone()]),
    ] {
        let evaluator = StructuralEvaluator::with_profile(&candidate, &profile);
        let mut names = NameTable::new(IdentifierNamespace::Fixture);
        let value = evaluator
            .decode_text(OUTER, "{alpha[beta [Inner]}", &mut names)
            .expect("alternative order cannot activate a later sibling early");
        assert_eq!(
            evaluator
                .encode_text(OUTER, &value, &names)
                .expect("alternative-order canonical text"),
            "{alpha[beta [Inner]}"
        );
    }
}

#[test]
fn several_sibling_trigger_sets_remain_position_local() {
    let form = delimited(
        BRACE,
        Delimiter::Brace,
        SequenceForm::Product(vec![
            StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Text)),
            pascal_group(SQUARE, Delimiter::SquareBracket),
            StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Text)),
            pascal_group(PARENTHESIS, Delimiter::Parenthesis),
        ]),
    );
    let (table, profile) = standard_table([entry(OUTER, form)]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let source = "{first[raw [One] second(raw (Two)}";
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, source, &mut names)
        .expect("each sibling activates only its own trigger set");
    assert_eq!(
        evaluator
            .encode_text(OUTER, &value, &names)
            .expect("multiple sibling-state canonical text"),
        source
    );
}

#[test]
fn table_seal_proves_trigger_disjointness_per_sequence_position() {
    let outer_boundary = TriggerIdentifier::new(20);
    let first_sibling = TriggerIdentifier::new(21);
    let second_sibling = TriggerIdentifier::new(22);
    let whitespace = TriggerIdentifier::new(23);
    let profile = TokenProfileData::new(
        ProfileRevision::new(18),
        vec![
            TriggerDefinition {
                identifier: outer_boundary,
                trigger: Trigger::Boundary {
                    opening: "{".to_owned(),
                    closing: "}".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: first_sibling,
                trigger: Trigger::Boundary {
                    opening: "<".to_owned(),
                    closing: ">".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: second_sibling,
                trigger: Trigger::Boundary {
                    opening: "<".to_owned(),
                    closing: "/>".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: whitespace,
                trigger: Trigger::Whitespace {
                    canonical_spelling: " ".to_owned(),
                },
            },
        ],
        TriggerSet::new(vec![outer_boundary, whitespace]),
        String::new(),
    )
    .seal()
    .expect("the root context never co-activates the sibling boundaries");
    let form = delimited(
        outer_boundary,
        Delimiter::Brace,
        SequenceForm::Product(vec![
            pascal_group(first_sibling, Delimiter::Parenthesis),
            pascal_group(second_sibling, Delimiter::SquareBracket),
        ]),
    );
    let table = table(
        &profile,
        TriggerSet::new(vec![whitespace]),
        [entry(OUTER, form)],
    );
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let source = "{<One> <Two/>}";
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, source, &mut names)
        .expect("same-opening sibling boundaries are disjoint by sequence state");
    assert_eq!(
        evaluator
            .encode_text(OUTER, &value, &names)
            .expect("per-position seal canonical text"),
        source
    );
}

#[test]
fn application_preflight_ignores_operator_glyphs_inside_bounded_children() {
    let carrier = StructuralForm::Leaf(LeafForm::with_trigger(
        LeafCodec::Carrier(CarrierLeaf::PipeText),
        PIPE_TEXT,
    ));
    let payload = delimited(
        BRACE,
        Delimiter::Brace,
        SequenceForm::Product(vec![StructuralForm::Leaf(LeafForm::scalar(
            ScalarLeaf::Text,
        ))]),
    );
    let form = StructuralForm::application(APPLICATION, carrier, payload);
    let (table, profile) = standard_table([entry(OUTER, form)]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let source = "(|left.right|).{payload.with.period}";
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, source, &mut names)
        .expect("only the outside application operator partitions the form");
    assert_eq!(
        evaluator
            .encode_text(OUTER, &value, &names)
            .expect("bounded application canonical text"),
        source
    );
}

#[test]
fn missing_outside_application_operator_is_a_safe_non_match() {
    let carrier = StructuralForm::Leaf(LeafForm::with_trigger(
        LeafCodec::Carrier(CarrierLeaf::PipeText),
        PIPE_TEXT,
    ));
    let form = StructuralForm::application(APPLICATION, carrier, StructuralForm::pascal_atom());
    let (table, profile) = standard_table([entry(OUTER, form)]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);

    assert!(matches!(
        evaluator.decode_text(OUTER, "(|head.with.period|)Payload", &mut names),
        Err(DecodeError::NoAlternative { core_type }) if core_type == OUTER
    ));
}

#[test]
fn utf8_negative_space_keeps_structural_bounds_on_character_boundaries() {
    let form = delimited(
        BRACE,
        Delimiter::Brace,
        SequenceForm::Product(vec![
            StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Text)),
            pascal_group(SQUARE, Delimiter::SquareBracket),
        ]),
    );
    let (table, profile) = standard_table([entry(OUTER, form)]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let source = "{é[β [Inner]}";
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(OUTER, source, &mut names)
        .expect("UTF-8 text remains inside checked byte bounds");
    assert_eq!(
        evaluator
            .encode_text(OUTER, &value, &names)
            .expect("UTF-8 canonical text"),
        source
    );
}

#[test]
fn sibling_state_failure_keeps_the_nametree_byte_identical() {
    let form = delimited(
        BRACE,
        Delimiter::Brace,
        SequenceForm::Product(vec![
            StructuralForm::Leaf(LeafForm::scalar(ScalarLeaf::Text)),
            pascal_group(SQUARE, Delimiter::SquareBracket),
        ]),
    );
    let (table, profile) = standard_table([entry(OUTER, form)]);
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    names.intern(Name::new("Prior")).expect("prior name");
    let before = names.to_archive_bytes().expect("before");
    let identity_before = names.identity().expect("identity before");

    assert!(matches!(
        evaluator.decode_text(OUTER, "{alpha[beta [lower]}", &mut names),
        Err(DecodeError::BoundedInterior { .. })
    ));
    assert_eq!(
        before.as_ref(),
        names.to_archive_bytes().expect("after").as_ref()
    );
    assert_eq!(identity_before, names.identity().expect("identity after"));
}

#[test]
fn literal_headed_alternatives_check_the_configured_spelling_before_payload() {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let mut lexicon = NameTable::new(IdentifierNamespace::Fixture);
    let tool_path = one_name(&mut lexicon, "ToolPath");
    let derive = one_name(&mut lexicon, "Derive");
    let configuration = one_name(&mut lexicon, "Configuration");
    let tool_path_form = literal_application(
        tool_path,
        delimited(BRACE, Delimiter::Brace, SequenceForm::Product(Vec::new())),
    );
    let derive_form = literal_application(
        derive,
        delimited(
            SQUARE,
            Delimiter::SquareBracket,
            SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        ),
    );
    let configuration_form = literal_application(
        configuration,
        delimited(BRACE, Delimiter::Brace, SequenceForm::Product(Vec::new())),
    );
    let table = table(
        &profile,
        TriggerSet::new(vec![WHITESPACE, COMMENT]),
        [entry_with_alternatives(
            OUTER,
            vec![tool_path_form, derive_form.clone(), configuration_form],
            derive_form,
        )],
    );
    let evaluator = StructuralEvaluator::with_profile_and_lexicon(&table, &profile, &lexicon);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);

    evaluator
        .decode_text(OUTER, "Derive.[Clone]", &mut names)
        .expect("Derive is selected by its literal head");
}

#[test]
fn payload_non_match_allows_a_later_disjoint_application_alternative() {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let mut lexicon = NameTable::new(IdentifierNamespace::Fixture);
    let derive = one_name(&mut lexicon, "Derive");
    let brace_payload = literal_application(
        derive,
        delimited(BRACE, Delimiter::Brace, SequenceForm::Product(Vec::new())),
    );
    let square_payload = literal_application(
        derive,
        delimited(
            SQUARE,
            Delimiter::SquareBracket,
            SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        ),
    );
    let table = table(
        &profile,
        TriggerSet::new(vec![WHITESPACE, COMMENT]),
        [entry_with_alternatives(
            OUTER,
            vec![brace_payload, square_payload.clone()],
            square_payload,
        )],
    );
    let evaluator = StructuralEvaluator::with_profile_and_lexicon(&table, &profile, &lexicon);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);

    evaluator
        .decode_text(OUTER, "Derive.[Clone]", &mut names)
        .expect("the square payload alternative remains viable");
}

#[test]
fn emitted_literal_application_redecodes_through_its_table() {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let mut lexicon = NameTable::new(IdentifierNamespace::Fixture);
    let tool_path = one_name(&mut lexicon, "ToolPath");
    let derive = one_name(&mut lexicon, "Derive");
    let configuration = one_name(&mut lexicon, "Configuration");
    let derive_form = literal_application(
        derive,
        delimited(
            SQUARE,
            Delimiter::SquareBracket,
            SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        ),
    );
    let table = table(
        &profile,
        TriggerSet::new(vec![WHITESPACE, COMMENT]),
        [entry_with_alternatives(
            OUTER,
            vec![
                literal_application(
                    tool_path,
                    delimited(BRACE, Delimiter::Brace, SequenceForm::Product(Vec::new())),
                ),
                derive_form.clone(),
                literal_application(
                    configuration,
                    delimited(BRACE, Delimiter::Brace, SequenceForm::Product(Vec::new())),
                ),
            ],
            derive_form,
        )],
    );
    let evaluator = StructuralEvaluator::with_profile_and_lexicon(&table, &profile, &lexicon);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert_eq!(one_name(&mut names, "ToolPath"), tool_path);
    assert_eq!(one_name(&mut names, "Derive"), derive);
    assert_eq!(one_name(&mut names, "Configuration"), configuration);
    let clone = one_name(&mut names, "Clone");
    let value = StructuralValue::chosen(
        0,
        StructuralValue::Application(
            Box::new(StructuralValue::Atom(derive)),
            Box::new(StructuralValue::Delimited(vec![StructuralValue::Atom(
                clone,
            )])),
        ),
    );

    let encoded = evaluator
        .encode_text(OUTER, &value, &names)
        .expect("typed value emits through the canonical form");
    assert_eq!(encoded, "Derive.[Clone]");
    let mut names_again = NameTable::new(IdentifierNamespace::Fixture);
    assert_eq!(one_name(&mut names_again, "ToolPath"), tool_path);
    assert_eq!(one_name(&mut names_again, "Derive"), derive);
    assert_eq!(one_name(&mut names_again, "Configuration"), configuration);

    assert_eq!(
        evaluator
            .decode_text(OUTER, &encoded, &mut names_again)
            .expect("canonical output re-decodes"),
        value
    );
}

#[test]
fn unknown_or_ambiguous_literal_alternatives_remain_refused() {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let mut lexicon = NameTable::new(IdentifierNamespace::Fixture);
    let derive = one_name(&mut lexicon, "Derive");
    let form = literal_application(
        derive,
        delimited(
            SQUARE,
            Delimiter::SquareBracket,
            SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        ),
    );
    let table = table(
        &profile,
        TriggerSet::new(vec![WHITESPACE, COMMENT]),
        [entry_with_alternatives(
            OUTER,
            vec![form.clone()],
            form.clone(),
        )],
    );
    let evaluator = StructuralEvaluator::with_profile_and_lexicon(&table, &profile, &lexicon);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(matches!(
        evaluator.decode_text(OUTER, "Unknown.[Clone]", &mut names),
        Err(DecodeError::NoAlternative { core_type }) if core_type == OUTER
    ));

    let ambiguous = entry_with_alternatives(OUTER, vec![form.clone(), form.clone()], form);
    let payload = TableIdentityPayload {
        core_universe: structural_codec::FIXTURE_UNIVERSE,
        core_layout_identity: EncodedLayoutIdentity([0x51; 32]),
        raw_profile_identity: RawProfileIdentity::from_profile(&profile),
        trivia_triggers: TriggerSet::new(vec![WHITESPACE, COMMENT]),
        leaf_codec_contracts: Vec::new(),
        entries: BTreeMap::from([(OUTER, ambiguous)]),
    };
    assert!(AddressedStructuralTable::seal(payload, &profile).is_err());
}
