# structural-codec

`structural-codec` is the shared evaluator for archived, fully typed structural
rule records.

The crate is generic over a caller-supplied vocabulary-root enum. Type,
constructor, descriptor, and mirror values retain complete root-fronted
encoded-ID chains without flattening. Declarations accept translator-issued
assignments, references use lookup-only resolution, and fixed vocabulary words
resolve through a read-only spelling projection. The crate has no identity
allocation surface.

Textual decode first discovers source-bounded blocks, then evaluates expected
typed positions through one bounded-cursor engine. Tokens use lexical
longest-match. Alternative forms must be provably disjoint over typed
positions; ambiguous shapes fail when the table seals instead of being
order-resolved.

`StructuralRule` is a convenience vocabulary. Downstream repositories may
archive their own real `StructureRecord` types and still use the same
evaluator, prover, and rendering path.

Fixed multi-block documents use `OrderedProduct`: an ordered set of links to
real typed record fields, each delegating to its own expected type.
`decode_text_bounded` returns the value plus exact full-source bounds keyed by
those field roles. Arity and position mismatches fail typed; no indexed root
splitter is exposed.

## Build and test

```sh
cargo test
nix flake check
```
