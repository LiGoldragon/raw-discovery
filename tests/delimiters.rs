//! Raw-layer witnesses for delimiter nesting and the delimited-block query
//! surface, ported and adapted from nota next-gen `tests/block_queries.rs`.
//! Span/reemit assertions are dropped: the recognized tree is span-free by
//! design, so structure is asserted directly.

use raw_discovery::{Delimiter, RecognizeError, Recognizer};

#[test]
fn parses_ordered_root_objects_in_source_order() {
    let document = Recognizer::standard()
        .recognize("(State [Statement]) { Topic [Text] }")
        .expect("valid nota");

    assert_eq!(document.holds_root_objects(), 2);
    let first = document.root_object_at(0).expect("first root");
    let second = document.root_object_at(1).expect("second root");

    assert!(first.is_parenthesis());
    assert!(second.is_brace());
    assert_eq!(
        first.holds_root_objects(),
        2,
        "State and the [Statement] vector"
    );
    assert_eq!(
        second.holds_root_objects(),
        2,
        "Topic and the [Text] vector"
    );
}

#[test]
fn exposes_recursive_shape_predicates() {
    let document = Recognizer::standard()
        .recognize("(Record [Entry Query])")
        .expect("valid nota");
    let root = document.root_object_at(0).expect("root");

    assert!(root.is_parenthesis());
    assert!(root.holds_two_root_objects());
    assert!(root.root_object_at(0).is_some_and(|block| {
        block.qualifies_as_pascal_case_symbol() && block.demote_to_string() == Some("Record")
    }));
    assert!(
        root.root_object_at(1)
            .is_some_and(|block| block.is_square_bracket())
    );
}

#[test]
fn exposes_delimiter_text_and_child_helpers() {
    let document = Recognizer::standard()
        .recognize("[alpha beta]")
        .expect("valid nota");
    let root = document.root_object_at(0).expect("root");

    assert_eq!(Delimiter::SquareBracket.opening_text(), "[");
    assert_eq!(Delimiter::SquareBracket.closing_text(), "]");
    assert_eq!(Delimiter::SquareBracket.description(), "square bracket");
    assert_eq!(
        Delimiter::Parenthesis.wrap(["Kind".to_owned(), "(Decision)".to_owned()]),
        "(Kind (Decision))"
    );
    assert!(root.is_delimited_with(Delimiter::SquareBracket));
    assert_eq!(
        root.as_delimited(Delimiter::SquareBracket)
            .expect("square children")
            .len(),
        2
    );
    assert!(root.as_delimited(Delimiter::Brace).is_none());
}

#[test]
fn nested_delimiters_recover_their_depth() {
    let document = Recognizer::standard()
        .recognize("{ outer [ middle ( inner ) ] }")
        .expect("valid nota");
    let brace = document.root_object_at(0).expect("root");
    assert!(brace.is_brace());
    let bracket = brace.root_object_at(1).expect("bracket child");
    assert!(bracket.is_square_bracket());
    let paren = bracket.root_object_at(1).expect("paren child");
    assert!(paren.is_parenthesis());
    assert_eq!(
        paren.root_object_at(0).and_then(|b| b.demote_to_string()),
        Some("inner")
    );
}

#[test]
fn reports_unclosed_delimiters_with_source_position() {
    let error = Recognizer::standard()
        .recognize("(Record [Entry]")
        .expect_err("invalid nota");

    assert!(matches!(
        error,
        RecognizeError::UnclosedDelimiter { position, .. }
            if position.line == 1 && position.column == 1
    ));
}

#[test]
fn reports_unexpected_close_with_source_position() {
    let error = Recognizer::standard()
        .recognize("alpha ]")
        .expect_err("stray close is rejected");
    assert!(matches!(
        error,
        RecognizeError::UnexpectedClose { found: ']', .. }
    ));
}
