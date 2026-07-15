//! The codec unit is the Core CONSTRUCTOR, and it is ASYMMETRIC: several
//! structurally-disjoint accepted decode forms, exactly one canonical encode form,
//! and a positional signature that MUST equal the constructor's Core field
//! signature (§4.6). A `StructuralEntry` gathers every constructor of one Core type.

use crate::form::StructuralForm;
use crate::ids::{CoreConstructorId, PositionalSignature, ScopedCoreTypeId};

/// One Core constructor's codec.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub struct ConstructorCodec {
    pub constructor: CoreConstructorId,
    /// Disjoint accepted inputs, proved non-shadowing by the disjointness checker.
    #[rkyv(omit_bounds)]
    pub decode_forms: Vec<StructuralForm>,
    /// The single canonical output form.
    #[rkyv(omit_bounds)]
    pub encode_form: StructuralForm,
    /// Positional; must equal the constructor's Core field signature.
    pub signature: PositionalSignature,
}

impl ConstructorCodec {
    pub fn new(
        constructor: CoreConstructorId,
        decode_forms: Vec<StructuralForm>,
        encode_form: StructuralForm,
        signature: PositionalSignature,
    ) -> Self {
        Self {
            constructor,
            decode_forms,
            encode_form,
            signature,
        }
    }
}

/// Every constructor codec for one Core type, keyed on decode by the expected type.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub struct StructuralEntry {
    pub core_type: ScopedCoreTypeId,
    #[rkyv(omit_bounds)]
    pub constructors: Vec<ConstructorCodec>,
}

impl StructuralEntry {
    pub fn new(core_type: ScopedCoreTypeId, constructors: Vec<ConstructorCodec>) -> Self {
        Self {
            core_type,
            constructors,
        }
    }

    /// The constructor codec at a decode-chosen index.
    pub fn constructor_at(&self, index: usize) -> Option<&ConstructorCodec> {
        self.constructors.get(index)
    }
}
