# TLS 依赖安全修复与验证

科研工作流提交 `3cef7e1463a014b0ca8ca0ff38174b3034f8555a` 的 [Windows Actions](https://github.com/luckysus/Suna/actions/runs/38571314630) 在 `Run Rust dependency policy scan` 失败，其后的测试与性能基准均跳过。公开页面注解只有退出码 1，gh 日志接口返回 Forbidden，页面提供的详细日志地址返回 404。因此不能宣称已取得该运行的具体错误文本。

## 已取得的实际证据

在同一提交上安装并运行流水线使用的 `cargo-deny 0.20.2`，`cargo deny check` 实际失败：锁定的 `rustls 0.23.45` 命中以下漏洞公告；`cargo-audit 0.22.2` 使用原有参数独立检查，也实际报告 4 项漏洞。公告均指向 `>=0.23.46` 修复版本。

- [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346)：客户端会话值的特定 Debug 输出可能包含 TLS 会话密钥。
- [RUSTSEC-2026-0347](https://rustsec.org/advisories/RUSTSEC-2026-0347)：特定 ECH 拒绝场景可能向外层服务器发送已缓冲应用数据。
- [RUSTSEC-2026-0348](https://rustsec.org/advisories/RUSTSEC-2026-0348)：启用特定 Brotli 功能时，证书解压可能在认证前分配大量内存。
- [RUSTSEC-2026-0349](https://rustsec.org/advisories/RUSTSEC-2026-0349)：未认证握手可能影响后续连接的密钥交换预测。

以上是本地真实扫描证据，不表示 Suna 的所有运行配置均触发每项漏洞，也不替代旧 Actions 运行的详细日志。

## 修改范围

`Cargo.toml` 将 rustls 最低版本提高到 `0.23.46`，主程序与两个开发工具的锁文件仅把该依赖从 `0.23.45` 升到 `0.23.46` 并更新对应校验和。其余依赖版本与锁定关系保留。保持原 ring、std、tls12 功能与所有证书验证、安全规则、公告策略、原有例外及流水线步骤。

开发工具重测另外发现：科研模板的旧隔离建表操作与主程序第 44 版迁移重复。工具现在核对已有列、主键及不可变触发器并复用原表；原型空库仍按原流程创建表。新增回归验证重复初始化不写入或替换历史，缺少不可变触发器时拒绝继续。该工具不接入主程序运行入口。

## 验证

- `cargo deny check`：advisories、bans、licenses、sources 全部通过，未添加漏洞忽略项。
- `cargo audit --ignore RUSTSEC-2023-0071`：使用原参数通过，4 项新增漏洞报告消除；原有维护状态等告警仍保留。
- 主程序所有 97 个 Cargo 目标：1095 项通过、0 项失败、7 项原有忽略。云环境缓存空间不足时改为逐目标执行并清理已完成的生成可执行文件；未减少、过滤或忽略任何测试，未修改源码或数据以绕过测试。
- 科研工作流开发工具：27 项通过、0 项失败或忽略；科研包开发工具：12 项通过、0 项失败或忽略，均使用 `--locked`。
- 发布、生命周期、生命周期矩阵、更新通道、工作流、案例复现、依赖 7 组原脚本契约及 Rust 格式检查通过。
- 上一工作流里程碑的前端 307 项、Worker 194 项、构建、运行边界与原安全检查结果见 [工作流验证](2026-10-14-scientific-workflow.md)。本修复未改这些源码；新提交完整 Windows CI 仍须另核对精确 SHA 的全部步骤。

个人 Windows 安装、Credential Manager、原生弹窗、DPI、真实 SQL Server 和授权服务验收仍未完成。原备份协议、用户配置与科研数据保持不变。
