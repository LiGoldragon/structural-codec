//! The one shared, source-bounded evaluator over archived typed records.

use std::collections::{BTreeMap, BTreeSet, HashMap};

#[cfg(test)]
use std::cell::Cell;

use name_table::{Name, NameInterner, NameResolver, NameTable};
use raw_discovery::{
    Atom, BlockTree, BoundaryDiscoveryContextIdentifier, BoundaryReader, DiscoveredBlock,
    DiscoveredBlockTree, SealedTokenProfile, SourceBound, Trigger, TriggerIdentifier,
    TriggerMatchKind,
};

use crate::codec::StructuralEntry;
use crate::error::{DecodeError, EncodeError};
use crate::form::{
    BorrowedFieldView, FieldVisitor, LeafCodec, Position, SharedDescriptor, StructuralRule,
    StructureRecord,
};
use crate::ids::{FieldRole, ScopedEncodedTypeId, StableRoleId};
use crate::table::AddressedStructuralTable;
use crate::value::{FieldValue, RoleKeyedMirror, ScalarValue, StructuralValue};

#[cfg(test)]
thread_local! {
    static CURSOR_CHILD_INDEX_PROBES: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn source_cursor_child_index_probes() -> usize {
    CURSOR_CHILD_INDEX_PROBES.with(Cell::get)
}

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
    fields: BTreeMap<StableRoleId, DraftFieldValue>,
    roles: BTreeMap<StableRoleId, SharedDescriptor>,
}

impl DecodeState {
    fn descriptor(&self, role: StableRoleId) -> Result<&SharedDescriptor, DecodeError> {
        self.roles
            .get(&role)
            .ok_or(DecodeError::MissingRole { role })
    }
}

/// A name reference held before a successful parse has an identifier space.
/// It is intentionally distinct from `name_table::Identifier`: a literal can
/// therefore never be mistaken for a speculative atom while a candidate is
/// still discardable.
#[derive(Clone, Copy)]
struct DraftNameIdentifier(usize);

/// The candidate-local, uninterned name ledger.
///
/// Each accepted form receives a clone of this ledger.  Only a completed
/// candidate replaces its caller's ledger, so failed forms have no names to
/// commit.  The completed root is materialized through the caller's supported
/// `NameInterner` only after its cursor has proven whole-source completion.
#[derive(Clone, Default)]
struct DraftNames {
    names: Vec<Name>,
    identifiers: HashMap<Name, DraftNameIdentifier>,
}

impl DraftNames {
    fn intern(&mut self, name: Name) -> DraftNameIdentifier {
        if let Some(&identifier) = self.identifiers.get(&name) {
            return identifier;
        }
        let identifier = DraftNameIdentifier(self.names.len());
        self.names.push(name.clone());
        self.identifiers.insert(name, identifier);
        identifier
    }

    fn materialize(
        self,
        value: DraftStructuralValue,
        interner: &mut impl NameInterner,
    ) -> Result<StructuralValue, DecodeError> {
        let identifiers = self
            .names
            .into_iter()
            .map(|name| interner.intern(name).map_err(DecodeError::from))
            .collect::<Result<Vec<_>, _>>()?;
        value.materialize(&identifiers)
    }
}

/// The structural mirror before candidate-local names have become durable
/// identifiers.  This keeps speculative names typed and uninterned rather than
/// encoding them in a sentinel `Identifier` range.
#[derive(Clone)]
struct DraftStructuralValue {
    constructor: crate::ids::EncodedConstructorId,
    fields: BTreeMap<StableRoleId, DraftFieldValue>,
}

impl DraftStructuralValue {
    fn materialize(
        self,
        identifiers: &[name_table::Identifier],
    ) -> Result<StructuralValue, DecodeError> {
        let mut fields = RoleKeyedMirror::default();
        for (role, value) in self.fields {
            fields.insert(role, value.materialize(identifiers)?);
        }
        Ok(StructuralValue::new(self.constructor, fields))
    }
}

#[derive(Clone)]
enum DraftAtom {
    Interned(DraftNameIdentifier),
    Literal(name_table::Identifier),
}

