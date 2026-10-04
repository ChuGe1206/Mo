# Win10 ABI v3 首键与查询内部计时

日期：2026-10-04。基线 `a3c6bcb`，继续以 Win10 10.0.19045 主机为主。
使用新建合成目录、独立 Broker/TIP 副本和固定 ABI probe；没有操作 VM。
50 ms key、400 ms activation 门槛保持不变，G2/G3 仍未完成。

## 本轮工程变更

- `tools/tip-latency-diagnostic.ps1` 保存双架构 fresh/existing 两个进程、各首轮与第二轮结果；只终止自己启动的进程。Broker readiness 上限 30 秒、probe 40 秒、退出/输出回收 3 秒。首轮结束后等 2 秒收集已超时 dispatch 的完成计时，第二轮不能替代首轮验收。脚本 exit 0 仅表示采集完成，结果中的 probe exit 才表示实际流程成功或失败。
- 脚本从可信机器 shared 数据复制 `user/build`，创建空的 owned profile，不复制真实用户词典或设置。使用 debug `--rime-prepared` 的 **local user/build 计划**；此模式没有设置 installed plan 的 shared prebuilt/staging 字段，不能当作安装态共享数据验收。
- `components-v3.patch` 是当前 ABI v3 源码上的计时补丁，排除旧 components.patch 的 mapped/image prefault 行为；`queries-v3.patch` 再加六个内部 scope：lookup_table、候选 Sort/Peek、has_exact_match_phrase、PrepareCandidate、CheckEmpty。两份补丁均不进入 runtime builder/stage。
- 八项脚本拒绝门、AST 及真实源码补丁 check/apply 已通过；每份补丁应用后重新检查严格 userdb startup 策略。最终 replay 的 14 份源码与第二版编译源码正规化换行后相同。两项检查加入 Windows CI，远端执行结果未验证。

## 实际二进制与路径

基线 DLL 为开发态 ABI v3 `BE5E3E374D9FBE12381A0E3D522079033D85C08F4675F7A9A556A6DC3D30D564`。
独立组件版 DLL 为 `6DE2485D34F9DCE60621CD65481BA822B44F8C759283F37273C320BE3516A2D4`；
增加查询内部 scope 后为 `C4BF71138EF2548BF6C11805433895E41EC4F296904FC1AB067F00C0E7280C85`。
它们分别保留于 `build/win10-abi3-components-v1/dist` 与 `dist-v2`。
当前 compile/source 已是第二版；source-v1 保存第一版改变的源码。

所有样本使用固定 debug + latency-trace Broker 副本 `93211EC1F076AF772DCBCCF75FD2EA711C692D7C2F540FBC31BC4377B266C4EB`。
TIP x64 `A798D962…`、Win32 `366C82E8…`，probe x64 `58CDA8D4…`、Win32 `6E96320A…`。
完整哈希在 Git JSON 与归档中。DLL 仍导出 v3，严格打开、必需 userdb 检查与禁止自动 recovery 保留。
诊断版不使用 prefault、reuse_logs、模拟暖键或输入重放准备；原 stage/已安装 VM DLL 未被覆盖。

## 双架构结果

每行是独立 Broker 进程；existing 复用该架构前一 fresh case 的合成 profile，OS 缓存、顺序、学习和主机负载没有随机化。
首个 dispatch 不等于整个 probe，后续键也可能失败。

| DLL / 架构 / profile | host ready ms | 首个 Actor dispatch µs | 排队 µs | 首轮 exit | 第二轮 exit |
| --- | ---: | ---: | ---: | ---: | ---: |
| 基线 x64 fresh | 2059 | 99051 | 33 | 1 | 1 |
| 基线 x64 existing | 430 | 3361 | 16 | 1 | 0 |
| 基线 Win32 fresh | 914 | 154578 | 17 | 1 | 1 |
| 基线 Win32 existing | 710 | 4209 | 12 | 1 | 0 |
| 组件 x64 fresh | 2440 | 86002 | 16 | 1 | 0 |
| 组件 x64 existing | 464 | 3719 | 42 | 0 | 0 |
| 组件 Win32 fresh | 920 | 168075 | 44 | 1 | 1 |
| 组件 Win32 existing | 864 | 79562 | 7 | 1 | 1 |
| 查询 x64 fresh | 2303 | 223078 | 8 | 1 | 0 |
| 查询 x64 existing | 376 | 5156 | 10 | 0 | 0 |
| 查询 Win32 fresh | 2739 | 128801 | 51 | 1 | 0 |
| 查询 Win32 existing | 492 | 6563 | 12 | 0 | 0 |

