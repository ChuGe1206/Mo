# Win10 0.0.15.0 旧候选皮肤排查 · 2026-10-12

## 原因与证据

用户截图中的记事本是 10 月 4 日启动的旧进程（PID 868），跨越了后续两次升级。
当前磁盘 TIP/Broker 与 0.0.15.0 封存载荷一致，设置为 Bamboo、横排。
但该记事本的映射模块大小仍为 229376 字节，与 0.0.11.0 的 PE SizeOfImage 一致；
新建自有合成记事本（PID 8428）的映射大小为 372736 字节，与 0.0.15.0 一致。
同一次客机只读扫描同时发现 explorer/conhost 仍为旧大小，msedge 为新大小。
这支持旧宿主保留旧 DLL 的判断；文件路径的磁盘 SHA256 不能证明内存映射版本，
SizeOfImage 也不是内存逐字节身份校验。没有依据将此问题归因于 VM 性能。

[只读扫描回执与边界](evidence/WIN10-TIP-HOST-0150-20261012.json)。
原始回执位于 build/win10-evidence-clean-v1/TipHostPreflight0150-v1/guest-tip-hosts.json。

## 修复

changed-payload 升级 preflight 原先仅检查 Broker/设置进程，遗漏加载 TIP 的宿主。
新版脚本在安装器调用前保存当前用户/会话的模块扫描；发现加载 Mo 的进程即拒绝，
明确要求先保存文档、关闭受影响应用；shell 持有 DLL 时需保存后注销。
已识别为当前用户但无法读取模块的进程也拒绝，避免当成空清单。
进程所有者无法读取的情况单独记录，其他账户/会话不属于此检查范围。
这不是全系统无 DLL 证明，也未解决扫描之后的进程创建竞争或完整 loaded-TIP 升级矩阵。

回执格式升至 3，记录 tip_host_scan；封存的 0.0.15.0 kit/包及历史回执未修改。
新检查用于后续新生成 kit，不宣称已更换当前安装器。检查只读模块元数据，
不读取文档/命令行/密码、不终止进程、不改变默认输入法或安全策略。

## 验证与当前边界

- PS7 与 WinPS5.1：各 10 项宿主检查及 22 项升级事务检查通过；另有 PS7 升级 kit 政策 34 项通过。CI 已加入双 shell gate，远端结果待回执。
- Win10 客机直接运行新扫描：发现 5 个加载 TIP 的进程，拒绝原因包括旧记事本；未调用安装器或终止进程。
- 原记事本含未保存文档，保留不动；新建合成 fixture 位于
  C:/MoDev/Evidence/Candidate0150-20261012-v1/Mo-new-window.txt。
- 新记事本已加载新版模块，但本轮没有实际输入。computer-use 图像捕获返回
  FrameArrived timed out: timed out waiting on channel；可访问性读取成功，
  点击画布返回 coordinate input geometry is unavailable。VBox 自带截图可读，
  未绕过 UI 输入限制或把 DLL 加载当作新皮肤/翻页/表情交互验收。
- 下一步先保存并重开旧文档；shell 仍驻留旧 DLL 时保存所有工作后注销/重启，
  再继续新版候选翻页、展开、Emoji 上屏/Esc 与设置即时重绘实测。
- 本轮未构建 Rust；默认 target 仍不存在，未为清理而重建。封存产物保留，G2/G3 与 50/400 ms 不变。
