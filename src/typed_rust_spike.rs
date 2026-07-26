//! Disposable Slice 0 witness: a second vocabulary driven by the same generic
//! evaluator object as a tiny Protos-primitive adapter.  This is proposed API
//! only; published `StructuralEvaluator` remains untouched in this workspace.

use std::collections::BTreeMap;

/// The single evaluator extension point.  It contains no language branch or
/// Rust-item match arm: a vocabulary owns its typed rule data and conversions.
pub trait DrivenVocabulary {
    type Rule;
    type Value: Clone + Eq + std::fmt::Debug;
    type Error: Clone + Eq + std::fmt::Debug;

    fn encode(&self, rule: &Self::Rule, value: &Self::Value) -> Result<String, Self::Error>;
    fn decode(&self, rule: &Self::Rule, source: &str) -> Result<Self::Value, Self::Error>;
    fn prove_disjoint(&self, left: &Self::Rule, right: &Self::Rule) -> Result<(), Self::Error>;
}

/// One shared driven evaluator.  Production primitives and the proposed Rust
/// vocabulary enter only through `DrivenVocabulary`.
#[derive(Clone, Debug)]
pub struct SharedEvaluator<V> {
    vocabulary: V,
}

impl<V> SharedEvaluator<V> {
    pub fn new(vocabulary: V) -> Self {
        Self { vocabulary }
    }
}

impl<V: DrivenVocabulary> SharedEvaluator<V> {
    pub fn encode(&self, rule: &V::Rule, value: &V::Value) -> Result<String, V::Error> {
        self.vocabulary.encode(rule, value)
    }

    pub fn decode(&self, rule: &V::Rule, source: &str) -> Result<V::Value, V::Error> {
        self.vocabulary.decode(rule, source)
    }

    pub fn prove_disjoint(&self, left: &V::Rule, right: &V::Rule) -> Result<(), V::Error> {
        self.vocabulary.prove_disjoint(left, right)
    }
}

