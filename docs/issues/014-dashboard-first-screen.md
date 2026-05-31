# Issue 014: Dashboard First Screen

## What To Build

Add a first-screen dashboard that lets users understand the health of their
local AI artifact library before drilling into the existing inventory list. The
dashboard should answer:

- What is installed, by artifact kind and target tool?
- Which tools are available or missing on this machine?
- What needs attention before the user imports, adapts, rewrites, or installs
  more artifacts?
- Which safe next action should the user take now?

This is not a marketing landing page and not a cloud analytics dashboard. It is
a dense local operations view for managing files safely. Filesystem state stays
the source of truth; registry data may enrich the dashboard, but must not become
the only source of truth.

## Product Metric Framework

North Star metric:

| Metric | Definition | Data Source | Visualization | Target | Alert Threshold |
| --- | --- | --- | --- | --- | --- |
| Ready compatible artifacts | Count of `ArtifactGroup` rows with at least one owned installation and no high-risk compatibility issue for installed/visible targets | `scan` + deterministic compatibility review | Large number with trend once registry history exists | User can see a non-zero ready library after scan | Drops to 0 after a scan, or scan has blocking errors |

Input metrics:

| Metric | Definition | Data Source | Visualization | Target | Alert Threshold |
| --- | --- | --- | --- | --- | --- |
| Tool coverage | Available adapters / known adapters | `InventoryDto.adapters` | Tool status strip | At least one writable target available | No writable target is available |
| Install coverage | Owned installations grouped by target and kind | `InventoryDto.groups[].installations` where `provenance == "owned"` | Stacked compact bars or table | User can see where every artifact lives | Same artifact installed in conflicting locations or no owned installs |
| Risk queue | Count of groups with compatibility warnings, missing body for Skill review, read-only target, or scan error | `InventoryDto`, backend compatibility DTOs, scan errors | Attention list | 0 high-priority items | Any conflict/incompatible review or scan error |
| Import readiness | Whether import flow can offer at least one compatible target for each artifact kind | `available_targets`, `Target::supports_kind` | Small matrix | Compatible targets visible before import | Selected kind has no compatible target |
| Recent safe actions | Last successful install, uninstall, draft confirm, or scan | Registry ledger + Tauri events in later slice | Activity list | User sees recent changes | Last write failed or registry differs from disk |

Health metrics:

| Metric | Definition | Data Source | Visualization | Target | Alert Threshold |
| --- | --- | --- | --- | --- | --- |
| Scan health | Scan completed, partial errors, and adapter presence | `InventoryDto.errors`, `AdapterStatusDto` | Banner and status row | Scan completes without errors | Any scan error |
| Registry agreement | Registry installations that still exist on disk / registry installations | Registry + scan comparison | Badge, later detail view | 100% agreement | Any registry-only installed row |
| Rewrite readiness | LLM provider configured and usable for draft-only rewrite flows | `get_translate_config` | Configuration badge | Clear configured/not configured state | User opens rewrite action without provider configured |

Business metric proxy:

| Metric | Definition | Data Source | Visualization | Target | Alert Threshold |
| --- | --- | --- | --- | --- | --- |
| Safe conversion funnel | Import preview -> reviewed -> confirmed install, stored locally only | Future event ledger | Funnel | Improve completion without reducing review acknowledgements | Confirm installs happen without review, or high-risk installs bypass acknowledgement |

## Dashboard Layout

The dashboard should become the default first content when no artifact is
selected. The sidebar and existing artifact list stay available.

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Local Library                                                       │
│ Ready compatible artifacts: 24        Scan: OK · 5 tools available  │
├──────────────────────┬──────────────────────┬──────────────────────┤
│ Skills               │ Extensions           │ Workflows             │
│ 21 installed          │ 3 installed          │ 0 managed             │
│ Claude/Codex/etc.     │ Gemini               │ deferred              │
├──────────────────────────────────────┬──────────────────────────────┤
│ Tool Coverage                         │ Needs Attention             │
│ claude-code  available  8 skills      │ 2 compatibility warnings    │
│ codex        available  11 skills     │ 1 missing tool directory    │
│ gemini       available  3 extensions  │ 0 high-risk imports pending │
├──────────────────────────────────────┴──────────────────────────────┤
│ Recent Safe Actions                                                  │
│ Installed foo to codex · Forked bar as custom skill · Scan completed  │
└─────────────────────────────────────────────────────────────────────┘
```

Navigation behavior:

- Sidebar gets a Dashboard item above artifact kind filters.
- Clicking Dashboard clears `selectedKind` and `selectedName`, then renders
  `DashboardPanel`.
- Clicking a dashboard metric filters the existing artifact list when the metric
  has a natural drill-down, such as `Skill`, `Extension`, or a target tool.
- Empty inventory shows the dashboard with a prominent import action instead of
  the current standalone empty state.

## Data Model

Add a backend command instead of deriving dashboard compatibility rules in the
frontend:

```ts
type DashboardMetricSeverity = "ok" | "info" | "warning" | "critical";

