//! Rust cue-to-termination pass-1 witnesses.
//!
//! These tests configure only lexical boundary rules. They do not interpret
//! the discovered blocks as Rust declarations or allocate language identity.

use raw_discovery::{
    BlockDiscoveryError, BlockTree, BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
    BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryError, BoundaryDiscoveryTransition,
    CharacterClass, CharacterSet, CueTerminatedBlockCueEvidence,
    CueTerminatedBlockDiscoveryConfiguration, CueTerminationRule, CueTerminationRuleIdentifier,
    DiscoveredCueTerminatedBlockTree, ProfileRevision, SourceBound, TokenProfileData,
    TokenProfileError, Trigger, TriggerDefinition, TriggerIdentifier, TriggerSet,
};

const PARENTHESIS: TriggerIdentifier = TriggerIdentifier::new(0);
const SQUARE: TriggerIdentifier = TriggerIdentifier::new(1);
const BRACE: TriggerIdentifier = TriggerIdentifier::new(2);
const STRING: TriggerIdentifier = TriggerIdentifier::new(3);
const LINE_COMMENT: TriggerIdentifier = TriggerIdentifier::new(4);
const BLOCK_COMMENT: TriggerIdentifier = TriggerIdentifier::new(5);
const WHITESPACE: TriggerIdentifier = TriggerIdentifier::new(6);
const STRUCT: CueTerminationRuleIdentifier = CueTerminationRuleIdentifier::new(0);
const ENUM: CueTerminationRuleIdentifier = CueTerminationRuleIdentifier::new(1);

fn rust_word_characters() -> CharacterClass {
    CharacterClass::Characters(CharacterSet::new(
        ('a'..='z').chain('A'..='Z').chain('0'..='9').chain(['_']),
    ))
}

fn profile() -> raw_discovery::SealedTokenProfile {
    TokenProfileData::new(
        ProfileRevision::new(40),
        vec![
            TriggerDefinition {
                identifier: PARENTHESIS,
                trigger: Trigger::Boundary {
                    opening: "(".to_owned(),
                    closing: ")".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: SQUARE,
                trigger: Trigger::Boundary {
                    opening: "[".to_owned(),
                    closing: "]".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: BRACE,
                trigger: Trigger::Boundary {
                    opening: "{".to_owned(),
                    closing: "}".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: STRING,
                trigger: Trigger::Carrier {
                    opening: "\"".to_owned(),
                    closing: "\"".to_owned(),
                    escape: Some("\\".to_owned()),
                },
            },
            TriggerDefinition {
                identifier: LINE_COMMENT,
                trigger: Trigger::LineComment {
                    opening: "//".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: BLOCK_COMMENT,
                trigger: Trigger::Carrier {
                    opening: "/*".to_owned(),
                    closing: "*/".to_owned(),
                    escape: None,
                },
            },
            TriggerDefinition {
                identifier: WHITESPACE,
                trigger: Trigger::Whitespace {
                    canonical_spelling: " ".to_owned(),
                },
            },
        ],
        TriggerSet::new(vec![
            PARENTHESIS,
            SQUARE,
            BRACE,
            STRING,
            LINE_COMMENT,
            BLOCK_COMMENT,
            WHITESPACE,
        ]),
        CharacterSet::from_text(""),
    )
    .seal()
    .expect("Rust pass-1 profile")
}

fn configuration() -> CueTerminatedBlockDiscoveryConfiguration {
    let root = BoundaryDiscoveryContextIdentifier::new(0);
    CueTerminatedBlockDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::new(vec![
                    PARENTHESIS,
                    SQUARE,
                    BRACE,
                    STRING,
                    LINE_COMMENT,
                    BLOCK_COMMENT,
                    WHITESPACE,
                ]),
            )],
            vec![
                BoundaryDiscoveryTransition::new(root, PARENTHESIS, root),
                BoundaryDiscoveryTransition::new(root, SQUARE, root),
                BoundaryDiscoveryTransition::new(root, BRACE, root),
            ],
        ),
        vec![
            CueTerminationRule::new(STRUCT, "struct", ";", rust_word_characters()),
            CueTerminationRule::through_boundary(ENUM, "enum", BRACE, rust_word_characters()),
        ],
    )
}

