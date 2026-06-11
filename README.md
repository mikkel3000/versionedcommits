## Versioned commits
This is my personal objection to conventional commits, which I do not like.
This allows me to write commits in any style I like, while still being able to generate tags and changelogs from the commit messages. 

It aims to be simple and explicit, and SemVer 2 compatible.

### Specification
Add a version hint to a commit message when that commit should affect the next release.
Hints are `@major`, `@minor` or `@patch`, and must start on a new line.

```txt
Implement the query parser

Handle quoted phrases and normalize whitespace before building the SQL query.

@<hint> release-note title (optional)
release-note description (optional)
@<hint>(optional)
```

The commit title and description are for the people working on the code: write them in whatever style explains the change to your team. They do not need to follow a conventional-commit format.

The version metadata is for the people receiving the release. Put a concise, user-facing release-note title after the opening hint. To add a user-facing description, close the metadata block with the same hint. A single hint is also valid; if it has no title or description, the commit title and description are used in the changelog.

For Markdown output, start release-note titles with `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, or `Security` to group them by the types described in [Keep a changelog](https://keepachangelog.com/en/1.1.0/).

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
  --output <path>           Prepend the output to a file
  --tag                     Create the generated Git tags
  --aliases                 Include moving minor and major tag aliases
  --commit                  Commit the changelog before creating the tags
  --pre <identifier>        Add SemVer prerelease metadata
  --build <identifier>      Add SemVer build metadata
```

`--commit` requires `--format markdown`, `--output`, and `--tag`. It commits only the generated changelog with the message `Update the changelog for version <tag>`, then creates the release tags on that commit. It refuses to run when other changes are staged.

### Golden path

This will update CHANGELOG.md, add it to a commit, point a new tag at that commit and push it to main.
```sh
# On branch main/master, run:
versionedcommits --format markdown --output CHANGELOG.md --tag --commit
## This creates the tags, updates CHANGELOG.md and adds it to a release commit.

## Update main with the tags and commit
git push origin main --tags
```
