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
//! ## Profiles are versioned data
//!
//! A [`RawProfile`] pairs a [`GlyphSet`] with a [`ProfileRevision`]. The glyph
//! vocabulary a recognizer admits is versioned data, never a runtime guess:
//! [`GlyphSet::Standard`] rejects the `$` sigil that [`GlyphSet::NomosExtended`]
//! admits, and admitting a new glyph is a new profile revision.
//!
//! ## The raw-layer boundary
//!
//! [`RawLayer`] is the seam the whole textual family sits on: NOTA-family forms
//! share the [`Recognizer`]; a foreign language (Rust via `syn`, for instance)
//! supplies its own adapter through [`RawLayer::Foreign`], a typed placeholder
//! this crate names but does not implement.
//!
//! Consumption and integration of this crate will readapt to the forthcoming
//! release-train flow.

mod block;
mod error;
mod profile;
mod recognizer;

pub use block::{Atom, AtomCase, Block, Delimiter, Document, PipeText};
pub use error::{RecognizeError, SourcePosition};
pub use profile::{GlyphSet, ProfileRevision, RawProfile};
pub use recognizer::{ForeignLanguage, ForeignRawLayer, RawLayer, Recognizer};
