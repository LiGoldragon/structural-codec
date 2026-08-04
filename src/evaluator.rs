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
use crate::plan::{PlannedFieldValue, PlannedName, PlannedStructuralValue};
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

    fn materialize_plan(self) -> PlannedStructuralValue<Root> {
        PlannedStructuralValue::new(
            self.constructor,
            self.fields
                .into_iter()
                .map(|(role, value)| (role, value.materialize_plan()))
                .collect(),
            self.field_bounds,
        )
    }
}

#[derive(Clone)]
enum DraftName<Root> {
    Bound(name_table::EncodedId<Root>),
    Planned(PlannedName),
}

#[derive(Clone)]
enum DraftFieldValue<Root> {
    Declaration(DraftName<Root>),
    Reference(DraftName<Root>),
    Literal(name_table::EncodedId<Root>),
    Scalar(ScalarValue),
    OrderedProduct,
    Delimited(Rc<DraftFieldValue<Root>>),
    Carrier(Rc<DraftFieldValue<Root>>),
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
            Self::Declaration(DraftName::Bound(encoded_id)) => {
                FieldValue::Declaration(DeclarationAssignment::new(encoded_id))
            }
            Self::Reference(DraftName::Bound(encoded_id)) => {
                FieldValue::Reference(ResolvedReference::new(encoded_id))
            }
            Self::Declaration(DraftName::Planned(_)) | Self::Reference(DraftName::Planned(_)) => {
                unreachable!("bound decoding cannot contain planned names")
            }
            Self::Literal(identifier) => FieldValue::Literal(identifier),
            Self::Scalar(value) => FieldValue::Scalar(value),
            Self::OrderedProduct => FieldValue::OrderedProduct,
            Self::Delimited(value) => {
                FieldValue::Delimited(Box::new(value.as_ref().clone().materialize()))
            }
            Self::Carrier(value) => {
                FieldValue::Carrier(Box::new(value.as_ref().clone().materialize()))
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

    fn materialize_plan(self) -> PlannedFieldValue<Root> {
        match self {
            Self::Declaration(DraftName::Planned(name)) => PlannedFieldValue::Declaration(name),
            Self::Reference(DraftName::Planned(name)) => PlannedFieldValue::Reference(name),
            Self::Declaration(DraftName::Bound(_)) | Self::Reference(DraftName::Bound(_)) => {
                unreachable!("allocation-free planning cannot contain bound names")
            }
            Self::Literal(identifier) => PlannedFieldValue::Literal(identifier),
            Self::Scalar(value) => PlannedFieldValue::Scalar(value),
            Self::OrderedProduct => PlannedFieldValue::OrderedProduct,
            Self::Delimited(value) => {
                PlannedFieldValue::Delimited(Box::new(value.as_ref().clone().materialize_plan()))
            }
            Self::Carrier(value) => {
                PlannedFieldValue::Carrier(Box::new(value.as_ref().clone().materialize_plan()))
            }
            Self::Application { head, payload } => PlannedFieldValue::Application {
                head: Box::new(head.as_ref().clone().materialize_plan()),
                payload: Box::new(payload.as_ref().clone().materialize_plan()),
            },
            Self::Delegated(value) => {
                PlannedFieldValue::Delegated(Box::new(value.as_ref().clone().materialize_plan()))
            }
            Self::Repeated(values) => PlannedFieldValue::Repeated(
                values
                    .into_iter()
                    .map(DraftFieldValue::materialize_plan)
                    .collect(),
            ),
        }
    }
}

trait EvaluationNames<Root>: EncodedNameResolver<Root> {
    fn declaration(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Result<DraftName<Root>, DecodeError<Root>>;

    fn reference(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Result<DraftName<Root>, DecodeError<Root>>;
}

struct BoundEvaluationNames<'bindings, Bindings: ?Sized> {
    bindings: &'bindings Bindings,
}

impl<Root, Bindings> EncodedNameResolver<Root> for BoundEvaluationNames<'_, Bindings>
where
    Bindings: DecodeNameBindings<Root> + ?Sized,
{
    fn resolve(&self, encoded_id: &name_table::EncodedId<Root>) -> Option<&name_table::Name> {
        self.bindings.resolve(encoded_id)
    }
}

impl<Root, Bindings> EvaluationNames<Root> for BoundEvaluationNames<'_, Bindings>
where
    Bindings: DecodeNameBindings<Root> + ?Sized,
{
    fn declaration(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Result<DraftName<Root>, DecodeError<Root>> {
        let bound = occurrence.bound();
        self.bindings
            .declaration_assignment(occurrence)
            .map(DeclarationAssignment::into_encoded_id)
            .map(DraftName::Bound)
            .ok_or(DecodeError::MissingDeclarationAssignment { bound })
    }

    fn reference(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Result<DraftName<Root>, DecodeError<Root>> {
        let bound = occurrence.bound();
        self.bindings
            .reference_resolution(occurrence)
            .map(ResolvedReference::into_encoded_id)
            .map(DraftName::Bound)
            .ok_or(DecodeError::UnresolvedReference { bound })
    }
}

struct PlanningEvaluationNames<'resolver, Resolver: ?Sized> {
    resolver: &'resolver Resolver,
}

impl<Root, Resolver> EncodedNameResolver<Root> for PlanningEvaluationNames<'_, Resolver>
where
    Resolver: EncodedNameResolver<Root> + ?Sized,
{
    fn resolve(&self, encoded_id: &name_table::EncodedId<Root>) -> Option<&name_table::Name> {
        self.resolver.resolve(encoded_id)
    }
}

impl<Root, Resolver> EvaluationNames<Root> for PlanningEvaluationNames<'_, Resolver>
where
    Resolver: EncodedNameResolver<Root> + ?Sized,
{
    fn declaration(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Result<DraftName<Root>, DecodeError<Root>> {
        Ok(DraftName::Planned(PlannedName::new(
            occurrence.spelling(),
            occurrence.bound(),
        )))
    }

    fn reference(
        &self,
        occurrence: NameOccurrence<'_>,
    ) -> Result<DraftName<Root>, DecodeError<Root>> {
        Ok(DraftName::Planned(PlannedName::new(
            occurrence.spelling(),
            occurrence.bound(),
        )))
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
    ) -> Result<Option<(String, SourceBound)>, DecodeError<Root>>
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
        Ok(Some((
            matched
                .body()
                .expect("carrier trigger matches carry a body")
                .to_owned(),
            SourceBound::checked(self.source, matched.start(), matched.end())?,
        )))
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

    /// Prove an application cue exists after the next structural head unit
    /// before the head is allowed to perform a semantic name query.
    ///
    /// Open repetitions use a failed application attempt to find the boundary
    /// with their following position. A bare token at that boundary must not
    /// be mistaken for an application head merely because the head descriptor
    /// itself could resolve it.
    fn application_operator_follows_head<Root, Record: StructureRecord<Root>>(
        &self,
        evaluator: &StructuralEvaluator<'_, Root, Record>,
        identifier: TriggerIdentifier,
        structural_stops: &[String],
    ) -> Result<bool, DecodeError<Root>>
    where
        Root: Clone + Ord,
    {
        let operator = evaluator.operator_text(identifier)?;
        let mut probe = self.clone();
        probe.skip_trivia(evaluator)?;

        if let Some(child) = probe.next_child_at_cursor() {
            probe.position = child.source_bound().end();
            probe.child_index += 1;
        } else if let Some(matched) = probe.local_match(evaluator)? {
            if matched.kind() != TriggerMatchKind::Carrier {
                return Ok(false);
            }
            probe.position = matched.end();
        } else if probe
            .take_bare(evaluator, Some(&operator), structural_stops)
            .is_err()
        {
            return Ok(false);
        }

        Ok(probe.source[probe.position..probe.bound.end()].starts_with(&operator))
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
        let names = BoundEvaluationNames { bindings };
        self.decode_text_to_draft(expected, source, &names)
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
        let names = BoundEvaluationNames { bindings };
        self.decode_text_to_draft(expected, source, &names)
            .map(DraftStructuralValue::materialize_bounded)
    }

    /// Select and validate one complete structural branch without allocating
    /// or accepting identities for declaration and reference occurrences.
    ///
    /// Fixed literal and exclusion vocabulary still resolve through immutable
    /// encoded identity. The returned role-keyed tree retains exact source
    /// spellings and bounds only at this TextualForm boundary.
    pub fn plan_text<Resolver: EncodedNameResolver<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        source: &str,
        resolver: &Resolver,
    ) -> Result<PlannedStructuralValue<Root>, DecodeError<Root>> {
        let names = PlanningEvaluationNames { resolver };
        self.decode_text_to_draft(expected, source, &names)
            .map(DraftStructuralValue::materialize_plan)
    }

    fn decode_text_to_draft<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        source: &str,
        names: &Names,
    ) -> Result<DraftStructuralValue<Root>, DecodeError<Root>> {
        let tree =
            DiscoveredBlockTree::discover(source, self.profile, self.table.block_discovery())?;
        let mut cursor =
            BoundedCursor::root(source, &tree, self.table.block_discovery().root_context());
        cursor.skip_trivia(self)?;
        if cursor.position == cursor.bound.end() {
            return Err(DecodeError::RootObjectCount);
        }
        let value =
            self.decode_type(expected, &mut cursor, names, &[], DecodeContinuation::Bound)?;
        if !cursor.finish(self)? {
            return Err(DecodeError::RootObjectCount);
        }
        Ok(value)
    }

    fn decode_type<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        expected: &EncodedTypeId<Root>,
        input: &mut BoundedCursor<'_, '_>,
        names: &Names,
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
                match self.decode_role(root, &mut candidate, &mut state, names, &next_active, scope)
                {
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
                    Err(error @ DecodeError::SequenceRepetitionBoundary { .. })
                        if single_decode_form =>
                    {
                        return Err(error);
                    }
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
                if let SharedDescriptor::OrderedSequence(sequence)
                | SharedDescriptor::AdjacentSequence(sequence) = state.descriptor(root)?
                {
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

    fn decode_role<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        role: StableRoleId,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        names: &Names,
        active: &[DecodeProgressKey<Root>],
        scope: DecodeScope<'_, '_>,
    ) -> Result<DraftFieldValue<Root>, DecodeError<Root>> {
        input.skip_trivia(self)?;
        let start = input.position;
        let descriptor = state.descriptor(role)?.clone();
        let value = self.decode_descriptor(&descriptor, input, state, names, active, scope)?;
        let bound = SourceBound::checked(input.source, start, input.position)?;
        state.fields.insert(role, value.clone());
        state.field_bounds.insert(role, bound);
        Ok(value)
    }

    fn decode_descriptor<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        descriptor: &SharedDescriptor<Root>,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        names: &Names,
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
                let assignment = names.declaration(NameOccurrence::new(atom.text(), bound))?;
                self.verify_draft_spelling(names, &assignment, atom.text(), bound)?;
                Ok(DraftFieldValue::Declaration(assignment))
            }
            SharedDescriptor::DeclarationExcluding {
                atom: form,
                excluded,
            } => {
                let (atom, bound) = self.take_source_atom(
                    input,
                    scope.continuation.terminator(),
                    scope.structural_stops,
                )?;
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                if excluded.iter().any(|encoded_id| {
                    names
                        .resolve(encoded_id)
                        .is_some_and(|name| name.as_str() == atom.text())
                }) {
                    return Err(DecodeError::ExcludedNameIdentity { bound });
                }
                let assignment = names.declaration(NameOccurrence::new(atom.text(), bound))?;
                self.verify_draft_spelling(names, &assignment, atom.text(), bound)?;
                if matches!(&assignment, DraftName::Bound(encoded_id) if excluded.contains(encoded_id))
                {
                    return Err(DecodeError::ExcludedNameIdentity { bound });
                }
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
                let reference = names.reference(NameOccurrence::new(atom.text(), bound))?;
                self.verify_draft_spelling(names, &reference, atom.text(), bound)?;
                Ok(DraftFieldValue::Reference(reference))
            }
            SharedDescriptor::ReferenceExcluding {
                atom: form,
                excluded,
            } => {
                let (atom, bound) = self.take_source_atom(
                    input,
                    scope.continuation.terminator(),
                    scope.structural_stops,
                )?;
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                if excluded.iter().any(|encoded_id| {
                    names
                        .resolve(encoded_id)
                        .is_some_and(|name| name.as_str() == atom.text())
                }) {
                    return Err(DecodeError::ExcludedNameIdentity { bound });
                }
                let reference = names.reference(NameOccurrence::new(atom.text(), bound))?;
                self.verify_draft_spelling(names, &reference, atom.text(), bound)?;
                if matches!(&reference, DraftName::Bound(encoded_id) if excluded.contains(encoded_id))
                {
                    return Err(DecodeError::ExcludedNameIdentity { bound });
                }
                Ok(DraftFieldValue::Reference(reference))
            }
            SharedDescriptor::Literal(identifier) => {
                let (atom, _) = self.take_source_atom(
                    input,
                    scope.continuation.terminator(),
                    scope.structural_stops,
                )?;
                let spelling =
                    names
                        .resolve(identifier)
                        .ok_or_else(|| DecodeError::UnknownEncodedName {
                            encoded_id: identifier.clone(),
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
                    names,
                    active,
                    scope.continuation,
                )?)))
            }
            SharedDescriptor::OrderedProduct(product) => {
                self.decode_ordered_product(product, input, state, names, active)?;
                Ok(DraftFieldValue::OrderedProduct)
            }
            SharedDescriptor::OrderedSequence(sequence) => {
                self.decode_ordered_sequence(sequence, input, state, names, active, scope)?;
                Ok(DraftFieldValue::OrderedProduct)
            }
            SharedDescriptor::AdjacentSequence(sequence) => {
                self.decode_ordered_sequence(sequence, input, state, names, active, scope)?;
                Ok(DraftFieldValue::OrderedProduct)
            }
            SharedDescriptor::Application {
                operator,
                head,
                payload,
            } => {
                if !input.application_operator_follows_head(
                    self,
                    *operator,
                    scope.structural_stops,
                )? {
                    return Err(DecodeError::BlockKindMismatch {
                        expected: "application",
                        found: "source",
                    });
                }
                let operator_text = self.operator_text(*operator)?;
                let head = self.decode_role(
                    *head,
                    input,
                    state,
                    names,
                    active,
                    DecodeScope {
                        continuation: DecodeContinuation::Before(&operator_text),
                        structural_stops: scope.structural_stops,
                    },
                )?;
                input.consume_operator(self, *operator)?;
                let payload = self.decode_role(*payload, input, state, names, active, scope)?;
                Ok(DraftFieldValue::Application {
                    head: Rc::new(head),
                    payload: Rc::new(payload),
                })
            }
            SharedDescriptor::InlineApplication {
                operator,
                head,
                payload,
            } => {
                if !input.application_operator_follows_head(
                    self,
                    *operator,
                    scope.structural_stops,
                )? {
                    return Err(DecodeError::BlockKindMismatch {
                        expected: "application",
                        found: "source",
                    });
                }
                let operator_text = self.operator_text(*operator)?;
                let head = self.decode_descriptor(
                    head,
                    input,
                    state,
                    names,
                    active,
                    DecodeScope {
                        continuation: DecodeContinuation::Before(&operator_text),
                        structural_stops: scope.structural_stops,
                    },
                )?;
                input.consume_operator(self, *operator)?;
                let payload =
                    self.decode_descriptor(payload, input, state, names, active, scope)?;
                Ok(DraftFieldValue::Application {
                    head: Rc::new(head),
                    payload: Rc::new(payload),
                })
            }
            SharedDescriptor::Alternation(alternatives) => {
                for alternative in alternatives {
                    let mut candidate = input.clone();
                    let mut candidate_state = state.clone();
                    match self.decode_descriptor(
                        alternative,
                        &mut candidate,
                        &mut candidate_state,
                        names,
                        active,
                        scope,
                    ) {
                        Ok(value) => {
                            *input = candidate;
                            *state = candidate_state;
                            return Ok(value);
                        }
                        Err(error) if error.is_structural_non_match() => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(DecodeError::NoDescriptorAlternative)
            }
            SharedDescriptor::Delimited { boundary, content }
            | SharedDescriptor::ItemBoundary { boundary, content } => {
                let mut interior = input.take_boundary(self, *boundary)?;
                let value = if matches!(
                    state.descriptor(*content)?,
                    SharedDescriptor::Repeated { .. }
                ) {
                    self.decode_repeated_role(*content, &mut interior, state, names, active)?
                } else {
                    self.decode_role(
                        *content,
                        &mut interior,
                        state,
                        names,
                        active,
                        DecodeScope {
                            continuation: DecodeContinuation::Bound,
                            structural_stops: &[],
                        },
                    )?
                };
                if !interior.finish(self)? {
                    return Err(DecodeError::RepetitionCardinality { found: 0 });
                }
                Ok(DraftFieldValue::Delimited(Rc::new(value)))
            }
            SharedDescriptor::Carrier { carrier, content } => {
                let Some((body, bound)) = input.take_carrier(self)? else {
                    return Err(DecodeError::BlockKindMismatch {
                        expected: "carrier",
                        found: "source",
                    });
                };
                if !matches!(
                    self.profile.definition(*carrier)?.trigger,
                    Trigger::Carrier { .. } | Trigger::CurlyText
                ) {
                    return Err(DecodeError::BlockKindMismatch {
                        expected: "carrier trigger",
                        found: "profile trigger",
                    });
                }
                Ok(DraftFieldValue::Carrier(Rc::new(
                    self.decode_carrier_content(content, &body, bound, names)?,
                )))
            }
            SharedDescriptor::Repeated { .. } => Err(DecodeError::BlockKindMismatch {
                expected: "repeated children",
                found: "source",
            }),
        }
    }

