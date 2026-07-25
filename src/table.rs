//! The addressed structural table: the external sidecar keyed by `ScopedEncodedTypeId`.
//! Its content identity is computed over `TableIdentityPayload` and STORED OUTSIDE
//! that payload (fixing the self-reference bug), and is EXCLUDED from Core value
//! identity by construction — Core hashing never sees the table. Old table decodes
//! old text, a new table encodes new text, and both reach the same Core value (§4.6).

use std::collections::{BTreeMap, BTreeSet};

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use raw_discovery::{
    SealedTokenProfile, TokenProfileError, Trigger, TriggerIdentifier, TriggerSet,
};

use crate::codec::StructuralEntry;
use crate::error::{DisjointnessError, TableError};
use crate::form::{SequenceForm, StructuralForm};
use crate::ids::{EncodedUniverseId, ScopedEncodedTypeId};

/// The identity of a Core layout the forms target (supplied by the Core side).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct EncodedLayoutIdentity(pub [u8; 32]);

/// The identity of the separately sealed token profile.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct RawProfileIdentity(pub [u8; 32]);

impl RawProfileIdentity {
    pub fn from_profile(profile: &SealedTokenProfile) -> Self {
        Self(*profile.identity().bytes())
    }

    pub fn matches(self, profile: &SealedTokenProfile) -> bool {
        self == Self::from_profile(profile)
    }
}

/// The identity of a leaf codec's contract.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct LeafCodecContractId(pub u32);

/// The table-identity pre-image. The resulting hash is stored on
/// `AddressedStructuralTable`, NEVER inside here.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TableIdentityPayload {
    pub core_universe: EncodedUniverseId,
    pub core_layout_identity: EncodedLayoutIdentity,
    pub raw_profile_identity: RawProfileIdentity,
    /// Trivia triggers active at every textual structural position.
    pub trivia_triggers: TriggerSet,
    pub leaf_codec_contracts: Vec<LeafCodecContractId>,
    pub entries: BTreeMap<ScopedEncodedTypeId, StructuralEntry>,
}

/// The hash domain for structural tables, layout-version tagged.
pub struct StructuralTableDomain;

impl HashDomain for StructuralTableDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "structural-codec 2026 addressed structural table",
            // Layout 6 binds recursive form positions to profile triggers and
            // carries the universal trivia set in the table pre-image.
            layout: LayoutVersion::new(6),
        }
    }
}

/// A sealed structural table with its identity stored outside the hashed payload.
#[derive(Clone, Debug)]
pub struct AddressedStructuralTable {
    payload: TableIdentityPayload,
    identity: ContentHash<StructuralTableDomain>,
}

impl AddressedStructuralTable {
    /// Prove every decode form disjoint, then compute the table identity over its
    /// complete payload and store that identity outside its pre-image.
    pub fn seal(
        payload: TableIdentityPayload,
        profile: &SealedTokenProfile,
    ) -> Result<Self, TableError> {
        if !payload.raw_profile_identity.matches(profile) {
            return Err(TableError::TokenProfileIdentityMismatch);
        }
        for entry in payload.entries.values() {
            entry.validate_disjoint_with(&payload.entries)?;
        }
        Self::validate_lexical_positions(&payload, profile)?;
        let identity = ContentHash::of_core(&payload)?;
        Ok(Self { payload, identity })
    }

    /// The table's content identity — co-versioned with the language package,
    /// EXCLUDED from Core value identity.
    pub fn identity(&self) -> ContentHash<StructuralTableDomain> {
        self.identity
    }

    /// Queried BY expected type, never globally searched; the input never selects its
    /// own type.
    pub fn entry(&self, expected: ScopedEncodedTypeId) -> Option<&StructuralEntry> {
        self.payload.entries.get(&expected)
    }

    pub fn raw_profile_identity(&self) -> RawProfileIdentity {
        self.payload.raw_profile_identity
    }

    pub fn trivia_triggers(&self) -> &TriggerSet {
        &self.payload.trivia_triggers
    }

    /// Validate conservative disjointness across every entry.
    pub fn validate_disjoint(&self) -> Result<(), DisjointnessError> {
        for entry in self.payload.entries.values() {
            entry.validate_disjoint_with(&self.payload.entries)?;
        }
        Ok(())
    }

    fn validate_lexical_positions(
        payload: &TableIdentityPayload,
        profile: &SealedTokenProfile,
    ) -> Result<(), TableError> {
        for identifier in payload.trivia_triggers.triggers() {
            Self::require_trigger_kind(profile, *identifier, "trivia", |kind| {
                matches!(
                    kind,
                    Trigger::Whitespace { .. } | Trigger::LineComment { .. }
                )
            })?;
        }
        profile.seal_trigger_set(payload.trivia_triggers.clone())?;
        for entry in payload.entries.values() {
            let initial =
                Self::initial_type_triggers(entry.core_type, payload, &mut BTreeSet::new())?;
            Self::seal_position(profile, &payload.trivia_triggers, &initial, None)?;
            for codec in &entry.constructors {
                for form in codec
                    .decode_forms
                    .iter()
                    .chain(std::iter::once(&codec.encode_form))
                {
                    let mut delegate_path = BTreeSet::new();
                    Self::validate_form_position(form, &[], payload, profile, &mut delegate_path)?;
                }
            }
        }
        Ok(())
    }

