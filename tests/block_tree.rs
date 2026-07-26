//! Runtime block-tree witnesses. These assertions cover only source-bounded
//! boundary discovery; typed parsing and archived `Block` compatibility stay
//! outside this surface.

use raw_discovery::{
    BlockDiscoveryError, BlockPrefixAttachment, BlockPrefixRule, BlockTree,
    BlockTreeDiscoveryConfiguration, BoundaryDiscoveryConfiguration, BoundaryDiscoveryContext,
    BoundaryDiscoveryContextIdentifier, BoundaryDiscoveryError, BoundaryDiscoveryTransition,
    CharacterClass, CharacterSet, ProfileRevision, RawProfile, TokenProfileData, TokenProfileError,
    Trigger, TriggerDefinition, TriggerIdentifier, TriggerSet,
};

const PARENTHESIS: TriggerIdentifier = TriggerIdentifier::new(0);
const SQUARE: TriggerIdentifier = TriggerIdentifier::new(1);
const BRACE: TriggerIdentifier = TriggerIdentifier::new(2);
const PIPE_TEXT: TriggerIdentifier = TriggerIdentifier::new(4);
const WHITESPACE: TriggerIdentifier = TriggerIdentifier::new(5);
const COMMENT: TriggerIdentifier = TriggerIdentifier::new(6);

fn context(value: u16) -> BoundaryDiscoveryContextIdentifier {
    BoundaryDiscoveryContextIdentifier::new(value)
}

fn protos_configuration() -> BlockTreeDiscoveryConfiguration {
    let root = context(0);
    let child = context(1);
    let active = TriggerSet::new(vec![
        PARENTHESIS,
        SQUARE,
        BRACE,
        PIPE_TEXT,
        WHITESPACE,
        COMMENT,
    ]);
    let boundaries = BoundaryDiscoveryConfiguration::new(
        root,
        vec![
            BoundaryDiscoveryContext::new(root, active.clone()),
            BoundaryDiscoveryContext::new(child, active),
        ],
        vec![
            BoundaryDiscoveryTransition::new(root, PARENTHESIS, child),
            BoundaryDiscoveryTransition::new(root, SQUARE, child),
            BoundaryDiscoveryTransition::new(root, BRACE, child),
            BoundaryDiscoveryTransition::new(child, PARENTHESIS, child),
            BoundaryDiscoveryTransition::new(child, SQUARE, child),
            BoundaryDiscoveryTransition::new(child, BRACE, child),
        ],
    );
    let prefix = BlockPrefixRule::new(
        ".",
        CharacterClass::Characters(CharacterSet::from_text(
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-",
        )),
    );
    BlockTreeDiscoveryConfiguration::new(
        boundaries,
        vec![
            BlockPrefixAttachment::new(PARENTHESIS, prefix.clone()),
            BlockPrefixAttachment::new(SQUARE, prefix.clone()),
            BlockPrefixAttachment::new(BRACE, prefix),
        ],
    )
}

fn discover(source: &str) -> raw_discovery::DiscoveredBlockTree {
    let profile = RawProfile::standard().seal().expect("standard profile");
    raw_discovery::DiscoveredBlockTree::discover(source, &profile, &protos_configuration())
        .expect("block tree")
}

fn text(source: &str, bound: raw_discovery::SourceBound) -> &str {
    &source[bound.start()..bound.end()]
}

fn projections<T: BlockTree>(block: &T) {
    let _ = block.source_bound();
    let _ = block.cue();
    let _ = block.prefix();
    let _ = block.content_bound();
    let _ = block.closing_bound();
    let _ = block.children();
}

#[test]
fn nested_blocks_keep_bounds_prefixes_and_source_order() {
    let source = "Public.Newtype.( CommitSequence [ rkyv.Archive ] { field.(Type) } )";
    let tree = discover(source);
    let root = tree.root_blocks().first().expect("one root");
    projections(root);

    assert_eq!(tree.root_blocks().len(), 1);
    assert_eq!(
        text(source, root.source_bound()),
        "Newtype.( CommitSequence [ rkyv.Archive ] { field.(Type) } )"
    );
    let prefix = root.prefix().expect("dotted prefix");
    assert_eq!(text(source, prefix.word()), "Newtype");
    assert_eq!(text(source, prefix.separator()), ".");
    assert_eq!(text(source, root.cue().bound()), "(");
    assert_eq!(root.cue().evidence(), PARENTHESIS);
    assert_eq!(
        text(source, root.content_bound()),
        " CommitSequence [ rkyv.Archive ] { field.(Type) } "
    );
    assert_eq!(text(source, root.closing_bound().expect("closing")), ")");

    let children = root.children();
    assert_eq!(children.len(), 2);
    assert_eq!(text(source, children[0].source_bound()), "[ rkyv.Archive ]");
    assert_eq!(text(source, children[1].source_bound()), "{ field.(Type) }");
    let field = children[1].children().first().expect("field block");
    assert_eq!(text(source, field.source_bound()), "field.(Type)");
    assert_eq!(
        text(source, field.prefix().expect("field prefix").word()),
        "field"
    );
    assert_eq!(text(source, field.content_bound()), "Type");
}

