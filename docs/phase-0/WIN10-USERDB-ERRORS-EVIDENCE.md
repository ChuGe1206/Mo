# Win10 实际用户词典错误拒绝与恢复

日期：2026-10-04。仅独立开发 DLL 和有 marker 的新合成目录；没有操作真实用户词典或 VM。策略见 ADR 0055（Proposed）。

## 实现范围

`native/librime/preparation/userdb-preserve.patch` 增量应用于锁定 core+Lua、preparation v2、学习策略后的源码：

- LevelDB 打开启用 `paranoid_checks`，不忽略本轮 WAL 恢复错误；`reuse_logs` 保持默认关闭。
- `UserDictionary::Load` 打开失败直接返回 false，移除自动 `userdb_recovery_task` 调度。没有调用 RepairDB、重命名、删除或重建。
- Memory 保存 `Load` 的返回状态；准备阶段要求恰好一个名为 `rime_ice` 的主用户词典，且 Load 成功、仍 loaded。可缺省的 stabledb 文本词典不新增必需性。
- 使用准备返回值和现有 `mo_rime_prepare_resources_v2` 的 noexcept 边界，将失败传给 Rust backend，再由现有 EngineService 启动错误传给 Broker。未新增会跨 C FFI 的异常。

该补丁**尚未加入 runtime-build、staging、CI runtime 或安装包**，原接受 runtime SHA `94D646160F78DFF6408E21DBD0C003CD7D71C5DD6B604AEDAA6CA780F96D93C1` 未变。实验继续使用 v2 导出，旧 DLL 也拥有该导出，故它不能作为正式新策略的身份门；产品接入前须定义可区分的新 ABI/来源 receipt 并重建验证。它也不拒绝显式 legacy debug `--rime` 路径的降级行为。

## 真实文件错误矩阵

没有使用 FaultEnv 或故障环境变量。先让实际 DLL/Actor 创建新用户词典，独立 pinned LevelDB fixture helper 在该已有 DB 中同步 batch 写入 32 条固定合成记录。每种故障从这个 DB 复制新 fixture：

| 故障 | Actor | Broker | 数据文件 | 解除故障后 |
| --- | --- | --- | --- | --- |
| 父进程用 FileShare.None 暂时独占唯一非空 WAL，触发真实读取 sharing error | exit 1；准备失败；无 READY/dispatch 输出 | exit 1；准备失败；无 listening | 两次拒绝后 WAL、MANIFEST、CURRENT、SST 文件集合和哈希保持一致 | 释放句柄后 Actor 两轮合成输入通过，Broker 就绪；32 条记录逐条读回 |
| 仅翻转新 fixture WAL 第一条记录的 checksum 字节 | 同上 | 同上 | 损坏副本在拒绝前后集合/哈希一致 | 父进程显式恢复原 fixture 字节后，同上 |

正常 Broker 在观察到 listening 后由父进程结束，exit -1 是已记录的清理动作。最终矩阵共 12 个 owned 子进程调用，含两次 seed、四次拒绝、四次正常恢复、两次记录验证。

比较排除 LevelDB 的 `LOG`、`LOG.old` 和 `LOCK`：打开尝试会更新日志，不宣称全部目录字节不变。CRC 恢复是测试父进程操作，不是产品修复能力；该小 WAL 的文件保留不代表大 WAL/中途 compaction、所有损坏类型、断电或非同步学习的无损保证。helper 直接写合成键用于完整性验证，未验证真实学习格式/词频语义。

Actor 的准备没有输入；恢复后的两轮只输入合成 `n`、检查五个候选并 Clear，不提交。其会话使用 Broker probe 选项，禁用学习；完整 TIP 后续回归另用默认学习设置。

## 保留的失败与验收边界

