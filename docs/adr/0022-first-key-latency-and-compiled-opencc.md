# ADR 0022：首键诊断、共享资源保活与 Emoji 预编译

- 状态：接受实现边界，压力/普通宿主验收待完成
- 日期：2026-09-17
- 延续：ADR 0019/0020/0021；不扩大 IPC 权限、放宽按键预算或重试歧义输入。

## 问题与证据

旧压力测试首个 N 未消费并不是单纯的崩溃恢复错误。加入开发态分段计时后，Win32 第 39/100 轮失败：Actor 排队 9 µs、执行 56,588 µs；客户端等待 header 超时 53,330 µs，取消排空 21 µs。未读候选 payload，写请求仅 13 µs。因此这一次的主要耗时在引擎，不是队列或传输背压。

独立 `latency_probe` 在同一个进程连续创建/关闭 20 个会话：原始 rime-ice 首次 `process_key` 32–42 ms，clear 后再输入约 0.7–2.1 ms；context 约 1–2 ms。只在 disposable build 副本中移除 `simplifier@emoji` 后，首键约 0.8–2.2 ms。原 schema/shared/user 根均未修改，此消融不是产品默认。

锁定 librime 源码的 [Simplifier/Opencc 实现](https://github.com/rime/librime/blob/33e78140250125871856cdc5b42ddc6a5fcd3cd4/src/rime/gear/simplifier.cc) 在实际转换时延迟初始化，component 的共享 map 只持有 weak owner。最后一个会话消失后，下一个新会话重复加载。Emoji 配置读取 text dictionary，故首次过滤包含解析开销。

仅预编译 Emoji/补充词典改善了首键，但不足以宣称问题解决：x64 第 26/100 轮焦点恢复仍出现执行 60,667 µs、排队 19 µs、header 超时 61,880 µs。必须保留这条失败证据，不能据前 25 轮通过报压力通过。

## 实现

Broker 使用 `RimeBackend::with_resource_anchor`，在专属引擎线程、启动 watchdog 内创建一个额外的私有 native session。它只持有共享资源，永不 dispatch/commit/clear、没有 wire/Actor token、不能成为 frontend session。正常用户会话继续独立创建/销毁；没有复用旧状态、伪按键预热或用户词频学习。Backend Drop 先显式回收 anchor，Engine 随后 cleanup-all/finalize，继续受现有停机 watchdog 保护。

直接 API 对照显示：保活+预编译时第一轮真实转换仍约 27 ms；之后每个新会话首键约 0.7–1.7 ms，会话创建约 2–7 ms。保活减少重复加载，但**不预热首次转换**，也不承诺系统调度延迟上界。两个伪 API 测试核对不同 frontend id 的输入/销毁、anchor 不接收输入且最后回收，以及创建失败的 finalize 顺序。当前只支持固定默认 schema；支持 schema 切换前必须重新审查按 schema/配置的保活策略。

构建态工具以 hash 锁定 OpenCC 1.1.9 的完整 source ZIP 与 bundled Marisa，保留原源码/许可证，不自动联网，不替换已存在的输出。只把 `emoji.txt`（4857 条）和 `others.txt`（1498 条）转换为 `.ocd2`，读回后验证全部 key 和**有序** values。JSON 仅替换 dictionary type/file；segmentation、group 顺序和词条不变。pack 保存原始源文件与生成哈希。20 个脚本/编译器检查覆盖 Unicode 路径、多值顺序、篡改/缺失/额外路径、优先级变化、额外配置字段（即使重算 manifest hash 也拒绝）、相对路径、重复 key、拒绝覆盖及错误 archive。

真实 smoke 的 `-OpenccDataDir` 仅把校验后的固定三个输出拷入独立 fixture。共享源、原 build 和真实用户数据不被编辑。安装/签名资源流水线尚未接入此 pack；不据自声明哈希断言更新来源可信或 GPL 发行合规。

## 计时与隐私边界

Rust `latency-trace` 必须显式编译且仅 debug assertions 生效。Actor 记录 create/dispatch/destroy 的排队/执行 µs，用容量 256 的 `try_send` 送给独立 logger；满/断开丢弃，不在输入线程写 stderr，不 join 可能阻塞的 logger。logger 在 ready 行之后启动，早期记录也计入 dropped。默认和关闭 debug assertions 的构建没有诊断时钟或 logger，计时 envelope 的零大小由编译期断言约束。

原生 `MoLatencyTrace`/`MO_LATENCY_TRACE` 默认关闭。开启时提供只读 `IBrokerDiagnostics`，记录 write/header/payload/cancel、request id/kind/错误，以及候选定位/显示阶段和 HRESULT/数量/焦点布尔值；没有输入键、词语、路径或 session token。默认 QI 返回 E_NOINTERFACE；开发构建验证 null 参数、零初值、COM identity 与卸载。harness 始终异步排空输出，失败保留 metadata，不自动重试。

客户端原用 GetTickCount64：Microsoft 说明其典型精度 [10–16 ms](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount64)，实测还出现超过 51–54 ms 的成功。改为 MSVC `steady_clock`（QPC）的单一端到端 deadline，不改 50 ms。内核 wait 向上取整，但有限预算不转 INFINITE；提交前、完成后及校验/解码后检查到期，过期回复不发布 snapshot。始终取消排空 pending I/O，不重发。算术回归涵盖亚毫秒、恰好到期、过期和 MAXDWORD；真实 IPC 检查零预算拒绝与输出 sentinel 保持不变、新会话从空 composition 开始。

时钟依据：[Microsoft 高精度时间戳](https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps)。更精确地拒绝迟到结果，不等于线程绝不会晚于预算返回；操作系统调度与取消排空仍可能延长实际返回时间。

## 未通过边界

保活+预编译的第一轮高频命令完成 23 轮后，在 x64 第 24/100 轮发现候选窗未显示；该轮按键正常返回，首个 dispatch 43,752 µs。不能把这条独立候选表现层风险解释为首键超时，也不能抹去失败后报告 100 轮通过。显示阶段诊断已增加，后续对照另行记录。

开启显示诊断后，另一次 x64 第 3/100 轮复现：`stage=11 result=0 count=0 focus=1`；首键 Actor 42,428 µs，后续 `nihao` 按键正常。stage 11 表示最后一次 Update 成功，但当前 snapshot 已为空；**尚未确认其清理来源**，不把宿主终止/布局事件猜测写成根因。新增 stage 12（匹配的宿主 composition 终止）与 13（layout destroy）保留现有清理语义，不抑制宿主回调以强行通过。

加入上述阶段诊断后的完整命令，x64 前 86 轮通过，第 87/100 轮首次 N 超时：排队 18 µs、Actor 59,181 µs，客户端 header 59,377 µs、总计 59,423 µs；尚未到实际退出注入。该次未进入 Win32 压力段。结论仍是**重复卸载开销已改善，首次真实转换的尾延迟与候选生命周期压力验收未通过**；不能因更长的连续通过区间宣称问题消失。

本阶段默认 fake x64/Win32 各 100 轮故障回归通过（每轮两次 owned Broker 退出/恢复）；默认 Rust 105 项、诊断 Rust 107 项与各自 1 项 compile-fail 通过，debug/release 的默认/诊断 Clippy、rustfmt 和拒绝警告的 rustdoc 通过。fake 的确定性结果不替代上述真实词库压力验收。

最终较小范围真实命令在默认关闭诊断与显式开启诊断两种构建下，均完成 x64/Win32 各 20 轮，以及 IPC/16-client pool/候选鼠标布局/延迟动作取消/EDIT 与 context 文本核对。该结果是有限回归通过，**不覆盖或取消上述 100 轮命令的失败**。默认 release 四项启动拒绝检查、16 项不改系统状态的 registered 事务策略检查通过；只读 registrar 再次确认 COM 双视图缺失、profile 未注册/启用/激活。构建工具重建再次核对完整 archive 与每个 extracted source；本机未进行 remote CI、注册输入法或安装。

以上仍为进程级重启/受控 text store，文件系统缓存未清空，不是机器冷启动、普通应用或安装后的延迟验收。尚无已签名的发行 runtime/resource、Windows 11 普通宿主矩阵、更多键序列/长句/中英文混输、TSF UIElement/混合 DPI 或尾延迟指标。
