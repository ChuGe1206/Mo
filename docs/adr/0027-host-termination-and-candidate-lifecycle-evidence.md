# ADR 0027：宿主终止清理与候选生命周期证据

- 日期：2026-09-18
- 状态：宿主终止缺陷已实现修复；历史偶发问题不据此全部关闭
- 延续：ADR 0022、0023、0026；保持 50ms 单一按键传输预算，不注册系统 TIP。

## 修复边界

`OnCompositionTerminated` 原来先丢弃 active range，再断开引擎。宿主已
终止 formal composition，但文档仍可能保留未提交拼音，后续焦点取消
又失去了清理范围。受控文本存储通过真实
`ITfContextOwnerCompositionServices::TerminateComposition` 稳定复现。

现在只在传入 composition 与 Mo 当前 composition 匹配时，使用通知
提供的写 cookie 对 Mo 自己的 live range 写入空串，然后释放组合状态。
在任何 Win32/文本调用前固定范围，终止期间使缓存按键失效，重入的
TestKey/HandleKey fail-open，鼠标动作和旧候选 identity 拒绝执行。
清理前后重验 composition/range，避免覆盖已被宿主替换的状态。
不请求嵌套锁，不再次调用 EndComposition，不提交当前候选、不重放输入，
也不额外等待 CloseSession 应答。已提交前缀不在该范围内。
清理失败仍使候选身份与连接失效，并返回实际 HRESULT；不宣称宿主拒绝
写入时也能强制删除文本。无 formal composition 的 fallback 仍走原有路径。

这是现有失焦/故障取消策略的补全：宿主强制终止时丢弃 Mo 未提交输入，
而不是把拼音当成正式上屏文字。用户主动提交仍按正常快照处理。

回归包含普通 owner termination 和等待写锁的鼠标 action 被终止两个
场景；清空范围的文本存储写回调还实际重入 TestKey/HandleKey，要求
`fired=1 fail_open=1`。都检查候选隐藏、完整 committed prefix、旧 action 不上屏，以及
新的输入可恢复且只提交一次。恢复后的期望文本显式累计，不自动接受
额外 commit。现有布局重入、翻页、失焦和两次 Broker 退出回归保留。

## 诊断和测试隔离

每个候选显示/隐藏等待都有固定生命周期 checkpoint。default TIP 不
提供诊断接口，但探针失败仍报告 checkpoint、expected visibility 和
候选/owner 可见性，且记录失败前有界的 65,536 字符探针输出。
失败信息先进入重定向日志，再以终止错误返回；失败不自动重试。
非可见性断言也报告同一状态：松键/鼠标检查分别记录 Pump 前后文本
是否变化，不记录文本本身，避免短路条件丢失 reset/transport 证据。

trace 构建额外区分 DispatchKey 总时长、发送前准备、连接与 modifier
读取时长，以及终止时的 owner active/foreground 状态和 sent-message
状态。只记录元数据，不增加文本/按键值/token 日志。计时结构扩展后
更换 development-only IID 为 `B37A59F4-8F18-4D58-90D2-37A516D04197`，
旧 IID 被拒绝，避免新 TIP 覆盖旧探针的较小 buffer。默认构建仍拒绝
新旧两种诊断 IID。trace 产物不是发行物。

受控探针原先直接创建 WS_VISIBLE 的 off-screen EDIT popup，可能激活
自己的窗口队列。单独加 WS_EX_NOACTIVATE 的实验仍实测
`created_foreground=0 created_active=1`，不能当作隔离通过。
现在先创建不可见窗口，再显式 SW_SHOWNOACTIVATE，同时验证 foreground
和 active 都不是该窗口。直接 key sink 的模型焦点仍显式设置；注册
系统路由保持原有真实宿主显示方式，不能用非激活模型代替其验收。

`tools/candidate-lifecycle-probe.ps1` 从当前源码全新构建双架构前端与
匹配探针、隔离 target-dir 的 debug Broker。它只借用已验证的 stage
runtime/data，在新的 build 子目录复制成 installed-like 兄弟布局，
不替换 stage release Broker、不关闭外来进程、不修改 profile/default。
只清理自己精确验证过且无 reparse 的 fixture，保留构建与日志。
`-Fake` 可区分真实引擎与宿主状态机；`-LatencyTrace` 明确开启元数据。
`-ActivatingTestHost` 仅保留旧模型的显式比较入口，可能激活测试窗口，
默认关闭。不把不同测试宿主模型的样本混合成同一压力结论。

