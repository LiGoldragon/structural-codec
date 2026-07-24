//! Direct textual execution of sealed structural forms.
//!
//! This is an implementation surface of [`StructuralEvaluator`], not a second
//! engine. It asks the current expected form for its active trigger set, matches
//! only that set at the cursor, and recurses immediately. No preliminary token
//! stream or parallel annotation tree exists.

use std::collections::BTreeSet;

use name_table::{NameInterner, NameResolver, NameTable, NameTransaction};
use raw_discovery::{
    Atom, BoundaryReader, BoundarySide, SealedTokenProfile, SealedTriggerSet, Trigger,
    TriggerIdentifier, TriggerMatch, TriggerMatchKind, TriggerSet,
};

use crate::error::{DecodeError, EncodeError};
use crate::evaluator::{DecodeDraft, StructuralEvaluator};
use crate::form::{DelegationPayload, LeafCodec, ScalarLeaf, SequenceForm, StructuralForm};
use crate::ids::ScopedEncodedTypeId;
use crate::value::{ScalarValue, StructuralValue};

struct TextReading<'source> {
    source: &'source str,
    byte_offset: usize,
}

impl<'source> TextReading<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            source,
            byte_offset: 0,
        }
    }

    fn mark(&self) -> usize {
        self.byte_offset
    }

    fn restore(&mut self, mark: usize) {
        self.byte_offset = mark;
    }

    fn is_end(&self) -> bool {
        self.byte_offset == self.source.len()
    }

    fn boundary<'profile>(
        &self,
        profile: &'profile SealedTokenProfile,
    ) -> BoundaryReader<'source, 'profile> {
        let mut boundary = BoundaryReader::new(self.source, profile);
        boundary.advance_to(self.byte_offset);
        boundary
    }

    fn skip_trivia(
        &mut self,
        profile: &SealedTokenProfile,
        active: &SealedTriggerSet,
    ) -> Result<(), DecodeError> {
        let mut boundary = self.boundary(profile);
        boundary.skip_trivia(active)?;
        self.byte_offset = boundary.byte_offset();
        Ok(())
    }

    fn longest_match(
        &self,
        profile: &SealedTokenProfile,
        active: &SealedTriggerSet,
    ) -> Result<Option<TriggerMatch>, DecodeError> {
        Ok(self.boundary(profile).longest_match(active)?)
    }

    fn consume(
        &mut self,
        profile: &SealedTokenProfile,
        active: &SealedTriggerSet,
    ) -> Result<Option<TriggerMatch>, DecodeError> {
        let mut boundary = self.boundary(profile);
        let matched = boundary.consume(active)?;
        self.byte_offset = boundary.byte_offset();
        Ok(matched)
    }

    fn read_bare(
        &mut self,
        profile: &SealedTokenProfile,
        active: &SealedTriggerSet,
    ) -> Result<Option<String>, DecodeError> {
        let mut boundary = self.boundary(profile);
        let bare = boundary.read_bare(active)?;
        self.byte_offset = boundary.byte_offset();
        Ok(bare)
    }
}

