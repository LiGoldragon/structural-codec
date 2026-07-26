use structural_codec::typed_rust_spike::{
    Attribute, AttributeSet, BuiltinType, Item, ItemName, NewtypePayload, NewtypeRule,
    PrimitiveRule, PrimitiveValue, ProtosPrimitiveVocabulary, RustError, RustVocabulary,
    SharedEvaluator, Symbol, TypeReference, Visibility, WrappedField,
};

fn newtype(name: u16, visibility: Visibility, attribute: Option<Attribute>) -> Item {
    Item::Newtype(NewtypePayload {
        item_name: ItemName(Symbol(name)),
        visibility,
        attributes: AttributeSet(attribute),
        wrapped_field: WrappedField {
            visibility: Visibility::Private,
            type_reference: TypeReference::Builtin(BuiltinType::Integer),
        },
    })
}

#[test]
fn shared_evaluator_drives_primitives_and_typed_rust_records() {
    let primitives = SharedEvaluator::new(ProtosPrimitiveVocabulary);
    let integer = PrimitiveRule {
        spelling: "Integer",
    };
    let boolean = PrimitiveRule {
        spelling: "Boolean",
    };
    assert_eq!(
        primitives.encode(&integer, &PrimitiveValue::Integer),
        Ok("Integer".to_owned())
    );
    assert_eq!(
        primitives.decode(&integer, "Integer"),
        Ok(PrimitiveValue::Integer)
    );
    assert_eq!(primitives.prove_disjoint(&integer, &boolean), Ok(()));

    let rust = SharedEvaluator::new(RustVocabulary::witness());
    let private = NewtypeRule::private();
    let public = NewtypeRule::public();
    let cases = [
        (
            private,
            newtype(1, Visibility::Private, None),
            "struct PrivateInteger(Integer);",
        ),
        (
            private,
            newtype(1, Visibility::Private, Some(Attribute::ReprTransparent)),
            "#[repr(transparent)]\nstruct PrivateInteger(Integer);",
        ),
        (
            public,
            newtype(2, Visibility::Public, None),
            "pub struct PublicInteger(Integer);",
        ),
        (
            public,
            newtype(2, Visibility::Public, Some(Attribute::ReprTransparent)),
            "#[repr(transparent)]\npub struct PublicInteger(Integer);",
        ),
    ];
    for (rule, value, expected_text) in cases {
        let text = rust.encode(&rule, &value).expect("typed newtype encodes");
        assert_eq!(text, expected_text);
        assert_eq!(rust.decode(&rule, &text), Ok(value));
    }
}

#[test]
fn typed_position_prover_never_uses_rule_order() {
    let evaluator = SharedEvaluator::new(RustVocabulary::witness());
    let private = NewtypeRule::private();
    let public = NewtypeRule::public();
    let crate_visible = NewtypeRule::crate_visible();

    // After shared Attributes, private's silent Visibility advances to the
    // typed ItemKeyword `struct`; public remains at typed Visibility `pub`.
    assert_eq!(evaluator.prove_disjoint(&private, &public), Ok(()));
    // `pub` and `pub(crate)` have an overlapping character prefix but distinct
    // complete textual domains, so no authored alternative order is needed.
    assert_eq!(evaluator.prove_disjoint(&public, &crate_visible), Ok(()));
    assert_eq!(
        evaluator.prove_disjoint(&public, &NewtypeRule::public()),
        Err(RustError::AmbiguousDomains)
    );
    let restricted = NewtypeRule {
        visibility: structural_codec::typed_rust_spike::VisibilityPosition {
            accepted: Visibility::Restricted,
        },
        ..NewtypeRule::private()
    };
    assert_eq!(
        evaluator.prove_disjoint(&public, &restricted),
        Err(RustError::UnknownDomain)
    );
}

#[test]
fn boundary_first_grouping_preserves_nested_boundaries_before_inner_interpretation() {
    let source = "#[outer({ [nested(\";\")] })]\n#[repr(transparent)]\npub struct PublicInteger(Integer); trailing";
    let grouped = RustVocabulary::group_newtype_item(source).expect("complete item extent");
    assert_eq!(
        grouped,
        "#[outer({ [nested(\";\")] })]\n#[repr(transparent)]\npub struct PublicInteger(Integer);"
    );
}

#[test]
fn unknown_or_ambiguous_rust_cases_refuse_without_fallback() {
    let evaluator = SharedEvaluator::new(RustVocabulary::witness());
    let public = NewtypeRule::public();
    assert_eq!(
        evaluator.decode(&public, "pub(crate) struct PublicInteger(Integer);"),
        Err(RustError::UnsupportedVisibility)
    );
    assert_eq!(
        RustVocabulary::group_newtype_item("struct PrivateInteger(Integer)"),
        Err(RustError::MissingTerminator)
    );
}
