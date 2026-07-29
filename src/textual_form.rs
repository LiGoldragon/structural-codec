//! First-class textual views over stringless encoded values.

use std::marker::PhantomData;

use crate::encoded_form::EncodedForm;
use crate::error::{DecodeError, EncodeError, SingleChunkRequired};
use crate::evaluator::StructuralEvaluator;
use crate::form::{StructuralRule, StructureRecord};
use crate::ids::EncodedTypeId;
use crate::names::{DecodeNameBindings, EncodedNameResolver};
use crate::table::AddressedStructuralTable;
use crate::value::StructuralValue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextualForm<Language> {
    chunks: Vec<TextChunk>,
    language: PhantomData<fn() -> Language>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChunk {
    pub name: ChunkName,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkName(pub String);

impl ChunkName {
    pub fn unit() -> Self {
        Self("unit".to_owned())
    }
}

impl<Language> TextualForm<Language> {
    pub fn single(text: String) -> Self {
        Self::from_chunks(vec![TextChunk {
            name: ChunkName::unit(),
            text,
        }])
    }

    pub fn from_chunks(chunks: Vec<TextChunk>) -> Self {
        Self {
            chunks,
            language: PhantomData,
        }
    }

    pub fn chunks(&self) -> &[TextChunk] {
        &self.chunks
    }

    pub fn sole_text(&self) -> Result<&str, SingleChunkRequired> {
        match self.chunks.as_slice() {
            [chunk] => Ok(chunk.text.as_str()),
            other => Err(SingleChunkRequired { count: other.len() }),
        }
    }
}

/// A textual interface driven by one shared structural evaluator.
pub trait Textual<Root, Record = StructuralRule<Root>>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    type Encoded: EncodedForm<VocabularyRoot = Root, Language = Self::Language>;
    type Language;
    type Error: From<DecodeError<Root>> + From<EncodeError<Root>> + From<SingleChunkRequired>;

    fn structuretree(&self) -> &AddressedStructuralTable<Root, Record>;
    fn missing_root_object(&self) -> Self::Error;

    fn reify(
        &self,
        expected: &EncodedTypeId<Root>,
        mirror: &StructuralValue<Root>,
    ) -> Result<Self::Encoded, Self::Error>;

    fn reflect(
        &self,
        expected: &EncodedTypeId<Root>,
        encoded: &Self::Encoded,
    ) -> Result<StructuralValue<Root>, Self::Error>;

    fn evaluator(&self) -> Result<StructuralEvaluator<'_, Root, Record>, Self::Error> {
        StructuralEvaluator::new(self.structuretree()).map_err(Self::Error::from)
    }

    fn unview<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        view: &TextualForm<Self::Language>,
        bindings: &Bindings,
    ) -> Result<Self::Encoded, Self::Error> {
        let text = view.sole_text()?;
        if text.is_empty() {
            return Err(self.missing_root_object());
        }
        let mirror = self
            .evaluator()?
            .decode_text(expected, text, bindings)
            .map_err(Self::Error::from)?;
        self.reify(expected, &mirror)
    }

    fn view<Resolver: EncodedNameResolver<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        encoded: &Self::Encoded,
        resolver: &Resolver,
    ) -> Result<TextualForm<Self::Language>, Self::Error> {
        let mirror = self.reflect(expected, encoded)?;
        let text = self.evaluator()?.encode_text(expected, &mirror, resolver)?;
        Ok(TextualForm::single(text))
    }
}
