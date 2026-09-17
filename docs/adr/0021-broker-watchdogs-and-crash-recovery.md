# ADR 0021：Broker 有界停机、引擎 watchdog 与断线恢复

- 状态：接受
- 日期：2026-09-17
- 演进：保留 ADR 0019/0020 的受保护固定槽、身份复核、唯一 Actor 和整帧预算；补齐进程生命周期边界。

## 决策

生产连接池接受协调者持有的 `PipeCancellation`。它是未命名、只置位、不复位的进程内事件，客户端协议没有退出命令。事件以 `Arc<File>` 持有，所有 connect/read/write 的等待同时观察它。停机优先于同时完成的 I/O；按具体 OVERLAPPED 取消并等待完成后才释放存储。取消错误使用 ConnectionAborted，不能使用会被 `read_exact`/`write_all` 无限重试的 Interrupted。

停机顺序为：停止接入/唤醒等待 → 各连接回收自己拥有的 session → 等待全部槽线程退出和 retained handle 关闭 → 向 Actor 发送 Shutdown → 在线程内 finalize 原生引擎。Shutdown 不依赖所有闲置 EngineClient clone 都已释放。某槽诊断错误先置位停机事件，再由协调者 join，避免等待较早的正常槽而看不到较后的失败槽。槽线程 panic 不尝试继续服务，直接 fail-stop。正常模式的预期 peer 错误不逐条写 stderr，避免不可信请求造成日志放大或阻塞停机。

三层固定预算分别约束不同风险：

- 引擎初始化：30 秒；初始化超时终止 Broker，不留下可能晚完成的 native worker。
- 每次 create/dispatch/destroy：排队与引擎执行共用 5 秒。普通 backend 错误仍是协议错误；超时或 reply channel 意外消失则终止 Broker。
- Actor 停机/finalize：5 秒；整个连接池从协调停机开始至所有 worker 和 Actor 完成另有 15 秒总预算，避免 64 个 session 的逐次回收累乘。总停机 watchdog 使用两个进程内事件等待，不做空闲轮询；完成与停机同时置位时，完成优先。

无法安全打断任意 librime C 调用，故超时不是取消 native 命令、更不是重试。fail-stop 使用进程 abort，不先等待日志、析构或原生 finalize。Rust watchdog 只在独立 Broker，不进入 C++ TIP/编辑器进程。预算是进程健康保护，不是输入延迟目标：TIP 保留更短的 50 ms 按键 deadline 和重连退避。

## TIP 的失败策略

鼠标候选动作只有取得 TSF RW cookie 并复核 context/generation/session/revision 后才发给引擎。如果通信失败或回复不明确，必须在**已有 cookie** 内清除自身的未提交预编辑，再断开 Broker；不能申请嵌套锁，也不能仅把运输错误返回给 TSF，让宿主结束 composition 后留下普通拼音文字。清除失败仍返回真实编辑错误，不能宣称已恢复。已提交文本不在清除范围内。

恢复建立新 connection generation/session，正常身份复核仍执行；不重放旧候选、旧按键或歧义 commit。恢复可能丢弃未提交输入，但不能自动补交旧词。本次重复故障测试实际发现旧预编辑偶发残留，以上策略及回归断言一并修复。

## 可重复验证

`cargo +stable test --workspace` 包括真实内核停机用例：等待连接、部分首帧、认证后完全空闲、写回背压，以及 16 槽同时停机、混合状态停机和后槽失败唤醒前槽。对仍存活的 peer 不等待 EOF，完成后可重新绑定整池名称。事件等待测试核对同时置位的完成优先和零预算超时。

八个独立子进程 fixture 注入启动/create/dispatch/destroy/finalize 卡死、引擎 panic、连接 worker panic、总停机超时。测试使用更短的私有预算，要求到达指定故障的 marker、3 秒内退出，以及固定 Rust/MSVC 的 fast-fail 状态 `0xc0000409`；任意非零测试退出不能伪装成成功。fixture 环境变量仅在测试函数读取，生产没有故障开关。总停机 fixture 验证 watchdog 本身，不声称已实测 1024 个慢原生 session 回收。

fake/真实词库 smoke 都默认运行 `broker-fault-harness.ps1`：每架构实际终止并重启自己的 Broker 两次。第一次保留等待写锁的鼠标动作，确认进程确已退出后才放行；只清除预编辑、保留先前三轮 commit。第二次在已经提交且没有预编辑时退出。缺席期间 test/key 一致 fail-open，恢复后各提交一次，额外 Space 不重放旧词；最终 EDIT 与 TSF context 都核对五轮词。协调等待泵消息，但停机前仍延迟全部文本锁，避免其他 layout 锁抢先执行待测动作。

两脚本均支持 `-FaultRepetitions 1..100`，失败立即终止，不通过自动重试掩盖。真实词库每轮复制已部署的 build 到独立可回收目录。harness 只终止自己启动的 Process 对象，不发现/终止外部 Broker，不修改注册、启用或默认输入法。全部 owned child/事件独立清理，日志异步排空，错误与输出等待有界。

## 限制

- 协调停机 API 已具备，但安装模式尚未接入服务控制、托盘退出或安装更新调度。harness 重启不等于已实现生产自动拉起/守护进程。
- 故障证据来自受控文本存储，不是注册后的 Notepad/WinUI/浏览器矩阵。宿主拒绝清除预编辑或主动终止 composition 的更多边界仍待验收。
- 尚未注入 engine commit 已发生、回复送出但 TSF 写入前退出，以及部分文档写入后的故障；不声称跨崩溃 exactly-once，也不保证歧义输入不丢失。
- 极端驱动不完成取消、全池恶意占用、空闲租约、运行时签名/ACL/reparse、资源与用户数据崩溃一致性、正式安装发行仍未验收。
- 高频真实词库重复运行出现过首个 N 未消费，探针立即失败。其中一次记录为 63 ms，发生在既有焦点恢复回归、实际故障注入前；说明问题不局限于 Broker 重启。与 50 ms 请求预算相符，但尚未定位引擎执行、磁盘加载或调度的具体占比。不能据后续正常通过宣称冷启动/压力稳定性；不放宽 deadline，也不自动重发失败按键。

内核等待依据 Microsoft [WaitForMultipleObjects](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitformultipleobjects)；取消必须排空的依据为 [CancelIoEx](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex)。
