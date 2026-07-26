//! Disposable R3 typed-kernel viability witness.
//!
//! This module is deliberately not an implementation proposal. It answers one
//! narrow question: can fixed heterogeneous record layouts remain archived data
//! while one evaluator walks them without record-specific decode, encode,
//! boundary, or disjointness code? The evaluator returns a generic
//! StructureTree. It does not reify that tree to an application Rust type, and
//! consequently does not answer the parked reify/reflect question.

use std::fmt;

/// One spelling owned by a typed descriptor position or decoded textual value.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct Spelling(String);

impl Spelling {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Spelling {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// A typed rule record, rather than a product distinguished by ordinal slots.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordType {
    ProtosPrimitiveRule,
    RustPrivateNewtypeRule,
    RustPublicNewtypeRule,
}

/// The closed role vocabulary. These values name positions in diagnostics and
/// archived trees; no evaluator branch recovers position meaning by an index.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionRole {
    ProtosPrimitiveKeyword,
    ProtosPrimitiveName,
    ProtosPrimitiveAssignment,
    ProtosPrimitiveKind,
    ProtosPrimitiveTerminator,
    NewtypeAttributes,
    NewtypeVisibility,
    NewtypeItemKeyword,
    NewtypeTypeName,
    NewtypeParenthesizedTypeReference,
    NewtypeTerminator,
}

/// A generic structural delimiter. A vector of these describes a nesting or
/// alternative vocabulary, never a record's fixed fields.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct DelimiterPair {
    pub opening: Spelling,
    pub closing: Spelling,
}

impl DelimiterPair {
    pub fn new(opening: impl Into<Spelling>, closing: impl Into<Spelling>) -> Self {
        Self {
            opening: opening.into(),
            closing: closing.into(),
        }
    }
}

/// One top-level item ending. The alternatives are grammar alternatives, not
/// fixed positions.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub enum ItemEnding {
    Exact(Spelling),
    Balanced(DelimiterPair),
}

/// Boundary-first extent data. The generic evaluator discovers an item boundary
/// before interpreting one typed record position.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct ItemExtentDescriptor {
    pub endings: Vec<ItemEnding>,
    pub nested_pairs: Vec<DelimiterPair>,
}

/// The accepted language of one typed position.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub enum PositionForm {
    /// This typed position consumes no source text.
    Absent,
    /// This typed position owns one literal spelling.
    Exact(Spelling),
    /// One identifier-shaped textual value.
    Identifier,
    /// One delimited textual value, retaining the typed descriptor spelling.
    Delimited {
        boundary: DelimiterPair,
        nested_pairs: Vec<DelimiterPair>,
    },
    /// Repeated delimited values. The vector represents repetition only.
    RepeatedDelimited {
        boundary: DelimiterPair,
        nested_pairs: Vec<DelimiterPair>,
    },
}

/// A recursive fixed-position descriptor. Position and End form a typed linked
/// algebra rather than a homogeneous product collection.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum PositionLayout {
    End,
    Position {
        role: PositionRole,
        form: PositionForm,
        #[rkyv(omit_bounds)]
        next: Box<PositionLayout>,
    },
}

impl PositionLayout {
    pub fn position(role: PositionRole, form: PositionForm, next: Self) -> Self {
        Self::Position {
            role,
            form,
            next: Box::new(next),
        }
    }

    pub fn role(&self) -> Option<PositionRole> {
        match self {
            Self::End => None,
            Self::Position { role, .. } => Some(*role),
        }
    }

    pub fn next(&self) -> Option<&Self> {
        match self {
            Self::End => None,
            Self::Position { next, .. } => Some(next),
        }
    }
}

/// One pure-data record descriptor.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct RecordDescriptor {
    pub record: RecordType,
    pub item_extent: ItemExtentDescriptor,
    pub positions: PositionLayout,
}

/// A data-loaded vocabulary. The collection is the set of record alternatives;
/// it never represents the fields of an individual record.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
pub struct VocabularyDescriptor {
    pub records: Vec<RecordDescriptor>,
}

