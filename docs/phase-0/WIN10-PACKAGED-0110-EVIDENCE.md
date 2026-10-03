# Win10 0.0.11.0 安装包与真实宿主续测

日期：2026-10-03。Windows 10 22H2 x64 build 19045；继续以 Win10 为优先目标。ADR 0054 的重复计数修复已进入新的 unsigned DevelopmentTest 包。本轮没有临时替换安装 DLL，未放宽 50 ms 按键传输 deadline。

## 包身份

| 项目 | 本轮身份 |
| --- | --- |
| 产品版本 | 0.0.11.0（从 0.0.10.0 升级，MSI 前三段递增） |
| MSI ProductCode | `{DA51A39A-4343-4C21-9A05-F036918762C7}` |
| Stage manifest SHA-256 | `2D796DC212348F760470B8CE2A547B02943C8EAE4377306CED388D077E476B7B` |
| Bundle SHA-256 | `25B1B2BA8DBCAA8904830F38E0303147FB5D208760611A84F09836414502E978` |
| MSI SHA-256 | `C653819E067C37DEAF6F688750586982031C719664727ED220D36D6A9AB58BFA` |
| x64 TIP SHA-256 | `6098F18A7154AB8FA4715723FCBD73053FC0B9ADF34BFC367387DF204010990D` |
| x86 TIP SHA-256 | `3BA1FD3D42A2D167DE240A0DEBBA4E5C8271194117144B65A000356942BA616F` |
| mo_tip.cpp SHA-256 | `4EEF027FF952464E12FD43D9194CE48A42EE927528F04AE658470A337B8F5CB1` |

新 stage 的锁定源部署及 x64/Win32 严格编译、ABI 探针通过；linked MSI/Burn 反向校验、MSI ICE 与 hash-locked VM kit 生成通过。receipt 标记 release、debug_assertions=false、latency_trace=false；开发故障注入保留，不能分发。构建入口及日志分别为 `build/build-win10-key-count.ps1`、`win10-key-count-stage.log`、`win10-key-count-link.log`、`win10-key-count-kit.log`。

## 升级、重启与输入

| 检查 | 有限结果与证据 |
| --- | --- |
| 0.0.10.0 → 0.0.11.0 变更载荷升级 | Bundle exit 0；旧 MSI state=-1、新 MSI state=5；132-file payload、目录 ACL、机器双视图 COM、用户 profile/finalizer 校验通过 |
| 默认输入法与设置 | 升级前后 default override 精确比较不变；原测试默认设置文件 SHA-256 保持不变 |
| 升级后、重启前首次输入 | 合成单键作为 ASCII 留在宿主；记事本加载新 x64 DLL，但 Broker 仍为 11:18 启动的旧进程；保留 negative JSON/截图，未证明直接原因 |
| 一次 Windows 重启与登录 | 正常进入桌面；重启后及收尾再次核验 payload/ACL/COM/profile/设置通过；最后 Broker 从固定安装路径于 13:46 新启动，晚于本次 boot |
| x64 记事本 | 新安装 DLL 的单键不重复、候选定位可见；连续两次 Space 提交，EDIT 精确回读预期四个 UTF-16 字符 |
| x86 记事本 | `SysWOW64/notepad.exe` 的 EDIT 精确回读同样通过；32 位 PowerShell 核验实际 x86 TIP 路径及包哈希 |
| x86 写字板 | `Program Files (x86)/Windows NT/Accessories/wordpad.exe` 的 RichEdit 精确回读同样通过；32 位 verifier 核验 x86 TIP 路径及包哈希 |
| 测试窗口与当前状态 | 所有 owned 记事本/写字板窗口关闭；Microsoft 拼音为前景，Mo 保持已注册/启用、未激活；默认设置文件保留 |

旧矩阵 verifier 限定同载荷升级；本轮使用单独的变更载荷 harness，没有把新源码套入旧矩阵。安装前校验原 0.0.10.0 载荷，安装后按新的 stage manifest 校验全部文件。日志原文中已有 RebootPending=1，但本次 Bundle 返回 0、restarting=No；不把该已有标记归因于 Mo。

第一份 x86 模块检查在 64 位 PowerShell 中未枚举到 Mo 模块，guard 拒绝了该尝试。`IsWow64Process` 确认宿主实际为 32 位；改用明确的 32 位 verifier 后记事本与写字板均核验通过。保留原拒绝截图与 `syswow64-notepad-identity.json`，其 x86_acceptance_passed=false 仅指那次未完成身份检查的尝试。

三种宿主均为合成测试内容；记录比较布尔值、长度、宿主和模块身份，未收集真实用户输入。本轮没有重跑繁体/深色 UI 设置；该功能的前轮有限证据见 [桌面实测](WIN10-DESKTOP-EVIDENCE.md)。

## 超时诊断进展与边界

trace 使用隔离的 native/target 构建与独立用户数据副本，未覆盖包中的默认 DLL。probe 失败后只在 harness 中等待 0.5/2 秒收集 Actor 完成日志；原 exit 1 保留，TIP deadline 不变。

- 2 秒收集窗口取得首次 dispatch：queue=37 µs、engine=1,309,201 µs；客户端约 50.5 ms 等待响应头超时。
- 同一新 Broker 另一次首次 dispatch 为 1,671,197 µs；失败后第二个 probe 的首键为约 2.9 ms，但下一个键仍超时，Actor 执行为 252,509 µs。因此不能把问题缩成单一首次键初始化。
- 直接 librime 分段探针：完整 prepared 资源下首次 process_key=1,043,506 µs，context=1,562 µs，后续 process_key 为毫秒级。支持将该样本的主要长耗时进一步缩到 native process_key。
- 旧 relocatable runtime 直接对照也出现首次 process_key=524,690 µs，不能把当前现象简单归因于新增 learning patch。临时移除 Emoji filter 的机械消融首次仍为 107,727 µs；它不使用相同准备流程，不能作为单因素因果证明或产品配置改动。

该包轮次结束时未定位到 process_key 内具体组件；后续主机独立诊断已细分到 Prism/词表/用户词典查询，见 [组件延迟续查](WIN10-NATIVE-LATENCY-EVIDENCE.md)。具体 CPU/I/O/调度根因仍未证明，也未证明 VM 重启前 ASCII 的原因与主机 trace 相同。新包桌面有限成功不覆盖完整真实词库 smoke、严格延迟、压力或首次切换时间指标。G2/G3 未整体通过。

## 原始证据与交接

证据根：`build/win10-evidence-clean-v1/Desktop0110-v1`。含 upgrade-state、post-reboot、三种宿主回读、模块枚举拒绝、final-state、安装日志、源码/receipt、诊断日志与合成截图。guest-transfer-manifest 的三个文件 size/SHA-256 与主机副本相符；host-evidence-manifest 绑定 50 个文件。诊断默认构建保持在原 native out，隔离 trace 留在 `build/latency-key-count-v1`。

VM 当前安装 **0.0.11.0（包含 ADR 0054 修复）**，保留新的真实 Broker 与默认设置；不是前轮恢复的 0.0.10.0。继续集中 Win10：定位 native process_key 长耗时，补浏览器/WinUI、真实鼠标、忙预编辑设置切换、混合 DPI/多屏，以及 loaded-TIP 完整升级/登录矩阵和稳定性测试。本轮的一次升级与重启证据不覆盖完整矩阵。
