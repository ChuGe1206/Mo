# Win10 首键：代码页与映射页组合诊断

日期：2026-10-03。接续[映射页试验](WIN10-MAPPED-PAGE-EVIDENCE.md)，只在 Windows 10 22H2 主机的独立进程和合成数据目录测量。当前开发基线为 `develop` 的 `8f43f33`。VM 仍保留上一轮 0.0.11.0，没有新增 VM 输入、loaded-TIP 或登录验收证据。

## 方法与边界

先用接受的生产 runtime 做完整 TIP 路径的 Lua filter 消融，再在隔离的诊断 DLL 中增加代码页准备。所有完整探针均先启动 owned Broker，等它宣布就绪后才执行，因此首键结果不包含 Broker 拉起成本。每组保留首轮和同一 Broker 的第二轮；两轮之间的 2 秒等待用于排空迟到诊断。首轮失败始终按失败记录。

新的[诊断源码](../../native/librime/diagnostics/README.md)只用于隔离构建：按地址找到当前 DLL，读取每个已提交可读映像页的一个字节，用 `QueryWorkingSetEx` 记录准备前后的 valid 页数。映射页策略仍只读取新打开的只读词库映射。两项准备均不发送 key、改变页保护、锁页或输出内存内容。准备后的页可能再次被回收；计数不能证明具体磁盘、硬缺页、Defender 或调度原因。开启开关的进程均使用有 marker 的合成用户目录。

50 ms key deadline、400 ms TIP activation budget 与首次切换 `<500 ms` 目标均保持原值。该实验没有纳入生产 runtime builder 或安装载荷。操作系统缓存、磁盘负载和运行顺序未固定；诊断日志同步写 stderr，也可能影响测量。

## 完整 TIP：移除 Lua filters 仍失败

生产 DLL 保留两个 Simplifier 与全部 translator，仅从编译 schema 删除七个 Lua filter。按 baseline / no-filter / no-filter / baseline 顺序，用新数据副本运行：

| 条件 | Broker ready | 首次 Actor dispatch | 首轮 exit | 第二轮 exit |
| --- | ---: | ---: | ---: | ---: |
| baseline A | 1,712 ms | 144,982 µs | 1 | 0 |
| no Lua filters A | 2,612 ms | 97,640 µs | 1 | 1 |
| no Lua filters B | 1,530 ms | 171,115 µs | 1 | 0 |
| baseline B | 1,801 ms | 107,623 µs | 1 | 1 |

首次 Actor 队列只有 28–43 µs，四组首轮均失败。此前直接 API 消融的有限改善没有在完整路径中成立，不足以支持修改产品 filters。

## 完整 TIP：代码页与映射页

下表使用同一 `FD5229…` 诊断 DLL、trace x64 TIP/Broker 和新合成用户目录。代码页开关准备了 890 个可读页，其中 652 个具有执行权限，共 3,645,440 字节；两组 image 样本的 valid 页数由 558 增至 890。

| 组别 / 顺序 | Broker ready | 首次 Actor dispatch | 首轮 / 第二轮 exit |
| --- | ---: | ---: | ---: |
| image 对照关 A | 3,340 ms | 55,875 µs | 1 / 1 |
| image 开 A | 1,875 ms | 5,364 µs | 0 / 0 |
| image 开 B | 1,864 ms | 44,744 µs | 0 / 0 |
| image 对照关 B | 1,895 ms | 131,775 µs | 1 / 1 |
| image-only A | 1,629 ms | 135,496 µs | 1 / 0 |
| mapped + image A | 2,018 ms | 4,110 µs | 0 / 0 |
| mapped + image B | 2,299 ms | 3,842 µs | 0 / 0 |
| image-only B | 1,667 ms | 31,067 µs | 0 / 0 |

单独准备代码页四次首轮中有一次仍超时，native `ProcessKey` 为 133,531 µs。组合样本把主要工作前移，但 Broker ready 超过 2 秒。前轮仅映射页准备的 7.1 秒启动、234,976 µs 首次 dispatch 失败仍有效。

再使用默认构建的 x64/Win32 TIP、probe 和 Broker，保持同一个诊断 runtime：

| 默认客户端 | 准备 | Broker ready | 首次 native ProcessKey | 首轮 / 第二轮 exit |
| --- | --- | ---: | ---: | ---: |
| x64 | 关 | 1,480 ms | 446,348 µs | 1 / 1 |
| x64 | mapped + image | 2,431 ms | 1,460 µs | 0 / 0 |
| Win32 | 关 | 1,508 ms | 78,769 µs | 1 / 1 |
| Win32 | mapped + image | 1,866 ms | 2,985 µs | 0 / 0 |

默认 Broker 不输出 Actor dispatch 计时，故该列使用 runtime 的 native 子区间，不能与完整 Actor 耗时直接等同。组合条件在这个 DLL 上四次首轮完整探针通过，包含真实 rime-ice 候选窗、模型鼠标/布局、延迟动作取消、重连及 TSF 终止/恢复断言。它不替代已注册桌面宿主或真实鼠标验收。