impl VocabularyDescriptor {
    pub fn record(&self, wanted: RecordType) -> Option<&RecordDescriptor> {
        self.records.iter().find(|record| record.record == wanted)
    }
}

/// Generic structural values. Record field values retain their named typed
/// roles in PositionValues, so a reader cannot recover a field by counting
/// through a homogeneous sequence.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum StructureTree {
    Text(Spelling),
    Delimited {
        boundary: DelimiterPair,
        body: Spelling,
    },
    Repeated(#[rkyv(omit_bounds)] Vec<StructureTree>),
    Absent,
    Record {
        record: RecordType,
        #[rkyv(omit_bounds)]
        positions: PositionValues,
    },
}

/// A recursive fixed-position value algebra parallel to PositionLayout.
#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, __C::Error: rkyv::rancor::Source)),
)]
pub enum PositionValues {
    End,
    Position {
        role: PositionRole,
        #[rkyv(omit_bounds)]
        value: Box<StructureTree>,
        #[rkyv(omit_bounds)]
        next: Box<PositionValues>,
    },
}

impl PositionValues {
    pub fn value(&self, wanted: PositionRole) -> Option<&StructureTree> {
        match self {
            Self::End => None,
            Self::Position { role, value, next } => {
                if *role == wanted {
                    Some(value)
                } else {
                    next.value(wanted)
                }
            }
        }
    }
}

/// A runtime source range, discovered structurally before record decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceExtent {
    end: usize,
}

impl SourceExtent {
    pub fn end(self) -> usize {
        self.end
    }
}

/// The decoded data plus the boundary-first source extent that contained it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedRecord {
    pub tree: StructureTree,
    pub extent: SourceExtent,
}

/// A disjointness proof names roles, never a flattened-field index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypedPositionDisjointness {
    pub left: PositionRole,
    pub right: PositionRole,
}

/// A deliberately small refusal vocabulary for the spike.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpikeError {
    UnknownRecord(RecordType),
    MissingItemEnding,
    UnclosedDelimiter(DelimiterPair),
    UnexpectedText {
        role: PositionRole,
        expected: PositionForm,
    },
    TrailingText,
    TreeWasNotRecord,
    LayoutValueMismatch {
        role: PositionRole,
    },
    NotProvablyDisjoint,
}

impl fmt::Display for SpikeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SpikeError {}

/// The one shared interpreter. It branches only over the descriptor algebra and
/// generic value algebra; it has no Rust, Protos, or record-specific arm.
pub struct SharedEvaluator;

impl SharedEvaluator {
    pub fn decode(
        vocabulary: &VocabularyDescriptor,
        record: RecordType,
        source: &str,
    ) -> Result<DecodedRecord, SpikeError> {
        let descriptor = vocabulary
            .record(record)
            .ok_or(SpikeError::UnknownRecord(record))?;
        let extent = Self::item_extent(&descriptor.item_extent, source)?;
        if !source[extent.end..].trim().is_empty() {
            return Err(SpikeError::TrailingText);
        }
        let item = &source[..extent.end];
        let mut cursor = 0;
        let positions = Self::decode_layout(&descriptor.positions, item, &mut cursor)?;
        if !item[cursor..].trim().is_empty() {
            return Err(SpikeError::TrailingText);
        }
        Ok(DecodedRecord {
            tree: StructureTree::Record { record, positions },
            extent,
        })
    }

    pub fn encode(
        vocabulary: &VocabularyDescriptor,
        tree: &StructureTree,
    ) -> Result<String, SpikeError> {
        let StructureTree::Record { record, positions } = tree else {
            return Err(SpikeError::TreeWasNotRecord);
        };
        let descriptor = vocabulary
            .record(*record)
            .ok_or(SpikeError::UnknownRecord(*record))?;
        Self::encode_layout(&descriptor.positions, positions)
    }