#[test]
fn children_are_isolated_to_their_parent_content_bound() {
    let source = "(alpha [beta]) tail [later]";
    let tree = discover(source);
    let roots = tree.root_blocks();
    assert_eq!(roots.len(), 2);
    let parent = &roots[0];
    let child = parent.children().first().expect("nested child");

    assert_eq!(text(source, parent.source_bound()), "(alpha [beta])");
    assert_eq!(text(source, child.source_bound()), "[beta]");
    assert!(child.source_bound().start() >= parent.content_bound().start());
    assert!(child.source_bound().end() <= parent.content_bound().end());
    assert_eq!(text(source, roots[1].source_bound()), "[later]");
}

#[test]
fn carriers_and_comments_remain_opaque_to_block_discovery() {
    let source = "( before (| ] { still carrier |) ;; [not a child]\n [actual] )";
    let tree = discover(source);
    let root = tree.root_blocks().first().expect("root");

    assert_eq!(root.children().len(), 1);
    assert_eq!(text(source, root.children()[0].source_bound()), "[actual]");
}

#[test]
fn mismatched_and_unclosed_boundaries_refuse_before_child_discovery() {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let configuration = protos_configuration();

    assert!(matches!(
        raw_discovery::DiscoveredBlockTree::discover("(outer[inner) ]", &profile, &configuration),
        Err(BlockDiscoveryError::Boundary(BoundaryDiscoveryError::Profile(
            TokenProfileError::MismatchedBoundary { expected, found, .. }
        ))) if expected == SQUARE && found == PARENTHESIS
    ));
    assert!(matches!(
        raw_discovery::DiscoveredBlockTree::discover("(outer[inner]", &profile, &configuration),
        Err(BlockDiscoveryError::Boundary(BoundaryDiscoveryError::Profile(
            TokenProfileError::UnclosedBoundary { identifier, byte_offset: 0 }
        ))) if identifier == PARENTHESIS
    ));
}

#[test]
fn every_boundary_family_admitted_by_its_context_is_discovered() {
    let angle = TriggerIdentifier::new(11);
    let guillemet = TriggerIdentifier::new(12);
    let profile = TokenProfileData::new(
        ProfileRevision::new(29),
        vec![
            TriggerDefinition {
                identifier: angle,
                trigger: Trigger::Boundary {
                    opening: "<".to_owned(),
                    closing: ">".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: guillemet,
                trigger: Trigger::Boundary {
                    opening: "«".to_owned(),
                    closing: "»".to_owned(),
                },
            },
        ],
        TriggerSet::new(vec![angle, guillemet]),
        CharacterSet::from_text(""),
    )
    .seal()
    .expect("custom boundary profile");
    let root = context(10);
    let configuration = BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::new(vec![angle, guillemet]),
            )],
            vec![
                BoundaryDiscoveryTransition::new(root, angle, root),
                BoundaryDiscoveryTransition::new(root, guillemet, root),
            ],
        ),
        Vec::new(),
    );
    let source = "<outer «inner»>";
    let tree = raw_discovery::DiscoveredBlockTree::discover(source, &profile, &configuration)
        .expect("tree");
    let outer = tree.root_blocks().first().expect("angle root");
    let inner = outer.children().first().expect("guillemet child");

    assert_eq!(outer.cue().evidence(), angle);
    assert_eq!(inner.cue().evidence(), guillemet);
    assert_eq!(text(source, outer.source_bound()), source);
    assert_eq!(text(source, inner.source_bound()), "«inner»");
}

#[test]
fn child_contexts_activate_non_root_declared_boundaries() {
    let outer = TriggerIdentifier::new(30);
    let inner = TriggerIdentifier::new(31);
    let profile = TokenProfileData::new(
        ProfileRevision::new(31),
        vec![
            TriggerDefinition {
                identifier: outer,
                trigger: Trigger::Boundary {
                    opening: "<".to_owned(),
                    closing: ">".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: inner,
                trigger: Trigger::Boundary {
                    opening: "[".to_owned(),
                    closing: "]".to_owned(),
                },
            },
        ],
        TriggerSet::new(vec![outer]),
        CharacterSet::from_text(""),
    )
    .seal()
    .expect("profile with a non-root boundary declaration");
    let root = context(30);
    let child = context(31);
    let leaf = context(32);
    let configuration = BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            root,
            vec![
                BoundaryDiscoveryContext::new(root, TriggerSet::new(vec![outer])),
                BoundaryDiscoveryContext::new(child, TriggerSet::new(vec![inner])),
                BoundaryDiscoveryContext::new(leaf, TriggerSet::new(Vec::new())),
            ],
            vec![
                BoundaryDiscoveryTransition::new(root, outer, child),
                BoundaryDiscoveryTransition::new(child, inner, leaf),
            ],
        ),
        Vec::new(),
    );

    let tree = raw_discovery::DiscoveredBlockTree::discover("<[inner]>", &profile, &configuration)
        .expect("child context activates its declared boundary");
    let outer_block = tree.root_blocks().first().expect("outer block");
    assert_eq!(outer_block.cue().evidence(), outer);
    assert_eq!(outer_block.children().len(), 1);
    assert_eq!(outer_block.children()[0].cue().evidence(), inner);
}

