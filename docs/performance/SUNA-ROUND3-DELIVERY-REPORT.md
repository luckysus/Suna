# Suna 性能优化第三轮返工 — 交付报告

**基线提交（保留未覆盖）:** `5c51c7f`
**本轮工作区:** 未提交本地修改（按要求暂缓提交）
**报告日期:** 2026-10-17
**汇总来源:** 由原始 JSON 自动生成（`auto-summary.json`），禁止手工抄数

> 状态图例：**[已实现]** 代码在工作区 · **[已执行·通过]** 本机已跑通 · **[合成环境实测]** headless/mock · **[未验证·阻塞]** 有脚本与证据但环境不可用 · **[未执行]** 未跑

---

## 逐项交付

### 1. Agent 训练重试（同 run/tool_call 恢复已有任务） **[已执行·通过]**

- **修复:** `steel_agent_gateway.rs` `start_training` 在 prepare 之前 `for_tool_call` 短路；不再依赖源文件仍存在。最终入队路径仍保留 `submit_with` 幂等。
- **复现:** 正式 gateway `execute_for_tool_call(start_training)` 首次入队 → 删除源 CSV → 同 tool_call 再调 → 同一 task_id，且 `background_tasks` 仅 1 条。
- **命令:** `cd F:/Suna/src-tauri; $env:CARGO_TARGET_DIR='F:/Suna/.cargo-target'; cargo test --lib start_training_tool_call_replay -- --nocapture`
- **原始结果:** `cargo-agent-replay.log` → `ok. 1 passed`（`start_training_tool_call_replay_returns_existing_task_after_source_removed`）
- **未完成项:** 无

### 2. 知识库待刷新任务归属隔离 **[已执行·通过]**

- **修复:** `KnowledgeCenterWorkspace.tsx` 所有切换路径（`selectKnowledgeBase` / 清空选中）调用 `clearContentRefreshOwnership()`，并 bump `kbaseLoadGeneration`；pending/synced 不跨库泄漏。
- **删除重复 helper:** `knowledgeContentRefresh.ts` / `.test.ts`（业务未使用的同构纯函数）已移除，改由真实组件测试覆盖。
- **复现/测试:** `KnowledgeCenterWorkspace.test.tsx` — A 刷新失败→切 B 不继续拉 A；完成事件重试至成功后停止；A/B IPC 计数。
- **命令:** `cd F:/Suna/frontend; npx vitest run src/features/knowledge/KnowledgeCenterWorkspace.test.tsx`
- **原始结果:** `vitest-affected.log`（KnowledgeCenterWorkspace + ChatMessageList 共 4 passed）；全量 `npm test` 372 passed。
- **合成 IPC 计数（browser-audit knowledge）:** 见下方「知识中心合成 IPC」（来自 `browser-results.json`）。
- **未完成项:** 无 WebView2 真机 A/B 切换；组件测试 + 合成 IPC 已覆盖。

### 3. 浏览器审计修正 **[已执行·通过]** **[合成环境实测]**

- **修复要点:** mock `list_messages_page` 严格 limit/游标；全量经真实「加载更早消息」；滚动锚定当前页唯一消息并记录身份/位置/偏移；加载中与富文本就绪后分开打字；富文本就绪断言 tables/strong/唯一 marker，非固定 300ms。
- **命令:** `$env:SUNA_AUDIT_OUTPUT='F:/Suna/.suna-temp/performance-audit-20261017-round3-rework'; node F:/Suna/frontend/scripts/performance/browser-audit.cjs`
- **原始结果:** `browser-results.json` / `browser-summary.json` / `auto-summary.json`

| 场景 | readyWallMs 中位 | 加载中 typing 中位 | 稳定 typing 中位 | 稳定 P90 | 断言消息数 | DOM(稳定) | scrollΔ |
|------|------------------|--------------------|------------------|----------|------------|-----------|---------|
| empty-first-page | 1685.3 | — | 49.5 | 63.4 | 0 | 337 | — |
| 100-first-page | 1966.1 | — | 35.0 | 91.3 | 80 | 3840 | — |
| 500-first-page | 1939.7 | — | 48.2 | 125.1 | 80 | 3840 | — |
| 100-full-rich | 2033.5 | 49.8 | 49.9 | 51.4 | 100 | 4718 | — |
| 500-full-rich | 1862.6 | 34.0 | 115.5 | 117.7 | 500 | 22318 | — |
| 500-scroll-older | 2141.0 | — | 34.0 | 51.2 | 160 | 3840 | -0.1/-0.1/-0.1 |

