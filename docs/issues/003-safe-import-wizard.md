# Issue 003: Safe Local And GitHub Import Wizard

## What To Build

Add an import wizard that accepts a local path or GitHub URL, stages the source, sniffs artifact kinds, shows an audit summary, and offers only compatible install targets.

## Acceptance Criteria

- [ ] Local directory import works without network access.
- [ ] GitHub import clones or fetches into a temporary staging directory and records the resolved commit SHA.
- [ ] The wizard blocks install if no supported artifact kind is found.
- [ ] The wizard blocks incompatible targets, such as installing a Warp workflow into Claude Code.
- [ ] The audit page shows changed files, manifest/frontmatter metadata, source URL or path, and warning text for executable commands or MCP config.
- [ ] Tests cover target filtering by artifact kind.

## Blocked By

Issue 001.

