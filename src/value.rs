//! The evaluator result: a constructor-tagged, role-keyed structural mirror.

use std::collections::BTreeMap;

use content_identity::{ArchiveError, ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use name_table::Identifier;

use crate::error::AuthoringError;
use crate::ids::{EncodedConstructorId, FieldRole, StableRoleId};

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum FieldValue {
    Atom(Identifier),
    Scalar(ScalarValue),
    Delimited(#[rkyv(omit_bounds)] Box<FieldValue>),
    Application {
        #[rkyv(omit_bounds)]
        head: Box<FieldValue>,
        #[rkyv(omit_bounds)]
        payload: Box<FieldValue>,
    },
    Delegated(#[rkyv(omit_bounds)] Box<StructuralValue>),
    Repeated(#[rkyv(omit_bounds)] Vec<FieldValue>),
}

/// A generic mirror keyed by archived stable role ids, never by a fixed field
/// index. Manual `Textual::reify`/`reflect` can name a compile-time role directly.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RoleKeyedMirror {
    values: BTreeMap<StableRoleId, FieldValue>,
}

impl RoleKeyedMirror {
    pub(crate) fn insert(&mut self, role: StableRoleId, value: FieldValue) {
        self.values.insert(role, value);
    }

    pub fn value<Role: FieldRole>(&self) -> Option<&FieldValue> {
        self.values.get(&StableRoleId::for_role::<Role>())
    }

    pub(crate) fn value_by_stable_role(&self, role: StableRoleId) -> Option<&FieldValue> {
        self.values.get(&role)
    }
}

/// Checked authoring state for a manual encoded-value mirror. Only a typed
/// field-role can add a value, so external `Textual::reflect` code never
/// writes a raw stable id or an untyped map entry.
#[derive(Clone, Debug)]
pub struct StructuralValueRecord {
    constructor: EncodedConstructorId,
    fields: RoleKeyedMirror,
}

impl StructuralValueRecord {
    pub fn insert<Role: FieldRole>(
        &mut self,
        value: FieldValue,
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
    pub fn finish(self) -> StructuralValue {
        StructuralValue::new(self.constructor, self.fields)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
pub struct StructuralValue {
    constructor: EncodedConstructorId,
    fields: RoleKeyedMirror,
}

impl StructuralValue {
    /// Start a checked manual mirror for the selected constructor. Each field
    /// is added with [`StructuralValueRecord::insert`], whose role is a Rust
    /// type rather than an integer key.
    pub fn record(constructor: EncodedConstructorId) -> StructuralValueRecord {
        StructuralValueRecord {
            constructor,
            fields: RoleKeyedMirror::default(),
        }
    }

    pub(crate) fn new(constructor: EncodedConstructorId, fields: RoleKeyedMirror) -> Self {
        Self {
            constructor,
            fields,
        }
    }

    pub fn constructor(&self) -> EncodedConstructorId {
        self.constructor
    }
    pub fn fields(&self) -> &RoleKeyedMirror {
        &self.fields
    }

    /// Retrieve a manual mirror field through its typed role.
    pub fn field<Role: FieldRole>(&self) -> Option<&FieldValue> {
        self.fields.value::<Role>()
    }

    pub fn content_identity(&self) -> Result<ContentHash<StructuralValueDomain>, ArchiveError> {
        ContentHash::of_core(self)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
pub enum ScalarValue {
    Integer(i64),
    Float(f64),
    Text(String),
    Boolean(bool),
}

/// Layout two is required because the archived mirror changed from an unkeyed
/// product/tree to constructor plus role-keyed values.
pub struct StructuralValueDomain;

impl HashDomain for StructuralValueDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 structural mirror value",
            layout: LayoutVersion::new(2),
        }
    }
}
