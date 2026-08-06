//! Public contract witnesses for translator-issued encoded-ID chains.

use std::cell::Cell;
use std::collections::BTreeMap;

use legacy_name_table::{EncodedId, LocalEncodedId, Name};
use raw_discovery::{
    BlockTreeDiscoveryConfiguration, BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
    BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryTransition, RawProfile,
    SealedTokenProfile, TriggerIdentifier, TriggerSet,
};
use structural_codec::{
    AcceptedDecodeForm, AddressedStructuralTable, AdjacentApplicationDelimitedBody,
    AdjacentApplicationDelimitedHead, AdjacentApplicationDelimitedItems,
    AdjacentApplicationDelimitedRoot, AdjacentApplicationDelimitedRule, ApplicationHead,
    ApplicationPayload, ApplicationRule, AtomCase, AtomDescriptor, BorrowedFieldView,
    ConstructorCodec, ContextualTextualPolicy, DeclarationAssignment, DecodeError, DecodeFormId,
    DecodeNameBindings, EncodeError, EncodedConstructorId, EncodedNameResolver, EncodedTypeId,
    FieldEnd, FieldLink, FieldRole, FieldValue, FieldVisitor, NameOccurrence, OrderedProduct,
    OrderedSequence, PlannedFieldValue, Position, ResolvedReference, RuleCoproduct,
    SharedDescriptor, StableRoleId, StructuralEntry, StructuralEvaluator, StructuralRule,
    StructuralValue, StructuralVocabularyIdentity, StructureRecord, TableError,
    TableIdentityPayload, TargetLayoutIdentity, TextualRenderingPolicy, UnaryRoot, UnaryRule,
};

const SQUARE: TriggerIdentifier = TriggerIdentifier::new(1);
const BRACE: TriggerIdentifier = TriggerIdentifier::new(2);
const APPLICATION: TriggerIdentifier = TriggerIdentifier::new(3);
const WHITESPACE: TriggerIdentifier = TriggerIdentifier::new(5);
const ANGLE: TriggerIdentifier = TriggerIdentifier::new(7);
const ROOT_CONTEXT: BoundaryDiscoveryContextIdentifier = BoundaryDiscoveryContextIdentifier::new(1);
const CHILD_CONTEXT: BoundaryDiscoveryContextIdentifier =
    BoundaryDiscoveryContextIdentifier::new(2);

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
enum FirstFixtureRoot {
    Universal,
    Rust,
}

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
enum SecondFixtureRoot {
    Authored,
    Fixed,
    Future,
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
struct DownstreamDeclarationRole;

impl FieldRole for DownstreamDeclarationRole {
    const STABLE_ID: u16 = 901;
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct DownstreamDeclarationRecord<Root> {
    declaration: Position<DownstreamDeclarationRole, Root>,
}

impl<Root> DownstreamDeclarationRecord<Root> {
    fn new() -> Self {
        Self {
            declaration: Position::try_new(SharedDescriptor::Declaration(
                AtomDescriptor::with_case(AtomCase::PascalCase),
            ))
            .expect("non-zero downstream role"),
        }
    }
}

impl<Root> StructureRecord<Root> for DownstreamDeclarationRecord<Root> {
    type View<'record>
        = FieldLink<'record, DownstreamDeclarationRole, Root, FieldEnd>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.declaration.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.declaration, FieldEnd)
    }
}

macro_rules! fixture_role {
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

fixture_role!(DocumentRootRole, 910);
fixture_role!(ImportsRole, 911);
fixture_role!(InputRole, 912);
fixture_role!(OutputRole, 913);
fixture_role!(TypesRole, 914);
fixture_role!(GenericsRole, 915);
fixture_role!(ImplsRole, 916);
fixture_role!(DelimitedRootRole, 920);
fixture_role!(DelimitedItemsRole, 921);
fixture_role!(SequenceRootRole, 930);
fixture_role!(SequenceKeywordRole, 931);
fixture_role!(SequenceDeclarationRole, 932);
fixture_role!(DerivedFutureRole, 940);
fixture_role!(DelimitedSequenceRootRole, 950);
fixture_role!(DelimitedSequenceContentRole, 951);
fixture_role!(DelimitedSequenceKeywordRole, 952);
fixture_role!(DelimitedSequenceDeclarationRole, 953);
fixture_role!(ApplicationBoundaryRootRole, 960);
fixture_role!(ApplicationBoundaryRepeatedRole, 961);
fixture_role!(ApplicationBoundaryLiteralRole, 962);
fixture_role!(AmbiguousSequenceRootRole, 963);
fixture_role!(AmbiguousSequenceRepeatedRole, 964);
fixture_role!(AmbiguousSequenceRequiredRole, 965);
fixture_role!(AmbiguousSequenceLiteralRole, 966);
fixture_role!(RejectedCandidateRole, 967);
fixture_role!(MultiDelegateRole, 968);

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct MultiDelegateRecord<Root> {
    delegated: Position<MultiDelegateRole, Root>,
}

impl<Root: Clone> MultiDelegateRecord<Root> {
    fn new(left: EncodedTypeId<Root>, right: EncodedTypeId<Root>) -> Self {
        Self {
            delegated: Position::try_new(SharedDescriptor::Alternation(vec![
                SharedDescriptor::Delegate {
                    target: left,
                    payload: None,
                },
                SharedDescriptor::Delegate {
                    target: right,
                    payload: None,
                },
            ]))
            .expect("multi-delegate role"),
        }
    }
}

impl<Root> StructureRecord<Root> for MultiDelegateRecord<Root> {
    type View<'record>
        = FieldLink<'record, MultiDelegateRole, Root, FieldEnd>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.delegated.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.delegated, FieldEnd)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct SixSlotDocumentRecord<Root> {
    document: Position<DocumentRootRole, Root>,
    imports: Position<ImportsRole, Root>,
    input: Position<InputRole, Root>,
    output: Position<OutputRole, Root>,
    types: Position<TypesRole, Root>,
    generics: Position<GenericsRole, Root>,
    impls: Position<ImplsRole, Root>,
}

impl<Root: Clone> SixSlotDocumentRecord<Root> {
    fn new(
        empty_braces: &EncodedTypeId<Root>,
        empty_square: &EncodedTypeId<Root>,
        types_block: &EncodedTypeId<Root>,
    ) -> Self {
        let product = OrderedProduct::try_new::<ImportsRole>()
            .and_then(OrderedProduct::then::<InputRole>)
            .and_then(OrderedProduct::then::<OutputRole>)
            .and_then(OrderedProduct::then::<TypesRole>)
            .and_then(OrderedProduct::then::<GenericsRole>)
            .and_then(OrderedProduct::then::<ImplsRole>)
            .expect("six distinct non-zero product roles");
        let delegate = |target: &EncodedTypeId<Root>| SharedDescriptor::Delegate {
            target: target.clone(),
            payload: None,
        };
        Self {
            document: Position::try_new(SharedDescriptor::OrderedProduct(product))
                .expect("document root role"),
            imports: Position::try_new(delegate(empty_braces)).expect("imports role"),
            input: Position::try_new(delegate(empty_square)).expect("input role"),
            output: Position::try_new(delegate(empty_square)).expect("output role"),
            types: Position::try_new(delegate(types_block)).expect("types role"),
            generics: Position::try_new(delegate(empty_braces)).expect("generics role"),
            impls: Position::try_new(delegate(empty_braces)).expect("impls role"),
        }
    }
}

struct SixSlotDocumentView<'record, Root> {
    record: &'record SixSlotDocumentRecord<Root>,
}

impl<Root> BorrowedFieldView<Root> for SixSlotDocumentView<'_, Root> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        visitor.field(&self.record.document);
        visitor.field(&self.record.imports);
        visitor.field(&self.record.input);
        visitor.field(&self.record.output);
        visitor.field(&self.record.types);
        visitor.field(&self.record.generics);
        visitor.field(&self.record.impls);
    }
}

impl<Root> StructureRecord<Root> for SixSlotDocumentRecord<Root> {
    type View<'record>
        = SixSlotDocumentView<'record, Root>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.document.role()
    }

    fn fields(&self) -> Self::View<'_> {
        SixSlotDocumentView { record: self }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct DelimitedItemsRecord<Root> {
    delimited: Position<DelimitedRootRole, Root>,
    items: Position<DelimitedItemsRole, Root>,
}

impl<Root: Clone> DelimitedItemsRecord<Root> {
    fn new(
        boundary: TriggerIdentifier,
        item: &EncodedTypeId<Root>,
        minimum: u64,
        maximum: Option<u64>,
    ) -> Self {
        let items = Position::try_new(SharedDescriptor::Repeated {
            minimum,
            maximum,
            element: Box::new(SharedDescriptor::Delegate {
                target: item.clone(),
                payload: None,
            }),
        })
        .expect("delimited items role");
        Self {
            delimited: Position::try_new(SharedDescriptor::Delimited {
                boundary,
                content: items.role(),
            })
            .expect("delimited root role"),
            items,
        }
    }
}

struct DelimitedItemsView<'record, Root> {
    record: &'record DelimitedItemsRecord<Root>,
}

impl<Root> BorrowedFieldView<Root> for DelimitedItemsView<'_, Root> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        visitor.field(&self.record.delimited);
        visitor.field(&self.record.items);
    }
}

impl<Root> StructureRecord<Root> for DelimitedItemsRecord<Root> {
    type View<'record>
        = DelimitedItemsView<'record, Root>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.delimited.role()
    }

    fn fields(&self) -> Self::View<'_> {
        DelimitedItemsView { record: self }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct LexicalSequenceRecord<Root> {
    sequence: Position<SequenceRootRole, Root>,
    keyword: Position<SequenceKeywordRole, Root>,
    declaration: Position<SequenceDeclarationRole, Root>,
}

