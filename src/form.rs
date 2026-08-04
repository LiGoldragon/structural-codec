//! Shared descriptor data and archived typed rule records.
//!
//! Fixed grammar positions are real fields of one of the archived records
//! below, each carrying a non-erased stable role. An ordered product stores
//! only links to those typed fields; it never stores the fields or their
//! descriptors in an untyped vector. `FieldLink` is the borrowed heterogeneous
//! traversal representation and is deliberately not archivable.

use std::marker::PhantomData;

use name_table::EncodedId;
use raw_discovery::{Atom, AtomCase, TriggerIdentifier};

use crate::error::AuthoringError;
use crate::ids::{EncodedTypeId, FieldRole, RoleIdentity, StableRoleId};

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct AtomDescriptor {
    pub case: Option<AtomCase>,
    pub trigger: Option<TriggerIdentifier>,
}

impl AtomDescriptor {
    pub const fn any_case() -> Self {
        Self {
            case: None,
            trigger: None,
        }
    }

    pub const fn with_case(case: AtomCase) -> Self {
        Self {
            case: Some(case),
            trigger: None,
        }
    }

    pub fn accepts(&self, atom: &Atom) -> bool {
        self.case.is_none_or(|case| case.matches(atom))
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum DelegationPayload {
    AtomCase(AtomCase),
}

impl DelegationPayload {
    pub fn accepts(self, atom: &Atom) -> bool {
        match self {
            Self::AtomCase(case) => case.matches(atom),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum LeafCodec {
    Integer,
    Float,
    Text,
    Boolean,
    PipeText,
    Foreign(ForeignLeafId),
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
pub struct ForeignLeafId(u16);

/// An ordered product of real typed positions.
///
/// Member links can only be minted from [`FieldRole`] types. The linked
/// positions remain separate archived fields in the enclosing record, so a
/// consumer retrieves their values by role rather than by numeric index.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct OrderedProduct {
    members: Vec<StableRoleId>,
}

impl OrderedProduct {
    /// Start a non-empty product with one typed field role.
    pub fn try_new<Role: FieldRole>() -> Result<Self, AuthoringError> {
        let member = RoleIdentity::<Role>::try_for_role()?.stable();
        Ok(Self {
            members: vec![member],
        })
    }

    /// Append one typed field role to the product.
    pub fn then<Role: FieldRole>(mut self) -> Result<Self, AuthoringError> {
        let member = RoleIdentity::<Role>::try_for_role()?.stable();
        if self.members.contains(&member) {
            return Err(AuthoringError::DuplicateRoleIdentity { role: member });
        }
        self.members.push(member);
        Ok(self)
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub(crate) fn members(&self) -> &[StableRoleId] {
        &self.members
    }
}

/// A fixed lexical sequence of real typed positions.
///
/// Unlike [`OrderedProduct`], whose members are already-discovered sibling
/// blocks, an ordered sequence advances through mixed lexical and bounded
/// positions inside one source bound. Every member remains a separate field
/// in the enclosing archived record.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct OrderedSequence {
    members: Vec<StableRoleId>,
}

impl OrderedSequence {
    /// Start a non-empty lexical sequence with one typed field role.
    pub fn try_new<Role: FieldRole>() -> Result<Self, AuthoringError> {
        let member = RoleIdentity::<Role>::try_for_role()?.stable();
        Ok(Self {
            members: vec![member],
        })
    }

    /// Append one typed field role to the lexical sequence.
    pub fn then<Role: FieldRole>(mut self) -> Result<Self, AuthoringError> {
        let member = RoleIdentity::<Role>::try_for_role()?.stable();
        if self.members.contains(&member) {
            return Err(AuthoringError::DuplicateRoleIdentity { role: member });
        }
        self.members.push(member);
        Ok(self)
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub(crate) fn members(&self) -> &[StableRoleId] {
        &self.members
    }
}

/// Data interpreted by the one evaluator. Role links name actual fields in the
/// enclosing archived record; fixed position payloads are never stored in a
/// recursive generic layout or retrieved by numeric index.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum SharedDescriptor<Root> {
    /// A declaration spelling whose identity is supplied by the translator.
    Declaration(AtomDescriptor),
    /// A reference spelling resolved by lookup-only caller state.
    Reference(AtomDescriptor),
    /// A declaration whose translator-supplied identity must not be one of the
    /// reserved identities.
    ///
    /// Mechanically derived grammars use this to retain an original name
    /// branch beside reserved keyword applications without matching the
    /// reserved words as ordinary declarations.
    DeclarationExcluding {
        atom: AtomDescriptor,
        excluded: Vec<EncodedId<Root>>,
    },
    /// A lookup-only reference whose resolved identity must not be one of the
    /// reserved identities.
    ///
    /// Exclusion is checked on the encoded identity after lookup; no spelling
    /// comparison or allocation path is introduced.
    ReferenceExcluding {
        atom: AtomDescriptor,
        excluded: Vec<EncodedId<Root>>,
    },
    /// A fixed vocabulary word identified by its complete encoded-ID chain.
    Literal(EncodedId<Root>),
    Leaf(LeafCodec),
    Delegate {
        target: EncodedTypeId<Root>,
        payload: Option<DelegationPayload>,
    },
    /// A fixed ordered sequence of sibling source blocks. Every linked role
    /// must carry a [`SharedDescriptor::Delegate`]; sealing verifies this.
    OrderedProduct(OrderedProduct),
    /// A fixed ordered sequence of mixed lexical and bounded typed positions.
    OrderedSequence(OrderedSequence),
    /// A fixed ordered sequence whose member spellings are adjacent rather
    /// than separated by contextual whitespace.
    ///
    /// This owns bare structural applications such as `Vector<Ordered>`: the
    /// first typed position ends at the second position's boundary, then the
    /// writer concatenates the two rendered positions exactly.
    AdjacentSequence(OrderedSequence),
    Application {
        operator: TriggerIdentifier,
        head: StableRoleId,
        payload: StableRoleId,
    },
    /// A nested application whose head and payload are local to this
    /// descriptor rather than linked sibling fields.
    ///
    /// This is the generic substrate for mechanically derived grammars such
    /// as Template(X): a widened landing position can admit a typed dotted
    /// construct without manufacturing a new Rust record or stable role for
    /// that source-language position.
    InlineApplication {
        operator: TriggerIdentifier,
        #[rkyv(omit_bounds)]
        head: Box<SharedDescriptor<Root>>,
        #[rkyv(omit_bounds)]
        payload: Box<SharedDescriptor<Root>>,
    },
    /// A closed, ordered alternation between descriptors at one typed
    /// position.
    ///
    /// Alternatives are data in the addressed table and are evaluated by the
    /// shared structural evaluator. They are not parser callbacks.
    Alternation(#[rkyv(omit_bounds)] Vec<SharedDescriptor<Root>>),
    Delimited {
        boundary: TriggerIdentifier,
        content: StableRoleId,
    },
    /// A carrier whose captured body is interpreted by another typed
    /// descriptor. This differs from a `PipeText` leaf: a carried declaration
    /// or reference remains an encoded identity rather than becoming text.
    Carrier {
        carrier: TriggerIdentifier,
        #[rkyv(omit_bounds)]
        content: Box<SharedDescriptor<Root>>,
    },
    Repeated {
        minimum: u64,
        maximum: Option<u64>,
        #[rkyv(omit_bounds)]
        element: Box<SharedDescriptor<Root>>,
    },
    /// A named boundary whose item extent is discovered before the item is
    /// evaluated. It is the generic hook for future language item rules.
    ItemBoundary {
        boundary: TriggerIdentifier,
        content: StableRoleId,
    },
}

/// One actual archived field. `RoleIdentity<Role>` contains the stable id in
/// the archive, preventing a type-only phantom role from disappearing from a
/// table identity.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Position<Role, Root, Descriptor = SharedDescriptor<Root>> {
    role: RoleIdentity<Role>,
    descriptor: Descriptor,
    vocabulary_root: PhantomData<fn() -> Root>,
}

impl<Role: FieldRole, Root, Descriptor> Position<Role, Root, Descriptor> {
    /// Construct a position carrying the stable identity declared by `Role`.
    /// The only invalid role identity (zero) is rejected before it can enter an
    /// archived rule record.
    pub fn try_new(descriptor: Descriptor) -> Result<Self, AuthoringError> {
        Ok(Self {
            role: RoleIdentity::try_for_role()?,
            descriptor,
            vocabulary_root: PhantomData,
        })
    }

    pub fn role(&self) -> StableRoleId {
        self.role.stable()
    }

    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
}

/// A borrowed heterogeneous field view. It has no archive derive and carries no
/// grammar algorithm: it can only expose positions to shared traversal.
pub struct FieldEnd;

pub struct FieldLink<'record, Role, Root, Tail> {
    head: &'record Position<Role, Root>,
    tail: Tail,
}

impl<'record, Role, Root, Tail> FieldLink<'record, Role, Root, Tail> {
    pub fn new(head: &'record Position<Role, Root>, tail: Tail) -> Self {
        Self { head, tail }
    }
}

pub trait FieldVisitor<Root> {
    fn field<Role: FieldRole>(&mut self, position: &Position<Role, Root>);
}

pub trait BorrowedFieldView<Root> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor);
}

impl<Root> BorrowedFieldView<Root> for FieldEnd {
    fn expose<Visitor: FieldVisitor<Root>>(&self, _visitor: &mut Visitor) {}
}

impl<Root, Role: FieldRole, Tail: BorrowedFieldView<Root>> BorrowedFieldView<Root>
    for FieldLink<'_, Role, Root, Tail>
{
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        visitor.field(self.head);
        self.tail.expose(visitor);
    }
}

pub trait StructureRecord<Root> {
    type View<'record>: BorrowedFieldView<Root>
    where
        Self: 'record;

