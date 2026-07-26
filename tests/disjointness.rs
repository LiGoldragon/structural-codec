//! The conservative checker accepts a table only when every pair of accepted
//! forms has a proof. These are the primitive-kernel corpus, ported onto the
//! archived R3 typed records rather than a spelling-equality substitute.

use std::collections::BTreeMap;

use name_table::{IdentifierNamespace, Name, NameTable, NameTableError};
use raw_discovery::{Recognizer, TriggerSet};

use crate::fixture::{APPLICATION_OPERATOR, BRACE_BOUNDARY, COMMENT_TRIVIA, WHITESPACE_TRIVIA};
use crate::{
    AcceptedDecodeForm, AddressedStructuralTable, ApplicationDelimitedRule, ApplicationRule,
    AtomCase, AtomDescriptor, ConstructorCodec, DecodeError, DecodeFormId, DelegationPayload,
    DisjointnessError, EncodeError, EncodedConstructorId, EncodedLanguage, FieldValue,
    RoleKeyedMirror, ScopedEncodedTypeId, SharedDescriptor, StructuralEntry, StructuralEvaluator,
    StructuralRule, StructuralValue, StructuralVocabularyIdentity, TableError,
    TableIdentityPayload, TargetLayoutIdentity, UnaryRoot, UnaryRule,
};

const TYPE: ScopedEncodedTypeId = ScopedEncodedTypeId::schema(0xf100);

fn unary(descriptor: SharedDescriptor) -> StructuralRule {
    StructuralRule::Unary(UnaryRule::new(descriptor).expect("non-zero built-in role"))
}

fn application(head: SharedDescriptor, payload: SharedDescriptor) -> StructuralRule {
    StructuralRule::Application(
        ApplicationRule::new(APPLICATION_OPERATOR, head, payload).expect("built-in roles"),
    )
}

fn application_delimited(head: SharedDescriptor) -> StructuralRule {
    StructuralRule::ApplicationDelimited(
        ApplicationDelimitedRule::new(
            APPLICATION_OPERATOR,
            BRACE_BOUNDARY,
            head,
            SharedDescriptor::Atom(AtomDescriptor::any_case()),
            0,
            None,
        )
        .expect("built-in roles"),
    )
}

fn codec(
    type_id: ScopedEncodedTypeId,
    local: u16,
    forms: Vec<(u16, StructuralRule)>,
    encode: StructuralRule,
) -> ConstructorCodec {
    ConstructorCodec::new(
        EncodedConstructorId::under(type_id, local),
        forms
            .into_iter()
            .map(|(identity, rule)| AcceptedDecodeForm::new(DecodeFormId::new(identity), rule))
            .collect(),
        encode,
    )
}

fn entry(type_id: ScopedEncodedTypeId, codecs: Vec<ConstructorCodec>) -> StructuralEntry {
    StructuralEntry::new(type_id, codecs)
}

fn seal_entries(
    entries: impl IntoIterator<Item = StructuralEntry>,
) -> Result<AddressedStructuralTable, TableError> {
    let profile = crate::fixture::FixtureBuilder::token_profile();
    let entries = entries
        .into_iter()
        .map(|entry| (entry.encoded_type(), entry))
        .collect();
    AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            EncodedLanguage::Schema,
            TargetLayoutIdentity::derive(b"disjointness typed-record layout"),
            profile.identity(),
            StructuralVocabularyIdentity::fixture(b"disjointness typed-record vocabulary"),
            TriggerSet::new(vec![WHITESPACE_TRIVIA, COMMENT_TRIVIA]),
            entries,
        ),
        &profile,
    )
}

fn seal_entry(entry: StructuralEntry) -> Result<AddressedStructuralTable, TableError> {
    seal_entries([entry])
}

fn atom(case: AtomCase) -> StructuralRule {
    unary(SharedDescriptor::Atom(AtomDescriptor::with_case(case)))
}

fn atom_any() -> StructuralRule {
    unary(SharedDescriptor::Atom(AtomDescriptor::any_case()))
}

