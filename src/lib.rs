//! # structural-codec
//!
//! The Core-associated, bidirectional, revisioned structural-form kernel of the
//! next-generation NOTA family, with the trusted evaluator that SHIPS IN THE
//! RUNTIME. Dialect tables are data-loadable at runtime and executed directly, both
//! directions, off the SAME forms — so round-trip coherence holds by construction.
//!
//! ## The kernel / authoring split
//!
//! [`StructuralForm`] is a minimal seven-case kernel. The psyche's named authoring
//! structs ([`authoring::ObjectSymbolPrefixedBlock`], [`authoring::DottedForm`]) live
//! in the AUTHORING vocabulary and [`authoring::AuthoringForm::normalize`] to kernel
//! forms before a form is ever hashed or evaluated, so the kernel stays small.
//!
//! ## The pieces
//!
//! - Identity keys: [`ids`] — universes, scoped Core-type ids, constructor ids.
//! - Forms: [`form`] (kernel) and [`authoring`] (the normalizing surface).
//! - Codecs: [`codec::ConstructorCodec`] (asymmetric: many disjoint decode forms, one
//!   canonical encode form) gathered per type in [`codec::StructuralEntry`].
//! - Table: [`table::AddressedStructuralTable`] — the external sidecar keyed by
//!   `ScopedCoreTypeId`, its content identity stored OUTSIDE the hashed payload and
//!   EXCLUDED from Core value identity.
//! - Disjointness: [`disjoint`] — conservative outer-shape validation; overlap it
//!   cannot rule out is a hard error.
//! - Evaluator: [`evaluator::StructuralEvaluator`] — the one trusted interpreter.
//! - Mirror: [`value::StructuralValue`] — the Core-agnostic generic currency.
//! - Conformance: [`conformance`] — the law-5 harness the generated codec will meet.
//! - Fixtures: [`fixture`] — the proof-of-concept universe and the acceptance gate.
//! - The Protos pairing: [`encoded_form`] — the TRUTH side ([`EncodedForm`] marker plus
//!   the typed [`EncodedConversion`] layer conversion `EncodedForm<T> -> EncodedForm<X>`,
//!   text-free) — beside [`textual_form`] — the VIEW side ([`Textual`] producing a
//!   first-class [`TextualForm<T>`] value through the two organs).

pub mod authoring;
pub mod codec;
pub mod conformance;
pub mod disjoint;
pub mod encoded_form;
pub mod error;
pub mod evaluator;
pub mod fixture;
pub mod form;
pub mod ids;
pub mod table;
pub mod textual_form;
pub mod value;
pub mod writer;

pub use codec::{ConstructorCodec, StructuralEntry};
pub use conformance::{ConformanceError, ConformanceHarness, GeneratedCodec};
pub use encoded_form::{Converted, EncodedConversion, EncodedForm};
pub use error::{DecodeError, DisjointnessError, EncodeError, TableError};
pub use evaluator::StructuralEvaluator;
pub use form::{
    AtomForm, CarrierLeaf, CaseExpectation, ForeignLeafId, LeafCodec, LeafForm, ScalarLeaf,
    SequenceForm, SigilPosition, SigilSpec, StructuralForm,
};
pub use ids::{
    CoreConstructorId, CoreUniverseId, FIXTURE_UNIVERSE, PositionalSignature, ScopedCoreTypeId,
    StructuralRevision,
};
pub use table::{
    AddressedStructuralTable, CoreLayoutIdentity, LeafCodecContractId, RawProfileIdentity,
    StructuralTableDomain, TableIdentityPayload,
};
pub use textual_form::{ChunkName, TextChunk, Textual, TextualForm};
pub use value::{ScalarValue, StructuralValue, StructuralValueDomain};
pub use writer::CanonicalText;