#[derive(Clone)]
enum DraftFieldValue {
    Atom(DraftAtom),
    Scalar(ScalarValue),
    Delimited(Box<DraftFieldValue>),
    Application {
        head: Box<DraftFieldValue>,
        payload: Box<DraftFieldValue>,
    },
    Delegated(Box<DraftStructuralValue>),
    Repeated(Vec<DraftFieldValue>),
}

impl DraftFieldValue {
    fn materialize(
        self,
        identifiers: &[name_table::Identifier],
    ) -> Result<FieldValue, DecodeError> {
        Ok(match self {
            Self::Atom(DraftAtom::Interned(DraftNameIdentifier(index))) => FieldValue::Atom(
                *identifiers
                    .get(index)
                    .ok_or(DecodeError::LeafNotFlattenable)?,
            ),
            Self::Atom(DraftAtom::Literal(identifier)) => FieldValue::Atom(identifier),
            Self::Scalar(value) => FieldValue::Scalar(value),
            Self::Delimited(value) => {
                FieldValue::Delimited(Box::new(value.materialize(identifiers)?))
            }
            Self::Application { head, payload } => FieldValue::Application {
                head: Box::new(head.materialize(identifiers)?),
                payload: Box::new(payload.materialize(identifiers)?),
            },
            Self::Delegated(value) => {
                FieldValue::Delegated(Box::new(value.materialize(identifiers)?))
            }
            Self::Repeated(values) => FieldValue::Repeated(
                values
                    .into_iter()
                    .map(|value| value.materialize(identifiers))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        })
    }
}

/// The structural continuation that proves a decoded type form is complete.
///
/// A type may occupy the whole current bound, the head before an enclosing
/// operator, or one element in a repeated interior.  Alternative selection
/// uses this local continuation; it never assigns precedence to table order.
#[derive(Clone, Copy)]
enum DecodeContinuation<'source> {
    Bound,
    Before(&'source str),
    Repeated,
}

impl<'source> DecodeContinuation<'source> {
    fn terminator(self) -> Option<&'source str> {
        match self {
            Self::Bound | Self::Repeated => None,
            Self::Before(text) => Some(text),
        }
    }
}

/// The continuation and local operator cues carried through one descriptor
/// walk.  The cues belong to the expected type's accepted forms, never to a
/// global token scan.
#[derive(Clone, Copy)]
struct DecodeScope<'source, 'stops> {
    continuation: DecodeContinuation<'source>,
    structural_stops: &'stops [String],
}

/// One sequential reader inside a source bound already established by pass one.
/// It holds only its current source byte and the next discovered child index;
/// it never rescans a parent or searches for a closing boundary.
#[derive(Clone)]
struct BoundedCursor<'source, 'tree> {
    source: &'source str,
    bound: SourceBound,
    children: &'tree [DiscoveredBlock],
    child_index: usize,
    position: usize,
    context: BoundaryDiscoveryContextIdentifier,
}

impl<'source, 'tree> BoundedCursor<'source, 'tree> {
    fn root(
        source: &'source str,
        tree: &'tree DiscoveredBlockTree,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Self {
        let bound = SourceBound::whole(source);
        Self {
            source,
            bound,
            children: tree.root_blocks(),
            child_index: 0,
            position: bound.start(),
            context,
        }
    }

