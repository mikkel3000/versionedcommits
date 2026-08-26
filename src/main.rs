use git2::{Oid, Repository, Sort};
use serde::Serialize;
use std::{env, error::Error, fs, io, path::Path};

mod cmd;
mod markdown;
#[cfg(test)]
pub(crate) use cmd::CliOptions;
pub(crate) use cmd::{OutputFormat, ReleaseMetadata, parse_cli_args};
pub(crate) use markdown::render_markdown;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum VersionBump {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VersionedCommit {
    bump: VersionBump,
    title: String,
    description: String,
}

#[derive(Debug, Serialize)]
struct Change {
    title: String,
    description: String,
    sha: String,
}

#[derive(Debug, Default, Serialize)]
struct Changelog {
    tags: Vec<String>,
    major: Vec<Change>,
    minor: Vec<Change>,
    patch: Vec<Change>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    major: usize,
    minor: usize,
    patch: usize,
}

fn parse_commit_message(message: &str) -> Vec<VersionedCommit> {
    let lines: Vec<&str> = message.lines().collect();
    let mut parsed = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let Some((hint, title)) = parse_version_hint(lines[index]) else {
            index += 1;
            continue;
        };

        match find_next_version_hint(&lines, index + 1) {
            None => {
                parsed.push(VersionedCommit {
                    bump: hint,
                    title,
                    description: String::new(),
                });
                index += 1;
            }
            Some((closing_index, closing_hint)) if closing_hint == hint => {
                let description = lines[index + 1..closing_index].join("\n");
                parsed.push(VersionedCommit {
                    bump: hint,
                    title,
                    description,
                });
                index = closing_index + 1;
            }
            Some((next_hint_index, _)) => {
                index = next_hint_index + 1;
            }
        }
    }

    parsed
}

fn parse_version_hint(line: &str) -> Option<(VersionBump, String)> {
    for (marker, hint) in [
        ("@major", VersionBump::Major),
        ("@minor", VersionBump::Minor),
        ("@patch", VersionBump::Patch),
    ] {
        let Some(rest) = line.strip_prefix(marker) else {
            continue;
        };

        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            return Some((hint, rest.trim().to_string()));
        }
    }

    None
}

fn find_next_version_hint(lines: &[&str], start: usize) -> Option<(usize, VersionBump)> {
    lines[start..]
        .iter()
        .enumerate()
        .find_map(|(offset, line)| parse_version_hint(line).map(|(hint, _)| (start + offset, hint)))
}

fn build_changelog(
    repo: &Repository,
    release: &ReleaseMetadata,
    include_aliases: bool,
) -> Result<Changelog, git2::Error> {
    let latest_tag = find_latest_semver_tag(repo)?;
    let base_version = latest_tag.map(|(version, _)| version).unwrap_or(Version {
        major: 0,
        minor: 0,
        patch: 0,
    });
    let mut changelog = Changelog::default();

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    if let Some((_, tagged_commit)) = latest_tag {
        revwalk.hide(tagged_commit)?;
    }
    revwalk.set_sorting(Sort::TIME)?;

    for commit in revwalk.map(|object_id| repo.find_commit(object_id?)) {
        let commit = commit?;
        let message = commit.message().unwrap_or_default();
        let commit_message = message_before_first_version_hint(message).trim();
        let (commit_title, commit_description) = commit_message
            .split_once('\n')
            .map(|(title, description)| (title.trim(), description.trim()))
            .unwrap_or((commit_message, ""));
        let sha = commit.id().to_string();

        for parsed in parse_commit_message(message) {
            let use_commit_message = parsed.title.is_empty() && parsed.description.is_empty();
            let change = Change {
                title: if use_commit_message {
                    commit_title.to_string()
                } else {
                    parsed.title
                },
                description: if use_commit_message {
                    commit_description.to_string()
                } else {
                    parsed.description
                },
                sha: sha.clone(),
            };

            match parsed.bump {
                VersionBump::Major => changelog.major.push(change),
                VersionBump::Minor => changelog.minor.push(change),
                VersionBump::Patch => changelog.patch.push(change),
            }
        }
    }

    let bump = match (
        changelog.major.is_empty(),
        changelog.minor.is_empty(),
        changelog.patch.is_empty(),
    ) {
        (false, _, _) => Some(VersionBump::Major),
        (_, false, _) => Some(VersionBump::Minor),
        (_, _, false) => Some(VersionBump::Patch),
        _ => None,
    };

    changelog.tags = generated_tags(base_version, bump, release, include_aliases);

    Ok(changelog)
}

fn message_before_first_version_hint(message: &str) -> &str {
    let mut line_start = 0;

    for line in message.split_inclusive('\n') {
        if parse_version_hint(line.trim_end_matches('\n')).is_some() {
            return &message[..line_start];
        }

        line_start += line.len();
    }

    message
}

fn find_latest_semver_tag(repo: &Repository) -> Result<Option<(Version, Oid)>, git2::Error> {
    let tag_names = repo.tag_names(None)?;
    let mut tagged_commits = Vec::new();

    for index in 0..tag_names.len() {
        let Some(tag) = tag_names.get(index)? else {
            continue;
        };
        let Some(version) = parse_semver_tag(tag) else {
            continue;
        };
        let Ok(object) = repo.revparse_single(&format!("refs/tags/{tag}")) else {
            continue;
        };
        let Ok(commit) = object.peel_to_commit() else {
            continue;
        };

        tagged_commits.push((commit.id(), version));
    }

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TIME)?;

    for oid in revwalk {
        let oid = oid?;
        let latest_for_commit = tagged_commits
            .iter()
            .filter(|(tagged_oid, _)| *tagged_oid == oid)
            .map(|(_, version)| *version)
            .max();

        if let Some(version) = latest_for_commit {
            return Ok(Some((version, oid)));
        }
    }

    Ok(None)
}

