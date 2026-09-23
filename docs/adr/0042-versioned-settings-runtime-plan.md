# ADR 0042：设置运行时计划通过可选 IPC 快照进入原生前端

## 状态

已接受并实现运行时计划、Broker 查询协议、连接时读取与候选窗主题应用；尚未实现
设置图形界面、自动变更通知或 librime/schema 选项激活。

## 背景

ADR 0041 只建立了磁盘格式。若原生 TIP 自己读取文件，会重复 Known Folder、解析和
损坏恢复逻辑，并绕过 Broker 已有的同用户认证边界。另一方面，直接把所有设置翻译成
`SessionOptions` 会错误声称当前 ABI 已支持 schema、简繁、候选数、Emoji 和学习策略；
`RimeBackend` 正确地仍然拒绝这些操作。

设置文件还可能在 Broker 运行期间被替换。刷新失败时不能用默认值覆盖最后一次有效
选择，也不能让旧客户端因协议新增字段而无法输入。

## 决策

1. `mo-settings` 把已校验的 `Settings` 编译为两部分：`PresentationPlan` 保存主题与
   注释显示意图；`EnginePreferences` 保存输入方案、简繁、候选数、Emoji 以及隐私
   派生后的有效学习状态。后者只是期望状态，不暴露 librime option 名称。
2. `SettingsRuntime` 持有最后一次有效快照和从 1 开始的进程内 revision。只有语义内容
   或 defaults/stored 来源变化才递增；损坏、I/O 失败和未来版本返回错误并保留旧快照。
3. IPC 新增可选 `FEATURE_SETTINGS_SNAPSHOT`、`GetSettings` 和固定 18 字节的
   `SettingsSnapshot`。请求必须在握手协商后、使用连接级零 session token 和空 payload；
   枚举、候选范围、布尔值及 `effective_learning` 关系均由两端复核。
4. 安装态 Broker 从 OS Known Folder 固定路径打开设置。已有损坏文件使启动失败；运行中
   刷新损坏返回稳定错误，Broker 内最后有效快照不被覆盖。debug/fake Broker 使用内存
   产品默认值，不读取开发机真实用户设置。
5. x64/Win32 TIP 在连接阶段查询一次快照，也提供显式刷新操作；旧 Broker 未协商该
   feature 时继续使用编译内默认值。查询不进入逐键 50 ms 热路径。
6. 本阶段只让候选窗主题实际生效：Light/Dark 使用 Mo 调色板，System 使用 Win32 系统
   颜色。注释尚未进入 candidate wire payload；候选页大小必须由引擎分页支持；输入方案、
   简繁、Emoji 和学习策略仍待单独的真实 librime 验收。

## 验证证据

- Rust workspace 138 项测试通过；新增测试覆盖计划分层、revision 稳定性、来源变化、
  损坏保持与恢复、18 字节 codec、非法枚举/范围/隐私派生值、协商与连接级请求约束。
- MSVC `/W4 /WX` 下 x64 与 Win32 TIP、registrar、ABI/IPC probe 均成功编译和加载。
- 双架构 C++ 客户端实际连接 Rust Broker，读取默认快照、显式刷新并继续完成 IPC、16
  连接池、候选点击、Edit Session、Broker 两次崩溃/恢复及无 commit replay 探针。
- 测试只启动仓库 debug fake Broker；没有注册、启用或安装 Mo，也没有创建真实
  `LocalAppData\Mo\Profile\settings-v1.mo`。

## 后果与后续

Broker 成为设置解释的单一入口，TIP 不接触设置文件，新增字段仍须 feature/格式升级。
当前显式刷新 API 还没有跨进程通知触发器：设置 UI 完成后需要选择受限命名事件或控制
通道，并证明不会把磁盘 I/O 放进按键热路径。

下一阶段优先实现独立的 Windows 设置前端以及安全目录创建/保存；随后为实际支持的
librime 能力逐项增加适配和真实 rime-ice 测试。未通过的字段必须在 UI 中明确为暂不可用，
不能仅因它们存在于快照中就标记为已生效。
