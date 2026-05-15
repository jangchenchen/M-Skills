# Issue 001: Foundation Domain Model And Parser

## What To Build

Create the core Rust domain model and parser layer for the three artifact kinds: skills, Gemini extensions, and Warp workflows. The result should let the app sniff a local directory and return compatible artifact candidates before any install happens.

## Acceptance Criteria

- [ ] A Rust workspace exists with `skillsmgr-core` and `skillsmgr-parse`.
- [ ] Domain types model `ArtifactKind`, `Scope`, `Target`, `Artifact`, `Installation`, and `ToolAdapter`.
- [ ] Parser can detect `SKILL.md`, `gemini-extension.json`, and Warp workflow YAML candidates.
- [ ] Skill parser reads YAML frontmatter for at least `name`, `description`, and optional `version`.
- [ ] Tests cover one valid sample for each artifact kind and one empty directory.

## Blocked By

None - can start immediately.

