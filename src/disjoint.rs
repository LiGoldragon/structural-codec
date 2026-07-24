//! Conservative outer-shape disjointness validation. This is the lineage of nota's
//! `validate_no_silent_conflicts`, but INVERTED to the conservative-safe direction the
//! design demands: nota permits by default and rejects only demonstrable shadows;
//! here a pair of accepted decode forms is accepted ONLY when it can be PROVEN that no
//! raw block could match both. Overlap the checker cannot rule out is a validation
//! ERROR, so a constructor can never silently swallow another's inputs.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::StructuralEntry;
use crate::error::{DisjointnessError, DisjointnessReason};
use crate::form::StructuralForm;
use crate::ids::ScopedEncodedTypeId;

/// The discriminating outer shape of a form — the block kind it can match. Forms
/// whose matchable kind cannot be pinned (delegates, leaves, products) are `Opaque`
/// and never prove disjoint against anything.
enum OuterShape<'form> {
    /// Matches a `Block::Atom` constrained by case (`None` = any case).
    NameAtom(Option<raw_discovery::AtomCase>),
    /// Matches a specific interned atom.
    Literal(name_table::Identifier),
    /// Matches a `Block::Application`.
    Application(&'form StructuralForm, &'form StructuralForm),
    /// Matches the opening of one profile-addressed boundary. The boundary
    /// identifier, not a legacy rendered delimiter enum, is what distinguishes
    /// direct textual alternatives.
    Delimited(raw_discovery::TriggerIdentifier),
    /// Matchable kind cannot be pinned — conservatively overlaps everything.
    Opaque,
}

/// The active direct-delegate expansions for one proof obligation. This is a
/// structural state (the delegated type identities), not a depth counter: re-entering
/// an active expansion means the proof remains unresolved at the same raw block.
#[derive(Default)]
struct DelegateProofState {
    active_expansions: BTreeSet<ScopedEncodedTypeId>,
}

