//! Addressed declarations of the values a structural grammar lands into.
//!
//! Grammar records describe accepted textual structure.  Landing declarations
//! describe the encoded value carried by the semantic roles in those records.
//! They are separate inputs because punctuation, delimiters, and other textual
//! scaffolding are not encoded-value fields.
//!
//! Stable roles are positional structural metadata.  They pair a grammar
//! position with a landing field; they are never authored or emitted name
//! identities.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::StructuralEntry;
use crate::form::{
    BorrowedFieldView, FieldVisitor, LeafCodec, Position, SharedDescriptor, StructureRecord,
};
use crate::ids::{EncodedConstructorId, EncodedTypeId, FieldRole, StableRoleId};
use crate::table::AddressedStructuralTable;

/// The encoded-value shape carried by one semantic grammar role.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum LandingShape<Root> {
    /// A translator-supplied declaration identity.
    Declaration,
    /// A lookup-only referenced identity.
    Reference,
    /// One fixed vocabulary value.
    Literal(name_table::EncodedId<Root>),
    /// One scalar leaf.
    Scalar(LeafCodec),
    /// One value of another addressed encoded type.
    Type(EncodedTypeId<Root>),
    /// An ordered value sequence with explicit cardinality.
    Sequence {
        minimum: u64,
        maximum: Option<u64>,
        #[rkyv(omit_bounds)]
        element: Box<LandingShape<Root>>,
    },
}

impl<Root> LandingShape<Root> {
    pub fn sequence(minimum: u64, maximum: Option<u64>, element: LandingShape<Root>) -> Self {
        Self::Sequence {
            minimum,
            maximum,
            element: Box::new(element),
        }
    }
}

/// One semantic field of one encoded constructor.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct LandingFieldDeclaration<Root> {
    role: StableRoleId,
    shape: LandingShape<Root>,
}

impl<Root> LandingFieldDeclaration<Root> {
    /// Pair a landing shape with a compile-time structural role.
    pub fn for_role<Role: FieldRole>(shape: LandingShape<Root>) -> Self {
        Self {
            role: StableRoleId::for_role::<Role>(),
            shape,
        }
    }

    pub const fn role(&self) -> StableRoleId {
        self.role
    }

    pub const fn shape(&self) -> &LandingShape<Root> {
        &self.shape
    }
}

/// The semantic fields of one constructor.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct LandingConstructorDeclaration<Root> {
    constructor: EncodedConstructorId<Root>,
    fields: Vec<LandingFieldDeclaration<Root>>,
}

impl<Root> LandingConstructorDeclaration<Root> {
    pub fn new(
        constructor: EncodedConstructorId<Root>,
        fields: Vec<LandingFieldDeclaration<Root>>,
    ) -> Self {
        Self {
            constructor,
            fields,
        }
    }

    pub const fn constructor(&self) -> &EncodedConstructorId<Root> {
        &self.constructor
    }

    pub fn fields(&self) -> &[LandingFieldDeclaration<Root>] {
        &self.fields
    }

    fn field(&self, role: StableRoleId) -> Option<&LandingFieldDeclaration<Root>> {
        self.fields.iter().find(|field| field.role == role)
    }
}

/// Every constructor of one encoded landing type.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct LandingTypeDeclaration<Root> {
    encoded_type: EncodedTypeId<Root>,
    constructors: Vec<LandingConstructorDeclaration<Root>>,
}

impl<Root> LandingTypeDeclaration<Root> {
    pub fn new(
        encoded_type: EncodedTypeId<Root>,
        constructors: Vec<LandingConstructorDeclaration<Root>>,
    ) -> Self {
        Self {
            encoded_type,
            constructors,
        }
    }

    pub const fn encoded_type(&self) -> &EncodedTypeId<Root> {
        &self.encoded_type
    }

    pub fn constructors(&self) -> &[LandingConstructorDeclaration<Root>] {
        &self.constructors
    }

    fn constructor(
        &self,
        identity: &EncodedConstructorId<Root>,
    ) -> Option<&LandingConstructorDeclaration<Root>>
    where
        Root: PartialEq,
    {
        self.constructors
            .iter()
            .find(|constructor| constructor.constructor() == identity)
    }
}

