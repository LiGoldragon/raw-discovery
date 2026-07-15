//! The entry point: a [`Recognizer`] turns source text into a [`Document`] of
//! raw [`Block`]s under a versioned [`RawProfile`], and the [`RawLayer`]
//! boundary that lets NOTA-family forms share the recognizer while foreign
//! languages supply their own adapter.
//!
//! The recognizer is a right-associative recursive-descent reader lifted
//! verbatim from nota's psyche-blessed next-gen parser. It discovers delimiter
//! nesting, right-associative dot-application, pipe text, and bare atoms — and
//! never classifies any of them. It is a hand-written reader by deliberate
//! design: it *is* the blessed ground-truth grammar being lifted into a
//! nota-independent boundary crate, so neither delegating to the nota codec (a
//! forbidden dependency) nor a parser-combinator rewrite (which would abandon
//! the verbatim lift of green, blessed code) applies here.

use crate::block::{Atom, Block, Delimiter, PipeText};
use crate::error::{RecognizeError, SourcePosition};
use crate::profile::{GlyphSet, RawProfile};

/// The raw-structure entry point. Carries the versioned profile it recognizes
/// under; a new glyph is a new [`RawProfile`], never a runtime guess.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Recognizer {
    profile: RawProfile,
}

impl Recognizer {
    pub fn new(profile: RawProfile) -> Self {
        Self { profile }
    }

    /// A recognizer under the base NOTA [`Standard`](GlyphSet::Standard) profile.
    pub fn standard() -> Self {
        Self::new(RawProfile::standard())
    }

    /// A recognizer under the Nomos [`NomosExtended`](GlyphSet::NomosExtended)
    /// profile (the `$` sigil admitted).
    pub fn nomos_extended() -> Self {
        Self::new(RawProfile::nomos_extended())
    }

    pub fn profile(&self) -> RawProfile {
        self.profile
    }

    /// Recognize the raw structure of `source` into an ordered document of
    /// top-level objects. Discovers structure; classifies nothing.
    pub fn recognize(&self, source: &str) -> Result<Document, RecognizeError> {
        let mut reading = SourceReading::new(source, self.profile.glyphs());
        let root_objects = reading.read_document()?;
        Ok(Document::from_root_objects(root_objects))
    }
}

pub use crate::block::Document;

/// The raw-layer boundary the whole textual family sits on. NOTA-family forms —
/// schema, Nomos, logos — share the [`Recognizer`]; a foreign language supplies
/// its own adapter through the [`Foreign`](RawLayer::Foreign) arm.
///
/// This is the principled seam: the recognizer discovers NOTA structure, and a
/// language whose surface is not NOTA (Rust through `syn`, say) is recognized by
/// a consumer-supplied adapter rather than by pretending its grammar is NOTA.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawLayer {
    /// A NOTA-family form: recognized by the shared [`Recognizer`].
    Recognizer(Recognizer),
    /// A foreign-language form: a typed placeholder here, implemented by the
    /// consuming crate's own parse/unparse adapter.
    Foreign(ForeignRawLayer),
}

impl RawLayer {
    /// A NOTA-family raw layer under the base [`Standard`](GlyphSet::Standard)
    /// profile.
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
/// It names which language the form targets; the actual parse-on-decode and
/// unparse-on-encode adapter (Rust's `syn` + `prettyplease`, for instance) lives
/// in the consuming crate, not here. raw-discovery holds no foreign grammar and
/// pulls in no foreign parser.
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

