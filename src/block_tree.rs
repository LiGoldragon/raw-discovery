//! Source-bounded block discovery.
//!
//! This module converts a single outside-in boundary traversal into untyped
//! source bounds. Its context, transition, and prefix rules are archiveable
//! canonical data; sealing binds them to one token profile. The resulting tree
//! and its source bounds remain runtime-only.

use thiserror::Error;

use crate::{
    BoundaryDiscoveryConfiguration, BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryError,
    CharacterClass, DiscoveredDelimitedBoundary, SealedBoundaryDiscoveryConfiguration,
    SealedTokenProfile, SealedTriggerSet, SourceBound, TokenProfileError, Trigger,
    TriggerIdentifier,
};

/// One inclusive spelling recorded as a block cue.
///
/// The cue bound is the opening spelling in the source. Its evidence is the
/// typed sealed rule identity that matched that spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockCue<Evidence = TriggerIdentifier> {
    bound: SourceBound,
    evidence: Evidence,
}

impl<Evidence: Copy> BlockCue<Evidence> {
    /// The source bound of the opening boundary spelling.
    pub fn bound(self) -> SourceBound {
        self.bound
    }

    /// The sealed boundary or cue rule that supplied this cue.
    pub fn evidence(self) -> Evidence {
        self.evidence
    }
}

/// Canonical identity of one cue-to-termination rule.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct CueTerminationRuleIdentifier(u16);

impl CueTerminationRuleIdentifier {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Canonical pass-1 rule for a word cue and its typed termination.
///
/// `word_characters` is used only to prove that the cue is a complete word,
/// rather than a prefix within another word. The rule does not describe the
/// block's grammar.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum CueTermination {
    /// End at an exact spelling observed at the cue's source level.
    ExactSpelling(String),
    /// End after one complete balanced boundary observed at the cue's source
    /// level. The boundary remains a discovered child of the cue block.
    ClosingBoundary(TriggerIdentifier),
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CueTerminationRule {
    identifier: CueTerminationRuleIdentifier,
    cue: String,
    termination: CueTermination,
    word_characters: CharacterClass,
}

impl CueTerminationRule {
    pub fn new(
        identifier: CueTerminationRuleIdentifier,
        cue: impl Into<String>,
        termination: impl Into<String>,
        word_characters: CharacterClass,
    ) -> Self {
        Self {
            identifier,
            cue: cue.into(),
            termination: CueTermination::ExactSpelling(termination.into()),
            word_characters,
        }
    }

    pub fn through_boundary(
        identifier: CueTerminationRuleIdentifier,
        cue: impl Into<String>,
        boundary: TriggerIdentifier,
        word_characters: CharacterClass,
    ) -> Self {
        Self {
            identifier,
            cue: cue.into(),
            termination: CueTermination::ClosingBoundary(boundary),
            word_characters,
        }
    }

    pub fn identifier(&self) -> CueTerminationRuleIdentifier {
        self.identifier
    }

    pub fn cue(&self) -> &str {
        &self.cue
    }

    pub fn termination(&self) -> &CueTermination {
        &self.termination
    }

