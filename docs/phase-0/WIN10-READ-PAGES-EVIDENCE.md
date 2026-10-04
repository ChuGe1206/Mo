# Win10 映射字段读取与系统预取实验

日期：2026-10-04。基线 develop `de54d81`，主机 Windows 10.0.19045。
本轮继续上轮 Sort/Peek 慢路径；全部键和 profile 均为合成数据，未操作 VM。
50 ms key、400 ms activation 不变，G2/G3 仍未完成。

## 工程变更与实际产物

`read-pages-v3.patch` 增加候选权重、string ID 标量读取测量，并细分
Table/StringTable GetString 和 Marisa reverse_lookup。新增头
`mo_read_diagnostic.h` 用 QueryWorkingSetEx + VirtualQuery 查询目标字段页，
分别计量实际 volatile load 与元数据查询；只输出固定标签、计数和耗时，
没有地址、词条、权重值或输入内容。每进程最多输出前 512 个读取样本。
关闭采样时直接返回原标量；开启时明确按 lhs/rhs 顺序读取权重，不能忽略
该顺序、强制 load 和额外 Windows API 对实验的扰动。

字段自然对齐且只取不超过 8 字节的算术类型；QueryWorkingSetEx 的 Valid
作为该地址的工作集驻留代理。查询支持不在工作集的地址，但不能由 Valid=0
判定磁盘硬缺页。[Microsoft API 说明](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-queryworkingsetex)

独立读取 DLL：`CF021ED0B69D5ECFC61C251F6543C8B82C1A84C831EF8CFE727E94CC80CF3E3F`，
保留于 `build/win10-abi3-components-v1/dist-v3`。在其后应用可选
`prefetch-v3.patch`，得到
`F2A4C015683CD718665E36A6014D31E03604C2A51F55B84C9EB1A7768BDABDBB`，
保留于 `dist-v4`；current source/compile 是 v4，source-v3 保存 v3 源码。
ABI v3 严格打开、必需 userdb 检查、禁止自动 recovery 与 wrapper `/W4 /WX` 保留。

预取只在测试进程开关 `MO_DIAG_PREFETCH_RANGES=1` 时，向只读 mapping 和
当前 DLL 已提交且可读的 image regions 发出 PrefetchVirtualMemory 提示。
不触摸页面、不加锁或改保护、不发送准备键；失败不改变健康 preparation
的布尔结果。函数指针布局在独立 strict probe 中与 SDK 结构比对。
系统文档明确：此提示缓存到物理内存，不加入进程工作集；也不保证低内存
情况下完整兑现，因此 API 返回成功不能代替后续时限验证。
[Microsoft 预取契约](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-prefetchvirtualmemory)

以上补丁均不进入 runtime builder、stage 或安装包。TIP x64/Win32 两列仅指
前端架构，Broker 与 librime 都是同一 x64 产物；不是两种 native 引擎架构对照。
实际 trace Broker 沿用固定副本 `93211EC1…`、TIP/probe 沿用上轮已归档字节。

## 单字段读取观察

采样开启的四个进程，首个 native ProcessKey 中各记录 419 次标量读取，
其中 24 次在查询成功时工作集 Valid=0。每个进程输出总数恰为 512。

| TIP / profile | 首个 ProcessKey µs | 首键标量 load 总 µs | 首键查询总 µs | 最大单次 load µs |
| --- | ---: | ---: | ---: | ---: |
| x64 fresh | 84500 | 3052 | 17238 | 3016（string ID） |
| x64 existing | 106787 | 15432 | 16025 | 10277（sort weight） |
| Win32 fresh | 25129 | 52 | 17726 | 4 |
| Win32 existing | 23598 | 31 | 17364 | 5 |

10277 µs 的 weight 样本：MEM_MAPPED、before Valid=0、after Valid=1、
process_faults=1、thread_cycles=99916；3016 µs 的 string ID 样本亦从
非驻留变为驻留、faults=1。定位到了具体 load 内的等待，但故障计数是整个
进程的、cycles 不能换成固定频率 CPU 时间；不能宣称已确定磁盘/Defender。
同样的 24 个非驻留样本在 Win32 fresh 总 load 只有 52 µs，说明单纯缺页数量
不能解释所有失败。约 16–18 ms 的查询时间本身是额外开销，不能加回或扣除
后把采样结果当作未采样性能。

解码慢区间也保留：采样关闭的 x64 进程有 Marisa reverse_lookup 约 33 ms；
采样开启的 existing x64 有约 21 ms 的区间、缺页增量 15。这些是整个 probe
期间的样本，不能全部归给首 N。

## 真实 TIP 对照

每组有 fresh/existing 各一进程、每进程首轮/第二轮各一 probe。关闭和开启
按顺序运行；缓存、合成学习、主机负载未随机化，不是冷启动或安装态测试。
所有路径仍是 debug local user/build，非 installed shared-prebuilt 计划。

