//! Outside-in boundary discovery witnesses. These tests deliberately separate
//! finding an enclosing close from interpreting the bounded interior.

use raw_discovery::{
    BoundaryReader, BoundarySide, RawProfile, SealedBoundaryDiscoverySet, SealedTokenProfile,
    SealedTriggerSet, TokenProfileError, TriggerIdentifier, TriggerMatchKind, TriggerSet,
};

const PARENTHESIS: TriggerIdentifier = TriggerIdentifier::new(0);
const SQUARE: TriggerIdentifier = TriggerIdentifier::new(1);
const BRACE: TriggerIdentifier = TriggerIdentifier::new(2);
const APPLICATION: TriggerIdentifier = TriggerIdentifier::new(3);
const PIPE_TEXT: TriggerIdentifier = TriggerIdentifier::new(4);
const WHITESPACE: TriggerIdentifier = TriggerIdentifier::new(5);
const COMMENT: TriggerIdentifier = TriggerIdentifier::new(6);

fn profile() -> SealedTokenProfile {
    RawProfile::standard().seal().expect("standard profile")
}

fn discovery(profile: &SealedTokenProfile) -> SealedBoundaryDiscoverySet {
    profile
        .seal_boundary_discovery_set(discovery_triggers())
        .expect("boundary discovery set")
}

fn discovery_triggers() -> TriggerSet {
    TriggerSet::new(vec![
        PARENTHESIS,
        SQUARE,
        BRACE,
        PIPE_TEXT,
        WHITESPACE,
        COMMENT,
    ])
}

fn active(profile: &SealedTokenProfile) -> SealedTriggerSet {
    profile
        .seal_trigger_set(discovery_triggers())
        .expect("ordinary active set")
}

#[test]
fn enclosing_close_is_found_before_the_interior_is_read() {
    let profile = profile();
    let discovery = discovery(&profile);
    let active = active(&profile);
    let source = "(outer [inner (| ] ) } |)] tail) after";
    let mut parent = BoundaryReader::new(source, &profile);

    let outer = parent
        .discover_delimited(PARENTHESIS, &discovery)
        .expect("outer boundary is discovered");

    assert_eq!(
        parent.remaining(),
        " after",
        "the parent advances past the matching close before child interpretation"
    );
    assert_eq!(
        parent.source_between(outer.interior().start(), outer.interior().end()),
        "outer [inner (| ] ) } |)] tail"
    );

    let mut interior = BoundaryReader::within(source, &profile, outer.interior());
    assert_eq!(
        interior.read_bare(&active).unwrap(),
        Some("outer".to_owned())
    );
    interior.skip_trivia(&active).unwrap();
    let square = interior
        .discover_delimited(SQUARE, &discovery)
        .expect("nested boundary is discovered inside the explicit bound");
    assert_eq!(
        interior.source_between(square.interior().start(), square.interior().end()),
        "inner (| ] ) } |)"
    );
}

#[test]
fn same_boundary_nesting_is_balanced_before_recursion() {
    let profile = profile();
    let discovery = discovery(&profile);
    let mut reader = BoundaryReader::new("(outer(inner))after", &profile);

    let outer = reader
        .discover_delimited(PARENTHESIS, &discovery)
        .expect("same-spelling nested boundaries balance");

    assert_eq!(
        reader.source_between(outer.interior().start(), outer.interior().end()),
        "outer(inner)"
    );
    assert_eq!(reader.remaining(), "after");
}

#[test]
fn carriers_are_opaque_while_the_enclosing_close_is_sought() {
    let profile = profile();
    let discovery = discovery(&profile);
    let source = "{before (| } ] ) \u{00bb} |) after}tail";
    let mut reader = BoundaryReader::new(source, &profile);

    let outer = reader
        .discover_delimited(BRACE, &discovery)
        .expect("apparent closers inside the carrier are data");

    assert_eq!(
        reader.source_between(outer.interior().start(), outer.interior().end()),
        "before (| } ] ) \u{00bb} |) after"
    );
    assert_eq!(reader.remaining(), "tail");
}

#[test]
fn a_missing_outer_close_is_not_consumed_as_a_child_close() {
    let profile = profile();
    let discovery = discovery(&profile);
    let mut reader = BoundaryReader::new("(outer(inner)", &profile);

    assert!(matches!(
        reader.discover_delimited(PARENTHESIS, &discovery),
        Err(TokenProfileError::UnclosedBoundary {
            identifier,
            byte_offset: 0,
        }) if identifier == PARENTHESIS
    ));
}

#[test]
fn a_mismatched_nested_close_is_typed_before_child_interpretation() {
    let profile = profile();
    let discovery = discovery(&profile);
    let mut reader = BoundaryReader::new("(outer[inner) ]", &profile);

    assert!(matches!(
        reader.discover_delimited(PARENTHESIS, &discovery),
        Err(TokenProfileError::MismatchedBoundary {
            expected,
            found,
            ..
        }) if expected == SQUARE && found == PARENTHESIS
    ));
}

#[test]
fn an_interior_reader_cannot_observe_text_after_its_enclosing_close() {
    let profile = profile();
    let discovery = discovery(&profile);
    let active = active(&profile);
    let source = "(alpha)beta";
    let mut parent = BoundaryReader::new(source, &profile);
    let outer = parent
        .discover_delimited(PARENTHESIS, &discovery)
        .expect("outer boundary");
    let mut child = BoundaryReader::within(source, &profile, outer.interior());

    assert_eq!(child.read_bare(&active).unwrap(), Some("alpha".to_owned()));
    assert!(child.is_end());
    assert!(child.longest_match(&active).unwrap().is_none());
    assert_eq!(parent.remaining(), "beta");
}

#[test]
fn horizontal_triggers_cannot_enter_a_boundary_discovery_set() {
    let profile = profile();
    assert!(matches!(
        profile.seal_boundary_discovery_set(TriggerSet::new(vec![
            PARENTHESIS,
            APPLICATION,
        ])),
        Err(TokenProfileError::UnsupportedBoundaryDiscoveryTrigger(identifier))
            if identifier == APPLICATION
    ));
}

#[test]
fn discovery_records_the_expected_opening_and_closing_sides() {
    let profile = profile();
    let active = profile
        .seal_trigger_set(TriggerSet::new(vec![PARENTHESIS]))
        .expect("single boundary active");
    let reader = BoundaryReader::new("(body)", &profile);
    let opening = reader
        .longest_match(&active)
        .expect("match")
        .expect("opening");
    assert_eq!(
        opening.kind(),
        TriggerMatchKind::Boundary(BoundarySide::Opening)
    );
}
