//! The identity keys of the kernel: universes, scoped Core-type ids, constructor
//! ids, and the positional signature a codec must honour. Every id is a plain
//! rkyv-archivable value so an entire table is content-identified data (§4.1.1).

/// The Core universe a type belongs to. The proof-of-concept works entirely in
/// one explicit fixture universe while the "unit of one schema" question stays
/// parked with the psyche (`primary-56d1.11`).
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
pub struct CoreUniverseId(u32);

impl CoreUniverseId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// The explicit fixture universe for this proof-of-concept.
pub const FIXTURE_UNIVERSE: CoreUniverseId = CoreUniverseId::new(0);

/// A Core-type identity scoped to a universe. Replaces the earlier bare
/// `CoreTypeId(u32)`: two universes may reuse local numbers without collision.
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
pub struct ScopedCoreTypeId {
    pub universe: CoreUniverseId,
    pub local: u32,
}

impl ScopedCoreTypeId {
    pub const fn new(universe: CoreUniverseId, local: u32) -> Self {
        Self { universe, local }
    }

    /// A type in the fixture universe.
    pub const fn fixture(local: u32) -> Self {
        Self::new(FIXTURE_UNIVERSE, local)
    }
}

/// A single Core constructor, addressed as an index within its Core type. A
/// product type has one constructor; a sum type has one per variant.
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
pub struct CoreConstructorId {
    pub core_type: ScopedCoreTypeId,
    pub constructor: u32,
}

impl CoreConstructorId {
    pub const fn new(core_type: ScopedCoreTypeId, constructor: u32) -> Self {
        Self {
            core_type,
            constructor,
        }
    }
}

/// The positional field signature a constructor's codec must equal — the Core
/// field types, in order. Encoding and decoding both walk this signature, so the
/// codec can never disagree with the Core layout it targets.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Default, Eq, Hash, PartialEq,
)]
pub struct PositionalSignature(Vec<ScopedCoreTypeId>);

impl PositionalSignature {
    pub fn new(fields: Vec<ScopedCoreTypeId>) -> Self {
        Self(fields)
    }

    pub fn fields(&self) -> &[ScopedCoreTypeId] {
        &self.0
    }

    pub fn arity(&self) -> usize {
        self.0.len()
    }
}

/// A monotone revision number for a structural table. Bumped whenever the
/// textual surface changes; the Core side never observes it.
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
pub struct StructuralRevision(u32);

impl StructuralRevision {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}
