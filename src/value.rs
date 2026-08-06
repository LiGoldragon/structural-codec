//! The evaluator result: a constructor-tagged, role-keyed structural mirror.

use std::collections::BTreeMap;

use raw_discovery::SourceBound;

use crate::error::AuthoringError;
use crate::ids::{EncodedConstructorId, FieldRole, StableRoleId};
use crate::names::{DeclarationAssignment, ResolvedReference};

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum FieldValue<Root> {
    Declaration(DeclarationAssignment<Root>),
    Reference(ResolvedReference<Root>),
    Literal(name_table::EncodedId<Root>),
    Scalar(ScalarValue),
    /// Marker for a fixed ordered product whose member values are stored under
    /// their own typed roles in the enclosing mirror.
    OrderedProduct,
    Delimited(#[rkyv(omit_bounds)] Box<FieldValue<Root>>),
    Carrier(#[rkyv(omit_bounds)] Box<FieldValue<Root>>),
    Application {
        #[rkyv(omit_bounds)]
        head: Box<FieldValue<Root>>,
        #[rkyv(omit_bounds)]
        payload: Box<FieldValue<Root>>,
    },
    Delegated(#[rkyv(omit_bounds)] Box<StructuralValue<Root>>),
    Repeated(#[rkyv(omit_bounds)] Vec<FieldValue<Root>>),
}

/// One decoded structural value with runtime-only source bounds keyed by the
/// same typed roles as its fields.
///
/// Source bounds describe this decode and are never archived or used as
/// durable identity.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceBoundedStructuralValue<Root> {
    value: StructuralValue<Root>,
    field_bounds: BTreeMap<StableRoleId, SourceBound>,
}

impl<Root> SourceBoundedStructuralValue<Root> {
    pub(crate) fn new(
        value: StructuralValue<Root>,
        field_bounds: BTreeMap<StableRoleId, SourceBound>,
    ) -> Self {
        Self {
            value,
            field_bounds,
        }
    }

    pub fn value(&self) -> &StructuralValue<Root> {
        &self.value
    }

    pub fn into_value(self) -> StructuralValue<Root> {
        self.value
    }

    /// The exact full-source bound consumed by one typed field.
    pub fn field_bound<Role: FieldRole>(&self) -> Option<SourceBound> {
        self.field_bounds
            .get(&StableRoleId::for_role::<Role>())
            .copied()
    }
}

/// A generic mirror keyed by archived stable role ids, never by a fixed field
/// index. A language-specific reifier can name a compile-time role directly.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
pub struct RoleKeyedMirror<Root> {
    values: BTreeMap<StableRoleId, FieldValue<Root>>,
}

impl<Root> Default for RoleKeyedMirror<Root> {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
}

impl<Root> RoleKeyedMirror<Root> {
    pub(crate) fn insert(&mut self, role: StableRoleId, value: FieldValue<Root>) {
        self.values.insert(role, value);
    }

    pub fn value<Role: FieldRole>(&self) -> Option<&FieldValue<Root>> {
        self.values.get(&StableRoleId::for_role::<Role>())
    }

    pub(crate) fn value_by_stable_role(&self, role: StableRoleId) -> Option<&FieldValue<Root>> {
        self.values.get(&role)
    }
}

/// Checked authoring state for a manual encoded-value mirror. Only a typed
/// field-role can add a value, so external reflection code never
/// writes a raw stable id or an untyped map entry.
#[derive(Clone, Debug)]
pub struct StructuralValueRecord<Root> {
    constructor: EncodedConstructorId<Root>,
    fields: RoleKeyedMirror<Root>,
}

impl<Root> StructuralValueRecord<Root> {
    pub fn insert<Role: FieldRole>(
        &mut self,
        value: FieldValue<Root>,
    ) -> Result<&mut Self, AuthoringError> {
        let role = StableRoleId::for_role::<Role>();
        if role.value() == 0 {
            return Err(AuthoringError::ZeroRoleIdentity);
        }
        if self.fields.values.contains_key(&role) {
            return Err(AuthoringError::DuplicateRoleIdentity { role });
        }
        self.fields.insert(role, value);
        Ok(self)
    }

    /// Finish the checked, typed-role mirror for shared evaluation.
    pub fn finish(self) -> StructuralValue<Root> {
        StructuralValue::new(self.constructor, self.fields)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
pub struct StructuralValue<Root> {
    constructor: EncodedConstructorId<Root>,
    fields: RoleKeyedMirror<Root>,
}

impl<Root> StructuralValue<Root> {
    /// Start a checked manual mirror for the selected constructor. Each field
    /// is added with [`StructuralValueRecord::insert`], whose role is a Rust
    /// type rather than an integer key.
    pub fn record(constructor: EncodedConstructorId<Root>) -> StructuralValueRecord<Root> {
        StructuralValueRecord {
            constructor,
            fields: RoleKeyedMirror::default(),
        }
    }

    pub(crate) fn new(
        constructor: EncodedConstructorId<Root>,
        fields: RoleKeyedMirror<Root>,
    ) -> Self {
        Self {
            constructor,
            fields,
        }
    }

    pub fn constructor(&self) -> &EncodedConstructorId<Root> {
        &self.constructor
    }
    pub fn fields(&self) -> &RoleKeyedMirror<Root> {
        &self.fields
    }

    /// Retrieve a manual mirror field through its typed role.
    pub fn field<Role: FieldRole>(&self) -> Option<&FieldValue<Root>> {
        self.fields.value::<Role>()
    }

    /// Read one decoded value by the stable role supplied by a verified,
    /// declaration-indexed consumer.
    ///
    /// This is deliberately read-only. Dynamic authoring still has no route
    /// around [`StructuralValueRecord::insert`]'s typed role requirement.
    pub fn field_by_role(&self, role: StableRoleId) -> Option<&FieldValue<Root>> {
        self.fields.value_by_stable_role(role)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
pub enum ScalarValue {
    Integer(i64),
    Float(f64),
    Text(String),
    Boolean(bool),
}
