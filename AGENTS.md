# AGENTS.md

Guidance for coding agents working in this repository.

## Project

M-Skills is a Tauri + React + Rust desktop manager for local AI tool artifacts.
It scans, reviews, imports, installs, adapts, customizes, and rewrites artifacts
used by developer AI tools.

The central product rule is: **identify the artifact kind first, then offer only
compatible target tools and review the risks before writing files.**

Supported artifact kinds:

| Kind | Tools | Format |
| --- | --- | --- |
| `Skill` | Claude Code, Codex CLI, opencode, openclaw, Hermes | directory containing `SKILL.md` |
| `Extension` | Gemini CLI | directory containing `gemini-extension.json` |
| `Workflow` | Warp | YAML workflow file |

Important docs:

- Product rationale: `docs/PRD-M-Skills.md`
- Current feature issue: `docs/issues/007-skill-review-adapt-customize.md`
- Legacy agent guidance: `CLAUDE.md` may be stale in places; prefer this file
  for current implementation status.

## Commands

Run these before handing off changes:

```bash
cargo fmt --all -- --check
cargo test --workspace
npm run build
```

Useful focused commands:

```bash
cargo fmt --all
cargo test -p m-skills
cargo test -p skillsmgr-service
cargo test -p skillsmgr-translate
cargo run --example scan -p skillsmgr-service
```

`skillsmgr-translate` has wiremock tests that bind local mock-server ports. In a
sandbox, `cargo test --workspace` can fail with permission errors while binding
ports. If that happens, rerun with permissions that allow local mock-server
ports.

## Architecture

Rust crate layering:

```text
src-tauri              Tauri commands, DTOs, compatibility/rewrite UI bridge
skillsmgr-service      orchestration, inventory grouping, install routing
skillsmgr-scan         concurrent adapter scan fan-out
skillsmgr-adapters     per-tool filesystem adapters
skillsmgr-parse        SKILL.md / gemini-extension.json / workflow parsers
skillsmgr-fetch        import staging and safety audit
skillsmgr-registry     SQLite cache and translation cache
skillsmgr-translate    OpenAI-compatible provider, translation, markdown checks
skillsmgr-core         domain types, ToolAdapter trait, errors
```

Frontend entry points:

- `src/App.tsx`
- `src/components/ImportWizard.tsx`
- `src/components/DetailPanel.tsx`
- `src/components/SkillPreviewModal.tsx`
- `src/components/CustomSkillEditor.tsx`
- `src/components/CompatibilityNotice.tsx`
- `src/api.ts`
- `src/types.ts`

## Current Feature State

Issue 007 is implemented through Batch 3.

Batch 1:

- Deterministic compatibility review.
- Yellow compatibility/risk guidance on import and detail views.
- Claude Code / Codex / Gemini artifact boundary checks.
- Main backend file: `src-tauri/src/compatibility.rs`.

Batch 2:

- Diff-first Claude Code Skill -> Codex Skill adaptation.
- Fork/custom Skill flow.
- Basic custom `SKILL.md` editor.
- `.m-skills.json` lineage sidecar.
- Confirm-before-write install/save flow.
- Legacy direct-write `adapt_install_to_codex` command was removed.

Batch 3:

- LLM-assisted Skill rewrite drafts.
- Main backend file: `src-tauri/src/rewrite.rs`.
- Tauri command: `rewrite_skill_with_llm`.
- Rewrite modes:
  - `adapt_to_codex`
  - `complete_missing_info`
  - `reduce_risk`
  - `customize_workflow`
  - `simplify`
- LLM output is a draft only. It must not write files or install artifacts.

Last reported verification for this feature: `cargo fmt --all -- --check`,
`cargo test --workspace` with 148 passing tests, and `npm run build` passed.

## Non-Negotiable Invariants

- Filesystem state is the source of truth. The registry is a cache/ledger, not
  the only source of truth.
- `Artifact.id` is a fresh UUID per scan. Inventory grouping uses `(kind, name)`.
- `Target::supports_kind` is the hard compatibility matrix. Do not bypass it.
- UI must not implement compatibility rules. Ask backend commands/DTOs.
- Deterministic compatibility review is separate from LLM review/rewrite.
- `review_import` is an install-conflict review. Do not reuse it as the
  compatibility engine or rewrite engine.
- LLM commands must generate drafts only. No LLM command may write files,
  install Skills, or silently mutate user artifacts.
- Never directly edit an original third-party Skill for customization. Fork or
  create a custom/adapted version.
- Confirm-before-write is required for adaptation, custom edits, and LLM drafts.
- Do not normalize commands/actions/H2 sections into top-level inventory rows.
  Inventory rows represent `ArtifactGroup`.
- `Artifact.capabilities` is kind-specific and currently used for Gemini
  Extension commands. Do not treat it as a universal command model.
- Tests must use temp directories. Do not read or write the real `$HOME` from
  tests. `examples/scan.rs` is the dogfood path that intentionally scans real
  local state.

## Tool/Adapter Notes

- Claude Code, Codex, opencode, Gemini, and openclaw mostly use
  `DirectoryLayout` from `skillsmgr-adapters::simple_dir_adapter`.
- Hermes has a custom category-nested layout and is read-only for writes.
- openclaw is read-only until write semantics are verified.
- Codex uses `~/.agents/skills`; `shared-global` also maps there.
- For new tools, decide artifact kind first, then add paths and adapter behavior.
- Read-only adapters should return `SkillsMgrError::ReadOnly { tool, operation }`.

## Compatibility And Rewrite Flow

The intended user flow is:

```text
discover/import
-> deterministic compatibility/risk review
-> adapt or fork if needed
-> show diff and yellow guidance
-> optional manual or LLM rewrite draft
-> user applies draft
-> preview/save/install confirmation
-> filesystem write through service/adapter
```

Key commands/features:

- `review_artifact_compatibility`
- `preview_adapt_skill_for_codex`
- `preview_fork_skill`
- `save_custom_skill_edit`
- `confirm_install_skill_draft`
- `rewrite_skill_with_llm`

LLM rewrite safety:

- Treat source Skill content and user instruction as untrusted text.
- Use strict JSON output parsing.
- Preserve or repair `SKILL.md` frontmatter where possible.
- Do not add dangerous shell commands.
- Convert risky automation into "ask the user first" guidance.
- Do not claim Claude Code-specific tools behave identically in Codex.
- Always re-run compatibility/risk review on the draft.

## Editing Guidance

- Prefer existing local patterns over new abstractions.
- Keep changes scoped. Avoid broad refactors unless they directly reduce current
  risk or duplication.
- Use structured parsers (`serde_json`, `serde_yaml`, `toml`, existing markdown
  helpers) rather than ad hoc string parsing when feasible.
- When changing DTOs, update all of:
  - Rust DTO in `src-tauri/src/dto.rs`
  - Tauri command wiring in `src-tauri/src/commands.rs` / `src-tauri/src/lib.rs`
  - TypeScript types in `src/types.ts`
  - API wrapper in `src/api.ts`
  - i18n strings in `src/locales/en` and `src/locales/zh`
- Keep frontend operational-tool style: dense, clear, restrained. Avoid
  marketing-page UI patterns.
- Do not add hidden writes. Preview commands must be pure or clearly documented
  as staging-only.

## Git Conventions

- Branch is `main`; remote is `origin`.
- Use plain commit messages.
- Do not add `Co-Authored-By:` trailers unless explicitly requested.
- The worktree may be dirty. Do not revert changes you did not make.