    fn validate_form_position(
        form: &StructuralForm,
        inherited: &[TriggerIdentifier],
        payload: &TableIdentityPayload,
        profile: &SealedTokenProfile,
        delegate_path: &mut BTreeSet<(ScopedEncodedTypeId, Vec<TriggerIdentifier>)>,
    ) -> Result<(), TableError> {
        match form {
            StructuralForm::Atom(atom) => {
                if let Some(trigger) = atom.trigger {
                    Self::require_trigger_kind(
                        profile,
                        trigger,
                        "leading character class",
                        |kind| matches!(kind, Trigger::LeadingCharacterClass { .. }),
                    )?;
                }
                Self::seal_position(profile, &payload.trivia_triggers, inherited, atom.trigger)?;
            }
            StructuralForm::Leaf(leaf) => {
                if let Some(trigger) = leaf.trigger {
                    Self::require_trigger_kind(
                        profile,
                        trigger,
                        "leaf carrier or class",
                        |kind| {
                            matches!(
                                kind,
                                Trigger::Carrier { .. } | Trigger::LeadingCharacterClass { .. }
                            )
                        },
                    )?;
                }
                Self::seal_position(profile, &payload.trivia_triggers, inherited, leaf.trigger)?;
            }
            StructuralForm::Literal(_) => {
                Self::seal_position(profile, &payload.trivia_triggers, inherited, None)?;
            }
            StructuralForm::Application {
                operator,
                head,
                payload: application_payload,
            } => {
                Self::require_trigger_kind(
                    profile,
                    *operator,
                    "application or punctuation",
                    |kind| {
                        matches!(
                            kind,
                            Trigger::Application { .. } | Trigger::Punctuation { .. }
                        )
                    },
                )?;
                let mut head_terminators = inherited.to_vec();
                head_terminators.push(*operator);
                Self::validate_form_position(
                    head,
                    &head_terminators,
                    payload,
                    profile,
                    delegate_path,
                )?;
                Self::validate_form_position(
                    application_payload,
                    inherited,
                    payload,
                    profile,
                    delegate_path,
                )?;
            }
            StructuralForm::Delimited {
                boundary, sequence, ..
            } => {
                Self::require_trigger_kind(profile, *boundary, "boundary", |kind| {
                    matches!(kind, Trigger::Boundary { .. })
                })?;
                Self::seal_position(
                    profile,
                    &payload.trivia_triggers,
                    inherited,
                    Some(*boundary),
                )?;
                let discovery =
                    Self::boundary_discovery_triggers(sequence, *boundary, payload, profile)?;
                profile.seal_boundary_discovery_set(discovery)?;
                match sequence {
                    SequenceForm::Product(forms) => {
                        for child in forms {
                            Self::validate_form_position(
                                child,
                                &[],
                                payload,
                                profile,
                                delegate_path,
                            )?;
                        }
                    }
                    SequenceForm::Repeat { element, .. } => {
                        Self::validate_form_position(element, &[], payload, profile, delegate_path)?
                    }
                }
            }
            StructuralForm::Delegate { target, .. } => {
                let initial = Self::initial_type_triggers(*target, payload, &mut BTreeSet::new())?;
                let mut active_initial = inherited.to_vec();
                active_initial.extend(initial);
                Self::seal_position(profile, &payload.trivia_triggers, &active_initial, None)?;
                let key = (*target, inherited.to_vec());
                if !delegate_path.insert(key.clone()) {
                    return Ok(());
                }
                let entry = payload
                    .entries
                    .get(target)
                    .ok_or(TableError::MissingLexicalDelegateTarget { target: *target })?;
                for codec in &entry.constructors {
                    for target_form in codec
                        .decode_forms
                        .iter()
                        .chain(std::iter::once(&codec.encode_form))
                    {
                        Self::validate_form_position(
                            target_form,
                            inherited,
                            payload,
                            profile,
                            delegate_path,
                        )?;
                    }
                }
                delegate_path.remove(&key);
            }
        }
        Ok(())
    }

    fn boundary_discovery_triggers(
        sequence: &SequenceForm,
        boundary: TriggerIdentifier,
        payload: &TableIdentityPayload,
        profile: &SealedTokenProfile,
    ) -> Result<TriggerSet, TableError> {
        let mut triggers = payload.trivia_triggers.triggers().to_vec();
        triggers.push(boundary);
        Self::boundary_sequence_triggers(
            sequence,
            payload,
            profile,
            &mut BTreeSet::new(),
            &mut triggers,
        )?;
        Ok(TriggerSet::new(triggers))
    }

