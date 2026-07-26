//! The recognizer must always terminate. A misplaced pipe-close (`|)`) at object
//! position must not spin the atom reader on a zero-width atom (which would grow
//! the block vector unboundedly). Ported from nota next-gen
//! `tests/parser_progress.rs`; each recognition runs in a watchdog thread so a
//! regression hangs the worker, not the test runner.

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
fn recognition_terminates_on_stray_pipe_close() {
    for input in [
        "(a |])",
        "[a |] b]",
        "(|])",
        "( |] )",
        "{ |} }",
        "( |) )",
        "(record [|]x)",
    ] {
        assert!(
            recognition_terminates(input),
            "recognition did not terminate on {input:?}"
        );
    }
}

#[test]
fn pipe_close_evidence_distinguishes_end_of_input_from_a_closing_glyph() {
    let truncated = Recognizer::standard()
        .recognize("|")
        .expect("a bare pipe at end of input remains an atom");
    assert_eq!(
        truncated
            .root_object_at(0)
            .and_then(|block| block.demote_to_string()),
        Some("|")
    );

    let error = Recognizer::standard()
        .recognize("|)")
        .expect_err("a stray pipe close is rejected");
    assert!(matches!(
        error,
        RecognizeError::UnexpectedClose {
            found: FoundClose::Glyph(')'),
            ..
        }
    ));
}
