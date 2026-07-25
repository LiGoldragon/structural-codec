# structural-codec

The Core-associated, bidirectional, revisioned **structural-form kernel** of the
next-generation NOTA family — with the trusted evaluator that **ships in the
runtime**. It is layer four (L4) of the shared-codec family, sitting atop
`content-identity`, `name-table`, and `raw-discovery`.

A dialect's textual surface is expressed as **data** — a table of structural
forms keyed by Core type — and one small trusted evaluator executes that table in
both directions. Because dialect tables are data-loadable at runtime and decode
and encode read the *same* forms, round-trip coherence holds by construction, and
new textual dialects can be added without regenerating codecs.

## The kernel / authoring split

`StructuralForm` is a minimal six-case kernel:

```
Atom · Leaf · Literal · Application{operator,head,payload} · Delimited{boundary,delimiter,sequence} · Delegate
```

`SequenceForm` separately carries fixed products and bounded repetition.

The psyche's named authoring structs (`ObjectSymbolPrefixedBlock`, `DottedForm`)
live in a separate **authoring vocabulary** and `normalize()` to kernel forms
*before* any form is hashed or evaluated, so the kernel stays small while the
authoring surface stays expressive. Example: `CommitSequence.{ Integer }` is
authored as an `ObjectSymbolPrefixedBlock` and normalizes to
`Application(Atom, Delimited)`.

## The pieces

- **Forms** — `StructuralForm` (kernel) and `AuthoringForm` (the normalizing surface).
- **Codecs** — `ConstructorCodec` is *asymmetric*: several structurally-disjoint
  accepted decode forms, exactly one canonical encode form, and a positional
  signature that must equal the constructor's Core field signature. A
  `StructuralEntry` gathers every constructor of one Core type.
- **Table** — `AddressedStructuralTable` is the external sidecar keyed by
  `ScopedEncodedTypeId`. Its content identity is computed over `TableIdentityPayload`
  and stored **outside** that payload, and is **excluded** from Core value identity
  by construction (Core hashing never sees the table).
- **Disjointness** — a conservative outer-shape checker: a pair of decode forms is
  accepted only when it can be *proven* that no block matches both. Overlap it
  cannot rule out is a hard error.
- **Evaluator** — `StructuralEvaluator` is the one trusted interpreter, both
  directions, over the generic `StructuralValue` mirror. It recognizes and
  emits text directly from the expected form's active trigger set. Its
  stack-local structural preflight advances through a sequence position by
  position, discovers a delimited form's outside close before semantic child
  decode, and partitions an application's outside operator before interpreting
  either side. Later-sibling triggers never become active early. The evaluator
  retains only checked current-form bounds and never constructs a preliminary
  token stream or persistent annotation tree. Decode alternatives at one
  expected type share their initial triggers, so an inactive alternative cannot
  swallow structure needed to select another.
- **Token profile** — the table pins a separately sealed profile identity.
  Application, boundary, atom, and leaf positions carry compact trigger
  identifiers, while table data carries the universal trivia set.
- **Conformance** — `ConformanceHarness` / `GeneratedCodec` is the law-5 contract a
  future `nota-derive`-generated codec will implement; today the evaluator is the
  sole implementation.

## The laws

The conformance laws are the acceptance gate (see `tests/laws.rs`):

1. `decode ∘ encode = core`
2. `encode ∘ decode = canonical(raw)`
3. a failed decode or reify leaves the NameTable unchanged (archived bytes and content identity)
4. old-table decode → new-table encode preserves Core value identity
5. interpreter and generated codec agree (scaffolding; evaluator is sole implementer)

## Build

`nix flake check` is the gate (build, test, clippy, fmt, doc). A dev shell is
provided:

```
nix develop
cargo test
```

## Status

Version 0.5.0. This micro-repository is the canonical structural-codec
producer. Consumers take its exact immutable revisions through the
producer-first train; it is not a compatibility mirror of a monorepo.
