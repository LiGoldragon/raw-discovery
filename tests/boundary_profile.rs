//! Laws of the sealed, cursor-local boundary profile.

use raw_discovery::{
    BoundaryReader, CharacterClass, CharacterSet, ProfileRevision, RawProfile, TokenProfileData,
    TokenProfileError, Trigger, TriggerDefinition, TriggerIdentifier, TriggerMatchKind, TriggerSet,
};

const STANDARD_PROFILE_LAYOUT_TWO: [u8; 32] = [
    0x3d, 0x40, 0x2a, 0x87, 0xc6, 0x77, 0x83, 0xeb, 0x5d, 0x5d, 0xd8, 0x6f, 0xb7, 0x9f, 0xc5, 0x13,
    0x0b, 0x23, 0xf0, 0xa1, 0x0d, 0x02, 0x9e, 0x2d, 0xaf, 0x12, 0x37, 0x61, 0x9c, 0xc3, 0x0d, 0x4f,
];

const STANDARD_PROFILE_LAYOUT_THREE: [u8; 32] = [
    0x0e, 0xd2, 0xcb, 0x68, 0x06, 0xf7, 0xd3, 0xf4, 0xbc, 0x54, 0x9f, 0x22, 0xc0, 0x6d, 0x81, 0x13,
    0x95, 0x2a, 0x0d, 0xfc, 0x70, 0x29, 0x23, 0x7a, 0x2e, 0xfa, 0xca, 0x6e, 0xf5, 0x9b, 0x73, 0x28,
];

const EXHAUSTIVE_PROFILE_LAYOUT_TWO: [u8; 32] = [
    0xfe, 0x70, 0x4d, 0xae, 0x7d, 0x6c, 0xce, 0x9a, 0x0e, 0x26, 0x1f, 0xcd, 0x70, 0xe7, 0x42, 0xbd,
    0xb2, 0x49, 0x08, 0xe1, 0xc7, 0x4f, 0xb1, 0x98, 0x08, 0x26, 0x47, 0xab, 0x43, 0xcd, 0x78, 0xcd,
];

const EXHAUSTIVE_PROFILE_LAYOUT_THREE: [u8; 32] = [
    0xc2, 0x9d, 0x3f, 0xab, 0x10, 0x1c, 0x77, 0x66, 0xc2, 0xb7, 0x38, 0xbb, 0x34, 0x93, 0xea, 0x5a,
    0x89, 0xac, 0xde, 0xba, 0x01, 0xe1, 0x7f, 0x7c, 0xd0, 0xe7, 0xa1, 0xa9, 0x0a, 0xf3, 0x2f, 0x81,
];

fn definition(identifier: u16, trigger: Trigger) -> TriggerDefinition {
    TriggerDefinition {
        identifier: TriggerIdentifier::new(identifier),
        trigger,
    }
}

fn profile(definitions: Vec<TriggerDefinition>, active: &[u16]) -> TokenProfileData {
    TokenProfileData::new(
        ProfileRevision::new(1),
        definitions,
        TriggerSet::new(active.iter().copied().map(TriggerIdentifier::new).collect()),
        CharacterSet::from_text("\""),
    )
}

#[test]
fn standard_profile_identity_is_an_absolute_layout_three_lock() {
    let sealed = RawProfile::standard().seal().expect("standard seals");
    assert_ne!(
        sealed.identity().bytes(),
        &STANDARD_PROFILE_LAYOUT_TWO,
        "layout three must replace the layout-two identity"
    );
    assert_ne!(
        sealed.identity().bytes(),
        &[0; 32],
        "sealed profiles must derive an identity"
    );
    assert_eq!(
        sealed.identity().bytes(),
        &STANDARD_PROFILE_LAYOUT_THREE,
        "profile data or its layout moved"
    );
}

