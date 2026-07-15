//! Law 5 scaffolding, exercised. There is no `nota-derive`-generated codec yet, so
//! the trusted evaluator is the SOLE current implementation of `GeneratedCodec`. This
//! test wires the evaluator behind the trait and runs the harness green, proving the
//! contract shape compiles and drives end-to-end. When a real generated codec arrives
//! in a later slice, it implements the same trait and this harness gains teeth.

use std::sync::OnceLock;

use name_table::{NameResolver, NameTable};
use raw_discovery::{Block, Recognizer};
use structural_codec::fixture::{COMMIT_SEQUENCE, FixtureBuilder};
use structural_codec::{
    AddressedStructuralTable, ConformanceHarness, DecodeError, EncodeError, GeneratedCodec,
    ScopedCoreTypeId, StructuralEvaluator, StructuralValue,
};

fn fixture_table() -> &'static AddressedStructuralTable {
    static TABLE: OnceLock<AddressedStructuralTable> = OnceLock::new();
    TABLE.get_or_init(|| FixtureBuilder::new().build().expect("seal fixture table"))
}

/// Stand-in "generated" codec whose behaviour is, for now, the evaluator itself.
struct EvaluatorBackedCodec(StructuralValue);

impl GeneratedCodec for EvaluatorBackedCodec {
    const CORE_TYPE: ScopedCoreTypeId = COMMIT_SEQUENCE;

    fn decode(block: &Block, names: &mut NameTable) -> Result<Self, DecodeError> {
        let evaluator = StructuralEvaluator::new(fixture_table());
        Ok(Self(evaluator.decode(COMMIT_SEQUENCE, block, names)?))
    }

    fn encode(&self, resolver: &dyn NameResolver) -> Result<Block, EncodeError> {
        let evaluator = StructuralEvaluator::new(fixture_table());
        evaluator.encode(COMMIT_SEQUENCE, &self.0, resolver)
    }

    fn to_structural(&self) -> StructuralValue {
        self.0.clone()
    }
}

#[test]
fn harness_runs_green_with_the_evaluator_as_sole_implementation() {
    let harness = ConformanceHarness::new(fixture_table(), COMMIT_SEQUENCE);
    let block = Recognizer::standard()
        .recognize("CommitSequence.{ Integer }")
        .expect("recognize")
        .root_object_at(0)
        .expect("root")
        .clone();
    harness
        .check::<EvaluatorBackedCodec>(&[block])
        .expect("interpreter and (evaluator-backed) codec agree");
}
