# 归档 Python 协议对照

这组纯 Python 源码用于与 Rust 隔离原型互读合成元数据 ZIP，未加入 Python Worker 操作表或桌面入口，不导入真实主库，不提取或执行资产。

22 项 pytest 在云端锁定 Worker 环境通过。协议仍为 draft.v1，只声明元数据引用与缺失资产；Rust 入口执行更严格的字段类型和规范内部文件名检查。生产接入使用 Rust 校验，不能把这组对照代码当作新的受权文件服务。

从本目录运行仓库 compute-worker 的锁定 Python：`python -m pytest -q`。测试使用合成 ZIP 与临时 SQLite，没有真实测量、服务调用或凭据。