impl StructuralEvaluator<'_> {
    /// Source text + expected type → structural value. Recognition is driven
    /// directly by the expected forms and their active trigger sets.
    pub fn decode_text(
        &self,
        expected: ScopedEncodedTypeId,
        source: &str,
        names: &mut NameTable,
    ) -> Result<StructuralValue, DecodeError> {
        names.try_intern(|transaction: &mut NameTransaction<'_>| {
            self.decode_text_with_interner(expected, source, transaction)
        })
    }

    /// The transactional inner operation used by [`Textual`](crate::Textual) so
    /// decode, reify, and every name allocation can share one commit boundary.
    pub fn decode_text_with_interner(
        &self,
        expected: ScopedEncodedTypeId,
        source: &str,
        interner: &mut impl NameInterner,
    ) -> Result<StructuralValue, DecodeError> {
        let profile = self.checked_profile_decode()?;
        let mut reading = TextReading::new(source);
        let draft = self.match_text_type(expected, &mut reading, &[], &[], false, &[])?;
        let trailing = self.active_set(profile, &[], None)?;
        reading.skip_trivia(profile, &trailing)?;
        if !reading.is_end() {
            return Err(DecodeError::TrailingText {
                byte_offset: reading.byte_offset,
            });
        }
        draft.resolve(interner).map_err(DecodeError::from)
    }

    fn match_text_type(
        &self,
        expected: ScopedEncodedTypeId,
        reading: &mut TextReading<'_>,
        terminators: &[TriggerIdentifier],
        inherited_entry_triggers: &[TriggerIdentifier],
        complete_on_trivia: bool,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<DecodeDraft, DecodeError> {
        if chain.contains(&expected) {
            return Err(DecodeError::DelegationCycle(expected));
        }
        let entry = self
            .table
            .entry(expected)
            .ok_or(DecodeError::UnknownType(expected))?;
        let mut child_chain = chain.to_vec();
        child_chain.push(expected);
        let mut entry_triggers = self.initial_type_triggers(expected, &mut BTreeSet::new())?;
        entry_triggers.extend_from_slice(inherited_entry_triggers);
        entry_triggers.sort_unstable();
        entry_triggers.dedup();
        for codec in &entry.constructors {
            for form in &codec.decode_forms {
                let mark = reading.mark();
                match self.match_text_form(
                    form,
                    reading,
                    terminators,
                    &entry_triggers,
                    complete_on_trivia,
                    &child_chain,
                ) {
                    Ok(draft)
                        if self.position_is_complete(
                            reading,
                            terminators,
                            complete_on_trivia,
                        )? =>
                    {
                        return Ok(DecodeDraft::Chosen {
                            constructor: codec.constructor.constructor,
                            payload: Box::new(draft),
                        });
                    }
                    Ok(_) | Err(_) => reading.restore(mark),
                }
            }
        }
        Err(DecodeError::NoAlternative {
            core_type: expected,
        })
    }

    fn match_text_form(
        &self,
        form: &StructuralForm,
        reading: &mut TextReading<'_>,
        terminators: &[TriggerIdentifier],
        entry_triggers: &[TriggerIdentifier],
        complete_on_trivia: bool,
        chain: &[ScopedEncodedTypeId],
    ) -> Result<DecodeDraft, DecodeError> {
        let position_triggers = Self::combined_triggers(terminators, entry_triggers, None);
        match form {
            StructuralForm::Atom(atom_form) => {
                let text =
                    self.read_position_text(reading, &position_triggers, atom_form.trigger)?;
                let atom = Atom::new(text.as_str());
                if !atom_form.accepts_case(&atom) {
                    return Err(DecodeError::CaseMismatch);
                }
                Ok(DecodeDraft::Atom(text))
            }
            StructuralForm::Literal(identifier) => {
                let text = self.read_position_text(reading, &position_triggers, None)?;
                let lexicon = self.lexicon.ok_or(DecodeError::LiteralMismatch)?;
                let expected = lexicon.resolve(*identifier)?;
                if expected.as_str() != text {
                    return Err(DecodeError::LiteralMismatch);
                }
                Ok(DecodeDraft::Atom(text))
            }
            StructuralForm::Leaf(leaf) => {
                let text = self.read_position_text(reading, &position_triggers, leaf.trigger)?;
                Ok(DecodeDraft::Scalar(Self::parse_scalar_text(
                    &leaf.codec,
                    text,
                )?))
            }
            StructuralForm::Application {
                operator,
                head,
                payload,
            } => {
                let head_terminators =
                    Self::combined_triggers(&position_triggers, &[], Some(*operator));
                let head =
                    self.match_text_form(head, reading, &head_terminators, &[], false, chain)?;
                self.consume_expected(
                    reading,
                    &position_triggers,
                    *operator,
                    |kind| {
                        matches!(
                            kind,
                            TriggerMatchKind::Application | TriggerMatchKind::Punctuation
                        )
                    },
                    "application operator",
                )?;
                let payload = self.match_text_form(
                    payload,
                    reading,
                    terminators,
                    &[],
                    complete_on_trivia,
                    &[],
                )?;
                Ok(DecodeDraft::Application(Box::new(head), Box::new(payload)))
            }
            StructuralForm::Delimited {
                boundary, sequence, ..
            } => {
                self.consume_expected(
                    reading,
                    &position_triggers,
                    *boundary,
                    |kind| matches!(kind, TriggerMatchKind::Boundary(BoundarySide::Opening)),
                    "opening boundary",
                )?;
                let children = self.match_text_sequence(sequence, reading, *boundary)?;
                self.consume_expected(
                    reading,
                    &[],
                    *boundary,
                    |kind| matches!(kind, TriggerMatchKind::Boundary(BoundarySide::Closing)),
                    "closing boundary",
                )?;
                Ok(DecodeDraft::Delimited(children))
            }
            StructuralForm::Delegate { target, payload } => {
                let draft = self.match_text_type(
                    *target,
                    reading,
                    terminators,
                    entry_triggers,
                    complete_on_trivia,
                    chain,
                )?;
                if let Some(payload) = payload
                    && !Self::draft_accepts_payload(*payload, &draft)
                {
                    return Err(DecodeError::DelegationPayloadMismatch { payload: *payload });
                }
                Ok(DecodeDraft::Delegated(Box::new(draft)))
            }
        }
    }

    fn match_text_sequence(
        &self,
        sequence: &SequenceForm,
        reading: &mut TextReading<'_>,
        boundary: TriggerIdentifier,
    ) -> Result<Vec<DecodeDraft>, DecodeError> {
        match sequence {
            SequenceForm::Product(forms) => forms
                .iter()
                .map(|form| self.match_text_form(form, reading, &[boundary], &[], true, &[]))
                .collect(),
            SequenceForm::Repeat {
                minimum,
                maximum,
                element,
            } => {
                let profile = self.checked_profile_decode()?;
                let active = self.active_set(profile, &[], Some(boundary))?;
                let mut children = Vec::new();
                loop {
                    reading.skip_trivia(profile, &active)?;
                    if reading
                        .longest_match(profile, &active)?
                        .is_some_and(|matched| {
                            matched.identifier() == boundary
                                && matches!(
                                    matched.kind(),
                                    TriggerMatchKind::Boundary(BoundarySide::Closing)
                                )
                        })
                    {
                        break;
                    }
                    if maximum.is_some_and(|maximum| children.len() as u64 == maximum) {
                        return Err(DecodeError::SequenceCardinality {
                            found: children.len() as u64 + 1,
                        });
                    }
                    let mark = reading.mark();
                    children.push(self.match_text_form(
                        element,
                        reading,
                        &[boundary],
                        &[],
                        true,
                        &[],
                    )?);
                    if reading.mark() == mark {
                        return Err(DecodeError::TextReaderDidNotAdvance);
                    }
                }
                if (children.len() as u64) < *minimum {
                    return Err(DecodeError::SequenceCardinality {
                        found: children.len() as u64,
                    });
                }
                Ok(children)
            }
        }
    }

    fn read_position_text(
        &self,
        reading: &mut TextReading<'_>,
        terminators: &[TriggerIdentifier],
        own: Option<TriggerIdentifier>,
    ) -> Result<String, DecodeError> {
        let profile = self.checked_profile_decode()?;
        let active = self.active_set(profile, terminators, own)?;
        reading.skip_trivia(profile, &active)?;
        match own {
            Some(identifier) => {
                let matched =
                    reading
                        .consume(profile, &active)?
                        .ok_or(DecodeError::ExpectedText {
                            byte_offset: reading.byte_offset,
                        })?;
                if matched.identifier() != identifier {
                    return Err(DecodeError::ExpectedTrigger {
                        expected: identifier,
                        found: Some(matched.identifier()),
                    });
                }
                matched
                    .body()
                    .map(str::to_owned)
                    .ok_or(DecodeError::ExpectedText {
                        byte_offset: matched.start(),
                    })
            }
            None => reading
                .read_bare(profile, &active)?
                .ok_or(DecodeError::ExpectedText {
                    byte_offset: reading.byte_offset,
                }),
        }
    }

    fn consume_expected(
        &self,
        reading: &mut TextReading<'_>,
        inherited: &[TriggerIdentifier],
        identifier: TriggerIdentifier,
        accepts: impl FnOnce(TriggerMatchKind) -> bool,
        expected_role: &'static str,
    ) -> Result<(), DecodeError> {
        let profile = self.checked_profile_decode()?;
        let active = self.active_set(profile, inherited, Some(identifier))?;
        reading.skip_trivia(profile, &active)?;
        let matched = reading
            .consume(profile, &active)?
            .ok_or(DecodeError::ExpectedTrigger {
                expected: identifier,
                found: None,
            })?;
        if matched.identifier() != identifier || !accepts(matched.kind()) {
            return Err(DecodeError::ExpectedTriggerRole {
                identifier,
                expected: expected_role,
            });
        }
        Ok(())
    }

    fn active_set(
        &self,
        profile: &SealedTokenProfile,
        inherited: &[TriggerIdentifier],
        own: Option<TriggerIdentifier>,
    ) -> Result<SealedTriggerSet, raw_discovery::TokenProfileError> {
        let mut triggers = self.table.trivia_triggers().triggers().to_vec();
        triggers.extend_from_slice(inherited);
        triggers.extend(own);
        profile.seal_trigger_set(TriggerSet::new(triggers))
    }

    fn position_is_complete(
        &self,
        reading: &mut TextReading<'_>,
        terminators: &[TriggerIdentifier],
        complete_on_trivia: bool,
    ) -> Result<bool, DecodeError> {
        let profile = self.checked_profile_decode()?;
        let active = self.active_set(profile, terminators, None)?;
        let before_trivia = reading.mark();
        reading.skip_trivia(profile, &active)?;
        Ok((complete_on_trivia && reading.mark() != before_trivia)
            || reading.is_end()
            || reading.longest_match(profile, &active)?.is_some())
    }

    fn initial_type_triggers(
        &self,
        expected: ScopedEncodedTypeId,
        path: &mut BTreeSet<ScopedEncodedTypeId>,
    ) -> Result<Vec<TriggerIdentifier>, DecodeError> {
        if !path.insert(expected) {
            return Ok(Vec::new());
        }
        let entry = self
            .table
            .entry(expected)
            .ok_or(DecodeError::UnknownType(expected))?;
        let mut triggers = Vec::new();
        for form in entry
            .constructors
            .iter()
            .flat_map(|codec| codec.decode_forms.iter())
        {
            self.initial_form_triggers(form, path, &mut triggers)?;
        }
        path.remove(&expected);
        triggers.sort_unstable();
        triggers.dedup();
        Ok(triggers)
    }

    fn initial_form_triggers(
        &self,
        form: &StructuralForm,
        path: &mut BTreeSet<ScopedEncodedTypeId>,
        triggers: &mut Vec<TriggerIdentifier>,
    ) -> Result<(), DecodeError> {
        match form {
            StructuralForm::Atom(atom) => triggers.extend(atom.trigger),
            StructuralForm::Leaf(leaf) => triggers.extend(leaf.trigger),
            StructuralForm::Literal(_) => {}
            StructuralForm::Application { operator, head, .. } => {
                triggers.push(*operator);
                self.initial_form_triggers(head, path, triggers)?;
            }
            StructuralForm::Delimited { boundary, .. } => triggers.push(*boundary),
            StructuralForm::Delegate { target, .. } => {
                triggers.extend(self.initial_type_triggers(*target, path)?);
            }
        }
        Ok(())
    }

    fn combined_triggers(
        first: &[TriggerIdentifier],
        second: &[TriggerIdentifier],
        own: Option<TriggerIdentifier>,
    ) -> Vec<TriggerIdentifier> {
        let mut triggers = first.to_vec();
        triggers.extend_from_slice(second);
        triggers.extend(own);
        triggers.sort_unstable();
        triggers.dedup();
        triggers
    }

    fn checked_profile_decode(&self) -> Result<&SealedTokenProfile, DecodeError> {
        let profile = self.profile.ok_or(DecodeError::MissingTokenProfile)?;
        if !self.table.raw_profile_identity().matches(profile) {
            return Err(DecodeError::TokenProfileIdentityMismatch);
        }
        Ok(profile)
    }

    fn checked_profile_encode(&self) -> Result<&SealedTokenProfile, EncodeError> {
        let profile = self.profile.ok_or(EncodeError::MissingTokenProfile)?;
        if !self.table.raw_profile_identity().matches(profile) {
            return Err(EncodeError::TokenProfileIdentityMismatch);
        }
        Ok(profile)
    }

    fn parse_scalar_text(codec: &LeafCodec, text: String) -> Result<ScalarValue, DecodeError> {
        match codec {
            LeafCodec::Scalar(ScalarLeaf::Integer) => text
                .parse()
                .map(ScalarValue::Integer)
                .map_err(|error: std::num::ParseIntError| {
                    DecodeError::ScalarParse(error.to_string())
                }),
            LeafCodec::Scalar(ScalarLeaf::Float) => {
                text.parse()
                    .map(ScalarValue::Float)
                    .map_err(|error: std::num::ParseFloatError| {
                        DecodeError::ScalarParse(error.to_string())
                    })
            }
            LeafCodec::Scalar(ScalarLeaf::Boolean) => text
                .parse()
                .map(ScalarValue::Boolean)
                .map_err(|error: std::str::ParseBoolError| {
                    DecodeError::ScalarParse(error.to_string())
                }),
            LeafCodec::Scalar(ScalarLeaf::Text) | LeafCodec::Carrier(_) => {
                Ok(ScalarValue::Text(text))
            }
            LeafCodec::Foreign(_) => Err(DecodeError::LeafNotFlattenable),
        }
    }

    fn draft_accepts_payload(payload: DelegationPayload, draft: &DecodeDraft) -> bool {
        match draft {
            DecodeDraft::Atom(text) => payload.accepts_atom(&Atom::new(text.as_str())),
            DecodeDraft::Delegated(inner) | DecodeDraft::Chosen { payload: inner, .. } => {
                Self::draft_accepts_payload(payload, inner)
            }
            DecodeDraft::Scalar(_) | DecodeDraft::Delimited(_) | DecodeDraft::Application(_, _) => {
                false
            }
        }
    }

    /// Structural value → canonical text through the same expected forms and
    /// token-profile data used by decoding.
    pub fn encode_text<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedEncodedTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
    ) -> Result<String, EncodeError> {
        let profile = self.checked_profile_encode()?;
        self.encode_text_type(expected, value, resolver, profile, &[])
    }

    fn encode_text_type<Resolver: NameResolver + ?Sized>(
        &self,
        expected: ScopedEncodedTypeId,
        value: &StructuralValue,
        resolver: &Resolver,
        profile: &SealedTokenProfile,
        terminators: &[TriggerIdentifier],
    ) -> Result<String, EncodeError> {
        let entry = self
            .table
            .entry(expected)
            .ok_or(EncodeError::UnknownType(expected))?;
        let (constructor, payload) = match value {
            StructuralValue::Chosen {
                constructor,
                payload,
            } => (*constructor, payload.as_ref()),
            _ => {
                return Err(EncodeError::ShapeMismatch(
                    "expected a constructor-tagged value at a type boundary",
                ));
            }
        };
        let codec = entry
            .constructors
            .iter()
            .find(|codec| codec.constructor.constructor == constructor)
            .ok_or(EncodeError::ConstructorOutOfRange {
                chosen: constructor,
                available: entry.constructors.len(),
            })?;
        self.encode_text_form(&codec.encode_form, payload, resolver, profile, terminators)
    }

    fn encode_text_form<Resolver: NameResolver + ?Sized>(
        &self,
        form: &StructuralForm,
        value: &StructuralValue,
        resolver: &Resolver,
        profile: &SealedTokenProfile,
        terminators: &[TriggerIdentifier],
    ) -> Result<String, EncodeError> {
        match (form, value) {
            (StructuralForm::Atom(atom_form), StructuralValue::Atom(identifier)) => {
                let name = resolver.resolve(*identifier)?;
                self.validate_emitted_spelling(
                    name.as_str(),
                    profile,
                    terminators,
                    atom_form.trigger,
                )?;
                Ok(name.as_str().to_owned())
            }
            (StructuralForm::Literal(expected), StructuralValue::Atom(identifier)) => {
                if identifier != expected {
                    return Err(EncodeError::LiteralMismatch);
                }
                let name = resolver.resolve(*identifier)?;
                self.validate_emitted_spelling(name.as_str(), profile, terminators, None)?;
                Ok(name.as_str().to_owned())
            }
            (StructuralForm::Leaf(leaf), StructuralValue::Scalar(scalar)) => {
                let body = Self::scalar_body(scalar);
                match leaf.trigger {
                    Some(identifier) => self.emit_trigger_body(profile, identifier, &body),
                    None => {
                        self.validate_emitted_spelling(&body, profile, terminators, None)?;
                        Ok(body)
                    }
                }
            }
            (
                StructuralForm::Application {
                    operator,
                    head,
                    payload,
                },
                StructuralValue::Application(value_head, value_payload),
            ) => {
                let mut head_terminators = terminators.to_vec();
                head_terminators.push(*operator);
                let head =
                    self.encode_text_form(head, value_head, resolver, profile, &head_terminators)?;
                let operator = Self::exact_trigger_spelling(profile, *operator)?;
                let payload =
                    self.encode_text_form(payload, value_payload, resolver, profile, terminators)?;
                Ok(format!("{head}{operator}{payload}"))
            }
            (
                StructuralForm::Delimited {
                    boundary, sequence, ..
                },
                StructuralValue::Delimited(children),
            ) => {
                let (opening, closing) = Self::boundary_spellings(profile, *boundary)?;
                let children =
                    self.encode_text_sequence(sequence, children, resolver, profile, *boundary)?;
                let separator = self.canonical_separator(profile)?;
                Ok(format!("{opening}{}{closing}", children.join(separator)))
            }
            (StructuralForm::Delegate { target, payload }, StructuralValue::Delegated(inner)) => {
                let text = self.encode_text_type(*target, inner, resolver, profile, terminators)?;
                if let Some(payload) = payload
                    && !payload.accepts_atom(&Atom::new(text.as_str()))
                {
                    return Err(EncodeError::DelegationPayloadMismatch { payload: *payload });
                }
                Ok(text)
            }
            _ => Err(EncodeError::ShapeMismatch(
                "value did not fit the canonical encode form",
            )),
        }
    }

    fn encode_text_sequence<Resolver: NameResolver + ?Sized>(
        &self,
        sequence: &SequenceForm,
        children: &[StructuralValue],
        resolver: &Resolver,
        profile: &SealedTokenProfile,
        boundary: TriggerIdentifier,
    ) -> Result<Vec<String>, EncodeError> {
        match sequence {
            SequenceForm::Product(forms) => {
                if forms.len() != children.len() {
                    return Err(EncodeError::ShapeMismatch(
                        "product arity did not match the value's children",
                    ));
                }
                forms
                    .iter()
                    .zip(children)
                    .map(|(form, child)| {
                        self.encode_text_form(form, child, resolver, profile, &[boundary])
                    })
                    .collect()
            }
            SequenceForm::Repeat {
                minimum,
                maximum,
                element,
            } => {
                if (children.len() as u64) < *minimum
                    || maximum.is_some_and(|maximum| children.len() as u64 > maximum)
                {
                    return Err(EncodeError::ShapeMismatch(
                        "repeat count did not fit the canonical sequence",
                    ));
                }
                children
                    .iter()
                    .map(|child| {
                        self.encode_text_form(element, child, resolver, profile, &[boundary])
                    })
                    .collect()
            }
        }
    }

    fn validate_emitted_spelling(
        &self,
        spelling: &str,
        profile: &SealedTokenProfile,
        terminators: &[TriggerIdentifier],
        own: Option<TriggerIdentifier>,
    ) -> Result<(), EncodeError> {
        let active = self.active_set(profile, terminators, own)?;
        let mut boundary = BoundaryReader::new(spelling, profile);
        match own {
            Some(identifier) => {
                let matched = boundary
                    .consume(&active)?
                    .ok_or(EncodeError::NonCanonicalSpelling)?;
                if matched.identifier() != identifier || !boundary.is_end() {
                    return Err(EncodeError::NonCanonicalSpelling);
                }
            }
            None => {
                let read = boundary
                    .read_bare(&active)?
                    .ok_or(EncodeError::NonCanonicalSpelling)?;
                if read != spelling || !boundary.is_end() {
                    return Err(EncodeError::NonCanonicalSpelling);
                }
            }
        }
        Ok(())
    }

    fn scalar_body(scalar: &ScalarValue) -> String {
        match scalar {
            ScalarValue::Integer(value) => value.to_string(),
            ScalarValue::Float(value) => value.to_string(),
            ScalarValue::Text(value) => value.clone(),
            ScalarValue::Boolean(value) => value.to_string(),
        }
    }

    fn exact_trigger_spelling(
        profile: &SealedTokenProfile,
        identifier: TriggerIdentifier,
    ) -> Result<&str, EncodeError> {
        match &profile.definition(identifier)?.trigger {
            Trigger::Application { glyph } | Trigger::Punctuation { glyph } => Ok(glyph),
            _ => Err(EncodeError::ExpectedTriggerRole {
                identifier,
                expected: "application or punctuation",
            }),
        }
    }

    fn boundary_spellings(
        profile: &SealedTokenProfile,
        identifier: TriggerIdentifier,
    ) -> Result<(&str, &str), EncodeError> {
        match &profile.definition(identifier)?.trigger {
            Trigger::Boundary { opening, closing } => Ok((opening, closing)),
            _ => Err(EncodeError::ExpectedTriggerRole {
                identifier,
                expected: "boundary",
            }),
        }
    }

    fn emit_trigger_body(
        &self,
        profile: &SealedTokenProfile,
        identifier: TriggerIdentifier,
        body: &str,
    ) -> Result<String, EncodeError> {
        match &profile.definition(identifier)?.trigger {
            Trigger::Carrier {
                opening,
                closing,
                escape,
            } => {
                let mut escaped = body.to_owned();
                if let Some(escape) = escape {
                    escaped = escaped.replace(escape, &format!("{escape}{escape}"));
                    escaped = escaped.replace(closing, &format!("{escape}{closing}"));
                } else if escaped.contains(closing) {
                    return Err(EncodeError::NonCanonicalSpelling);
                }
                Ok(format!("{opening}{escaped}{closing}"))
            }
            Trigger::LeadingCharacterClass { .. } => {
                self.validate_emitted_spelling(body, profile, &[], Some(identifier))?;
                Ok(body.to_owned())
            }
            _ => Err(EncodeError::ExpectedTriggerRole {
                identifier,
                expected: "carrier or leading character class",
            }),
        }
    }

    fn canonical_separator<'profile>(
        &self,
        profile: &'profile SealedTokenProfile,
    ) -> Result<&'profile str, EncodeError> {
        for identifier in self.table.trivia_triggers().triggers() {
            if let Trigger::Whitespace { canonical_spelling } =
                &profile.definition(*identifier)?.trigger
            {
                return Ok(canonical_spelling);
            }
        }
        Err(EncodeError::MissingCanonicalSeparator)
    }
}
