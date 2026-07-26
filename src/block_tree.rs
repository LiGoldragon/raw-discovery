//! Runtime-only, source-bounded block discovery.
//!
//! This module converts a single outside-in boundary traversal into untyped
//! source bounds. Boundary activation and prefix attachment are supplied as
//! runtime data; this module does not interpret the bounded content.

use thiserror::Error;

use crate::{
    BoundaryDiscoveryConfiguration, BoundaryDiscoveryError, CharacterClass,
    DiscoveredDelimitedBoundary, SealedTokenProfile, SourceBound, TokenProfileError,
    TriggerIdentifier,
};

/// One opening boundary recorded as a block cue.
///
/// The cue bound is the opening spelling in the source. Its evidence is the
/// profile boundary declaration that matched that spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockCue {
    bound: SourceBound,
    evidence: TriggerIdentifier,
}

impl BlockCue {
    /// The source bound of the opening boundary spelling.
    pub fn bound(self) -> SourceBound {
        self.bound
    }

    /// The profile boundary declaration that supplied this cue.
    pub fn evidence(self) -> TriggerIdentifier {
        self.evidence
    }
}

/// Explicit runtime data for one prefix spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockPrefixRule {
    separator: String,
    word_characters: CharacterClass,
}

impl BlockPrefixRule {
    pub fn new(separator: impl Into<String>, word_characters: CharacterClass) -> Self {
        Self {
            separator: separator.into(),
            word_characters,
        }
    }

    pub fn separator(&self) -> &str {
        &self.separator
    }

    pub fn word_characters(&self) -> &CharacterClass {
        &self.word_characters
    }
}

/// One prefix rule attached to an opening boundary declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockPrefixAttachment {
    boundary: TriggerIdentifier,
    rule: BlockPrefixRule,
}

impl BlockPrefixAttachment {
    pub fn new(boundary: TriggerIdentifier, rule: BlockPrefixRule) -> Self {
        Self { boundary, rule }
    }

    pub fn boundary(&self) -> TriggerIdentifier {
        self.boundary
    }

    pub fn rule(&self) -> &BlockPrefixRule {
        &self.rule
    }
}

/// The prefix source evidence attached to one discovered block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockPrefix {
    word: SourceBound,
    separator: SourceBound,
}

impl BlockPrefix {
    /// The source bound of the prefix word.
    pub fn word(self) -> SourceBound {
        self.word
    }

    /// The source bound of the configured separator before the opening.
    pub fn separator(self) -> SourceBound {
        self.separator
    }
}

/// Runtime rule data for a source-bounded block tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockTreeDiscoveryConfiguration {
    boundaries: BoundaryDiscoveryConfiguration,
    prefixes: Vec<BlockPrefixAttachment>,
}

impl BlockTreeDiscoveryConfiguration {
    pub fn new(
        boundaries: BoundaryDiscoveryConfiguration,
        prefixes: Vec<BlockPrefixAttachment>,
    ) -> Self {
        Self {
            boundaries,
            prefixes,
        }
    }

    pub fn boundaries(&self) -> &BoundaryDiscoveryConfiguration {
        &self.boundaries
    }

    pub fn prefixes(&self) -> &[BlockPrefixAttachment] {
        &self.prefixes
    }

    fn validate(&self) -> Result<(), BlockDiscoveryError> {
        for (index, prefix) in self.prefixes.iter().enumerate() {
            if prefix.rule.separator.is_empty() {
                return Err(BlockDiscoveryError::EmptyPrefixSeparator {
                    boundary: prefix.boundary,
                });
            }
            if self.prefixes[..index]
                .iter()
                .any(|prior| prior.boundary == prefix.boundary)
            {
                return Err(BlockDiscoveryError::DuplicatePrefixRule {
                    boundary: prefix.boundary,
                });
            }
        }
        Ok(())
    }

    fn prefix_rule(&self, boundary: TriggerIdentifier) -> Option<&BlockPrefixRule> {
        self.prefixes
            .iter()
            .find(|prefix| prefix.boundary == boundary)
            .map(BlockPrefixAttachment::rule)
    }
}

/// The universal, untyped block-tree projections.
///
/// Implementors expose runtime source mechanics only. The content bound is
/// the text within the opening and closing spellings; child order follows the
/// source order in that bound.
pub trait BlockTree {
    /// The complete source range of this block, including an attached prefix,
    /// opening, content, and closing spelling.
    fn source_bound(&self) -> SourceBound;

    /// The opening boundary and the profile declaration that matched it.
    fn cue(&self) -> BlockCue;

    /// The configured prefix evidence, when the source matched its rule.
    fn prefix(&self) -> Option<BlockPrefix>;

    /// The source range inside the opening and closing spellings.
    fn content_bound(&self) -> SourceBound;

