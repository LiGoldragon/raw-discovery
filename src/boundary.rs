//! Cursor-local boundary matching under one sealed active trigger set.
//!
//! This reader never produces a preliminary token stream. A caller supplies the
//! trigger set for its current expected structural position, consumes one
//! boundary event, and recursively supplies the next position's set.

use content_identity::ContentHash;

use crate::profile::{
    CharacterClass, SealedBoundaryDiscoverySet, SealedTokenProfile, SealedTriggerSet,
    TokenProfileDomain, TokenProfileError, Trigger, TriggerIdentifier, TriggerSet,
};
use thiserror::Error;

/// One validated half-open range within a source text.
///
/// Bounds are runtime cursor state, not archiveable structure or durable
/// identity. A delimited discovery creates the interior bound; recursive
/// readers consume only that bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceBound {
    start: usize,
    end: usize,
}

impl SourceBound {
    pub fn whole(source: &str) -> Self {
        Self {
            start: 0,
            end: source.len(),
        }
    }

    pub fn checked(source: &str, start: usize, end: usize) -> Result<Self, TokenProfileError> {
        if start > end
            || end > source.len()
            || !source.is_char_boundary(start)
            || !source.is_char_boundary(end)
        {
            return Err(TokenProfileError::InvalidSourceBound {
                start,
                end,
                source_length: source.len(),
            });
        }
        Ok(Self { start, end })
    }

    pub fn start(self) -> usize {
        self.start
    }

    pub fn end(self) -> usize {
        self.end
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    fn between(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// The outside boundary and explicitly bounded interior discovered before any
/// child form is interpreted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelimitedBoundary {
    identifier: TriggerIdentifier,
    opening: SourceBound,
    interior: SourceBound,
    closing: SourceBound,
}

impl DelimitedBoundary {
    pub fn identifier(self) -> TriggerIdentifier {
        self.identifier
    }

    pub fn opening(self) -> SourceBound {
        self.opening
    }

    pub fn interior(self) -> SourceBound {
        self.interior
    }

    pub fn closing(self) -> SourceBound {
        self.closing
    }

    pub fn end(self) -> usize {
        self.closing.end
    }
}

/// Canonical identity of one boundary-discovery context.
///
/// A context selects the boundary, carrier, and trivia rules active at one
/// source level. It is archived rule data; sealing binds it to one exact token
/// profile before discovery can use it.
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
pub struct BoundaryDiscoveryContextIdentifier(u16);

impl BoundaryDiscoveryContextIdentifier {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

/// One context's declared outside-in discovery triggers.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct BoundaryDiscoveryContext {
    identifier: BoundaryDiscoveryContextIdentifier,
    triggers: TriggerSet,
}

impl BoundaryDiscoveryContext {
    pub fn new(identifier: BoundaryDiscoveryContextIdentifier, triggers: TriggerSet) -> Self {
        Self {
            identifier,
            triggers,
        }
    }

    pub fn identifier(&self) -> BoundaryDiscoveryContextIdentifier {
        self.identifier
    }

    pub fn triggers(&self) -> &TriggerSet {
        &self.triggers
    }
}

/// A boundary-specific transition to the context governing its interior.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundaryDiscoveryTransition {
    context: BoundaryDiscoveryContextIdentifier,
    boundary: TriggerIdentifier,
    child_context: BoundaryDiscoveryContextIdentifier,
}

impl BoundaryDiscoveryTransition {
    pub fn new(
        context: BoundaryDiscoveryContextIdentifier,
        boundary: TriggerIdentifier,
        child_context: BoundaryDiscoveryContextIdentifier,
    ) -> Self {
        Self {
            context,
            boundary,
            child_context,
        }
    }

    pub fn context(self) -> BoundaryDiscoveryContextIdentifier {
        self.context
    }

    pub fn boundary(self) -> TriggerIdentifier {
        self.boundary
    }

    pub fn child_context(self) -> BoundaryDiscoveryContextIdentifier {
        self.child_context
    }
}

/// Canonical rule data for context-specific outside-in boundary discovery.
///
/// The root context governs top-level source. Each active boundary has an
/// explicit transition to the context used to find and record its children.
/// Construction normalizes context and transition order. Sealing rejects an
/// archived value that bypassed those constructors with noncanonical nested
/// trigger, context, or transition ordering.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct BoundaryDiscoveryConfiguration {
    root: BoundaryDiscoveryContextIdentifier,
    contexts: Vec<BoundaryDiscoveryContext>,
    transitions: Vec<BoundaryDiscoveryTransition>,
}