impl<Root: Clone> LexicalSequenceRecord<Root> {
    fn new(keyword: EncodedId<Root>) -> Self {
        let sequence = OrderedSequence::try_new::<SequenceKeywordRole>()
            .and_then(OrderedSequence::then::<SequenceDeclarationRole>)
            .expect("two distinct lexical positions");
        Self {
            sequence: Position::try_new(SharedDescriptor::OrderedSequence(sequence))
                .expect("sequence root"),
            keyword: Position::try_new(SharedDescriptor::Literal(keyword))
                .expect("keyword position"),
            declaration: Position::try_new(SharedDescriptor::Declaration(
                AtomDescriptor::with_case(AtomCase::PascalCase),
            ))
            .expect("declaration position"),
        }
    }
}

struct LexicalSequenceView<'record, Root> {
    record: &'record LexicalSequenceRecord<Root>,
}

impl<Root> BorrowedFieldView<Root> for LexicalSequenceView<'_, Root> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        visitor.field(&self.record.sequence);
        visitor.field(&self.record.keyword);
        visitor.field(&self.record.declaration);
    }
}

impl<Root> StructureRecord<Root> for LexicalSequenceRecord<Root> {
    type View<'record>
        = LexicalSequenceView<'record, Root>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.sequence.role()
    }

    fn fields(&self) -> Self::View<'_> {
        LexicalSequenceView { record: self }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct DerivedFutureRecord<Root> {
    future_or_value: Position<DerivedFutureRole, Root>,
}

impl<Root: Clone> DerivedFutureRecord<Root> {
    fn new(keyword: EncodedId<Root>) -> Self {
        Self {
            future_or_value: Position::try_new(SharedDescriptor::Alternation(vec![
                SharedDescriptor::InlineApplication {
                    operator: APPLICATION,
                    head: Box::new(SharedDescriptor::Literal(keyword)),
                    payload: Box::new(SharedDescriptor::Reference(AtomDescriptor::any_case())),
                },
                SharedDescriptor::Declaration(AtomDescriptor::any_case()),
            ]))
            .expect("derived future role"),
        }
    }
}

impl<Root> StructureRecord<Root> for DerivedFutureRecord<Root> {
    type View<'record>
        = FieldLink<'record, DerivedFutureRole, Root, FieldEnd>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.future_or_value.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.future_or_value, FieldEnd)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct RejectedCandidateRecord<Root> {
    selected: Position<RejectedCandidateRole, Root>,
}

impl<Root: Clone> RejectedCandidateRecord<Root> {
    fn new(
        rejected_payload: EncodedId<Root>,
        selected_head: EncodedId<Root>,
        selected_payload: EncodedId<Root>,
    ) -> Self {
        Self {
            selected: Position::try_new(SharedDescriptor::Alternation(vec![
                SharedDescriptor::InlineApplication {
                    operator: APPLICATION,
                    head: Box::new(SharedDescriptor::Declaration(AtomDescriptor::any_case())),
                    payload: Box::new(SharedDescriptor::Literal(rejected_payload)),
                },
                SharedDescriptor::InlineApplication {
                    operator: APPLICATION,
                    head: Box::new(SharedDescriptor::Literal(selected_head)),
                    payload: Box::new(SharedDescriptor::Literal(selected_payload)),
                },
            ]))
            .expect("rejected candidate role"),
        }
    }
}

impl<Root> StructureRecord<Root> for RejectedCandidateRecord<Root> {
    type View<'record>
        = FieldLink<'record, RejectedCandidateRole, Root, FieldEnd>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.selected.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.selected, FieldEnd)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct GenericDelimitedContentRecord<Root> {
    root: Position<DelimitedSequenceRootRole, Root>,
    content: Position<DelimitedSequenceContentRole, Root>,
    keyword: Position<DelimitedSequenceKeywordRole, Root>,
    declaration: Position<DelimitedSequenceDeclarationRole, Root>,
}

impl<Root: Clone> GenericDelimitedContentRecord<Root> {
    fn new(keyword: EncodedId<Root>) -> Self {
        let content = OrderedSequence::try_new::<DelimitedSequenceKeywordRole>()
            .and_then(OrderedSequence::then::<DelimitedSequenceDeclarationRole>)
            .expect("two content positions");
        let content = Position::try_new(SharedDescriptor::OrderedSequence(content))
            .expect("delimited content");
        Self {
            root: Position::try_new(SharedDescriptor::Delimited {
                boundary: BRACE,
                content: content.role(),
            })
            .expect("delimited root"),
            content,
            keyword: Position::try_new(SharedDescriptor::Literal(keyword))
                .expect("content keyword"),
            declaration: Position::try_new(SharedDescriptor::Declaration(
                AtomDescriptor::any_case(),
            ))
            .expect("content declaration"),
        }
    }
}

struct GenericDelimitedContentView<'record, Root> {
    record: &'record GenericDelimitedContentRecord<Root>,
}

impl<Root> BorrowedFieldView<Root> for GenericDelimitedContentView<'_, Root> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        visitor.field(&self.record.root);
        visitor.field(&self.record.content);
        visitor.field(&self.record.keyword);
        visitor.field(&self.record.declaration);
    }
}

impl<Root> StructureRecord<Root> for GenericDelimitedContentRecord<Root> {
    type View<'record>
        = GenericDelimitedContentView<'record, Root>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        GenericDelimitedContentView { record: self }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct ApplicationBoundaryRecord<Root> {
    root: Position<ApplicationBoundaryRootRole, Root>,
    repeated: Position<ApplicationBoundaryRepeatedRole, Root>,
    literal: Position<ApplicationBoundaryLiteralRole, Root>,
}

impl<Root: Clone> ApplicationBoundaryRecord<Root> {
    fn new(literal: EncodedId<Root>) -> Self {
        let sequence = OrderedSequence::try_new::<ApplicationBoundaryRepeatedRole>()
            .and_then(OrderedSequence::then::<ApplicationBoundaryLiteralRole>)
            .expect("application repetition and following literal roles are distinct");
        Self {
            root: Position::try_new(SharedDescriptor::OrderedSequence(sequence))
                .expect("application boundary sequence"),
            repeated: Position::try_new(SharedDescriptor::Repeated {
                minimum: 0,
                maximum: None,
                element: Box::new(SharedDescriptor::InlineApplication {
                    operator: APPLICATION,
                    head: Box::new(SharedDescriptor::Reference(AtomDescriptor::any_case())),
                    payload: Box::new(SharedDescriptor::Reference(AtomDescriptor::any_case())),
                }),
            })
            .expect("repeated application position"),
            literal: Position::try_new(SharedDescriptor::Literal(literal))
                .expect("following literal position"),
        }
    }
}

struct ApplicationBoundaryView<'record, Root> {
    record: &'record ApplicationBoundaryRecord<Root>,
}

impl<Root> BorrowedFieldView<Root> for ApplicationBoundaryView<'_, Root> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        visitor.field(&self.record.root);
        visitor.field(&self.record.repeated);
        visitor.field(&self.record.literal);
    }
}

impl<Root> StructureRecord<Root> for ApplicationBoundaryRecord<Root> {
    type View<'record>
        = ApplicationBoundaryView<'record, Root>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        ApplicationBoundaryView { record: self }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
struct AmbiguousSequenceRecord<Root> {
    root: Position<AmbiguousSequenceRootRole, Root>,
    repeated: Position<AmbiguousSequenceRepeatedRole, Root>,
    required: Position<AmbiguousSequenceRequiredRole, Root>,
    literal: Position<AmbiguousSequenceLiteralRole, Root>,
}

impl<Root: Clone> AmbiguousSequenceRecord<Root> {
    fn new(keyword: EncodedId<Root>, literal: EncodedId<Root>) -> Self {
        let application = || SharedDescriptor::InlineApplication {
            operator: APPLICATION,
            head: Box::new(SharedDescriptor::Literal(keyword.clone())),
            payload: Box::new(SharedDescriptor::Reference(AtomDescriptor::any_case())),
        };
        let sequence = OrderedSequence::try_new::<AmbiguousSequenceRepeatedRole>()
            .and_then(OrderedSequence::then::<AmbiguousSequenceRequiredRole>)
            .and_then(OrderedSequence::then::<AmbiguousSequenceLiteralRole>)
            .expect("ambiguous repetition and its typed tail roles are distinct");
        Self {
            root: Position::try_new(SharedDescriptor::OrderedSequence(sequence))
                .expect("ambiguous sequence"),
            repeated: Position::try_new(SharedDescriptor::Repeated {
                minimum: 0,
                maximum: None,
                element: Box::new(application()),
            })
            .expect("open repeated applications"),
            required: Position::try_new(application()).expect("required application"),
            literal: Position::try_new(SharedDescriptor::Literal(literal))
                .expect("required tail literal"),
        }
    }
}

struct AmbiguousSequenceView<'record, Root> {
    record: &'record AmbiguousSequenceRecord<Root>,
}

impl<Root> BorrowedFieldView<Root> for AmbiguousSequenceView<'_, Root> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        visitor.field(&self.record.root);
        visitor.field(&self.record.repeated);
        visitor.field(&self.record.required);
        visitor.field(&self.record.literal);
    }
}

impl<Root> StructureRecord<Root> for AmbiguousSequenceRecord<Root> {
    type View<'record>
        = AmbiguousSequenceView<'record, Root>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }

    fn fields(&self) -> Self::View<'_> {
        AmbiguousSequenceView { record: self }
    }
}

type ProductFixtureRule<Root> = RuleCoproduct<
    SixSlotDocumentRecord<Root>,
    RuleCoproduct<DelimitedItemsRecord<Root>, StructuralRule<Root>>,
>;

struct Bindings<Root> {
    declarations: BTreeMap<(usize, usize), (String, EncodedId<Root>)>,
    references: BTreeMap<(usize, usize), (String, EncodedId<Root>)>,
    spellings: BTreeMap<EncodedId<Root>, Name>,
    declaration_queries: Cell<usize>,
    reference_queries: Cell<usize>,
}

impl<Root> Default for Bindings<Root> {
    fn default() -> Self {
        Self {
            declarations: BTreeMap::new(),
            references: BTreeMap::new(),
            spellings: BTreeMap::new(),
            declaration_queries: Cell::new(0),
            reference_queries: Cell::new(0),
        }
    }
}

