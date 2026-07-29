//! Equivalence contract for an independently authored codec and the shared
//! evaluator over the same read-only name inputs.

use raw_discovery::SealedTokenProfile;

use crate::error::{DecodeError, EncodeError};
use crate::evaluator::StructuralEvaluator;
use crate::form::{StructuralRule, StructureRecord};
use crate::ids::EncodedTypeId;
use crate::names::{DecodeNameBindings, EncodedNameResolver};
use crate::table::AddressedStructuralTable;
use crate::value::StructuralValue;

pub trait GeneratedCodec<Root>: Sized {
    fn core_type() -> EncodedTypeId<Root>;

    fn decode(
        source: &str,
        bindings: &impl DecodeNameBindings<Root>,
    ) -> Result<Self, DecodeError<Root>>;

    fn encode(
        &self,
        resolver: &impl EncodedNameResolver<Root>,
    ) -> Result<String, EncodeError<Root>>;

    fn to_structural(&self) -> StructuralValue<Root>;
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ConformanceError<Root> {
    #[error("interpreter and independent codec produced different structural values")]
    ValueMismatch,
    #[error("interpreter and independent codec produced different canonical output")]
    CanonicalOutput,
    #[error("interpreter and independent codec disagreed on whether decoding succeeded")]
    ErrorDisagreement,
    #[error(transparent)]
    Encode(#[from] EncodeError<Root>),
}

pub struct ConformanceHarness<'table, Root, Record = StructuralRule<Root>>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    evaluator: StructuralEvaluator<'table, Root, Record>,
    expected: EncodedTypeId<Root>,
}

impl<'table, Root, Record> ConformanceHarness<'table, Root, Record>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    pub fn new(
        table: &'table AddressedStructuralTable<Root, Record>,
        profile: &'table SealedTokenProfile,
        expected: EncodedTypeId<Root>,
    ) -> Result<Self, DecodeError<Root>> {
        Ok(Self {
            evaluator: StructuralEvaluator::with_profile(table, profile)?,
            expected,
        })
    }

    pub fn check<T, Bindings>(
        &self,
        fixtures: &[String],
        bindings: &Bindings,
    ) -> Result<(), ConformanceError<Root>>
    where
        T: GeneratedCodec<Root>,
        Bindings: DecodeNameBindings<Root>,
        StructuralValue<Root>: PartialEq,
    {
        for source in fixtures {
            let generated = T::decode(source, bindings);
            let interpreted = self.evaluator.decode_text(&self.expected, source, bindings);

            match (generated, interpreted) {
                (Ok(typed), Ok(mirror)) => {
                    if typed.to_structural() != mirror {
                        return Err(ConformanceError::ValueMismatch);
                    }
                    let generated_text = typed.encode(bindings)?;
                    let interpreted_text =
                        self.evaluator
                            .encode_text(&self.expected, &mirror, bindings)?;
                    if generated_text != interpreted_text {
                        return Err(ConformanceError::CanonicalOutput);
                    }
                }
                (Err(_), Err(_)) => {}
                _ => return Err(ConformanceError::ErrorDisagreement),
            }
        }
        Ok(())
    }
}