impl BoundaryDiscoveryConfiguration {
    pub fn new(
        root: BoundaryDiscoveryContextIdentifier,
        mut contexts: Vec<BoundaryDiscoveryContext>,
        mut transitions: Vec<BoundaryDiscoveryTransition>,
    ) -> Self {
        contexts.sort_unstable_by_key(BoundaryDiscoveryContext::identifier);
        transitions.sort_unstable_by_key(|transition| {
            (
                transition.context(),
                transition.boundary(),
                transition.child_context(),
            )
        });
        Self {
            root,
            contexts,
            transitions,
        }
    }

    pub fn root(&self) -> BoundaryDiscoveryContextIdentifier {
        self.root
    }

    pub fn contexts(&self) -> &[BoundaryDiscoveryContext] {
        &self.contexts
    }

    pub fn transitions(&self) -> &[BoundaryDiscoveryTransition] {
        &self.transitions
    }

    pub fn seal(
        &self,
        profile: &SealedTokenProfile,
    ) -> Result<SealedBoundaryDiscoveryConfiguration, BoundaryDiscoveryError> {
        let mut context_identifiers: Vec<_> = self
            .contexts
            .iter()
            .map(BoundaryDiscoveryContext::identifier)
            .collect();
        context_identifiers.sort_unstable();
        if let Some(&context) = context_identifiers
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(&pair[0]))
        {
            return Err(BoundaryDiscoveryError::DuplicateContext { context });
        }
        if !self
            .contexts
            .windows(2)
            .all(|pair| pair[0].identifier() < pair[1].identifier())
        {
            return Err(BoundaryDiscoveryError::NoncanonicalContextOrder);
        }
        for context in &self.contexts {
            if !context.triggers.is_canonical() {
                return Err(BoundaryDiscoveryError::NoncanonicalContextTriggerSet {
                    context: context.identifier,
                });
            }
        }

        let mut transition_keys: Vec<_> = self
            .transitions
            .iter()
            .map(|transition| (transition.context(), transition.boundary()))
            .collect();
        transition_keys.sort_unstable();
        if let Some(&(context, boundary)) = transition_keys
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(&pair[0]))
        {
            return Err(BoundaryDiscoveryError::DuplicateTransition { context, boundary });
        }
        if !self.transitions.windows(2).all(|pair| {
            (
                pair[0].context(),
                pair[0].boundary(),
                pair[0].child_context(),
            ) < (
                pair[1].context(),
                pair[1].boundary(),
                pair[1].child_context(),
            )
        }) {
            return Err(BoundaryDiscoveryError::NoncanonicalTransitionOrder);
        }

        let mut contexts = Vec::with_capacity(self.contexts.len());
        for context in &self.contexts {
            contexts.push(SealedBoundaryDiscoveryContext {
                identifier: context.identifier,
                active: profile.seal_boundary_discovery_set(context.triggers.clone())?,
            });
        }
        let sealed = SealedBoundaryDiscoveryConfiguration {
            root: self.root,
            contexts,
            transitions: self.transitions.clone(),
            profile_identity: profile.identity(),
        };
        sealed.context(self.root)?;

        for transition in &sealed.transitions {
            let parent = sealed.context(transition.context)?;
            sealed.context(transition.child_context)?;
            if !parent.active.triggers().contains(&transition.boundary) {
                return Err(BoundaryDiscoveryError::InactiveTransitionBoundary {
                    context: transition.context,
                    boundary: transition.boundary,
                });
            }
            if !matches!(
                profile.definition(transition.boundary)?.trigger,
                Trigger::Boundary { .. }
            ) {
                return Err(BoundaryDiscoveryError::TransitionRequiresBoundary {
                    context: transition.context,
                    trigger: transition.boundary,
                });
            }
        }
        for context in &sealed.contexts {
            for identifier in context.active.triggers() {
                if matches!(
                    profile.definition(*identifier)?.trigger,
                    Trigger::Boundary { .. }
                ) && !sealed.transitions.iter().any(|transition| {
                    transition.context == context.identifier && transition.boundary == *identifier
                }) {
                    return Err(BoundaryDiscoveryError::MissingChildContext {
                        context: context.identifier,
                        boundary: *identifier,
                    });
                }
            }
        }
        Ok(sealed)
    }
}

/// A sealed runtime discovery configuration bound to one token profile.
#[derive(Clone, Debug)]
pub struct SealedBoundaryDiscoveryConfiguration {
    root: BoundaryDiscoveryContextIdentifier,
    contexts: Vec<SealedBoundaryDiscoveryContext>,
    transitions: Vec<BoundaryDiscoveryTransition>,
    profile_identity: ContentHash<TokenProfileDomain>,
}

impl SealedBoundaryDiscoveryConfiguration {
    pub fn root(&self) -> BoundaryDiscoveryContextIdentifier {
        self.root
    }

    fn context(
        &self,
        identifier: BoundaryDiscoveryContextIdentifier,
    ) -> Result<&SealedBoundaryDiscoveryContext, BoundaryDiscoveryError> {
        self.contexts
            .iter()
            .find(|context| context.identifier == identifier)
            .ok_or(BoundaryDiscoveryError::UnknownContext {
                context: identifier,
            })
    }

