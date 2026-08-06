//! Allocation-free structural planning at the source boundary.
//!
//! A plan is the selected typed parse tree before declaration and reference
//! identities exist. Exact source spellings and bounds live only on this
//! boundary value; they never enter encoded form.

use std::collections::BTreeMap;

use name_table::{EncodedName, TextualName};
use raw_discovery::SourceBound;

use crate::ids::{EncodedConstructorId, FieldRole, StableRoleId};
use crate::value::ScalarValue;

/// One declaration or reference occurrence selected by structural planning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedName {
    spelling: TextualName,
    bound: SourceBound,
}

impl PlannedName {
    pub(crate) fn new(spelling: impl Into<String>, bound: SourceBound) -> Self {
        Self {
            spelling: TextualName::new(spelling),
            bound,
        }
    }

    /// Exact source spelling at this typed position.
    pub fn spelling(&self) -> &TextualName {
        &self.spelling
    }

    /// Exact full-source byte bound selected for this occurrence.
    pub const fn bound(&self) -> SourceBound {
        self.bound
    }
}

/// One field in an allocation-free structural plan.
///
/// Declaration and reference variants remain distinct. The surrounding
/// constructor and role-keyed tree provide the structural address and parent
/// context needed by a language boundary to build its nested authority graph.
#[derive(Clone, Debug, PartialEq)]
pub enum PlannedFieldValue<Root> {
    Declaration(PlannedName),
    Reference(PlannedName),
    Literal(EncodedName),
    Scalar(ScalarValue),
    OrderedProduct,
    Delimited(Box<PlannedFieldValue<Root>>),
    Carrier(Box<PlannedFieldValue<Root>>),
    Application {
        head: Box<PlannedFieldValue<Root>>,
        payload: Box<PlannedFieldValue<Root>>,
    },
    Delegated(Box<PlannedStructuralValue<Root>>),
    Repeated(Vec<PlannedFieldValue<Root>>),
}

/// The one successful constructor branch and its role-keyed planned fields.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedStructuralValue<Root> {
    constructor: EncodedConstructorId<Root>,
    fields: BTreeMap<StableRoleId, PlannedFieldValue<Root>>,
    field_bounds: BTreeMap<StableRoleId, SourceBound>,
}

impl<Root> PlannedStructuralValue<Root> {
    pub(crate) fn new(
        constructor: EncodedConstructorId<Root>,
        fields: BTreeMap<StableRoleId, PlannedFieldValue<Root>>,
        field_bounds: BTreeMap<StableRoleId, SourceBound>,
    ) -> Self {
        Self {
            constructor,
            fields,
            field_bounds,
        }
    }

    /// Constructor selected by the shared structural evaluator.
    pub fn constructor(&self) -> &EncodedConstructorId<Root> {
        &self.constructor
    }

    /// Retrieve one planned field through its typed role.
    pub fn field<Role: FieldRole>(&self) -> Option<&PlannedFieldValue<Root>> {
        self.field_by_role(StableRoleId::for_role::<Role>())
    }

    /// Retrieve one planned field through a verified stable role.
    pub fn field_by_role(&self, role: StableRoleId) -> Option<&PlannedFieldValue<Root>> {
        self.fields.get(&role)
    }

    /// Exact full-source bound consumed by one typed field.
    pub fn field_bound<Role: FieldRole>(&self) -> Option<SourceBound> {
        self.field_bounds
            .get(&StableRoleId::for_role::<Role>())
            .copied()
    }
}
