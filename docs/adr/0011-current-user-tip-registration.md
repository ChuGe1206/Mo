# ADR 0011：开发态 TSF 注册分层与双架构 COM 视图

- 状态：接受
- 日期：2026-09-15

## 背景

Windows 不会仅凭 TSF language profile 找到 Mo TIP。一个可被宿主加载的键盘 TIP 同时依赖三类状态：标准 COM 进程内服务器注册、TSF profile、以及 `GUID_TFCAT_TIP_KEYBOARD` category。64 位 Windows 又将进程内 COM activation 数据分为 x64 和 x86 注册表视图；两个视图必须分别指向位数匹配的 DLL。

旧 registrar 只调用了 `ITfInputProcessorProfileMgr::RegisterProfile`、category API 和 `InstallLayoutOrTip`，没有注册 COM activation。因此即使 profile 出现在用户输入法列表中，宿主也无法通过 CLSID 创建 `MoTip.dll`。

## 决策

- 开发态 COM 注册写入 `HKCU\Software\Classes\CLSID`，不要求管理员权限，也不改整机 HKLM 状态。
- 一个 x64 registrar 使用 `KEY_WOW64_64KEY` 与 `KEY_WOW64_32KEY` 显式写入两个视图，分别绑定 x64 与 x86 DLL；不直接拼接 `Wow6432Node` 物理路径。
- `InprocServer32` 的 `ThreadingModel` 固定为 `Apartment`，与 TIP 当前 COM/TSF 生命周期模型一致。
- 注册只接受存在的绝对文件路径。已有相同路径视为幂等；已有不同路径立即失败，避免静默劫持或覆盖未知注册。
- profile 的 `bEnabledByDefault` 设为 `FALSE`。注册与加入当前用户输入法列表保持为两个显式阶段，绝不把 Mo 设为默认输入法。
- registrar 提供只读 `status`；隔离自测使用独立测试 CLSID，在两个真实 COM 视图写入、读回并清理，不触碰 Mo 正式 CLSID 或 TSF profile。
- 正式 MSI 仍由 per-machine 组件拥有 HKLM 双视图，用户态 finalizer 只负责 profile/启用。安装、修复和卸载事务不能依赖开发态 HKCU 命令。

## 结果

开发构建现在具备系统宿主加载前所需的 COM 注册原语，并能重复验证双视图与清理逻辑。它尚未证明注册后的 Notepad/WinUI 激活，也尚未形成 Setup 的跨提权事务；这些仍属于 G2/G3 后续验收。

## 依据

- [Microsoft：Text Service Registration](https://learn.microsoft.com/en-us/windows/win32/tsf/text-service-registration)
- [Microsoft：InprocServer32](https://learn.microsoft.com/en-us/windows/win32/com/inprocserver32)
- [Microsoft：32-bit and 64-bit Application Data in the Registry](https://learn.microsoft.com/en-us/windows/win32/sysinfo/32-bit-and-64-bit-application-data-in-the-registry)
- [Microsoft：HKEY_CLASSES_ROOT Key](https://learn.microsoft.com/en-us/windows/win32/sysinfo/hkey-classes-root-key)
- [Microsoft：InstallLayoutOrTip](https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip)
