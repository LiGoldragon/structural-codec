//! The kernel `StructuralForm`: the minimal, revisioned, bidirectional vocabulary
//! the trusted evaluator reads in both directions. A form is DATA — it carries no
//! parsing code. The authoring vocabulary (`crate::authoring`) normalizes to these
//! six cases before a form is ever hashed or evaluated, so the kernel stays small.
//!
//! The recursive cases (`Product`, `Application`, `Delimited`) carry the same rkyv
//! bound attributes raw-discovery proved on its `Block`, so an entire form tree is
//! content-identified data.

use name_table::Identifier;
use raw_discovery::{Atom, AtomCase, Delimiter, TriggerIdentifier};

use crate::ids::ScopedEncodedTypeId;

/// The six-case kernel. `macro` is reserved for Nomos; textual structure is
/// represented as `StructuralForm` data (settled terminology, design §4.1).
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum StructuralForm {
    /// A single bare atom, case-constrained; always resolves to a name.
    Atom(AtomForm),
    /// A scalar leaf (flatten-then-parse) or an explicit carrier.
    Leaf(LeafForm),
    /// An interned keyword the input must present verbatim.
    Literal(Identifier),
    /// Right-associative application `head.payload`.
    Application {
        /// The profile trigger that spells the application operator.
        operator: TriggerIdentifier,
        #[rkyv(omit_bounds)]
        head: Box<StructuralForm>,
        #[rkyv(omit_bounds)]
        payload: Box<StructuralForm>,
    },
    /// A delimiter around a sequence (the sequence algebra).
    Delimited {
        /// The profile trigger that supplies this group's opening and closing
        /// boundary spellings.
        boundary: TriggerIdentifier,
        delimiter: Delimiter,
        #[rkyv(omit_bounds)]
        sequence: SequenceForm,
    },
    /// Constructs a wrapper level over another Core type. An optional typed
    /// payload constrains how that expected-type position reads input.
    /// Transparent cycles are rejected; recursion is permitted only after
    /// consuming structure.
    Delegate {
        target: ScopedEncodedTypeId,
        payload: Option<DelegationPayload>,
    },
}

impl StructuralForm {
    /// A bare PascalCase name atom — the dominant declaration head.
    pub fn pascal_atom() -> Self {
        Self::Atom(AtomForm::with_case(AtomCase::PascalCase))
    }

    /// A bare camelCase name atom.
    pub fn camel_atom() -> Self {
        Self::Atom(AtomForm::with_case(AtomCase::CamelCase))
    }

    /// A right-associative `head.payload` application.
    pub fn application(
        operator: TriggerIdentifier,
        head: StructuralForm,
        payload: StructuralForm,
    ) -> Self {
        Self::Application {
            operator,
            head: Box::new(head),
            payload: Box::new(payload),
        }
    }

    /// A transparent delegation with no position-specific direction.
    pub fn delegate(target: ScopedEncodedTypeId) -> Self {
        Self::Delegate {
            target,
            payload: None,
        }
    }

    /// A delegation whose expected-type position is directed by sealed typed
    /// payload data.
    pub fn delegate_with_payload(target: ScopedEncodedTypeId, payload: DelegationPayload) -> Self {
        Self::Delegate {
            target,
            payload: Some(payload),
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

/// A single bare atom, constrained only by its raw capitalization class.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct AtomForm {
    /// `None` accepts any case.
    pub case: Option<AtomCase>,
    /// An optional profile trigger that recognizes this atom carrier. `None`
    /// reads the negative space between the active position's triggers.
    pub trigger: Option<TriggerIdentifier>,
}

impl AtomForm {
    /// An atom that accepts every raw capitalization class.
    pub fn any_case() -> Self {
        Self {
            case: None,
            trigger: None,
        }
    }

    /// Constrain this atom with raw-discovery's public, partitioned predicate.
    pub fn with_case(case: AtomCase) -> Self {
        Self {
            case: Some(case),
            trigger: None,
        }
    }

    /// Constrain case and recognize the spelling through one profile trigger.
    pub fn with_trigger(case: Option<AtomCase>, trigger: TriggerIdentifier) -> Self {
        Self {
            case,
            trigger: Some(trigger),
        }
    }

    /// Whether a discovered atom satisfies this form's case constraint.
    pub fn accepts_case(&self, atom: &Atom) -> bool {
        match self.case {
            None => true,
            Some(expected) => expected.matches(atom),
        }
    }
}

/// Closed, typed data that directs one expected-type delegation position.
///
/// The atom case is the deliberately small first payload kind: it generalizes the
/// existing case expectation without reviving the unused sigil surface. Future
/// direction must add a new enum variant, which makes table identity and the
/// disjointness proof change deliberately.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum DelegationPayload {
    /// Require the delegated position to receive an atom of this case before its
    /// target entry is evaluated.
    AtomCase(AtomCase),
}

impl DelegationPayload {
    /// Whether this payload accepts a discovered atom at the delegated position.
    pub fn accepts_atom(self, atom: &Atom) -> bool {
        match self {
            Self::AtomCase(case) => case.matches(atom),
        }
    }

    /// The structural constraint that the disjointness prover combines with the
    /// delegated target's decode forms.
    pub(crate) fn constraint_form(self) -> StructuralForm {
        match self {
            Self::AtomCase(case) => StructuralForm::Atom(AtomForm::with_case(case)),
        }
    }
}

/// The leaf/carrier model. A leaf either flattens-and-parses a scalar (the rejoin
/// mechanism, identical for float and string) or names a carrier for content a
/// bare atom or `()` cannot hold.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct LeafForm {
    pub codec: LeafCodec,
    /// An optional profile trigger that carries this leaf. `None` reads the
    /// negative space between the active position's triggers.
    pub trigger: Option<TriggerIdentifier>,
}

impl LeafForm {
    pub fn scalar(scalar: ScalarLeaf) -> Self {
        Self {
            codec: LeafCodec::Scalar(scalar),
            trigger: None,
        }
    }

    /// A leaf recognized and emitted through one profile trigger.
    pub fn with_trigger(codec: LeafCodec, trigger: TriggerIdentifier) -> Self {
        Self {
            codec,
            trigger: Some(trigger),
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
