# Suna 科研项目元数据包 v1

`research-package-manifest.v1` 是项目结构与成果引用的独立分享协议。它不改变工作区备份格式 1/2。首期没有原始 CSV、模型对象、原文全文、领域包、系统配置或凭据；仅凭该包不能复现训练，也不能恢复完整项目。

## 文件与标识

包使用普通 ZIP，唯一允许的条目是 `manifest.json` 和 `records/0000.json` 至 `records/2046.json`。条目名为固定 ASCII 相对路径；操作系统中的 ZIP 文件名可以包含中文。只接受 Stored/Deflate，不接受 ZIP64、多磁盘、加密、目录、符号链接、脚本、二进制或未声明条目。导入检查不提取任何文件。

manifest 必须包含以下全部字段，拒绝其他字段：

```json
{
  "schema_version": "research-package-manifest.v1",
  "scope": "metadata_only",
  "source_project_id": "11111111-1111-4111-8111-111111111111",
  "entries": [{"name": "records/0000.json", "bytes": 62, "sha256": "8939c0187eb50b8e823788bb0e97c2d9987564335999b18590c0236dc0cd5a27"}],
  "external_dependencies": [],
  "edges": [],
  "app_version": "1.0.0",
  "assets_included": false,
  "provenance_mode": "references_only",
  "rights_status": "not_assessed"
}
```

上例是合成结构说明，其中记录的精确 UTF-8 内容为 `{"id":"11111111-1111-4111-8111-111111111111","kind":"project"}`，已按这些字节计算长度与摘要。生产导出必须从实际记录重新计算。`source_project_id`、记录 ID、任务 ID、引用 ID 均为 UUID；记录中恰好一个项目，且 ID 与 header 一致。

记录只允许 `id`、`kind`、`name`、`version`、`review_status`、`source_sha256`、`source_task_id`、`source_artifact_sha256`、`model_sha256`、`references`、`origin_kind`、`origin_schema`、`units`、`algorithm`、`artifact_version`。类型仅为项目、成果、模型、数据集、报告、证据。版本为正整数；未知信息缺省或 null，不能从名称或当前模型猜测历史元数据。

`units` 是至多 256 项 `{name, unit}`；未知单位为 null。注册模型的输入/输出单位取自冻结模型卡，数据字段单位取自冻结数据快照。`artifact_version` 保留实际模型对象协议版本，与注册版本号分开；算法只允许现有 Worker 的 9 种算法，不能把缺失算法改为默认算法。原始行、预测数组、模型 pickle、绝对路径、项目备注和配置不会进入记录。

## 哈希与血缘

`entries[].sha256` 保护包内精简 JSON 的字节；`source_artifact_sha256` 保留原科研成果的历史摘要；`model_sha256` / `source_sha256` 保留原模型 / 数据源摘要。这些摘要互不替代。精简记录不包含历史成果全文，因此不能宣称已仅用引用重新验证原资产内容。

`receiptSha256` 是按键排序的紧凑 UTF-8 manifest JSON 的 SHA256；`packageSha256` 是 ZIP 文件全部字节的 SHA256。前者标识协议内容，后者绑定用户实际预览并导出的文件。导出读项目、对象分页与已校验成果时使用同一 SQLite 读取快照，不改源库。再次导出必须重新读取，并核对预览时的 `packageSha256`；内容变化后必须重新预览。

外部依赖只允许 `resource_id`、`status`、`kind`、`expected_sha256`、`unit`，状态始终 `missing`。已知预期摘要从冻结来源获取，未重新定位并校验实际资产前不能变为 ready。同一资源的冻结快照若包含冲突预期摘要，导出拒绝选择某一版本覆盖另一版本。`rights_status=not_assessed` 不代表获得复制或公开原文 / 数据的权利。

边仅允许 `from_id`、`to_id`、`relation`。关系沿用已有成果关系：uses_dataset、uses_model、generated_by_run、based_on_evidence、recommends_experiment、measured_against、supersedes、included_in_report。记录、依赖与边去重；未知引用、自引用或有向环拒绝整个包。

## 预算与拒绝条件

ZIP 总字节及声明解压总量分别不超过 64 MiB，单文件不超过 8 MiB，至多 2048 条 ZIP 项、2047 条记录、2048 项外部依赖、10000 条边。膨胀比不超过 128。JSON 拒绝重复键、非有限数值、未知字段和超过 32 层的结构。条目集合、大小或 SHA 不一致时不产生导入结果。

## 桌面文件边界与临时检查

桌面保存 / 读取路径来自原生文件选择器，不接受 webview 提交的任意绝对路径。保存复用既有受权写盘、覆盖确认、回读校验。取消返回 null。读取复用实际路径授权和打开句柄核对，拒绝链接、非普通文件、超大文件以及读取期间发生变化的文件；结果不包含生产路径。

临时检查仅接受不可伪造的 `ValidatedPackage`，创建独立临时 SQLite，事务写入引用元数据与新 UUID 映射，再重复导入核对幂等。失败回滚；不接受用户主库连接，也不改项目、模型激活状态、配置或测量。结果固定 `status=metadata_only_missing_assets`、`writesToLiveWorkspace=false`、`assetsRestored=false`，不能当作跨机器完整恢复通过。

原始资产授权打包、许可证审核、受管文件复位、工作区冲突处理、正式主库导入和跨机器复现另行开发与验收。个人 Windows 原生文件选择、安装、Credential Manager 与真实 SQL Server 联调仍 pending-native / pending-network。
