# Issue 008: Smart Add Skill Entry

## What To Build

Add a unified "Smart Add Skill" entry point that starts with the user's target
tool choice, accepts one input, and routes it safely as a URL, local file, or
natural-language install request.

This issue is the parent for the Stage 1.5 slices. The core entry belongs in the
personal inventory 1.0 roadmap. Full market search is intentionally separated so
it can move to the discovery roadmap without blocking the safer local flow.

## Decisions

- Target tools are selected first. The flow should not continue until at least
  one target is selected.
- Natural-language input is disabled when the LLM provider is not configured,
  with a clear prompt to configure it.
- Non-install requests show a useful reason from the classifier.
- GitHub Code Search is the recommended MVP search source, but it is deferred
  until the discovery/search slice.
- LLM output must never trigger install, write files, or return executable
  commands. It may only return a search query and explanatory reason.

## Child Issues

- Issue 009: Raw URL Single-File Import
- Issue 010: Smart Add Input And Routing
- Issue 011: Natural-Language Skill Install Intent Classifier
- Issue 012: Unified Import Risk Review
- Issue 013: GitHub Skill Search MVP

## Acceptance Criteria

- [ ] URL, local file, and natural-language input all enter the existing preview,
      audit, compatibility, and confirm-before-write install flow.
- [ ] Target selection happens before input submission.
- [ ] Natural-language classification cannot produce a path, command, URL, or
      install action.
- [ ] Every path shows import audit and compatibility review before install.
- [ ] Tests cover routing and safety invariants for each implemented child issue.

## Blocked By

Issue 007.

