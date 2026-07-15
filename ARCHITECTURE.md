# structural-codec — architecture

This file records the durable direction of the crate. It is backed by the
psyche-accepted design in the primary workspace `reports/logos/up-close-design-v1.md`
(§4.1, §4.1.1, §4.6, §7.3) and `reports/logos/shared-codec-library-v1.md`.

## Purpose

structural-codec is L4 — the hardened kernel of the shared-codec family. It turns
a dialect's textual behaviour into content-addressed **data** (a table of
structural forms) and ships **one trusted evaluator** that executes that table in
both directions. Parser behaviour becomes serializable, inspectable, versionable
data with a proven round-trip — the psyche's "library instinct" raised one level.

## Dependency posture (strictly downward)

```
content-identity  ← name-table  ← raw-discovery  ← structural-codec
```

Stringless Core never depends on text: `content-identity` (portable archive +
`ContentHash<Domain>`) and `name-table` (the identifier space) sit below the raw
text layer. structural-codec consumes all three as git dependencies pinned to
published revisions. It edits none of them.

## The kernel / authoring split (design ruling 1)

The kernel `StructuralForm` is deliberately seven cases. The psyche's named
authoring structs are preserved as a distinct **authoring vocabulary**
(`AuthoringForm`) that `normalize()`s to kernel forms before hashing or
evaluation. This keeps the substrate minimal and content-identity stable while the
authoring surface stays expressive. `macro` is reserved for Nomos; the parser-side
data is a `StructuralForm`. The view family is `Textual*`.

## Table identity lives outside the payload (design §4.6)

A table's content identity is computed over `TableIdentityPayload`
(`CoreUniverseId`, Core-layout identity, raw-profile identity, the exact committed
lexicon bytes, leaf-codec contract identities, and the entries) and **stored on
the table, not inside the hashed payload** — this fixes the self-reference bug of
an earlier rendering. The table identity is **excluded from Core value identity by
construction**: Core hashing never sees the table, so text evolution can never
move Core identity. Old table decodes old text, a new table encodes new text, both
reach the same Core value.

## The evaluator ships in the runtime (Fork C, settled)

The psyche settled that the evaluator ships in the runtime, not only in
conformance tests: dialect tables are genuinely data-loadable at runtime and the
evaluator executes them directly. Generated codecs (arriving with `nota-derive` in
a later slice) remain the fast path; the conformance laws keep the two in
agreement. This is why a `ConstructorCodec` is data, not codegen-only.

## Decoding discipline

- Alternatives are matched **purely** (no interning), so backtracking across a
  constructor's disjoint decode forms — and across a type's constructors reached by
  `Delegate` — is free of side effects. The winning path is then interned through a
  speculative `NameTransaction`; a failed decode never gets past that transaction's
  rollback, so the NameTable is left byte-for-byte unchanged (law 3).
- **Delegation constructs every wrapper level** and rejects transparent cycles;
  recursion is permitted only after structure is consumed (left-recursion guard).
- The `Text` scalar leaf and the `Float` scalar leaf share one control path: a
  dotted raw `Application` rejoins via `Block::dotted_text`, and the expected type's
  terminal scalar decides the parse. Wrapper depth (a `Delegate` chain) is
  transparent.

## Disjointness is conservative-safe (inverted from nota's lineage)

nota's `validate_no_silent_conflicts` permits by default and rejects only
demonstrable shadows. This crate **inverts** that: a pair of decode forms is
accepted only when it can be *proven* that no block matches both (different block
kinds, distinct concrete atom cases, distinct literals, distinct delimiters, or a
provably-disjoint application position). Anything opaque (a delegate, leaf, or
product form) or unprovable is a hard error — a constructor can never silently
swallow another's inputs.

## Deviations and flagged placements

- **`StructuralValue::Delimited` does not store the delimiter.** The delimiter is
  pure syntax fixed by the constructor's form and recovered on encode, so a
  delimiter-only textual revision does not move a value's identity (required for
  law 4). This deviates from §4.4's pre-hardening sketch, which carried the
  delimiter in the mirror.
- **The canonical Block→text writer lives here** (`writer::CanonicalText`) for now.
  Its eventual home may be `raw-discovery` (as the inverse of
  `Recognizer::recognize`); raw-discovery is not edited for it in this slice.
- **`Literal` decode needs a table-scoped lexicon resolver** (`StructuralEvaluator::
  with_lexicon`). The fixture universe avoids `Literal` on decode paths; encode
  always has the caller's resolver.
- **Grammar readings.** `SigilSpec`/`$` and float-from-dotted-text ride on
  non-rejected grammar readings, gated behind profile revisions until accepted.
- **Signature-versus-Core validation is deferred**: the proof-of-concept has no
  Core layout to check `PositionalSignature` against, so the fixture universe
  de-blocks the parked schema-unit question with an explicit `FIXTURE_UNIVERSE`.

## Versioning

Behaviour that changes a public contract, the storage/wire archive layout, or the
table-identity pre-image must bump the relevant layout version (`HashDomain`
layout tags) or state why none is needed, and preserve compatibility unless a
break is explicitly accepted.
