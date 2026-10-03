# ADR 0010：TSF 按键决策缓存与 Edit Session 上屏

- 状态：Accepted
- 日期：2026-09-12

## Context

TSF 会先调用 `OnTestKeyDown/OnTestKeyUp` 判断按键是否由输入法处理，再调用对应的 `OnKeyDown/OnKeyUp` 执行操作。若两次回调都独立发送到 Broker，同一个物理按键会推进引擎两次，产生重复预编辑或重复上屏。另一方面，TIP 不能直接修改宿主文本；Windows 要求通过 `ITfContext::RequestEditSession` 获得 edit cookie，再用 `ITfRange` 或 `ITfInsertAtSelection` 修改内容。

## Decision

- TIP 按 `(ITfContext*, virtual key, LPARAM 除低 16 位 repeat count 外的全部位, key direction)` 缓存最近一次 Broker snapshot（Win10 实测正规化见 ADR 0054）。test 回调负责首次有界派发，匹配的 key 回调只消费缓存；不匹配时允许 key 回调自行派发，以兼容不先调用 test 的宿主。
- handled snapshot 使用 `TF_ES_SYNC | TF_ES_READWRITE` 请求同步 Edit Session。Windows 文档明确允许 key event handler 请求同步读写会话；所有宿主修改只发生在 `DoEditSession` 内。
- Broker 的 UTF-8 commit/composition 使用 `MB_ERR_INVALID_CHARS` 严格转为 UTF-16。预编辑首次通过 `ITfInsertAtSelection::InsertTextAtSelection` 建立 Range，再尝试 `ITfContextComposition::StartComposition`；后续更新在同一 Range 上执行 `ITfRange::SetText`。
- 正式 Composition 成功时，提交或清除后调用 `ITfComposition::EndComposition`。若受限或旧宿主拒绝创建 Composition，TIP 保留同一活动 Range 作为降级路径，使预编辑仍可更新、提交和撤销。
- Broker/协议/Edit Session 在修改文档前失败时返回 `eaten = FALSE`；一旦文档已经改变，即使后续结束组合失败也保持吞键，防止宿主再次插入同一按键。歧义错误会断开 Broker，会话随后通过 250 ms 节流重建。
- 失焦和停用会清除活动预编辑、结束可用的 Composition、关闭 Broker session 并清空按键缓存。
- 原生 probe 提供一个真实 TSF Context，其 context owner 是由 Windows EDIT 控件承载的最小 `ITextStoreACP`。x64/Win32 均验证 test/key 决策一致、Edit Session 锁正确、最终文本只提交一次；真实 rime smoke 额外验证 `NIHAO + Space -> 你好`。

## Consequences

- 同一个按键只产生一次引擎状态变更，commit 有明确的“至多一次写入当前宿主文档”边界。
- TIP 热路径目前仍包含最长 50 ms 的同步 Broker 往返；已存在管道的首次激活握手预算为 400 ms，管道不存在则立即 fail-open。这是 Phase 0 为先验证正确性接受的技术债；候选窗阶段需评估异步预热、超时遥测与更严格的延迟预算。
- 受控 `ITextStoreACP` probe 可以在不污染系统注册表和输入法列表的情况下重复运行，但不能证明系统注册、Notepad/WinUI 的宿主行为或候选 UI；这些仍是 G2 的后续硬验收项。
- 参考的 Windows 契约：[RequestEditSession](https://learn.microsoft.com/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession)、[InsertTextAtSelection](https://learn.microsoft.com/windows/win32/api/msctf/nf-msctf-itfinsertatselection-inserttextatselection)、[SetText](https://learn.microsoft.com/windows/win32/api/msctf/nf-msctf-itfrange-settext)、[StartComposition](https://learn.microsoft.com/windows/win32/api/msctf/nf-msctf-itfcontextcomposition-startcomposition)、[EndComposition](https://learn.microsoft.com/windows/win32/api/msctf/nf-msctf-itfcomposition-endcomposition)。
