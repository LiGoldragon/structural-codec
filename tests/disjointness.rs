//! The conservative disjointness checker: accepts a pair of decode forms only when
//! it can PROVE no block matches both; unprovable overlap is a hard error.

use structural_codec::fixture::{FIELD, FixtureBuilder};
use structural_codec::{
    AtomForm, CaseExpectation, ConstructorCodec, CoreConstructorId, PositionalSignature,
    ScopedCoreTypeId, StructuralEntry, StructuralForm,
};

fn entry_with_forms(forms: Vec<StructuralForm>) -> StructuralEntry {
    let core_type = ScopedCoreTypeId::fixture(100);
    let constructors = forms
        .into_iter()
        .enumerate()
        .map(|(index, form)| {
            ConstructorCodec::new(
                CoreConstructorId::new(core_type, index as u32),
                vec![form.clone()],
                form,
                PositionalSignature::default(),
            )
        })
        .collect();
    StructuralEntry::new(core_type, constructors)
}

/// The `Field` alternatives (bare `Type` atom versus `name.Type` application) are
/// provably disjoint — different block kinds.
#[test]
fn field_alternatives_are_provably_disjoint() {
    let table = FixtureBuilder::new().build().expect("seal");
    table
        .validate_disjoint()
        .expect("the whole fixture table validates");

    // and specifically the Field entry.
    let field = FixtureBuilder::new()
        .build()
        .expect("seal")
        .entry(FIELD)
        .expect("field entry")
        .clone();
    field.validate_disjoint().expect("field entry validates");
}

/// Two atoms of DIFFERENT concrete case are provably disjoint.
#[test]
fn distinct_atom_cases_are_disjoint() {
    let entry = entry_with_forms(vec![
        StructuralForm::Atom(AtomForm::with_case(CaseExpectation::PascalCase)),
        StructuralForm::Atom(AtomForm::with_case(CaseExpectation::CamelCase)),
    ]);
    entry.validate_disjoint().expect("distinct cases disjoint");
}

/// Two atoms of the SAME case overlap — the checker cannot prove them distinct, so it
/// errors (conservative-safe).
#[test]
fn identical_atom_cases_are_rejected() {
    let entry = entry_with_forms(vec![
        StructuralForm::Atom(AtomForm::with_case(CaseExpectation::PascalCase)),
        StructuralForm::Atom(AtomForm::with_case(CaseExpectation::PascalCase)),
    ]);
    assert!(
        entry.validate_disjoint().is_err(),
        "overlapping atom cases must be rejected"
    );
}

/// Delegate forms are opaque — their matchable block kind is unknown — so a pair of
/// them can never be proven disjoint and is rejected.
#[test]
fn delegate_forms_are_conservatively_rejected() {
    let entry = entry_with_forms(vec![
        StructuralForm::Delegate(ScopedCoreTypeId::fixture(200)),
        StructuralForm::Delegate(ScopedCoreTypeId::fixture(201)),
    ]);
    assert!(
        entry.validate_disjoint().is_err(),
        "opaque delegate forms must be conservatively rejected"
    );
}

/// An atom versus an application of a distinguishing head is disjoint by block kind.
#[test]
fn atom_versus_application_is_disjoint() {
    let entry = entry_with_forms(vec![
        StructuralForm::pascal_atom(),
        StructuralForm::application(StructuralForm::camel_atom(), StructuralForm::pascal_atom()),
    ]);
    entry
        .validate_disjoint()
        .expect("atom and application are disjoint");
}
