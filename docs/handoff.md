# M-Skills 交接文档（2026-05-24，更新版）

> 给新窗口的 Claude：**按顺序读 `CLAUDE.md` → 本文件 → `docs/issues/008-smart-add-skill-entry.md`**。三份合起来就是全部需要的上下文。

---

## 1. 5 分钟搞清楚状态

```bash
# 验证工作区健康
cargo fmt --all -- --check         # 应当 0 输出
cargo test --workspace 2>&1 | grep -E "test result|FAILED"   # 全 ok
npm run build 2>&1 | tail -10      # ✓ built

# 看一眼未提交改动
git status --short
git log --oneline -5
```

期望看到的：
- 最近的 commit 是 `4df9f6f Add skill review, rewrite, and summaries`
- 未提交：6 个 `?? docs/issues/00{8,9,10,11,12,13}-*.md`、`?? docs/handoff.md`
- 未提交：~10 个修改文件（Cargo.toml、Cargo.lock、`crates/skillsmgr-fetch/src/lib.rs` 加了 297 行、`src/types.ts` 加了 `Url`/`RawUrl` 变体等）——这些都是 **Issue 009（Raw URL Single-File Import）的实现**

如果以上对得上，状态就是干净的。

---

## 2. 时间线（仓库当前状态）

### 2.1 已提交（commit `4df9f6f`）

**AI Skill 自动总结 + P0 加固**。包含：

- 每个已安装 Skill 在 DetailPanel 顶部展示 "AI Summary" 区块（commands / capabilities / useCases / examples）
- 触发：安装后 `tokio::spawn` fire-and-forget + DetailPanel 打开时 lazy
- 缓存：SQLite 表 `skill_summaries`，键 `(skill_name, source_sha256, locale)`；内容变化自动失效；作用域仅 Skill kind
- **负缓存（`SummaryFailureCache`）**：永久失败（parse 错误、4xx 401/403/404/422）内存中记 10 分钟；瞬时失败（5xx、超时、未配置）不缓存；`set_translate_config` 自动 `clear_all`
- **损坏行自动清理**：缓存 JSON 反序列化失败时自动 evict 并走 LLM 重生成
- 23 个 summary 相关测试全绿

关键文件：
- `src-tauri/src/summary.rs`（新增）
- `crates/skillsmgr-registry/src/lib.rs`（`skill_summaries` 表）
- `crates/skillsmgr-translate/src/lib.rs`（`TranslationManager` proxy 方法）
- `src-tauri/src/dto.rs` / `commands.rs` / `state.rs` / `lib.rs`
- `src/components/DetailPanel.tsx`、`src/api.ts`、`src/types.ts`
- `src/locales/{en,zh}/{artifact,errors}.json`

### 2.2 未提交（本会话余下产物）

**A. 5 个 Smart Add 切片的 issue 规划文档**（`docs/issues/008-013`）：

| Issue | 标题 | 依赖 | 实现状态 |
|---|---|---|---|
| 008 | Smart Add Skill Entry（父） | 007 | 文档完成 |
| 009 | Raw URL Single-File Import | 008 | **实现完成，未提交** |
| 010 | Smart Add Input And Routing（前端 + 路由） | 009 | 待实现 |
| 011 | NL Skill Install Intent Classifier | 010 | 待实现 |
| 012 | Unified Import Risk Review | 010 | 待实现 |
| 013 | GitHub Skill Search MVP | 011 + 012 | **延后到 Stage 3** |

依赖图：`009 → 010 → 011 → 013`、`010 → 012 → 013`。

**B. Issue 009 的实现代码**（已通过 fmt + test + build）：

