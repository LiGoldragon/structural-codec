> **DEPRECATED.** This crate is consolidated into
> [LiGoldragon/protos](https://github.com/LiGoldragon/protos) as a member of the
> protos cargo workspace. Every consumer now pins protos. This repository is kept
> only for history; do not add new pins to it.

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

`StructuralForm` is a minimal seven-case kernel:

```
Product · Atom · Leaf · Literal · Application{head,payload} · Delimited{delimiter,sequence} · Delegate
```

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
  `ScopedCoreTypeId`. Its content identity is computed over `TableIdentityPayload`
  and stored **outside** that payload, and is **excluded** from Core value identity
  by construction (Core hashing never sees the table).
- **Disjointness** — a conservative outer-shape checker: a pair of decode forms is
  accepted only when it can be *proven* that no block matches both. Overlap it
  cannot rule out is a hard error.
- **Evaluator** — `StructuralEvaluator` is the one trusted interpreter, both
  directions, over the generic `StructuralValue` mirror.
- **Conformance** — `ConformanceHarness` / `GeneratedCodec` is the law-5 contract a
  future `nota-derive`-generated codec will implement; today the evaluator is the
  sole implementation.

## The laws

The conformance laws are the acceptance gate (see `tests/laws.rs`):

1. `decode ∘ encode = core`
2. `encode ∘ decode = canonical(raw)`
3. a failed decode leaves the NameTable unchanged (archived bytes and content identity)
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

Version 0.1.0, proof-of-concept. The fixture universe in `src/fixture.rs` is the
worked acceptance gate; `ScopedCoreTypeId` is scoped to an explicit fixture
universe while the "unit of one schema" question is parked with the psyche.
