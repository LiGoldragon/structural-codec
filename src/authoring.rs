//! The AUTHORING vocabulary — the psyche's named structs, kept in the surface but
//! kept OUT of the kernel. Each authoring form `normalize`s to a plain kernel
//! `StructuralForm` before the form is ever hashed or evaluated, so the kernel stays
//! six cases while the authoring surface stays expressive (design ruling 1, §4.1).

use raw_discovery::{Delimiter, TriggerIdentifier};

use crate::form::{AtomForm, SequenceForm, StructuralForm};

/// An authored form, in the vocabulary the psyche writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthoringForm {
    /// A PascalCase object symbol dot-prefixing a delimited block —
    /// `CommitSequence.{ Integer }`.
    ObjectPrefixed(ObjectSymbolPrefixedBlock),
    /// A dotted segment run — `rkyv.Archive`.
    Dotted(DottedForm),
    /// Anything already expressed in the kernel.
    Kernel(StructuralForm),
}

impl AuthoringForm {
    /// Lower this authored form to the kernel form the evaluator and the hasher see.
    pub fn normalize(&self) -> StructuralForm {
        match self {
            Self::ObjectPrefixed(block) => block.normalize(),
            Self::Dotted(dotted) => dotted.normalize(),
            Self::Kernel(form) => form.clone(),
        }
    }
}

/// `Object.{ … }`: a case-constrained object symbol dot-prefixing a delimited block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectSymbolPrefixedBlock {
    pub object: AtomForm,
    pub operator: TriggerIdentifier,
    pub boundary: TriggerIdentifier,
    pub delimiter: Delimiter,
    pub sequence: SequenceForm,
}

impl ObjectSymbolPrefixedBlock {
    /// Normalizes to `Application(Atom(object), Delimited { delimiter, sequence })`.
    pub fn normalize(&self) -> StructuralForm {
        StructuralForm::application(
            self.operator,
            StructuralForm::Atom(self.object.clone()),
            StructuralForm::Delimited {
                boundary: self.boundary,
                delimiter: self.delimiter,
                sequence: self.sequence.clone(),
            },
        )
    }
}

/// A qualified-path run `a.b.c`, structurally requiring at least two segments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DottedForm {
    pub operator: TriggerIdentifier,
    pub head: StructuralForm,
    pub payload: StructuralForm,
    pub continuation: Vec<StructuralForm>,
}

impl DottedForm {
    /// Normalizes to a right-associative `Application` chain.
    pub fn normalize(&self) -> StructuralForm {
        let payload_chain = self
            .continuation
            .iter()
            .rev()
            .cloned()
            .reduce(|chain, segment| StructuralForm::application(self.operator, segment, chain))
            .map_or_else(
                || self.payload.clone(),
                |continuation| {
                    StructuralForm::application(self.operator, self.payload.clone(), continuation)
                },
            );
        StructuralForm::application(self.operator, self.head.clone(), payload_chain)
    }
}
