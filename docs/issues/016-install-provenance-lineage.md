# Issue 016: Generalized Install Provenance — Lineage Sidecar for Imports

## What To Build

Make every install record where it came from, durably, by writing the
`.m-skills.json` lineage sidecar on the **import confirm path** — not just the
adapt/fork/edit draft path that writes it today. Generalize `LineageDto` so one
sidecar shape covers market, GitHub, raw-URL, and local imports as well as the
existing fork/adaptation/edit cases.

This is **P0a** from Issue 015's Decisions section: the shared prerequisite for
update detection (Stage 2.1), in-library "source" identification, and
reinstall/uninstall provenance.

## Current State / The Gap

Traced 2026-06-08 (anchors are current line numbers):

- `import_candidate` (`crates/skillsmgr-fetch/src/lib.rs:236`) already sets
  `Artifact.source`:
  - GitHub → `Source::GitHub { url, rev: <resolved_commit_sha> }` — URL **and**
    the pinned commit SHA are captured.
  - RawUrl → `Source::Url { url }` — URL, no rev.
  - Local → `Source::Local { path: <staged root> }`.
- The import confirm command is `install(candidate_index, targets)`
  (`src-tauri/src/commands.rs:458`). It calls `service.install_from_candidate`
  per target, then spawns the summary and emits `installation-changed`.
  **It writes no sidecar.**
- `install_from_candidate` (`crates/skillsmgr-service/src/lib.rs:167`) records
  the original `candidate.artifact` (with real `Source`) into the registry, but
  the registry is a rebuildable cache (CLAUDE.md invariant) — provenance does
  not survive a rebuild-from-disk. Only a sidecar makes it durable.
- `write_lineage_sidecar` (`commands.rs:2181`) and `read_lineage_sidecar`
  (`dto.rs:850`) already exist, and the inventory DTO already surfaces
  `LineageDto` — adapt/fork/edit skills already show lineage; imports will start
  showing it the moment we write it.

Two concrete defects this must fix:

1. **Market source identity is lost.** `preview_market_skill`
   (`commands.rs:1019`) delegates to `preview_github_import` /
   `preview_local_import` and keeps no record of `providerId` / `externalId`.
2. **ASI staged-content records a dead temp path.** When Agent Skills Index
   returns `skill_md_content`, the flow stages it to a `tempfile::tempdir()` and
   calls `preview_local_import`, so `Source` becomes `Local { path: <temp dir> }`
   — a path deleted right after import. ASI installs currently record a useless
   source.

## Decisions To Ratify

**D1 — `sourceRev` vs overloading `sourceHash`.** Add `sourceRev: Option<String>`
for the pinned upstream git commit (GitHub path has it via `Source::GitHub.rev`).
Keep `sourceHash` as the content-identity hash and make it `Option` (raw-URL /
ASI-staged content have no git rev → fall back to content hash; GitHub can
populate both). _Recommend: add `sourceRev`, make `sourceHash` optional._

**D2 — how market identity reaches the sidecar.** Keep `skillsmgr-fetch`
market-agnostic. Store an `Option<MarketOrigin { provider_id, external_id,
upstream_url }>` in `AppState` next to `pending_import`, set by
`preview_market_skill`, consumed by `install`. Do **not** add a `Market` variant
to `ImportSource` (it would leak market concepts into the generic fetch crate).
_Recommend: MarketOrigin in src-tauri AppState._

