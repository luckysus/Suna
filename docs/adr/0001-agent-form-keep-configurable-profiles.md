# ADR 0001：Agent 形态保持「可配置 Agent Profile + 预设 + 动态子 Agent 委派」

日期：2026-10-11
状态：已接受（用户决策）

## 背景

开发文档第 48–49 章描述的 Agent 形态是「8 个固定内置专家 Agent」。
实际实现（`src-tauri/src/agent/profiles.rs`、`src-tauri/src/agent/runtime/subagents.rs`、
迁移 0033 的 `agents` 表 + `agent.profiles` 设置键）走的是另一条等价路线：

- **可配置 Agent Profile**：持久化在 `agents` 表，字段覆盖模型路由、
  权限限制（文件/Shell/网络/数据库/MCP/危险操作确认）、轮次与工具调用上限；
- **内置预设**（`preset_ids()`：master/knowledge/literature/data/material/
  prediction/optimization/experiment/report），预设可重置、可复制为自定义 Profile；
- **动态子 Agent 委派**（`runtime/subagents.rs`）：主 Agent 按任务把子任务
  委派给匹配的 Profile，子轮次独立计数与限权。

功能上与「8 个固定 Agent」等价，但用户可增改 Profile，灵活性更高。

## 决定

**保持现状**（用户 2026-10-11 明确拍板）：不把实现改回 8 个硬编码 Agent。
开发文档对比中，第 48–49 章按「功能等价」计为已实现。

## 备选方案

- **改回 8 个固定 Agent**：与文档字面一致，但牺牲用户自定义能力，
  且需要重写已落地并测试的 profiles/subagents 链路。否决。
- **固定 Agent + 可配置扩展并存**：两套形态长期并行，维护成本高、
  语义易混淆。否决。

## 影响

- 后续 Agent 相关开发（第 50–53 章及之后）一律扩展 Profile/预设/
  委派机制，不再引入「固定 Agent」概念。
- 文档对比（`docs/IMPLEMENTATION_STATUS_DEV_DOC.md`）中相关条目
  标注「功能等价，保持现状」，不再作为差距跟踪。