    fn active<'table, Record: StructureRecord>(
        &self,
        evaluator: &'table StructuralEvaluator<'table, Record>,
    ) -> Result<&'table raw_discovery::SealedTriggerSet, DecodeError> {
        Ok(evaluator
            .table
            .block_discovery()
            .active_triggers(self.context)
            .map_err(raw_discovery::BlockDiscoveryError::from)?)
    }

    fn local_match<Record: StructureRecord>(
        &self,
        evaluator: &StructuralEvaluator<'_, Record>,
    ) -> Result<Option<raw_discovery::TriggerMatch>, DecodeError> {
        let remaining = SourceBound::checked(self.source, self.position, self.bound.end())?;
        Ok(
            BoundaryReader::within(self.source, evaluator.profile, remaining)?
                .longest_match(self.active(evaluator)?)?,
        )
    }

    fn skip_trivia<Record: StructureRecord>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Record>,
    ) -> Result<(), DecodeError> {
        while let Some(matched) = self.local_match(evaluator)? {
            if !matched.is_trivia() {
                break;
            }
            self.position = matched.end();
        }
        Ok(())
    }

    fn next_child_at_cursor(&self) -> Option<&'tree DiscoveredBlock> {
        #[cfg(test)]
        CURSOR_CHILD_INDEX_PROBES.with(|probes| probes.set(probes.get() + 1));
        self.children.get(self.child_index).filter(|child| {
            child.cue().bound().start() == self.position
                && child.cue().bound().end() <= self.bound.end()
        })
    }

    fn finish<Record: StructureRecord>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Record>,
    ) -> Result<bool, DecodeError> {
        self.skip_trivia(evaluator)?;
        Ok(self.position == self.bound.end() && self.child_index == self.children.len())
    }

    /// Verify that a type form completed at its enclosing structural
    /// continuation.  This is deliberately cursor-local: it neither searches
    /// forward nor assigns an order to accepted forms.
    fn completes<Record: StructureRecord>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Record>,
        continuation: DecodeContinuation<'_>,
    ) -> Result<bool, DecodeError> {
        match continuation {
            DecodeContinuation::Bound => self.finish(evaluator),
            DecodeContinuation::Before(operator) => {
                Ok(self.source[self.position..self.bound.end()].starts_with(operator))
            }
            DecodeContinuation::Repeated => {
                // A nested form may prove that it reaches a repeated-element
                // separator, but it cannot consume that separator.  The
                // repeated loop owns one consumption before choosing its next
                // element; otherwise an inner delegate could hide the cue its
                // enclosing application still needs to complete.
                if self
                    .local_match(evaluator)?
                    .is_some_and(|matched| matched.is_trivia())
                {
                    return Ok(true);
                }
                Ok(
                    (self.position == self.bound.end() && self.child_index == self.children.len())
                        || self.next_child_at_cursor().is_some(),
                )
            }
        }
    }

    fn take_bare<Record: StructureRecord>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Record>,
        terminator: Option<&str>,
        structural_stops: &[String],
    ) -> Result<SourceBound, DecodeError> {
        self.skip_trivia(evaluator)?;
        if self.position == self.bound.end() || self.next_child_at_cursor().is_some() {
            return Err(DecodeError::BlockKindMismatch {
                expected: "bare atom",
                found: "source boundary",
            });
        }
        if self.local_match(evaluator)?.is_some() {
            return Err(DecodeError::BlockKindMismatch {
                expected: "bare atom",
                found: "source trigger",
            });
        }
        let start = self.position;
        while self.position < self.bound.end() {
            if terminator.is_some_and(|text| self.source[self.position..].starts_with(text))
                || structural_stops
                    .iter()
                    .any(|text| self.source[self.position..self.bound.end()].starts_with(text))
                || self.next_child_at_cursor().is_some()
                || self.local_match(evaluator)?.is_some()
            {
                break;
            }
            let character = self.source[self.position..self.bound.end()]
                .chars()
                .next()
                .expect("the bounded cursor is inside source");
            if evaluator.profile.bare_character_is_forbidden(character) {
                return Err(raw_discovery::TokenProfileError::ForbiddenBareCharacter {
                    character,
                    byte_offset: self.position,
                }
                .into());
            }
            self.position += character.len_utf8();
        }
        if start == self.position {
            return Err(DecodeError::LeafNotFlattenable);
        }
        Ok(SourceBound::checked(self.source, start, self.position)?)
    }

    fn take_carrier<Record: StructureRecord>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Record>,
    ) -> Result<Option<String>, DecodeError> {
        self.skip_trivia(evaluator)?;
        let Some(matched) = self.local_match(evaluator)? else {
            return Ok(None);
        };
        if matched.kind() != TriggerMatchKind::Carrier {
            return Ok(None);
        }
        self.position = matched.end();
        Ok(Some(
            matched
                .body()
                .expect("carrier trigger matches carry a body")
                .to_owned(),
        ))
    }

    fn take_boundary<Record: StructureRecord>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Record>,
        boundary: TriggerIdentifier,
    ) -> Result<Self, DecodeError> {
        self.skip_trivia(evaluator)?;
        let child = self
            .next_child_at_cursor()
            .filter(|child| child.cue().evidence() == boundary)
            .ok_or(DecodeError::BoundaryMismatch { boundary })?;
        let context = evaluator
            .table
            .block_discovery()
            .child_context(self.context, boundary)
            .map_err(raw_discovery::BlockDiscoveryError::from)?;
        self.position = child.source_bound().end();
        self.child_index += 1;
        let content = child.content_bound();
        Ok(Self {
            source: self.source,
            bound: content,
            children: child.children(),
            child_index: 0,
            position: content.start(),
            context,
        })
    }

    fn consume_operator<Record: StructureRecord>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Record>,
        identifier: TriggerIdentifier,
    ) -> Result<(), DecodeError> {
        let (Trigger::Application { glyph } | Trigger::Punctuation { glyph }) =
            &evaluator.profile.definition(identifier)?.trigger
        else {
            return Err(DecodeError::BlockKindMismatch {
                expected: "application trigger",
                found: "profile trigger",
            });
        };
        if !self.source[self.position..self.bound.end()].starts_with(glyph) {
            return Err(DecodeError::BlockKindMismatch {
                expected: "application",
                found: "source",
            });
        }
        self.position += glyph.len();
        Ok(())
    }

    fn text(&self, bound: SourceBound) -> &'source str {
        &self.source[bound.start()..bound.end()]
    }
}

