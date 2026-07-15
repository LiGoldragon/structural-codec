//! Law 5 scaffolding: the conformance contract between the trusted evaluator and a
//! future generated codec (arriving with `nota-derive` in a later slice). The
//! `GeneratedCodec` trait is the shape the generated side will implement; the
//! `ConformanceHarness` exercises it against the evaluator and asserts agreement on
//! the Core value, the NameTable delta, the canonical output, and the typed-error
//! decision. TODAY the evaluator is the sole implementation — this trait has no
//! generated implementor yet, so the harness is compiled-but-dormant scaffolding.

use name_table::{NameResolver, NameTable, NameTableError};
use raw_discovery::Block;

use crate::error::{DecodeError, EncodeError};
use crate::evaluator::StructuralEvaluator;
use crate::ids::ScopedCoreTypeId;
use crate::table::AddressedStructuralTable;
use crate::value::StructuralValue;
use crate::writer::CanonicalText;

/// The contract a generated codec implements so it can be proven equivalent to the
/// evaluator over the same fixtures.
pub trait GeneratedCodec: Sized {
    const CORE_TYPE: ScopedCoreTypeId;

    fn decode(block: &Block, names: &mut NameTable) -> Result<Self, DecodeError>;
    fn encode(&self, resolver: &dyn NameResolver) -> Result<Block, EncodeError>;
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
pub struct ConformanceHarness<'table> {
    evaluator: StructuralEvaluator<'table>,
    expected: ScopedCoreTypeId,
}

impl<'table> ConformanceHarness<'table> {
    pub fn new(table: &'table AddressedStructuralTable, expected: ScopedCoreTypeId) -> Self {
        Self {
            evaluator: StructuralEvaluator::new(table),
            expected,
        }
    }

    /// Assert the generated codec `T` agrees with the evaluator on every fixture.
    pub fn check<T: GeneratedCodec>(&self, fixtures: &[Block]) -> Result<(), ConformanceError> {
        for block in fixtures {
            let mut names_generated = NameTable::new();
            let generated = T::decode(block, &mut names_generated);

            let mut names_interpreted = NameTable::new();
            let interpreted = self
                .evaluator
                .decode(self.expected, block, &mut names_interpreted);

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
                    let generated_block = typed.encode(&names_generated)?;
                    let interpreted_block =
                        self.evaluator
                            .encode(self.expected, &mirror, &names_interpreted)?;
                    if generated_block.canonical_text() != interpreted_block.canonical_text() {
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