impl StructuralForm {
    fn outer_shape(&self) -> OuterShape<'_> {
        match self {
            Self::Atom(atom) => OuterShape::NameAtom(atom.case),
            Self::Literal(identifier) => OuterShape::Literal(*identifier),
            Self::Application { head, payload, .. } => OuterShape::Application(head, payload),
            Self::Delimited { boundary, .. } => OuterShape::Delimited(*boundary),
            Self::Leaf(_) | Self::Delegate { .. } => OuterShape::Opaque,
        }
    }

    /// `Ok(())` when it is PROVEN that no raw block matches both forms; `Err(reason)`
    /// when disjointness cannot be established (conservatively an overlap). At a
    /// table seal, a delegate expands to every direct decode form in its target entry.
    /// A standalone entry has no such table context, so delegates remain opaque.
    fn prove_disjoint_from(
        &self,
        other: &StructuralForm,
        entries: Option<&BTreeMap<ScopedEncodedTypeId, StructuralEntry>>,
        state: &mut DelegateProofState,
    ) -> Result<(), ProofFailure> {
        // Two directed positions can prove disjoint from their sealed payloads
        // alone, before either transparent target needs expansion. This keeps a
        // useful direction from being mistaken for an unguarded delegate cycle.
        if let (
            StructuralForm::Delegate {
                payload: Some(left_payload),
                ..
            },
            StructuralForm::Delegate {
                payload: Some(right_payload),
                ..
            },
        ) = (self, other)
        {
            let left_constraint = left_payload.constraint_form();
            let right_constraint = right_payload.constraint_form();
            if left_constraint
                .prove_disjoint_from(&right_constraint, entries, state)
                .is_ok()
            {
                return Ok(());
            }
        }

        if let StructuralForm::Delegate { target, payload } = self {
            let entry = entries
                .and_then(|entries| entries.get(target))
                .ok_or(DisjointnessReason::MissingDelegateTarget { target: *target })?;
            if !state.active_expansions.insert(*target) {
                return Err(ProofFailure::DelegateExpansionCycle { reentered: *target });
            }
            let proof = entry
                .constructors
                .iter()
                .flat_map(|codec| &codec.decode_forms)
                .try_for_each(|form| {
                    Self::prove_directed_delegate_form_disjoint(
                        *payload, form, other, entries, state,
                    )
                });
            state.active_expansions.remove(target);
            return proof;
        }
        if matches!(other, StructuralForm::Delegate { .. }) {
            return other.prove_disjoint_from(self, entries, state);
        }

        match (self.outer_shape(), other.outer_shape()) {
            (OuterShape::Opaque, _) | (_, OuterShape::Opaque) => {
                Err(DisjointnessReason::OpaqueForm.into())
            }

            // Different block kinds are mutually exclusive: a block is exactly one of
            // atom / application / delimited.
            (OuterShape::NameAtom(_) | OuterShape::Literal(_), OuterShape::Application(_, _))
            | (OuterShape::Application(_, _), OuterShape::NameAtom(_) | OuterShape::Literal(_)) => {
                Ok(())
            }
            (OuterShape::NameAtom(_) | OuterShape::Literal(_), OuterShape::Delimited(_))
            | (OuterShape::Delimited(_), OuterShape::NameAtom(_) | OuterShape::Literal(_)) => {
                Ok(())
            }
            (OuterShape::Application(_, _), OuterShape::Delimited(_))
            | (OuterShape::Delimited(_), OuterShape::Application(_, _)) => Ok(()),

            // Two case-constrained name atoms are disjoint only when both cases are
            // concrete and different; a `None` case accepts every atom.
            (OuterShape::NameAtom(left_case), OuterShape::NameAtom(right_case)) => {
                match (left_case, right_case) {
                    (Some(left_case), Some(right_case)) if left_case != right_case => Ok(()),
                    _ => Err(DisjointnessReason::OverlappingAtomCase.into()),
                }
            }

            // Two literals are disjoint only when they name different keywords.
            (OuterShape::Literal(left), OuterShape::Literal(right)) => {
                if left == right {
                    Err(DisjointnessReason::SameLiteral.into())
                } else {
                    Ok(())
                }
            }

            // A literal may satisfy any unconstrained name atom. Grammar positions
            // must not mix the two categories; no lexical exclusion can repair that.
            (OuterShape::NameAtom(_), OuterShape::Literal(_))
            | (OuterShape::Literal(_), OuterShape::NameAtom(_)) => {
                Err(DisjointnessReason::LiteralMayMatchNameAtom.into())
            }

            // Applications are disjoint if EITHER position is provably disjoint. A
            // guarded recursive payload is never expanded when the heads already
            // separate the alternatives.
            (
                OuterShape::Application(left_head, left_payload),
                OuterShape::Application(right_head, right_payload),
            ) => {
                let head_proof = left_head.prove_disjoint_from(right_head, entries, state);
                if head_proof.is_ok() {
                    return Ok(());
                }
                let payload_proof = left_payload.prove_disjoint_from(right_payload, entries, state);
                if payload_proof.is_ok() {
                    return Ok(());
                }
                match (head_proof, payload_proof) {
                    (Err(cycle @ ProofFailure::DelegateExpansionCycle { .. }), _)
                    | (_, Err(cycle @ ProofFailure::DelegateExpansionCycle { .. })) => Err(cycle),
                    _ => Err(DisjointnessReason::ApplicationPositionsNotDisjoint.into()),
                }
            }

            // Delimited forms are disjoint only when they activate distinct sealed
            // boundary definitions. A shared boundary would need a proof over the
            // inner sequence, which we do not attempt (conservatively an overlap).
            (OuterShape::Delimited(left), OuterShape::Delimited(right)) => {
                if left == right {
                    Err(DisjointnessReason::SharedDelimiter.into())
                } else {
                    Ok(())
                }
            }
        }
    }

    /// A directed delegation accepts the intersection of its target form and the
    /// payload constraint. That intersection is disjoint from `other` when either
    /// operand is disjoint from `other`, or when the payload itself excludes the
    /// target form. Every branch is structural; no decode order participates.
    fn prove_directed_delegate_form_disjoint(
        payload: Option<crate::form::DelegationPayload>,
        target_form: &StructuralForm,
        other: &StructuralForm,
        entries: Option<&BTreeMap<ScopedEncodedTypeId, StructuralEntry>>,
        state: &mut DelegateProofState,
    ) -> Result<(), ProofFailure> {
        let Some(payload) = payload else {
            return target_form.prove_disjoint_from(other, entries, state);
        };
        let constraint = payload.constraint_form();
        let payload_target_proof = constraint.prove_disjoint_from(target_form, entries, state);
        if payload_target_proof.is_ok() {
            return Ok(());
        }
        let payload_other_proof = constraint.prove_disjoint_from(other, entries, state);
        if payload_other_proof.is_ok() {
            return Ok(());
        }
        let target_other_proof = target_form.prove_disjoint_from(other, entries, state);
        if target_other_proof.is_ok() {
            return Ok(());
        }
        match (
            payload_target_proof,
            payload_other_proof,
            target_other_proof,
        ) {
            (Ok(()), _, _) | (_, Ok(()), _) | (_, _, Ok(())) => Ok(()),
            (Err(cycle @ ProofFailure::DelegateExpansionCycle { .. }), _, _)
            | (_, Err(cycle @ ProofFailure::DelegateExpansionCycle { .. }), _)
            | (_, _, Err(cycle @ ProofFailure::DelegateExpansionCycle { .. })) => Err(cycle),
            (_, _, Err(failure)) => Err(failure),
        }
    }
}

