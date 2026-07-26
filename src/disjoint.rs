//! Conservative disjointness proof over shared descriptors and typed positions.
//!
//! The proof is deliberately stricter than the evaluator: an alternative pair
//! seals only when a structural distinction is proved. In particular, delegate
//! payload constraints participate in the proof and an active delegate
//! expansion is a typed cycle failure, never a missing target.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::StructuralEntry;
use crate::error::{DisjointnessError, DisjointnessReason};
use crate::form::{
    AtomDescriptor, BorrowedFieldView, DelegationPayload, FieldVisitor, Position, SharedDescriptor,
    StructureRecord,
};
use crate::ids::{FieldRole, ScopedEncodedTypeId, StableRoleId};

struct Collector {
    fields: BTreeMap<StableRoleId, SharedDescriptor>,
}

impl FieldVisitor for Collector {
    fn field<Role: FieldRole>(&mut self, position: &Position<Role>) {
        self.fields
            .insert(position.role(), position.descriptor().clone());
    }
}

fn fields<Record: StructureRecord>(
    rule: &Record,
) -> (StableRoleId, BTreeMap<StableRoleId, SharedDescriptor>) {
    let mut collector = Collector {
        fields: BTreeMap::new(),
    };
    rule.fields().expose(&mut collector);
    (rule.root_role(), collector.fields)
}

enum Outer<'a> {
    Atom(Option<raw_discovery::AtomCase>),
    Literal(name_table::Identifier),
    Application(&'a SharedDescriptor, &'a SharedDescriptor),
    Boundary(raw_discovery::TriggerIdentifier),
    Opaque,
}

enum ProofFailure {
    Reason(DisjointnessReason),
    Cycle(ScopedEncodedTypeId),
}

type ProofResult = Result<(), ProofFailure>;

fn reason<T>(reason: DisjointnessReason) -> Result<T, ProofFailure> {
    Err(ProofFailure::Reason(reason))
}

fn outer<'a>(
    descriptor: &'a SharedDescriptor,
    roles: &'a BTreeMap<StableRoleId, SharedDescriptor>,
) -> Result<Outer<'a>, ProofFailure> {
    match descriptor {
        SharedDescriptor::Atom(atom) => Ok(Outer::Atom(atom.case)),
        SharedDescriptor::Literal(identifier) => Ok(Outer::Literal(*identifier)),
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
        SharedDescriptor::Delimited { boundary, .. }
        | SharedDescriptor::ItemBoundary { boundary, .. } => Ok(Outer::Boundary(*boundary)),
        SharedDescriptor::Leaf(_) | SharedDescriptor::Repeated { .. } => Ok(Outer::Opaque),
        SharedDescriptor::Delegate { .. } => unreachable!("delegates expand before outer proof"),
    }
}

impl<Record: StructureRecord> StructuralEntry<Record> {
    pub fn validate_disjoint(&self) -> Result<(), DisjointnessError> {
        self.validate_disjoint_against(None)
    }

    pub(crate) fn validate_disjoint_with(
        &self,
        entries: &BTreeMap<ScopedEncodedTypeId, StructuralEntry<Record>>,
    ) -> Result<(), DisjointnessError> {
        self.validate_disjoint_against(Some(entries))
    }

