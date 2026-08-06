# raw-discovery

`raw-discovery` discovers recursive, source-bounded blocks. It has no
unbounded syntax tree, language recognizer, foreign-language placeholder, or
semantic classification.

`BlockTreeDiscoveryConfiguration` and
`CueTerminatedBlockDiscoveryConfiguration` are canonical rule data. They seal
against one `SealedTokenProfile` before discovery. The resulting
`DiscoveredBlockTree` and `DiscoveredCueTerminatedBlockTree` retain exact
runtime source bounds and sealed cue evidence; they are not durable archive
values.

`BoundaryReader` performs outside-in traversal with configured boundaries,
opaque carriers, and trivia. A child reads only within its parent content
bound. Typed decoding belongs to the structural layer above this crate.

`ContentAddressedHash` identifies canonical token-profile data from its
archive bytes. It has no domain-specific alternative.

Run the checks with `nix flake check --max-jobs 0` or `cargo test`.
