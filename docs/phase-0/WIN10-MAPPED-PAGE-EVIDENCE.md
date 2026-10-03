# Win10 首键：映射页准备的隔离试验

日期：2026-10-03。接续 [native 组件诊断](WIN10-NATIVE-LATENCY-EVIDENCE.md)。试验在 Windows 10 Pro 22H2（19045.6466）主机进行，本轮未操作 VM。0.0.11.0 包、生产 `rime.dll`、TIP 以及 50 ms key deadline 均未变。

## 方法

在已隔离的 librime 源码副本中增加 `MO_DIAG_PREFAULT_MAPPED=1` 开关：只在打开只读文件映射后按系统页大小读取每页一个字节。它不发送 key、不修改候选/用户数据，也不输出文件名或内容。关闭开关时执行同一份诊断 DLL。代码差异、三个版本的运行库 SHA、固定数据源与脚本均由下方证据绑定。新的 [Actor 探针](../../crates/mo-rime/examples/actor_latency_probe.rs) 使用带 marker 的独立编译数据副本，测量 engine 准备、Broker 实际 `rime_ice` 方案及三项会话选项的创建/首键；它只输出耗时和候选数量。

## 结果

直接 API 在三个新用户数据副本中按「关、开、关」运行，每组 20 个 session × 2 个合成键，均断言 handled、无 commit、清空/销毁成功：

| 条件 | 首次 `process_key` | 映射读取 | 读取耗时合计 |
| --- | ---: | ---: | ---: |
| 关 A | 401,397 µs | 0 | 0 |
| 开 | 1,472 µs | 27 次，89,740,108 字节 | 1,033,961 µs |
| 关 B | 155,144 µs | 0 | 0 |

这说明逐页读取能在该直接样本中把部分耗时前移；它把准备成本加到了首次输入前。开/关按顺序执行，OS 缓存和磁盘负载没有固定，不能由此推出稳定的 50 ms 修复。

按 Broker 会话设置进行的六组 Actor 隔离样本，每组两次合成键，均有 5 个候选且无 commit：

| 条件 | 首次 Actor dispatch |
| --- | ---: |
| 默认 A，不触碰 | 68,520 µs |
| Broker 选项 A，不触碰 | 31,285 µs |
| Broker 选项，触碰 | 5,331 µs |
| 默认选项，触碰 | 740,774 µs |
| Broker 选项 B，不触碰 | 59,038 µs |
| 默认 B，不触碰 | 80,075 µs |

相同的逐页策略在 Actor 中仍出现 740 ms 样本，不能保证改善；方案/选项与缓存、调度在该顺序试验中也不能拆成单一因果。长区间的线程 CPU 周期数远少于按 wall 时间持续占用 CPU 的量级，进程缺页计数增加，但这些计数不能区分硬缺页、磁盘、Defender 与调度等待。

完整 TIP→Broker 在开关打开、Broker 已预先启动并就绪时仍 **首次 probe exit 1**：客户端等待响应头约 49.9 ms 报 1460，Actor 首次 dispatch 为 234,976 µs、队列 23 µs；同一 Broker 的第二次完整 probe exit 0。该 Broker 从启动到宣布就绪为 **7,132 ms**，其中就绪前 7 次映射读取耗时合计 1,986,772 µs。第一次带开关的完整试验也出现首轮失败；其旧脚本在写结果 JSON 时出错，原始 probe/Actor 日志保留，不计为有完整结果记录的对照。

这项准备目前同时带来较高启动成本，且未解决首次桌面请求，不能纳入产品或更新 G2/G3 验收状态。它只覆盖本机隔离样本；不把同进程第二次通过算作首次切换、虚拟机或日常输入通过。

## 证据与下一步

[原始结果](../../build/win10-evidence-clean-v1/MappedPages-v1/results.json)和[54 项文件清单](../../build/win10-evidence-clean-v1/MappedPages-v1/host-evidence-manifest.json)绑定日志、两轮 TIP 探针、源码差异、诊断 DLL 和脚本。诊断 DLL SHA-256 为 `2C1476BF8BD803DF1940E79B3C12C776D9CE0813A2CFAB4484C6653D046AD055`；生产 DLL 仍为 `94D646160F78DFF6408E21DBD0C003CD7D71C5DD6B604AEDAA6CA780F96D93C1`。新增 Rust 探针已通过 build、rustfmt 与 Clippy `-D warnings`；本轮仅新增探针源码，没有发布或替换生产 DLL。

下一步需在拥有测试进程内继续细分候选懒加载、用户词典读取与代码页等待，并测量首次启动和加载 TIP 的完整路径。任何准备方案都要同时满足无输入、session 隔离、Broker 就绪时限和首次键 50 ms。浏览器/WinUI、真实鼠标、忙预编辑设置切换以及 loaded-TIP 升级/登录矩阵仍待 Win10 VM 验证。