    pub fn word_characters(&self) -> &CharacterClass {
        &self.word_characters
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

    /// The configured root context for source at the outermost level.
    pub fn root_context(&self) -> BoundaryDiscoveryContextIdentifier {
        self.boundaries.root()
    }

    /// The exact, profile-sealed discovery triggers admitted at one context.
    ///
    /// Consumers that interpret already-discovered source use this instead of
    /// flattening trigger sets across recursive contexts. The returned set is
    /// the same one the boundary reader validated while building the tree.
    pub fn active_triggers(
        &self,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<&SealedTriggerSet, BoundaryDiscoveryError> {
        self.boundaries.active_triggers(context)
    }

    /// The context governing the interior of `boundary` opened at `context`.
    ///
    /// This is a validated transition from the sealed discovery configuration;
    /// callers cannot infer it from trigger ordering or a global default.
    pub fn child_context(
        &self,
        context: BoundaryDiscoveryContextIdentifier,
        boundary: TriggerIdentifier,
    ) -> Result<BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryError> {
        self.boundaries.child_context(context, boundary)
    }
}

/// The universal, untyped block-tree projections.
///
/// Implementors expose runtime source mechanics only. The content bound is
/// the text within the opening and closing spellings; child order follows the
/// source order in that bound.
pub trait BlockTree {
    /// The typed rule-identity carried by this language's cue.
    type CueEvidence: Copy;

    /// The complete source range of this block, including an attached prefix,
    /// opening, content, and closing spelling.
    fn source_bound(&self) -> SourceBound;

    /// The opening boundary and the profile declaration that matched it.
    fn cue(&self) -> BlockCue<Self::CueEvidence>;

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
    type CueEvidence = TriggerIdentifier;

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

/// Evidence for either a cue-terminated block or one of its delimited children.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CueTerminatedBlockCueEvidence {
    CueTermination(CueTerminationRuleIdentifier),
    Boundary(TriggerIdentifier),
}

/// One runtime-only cue-terminated block and its recursively delimited children.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredCueTerminatedBlock {
    source: SourceBound,
    cue: BlockCue<CueTerminatedBlockCueEvidence>,
    content: SourceBound,
    closing: SourceBound,
    children: Vec<Self>,
}

impl BlockTree for DiscoveredCueTerminatedBlock {
    type CueEvidence = CueTerminatedBlockCueEvidence;

    fn source_bound(&self) -> SourceBound {
        self.source
    }

    fn cue(&self) -> BlockCue<Self::CueEvidence> {
        self.cue
    }

    fn prefix(&self) -> Option<BlockPrefix> {
        None
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

/// Canonical archiveable rules for cue-to-termination pass-1 discovery.
///
/// Delimiter, carrier, comment, and trivia behavior is supplied by the same
/// boundary configuration used by [`DiscoveredBlockTree`]. Cue rules add only
/// complete-word beginnings and exact termination spellings.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CueTerminatedBlockDiscoveryConfiguration {
    boundaries: BoundaryDiscoveryConfiguration,
    rules: Vec<CueTerminationRule>,
}

impl CueTerminatedBlockDiscoveryConfiguration {
    pub fn new(
        boundaries: BoundaryDiscoveryConfiguration,
        mut rules: Vec<CueTerminationRule>,
    ) -> Self {
        rules.sort_unstable_by_key(CueTerminationRule::identifier);
        Self { boundaries, rules }
    }

    pub fn boundaries(&self) -> &BoundaryDiscoveryConfiguration {
        &self.boundaries
    }

    pub fn rules(&self) -> &[CueTerminationRule] {
        &self.rules
    }

