# 第四、第五部分交付状态

基线与避免重复开发的审计见 [parts-4-5-audit.md](parts-4-5-audit.md)。本轮完成文档规定的优先阶段；以下状态区分已实现与未实现，不将后续工作算作完成。

| 里程碑 | 状态 | 可用闭环 |
|---|---|---|
| M5.1 | 已实现并验证 | 持久化项目、工作区隔离、现有对象关联、归档和当前项目 |
| M5.2 | 已实现并验证 | 原任务映射、不可变成果、提交来源冻结、多模型血缘、对账 |
| M5.3 | 已实现并验证 | 独立执行/复测、逐项测量、追加审核、偏差、新数据版本 |
| M4.1 | 已实现并验证 | 字段角色/别名、显式单位、映射版本、专业规则、冲突及候选复核 |
| M4.2 | 已实现并验证 | 真实调度与 Worker 闭环、手动再训练、预算/去重/约束的 GP 下一轮推荐 |
| M5.4 | 未实施 | 项目专用 Agent Tools、完整项目报告/manifest、跨页双向深链接 |
| M5.5 | 未完成 | 大规模分页/性能、可访问性、双主题/DPI 和 Windows 桌面全面验收 |
| M4.3 | 未实施 | 显微图像标尺/ROI/统计工作台 |
| M4.4 | 未实施 | 在线元数据发现、DOI 去重与合法全文工作流（已有本地证据功能保留） |
| M4.5 | 未实施 | 文件型专业计算适配与增强工业导入（已有只读 SQL 连接保留） |
| M4.6 | 未完成 | 发布硬化和 Windows 原生/安装包验收 |

五项优先里程碑完成 5/5。全文共有 11 项里程碑，不能据此宣称全部功能完成。按数量计为 5/11，并非工作量估算。上传文档允许大型任务优先交付可完整验收的核心闭环，其余在此明确列出。

验证使用临时 SQLite、中文临时目录与明确标识的合成算法夹具，不触碰用户生产数据库。真实闭环测试启动原 Rust Scheduler 和 Python stdio Worker，训练 Linear Regression 与 Random Forest、执行带混合单位设备规则的 NSGA-II、取得至少五个建议，审核其中两项测量，追加到新数据版本，再训练并检查原产物保持完整，生成三项下一轮推荐并重开数据库核对成果。

可复核命令（按当前环境安装 Python/Rust/桌面依赖；不关闭校验）：

- `npm --prefix frontend test`、`npm --prefix frontend run build`、`npm --prefix frontend run test:boundaries`
- `compute-worker/.venv/bin/python -m pytest compute-worker/tests -q`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib storage::repositories`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib material`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --test research_feedback_loop`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked --test domains --test steel_optimization_task`

`research_feedback_loop` 自动选择 Unix/Windows 本地 venv，也接受 `SUNA_COMPUTE_WORKER_PYTHON` 指向已安装计算依赖的 Python。云端 Linux 检查使用运行时图标配置覆盖和本地系统库，没有更改 Windows 打包配置。Windows WebView2、原生保存/覆盖/中文路径、系统凭据及打包 Worker/安装包仍需原生验收，未宣称通过。