    fn root_role(&self) -> StableRoleId;
    fn fields(&self) -> Self::View<'_>;
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
        #[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
        pub struct $name(PhantomData<()>);
        impl FieldRole for $name {
            const STABLE_ID: u16 = $id;
        }
    };
}

role!(UnaryRoot, 1);
role!(ApplicationRoot, 2);
role!(ApplicationHead, 3);
role!(ApplicationPayload, 4);
role!(ApplicationDelimitedRoot, 5);
role!(ApplicationDelimitedHead, 6);
role!(ApplicationDelimitedBody, 7);
role!(ApplicationDelimitedItems, 8);
role!(AdjacentApplicationDelimitedRoot, 9);
role!(AdjacentApplicationDelimitedHead, 10);
role!(AdjacentApplicationDelimitedBody, 11);
role!(AdjacentApplicationDelimitedItems, 12);

/// One actual fixed-position rule with one root field.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct UnaryRule<Root> {
    root: Position<UnaryRoot, Root>,
}

impl<Root> UnaryRule<Root> {
    /// Construct the one fixed field of a unary rule.
    pub fn new(root: SharedDescriptor<Root>) -> Result<Self, AuthoringError> {
        Ok(Self {
            root: Position::try_new(root)?,
        })
    }

    pub fn root(&self) -> &Position<UnaryRoot, Root> {
        &self.root
    }
}

