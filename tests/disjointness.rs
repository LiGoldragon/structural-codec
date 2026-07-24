//! The conservative disjointness checker: accepts a pair of decode forms only when
//! it can PROVE no block matches both; unprovable overlap is a hard error.

use std::collections::BTreeMap;

use name_table::{IdentifierNamespace, Name, NameTable};
use raw_discovery::{Delimiter, Recognizer, TriggerSet};
use structural_codec::fixture::{
    APPLICATION_OPERATOR, BRACE_BOUNDARY, COMMENT_TRIVIA, FIELD, FixtureBuilder, WHITESPACE_TRIVIA,
};
use structural_codec::{
    AddressedStructuralTable, AtomCase, AtomForm, ConstructorCodec, DelegationPayload, EncodeError,
    EncodedConstructorId, EncodedLayoutIdentity, PositionalSignature, RawProfileIdentity,
    ScopedEncodedTypeId, StructuralEntry, StructuralEvaluator, StructuralForm, StructuralValue,
    TableError, TableIdentityPayload,
};

fn entry_with_forms(forms: Vec<StructuralForm>) -> StructuralEntry {
    let core_type = ScopedEncodedTypeId::fixture(100);
    let constructors = forms
        .into_iter()
        .enumerate()
        .map(|(index, form)| {
            ConstructorCodec::new(
                EncodedConstructorId::new(core_type, index as u32),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )
        })
        .collect();
    StructuralEntry::new(core_type, constructors)
}

fn sealed_table(entry: StructuralEntry) -> Result<AddressedStructuralTable, TableError> {
    sealed_entries(BTreeMap::from([(entry.core_type, entry)]))
}

fn sealed_entries(
    entries: BTreeMap<ScopedEncodedTypeId, StructuralEntry>,
) -> Result<AddressedStructuralTable, TableError> {
    let profile = FixtureBuilder::token_profile();
    AddressedStructuralTable::seal(
        TableIdentityPayload {
            core_universe: structural_codec::FIXTURE_UNIVERSE,
            core_layout_identity: EncodedLayoutIdentity([0; 32]),
            raw_profile_identity: RawProfileIdentity::from_profile(&profile),
            trivia_triggers: TriggerSet::new(vec![WHITESPACE_TRIVIA, COMMENT_TRIVIA]),
            leaf_codec_contracts: Vec::new(),
            entries,
        },
        &profile,
    )
}

fn chosen_constructor(value: StructuralValue) -> u32 {
    let StructuralValue::Chosen { constructor, .. } = value else {
        panic!("expected a constructor-tagged value");
    };
    constructor
}

/// The `Field` entry has exactly one bare-type constructor; the banned named
/// alternative cannot reappear as a second accepted decode form.
#[test]
fn field_entry_is_the_sole_elided_constructor() {
    let table = FixtureBuilder::new().build().expect("seal");
    table
        .validate_disjoint()
        .expect("the whole fixture table validates");

    let field = table.entry(FIELD).expect("field entry");
    assert_eq!(
        field.constructors.len(),
        1,
        "only the elided field form remains"
    );
    assert_eq!(field.constructors[0].decode_forms.len(), 1);
    assert_eq!(
        field.constructors[0].encode_form,
        StructuralForm::pascal_atom()
    );
}

/// Two atoms of DIFFERENT concrete case are provably disjoint.
#[test]
fn distinct_atom_cases_are_disjoint() {
    let entry = entry_with_forms(vec![
        StructuralForm::Atom(AtomForm::with_case(AtomCase::PascalCase)),
        StructuralForm::Atom(AtomForm::with_case(AtomCase::CamelCase)),
    ]);
    entry.validate_disjoint().expect("distinct cases disjoint");
}

