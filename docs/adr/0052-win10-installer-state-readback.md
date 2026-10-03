# ADR 0052：Win10 安装事务状态回读

## 状态

2026-10-03：已实现；Win10 VM 的 SYSTEM 机器注册/回滚与普通用户 finalizer 事务实测通过。0.0.9.11 开发态 Bundle 的 Win10 clean install/repair/uninstall 完整生命周期通过；升级与失败矩阵、真实输入及其他验收项未完成，G3 仍未整体通过。

## 发现

- PowerShell 5.1 直接调用 GUI Bundle 会在提权窗口完成前返回。原 harness 未等待真实退出，产生假失败；生命周期与矩阵 harness 改为 `Start-Process -Wait -PassThru` 并读取该进程 ExitCode。
- 0.0.9.7 MSI 在 SYSTEM 下实际写入机器 profile/category，却因 TSF category 枚举回读缺失而返回 E_UNEXPECTED，安装及逆操作均失败。
- 改为读取 HKLM 64 位视图的 profile 与双向 category 键后，0.0.9.9 MSI 安装返回 0；Bundle 随后因当前用户 finalizer 返回 E_UNEXPECTED 而回滚。
- `InstallLayoutOrTip` 实际成功启用 profile，但同进程的 `GetProfile` 返回旧 enabled 位。重新创建 TSF manager 也未解决。另进程 status 能看到启用，证明不能把同进程 TSF 缓存用作事务回读。

## 决策

机器事务仍调用 TSF API 变更状态；回读使用 HKLM 64 位视图的确切 profile 键和两条 reciprocal category 映射。两条映射不一致时拒绝继续，不直接修改 CTF 注册表。

当前用户启用/禁用仍使用从 System32 加载的 `input.dll!InstallLayoutOrTip`；前态和后态使用同 DLL 的 `EnumEnabledLayoutOrTip` 枚举用户设置。结构 ABI 按 Microsoft 文档声明，匹配 profile type、语言、CLSID、profile GUID，排除 LOT_DISABLED 位。枚举设置有数量上限、额外容量及有限重试，拒绝截断快照。`status` 的 enabled 位采用同一查询。

保持当前用户事务 receipt、标准用户 token 检查、安装目录/双架构机器 COM 校验、不抢默认输入法等契约。`com.x64/x86` 输出仍专门表示 HKCU 注册，安装后 missing 是检查没有用户 COM shadow；不是机器 COM 缺失。

## 验证与证据

- `native/windows-tip/build-probe.ps1 -Architecture All -Backend MSBuild`：两架构零警告/错误，ABI、机器事务 policy、用户 finalizer policy probe 通过；增加禁用/default 标志、外来语言/CLSID/profile/type 判别。
- Win10 22H2 build 19045 VM，普通用户调用新 registrar：安装回滚、安装、修复、移除、移除回滚、再次移除共六项均 exit 0，enabled/marker/receipt 均与前态恢复契约一致。
- 本地原始证据：`build/win10-evidence-clean-v1/probe`、`V3`、`V099`、`enum-finalizer-transaction.txt`；构建日志 `build/win10-enum-build.log`。
- 这些检查不证明真实桌面输入、升级矩阵、签名发布或 Win11 支持。旧失败日志继续保留。

## API 来源

- [Microsoft EnumEnabledLayoutOrTip](https://learn.microsoft.com/en-us/windows/win32/tsf/enumenabledlayoutortip)：ABI、参数、计数语义，以及 IME 语言可能返回未启用项目的注意事项。
- [Microsoft InstallLayoutOrTip](https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip)：启用/禁用契约及动态加载要求。
- [Google Mozc input_dll.h，blob 208d9d1b6fbb18515c66d4be9ad1dc6208a1e6e8](https://code.googlesource.com/mozc/+/refs/heads/master/src/win32/base/input_dll.h)：交叉核对公开 SDK 未定义的 LOT_DISABLED=0x0002 常量；未复制上游实现代码或资产。

## 0.0.9.10 生命周期复测

MSI ICE、Bundle 反向解包与内嵌 payload 校验通过。Win10 VM 的 Install/Repair/Uninstall 均 exit 0，安装/修复后文件哈希、ACL、双视图机器 COM、默认/active 未切换检查通过；卸载后 profile/COM 缺失、用户 marker 删除，但 `Program Files\Mo` 空根目录仍在，完整 harness completed=false。原始证据 `build/win10-evidence-clean-v1/V0910/vm-lifecycle-evidence.json`，不得改写为成功。

原因：回滚 marker 必须保留到 commit，导致 RemoveFolders 阶段不能删除根目录；commit 删除 marker 后仅剩空根。修复是在机器 remove commit 成功删除 marker 后，对同一固定 Program Files 根调用非递归 RemoveDirectory；拒绝 reparse/file，未知内容保留。清理失败仅报告 HRESULT，不在 receipt 已删除后再触发不可恢复的 commit rollback；harness 继续严格检查无残留。机器事务 probe 增加未知文件保留、空目录删除、已缺失目录幂等检查。该修复已由下述 0.0.9.11 完整生命周期复测确认。

## 0.0.9.11 完整生命周期通过

2026-10-03，Win10 22H2 VM 从普通用户 token 启动哈希锁定 kit，Install/Repair/Uninstall 三阶段均 exit 0。harness 记录 completed=true、install_default_unchanged=true、install_tree_security_audited=true、failure=null。安装与修复后文件哈希/完整 inventory、ACL/owner/hardlink/reparse 检查、双架构 HKLM COM、用户 marker/receipt 均通过；卸载后安装根目录消失，profile 未注册/启用/激活，机器 COM 和用户 marker 均缺失。保留预期 remove-v1-enabled receipt 供后续逆操作恢复，不把它当作残留失败。

- stage：`build/mo-windows-stage-win10-clean-root-v1/stage`，SHA-256 `5B5AEDE16BC1DBE1A0495AE54A316AB3B466035EDB4F44CB8D3DEBF134DE1E56`。
- Bundle：`build/mo-linked-win10-clean-root-0911-v1/mo-setup-development-unsigned.exe`，SHA-256 `53CA5601F25A83190BB5BFCEA777CECA39C9ED77EC809AFFB1F36094C552AB2A`。
- MSI：SHA-256 `FDEB685FD9BDFD9656F00B173661DFC8A91ECA1BFBB3E261C45AD077539CA5A6`。MSI ICE 与 Bundle 反向解包/payload 校验通过。
- 原始证据：`build/win10-evidence-clean-v1/V0911/vm-lifecycle-evidence.json` 和同目录六份日志，转移到主机后按 guest evidence 中的 size/SHA-256 逐份验证通过。
- 源码验证：双架构 MSBuild/probe 零警告错误，新增目录保留/删除测试通过；VM policy 34 项通过；git diff --check 通过。

这是一台 Win10 VM、未签名 DevelopmentTest 包的完整有限样本。未覆盖升级矩阵、MSI/Burn 注入失败回滚、真实桌面输入/宿主/登录重启、发行签名或 Win11。G3 不升级为整体通过。
