# 科研归档 Rust 隔离验证

尚未接入主程序。本原型引用当前 Suna 真实项目/成果仓库，复用已有 zip 4.0、SHA256 和 SQLite 依赖；不改变备份格式 1/2，不读取凭据，不提取 ZIP 文件，不接受真实主库作为导入目标。

12 项 Linux 测试通过：元数据/中文标签/原哈希保持；严格 JSON 重复字段与深度限制；ZIP 规范路径、符号链接、条目白名单、经典中央目录预检查、体积和压缩膨胀预算；来源摘要、重复资源、引用完整性和 DAG 循环；原型临时 SQLite 幂等、关闭后重开、事务失败回滚与主库表/嵌套事务拒绝。

真实 Suna 公共仓库接口生成的 1100 个合成成果被去重为 1101 条引用记录与 4 条血缘边，原成果摘要保持，合成私密路径/密钥/原始行/项目备注均未导出，源连接 total_changes 不变。Rust/Python 互读各自输出的包，同一 ZIP 的清单摘要一致，两者均能在自己的临时库重复导入而不重复记录。验证记录见 [科研功能隔离验证记录](../../../docs/validation/2026-10-14-workflow-archive-preparation.md)，对照代码位于相邻 research-package-reference 目录。

当前仅支持 research-package-manifest.draft.v1 / metadata_only。内部文件名限制为 manifest.json 与 records/四位数字.json；中文可用于元数据与最终桌面外部文件名，内部资源文件名不接收 Unicode 路径别名。使用经典 ZIP，不接收多磁盘、ZIP64、加密或执行文件。

临时导入结果始终 metadata_only_missing_assets，assetsRestored=false、writesToLiveWorkspace=false；不能把它当作已恢复科研项目或可运行模型。正式接入仍需版本化协议文档、现有 Tauri 受权文件服务、实际项目页面交互、文件选择后的安全读取和原生 Windows 验收。受管文件/模型资产、许可确认、真实工作区导入与冲突修复属于后续阶段。

实际只运行了 Linux；Windows 原生弹窗/安装/跨机器恢复尚未验收。
