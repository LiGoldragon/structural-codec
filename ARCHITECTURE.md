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

The kernel `StructuralForm` is deliberately six cases. The psyche's named
authoring structs are preserved as a distinct **authoring vocabulary**
(`AuthoringForm`) that `normalize()`s to kernel forms before hashing or
evaluation. This keeps the substrate minimal and content-identity stable while the
authoring surface stays expressive. `macro` is reserved for Nomos; textual
structure is `StructuralForm` data. The view family is `Textual*`.

## Table identity lives outside the payload (design §4.6)

A table's content identity is computed over `TableIdentityPayload`
(`EncodedUniverseId`, Core-layout identity, raw-profile identity, leaf-codec contract
identities, and the entries) and **stored on the table, not inside the hashed
payload** — this fixes the self-reference bug of an earlier rendering. The
payload pins the separately sealed token-profile identity, universal trivia
triggers, and every form-position trigger. The table identity is **excluded
from Core value identity by construction**: Core hashing never sees the table,
so text evolution can never move Core identity. Old table decodes old text, a
new table encodes new text, both reach the same Core value.

## The evaluator ships in the runtime (Fork C, settled)

The psyche settled that the evaluator ships in the runtime, not only in
conformance tests: dialect tables are genuinely data-loadable at runtime and the
evaluator executes them directly. Generated codecs (arriving with `nota-derive` in
a later slice) remain the fast path; the conformance laws keep the two in
agreement. This is why a `ConstructorCodec` is data, not codegen-only.

## Decoding discipline

- Alternatives are matched **purely** (no interning), so backtracking across a
  constructor's disjoint decode forms — and across a type's constructors
  reached by `Delegate` — is free of side effects. `Textual::unview` holds one
  speculative `NameTransaction` across both decode and reify; either operation
  failing leaves the NameTable byte-for-byte unchanged (law 3).
- Recognition is boundary-first and recursive. The current expected form
  activates an unordered sealed trigger set, universal longest-complete-match
  is applied only within that set, and a delimited form discovers its complete
  outside boundary before any expected child is interpreted. There is no
  preliminary token stream, annotation tree, authored precedence, parser
  callback, or second textual engine.
- Decode alternatives under one expected type share the union of their initial
  triggers while that alternative is selected. A candidate is accepted only
  after it reaches the enclosing terminator, a sequence trivia boundary, or end
  of input. This preserves structural disjointness without making alternatives'
  declaration order semantic.
- A delimited form carries its boundary trigger. The table seal derives an
  exact discovery set from its reachable interior boundary/carrier forms and
  universal trivia, rejecting horizontal triggers from that state. The shared
  raw reader balances nested boundaries and skips configured carriers while it
  partitions the whole group into opening/interior/closing ranges. Only then
  does the evaluator recurse through a `TextReading` bounded to the interior.
  Products, repetitions, applications, and failed children cannot read across
  that bound.
- Safe alternative mismatch remains backtrackable. Once an expected opener is
  matched, malformed boundary/carrier structure and bounded-child failures are
  committed typed errors with byte ranges; they never collapse into
  `NoAlternative`.
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
provably-disjoint application position). Anything opaque (a delegate or leaf) or
unprovable is a hard error — a constructor can never silently swallow another's
inputs.

## Deviations and flagged placements

- **`StructuralValue::Delimited` does not store the delimiter.** Delimiter-only
  table revisions preserve the StructuralValue mirror hash; structural respellings
  move it by design (law 4). This deviates from §4.4's pre-hardening sketch, which
  carried the delimiter in the mirror.
- **Canonical text is emitted by the shared evaluator from sealed data.**
  Operator, boundary, carrier, and canonical trivia spellings come from the
  pinned token profile. The retired `Block→text` writer no longer exists.
- **`Literal` decode needs a table-scoped lexicon resolver**
  (`StructuralEvaluator::with_profile_and_lexicon`). The fixture universe avoids
  `Literal` on decode paths; encode always has the caller's resolver.
- **Directed delegation.** Expected-type positions may carry an optional closed
  `DelegationPayload`; the first payload kind directs atom case. It is sealed into
  table identity, participates in disjointness proof, and is enforced during
  both decode and encode. The unused `SigilSpec`/`$` surface was retired rather
  than kept as speculative grammar.
- **Signature-versus-Core validation is deferred**: the proof-of-concept has no
  Core layout to check `PositionalSignature` against, so the fixture universe
  de-blocks the parked schema-unit question with an explicit `FIXTURE_UNIVERSE`.

## Versioning

Behaviour that changes a public contract, the storage/wire archive layout, or
the table-identity pre-image must bump the relevant layout version
(`HashDomain` layout tags) or state why none is needed. Layout 6 adds recursive
position triggers and the canonical trivia set. Absolute digest tests lock
every contextual hash domain so archive-image drift is a red test. Boundary
regions are ephemeral UTF-8-checked cursor state and are not archived; this
correction therefore leaves the layout-6 table and layout-1 value pre-images
unchanged.
