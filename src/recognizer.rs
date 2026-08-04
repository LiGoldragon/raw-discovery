//! The entry point: a [`Recognizer`] turns source text into a [`Document`] of
//! raw [`Block`]s under a sealed profile, and the [`RawLayer`] typed
//! target-language boundary.
//!
//! The compatibility recognizer discovers established NOTA delimiter nesting,
//! right-associative dot-application, bare angle applications, curly text, and
//! bare atoms without
//! classifying meaning. Its cursor consults a sealed trigger set and never
//! produces a preliminary token stream. New textual surfaces use the same
//! boundary-profile machinery through their sealed structural forms.

use crate::block::{ApplicationForm, Atom, Block, CurlyText, Delimiter};
use crate::boundary::{BoundaryReader, BoundarySide, TriggerMatch, TriggerMatchKind};
use crate::error::{FoundClose, RecognizeError, SourcePosition};
use crate::profile::{RawProfile, SealedTokenProfile, SealedTriggerSet, TokenProfileError};

/// The raw-structure entry point. Carries the versioned profile it recognizes
/// under; a new glyph is a new [`RawProfile`], never a runtime guess.
#[derive(Clone, Debug)]
pub struct Recognizer {
    profile: SealedTokenProfile,
}

impl PartialEq for Recognizer {
    fn eq(&self, other: &Self) -> bool {
        self.profile.identity() == other.profile.identity()
    }
}

impl Eq for Recognizer {}

impl Recognizer {
    pub fn new(profile: RawProfile) -> Self {
        Self::with_profile(
            profile
                .seal()
                .expect("the named compatibility profiles are statically valid"),
        )
    }

    pub fn with_profile(profile: SealedTokenProfile) -> Self {
        Self { profile }
    }

    /// A recognizer under the base NOTA
    /// [`Standard`](crate::GlyphSet::Standard) profile.
    pub fn standard() -> Self {
        Self::new(RawProfile::standard())
    }

    /// A recognizer under the Nomos
    /// [`NomosExtended`](crate::GlyphSet::NomosExtended) profile.
    pub fn nomos_extended() -> Self {
        Self::new(RawProfile::nomos_extended())
    }

    pub fn profile(&self) -> &SealedTokenProfile {
        &self.profile
    }

    /// Recognize the raw structure of `source` into an ordered document of
    /// top-level objects. Discovers structure; classifies nothing.
    pub fn recognize(&self, source: &str) -> Result<Document, RecognizeError> {
        let mut reading = SourceReading::new(source, &self.profile);
        let root_objects = reading.read_document()?;
        Ok(Document::from_root_objects(root_objects))
    }
}

pub use crate::block::Document;

/// The raw-layer boundary the whole textual family sits on.
/// [`Foreign`](RawLayer::Foreign) names a target language but does not install
/// a parser, printer, or second evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawLayer {
    /// A NOTA-family form: recognized by the shared [`Recognizer`].
    Recognizer(Recognizer),
    /// A foreign-language form: a typed placeholder here, implemented by the
    /// consuming crate's sealed textual data.
    Foreign(ForeignRawLayer),
}

impl RawLayer {
    /// A NOTA-family raw layer under the base
    /// [`Standard`](crate::GlyphSet::Standard) profile.
    pub fn standard() -> Self {
        Self::Recognizer(Recognizer::standard())
    }

    /// A NOTA-family raw layer under the Nomos profile.
    pub fn nomos_extended() -> Self {
        Self::Recognizer(Recognizer::nomos_extended())
    }

    /// The shared recognizer, when this is a NOTA-family layer.
    pub fn recognizer(&self) -> Option<&Recognizer> {
        match self {
            Self::Recognizer(recognizer) => Some(recognizer),
            Self::Foreign(_) => None,
        }
    }

    /// The foreign-language placeholder, when this is a foreign layer.
    pub fn foreign(&self) -> Option<&ForeignRawLayer> {
        match self {
            Self::Foreign(foreign) => Some(foreign),
            Self::Recognizer(_) => None,
        }
    }
}

/// The foreign-language arm of the raw-layer boundary — a **typed placeholder**.
/// It names which language the form targets. The target supplies sealed lexical
/// and structural-form data to the shared evaluator. raw-discovery holds no
/// target grammar and exposes no parser escape hatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignRawLayer {
    language: ForeignLanguage,
}

impl ForeignRawLayer {
    pub fn new(language: ForeignLanguage) -> Self {
        Self { language }
    }

    pub fn language(&self) -> &ForeignLanguage {
        &self.language
    }
}

/// The identity of a foreign source language behind a
/// [`ForeignRawLayer`] — `rust`, and whatever the family emits next.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignLanguage {
    name: String,
}

