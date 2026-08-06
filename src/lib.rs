//! # raw-discovery
//!
//! Source-bounded discovery of recursive delimiter and cue-terminated
//! [`BlockTree`] values. It supplies no unbounded syntax tree, language
//! recognition, target-language placeholder, or semantic classification.

mod block_tree;
mod boundary;
mod profile;

pub use block_tree::{
    BlockCue, BlockDiscoveryError, BlockPrefix, BlockPrefixAttachment, BlockPrefixRule, BlockTree,
    BlockTreeDiscoveryConfiguration, CueTerminatedBlockCueEvidence,
    CueTerminatedBlockDiscoveryConfiguration, CueTermination, CueTerminationRule,
    CueTerminationRuleIdentifier, DiscoveredBlock, DiscoveredBlockTree,
    DiscoveredCueTerminatedBlock, DiscoveredCueTerminatedBlockTree,
    SealedBlockTreeDiscoveryConfiguration, SealedCueTerminatedBlockDiscoveryConfiguration,
};
pub use boundary::{
    BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext, BoundaryDiscoveryContextIdentifier,
    BoundaryDiscoveryError, BoundaryDiscoveryTransition, BoundaryReader, BoundarySide,
    DelimitedBoundary, DiscoveredDelimitedBoundary, SealedBoundaryDiscoveryConfiguration,
    SourceBound, TriggerMatch, TriggerMatchKind,
};
pub use profile::{
    CharacterClass, CharacterSet, GlyphSet, ProfileRevision, RawProfile,
    SealedBoundaryDiscoverySet, SealedTokenProfile, SealedTriggerSet, TokenProfileData,
    TokenProfileError, Trigger, TriggerDefinition, TriggerIdentifier, TriggerSet, TriggerTextRole,
};