/// Two atoms of the SAME case overlap — the checker cannot prove them distinct, so it
/// errors (conservative-safe).
#[test]
fn identical_atom_cases_are_rejected() {
    let entry = entry_with_forms(vec![
        StructuralForm::Atom(AtomForm::with_case(AtomCase::PascalCase)),
        StructuralForm::Atom(AtomForm::with_case(AtomCase::PascalCase)),
    ]);
    assert!(
        entry.validate_disjoint().is_err(),
        "overlapping atom cases must be rejected"
    );
}

/// Delegate forms are opaque — their matchable block kind is unknown — so a pair of
/// them can never be proven disjoint and is rejected.
#[test]
fn delegate_forms_are_conservatively_rejected() {
    let entry = entry_with_forms(vec![
        StructuralForm::delegate(ScopedEncodedTypeId::fixture(200)),
        StructuralForm::delegate(ScopedEncodedTypeId::fixture(201)),
    ]);
    assert!(
        entry.validate_disjoint().is_err(),
        "opaque delegate forms must be conservatively rejected"
    );
}

/// An unguarded self-delegate cycle is rejected at seal time as a typed structural
/// failure rather than recursing until the process aborts.
#[test]
fn seal_rejects_self_delegate_cycle_with_typed_failure() {
    let recursive = ScopedEncodedTypeId::fixture(210);
    let delegate = StructuralForm::delegate(recursive);
    let delimited = StructuralForm::Delimited {
        boundary: BRACE_BOUNDARY,
        delimiter: Delimiter::Brace,
        sequence: structural_codec::SequenceForm::zero_or_more(StructuralForm::pascal_atom()),
    };
    let entry = StructuralEntry::new(
        recursive,
        vec![
            ConstructorCodec::new(
                EncodedConstructorId::new(recursive, 0),
                vec![delegate.clone()],
                delegate,
                PositionalSignature::default(),
            ),
            ConstructorCodec::new(
                EncodedConstructorId::new(recursive, 1),
                vec![delimited.clone()],
                delimited,
                PositionalSignature::default(),
            ),
        ],
    );

    let Err(TableError::Disjointness(error)) = sealed_table(entry) else {
        panic!("an unguarded self-delegate cycle must fail sealing");
    };
    assert!(matches!(
        error,
        structural_codec::DisjointnessError::DelegateExpansionCycle {
            core_type,
            first: 0,
            second: 1,
            reentered,
        } if core_type == recursive && reentered == recursive
    ));

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&error).expect("archive typed failure");
    let restored =
        rkyv::from_bytes::<structural_codec::DisjointnessError, rkyv::rancor::Error>(&bytes)
            .expect("restore typed failure");
    assert!(matches!(
        restored,
        structural_codec::DisjointnessError::DelegateExpansionCycle {
            core_type,
            first: 0,
            second: 1,
            reentered,
        } if core_type == recursive && reentered == recursive
    ));
}

/// A mutual unguarded delegate cycle also returns the exact typed seal failure.
#[test]
fn seal_rejects_mutual_delegate_cycle_with_typed_failure() {
    let outer = ScopedEncodedTypeId::fixture(220);
    let left = ScopedEncodedTypeId::fixture(221);
    let right = ScopedEncodedTypeId::fixture(222);
    let single_delegate = |core_type, target| {
        let delegate = StructuralForm::delegate(target);
        StructuralEntry::new(
            core_type,
            vec![ConstructorCodec::new(
                EncodedConstructorId::new(core_type, 0),
                vec![delegate.clone()],
                delegate,
                PositionalSignature::default(),
            )],
        )
    };
    let outer_delegate = StructuralForm::delegate(left);
    let outer_delimited = StructuralForm::Delimited {
        boundary: BRACE_BOUNDARY,
        delimiter: Delimiter::Brace,
        sequence: structural_codec::SequenceForm::zero_or_more(StructuralForm::pascal_atom()),
    };
    let outer_entry = StructuralEntry::new(
        outer,
        vec![
            ConstructorCodec::new(
                EncodedConstructorId::new(outer, 0),
                vec![outer_delegate.clone()],
                outer_delegate,
                PositionalSignature::default(),
            ),
            ConstructorCodec::new(
                EncodedConstructorId::new(outer, 1),
                vec![outer_delimited.clone()],
                outer_delimited,
                PositionalSignature::default(),
            ),
        ],
    );

    assert!(matches!(
        sealed_entries(BTreeMap::from([
            (outer, outer_entry),
            (left, single_delegate(left, right)),
            (right, single_delegate(right, left)),
        ])),
        Err(TableError::Disjointness(
            structural_codec::DisjointnessError::DelegateExpansionCycle {
                core_type,
                first: 0,
                second: 1,
                reentered,
            }
        )) if core_type == outer && reentered == left
    ));
}

