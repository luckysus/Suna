# Suna 开发程度评估（依据 `开发文档.md`）

- 更新日期：2026-10-11
- 评估依据：`开发文档.md`（89 章产品规格，用户提供）
- 评估方式：保留原逐章代码核对清单，补充第 60 章实现与本轮真实环境验证；代码覆盖与实际验收分别记录。
- 说明：本文按 `开发文档.md` 的口径评估。基于新增 19 份文档包的评估见 `docs/IMPLEMENTATION_STATUS.md`。

## 评估立场（重要）

**本文的唯一基准是 `开发文档.md`。** 凡文档写明的要求，未实现即计入缺口，不因"工程上更合理"而豁免。

- 正文各章的「状态」列只做事实判定：✅ 符合文档 / 🟡 部分符合 / ❌ 未实现。
- 本文**不包含**评估者的技术取舍建议。若某处需要工程判断，会单独标注为「评估者注」并明确它不是文档要求。
- 第 20 项（P3）各项虽然实现方式与文档不同且功能可用，**仍按缺口计**，因为文档是唯一基准。

## 总体结论

**第 60 章“更改数据目录”已补齐，主要功能已有对应代码；目前不能认定为 100% 真实环境验收通过。**

- 原 A–M 组（1–89 章）清单记录已有功能实现，本轮重点核验存储迁移、真实检索、模型、文献、桌面导出和拖拽。
- PostgreSQL/pgvector 生产检索 SQL、百炼真实流式聊天、真实 OA 文献导入后检索/阅读、桌面 SVG → PNG 及目录迁移已通过。17 页浅深色截图已建立基线，截图复核发现部分深色样式仍不一致；真实 Embedding/Rerank、独立像素回归比较及完整业务状态验收尚未完成。详细证据见 `docs/validation/2026-10-11-live-validation.md`。

| 分组                    | 章节    | 当前记录      |
| --------------------- | ----- | -------- |
| A 产品与架构               | 1–5   | 代码已覆盖 |
| B Agent 架构与 Workspace | 6–10  | 代码已覆盖 |
| C 界面与视觉               | 11–21 | 代码已覆盖；深色视觉仍有问题 |
| D 知识中心与文献             | 22–29 | 代码已覆盖；本地文献检索已实测 |
| E 数据实验室与预测            | 30–37 | 代码已覆盖 |
| F 工艺优化                | 38–43 | 代码已覆盖 |
| G 实验助手                | 44–46 | 代码已覆盖 |
| H 管理模块                | 47–53 | 代码已覆盖 |
| I 模型与报告               | 54–56 | 代码已覆盖；真实聊天与图表导出已实测 |
| J 设置                  | 57–62 | 目录更改已补齐并实测 |
| K UI 规范与交互            | 63–74 | 代码已覆盖；拖拽真机路径已验证 |
| L 工程结构                | 75–82 | 代码已覆盖 |
| M 原则与开发要求             | 83–89 | 深色样式一致性待修复；验收尚未完成 |

---

## A. 产品与架构（1–5 章）— 代码对照

| 章   | 要求                                                                                                                                     | 状态        | 证据                                                                                                                                                                   |
| --- | -------------------------------------------------------------------------------------------------------------------------------------- | --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1–2 | 定位：面向钢铁材料研发的 AI Agent 工作平台；"聊天是入口，Agent 是核心"                                                                                           | ✅         | `README.md`、`README.en.md`、`docs/PRODUCT.md`、侧栏副标题与关于面板文案一致                                                                                                          |
| 3   | 15 个核心模块                                                                                                                               | ✅         | `frontend/src/app/navigation.ts` 19 个导航项覆盖全部 15 个模块                                                                                                                  |
| 4   | Rust + Tauri 2；React 19 + TS + Vite；Tailwind；Recharts；React Markdown；KaTeX；**Agent 核心由 Rust 实现，不依赖 LangGraph**；**Python 不作核心 Runtime** | ✅ 技术栈全部一致 | `Cargo.toml`（tauri 2）、`package.json`（react 19 / typescript 6 / vite 7 / tailwind 4 / recharts / react-markdown / katex）；无 LangGraph 依赖；Python 仅为 `compute-worker` 侧车 |
| 4   | UI：shadcn/ui + Radix UI                                                                                                                | ✅ 已采用     | 16 个 `@radix-ui/*` 依赖 + `src/components/ui/` 22 个组件；原生 `<select>` 31 个、`<textarea>` 13 个、checkbox 14 个已全部迁移，仅 3 个 `type="file"` 保持原生                                 |
| 4   | State：Zustand                                                                                                                          | ✅ 已采用     | `zustand` 依赖 + `src/stores/` 11 个文件（8 个 store + storeUtils + settingsPersistence + localeModel 等），4 处 Context（appearance / theme / i18n / chatController）已迁移         |
| 4   | Charts：ECharts 或 Recharts                                                                                                              | ✅         | Recharts；`src/components/charts/` 8 个图表组件，图表类型 9 种                                                                                                                   |
| 5   | 分层：`React → Tauri IPC → Rust Core → Agent Runtime → LLM/RAG/Tools/MCP/Models`                                                          | ✅ 完全一致    | `SunaApp` → `bridge/desktop.ts` → `app/commands.rs`（195 个命令）→ `agent/runtime/` → `providers` / `rag` / `tools` / `mcp`                                               |
| 5   | React 职责：UI / Chat / 数据可视化 / 页面交互 / 设置 / Agent 状态展示 / 知识库管理                                                                            | ✅         | 数据可视化已从 1 种（BarChart）补齐到 9 种（柱状/箱线/热力/直方/雷达/散点/折线/PCA/SHAP）                                                                                                          |

