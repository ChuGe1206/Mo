# Win10 LevelDB 崩溃与错误恢复证据

日期：2026-10-04，基线 `150934900d13183dfe194b819c73473438234eaa`。延续 [Broker 启动诊断](WIN10-BROKER-STARTUP-EVIDENCE.md)，使用 Win10 主机、pinned LevelDB 1.23 和合成文件。日志复用仍只用于诊断，不进入产品；本轮没有更改 runtime/stage、输入状态或 VM。50 ms key、400 ms activation、首次切换 `<500 ms` 门槛及 G2/G3 状态不变。

## 实现与边界

新增 [原生恢复探针](../../native/librime/diagnostics/db_recovery_probe.cpp)、[严格 MSBuild 工程](../../native/librime/diagnostics/DbRecoveryProbe.vcxproj)、[矩阵 harness](../../tools/test-db-recovery.ps1)与[策略检查](../../tools/test-db-recovery-policy.ps1)。编译使用 x64 C++17 `/W4 /WX /MT`，显式消费接受 runtime 的 LevelDB headers/library；不链接 librime，不经过实际 Engine Actor/TSF 或上游用户词典恢复任务。

FaultEnv 包裹既有 DbEnv，按合成 DB 内部的 log/manifest 文件角色注入追加打开、日志打开读取或 Sync 错误；其他调用继续转发原操作。文件 wrapper/Env 生命周期覆盖 DB。stdout/stderr 只输出固定状态、计数和组件时长，没有输入或记录内容。所有 DB 仅包含探针生成的合成数据；原生 marker 与绝对路径守卫、harness SHA 和全新 build 子目录守卫限制测试入口。

崩溃测试由父 harness 创建子进程。子进程完成 `WriteOptions.sync=true` 更新/删除、读回后输出 durable_ready 并阻塞；父进程收到握手且确认子进程仍活着才强制终止。四种 writer/reader reuse 组合均使用独立目录，实际终止 exit 为 -1，没有执行 DB/C++ 栈析构。此测试覆盖同步写返回之后的进程终止，不覆盖写入途中、设备断电或硬件持久性。

当前 librime 的 Put/Delete/CommitBatch 使用默认 `WriteOptions`，即非同步写；DB 打开也沿用默认 `paranoid_checks=false`。因此本轮显式同步写测试不能作为全部实际学习更新的耐久性保证。

## 14 组矩阵与复验

每套矩阵有 12 组正向检查、2 组明确负向证据。第一套用隔离 runtime source 下的诊断头编译；第二套使用独立复制诊断头的 CI 布局，重复相同 14 组。两套共 60 次 native 子进程调用，其中 8 个在 durable_ready 后被父进程强制终止；两套结果一致。

| 组 | 条件 | 实测结果 |
| --- | --- | --- |
| crash-w0-r0 / w0-r1 / w1-r0 / w1-r1 | writer/首次 reader 的 reuse 四种组合；每组 128 条合成记录，更新及每七条删除一条 | 同步更新/删除全部恢复；可继续同步写读回，随后相反 reuse 模式重开仍保留更新/删除 |
| append-log | reuse 开，日志 NewAppendableFile 返回合成 I/O 错误 | 命中注入，正常回退，128 条原记录完整；可继续写，再以 legacy 重开检查原记录 |
| append-manifest | reuse 开，MANIFEST NewAppendableFile 返回合成 I/O 错误 | 命中注入，正常回退，原记录完整、可继续写与 legacy 重开 |
| sync-error-r0 / r1 | DB 打开后才对日志 Sync 注入错误 | 同步 Put 返回 I/O 错误；下次 Put 仍拒绝，第二次不再调用 Sync；正常重开保留此前成功同步写的原记录 |
| read-error-r0-strict0 / r1-strict0 | 默认 paranoid 关，日志 NewSequentialFile 返回合成 I/O 错误 | **DB::Open 返回成功，128 条原记录全部缺失**；探针 exit 2，作为负向证据保留 |
| read-error-r0-strict1 / r1-strict1 | paranoid 开，其余同上 | DB::Open 返回 I/O 错误且无 DB；移除注入后正常重开，128 条原记录完整 |
| large-r0 / r1 | 单次同步 batch 写 32,768 条、每条额外 256 字节；初始 64 MiB buffer 留 WAL，重开使用默认 4 MiB buffer | recovery 创建 table、未走日志追加复用；逐条读回完整，可继续写；相反 reuse 重开仍完整 |

large 样本有意跨越恢复 buffer，是有限的合成回退检查，不代表长期真实用户词典、全部 compaction/manifest 大小或多次断电行为。其结果不新增启动延迟通过结论。

### 读取错误：当前默认策略的负向结果

