//! Sealed lexical boundary data shared by every textual form.
//!
//! A profile does not tokenize a source horizontally. It defines the boundary
//! triggers that a sealed structural form may activate at one recursive
//! position. The boundary reader compares only that active set at the current
//! cursor. Longest complete match is universal machinery; authored precedence
//! is deliberately unrepresentable.

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion};
use thiserror::Error;

static WHITESPACE_CLASS: CharacterClass = CharacterClass::Whitespace;

/// A monotonic revision carried in a lexical profile's identity preimage.
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
    pub const fn new(revision: u32) -> Self {
        Self(revision)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Compatibility names for the two established NOTA-family profiles.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlyphSet {
    Standard,
    NomosExtended,
}

impl GlyphSet {
    pub const fn admits_dollar_sigil(self) -> bool {
        matches!(self, Self::NomosExtended)
    }
}

/// The established profile selector. Its sealed representation is produced by
/// [`RawProfile::seal`].
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawProfile {
    revision: ProfileRevision,
    glyphs: GlyphSet,
}

impl RawProfile {
    pub const fn new(revision: ProfileRevision, glyphs: GlyphSet) -> Self {
        Self { revision, glyphs }
    }

    pub const fn standard() -> Self {
        Self::new(ProfileRevision::new(1), GlyphSet::Standard)
    }

    pub const fn nomos_extended() -> Self {
        Self::new(ProfileRevision::new(1), GlyphSet::NomosExtended)
    }

    pub const fn revision(self) -> ProfileRevision {
        self.revision
    }

    pub const fn glyphs(self) -> GlyphSet {
        self.glyphs
    }

    /// Seal the compatibility profile as generic boundary data.
    pub fn seal(self) -> Result<SealedTokenProfile, TokenProfileError> {
        TokenProfileData::nota(self).seal()
    }
}

/// Compact identity of one trigger definition within a profile.
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
pub struct TriggerIdentifier(u16);

impl TriggerIdentifier {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

/// A character class used by cursor-local trigger matching.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum CharacterClass {
    AsciiDigit,
    AsciiAlphabetic,
    AsciiAlphanumeric,
    Whitespace,
    Characters(String),
}

impl CharacterClass {
    pub fn matches(&self, character: char) -> bool {
        match self {
            Self::AsciiDigit => character.is_ascii_digit(),
            Self::AsciiAlphabetic => character.is_ascii_alphabetic(),
            Self::AsciiAlphanumeric => character.is_ascii_alphanumeric(),
            Self::Whitespace => character.is_whitespace(),
            Self::Characters(characters) => characters.contains(character),
        }
    }

    fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Characters(left), Self::Characters(right)) => {
                left.chars().any(|character| right.contains(character))
            }
            (Self::Characters(characters), class) | (class, Self::Characters(characters)) => {
                characters.chars().any(|character| class.matches(character))
            }
            (Self::AsciiDigit, Self::AsciiDigit)
            | (Self::AsciiDigit, Self::AsciiAlphanumeric)
            | (Self::AsciiAlphanumeric, Self::AsciiDigit)
            | (Self::AsciiAlphabetic, Self::AsciiAlphabetic)
            | (Self::AsciiAlphabetic, Self::AsciiAlphanumeric)
            | (Self::AsciiAlphanumeric, Self::AsciiAlphabetic)
            | (Self::AsciiAlphanumeric, Self::AsciiAlphanumeric)
            | (Self::Whitespace, Self::Whitespace) => true,
            (Self::AsciiDigit, Self::AsciiAlphabetic)
            | (Self::AsciiAlphabetic, Self::AsciiDigit)
            | (Self::AsciiDigit, Self::Whitespace)
            | (Self::Whitespace, Self::AsciiDigit)
            | (Self::AsciiAlphabetic, Self::Whitespace)
            | (Self::Whitespace, Self::AsciiAlphabetic)
            | (Self::AsciiAlphanumeric, Self::Whitespace)
            | (Self::Whitespace, Self::AsciiAlphanumeric) => false,
        }
    }
}

/// One generic boundary trigger definition.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Trigger {
    Boundary {
        opening: String,
        closing: String,
    },
    Application {
        glyph: String,
    },
    Punctuation {
        glyph: String,
    },
    Carrier {
        opening: String,
        closing: String,
        escape: Option<String>,
    },
    Whitespace {
        canonical_spelling: String,
    },
    LineComment {
        opening: String,
    },
    LeadingCharacterClass {
        leading: CharacterClass,
        continuation: CharacterClass,
    },
}

