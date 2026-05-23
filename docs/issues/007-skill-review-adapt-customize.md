# Issue 007: Skill Review, Adaptation, And Customization

## What To Build

Extend M-Skills from a safe installer into a safe Skill maintenance workflow. Users should be able to review whether a Skill fits Claude Code, Codex, and Gemini; adapt a Claude Code Skill for Codex only after inspecting the diff; fork third-party Skills into local custom versions; and eventually use an LLM to generate editable improvement drafts.

This issue is intentionally split into three batches so future work can continue without re-deciding scope.

## Current State

All three batches are implemented. Verified with `cargo fmt --all -- --check`, `cargo test --workspace` (148 passed / 0 failed), and `npm run build`.

Batch 1 — deterministic compatibility review:

- `src-tauri/src/compatibility.rs` holds the rule engine.
- Import preview candidates expose `compatibilityReviews`.
- Import wizard and detail panel show yellow compatibility/risk guidance.
- `review_artifact_compatibility` reviews an existing artifact against selected targets.

Batch 2 — diff-first adaptation and local custom Skills:

- Legacy `adapt_install_to_codex` direct-install command was removed.
- `preview_adapt_skill_for_codex` returns a draft + diff + compatibility reviews without writing.
- `preview_fork_skill` builds a fork draft for a chosen target without writing.
- `save_custom_skill_edit` re-parses an edited `SKILL.md`, runs compatibility review, and returns a draft preview.
- `confirm_install_skill_draft` is the single write path; it installs an adapted/forked/edited draft and writes `.m-skills.json` lineage metadata next to the Skill.
- Frontend uses `SkillPreviewModal` and `CustomSkillEditor` to render diff, risk notices, and require explicit confirmation before any write.

Batch 3 — LLM-assisted Skill rewriting:

- `src-tauri/src/rewrite.rs` holds the prompt builder and strict-JSON parser; the source `SKILL.md` and user instruction are fenced as untrusted content.
- `rewrite_skill_with_llm` uses the OpenAI-compatible provider, returns `RewriteSkillOutcomeDto` (`draftBody`, `summary`, `notes`, `providerKind`, `model`, `compatibilityReviews`), and never touches disk.
- Five modes ship: `adapt_to_codex`, `complete_missing_info`, `reduce_risk`, `customize_workflow`, `simplify`.
- AI rewrite panel inside `CustomSkillEditor` shows the LLM draft, diff vs current editor, summary/notes, and compatibility notice. Apply-draft routes through the existing `save_custom_skill_edit` → `confirm_install_skill_draft` flow.
- Errors: `rewriteNotConfigured`, `rewriteParseFailed`, `rewriteInvalidMode`.

## Batch 2: Diff-First Adaptation And Local Custom Skills

### What To Build

Turn adaptation and user edits into a confirm-before-write flow.

Required behavior:

- Replace direct Codex adaptation install with:
  - `preview_adapt_skill_for_codex`
  - `confirm_install_adapted_skill`
- Show original `SKILL.md`, adapted `SKILL.md`, diff, compatibility reviews, and risk guidance before writing files.
- Add `Fork as custom skill` for existing or imported Skills.
- Add a basic `SKILL.md` editor for custom versions.
- Save custom/adapted versions without overwriting the original Skill.
- Re-run compatibility review before save or install.
- Record local lineage metadata in a sidecar file, preferably `.m-skills.json`, near the custom Skill.

Suggested lineage shape:

```json
{
  "sourceKind": "fork",
  "sourceTool": "claude-code",
  "sourcePath": "/path/to/original/skill",
  "sourceUrl": null,
  "sourceHash": "sha256...",
  "parentName": "original-skill"
}
```

Use `sourceKind: "adaptation"` for Claude Code -> Codex adapted versions.

### Acceptance Criteria

- [x] Clicking "Adapt install to Codex" opens a preview instead of installing immediately.
- [x] The preview shows original content, adapted content, diff, and yellow compatibility/risk guidance.
- [x] No files are written until the user confirms.
- [x] Confirm installs the adapted Skill to Codex and refreshes inventory.
- [x] Name conflicts are handled before install; do not overwrite existing Skills.
- [x] Users can fork a Skill into a local custom Skill without modifying the original.
- [x] Users can edit custom `SKILL.md`, review diff, and save.
- [x] Custom/adapted Skills include `.m-skills.json` lineage metadata.
- [x] Tests cover preview-without-write, confirmed install, fork save, conflict handling, and lineage metadata.

Note: the single confirm command shipped as `confirm_install_skill_draft` (general — handles adapted, forked, and edited drafts) instead of the spec's `confirm_install_adapted_skill`.

### Out Of Scope For Batch 2

- LLM rewriting.
- Three-way merge with upstream updates.
- Registry schema migration for lineage.
- Gemini Extension generation from `SKILL.md`.
- Rich Markdown editor.

## Batch 3: LLM-Assisted Skill Rewriting

### What To Build

Add LLM-assisted editing on top of the Batch 2 editor and diff flow. The LLM must only generate drafts. It must never write files or install artifacts without user confirmation.

Use the existing OpenAI-compatible provider configuration and keyring used by translation/review.

Suggested request shape:

```ts
type RewriteMode =
  | "adapt_to_codex"
  | "complete_missing_info"
  | "reduce_risk"
  | "customize_workflow"
  | "simplify";

type RewriteSkillRequest = {
  artifact: ArtifactDto;
  mode: RewriteMode;
  userInstruction: string;
  locale: string;
};

type RewriteSkillOutcome = {
  draftBody: string;
  summary: string;
  notes: string[];
  providerKind: string;
  model: string;
  compatibilityReviews: CompatibilityReviewDto[];
};
```

Required behavior:

- Add `rewrite_skill_with_llm`.
- Prompt must treat the source Skill as untrusted text, not instructions to follow.
- Prompt must ask for strict JSON output only.
- Preserve or repair `SKILL.md` frontmatter when possible.
- Do not add dangerous commands; convert risky automation into "ask user first" guidance.
- Do not claim Claude Code-specific behavior works identically in Codex.
- After a draft is returned, re-run compatibility/risk review and show diff.
- User can continue revising, manually edit, save as custom Skill, or install to a compatible target.

### Acceptance Criteria

- [x] Users can choose an LLM rewrite mode and enter natural-language requirements.
- [x] Missing LLM configuration produces a clear non-blocking error.
- [x] LLM output is parsed as strict JSON; malformed output returns `rewriteParseFailed`.
- [x] LLM drafts do not write or install anything automatically.
- [x] Drafts show summary, notes, diff, and compatibility/risk guidance.
- [x] Users can continue from a draft into manual editing and save through the Batch 2 flow.
- [x] Tests cover prompt construction, JSON parsing, missing config, malformed output, and no-write-before-confirm.

### Out Of Scope For Batch 3

- Automatic upstream update merge.
- Publishing custom Skills to GitHub.
- Multi-model comparison.
- Automatic validation by running the Skill.
- Gemini Extension generation.

## Implementation Notes

- Keep deterministic compatibility review separate from LLM conflict/rewrite review.
- UI should not implement compatibility rules; use backend review DTOs.
- Existing `review_import` remains an install-conflict review and should not become the compatibility engine.
- Prefer a single reusable diff/preview component for Codex adaptation, custom fork edits, and LLM drafts.
- Use integration-style tests through Tauri commands or service APIs where practical.

## Verification

Before considering each batch complete:

```bash
cargo fmt --all -- --check
cargo test --workspace
npm run build
```

If `skillsmgr-translate` wiremock tests fail with local port binding errors in a sandbox, rerun the same test command with permissions that allow local mock-server ports.
