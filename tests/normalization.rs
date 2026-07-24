//! The authoring vocabulary normalizes to kernel forms before hashing or evaluation.

use raw_discovery::Delimiter;
use structural_codec::authoring::{AuthoringForm, DottedForm, ObjectSymbolPrefixedBlock};
use structural_codec::fixture::{APPLICATION_OPERATOR, BRACE_BOUNDARY};
use structural_codec::{AtomCase, AtomForm, SequenceForm, StructuralForm};

/// `Object.{ Type }` authoring sugar normalizes to `Application(Atom, Delimited)`.
#[test]
fn object_prefixed_block_normalizes_to_application() {
    let authored = AuthoringForm::ObjectPrefixed(ObjectSymbolPrefixedBlock {
        object: AtomForm::with_case(AtomCase::PascalCase),
        operator: APPLICATION_OPERATOR,
        boundary: BRACE_BOUNDARY,
        delimiter: Delimiter::Brace,
        sequence: SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
    });

    let expected = StructuralForm::application(
        APPLICATION_OPERATOR,
        StructuralForm::Atom(AtomForm::with_case(AtomCase::PascalCase)),
        StructuralForm::Delimited {
            boundary: BRACE_BOUNDARY,
            delimiter: Delimiter::Brace,
            sequence: SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
        },
    );
    assert_eq!(authored.normalize(), expected);
}

/// A dotted run normalizes to a right-associative application chain.
#[test]
fn dotted_form_normalizes_to_right_associative_chain() {
    let authored = AuthoringForm::Dotted(DottedForm {
        operator: APPLICATION_OPERATOR,
        head: StructuralForm::pascal_atom(),
        payload: StructuralForm::pascal_atom(),
        continuation: vec![StructuralForm::camel_atom()],
    });

    let expected = StructuralForm::application(
        APPLICATION_OPERATOR,
        StructuralForm::pascal_atom(),
        StructuralForm::application(
            APPLICATION_OPERATOR,
            StructuralForm::pascal_atom(),
            StructuralForm::camel_atom(),
        ),
    );
    assert_eq!(authored.normalize(), expected);
}