    pub(crate) fn child_context(
        &self,
        context: BoundaryDiscoveryContextIdentifier,
        boundary: TriggerIdentifier,
    ) -> Result<BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryError> {
        self.transitions
            .iter()
            .find(|transition| transition.context == context && transition.boundary == boundary)
            .map(|transition| transition.child_context())
            .ok_or(BoundaryDiscoveryError::MissingChildContext { context, boundary })
    }

    pub(crate) fn active_triggers(
        &self,
        context: BoundaryDiscoveryContextIdentifier,
    ) -> Result<&SealedTriggerSet, BoundaryDiscoveryError> {
        Ok(self.context(context)?.active.active_triggers())
    }

    pub(crate) fn configures_boundary(&self, boundary: TriggerIdentifier) -> bool {
        self.contexts
            .iter()
            .any(|context| context.active.triggers().contains(&boundary))
    }

    pub(crate) fn matches_profile(&self, profile: &SealedTokenProfile) -> bool {
        self.profile_identity == profile.identity()
    }
}

#[derive(Clone, Debug)]
struct SealedBoundaryDiscoveryContext {
    identifier: BoundaryDiscoveryContextIdentifier,
    active: SealedBoundaryDiscoverySet,
}

/// A source-bounded tree of generic delimited boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredDelimitedBoundary {
    boundary: DelimitedBoundary,
    children: Vec<Self>,
}

impl DiscoveredDelimitedBoundary {
    pub fn boundary(&self) -> DelimitedBoundary {
        self.boundary
    }

    pub fn children(&self) -> &[Self] {
        &self.children
    }
}

/// Failures while applying runtime boundary-discovery context data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BoundaryDiscoveryError {
    #[error(transparent)]
    Profile(#[from] TokenProfileError),

    #[error("boundary discovery context {context:?} is declared more than once")]
    DuplicateContext {
        context: BoundaryDiscoveryContextIdentifier,
    },

    #[error("boundary discovery contexts are not in canonical identifier order")]
    NoncanonicalContextOrder,

    #[error("boundary discovery trigger set for {context:?} is not canonical")]
    NoncanonicalContextTriggerSet {
        context: BoundaryDiscoveryContextIdentifier,
    },

    #[error("boundary discovery context {context:?} is not declared")]
    UnknownContext {
        context: BoundaryDiscoveryContextIdentifier,
    },

    #[error("boundary {boundary:?} has more than one child context from {context:?}")]
    DuplicateTransition {
        context: BoundaryDiscoveryContextIdentifier,
        boundary: TriggerIdentifier,
    },

    #[error("boundary discovery transitions are not in canonical order")]
    NoncanonicalTransitionOrder,

    #[error("boundary {boundary:?} is not active in context {context:?}")]
    InactiveTransitionBoundary {
        context: BoundaryDiscoveryContextIdentifier,
        boundary: TriggerIdentifier,
    },

    #[error(
        "trigger {trigger:?} cannot select a child boundary-discovery context from {context:?}"
    )]
    TransitionRequiresBoundary {
        context: BoundaryDiscoveryContextIdentifier,
        trigger: TriggerIdentifier,
    },

    #[error("boundary {boundary:?} opened in context {context:?} has no child context")]
    MissingChildContext {
        context: BoundaryDiscoveryContextIdentifier,
        boundary: TriggerIdentifier,
    },

    #[error("unexpected closing boundary {identifier:?} at byte {}", bound.start())]
    UnexpectedClose {
        identifier: TriggerIdentifier,
        bound: SourceBound,
    },
}

/// The side of a configured boundary matched at the cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundarySide {
    Opening,
    Closing,
}

/// The kind of one cursor-local complete match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerMatchKind {
    Boundary(BoundarySide),
    Application,
    Punctuation,
    Carrier,
    Trivia,
    LeadingCharacterClass,
}

/// One complete trigger match at the current cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggerMatch {
    identifier: TriggerIdentifier,
    kind: TriggerMatchKind,
    start: usize,
    end: usize,
    body: Option<String>,
}

impl TriggerMatch {
    pub fn identifier(&self) -> TriggerIdentifier {
        self.identifier
    }

