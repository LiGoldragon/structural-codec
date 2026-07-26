//! Shared evaluation of archived, fully typed structural records.
//!
//! R3 removes fixed product vectors; R4 removes flat type and constructor ids.
//! Tables persist actual typed records, while the evaluator's only generic result
//! is a role-keyed mirror for manual language reification and reflection.
//!
//! Field roles cannot be transposed: the two position types differ at compile time.
//!
//! ```compile_fail
//! use structural_codec::{ApplicationHead, ApplicationPayload, Position};
//! let payload: Position<ApplicationPayload> = todo!();
//! let _: Position<ApplicationHead> = payload;
//! ```
//!
//! Flat raw type ids and the old fixed-product vocabulary have no construction
//! surface.
//!
//! ```compile_fail
//! let _ = structural_codec::ScopedEncodedTypeId(7);
//! ```
//!
//! ```compile_fail
//! let _ = structural_codec::SequenceForm::Product(vec![]);
//! ```
//!
//! ```compile_fail
//! let _ = structural_codec::PositionalSignature::default();
//! ```
//!
//! A target layout is derived data, not a zero-filled digest wrapper; and the
//! closed language dimension refuses invented or wrong language ids.
//!
//! ```compile_fail
//! let _ = structural_codec::TargetLayoutIdentity([0; 32]);
//! ```
//!
//! ```compile_fail
//! let _ = structural_codec::EncodedLanguage::Rust;
//! ```

pub mod authoring;
pub mod codec;
pub mod conformance;
pub mod disjoint;
pub mod encoded_form;
pub mod error;
pub mod evaluator;
pub mod form;
pub mod ids;
pub mod table;
pub mod textual_form;
pub mod value;

pub use codec::{AcceptedDecodeForm, ConstructorCodec, StructuralEntry};
pub use conformance::{ConformanceError, ConformanceHarness, GeneratedCodec};
pub use encoded_form::{Converted, EncodedConversion, EncodedForm};
pub use error::{
    AuthoringError, DecodeError, DisjointnessError, EncodeError, SingleChunkRequired, TableError,
};
pub use evaluator::StructuralEvaluator;
pub use form::{
    ApplicationDelimitedBody, ApplicationDelimitedFieldView, ApplicationDelimitedHead,
    ApplicationDelimitedItems, ApplicationDelimitedRoot, ApplicationDelimitedRule, ApplicationHead,
    ApplicationPayload, ApplicationRoot, ApplicationRule, AtomDescriptor, BorrowedFieldView,
    DelegationPayload, FieldEnd, FieldLink, FieldVisitor, ForeignLeafId, LeafCodec, Position,
    RuleCoproduct, RuleCoproductView, SharedDescriptor, StructuralRule, StructuralRuleView,
    StructureRecord, UnaryRoot, UnaryRule,
};
pub use ids::{
    DecodeFormId, EncodedConstructorId, EncodedLanguage, FieldRole, ScopedEncodedTypeId,
    StableRoleId,
};
pub use raw_discovery::AtomCase;
pub use table::{
    AddressedStructuralTable, ArchivedTablePayload, ContextualTextualPolicy,
    FixtureVocabularyDomain, StructuralTableDomain, StructuralVocabularyDomain,
    StructuralVocabularyIdentity, TableIdentityPayload, TargetLayoutDomain, TargetLayoutIdentity,
    TextualRenderingPolicy,
};
pub use textual_form::{ChunkName, TextChunk, Textual, TextualForm};
pub use value::{
    FieldValue, RoleKeyedMirror, ScalarValue, StructuralValue, StructuralValueDomain,
    StructuralValueRecord,
};

// Fixture construction and all R3/R4 acceptance cases are test-only. Keeping
// them in the crate test build lets them use crate-private archived constructors
// without creating a consumer-facing fixture API.
#[cfg(test)]
#[path = "../tests/boundary_first.rs"]
mod boundary_first;
#[cfg(test)]
#[path = "../tests/conformance_harness.rs"]
mod conformance_harness;
#[cfg(test)]
#[path = "../tests/contextual_textual.rs"]
mod contextual_textual;
#[cfg(test)]
#[path = "../tests/disjointness.rs"]
mod disjointness;
#[cfg(test)]
#[path = "../tests/encoded_form.rs"]
mod encoded_form_tests;
#[cfg(test)]
#[path = "../tests/evaluator_behavior.rs"]
mod evaluator_behavior;
#[cfg(test)]
#[path = "../tests/support/fixture.rs"]
mod fixture;
#[cfg(test)]
#[path = "../tests/identity_locks.rs"]
mod identity_locks;
#[cfg(test)]
#[path = "../tests/laws.rs"]
mod laws;
#[cfg(test)]
#[path = "../tests/normalization.rs"]
mod normalization;
#[cfg(test)]
#[path = "../tests/textual_evaluator.rs"]
mod textual_evaluator;