- **滚动锚定样例（round0）:** `{"messageId":"message-420","beforeTop":191.390625,"afterTop":191.265625,"deltaPx":-0.125,"beforeSnippet":"合成研究问题 420：分析热处理工艺与材料性能的关系。[[USER_MARKER_420]]","afterSnippet":"合成研究问题 420：分析热处理工艺与材料性能的关系。[[USER_MARKER_420]]"}`
- **500 全量富文本断言（各轮 tables）:** `[250,250,250]`；strong: `[250,250,250]`
- **逐次输入样本/长任务:** 保留在 `browser-results.json` 各轮 `typingDuringLoad` / `typingStable`。
- **环境声明:** Windows headless Chromium, production dist, loopback HTTP, synthetic IPC; keypress-to-double-rAF is not standard INP
- **未完成项:** 非 Tauri WebView2 / 非标准 INP；仅 headless Chromium 合成 IPC。

### 4. 正式入口训练并发验证（IPC / Agent / 工作流） **[已执行·通过]**

- **要求落实:** 准备期并发读+小写；DB 失败计入 failures；分别记录 prepare / 事务持锁；不以 WAL 读成功代替写锁证明。Agent 正式 gateway 响应含 `timings.prepare_ms` / `timings.enqueue_or_tx_ms`。
- **命令:** `cd F:/Suna/src-tauri; $env:CARGO_TARGET_DIR='F:/Suna/.cargo-target'; cargo test --test training_entry_concurrency -- --nocapture`
- **原始 JSON:**
  - IPC: `{"concurrent_reads":{"busy_or_locked":0,"failures":0,"max_ms":19,"median_ms":19,"samples":2},"concurrent_writes":{"busy_or_locked":0,"failures":0,"max_ms":68,"median_ms":68,"samples":2},"enqueue_or_tx_ms":57,"entry":"ipc_prepare_then_enqueue","prepare_ms":38,"task_id":"824fcec5-2fe2-402a-bf13-e2bacb7326af"}`
  - Agent: `{"concurrent_reads":{"busy_or_locked":0,"failures":0,"max_ms":18,"median_ms":0,"samples":3},"concurrent_writes":{"busy_or_locked":0,"failures":0,"max_ms":56,"median_ms":34,"samples":3},"enqueue_or_tx_ms":35,"entry":"agent_gateway_start_training","prepare_ms":31,"task_id":"d7f9b538-6bf1-4710-9d4c-87d313224179","total_ms":88}`
  - Workflow: `{"atomic_mut_tx_ms":33,"concurrent_during_prepare":{"reads":{"busy_or_locked":0,"failures":0,"max_ms":16,"median_ms":16,"samples":2},"writes":{"busy_or_locked":0,"failures":0,"max_ms":122,"median_ms":122,"samples":2}},"entry":"workflow_prepare_outside_atomic_mut","note":"concurrent read/write ran during prepare; stopped before atomic_mut enqueue","prepare_ms":28,"task_id":"2c3d8fee-1fc9-4569-b9ca-1786ee5d82dd"}`
- **摘要:**
  - IPC prepare_ms=38, enqueue_or_tx_ms=57, reads.busy=0, writes.busy=0, failures=0/0
  - Agent prepare_ms=31, enqueue_or_tx_ms=35, total_ms=88, reads.busy=0, writes.busy=0
  - Workflow prepare_ms=28, atomic_mut_tx_ms=33, concurrent_during_prepare reads/writes samples=2/2
- **未完成项:** 无

### 5. 可执行 PG/pgvector 正式路径验证 **[未验证·阻塞]**

- **脚本:** `F:/Suna/scripts/verify-pgvector-isolated.ps1`（正式 knowledge migrations + 容器内 psql；vector 非 bytea；含导入/索引/EXPLAIN/Recall@K）
- **命令:** `powershell -File F:/Suna/scripts/verify-pgvector-isolated.ps1`
- **原始结果:** `{"outputDir":"F:/Suna/.suna-temp/pgvector-verify-round3-rework","status":"blocked","evidence":"Get-Command docker failed","reason":"docker_missing"}`
- **阻塞证据:** `F:/Suna/.suna-temp/pgvector-verify-round3-rework/run.log` → `BLOCKED: docker CLI not found on PATH.`
- **未完成项:** EXPLAIN、索引检查、Recall@K **未执行**（环境缺 Docker）。脚本已交付，待 Docker 可用后重跑。