**D3 — fix the ASI temp-path source.** Capture the upstream URL (the ASI detail
response's `github_url` when present, else the ASI skill detail URL) into
`MarketOrigin.upstream_url`, regardless of whether the flow staged content or
fell through to GitHub. The sidecar records the upstream URL, never the temp dir.
_Recommend: yes._

**D4 — no backfill.** Already-installed skills have no source on disk; do not
fabricate one. Lineage stays `None` and the UI shows "source unknown".
_Recommend: yes._

**D5 — write the sidecar in the `install` command, not the service.** Reuse and
extend `write_lineage_sidecar` from the command layer after
`install_from_candidate` returns, writing into each `installation.on_disk_path`.
Keeps lineage/market concepts out of `skillsmgr-service`, and the command layer
holds both the real `candidate.artifact.source` and the `MarketOrigin`.
_Recommend: yes._

## Lineage Model (after D1)

`LineageDto` (camelCase at the serde boundary):

| field | type | meaning |
|---|---|---|
| `sourceKind` | string | `market` \| `github` \| `url` \| `local` \| `fork` \| `adaptation` \| `edit` |
| `providerId` | string? | market only: `skillsmd` / `agent-skills-index` |
| `externalId` | string? | market only: provider id (e.g. `owner/repo`) |
| `sourceUrl` | string? | repo / raw / upstream URL |
| `sourceRev` | string? | pinned upstream git commit (GitHub path) |
| `sourceHash` | string? | content sha256 — **now optional** |
| `sourceTool` | string? | existing — fork/adapt source tool |
| `sourcePath` | string? | existing — local source path |
| `parentName` | string? | existing — **now optional** (imports have no parent) |
| `fetchedAt` | string? | RFC3339 install/fetch time |

`parentName` and `sourceHash` move from required to optional — the only
backward-compat change for the existing adapt/fork/edit writers.

## Acceptance Criteria

> **Implemented & verified 2026-06-08** (`cargo fmt`/`cargo test --workspace`/
> `npm run build` all green). Two refinements surfaced during implementation:
> 1. Provenance is read from the **`ImportSource`** on the pending preview plus
>    `stage.resolved_commit_sha`, **not** `candidate.artifact.source` —
>    `stage_local_path` copies local sources into a temp dir, so the artifact
>    source's `Local` path is the staged temp dir, not the user's real path.
>    `ImportSource` carries the real user path / upstream URL.
> 2. `build_import_lineage` returns `LineageDto` (not `Option`) — every
>    `ImportSource` variant yields provenance; `Bundled`/`Unknown` never occur on
>    the import path. The source-badge **UI** remains a thin follow-up; the
>    fields are surfaced on the inventory DTO via the existing
>    `read_lineage_sidecar`.

- [x] `LineageDto` generalized per the table; `parentName` / `sourceHash`
      optional; mirrored by hand in `src/types.ts`.
- [x] The `install` confirm command writes `.m-skills.json` into every
      successful `installation.on_disk_path` (best-effort; a sidecar failure
      never flips a successful install to failed).
- [x] GitHub import sidecar records `sourceKind:"github"`, `sourceUrl`, and
      `sourceRev` (the resolved commit SHA).
- [x] Raw-URL import records `sourceKind:"url"` + `sourceUrl`.
- [x] Local import records `sourceKind:"local"` + `sourcePath` (real user path,
      not the staged temp dir).
- [x] Market install records `sourceKind:"market"`, `providerId`, `externalId`,
      and a real `sourceUrl` — never a temp dir, including the ASI
      staged-content path.
- [x] Existing adapt/fork/edit lineage writing still passes (optional-field
      migration is backward-compatible); existing tests stay green.
- [x] Inventory surfaces the new fields so a "from SkillsMD" / source badge can
      render from `LineageDto` (badge styling is a thin follow-up).
- [x] Tests: `build_import_lineage` per source kind (github / url / local /
      market-skillsmd / market-asi-staged), on-disk round-trip, and old-sidecar
      deserialization compatibility.

## Out Of Scope

- Update detection / diff against `sourceRev` — Stage 2.1. This issue is its
  prerequisite, not its delivery.
- The visual source badge styling — fields land here; rendering is a small
  follow-up that can ship separately.
- Backfilling provenance for already-installed skills (D4).

## Blocked By

Nothing. All dependencies are already in the tree: Issue 015's market path,
`write_lineage_sidecar`, and `Source::GitHub { url, rev }` with the resolved SHA.
