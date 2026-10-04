# Win10 Broker 启动分项证据

日期：2026-10-04，基线 `ecd8953fac635bc77c59cad9923f5262c1a23fb5`。延续 [DB 打开诊断](WIN10-DB-OPEN-EVIDENCE.md)，只在 Win10 主机的合成目录工作。生产 runtime/stage、VM 0.0.11.0 未变；50 ms key、400 ms activation、首次切换 `<500 ms` 门槛不变，G2/G3 不升级。

## 诊断实现与测量边界

[启动计时模块](../../crates/mo-broker/src/startup_latency.rs) 与既有 `latency-trace` 共用 256 项有界通道。仅 `debug_assertions && latency-trace` 读取 Instant/写固定八槽 atomic；Span 在默认和 release 构建为零大小，编译期断言持续约束。main 入口开启采集；没有调用 begin 的库调用不发布启动记录。

启动记录先存时长，捕获 main-to-ready 后输出既有 listening 行，初始化既有后台 logger，再通过 try_send 发布。满/断开通道计丢弃，不等待 stderr，也不 join logger。字段只有固定 phase、elapsed_us、dropped；不传路径、设置、会话 token 或输入。记录代表每个 Broker 进程的一次启动，不能用来描述同进程反复创建服务的独立启动。

| phase | 范围 |
| --- | --- |
| parse | 参数采集、启动计划解析及开发模式目录/文件校验 |
| pipe_bind | 默认 16 槽受保护 pipe pool 绑定 |
| settings | 默认设置/安装设置打开与 cancellation 对象创建；本轮仅默认设置 |
| engine_load | Engine::load，包括加载 DLL 和 native 服务初始化 |
| backend_prepare | prepared backend 构造，包括保活 Session 和 native PrepareResources |
| engine_start | EngineService 启线程与等待 factory 完成；包含 engine_load、backend_prepare |
| workers_start | 16 个连接 worker 的创建与 gate 发送；不保证每个 worker 已获得调度 |
| main_to_ready | main 入口至 listening 行写入之前，包含前述阶段及少量 glue/watchdog 成本 |

嵌套阶段不能全部相加。main_to_ready 不含 OS 创建进程、PE/CRT 初始化；host ready 从父进程 Process.Start 调用前计时，至父进程读到 listening 行。两者差值还包含父进程启动开销、调度和 stderr 发送/接收，host 毫秒计数也有量化误差。不能将差值直接称为 PE 加载耗时，或据此断言 Defender、磁盘、IPC 有问题。native 同步 stderr tracing 仍会扰动样本。

## 五组完整 TIP 对照

同一全新 owned profile 从 pinned compiled seed 复制 `user/build`，第一组 fresh；后四组保留前组产生的合成用户词典状态。默认设置允许本地学习，所有键均由受控探针生成。此路径没有测试安装态共享 machine prebuilt 配置，没有注册 TIP 或操作 VM。

所有进程使用相同 `CC732CB…` DLL，开启 mapped/image 准备、DB I/O tracing；reuse 只改变上游 experimental reuse_logs 选项。该选项未接入产品。按实际运行顺序：

| case | host ready ms | parse µs | pipe_bind µs | Engine load µs | backend prepare µs | main_to_ready µs | host-minus-main µs |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| x64-fresh-reuse | 2,044 | 114,770 | 2,797 | 31,336 | 1,694,576 | 1,844,434 | 199,566 |
| x64-existing-off | 900 | 499 | 1,426 | 6,853 | 566,147 | 575,713 | 324,287 |
| x64-existing-reuse | 298 | 468 | 1,437 | 6,138 | 147,308 | 156,119 | 141,881 |
| Win32-existing-reuse | 175 | 441 | 1,366 | 5,712 | 141,174 | 149,687 | 25,313 |
| Win32-existing-off | 1,116 | 455 | 1,704 | 7,394 | 653,593 | 664,198 | 451,802 |

settings 为 7–13 µs，workers_start 为 457–661 µs。engine_start 分别为 1,726,191 / 573,253 / 153,602 / 147,253 / 661,300 µs；它包含 load/prepare，不能重复累计。五组各首轮和第二轮完整探针均 exit 0（十次），每次均核对真实 rime-ice 候选/模型鼠标/布局/延迟取消/重连与恰好两次显式 TSF 终止重入 fail-open marker。八条启动记录均在 listening 后、dropped=0。

| case | 首个 DB::Open µs | 该 Open 内 Sync 次数/总 µs | 首次 Actor queue/dispatch µs | native Session / PrepareResources µs |
| --- | ---: | ---: | ---: | ---: |
| x64-fresh-reuse | 311,700 | 2 / 293,722 | 44 / 3,634 | 1,540,269 / 154,264 |
| x64-existing-off | 413,364 | 3 / 385,998 | 11 / 3,831 | 480,177 / 85,937 |
| x64-existing-reuse | 3,152 | 0 / 0 | 13 / 4,951 | 67,884 / 79,396 |
| Win32-existing-reuse | 3,033 | 0 / 0 | 40 / 3,980 | 62,890 / 78,254 |
| Win32-existing-off | 494,666 | 3 / 418,164 | 60 / 3,647 | 569,450 / 84,095 |

