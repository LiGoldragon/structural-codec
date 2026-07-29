//! The one shared, source-bounded evaluator over archived typed records.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

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
use crate::ids::{EncodedTypeId, FieldRole, StableRoleId};
use crate::names::{
    DeclarationAssignment, DecodeNameBindings, EncodedNameResolver, NameOccurrence,
    ResolvedReference,
};
use crate::table::AddressedStructuralTable;
use crate::value::{
    FieldValue, RoleKeyedMirror, ScalarValue, SourceBoundedStructuralValue, StructuralValue,
};

struct DescriptorCollector<Root> {
    fields: BTreeMap<StableRoleId, SharedDescriptor<Root>>,
}

impl<Root: Clone> FieldVisitor<Root> for DescriptorCollector<Root> {
    fn field<Role: FieldRole>(&mut self, position: &Position<Role, Root>) {
        self.fields
            .insert(position.role(), position.descriptor().clone());
    }
}

#[derive(Clone)]
struct DecodeState<Root> {
    fields: BTreeMap<StableRoleId, DraftFieldValue<Root>>,
    field_bounds: BTreeMap<StableRoleId, SourceBound>,
    roles: BTreeMap<StableRoleId, SharedDescriptor<Root>>,
}

impl<Root> DecodeState<Root> {
    fn descriptor(&self, role: StableRoleId) -> Result<&SharedDescriptor<Root>, DecodeError<Root>> {
        self.roles
            .get(&role)
            .ok_or(DecodeError::MissingRole { role })
    }
}

#[derive(Clone)]
struct DraftStructuralValue<Root> {
    constructor: crate::ids::EncodedConstructorId<Root>,
    fields: BTreeMap<StableRoleId, DraftFieldValue<Root>>,
    field_bounds: BTreeMap<StableRoleId, SourceBound>,
}

impl<Root: Clone> DraftStructuralValue<Root> {
    fn materialize_value(self) -> StructuralValue<Root> {
        let mut fields = RoleKeyedMirror::default();
        for (role, value) in self.fields {
            fields.insert(role, value.materialize());
        }
        StructuralValue::new(self.constructor, fields)
    }

    fn materialize_bounded(self) -> SourceBoundedStructuralValue<Root> {
        let field_bounds = self.field_bounds.clone();
        SourceBoundedStructuralValue::new(self.materialize_value(), field_bounds)
    }
}

#[derive(Clone)]
enum DraftFieldValue<Root> {
    Declaration(DeclarationAssignment<Root>),
    Reference(ResolvedReference<Root>),
    Literal(name_table::EncodedId<Root>),
    Scalar(ScalarValue),
    OrderedProduct,
    Delimited(Rc<DraftFieldValue<Root>>),
    Application {
        head: Rc<DraftFieldValue<Root>>,
        payload: Rc<DraftFieldValue<Root>>,
    },
    Delegated(Rc<DraftStructuralValue<Root>>),
    Repeated(Vec<DraftFieldValue<Root>>),
}

