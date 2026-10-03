# ADR 0053：MSI major-upgrade 版本比较守卫

## 状态

2026-10-03：已实现并通过 39 项 VM policy 测试。实际链接的 0.0.9.11 → 0.0.9.12 版本对被拒绝，未写完成目录。Win10 的 0.0.9.11 → 0.0.10.0 六阶段回滚/升级矩阵完整通过。

## 背景与决策

构建接受四段产品版本；Burn 能区分第四段，但 Windows Installer 的 ProductVersion/Upgrade 比较仅使用 major.minor.build。此前 verify-linked-upgrade-pair、matrix kit 生成器和 VM kit 校验仅按 .NET 四段 Version 比较，可能把 0.0.9.11 → 0.0.9.12 错记成 major-upgrade 版本对。此前只改变第四段的开发包不能作为有效 MSI major-upgrade 证据。

增加共享的 Get-MoMsiUpgradeVersion 和 Assert-MoMsiMajorUpgradeVersions，按 MSI 前三段严格递增；同时约束 major/minor ≤255、build ≤65535。保留构建器四段版本的接口。host pair verifier、kit 生成器和 guest matrix kit 校验均使用同一规则；kit 仍绑定精确包哈希、相同 stage、不同 ProductCode、固定 UpgradeCode。

不放宽 MajorUpgrade 的同版本策略，验证采用真实递增的前三段版本。

## 验证

- test-vm-lifecycle-policy.ps1：39 项通过；新增有效前三段递增、仅第四段递增、降级、MSI 越界、guest manifest 无效版本对检查。
- 实际 linked 0.0.9.11 / 0.0.9.12 的 verifier 拒绝日志：build/win10-msi-revision-pair-rejection.log；被拒绝的 output completion directory 不存在。
- Win10 六阶段矩阵完整通过，原始 evidence completed=true；两次注入失败、缺失 marker 修复拒绝、基线安装、major upgrade 与卸载均符合预期，十三份日志转移哈希核对通过。详见 [Win10 安装实测](../phase-0/WIN10-INSTALLER-EVIDENCE.md)。旧 clean lifecycle 成功证据不受版本对规则影响。

## 来源

[Microsoft ProductVersion](https://learn.microsoft.com/en-us/windows/win32/msi/productversion) 明确 MSI 的三段格式、数值上限，以及第四段被忽略的升级规则。
