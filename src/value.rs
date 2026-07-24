//! The evaluator's generic structural value type: a Core-agnostic representation of a
//! decoded value. A generated codec recovers the concrete Core type (§4.5), and the
//! conformance laws prove the two agree. The value type is content-identifiable; the
//! delimiter-only law witnesses that changing delimiters leaves its identity unmoved.

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use name_table::Identifier;
use raw_discovery::{Atom, Block};

use content_identity::ArchiveError;

/// A generic structural value — the value type both evaluator directions use.
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
    /// A delimited run of children. The delimiter itself is NOT stored: delimiter-only
    /// table revisions preserve the StructuralValue mirror hash; structural
    /// respellings move it by design (law 4).
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

    /// The content identity of this value, under its own hash domain. Delimiter-only
    /// table revisions preserve this hash; other structural respellings may move it.
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
    /// Render this scalar back to a raw block. Numeric and boolean scalars use the
    /// dotted rejoin; text follows the canonical string law: bare dotted text when
    /// possible, parenthesized words when spaces require it, and pipe text otherwise.
    pub fn render_block(&self) -> Block {
        match self {
            Self::Integer(value) => Self::render_dotted(&value.to_string()),
            Self::Float(value) => Self::render_dotted(&value.to_string()),
            Self::Boolean(value) => Self::render_dotted(&value.to_string()),
            Self::Text(value) => Self::render_text(value),
        }
    }

    fn render_text(value: &str) -> Block {
        if Self::qualifies_as_bare_dotted_text(value) {
            return Self::render_dotted(value);
        }
        if Self::qualifies_as_parenthesized_text(value) {
            return Block::Delimited {
                delimiter: raw_discovery::Delimiter::Parenthesis,
                root_objects: value.split(' ').map(Self::render_dotted).collect(),
            };
        }
        Block::PipeText(raw_discovery::PipeText::new(value))
    }

    fn qualifies_as_bare_dotted_text(value: &str) -> bool {
        !value.is_empty()
            && !value.contains(";;")
            && value
                .split('.')
                .all(|segment| !segment.is_empty() && Atom::new(segment).qualifies_as_symbol())
    }

    fn qualifies_as_parenthesized_text(value: &str) -> bool {
        value.contains(' ')
            && !value
                .chars()
                .any(|character| character.is_whitespace() && character != ' ')
            && value.split(' ').all(Self::qualifies_as_bare_dotted_text)
    }

    fn render_dotted(text: &str) -> Block {
        let segments: Vec<&str> = text.split('.').collect();
        let (last, leading) = segments
            .split_last()
            .expect("a split string always has one segment");
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
