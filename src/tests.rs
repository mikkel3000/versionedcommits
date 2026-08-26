use super::*;
use git2::{IndexAddOption, Oid, Repository, Signature, Time};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_REPO_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

macro_rules! parse_commit_tests {
    ($($test_name:ident: { message: $message:expr, expected: $expected:expr $(,)? }),+ $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                assert_eq!(parse_commit_message($message), $expected);
            }
        )+
    };
}

macro_rules! invalid_commit_tests {
    ($($test_name:ident: $message:expr),+ $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                assert_eq!(parse_commit_message($message), Vec::new());
            }
        )+
    };
}

macro_rules! cli_parse_tests {
    ($($test_name:ident: { args: [$($arg:expr),* $(,)?], expected: $expected:expr $(,)? }),+ $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                let args = vec![$($arg.to_string()),*];
                assert_eq!(parse_cli_args(&args), Ok($expected));
            }
        )+
    };
}

macro_rules! cli_reject_tests {
    ($($test_name:ident: [$($arg:expr),* $(,)?]),+ $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                let args = vec![$($arg.to_string()),*];
                assert!(parse_cli_args(&args).is_err());
            }
        )+
    };
}

macro_rules! markdown_tests {
    ($($test_name:ident: { changelog: $changelog:expr, expected: $expected:expr $(,)? }),+ $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                assert_eq!(render_markdown(&$changelog), $expected);
            }
        )+
    };
}

macro_rules! generated_tag_tests {
    ($($test_name:ident: { release: $release:expr, expected: $expected:expr $(,)? }),+ $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                let actual = generated_tags(
                    Version {
                        major: 1,
                        minor: 2,
                        patch: 3,
                    },
                    Some(VersionBump::Minor),
                    &$release,
                    false,
                );

                assert_eq!(actual, $expected);
            }
        )+
    };
}

macro_rules! changelog_json_tests {
    ($($test_name:ident: $case:expr),+ $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                assert_changelog_json($case);
            }
        )+
    };
}

macro_rules! git_tag_tests {
    ($($test_name:ident: $case:expr),+ $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                assert_applies_generated_git_tags($case);
            }
        )+
    };
}

parse_commit_tests! {
    parses_minor_bump_without_changelog_title_or_description: {
        message: "Whatever commit title you want to write\n\
                  \n\
                  Whatever commit you want to write\n\
                  \n\
                  @minor",
        expected: vec![VersionedCommit {
            bump: VersionBump::Minor,
            title: String::new(),
            description: String::new(),
        }],
    },
    parses_minor_bump_with_changelog_title_and_no_description: {
        message: "Whatever title you want to write\n\
                  \n\
                  Whatever description you want to write\n\
                  \n\
                  @minor An optional title for your change",
        expected: vec![VersionedCommit {
            bump: VersionBump::Minor,
            title: "An optional title for your change".to_string(),
            description: String::new(),
        }],
    },
    parses_major_bump_with_changelog_title_and_description: {
        message: "Whatever title you want to write\n\
                  \n\
                  Whatever description you want to write\n\
                  \n\
                  @major An optional title for your change\n\
                  An optional description of your change.\n\
                  Everything written between the two '@major' tags will end up in the changelog.\n\
                  @major",
        expected: vec![VersionedCommit {
            bump: VersionBump::Major,
            title: "An optional title for your change".to_string(),
            description: "An optional description of your change.\n\
                          Everything written between the two '@major' tags will end up in the changelog."
                .to_string(),
        }],
    },
    parses_patch_bump_with_title_and_description: {
        message: "Fix a bug\n\
                  \n\
                  @patch Correct widget ordering\n\
                  Widgets are now sorted before rendering.\n\
                  @patch",
        expected: vec![VersionedCommit {
            bump: VersionBump::Patch,
            title: "Correct widget ordering".to_string(),
            description: "Widgets are now sorted before rendering.".to_string(),
        }],
    },
    parses_major_bump_with_title_and_following_commit_prose: {
        message: "Fix the deployment script\n\
                  \n\
                  @major Rename the public API\n\
                  The commit message continues without adding changelog description.",
        expected: vec![VersionedCommit {
            bump: VersionBump::Major,
            title: "Rename the public API".to_string(),
            description: String::new(),
        }],
    },
}

