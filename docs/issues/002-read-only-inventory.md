# Issue 002: Read-Only Installed Artifact Inventory

## What To Build

Build an end-to-end read-only inventory path for installed MVP targets. The user should be able to start the app, scan known global and project locations, and see what artifacts are already on disk.

## Acceptance Criteria

- [ ] Claude Code, Codex CLI, opencode, and Gemini adapters scan global and project scopes.
- [ ] Adapter presence detection reports whether each tool directory or binary is available.
- [ ] Scan results include artifact kind, name, description, target, scope, status, and on-disk path.
- [ ] Frontend displays grouped artifacts with one row per installation.
- [ ] Temporary-directory adapter tests cover global and project scans.

## Blocked By

Issue 001.

