//! Opaque, language-scoped encoded identities and non-erased field roles.

use std::marker::PhantomData;

/// The closed language dimension carried by each encoded type and constructor.
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
pub enum EncodedLanguage {
    Schema,
    Logos,
    Nomos,
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
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
enum EncodedTypeIdentity {
    Schema(u16),
    Logos(u16),
    Nomos(u16),
}

/// A language-qualified encoded type id. Its archive carries the language variant
/// and its `u16` local; neither raw construction nor a flat integer projection is
/// public.
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
pub struct ScopedEncodedTypeId(EncodedTypeIdentity);

impl ScopedEncodedTypeId {
    /// Construct a Schema-owned encoded type identity.
    pub const fn schema(local: u16) -> Self {
        Self(EncodedTypeIdentity::Schema(local))
    }

    /// Construct a Logos-owned encoded type identity.
    pub const fn logos(local: u16) -> Self {
        Self(EncodedTypeIdentity::Logos(local))
    }

    /// Construct a Nomos-owned encoded type identity.
    pub const fn nomos(local: u16) -> Self {
        Self(EncodedTypeIdentity::Nomos(local))
    }

    pub const fn language(self) -> EncodedLanguage {
        match self.0 {
            EncodedTypeIdentity::Schema(_) => EncodedLanguage::Schema,
            EncodedTypeIdentity::Logos(_) => EncodedLanguage::Logos,
            EncodedTypeIdentity::Nomos(_) => EncodedLanguage::Nomos,
        }
    }

    /// The local `u16` within this identity's language namespace.
    pub const fn local(self) -> u16 {
        match self.0 {
            EncodedTypeIdentity::Schema(local)
            | EncodedTypeIdentity::Logos(local)
            | EncodedTypeIdentity::Nomos(local) => local,
        }
    }

    #[cfg(test)]
    pub(crate) const fn fixture_schema(local: u16) -> Self {
        Self(EncodedTypeIdentity::Schema(local))
    }

    pub(crate) const fn is_reserved_fixture_schema(self) -> bool {
        matches!(self.0, EncodedTypeIdentity::Schema(0xf000..=0xffff))
    }
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
enum ConstructorIdentity {
    Schema { type_local: u16, constructor: u16 },
    Logos { type_local: u16, constructor: u16 },
    Nomos { type_local: u16, constructor: u16 },
}

/// A constructor identity permanently associated with its owning encoded type.
/// It cannot be constructed from an unscoped number and cannot be re-used under
/// a different type entry.
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
pub struct EncodedConstructorId(ConstructorIdentity);

impl EncodedConstructorId {
    /// Construct an encoded constructor under its owning encoded type. The
    /// language and type association are copied from `type_id`, so callers
    /// cannot combine a Schema constructor with a Logos type.
    pub const fn under(type_id: ScopedEncodedTypeId, constructor: u16) -> Self {
        match type_id.0 {
            EncodedTypeIdentity::Schema(type_local) => Self(ConstructorIdentity::Schema {
                type_local,
                constructor,
            }),
            EncodedTypeIdentity::Logos(type_local) => Self(ConstructorIdentity::Logos {
                type_local,
                constructor,
            }),
            EncodedTypeIdentity::Nomos(type_local) => Self(ConstructorIdentity::Nomos {
                type_local,
                constructor,
            }),
        }
    }

    pub const fn type_id(self) -> ScopedEncodedTypeId {
        match self.0 {
            ConstructorIdentity::Schema { type_local, .. } => {
                ScopedEncodedTypeId(EncodedTypeIdentity::Schema(type_local))
            }
            ConstructorIdentity::Logos { type_local, .. } => {
                ScopedEncodedTypeId(EncodedTypeIdentity::Logos(type_local))
            }
            ConstructorIdentity::Nomos { type_local, .. } => {
                ScopedEncodedTypeId(EncodedTypeIdentity::Nomos(type_local))
            }
        }
    }

    pub const fn language(self) -> EncodedLanguage {
        self.type_id().language()
    }

    /// The local constructor number within [`Self::type_id`].
    pub const fn local(self) -> u16 {
        match self.0 {
            ConstructorIdentity::Schema { constructor, .. }
            | ConstructorIdentity::Logos { constructor, .. }
            | ConstructorIdentity::Nomos { constructor, .. } => constructor,
        }
    }

    #[cfg(test)]
    pub(crate) const fn fixture_schema(type_id: ScopedEncodedTypeId, constructor: u16) -> Self {
        match type_id.0 {
            EncodedTypeIdentity::Schema(type_local) => Self(ConstructorIdentity::Schema {
                type_local,
                constructor,
            }),
            EncodedTypeIdentity::Logos(_) | EncodedTypeIdentity::Nomos(_) => {
                panic!("fixture constructors are reserved Schema identities")
            }
        }
    }
}

/// A stable field-role identity archived alongside every position. It is not a
/// zero-byte marker: its `u16` is part of the table's bytes and therefore its
/// content identity.
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

/// A compile-time field role. Consumers choose a stable non-zero id once for a
/// vocabulary; [`Position`](crate::form::Position) archives that id rather than
/// relying on this marker to survive serialization.
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

/// An accepted textual form has a stable id; a runtime vector may hold
/// alternatives but never supplies their meaning by position.
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
    /// Construct the stable identity of one accepted decode form. Its scope is
    /// its constructor; [`AddressedStructuralTable`](crate::AddressedStructuralTable)
    /// sealing rejects a duplicate within that scope.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// The archived local value. It is meaningful only under its constructor.
    pub const fn value(self) -> u16 {
        self.0
    }
}
