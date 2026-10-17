<div align="center">

<img src="docs/assets/suna-banner.png" alt="Suna" width="600" />

<h1>Suna</h1>

**钢铁材料研发智能体平台**

AI Agent Platform for Steel Materials Research

*AI for Steel Research* · 让钢铁材料研发更智能

Windows 优先、本地优先的桌面智能体工作台。

简体中文 · [English](README.en.md)

[GitHub](https://github.com/luckysus/Suna) · [Gitee](https://gitee.com/neusu/Suna)

[![质量检查](https://github.com/luckysus/Suna/actions/workflows/quality.yml/badge.svg)](https://github.com/luckysus/Suna/actions/workflows/quality.yml)
![许可证](https://img.shields.io/badge/license-Apache--2.0-blue)
![平台](https://img.shields.io/badge/platform-Windows%2010-0078D4)
![桌面框架](https://img.shields.io/badge/Tauri-2-FFC131)
![语言](https://img.shields.io/badge/Rust-stable-b7410e)
![状态](https://img.shields.io/badge/status-开发中-orange)

</div>

> **项目状态**
>
> Suna 正处于公开发布前的工程开发阶段。`main` 分支提供可运行的开发构建，但不是已发布的稳定版安装包。

## 为材料工程而生

Suna 是面向钢铁、材料和工业研发场景的桌面智能体工作台。它将本地对话、文档解析、检索与引用、生产数据分析和受控工具执行整合到同一个工作区。

它不是通用聊天壳：你可以使用自己的模型和服务，把会话、知识库、索引、任务与记忆保留在自己的电脑上，并让每个结论尽可能回到原始文档、页码、表格或数据来源。

## 核心能力

- **本地工作区**：打开客户端即进入工作台；模型与检索服务可在设置页按需配置，不要求注册 Suna 账号。
- **用户自选模型**：支持 OpenAI 兼容接口与 Ollama；Embedding、重排和文档解析服务由用户自行配置。
- **本地知识库**：导入 PDF、Markdown、TXT、HTML、DOCX、CSV 与 XLSX，构建本地检索与证据引用。
- **上下文与记忆**：围绕会话、任务、摘要、草稿和长期记忆组织 Agent 的工作上下文。
- **钢铁与材料工作流**：覆盖生产数据导入、确定性计算、模型推理、预测和约束优化等场景。
- **可控扩展**：通过 MCP、Markdown Skills 和领域包扩展能力；写入文件、执行 Shell 等高风险操作需要明确确认。

## Provider 配置

| 能力 | 推荐起点 | 用途 |
| --- | --- | --- |
| LLM | OpenAI-compatible / Ollama | 对话与 Agent 推理 |
| Embedding | SiliconFlow `BAAI/bge-m3` | 知识库向量化 |
| Reranker | SiliconFlow `BAAI/bge-reranker-v2-m3` | 检索结果重排 |
| 文档解析 | MinerU | PDF 与复杂版面解析 |

SiliconFlow 的免费版和 Pro 版由用户自行选择。API Key 通过设置页保存到 Windows Credential Manager；本地数据库只保存 Provider 配置与凭据引用，不保存明文密钥。

## 快速开始

开发环境需要 Windows 10、Node.js 20 / 22 / 24（`.nvmrc` 固定为 24）、Rust stable、Visual Studio Build Tools（`Desktop development with C++`）、WebView2 Runtime、Git 与 Tauri 2 的 Windows 前置依赖。

```powershell
git clone https://github.com/luckysus/Suna.git
Set-Location Suna

Set-Location frontend
npm install

Set-Location ../src-tauri
cargo tauri dev
```

`cargo tauri dev` 会通过 Tauri 的 `beforeDevCommand` 自动启动 Vite 开发服务器。如果尚未安装 Tauri CLI：

```powershell
cargo install tauri-cli --version "^2"
```

也可以从仓库根目录运行 `./start-desktop.bat`：它会检查 Node、Rust 与 Tauri CLI，安装前端依赖并直接启动开发环境。

### 可选：构建 Python 计算 worker

训练、预测与约束优化由独立的 Python 计算 worker 提供，它以受限子进程运行（stdio JSON-RPC），不开放网络端口。开发时如需这些能力，先构建并放入资源目录（需要 Python 3.12+ 与 [uv](https://docs.astral.sh/uv/)）：

```powershell
Set-Location compute-worker
powershell -File build.ps1 -OutputDirectory ..\src-tauri\resources\compute-worker
```

脚本会在锁定环境（`uv sync --frozen`）中运行 worker 测试，用 PyInstaller 打包单文件 `suna-compute-worker.exe`，并生成 SHA-256 与组件清单。

## 仓库结构

| 目录 | 内容 |
| --- | --- |
| `frontend/` | React 19 + TypeScript + Vite 桌面前端 |
| `src-tauri/` | Rust 桌面主进程：Agent 运行时、工具与 MCP、存储与迁移 |
| `compute-worker/` | Python 计算 worker：训练、预测、约束优化与实验设计 |
| `domain-packs/` | 随安装包分发的领域包（钢铁场景提示词、技能与评估定义） |
| `fixtures/` | 测试与验收使用的数据夹具 |
| `docs/` | 架构、协议、开发与验证文档；总入口见 [docs/INDEX.md](docs/INDEX.md) |
| `scripts/` | 构建、发布、质量与生命周期脚本 |
| `tools/windows-acceptance/` | Windows 端到端验收工具 |
| `case-studies/` | 钢铁案例研究材料与演示数据 |

## 开发与测试

**前端**（`frontend/`）

```powershell
npm test                # Vitest 单元与集成测试
npm run test:boundaries # 运行时边界检查
npm run build           # 类型检查 + 生产构建
npm run test:e2e        # Playwright 端到端测试
```

**Rust**（`src-tauri/`）

```powershell
cargo fmt --all -- --check
cargo test
```

**计算 worker**（`compute-worker/`）

```powershell
uv sync --frozen --extra test
.venv\Scripts\python -m pytest -q
```

**完整质量门**：从仓库根目录运行 `scripts/test.ps1`，依次执行脚本契约、前端与 Rust 三个阶段（可用 `-Stage contracts|frontend|rust` 单选）。CI 定义见 [.github/workflows/quality.yml](.github/workflows/quality.yml)。

## 本地数据、隐私与网络

Suna 将 `suna.sqlite3` 存放在操作系统应用数据目录，而不是仓库目录。会话、消息、摘要、记忆、设置、知识库元数据和任务状态默认保留在本机。

“本地优先”不等于自动离线：当你启用云端 LLM、SiliconFlow、MinerU 或 MCP 时，相应请求会发送到你配置或启用的服务。当前版本不包含应用内自动更新；应用升级由用户手动安装新的 Windows 10 构建包。

## 扩展与文档

- [文档索引](docs/INDEX.md)
- [贡献指南](CONTRIBUTING.md)
- [事件协议](docs/AGENT_RUNTIME.md)
- [MCP 与 Skills 扩展](docs/TOOLS_MCP_SKILLS.md)
- [架构与领域包](docs/ARCHITECTURE.md)
- [钢铁案例研究](case-studies/steel-release/README.md)
- [安全策略](SECURITY.md)

## 贡献

欢迎提交 Issue、文档、测试与代码。请先阅读 [贡献指南](CONTRIBUTING.md)，并不要提交 API Key、企业数据、真实用户会话、构建产物或生成目录。

## 许可证

Suna 使用 Apache License 2.0。完整条款见 [LICENSE](LICENSE)，相关声明见 [NOTICE](NOTICE)。
