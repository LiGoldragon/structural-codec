# structural-codec architecture

The crate owns the archived structural-codec sidecar and its one shared
evaluator. It depends on `content-identity`, `name-table`, and `raw-discovery`;
it does not own a language parser or a second language-specific interpreter.

## R3 fixed rules

Fixed positions are actual archived record fields. `Position<Role, Descriptor>`
stores both a descriptor and an archived stable role identity. A role marker alone
is never the identity. `FieldLink` is a borrowed heterogeneous view used only
while the shared evaluator or prover traverses a record; it is not archivable.

`ConstructorCodec<R>`, `StructuralEntry<R>`, `TableIdentityPayload<R>`,
`AddressedStructuralTable<R>`, `StructuralEvaluator<R>`, and the prover all
operate over a record `R` that exposes only its borrowed typed fields and root
role through `StructureRecord`. `StructuralRule` remains the kernel convenience
vocabulary; it is not the only accepted rule carrier. A downstream vocabulary
combines several archived record types with `RuleCoproduct<L, R>`, whose branch
match selects data only. Decode, encode, boundary lookup, and proof all run in
`StructuralEvaluator` or `disjoint`; no record trait supplies those algorithms.
A fixed product, positional signature, fixed-position vector, and position
counting have no persisted representation.

## R4 identities and sidecars

`ScopedEncodedTypeId` and `EncodedConstructorId` are opaque archived values.
Their private Schema/Logos/Nomos variants carry `u16` locals, and constructor
identity contains its owning type identity. Public associated constructors mint
the language-qualified type and constructor-under-type associations; a table
then rejects duplicate constructor/form identities and any entry, constructor,
or language disagreement. Alternative vector order consequently cannot select
meaning.

The table pins its `ContentHash<TokenProfileDomain>` directly. Its target layout
uses a structural-codec-owned `TargetLayoutDomain`; raw bytes and zero/default
layout identities have no API. Table data also holds a typed vocabulary identity.
Fixture vocabularies have their own content-hash domain and may contain only the
reserved Schema range, preventing accidental composition with production
sidecars.

`StructuralTableDomain` moves from layout 7 to layout 8 because the archived
identity payload now includes the canonical pass-one discovery configuration.
`StructuralValueDomain` remains at layout 2: the archived value stays
constructor-tagged and role-keyed.

## Boundaries and values

Delimited descriptors name the authoritative profile boundary trigger; they do
not duplicate `raw-discovery::Delimiter` in a table preimage. A sealed table
owns the exact sealed token profile and its canonical, archiveable
block-discovery configuration. It validates both together while sealing and
includes the rules in the table identity payload. Text first creates a complete
source-bounded `DiscoveredBlockTree` from those table-owned rules. The same
shared evaluator then interprets each expected descriptor against only its
bounded source and the already-discovered children. Direct `Block` evaluation
remains a separate low-level compatibility surface.

The evaluator result is `StructuralValue { constructor, fields }`, where
`fields` is a `RoleKeyedMirror`. A manual `Textual::reflect` starts a checked
`StructuralValue::record` and adds values only through typed field roles;
`reify` retrieves them through the same typed role API. Reification and
reflection stay manual on `Textual`; no grammar or ordering logic is derived
there. Several accepted text forms may map to one constructor, subject to the
conservative disjointness proof. Delegate payload constraints participate in
that proof, and active delegate expansion is a typed cycle error.
`MissingLexicon` and name-resolution failures remain distinct from
`LiteralMismatch`; only a genuine structural non-match may try a disjoint
alternative. A `Textual` implementation supplies the table but no parallel
profile or discovery configuration, and its encoded form is statically
associated with the same language marker as its textual view.

## Open item

Conformance law 5's downstream generated-codec witness remains homeless: the
former derive repository is frozen and no derive path is revived here. The
shared evaluator and its conformance harness remain the one current kernel
surface until that witness has an explicitly authorized home.
