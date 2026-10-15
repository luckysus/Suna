# 题录分页保存与并发任务写入

审计 `e817e9a` 的文献持久化：HTTP 完成后使用延迟 SAVEPOINT，题录身份查询先形成读快照，再写身份/修订。另一连接写入 Scheduler 进度时，SQLite WAL 的读快照可能无法升级为写事务；busy timeout 也不能解决这种锁升级。通用错误归一化会把带空格的 SQLite 错误变成 `handler_error`。

本轮全量本地 Rust 首次出现一项原有 Crossref 回环测试失败，进度10、`handler_error`；该原始失败未保存底层 SQLite 错误码，无法追认其完全同因。未修改生产逻辑的后续三轮完整 lib 均通过（每轮491 passed / 3项原有ignored）。保留这些真实记录，不把一次偶发失败写成已确认的根因。

另新增**可控竞争回归**，对原分页保存逻辑实际得到 `database is locked`：测试线程先持有写事务并更新合成后台任务；仅当保存连接真正申请写锁、调用 busy handler 时才允许竞争写入者提交。原延迟事务先读后写，不能等待，回归实际失败。没有靠缩短超时、sleep碰运气或放宽断言制造通过。

修复将同一页保存改为现有 rusqlite IMMEDIATE 事务，在首次读取前取得写锁；联网、解析和 await 均在事务外，仍最多20条/2MiB。事务内重新核对联网开关。`repo::store` 的原 SAVEPOINT、DOI身份去重与不可变修订复用不变；整页遇到非法记录时全部回滚。关闭联网不写入新题录；既有离线缓存不受影响。

已知 SQLite busy/locked 错误转为有界、可重试的 `literature_storage_busy`，使用原 Scheduler 的重试、速率限制和取消机制，不增加私有重试循环，不输出原始存储错误或路径。业务校验错误仍为永久失败。临时定位输出已移除，不提交诊断打印。

针对性真实验证：5 passed、0 failed、1项原有公网 ignored，包括竞争写入后保存、半页失败回滚、关闭联网、Crossref/Europe PMC 回环 HTTP → Scheduler → SQLite → 离线重开。本批题录锁与数据激活修复的最终工作树按Cargo metadata重新枚举全部97个目标，1132 passed、0 failed、7项原有ignored，311秒；初次失败记录保留，未新增忽略。格式和空白检查通过。基线e817e9a对应Windows Run 38682257968提交时仍在运行（已通过Rust/安全/钢铁案例）；新修复推送后须按最终精确SHA核对完整Windows CI，不能把基线结果当作本修复已过。公网服务与个人 Windows 仍为 NOT_RUN / pending-network / pending-native，不能用回环服务代替真实服务验收。
