//! The grammar and landing catalogs are independent, addressed inputs whose
//! agreement is checked by one shared traversal.

use name_table::{EncodedId, LocalEncodedId, Name};
use raw_discovery::{
    BlockTreeDiscoveryConfiguration, BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
    BoundaryDiscoveryContextIdentifier, RawProfile, SealedTokenProfile, TriggerIdentifier,
    TriggerSet,
};
use structural_codec::{
    AcceptedDecodeForm, AddressedStructuralTable, AtomDescriptor, BorrowedFieldView,
    ConstructorCodec, ContextualTextualPolicy, DecodeFormId, DecodeNameBindings,
    EncodedConstructorId, EncodedNameResolver, EncodedTypeId, FieldEnd, FieldLink, FieldRole,
    FieldValue, FieldVisitor, LandingConstructorDeclaration, LandingDeclarationCatalog,
    LandingFieldDeclaration, LandingShape, LandingTypeDeclaration, LanguageDeclaration,
    LanguageDeclarationError, NameOccurrence, OrderedSequence, Position, ResolvedReference,
    RuleCoproduct, SharedDescriptor, StableRoleId, StructuralEntry, StructuralEvaluator,
    StructuralVocabularyIdentity, StructureRecord, TableIdentityPayload, TargetLayoutIdentity,
    TextualRenderingPolicy,
};

const CARRIER: TriggerIdentifier = TriggerIdentifier::new(4);
const WHITESPACE: TriggerIdentifier = TriggerIdentifier::new(5);
const ROOT_CONTEXT: BoundaryDiscoveryContextIdentifier = BoundaryDiscoveryContextIdentifier::new(1);

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
enum Root {
    Universal,
    Fixed,
}

macro_rules! role {
    ($name:ident, $id:expr) => {
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
        struct $name;

        impl FieldRole for $name {
            const STABLE_ID: u16 = $id;
        }
    };
}

role!(ParentRoot, 1);
role!(ParentKeyword, 2);
role!(ParentName, 3);
role!(ParentChildren, 4);
role!(ChildReference, 5);
role!(AbsentRole, 6);

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct ParentRecord {
    root: Position<ParentRoot, Root>,
    keyword: Position<ParentKeyword, Root>,
    name: Position<ParentName, Root>,
    children: Position<ParentChildren, Root>,
}

impl ParentRecord {
    fn new(keyword: EncodedId<Root>, child: EncodedTypeId<Root>) -> Self {
        Self {
            root: Position::try_new(SharedDescriptor::OrderedSequence(
                OrderedSequence::try_new::<ParentKeyword>()
                    .and_then(OrderedSequence::then::<ParentName>)
                    .and_then(OrderedSequence::then::<ParentChildren>)
                    .expect("distinct positional roles"),
            ))
            .expect("root role"),
            keyword: Position::try_new(SharedDescriptor::Literal(keyword)).expect("keyword role"),
            name: Position::try_new(SharedDescriptor::Declaration(AtomDescriptor::any_case()))
                .expect("name role"),
            children: Position::try_new(SharedDescriptor::Repeated {
                minimum: 1,
                maximum: None,
                element: Box::new(SharedDescriptor::Delegate {
                    target: child,
                    payload: None,
                }),
            })
            .expect("children role"),
        }
    }
}

struct ParentView<'a>(&'a ParentRecord);

impl BorrowedFieldView<Root> for ParentView<'_> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        visitor.field(&self.0.root);
        visitor.field(&self.0.keyword);
        visitor.field(&self.0.name);
        visitor.field(&self.0.children);
    }
}

impl StructureRecord<Root> for ParentRecord {
    type View<'a> = ParentView<'a>;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        ParentView(self)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct ChildRecord {
    reference: Position<ChildReference, Root>,
}

impl ChildRecord {
    fn new() -> Self {
        Self {
            reference: Position::try_new(SharedDescriptor::Carrier {
                carrier: CARRIER,
                content: Box::new(SharedDescriptor::Reference(AtomDescriptor::any_case())),
            })
            .expect("carried reference role"),
        }
    }
}

impl StructureRecord<Root> for ChildRecord {
    type View<'a> = FieldLink<'a, ChildReference, Root, FieldEnd>;

    fn root_role(&self) -> StableRoleId {
        self.reference.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.reference, FieldEnd)
    }
}

type Rule = RuleCoproduct<ParentRecord, ChildRecord>;

struct Fixture {
    table: AddressedStructuralTable<Root, Rule>,
    parent: EncodedTypeId<Root>,
    child: EncodedTypeId<Root>,
    other: EncodedTypeId<Root>,
}

