//! Shared evaluation of archived, fully typed structural records.
//!
//! Tables persist actual typed records and complete root-fronted encoded-ID
//! chains. The evaluator returns a role-keyed mirror for manual language
//! reification and reflection.
//!
//! Field roles cannot be transposed: the two position types differ at compile time.
//!
//! ```compile_fail
//! use structural_codec::{ApplicationHead, ApplicationPayload, Position};
//! enum Root {}
//! let payload: Position<ApplicationPayload, Root> = todo!();
//! let _: Position<ApplicationHead, Root> = payload;
//! ```
//!
//! Flat raw type ids and the old fixed-product vocabulary have no construction
//! surface.
//!
//! ```compile_fail
//! let _ = structural_codec::EncodedTypeId(7);
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
//! A target layout is derived data, not a zero-filled digest wrapper.
//!
//! ```compile_fail
//! let _ = structural_codec::TargetLayoutIdentity([0; 32]);
//! ```
//!

pub mod authoring;
pub mod codec;
pub mod conformance;
pub mod disjoint;
pub mod error;
pub mod evaluator;
pub mod form;
pub mod ids;
pub mod names;
pub mod plan;
pub mod table;
pub mod value;

pub use codec::{AcceptedDecodeForm, ConstructorCodec, StructuralEntry};
pub use conformance::{ConformanceError, ConformanceHarness, GeneratedCodec};
pub use error::{AuthoringError, DecodeError, DisjointnessError, EncodeError, TableError};
pub use evaluator::{
    StructuralEncoding, StructuralEvaluator, StructuralPlanning, TypedContextualDecoding,
};
pub use form::{
    AdjacentApplicationDelimitedBody, AdjacentApplicationDelimitedFieldView,
    AdjacentApplicationDelimitedHead, AdjacentApplicationDelimitedItems,
    AdjacentApplicationDelimitedRoot, AdjacentApplicationDelimitedRule, ApplicationDelimitedBody,
    ApplicationDelimitedFieldView, ApplicationDelimitedHead, ApplicationDelimitedItems,
    ApplicationDelimitedRoot, ApplicationDelimitedRule, ApplicationFieldView, ApplicationHead,
    ApplicationPayload, ApplicationRoot, ApplicationRule, AtomDescriptor, BorrowedFieldView,
    DelegationPayload, FieldEnd, FieldLink, FieldVisitor, LeafCodec, OrderedProduct,
    OrderedSequence, Position, RuleCoproduct, RuleCoproductView, SharedDescriptor, StructuralRule,
    StructuralRuleView, StructureRecord, UnaryRoot, UnaryRule,
};
pub use ids::{DecodeFormId, EncodedConstructorId, EncodedTypeId, FieldRole, StableRoleId};
pub use name_table::{EncodedId, LocalEncodedId, TableAddress};
pub use names::{
    DeclarationAssignment, DecodeNameBindings, EncodedNameResolver, NameOccurrence,
    ResolvedReference,
};
pub use plan::{PlannedFieldValue, PlannedName, PlannedStructuralValue};
pub use raw_discovery::AtomCase;
pub use table::{
    AddressedStructuralTable, ArchivedTablePayload, ContextualTextualPolicy,
    FixtureVocabularyDomain, StructuralTableDomain, StructuralVocabularyDomain,
    StructuralVocabularyIdentity, TableIdentityPayload, TargetLayoutDomain, TargetLayoutIdentity,
    TextualRenderingPolicy,
};
pub use value::{
    FieldValue, RoleKeyedMirror, ScalarValue, SourceBoundedStructuralValue, StructuralValue,
    StructuralValueRecord,
};

// The former internal corpus depended on the retired flat, locally allocating
// NameTable API. Its structural/refusal coverage is rehomed in the public
// downstream_authoring contract, which can only use supported consumer APIs.