invalid_commit_tests! {
    ignores_message_without_any_versioned_commit_tag: "Fix the deployment script\n\
                                                       \n\
                                                       This commit has no versioning metadata.",
    ignores_tag_embedded_in_prose: "Fix the deployment script\n\
                                   \n\
                                   This mentions @minor but does not start a metadata line.",
    ignores_unsupported_tag: "Fix the deployment script\n\
                              \n\
                              @breaking Rename the public API",
    ignores_description_closed_by_a_different_tag: "Fix the deployment script\n\
                                                    \n\
                                                    @minor Rename the public API\n\
                                                    The change description is not closed correctly.\n\
                                                    @patch",
}

cli_parse_tests! {
    parses_default_json_format_without_release_metadata: {
        args: [],
        expected: CliOptions::default(),
    },
    parses_explicit_json_format: {
        args: ["--format", "json"],
        expected: CliOptions {
            output_format: OutputFormat::Json,
            ..CliOptions::default()
        },
    },
    parses_markdown_format: {
        args: ["--format", "markdown"],
        expected: CliOptions {
            output_format: OutputFormat::Markdown,
            ..CliOptions::default()
        },
    },
    parses_next_tag_output: {
        args: ["--next-tag"],
        expected: CliOptions {
            output_format: OutputFormat::NextTag,
            ..CliOptions::default()
        },
    },
    parses_output_path: {
        args: ["--output", "release-notes.md"],
        expected: CliOptions {
            output_path: Some("release-notes.md".to_string()),
            ..CliOptions::default()
        },
    },
    parses_apply_generated_tags: {
        args: ["--tag"],
        expected: CliOptions {
            apply_tags: true,
            ..CliOptions::default()
        },
    },
    parses_moving_aliases: {
        args: ["--aliases"],
        expected: CliOptions {
            include_aliases: true,
            ..CliOptions::default()
        },
    },
    parses_commit_changelog_release: {
        args: ["--format", "markdown", "--output", "CHANGELOG.md", "--tag", "--commit"],
        expected: CliOptions {
            output_format: OutputFormat::Markdown,
            output_path: Some("CHANGELOG.md".to_string()),
            apply_tags: true,
            commit_changelog: true,
            ..CliOptions::default()
        },
    },
    parses_prerelease_metadata_only: {
        args: ["--pre", "alpha.1"],
        expected: CliOptions {
            release: ReleaseMetadata {
                prerelease: Some("alpha.1".to_string()),
                build: None,
            },
            ..CliOptions::default()
        },
    },
    parses_build_metadata_only: {
        args: ["--build", "20260612"],
        expected: CliOptions {
            release: ReleaseMetadata {
                prerelease: None,
                build: Some("20260612".to_string()),
            },
            ..CliOptions::default()
        },
    },
    parses_prerelease_and_build_metadata_with_markdown_format: {
        args: ["--pre", "rc.1", "--build", "20260612", "--format", "markdown"],
        expected: CliOptions {
            output_format: OutputFormat::Markdown,
            release: ReleaseMetadata {
                prerelease: Some("rc.1".to_string()),
                build: Some("20260612".to_string()),
            },
            ..CliOptions::default()
        },
    },
}

cli_reject_tests! {
    rejects_missing_prerelease_value: ["--pre"],
    rejects_missing_build_value: ["--build"],
    rejects_missing_format_value: ["--format"],
    rejects_invalid_format_value: ["--format", "xml"],
    rejects_missing_output_path: ["--output"],
    rejects_empty_output_path: ["--output", ""],
    rejects_empty_prerelease_identifier: ["--pre", ""],
    rejects_empty_build_identifier: ["--build", ""],
    rejects_invalid_prerelease_character: ["--pre", "rc_1"],
    rejects_invalid_build_character: ["--build", "2026/06/12"],
    rejects_prerelease_numeric_identifier_with_leading_zero: ["--pre", "rc.01"],
    rejects_commit_without_tag: ["--format", "markdown", "--output", "CHANGELOG.md", "--commit"],
    rejects_commit_without_markdown: ["--output", "CHANGELOG.md", "--tag", "--commit"],
    rejects_commit_without_output: ["--format", "markdown", "--tag", "--commit"],
    rejects_unknown_flag: ["--channel", "alpha"],
}

