# Issue 012: Unified Import Risk Review

## What To Build

Make the import preview risk review consistent across local files, GitHub
repositories, raw URLs, and future natural-language search results. Users should
see import audit warnings, compatibility review, and source/origin guidance
before any install confirmation.

This issue should not introduce new compatibility rules in the frontend. It
should present backend DTOs clearly and enforce acknowledgement only where the
existing review data says risk is elevated.

## Acceptance Criteria

- [ ] Import preview shows audit warnings for executable commands, MCP config,
      dangerous shell patterns, prompt injection, and large payloads.
- [ ] Import preview shows compatibility review for each selected or available
      target.
- [ ] High-risk audit warnings or conflict review require explicit user
      acknowledgement before install.
- [ ] Incompatible targets cannot be selected for install.
- [ ] Raw URL imports show guidance to verify the source and author.
- [ ] Natural-language search results, once available, show guidance that the
      result came from an AI-extracted search intent and must be source-checked.
- [ ] The UI remains dense and operational rather than marketing-like.
- [ ] Tests or build checks cover the install gating state and TypeScript types.

## Blocked By

Issue 010.