    fn boundary_sequence_triggers(
        sequence: &SequenceForm,
        payload: &TableIdentityPayload,
        profile: &SealedTokenProfile,
        path: &mut BTreeSet<ScopedEncodedTypeId>,
        triggers: &mut Vec<TriggerIdentifier>,
    ) -> Result<(), TableError> {
        match sequence {
            SequenceForm::Product(forms) => {
                for form in forms {
                    Self::boundary_form_triggers(form, payload, profile, path, triggers)?;
                }
            }
            SequenceForm::Repeat { element, .. } => {
                Self::boundary_form_triggers(element, payload, profile, path, triggers)?;
            }
        }
        Ok(())
    }

    fn boundary_form_triggers(
        form: &StructuralForm,
        payload: &TableIdentityPayload,
        profile: &SealedTokenProfile,
        path: &mut BTreeSet<ScopedEncodedTypeId>,
        triggers: &mut Vec<TriggerIdentifier>,
    ) -> Result<(), TableError> {
        match form {
            StructuralForm::Atom(_) | StructuralForm::Literal(_) => {}
            StructuralForm::Leaf(leaf) => {
                if let Some(identifier) = leaf.trigger
                    && matches!(
                        profile.definition(identifier)?.trigger,
                        Trigger::Carrier { .. }
                    )
                {
                    triggers.push(identifier);
                }
            }
            StructuralForm::Application {
                head,
                payload: application_payload,
                ..
            } => {
                Self::boundary_form_triggers(head, payload, profile, path, triggers)?;
                Self::boundary_form_triggers(
                    application_payload,
                    payload,
                    profile,
                    path,
                    triggers,
                )?;
            }
            StructuralForm::Delimited {
                boundary, sequence, ..
            } => {
                triggers.push(*boundary);
                Self::boundary_sequence_triggers(sequence, payload, profile, path, triggers)?;
            }
            StructuralForm::Delegate { target, .. } => {
                if !path.insert(*target) {
                    return Ok(());
                }
                let entry = payload
                    .entries
                    .get(target)
                    .ok_or(TableError::MissingLexicalDelegateTarget { target: *target })?;
                for target_form in entry.constructors.iter().flat_map(|codec| {
                    codec
                        .decode_forms
                        .iter()
                        .chain(std::iter::once(&codec.encode_form))
                }) {
                    Self::boundary_form_triggers(target_form, payload, profile, path, triggers)?;
                }
                path.remove(target);
            }
        }
        Ok(())
    }

    fn initial_type_triggers(
        expected: ScopedEncodedTypeId,
        payload: &TableIdentityPayload,
        path: &mut BTreeSet<ScopedEncodedTypeId>,
    ) -> Result<Vec<TriggerIdentifier>, TableError> {
        if !path.insert(expected) {
            return Ok(Vec::new());
        }
        let entry = payload
            .entries
            .get(&expected)
            .ok_or(TableError::MissingLexicalDelegateTarget { target: expected })?;
        let mut triggers = Vec::new();
        for form in entry
            .constructors
            .iter()
            .flat_map(|codec| codec.decode_forms.iter())
        {
            Self::initial_form_triggers(form, payload, path, &mut triggers)?;
        }
        path.remove(&expected);
        triggers.sort_unstable();
        triggers.dedup();
        Ok(triggers)
    }

    fn initial_form_triggers(
        form: &StructuralForm,
        payload: &TableIdentityPayload,
        path: &mut BTreeSet<ScopedEncodedTypeId>,
        triggers: &mut Vec<TriggerIdentifier>,
    ) -> Result<(), TableError> {
        match form {
            StructuralForm::Atom(atom) => triggers.extend(atom.trigger),
            StructuralForm::Leaf(leaf) => triggers.extend(leaf.trigger),
            StructuralForm::Literal(_) => {}
            StructuralForm::Application { operator, head, .. } => {
                triggers.push(*operator);
                Self::initial_form_triggers(head, payload, path, triggers)?;
            }
            StructuralForm::Delimited { boundary, .. } => triggers.push(*boundary),
            StructuralForm::Delegate { target, .. } => {
                triggers.extend(Self::initial_type_triggers(*target, payload, path)?);
            }
        }
        Ok(())
    }

    fn seal_position(
        profile: &SealedTokenProfile,
        trivia: &TriggerSet,
        inherited: &[TriggerIdentifier],
        own: Option<TriggerIdentifier>,
    ) -> Result<(), TokenProfileError> {
        let mut triggers = trivia.triggers().to_vec();
        triggers.extend_from_slice(inherited);
        triggers.extend(own);
        profile.seal_trigger_set(TriggerSet::new(triggers))?;
        Ok(())
    }

    fn require_trigger_kind(
        profile: &SealedTokenProfile,
        identifier: TriggerIdentifier,
        expected: &'static str,
        accepts: impl FnOnce(&Trigger) -> bool,
    ) -> Result<(), TableError> {
        let definition = profile.definition(identifier)?;
        if !accepts(&definition.trigger) {
            return Err(TableError::WrongTriggerKind {
                identifier,
                expected,
            });
        }
        Ok(())
    }
}