impl Trigger {
    /// The canonical emitted spelling, when this generic trigger owns one.
    pub fn canonical_spelling(&self) -> Option<&str> {
        match self {
            Self::Whitespace { canonical_spelling } => Some(canonical_spelling),
            _ => None,
        }
    }

    fn exact_spellings(&self) -> Vec<&str> {
        match self {
            Self::Boundary { opening, closing } => vec![opening, closing],
            Self::Application { glyph } | Self::Punctuation { glyph } => vec![glyph],
            Self::Carrier { .. }
            | Self::Whitespace { .. }
            | Self::LineComment { .. }
            | Self::LeadingCharacterClass { .. } => Vec::new(),
        }
    }

    fn character_classes(&self) -> Option<(&CharacterClass, &CharacterClass)> {
        match self {
            Self::Whitespace { .. } => Some((&WHITESPACE_CLASS, &WHITESPACE_CLASS)),
            Self::LeadingCharacterClass {
                leading,
                continuation,
            } => Some((leading, continuation)),
            _ => None,
        }
    }

    fn validate(&self, identifier: TriggerIdentifier) -> Result<(), TokenProfileError> {
        let non_empty = |text: &str, role| {
            if text.is_empty() {
                Err(TokenProfileError::EmptyTrigger { identifier, role })
            } else {
                Ok(())
            }
        };
        match self {
            Self::Boundary { opening, closing } => {
                non_empty(opening, TriggerTextRole::BoundaryOpening)?;
                non_empty(closing, TriggerTextRole::BoundaryClosing)?;
                if opening == closing {
                    return Err(TokenProfileError::AmbiguousTriggerDefinition(identifier));
                }
                Ok(())
            }
            Self::Application { glyph } | Self::Punctuation { glyph } => {
                non_empty(glyph, TriggerTextRole::ExactGlyph)
            }
            Self::Carrier {
                opening,
                closing,
                escape,
            } => {
                non_empty(opening, TriggerTextRole::CarrierOpening)?;
                non_empty(closing, TriggerTextRole::CarrierClosing)?;
                if let Some(escape) = escape {
                    non_empty(escape, TriggerTextRole::CarrierEscape)?;
                }
                Ok(())
            }
            Self::LineComment { opening } => non_empty(opening, TriggerTextRole::TriviaOpening),
            Self::Whitespace { canonical_spelling } => {
                non_empty(canonical_spelling, TriggerTextRole::CanonicalSpelling)
            }
            Self::LeadingCharacterClass { .. } => Ok(()),
        }
    }
}

/// An identified trigger stored in profile data.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TriggerDefinition {
    pub identifier: TriggerIdentifier,
    pub trigger: Trigger,
}

/// An unordered set of triggers activated by one sealed structural position.
///
/// Vector order is not precedence. Seal validation and matching treat it as a
/// set; longest complete match is the only selection law.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TriggerSet {
    triggers: Vec<TriggerIdentifier>,
}

impl TriggerSet {
    pub fn new(mut triggers: Vec<TriggerIdentifier>) -> Self {
        triggers.sort_unstable();
        triggers.dedup();
        Self { triggers }
    }

    pub fn triggers(&self) -> &[TriggerIdentifier] {
        &self.triggers
    }
}

/// The complete identity-bearing lexical profile data.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TokenProfileData {
    revision: ProfileRevision,
    definitions: Vec<TriggerDefinition>,
    root_triggers: TriggerSet,
    forbidden_bare_characters: String,
}

impl TokenProfileData {
    pub fn new(
        revision: ProfileRevision,
        definitions: Vec<TriggerDefinition>,
        root_triggers: TriggerSet,
        forbidden_bare_characters: String,
    ) -> Self {
        Self {
            revision,
            definitions,
            root_triggers,
            forbidden_bare_characters,
        }
    }

