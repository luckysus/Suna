<div align="center">

<img src="docs/assets/suna-banner.png" alt="Suna" width="600" />

<h1>Suna</h1>

**AI Agent Platform for Steel Materials Research**

钢铁材料研发智能体平台

*AI for Steel Research* · Making steel materials research smarter

A Windows-first, local-first desktop agent workbench.

[简体中文](README.md) · English

[GitHub](https://github.com/luckysus/Suna) · [Gitee](https://gitee.com/neusu/Suna)

[![Quality checks](https://github.com/luckysus/Suna/actions/workflows/quality.yml/badge.svg)](https://github.com/luckysus/Suna/actions/workflows/quality.yml)
![License](https://img.shields.io/badge/license-Apache--2.0-blue)
![Platform](https://img.shields.io/badge/platform-Windows%2010-0078D4)
![Desktop framework](https://img.shields.io/badge/Tauri-2-FFC131)
![Language](https://img.shields.io/badge/Rust-stable-b7410e)
![Status](https://img.shields.io/badge/status-in%20development-orange)

</div>

> **Project status**
>
> Suna is in the engineering phase before its public release. The `main` branch provides runnable development builds, not a published stable installer.

## Built for materials engineering

Suna is a desktop agent workbench for steel, materials, and industrial R&D. It brings local conversations, document parsing, retrieval with citations, production-data analysis, and controlled tool execution into one workspace.

It is not a generic chat shell. Use your own providers, keep conversations, knowledge bases, indexes, tasks, and memories on your own computer, and trace conclusions back to source documents, pages, tables, or data.

## Core capabilities

- **Local workspace**: open the client directly into the workbench; configure models and retrieval services in Settings when needed, without registering a Suna account.
- **Your choice of models**: OpenAI-compatible APIs and Ollama are supported; embedding, reranking, and parsing services are configured by the user.
- **Local knowledge base**: import PDF, Markdown, TXT, HTML, DOCX, CSV, and XLSX to build local retrieval and evidence citations.
- **Context and memory**: organize the agent's work around conversations, tasks, summaries, drafts, and long-term memory.
- **Steel and materials workflows**: support production-data import, deterministic calculations, model inference, prediction, and constrained optimization.
- **Controlled extensibility**: extend with MCP, Markdown Skills, and domain packages; high-risk actions such as file writes and shell execution require explicit confirmation.

## Provider setup

| Capability | Recommended start | Purpose |
| --- | --- | --- |
| LLM | OpenAI-compatible / Ollama | Conversations and agent reasoning |
| Embedding | SiliconFlow `BAAI/bge-m3` | Knowledge-base vectorization |
| Reranker | SiliconFlow `BAAI/bge-reranker-v2-m3` | Retrieval reranking |
| Document parsing | MinerU | PDF and complex-layout parsing |

Users choose SiliconFlow Free or Pro themselves. API keys are entered in Settings and stored in Windows Credential Manager; the local database stores provider configuration and secret references, never plaintext keys.

## Quick start

The development environment requires Windows 10, Node.js 20 / 22 / 24 (`.nvmrc` pins 24), Rust stable, Visual Studio Build Tools with `Desktop development with C++`, WebView2 Runtime, Git, and the Tauri 2 Windows prerequisites.

```powershell
git clone https://github.com/luckysus/Suna.git
Set-Location Suna

Set-Location frontend
npm install

Set-Location ../src-tauri
cargo tauri dev
```

`cargo tauri dev` starts the Vite dev server automatically through Tauri's `beforeDevCommand`. If the Tauri CLI is not installed:

```powershell
cargo install tauri-cli --version "^2"
```

You can also run `./start-desktop.bat` from the repository root: it checks Node, Rust, and the Tauri CLI, installs frontend dependencies, and starts the development environment.

### Optional: build the Python compute worker

Training, prediction, and constrained optimization run in a separate Python compute worker. It runs as a restricted subprocess (stdio JSON-RPC) and exposes no network port. To enable these capabilities in development, build it into the resources directory (requires Python 3.12+ and [uv](https://docs.astral.sh/uv/)):

```powershell
Set-Location compute-worker
powershell -File build.ps1 -OutputDirectory ..\src-tauri\resources\compute-worker
```

The script runs the worker tests inside the locked environment (`uv sync --frozen`), packages a single-file `suna-compute-worker.exe` with PyInstaller, and emits a SHA-256 checksum and component manifest.

## Repository layout

| Directory | Contents |
| --- | --- |
| `frontend/` | React 19 + TypeScript + Vite desktop frontend |
| `src-tauri/` | Rust desktop main process: agent runtime, tools and MCP, storage and migrations |
| `compute-worker/` | Python compute worker: training, prediction, constrained optimization, and design of experiments |
| `domain-packs/` | Domain packages shipped with the installer (steel prompts, skills, and evaluation definitions) |
| `fixtures/` | Data fixtures for tests and acceptance runs |
| `docs/` | Architecture, protocol, development, and validation documents; start at [docs/INDEX.md](docs/INDEX.md) |
| `scripts/` | Build, release, quality, and lifecycle scripts |
| `tools/windows-acceptance/` | Windows end-to-end acceptance tooling |
| `case-studies/` | Steel case-study materials and demo data |

## Development and testing

**Frontend** (`frontend/`)

```powershell
npm test                # Vitest unit and integration tests
npm run test:boundaries # Runtime boundary checks
npm run build           # Type check + production build
npm run test:e2e        # Playwright end-to-end tests
```

**Rust** (`src-tauri/`)

```powershell
cargo fmt --all -- --check
cargo test
```

**Compute worker** (`compute-worker/`)

```powershell
uv sync --frozen --extra test
.venv\Scripts\python -m pytest -q
```

**Full quality gate**: run `scripts/test.ps1` from the repository root. It executes the contract, frontend, and Rust stages in order (select one with `-Stage contracts|frontend|rust`). CI is defined in [.github/workflows/quality.yml](.github/workflows/quality.yml).

## Local data, privacy, and network

Suna stores `suna.sqlite3` in the operating system application-data directory rather than in the repository. Conversations, messages, summaries, memories, settings, knowledge metadata, and task state remain local by default.

Local-first does not mean automatically offline. When you enable a cloud LLM, SiliconFlow, MinerU, or MCP, related requests are sent to the service you configured or enabled. The current version does not include in-app automatic updates; users install newer Windows 10 builds manually.

## Extensions and documentation

- [Documentation index](docs/INDEX.md)
- [Contributing](CONTRIBUTING.md)
- [Event protocol](docs/AGENT_RUNTIME.md)
- [MCP and Skills extensions](docs/TOOLS_MCP_SKILLS.md)
- [Architecture and domain packages](docs/ARCHITECTURE.md)
- [Steel case study](case-studies/steel-release/README.md)
- [Security policy](SECURITY.md)

## Contributing

Issues, documentation, tests, and code contributions are welcome. Read the [contribution guide](CONTRIBUTING.md) first, and do not submit API keys, enterprise data, real user sessions, build artifacts, or generated directories.

## License

Suna is released under the Apache License 2.0. See [LICENSE](LICENSE) for the full text and [NOTICE](NOTICE) for attribution.