合计 24 次完整 probe，13 次 exit 1，11 次 exit 0；有限成功不取消失败。
基线 fresh 的首 N 均在客户端 phase=3 响应头等待、error=1460、约 50 ms 后失败。
x64 existing 首 N 成功但后续 H 失败；Win32 existing 也在 H 失败。
外部 GetTickCount64 的 47/62 ms 粗粒度数值不是 Actor 执行时长。
上述 ready 是父进程启动到收到 listening；包含调度和日志接收成本，且 key 测试在 ready 之后开始，不能据此判定首次拉起成功。

## 新增细分发现

第二版 x64 fresh 首个 native ProcessKey 为 220175 µs，其中：

- 中文 Evaluate 104255 µs；Dictionary::Lookup 99974 µs，其内部候选 `DictEntryIterator::Sort` 99915 µs，进程缺页增量 24、线程 cycles 1170336。
- 英文 Dictionary::LookupWords 80616 µs；两个 Table::QueryWords 为 22945/14429 µs，Prism::ExpandSearch 9578 µs。未覆盖工作仍包含在 self 区间，不能把这些嵌套时间相加。
- 候选 DictEntryIterator::Peek 9662 µs；UserDictionary::Lookup 4182 µs。

Win32 fresh 首个 ProcessKey 125495 µs；UserDictionary::Lookup 36747 µs、英文 LookupWords 55120 µs、Prism::ExpandSearch 30853 µs、Sort 18869 µs（缺页增量 23）。
第一版还保留后续 Evaluate 81631 µs、self 81518 µs 的失败；第二版未在同一条件重现，不能宣称补计时消除了它。

Sort 调用 partial_sort，其比较器访问词表映射中的候选权重；Peek 读取映射词条文本。
计时将长区间定位到这些访问，但没有证明硬缺页、磁盘、Defender 或调度的具体责任。
PageFaultCount 是进程计数、混合软/硬缺页；GetThreadTimes 约 15.625 ms 的量化会给出 0 或跨界增量，cycles 不能直接换成固定频率 CPU 时间。
同步 stderr、额外 API 与 scope 会扰动 wall time，组件数据也不能归给未诊断的基线 DLL。

## 验证、失败与封存

Rust fmt、workspace/all-targets Clippy `-D warnings`、完整 workspace tests 通过；双架构 trace native ABI/非变更 registrar 检查通过。
两版独立 DLL 使用固定依赖与原 strict `/W4 /WX` wrapper 构建；上游源码仍有既存警告，不把整个 librime 构建描述为零警告。
第一次复制 DLL 使用了错误的 lib/Release 路径，实际输出为 bin/Release，修正后运行；查询补丁生成器最初因重复 anchor 在编辑源码前拒绝，修正行锚点后构建。
最终脚本拒绝门与补丁复放通过；所有 owned Broker/probe 已退出。

[Git 结果与归档身份](evidence/WIN10-ABI3-LATENCY-20261004.json)绑定本机 `build/win10-evidence-clean-v1/Abi3Latency-v1`。
封存实际 PE、原始成功/失败日志、两版源码、编译契约、合成 DB/profile 与 harness。
第一版组件 collector 的历史文本从生成器及记录的修改重建，明确标为 reconstructed；最终版本是实际采集副本。

这轮完成测量和诊断工程，尚未修复首键超时。下一步调查排序/解码访问中的等待与缺页，再验证后续键和 fresh 启动预算；不采用人工暖键、不放宽 deadline。
VM 仍为 0.0.11.0，安装态新 ABI、注册宿主、loaded-TIP 升级及登录/重启待做。