impl<Root> StructureRecord<Root> for UnaryRule<Root> {
    type View<'record>
        = FieldLink<'record, UnaryRoot, Root, FieldEnd>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }
    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.root, FieldEnd)
    }
}

/// An actual record for a fixed two-position application.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ApplicationRule<Root> {
    application: Position<ApplicationRoot, Root>,
    head: Position<ApplicationHead, Root>,
    payload: Position<ApplicationPayload, Root>,
}

impl<Root> ApplicationRule<Root> {
    /// Construct the three typed fields of an application rule. The root's
    /// links are minted from the two field-role types, never supplied as raw
    /// numeric positions by a caller.
    pub fn new(
        operator: TriggerIdentifier,
        head: SharedDescriptor<Root>,
        payload: SharedDescriptor<Root>,
    ) -> Result<Self, AuthoringError> {
        Ok(Self {
            application: Position::try_new(SharedDescriptor::Application {
                operator,
                head: StableRoleId::for_role::<ApplicationHead>(),
                payload: StableRoleId::for_role::<ApplicationPayload>(),
            })?,
            head: Position::try_new(head)?,
            payload: Position::try_new(payload)?,
        })
    }

    pub fn application(&self) -> &Position<ApplicationRoot, Root> {
        &self.application
    }

    pub fn head(&self) -> &Position<ApplicationHead, Root> {
        &self.head
    }

