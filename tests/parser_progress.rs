//! The recognizer must always terminate. A misplaced pipe-close (`|)`) at object
//! position must not spin the atom reader on a zero-width atom (which would grow
//! the block vector unboundedly). Ported from nota next-gen
//! `tests/parser_progress.rs`; each recognition runs in a watchdog thread so a
//! regression hangs the worker, not the test runner.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use raw_discovery::Recognizer;

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
