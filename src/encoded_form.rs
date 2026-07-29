//! Stringless encoded values and typed layer conversion.
//!
//! Full encoded-ID chains travel inside the values themselves. This module
//! does not invent a module-table/Capsule pin composition.

/// The truth-side marker paired with a textual view.
pub trait EncodedForm {
    /// The caller-supplied vocabulary root enum carried by every encoded ID.
    type VocabularyRoot;
    /// The language family whose textual view this value may use.
    type Language;
}

/// The output of a stringless layer conversion.
#[derive(Clone, Debug)]
pub struct Converted<Target> {
    pub target: Target,
}

/// A typed `EncodedForm<T> -> EncodedForm<X>` conversion.
///
/// No spelling table is read, allocated, flattened, or composed here. Source
/// encoded-ID chains are carried or transformed as typed data by the concrete
/// converter.
pub trait EncodedConversion {
    type Source: EncodedForm;
    type Target: EncodedForm<VocabularyRoot = <Self::Source as EncodedForm>::VocabularyRoot>;
    type Error;

    fn convert(&self, source: &Self::Source) -> Result<Converted<Self::Target>, Self::Error>;
}
