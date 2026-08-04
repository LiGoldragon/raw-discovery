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
    0xab, 0x97, 0xaa, 0x72, 0xf8, 0xea, 0x75, 0xf7, 0x66, 0x2b, 0xe6, 0xda, 0x16, 0xec, 0x01, 0x8d,
    0x55, 0x9b, 0x17, 0xd6, 0xb2, 0x69, 0x35, 0x10, 0x98, 0x9d, 0xbe, 0x1c, 0xd1, 0xb6, 0xf4, 0x6f,
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
    // right-associative dotted chain, curly text, angle application, and bare atoms.
    round_trips(
        "Public.Newtype.( CommitSequence [ rkyv.Archive Clone ] “literal ] body” Vector<Ordered> ) trailing",
    );
}

#[test]
fn established_standard_document_archive_is_an_absolute_lock() {
    let document = Recognizer::standard()
        .recognize(
            "Public.Newtype.( CommitSequence [ rkyv.Archive Clone ] “literal ] body” Vector<Ordered> ) trailing",
        )
        .expect("standard document");
    let bytes = document.to_archive_bytes().expect("archive");
    let identity = ContentHash::<StandardDocumentCompatibilityDomain>::derive(bytes.as_ref());
    assert_eq!(
        identity.bytes(),
        &STANDARD_DOCUMENT_ARCHIVE,
        "redesigned standard Block and Document archive bytes moved"
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
        "“curly text”",
        "Vector<Ordered>",
    ] {
        round_trips(source);
    }
}
