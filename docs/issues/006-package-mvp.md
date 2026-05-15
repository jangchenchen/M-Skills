# Issue 006: Package And Harden MVP

## What To Build

Prepare the MVP for real local use with packaging, error recovery, and safety polish.

## Acceptance Criteria

- [ ] macOS package builds through Tauri bundler.
- [ ] App has a first-run empty state and clear errors for missing tools.
- [ ] Import/install errors show recovery actions and leave staging data clean.
- [ ] Documentation explains artifact kinds, supported tools, known limitations, and why Warp is separate.
- [ ] Security notes explain that imported agent instructions can cause downstream tools to execute commands.
- [ ] Smoke test covers launch, scan, local import preview, install, and uninstall.

## Blocked By

Issues 002, 003, 004, and 005.