---

## B. Agent 架构与 Workspace（6–10 章）— 代码对照

| 章    | 要求                                                                                                                      | 状态   | 证据                                                                                                                                                                                                                                       |
| ---- | ----------------------------------------------------------------------------------------------------------------------- | ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 6    | Master Agent + 8 个专业子 Agent（Knowledge / Literature / Data / Material / Prediction / Optimization / Experiment / Report） | ✅    | `agent/profiles.rs::presets()` 定义 Master + 8 个专家预设，各有独立 system prompt、描述与工具白名单；只有 Master 持有 `agent.task`，其余 8 个作为可委派子 Agent；`app/desktop_agent_runtime.rs` 把可委派专家清单注入 Master 提示词，子 Agent 通过 `agent/runtime/subagents/` 的 child turn 机制执行 |
| 7    | Master Agent 职责（理解、拆分、调度、汇总）                                                                                            | ✅    | `agent/runtime/loop/`（operations / generation / background）+ `loop_types.rs`                                                                                                                                                             |
| 8    | 7 种 Agent 状态（idle/thinking/planning/running/waiting/completed/failed）                                                   | ✅ 超集 | 协议 `AgentRunState` 有 11 态：created / preparing / generating / awaiting_permission / executing_tools / verifying / completing / completed / cancelled / failed / interrupted                                                               |
| 9–10 | 右侧 Agent Workspace：当前任务、子 Agent 状态、工具调用、执行步骤、耗时、结果、进度                                                                   | ✅    | `features/chat/AgentRunInspector.tsx`（运行检查器：状态、sequence、工具数、token、进度、工具调用、权限、checkpoint、恢复、引用、错误）+ `ChildAgentPanel.tsx`                                                                                                                 |

---

## C. 界面与视觉（11–21 章）— 代码对照

| 章     | 要求                                                                                                                                                                                     | 状态 | 证据                                                                                                                                                                              |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 11    | 三栏桌面布局（Sidebar / Main / Agent Workspace）                                                                                                                                               | ✅  | `app/SunaApp.tsx` + `features/chat/DesktopChatWorkspace.tsx`                                                                                                                    |
| 12–13 | Sidebar：Logo、新对话、7 个研究模块、更多、最近、用户中心；宽度 250–280px；顶部 `Suna` + `Steel Research Agent`                                                                                                    | ✅  | 侧栏宽度 260px（narrow 250 / wide 280），落在文档区间；品牌区为 `Suna` + `Steel Research Agent`；含新对话、7 个研究模块、更多、最近对话、底部用户中心                                                                       |
| 14–15 | Light Theme；主色 `#2563EB`、浅蓝 `#EFF6FF`、背景 `#F8FAFC`、白 `#FFFFFF`、边框 `#E5E7EB`、主文字 `#111827`、次文字 `#6B7280`、成功 `#22C55E`、警告 `#F59E0B`、错误 `#EF4444`；禁止黑色主界面 / 大面积渐变 / 金属纹理 / 3D / 玻璃拟态 / 发光 | ✅  | `design/tokens.css` 的浅色令牌即文档主色板（新增 `--suna-primary` / `--suna-warning`）；主界面 `suna-shell.css` 全部改用令牌；侧栏原先的 `linear-gradient` 已改为纯色 `var(--suna-sidebar)`                         |
| 16    | 重新设计 Logo："S" + 晶体结构 / AI 节点；禁用钢卷/工厂/炉子/齿轮/锤子                                                                                                                                          | ✅  | `frontend/src/components/SunaLogo.tsx`：等轴晶体晶格，几何/科技/简洁，无文档禁用元素                                                                                                                  |
| 17    | 智能问答为默认首页；顶部模型选择 / Agent 模式 / 知识库状态；欢迎区 `Suna` + `钢铁材料研发智能助手` + 材料分析/文献研究/数据分析/性能预测/工艺优化/实验设计                                                                                          | ✅  | `startupPage` 默认 `"chat"`；头部含模型切换与 Agent/知识库入口；欢迎区品牌行为 `Suna` + `钢铁材料研发智能助手`，6 张能力卡片与文档一致                                                                                       |
| 18    | Chat 输入框：文本 / 文件拖拽（PDF、Excel、CSV、Word、图片）；底部添加文件、知识库、Agent、模型、发送                                                                                                                       | ✅  | `DesktopChatWorkspace` 的 composer 支持 `onDragOver`/`onDrop`（拖拽遮罩 + 复用既有类型过滤，覆盖 PDF/DOC/DOCX/XLS/XLSX/CSV/TXT/MD/JSON/HTML/图片）+ 粘贴图片；底部含附件、智能搜索、知识库、Agent、模型、参数、发送                |
| 19    | 模型选择：Provider / Model / Temperature / Context / Max Tokens                                                                                                                             | ✅  | composer 的模型按钮切换 Provider/Model；新增「参数」面板（`ChatModelParameters`）展示 Provider/Model 并可编辑 Temperature / Context / Max Tokens，写回 `model.preferences`                                 |
| 20    | Chat 消息：用户右侧 / AI 左侧；Markdown / 代码 / 公式 / 表格 / 图片 / 图表 / 引用 / 文件 / Agent 结果                                                                                                            | ✅  | `.suna-chat-user-turn` 右对齐、`.suna-chat-assistant-turn` 左对齐；`AnswerRenderer` = ReactMarkdown + GFM（表格/代码）+ **KaTeX 公式**（`remark-math` + `rehype-katex`，新增 2 个测试）+ 引用/图片/金相/网页悬浮卡 |
| 21    | AI 回答结构：结论 → 分析依据 → 知识来源 → 数据分析 → 模型结果 → 建议 → 引用；只展示任务进度 / Agent 状态 / 工具调用摘要 / 结果，不暴露内部推理                                                                                              | ✅  | 默认 system prompt（`agent/desktop/model.rs`）明确要求按该顺序组织回答且不暴露内部推理；前端不渲染 reasoning 事件，只展示运行状态、工具调用与结果                                                                               |


