# M-Skills Backlog

延后项与未决策略的集中清单。本文件是导航索引：每条都给出"为什么搁置"和"什么时候重新评估"。

> 已立项的切片走 `docs/issues/NNN-*.md`；roadmap 与时间分布走 `docs/handoff.md` §5。本 backlog **只列没有 issue 文档、且不归 handoff 当期跟踪**的工作。

---

## 1. Smart Add 闭环（搜市场 → 选 → 装）

**当前状态**：Issue 010（输入 + 路由）+ Issue 011（NL 分类 → 搜索词）已合入 main。Smart Add 用户体验链条到"分类后展示搜索词"为止。

**缺什么**：搜索词 → 候选列表 → 用户选 → 进现有 `preview_import` 流。这是 Issue 013（GitHub Search MVP + 发现页）的范畴，已规划，工作量 6–8 周，定位 Stage 3。

**为什么先不做**：
- 要先拍板市场数据源（见本文件 §4 Item 11）
- 是"工具管理 → 分发平台"的商业化拐点，需要独立讨论而不是切片实施

**临时缓解**：011 的 `searchQuery` 输出仅做展示。用户拿到搜索词后自行去 GitHub 找仓库，复制 URL 回 wizard 走 010 的 URL 路径。

**重新评估时机**：Stage 1 / Stage 2 收尾时，或 §4 Item 11 拍板后立即启动。

---

## 2. 本地导入：dialog picker / zip 解压 / 批量

**当前状态**：本地导入只支持"粘贴绝对路径"。无文件夹选择器、无 zip 解压、无批量。

**搁置原因**：
- 需要 `tauri-plugin-dialog` 与 `zip` crate，两个新依赖
- zip 解压必须防御 zip slip、zip bomb、size limit、entry count limit，安全工作不能赶
- 真正解决"用户从网上下载 10 个压缩包"的体验需要批量 preview UI（multi-candidate × multi-target install），这是新 issue 级别的改动，不是切片

**未来切法**（顺序固定）：
1. dialog picker（单文件夹 / 单 zip）—— 加 `tauri-plugin-dialog` 即可，~30 min
2. zip 解压 —— 加 `zip` crate + 安全防御（zip slip / 压缩比 / 总大小 / entry 数）+ fixture 测试，1 天
3. 批量 preview UI —— `BatchPreviewStep` 组件、`install_many` 命令、表格视图、全选/反选

**重新评估时机**：用户反馈强烈，或 §3 仪表盘 / Stage 1 其它项收尾后。

---

## 3. 父目录扫描入口（"Skills Library" 概念验证）

把"用户选 `~/Downloads/`，自动找出所有可装的 skill 目录或 zip"做成一级入口。是 §2 批量导入的极致形式。

**当前状态**：未立项。

**为什么先不做**：依赖 §2 全部基础设施。先看 §2 落地后的真实使用模式。

---

## 4. Roadmap 上未拍板的产品决策（与 handoff §6 同步）

这些决策不阻塞当前切片，但早晚要定，每条会触发新 issue。

| Item | 决策点 | 当前推荐 | 重新评估时机 |
|---|---|---|---|
| **2** | Claude Code / opencode / Hermes 没 native disable 时怎么禁用 | `.m-skills.json` 加 `disabled: true` + 文件名加 `.disabled` 后缀双保险 | Stage 1.3（enable/disable 全工具兜底）启动前 |
| **9** | 更新检测频率、联网默认、备份数量 | 启动 + 每 24h；默认 opt-in；每 skill 留最近 3 个版本 | Stage 2.1（更新检测 + 回滚）启动前 |
| **11** | 市场数据源 | GitHub Code Search 作 MVP（见 Issue 013） | Smart Add 闭环（§1）启动前 |
| **12** | 模板化抽象深度 | A（fork 时变量替换）→ B（frontmatter `parameters`）渐进；C（模板+instance 继承）视采纳率 | Stage 3.2 启动前 |

Item 1（Smart Add 系列决策）已在 `docs/issues/008-smart-add-skill-entry.md` 的 Decisions 段落定稿，不在此重列。

---

## 5. 已知技术债与小问题

不阻塞、不立 issue，但写新代码时值得知道。CLAUDE.md "Known gaps" 段已列大头，本节补充 backlog 视角下的优先级。

| 项 | 位置 | 何时应处理 |
|---|---|---|
| `simple_dir_adapter::copy_dir_contents` 同步 fs in async | `crates/skillsmgr-adapters/src/simple_dir_adapter.rs` | 当扩展到大 payload 时（如 §2 批量导入） |
| `DirectoryLayout::detect()` 仅检查目录存在 | 同上 | 当某工具的"shared root vs installed"歧义引发 bug |
| Parser 不校验 agentskills.io 名称 regex / description 长度 | `crates/skillsmgr-parse/` | 当首次出现因命名规则被 opencode/Hermes 拒装的报修 |
| Hermes / openclaw / Warp 写路径未验证 | 各 adapter | 当业务上需要写入这些工具时 |
| `src/types.ts` 与 Rust DTO 手工对齐，无 codegen | `src/types.ts` | 当镜像差异再次造成线上 bug（参考 commit `5e1484c` 的 tag 大小写事件） |

---

## 维护说明

- 新搁置项加到本文件并标注"为什么先不做 / 重新评估时机"。
- 决策一旦拍板，移到 `docs/issues/` 立 issue，从本文件删去。
- handoff §5 / §6 与本文件 §4 应保持同步——decision 推荐措辞改动时两处一起改。
