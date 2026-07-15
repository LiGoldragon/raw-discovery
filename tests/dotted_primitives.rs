//! Witnesses for the dotted split/join primitives — the `split_at_first_dot`
//! family and `Block::dotted_text` — ported from nota next-gen. Split and join
//! are inverses over a dotted chain, and neither classifies the text it moves.

use raw_discovery::{Atom, Recognizer};

/// The static string split takes a prefix and an optional remainder at the first
/// period, and reports `None` for a dotless string.
#[test]
fn split_text_at_first_dot_takes_prefix_and_remainder() {
    assert_eq!(
        Atom::split_text_at_first_dot("rustfmt.skip"),
        Some(("rustfmt", Some("skip")))
    );
    assert_eq!(
        Atom::split_text_at_first_dot("alpha.beta.gamma"),
        Some(("alpha", Some("beta.gamma"))),
        "only the first period splits; the remainder keeps its dots"
    );
    assert_eq!(
        Atom::split_text_at_first_dot("trailing."),
        Some(("trailing", None)),
        "a trailing period yields an empty (None) remainder"
    );
    assert_eq!(Atom::split_text_at_first_dot("plain"), None);
}

/// The owned-atom split mirrors the string split.
#[test]
fn split_at_first_dot_yields_prefix_and_remainder_atoms() {
    let (prefix, remainder) = Atom::new("rkyv.Archive")
        .split_at_first_dot()
        .expect("splits at the dot");
    assert_eq!(prefix.text(), "rkyv");
    assert_eq!(remainder.expect("remainder").text(), "Archive");

    assert!(Atom::new("dotless").split_at_first_dot().is_none());
}

/// `dotted_text` joins a discovered application chain back to flat dotted text —
/// the inverse of the recognizer's split of a dotted run into an application.
#[test]
fn dotted_text_joins_a_recognized_chain() {
    let document = Recognizer::standard()
        .recognize("alpha.beta.gamma")
        .expect("valid nota");
    let block = document.root_object_at(0).expect("root");
    assert!(block.is_application());
    assert_eq!(block.dotted_text(), Some("alpha.beta.gamma".to_owned()));
}

/// `dotted_text` is `None` when any segment is a delimited or pipe-text block,
/// since those carry no flat text form.
#[test]
fn dotted_text_is_none_for_a_delimited_segment() {
    let document = Recognizer::standard()
        .recognize("head.(a b)")
        .expect("valid nota");
    let block = document.root_object_at(0).expect("root");
    assert!(block.is_application());
    assert_eq!(block.dotted_text(), None);
}

/// A bare atom's dotted text is the atom itself — the base case of the join.
#[test]
fn dotted_text_of_a_bare_atom_is_the_atom() {
    let document = Recognizer::standard()
        .recognize("solitary")
        .expect("valid nota");
    let block = document.root_object_at(0).expect("root");
    assert_eq!(block.dotted_text(), Some("solitary".to_owned()));
}