## D. 知识中心与文献（22–29 章）— 代码对照

| 章  | 要求                                                             | 状态 | 证据                                                                                                                                                                                           |
| -- | -------------------------------------------------------------- | -- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 22 | 知识中心管理 PDF / Word / Excel / CSV / 论文 / 专利 / 标准 / 实验记录 / 工艺文件   | ✅  | `features/knowledge/KnowledgeCenterWorkspace.tsx`；`rag/parse/` 支持 PDF / DOCX / XLSX / CSV / HTML / Markdown / TXT；新增**文档类型分类**（论文 / 专利 / 标准 / 实验记录 / 工艺文件 / 其他），存于文档元数据，可在文档抽屉编辑、在文档表展示并参与筛选 |
| 23 | 多知识库；每个知识库显示文件数量、知识片段、Embedding 状态、更新时间                        | ✅  | 新增 Tauri 命令 `get_postgres_knowledge_base_metrics`（单条 SQL 聚合每个知识库的文档数 / 已索引文档 / 片段数 / 已生成 Embedding 的片段数）；知识库卡片四要素齐全（文档 N、片段 M、Embedding 已索引/生成中/待生成/未索引、更新于）                                 |
| 24 | 上传流程：解析 → 切分 → Embedding → 索引；支持拖拽 / 点击 / 批量 / 文件夹导入；显示进度      | ✅  | `rag/ingest` / `rag/chunk` / `rag/index` / `rag/tasks`；导入文档 / 导入文件夹 / 拖拽导入；`IngestionJobs` 展示解析、切分、Embedding、索引各阶段状态与进度                                                                      |
| 25 | 知识检索：自然语言 + 过滤（材料 / 工艺 / 性能 / 年份 / 来源）；结果含标题、摘要、片段、来源、相关度      | ✅  | 检索面板含 材料 / 工艺 / 性能 / 年份 / **来源** 五个文档规定过滤器（另加文档类型与标签）；后端 `PostgresKnowledgeSearchFilters` 新增 `source`，命中 `metadata.source` 或文档名；结果展示文档名、片段、来源位置与相关度，并支持打开原文 / 复制 / 加入对话 / 收藏 / 引用            |
| 26 | 文献搜索、阅读、总结、对比、综述、研究趋势、研究空白                                     | ✅  | `features/literature/LiteratureResearchPage.tsx`：检索 / 阅读 / 总结 / 对比 / 综述 / **研究趋势**（新增视图，输出热点主题、时间演化、常用材料与工艺、方法与数据趋势、研究空白）                                                                    |
| 27 | 文献搜索结果显示标题、作者、期刊、年份、摘要、相关度；支持收藏、加入知识库、总结、对比、引用                 | ✅  | 结果行与详情面板展示 标题 / 作者 / 期刊 / 年份 / 摘要片段 / 相关度（作者、期刊、年份优先取文档元数据，缺省回退来源名与来源位置年份）；新增**加入知识库**动作（选择目标知识库后移动源文档），并保留收藏 / 总结 / 对比 / 引用                                                                 |
| 28 | 文献对比：研究材料、成分、工艺、组织、性能、研究结论、差异，使用表格比较                           | ✅  | 对比区保留证据表（研究对象 / 期刊 / 年份 / 证据位置），并让 Agent 只输出 `文献 \| 研究材料 \| 成分 \| 工艺 \| 组织 \| 性能 \| 研究结论 \| 差异` 的 Markdown 表格（证据不足填「未记录」），用 `AnswerRenderer` 渲染                                              |
| 29 | 文献综述：选择 10 / 20 / 50 篇；输出研究背景、研究现状、研究方法、研究结论、争议、研究空白、未来方向、参考文献 | ✅  | 工具栏新增**综述篇数 10 / 20 / 50** 选择（对比选中优先，否则取前 N 篇）；综述提示词要求依次输出 研究背景 / 研究现状 / 研究方法 / 研究结论 / 争议 / 研究空白 / 未来方向 / 参考文献                                                                               |

> 文献检索仍以本地知识库为数据源；作者 / 期刊 / 年份取文档元数据，未填写的条目显示「未记录」。

---

## E. 数据实验室与性能预测（30–37 章）— 代码对照

