# 文献元数据 Provider 接口与 Europe PMC

本次在本地 `4618503fe482c157b25fa98f6d16361767cd05d2` 图像输入里程碑之后独立开发。先确认现有公开发现只有 Crossref：Search 没有 provider 字段，HTTP/参数/解析与任务在一个模块，仓库和限流键也固定为 crossref；现有 SQLite 修订、DOI身份、项目来源、RAG全文/证据与版权确认均已可用。

## 实现与兼容

- 提取公共题录类型、原 DOI 归一化和原 Crossref 解析，保留原 Rust 路径的 re-export、元数据字段/JSON。新增 `LiteratureMetadataProvider` 请求/解析接口和类型化来源目录，接入实际 Europe PMC 参数与 JSON 响应。没有第二套文献库、HTTP客户端、Scheduler 或模型配置。
- 原 Tauri `search_public_literature` / `literature_metadata_search` 接受可选来源，旧载荷默认 Crossref。实际任务冻结来源、词语、年份和游标，结果记录来源，仍复用固定官方地址、拒绝跳转、连接/请求超时、2MiB、20条、取消/关闭联网门禁及原重试策略。
- 原仓库保存两种来源，跨来源相同 DOI 共用身份、追加独立修订和 SHA；无 DOI 使用 Provider/外部 ID 命名空间。每工作区每 Provider 独立 1秒限流，默认联网关闭不变；不新增数据库迁移或改动现有设置。
- Europe PMC 规范化 MED/PPR 等来源身份、DOI、作者、期刊、年份、摘要及元数据许可；缺失保持 unknown，非法身份/年份/DOI、超量页面/作者/游标拒绝。即使上游标注开放获取，也保持 `fulltextState=link_only`，不下载全文、推断数值或跳过原文/版权/人工证据审核。
- 原页面增加真实来源选择并通过桌面桥提交；下一页使用返回任务冻结的来源/条件。切换服务清除旧游标，并阻止迟到的另一来源结果提供可点击的错误游标。显示实际记录来源和元数据修订，联网和RAG的个人模型配置保持原流程。

## 真实测试

针对性 Rust：16 passed，保留2项原有真实服务 ignored。其中新增4项接口/解析测试、1项仓库跨来源身份/修订/限流回归、1项 Europe PMC 真实回环 HTTP/Scheduler/缓存/项目/重开测试。Crossref 原 HTTP 测试保留，并共同检查中文和 `+?` 检索词编码、请求预算/游标、任务来源与输出、默认 link-only、离线重开及 SHA。

实现完成后的完整回归：

| 验证 | 实际结果 |
| --- | --- |
| Rust全部97个Cargo目标（含文档测试），逐目标锁定执行 | 1124 passed、0 failed、7项原有 ignored，317秒 |
| 文献针对性Rust | 16 passed、2项原有真实服务 ignored；其中两个实际回环HTTP调度闭环均通过 |
| 前端完整Vitest | 61个文件、320 passed；来源/分页/迟到结果5项（保留原3项） |
| TypeScript、生产构建、运行边界 | 通过，166个运行文件，保留原有大块构建告警 |
| Rust格式、空白、Actions状态合同 | 通过，合同7项；依赖锁文件、CI与权限未改 |

前端测试使用真实 `useComputeTask` 和明确的桌面桥夹具，不当作原生或真实服务调用。完整 Rust 枚举全部目标，未减少用例或新增忽略。

当前远端仍是归档提交 `c6eabf8a7726ef881ff1852e40122fdd36e61b19`；其 [Run 38654574937](https://github.com/luckysus/Suna/actions/runs/38654574937) 实际在 Run deterministic Rust suite 失败，后续业务检查未执行。公开页面只有退出码1，gh返回Forbidden、日志页面404，具体失败测试/编译错误尚缺证据，不能猜测。本地两个新里程碑尚未推送，不能将本地通过或更早提交的绿色CI称为它们的Windows验证。

证据位于云端 `/workspace/suna-independent-development/literature-provider-*`；仅使用合成题录及本机回环 HTTP，没有公开服务访问、用户文献、账号或凭据。Python Worker 未改；其本轮图像扩展后的锁定测试为217 passed/2项原有打包 skipped，最终完整 CI 仍需重跑。

## 剩余验收

Crossref/Europe PMC 真实公网、上游实际分页/年份与节流、真实许可和全文权利、授权嵌入/解析 Provider 都为 `pending-network`；没有宣称服务可用或正文证据已经审核。个人 Windows 安装、Credential Manager、原生弹窗、DPI、SQL Server 仍依照 [Windows 手册](../development/windows-research-acceptance.md) 待验收。OpenAlex等后续来源未提供静态占位选项，也未硬编码个人模型 API。