impl<Root: Clone + Ord> Bindings<Root> {
    fn spelling(&mut self, encoded_id: &EncodedId<Root>, spelling: &str) {
        self.spellings
            .insert(encoded_id.clone(), Name::new(spelling));
    }

    fn declaration(
        &mut self,
        start: usize,
        end: usize,
        spelling: &str,
        encoded_id: &EncodedId<Root>,
    ) {
        self.spelling(encoded_id, spelling);
        self.declarations
            .insert((start, end), (spelling.to_owned(), encoded_id.clone()));
    }

    fn reference(
        &mut self,
        start: usize,
        end: usize,
        spelling: &str,
        encoded_id: &EncodedId<Root>,
    ) {
        self.spelling(encoded_id, spelling);
        self.references
            .insert((start, end), (spelling.to_owned(), encoded_id.clone()));
    }
}

impl<Root: Ord> EncodedNameResolver<Root> for Bindings<Root> {
    fn resolve(&self, encoded_id: &EncodedId<Root>) -> Option<&Name> {
        self.spellings.get(encoded_id)
    }
}

impl<Root: Clone + Ord> DecodeNameBindings<Root> for Bindings<Root> {
    fn declaration_assignment(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Option<DeclarationAssignment<Root>> {
        self.declaration_queries
            .set(self.declaration_queries.get() + 1);
        let key = (occurrence.bound().start(), occurrence.bound().end());
        self.declarations
            .get(&key)
            .filter(|(spelling, _)| spelling == occurrence.spelling())
            .map(|(_, encoded_id)| DeclarationAssignment::new(encoded_id.clone()))
    }

    fn reference_resolution(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Option<ResolvedReference<Root>> {
        self.reference_queries.set(self.reference_queries.get() + 1);
        let key = (occurrence.bound().start(), occurrence.bound().end());
        self.references
            .get(&key)
            .filter(|(spelling, _)| spelling == occurrence.spelling())
            .map(|(_, encoded_id)| ResolvedReference::new(encoded_id.clone()))
    }
}

fn encoded<Root>(root: Root, chain: &[u16]) -> EncodedId<Root> {
    EncodedId::new(
        root,
        chain.iter().copied().map(LocalEncodedId::new).collect(),
    )
    .expect("fixture chains are non-empty")
}

fn type_id<Root>(root: Root, chain: &[u16]) -> EncodedTypeId<Root> {
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
                TriggerSet::new(vec![WHITESPACE]),
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
        None,
    )])
}

fn product_discovery() -> BlockTreeDiscoveryConfiguration {
    let active = TriggerSet::new(vec![SQUARE, BRACE, WHITESPACE]);
    BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            ROOT_CONTEXT,
            vec![
                BoundaryDiscoveryContext::new(ROOT_CONTEXT, active.clone()),
                BoundaryDiscoveryContext::new(CHILD_CONTEXT, active),
            ],
            vec![
                BoundaryDiscoveryTransition::new(ROOT_CONTEXT, SQUARE, CHILD_CONTEXT),
                BoundaryDiscoveryTransition::new(ROOT_CONTEXT, BRACE, CHILD_CONTEXT),
                BoundaryDiscoveryTransition::new(CHILD_CONTEXT, SQUARE, CHILD_CONTEXT),
                BoundaryDiscoveryTransition::new(CHILD_CONTEXT, BRACE, CHILD_CONTEXT),
            ],
        ),
        vec![],
    )
}

fn product_rendering() -> TextualRenderingPolicy {
    TextualRenderingPolicy::new(vec![
        ContextualTextualPolicy::new(ROOT_CONTEXT, Some(WHITESPACE), None),
        ContextualTextualPolicy::new(CHILD_CONTEXT, Some(WHITESPACE), None),
    ])
}

fn angle_discovery() -> BlockTreeDiscoveryConfiguration {
    let active = TriggerSet::new(vec![ANGLE, WHITESPACE]);
    BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            ROOT_CONTEXT,
            vec![
                BoundaryDiscoveryContext::new(ROOT_CONTEXT, active.clone()),
                BoundaryDiscoveryContext::new(CHILD_CONTEXT, active),
            ],
            vec![
                BoundaryDiscoveryTransition::new(ROOT_CONTEXT, ANGLE, CHILD_CONTEXT),
                BoundaryDiscoveryTransition::new(CHILD_CONTEXT, ANGLE, CHILD_CONTEXT),
            ],
        ),
        vec![],
    )
}

fn angle_rendering() -> TextualRenderingPolicy {
    TextualRenderingPolicy::new(vec![
        ContextualTextualPolicy::new(ROOT_CONTEXT, Some(WHITESPACE), None),
        ContextualTextualPolicy::new(CHILD_CONTEXT, Some(WHITESPACE), None),
    ])
}

fn entry<Root: Clone>(
    type_id: EncodedTypeId<Root>,
    rule: StructuralRule<Root>,
) -> StructuralEntry<Root> {
    let constructor = EncodedConstructorId::under(&type_id, 1);
    StructuralEntry::new(
        type_id,
        vec![ConstructorCodec::new(
            constructor,
            vec![AcceptedDecodeForm::new(DecodeFormId::new(1), rule.clone())],
            rule,
        )],
    )
}

fn typed_entry<Root: Clone, Record: Clone>(
    type_id: EncodedTypeId<Root>,
    rule: Record,
) -> StructuralEntry<Root, Record> {
    let constructor = EncodedConstructorId::under(&type_id, 1);
    StructuralEntry::new(
        type_id,
        vec![ConstructorCodec::new(
            constructor,
            vec![AcceptedDecodeForm::new(DecodeFormId::new(1), rule.clone())],
            rule,
        )],
    )
}

fn table<Root>(
    entries: Vec<StructuralEntry<Root>>,
) -> Result<AddressedStructuralTable<Root>, TableError<Root>>
where
    Root: rkyv::Archive + Clone + Ord,
    Root: for<'serialize> rkyv::Serialize<
            rkyv::rancor::Strategy<
                rkyv::ser::Serializer<
                    rkyv::util::AlignedVec,
                    rkyv::ser::allocator::ArenaHandle<'serialize>,
                    rkyv::ser::sharing::Share,
                >,
                rkyv::rancor::Error,
            >,
        >,
{
    let profile = profile();
    AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"chain-aware downstream fixture"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"chain-aware typed records"),
            discovery(),
            rendering(),
            entries,
        ),
        &profile,
    )
}

struct ProductFixture {
    table: AddressedStructuralTable<FirstFixtureRoot, ProductFixtureRule<FirstFixtureRoot>>,
    document: EncodedTypeId<FirstFixtureRoot>,
}

fn product_fixture() -> ProductFixture {
    let document = type_id(FirstFixtureRoot::Universal, &[30]);
    let empty_braces = type_id(FirstFixtureRoot::Universal, &[31]);
    let empty_square = type_id(FirstFixtureRoot::Universal, &[32]);
    let types_block = type_id(FirstFixtureRoot::Universal, &[33]);
    let newtype = type_id(FirstFixtureRoot::Universal, &[34]);

    let document_rule = ProductFixtureRule::Left(SixSlotDocumentRecord::new(
        &empty_braces,
        &empty_square,
        &types_block,
    ));
    let empty_braces_rule = ProductFixtureRule::Right(RuleCoproduct::Left(
        DelimitedItemsRecord::new(BRACE, &newtype, 0, Some(0)),
    ));
    let empty_square_rule = ProductFixtureRule::Right(RuleCoproduct::Left(
        DelimitedItemsRecord::new(SQUARE, &newtype, 0, Some(0)),
    ));
    let types_rule = ProductFixtureRule::Right(RuleCoproduct::Left(DelimitedItemsRecord::new(
        BRACE,
        &newtype,
        1,
        Some(1),
    )));
    let newtype_rule =
        ProductFixtureRule::Right(RuleCoproduct::Right(StructuralRule::Application(
            ApplicationRule::new(
                APPLICATION,
                SharedDescriptor::Declaration(AtomDescriptor::with_case(AtomCase::PascalCase)),
                SharedDescriptor::Reference(AtomDescriptor::with_case(AtomCase::PascalCase)),
            )
            .expect("newtype application roles"),
        )));
    let profile = profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"six sibling typed roots"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"six-slot product vocabulary"),
            product_discovery(),
            product_rendering(),
            vec![
                typed_entry(document.clone(), document_rule),
                typed_entry(empty_braces, empty_braces_rule),
                typed_entry(empty_square, empty_square_rule),
                typed_entry(types_block, types_rule),
                typed_entry(newtype, newtype_rule),
            ],
        ),
        &profile,
    )
    .expect("six-slot product table");
    ProductFixture { table, document }
}

