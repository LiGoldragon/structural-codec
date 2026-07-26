//! Law 5 scaffolding: the conformance contract between the trusted evaluator and a
//! future downstream codec. The `GeneratedCodec` trait is the shape the generated
//! or independently authored side will implement; the
//! `ConformanceHarness` exercises it against the evaluator and asserts agreement on
//! the encoded value, the NameTable delta, the canonical output, and the typed-error
//! decision. TODAY the evaluator is the sole implementation — this trait has no
//! generated implementor yet, so the harness is compiled-but-dormant scaffolding.
//! Its downstream witness remains homeless while the former derive repository is
//! frozen; this crate does not revive a derive path.

use name_table::{IdentifierNamespace, NameResolver, NameTable, NameTableError};
use raw_discovery::SealedTokenProfile;

use crate::error::{DecodeError, EncodeError};
use crate::evaluator::StructuralEvaluator;
use crate::form::{StructuralRule, StructureRecord};
use crate::ids::ScopedEncodedTypeId;
use crate::table::AddressedStructuralTable;
use crate::value::StructuralValue;

/// The contract a generated codec implements so it can be proven equivalent to the
/// evaluator over the same fixtures.
pub trait GeneratedCodec: Sized {
    const CORE_TYPE: ScopedEncodedTypeId;

    fn decode(source: &str, names: &mut NameTable) -> Result<Self, DecodeError>;
    fn encode<Resolver: NameResolver>(&self, resolver: &Resolver) -> Result<String, EncodeError>;
    fn to_structural(&self) -> StructuralValue;
}

/// Where the interpreter and a generated codec disagreed.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConformanceError {
    #[error("interpreter and generated codec produced different structural values")]
    ValueMismatch,
    #[error("interpreter and generated codec left different NameTable deltas")]
    NameTableDelta,
    #[error("interpreter and generated codec produced different canonical output")]
    CanonicalOutput,
    #[error("interpreter and generated codec disagreed on whether decoding succeeded")]
    ErrorDisagreement,
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error(transparent)]
    Names(#[from] NameTableError),
}

/// Runs the conformance contract for one expected type over a fixture set.
pub struct ConformanceHarness<'table, Record = StructuralRule>
where
    Record: StructureRecord,
{
    evaluator: StructuralEvaluator<'table, Record>,
    expected: ScopedEncodedTypeId,
}

impl<'table, Record: StructureRecord> ConformanceHarness<'table, Record> {
    pub fn new(
        table: &'table AddressedStructuralTable<Record>,
        profile: &'table SealedTokenProfile,
        expected: ScopedEncodedTypeId,
    ) -> Result<Self, DecodeError> {
        Ok(Self {
            evaluator: StructuralEvaluator::with_profile(table, profile)?,
            expected,
        })
    }

    /// Assert the generated codec `T` agrees with the evaluator on every fixture.
    pub fn check<T: GeneratedCodec>(&self, fixtures: &[String]) -> Result<(), ConformanceError> {
        for source in fixtures {
            let mut names_generated = NameTable::new(IdentifierNamespace::Fixture);
            let generated = T::decode(source, &mut names_generated);

            let mut names_interpreted = NameTable::new(IdentifierNamespace::Fixture);
            let interpreted =
                self.evaluator
                    .decode_text(self.expected, source, &mut names_interpreted);

            match (generated, interpreted) {
                (Ok(typed), Ok(mirror)) => {
                    if typed.to_structural() != mirror {
                        return Err(ConformanceError::ValueMismatch);
                    }
                    if names_generated.to_archive_bytes()?.as_ref()
                        != names_interpreted.to_archive_bytes()?.as_ref()
                    {
                        return Err(ConformanceError::NameTableDelta);
                    }
                    let generated_text = typed.encode(&names_generated)?;
                    let interpreted_text =
                        self.evaluator
                            .encode_text(self.expected, &mirror, &names_interpreted)?;
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
