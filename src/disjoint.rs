//! Conservative disjointness proof over shared descriptors and typed positions.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::StructuralEntry;
use crate::error::{DisjointnessError, DisjointnessReason};
use crate::form::{
    AtomDescriptor, BorrowedFieldView, DelegationPayload, FieldVisitor, Position, SharedDescriptor,
    StructureRecord,
};
use crate::ids::{EncodedTypeId, FieldRole, StableRoleId};

struct Collector<Root> {
    fields: BTreeMap<StableRoleId, SharedDescriptor<Root>>,
}

impl<Root: Clone> FieldVisitor<Root> for Collector<Root> {
    fn field<Role: FieldRole>(&mut self, position: &Position<Role, Root>) {
        self.fields
            .insert(position.role(), position.descriptor().clone());
    }
}

fn fields<Root: Clone, Record: StructureRecord<Root>>(
    rule: &Record,
) -> (StableRoleId, BTreeMap<StableRoleId, SharedDescriptor<Root>>) {
    let mut collector = Collector {
        fields: BTreeMap::new(),
    };
    rule.fields().expose(&mut collector);
    (rule.root_role(), collector.fields)
}

enum Outer<'a, Root> {
    Named(
        Option<raw_discovery::AtomCase>,
        &'a [name_table::EncodedId<Root>],
    ),
    Literal(&'a name_table::EncodedId<Root>),
    Application(&'a SharedDescriptor<Root>, &'a SharedDescriptor<Root>),
    Boundary(raw_discovery::TriggerIdentifier),
    Carrier(raw_discovery::TriggerIdentifier),
    Sequence(&'a crate::form::OrderedSequence),
    Opaque,
}

enum ProofFailure<Root> {
    Reason(DisjointnessReason<Root>),
    Cycle(EncodedTypeId<Root>),
}

type ProofResult<Root> = Result<(), ProofFailure<Root>>;

fn reason<Root, T>(reason: DisjointnessReason<Root>) -> Result<T, ProofFailure<Root>> {
    Err(ProofFailure::Reason(reason))
}

fn outer<'a, Root>(
    descriptor: &'a SharedDescriptor<Root>,
    roles: &'a BTreeMap<StableRoleId, SharedDescriptor<Root>>,
) -> Result<Outer<'a, Root>, ProofFailure<Root>> {
    match descriptor {
        SharedDescriptor::Declaration(atom) | SharedDescriptor::Reference(atom) => {
            Ok(Outer::Named(atom.case, &[]))
        }
        SharedDescriptor::DeclarationExcluding { atom, excluded }
        | SharedDescriptor::ReferenceExcluding { atom, excluded } => {
            Ok(Outer::Named(atom.case, excluded))
        }
        SharedDescriptor::Literal(identifier) => Ok(Outer::Literal(identifier)),
        SharedDescriptor::Application { head, payload, .. } => {
            Ok(Outer::Application(
                roles
                    .get(head)
                    .ok_or(ProofFailure::Reason(DisjointnessReason::MissingRole {
                        role: *head,
                    }))?,
                roles.get(payload).ok_or(ProofFailure::Reason(
                    DisjointnessReason::MissingRole { role: *payload },
                ))?,
            ))
        }
        SharedDescriptor::InlineApplication { head, payload, .. } => {
            Ok(Outer::Application(head, payload))
        }
        SharedDescriptor::Delimited { boundary, .. }
        | SharedDescriptor::ItemBoundary { boundary, .. } => Ok(Outer::Boundary(*boundary)),
        SharedDescriptor::Carrier { carrier, .. } => Ok(Outer::Carrier(*carrier)),
        SharedDescriptor::OrderedSequence(sequence)
        | SharedDescriptor::AdjacentSequence(sequence) => Ok(Outer::Sequence(sequence)),
        SharedDescriptor::Leaf(_)
        | SharedDescriptor::Repeated { .. }
        | SharedDescriptor::OrderedProduct(_) => Ok(Outer::Opaque),
        SharedDescriptor::Alternation(_) => {
            unreachable!("alternations distribute before outer proof")
        }
        SharedDescriptor::Delegate { .. } => unreachable!("delegates expand before outer proof"),
    }
}

