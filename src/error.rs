//! Typed recognition errors, each carrying the source position that a reader
//! needs to point at the offending glyph.
//!
//! Positions live only on the error type. The recognized [`Block`](crate::Block)
//! tree is deliberately span-free so it stays portable, content-addressable
//! structure — byte offsets into one particular source string are not portable
//! identity, so they are recovered here for diagnostics rather than carried on
//! the structure.

use thiserror::Error;

use crate::block::Delimiter;

/// A byte-and-line position in the recognized source. Carried by
/// [`RecognizeError`] alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub byte_offset: usize,
    pub line: usize,
    pub column: usize,
}

/// Every way raw recognition can reject a source. The recognizer discovers
/// structure and never classifies meaning, so every variant here is a
/// *structural* fault — an unbalanced delimiter, a dangling dot-application, or
/// a glyph the active profile's [`GlyphSet`](crate::GlyphSet) does not admit —
/// never a "wrong type" or "unknown name".
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum RecognizeError {
    /// A closing delimiter appeared with no matching opener at this position.
    #[error("unexpected closing delimiter `{found}` at {}:{}", .position.line, .position.column)]
    UnexpectedClose {
        found: char,
        position: SourcePosition,
    },

    /// A delimiter opened here was never closed before the end of input.
    #[error("unclosed `{}` opened at {}:{}", .delimiter.opening_text(), .position.line, .position.column)]
    UnclosedDelimiter {
        delimiter: Delimiter,
        position: SourcePosition,
    },

    /// A `(|` pipe-text block opened here was never closed by `|)`.
    #[error("unclosed `(|` pipe text opened at {}:{}", .position.line, .position.column)]
    UnclosedPipeText { position: SourcePosition },

    /// A period appeared at object position with no head to its left. A dot
    /// binds a head to a following payload, so it can never start an object.
    #[error("unexpected `.` with no head object at {}:{}", .position.line, .position.column)]
    UnexpectedDot { position: SourcePosition },

    /// A dot-application period is not immediately followed by a glued payload:
    /// whitespace, a comment, a closing delimiter, or the end of input followed
    /// the period.
    #[error(
        "dot-application `.` at {}:{} is not followed by a glued payload object",
        .position.line,
        .position.column
    )]
    DanglingApplication { position: SourcePosition },

    /// An atom bears a glyph that the active profile's glyph set does not
    /// admit — the `$` sigil under the [`Standard`](crate::GlyphSet::Standard)
    /// set, for instance. A new glyph is a new profile revision, never a runtime
    /// guess.
    #[error(
        "glyph `{glyph}` is not admitted by the active profile glyph set at {}:{}",
        .position.line,
        .position.column
    )]
    UnsupportedGlyph {
        glyph: char,
        position: SourcePosition,
    },
}
