//! Witnesses for the versioned raw profile: the `Standard` glyph set rejects the
//! `$` sigil that `NomosExtended` admits, and the profile revision is data. The
//! glyph vocabulary is versioned data, never a runtime guess.

use raw_discovery::{GlyphSet, ProfileRevision, RawProfile, RecognizeError, Recognizer};

#[test]
fn standard_rejects_a_dollar_sigiled_atom() {
    let error = Recognizer::standard()
        .recognize("$capture")
        .expect_err("the standard glyph set forbids the $ sigil");
    assert!(
        matches!(error, RecognizeError::UnsupportedGlyph { glyph: '$', .. }),
        "{error}"
    );
}

#[test]
fn nomos_extended_admits_a_dollar_sigiled_atom() {
    let document = Recognizer::nomos_extended()
        .recognize("$capture")
        .expect("the nomos glyph set admits the $ sigil");
    let atom = document.root_object_at(0).expect("root");
    assert_eq!(atom.demote_to_string(), Some("$capture"));
    assert!(atom.qualifies_as_symbol());
}

#[test]
fn a_dollar_sigil_is_rejected_wherever_it_hides_under_standard() {
    // Inside a delimiter and inside a dotted head, the standard set still rejects.
    for source in ["(alpha $beta)", "head.$payload"] {
        let error = Recognizer::standard()
            .recognize(source)
            .expect_err("standard rejects $ anywhere");
        assert!(
            matches!(error, RecognizeError::UnsupportedGlyph { glyph: '$', .. }),
            "{source}: {error}"
        );
    }
}

#[test]
fn the_standard_glyph_set_forbids_the_sigil_and_nomos_admits_it() {
    assert!(!GlyphSet::Standard.admits_dollar_sigil());
    assert!(GlyphSet::NomosExtended.admits_dollar_sigil());
}

#[test]
fn a_profile_is_versioned_data() {
    let profile = RawProfile::new(ProfileRevision::new(7), GlyphSet::NomosExtended);
    assert_eq!(profile.revision(), ProfileRevision::new(7));
    assert_eq!(profile.revision().value(), 7);
    assert_eq!(profile.glyphs(), GlyphSet::NomosExtended);

    // Revisions order, so a consumer can pin and compare them.
    assert!(ProfileRevision::new(1) < ProfileRevision::new(2));

    // The named profiles are ordinary data values.
    assert_eq!(RawProfile::standard().glyphs(), GlyphSet::Standard);
    assert_eq!(
        RawProfile::nomos_extended().glyphs(),
        GlyphSet::NomosExtended
    );
}
