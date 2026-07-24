//! Laws of the sealed, cursor-local boundary profile.

use raw_discovery::{
    BoundaryReader, CharacterClass, ProfileRevision, RawProfile, TokenProfileData,
    TokenProfileError, Trigger, TriggerDefinition, TriggerIdentifier, TriggerMatchKind, TriggerSet,
};

const STANDARD_PROFILE_LAYOUT_ONE: [u8; 32] = [
    0xe7, 0xd1, 0x44, 0xca, 0x2f, 0x96, 0x69, 0x6b, 0xdf, 0x3a, 0xd2, 0x3f, 0xe7, 0xd8, 0xa3, 0xb7,
    0xa3, 0x75, 0x6b, 0x9f, 0x30, 0x76, 0x8d, 0x5c, 0x93, 0x64, 0x51, 0xd4, 0xd2, 0x5a, 0x7c, 0xd6,
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
        "\"".to_owned(),
    )
}

#[test]
fn standard_profile_identity_is_an_absolute_layout_one_lock() {
    let sealed = RawProfile::standard().seal().expect("standard seals");
    assert_eq!(
        sealed.identity().bytes(),
        &STANDARD_PROFILE_LAYOUT_ONE,
        "profile data or its layout moved"
    );
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
            definition(1, Trigger::Whitespace),
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
            definition(1, Trigger::Whitespace),
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
