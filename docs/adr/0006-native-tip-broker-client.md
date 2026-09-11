# ADR 0006：原生 TIP Broker 客户端与 fail-open 边界

- 状态：Accepted
- 日期：2026-09-12

## Context

TSF 把 DLL 加载进任意文本宿主。Broker 未启动、响应超时或协议损坏时，Mo 不能冻结宿主，也不能吞掉用户按键。`OnTestKeyDown/Up` 与 `OnKeyDown/Up` 还可能针对同一输入事件成对调用；在没有 edit session 和事件决策缓存前，从测试回调直接推进引擎会造成重复处理或状态与上屏结果分离。

## Decision

- C++ TIP 只持有一个极薄的 `BrokerClient`；协议状态、帧校验与 Named Pipe I/O 不进入 COM 对象主体。
- 客户端固定连接 `\\.\pipe\LOCAL\Mo.Input.Broker.v1`，请求精确权限并使用 identification-only SQOS，不接受调用方覆盖 endpoint。
- 连接、读写和关闭均使用 overlapped I/O 与端到端硬 deadline；超时、部分结果、request id 错配、CRC/UTF-8/边界错误都会关闭句柄并废弃会话。
- `ActivateEx` 在 key sink 成功挂载后，仅用 50 ms 做 best-effort Broker 握手与 `OpenSession`。Broker 不可用不导致 TIP 激活失败；`Deactivate` 先解除 sink，再限时关闭会话。
- 在 edit session、测试/执行回调决策缓存和幂等提交闭环前，四个 key 回调一致 fail-open，不向 Broker 发送按键。
- x64 与 Win32 客户端以独立 probe 对同一个 x64 Rust Broker 做跨进程协议冒烟；probe 不注册或启用 TIP。

## Consequences

- Broker 故障不会把宿主锁死；不明确的 I/O 结果也不会被误当成可重试提交。
- 两种客户端位数已证明共享同一 wire protocol，而不是依赖进程内 ABI。
- 目前只有传输垂直切片，没有用户可见输入；真实 TSF 宿主、edit session、composition/candidate UI 与故障恢复仍是硬验收门。
- C++ 与 Rust 各自维护 wire 常量，后续必须以兼容性 golden vectors 或生成代码防止漂移。
- 受保护的 pipe 和会话 SID 校验用于服务端认证客户端；客户端验证 Broker 身份、安装目录保护和签名链仍需在威胁模型中完成。
