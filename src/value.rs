//! The evaluator's generic currency: a Core-agnostic structural mirror of a decoded
//! value. The concrete Core type is recovered by a generated codec (§4.5); the
//! conformance laws prove the two agree. The mirror is content-identifiable, so law
//! 4 can assert a value's identity never moves across table revisions.

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use name_table::Identifier;
use raw_discovery::{Atom, Block};

use content_identity::ArchiveError;

/// A structural value — the shape both evaluator directions pivot on.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum StructuralValue {
    /// A resolved name.
    Atom(Identifier),
    /// A flattened scalar leaf.
    Scalar(ScalarValue),
    /// A delimited run of children. The delimiter itself is NOT stored: it is pure
    /// syntax fixed by the constructor's form and recovered on encode, so a
    /// delimiter-only textual revision does not move this value's identity (law 4).
    /// This deviates from §4.4's pre-hardening sketch, which carried the delimiter.
    Delimited(#[rkyv(omit_bounds)] Vec<StructuralValue>),
    /// A right-associative application.
    Application(
        #[rkyv(omit_bounds)] Box<StructuralValue>,
        #[rkyv(omit_bounds)] Box<StructuralValue>,
    ),
    /// Passed through a transparent delegate wrapper. Every wrapper level is a
    /// distinct `Delegated` layer, so delegation constructs the whole chain.
    Delegated(#[rkyv(omit_bounds)] Box<StructuralValue>),
    /// Which disjoint constructor of the expected type matched, and its payload.
    Chosen {
        constructor: u32,
        #[rkyv(omit_bounds)]
        payload: Box<StructuralValue>,
    },
    /// The empty product.
    Empty,
}

impl StructuralValue {
    pub fn chosen(constructor: u32, payload: StructuralValue) -> Self {
        Self::Chosen {
            constructor,
            payload: Box::new(payload),
        }
    }

    /// The content identity of this value, under its own hash domain. Text
    /// evolution across table revisions must never move this hash (law 4).
    pub fn content_identity(&self) -> Result<ContentHash<StructuralValueDomain>, ArchiveError> {
        ContentHash::of_core(self)
    }
}

/// A flattened scalar leaf value.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq)]
pub enum ScalarValue {
    Integer(i64),
    Float(f64),
    Text(String),
    Boolean(bool),
}

impl ScalarValue {
    /// Render this scalar back to a raw block, inverting the flatten-then-parse
    /// rejoin: dotted text rebuilds a right-associative `Application` chain (so
    /// `-122.3` becomes `Application(-122, 3)` and `a.b.c` a three-atom chain),
    /// and dot-free text is a bare atom.
    pub fn render_block(&self) -> Block {
        let text = match self {
            Self::Integer(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::Boolean(value) => value.to_string(),
        };
        let segments: Vec<&str> = text.split('.').collect();
        let (last, leading) = segments
            .split_last()
            .expect("split always yields at least one segment");
        let mut block = Block::Atom(Atom::new(*last));
        for segment in leading.iter().rev() {
            block = Block::Application {
                head: Box::new(Block::Atom(Atom::new(*segment))),
                payload: Box::new(block),
            };
        }
        block
    }
}

/// The hash domain for structural mirror values, layout-version tagged.
pub struct StructuralValueDomain;

impl HashDomain for StructuralValueDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 structural mirror value",
            layout: LayoutVersion::new(1),
        }
    }
}