/// A minimal existing-style primitive adapter.  It is intentionally data driven
/// so this witness can show the same `SharedEvaluator` executes both vocabularies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitiveRule {
    pub spelling: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimitiveValue {
    Integer,
    Boolean,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimitiveError {
    RuleValueMismatch,
    UnexpectedText,
    Overlap,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProtosPrimitiveVocabulary;

impl DrivenVocabulary for ProtosPrimitiveVocabulary {
    type Rule = PrimitiveRule;
    type Value = PrimitiveValue;
    type Error = PrimitiveError;

    fn encode(
        &self,
        rule: &PrimitiveRule,
        value: &PrimitiveValue,
    ) -> Result<String, PrimitiveError> {
        let expected = match value {
            PrimitiveValue::Integer => "Integer",
            PrimitiveValue::Boolean => "Boolean",
        };
        (rule.spelling == expected)
            .then(|| rule.spelling.to_owned())
            .ok_or(PrimitiveError::RuleValueMismatch)
    }

    fn decode(&self, rule: &PrimitiveRule, source: &str) -> Result<PrimitiveValue, PrimitiveError> {
        if source != rule.spelling {
            return Err(PrimitiveError::UnexpectedText);
        }
        match rule.spelling {
            "Integer" => Ok(PrimitiveValue::Integer),
            "Boolean" => Ok(PrimitiveValue::Boolean),
            _ => Err(PrimitiveError::UnexpectedText),
        }
    }

    fn prove_disjoint(
        &self,
        left: &PrimitiveRule,
        right: &PrimitiveRule,
    ) -> Result<(), PrimitiveError> {
        (left.spelling != right.spelling)
            .then_some(())
            .ok_or(PrimitiveError::Overlap)
    }
}

/// A stringless symbol reference.  Text lives in the vocabulary's external name
/// map, just as names live outside an encoded form in the name table.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Symbol(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemName(pub Symbol);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Crate,
    Restricted,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinType {
    Integer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeReference {
    Builtin(BuiltinType),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Attribute {
    ReprTransparent,
}

/// Slice 0 deliberately supports zero or one typed attribute without storing
/// syntax text or a generic structural-form list in the encoded value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttributeSet(pub Option<Attribute>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrappedField {
    pub visibility: Visibility,
    pub type_reference: TypeReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NewtypePayload {
    pub item_name: ItemName,
    pub visibility: Visibility,
    pub attributes: AttributeSet,
    pub wrapped_field: WrappedField,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Item {
    Newtype(NewtypePayload),
}

/// Each grammar position has its own type.  `NewtypeRule` is a record, never a
/// product vector: its Rust order is expressed by fields, not literal indices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attributes {
    pub maximum: One,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum One {
    One,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibilityPosition {
    pub accepted: Visibility,
}

/// Spelling is data on the typed rule entry, never an untyped product member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKeyword {
    Struct { spelling: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeName;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parenthesized<T> {
    pub inner: T,
    pub opening: &'static str,
    pub closing: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Terminator {
    pub spelling: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NewtypeRule {
    pub attributes: Attributes,
    pub visibility: VisibilityPosition,
    pub item_keyword: ItemKeyword,
    pub type_name: TypeName,
    pub wrapped: Parenthesized<TypeReference>,
    pub terminator: Terminator,
}

impl NewtypeRule {
    pub const fn private() -> Self {
        Self {
            attributes: Attributes { maximum: One::One },
            visibility: VisibilityPosition {
                accepted: Visibility::Private,
            },
            item_keyword: ItemKeyword::Struct { spelling: "struct" },
            type_name: TypeName,
            wrapped: Parenthesized {
                inner: TypeReference::Builtin(BuiltinType::Integer),
                opening: "(",
                closing: ")",
            },
            terminator: Terminator { spelling: ";" },
        }
    }

    pub const fn public() -> Self {
        Self {
            visibility: VisibilityPosition {
                accepted: Visibility::Public,
            },
            ..Self::private()
        }
    }

    pub const fn crate_visible() -> Self {
        Self {
            visibility: VisibilityPosition {
                accepted: Visibility::Crate,
            },
            ..Self::private()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextDomain {
    Exact(&'static str),
    Unknown,
}

impl VisibilityPosition {
    /// Private visibility has no spelling.  For proof it advances to the typed
    /// keyword position; public and crate visibility own their spelling here.
    const fn emitted_domain(self) -> Option<TextDomain> {
        match self.accepted {
            Visibility::Private => None,
            Visibility::Public => Some(TextDomain::Exact("pub")),
            Visibility::Crate => Some(TextDomain::Exact("pub(crate)")),
            Visibility::Restricted => Some(TextDomain::Unknown),
        }
    }
}

impl ItemKeyword {
    const fn text_domain(self) -> TextDomain {
        match self {
            Self::Struct { spelling } => TextDomain::Exact(spelling),
        }
    }

    const fn spelling(self) -> &'static str {
        match self {
            Self::Struct { spelling } => spelling,
        }
    }
}

impl NewtypeRule {
    /// This preserves the record's typed fields while comparing the first emitted
    /// head domain: private reaches `ItemKeyword`, public stops at `Visibility`.
    const fn first_emitted_head_domain(self) -> TextDomain {
        match self.visibility.emitted_domain() {
            Some(domain) => domain,
            None => self.item_keyword.text_domain(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustError {
    RuleValueMismatch,
    UnknownName,
    UnsupportedAttribute,
    UnsupportedVisibility,
    UnsupportedType,
    UnexpectedHead,
    MissingTerminator,
    UnterminatedBoundary,
    MismatchedBoundary,
    AmbiguousDomains,
    UnknownDomain,
}

/// The proposed second structuretree vocabulary.  The external names stand in
/// for the composed name table; `Item` itself remains fully typed and stringless.
#[derive(Clone, Debug)]
pub struct RustVocabulary {
    names: BTreeMap<Symbol, &'static str>,
    reverse_names: BTreeMap<&'static str, Symbol>,
}

impl RustVocabulary {
    pub fn witness() -> Self {
        let names = BTreeMap::from([(Symbol(1), "PrivateInteger"), (Symbol(2), "PublicInteger")]);
        let reverse_names = names
            .iter()
            .map(|(symbol, text)| (*text, *symbol))
            .collect();
        Self {
            names,
            reverse_names,
        }
    }

    fn name(&self, name: ItemName) -> Result<&'static str, RustError> {
        self.names
            .get(&name.0)
            .copied()
            .ok_or(RustError::UnknownName)
    }

    fn visibility_spelling(visibility: Visibility) -> Result<&'static str, RustError> {
        match visibility {
            Visibility::Private => Ok(""),
            Visibility::Public => Ok("pub "),
            Visibility::Crate | Visibility::Restricted => Err(RustError::UnsupportedVisibility),
        }
    }

    fn type_spelling(reference: TypeReference) -> Result<&'static str, RustError> {
        match reference {
            TypeReference::Builtin(BuiltinType::Integer) => Ok("Integer"),
        }
    }

    fn attribute_spelling(attributes: AttributeSet) -> Result<&'static str, RustError> {
        match attributes.0 {
            None => Ok(""),
            Some(Attribute::ReprTransparent) => Ok("#[repr(transparent)]\n"),
        }
    }

    fn accepts(rule: &NewtypeRule, payload: NewtypePayload) -> bool {
        rule.visibility.accepted == payload.visibility
            && rule.wrapped.inner == payload.wrapped_field.type_reference
            && payload.wrapped_field.visibility == Visibility::Private
            && matches!(rule.attributes.maximum, One::One)
    }

    fn prove_domains(left: TextDomain, right: TextDomain) -> Result<(), RustError> {
        match (left, right) {
            (TextDomain::Unknown, _) | (_, TextDomain::Unknown) => Err(RustError::UnknownDomain),
            (TextDomain::Exact(left), TextDomain::Exact(right)) if left != right => Ok(()),
            (TextDomain::Exact(_), TextDomain::Exact(_)) => Err(RustError::AmbiguousDomains),
        }
    }

    /// Boundary-first item extent.  It consumes only typed outer attributes,
    /// then the typed head, then its matching parenthesis and semicolon.  Attribute
    /// interiors are carried through untouched and nested (), [], and {} balance.
    pub fn group_newtype_item(source: &str) -> Result<&str, RustError> {
        let mut offset = Self::skip_outer_attributes(source, 0)?;
        offset = Self::skip_space(source, offset);
        if source[offset..].starts_with("pub") {
            let after = offset + "pub".len();
            if source[after..].starts_with(char::is_whitespace) {
                offset = Self::skip_space(source, after);
            } else if source.as_bytes().get(after) == Some(&b'(') {
                offset = Self::skip_space(source, Self::balanced_end(source, after)?);
            }
        }
        if !Self::starts_word(source, offset, "struct") {
            return Err(RustError::UnexpectedHead);
        }
        offset += "struct".len();
        offset = Self::skip_space(source, offset);
        let name_end = Self::scan_identifier(source, offset).ok_or(RustError::UnexpectedHead)?;
        offset = Self::skip_space(source, name_end);
        if source.as_bytes().get(offset) != Some(&b'(') {
            return Err(RustError::UnexpectedHead);
        }
        offset = Self::balanced_end(source, offset)?;
        offset = Self::skip_space(source, offset);
        if source.as_bytes().get(offset) != Some(&b';') {
            return Err(RustError::MissingTerminator);
        }
        Ok(&source[..offset + 1])
    }

    fn skip_outer_attributes(source: &str, mut offset: usize) -> Result<usize, RustError> {
        loop {
            offset = Self::skip_space(source, offset);
            if !source[offset..].starts_with("#[") {
                return Ok(offset);
            }
            offset = Self::balanced_end(source, offset + 1)?;
        }
    }

    fn skip_space(source: &str, mut offset: usize) -> usize {
        while let Some(character) = source[offset..].chars().next() {
            if !character.is_whitespace() {
                break;
            }
            offset += character.len_utf8();
        }
        offset
    }

    fn starts_word(source: &str, offset: usize, word: &str) -> bool {
        source[offset..].starts_with(word)
            && source[offset + word.len()..]
                .chars()
                .next()
                .is_none_or(|character| !Self::identifier_continue(character))
    }

    fn scan_identifier(source: &str, offset: usize) -> Option<usize> {
        let mut end = offset;
        let mut characters = source[offset..].chars();
        let first = characters.next()?;
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return None;
        }
        end += first.len_utf8();
        for character in characters {
            if !Self::identifier_continue(character) {
                break;
            }
            end += character.len_utf8();
        }
        Some(end)
    }

    const fn identifier_continue(character: char) -> bool {
        character == '_' || character.is_ascii_alphanumeric()
    }

    fn balanced_end(source: &str, opening: usize) -> Result<usize, RustError> {
        let mut stack = Vec::new();
        let mut quote = None;
        let mut escaped = false;
        for (relative, character) in source[opening..].char_indices() {
            let index = opening + relative;
            if let Some(active_quote) = quote {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == active_quote {
                    quote = None;
                }
                continue;
            }
            if matches!(character, '\'' | '"') {
                quote = Some(character);
                continue;
            }
            match character {
                '(' => stack.push(')'),
                '[' => stack.push(']'),
                '{' => stack.push('}'),
                ')' | ']' | '}' => {
                    let expected = stack.pop().ok_or(RustError::MismatchedBoundary)?;
                    if character != expected {
                        return Err(RustError::MismatchedBoundary);
                    }
                    if stack.is_empty() {
                        return Ok(index + character.len_utf8());
                    }
                }
                _ => {}
            }
        }
        Err(RustError::UnterminatedBoundary)
    }

    fn parse_attributes(source: &str) -> Result<(AttributeSet, usize), RustError> {
        let mut offset = 0;
        let mut attribute = None;
        loop {
            offset = Self::skip_space(source, offset);
            if !source[offset..].starts_with("#[") {
                return Ok((AttributeSet(attribute), offset));
            }
            if attribute.is_some() {
                return Err(RustError::UnsupportedAttribute);
            }
            let end = Self::balanced_end(source, offset + 1)?;
            attribute = match &source[offset..end] {
                "#[repr(transparent)]" => Some(Attribute::ReprTransparent),
                _ => return Err(RustError::UnsupportedAttribute),
            };
            offset = end;
        }
    }
}

impl DrivenVocabulary for RustVocabulary {
    type Rule = NewtypeRule;
    type Value = Item;
    type Error = RustError;

    fn encode(&self, rule: &NewtypeRule, value: &Item) -> Result<String, RustError> {
        let Item::Newtype(payload) = value;
        if !Self::accepts(rule, *payload) {
            return Err(RustError::RuleValueMismatch);
        }
        Ok(format!(
            "{}{}{} {}{}{}{}",
            Self::attribute_spelling(payload.attributes)?,
            Self::visibility_spelling(payload.visibility)?,
            rule.item_keyword.spelling(),
            self.name(payload.item_name)?,
            rule.wrapped.opening,
            Self::type_spelling(payload.wrapped_field.type_reference)?,
            rule.wrapped.closing,
        ) + rule.terminator.spelling)
    }

    fn decode(&self, rule: &NewtypeRule, source: &str) -> Result<Item, RustError> {
        let grouped = Self::group_newtype_item(source)?;
        if grouped.len() != source.len() {
            return Err(RustError::UnexpectedHead);
        }
        let (attributes, mut offset) = Self::parse_attributes(grouped)?;
        offset = Self::skip_space(grouped, offset);
        let visibility = if grouped[offset..].starts_with("pub ") {
            offset = Self::skip_space(grouped, offset + "pub".len());
            Visibility::Public
        } else if grouped[offset..].starts_with("pub(") {
            return Err(RustError::UnsupportedVisibility);
        } else {
            Visibility::Private
        };
        if !Self::starts_word(grouped, offset, rule.item_keyword.spelling()) {
            return Err(RustError::UnexpectedHead);
        }
        offset = Self::skip_space(grouped, offset + rule.item_keyword.spelling().len());
        let name_end = Self::scan_identifier(grouped, offset).ok_or(RustError::UnexpectedHead)?;
        let name = self
            .reverse_names
            .get(&grouped[offset..name_end])
            .copied()
            .ok_or(RustError::UnknownName)?;
        offset = Self::skip_space(grouped, name_end);
        if !grouped[offset..].starts_with(rule.wrapped.opening) {
            return Err(RustError::UnexpectedHead);
        }
        let wrapped_end = Self::balanced_end(grouped, offset)?;
        let inner_start = offset + rule.wrapped.opening.len();
        let inner_end = wrapped_end - rule.wrapped.closing.len();
        if grouped[inner_start..inner_end].trim() != Self::type_spelling(rule.wrapped.inner)? {
            return Err(RustError::UnsupportedType);
        }
        if !grouped[wrapped_end..]
            .trim_start()
            .starts_with(rule.terminator.spelling)
        {
            return Err(RustError::MissingTerminator);
        }
        let item = Item::Newtype(NewtypePayload {
            item_name: ItemName(name),
            visibility,
            attributes,
            wrapped_field: WrappedField {
                visibility: Visibility::Private,
                type_reference: rule.wrapped.inner,
            },
        });
        match item {
            Item::Newtype(payload) if Self::accepts(rule, payload) => Ok(item),
            Item::Newtype(_) => Err(RustError::RuleValueMismatch),
        }
    }

    fn prove_disjoint(&self, left: &NewtypeRule, right: &NewtypeRule) -> Result<(), RustError> {
        Self::prove_domains(
            left.first_emitted_head_domain(),
            right.first_emitted_head_domain(),
        )
    }
}