impl Fixture {
    fn new() -> Self {
        let parent = type_id(Root::Universal, &[1]);
        let child = type_id(Root::Universal, &[2]);
        let other = type_id(Root::Universal, &[3]);
        let keyword = encoded(Root::Fixed, &[9]);
        let profile = profile();
        let table = AddressedStructuralTable::seal(
            TableIdentityPayload::new(
                TargetLayoutIdentity::derive(b"landing declaration witness"),
                profile.identity(),
                StructuralVocabularyIdentity::language(b"landing declaration grammar"),
                discovery(),
                rendering(),
                vec![
                    entry(
                        parent.clone(),
                        Rule::Left(ParentRecord::new(keyword, child.clone())),
                    ),
                    entry(child.clone(), Rule::Right(ChildRecord::new())),
                ],
            ),
            &profile,
        )
        .expect("sealed fixture grammar");
        Self {
            table,
            parent,
            child,
            other,
        }
    }

    fn catalog(
        &self,
        parent_fields: Vec<LandingFieldDeclaration<Root>>,
        child_fields: Vec<LandingFieldDeclaration<Root>>,
    ) -> LandingDeclarationCatalog<Root> {
        LandingDeclarationCatalog::try_new(vec![
            declaration(&self.parent, parent_fields),
            declaration(&self.child, child_fields),
        ])
        .expect("well-formed addressed catalog")
    }

    fn matching_catalog(&self) -> LandingDeclarationCatalog<Root> {
        self.catalog(
            vec![
                LandingFieldDeclaration::for_role::<ParentName>(LandingShape::Declaration),
                LandingFieldDeclaration::for_role::<ParentChildren>(LandingShape::sequence(
                    1,
                    None,
                    LandingShape::Type(self.child.clone()),
                )),
            ],
            vec![LandingFieldDeclaration::for_role::<ChildReference>(
                LandingShape::Reference,
            )],
        )
    }
}

fn encoded(root: Root, chain: &[u16]) -> EncodedId<Root> {
    EncodedId::new(
        root,
        chain.iter().copied().map(LocalEncodedId::new).collect(),
    )
    .expect("non-empty fixture identity")
}

fn type_id(root: Root, chain: &[u16]) -> EncodedTypeId<Root> {
    EncodedTypeId::new(encoded(root, chain))
}

fn profile() -> SealedTokenProfile {
    RawProfile::standard().seal().expect("standard profile")
}

fn discovery() -> BlockTreeDiscoveryConfiguration {
    BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            ROOT_CONTEXT,
            vec![BoundaryDiscoveryContext::new(
                ROOT_CONTEXT,
                TriggerSet::new(vec![CARRIER, WHITESPACE]),
            )],
            vec![],
        ),
        vec![],
    )
}

fn rendering() -> TextualRenderingPolicy {
    TextualRenderingPolicy::new(vec![ContextualTextualPolicy::new(
        ROOT_CONTEXT,
        Some(WHITESPACE),
        Some(CARRIER),
    )])
}

fn entry(encoded_type: EncodedTypeId<Root>, rule: Rule) -> StructuralEntry<Root, Rule> {
    let constructor = EncodedConstructorId::under(&encoded_type, 1);
    StructuralEntry::new(
        encoded_type,
        vec![ConstructorCodec::new(
            constructor,
            vec![AcceptedDecodeForm::new(DecodeFormId::new(1), rule.clone())],
            rule,
        )],
    )
}

fn declaration(
    encoded_type: &EncodedTypeId<Root>,
    fields: Vec<LandingFieldDeclaration<Root>>,
) -> LandingTypeDeclaration<Root> {
    LandingTypeDeclaration::new(
        encoded_type.clone(),
        vec![LandingConstructorDeclaration::new(
            EncodedConstructorId::under(encoded_type, 1),
            fields,
        )],
    )
}

struct Bindings {
    identity: EncodedId<Root>,
    spelling: Name,
}

impl EncodedNameResolver<Root> for Bindings {
    fn resolve(&self, encoded_id: &EncodedId<Root>) -> Option<&Name> {
        (encoded_id == &self.identity).then_some(&self.spelling)
    }
}

impl DecodeNameBindings<Root> for Bindings {
    fn declaration_assignment(
        &self,
        _occurrence: NameOccurrence<'_>,
    ) -> Option<structural_codec::DeclarationAssignment<Root>> {
        None
    }

