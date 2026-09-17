# ADR 0023：候选定位重入保护与按键共用截止时间

- 日期：2026-09-17
- 状态：接受实现边界，真实压力与普通宿主仍待验收
- 延续：ADR 0022；不放宽 50 ms、不重试歧义输入、不屏蔽宿主回调。

## 确定性缺陷与回归

异步 Edit Session 入口检查身份，不足以保证之后定位/显示期间仍有效。宿主 COM 方法和窗口调用可能在同线程重入，使状态失效。原实现 GetTextExt 后直接使用矩形显示候选，没有复核定位过程中的失效事件。

受控 EditTextStore 新增一次性 GetTextExt hook：先测量矩形，再同步通知 TIP 的 TF_LC_CHANGE，暂缓后续锁，最后返回已失效的矩形。输入只发生于 test-owned fixture；完成后 Escape 清除，既有最终 EDIT/context 文本断言不变。

旧实现的 x64 fake 回归失败。进一步在同一份代码中临时仅禁用身份检查做 A/B，得到 `MO_REENTRY key_ok=1 callback=1 result=0 stale_visible=1`，证明不是按键拒绝、hook 未运行或通知失败。该临时禁用已立即恢复，最终源码没有绕过开关。保护开启的默认 x64/Win32 各 100 轮故障回归通过，每轮同时包含重入检查和两次 owned Broker 退出/恢复。

这是**失效旧位置被重新显示**的确定性缺陷，不证明 ADR 0022 中偶发“候选消失”全部来自同一原因；后者继续保留为待定位项。

## 实现边界

- 定位入口捕获拥有 COM 引用的 context/range，以及 epoch/revision/connection generation/token。宿主调用期间成员被清理，局部对象仍存活，不继续解引用可能已清空的成员。
- 匹配布局通知、状态回收、断开连接推进 candidate epoch；饱和而不回绕，到达上限后不再接受定位身份。
- GetTextExt/GetWnd 完成后复核身份、焦点、连接及 authoritative snapshot revision；窗口 Update 前后再次核对。失效即隐藏，不重新显示旧页面。新的定位仍走现有身份校验的只读 Edit Session。
- epoch 是本线程生命周期状态，不改变 IPC Snapshot、候选授权或 Rust session id 边界。排序和 commit 仍由引擎及写锁内 CandidateAction 决定。

平台语义参照 Microsoft [OnLayoutChange](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itftextlayoutsink-onlayoutchange) 与 [OnCompositionTerminated](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcompositionsink-oncompositionterminated)。默认 view 的正常通知是 TF_LC_CHANGE；保留 TF_LC_DESTROY 防御性清理，不将其猜测为已确认根因。

## 按键的传输预算不累加

原 BrokerClient 每个操作已有 deadline，但 TIP 一次 DispatchKey 中先 EnsureBrokerConnected(50 ms)，再 SendKey(50 ms)，context 切换还可能等待旧会话 CloseSession。各阶段分别受限，不等于整次按键共用 50 ms。

新增 ConnectAndOpenUntil/SendKeyUntil 接受同一绝对 QPC-backed deadline，保留原 timeout API 作为 wrapper。TIP 在 DispatchKey 入口创建一次 50 ms 截止时间，用于重连、Hello/OpenSession、KeyEvent/回复。context 切换与按键错误不额外等 CloseSession ack，而是零预算断开，让现有 disconnect 回收路径销毁旧会话；不重发输入。

IPC 回归验证：预算耗尽后，即使新连接已成功，旧绝对 deadline 的 Space 仍被拒绝、连接清理、sentinel snapshot 保持不变；新会话使用仍有效的一份共用 deadline，Z 从空预编辑开始且无旧 commit。该检查在 fake/真实词库和双架构客户端执行。

这只统一**传输等待预算**。调度/取消排空可能延长实际返回时间；同步宿主 COM、文档写入和候选渲染没有可强制中止的 50 ms 保证。激活/显式焦点恢复仍有各自既定的 400 ms 准备预算。

## 诊断与未通过边界

显式诊断构建独立记录最后候选清理原因/次数、snapshot 是否存在、最后 key Edit Session 两个 HRESULT，不被后续“Update 成功”覆盖。原 count=0 不足以区分无 snapshot 与存在空 snapshot，新字段消除歧义。原因是固定枚举，不含输入文本、键值、路径或 session token；默认构建没有这些字段或 COM 诊断接口。

新增清理诊断、重入保护落地前的完整真实命令，x64 第 89/100 轮首次 N 超时：排队 10 µs、Actor 50,440 µs、客户端 header 50,689 µs/总计 50,727 µs，尚未到第一次实际退出注入；未进入 Win32 压力段。该失败不因 guard 修复或 fake 通过而抹去。

保护与共用截止时间落地后的完整命令，x64 第 82/100 轮仍出现首次 N 超时：排队 3,469 µs、Actor 55,959 µs、header 59,590 µs/总计 59,626 µs，尚未到第一次实际退出注入。该轮有排队开销，但引擎本身也超过 50 ms；不能宣称 cold-tail 已通过。前 81 轮同时通过真实重入/候选/文档/故障恢复检查，未进入该命令的 Win32 压力段。

Win32 随后独立运行一份非重试命令，真实词库 100/100 轮通过，覆盖同一个 x64 Broker 的 IPC/16-client pool、候选/重入/延迟锁/焦点恢复与最终 EDIT/context 文本。它是这一平台和此命令范围的证据，不取消 x64 或更早的 cold-tail 失败，不构成双架构真实压力整体通过。

最终恢复默认关闭诊断的构建后，真实预编译词库完成 x64/Win32 各 20 轮故障恢复及完整 IPC/pool/TIP 检查，包含有效 context view 的重入 fixture；MSVC 双架构零警告。Rust workspace 的 105 项运行时测试和 1 项 compile-fail doc test、fmt、默认 all-targets Clippy（warnings-as-errors）通过；16 项注册事务内存测试与 20 项 OpenCC 资源/编译器检查通过。只读 registrar 状态仍为双视图 COM 缺失、profile 未注册/启用/激活。有限回归不替代上述失败的 100 轮命令，也没有注册或改变默认输入法。

首次转换仍有 OpenCC 延迟初始化，resource anchor 不发送伪输入预热。后续应在正式 allowlisted 自构建 runtime 验证无输入资源准备，将首次加载前移到 Broker ready 之前；当前官方开发 DLL 不具备该 Mo 准备能力，尚未宣称实现或性能通过。注册系统路由、Notepad/WinUI、机器冷启动、发行安装仍未通过。
