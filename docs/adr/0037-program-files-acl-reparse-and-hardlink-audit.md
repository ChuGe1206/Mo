# ADR 0037：Program Files ACL、重解析点与硬链接审计

## 状态

已接受（审计策略和一次性 VM 门已闭环；尚未取得真实 VM 安装证据）

## 背景

逐文件 SHA-256 只能证明读取时的内容与 stage 一致，不能证明普通用户无法在后续替换 DLL、
词库或 Broker。安装树中若存在 junction/symlink，递归枚举还可能离开 `Program Files\Mo`；
多硬链接文件则让树外路径能够修改同一文件。仅检查根目录字符串或安装器作者层不足以关闭
这些加载时信任风险。

## 决策

1. VM 策略使用显式栈枚举安装树。每个子项先读取属性并拒绝任意 reparse point，只有普通
   目录才入栈；不再使用可能先穿越 junction 的递归枚举。
2. 每个安装文件通过只读、共享读写删除且带 `FILE_FLAG_OPEN_REPARSE_POINT` 的 Win32 handle
   读取 `BY_HANDLE_FILE_INFORMATION.nNumberOfLinks`，必须精确为 1。哈希、大小、路径大小写、
   文件和目录集合检查保持不变。
3. 安装根必须精确为 OS `ProgramFiles\Mo`。Program Files、安装根及所有后代分别读取 owner
   与 DACL；owner 只接受 TrustedInstaller、LocalSystem 或 Builtin Administrators。
4. 缺失/null DACL 一律拒绝。任何 AccessAllowed ACE 若向非受信 SID 授予写数据、追加、
   写扩展属性、删除子项、写属性、DELETE、WRITE_DAC、WRITE_OWNER、GENERIC_WRITE 或
   GENERIC_ALL，立即拒绝。Creator Owner 只作为继承模板受信；能否创建入口仍由父目录 ACE
   独立限制。
5. clean lifecycle 的 install/repair，以及 rollback/upgrade matrix 的 base install、
   marker 故障保持态和 major upgrade 后都必须重新通过完整审计。成功 evidence 升为 format 2，
   并明确记录 `install_tree_security_audited=true`。

## 验证

- 33 项非安装测试通过，其中新增受信 SDDL、非受信 owner、Users Full Control、
  Authenticated Users GENERIC_WRITE、树外硬链接和目录 junction 的正负向用例。
- 当前开发主机的真实 `C:\Program Files` owner 为 TrustedInstaller，13 条 ACE 通过同一
  fail-closed DACL 模型；没有创建 `Program Files\Mo` 或修改 ACL。
- guest 策略保持 Windows PowerShell 5.1 兼容，Win32 link-count helper 只申请属性读取。
- 测试没有执行 MSI/Bundle、提权、注册或启用输入法。

## 后果

一次性 VM 验收现可回答“安装时字节正确且安装后普通主体不可写、无路径转向、无树外硬链接”。
它仍不是竞态下的持续 handle-based 防替换，也不验证 Authenticode；真实签名后还需把签名状态
加入同一 VM evidence。当前主机没有可用一次性 VM，因此 G3 仍未通过。