#[test]
fn a_declared_child_boundary_cannot_be_silently_ignored() {
    let profile = RawProfile::standard().seal().expect("standard profile");
    let root = context(33);
    let child = context(34);
    let configuration = BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            root,
            vec![
                BoundaryDiscoveryContext::new(root, TriggerSet::new(vec![PARENTHESIS])),
                BoundaryDiscoveryContext::new(child, TriggerSet::new(vec![SQUARE])),
            ],
            vec![BoundaryDiscoveryTransition::new(root, PARENTHESIS, child)],
        ),
        Vec::new(),
    );

    assert!(matches!(
        raw_discovery::DiscoveredBlockTree::discover("([inner])", &profile, &configuration),
        Err(BlockDiscoveryError::Boundary(BoundaryDiscoveryError::MissingChildContext {
            context,
            boundary,
        })) if context == child && boundary == SQUARE
    ));
}

#[test]
fn alternate_prefix_alphabet_is_runtime_rule_data() {
    let boundary = TriggerIdentifier::new(40);
    let profile = TokenProfileData::new(
        ProfileRevision::new(32),
        vec![TriggerDefinition {
            identifier: boundary,
            trigger: Trigger::Boundary {
                opening: "(".to_owned(),
                closing: ")".to_owned(),
            },
        }],
        TriggerSet::new(vec![boundary]),
        CharacterSet::from_text(""),
    )
    .seal()
    .expect("custom profile");
    let root = context(40);
    let configuration = BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::new(vec![boundary]),
            )],
            vec![BoundaryDiscoveryTransition::new(root, boundary, root)],
        ),
        vec![BlockPrefixAttachment::new(
            boundary,
            BlockPrefixRule::new(
                "::",
                CharacterClass::Characters(CharacterSet::from_text("αβ")),
            ),
        )],
    );
    let source = "α::(body)";
    let tree = raw_discovery::DiscoveredBlockTree::discover(source, &profile, &configuration)
        .expect("alternate prefix rule");
    let block = tree.root_blocks().first().expect("block");
    let prefix = block.prefix().expect("configured prefix");

    assert_eq!(text(source, block.source_bound()), source);
    assert_eq!(text(source, prefix.word()), "α");
    assert_eq!(text(source, prefix.separator()), "::");
}

#[test]
fn horizontal_application_punctuation_and_token_triggers_do_not_enter_discovery() {
    let boundary = TriggerIdentifier::new(50);
    let application = TriggerIdentifier::new(51);
    let punctuation = TriggerIdentifier::new(52);
    let token = TriggerIdentifier::new(53);
    let profile = TokenProfileData::new(
        ProfileRevision::new(33),
        vec![
            TriggerDefinition {
                identifier: boundary,
                trigger: Trigger::Boundary {
                    opening: "<".to_owned(),
                    closing: ">".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: application,
                trigger: Trigger::Application {
                    glyph: ".".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: punctuation,
                trigger: Trigger::Punctuation {
                    glyph: ">>".to_owned(),
                },
            },
            TriggerDefinition {
                identifier: token,
                trigger: Trigger::LeadingCharacterClass {
                    leading: CharacterClass::AsciiAlphabetic,
                    continuation: CharacterClass::AsciiAlphanumeric,
                },
            },
        ],
        TriggerSet::new(vec![boundary, application, punctuation, token]),
        CharacterSet::from_text(""),
    )
    .seal()
    .expect("profile with horizontal triggers");

    assert!(matches!(
        profile.seal_boundary_discovery_set(TriggerSet::new(vec![
            boundary,
            application,
            punctuation,
            token,
        ])),
        Err(TokenProfileError::UnsupportedBoundaryDiscoveryTrigger(identifier))
            if identifier == application
    ));

    let root = context(50);
    let configuration = BlockTreeDiscoveryConfiguration::new(
        BoundaryDiscoveryConfiguration::new(
            root,
            vec![BoundaryDiscoveryContext::new(
                root,
                TriggerSet::new(vec![boundary]),
            )],
            vec![BoundaryDiscoveryTransition::new(root, boundary, root)],
        ),
        Vec::new(),
    );
    let tree = raw_discovery::DiscoveredBlockTree::discover("<<inner>>", &profile, &configuration)
        .expect("two closing boundaries still balance");
    let outer = tree.root_blocks().first().expect("outer block");
    assert_eq!(outer.children().len(), 1);
    assert_eq!(outer.children()[0].cue().evidence(), boundary);
}
