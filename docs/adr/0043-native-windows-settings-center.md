# ADR 0043：Rust 原生 Windows 设置中心进入安装与发行合同

## 状态

已接受并实现首版图形前端、安全保存、损坏恢复、离线 stage、开始菜单、合规与签名
合同；尚未实现跨进程变更通知、引擎选项激活或真实安装后的桌面验收。

## 背景

ADR 0041/0042 已建立强类型磁盘格式、Broker 最后有效快照和 TIP 主题应用，但普通用户
仍没有可操作入口。Mo 的目标是安装即用，不能要求用户编辑 YAML、Lua 或复制词库。
同时，设置结构中多数引擎偏好尚未完成 librime 适配；若界面提前允许保存，会制造
“选项可用但实际无效”的错误产品承诺。

设置文件还可能损坏、来自未来版本，或其父目录被文件/reparse point 占用。图形程序
不能在打开时静默覆盖，也不能为首次查看设置而产生用户状态。

## 决策

1. 新增 `mo-settings-app`，采用 Rust + Win32 控件自主实现，不引入 WebView、UI 框架或
   额外运行时。程序以普通用户、`asInvoker` 语义运行，当前只支持 Windows。
2. 首版只开放候选窗 System/Light/Dark 主题。输入方案、简繁、候选数、注释、Emoji、
   本地学习和隐私设置以只读摘要显示，并明确标注仍在接入。
3. `SettingsController` 隔离 UI 与存储：首次运行只加载产品默认值且不创建文件；主题
   保存保留所有其他强类型字段；重新读取可观察外部原子替换。
4. 损坏、非法或未来版本文件进入 `RecoveryRequired`。普通保存被禁用，只有用户明确
   点击“恢复默认设置”才能覆盖，避免把解析失败伪装成默认配置。
5. 保存前从 LocalAppData 根开始逐级验证并创建精确的 `Mo\Profile`，拒绝文件、符号
   链接/reparse point，再复用 ADR 0041 的同目录 flush + 原子替换。
6. `prepare-stage.ps1` 从源码快照离线、锁定地构建 `mo-settings.exe`；stage 清单、PE
   架构、build receipt、逐文件合规和 Authenticode 内层目标都必须覆盖它。
7. WiX 为设置可执行文件生成稳定 component/file id，并由同一文件组件拥有一个
   advertised 开始菜单快捷方式“Mo (墨) 输入法设置”。作者层和链接后反编译验收都
   校验快捷方式，不执行 MSI/Bundle。

## 验证证据

- `mo-settings` 19 项测试、`mo-settings-app` 4 项控制器测试通过；workspace fmt、默认
  all-targets Clippy 通过。测试覆盖首次无落盘、字段保留、显式损坏恢复、外部替换及
  安全目录创建/文件占位拒绝。
- 全新离线 stage 含 82 份 Mo source、132 份 payload、6 份 evidence 和最终 manifest，
  manifest SHA-256 为
  `B3C0BE322036D56AD26F195F28D82C28E89FDEE3D2675DE423F76FD510440D7A`；88 项 staging
  与 20 项完整 WiX 作者层/篡改拒绝测试通过；机器素材完成 7 组 golden，x64/Win32
  各 10 轮 TIP/runtime/data 故障恢复及共 40 次明确 Broker 退出通过。
- WiX 4.0.6 实际链接并反编译未执行的 `0.0.6.0` ProductionShape：MSI SHA-256 为
  `EFB02B28F90C57C9D953F1420A582C91F86C284A1755AC258B5C6DA56B62BBC2`，Bundle SHA-256
  为 `2190D4D75CFA48926C5A54D73F311CB5641194F981D9F706A721F63E55A346CA`。反编译结果
  含 132 File、133 Component 及唯一 settings shortcut。
- 132 文件的 SPDX/通知草案完成确定性生成和独立核验；12 项合规测试、8 项来源材料
  测试和包含六个内层 PE 的 7 项签名计划测试通过。
- 未启动设置 GUI，未创建真实 `LocalAppData\Mo\Profile\settings-v1.mo`；未安装、注册、
  启用或设为默认输入法，未执行 MSI/Bundle，也未访问签名证书或时间戳服务。

## 后果与后续

用户路径第一次具备可发现、可安全保存的原生设置入口；安装和发行证据不会漏掉新增
可执行文件。代价是界面暂时保守，只让已验证的主题可编辑。

下一阶段应增加受限命名事件或控制通道，让运行中的 Broker 在非按键热路径刷新设置；
随后按输入方案、简繁、候选页大小、Emoji、注释与学习/隐私顺序逐项实现真实 librime
适配、失败回滚和 rime-ice 验收。包含快捷方式的新安装合同还必须在可销毁 Windows 11
VM 完成 clean install/repair/rollback/upgrade/uninstall、MSI ICE 与普通桌面宿主检查。
