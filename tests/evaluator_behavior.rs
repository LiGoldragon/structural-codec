use std::collections::BTreeMap;
use std::marker::PhantomData;

use crate::fixture::{
    APPLICATION_OPERATOR, BRACE_BOUNDARY, COMMIT_SEQUENCE, FixtureBuilder, SQUARE_BOUNDARY,
};
use crate::{
    AcceptedDecodeForm, AddressedStructuralTable, ApplicationDelimitedHead, ApplicationRule,
    AtomCase, AtomDescriptor, ConstructorCodec, DecodeError, DecodeFormId, EncodedConstructorId,
    EncodedLanguage, FieldEnd, FieldLink, FieldRole, FieldValue, Position, RuleCoproduct,
    ScopedEncodedTypeId, SharedDescriptor, StableRoleId, StructuralEntry, StructuralEvaluator,
    StructuralRule, StructuralVocabularyIdentity, StructureRecord, TableIdentityPayload,
    TargetLayoutIdentity, UnaryRule,
};
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

const TYPE_REFERENCE: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf101);
const INTERFACE_VARIANT: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf102);
const INTERFACE: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf103);
const DECLARATION: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf104);
const DECLARATIONS: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf105);
const NESTED_INTERFACES: ScopedEncodedTypeId = ScopedEncodedTypeId::fixture_schema(0xf106);

macro_rules! repeated_role {
    ($name:ident, $stable_id:expr) => {
        #[derive(
            rkyv::Archive,
            rkyv::Serialize,
            rkyv::Deserialize,
            Clone,
            Copy,
            Debug,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
        )]
        struct $name(PhantomData<()>);

        impl FieldRole for $name {
            const STABLE_ID: u16 = $stable_id;
        }
    };
}

repeated_role!(DelimitedRoot, 1001);
repeated_role!(DelimitedItems, 1002);

/// A downstream-authored typed source table shape: a delimited repeated field
/// with no fixed product positions.  Building `items` before `root` mirrors
/// the source-table construction that exposed the cursor ownership failure.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct DelimitedRule {
    root: Position<DelimitedRoot>,
    items: Position<DelimitedItems>,
}

impl DelimitedRule {
    fn new(boundary: raw_discovery::TriggerIdentifier, element: SharedDescriptor) -> Self {
        let items = Position::try_new(SharedDescriptor::Repeated {
            minimum: 0,
            maximum: None,
            element: Box::new(element),
        })
        .expect("typed repeated items");
        let root = Position::try_new(SharedDescriptor::Delimited {
            boundary,
            content: items.role(),
        })
        .expect("typed delimited root");
        Self { root, items }
    }
}

impl StructureRecord for DelimitedRule {
    type View<'record> =
        FieldLink<'record, DelimitedRoot, FieldLink<'record, DelimitedItems, FieldEnd>>;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.root, FieldLink::new(&self.items, FieldEnd))
    }
}

type RepeatedRules = RuleCoproduct<DelimitedRule, StructuralRule>;

fn unary_rule(descriptor: SharedDescriptor) -> RepeatedRules {
    RepeatedRules::Right(StructuralRule::Unary(
        UnaryRule::new(descriptor).expect("built-in unary role"),
    ))
}

fn application_rule(head: SharedDescriptor, payload: SharedDescriptor) -> RepeatedRules {
    RepeatedRules::Right(StructuralRule::Application(
        ApplicationRule::new(APPLICATION_OPERATOR, head, payload)
            .expect("built-in application roles"),
    ))
}

fn repeated_entry(
    type_id: ScopedEncodedTypeId,
    rule: RepeatedRules,
) -> StructuralEntry<RepeatedRules> {
    StructuralEntry::new(
        type_id,
        vec![ConstructorCodec::new(
            EncodedConstructorId::fixture_schema(type_id, 1),
            vec![AcceptedDecodeForm::new(DecodeFormId::new(1), rule.clone())],
            rule,
        )],
    )
}