fn parse_semver_tag(tag: &str) -> Option<Version> {
    let version = tag.strip_prefix('v')?;

    if version.contains(['-', '+']) {
        return None;
    }

    let mut parts = version.split('.');

    let major = parse_semver_number(parts.next()?)?;
    let minor = parse_semver_number(parts.next()?)?;
    let patch = parse_semver_number(parts.next()?)?;

    if parts.next().is_some() {
        return None;
    }

    Some(Version {
        major,
        minor,
        patch,
    })
}

fn parse_semver_number(value: &str) -> Option<usize> {
    if value != "0" && value.starts_with('0') {
        return None;
    }

    value.parse().ok()
}

fn generated_tags(
    base: Version,
    bump: Option<VersionBump>,
    release: &ReleaseMetadata,
    include_aliases: bool,
) -> Vec<String> {
    let Some(bump) = bump else {
        return Vec::new();
    };

    let (major, minor, patch) = match bump {
        VersionBump::Major => (base.major + 1, 0, 0),
        VersionBump::Minor => (base.major, base.minor + 1, 0),
        VersionBump::Patch => (base.major, base.minor, base.patch + 1),
    };
    let next = Version {
        major,
        minor,
        patch,
    };

    let mut full_tag = format!("v{}.{}.{}", next.major, next.minor, next.patch);

    if let Some(prerelease) = &release.prerelease {
        full_tag.push('-');
        full_tag.push_str(prerelease);
    }

    if let Some(build) = &release.build {
        full_tag.push('+');
        full_tag.push_str(build);
    }

    let mut tags = vec![full_tag];

    if include_aliases {
        tags.push(format!("v{}.{}", next.major, next.minor));
        tags.push(format!("v{}", next.major));
    }

    tags
}

fn apply_generated_tags(repo: &Repository, changelog: &Changelog) -> Result<(), git2::Error> {
    let Some(target) = repo.head()?.target() else {
        return Err(git2::Error::from_str("HEAD does not point to a commit"));
    };

    for (index, tag) in changelog.tags.iter().enumerate() {
        let force = index > 0;
        let reference_name = format!("refs/tags/{tag}");
        let message = if force {
            format!("Update versionedcommits moving release alias {tag}")
        } else {
            format!("Create versionedcommits release tag {tag}")
        };

        repo.reference(&reference_name, target, force, &message)?;
    }

    Ok(())
}

fn ensure_changelog_commit_is_safe(
    repo: &Repository,
    changelog: &Changelog,
) -> Result<(), git2::Error> {
    let head = repo.head()?.peel_to_commit()?;
    let mut index = repo.index()?;

    if index.write_tree()? != head.tree_id() {
        return Err(git2::Error::from_str(
            "cannot create changelog commit while the index contains staged changes",
        ));
    }

    if let Some(tag) = changelog.tags.first() {
        let reference_name = format!("refs/tags/{tag}");
        if repo.find_reference(&reference_name).is_ok() {
            return Err(git2::Error::from_str(
                "cannot create changelog commit because the release tag already exists",
            ));
        }
    }

    Ok(())
}

fn commit_changelog(
    repo: &Repository,
    output_path: &str,
    version: &str,
) -> Result<Oid, Box<dyn Error>> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("cannot commit a changelog in a bare repository"))?
        .canonicalize()?;
    let output_path = Path::new(output_path).canonicalize()?;
    let repository_path = output_path.strip_prefix(&workdir).map_err(|_| {
        git2::Error::from_str("changelog output must be inside the repository worktree")
    })?;

    let parent = repo.head()?.peel_to_commit()?;
    let mut index = repo.index()?;
    index.add_path(repository_path)?;
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;
    let signature = repo.signature()?;
    let message = format!("Update the changelog for version {version}");

    Ok(repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &[&parent],
    )?)
}

fn prepend_to_file(path: &str, content: &str) -> io::Result<()> {
    let path = Path::new(path);

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let existing = match fs::read_to_string(path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };

    fs::write(path, format!("{content}{existing}"))
}

fn render_next_tag(changelog: &Changelog) -> String {
    changelog
        .tags
        .first()
        .map(|tag| format!("{tag}\n"))
        .unwrap_or_default()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let options = parse_cli_args(&args)
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    let repo = Repository::discover(".")?;
    let changelog = build_changelog(&repo, &options.release, options.include_aliases)?;

    let output = match options.output_format {
        OutputFormat::Json => {
            let mut output = serde_json::to_string_pretty(&changelog)?;
            output.push('\n');
            output
        }
        OutputFormat::Markdown => render_markdown(&changelog),
        OutputFormat::NextTag => render_next_tag(&changelog),
    };

    if options.commit_changelog && !changelog.tags.is_empty() {
        ensure_changelog_commit_is_safe(&repo, &changelog)?;

        let output_path = options
            .output_path
            .as_deref()
            .expect("validated --commit output path");
        let version = changelog.tags.first().expect("non-empty generated tags");

        prepend_to_file(output_path, &output)?;
        commit_changelog(&repo, output_path, version)?;
        apply_generated_tags(&repo, &changelog)?;
    } else {
        if options.apply_tags {
            apply_generated_tags(&repo, &changelog)?;
        }

        if let Some(output_path) = options.output_path {
            prepend_to_file(&output_path, &output)?;
        } else {
            print!("{output}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
