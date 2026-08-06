# structural-codec architecture

The crate owns archived structural rule records, one conservative
disjointness prover, and one shared evaluator. It does not own a language
parser, identity allocator, name-table writer, or second language-specific
interpreter.

## Strict encoded bodies

`EncodedForm` is the narrow truth-side contract reinstated by the 2026-08-07
`TrueNamed` ruling. An implementation is the strict, name-free encoded value
itself: `EncodedForm: TrueNamed`, so its identity is the existing portable
rkyv wire bytes. It carries no conversion, textual-projection, or wrapper
layer. Its own `TextualName` is external metadata; references in the strict
body are `EncodedName` values.

## Typed rules

Fixed positions are actual archived record fields.
`Position<Role, Root, Descriptor>` stores a descriptor and the stable identity
of its typed role. `FieldLink` is only a borrowed traversal view and cannot be
archived.

`OrderedProduct` links those real typed fields in source order. Its links can
only be minted from `FieldRole` types, and sealing requires every member to be
a delegated expected type. Decode consumes exactly one discovered sibling
block per member; consumers retrieve each result and source bound by its role,
never through an indexed root vector.

`OrderedSequence` links mixed lexical and bounded typed positions inside one
source bound. The shared evaluator advances those positions in declared order,
including bounded optional repetition, and encoding follows the same links in
reverse. It is the complete-record surface for languages such as Rust whose
item positions are not all sibling blocks.

All table, proof, decode, and encode machinery is generic over a caller-supplied
language marker and a `StructureRecord<Language>`. `StructuralRule<Language>` is the kernel
convenience vocabulary. Downstream typed records can be combined with
`RuleCoproduct`; the branches select data only.

Accepted forms are identified by stable constructor and form identities.
Vector order never selects meaning. Every pair of accepted forms must be
provably disjoint over typed positions. An overlap is refused at seal.
Table entries are canonically sorted by opaque encoded type identity before
the table identity is derived.

## Encoded names and name roles

`EncodedTypeId<Language>` carries one authority-issued `EncodedName` and no
hierarchical projection. `EncodedConstructorId<Language>` keeps its owning
type identity plus a distinct constructor-local number.

Named structural positions are explicit:

- `Declaration` consumes a `DeclarationAssignment<Language>` already issued
  by the authority.
- `Reference` consumes a `ResolvedReference<Language>` from lookup-only caller
  state.
- `Literal` carries an `EncodedName` for a fixed vocabulary word.

`DecodeNameBindings` has distinct methods for declarations and references and
receives the exact source bound. Equal spellings in different modules can
therefore resolve differently without this crate inventing module context.
`EncodedNameResolver` provides read-only spelling projection for encoding and
literal comparison.

There is no allocation method, mutable name table, identity-continuation
mechanism, fallback authority, or flattened identifier. Missing assignments,
unresolved references, and unknown spelling projections fail typed.

## Shared evaluation

Pass one builds a complete source-bounded block tree from the table-owned
discovery configuration. Pass two walks expected typed records through one
bounded cursor. Token reads consume the longest run accepted at the current
lexical position. Typed disjointness and conservative refusal govern above the
token level.

Products advance sibling block by sibling block, ordered sequences advance
lexical position by lexical position, repetition advances element by element,
applications consume their expected operator, and bounded descriptors enter
already-discovered children. Encoding follows the same descriptors in reverse
under the table-owned rendering policy.

The result is a constructor-tagged `StructuralValue<Language>` whose role-keyed
fields retain declaration, reference, and literal roles as distinct variants.
`decode_text_bounded` additionally returns runtime-only, full-source
`SourceBound`s keyed by those same typed roles.

## Deliberate boundaries

The crate does not define production authority allocation or emitted textual
metadata. It does not define Capsule/table pin
composition, move, retirement, dynamic-enum identity, or recursive per-thing
content hashing.

The table identity layout is 11 because the archived identity payload now
carries generic opaque identities, typed name-position roles, and distinct
ordered-product and ordered-sequence descriptors.

The former internal test corpus depended on the retired flat, locally
allocating `NameTable` API. Its structural/refusal claims are rehomed in the
public `downstream_authoring` contract, including two unrelated language
markers, opaque-name preservation, declaration/reference separation, lookup refusal,
token longest-match, typed overlap refusal, and six sibling typed root blocks
with exact source bounds and typed arity/position refusal.
