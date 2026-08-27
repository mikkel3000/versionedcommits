## versionedcommits 
This is my personal objection to conventional commits, which I do not like.
This allows me to write commits in any style I like, while still being able to generate tags and changelogs from the commit messages. 

It aims to be a simple and explicit, SemVer 2 compatible release generator.

### Usage in GitHub Actions 

Add this workflow to `.github/workflows/versionedcommits.yml` in your repository:

```yaml
name: Versioned releases

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
    types: [closed]
  workflow_dispatch:

permissions:
  contents: write
  pull-requests: write
  statuses: write

jobs:
  versionedcommits:
    uses: mikkel3000/versionedcommits/.github/workflows/versionedcommits.yml@v0.1.0
```

In **Settings → Actions → General**, enable **Allow GitHub Actions to create and approve pull requests**. The reusable workflow uses the calling repository's `GITHUB_TOKEN`; no extra secret is required.

### Write commits 
Add version hints to your commit message:

Hints are `@major`, `@minor` or `@patch`, and must start on a new line.

```txt
Commit title whatever you want

Commit message body whatever you want.

@<hint> release-note title(optional)
release-note description(optional)
@<hint>(optional)
```

The commit title and description are for the people working on the code: write them in whatever style explains the change to your team. They do not need to follow a conventional-commit format.The version metadata is for the users of the release. 

Markdown output, start release-note titles with `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, or `Security` to group them by the types described in [Keep a changelog](https://keepachangelog.com/en/1.1.0/).

### Examples

#### Recommended: implementation-focused commit, user-facing release note
```txt
Implement search-query parser

Parse quoted phrases and normalize whitespace before constructing the query.

@minor Added project search
Search now supports partial project names and shows the closest matches first.
@minor
```

The commit explains how the feature was built. The `@minor` block explains what users get in the release.

#### One-line user-facing release note
```txt
Return validation errors from the API

Map malformed request parameters to the public validation-error response.

@patch Fixed invalid request handling
```

#### Use the commit message as the release note when it is already user-facing
```txt
Added project search

Users can search for projects by name.

@minor
```

In this last form, the changelog uses the commit title and description because the hint has no release-note title or description.

### The binary
Using libgit2 to analyze your git tags and commits, the binary outputs json to stdout like:
```json
{
  "tags": ["v1.0.0"],
  "major": [
    {
      "title": "",
      "description": "",
      "sha": ""
    }
  ],
  "minor": [
    {
      "title": "",
      "description": "",
      "sha": ""
    }
  ],
  "patch": [
    {
      "title": "",
      "description": "",
      "sha": ""
    }
  ]
}
```

The value in `tags` is the new SemVer 2.0.0 release tag generated from commits since the latest stable semantic version tag. Add `--aliases` when you also want moving minor and major aliases such as `v1.0` and `v1`.

```txt
Usage: versionedcommits [OPTIONS]

Options:
  --format <json|markdown>  Output format [default: json]
  --next-tag                Output only the next immutable release tag
  --output <path>           Prepend the output to a file
  --tag                     Create the generated Git tags
  --aliases                 Include moving minor and major tag aliases
  --commit                  Commit the changelog before creating the tags
  --pre <identifier>        Add SemVer prerelease metadata
  --build <identifier>      Add SemVer build metadata
```

`--commit` requires `--format markdown`, `--output`, and `--tag`. It commits only the generated changelog with the message `Update the changelog for version <tag>`, then creates the release tags on that commit. It refuses to run when other changes are staged.

### Local release

Without GitHub Actions, the equivalent one-run path is:

```sh
versionedcommits --format markdown --output CHANGELOG.md --tag --commit
git push origin main --tags
```

The CI golden path intentionally does not maintain an `Unreleased` section. Release PRs contain the final versioned changelog entry; continuously generated unreleased notes can be added separately by projects that want them in hooks.
