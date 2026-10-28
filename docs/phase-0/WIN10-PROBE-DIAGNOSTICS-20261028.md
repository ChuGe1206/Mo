# Win10 烟测失败证据保全 · 2026-10-28

## 变更

`tools/tip-rime-smoke.ps1` 的 trace 失败路径原先立即结束自有 Broker。
异步日志线程可能仍在写入，旧 `mode-trace-broker.log` 只留下半行 `MO_LATENCY op=`，
不足以判断 Actor 队列与引擎处理耗时。

新增 `probe-broker-diagnostics.ps1`：在探针已经返回失败后，仅 trace 模式允许
1000 ms 诊断等待，然后终止调用者创建的确切子进程；退出等待 3000 ms、
stderr drain 1000 ms，保留最后 65536 字符并标记截断。
原始探针失败与诊断清理失败分别保留，普通输出也持久化失败摘要。
这些是烟测清理预算，不改变输入法 50/400 ms、IPC、引擎行为或安装载荷。

## 验证与定位

- `tools/test-probe-broker-diagnostics.ps1` 33 项通过：真实合成子进程的延迟/半行输出、
  挂起后的有界停止、非 trace 清理、大日志尾部保留；提取生产烟测函数实际执行失败路径，
  确认退出码 7 原因和延迟元数据均保留，临时用户目录回收；注入目录清理失败时，
  原始异常继续抛出，清理失败另行持久化；探针成功但清理失败仍拒绝通过，
  仍在运行的子进程数据不删除。
- 已加入 Windows CI。初轮测试断言均通过，但预期失败的子进程遗留 LASTEXITCODE=7，
  外层误报测试失败；脚本现仅在全部断言和清理完成后明确 exit 0。首轮日志保留。
- fmt、默认及 latency-trace strict Clippy、workspace 207 passed / 1 ignored。
  本轮未修改 Rust/C++，未重复原生双架构构建；使用已有 x64 trace 原生探针作定向检查。
- 一次真实 Rime / TSF 定向执行仍失败，完整证据成功保留：request=11、kind=5，
  响应头等待 50652 us、总时长 50674 us；对应无并发连接的最后一个 dispatch
  queue=8 us、engine=288011 us、dropped=0，之后 destroy 正常完成。
  因此本次超时的主要区间在 Actor 的引擎处理内，而非 Actor 排队。
  引擎区间含 backend apply 和快照构造，也包含线程被调度出去的时间；
  **尚不能区分 librime 计算、文件/缺页 I/O、调度等具体原因**，不宣告性能通过。
- 本轮只做一次真实 Rime 定向运行，没有再次扩大时延验证矩阵。
  后续需要时细分 native process_key、context/status 与快照阶段，再决定修复方向。

原始日志位于 build/probe-diagnostics-*-20261028*；
[结构化记录](evidence/PROBE-DIAGNOSTICS-20261028.json) 保存哈希及验证边界。

## VM 与后续

VM 在线，UI 捕获首次 FrameArrived 超时，重新获取窗口后仍捕获超时；
VirtualBox setscreenlayout primary 1045×750×32 返回成功，但报告仍 bpp=0/flags=2，
截图仍 E_FAIL。本轮没有发起 UAC、安装或重启，没有改变安全设置。
实际安装的最新核验仍为 0.0.18.0，待升级包为客机 C:/MoDev/Packages/0.0.20.0-v2。
本轮只变更开发工具，不重新生成安装包；真实任务栏中英文图标验收仍待完成。

下一步优先恢复可观察的 VM 桌面并完成 0180→0200 安装态验证；
图形通道未恢复时继续可独立开展的输入法功能开发，避免重复发起不可见提权。
本轮测试结束后回收默认 target 与本轮合成 Rime 用户目录约 **0.91 GiB** 逻辑文件占用，
安装包、stage、源码、日志、探针、0180 基线及待升级 kit 保留且保护哈希不变。
[清理记录](evidence/PROBE-DIAGNOSTICS-CLEANUP-20261028.json) 保留路径、时间和释放空间。
