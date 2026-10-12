# 真实 Embedding/Rerank 与独立像素回归（2026-10-12）

本轮补齐 2026-10-11 记录中的真实云 Embedding/Rerank、已发现的深色样式问题和独立像素比较。用户数据目录、数据库和原文件没有迁移、删除或改写；所有浏览器验证使用后台 Chromium，没有启动桌面窗口或操作鼠标。

## 真实百炼检索模型

本机进程、用户、系统三个环境范围均未发现名称包含 `SILICON` 的环境变量；进程及用户范围存在 `DASHSCOPE_API_KEY`、`DASHSCOPE_BASE_URL`。凭据只从环境读取，没有写入源码、截图或日志。

| 能力 | 实际模型 | 应用请求路径 | 真实结果 |
| --- | --- | --- | --- |
| Embedding | `qwen3.7-text-embedding` | `configured_embedding_provider` → 生产 HTTP Provider → 百炼兼容接口 `/embeddings` | 4 条向量，每条 1024 维；数量、维度、有限值及非零断言通过 |
| Rerank | `qwen3.7-text-rerank` | `configured_rerank_provider` → 生产 HTTP Provider → 百炼原生 `/api/v1/services/rerank/text-rerank/text-rerank` | 3 个候选全部返回，ID 映射和有限分数正确，相关钢材资料排首位 |

查询为“Q355B钢材的屈服强度是多少？”。文档分别描述海水温度、Q355B 屈服强度、烹饪米饭。查询与钢材文档的余弦相似度 **0.9237**，与海水文档 **0.1870**，真实语义区分断言通过。先前聊天验证使用 `qwen3.8-flash`，与本轮两个检索模型分别记录。

测试在 `src-tauri/tests/live_smoke.rs::real_bailian_retrieval_smoke`，为显式选择才执行的 ignored 测试；普通测试不会调用云服务。它直接调用应用生产 Provider 工厂，复用 HTTP 超时、重试、有界响应、脱敏、索引映射和浮点校验。

产品已支持百炼检索模型：设置页的 Embedding、Reranker 服务类型可选 SiliconFlow / 阿里百炼，分别提供默认模型与 URL；连接测试、知识库查询/重排、PostgreSQL 文档向量生成和持久化导入任务均允许 Qwen。任务仍检查工作区、启用状态、配置修订号和凭据代数；MinerU 的类型限制保留。检索模型不会出现在聊天/Agent 模型列表，也不会把百炼聊天或重排模型误列为 Embedding。

新增契约测试覆盖百炼每批 10 条向量、乱序索引归位、原生重排请求/响应、非法索引/分数/数量、任务 Provider 加载及导入时模型固定。SiliconFlow 的原有请求格式和测试保留。

## 深色样式修复

将管理、知识、数据、文献、预测、优化、实验、报告和聊天欢迎区域中的白色背景、暗色正文、硬编码边框改为已有主题变量。成功/错误/警告文字有独立的浅深色语义变量；页头文字与“待配置”状态也使用主题颜色。主按钮的填充与强调文字分开，使白色按钮文字保持足够对比度。

报告 SVG 自包含画布、图表颜色和主题预览示意图保留固定输出配色。没有替换现有组件库或新增运行时依赖。原有 E2E 的原生 select 操作已改为实际 Radix combobox/option 操作，品牌断言限定可见文字；固定 IPC 补齐 Tauri Webview 元数据和事件注销入口。

## 固定基线与独立比较

`frontend/e2e/visual.spec.ts` 使用真实应用页面和稳定 IPC fixture，覆盖 17 个路由 × 浅/深两套主题 **34 张页面截图**，另覆盖百炼 Embedding / Reranker 配置表单 × 两套主题 **4 张截图**。

- 后台 Chromium，Windows，固定 1280×820 视口、1.25 DPI、中文语言、固定日期；等待字体与加载状态完成，关闭截图动画及光标。
- 自动检查页面/欢迎卡/页头/主操作及选中导航文字对比度（普通文字 4.5，大字 3）、深色主要面板无白色残留、无水平溢出、聊天输入框位于视口内。
- 显式更新基线后静态审阅，再用新的测试进程执行普通比较。普通配置 `updateSnapshots: "none"`，禁止缺少基线时自动生成或覆盖。
- 比较使用 Playwright YIQ 阈值 `0.15`，允许变化像素比例至多 `0.0001`（0.01%）。相同平台、浏览器与字体下复用已审阅 PNG。
- 负向检查将深色欢迎卡背景改为红色，确认像素比较产生差异失败；负向检查在更新基线时跳过，在普通比较时必须通过。

最终独立进程普通运行：**48 passed / 0 failed / 0 skipped**，其中 **39 项视觉检查**（38 张比较 + 1 项改色检出），原有启动/导航/知识库 E2E **9 项**。运行前后 38 个 PNG 的 SHA-256 完全相同，确认没有更新基线。结果与哈希保存在 `2026-10-12-visual-results.json`。

本轮是后台 Chromium 的页面级像素回归，使用固定测试数据；与 10-11 的真实 Tauri/WebView2 截图分开记录。没有把首次截图、更新基线或固定 IPC 说成真实业务数据验收；完整业务状态、其他 DPI/系统/浏览器不在本轮覆盖中。

## 常规检查

| 检查 | 结果 |
| --- | --- |
| Rust lib | 324 passed / 2 ignored |
| Rust Provider 契约 | 59 passed |
| Rust architecture / agent_protocol_contract | 28 / 3 passed |
| Rust rag_commands / rag_embedding / rag_import / rag_provider_factory | 6 / 5 / 11 / 5 passed |
| 真实百炼 Embedding/Rerank | 1 passed |
| 后台 E2E（含独立像素比较） | 48 passed / 0 failed / 0 skipped |
| 前端 Vitest | 37 文件 / 239 测试通过 |
| 前端 runtime boundaries | 134 文件通过 |
| TypeScript 与生产构建 | 通过，保留既有 bundle 大小提示 |

## 复现

真实模型需要本机已有 `DASHSCOPE_API_KEY`、`DASHSCOPE_BASE_URL`，不要将凭据粘贴到命令或测试文件。可用 `SUNA_LIVE_EMBEDDING_MODEL`、`SUNA_LIVE_RERANK_MODEL` 选择其他获授权模型。

在 `F:/Suna/src-tauri`：

```powershell
cargo test --test live_smoke real_bailian_retrieval_smoke -- --ignored --nocapture
cargo test --test providers --test rag_provider_factory --test rag_import --test rag_embedding --test rag_commands --quiet
```

在 `F:/Suna/frontend`：

```powershell
# 普通回归：不更新基线，包含像素差异负向检查及原有知识库 E2E。
node --require ./scripts/libuv-fix.cjs node_modules/playwright/cli.js test --workers=2
npm test -- --maxWorkers=2
npm run test:boundaries
npm run build

# 仅在有意修改 UI、复核截图后更新；之后必须再次运行普通回归。
node --require ./scripts/libuv-fix.cjs node_modules/playwright/cli.js test e2e/visual.spec.ts --update-snapshots=all --workers=2
```

38 张 PNG 基线保存在 `frontend/e2e/visual.spec.ts-snapshots/`。本机过程日志与汇总图位于 `C:/Users/Administrator/AppData/Local/Temp/suna-validation-20261012/`，临时产物可能被系统清理。