#[test]
fn prepends_output_to_file_and_creates_parent_directories() {
    let temp_dir = TempRepoDir::new("prepend output to file");
    let output_path = temp_dir.path().join("release").join("notes.md");

    prepend_to_file(output_path.to_str().unwrap(), "old release\n").unwrap();
    prepend_to_file(output_path.to_str().unwrap(), "new release\n").unwrap();

    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        "new release\nold release\n"
    );
}

#[test]
fn renders_only_the_next_immutable_tag() {
    let changelog = Changelog {
        tags: vec!["v1.3.0".to_string(), "v1.3".to_string(), "v1".to_string()],
        ..Changelog::default()
    };

    assert_eq!(render_next_tag(&changelog), "v1.3.0\n");
    assert_eq!(render_next_tag(&Changelog::default()), "");
}

markdown_tests! {
    renders_empty_changelog_without_a_release_heading: {
        changelog: Changelog::default(),
        expected: "",
    },
    renders_keep_a_changelog_style_sections_with_short_shas: {
        changelog: Changelog {
            tags: vec!["v1.3.0".to_string(), "v1.3".to_string(), "v1".to_string()],
            major: vec![],
            minor: vec![Change {
                title: "New dashboard".to_string(),
                description: "Adds account-level filtering.\nImproves report loading.".to_string(),
                sha: "2470bed0afb5b54787d1fd5a7b960de1e30009da".to_string(),
            }],
            patch: vec![Change {
                title: "Dashboard polish".to_string(),
                description: String::new(),
                sha: "3b338e76cebcbc335d8de16f58bb43836ae1c4cd".to_string(),
            }],
        },
        expected: "## [v1.3.0]\n\n### Minor\n\n- New dashboard (`2470bed`)\n\n  Adds account-level filtering.\n  Improves report loading.\n\n### Patch\n\n- Dashboard polish (`3b338e7`)\n",
    },
    renders_keep_a_changelog_change_type_sections_when_titles_start_with_change_types: {
        changelog: Changelog {
            tags: vec!["v1.3.0".to_string(), "v1.3".to_string(), "v1".to_string()],
            major: vec![Change {
                title: "Removed legacy authentication API".to_string(),
                description: String::new(),
                sha: "11111110afb5b54787d1fd5a7b960de1e30009da".to_string(),
            }],
            minor: vec![Change {
                title: "Added project search".to_string(),
                description: "Users can search for projects by name.".to_string(),
                sha: "22222220afb5b54787d1fd5a7b960de1e30009da".to_string(),
            }],
            patch: vec![Change {
                title: "Fixed result ordering".to_string(),
                description: String::new(),
                sha: "33333330afb5b54787d1fd5a7b960de1e30009da".to_string(),
            }],
        },
        expected: "## [v1.3.0]\n\n### Added\n\n- project search (`2222222`)\n\n  Users can search for projects by name.\n\n### Removed\n\n- legacy authentication API (`1111111`)\n\n### Fixed\n\n- result ordering (`3333333`)\n",
    },
    renders_changelog_title_without_untitled_fallback: {
        changelog: Changelog {
            tags: vec!["v0.0.1".to_string(), "v0.0".to_string(), "v0".to_string()],
            major: vec![],
            minor: vec![],
            patch: vec![Change {
                title: "Add table driven tests for parse_commit_message".to_string(),
                description: String::new(),
                sha: "2470bed0afb5b54787d1fd5a7b960de1e30009da".to_string(),
            }],
        },
        expected: "## [v0.0.1]\n\n### Patch\n\n- Add table driven tests for parse_commit_message (`2470bed`)\n",
    },
}

generated_tag_tests! {
    generates_stable_release_tag: {
        release: ReleaseMetadata::default(),
        expected: vec!["v1.3.0".to_string()],
    },
    generates_prerelease_only_on_full_tag: {
        release: ReleaseMetadata {
            prerelease: Some("alpha.1".to_string()),
            build: None,
        },
        expected: vec!["v1.3.0-alpha.1".to_string()],
    },
    generates_build_metadata_only_on_full_tag: {
        release: ReleaseMetadata {
            prerelease: None,
            build: Some("20260612".to_string()),
        },
        expected: vec!["v1.3.0+20260612".to_string()],
    },
    generates_prerelease_and_build_metadata_only_on_full_tag: {
        release: ReleaseMetadata {
            prerelease: Some("rc.1".to_string()),
            build: Some("20260612".to_string()),
        },
        expected: vec!["v1.3.0-rc.1+20260612".to_string()],
    },
}

