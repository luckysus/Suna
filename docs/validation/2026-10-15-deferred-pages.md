# 专业页面按需加载与生产构建验证

基线为 `65f8f520b4f9a59a51afa72c9730aa70b4a6fc43`。审计发现应用根同步导入全部专业页面，启动对话中心时也加载报告、图表及其他专业模块。本次复用原路由、上下文、页面和桌面桥，只改变本地模块的加载时机；不增加服务、依赖、权限、数据库迁移或 API 配置。

13 个专业页面采用字面量动态导入。加载成功后缓存模块并共享进行中的请求；切换页面会忽略迟到的加载回复。加载失败保留应用外壳，显示明确的重新加载入口及未保存输入会关闭的提示。首次真实浏览器测试证实，浏览器会缓存失败的 ESM 请求，仅重试同一 import 无法可靠恢复；最终方案采用用户主动重新加载当前文档，生产浏览器故障注入已经通过。

生产安全扫描原来只跟踪静态依赖，改为同时遍历字面量动态导入及其后代。保留全部原有禁止项和文件行数预算，增加嵌套延迟模块中的非法 IPC、循环依赖和重复导入回归；扫描范围从 178 个生产文件变为 179 个。没有利用拆包把专业页面移出安全检查。

## 实际验证

Linux 云端、合成数据与 IPC 夹具，浏览器为系统 Chromium 154。浏览器测试只访问回环地址，其他请求阻断；未使用用户的浏览器资料或真实服务。

| 检查 | 结果 |
| --- | --- |
| TypeScript / Vite 生产构建 | 通过，最终构建 10.07 秒 |
| 实际生产 dist 浏览器 | 2 passed、0 failed、0 skipped、0 flaky；包含导航后才请求专业模块、首个优化模块请求失败后主动重载恢复 |
| 既有浏览器功能流程 | 9 passed：启动、导航、科研项目浅深主题及 CSS 缩放、文献、显微工作台 |
| 延迟加载和数据激活专项 | 10 passed；异步夹具显式完成 React 更新，核对勾选/按钮状态、原生请求 ID 与 SHA，并保留失败、源版本改变、迟到回复等负例 |
| 前端全量 | 最终异步交互调整后复验：69 文件、357 passed、0 failed，70.65 秒 |
| 安全图遍历 / 运行时边界 | 2 passed，179 个生产文件 |
| Rust 架构合同 | 28 passed；本次没有 Rust 生产改动，最近全量 Rust 为 1157 passed、0 failed、7 项原有 ignored |

全量回归曾暴露数据激活测试同步事件与异步原生夹具衔接不稳定：初次全量为 356 passed、1 failed，隔离重跑通过；单独增加初始挂载等待仍出现一次未调用激活的失败。最终测试等待原生列表和规范化后的 React 更新、明确核对用户确认后的状态，并核对实际 IPC。没有据此修改生产版本校验逻辑、删除负例或扩大超时，初轮失败日志保留。

同一工作区的基线与最终入口文件：

| 产物 | 解压后的 JS 字节 | SHA256 |
| --- | ---: | --- |
| 基线入口 | 2790669 | `c908b27480271bf9573f0b3d8d45a94d8682eb1dbfc0a399ec13b7dcfd8cbb30` |
| 按需加载入口 | 956988 | `afaa8ba45771cace0e267c9fcd73239057895e3cdd030c26a062bf19b06be337` |

入口减少 65.71%。生产浏览器也实际观测到初始只请求该入口，导航后再请求预测模块；这是代码加载量测量，不是启动耗时、网络流量、内存或原生 Windows 性能结论。入口及报告导出仍有大于 500 kB 的 bundle 提示，未隐藏提示或提高阈值。

复验命令：

```bash
npm --prefix frontend test
npm --prefix frontend run test:boundaries
npm --prefix frontend run build
# 已安装 Playwright Chromium 时不需要 SUNA_PLAYWRIGHT_EXECUTABLE。
SUNA_PLAYWRIGHT_EXECUTABLE=/usr/bin/chromium npm --prefix frontend run test:e2e:production
```

原始输出保存在云端工作区 `/workspace/suna-independent-development/deferred-pages-*`，没有把本地绝对路径或私人资料加入产品导出。生产浏览器使用单独配置，不更新既有 Windows 视觉金标准。

## CI 和原生边界

本次推送前已确认受管包提交 `3aeaacb89bc71862944b72aa705fe259d5b5a462` 的 [Run 38715025660](https://github.com/luckysus/Suna/actions/runs/38715025660) 全部 31 个步骤成功，包括 Windows Rust、安全、Worker、前端、四项性能基准和 Complete job。该结果不等于本次新提交已经通过 CI；新 SHA 推送后须独立检查。

Windows 安装包的本地资源协议、离线跨模块导航、WebView2 模块加载失败、原生缩放及个人电脑验收均 `NOT_RUN / pending-native`。用户自定义启动页、上下文和配置沿用现有实现，不覆盖任何保存值。
