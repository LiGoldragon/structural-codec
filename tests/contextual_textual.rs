//! Source-route regressions for context-bounded cursor evaluation.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use name_table::{IdentifierNamespace, NameTable};
use raw_discovery::{
    BlockTreeDiscoveryConfiguration, BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
    BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryTransition, CharacterSet, ProfileRevision,
    TokenProfileData, Trigger, TriggerDefinition, TriggerIdentifier, TriggerSet,
};

use crate::{
    AcceptedDecodeForm, AddressedStructuralTable, AtomDescriptor, ConstructorCodec,
    ContextualTextualPolicy, DecodeFormId, EncodedConstructorId, EncodedLanguage, FieldEnd,
    FieldLink, FieldRole, FieldValue, LeafCodec, Position, RuleCoproduct, ScopedEncodedTypeId,
    SharedDescriptor, StableRoleId, StructuralEntry, StructuralEvaluator,
    StructuralVocabularyIdentity, StructureRecord, TableIdentityPayload, TargetLayoutIdentity,
    TextualRenderingPolicy,
};

const VALUE: ScopedEncodedTypeId = ScopedEncodedTypeId::schema(0x9610);
const TEXT: ScopedEncodedTypeId = ScopedEncodedTypeId::schema(0x9611);
const OUTER: TriggerIdentifier = TriggerIdentifier::new(20);
const DOT: TriggerIdentifier = TriggerIdentifier::new(21);
const ROOT_CARRIER_A: TriggerIdentifier = TriggerIdentifier::new(22);
const ROOT_CARRIER_B: TriggerIdentifier = TriggerIdentifier::new(23);
const ROOT_COMMENT: TriggerIdentifier = TriggerIdentifier::new(24);
const ROOT_WHITESPACE: TriggerIdentifier = TriggerIdentifier::new(25);
const CHILD_CARRIER: TriggerIdentifier = TriggerIdentifier::new(26);
const CHILD_COMMENT: TriggerIdentifier = TriggerIdentifier::new(27);
const CHILD_WHITESPACE: TriggerIdentifier = TriggerIdentifier::new(28);

const ROOT: BoundaryDiscoveryContextIdentifier = BoundaryDiscoveryContextIdentifier::new(81);
const CHILD: BoundaryDiscoveryContextIdentifier = BoundaryDiscoveryContextIdentifier::new(82);

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
struct ProductRoot(PhantomData<()>);
impl FieldRole for ProductRoot {
    const STABLE_ID: u16 = 301;
}

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
struct ProductHead(PhantomData<()>);
impl FieldRole for ProductHead {
    const STABLE_ID: u16 = 302;
}

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
struct ProductPayload(PhantomData<()>);
impl FieldRole for ProductPayload {
    const STABLE_ID: u16 = 303;
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct ProductRule {
    root: Position<ProductRoot>,
    head: Position<ProductHead>,
    payload: Position<ProductPayload>,
}

impl ProductRule {
    fn new() -> Self {
        Self {
            root: Position::try_new(SharedDescriptor::Application {
                operator: DOT,
                head: StableRoleId::for_role::<ProductHead>(),
                payload: StableRoleId::for_role::<ProductPayload>(),
            })
            .expect("typed root"),
            head: Position::try_new(SharedDescriptor::Atom(AtomDescriptor::any_case()))
                .expect("typed head"),
            payload: Position::try_new(SharedDescriptor::Leaf(LeafCodec::Text))
                .expect("typed payload"),
        }
    }
}

impl StructureRecord for ProductRule {
    type View<'record> = FieldLink<
        'record,
        ProductRoot,
        FieldLink<'record, ProductHead, FieldLink<'record, ProductPayload, FieldEnd>>,
    >;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(
            &self.root,
            FieldLink::new(&self.head, FieldLink::new(&self.payload, FieldEnd)),
        )
    }
}

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
struct ItemRoot(PhantomData<()>);
impl FieldRole for ItemRoot {
    const STABLE_ID: u16 = 304;
}

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
struct ItemValues(PhantomData<()>);
impl FieldRole for ItemValues {
    const STABLE_ID: u16 = 305;
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct ItemRule {
    root: Position<ItemRoot>,
    values: Position<ItemValues>,
}

impl ItemRule {
    fn new() -> Self {
        Self {
            root: Position::try_new(SharedDescriptor::ItemBoundary {
                boundary: OUTER,
                content: StableRoleId::for_role::<ItemValues>(),
            })
            .expect("typed item root"),
            values: Position::try_new(SharedDescriptor::Repeated {
                minimum: 1,
                maximum: None,
                element: Box::new(SharedDescriptor::Delegate {
                    target: TEXT,
                    payload: None,
                }),
            })
            .expect("typed repeated values"),
        }
    }
}

impl StructureRecord for ItemRule {
    type View<'record> = FieldLink<'record, ItemRoot, FieldLink<'record, ItemValues, FieldEnd>>;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.root, FieldLink::new(&self.values, FieldEnd))
    }
}