    fn nota(profile: RawProfile) -> Self {
        let definitions = vec![
            TriggerDefinition {
                identifier: TriggerIdentifier::new(0),
                trigger: Trigger::Boundary {
                    opening: "(".to_owned(),
                    closing: ")".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: TriggerIdentifier::new(1),
                trigger: Trigger::Boundary {
                    opening: "[".to_owned(),
                    closing: "]".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: TriggerIdentifier::new(2),
                trigger: Trigger::Boundary {
                    opening: "{".to_owned(),
                    closing: "}".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: TriggerIdentifier::new(3),
                trigger: Trigger::Application {
                    glyph: ".".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: TriggerIdentifier::new(4),
                trigger: Trigger::Carrier {
                    opening: "(|".to_owned(),
                    closing: "|)".to_owned(),
                    escape: Some("\\".to_owned()),
                },
            },
            TriggerDefinition {
                identifier: TriggerIdentifier::new(5),
                trigger: Trigger::Whitespace {
                    canonical_spelling: " ".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: TriggerIdentifier::new(6),
                trigger: Trigger::LineComment {
                    opening: ";;".to_owned(),
                },
            },
        ];
        let forbidden_bare_characters = if profile.glyphs().admits_dollar_sigil() {
            "\"".to_owned()
        } else {
            "\"$".to_owned()
        };
        Self::new(
            profile.revision(),
            definitions,
            TriggerSet::new((0..=6).map(TriggerIdentifier::new).collect()),
            forbidden_bare_characters,
        )
    }

    pub fn seal(self) -> Result<SealedTokenProfile, TokenProfileError> {
        SealedTokenProfile::seal(self)
    }
}

/// Layout-2 contextual identity of sealed token-profile data.
pub struct TokenProfileDomain;

impl HashDomain for TokenProfileDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "raw-discovery 2026 recursive token profile",
            layout: LayoutVersion::new(2),
        }
    }
}

/// A token profile whose definitions and root context passed universal static
/// ambiguity checks.
#[derive(Clone, Debug)]
pub struct SealedTokenProfile {
    data: TokenProfileData,
    identity: ContentHash<TokenProfileDomain>,
}

impl SealedTokenProfile {
    fn seal(data: TokenProfileData) -> Result<Self, TokenProfileError> {
        for (index, definition) in data.definitions.iter().enumerate() {
            definition.trigger.validate(definition.identifier)?;
            if data.definitions[..index]
                .iter()
                .any(|prior| prior.identifier == definition.identifier)
            {
                return Err(TokenProfileError::DuplicateTriggerIdentifier(
                    definition.identifier,
                ));
            }
        }
        let identity = ContentHash::<TokenProfileDomain>::of_core(&data)?;
        let sealed = Self { data, identity };
        sealed.seal_trigger_set(sealed.data.root_triggers.clone())?;
        Ok(sealed)
    }

    pub fn standard() -> Self {
        RawProfile::standard()
            .seal()
            .expect("the built-in Standard profile is statically valid")
    }

    pub fn nomos_extended() -> Self {
        RawProfile::nomos_extended()
            .seal()
            .expect("the built-in Nomos profile is statically valid")
    }

    pub fn identity(&self) -> ContentHash<TokenProfileDomain> {
        self.identity
    }

    pub fn revision(&self) -> ProfileRevision {
        self.data.revision
    }

    pub fn root_trigger_set(&self) -> SealedTriggerSet {
        self.seal_trigger_set(self.data.root_triggers.clone())
            .expect("the root trigger set was validated when the profile sealed")
    }

    pub fn seal_trigger_set(
        &self,
        triggers: TriggerSet,
    ) -> Result<SealedTriggerSet, TokenProfileError> {
        for identifier in triggers.triggers() {
            self.definition(*identifier)?;
        }
        for (position, left) in triggers.triggers().iter().enumerate() {
            for right in &triggers.triggers()[position + 1..] {
                let left_trigger = &self.definition(*left)?.trigger;
                let right_trigger = &self.definition(*right)?.trigger;
                if Self::can_tie(left_trigger, right_trigger) {
                    return Err(TokenProfileError::AmbiguousTriggerSet {
                        first: *left,
                        second: *right,
                    });
                }
            }
        }
        Ok(SealedTriggerSet {
            profile_identity: self.identity,
            triggers,
        })
    }

    /// Seal the subset of triggers that may participate while discovering an
    /// enclosing boundary before its interior is interpreted.
    ///
    /// Boundary discovery is deliberately narrower than ordinary position
    /// recognition: only nested boundaries, opaque carriers, and trivia may
    /// affect where the enclosing close lies. Application, punctuation, and
    /// leading-character-class triggers belong to later recursive states and
    /// cannot influence this outside-in pass.
    pub fn seal_boundary_discovery_set(
        &self,
        triggers: TriggerSet,
    ) -> Result<SealedBoundaryDiscoverySet, TokenProfileError> {
        let triggers = self.seal_trigger_set(triggers)?;
        for identifier in triggers.triggers() {
            if !matches!(
                self.definition(*identifier)?.trigger,
                Trigger::Boundary { .. }
                    | Trigger::Carrier { .. }
                    | Trigger::Whitespace { .. }
                    | Trigger::LineComment { .. }
            ) {
                return Err(TokenProfileError::UnsupportedBoundaryDiscoveryTrigger(
                    *identifier,
                ));
            }
        }
        Ok(SealedBoundaryDiscoverySet { triggers })
    }

    pub fn definition(
        &self,
        identifier: TriggerIdentifier,
    ) -> Result<&TriggerDefinition, TokenProfileError> {
        self.data
            .definitions
            .iter()
            .find(|definition| definition.identifier == identifier)
            .ok_or(TokenProfileError::UnknownTriggerIdentifier(identifier))
    }

    pub fn bare_character_is_forbidden(&self, character: char) -> bool {
        self.data.forbidden_bare_characters.contains(character)
    }

    fn can_tie(left: &Trigger, right: &Trigger) -> bool {
        if left
            .exact_spellings()
            .iter()
            .any(|left| right.exact_spellings().contains(left))
        {
            return true;
        }
        if let (Some((left, _)), Some((right, _))) =
            (left.character_classes(), right.character_classes())
        {
            if left.overlaps(right) {
                return true;
            }
        }
        for exact in left.exact_spellings() {
            if right
                .character_classes()
                .is_some_and(|(leading, continuation)| {
                    Self::character_class_matches_entire(leading, continuation, exact)
                })
            {
                return true;
            }
        }
        for exact in right.exact_spellings() {
            if left
                .character_classes()
                .is_some_and(|(leading, continuation)| {
                    Self::character_class_matches_entire(leading, continuation, exact)
                })
            {
                return true;
            }
        }
        if let (Some((left_opening, _)), Some((right_opening, _))) =
            (Self::carrier_parts(left), Self::carrier_parts(right))
            && Self::prefixes_overlap(left_opening, right_opening)
        {
            return true;
        }
        if let (Some(left_opening), Some(right_opening)) =
            (Self::comment_opening(left), Self::comment_opening(right))
            && Self::prefixes_overlap(left_opening, right_opening)
        {
            return true;
        }
        if let (Some((carrier, _)), Some(comment)) =
            (Self::carrier_parts(left), Self::comment_opening(right))
            && Self::prefixes_overlap(carrier, comment)
        {
            return true;
        }
        if let (Some(comment), Some((carrier, _))) =
            (Self::comment_opening(left), Self::carrier_parts(right))
            && Self::prefixes_overlap(carrier, comment)
        {
            return true;
        }
        for (dynamic, other) in [(left, right), (right, left)] {
            if let Some((opening, closing)) = Self::carrier_parts(dynamic)
                && other
                    .exact_spellings()
                    .iter()
                    .any(|spelling| spelling.starts_with(opening) && spelling.ends_with(closing))
            {
                return true;
            }
            if let Some(opening) = Self::comment_opening(dynamic)
                && other
                    .exact_spellings()
                    .iter()
                    .any(|spelling| spelling.starts_with(opening))
            {
                return true;
            }
            if let Some(opening) = Self::dynamic_opening(dynamic)
                && other.character_classes().is_some_and(|(leading, _)| {
                    opening
                        .chars()
                        .next()
                        .is_some_and(|character| leading.matches(character))
                })
            {
                return true;
            }
        }
        false
    }

    fn character_class_matches_entire(
        leading: &CharacterClass,
        continuation: &CharacterClass,
        spelling: &str,
    ) -> bool {
        let mut characters = spelling.chars();
        characters
            .next()
            .is_some_and(|first| leading.matches(first))
            && characters.all(|character| continuation.matches(character))
    }

    fn prefixes_overlap(left: &str, right: &str) -> bool {
        left.starts_with(right) || right.starts_with(left)
    }

    fn carrier_parts(trigger: &Trigger) -> Option<(&str, &str)> {
        match trigger {
            Trigger::Carrier {
                opening, closing, ..
            } => Some((opening, closing)),
            _ => None,
        }
    }

    fn comment_opening(trigger: &Trigger) -> Option<&str> {
        match trigger {
            Trigger::LineComment { opening } => Some(opening),
            _ => None,
        }
    }

    fn dynamic_opening(trigger: &Trigger) -> Option<&str> {
        Self::carrier_parts(trigger)
            .map(|(opening, _)| opening)
            .or_else(|| Self::comment_opening(trigger))
    }
}

/// An active trigger set sealed against one exact token profile.
#[derive(Clone, Debug)]
pub struct SealedTriggerSet {
    profile_identity: ContentHash<TokenProfileDomain>,
    triggers: TriggerSet,
}

impl SealedTriggerSet {
    pub fn profile_identity(&self) -> ContentHash<TokenProfileDomain> {
        self.profile_identity
    }

    pub fn triggers(&self) -> &[TriggerIdentifier] {
        self.triggers.triggers()
    }
}

/// A sealed active set restricted to generic outside-in boundary discovery.
///
/// This derived value is not archived and does not add another identity plane;
/// its profile identity and universal ambiguity proof come from the wrapped
/// [`SealedTriggerSet`].
#[derive(Clone, Debug)]
pub struct SealedBoundaryDiscoverySet {
    triggers: SealedTriggerSet,
}

impl SealedBoundaryDiscoverySet {
    pub fn profile_identity(&self) -> ContentHash<TokenProfileDomain> {
        self.triggers.profile_identity()
    }

    pub fn triggers(&self) -> &[TriggerIdentifier] {
        self.triggers.triggers()
    }

    pub(crate) fn active_triggers(&self) -> &SealedTriggerSet {
        &self.triggers
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerTextRole {
    BoundaryOpening,
    BoundaryClosing,
    ExactGlyph,
    CarrierOpening,
    CarrierClosing,
    CarrierEscape,
    TriviaOpening,
    CanonicalSpelling,
}

/// A profile or active trigger set could not be sealed or executed.
#[derive(Debug, Clone, Eq, PartialEq, Error)]
pub enum TokenProfileError {
    #[error("trigger {identifier:?} has an empty {role:?}")]
    EmptyTrigger {
        identifier: TriggerIdentifier,
        role: TriggerTextRole,
    },
    #[error("trigger identifier {0:?} appears more than once")]
    DuplicateTriggerIdentifier(TriggerIdentifier),
    #[error("trigger {0:?} has two meanings with the same complete spelling")]
    AmbiguousTriggerDefinition(TriggerIdentifier),
    #[error("active trigger set refers to missing trigger {0:?}")]
    UnknownTriggerIdentifier(TriggerIdentifier),
    #[error("active trigger set permits an equal-length tie between {first:?} and {second:?}")]
    AmbiguousTriggerSet {
        first: TriggerIdentifier,
        second: TriggerIdentifier,
    },
    #[error("active trigger set belongs to a different token profile")]
    TriggerSetProfileMismatch,
    #[error("trigger {0:?} cannot participate in outside-in boundary discovery")]
    UnsupportedBoundaryDiscoveryTrigger(TriggerIdentifier),
    #[error("boundary trigger {identifier:?} is not active for boundary discovery")]
    InactiveBoundary { identifier: TriggerIdentifier },
    #[error("expected opening boundary {identifier:?} at byte {byte_offset}")]
    ExpectedBoundaryOpening {
        identifier: TriggerIdentifier,
        byte_offset: usize,
    },
    #[error("boundary {identifier:?} opened at byte {byte_offset} but no matching close was found")]
    UnclosedBoundary {
        identifier: TriggerIdentifier,
        byte_offset: usize,
    },
    #[error(
        "boundary {found:?} closed at byte {byte_offset} while boundary {expected:?} was active"
    )]
    MismatchedBoundary {
        expected: TriggerIdentifier,
        found: TriggerIdentifier,
        byte_offset: usize,
    },
    #[error("trigger {0:?} has no representation in the compatibility Block algebra")]
    UnsupportedCompatibilityTrigger(TriggerIdentifier),
    #[error("boundary trigger {0:?} has no compatibility Block delimiter")]
    UnsupportedCompatibilityBoundary(TriggerIdentifier),
    #[error("carrier {identifier:?} opened at byte {byte_offset} but never closed")]
    UnclosedCarrier {
        identifier: TriggerIdentifier,
        byte_offset: usize,
    },
    #[error("bare atom contains forbidden character `{character}` at byte {byte_offset}")]
    ForbiddenBareCharacter { character: char, byte_offset: usize },
    #[error("content identity failed: {0}")]
    Identity(String),
}

impl From<content_identity::ArchiveError> for TokenProfileError {
    fn from(error: content_identity::ArchiveError) -> Self {
        Self::Identity(error.to_string())
    }
}