#[test]
fn generates_moving_aliases_when_requested() {
    let tags = generated_tags(
        Version {
            major: 1,
            minor: 2,
            patch: 3,
        },
        Some(VersionBump::Minor),
        &ReleaseMetadata::default(),
        true,
    );

    assert_eq!(tags, vec!["v1.3.0", "v1.3", "v1"]);
}

changelog_json_tests! {
    builds_patch_tag_from_zero_base_when_no_semantic_tag_exists: ChangelogCase {
        commits: vec![CommitSpec {
            message: "Add table driven tests for parse_commit_message\n\n@patch",
            tag_after: None,
        }],
        release: ReleaseMetadata::default(),
        expected_tags: vec!["v0.0.1"],
        expected_major: vec![],
        expected_minor: vec![],
        expected_patch: vec![ExpectedChange {
            commit_index: 0,
            title: "Add table driven tests for parse_commit_message",
            description: "",
        }],
    },
    uses_commit_title_and_description_when_version_metadata_omits_both: ChangelogCase {
        commits: vec![CommitSpec {
            message: "Fix CLI output\n\nUse stable stdout formatting for release metadata.\n\n@patch",
            tag_after: None,
        }],
        release: ReleaseMetadata::default(),
        expected_tags: vec!["v0.0.1"],
        expected_major: vec![],
        expected_minor: vec![],
        expected_patch: vec![ExpectedChange {
            commit_index: 0,
            title: "Fix CLI output",
            description: "Use stable stdout formatting for release metadata.",
        }],
    },
    uses_empty_description_when_version_metadata_only_has_title: ChangelogCase {
        commits: vec![CommitSpec {
            message: "Fix CLI output\n\nUse stable stdout formatting for release metadata.\n\n@patch Stable stdout formatting",
            tag_after: None,
        }],
        release: ReleaseMetadata::default(),
        expected_tags: vec!["v0.0.1"],
        expected_major: vec![],
        expected_minor: vec![],
        expected_patch: vec![ExpectedChange {
            commit_index: 0,
            title: "Stable stdout formatting",
            description: "",
        }],
    },
    builds_from_commits_after_latest_semantic_tag_and_ignores_older_versioned_commits: ChangelogCase {
        commits: vec![
            CommitSpec {
                message: "Release old API\n\n@major Old public API",
                tag_after: Some("v1.2.3"),
            },
            CommitSpec {
                message: "Add a feature\n\n@minor New dashboard",
                tag_after: None,
            },
            CommitSpec {
                message: "Fix the feature\n\n@patch Dashboard polish",
                tag_after: None,
            },
        ],
        release: ReleaseMetadata::default(),
        expected_tags: vec!["v1.3.0"],
        expected_major: vec![],
        expected_minor: vec![ExpectedChange {
            commit_index: 1,
            title: "New dashboard",
            description: "",
        }],
        expected_patch: vec![ExpectedChange {
            commit_index: 2,
            title: "Dashboard polish",
            description: "",
        }],
    },
    builds_major_bump_when_major_minor_and_patch_commits_exist_since_latest_tag: ChangelogCase {
        commits: vec![
            CommitSpec {
                message: "Release current API",
                tag_after: Some("v2.4.9"),
            },
            CommitSpec {
                message: "Fix existing behavior\n\n@patch Compatibility fix",
                tag_after: None,
            },
            CommitSpec {
                message: "Replace API\n\n@major Rename the public API\nDocument the replacement path.\n@major",
                tag_after: None,
            },
        ],
        release: ReleaseMetadata::default(),
        expected_tags: vec!["v3.0.0"],
        expected_major: vec![ExpectedChange {
            commit_index: 2,
            title: "Rename the public API",
            description: "Document the replacement path.",
        }],
        expected_minor: vec![],
        expected_patch: vec![ExpectedChange {
            commit_index: 1,
            title: "Compatibility fix",
            description: "",
        }],
    },
    builds_empty_output_when_no_commits_after_latest_tag_contain_version_metadata: ChangelogCase {
        commits: vec![
            CommitSpec {
                message: "Release current CLI",
                tag_after: Some("v1.0.0"),
            },
            CommitSpec {
                message: "Refactor internal helpers",
                tag_after: None,
            },
        ],
        release: ReleaseMetadata::default(),
        expected_tags: vec![],
        expected_major: vec![],
        expected_minor: vec![],
        expected_patch: vec![],
    },
    builds_prerelease_and_build_metadata_only_on_generated_full_tag: ChangelogCase {
        commits: vec![
            CommitSpec {
                message: "Release current CLI",
                tag_after: Some("v1.2.3"),
            },
            CommitSpec {
                message: "Add beta feature\n\n@minor Public beta",
                tag_after: None,
            },
        ],
        release: ReleaseMetadata {
            prerelease: Some("rc.1".to_string()),
            build: Some("20260612".to_string()),
        },
        expected_tags: vec!["v1.3.0-rc.1+20260612"],
        expected_major: vec![],
        expected_minor: vec![ExpectedChange {
            commit_index: 1,
            title: "Public beta",
            description: "",
        }],
        expected_patch: vec![],
    },
    builds_from_stable_release_base_when_existing_prerelease_tags_exist: ChangelogCase {
        commits: vec![
            CommitSpec {
                message: "Release current CLI",
                tag_after: Some("v1.2.3"),
            },
            CommitSpec {
                message: "Prepare release candidate",
                tag_after: Some("v1.3.0-rc.1"),
            },
            CommitSpec {
                message: "Fix release candidate\n\n@patch Candidate fix",
                tag_after: None,
            },
        ],
        release: ReleaseMetadata::default(),
        expected_tags: vec!["v1.2.4"],
        expected_major: vec![],
        expected_minor: vec![],
        expected_patch: vec![ExpectedChange {
            commit_index: 2,
            title: "Candidate fix",
            description: "",
        }],
    },
    ignores_stable_tags_with_leading_zeroes: ChangelogCase {
        commits: vec![
            CommitSpec {
                message: "Release invalid version",
                tag_after: Some("v01.2.3"),
            },
            CommitSpec {
                message: "Fix release metadata\n\n@patch Metadata fix",
                tag_after: None,
            },
        ],
        release: ReleaseMetadata::default(),
        expected_tags: vec!["v0.0.1"],
        expected_major: vec![],
        expected_minor: vec![],
        expected_patch: vec![ExpectedChange {
            commit_index: 1,
            title: "Metadata fix",
            description: "",
        }],
    },
}