/// Guarded recursion remains valid: the distinct application heads prove separation
/// before either recursive payload needs expansion.
#[test]
fn seal_preserves_guarded_recursive_alternatives() {
    let recursive = ScopedEncodedTypeId::fixture(230);
    let pascal = StructuralForm::application(
        APPLICATION_OPERATOR,
        StructuralForm::pascal_atom(),
        StructuralForm::delegate(recursive),
    );
    let camel = StructuralForm::application(
        APPLICATION_OPERATOR,
        StructuralForm::camel_atom(),
        StructuralForm::delegate(recursive),
    );
    let entry = StructuralEntry::new(
        recursive,
        vec![
            ConstructorCodec::new(
                EncodedConstructorId::new(recursive, 0),
                vec![pascal.clone()],
                pascal,
                PositionalSignature::default(),
            ),
            ConstructorCodec::new(
                EncodedConstructorId::new(recursive, 1),
                vec![camel.clone()],
                camel,
                PositionalSignature::default(),
            ),
        ],
    );

    sealed_table(entry).expect("guarded recursion seals through its distinct heads");
}

/// An atom versus an application of a distinguishing head is disjoint by block kind.
#[test]
fn atom_versus_application_is_disjoint() {
    let entry = entry_with_forms(vec![
        StructuralForm::pascal_atom(),
        StructuralForm::application(
            APPLICATION_OPERATOR,
            StructuralForm::camel_atom(),
            StructuralForm::pascal_atom(),
        ),
    ]);
    entry
        .validate_disjoint()
        .expect("atom and application are disjoint");
}

#[test]
fn type_alternatives_share_their_initial_trigger_set() {
    let application = StructuralForm::application(
        APPLICATION_OPERATOR,
        StructuralForm::pascal_atom(),
        StructuralForm::pascal_atom(),
    );
    let table = sealed_table(entry_with_forms(vec![
        StructuralForm::Atom(AtomForm::any_case()),
        application,
    ]))
    .expect("structurally distinct alternatives seal");
    let profile = FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile(&table, &profile);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);

    assert_eq!(
        chosen_constructor(
            evaluator
                .decode_text(ScopedEncodedTypeId::fixture(100), "Alpha.Beta", &mut names,)
                .expect("the application trigger is active while alternatives are selected")
        ),
        1
    );
}

/// Sealing is the mandatory proof boundary: no addressed table can contain an
/// unprovable overlap.
#[test]
fn seal_rejects_unprovable_decode_forms() {
    let error = sealed_table(entry_with_forms(vec![
        StructuralForm::pascal_atom(),
        StructuralForm::pascal_atom(),
    ]))
    .expect_err("two PascalCase alternatives overlap");
    assert!(
        matches!(error, TableError::Disjointness(_)),
        "seal reports the typed disjointness failure"
    );
}

/// Literal and unconstrained name-atom alternatives overlap. Sealing fails rather
/// than trying to preserve a keyword category with lexical exclusions.
#[test]
fn literal_and_unconstrained_name_atom_are_rejected() {
    let mut lexicon = NameTable::new(IdentifierNamespace::Fixture);
    let integer = lexicon
        .intern(Name::new("Integer"))
        .expect("intern Integer");
    let entry = entry_with_forms(vec![
        StructuralForm::Literal(integer),
        StructuralForm::Atom(AtomForm::any_case()),
    ]);

    assert!(
        sealed_table(entry).is_err(),
        "sealing rejects literal/name-atom overlap"
    );
}

