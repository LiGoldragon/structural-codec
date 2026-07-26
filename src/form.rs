//! Shared descriptor data and archived typed rule records.
//!
//! Fixed grammar positions are never a product vector. They are real fields of
//! one of the archived records below, each carrying a non-erased stable role.
//! `FieldLink` is the only heterogeneous representation; it borrows records for
//! traversal and is deliberately not archivable or storable in a table.

use std::marker::PhantomData;

use name_table::Identifier;
use raw_discovery::{Atom, AtomCase, TriggerIdentifier};

use crate::error::AuthoringError;
use crate::ids::{FieldRole, RoleIdentity, ScopedEncodedTypeId, StableRoleId};

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

/// Data interpreted by the one evaluator. Role links name actual fields in the
/// enclosing archived record; no fixed rule is represented as a recursive
/// generic layout or an indexed sequence.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum SharedDescriptor {
    Atom(AtomDescriptor),
    Literal(Identifier),
    Leaf(LeafCodec),
    Delegate {
        target: ScopedEncodedTypeId,
        payload: Option<DelegationPayload>,
    },
    Application {
        operator: TriggerIdentifier,
        head: StableRoleId,
        payload: StableRoleId,
    },
    Delimited {
        boundary: TriggerIdentifier,
        content: StableRoleId,
    },
    Repeated {
        minimum: u64,
        maximum: Option<u64>,
        #[rkyv(omit_bounds)]
        element: Box<SharedDescriptor>,
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
pub struct Position<Role, Descriptor = SharedDescriptor> {
    role: RoleIdentity<Role>,
    descriptor: Descriptor,
}

impl<Role: FieldRole, Descriptor> Position<Role, Descriptor> {
    /// Construct a position carrying the stable identity declared by `Role`.
    /// The only invalid role identity (zero) is rejected before it can enter an
    /// archived rule record.
    pub fn try_new(descriptor: Descriptor) -> Result<Self, AuthoringError> {
        Ok(Self {
            role: RoleIdentity::try_for_role()?,
            descriptor,
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

pub struct FieldLink<'record, Role, Tail> {
    head: &'record Position<Role>,
    tail: Tail,
}

impl<'record, Role, Tail> FieldLink<'record, Role, Tail> {
    pub fn new(head: &'record Position<Role>, tail: Tail) -> Self {
        Self { head, tail }
    }
}

pub trait FieldVisitor {
    fn field<Role: FieldRole>(&mut self, position: &Position<Role>);
}

pub trait BorrowedFieldView {
    fn expose<Visitor: FieldVisitor>(&self, visitor: &mut Visitor);
}

impl BorrowedFieldView for FieldEnd {
    fn expose<Visitor: FieldVisitor>(&self, _visitor: &mut Visitor) {}
}

impl<Role: FieldRole, Tail: BorrowedFieldView> BorrowedFieldView for FieldLink<'_, Role, Tail> {
    fn expose<Visitor: FieldVisitor>(&self, visitor: &mut Visitor) {
        visitor.field(self.head);
        self.tail.expose(visitor);
    }
}

pub trait StructureRecord {
    type View<'record>: BorrowedFieldView
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

/// One actual fixed-position rule with one root field.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct UnaryRule {
    root: Position<UnaryRoot>,
}

impl UnaryRule {
    /// Construct the one fixed field of a unary rule.
    pub fn new(root: SharedDescriptor) -> Result<Self, AuthoringError> {
        Ok(Self {
            root: Position::try_new(root)?,
        })
    }

    pub fn root(&self) -> &Position<UnaryRoot> {
        &self.root
    }
}

impl StructureRecord for UnaryRule {
    type View<'record> = FieldLink<'record, UnaryRoot, FieldEnd>;

    fn root_role(&self) -> StableRoleId {
        self.root.role()
    }
    fn fields(&self) -> Self::View<'_> {
        FieldLink::new(&self.root, FieldEnd)
    }
}

/// An actual record for a fixed two-position application.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ApplicationRule {
    application: Position<ApplicationRoot>,
    head: Position<ApplicationHead>,
    payload: Position<ApplicationPayload>,
}

impl ApplicationRule {
    /// Construct the three typed fields of an application rule. The root's
    /// links are minted from the two field-role types, never supplied as raw
    /// numeric positions by a caller.
    pub fn new(
        operator: TriggerIdentifier,
        head: SharedDescriptor,
        payload: SharedDescriptor,
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

    pub fn application(&self) -> &Position<ApplicationRoot> {
        &self.application
    }

    pub fn head(&self) -> &Position<ApplicationHead> {
        &self.head
    }

    pub fn payload(&self) -> &Position<ApplicationPayload> {
        &self.payload
    }
}

impl StructureRecord for ApplicationRule {
    type View<'record> = FieldLink<
        'record,
        ApplicationRoot,
        FieldLink<'record, ApplicationHead, FieldLink<'record, ApplicationPayload, FieldEnd>>,
    >;

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
pub struct ApplicationDelimitedRule {
    application: Position<ApplicationDelimitedRoot>,
    head: Position<ApplicationDelimitedHead>,
    body: Position<ApplicationDelimitedBody>,
    items: Position<ApplicationDelimitedItems>,
}

impl ApplicationDelimitedRule {
    /// Construct the four fixed fields of an application-delimited rule.
    pub fn new(
        operator: TriggerIdentifier,
        boundary: TriggerIdentifier,
        head: SharedDescriptor,
        element: SharedDescriptor,
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

    pub fn application(&self) -> &Position<ApplicationDelimitedRoot> {
        &self.application
    }

    pub fn head(&self) -> &Position<ApplicationDelimitedHead> {
        &self.head
    }

    pub fn body(&self) -> &Position<ApplicationDelimitedBody> {
        &self.body
    }

    pub fn items(&self) -> &Position<ApplicationDelimitedItems> {
        &self.items
    }
}

impl StructureRecord for ApplicationDelimitedRule {
    type View<'record> = FieldLink<
        'record,
        ApplicationDelimitedRoot,
        FieldLink<
            'record,
            ApplicationDelimitedHead,
            FieldLink<
                'record,
                ApplicationDelimitedBody,
                FieldLink<'record, ApplicationDelimitedItems, FieldEnd>,
            >,
        >,
    >;

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
pub enum StructuralRule {
    Unary(UnaryRule),
    Application(ApplicationRule),
    ApplicationDelimited(ApplicationDelimitedRule),
}

impl StructuralRule {
    pub(crate) fn root_role(&self) -> StableRoleId {
        match self {
            Self::Unary(rule) => rule.root_role(),
            Self::Application(rule) => rule.root_role(),
            Self::ApplicationDelimited(rule) => rule.root_role(),
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

impl<Left: BorrowedFieldView, Right: BorrowedFieldView> BorrowedFieldView
    for RuleCoproductView<Left, Right>
{
    fn expose<Visitor: FieldVisitor>(&self, visitor: &mut Visitor) {
        match self {
            Self::Left(fields) => fields.expose(visitor),
            Self::Right(fields) => fields.expose(visitor),
        }
    }
}

impl<Left: StructureRecord, Right: StructureRecord> StructureRecord for RuleCoproduct<Left, Right> {
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
pub type ApplicationDelimitedFieldView<'record> = FieldLink<
    'record,
    ApplicationDelimitedRoot,
    FieldLink<
        'record,
        ApplicationDelimitedHead,
        FieldLink<
            'record,
            ApplicationDelimitedBody,
            FieldLink<'record, ApplicationDelimitedItems, FieldEnd>,
        >,
    >,
>;

pub enum StructuralRuleView<'record> {
    Unary(FieldLink<'record, UnaryRoot, FieldEnd>),
    Application(
        FieldLink<
            'record,
            ApplicationRoot,
            FieldLink<'record, ApplicationHead, FieldLink<'record, ApplicationPayload, FieldEnd>>,
        >,
    ),
    ApplicationDelimited(ApplicationDelimitedFieldView<'record>),
}

impl BorrowedFieldView for StructuralRuleView<'_> {
    fn expose<Visitor: FieldVisitor>(&self, visitor: &mut Visitor) {
        match self {
            Self::Unary(fields) => fields.expose(visitor),
            Self::Application(fields) => fields.expose(visitor),
            Self::ApplicationDelimited(fields) => fields.expose(visitor),
        }
    }
}

impl StructureRecord for StructuralRule {
    type View<'record> = StructuralRuleView<'record>;

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
        }
    }
}
