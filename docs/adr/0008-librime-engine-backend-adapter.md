# ADR 0008：librime EngineBackend 适配边界

- 状态：Accepted
- 日期：2026-09-12

## Context

安全 FFI 层原有的 `Session<'Engine>` 借用模型适合直接调用，但不能作为 `EngineActor::Session`：Actor 同时拥有 backend 与 backend session，若 session 借用 backend 内的 Engine 会形成 Rust 无法安全表达的自引用结构。与此同时，librime 的 `process_key` 接受 X11 keysym 形状的 keycode 与自定义 modifier mask，不能直接传 Windows virtual key 或 Mo 的 modifier bits。

## Decision

- Engine 保留对 librime 进程级生命周期的唯一所有权，并提供仅 crate 内可见、必须携带 session id 的受控操作。公开 RAII `Session` 与 `RimeBackend` 共用这些操作；原生 session id 不公开、不可由调用方构造。
- `RimeBackendSession` 只保存与所属 Engine 绑定的私有 id。Actor 的 create/apply/destroy 分别调用 librime create、process/snapshot、destroy；Engine 最终析构仍用 `cleanup_all_sessions` 兜底。
- 已归一化的特殊键直接使用 X11 keysym；文本事件的 Latin-1 标量直接映射，其他 Unicode 标量编码为 `0x01000000 | codepoint`。modifier 严格映射为 librime 的 Shift `1<<0`、Lock `1<<1`、Control `1<<2`、Alt/Mod1 `1<<3`、Super `1<<26` 与 Release `1<<30`，未知 Mo modifier bits 显式报错而不是丢弃。
- 每次命令后依次提取 unread commit、context 与 status，并投影为完全 owned 的 `EngineOutput`。composition 的 native byte length、cursor 与 selection 必须与复制后的 UTF-8 字符边界一致，否则整次命令返回错误。
- 当前承诺的 C ABI 前缀只到 `free_status`。需要后续 API slot 的 schema/options、选词与翻页命令显式返回 unsupported；扩展前缀时必须同步更新 C/Rust ABI probe，禁止静默忽略。
- Broker 仍默认使用 `FakeBackend`，直到 DLL 来源、插件允许列表、预编译 rime-ice 路径、部署完成标记和启动失败降级策略完成闭环。

## Consequences

- FFI 层不需要 unsafe 自引用或泄漏 Engine 引用，日常 RAII API 也不产生第二套 native 调用实现。
- Actor 已可替换为真实 librime 后端，且 backend 错误会沿既有 Broker 稳定错误边界返回。
- 原生输出异常不会携带不可信偏移进入 TSF UTF-16 edit session。
- 仍未证明发布版自构建 librime DLL 和预编译 rime-ice 资源可由 Broker 一键启动；这属于下一检查点而非本 ADR 的完成声明。

修饰位与 API 形状以锁定的 librime 1.17.0 源码为准：[rime/key_table.h](https://github.com/rime/librime/blob/33e78140250125871856cdc5b42ddc6a5fcd3cd4/src/rime/key_table.h) 与 [rime_api.h](https://github.com/rime/librime/blob/33e78140250125871856cdc5b42ddc6a5fcd3cd4/src/rime_api.h)。
