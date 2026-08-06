//! Evaluator-local atom evidence extracted from a source-bounded region.

/// A scalar spelling considered by the structural evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Atom {
    text: String,
}

impl Atom {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn qualifies_as_symbol(&self) -> bool {
        !self.text.is_empty()
            && self.text.chars().all(|character| {
                !character.is_whitespace()
                    && !matches!(
                        character,
                        '"' | '.' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '“' | '”' | '|'
                    )
            })
    }

    fn is_pascal_case(&self) -> bool {
        self.qualifies_as_symbol()
            && self
                .text
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
            && !self.text.contains('-')
    }

    fn is_camel_case(&self) -> bool {
        self.qualifies_as_symbol()
            && self
                .text
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_lowercase())
            && !self.text.contains('-')
    }

    fn is_kebab_case(&self) -> bool {
        self.qualifies_as_symbol() && self.text.contains('-')
    }
}

/// A structural atom-case constraint, with no language meaning attached.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomCase {
    Symbol,
    PascalCase,
    CamelCase,
    KebabCase,
}

impl AtomCase {
    pub fn of(atom: &Atom) -> Self {
        if atom.is_pascal_case() {
            Self::PascalCase
        } else if atom.is_camel_case() {
            Self::CamelCase
        } else if atom.is_kebab_case() {
            Self::KebabCase
        } else {
            Self::Symbol
        }
    }

    pub fn matches(self, atom: &Atom) -> bool {
        self == Self::of(atom)
    }
}
