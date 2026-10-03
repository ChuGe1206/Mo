# Win10 native 首键延迟组件诊断

日期：2026-10-03。接续 [0.0.11.0 包与桌面有限实测](WIN10-PACKAGED-0110-EVIDENCE.md)。本轮在 Windows 10 Pro 22H2、19045.6466 主机上执行，不操作 VM；上一轮 VM 状态仍记录为已安装 0.0.11.0。生产代码、已验证的包和 50 ms key deadline 未变。G2/G3 不升级为整体通过。

## 隔离与复现

源码从 `build/mo-runtime-learning-v1/inputs/librime` 复制到 `build/runtime-component-trace-v1/source`，复用已验证依赖的只读路径，输出独立 `dist`。没有改动 accepted runtime 的 inputs/dist。诊断通过同一 `latency_probe` 及隔离 TIP/Broker；每次使用新建、带 fixture marker 的编译用户数据副本。

所有准备调用保留两个 Simplifier 和无输入 anchor，并断言 context 不变、没有 commit、未 composing。只有测试会话接收固定合成输入，直接 API 每组 20 个会话 × 2 次输入，断言 handled、无 commit、context/status、清空和销毁。17 组共 **680 个直接 API 合成键断言**通过，不能算实时 50 ms 指标通过。日志只记录时间、组件名、身份和计数。

诊断三个阶段的 DLL 身份见以下 SHA-256；原生产运行库 SHA-256 仍为 `94D646160F78DFF6408E21DBD0C003CD7D71C5DD6B604AEDAA6CA780F96D93C1`。

| 阶段 | 诊断 DLL SHA-256 |
| --- | --- |
| v1 嵌套 wall/self 计时 | `F684DDC619E382C5AEF3E23B37CBA9F9205876219172DAAEBD64B40066F37451` |
| v2 加线程 CPU / 进程缺页 | `6F21F0FB53DAAD56457BC885FE54D1829C11E04DBBDC3B40A0C1825481A631C2` |
| v3 加 native 查询细分；v4/v5 复用 | `3096DADF167A3C2EB5319093D753DB563A762073F6A99B4756C5899F01BB8B8A` |

## 配置消融不足以归因

在测试副本中分别移除 Lua filter、Lua translator，以及七个单独 filter；配置中两个 Simplifier 不动，均使用相同的 prepare-resources 入口。每组完整日志与配置 SHA 均保留。

| 配置，按本轮部分执行顺序 | 首次 process_key / µs |
| --- | ---: |
| baseline | 317,857 |
| no-lua-filters | 1,455 |
| no-lua-translators | 98,101 |
| no-lua-filter-translator | 18,018 |
| baseline-repeat | 12,367 |

同一完整配置 baseline 首次约 318 ms，而序列末重测约 12 ms；未控制 OS 文件/代码缓存与调度状态。单次移除 filter 的快慢不能作为因果结论，也没有删除产品中的 Lua 功能。全部单项结果见 results.json。

## 明确缩到查询函数

| 样本 | 首次 process_key | 中文 Query | melt_eng Query | IPC / Actor |
| --- | ---: | ---: | ---: | --- |
| 直接 v1 | 240,647 µs | 141,157 µs | 91,029 µs | 不经 IPC |
| TIP→Broker v1 | 144,852 µs（内部） | 96,009 µs | 28,694 µs | 首次 probe exit 1；Actor dispatch 148,011 µs、queue 42 µs |
| 直接 v2 | 71,168 µs | 34,903 µs | 34,803 µs | 不经 IPC |
| TIP→Broker v2 | 120,709 µs（内部） | 60,140 µs | 33,619 µs | 首次 probe exit 1；Actor dispatch 122,503 µs、queue 22 µs |
| 直接 v3 | 216,097 µs | 53,254 µs | 148,557 µs | 不经 IPC |
| TIP→Broker v3 | 见原 trace | 76,450 µs | 46,338 µs | 首次 probe exit 1；Actor dispatch 160,818 µs、queue 11 µs |

