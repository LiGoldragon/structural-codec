//! Opaque encoded-name carriers and non-erased field roles.

use std::marker::PhantomData;

use name_table::EncodedName;

/// A production-compatible encoded type identity.
///
/// `Language` distinguishes type families at compile time only. The runtime
/// identity is one opaque authority-issued `EncodedName`.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
pub struct EncodedTypeId<Language> {
    encoded_name: EncodedName,
    marker: PhantomData<Language>,
}

impl<Language> EncodedTypeId<Language> {
    /// Carry an authority-issued encoded name as a structural type identity.
    pub const fn new(encoded_name: EncodedName) -> Self {
        Self {
            encoded_name,
            marker: PhantomData,
        }
    }

    /// The opaque authority-issued encoded name.
    pub const fn encoded_name(&self) -> &EncodedName {
        &self.encoded_name
    }

    /// Recover the opaque authority-issued encoded name.
    pub const fn into_encoded_name(self) -> EncodedName {
        self.encoded_name
    }
}

impl<Language> Clone for EncodedTypeId<Language> {
    fn clone(&self) -> Self {
        Self::new(self.encoded_name)
    }
}

/// A constructor identity permanently associated with its owning encoded type.
///
/// The local constructor number is meaningful only under its owning encoded
/// type. It never replaces that type identity.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
pub struct EncodedConstructorId<Language> {
    type_id: EncodedTypeId<Language>,
    local: u16,
}

impl<Language> Clone for EncodedConstructorId<Language> {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id.clone(),
            local: self.local,
        }
    }
}

impl<Language> EncodedConstructorId<Language> {
    pub fn under(type_id: &EncodedTypeId<Language>, local: u16) -> Self {
        Self {
            type_id: type_id.clone(),
            local,
        }
    }
}

impl<Language> EncodedConstructorId<Language> {
    pub fn type_id(&self) -> &EncodedTypeId<Language> {
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