| DLL / 开关 / TIP / profile | host ready ms | 首个 Actor dispatch µs | 首轮 exit | 第二轮 exit |
| --- | ---: | ---: | ---: | ---: |
| v3 read off / x64 fresh | 1587 | 176096 | 1 | 0 |
| v3 read off / x64 existing | 377 | 6841 | 0 | 0 |
| v3 read off / Win32 fresh | 1704 | 74733 | 1 | 0 |
| v3 read off / Win32 existing | 573 | 7528 | 0 | 0 |
| v3 read on / x64 fresh | 1849 | 94928 | 1 | 0 |
| v3 read on / x64 existing | 748 | 115741 | 1 | 0 |
| v3 read on / Win32 fresh | 1484 | 35715 | 0 | 0 |
| v3 read on / Win32 existing | 331 | 32302 | 0 | 0 |
| v4 prefetch off / x64 fresh | 1247 | 141533 | 1 | 0 |
| v4 prefetch off / x64 existing | 358 | 8196 | 0 | 0 |
| v4 prefetch off / Win32 fresh | 1248 | 68204 | 1 | 0 |
| v4 prefetch off / Win32 existing | 290 | 6614 | 0 | 0 |
| v4 prefetch on / x64 fresh | 1020 | 9481 | 0 | 0 |
| v4 prefetch on / x64 existing | 394 | 6843 | 0 | 0 |
| v4 prefetch on / Win32 fresh | 1636 | 129216 | 1 | 0 |
| v4 prefetch on / Win32 existing | 1355 | 184683 | 1 | 1 |

32 次完整 probe 中 9 次失败、23 次通过。首个 Actor 排队 10–61 µs。
第二轮不能替代首轮；少数 ready <400 ms 样本不能宣布稳定激活。

预取开启每个 startup 均调用七个 mapping + 一次 image 预取，API 全返回 1，
image 11 段/3653632 字节。x64 fresh native ProcessKey 3564 µs，但 host ready
仍 1020 ms；Win32 fresh/existing native 为 125027/180277 µs，完整流程仍失败。
Win32 existing 的 60703784-byte mapping 单次预取耗时 486438 µs，已超过
activation 总预算。进程中其后会话还有预取调用，总次数不能当作 startup
次数；原始日志完整保留。**不接入产品，不认为该提示已修复延迟。**

## 测试修复与验证

首轮完整 workspace 的设置中心测试在 Fixture::new/create_dir 报 Windows
183 AlreadyExists。原 fixture 仅以 PID + SystemTime.as_nanos 命名，不能保证
并行唯一。`80adaaf` 添加原子序列与限定的 AlreadyExists 重试，仅在实际创建
成功后取得清理所有权；固定相同时间戳的八线程回归验证目录、数据及清理独立。
这是 test-only 修复，未修改产品设置保存行为；旧失败日志继续保留。

修复后 settings-app 六项和完整 workspace tests、fmt、Clippy -D warnings 通过。
独立 `/W4 /WX /MT` read/prefetch probe 零警告/错误；off/on/prefetch/invalid 四项
通过，未改变标量值、512 cap、预取后页面仍未进入工作集的控制均通过。
首个 strict build 因 NOMINMAX 重定义被拒绝，修正 guard 后构建，旧日志保留。
三份诊断补丁及第四份可选实验真实 check/apply 后各通过 userdb policy，
与当前编译源码正规化换行后核对一致；八项 collector 拒绝门与三个 AST 通过。
CI 已接入 strict probe 和可选 replay，十个 multiline PowerShell block AST 通过，
YAML diff 手工核对；远端 CI 未验证。所有 owned Broker/probe 已结束。

## 证据与后续

[Git 汇总及清单身份](evidence/WIN10-READ-PAGES-20261004.json)绑定本机
`build/win10-evidence-clean-v1/ReadPages-v1`，包含实际两版 DLL、两版 strict probe、
双架构前端/Broker、32 次流程原始日志、源码/header/编译契约与合成 profile。
接受的 ABI v3 stage BE5E3E37…、VM 0.0.11.0 均未改。

已把慢路径缩到实际映射字段 load 和字符串 trie 解码；系统预取收益不稳定且
启动超预算。下一步在隔离 Win10 环境取内核缺页/I/O/调度证据，降低观测扰动，
再验证 fresh 首次拉起与后续键；安装态新 ABI 和宿主/loaded-TIP/登录矩阵待做。

2026-10-05 收尾：按实验起始日保留证据文件名；567 个归档文件、245 个外部输入
校验通过，manifest 18BCD35D…，接受 stage DLL 哈希未变。32 次探针统计为
9 失败/23 通过，完整 workspace 在修复测试目录碰撞后通过。