fn exercise_root<Root>(authored_root: Root, fixed_root: Root)
where
    Root: rkyv::Archive + Clone + std::fmt::Debug + Eq + Ord,
    Root: for<'serialize> rkyv::Serialize<
            rkyv::rancor::Strategy<
                rkyv::ser::Serializer<
                    rkyv::util::AlignedVec,
                    rkyv::ser::allocator::ArenaHandle<'serialize>,
                    rkyv::ser::sharing::Share,
                >,
                rkyv::rancor::Error,
            >,
        >,
{
    let declaration_type = type_id(authored_root.clone(), &[1, 2, 3]);
    let literal_type = type_id(fixed_root.clone(), &[9, 5]);
    let literal_word = encoded(fixed_root, &[11, 7, 4]);

    let declaration_rule = StructuralRule::Application(
        ApplicationRule::new(
            APPLICATION,
            SharedDescriptor::Declaration(AtomDescriptor::with_case(AtomCase::PascalCase)),
            SharedDescriptor::Reference(AtomDescriptor::with_case(AtomCase::PascalCase)),
        )
        .expect("typed application roles"),
    );
    let literal_rule = StructuralRule::Unary(
        UnaryRule::new(SharedDescriptor::Literal(literal_word.clone()))
            .expect("typed literal role"),
    );
    let table = table(vec![
        entry(declaration_type.clone(), declaration_rule),
        entry(literal_type.clone(), literal_rule),
    ])
    .expect("table seals");
    let evaluator = StructuralEvaluator::new(&table).expect("shared evaluator");

    let assigned = encoded(authored_root.clone(), &[41, 8, 16]);
    let resolved = encoded(authored_root, &[3, 21]);
    let mut bindings = Bindings::default();
    bindings.declaration(0, 4, "Id16", &assigned);
    bindings.reference(5, 12, "Integer", &resolved);
    bindings.spelling(&literal_word, "struct");

    let value = evaluator
        .decode_text(&declaration_type, "Id16.Integer", &bindings)
        .expect("translator inputs satisfy typed positions");
    match value
        .field::<ApplicationHead>()
        .expect("typed declaration position")
    {
        FieldValue::Declaration(found) => assert_eq!(found.encoded_id(), &assigned),
        other => panic!("unexpected declaration value: {other:?}"),
    }
    match value
        .field::<ApplicationPayload>()
        .expect("typed reference position")
    {
        FieldValue::Reference(found) => assert_eq!(found.encoded_id(), &resolved),
        other => panic!("unexpected reference value: {other:?}"),
    }
    assert_eq!(
        assigned
            .chain()
            .iter()
            .map(|part| part.value())
            .collect::<Vec<_>>(),
        vec![41, 8, 16],
        "the full module-allocated chain survives evaluation"
    );
    assert_eq!(
        evaluator
            .encode_text(&declaration_type, &value, &bindings)
            .expect("encode through same evaluator"),
        "Id16.Integer"
    );

    let literal = evaluator
        .decode_text(&literal_type, "struct", &bindings)
        .expect("fixed-root vocabulary resolves without fallback");
    assert!(matches!(
        literal.field::<structural_codec::UnaryRoot>(),
        Some(FieldValue::Literal(found)) if found == &literal_word
    ));
}

#[test]
fn two_unrelated_root_sets_retain_full_chains_through_one_evaluator() {
    exercise_root(FirstFixtureRoot::Universal, FirstFixtureRoot::Rust);
    exercise_root(SecondFixtureRoot::Authored, SecondFixtureRoot::Fixed);
}

#[test]
fn unresolved_references_refuse_without_any_allocation_surface() {
    let expected = type_id(FirstFixtureRoot::Universal, &[1, 9]);
    let rule = StructuralRule::Application(
        ApplicationRule::new(
            APPLICATION,
            SharedDescriptor::Declaration(AtomDescriptor::any_case()),
            SharedDescriptor::Reference(AtomDescriptor::any_case()),
        )
        .expect("typed roles"),
    );
    let table = table(vec![entry(expected.clone(), rule)]).expect("table");
    let evaluator = StructuralEvaluator::new(&table).expect("evaluator");
    let assigned = encoded(FirstFixtureRoot::Universal, &[8, 13]);
    let mut bindings = Bindings::default();
    bindings.declaration(0, 4, "Name", &assigned);
    let declarations_before = bindings.declarations.clone();
    let references_before = bindings.references.clone();

    assert!(matches!(
        evaluator.decode_text(&expected, "Name.Missing", &bindings),
        Err(DecodeError::UnresolvedReference { bound })
            if bound.start() == 5 && bound.end() == 12
    ));
    assert_eq!(bindings.declarations, declarations_before);
    assert_eq!(bindings.references, references_before);
    assert_eq!(bindings.declaration_queries.get(), 1);
    assert_eq!(bindings.reference_queries.get(), 1);
}

#[test]
fn token_runs_are_longest_match_and_never_split_to_make_a_parse_work() {
    let expected = type_id(FirstFixtureRoot::Universal, &[2, 2]);
    let rule = StructuralRule::Unary(
        UnaryRule::new(SharedDescriptor::Declaration(AtomDescriptor::with_case(
            AtomCase::PascalCase,
        )))
        .expect("typed role"),
    );
    let table = table(vec![entry(expected.clone(), rule)]).expect("table");
    let evaluator = StructuralEvaluator::new(&table).expect("evaluator");
    let id = encoded(FirstFixtureRoot::Universal, &[99, 16]);
    let mut bindings = Bindings::default();
    bindings.declaration(0, 4, "Id16", &id);

    let value = evaluator
        .decode_text(&expected, "Id16", &bindings)
        .expect("whole token accepted");
    assert!(matches!(
        value.field::<structural_codec::UnaryRoot>(),
        Some(FieldValue::Declaration(found)) if found.encoded_id() == &id
    ));

    let mut split_only = Bindings::default();
    split_only.declaration(0, 3, "Id1", &id);
    assert!(matches!(
        evaluator.decode_text(&expected, "Id16", &split_only),
        Err(DecodeError::MissingDeclarationAssignment { bound })
            if bound.start() == 0 && bound.end() == 4
    ));
}

#[test]
fn downstream_typed_record_uses_the_same_shared_evaluator() {
    let expected = type_id(FirstFixtureRoot::Universal, &[6, 4, 2]);
    let record = DownstreamDeclarationRecord::new();
    let application_entry = StructuralEntry::new(
        expected.clone(),
        vec![ConstructorCodec::new(
            EncodedConstructorId::under(&expected, 1),
            vec![AcceptedDecodeForm::new(
                DecodeFormId::new(1),
                record.clone(),
            )],
            record,
        )],
    );
    let profile = profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"downstream custom typed record"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"downstream custom vocabulary"),
            discovery(),
            rendering(),
            vec![application_entry],
        ),
        &profile,
    )
    .expect("custom record table");

    let assigned = encoded(FirstFixtureRoot::Universal, &[55, 34, 21]);
    let mut bindings = Bindings::default();
    bindings.declaration(0, 8, "Sequence", &assigned);
    let value = StructuralEvaluator::new(&table)
        .expect("same evaluator")
        .decode_text(&expected, "Sequence", &bindings)
        .expect("custom typed declaration");
    assert!(matches!(
        value.field::<DownstreamDeclarationRole>(),
        Some(FieldValue::Declaration(found)) if found.encoded_id() == &assigned
    ));
}

#[test]
fn adjacent_angle_application_is_strict_and_reemits_without_dot_or_spacing() {
    let expected = type_id(FirstFixtureRoot::Universal, &[7, 1]);
    let rule = StructuralRule::AdjacentApplicationDelimited(
        AdjacentApplicationDelimitedRule::new(
            ANGLE,
            SharedDescriptor::Reference(AtomDescriptor::with_case(AtomCase::PascalCase)),
            SharedDescriptor::Reference(AtomDescriptor::with_case(AtomCase::PascalCase)),
            1,
            Some(2),
        )
        .expect("adjacent application rule"),
    );
    let constructor = EncodedConstructorId::under(&expected, 1);
    let profile = profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"adjacent angle application layout"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"adjacent angle application vocabulary"),
            angle_discovery(),
            angle_rendering(),
            vec![entry(expected.clone(), rule)],
        ),
        &profile,
    )
    .expect("adjacent angle table");
    let result = encoded(FirstFixtureRoot::Universal, &[7, 2]);
    let vector = encoded(FirstFixtureRoot::Universal, &[7, 3]);
    let error = encoded(FirstFixtureRoot::Universal, &[7, 4]);
    let mut bindings = Bindings::default();
    bindings.reference(0, 6, "Result", &result);
    bindings.reference(7, 13, "Vector", &vector);
    bindings.reference(14, 19, "Error", &error);
    let evaluator = StructuralEvaluator::new(&table).expect("adjacent evaluator");

    let decoded = evaluator
        .decode_text(&expected, "Result<Vector Error>", &bindings)
        .expect("strict bare angle application");
    assert!(matches!(
        decoded.field::<AdjacentApplicationDelimitedHead>(),
        Some(FieldValue::Reference(found)) if found.encoded_id() == &result
    ));
    assert!(matches!(
        decoded.field::<AdjacentApplicationDelimitedItems>(),
        Some(FieldValue::Repeated(items))
            if matches!(items.as_slice(), [
                FieldValue::Reference(first),
                FieldValue::Reference(second),
            ] if first.encoded_id() == &vector && second.encoded_id() == &error)
    ));
    assert!(
        evaluator
            .decode_text(&expected, "Result.<Vector Error>", &bindings)
            .is_err(),
        "the obsolete dot spelling is not a compatibility path"
    );

    let items = FieldValue::Repeated(vec![
        FieldValue::Reference(ResolvedReference::new(vector.clone())),
        FieldValue::Reference(ResolvedReference::new(error.clone())),
    ]);
    let mut reflected = StructuralValue::record(constructor);
    reflected
        .insert::<AdjacentApplicationDelimitedRoot>(FieldValue::OrderedProduct)
        .expect("adjacent root");
    reflected
        .insert::<AdjacentApplicationDelimitedHead>(FieldValue::Reference(ResolvedReference::new(
            result,
        )))
        .expect("adjacent head");
    reflected
        .insert::<AdjacentApplicationDelimitedBody>(FieldValue::Delimited(Box::new(items.clone())))
        .expect("adjacent body");
    reflected
        .insert::<AdjacentApplicationDelimitedItems>(items)
        .expect("adjacent items");
    assert_eq!(
        evaluator
            .encode_text(&expected, &reflected.finish(), &bindings)
            .expect("adjacent angle application encoding"),
        "Result<Vector Error>"
    );
}

