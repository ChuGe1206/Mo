# ADR 0047：x86 COM 注册路径与 MSI ICE69

## 状态

已实现并通过当前 Windows 主机的 MSI ICE 静态校验；真实安装、修复和升级仍待隔离 VM 验证。

## 背景与决策

Mo 把 x86 TIP DLL 固定安装在 `Program Files\Mo\tip\x86`，由 64 位文件组件拥有；对应的 HKLM 32 位 COM 视图必须由独立的 `always32` 注册表组件写入。原值 `[#TipX86File]` 跨组件引用文件，WiX 的 ICE69 报告警告。Windows Installer 对文件引用的解析还依赖被引用组件的安装动作状态，在修复或升级时可能得到空值；见 [ICE69](https://learn.microsoft.com/en-us/windows/win32/msi/ice69) 与 [Formatted](https://learn.microsoft.com/en-us/windows/win32/msi/formatted)。

注册表值改为 `[INSTALLFOLDER]tip\x86\mo-tip.dll`。`INSTALLFOLDER` 是 MSI Directory 表中的安装根，注册表写入发生在 `CostFinalize` 之后，此时目录属性以分隔符结尾；见 [Using a Directory Property in a Path](https://learn.microsoft.com/en-us/windows/win32/msi/using-a-directory-property-in-a-path)。文件归属、安装目录及 32 位注册表视图不变。生成器、作者层负向测试和链接后反编译验证均锁定此值。

## 证据与限制

使用当前 132-file stage 链接的未签名 ProductionShape `0.0.9.1` MSI 经 WiX 4.0.6 `msi validate` 返回 0，且没有警告或错误；Bundle 内嵌 MSI 字节已反向核对。此静态检查不证明 Windows Installer 在真实机器上的注册、修复、升级、回滚或卸载行为，G3 仍未通过。