/// An addressed landing declaration catalog.
///
/// There is intentionally no whole-catalog iterator.  Consumers enter through
/// an expected type and recursively follow declared type references.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct LandingDeclarationCatalog<Root> {
    declarations: Vec<LandingTypeDeclaration<Root>>,
}

impl<Root: Clone + Ord> LandingDeclarationCatalog<Root> {
    pub fn try_new(
        mut declarations: Vec<LandingTypeDeclaration<Root>>,
    ) -> Result<Self, LanguageDeclarationError<Root>> {
        declarations.sort_by(|left, right| left.encoded_type.cmp(&right.encoded_type));
        for pair in declarations.windows(2) {
            if pair[0].encoded_type == pair[1].encoded_type {
                return Err(LanguageDeclarationError::DuplicateLandingType {
                    encoded_type: pair[0].encoded_type.clone(),
                });
            }
        }
        for declaration in &mut declarations {
            if declaration.constructors.is_empty() {
                return Err(LanguageDeclarationError::EmptyLandingType {
                    encoded_type: declaration.encoded_type.clone(),
                });
            }
            declaration
                .constructors
                .sort_by(|left, right| left.constructor.cmp(&right.constructor));
            let mut constructors = BTreeSet::new();
            for constructor in &mut declaration.constructors {
                if constructor.constructor.type_id() != &declaration.encoded_type {
                    return Err(LanguageDeclarationError::LandingConstructorUnderWrongType {
                        encoded_type: declaration.encoded_type.clone(),
                        constructor: constructor.constructor.clone(),
                    });
                }
                if !constructors.insert(constructor.constructor.clone()) {
                    return Err(LanguageDeclarationError::DuplicateLandingConstructor {
                        constructor: constructor.constructor.clone(),
                    });
                }
                constructor
                    .fields
                    .sort_by_key(LandingFieldDeclaration::role);
                let mut roles = BTreeSet::new();
                for field in &constructor.fields {
                    if !roles.insert(field.role) {
                        return Err(LanguageDeclarationError::DuplicateLandingRole {
                            constructor: constructor.constructor.clone(),
                            role: field.role,
                        });
                    }
                }
            }
        }
        Ok(Self { declarations })
    }

    pub fn declaration(
        &self,
        expected: &EncodedTypeId<Root>,
    ) -> Option<&LandingTypeDeclaration<Root>> {
        self.declarations
            .binary_search_by(|declaration| declaration.encoded_type.cmp(expected))
            .ok()
            .map(|index| &self.declarations[index])
    }
}

/// The verified addressed closure needed to interpret one landing root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLandingClosure<Root> {
    root: EncodedTypeId<Root>,
    addressed_types: Vec<EncodedTypeId<Root>>,
}

impl<Root> VerifiedLandingClosure<Root> {
    pub const fn root(&self) -> &EncodedTypeId<Root> {
        &self.root
    }

    pub fn addressed_types(&self) -> &[EncodedTypeId<Root>] {
        &self.addressed_types
    }
}

/// A structural grammar paired with its encoded landing declarations.
pub struct LanguageDeclaration<'language, Root, Record> {
    grammar: &'language AddressedStructuralTable<Root, Record>,
    landing: &'language LandingDeclarationCatalog<Root>,
}

impl<'language, Root, Record> LanguageDeclaration<'language, Root, Record> {
    pub const fn new(
        grammar: &'language AddressedStructuralTable<Root, Record>,
        landing: &'language LandingDeclarationCatalog<Root>,
    ) -> Self {
        Self { grammar, landing }
    }
}