pinned `DBImpl::MaybeIgnoreError` 在 paranoid 关闭时将部分错误改为 OK；`RecoverLogFile` 的日志打开失败经过该路径。两种 reuse 条件均实际返回打开成功并缺失 128 条已成功同步写的记录。它说明当前默认恢复策略存在需要处理的边界；不能依据普通重开或有限崩溃通过就采用日志复用。

严格检查对照只证明直接 LevelDB API 能拒绝本次错误并保留后续恢复机会，不构成 Mo 产品修复。上游 `UserDictionary::Load` 在打开失败时有调度 `userdb_recovery_task` 的路径；该任务会尝试 RepairDB，失败后还有 rename/remove/recreate 的分支。是否在 Mo 当前 startup 中运行、如何保留原文件、如何把失败传给 Engine Actor/Broker，均未经过本轮真实 librime 故障注入。因此不能只设置 paranoid 就宣称产品已无损拒绝错误。

### 同步错误：失败返回不代表记录不存在

两种 reuse 条件的 Sync 错误均被返回并锁定为后续写错误。正常关闭并重开后，`failed_write_present=1`：那条返回 I/O 错误的记录实际可读。注入发生在 Append 之后、真实 Sync 之前，后续关闭可能写出剩余 buffer。这是本次失败操作结果不确定的实证；调用者不能据失败返回推断没有写入或盲目重放。它不说明本次失败写已经具备断电耐久性。

## 验证与 CI

- 两份实际 native 构建均零警告/错误，使用相同 pinned LevelDB library；因输出/PDB/include 路径不同，PE 哈希不同，不宣称字节可复现。
- 两项新工程缺 LevelDB include/缺诊断 include 在编译前拒绝，没有创建输出目录。
- 五项 harness 前置拒绝（相对/缺 probe、hash 不符、escaping name、既有 evidence）通过，拒绝时没有启动子进程或改写既有证据；五项原生 marker/参数拒绝通过，没有创建 DB。两份实际 probe 都执行最终策略检查。
- 两脚本与 CI run 区块 AST、仓库方式的 vswhere/MSBuild 查找通过。新矩阵已加入现有 pinned runtime CI；本次未运行远端 Actions，第二套为本机 CI 布局复验。
- Rust 1.97.1 fmt、workspace/all-targets Clippy `-D warnings` 通过。workspace 首轮在 worker-panic 子进程三秒退出断言失败；原日志保留。随后单项复跑通过（1.83 秒），完整 workspace 复跑通过。没有修改该断言或生产 watchdog，失败原因仍未定位，不能称偶发问题已修复。

## 身份与封存

LevelDB commit `99b3c03b3284f5886f9ef9a4ef703d57373e61be`；librime 源码检查基于 `33e78140250125871856cdc5b42ddc6a5fcd3cd4`。源码、许可与实际 production provenance 保存在本机归档，用于核对这些固定行为。

| 产物 | SHA-256 |
| --- | --- |
| 首套 recovery probe | `EF3D073A9E4739974F848701902F2A0B5F268E3129F10E8A7F4CBD1D28C443C2` |
| CI 布局 recovery probe | `BF47CA224B42A481D9C5936E57DE04FD19226298B631A94684E527B81F61BCCB` |
| 接受的生产 runtime，未变 | `94D646160F78DFF6408E21DBD0C003CD7D71C5DD6B604AEDAA6CA780F96D93C1` |

[Git 汇总](evidence/WIN10-DB-RECOVERY-20261004.json)、[本机原始汇总](../../build/win10-evidence-clean-v1/DbRecovery-v1/results.json)与[清单](../../build/win10-evidence-clean-v1/DbRecovery-v1/host-evidence-manifest.json)绑定 386 个文件及 20 个外部输入身份，manifest SHA-256 为 `F3F76F198F5BBEDCCC3DE1D3D76676C69BA5E98915D8E7CEE9C938380F9B61FF`。归档包含两套完整合成 DB/日志、两份 probe、构建成功/拒绝日志、源码/CI、pinned 源码与许可、Rust 首次失败及复跑证据；真实用户数据未参与。

首套 harness 的普通子进程上限为 120 秒，之后按开发约束改为 60 秒；第二套使用当前 60 秒版本。归档 `harness-at-first-matrix.ps1` 重建该唯一 timeout 差异，当前完整脚本另行保留；所有正常调用都在原上限内结束。build 归档不提交 Git，外部输入身份是采集时记录。

下一步优先设计并验证 Mo 的用户词典错误策略：拒绝将读取失败当作成功，保留原 DB，避免自动修复/重建绕过错误传播，同时验证实际 Actor/Broker 的准备失败及后续正常恢复。之后再评估日志复用、非同步学习的崩溃边界与 fresh 启动成本；继续以 Win10 安装态和注册宿主为验收目标。本轮仍没有运行新的 TIP/VM 故障恢复验收。