#[test]
fn dotted_applications_are_disjoint_from_adjacent_angle_applications() {
    let expected = type_id(FirstFixtureRoot::Universal, &[7, 10]);
    let identity = StructuralRule::Unary(
        UnaryRule::new(SharedDescriptor::Reference(AtomDescriptor::with_case(
            AtomCase::PascalCase,
        )))
        .expect("identity rule"),
    );
    let adjacent = StructuralRule::AdjacentApplicationDelimited(
        AdjacentApplicationDelimitedRule::new(
            ANGLE,
            SharedDescriptor::Reference(AtomDescriptor::with_case(AtomCase::PascalCase)),
            SharedDescriptor::Delegate {
                target: expected.clone(),
                payload: None,
            },
            1,
            None,
        )
        .expect("adjacent angle rule"),
    );
    let dotted = StructuralRule::Application(
        ApplicationRule::new(
            APPLICATION,
            SharedDescriptor::Declaration(AtomDescriptor::with_case(AtomCase::PascalCase)),
            SharedDescriptor::Reference(AtomDescriptor::with_case(AtomCase::PascalCase)),
        )
        .expect("dotted application rule"),
    );
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"dotted versus adjacent application layout"),
            profile().identity(),
            StructuralVocabularyIdentity::language(
                b"dotted versus adjacent application vocabulary",
            ),
            angle_discovery(),
            angle_rendering(),
            vec![StructuralEntry::new(
                expected.clone(),
                vec![
                    ConstructorCodec::new(
                        EncodedConstructorId::under(&expected, 0),
                        vec![AcceptedDecodeForm::new(
                            DecodeFormId::new(0),
                            identity.clone(),
                        )],
                        identity,
                    ),
                    ConstructorCodec::new(
                        EncodedConstructorId::under(&expected, 1),
                        vec![AcceptedDecodeForm::new(
                            DecodeFormId::new(0),
                            adjacent.clone(),
                        )],
                        adjacent,
                    ),
                    ConstructorCodec::new(
                        EncodedConstructorId::under(&expected, 2),
                        vec![AcceptedDecodeForm::new(
                            DecodeFormId::new(0),
                            dotted.clone(),
                        )],
                        dotted,
                    ),
                ],
            )],
        ),
        &profile(),
    )
    .expect("adjacent and dotted outer forms are provably disjoint");
    let result = encoded(FirstFixtureRoot::Universal, &[7, 11]);
    let ordered = encoded(FirstFixtureRoot::Universal, &[7, 12]);
    let sortable = encoded(FirstFixtureRoot::Universal, &[7, 13]);
    let left = encoded(FirstFixtureRoot::Universal, &[7, 14]);
    let mut bindings = Bindings::default();
    bindings.reference(0, 6, "Result", &result);
    bindings.reference(7, 14, "Ordered", &ordered);
    bindings.reference(5, 13, "Sortable", &sortable);
    bindings.reference(0, 4, "Left", &left);
    bindings.declaration(0, 4, "Left", &left);
    let evaluator = StructuralEvaluator::new(&table).expect("application evaluator");

    assert_eq!(
        evaluator
            .decode_text(&expected, "Result<Ordered>", &bindings)
            .expect("adjacent angle application")
            .constructor()
            .local(),
        1
    );
    assert_eq!(
        evaluator
            .decode_text(&expected, "Left.Sortable", &bindings)
            .expect("dotted application")
            .constructor()
            .local(),
        2
    );
}

#[test]
fn adjacent_angle_application_preserves_nested_strict_shape() {
    let expected = type_id(FirstFixtureRoot::Universal, &[7, 5]);
    let identity = StructuralRule::Unary(
        UnaryRule::new(SharedDescriptor::Reference(AtomDescriptor::with_case(
            AtomCase::PascalCase,
        )))
        .expect("identity rule"),
    );
    let application = StructuralRule::AdjacentApplicationDelimited(
        AdjacentApplicationDelimitedRule::new(
            ANGLE,
            SharedDescriptor::Reference(AtomDescriptor::with_case(AtomCase::PascalCase)),
            SharedDescriptor::Delegate {
                target: expected.clone(),
                payload: None,
            },
            1,
            None,
        )
        .expect("recursive adjacent application rule"),
    );
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"nested adjacent angle application layout"),
            profile().identity(),
            StructuralVocabularyIdentity::language(b"nested adjacent angle application vocabulary"),
            angle_discovery(),
            angle_rendering(),
            vec![StructuralEntry::new(
                expected.clone(),
                vec![
                    ConstructorCodec::new(
                        EncodedConstructorId::under(&expected, 0),
                        vec![AcceptedDecodeForm::new(
                            DecodeFormId::new(0),
                            identity.clone(),
                        )],
                        identity,
                    ),
                    ConstructorCodec::new(
                        EncodedConstructorId::under(&expected, 1),
                        vec![AcceptedDecodeForm::new(
                            DecodeFormId::new(0),
                            application.clone(),
                        )],
                        application,
                    ),
                ],
            )],
        ),
        &profile(),
    )
    .expect("nested adjacent angle table");
    let result = encoded(FirstFixtureRoot::Universal, &[7, 6]);
    let vector = encoded(FirstFixtureRoot::Universal, &[7, 7]);
    let ordered = encoded(FirstFixtureRoot::Universal, &[7, 8]);
    let error = encoded(FirstFixtureRoot::Universal, &[7, 9]);
    let mut bindings = Bindings::default();
    bindings.reference(0, 6, "Result", &result);
    bindings.reference(7, 13, "Vector", &vector);
    bindings.reference(14, 21, "Ordered", &ordered);
    bindings.reference(23, 28, "Error", &error);
    let evaluator = StructuralEvaluator::new(&table).expect("nested adjacent evaluator");

    let decoded = evaluator
        .decode_text(&expected, "Result<Vector<Ordered> Error>", &bindings)
        .expect("nested strict bare angle application");
    let Some(FieldValue::Repeated(items)) = decoded.field::<AdjacentApplicationDelimitedItems>()
    else {
        panic!("outer type application keeps its typed argument sequence");
    };
    let [
        FieldValue::Delegated(vector_value),
        FieldValue::Delegated(error_value),
    ] = items.as_slice()
    else {
        panic!("outer type application preserves its two delegated arguments");
    };
    assert_eq!(
        vector_value.constructor(),
        &EncodedConstructorId::under(&expected, 1)
    );
    assert_eq!(
        error_value.constructor(),
        &EncodedConstructorId::under(&expected, 0)
    );
    assert!(matches!(
        vector_value.field::<AdjacentApplicationDelimitedItems>(),
        Some(FieldValue::Repeated(items))
            if matches!(items.as_slice(), [FieldValue::Delegated(ordered_value)]
                if ordered_value.constructor() == &EncodedConstructorId::under(&expected, 0))
    ));
    assert!(
        evaluator
            .decode_text(&expected, "Result.<Vector<Ordered> Error>", &bindings)
            .is_err()
    );
    assert!(
        evaluator
            .decode_text(&expected, "Result<|Vector| Error>", &bindings)
            .is_err()
    );
    assert_eq!(
        evaluator
            .encode_text(&expected, &decoded, &bindings)
            .expect("decoded nested angle application reemits"),
        "Result<Vector<Ordered> Error>"
    );

    let identity_value = |reference| {
        let mut value = StructuralValue::record(EncodedConstructorId::under(&expected, 0));
        value
            .insert::<UnaryRoot>(FieldValue::Reference(ResolvedReference::new(reference)))
            .expect("identity role");
        value.finish()
    };
    let adjacent_value = |head, items| {
        let items = FieldValue::Repeated(items);
        let body = FieldValue::Delimited(Box::new(items.clone()));
        let mut value = StructuralValue::record(EncodedConstructorId::under(&expected, 1));
        value
            .insert::<AdjacentApplicationDelimitedRoot>(FieldValue::OrderedProduct)
            .expect("adjacent root");
        value
            .insert::<AdjacentApplicationDelimitedHead>(FieldValue::Reference(
                ResolvedReference::new(head),
            ))
            .expect("adjacent head");
        value
            .insert::<AdjacentApplicationDelimitedBody>(body)
            .expect("adjacent body");
        value
            .insert::<AdjacentApplicationDelimitedItems>(items)
            .expect("adjacent items");
        value.finish()
    };
    let vector_value = adjacent_value(
        vector.clone(),
        vec![FieldValue::Delegated(Box::new(identity_value(ordered)))],
    );
    let reflected = adjacent_value(
        result,
        vec![
            FieldValue::Delegated(Box::new(vector_value)),
            FieldValue::Delegated(Box::new(identity_value(error))),
        ],
    );
    assert_eq!(
        evaluator
            .encode_text(&expected, &reflected, &bindings)
            .expect("nested adjacent angle encoding"),
        "Result<Vector<Ordered> Error>"
    );
}

#[test]
fn ordered_sequence_decodes_and_encodes_mixed_lexical_positions() {
    let expected = type_id(FirstFixtureRoot::Rust, &[8, 1]);
    let keyword = encoded(FirstFixtureRoot::Rust, &[3]);
    let record = LexicalSequenceRecord::new(keyword.clone());
    let constructor = EncodedConstructorId::under(&expected, 1);
    let entry = StructuralEntry::new(
        expected.clone(),
        vec![ConstructorCodec::new(
            constructor.clone(),
            vec![AcceptedDecodeForm::new(
                DecodeFormId::new(1),
                record.clone(),
            )],
            record,
        )],
    );
    let profile = profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"mixed lexical sequence layout"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"mixed lexical sequence vocabulary"),
            discovery(),
            rendering(),
            vec![entry],
        ),
        &profile,
    )
    .expect("sequence table");
    let declaration = encoded(FirstFixtureRoot::Universal, &[7, 16]);
    let mut bindings = Bindings::default();
    bindings.spelling(&keyword, "pub");
    bindings.declaration(4, 10, "Widget", &declaration);

    let evaluator = StructuralEvaluator::new(&table).expect("sequence evaluator");
    let decoded = evaluator
        .decode_text(&expected, "pub Widget", &bindings)
        .expect("mixed lexical positions");
    assert!(matches!(
        decoded.field::<SequenceKeywordRole>(),
        Some(FieldValue::Literal(found)) if found == &keyword
    ));
    assert!(matches!(
        decoded.field::<SequenceDeclarationRole>(),
        Some(FieldValue::Declaration(found)) if found.encoded_id() == &declaration
    ));

    let mut reflected = StructuralValue::record(constructor);
    reflected
        .insert::<SequenceRootRole>(FieldValue::OrderedProduct)
        .expect("sequence marker");
    reflected
        .insert::<SequenceKeywordRole>(FieldValue::Literal(keyword))
        .expect("keyword");
    reflected
        .insert::<SequenceDeclarationRole>(FieldValue::Declaration(DeclarationAssignment::new(
            declaration,
        )))
        .expect("declaration");
    assert_eq!(
        evaluator
            .encode_text(&expected, &reflected.finish(), &bindings)
            .expect("sequence encoding"),
        "pub Widget"
    );
    assert!(
        evaluator
            .decode_text(&expected, "Widget pub", &bindings)
            .is_err()
    );
}

