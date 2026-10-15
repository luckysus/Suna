# 科研项目元数据包 v1：接入与验证

基线为 `luckysus/Suna` main `1a13acd6ddd28b06e7fe2c22c6de5fddd166e610`。其 [Windows Run 38578642205](https://github.com/luckysus/Suna/actions/runs/38578642205) 已核对 31 项全部 success，包含安全、全部测试、案例、四项基准及清理。随后才将隔离实现接入 main 工作区。本里程碑自己的 SHA 和完整 Actions 必须在提交推送后另行核查，不能沿用基线结论。

## 实现范围

- 从现有科研项目和不可变成果仓库读取实际分页、历史摘要和 DAG；通过同一 SQLite 读取快照生成严格白名单元数据，保留原 UUID、成果/数据/模型 SHA、模型注册版本、模型对象协议版本、算法、来源任务及已知单位。源库不写入。
- 正式协议为 `research-package-manifest.v1`，明确 `metadata_only`、`assets_included=false`、`provenance_mode=references_only`、`rights_status=not_assessed`。不复制原始行、模型对象、全文、项目备注、配置或凭据；不把历史摘要重算成精简记录的摘要。
- 真实 Tauri 命令 `preview_research_package`、`export_research_package`、`inspect_research_package_file` 已注册。项目的“成果与报告”及“活动与溯源”视图可预览实际数量/字节、确认名称与标题可分享、选择位置导出并处理取消/失败。再次导出重新读取项目，摘要变化必须重新预览。
- 复用现有 Rust 原生文件选择、覆盖确认、授权写盘和回读校验。新命令不接受前端绝对路径；检查文件使用既有路径/打开句柄授权，结果不包含生产路径。没有新增权限、依赖或第二套项目/模型注册表。
- ZIP/JSON 检查拒绝穿越、别名、链接、加密、重复项、未知字段/版本、CRC/长度/SHA 不一致、过深 JSON、超量条目、过大文件及高膨胀比；引用缺失、有向环或同资源冲突摘要时拒绝继续。不提取或执行文件。
- 检查仅创建独立临时 SQLite，在事务中生成新 UUID 映射并重复导入核对幂等；失败回滚。界面明确外部资源 missing、检查范围为临时副本；`assetsRestored=false`、`writesToLiveWorkspace=false`。没有正式主库导入、资产恢复、模型激活或实验测量写入。

先审计的已有功能是 JSON `research-manifest.v1`、项目报告、备份格式 1/2，以及 `tools/development` 下的隔离 draft 原型。它们保留原用途；新功能是应用入口中的正式元数据 ZIP 协议，不改变旧备份。协议和预算见 [科研包 v1](../research-package-v1.md)。

## 实际云端验证

环境为 Linux、Rust 1.99、Node 24、现有锁定依赖。数据均为合成夹具，未读取用户项目或凭据。

| 验证 | 实际结果 |
| --- | --- |
| 主程序全部 97 个 Cargo 目标，逐目标 `cargo test --locked --jobs 1 ... -- --test-threads=1` | 1113 passed，0 failed，7 项原有 ignored |
| 其中归档模块：实际仓库、临时库、文件授权、恶意 ZIP 和回滚 | 18 passed，0 ignored |
| 完整前端 Vitest（61 个文件） | 315 passed |
| TypeScript、生产构建、运行边界 | 通过，165 个运行文件；保留现有大块构建告警 |
| 独立 Python ZIP/哈希核对 | 1101 条记录、1100 个原成果摘要、4 条血缘边逐项一致；临时重复导入幂等 |
| Rust 格式、空白检查、依赖/权限差异核对 | 通过；Cargo/Worker 锁文件、CI、安全规则、capabilities 未改 |

完整 Rust 使用 Cargo metadata 枚举所有目标，在已完成后删除忽略目录中的生成测试可执行文件以控制云端空间；未减少目标、过滤用例或新增 ignore。归档模块还曾在隔离工作区先行验证 18 项；前端测试和构建通过后，逐文件核对复制到 main。main 的生产构建亦实际执行。Python 检查使用真实 Suna 仓库生成的千对象合成清单，不是手写理想血缘。

云端原始记录位于 `/workspace/suna-independent-development/`：`package-main-rust-summary.json`、`package-main-rust-all-targets.log`、`package-main-frontend-tests.log`、`package-main-frontend-build.log`、`package-main-frontend-boundaries.log`、`research-package-v1-cross-check.log`。这些临时测试文件和二进制不提交到仓库。

提交 `c6eabf8a7726ef881ff1852e40122fdd36e61b19` 的 [Run 38654574937](https://github.com/luckysus/Suna/actions/runs/38654574937) 后续实际失败：用户提供的真实日志显示科研包18项测试全部通过，库测试整体476 passed / 1 failed / 3 ignored，唯一失败为工作流恢复测试等待中间状态超时。后续安全、案例及基准步骤未执行，不能视为科研包的完整Windows验证；修复及最终提交CI记录见 [工作流恢复记录](2026-10-15-workflow-recovery.md)。

Python Worker 源码本次未改；基线完整 Windows CI 已通过其锁定测试，新提交 CI 仍须按精确 SHA 重跑。现有 Agent 继续从 ProviderProfile 读取地址/模型、通过 SecretRef 取得凭据；本功能没有外部模型 API、硬编码用户服务或真实大模型调用验收。无 API 配置不会影响本地归档/预测/优化开发，真实服务兼容性仍 pending-network。

## 剩余验收

用户个人 Windows 的原生文件选择、取消、覆盖、权限拒绝、磁盘满、DPI/键盘、安装及 Credential Manager 仍 `NOT_RUN / pending-native`；SQL Server/授权服务仍 pending-network。具体记录项已补入 [Windows 手册](../development/windows-research-acceptance.md)，不能把 CI 成功当作个人电脑验收通过。

首期没有授权资产打包、许可证审核、跨工作区正式导入、外部文件重定位或跨机器完整复现。默认最小化分享不代表资源已获授权；收到的引用及哈希也不等同于完成原资产或科研结论的真实性审查。
