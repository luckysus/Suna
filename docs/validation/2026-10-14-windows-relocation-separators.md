# Cloud-0：Windows JSON 路径断言修复

- 基线：`022d627daf36869cdfcaa76a8757472b2c8a86c8`；已 fetch 并核对最新 main，无其他未提交改动。
- 上一轮 [Run 38423460599](https://github.com/luckysus/Suna/actions/runs/38423460599) 的完整真实 Windows 日志由用户提供：376 passed、1 failed、3 ignored，原四项失败全部通过。
- 唯一失败：`storage::data_directory::tests::relocation_keeps_real_frozen_artifact_hashes_and_is_idempotent`，JSON 路径的 `objects\\paper.txt` 与期望字符串 `objects/paper.txt` 分隔符不同。

## 原因与改动

`relocate_path` 通过组件比较确定源目录边界，再逐个 `PathBuf::push` 构造目标路径；`relocate_json` 序列化这个本地文件系统路径。Windows 生成本地反斜杠，而测试的 `join("objects/paper.txt")` 保留混合分隔符。二者路径组件相同，项目没有要求运营路径 JSON 使用统一的 URL/跨平台分隔符格式。因此是测试字符串断言错误，未修改生产迁移、序列化、授权或 Windows 设备路径拒绝逻辑。

- JSON 内已重定位路径和目录改用 `Path` 组件语义比较，同时明确文件不再指向 source。
- 外部路径仍逐字节比较，标题路径仍不重定位；历史成果 payload 与 SHA 保留，并增加第二次重定位后的历史成果校验。
- 新增跨平台 JSON 序列化往返回归，检查中文/空格、目录边界、目标组件、原文件、外部字符串和序列化幂等；Windows 额外实际输入全正斜杠和混合分隔符。
- 原测试的 SQLite `integrity_check`、原文件内容、历史哈希、重复数据库重定位检查全部保留。
- 无数据迁移、依赖、用户配置、安全权限、测试忽略或 CI 门禁变更。

## 本地验证

以下实际在 Linux 执行，使用现有锁定 Rust 环境；测试图标仅通过运行时 `TAURI_CONFIG` 覆盖，Windows 打包配置不变。

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | 通过 |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib storage::data_directory::tests --jobs 1 -- --test-threads=1` | 7 通过、0 失败 |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib --jobs 1 -- --test-threads=1` | 374 通过、0 失败、3 原有忽略 |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --test permission_paths --test permission_paths_common --jobs 1 -- --test-threads=1` | 公共路径 2 通过；Windows-only target 在 Linux 为 0 测试，不能算 Windows 通过 |
| `git diff --check` | 通过 |

当前环境原始日志：`/workspace/suna-independent-development/cloud0-separator-rust-tests.log`、`cloud0-separator-rust-lib.log`、`cloud0-separator-permission-paths.log`。这次没有前端或 Worker 修改，不以重复运行无关本地测试代替完整 Windows CI。

## CI 与剩余验证

本文件写入时修复尚未推送，CI 为 pending。提交 SHA 可通过本文件的 git 历史获取；推送后必须检查精确 SHA 的完整 `Suna quality`，包括安全测试、案例、所有 benchmark 及最后步骤，不能仅以 Rust 通过判断全绿。后续最终结论保存在当前任务的独立 CI 证据，并在下一阶段验证报告中引用。

用户个人 Windows 安装、Credential Manager、原生保存/取消、真实 SQL Server 等仍为 `pending-native / pending-network`。本测试仅用隔离临时目录、SQLite 和合成记录，不访问真实科研数据或凭据。
