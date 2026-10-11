# 2026-10-11 真实环境验证记录

本轮补齐开发文档第 60 章数据目录更改，并验证此前仅有代码或模拟测试证据的集成。以下结果区分“已实测”与“仍未验收”；不代表全部业务流程已完成端到端验收。

## 环境与隔离

- Windows 10，真实 Tauri/WebView2 开发窗口，默认窗口 1280×820，devicePixelRatio 1.25。
- PostgreSQL 18.3，pgvector 0.8.2，本机已配置的 Suna 知识库连接。
- 百炼兼容接口 `https://dashscope.aliyuncs.com/compatible-mode/v1`，模型 ID `qwen3.8-flash`。
- PostgreSQL 凭据从系统凭据库读取，百炼凭据从本机环境变量读取，仅在内存中使用。
- PostgreSQL fixture 使用单连接事务与临时 schema，执行生产迁移和检索 SQL 后回滚并断言 schema 无残留。
- 桌面测试使用独立的 `SUNA_STORAGE_CONFIG_FILE` 和临时数据目录。没有切换用户实际数据根目录，没有删除用户数据库。
- 未新增依赖。实时集成测试使用 `#[ignore]` 显式运行，缺配置会失败，不再静默跳过后显示“通过”。

## 已完成验证

| 项目 | 实际结果 | 范围 |
| --- | --- | --- |
| PostgreSQL/pgvector | 通过 | 生产词法、向量、两条混合检索、来源筛选、相似度阈值、EvidencePack；事务全部回滚 |
| 真实模型 | 通过 | 生产 configured_chat_provider 调用百炼 qwen3.8-flash，收到真实流式事件和 `SUNA_LIVE_OK` |
| 真实文献 | 通过 | Europe PMC OA 正文 → 生产 ingest/parse/chunk → PG 文献搜索和章节读取，38 个片段 |
| SVG → PNG | 通过 | 真实桌面 Canvas 栅格化中文及负数柱状图、相关性热图；1440×640 / 1440×380 |
| 报告导出 | 通过 | DOCX/PPTX ZIP 内各检查至少两张 PNG，签名正确 |
| Tauri 原生拖拽路径 | 手工真机验证通过 | 用户 PDF 拖入聊天附件；捕获真实 `drop` 事件；文本 fixture 的附件也包含本地绝对 path |
| 逐页检查 | 已建立首次基线，有视觉缺陷 | 17 页 × 浅深色，34 截图，0 pageerror / 水平溢出；聊天输入框在视口内 |
| 目录迁移/切换 | 通过 | 真实 IPC、UI 待迁移状态、应用重启、文件及会话/消息保留、自有路径重定位、再次重启持久化、取消、非空目标失败回退 |

### 数据目录实现

设置页选择空目录并确认后，持久化待迁移状态，可取消或点击“重启并迁移”。自然关闭后下一次启动也会处理迁移。当前运行中的目录保持固定，迁移发生在数据库、知识 API 和后台任务初始化之前。

完整文件树复制后校验 SHA256；SQLite 使用 `VACUUM INTO` 纳入已提交 WAL，再对自有路径字段和 JSON 路径进行重定位，并执行 `integrity_check`。不可变的检索审计记录保留原始路径；PostgreSQL 文档路径通过历史目录映射解析，不改写用户 PostgreSQL 数据。

全部成功后才切换并保存新根目录。失败继续原目录并展示原因；原目录和部分复制的文件均保留，不自动删除。拒绝相同、互相嵌套、非空、相对、文件、符号链接及 Windows Junction 目标，并检查空间。运行中的 Agent 阻止立即重启，调度器需要先停止。`SUNA_DATA_DIR` 优先，使用它时设置页说明固定原因并禁止修改。

知识内容目录展示修正为实际的 `objects`；模型存储于 SQLite，设置页显示其所在数据根目录。

### 真实文献与检索修复

- 标题：Frictional behaviour of steel-steel and steel-concrete contact surfaces for offshore slip joint applications.
- DOI：`10.1038/s41598-026-65241-w`；PMCID：`PMC13442889`。
- 正文源：`https://www.ebi.ac.uk/europepmc/webservices/rest/PMC13442889/fullTextXML`。
- 下载 XML 165811 字节，提取的正文 TXT 76457 字节，经生产解析产生 38 个片段。

真实调用暴露了章节读取把操作模式 `section` 加入 AND 检索词的问题，导致正文存在仍无结果。现改为使用原查询词，真实文献章节读取测试覆盖此回归。

产品文献入口仍查询本地知识库。本次下载真实 OA 文献用于集成验证，没有新增在线 Europe PMC、Crossref 或 arXiv Provider。

### 桌面图表与逐页截图

`bar.png` 1440×640，46144 字节，66025 个非白色像素；`heatmap.png` 1440×380，28929 字节，38685 个非白色像素。实际生成的 `report.docx` 为 69013 字节，`report.pptx` 为 137459 字节。

覆盖页面：聊天、知识中心、文献、数据实验室、预测、优化、实验助手、Agent、工具、MCP、Skill、模型、报告、设置、账户、关于、诊断。

