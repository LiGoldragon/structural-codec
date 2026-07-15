//! The authoring vocabulary normalizes to kernel forms before hashing or evaluation.

use raw_discovery::Delimiter;
use structural_codec::authoring::{AuthoringForm, DottedForm, ObjectSymbolPrefixedBlock};
use structural_codec::{AtomForm, CaseExpectation, SequenceForm, StructuralForm};

/// `Object.{ Type }` authoring sugar normalizes to `Application(Atom, Delimited)`.
#[test]
fn object_prefixed_block_normalizes_to_application() {
    let authored = AuthoringForm::ObjectPrefixed(ObjectSymbolPrefixedBlock {
        object: AtomForm::with_case(CaseExpectation::PascalCase),
        delimiter: Delimiter::Brace,
        sequence: SequenceForm::Product(vec![StructuralForm::pascal_atom()]),
    });

    let expected = StructuralForm::application(
        StructuralForm::Atom(AtomForm::with_case(CaseExpectation::PascalCase)),
        StructuralForm::Delimited {
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
        segments: vec![
            StructuralForm::pascal_atom(),
            StructuralForm::pascal_atom(),
            StructuralForm::camel_atom(),
        ],
    });

    let expected = StructuralForm::application(
        StructuralForm::pascal_atom(),
        StructuralForm::application(StructuralForm::pascal_atom(), StructuralForm::camel_atom()),
    );
    assert_eq!(authored.normalize(), expected);
}