#[test]
fn descriptor_alternation_decodes_dotted_future_lookup_only_or_literal_declaration() {
    let expected = type_id(FirstFixtureRoot::Universal, &[8, 2]);
    let keyword = encoded(FirstFixtureRoot::Universal, &[8, 3]);
    let record = DerivedFutureRecord::new(keyword.clone());
    let derived_role = record.future_or_value.role();
    let entry = StructuralEntry::new(
        expected.clone(),
        vec![ConstructorCodec::new(
            EncodedConstructorId::under(&expected, 1),
            vec![AcceptedDecodeForm::new(
                DecodeFormId::new(1),
                record.clone(),
            )],
            record,
        )],
    );
    let profile = profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"derived position alternation"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"inline application alternation"),
            discovery(),
            rendering(),
            vec![entry],
        ),
        &profile,
    )
    .expect("derived descriptor table");
    let target = encoded(FirstFixtureRoot::Universal, &[8, 4]);
    let declaration = encoded(FirstFixtureRoot::Universal, &[8, 5]);
    let mut bindings = Bindings::default();
    bindings.spelling(&keyword, "Realize");
    bindings.reference(8, 14, "target", &target);
    bindings.declaration(0, 6, "Widget", &declaration);

    let evaluator = StructuralEvaluator::new(&table).expect("shared evaluator");
    let no_fixed_vocabulary = Bindings::<FirstFixtureRoot>::default();
    assert!(matches!(
        evaluator.plan_text(&expected, "Realize.target", &no_fixed_vocabulary),
        Err(DecodeError::UnknownEncodedName { encoded_id }) if encoded_id == keyword
    ));
    assert_eq!(no_fixed_vocabulary.declaration_queries.get(), 0);
    assert_eq!(no_fixed_vocabulary.reference_queries.get(), 0);

    let future_plan = evaluator
        .plan_text(&expected, "Realize.target", &bindings)
        .expect("allocation-free dotted future plan");
    assert!(matches!(
        future_plan.field::<DerivedFutureRole>(),
        Some(PlannedFieldValue::Application { head, payload })
            if matches!(head.as_ref(), PlannedFieldValue::Literal(found) if found == &keyword)
                && matches!(
                    payload.as_ref(),
                    PlannedFieldValue::Reference(found)
                        if found.spelling().as_str() == "target"
                            && (found.bound().start(), found.bound().end()) == (8, 14)
                )
    ));
    assert_eq!(bindings.declaration_queries.get(), 0);
    assert_eq!(bindings.reference_queries.get(), 0);

    let future = evaluator
        .decode_text(&expected, "Realize.target", &bindings)
        .expect("dotted future");
    assert_eq!(future_plan.constructor(), future.constructor());
    assert!(matches!(
        future.field::<DerivedFutureRole>(),
        Some(FieldValue::Application { head, payload })
            if matches!(head.as_ref(), FieldValue::Literal(found) if found == &keyword)
                && matches!(payload.as_ref(), FieldValue::Reference(found) if found.encoded_id() == &target)
    ));
    assert_eq!(
        future.field_by_role(derived_role),
        future.field::<DerivedFutureRole>(),
        "verified declaration-indexed consumers can read the same role without a Rust type"
    );
    assert_eq!(bindings.declaration_queries.get(), 0);
    assert_eq!(bindings.reference_queries.get(), 1);
    assert_eq!(
        evaluator
            .encode_text(&expected, &future, &bindings)
            .expect("future branch encodes through the same alternation"),
        "Realize.target"
    );

    let literal = evaluator
        .decode_text(&expected, "Widget", &bindings)
        .expect("literal declaration branch");
    assert!(matches!(
        literal.field::<DerivedFutureRole>(),
        Some(FieldValue::Declaration(found)) if found.encoded_id() == &declaration
    ));
    assert_eq!(bindings.declaration_queries.get(), 1);
    assert_eq!(bindings.reference_queries.get(), 1);
    assert_eq!(
        evaluator
            .encode_text(&expected, &literal, &bindings)
            .expect("literal branch encodes through the same alternation"),
        "Widget"
    );

    let literal_plan = evaluator
        .plan_text(&expected, "Widget", &bindings)
        .expect("allocation-free literal declaration plan");
    assert!(matches!(
        literal_plan.field::<DerivedFutureRole>(),
        Some(PlannedFieldValue::Declaration(found))
            if found.spelling().as_str() == "Widget"
                && (found.bound().start(), found.bound().end()) == (0, 6)
    ));
    assert_eq!(literal_plan.constructor(), literal.constructor());
    assert_eq!(
        bindings.declaration_queries.get(),
        1,
        "planning never calls the declaration-assignment boundary"
    );
    assert_eq!(
        bindings.reference_queries.get(),
        1,
        "planning never calls the reference-resolution boundary"
    );
}

#[test]
fn rejected_alternation_candidate_cannot_leak_its_unique_planned_declaration() {
    let expected = type_id(FirstFixtureRoot::Universal, &[8, 20]);
    let rejected_payload = encoded(FirstFixtureRoot::Universal, &[8, 21]);
    let selected_head = encoded(FirstFixtureRoot::Universal, &[8, 22]);
    let selected_payload = encoded(FirstFixtureRoot::Universal, &[8, 23]);
    let record = RejectedCandidateRecord::new(
        rejected_payload.clone(),
        selected_head.clone(),
        selected_payload.clone(),
    );
    let entry = StructuralEntry::new(
        expected.clone(),
        vec![ConstructorCodec::new(
            EncodedConstructorId::under(&expected, 1),
            vec![AcceptedDecodeForm::new(
                DecodeFormId::new(1),
                record.clone(),
            )],
            record,
        )],
    );
    let profile = profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"rejected occurrence isolation"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"alternation candidate rollback"),
            discovery(),
            rendering(),
            vec![entry],
        ),
        &profile,
    )
    .expect("rejected-candidate table");
    let mut bindings = Bindings::default();
    bindings.spelling(&rejected_payload, "Bad");
    bindings.spelling(&selected_head, "Unique");
    bindings.spelling(&selected_payload, "Good");

    let plan = StructuralEvaluator::new(&table)
        .expect("shared evaluator")
        .plan_text(&expected, "Unique.Good", &bindings)
        .expect("second alternation candidate is selected");

    assert!(matches!(
        plan.field::<RejectedCandidateRole>(),
        Some(PlannedFieldValue::Application { head, payload })
            if matches!(
                head.as_ref(),
                PlannedFieldValue::Literal(found) if found == &selected_head
            ) && matches!(
                payload.as_ref(),
                PlannedFieldValue::Literal(found) if found == &selected_payload
            )
    ));
    assert_eq!(
        bindings.declaration_queries.get(),
        0,
        "the rejected candidate's Unique declaration is absent from the selected plan"
    );
    assert_eq!(bindings.reference_queries.get(), 0);
}

#[test]
fn reserved_application_is_disjoint_from_an_excluding_reference_application() {
    let expected = type_id(FirstFixtureRoot::Universal, &[8, 9]);
    let keyword = encoded(FirstFixtureRoot::Universal, &[8, 10]);
    let future = StructuralRule::Application(
        ApplicationRule::new(
            APPLICATION,
            SharedDescriptor::Literal(keyword.clone()),
            SharedDescriptor::Reference(AtomDescriptor::any_case()),
        )
        .expect("future application"),
    );
    let ordinary = StructuralRule::Application(
        ApplicationRule::new(
            APPLICATION,
            SharedDescriptor::ReferenceExcluding {
                atom: AtomDescriptor::any_case(),
                excluded: vec![keyword.clone()],
            },
            SharedDescriptor::Reference(AtomDescriptor::any_case()),
        )
        .expect("ordinary application"),
    );
    let application_entry = StructuralEntry::new(
        expected.clone(),
        vec![
            ConstructorCodec::new(
                EncodedConstructorId::under(&expected, 1),
                vec![AcceptedDecodeForm::new(
                    DecodeFormId::new(1),
                    future.clone(),
                )],
                future,
            ),
            ConstructorCodec::new(
                EncodedConstructorId::under(&expected, 2),
                vec![AcceptedDecodeForm::new(
                    DecodeFormId::new(1),
                    ordinary.clone(),
                )],
                ordinary,
            ),
        ],
    );
    let profile = profile();
    let application_table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"reserved application exclusion"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"identity-level reserved words"),
            discovery(),
            rendering(),
            vec![application_entry],
        ),
        &profile,
    )
    .expect("identity exclusion proves the applications disjoint");
    let target = encoded(FirstFixtureRoot::Universal, &[8, 11]);
    let module = encoded(FirstFixtureRoot::Universal, &[8, 12]);
    let member = encoded(FirstFixtureRoot::Universal, &[8, 13]);
    let mut bindings = Bindings::default();
    bindings.spelling(&keyword, "Realize");
    bindings.reference(8, 14, "target", &target);
    bindings.reference(0, 6, "module", &module);
    bindings.reference(7, 13, "Member", &member);
    let evaluator = StructuralEvaluator::new(&application_table).expect("shared evaluator");

    let decoded = evaluator
        .decode_text(&expected, "Realize.target", &bindings)
        .expect("reserved application");
    assert_eq!(
        decoded.constructor(),
        &EncodedConstructorId::under(&expected, 1)
    );
    let decoded = evaluator
        .decode_text(&expected, "module.Member", &bindings)
        .expect("ordinary reference application");
    assert_eq!(
        decoded.constructor(),
        &EncodedConstructorId::under(&expected, 2)
    );

    let mut excluded = Bindings::default();
    excluded.spelling(&keyword, "Realize");
    excluded.reference(0, 7, "Realize", &keyword);
    let reference_only = type_id(FirstFixtureRoot::Universal, &[8, 14]);
    let reference_entry = entry(
        reference_only.clone(),
        StructuralRule::Unary(
            UnaryRule::new(SharedDescriptor::ReferenceExcluding {
                atom: AtomDescriptor::any_case(),
                excluded: vec![keyword],
            })
            .expect("excluding reference"),
        ),
    );
    let reference_table = table(vec![reference_entry]).expect("reference-only table");
    let reference_evaluator =
        StructuralEvaluator::new(&reference_table).expect("reference evaluator");
    assert!(matches!(
        reference_evaluator.decode_text(&reference_only, "Realize", &excluded),
        Err(DecodeError::NoAlternative { .. })
    ));
    assert_eq!(
        excluded.reference_queries.get(),
        0,
        "reserved identities refuse before lookup-only reference resolution"
    );
}