每个 Broker 在首次失败后等待 2 秒收集已发生的 dispatch，随后第二个完整 probe 均 **exit 0**，覆盖该探针的候选窗、鼠标调用、布局、取消、重连与终止/恢复断言。这是同进程资源已使用后的有限成功；没有扩大 deadline，没有把首次 exit 1 覆盖掉，也不是 VM 人工鼠标验收。v1 客户端等待响应头约 50.8 ms 超时，原失败日志保留。

v3 将耗时继续细分：直接样本 `Prism::CommonPrefixSearch` 39,275 µs、中文 `Table::Query` 13,678 µs；英文 `Dictionary::LookupWords` 147,435 µs，其中 `Table::QueryWords` 两段为 50,296 / 6,329 µs，剩余未细分部分约 90,443 µs。TIP 样本还记录 `UserDictionary::Lookup` 26,550 µs、`Prism::ExpandSearch` 7,984 µs。Lua corrector/pin 等也有 10–17 ms 的样本，不能断言全部长耗时只来自某一个词库。

这些长区间的 `GetThreadTimes` 差值常为 0，且进程 PageFaultCount 增加；例如直接 v3 ProcessKey 216,088 µs、CPU 差值 0、缺页增量 131。**0 是计时差值，不是证明未执行 CPU 指令**：观测中 CPU 计数以约 15,625 µs 跳变，甚至短区间会跨过一个计数更新。缺页是进程级计数，不能区分软/硬缺页，不能单独证明磁盘、Defender 或调度的根因。当前证据支持继续检查读取/映射和等待，而非直接认定 Lua 计算过重。

诊断只输出 ≥500 µs 的 Scope；self 扣除已计时子 scope 的 wall，仍含未计时函数、等待以及诊断开销。不能把其绝对时间当成发行性能基准。

## 文件预读试验仍未解决

v4 在启动 engine 前只读取新测试副本的 17 个编译 `.bin` 文件（76,146,808 bytes），没有 key/commit 调用。预读耗时 5,777 ms，随后首次 process_key **65,446 µs**。紧接的新副本 v5 不预读，首次 **66,955 µs**；两者后续单键最大为 2,342 / 1,509 µs。

这一个顺序样本既不能证明预读没有任何作用，也没有证成修复。整文件预读增加了准备时间且仍超过 key deadline，因此没有加入产品。用户词典、native/Lua 代码页、实际映射页驻留与 OS 调度仍需分开验证。

## 原始证据与下一步

证据根 [LatencyComponents-v1](../../build/win10-evidence-clean-v1/LatencyComponents-v1/results.json)：105 个文件被 [host manifest](../../build/win10-evidence-clean-v1/LatencyComponents-v1/host-evidence-manifest.json) 绑定，含 17 组直接日志、3 组失败/再次成功完整 TIP 日志、runtime provenance、编译 fixture manifest、三份诊断 DLL、构建日志、诊断源码 diff 与执行脚本。新增 Mo 诊断源码沿用仓库 Apache-2.0 许可，诊断 DLL 仅供本地试验。

下一步继续 Win10：针对实际只读映射页、用户词典读取和代码首次使用的等待细分，必要时增加只针对 owned 测试进程的高分辨率 CPU/等待证据；资源准备实验必须无输入、保持 session 隔离并衡量 ready/激活成本。找到有效变更后重新跑严格 50 ms 的完整真实词库与双架构检查。浏览器/WinUI、实际鼠标、忙预编辑设置切换、混合 DPI/多屏和 loaded-TIP 完整升级/登录矩阵仍待完成。

## 2026-10-03 映射页试验续查

逐页读取已在直接 API 样本中把首键从 401/155 ms 降到 1.5 ms，但完整 TIP→Broker 首轮仍超时，Broker 就绪为 7.1 秒；Actor 样本也出现 741 ms。结论与原始证据见 [映射页试验](WIN10-MAPPED-PAGE-EVIDENCE.md)。本节此前的组件定位仍有效，映射页预读不能据此作为产品修复。