## 启动耗时分项

随后补充 session、component、词库、用户词典、OpenCC 与 Lua 初始化计时，编译成独立 `E6B852…` DLL。该版本映像为 891 个可读页、653 个有执行权限的页；不能与前一个 DLL 的页数混用。构建期间曾发生 MSBuild 节点环境冲突和 Windows `StartService` 宏导致的链接失败；禁用节点复用及隔离头文件消除宏污染后构建成功，失败日志仍保留。

四次 Actor 进程使用 Broker 的 schema/Emoji/繁简/禁学习选项，每次两轮合成键，均断言 handled、有预编辑、无 commit，清空并销毁成功。两份新目录各自紧接一次复用进程：

| 条件 | Engine load | Actor input-free prepare | 首个 Session 构造 | 首个 Dictionary::Load | 首个 UserDictionary::Load |
| --- | ---: | ---: | ---: | ---: | ---: |
| 新目录 A | 111,615 µs | 5,415,057 µs | 5,135,793 µs | 4,052,876 µs | 421,307 µs |
| 复用 A | 7,273 µs | 352,222 µs | 250,242 µs | 24,041 µs | 183,226 µs |
| 新目录 B | 6,334 µs | 1,529,506 µs | 1,449,575 µs | 835,621 µs | 401,554 µs |
| 复用 B | 6,000 µs | 333,905 µs | 253,945 µs | 21,420 µs | 193,865 µs |

表中 Session 包含词库和用户词典子区间，不能相加。Actor prepare 包含 Session 创建及随后 OpenCC/image 准备。四组首次 dispatch 为 3,548–3,924 µs；八次合成键均有五个候选。长区间主要落在词库加载和用户词典打开，而非这些样本的 Lua 初始化。新/复用比较同时改变了目录状态和 OS 缓存，不能把差值全部归因于创建用户词典。

用这个新 DLL 和复用 A 目录再做默认客户端完整回归：x64 与 Win32 首轮、第二轮均 exit 0，首次 native ProcessKey 分别为 1,865/2,973 µs；Broker ready 分别为 **1,702/495 ms**。因此 Actor 复用样本的 334–352 ms 不构成稳定的完整启动上界，现有 activation/首次切换门槛仍未满足。

## 可复现材料与下一步

[Git 中的结果汇总](evidence/WIN10-IMAGE-PAGES-20261003.json)保存全部表格数据与原始清单哈希，供远端上下文恢复。[本机原始汇总](../../build/win10-evidence-clean-v1/ImagePages-v1/results.json)与[哈希清单](../../build/win10-evidence-clean-v1/ImagePages-v1/host-evidence-manifest.json)绑定 148 个文件（18 组首轮/第二轮完整探针、四组 Actor 日志、两版 DLL、源码前后快照、构建成功/失败日志和历史 harness）及 166 个外部输入身份。字典、OpenCC、probe/TIP/Broker 的源路径与哈希另外锁定，不复制真实用户数据。原始 build 产物保持本机归档，没有提交到 Git。

| DLL | SHA-256 |
| --- | --- |
| 接受的生产 runtime，未改变 | `94D646160F78DFF6408E21DBD0C003CD7D71C5DD6B604AEDAA6CA780F96D93C1` |
| 代码页诊断 | `FD5229ACB1B51D18476D0174B64A77C22842AE82FDFA45D072D7D5D358E0192E` |
| 增加启动分项的诊断 | `E6B85279359F11DD51F2D52ABF7121AB106C8972F16D31540DB854A4F484D08D` |

保存的 14 文件补丁已在接受源码的独立副本上检查、实际应用，并逐文件核对与编译源码一致（只正规化换行）。新头文件与编译副本一致。诊断构建和上述默认双架构完整探针通过；这次没有 Rust 生产代码变更。2026-10-04 收尾的 Rust 1.97.1 fmt、workspace/all-targets Clippy `-D warnings` 与 workspace tests 通过。测试重建了 `target/debug/mo-broker.exe`，当前 SHA-256 为 `277CEE927571F5521E44ECBFF68CA6B54AD85801DB98C9D924AA8EC2803396C5`；历史完整探针用的是 `65B3BA7FED94E50CBE5978E59D905711629222BDC125176016298EA4061BAA05`。外部输入清单记录采集时的身份，不能拿当前重建文件冒充当时的二进制；148 个归档文件仍一致。

下一步继续细分 `UserDictionary::Load → Db::Open` 与 `Dictionary::Load` 的映射准备成本，分别测量 fresh/existing profile、首次拉起和受控内存压力。任何产品方案须同时满足无输入/session 隔离、50 ms key 与 400 ms activation，之后才进入已注册 Win10 宿主和 loaded-TIP 验收。G2/G3 状态保持不变。
