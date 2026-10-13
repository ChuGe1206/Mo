# Win10 0.0.16.0 续测与审计质量补强

2026-10-13，develop。[结构化证据](evidence/WIN10-CONTINUATION-20261013.json)。

## 安装态验证与用户反馈

封存 source 的 x64/Win32 ABI 探针对 Program Files 中实际安装的双架构 TIP 通过，
包括加载、导出、类工厂、Ex/key sink 生命周期和卸载；退出码均为 0，原真实 Broker 保留。
这不是完整输入交互夹具，也没有启动 fake Broker 或操作用户文档。
computer-use 两次 capture 分别 FrameArrived/window capture 超时，停止重试。

用户在新空白记事本反馈：五候选完整、箭头翻页、笑脸面板和 Esc 正常。
仍存在：左右键改变引擎编辑位置但可见光标不动、翻页快捷键不明确、标点先进入文档而候选残留。
下一步修复逻辑键翻译和预编辑光标/分段元数据；左右键继续承担拼写编辑。
当前默认词库配置支持 PageUp/PageDown、-/= 翻页，须在修复后实际核验。

## 审计桥接补强

completion 等待保持 120000 ms，归为具名常量，范围仍从 UAC 返回后起算。
非零 worker 退出码拒绝；等待异常/超时和成功都释放所属 Process 句柄，不终止 worker。
先获取 Handle，保留进程结束后的退出状态。依据
[Microsoft WaitForExit 生命周期说明](https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.waitforexit?view=netframework-4.8)。
Win10 PS5 合成子进程退出 0/7 均正确读取。PS7/PS5 各 30 项包含退出失败、等待异常、
无效回执和复制失败；扫描 14、事务 22 项/双 shell、PS7 kit 34 项通过。
fmt、默认/latency-trace strict Clippy、workspace tests 通过，统计见结构化证据。

新封存 kit 为 build/mo-vm-changed-upgrade-kit-0150-0160-v4；旧 v3 安装回执及程序载荷保留。
本轮没有安装 v4、再次执行管理员 worker、改动已安装版本或修改 UAC。
已有成功升级的 VM 不可直接重复运行以 0150 为基线的 driver。

## 失败历史和边界

首次 ABI 包装用 Start-Process 的 Process 对象得到空 ExitCode，探针 stdout 本身报告通过；
保留 guest-evidence/error.json。最终改用直接 Diagnostics.Process，取得两个真实 0 退出码。
kit 包装首轮相对路径被工具正确拒绝，改绝对路径后封存通过，首轮日志保留。
G2/G3、真实首键 50/400 ms、加载宿主升级矩阵、物理多 DPI 和升级后重启仍开放。
默认 target 暂留给正在推进的输入修复，在完整构建/验证里程碑后按 idle/path 门禁清理。