git_tag_tests! {
    applies_full_tag_and_moving_aliases: TagCase {
        commits: vec![
            CommitSpec {
                message: "Release current CLI",
                tag_after: Some("v1.0.0"),
            },
            CommitSpec {
                message: "Fix CLI output\n\n@patch CLI output fix",
                tag_after: None,
            },
        ],
        release: ReleaseMetadata::default(),
        include_aliases: true,
        existing_aliases: vec![],
        expected_created_tags: vec!["v1.0.1", "v1.0", "v1"],
        expect_error: false,
    },
    updates_existing_major_and_minor_aliases: TagCase {
        commits: vec![
            CommitSpec {
                message: "Release current CLI",
                tag_after: Some("v1.0.0"),
            },
            CommitSpec {
                message: "Fix CLI output\n\n@patch CLI output fix",
                tag_after: None,
            },
        ],
        release: ReleaseMetadata::default(),
        include_aliases: true,
        existing_aliases: vec!["v1.0", "v1"],
        expected_created_tags: vec!["v1.0.1", "v1.0", "v1"],
        expect_error: false,
    },
    applies_no_tags_when_no_generated_tags_exist: TagCase {
        commits: vec![CommitSpec {
            message: "Refactor internal helpers",
            tag_after: None,
        }],
        release: ReleaseMetadata::default(),
        include_aliases: false,
        existing_aliases: vec![],
        expected_created_tags: vec![],
        expect_error: false,
    },
}