fn one_constructor(type_id: ScopedEncodedTypeId, rule: StructuralRule) -> StructuralEntry {
    entry(
        type_id,
        vec![codec(type_id, 1, vec![(1, rule.clone())], rule)],
    )
}

fn single_block(source: &str) -> raw_discovery::Block {
    Recognizer::standard()
        .recognize(source)
        .expect("recognized source")
        .root_object_at(0)
        .expect("one root")
        .clone()
}

#[test]
fn field_entry_is_the_sole_elided_constructor() {
    let table = crate::fixture::FixtureBuilder::new()
        .build()
        .expect("fixture seals");
    let field = table.entry(crate::fixture::FIELD).expect("field");
    assert_eq!(field.constructors().len(), 1);
    assert_eq!(field.constructors()[0].decode_forms().len(), 1);
}

#[test]
fn distinct_atom_cases_are_disjoint_and_identical_cases_are_rejected() {
    let distinct = entry(
        TYPE,
        vec![codec(
            TYPE,
            1,
            vec![
                (1, atom(AtomCase::PascalCase)),
                (2, atom(AtomCase::CamelCase)),
            ],
            atom(AtomCase::PascalCase),
        )],
    );
    seal_entry(distinct).expect("distinct cases prove disjoint");

    let overlap = entry(
        TYPE,
        vec![codec(
            TYPE,
            1,
            vec![
                (1, atom(AtomCase::PascalCase)),
                (2, atom(AtomCase::PascalCase)),
            ],
            atom(AtomCase::PascalCase),
        )],
    );
    assert!(matches!(
        seal_entry(overlap),
        Err(TableError::Disjointness(
            DisjointnessError::NotProvablyDisjoint { .. }
        ))
    ));
}

#[test]
fn bare_delegate_forms_remain_conservative_without_a_table() {
    let left = ScopedEncodedTypeId::schema(0xf101);
    let right = ScopedEncodedTypeId::schema(0xf102);
    let delegated = entry(
        TYPE,
        vec![
            codec(
                TYPE,
                1,
                vec![(
                    1,
                    unary(SharedDescriptor::Delegate {
                        target: left,
                        payload: None,
                    }),
                )],
                unary(SharedDescriptor::Delegate {
                    target: left,
                    payload: None,
                }),
            ),
            codec(
                TYPE,
                2,
                vec![(
                    1,
                    unary(SharedDescriptor::Delegate {
                        target: right,
                        payload: None,
                    }),
                )],
                unary(SharedDescriptor::Delegate {
                    target: right,
                    payload: None,
                }),
            ),
        ],
    );
    assert!(delegated.validate_disjoint().is_err());
}

#[test]
fn unguarded_delegate_cycles_are_typed_seal_failures() {
    let recursive = ScopedEncodedTypeId::schema(0xf103);
    let self_cycle = entry(
        recursive,
        vec![
            codec(
                recursive,
                1,
                vec![(
                    1,
                    unary(SharedDescriptor::Delegate {
                        target: recursive,
                        payload: None,
                    }),
                )],
                unary(SharedDescriptor::Delegate {
                    target: recursive,
                    payload: None,
                }),
            ),
            codec(
                recursive,
                2,
                vec![(1, atom(AtomCase::PascalCase))],
                atom(AtomCase::PascalCase),
            ),
        ],
    );
    assert!(matches!(
        seal_entry(self_cycle),
        Err(TableError::Disjointness(DisjointnessError::DelegateExpansionCycle {
            core_type,
            reentered,
        })) if core_type == recursive && reentered == recursive
    ));

    let outer = ScopedEncodedTypeId::schema(0xf104);
    let left = ScopedEncodedTypeId::schema(0xf105);
    let right = ScopedEncodedTypeId::schema(0xf106);
    let mutual = seal_entries([
        entry(
            outer,
            vec![
                codec(
                    outer,
                    1,
                    vec![(
                        1,
                        unary(SharedDescriptor::Delegate {
                            target: left,
                            payload: None,
                        }),
                    )],
                    unary(SharedDescriptor::Delegate {
                        target: left,
                        payload: None,
                    }),
                ),
                codec(
                    outer,
                    2,
                    vec![(1, atom(AtomCase::PascalCase))],
                    atom(AtomCase::PascalCase),
                ),
            ],
        ),
        one_constructor(
            left,
            unary(SharedDescriptor::Delegate {
                target: right,
                payload: None,
            }),
        ),
        one_constructor(
            right,
            unary(SharedDescriptor::Delegate {
                target: left,
                payload: None,
            }),
        ),
    ]);
    assert!(matches!(
        mutual,
        Err(TableError::Disjointness(DisjointnessError::DelegateExpansionCycle {
            core_type,
            reentered,
        })) if core_type == outer && reentered == left
    ));
}

