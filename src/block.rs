//! The raw structural model: the [`Block`] tree the recognizer discovers, and
//! the primitives that read structure off it.
//!
//! Every type here discovers *structure* and attaches no meaning. An
//! [`AtomCase`] is exposed as data — a reader may ask whether an atom reads as
//! PascalCase — but the crate stamps no "object" / "name" / "type" judgement on
//! it. That is the boundary this crate exists to hold.
//!
//! The tree is span-free. The recognizer tracks source positions only to build
//! [`RecognizeError`](crate::RecognizeError) diagnostics; the discovered
//! structure carries no byte offsets, so it is portable, content-addressable
//! data that round-trips through rkyv.

/// A recognized document: the ordered top-level objects of one source text.
///
/// A source is a *sequence* of root objects, so recognition yields this rather
/// than a single block. (The up-close design sketch wrote the recognizer as
/// `-> Block`; a faithful lift of nota's `Document { root_objects }` keeps the
/// sequence.)
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(
        __S: rkyv::ser::Writer + rkyv::ser::Allocator,
        __S::Error: rkyv::rancor::Source,
    ),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(
        __C: rkyv::validation::ArchiveContext,
        __C::Error: rkyv::rancor::Source,
    )),
)]
pub struct Document {
    #[rkyv(omit_bounds)]
    root_objects: Vec<Block>,
}

impl Document {
    /// The recognized top-level objects, in source order.
    pub fn root_objects(&self) -> &[Block] {
        &self.root_objects
    }

    /// The root object at `index`, or `None` past the end.
    pub fn root_object_at(&self, index: usize) -> Option<&Block> {
        self.root_objects.get(index)
    }

    /// How many top-level objects the source held.
    pub fn holds_root_objects(&self) -> usize {
        self.root_objects.len()
    }

    pub(crate) fn from_root_objects(root_objects: Vec<Block>) -> Self {
        Self { root_objects }
    }
}

/// The raw structural node. Four discovered shapes, no meaning attached.
///
/// [`Application`](Block::Application) is a **designed-explicit** variant. In
/// nota's current parser, application is expressed structurally — a dotted head
/// glued to its argument group — and never named as a variant. The accepted
/// design promotes it to a first-class node so the raw layer names what nota
/// leaves implicit; the right-associative binding rule (`A.B.C = App(A, App(B, C))`)
/// is unchanged and psyche-blessed.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    serialize_bounds(
        __S: rkyv::ser::Writer + rkyv::ser::Allocator,
        __S::Error: rkyv::rancor::Source,
    ),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(
        __C: rkyv::validation::ArchiveContext,
        __C::Error: rkyv::rancor::Source,
    )),
)]
pub enum Block {
    /// A delimiter around an ordered sequence of objects: `( … )`, `[ … ]`,
    /// `{ … }`.
    Delimited {
        delimiter: Delimiter,
        #[rkyv(omit_bounds)]
        root_objects: Vec<Block>,
    },
    /// A dot-application: a head object bound to the payload glued after a
    /// period, as one node. Right-associative: `A.B.C` is `App(A, App(B, C))`.
    Application {
        #[rkyv(omit_bounds)]
        head: Box<Block>,
        #[rkyv(omit_bounds)]
        payload: Box<Block>,
    },
    /// A `(| … |)` multiline-string carrier: literal text a bare atom or a
    /// delimiter cannot hold.
    PipeText(PipeText),
    /// A bare atom: an unbroken run of symbol characters.
    Atom(Atom),
}

impl Block {
    /// The head and payload of a dot-application, or `None` for any other
    /// block. The head is the object left of the period (itself an application
    /// for a dotted chain); the payload is the single object right of it.
    pub fn as_application(&self) -> Option<(&Block, &Block)> {
        match self {
            Self::Application { head, payload } => Some((head, payload)),
            Self::Delimited { .. } | Self::PipeText(_) | Self::Atom(_) => None,
        }
    }

    /// The head object of a dot-application — the object left of the period.
    pub fn application_head(&self) -> Option<&Block> {
        self.as_application().map(|(head, _)| head)
    }

