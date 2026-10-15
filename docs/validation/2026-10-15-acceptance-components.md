# 显微输入 Windows 验收辅助工具（未完成原生验收）

复用已有 `tools/windows-acceptance` 目录和原 W01–W13 准备流程，增加可显式执行的 `verify-scientific-input.py`。它调用当前检出源码的真实 Python 图像组件，对已有 CC0 TIFF/PNG 金标准进行源 SHA、大小端、16位量程/窗口、阈值、ROI/标尺、RGB/EXIF原始坐标及不支持格式检查；不启动 Tauri、安装器、打包Worker、个人服务或用户数据库。

工具拒绝旧安装位置的模块冒充检出源码，记录两个实际组件 SHA、工具 SHA、Git HEAD/dirty 状态和期望 SHA 匹配情况；只有明确期望 SHA 且检出干净时才允许 `cleanCommitVerified=true`。不导出用户名、主机名、绝对源码路径、凭据或原始用户数据。输出要求本地绝对路径和已有常规父目录，拒绝穿越、设备/ADS/非法名称，只以新文件写入，不覆盖现有证据。

## 实际云端验证

- 新增7项测试通过：真实合成像素核对与权限/验收边界、SHA篡改、中文输出/不覆盖、危险或缺失路径、旧模块位置、缺Git/提交不符和实际命令流程。
- 完整 Python Worker：224 passed、2项原有打包验收 skipped，17.12秒；没有新增 skip 或联网测试。
- 实际 CLI 已执行并成功生成独立 JSON：期望 HEAD 为 `4a02c02c14a9c9efb83cc9068d50c01afbe401fd`，当时含本工具未提交文件，因此报告 `checkoutDirty=true`、`cleanCommitVerified=false`。`componentState=passed`，`nativeAcceptance=NOT_RUN`、真实服务 pending-network、`ciConclusion=not-verified-by-this-tool`；没有把源码组件通过变为安装包验收通过。
- Rust、React 和依赖未改；继承同一代码状态的上一里程碑实际本地结果1124 Rust通过/0失败/7原有忽略、320前端通过/构建/边界通过，不将其当作本次Windows验收。

临时证据在 `/workspace/suna-independent-development/acceptance-components-worker-all.log` 和 `acceptance-components-evidence-20261015.json`；不提交生成报告或临时数据库。

当前已推送的 `c6eabf8` 对应 [Run 38654574937](https://github.com/luckysus/Suna/actions/runs/38654574937) 仍是 Rust步骤失败、具体错误日志读取受限，尚未修复或宣称CI通过。本工具及前两项新功能先本地独立提交；需要实际失败测试/编译错误证据后优先修复并检查新的完整Windows CI。

个人 Windows 原生安装、Credential Manager、弹窗、DPI、图像导入显示、备份迁移、SQL Server和授权真实服务仍全部待验收。操作和状态定义见 [工具说明](../../tools/windows-acceptance/README.md) 与 [Windows手册](../development/windows-research-acceptance.md)。