    pub fn seal(
        &self,
        profile: &SealedTokenProfile,
    ) -> Result<SealedCueTerminatedBlockDiscoveryConfiguration, BlockDiscoveryError> {
        let boundaries = self.boundaries.seal(profile)?;
        let mut rule_identifiers: Vec<_> = self
            .rules
            .iter()
            .map(CueTerminationRule::identifier)
            .collect();
        rule_identifiers.sort_unstable();
        if let Some(&rule) = rule_identifiers
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(&pair[0]))
        {
            return Err(BlockDiscoveryError::DuplicateCueRule { rule });
        }
        if !self
            .rules
            .windows(2)
            .all(|pair| pair[0].identifier() < pair[1].identifier())
        {
            return Err(BlockDiscoveryError::NoncanonicalCueRuleOrder);
        }
        for rule in &self.rules {
            if rule.cue.is_empty() {
                return Err(BlockDiscoveryError::EmptyCue {
                    rule: rule.identifier,
                });
            }
            match &rule.termination {
                CueTermination::ExactSpelling(spelling) if spelling.is_empty() => {
                    return Err(BlockDiscoveryError::EmptyTermination {
                        rule: rule.identifier,
                    });
                }
                CueTermination::ClosingBoundary(boundary) => {
                    if !boundaries
                        .active_triggers(boundaries.root())?
                        .triggers()
                        .contains(boundary)
                    {
                        return Err(TokenProfileError::InactiveBoundary {
                            identifier: *boundary,
                        }
                        .into());
                    }
                    if !matches!(
                        profile.definition(*boundary)?.trigger,
                        Trigger::Boundary { .. }
                    ) {
                        return Err(TokenProfileError::UnsupportedBoundaryDiscoveryTrigger(
                            *boundary,
                        )
                        .into());
                    }
                }
                CueTermination::ExactSpelling(_) => {}
            }
            if !rule.word_characters.is_canonical() {
                return Err(BlockDiscoveryError::NoncanonicalCueAlphabet {
                    rule: rule.identifier,
                });
            }
            if !rule
                .cue
                .chars()
                .all(|character| rule.word_characters.matches(character))
            {
                return Err(BlockDiscoveryError::CueOutsideWordAlphabet {
                    rule: rule.identifier,
                });
            }
            ensure_rule_spelling_is_disjoint(profile, &boundaries, rule.identifier, &rule.cue)?;
            if let CueTermination::ExactSpelling(termination) = &rule.termination {
                ensure_rule_spelling_is_disjoint(
                    profile,
                    &boundaries,
                    rule.identifier,
                    termination,
                )?;
            }
        }
        for (position, left) in self.rules.iter().enumerate() {
            if let Some(right) = self.rules[position + 1..]
                .iter()
                .find(|right| right.cue == left.cue)
            {
                return Err(BlockDiscoveryError::DuplicateCueSpelling {
                    first: left.identifier,
                    second: right.identifier,
                });
            }
        }
        Ok(SealedCueTerminatedBlockDiscoveryConfiguration {
            boundaries,
            rules: self.rules.clone(),
        })
    }
}

/// Runtime cue rules proven against one exact token profile.
#[derive(Clone, Debug)]
pub struct SealedCueTerminatedBlockDiscoveryConfiguration {
    boundaries: SealedBoundaryDiscoveryConfiguration,
    rules: Vec<CueTerminationRule>,
}

impl SealedCueTerminatedBlockDiscoveryConfiguration {
    fn matches_profile(&self, profile: &SealedTokenProfile) -> bool {
        self.boundaries.matches_profile(profile)
    }

    pub fn rules(&self) -> &[CueTerminationRule] {
        &self.rules
    }
}

/// The source-ordered roots found by cue-to-termination pass 1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredCueTerminatedBlockTree {
    root_blocks: Vec<DiscoveredCueTerminatedBlock>,
}

impl DiscoveredCueTerminatedBlockTree {
    pub fn discover(
        source: &str,
        profile: &SealedTokenProfile,
        configuration: &SealedCueTerminatedBlockDiscoveryConfiguration,
    ) -> Result<Self, BlockDiscoveryError> {
        if !configuration.matches_profile(profile) {
            return Err(TokenProfileError::TriggerSetProfileMismatch.into());
        }
        let mut reader = crate::BoundaryReader::new(source, profile);
        let root_context = configuration.boundaries.root();
        let active = configuration.boundaries.active_triggers(root_context)?;
        let mut root_blocks = Vec::new();
        while !reader.is_end() {
            if let Some(rule) =
                matching_cue_rule(source, reader.byte_offset(), &configuration.rules)
            {
                let cue_start = reader.byte_offset();
                let cue = reader
                    .consume_exact_spelling(rule.cue())
                    .expect("the selected cue was matched at the current cursor");
                let (children, closing) = match rule.termination() {
                    CueTermination::ExactSpelling(termination) => {
                        reader.discover_children_until(termination, &configuration.boundaries)?
                    }
                    CueTermination::ClosingBoundary(boundary) => reader
                        .discover_children_through_boundary(*boundary, &configuration.boundaries)?,
                };
                let Some(closing) = closing else {
                    return Err(BlockDiscoveryError::UnclosedCueTerminatedBlock {
                        rule: rule.identifier(),
                        cue,
                    });
                };
                let children = children
                    .iter()
                    .map(DiscoveredCueTerminatedBlock::from_boundary)
                    .collect::<Vec<_>>();
                root_blocks.push(DiscoveredCueTerminatedBlock {
                    source: SourceBound::checked(source, cue_start, closing.end())?,
                    cue: BlockCue {
                        bound: cue,
                        evidence: CueTerminatedBlockCueEvidence::CueTermination(rule.identifier()),
                    },
                    content: SourceBound::checked(source, cue.end(), closing.start())?,
                    closing,
                    children,
                });
                continue;
            }

            if reader.longest_match(active)?.is_some() {
                reader.consume(active)?;
            } else {
                reader.advance_uninterpreted_character();
            }
        }
        Ok(Self { root_blocks })
    }

