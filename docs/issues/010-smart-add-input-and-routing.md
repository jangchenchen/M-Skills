# Issue 010: Smart Add Input And Routing

## What To Build

Replace or extend the import wizard entry with a Smart Add input that requires
target tools first, classifies the user's input as URL, local file, or Ask AI,
and routes URL/local input into the existing preview flow.

The completed slice should be demoable without market search: URL and local
paths install through the current preview pipeline, while Ask AI either invokes
the classifier from Issue 011 or shows the configured disabled state.

## Acceptance Criteria

- [ ] The add flow starts with target tool selection and blocks submit until at
      least one target is selected.
- [ ] The input classifies obvious URLs, GitHub SSH URLs, local paths, and
      natural-language text as the user types.
- [ ] URL input routes to `preview_import`, including raw HTTPS URLs from
      Issue 009.
- [ ] Local file or directory input routes to `preview_import`.
- [ ] Ask AI is visibly unavailable when the LLM provider is not configured and
      points the user to settings.
- [ ] Target choices constrain the install targets shown after preview; the
      frontend does not implement compatibility rules itself.
- [ ] Tests or build checks cover the routing state and TypeScript DTO usage.

## Blocked By

Issue 009.

