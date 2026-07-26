//! Source-bounded block discovery.
//!
//! This module converts a single outside-in boundary traversal into untyped
//! source bounds. Its context, transition, and prefix rules are archiveable
//! canonical data; sealing binds them to one token profile. The resulting tree
//! and its source bounds remain runtime-only.

use thiserror::Error;

use crate::{
    BoundaryDiscoveryConfiguration, BoundaryDiscoveryError, CharacterClass,
    DiscoveredDelimitedBoundary, SealedBoundaryDiscoveryConfiguration, SealedTokenProfile,
    SourceBound, TokenProfileError, Trigger, TriggerIdentifier,
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

/// Canonical rule data for one prefix spelling.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
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
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
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

/// Canonical archiveable rule data for a source-bounded block tree.
///
/// The configuration itself carries no source bounds. [`Self::seal`] validates
/// it against one exact [`SealedTokenProfile`] before it can drive discovery.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct BlockTreeDiscoveryConfiguration {
    boundaries: BoundaryDiscoveryConfiguration,
    prefixes: Vec<BlockPrefixAttachment>,
}

impl BlockTreeDiscoveryConfiguration {
    pub fn new(
        boundaries: BoundaryDiscoveryConfiguration,
        mut prefixes: Vec<BlockPrefixAttachment>,
    ) -> Self {
        prefixes.sort_unstable_by_key(BlockPrefixAttachment::boundary);
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

    /// Bind this canonical rule data to `profile` after validating every
    /// reference and every pass-1 restriction.
    pub fn seal(
        &self,
        profile: &SealedTokenProfile,
    ) -> Result<SealedBlockTreeDiscoveryConfiguration, BlockDiscoveryError> {
        let boundaries = self.boundaries.seal(profile)?;
        let mut prefix_boundaries: Vec<_> = self
            .prefixes
            .iter()
            .map(BlockPrefixAttachment::boundary)
            .collect();
        prefix_boundaries.sort_unstable();
        if let Some(&boundary) = prefix_boundaries
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(&pair[0]))
        {
            return Err(BlockDiscoveryError::DuplicatePrefixRule { boundary });
        }
        if !self
            .prefixes
            .windows(2)
            .all(|pair| pair[0].boundary() < pair[1].boundary())
        {
            return Err(BlockDiscoveryError::NoncanonicalPrefixOrder);
        }
        for prefix in &self.prefixes {
            if prefix.rule.separator.is_empty() {
                return Err(BlockDiscoveryError::EmptyPrefixSeparator {
                    boundary: prefix.boundary,
                });
            }
            if !prefix.rule.word_characters.is_canonical() {
                return Err(BlockDiscoveryError::NoncanonicalPrefixAlphabet {
                    boundary: prefix.boundary,
                });
            }
            if !matches!(
                profile.definition(prefix.boundary)?.trigger,
                Trigger::Boundary { .. }
            ) {
                return Err(BlockDiscoveryError::PrefixRequiresBoundary {
                    trigger: prefix.boundary,
                });
            }
            if !boundaries.configures_boundary(prefix.boundary) {
                return Err(BlockDiscoveryError::UnconfiguredPrefixBoundary {
                    boundary: prefix.boundary,
                });
            }
        }
        Ok(SealedBlockTreeDiscoveryConfiguration {
            boundaries,
            prefixes: self.prefixes.clone(),
        })
    }
}

/// Runtime rule data proven against one exact token profile.
///
/// This derived value intentionally has no archive representation: its active
/// discovery sets are bound to the profile identity supplied at sealing time.
#[derive(Clone, Debug)]
pub struct SealedBlockTreeDiscoveryConfiguration {
    boundaries: SealedBoundaryDiscoveryConfiguration,
    prefixes: Vec<BlockPrefixAttachment>,
}

impl SealedBlockTreeDiscoveryConfiguration {
    fn matches_profile(&self, profile: &SealedTokenProfile) -> bool {
        self.boundaries.matches_profile(profile)
    }

