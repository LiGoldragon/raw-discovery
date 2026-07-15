//! Witness that a recognized [`Document`] round-trips through the portable rkyv
//! archive discipline — the exact little-endian / 32-bit-pointer / unaligned
//! feature set, with validation on read. This is the discipline content-identity
//! will later own as the shared `PortableArchive` bound; until it publishes, the
//! feature set is mirrored inline and exercised here.

use raw_discovery::{Document, Recognizer};

fn round_trips(source: &str) {
    let document = Recognizer::standard()
        .recognize(source)
        .expect("valid nota structure");
    let bytes =
        rkyv::to_bytes::<rkyv::rancor::Error>(&document).expect("document serializes to bytes");
    let restored = rkyv::from_bytes::<Document, rkyv::rancor::Error>(&bytes)
        .expect("archived bytes validate and deserialize");
    assert_eq!(
        restored, document,
        "round trip preserves the structure for {source:?}"
    );
}

#[test]
fn a_nested_document_round_trips_through_rkyv() {
    // Every block shape at once: delimiters nested three deep, a
    // right-associative dotted chain, pipe text, and bare atoms.
    round_trips(
        "Public.Newtype.( CommitSequence [ rkyv.Archive Clone ] (|literal ] body|) ) trailing",
    );
}

#[test]
fn each_block_shape_round_trips() {
    for source in [
        "alpha",
        "(a b c)",
        "[a b c]",
        "{a b c}",
        "head.payload",
        "a.b.c",
        "(|pipe text|)",
    ] {
        round_trips(source);
    }
}