    fn validate_disjoint_against(
        &self,
        entries: Option<&BTreeMap<ScopedEncodedTypeId, StructuralEntry<Record>>>,
    ) -> Result<(), DisjointnessError> {
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
                            core_type: self.encoded_type(),
                            first: *left_constructor,
                            second: *right_constructor,
                            reason,
                        });
                    }
                    Err(ProofFailure::Cycle(reentered)) => {
                        return Err(DisjointnessError::DelegateExpansionCycle {
                            core_type: self.encoded_type(),
                            reentered,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

fn prove_rules<Record: StructureRecord>(
    left: &Record,
    right: &Record,
    entries: Option<&BTreeMap<ScopedEncodedTypeId, StructuralEntry<Record>>>,
    active: &mut BTreeSet<ScopedEncodedTypeId>,
) -> ProofResult {
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

fn prove<Record: StructureRecord>(
    left: &SharedDescriptor,
    left_roles: &BTreeMap<StableRoleId, SharedDescriptor>,
    right: &SharedDescriptor,
    right_roles: &BTreeMap<StableRoleId, SharedDescriptor>,
    entries: Option<&BTreeMap<ScopedEncodedTypeId, StructuralEntry<Record>>>,
    active: &mut BTreeSet<ScopedEncodedTypeId>,
) -> ProofResult {
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
        (SharedDescriptor::Delegate { target, payload }, _) => {
            expand(*target, *payload, right, right_roles, entries, active)
        }
        (_, SharedDescriptor::Delegate { target, payload }) => {
            expand(*target, *payload, left, left_roles, entries, active)
        }
        _ => match (outer(left, left_roles)?, outer(right, right_roles)?) {
            (Outer::Opaque, _) | (_, Outer::Opaque) => reason(DisjointnessReason::OpaqueForm),
            (Outer::Atom(left), Outer::Atom(right)) => match (left, right) {
                (Some(left), Some(right)) if left != right => Ok(()),
                _ => reason(DisjointnessReason::OverlappingAtomCase),
            },
            (Outer::Literal(left), Outer::Literal(right)) if left != right => Ok(()),
            (Outer::Literal(_), Outer::Literal(_)) => reason(DisjointnessReason::SameLiteral),
            (Outer::Atom(_), Outer::Literal(_)) | (Outer::Literal(_), Outer::Atom(_)) => {
                reason(DisjointnessReason::LiteralMayMatchNameAtom)
            }
            (Outer::Application(_, _), Outer::Atom(_))
            | (Outer::Atom(_), Outer::Application(_, _))
            | (Outer::Application(_, _), Outer::Literal(_))
            | (Outer::Literal(_), Outer::Application(_, _))
            | (Outer::Boundary(_), Outer::Atom(_))
            | (Outer::Atom(_), Outer::Boundary(_))
            | (Outer::Boundary(_), Outer::Literal(_))
            | (Outer::Literal(_), Outer::Boundary(_))
            | (Outer::Application(_, _), Outer::Boundary(_))
            | (Outer::Boundary(_), Outer::Application(_, _)) => Ok(()),
            (Outer::Boundary(left), Outer::Boundary(right)) if left != right => Ok(()),
            (Outer::Boundary(_), Outer::Boundary(_)) => reason(DisjointnessReason::SharedBoundary),
            (
                Outer::Application(left_head, left_payload),
                Outer::Application(right_head, right_payload),
            ) => {
                match prove(
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
                }
            }
        },
    }
}

fn prove_payloads(left: DelegationPayload, right: DelegationPayload) -> ProofResult {
    match (left, right) {
        (DelegationPayload::AtomCase(left), DelegationPayload::AtomCase(right))
            if left != right =>
        {
            Ok(())
        }
        _ => reason(DisjointnessReason::OverlappingAtomCase),
    }
}

fn payload_descriptor(payload: DelegationPayload) -> SharedDescriptor {
    match payload {
        DelegationPayload::AtomCase(case) => {
            SharedDescriptor::Atom(AtomDescriptor::with_case(case))
        }
    }
}

fn expand<Record: StructureRecord>(
    target: ScopedEncodedTypeId,
    payload: Option<DelegationPayload>,
    other: &SharedDescriptor,
    other_roles: &BTreeMap<StableRoleId, SharedDescriptor>,
    entries: Option<&BTreeMap<ScopedEncodedTypeId, StructuralEntry<Record>>>,
    active: &mut BTreeSet<ScopedEncodedTypeId>,
) -> ProofResult {
    let entries = entries.ok_or(ProofFailure::Reason(
        DisjointnessReason::MissingDelegateTarget { target },
    ))?;
    if !active.insert(target) {
        return Err(ProofFailure::Cycle(target));
    }
    let result = (|| {
        let entry = entries.get(&target).ok_or(ProofFailure::Reason(
            DisjointnessReason::MissingDelegateTarget { target },
        ))?;
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
