# Win10 用户词典 ABI v3 接入证据

日期：2026-10-04；主机 Windows 10 专业版 10.0.19045，开发分支 develop。
契约见 [ADR 0055](../adr/0055-userdb-startup-error-preservation.md)，原始策略试验见
[用户词典错误证据](WIN10-USERDB-ERRORS-EVIDENCE.md)。

## 实现与身份

原生 wrapper 仅导出 `mo_rime_prepare_resources_v3`；dumpbin 实际导出核验通过。
Rust prepared backend 在创建私有 anchor 前检查 v3，v1/v2 不回退。
部署数据工具同步要求 v3。核心资源 patch 保留历史文件名 resources-v2.patch；
新增的 userdb-preserve.patch 在核心 learning patch 后应用，使用明确的 vendor
git-dir/work-tree，避免父仓库路径导致补丁被跳过。

新 builder 从六份固定 Git 源归档和已校验工具包完整构建，快照 15 个 Mo 输入。
源码守卫检查严格 LevelDB recovery、普通 Load 失败不调度自动修复、
Memory 保存 Load 成功状态，以及恰好一个健康 rime_ice 主用户词典。
可缺省的 stable 文本词典仍可缺省，没有启用 log reuse。
format 2 provenance 新增 ABI 3、strict-open-no-auto-recovery-v1 及补丁哈希绑定；
runtime 与 completed-stage 两处检查都要求该契约。

| 产物 | SHA-256 |
| --- | --- |
| 新 DLL | `BE5E3E374D9FBE12381A0E3D522079033D85C08F4675F7A9A556A6DC3D30D564` |
| 用户词典 patch | `4CA43FDB22F4E8C9CEC58A3D9DB97BBE70082A768906DEAADD8CC85F69DCA422` |
| runtime provenance | `11C4396204CB2657696AB9EAE9E58161718B773DCFF9F631033D9CFD39389002` |
| 新 stage manifest | `9650E87A48E2B9DC0D12FCEE328789F57BA5678FD4A920C5E0A00D5F5AD02383` |

新 runtime 在 `build/mo-runtime-userdb-v3`；全新 release stage 在
`build/mo-windows-stage-userdb-v3/stage`。后者为 non-installable development stage，
Rust release/debug_assertions=false/latency_trace=false，双架构 TIP 与 release Broker
均由新源码快照重编译，固定 rime-ice 数据由新 DLL 部署。

## 已通过检查

| 检查 | 结果 |
| --- | --- |
| 默认/trace workspace、fmt、两套 Clippy、workspace/examples build | 通过 |
| 双架构 native ABI，fake IPC/pool/UI/edit 与四次外部 Broker 退出 | 通过 |
| 新源码守卫，八种回退 mutation，旧 runtime 契约拒绝 | 通过 |
| 旧 ABI 2 staging 拒绝 | 创建输出目录前拒绝 |
| 实际用户词典矩阵 | 12 child；4 次在 ready 前拒绝，2 组各 32 条同步合成记录保全 |
| harness/AST | 9 守卫、2 AST 通过 |
| preparation | 3 case 通过，旧 ABI 2 在 anchor 前拒绝 |
| relocation/resource preparation | 27 case 通过 |
| 隐私→正常→隐私学习 | 持久化词条数 0/1/1，正常学习频率保持 1 |
| 方案、简繁体、Emoji | 五方案×两模式及 Emoji on/off 通过 |
| 新 stage | 97 policy、21 authoring、18 lifecycle policy 通过 |
| 新 stage 真实数据 | 七项 machine-only golden、准备、Actor paging/page-local selection 通过 |

sharing-denied WAL 和 CRC 损坏仅作用于新 marker 用户目录。四次失败后，
两份原数据文件的集合与哈希一致，排除 LOG/LOG.old/LOCK；没有重命名或删除数据库。
解除父进程文件共享锁、或恢复原测试 WAL 字节后，Actor 可执行两次合成 N/Clear，
Broker 可就绪，helper 读回全部 32 条同步记录。
两个 recovered Broker 是父 harness 在确认 ready 后停止，退出 -1 为预期清理，
不作为内部 watchdog 或真实 TIP crash-recovery 通过的证据。

本机 preparation 的 missing 用例使用旧 ABI 2 DLL；CI 的同一用例使用固定官方 DLL。
旧 extension 缺失、资源准备失败都不允许 Broker 宣布就绪。

CI 接入源码守卫、真实错误矩阵和学习/方案回归。四个 native 负向守卫完成后
会留下非零 LASTEXITCODE；仅在全部九守卫/两 AST 断言通过后清除此状态，避免
GitHub PowerShell epilogue 误报。同进程调用已确认状态 0。所有实际失败仍保留。
九个 CI PowerShell block 的 AST 通过；本机缺少 PyYAML，未做 YAML parser 验证，
也未将远端 CI 声称为通过。

## 严格 TIP 仍失败

| 路径 | 结果 |
| --- | --- |
| 新 stage x64 fault harness | 首个 N 未消费；探针外部耗时 63 ms；exit 1 |
| 独立新 fixture Win32 fault harness | 首个 N 未消费；探针外部耗时 47 ms；exit 1 |

两项都在 stop cycle 0 前失败，所以没有新增真实 fault crash/recovery 通过。
TIP 为 stage 的相同 release 字节，匹配的 sibling bin 放置受控诊断 Broker；
不是 VM 安装态或 release Broker 的系统路由验收。没有改变 50 ms 健康预算。
47 ms 是探针外部计时，不等同于 Actor 执行时长，也不能据此推断具体超时根因。
seed Actor 的独立首轮 prepare 约 1.86 秒、首 dispatch 约 88.4 ms，同样不能用
数据保全通过替代延迟门。G2/G3 继续开放。

第一次组合检查父进程在 authoring 后中止，未留下 lifecycle/runtime 结果；
保留该日志，随后独立完成 lifecycle 18 项及实际 runtime 测试。runtime 失败
没有重试成通过。错误的 Rust feature 调用和缺失 YAML parser 日志也保留。

## 复现与归档

先用 [runtime builder](../../tools/runtime-build/README.md) 的固定输入构建新目录。
执行 `tools/runtime-build/test-userdb-policy.ps1`，传入新 RuntimeBuildDirectory
及旧 LegacyRuntimeBuildDirectory。准备探针和实际 fault matrix 的本机完整命令
见归档 runners；每次 matrix EvidenceName、learning output、stage output 必须新建。
stage 策略由 verify-stage/test-staging/test-package-authoring 复验；
test-stage-runtime 保留原有时限。Win32 独立 runner 保留全部输入映像身份。

[Git 汇总](evidence/WIN10-USERDB-ABI3-20261004.json) 绑定
`build/win10-evidence-clean-v1/UserDbAbi3-v1`：610 文件、610 输入记录，
manifest SHA-256 `283576647E9A67220E4E55136C6E859D0BEEF7F13FFBF0A99A71C8AF4A3F9294`。包括实际 DLL/PE、源码及 patch、
固定源归档、全新 stage、全部合成 DB、正常和失败日志；最初 policy 脚本和最终
CI 状态修复分别快照。build 归档不进入 Git。

本轮没有操作 VM；已安装 0.0.11.0 与其旧 94D646… runtime 保持原状态。
下一步先定位当前 Win10 真实 TIP 首键失败，再复验安装态新 ABI 的普通宿主、
loaded-TIP 升级、登录重启。显式备份/恢复 UX、通用损坏修复和断电持久性仍是独立工作。
