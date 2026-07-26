//! [`TextualForm`] is the textual view value for an encoded form. [`Textual`] is the
//! textual interface that produces and consumes that view. It is the textual side of the
//! Protos pairing beside [`EncodedForm`] and
//! [`EncodedConversion`](crate::EncodedConversion).
//!
//! ## The textual interface
//!
//! An **EncodedForm** is a stringless Core value family. A [`TextualForm<T>`] is a
//! first-class textual view of that encoded form, not a bare string. A [`Textual`]
//! implementation uses:
//!
//! - the **nametree** — a [`NameTable`], written on decode and read on encode;
//! - the **structuretree** — an [`AddressedStructuralTable`], the sealed, data-driven
//!   encoder and decoder that [`StructuralEvaluator`] walks in both directions.
//!
//! [`view`](Textual::view) renders an encoded-form value as a [`TextualForm<T>`].
//! [`unview`](Textual::unview) reads a [`TextualForm<T>`] back into the encoded form. Both
//! use the trusted evaluator over the structuretree and nametree. The only language-specific
//! code is [`reify`](Textual::reify) and [`reflect`](Textual::reflect), which translate
//! between the generic [`StructuralValue`] and the language's encoded form.
//!
//! ## The textual view is a first-class value
//!
//! [`TextualForm<T>`] is an indexed set of named text chunks — a filename→text index —
//! so a unit that renders as many named files (or is read back from them) is expressible
//! as one value, symmetric for input and output. The common single-document case is the
//! trivial one-chunk index ([`TextualForm::single`] / [`TextualForm::sole_text`]). Text
//! lives only in the textual view, never inside an EncodedForm.
//!
//! `T` (the [`Language`](Textual::Language) marker) is the same identity the paired
//! [`EncodedForm<T>`](crate::EncodedForm) carries, so a language's encoded form, textual
//! view, and conversions all agree on one marker.
//!
//! [`SharedDescriptor`]: crate::form::SharedDescriptor

use std::marker::PhantomData;

use name_table::{NameTable, NameTableError, NameTransaction};
use raw_discovery::SealedTokenProfile;

use crate::encoded_form::EncodedForm;
use crate::error::{DecodeError, EncodeError, SingleChunkRequired};
use crate::evaluator::StructuralEvaluator;
use crate::form::{StructuralRule, StructureRecord};
use crate::ids::ScopedEncodedTypeId;
use crate::table::AddressedStructuralTable;
use crate::value::StructuralValue;

/// The rendered textual VIEW of an [`EncodedForm<T>`](crate::EncodedForm) — the
/// first-class value a [`Textual::view`] produces and a [`Textual::unview`] consumes. An
/// indexed set of named text chunks (a filename→text index); the common single-document
/// case is the trivial one-chunk index. The `Language` marker is the `T` in
/// `TextualForm<T>` — the identity the paired encoded form and textual interface share.
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