    pub fn kind(&self) -> TriggerMatchKind {
        self.kind
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    /// Carrier content or leading-class spelling captured by this match.
    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn is_trivia(&self) -> bool {
        matches!(self.kind, TriggerMatchKind::Trivia)
    }
}

/// A data-bearing cursor into one source text under one sealed profile.
pub struct BoundaryReader<'source, 'profile> {
    source: &'source str,
    profile: &'profile SealedTokenProfile,
    byte_offset: usize,
    bound: SourceBound,
    #[cfg(test)]
    boundary_entries: usize,
}

#[derive(Clone, Copy)]
enum BoundaryTraversal<'a> {
    Static(&'a SealedBoundaryDiscoverySet),
    Configured {
        configuration: &'a SealedBoundaryDiscoveryConfiguration,
        context: BoundaryDiscoveryContextIdentifier,
    },
}

impl<'a> BoundaryTraversal<'a> {
    fn active(self) -> Result<&'a SealedBoundaryDiscoverySet, BoundaryDiscoveryError> {
        match self {
            Self::Static(active) => Ok(active),
            Self::Configured {
                configuration,
                context,
            } => Ok(&configuration.context(context)?.active),
        }
    }

    fn child_for(
        self,
        boundary: TriggerIdentifier,
    ) -> Result<BoundaryTraversal<'a>, BoundaryDiscoveryError> {
        match self {
            Self::Static(active) => Ok(Self::Static(active)),
            Self::Configured {
                configuration,
                context,
            } => Ok(Self::Configured {
                configuration,
                context: configuration.child_context(context, boundary)?,
            }),
        }
    }
}

#[derive(Clone, Copy)]
struct ExpectedBoundary {
    identifier: TriggerIdentifier,
    opening_start: usize,
}

impl<'source, 'profile> BoundaryReader<'source, 'profile> {
    pub fn new(source: &'source str, profile: &'profile SealedTokenProfile) -> Self {
        Self {
            source,
            profile,
            byte_offset: 0,
            bound: SourceBound::whole(source),
            #[cfg(test)]
            boundary_entries: 0,
        }
    }

    /// Start a cursor at the beginning of one bound previously obtained from
    /// this source by generic boundary discovery.
    pub fn within(
        source: &'source str,
        profile: &'profile SealedTokenProfile,
        bound: SourceBound,
    ) -> Result<Self, TokenProfileError> {
        let bound = SourceBound::checked(source, bound.start, bound.end)?;
        Ok(Self {
            source,
            profile,
            byte_offset: bound.start,
            bound,
            #[cfg(test)]
            boundary_entries: 0,
        })
    }

    pub fn bound(&self) -> SourceBound {
        self.bound
    }

    pub fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub fn is_end(&self) -> bool {
        self.byte_offset == self.bound.end
    }