#[test]
fn every_profile_and_character_class_variant_has_an_absolute_layout_three_lock() {
    let sealed = profile(
        vec![
            definition(
                0,
                Trigger::Boundary {
                    opening: "(".to_owned(),
                    closing: ")".to_owned(),
                },
            ),
            definition(
                1,
                Trigger::Application {
                    glyph: ".".to_owned(),
                },
            ),
            definition(
                2,
                Trigger::Punctuation {
                    glyph: "=>".to_owned(),
                },
            ),
            definition(
                3,
                Trigger::Carrier {
                    opening: "r#\"".to_owned(),
                    closing: "\"#".to_owned(),
                    escape: Some("\\".to_owned()),
                },
            ),
            definition(
                4,
                Trigger::Whitespace {
                    canonical_spelling: " ".to_owned(),
                },
            ),
            definition(
                5,
                Trigger::LineComment {
                    opening: "//".to_owned(),
                },
            ),
            definition(
                6,
                Trigger::LeadingCharacterClass {
                    leading: CharacterClass::AsciiDigit,
                    continuation: CharacterClass::AsciiAlphabetic,
                },
            ),
            definition(
                7,
                Trigger::LeadingCharacterClass {
                    leading: CharacterClass::AsciiAlphanumeric,
                    continuation: CharacterClass::Whitespace,
                },
            ),
            definition(
                8,
                Trigger::LeadingCharacterClass {
                    leading: CharacterClass::Characters(CharacterSet::from_text("_\u{00df}")),
                    continuation: CharacterClass::Characters(CharacterSet::from_text("-_")),
                },
            ),
        ],
        &[],
    )
    .seal()
    .expect("exhaustive profile seals");
    assert_ne!(
        sealed.identity().bytes(),
        &EXHAUSTIVE_PROFILE_LAYOUT_TWO,
        "layout three must replace the layout-two identity"
    );
    assert_ne!(
        sealed.identity().bytes(),
        &[0; 32],
        "sealed profiles must derive an identity"
    );
    assert_eq!(
        sealed.identity().bytes(),
        &EXHAUSTIVE_PROFILE_LAYOUT_THREE,
        "a token-profile or character-class archive variant moved"
    );
}

#[test]
fn character_sets_normalize_order_and_duplicates_before_identity() {
    let reordered = CharacterSet::new(['\u{00df}', '_', '-', '_']);
    let canonical = CharacterSet::new(['-', '_', '\u{00df}']);
    assert_eq!(reordered, canonical);
    assert_eq!(reordered.characters(), &['-', '_', '\u{00df}']);

    let reordered_profile = profile(
        vec![definition(
            0,
            Trigger::LeadingCharacterClass {
                leading: CharacterClass::Characters(reordered),
                continuation: CharacterClass::AsciiAlphanumeric,
            },
        )],
        &[],
    )
    .seal()
    .expect("reordered profile seals");
    let canonical_profile = profile(
        vec![definition(
            0,
            Trigger::LeadingCharacterClass {
                leading: CharacterClass::Characters(canonical),
                continuation: CharacterClass::AsciiAlphanumeric,
            },
        )],
        &[],
    )
    .seal()
    .expect("canonical profile seals");

    assert_eq!(reordered_profile.identity(), canonical_profile.identity());
}

#[test]
fn whitespace_canonical_spelling_is_nonempty_and_identity_bearing() {
    let standard = RawProfile::standard().seal().expect("standard seals");
    let standard_whitespace = standard
        .definition(TriggerIdentifier::new(5))
        .expect("standard whitespace definition");
    assert_eq!(standard_whitespace.trigger.canonical_spelling(), Some(" "));
    assert!(
        standard_whitespace
            .trigger
            .canonical_spelling()
            .is_some_and(|spelling| !spelling.is_empty())
    );

    let space = profile(
        vec![definition(
            0,
            Trigger::Whitespace {
                canonical_spelling: " ".to_owned(),
            },
        )],
        &[0],
    )
    .seal()
    .expect("space profile seals");
    let tab = profile(
        vec![definition(
            0,
            Trigger::Whitespace {
                canonical_spelling: "\t".to_owned(),
            },
        )],
        &[0],
    )
    .seal()
    .expect("tab profile seals");
    assert_ne!(space.identity(), tab.identity());

    let empty = profile(
        vec![definition(
            0,
            Trigger::Whitespace {
                canonical_spelling: String::new(),
            },
        )],
        &[0],
    )
    .seal();
    assert!(matches!(
        empty,
        Err(TokenProfileError::EmptyTrigger {
            identifier,
            role: raw_discovery::TriggerTextRole::CanonicalSpelling,
        }) if identifier == TriggerIdentifier::new(0)
    ));
}

