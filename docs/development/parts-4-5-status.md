# 第四、第五部分交付状态

基线与避免重复开发的审计见 [parts-4-5-audit.md](parts-4-5-audit.md)。本轮完成文档规定的优先阶段；以下状态区分已实现与未实现，不将后续工作算作完成。

| 里程碑 | 状态 | 可用闭环 |
|---|---|---|
| M5.1 | 已实现并验证 | 持久化项目、工作区隔离、现有对象关联、归档和当前项目 |
| M5.2 | 已实现并验证 | 原任务映射、不可变成果、提交来源冻结、多模型血缘、对账 |
| M5.3 | 已实现并验证 | 独立执行/复测、逐项测量、追加审核、偏差、新数据版本 |
| M4.1 | 已实现并验证 | 字段角色/别名、显式单位、映射版本、专业规则、冲突及候选复核 |
| M4.2 | 已实现并验证 | 真实调度与 Worker 闭环、手动再训练、预算/去重/约束的 GP 下一轮推荐 |
| M5.4 | 已实现并验证（Windows 原生待验收） | 冻结 Agent 上下文、受控 Tools、同任务来源链接、持久化报告和元数据 manifest |
| M5.5 | 云端硬化通过，Windows 原生待验收 | 对象搜索分页、稳定排序、迟到响应门禁、任务事件合并、Radix 焦点弹窗和窗口/主题验证 |
| M4.3 | 已实现并验证（Windows 原生待验收） | 管理原图、版本标尺/ROI、真实 Worker 阈值/连通区域统计、批量错误、追加审核、项目报告 |
| M4.4 | 未实施 | 在线元数据发现、DOI 去重与合法全文工作流（已有本地证据功能保留） |
| M4.5 | 未实施 | 文件型专业计算适配与增强工业导入（已有只读 SQL 连接保留） |
| M4.6 | 未完成 | 发布硬化和 Windows 原生/安装包验收 |

五项优先里程碑完成 5/5。全文共有 11 项里程碑，不能据此宣称全部功能完成。按已实现闭环数量计为 8/11（其中 M5.5 云端硬化通过，Windows 原生待验收），并非工作量估算。上传文档允许大型任务优先交付可完整验收的核心闭环，其余在此明确列出。

验证使用临时 SQLite、中文临时目录与明确标识的合成算法夹具，不触碰用户生产数据库。真实闭环测试启动原 Rust Scheduler 和 Python stdio Worker，训练 Linear Regression 与 Random Forest、执行带混合单位设备规则的 NSGA-II、取得至少五个建议，审核其中两项测量，追加到新数据版本，再训练并检查原产物保持完整，生成三项下一轮推荐并重开数据库核对成果。

可复核命令（按当前环境安装 Python/Rust/桌面依赖；不关闭校验）：

- `npm --prefix frontend test`、`npm --prefix frontend run build`、`npm --prefix frontend run test:boundaries`
- `compute-worker/.venv/bin/python -m pytest compute-worker/tests -q`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib storage::repositories`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib material`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --test research_feedback_loop`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --test domains --test steel_optimization_task`

`research_feedback_loop` 自动选择 Unix/Windows 本地 venv，也接受 `SUNA_COMPUTE_WORKER_PYTHON` 指向已安装计算依赖的 Python。云端 Linux 检查使用运行时图标配置覆盖和本地系统库，没有更改 Windows 打包配置。Windows WebView2、原生保存/覆盖/中文路径、系统凭据及打包 Worker/安装包仍需原生验收，未宣称通过。

## M5.4 续期交付

冻结每个新 Agent run 的项目上下文（迁移 0040），旧 run 保留未分配状态。项目工具沿用专业 Tool allowlist 和确认机制：列项目、读上下文、分页读成果、确认后关联已有成果。计算提交在原 Agent 来源事务内复用背景任务，切换当前项目不会改变运行中任务的归属。

项目活动可打开原任务和来源会话；预测页读取原任务的结果快照，优化页恢复原计算任务。项目报告从 SHA256 校验后的成果生成并持久化到原 reports，报告、链接、成果和血缘共同回滚；DOCX/PPTX/Markdown 使用现有原生导出，PDF 使用系统打印。manifest 只含元数据、哈希、引用和缺失提示，不含原始训练行、模型 Blob、凭据或原始绝对路径。它是复核清单，并不声称可以独立还原原始文件或导入恢复。

验证：仓储回归 37 项通过，随后追加 Agent 同一任务/幂等来源测试；前端 264 项通过，生产构建和运行时边界检查通过。Windows 原生对话框、WebView2 和系统打印仍待原生验收。保存的旧 Agent 自定义工具配置和已激活领域包不自动覆盖，新领域包版本 1.2.0 需按现有机制激活。


## M5.5 续期交付

分页读取可关联对象并按名称搜索；血缘独立分页，不再和 25 条成果分页共享 100 条关系的偏移。任务事件合并为 250ms 的一次读取，并用请求序号拒绝切换项目后的旧响应。项目编辑复用 Radix Dialog，提供焦点限制、Escape 和描述；长哈希与 JSON 不撑宽页面。

SQLite 测试 5/5 包含 275 对象分页与索引检查、0035 升级、0040 失败回滚、中文文件重开及设置保留。前端原有 264 项通过；新增对象分页和 Escape 测试单独验证。Chromium 4/4 包括既有联合优化/科研预测回归，以及科研项目浅深主题、1366×768、125%/150% 页面缩放、焦点与无横向溢出。这些是浏览器 IPC 夹具测试，系统 DPI/WebView2/原生文件对话框尚未运行。生产构建通过；现有大包体积警告保留，没有通过提高阈值隐藏。Windows 后续步骤见 [原生验收清单](windows-research-acceptance.md)。


M4.3 使用说明、科学语义、依赖及实际测试见 [显微组织工作台](microstructure-lab.md)。新增迁移0041保留旧数据；图像源与标注不覆盖，旧联合优化和实测回流集成测试仍通过。