type CustomRules = RuleCoproduct<ProductRule, ItemRule>;
type Rules = RuleCoproduct<CustomRules, crate::StructuralRule>;

fn profile() -> raw_discovery::SealedTokenProfile {
    TokenProfileData::new(
        ProfileRevision::new(91),
        vec![
            TriggerDefinition {
                identifier: OUTER,
                trigger: Trigger::Boundary {
                    opening: "{".to_owned(),
                    closing: "}".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: DOT,
                trigger: Trigger::Application {
                    glyph: ".".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: ROOT_CARRIER_A,
                trigger: Trigger::Carrier {
                    opening: "(|".to_owned(),
                    closing: "|)".to_owned(),
                    escape: None,
                },
            },
            TriggerDefinition {
                identifier: ROOT_CARRIER_B,
                trigger: Trigger::Carrier {
                    opening: "[[|".to_owned(),
                    closing: "|]]".to_owned(),
                    escape: None,
                },
            },
            TriggerDefinition {
                identifier: ROOT_COMMENT,
                trigger: Trigger::LineComment {
                    opening: "//".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: ROOT_WHITESPACE,
                trigger: Trigger::Whitespace {
                    canonical_spelling: "\t".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: CHILD_CARRIER,
                trigger: Trigger::Carrier {
                    opening: "[|".to_owned(),
                    closing: "|]".to_owned(),
                    escape: None,
                },
            },
            TriggerDefinition {
                identifier: CHILD_COMMENT,
                trigger: Trigger::LineComment {
                    opening: "##".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: CHILD_WHITESPACE,
                trigger: Trigger::Whitespace {
                    canonical_spelling: "\n".to_owned(),
                },
            },
        ],
        TriggerSet::new(vec![
            OUTER,
            ROOT_CARRIER_A,
            ROOT_CARRIER_B,
            ROOT_COMMENT,
            ROOT_WHITESPACE,
        ]),
        CharacterSet::from_text(""),
    )
    .seal()
    .expect("custom profile")
}

fn discovery(reverse_root_carriers: bool) -> BlockTreeDiscoveryConfiguration {
    let carriers = if reverse_root_carriers {
        vec![ROOT_CARRIER_B, ROOT_CARRIER_A]
    } else {
        vec![ROOT_CARRIER_A, ROOT_CARRIER_B]
    };
    let mut root = vec![OUTER, ROOT_COMMENT, ROOT_WHITESPACE];
    root.extend(carriers);
    BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            ROOT,
            vec![
                BoundaryDiscoveryContext::new(ROOT, TriggerSet::new(root)),
                BoundaryDiscoveryContext::new(
                    CHILD,
                    TriggerSet::new(vec![CHILD_CARRIER, CHILD_COMMENT, CHILD_WHITESPACE]),
                ),
            ],
            vec![BoundaryDiscoveryTransition::new(ROOT, OUTER, CHILD)],
        ),
        vec![],
    )
}

fn table(reverse_root_carriers: bool) -> AddressedStructuralTable<Rules> {
    let profile = profile();
    let product = Rules::Left(CustomRules::Left(ProductRule::new()));
    let item = Rules::Left(CustomRules::Right(ItemRule::new()));
    let text_rule = Rules::Right(crate::StructuralRule::Unary(
        crate::UnaryRule::new(SharedDescriptor::Leaf(LeafCodec::Text)).expect("text rule"),
    ));
    let value_entry = StructuralEntry::new(
        VALUE,
        vec![
            ConstructorCodec::new(
                EncodedConstructorId::under(VALUE, 1),
                vec![AcceptedDecodeForm::new(
                    DecodeFormId::new(1),
                    product.clone(),
                )],
                product,
            ),
            ConstructorCodec::new(
                EncodedConstructorId::under(VALUE, 2),
                vec![AcceptedDecodeForm::new(DecodeFormId::new(2), item.clone())],
                item,
            ),
        ],
    );
    let text_entry = StructuralEntry::new(
        TEXT,
        vec![ConstructorCodec::new(
            EncodedConstructorId::under(TEXT, 1),
            vec![AcceptedDecodeForm::new(
                DecodeFormId::new(3),
                text_rule.clone(),
            )],
            text_rule,
        )],
    );
    AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            EncodedLanguage::Schema,
            TargetLayoutIdentity::derive(b"contextual cursor target layout"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"contextual cursor vocabulary"),
            discovery(reverse_root_carriers),
            TextualRenderingPolicy::new(vec![
                ContextualTextualPolicy::new(ROOT, Some(ROOT_WHITESPACE), Some(ROOT_CARRIER_B)),
                ContextualTextualPolicy::new(CHILD, Some(CHILD_WHITESPACE), Some(CHILD_CARRIER)),
            ]),
            BTreeMap::from([(VALUE, value_entry), (TEXT, text_entry)]),
        ),
        &profile,
    )
    .expect("custom table")
}

#[test]
fn source_cursor_uses_context_local_carriers_trivia_and_explicit_rendering_policy() {
    let canonical = table(false);
    let reordered = table(true);
    assert_eq!(canonical.identity(), reordered.identity());

    let evaluator = StructuralEvaluator::new(&canonical).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Schema);

    // Product and sum branches are downstream-defined typed records, not
    // built-in rule structs. Root carrier A decodes, but policy selects B.
    let product = evaluator
        .decode_text(VALUE, "Head.(|needs a carrier|)", &mut names)
        .expect("custom product source branch");
    assert_eq!(product.constructor().local(), 1);
    assert_eq!(
        evaluator
            .encode_text(VALUE, &product, &names)
            .expect("canonical product"),
        "Head.[[|needs a carrier|]]"
    );

    // Child-only carrier and comment work in the transitioned child context.
    let child = evaluator
        .decode_text(VALUE, "{## child trivia\n[|a b|]\n[|c d|]}", &mut names)
        .expect("item-boundary source branch");
    assert_eq!(child.constructor().local(), 2);
    assert_eq!(
        evaluator
            .encode_text(VALUE, &child, &names)
            .expect("canonical child list"),
        "{[|a b|]\n[|c d|]}"
    );
    let reparsed = evaluator
        .decode_text(
            VALUE,
            &evaluator
                .encode_text(VALUE, &child, &names)
                .expect("canonical child text"),
            &mut names,
        )
        .expect("canonical child reparse");
    assert_eq!(reparsed, child);

    // Root trivia is not silently admitted in the child context: it becomes
    // a text item rather than being discarded.
    let root_comment_inside_child = evaluator
        .decode_text(VALUE, "{//root-is-data\n[|a b|]}", &mut names)
        .expect("root comment remains child data");
    let Some(FieldValue::Delimited(content)) = root_comment_inside_child.field::<ItemRoot>() else {
        panic!("item root is delimited")
    };
    let FieldValue::Repeated(values) = content.as_ref() else {
        panic!("item content repeats")
    };
    assert_eq!(values.len(), 2);

    // The child carrier is not a root carrier. At root it is just the bare
    // payload spelling, rather than an opaque carrier match.
    let root_spelling = evaluator
        .decode_text(VALUE, "Head.[|child-only|]", &mut names)
        .expect("child spelling is only root bare text");
    assert!(matches!(
        root_spelling.field::<ProductPayload>(),
        Some(FieldValue::Scalar(crate::ScalarValue::Text(value))) if value == "[|child-only|]"
    ));
}

#[test]
fn cursor_child_index_is_bounded_without_parent_rescans() {
    let table = table(false);
    let evaluator = StructuralEvaluator::new(&table).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let values = std::iter::repeat_n("[|x y|]", 128)
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("{{{values}}}");
    evaluator
        .decode_text(VALUE, &source, &mut names)
        .expect("one child boundary with many bounded items");
    assert!(
        crate::evaluator::source_cursor_child_index_probes() <= 4,
        "only the root child index is probed; interior text is never split or searched"
    );
}