    pub fn payload(&self) -> &Position<ApplicationPayload, Root> {
        &self.payload
    }
}

impl<Root> StructureRecord<Root> for ApplicationRule<Root> {
    type View<'record>
        = FieldLink<
        'record,
        ApplicationRoot,
        Root,
        FieldLink<
            'record,
            ApplicationHead,
            Root,
            FieldLink<'record, ApplicationPayload, Root, FieldEnd>,
        >,
    >
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.application.role()
    }
    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(
            &self.application,
            FieldLink::new(&self.head, FieldLink::new(&self.payload, FieldEnd)),
        )
    }
}

/// An actual record for `application(head, delimited(repeated(item)))`. The
/// repeated item is the only runtime sequence and therefore has explicit bounds.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ApplicationDelimitedRule<Root> {
    application: Position<ApplicationDelimitedRoot, Root>,
    head: Position<ApplicationDelimitedHead, Root>,
    body: Position<ApplicationDelimitedBody, Root>,
    items: Position<ApplicationDelimitedItems, Root>,
}

impl<Root> ApplicationDelimitedRule<Root> {
    /// Construct the four fixed fields of an application-delimited rule.
    pub fn new(
        operator: TriggerIdentifier,
        boundary: TriggerIdentifier,
        head: SharedDescriptor<Root>,
        element: SharedDescriptor<Root>,
        minimum: u64,
        maximum: Option<u64>,
    ) -> Result<Self, AuthoringError> {
        let head_role = StableRoleId::for_role::<ApplicationDelimitedHead>();
        let body_role = StableRoleId::for_role::<ApplicationDelimitedBody>();
        let items_role = StableRoleId::for_role::<ApplicationDelimitedItems>();
        Ok(Self {
            application: Position::try_new(SharedDescriptor::Application {
                operator,
                head: head_role,
                payload: body_role,
            })?,
            head: Position::try_new(head)?,
            body: Position::try_new(SharedDescriptor::Delimited {
                boundary,
                content: items_role,
            })?,
            items: Position::try_new(SharedDescriptor::Repeated {
                minimum,
                maximum,
                element: Box::new(element),
            })?,
        })
    }

    pub fn application(&self) -> &Position<ApplicationDelimitedRoot, Root> {
        &self.application
    }

    pub fn head(&self) -> &Position<ApplicationDelimitedHead, Root> {
        &self.head
    }

    pub fn body(&self) -> &Position<ApplicationDelimitedBody, Root> {
        &self.body
    }

    pub fn items(&self) -> &Position<ApplicationDelimitedItems, Root> {
        &self.items
    }
}

impl<Root> StructureRecord<Root> for ApplicationDelimitedRule<Root> {
    type View<'record>
        = FieldLink<
        'record,
        ApplicationDelimitedRoot,
        Root,
        FieldLink<
            'record,
            ApplicationDelimitedHead,
            Root,
            FieldLink<
                'record,
                ApplicationDelimitedBody,
                Root,
                FieldLink<'record, ApplicationDelimitedItems, Root, FieldEnd>,
            >,
        >,
    >
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.application.role()
    }
    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(
            &self.application,
            FieldLink::new(
                &self.head,
                FieldLink::new(&self.body, FieldLink::new(&self.items, FieldEnd)),
            ),
        )
    }
}

