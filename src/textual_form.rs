//! [`TextualForm`] — the give-a-language-a-mouth operation, seated once as the shared
//! trait every family member (schema, logos, Nomos, the Rust form) implements.
//!
//! ## The relationship the trait models (ruled)
//!
//! An **EncodedForm** — a stringless Core value family — is the truth. A
//! **TextualForm** is one textual VIEW on it, produced and consumed through the two
//! organs:
//!
//! - the **nametree** — a [`NameTable`], written on decode and read on encode;
//! - the **structuretree** — an [`AddressedStructuralTable`], the sealed, data-driven
//!   enc/decoder ([`StructuralEvaluator`] walks it in both directions).
//!
//! [`view`](TextualForm::view) renders an encoded value as canonical text;
//! [`unview`](TextualForm::unview) reads text back into the encoded value. Both run
//! through the ONE trusted evaluator over the sealed structuretree plus the nametree —
//! never a bespoke, per-type text parse or print path. The only per-language code is
//! [`reify`](TextualForm::reify) / [`reflect`](TextualForm::reflect): the translation
//! between the generic [`StructuralValue`] mirror the evaluator speaks and the
//! language's own EncodedForm. A language grows a new capability by extending the
//! structuretree's form vocabulary (a new [`StructuralForm`] shape, sealed and
//! disjointness-proved), never by reading or writing text outside the organs.
//!
//! [`StructuralForm`]: crate::form::StructuralForm

use name_table::{NameResolver, NameTable};
use raw_discovery::{RecognizeError, Recognizer};

use crate::error::{DecodeError, EncodeError};
use crate::evaluator::StructuralEvaluator;
use crate::ids::ScopedCoreTypeId;
use crate::table::AddressedStructuralTable;
use crate::value::StructuralValue;
use crate::writer::CanonicalText;

/// One textual mouth of an EncodedForm: text ⇄ Core, driven entirely by the two
/// organs. Implement the four required members (the structuretree organ, the optional
/// literal lexicon, and the [`reify`](Self::reify) / [`reflect`](Self::reflect)
/// mirror translation) and the [`view`](Self::view) / [`unview`](Self::unview)
/// operation is provided — the same operation for every language.
pub trait TextualForm {
    /// The EncodedForm this text is a view on — a stringless Core value family.
    type Encoded;

    /// The crate-boundary error, constructible from the shared codec failures the
    /// provided operation raises. The one language-specific error a mouth must supply
    /// on top of these is [`missing_root_object`](Self::missing_root_object).
    type Error: From<RecognizeError> + From<DecodeError> + From<EncodeError>;

    /// The structuretree organ: the sealed table the trusted evaluator walks in both
    /// directions. This is the data-driven enc/decoder itself, expressed as data.
    fn structuretree(&self) -> &AddressedStructuralTable;

    /// The lexicon the table's [`Literal`](crate::form::StructuralForm::Literal) forms
    /// resolve through; `None` when the table carries no literal keywords.
    fn lexicon(&self) -> Option<&dyn NameResolver> {
        None
    }

    /// The error this mouth raises when a source held no root object to un-view.
    fn missing_root_object(&self) -> Self::Error;

    /// un-view: reify a decoded generic mirror into the EncodedForm. The only place a
    /// language's own value shapes are read out of the shared mirror.
    fn reify(
        &self,
        expected: ScopedCoreTypeId,
        mirror: &StructuralValue,
        names: &mut NameTable,
    ) -> Result<Self::Encoded, Self::Error>;

    /// view: reflect an EncodedForm value into the generic mirror the evaluator
    /// renders. The only place a language's own value shapes are written into the
    /// shared mirror.
    fn reflect(
        &self,
        expected: ScopedCoreTypeId,
        encoded: &Self::Encoded,
        names: &mut NameTable,
    ) -> Result<StructuralValue, Self::Error>;

    // ===== the provided give-a-mouth operation (identical for every language) =====

    /// The trusted evaluator over the two organs — with the literal lexicon when the
    /// table carries `Literal` forms, plain otherwise.
    fn evaluator(&self) -> StructuralEvaluator<'_> {
        match self.lexicon() {
            Some(lexicon) => StructuralEvaluator::with_lexicon(self.structuretree(), lexicon),
            None => StructuralEvaluator::new(self.structuretree()),
        }
    }

    /// un-view NOTA-family text into the EncodedForm through the two organs: recognize
    /// the source structure, let the trusted evaluator decode it against the expected
    /// type into the generic mirror, then reify the mirror into the EncodedForm. The
    /// expected type drives the evaluator; the input never selects its own type.
    fn unview(
        &self,
        expected: ScopedCoreTypeId,
        text: &str,
        names: &mut NameTable,
    ) -> Result<Self::Encoded, Self::Error> {
        let document = Recognizer::standard().recognize(text)?;
        let block = document
            .root_object_at(0)
            .ok_or_else(|| self.missing_root_object())?;
        let mirror = self.evaluator().decode(expected, block, names)?;
        self.reify(expected, &mirror, names)
    }

    /// view an EncodedForm value as canonical NOTA-family text through the two organs:
    /// reflect it into the generic mirror, let the trusted evaluator render the mirror
    /// to a block off the one canonical encode form, then write the canonical text.
    fn view(
        &self,
        expected: ScopedCoreTypeId,
        encoded: &Self::Encoded,
        names: &mut NameTable,
    ) -> Result<String, Self::Error> {
        let mirror = self.reflect(expected, encoded, names)?;
        let block = self.evaluator().encode(expected, &mirror, names)?;
        Ok(block.canonical_text())
    }
}