/// The trusted textual evaluator. Pass one owns recursive boundary discovery;
/// this evaluator owns the single typed traversal of records, roles, products,
/// sums, delegation, and repetition over the resulting bounded cursor.
pub struct StructuralEvaluator<'table, Record = StructuralRule> {
    pub(crate) table: &'table AddressedStructuralTable<Record>,
    pub(crate) profile: &'table SealedTokenProfile,
    pub(crate) lexicon: Option<&'table NameTable>,
}

impl<'table, Record: StructureRecord> StructuralEvaluator<'table, Record> {
    /// Construct the table-owned evaluator used by `Textual`.
    pub fn new(table: &'table AddressedStructuralTable<Record>) -> Result<Self, DecodeError> {
        Ok(Self {
            table,
            profile: table.token_profile(),
            lexicon: None,
        })
    }

    /// Construct the table-owned evaluator with literal resolution data.
    pub fn with_lexicon(
        table: &'table AddressedStructuralTable<Record>,
        lexicon: &'table NameTable,
    ) -> Result<Self, DecodeError> {
        let mut evaluator = Self::new(table)?;
        evaluator.lexicon = Some(lexicon);
        Ok(evaluator)
    }

    /// Compatibility constructor that verifies a supplied profile is the
    /// table-owned profile. It does not restore a raw-`Block` evaluation path.
    pub fn with_profile(
        table: &'table AddressedStructuralTable<Record>,
        profile: &'table SealedTokenProfile,
    ) -> Result<Self, DecodeError> {
        if table.token_profile_identity() != profile.identity() {
            return Err(DecodeError::TokenProfileIdentityMismatch);
        }
        Self::new(table)
    }

    /// Compatibility constructor with literal data; textual traversal remains
    /// table/profile-owned.
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