| 章  | 要求                                                                                                 | 状态     | 证据                                                                                                                                                                                                                                                       |
| -- | -------------------------------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 30 | 数据实验室：支持 Excel / CSV / 数据库 / JSON                                                                  | ✅      | `features/data/DataLabPage.tsx` + `steel/datasets/`（csv.rs / xlsx.rs）；导入含列映射与激活                                                                                                                                                                          |
| 31 | 数据分析：数据清洗、缺失值、异常值、重复值、统计分析、相关性、特征重要性、PCA、聚类、SHAP                                                   | ✅      | 统计/相关性/质量指标见 `steel/analysis.rs`；清洗引擎 `steel/cleaning.rs`（重复行、缺失值保留/删行/均值填充、IQR 异常值保留/删行/截断，结果落盘为新数据集）+ `DataLabCleaningPanel.tsx`；特征重要性与 SHAP 见 `DataLabExplainPanel.tsx` + `ShapBarChart`；PCA 投影与 k-means 聚类见 `PcaScatter`（`DataLabPage` 传 `clusters`） |
| 32 | 数据可视化：折线 / 柱状 / 散点 / 热力 / 箱线 / 雷达 / PCA / SHAP / Pareto Front；科研论文风格                               | ✅      | `components/charts/` 10 个组件：`LineTrendChart` / 柱状（recharts Bar）/ `ScatterPlot` / `CorrelationHeatmap` / `ColumnBoxPlot` / `ColumnRadarProfile` / `PcaScatter` / `ShapBarChart` / `ParetoFrontChart` / `DistributionHistogram`；白底、低饱和、无 3D/渐变             |
| 33 | 性能预测指标：YS / TS / EL / 硬度 / 冲击韧性 / 耐磨性 / r-value / 晶粒尺寸 / 组织比例                                      | ✅      | `predictionModel.ts` 内置上述指标的规范名与单位（屈服强度 YS=MPa、抗拉强度 TS=MPa、延伸率 EL=%、硬度 HV、冲击韧性 J、耐磨性 mg/mm²、塑性应变比 r-value、晶粒尺寸 μm、组织比例 %），目标/特征/输入标签统一展示                                                                                                                 |
| 34 | 预测流程：选择材料体系 / 数据集 / 模型 / 输入变量 / 预测目标                                                               | ✅      | `features/prediction/PerformancePredictionPage.tsx`：选数据集 → 训练 → 输入变量 → 预测，含适用范围告警与预测区间                                                                                                                                                                   |
| 35 | 模型中心：XGBoost / RandomForest / LightGBM / SVR / MLP / Transformer / ONNX / 自定义；模型卡片含 R²/MAE/RMSE/版本 | ✅ 10 族 | `linear_regression / elasticnet / random_forest / hist_gradient_boosting / lightgbm / xgboost / svr / mlp / transformer`（`transformer.py` 为纯 numpy FT-Transformer，无需 PyTorch）+ ONNX 导出与推理；指标含 metrics / feature_importance / 版本                          |
| 36 | 模型比较：多个模型同时预测并对比预测值/真实值/误差/R²/MAE/RMSE                                                             | ✅      | `ModelComparisonPanel.tsx`：勾选多算法 → 同一组输入依次训练+预测 → 表格对比预测值/实测值/误差/R²/MAE/RMSE，并标注验证集 R² 最优者                                                                                                                                                               |
| 37 | 模型解释：Feature Importance / SHAP / Partial Dependence / Prediction Interval                          | ✅      | Feature Importance 占比条 + `ShapBarChart`（SHAP）+ `PredictionPdpPanel.tsx`（固定其余输入、单特征网格逐点预测画 PDP）+ 预测区间（按验证集 RMSE 给 95% 区间，线性与 sklearn 路径均带）                                                                                                                |

> 预测与优化链路已具备运行环境（numpy / scikit-learn 1.9.0 / optuna 4.9.0 / onnxruntime 1.28.0 / lightgbm 4.7.0 / xgboost 3.4.1）。

---

## F. 工艺优化（38–43 章）— 代码对照

| 章     | 要求                                                                                    | 状态    | 证据                                                                                                          |
| ----- | ------------------------------------------------------------------------------------- | ----- | ----------------------------------------------------------------------------------------------------------- |
| 38    | 单目标 / 多目标 / 约束优化                                                                      | ✅     | `compute-worker/suna_worker/optimization.py` 的 `optimize_constrained`（含等式约束与固定变量）                           |
| 39–40 | 优化目标（最大化硬度/强度/耐磨/延伸率/韧性；最小化成本/能耗）；优化变量（淬火温度/回火温度/保温时间/冷却速度/轧制温度/压下率/合金元素）             | ✅     | 目标与变量由数据集列动态选择，方向可配（最大化/最小化），列语义经 `columnDisplayName` 映射到上述指标                                               |
| 41    | 算法：NSGA-II / Bayesian Optimization / Genetic Algorithm / Particle Swarm / Grid Search | ✅ 5/5 | `SUPPORTED_ALGORITHMS = {nsga2, tpe, ga, pso, grid}`；ga/pso 单目标，nsga2/grid 支持多目标                            |
| 42    | Pareto Front：必须支持二维与三维                                                                | ✅     | `OptimizationParetoChart.tsx`：2 目标用 Recharts 散点 + 非支配前沿折线；3 目标用纯 SVG 可旋转投影（拖拽 yaw/pitch，深度排序控制遮挡）；>3 目标回退表格 |
| 43    | 输出多个候选方案，可查看/比较/保存/加入实验计划                                                             | ✅     | 候选方案表（方案 A/B…、预测值、目标值、工艺取值）+「加入实验计划」（`createExperimentPlan`）+ 导出 JSON；训练任务已与性能预测联动（自动列出该数据集的已完成训练任务）        |

---

## G. 实验助手（44–46 章）— 代码对照

| 章  | 要求                                                         | 状态    | 证据                                                                                                                                                   |
| -- | ---------------------------------------------------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| 44 | 实验设计、下一实验推荐、变量选择、实验组合、主动学习                                 | ✅     | `features/experiment/ExperimentDesignPanel.tsx`（13 KB）+ `ExperimentAssistantPage.tsx`：填变量范围 → 生成实验组合 → 加入实验计划                                        |
| 45 | DOE / 正交实验 / 响应面 / Bayesian Optimization / Active Learning | ✅ 5/5 | `suna_worker/design.py`：`SUPPORTED_METHODS = {doe, orthogonal, ccd, bayesian, active_learning}`（全因子、Hadamard/L9/L25 正交表、中心复合响应面、高斯过程+期望改进、不确定性+覆盖缺口） |
| 46 | 输出下一组实验并解释"为什么推荐"                                          | ✅     | 结果含 `notes` 文字解释 + 期望改进 / 预测 σ / 覆盖缺口 列；支持「全部加入实验计划」                                                                                                 |

