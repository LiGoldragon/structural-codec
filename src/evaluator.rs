//! The one shared evaluator over archived typed records.

use std::collections::BTreeMap;

use name_table::{Name, NameInterner, NameResolver, NameTable};
use raw_discovery::{Atom, Block, Delimiter, SealedTokenProfile, Trigger};

use crate::error::{DecodeError, EncodeError};
use crate::form::{
    BorrowedFieldView, FieldVisitor, LeafCodec, Position, SharedDescriptor, StructuralRule,
    StructureRecord,
};
use crate::ids::{FieldRole, ScopedEncodedTypeId, StableRoleId};
use crate::table::AddressedStructuralTable;
use crate::value::{FieldValue, RoleKeyedMirror, ScalarValue, StructuralValue};

struct DescriptorCollector {
    fields: BTreeMap<StableRoleId, SharedDescriptor>,
}

impl FieldVisitor for DescriptorCollector {
    fn field<Role: FieldRole>(&mut self, position: &Position<Role>) {
        self.fields
            .insert(position.role(), position.descriptor().clone());
    }
}

struct DecodeState {
    fields: RoleKeyedMirror,
    roles: BTreeMap<StableRoleId, SharedDescriptor>,
}

impl DecodeState {
    fn descriptor(&self, role: StableRoleId) -> Result<&SharedDescriptor, DecodeError> {
        self.roles
            .get(&role)
            .ok_or(DecodeError::MissingRole { role })
    }
}

/// The trusted evaluator. A sealed profile is mandatory even for direct `Block`
/// execution, so a boundary is validated from its authoritative trigger rather
/// than guessed from raw-discovery's compatibility delimiter enum.
pub struct StructuralEvaluator<'table, Record = StructuralRule> {
    pub(crate) table: &'table AddressedStructuralTable<Record>,
    pub(crate) profile: &'table SealedTokenProfile,
    pub(crate) lexicon: Option<&'table NameTable>,
}

impl<'table, Record: StructureRecord> StructuralEvaluator<'table, Record> {
    /// Construct a text-capable evaluator from the table's sealed profile and
    /// sealed discovery rules. This is the constructor used by `Textual`.
    pub fn new(table: &'table AddressedStructuralTable<Record>) -> Result<Self, DecodeError> {
        Ok(Self {
            table,
            profile: table.token_profile(),
            lexicon: None,
        })
    }

    /// Construct a table-owned evaluator with literal resolution data.
    pub fn with_lexicon(
        table: &'table AddressedStructuralTable<Record>,
        lexicon: &'table NameTable,
    ) -> Result<Self, DecodeError> {
        let mut evaluator = Self::new(table)?;
        evaluator.lexicon = Some(lexicon);
        Ok(evaluator)
    }

    /// Compatibility constructor for low-level `Block` evaluation. Textual
    /// execution still uses the profile owned by `table`.
    pub fn with_profile(
        table: &'table AddressedStructuralTable<Record>,
        profile: &'table SealedTokenProfile,
    ) -> Result<Self, DecodeError> {
        if table.token_profile_identity() != profile.identity() {
            return Err(DecodeError::TokenProfileIdentityMismatch);
        }
        Self::new(table)
    }

    pub fn with_profile_and_lexicon(
        table: &'table AddressedStructuralTable<Record>,
        profile: &'table SealedTokenProfile,
        lexicon: &'table NameTable,
    ) -> Result<Self, DecodeError> {
        if table.token_profile_identity() != profile.identity() {
            return Err(DecodeError::TokenProfileIdentityMismatch);
        }
        Self::with_lexicon(table, lexicon)
    }

    pub fn decode(
        &self,
        expected: ScopedEncodedTypeId,
        block: &Block,
        names: &mut NameTable,
    ) -> Result<StructuralValue, DecodeError> {
        names.try_intern(|transaction| self.decode_with_interner(expected, block, transaction))
    }

    pub fn decode_with_interner(
        &self,
        expected: ScopedEncodedTypeId,
        block: &Block,
        interner: &mut impl NameInterner,
    ) -> Result<StructuralValue, DecodeError> {
        self.decode_type(expected, block, interner, &[])
    }