fn discover(source: &str) -> Result<DiscoveredCueTerminatedBlockTree, BlockDiscoveryError> {
    let profile = profile();
    let configuration = configuration().seal(&profile)?;
    DiscoveredCueTerminatedBlockTree::discover(source, &profile, &configuration)
}

fn text(source: &str, bound: SourceBound) -> &str {
    &source[bound.start()..bound.end()]
}

#[test]
fn struct_is_an_inclusive_cue_with_exact_source_bounds() {
    let source = "const BEFORE: () = (); pub struct CommitSequence(Integer); fn after() {}";
    let tree = discover(source).expect("cue-terminated tree");
    let block = tree.root_blocks().first().expect("one struct block");

    assert_eq!(tree.root_blocks().len(), 1);
    assert_eq!(
        text(source, block.source_bound()),
        "struct CommitSequence(Integer);"
    );
    assert_eq!(text(source, block.cue().bound()), "struct");
    assert_eq!(
        block.cue().evidence(),
        CueTerminatedBlockCueEvidence::CueTermination(STRUCT)
    );
    assert_eq!(
        text(source, block.content_bound()),
        " CommitSequence(Integer)"
    );
    assert_eq!(text(source, block.closing_bound().expect("semicolon")), ";");
    let child = block.children().first().expect("tuple boundary child");
    assert_eq!(text(source, child.source_bound()), "(Integer)");
    assert_eq!(text(source, child.content_bound()), "Integer");
    assert_eq!(
        child.cue().evidence(),
        CueTerminatedBlockCueEvidence::Boundary(PARENTHESIS)
    );
}

#[test]
fn enum_terminates_after_its_balanced_body_boundary() {
    let source = "pub enum Status { Ready, Batch(Vec<u8>), } pub struct After(u8); fn after() {}";
    let tree = discover(source).expect("boundary-terminated enum");
    assert_eq!(tree.root_blocks().len(), 2);

    let enumeration = &tree.root_blocks()[0];
    assert_eq!(
        enumeration.cue().evidence(),
        CueTerminatedBlockCueEvidence::CueTermination(ENUM)
    );
    assert_eq!(
        text(source, enumeration.source_bound()),
        "enum Status { Ready, Batch(Vec<u8>), }"
    );
    assert_eq!(
        text(
            source,
            enumeration.closing_bound().expect("enum body closing")
        ),
        "}"
    );
    let body = enumeration.children().first().expect("enum body");
    assert_eq!(
        body.cue().evidence(),
        CueTerminatedBlockCueEvidence::Boundary(BRACE)
    );
    assert_eq!(
        text(source, body.source_bound()),
        "{ Ready, Batch(Vec<u8>), }"
    );

    assert_eq!(
        text(source, tree.root_blocks()[1].source_bound()),
        "struct After(u8);"
    );
}

#[test]
fn delimiter_children_are_recursive_and_termination_is_top_level_only() {
    let source = "struct Deep((u8, [u16; 2]));";
    let tree = discover(source).expect("recursive tree");
    let block = tree.root_blocks().first().expect("struct");
    let tuple = block.children().first().expect("outer tuple");
    let inner_tuple = tuple.children().first().expect("inner tuple");
    let array = inner_tuple.children().first().expect("array");

    assert_eq!(text(source, tuple.source_bound()), "((u8, [u16; 2]))");
    assert_eq!(text(source, inner_tuple.source_bound()), "(u8, [u16; 2])");
    assert_eq!(text(source, array.source_bound()), "[u16; 2]");
    assert_eq!(
        text(source, block.closing_bound().expect("top-level semicolon")),
        &source[source.len() - 1..]
    );
    assert!(array.source_bound().start() >= inner_tuple.content_bound().start());
    assert!(array.source_bound().end() <= inner_tuple.content_bound().end());
}

