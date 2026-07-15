# raw-discovery — architecture

This crate is the language-agnostic raw structure layer (crate L3) of the
psyche-accepted shared-codec language family. Its direction is fixed by the
accepted design in `reports/logos/up-close-design-v1.md` (§5, the
`raw-discovery` crate, and §4.2 the versioned profile) and
`reports/logos/shared-codec-library-v1.md` (§2.4). This document states the
durable boundary the crate holds; it does not restate the code.

## The one invariant: discover structure, never classify

The raw layer discovers structure and never classifies meaning. It knows
delimiters and dots, not "declaration", "field", "name", or "type". Expected
types — the machinery that reads meaning off this structure — live entirely in
the crates above (`structural-codec` and the per-language forms), never here.

This is why the crate exists as a *separate boundary* rather than a module
inside a codec: a structure-only consumer (a formatter, a linter, a tree-sitter
bridge) links raw-discovery alone and cannot reach any language model from
inside it. The dependency graph enforces the invariant — raw-discovery depends
only on `rkyv` and `thiserror`, and on no Core language type.

## Application is a designed-explicit promotion

nota's current parser expresses application *structurally*: a dotted head glued
to its argument group, with no `Application` variant in its `Block` model. The
accepted design (up-close §5) deliberately **promotes** application to a
first-class `Block::Application { head, payload }` variant, so the raw layer
names what nota leaves implicit. The binding rule is unchanged and
psyche-blessed: the dot is right-associative, `A.B.C = A.(B.C)`, so the head is
always the leftmost single segment and the payload is the remainder.

This is the one place the crate is *designed-new* rather than a verbatim lift.
The block queries, the dotted split/join primitives, the capitalization
predicates, and the recursive-descent reader are lifted verbatim from nota
next-gen (tip `18e2e8d0`); the explicit `Application` variant is the design's
promotion of nota's implicit structural application.

## Capitalization is exposed as data, not meaning

Capitalization is semantic at the family level — a capitalized-leading atom
reads as an object, a lowercase-leading atom as a name. This crate exposes the
classifier (`AtomCase`) as **data** and attaches no meaning to it. A reader may
ask whether an atom reads as PascalCase; the crate never stamps "object" or
"name" onto the atom. `AtomCase::of` classifies every non-empty atom into
exactly one case, with `Symbol` as the catch-all.

## Profiles are versioned data, never runtime guessing

Delimiters and glyphs serve the reader; a new glyph requires an explicit
versioned profile revision, never a runtime heuristic (the accepted Codex
hardening). A `RawProfile` is `{ revision: ProfileRevision, glyphs: GlyphSet }`,
and `GlyphSet` is a closed enum: `Standard` (the base NOTA glyphs) or
`NomosExtended` (the base set plus the `$` sigil). A recognizer under `Standard`
that meets a `$` raises `RecognizeError::UnsupportedGlyph` rather than guessing.
Two readers that disagree about the glyph vocabulary disagree by *identity*,
spot-checkably, not by silent drift.

## The raw-layer boundary: shared recognizer vs foreign adapters

`RawLayer` is the principled seam the whole textual family sits on. NOTA-family
forms — schema, Nomos, logos — share the one `Recognizer`. A foreign language
whose surface is not NOTA (Rust, recognized by `syn` and emitted by
`prettyplease`) is served by its own adapter through the `RawLayer::Foreign`
arm. In this crate the `Foreign` arm is a **typed placeholder**: it names the
target language and holds no foreign grammar. The consuming crate implements the
adapter. This keeps foreign-language parsing out of the raw NOTA layer instead
of pretending a foreign grammar is NOTA.

## Structure is span-free

The recognized `Block` tree carries no source spans. The recognizer tracks
source positions only to build `RecognizeError` diagnostics; byte offsets into
one particular source string are not portable identity, so they are recovered
for errors and never attached to the structure. This keeps the discovered
structure portable, content-addressable data that round-trips through rkyv.

(The up-close §5 sketch wrote spans as "dropped from the archived form"; since
the recognized `Block` *is* the archivable form here, spans are dropped from the
model entirely and live only on the error type. A future consumer needing spans
layers them above this crate.)

## Serialization and the portable bound

The data types derive rkyv under the portable-archive feature discipline —
little-endian, 32-bit-pointer, unaligned, `bytecheck` validation on read. The
shared `PortableArchive` bound will live in the `content-identity` crate (the
family's leaf). Until that crate publishes, this crate mirrors the exact feature
set inline in `Cargo.toml` and exercises the full round-trip in `tests/archive.rs`;
adopting the shared bound once `content-identity` lands is tracked in the epic.

## Consumption readapts to the release train

This crate is slice one of the family proof of concept. Consumption and
integration — which crates depend on it, how it is pinned, how it builds in the
wider graph — will readapt to the forthcoming release-train flow; nothing here
assumes a final integration shape.