/// An actual record for an adjacent application whose payload is a delimited
/// repeated list: `head<item …>`. The adjacent root is distinct from dotted
/// application so the writer cannot silently reintroduce a separator.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct AdjacentApplicationDelimitedRule<Root> {
    application: Position<AdjacentApplicationDelimitedRoot, Root>,
    head: Position<AdjacentApplicationDelimitedHead, Root>,
    body: Position<AdjacentApplicationDelimitedBody, Root>,
    items: Position<AdjacentApplicationDelimitedItems, Root>,
}

impl<Root> AdjacentApplicationDelimitedRule<Root> {
    /// Construct the four typed fields of a bare `head<items…>` application.
    pub fn new(
        boundary: TriggerIdentifier,
        head: SharedDescriptor<Root>,
        element: SharedDescriptor<Root>,
        minimum: u64,
        maximum: Option<u64>,
    ) -> Result<Self, AuthoringError> {
        let items_role = StableRoleId::for_role::<AdjacentApplicationDelimitedItems>();
        Ok(Self {
            application: Position::try_new(SharedDescriptor::AdjacentSequence(
                OrderedSequence::try_new::<AdjacentApplicationDelimitedHead>()?
                    .then::<AdjacentApplicationDelimitedBody>()?,
            ))?,
            head: Position::try_new(head)?,
            body: Position::try_new(SharedDescriptor::Delimited {
                boundary,
                content: items_role,
            })?,
            items: Position::try_new(SharedDescriptor::Repeated {
                minimum,
                maximum,
                element: Box::new(element),
            })?,
        })
    }

    pub fn application(&self) -> &Position<AdjacentApplicationDelimitedRoot, Root> {
        &self.application
    }

    pub fn head(&self) -> &Position<AdjacentApplicationDelimitedHead, Root> {
        &self.head
    }

    pub fn body(&self) -> &Position<AdjacentApplicationDelimitedBody, Root> {
        &self.body
    }

    pub fn items(&self) -> &Position<AdjacentApplicationDelimitedItems, Root> {
        &self.items
    }
}

impl<Root> StructureRecord<Root> for AdjacentApplicationDelimitedRule<Root> {
    type View<'record>
        = FieldLink<
        'record,
        AdjacentApplicationDelimitedRoot,
        Root,
        FieldLink<
            'record,
            AdjacentApplicationDelimitedHead,
            Root,
            FieldLink<
                'record,
                AdjacentApplicationDelimitedBody,
                Root,
                FieldLink<'record, AdjacentApplicationDelimitedItems, Root, FieldEnd>,
            >,
        >,
    >
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.application.role()
    }

    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(
            &self.application,
            FieldLink::new(
                &self.head,
                FieldLink::new(&self.body, FieldLink::new(&self.items, FieldEnd)),
            ),
        )
    }
}

/// The kernel convenience coproduct of built-in rule shapes. Its branches select
/// data only; decode, encode, boundary work, and proof remain in shared
/// machinery.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum StructuralRule<Root> {
    Unary(UnaryRule<Root>),
    Application(ApplicationRule<Root>),
    ApplicationDelimited(ApplicationDelimitedRule<Root>),
    AdjacentApplicationDelimited(AdjacentApplicationDelimitedRule<Root>),
}

impl<Root> StructuralRule<Root> {
    pub(crate) fn root_role(&self) -> StableRoleId {
        match self {
            Self::Unary(rule) => rule.root_role(),
            Self::Application(rule) => rule.root_role(),
            Self::ApplicationDelimited(rule) => rule.root_role(),
            Self::AdjacentApplicationDelimited(rule) => rule.root_role(),
        }
    }
}

/// A structural-codec-owned, archived type-level coproduct for a downstream
/// vocabulary's fixed record types. It carries only one archived record branch;
/// the shared evaluator and prover are the only places that interpret the
/// descriptors exposed by that record.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum RuleCoproduct<Left, Right> {
    Left(Left),
    Right(Right),
}

/// Borrowed data-only field view of a [`RuleCoproduct`].
pub enum RuleCoproductView<Left, Right> {
    Left(Left),
    Right(Right),
}