type DashboardKindSummaryDto = {
  kind: ArtifactKind;
  groups: number;
  ownedInstallations: number;
  visibleInstallations: number;
  compatibleTargets: TargetDto[];
};

type DashboardToolSummaryDto = {
  adapterId: string;
  available: boolean;
  writable: boolean;
  supportedKinds: ArtifactKind[];
  ownedInstallations: number;
  missingReason?: string;
};

type DashboardAttentionItemDto = {
  severity: DashboardMetricSeverity;
  title: string;
  body: string;
  action: "open_settings" | "open_import" | "filter_kind" | "select_artifact" | "none";
  kind?: ArtifactKind;
  artifactName?: string;
};

type DashboardDto = {
  generatedAt: string;
  readyArtifactGroups: number;
  totalGroups: number;
  totalOwnedInstallations: number;
  scanErrors: string[];
  kindSummaries: DashboardKindSummaryDto[];
  toolSummaries: DashboardToolSummaryDto[];
  attentionItems: DashboardAttentionItemDto[];
};
```

Suggested Tauri command:

- `get_dashboard(cwd?: string): Promise<DashboardDto>`

Implementation note: `get_dashboard` may call the same service scan path as
`scan` for Batch 1, but the DTO should be separate so the frontend does not
grow product rules. Later, it can combine scan output with registry ledger
history without changing the UI contract much.

## Implementation Plan

Batch 1 - Read-only dashboard from scan:

- Add backend dashboard DTOs in `src-tauri/src/dto.rs`.
- Add `dashboard` service helper that consumes `Inventory` and adapter
  capabilities.
- Add Tauri command `get_dashboard`.
- Add TypeScript DTOs and API wrapper in `src/types.ts` and `src/api.ts`.
- Add `DashboardPanel.tsx` and route it from `App.tsx`.
- Add i18n strings in `src/locales/en` and `src/locales/zh`.
- Keep compatibility and target support decisions in Rust.

Batch 2 - Attention and drill-down:

- Add attention items for scan errors, missing writable targets, unsupported
  artifact kinds, and read-only tools.
- Make metric clicks filter kind/tool in the existing list.
- Show clear recovery actions: import, settings, rescan, or select artifact.
- Keep high-risk import items out of this batch unless pending import state is
  intentionally persisted.

Batch 3 - Registry history and trends:

- Add a local event ledger for scan completion, installs, uninstalls, draft
  confirms, rewrite draft generation, and failed writes.
- Add small trend deltas for ready artifacts, installs by target, and failed
  operations.
- Add registry-vs-filesystem agreement checks.
- Keep event data local by default; do not introduce external telemetry.

## UX Requirements

- The dashboard must feel like an operations console: compact, scannable, and
  useful on repeated daily use.
- Do not use hero sections, marketing cards, or explanatory onboarding copy.
- Cards are allowed for metric tiles and repeated attention items only.
- Use existing dark UI conventions and restrained color. Reserve yellow/amber
  for caution and red for critical issues.
- Every warning should have a concrete next action or a reason why it is
  informational only.
- The dashboard must not hide the artifact list; it should help the user choose
  what to inspect next.

## Acceptance Criteria

- [ ] App opens to Dashboard when no artifact is selected.
- [ ] Dashboard summarizes artifact counts by kind and owned installation counts
      by tool.
- [ ] Dashboard shows adapter availability and distinguishes missing tools from
      tools with zero installed artifacts.
- [ ] Dashboard shows scan errors and at least one actionable attention item
      when errors exist.
- [ ] Dashboard does not implement compatibility rules in React.
- [ ] Dashboard actions can open import/settings or filter/select existing
      inventory without writing files.
- [ ] Empty inventory still offers a safe import path.
- [ ] TypeScript, Rust DTOs, command registration, and i18n strings are updated.
- [ ] Tests cover dashboard summary construction from temp-directory scan data.
- [ ] `cargo fmt --all -- --check`, `cargo test --workspace`, and
      `npm run build` pass before handoff.

## Non-Goals

- No external analytics or telemetry.
- No marketplace/search candidate dashboard in this issue.
- No background update checks.
- No install/write action from a metric tile.
- No replacement for the existing detail panel, import wizard, custom editor, or
  compatibility review UI.

## Open Questions

- Should dashboard counts include `alsoVisibleTo` as coverage, or only owned
  installations?
- Should Hermes/openclaw read-only installs appear in the same tool coverage
  table, or in a separate read-only section?
- Should scan run automatically when opening Dashboard, or should it rely on the
  app-level inventory query already running in `App.tsx`?
- When registry history exists, how long should dashboard trends look back: last
  24 hours, 7 days, or since last app launch?