---

## H. 管理模块（47–53 章）— 代码对照

| 章     | 要求                                                                                                                   | 状态 | 证据                                                                                                                                                                                                                                                                                      |
| ----- | -------------------------------------------------------------------------------------------------------------------- | -- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 47    | Agent 管理：列表、状态、模型、工具、Skill                                                                                           | ✅  | `features/management/AgentManagementPage.tsx`（13.6 KB）+ `features/agents/AgentSchedulesPanel.tsx`（Cron 计划）                                                                                                                                                                              |
| 48    | 创建 Agent：Name / Description / System Prompt / Model / Tools / Skills / Memory / Permission                           | ✅  | Agent Profile 支持提示词、Provider、精确工具白名单、权限限制、执行预算；专家不能超过父 Agent 权限                                                                                                                                                                                                                         |
| 49    | Tool Center：文件 / 数据库 / Python / 模型 / 搜索 / 绘图 / 计算                                                                    | ✅  | `CapabilityManagementPage.tsx` 列出工具能力并支持**启用 / 停用**（`toggleTool`），停用后不出现在 Agent 工具清单                                                                                                                                                                                                    |
| 50–51 | MCP：本地 / HTTP / Stdio；显示名称、状态、连接方式、工具数量、权限；详情含 Server/Transport/Tools/Resources/Prompts/Status/Logs；支持启用/禁用/重启/删除/测试 | ✅  | `features/mcp/McpManagementPage.tsx`（13 KB）：CRUD + 健康检查 + 重启 + 工具列表 + 凭据管理；传输支持 stdio / streamable_http / sse                                                                                                                                                                           |
| 52    | Skill：默认 8 个（耐磨钢分析、IF钢分析、论文总结、数据分析、性能预测、工艺优化、实验设计、金相分析）                                                              | ✅  | `src-tauri/resources/skills/` 内置 8 个默认 Skill（`wear-resistant-steel-analysis` / `if-steel-analysis` / `paper-summary` / `data-analysis` / `performance-prediction` / `process-optimization` / `experiment-design` / `metallographic-analysis`），与文档清单一一对应；`skills/bundled.rs` 负责首次播种到用户目录 |
| 53    | 创建 Skill：Name / Description / System Prompt / Tools / Agent / Model / Knowledge Base；启用/禁用/复制/编辑/删除                  | ✅  | `SettingsSkillsPanel.tsx` 页面内**创建 / 编辑 / 复制 / 删除**（`createSkill` / `updateSkill` / `duplicateSkill` / `deleteSkill`，写入用户目录）；启停、tags、版本、`content_sha256`、12 个上限、错误列表均已实现                                                                                                                 |


---

## I. 模型与报告（54–56 章）— 代码对照

| 章  | 要求                                                                                                     | 状态 | 证据                                                                                                                                                                                                                       |
| -- | ------------------------------------------------------------------------------------------------------ | -- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 54 | 模型管理：多 Provider（OpenAI / Anthropic / DeepSeek / Qwen / OpenRouter / Local / Custom API）；每个 Provider 含 API Key / Base URL / Models / Default Model | ✅  | `providers/profiles.rs` 的 `ProviderKind` = `deepseek` / `anthropic` / `qwen` / `open_ai_compatible` / `ollama`（另有 `siliconflow` / `mineru` 供 embedding·rerank·解析槽位）；`SettingsProviderCard.tsx` 提供类型下拉并自动填 Base URL 与默认模型；OpenAI、OpenRouter、Custom API 由「OpenAI 兼容 + 自定义 Base URL」覆盖；默认模型用 `setDefaultProvider` |
| 55 | 模型配置：Provider / Model / API Key / Base URL / Temperature / Max Tokens / Context Length / Timeout；支持测试连接 / 设置默认 / 删除              | ✅  | `SettingsProviderCard.tsx`（Provider / Model / API Key / Base URL / Temperature / Max Tokens）+ `ModelRuntimeSettings.tsx`（Context Length、单次模型请求超时秒）；动作齐全：`testProviderProfile`（测试连接）/ `setDefaultProvider`（设置默认）/ `deleteProviderProfile` + `deleteProviderSecret`（删除） |
| 56 | 科研报告：研究报告 / 实验报告 / 文献综述 / 技术报告；PPT / Markdown / PDF / Word；过程 数据→分析→图表→结论→引用→报告                        | ✅  | `features/reports/ResearchReportPage.tsx`：4 种报告类型；可选数据集 → `analyzeSteelDataset` → `reportCharts.ts` 生成「字段均值柱状图 + 相关性热力图」；导出 Markdown / Word（`docx`，图表栅格化为 PNG 嵌入）/ PPT（`pptxgenjs`，每图一页）/ PDF（打印 HTML 内联 SVG）；生成提示词要求逐条给出可追溯来源（作者 / 文档名 / 年份 / 来源位置），无证据处标注「未记录」 |

> 报告图表为**自包含 SVG**（显式颜色、不依赖 CSS 变量），同一份产物三处复用：预览内联、PDF 内联、Word/PPT 经 `svgToPng` 栅格化后嵌入。

---