#[test]
fn guarded_recursion_and_block_kind_proofs_still_seal() {
    let recursive = ScopedEncodedTypeId::schema(0xf107);
    let pascal = application(
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
        SharedDescriptor::Delegate {
            target: recursive,
            payload: None,
        },
    );
    let camel = application(
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::CamelCase)),
        SharedDescriptor::Delegate {
            target: recursive,
            payload: None,
        },
    );
    seal_entry(entry(
        recursive,
        vec![
            codec(recursive, 1, vec![(1, pascal.clone())], pascal),
            codec(recursive, 2, vec![(1, camel.clone())], camel),
        ],
    ))
    .expect("distinguishing heads guard recursive payloads");

    seal_entry(entry(
        TYPE,
        vec![
            codec(
                TYPE,
                1,
                vec![(1, atom(AtomCase::PascalCase))],
                atom(AtomCase::PascalCase),
            ),
            codec(
                TYPE,
                2,
                vec![(
                    1,
                    application(
                        SharedDescriptor::Atom(AtomDescriptor::any_case()),
                        SharedDescriptor::Atom(AtomDescriptor::any_case()),
                    ),
                )],
                application(
                    SharedDescriptor::Atom(AtomDescriptor::any_case()),
                    SharedDescriptor::Atom(AtomDescriptor::any_case()),
                ),
            ),
        ],
    ))
    .expect("atom and application have disjoint outer kinds");
}

#[test]
fn literal_and_unconstrained_atom_are_not_admitted_as_alternatives() {
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let literal = names.intern(Name::new("Integer")).expect("literal");
    let literal_rule = unary(SharedDescriptor::Literal(literal));
    assert!(matches!(
        seal_entry(entry(
            TYPE,
            vec![
                codec(TYPE, 1, vec![(1, literal_rule.clone())], literal_rule),
                codec(TYPE, 2, vec![(1, atom_any())], atom_any()),
            ],
        )),
        Err(TableError::Disjointness(_))
    ));
}

#[test]
fn literal_lexicon_causes_are_preserved_without_alternative_fallback() {
    let mut schema_names = NameTable::new(IdentifierNamespace::Schema);
    let missing = schema_names
        .intern(Name::new("Missing"))
        .expect("schema id");
    let mut lexicon = NameTable::new(IdentifierNamespace::Fixture);
    let present = lexicon.intern(Name::new("Present")).expect("fixture id");
    let missing_rule = unary(SharedDescriptor::Literal(missing));
    let present_rule = unary(SharedDescriptor::Literal(present));
    let table = seal_entry(entry(
        TYPE,
        vec![
            codec(TYPE, 1, vec![(1, missing_rule.clone())], missing_rule),
            codec(TYPE, 2, vec![(1, present_rule.clone())], present_rule),
        ],
    ))
    .expect("different literals are disjoint");
    let profile = crate::fixture::FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile_and_lexicon(&table, &profile, &lexicon)
        .expect("profile pin");
    let mut decoded_names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(matches!(
        evaluator.decode(TYPE, &single_block("Present"), &mut decoded_names),
        Err(DecodeError::Names(NameTableError::UnknownNamespace(
            IdentifierNamespace::Schema
        )))
    ));
}