- 第一版检查全部 Memory 用户词典，使可缺省 stabledb 缺失时正常准备被拒绝；日志保留于 `matrix/win10-userdb-errors-v1`。修订版只强制主用户词典，完整故障矩阵通过。首版 DLL 已被本实验目录的重建覆盖，不能当作已归档二进制。
- 第一次 TIP harness 启动归档位置的 Broker，TIP 按固定开发映像身份拒绝，四次都在 0 ms 不消费 N；没有 dispatch trace。复验使用相同 SHA 的 Broker 临时放在 `target/debug/mo-broker.exe`，结束后恢复原文件；源码核对与位置更正后实际出现 dispatch。旧失败仍保留。
- 修正位置后的 x64 首/第二轮仍 exit 1（N/I 约 47 ms 未消费）；Win32 首轮 exit 1（N 约 63 ms），第二轮完整 exit 0。第一条 dispatch engine 分别 77,382 / 204,044 µs；就绪 2,951 / 1,757 ms。harness 的 combined 标签沿用历史名称；新 DLL 未包含代码页或映射预读补丁。未修改 50 ms key、400 ms activation 或首次切换预算，不能升级为延迟/双架构 TIP 全通过。
- Rust fmt、workspace/all-targets Clippy `-D warnings`、mo-rime all-targets（22 library + 3 Actor 参数测试）、九个策略拒绝和两个脚本 AST 通过。fixture helper `/W4 /WX /MT` 编译通过；runtime 沿用既有编译设置，有上游 C4251 等警告，不声称 runtime 零警告。
- workspace 默认首轮四个 watchdog 子进程三秒退出检查失败，默认复跑剩 panic 一项，串行复跑剩 startup 一项；三份原日志保留。随后 startup 单项通过（2.13 秒）。完整 workspace 本轮仍未通过，没有放宽断言或修改 watchdog，也没有定位其根因。

G2/G3 状态和 VM 0.0.11.0 不变。该策略修复方向已获实际 native/Actor/Broker 有限错误证据，尚不能替换接受运行时或推进发行。

## 重现

1. 在全新 build 子目录复制/准备接受运行时的锁定源码和 Mo preparation sources；依次应用现有 preparation、机器 Lua、学习补丁，再从仓库根用 `git apply --directory=<新源码相对目录> native/librime/preparation/userdb-preserve.patch`，或像 runtime-build 一样显式指定 git-dir/work-tree；不能只对 repo 内子目录使用 -C。禁止覆盖既有 dist/prefix。
2. 用 runtime-build 同等 x64 Release/static CRT/core+Lua CMake 参数，复用已核对的 pinned 依赖；生成独立 DLL 并复制接受 runtime 的相邻 opencc 资源。源与生成 cache/proj 已归档；cache 八项 include/library 路径检查及实际 patch 应用后五份文件与编译源码的 LF 正规化逐份比较通过。
3. 用 `UserDbFixtureProbe.vcxproj`，显式传 `MoLevelDbIncludeDir`、`MoLevelDbLibrary` 和新 `MoDiagnosticOutDir` 构建 helper。它拒绝相对路径、缺 marker/DB、未知模式和额外参数；原生路径和 DB 内容拒绝 reparse。
4. 调用 `tools/test-userdb-errors.ps1`，传绝对 Runtime/Actor/Broker/FixtureProbe/SharedData/CompiledData 路径及 `ExpectedHashes`（runtime/actor/broker/fixture 四键）。harness 只创建 repo/build 下新 EvidenceName，拒绝旧目录、逃逸、输入 hash 不符及 reparse；子进程上限 30 秒，清理只结束自己启动的 PID。策略测试见 `tools/test-userdb-errors-policy.ps1`。

## 身份与封存

| 产物 | SHA-256 |
| --- | --- |
| 独立 policy DLL | `E8D8C1FD3234053BEF98C69BF2A502F9E0CF46FF0971DE557799FD727CC5CE0B` |
| Actor probe | `701EAC4ACA3E6F221111D7A29C108383E828DB755C44DB63C3BF255BE51FE373` |
| 实测 trace Broker | `DD3DB30E7D7287A186A4419075579395246BDA1C6B8A458EA04A089680A33817` |
| 最终 fixture helper | `74A82A4F4A2F5381914230EC380EA3A5C9678601E55D83D9B1CBC1D275901D8D` |

[Git 汇总](evidence/WIN10-USERDB-ERRORS-20261004.json) 绑定本机 `build/win10-evidence-clean-v1/UserDbErrors-v1` 清单：193 个文件、132 个外部身份，manifest SHA-256 `3856C97072B9A906470C0CEF6BBF953CB141FE016F498C5D1BD19367859230B6`。归档包含实际 DLL/OpenCC/三种 probe及双架构 TIP、修改前后源码、脚本、generated cache/project、全部失败/成功日志与合成 DB；compiled/schema 原始输入记外部哈希。build 归档不提交 Git。

下一步：先处理完整 workspace 的 watchdog 验收缺口，再将新用户词典策略通过可区别旧 DLL 的 ABI/provenance/CI 门接入构建，覆盖 privacy/learning/schema/preferences/故障退出和新 stage。同步学习与显式备份/恢复策略、安装态/VM 普通宿主、cold startup 延迟仍待验证；日志复用暂不采用。
