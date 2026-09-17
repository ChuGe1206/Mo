# ADR 0020：可取消的服务端 I/O 与整帧预算

- 状态：接受
- 日期：2026-09-17
- 演进：保留 ADR 0019 的固定受保护槽、原始 handle、连接身份与唯一 Actor；替代同步服务端读写及毫秒 Peek 轮询。

## 背景

固定连接槽限制了资源增长，但停止读取回复的客户端仍可能阻塞同步 WriteFile。更隐蔽的是管道 FlushFileBuffers 会等待客户端读空缓冲区；这不是输入响应需要的协议确认。仅限制首帧/组帧、却保留无限写/flush，不能保证槽能从慢客户端恢复。

## 决策

- 每槽仍是独立 first-instance 管道，server handle 增加 `FILE_FLAG_OVERLAPPED`。connect/read/write 都使用显式 OVERLAPPED 与本次操作专属的 unnamed manual-reset event，同一 stream 同时只有一个操作。监听等待客户端接入不计入首帧时限。
- 首帧从接入后开始，整个 header/payload/短读共用 2 秒绝对预算，并通过原 MOIP decoder 验证长度、版本、CRC；完整读取后才模拟并认证 logon SID。坏帧不进入 Broker 状态机。
- 已认证完全空闲连接保留不超时语义，以 pending 单字节 read 等待活动，不做每毫秒轮询。首字节返回后，借助 prefix reader 将它交回原 decoder，剩余 header/payload/短读共用 2 秒 assembly 预算。收到零碎字节不能续期；长度上限在分配 payload 前验证。
- Broker 使用 `write_frame_with_timeout`，回复的 header、payload 和所有 short writes 共用一个 2 秒绝对预算，而不是每次 write 重新计时。通用 stream Read/Write 仍有单操作默认预算，但生产帧入口使用整帧预算。
- flush 为 no-op：overlapped write 的内核完成已被等待，不再调用会阻塞对端消费的 FlushFileBuffers。这不声称对端已经消费或文档已经写入；请求号/响应及 TIP Edit Session 处理这些独立语义。
- pending 操作超时或等待失败时，只取消该 OVERLAPPED，随后最多等待 1 秒内核完成；直到完成才可释放 record/event/借用 buffer。若极端内核/驱动故障仍未完成，则进程 abort，不能释放仍被内核使用的栈存储，也不能无限挂起。当前服务端 Rust 模块不装入 C++ TIP 宿主；这一 fail-stop 发生在 Broker 或测试进程，不是用户编辑器。该极端分支尚未故障注入。
- 即使正常完成赢了取消竞争，过期操作仍返回错误并退出连接。生产不重试回复字节，不重新执行可能已经产生 commit 的引擎命令。BrokerConnection 析构回收连接所属引擎会话，accepted stream 断开后原 retained 槽可复用。
- 修复诊断 Rust PipeClient 在名称缺失/繁忙恰逢 deadline 到达时返回 NotFound 的竞态：这些重试条件耗尽总预算后统一返回 TimedOut。C++ 客户端的总时限与服务端身份复核不变。

## 验证

真实内核对象测试故意不读取回复，确认填满管道后确实进入超时、取消完成、DACL 不变且新客户端接入；另行验证 pending read 取消后没有遗留操作读走新客户端的数据、零预算没有输出字节、flush 在 peer 未消费时返回、分片 payload 不续期和空闲等待不消耗 assembly 预算。

共享 Actor 的 Broker 测试对截断与 CRC 损坏的 Space 请求核对退出错误、创建/销毁数量及实际 backend 命令日志。两次坏 Space 都未到达引擎，新连接从新预编辑开始，正常 Space 仅提交一次，后续 Space 不重放旧词。

x64/Win32 C++ fake 与真实锁定 librime/rime-ice 测试在既有候选授权隔离之后额外完成三轮整池满载/释放/重新连接，核对新预编辑、准确提交与不重放；候选窗/鼠标/异步编辑取消回归仍通过。测试不假设 Rime 完整预编辑无音节分隔，也不把无预编辑时原生的正常空格提交当成旧词重放。

## 尚未证明

这些是本 ADR 当时的传输/协议故障证据。后续 ADR 0021 已补齐 EngineClient/watchdog、总停机预算与受控 Broker 退出/重启回归；无法安全中断任意 librime C 调用，超时仍采用进程 fail-stop 而不是 native 取消。

目前是 overlapped 内核操作加固定线程等待，不是 IOCP 全异步调度。已认证空闲租约、全池恶意占用、Broker 在 engine commit 后/TSF 写入前的歧义注入、真实注册宿主和安装发行仍待验收。协调停机 API/工作线程 panic 后续见 ADR 0021，但生产退出控制尚未接入。尚不声称 exactly-once 跨崩溃提交。

依据 Microsoft 官方 [Overlapped Pipe Server](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-server-using-overlapped-i-o) 与 [CancelIoEx](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex)。