fn repeated_source_table() -> AddressedStructuralTable<RepeatedRules> {
    let profile = FixtureBuilder::token_profile();
    let pascal = || SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase));
    let type_reference = unary_rule(pascal());
    let interface_variant = application_rule(
        pascal(),
        SharedDescriptor::Delegate {
            target: TYPE_REFERENCE,
            payload: None,
        },
    );
    let interface = RepeatedRules::Left(DelimitedRule::new(
        SQUARE_BOUNDARY,
        SharedDescriptor::Delegate {
            target: INTERFACE_VARIANT,
            payload: None,
        },
    ));
    let declaration = application_rule(
        pascal(),
        SharedDescriptor::Delegate {
            target: TYPE_REFERENCE,
            payload: None,
        },
    );
    let declarations = RepeatedRules::Left(DelimitedRule::new(
        BRACE_BOUNDARY,
        SharedDescriptor::Delegate {
            target: DECLARATION,
            payload: None,
        },
    ));
    let nested_interfaces = RepeatedRules::Left(DelimitedRule::new(
        SQUARE_BOUNDARY,
        SharedDescriptor::Delegate {
            target: INTERFACE,
            payload: None,
        },
    ));

    let entries = [
        repeated_entry(TYPE_REFERENCE, type_reference),
        repeated_entry(INTERFACE_VARIANT, interface_variant),
        repeated_entry(INTERFACE, interface),
        repeated_entry(DECLARATION, declaration),
        repeated_entry(DECLARATIONS, declarations),
        repeated_entry(NESTED_INTERFACES, nested_interfaces),
    ]
    .into_iter()
    .map(|entry| (entry.encoded_type(), entry))
    .collect::<BTreeMap<_, _>>();

    AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            EncodedLanguage::Schema,
            TargetLayoutIdentity::derive(b"repeated separator source-table layout"),
            profile.identity(),
            StructuralVocabularyIdentity::fixture(b"repeated separator source-table vocabulary"),
            FixtureBuilder::block_discovery(),
            FixtureBuilder::textual_rendering(),
            entries,
        ),
        &profile,
    )
    .expect("source table seals with its expected forms disjoint")
}

fn repeated_len(value: &crate::StructuralValue) -> usize {
    let Some(FieldValue::Repeated(values)) = value.field::<DelimitedItems>() else {
        panic!("the typed delimited source rule stores its items as a repeated field");
    };
    values.len()
}

#[test]
fn repeated_separator_stays_at_the_element_boundary_in_a_typed_source_table() {
    let table = repeated_source_table();
    let evaluator = StructuralEvaluator::new(&table).expect("source table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);

    // The delegated application is independently valid, as is one element.
    evaluator
        .decode_text(INTERFACE_VARIANT, "Record.RecordPayload", &mut names)
        .expect("delegated application");
    assert_eq!(
        repeated_len(
            &evaluator
                .decode_text(INTERFACE, "[Record.RecordPayload]", &mut names)
                .expect("single repeated element"),
        ),
        1
    );

    // A delegated application and a product-shaped declaration each retain
    // their separator for the repeated loop to consume exactly once.
    let interfaces = evaluator
        .decode_text(
            INTERFACE,
            "[Record.RecordPayload Observe.ObservePayload]",
            &mut names,
        )
        .expect("two delegated applications");
    assert_eq!(repeated_len(&interfaces), 2);
    assert_eq!(
        evaluator
            .encode_text(INTERFACE, &interfaces, &names)
            .expect("canonical repeated application rendering"),
        "[Record.RecordPayload Observe.ObservePayload]"
    );

    assert_eq!(
        repeated_len(
            &evaluator
                .decode_text(DECLARATIONS, "{Note.String Thing.String}", &mut names)
                .expect("two repeated product fields"),
        ),
        2
    );

    assert_eq!(
        repeated_len(
            &evaluator
                .decode_text(
                    NESTED_INTERFACES,
                    "[[Record.RecordPayload Observe.ObservePayload] [Note.String Thing.String]]",
                    &mut names,
                )
                .expect("nested repeated lists"),
        ),
        2
    );

    // A newline is trivia but not a space separator.  Input acceptance remains
    // context-owned; rendering still follows the table's canonical whitespace
    // policy.
    let newline_separated = evaluator
        .decode_text(
            INTERFACE,
            "[Record.RecordPayload\nObserve.ObservePayload]",
            &mut names,
        )
        .expect("non-space trivia separator");
    assert_eq!(repeated_len(&newline_separated), 2);
    assert_eq!(
        evaluator
            .encode_text(INTERFACE, &newline_separated, &names)
            .expect("canonical rendering remains policy-owned"),
        "[Record.RecordPayload Observe.ObservePayload]"
    );

    // Neither an absent nor a malformed separator may be treated as an
    // element boundary.  A failed source must also leave the caller's names
    // untouched, through the evaluator's existing transaction boundary.
    let names_before_refusal = names.len();
    for source in [
        "[Record.RecordPayloadObserve.ObservePayload]",
        "[Record.RecordPayload,Observe.ObservePayload]",
    ] {
        assert!(matches!(
            evaluator.decode_text(INTERFACE, source, &mut names),
            Err(DecodeError::NoAlternative { core_type }) if core_type == INTERFACE
        ));
        assert_eq!(names.len(), names_before_refusal);
    }
}
