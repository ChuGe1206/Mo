# Win10 watchdog 进程退出延迟

日期：2026-10-04；Win10 22H2 build 19045，Rust 1.97.1/MSVC。仅合成 fault fixture 和非注册 TIP；VM 0.0.11.0 没有改动。

## 旧版实际时序

上一轮默认 workspace 四项退出超时，默认复跑 panic 超时，串行复跑 startup 超时，详见 [用户词典错误证据](WIN10-USERDB-ERRORS-EVIDENCE.md)。未改三秒断言。本轮独立归档旧机制 test exe，仅在 cfg(test) 的 abort 前加固定 `MO_TEST_FAIL_STOP_ENTER` 输出；生产未加入这个 marker。

父进程记录 fault marker、fail_stop marker、真正退出，并只读取 WerFault 的 PID 参数是否匹配自己启动的 child，不保存一般进程命令行。

| 阶段 | fault 收到 ms | fail_stop 收到 ms | 真正退出 ms | 原三秒门 |
| --- | ---: | ---: | ---: | --- |
| startup | 59 | 126 | 5,101 | 失败 |
| create | 152 | 300 | 2,183 | 通过 |
| apply | 70 | 149 | 2,733 | 通过 |
| destroy | 67 | 144 | 4,372 | 失败 |
| finalize | 1,592 | 1,661 | 4,116 | 失败 |
| panic | 158 | 234 | 7,430 | 失败 |
| worker-panic | 153 | 251 | 3,264 | 失败 |
| pool-shutdown | 364 | 445 | 2,689 | 通过 |

八项均到达退出函数；均观察到 WerFault 指向该 child PID；均实际以 `C0000409` 结束，未由父进程杀死。该样本证明延迟位于到达退出边界之后，不能直接解释为 watchdog 没触发。Microsoft 的 fastfail 文档说明该异常与 WER/调试基础设施交互；本轮直接终止对照支持修改这条路径。

这些是父进程收到 stdout/轮询后的时戳，不是 native CPU 时间。旧版额外读取 WMI，父进程可能因此晚记录；不能把相减区间全部宣称为 WER 耗时，也不据此排除全部历史调度问题。

## 修改和实际对照

`mo-windows-platform::fail_stop_current_process` 只接受当前进程：用 GetCurrentProcess pseudo-handle 调用 TerminateProcess，固定退出码 `0xE04D4F01`。没有 PID 参数、分配、日志、析构或正常 DLL detach。API 若意外返回则 fallback abort；非 Windows 仍 abort。Broker 保持 unsafe_code=forbid，原生 FFI 留在平台 crate。

Broker 的 watchdog、连接 worker panic、Actor 断连/超时继续共用 lifecycle::fail_stop。八个 unit fixture 仍要求正确 fault marker、三秒内实际退出和固定状态；改为拒绝任意非零或普通 panic 状态。启动 30 秒、请求/停机 5 秒的生产健康预算及 TIP 50 ms 均保持原值。

独立新 exe 连续三组、每组八阶段，共 24 child：

| 阶段 | 三次实际退出范围 ms |
| --- | ---: |
| startup | 117–141 |
| create | 120–215 |
| apply | 131–132 |
| destroy | 130–134 |
| finalize | 129–171 |
| panic | 53–64 |
| worker-panic | 36–114 |
| pool-shutdown | 121–131 |

全部固定 `E04D4F01`，全部原三秒门通过，没有 parent kill；整体 36–215 ms。新模式不查询 WER，所以 summary 中 matching_wer_reporter_observed=false 不表示已检测并证明不存在任何 WER 进程。当前 production source 没有诊断 marker/故障环境变量读取。

## 验证

- 默认完整 workspace、latency-trace 完整 workspace、两套 Clippy -D warnings、fmt 通过；旧超时证据仍保留。
- Release 优化的 Broker library EngineService 12 项通过，含八类实际子进程故障；test harness 的 unwind 模式不是生产 panic=abort 的全部验收。
- 双架构 MSBuild strict probe/ABI 与非改动 machine/current-user finalizer policy probe 通过；默认 fake TIP 完整 IPC、pool、候选/鼠标/布局/编辑/重连通过，各一轮故障恢复，共四次由 harness 明确结束 Broker。该外部 crash/restart 检查与新的内部 fail_stop 矩阵分开计数。
- 六项 harness 前置拒绝（相对路径、缺文件、hash、不合法名字、既有目录、不合法 variant）和两个脚本 AST 通过。harness 子进程八秒上限只用于观察旧机制的超时，不改三个秒的通过标准；只清理自己启动的 child。
- workspace/debug 与生产形状 Release Broker 构建另记归档；不安装/分发。

## 机制边界

仅 Broker lifecycle 调用改为直接终止；mo-windows-pipe 的极端 I/O drain/身份恢复失败 abort 和 Rust 隐式 panic=abort 暂未统一。TerminateProcess 不运行正常清理，并仍依赖 pending kernel I/O 完成或取消；没有证明永久驱动故障也能有界结束。该修改不新增学习耐久性、跨崩溃 exactly-once、自动重放或恢复被歧义取消的输入。

没有修改 WER 注册表/策略、默认输入法或 VM。真实词库首键/activation 超时仍未修复，G2/G3 不升级。接受 DLL SHA `94D646160F78DFF6408E21DBD0C003CD7D71C5DD6B604AEDAA6CA780F96D93C1` 保持不变，ADR 0055 新词典策略仍为独立原型。

## 重现和来源

运行 `tools/test-watchdog-exit.ps1`，显式传绝对 TestExe、ExpectedSha256、Variant、全新 EvidenceName。该入口只适合归档的合成测试 executable：legacy-abort 必须有 test-only stop marker，current-terminate 必须以固定新状态三秒内结束。日常单元验收用原 `cargo test --workspace`。

[Git 汇总](evidence/WIN10-WATCHDOG-EXIT-20261004.json) 绑定本机归档 `build/win10-evidence-clean-v1/WatchdogExit-v1`；绑定 117 文件和 8 外部身份，manifest SHA-256 `F8409E574A34269689478FC08C0553F88F944A7F48CF238B940D82AD75D56AC7`。包含旧/新/Release test exe、实际 Broker/TIP、源码、全部时序/构建/策略和 workspace/TIP 日志。build 归档不提交 Git。

微软原始契约：[TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)、[fastfail](https://learn.microsoft.com/en-us/cpp/intrinsics/fastfail?view=msvc-170)。下一步推进 ADR 0055 的新 ABI/构建来源/CI 接入，随后继续 Win10 安装态真实词库与宿主验收。
