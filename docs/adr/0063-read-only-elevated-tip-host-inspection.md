# ADR 0063：开发虚拟机的只读管理员宿主复核

## 状态

已接受；2026-10-12，develop。仅用于一次性 Windows 开发虚拟机的 Changed 升级工具。

## 原因

用户重启 Win10 后已没有观察到 Mo TIP 加载，但普通权限无法读取 ctfmon、ChsIME 的模块。
提高权限后这两项可读；反恶意软件轻量保护进程即使管理员也不能枚举模块。
直接按名称跳过、忽略所有读取失败或把整个升级 driver 提权，都会削弱现有合同。

## 决策

- 已观察到已加载 TIP 时仍立即拒绝；只有没有已加载 TIP、但本用户本会话模块读取失败时，
  才请求同一 SID、同一会话的管理员只读 worker。UAC 由用户确认。
- worker 仅检查进程归属、模块、保护级别和已安装 DLL 签名，并写审计回执。
  它不调用安装器、不终止进程、不修改安全设置。原普通权限 driver 校验后继续安装。
- 模块枚举失败的进程仅在 OS 查询明确返回 ANTIMALWARE_LIGHT，且安装目录的 x64/x86 TIP
  均为 NotSigned 时记录为保护级别例外。签名代码、其他保护级别或查询失败继续拒绝。
  无进程名白名单；已观察到加载始终拒绝。
- 保护查询使用 GetProcessInformation / ProcessProtectionLevelInfo 和
  PROCESS_QUERY_LIMITED_INFORMATION。反恶意软件进程的代码完整性限制依据
  [Microsoft 保护服务文档](https://learn.microsoft.com/en-us/windows/win32/services/protecting-anti-malware-services-)；
  [保护级别结构](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-process_protection_level_information)
  描述返回值。由这些限制推断本开发包的未签名 TIP 不能加载到该保护进程，例外不适用于正式签名包。
- 回执保存到 ProgramData/MoInstallerTest/HostAudits 的随机 nonce 子目录。
  目录和文件必须由 Administrators/SYSTEM 所有，普通主体没有写、删除或权限变更权限，
  路径不得含重解析点。读取上限 256 KiB，格式严格检查。
- 回执必须匹配当前 nonce、发起 SID/会话和 worker SHA256，elevated 为布尔 true，
  时间为 UTC 且不早于 30 秒、最多未来 2 秒。拒绝过期、身份错配、可写或失败扫描回执。
- driver 证据格式升为 4，新增 tip_host_inspection；kit 增加两个脚本，维持 format 1
  和完整文件哈希清单。旧 kit 不覆盖，程序 stage、MSI/Burn 及用户数据不变。

## 验证与边界

PS7 与 Windows PowerShell 5.1 分别通过 14 项扫描、26 项审计/启动适配、22 项事务测试；
PS7 kit 34 项通过。新审计测试接入两个 CI shell，远端 CI 尚未核验。
真实 Win10 worker 和普通权限 0.0.15.0→0.0.16.0 升级通过；详见
[安装态证据](../phase-0/WIN10-INSTALLED-0160-20261012.md)。

扫描仅覆盖已识别的当前用户、当前会话，不能证明其他账户/会话完全不存在宿主，
也不是持续锁定宿主启动的机制。审计在 preflight 时检查，后续用户仍须避免启动 Mo 应用。
测试 kit 及其脚本由开发者信任和哈希核验；该桥接不构成生产环境的独立安全代理。
首次 worker 参数拆行错误、短 guestcontrol 请求失效及一次 UAC 取消均保留失败材料，
最终采用完整参数 splat 与持久启动通道；没有通过降低 UAC 或安全防护完成升级。

## 2026-10-13 生命周期补强

worker 必须正常退出且 ExitCode=0，之后才读取回执；等待异常/超时不接受回执。
UAC 返回后完成预算仍为 120000 ms，使用带单位的具名常量。
提前获取并最终 Dispose 所属进程句柄，Dispose 不终止 worker。
两 shell 30 项及 Win10 普通合成子进程 0/7 通过；当前补强版本未重新执行提权 worker。
新 v4 kit 与原 v3 升级证据分别保存，详见续测记录。
