# Cloud-2.2：科研模型卡和数据卡

基线 `f7bef8d51050d57614121895f9cfdad046387be0`。[Windows Run 38501647064](https://github.com/luckysus/Suna/actions/runs/38501647064) 全部 31 个步骤成功后开始本里程碑。作者/提交者均使用 `luckysus` 和已核对的 GitHub noreply；保留已有提交历史。

## 真实实现

新训练请求冻结输入总行数、有效行数、排除缺失目标的规则、分组/时间字段名称。读取完成任务的冻结请求和模型产物生成 `model-card.v1 / data-card.v1`，不使用当前可变数据集重新推断历史训练来源。

科研卡随现有 `steel_models.manifest_json` 在同一事务中保存；没有新建模型注册表或 SQLite 迁移，模型产物 JSON 字节和原模型 SHA 不变。科研卡摘要绑定登记 ID、版本和来源；查询校验精确模型字节 SHA、卡片摘要及从来源重建的白名单字段。两个版本同时存在，摘要不符时拒绝写入并回滚版本分配。

旧模型查询为 `legacy_derived / persisted=false`，不改写原记录；无来源、单位、规则或测试记录时显示未知。原有科研快照不加入新的空属性，保持历史内容及摘要和重复登记幂等性。活动版本删除/切换规则、专业约束、训练器和既有预测 API 保持原行为。

Tauri 只读命令 `get_steel_model_scientific_cards` 已登记到实际 invoke handler，模型中心“科研卡”通过 desktop bridge 读取。版本切换丢弃旧异步响应，核对模型 ID/SHA，错误提示不显示后端路径。Agent `get_model_status` 返回相同校验后的卡片，旧完整 manifest 改为现有安全元数据投影，剔除二进制和凭据字段。注册科研成果快照冻结同一份卡片，项目 manifest/报告读取不可变快照，不复制原始训练行。

记录实际算法、已保存超参数、特征/目标/单位、数据 SHA、划分和各子集数量、测试指标、已记录校准覆盖率及限制。独立测试样本数缺失时不显示“实际测试覆盖率”；校准可交换性始终未验证。质量为 unknown，审核为 unreviewed，不把保存成功当科研审核通过。

## 验证记录

环境：Linux，Rust 1.99.0、Node 24.19、锁定 Python 环境、PowerShell 7.6.6。所有材料数据均为合成夹具。

| 检查 | 实际结果 |
| --- | --- |
| 科研卡核心单测 | 19 项通过 |
| SQLite 登记/版本/重开/摘要/旧成果哈希回归 | 4 项通过 |
| 实际 Scheduler → stdio Worker → 注册 → SQLite → 项目快照/报告 → 重开 | 1 项通过，包含缺失目标过滤和真实校准/测试结果抽取 |
| 前端完整测试 | 55 文件，292 项通过 |
| 前端构建/类型检查 | 通过，保留原大包提示 |
| 运行时边界 | 156 文件通过 |
| 锁定 Worker 全套 | 185 通过、2 条件跳过、48 已知警告 |
| 脚本合同 | 既有合同及 Actions 4 项测试通过 |
| Rust 全套 | 95 个目标，1040 通过、0 失败、7 项原有忽略 |
| fmt / diff | 通过 |

Worker 集成同时检查 7 个来源行中 1 个目标缺失行被排除，以及另一份合成数据的 16 个独立测试样本、20 个校准样本。卡片覆盖率与实际评估产物逐项一致，但不宣称时间划分满足可交换性。原 CSV SHA 保持，旧版本卡片在另一数据版本训练后不变。Agent 回归直接调用现有工具入口并检查不返回模型 Blob、路径或密码字段。

首轮全量 Rust 发现 `steel_models.rs` 超出 400 行、Tauri 命令模块超出 150 行；按职责拆分测试和只读命令，完整保留原门禁和原测试。架构 28 项与登记回归 4 项已复验通过，没有调整阈值或忽略失败。

提交：`6e8a172a43c762f12ff70feee5872abce3e9b330`。对应 [Windows Run 38517617942](https://github.com/luckysus/Suna/actions/runs/38517617942)，[Job 115699850497](https://github.com/luckysus/Suna/actions/runs/38517617942/job/115699850497) 的全部 **31 个步骤成功**，包含依赖安全扫描、完整 Rust/Worker/前端、应用安全、钢材案例、四项性能基准与 Complete job。最终结论 success，精确 SHA 已核对。

原始证据：工作区 `model-cards-*.log` 与 `ci-6e8a172a43c762f12ff70feee5872abce3e9b330.json`。Windows runner 验证不能替代下面的个人电脑验收。

## 剩余边界

当前审核状态只记录“未审核”，尚无人工审核修改流程。可选库版本只保留来源实际记录的信息；不补造完整环境版本。ONNX 登记缺少完整训练元数据时，科研卡入口明确不可用。距离/联合适用域、Pareto 逐模型风险、正式研究包和科研模板执行仍是后续里程碑。

个人 Windows 安装、Credential Manager、原生弹窗/DPI、真实 SQL Server 与授权外部服务保持 `pending-native / pending-network`。Windows runner 的自动化验证不能替代个人电脑验收。
