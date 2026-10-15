# M4.4 文献发现与原文证据

在文献研究选择“公开发现 / 本地题录”。默认读取 SQLite 本地缓存，联网默认关闭。显式启用后才允许用户向所选来源提交检索：Crossref（api.crossref.org）或 Europe PMC（www.ebi.ac.uk）。只外发关键词、年份和分页游标，拒绝跳转，15秒超时，2MiB响应上限、20条分页，每个工作区/Provider每秒最多一次。取消、错误、重试和断线恢复复用背景任务；关闭联网会停止运行中的请求。服务选择进入原任务载荷和结果；分页沿用返回结果的冻结来源/条件，切换来源后不能误用上次游标。

`LiteratureMetadataProvider` 将请求参数/响应解析与调度、持久化和权限分开；两个实际适配器共用原 SQLite 表、修订仓库和全文流程。旧请求/任务没有 provider 字段时仍使用 Crossref；旧元数据 JSON、字段和 DOI 归一化规则不改变，不新增迁移或覆盖用户配置。生产 IPC 只能选择已注册来源，不能指定任意端点；回环 HTTP 地址仅存在于测试构建。公共题录服务地址属于适配器目录，个人大模型地址、模型名和密钥仍来自既有 ProviderProfile/SecretRef 配置。

DOI 大小写与常见 URL/doi: 前缀归一化，按工作区唯一身份去重，包括不同 Provider 返回同一 DOI 的情况；来源更新追加 SHA256 校验的修订，保留已有修订。无 DOI 的 Europe PMC 条目按来源与 MED/PPR 等外部 ID 去重。作者、期刊、年份与许可缺失即显示未记录。`isOpenAccess` 或元数据中的许可不是原文授权审核，条目始终 `link_only`。题录加入项目仍标“metadata only / 无原文”，不能从摘要推测实验数值。BibTeX 使用原生文件导出。年份统计注明当前分页的用户检索集合，不代表整个领域。

全文由用户提供有权使用的 PDF/Markdown/TXT/DOCX，需选择现有本地知识库、实际嵌入 Provider/维数和所需解析 Provider，并确认版权及可能向当前 RAG Provider 发送全文/片段。复用原 import_local_document 与索引任务，不绕过付费墙、不自动下载期刊 PDF。没有已配置 Provider 时不可伪称索引完成，原知识库配置不会被覆盖。

本地证据检索复用 FTS，显示真实 document/version/chunk 和页码/章节。归档时 Rust 校验工作区、片段哈希、逐字引用、数值与单位配对、字段单位兼容以及 DOI 与全文版本的关联。人工确认生成不可变审核证据，随后可以导出或创建新的数据实验室草稿，来源明确为 literature。未审核的旧 AI 草稿保留原流程，不自动转入新原生审核库或训练。新 JSON 表格读取支持对象数组、有限数值、缺失字段和嵌套溯源 JSON 单元格，64MiB/256列/十万预览行上限。

原 M4.4 历史验证为 Rust 7项、前端273项；原生 Markdown→真实文档版本/片段→审核→新草稿与重开通过，不能沿用该数字作为新适配器结论。当前接口扩展见 [Provider 验证记录](../validation/2026-10-15-literature-providers.md)。两种来源均执行真实本地 HTTP→原 Scheduler→修订缓存/项目来源→关闭联网后离线读取/重开测试，但未执行真实公网验收；Crossref先前公网403/连接受限结论不视为恢复。远端嵌入/解析 Provider、OA/PDF版权及 Windows 原生导入仍待真实环境验证。OpenAlex与其他来源留待后续适配。