#[test]
fn does_not_overwrite_existing_full_version_tag() {
    let repo_dir = TempRepoDir::new("does not overwrite existing full version tag");
    let repo = Repository::init(repo_dir.path()).unwrap();
    let oid = create_commit(&repo, 0, "Release current CLI");
    let object = repo.find_object(oid, None).unwrap();
    repo.tag_lightweight("v1.0.1", &object, false).unwrap();

    let changelog = Changelog {
        tags: vec!["v1.0.1".to_string(), "v1.0".to_string(), "v1".to_string()],
        ..Changelog::default()
    };

    assert!(apply_generated_tags(&repo, &changelog).is_err());
}

#[test]
fn commits_changelog_before_applying_generated_tags() {
    let repo_dir = TempRepoDir::new("commit changelog before tags");
    let repo = Repository::init(repo_dir.path()).unwrap();
    configure_test_signature(&repo);
    create_commit(
        &repo,
        0,
        "Add project search\n\n@minor Added project search",
    );

    let changelog = build_changelog(&repo, &ReleaseMetadata::default(), false).unwrap();
    let markdown = render_markdown(&changelog);
    let output_path = repo_dir.path().join("CHANGELOG.md");

    ensure_changelog_commit_is_safe(&repo, &changelog).unwrap();
    prepend_to_file(output_path.to_str().unwrap(), &markdown).unwrap();
    let commit_oid = commit_changelog(
        &repo,
        output_path.to_str().unwrap(),
        changelog.tags.first().unwrap(),
    )
    .unwrap();
    apply_generated_tags(&repo, &changelog).unwrap();

    let commit = repo.find_commit(commit_oid).unwrap();
    assert_eq!(
        commit.message().unwrap(),
        "Update the changelog for version v0.1.0"
    );
    assert_eq!(repo.head().unwrap().target(), Some(commit_oid));
    assert_eq!(tag_target(&repo, "v0.1.0"), Some(commit_oid));
    assert_eq!(tag_target(&repo, "v0.1"), None);
    assert_eq!(tag_target(&repo, "v0"), None);

    let changelog_entry = commit
        .tree()
        .unwrap()
        .get_path(Path::new("CHANGELOG.md"))
        .unwrap();
    let changelog_blob = repo.find_blob(changelog_entry.id()).unwrap();
    assert_eq!(changelog_blob.content(), markdown.as_bytes());
}

