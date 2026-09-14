# ADR 0014：宿主终止 Composition 的所有权收口

- 状态：接受
- 日期：2026-09-15

## 背景

TSF 宿主可以在 TIP 主动提交或清空之前终止 composition，例如焦点、selection 或文档状态发生变化。若 TIP 只保存 `ITfComposition`/`ITfRange` 而不实现终止回调，后续按键可能继续操作已经失效的 range，造成断连、残留预编辑或重复上屏。

## 决策

- `TextService` 实现并暴露 `ITfCompositionSink`，创建正式 composition 时将自身作为 sink 传给 `StartComposition`。
- `OnCompositionTerminated` 只在回调对象等于当前 owned composition 时清除 composition、range 和 context；未知对象不影响当前状态，空指针明确返回 `E_INVALIDARG`。
- 主动提交/清空先把 owned COM 引用复制到局部变量，再清除成员状态，最后调用 `EndComposition`。这样同步回调不会在仍执行成员调用时释放最后一个引用，也不会二次清理旧状态。
- 受控 x64/Win32 探针连续执行两轮 composition/commit，并核对最终文本为 `mm`；这覆盖结束后立即重新开始 composition 的生命周期。真实 rime-ice smoke 同样期望连续两次 `你好`。

## 结果

composition 的宿主终止与 TIP 主动终止现在汇合到同一清理不变量，避免真实 Notepad/WinUI 生命周期中的悬挂 range。该证据仍来自受控 TSF text store；注册宿主矩阵尚需执行。