    fn reference_resolution(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Option<ResolvedReference<Root>> {
        (occurrence.spelling() == self.spelling.as_str())
            .then(|| ResolvedReference::new(self.identity.clone()))
    }
}

#[test]
fn addressed_root_verification_follows_only_declared_type_edges() {
    let fixture = Fixture::new();
    let catalog = fixture.matching_catalog();
    let verified = LanguageDeclaration::new(&fixture.table, &catalog)
        .verify_root(&fixture.parent)
        .expect("grammar and landing declarations agree");

    assert_eq!(verified.root(), &fixture.parent);
    assert_eq!(
        verified.addressed_types(),
        &[fixture.parent.clone(), fixture.child.clone()]
    );
}

#[test]
fn carrier_content_remains_a_lookup_only_typed_reference() {
    let fixture = Fixture::new();
    let identity = encoded(Root::Universal, &[77]);
    let bindings = Bindings {
        identity: identity.clone(),
        spelling: Name::new("Feature"),
    };
    let evaluator = StructuralEvaluator::new(&fixture.table).expect("table-owned evaluator");
    let decoded = evaluator
        .decode_text(&fixture.child, "“Feature”", &bindings)
        .expect("typed carrier reference");

    assert_eq!(
        decoded.field::<ChildReference>(),
        Some(&FieldValue::Carrier(Box::new(FieldValue::Reference(
            ResolvedReference::new(identity)
        ))))
    );
    assert_eq!(
        evaluator
            .encode_text(&fixture.child, &decoded, &bindings)
            .expect("canonical carrier encoding"),
        "“Feature”"
    );
}

#[test]
fn disagreement_refusals_are_typed_before_decode() {
    let fixture = Fixture::new();

    let missing_role = fixture.catalog(
        vec![LandingFieldDeclaration::for_role::<ParentName>(
            LandingShape::Declaration,
        )],
        vec![LandingFieldDeclaration::for_role::<ChildReference>(
            LandingShape::Reference,
        )],
    );
    assert!(matches!(
        LanguageDeclaration::new(&fixture.table, &missing_role).verify_root(&fixture.parent),
        Err(LanguageDeclarationError::UndeclaredSemanticRole { role, .. })
            if role == LandingFieldDeclaration::<Root>::for_role::<ParentChildren>(
                LandingShape::Declaration
            ).role()
    ));

    let extra_role = fixture.catalog(
        vec![
            LandingFieldDeclaration::for_role::<ParentName>(LandingShape::Declaration),
            LandingFieldDeclaration::for_role::<ParentChildren>(LandingShape::sequence(
                1,
                None,
                LandingShape::Type(fixture.child.clone()),
            )),
            LandingFieldDeclaration::for_role::<AbsentRole>(LandingShape::Reference),
        ],
        vec![LandingFieldDeclaration::for_role::<ChildReference>(
            LandingShape::Reference,
        )],
    );
    assert!(matches!(
        LanguageDeclaration::new(&fixture.table, &extra_role).verify_root(&fixture.parent),
        Err(LanguageDeclarationError::MissingGrammarRole { role, .. })
            if role == LandingFieldDeclaration::<Root>::for_role::<AbsentRole>(
                LandingShape::Reference
            ).role()
    ));

    let wrong_delegate = fixture.catalog(
        vec![
            LandingFieldDeclaration::for_role::<ParentName>(LandingShape::Declaration),
            LandingFieldDeclaration::for_role::<ParentChildren>(LandingShape::sequence(
                1,
                None,
                LandingShape::Type(fixture.other.clone()),
            )),
        ],
        vec![LandingFieldDeclaration::for_role::<ChildReference>(
            LandingShape::Reference,
        )],
    );
    assert!(matches!(
        LanguageDeclaration::new(&fixture.table, &wrong_delegate).verify_root(&fixture.parent),
        Err(LanguageDeclarationError::DelegateTargetMismatch { expected, found, .. })
            if expected == fixture.other && found == fixture.child
    ));

    let wrong_cardinality = fixture.catalog(
        vec![
            LandingFieldDeclaration::for_role::<ParentName>(LandingShape::Declaration),
            LandingFieldDeclaration::for_role::<ParentChildren>(LandingShape::sequence(
                0,
                Some(1),
                LandingShape::Type(fixture.child.clone()),
            )),
        ],
        vec![LandingFieldDeclaration::for_role::<ChildReference>(
            LandingShape::Reference,
        )],
    );
    assert!(matches!(
        LanguageDeclaration::new(&fixture.table, &wrong_cardinality).verify_root(&fixture.parent),
        Err(LanguageDeclarationError::CardinalityMismatch {
            expected_minimum: 0,
            expected_maximum: Some(1),
            found_minimum: 1,
            found_maximum: None,
            ..
        })
    ));
}
