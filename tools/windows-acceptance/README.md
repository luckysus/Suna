# Windows 验收准备工具

以 Suna main `6e8a172a43c762f12ff70feee5872abce3e9b330` 和上传的 Windows 手册为基线。这些是验收准备脚本，未替代应用业务入口。本目录是验收工具源码，不是 Windows 应用安装包，也不含实际 Worker 可执行文件。

工具会生成 W01–W13 的 Markdown / JSON 记录，按已选择的发布目录核对现有 `release-manifest.json` 的 SHA、字节数与提交身份，并可核对现有 `worker-artifact-manifest.json`。不启动应用、安装器、Worker、外部服务，不读取凭据或用户数据库。不替代仓库现有 `build-release.ps1` 或 `release-check.ps1`。

以下命令均从仓库根目录运行。第一次仅生成待验收记录（Windows PowerShell，使用新的隔离输出目录；盘符可换成实际存在的盘）：

```powershell
pwsh -NoProfile -File .\tools\windows-acceptance\scripts\prepare-windows-acceptance.ps1 `
  -OutputDirectory 'D:\Suna-Acceptance\中文 验收记录' `
  -Commit '6e8a172a43c762f12ff70feee5872abce3e9b330' `
  -ActionsUrl 'https://github.com/luckysus/Suna/actions/runs/38517617942'
```

没有 PowerShell 7 时可使用 `powershell -NoProfile -File`，但 Windows PowerShell 5 的实际兼容性尚未验证，必须记录环境版本。

若已有来源可核验的候选发布目录和 Worker 目录，可加 `-CandidateDirectory 'D:\Suna-Candidate' -WorkerDirectory 'D:\Suna-Worker'`。目录必须是本地绝对路径；报告输出目录必须尚不存在，已有证据不会被覆盖。工具拒绝路径穿越、设备/网络路径、重解析点、ADS、非法文件名、重复条目、超预算文件以及 SHA 不匹配。不能从“哈希通过”推断安装、签名、协议或算法任务通过。

全部 W01–W13 初始为 `NOT_RUN / BLOCKED`；W08/W09 需授权真实服务。`stableReleaseAccepted` 始终为 false。`ciConclusion` 始终为 `not-verified-by-this-tool`，必须另外核对精确 SHA 的完整 Actions 结果。基线 6e8a172 对应的完整 Windows Actions 已核查成功；工具自身不查询 Actions，实际候选发布仍必须按精确 SHA 单独核查。请勿把这个工具生成的记录当作 CI 或原生验收通过。

独立合同测试：

```powershell
pwsh -NoProfile -NonInteractive -File .\tools\windows-acceptance\scripts\tests\acceptance-evidence.contract.ps1
```

测试在 Linux PowerShell 7.6.6 上执行，使用文本冒充二进制的**合成夹具**检查哈希/篡改/路径/不覆盖/未验收状态；夹具从未作为程序执行。这些结果不能代替 Windows 原生测试。附带 `fixtures/research-synthetic.csv` 是仓库已有演示数据的逐字节副本，不是真实实验测量。

明日仍需人工或本地授权 Codex 验证安装、WebView2、Worker 实际加载、中文路径导出、迁移、办公格式、Windows Credential Manager、离线边界、真实服务、长任务及备份恢复；保留上传手册的全部确认与隔离要求。

## 显微输入组件核对（源码 Python，不是安装包验收）

在仓库根目录使用已经按锁文件安装的计算 Worker Python，先运行上面的准备脚本创建记录目录，再执行：

```powershell
.\compute-worker\.venv\Scripts\python.exe .\tools\windows-acceptance\scripts\verify-scientific-input.py `
  --output 'D:\Suna-Acceptance\中文 验收记录\scientific-input.json' `
  --expected-commit '<当前待验收的完整40位提交SHA>'
```

工具实际调用本检出源码的 Pillow/NumPy/SciPy 分析组件，核对8个 CC0 合成 TIFF/PNG 原文件 SHA、两种字节序/16位量程与窗口像素、阈值占比、ROI/物理标尺、RGB与EXIF原始坐标，以及不支持输入的拒绝结果。源图不修改，报告只创建一次，不覆盖；需要已存在的本地绝对路径目录。记录模块内容摘要、实际 Git HEAD、检出是否有修改、工具摘要、平台和Python版本；不导出主机名、用户名、绝对源码路径或用户数据。旧安装的其他副本模块会被拒绝，不能拿它冒充当前检出的验证。

报告中的 `componentState=passed` 只表示源码组件/合成像素核对通过。`nativeAcceptance`、安装包Worker、原生弹窗、DPI、Credential Manager仍 `NOT_RUN`，Crossref/EuropePMC/模型API/SQL Server仍 pending-network；`ciConclusion=not-verified-by-this-tool`、`stableReleaseAccepted=false`。有未提交修改或未指定期望 SHA 时不能标记 cleanCommitVerified。此工具不运行 Tauri、安装器、服务、模型API或用户数据库，不能将其报告填写成个人 Windows 原生验收通过。

组件工具测试可用 `compute-worker/.venv/bin/python -m pytest compute-worker/tests/test_acceptance_input_tool.py -q`；Linux源组件检查不代表 Windows 已执行。