impl<Root: Clone> DraftFieldValue<Root> {
    fn materialize(self) -> FieldValue<Root> {
        match self {
            Self::Declaration(assignment) => FieldValue::Declaration(assignment),
            Self::Reference(reference) => FieldValue::Reference(reference),
            Self::Literal(identifier) => FieldValue::Literal(identifier),
            Self::Scalar(value) => FieldValue::Scalar(value),
            Self::OrderedProduct => FieldValue::OrderedProduct,
            Self::Delimited(value) => {
                FieldValue::Delimited(Box::new(value.as_ref().clone().materialize()))
            }
            Self::Application { head, payload } => FieldValue::Application {
                head: Box::new(head.as_ref().clone().materialize()),
                payload: Box::new(payload.as_ref().clone().materialize()),
            },
            Self::Delegated(value) => {
                FieldValue::Delegated(Box::new(value.as_ref().clone().materialize_value()))
            }
            Self::Repeated(values) => FieldValue::Repeated(
                values
                    .into_iter()
                    .map(DraftFieldValue::materialize)
                    .collect(),
            ),
        }
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

/// The complete local state that a recursive type evaluation must not revisit.
///
/// Type identity alone is too coarse: an application may consume its head and
/// operator before its payload delegates back to the same type.  The source
/// position distinguishes that productive recursion from a transparent
/// delegate cycle.  The bound, child cursor, context, and continuation are
/// retained because each contributes to the meaning of completion at that
/// position.
#[derive(Clone, Eq, PartialEq)]
struct DecodeProgressKey<Root> {
    expected: EncodedTypeId<Root>,
    bound_start: usize,
    bound_end: usize,
    child_index: usize,
    position: usize,
    context: BoundaryDiscoveryContextIdentifier,
    continuation: DecodeContinuationKey,
}

#[derive(Clone, Eq, PartialEq)]
enum DecodeContinuationKey {
    Bound,
    Before(String),
    Repeated,
}

impl<Root: Clone> DecodeProgressKey<Root> {
    fn at(
        expected: &EncodedTypeId<Root>,
        input: &BoundedCursor<'_, '_>,
        continuation: DecodeContinuation<'_>,
    ) -> Self {
        let continuation = match continuation {
            DecodeContinuation::Bound => DecodeContinuationKey::Bound,
            DecodeContinuation::Before(operator) => {
                DecodeContinuationKey::Before(operator.to_owned())
            }
            DecodeContinuation::Repeated => DecodeContinuationKey::Repeated,
        };
        Self {
            expected: expected.clone(),
            bound_start: input.bound.start(),
            bound_end: input.bound.end(),
            child_index: input.child_index,
            position: input.position,
            context: input.context,
            continuation,
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

    fn active<'table, Root, Record: StructureRecord<Root>>(
        &self,
        evaluator: &'table StructuralEvaluator<'table, Root, Record>,
    ) -> Result<&'table raw_discovery::SealedTriggerSet, DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
        Ok(evaluator
            .table
            .block_discovery()
            .active_triggers(self.context)
            .map_err(raw_discovery::BlockDiscoveryError::from)?)
    }

    fn local_match<Root, Record: StructureRecord<Root>>(
        &self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
    ) -> Result<Option<raw_discovery::TriggerMatch>, DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
        let remaining = SourceBound::checked(self.source, self.position, self.bound.end())?;
        Ok(
            BoundaryReader::within(self.source, evaluator.profile, remaining)?
                .longest_match(self.active(evaluator)?)?,
        )
    }

    fn skip_trivia<Root, Record: StructureRecord<Root>>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
    ) -> Result<(), DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
        while let Some(matched) = self.local_match(evaluator)? {
            if !matched.is_trivia() {
                break;
            }
            self.position = matched.end();
        }
        Ok(())
    }

    fn next_child_at_cursor(&self) -> Option<&'tree DiscoveredBlock> {
        self.children.get(self.child_index).filter(|child| {
            child.cue().bound().start() == self.position
                && child.cue().bound().end() <= self.bound.end()
        })
    }

    fn finish<Root, Record: StructureRecord<Root>>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
    ) -> Result<bool, DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
        self.skip_trivia(evaluator)?;
        Ok(self.position == self.bound.end() && self.child_index == self.children.len())
    }

    /// Verify that a type form completed at its enclosing structural
    /// continuation.  This is deliberately cursor-local: it neither searches
    /// forward nor assigns an order to accepted forms.
    fn completes<Root, Record: StructureRecord<Root>>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
        continuation: DecodeContinuation<'_>,
        structural_stops: &[String],
    ) -> Result<bool, DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
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
                    let mut after_trivia = self.clone();
                    after_trivia.skip_trivia(evaluator)?;
                    if structural_stops.iter().any(|text| {
                        after_trivia.source[after_trivia.position..after_trivia.bound.end()]
                            .starts_with(text)
                    }) {
                        return Ok(false);
                    }
                    return Ok(true);
                }
                Ok(
                    (self.position == self.bound.end() && self.child_index == self.children.len())
                        || self.next_child_at_cursor().is_some(),
                )
            }
        }
    }

    fn take_bare<Root, Record: StructureRecord<Root>>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
        terminator: Option<&str>,
        structural_stops: &[String],
    ) -> Result<SourceBound, DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
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

    fn take_carrier<Root, Record: StructureRecord<Root>>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
    ) -> Result<Option<String>, DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
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

    fn take_boundary<Root, Record: StructureRecord<Root>>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
        boundary: TriggerIdentifier,
    ) -> Result<Self, DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
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

    fn consume_operator<Root, Record: StructureRecord<Root>>(
        &mut self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
        identifier: TriggerIdentifier,
    ) -> Result<(), DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
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
pub struct StructuralEvaluator<'table, Root, Record = StructuralRule<Root>> {
    pub(crate) table: &'table AddressedStructuralTable<Root, Record>,
    pub(crate) profile: &'table SealedTokenProfile,
}

