//! Raw-layer witnesses for right-associative dot-application binding, ported and
//! adapted from nota next-gen `tests/next_gen_grammar.rs`. These cover only the
//! *raw* rules — the discovered application shape and its glued-period
//! constraint — not the typed codec (float/string/vector/map decoding), which
//! belongs to nota's codec and the future structural-codec.

use raw_discovery::{Block, Recognizer};

fn recognize_single(source: &str) -> Block {
    let document = Recognizer::standard()
        .recognize(source)
        .expect("valid nota structure");
    assert_eq!(document.holds_root_objects(), 1, "one root object");
    document.root_object_at(0).expect("root").clone()
}

/// A glued period binds a head to the following payload as one application
/// block; head and payload are recovered structurally, not by splitting atom
/// text.
#[test]
fn dot_binds_head_to_payload_as_one_application() {
    let block = recognize_single("Variant.Data");
    let (head, payload) = block.as_application().expect("dot-application");
    assert_eq!(head.demote_to_string(), Some("Variant"));
    assert_eq!(payload.demote_to_string(), Some("Data"));
}

/// A delimited payload — parenthesis, square bracket, or brace — binds to its
/// head as one application.
#[test]
fn dot_binds_each_delimited_payload_kind() {
    for source in ["Variant.(a b)", "Variant.[a b]", "Variant.{a b}"] {
        let block = recognize_single(source);
        let (head, payload) = block.as_application().expect("dot-application");
        assert_eq!(head.demote_to_string(), Some("Variant"), "{source}");
        assert_eq!(
            payload.holds_root_objects(),
            2,
            "{source} payload holds its two objects"
        );
    }
}

/// A dotted chain binds right-associatively: the head is the leftmost single
/// segment and the payload is the remainder, so `Private.secretDigest.StateDigest`
/// reads as visibility, then the (name, type) remainder.
#[test]
fn dotted_chain_binds_right_associatively() {
    let block = recognize_single("Private.secretDigest.StateDigest");
    let (visibility, remainder) = block.as_application().expect("outer application");
    assert_eq!(visibility.demote_to_string(), Some("Private"));

    let (name, kind) = remainder.as_application().expect("inner application");
    assert_eq!(name.demote_to_string(), Some("secretDigest"));
    assert_eq!(kind.demote_to_string(), Some("StateDigest"));
}

/// The period binds only when glued to both sides.
#[test]
fn a_period_binds_only_when_glued_on_both_sides() {
    let recognizer = Recognizer::standard();
    assert!(
        recognizer.recognize("Head .Payload").is_err(),
        "a space before the period is not a binding"
    );
    assert!(
        recognizer.recognize("Head. Payload").is_err(),
        "a space after the period leaves the application dangling"
    );
    assert!(
        recognizer.recognize("Head.").is_err(),
        "a trailing period has no payload"
    );
    assert!(
        recognizer.recognize(".Payload").is_err(),
        "a leading period has no head"
    );
}

/// A period is a structural operator, so an atom never contains one: a dotted
/// path is an application whose flat text is reconstructed from its segments
/// rather than read as a single atom.
#[test]
fn a_dotted_path_reconstructs_its_flat_text() {
    let block = recognize_single("rustfmt.skip");
    assert!(block.is_application(), "rustfmt.skip is an application");
    assert_eq!(
        block.demote_to_string(),
        None,
        "an application is not a flat string"
    );
    assert_eq!(block.dotted_text(), Some("rustfmt.skip".to_owned()));
}

/// A float literal's fractional period is a structural dot, so `-122.3` parses
/// as an application whose dotted text a numeric consumer would reconstruct. The
/// raw layer discovers the application and never decides it is a number.
#[test]
fn a_fractional_period_is_a_structural_dot() {
    let block = recognize_single("-122.3");
    assert!(block.is_application(), "the period in -122.3 binds");
    assert_eq!(block.demote_to_string(), None);
    assert_eq!(block.dotted_text(), Some("-122.3".to_owned()));
}

/// The psyche-authored base sample parses with its intended nested application
/// shape: `Public.Newtype.( … )` is visibility, then kind, then the
/// parenthesised body.
#[test]
fn psyche_authored_newtype_sample_parses_with_intended_shape() {
    let source = r#"Public.Newtype.(
  CommitSequence
  [ Literal.[rustfmt.skip]
    Derive.[rkyv.[Archive Serialize Deserialize] Clone Debug PartialEq Eq] ]
  Integer
)"#;
    let block = recognize_single(source);

    let (visibility, remainder) = block.as_application().expect("outer application");
    assert_eq!(visibility.demote_to_string(), Some("Public"));
    let (kind, body) = remainder.as_application().expect("kind application");
    assert_eq!(kind.demote_to_string(), Some("Newtype"));
    assert!(
        body.is_parenthesis(),
        "the body is the parenthesised payload"
    );
    assert_eq!(
        body.holds_root_objects(),
        3,
        "name, attributes vector, type"
    );
}