## J. 设置（57–62 章）— 代码对照

| 章  | 要求                                                              | 状态 | 证据                                                                                                                                            |
| -- | --------------------------------------------------------------- | -- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| 57 | 设置页：左菜单 + 右内容；账户/模型/常规/外观/知识库/Agent/MCP/Skills/数据/快捷键/存储/关于     | ✅  | `SettingsPagePanel.tsx` 11 个 tab：account / providers / general / appearance / knowledge / agent / mcp / skill / databases / shortcuts / about |
| 58 | 常规：启动行为、默认页面、自动保存、自动更新、确认提示、通知                                  | ✅  | 默认页面、恢复会话、保存草稿、自动更新、通知、显示工具详情、危险操作确认                                                                                                          |
| 59 | 外观：Light / Dark / System；字体大小、界面密度、Sidebar 宽度、Agent Panel 显示、动画 | ✅ | `SettingsPreferencesPanel.tsx`：主题（Light / Dark / System 卡片预览）、语言、**字体大小**（小 / 标准 / 大）、**界面密度**（紧凑 / 舒适 / 宽松）、**侧栏宽度**（窄 / 标准 / 宽）、**动画**开关、**Agent Panel** 开关；偏好写入 `ui.preferences` 并即时应用到 `document.documentElement.dataset`（含 `data-sidebar-width`，由 `suna-shell.css` 消费） |
| 60 | 数据存储：数据目录、知识库目录、模型目录、缓存、日志；打开目录 / 更改目录 / 清理缓存 | ✅ | `StorageDataPanel` 可选择空目录、确认、取消待迁移操作、重启迁移及查看结果。下次启动在数据库/任务初始化前复制并校验完整数据树，SQLite 使用 `VACUUM INTO` 保留已提交 WAL，重定位自有路径，成功才持久化新目录；失败继续使用原目录，原文件与部分副本均保留。拒绝同目录/嵌套/非空/符号链接/Junction，并检查空间；`SUNA_DATA_DIR` 覆盖时提示并禁止 UI 切换。真实重启迁移、历史保留、路径重定位、再次重启、取消、失败回退均已验证。知识内容目录修正为 `objects`，模型存于 SQLite，显示其所在根目录。 |
| 61 | 用户中心                                                            | ✅  | `SettingsAccountPanel.tsx`                                                                                                                    |
| 62 | 关于                                                              | ✅  | 设置页 about tab（版本、Rust、Tauri、React、License、GitHub）                                                                                             |

## K. UI 规范与交互（63–74 章）— 代码对照

| 章  | 要求                                                                                                                                | 状态     | 证据                                                                                                                        |
| -- | --------------------------------------------------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------- |
| 63 | 组件规范：Button / Input / Textarea / Select / Dropdown / Dialog / Drawer / Tabs / Card / Table / Badge / Tooltip / Toast / Command Menu / Progress / Avatar | ✅ | `components/ui/` 22 个 shadcn 风格组件（16 个 `@radix-ui/*` 依赖）；本次补齐 **Toast**（`toast.tsx`：Provider + `useToast`，挂载于应用根部）与 **Command Menu**（`command-menu.tsx`：Ctrl+K 命令面板，支持分组） |
| 64 | Card：白底 + border + 轻圆角 8–12px；禁阴影/渐变/玻璃                                                                                           | ✅      | CSS 统一实现                                                                                                                  |
| 65 | Typography：Inter / 系统字体；中文 Noto Sans SC；标题 20–28px、正文 14–15px、辅助 12–13px                                                          | ✅      | `@fontsource/inter`、`@fontsource/noto-sans-sc`、`@fontsource/jetbrains-mono`                                               |
| 66 | 图标：Lucide，统一线性，不混用 Emoji / 3D                                                                                                     | ✅      | `lucide-react` 全局使用                                                                                                       |
| 67 | 表格：可排序 / 筛选 / 分页 / 复制 / 导出 | ✅ | 新增 `components/ui/data-table.tsx` + `dataTableModel.ts`（排序 / 筛选 / 分页 / TSV 复制 / 带 BOM 的 CSV 导出，纯逻辑含单测），落地为数据实验室「数据表」页签（`DataLabTablePanel`） |
| 68 | 所有支持文件的页面支持 Drag & Drop，显示上传/解析/Embedding 进度 | ✅ | 新增通用 `components/common/FileDropZone.tsx`（扩展名校验 + 拖拽遮罩 + 拒绝反馈），已接入数据实验室、文献研究、工艺优化；知识中心与聊天输入框原有拖拽保留；解析 / Embedding 进度由知识中心 `IngestionJobs` 统一展示 |
| 69 | Streaming：思考/检索/分析/工具/生成状态；不暴露 CoT                                                                                                | ✅      | `reasoning_delta` / `reasoning_completed` 独立事件；检查器只展示高层状态                                                                 |
| 70 | Agent 运行日志：Task / Agent / Tool / Status / Duration / Result                                                                       | ✅      | `AgentRunInspector` + `ChildAgentPanel`                                                                                   |
| 71 | 错误处理：不直接显示 "Error"；给出重试 / 修改 / 查看详情 | ✅ | 新增共享 `utils/errorMessage.ts`（脱敏 + 归类 connection / tool / data / permission / timeout，导出 `describeError`），`settingsError.ts` 改为委托；全应用 **67 处**内联 `instanceof Error ? …` 已收敛到统一入口 |
| 72 | 全局搜索 Ctrl+K：对话 / 知识库 / 文献 / Agent / Skill / MCP / 模型 / 设置 | ✅ | 新增 `app/globalSearch.ts`：静态实体（知识库 / Agent / Skill / MCP / 模型）加载一次后本地过滤，文献走后端检索；命令面板按「页面与设置 / 对话 / 知识库 / Agent / Skill / MCP / 模型 / 文献」分组 |
| 73 | 快捷键：Ctrl+N 新对话、Ctrl+K 全局搜索、Ctrl+Shift+K 知识库、Ctrl+Enter 发送、Esc 停止 | ✅ 5/5 | 新增共享 `app/shortcuts.ts`（SunaApp 与设置面板共用，避免两边漂移）；补齐 **Ctrl+Shift+K → 知识中心**；设置面板可改键并持久化到 `ui.shortcuts` |
| 74 | 本地数据安全：本地优先、API Key 安全存储、敏感数据不入日志                                                                                                 | ✅      | 密钥存 Windows Credential Manager；`diagnostics/redaction.rs` + panic hook 脱敏；本次已把日志统一到脱敏出口                                   |