    fn prefix_rule(&self, boundary: TriggerIdentifier) -> Option<&BlockPrefixRule> {
        self.prefixes
            .binary_search_by_key(&boundary, BlockPrefixAttachment::boundary)
            .ok()
            .map(|index| self.prefixes[index].rule())
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
        configuration: &SealedBlockTreeDiscoveryConfiguration,
    ) -> Result<Self, BlockDiscoveryError> {
        if !configuration.matches_profile(profile) {
            return Err(TokenProfileError::TriggerSetProfileMismatch.into());
        }
        let mut reader = crate::BoundaryReader::new(source, profile);
        let discovered = reader.discover_boundary_children(&configuration.boundaries)?;
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
        configuration: &SealedBlockTreeDiscoveryConfiguration,
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

    #[error("block-prefix rules are not in canonical boundary order")]
    NoncanonicalPrefixOrder,

    #[error("prefix alphabet for boundary {boundary:?} is not canonical")]
    NoncanonicalPrefixAlphabet { boundary: TriggerIdentifier },

    #[error("trigger {trigger:?} cannot carry a block-prefix rule")]
    PrefixRequiresBoundary { trigger: TriggerIdentifier },

    #[error("prefix rule for boundary {boundary:?} is not active in any discovery context")]
    UnconfiguredPrefixBoundary { boundary: TriggerIdentifier },
}

#[cfg(test)]
mod tests {
    use super::{
        BlockDiscoveryError, BlockPrefixAttachment, BlockPrefixRule,
        BlockTreeDiscoveryConfiguration,
    };
    use crate::{
        BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
        BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryTransition, CharacterClass,
        CharacterSet, RawProfile, TriggerIdentifier, TriggerSet,
    };

    fn archive_round_trip(
        configuration: &BlockTreeDiscoveryConfiguration,
    ) -> BlockTreeDiscoveryConfiguration {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(configuration)
            .expect("archive block-tree configuration");
        rkyv::from_bytes::<BlockTreeDiscoveryConfiguration, rkyv::rancor::Error>(&bytes)
            .expect("validated block-tree configuration archive")
    }

    #[test]
    fn archived_noncanonical_prefix_order_and_alphabet_refuse() {
        let profile = RawProfile::standard().seal().expect("standard profile");
        let root = BoundaryDiscoveryContextIdentifier::new(95);
        let parenthesis = TriggerIdentifier::new(0);
        let square = TriggerIdentifier::new(1);
        let two_boundaries = BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::new(vec![parenthesis, square]),
            )],
            vec![
                BoundaryDiscoveryTransition::new(root, parenthesis, root),
                BoundaryDiscoveryTransition::new(root, square, root),
            ],
        );
        let reordered = BlockTreeDiscoveryConfiguration {
            boundaries: two_boundaries,
            prefixes: vec![
                BlockPrefixAttachment::new(
                    square,
                    BlockPrefixRule::new(".", CharacterClass::AsciiAlphabetic),
                ),
                BlockPrefixAttachment::new(
                    parenthesis,
                    BlockPrefixRule::new(".", CharacterClass::AsciiAlphabetic),
                ),
            ],
        };
        let reordered = archive_round_trip(&reordered);
        assert!(matches!(
            reordered.seal(&profile),
            Err(BlockDiscoveryError::NoncanonicalPrefixOrder)
        ));

        let one_boundary = BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::new(vec![parenthesis]),
            )],
            vec![BoundaryDiscoveryTransition::new(root, parenthesis, root)],
        );
        let noncanonical_alphabet = BlockTreeDiscoveryConfiguration {
            boundaries: one_boundary.clone(),
            prefixes: vec![BlockPrefixAttachment::new(
                parenthesis,
                BlockPrefixRule::new(
                    ".",
                    CharacterClass::Characters(CharacterSet::from_unchecked_for_test(vec![
                        'z', 'a', 'z',
                    ])),
                ),
            )],
        };
        let noncanonical_alphabet = archive_round_trip(&noncanonical_alphabet);
        assert!(matches!(
            noncanonical_alphabet.seal(&profile),
            Err(BlockDiscoveryError::NoncanonicalPrefixAlphabet { boundary })
                if boundary == parenthesis
        ));

        let empty_separator = BlockTreeDiscoveryConfiguration {
            boundaries: one_boundary,
            prefixes: vec![BlockPrefixAttachment::new(
                parenthesis,
                BlockPrefixRule::new("", CharacterClass::AsciiAlphabetic),
            )],
        };
        let empty_separator = archive_round_trip(&empty_separator);
        assert!(matches!(
            empty_separator.seal(&profile),
            Err(BlockDiscoveryError::EmptyPrefixSeparator { boundary }) if boundary == parenthesis
        ));
    }
}
