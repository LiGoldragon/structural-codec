//! Text execution first discovers configured source boundaries, then evaluates
//! the expected typed record against bounded source. It never reconstructs a
//! raw `Block` or `Document` for the textual path.

use std::collections::BTreeMap;

use name_table::{Name, NameInterner, NameResolver, NameTable};
use raw_discovery::{
    Atom, BlockTree, DiscoveredBlock, DiscoveredBlockTree, SourceBound, Trigger, TriggerIdentifier,
};

use crate::error::{DecodeError, EncodeError};
use crate::evaluator::StructuralEvaluator;
use crate::form::{
    BorrowedFieldView, FieldVisitor, LeafCodec, Position, SharedDescriptor, StructureRecord,
};
use crate::ids::{FieldRole, ScopedEncodedTypeId, StableRoleId};
use crate::value::{FieldValue, RoleKeyedMirror, ScalarValue, StructuralValue};

struct SourceDescriptorCollector {
    fields: BTreeMap<StableRoleId, SharedDescriptor>,
}

impl FieldVisitor for SourceDescriptorCollector {
    fn field<Role: FieldRole>(&mut self, position: &Position<Role>) {
        self.fields
            .insert(position.role(), position.descriptor().clone());
    }
}

struct SourceDecodeState {
    fields: RoleKeyedMirror,
    roles: BTreeMap<StableRoleId, SharedDescriptor>,
}

impl SourceDecodeState {
    fn descriptor(&self, role: StableRoleId) -> Result<&SharedDescriptor, DecodeError> {
        self.roles
            .get(&role)
            .ok_or(DecodeError::MissingRole { role })
    }
}

#[derive(Clone, Copy)]
struct SourceInput<'source, 'tree> {
    source: &'source str,
    bound: SourceBound,
    node: Option<&'tree DiscoveredBlock>,
    children: &'tree [DiscoveredBlock],
}

impl<'source, 'tree> SourceInput<'source, 'tree> {
    fn text(self) -> &'source str {
        &self.source[self.bound.start()..self.bound.end()]
    }

    fn for_node(source: &'source str, node: &'tree DiscoveredBlock) -> Self {
        Self {
            source,
            bound: node.source_bound(),
            node: Some(node),
            children: node.children(),
        }
    }
}