impl ForeignLanguage {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// The Rust target language.
    pub fn rust() -> Self {
        Self::new("rust")
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The reading cursor over one source text. Data-bearing (it owns the source
/// slice, the byte/line cursor, and the active trigger set), so the recognition
/// rules are methods on it rather than free functions.
struct SourceReading<'source> {
    source: &'source str,
    cursor: Cursor,
    profile: &'source SealedTokenProfile,
    active: SealedTriggerSet,
}

impl<'source> SourceReading<'source> {
    fn new(source: &'source str, profile: &'source SealedTokenProfile) -> Self {
        Self {
            source,
            cursor: Cursor::start(),
            profile,
            active: profile.root_trigger_set(),
        }
    }

    fn read_document(&mut self) -> Result<Vec<Block>, RecognizeError> {
        let mut root_objects = Vec::new();
        loop {
            self.skip_spacing()?;
            if self.peek().is_none() {
                return Ok(root_objects);
            }
            if let Some(matched) = self.matched_trigger()? {
                if matches!(
                    matched.kind(),
                    TriggerMatchKind::Boundary(BoundarySide::Closing)
                ) {
                    return Err(RecognizeError::UnexpectedClose {
                        found: self.found_close(),
                        position: self.cursor.position(),
                    });
                }
            }
            if let Some(character) = self.peek()
                && Delimiter::from_closing(character).is_some()
            {
                return Err(RecognizeError::UnexpectedClose {
                    found: FoundClose::Glyph(character),
                    position: self.cursor.position(),
                });
            }
            root_objects.push(self.read_object()?);
        }
    }

    /// Read one object: a primary object and, when a glued period or adjacent
    /// angle group follows, the application binding it to its payload. Dots are
    /// right-associative: `A.B.C = App(A, App(B, C))`.
    fn read_object(&mut self) -> Result<Block, RecognizeError> {
        let head = self.read_primary()?;
        let Some(application) = self.matched_trigger()? else {
            return Ok(head);
        };
        if matches!(application.kind(), TriggerMatchKind::Application) {
            let dot = self.cursor.position();
            self.consume_to(application.end());
            if self.at_angle_opening()? || !self.at_primary_start()? {
                return Err(RecognizeError::DanglingApplication { position: dot });
            }
            let payload = self.read_object()?;
            return Ok(Block::Application {
                form: ApplicationForm::Dot,
                head: Box::new(head),
                payload: Box::new(payload),
            });
        }
        if matches!(
            application.kind(),
            TriggerMatchKind::Boundary(BoundarySide::Opening)
        ) && self.compatibility_delimiter(&application)? == Delimiter::Angle
        {
            let start = self.cursor.position();
            self.consume_to(application.end());
            let payload = self.read_delimited(Delimiter::Angle, application.identifier(), start)?;
            return Ok(Block::Application {
                form: ApplicationForm::Angle,
                head: Box::new(head),
                payload: Box::new(payload),
            });
        }
        Ok(head)
    }

    /// Read a single primary object: a delimited block, a curly-text block, or a
    /// bare atom. A primary never consumes a trailing dot-application; that is
    /// [`read_object`](SourceReading::read_object)'s job. A leading period has
    /// no head and is rejected.
    fn read_primary(&mut self) -> Result<Block, RecognizeError> {
        match self.matched_trigger()? {
            Some(matched) if matches!(matched.kind(), TriggerMatchKind::Carrier) => {
                let text = matched.body().unwrap_or_default().to_owned();
                self.consume_to(matched.end());
                Ok(Block::CurlyText(CurlyText::new(text)))
            }
            Some(matched)
                if matches!(
                    matched.kind(),
                    TriggerMatchKind::Boundary(BoundarySide::Opening)
                ) =>
            {
                let delimiter = self.compatibility_delimiter(&matched)?;
                let start = self.cursor.position();
                self.consume_to(matched.end());
                self.read_delimited(delimiter, matched.identifier(), start)
            }
            Some(matched) if matches!(matched.kind(), TriggerMatchKind::Application) => {
                Err(RecognizeError::UnexpectedDot {
                    position: self.cursor.position(),
                })
            }
            Some(matched)
                if matches!(
                    matched.kind(),
                    TriggerMatchKind::Boundary(BoundarySide::Closing)
                ) =>
            {
                Err(RecognizeError::UnexpectedClose {
                    found: self.found_close(),
                    position: self.cursor.position(),
                })
            }
            Some(matched) if matched.is_trivia() => Err(RecognizeError::Profile {
                source: TokenProfileError::UnsupportedCompatibilityTrigger(matched.identifier()),
                position: self.cursor.position(),
            }),
            Some(_) | None => self.read_atom(),
        }
    }

    /// Whether the cursor sits on a character that can begin a primary object —
    /// the gate deciding whether a dot-application period has a glued payload.
    fn at_primary_start(&self) -> Result<bool, RecognizeError> {
        Ok(match self.matched_trigger()? {
            Some(matched) => matches!(
                matched.kind(),
                TriggerMatchKind::Boundary(BoundarySide::Opening)
                    | TriggerMatchKind::Carrier
                    | TriggerMatchKind::Punctuation
                    | TriggerMatchKind::LeadingCharacterClass
            ),
            None => match self.peek() {
                None => false,
                Some(character) if Delimiter::from_closing(character).is_some() => false,
                Some(_) => true,
            },
        })
    }

    fn read_delimited(
        &mut self,
        delimiter: Delimiter,
        identifier: crate::TriggerIdentifier,
        start: SourcePosition,
    ) -> Result<Block, RecognizeError> {
        let mut root_objects = Vec::new();
        loop {
            self.skip_spacing()?;
            let Some(character) = self.peek() else {
                return Err(RecognizeError::UnclosedDelimiter {
                    delimiter,
                    position: start,
                });
            };
            if let Some(matched) = self.matched_trigger()?
                && let TriggerMatchKind::Boundary(BoundarySide::Closing) = matched.kind()
            {
                if matched.identifier() == identifier {
                    self.consume_to(matched.end());
                    return Ok(Block::Delimited {
                        delimiter,
                        root_objects,
                    });
                }
                return Err(RecognizeError::UnexpectedClose {
                    found: FoundClose::Glyph(character),
                    position: self.cursor.position(),
                });
            }
            root_objects.push(self.read_object()?);
        }
    }

    fn read_atom(&mut self) -> Result<Block, RecognizeError> {
        let start = self.cursor.position();
        let mut boundary = BoundaryReader::new(self.source, self.profile);
        boundary.advance_to(start.byte_offset);
        let text = boundary
            .read_bare(&self.active)
            .map_err(|error| self.recognize_profile_error(error))?;
        let Some(text) = text else {
            let matched = boundary
                .longest_match(&self.active)
                .map_err(|error| self.recognize_profile_error(error))?
                .expect("read_atom is called only at a non-empty primary position");
            return Err(RecognizeError::Profile {
                source: TokenProfileError::UnsupportedCompatibilityTrigger(matched.identifier()),
                position: start,
            });
        };
        self.consume_to(boundary.byte_offset());
        Ok(Block::Atom(Atom::new(text)))
    }

    fn at_angle_opening(&self) -> Result<bool, RecognizeError> {
        Ok(self.matched_trigger()?.is_some_and(|matched| {
            matches!(
                matched.kind(),
                TriggerMatchKind::Boundary(BoundarySide::Opening)
            ) && self
                .compatibility_delimiter(&matched)
                .is_ok_and(|delimiter| delimiter == Delimiter::Angle)
        }))
    }

    fn found_close(&self) -> FoundClose {
        self.peek()
            .map(FoundClose::Glyph)
            .unwrap_or(FoundClose::EndOfInput)
    }

    fn skip_spacing(&mut self) -> Result<(), RecognizeError> {
        loop {
            let Some(matched) = self.matched_trigger()? else {
                return Ok(());
            };
            if !matched.is_trivia() {
                return Ok(());
            }
            self.consume_to(matched.end());
        }
    }

    fn matched_trigger(&self) -> Result<Option<TriggerMatch>, RecognizeError> {
        let mut boundary = BoundaryReader::new(self.source, self.profile);
        boundary.advance_to(self.cursor.byte_offset);
        boundary
            .longest_match(&self.active)
            .map_err(|error| self.recognize_profile_error(error))
    }

    fn recognize_profile_error(&self, error: TokenProfileError) -> RecognizeError {
        match error {
            TokenProfileError::UnclosedCarrier { .. }
            | TokenProfileError::UnclosedCurlyText { .. } => RecognizeError::UnclosedCurlyText {
                position: self.cursor.position(),
            },
            TokenProfileError::InvalidCurlyTextEscape { .. } => {
                RecognizeError::InvalidCurlyTextEscape {
                    position: self.cursor.position(),
                }
            }
            TokenProfileError::ForbiddenBareCharacter { character, .. } => {
                RecognizeError::UnsupportedGlyph {
                    glyph: character,
                    position: self.cursor.position(),
                }
            }
            source => RecognizeError::Profile {
                source,
                position: self.cursor.position(),
            },
        }
    }

    fn compatibility_delimiter(&self, matched: &TriggerMatch) -> Result<Delimiter, RecognizeError> {
        let text = self.source_between(matched.start(), matched.end());
        text.chars()
            .next()
            .and_then(Delimiter::from_opening)
            .filter(|_| text.chars().count() == 1)
            .ok_or_else(|| RecognizeError::Profile {
                source: TokenProfileError::UnsupportedCompatibilityBoundary(matched.identifier()),
                position: self.cursor.position(),
            })
    }

    fn source_between(&self, start: usize, end: usize) -> &str {
        &self.source[start..end]
    }

    fn consume_to(&mut self, end: usize) {
        while self.cursor.byte_offset < end {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.cursor.byte_offset..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.cursor.advance(character);
        Some(character)
    }
}

/// A byte-and-line cursor into the source, advanced character by character.
#[derive(Clone, Copy, Debug)]
struct Cursor {
    byte_offset: usize,
    line: usize,
    column: usize,
}

impl Cursor {
    fn start() -> Self {
        Self {
            byte_offset: 0,
            line: 1,
            column: 1,
        }
    }

    fn position(self) -> SourcePosition {
        SourcePosition {
            byte_offset: self.byte_offset,
            line: self.line,
            column: self.column,
        }
    }

    fn advance(&mut self, character: char) {
        self.byte_offset += character.len_utf8();
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
    }
}
