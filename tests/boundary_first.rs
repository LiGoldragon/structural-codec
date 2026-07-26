use std::collections::BTreeMap;

use crate::fixture::{BRACE_BOUNDARY, COMMIT_SEQUENCE, FixtureBuilder};
use crate::{
    AcceptedDecodeForm, AddressedStructuralTable, ApplicationDelimitedRule, AtomCase,
    AtomDescriptor, ConstructorCodec, DecodeError, DecodeFormId, EncodedConstructorId,
    EncodedLanguage, SharedDescriptor, StructuralEntry, StructuralEvaluator, StructuralRule,
    StructuralVocabularyIdentity, TableIdentityPayload, TargetLayoutIdentity,
};
use name_table::{IdentifierNamespace, NameTable};
use raw_discovery::{
    BlockPrefixAttachment, BlockPrefixRule, BlockTree, BlockTreeDiscoveryConfiguration,
    BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext, BoundaryDiscoveryContextIdentifier,
    BoundaryDiscoveryTransition, CharacterClass, CharacterSet, DiscoveredBlockTree,
    ProfileRevision, TokenProfileData, Trigger, TriggerDefinition, TriggerIdentifier, TriggerSet,
};

#[test]
fn enclosing_nested_tree_is_complete_before_typed_interior_refusal() {
    let table = FixtureBuilder::new().build().expect("table");
    let source = "CommitSequence.{ (wrong[deep]) ;; (ignored [ignored])\n (| ] } ( [ |) }";
    let tree =
        DiscoveredBlockTree::discover(source, table.token_profile(), table.block_discovery())
            .expect("all configured boundaries discovered before typed evaluation");
    let outer = tree.root_blocks().first().expect("prefixed outer block");
    let nested = outer.children().first().expect("nested parenthesis");

    assert_eq!(outer.source_bound().start(), 0);
    assert_eq!(outer.source_bound().end(), source.len());
    assert_eq!(outer.cue().evidence(), BRACE_BOUNDARY);
    assert_eq!(
        outer.children().len(),
        1,
        "carrier and comment contents stay opaque"
    );
    assert_eq!(
        nested.children().len(),
        1,
        "nested square block is already present"
    );

    let evaluator = StructuralEvaluator::new(&table).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    assert!(matches!(
        evaluator.decode_text(COMMIT_SEQUENCE, source, &mut names),
        Err(DecodeError::NoAlternative { .. })
    ));
}

#[test]
fn configured_angle_boundary_decodes_and_encodes_without_a_compatibility_delimiter() {
    let angle = TriggerIdentifier::new(11);
    let dot = TriggerIdentifier::new(12);
    let whitespace = TriggerIdentifier::new(13);
    let profile = TokenProfileData::new(
        ProfileRevision::new(41),
        vec![
            TriggerDefinition {
                identifier: angle,
                trigger: Trigger::Boundary {
                    opening: "<".to_owned(),
                    closing: ">".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: dot,
                trigger: Trigger::Application {
                    glyph: ".".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: whitespace,
                trigger: Trigger::Whitespace {
                    canonical_spelling: " ".to_owned(),
                },
            },
        ],
        TriggerSet::new(vec![angle, dot, whitespace]),
        CharacterSet::from_text(""),
    )
    .seal()
    .expect("angle profile");
    let root = BoundaryDiscoveryContextIdentifier::new(1);
    let discovery = BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::new(vec![angle, whitespace]),
            )],
            vec![BoundaryDiscoveryTransition::new(root, angle, root)],
        ),
        vec![BlockPrefixAttachment::new(
            angle,
            BlockPrefixRule::new(".", CharacterClass::AsciiAlphabetic),
        )],
    );
    let rule = StructuralRule::ApplicationDelimited(
        ApplicationDelimitedRule::new(
            dot,
            angle,
            SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
            SharedDescriptor::Atom(AtomDescriptor::with_case(AtomCase::PascalCase)),
            1,
            Some(1),
        )
        .expect("typed rule"),
    );
    let entry = StructuralEntry::new(
        COMMIT_SEQUENCE,
        vec![ConstructorCodec::new(
            EncodedConstructorId::fixture_schema(COMMIT_SEQUENCE, 1),
            vec![AcceptedDecodeForm::new(DecodeFormId::new(1), rule.clone())],
            rule,
        )],
    );
    let table = AddressedStructuralTable::seal(
        TableIdentityPayload::new(
            EncodedLanguage::Schema,
            TargetLayoutIdentity::derive(b"angle boundary encoded layout"),
            profile.identity(),
            StructuralVocabularyIdentity::fixture(b"angle boundary vocabulary"),
            discovery,
            crate::TextualRenderingPolicy::new(vec![crate::ContextualTextualPolicy::new(
                root,
                Some(whitespace),
                None,
            )]),
            BTreeMap::from([(COMMIT_SEQUENCE, entry)]),
        ),
        &profile,
    )
    .expect("angle table");
    let evaluator = StructuralEvaluator::new(&table).expect("table evaluator");
    let mut names = NameTable::new(IdentifierNamespace::Fixture);
    let value = evaluator
        .decode_text(COMMIT_SEQUENCE, "CommitSequence.<Integer>", &mut names)
        .expect("angle decode");

    assert_eq!(
        evaluator
            .encode_text(COMMIT_SEQUENCE, &value, &names)
            .expect("angle encode"),
        "CommitSequence.<Integer>"
    );
}