    /// The payload object of a dot-application — the object right of the period.
    pub fn application_payload(&self) -> Option<&Block> {
        self.as_application().map(|(_, payload)| payload)
    }

    pub fn is_parenthesis(&self) -> bool {
        self.is_delimited_with(Delimiter::Parenthesis)
    }

    pub fn is_square_bracket(&self) -> bool {
        self.is_delimited_with(Delimiter::SquareBracket)
    }

    pub fn is_brace(&self) -> bool {
        self.is_delimited_with(Delimiter::Brace)
    }

    pub fn is_pipe_text(&self) -> bool {
        matches!(self, Self::PipeText(_))
    }

    pub fn is_atom(&self) -> bool {
        matches!(self, Self::Atom(_))
    }

    pub fn is_application(&self) -> bool {
        matches!(self, Self::Application { .. })
    }

    pub fn is_delimited_with(&self, delimiter: Delimiter) -> bool {
        matches!(self, Self::Delimited { delimiter: found, .. } if *found == delimiter)
    }

    /// The children of a delimited block of the requested delimiter kind, or
    /// `None` for any other block.
    pub fn as_delimited(&self, delimiter: Delimiter) -> Option<&[Block]> {
        match self {
            Self::Delimited {
                delimiter: found,
                root_objects,
            } if *found == delimiter => Some(root_objects),
            Self::Delimited { .. }
            | Self::Application { .. }
            | Self::PipeText(_)
            | Self::Atom(_) => None,
        }
    }

    /// How many child objects a delimited block holds (zero for every other
    /// shape — an application binds exactly two objects but exposes them through
    /// [`as_application`](Block::as_application), not as delimited children).
    pub fn holds_root_objects(&self) -> usize {
        match self {
            Self::Delimited { root_objects, .. } => root_objects.len(),
            Self::Application { .. } | Self::PipeText(_) | Self::Atom(_) => 0,
        }
    }

    pub fn holds_single_root_object(&self) -> bool {
        self.holds_root_objects() == 1
    }

    pub fn holds_two_root_objects(&self) -> bool {
        self.holds_root_objects() == 2
    }

    pub fn root_object_at(&self, index: usize) -> Option<&Block> {
        match self {
            Self::Delimited { root_objects, .. } => root_objects.get(index),
            Self::Application { .. } | Self::PipeText(_) | Self::Atom(_) => None,
        }
    }

    pub fn root_objects(&self) -> &[Block] {
        match self {
            Self::Delimited { root_objects, .. } => root_objects,
            Self::Application { .. } | Self::PipeText(_) | Self::Atom(_) => &[],
        }
    }

    /// The atom this block is, or `None` for a delimited, application, or
    /// pipe-text block.
    pub fn atom(&self) -> Option<&Atom> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::Delimited { .. } | Self::Application { .. } | Self::PipeText(_) => None,
        }
    }

    pub fn qualifies_as_symbol(&self) -> bool {
        self.atom().is_some_and(Atom::qualifies_as_symbol)
    }

    pub fn qualifies_as_pascal_case_symbol(&self) -> bool {
        self.atom()
            .is_some_and(Atom::qualifies_as_pascal_case_symbol)
    }

    pub fn qualifies_as_camel_case_symbol(&self) -> bool {
        self.atom()
            .is_some_and(Atom::qualifies_as_camel_case_symbol)
    }

    pub fn qualifies_as_kebab_case_symbol(&self) -> bool {
        self.atom()
            .is_some_and(Atom::qualifies_as_kebab_case_symbol)
    }

    /// The flat text of an atom or pipe-text block, or `None` for a delimited or
    /// application block. This exposes the atom's characters without classifying
    /// them.
    pub fn demote_to_string(&self) -> Option<&str> {
        match self {
            Self::Atom(atom) => Some(atom.text()),
            Self::PipeText(pipe_text) => Some(pipe_text.text()),
            Self::Delimited { .. } | Self::Application { .. } => None,
        }
    }

    /// The flat dotted text of an atom or a dotted chain of atoms, joining the
    /// segments with periods: `Atom("42")` → `"42"`, `App(rustfmt, skip)` →
    /// `"rustfmt.skip"`, `App(-122, 3)` → `"-122.3"`. `None` when any segment is
    /// a delimited or pipe-text block, since those carry no flat text form.
    ///
    /// This is the join side of the dotted-primitive pair (the split side is
    /// [`Atom::split_at_first_dot`]). A consumer that *expects* a dotted literal
    /// at a position — a qualified path, a float whose fractional period is a
    /// structural dot — reconstructs it here; the raw layer itself never decided
    /// the atom was a path or a number.
    pub fn dotted_text(&self) -> Option<String> {
        match self {
            Self::Atom(atom) => Some(atom.text().to_owned()),
            Self::Application { head, payload } => {
                let head = head.dotted_text()?;
                let payload = payload.dotted_text()?;
                Some(format!("{head}.{payload}"))
            }
            Self::Delimited { .. } | Self::PipeText(_) => None,
        }
    }
}