- `Cargo.toml`、`Cargo.lock` — 工作区依赖调整
- `crates/skillsmgr-core/src/lib.rs` — `Source::Url { url }` 变体（+1 行）
- `crates/skillsmgr-fetch/Cargo.toml` + `src/lib.rs` — **+297 行**主实现：fetch、sniff、size limit、content-type 白名单、staging、audit
- `crates/skillsmgr-registry/src/lib.rs` — Source URL 序列化（+1 行）
- `crates/skillsmgr-service/src/lib.rs` — preview_import 路由（+11 行）
- `crates/skillsmgr-translate/Cargo.toml` — 1 行 dep 微调
- `src-tauri/src/commands.rs` — `preview_import` 调度（+11 行）
- `src-tauri/src/dto.rs` — `SourceDto::Url`（+4 行）
- `src/types.ts` — `SourceDto::Url` + `ImportSourceDto::RawUrl` 变体

**C. 本文件 `docs/handoff.md`**

### 2.3 推荐提交分组（按 `CLAUDE.md` 的 git 约定，不加 Co-Authored-By）

建议拆成 3 个 commit，避免一个大杂烩：

```
Add Smart Add roadmap issue docs

Document the Smart Add Skill Entry feature as a parent issue (008) with
five sliced child issues covering raw URL import (009), smart input and
routing (010), natural-language intent classification (011), unified
import risk review (012), and a deferred GitHub Code Search MVP (013).
Dependencies are 009 → 010 → 011, with 012 also depending on 010 and
013 depending on 011 and 012.
```

```
Import single-file Skills from raw HTTPS URLs

Extend the import pipeline so any HTTPS URL pointing to a SKILL.md or
gemini-extension.json can be staged into a temp directory and run
through the existing sniff, audit, compatibility, and preview flow.
GitHub repo URLs still go through the existing GitHub import path; only
non-GitHub HTTPS URLs go through the raw fetch. Enforces a 1 MB size
limit and restricts content types to text/plain, text/markdown, and
application/json. Source metadata records the original URL without
pretending it is a GitHub repository.
```

```
Document next-session handoff state

Capture the post-summary roadmap, the Smart Add issue structure, and
the current uncommitted state so a fresh Claude session can pick up
without re-deriving context.
```

如果不想把 handoff 进库，第 3 个 commit 可省略，文件保留在工作区或移到 `.claude/` 之外。

---

## 3. 接下来要做的事

**当前明确下一项：Issue 010**（Smart Add Input And Routing）

010 是 Smart Add 流程的前端落地。规格在 `docs/issues/010-smart-add-input-and-routing.md`，要点：

- 强制先选 target tool(s)，未选不让继续
- 输入框实时分类 URL / Local file / Ask AI
- URL 走 Issue 009 的 `preview_import`（已实现）
- Local 走 `preview_import` 的本地路径
- Ask AI 在 LLM 未配置时显示"不可用 + 引导到 Settings"
- 暂时不连市场搜索（那是 011 + 013 的事）

实现位置：
- 新组件 `src/components/SmartAddSkillInput.tsx`（替代或包裹现有 `src/components/ImportWizard.tsx`）
- 复用现有 `src/api.ts::previewImport`
- 在 Tauri 侧加路径存在性检查命令（如 `check_path_exists(path)`）让前端能区分 file vs NL
- i18n key 加到 `src/locales/{en,zh}/wizard.json`（已有）

**完成 010 后**，自然进 011（NL 意图分类器）。

---

## 4. Issue 010 实施前需澄清的点

010 文档的 Acceptance Criteria 已经定义了行为契约，但有几个 UX 细节没拍板，新窗口应该问用户：

1. **"target 先选"在 UI 里的位置**：modal 顶部 chip 选择 vs 进入流程前一个独立步骤
2. **是替换现有 ImportWizard 还是新增并存**：现有 wizard 处理 GitHub URL + 本地路径已经能用，新组件如果替换，要确保 GitHub URL 走 009 已扩展的 `preview_import` 路径
3. **Ask AI 模式在 LLM 未配置时的视觉**：disabled 按钮 + tooltip vs 显眼的引导卡片
4. **GitHub SSH URL（`git@github.com:user/repo.git`）的处理**：算 URL 还是当成 NL