---

## L. 工程结构（75–82 章）— 代码对照

| 章  | 要求                                                                                                                                                                         | 状态              | 证据                                                                                                                                                                                                                                          |
| -- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 75 | Rust 模块：agent / llm / tools / mcp / knowledge / database / security / commands                                                                                             | ✅ 8/8 概念覆盖，命名不同 | `agent/` ✓、`tools/` ✓、`mcp/` ✓、`database/` ✓、`app/`(≈commands) ✓、`providers/`(≈llm) ✓、`rag/`+`knowledge_db.rs`(≈knowledge) ✓、`permissions/`(≈security) ✓                                                                                    |
| 76 | React 结构：components / pages / stores / hooks / services / types / utils | ✅ | `components/` ✓、`features/`(≈pages) ✓、`stores/` ✓、`bridge/`(≈services) ✓、`utils/` ✓；本次补齐 **`hooks/`**（`useDebouncedValue` / `useDebouncedCallback`，已用于知识中心筛选与设置面板自动保存，含单测）与 **`types/`**（`types/navigation.ts` 承载 `SectionId`，`components/ui/command-menu.tsx` 改为依赖 `types/` 而非 `app/`） |
| 77 | 状态管理用 Zustand；8 个 store（chat / agent / model / knowledge / mcp / skill / settings / task）                                                                                  | ✅               | `zustand` 依赖 + `src/stores/` 11 个文件；8 个 store 全部建立并接线，4 处 Context（appearance / theme / i18n / chatController）迁移为薄适配层                                                                                                                        |
| 78 | Chat State：conversationId / messages / model / agentMode / attachments / streaming / taskStatus / activeAgents / toolCalls                                                 | ✅               | `chatController` + `agentEvents.ts` 的 `AgentRunView`                                                                                                                                                                                        |
| 79 | Agent State：agentId / name / status / model / currentTask / tools / skills / progress / logs                                                                               | ✅               | `agent/profiles.rs` + `AgentRunView` + `ChildAgentTurnRecord`                                                                                                                                                                               |
| 80 | 数据模型 14 个：Conversation / Message / Agent / Tool / Skill / MCPServer / KnowledgeBase / Document / Dataset / Model / PredictionTask / OptimizationTask / Experiment / Report | ✅ 14/14 落表 | `0033_data_model_completion.sql` 建 agents / tools / skills / prediction_tasks / optimization_tasks / experiments；本次新增 `0035_reports.sql` 补上 **reports**（此前文档声称已落表、实际缺失）；新增迁移测试 `every_documented_data_model_has_a_table` 逐表断言 14 个模型 |
| 81 | 页面路由 `/chat` `/knowledge` …                                                                                                                                                | ✅               | `src/app/sectionRoute.ts`：17 条路径映射 + `useSectionRoute`（`pushState` + `popstate`），根路径 `/` 归为 chat；第 81 章要求的 10 条路由全部覆盖                                                                                                                       |
| 82 | 主界面最终结构（三栏 ASCII 图）                                                                                                                                                        | ✅               | 与 `SunaApp` 实际结构一致                                                                                                                                                                                                                          |

## M. 设计原则与开发要求（83–89 章）— 代码对照

| 章     | 要求                                                                       | 状态     | 证据                                                                                                                   |
| ----- | ------------------------------------------------------------------------ | ------ | -------------------------------------------------------------------------------------------------------------------- |
| 83    | 10 条产品设计原则（Agent 优先、聊天是入口、结果可追溯、行为可观察、用户有最终决策权…）                         | ✅      | 事件流 + 引用 + 权限确认 + 检查器均体现                                                                                             |
| 84    | 不允许出现的设计（渐变/金属纹理/机器人头像/炫酷发光/3D卡片/复杂背景/大量 Emoji/过度圆润）                     | ✅      | 代码中无违反项                                                                                                              |
| 85–86 | 最终视觉目标与一句话定位                                                             | ✅      | 与 `README.md` 一致                                                                                                     |
| 87    | Phase 1–8 开发顺序                                                           | ✅ 完成   | Phase 1 界面 / 聊天 ✓；2 LLM Provider / Streaming ✓；3 知识库 RAG ✓；4 文献 / 数据实验室 ✓；5 预测 ✓；6 优化 ✓；7 实验 ✓；8 MCP / Skills / Multi-Agent ✓ |
| 88 | 所有页面统一 Sidebar / Header / 字体 / 颜色 / Button / Card / Table / Dialog / 状态色 | 🟡 | 共享 Button / Card / Table / Dialog 与主题令牌已有实现；17 页浅深色实机截图复核发现 Agent 管理、数据实验室、知识中心、预测、优化、报告、Skill、工具及聊天欢迎卡片仍有深色背景/文字对比度不一致，不能据此认定视觉统一验收通过。 |
| 89 | 可长期迭代的桌面 AI Agent 产品 | 🟡 | Rust lib **324 passed / 2 ignored**，`architecture` 28 项、`agent_protocol_contract` 3 项；前端 **37 文件 / 236 用例**、边界检查 **133 文件**、生产构建通过。真实验证已有记录，但完整业务状态和历史视觉回归尚未验收。 |


