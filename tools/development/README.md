# 科研功能接入前的隔离验证

这里保存可运行的代码与测试，供后续接入主程序。当前没有注册新的桌面命令、Agent 工具或任务处理器，也没有迁移用户主库。

| 模块 | 实际实现 | 当前限制 |
| --- | --- | --- |
| research-workflow | 模板版本存储、只读预检、原 Scheduler/Worker 双模型闭环、来源 DAG、取消/超时维护、恢复与状态投影 | 仅合成临时库；权限入口、主程序调度接入、登记事务守卫和非激活登记仍待完成 |
| research-workflow-ui | 模板编辑、保存、预览确认、事件刷新、取消和重试交互 | 服务通过依赖注入测试，尚未连接真实 Tauri |
| research-package | Rust 元数据 ZIP、原成果摘要、引用去重、严格解析、安全预算、血缘校验、临时 SQLite 导入模拟 | 不恢复模型/文件，不导入用户工作区，协议仍为 draft.v1 |

全部数据夹具均为合成数据。文档中的测试通过只表示已运行的云端测试；不能推断 Windows 原生弹窗、安装、凭据或授权服务通过。

先准备仓库已有的锁定 Worker 与前端依赖，然后在仓库根目录执行：

```powershell
pwsh -NoProfile -File tools/development/test-validation-labs.ps1
```

脚本复用 `src-tauri/target` 和 `frontend/node_modules`，不安装新依赖、不读取用户数据库。Rust 使用各自已提交的锁文件；前端验证工具创建仅供 Node 使用的依赖目录链接，Windows 使用普通目录 junction。

也可分别在两个 Rust 模块中运行 `cargo test --locked --lib -- --test-threads=1`，或在根目录运行 `node tools/development/test-workflow-ui.mjs`。

跨语言互读可在准备好同一环境后运行 `python tools/development/test-archive-compatibility.py`；只生成临时合成清单和 ZIP，退出后自动清理。

Windows 验收记录工具另见 [tools/windows-acceptance](../windows-acceptance/README.md)。
