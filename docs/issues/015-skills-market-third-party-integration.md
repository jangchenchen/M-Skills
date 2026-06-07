# Issue 015: Skills Market Third-party Integration

## What To Build

Add a left-sidebar `Skills Market` module that makes the market strategy
explicit: M-Skills should not operate its own marketplace. It should connect to
third-party Skill registries only when the provider publishes an allowed
integration path, then route every selected Skill back through the existing
preview, audit, compatibility review, and confirm-before-write flow.

The shipped UI is a search-and-preview integration panel: it queries the
allowed sources and routes any selected skill back through the existing
preview → audit → compatibility → confirm flow. There is no separate write
path — selection reuses the import wizard's confirm step.

## Confirmed Candidate Sources

| Source | Access status | Why it is eligible | First integration shape |
| --- | --- | --- | --- |
| SkillsMD | API OK | Public REST API, no auth, documented CORS, cache guidance. | Read-only searchable source; resolve result to raw `SKILL.md`, then use existing import preview. |
| Agent Skills Index | API OK | Public API with search, categories, detail, rate limit, and `SKILL.md` content in detail responses. | Broad discovery and category browsing; stage fetched content before audit. |
| Agensi | MCP path | Marketplace access is documented through an MCP server with search/detail/install tools and account state. | Future MCP connector; do not allow its install tool to write directly. |
| LarryBrain | API gated | Public search and free skill file download are available; premium access requires API key. | OpenClaw-oriented source after target compatibility rules are verified. |

References checked:

- `https://skillsmd.dev/api.html`
- `https://agentskillsindex.com/fr/docs/api`
- `https://www.agensi.io/mcp`
- `https://www.larrybrain.com/docs/api-reference`

## Product Rules

- Do not iframe or webview-wrap a third-party marketplace unless the provider
  explicitly allows embedding.
- Prefer API or MCP access that is publicly documented.
- Treat every fetched listing, `SKILL.md`, or marketplace response as untrusted
  import input.
- Never write directly from a market result to a target tool directory.
- Market selection must route to one of:
  - raw URL import preview,
  - staged draft preview,
  - future MCP connector preview.
- Always re-run deterministic compatibility/risk review after fetching the
  candidate.

## Acceptance Criteria

- [x] Sidebar has a `Skills Market` module.
- [x] Selecting it opens a dedicated market view, not a local artifact filter.
- [x] The view explains that M-Skills is wrapping allowed third-party sources,
      not operating its own market.
- [x] The view lists candidate third-party sources and the allowed integration
      path for each.
- [x] A backend command searches the selected provider (`search_market_skills` queries SkillsMD and Agent Skills Index concurrently).
- [x] Search results route into existing raw URL or staged draft import flows (`preview_market_skill` delegates to `preview_github_import` or `preview_local_import`).
- [x] Provider network errors are recoverable and do not affect local inventory (partial results shown with per-provider error banners).

## Follow-up Implementation Notes

Issue 013 already describes the GitHub Code Search path. That remains useful as
an index fallback, but GitHub Code Search is not itself a Skills marketplace.
For the first real market search, start with SkillsMD or Agent Skills Index
because both publish API access that maps directly to a read-only discovery
experience.

## Decisions & Next Steps (2026-06-08)

The MVP shipped every acceptance criterion above: concurrent SkillsMD + Agent
Skills Index search, per-provider error/rate-limit handling, a 60s in-memory
result cache, cross-source dedup, and a `preview_market_skill` path that routes
each selection back through import preview → audit → compatibility → confirm.
The following is the ratified plan for deepening it. Guiding principle: **the
moat is being the safest acquisition entry point, not connecting the most
sources.**

### P0 — to be sliced next (own issues)

**P0a. Generalize install provenance into a reusable lineage model.** Today only
the adapt/fork/edit draft path (`confirm_install_skill_draft` →
`write_lineage_sidecar`) writes `.m-skills.json`. The market/URL/local **import
confirm** path (which consumes `pending_import`) writes no sidecar, so market
installs currently record no source. Close this generically — not
market-specific:

- Extend `LineageDto` (`src-tauri/src/dto.rs`): add `sourceKind: "market"` plus
  `providerId`, `externalId`, `fetchedAt`; reuse existing `sourceUrl` for the
  repo/source URL.
- Record the **resolved upstream rev/hash**. Existing `sourceHash` is a content
  sha256 of the parent SKILL.md; update detection needs the pinned upstream
  commit/ref, which is a *different* value. Prefer a distinct `sourceRev` field
  over overloading `sourceHash`.
- Make the **import confirm path** write the sidecar too, not just the draft
  path. This is the actual missing link, not just the new fields.
- Mirror new fields in `src/types.ts` (hand-synced, no codegen) and surface them
  in the inventory DTO so the library can show a "from SkillsMD" badge.

Why first: shared prerequisite for **update detection (Stage 2.1)**,
**in-library source identification**, and **reinstall/uninstall provenance**.
Small change, unlocks the most downstream.

**P0b. Unify discovery: route Smart Add NL queries to `search_market_skills`
first; demote GitHub search to fallback. — ✅ Shipped 2026-06-08.** Smart Add now
runs the classified `searchQuery` through `search_market_skills` first
(`ImportWizard.tsx::runSmartAddSearch`); curated market candidates render
directly, and GitHub Code Search (Issue 013) is hit only as a fallback when the
market returns nothing. Market candidates preview via `previewMarketSkill()`,
GitHub fallback results via `previewImport()`; per-provider partial errors
surface in the results area rather than being swallowed by the fallback.
Rationale was not only UX closure — `search_github_skills` is fundamentally repo
search with worse signal-to-noise and weaker safety pre-signals than curated
registries. The two discovery systems are now one.

### P1 — deepen the moat

**P1a. Cross-source trust aggregation.** `merge_and_dedup` keeps one result but
discards the signal that a skill was indexed by more than one source. Add
`sourceCount` / `providerIds` to the candidate and weight ranking by it.

**P1b. Risk pre-signal, scoped to preview.** Surface deterministic risk signals
(shell commands, network access, file writes, frontmatter anomalies) at the
**top of preview / in an expanded result row** — not as a card badge. Card-level
risk would require per-result content fetch (N+1) and the card has no
content-level audit data today.

### P2

**Disk-persistent cache + "last synced" + offline browse.** Current cache is
in-memory, 60s TTL (`MarketSearchCache`, `state.rs`); lost on restart. A
desktop-appropriate refinement, lower priority than the above.

### Separate track — needs its own issue

**Agensi MCP connector.** Makes M-Skills an MCP client (new architectural
direction). Strictly a **discovery/preview connector**: its `install` tool must
never bypass confirm-before-write. Slice as its own issue, not folded in.
