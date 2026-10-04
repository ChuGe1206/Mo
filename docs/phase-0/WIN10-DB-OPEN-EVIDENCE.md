# Win10 用户词典打开与映射准备证据

日期：2026-10-04。延续 [代码页/启动分项诊断](WIN10-IMAGE-PAGE-EVIDENCE.md)，基线提交 `05077fa4c0f287d245551b6c9d28988daca5974a`。本轮在 Win10 主机使用隔离源码、诊断 runtime 和合成目录，没有操作 VM。G2/G3 状态不升级；50 ms key、400 ms activation、首次切换 `<500 ms` 门槛不变。

## 方法与身份

新增 [增量补丁和使用说明](../../native/librime/diagnostics/README.md)，在既有 `components.patch` 后应用 `db-open.patch`。DB Env/file wrapper 转发原参数、Status、锁与 Sync；只记录固定方法名和耗时，不记录路径、键、值或内容。Sync 不设过滤阈值；其他 scope 默认只输出至少 500 µs 的调用。同步 stderr 输出会扰动计时。

隔离 DLL 位于 `build/win10-db-open-v1/dist/lib/rime.dll`，SHA-256 为 `CC732CB66A745372D97E73AC4688E4D15240BFCB00385D37B17AEF281D37D65E`。编译目录 `runtime-component-trace-v1/compile` 当前也生成此版本；之前独立 dist 中的 v4、代码页和启动诊断版本继续保留，不能凭目录名判断版本。

依赖为 pinned LevelDB 1.23，commit `99b3c03b3284f5886f9ef9a4ef703d57373e61be`。其 `reuse_logs` 默认关闭、源码标为 experimental。诊断开关 `MO_DIAG_REUSE_DB_LOGS=1` 仅设置该原生选项；不覆盖同步写、恢复、锁或 fresh/large/failed-append 回退逻辑。该选项没有进入产品构建器、stage 或安装包。

全部 Actor/完整 TIP 样本开启 mapped + image 准备及 DB I/O tracing；表中的 off 仅指日志复用关闭。代码页探测为 893 个可读页、654 个具有执行权限的页；这些是本版本计数。OS 缓存、主机负载和测试目录状态没有随机化控制，不是机器冷启动样本。

## Actor：独立用户目录与共享预编译目录

[Actor 探针](../../crates/mo-rime/examples/actor_latency_probe.rs) 新增 `--machine-prebuilt`，将 `prebuilt_data_dir`、`staging_dir` 同时指向有 marker 的编译 fixture，匹配安装计划的两个配置字段。用户目录仍单独有 marker；shared fresh 开始时没有 `user/build`。编译目录从 `build/mo-user-keyroute-v1/build` 复制一次，每个文件在结束时哈希不变。此模型不覆盖 Program Files ACL 或安装态启动。

每个进程使用 `--broker-plan`，禁学习，两次合成 `n`，检查 handled、预编辑、五个候选、无 commit，随后清空/销毁。八个进程均 exit 0。按实际运行顺序：

| case | 共享 prebuilt | reuse | Actor prepare µs | 首个 DB::Open µs | 该 Open 内 Sync 次数/总 µs | 首个 Actor dispatch µs |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| local-fresh | 否 | 关 | 2,398,609 | 365,430 | 4 / 313,088 | 5,499 |
| local-existing | 否 | 关 | 707,110 | 202,808 | 3 / 182,169 | 3,811 |
| shared-fresh | 是 | 关 | 1,428,130 | 509,909 | 4 / 490,867 | 4,168 |
| shared-existing-off-a | 是 | 关 | 1,340,529 | 220,948 | 3 / 202,159 | 3,868 |
| shared-existing-on-a | 是 | 开 | 147,756 | 2,762 | 0 / 0 | 3,332 |
| shared-existing-on-b | 是 | 开 | 141,760 | 2,213 | 0 / 0 | 3,661 |
| shared-existing-off-b | 是 | 关 | 242,107 | 105,686 | 2 / 101,346 | 3,553 |
| shared-fresh-reuse | 是 | 开 | 403,214 | 261,509 | 2 / 257,396 | 5,714 |

Sync 只计与首个 DB::Open 相同调用线程、parent id 匹配的区间，不包括后续同步写或后台工作。因 Sync 阈值为零，表中的零次不是阈值过滤的结果。Windows LevelDB 的 Sync 包含 FlushBuffer 和 FlushFileBuffers；整段耗时不能全部归因于磁盘或单个 Windows API。

复用既有 shared profile 的 DB::Open 从关闭选项时的 106–221 ms 降至打开时的 2–3 ms，同时观察到该 Open 中 Sync 从 2–3 次变为零。fresh reuse 仍需要两次 Sync，DB 打开 262 ms，Actor prepare 已达 403 ms，尚未包含所有进程启动成本。

