# Issue 013: GitHub Skill Search MVP

## What To Build

Add a minimal discovery/search backend for Smart Add natural-language queries
using GitHub Code Search. Given a search query and selected targets, return a
candidate list of real `SKILL.md` files with parsed metadata and compatibility
badges. Installing a candidate must route back through raw URL import and the
normal preview/audit/confirm flow.

This issue is intentionally deferred from the Stage 1 Smart Add core. It can be
implemented in Stage 3 discovery work unless the product needs searchable
installation earlier.

## Acceptance Criteria

- [ ] A backend command searches GitHub Code Search for `SKILL.md` files using
      the extracted query.
- [ ] Each result fetches enough metadata to show Skill name, description,
      repository, stars, and recent activity when available.
- [ ] Each result runs deterministic compatibility review for the selected
      targets.
- [ ] Results are sorted by compatibility, repository signal, and recency.
- [ ] Search results are cached client-side or backend-side for about one hour.
- [ ] Installing a result never writes directly; it sends the raw `SKILL.md` URL
      through Issue 009's raw URL preview path.
- [ ] Rate-limit and network errors are shown as recoverable search failures.
- [ ] Tests cover query construction, result parsing, compatibility annotation,
      and install routing back to preview.

## Blocked By

Issue 011 and Issue 012.