#[test]
fn rejects_changelog_commit_when_other_changes_are_staged() {
    let repo_dir = TempRepoDir::new("reject staged changes");
    let repo = Repository::init(repo_dir.path()).unwrap();
    create_commit(
        &repo,
        0,
        "Add project search\n\n@minor Added project search",
    );
    fs::write(repo_dir.path().join("staged.txt"), "unrelated change\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("staged.txt")).unwrap();
    index.write().unwrap();
    let changelog = build_changelog(&repo, &ReleaseMetadata::default(), false).unwrap();

    assert!(ensure_changelog_commit_is_safe(&repo, &changelog).is_err());
}

struct CommitSpec {
    message: &'static str,
    tag_after: Option<&'static str>,
}

struct ExpectedChange {
    commit_index: usize,
    title: &'static str,
    description: &'static str,
}

struct ChangelogCase {
    commits: Vec<CommitSpec>,
    release: ReleaseMetadata,
    expected_tags: Vec<&'static str>,
    expected_major: Vec<ExpectedChange>,
    expected_minor: Vec<ExpectedChange>,
    expected_patch: Vec<ExpectedChange>,
}

struct TagCase {
    commits: Vec<CommitSpec>,
    release: ReleaseMetadata,
    include_aliases: bool,
    existing_aliases: Vec<&'static str>,
    expected_created_tags: Vec<&'static str>,
    expect_error: bool,
}

fn assert_changelog_json(case: ChangelogCase) {
    let repo_dir = TempRepoDir::new("changelog json test");
    let repo = Repository::init(repo_dir.path()).unwrap();
    let commit_ids = create_commits(&repo, &case.commits);
    let changelog = build_changelog(&repo, &case.release, false).unwrap();
    let actual = serde_json::to_value(&changelog).unwrap();
    let expected = expected_json(
        &case.expected_tags,
        &expected_changes(&commit_ids, &case.expected_major),
        &expected_changes(&commit_ids, &case.expected_minor),
        &expected_changes(&commit_ids, &case.expected_patch),
    );

    assert_eq!(actual, expected);
}

fn assert_applies_generated_git_tags(case: TagCase) {
    let repo_dir = TempRepoDir::new("git tag test");
    let repo = Repository::init(repo_dir.path()).unwrap();
    let commit_ids = create_commits(&repo, &case.commits);

    for alias in &case.existing_aliases {
        let object = repo.find_object(commit_ids[0], None).unwrap();
        repo.tag_lightweight(alias, &object, false).unwrap();
    }

    let changelog = build_changelog(&repo, &case.release, case.include_aliases).unwrap();
    let result = apply_generated_tags(&repo, &changelog);

    assert_eq!(result.is_err(), case.expect_error);

    if case.expect_error {
        return;
    }

    let head = repo.head().unwrap().target().unwrap();

    for tag in &case.expected_created_tags {
        assert_eq!(tag_target(&repo, tag), Some(head), "{tag}");
    }
}

fn create_commits(repo: &Repository, commits: &[CommitSpec]) -> Vec<Oid> {
    let mut commit_ids = Vec::new();

    for (index, commit) in commits.iter().enumerate() {
        let oid = create_commit(repo, index, commit.message);
        commit_ids.push(oid);

        if let Some(tag) = commit.tag_after {
            let object = repo.find_object(oid, None).unwrap();
            repo.tag_lightweight(tag, &object, false).unwrap();
        }
    }

    commit_ids
}

fn create_commit(repo: &Repository, index: usize, message: &str) -> Oid {
    let workdir = repo.workdir().unwrap();
    fs::write(workdir.join("content.txt"), format!("commit {index}\n")).unwrap();

    let mut index_file = repo.index().unwrap();
    index_file
        .add_all(["content.txt"], IndexAddOption::DEFAULT, None)
        .unwrap();
    let tree_oid = index_file.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let signature = Signature::new(
        "Versioned Commits",
        "versioned-commits@example.com",
        &Time::new(1_700_000_000 + index as i64, 0),
    )
    .unwrap();

    let parent = repo
        .head()
        .ok()
        .and_then(|head| head.target())
        .map(|oid| repo.find_commit(oid).unwrap());
    let parents = parent.iter().collect::<Vec<_>>();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )
    .unwrap()
}

fn configure_test_signature(repo: &Repository) {
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Versioned Commits").unwrap();
    config
        .set_str("user.email", "versioned-commits@example.com")
        .unwrap();
}

fn tag_target(repo: &Repository, tag: &str) -> Option<Oid> {
    repo.revparse_single(&format!("refs/tags/{tag}"))
        .ok()
        .map(|object| object.id())
}

fn expected_changes<'a>(
    commit_ids: &[Oid],
    changes: &'a [ExpectedChange],
) -> Vec<RenderedChange<'a>> {
    changes
        .iter()
        .map(|change| RenderedChange {
            title: change.title,
            description: change.description,
            sha: commit_ids[change.commit_index].to_string(),
        })
        .collect()
}

struct RenderedChange<'a> {
    title: &'a str,
    description: &'a str,
    sha: String,
}

fn expected_json(
    tags: &[&str],
    major: &[RenderedChange],
    minor: &[RenderedChange],
    patch: &[RenderedChange],
) -> serde_json::Value {
    json!({
        "tags": tags,
        "major": expected_change_values(major),
        "minor": expected_change_values(minor),
        "patch": expected_change_values(patch),
    })
}

fn expected_change_values(changes: &[RenderedChange]) -> Vec<serde_json::Value> {
    changes
        .iter()
        .map(|change| {
            json!({
                "title": change.title,
                "description": change.description,
                "sha": change.sha,
            })
        })
        .collect()
}

struct TempRepoDir {
    path: PathBuf,
}

impl TempRepoDir {
    fn new(name: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = TEMP_REPO_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "versionedcommits-{}-{}-{timestamp}-{sequence}",
            std::process::id(),
            name.replace(' ', "-")
        ));

        fs::create_dir(&path).unwrap();

        Self { path }
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempRepoDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