#[test]
fn delimited_content_can_be_a_fixed_ordered_sequence() {
    let expected = type_id(FirstFixtureRoot::Universal, &[8, 6]);
    let keyword = encoded(FirstFixtureRoot::Universal, &[8, 7]);
    let record = GenericDelimitedContentRecord::new(keyword.clone());
    let constructor = EncodedConstructorId::under(&expected, 1);
    let entry = StructuralEntry::new(
        expected.clone(),
        vec![ConstructorCodec::new(
            constructor,
            vec![AcceptedDecodeForm::new(
                DecodeFormId::new(1),
                record.clone(),
            )],
            record,
        )],
    );
    let profile = profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"generic delimited content"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"ordered delimited content"),
            product_discovery(),
            product_rendering(),
            vec![entry],
        ),
        &profile,
    )
    .expect("generic delimited table");
    let declaration = encoded(FirstFixtureRoot::Universal, &[8, 8]);
    let mut bindings = Bindings::default();
    bindings.spelling(&keyword, "Public");
    bindings.declaration(8, 14, "Widget", &declaration);

    let decoded = StructuralEvaluator::new(&table)
        .expect("shared evaluator")
        .decode_text(&expected, "{Public Widget}", &bindings)
        .expect("ordered sequence inside boundary");
    assert!(matches!(
        decoded.field::<DelimitedSequenceContentRole>(),
        Some(FieldValue::OrderedProduct)
    ));
    assert!(matches!(
        decoded.field::<DelimitedSequenceDeclarationRole>(),
        Some(FieldValue::Declaration(found)) if found.encoded_id() == &declaration
    ));
    assert_eq!(
        StructuralEvaluator::new(&table)
            .expect("shared evaluator")
            .encode_text(&expected, &decoded, &bindings)
            .expect("generic delimited content encodes"),
        "{Public Widget}"
    );
}

#[test]
fn ordered_product_decodes_six_typed_root_blocks_with_absolute_bounds() {
    let fixture = product_fixture();
    let evaluator = StructuralEvaluator::new(&fixture.table).expect("product evaluator");
    let source = "  {}\n []  []\n {Widget.Integer}\n {}\t{}  ";
    let assigned = encoded(FirstFixtureRoot::Universal, &[30, 7]);
    let resolved = encoded(FirstFixtureRoot::Universal, &[3]);
    let mut bindings = Bindings::default();

    let plan = evaluator
        .plan_text(&fixture.document, source, &bindings)
        .expect("allocation-free six-slot plan");
    let Some(PlannedFieldValue::Delegated(types)) = plan.field::<TypesRole>() else {
        panic!("types role retains its delegated structural context");
    };
    let Some(PlannedFieldValue::Repeated(items)) = types.field::<DelimitedItemsRole>() else {
        panic!("types block retains its repeated structural context");
    };
    let [PlannedFieldValue::Delegated(item)] = items.as_slice() else {
        panic!("one selected item retains its constructor and typed roles");
    };
    assert!(matches!(
        item.field::<ApplicationHead>(),
        Some(PlannedFieldValue::Declaration(found))
            if found.spelling().as_str() == "Widget"
                && (found.bound().start(), found.bound().end()) == (15, 21)
    ));
    assert!(matches!(
        item.field::<ApplicationPayload>(),
        Some(PlannedFieldValue::Reference(found))
            if found.spelling().as_str() == "Integer"
                && (found.bound().start(), found.bound().end()) == (22, 29)
    ));
    assert_eq!(bindings.declaration_queries.get(), 0);
    assert_eq!(bindings.reference_queries.get(), 0);
    assert!(
        evaluator
            .plan_text(&fixture.document, "{} [] [] {Widget.} {} {}", &bindings,)
            .is_err(),
        "malformed source yields no partial plan"
    );

    bindings.declaration(15, 21, "Widget", &assigned);
    bindings.reference(22, 29, "Integer", &resolved);

    let decoded = evaluator
        .decode_text_bounded(&fixture.document, source, &bindings)
        .expect("six typed sibling roots");
    for present in [
        matches!(
            decoded.value().field::<ImportsRole>(),
            Some(FieldValue::Delegated(_))
        ),
        matches!(
            decoded.value().field::<InputRole>(),
            Some(FieldValue::Delegated(_))
        ),
        matches!(
            decoded.value().field::<OutputRole>(),
            Some(FieldValue::Delegated(_))
        ),
        matches!(
            decoded.value().field::<TypesRole>(),
            Some(FieldValue::Delegated(_))
        ),
        matches!(
            decoded.value().field::<GenericsRole>(),
            Some(FieldValue::Delegated(_))
        ),
        matches!(
            decoded.value().field::<ImplsRole>(),
            Some(FieldValue::Delegated(_))
        ),
    ] {
        assert!(present, "every product member remains typed and delegated");
    }
    let bounds = [
        decoded.field_bound::<ImportsRole>(),
        decoded.field_bound::<InputRole>(),
        decoded.field_bound::<OutputRole>(),
        decoded.field_bound::<TypesRole>(),
        decoded.field_bound::<GenericsRole>(),
        decoded.field_bound::<ImplsRole>(),
    ]
    .map(|bound| bound.expect("typed field bound"))
    .map(|bound| (bound.start(), bound.end()));
    assert_eq!(
        bounds,
        [(2, 4), (6, 8), (10, 12), (14, 30), (32, 34), (35, 37)]
    );
    assert_eq!(bindings.declaration_queries.get(), 1);
    assert_eq!(bindings.reference_queries.get(), 1);
    assert_eq!(plan.constructor(), decoded.value().constructor());
    assert_eq!(
        evaluator
            .encode_text(&fixture.document, decoded.value(), &bindings)
            .expect("product encoding follows the same typed roles"),
        "{} [] [] {Widget.Integer} {} {}"
    );
}

#[test]
fn ordered_product_refuses_too_few_and_too_many_root_blocks_with_typed_arity() {
    let fixture = product_fixture();
    let evaluator = StructuralEvaluator::new(&fixture.table).expect("product evaluator");
    let bindings = Bindings::<FirstFixtureRoot>::default();

    assert!(matches!(
        evaluator.decode_text(&fixture.document, "{} [] [] {} {}", &bindings),
        Err(DecodeError::ProductArityMismatch {
            expected: 6,
            found: 5
        })
    ));
    assert!(matches!(
        evaluator.decode_text(&fixture.document, "{} [] [] {} {} {} {}", &bindings),
        Err(DecodeError::ProductArityMismatch {
            expected: 6,
            found: 7
        })
    ));
}

#[test]
fn ordered_product_refuses_a_block_that_does_not_match_its_typed_position() {
    let fixture = product_fixture();
    let evaluator = StructuralEvaluator::new(&fixture.table).expect("product evaluator");
    let bindings = Bindings::<FirstFixtureRoot>::default();

    assert!(matches!(
        evaluator.decode_text(&fixture.document, "[] [] [] {} {} {}", &bindings),
        Err(DecodeError::ProductPositionMismatch {
            position: 0,
            role,
            bound
        }) if role.value() == ImportsRole::STABLE_ID
            && bound.start() == 0
            && bound.end() == 2
    ));
}

#[test]
fn repeated_application_boundary_is_proven_before_name_lookup() {
    let expected = type_id(FirstFixtureRoot::Universal, &[10, 20]);
    let private = encoded(FirstFixtureRoot::Universal, &[10, 21]);
    let record = ApplicationBoundaryRecord::new(private.clone());
    let constructor = EncodedConstructorId::under(&expected, 1);
    let application_entry = StructuralEntry::new(
        expected.clone(),
        vec![ConstructorCodec::new(
            constructor,
            vec![AcceptedDecodeForm::new(
                DecodeFormId::new(1),
                record.clone(),
            )],
            record,
        )],
    );
    let profile = profile();
    let application_table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"application operator prelookup boundary"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"repeated applications before a fixed literal"),
            discovery(),
            rendering(),
            vec![application_entry],
        ),
        &profile,
    )
    .expect("application-boundary table");
    let evaluator = StructuralEvaluator::new(&application_table).expect("shared evaluator");

    let mut bindings = Bindings::default();
    bindings.spelling(&private, "Private");
    let decoded = evaluator
        .decode_text(&expected, "Private", &bindings)
        .expect("the following literal terminates the open repetition");
    assert!(matches!(
        decoded.field::<ApplicationBoundaryRepeatedRole>(),
        Some(FieldValue::Repeated(values)) if values.is_empty()
    ));
    assert!(matches!(
        decoded.field::<ApplicationBoundaryLiteralRole>(),
        Some(FieldValue::Literal(found)) if found == &private
    ));
    assert_eq!(
        bindings.reference_queries.get(),
        0,
        "a token without the application cue cannot query the head or payload"
    );
    assert_eq!(bindings.declaration_queries.get(), 0);

    let missing = Bindings::default();
    let missing_result = evaluator.decode_text(&expected, "missing.Member Private", &missing);
    assert!(matches!(
        missing_result,
        Err(DecodeError::SequenceRepetitionBoundary { role, refusal })
            if role.value() == ApplicationBoundaryRepeatedRole::STABLE_ID
                && matches!(
                    refusal.as_ref(),
                    DecodeError::UnresolvedReference { bound }
                        if bound.start() == 0 && bound.end() == 7
                )
    ));
    assert_eq!(
        missing.reference_queries.get(),
        1,
        "once the operator is present, the typed lookup-only failure controls"
    );
    assert_eq!(missing.declaration_queries.get(), 0);
}

