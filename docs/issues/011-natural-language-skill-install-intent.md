# Issue 011: Natural-Language Skill Install Intent Classifier

## What To Build

Add an LLM-backed classifier for natural-language Smart Add input. The command
decides whether the user is asking to install a Skill and, if so, extracts a
plain search query for a future search source.

The classifier must be safe by construction: it cannot return a path, command,
URL, target install action, or file mutation. It returns only an install flag,
an optional search query, and an optional reason.

## Acceptance Criteria

- [ ] A backend intent module builds strict JSON prompts with user input fenced
      as untrusted content.
- [ ] A backend parser accepts only the fixed outcome shape:
      `isInstallRequest`, `searchQuery`, and `reason`.
- [ ] A Tauri command `classify_skill_request(input, locale)` returns the parsed
      outcome.
- [ ] Missing provider configuration returns `intentNotConfigured`.
- [ ] Malformed model output returns `intentParseFailed`.
- [ ] Non-install requests return or surface `notInstallRequest` with the
      model-provided reason.
- [ ] The frontend Ask AI path displays the reason for non-install requests.
- [ ] A successful classification shows the extracted search query but does not
      install, write files, fetch arbitrary URLs, or run commands.
- [ ] Tests cover prompt fencing, strict JSON parsing, missing configuration,
      malformed output, non-install requests, and the no-write invariant.

## Blocked By

Issue 010.

