# Cloud-2.1：训练与预测适用域接入

## 基线与已通过的 Windows 门禁

基线 `bdee56e3d6035dd7921a829a84966dfd823dd3dc` 的 [Run 38491043668](https://github.com/luckysus/Suna/actions/runs/38491043668) 最终成功；[windows-quality job](https://github.com/luckysus/Suna/actions/runs/38491043668/job/115616024911) 的 31 个步骤全部 success，包含 Rust、应用安全、科研案例、全部性能基准、空白及清理。通过公开 Actions 页面逐项核对，未把 Rust 单项通过当作全绿。

本里程碑在该门禁通过后开始生产接入。提交使用 `luckysus` 与 GitHub noreply；用户明确选择保留原提交和 CI，没有历史重写或强推。

## 已有实现与增量改动

复用实际训练/评估、Scheduler、stdio Worker、任务 checkpoint、预测语义表、模型产物、科研成果与报告；没有新建任务系统、训练器、模型注册表或 SQLite 迁移。

- 训练提交冻结名称、声明单位、目标字段；未知单位保持 null。线性、sklearn 与 Transformer 新训练产物保存版本化 `applicability_support`，读取现有训练集范围及均值；模型整体内容 ID 和支持域摘要校验绑定元数据。验证、校准、独立测试数据不参与拟合支持范围。
- 原预测接口增加可选 `inputSchema`。按模型名称重排输入，复用实际材料单位引擎处理温度/质量分数等换算，拒绝重复/缺失字段、不兼容单位、旧模型未知单位的显式换算及溢出。旧调用仍按模型特征序输入。
- Worker 返回逐行 `within_domain / warning / outside_domain / unknown`、原因、训练阈值来源、版本及支持域摘要。旧模型无支持字段为 unknown；缺失值可使用已记录训练均值并标 warning。单批 10000 行、128 特征、2MiB 详细证据预算，超预算明确要求减小批次，不截断成假完整结果。
- Rust 校验风险行身份、模型/摘要归属、科学语义并持久化到实际 checkpoint。科研成果只保留风险证据，剔除输入行、路径和非声明字段；报告读取相同不可变成果。`input_rows` 仅用于已授权的本地预测结果/用户选择的 CSV，不复制进默认研究 manifest。
- 单点页面与批量表显示四种状态及中文原因；批量输入可显式声明 schema/单位，表格和 CSV 使用后端转换后的模型序与训练单位，风险及摘要随导出保存。空白单点输入不再被 `Number("")` 静默解释为零。
- 原范围警告、校准区间和预测 API 保持兼容。范围筛查不提供概率、联合支持或物理可行性保证，也不默认过滤优化点。

## 实测发现与修复

真实 Scheduler → Worker → SQLite 测试发现默认 serde_json 浮点解析把训练缩放量 `11.180339887498949` 变成 `11.180339887498947`，破坏模型内容 ID。启用现有 serde_json 的 `float_roundtrip` feature，保持精确 binary64 往返；没有升级依赖或改 Cargo.lock，没有删除模型完整性检查，增加精确位比较和实际跨进程持久化回归。

完整旧 Random Forest 任务测试还暴露 `missing_dataset_sha256` 原因码被仅字母格式误拒绝，已改为明确白名单并增加缺失信息回归；旧记录继续 unknown。两处旧失败诊断改为只输出错误码，避免整份 TaskRecord 输出模型 Blob/原始输入。

公开 Actions 状态读取器增加 GitHub 当前 `completed successfully` 标签识别，4 项只读 HTML 夹具测试纳入现有 contracts 阶段；不更改 workflow 触发、权限、基准阈值或安全扫描。

## 实际自动化验证（Linux，合成数据）

| 检查 | 结果 |
| --- | --- |
| 完整 Rust `cargo test --locked --no-fail-fast --jobs 1 -- --test-threads=1` | 93 个目标，1015 通过、0 失败、7 原有忽略 |
| Python 全套 `compute-worker/.venv/bin/python -m pytest -q compute-worker/tests` | 185 通过、0 失败、2 条件跳过、48 已知警告 |
| 前端 `npm --prefix frontend test` | 54 文件、287 通过 |
| 前端生产构建 | 通过；原大包提示保留，未调整阈值 |
| 前端运行时边界 | 155 文件通过 |
| `scripts/test.ps1 -Stage contracts`，Linux PowerShell 7.6.6 | 原 7 项合同和新增 Actions 4 单测全部通过 |
| fmt / diff | 通过 |

新的真实集成测试使用临时 SQLite、实际 Scheduler 和 Worker：训练单位/范围、不同字段序、K 与 fraction 换算、逐行风险、科研 manifest 投影、原 CSV SHA 不变、数据库关闭后重开读取。前端两项交互测试实际触发 bridge mock 提交和原生导出调用，检查 CSV 使用转换后的数值；这属于 jsdom/IPC mock，不是 Windows 保存弹窗验收。

原始日志保存在当前工作区 `/workspace/suna-independent-development/cloud2-*.log`。首轮完整 Rust 的一个原因码失败已定位并修复，最终全套复验为零失败；没有忽略测试。

后续复核：提交 `f7bef8d51050d57614121895f9cfdad046387be0` 的 [Run 38501647064](https://github.com/luckysus/Suna/actions/runs/38501647064) 最终成功；[Windows job](https://github.com/luckysus/Suna/actions/runs/38501647064/job/115649736513) 的 31 个步骤全部 success，含四项性能基准和 Complete job。公开页面证据保存在 `ci-f7bef8d51050d57614121895f9cfdad046387be0.json`，没有使用 Rust 单项状态代替最终结论。

## 本里程碑范围与待办

这是训练和单点/批量预测的适用域最小接入，不宣称整个 Cloud-2 完成。距离型/多变量支持、模型比较与 Agent 专门展示、每模型 Pareto 风险和主动筛选仍需后续接入；模型卡/数据卡、项目研究包、科研模板尚未在此提交实现。专业硬约束与校准区间保留原逻辑，不能用范围内状态替代真实科研验证。

用户个人 Windows 安装、Credential Manager、原生弹窗、DPI、SQL Server 及授权外部服务均保持 `pending-native / pending-network`。
