//! Raw-layer witnesses for the `“ … ”` curly-text carrier. Curly text holds
//! literal content — stray delimiters, quotes, and whitespace — that a bare
//! atom cannot represent, and it is never recursively parsed.

use raw_discovery::Recognizer;

#[test]
fn curly_text_is_delimiter_safe_and_not_recursively_parsed() {
    let source = "“macro body with ] and \" and apostrophe's text”";
    let document = Recognizer::standard()
        .recognize(source)
        .expect("valid nota");
    let root = document.root_object_at(0).expect("root");

    assert!(root.is_curly_text());
    assert_eq!(
        root.demote_to_string(),
        Some("macro body with ] and \" and apostrophe's text")
    );
}

#[test]
fn curly_text_balances_nested_quotes_and_escapes_unmatched_quotes() {
    let source = "“outer “nested” \\” literal close \\“ literal open \\\\ slash”";
    let document = Recognizer::standard()
        .recognize(source)
        .expect("valid nota");
    let root = document.root_object_at(0).expect("root");

    assert!(root.is_curly_text());
    assert_eq!(
        root.demote_to_string(),
        Some("outer “nested” ” literal close “ literal open \\ slash")
    );
}

#[test]
fn unclosed_or_malformed_curly_text_is_rejected() {
    let error = Recognizer::standard()
        .recognize("“never closed")
        .expect_err("unclosed curly text");
    assert!(
        matches!(
            error,
            raw_discovery::RecognizeError::UnclosedCurlyText { .. }
        ),
        "{error}"
    );

    assert!(
        Recognizer::standard()
            .recognize("“bad \\q escape”")
            .is_err()
    );
}
