//! Text entry points delegate raw boundary-first discovery to the sealed profile,
//! then execute the same typed-record evaluator used for direct `Block` values.

use name_table::{NameInterner, NameResolver, NameTable};
use raw_discovery::{Block, Recognizer};

use crate::error::{DecodeError, EncodeError};
use crate::evaluator::StructuralEvaluator;
use crate::form::StructureRecord;
use crate::ids::ScopedEncodedTypeId;
use crate::value::StructuralValue;

impl<Record: StructureRecord> StructuralEvaluator<'_, Record> {
    pub fn decode_text(
        &self,
        expected: ScopedEncodedTypeId,
        source: &str,
        names: &mut NameTable,
    ) -> Result<StructuralValue, DecodeError> {
        names
            .try_intern(|transaction| self.decode_text_with_interner(expected, source, transaction))
    }

    pub fn decode_text_with_interner(
        &self,
        expected: ScopedEncodedTypeId,
        source: &str,
        interner: &mut impl NameInterner,
    ) -> Result<StructuralValue, DecodeError> {
        let document = Recognizer::with_profile(self.profile.clone())
            .recognize(source)
            .map_err(DecodeError::from)?;
        if document.holds_root_objects() != 1 {
            return Err(DecodeError::RootObjectCount);
        }
        self.decode_with_interner(
            expected,
            document.root_object_at(0).expect("one root checked"),
            interner,
        )
    }

    pub fn encode_text<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedEncodedTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
    ) -> Result<String, EncodeError> {
        Ok(Self::render_block(&self.encode(expected, value, resolver)?))
    }

    fn render_block(block: &Block) -> String {
        match block {
            Block::Atom(atom) => atom.text().to_owned(),
            Block::PipeText(text) => format!("(|{}|)", text.text()),
            Block::Application { head, payload } => format!(
                "{}.{}",
                Self::render_block(head),
                Self::render_block(payload)
            ),
            Block::Delimited {
                delimiter,
                root_objects,
            } => delimiter.wrap(root_objects.iter().map(Self::render_block)),
        }
    }
}
