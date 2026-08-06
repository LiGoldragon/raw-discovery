# raw-discovery architecture

`raw-discovery` is the canonical producer of untyped, source-bounded boundary
discovery. Its live values are `BlockTree` implementations and their root
containers: `DiscoveredBlockTree` for configured delimiters and
`DiscoveredCueTerminatedBlockTree` for cue-to-termination rules.

`SourceBound` is a checked half-open range in one input. `BlockCue` records
the matched opening and sealed rule evidence; `BlockPrefix` records configured
prefix evidence. Recursive children are discovered only inside the enclosing
content bound. Source bounds are runtime references, never archive identity.

`BoundaryReader` performs the common outside-in traversal. It balances only
configured boundaries, makes configured carriers and trivia opaque, and does
not construct a token stream. `TokenProfileData` and discovery configurations
are canonical archiveable rule data; sealing rejects ambiguous or invalid
rules and binds them to a pure `ContentAddressedHash` of the profile archive.

This crate has no unbounded syntax tree, language reader, foreign-language
arm, or semantic classification. Typed interpretation lives above the
source-bounded boundary.

Code: `src/boundary.rs` (bounded traversal), `src/block_tree.rs` (runtime
trees and configurations), `src/profile.rs` (canonical profile data). The
source-bounded witnesses are `tests/block_tree.rs`,
`tests/boundary_discovery.rs`, and `tests/rust_boundary_discovery.rs`.
