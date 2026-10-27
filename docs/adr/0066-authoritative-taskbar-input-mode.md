# ADR 0066：任务栏中英文模式来自引擎确认

日期：2026-10-27。状态：实现；安装态任务栏验收待完成。

## 问题

原有 `EngineStatus.ascii_mode` 未传递到 TIP；TIP 也未注册语言栏模式项。
输入方案的品牌图标是静态资源，不能表示同一中文输入方案内的中英文切换。

## 决策

1. 增加协商能力 `FEATURE_INPUT_MODE = 1 << 10`，依赖已有 CompositionPositions。
   新消息 `ModeSnapshot = 21` 在原 CompositionSnapshot 前增加两个规范布尔字节：
   ascii_mode、disabled。旧消息及字节保持原样；协商容量不足时不启用能力。
2. `InputMode = 22` 为 little-endian u64 revision 加 u8 action，共九字节。
   Inspect=0 只接受 revision=0；Chinese=1 / English=2 只接受非零的当前 revision。
   Broker 独立拒绝陈旧 revision、跨连接 token 和组词中变更。拒绝不清空候选或预编辑。
3. 新会话和设置替换会话后，通过 `EngineCommand::Inspect` 获取真实状态，
   不发送伪造按键；点击变更仅允许 `SetOption("ascii_mode")`。Engine Actor 仍是 librime
   唯一调用者；每次按键与候选动作都携带最新模式。请求仍使用原来的整体 50/400 ms 预算。
   原生 GetStatus 未返回结果时标记为不可用；后续有效快照再恢复，不能使用默认 false 冒充中文。
4. TIP 注册 `ITfLangBarItemButton`，采用 Windows 要求的 `GUID_LBI_INPUTMODE`。
   中文显示“中”，英文显示“英”，状态缺失显示“?”，引擎禁用显示“–”。
   通过 `ITfLangBarItemSink::OnUpdate` 通知图标、文字、tooltip 和可用状态更新。
   空闲点击可切换；组词、待应用的已消费按键及终止处理中禁止点击变更。
   不依据 Shift 的物理按下猜测状态；真实引擎 Shift 抬起快照决定显示结果。
5. 图标按系统小图标尺寸生成单色 AND/XOR mask，采用 TEXTCOLORICON 由系统着色。
   中英文 tooltip 集中在原生双语言资源表中，使用有类型资源键。返回 HICON/BSTR 的所有权
   交给调用方；COM item/sink 固定引用并处理重入卸载，先移出成员再通知及移除项目。
6. 注册器增加 `GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT`，核查 HKLM 原生视图的双索引。
   不声明尚未支持的 Immersive/SecureMode 能力。机器安装、移除和回滚一起管理该类别。
   回滚内容升级为 transaction-v2（profile/category/systray 三位），文件路径保持原样；
   严格读取旧 transaction-v1 并将其 systray 还原为 false，拒绝未知版本和额外内容。

## 验证与边界

- Rust 独立 wire golden、规范布尔/截断/额外字节/动作与 revision 矛盾；旧 payload 不变。
- Broker 协商依赖/最小容量、状态变更、陈旧 revision、错误 token、组词拒绝和保全。
- 双架构原生 COM 注册/移除、sink 通知重试与 cookie、重入 Deactivate、HICON/BSTR 所有权；
  七种尺寸的单色 mask、图形差异、中英文 tooltip，以及卸载引用计数。
- Fake 和真实 Rime 的 C++ client 状态查询/点击、ASCII 直通、中文组词、会话替换/断线重连；
  真实 TSF 编辑锁夹具检查点击和 Rime Shift 切换后的 item 文字。
- 机器事务合成自测覆盖三状态位全部组合，以及旧 v1 的四组合；不注册主机真实 Mo profile。
- 实际 Win10 任务栏显示、主题着色/高对比度、Explorer 生命周期和真实鼠标/键盘路由
  仍需在配套新包安装后验收。原生图形样张不等同于真实任务栏截图。

## 官方依据

- [TF_LANGBARITEMINFO：Windows 8+ 使用 GUID_LBI_INPUTMODE](https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/ns-ctfutb-tf_langbariteminfo)
- [GetIcon：系统尺寸、单色图标和调用者释放责任](https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/nf-ctfutb-itflangbaritembutton-geticon)
- [语言栏更新标志](https://learn.microsoft.com/en-us/windows/win32/tsf/tf-lbi--constants)
- [预定义类别：SYSTRAYSUPPORT](https://learn.microsoft.com/en-us/windows/win32/tsf/predefined-category-values)
- [Microsoft SampleIME 注册类别实现](https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/IME/cpp/SampleIME/Register.cpp)