Dictionary 的长区间主要落在 `TouchReadOnlyPages`，不是 FileOpen/MapView。shared-fresh 首个 Dictionary::Load 为 532,565 µs，其中首个 table touch 491,697 µs、prism touch 38,464 µs；shared-existing-off-a 为 529,073 µs，其中 table touch 526,215 µs；on-a 为 21,916 µs，其中 table touch 21,554 µs。缓存/顺序混杂明显，不能把整个 prepare 差值都归因于日志复用。local-fresh 的 Engine load 另有 336,067 µs，其余为约 5.9–11.1 ms。

## 完整 TIP：双架构首轮和第二轮

默认开发 Broker 与 x64/Win32 TIP/ABI probe 在 local profile 上运行四组，每组首轮及第二轮各一次，八次均 exit 0；完整候选、模型鼠标/布局、延迟动作取消、重连和 TSF 终止/恢复 marker 通过。Broker SHA-256 为 `277CEE927571F5521E44ECBFF68CA6B54AD85801DB98C9D924AA8EC2803396C5`，实际使用副本已归档。

此路径使用已有 `user/build`，没有测试上述共享 prebuilt 配置。默认设置允许本地学习；全部输入仍为合成输入且仅在 owned fixture 中进行。这与 Actor 禁学习条件不同，目录状态变化也是顺序对照的混杂因素。

| case | Broker ready ms | 首个 native ProcessKey µs | 首个 DB::Open µs | 该 Open 内 Sync 次数/总 µs |
| --- | ---: | ---: | ---: | ---: |
| x64-off | 2,316 | 1,783 | 321,279 | 2 / 220,217 |
| x64-reuse | 471 | 1,639 | 19,569 | 0 / 0 |
| Win32-reuse | 525 | 1,515 | 3,057 | 0 / 0 |
| Win32-off | 532 | 1,542 | 333,116 | 3 / 308,309 |

Session / PrepareResources 分别为 1,244,138 / 130,744 µs、83,649 / 80,376 µs、74,702 / 85,568 µs、394,257 / 80,146 µs。它们是嵌套或分段区间，不能与总启动时间直接相加推算未经测量的阶段。

首键计时开始前 Broker 已准备好，不包含首次激活拉起进程。471/525 ms 都超过 400 ms，Win32 也超过首次切换 500 ms 目标。剩余总启动成本需要测量 Rust 参数/计划解析、pipe pool、Engine load、prepare 和 readiness，尚无证据将其归因于某个 IPC 阶段。这些有限成功不能代替已注册桌面宿主验收。

## 检查与证据封存

- 独立 x64 C++17 `/W4 /WX /MT` DB probe 构建零警告/错误；同步合成写读回、legacy/reuse 双向重开、重复锁拒绝、缺 DB 拒绝、失败创建状态保留均通过。此检查不证明崩溃/断电恢复、大用户词典或 I/O 故障回退。
- 缺 pinned LevelDB include、缺诊断 include 两项构建守卫在编译前拒绝，无输出目录。
- 两份补丁在接受源码的独立副本中检查、实际应用；15 个改变的源文件与编译副本正规化换行后相同，三个诊断头与编译副本哈希相同。
- Rust 1.97.1 fmt、workspace/all-targets Clippy `-D warnings`、workspace tests 通过。Actor 参数/marker 三项测试通过，并加入 CI。

[Git 结果汇总](evidence/WIN10-DB-OPEN-20261004.json)与[本机原始汇总](../../build/win10-evidence-clean-v1/DbOpen-v1/results.json)、[清单](../../build/win10-evidence-clean-v1/DbOpen-v1/host-evidence-manifest.json)绑定 92 个归档文件和 205 个外部输入身份。清单 SHA-256 为 `37BFAB3DE9C5355DFDB90BA91F305E230FCF1BF3CF34F1E634B00A0063E7DCA3`。归档包含实际 DLL、Broker、Actor、两架构 TIP/probe、DB probe、源码前后快照、成功/失败构建日志及历史 harness；数据/OpenCC/LevelDB 输入另记哈希。build 归档不提交 Git；外部输入身份是采集时记录，之后的可变路径不能冒充历史产物。

Actor 源码在构建后只改了顶部用途注释；归档 `actor-probe-source-at-build.rs` 保留构建时注释和功能代码。归档不承诺 PE 字节可复现或完整 PDB 调试环境。

接受的生产 runtime SHA-256 仍为 `94D646160F78DFF6408E21DBD0C003CD7D71C5DD6B604AEDAA6CA780F96D93C1`；VM 仍保留 0.0.11.0，本轮无新 VM 验收。下一步优先细分 Broker 启动剩余阶段，随后验证代表性 profile 与错误/崩溃恢复，才考虑产品选项；继续以 Win10 为主，补齐注册宿主、loaded-TIP 和登录矩阵。

后续 Broker 八阶段计时与 fake 启动对照见 [启动分项证据](WIN10-BROKER-STARTUP-EVIDENCE.md)，原有超预算样本继续保留。
