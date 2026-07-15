//! The one trusted evaluator over the generic `StructuralValue` mirror. It SHIPS IN
//! THE RUNTIME: a data-loaded table is executed directly, both directions, off the
//! SAME forms — so round-trip coherence holds by construction (§4.6, Fork C settled).
//!
//! Decoding matches PURELY (no interning) so alternatives backtrack for free, then
//! interns the winning path through a speculative `NameTransaction`. A failed decode
//! never opens past that transaction's rollback, so the NameTable is left byte-for-byte
//! unchanged (interning atomicity, law 3). Delegation constructs every wrapper level
//! and rejects transparent cycles; recursion is permitted only after structure is
//! consumed (left-recursion guard).

use name_table::{Name, NameInterner, NameResolver, NameTable, NameTransaction};
use raw_discovery::{Atom, Block};

use crate::codec::StructuralEntry;
use crate::error::{DecodeError, EncodeError};
use crate::form::{CarrierLeaf, LeafCodec, ScalarLeaf, SequenceForm, StructuralForm};
use crate::ids::ScopedCoreTypeId;
use crate::table::AddressedStructuralTable;
use crate::value::{ScalarValue, StructuralValue};

/// A decoded shape before interning: atoms carry their raw text so a failed
/// alternative costs no NameTable effect. `resolve` interns the winning path.
enum DecodeDraft {
    Atom(String),
    Scalar(ScalarValue),
    Delimited(Vec<DecodeDraft>),
    Application(Box<DecodeDraft>, Box<DecodeDraft>),
    Delegated(Box<DecodeDraft>),
    Chosen {
        constructor: u32,
        payload: Box<DecodeDraft>,
    },
}

impl DecodeDraft {
    fn resolve(self, interner: &mut impl NameInterner) -> StructuralValue {
        match self {
            Self::Atom(text) => StructuralValue::Atom(interner.intern(Name::new(text))),
            Self::Scalar(scalar) => StructuralValue::Scalar(scalar),
            Self::Delimited(children) => StructuralValue::Delimited(
                children
                    .into_iter()
                    .map(|child| child.resolve(interner))
                    .collect(),
            ),
            Self::Application(head, payload) => StructuralValue::Application(
                Box::new(head.resolve(interner)),
                Box::new(payload.resolve(interner)),
            ),
            Self::Delegated(inner) => StructuralValue::Delegated(Box::new(inner.resolve(interner))),
            Self::Chosen {
                constructor,
                payload,
            } => StructuralValue::Chosen {
                constructor,
                payload: Box::new(payload.resolve(interner)),
            },
        }
    }
}

/// The trusted interpreter of a table, both directions.
pub struct StructuralEvaluator<'table> {
    table: &'table AddressedStructuralTable,
    /// Optional resolver for the table's literal keywords (needed only to decode
    /// `Literal` forms; encoding always has the caller's resolver).
    lexicon: Option<&'table dyn NameResolver>,
}

impl<'table> StructuralEvaluator<'table> {
    pub fn new(table: &'table AddressedStructuralTable) -> Self {
        Self {
            table,
            lexicon: None,
        }
    }

    /// An evaluator that can also decode `Literal` forms against the table's lexicon.
    pub fn with_lexicon(
        table: &'table AddressedStructuralTable,
        lexicon: &'table dyn NameResolver,
    ) -> Self {
        Self {
            table,
            lexicon: Some(lexicon),
        }
    }

    // ===== decode =====

