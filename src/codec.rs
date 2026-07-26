//! Constructor-addressed archived typed rule records.

use crate::form::StructuralRule;
use crate::ids::{DecodeFormId, EncodedConstructorId, ScopedEncodedTypeId};

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct AcceptedDecodeForm<Record = StructuralRule> {
    identity: DecodeFormId,
    rule: Record,
}

impl<Record> AcceptedDecodeForm<Record> {
    /// Construct one accepted decode form. Duplicate identities are refused by
    /// table sealing within the owning constructor.
    pub fn new(identity: DecodeFormId, rule: Record) -> Self {
        Self { identity, rule }
    }

    pub fn identity(&self) -> DecodeFormId {
        self.identity
    }

    pub fn rule(&self) -> &Record {
        &self.rule
    }
}

/// One constructor's many disjoint accepted forms and one canonical form.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ConstructorCodec<Record = StructuralRule> {
    constructor: EncodedConstructorId,
    decode_forms: Vec<AcceptedDecodeForm<Record>>,
    encode_form: Record,
}

impl<Record> ConstructorCodec<Record> {
    /// Construct a codec under its explicit constructor identity. Sealing
    /// verifies that identity belongs to the enclosing entry and that decode
    /// form identities are unique.
    pub fn new(
        constructor: EncodedConstructorId,
        decode_forms: Vec<AcceptedDecodeForm<Record>>,
        encode_form: Record,
    ) -> Self {
        Self {
            constructor,
            decode_forms,
            encode_form,
        }
    }

    pub fn constructor(&self) -> EncodedConstructorId {
        self.constructor
    }

    pub fn decode_forms(&self) -> &[AcceptedDecodeForm<Record>] {
        &self.decode_forms
    }

    pub fn encode_form(&self) -> &Record {
        &self.encode_form
    }
}

/// Every constructor codec for one encoded type. Constructor identities, not
/// vector order, choose the canonical encoder.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct StructuralEntry<Record = StructuralRule> {
    encoded_type: ScopedEncodedTypeId,
    constructors: Vec<ConstructorCodec<Record>>,
}

impl<Record> StructuralEntry<Record> {
    /// Construct all codecs for one encoded type. The table seal is the
    /// uniqueness and disjointness boundary for this collection.
    pub fn new(
        encoded_type: ScopedEncodedTypeId,
        constructors: Vec<ConstructorCodec<Record>>,
    ) -> Self {
        Self {
            encoded_type,
            constructors,
        }
    }

    pub fn encoded_type(&self) -> ScopedEncodedTypeId {
        self.encoded_type
    }

    pub fn constructors(&self) -> &[ConstructorCodec<Record>] {
        &self.constructors
    }
}