入口在创建输出前验证已安装 Rust 1.97.1，禁止自动安装。18 项安全
检查覆盖相对/缺失 stage、输出越界/复用、参数、缺失 compiler、未创建
输出与 stage 未变，以及缺失/0/1/空的 rustup 环境偏好恢复。初次测试
暴露 .NET SetEnvironmentVariable(null) 在本机留下空值而非删除变量；
现对缺失偏好显式 Remove-Item Env:，修正后 18 项通过。只影响当前
脚本进程，未改用户/机器环境。负向与修正日志分别为
`build/mo-candidate-lifecycle-policy.log` / `...-policy-final.log`。

## 保留的负向证据

1. 未改 production TIP 的第一批 trace 完整回归 x64 25 轮通过，第 26
   轮失败，未进入 Win32。检查点为第二次 `initial-composition`，
   `expected=1 stage=12 count=0 focus=1 snapshot=0 reset=1 resets=4`。
   11 次按键传输都成功，最后 request 13 为 `error=0 total_us=2027`；
   Actor 相应 dispatch 约 1775µs、queue 48µs。说明该样本在成功按键后
   收到匹配的 TSF composition termination，**不是传输超时**。是谁
   触发终止尚未证实，不能直接归因于用户切窗、资源搬迁或词库。
   构建/成功轮记录 `build/mo-candidate-lifecycle-trace-4.log`；当时旧
   harness 的终止错误只出现在命令输出，具体失败元数据在此保留。
2. 新普通 owner termination 回归对修复前 TIP 第一次执行即失败：
   `result=0 prefix_intact=0`。原 x64 TIP SHA-256 为
   `E3042E307697F94F521AB482D1332DEA06A68DBB21AF2F90E21F0CDCD456138D`，
   构建记录 `build/mo-candidate-owner-before.log`。这是确定性残留
   缺陷，不以重跑替代修复。
3. 第一批修复后旧激活宿主模型 x64 8 轮完整通过；第 9 轮 owner
   termination 回归已通过，但第一次故障重启后的恢复输入失败，未进
   Win32。记录 `error=1460 total_us=62`，测试回调 elapsed 843ms。
   当前 trace 尚不能拆分该时长，保留
   `build/mo-candidate-owner-after.log`。不能只看 62µs 宣称回调未超时，
   也不能把终止清理修复当成该尾延迟风险的解决。
4. 非激活宿主首次实验在模型开始前即因 active queue 验证失败而拒绝，
   `build/mo-candidate-lifecycle-noactivate-trace-2.log`。随后改为显式
   SW_SHOWNOACTIVATE；这项测试隔离修复不是证明第 1 项终止原因的
   因果实验，不能抹去 ADR 0022/0023/0026 的历史负向样本。
5. 加终止重入保护后的真实 trace 批次 x64 20/20、Win32 9/20 完整
   通过，Win32 第 10 轮在 stop cycle 0 前出现 key-up/stale press
   检查失败，`build/mo-candidate-termination-reentry-trace.log` 保留。
   该次 created_foreground/active 均为 0，但具体短路子项未记录。
   不能宣称非激活模型或终止清理消除了全部偶发问题，也不能凭最后
   request 13 的成功时长判断未记录的松键请求一定成功。最终探针
   为该分支补齐 result/eaten/Pump 前后未变标志及统一状态输出。
6. 最终统一诊断探针的真实 trace 批次 x64 20/20、Win32 1/20 完整
   通过，Win32 第 2 轮在 stop cycle 0 前的第二次 initial composition
   失败，记录 `build/mo-candidate-invariants-final-trace.log`。再次确认
   `stage=12 reset=1 result=0 focus=1 snapshot=0`；最后 request 13
   传输 3194µs、dispatch 3614µs、prepare 6µs，终止时 owner
   active/foreground/sent-message 三项都是 0。因此单纯的 owner 激活
   不是充分解释，不能称非激活测试隔离关闭了该缺陷。仍未定位 TSF
   内部终止来源，尤其不能忽略通知或重显示已终止组合来取得绿色。

