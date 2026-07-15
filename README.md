# raw-discovery

The language-agnostic **raw structure layer** of the NOTA language family. It
discovers structure and **never classifies**.

A `Recognizer` reads source text into a tree of raw `Block`s — delimiter
nesting, right-associative dot-application, pipe text, bare atoms — and attaches
no meaning to any of it. Words like *declaration*, *field*, *name*, and *type*
are ones this crate does not know. The expected types that give structure
meaning live entirely in the crates above it. That invariant is the crate's
reason to exist as a boundary: a consumer that wants only structure — a
formatter, a linter, a tree-sitter bridge — links raw-discovery alone and never
drags in any language model.

## What it discovers

- `Block` — the raw node: `Delimited`, `Application`, `PipeText`, `Atom`.
  `Application` is a **designed-explicit** variant: nota expresses application
  structurally through a dotted head, and this crate promotes it to a
  first-class node so the raw layer names what nota leaves implicit. The dot is
  right-associative — `A.B.C = A.(B.C)` — which is a psyche-blessed rule.
- `Delimiter` — `( )`, `[ ]`, `{ }`.
- The dotted primitives: `Atom::split_at_first_dot` /
  `Atom::split_text_at_first_dot` (split) and `Block::dotted_text` (join).
- `AtomCase` — the capitalization classifier, exposed as **data**. The family
  reads capitalized-leading atoms as objects and lowercase-leading atoms as
  names, but that meaning lives outside this crate; here a case is a fact about
  an atom's characters and nothing more.

## Profiles are versioned data

A `RawProfile` pairs a `GlyphSet` with a `ProfileRevision`. The glyph vocabulary
a recognizer admits is versioned data, never a runtime guess: `GlyphSet::Standard`
rejects the `$` sigil that `GlyphSet::NomosExtended` admits, and admitting a new
glyph is a new profile revision.

```rust
use raw_discovery::Recognizer;

let document = Recognizer::standard()
    .recognize("Public.Newtype.( CommitSequence [ rkyv.Archive Clone ] Integer )")
    .expect("valid nota structure");
let block = document.root_object_at(0).unwrap();
assert!(block.is_application());
```

## The raw-layer boundary

`RawLayer` is the seam the whole textual family sits on: NOTA-family forms
(schema, Nomos, logos) share the `Recognizer`; a foreign language (Rust via
`syn`, for instance) supplies its own adapter through `RawLayer::Foreign`, a
typed placeholder this crate names but does not implement.

## Status

Version 0.1.0 — slice one of the language-family proof of concept. Serialization
uses rkyv under the portable-archive feature discipline (little-endian,
32-bit-pointer, unaligned, validated-on-read). Consumption and integration will
readapt to the forthcoming release-train flow.

See `ARCHITECTURE.md` for the durable direction and the boundary rulings this
crate embodies. Built and checked through Nix: `nix flake check`.

## Licence

MIT OR Apache-2.0.
