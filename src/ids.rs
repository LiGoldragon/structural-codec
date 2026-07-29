//! Opaque encoded-ID-chain carriers and non-erased field roles.

use std::marker::PhantomData;

use name_table::EncodedId;

/// A production-compatible encoded type identity.
///
/// The caller supplies the root enum. The complete, non-empty module-local
/// chain is retained; this crate has no flat or component-local projection.
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
pub struct EncodedTypeId<Root>(EncodedId<Root>);

impl<Root> EncodedTypeId<Root> {
    /// Carry a translator-issued encoded ID as a structural type identity.
    pub fn new(encoded_id: EncodedId<Root>) -> Self {
        Self(encoded_id)
    }

    /// The complete root-fronted encoded-ID chain.
    pub fn encoded_id(&self) -> &EncodedId<Root> {
        &self.0
    }

    /// Recover the complete root-fronted encoded-ID chain.
    pub fn into_encoded_id(self) -> EncodedId<Root> {
        self.0
    }
}

/// A constructor identity permanently associated with its owning encoded type.
///
/// The local constructor number is meaningful only under the complete owning
/// encoded-ID chain. It never replaces or flattens that chain.
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
pub struct EncodedConstructorId<Root> {
    type_id: EncodedTypeId<Root>,
    local: u16,
}

impl<Root: Clone> EncodedConstructorId<Root> {
    pub fn under(type_id: &EncodedTypeId<Root>, local: u16) -> Self {
        Self {
            type_id: type_id.clone(),
            local,
        }
    }
}

impl<Root> EncodedConstructorId<Root> {
    pub fn type_id(&self) -> &EncodedTypeId<Root> {
        &self.type_id
    }

    /// The constructor-local value under [`Self::type_id`].
    pub const fn local(&self) -> u16 {
        self.local
    }
}

/// A stable field-role identity archived alongside every position.
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
pub struct StableRoleId(u16);

impl StableRoleId {
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// A compile-time field role. Consumers choose one stable non-zero ID for each
/// typed position.
pub trait FieldRole: rkyv::Archive {
    const STABLE_ID: u16;
}

/// The stored identity associated with one compile-time role.
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
pub struct RoleIdentity<Role> {
    stable: StableRoleId,
    marker: PhantomData<Role>,
}

impl<Role: FieldRole> RoleIdentity<Role> {
    pub(crate) fn try_for_role() -> Result<Self, crate::error::AuthoringError> {
        if Role::STABLE_ID == 0 {
            return Err(crate::error::AuthoringError::ZeroRoleIdentity);
        }
        Ok(Self {
            stable: StableRoleId(Role::STABLE_ID),
            marker: PhantomData,
        })
    }

    pub const fn stable(&self) -> StableRoleId {
        self.stable
    }
}

impl StableRoleId {
    pub(crate) const fn for_role<Role: FieldRole>() -> Self {
        Self(Role::STABLE_ID)
    }
}

/// An accepted textual form has a stable ID under its constructor.
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
pub struct DecodeFormId(u16);

impl DecodeFormId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}