#[test]
fn canonical_literal_encode_rejects_a_different_atom() {
    let mut lexicon = NameTable::new(IdentifierNamespace::Fixture);
    let expected = lexicon
        .intern(Name::new("ExpectedLiteral"))
        .expect("intern expected literal");
    let other = lexicon
        .intern(Name::new("OtherLiteral"))
        .expect("intern other literal");
    let table = sealed_table(entry_with_forms(vec![StructuralForm::Literal(expected)]))
        .expect("single literal table seals");
    let profile = FixtureBuilder::token_profile();
    let evaluator = StructuralEvaluator::with_profile_and_lexicon(&table, &profile, &lexicon);
    let value = StructuralValue::chosen(0, StructuralValue::Atom(other));

    assert!(matches!(
        evaluator.encode(ScopedEncodedTypeId::fixture(100), &value, &lexicon),
        Err(EncodeError::LiteralMismatch)
    ));
    assert!(matches!(
        evaluator.encode_text(ScopedEncodedTypeId::fixture(100), &value, &lexicon),
        Err(EncodeError::LiteralMismatch)
    ));
}

/// Construction order cannot change an already-proven disjoint decode result.
#[test]
fn disjoint_constructor_order_does_not_change_the_chosen_identifier() {
    let core_type = ScopedEncodedTypeId::fixture(101);
    let atom = StructuralForm::pascal_atom();
    let application = StructuralForm::application(
        APPLICATION_OPERATOR,
        StructuralForm::pascal_atom(),
        StructuralForm::pascal_atom(),
    );
    let table_with_order = |forms: Vec<(u32, StructuralForm)>| {
        let constructors = forms
            .into_iter()
            .map(|(constructor, form)| {
                ConstructorCodec::new(
                    EncodedConstructorId::new(core_type, constructor),
                    vec![form.clone()],
                    form,
                    PositionalSignature::default(),
                )
            })
            .collect();
        sealed_table(StructuralEntry::new(core_type, constructors)).expect("seal")
    };
    let first = table_with_order(vec![(7, atom.clone()), (9, application.clone())]);
    let second = table_with_order(vec![(9, application), (7, atom)]);
    let block = Recognizer::standard()
        .recognize("Integer")
        .expect("recognize")
        .root_object_at(0)
        .expect("root")
        .clone();

    for table in [&first, &second] {
        let mut names = NameTable::new(IdentifierNamespace::Fixture);
        let value = StructuralEvaluator::new(table)
            .decode(core_type, &block, &mut names)
            .expect("decode bare name");
        assert_eq!(chosen_constructor(value), 7);
    }
}

