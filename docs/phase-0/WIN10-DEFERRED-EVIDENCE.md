# Win10 固定缓冲与当前线程调度证据

日期：2026-10-07（客户端日期）；本轮合成能力检查开始于 2026-10-06。
基线 develop d398ef7；仅 Win10 主机开发诊断。未在 VM 安装新版本、改系统输入路由，
或修改接受的 ABI v3 runtime。50 ms 键请求与 400 ms 激活时限保持不变。

## 版本与测量边界

| 版本 | DLL SHA-256 | 用途 |
| --- | --- | --- |
| accepted ABI v3 | BE5E3E374D9FBE12381A0E3D522079033D85C08F4675F7A9A556A6DC3D30D564 | 未加内部诊断的对照 |
| diagnostic v5 | 6830E63ECD621E59B943AE25ACF36A21DBA6C90C93D37278E1FEEB93E76174B7 | key-only 固定缓冲，保留产物；未用于 TIP 矩阵 |
| diagnostic v6 | D00533045C95D5ECDBDEDB210D9DEE444D1642809CCAED6E9C47A6570324423B | key-only 固定缓冲与可选调度快照 |
| diagnostic v7 | 4C3E5B737141CCAED1122DDBD31C47662B92A62DB801AE2A124528A31A1F2916 | 补全惰性 Menu::Prepare 采样盲区 |

v5/v6/v7 分别保留 dist-v5/dist-v6/dist-v7；source-v5/source-v6 保留前版源码，
current source/compile 对应 v7。第五份 deferred-v3.patch 包含 ProcessKey 和
Menu::Prepare；源重放与实际编译的文件及头文件统一换行/末尾换行后核对一致，每份补丁应用后通过
严格 userdb startup 守卫。四组 v6/accepted 对照随后补测 v7，不将不同版本混作同 DLL。

固定 thread-local 缓冲最多 2048 scope/每个 owning capture、512 scalar/每进程；
低于 scope 阈值的记录省略，root 强制记录。嵌套 capture 复用 owner；
达到 scalar 上限后不再计时该字段。读取仅取时钟与 volatile load，不逐字段查询
驻留、缺页、CPU 或调度。scope 标签构造、原子与计时仍有扰动。

MO_DEFER_FLUSH 分开 arm/capture/flush；数据排出在 native scope 结束后、
调用者返回前。Actor 请求与真实 TIP 仍包括排出及候选生成时间。
不能把 native ProcessKey wall 当作完整请求或 IPC 验收。

- Broker 始终为固定 93211EC1… latency-trace debug 构建。
- 前端 x64/Win32 各使用固定 trace TIP/probe；引擎与 DLL 均为 x64。
- 合成 fresh profile 是新目录；existing 在该目录复用数据库。
- 仅 debug local user/build 计划，不是安装态 shared-prebuilt。
- 顺序、缓存、调度/宿主负载未控制或随机化，不代表系统冷启动。
- 所有开关默认关闭，collector 清除继承值，拒绝混合页面查询/预取及单独调度开关。
- 这些诊断补丁不进入产品 runtime/stage 或安装包。

## 完整 TIP 结果

| 模式 | 前端/目录 | ready ms | 首 Actor µs | 第一/第二 probe exit |
| --- | --- | ---: | ---: | --- |
| base | x64/fresh | 2357 | 290048 | 1/0 |
| base | x64/existing | 289 | 2943 | 0/0 |
| base | Win32/fresh | 1189 | 88420 | 1/1 |
| base | Win32/existing | 375 | 2528 | 0/0 |
| off | x64/fresh | 2118 | 139172 | 1/1 |
| off | x64/existing | 551 | 7161 | 1/0 |
| off | Win32/fresh | 1488 | 85308 | 1/1 |
| off | Win32/existing | 538 | 6491 | 1/0 |
| on | x64/fresh | 2227 | 74749 | 1/1 |
| on | x64/existing | 480 | 9179 | 0/0 |
| on | Win32/fresh | 1667 | 67850 | 1/0 |
| on | Win32/existing | 375 | 7010 | 0/0 |
| thread | x64/fresh | 1480 | 84081 | 1/1 |
| thread | x64/existing | 303 | 6845 | 1/0 |
| thread | Win32/fresh | 1473 | 104495 | 1/0 |
| thread | Win32/existing | 418 | 7934 | 0/0 |
| menu | x64/fresh | 3672 | 324305 | 1/1 |
| menu | x64/existing | 452 | 4580 | 1/0 |
| menu | Win32/fresh | 954 | 90274 | 1/1 |
| menu | Win32/existing | 274 | 4901 | 0/0 |

40 次完整 probe：21 失败、19 通过；其中首轮 14 次、第二轮 7 次失败。
所有负向结果保留，脚本零退出仅表示采集完成。首轮快不能替代完整流程；
第二轮也不能替代首轮或首次激活。base 的 x64 fresh 首 Actor 290048 µs，
Win32 fresh 88420 µs，未加内部诊断也仍失败。

## 降低观测扰动后的字段与调度

v6 deferred/no-dispatch：fresh 首 native x64/Win32 为 68607/62147 µs；
读取各 419 条，合计 16.442/24.261 ms，最大 weight 窗口 5.083/14.041 ms；
排出 1664/1411 µs。existing 首 native 2079/1703 µs，读取合计约 70 µs。

