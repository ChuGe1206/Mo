# ADR 0031：当前用户 finalizer、Burn 权限拆分与持久回滚

## 状态

已接受（开发作者层与隔离状态协议已验证；真实 Bundle/用户输入状态尚未验收）

## 背景

机器 MSI 可以拥有 Program Files、HKLM 双 COM 视图和 TSF profile/category，
但不能以 Windows Installer 的系统上下文修改发起安装用户的输入法列表。普通
用户进程又不应承担机器注册。仅在安装末尾调用一次 `InstallLayoutOrTip` 还存在
两个问题：Burn 的正向和逆向动作会跨进程，进程内补偿不足以恢复原先的 enabled
bit；进程若在输入 API 与 marker 写入之间崩溃，也会留下无法归属的用户状态。

## 决策

1. Burn 先执行隐藏的 per-machine MSI，再执行 vital、`PerMachine=no` 的 registrar
   ExePackage。三个基础 arguments 显式为空，`CommandLine` 根据不变的全局
   `WixBundleAction` 选择完整命令：Install(5) 正向 install、逆向 rollback-install；
   Uninstall(3) 正向 remove、逆向 rollback-remove；Repair(7) 只执行 repair。因而
   Repair 遇到 marker 缺失会在任何变更前失败，而不会误走 install。
2. Bundle 在 64 位 HKCU 的机器本地、非漫游路径
   `Software\Classes\Local Settings\Software\Mo\InputMethod\Setup` 检测
   `UserFinalizer=mo-user-finalizer-v1`。缺失或任意其他值都不算已安装。
3. finalizer 拒绝提升令牌、Session 0 和 AppContainer。install/repair 与卸载回滚
   从 `FOLDERID_ProgramFilesX64\Mo` 推导固定路径，核对 x64/x86 TIP 文件、HKLM
   两个 COM view 的精确路径与 `ThreadingModel=Apartment`、profile/category；还
   拒绝 HKCU 两个 WOW64 view 中任何同 CLSID key，防止 per-user COM shadow。
4. 所有当前会话用户状态变更由带固定 GUID 的 `Local` named mutex 串行化，最长
   等待 30 秒。`WAIT_ABANDONED` 视作取得锁，超时和异常均 fail-closed。它只解决
   同一登录会话；跨会话/多用户生命周期仍是发行门。
5. 同一机器本地 key 中保留 `UserFinalizerTransaction` 持久 undo journal。六种精确
   值编码 install/repair/remove 和变更前 enabled bit。写入、删除均 `RegFlushKey`
   后读回；字符串类型、长度、NUL 与 64 KiB 上限严格校验。journal 必须在调用
   输入 API 前落盘，并在成功后保留，因为 ExePackage 没有 commit callback，后续
   Burn 逆向进程仍需读取原始 enabled bit。下一次正常操作会覆盖旧 receipt。
6. fresh install 只接受无 marker 且未启用的未拥有状态，先写 install journal，再
   启用并提交 marker；同一 install journal 可恢复中断。repair 必须已有精确 marker，
   写 repair journal 后重新启用。remove 必须拥有 marker，写 remove journal 后撤销
   启用并删除 marker；机器 profile 已被外部移除时只回收 marker。
7. `rollback-install-current-user-fixed` 只消费合法 install journal，恢复原 disabled
   bit并删除 marker/journal；`rollback-remove-current-user-fixed` 在机器边界已恢复
   后只消费 remove journal，恢复原 enabled bit与 marker，再删 journal。任一步失败
   都保留可检测、可重试的持久状态并向 Burn 返回失败。
8. ExePackage 以稳定的 `Mo.CurrentUserFinalizer.v1` dependency provider 和产品版本
   参与 Burn 引用计数；构建显式加载 Dependency extension，兼容升级不创建新的
   无关 owner。
9. 启用只给 `InstallLayoutOrTip` 传 0，撤销只传 `ILOT_UNINSTALL`。源码与作者层
   测试拒绝 `ILOT_DEFPROFILE`、`ILOT_DEFUSER4`、`ILOT_CLEANINSTALL`；Mo 不静默
   成为默认输入法，也不清空其他输入法。

## 验证

- x64/Win32 registrar 以 `/W4 /WX` 全新编译，零警告；五类操作 × marker × 七种
  journal × enabled bit 的 140 个纯状态组合及四种进程上下文通过。
- 隔离 HKCU 测试真实写入、读回并清理双 COM view、marker 和全部六种 journal；
  连续执行后无残留，不注册、不启用 Mo。
- 缺少机器安装/profile 的本机 preflight 对五个正向/逆向命令核对精确 HRESULT，
  前后 `status` 不变。状态解析器 18 个内存场景通过。
- 作者层锁定 MSI -> finalizer 顺序、空基础参数、三组全局 action 路由、机器本地
  marker、稳定 dependency provider、WiX CLI 与三项 4.0.6 extension 锁定和
  native/WXS 协议；完整
  素材共 17 项。新鲜 v2 stage 为 72-source/137-file，manifest SHA-256
  `745DF9C48F5D72498ED9D8CD4970B586054F2D5E31F624E8C03ECC6A40317B3E`；79 项
  staging、7 组 golden、x64/Win32 各 10 轮故障恢复（40 次明确 Broker 退出）通过。

## 限制

本机没有 WiX v4 CLI，因此尚未链接或执行 MSI/Bundle，也未运行真实
`InstallLayoutOrTip` 用户变更。仍须在隔离 VM 验证 clean install、marker 缺失的
Repair、强制回滚、major upgrade、卸载，以及已提升启动/UAC 关闭时 per-user child
不会越权。多用户机器上逐用户撤销、跨会话互斥、旧 Bundle 升级引用计数实测、
loaded TIP 和签名也未闭环。锁定的 WiX 4.0.6/extension 尚未在本机安装和链接。
Standard BA 仍是临时界面；开发包
不可分发，G3 保持未通过。
