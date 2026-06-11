#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    #[default]
    Json,
    Markdown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CliOptions {
    pub(crate) output_format: OutputFormat,
    pub(crate) output_path: Option<String>,
    pub(crate) apply_tags: bool,
    pub(crate) include_aliases: bool,
    pub(crate) commit_changelog: bool,
    pub(crate) release: ReleaseMetadata,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ReleaseMetadata {
    pub(crate) prerelease: Option<String>,
    pub(crate) build: Option<String>,
}

pub(crate) fn parse_cli_args(args: &[String]) -> Result<CliOptions, String> {
    let mut options = CliOptions::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--format" => {
                let Some(format) = args.get(index + 1) else {
                    return Err(usage().to_string());
                };
                options.output_format = parse_output_format(format)?;
                index += 2;
            }
            "--output" => {
                let Some(output_path) = args.get(index + 1) else {
                    return Err(usage().to_string());
                };
                if output_path.is_empty() {
                    return Err("output path cannot be empty".to_string());
                }
                options.output_path = Some(output_path.clone());
                index += 2;
            }
            "--tag" => {
                options.apply_tags = true;
                index += 1;
            }
            "--aliases" => {
                options.include_aliases = true;
                index += 1;
            }
            "--commit" => {
                options.commit_changelog = true;
                index += 1;
            }
            "--pre" => {
                let Some(prerelease) = args.get(index + 1) else {
                    return Err(usage().to_string());
                };
                if !is_valid_prerelease_metadata(prerelease) {
                    return Err("invalid prerelease metadata".to_string());
                }
                options.release.prerelease = Some(prerelease.clone());
                index += 2;
            }
            "--build" => {
                let Some(build) = args.get(index + 1) else {
                    return Err(usage().to_string());
                };
                if !is_valid_semver_identifiers(build) {
                    return Err("invalid build metadata".to_string());
                }
                options.release.build = Some(build.clone());
                index += 2;
            }
            _ => return Err(usage().to_string()),
        }
    }

    validate_cli_options(&options)?;

    Ok(options)
}

fn validate_cli_options(options: &CliOptions) -> Result<(), String> {
    if !options.commit_changelog {
        return Ok(());
    }

    if !options.apply_tags {
        return Err("--commit requires --tag".to_string());
    }
    if options.output_format != OutputFormat::Markdown {
        return Err("--commit requires --format markdown".to_string());
    }
    if options.output_path.is_none() {
        return Err("--commit requires --output path".to_string());
    }

    Ok(())
}

pub(crate) fn parse_output_format(format: &str) -> Result<OutputFormat, String> {
    match format {
        "json" => Ok(OutputFormat::Json),
        "markdown" => Ok(OutputFormat::Markdown),
        _ => Err("format must be either json or markdown".to_string()),
    }
}

fn is_valid_prerelease_metadata(value: &str) -> bool {
    is_valid_semver_identifiers(value)
        && value
            .split('.')
            .all(|identifier| !has_leading_zero_numeric_identifier(identifier))
}

fn is_valid_semver_identifiers(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
}

fn has_leading_zero_numeric_identifier(identifier: &str) -> bool {
    identifier.len() > 1
        && identifier.starts_with('0')
        && identifier.chars().all(|c| c.is_ascii_digit())
}

pub(crate) fn usage() -> &'static str {
    "usage: versionedcommits [--format json|markdown] [--output path] [--tag] [--aliases] [--commit] [--pre identifier] [--build identifier]"
}
