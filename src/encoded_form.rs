//! [`EncodedForm`] and [`EncodedConversion`] — the TRUTH-side half of the Protos
//! library pairing, seated beside the [`Textual`](crate::textual_form::Textual) view side.
//!
//! ## The pairing (ruled)
//!
//! A language family member has two faces of one truth:
//!
//! - its **EncodedForm** — a stringless, Core-associated value family (its names live
//!   in the nametree [`NameTable`], its shapes in the structuretree
//!   [`AddressedStructuralTable`](crate::table::AddressedStructuralTable)); and
//! - its **TextualForm** — one textual VIEW on that EncodedForm, produced and consumed
//!   through a [`Textual`](crate::textual_form::Textual).
//!
//! [`EncodedForm`] marks the truth side. A concrete Core value type (`EncodedSchema`, the
//! lowered logos item set) IS an EncodedForm for its language `T`; the marker ties the
//! value family to the language identity the paired [`TextualForm`](crate::TextualForm)
//! and [`Textual`](crate::textual_form::Textual) share.
//!
//! ## The layer conversion — `EncodedForm<T> -> EncodedForm<X>` (ruled)
//!
//! [`EncodedConversion`] is the reusable piece the library creates for the psyche's
//! *real type conversion*: a language layer is converted to the next by moving its
//! EncodedForm to another EncodedForm, threading a composed nametree — and NO text
//! appears anywhere on the path. The schema→logos lowering through the Nomos macros is
//! the first instance: `EncodedSchema` (EncodedForm of schema) plus its `NameTable` go in,
//! and the lowered logos items (EncodedForm of logos) plus a `NameTable` with a
//! component-owned Logos home namespace and complete borrowed source namespace slices
//! come out, entirely as typed data.
//!
//! ### On the generic spelling
//!
//! The psyche named the shape `EncodedForm<T> -> EncodedForm<X> or similar`. Rust's
//! trait system expresses "generic over the language `T`" through an associated
//! [`Language`](EncodedForm::Language) marker rather than a type parameter on the trait
//! itself, so a value type implements `EncodedForm` once and NAMES its language; the
//! conversion's [`Source`](EncodedConversion::Source) and
//! [`Target`](EncodedConversion::Target) are those two encoded forms. This is the
//! closest faithful expression; where it differs from the literal `EncodedForm<T>` it
//! differs only in where the `T` is written (an associated type, not a parameter).

use name_table::NameTable;

/// The truth-side marker of the Protos pairing: a stringless, Core-associated encoded
/// value family — the thing a [`Textual`](crate::textual_form::Textual) views and an
/// [`EncodedConversion`] moves. Implemented by a language's own Core value type
/// (`EncodedSchema` and the lowered logos item set are the first instances), it carries no
/// text: names live in the nametree, shapes in the structuretree.
///
/// [`Language`](Self::Language) is the `T` in `EncodedForm<T>` — the identity the paired
/// [`TextualForm`](crate::TextualForm) view and [`Textual`](crate::textual_form::Textual)
/// share, so a language's truth, view, and conversions all agree on one marker.
pub trait EncodedForm {
    /// The language this encoded value family belongs to (the `T` in `EncodedForm<T>`).
    type Language;
}

/// The output of an [`EncodedConversion`]: the produced target EncodedForm plus the
/// composed nametree that resolves every identifier it carries. The target owns its
/// home namespace and borrows each complete source namespace slice. Composition
/// preserves identifier variants and local indices without copying, flattening, or
/// renumbering names.
#[derive(Clone, Debug)]
pub struct Converted<Target> {
    /// The produced target EncodedForm (`EncodedForm<X>`).
    pub target: Target,
    /// The composed nametree resolving the target's identifiers.
    pub names: NameTable,
}

/// A typed layer conversion `EncodedForm<T> -> EncodedForm<X>`, expressed entirely as
/// data with NO text on the path. The composed nametree crosses the layer: the source
/// EncodedForm and its names go in, and the target EncodedForm plus its component-owned
/// home namespace and borrowed complete source slices come out. The schema→logos
/// lowering through the Nomos macros is the first instance.
///
/// The absence of any `&str` / `String` in this signature is the structural proof of the
/// psyche's ruling: the conversion is a real type conversion, with no string
/// manipulation. Text enters the family only through a
/// [`Textual`](crate::textual_form::Textual), never here.
pub trait EncodedConversion {
    /// The source EncodedForm (`EncodedForm<T>`).
    type Source;
    /// The produced target EncodedForm (`EncodedForm<X>`).
    type Target;
    /// The conversion's typed failure.
    type Error;

    /// Convert the source EncodedForm into the target, threading the composed
    /// nametree: `names` resolves the source identifiers, and the returned
    /// [`Converted`] carries a target-owned home namespace plus complete borrowed source
    /// slices resolving the target's identifiers. No string is read or written on this
    /// path.
    fn convert(
        &self,
        source: &Self::Source,
        names: &NameTable,
    ) -> Result<Converted<Self::Target>, Self::Error>;
}