Sync 统计仍只覆盖首个 DB::Open 调用线程及匹配 parent id，包含原生 FlushBuffer + FlushFileBuffers，不证明磁盘单独成本。本轮已有目录的主要已测差异仍落在用户词典打开；warm reuse 的 backend prepare 稳定在这些样本的约 141–147 ms，pipe 绑定约 1.4 ms。

fresh 不同：native Session 达 1.54 秒，parse 115 ms；首个映射 FileOpen 有 213,940 µs，另一个词典加载区间超过 400 ms，首次资源准备 154 ms。fresh/复用同时改变文件状态、缓存和运行顺序，尚未确定全部等待来源。不能把已有目录的改善推广为首次创建或机器冷启动修复。

298/175 ms 是本轮有限已有目录样本，先前 471/525 ms 超预算样本继续保留。首键发生在 prestarted Broker ready 之后，完整 TIP 通过不等于首次激活总延迟通过；fresh 2,044 ms 仍远超预算。当前实验不是安装态或注册桌面宿主验收。

## 无 librime 的 fake 对照

三次启动同 SHA 的归档 Broker 副本（路径与完整 TIP 的 root Broker 不同），无输入，只采集就绪及六个适用 phase：

| trial | host ready ms | main_to_ready µs | pipe_bind µs | host-minus-main µs |
| --- | ---: | ---: | ---: | ---: |
| 0 | 1,705 | 2,769 | 1,699 | 1,702,231 |
| 1 | 26 | 2,305 | 1,366 | 23,695 |
| 2 | 29 | 2,458 | 1,446 | 26,542 |

首次副本启动的巨大差值在没有 librime 时也出现，说明需要继续测量外部启动/接收阶段。不同路径、首次复制/启动、缓存和调度都是混杂因素；不证明任何具体扫描器、驱动或 API 是原因。后两次 26/29 ms 也不是首次启动上界。

## 检查、身份与交接

- Rust 1.97.1 fmt、默认/trace 的 debug/release 四种 workspace/all-targets Clippy `-D warnings`、默认/trace 两套 workspace tests 通过。
- 新增 absent/zero/saturated 时长测试，固定 phase 输出与通道满/断开测试；沿用既有非阻塞 logger 检查。CI 现有 trace workspace gate 已覆盖它们。
- 实际默认 debug fake 仅输出原 listening 行；默认 debug 与启用 feature 的 release PE 均无 `MO_STARTUP`/`MO_LATENCY` 输出字符串。两种 release 实际二进制各五项启动拒绝检查通过，没有安装/注册变更。
- 所有 harness 创建的 Broker 均已退出；后续 Cargo 重建的 root Broker 不可计入历史样本，实际测试副本已封存。

| 产物 | SHA-256 |
| --- | --- |
| 实测 debug trace Broker | `DD3DB30E7D7287A186A4419075579395246BDA1C6B8A458EA04A089680A33817` |
| 关闭诊断的 debug Broker | `27162C30D6285DC49965A2F2418D95644051ED0AF979F0556F4FFA90561A5FC0` |
| 启用 feature 的 release Broker | `F25E962599C5AAB27C46ABC34A451946D3EAB7783E7DD11729BBEBF3BD57585C` |
| 延用隔离 runtime | `CC732CB66A745372D97E73AC4688E4D15240BFCB00385D37B17AEF281D37D65E` |
| 接受的生产 runtime，未变 | `94D646160F78DFF6408E21DBD0C003CD7D71C5DD6B604AEDAA6CA780F96D93C1` |

[Git 结果汇总](evidence/WIN10-BROKER-STARTUP-20261004.json)、[本机原始汇总](../../build/win10-evidence-clean-v1/BrokerStartup-v1/results.json)与[哈希清单](../../build/win10-evidence-clean-v1/BrokerStartup-v1/host-evidence-manifest.json)绑定 55 个归档文件和 157 个外部输入身份。清单 SHA-256 为 `C1A50611CC5CC3F81EFE6726D228E23D1F7F076E822F5F89D40D65EC76CAE8F8`。归档含实际三版 Broker、DLL、双架构 TIP/probe、五组完整日志、三个 fake 日志、源码、历史 harness 与检查日志；延用 native 编译源码由上一轮 DbOpen 清单绑定。大文件 build 归档保持本机，Git 汇总供远端恢复；不是 PE 字节可复现承诺。

下一步继续 Win10：验证 experimental reuse_logs 的合成崩溃/错误恢复与代表性词典，再决定是否形成产品方案；同时细分 fresh Session、路径/映射打开与进程外启动成本。产品方案仍需保持无输入准备/会话隔离，满足原预算，再进入安装态共享数据、注册宿主、loaded-TIP 和登录矩阵。