#[test]
fn whitespace_matching_remains_class_driven_despite_its_canonical_spelling() {
    let sealed = profile(
        vec![definition(
            0,
            Trigger::Whitespace {
                canonical_spelling: " ".to_owned(),
            },
        )],
        &[0],
    )
    .seal()
    .expect("profile seals");
    let active = sealed.root_trigger_set();
    let reader = BoundaryReader::new("\t\u{2003}tail", &sealed);
    let matched = reader
        .longest_match(&active)
        .expect("matching succeeds")
        .expect("whitespace run matches");
    assert_eq!(matched.kind(), TriggerMatchKind::Trivia);
    assert_eq!(matched.end(), "\t\u{2003}".len());
}

#[test]
fn longest_complete_match_is_universal_within_the_active_set() {
    let sealed = profile(
        vec![
            definition(
                0,
                Trigger::Application {
                    glyph: ".".to_owned(),
                },
            ),
            definition(
                1,
                Trigger::Punctuation {
                    glyph: "..=".to_owned(),
                },
            ),
        ],
        &[0, 1],
    )
    .seal()
    .expect("prefix overlap is resolved by length");
    let active = sealed.root_trigger_set();
    let reader = BoundaryReader::new("..=tail", &sealed);
    let matched = reader
        .longest_match(&active)
        .expect("matching succeeds")
        .expect("punctuation matches");
    assert_eq!(matched.identifier(), TriggerIdentifier::new(1));
    assert_eq!(matched.kind(), TriggerMatchKind::Punctuation);
    assert_eq!(matched.end(), 3);
}

#[test]
fn equal_complete_matches_cannot_seal() {
    let outcome = profile(
        vec![
            definition(
                0,
                Trigger::Application {
                    glyph: ".".to_owned(),
                },
            ),
            definition(
                1,
                Trigger::Punctuation {
                    glyph: ".".to_owned(),
                },
            ),
        ],
        &[0, 1],
    )
    .seal();
    assert!(matches!(
        outcome,
        Err(TokenProfileError::AmbiguousTriggerSet {
            first,
            second,
        }) if first == TriggerIdentifier::new(0) && second == TriggerIdentifier::new(1)
    ));
}

#[test]
fn empty_carrier_openings_are_unrepresentable_after_seal() {
    let outcome = profile(
        vec![definition(
            0,
            Trigger::Carrier {
                opening: String::new(),
                closing: "\"".to_owned(),
                escape: None,
            },
        )],
        &[0],
    )
    .seal();
    assert!(matches!(
        outcome,
        Err(TokenProfileError::EmptyTrigger {
            identifier,
            ..
        }) if identifier == TriggerIdentifier::new(0)
    ));
}

#[test]
fn a_leading_character_class_participates_in_disjointness() {
    let outcome = profile(
        vec![
            definition(
                0,
                Trigger::LeadingCharacterClass {
                    leading: CharacterClass::AsciiDigit,
                    continuation: CharacterClass::AsciiDigit,
                },
            ),
            definition(
                1,
                Trigger::Punctuation {
                    glyph: "7".to_owned(),
                },
            ),
        ],
        &[0, 1],
    )
    .seal();
    assert!(matches!(
        outcome,
        Err(TokenProfileError::AmbiguousTriggerSet { .. })
    ));
}

#[test]
fn a_complete_multi_character_class_tie_cannot_seal() {
    let outcome = profile(
        vec![
            definition(
                0,
                Trigger::LeadingCharacterClass {
                    leading: CharacterClass::AsciiAlphabetic,
                    continuation: CharacterClass::AsciiAlphanumeric,
                },
            ),
            definition(
                1,
                Trigger::Punctuation {
                    glyph: "abc7".to_owned(),
                },
            ),
        ],
        &[0, 1],
    )
    .seal();
    assert!(matches!(
        outcome,
        Err(TokenProfileError::AmbiguousTriggerSet { .. })
    ));
}

