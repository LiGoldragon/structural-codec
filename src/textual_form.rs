//! [`TextualForm`] — the textual VIEW value — and [`Textual`], the give-a-language-a-mouth
//! operation that produces and consumes it. The view side of the Protos pairing, seated
//! beside the truth side ([`EncodedForm`](crate::EncodedForm) /
//! [`EncodedConversion`](crate::EncodedConversion)).
//!
//! ## The relationship the pair models (ruled)
//!
//! An **EncodedForm** — a stringless Core value family — is the truth. A
//! [`TextualForm<T>`] is one textual VIEW on it: a first-class VALUE (not a bare
//! string), produced and consumed through a [`Textual`] driving the two organs:
//!
//! - the **nametree** — a [`NameTable`], written on decode and read on encode;
//! - the **structuretree** — an [`AddressedStructuralTable`], the sealed, data-driven
//!   enc/decoder ([`StructuralEvaluator`] walks it in both directions).
//!
//! [`view`](Textual::view) renders an EncodedForm value as a [`TextualForm<T>`];
//! [`unview`](Textual::unview) reads a [`TextualForm<T>`] back into the EncodedForm. Both
//! run through the ONE trusted evaluator over the sealed structuretree plus the
//! nametree — never a bespoke, per-type text parse or print path. The only per-language
//! code is [`reify`](Textual::reify) / [`reflect`](Textual::reflect): the translation
//! between the generic [`StructuralValue`] mirror the evaluator speaks and the
//! language's own EncodedForm.
//!
//! ## The view is a first-class value (ruled refinement)
//!
//! [`TextualForm<T>`] is an indexed set of named text chunks — a filename→text index —
//! so a unit that renders as many named files (or is read back from them) is expressible
//! as ONE value, symmetric for input and output. The common single-document case is the
//! trivial one-chunk index ([`TextualForm::single`] / [`TextualForm::sole_text`]). Text
//! lives ONLY here, inside the mouth's currency — never inside an EncodedForm.
//!
//! `T` (the [`Language`](Textual::Language) marker) is the same identity the paired
//! [`EncodedForm<T>`](crate::EncodedForm) carries, so a language's truth, view, and
//! conversions all agree on one marker.
//!
//! [`StructuralForm`]: crate::form::StructuralForm

use std::marker::PhantomData;

use name_table::{NameResolver, NameTable};
use raw_discovery::{RecognizeError, Recognizer};

use crate::error::{DecodeError, EncodeError, SingleChunkRequired};
use crate::evaluator::StructuralEvaluator;
use crate::ids::ScopedCoreTypeId;
use crate::table::AddressedStructuralTable;
use crate::value::StructuralValue;
use crate::writer::CanonicalText;

/// The rendered textual VIEW of an [`EncodedForm<T>`](crate::EncodedForm) — the
/// first-class value a [`Textual::view`] produces and a [`Textual::unview`] consumes. An
/// indexed set of named text chunks (a filename→text index); the common single-document
/// case is the trivial one-chunk index. The `Language` marker is the `T` in
/// `TextualForm<T>` — the identity the paired encoded form and mouth share.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextualForm<Language> {
    chunks: Vec<TextChunk>,
    language: PhantomData<fn() -> Language>,
}

/// One named text chunk of a [`TextualForm`] — a filename paired with its rendered text.
/// A single-file view carries exactly one, filed under the [`unit`](ChunkName::unit)
/// name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChunk {
    /// The name this chunk is filed under (a filename, in a multi-file view).
    pub name: ChunkName,
    /// The chunk's rendered text.
    pub text: String,
}

/// The name a [`TextChunk`] is filed under inside a [`TextualForm`] index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkName(pub String);

impl ChunkName {
    /// The canonical name of the sole chunk in a single-document view.
    pub fn unit() -> Self {
        ChunkName("unit".to_string())
    }
}

impl<Language> TextualForm<Language> {
    /// The trivial single-chunk view: one document filed under the
    /// [`unit`](ChunkName::unit) name.
    pub fn single(text: String) -> Self {
        Self::from_chunks(vec![TextChunk {
            name: ChunkName::unit(),
            text,
        }])
    }

    /// A view over an explicit set of named chunks (the multi-file index).
    pub fn from_chunks(chunks: Vec<TextChunk>) -> Self {
        Self {
            chunks,
            language: PhantomData,
        }
    }

    /// The chunks this view carries, in index order.
    pub fn chunks(&self) -> &[TextChunk] {
        &self.chunks
    }

    /// The sole chunk's text for the single-document case; a loud, typed error when the
    /// view is empty or carries several chunks (the multi-chunk un-view is deferred).
    pub fn sole_text(&self) -> Result<&str, SingleChunkRequired> {
        match self.chunks.as_slice() {
            [chunk] => Ok(chunk.text.as_str()),
            other => Err(SingleChunkRequired { count: other.len() }),
        }
    }
}

/// One textual mouth of an [`EncodedForm<T>`](crate::EncodedForm): a [`TextualForm<T>`]
/// ⇄ Core view, driven entirely by the two organs. Implement the required members (the
/// structuretree organ, the optional literal lexicon, and the [`reify`](Self::reify) /
/// [`reflect`](Self::reflect) mirror translation) and the [`view`](Self::view) /
/// [`unview`](Self::unview) operation is provided — the same operation for every
/// language.
pub trait Textual {
    /// The EncodedForm this text is a view on — a stringless Core value family.
    type Encoded;

    /// The language marker `T` shared with the produced [`TextualForm<T>`] value and the
    /// paired [`EncodedForm<T>`](crate::EncodedForm).
    type Language;

    /// The crate-boundary error, constructible from the shared codec failures the
    /// provided operation raises. The one language-specific error a mouth must supply on
    /// top of these is [`missing_root_object`](Self::missing_root_object).
    type Error: From<RecognizeError>
        + From<DecodeError>
        + From<EncodeError>
        + From<SingleChunkRequired>;

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

    /// view: reflect an EncodedForm value into the generic mirror the evaluator renders.
    /// The only place a language's own value shapes are written into the shared mirror.
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

    /// un-view a [`TextualForm<T>`] back into the EncodedForm through the two organs:
    /// recognize the view's sole document, let the trusted evaluator decode it against
    /// the expected type into the generic mirror, then reify the mirror into the
    /// EncodedForm. The expected type drives the evaluator; the input never selects its
    /// own type.
    fn unview(
        &self,
        expected: ScopedCoreTypeId,
        view: &TextualForm<Self::Language>,
        names: &mut NameTable,
    ) -> Result<Self::Encoded, Self::Error> {
        let text = view.sole_text()?;
        let document = Recognizer::standard().recognize(text)?;
        let block = document
            .root_object_at(0)
            .ok_or_else(|| self.missing_root_object())?;
        let mirror = self.evaluator().decode(expected, block, names)?;
        self.reify(expected, &mirror, names)
    }

    /// view an EncodedForm value as a [`TextualForm<T>`] through the two organs: reflect
    /// it into the generic mirror, let the trusted evaluator render the mirror to a block
    /// off the one canonical encode form, then package the canonical text as the sole
    /// chunk of the view.
    fn view(
        &self,
        expected: ScopedCoreTypeId,
        encoded: &Self::Encoded,
        names: &mut NameTable,
    ) -> Result<TextualForm<Self::Language>, Self::Error> {
        let mirror = self.reflect(expected, encoded, names)?;
        let block = self.evaluator().encode(expected, &mirror, names)?;
        Ok(TextualForm::single(block.canonical_text()))
    }
}