    /// Decode only after raw-discovery has constructed the whole block tree.
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
        #[cfg(test)]
        CURSOR_CHILD_INDEX_PROBES.with(|probes| probes.set(0));
        let tree =
            DiscoveredBlockTree::discover(source, self.profile, self.table.block_discovery())?;
        let mut cursor =
            BoundedCursor::root(source, &tree, self.table.block_discovery().root_context());
        cursor.skip_trivia(self)?;
        if cursor.position == cursor.bound.end() {
            return Err(DecodeError::RootObjectCount);
        }
        let mut drafts = DraftNames::default();
        let value = self.decode_type(
            expected,
            &mut cursor,
            &mut drafts,
            &[],
            DecodeContinuation::Bound,
        )?;
        cursor
            .finish(self)?
            .then(|| drafts.materialize(value, interner))
            .transpose()?
            .ok_or(DecodeError::RootObjectCount)
    }

    fn decode_type(
        &self,
        expected: ScopedEncodedTypeId,
        input: &mut BoundedCursor<'_, '_>,
        drafts: &mut DraftNames,
        chain: &[ScopedEncodedTypeId],
        continuation: DecodeContinuation<'_>,
    ) -> Result<DraftStructuralValue, DecodeError> {
        if chain.contains(&expected) {
            return Err(DecodeError::DelegationCycle(expected));
        }
        let entry = self
            .table
            .entry(expected)
            .ok_or(DecodeError::UnknownType(expected))?;
        let structural_stops = self.alternative_operators(entry)?;
        let scope = DecodeScope {
            continuation,
            structural_stops: &structural_stops,
        };
        let mut next_chain = chain.to_vec();
        next_chain.push(expected);
        for codec in entry.constructors() {
            for accepted in codec.decode_forms() {
                let mut candidate = input.clone();
                let mut candidate_drafts = drafts.clone();
                let mut state = Self::state_for(accepted.rule());
                let root = accepted.rule().root_role();
                match self.decode_role(
                    root,
                    &mut candidate,
                    &mut state,
                    &mut candidate_drafts,
                    &next_chain,
                    scope,
                ) {
                    Ok(_) if candidate.completes(self, continuation)? => {
                        *input = candidate;
                        *drafts = candidate_drafts;
                        return Ok(DraftStructuralValue {
                            constructor: codec.constructor(),
                            fields: state.fields,
                        });
                    }
                    Ok(_) => {}
                    Err(error) if error.is_structural_non_match() => {}
                    Err(error) => return Err(error),
                }
            }
        }
        Err(DecodeError::NoAlternative {
            core_type: expected,
        })
    }

    /// The only local lexical cues that can extend a top-level alternative are
    /// its configured application operators.  A set makes their discovery
    /// independent of constructor/form vector order; it is used solely as a
    /// bounded cursor stop, never as a choice priority.
    fn alternative_operators(
        &self,
        entry: &StructuralEntry<Record>,
    ) -> Result<Vec<String>, DecodeError> {
        let mut operators = BTreeSet::new();
        for codec in entry.constructors() {
            for accepted in codec.decode_forms() {
                let state = Self::state_for(accepted.rule());
                let root = accepted.rule().root_role();
                if let SharedDescriptor::Application { operator, .. } = state.descriptor(root)? {
                    operators.insert(self.operator_text(*operator)?);
                }
            }
        }
        Ok(operators.into_iter().collect())
    }

    fn state_for(rule: &Record) -> DecodeState {
        let mut collector = DescriptorCollector {
            fields: BTreeMap::new(),
        };
        rule.fields().expose(&mut collector);
        DecodeState {
            fields: BTreeMap::new(),
            roles: collector.fields,
        }
    }

    fn decode_role(
        &self,
        role: StableRoleId,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState,
        drafts: &mut DraftNames,
        chain: &[ScopedEncodedTypeId],
        scope: DecodeScope<'_, '_>,
    ) -> Result<DraftFieldValue, DecodeError> {
        let descriptor = state.descriptor(role)?.clone();
        let value = self.decode_descriptor(&descriptor, input, state, drafts, chain, scope)?;
        state.fields.insert(role, value.clone());
        Ok(value)
    }

    fn decode_descriptor(
        &self,
        descriptor: &SharedDescriptor,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState,
        drafts: &mut DraftNames,
        chain: &[ScopedEncodedTypeId],
        scope: DecodeScope<'_, '_>,
    ) -> Result<DraftFieldValue, DecodeError> {
        match descriptor {
            SharedDescriptor::Atom(form) => {
                let atom = self.take_source_atom(
                    input,
                    scope.continuation.terminator(),
                    scope.structural_stops,
                )?;
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                Ok(DraftFieldValue::Atom(DraftAtom::Interned(
                    drafts.intern(Name::new(atom.text())),
                )))
            }
            SharedDescriptor::Literal(identifier) => {
                let atom = self.take_source_atom(
                    input,
                    scope.continuation.terminator(),
                    scope.structural_stops,
                )?;
                let lexicon = self.lexicon.ok_or(DecodeError::MissingLexicon)?;
                if lexicon.resolve(*identifier)?.as_str() != atom.text() {
                    return Err(DecodeError::LiteralMismatch);
                }
                Ok(DraftFieldValue::Atom(DraftAtom::Literal(*identifier)))
            }
            SharedDescriptor::Leaf(codec) => Ok(DraftFieldValue::Scalar(self.decode_leaf(
                codec,
                input,
                scope.continuation.terminator(),
                scope.structural_stops,
            )?)),
            SharedDescriptor::Delegate { target, payload } => {
                if let Some(payload) = payload {
                    let mut preview = input.clone();
                    let atom = self
                        .take_source_atom(
                            &mut preview,
                            scope.continuation.terminator(),
                            scope.structural_stops,
                        )
                        .map_err(|_| DecodeError::DelegationPayloadMismatch {
                            payload: *payload,
                        })?;
                    if !payload.accepts(&atom) {
                        return Err(DecodeError::DelegationPayloadMismatch { payload: *payload });
                    }
                }
                Ok(DraftFieldValue::Delegated(Box::new(self.decode_type(
                    *target,
                    input,
                    drafts,
                    chain,
                    scope.continuation,
                )?)))
            }
            SharedDescriptor::Application {
                operator,
                head,
                payload,
            } => {
                let operator_text = self.operator_text(*operator)?;
                let head = self.decode_role(
                    *head,
                    input,
                    state,
                    drafts,
                    chain,
                    DecodeScope {
                        continuation: DecodeContinuation::Before(&operator_text),
                        structural_stops: scope.structural_stops,
                    },
                )?;
                input.consume_operator(self, *operator)?;
                let payload = self.decode_role(*payload, input, state, drafts, chain, scope)?;
                Ok(DraftFieldValue::Application {
                    head: Box::new(head),
                    payload: Box::new(payload),
                })
            }
            SharedDescriptor::Delimited { boundary, content }
            | SharedDescriptor::ItemBoundary { boundary, content } => {
                let mut interior = input.take_boundary(self, *boundary)?;
                let value =
                    self.decode_repeated_role(*content, &mut interior, state, drafts, chain)?;
                if !interior.finish(self)? {
                    return Err(DecodeError::RepetitionCardinality { found: 0 });
                }
                Ok(DraftFieldValue::Delimited(Box::new(value)))
            }
            SharedDescriptor::Repeated { .. } => Err(DecodeError::BlockKindMismatch {
                expected: "repeated children",
                found: "source",
            }),
        }
    }

    fn decode_repeated_role(
        &self,
        role: StableRoleId,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState,
        drafts: &mut DraftNames,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<DraftFieldValue, DecodeError> {
        let descriptor = state.descriptor(role)?.clone();
        let SharedDescriptor::Repeated {
            minimum,
            maximum,
            element,
        } = descriptor
        else {
            return Err(DecodeError::MissingRole { role });
        };
        let mut values = Vec::new();
        // This loop is the sole owner of separator trivia between repeated
        // elements.  `DecodeContinuation::Repeated` only observes the cue,
        // allowing every nested descriptor to complete against it without
        // stealing it from an enclosing descriptor.
        while !input.finish(self)? {
            let before = input.position;
            values.push(self.decode_descriptor(
                &element,
                input,
                state,
                drafts,
                chain,
                DecodeScope {
                    continuation: DecodeContinuation::Repeated,
                    structural_stops: &[],
                },
            )?);
            if input.position == before {
                return Err(DecodeError::LeafNotFlattenable);
            }
        }
        let found = u64::try_from(values.len()).expect("platform usize fits u64");
        if found < minimum || maximum.is_some_and(|top| found > top) {
            return Err(DecodeError::RepetitionCardinality { found });
        }
        let value = DraftFieldValue::Repeated(values);
        state.fields.insert(role, value.clone());
        Ok(value)
    }

    fn take_source_atom(
        &self,
        input: &mut BoundedCursor<'_, '_>,
        terminator: Option<&str>,
        structural_stops: &[String],
    ) -> Result<Atom, DecodeError> {
        let bound = input.take_bare(self, terminator, structural_stops)?;
        let text = input.text(bound);
        if text.contains('.') {
            return Err(DecodeError::BlockKindMismatch {
                expected: "atom",
                found: "dotted source",
            });
        }
        Ok(Atom::new(text))
    }

    fn decode_leaf(
        &self,
        codec: &LeafCodec,
        input: &mut BoundedCursor<'_, '_>,
        terminator: Option<&str>,
        structural_stops: &[String],
    ) -> Result<ScalarValue, DecodeError> {
        let mut carrier = input.clone();
        if let Some(body) = carrier.take_carrier(self)? {
            *input = carrier;
            return match codec {
                LeafCodec::Text | LeafCodec::PipeText => Ok(ScalarValue::Text(body)),
                _ => Err(DecodeError::LeafNotFlattenable),
            };
        }
        if matches!(codec, LeafCodec::PipeText) {
            return Err(DecodeError::BlockKindMismatch {
                expected: "pipe text",
                found: "source",
            });
        }
        let bound = input.take_bare(self, terminator, structural_stops)?;
        let text = input.text(bound);
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
            LeafCodec::Text => Ok(ScalarValue::Text(text.to_owned())),
            LeafCodec::Boolean => match text {
                "true" => Ok(ScalarValue::Boolean(true)),
                "false" => Ok(ScalarValue::Boolean(false)),
                other => Err(DecodeError::ScalarParse(format!(
                    "not a boolean keyword: {other}"
                ))),
            },
            LeafCodec::PipeText => unreachable!("pipe text returned before bare leaf parsing"),
            LeafCodec::Foreign(_) => Err(DecodeError::LeafNotFlattenable),
        }
    }

    /// Render directly from descriptor data under explicit table-owned context
    /// policy. No raw `Block` renderer participates in textual output.
    pub fn encode_text<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedEncodedTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
    ) -> Result<String, EncodeError> {
        self.encode_type_at_context(
            expected,
            value,
            resolver,
            self.table.block_discovery().root_context(),
        )
    }

    /// The internal encoder carries the current recursive discovery context.
    /// Only the public entry point selects root; recursive descriptor arms
    /// retain or transition this context explicitly.
    fn encode_type_at_context<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedEncodedTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
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
        let mut collector = DescriptorCollector {
            fields: BTreeMap::new(),
        };
        codec.encode_form().fields().expose(&mut collector);
        self.encode_role(
            codec.encode_form().root_role(),
            &collector.fields,
            value.fields(),
            resolver,
            context,
        )
    }

    fn encode_role<Resolver: NameResolver + ?Sized>(
        &self,
        role: StableRoleId,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError> {
        let descriptor = descriptors
            .get(&role)
            .ok_or(EncodeError::MissingRole { role })?;
        let value = mirror
            .value_by_stable_role(role)
            .ok_or(EncodeError::MissingRole { role })?;
        self.encode_descriptor(descriptor, value, descriptors, mirror, resolver, context)
    }

    fn encode_descriptor<Resolver: NameResolver + ?Sized>(
        &self,
        descriptor: &SharedDescriptor,
        value: &FieldValue,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
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
                self.encode_leaf(codec, value, context)
            }
            (SharedDescriptor::Delegate { target, payload }, FieldValue::Delegated(value)) => {
                let text = self.encode_type_at_context(*target, value, resolver, context)?;
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
            ) => Ok(format!(
                "{}{}{}",
                self.encode_role(*head, descriptors, mirror, resolver, context)?,
                self.operator_text_encode(*operator)?,
                self.encode_role(*payload, descriptors, mirror, resolver, context)?,
            )),
            (
                SharedDescriptor::Delimited { boundary, content }
                | SharedDescriptor::ItemBoundary { boundary, content },
                FieldValue::Delimited(_),
            ) => {
                let active = self
                    .table
                    .block_discovery()
                    .active_triggers(context)
                    .map_err(|_| EncodeError::NonCanonicalSpelling)?;
                if !active.triggers().contains(boundary) {
                    return Err(EncodeError::NonCanonicalSpelling);
                }
                let child_context = self
                    .table
                    .block_discovery()
                    .child_context(context, *boundary)
                    .map_err(|_| EncodeError::NonCanonicalSpelling)?;
                let Trigger::Boundary { opening, closing } =
                    &self.profile.definition(*boundary)?.trigger
                else {
                    return Err(EncodeError::ShapeMismatch);
                };
                Ok(format!(
                    "{}{}{}",
                    opening,
                    self.encode_repeated_role(
                        *content,
                        descriptors,
                        mirror,
                        resolver,
                        child_context
                    )?,
                    closing,
                ))
            }
            _ => Err(EncodeError::ShapeMismatch),
        }
    }

    fn encode_repeated_role<Resolver: NameResolver + ?Sized>(
        &self,
        role: StableRoleId,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor>,
        mirror: &RoleKeyedMirror,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
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
        let rendered = values
            .iter()
            .map(|value| {
                self.encode_descriptor(element, value, descriptors, mirror, resolver, context)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if rendered.len() < 2 {
            return Ok(rendered.concat());
        }
        let policy = self
            .table
            .textual_rendering()
            .for_context(context)
            .ok_or(EncodeError::NonCanonicalSpelling)?;
        let separator = policy
            .separator()
            .ok_or(EncodeError::NonCanonicalSpelling)?;
        let Trigger::Whitespace { canonical_spelling } =
            &self.profile.definition(separator)?.trigger
        else {
            return Err(EncodeError::NonCanonicalSpelling);
        };
        Ok(rendered.join(canonical_spelling))
    }

    fn encode_leaf(
        &self,
        codec: &LeafCodec,
        value: &ScalarValue,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError> {
        match (codec, value) {
            (LeafCodec::Integer, ScalarValue::Integer(value)) => Ok(value.to_string()),
            (LeafCodec::Float, ScalarValue::Float(value)) => Ok(value.to_string()),
            (LeafCodec::Boolean, ScalarValue::Boolean(value)) => Ok(value.to_string()),
            (LeafCodec::Text, ScalarValue::Text(value)) if Self::bare_dotted(value) => {
                Ok(value.clone())
            }
            (LeafCodec::Text | LeafCodec::PipeText, ScalarValue::Text(value)) => {
                self.encode_carrier(value, context)
            }
            _ => Err(EncodeError::ShapeMismatch),
        }
    }

    fn encode_carrier(
        &self,
        body: &str,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError> {
        let policy = self
            .table
            .textual_rendering()
            .for_context(context)
            .ok_or(EncodeError::NonCanonicalSpelling)?;
        let carrier = policy.carrier().ok_or(EncodeError::NonCanonicalSpelling)?;
        let Trigger::Carrier {
            opening,
            closing,
            escape,
        } = &self.profile.definition(carrier)?.trigger
        else {
            return Err(EncodeError::NonCanonicalSpelling);
        };
        let body = escape.as_ref().map_or_else(
            || body.to_owned(),
            |escape| body.replace(closing, &format!("{escape}{closing}")),
        );
        Ok(format!("{opening}{body}{closing}"))
    }

    fn bare_dotted(value: &str) -> bool {
        !value.is_empty()
            && value
                .split('.')
                .all(|part| Atom::new(part).qualifies_as_symbol())
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

    fn operator_text_encode(&self, identifier: TriggerIdentifier) -> Result<&str, EncodeError> {
        match &self.profile.definition(identifier)?.trigger {
            Trigger::Application { glyph } | Trigger::Punctuation { glyph } => Ok(glyph),
            _ => Err(EncodeError::ShapeMismatch),
        }
    }
}