    /// The Rust foreign language — the reference foreign adapter the family
    /// draws (recognized by `syn`, emitted by `prettyplease`, in the consumer).
    pub fn rust() -> Self {
        Self::new("rust")
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The reading cursor over one source text. Data-bearing (it owns the source
/// slice, the byte/line cursor, and the active glyph set), so the recognition
/// rules are methods on it rather than free functions.
struct SourceReading<'source> {
    source: &'source str,
    cursor: Cursor,
    glyphs: GlyphSet,
}

impl<'source> SourceReading<'source> {
    fn new(source: &'source str, glyphs: GlyphSet) -> Self {
        Self {
            source,
            cursor: Cursor::start(),
            glyphs,
        }
    }

    fn read_document(&mut self) -> Result<Vec<Block>, RecognizeError> {
        let mut root_objects = Vec::new();
        loop {
            self.skip_spacing();
            let Some(character) = self.peek() else {
                return Ok(root_objects);
            };
            if Delimiter::from_closing(character).is_some() {
                return Err(RecognizeError::UnexpectedClose {
                    found: character,
                    position: self.cursor.position(),
                });
            }
            root_objects.push(self.read_object()?);
        }
    }

    /// Read one object: a primary object and, when a glued period follows, the
    /// dot-application binding it to the remainder. Right-associative:
    /// `A.B.C = App(A, App(B, C))`. A period binds only when glued to both its
    /// head and its payload.
    fn read_object(&mut self) -> Result<Block, RecognizeError> {
        let head = self.read_primary()?;
        if self.peek() != Some('.') {
            return Ok(head);
        }
        let dot = self.cursor.position();
        self.bump();
        if !self.at_primary_start() {
            return Err(RecognizeError::DanglingApplication { position: dot });
        }
        let payload = self.read_object()?;
        Ok(Block::Application {
            head: Box::new(head),
            payload: Box::new(payload),
        })
    }

    /// Read a single primary object: a delimited block, a pipe-text block, or a
    /// bare atom. A primary never consumes a trailing dot-application; that is
    /// [`read_object`](SourceReading::read_object)'s job. A leading period has
    /// no head and is rejected.
    fn read_primary(&mut self) -> Result<Block, RecognizeError> {
        match self.peek() {
            Some('(') if self.peek_next() == Some('|') => self.read_pipe_text(),
            Some('(') => self.read_delimited(Delimiter::Parenthesis),
            Some('[') => self.read_delimited(Delimiter::SquareBracket),
            Some('{') => self.read_delimited(Delimiter::Brace),
            Some('.') => Err(RecognizeError::UnexpectedDot {
                position: self.cursor.position(),
            }),
            // A misplaced pipe-close (`|)`) at object position would make
            // `read_atom` return a zero-width atom without advancing, so the
            // enclosing loop would spin forever, growing the vector until memory
            // is exhausted. Reject it so the reader always makes progress on
            // malformed input.
            Some('|') if self.at_pipe_close() => Err(RecognizeError::UnexpectedClose {
                found: self.peek_next().unwrap_or('|'),
                position: self.cursor.position(),
            }),
            Some(_) | None => self.read_atom(),
        }
    }

    /// Whether the cursor sits on a character that can begin a primary object —
    /// the gate deciding whether a dot-application period has a glued payload.
    fn at_primary_start(&self) -> bool {
        match self.peek() {
            None => false,
            Some('.') => false,
            Some(character) if character.is_whitespace() => false,
            Some(character) if Delimiter::from_closing(character).is_some() => false,
            Some(_) if self.at_comment_start() => false,
            Some(_) if self.at_pipe_close() => false,
            Some(_) => true,
        }
    }

    fn read_delimited(&mut self, delimiter: Delimiter) -> Result<Block, RecognizeError> {
        let start = self.cursor.position();
        self.bump();
        let mut root_objects = Vec::new();
        loop {
            self.skip_spacing();
            let Some(character) = self.peek() else {
                return Err(RecognizeError::UnclosedDelimiter {
                    delimiter,
                    position: start,
                });
            };
            if character == delimiter.closing_character() {
                self.bump();
                return Ok(Block::Delimited {
                    delimiter,
                    root_objects,
                });
            }
            if Delimiter::from_closing(character).is_some() {
                return Err(RecognizeError::UnexpectedClose {
                    found: character,
                    position: self.cursor.position(),
                });
            }
            root_objects.push(self.read_object()?);
        }
    }

    fn read_pipe_text(&mut self) -> Result<Block, RecognizeError> {
        let start = self.cursor.position();
        self.bump();
        self.bump();
        let mut text = String::new();
        while let Some(character) = self.peek() {
            if character == '\\' {
                self.bump();
                if let Some(escaped) = self.peek() {
                    text.push(escaped);
                    self.bump();
                } else {
                    text.push('\\');
                }
            } else if character == '|' && self.peek_next() == Some(')') {
                self.bump();
                self.bump();
                return Ok(Block::PipeText(PipeText::new(text)));
            } else {
                text.push(character);
                self.bump();
            }
        }
        Err(RecognizeError::UnclosedPipeText { position: start })
    }

    fn read_atom(&mut self) -> Result<Block, RecognizeError> {
        let start = self.cursor.position();
        while let Some(character) = self.peek() {
            if character.is_whitespace()
                || character == '.'
                || Delimiter::from_opening(character).is_some()
                || Delimiter::from_closing(character).is_some()
                || self.at_comment_start()
                || self.at_pipe_close()
            {
                break;
            }
            self.bump();
        }
        let end = self.cursor.position();
        let text = self.source[start.byte_offset..end.byte_offset].to_owned();
        self.check_atom_glyphs(&text, start)?;
        Ok(Block::Atom(Atom::new(text)))
    }

    /// Reject glyphs the active profile does not admit. Today the `$` sigil
    /// under the [`Standard`](GlyphSet::Standard) set is the only such glyph;
    /// under [`NomosExtended`](GlyphSet::NomosExtended) it passes. This is where
    /// the profile is versioned data rather than a runtime heuristic.
    fn check_atom_glyphs(&self, text: &str, start: SourcePosition) -> Result<(), RecognizeError> {
        if !self.glyphs.admits_dollar_sigil() && text.contains('$') {
            return Err(RecognizeError::UnsupportedGlyph {
                glyph: '$',
                position: start,
            });
        }
        Ok(())
    }

    fn at_pipe_close(&self) -> bool {
        self.peek() == Some('|') && self.peek_next() == Some(')')
    }

    fn at_comment_start(&self) -> bool {
        self.peek() == Some(';') && self.peek_next() == Some(';')
    }

    fn skip_spacing(&mut self) {
        loop {
            match self.peek() {
                Some(character) if character.is_whitespace() => {
                    self.bump();
                }
                Some(';') if self.peek_next() == Some(';') => {
                    while let Some(character) = self.peek() {
                        self.bump();
                        if character == '\n' {
                            break;
                        }
                    }
                }
                _ => return,
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.cursor.byte_offset..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let mut characters = self.source[self.cursor.byte_offset..].chars();
        characters.next()?;
        characters.next()
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