#[test]
fn ordered_sequence_chooses_the_greatest_repetition_count_with_a_typed_tail() {
    let expected = type_id(FirstFixtureRoot::Universal, &[10, 30]);
    let keyword = encoded(FirstFixtureRoot::Universal, &[10, 31]);
    let private = encoded(FirstFixtureRoot::Universal, &[10, 32]);
    let record = AmbiguousSequenceRecord::new(keyword.clone(), private.clone());
    let constructor = EncodedConstructorId::under(&expected, 1);
    let sequence_entry = StructuralEntry::new(
        expected.clone(),
        vec![ConstructorCodec::new(
            constructor,
            vec![AcceptedDecodeForm::new(
                DecodeFormId::new(1),
                record.clone(),
            )],
            record,
        )],
    );
    let profile = profile();
    let sequence_table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"greatest viable sequence repetition"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"typed sequence tail backtracking"),
            discovery(),
            rendering(),
            vec![sequence_entry],
        ),
        &profile,
    )
    .expect("ambiguous sequence table");
    let evaluator = StructuralEvaluator::new(&sequence_table).expect("shared evaluator");

    let source = "Realize.first Realize.second Private";
    let first = encoded(FirstFixtureRoot::Universal, &[10, 33]);
    let second = encoded(FirstFixtureRoot::Universal, &[10, 34]);
    let mut bindings = Bindings::default();
    bindings.spelling(&keyword, "Realize");
    bindings.spelling(&private, "Private");
    bindings.reference(8, 13, "first", &first);
    bindings.reference(22, 28, "second", &second);
    let plan = evaluator
        .plan_text(&expected, source, &bindings)
        .expect("allocation-free plan follows the viable typed tail");
    assert!(matches!(
        plan.field::<AmbiguousSequenceRepeatedRole>(),
        Some(PlannedFieldValue::Repeated(values))
            if matches!(
                values.as_slice(),
                [PlannedFieldValue::Application { payload, .. }]
                    if matches!(
                        payload.as_ref(),
                        PlannedFieldValue::Reference(found)
                            if found.spelling().as_str() == "first"
                    )
            )
    ));
    assert!(matches!(
        plan.field::<AmbiguousSequenceRequiredRole>(),
        Some(PlannedFieldValue::Application { payload, .. })
            if matches!(
                payload.as_ref(),
                PlannedFieldValue::Reference(found)
                    if found.spelling().as_str() == "second"
            )
    ));
    assert_eq!(
        bindings.reference_queries.get(),
        0,
        "planning retains only the selected branch and performs no lookup callback"
    );
    let decoded = evaluator
        .decode_text(&expected, source, &bindings)
        .expect("the greatest viable repetition count is one");
    assert_eq!(plan.constructor(), decoded.constructor());
    assert!(matches!(
        decoded.field::<AmbiguousSequenceRepeatedRole>(),
        Some(FieldValue::Repeated(values)) if values.len() == 1
    ));
    assert!(matches!(
        decoded.field::<AmbiguousSequenceRequiredRole>(),
        Some(FieldValue::Application { payload, .. })
            if matches!(payload.as_ref(), FieldValue::Reference(found) if found.encoded_id() == &second)
    ));
    assert!(matches!(
        decoded.field::<AmbiguousSequenceLiteralRole>(),
        Some(FieldValue::Literal(found)) if found == &private
    ));

    let refusal_source = "Realize.only Mystery";
    let only = encoded(FirstFixtureRoot::Universal, &[10, 35]);
    let mut refusal_bindings = Bindings::default();
    refusal_bindings.spelling(&keyword, "Realize");
    refusal_bindings.spelling(&private, "Private");
    refusal_bindings.reference(8, 12, "only", &only);
    assert!(matches!(
        evaluator.decode_text(&expected, refusal_source, &refusal_bindings),
        Err(DecodeError::SequenceRepetitionBoundary { role, refusal })
            if role.value() == AmbiguousSequenceRepeatedRole::STABLE_ID
                && matches!(refusal.as_ref(), DecodeError::LiteralMismatch)
    ));
}

#[test]
fn table_identity_is_independent_of_entry_submission_order() {
    let left = type_id(FirstFixtureRoot::Universal, &[10, 1]);
    let right = type_id(FirstFixtureRoot::Rust, &[4, 9]);
    let left_entry = entry(
        left,
        StructuralRule::Unary(
            UnaryRule::new(SharedDescriptor::Declaration(AtomDescriptor::any_case()))
                .expect("left role"),
        ),
    );
    let right_entry = entry(
        right,
        StructuralRule::Unary(
            UnaryRule::new(SharedDescriptor::Reference(AtomDescriptor::any_case()))
                .expect("right role"),
        ),
    );

    let forward =
        table(vec![left_entry.clone(), right_entry.clone()]).expect("forward table order");
    let reverse = table(vec![right_entry, left_entry]).expect("reverse table order");
    assert_eq!(forward.identity(), reverse.identity());
}

#[test]
fn typed_position_overlap_is_refused_instead_of_order_resolved() {
    let expected = type_id(FirstFixtureRoot::Universal, &[7, 7]);
    let declaration = StructuralRule::Unary(
        UnaryRule::new(SharedDescriptor::Declaration(AtomDescriptor::any_case()))
            .expect("declaration role"),
    );
    let reference = StructuralRule::Unary(
        UnaryRule::new(SharedDescriptor::Reference(AtomDescriptor::any_case()))
            .expect("reference role"),
    );
    let entry = StructuralEntry::new(
        expected.clone(),
        vec![
            ConstructorCodec::new(
                EncodedConstructorId::under(&expected, 1),
                vec![AcceptedDecodeForm::new(
                    DecodeFormId::new(1),
                    declaration.clone(),
                )],
                declaration,
            ),
            ConstructorCodec::new(
                EncodedConstructorId::under(&expected, 2),
                vec![AcceptedDecodeForm::new(
                    DecodeFormId::new(1),
                    reference.clone(),
                )],
                reference,
            ),
        ],
    );

    assert!(matches!(
        table(vec![entry]),
        Err(TableError::Disjointness(
            structural_codec::DisjointnessError::NotProvablyDisjoint { .. }
        ))
    ));
}

#[test]
fn delegated_alternation_recovers_only_when_the_value_belongs_to_another_target() {
    type Rule =
        RuleCoproduct<MultiDelegateRecord<FirstFixtureRoot>, StructuralRule<FirstFixtureRoot>>;

    let parent = type_id(FirstFixtureRoot::Universal, &[60]);
    let reverse_parent = type_id(FirstFixtureRoot::Universal, &[63]);
    let left = type_id(FirstFixtureRoot::Universal, &[61]);
    let right = type_id(FirstFixtureRoot::Universal, &[62]);
    let left_word = encoded(FirstFixtureRoot::Universal, &[70]);
    let right_word = encoded(FirstFixtureRoot::Universal, &[71]);
    let parent_rule = Rule::Left(MultiDelegateRecord::new(left.clone(), right.clone()));
    let reverse_parent_rule = Rule::Left(MultiDelegateRecord::new(right.clone(), left.clone()));
    let left_rule = Rule::Right(StructuralRule::Unary(
        UnaryRule::new(SharedDescriptor::Literal(left_word.clone())).expect("left literal"),
    ));
    let right_rule = Rule::Right(StructuralRule::Unary(
        UnaryRule::new(SharedDescriptor::Literal(right_word.clone())).expect("right literal"),
    ));
    let profile = profile();
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            TargetLayoutIdentity::derive(b"multi delegate alternation"),
            profile.identity(),
            StructuralVocabularyIdentity::language(b"delegated target ownership"),
            discovery(),
            rendering(),
            vec![
                typed_entry(parent.clone(), parent_rule),
                typed_entry(reverse_parent.clone(), reverse_parent_rule),
                typed_entry(left.clone(), left_rule),
                typed_entry(right.clone(), right_rule),
            ],
        ),
        &profile,
    )
    .expect("distinct delegate targets seal");
    let evaluator = StructuralEvaluator::new(&table).expect("shared evaluator");
    let mut bindings = Bindings::default();
    bindings.spelling(&left_word, "Left");
    bindings.spelling(&right_word, "Right");

    let right_value = evaluator
        .decode_text(&right, "Right", &bindings)
        .expect("right delegated target decodes");
    let parent_constructor = EncodedConstructorId::under(&parent, 1);
    let mut parent_record = StructuralValue::record(parent_constructor.clone());
    parent_record
        .insert::<MultiDelegateRole>(FieldValue::Delegated(Box::new(right_value)))
        .expect("typed delegated field");
    let value = parent_record.finish();
    assert_eq!(
        evaluator
            .encode_text(&parent, &value, &bindings)
            .expect("first target mismatch recovers to the right delegate"),
        "Right"
    );

    let left_value = evaluator
        .decode_text(&left, "Left", &bindings)
        .expect("left delegated target decodes");
    let mut reverse_parent_record =
        StructuralValue::record(EncodedConstructorId::under(&reverse_parent, 1));
    reverse_parent_record
        .insert::<MultiDelegateRole>(FieldValue::Delegated(Box::new(left_value)))
        .expect("typed reverse delegated field");
    assert_eq!(
        evaluator
            .encode_text(&reverse_parent, &reverse_parent_record.finish(), &bindings)
            .expect("reversed first target mismatch recovers to the left delegate"),
        "Left"
    );

    let unknown_right_constructor = EncodedConstructorId::under(&right, 2);
    let unknown_right = StructuralValue::record(unknown_right_constructor.clone()).finish();
    let mut corrupted_parent = StructuralValue::record(parent_constructor);
    corrupted_parent
        .insert::<MultiDelegateRole>(FieldValue::Delegated(Box::new(unknown_right)))
        .expect("typed delegated corruption");
    assert!(matches!(
        evaluator.encode_text(&parent, &corrupted_parent.finish(), &bindings),
        Err(EncodeError::UnknownConstructor { chosen })
            if chosen == unknown_right_constructor
    ));
}