/// The table-level seal expands delegates to prove a wrapper form is disjoint from a
/// delimited alternative, while decoding retains the delegate's constructor wrapper.
#[test]
fn seal_proves_disjointness_through_a_delegate() {
    let reference = ScopedEncodedTypeId::fixture(200);
    let declaration = ScopedEncodedTypeId::fixture(201);
    let reference_entry = StructuralEntry::new(
        reference,
        vec![ConstructorCodec::new(
            EncodedConstructorId::new(reference, 7),
            vec![StructuralForm::pascal_atom()],
            StructuralForm::pascal_atom(),
            PositionalSignature::default(),
        )],
    );
    let newtype = StructuralForm::application(
        APPLICATION_OPERATOR,
        StructuralForm::pascal_atom(),
        StructuralForm::delegate(reference),
    );
    let structure = StructuralForm::application(
        APPLICATION_OPERATOR,
        StructuralForm::pascal_atom(),
        StructuralForm::Delimited {
            boundary: BRACE_BOUNDARY,
            delimiter: Delimiter::Brace,
            sequence: structural_codec::SequenceForm::zero_or_more(StructuralForm::pascal_atom()),
        },
    );
    let declaration_entry = StructuralEntry::new(
        declaration,
        vec![
            ConstructorCodec::new(
                EncodedConstructorId::new(declaration, 0),
                vec![newtype.clone()],
                newtype,
                PositionalSignature::default(),
            ),
            ConstructorCodec::new(
                EncodedConstructorId::new(declaration, 1),
                vec![structure.clone()],
                structure,
                PositionalSignature::default(),
            ),
        ],
    );
    let profile = FixtureBuilder::token_profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload {
            core_universe: structural_codec::FIXTURE_UNIVERSE,
            core_layout_identity: EncodedLayoutIdentity([0; 32]),
            raw_profile_identity: RawProfileIdentity::from_profile(&profile),
            trivia_triggers: TriggerSet::new(vec![WHITESPACE_TRIVIA, COMMENT_TRIVIA]),
            leaf_codec_contracts: Vec::new(),
            entries: BTreeMap::from([
                (reference, reference_entry),
                (declaration, declaration_entry),
            ]),
        },
        &profile,
    )
    .expect("the delegate expands to a Pascal atom, disjoint from a brace");

    let block = Recognizer::standard()
        .recognize("Record.Target")
        .expect("recognize")
        .root_object_at(0)
        .expect("root")
        .clone();
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = StructuralEvaluator::new(&table)
        .decode(declaration, &block, &mut names)
        .expect("decode newtype form");
    let StructuralValue::Chosen { payload, .. } = value else {
        panic!("declaration is constructor-tagged");
    };
    let StructuralValue::Application(_, body) = payload.as_ref() else {
        panic!("declaration is an application");
    };
    assert!(
        matches!(body.as_ref(), StructuralValue::Delegated(inner) if matches!(inner.as_ref(), StructuralValue::Chosen { constructor: 7, .. })),
        "the evaluator retains the delegated reference constructor"
    );
}

fn unconstrained_atom_target(target: ScopedEncodedTypeId) -> StructuralEntry {
    let form = StructuralForm::Atom(AtomForm::any_case());
    StructuralEntry::new(
        target,
        vec![ConstructorCodec::new(
            EncodedConstructorId::new(target, 0),
            vec![form.clone()],
            form,
            PositionalSignature::default(),
        )],
    )
}

fn directed_delegate_entry(
    outer: ScopedEncodedTypeId,
    target: ScopedEncodedTypeId,
    payloads: &[DelegationPayload],
) -> StructuralEntry {
    StructuralEntry::new(
        outer,
        payloads
            .iter()
            .copied()
            .enumerate()
            .map(|(constructor, payload)| {
                let form = StructuralForm::delegate_with_payload(target, payload);
                ConstructorCodec::new(
                    EncodedConstructorId::new(outer, constructor as u32),
                    vec![form.clone()],
                    form,
                    PositionalSignature::new(vec![target]),
                )
            })
            .collect(),
    )
}

/// Typed payloads direct the expected-type position before the target entry is
/// evaluated, so a case-specific delegation chooses the matching constructor.
#[test]
fn payload_directed_position_decodes_as_directed() {
    let target = ScopedEncodedTypeId::fixture(250);
    let outer = ScopedEncodedTypeId::fixture(251);
    let table = sealed_entries(BTreeMap::from([
        (target, unconstrained_atom_target(target)),
        (
            outer,
            directed_delegate_entry(
                outer,
                target,
                &[
                    DelegationPayload::AtomCase(AtomCase::PascalCase),
                    DelegationPayload::AtomCase(AtomCase::CamelCase),
                ],
            ),
        ),
    ]))
    .expect("seal case-directed delegates");
    let evaluator = StructuralEvaluator::new(&table);

    let pascal = Recognizer::standard()
        .recognize("Entry")
        .expect("recognize Pascal atom")
        .root_object_at(0)
        .expect("Pascal root")
        .clone();
    let camel = Recognizer::standard()
        .recognize("entry")
        .expect("recognize camel atom")
        .root_object_at(0)
        .expect("camel root")
        .clone();
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert_eq!(
        chosen_constructor(
            evaluator
                .decode(outer, &pascal, &mut names)
                .expect("Pascal decode")
        ),
        0
    );
    assert_eq!(
        chosen_constructor(
            evaluator
                .decode(outer, &camel, &mut names)
                .expect("camel decode")
        ),
        1
    );
}

