# 科研隔离库完整行核验与数据库快照

基线为 OpenAlex 提交 `658814b594a5cfc683a1f0d4695487a5ff43a3cd`。复用受管包、原始 ZIP、隔离目录、原生打开与重复导入入口，不新增导入模式或用户主库操作。

审计发现两项实际缺口：隔离 SQLite 的对象正文有核对，但资产和血缘只比较数量；收据中的数据库摘要可自行重算，不能独立证明这些行与原包一致。另一个问题是核对数据库文件字节之后又按源路径打开 SQLite，校验与使用可能读到不同文件。新增回归在旧逻辑下实际失败，分别显示 `forged role accepted`、`forged relation accepted` 和替换源路径后无法读取已核对快照，原始失败日志保留。

## 实现

数据库 SHA 核对后只把同一份有界字节载入只读内存 SQLite，不再按源路径打开或运行输入 SQL。启用 `trusted_schema=OFF` / `query_only=ON`，仅接受原应用创建的三张隔离表及原结构，额外表、视图和触发器拒绝。查询最多读取期望数量加一条，禁止截断后声称完整。

逐项核对原始对象 ID、唯一新 UUID、严格 JSON 正文和原记录摘要；资产的资源 ID、相对名、SHA、字节数和角色必须与原包完整集合相同；血缘端点必须是原包对象的新 ID 映射，关系和集合也必须完全一致。收据 ID 映射仍单独核对，原 ZIP 与全部真实资产的校验保留。哈希不是签名或来源授权证明，未增加此类宣称。

使用已锁定的 rusqlite 0.32.1 `serialize` 功能，不增加 crate 或变更锁定版本。该库 OwnedData 要求 SQLite 分配器；小型 unsafe 边界只分配已检查的非零长度、检查空指针、复制确切字节，并移交库的只读/关闭释放所有权。没有把 Rust Vec 指针交给 SQLite 释放。输入仍受原 128MiB 总预算及原严格元数据预算限制。

旧 SQLite 文件、收据、包、原成果和用户配置不改写。正常打开使用读取快照；另一进程后来修改文件不属于该快照，后续打开仍重新核验，不自动修复损坏。完整跨机器项目恢复和崩溃清理仍未实现。

## 实际验证

| 检查 | 结果 |
| --- | --- |
| 旧实现回归 | 3 项实际失败，复现资产、血缘及路径重开问题 |
| 修复后受管包专项 | 18 passed、0 failed、0 ignored，5.22 秒；包含真实模型/图像/数据集隔离、原始 Deflate ZIP、共享物理资产、旧收据兼容和链接拒绝 |
| 全部 Rust Cargo 目标 | 97 目标、1168 passed、0 failed、7 项原有 ignored；包含中断目标的实际续跑 |
| 前端 | 本期无前端代码修改，前一独立里程碑全量 358 passed 与生产浏览器 2 passed 仍为该源码验证记录 |

新增 5 项回归覆盖：重算收据后的五个资产字段、三个血缘字段，原包和故障数据库不被覆盖；校验后实际替换磁盘文件，内存仍只读取原核验字节，下一次打开拒绝损坏文件；额外表、视图、触发器、把资产表换成视图；重复 JSON 键不能利用 last-key-wins 掩盖身份。越界 ID 造假夹具显式关闭的是自身离线编辑连接的外键检查，生产外键和原项目权限保持。

复验：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked --jobs 1 --lib managed:: -- --test-threads=1
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
```

全部 Cargo 目标在本云端按每目标等价 `--locked --jobs 1 -- --test-threads=1` 执行，避免生成测试可执行文件耗尽临时盘；不删源码或用户数据。日志位于 `/workspace/suna-independent-development/isolation-snapshot-*`，仅合成夹具。

初次全量在已通过前63个目标后，`rag_mineru` 编译写增量缓存报 `No space left on device`，没有测试断言失败。确认没有编译进程后，只清理 `/tmp/suna-incremental-cache-20261014` 中2026-10-15之前生成的12个本地库缓存目录，保留当前缓存、全部依赖、源码、日志和科研数据，释放约9GiB。相同源码从 `rag_mineru` 本身及其后全部目标续跑通过，续跑91秒；最终计数合并前63个实际成功目标与后34个实际成功目标，没有跳过失败目标，也没有把首次构建失败隐藏为一次全绿。

## CI 与原生待办

最新已推送 `371d669` 的 [Run 38748036092](https://github.com/luckysus/Suna/actions/runs/38748036092) 在本期开发期间继续执行，没有为抢推新提交取消该运行。本期提交推送后须检查自己 SHA 的全部 Windows 步骤。此前 `3aeaacb` 的 [Run 38715025660](https://github.com/luckysus/Suna/actions/runs/38715025660) 全部 31 步成功；不能据此称本期已通过 Windows CI。

个人 Windows 文件系统竞态、原生对话框、磁盘不足/强制退出、安装包、Credential Manager、SQL Server 和真实服务仍 `NOT_RUN / pending-native` 或 `pending-network`。可控跨平台路径替换测试没有替代这些验收。