    /// The closing spelling when this boundary model has one.
    fn closing_bound(&self) -> Option<SourceBound>;

    /// Recursively discovered delimiter children in source order.
    fn children(&self) -> &[Self]
    where
        Self: Sized;
}

/// One runtime-only node in a boundary-discovered block tree.
///
/// It deliberately has no archive derives: the bounds refer to one source
/// text and therefore do not belong to portable `Block` or `Document`
/// identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredBlock {
    source: SourceBound,
    cue: BlockCue,
    prefix: Option<BlockPrefix>,
    content: SourceBound,
    closing: SourceBound,
    children: Vec<Self>,
}

impl BlockTree for DiscoveredBlock {
    fn source_bound(&self) -> SourceBound {
        self.source
    }

    fn cue(&self) -> BlockCue {
        self.cue
    }

    fn prefix(&self) -> Option<BlockPrefix> {
        self.prefix
    }

    fn content_bound(&self) -> SourceBound {
        self.content
    }

    fn closing_bound(&self) -> Option<SourceBound> {
        Some(self.closing)
    }

    fn children(&self) -> &[Self] {
        &self.children
    }
}

/// The root sequence of a runtime-only block tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredBlockTree {
    root_blocks: Vec<DiscoveredBlock>,
}

impl DiscoveredBlockTree {
    /// Discover configured delimiter blocks in one outside-in traversal.
    pub fn discover(
        source: &str,
        profile: &SealedTokenProfile,
        configuration: &BlockTreeDiscoveryConfiguration,
    ) -> Result<Self, BlockDiscoveryError> {
        configuration.validate()?;
        let boundaries = configuration.boundaries.seal(profile)?;
        let mut reader = crate::BoundaryReader::new(source, profile);
        let discovered = reader.discover_boundary_children(&boundaries)?;
        let root_bound = SourceBound::whole(source);
        let root_blocks = discovered
            .iter()
            .map(|boundary| {
                DiscoveredBlock::from_boundary(source, root_bound, boundary, configuration)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { root_blocks })
    }

    /// Top-level blocks in source order.
    pub fn root_blocks(&self) -> &[DiscoveredBlock] {
        &self.root_blocks
    }
}

impl DiscoveredBlock {
    fn from_boundary(
        source: &str,
        containing_bound: SourceBound,
        discovered: &DiscoveredDelimitedBoundary,
        configuration: &BlockTreeDiscoveryConfiguration,
    ) -> Result<Self, BlockDiscoveryError> {
        let boundary = discovered.boundary();
        let prefix = configuration
            .prefix_rule(boundary.identifier())
            .map(|rule| block_prefix(source, containing_bound, boundary.opening(), rule))
            .transpose()?
            .flatten();
        let source_start = prefix.map_or(boundary.opening().start(), |prefix| prefix.word.start());
        let children = discovered
            .children()
            .iter()
            .map(|child| Self::from_boundary(source, boundary.interior(), child, configuration))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            source: SourceBound::checked(source, source_start, boundary.closing().end())?,
            cue: BlockCue {
                bound: boundary.opening(),
                evidence: boundary.identifier(),
            },
            prefix,
            content: boundary.interior(),
            closing: boundary.closing(),
            children,
        })
    }
}

fn block_prefix(
    source: &str,
    containing_bound: SourceBound,
    opening: SourceBound,
    rule: &BlockPrefixRule,
) -> Result<Option<BlockPrefix>, BlockDiscoveryError> {
    let Some(separator_start) = opening.start().checked_sub(rule.separator.len()) else {
        return Ok(None);
    };
    if separator_start < containing_bound.start()
        || source.get(separator_start..opening.start()) != Some(rule.separator())
    {
        return Ok(None);
    }

    let mut word_start = separator_start;
    while word_start > containing_bound.start() {
        let character = source[..word_start]
            .chars()
            .next_back()
            .expect("a non-empty prefix has a final character");
        if !rule.word_characters.matches(character) {
            break;
        }
        word_start -= character.len_utf8();
    }
    if word_start == separator_start {
        return Ok(None);
    }

    Ok(Some(BlockPrefix {
        word: SourceBound::checked(source, word_start, separator_start)?,
        separator: SourceBound::checked(source, separator_start, opening.start())?,
    }))
}

/// Structural failures while finding configured block boundaries.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BlockDiscoveryError {
    #[error(transparent)]
    Boundary(#[from] BoundaryDiscoveryError),

    #[error(transparent)]
    Profile(#[from] TokenProfileError),

    #[error("prefix separator for boundary {boundary:?} is empty")]
    EmptyPrefixSeparator { boundary: TriggerIdentifier },

    #[error("boundary {boundary:?} has more than one prefix rule")]
    DuplicatePrefixRule { boundary: TriggerIdentifier },
}