impl<Root, Left: BorrowedFieldView<Root>, Right: BorrowedFieldView<Root>> BorrowedFieldView<Root>
    for RuleCoproductView<Left, Right>
{
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        match self {
            Self::Left(fields) => fields.expose(visitor),
            Self::Right(fields) => fields.expose(visitor),
        }
    }
}

impl<Root, Left: StructureRecord<Root>, Right: StructureRecord<Root>> StructureRecord<Root>
    for RuleCoproduct<Left, Right>
{
    type View<'record>
        = RuleCoproductView<Left::View<'record>, Right::View<'record>>
    where
        Self: 'record;

    fn root_role(&self) -> StableRoleId {
        match self {
            Self::Left(record) => record.root_role(),
            Self::Right(record) => record.root_role(),
        }
    }

    fn fields(&self) -> Self::View<'_> {
        match self {
            Self::Left(record) => RuleCoproductView::Left(record.fields()),
            Self::Right(record) => RuleCoproductView::Right(record.fields()),
        }
    }
}

/// Borrowed data-only field view of the built-in [`StructuralRule`] convenience
/// vocabulary.
pub type ApplicationFieldView<'record, Root> = FieldLink<
    'record,
    ApplicationRoot,
    Root,
    FieldLink<
        'record,
        ApplicationHead,
        Root,
        FieldLink<'record, ApplicationPayload, Root, FieldEnd>,
    >,
>;

pub type ApplicationDelimitedFieldView<'record, Root> = FieldLink<
    'record,
    ApplicationDelimitedRoot,
    Root,
    FieldLink<
        'record,
        ApplicationDelimitedHead,
        Root,
        FieldLink<
            'record,
            ApplicationDelimitedBody,
            Root,
            FieldLink<'record, ApplicationDelimitedItems, Root, FieldEnd>,
        >,
    >,
>;

pub type AdjacentApplicationDelimitedFieldView<'record, Root> = FieldLink<
    'record,
    AdjacentApplicationDelimitedRoot,
    Root,
    FieldLink<
        'record,
        AdjacentApplicationDelimitedHead,
        Root,
        FieldLink<
            'record,
            AdjacentApplicationDelimitedBody,
            Root,
            FieldLink<'record, AdjacentApplicationDelimitedItems, Root, FieldEnd>,
        >,
    >,
>;

pub enum StructuralRuleView<'record, Root> {
    Unary(FieldLink<'record, UnaryRoot, Root, FieldEnd>),
    Application(ApplicationFieldView<'record, Root>),
    ApplicationDelimited(ApplicationDelimitedFieldView<'record, Root>),
    AdjacentApplicationDelimited(AdjacentApplicationDelimitedFieldView<'record, Root>),
}

impl<Root> BorrowedFieldView<Root> for StructuralRuleView<'_, Root> {
    fn expose<Visitor: FieldVisitor<Root>>(&self, visitor: &mut Visitor) {
        match self {
            Self::Unary(fields) => fields.expose(visitor),
            Self::Application(fields) => fields.expose(visitor),
            Self::ApplicationDelimited(fields) => fields.expose(visitor),
            Self::AdjacentApplicationDelimited(fields) => fields.expose(visitor),
        }
    }
}

impl<Root> StructureRecord<Root> for StructuralRule<Root> {
    type View<'record>
        = StructuralRuleView<'record, Root>
    where
        Root: 'record;

    fn root_role(&self) -> StableRoleId {
        self.root_role()
    }

    fn fields(&self) -> Self::View<'_> {
        match self {
            Self::Unary(rule) => StructuralRuleView::Unary(rule.fields()),
            Self::Application(rule) => StructuralRuleView::Application(rule.fields()),
            Self::ApplicationDelimited(rule) => {
                StructuralRuleView::ApplicationDelimited(rule.fields())
            }
            Self::AdjacentApplicationDelimited(rule) => {
                StructuralRuleView::AdjacentApplicationDelimited(rule.fields())
            }
        }
    }
}
