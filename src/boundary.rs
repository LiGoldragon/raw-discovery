//! Cursor-local boundary matching under one sealed active trigger set.
//!
//! This reader never produces a preliminary token stream. A caller supplies the
//! trigger set for its current expected structural position, consumes one
//! boundary event, and recursively supplies the next position's set.

use crate::profile::{
    CharacterClass, SealedBoundaryDiscoverySet, SealedTokenProfile, SealedTriggerSet,
    TokenProfileError, Trigger, TriggerIdentifier,
};

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
}

impl<'source, 'profile> BoundaryReader<'source, 'profile> {
    pub fn new(source: &'source str, profile: &'profile SealedTokenProfile) -> Self {
        Self {
            source,
            profile,
            byte_offset: 0,
            bound: SourceBound::whole(source),
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
        let Trigger::Boundary { opening, .. } = &self.profile.definition(identifier)?.trigger
        else {
            return Err(TokenProfileError::UnsupportedBoundaryDiscoveryTrigger(
                identifier,
            ));
        };

        // Expectation selected this group form before boundary discovery began,
        // so only its opening is active at this recursive state. Carriers and
        // nested boundaries in `active` become active after the opener, while
        // locating the matching close.
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
        let interior_start = self.byte_offset;
        let mut stack = vec![identifier];

        loop {
            if self.is_end() {
                return Err(TokenProfileError::UnclosedBoundary {
                    identifier,
                    byte_offset: opening.start,
                });
            }

            let Some(matched) = self.longest_match(active.active_triggers())? else {
                self.advance_character()
                    .expect("the bounded end condition was checked");
                continue;
            };

            match matched.kind {
                TriggerMatchKind::Boundary(BoundarySide::Opening) => {
                    stack.push(matched.identifier);
                    self.advance_to(matched.end);
                }
                TriggerMatchKind::Boundary(BoundarySide::Closing) => {
                    let expected = *stack
                        .last()
                        .expect("the enclosing boundary remains on the stack");
                    if matched.identifier != expected {
                        return Err(TokenProfileError::MismatchedBoundary {
                            expected,
                            found: matched.identifier,
                            byte_offset: matched.start,
                        });
                    }
                    stack.pop();
                    if stack.is_empty() {
                        let interior = SourceBound::between(interior_start, matched.start);
                        let result = DelimitedBoundary {
                            identifier,
                            opening: SourceBound::between(opening.start, opening.end),
                            interior,
                            closing: SourceBound::between(matched.start, matched.end),
                        };
                        self.advance_to(matched.end);
                        return Ok(result);
                    }
                    self.advance_to(matched.end);
                }
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
