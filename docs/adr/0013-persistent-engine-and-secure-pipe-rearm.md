# ADR 0013：常驻引擎与安全 Pipe 重建

- 状态：接受
- 日期：2026-09-15

## 背景

最初的 Broker 在一个客户端断开后随即退出，并由该连接直接拥有 Engine Actor。这不符合桌面输入法的常驻生命周期，也使不同连接的 revision 失去全局顺序。直接把 Named Pipe 改成多实例又会遇到安全冲突：Windows 创建同名后续实例要求 `FILE_CREATE_PIPE_INSTANCE`，而 Mo 的受保护 DACL 正是刻意不向同一登录会话授予此权限，以阻止普通同用户进程伪造 Broker 实例。当前 TIP 尚未反向认证服务端身份，不能安全放宽该权限。

## 决策

- librime backend 由工厂闭包在 `mo-engine` 专用线程内构造、使用和销毁；即使 backend 是 `!Send + !Sync`，也不会跨线程移动。
- 每个 Broker 连接继续独立拥有握手、request id、generation 和 wire session token 映射；引擎操作通过同步命令通道发送到唯一 Engine Actor，revision 因而跨连接保持全局递增。
- Broker 不再在正常客户端断开后退出。它关闭已认证流，然后使用原有受保护 DACL 和 `FILE_FLAG_FIRST_PIPE_INSTANCE` 重建监听实例。
- 客户端连接器在总 deadline 内处理监听实例重建的短暂 `FILE_NOT_FOUND`/`PIPE_BUSY` 窗口，不能无限等待。
- 已认证连接一旦出现首字节，必须在固定 assembly deadline 内形成完整有界帧；半帧/slowloris 输入会断开并触发安全重建。完全空闲的正常 TIP 连接暂不因该 deadline 被误杀。
- 当前仍不启用同名多实例并发；先设计 TIP 对 Broker PID、受信安装路径/签名及进程令牌的反向认证，再评估授予创建后续实例权限。不得为了并发回退已经验证的 DACL 边界。

## 结果

Broker 与 librime 可跨连续宿主连接常驻，Engine Actor 的顺序语义不再局限于单连接。两个真实 Named Pipe 连接的测试证明监听实例可安全重建、会话完全回收且第二个 snapshot revision 大于第一个；负向测试证明同登录会话无法创建第二服务端实例，半帧测试证明 assembly deadline 生效。多个长期存活宿主同时输入仍未解决：单实例繁忙时客户端只会在自己的硬 deadline 内等待或 fail-open。