/// The three reader-serving delimiters. New glyphs require an explicit versioned
/// profile revision, never runtime guessing — so this set is closed here and
/// grows only through [`GlyphSet`](crate::GlyphSet).
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Delimiter {
    Parenthesis,
    SquareBracket,
    Brace,
}

impl Delimiter {
    pub fn opening_text(self) -> &'static str {
        match self {
            Self::Parenthesis => "(",
            Self::SquareBracket => "[",
            Self::Brace => "{",
        }
    }

    pub fn closing_text(self) -> &'static str {
        match self {
            Self::Parenthesis => ")",
            Self::SquareBracket => "]",
            Self::Brace => "}",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Parenthesis => "parenthesis",
            Self::SquareBracket => "square bracket",
            Self::Brace => "brace",
        }
    }

    /// Wrap already-rendered children in this delimiter, space-joined:
    /// `Parenthesis.wrap(["Kind", "(Decision)"])` → `"(Kind (Decision))"`.
    pub fn wrap(self, children: impl IntoIterator<Item = String>) -> String {
        let children = children.into_iter().collect::<Vec<_>>();
        format!(
            "{}{}{}",
            self.opening_text(),
            children.join(" "),
            self.closing_text()
        )
    }

    pub(crate) fn closing_character(self) -> char {
        match self {
            Self::Parenthesis => ')',
            Self::SquareBracket => ']',
            Self::Brace => '}',
        }
    }

    pub(crate) fn from_opening(character: char) -> Option<Self> {
        match character {
            '(' => Some(Self::Parenthesis),
            '[' => Some(Self::SquareBracket),
            '{' => Some(Self::Brace),
            _ => None,
        }
    }

    pub(crate) fn from_closing(character: char) -> Option<Self> {
        match character {
            ')' => Some(Self::Parenthesis),
            ']' => Some(Self::SquareBracket),
            '}' => Some(Self::Brace),
            _ => None,
        }
    }
}

/// A `(| … |)` multiline-string carrier. Holds literal text — delimiters,
/// comment markers, whitespace — that a bare atom cannot represent.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct PipeText {
    text: String,
}

impl PipeText {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A bare atom: an unbroken run of symbol characters, carrying no meaning. A
/// period never appears inside an atom — it is a structural dot-application
/// operator — so an atom is always a single dotless segment.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Atom {
    text: String,
}

impl Atom {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Split `text` at its first period into a prefix and, when text follows the
    /// period, a remainder. `None` when `text` carries no period. This is the
    /// single source of the dotted split rule; a period is ordinary text, so
    /// only a consumer expecting a dotted prefix at this position calls it — the
    /// split is expectation-driven, never a content classification.
    pub fn split_text_at_first_dot(text: &str) -> Option<(&str, Option<&str>)> {
        let dot = text.find('.')?;
        let prefix = &text[..dot];
        let remainder = &text[dot + 1..];
        let remainder = if remainder.is_empty() {
            None
        } else {
            Some(remainder)
        };
        Some((prefix, remainder))
    }

