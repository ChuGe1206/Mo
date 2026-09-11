# ADR 0005：Windows Named Pipe 安全边界

- 状态：Accepted
- 日期：2026-09-11

## Context

TSF DLL 被加载进不同位数、不同完整性级别的宿主进程，不能把任意本机 pipe 客户端直接视为可信。Windows Named Pipe 的默认安全描述符会给 Everyone 和 Anonymous 读权限，也不能阻止远程客户端。Mo 还需要避免未认证客户端通过半包长期占住 Broker。

## Decision

- 管道地址固定在 `\\.\pipe\LOCAL\Mo.Input.*`，endpoint 只允许有限 ASCII 字符，不接受调用方提供路径。
- Broker 从自身 token 提取带 `SE_GROUP_LOGON_ID` 的登录 SID，并以 protected DACL 只授权该 SID。
- 客户端权限使用具体的文件数据/属性/同步位，不授予与 `FILE_APPEND_DATA` 同值的 `FILE_CREATE_PIPE_INSTANCE`。
- 服务端设置 `PIPE_REJECT_REMOTE_CLIENTS`；客户端使用 `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION`。
- 服务端先等待一个受 64 KiB 协议上限和硬 deadline 约束的完整首帧，再模拟 pipe 客户端、读取线程 token 并核对登录 SID。认证失败时不派发任何请求。
- `RevertToSelf` 失败时立即中止 Broker，绝不继续在客户端安全上下文中运行。
- Phase 0 首版为单 pipe 实例、同步 I/O；并发 listener pool、overlapped deadline 和 AppContainer 兼容性需要独立实测后才能宣称完成。

## Consequences

- 默认 ACL 的宽权限不会进入产品实现，跨登录会话和远程客户端在内核边界被拒绝。
- Win32 `unsafe` 代码集中在 `mo-windows-pipe`，领域、引擎与 Broker 状态机继续保持 safe Rust。
- 同一登录会话内的恶意进程仍属于剩余威胁；代码签名、Broker 身份验证、进程完整性级别和安装路径保护需要在后续威胁模型中闭环。
- 当前实现不能证明 AppContainer 宿主可连接，也不能支撑多个并发 TSF 客户端，因此还不是最终生产传输。
