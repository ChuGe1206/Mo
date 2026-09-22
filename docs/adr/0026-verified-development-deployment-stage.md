# ADR 0026：固定布局的开发素材包与预编译词库闭环

- 日期：2026-09-18
- 状态：实现完成；仅开发构建一致性，不是发行授权；用户 Lua/staging 后续由 ADR 0039 收口
- 延续：ADR 0016、0024、0025；不改变 TSF 注册权限、IPC 或 50 ms 按键传输预算。

## 决策

在安装事务前先建立一条不写机器/真实用户配置的素材准备流程。
`installer/windows/prepare-stage.ps1` 不再接受任意预构建 Broker/TIP；
它消费明确的已验证运行时和锁定 rime-ice Git 对象，快照 Mo 源码、
全新构建，并把可安装路径的素材组织到 `stage/payload/Mo`。
该路径只是安装布局的镜像，不会复制进 Program Files。

运行时门核对六份 Git archive 的实际哈希与固定 commit、11 份 Mo
输入的 workspace/snapshot/provenance 三方哈希、固定公共 API header、
DLL、v2 preparation ABI、仅 Lua 插件及全部 33 份 DLL 相邻转换资源。
不把官方包含 octagram 的验证资产引入素材包。

rime-ice 只消费 commit `6810e8916d160498620a16fef2135956fecbd485`
生成的 Git archive；必须符合已有锁定哈希
`CD1895FBC961131A62F23277F636C27A6FB941DAC66DAF43C4A10D4E9E6ADAD3`。
不读取源 checkout 的 mutable/ignored 编译文件；不修改任何上游 YAML
或 Lua。64 份选定输入在隔离标记目录通过 Mo 自编译 deploy helper 和
已验证 DLL 编译出 29 份配置/词库输出，包含 melt_eng、radical_pinyin、
rime_ice 和已有全拼/双拼/T9 配置。所有源码输入、产物与运行时依赖均
进入数据 receipt；归档、LICENSE、Credits 原样保留在 evidence 中。

Mo 编译使用明确的 x64 Rust target、release、关闭 debug assertions、
无诊断 feature、locked/offline Cargo 与全新 target-dir；MSBuild 全新
重建 x64/Win32 TIP、ABI probe、registrar，显式关闭 latency trace。
独立检查 Broker 拒绝 `--fake`、两个 ABI probe 的诊断 IID 必须拒绝。
明确的编译器环境覆盖被拒绝，但构建宿主及工具链本身仍属于可信前置。

最终目录为 131 个 payload 文件及 6 个 evidence 文件；中间对象和探针
留在 `working/`，不进入 payload。清单覆盖全部文件、大小、SHA-256，
拒绝额外/缺失文件与空目录、路径遍历/ADS/保留设备名/大小写别名、
reparse 点、错误架构/EXE-DLL 标记、错误依赖或诊断 receipt。
JSON 必须是有界有效 UTF-8 对象，拒绝重复或大小写冲突属性、NUL、
超深和超过 1 MiB 的 metadata；开发标记必须是真正的 boolean。
pending 清单验证成功后才改名为最终 `mo-stage.json` 完成标记。

MSVC 初始化的唯一 cmd 调用仍需要批处理环境，但构建 output 与 VS
 安装路径明确拒绝 `%`、`!`、引号，避免百分号/延迟变量展开穿透路径
 引号；三项拒绝在读取编译来源或创建输出前执行。该限制只属于构建
 工具，不限制用户输入字符；命令变量/延迟展开机制见
 [Microsoft cmd](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cmd)。