/// The textual interface for an [`EncodedForm<T>`](crate::EncodedForm). It produces and
/// consumes a [`TextualForm<T>`] view through the nametree and structuretree. Implement
/// the structuretree, optional literal lexicon, and [`reify`](Self::reify) /
/// [`reflect`](Self::reflect) translation between the generic structural value and the
/// encoded form. [`view`](Self::view) and [`unview`](Self::unview) are provided for every
/// language.
pub trait Textual<Record = StructuralRule>
where
    Record: StructureRecord,
{
    /// The EncodedForm this text is a view on — a stringless Core value family.
    /// Rust can express the fixed association directly: an implementation's
    /// encoded family and textual view carry exactly the same language marker.
    type Encoded: EncodedForm<Language = Self::Language>;

    /// The language marker `T` shared with the produced [`TextualForm<T>`] value and the
    /// paired [`EncodedForm<T>`](crate::EncodedForm).
    type Language;

    /// The crate-boundary error, constructible from the shared codec failures the
    /// provided operation raises. The one language-specific error an implementation must
    /// supply on top of these is [`missing_root_object`](Self::missing_root_object).
    type Error: From<DecodeError>
        + From<EncodeError>
        + From<NameTableError>
        + From<SingleChunkRequired>;

    /// The structuretree: the sealed table the trusted evaluator walks in both directions.
    /// This data defines the encoder and decoder.
    fn structuretree(&self) -> &AddressedStructuralTable<Record>;

    /// The separately sealed token profile whose identity the structuretree
    /// pins. It supplies every textual boundary spelling and carrier policy.
    fn token_profile(&self) -> &SealedTokenProfile;

    /// The lexicon the table's [`Literal`](crate::form::SharedDescriptor::Literal) descriptors
    /// resolve through; `None` when the table carries no literal keywords.
    fn lexicon(&self) -> Option<&NameTable> {
        None
    }

    /// The error this textual interface raises when a source holds no root object to unview.
    fn missing_root_object(&self) -> Self::Error;

    /// Reify a decoded generic structural value into the EncodedForm. This is the only
    /// place a language's value shapes are read from the shared value type.
    fn reify(
        &self,
        expected: ScopedEncodedTypeId,
        mirror: &StructuralValue,
        names: &mut NameTransaction<'_>,
    ) -> Result<Self::Encoded, Self::Error>;

    /// Reflect an EncodedForm value into the generic structural value the evaluator renders.
    /// This is the only place a language's value shapes are written into the shared value type.
    fn reflect(
        &self,
        expected: ScopedEncodedTypeId,
        encoded: &Self::Encoded,
        names: &NameTable,
    ) -> Result<StructuralValue, Self::Error>;

    // ===== provided textual-interface operations (identical for every language) =====

    /// The trusted evaluator over the nametree and structuretree, with the literal lexicon
    /// when the table carries `Literal` forms and without it otherwise.
    fn evaluator(&self) -> Result<StructuralEvaluator<'_, Record>, Self::Error> {
        match self.lexicon() {
            Some(lexicon) => StructuralEvaluator::with_profile_and_lexicon(
                self.structuretree(),
                self.token_profile(),
                lexicon,
            )
            .map_err(Self::Error::from),
            None => StructuralEvaluator::with_profile(self.structuretree(), self.token_profile())
                .map_err(Self::Error::from),
        }
    }

    /// Read a [`TextualForm<T>`] back into the EncodedForm through the nametree and
    /// structuretree. Execute the view's sole document directly against the expected
    /// type, then reify the resulting generic structural value into the EncodedForm.
    /// The expected type drives the evaluator; the input never selects its own type.
    fn unview(
        &self,
        expected: ScopedEncodedTypeId,
        view: &TextualForm<Self::Language>,
        names: &mut NameTable,
    ) -> Result<Self::Encoded, Self::Error> {
        let text = view.sole_text()?;
        if text.is_empty() {
            return Err(self.missing_root_object());
        }
        let evaluator = self.evaluator()?;
        names.try_intern(|transaction| {
            let mirror = evaluator
                .decode_text_with_interner(expected, text, transaction)
                .map_err(Self::Error::from)?;
            self.reify(expected, &mirror, transaction)
        })
    }

    /// Render an EncodedForm value as a [`TextualForm<T>`] through the nametree and
    /// structuretree. Reflect it into the generic structural value, let the trusted evaluator
    /// execute that value through the canonical encode form and pinned token profile,
    /// then package the canonical text as the sole chunk of the view.
    fn view(
        &self,
        expected: ScopedEncodedTypeId,
        encoded: &Self::Encoded,
        names: &NameTable,
    ) -> Result<TextualForm<Self::Language>, Self::Error> {
        let mirror = self.reflect(expected, encoded, names)?;
        let text = self.evaluator()?.encode_text(expected, &mirror, names)?;
        Ok(TextualForm::single(text))
    }
}
