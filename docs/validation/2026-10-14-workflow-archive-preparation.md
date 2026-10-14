# 科研模板、归档与 Windows 验收工具的隔离验证

本次提交保存可运行的开发实现和验证工具，没有把科研工作流注册为桌面或 Agent 功能，也没有向用户主库安装新表。正式接入仍是下一里程碑。

## 基线与完整 CI

主程序基线是 `c1f871b1320490631dd1394c40576250b5c92701`。其 [完整 Windows CI](https://github.com/luckysus/Suna/actions/runs/38527970264) 已核查 success，31 个步骤全部成功，包括 Rust、前端、Worker、安全、案例复现、四项性能基准及清理。本次准备代码尚未执行 Windows 原生验收；本提交自己的 Actions 结论必须另按新 SHA 核对。

此前模型卡提交 `6e8a172a43c762f12ff70feee5872abce3e9b330` 的 [完整 Windows CI](https://github.com/luckysus/Suna/actions/runs/38517617942) 也已全绿。保留已推送历史，后续提交署名为 luckysus。

准备提交 `815f391cc43f624bf90e24a72edf4277db6a1aa3` 的 [完整 Windows CI](https://github.com/luckysus/Suna/actions/runs/38559062057) 已在 2026-10-14 12:58:31 UTC 核对成功：31 个步骤全部 success。此结论仍不代表个人 Windows 原生验收通过。

## 真正运行的代码

- `tools/development/research-workflow` 引用实际 Suna 库和已锁定 Worker，在合成临时 SQLite 中保存模板版本、预检数据/单位/划分/规则、创建原 Scheduler 任务、训练两个独立性能模型、登记科研模型卡、执行联合优化、生成 proposed 计划。父子来源通过原 research_artifacts/research_run_sources 绑定，不建立第二套任务状态或模型注册表。
- 运行验证覆盖项目冻结、重复提交、模板修订不影响旧运行、登记检查点缺失后的恢复、失败节点安全重试、源数据变更、专业规则变更、确认哈希、目标泄漏防护、受控单线程超时、取消范围和脱敏状态投影。没有生成实验测量。
- `research-workflow-ui` 验证保存版本、只读预览、人工确认、过期响应、事件刷新、取消和重试的 React 交互。服务为测试依赖注入，尚未连接实际 Tauri；不能称为已交付应用页面。
- `research-package` 使用现有 Rust ZIP/SHA256/SQLite 依赖做 metadata_only 格式、严格 JSON、ZIP 路径和资源预算、原成果哈希、去重、引用/DAG 校验及临时导入模拟。不提取 ZIP，不执行包内文件，不接受用户主库连接，不恢复资产。
- `research-package-reference` 是 Python 协议对照，不加入 Worker 操作表。有效合成包可与 Rust 相互读取；生产接入必须使用 Rust 的严格检查。
- `tools/windows-acceptance` 核对明确提供的候选 manifest，并生成 W01–W13 待验收记录，不安装或启动任何候选程序，不读取凭据，不接受旧记录覆盖。

## 实际云端测试

复制到仓库并修正相对路径后已重新运行：

| 验证 | 真实结果 |
| --- | --- |
| 科研工作流 Rust／实际 Scheduler、Worker、SQLite | 26 passed，0 failed |
| 归档 Rust／实际项目仓库和临时库 | 12 passed，0 failed |
| 工作流 React 交互 | 6 passed，TypeScript 检查通过 |
| Python 归档协议对照 | 22 passed |
| 验收证据脚本 | 27 assertions passed，fixture-only |
| Rust/Python 双向归档检查 | 1101 条记录、4 条血缘边；同一包摘要一致，重复临时导入幂等 |

千对象验证使用实际 Suna 仓库生成 1100 个合成成果及对应链接，去重为 1101 条引用记录。逐个保留原成果摘要，源连接 total_changes 不变，合成私密路径、密钥、原始行和项目备注哨兵未导出。

全部数据是合成夹具；受控长任务是故障注入。验收脚本的模拟二进制仅是文本夹具，未作为程序执行。测试环境为 Linux、Rust 1.99、Node 24、锁定 Python 与 PowerShell 7.6.6。

复核命令：`pwsh -NoProfile -File tools/development/test-validation-labs.ps1`。另运行 `python tools/development/test-archive-compatibility.py` 可重新生成真实仓库合成清单，完成双向 ZIP 和临时导入检查。两个脚本均不安装新依赖。

## 正式接入仍须完成

模板版本表迁移、已有 Scheduler 维护入口、Tauri/Agent 确认权限、真实 Worker 依赖诊断、桌面与 Inspector、原任务取消入口尚未接入。模型登记还需在原登记事务内重核父任务取消/超时许可，并使工作流新版本保持非激活，随后验证并发取消、恢复及历史科研哈希。

归档协议仍为 `research-package-manifest.draft.v1`；正式协议、原受权文件服务、真实项目导出及文件选择后的安全检查需要下一阶段。临时导入状态始终 `metadata_only_missing_assets`，`assetsRestored=false`、`writesToLiveWorkspace=false`。真实主库导入、受管文件/模型、许可证、冲突处理和跨机器复现未完成。

个人 Windows 安装、Credential Manager、原生弹窗、DPI、SQL Server 和授权真实服务仍是 `NOT_RUN/BLOCKED` 或 pending-native/pending-network。完整 CI 不能替代这些验收。
