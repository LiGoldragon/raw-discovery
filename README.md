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

## Profiles are sealed, versioned data

`TokenProfileData` gives generic boundary triggers compact identifiers and
pins the complete profile under a contextual content identity. Structural
forms activate only the trigger set relevant to the current recursive
position. The boundary reader applies universal longest-complete-match inside
that active set; equal complete matches are rejected when the set seals.
Authored precedence is not representable.

Recognition is boundary-first and recursive. A group boundary is found while
configured carriers and trivia are respected, and its interior is then read
under the expected interior forms. The negative space between active triggers
is a bare atom. No preliminary token stream or parallel annotation tree is
constructed.

`RawProfile` and `GlyphSet` remain compatibility selectors for the established
NOTA profiles. Sealing either selector produces the same generic
`SealedTokenProfile` machinery used by new textual forms. Two readers that
disagree about lexical data disagree by profile identity rather than silently
drifting.

```rust
use raw_discovery::Recognizer;

let document = Recognizer::standard()
    .recognize("Public.Newtype.( CommitSequence [ rkyv.Archive Clone ] Integer )")
    .expect("valid nota structure");
let block = document.root_object_at(0).unwrap();
assert!(block.is_application());
```

## The raw-layer boundary

`RawLayer` names the textual family boundary. `RawLayer::Foreign` is only a
typed language placeholder; it is not a parser escape hatch. Target languages
supply sealed profile and structural-form data to the shared evaluator. They do
not install another parser, printer, or textual engine here.

## Status

This micro-repository is the canonical producer for raw structural discovery.
Serialization uses rkyv under the portable-archive feature discipline
(little-endian, 32-bit-pointer, unaligned, validated-on-read). Contextual
identity domains have absolute digest locks so an archive-image change requires
an explicit layout-version decision.

See `ARCHITECTURE.md` for the durable direction and the boundary rulings this
crate embodies. Built and checked through Nix: `nix flake check`.

## Licence

MIT OR Apache-2.0.