impl<Record: StructureRecord> StructuralEvaluator<'_, Record> {
    /// Decode source by completing boundary discovery before any typed form is
    /// evaluated. The expected descriptor only receives source within its
    /// current bound and the already-discovered children of that bound.
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
        let tree =
            DiscoveredBlockTree::discover(source, self.profile, self.table.block_discovery())?;
        let whole = self.trim_bound(source, SourceBound::whole(source))?;
        if whole.is_empty() {
            return Err(DecodeError::RootObjectCount);
        }
        let root = SourceInput {
            source,
            bound: whole,
            node: tree.root_blocks().iter().find(|node| {
                node.source_bound().start() == whole.start()
                    && node.source_bound().end() == whole.end()
            }),
            children: tree.root_blocks(),
        };
        self.decode_source_type(expected, root, interner, &[])
    }

    /// Render directly from the expected typed descriptors and the profile
    /// spellings sealed into the table. The low-level `Block` encoder remains a
    /// separate compatibility API; Textual never uses it.
    pub fn encode_text<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedEncodedTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
    ) -> Result<String, EncodeError> {
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
        let mut collector = SourceDescriptorCollector {
            fields: BTreeMap::new(),
        };
        codec.encode_form().fields().expose(&mut collector);
        self.encode_source_role(
            codec.encode_form().root_role(),
            &collector.fields,
            value.fields(),
            resolver,
        )
    }

    fn decode_source_type(
        &self,
        expected: ScopedEncodedTypeId,
        input: SourceInput<'_, '_>,
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
                let mut state = Self::source_state_for(accepted.rule());
                let root = accepted.rule().root_role();
                match self.decode_source_role(root, input, &mut state, interner, &next_chain) {
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

    fn source_state_for(rule: &Record) -> SourceDecodeState {
        let mut collector = SourceDescriptorCollector {
            fields: BTreeMap::new(),
        };
        rule.fields().expose(&mut collector);
        SourceDecodeState {
            fields: RoleKeyedMirror::default(),
            roles: collector.fields,
        }
    }

    fn decode_source_role(
        &self,
        role: StableRoleId,
        input: SourceInput<'_, '_>,
        state: &mut SourceDecodeState,
        interner: &mut impl NameInterner,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<FieldValue, DecodeError> {
        let descriptor = state.descriptor(role)?.clone();
        let value = self.decode_source_descriptor(&descriptor, input, state, interner, chain)?;
        state.fields.insert(role, value.clone());
        Ok(value)
    }

    fn decode_source_descriptor(
        &self,
        descriptor: &SharedDescriptor,
        input: SourceInput<'_, '_>,
        state: &mut SourceDecodeState,
        interner: &mut impl NameInterner,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<FieldValue, DecodeError> {
        let input = self.trim_input(input)?;
        match descriptor {
            SharedDescriptor::Atom(atom_form) => {
                let atom = self.source_atom(input)?;
                if !atom_form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                Ok(FieldValue::Atom(interner.intern(Name::new(atom.text()))?))
            }
            SharedDescriptor::Literal(identifier) => {
                let atom = self.source_atom(input)?;
                let lexicon = self.lexicon.ok_or(DecodeError::MissingLexicon)?;
                if lexicon.resolve(*identifier)?.as_str() != atom.text() {
                    return Err(DecodeError::LiteralMismatch);
                }
                Ok(FieldValue::Atom(*identifier))
            }
            SharedDescriptor::Leaf(codec) => {
                Ok(FieldValue::Scalar(self.decode_source_leaf(codec, input)?))
            }
            SharedDescriptor::Delegate { target, payload } => {
                if let Some(payload) = payload {
                    let atom = self.source_atom(input).map_err(|_| {
                        DecodeError::DelegationPayloadMismatch { payload: *payload }
                    })?;
                    if !payload.accepts(&atom) {
                        return Err(DecodeError::DelegationPayloadMismatch { payload: *payload });
                    }
                }
                Ok(FieldValue::Delegated(Box::new(
                    self.decode_source_type(*target, input, interner, chain)?,
                )))
            }
            SharedDescriptor::Application {
                operator,
                head,
                payload,
            } => {
                let operator_text = self.operator_text(*operator)?;
                let (head_input, payload_input) =
                    self.source_application_inputs(input, &operator_text)?;
                let head = self.decode_source_role(*head, head_input, state, interner, chain)?;
                let payload =
                    self.decode_source_role(*payload, payload_input, state, interner, chain)?;
                Ok(FieldValue::Application {
                    head: Box::new(head),
                    payload: Box::new(payload),
                })
            }
            SharedDescriptor::Delimited { boundary, content }
            | SharedDescriptor::ItemBoundary { boundary, content } => {
                let node = self.source_boundary_node(input, *boundary)?;
                let content = self.decode_source_children(
                    *content,
                    node,
                    input.source,
                    state,
                    interner,
                    chain,
                )?;
                Ok(FieldValue::Delimited(Box::new(content)))
            }
            SharedDescriptor::Repeated { .. } => Err(DecodeError::BlockKindMismatch {
                expected: "repeated children",
                found: "source",
            }),
        }
    }

    fn decode_source_children(
        &self,
        role: StableRoleId,
        node: &DiscoveredBlock,
        source: &str,
        state: &mut SourceDecodeState,
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
        let items = self.source_items(source, node)?;
        let found = u64::try_from(items.len()).expect("platform usize fits u64");
        if found < minimum || maximum.is_some_and(|top| found > top) {
            return Err(DecodeError::RepetitionCardinality { found });
        }
        let values = items
            .into_iter()
            .map(|item| self.decode_source_descriptor(&element, item, state, interner, chain))
            .collect::<Result<Vec<_>, _>>()?;
        let value = FieldValue::Repeated(values);
        state.fields.insert(role, value.clone());
        Ok(value)
    }

    fn source_items<'source, 'tree>(
        &self,
        source: &'source str,
        node: &'tree DiscoveredBlock,
    ) -> Result<Vec<SourceInput<'source, 'tree>>, DecodeError> {
        let content = node.content_bound();
        let mut cursor = self.skip_trivia(source, content.start(), content.end())?;
        let mut items = Vec::new();
        while cursor < content.end() {
            if let Some(child) = node
                .children()
                .iter()
                .find(|child| child.source_bound().start() == cursor)
            {
                items.push(SourceInput::for_node(source, child));
                cursor = self.skip_trivia(source, child.source_bound().end(), content.end())?;
                continue;
            }
            if let Some(end) = self.source_carrier_end(source, cursor, content.end())? {
                items.push(SourceInput {
                    source,
                    bound: SourceBound::checked(source, cursor, end)?,
                    node: None,
                    children: &[],
                });
                cursor = self.skip_trivia(source, end, content.end())?;
                continue;
            }
            let start = cursor;
            loop {
                if cursor == content.end()
                    || node
                        .children()
                        .iter()
                        .any(|child| child.source_bound().start() == cursor)
                    || self
                        .source_carrier_end(source, cursor, content.end())?
                        .is_some()
                {
                    break;
                }
                let after_trivia = self.skip_trivia(source, cursor, content.end())?;
                if after_trivia > cursor {
                    break;
                }
                cursor = self.next_character(source, cursor, content.end())?;
            }
            if start == cursor {
                return Err(DecodeError::LeafNotFlattenable);
            }
            items.push(SourceInput {
                source,
                bound: SourceBound::checked(source, start, cursor)?,
                node: None,
                children: node.children(),
            });
            cursor = self.skip_trivia(source, cursor, content.end())?;
        }
        Ok(items)
    }

    fn decode_source_leaf(
        &self,
        codec: &LeafCodec,
        input: SourceInput<'_, '_>,
    ) -> Result<ScalarValue, DecodeError> {
        let text = input.text();
        match codec {
            LeafCodec::Integer => {
                text.parse()
                    .map(ScalarValue::Integer)
                    .map_err(|error: std::num::ParseIntError| {
                        DecodeError::ScalarParse(error.to_string())
                    })
            }
            LeafCodec::Float => {
                text.parse()
                    .map(ScalarValue::Float)
                    .map_err(|error: std::num::ParseFloatError| {
                        DecodeError::ScalarParse(error.to_string())
                    })
            }
            LeafCodec::Text => self
                .source_carrier_body(input)
                .map(ScalarValue::Text)
                .or_else(|| self.source_flat_text(input).map(ScalarValue::Text))
                .ok_or(DecodeError::LeafNotFlattenable),
            LeafCodec::Boolean => match text {
                "true" => Ok(ScalarValue::Boolean(true)),
                "false" => Ok(ScalarValue::Boolean(false)),
                other => Err(DecodeError::ScalarParse(format!(
                    "not a boolean keyword: {other}"
                ))),
            },
            LeafCodec::PipeText => self
                .source_carrier_body(input)
                .map(ScalarValue::Text)
                .ok_or(DecodeError::BlockKindMismatch {
                    expected: "pipe text",
                    found: "source",
                }),
            LeafCodec::Foreign(_) => Err(DecodeError::LeafNotFlattenable),
        }
    }

    fn source_boundary_node<'source, 'tree>(
        &self,
        input: SourceInput<'source, 'tree>,
        boundary: TriggerIdentifier,
    ) -> Result<&'tree DiscoveredBlock, DecodeError> {
        let matches = |node: &&DiscoveredBlock| {
            let close = node.closing_bound().expect("delimited tree nodes close");
            node.cue().evidence() == boundary
                && node.cue().bound().start() == input.bound.start()
                && close.end() == input.bound.end()
        };
        input
            .node
            .filter(|node| matches(node))
            .or_else(|| input.children.iter().find(matches))
            .ok_or(DecodeError::BoundaryMismatch { boundary })
    }

    fn source_application_inputs<'source, 'tree>(
        &self,
        input: SourceInput<'source, 'tree>,
        operator: &str,
    ) -> Result<(SourceInput<'source, 'tree>, SourceInput<'source, 'tree>), DecodeError> {
        if let Some(node) = input.node
            && let Some(prefix) = node.prefix()
            && self.source_between(input.source, prefix.separator()) == operator
        {
            let close = node.closing_bound().expect("delimited tree nodes close");
            let head = SourceInput {
                source: input.source,
                bound: prefix.word(),
                node: None,
                children: &[],
            };
            let payload = SourceInput {
                source: input.source,
                bound: SourceBound::checked(input.source, node.cue().bound().start(), close.end())?,
                node: Some(node),
                children: node.children(),
            };
            return Ok((head, payload));
        }

        let at =
            self.source_operator_at(input, operator)?
                .ok_or(DecodeError::BlockKindMismatch {
                    expected: "application",
                    found: "source",
                })?;
        let head = self.trim_bound(
            input.source,
            SourceBound::checked(input.source, input.bound.start(), at)?,
        )?;
        let payload_start = at + operator.len();
        let payload = self.trim_bound(
            input.source,
            SourceBound::checked(input.source, payload_start, input.bound.end())?,
        )?;
        if head.is_empty() || payload.is_empty() {
            return Err(DecodeError::BlockKindMismatch {
                expected: "application",
                found: "source",
            });
        }
        Ok((
            self.source_subinput(input, head),
            self.source_subinput(input, payload),
        ))
    }

    fn source_subinput<'source, 'tree>(
        &self,
        parent: SourceInput<'source, 'tree>,
        bound: SourceBound,
    ) -> SourceInput<'source, 'tree> {
        if let Some(node) = parent.node {
            let close = node.closing_bound().expect("delimited tree nodes close");
            if node.source_bound() == bound
                || (node.cue().bound().start() == bound.start() && close.end() == bound.end())
            {
                return SourceInput {
                    source: parent.source,
                    bound,
                    node: Some(node),
                    children: node.children(),
                };
            }
        }
        if let Some(node) = parent
            .children
            .iter()
            .find(|node| node.source_bound() == bound)
        {
            return SourceInput {
                source: parent.source,
                bound,
                node: Some(node),
                children: node.children(),
            };
        }
        SourceInput {
            source: parent.source,
            bound,
            node: None,
            children: parent.children,
        }
    }

    fn source_operator_at(
        &self,
        input: SourceInput<'_, '_>,
        operator: &str,
    ) -> Result<Option<usize>, DecodeError> {
        let mut cursor = input.bound.start();
        while cursor < input.bound.end() {
            if let Some(node) = input.children.iter().find(|node| {
                node.source_bound().start() == cursor
                    && node.source_bound().end() <= input.bound.end()
            }) {
                cursor = node.source_bound().end();
                continue;
            }
            if let Some(end) = self.source_carrier_end(input.source, cursor, input.bound.end())? {
                cursor = end;
                continue;
            }
            if input.source[cursor..input.bound.end()].starts_with(operator) {
                return Ok(Some(cursor));
            }
            cursor = self.next_character(input.source, cursor, input.bound.end())?;
        }
        Ok(None)
    }

    fn source_atom(&self, input: SourceInput<'_, '_>) -> Result<Atom, DecodeError> {
        if input.children.iter().any(|node| {
            node.source_bound().start() >= input.bound.start()
                && node.source_bound().end() <= input.bound.end()
        }) || self.source_carrier_body(input).is_some()
            || input.text().is_empty()
            || input.text().chars().any(char::is_whitespace)
            || input.text().contains('.')
        {
            return Err(DecodeError::BlockKindMismatch {
                expected: "atom",
                found: "source",
            });
        }
        Ok(Atom::new(input.text()))
    }

    fn source_flat_text(&self, input: SourceInput<'_, '_>) -> Option<String> {
        let text = input.text();
        (!text.is_empty()
            && !text.chars().any(char::is_whitespace)
            && !input.children.iter().any(|node| {
                node.source_bound().start() >= input.bound.start()
                    && node.source_bound().end() <= input.bound.end()
            }))
        .then(|| text.to_owned())
    }

    fn source_carrier_body(&self, input: SourceInput<'_, '_>) -> Option<String> {
        let end = self
            .source_carrier_end(input.source, input.bound.start(), input.bound.end())
            .ok()??;
        (end == input.bound.end()).then(|| {
            self.carrier_body_at(input.source, input.bound.start(), end)
                .expect("carrier end was obtained from its profile spelling")
        })
    }

    fn source_carrier_end(
        &self,
        source: &str,
        start: usize,
        limit: usize,
    ) -> Result<Option<usize>, DecodeError> {
        for identifier in self.table.discovery_trigger_identifiers() {
            let Trigger::Carrier {
                opening,
                closing,
                escape,
            } = &self.profile.definition(identifier)?.trigger
            else {
                continue;
            };
            if !source[start..limit].starts_with(opening) {
                continue;
            }
            let mut cursor = start + opening.len();
            while cursor < limit {
                if let Some(escape) = escape
                    && source[cursor..limit].starts_with(escape)
                {
                    cursor += escape.len();
                    if cursor < limit {
                        cursor = self.next_character(source, cursor, limit)?;
                    }
                    continue;
                }
                if source[cursor..limit].starts_with(closing) {
                    return Ok(Some(cursor + closing.len()));
                }
                cursor = self.next_character(source, cursor, limit)?;
            }
        }
        Ok(None)
    }

    fn carrier_body_at(&self, source: &str, start: usize, end: usize) -> Option<String> {
        self.table
            .discovery_trigger_identifiers()
            .into_iter()
            .find_map(|identifier| {
                let definition = self.profile.definition(identifier).ok()?;
                let Trigger::Carrier {
                    opening, closing, ..
                } = &definition.trigger
                else {
                    return None;
                };
                (source[start..end].starts_with(opening) && source[start..end].ends_with(closing))
                    .then(|| source[start + opening.len()..end - closing.len()].to_owned())
            })
    }

    fn trim_input<'source, 'tree>(
        &self,
        input: SourceInput<'source, 'tree>,
    ) -> Result<SourceInput<'source, 'tree>, DecodeError> {
        Ok(SourceInput {
            bound: self.trim_bound(input.source, input.bound)?,
            ..input
        })
    }

    fn trim_bound(&self, source: &str, bound: SourceBound) -> Result<SourceBound, DecodeError> {
        let start = self.skip_trivia(source, bound.start(), bound.end())?;
        let mut end = bound.end();
        loop {
            let trimmed = self.trim_trailing_whitespace(source, start, end)?;
            let comment = self.trailing_comment_start(source, start, trimmed)?;
            if let Some(comment) = comment {
                end = comment;
                continue;
            }
            return SourceBound::checked(source, start, trimmed).map_err(DecodeError::from);
        }
    }

    fn skip_trivia(
        &self,
        source: &str,
        mut cursor: usize,
        limit: usize,
    ) -> Result<usize, DecodeError> {
        loop {
            if self.has_whitespace_trivia()? {
                while cursor < limit {
                    let character = source[cursor..limit]
                        .chars()
                        .next()
                        .expect("cursor is in range");
                    if !character.is_whitespace() {
                        break;
                    }
                    cursor += character.len_utf8();
                }
            }
            let mut consumed_comment = false;
            for opening in self.comment_openings()? {
                if source[cursor..limit].starts_with(&opening) {
                    cursor += opening.len();
                    while cursor < limit {
                        let character = source[cursor..limit]
                            .chars()
                            .next()
                            .expect("cursor is in range");
                        cursor += character.len_utf8();
                        if character == '\n' {
                            break;
                        }
                    }
                    consumed_comment = true;
                    break;
                }
            }
            if !consumed_comment {
                return Ok(cursor);
            }
        }
    }

    fn trim_trailing_whitespace(
        &self,
        source: &str,
        start: usize,
        mut end: usize,
    ) -> Result<usize, DecodeError> {
        if !self.has_whitespace_trivia()? {
            return Ok(end);
        }
        while end > start {
            let character = source[..end].chars().next_back().expect("non-empty prefix");
            if !character.is_whitespace() {
                break;
            }
            end -= character.len_utf8();
        }
        Ok(end)
    }

    fn trailing_comment_start(
        &self,
        source: &str,
        start: usize,
        end: usize,
    ) -> Result<Option<usize>, DecodeError> {
        let openings = self.comment_openings()?;
        let mut cursor = start;
        let mut last = None;
        while cursor < end {
            if let Some(carrier_end) = self.source_carrier_end(source, cursor, end)? {
                cursor = carrier_end;
                continue;
            }
            if let Some(opening) = openings
                .iter()
                .find(|opening| source[cursor..end].starts_with(*opening))
            {
                last = Some(cursor);
                cursor += opening.len();
                continue;
            }
            cursor = self.next_character(source, cursor, end)?;
        }
        Ok(last.filter(|comment| self.skip_trivia(source, *comment, end).ok() == Some(end)))
    }

    fn has_whitespace_trivia(&self) -> Result<bool, DecodeError> {
        self.table
            .discovery_trigger_identifiers()
            .into_iter()
            .try_fold(false, |found, identifier| {
                Ok(found
                    || matches!(
                        self.profile.definition(identifier)?.trigger,
                        Trigger::Whitespace { .. }
                    ))
            })
    }

    fn comment_openings(&self) -> Result<Vec<String>, DecodeError> {
        let mut openings = Vec::new();
        for identifier in self.table.discovery_trigger_identifiers() {
            if let Trigger::LineComment { opening } = &self.profile.definition(identifier)?.trigger
            {
                openings.push(opening.clone());
            }
        }
        Ok(openings)
    }

    fn next_character(
        &self,
        source: &str,
        cursor: usize,
        limit: usize,
    ) -> Result<usize, DecodeError> {
        let character = source[cursor..limit]
            .chars()
            .next()
            .ok_or(DecodeError::LeafNotFlattenable)?;
        Ok(cursor + character.len_utf8())
    }

    fn source_between<'source>(&self, source: &'source str, bound: SourceBound) -> &'source str {
        &source[bound.start()..bound.end()]
    }

    fn operator_text(&self, identifier: TriggerIdentifier) -> Result<String, DecodeError> {
        match &self.profile.definition(identifier)?.trigger {
            Trigger::Application { glyph } | Trigger::Punctuation { glyph } => Ok(glyph.clone()),
            _ => Err(DecodeError::BlockKindMismatch {
                expected: "application trigger",
                found: "profile trigger",
            }),
        }
    }

    fn encode_source_role<Resolver: NameResolver + ?Sized>(
        &self,
        role: StableRoleId,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
    ) -> Result<String, EncodeError> {
        let descriptor = descriptors
            .get(&role)
            .ok_or(EncodeError::MissingRole { role })?;
        let value = mirror
            .value_by_stable_role(role)
            .ok_or(EncodeError::MissingRole { role })?;
        self.encode_source_descriptor(descriptor, value, descriptors, mirror, resolver)
    }

    fn encode_source_descriptor<Resolver: NameResolver + ?Sized>(
        &self,
        descriptor: &SharedDescriptor,
        value: &FieldValue,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
    ) -> Result<String, EncodeError> {
        match (descriptor, value) {
            (SharedDescriptor::Atom(_), FieldValue::Atom(identifier)) => {
                Ok(resolver.resolve(*identifier)?.as_str().to_owned())
            }
            (SharedDescriptor::Literal(expected), FieldValue::Atom(identifier))
                if expected == identifier =>
            {
                Ok(resolver.resolve(*identifier)?.as_str().to_owned())
            }
            (SharedDescriptor::Literal(_), FieldValue::Atom(_)) => {
                Err(EncodeError::LiteralMismatch)
            }
            (SharedDescriptor::Leaf(codec), FieldValue::Scalar(value)) => {
                self.encode_source_leaf(codec, value)
            }
            (SharedDescriptor::Delegate { target, payload }, FieldValue::Delegated(value)) => {
                let text = self.encode_text(*target, value, resolver)?;
                if let Some(payload) = payload {
                    let atom = Atom::new(&text);
                    if text.contains('.') || !payload.accepts(&atom) {
                        return Err(EncodeError::DelegationPayloadMismatch { payload: *payload });
                    }
                }
                Ok(text)
            }
            (
                SharedDescriptor::Application {
                    operator,
                    head,
                    payload,
                },
                FieldValue::Application { .. },
            ) => {
                let operator = match &self.profile.definition(*operator)?.trigger {
                    Trigger::Application { glyph } | Trigger::Punctuation { glyph } => glyph,
                    _ => return Err(EncodeError::ShapeMismatch),
                };
                Ok(format!(
                    "{}{}{}",
                    self.encode_source_role(*head, descriptors, mirror, resolver)?,
                    operator,
                    self.encode_source_role(*payload, descriptors, mirror, resolver)?,
                ))
            }
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
                Ok(format!(
                    "{}{}{}",
                    opening,
                    self.encode_source_children(*content, descriptors, mirror, resolver)?,
                    closing
                ))
            }
            _ => Err(EncodeError::ShapeMismatch),
        }
    }

    fn encode_source_children<Resolver: NameResolver + ?Sized>(
        &self,
        role: StableRoleId,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
    ) -> Result<String, EncodeError> {
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
            .map(|value| {
                self.encode_source_descriptor(element, value, descriptors, mirror, resolver)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| values.join(" "))
    }

    fn encode_source_leaf(
        &self,
        codec: &LeafCodec,
        value: &ScalarValue,
    ) -> Result<String, EncodeError> {
        match (codec, value) {
            (LeafCodec::Integer, ScalarValue::Integer(value)) => Ok(value.to_string()),
            (LeafCodec::Float, ScalarValue::Float(value)) => Ok(value.to_string()),
            (LeafCodec::Boolean, ScalarValue::Boolean(value)) => Ok(value.to_string()),
            (LeafCodec::Text, ScalarValue::Text(value)) if Self::bare_dotted(value) => {
                Ok(value.clone())
            }
            (LeafCodec::Text | LeafCodec::PipeText, ScalarValue::Text(value)) => {
                self.encode_carrier(value)
            }
            _ => Err(EncodeError::ShapeMismatch),
        }
    }

    fn bare_dotted(value: &str) -> bool {
        !value.is_empty()
            && value
                .split('.')
                .all(|part| Atom::new(part).qualifies_as_symbol())
    }

    fn encode_carrier(&self, body: &str) -> Result<String, EncodeError> {
        for identifier in self.table.discovery_trigger_identifiers() {
            let Trigger::Carrier {
                opening,
                closing,
                escape,
            } = &self.profile.definition(identifier)?.trigger
            else {
                continue;
            };
            let body = escape.as_ref().map_or_else(
                || body.to_owned(),
                |escape| body.replace(closing, &format!("{escape}{closing}")),
            );
            return Ok(format!("{opening}{body}{closing}"));
        }
        Err(EncodeError::ShapeMismatch)
    }
}
