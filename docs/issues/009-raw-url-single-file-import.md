# Issue 009: Raw URL Single-File Import

## What To Build

Allow the existing import preview command to accept an HTTPS URL that directly
points to one supported artifact file. The downloaded file is staged into a
temporary directory and then reuses the existing sniff, audit, compatibility,
preview, and install pipeline.

This is the shared entry path for pasted raw Skill URLs and future search-result
installs.

## Acceptance Criteria

- [ ] `preview_import` routes GitHub repository URLs through the existing GitHub
      import path and non-GitHub HTTPS URLs through raw URL import.
- [ ] Raw URL import accepts only a single file up to 1 MB.
- [ ] Raw URL import accepts only `text/plain`, `text/markdown`, and
      `application/json` content types.
- [ ] Downloaded content is staged first and then fully audited before install is
      possible.
- [ ] Raw `SKILL.md` and `gemini-extension.json` URLs are sniffed into the
      correct artifact kind.
- [ ] The artifact source records the original URL without pretending it is a
      GitHub repository.
- [ ] Tests cover success, unsupported scheme, unsupported content type, payload
      limit, and source metadata.

## Current State

Implemented locally in the current working tree. Verified with:

```bash
cargo fmt --all -- --check
cargo test --workspace
npm run build
```

## Blocked By

Issue 008.

