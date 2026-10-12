# Win10 0.0.14.0 实际升级 · 2026-10-12

在现有 Win10 VM 使用已核验的 Package0140-v1/kit 完成普通权限发起的
0.0.11.0 → 0.0.14.0 变更载荷升级。用户手动确认安装器 UAC；本机未安装。
虚拟机测试账户凭据仅用于本地 guestcontrol，不写入开发证据或 Git。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| 安装前真实基线、载荷、ACL、机器 COM、用户 profile | 通过 |
| 后台旧 Broker | 确认 Mo 未激活后停止已核验的安装路径进程；等待其退出后启动升级 |
| 安装器 | exit 0 |
| 安装后载荷、ACL、机器双视图 COM | 通过 |
| 原设置文件和默认输入法 | 升级前后保持不变 |
| 最终 Installed 状态 | final_state_verified=true，failure/final_state_failure=null |
| 回传两份安装日志 | size/SHA256 与客机回执完全一致 |
| C:/MoDev 归档 | 17 个旧来源不存在，全部归档目标存在，与回执一致 |
| 桌面输入、loaded-TIP、重启 | 本轮未执行 |

[回执字段副本](evidence/WIN10-UPGRADE-0140-20261012.json) 按仓库格式保存，字段值未改。
原文件与两份安装日志保留在 build/win10-evidence-clean-v1/ChangedUpgradeGuest0140-v1；
原始及格式化副本 SHA256 见 [证据清单](evidence/WIN10-WORKSPACE-20261012.json)。

本次仍为 unsigned DevelopmentTest，包含开发态故障注入。
包、stage、kit 的版本与哈希均与既有 0.0.14.0 记录匹配，没有替换已封存载荷。
当前虚拟机安装基线已更新为 0.0.14.0；原有首键负向结果、50/400 ms 与 G2/G3 不变。

下一步检查安装态设置、候选翻页、独立表情/Esc、多 DPI/高对比度和真实宿主。
本次安装结果不覆盖随后进行的源码常量与质量规则改造；那些变更须独立验证后再打包。
