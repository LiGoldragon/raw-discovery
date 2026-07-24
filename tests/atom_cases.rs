//! Raw-layer witnesses for the capitalization classifier and structural-atom
//! candidates, ported and adapted from nota next-gen `tests/block_queries.rs`.
//! The classifier is exposed as data; no test asserts any meaning is stamped
//! onto an atom.

use raw_discovery::{Atom, AtomCase, Recognizer};

/// Case-shaped structural candidate predicates are answered on demand from the
/// atom's characters; they do not stamp a meaning onto the atom. `42` is not a
/// number here — the recognizer records no content classification.
#[test]
fn exposes_structural_candidates_without_content_classification() {
    let document = Recognizer::standard()
        .recognize(
            "TypeName field-name camelName schema:module:Type CustomMacro RecordPayload 42 name@host required* a&b score^2 100% x>y x<y path/to a;b",
        )
        .expect("valid nota");
    let roots = document.root_objects();

    assert!(roots[0].qualifies_as_pascal_case_symbol());
    assert!(roots[1].qualifies_as_kebab_case_symbol());
    assert!(roots[2].qualifies_as_camel_case_symbol());
    assert!(roots[3].qualifies_as_symbol());
    assert!(roots[4].qualifies_as_symbol());
    assert_eq!(
        roots[4].demote_to_string(),
        Some("CustomMacro"),
        "macro names are plain symbols; context above this layer decides meaning"
    );
    assert!(roots[5].qualifies_as_pascal_case_symbol());

    // `42` is a symbol-safe atom, not a number: no content classification.
    assert!(roots[6].qualifies_as_symbol());
    assert_eq!(roots[6].demote_to_string(), Some("42"));
    assert!(!roots[6].qualifies_as_pascal_case_symbol());
    assert!(!roots[6].qualifies_as_camel_case_symbol());
    assert!(!roots[6].qualifies_as_kebab_case_symbol());

    for root in &roots[7..] {
        assert!(root.qualifies_as_symbol(), "{root:?}");
    }
}

/// `AtomCase::of` classifies every non-empty atom into exactly one case, with
/// `Symbol` as the catch-all.
#[test]
fn atom_case_classifies_each_leading_shape() {
    assert_eq!(AtomCase::of(&Atom::new("TypeName")), AtomCase::PascalCase);
    assert_eq!(AtomCase::of(&Atom::new("camelName")), AtomCase::CamelCase);
    assert_eq!(AtomCase::of(&Atom::new("kebab-name")), AtomCase::KebabCase);
    assert_eq!(AtomCase::of(&Atom::new("42")), AtomCase::Symbol);
    assert_eq!(
        AtomCase::of(&Atom::new("@handle")),
        AtomCase::Symbol,
        "a non-letter leading character is the Symbol catch-all"
    );
    // The classifier reads only the leading character and the dash; interior
    // punctuation does not disturb the camelCase reading.
    assert_eq!(AtomCase::of(&Atom::new("name@host")), AtomCase::CamelCase);
}

/// A dash makes an atom kebab regardless of its leading letter — kebab and the
/// dashless Pascal/camel readings are disjoint.
#[test]
fn a_dashed_atom_reads_as_kebab_not_pascal_or_camel() {
    let dashed = Atom::new("Foo-bar");
    assert_eq!(AtomCase::of(&dashed), AtomCase::KebabCase);
    assert!(!dashed.qualifies_as_pascal_case_symbol());
    assert!(!dashed.qualifies_as_camel_case_symbol());
}

/// `AtomCase::matches` is the public partition predicate: only the atom's one
/// classified case matches.
#[test]
fn atom_case_matches_agrees_with_partitioned_classification() {
    let atom = Atom::new("PascalThing");
    assert!(AtomCase::PascalCase.matches(&atom));
    assert!(!AtomCase::Symbol.matches(&atom));
    assert!(!AtomCase::CamelCase.matches(&atom));
    assert!(!AtomCase::KebabCase.matches(&atom));
}

/// A double semicolon is a comment; a single semicolon is ordinary atom text.
#[test]
fn double_semicolon_is_comment_and_single_semicolon_is_atom_text() {
    let source = "alpha;beta ;; comment text\n gamma;; trailing comment";
    let document = Recognizer::standard()
        .recognize(source)
        .expect("valid nota");
    let roots = document.root_objects();

    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].demote_to_string(), Some("alpha;beta"));
    assert_eq!(roots[1].demote_to_string(), Some("gamma"));
}
