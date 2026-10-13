# 第四、第五部分续期云端验证

验证环境：Linux、Node24、Python3.12本地锁定venv、Rust及本地桌面库。Rust测试使用本地运行时图标配置，不修改Windows打包图标。全部数据为独立临时SQLite、合成图像/数值记录和回环HTTP夹具，没有访问用户生产数据库或修改已保存的模型/服务配置。

| 检查 | 实际结果 |
|---|---|
| 完整Rust套件，90个目标，`cargo test --locked --no-fail-fast -- --test-threads=1` | 998通过、0失败、7条件/显式测试忽略 |
| 显式科研基准（默认忽略） | 真实仓储API，1000成果、25条分页、100次读取，通过 |
| 前端Vitest | 279通过 |
| TypeScript/Vite构建、运行时边界、Rust格式及diff检查 | 通过；原有大包体积警告保留 |
| Python Worker pytest | 153通过、2条件测试跳过 |
| Chromium业务回归 | 6通过：独立多模型优化、科研预测/批量导出、显微坐标/持久化、公开题录、项目浅深主题与键盘 |

Rust完整测试包括原Agent、RAG、权限/凭据、升级、模板、统计、文件交换、真实stdlib帧Worker和科研反馈/再训练流程。新增诊断通过Rust→Worker实际加载依赖，不以文件存在代替可用。备份回归覆盖受管副本和旧测量/规范化数据、另一中文目录恢复、ID/哈希/审核记录不变、有效ZIP CRC但内容被改时拒绝，以及失败不触碰目标。

前端假时钟推进一小时保持任务运行，随后完成，检查没有固定轮询次数的伪超时；不代表实际运行一小时的性能验收。

实际合成内存SQLite基准（ms）：

| 操作 | p50 | p95 | max |
|---|---:|---:|---:|
| 项目成果分页 | 4.080 | 8.333 | 21.102 |
| 按ID读持久任务 | 0.076 | 0.199 | 9.005 |

该基准测查询接口，不代表Windows磁盘、安装包内存、百万行或真实24小时计算负载。已存任务的旧时间戳是显式状态夹具。

GitHub Actions每次按提交SHA复查，真实失败及修复见[跟踪记录](../development/actions-follow-up.md)。公开页面可以读取步骤，完整日志/API受访问限制；本地通过和Windows流水线通过分别记录。没有删除安全扫描、扩大模块预算或新增公告忽略来过门禁。

剩余：Windows干净安装、WebView2/DPI、原生对话框、系统凭据和完整打包Worker；真实SQL Server；公网Crossref及远端Embedding/PDF RAG；跨机器领域包及外部文件的恢复。回环HTTP/查询缓存夹具不算这些实际服务联调。对应说明和待执行清单见[科研入门](../development/research-first-run.md)、[工业来源](../development/industrial-sources.md)、[文献发现](../development/literature-discovery.md)和[Windows原生清单](../development/windows-research-acceptance.md)。