/// A proof failure is either an ordinary conservative refusal or a direct-delegate
/// cycle, which must be surfaced separately at the public seal boundary.
enum ProofFailure {
    Disjointness(DisjointnessReason),
    DelegateExpansionCycle { reentered: ScopedEncodedTypeId },
}

impl From<DisjointnessReason> for ProofFailure {
    fn from(reason: DisjointnessReason) -> Self {
        Self::Disjointness(reason)
    }
}

impl StructuralEntry {
    /// Validate that every accepted decode form across ALL constructors of this entry
    /// is pairwise provably disjoint. Without a complete table, delegates are opaque.
    pub fn validate_disjoint(&self) -> Result<(), DisjointnessError> {
        self.validate_disjoint_against(None)
    }

    /// Validate this entry against the complete table under construction. Delegates
    /// expand to their target entries' direct decode forms, preserving evaluator
    /// wrapper semantics while allowing the seal proof to inspect their shape.
    pub(crate) fn validate_disjoint_with(
        &self,
        entries: &BTreeMap<ScopedEncodedTypeId, StructuralEntry>,
    ) -> Result<(), DisjointnessError> {
        self.validate_disjoint_against(Some(entries))
    }

    fn validate_disjoint_against(
        &self,
        entries: Option<&BTreeMap<ScopedEncodedTypeId, StructuralEntry>>,
    ) -> Result<(), DisjointnessError> {
        let forms: Vec<&StructuralForm> = self
            .constructors
            .iter()
            .flat_map(|codec| codec.decode_forms.iter())
            .collect();

        for (first, left) in forms.iter().enumerate() {
            for (second, right) in forms.iter().enumerate().skip(first + 1) {
                let mut state = DelegateProofState::default();
                if let Err(failure) = left.prove_disjoint_from(right, entries, &mut state) {
                    return Err(match failure {
                        ProofFailure::Disjointness(reason) => {
                            DisjointnessError::NotProvablyDisjoint {
                                core_type: self.core_type,
                                first,
                                second,
                                reason,
                            }
                        }
                        ProofFailure::DelegateExpansionCycle { reentered } => {
                            DisjointnessError::DelegateExpansionCycle {
                                core_type: self.core_type,
                                first,
                                second,
                                reentered,
                            }
                        }
                    });
                }
            }
        }
        Ok(())
    }
}