默认窗口曾发现聊天输入框超出视口，已通过聊天容器高度与网格约束修复，之后实机截图断言通过。静态截图复核另发现部分深色页面仍使用白色卡片和浅色页面的文字颜色：Agent 管理、数据实验室、知识中心、预测、优化、报告、Skill、工具及聊天欢迎卡片存在背景/文字对比度不一致。这是第 88 章仍需修复的实际问题。

本次 `mismatch` 为 `null`：保存了首次实机基线，没有历史基线，也没有独立第二轮比较。脚本支持后续 `--baseline` 像素比较（RGB 任一通道差超过 32 计为变化，变化比例不超过 0.5%），`--update-baseline` 显式更新。不能把首次截图记作像素回归通过。

### 原生拖拽范围

共享 `useNativeFileDrop` 接 Tauri 路径事件，按区域和 DPI 过滤，处理退出及订阅清理；聊天、知识中心和通用 FileDropZone 使用该入口，保留浏览器 DOM fallback。组件测试覆盖事件路径、DPI 区域和卸载清理。

用户手工 PDF 拖拽已在真机显示附件，实际捕获两次 Tauri `drop` 事件；另观察到隔离文本 fixture 的附件记录具有非空绝对路径。自动 OLE 拖拽受焦点/DPI 干扰，未形成稳定通过证据，因此不计入通过项，其自动脚本已移除。

用户反馈窗口与鼠标操作后，已停止这类测试；本轮 Suna 测试窗口、拖拽辅助进程及 Vite 服务均已结束。

## 常规检查

| 检查 | 结果 |
| --- | --- |
| Rust `cargo test --lib --quiet` | 324 passed / 2 ignored |
| Rust `architecture` | 28 passed |
| Rust `agent_protocol_contract` | 3 passed |
| 前端完整 Vitest | 37 文件 / 236 测试通过 |
| 前端 runtime boundaries | 133 文件通过 |
| 前端 `npm run build` | TypeScript 与生产构建通过，保留既有 chunk 提示 |
| 最后存储提示改动相关测试 | 3 文件 / 36 测试通过 |

worker 上轮结果为 125 passed / 2 skipped，本轮没有重跑。

## 复现入口

以下命令不会由普通单元测试自动执行。模型需要本机 `DASHSCOPE_API_KEY`、`DASHSCOPE_BASE_URL`；PostgreSQL 测试使用应用 SQLite 中的现有配置与系统凭据库，可用 `SUNA_LIVE_DATA_DIR` 指定该配置所在目录。不要把密钥或含密码的连接串写入仓库。

在 `F:/Suna/src-tauri`：

```powershell
cargo test --lib real_postgres_retrieval_sql -- --ignored --nocapture
cargo test --test live_smoke real_bailian_chat_smoke -- --ignored --nocapture
$env:SUNA_LIVE_LITERATURE_FILE = 'C:/Users/Administrator/AppData/Local/Temp/suna-live-validation-20261011/paper.txt'
cargo test --lib real_literature_source_smoke -- --ignored --nocapture
```

桌面脚本需要 Vite 开发服务和真实 Tauri 开发窗口，使用 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223` 提供 CDP。迁移脚本还要求应用从隔离测试根目录启动、`SUNA_STORAGE_CONFIG_FILE` 指向隔离 JSON 配置（其 `path` 指向测试数据根目录）、不设置 `SUNA_DATA_DIR`。它会切换页面并重启该应用，不应指向用户实际数据。

在 `F:/Suna/frontend`：

```powershell
node scripts/desktop-smoke.mjs --output <截图结果目录> --baseline <基线目录>
node scripts/desktop-storage-smoke.mjs --root <隔离测试根目录>
```

## 证据文件

本轮产物根目录：`C:/Users/Administrator/AppData/Local/Temp/suna-live-validation-20261011`。临时产物未复制入仓库，目录保留供查阅；系统清理临时目录会移除它们。

- `migration-result.json`：源文件保留、复制、历史、自有路径重定位、再次重启、取消、失败回退均为 true。
- `migration-pending.png`、`migration-success.png`、`migration-failed.png`：三个真实 UI 状态。
- `data-a/`、`migrated-1791731968727/`：迁移前后隔离数据，源目录保留。
- `paper-source.json`、`paper.xml`、`paper.txt`：真实文献元数据与正文。
- `visual-final-first/results.json`：34 页截图及图表/导出结果。
- `visual-final-first/light-*.png`、`dark-*.png`：逐页截图；两张 `*-contact-sheet.png` 为静态汇总。
- `visual-final-first/bar.png`、`heatmap.png`、`report.docx`、`report.pptx`：实际桌面导出。
- `visual-final-baseline/`：本轮首次实机截图基线。
- `frontend-build-final.log`：最后生产构建日志。

## 尚未完成的验收

1. 真实云 Embedding/Rerank 未调用；目前产品实现使用 SiliconFlow，未配置其真实服务凭据。固定向量的 pgvector SQL 验证不能替代模型向量生成与重排验证。
2. 部分深色样式仍需修复；没有历史视觉基准或独立第二轮像素比较，本轮截图主要覆盖空/未配置状态，未覆盖每个业务状态。
3. 自动 OLE 拖拽没有稳定通过；手工原生路径证据不等于全部拖拽区域的自动端到端覆盖。
4. 迁移在隔离本机数据验证，未迁移用户实际数据；没有执行断电或任意进程崩溃注入测试。