这些都是小决策，可在写第一版后边调整。

---

## 5. 整体 roadmap 重申

| 阶段 | 子项 | 状态 | 工作量 |
|---|---|---|---|
| **1：本地 1.0** | 1.1 仪表盘首屏 | pending | 2 周 |
| | 1.2 安全 UI 补全（audit warnings 展示） | pending | 1 周 |
| | 1.3 enable/disable 全工具兜底 | pending | 1 周 |
| | 1.4 一键全工具安装 | pending | 1 周 |
| | 1.5.1 = Issue 009 通用 URL 导入 | ✅ 实现完成（未提交） | — |
| | 1.5.2 = Issue 010 智能输入 + 路由 | pending | 1 周 |
| | 1.5.3 = Issue 011 NL 意图分类 | pending | 1 周 |
| | 1.5.5 = Issue 012 风险统一展示 | pending | 0.5 周 |
| **2：演化** | 2.1 更新检测 + 回滚（Item 9） | pending | 4 周 |
| | 2.2 跨工具适配补全（非 Claude→Codex 单方向） | pending | 1 周 |
| | 2.3 本地遥测埋点基础 | pending | 1 周 |
| **3：走出本地** | 3.1 = Issue 013 GitHub Search MVP + 发现页 | pending | 6-8 周 |
| | 3.2 模板化 skill（Item 12） | pending | 4-6 周 |

商业化判断（用户原话）："软件门槛低，内容/插件/创意才是关键"——阶段 3 是从"管理工具"转"分发平台"的拐点。

---

## 6. 整体 roadmap 还未澄清的 4 个决策（不阻塞当前 010，但早晚要拍）

| 项 | 决策 | 推荐 |
|---|---|---|
| Item 2 | Claude Code / opencode / Hermes 没有 native disable，怎么禁用？ | `.m-skills.json` 加 `disabled: true` + 文件名加 `.disabled` 后缀双保险 |
| Item 9 | 更新检测频率、联网默认、备份数量 | 启动 + 每 24h；默认 opt-in；每 skill 留最近 3 个版本 |
| Item 11 | 市场数据源 | 见 Issue 013：GitHub Code Search 作 MVP |
| Item 12 | 模板化抽象深度 | A（fork 时变量替换）→ B（frontmatter `parameters` 段）渐进；C（模板+instance 继承）视采纳率再说 |

Item 1（Smart Add）的 4 个决策**已经在 Issue 008 的 Decisions 段落里拍定**——不要再问用户。

---

## 7. 代码导航 cheat sheet

### 后端关键路径

```
src-tauri/
├── src/
│   ├── lib.rs                Tauri 入口、AppState 装配、命令注册
│   ├── state.rs              AppState（含 summary_failures）
│   ├── commands.rs           所有 Tauri commands（含 summary_core）
│   ├── dto.rs                所有 DTO（camelCase serde 边界）
│   ├── compatibility.rs      确定性兼容性引擎
│   ├── review.rs             LLM 导入冲突 review
│   ├── rewrite.rs            LLM SKILL.md 改写（intent.rs 的模板）
│   └── summary.rs            AI 总结 + SummaryFailureCache + is_permanent_failure
crates/
├── skillsmgr-core/           Result/Error、Artifact、Target、Capability
├── skillsmgr-adapters/       per-tool ToolAdapter；DirectoryLayout 模板
├── skillsmgr-parse/          SKILL.md / gemini-extension.json / Warp YAML
├── skillsmgr-fetch/          Git/URL/本地导入；ImportAudit；**Issue 009 raw URL 在此**
├── skillsmgr-registry/       SQLite 缓存；含 skill_summaries 表
├── skillsmgr-service/        Service::with_home、inventory、install 编排
├── skillsmgr-scan/           并发扫描（JoinSet）
└── skillsmgr-translate/      LLM provider + keyring + TranslationManager
```

### 前端关键路径

