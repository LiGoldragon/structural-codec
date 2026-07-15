//! Conservative outer-shape disjointness validation. This is the lineage of nota's
//! `validate_no_silent_conflicts`, but INVERTED to the conservative-safe direction the
//! design demands: nota permits by default and rejects only demonstrable shadows;
//! here a pair of accepted decode forms is accepted ONLY when it can be PROVEN that no
//! raw block could match both. Overlap the checker cannot rule out is a validation
//! ERROR, so a constructor can never silently swallow another's inputs.

use crate::codec::StructuralEntry;
use crate::error::DisjointnessError;
use crate::form::StructuralForm;

/// The discriminating outer shape of a form — the block kind it can match. Forms
/// whose matchable kind cannot be pinned (delegates, leaves, products) are `Opaque`
/// and never prove disjoint against anything.
enum OuterShape<'form> {
    /// Matches a `Block::Atom` constrained by case (`None` = any case).
    NameAtom(Option<crate::form::CaseExpectation>),
    /// Matches a specific interned atom.
    Literal(name_table::Identifier),
    /// Matches a `Block::Application`.
    Application(&'form StructuralForm, &'form StructuralForm),
    /// Matches a `Block::Delimited` of a given delimiter.
    Delimited(raw_discovery::Delimiter),
    /// Matchable kind cannot be pinned — conservatively overlaps everything.
    Opaque,
}

impl StructuralForm {
    fn outer_shape(&self) -> OuterShape<'_> {
        match self {
            Self::Atom(atom) => OuterShape::NameAtom(atom.case),
            Self::Literal(identifier) => OuterShape::Literal(*identifier),
            Self::Application { head, payload } => OuterShape::Application(head, payload),
            Self::Delimited { delimiter, .. } => OuterShape::Delimited(*delimiter),
            Self::Leaf(_) | Self::Delegate(_) | Self::Product(_) => OuterShape::Opaque,
        }
    }

    /// `Ok(())` when it is PROVEN that no raw block matches both forms; `Err(reason)`
    /// when disjointness cannot be established (conservatively an overlap).
    fn prove_disjoint_from(&self, other: &StructuralForm) -> Result<(), &'static str> {
        match (self.outer_shape(), other.outer_shape()) {
            (OuterShape::Opaque, _) | (_, OuterShape::Opaque) => {
                Err("a delegate, leaf, or product form has no pinned block kind")
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
            (OuterShape::NameAtom(left), OuterShape::NameAtom(right)) => match (left, right) {
                (Some(left_case), Some(right_case)) if left_case != right_case => Ok(()),
                _ => Err("both forms accept an overlapping atom case"),
            },

            // Two literals are disjoint only when they name different keywords.
            (OuterShape::Literal(left), OuterShape::Literal(right)) => {
                if left == right {
                    Err("both forms require the same interned literal")
                } else {
                    Ok(())
                }
            }

            // A literal atom's text may satisfy a name atom's case; without the
            // resolver we cannot rule it out, so this stays conservatively unprovable.
            (OuterShape::NameAtom(_), OuterShape::Literal(_))
            | (OuterShape::Literal(_), OuterShape::NameAtom(_)) => {
                Err("a literal atom might satisfy the name atom's case constraint")
            }

            // Applications are disjoint if EITHER position is provably disjoint.
            (
                OuterShape::Application(left_head, left_payload),
                OuterShape::Application(right_head, right_payload),
            ) => {
                if left_head.prove_disjoint_from(right_head).is_ok()
                    || left_payload.prove_disjoint_from(right_payload).is_ok()
                {
                    Ok(())
                } else {
                    Err("neither the application head nor payload is provably disjoint")
                }
            }

            // Delimited forms are disjoint only when their delimiters differ; a shared
            // delimiter would need a proof over the inner sequence, which we do not
            // attempt (conservatively an overlap).
            (OuterShape::Delimited(left), OuterShape::Delimited(right)) => {
                if left == right {
                    Err("both forms use the same delimiter")
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl StructuralEntry {
    /// Validate that every accepted decode form across ALL constructors of this entry
    /// is pairwise provably disjoint. Any unprovable pair is a hard error.
    pub fn validate_disjoint(&self) -> Result<(), DisjointnessError> {
        let forms: Vec<&StructuralForm> = self
            .constructors
            .iter()
            .flat_map(|codec| codec.decode_forms.iter())
            .collect();

        for (first, left) in forms.iter().enumerate() {
            for (offset, right) in forms.iter().enumerate().skip(first + 1) {
                if let Err(reason) = left.prove_disjoint_from(right) {
                    return Err(DisjointnessError::NotProvablyDisjoint {
                        core_type: self.core_type,
                        first,
                        second: offset,
                        reason,
                    });
                }
            }
        }
        Ok(())
    }
}