    pub fn item_extent(
        descriptor: &ItemExtentDescriptor,
        source: &str,
    ) -> Result<SourceExtent, SpikeError> {
        let mut cursor = 0;
        let mut nesting = Vec::new();
        while cursor < source.len() {
            if nesting.is_empty() {
                for ending in &descriptor.endings {
                    match ending {
                        ItemEnding::Exact(spelling)
                            if source[cursor..].starts_with(spelling.as_str()) =>
                        {
                            return Ok(SourceExtent {
                                end: cursor + spelling.as_str().len(),
                            });
                        }
                        ItemEnding::Balanced(pair)
                            if source[cursor..].starts_with(pair.opening.as_str()) =>
                        {
                            if let Some(end) =
                                Self::balanced_end(source, cursor, pair, &descriptor.nested_pairs)?
                            {
                                return Ok(SourceExtent { end });
                            }
                        }
                        ItemEnding::Exact(_) | ItemEnding::Balanced(_) => {}
                    }
                }
            }

            if let Some(pair) = Self::opening_at(source, cursor, &descriptor.nested_pairs) {
                cursor += pair.opening.as_str().len();
                nesting.push(pair);
            } else if let Some(pair) = nesting.last()
                && source[cursor..].starts_with(pair.closing.as_str())
            {
                cursor += pair.closing.as_str().len();
                nesting.pop();
            } else {
                cursor += Self::next_character_width(&source[cursor..]);
            }
        }
        if let Some(pair) = nesting.pop() {
            return Err(SpikeError::UnclosedDelimiter(pair.clone()));
        }
        Err(SpikeError::MissingItemEnding)
    }

    pub fn prove_disjoint(
        left: &RecordDescriptor,
        right: &RecordDescriptor,
    ) -> Result<TypedPositionDisjointness, SpikeError> {
        Self::prove_layout_disjoint(&left.positions, &right.positions)
    }

    fn prove_layout_disjoint(
        left: &PositionLayout,
        right: &PositionLayout,
    ) -> Result<TypedPositionDisjointness, SpikeError> {
        match (left, right) {
            (PositionLayout::End, PositionLayout::End) => Err(SpikeError::NotProvablyDisjoint),
            (
                PositionLayout::Position {
                    role: left_role,
                    form: left_form,
                    next: left_next,
                },
                PositionLayout::Position {
                    role: right_role,
                    form: right_form,
                    next: right_next,
                },
            ) => {
                if Self::forms_are_disjoint(left_form, right_form) {
                    Ok(TypedPositionDisjointness {
                        left: *left_role,
                        right: *right_role,
                    })
                } else {
                    Self::prove_layout_disjoint(left_next, right_next)
                }
            }
            (PositionLayout::End, PositionLayout::Position { .. })
            | (PositionLayout::Position { .. }, PositionLayout::End) => {
                Err(SpikeError::NotProvablyDisjoint)
            }
        }
    }

    fn forms_are_disjoint(left: &PositionForm, right: &PositionForm) -> bool {
        match (left, right) {
            (PositionForm::Absent, PositionForm::Absent) => false,
            (PositionForm::Absent, _) | (_, PositionForm::Absent) => true,
            (PositionForm::Exact(left), PositionForm::Exact(right)) => left != right,
            (PositionForm::Exact(_), PositionForm::Identifier)
            | (PositionForm::Identifier, PositionForm::Exact(_)) => false,
            (PositionForm::Exact(_), PositionForm::Delimited { .. })
            | (PositionForm::Delimited { .. }, PositionForm::Exact(_))
            | (PositionForm::Exact(_), PositionForm::RepeatedDelimited { .. })
            | (PositionForm::RepeatedDelimited { .. }, PositionForm::Exact(_))
            | (PositionForm::Identifier, PositionForm::Delimited { .. })
            | (PositionForm::Delimited { .. }, PositionForm::Identifier)
            | (PositionForm::Identifier, PositionForm::RepeatedDelimited { .. })
            | (PositionForm::RepeatedDelimited { .. }, PositionForm::Identifier)
            | (PositionForm::Delimited { .. }, PositionForm::RepeatedDelimited { .. })
            | (PositionForm::RepeatedDelimited { .. }, PositionForm::Delimited { .. }) => true,
            (PositionForm::Identifier, PositionForm::Identifier)
            | (PositionForm::Delimited { .. }, PositionForm::Delimited { .. })
            | (PositionForm::RepeatedDelimited { .. }, PositionForm::RepeatedDelimited { .. }) => {
                false
            }
        }
    }

