# raw-discovery — architecture

`raw-discovery` is the language-agnostic first pass of the shared structural
codec. It discovers recursive blocks and their exact source bounds without
classifying declarations, fields, names, or types. Expected-type interpretation
belongs to the shared evaluator and per-language structural data above this
crate.

## The live source-bounded path

The current source-bounded mechanism is implemented in `boundary.rs` and
`block_tree.rs`:

- `SourceBound` is one validated half-open range in a particular source text.
- `BlockCue<Evidence>` records the opening source bound and the sealed
  boundary or cue-to-termination rule that matched it.
- `BlockPrefix` records an optional configured prefix word and separator.
- `BlockTree` is the universal untyped projection: complete `source_bound`,
  `cue`, optional `prefix`, `content_bound`, optional `closing_bound`, and
  recursive `children`.
- `DiscoveredBlock` implements that trait, and `DiscoveredBlockTree` holds the
  source-ordered roots.
- `BlockTreeDiscoveryConfiguration` is archiveable rule data. Its sealed form is
  bound to one exact token profile before it may discover source.

Every discovered block on this live path carries its source range. Bounds are
runtime references into the source being decoded, not durable identity, and the
runtime tree intentionally has no archive derives. The discovery configuration,
not a source-bound result, is the portable data.

The `Block` / `Document` recognizer model still exists for established NOTA
compatibility. That older portable tree does not carry bounds and must not be
presented as the pass-1 architecture or as evidence that discovered structure is
span-free.

## Boundary-first recursion

Discovery is outside-in. `BoundaryReader` finds configured opening and closing
boundaries, balances active nested boundaries, treats configured carrier
interiors such as strings and comments as opaque, and returns a bounded
interior. Recursive discovery then constructs children only inside that
interior. A child cannot consume or inspect source outside its parent's content
bound.

This pass finds boundaries only. It does not parse a full grammar and does not
construct a preliminary token stream. Typed parsing revisits the bounded content
in pass 2 under an expected structural type.

The live traversal has both delimiter and cue-to-termination entry points.
Both use the balanced-scan core in `discover_delimited_with`; only a public
convenience wrapper around the delimiter core is test-only.

## Cues and language rule data

A cue is evidence that a block begins. In `DiscoveredBlock`, `BlockCue`
represents a configured opening boundary. Protos-family prefixes may be
attached to those openings through data-driven prefix rules.

`CueTerminatedBlockDiscoveryConfiguration` adds archiveable complete-word cue
and typed termination rules to the same sealed delimiter configuration.
`DiscoveredCueTerminatedBlockTree` treats the cue as inclusive, observes the
termination only at the cue's source level, and recursively carries every
delimited child with exact source bounds. Termination may be an exact spelling,
as for a Rust `struct`/`;`, or the closing side of one balanced boundary that
remains a child, as for a Rust enum body. This is boundary discovery, not a
Rust grammar: the rule records neither declarations nor fields, and it
allocates no identities.

Configured strings and comments remain opaque to both delimiter balancing and
cue-to-termination scanning.

## Profiles and longest match

`TokenProfileData` contains canonical trigger definitions, active sets, and
profile revision. Sealing validates those rules and binds consumers to the
profile identity.

Token-level longest match is lexical law: one token is the longest run accepted
by its character class. At configured boundary positions, selection is limited
to the exact sealed active set; vector order never establishes precedence.
Typed disjointness and conservative refusal govern structural choices above the
token level.

## Boundary of responsibility

`raw-discovery` depends on no encoded language model. It returns untyped,
source-bounded structure plus the rule evidence needed to interpret it. It
never labels a block as a declaration, field, name, or type.

`RawLayer::Foreign` is a typed target-language placeholder, not an adapter slot
or grammar escape hatch. A language supplies typed rule data to the common
mechanism; it does not install a foreign parser, printer, or parallel evaluator.

The portable rule types follow the shared rkyv discipline. Absolute archive and
profile digest witnesses force explicit revision decisions when that portable
rule data changes. Runtime source bounds are checked for valid UTF-8 boundaries
and source extent rather than archived or hashed as portable structure.

## Current code map

- `src/boundary.rs` — `SourceBound`, configured recursive boundary traversal,
  carrier opacity, and the live balanced-scan core.
- `src/block_tree.rs` — `BlockCue`, `BlockTree`, source-bounded delimiter and
  cue-terminated runtime nodes, prefix rules, and discovery configurations.
- `src/profile.rs` — canonical trigger/profile data and sealing.
- `src/block.rs`, `src/recognizer.rs`, `src/error.rs` — the older span-free NOTA
  compatibility recognizer; not the target pass-1 tree.
- `tests/block_tree.rs`, `tests/boundary_discovery.rs`,
  `tests/rust_boundary_discovery.rs` — the current source-bounded witnesses.

This micro-repository is the canonical producer. Consumers take exact immutable
revisions through the producer-first release train.
