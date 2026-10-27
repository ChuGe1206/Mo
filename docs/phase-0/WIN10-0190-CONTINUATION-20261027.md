# Win10 0190 续验：2026-10-27

用户要求继续开发与 Win10 验证。仓库 develop 为 7b9b735，工作区起始干净。

## 已完成

- 启动此前关闭的 win10 虚拟机；保留 MoEvidence 共享映射，不重启或重复复制客机包。
- 完整核验保留的 0180 v3 stage 138 文件、0190 v2 stage 138 文件、18→19 v2 kit 14 文件。
  文件大小和 SHA-256 均匹配原清单，三份清单身份与 2026-10-14 封存结果一致。
- computer-use 两次画面捕获超时后停止该捕获路径；VBox 截图确认 Win10 锁屏。
- 只读 guestcontrol 返回 Guest 可用、Explorer 会话 0、LogonUI 1、已安装设置 EXE 存在。
- 未启动编译或测试，不重新生成缓存；未操作认证界面或安全提示，未发起安装。

## 接续条件

最后独立验证的安装版本为 0.0.18.0，本轮尚未重新读取安装版本。
已请用户手动解锁并登录；computer-use 技能禁止代操作认证界面。
登录后先核验实际安装基线、客机 0.0.19.0-v2 kit 的 14 文件和升级证据目录新鲜性，
再按原 driver 执行卸载旧 TIP 宿主的 18→19 升级；需要 UAC 时由用户确认。
首次证据目录仍计划为 C:/MoDev/Evidence/Upgrade0190-v1，只有确认不存在后才能使用。
安装后继续双架构 ABI/中英预览及物理快慢 /?、标点、数字/Shift、小键盘和快捷键验证。
非 US/dead/AltGr、G2/G3 和 50/400 ms 验收保持未完成，不用静态转换测试替代实机结论。

[结构化记录](evidence/WIN10-0190-CONTINUATION-20261027.json)；原始截图和核验结果在
build/win10-0190-20261027-v1。本轮只更新文档，执行 git diff --check，不启动缓存重建。

## 用户解锁后的续接

- 用户已解锁并授权继续 VM 操作；截图确认桌面。异步回执核验客机实际 0180、14 文件 kit、
  Session 1 普通权限、LogonUI 0、Broker/设置 0、Upgrade0190-v1 不存在。
- 两次同步 guestcontrol 查询超时退出 20；改为异步共享回执后取得完整校验结果，没有重复复制 kit。
- Upgrade0190-v1 在只读管理员复核时返回“操作已被用户取消”；installer_invocation_attempted=false，
  安装器没有执行。47 进程初扫有未能读取的进程，不能跳过管理员复核。
  [原始取消回执](evidence/WIN10-UPGRADE-0190-V1-CANCELLED-20261027.json) 保留失败与最终状态。
- 新鲜 Upgrade0190-v2 的只读提权也返回取消，installer_invocation_attempted=false，安装器未执行。
  [第二次取消回执](evidence/WIN10-UPGRADE-0190-V2-CANCELLED-20261027.json) 已保留；停止重试，
  等待用户说明是否看到了 UAC。截图通道 E_FAIL，激活窗口后 computer-use 仍 FrameArrived 超时，
  不声称已看到具体权限提示。v1/v2 目录均不可重跑，后续如重发必须使用全新 v3 目录。
- 0190 安装后验证工具已准备：复制当前封存的 x64/Win32 ABI probe，并绑定两架构 TIP、Broker、
  设置 EXE 的 stage 哈希；三份 guest helper 脚本 AST 检查通过。安装完成前不运行这些工具。
- 认证界面和安全提示仍由用户处理；没有修改安全设置或自动确认 UAC。
