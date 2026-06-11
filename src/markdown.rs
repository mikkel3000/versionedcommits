use crate::{Change, Changelog};

const CHANGE_TYPES: [&str; 6] = [
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
];

pub(crate) fn render_markdown(changelog: &Changelog) -> String {
    let Some(version) = changelog.tags.first() else {
        return String::new();
    };

    let mut markdown = format!("## [{version}]\n");

    append_change_type_sections(&mut markdown, changelog);
    append_fallback_section(&mut markdown, "Major", &changelog.major);
    append_fallback_section(&mut markdown, "Minor", &changelog.minor);
    append_fallback_section(&mut markdown, "Patch", &changelog.patch);

    markdown
}

fn append_change_type_sections(markdown: &mut String, changelog: &Changelog) {
    for change_type in CHANGE_TYPES {
        let changes = all_changes(changelog)
            .filter_map(|change| {
                categorized_title(&change.title)
                    .filter(|(category, _)| *category == change_type)
                    .map(|(_, title)| (change, title))
            })
            .collect::<Vec<_>>();

        append_markdown_section(markdown, change_type, &changes);
    }
}

fn append_fallback_section(markdown: &mut String, heading: &str, changes: &[Change]) {
    let changes = changes
        .iter()
        .filter(|change| categorized_title(&change.title).is_none())
        .map(|change| (change, change.title.as_str()))
        .collect::<Vec<_>>();

    append_markdown_section(markdown, heading, &changes);
}

fn append_markdown_section(markdown: &mut String, heading: &str, changes: &[(&Change, &str)]) {
    if changes.is_empty() {
        return;
    }
    markdown.push_str(&format!("\n### {heading}\n\n"));

    for (change, title) in changes {
        markdown.push_str(&format!("- {title} (`{}`)\n", short_sha(&change.sha)));

        if !change.description.is_empty() {
            markdown.push('\n');
            for line in change.description.lines() {
                markdown.push_str(&format!("  {line}\n"));
            }
        }
    }
}

fn all_changes(changelog: &Changelog) -> impl Iterator<Item = &Change> {
    changelog
        .major
        .iter()
        .chain(&changelog.minor)
        .chain(&changelog.patch)
}

fn categorized_title(title: &str) -> Option<(&'static str, &str)> {
    let (first_word, rest) = title.split_once(char::is_whitespace)?;
    let change_type = CHANGE_TYPES
        .iter()
        .find(|change_type| **change_type == first_word)?;
    let title = rest.trim();

    if title.is_empty() {
        None
    } else {
        Some((*change_type, title))
    }
}

fn short_sha(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}
