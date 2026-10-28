# 0200 Win10 升级续验 · 2026-10-28

## 本轮状态

虚拟机恢复有效显示：VBox 报告 1045×750×32、flags=1，桌面截图成功。
上轮 bpp=0/flags=2 与截图 E_FAIL 当前不再复现；没有证据确认恢复的具体原因，
不将其归因于某一次显示布局请求。

普通权限 Session1 重新核验：实际版本 **0.0.18.0**，Broker/设置进程均为 0；
客机 C:/MoDev/Packages/0.0.20.0-v2 的 14 个包内文件及 kit SHA-256 一致。

本轮从新证据目录 C:/MoDev/Evidence/Upgrade0200-v1 发起正式 driver。
只读 host scan 检查 56 个进程，没有发现加载旧 TIP 的进程，但两个进程无法完整检查，
因此按现有策略请求管理员只读复核。截图明确显示
“Windows PowerShell / Microsoft Windows”的 UAC，已请用户确认。

提权调用约 123224 ms 后返回“操作已被用户取消”，本轮**安装器没有被调用**。
不能仅凭这个错误判断用户点击了取消还是提示超时。
返回后桌面截图再次成功，普通权限回执确认实际版本仍0180、Broker/设置仍0、kit14文件仍匹配。
尚未完成 ValidateBase，不能将普通 registrar 最终状态中的 HKCU COM missing 等字段
当作已完成机器安装树核验；final_state_verified=false 如实保留。

## 证据与后续入口

- 主机原材料：build/win10-evidence-clean-v1/Package0200-v2/continuation-20261028-v1。
- 本次 guest driver 使用 Upgrade0200-v1；回传目录沿用 wrapper 名称 upgrade-evidence-v2，
  只表示此次回传文件夹，**不是第二次 guest 升级尝试**。
- 保存桌面/UAC/返回桌面三幅截图、前后回执、driver 状态、只读提权启动记录与 upgrade-state。
  [结构化证据](evidence/WIN10-0200-CONTINUATION-20261028.json) 绑定哈希。
- 已准备 Package0200-v2/installed-validation-tools-v1：使用封存工作目录的 x64/Win32 ABI 探针，
  清单绑定0200双架构 DLL、Broker、设置程序哈希。原生二进制未重编译。
  脚本在客机 Windows PowerShell 5.1.19041.6456 通过 AST 检查，两份共享探针哈希一致；
  **没有执行安装态工具**，避免将0180误验成0200。
- 后续重新发起必须使用全新 Upgrade0200-v2 与独立回传目录，不重跑或覆盖本次证据。
  先重新核验版本和旧 TIP 加载情况，再进行只读复核及安装器两阶段提权。
  computer-use 技能禁止代操作安全权限提示，需要用户处理 UAC。
- 成功升级后再运行配套 ABI/双语言预览，以及任务栏“中/英”、Shift/点击、候选与标点实际验收。
  原始50/400ms超时仍开放；本轮未修改代码、未打包、未构建或启动额外时延矩阵。

封存 stage/kit 清单身份未改变；默认 target 仍不存在，无需 cargo clean。