### 6. 回归与工程检查 **[已执行·通过]**

| 检查 | 状态 | 原始日志/产物 |
|------|------|----------------|
| npm test | 74 files / 372 tests passed | `npm-test.log` |
| npm run test:boundaries | pass (192 files) | `npm-boundaries.log` |
| npm run build | built in ~20s | `npm-build.log` |
| cargo fmt --check | passed | `cargo-fmt-check.log` |
| cargo test --test training_entry_concurrency | 3 passed | `cargo-training-entry.log` |
| cargo test --lib start_training_tool_call_replay | 1 passed | `cargo-agent-replay.log` |
| cargo test --test steel_training_prepare | 3 passed | `cargo-steel-prepare.log` |
| vitest KnowledgeCenterWorkspace + ChatMessageList | 4 passed | `vitest-affected.log` |
| browser-audit 3×6 | exit 0 | `browser-results.json` |
| pgvector verify | blocked docker_missing | `pgvector-status.json` |

- **删除/替换未接入业务的 helper:** `knowledgeContentRefresh.ts(.test.ts)`、`chatPaginationOwnership.test.ts` 已删除；分页归属由 `ChatMessageList.test.tsx` + controller 路径覆盖。
- **未完成项:** pgvector 真机验证（见 §5）；未 git commit（按要求暂缓）。

---

## 知识中心合成 IPC（browser-audit）

```json
{
  "initial": {
    "boot_timing": 6,
    "db_init": 1,
    "plugin:app|set_app_theme": 2,
    "plugin:window|is_maximized": 1,
    "plugin:event|listen": 7,
    "list_research_projects": 1,
    "get_current_research_project": 1,
    "get_setting": 7,
    "list_postgres_knowledge_bases": 3,
    "list_agent_profiles": 2,
    "list_skills": 1,
    "list_mcp_servers": 1,
    "list_provider_profiles": 2,
    "plugin:window|set_theme": 2,
    "recover_agent_runs": 1,
    "list_conversations": 1,
    "get_postgres_knowledge_base_metrics": 1,
    "get_postgres_knowledge_health": 1,
    "list_postgres_documents": 1,
    "list_postgres_ingestion_jobs": 1,
    "list_postgres_tags": 1
  },
  "afterPoll": {
    "boot_timing": 6,
    "db_init": 1,
    "plugin:app|set_app_theme": 2,
    "plugin:window|is_maximized": 1,
    "plugin:event|listen": 9,
    "list_research_projects": 1,
    "get_current_research_project": 1,
    "get_setting": 7,
    "list_postgres_knowledge_bases": 3,
    "list_agent_profiles": 2,
    "list_skills": 1,
    "list_mcp_servers": 1,
    "list_provider_profiles": 2,
    "plugin:window|set_theme": 2,
    "recover_agent_runs": 1,
    "list_conversations": 1,
    "get_postgres_knowledge_base_metrics": 3,
    "get_postgres_knowledge_health": 1,
    "list_postgres_documents": 1,
    "list_postgres_ingestion_jobs": 3,
    "list_postgres_tags": 1,
    "plugin:event|unlisten": 2
  }
}
```

## 产物目录

`F:/Suna/.suna-temp/performance-audit-20261017-round3-rework/`

- `browser-results.json` / `browser-summary.json` / `auto-summary.json`
- `training-entry-{ipc,agent,workflow}.json`
- `pgvector-status.json`
- 各类 `npm-*.log` / `cargo-*.log` / `vitest-affected.log`

## 复现命令总表

```powershell
cd F:/Suna/frontend
npm test
npm run test:boundaries
npm run build
npx vitest run src/features/knowledge/KnowledgeCenterWorkspace.test.tsx src/features/chat/ChatMessageList.test.tsx
$env:SUNA_AUDIT_OUTPUT='F:/Suna/.suna-temp/performance-audit-20261017-round3-rework'
node scripts/performance/browser-audit.cjs

cd F:/Suna/src-tauri
$env:CARGO_TARGET_DIR='F:/Suna/.cargo-target'
cargo fmt --check
cargo test --test training_entry_concurrency -- --nocapture
cargo test --lib start_training_tool_call_replay -- --nocapture
cargo test --test steel_training_prepare -- --nocapture

powershell -File F:/Suna/scripts/verify-pgvector-isolated.ps1
```