impl<Root, Record> LanguageDeclaration<'_, Root, Record>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    /// Verify exactly the recursively addressed grammar/declaration closure.
    pub fn verify_root(
        &self,
        root: &EncodedTypeId<Root>,
    ) -> Result<VerifiedLandingClosure<Root>, LanguageDeclarationError<Root>> {
        let mut pending = vec![root.clone()];
        let mut verified = BTreeSet::new();
        while let Some(expected) = pending.pop() {
            if !verified.insert(expected.clone()) {
                continue;
            }
            let grammar = self.grammar.entry(&expected).ok_or_else(|| {
                LanguageDeclarationError::MissingGrammarType {
                    encoded_type: expected.clone(),
                }
            })?;
            let landing = self.landing.declaration(&expected).ok_or_else(|| {
                LanguageDeclarationError::MissingLandingType {
                    encoded_type: expected.clone(),
                }
            })?;
            self.verify_entry(grammar, landing, &mut pending)?;
        }
        Ok(VerifiedLandingClosure {
            root: root.clone(),
            addressed_types: verified.into_iter().collect(),
        })
    }

    fn verify_entry(
        &self,
        grammar: &StructuralEntry<Root, Record>,
        landing: &LandingTypeDeclaration<Root>,
        pending: &mut Vec<EncodedTypeId<Root>>,
    ) -> Result<(), LanguageDeclarationError<Root>> {
        for codec in grammar.constructors() {
            let declaration = landing.constructor(codec.constructor()).ok_or_else(|| {
                LanguageDeclarationError::MissingLandingConstructor {
                    constructor: codec.constructor().clone(),
                }
            })?;
            for accepted in codec.decode_forms() {
                Self::verify_record(accepted.rule(), codec.constructor(), declaration, pending)?;
            }
            Self::verify_record(
                codec.encode_form(),
                codec.constructor(),
                declaration,
                pending,
            )?;
        }
        for declaration in landing.constructors() {
            if !grammar
                .constructors()
                .iter()
                .any(|codec| codec.constructor() == declaration.constructor())
            {
                return Err(LanguageDeclarationError::MissingGrammarConstructor {
                    constructor: declaration.constructor().clone(),
                });
            }
        }
        Ok(())
    }

    fn verify_record(
        record: &Record,
        constructor: &EncodedConstructorId<Root>,
        landing: &LandingConstructorDeclaration<Root>,
        pending: &mut Vec<EncodedTypeId<Root>>,
    ) -> Result<(), LanguageDeclarationError<Root>> {
        let mut collector = DescriptorCollector {
            fields: BTreeMap::new(),
        };
        record.fields().expose(&mut collector);

        for (role, descriptor) in &collector.fields {
            if descriptor_requires_landing(descriptor) && landing.field(*role).is_none() {
                return Err(LanguageDeclarationError::UndeclaredSemanticRole {
                    constructor: constructor.clone(),
                    role: *role,
                });
            }
        }
        for field in landing.fields() {
            let descriptor = collector.fields.get(&field.role).ok_or_else(|| {
                LanguageDeclarationError::MissingGrammarRole {
                    constructor: constructor.clone(),
                    role: field.role,
                }
            })?;
            verify_shape(constructor, field.role, field.shape(), descriptor)?;
            collect_targets(field.shape(), pending);
        }
        Ok(())
    }
}

struct DescriptorCollector<Root> {
    fields: BTreeMap<StableRoleId, SharedDescriptor<Root>>,
}

impl<Root: Clone> FieldVisitor<Root> for DescriptorCollector<Root> {
    fn field<Role: FieldRole>(&mut self, position: &Position<Role, Root>) {
        self.fields
            .insert(position.role(), position.descriptor().clone());
    }
}

fn descriptor_requires_landing<Root>(descriptor: &SharedDescriptor<Root>) -> bool {
    match descriptor {
        SharedDescriptor::Declaration(_)
        | SharedDescriptor::Reference(_)
        | SharedDescriptor::Leaf(_)
        | SharedDescriptor::Delegate { .. }
        | SharedDescriptor::Repeated { .. } => true,
        SharedDescriptor::Carrier { content, .. } => descriptor_requires_landing(content),
        SharedDescriptor::Literal(_)
        | SharedDescriptor::OrderedProduct(_)
        | SharedDescriptor::OrderedSequence(_)
        | SharedDescriptor::Application { .. }
        | SharedDescriptor::InlineApplication { .. }
        | SharedDescriptor::Alternation(_)
        | SharedDescriptor::Delimited { .. }
        | SharedDescriptor::ItemBoundary { .. } => false,
    }
}