impl<'table, Root, Record> StructuralEvaluator<'table, Root, Record>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    /// Construct the table-owned evaluator used by `Textual`.
    pub fn new(
        table: &'table AddressedStructuralTable<Root, Record>,
    ) -> Result<Self, DecodeError<Root>> {
        Ok(Self {
            table,
            profile: table.token_profile(),
        })
    }

    /// Compatibility constructor that verifies a supplied profile is the
    /// table-owned profile. It does not restore a raw-`Block` evaluation path.
    pub fn with_profile(
        table: &'table AddressedStructuralTable<Root, Record>,
        profile: &'table SealedTokenProfile,
    ) -> Result<Self, DecodeError<Root>> {
        if table.token_profile_identity() != profile.identity() {
            return Err(DecodeError::TokenProfileIdentityMismatch);
        }
        Self::new(table)
    }

    /// Decode only after raw-discovery has constructed the whole block tree.
    pub fn decode_text<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        source: &str,
        bindings: &Bindings,
    ) -> Result<StructuralValue<Root>, DecodeError<Root>> {
        self.decode_text_to_draft(expected, source, bindings)
            .map(DraftStructuralValue::materialize_value)
    }

    /// Decode with the exact full-source bound consumed by every typed field
    /// in the selected top-level record.
    pub fn decode_text_bounded<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        source: &str,
        bindings: &Bindings,
    ) -> Result<SourceBoundedStructuralValue<Root>, DecodeError<Root>> {
        self.decode_text_to_draft(expected, source, bindings)
            .map(DraftStructuralValue::materialize_bounded)
    }

    fn decode_text_to_draft<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        source: &str,
        bindings: &Bindings,
    ) -> Result<DraftStructuralValue<Root>, DecodeError<Root>> {
        let tree =
            DiscoveredBlockTree::discover(source, self.profile, self.table.block_discovery())?;
        let mut cursor =
            BoundedCursor::root(source, &tree, self.table.block_discovery().root_context());
        cursor.skip_trivia(self)?;
        if cursor.position == cursor.bound.end() {
            return Err(DecodeError::RootObjectCount);
        }
        let value = self.decode_type(
            expected,
            &mut cursor,
            bindings,
            &[],
            DecodeContinuation::Bound,
        )?;
        if !cursor.finish(self)? {
            return Err(DecodeError::RootObjectCount);
        }
        Ok(value)
    }

    fn decode_type<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        input: &mut BoundedCursor<'_, '_>,
        bindings: &Bindings,
        active: &[DecodeProgressKey<Root>],
        continuation: DecodeContinuation<'_>,
    ) -> Result<DraftStructuralValue<Root>, DecodeError<Root>> {
        let progress = DecodeProgressKey::at(expected, input, continuation);
        if active.contains(&progress) {
            return Err(DecodeError::DelegationCycle(expected.clone()));
        }
        let entry = self
            .table
            .entry(expected)
            .ok_or_else(|| DecodeError::UnknownType(expected.clone()))?;
        let single_decode_form = entry
            .constructors()
            .iter()
            .map(|codec| codec.decode_forms().len())
            .sum::<usize>()
            == 1;
        let structural_stops = self.alternative_operators(entry)?;
        let scope = DecodeScope {
            continuation,
            structural_stops: &structural_stops,
        };
        let mut next_active = active.to_vec();
        next_active.push(progress);
        for codec in entry.constructors() {
            for accepted in codec.decode_forms() {
                let mut candidate = input.clone();
                let mut state = Self::state_for(accepted.rule());
                let root = accepted.rule().root_role();
                match self.decode_role(
                    root,
                    &mut candidate,
                    &mut state,
                    bindings,
                    &next_active,
                    scope,
                ) {
                    Ok(_) if candidate.completes(self, continuation, scope.structural_stops)? => {
                        *input = candidate;
                        return Ok(DraftStructuralValue {
                            constructor: codec.constructor().clone(),
                            fields: state.fields,
                            field_bounds: state.field_bounds,
                        });
                    }
                    Ok(_) => {}
                    Err(
                        error @ (DecodeError::ProductArityMismatch { .. }
                        | DecodeError::ProductPositionMismatch { .. }),
                    ) if single_decode_form => return Err(error),
                    Err(error) if error.is_structural_non_match() => {}
                    Err(error) => return Err(error),
                }
            }
        }
        Err(DecodeError::NoAlternative {
            core_type: expected.clone(),
        })
    }

    /// The only local lexical cues that can extend a top-level alternative are
    /// its configured application operators.  A set makes their discovery
    /// independent of constructor/form vector order; it is used solely as a
    /// bounded cursor stop, never as a choice priority.
    fn alternative_operators(
        &self,
        entry: &StructuralEntry<Root, Record>,
    ) -> Result<Vec<String>, DecodeError<Root>> {
        let mut operators = BTreeSet::new();
        for codec in entry.constructors() {
            for accepted in codec.decode_forms() {
                let state = Self::state_for(accepted.rule());
                let root = accepted.rule().root_role();
                if let SharedDescriptor::Application { operator, .. } = state.descriptor(root)? {
                    operators.insert(self.operator_text(*operator)?);
                }
                if let SharedDescriptor::OrderedSequence(sequence) = state.descriptor(root)? {
                    let Some(second) = sequence.members().get(1) else {
                        continue;
                    };
                    if let SharedDescriptor::Delimited { boundary, .. }
                    | SharedDescriptor::ItemBoundary { boundary, .. } =
                        state.descriptor(*second)?
                    {
                        let Trigger::Boundary { opening, .. } =
                            &self.profile.definition(*boundary)?.trigger
                        else {
                            return Err(DecodeError::BlockKindMismatch {
                                expected: "boundary trigger",
                                found: "profile trigger",
                            });
                        };
                        operators.insert(opening.clone());
                    }
                }
            }
        }
        Ok(operators.into_iter().collect())
    }

    fn state_for(rule: &Record) -> DecodeState<Root> {
        let mut collector = DescriptorCollector {
            fields: BTreeMap::new(),
        };
        rule.fields().expose(&mut collector);
        DecodeState {
            fields: BTreeMap::new(),
            field_bounds: BTreeMap::new(),
            roles: collector.fields,
        }
    }

    fn decode_role<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        role: StableRoleId,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        bindings: &Bindings,
        active: &[DecodeProgressKey<Root>],
        scope: DecodeScope<'_, '_>,
    ) -> Result<DraftFieldValue<Root>, DecodeError<Root>> {
        input.skip_trivia(self)?;
        let start = input.position;
        let descriptor = state.descriptor(role)?.clone();
        let value = self.decode_descriptor(&descriptor, input, state, bindings, active, scope)?;
        let bound = SourceBound::checked(input.source, start, input.position)?;
        state.fields.insert(role, value.clone());
        state.field_bounds.insert(role, bound);
        Ok(value)
    }

    fn decode_descriptor<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        descriptor: &SharedDescriptor<Root>,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        bindings: &Bindings,
        active: &[DecodeProgressKey<Root>],
        scope: DecodeScope<'_, '_>,
    ) -> Result<DraftFieldValue<Root>, DecodeError<Root>> {
        match descriptor {
            SharedDescriptor::Declaration(form) => {
                let (atom, bound) = self.take_source_atom(
                    input,
                    scope.continuation.terminator(),
                    scope.structural_stops,
                )?;
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                let assignment = bindings
                    .declaration_assignment(NameOccurrence::new(atom.text(), bound))
                    .ok_or(DecodeError::MissingDeclarationAssignment { bound })?;
                self.verify_bound_spelling(bindings, assignment.encoded_id(), atom.text(), bound)?;
                Ok(DraftFieldValue::Declaration(assignment))
            }
            SharedDescriptor::Reference(form) => {
                let (atom, bound) = self.take_source_atom(
                    input,
                    scope.continuation.terminator(),
                    scope.structural_stops,
                )?;
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                let reference = bindings
                    .reference_resolution(NameOccurrence::new(atom.text(), bound))
                    .ok_or(DecodeError::UnresolvedReference { bound })?;
                self.verify_bound_spelling(bindings, reference.encoded_id(), atom.text(), bound)?;
                Ok(DraftFieldValue::Reference(reference))
            }
            SharedDescriptor::Literal(identifier) => {
                let (atom, _) = self.take_source_atom(
                    input,
                    scope.continuation.terminator(),
                    scope.structural_stops,
                )?;
                let spelling = bindings.resolve(identifier).ok_or_else(|| {
                    DecodeError::UnknownEncodedName {
                        encoded_id: identifier.clone(),
                    }
                })?;
                if spelling.as_str() != atom.text() {
                    return Err(DecodeError::LiteralMismatch);
                }
                Ok(DraftFieldValue::Literal(identifier.clone()))
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
                    let (atom, _) = self
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
                Ok(DraftFieldValue::Delegated(Rc::new(self.decode_type(
                    target,
                    input,
                    bindings,
                    active,
                    scope.continuation,
                )?)))
            }
            SharedDescriptor::OrderedProduct(product) => {
                self.decode_ordered_product(product, input, state, bindings, active)?;
                Ok(DraftFieldValue::OrderedProduct)
            }
            SharedDescriptor::OrderedSequence(sequence) => {
                self.decode_ordered_sequence(sequence, input, state, bindings, active, scope)?;
                Ok(DraftFieldValue::OrderedProduct)
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
                    bindings,
                    active,
                    DecodeScope {
                        continuation: DecodeContinuation::Before(&operator_text),
                        structural_stops: scope.structural_stops,
                    },
                )?;
                input.consume_operator(self, *operator)?;
                let payload = self.decode_role(*payload, input, state, bindings, active, scope)?;
                Ok(DraftFieldValue::Application {
                    head: Rc::new(head),
                    payload: Rc::new(payload),
                })
            }
            SharedDescriptor::Delimited { boundary, content }
            | SharedDescriptor::ItemBoundary { boundary, content } => {
                let mut interior = input.take_boundary(self, *boundary)?;
                let value =
                    self.decode_repeated_role(*content, &mut interior, state, bindings, active)?;
                if !interior.finish(self)? {
                    return Err(DecodeError::RepetitionCardinality { found: 0 });
                }
                Ok(DraftFieldValue::Delimited(Rc::new(value)))
            }
            SharedDescriptor::Repeated { .. } => Err(DecodeError::BlockKindMismatch {
                expected: "repeated children",
                found: "source",
            }),
        }
    }

    fn decode_ordered_product<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        product: &crate::form::OrderedProduct,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        bindings: &Bindings,
        active: &[DecodeProgressKey<Root>],
    ) -> Result<(), DecodeError<Root>> {
        input.skip_trivia(self)?;
        let found = input.children.len().saturating_sub(input.child_index);
        if found != product.len() {
            return Err(DecodeError::ProductArityMismatch {
                expected: product.len(),
                found,
            });
        }

        for (position, role) in product.members().iter().copied().enumerate() {
            input.skip_trivia(self)?;
            let Some(child) = input.next_child_at_cursor() else {
                let bound = SourceBound::checked(input.source, input.position, input.position)?;
                return Err(DecodeError::ProductPositionMismatch {
                    position,
                    role,
                    bound,
                });
            };
            let bound = child.source_bound();
            let child_index = input.child_index;
            let decoded = self.decode_role(
                role,
                input,
                state,
                bindings,
                active,
                DecodeScope {
                    continuation: DecodeContinuation::Repeated,
                    structural_stops: &[],
                },
            );
            match decoded {
                Ok(_) if input.child_index == child_index + 1 && input.position == bound.end() => {}
                Ok(_)
                | Err(
                    DecodeError::BlockKindMismatch { .. }
                    | DecodeError::CaseMismatch
                    | DecodeError::LiteralMismatch
                    | DecodeError::DelegationPayloadMismatch { .. }
                    | DecodeError::RepetitionCardinality { .. }
                    | DecodeError::LeafNotFlattenable
                    | DecodeError::ScalarParse(_)
                    | DecodeError::BoundaryMismatch { .. }
                    | DecodeError::ProductArityMismatch { .. }
                    | DecodeError::ProductPositionMismatch { .. }
                    | DecodeError::NoAlternative { .. },
                ) => {
                    return Err(DecodeError::ProductPositionMismatch {
                        position,
                        role,
                        bound,
                    });
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn decode_ordered_sequence<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        sequence: &crate::form::OrderedSequence,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        bindings: &Bindings,
        active: &[DecodeProgressKey<Root>],
        scope: DecodeScope<'_, '_>,
    ) -> Result<(), DecodeError<Root>> {
        for (position, role) in sequence.members().iter().copied().enumerate() {
            let descriptor = state.descriptor(role)?.clone();
            if matches!(descriptor, SharedDescriptor::Repeated { .. }) {
                self.decode_sequence_repeated_role(role, input, state, bindings, active)?;
                continue;
            }
            let continuation = if position + 1 == sequence.len() {
                scope.continuation
            } else {
                DecodeContinuation::Repeated
            };
            let before = input.position;
            self.decode_role(
                role,
                input,
                state,
                bindings,
                active,
                DecodeScope {
                    continuation,
                    structural_stops: scope.structural_stops,
                },
            )?;
            if input.position == before {
                return Err(DecodeError::ProductPositionMismatch {
                    position,
                    role,
                    bound: SourceBound::checked(input.source, before, before)?,
                });
            }
        }
        Ok(())
    }

    fn decode_sequence_repeated_role<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        role: StableRoleId,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        bindings: &Bindings,
        active: &[DecodeProgressKey<Root>],
    ) -> Result<DraftFieldValue<Root>, DecodeError<Root>> {
        let descriptor = state.descriptor(role)?.clone();
        let SharedDescriptor::Repeated {
            minimum,
            maximum,
            element,
        } = descriptor
        else {
            return Err(DecodeError::MissingRole { role });
        };
        input.skip_trivia(self)?;
        let start = input.position;
        let mut values = Vec::new();
        while maximum
            .is_none_or(|top| u64::try_from(values.len()).expect("platform usize fits u64") < top)
        {
            let mut candidate = input.clone();
            let mut candidate_state = state.clone();
            match self.decode_descriptor(
                &element,
                &mut candidate,
                &mut candidate_state,
                bindings,
                active,
                DecodeScope {
                    continuation: DecodeContinuation::Repeated,
                    structural_stops: &[],
                },
            ) {
                Ok(value) if candidate.position > input.position => {
                    *input = candidate;
                    *state = candidate_state;
                    values.push(value);
                }
                Ok(_) => return Err(DecodeError::LeafNotFlattenable),
                Err(error) if error.is_structural_non_match() => break,
                Err(error) => return Err(error),
            }
        }
        let found = u64::try_from(values.len()).expect("platform usize fits u64");
        if found < minimum {
            return Err(DecodeError::RepetitionCardinality { found });
        }
        let value = DraftFieldValue::Repeated(values);
        let bound = SourceBound::checked(input.source, start, input.position)?;
        state.fields.insert(role, value.clone());
        state.field_bounds.insert(role, bound);
        Ok(value)
    }

    fn decode_repeated_role<Bindings: DecodeNameBindings<Root> + ?Sized>(
        &self,
        role: StableRoleId,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        bindings: &Bindings,
        active: &[DecodeProgressKey<Root>],
    ) -> Result<DraftFieldValue<Root>, DecodeError<Root>> {
        let descriptor = state.descriptor(role)?.clone();
        let SharedDescriptor::Repeated {
            minimum,
            maximum,
            element,
        } = descriptor
        else {
            return Err(DecodeError::MissingRole { role });
        };
        input.skip_trivia(self)?;
        let start = input.position;
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
                bindings,
                active,
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
        let bound = SourceBound::checked(input.source, start, input.position)?;
        state.fields.insert(role, value.clone());
        state.field_bounds.insert(role, bound);
        Ok(value)
    }

    fn take_source_atom(
        &self,
        input: &mut BoundedCursor<'_, '_>,
        terminator: Option<&str>,
        structural_stops: &[String],
    ) -> Result<(Atom, SourceBound), DecodeError<Root>> {
        let bound = input.take_bare(self, terminator, structural_stops)?;
        let text = input.text(bound);
        if text.contains('.') {
            return Err(DecodeError::BlockKindMismatch {
                expected: "atom",
                found: "dotted source",
            });
        }
        Ok((Atom::new(text), bound))
    }

    fn verify_bound_spelling(
        &self,
        resolver: &(impl EncodedNameResolver<Root> + ?Sized),
        encoded_id: &name_table::EncodedId<Root>,
        source_spelling: &str,
        bound: SourceBound,
    ) -> Result<(), DecodeError<Root>> {
        let resolved =
            resolver
                .resolve(encoded_id)
                .ok_or_else(|| DecodeError::UnknownEncodedName {
                    encoded_id: encoded_id.clone(),
                })?;
        if resolved.as_str() != source_spelling {
            return Err(DecodeError::NameBindingMismatch { bound });
        }
        Ok(())
    }

    fn decode_leaf(
        &self,
        codec: &LeafCodec,
        input: &mut BoundedCursor<'_, '_>,
        terminator: Option<&str>,
        structural_stops: &[String],
    ) -> Result<ScalarValue, DecodeError<Root>> {
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
    pub fn encode_text<Resolver: EncodedNameResolver<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        value: &StructuralValue<Root>,
        resolver: &Resolver,
    ) -> Result<String, EncodeError<Root>> {
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
    fn encode_type_at_context<Resolver: EncodedNameResolver<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        value: &StructuralValue<Root>,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError<Root>> {
        let entry = self
            .table
            .entry(expected)
            .ok_or_else(|| EncodeError::UnknownType(expected.clone()))?;
        let codec = entry
            .constructors()
            .iter()
            .find(|codec| codec.constructor() == value.constructor())
            .ok_or(EncodeError::UnknownConstructor {
                chosen: value.constructor().clone(),
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

    fn encode_role<Resolver: EncodedNameResolver<Root> + ?Sized>(
        &self,
        role: StableRoleId,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
        mirror: &RoleKeyedMirror<Root>,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError<Root>> {
        let descriptor = descriptors
            .get(&role)
            .ok_or(EncodeError::MissingRole { role })?;
        let value = mirror
            .value_by_stable_role(role)
            .ok_or(EncodeError::MissingRole { role })?;
        self.encode_descriptor(descriptor, value, descriptors, mirror, resolver, context)
    }

    fn encode_descriptor<Resolver: EncodedNameResolver<Root> + ?Sized>(
        &self,
        descriptor: &SharedDescriptor<Root>,
        value: &FieldValue<Root>,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
        mirror: &RoleKeyedMirror<Root>,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError<Root>> {
        match (descriptor, value) {
            (SharedDescriptor::Declaration(_), FieldValue::Declaration(assignment)) => {
                Self::resolve_text(resolver, assignment.encoded_id())
            }
            (SharedDescriptor::Reference(_), FieldValue::Reference(reference)) => {
                Self::resolve_text(resolver, reference.encoded_id())
            }
            (SharedDescriptor::Literal(expected), FieldValue::Literal(identifier))
                if expected == identifier =>
            {
                Self::resolve_text(resolver, identifier)
            }
            (SharedDescriptor::Literal(_), FieldValue::Literal(_)) => {
                Err(EncodeError::LiteralMismatch)
            }
            (SharedDescriptor::Leaf(codec), FieldValue::Scalar(value)) => {
                self.encode_leaf(codec, value, context)
            }
            (SharedDescriptor::Delegate { target, payload }, FieldValue::Delegated(value)) => {
                let text = self.encode_type_at_context(target, value, resolver, context)?;
                if let Some(payload) = payload {
                    let atom = Atom::new(&text);
                    if text.contains('.') || !payload.accepts(&atom) {
                        return Err(EncodeError::DelegationPayloadMismatch { payload: *payload });
                    }
                }
                Ok(text)
            }
            (SharedDescriptor::OrderedProduct(product), FieldValue::OrderedProduct) => {
                let rendered = product
                    .members()
                    .iter()
                    .map(|role| self.encode_role(*role, descriptors, mirror, resolver, context))
                    .collect::<Result<Vec<_>, _>>()?;
                self.join_product_members(rendered, context)
            }
            (SharedDescriptor::OrderedSequence(sequence), FieldValue::OrderedProduct) => {
                let rendered = sequence
                    .members()
                    .iter()
                    .map(|role| self.encode_role(*role, descriptors, mirror, resolver, context))
                    .collect::<Result<Vec<_>, _>>()?;
                self.join_product_members(rendered, context)
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
            (
                SharedDescriptor::Repeated {
                    minimum,
                    maximum,
                    element,
                },
                FieldValue::Repeated(values),
            ) => self.encode_repeated_values(
                element,
                *minimum,
                *maximum,
                values,
                descriptors,
                mirror,
                resolver,
                context,
            ),
            _ => Err(EncodeError::ShapeMismatch),
        }
    }

    fn join_product_members(
        &self,
        rendered: Vec<String>,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError<Root>> {
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

    fn encode_repeated_role<Resolver: EncodedNameResolver<Root> + ?Sized>(
        &self,
        role: StableRoleId,
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
        mirror: &RoleKeyedMirror<Root>,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError<Root>> {
        let SharedDescriptor::Repeated {
            minimum,
            maximum,
            element,
        } = descriptors
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
        self.encode_repeated_values(
            element,
            *minimum,
            *maximum,
            values,
            descriptors,
            mirror,
            resolver,
            context,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_repeated_values<Resolver: EncodedNameResolver<Root> + ?Sized>(
        &self,
        element: &SharedDescriptor<Root>,
        minimum: u64,
        maximum: Option<u64>,
        values: &[FieldValue<Root>],
        descriptors: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
        mirror: &RoleKeyedMirror<Root>,
        resolver: &Resolver,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError<Root>> {
        let found = u64::try_from(values.len()).expect("platform usize fits u64");
        if found < minimum || maximum.is_some_and(|top| found > top) {
            return Err(EncodeError::RepetitionCardinality { found });
        }
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

    fn resolve_text(
        resolver: &(impl EncodedNameResolver<Root> + ?Sized),
        encoded_id: &name_table::EncodedId<Root>,
    ) -> Result<String, EncodeError<Root>> {
        resolver
            .resolve(encoded_id)
            .map(|name| name.as_str().to_owned())
            .ok_or_else(|| EncodeError::UnknownEncodedName {
                encoded_id: encoded_id.clone(),
            })
    }

    fn encode_leaf(
        &self,
        codec: &LeafCodec,
        value: &ScalarValue,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<String, EncodeError<Root>> {
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
    ) -> Result<String, EncodeError<Root>> {
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

    fn operator_text(&self, identifier: TriggerIdentifier) -> Result<String, DecodeError<Root>> {
        match &self.profile.definition(identifier)?.trigger {
            Trigger::Application { glyph } | Trigger::Punctuation { glyph } => Ok(glyph.clone()),
            _ => Err(DecodeError::BlockKindMismatch {
                expected: "application trigger",
                found: "profile trigger",
            }),
        }
    }

    fn operator_text_encode(
        &self,
        identifier: TriggerIdentifier,
    ) -> Result<&str, EncodeError<Root>> {
        match &self.profile.definition(identifier)?.trigger {
            Trigger::Application { glyph } | Trigger::Punctuation { glyph } => Ok(glyph),
            _ => Err(EncodeError::ShapeMismatch),
        }
    }
}
