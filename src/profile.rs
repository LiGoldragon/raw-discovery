//! The versioned raw profile: which glyphs the recognizer is allowed to see.
//!
//! Lexical variation is **versioned data**, never a runtime guess. A profile
//! names a [`GlyphSet`] and a [`ProfileRevision`]; admitting a new glyph (the
//! `$` sigil, a future operator) is a new revision of this data, so two readers
//! that disagree about the glyph vocabulary disagree by *identity*, spot-checkably,
//! rather than by silent heuristic drift.

/// A monotonic revision number for a [`RawProfile`]. Distinct revisions may
/// admit distinct glyph vocabularies; the revision is the identity a consumer
/// pins.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
)]
pub struct ProfileRevision(u32);

impl ProfileRevision {
    pub fn new(revision: u32) -> Self {
        Self(revision)
    }

    pub fn value(self) -> u32 {
        self.0
    }
}

/// The glyph vocabulary a profile admits. `Standard` is the base NOTA set —
/// `.`, `( )`, `[ ]`, `{ }`, `(| |)`, `;;`. `NomosExtended` additionally admits
/// the `$` sigil that Nomos macros carry.
///
/// The set is a closed enum, not an open configuration, so a glyph is either in
/// the family's vocabulary or it is not; there is no third "maybe, if it looks
/// like one" state.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlyphSet {
    Standard,
    NomosExtended,
}

impl GlyphSet {
    /// Whether an atom may carry the `$` sigil under this set. `Standard`
    /// forbids it; `NomosExtended` admits it. A recognizer under `Standard` that
    /// meets a `$` raises
    /// [`UnsupportedGlyph`](crate::RecognizeError::UnsupportedGlyph) rather than
    /// guessing.
    pub fn admits_dollar_sigil(self) -> bool {
        matches!(self, Self::NomosExtended)
    }
}

/// A versioned raw profile: the glyph vocabulary plus its revision. This is the
/// data the [`Recognizer`](crate::Recognizer) is parameterized by.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawProfile {
    revision: ProfileRevision,
    glyphs: GlyphSet,
}

impl RawProfile {
    pub fn new(revision: ProfileRevision, glyphs: GlyphSet) -> Self {
        Self { revision, glyphs }
    }

    /// The base NOTA profile — the `Standard` glyph set at revision 1.
    pub fn standard() -> Self {
        Self::new(ProfileRevision::new(1), GlyphSet::Standard)
    }

    /// The Nomos profile — the `NomosExtended` glyph set (with `$`) at
    /// revision 1.
    pub fn nomos_extended() -> Self {
        Self::new(ProfileRevision::new(1), GlyphSet::NomosExtended)
    }

    pub fn revision(self) -> ProfileRevision {
        self.revision
    }

    pub fn glyphs(self) -> GlyphSet {
        self.glyphs
    }
}
