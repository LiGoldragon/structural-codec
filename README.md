# structural-codec

`structural-codec` is the shared evaluator for archived structural rule records.
Each fixed rule is a real typed Rust struct whose fields are distinct
`Position<Role, Descriptor>` values. The archived position carries its stable role id, while
the evaluator receives only an ephemeral heterogeneous borrowed view for shared
traversal.

The table stores a language dimension, a typed target-layout content identity,
the exact `ContentHash<TokenProfileDomain>`, canonical block-discovery rules,
an explicit per-context textual rendering policy, and a vocabulary identity.
Sealing binds the rules and policy to the exact profile and the table owns the
runtime values. Textual decode discovers source-bounded blocks before one
expectation-driven bounded-cursor traversal; Textual encode follows the same
descriptor/context path and writes only explicit policy-selected profile
spellings. Fixture vocabularies use a separate
identity domain and reserved Schema ids, so they cannot compose with production
Schema sidecars.

There is one evaluator and one conservative disjointness prover.
`ConstructorCodec<R>` and `AddressedStructuralTable<R>` accept any archived
`StructureRecord`; `StructuralRule` is only the built-in convenience vocabulary.
`RuleCoproduct<L, R>` combines downstream record shapes while exposing data
only, so no language-specific grammar interpreter is required. Runtime vectors
are limited to accepted alternatives and explicit repetition; stable
constructor/form identities, never vector position, identify alternatives.

`StructuralValue` is a constructor-tagged role-keyed mirror. Language
`Textual::reify` and `Textual::reflect` remain manual mappings between this
mirror and encoded values; derivation remains deliberately open.

Downstream vocabularies author opaque language-scoped ids through checked
constructors, define their own `FieldRole` markers, and seal a complete typed
table before evaluation. The seal rejects duplicate constructor and decode-form
identities as well as cross-language associations. Manual `Textual::reflect`
uses `StructuralValue::record` to insert values by typed role, and `reify`
retrieves them the same way; neither path writes raw role ids or map entries.

## Build and test

```sh
cargo test
nix flake check
```
