# 工作流恢复测试同步修复与验证

## 原始 Windows 证据

用户提供 [Run 38654574937](https://github.com/luckysus/Suna/actions/runs/38654574937) 的真实日志。对应提交 `c6eabf8a7726ef881ff1852e40122fdd36e61b19`：编译通过（rustls 0.23.46），库测试476 passed、1 failed、3项原有 ignored。唯一失败为 `research_workflow::runtime_recovery_tests::restart_reuses_registered_versions_after_registration_checkpoint_gap`，在 `runtime_support.rs:69` 的90秒等待处报 `workflow deadline`。原日志未记录超时当刻的任务状态；依赖扫描和前端等此前步骤通过，Rust失败后的安全、案例及基准等步骤未执行，不能称完整CI通过。

## 可复现的同步缺陷与修复

原恢复测试必须在轮询中看到 `WaitingExternal && progress == 40`，但真实 Scheduler 在同一次 tick 内先收取处理结果，再领取已经到期的任务。该状态不保证能被轮询观察到；已经继续或完成的任务也会被旧等待条件误判为超时。

新增测试专用 `RegistrationBoundary` 包装真实处理器，不替代训练、注册、维护或恢复逻辑。只对合成测试库把实际首次注册检查点的下次调度时间置为过去，确定性复现同tick重新领取；新增回归验证真实双模型训练/优化仍完成，但轮询从未看到该中间状态。该回归实际本地通过，未依赖随机机器负载或增加等待时间。

原恢复测试改为在真实模型和检查点落库之后使用同步点停住处理器，使用原 Scheduler 关闭/回收路径，再读取持久化的 `Interrupted` 检查点。只清空检查点中的已注册模型引用来模拟原目标故障，通过原 `recover()` 恢复。保留模型2个、任务4个、实验计划1个的原断言，并新增：

- 重用完全相同的模型ID、版本、来源任务、SHA、模型/manifest/artifact原始内容与未激活状态。
- 科研卡、冻结成果ID/哈希/完整快照在数据库重开后保持不变。
- 训练子任务身份不变、计划仍为proposed、接受仍需确认、不生成任何实验测量。

测试等待仍为90秒，业务预算仍为原300秒；未修改生产 Scheduler、工作流检查点时序、权限、取消/截止时间守卫、依赖或CI。等待工具仅新增脱敏状态/进度/尝试次数/错误代码，并在意外结束时立即报告，避免再只得到无状态的超时。

## 验证记录

| 验证 | 实际结果 |
| --- | --- |
| 确定性过期检查点/同tick重新领取回归 | 1 passed，4.28秒 |
| 包含原失败项的全部工作流库测试 | 35 passed、0 failed，25.81秒 |
| 全部97个Cargo目标（库、二进制、集成与文档） | 1125 passed、0 failed、7项原有 ignored，238秒 |
| Rust格式、空白检查 | 通过 |
| Actions新旧页面及完整作业结论合同 | 7 passed |

完整Rust按Cargo metadata枚举全部目标，逐目标锁定执行；未减少用例、增加ignore或延长超时。临时证据为云端 `workflow-race-reproduction.log`、`workflow-recovery-focused.log`、`workflow-recovery-main-rust-all-targets.log` 与 `workflow-recovery-main-rust-summary.json`。前端和Worker本修复未改；本批前两个里程碑实际前端320 passed/构建/166项边界、第三里程碑完整Worker224 passed/2项原有打包 skipped的记录保留。

以上为提交前的本地结果。新提交尚待推送后按精确SHA核对完整Windows CI，不预填成功；依赖扫描、Worker、前端、Rust、安全、钢铁案例、四项基准和收尾必须全部通过。

本批次另含三个已经分别本地提交和测试的里程碑：`4618503` TIFF/16位显微输入，`4a02c02` Europe PMC题录接口与持久化，`c252a65` 显微输入组件验收工具。它们的具体范围和本地证据分别见同目录的显微输入、文献Provider和验收组件记录。

个人Windows安装、Credential Manager、原生弹窗/DPI、真实SQL Server与授权Provider仍为NOT_RUN / pending-native / pending-network。完整CI即使通过也不替代这些验收。
