//! # raw-discovery — the language-agnostic raw structure layer
//!
//! This crate discovers structure and **never classifies**. A [`Recognizer`]
//! reads source text into a tree of raw [`Block`]s — delimiter nesting,
//! right-associative dot-application, pipe text, bare atoms — and attaches no
//! meaning to any of it. "Declaration", "field", "name", "type" are words this
//! crate does not know; the expected types that give structure meaning live
//! entirely in the crates above it.
//!
//! That invariant is the crate's reason to exist as a boundary. A consumer that
//! wants only structure — a formatter, a linter, a tree-sitter bridge — links
//! raw-discovery alone and never drags in any Core language model.
//!
//! ## What is discovered
//!
//! - [`Block`] — the raw node: [`Delimited`](Block::Delimited),
//!   [`Application`](Block::Application), [`PipeText`], [`Atom`]. Application is
//!   a **designed-explicit** variant: nota expresses it structurally through a
//!   dotted head, and the accepted design promotes it to a first-class node so
//!   the raw layer names what nota leaves implicit.
//! - [`Delimiter`] — `( )`, `[ ]`, `{ }`.
//! - The dotted primitives: [`Atom::split_at_first_dot`] /
//!   [`Atom::split_text_at_first_dot`] (split) and [`Block::dotted_text`] (join).
//! - [`AtomCase`] — the capitalization classifier, exposed as **data** with no
//!   meaning attached.
//!
//! ## Profiles are sealed, versioned data
//!
//! [`TokenProfileData`] identifies generic boundary triggers. A structural
//! position activates a [`TriggerSet`], and [`BoundaryReader`] applies longest
//! complete match only within that sealed active set. Recognition is
//! boundary-first and recursive; it never constructs a preliminary token
//! stream. [`RawProfile`] and [`GlyphSet`] remain compatibility selectors for
//! the established NOTA profiles and seal into the same generic machinery.
//!
//! ## The raw-layer boundary
//!
//! [`RawLayer::Foreign`] is a typed target-language placeholder, not a parser
//! escape hatch. Target languages supply sealed profile and structural-form
//! data to the shared evaluator rather than installing another textual engine.

mod block;
mod boundary;
mod error;
mod profile;
mod recognizer;

pub use block::{Atom, AtomCase, Block, Delimiter, Document, PipeText};
pub use boundary::{
    BoundaryReader, BoundarySide, DelimitedBoundary, SourceBound, TriggerMatch, TriggerMatchKind,
};
pub use error::{FoundClose, RecognizeError, SourcePosition};
pub use profile::{
    CharacterClass, CharacterSet, GlyphSet, ProfileRevision, RawProfile,
    SealedBoundaryDiscoverySet, SealedTokenProfile, SealedTriggerSet, TokenProfileData,
    TokenProfileDomain, TokenProfileError, Trigger, TriggerDefinition, TriggerIdentifier,
    TriggerSet, TriggerTextRole,
};
pub use recognizer::{ForeignLanguage, ForeignRawLayer, RawLayer, Recognizer};
