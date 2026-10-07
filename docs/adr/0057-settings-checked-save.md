# ADR 0057：设置保存检查旧快照并串行化写入

## 状态

已接受，Win10 本机存储与控制器回归通过；安装态 GUI 交互尚未复验。

## 背景

设置中心保留打开时的文档。原保存逻辑直接把该快照的修改原子替换到磁盘，
原子替换只能避免半个文件，不能避免旧窗口覆盖另一个窗口的新偏好；
文件随后升级为未来格式时，仍打开的旧窗口也可能覆盖它。

## 决策

1. 保留 v1 文件、路径、编码与所有运行时偏好语义。新增
   save_atomic_if_unchanged，以 LoadedSettings 中的强类型值及 absent/stored
   来源为前提；保存前在写入保护内重新读取，变化或新损坏/未来格式返回 Changed，
   不改磁盘、不修改控制器快照、不广播成功通知。
2. 两种 Mo 保存 API 共用文件旁的 reserved .write-lock。
   create_new 拒绝既有文件；Win10 使用 share_mode(0) 和 DELETE_ON_CLOSE，
   仅创建并拥有自己的新文件。检查、flush、原子替换均在同一保护生命周期内。
   被占用立即返回 WriteBusy，不在 UI 阻塞等待。关闭句柄或进程退出删除自己的保护，
   不采用、截断或删除外来 marker。
3. theme 与 primary preferences 使用检查保存。冲突提示点击“重新读取”，忙提示
   稍后重试；错误时保留本窗口的编辑选择。重新读取后才能基于新值保存。
4. 明确“恢复默认设置”仍调用无条件替换 API，允许用户主动替换损坏/未来文档；
   该操作也必须取得同一个写入保护，不与另一 Mo 写入并发。
5. 检查的是语义值，不以 CRLF/字段顺序为冲突；不是递增磁盘版本或历史恢复格式。
   该协议保护参与此 API 的 Mo 写入者，不声称阻止旧版程序或任意外部直接写入。
   非 Windows 的普通关闭会删除 sidecar，但异常退出清理未验收；本阶段验证范围为 Win10。

## 验证

- mo-settings 25 项通过；一个 ignored 项是被父测试显式启动的自有 child 入口，
  不是漏测：父进程验证跨进程占用、强制结束、无遗留保护及再次保存。
- mo-settings-app 9 项通过：两个窗口、重新读取、未来格式保护与显式恢复、
  旧窗口不能关闭另一窗口的隐私偏好；现有字段保留和首次不落盘通过。
- 八线程同时用 absent 快照保存，仅一个成功；其余 Changed/WriteBusy；
  absent/stored 与删除变换、新损坏文档、外来 marker 原字节保全通过。
- fmt、workspace Clippy -D warnings、完整 workspace 及设置 EXE 构建通过。
  合成路径均由测试独占创建，不操作真实用户配置或 Windows 输入注册状态。
- GUI 使用既有错误状态栏，未增加布局或控件；本轮未打开安装态设置窗口，
  未更新 stage/安装包/VM，相关桌面视觉与升级验收继续保留。

## 参考

[Rust Windows OpenOptions](https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html)
与 [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
定义句柄共享和关闭删除行为；本机强制结束回归核对了实际清理结果。

原始日志、源码与实际 EXE 绑定在 [证据清单](../phase-0/evidence/WIN10-SETTINGS-SAVE-20261007.json)。
