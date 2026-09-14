# ADR 0007：Broker 到 Engine Actor 的会话路由

- 状态：Accepted
- 日期：2026-09-12

## Context

早期 Broker 为验证协议，在每个连接里另存 composition/revision 并直接做 ASCII echo。这会形成第二套输入状态机，无法复用 Engine Actor 的严格命令顺序和 generation 防 ABA 语义。IPC 只有一个连接内 opaque session token，而领域层的 `SessionToken` 由 slot 与 generation 组成，两者也不能直接互换。

## Decision

- `BrokerConnection<B>` 以 `EngineBackend` 泛型持有 `EngineActor<B>`；Phase 0 默认使用确定性的 `FakeBackend`，librime 将作为后续可替换后端接入。
- Broker 为每个 wire token 保存对应的领域 `SessionToken`。wire token 只负责连接内相关性，Actor token 负责引擎槽位生命周期和陈旧请求隔离。
- `OpenSession`、`KeyEvent`、`CloseSession` 分别调用 Actor 的 create、dispatch、destroy；连接释放时尽力销毁所有尚存 Actor session。
- snapshot 的 revision 取自 Actor 的全局命令顺序，composition、commit 和候选从 owned `EngineSnapshot` 投影到有硬上限的 IPC 格式。
- 当前 IPC 尚未携带布局解析后的 Unicode 文本。过渡适配器只为 ASCII 字母与数字推断文本，并把已知 Windows 导航/编辑键转换为 X11 keysym 形状的领域逻辑键码；完整 Unicode、死键与 AltGr 布局转换必须在真实 TSF 前端验证后落定。
- 最初的 Phase 0 Named Pipe 一次服务一个连接，所以 Actor 暂由连接直接持有；该过渡状态已由 ADR 0013 更新为进程级专用线程与命令通道，wire token 映射仍保留在各连接状态机内。

## Consequences

- Broker 不再有旁路 composition 状态，测试路径与未来 librime 路径共享同一 Actor 契约。
- 多 session 的 revision 是严格全局顺序，关闭和断开都能触发后端会话回收。
- 当前 FakeBackend 只用于确定性架构验证，不代表中文转换能力或最终按键语义。
- 多连接并发、完整 Unicode/死键/AltGr 键盘布局和后端故障恢复仍需后续硬验收。