    pub fn root_blocks(&self) -> &[DiscoveredCueTerminatedBlock] {
        &self.root_blocks
    }
}

impl DiscoveredCueTerminatedBlock {
    fn from_boundary(discovered: &DiscoveredDelimitedBoundary) -> Self {
        let boundary = discovered.boundary();
        Self {
            source: SourceBound::between(boundary.opening().start(), boundary.closing().end()),
            cue: BlockCue {
                bound: boundary.opening(),
                evidence: CueTerminatedBlockCueEvidence::Boundary(boundary.identifier()),
            },
            content: boundary.interior(),
            closing: boundary.closing(),
            children: discovered
                .children()
                .iter()
                .map(Self::from_boundary)
                .collect(),
        }
    }
}

fn matching_cue_rule<'rules>(
    source: &str,
    byte_offset: usize,
    rules: &'rules [CueTerminationRule],
) -> Option<&'rules CueTerminationRule> {
    rules
        .iter()
        .filter(|rule| cue_matches(source, byte_offset, rule))
        .max_by_key(|rule| rule.cue.len())
}

fn cue_matches(source: &str, byte_offset: usize, rule: &CueTerminationRule) -> bool {
    let Some(after_cue) = byte_offset.checked_add(rule.cue.len()) else {
        return false;
    };
    if source.get(byte_offset..after_cue) != Some(rule.cue()) {
        return false;
    }
    let before_is_word = source[..byte_offset]
        .chars()
        .next_back()
        .is_some_and(|character| rule.word_characters.matches(character));
    let after_is_word = source[after_cue..]
        .chars()
        .next()
        .is_some_and(|character| rule.word_characters.matches(character));
    !before_is_word && !after_is_word
}

fn ensure_rule_spelling_is_disjoint(
    profile: &SealedTokenProfile,
    boundaries: &SealedBoundaryDiscoveryConfiguration,
    rule: CueTerminationRuleIdentifier,
    spelling: &str,
) -> Result<(), BlockDiscoveryError> {
    for context in boundaries.contexts() {
        for trigger in boundaries.active_triggers(context)?.triggers() {
            if trigger_spelling_overlaps(profile.definition(*trigger)?, spelling) {
                return Err(BlockDiscoveryError::CueRuleTriggerOverlap {
                    rule,
                    trigger: *trigger,
                });
            }
        }
    }
    Ok(())
}