## 缺口汇总（按补齐优先级）

### P1 — 科研能力深度（文档明确要求、当前缺失）

> 原 P1 各项（数据可视化类型 / 数据分析 / 模型比较 / 模型解释 / 优化算法族 / Pareto 可视化 / 实验设计算法 / 报告导出格式）**已全部补齐**，见 E–I 组各章证据。

（当前无剩余 P1 项）

### P2 — 一致性与体验

> 原 P2 五项（统一错误处理 / 全局搜索覆盖面 / 拖拽上传 / 表格能力 / Ctrl+Shift+K）**已全部补齐**，见 K 组各章证据。

本轮新增确认项：第 88 章深色样式一致性，见真实截图与验证记录；历史像素基线比较及完整业务状态覆盖仍待完成。

### P3 — 未按文档实现的结构项

> 说明：以下各项均为 `开发文档.md` 的明确要求，**现已全部按文档补齐**。

1. ~~状态管理（77 章）~~：已改为 Zustand（`src/stores/` 11 个文件）。
2. ~~UI 组件库（4 章）~~：已引入 shadcn/ui + Radix UI（16 个 `@radix-ui/*` 依赖）。
3. ~~专业 Agent 形态（6 章）~~：已定义 Master + 8 个专业子 Agent 预设（`agent/profiles.rs::presets()`）。
4. ~~页面路由（81 章）~~：已有 URL 路由（`src/app/sectionRoute.ts`，17 条路径映射）。
5. ~~数据模型落表（80 章）~~：14 个模型已全部落表（`0033` 补 6 张、`0035` 补 `reports`），并由迁移测试逐表断言。

---

## 与 `开发文档.md` Phase 1–8 的对照

| Phase | 文档目标                                                            | 实际   |
| ----- | --------------------------------------------------------------- | ---- |
| 1     | Desktop UI、Chat、Sidebar、Agent Workspace、Model Selector、Settings | ✅ 完成 |
| 2     | LLM Provider、Streaming、Conversation、File Upload                 | ✅ 完成 |
| 3     | Knowledge Base、RAG                                              | ✅ 完成 |
| 4     | Literature、Data Lab                                             | ✅ 完成 |
| 5     | Prediction                                                      | ✅ 完成 |
| 6     | Optimization                                                    | ✅ 完成 |
| 7     | Experiment                                                      | ✅ 完成 |
| 8     | MCP、Skills、Multi-Agent                                          | ✅ 完成 |

---

## 评估边界

- 本轮复核：前端 **37 文件 / 236 测试**，边界检查 **133 文件**，`tsc` 与生产构建；Rust lib **324 passed / 2 ignored**，`architecture` 28 项，`agent_protocol_contract` 3 项。最后的存储环境变量提示改动另通过 3 文件 / 36 项相关测试与生产构建。worker 上轮结果为 125 passed / 2 skipped，本轮未重跑。
- PostgreSQL **18.3 / pgvector 0.8.2**：执行生产词法、向量、两条混合检索、来源过滤、阈值和 EvidencePack SQL；临时 schema 在单连接事务内创建，全部回滚并断言无残留，未删除或改写用户原知识库数据。
- 百炼 **qwen3.8-flash**：使用本机 `DASHSCOPE_*` 环境变量，经生产 Provider 收到真实流式事件与指定响应标记。密钥未写入测试文件或日志。独立云 Embedding/Rerank 未调用，固定向量 SQL 测试不能替代该验证。
- 真实文献：Europe PMC OA 文献 `10.1038/s41598-026-65241-w` 正文下载、生产 ingest/parse/chunk 得到 **38 个片段**，经 PostgreSQL 生产文献检索与章节读取通过。修复了章节模式 `section` 被误加为检索词导致空结果的问题。产品检索入口仍是本地知识库，本轮没有新增在线文献源 Provider。
- 真实 Windows Tauri/WebView2：柱状图与热图 SVG → PNG，尺寸 **1440×640 / 1440×380**，含中文及负数；DOCX/PPTX 导出并检查嵌入 PNG。17 页 × 浅/深色共 **34 张截图**，无 pageerror / 水平溢出，聊天输入框已修复为留在窗口内。截图基线已建立，**没有历史基线或独立第二轮像素比较**；截图亦暴露部分深色视觉问题，不能宣称逐页回归全通过。
- 原生拖拽：用户手工拖入 PDF、实机 Tauri `drop` 事件及附件路径均已验证；共享路径注入 hook 含 DPI/区域过滤及订阅清理测试。自动 OLE 拖拽受焦点/DPI 干扰，没有计入稳定通过项，其脚本已移除。
- 目录迁移：隔离测试数据上的实际 IPC、UI 待迁移提示、重启、文件/历史/路径验证、再次重启持久化、取消和非空目标失败回退全部通过；用户实际数据根目录未切换，源目录未删除。
- 用户反馈后已停止窗口/鼠标测试并关闭本轮测试进程及 Vite 服务。详细复现命令、证据路径和未覆盖项见 `docs/validation/2026-10-11-live-validation.md`。