    fn decode_layout(
        layout: &PositionLayout,
        source: &str,
        cursor: &mut usize,
    ) -> Result<PositionValues, SpikeError> {
        match layout {
            PositionLayout::End => {
                Self::skip_whitespace(source, cursor);
                Ok(PositionValues::End)
            }
            PositionLayout::Position { role, form, next } => {
                Self::skip_whitespace(source, cursor);
                let value = Self::decode_form(*role, form, source, cursor)?;
                let next = Self::decode_layout(next, source, cursor)?;
                Ok(PositionValues::Position {
                    role: *role,
                    value: Box::new(value),
                    next: Box::new(next),
                })
            }
        }
    }

    fn decode_form(
        role: PositionRole,
        form: &PositionForm,
        source: &str,
        cursor: &mut usize,
    ) -> Result<StructureTree, SpikeError> {
        match form {
            PositionForm::Absent => Ok(StructureTree::Absent),
            PositionForm::Exact(expected) => {
                if !Self::spelling_at(source, *cursor, expected) {
                    return Err(SpikeError::UnexpectedText {
                        role,
                        expected: form.clone(),
                    });
                }
                *cursor += expected.as_str().len();
                Ok(StructureTree::Text(expected.clone()))
            }
            PositionForm::Identifier => {
                let end = Self::identifier_end(source, *cursor).ok_or_else(|| {
                    SpikeError::UnexpectedText {
                        role,
                        expected: form.clone(),
                    }
                })?;
                let value = Spelling::new(&source[*cursor..end]);
                *cursor = end;
                Ok(StructureTree::Text(value))
            }
            PositionForm::Delimited {
                boundary,
                nested_pairs,
            } => {
                let end = Self::delimited_end(source, *cursor, boundary, nested_pairs)?;
                let body_start = *cursor + boundary.opening.as_str().len();
                let body_end = end - boundary.closing.as_str().len();
                let body = Spelling::new(&source[body_start..body_end]);
                *cursor = end;
                Ok(StructureTree::Delimited {
                    boundary: boundary.clone(),
                    body,
                })
            }
            PositionForm::RepeatedDelimited {
                boundary,
                nested_pairs,
            } => {
                let mut values = Vec::new();
                while source[*cursor..].starts_with(boundary.opening.as_str()) {
                    let end = Self::delimited_end(source, *cursor, boundary, nested_pairs)?;
                    let body_start = *cursor + boundary.opening.as_str().len();
                    let body_end = end - boundary.closing.as_str().len();
                    values.push(StructureTree::Delimited {
                        boundary: boundary.clone(),
                        body: Spelling::new(&source[body_start..body_end]),
                    });
                    *cursor = end;
                    Self::skip_whitespace(source, cursor);
                }
                Ok(StructureTree::Repeated(values))
            }
        }
    }

    fn encode_layout(
        layout: &PositionLayout,
        values: &PositionValues,
    ) -> Result<String, SpikeError> {
        match (layout, values) {
            (PositionLayout::End, PositionValues::End) => Ok(String::new()),
            (
                PositionLayout::Position { role, form, next },
                PositionValues::Position {
                    role: found,
                    value,
                    next: values_next,
                },
            ) if role == found => {
                let head = Self::encode_form(*role, form, value)?;
                let tail = Self::encode_layout(next, values_next)?;
                Ok(match (head.is_empty(), tail.is_empty()) {
                    (true, _) => tail,
                    (_, true) => head,
                    (false, false) => format!("{head} {tail}"),
                })
            }
            (PositionLayout::Position { role, .. }, _) => {
                Err(SpikeError::LayoutValueMismatch { role: *role })
            }
            (PositionLayout::End, PositionValues::Position { role, .. }) => {
                Err(SpikeError::LayoutValueMismatch { role: *role })
            }
        }
    }