impl<Root, Record> StructuralEntry<Root, Record>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    pub fn validate_disjoint(&self) -> Result<(), DisjointnessError<Root>> {
        self.validate_disjoint_against(None)
    }

    pub(crate) fn validate_disjoint_with(
        &self,
        entries: &[StructuralEntry<Root, Record>],
    ) -> Result<(), DisjointnessError<Root>> {
        self.validate_disjoint_against(Some(entries))
    }

    fn validate_disjoint_against(
        &self,
        entries: Option<&[StructuralEntry<Root, Record>]>,
    ) -> Result<(), DisjointnessError<Root>> {
        let alternatives = self
            .constructors()
            .iter()
            .flat_map(|codec| {
                codec
                    .decode_forms()
                    .iter()
                    .map(move |form| (codec.constructor(), form.identity(), form.rule()))
            })
            .collect::<Vec<_>>();
        for (left_constructor, left_form, left) in &alternatives {
            for (right_constructor, right_form, right) in &alternatives {
                if (left_constructor, left_form) >= (right_constructor, right_form) {
                    continue;
                }
                let mut active = BTreeSet::new();
                match prove_rules(*left, *right, entries, &mut active) {
                    Ok(()) => {}
                    Err(ProofFailure::Reason(reason)) => {
                        return Err(DisjointnessError::NotProvablyDisjoint {
                            core_type: self.encoded_type().clone(),
                            first: (*left_constructor).clone(),
                            second: (*right_constructor).clone(),
                            reason,
                        });
                    }
                    Err(ProofFailure::Cycle(reentered)) => {
                        return Err(DisjointnessError::DelegateExpansionCycle {
                            core_type: self.encoded_type().clone(),
                            reentered,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

fn prove_rules<Root, Record>(
    left: &Record,
    right: &Record,
    entries: Option<&[StructuralEntry<Root, Record>]>,
    active: &mut BTreeSet<EncodedTypeId<Root>>,
) -> ProofResult<Root>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    let (left_root, left_roles) = fields(left);
    let (right_root, right_roles) = fields(right);
    prove(
        left_roles.get(&left_root).ok_or(ProofFailure::Reason(
            DisjointnessReason::MissingRole { role: left_root },
        ))?,
        &left_roles,
        right_roles.get(&right_root).ok_or(ProofFailure::Reason(
            DisjointnessReason::MissingRole { role: right_root },
        ))?,
        &right_roles,
        entries,
        active,
    )
}

fn prove<Root, Record>(
    left: &SharedDescriptor<Root>,
    left_roles: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
    right: &SharedDescriptor<Root>,
    right_roles: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
    entries: Option<&[StructuralEntry<Root, Record>]>,
    active: &mut BTreeSet<EncodedTypeId<Root>>,
) -> ProofResult<Root>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    if let SharedDescriptor::Alternation(alternatives) = left {
        for alternative in alternatives {
            let mut branch_active = active.clone();
            prove(
                alternative,
                left_roles,
                right,
                right_roles,
                entries,
                &mut branch_active,
            )?;
        }
        return Ok(());
    }
    if let SharedDescriptor::Alternation(alternatives) = right {
        for alternative in alternatives {
            let mut branch_active = active.clone();
            prove(
                left,
                left_roles,
                alternative,
                right_roles,
                entries,
                &mut branch_active,
            )?;
        }
        return Ok(());
    }

    if let (
        SharedDescriptor::Delegate {
            payload: Some(left_payload),
            ..
        },
        SharedDescriptor::Delegate {
            payload: Some(right_payload),
            ..
        },
    ) = (left, right)
    {
        match prove_payloads(*left_payload, *right_payload) {
            Ok(()) => return Ok(()),
            Err(ProofFailure::Reason(_)) => {}
            Err(cycle) => return Err(cycle),
        }
    }

    match (left, right) {
        (SharedDescriptor::Delegate { target, payload }, _) => expand(
            target.clone(),
            *payload,
            right,
            right_roles,
            entries,
            active,
        ),
        (_, SharedDescriptor::Delegate { target, payload }) => {
            expand(target.clone(), *payload, left, left_roles, entries, active)
        }
        _ => match (outer(left, left_roles)?, outer(right, right_roles)?) {
            (Outer::Opaque, _) | (_, Outer::Opaque) => reason(DisjointnessReason::OpaqueForm),
            (Outer::Carrier(left), Outer::Carrier(right)) if left != right => Ok(()),
            (Outer::Carrier(_), Outer::Carrier(_)) => reason(DisjointnessReason::SharedBoundary),
            (Outer::Carrier(_), _) | (_, Outer::Carrier(_)) => Ok(()),
            (Outer::Named(left, _), Outer::Named(right, _)) => match (left, right) {
                (Some(left), Some(right)) if left != right => Ok(()),
                _ => reason(DisjointnessReason::OverlappingAtomCase),
            },
            (Outer::Literal(left), Outer::Literal(right)) if left != right => Ok(()),
            (Outer::Literal(_), Outer::Literal(_)) => reason(DisjointnessReason::SameLiteral),
            (Outer::Named(_, excluded), Outer::Literal(literal))
            | (Outer::Literal(literal), Outer::Named(_, excluded)) => {
                if excluded.contains(literal) {
                    Ok(())
                } else {
                    reason(DisjointnessReason::LiteralMayMatchNameAtom)
                }
            }
            (Outer::Application(_, _), Outer::Named(_, _))
            | (Outer::Named(_, _), Outer::Application(_, _))
            | (Outer::Application(_, _), Outer::Literal(_))
            | (Outer::Literal(_), Outer::Application(_, _))
            | (Outer::Boundary(_), Outer::Named(_, _))
            | (Outer::Named(_, _), Outer::Boundary(_))
            | (Outer::Boundary(_), Outer::Literal(_))
            | (Outer::Literal(_), Outer::Boundary(_))
            | (Outer::Application(_, _), Outer::Boundary(_))
            | (Outer::Boundary(_), Outer::Application(_, _)) => Ok(()),
            (Outer::Boundary(left), Outer::Boundary(right)) if left != right => Ok(()),
            (Outer::Boundary(_), Outer::Boundary(_)) => reason(DisjointnessReason::SharedBoundary),
            (Outer::Sequence(left), Outer::Sequence(right)) => {
                let mut first_cycle = None;
                for (left_role, right_role) in left
                    .members()
                    .iter()
                    .copied()
                    .zip(right.members().iter().copied())
                {
                    let left_member = left_roles.get(&left_role).ok_or(ProofFailure::Reason(
                        DisjointnessReason::MissingRole { role: left_role },
                    ))?;
                    let right_member = right_roles.get(&right_role).ok_or(ProofFailure::Reason(
                        DisjointnessReason::MissingRole { role: right_role },
                    ))?;
                    match prove(
                        left_member,
                        left_roles,
                        right_member,
                        right_roles,
                        entries,
                        active,
                    ) {
                        Ok(()) => return Ok(()),
                        Err(ProofFailure::Cycle(reentered)) => {
                            first_cycle.get_or_insert(reentered);
                        }
                        Err(ProofFailure::Reason(_)) => {}
                    }
                }
                match first_cycle {
                    Some(reentered) => Err(ProofFailure::Cycle(reentered)),
                    None => reason(DisjointnessReason::OpaqueForm),
                }
            }
            (
                Outer::Sequence(sequence),
                Outer::Named(_, _) | Outer::Literal(_) | Outer::Boundary(_),
            ) => prove_sequence_against_atom(
                sequence,
                left_roles,
                right,
                right_roles,
                entries,
                active,
            ),
            (
                Outer::Named(_, _) | Outer::Literal(_) | Outer::Boundary(_),
                Outer::Sequence(sequence),
            ) => prove_sequence_against_atom(
                sequence,
                right_roles,
                left,
                left_roles,
                entries,
                active,
            ),
            (Outer::Sequence(_), _) | (_, Outer::Sequence(_)) => {
                reason(DisjointnessReason::OpaqueForm)
            }
            (
                Outer::Application(left_head, left_payload),
                Outer::Application(right_head, right_payload),
            ) => match prove(
                left_head,
                left_roles,
                right_head,
                right_roles,
                entries,
                active,
            ) {
                Ok(()) => Ok(()),
                Err(ProofFailure::Cycle(reentered)) => Err(ProofFailure::Cycle(reentered)),
                Err(ProofFailure::Reason(_)) => match prove(
                    left_payload,
                    left_roles,
                    right_payload,
                    right_roles,
                    entries,
                    active,
                ) {
                    Ok(()) => Ok(()),
                    Err(ProofFailure::Cycle(reentered)) => Err(ProofFailure::Cycle(reentered)),
                    Err(ProofFailure::Reason(_)) => {
                        reason(DisjointnessReason::ApplicationPositionsNotDisjoint)
                    }
                },
            },
        },
    }
}

fn prove_sequence_against_atom<Root, Record>(
    sequence: &crate::form::OrderedSequence,
    sequence_roles: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
    atom: &SharedDescriptor<Root>,
    atom_roles: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
    entries: Option<&[StructuralEntry<Root, Record>]>,
    active: &mut BTreeSet<EncodedTypeId<Root>>,
) -> ProofResult<Root>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    let Some((first, tail)) = sequence.members().split_first() else {
        return reason(DisjointnessReason::OpaqueForm);
    };
    let first =
        sequence_roles
            .get(first)
            .ok_or(ProofFailure::Reason(DisjointnessReason::MissingRole {
                role: *first,
            }))?;
    if prove(first, sequence_roles, atom, atom_roles, entries, active).is_ok() {
        return Ok(());
    }
    if tail.iter().any(|role| {
        sequence_roles
            .get(role)
            .is_some_and(|descriptor| directly_guaranteed_nonempty(descriptor, sequence_roles))
    }) {
        return Ok(());
    }
    reason(DisjointnessReason::OpaqueForm)
}

fn directly_guaranteed_nonempty<Root>(
    descriptor: &SharedDescriptor<Root>,
    roles: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
) -> bool {
    match descriptor {
        SharedDescriptor::Declaration(_)
        | SharedDescriptor::Reference(_)
        | SharedDescriptor::DeclarationExcluding { .. }
        | SharedDescriptor::ReferenceExcluding { .. }
        | SharedDescriptor::Literal(_)
        | SharedDescriptor::Leaf(_)
        | SharedDescriptor::Application { .. }
        | SharedDescriptor::InlineApplication { .. }
        | SharedDescriptor::Delimited { .. }
        | SharedDescriptor::Carrier { .. }
        | SharedDescriptor::ItemBoundary { .. } => true,
        SharedDescriptor::Alternation(alternatives) => alternatives
            .iter()
            .all(|alternative| directly_guaranteed_nonempty(alternative, roles)),
        SharedDescriptor::Repeated {
            minimum, element, ..
        } => *minimum > 0 && directly_guaranteed_nonempty(element, roles),
        SharedDescriptor::OrderedSequence(sequence)
        | SharedDescriptor::AdjacentSequence(sequence) => sequence.members().iter().any(|role| {
            roles
                .get(role)
                .is_some_and(|member| directly_guaranteed_nonempty(member, roles))
        }),
        SharedDescriptor::OrderedProduct(product) => !product.is_empty(),
        SharedDescriptor::Delegate { .. } => false,
    }
}

fn prove_payloads<Root>(left: DelegationPayload, right: DelegationPayload) -> ProofResult<Root> {
    match (left, right) {
        (DelegationPayload::AtomCase(left), DelegationPayload::AtomCase(right))
            if left != right =>
        {
            Ok(())
        }
        _ => reason(DisjointnessReason::OverlappingAtomCase),
    }
}

fn payload_descriptor<Root>(payload: DelegationPayload) -> SharedDescriptor<Root> {
    match payload {
        DelegationPayload::AtomCase(case) => {
            SharedDescriptor::Reference(AtomDescriptor::with_case(case))
        }
    }
}

fn expand<Root, Record>(
    target: EncodedTypeId<Root>,
    payload: Option<DelegationPayload>,
    other: &SharedDescriptor<Root>,
    other_roles: &BTreeMap<StableRoleId, SharedDescriptor<Root>>,
    entries: Option<&[StructuralEntry<Root, Record>]>,
    active: &mut BTreeSet<EncodedTypeId<Root>>,
) -> ProofResult<Root>
where
    Root: Clone + Ord,
    Record: StructureRecord<Root>,
{
    let entries = entries.ok_or_else(|| {
        ProofFailure::Reason(DisjointnessReason::MissingDelegateTarget {
            target: target.clone(),
        })
    })?;
    if !active.insert(target.clone()) {
        return Err(ProofFailure::Cycle(target));
    }
    let result = (|| {
        let entry = entries
            .iter()
            .find(|entry| entry.encoded_type() == &target)
            .ok_or_else(|| {
                ProofFailure::Reason(DisjointnessReason::MissingDelegateTarget {
                    target: target.clone(),
                })
            })?;
        for codec in entry.constructors() {
            for accepted in codec.decode_forms() {
                let (root, roles) = fields(accepted.rule());
                let descriptor = roles.get(&root).ok_or(ProofFailure::Reason(
                    DisjointnessReason::MissingRole { role: root },
                ))?;
                if let Some(payload) = payload {
                    let restriction = payload_descriptor(payload);
                    match prove(
                        &restriction,
                        &BTreeMap::new(),
                        other,
                        other_roles,
                        Some(entries),
                        active,
                    ) {
                        Ok(()) => continue,
                        Err(ProofFailure::Cycle(reentered)) => {
                            return Err(ProofFailure::Cycle(reentered));
                        }
                        Err(ProofFailure::Reason(_)) => {}
                    }
                    match prove(
                        descriptor,
                        &roles,
                        &restriction,
                        &BTreeMap::new(),
                        Some(entries),
                        active,
                    ) {
                        Ok(()) => continue,
                        Err(ProofFailure::Cycle(reentered)) => {
                            return Err(ProofFailure::Cycle(reentered));
                        }
                        Err(ProofFailure::Reason(_)) => {}
                    }
                }
                prove(
                    descriptor,
                    &roles,
                    other,
                    other_roles,
                    Some(entries),
                    active,
                )?;
            }
        }
        Ok(())
    })();
    active.remove(&target);
    result
}