```
src/
├── api.ts                    Tauri invoke 包装
├── types.ts                  DTO 类型（含新 Url/RawUrl 变体）
├── components/
│   ├── DetailPanel.tsx       含 AI Summary section
│   ├── ImportWizard.tsx      ← Issue 010 改动点
│   ├── CustomSkillEditor.tsx
│   ├── SkillPreviewModal.tsx
│   └── CompatibilityNotice.tsx
├── locales/{en,zh}/
│   ├── artifact.json
│   ├── errors.json
│   ├── wizard.json           ← Smart Add 文案放这里
│   └── settings.json
└── useErrorMessage.ts        ErrorDto → 用户文案
```

### 文档

```
docs/
├── PRD-M-Skills.md           权威 PRD
├── handoff.md                ← 本文件
└── issues/
    ├── 001-006               基础 / inventory / 导入 / 安装 / registry / package
    ├── 007                   Skill review/adapt/customize（前置完成）
    ├── 008                   Smart Add 父 issue（含决策）
    ├── 009                   Raw URL 单文件导入（实现完成）
    ├── 010                   Smart Add 输入与路由（下一项）
    ├── 011                   NL 意图分类
    ├── 012                   统一风险展示
    └── 013                   GitHub Search MVP（延后 Stage 3）
```

### 关键不变式（CLAUDE.md 已有，强调一下）

- **文件系统是真相，registry 是缓存**
- **`Artifact.id` 每次扫描新生成**，分组用 `(kind, name)`
- **Inventory 行 = ArtifactGroup**；命令/能力是 group 子项
- **Diff-first / preview-confirm** 所有 skill 草案路径
- **兼容性逻辑只在 `src-tauri/src/compatibility.rs`**，前端不做判断
- **LLM 仅生成草案**；源/用户输入全 fenced 为 untrusted
- **LLM 永远不能直接触发安装**（Smart Add NL 路径的安全基石——参考 Issue 011 决策）
- **`.m-skills.json` 写在 skill 目录侧**（lineage sidecar），不放进 SKILL.md frontmatter
- **测试用 `tempfile::tempdir()`**，绝不读真实 `$HOME`

---

## 8. Pre-commit 契约（每次提交前必跑）

```bash
cargo fmt --all -- --check
cargo test --workspace
npm run build           # tsc -b && vite build
```

Git 约定：plain commit message（subject + 可选 body）；**不加 Co-Authored-By trailer**。

---

## 9. 推荐开工顺序（给新窗口）

1. 跑 §1 的验证命令确认状态
2. 决定要不要先把 §2.3 的 3 个 commit 推进去（推荐**先提交**——issue 文档和 009 实现是两个干净的逻辑分组，handoff 是可选第三个）
3. 读 `docs/issues/010-smart-add-input-and-routing.md`，按 §4 询问用户 4 个 UX 细节
4. 用 `EnterPlanMode` 写一份具体 plan（参考 `/Users/chen/.claude/plans/federated-dreaming-glacier.md` 格式——那是 AI Summary 功能的 plan）
5. 实施 010；完成后接着 011 → 012；013 暂搁

---

## 10. 已知但未处理的小问题（不阻塞但值得记）

- `simple_dir_adapter::copy_dir_contents` 用同步 `std::fs` 在 async 函数里——skill 目录小可接受，扩展到大 payload 时需要 `spawn_blocking`
- `DirectoryLayout::detect()` 只检查目录存在性，对"共享根"工具会误判
- Parser **不校验 agentskills.io 名称 regex**（`^[a-z0-9]+(-[a-z0-9]+)*$`）也不限制 description 长度——opencode/Hermes 会拒
- `src/types.ts` 与 Rust DTO **手工对齐**，没有 codegen——改 serde-renamed 字段要双边同步
- Hermes / openclaw / Warp 是只读 adapters（MVP），写路径未验证
- Issue 009 引入了一个 HTTP fetch 依赖（看 Cargo 改动）——新窗口接 010/011 时如有需要再加，不必重复引入

完。
