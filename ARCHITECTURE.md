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
inside a codec: a structure-only consumer such as a formatter or linter links
raw-discovery alone and cannot reach any language model from inside it. The
dependency graph enforces the invariant — raw-discovery depends only on leaf
mechanisms such as `content-identity`, `rkyv`, and `thiserror`, and on no Core
language type.

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

## Profiles are sealed data, never runtime guessing

`TokenProfileData` carries generic trigger definitions, a profile revision,
the root trigger set, and the negative-space exclusions for bare atoms. A
whitespace trigger carries its nonempty canonical emitted spelling as
identity-bearing data while recognition continues to match the generic
whitespace class and its complete runs. Sealing validates every definition and
active set, rejects equal complete matches, and pins the data under
`TokenProfileDomain`. A new lexical rule therefore changes explicit versioned
data and identity rather than reader code or a runtime heuristic.

Selection is local to an expected structural position. A sealed form activates
its trigger set at the current cursor, and the generic boundary reader chooses
the longest complete match only within that set. Vector order cannot encode
precedence. Operators are inactive outside expression positions because those
positions do not activate their identifiers. A closer such as `>` therefore
cannot silently become `>>` when only the closer is active.

Recognition is boundary-first and recursive, not a horizontal tokenization
pass. A group form finds its configured outside boundary while respecting its
configured carriers and trivia, then reads the interior under the expected
interior forms and trigger sets. Negative space between active triggers is bare
text. No `LexicalToken` block, preliminary token stream, or parallel annotation
tree exists.

`RawProfile` and `GlyphSet` remain compatibility selectors for the established
NOTA profiles. They seal into the same generic profile representation. Two
readers that disagree about lexical rules disagree by content identity.

## The raw-layer boundary

`RawLayer::Foreign` is a typed placeholder that names a target language. It is
not an adapter slot and not a grammar escape hatch. A target language supplies
sealed profile and structural-form data to the shared evaluator; it does not
install a foreign parser, printer, or second evaluator in raw-discovery.

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
little-endian, 32-bit-pointer, unaligned, `bytecheck` validation on read.
`content-identity` owns the shared contextual hashing mechanism. Tests lock an
absolute profile digest and an absolute digest of a composite archived document;
archive-image drift is therefore a failing test that forces an explicit
layout-version decision.

## Producer-first consumption

This micro-repository is the canonical producer for raw structural discovery.
Consumers take exact immutable revisions through the producer-first release
train. It is not a compatibility mirror of a monorepo.
