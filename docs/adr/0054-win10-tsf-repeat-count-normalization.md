# ADR 0054：Win10 TSF 回调重复计数正规化

- 状态：Accepted
- 日期：2026-10-03

## Context

Win10 22H2 x64 记事本、安装态 0.0.10.0 首次真实桌面验证发现：单次按键被引擎推进两次。独立诊断副本仅输出回调种类与缓存匹配布尔值，不输出按键值、文本或指针；同一测试/正式回调的 context、virtual-key、scan/flags 相同，LPARAM 低 16 位 repeat count 不同，原来的完整 LPARAM 比较失配。该实测也说明先前受控探针每次原样复用 LPARAM 未覆盖真实宿主差异。

Windows [KeyDown 契约](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeystrokemgr-keydown) 描述 LPARAM 包含 repeat count、scan code 和状态 flags；官方常规消息循环示例并不保证所有 legacy 宿主都原样传递低字。这里的差异以 VM 诊断为依据，不推广为 Windows 全部宿主行为。

## Decision

- ADR 0010 的缓存身份继续保留 context、virtual-key、方向以及 LPARAM 除低 16 位外的全部位。仅忽略 repeat count；scan code、extended/context、bit 30 的真实 auto-repeat 和 transition 等位继续严格比较。
- Broker 的 KeyEvent 已以 scan code/flags/IsRepeat 投影输入，不使用低字计数。本次不改引擎语义、不扩大缓存生存期、不增加重发或吞键策略，也不调整 50 ms deadline。
- probe 的 SendTestedKey 在匹配 test/key 回调中改写低字计数，并在正式回调前追加 0/1/2 三组重复查询；既有最终 EDIT/TSF 文本、候选、通知、终止和故障恢复断言应保持。
- 日志型诊断只存在忽略的 build 副本。VM 临时替换的原 DLL 已按哈希恢复，并重新核验完整安装 payload、权限和机器 COM；本次修复尚未进入 linked 安装包。

## Evidence and limits

- 同一 fake Broker 与新探针对照：旧 x64 DLL exit 1（重复查询改变待处理决策），修复默认 DLL exit 0。
- x64/Win32 严格默认编译和 fake IPC/pool/TIP/edit/鼠标/终止/双次 Broker 故障恢复全部通过。
- 同源码默认 DLL 的 Win10 x64 记事本有限功能验证：单键不重复；两次提交的 EDIT 精确回读通过；设置 UI 保存繁体/深色、候选窗深色与繁体提交精确回读通过。这里不声称首次切换、冷启动、压力或全宿主稳定。
- 完整真实词库回归两次 default 和一次 trace 均在 x64 TIP 首键失败；trace 是等待 Broker 响应约 50.2 ms 超时，缺少该 dispatch 的 Actor 完成计时，不能据此断言根因在引擎。三次原始负向日志保留，Win32 后续阶段未执行。不能将 fake/VM 有限成功改写成完整真实词库通过。
- 详见 [Win10 桌面实测](../phase-0/WIN10-DESKTOP-EVIDENCE.md)。G2/G3 不升级为整体通过。

## 后续打包与诊断补证（2026-10-03）

修复已进入 0.0.11.0 unsigned DevelopmentTest 包，MSI ICE、Win10 变更载荷升级 exit 0、一次重启后状态、安装态 x64/x86 记事本及 x86 写字板精确回读有限通过。原“尚未打包”描述指前轮临时 DLL 验证阶段。隔离 trace 保留首键失败，延后收集取得 Actor 执行约 1.31/1.67 秒；直接探针将该样本主要长耗时缩到 native process_key，后续键也有 252 ms。具体组件与 CPU/I/O 原因仍未知；50 ms deadline 不变，G2/G3 未整体通过。详见 [新包续测证据](../phase-0/WIN10-PACKAGED-0110-EVIDENCE.md)。