#[test]
fn missing_lexicon_and_literal_encode_mismatch_keep_their_precise_errors() {
    let mut lexicon = NameTable::new(IdentifierNamespace::Fixture);
    let expected = lexicon.intern(Name::new("Expected")).expect("expected");
    let other = lexicon.intern(Name::new("Other")).expect("other");
    let rule = unary(SharedDescriptor::Literal(expected));
    let table = seal_entry(one_constructor(TYPE, rule)).expect("one literal seals");
    let profile = crate::fixture::FixtureBuilder::token_profile();
    let without_lexicon = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(matches!(
        without_lexicon.decode(TYPE, &single_block("Expected"), &mut names),
        Err(DecodeError::MissingLexicon)
    ));

    let constructor = table.entry(TYPE).expect("entry").constructors()[0].constructor();
    let mut fields = RoleKeyedMirror::default();
    fields.insert(
        crate::StableRoleId::for_role::<UnaryRoot>(),
        FieldValue::Atom(other),
    );
    let value = StructuralValue::new(constructor, fields);
    let with_lexicon =
        StructuralEvaluator::with_profile_and_lexicon(&table, &profile, &lexicon).expect("profile");
    assert!(matches!(
        with_lexicon.encode(TYPE, &value, &lexicon),
        Err(EncodeError::LiteralMismatch)
    ));
}

#[test]
fn unknown_type_is_terminal_and_never_falls_through_an_alternative() {
    let table = crate::fixture::FixtureBuilder::new()
        .build()
        .expect("fixture");
    let profile = crate::fixture::FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
    let unknown = ScopedEncodedTypeId::schema(0xfffe);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(matches!(
        evaluator.decode(unknown, &single_block("Entry"), &mut names),
        Err(DecodeError::UnknownType(actual)) if actual == unknown
    ));
}

#[test]
fn accepted_application_payload_non_match_falls_through_to_the_disjoint_form() {
    let pascal = application(
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
    );
    let camel = application(
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::CamelCase)),
    );
    let table = seal_entry(entry(
        TYPE,
        vec![
            codec(TYPE, 7, vec![(1, pascal.clone())], pascal),
            codec(TYPE, 9, vec![(1, camel.clone())], camel),
        ],
    ))
    .expect("payload cases prove disjoint");
    let profile = crate::fixture::FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert_eq!(
        evaluator
            .decode(TYPE, &single_block("Head.payload"), &mut names)
            .expect("second payload alternative")
            .constructor()
            .local(),
        9
    );
}

#[test]
fn constructor_and_form_vector_order_never_select_meaning() {
    let application_rule = application(
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
    );
    let bare_rule = atom(AtomCase::PascalCase);
    let make = |reverse: bool| {
        let mut codecs = vec![
            codec(TYPE, 7, vec![(1, bare_rule.clone())], bare_rule.clone()),
            codec(
                TYPE,
                9,
                vec![(2, application_rule.clone())],
                application_rule.clone(),
            ),
        ];
        if reverse {
            codecs.reverse();
        }
        seal_entry(entry(TYPE, codecs)).expect("disjoint table")
    };
    let profile = crate::fixture::FixtureBuilder::token_profile();
    for table in [make(false), make(true)] {
        let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
        let mut names = NameTable::new(IdentifierNamespace::Fixture);
        assert_eq!(
            evaluator
                .decode(TYPE, &single_block("Entry"), &mut names)
                .expect("bare alternative")
                .constructor()
                .local(),
            7
        );
    }
}

