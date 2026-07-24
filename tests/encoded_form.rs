//! Kernel evidence for the EncodedForm/TextualForm pairing.
//!
//! The typed conversion layer is text-free: source identifiers are retained as
//! identifiers and the target NameTable composes the source slice rather than
//! resolving and re-interning its names.

use name_table::{IdentifierNamespace, Name, NameTable};
use structural_codec::{Converted, EncodedConversion, EncodedForm, TextualForm};

struct SourceLanguage;
#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceForm {
    name: name_table::Identifier,
}
impl EncodedForm for SourceForm {
    type Language = SourceLanguage;
}

struct TargetLanguage;
#[derive(Clone, Debug, PartialEq, Eq)]
struct TargetForm {
    original: name_table::Identifier,
}
impl EncodedForm for TargetForm {
    type Language = TargetLanguage;
}

/// A toy schema-to-Logos conversion. Its body touches neither `str` nor `String`:
/// the source identifier stays intact while the target NameTable borrows the
/// schema slice and owns an empty Logos slice for future boundary allocation.
struct ComposeConversion;

impl EncodedConversion for ComposeConversion {
    type Source = SourceForm;
    type Target = TargetForm;
    type Error = name_table::NameTableError;

    fn convert(
        &self,
        source: &SourceForm,
        names: &NameTable,
    ) -> Result<Converted<TargetForm>, Self::Error> {
        let names = NameTable::new(IdentifierNamespace::Logos).compose(names)?;
        Ok(Converted {
            target: TargetForm {
                original: source.name,
            },
            names,
        })
    }
}

#[test]
fn typed_conversion_borrows_source_names_without_text_or_copying() {
    let mut names = NameTable::new(IdentifierNamespace::Schema);
    let source_name = names
        .intern(Name::new("Source"))
        .expect("schema allocation");
    let source = SourceForm { name: source_name };

    let converted = ComposeConversion.convert(&source, &names).expect("convert");

    assert_eq!(converted.target.original, source_name);
    assert_eq!(
        converted.names.resolve(source_name).unwrap().as_str(),
        "Source"
    );
    assert_eq!(converted.names.len(), 0, "the Logos home starts empty");
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
