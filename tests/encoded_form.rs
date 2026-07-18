//! Kernel-level evidence for the TRUTH side of the Protos pairing: the
//! [`EncodedForm`] marker, the typed [`EncodedConversion`] layer conversion
//! (`EncodedForm<T> -> EncodedForm<X>`, text-free), and the first-class
//! [`TextualForm<T>`] view value the [`Textual`](structural_codec::Textual) produces.
//!
//! These stand the seated surface up in isolation with toy encoded forms, so a real
//! instance (schema→logos through the Nomos macros, in `core-nomos`) inherits a proven
//! contract. The load-bearing proof is structural: the conversion signature carries no
//! `&str` / `String`, so a conversion is a real type conversion with no text on the path.

use name_table::{Name, NameTable};
use structural_codec::{Converted, EncodedConversion, EncodedForm, TextualForm};

/// A toy source encoded form: a stringless value carrying only an interned identifier.
struct SourceLanguage;
#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceForm {
    name: name_table::Identifier,
}
impl EncodedForm for SourceForm {
    type Language = SourceLanguage;
}

/// A toy target encoded form: the same identifier, plus a target-only appended name.
struct TargetLanguage;
#[derive(Clone, Debug, PartialEq, Eq)]
struct TargetForm {
    original: name_table::Identifier,
    appended: name_table::Identifier,
}
impl EncodedForm for TargetForm {
    type Language = TargetLanguage;
}

/// A toy layer conversion: it preserves the source identifier and appends one
/// target-only name into the continuous nametree — the shape the real schema→logos
/// lowering has, with no text anywhere on the path.
struct AppendConversion {
    appended: &'static str,
}

impl EncodedConversion for AppendConversion {
    type Source = SourceForm;
    type Target = TargetForm;
    type Error = std::convert::Infallible;

    fn convert(
        &self,
        source: &SourceForm,
        names: &NameTable,
    ) -> Result<Converted<TargetForm>, Self::Error> {
        let mut extended = NameTable::extend_from(names);
        let appended = extended.intern(Name::new(self.appended));
        Ok(Converted {
            target: TargetForm {
                original: source.name,
                appended,
            },
            names: extended,
        })
    }
}

#[test]
fn typed_conversion_threads_the_continuous_nametree_without_text() {
    let mut names = NameTable::new();
    let source_name = names.intern(Name::new("Source"));
    let source = SourceForm { name: source_name };

    let conversion = AppendConversion { appended: "Target" };
    let converted = conversion.convert(&source, &names).expect("convert");

    // The source index is preserved in the extended, continuous table.
    assert_eq!(converted.target.original, source_name);
    assert_eq!(
        converted.names.resolve(source_name).unwrap().as_str(),
        "Source"
    );
    // The target-only name is resolvable only in the extended table.
    assert_eq!(
        converted
            .names
            .resolve(converted.target.appended)
            .unwrap()
            .as_str(),
        "Target"
    );
}

#[test]
fn textual_form_value_carries_the_single_document_case_trivially() {
    let view: TextualForm<SourceLanguage> = TextualForm::single("hello".to_string());
    assert_eq!(view.chunks().len(), 1);
    assert_eq!(view.sole_text().unwrap(), "hello");
}

#[test]
fn textual_form_multi_chunk_view_refuses_a_sole_text_read() {
    use structural_codec::{ChunkName, TextChunk};
    let view: TextualForm<SourceLanguage> = TextualForm::from_chunks(vec![
        TextChunk {
            name: ChunkName("a.rs".to_string()),
            text: "one".to_string(),
        },
        TextChunk {
            name: ChunkName("b.rs".to_string()),
            text: "two".to_string(),
        },
    ]);
    let error = view.sole_text().expect_err("two chunks has no sole text");
    assert_eq!(error.count, 2);
}
