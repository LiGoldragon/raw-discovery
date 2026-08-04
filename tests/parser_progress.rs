//! The recognizer must always terminate on malformed structural input. Each
//! recognition runs in a watchdog thread so a regression hangs the worker, not
//! the test runner.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use raw_discovery::{FoundClose, RecognizeError, Recognizer};

fn recognition_terminates(input: &str) -> bool {
    let owned = input.to_string();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = Recognizer::standard().recognize(&owned);
        let _ = sender.send(());
    });
    receiver.recv_timeout(Duration::from_secs(4)).is_ok()
}

#[test]
fn recognition_terminates_on_malformed_curly_or_angle_input() {
    for input in [
        "(a >)",
        "[a <] b]",
        "“]",
        "( “bad \\q escape” )",
        "{ < > }",
        "( Vector.<Ordered> )",
        "(record <])",
    ] {
        assert!(
            recognition_terminates(input),
            "recognition did not terminate on {input:?}"
        );
    }
}

#[test]
fn unmatched_angle_close_is_rejected_with_closing_glyph_evidence() {
    let error = Recognizer::standard()
        .recognize(">")
        .expect_err("a stray angle close is rejected");
    assert!(matches!(
        error,
        RecognizeError::UnexpectedClose {
            found: FoundClose::Glyph('>'),
            ..
        }
    ));
}
