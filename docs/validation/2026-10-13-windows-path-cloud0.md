# Cloud-0：Windows 路径授权与迁移修复

- 仓库：<https://github.com/luckysus/Suna>
- 开工已执行 `git fetch origin main`；基线 HEAD/origin/main：`13b8418ee3c4d149e80efd61a014e534420081aa`。
- 本次代码提交可由 `git log -1 --format=%H -- docs/validation/2026-10-13-windows-path-cloud0.md` 复核。
- 原失败：[Run 38406843597 / windows-quality](https://github.com/luckysus/Suna/actions/runs/38406843597/job/115328570582)，公开状态确认 `Run deterministic Rust suite` 失败。
- GitHub API `gh run view --log-failed` 实际返回 Forbidden。四个失败单测和异常来自用户上传任务书的已见日志；没有把无法读取的完整远端日志声称为已读取。

## 审计与最小修复

| 已有实现 | 差距 | 本次改动与新增验证 |
|---|---|---|
| raw path 拒绝设备命名空间、UNC、ADS；授权结果保存 canonical path | 系统产生的 `\\?\C:\...` 再传入 raw path API，内部合法路径被拒绝 | `AuthorizedPath::local_path` 只从不可伪造的授权结果派生普通本地磁盘路径；不提供接受任意 verbatim 输入的授权入口 |
| 导出临时文件校验与选定路径保存 | 临时文件路径/授权根错误复用 canonical 字符串 | 在首次路径授权时冻结 roots；派生临时路径，写入前后保留句柄与目标检查，覆盖截断语义保留 |
| 实测审核生成新 CSV/成果/血缘；单位规范化生成新版本 | 存储 canonical 字符串导致后续数据预览/哈希授权失败 | 用授权结果派生可再次校验的路径；新文件创建、Windows 句柄检查和失败清理，不改旧文件、审核或规则 |
| 数据目录迁移用 VACUUM INTO 保留已提交 WAL，并校验副本 | 字符串表示比较无法识别 Windows TEMP 的短名/长名等价 | 按真实文件系统及最近存在祖先解析路径身份，再比较组件；支持已有文件和不存在末尾的别名/大小写/分隔符；拒绝设备/网络/ADS/父目录逃逸 |
| 历史成果/审计有不可变触发器和哈希 | 全表 JSON 重定位可能触发不可变限制或改变历史证据 | 跳过带不可变 UPDATE 触发器的表及设置审计；运营路径重定位，历史哈希与原始 payload 保留 |

没有数据库迁移、配置覆盖、签名/权限白名单放宽、依赖公告新增忽略、质量阈值或 workflow 触发规则修改。组件比较在 Unix 保持大小写敏感；Windows 比较本地规范路径并保留边界。源目录和失败副本的保留策略不变。

原迁移测试改为核对真实目标文件 canonical 身份，并明确断言不是源文件；不要求目标的短名和长名字符串完全相同。JSON 路径也核对目标身份。新测试使用真实最新 SQLite schema，验证不可变成果内容及 SHA 不变、JSON 数组和边界、重复重定位幂等。

## 测试证据

Linux 测试使用现有锁定工具链、临时数据目录和测试图标运行时覆盖；Windows正式图标与打包配置未改变。结果在提交前填写；原始报告保存在当前任务的 `/tmp/suna-cloud0-*.log`，不是跨任务永久日志。

- 第一轮 Rust lib：372 通过、0 失败、3 条件测试忽略；原四项中的三个跨平台业务路径测试和迁移测试通过。该轮不含随后新增的迁移回归。
- 完整 Rust：`cargo test --manifest-path src-tauri/Cargo.toml --locked --no-fail-fast -j 1 -- --test-threads=1`，92 个目标完成、1001 通过、0 失败、7 条件/显式测试忽略，退出0。原四项过滤名全部实际通过；`/tmp/suna-cloud0-all-rust.log`。
- 前端：`npm --prefix frontend test`，52 文件、279 通过、退出0；`/tmp/suna-cloud0-frontend.log`。
- Python：`compute-worker/.venv/bin/python -m pytest compute-worker/tests -q`，153 通过、2 条件跳过、48 已知警告、退出0；`/tmp/suna-cloud0-python.log`。
- `npm --prefix frontend run test:boundaries`，154 文件通过；生产构建通过、保留原大包提示，无阈值调整；`/tmp/suna-cloud0-build.log`。
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` 与 `git diff --check` 通过。当前环境未装 PowerShell，发布脚本合同由不变的完整 Windows CI 执行，不声称已在 Linux 运行 PowerShell。
- 新公共路径集成测试在 Linux 实际运行；Windows-only 回归等待该提交 runner，不能把 Linux 的零 Windows 测试算通过。
- Windows矩阵：中文/空格、>260路径、盘符大小写、分隔符、存在/缺失末尾、raw canonical 拒绝与 typed canonical 复用、设备/UNC/ADS、句柄移动/替换、junction 父目录替换/越界；8.3名称由真实 `GetShortPathNameW` 获取，文件系统没有短别名时明确输出限制。
- 覆盖确认、取消和中文保存对话框保留原有流程，必须按 W03/W06 在原生桌面执行，未模拟宣称通过。

## Windows CI 与人工验收

本修复推送后用 `python scripts/check-actions.py --sha <本次实际 SHA>` 等待 **完整** Suna quality 最终状态。结果将记录在下一阶段验证报告及交付汇报；此文件写入时尚未推送，不声明 runner 通过。

- `cloud-actual`：Linux 本地文件系统/SQLite/原 Worker 自动化验证，具体通过数以完成日志为准。
- `fixture-only`：合成记录和隔离路径负例，无真实研究数据。
- `pending-native`：用户电脑 W01–W07、W10–W13，尤其原生保存/取消、Credential Manager、安装包与数据目录迁移。
- `pending-network`：授权 SQL Server、Crossref、远端 RAG；本次不请求凭据、不访问生产库。

当前因 API 限制无法读取私有日志；公开页面仍可检查该 SHA 的步骤和最终状态。如果该 SHA 再失败，继续处理失败门禁，不能启动高耦合功能开发或用本地通过代替 CI 成功。
