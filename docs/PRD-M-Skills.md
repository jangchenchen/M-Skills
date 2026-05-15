# PRD: M-Skills

## 1. Summary

M-Skills is a desktop manager for AI agent skills, Gemini CLI extensions, and Warp workflows. It helps users see what is installed, import new artifacts safely, and install them only into compatible tools.

The first version focuses on skills and extensions. It keeps workflows separate because Warp workflows are command templates, not agent skills.

## 2. Contacts

| Name | Role | Comment |
| --- | --- | --- |
| Product owner | Decision maker | Owns scope and cross-tool behavior decisions. |
| Desktop engineer | Backend and Tauri owner | Owns Rust domain model, adapters, scan/import flows, and packaging. |
| Frontend engineer | UI owner | Owns React interface, install wizard, artifact detail panel, and audit view. |
| Security reviewer | Safety owner | Reviews import audit, path handling, and command-execution risk. |

## 3. Background

Agent tooling is converging on reusable local artifacts, but the artifacts are not all the same thing. Claude Code, Codex CLI, opencode, openclaw, and Hermes mostly use an AgentSkills-style `SKILL.md`. Gemini CLI uses extensions with `gemini-extension.json`, commands, context, and optional MCP config. Warp uses YAML workflows that describe parameterized terminal commands.

Treating all seven tools as one model would create bad installs. A `SKILL.md` copied into Warp is not useful. A Gemini extension copied into Claude Code is not a skill. M-Skills must model the artifact kind first, then offer only compatible install targets.

Why now: AI coding tools are becoming multi-agent and multi-client. Users increasingly collect skills from GitHub and want a single place to inspect, import, update, and remove them without memorizing every tool's filesystem layout.

## 4. Objective

The objective is to make local AI artifact management safe, visible, and predictable across major developer tools.

Success means a user can answer three questions quickly:

- What skills or extensions do I have installed?
- Where is each artifact installed?
- Can I import this GitHub repo without installing it into the wrong tool?

Key results for MVP:

- Scan and display installed Claude Code, Codex CLI, opencode, and Gemini CLI artifacts across global and project scopes.
- Import a GitHub or local directory after sniffing its artifact kind and showing a safety audit.
- Install and uninstall compatible artifacts without corrupting existing user files.
- Keep a SQLite cache that can be rebuilt from disk, with filesystem state treated as the source of truth.
- Ship adapter tests that use temporary directories for all MVP tools.

## 5. Market Segments

Primary users are developers who use more than one AI coding client and want shared capabilities across them.

Important jobs:

- Keep personal skill libraries organized.
- Try third-party skills from GitHub without losing track of where they were installed.
- Move between Claude Code, Codex CLI, opencode, and Gemini CLI without duplicating manual setup work.
- Audit imported instructions before allowing an agent to use them.

Constraints:

- Users may already have custom files in tool directories.
- Some tools support native enable/disable controls; others only support file presence.
- Tool behavior changes over time, so adapters must isolate tool-specific rules.

## 6. Value Propositions

M-Skills reduces setup friction by turning scattered filesystem conventions into one visible inventory.

It reduces risk by sniffing artifact kind before install and blocking incompatible targets. It also reduces silent supply-chain risk by requiring an audit page for imported artifacts before installation.

It gives power users control without hiding the underlying files. Installs are regular directories on disk, and the SQLite database is a cache plus installation ledger, not the only source of truth.

## 7. Solution

### 7.1 UX and User Flows

The app has four main areas:

- Sidebar: artifact categories, install targets, and detected tool availability.
- Artifact list: grouped by artifact kind and install status.
- Detail panel: metadata, source, installed targets, on-disk paths, and warnings.
- Add/import wizard: URL or local path input, sniff result, safety audit, target selection, and install progress.

Startup flow:

1. App starts and detects adapters.
2. Service scans each adapter concurrently.
3. Registry upserts artifacts and installations.
4. Frontend receives a grouped artifact view.

Import flow:

1. User provides a GitHub URL or local path.
2. App fetches or opens the source in a temporary/import staging area.
3. Parser sniffs artifact kinds:
   - `SKILL.md` means `Skill`.
   - `gemini-extension.json` means `Extension`.
   - Warp workflow YAML means `Workflow`.
4. App shows a safety audit and compatible targets only.
5. User confirms install.
6. Service calls the selected adapters and updates the registry.

Uninstall flow:

1. User removes one installation, not necessarily the artifact everywhere.
2. Service routes to the owning adapter.
3. Adapter removes only files it owns or safely recognizes.
4. Registry marks the installation removed after filesystem success.

### 7.2 Key Features

Artifact model:

- `Skill`: AgentSkills-style directory with `SKILL.md` and YAML frontmatter.
- `Extension`: Gemini CLI extension directory with `gemini-extension.json`.
- `Workflow`: Warp YAML workflow file.

Targets:

- `ClaudeCode` supports `Skill`.
- `Codex` supports `Skill`.
- `Opencode` supports `Skill`.
- `Openclaw` supports `Skill`, but MVP is read-only unless disable/install semantics are confirmed.
- `Hermes` supports `Skill`, but MVP is read-only unless disable/install semantics are confirmed.
- `Gemini` supports `Extension`.
- `Warp` supports `Workflow`, deferred to a later workflow tab.
- `SharedGlobal` represents `~/.agents/skills` as the shared A-class global target.