    fn decode_ordered_product<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        product: &crate::form::OrderedProduct,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        names: &Names,
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
                names,
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

    fn decode_ordered_sequence<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        sequence: &crate::form::OrderedSequence,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        names: &Names,
        active: &[DecodeProgressKey<Root>],
        scope: DecodeScope<'_, '_>,
    ) -> Result<(), DecodeError<Root>> {
        self.decode_ordered_sequence_from(sequence, 0, input, state, names, active, scope)
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_ordered_sequence_from<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        sequence: &crate::form::OrderedSequence,
        position: usize,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        names: &Names,
        active: &[DecodeProgressKey<Root>],
        scope: DecodeScope<'_, '_>,
    ) -> Result<(), DecodeError<Root>> {
        let Some(role) = sequence.members().get(position).copied() else {
            return Ok(());
        };
        let descriptor = state.descriptor(role)?.clone();

        if let SharedDescriptor::Repeated {
            minimum,
            maximum,
            element,
        } = descriptor
        {
            return self.decode_sequence_repetition_with_tail(
                sequence, position, role, minimum, maximum, &element, input, state, names, active,
                scope,
            );
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
            names,
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
        self.decode_ordered_sequence_from(
            sequence,
            position + 1,
            input,
            state,
            names,
            active,
            scope,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_sequence_repetition_with_tail<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        sequence: &crate::form::OrderedSequence,
        position: usize,
        role: StableRoleId,
        minimum: u64,
        maximum: Option<u64>,
        element: &SharedDescriptor<Root>,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        names: &Names,
        active: &[DecodeProgressKey<Root>],
        scope: DecodeScope<'_, '_>,
    ) -> Result<(), DecodeError<Root>> {
        input.skip_trivia(self)?;
        let start = input.position;
        let mut scan_input = input.clone();
        let mut scan_state = state.clone();
        let mut values = Vec::new();
        let mut candidates = Vec::new();
        if minimum == 0 {
            candidates.push((scan_input.clone(), scan_state.clone(), values.clone()));
        }
        let mut best_refusal = None;

        while maximum
            .is_none_or(|top| u64::try_from(values.len()).expect("platform usize fits u64") < top)
        {
            let mut candidate = scan_input.clone();
            let mut candidate_state = scan_state.clone();
            match self.decode_descriptor(
                element,
                &mut candidate,
                &mut candidate_state,
                names,
                active,
                DecodeScope {
                    continuation: DecodeContinuation::Repeated,
                    structural_stops: &[],
                },
            ) {
                Ok(value) if candidate.position > scan_input.position => {
                    scan_input = candidate;
                    scan_state = candidate_state;
                    values.push(value);
                    if u64::try_from(values.len()).expect("platform usize fits u64") >= minimum {
                        candidates.push((scan_input.clone(), scan_state.clone(), values.clone()));
                    }
                }
                Ok(_) => return Err(DecodeError::LeafNotFlattenable),
                Err(error) => {
                    best_refusal = Some((candidate.position, error));
                    break;
                }
            }
        }

        for (mut candidate_input, mut candidate_state, candidate_values) in
            candidates.into_iter().rev()
        {
            let repeated = DraftFieldValue::Repeated(candidate_values);
            let bound = SourceBound::checked(input.source, start, candidate_input.position)?;
            candidate_state.fields.insert(role, repeated);
            candidate_state.field_bounds.insert(role, bound);
            match self.decode_ordered_sequence_from(
                sequence,
                position + 1,
                &mut candidate_input,
                &mut candidate_state,
                names,
                active,
                scope,
            ) {
                Ok(()) => {
                    *input = candidate_input;
                    *state = candidate_state;
                    return Ok(());
                }
                Err(refusal) => {
                    let progress = candidate_input.position;
                    let refusal_is_structural = refusal.is_structural_non_match();
                    let replaces = best_refusal.as_ref().is_none_or(
                        |(best_progress, best): &(usize, DecodeError<Root>)| {
                            let best_is_structural = best.is_structural_non_match();
                            (best_is_structural && !refusal_is_structural)
                                || (best_is_structural == refusal_is_structural
                                    && progress > *best_progress)
                        },
                    );
                    if replaces {
                        best_refusal = Some((progress, refusal));
                    }
                }
            }
        }

        let refusal = best_refusal.map(|(_, refusal)| refusal).unwrap_or_else(|| {
            DecodeError::RepetitionCardinality {
                found: u64::try_from(values.len()).expect("platform usize fits u64"),
            }
        });
        Err(DecodeError::SequenceRepetitionBoundary {
            role,
            refusal: Box::new(refusal),
        })
    }

    fn decode_repeated_role<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        role: StableRoleId,
        input: &mut BoundedCursor<'_, '_>,
        state: &mut DecodeState<Root>,
        names: &Names,
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
                names,
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

    fn verify_draft_spelling(
        &self,
        resolver: &(impl EncodedNameResolver<Root> + ?Sized),
        name: &DraftName<Root>,
        source_spelling: &str,
        bound: SourceBound,
    ) -> Result<(), DecodeError<Root>> {
        let DraftName::Bound(encoded_id) = name else {
            return Ok(());
        };
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
        if let Some((body, _)) = carrier.take_carrier(self)? {
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

    fn decode_carrier_content<Names: EvaluationNames<Root> + ?Sized>(
        &self,
        descriptor: &SharedDescriptor<Root>,
        body: &str,
        bound: SourceBound,
        names: &Names,
    ) -> Result<DraftFieldValue<Root>, DecodeError<Root>> {
        let atom = Atom::new(body);
        match descriptor {
            SharedDescriptor::Alternation(alternatives) => {
                for alternative in alternatives {
                    match self.decode_carrier_content(alternative, body, bound, names) {
                        Ok(value) => return Ok(value),
                        Err(error) if error.is_structural_non_match() => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(DecodeError::NoDescriptorAlternative)
            }
            SharedDescriptor::Declaration(form) => {
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                let assignment = names.declaration(NameOccurrence::new(body, bound))?;
                self.verify_draft_spelling(names, &assignment, body, bound)?;
                Ok(DraftFieldValue::Declaration(assignment))
            }
            SharedDescriptor::DeclarationExcluding {
                atom: form,
                excluded,
            } => {
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                if excluded.iter().any(|encoded_id| {
                    names
                        .resolve(encoded_id)
                        .is_some_and(|name| name.as_str() == body)
                }) {
                    return Err(DecodeError::ExcludedNameIdentity { bound });
                }
                let assignment = names.declaration(NameOccurrence::new(body, bound))?;
                self.verify_draft_spelling(names, &assignment, body, bound)?;
                if matches!(&assignment, DraftName::Bound(encoded_id) if excluded.contains(encoded_id))
                {
                    return Err(DecodeError::ExcludedNameIdentity { bound });
                }
                Ok(DraftFieldValue::Declaration(assignment))
            }
            SharedDescriptor::Reference(form) => {
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                let reference = names.reference(NameOccurrence::new(body, bound))?;
                self.verify_draft_spelling(names, &reference, body, bound)?;
                Ok(DraftFieldValue::Reference(reference))
            }
            SharedDescriptor::ReferenceExcluding {
                atom: form,
                excluded,
            } => {
                if !form.accepts(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                if excluded.iter().any(|encoded_id| {
                    names
                        .resolve(encoded_id)
                        .is_some_and(|name| name.as_str() == body)
                }) {
                    return Err(DecodeError::ExcludedNameIdentity { bound });
                }
                let reference = names.reference(NameOccurrence::new(body, bound))?;
                self.verify_draft_spelling(names, &reference, body, bound)?;
                if matches!(&reference, DraftName::Bound(encoded_id) if excluded.contains(encoded_id))
                {
                    return Err(DecodeError::ExcludedNameIdentity { bound });
                }
                Ok(DraftFieldValue::Reference(reference))
            }
            SharedDescriptor::Literal(identifier) => {
                let spelling =
                    names
                        .resolve(identifier)
                        .ok_or_else(|| DecodeError::UnknownEncodedName {
                            encoded_id: identifier.clone(),
                        })?;
                if spelling.as_str() != body {
                    return Err(DecodeError::LiteralMismatch);
                }
                Ok(DraftFieldValue::Literal(identifier.clone()))
            }
            SharedDescriptor::Leaf(LeafCodec::Text | LeafCodec::PipeText) => {
                Ok(DraftFieldValue::Scalar(ScalarValue::Text(body.to_owned())))
            }
            _ => Err(DecodeError::BlockKindMismatch {
                expected: "carrier-compatible typed content",
                found: "structural descriptor",
            }),
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
            (
                SharedDescriptor::DeclarationExcluding { excluded, .. },
                FieldValue::Declaration(assignment),
            ) => {
                if excluded.contains(assignment.encoded_id()) {
                    return Err(EncodeError::ExcludedNameIdentity);
                }
                Self::resolve_text(resolver, assignment.encoded_id())
            }
            (SharedDescriptor::Reference(_), FieldValue::Reference(reference)) => {
                Self::resolve_text(resolver, reference.encoded_id())
            }
            (
                SharedDescriptor::ReferenceExcluding { excluded, .. },
                FieldValue::Reference(reference),
            ) => {
                if excluded.contains(reference.encoded_id()) {
                    return Err(EncodeError::ExcludedNameIdentity);
                }
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
                if value.constructor().type_id() != target {
                    return Err(EncodeError::DelegationPayloadMismatch { payload: *payload });
                }
                let text = self.encode_type_at_context(target, value, resolver, context)?;
                if let Some(payload) = payload {
                    let atom = Atom::new(&text);
                    if text.contains('.') || !payload.accepts(&atom) {
                        return Err(EncodeError::DelegationPayloadMismatch {
                            payload: Some(*payload),
                        });
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
            (SharedDescriptor::AdjacentSequence(sequence), FieldValue::OrderedProduct) => {
                let rendered = sequence
                    .members()
                    .iter()
                    .map(|role| self.encode_role(*role, descriptors, mirror, resolver, context))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rendered.concat())
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
                SharedDescriptor::InlineApplication {
                    operator,
                    head,
                    payload,
                },
                FieldValue::Application {
                    head: value_head,
                    payload: value_payload,
                },
            ) => Ok(format!(
                "{}{}{}",
                self.encode_descriptor(head, value_head, descriptors, mirror, resolver, context)?,
                self.operator_text_encode(*operator)?,
                self.encode_descriptor(
                    payload,
                    value_payload,
                    descriptors,
                    mirror,
                    resolver,
                    context
                )?,
            )),
            (SharedDescriptor::Alternation(alternatives), value) => {
                for alternative in alternatives {
                    match self.encode_descriptor(
                        alternative,
                        value,
                        descriptors,
                        mirror,
                        resolver,
                        context,
                    ) {
                        Ok(rendered) => return Ok(rendered),
                        Err(
                            EncodeError::ShapeMismatch
                            | EncodeError::LiteralMismatch
                            | EncodeError::ExcludedNameIdentity
                            | EncodeError::DelegationPayloadMismatch { .. }
                            | EncodeError::NoDescriptorAlternative,
                        ) => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(EncodeError::NoDescriptorAlternative)
            }
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
                let content = if matches!(
                    descriptors.get(content),
                    Some(SharedDescriptor::Repeated { .. })
                ) {
                    self.encode_repeated_role(
                        *content,
                        descriptors,
                        mirror,
                        resolver,
                        child_context,
                    )?
                } else {
                    self.encode_role(*content, descriptors, mirror, resolver, child_context)?
                };
                Ok(format!("{}{}{}", opening, content, closing,))
            }
            (SharedDescriptor::Carrier { carrier, content }, FieldValue::Carrier(value)) => {
                let body =
                    self.encode_descriptor(content, value, descriptors, mirror, resolver, context)?;
                self.encode_carrier_with(*carrier, &body)
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
        self.profile
            .definition(carrier)?
            .trigger
            .render_text_carrier(body)
            .ok_or(EncodeError::NonCanonicalSpelling)
    }

    fn encode_carrier_with(
        &self,
        carrier: TriggerIdentifier,
        body: &str,
    ) -> Result<String, EncodeError<Root>> {
        self.profile
            .definition(carrier)?
            .trigger
            .render_text_carrier(body)
            .ok_or(EncodeError::ShapeMismatch)
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
