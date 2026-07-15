//! Raw-layer witnesses for the `(| … |)` pipe-text carrier, ported from nota
//! next-gen `tests/block_queries.rs`. Pipe text holds literal content — stray
//! delimiters, quotes, the close marker itself when escaped — that a bare atom
//! cannot represent, and it is never recursively parsed.

use raw_discovery::Recognizer;

#[test]
fn pipe_text_is_delimiter_safe_and_not_recursively_parsed() {
    let source = "(|macro body with ] and \" and apostrophe's text|)";
    let document = Recognizer::standard()
        .recognize(source)
        .expect("valid nota");
    let root = document.root_object_at(0).expect("root");

    assert!(root.is_pipe_text());
    assert_eq!(
        root.demote_to_string(),
        Some("macro body with ] and \" and apostrophe's text")
    );
}

#[test]
fn pipe_text_escapes_single_pipe_close_marker() {
    let source = "(|macro body can contain \\|) without ending|)";
    let document = Recognizer::standard()
        .recognize(source)
        .expect("valid nota");
    let root = document.root_object_at(0).expect("root");

    assert!(root.is_pipe_text());
    assert_eq!(
        root.demote_to_string(),
        Some("macro body can contain |) without ending")
    );
}

#[test]
fn an_unclosed_pipe_text_is_rejected() {
    let error = Recognizer::standard()
        .recognize("(|never closed")
        .expect_err("unclosed pipe text");
    assert!(
        matches!(
            error,
            raw_discovery::RecognizeError::UnclosedPipeText { .. }
        ),
        "{error}"
    );
}
