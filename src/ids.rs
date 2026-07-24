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
pub struct EncodedUniverseId(u32);

impl EncodedUniverseId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// The explicit fixture universe for this proof-of-concept.
pub const FIXTURE_UNIVERSE: EncodedUniverseId = EncodedUniverseId::new(0);

/// A Core-type identity scoped to a universe. Replaces the earlier bare
/// `EncodedTypeId(u32)`: two universes may reuse local numbers without collision.
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
pub struct ScopedEncodedTypeId {
    pub universe: EncodedUniverseId,
    pub local: u32,
}

impl ScopedEncodedTypeId {
    pub const fn new(universe: EncodedUniverseId, local: u32) -> Self {
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
pub struct EncodedConstructorId {
    pub core_type: ScopedEncodedTypeId,
    pub constructor: u32,
}

impl EncodedConstructorId {
    pub const fn new(core_type: ScopedEncodedTypeId, constructor: u32) -> Self {
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
pub struct PositionalSignature(Vec<ScopedEncodedTypeId>);

impl PositionalSignature {
    pub fn new(fields: Vec<ScopedEncodedTypeId>) -> Self {
        Self(fields)
    }

    pub fn fields(&self) -> &[ScopedEncodedTypeId] {
        &self.0
    }

    pub fn arity(&self) -> usize {
        self.0.len()
    }
}