fn collect_targets<Root: Clone>(
    shape: &LandingShape<Root>,
    pending: &mut Vec<EncodedTypeId<Root>>,
) {
    match shape {
        LandingShape::Type(target) => pending.push(target.clone()),
        LandingShape::Sequence { element, .. } => collect_targets(element, pending),
        LandingShape::Declaration
        | LandingShape::Reference
        | LandingShape::Literal(_)
        | LandingShape::Scalar(_) => {}
    }
}

fn verify_shape<Root: Clone + Eq>(
    constructor: &EncodedConstructorId<Root>,
    role: StableRoleId,
    expected: &LandingShape<Root>,
    found: &SharedDescriptor<Root>,
) -> Result<(), LanguageDeclarationError<Root>> {
    if let SharedDescriptor::Carrier { content, .. } = found {
        return verify_shape(constructor, role, expected, content);
    }
    match (expected, found) {
        (LandingShape::Declaration, SharedDescriptor::Declaration(_))
        | (LandingShape::Reference, SharedDescriptor::Reference(_)) => Ok(()),
        (LandingShape::Literal(expected), SharedDescriptor::Literal(found))
            if expected == found =>
        {
            Ok(())
        }
        (LandingShape::Scalar(expected), SharedDescriptor::Leaf(found)) if expected == found => {
            Ok(())
        }
        (LandingShape::Type(expected), SharedDescriptor::Delegate { target: found, .. })
            if expected == found =>
        {
            Ok(())
        }
        (
            LandingShape::Sequence {
                minimum: expected_minimum,
                maximum: expected_maximum,
                element: expected_element,
            },
            SharedDescriptor::Repeated {
                minimum: found_minimum,
                maximum: found_maximum,
                element: found_element,
            },
        ) if expected_minimum == found_minimum && expected_maximum == found_maximum => {
            verify_shape(constructor, role, expected_element, found_element)
        }
        (LandingShape::Type(expected), SharedDescriptor::Delegate { target: found, .. }) => {
            Err(LanguageDeclarationError::DelegateTargetMismatch {
                constructor: constructor.clone(),
                role,
                expected: expected.clone(),
                found: found.clone(),
            })
        }
        (
            LandingShape::Sequence {
                minimum: expected_minimum,
                maximum: expected_maximum,
                ..
            },
            SharedDescriptor::Repeated {
                minimum: found_minimum,
                maximum: found_maximum,
                ..
            },
        ) => Err(LanguageDeclarationError::CardinalityMismatch {
            constructor: constructor.clone(),
            role,
            expected_minimum: *expected_minimum,
            expected_maximum: *expected_maximum,
            found_minimum: *found_minimum,
            found_maximum: *found_maximum,
        }),
        _ => Err(LanguageDeclarationError::LandingShapeMismatch {
            constructor: constructor.clone(),
            role,
            expected: LandingShapeKind::of(expected),
            found: DescriptorKind::of(found),
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LandingShapeKind {
    Declaration,
    Reference,
    Literal,
    Scalar,
    Type,
    Sequence,
}

impl LandingShapeKind {
    fn of<Root>(shape: &LandingShape<Root>) -> Self {
        match shape {
            LandingShape::Declaration => Self::Declaration,
            LandingShape::Reference => Self::Reference,
            LandingShape::Literal(_) => Self::Literal,
            LandingShape::Scalar(_) => Self::Scalar,
            LandingShape::Type(_) => Self::Type,
            LandingShape::Sequence { .. } => Self::Sequence,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorKind {
    Declaration,
    Reference,
    Literal,
    Leaf,
    Delegate,
    OrderedProduct,
    OrderedSequence,
    Application,
    InlineApplication,
    Alternation,
    Delimited,
    Carrier,
    Repeated,
    ItemBoundary,
}

impl DescriptorKind {
    fn of<Root>(descriptor: &SharedDescriptor<Root>) -> Self {
        match descriptor {
            SharedDescriptor::Declaration(_) => Self::Declaration,
            SharedDescriptor::Reference(_) => Self::Reference,
            SharedDescriptor::Literal(_) => Self::Literal,
            SharedDescriptor::Leaf(_) => Self::Leaf,
            SharedDescriptor::Delegate { .. } => Self::Delegate,
            SharedDescriptor::OrderedProduct(_) => Self::OrderedProduct,
            SharedDescriptor::OrderedSequence(_) => Self::OrderedSequence,
            SharedDescriptor::Application { .. } => Self::Application,
            SharedDescriptor::InlineApplication { .. } => Self::InlineApplication,
            SharedDescriptor::Alternation(_) => Self::Alternation,
            SharedDescriptor::Delimited { .. } => Self::Delimited,
            SharedDescriptor::Carrier { .. } => Self::Carrier,
            SharedDescriptor::Repeated { .. } => Self::Repeated,
            SharedDescriptor::ItemBoundary { .. } => Self::ItemBoundary,
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum LanguageDeclarationError<Root> {
    #[error("landing catalog repeats encoded type {encoded_type:?}")]
    DuplicateLandingType { encoded_type: EncodedTypeId<Root> },
    #[error("landing type {encoded_type:?} has no constructors")]
    EmptyLandingType { encoded_type: EncodedTypeId<Root> },
    #[error(
        "landing constructor {constructor:?} belongs under a different type than {encoded_type:?}"
    )]
    LandingConstructorUnderWrongType {
        encoded_type: EncodedTypeId<Root>,
        constructor: EncodedConstructorId<Root>,
    },
    #[error("landing catalog repeats constructor {constructor:?}")]
    DuplicateLandingConstructor {
        constructor: EncodedConstructorId<Root>,
    },
    #[error("landing constructor {constructor:?} repeats semantic role {role:?}")]
    DuplicateLandingRole {
        constructor: EncodedConstructorId<Root>,
        role: StableRoleId,
    },
    #[error("grammar has no addressed type {encoded_type:?}")]
    MissingGrammarType { encoded_type: EncodedTypeId<Root> },
    #[error("landing catalog has no addressed type {encoded_type:?}")]
    MissingLandingType { encoded_type: EncodedTypeId<Root> },
    #[error("grammar constructor {constructor:?} has no landing declaration")]
    MissingLandingConstructor {
        constructor: EncodedConstructorId<Root>,
    },
    #[error("landing constructor {constructor:?} has no grammar record")]
    MissingGrammarConstructor {
        constructor: EncodedConstructorId<Root>,
    },
    #[error("grammar constructor {constructor:?} has undeclared semantic role {role:?}")]
    UndeclaredSemanticRole {
        constructor: EncodedConstructorId<Root>,
        role: StableRoleId,
    },
    #[error("landing constructor {constructor:?} role {role:?} is absent from its grammar record")]
    MissingGrammarRole {
        constructor: EncodedConstructorId<Root>,
        role: StableRoleId,
    },
    #[error("constructor {constructor:?} role {role:?} expects {expected:?}, found {found:?}")]
    LandingShapeMismatch {
        constructor: EncodedConstructorId<Root>,
        role: StableRoleId,
        expected: LandingShapeKind,
        found: DescriptorKind,
    },
    #[error(
        "constructor {constructor:?} role {role:?} delegates to {found:?}, expected {expected:?}"
    )]
    DelegateTargetMismatch {
        constructor: EncodedConstructorId<Root>,
        role: StableRoleId,
        expected: EncodedTypeId<Root>,
        found: EncodedTypeId<Root>,
    },
    #[error(
        "constructor {constructor:?} role {role:?} cardinality {found_minimum}..{found_maximum:?} differs from {expected_minimum}..{expected_maximum:?}"
    )]
    CardinalityMismatch {
        constructor: EncodedConstructorId<Root>,
        role: StableRoleId,
        expected_minimum: u64,
        expected_maximum: Option<u64>,
        found_minimum: u64,
        found_maximum: Option<u64>,
    },
}