    /// Text tree + expected type → structural value. The expected type limits the
    /// lookup; the input never selects its own type. Interning is atomic: on failure
    /// the NameTable is unchanged.
    pub fn decode(
        &self,
        expected: ScopedCoreTypeId,
        block: &Block,
        names: &mut NameTable,
    ) -> Result<StructuralValue, DecodeError> {
        names.try_intern(|transaction: &mut NameTransaction<'_>| {
            let draft = self.match_type(expected, block, &[])?;
            Ok(draft.resolve(transaction))
        })
    }

    /// Decode `block` under `expected`, trying each constructor's disjoint decode
    /// forms. `chain` carries the transparent-delegation path for cycle detection.
    fn match_type(
        &self,
        expected: ScopedCoreTypeId,
        block: &Block,
        chain: &[ScopedCoreTypeId],
    ) -> Result<DecodeDraft, DecodeError> {
        if chain.contains(&expected) {
            return Err(DecodeError::DelegationCycle(expected));
        }
        let entry = self
            .table
            .entry(expected)
            .ok_or(DecodeError::UnknownType(expected))?;
        let mut child_chain = chain.to_vec();
        child_chain.push(expected);

        for (index, codec) in entry.constructors.iter().enumerate() {
            for form in &codec.decode_forms {
                if let Ok(draft) = self.match_form(form, block, &child_chain) {
                    return Ok(DecodeDraft::Chosen {
                        constructor: index as u32,
                        payload: Box::new(draft),
                    });
                }
            }
        }
        Err(DecodeError::NoAlternative {
            core_type: expected,
        })
    }

    fn match_form(
        &self,
        form: &StructuralForm,
        block: &Block,
        chain: &[ScopedCoreTypeId],
    ) -> Result<DecodeDraft, DecodeError> {
        match form {
            StructuralForm::Atom(atom_form) => {
                let atom = block.atom().ok_or(DecodeError::BlockKindMismatch {
                    expected: "atom",
                    found: Self::block_kind(block),
                })?;
                if atom_form.accepts(atom) {
                    Ok(DecodeDraft::Atom(atom.text().to_owned()))
                } else {
                    Err(DecodeError::CaseMismatch)
                }
            }

            StructuralForm::Literal(identifier) => {
                let atom = block.atom().ok_or(DecodeError::BlockKindMismatch {
                    expected: "atom",
                    found: Self::block_kind(block),
                })?;
                let lexicon = self.lexicon.ok_or(DecodeError::LiteralMismatch)?;
                let name = lexicon.resolve(*identifier)?;
                if name.as_str() == atom.text() {
                    Ok(DecodeDraft::Atom(atom.text().to_owned()))
                } else {
                    Err(DecodeError::LiteralMismatch)
                }
            }

            StructuralForm::Leaf(leaf) => self.match_leaf(&leaf.codec, block),

            StructuralForm::Application { head, payload } => {
                let (block_head, block_payload) =
                    block
                        .as_application()
                        .ok_or(DecodeError::BlockKindMismatch {
                            expected: "application",
                            found: Self::block_kind(block),
                        })?;
                Ok(DecodeDraft::Application(
                    Box::new(self.match_form(head, block_head, &[])?),
                    Box::new(self.match_form(payload, block_payload, &[])?),
                ))
            }

            StructuralForm::Delimited {
                delimiter,
                sequence,
            } => {
                let children =
                    block
                        .as_delimited(*delimiter)
                        .ok_or(DecodeError::BlockKindMismatch {
                            expected: delimiter.description(),
                            found: Self::block_kind(block),
                        })?;
                Ok(DecodeDraft::Delimited(
                    self.match_sequence(sequence, children)?,
                ))
            }

            StructuralForm::Product(forms) => {
                if forms.len() == 1 {
                    self.match_form(&forms[0], block, chain)
                } else {
                    Err(DecodeError::ProductArity {
                        form: forms.len(),
                        blocks: 1,
                    })
                }
            }

            StructuralForm::Delegate(target) => Ok(DecodeDraft::Delegated(Box::new(
                self.match_type(*target, block, chain)?,
            ))),
        }
    }

    fn match_sequence(
        &self,
        sequence: &SequenceForm,
        blocks: &[Block],
    ) -> Result<Vec<DecodeDraft>, DecodeError> {
        match sequence {
            SequenceForm::Product(forms) => {
                if forms.len() != blocks.len() {
                    return Err(DecodeError::ProductArity {
                        form: forms.len(),
                        blocks: blocks.len(),
                    });
                }
                forms
                    .iter()
                    .zip(blocks)
                    .map(|(form, block)| self.match_form(form, block, &[]))
                    .collect()
            }
            SequenceForm::Repeat {
                minimum,
                maximum,
                element,
            } => {
                let count = blocks.len() as u64;
                if count < *minimum || maximum.is_some_and(|top| count > top) {
                    return Err(DecodeError::SequenceCardinality { found: count });
                }
                blocks
                    .iter()
                    .map(|block| self.match_form(element, block, &[]))
                    .collect()
            }
        }
    }

    fn match_leaf(&self, codec: &LeafCodec, block: &Block) -> Result<DecodeDraft, DecodeError> {
        match codec {
            LeafCodec::Scalar(scalar) => {
                let text = block.dotted_text().ok_or(DecodeError::LeafNotFlattenable)?;
                let value = match scalar {
                    ScalarLeaf::Integer => ScalarValue::Integer(text.parse().map_err(
                        |error: std::num::ParseIntError| {
                            DecodeError::ScalarParse(error.to_string())
                        },
                    )?),
                    ScalarLeaf::Float => ScalarValue::Float(text.parse().map_err(
                        |error: std::num::ParseFloatError| {
                            DecodeError::ScalarParse(error.to_string())
                        },
                    )?),
                    ScalarLeaf::Text => ScalarValue::Text(text),
                    ScalarLeaf::Boolean => match text.as_str() {
                        "true" => ScalarValue::Boolean(true),
                        "false" => ScalarValue::Boolean(false),
                        other => {
                            return Err(DecodeError::ScalarParse(format!(
                                "not a boolean keyword: {other}"
                            )));
                        }
                    },
                };
                Ok(DecodeDraft::Scalar(value))
            }
            LeafCodec::Carrier(CarrierLeaf::PipeText) => match block {
                Block::PipeText(pipe) => Ok(DecodeDraft::Scalar(ScalarValue::Text(
                    pipe.text().to_owned(),
                ))),
                other => Err(DecodeError::BlockKindMismatch {
                    expected: "pipe text",
                    found: Self::block_kind(other),
                }),
            },
            LeafCodec::Foreign(_) => Err(DecodeError::LeafNotFlattenable),
        }
    }

    fn block_kind(block: &Block) -> &'static str {
        match block {
            Block::Atom(_) => "atom",
            Block::Application { .. } => "application",
            Block::Delimited { .. } => "delimited",
            Block::PipeText(_) => "pipe text",
        }
    }

    // ===== encode =====

    /// Structural value → text tree. The value's chosen constructor selects the ONE
    /// canonical encode form, so encoding never echoes a non-canonical decode form.
    pub fn encode<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedCoreTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
    ) -> Result<Block, EncodeError> {
        self.encode_type(expected, value, resolver)
    }

    fn encode_type<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedCoreTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
    ) -> Result<Block, EncodeError> {
        let entry: &StructuralEntry = self
            .table
            .entry(expected)
            .ok_or(EncodeError::UnknownType(expected))?;
        let (constructor, payload) = match value {
            StructuralValue::Chosen {
                constructor,
                payload,
            } => (*constructor, payload.as_ref()),
            _ => {
                return Err(EncodeError::ShapeMismatch(
                    "expected a constructor-tagged value at a type boundary",
                ));
            }
        };
        let codec = entry.constructor_at(constructor as usize).ok_or(
            EncodeError::ConstructorOutOfRange {
                chosen: constructor,
                available: entry.constructors.len(),
            },
        )?;
        self.encode_form_walk(&codec.encode_form, payload, resolver)
    }

    fn encode_form_walk<Resolver: NameResolver + ?Sized>(
        &self,
        form: &StructuralForm,
        value: &StructuralValue,
        resolver: &Resolver,
    ) -> Result<Block, EncodeError> {
        match (form, value) {
            (StructuralForm::Atom(_) | StructuralForm::Literal(_), StructuralValue::Atom(id)) => {
                let name = resolver.resolve(*id)?;
                Ok(Block::Atom(Atom::new(name.as_str())))
            }
            (StructuralForm::Leaf(_), StructuralValue::Scalar(scalar)) => Ok(scalar.render_block()),
            (
                StructuralForm::Application { head, payload },
                StructuralValue::Application(value_head, value_payload),
            ) => Ok(Block::Application {
                head: Box::new(self.encode_form_walk(head, value_head, resolver)?),
                payload: Box::new(self.encode_form_walk(payload, value_payload, resolver)?),
            }),
            (
                StructuralForm::Delimited {
                    delimiter,
                    sequence,
                },
                StructuralValue::Delimited(children),
            ) => Ok(Block::Delimited {
                delimiter: *delimiter,
                root_objects: self.encode_sequence(sequence, children, resolver)?,
            }),
            (StructuralForm::Delegate(target), StructuralValue::Delegated(inner)) => {
                self.encode_type(*target, inner, resolver)
            }
            (StructuralForm::Product(forms), value) if forms.len() == 1 => {
                self.encode_form_walk(&forms[0], value, resolver)
            }
            _ => Err(EncodeError::ShapeMismatch(
                "value did not fit the canonical encode form",
            )),
        }
    }

    fn encode_sequence<Resolver: NameResolver + ?Sized>(
        &self,
        sequence: &SequenceForm,
        children: &[StructuralValue],
        resolver: &Resolver,
    ) -> Result<Vec<Block>, EncodeError> {
        match sequence {
            SequenceForm::Product(forms) => {
                if forms.len() != children.len() {
                    return Err(EncodeError::ShapeMismatch(
                        "product arity did not match the value's children",
                    ));
                }
                forms
                    .iter()
                    .zip(children)
                    .map(|(form, child)| self.encode_form_walk(form, child, resolver))
                    .collect()
            }
            SequenceForm::Repeat { element, .. } => children
                .iter()
                .map(|child| self.encode_form_walk(element, child, resolver))
                .collect(),
        }
    }
}
