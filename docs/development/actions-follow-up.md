# 推送后的 Actions 核查与修复

2026-10-13 实际读取公开 Actions 页面（API 返回 Forbidden，但公开页面可访问）：

| 提交 | 运行 | 结果 |
|---|---|---|
| 8dc3f73 M5.4 | [192](https://github.com/luckysus/Suna/actions/runs/38352599415) | 被后续推送取消 |
| b1ed50c M5.5 | [193](https://github.com/luckysus/Suna/actions/runs/38353041052) | Rust advisory scan 失败 |
| 70b7efb M4.3 | [194](https://github.com/luckysus/Suna/actions/runs/38355197587) | Rust advisory scan 失败 |

本地安装与流水线相同的 cargo-audit 0.22.2，对同一锁文件复现了五条 quinn-proto 0.11.16 公告（RUSTSEC-2026-0338、RUSTSEC-2026-0339、RUSTSEC-2026-0340、RUSTSEC-2026-0341、RUSTSEC-2026-0342）。升级至修复版本0.11.18；保持已有rsa例外，不添加新的忽略项。依赖扫描与后续Rust格式检查是不同门禁，后者也做了仓库格式修正。后续推送的云端审计结果和Windows完整流水线结论分别记录，不能用本地通过代替远端完成。

每次推送后读取提交SHA对应运行，继续开发一段时间后再查直到结束；失败读取具体步骤，取消则说明被哪个后续推送替代。优先使用gh CLI，API不可用时可运行 `python scripts/check-actions.py --sha <SHA>`，脚本只读取公开页面，没有凭据处理或远程重跑功能。退出0表示该SHA已读取运行均成功，1表示失败，2表示尚未完成/取消/无法核查。日志无法访问时明确说明，并尽量复现失败的同版本命令。

修复前后实际执行 `cargo audit --ignore RUSTSEC-2023-0071 --json`：5条漏洞→0条。`cargo fmt --all -- --check`通过。完整 `cargo test --locked --lib -- --test-threads=1`：364通过、2真实PostgreSQL/文献环境测试跳过。新增图像处理器导致的旧注册数量断言一并更新，并校验处理器存在及去重。尚未用本地结果宣称Windows流水线成功。

第二次实际核查：[195](https://github.com/luckysus/Suna/actions/runs/38358898275) 的 Rust advisory scan 已成功，随后 npm audit 报 image-size 的 JXL/HEIF/ICNS 无限循环高危公告（GHSA-5p2g-fcmc-qvqq、GHSA-w3rx-r6r6-pgpr）。上游已发布2.0.4，但PptxGenJS仍约束1.x，因此仅覆盖该依赖到修复版2.0.4，保留报告导出。npm audit复查0漏洞，真实DOCX/PPTX包和PNG图表嵌入回归通过。公开页面存在缓存且进行中标签含空格，状态核查脚本补充分钟缓存刷新和进行中识别。
