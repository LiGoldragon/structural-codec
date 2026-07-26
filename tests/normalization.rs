use crate::authoring::ApplicationDelimitedAuthoring;
use crate::fixture::{APPLICATION_OPERATOR, BRACE_BOUNDARY};
use crate::{AtomCase, AtomDescriptor, LeafCodec, SharedDescriptor, StructuralRule};

#[test]
fn authoring_normalizes_to_an_archived_typed_record_not_a_product() {
    let rule = ApplicationDelimitedAuthoring {
        operator: APPLICATION_OPERATOR,
        boundary: BRACE_BOUNDARY,
        head: AtomDescriptor::with_case(AtomCase::PascalCase),
        element: SharedDescriptor::Leaf(LeafCodec::Integer),
    }
    .normalize();
    assert!(matches!(rule, Ok(StructuralRule::ApplicationDelimited(_))));
}