    fn encode_form(
        role: PositionRole,
        form: &PositionForm,
        value: &StructureTree,
    ) -> Result<String, SpikeError> {
        match (form, value) {
            (PositionForm::Absent, StructureTree::Absent) => Ok(String::new()),
            (PositionForm::Exact(expected), StructureTree::Text(found)) if expected == found => {
                Ok(found.as_str().to_owned())
            }
            (PositionForm::Identifier, StructureTree::Text(identifier))
                if Self::identifier_end(identifier.as_str(), 0)
                    == Some(identifier.as_str().len()) =>
            {
                Ok(identifier.as_str().to_owned())
            }
            (
                PositionForm::Delimited { boundary, .. },
                StructureTree::Delimited {
                    boundary: found,
                    body,
                },
            ) if boundary == found => Ok(format!(
                "{}{}{}",
                boundary.opening.as_str(),
                body.as_str(),
                boundary.closing.as_str()
            )),
            (PositionForm::RepeatedDelimited { boundary, .. }, StructureTree::Repeated(values)) => {
                values
                    .iter()
                    .map(|value| {
                        Self::encode_form(
                            role,
                            &PositionForm::Delimited {
                                boundary: boundary.clone(),
                                nested_pairs: Vec::new(),
                            },
                            value,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(|values| values.join(" "))
            }
            _ => Err(SpikeError::LayoutValueMismatch { role }),
        }
    }

    fn delimited_end(
        source: &str,
        cursor: usize,
        boundary: &DelimiterPair,
        nested_pairs: &[DelimiterPair],
    ) -> Result<usize, SpikeError> {
        if !source[cursor..].starts_with(boundary.opening.as_str()) {
            return Err(SpikeError::MissingItemEnding);
        }
        Self::balanced_end(source, cursor, boundary, nested_pairs)?
            .ok_or_else(|| SpikeError::UnclosedDelimiter(boundary.clone()))
    }

    fn balanced_end(
        source: &str,
        cursor: usize,
        boundary: &DelimiterPair,
        nested_pairs: &[DelimiterPair],
    ) -> Result<Option<usize>, SpikeError> {
        let mut offset = cursor + boundary.opening.as_str().len();
        let mut nesting = vec![boundary.clone()];
        while offset < source.len() {
            if let Some(pair) = Self::opening_at(source, offset, nested_pairs) {
                offset += pair.opening.as_str().len();
                nesting.push(pair.clone());
            } else if let Some(pair) = nesting.last()
                && source[offset..].starts_with(pair.closing.as_str())
            {
                offset += pair.closing.as_str().len();
                nesting.pop();
                if nesting.is_empty() {
                    return Ok(Some(offset));
                }
            } else {
                offset += Self::next_character_width(&source[offset..]);
            }
        }
        Ok(None)
    }

    fn opening_at<'a>(
        source: &str,
        cursor: usize,
        pairs: &'a [DelimiterPair],
    ) -> Option<&'a DelimiterPair> {
        pairs
            .iter()
            .filter(|pair| source[cursor..].starts_with(pair.opening.as_str()))
            .max_by_key(|pair| pair.opening.as_str().len())
    }

    fn spelling_at(source: &str, cursor: usize, spelling: &Spelling) -> bool {
        let expected = spelling.as_str();
        let remainder = &source[cursor..];
        if !remainder.starts_with(expected) {
            return false;
        }
        let after = &remainder[expected.len()..];
        !matches!(after.chars().next(), Some(character) if Self::identifier_continue(character))
    }

    fn identifier_end(source: &str, cursor: usize) -> Option<usize> {
        let mut characters = source[cursor..].char_indices();
        let (_, first) = characters.next()?;
        if !Self::identifier_start(first) {
            return None;
        }
        let mut end = cursor + first.len_utf8();
        for (offset, character) in characters {
            if !Self::identifier_continue(character) {
                break;
            }
            end = cursor + offset + character.len_utf8();
        }
        Some(end)
    }

    fn identifier_start(character: char) -> bool {
        character == '_' || character.is_ascii_alphabetic()
    }

    fn identifier_continue(character: char) -> bool {
        Self::identifier_start(character) || character.is_ascii_digit()
    }

    fn skip_whitespace(source: &str, cursor: &mut usize) {
        while let Some(character) = source[*cursor..].chars().next() {
            if !character.is_whitespace() {
                return;
            }
            *cursor += character.len_utf8();
        }
    }

    fn next_character_width(source: &str) -> usize {
        source
            .chars()
            .next()
            .expect("called only before the source end")
            .len_utf8()
    }
}

/// The all-data minimal vocabulary used by the focused witness.
pub fn spike_vocabulary() -> VocabularyDescriptor {
    let round = DelimiterPair::new("(", ")");
    let square = DelimiterPair::new("[", "]");
    let curly = DelimiterPair::new("{", "}");
    let nesting = vec![square.clone(), round.clone(), curly.clone()];
    let item_extent = ItemExtentDescriptor {
        endings: vec![
            ItemEnding::Exact(";".into()),
            ItemEnding::Balanced(curly.clone()),
        ],
        nested_pairs: nesting.clone(),
    };
    let newtype_positions = |visibility| {
        PositionLayout::position(
            PositionRole::NewtypeAttributes,
            PositionForm::RepeatedDelimited {
                boundary: DelimiterPair::new("#[", "]"),
                nested_pairs: nesting.clone(),
            },
            PositionLayout::position(
                PositionRole::NewtypeVisibility,
                visibility,
                PositionLayout::position(
                    PositionRole::NewtypeItemKeyword,
                    PositionForm::Exact("struct".into()),
                    PositionLayout::position(
                        PositionRole::NewtypeTypeName,
                        PositionForm::Identifier,
                        PositionLayout::position(
                            PositionRole::NewtypeParenthesizedTypeReference,
                            PositionForm::Delimited {
                                boundary: round.clone(),
                                nested_pairs: nesting.clone(),
                            },
                            PositionLayout::position(
                                PositionRole::NewtypeTerminator,
                                PositionForm::Exact(";".into()),
                                PositionLayout::End,
                            ),
                        ),
                    ),
                ),
            ),
        )
    };
    VocabularyDescriptor {
        records: vec![
            RecordDescriptor {
                record: RecordType::ProtosPrimitiveRule,
                item_extent: item_extent.clone(),
                positions: PositionLayout::position(
                    PositionRole::ProtosPrimitiveKeyword,
                    PositionForm::Exact("primitive".into()),
                    PositionLayout::position(
                        PositionRole::ProtosPrimitiveName,
                        PositionForm::Identifier,
                        PositionLayout::position(
                            PositionRole::ProtosPrimitiveAssignment,
                            PositionForm::Exact("=".into()),
                            PositionLayout::position(
                                PositionRole::ProtosPrimitiveKind,
                                PositionForm::Exact("scalar".into()),
                                PositionLayout::position(
                                    PositionRole::ProtosPrimitiveTerminator,
                                    PositionForm::Exact(";".into()),
                                    PositionLayout::End,
                                ),
                            ),
                        ),
                    ),
                ),
            },
            RecordDescriptor {
                record: RecordType::RustPrivateNewtypeRule,
                item_extent: item_extent.clone(),
                positions: newtype_positions(PositionForm::Absent),
            },
            RecordDescriptor {
                record: RecordType::RustPublicNewtypeRule,
                item_extent,
                positions: newtype_positions(PositionForm::Exact("pub".into())),
            },
        ],
    }
}