MVP scope:

- Build scan, install, uninstall, and import for Claude Code, Codex CLI, opencode, and Gemini CLI.
- Build read-only scan for Hermes and openclaw only if time allows.
- Defer Warp workflow management to v1.2.
- Defer cross-tool disable toggles except where a tool has clear native semantics.

Non-goals for MVP:

- No marketplace publishing.
- No automatic background updates.
- No silent install from remote repositories.
- No attempt to normalize every artifact into one universal schema.

### 7.3 Technology

Desktop shell:

- Tauri 2.x with a Rust backend.
- React, TypeScript, Tailwind, and shadcn/ui for the frontend.
- TanStack Query for frontend state, because backend and filesystem state are authoritative.

Rust backend:

- `skillsmgr-core`: domain types, errors, adapter traits.
- `skillsmgr-parse`: parsers for `SKILL.md`, Gemini extension manifests, and Warp workflows.
- `skillsmgr-adapters`: per-tool adapter implementations.
- `skillsmgr-scan`: filesystem scan and notify watchers.
- `skillsmgr-fetch`: GitHub/local import staging.
- `skillsmgr-registry`: SQLite cache and installation ledger.
- `skillsmgr-service`: application orchestration.
- `skillsmgr-tauri`: Tauri IPC commands and app entrypoint.

Infrastructure:

- `tokio` for async runtime.
- `notify` for filesystem watching.
- `gix` preferred for Git fetches to avoid C dependencies.
- `serde_yaml` and `toml_edit` for config parsing/writing.
- `SQLite` and `sqlx` for the local registry.
- `tempfile`, `rstest`, and `insta` for tests.

### 7.4 Assumptions

- Most A-class users will accept `~/.agents/skills` as the preferred shared global install target when the tool supports reading it.
- Users prefer safe import and explicit target choice over one-click silent installation.
- GitHub imports should pin to a commit SHA in v1, with explicit update later.
- Same-name artifacts from different sources should be treated as conflicts in v1.
- The database can always be rebuilt from disk plus source metadata where available.

Open questions:

- Should "install to all A-class tools" write once to `~/.agents/skills`, or copy into each native directory for clearer per-tool ownership?
- What exact disable semantics should M-Skills expose per tool?
- Should imported skills run through an automated risk scanner before the human audit page?
- How should project-scope installs be selected when the app is launched outside a repository?

## 8. Release

MVP can be built in roughly six weeks if scope stays narrow.

Version 0.1:

- Rust workspace with domain model, parser, and adapter trait.
- Filesystem scan for Claude Code, Codex CLI, opencode, and Gemini CLI.
- Minimal UI that lists artifacts and installations.

Version 0.2:

- Local directory import.
- GitHub import pinned to commit.
- Safety audit page.
- Install and uninstall for MVP targets.

Version 0.3:

- SQLite registry.
- Tauri events for scan and install progress.
- Error recovery and conflict handling.
- Packaged macOS build.

Later:

- Hermes and openclaw write support after semantics are verified.
- Warp workflow tab.
- Native enable/disable support where safe.
- Update checks and diffs.

## Appendix: Current Tool Facts

The product model is based on three artifact classes:

| Class | Tools | Artifact | Format |
| --- | --- | --- | --- |
| AgentSkills-style | Claude Code, Codex CLI, opencode, openclaw, Hermes | Skill | Directory containing `SKILL.md` with YAML frontmatter. |
| Extension-style | Gemini CLI | Extension | Directory containing `gemini-extension.json`, optional commands, context, and MCP config. |
| Workflow-style | Warp | Workflow | YAML workflow files with parameterized commands. |

Known path targets:

| Tool | Global | Project | MVP support |
| --- | --- | --- | --- |
| Claude Code | `~/.claude/skills/<name>/` | `.claude/skills/<name>/` | Scan, install, uninstall. |
| Codex CLI | `~/.agents/skills/<name>/` | `.agents/skills/<name>/` | Scan, install, uninstall. |
| opencode | `~/.config/opencode/skills/<name>/`, plus shared skill dirs | `.opencode/skills/<name>/`, `.claude/skills/<name>/`, `.agents/skills/<name>/` | Scan, install, uninstall. |
| openclaw | `~/.openclaw/skills/` | `<workspace>/skills/` | Read-only until write semantics are verified. |
| Hermes | `~/.hermes/skills/<category>/<name>/` | External read-only dirs | Read-only until write semantics are verified. |
| Gemini CLI | `~/.gemini/extensions/<name>/` | `.gemini/extensions/<name>/` | Scan, install, uninstall. |
| Warp | OS-specific workflows directory | `.warp/workflows/` | Deferred. |

Notes from current documentation:

- Claude Code documents `skillOverrides`, so disable behavior is not purely file presence for all cases.
- OpenClaw documents agent allowlists for skill visibility, but MVP still treats write/disable behavior as unverified.
- Hermes documents uninstall/reset commands, but MVP should avoid destructive deletion until reversible behavior is designed.