#[test]
fn strings_and_comments_are_opaque_to_cues_termination_and_delimiters() {
    let source = concat!(
        "struct Text(\"; ) struct Hidden(Bad);\");\n",
        "// struct Commented(Bad);\n",
        "/* struct BlockCommented([Bad); */\n",
        "struct Actual(/* ; ) */ Integer);"
    );
    let tree = discover(source).expect("opaque strings and comments");

    assert_eq!(tree.root_blocks().len(), 2);
    assert_eq!(
        text(source, tree.root_blocks()[0].source_bound()),
        "struct Text(\"; ) struct Hidden(Bad);\");"
    );
    assert_eq!(
        text(source, tree.root_blocks()[1].source_bound()),
        "struct Actual(/* ; ) */ Integer);"
    );
    assert_eq!(tree.root_blocks()[0].children().len(), 1);
    assert_eq!(tree.root_blocks()[1].children().len(), 1);
}

#[test]
fn cue_matching_requires_a_complete_rust_word() {
    let source = "structural Alias(Integer); a_struct Fake(Integer); struct Real(Integer);";
    let tree = discover(source).expect("complete-word cue");

    assert_eq!(tree.root_blocks().len(), 1);
    assert_eq!(
        text(source, tree.root_blocks()[0].source_bound()),
        "struct Real(Integer);"
    );
}

#[test]
fn missing_termination_and_unclosed_opaque_or_delimited_regions_refuse_typed() {
    assert!(matches!(
        discover("struct Missing(Integer)"),
        Err(BlockDiscoveryError::UnclosedCueTerminatedBlock { rule, cue })
            if rule == STRUCT && cue.start() == 0
    ));
    assert!(matches!(
        discover("struct Mismatched([Integer);"),
        Err(BlockDiscoveryError::Boundary(BoundaryDiscoveryError::Profile(
            TokenProfileError::MismatchedBoundary { expected, found, .. }
        ))) if expected == SQUARE && found == PARENTHESIS
    ));
    assert!(matches!(
        discover("struct String(\"unterminated);"),
        Err(BlockDiscoveryError::Boundary(BoundaryDiscoveryError::Profile(
            TokenProfileError::UnclosedCarrier { identifier, .. }
        ))) if identifier == STRING
    ));
}

#[test]
fn cue_rules_are_archiveable_and_refuse_ambiguous_or_malformed_data() {
    let profile = profile();
    let canonical = configuration();
    let bytes =
        rkyv::to_bytes::<rkyv::rancor::Error>(&canonical).expect("archive cue-termination rules");
    let restored =
        rkyv::from_bytes::<CueTerminatedBlockDiscoveryConfiguration, rkyv::rancor::Error>(&bytes)
            .expect("validate cue-termination rule archive");
    assert_eq!(restored, canonical);
    restored.seal(&profile).expect("restored rules seal");

    let root = BoundaryDiscoveryContextIdentifier::new(1);
    let boundaries = BoundaryDiscoveryConfiguration::new(
        root,
        vec![BoundaryDiscoveryContext::new(
            root,
            TriggerSet::new(vec![PARENTHESIS]),
        )],
        vec![BoundaryDiscoveryTransition::new(root, PARENTHESIS, root)],
    );
    let duplicate = CueTerminatedBlockDiscoveryConfiguration::new(
        boundaries.clone(),
        vec![
            CueTerminationRule::new(STRUCT, "struct", ";", rust_word_characters()),
            CueTerminationRule::new(STRUCT, "record", ";", rust_word_characters()),
        ],
    );
    assert!(matches!(
        duplicate.seal(&profile),
        Err(BlockDiscoveryError::DuplicateCueRule { rule }) if rule == STRUCT
    ));

    let overlapping = CueTerminatedBlockDiscoveryConfiguration::new(
        boundaries,
        vec![CueTerminationRule::new(
            STRUCT,
            "(",
            ";",
            CharacterClass::Characters(CharacterSet::from_text("(")),
        )],
    );
    assert!(matches!(
        overlapping.seal(&profile),
        Err(BlockDiscoveryError::CueRuleTriggerOverlap { rule, trigger })
            if rule == STRUCT && trigger == PARENTHESIS
    ));
}
