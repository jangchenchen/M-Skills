# Issue 004: Install And Uninstall MVP Targets

## What To Build

Implement install and uninstall for Claude Code, Codex CLI, opencode, and Gemini CLI. Each operation should be safe, reversible where possible, and should avoid overwriting unrelated user files.

## Acceptance Criteria

- [ ] Install copies a staged artifact into the selected target scope.
- [ ] Install refuses same-name conflicts from different sources in v1.
- [ ] Uninstall removes only the selected installation.
- [ ] Uninstall refuses to remove paths that do not match the expected managed target layout.
- [ ] UI can install one artifact into multiple compatible targets and uninstall one installation at a time.
- [ ] Tests cover conflict, successful install, and successful uninstall for each MVP adapter.

## Blocked By

Issues 001 and 003.