## 确定性重入负向证明

`tools/test-termination-reentry-regression.ps1` 把最终写重入探针与
已构建的前一版 default TIP 配对（不替换原 stage）。两架构都在实际
写回调中出现 `MO_TERMINATION_REENTRY fired=1 fail_open=0`，验证旧版
确实错误接受重入输入；任意 UI/超时失败不算该回归的成功证据。
旧 x64/x86 TIP SHA-256 分别为
`2F50AF4ADF76B4769BCE3A8FCB309E37DA87F1FB1557444A81099D1926D47615` /
`DDB7D75D9A1A2549591A9EFD683DA0D7E2D2B556BD39DD83E4816A3CD7ED4BF4`。
完整记录 `build/mo-candidate-reentry-before-guard.log`，不涉及真实用户
文本、实际引擎或系统注册。

## 分组验证记账

在加入写重入注入前，非激活真实 trace、实际素材 default、独立 fake
三组各 x64/Win32 20/20 完整通过，分别为
`mo-candidate-lifecycle-noactivate-trace-3.log`、
`mo-stage-owner-termination-runtime.log`、
`mo-candidate-lifecycle-default-fake.log`（均在 build 下）。每组 80 次
owner termination / 80 次明确 Broker 退出，不能混入激活模型负向组
或解释为最终扩大后的矩阵全通过。中间 stage
`mo-windows-stage-owner-termination` 的清单为
`613A665045048908E4353225A953075EE1F5E958D3E5F619E3F8789B21944002`，
68-source 对应、79 项 stage policy 和 7 组 prebuilt-only golden 通过。
该中间版本不含最终重入保护，保留作确定性负向对照，不是最终素材。

最终 native source 与素材在 `build/mo-windows-stage-candidate-final`：
68 份 workspace/snapshot 哈希对应、137-file 清单与六次 strict native
rebuild/default ABI 通过。清单 SHA-256 为
`0CE4054EBFD6329CD12C4545C176ABE200DE3B80C37B7664C618DE7E4DE06109`。
DLL 仍是 ADR 0025 verified v2 core+Lua，不更新源码/词库/转换资源。
旧 trace 探针对最终新 IID 的 x64/Win32 兼容性负向均在读取 buffer 前
以 E_NOINTERFACE 拒绝，日志 `mo-candidate-retired-abi-{x64,Win32}.log`。
Rust default 109+compile-fail、default all-targets Clippy/fmt 与四份脚本
AST 通过。CI 已接入安全入口与 trace 命令，但未推送/远端运行。

最终同素材实际 default 全命令 x64/Win32 各 20/20，通过 7 组
prebuilt-only golden、preparation、Actor、布局/焦点与两次真实故障
恢复；日志 `build/mo-stage-candidate-final-runtime.log`。每轮两个
owner termination 都实际重入写回调且为 `fired=1 fail_open=1`，共
80 项终止/重入、40 个完整故障 rounds / 80 次明确 Broker 退出。
独立最终 source default/fake x64/Win32 各 20/20，同样有 80 项
终止/重入及 80 次退出，`build/mo-candidate-final-default-fake.log`。
这两组不抵消第 6 项 final trace 压力失败，不混称所有模式通过。
最终 79 项素材与 18 项入口策略通过（`mo-stage-candidate-final-policy.log`
/ `mo-candidate-final-entry-policy.log`）；Rust trace 111+compile-fail
也再次通过。最终 registrar 只读状态仍是双视图 COM missing、profile
registered/enabled/active 全 false，未更改默认输入法。

## 验收限制

仍需更广压力与尾延迟定位、管理员明确准备后的双架构注册路由、
Notepad/WinUI/AppContainer/混合 DPI 等普通宿主验证，以及正式
bootstrap、Broker 生命周期、配置与资源信任边界和安装事务。
本阶段修复确定性宿主终止残留，不等于候选所有偶发问题已关闭，
不升级为日常使用、正式安装或发行通过。

API 依据：[OnCompositionTerminated](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcompositionsink-oncompositionterminated)、
[owner TerminateComposition](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextownercompositionservices-terminatecomposition)、
[RequestEditSession](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession)、
[CreateWindowExW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw)。
