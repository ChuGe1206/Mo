# ADR 0030：完整 payload 与机器级 TSF 的 MSI 回滚事务

## 状态

已接受（开发作者层；未完成真实 MSI/VM 验收）

## 背景

旧 WiX 占位只列出 Broker、registrar 和两个 TIP，未安装 librime、OpenCC
或 rime-ice，也没有调用 TSF profile/category API。仅靠 MSI Registry 表可以
原生回滚 COM 双视图，却不能替代 `ITfInputProcessorProfileMgr` 与
`ITfCategoryMgr`。直接用一个无状态 EXE custom action 又无法区分“本次新建”
和“修复/升级前已存在”，失败时可能删除原状态。

## 决策

1. `wix-payload.ps1` 只消费已通过 `Assert-MoPreparedStage` 的 payload，按规范
   相对路径生成一文件一 component。文件、component、directory ID 与 GUID
   由带域分隔的 SHA-256 确定性导出；四个跨文件引用保留固定 ID。
2. 生成后不信任生成器本身：验证器从 WiX XML 的目录祖先反向构造每个目标
   路径，并与 stage 逐项核对数量、唯一性、source、GUID、bitness、key path、
   component-group 引用和 TIP 的两个 COM 视图。x86 DLL 文件仍由
   `ProgramFiles64Folder` 下的 64-bit file component 管理，其 32-bit COM 值
   由 `ProgramFilesFolder` 下独立 registry-only component 管理，避免 ICE80。
   evidence 永不进入安装树。
3. registrar 的安装器命令不接受路径参数，只从
   `FOLDERID_ProgramFilesX64\Mo` 推导 TIP 与两个固定 marker，避免 elevated
   custom action 解析用户可控命令行。
4. install/repair 和 remove 在任何 TSF 变更前，以 `CREATE_NEW` 写入 marker，
   内容是版本、操作类型，以及变更前 profile/category presence bit。marker
   必须是普通非 reparse 文件且内容精确匹配。deferred action 把状态收敛到
   `11` 或 `00`；rollback action 恢复 marker 中的任意四种状态；commit action
   仅在成功结束时删除 marker。
5. custom action 使用 MSI Binary 表内嵌的 x64 registrar，因此卸载的
   `RemoveFiles` 之后仍可执行 commit/rollback。所有机器变更都是 deferred /
   rollback / commit、`Impersonate=no`；rollback action 排在其对应变更之前。
   包在 `RollbackDisabled` 时拒绝启动。
6. 全新安装要求变更前状态严格为 `00`；若已有开发或外来 Mo profile/category，
   在写 marker 和调用 TSF API 前以 `ERROR_ALREADY_EXISTS` 拒绝，避免失败时把
   原 icon/path 元数据偷换成安装路径。repair/major upgrade 才允许刷新既有状态。
   major upgrade 卸载旧包时不移除稳定 profile；新包记录此前 `11`，失败恢复
   `11`。正常卸载仍记录并删除 profile，失败恢复原 presence bits。
7. 当前用户启用、Broker 启动和默认输入法不属于机器 MSI custom action。
   它们必须由后续非提升 finalizer 处理，且永不抢默认输入法。

## 验证

- x64/Win32 registrar 以 `/W4 /WX` 构建并通过 ABI load probe。
- 两个架构以及最终 staged x64 registrar 都完成 install/remove × 四种 prior
  state 的 marker round-trip、重复创建拒绝和无残留测试；测试不调用 TSF
  注册 API、不提权。
- 137-file stage 含 131 个 payload、6 个 evidence，72 个 Mo source snapshot；
  manifest SHA-256 为
  `8DC5C950778F7225EFC9F07F8060C6C3C166EC24232DF23A0929914E75DA3FAA`。
- 13 项包作者层测试覆盖静态事务、完整映射、确定性输出、source/安装根篡改、
  缺引用和 evidence 排除。素材运行时双架构各 10 轮故障恢复通过，共 40 次明确
  Broker 退出；7 组 prebuilt-only golden 通过。

## 限制

本机没有 WiX v4 CLI，因此没有生成或执行 MSI/Bundle；XML 策略通过不等于
Windows Installer 序列真实通过。marker 是一致性/回滚协议，不是抵御本机
管理员的安全边界；Program Files ACL、祖先 reparse 与竞态仍需安装后检查。
尚未注入 custom action 中止、commit 失败、系统重启、loaded TIP、major upgrade
和卸载失败。Standard BA 没有 current-user finalizer，产物未签名，不能分发，
G3 保持未通过。