    pub fn remaining(&self) -> &'source str {
        &self.source[self.byte_offset..self.bound.end]
    }

    pub fn source_between(&self, start: usize, end: usize) -> &'source str {
        assert!(
            start >= self.bound.start && end <= self.bound.end && start <= end,
            "source range must stay inside the reader bound"
        );
        &self.source[start..end]
    }

    pub fn advance_to(&mut self, byte_offset: usize) {
        assert!(
            byte_offset >= self.byte_offset
                && byte_offset <= self.bound.end
                && self.source.is_char_boundary(byte_offset),
            "boundary reader advances only to a later UTF-8 boundary"
        );
        self.byte_offset = byte_offset;
    }

    pub fn advance_character(&mut self) -> Option<char> {
        let character = self.remaining().chars().next()?;
        self.byte_offset += character.len_utf8();
        Some(character)
    }

    /// Match the longest complete trigger in the active set at the current
    /// cursor. Equal complete lengths from distinct definitions are refused even
    /// though the set's seal proof should make that state unreachable.
    pub fn longest_match(
        &self,
        active: &SealedTriggerSet,
    ) -> Result<Option<TriggerMatch>, TokenProfileError> {
        if active.profile_identity() != self.profile.identity() {
            return Err(TokenProfileError::TriggerSetProfileMismatch);
        }
        let mut winner: Option<TriggerMatch> = None;
        for identifier in active.triggers() {
            let definition = self.profile.definition(*identifier)?;
            let Some(candidate) = self.match_trigger(*identifier, &definition.trigger)? else {
                continue;
            };
            match &winner {
                None => winner = Some(candidate),
                Some(previous) => {
                    let previous_length = previous.end - previous.start;
                    let candidate_length = candidate.end - candidate.start;
                    if candidate_length > previous_length {
                        winner = Some(candidate);
                    } else if candidate_length == previous_length
                        && candidate.identifier != previous.identifier
                    {
                        return Err(TokenProfileError::AmbiguousTriggerSet {
                            first: previous.identifier,
                            second: candidate.identifier,
                        });
                    }
                }
            }
        }
        Ok(winner)
    }

    pub fn consume(
        &mut self,
        active: &SealedTriggerSet,
    ) -> Result<Option<TriggerMatch>, TokenProfileError> {
        let matched = self.longest_match(active)?;
        if let Some(matched) = &matched {
            self.advance_to(matched.end);
        }
        Ok(matched)
    }

    /// Consume every trivia match active at this recursive position.
    pub fn skip_trivia(&mut self, active: &SealedTriggerSet) -> Result<(), TokenProfileError> {
        loop {
            let Some(matched) = self.longest_match(active)? else {
                return Ok(());
            };
            if !matched.is_trivia() {
                return Ok(());
            }
            self.advance_to(matched.end);
        }
    }

    /// Read the negative space between active triggers as one bare atom.
    pub fn read_bare(
        &mut self,
        active: &SealedTriggerSet,
    ) -> Result<Option<String>, TokenProfileError> {
        if self.longest_match(active)?.is_some() || self.is_end() {
            return Ok(None);
        }
        let start = self.byte_offset;
        loop {
            if self.is_end() || self.longest_match(active)?.is_some() {
                break;
            }
            let position = self.byte_offset;
            let character = self
                .advance_character()
                .expect("the end condition was checked");
            if self.profile.bare_character_is_forbidden(character) {
                return Err(TokenProfileError::ForbiddenBareCharacter {
                    character,
                    byte_offset: position,
                });
            }
        }
        Ok(Some(self.source[start..self.byte_offset].to_owned()))
    }

    /// Discover one complete delimited region before interpreting any child.
    ///
    /// The reader must be positioned at the expected opening. The shared
    /// machinery then walks glyph-by-glyph inside this reader's existing bound,
    /// balancing every active nested boundary and consuming configured carriers
    /// and trivia opaquely. On success the parent cursor advances past the
    /// matching close and the returned interior is an independent bound for
    /// recursive interpretation.
    pub fn discover_delimited(
        &mut self,
        identifier: TriggerIdentifier,
        active: &SealedBoundaryDiscoverySet,
    ) -> Result<DelimitedBoundary, TokenProfileError> {
        if active.profile_identity() != self.profile.identity() {
            return Err(TokenProfileError::TriggerSetProfileMismatch);
        }
        if !active.triggers().contains(&identifier) {
            return Err(TokenProfileError::InactiveBoundary { identifier });
        }
        match self.discover_delimited_with(identifier, BoundaryTraversal::Static(active)) {
            Ok(discovered) => Ok(discovered.boundary),
            Err(BoundaryDiscoveryError::Profile(error)) => Err(error),
            Err(_) => unreachable!("a static boundary-discovery set has no context failures"),
        }
    }

    /// Discover the configured top-level boundary tree in one cursor traversal.
    ///
    /// Each context supplies the active boundary, carrier, and trivia rules at
    /// its source level. An opening boundary transitions to the context that
    /// governs its interior before the reader continues toward that boundary's
    /// matching close.
    pub fn discover_boundary_children(
        &mut self,
        configuration: &SealedBoundaryDiscoveryConfiguration,
    ) -> Result<Vec<DiscoveredDelimitedBoundary>, BoundaryDiscoveryError> {
        if !configuration.matches_profile(self.profile) {
            return Err(TokenProfileError::TriggerSetProfileMismatch.into());
        }
        let traversal = BoundaryTraversal::Configured {
            configuration,
            context: configuration.root(),
        };
        let (children, closing) = self.discover_children_with(None, traversal)?;
        debug_assert!(closing.is_none());
        Ok(children)
    }

    fn discover_delimited_with(
        &mut self,
        identifier: TriggerIdentifier,
        traversal: BoundaryTraversal<'_>,
    ) -> Result<DiscoveredDelimitedBoundary, BoundaryDiscoveryError> {
        #[cfg(test)]
        {
            self.boundary_entries += 1;
        }
        let Trigger::Boundary { opening, .. } = &self.profile.definition(identifier)?.trigger
        else {
            return Err(TokenProfileError::UnsupportedBoundaryDiscoveryTrigger(identifier).into());
        };
        let opening = self
            .exact_match(
                identifier,
                TriggerMatchKind::Boundary(BoundarySide::Opening),
                opening,
            )
            .ok_or(TokenProfileError::ExpectedBoundaryOpening {
                identifier,
                byte_offset: self.byte_offset,
            })?;
        self.advance_to(opening.end);
        let (children, closing) = self.discover_children_with(
            Some(ExpectedBoundary {
                identifier,
                opening_start: opening.start,
            }),
            traversal,
        )?;
        let closing = closing.expect("an expected boundary always returns its closing bound");
        Ok(DiscoveredDelimitedBoundary {
            boundary: DelimitedBoundary {
                identifier,
                opening: SourceBound::between(opening.start, opening.end),
                interior: SourceBound::between(opening.end, closing.start),
                closing,
            },
            children,
        })
    }

    fn discover_children_with(
        &mut self,
        expected: Option<ExpectedBoundary>,
        traversal: BoundaryTraversal<'_>,
    ) -> Result<(Vec<DiscoveredDelimitedBoundary>, Option<SourceBound>), BoundaryDiscoveryError>
    {
        let mut children = Vec::new();
        loop {
            if self.is_end() {
                return match expected {
                    Some(expected) => Err(TokenProfileError::UnclosedBoundary {
                        identifier: expected.identifier,
                        byte_offset: expected.opening_start,
                    }
                    .into()),
                    None => Ok((children, None)),
                };
            }

            let active = self.discovery_active(traversal)?;
            let Some(matched) = self.longest_discovery_match(active, expected)? else {
                self.advance_character()
                    .expect("the bounded end condition was checked");
                continue;
            };
            match matched.kind {
                TriggerMatchKind::Boundary(BoundarySide::Opening) => {
                    let child = self.discover_delimited_with(
                        matched.identifier,
                        traversal.child_for(matched.identifier)?,
                    )?;
                    children.push(child);
                }
                TriggerMatchKind::Boundary(BoundarySide::Closing) => match expected {
                    Some(expected) if expected.identifier == matched.identifier => {
                        let closing = SourceBound::between(matched.start, matched.end);
                        self.advance_to(matched.end);
                        return Ok((children, Some(closing)));
                    }
                    Some(expected) => {
                        return Err(TokenProfileError::MismatchedBoundary {
                            expected: expected.identifier,
                            found: matched.identifier,
                            byte_offset: matched.start,
                        }
                        .into());
                    }
                    None => {
                        return Err(BoundaryDiscoveryError::UnexpectedClose {
                            identifier: matched.identifier,
                            bound: SourceBound::between(matched.start, matched.end),
                        });
                    }
                },
                TriggerMatchKind::Carrier | TriggerMatchKind::Trivia => {
                    self.advance_to(matched.end);
                }
                TriggerMatchKind::Application
                | TriggerMatchKind::Punctuation
                | TriggerMatchKind::LeadingCharacterClass => {
                    unreachable!("a sealed boundary discovery set excludes horizontal triggers")
                }
            }
        }
    }

    fn discovery_active<'traversal>(
        &self,
        traversal: BoundaryTraversal<'traversal>,
    ) -> Result<&'traversal SealedBoundaryDiscoverySet, BoundaryDiscoveryError> {
        let active = traversal.active()?;
        if active.profile_identity() != self.profile.identity() {
            return Err(TokenProfileError::TriggerSetProfileMismatch.into());
        }
        Ok(active)
    }

    fn longest_discovery_match(
        &self,
        active: &SealedBoundaryDiscoverySet,
        expected: Option<ExpectedBoundary>,
    ) -> Result<Option<TriggerMatch>, BoundaryDiscoveryError> {
        let configured = self.longest_match(active.active_triggers())?;
        let expected = expected
            .map(|expected| {
                let Trigger::Boundary { closing, .. } =
                    &self.profile.definition(expected.identifier)?.trigger
                else {
                    return Err(TokenProfileError::UnsupportedBoundaryDiscoveryTrigger(
                        expected.identifier,
                    ));
                };
                Ok(self.exact_match(
                    expected.identifier,
                    TriggerMatchKind::Boundary(BoundarySide::Closing),
                    closing,
                ))
            })
            .transpose()?;
        match (configured, expected.flatten()) {
            (Some(configured), Some(expected)) => {
                let configured_length = configured.end - configured.start;
                let expected_length = expected.end - expected.start;
                if configured_length == expected_length
                    && configured.identifier != expected.identifier
                {
                    return Err(TokenProfileError::AmbiguousTriggerSet {
                        first: configured.identifier,
                        second: expected.identifier,
                    }
                    .into());
                }
                Ok((expected_length > configured_length)
                    .then_some(expected)
                    .or(Some(configured)))
            }
            (Some(configured), None) => Ok(Some(configured)),
            (None, Some(expected)) => Ok(Some(expected)),
            (None, None) => Ok(None),
        }
    }

    fn match_trigger(
        &self,
        identifier: TriggerIdentifier,
        trigger: &Trigger,
    ) -> Result<Option<TriggerMatch>, TokenProfileError> {
        match trigger {
            Trigger::Boundary { opening, closing } => {
                let opening_match = self.exact_match(
                    identifier,
                    TriggerMatchKind::Boundary(BoundarySide::Opening),
                    opening,
                );
                let closing_match = self.exact_match(
                    identifier,
                    TriggerMatchKind::Boundary(BoundarySide::Closing),
                    closing,
                );
                Ok(Self::longer(opening_match, closing_match))
            }
            Trigger::Application { glyph } => {
                Ok(self.exact_match(identifier, TriggerMatchKind::Application, glyph))
            }
            Trigger::Punctuation { glyph } => {
                Ok(self.exact_match(identifier, TriggerMatchKind::Punctuation, glyph))
            }
            Trigger::Carrier {
                opening,
                closing,
                escape,
            } => self.match_carrier(identifier, opening, closing, escape.as_deref()),
            Trigger::Whitespace { .. } => Ok(self.match_character_class(
                identifier,
                TriggerMatchKind::Trivia,
                &CharacterClass::Whitespace,
                &CharacterClass::Whitespace,
            )),
            Trigger::LineComment { opening } => {
                if !self.remaining().starts_with(opening) {
                    return Ok(None);
                }
                let end = self
                    .remaining()
                    .find('\n')
                    .map_or(self.bound.end, |relative| {
                        self.byte_offset + relative + '\n'.len_utf8()
                    });
                Ok(Some(TriggerMatch {
                    identifier,
                    kind: TriggerMatchKind::Trivia,
                    start: self.byte_offset,
                    end,
                    body: None,
                }))
            }
            Trigger::LeadingCharacterClass {
                leading,
                continuation,
            } => Ok(self.match_character_class(
                identifier,
                TriggerMatchKind::LeadingCharacterClass,
                leading,
                continuation,
            )),
        }
    }

    fn exact_match(
        &self,
        identifier: TriggerIdentifier,
        kind: TriggerMatchKind,
        glyph: &str,
    ) -> Option<TriggerMatch> {
        self.remaining().starts_with(glyph).then(|| TriggerMatch {
            identifier,
            kind,
            start: self.byte_offset,
            end: self.byte_offset + glyph.len(),
            body: None,
        })
    }

    fn match_carrier(
        &self,
        identifier: TriggerIdentifier,
        opening: &str,
        closing: &str,
        escape: Option<&str>,
    ) -> Result<Option<TriggerMatch>, TokenProfileError> {
        if !self.remaining().starts_with(opening) {
            return Ok(None);
        }
        let mut cursor = self.byte_offset + opening.len();
        let mut body = String::new();
        while cursor < self.bound.end {
            let remaining = &self.source[cursor..self.bound.end];
            if remaining.starts_with(closing) {
                return Ok(Some(TriggerMatch {
                    identifier,
                    kind: TriggerMatchKind::Carrier,
                    start: self.byte_offset,
                    end: cursor + closing.len(),
                    body: Some(body),
                }));
            }
            if let Some(escape) = escape {
                if remaining.starts_with(escape) {
                    cursor += escape.len();
                    let Some(character) = self.source[cursor..self.bound.end].chars().next() else {
                        body.push_str(escape);
                        break;
                    };
                    body.push(character);
                    cursor += character.len_utf8();
                    continue;
                }
            }
            let character = remaining
                .chars()
                .next()
                .expect("cursor remains inside source");
            body.push(character);
            cursor += character.len_utf8();
        }
        Err(TokenProfileError::UnclosedCarrier {
            identifier,
            byte_offset: self.byte_offset,
        })
    }

    fn match_character_class(
        &self,
        identifier: TriggerIdentifier,
        kind: TriggerMatchKind,
        leading: &CharacterClass,
        continuation: &CharacterClass,
    ) -> Option<TriggerMatch> {
        let mut characters = self.remaining().char_indices();
        let (_, first) = characters.next()?;
        if !leading.matches(first) {
            return None;
        }
        let mut length = first.len_utf8();
        for (position, character) in characters {
            if !continuation.matches(character) {
                break;
            }
            length = position + character.len_utf8();
        }
        Some(TriggerMatch {
            identifier,
            kind,
            start: self.byte_offset,
            end: self.byte_offset + length,
            body: (kind == TriggerMatchKind::LeadingCharacterClass)
                .then(|| self.source[self.byte_offset..self.byte_offset + length].to_owned()),
        })
    }

    fn longer(left: Option<TriggerMatch>, right: Option<TriggerMatch>) -> Option<TriggerMatch> {
        match (left, right) {
            (Some(left), Some(right)) if right.end - right.start > left.end - left.start => {
                Some(right)
            }
            (Some(left), _) => Some(left),
            (None, right) => right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
        BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryError, BoundaryDiscoveryTransition,
        BoundaryReader,
    };
    use crate::{RawProfile, TokenProfileError, TriggerIdentifier, TriggerSet};

    fn archive_round_trip(
        configuration: &BoundaryDiscoveryConfiguration,
    ) -> BoundaryDiscoveryConfiguration {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(configuration)
            .expect("archive boundary configuration");
        rkyv::from_bytes::<BoundaryDiscoveryConfiguration, rkyv::rancor::Error>(&bytes)
            .expect("validated boundary configuration archive")
    }

    #[test]
    fn configured_tree_enters_each_nested_boundary_once() {
        let profile = RawProfile::standard().seal().expect("standard profile");
        let root = BoundaryDiscoveryContextIdentifier::new(90);
        let child = BoundaryDiscoveryContextIdentifier::new(91);
        let active = TriggerSet::new(vec![
            TriggerIdentifier::new(0),
            TriggerIdentifier::new(1),
            TriggerIdentifier::new(4),
            TriggerIdentifier::new(5),
            TriggerIdentifier::new(6),
        ]);
        let configuration = BoundaryDiscoveryConfiguration::new(
            root,
            vec![
                BoundaryDiscoveryContext::new(root, active.clone()),
                BoundaryDiscoveryContext::new(child, active),
            ],
            vec![
                BoundaryDiscoveryTransition::new(root, TriggerIdentifier::new(0), child),
                BoundaryDiscoveryTransition::new(root, TriggerIdentifier::new(1), child),
                BoundaryDiscoveryTransition::new(child, TriggerIdentifier::new(0), child),
                BoundaryDiscoveryTransition::new(child, TriggerIdentifier::new(1), child),
            ],
        )
        .seal(&profile)
        .expect("runtime configuration");
        let mut reader = BoundaryReader::new("(outside [inside])", &profile);

        let tree = reader
            .discover_boundary_children(&configuration)
            .expect("single traversal");

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children().len(), 1);
        assert_eq!(
            reader.boundary_entries, 2,
            "the traversal enters the outer and inner boundaries once each"
        );
    }

    #[test]
    fn archived_noncanonical_context_sets_refuse_before_profile_use() {
        let profile = RawProfile::standard().seal().expect("standard profile");
        let root = BoundaryDiscoveryContextIdentifier::new(92);
        let parenthesis = TriggerIdentifier::new(0);
        let square = TriggerIdentifier::new(1);

        let reordered = BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::from_unchecked_for_test(vec![square, parenthesis]),
            )],
            vec![
                BoundaryDiscoveryTransition::new(root, parenthesis, root),
                BoundaryDiscoveryTransition::new(root, square, root),
            ],
        );
        let reordered = archive_round_trip(&reordered);
        assert!(matches!(
            reordered.seal(&profile),
            Err(BoundaryDiscoveryError::NoncanonicalContextTriggerSet { context }) if context == root
        ));

        let duplicated = BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::from_unchecked_for_test(vec![parenthesis, parenthesis]),
            )],
            vec![BoundaryDiscoveryTransition::new(root, parenthesis, root)],
        );
        let duplicated = archive_round_trip(&duplicated);
        assert!(matches!(
            duplicated.seal(&profile),
            Err(BoundaryDiscoveryError::NoncanonicalContextTriggerSet { context }) if context == root
        ));
    }

    #[test]
    fn archived_noncanonical_context_and_transition_order_refuse() {
        let profile = RawProfile::standard().seal().expect("standard profile");
        let root = BoundaryDiscoveryContextIdentifier::new(93);
        let child = BoundaryDiscoveryContextIdentifier::new(94);
        let parenthesis = TriggerIdentifier::new(0);
        let square = TriggerIdentifier::new(1);

        let reordered_contexts = BoundaryDiscoveryConfiguration {
            root,
            contexts: vec![
                BoundaryDiscoveryContext::new(child, TriggerSet::new(Vec::new())),
                BoundaryDiscoveryContext::new(root, TriggerSet::new(Vec::new())),
            ],
            transitions: Vec::new(),
        };
        let reordered_contexts = archive_round_trip(&reordered_contexts);
        assert!(matches!(
            reordered_contexts.seal(&profile),
            Err(BoundaryDiscoveryError::NoncanonicalContextOrder)
        ));

        let reordered_transitions = BoundaryDiscoveryConfiguration {
            root,
            contexts: vec![
                BoundaryDiscoveryContext::new(root, TriggerSet::new(vec![parenthesis, square])),
                BoundaryDiscoveryContext::new(child, TriggerSet::new(Vec::new())),
            ],
            transitions: vec![
                BoundaryDiscoveryTransition::new(root, square, child),
                BoundaryDiscoveryTransition::new(root, parenthesis, child),
            ],
        };
        let reordered_transitions = archive_round_trip(&reordered_transitions);
        assert!(matches!(
            reordered_transitions.seal(&profile),
            Err(BoundaryDiscoveryError::NoncanonicalTransitionOrder)
        ));
    }

    #[test]
    fn configured_discovery_refuses_a_wrong_profile_before_empty_source_succeeds() {
        let standard = RawProfile::standard().seal().expect("standard profile");
        let nomos = RawProfile::nomos_extended().seal().expect("nomos profile");
        let root = BoundaryDiscoveryContextIdentifier::new(96);
        let configuration = BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::new(Vec::new()),
            )],
            Vec::new(),
        )
        .seal(&standard)
        .expect("standard-bound configuration");
        let mut reader = BoundaryReader::new("", &nomos);

        assert!(matches!(
            reader.discover_boundary_children(&configuration),
            Err(BoundaryDiscoveryError::Profile(
                TokenProfileError::TriggerSetProfileMismatch
            ))
        ));
    }
}