v6 deferred/dispatch：fresh 首 native 为 77726/99042 µs，排出 1411/1200 µs；
单 weight 窗口最大 14.868/13.525 ms；current-thread 切换 65/34 次，
位图 2147483649（0、31）/1（0）。所有启用、两次读取、关闭返回 0。
快 existing 首 native 1396/1690 µs 的快照为零切换。

v7 x64 fresh：首 native 321378 µs；419 条字段窗口合计 268.170 ms，
首 weight 窗口 69.834 ms，排出 1192 µs；调度 29 次，位 0、31。
Win32 fresh 首 native 87724 µs，26 次切换，位 0；读取合计 35.697 ms，
最大窗口 9.925 ms。慢窗口在取消驻留查询、热路径同步日志后仍存在。
时钟窗口可能含中断/等待，不能将所有 wall 归为 CPU 执行或实际磁盘读取。

线程能力检查只请求当前自有线程、hardware counters=0；一份 10 ms 合成 Sleep
得到一次切换、位图 16，成功关闭。没有启动/取消系统 ETW/WPR，
当前 token 非提升管理员；没有采集其他应用的内容、全局文件名或真实输入。

API 的累计切换计数、上次读取后的等待位图来自
[PERFORMANCE_DATA](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-performance_data)。
位 0、31 对应 Executive、WrDispatchInt，见
[微软 KernelWaitReason](https://learn.microsoft.com/en-us/dotnet/api/microsoft.windows.eventtracing.cpu.kernelwaitreason?view=trace-processor-dotnet-1.0)。
这些都是聚合计数；无等待逐项时长、顺序、地址或文件身份，不能证明哪个 load
导致了哪个切换；没有 PageIn 位也不能排除缺页。CPU cycles 保留原单位不换算时间。
[启用 API](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-enablethreadprofiling)
仅用于当前线程，不配置全局硬件计数器。

## 惰性候选生成盲区与后续键

v6 x64 fresh 的第三个 native capture 为 31501 µs；之后未缓冲的 Menu::Prepare
120829 µs，Actor 完整请求 152537 µs。其间 Sort 57250 µs、两段 Marisa decode
23039/35909 µs。此时调度快照仅覆盖 ProcessKey，不能解释后续候选区间，
外层原 scope 仍有查询/同步日志扰动；原始负向记录保留。

因此 v7 给 Menu::Prepare 增加 owning capture，嵌套时不会重复排出。
真实 v7 x64 fresh 的第八个 frame（menu）94298 µs、排出 58 µs、两次切换、
位 0；其中 Sort 51677 µs。候选生成本身可以超过 50 ms。
这里只是定位慢区间，补测版本/时序不同，不能宣称前后数值为优化收益。

x64 existing 的 v7 首 native 1357 µs/首 Actor 4580 µs，但后续 native 达
256183 µs（17 次切换、位 0），完整首 probe 仍失败。保留所有 dispatch 摘要及
>10 ms key/menu 帧；采样 cap 不重置为每个键，以免误认为后续仍有字段样本。

## 验证与证据

- 严格 /W4 /WX /MT 的独立 probe 构建零警告/错误；上游 librime 警告仍存在。
- 六项 deferred probe：off/on/overflow/dispatch/preprofile/invalid 通过；
  512 cap、2048 scope、105 溢出记录、截断、层级/顺序、双 capture 重置、
  标量值、已有采集所有权保护与禁用控制通过。preprofile 保留外部 owner，
  报 AlreadyExists/invalid 后由测试自己的 owner 关闭，不把拒绝报为有效数据。
- 原四项 read/prefetch probe 回归、十一项 collector 拒绝门及 AST 通过。
- 最终五补丁 replay 与实际源码/头文件统一换行/末尾换行后一致，userdb startup 守卫逐项通过。
- fmt、Clippy -D warnings、完整 workspace 通过（本轮 Rust 源码未改）。
- 十个非空 CI PowerShell block、四个脚本 AST 严格检查通过；YAML diff 手工核对。
  远端 CI 未验证。初版辅助 AST 采集表达式错误且未遇错停，输出不能算通过；
  修正后以最终严格检查日志为准。
- 日志及记录均为固定合成输入；没有真实键、候选、剪贴板或口令。
- 所有 owned Broker/probe 已退出，stage BE5E3E37… 与 VM 0.0.11.0 保持原状。

[Git 汇总与清单身份](evidence/WIN10-DEFERRED-20261007.json)绑定
build/win10-evidence-clean-v1/Deferred-v1。原始 PE 按 SHA 去重保存，逐个核对
实际 run 副本；源码、日志、最终合成数据库状态保存。体积较大的未修改
profile/build 字典仍在原 owned 路径，作为 external input 哈希绑定并校验，
不是 fresh 时刻的数据快照。旧 ReadPages-v1 原始证据未改。

下一步集中 Win10 的映射字段、Marisa decode 与惰性候选生成等待；
需要把当前线程聚合证据进一步关联到缺页/I/O/调度逐项事件，并检验首次拉起、
完整后续键及未加诊断的产品路径。安装态新 ABI、普通宿主、loaded-TIP、
登录/重启矩阵仍待完成。G2/G3 及 50/400 ms 验收不宣告完成。
