//! Focused disposable witness for the R3 typed-kernel spike.

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion, PortableArchive};
use name_table::Identifier;
use structural_codec::r3_typed_kernel_spike::{
    PositionRole, RecordType, SharedEvaluator, SpikeError, StructureTree, VocabularyDescriptor,
    spike_vocabulary,
};

struct R3TypedTreeDomain;

impl HashDomain for R3TypedTreeDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "disposable R3 typed-kernel viability spike",
            layout: LayoutVersion::new(1),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Clone, Debug, Eq, PartialEq)]
struct ArchivedBoundary {
    language_identifier: Identifier,
    tree_identity: ContentHash<R3TypedTreeDomain>,
}

fn descriptor(record: RecordType) -> structural_codec::r3_typed_kernel_spike::RecordDescriptor {
    spike_vocabulary()
        .records
        .into_iter()
        .find(|descriptor| descriptor.record == record)
        .expect("fixture contains requested descriptor")
}

#[test]
fn generic_evaluator_decodes_encodes_and_archives_private_public_and_protos_records() {
    let vocabulary = spike_vocabulary();
    let vocabulary_bytes = vocabulary
        .to_archive_bytes()
        .expect("archive pure-data vocabulary");
    let vocabulary = VocabularyDescriptor::from_archive_bytes(vocabulary_bytes.as_ref())
        .expect("restore vocabulary");
    let private = SharedEvaluator::decode(
        &vocabulary,
        RecordType::RustPrivateNewtypeRule,
        "#[outer(inner{value})] struct Wrapper(Inner);",
    )
    .expect("private typed newtype");
    let public = SharedEvaluator::decode(
        &vocabulary,
        RecordType::RustPublicNewtypeRule,
        "#[outer(inner{value})] pub struct Wrapper(Inner);",
    )
    .expect("public typed newtype");
    let primitive = SharedEvaluator::decode(
        &vocabulary,
        RecordType::ProtosPrimitiveRule,
        "primitive Integer = scalar;",
    )
    .expect("typed Protos primitive");

    assert!(matches!(
        private
            .tree
            .as_record_positions()
            .value(PositionRole::NewtypeVisibility),
        Some(StructureTree::Absent)
    ));
    assert!(matches!(
        public
            .tree
            .as_record_positions()
            .value(PositionRole::NewtypeVisibility),
        Some(StructureTree::Text(spelling)) if spelling.as_str() == "pub"
    ));
    assert!(matches!(
        primitive
            .tree
            .as_record_positions()
            .value(PositionRole::ProtosPrimitiveName),
        Some(StructureTree::Text(spelling)) if spelling.as_str() == "Integer"
    ));

    let encoded = SharedEvaluator::encode(&vocabulary, &public.tree).expect("generic encode");
    let decoded_again =
        SharedEvaluator::decode(&vocabulary, RecordType::RustPublicNewtypeRule, &encoded)
            .expect("generic decode after generic encode");
    assert_eq!(decoded_again.tree, public.tree);

    let bytes = public
        .tree
        .to_archive_bytes()
        .expect("archive structure tree");
    let restored =
        StructureTree::from_archive_bytes(bytes.as_ref()).expect("restore structure tree");
    assert_eq!(restored, public.tree);
}

#[test]
fn newtype_has_six_named_typed_positions_without_a_product_collection() {
    let record = descriptor(RecordType::RustPublicNewtypeRule);
    let attributes = &record.positions;
    let visibility = attributes.next().expect("visibility position");
    let keyword = visibility.next().expect("keyword position");
    let type_name = keyword.next().expect("name position");
    let type_reference = type_name.next().expect("type-reference position");
    let terminator = type_reference.next().expect("terminator position");

    assert_eq!(attributes.role(), Some(PositionRole::NewtypeAttributes));
    assert_eq!(visibility.role(), Some(PositionRole::NewtypeVisibility));
    assert_eq!(keyword.role(), Some(PositionRole::NewtypeItemKeyword));
    assert_eq!(type_name.role(), Some(PositionRole::NewtypeTypeName));
    assert_eq!(
        type_reference.role(),
        Some(PositionRole::NewtypeParenthesizedTypeReference)
    );
    assert_eq!(terminator.role(), Some(PositionRole::NewtypeTerminator));
    assert!(
        terminator
            .next()
            .expect("typed layout end")
            .role()
            .is_none()
    );
}

#[test]
fn boundary_first_extent_preserves_nested_attribute_paren_and_brace_structure() {
    let descriptor = descriptor(RecordType::RustPublicNewtypeRule);
    let semicolon_source = "#[outer(inner{value})] pub struct Wrapper(Inner); trailing";
    let semicolon_extent = SharedEvaluator::item_extent(&descriptor.item_extent, semicolon_source)
        .expect("semicolon extent");
    assert_eq!(
        &semicolon_source[..semicolon_extent.end()],
        "#[outer(inner{value})] pub struct Wrapper(Inner);"
    );

    let brace_source = "pub struct Unit { field: (Inner) } trailing";
    let brace_extent =
        SharedEvaluator::item_extent(&descriptor.item_extent, brace_source).expect("brace extent");
    assert_eq!(
        &brace_source[..brace_extent.end()],
        "pub struct Unit { field: (Inner) }"
    );
}

#[test]
fn typed_position_proof_names_roles_and_refuses_unproven_overlap() {
    let private = descriptor(RecordType::RustPrivateNewtypeRule);
    let public = descriptor(RecordType::RustPublicNewtypeRule);
    assert_eq!(
        SharedEvaluator::prove_disjoint(&private, &public).expect("private and public differ"),
        structural_codec::r3_typed_kernel_spike::TypedPositionDisjointness {
            left: PositionRole::NewtypeVisibility,
            right: PositionRole::NewtypeVisibility,
        }
    );
    assert_eq!(
        SharedEvaluator::prove_disjoint(&public, &public),
        Err(SpikeError::NotProvablyDisjoint)
    );
}

#[test]
fn r4_variant_wrapped_u16_identifier_and_typed_hash_boundary_archive() {
    let vocabulary = spike_vocabulary();
    let tree = SharedEvaluator::decode(
        &vocabulary,
        RecordType::ProtosPrimitiveRule,
        "primitive Integer = scalar;",
    )
    .expect("typed primitive");
    let tree_identity =
        ContentHash::<R3TypedTreeDomain>::of_core(&tree.tree).expect("typed tree identity");
    let boundary = ArchivedBoundary {
        language_identifier: Identifier::Logos(17),
        tree_identity,
    };

    let bytes = boundary.to_archive_bytes().expect("archive typed boundary");
    let restored =
        ArchivedBoundary::from_archive_bytes(bytes.as_ref()).expect("restore typed boundary");
    assert_eq!(restored, boundary);
    assert_eq!(restored.language_identifier.local(), 17);
}

trait RecordTree {
    fn as_record_positions(&self) -> &structural_codec::r3_typed_kernel_spike::PositionValues;
}

impl RecordTree for StructureTree {
    fn as_record_positions(&self) -> &structural_codec::r3_typed_kernel_spike::PositionValues {
        match self {
            StructureTree::Record { positions, .. } => positions,
            _ => panic!("decoded record must carry typed positions"),
        }
    }
}