/// Encoding applies the same directed payload as decoding. A target value cannot
/// print text the selected delegate would refuse on the way back in.
#[test]
fn payload_directed_position_rejects_nonconforming_encode() {
    let target = ScopedEncodedTypeId::fixture(252);
    let outer = ScopedEncodedTypeId::fixture(253);
    let table = sealed_entries(BTreeMap::from([
        (target, unconstrained_atom_target(target)),
        (
            outer,
            directed_delegate_entry(
                outer,
                target,
                &[DelegationPayload::AtomCase(AtomCase::PascalCase)],
            ),
        ),
    ]))
    .expect("seal case-directed delegate");
    let evaluator = StructuralEvaluator::new(&table);
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let identifier = names.intern(Name::new("entry")).expect("intern camel atom");
    let value = StructuralValue::Chosen {
        constructor: 0,
        payload: Box::new(StructuralValue::Delegated(Box::new(
            StructuralValue::Chosen {
                constructor: 0,
                payload: Box::new(StructuralValue::Atom(identifier)),
            },
        ))),
    };

    assert!(matches!(
        evaluator.encode(outer, &value, &names),
        Err(EncodeError::DelegationPayloadMismatch {
            payload: DelegationPayload::AtomCase(AtomCase::PascalCase)
        })
    ));
}

/// Payload bytes belong to the sealed table pre-image. A changed direction cannot
/// retain a table identity by accident.
#[test]
fn changing_delegation_payload_moves_table_identity() {
    let target = ScopedEncodedTypeId::fixture(260);
    let outer = ScopedEncodedTypeId::fixture(261);
    let table_for = |payload| {
        sealed_entries(BTreeMap::from([
            (target, unconstrained_atom_target(target)),
            (outer, directed_delegate_entry(outer, target, &[payload])),
        ]))
        .expect("seal payload table")
    };

    let pascal = table_for(DelegationPayload::AtomCase(AtomCase::PascalCase));
    let camel = table_for(DelegationPayload::AtomCase(AtomCase::CamelCase));
    assert_ne!(pascal.identity(), camel.identity());
}

/// The seal proof consumes a delegate payload: without the two distinct payload
/// constraints, these two delegates expand to the same unconstrained target and
/// overlap; with them, the atom cases prove disjoint.
#[test]
fn disjointness_prover_consumes_delegation_payloads() {
    let target = ScopedEncodedTypeId::fixture(270);
    let outer = ScopedEncodedTypeId::fixture(271);
    let directed = directed_delegate_entry(
        outer,
        target,
        &[
            DelegationPayload::AtomCase(AtomCase::PascalCase),
            DelegationPayload::AtomCase(AtomCase::CamelCase),
        ],
    );
    sealed_entries(BTreeMap::from([
        (target, unconstrained_atom_target(target)),
        (outer, directed),
    ]))
    .expect("payload cases prove the delegates disjoint");

    let plain = StructuralEntry::new(
        outer,
        vec![
            ConstructorCodec::new(
                EncodedConstructorId::new(outer, 0),
                vec![StructuralForm::delegate(target)],
                StructuralForm::delegate(target),
                PositionalSignature::new(vec![target]),
            ),
            ConstructorCodec::new(
                EncodedConstructorId::new(outer, 1),
                vec![StructuralForm::delegate(target)],
                StructuralForm::delegate(target),
                PositionalSignature::new(vec![target]),
            ),
        ],
    );
    assert!(
        sealed_entries(BTreeMap::from([
            (target, unconstrained_atom_target(target)),
            (outer, plain),
        ]))
        .is_err(),
        "without payload direction, the same target forms overlap"
    );
}