#[test]
fn delegate_expansion_preserves_directed_payload_and_wrapper_value() {
    let target = ScopedEncodedTypeId::schema(0xf108);
    let outer = ScopedEncodedTypeId::schema(0xf109);
    let target_entry = one_constructor(target, atom_any());
    let pascal_delegate = unary(SharedDescriptor::Delegate {
        target,
        payload: Some(DelegationPayload::AtomCase(AtomCase::PascalCase)),
    });
    let camel_delegate = unary(SharedDescriptor::Delegate {
        target,
        payload: Some(DelegationPayload::AtomCase(AtomCase::CamelCase)),
    });
    let table = seal_entries([
        target_entry,
        entry(
            outer,
            vec![
                codec(
                    outer,
                    1,
                    vec![(1, pascal_delegate.clone())],
                    pascal_delegate,
                ),
                codec(outer, 2, vec![(1, camel_delegate.clone())], camel_delegate),
            ],
        ),
    ])
    .expect("payload constraints prove directed delegates disjoint");
    let profile = crate::fixture::FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let pascal = evaluator
        .decode(outer, &single_block("Entry"), &mut names)
        .expect("Pascal delegate");
    let camel = evaluator
        .decode(outer, &single_block("entry"), &mut names)
        .expect("camel delegate");
    assert_eq!(pascal.constructor().local(), 1);
    assert_eq!(camel.constructor().local(), 2);
    assert!(matches!(
        pascal.fields().value::<UnaryRoot>(),
        Some(FieldValue::Delegated(inner)) if inner.constructor().type_id() == target
    ));

    let plain = entry(
        outer,
        vec![
            codec(
                outer,
                1,
                vec![(
                    1,
                    unary(SharedDescriptor::Delegate {
                        target,
                        payload: None,
                    }),
                )],
                unary(SharedDescriptor::Delegate {
                    target,
                    payload: None,
                }),
            ),
            codec(
                outer,
                2,
                vec![(
                    1,
                    unary(SharedDescriptor::Delegate {
                        target,
                        payload: None,
                    }),
                )],
                unary(SharedDescriptor::Delegate {
                    target,
                    payload: None,
                }),
            ),
        ],
    );
    assert!(matches!(
        seal_entries([one_constructor(target, atom_any()), plain]),
        Err(TableError::Disjointness(_))
    ));
}

#[test]
fn delegate_proof_reaches_a_boundary_and_payload_changes_move_table_identity() {
    let target = ScopedEncodedTypeId::schema(0xf10a);
    let outer = ScopedEncodedTypeId::schema(0xf10b);
    let delegate = application(
        SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
        SharedDescriptor::Delegate {
            target,
            payload: None,
        },
    );
    let boundary = application_delimited(SharedDescriptor::Atom(AtomDescriptor::with_case(
        AtomCase::PascalCase,
    )));
    seal_entries([
        one_constructor(target, atom(AtomCase::PascalCase)),
        entry(
            outer,
            vec![
                codec(outer, 1, vec![(1, delegate.clone())], delegate),
                codec(outer, 2, vec![(1, boundary.clone())], boundary),
            ],
        ),
    ])
    .expect("delegate atom and boundary are disjoint through the shared proof");

    let table_for = |payload| {
        seal_entries([
            one_constructor(target, atom_any()),
            entry(
                outer,
                vec![codec(
                    outer,
                    1,
                    vec![(
                        1,
                        unary(SharedDescriptor::Delegate {
                            target,
                            payload: Some(payload),
                        }),
                    )],
                    unary(SharedDescriptor::Delegate {
                        target,
                        payload: Some(payload),
                    }),
                )],
            ),
        ])
        .expect("single directed delegate seals")
    };
    assert_ne!(
        table_for(DelegationPayload::AtomCase(AtomCase::PascalCase)).identity(),
        table_for(DelegationPayload::AtomCase(AtomCase::CamelCase)).identity(),
    );
}