#[test]
fn prefix_compatible_carriers_cannot_share_an_active_set() {
    let outcome = profile(
        vec![
            definition(
                0,
                Trigger::Carrier {
                    opening: "r\"".to_owned(),
                    closing: "\"".to_owned(),
                    escape: None,
                },
            ),
            definition(
                1,
                Trigger::Carrier {
                    opening: "r\"#".to_owned(),
                    closing: "#\"".to_owned(),
                    escape: None,
                },
            ),
        ],
        &[0, 1],
    )
    .seal();
    assert!(matches!(
        outcome,
        Err(TokenProfileError::AmbiguousTriggerSet { .. })
    ));
}

#[test]
fn a_comment_and_exact_spelling_tie_cannot_seal() {
    let outcome = profile(
        vec![
            definition(
                0,
                Trigger::LineComment {
                    opening: "//".to_owned(),
                },
            ),
            definition(
                1,
                Trigger::Punctuation {
                    glyph: "//=".to_owned(),
                },
            ),
        ],
        &[0, 1],
    )
    .seal();
    assert!(matches!(
        outcome,
        Err(TokenProfileError::AmbiguousTriggerSet { .. })
    ));
}

#[test]
fn a_boundary_cannot_assign_one_spelling_to_both_sides() {
    let outcome = profile(
        vec![definition(
            0,
            Trigger::Boundary {
                opening: "|".to_owned(),
                closing: "|".to_owned(),
            },
        )],
        &[0],
    )
    .seal();
    assert!(matches!(
        outcome,
        Err(TokenProfileError::AmbiguousTriggerDefinition(identifier))
            if identifier == TriggerIdentifier::new(0)
    ));
}

#[test]
fn bare_atoms_are_negative_space_between_active_triggers() {
    let sealed = profile(
        vec![
            definition(
                0,
                Trigger::Punctuation {
                    glyph: "+".to_owned(),
                },
            ),
            definition(
                1,
                Trigger::Whitespace {
                    canonical_spelling: " ".to_owned(),
                },
            ),
        ],
        &[0, 1],
    )
    .seal()
    .expect("profile seals");
    let active = sealed.root_trigger_set();
    let mut reader = BoundaryReader::new("alpha+beta", &sealed);

    assert_eq!(
        reader.read_bare(&active).expect("bare read").as_deref(),
        Some("alpha")
    );
    let plus = reader
        .consume(&active)
        .expect("punctuation read")
        .expect("punctuation exists");
    assert_eq!(plus.kind(), TriggerMatchKind::Punctuation);
    assert_eq!(
        reader.read_bare(&active).expect("bare read").as_deref(),
        Some("beta")
    );
}

#[test]
fn carrier_matching_starts_only_on_its_complete_opening_prefix() {
    let sealed = profile(
        vec![
            definition(
                0,
                Trigger::Carrier {
                    opening: "r\"".to_owned(),
                    closing: "\"".to_owned(),
                    escape: Some("\\".to_owned()),
                },
            ),
            definition(
                1,
                Trigger::Whitespace {
                    canonical_spelling: " ".to_owned(),
                },
            ),
        ],
        &[0, 1],
    )
    .seal()
    .expect("profile seals");
    let active = sealed.root_trigger_set();
    let mut reader = BoundaryReader::new("raw r\"body\"", &sealed);

    assert_eq!(
        reader.read_bare(&active).expect("bare read").as_deref(),
        Some("raw")
    );
    reader.skip_trivia(&active).expect("space");
    let carrier = reader
        .consume(&active)
        .expect("carrier read")
        .expect("carrier exists");
    assert_eq!(carrier.kind(), TriggerMatchKind::Carrier);
    assert_eq!(carrier.body(), Some("body"));
    assert!(reader.is_end());
}

#[test]
fn an_active_set_is_bound_to_the_profile_that_sealed_it() {
    let standard = RawProfile::standard().seal().expect("standard");
    let nomos = RawProfile::nomos_extended().seal().expect("nomos");
    let standard_active = standard.root_trigger_set();
    let reader = BoundaryReader::new("alpha", &nomos);
    assert!(matches!(
        reader.longest_match(&standard_active),
        Err(TokenProfileError::TriggerSetProfileMismatch)
    ));
}
