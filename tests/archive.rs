//! Witness that a recognized [`Document`] round-trips through the portable rkyv
//! archive discipline — the exact little-endian / 32-bit-pointer / unaligned
//! feature set, with validation on read. `content-identity` owns the shared
//! `PortableArchive` bound used here.

use content_identity::{ContentHash, DomainSeparation, HashDomain, LayoutVersion, PortableArchive};
use raw_discovery::{Document, Recognizer};

struct StandardDocumentCompatibilityDomain;

impl HashDomain for StandardDocumentCompatibilityDomain {
    fn separation() -> DomainSeparation {
        DomainSeparation::Contextual {
            context: "raw-discovery standard document archive compatibility",
            layout: LayoutVersion::new(1),
        }
    }
}

const STANDARD_DOCUMENT_ARCHIVE: [u8; 32] = [
    0x68, 0x7b, 0x2c, 0xbe, 0xe0, 0x6f, 0x2c, 0x20, 0x2e, 0xea, 0x1b, 0xd6, 0x54, 0xb9, 0x82, 0x28,
    0x19, 0x68, 0x5f, 0x89, 0x4e, 0x86, 0x34, 0xe2, 0x94, 0xf3, 0x5f, 0x47, 0x1f, 0x0b, 0x33, 0x00,
];

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
fn established_standard_document_archive_is_an_absolute_lock() {
    let document = Recognizer::standard()
        .recognize(
            "Public.Newtype.( CommitSequence [ rkyv.Archive Clone ] (|literal ] body|) ) trailing",
        )
        .expect("standard document");
    let bytes = document.to_archive_bytes().expect("archive");
    let identity = ContentHash::<StandardDocumentCompatibilityDomain>::derive(bytes.as_ref());
    assert_eq!(
        identity.bytes(),
        &STANDARD_DOCUMENT_ARCHIVE,
        "existing Block and Document archive bytes moved"
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