PE 检查只读 DOS/PE/COFF/optional-header 的架构和 image/DLL 标记，
不把 header 检查解释为签名或完整 loader 验证；字段含义依据
[Microsoft PE Format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format)。
工具链发现使用进程级 `RUSTUP_AUTO_INSTALL=0`，避免只读发现隐式
安装缺失的 toolchain；含义依据
[rustup 环境变量](https://rust-lang.github.io/rustup/environment-variables.html)。
Git 只对明确的来源路径使用单次 `-c safe.directory=...`，不写全局配置。

## 真实回归与保留的负向结果

- 初次 sandbox 读取 Temp 上游仓库被 Git ownership 检查拒绝；随后
  使用来源路径限定的单次配置与实际 archive 固定哈希双重约束。
- 初次工具链发现遇到 rustup 的隐式 channel 同步行为；已经关闭
  discovery 自动安装，并允许显式选择本机现有 stable alias，但必须
 报告 pinned Rust 1.97.1，未安装指定版本时在创建输出前拒绝。
- 初次 C++/Cargo 快照调用遇到 PowerShell array 输出被当成单个路径；
 已经修正枚举与空数组边界，旧目录不覆盖、不冒充有效完成产物。
- 第一轮实际素材 TIP 故障探针因 Broker 文件身份不匹配在第一个按键
 拒绝。安装布局 TIP 正确要求同树 `bin/mo-broker.exe`，而测试错误地
 配对仓库 debug image。这不是输入引擎超时。修正只发生在一次性
 fixture：同字节 TIP 副本与 diagnostic Broker 同树配对。未添加路径
 覆盖口、未替换素材中的 release Broker、未放宽身份/时限约束。
- 最终构建的 deploy helper `/W4 /WX /sdl /GS /guard:cf` 成功；六个
 原生项目 rebuild 均 0 warning/0 error，双架构 default ABI probe 通过。
 此结论仅针对这些 Mo 项目，不能覆盖 librime/Lua 既有上游 warning。
- 本机 staging policy 79 项通过，包含实际素材包搬迁/变更/额外文件、
 completion-marker 丢失、错误架构、诊断文件与诊断 receipt。重新
 计算外层 inventory 后的后三种错误仍由独立契约门拒绝。fixture 的
 空目录可安全清理，reparse 遍历仍拒绝，避免失败清理覆盖原始错误。
- `resource_pack_smoke` 使用空 managed user/staging 与 explicit prebuilt
 路径完成 7 组真实候选选择：你好、👋、hello、日期、Unicode 中、数字
 一百二十三、计算器 3；每次 commit 只取到一次，staging 始终为空。
 日期使用测试时本机日期，不宣称静态日期快照为长期 golden。
- input-free preparation 的 context/commit/Emoji 与 Actor 翻页选择
 检查通过；与旧用户目录内的 schema 副本无关的 prebuilt-only 输入
 测试证明这七个案例不触发在线 schema/字典生成。
- 功能快照 `build/mo-windows-stage-golden` 的 68 份 Mo 源码快照均与 workspace
 实际字节对应；137-file stage inventory 验证成功。最终清单 SHA-256：
 `8BECA65E2D5C5E46604593F2EDEDF34AD15AAC9530D01C8F170A18927919A975`。
 DLL 仍为 ADR 0025 最终 verified 产物
 `12498660CA4AD436DA8A1EC3A7F277DF356B624A9B25FAD36F4E0883AA50FD15`。
- 同最终产物第一条 10 轮命令 x64 10/10、Win32 6/10 通过，Win32
 第 7 轮在 stop cycle 0 前返回候选窗 visibility mismatch；日志
 `build/mo-stage-golden-runtime.log` 保留。default 模式未提供具体候选
 reset/阶段元数据，**根因未定位**；不能推断一定是消失、engine 超时、
 输入内容错误或外部窗口影响，也不将构建/IO 并发直接当成根因。
- 构建结束后的独立完整复跑同产物 x64/Win32 各 10/10 通过，20 个
 fault rounds / 40 次明确故障退出，golden/preparation/Actor 及 stage
 前后清单检查均通过。完整保留 `build/mo-stage-golden-runtime-idle-rerun.log`。
 第一条命令与复跑分别记账，不把有限复跑通过解释为修复偶发问题。
- 增加命令路径安全门后再次全新构建，最终产物位于
 `build/mo-windows-stage-safe`；68-source workspace/snapshot 对应通过。
 清单 SHA-256 为
 `ED575F61F1198A056C13FFB87D67CBA35D386E74FC806674CA64C2A4BCA3BDA1`，
 仍使用同一 verified runtime DLL。六个 native rebuild 的零 warning/
 error、双架构 default ABI、79 项 policy 与最终同产物 7 组 golden、
 preparation/Actor、x64/Win32 各 10/10（40 次明确退出）及前后 stage
 inventory 均通过；完整日志 `build/mo-stage-safe-build.log`、
 `build/mo-stage-safe-policy.log`、`build/mo-stage-safe-runtime.log`。
 这次通过仍不升级偶发 visibility 问题为已解决。
- 全 workspace default 109 项 + 1 compile-fail、trace 111 + 1 通过；
 fmt、default/trace 的 debug/release 四种 all-targets Clippy 和 7 份
 installer PowerShell AST 通过。CI 仅新增流程，尚未远端运行。

## 尚未关闭的发行门

所有清单固定为 development-only / redistributable=false /
installable=false；unsigned receipts 只是内部一致性证据，不是可信
签名/来源认证，不能凭验证成功执行外来素材包。构建工具、Cargo home
及 OS 不是 hermetic；路径检查不是 handle-based 并发/祖先 ACL 证明，
也不是全 hard-link 排除或资源内容认证。

完整 Rime/Lua/user/staging 覆盖策略、VC prerequisite、逐文件 SPDX
与第三方 notices、完整对应源/修改记录审查和正式许可证结论仍待完成。
保留 rime-ice 全归档不等于完整发行合规或运行时对应源包齐全。

WiX 入口已移除四个任意 artifact 参数，改为只接受验证过的 stage，
仍必须显式 `-AllowPlaceholderBuild`；当前 authoring 只包含原来的
四个 Mo 二进制，未加入 runtime/data、TSF 事务、用户 finalizer 或签名。
本机无 WiX，缺工具前置门已验证拒绝，未生成 MSI/Setup。

注册后系统 key route、Notepad/WinUI/AppContainer/混合 DPI、安装/升级/
修复/卸载回滚、启动与托盘/设置体验仍未通过。受控 fixture 使用
diagnostic Broker；不是安装模式 release 服务、真实宿主或系统冷启动
验收。有限压力不清除 ADR 0022/0023 的历史负向证据，尚不可日常使用。