    /// The atom's text split at its first period into a prefix atom and an
    /// optional remainder atom — the owned-value form of
    /// [`split_text_at_first_dot`](Atom::split_text_at_first_dot). Note that a
    /// *recognized* atom never carries a period (the recognizer binds a dotted
    /// run into an [`Application`](Block::Application)); this primitive serves a
    /// consumer that has flattened a chain back to text with
    /// [`Block::dotted_text`] and wants to re-split it.
    pub fn split_at_first_dot(&self) -> Option<(Atom, Option<Atom>)> {
        let (prefix, remainder) = Self::split_text_at_first_dot(&self.text)?;
        Some((Atom::new(prefix), remainder.map(Atom::new)))
    }

    /// Whether every character is a bare symbol character (non-empty). A bare
    /// atom excludes whitespace, the period, quote, and every delimiter glyph.
    pub fn qualifies_as_symbol(&self) -> bool {
        !self.text.is_empty()
            && self
                .text
                .chars()
                .all(|character| AtomCharacter::new(character).is_bare_symbol())
    }

    /// A symbol whose first character is ASCII uppercase and which carries no
    /// dash — the PascalCase reading, exposed as a candidate, not a verdict.
    pub fn qualifies_as_pascal_case_symbol(&self) -> bool {
        self.qualifies_as_symbol()
            && self
                .text
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
            && !self.text.contains('-')
    }

    /// A symbol whose first character is ASCII lowercase and which carries no
    /// dash — the camelCase reading.
    pub fn qualifies_as_camel_case_symbol(&self) -> bool {
        self.qualifies_as_symbol()
            && self
                .text
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_lowercase())
            && !self.text.contains('-')
    }

    /// A symbol carrying a dash — the kebab-case reading.
    pub fn qualifies_as_kebab_case_symbol(&self) -> bool {
        self.qualifies_as_symbol() && self.text.contains('-')
    }
}

/// The capitalization classifier, exposed as **data**. The family attaches
/// semantics to case — capitalized-leading reads as an object, lowercase-leading
/// as a name — but that meaning lives entirely outside this crate. Here the case
/// is a fact about an atom's characters and nothing more.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomCase {
    /// A symbol that is neither Pascal, camel, nor kebab — a digit run, a
    /// sigiled or punctuated symbol. The catch-all.
    Symbol,
    PascalCase,
    CamelCase,
    KebabCase,
}

impl AtomCase {
    /// Classify an atom by its characters. Every non-empty symbol falls into
    /// exactly one case, with [`Symbol`](AtomCase::Symbol) as the catch-all.
    pub fn of(atom: &Atom) -> Self {
        if atom.qualifies_as_pascal_case_symbol() {
            Self::PascalCase
        } else if atom.qualifies_as_camel_case_symbol() {
            Self::CamelCase
        } else if atom.qualifies_as_kebab_case_symbol() {
            Self::KebabCase
        } else {
            Self::Symbol
        }
    }

    /// Whether the atom reads as this case.
    pub fn matches(self, atom: &Atom) -> bool {
        match self {
            Self::Symbol => atom.qualifies_as_symbol(),
            Self::PascalCase => atom.qualifies_as_pascal_case_symbol(),
            Self::CamelCase => atom.qualifies_as_camel_case_symbol(),
            Self::KebabCase => atom.qualifies_as_kebab_case_symbol(),
        }
    }
}

/// One character weighed against the bare-atom rule. Data-bearing (it wraps the
/// character), so the rule is a method, not a free predicate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AtomCharacter {
    character: char,
}

impl AtomCharacter {
    pub(crate) fn new(character: char) -> Self {
        Self { character }
    }

    /// Whether this character may sit inside a bare atom. The period is a
    /// structural dot-application operator, so it can never appear in a bare
    /// atom; a string whose content carries a period is therefore an
    /// application of atoms, never a single atom.
    pub(crate) fn is_bare_symbol(self) -> bool {
        !self.character.is_whitespace()
            && !matches!(
                self.character,
                '"' | '.' | '(' | ')' | '[' | ']' | '{' | '}'
            )
    }
}