    fn decode_type(
        &self,
        expected: ScopedEncodedTypeId,
        block: &Block,
        interner: &mut impl NameInterner,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<StructuralValue, DecodeError> {
        if chain.contains(&expected) {
            return Err(DecodeError::DelegationCycle(expected));
        }
        let entry = self
            .table
            .entry(expected)
            .ok_or(DecodeError::UnknownType(expected))?;
        let mut next_chain = chain.to_vec();
        next_chain.push(expected);
        for codec in entry.constructors() {
            for accepted in codec.decode_forms() {
                let mut state = Self::state_for(accepted.rule());
                let root = accepted.rule().root_role();
                match self.decode_role(root, block, &mut state, interner, &next_chain) {
                    Ok(_) => return Ok(StructuralValue::new(codec.constructor(), state.fields)),
                    Err(error) if error.is_structural_non_match() => {}
                    Err(error) => return Err(error),
                }
            }
        }
        Err(DecodeError::NoAlternative {
            core_type: expected,
        })
    }

    fn state_for(rule: &Record) -> DecodeState {
        let mut collector = DescriptorCollector {
            fields: BTreeMap::new(),
        };
        rule.fields().expose(&mut collector);
        DecodeState {
            fields: RoleKeyedMirror::default(),
            roles: collector.fields,
        }
    }

    fn decode_role(
        &self,
        role: StableRoleId,
        block: &Block,
        state: &mut DecodeState,
        interner: &mut impl NameInterner,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<FieldValue, DecodeError> {
        let descriptor = state.descriptor(role)?.clone();
        let value = self.decode_descriptor(&descriptor, block, state, interner, chain)?;
        state.fields.insert(role, value.clone());
        Ok(value)
    }

    fn decode_descriptor(
        &self,
        descriptor: &SharedDescriptor,
        block: &Block,
        state: &mut DecodeState,
        interner: &mut impl NameInterner,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<FieldValue, DecodeError> {
        match descriptor {
            SharedDescriptor::Atom(atom_form) => {
                let atom = block.atom().ok_or(DecodeError::BlockKindMismatch {
                    expected: "atom",
                    found: Self::block_kind(block),
                })?;
                if !atom_form.accepts(atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                Ok(FieldValue::Atom(interner.intern(Name::new(atom.text()))?))
            }
            SharedDescriptor::Literal(identifier) => {
                let atom = block.atom().ok_or(DecodeError::BlockKindMismatch {
                    expected: "atom",
                    found: Self::block_kind(block),
                })?;
                let lexicon = self.lexicon.ok_or(DecodeError::MissingLexicon)?;
                if lexicon.resolve(*identifier)?.as_str() != atom.text() {
                    return Err(DecodeError::LiteralMismatch);
                }
                Ok(FieldValue::Atom(*identifier))
            }
            SharedDescriptor::Leaf(codec) => {
                Ok(FieldValue::Scalar(self.decode_leaf(codec, block)?))
            }
            SharedDescriptor::Delegate { target, payload } => {
                if let Some(payload) = payload {
                    let atom = block
                        .atom()
                        .ok_or(DecodeError::DelegationPayloadMismatch { payload: *payload })?;
                    if !payload.accepts(atom) {
                        return Err(DecodeError::DelegationPayloadMismatch { payload: *payload });
                    }
                }
                Ok(FieldValue::Delegated(Box::new(
                    self.decode_type(*target, block, interner, chain)?,
                )))
            }
            SharedDescriptor::Application { head, payload, .. } => {
                let (head_block, payload_block) =
                    block
                        .as_application()
                        .ok_or(DecodeError::BlockKindMismatch {
                            expected: "application",
                            found: Self::block_kind(block),
                        })?;
                let head = self.decode_role(*head, head_block, state, interner, chain)?;
                let payload = self.decode_role(*payload, payload_block, state, interner, chain)?;
                Ok(FieldValue::Application {
                    head: Box::new(head),
                    payload: Box::new(payload),
                })
            }
            SharedDescriptor::Delimited { boundary, content }
            | SharedDescriptor::ItemBoundary { boundary, content } => {
                let children = self.boundary_children(*boundary, block)?;
                let content =
                    self.decode_children_role(*content, children, state, interner, chain)?;
                Ok(FieldValue::Delimited(Box::new(content)))
            }
            SharedDescriptor::Repeated { .. } => Err(DecodeError::BlockKindMismatch {
                expected: "repeated children",
                found: Self::block_kind(block),
            }),
        }
    }

    fn decode_children_role(
        &self,
        role: StableRoleId,
        children: &[Block],
        state: &mut DecodeState,
        interner: &mut impl NameInterner,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<FieldValue, DecodeError> {
        let descriptor = state.descriptor(role)?.clone();
        let SharedDescriptor::Repeated {
            minimum,
            maximum,
            element,
        } = descriptor
        else {
            return Err(DecodeError::MissingRole { role });
        };
        let found = u64::try_from(children.len()).expect("platform usize fits u64");
        if found < minimum || maximum.is_some_and(|top| found > top) {
            return Err(DecodeError::RepetitionCardinality { found });
        }
        let values = children
            .iter()
            .map(|child| self.decode_descriptor(&element, child, state, interner, chain))
            .collect::<Result<Vec<_>, _>>()?;
        let value = FieldValue::Repeated(values);
        state.fields.insert(role, value.clone());
        Ok(value)
    }

    fn decode_leaf(&self, codec: &LeafCodec, block: &Block) -> Result<ScalarValue, DecodeError> {
        match codec {
            LeafCodec::Integer => block
                .dotted_text()
                .ok_or(DecodeError::LeafNotFlattenable)?
                .parse()
                .map(ScalarValue::Integer)
                .map_err(|error: std::num::ParseIntError| {
                    DecodeError::ScalarParse(error.to_string())
                }),
            LeafCodec::Float => block
                .dotted_text()
                .ok_or(DecodeError::LeafNotFlattenable)?
                .parse()
                .map(ScalarValue::Float)
                .map_err(|error: std::num::ParseFloatError| {
                    DecodeError::ScalarParse(error.to_string())
                }),
            LeafCodec::Text => match block {
                Block::PipeText(text) => Ok(ScalarValue::Text(text.text().to_owned())),
                _ => block
                    .dotted_text()
                    .map(ScalarValue::Text)
                    .ok_or(DecodeError::LeafNotFlattenable),
            },
            LeafCodec::Boolean => match block.dotted_text().as_deref() {
                Some("true") => Ok(ScalarValue::Boolean(true)),
                Some("false") => Ok(ScalarValue::Boolean(false)),
                Some(other) => Err(DecodeError::ScalarParse(format!(
                    "not a boolean keyword: {other}"
                ))),
                None => Err(DecodeError::LeafNotFlattenable),
            },
            LeafCodec::PipeText => match block {
                Block::PipeText(text) => Ok(ScalarValue::Text(text.text().to_owned())),
                _ => Err(DecodeError::BlockKindMismatch {
                    expected: "pipe text",
                    found: Self::block_kind(block),
                }),
            },
            LeafCodec::Foreign(_) => Err(DecodeError::LeafNotFlattenable),
        }
    }

    fn boundary_children<'block>(
        &self,
        boundary: raw_discovery::TriggerIdentifier,
        block: &'block Block,
    ) -> Result<&'block [Block], DecodeError> {
        let definition = self.profile.definition(boundary)?;
        let Trigger::Boundary { opening, closing } = &definition.trigger else {
            return Err(DecodeError::BoundaryMismatch { boundary });
        };
        let Block::Delimited {
            delimiter,
            root_objects,
        } = block
        else {
            return Err(DecodeError::BlockKindMismatch {
                expected: "profile boundary",
                found: Self::block_kind(block),
            });
        };
        if delimiter.opening_text() != opening || delimiter.closing_text() != closing {
            return Err(DecodeError::BoundaryMismatch { boundary });
        }
        Ok(root_objects)
    }

    pub fn encode<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedEncodedTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
    ) -> Result<Block, EncodeError> {
        let entry = self
            .table
            .entry(expected)
            .ok_or(EncodeError::UnknownType(expected))?;
        let codec = entry
            .constructors()
            .iter()
            .find(|codec| codec.constructor() == value.constructor())
            .ok_or(EncodeError::UnknownConstructor {
                chosen: value.constructor(),
            })?;
        let mut collector = DescriptorCollector {
            fields: BTreeMap::new(),
        };
        codec.encode_form().fields().expose(&mut collector);
        self.encode_role(
            codec.encode_form().root_role(),
            &collector.fields,
            value.fields(),
            resolver,
        )
    }

    fn encode_role<Resolver: NameResolver + ?Sized>(
        &self,
        role: StableRoleId,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
    ) -> Result<Block, EncodeError> {
        let descriptor = descriptors
            .get(&role)
            .ok_or(EncodeError::MissingRole { role })?;
        let value = mirror
            .value_by_stable_role(role)
            .ok_or(EncodeError::MissingRole { role })?;
        self.encode_descriptor(descriptor, value, descriptors, mirror, resolver)
    }

    fn encode_descriptor<Resolver: NameResolver + ?Sized>(
        &self,
        descriptor: &SharedDescriptor,
        value: &FieldValue,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
    ) -> Result<Block, EncodeError> {
        match (descriptor, value) {
            (SharedDescriptor::Atom(_), FieldValue::Atom(identifier)) => Ok(Block::Atom(
                Atom::new(resolver.resolve(*identifier)?.as_str()),
            )),
            (SharedDescriptor::Literal(expected), FieldValue::Atom(identifier))
                if expected == identifier =>
            {
                Ok(Block::Atom(Atom::new(
                    resolver.resolve(*identifier)?.as_str(),
                )))
            }
            (SharedDescriptor::Literal(_), FieldValue::Atom(_)) => {
                Err(EncodeError::LiteralMismatch)
            }
            (SharedDescriptor::Leaf(_), FieldValue::Scalar(value)) => Ok(value.render_block()),
            (SharedDescriptor::Delegate { target, payload }, FieldValue::Delegated(value)) => {
                let block = self.encode(*target, value, resolver)?;
                if let Some(payload) = payload {
                    let atom = block
                        .atom()
                        .ok_or(EncodeError::DelegationPayloadMismatch { payload: *payload })?;
                    if !payload.accepts(atom) {
                        return Err(EncodeError::DelegationPayloadMismatch { payload: *payload });
                    }
                }
                Ok(block)
            }
            (
                SharedDescriptor::Application { head, payload, .. },
                FieldValue::Application { .. },
            ) => Ok(Block::Application {
                head: Box::new(self.encode_role(*head, descriptors, mirror, resolver)?),
                payload: Box::new(self.encode_role(*payload, descriptors, mirror, resolver)?),
            }),
            (
                SharedDescriptor::Delimited { boundary, content }
                | SharedDescriptor::ItemBoundary { boundary, content },
                FieldValue::Delimited(_),
            ) => {
                let Trigger::Boundary { opening, closing } =
                    &self.profile.definition(*boundary)?.trigger
                else {
                    return Err(EncodeError::ShapeMismatch);
                };
                let delimiter =
                    Self::delimiter_for(opening, closing).ok_or(EncodeError::ShapeMismatch)?;
                Ok(Block::Delimited {
                    delimiter,
                    root_objects: self.encode_children_role(
                        *content,
                        descriptors,
                        mirror,
                        resolver,
                    )?,
                })
            }
            _ => Err(EncodeError::ShapeMismatch),
        }
    }

    fn encode_children_role<Resolver: NameResolver + ?Sized>(
        &self,
        role: StableRoleId,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
    ) -> Result<Vec<Block>, EncodeError> {
        let SharedDescriptor::Repeated { element, .. } = descriptors
            .get(&role)
            .ok_or(EncodeError::MissingRole { role })?
        else {
            return Err(EncodeError::ShapeMismatch);
        };
        let FieldValue::Repeated(values) = mirror
            .value_by_stable_role(role)
            .ok_or(EncodeError::MissingRole { role })?
        else {
            return Err(EncodeError::ShapeMismatch);
        };
        values
            .iter()
            .map(|value| self.encode_descriptor(element, value, descriptors, mirror, resolver))
            .collect()
    }

    fn delimiter_for(opening: &str, closing: &str) -> Option<Delimiter> {
        [
            Delimiter::Parenthesis,
            Delimiter::SquareBracket,
            Delimiter::Brace,
        ]
        .into_iter()
        .find(|delimiter| {
            delimiter.opening_text() == opening && delimiter.closing_text() == closing
        })
    }

    fn block_kind(block: &Block) -> &'static str {
        match block {
            Block::Atom(_) => "atom",
            Block::Application { .. } => "application",
            Block::Delimited { .. } => "delimited",
            Block::PipeText(_) => "pipe text",
        }
    }
}

#[cfg(test)]
mod tests {
    use name_table::{IdentifierNamespace, Name};

    use super::*;

    #[test]
    fn literal_without_lexicon_is_not_collapsed_into_alternative_failure() {
        let table = crate::fixture::FixtureBuilder::new()
            .build()
            .expect("fixture table seals");
        let profile = crate::fixture::FixtureBuilder::token_profile();
        let evaluator = StructuralEvaluator::with_profile(&table, &profile)
            .expect("the profile is pinned to the table");
        let mut literal_names = NameTable::new(IdentifierNamespace::Fixture);
        let literal = literal_names
            .intern(Name::new("reserved"))
            .expect("literal allocation");
        let mut decode_names = NameTable::new(IdentifierNamespace::Fixture);
        let mut state = DecodeState {
            fields: RoleKeyedMirror::default(),
            roles: BTreeMap::new(),
        };

        assert!(matches!(
            evaluator.decode_descriptor(
                &SharedDescriptor::Literal(literal),
                &Block::Atom(Atom::new("reserved")),
                &mut state,
                &mut decode_names,
                &[],
            ),
            Err(DecodeError::MissingLexicon)
        ));
    }
}