#[test]
fn r4_ids_archive_as_distinct_language_variants_and_associations_are_checked() {
    let schema = ScopedEncodedTypeId::schema(9);
    let logos = ScopedEncodedTypeId::logos(9);
    let nomos = ScopedEncodedTypeId::nomos(9);
    assert_ne!(schema, logos);
    assert_ne!(logos, nomos);
    for identity in [schema, logos, nomos] {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&identity).expect("archive identity");
        let restored = rkyv::from_bytes::<ScopedEncodedTypeId, rkyv::rancor::Error>(&bytes)
            .expect("restore identity");
        assert_eq!(restored, identity);
    }

    let schema_entry = ScopedEncodedTypeId::schema(0xf10c);
    let logos_constructor = EncodedConstructorId::under(ScopedEncodedTypeId::logos(3), 1);
    let wrong_constructor = entry(
        schema_entry,
        vec![ConstructorCodec::new(
            logos_constructor,
            vec![AcceptedDecodeForm::new(DecodeFormId::new(1), atom_any())],
            atom_any(),
        )],
    );
    assert!(matches!(
        seal_entry(wrong_constructor),
        Err(TableError::ConstructorUnderWrongEntry { constructor, entry })
            if constructor == logos_constructor && entry == schema_entry
    ));

    let profile = crate::fixture::FixtureBuilder::token_profile();
    let logos_entry = entry(
        ScopedEncodedTypeId::logos(7),
        vec![codec(
            ScopedEncodedTypeId::logos(7),
            1,
            vec![(1, atom_any())],
            atom_any(),
        )],
    );
    assert!(matches!(
        AddressedStructuralTable::seal(
            TableIdentityPayload::new(
                EncodedLanguage::Schema,
                TargetLayoutIdentity::derive(b"cross-language table"),
                profile.identity(),
                StructuralVocabularyIdentity::language(b"cross-language vocabulary"),
                TriggerSet::new(vec![]),
                BTreeMap::from([(logos_entry.encoded_type(), logos_entry)]),
            ),
            &profile,
        ),
        Err(TableError::LanguageMismatch { table: EncodedLanguage::Schema, encoded })
            if encoded == ScopedEncodedTypeId::logos(7)
    ));
}

#[test]
fn duplicate_constructor_and_decode_form_identities_are_refused_at_seal() {
    let duplicate_constructor = EncodedConstructorId::under(TYPE, 1);
    let constructor_duplicate = entry(
        TYPE,
        vec![
            ConstructorCodec::new(
                duplicate_constructor,
                vec![AcceptedDecodeForm::new(
                    DecodeFormId::new(1),
                    atom(AtomCase::PascalCase),
                )],
                atom(AtomCase::PascalCase),
            ),
            ConstructorCodec::new(
                duplicate_constructor,
                vec![AcceptedDecodeForm::new(
                    DecodeFormId::new(2),
                    atom(AtomCase::CamelCase),
                )],
                atom(AtomCase::CamelCase),
            ),
        ],
    );
    assert!(matches!(
        seal_entry(constructor_duplicate),
        Err(TableError::DuplicateConstructor { constructor, .. }) if constructor == duplicate_constructor
    ));

    let form_duplicate = entry(
        TYPE,
        vec![codec(
            TYPE,
            1,
            vec![
                (7, atom(AtomCase::PascalCase)),
                (7, atom(AtomCase::CamelCase)),
            ],
            atom(AtomCase::PascalCase),
        )],
    );
    assert!(matches!(
        seal_entry(form_duplicate),
        Err(TableError::DuplicateDecodeForm { form, .. }) if form == DecodeFormId::new(7)
    ));
}

#[test]
fn profile_trivia_completion_rewinds_before_shared_decode() {
    let table = crate::fixture::FixtureBuilder::new()
        .build()
        .expect("fixture");
    let profile = crate::fixture::FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile).expect("profile");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(
            crate::fixture::COMMIT_SEQUENCE,
            "CommitSequence.{ Integer ;; accepted trivia\n }",
            &mut names,
        )
        .expect("trivia completion is rewound before direct evaluation");
    assert_eq!(
        evaluator
            .encode_text(crate::fixture::COMMIT_SEQUENCE, &value, &names)
            .expect("canonical text"),
        "CommitSequence.{Integer}"
    );
}
