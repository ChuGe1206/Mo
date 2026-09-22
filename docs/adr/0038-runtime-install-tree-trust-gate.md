# ADR 0038：Broker 运行时安装树信任门

## 状态

已接受（代码和非安装回归已闭环；真实 Program Files 安装启动仍待一次性 VM）

后续 ADR 0039 把 staging/prebuilt 与 Lua 模块入口一并固定到本信任门审计的机器安装树；本文提到的用户 `build` 创建步骤已取消。

## 背景

ADR 0037 已让一次性 VM 在 install、repair、rollback 保持态和 major upgrade 后审计
`Program Files\Mo`。但安装完成后到任意一次 Broker 启动之间，机器资产仍可能被管理员操作、
第三方安装器或磁盘恢复流程改变。只在 VM 验收时检查，不能让日常启动对当前实际安装树
fail-closed；而仅核对 Broker 自身 canonical path 也不能保护随后加载的 `rime.dll`、OpenCC
资源和词库。

## 决策

1. `mo-windows-platform` 提供 Windows 专用 `validate_installation_tree`。调用者必须同时给出
   Known Folder 得到的 `ProgramFilesX64` 和精确的 `ProgramFilesX64\Mo`，不能选择其他信任根。
2. 先核对当前 Broker 与固定安装映像的 canonical path。仓库、搬迁或缺失安装布局在访问
   安装树前即拒绝，保持 release smoke 的确定错误边界。
3. 身份核对通过后，审计 Program Files、Mo 根和显式栈遍历得到的每个后代。每个路径都用
   `FILE_FLAG_OPEN_REPARSE_POINT` 的 Win32 handle 读取属性；任意 reparse point、文件/目录类型
   异常或文件硬链接数不为 1 都拒绝。
4. 同一 handle 读取 owner 与 DACL。owner 只接受 TrustedInstaller、LocalSystem 或 Builtin
   Administrators；缺失/null DACL 拒绝。向非受信 SID 授予任一 write-like 权限的 allow ACE
   拒绝；Creator Owner 只允许作为继承模板。不能可靠解析的非基础 write-like allow ACE
   fail-closed。
5. 审计必须在创建 `LocalAppData\Mo\Rime\build`、绑定命名管道和加载 `rime.dll` 之前完成。
   用于 disposable fixture 的内部资源准备函数不代表 release 入口；默认 release 只调用包含
   信任审计的入口。

## 验证

- Rust SDDL 用例覆盖受信 owner/writer、普通 Users 只读、非受信 owner、Users Full Control、
  Authenticated Users GENERIC_WRITE 和 null DACL。
- 临时文件双硬链接负例在 ACL 判断前被拒绝；外部安装根负例被拒绝。
- 当前主机仅只读验证真实 Program Files ACL 能通过相同策略，没有创建 `Program Files\Mo`。
- release Broker 子进程继续拒绝诊断参数、调用者路径和仓库映像；仓库映像先返回固定路径错误，
  证明不会误把不存在的安装树审计放在身份门之前。
- 测试没有运行 MSI/Bundle、注册或启用输入法，也没有创建用户 Rime 目录。

## 后果

每次 release Broker 启动都会重新判断当前机器资产是否仍处于安装器预期的只读边界，失败时
不会开放 IPC 或加载 native code。该门不替代 Authenticode、逐文件发行哈希、安装器签名和
真实 VM 生命周期证据。审计完成到后续打开资源之间仍存在 TOCTOU 窗口；要抵抗已具备受信
写权限的高权限攻击者，后续需采用持续 handle 身份绑定/签名复核，不能把本 ADR 描述为完整
运行时防篡改。
