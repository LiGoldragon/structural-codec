//! The kernel `StructuralForm`: the minimal, revisioned, bidirectional vocabulary
//! the trusted evaluator reads in both directions. A form is DATA — it carries no
//! parsing code. The authoring vocabulary (`crate::authoring`) normalizes to these
//! seven cases before a form is ever hashed or evaluated, so the kernel stays small.
//!
//! The recursive cases (`Product`, `Application`, `Delimited`) carry the same rkyv
//! bound attributes raw-discovery proved on its `Block`, so an entire form tree is
//! content-identified data.

use name_table::Identifier;
use raw_discovery::{Atom, AtomCase, Delimiter};

use crate::ids::ScopedCoreTypeId;

/// The seven-case kernel. `macro` is reserved for Nomos; the parser-side data is a
/// `StructuralForm` (settled terminology, design §4.1).
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum StructuralForm {
    /// Heterogeneous positional tuple over a run of sibling blocks.
    Product(#[rkyv(omit_bounds)] Vec<StructuralForm>),
    /// A single bare atom, case- and sigil-constrained; always resolves to a name.
    Atom(AtomForm),
    /// A scalar leaf (flatten-then-parse) or an explicit carrier.
    Leaf(LeafForm),
    /// An interned keyword the input must present verbatim.
    Literal(Identifier),
    /// Right-associative application `head.payload`.
    Application {
        #[rkyv(omit_bounds)]
        head: Box<StructuralForm>,
        #[rkyv(omit_bounds)]
        payload: Box<StructuralForm>,
    },
    /// A delimiter around a sequence (the sequence algebra).
    Delimited {
        delimiter: Delimiter,
        #[rkyv(omit_bounds)]
        sequence: SequenceForm,
    },
    /// Constructs a wrapper level over another Core type. Transparent cycles are
    /// rejected; recursion is permitted only after consuming structure.
    Delegate(ScopedCoreTypeId),
}

impl StructuralForm {
    /// A bare PascalCase name atom — the dominant declaration head.
    pub fn pascal_atom() -> Self {
        Self::Atom(AtomForm::with_case(CaseExpectation::PascalCase))
    }

    /// A bare camelCase name atom.
    pub fn camel_atom() -> Self {
        Self::Atom(AtomForm::with_case(CaseExpectation::CamelCase))
    }

    /// A right-associative `head.payload` application.
    pub fn application(head: StructuralForm, payload: StructuralForm) -> Self {
        Self::Application {
            head: Box::new(head),
            payload: Box::new(payload),
        }
    }
}

/// The repetition/tuple algebra inside a delimiter. Repetition is ALWAYS explicit
/// here; it is never implied by a count constraint elsewhere.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum SequenceForm {
    /// Fixed heterogeneous positional slots.
    Product(#[rkyv(omit_bounds)] Vec<StructuralForm>),
    /// Homogeneous repetition of one element, bounded `[minimum, maximum]`.
    Repeat {
        minimum: u64,
        maximum: Option<u64>,
        #[rkyv(omit_bounds)]
        element: Box<StructuralForm>,
    },
}

impl SequenceForm {
    /// Zero-or-more of one element.
    pub fn zero_or_more(element: StructuralForm) -> Self {
        Self::Repeat {
            minimum: 0,
            maximum: None,
            element: Box::new(element),
        }
    }

    /// Whether a repetition count is within this sequence's bounds.
    pub fn admits_count(&self, count: u64) -> bool {
        match self {
            Self::Product(forms) => forms.len() as u64 == count,
            Self::Repeat {
                minimum, maximum, ..
            } => count >= *minimum && maximum.is_none_or(|top| count <= top),
        }
    }
}

/// A single bare atom, case- and sigil-constrained.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct AtomForm {
    /// `None` accepts any case.
    pub case: Option<CaseExpectation>,
    /// The `$` escape rides here as a sigil; `None` requires no sigil.
    pub sigil: Option<SigilSpec>,
}

impl AtomForm {
    pub fn with_case(case: CaseExpectation) -> Self {
        Self {
            case: Some(case),
            sigil: None,
        }
    }

    /// Whether a discovered atom satisfies this form's case constraint. (Sigil
    /// matching is reserved for the not-yet-accepted `$` profile.)
    pub fn accepts(&self, atom: &Atom) -> bool {
        match self.case {
            None => true,
            Some(expected) => expected.matches(atom),
        }
    }
}

/// The capitalization expectation — mirrors raw-discovery's `AtomCase`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaseExpectation {
    Symbol,
    PascalCase,
    CamelCase,
    KebabCase,
}

impl CaseExpectation {
    /// The raw-discovery case this expectation corresponds to.
    pub fn raw_case(self) -> AtomCase {
        match self {
            Self::Symbol => AtomCase::Symbol,
            Self::PascalCase => AtomCase::PascalCase,
            Self::CamelCase => AtomCase::CamelCase,
            Self::KebabCase => AtomCase::KebabCase,
        }
    }

    pub fn matches(self, atom: &Atom) -> bool {
        AtomCase::of(atom) == self.raw_case()
    }
}

/// The `$`-style sigil specification. Reserved for the Nomos-extended profile.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SigilSpec {
    pub character: String,
    pub position: SigilPosition,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum SigilPosition {
    Prefix,
    Suffix,
}

/// The leaf/carrier model. A leaf either flattens-and-parses a scalar (the rejoin
/// mechanism, identical for float and string) or names a carrier for content a
/// bare atom or `()` cannot hold.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct LeafForm {
    pub codec: LeafCodec,
}

impl LeafForm {
    pub fn scalar(scalar: ScalarLeaf) -> Self {
        Self {
            codec: LeafCodec::Scalar(scalar),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum LeafCodec {
    /// Flatten-then-parse: a single atom flattens to itself; a dotted application
    /// rejoins through `Block::dotted_text`.
    Scalar(ScalarLeaf),
    /// An explicit carrier for content a bare atom or `()` cannot represent.
    Carrier(CarrierLeaf),
    /// A foreign (e.g. Rust) custom leaf, named by contract id.
    Foreign(ForeignLeafId),
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarLeaf {
    Integer,
    Float,
    Text,
    Boolean,
}

/// The carrier vocabulary; extends as other carriers earn a form.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum CarrierLeaf {
    /// The `(| |)` pipe-text carrier.
    PipeText,
}

/// The identity of a foreign leaf codec's contract.
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
pub struct ForeignLeafId(pub u32);