fn trigger_spelling_overlaps(definition: &crate::TriggerDefinition, spelling: &str) -> bool {
    let prefixes_overlap = |other: &str| spelling.starts_with(other) || other.starts_with(spelling);
    match &definition.trigger {
        Trigger::Boundary { opening, closing } => {
            prefixes_overlap(opening) || prefixes_overlap(closing)
        }
        Trigger::Carrier { opening, .. } => prefixes_overlap(opening),
        Trigger::CurlyText => prefixes_overlap("“"),
        Trigger::LineComment { opening } => prefixes_overlap(opening),
        Trigger::Whitespace { .. } => spelling.chars().next().is_some_and(char::is_whitespace),
        Trigger::Application { .. }
        | Trigger::Punctuation { .. }
        | Trigger::LeadingCharacterClass { .. } => false,
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

    #[error("cue-termination rule {rule:?} is declared more than once")]
    DuplicateCueRule { rule: CueTerminationRuleIdentifier },

    #[error("cue-termination rules are not in canonical identifier order")]
    NoncanonicalCueRuleOrder,

    #[error("cue-termination rules {first:?} and {second:?} have the same cue spelling")]
    DuplicateCueSpelling {
        first: CueTerminationRuleIdentifier,
        second: CueTerminationRuleIdentifier,
    },

    #[error("cue-termination rule {rule:?} has an empty cue")]
    EmptyCue { rule: CueTerminationRuleIdentifier },

    #[error("cue-termination rule {rule:?} has an empty termination spelling")]
    EmptyTermination { rule: CueTerminationRuleIdentifier },

    #[error("cue-termination rule {rule:?} has a noncanonical word alphabet")]
    NoncanonicalCueAlphabet { rule: CueTerminationRuleIdentifier },

    #[error("cue-termination rule {rule:?} contains a cue character outside its word alphabet")]
    CueOutsideWordAlphabet { rule: CueTerminationRuleIdentifier },

    #[error("cue-termination rule {rule:?} overlaps boundary-discovery trigger {trigger:?}")]
    CueRuleTriggerOverlap {
        rule: CueTerminationRuleIdentifier,
        trigger: TriggerIdentifier,
    },

    #[error(
        "cue-termination rule {rule:?} opened at byte {} but no termination was found",
        cue.start()
    )]
    UnclosedCueTerminatedBlock {
        rule: CueTerminationRuleIdentifier,
        cue: SourceBound,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        BlockDiscoveryError, BlockPrefixAttachment, BlockPrefixRule,
        BlockTreeDiscoveryConfiguration,
    };
    use crate::{
        BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
        BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryError, BoundaryDiscoveryTransition,
        CharacterClass, CharacterSet, RawProfile, TriggerIdentifier, TriggerSet,
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

    #[test]
    fn sealed_context_accessors_preserve_active_set_and_transition() {
        let profile = RawProfile::standard().seal().expect("standard profile");
        let root = BoundaryDiscoveryContextIdentifier::new(97);
        let child = BoundaryDiscoveryContextIdentifier::new(98);
        let parenthesis = TriggerIdentifier::new(0);
        let curly_text = TriggerIdentifier::new(4);
        let configuration = BlockTreeDiscoveryConfiguration::new(
            BoundaryDiscoveryConfiguration::new(
                root,
                vec![
                    BoundaryDiscoveryContext::new(root, TriggerSet::new(vec![parenthesis])),
                    BoundaryDiscoveryContext::new(child, TriggerSet::new(vec![curly_text])),
                ],
                vec![BoundaryDiscoveryTransition::new(root, parenthesis, child)],
            ),
            vec![],
        )
        .seal(&profile)
        .expect("sealed contexts");

        assert_eq!(configuration.root_context(), root);
        assert_eq!(
            configuration
                .child_context(root, parenthesis)
                .expect("declared transition"),
            child
        );
        assert_eq!(
            configuration
                .active_triggers(child)
                .expect("child active set")
                .triggers(),
            &[curly_text]
        );

        let unknown = BoundaryDiscoveryContextIdentifier::new(99);
        assert!(matches!(
            configuration.active_triggers(unknown),
            Err(BoundaryDiscoveryError::UnknownContext { context }) if context == unknown
        ));
        let undeclared = TriggerIdentifier::new(1);
        assert!(matches!(
            configuration.child_context(root, undeclared),
            Err(BoundaryDiscoveryError::MissingChildContext { context, boundary })
                if context == root && boundary == undeclared
        ));
    }
}
