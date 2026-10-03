# ADR 0050：按输入会话应用 Emoji 开关

## 背景与决策

设置文件已有 `emoji` 布尔字段，但此前设置中心只读显示，Broker 也未把它传给 librime。锁定的五个 rime-ice 方案均提供 `emoji` 选项；实际词库中输入 `nihao` 时，该选项控制候选 `👋` 是否出现。因此设置中心开放“启用 Emoji”复选框，并与方案、简繁、主题、候选注释一次原子保存。

Broker 在每次生产会话创建时，把设置快照的 `emoji` 值作为允许列表中的会话选项传给 librime。TIP 从已认证 Broker 获得设置变化后比较当前会话的 Emoji 值，沿 ADR 0046 的空闲边界替换会话：等当前按键配对和预编辑结束，先打开新会话，再退休旧会话。失败时保留旧会话及其输入；成功后清除旧候选页身份。主题和注释仍能直接重绘当前候选窗。IPC 与设置文件格式均不变。

## 验证与剩余边界

- 真实锁定 librime/rime-ice、独立用户目录验证 `emoji=false` 时 `nihao` 候选无 `👋`，`emoji=true` 时出现；五个方案及简繁组合仍可创建和输入。
- Rust 测试覆盖设置原子保存、五种方案 × 两种字符模式 × 两种 Emoji 状态的选项映射，以及新会话创建失败时旧预编辑保持；x64/Win32 原生构建和探针覆盖 TIP 可加载与生命周期。
- 当前源码重建的 ProductionShape 与 DevelopmentTest stage 均通过 138 文件清单、七组机器词库 golden 及双架构各一轮 Broker 故障恢复。ProductionShape `0.0.9.5`、DevelopmentTest `0.0.9.5/0.0.9.6` 的 MSI/Bundle 均通过链接反向核验与 MSI ICE；同载荷升级对及 clean/matrix VM kit 已核验哈希，但尚未执行安装。
- 同一 ProductionShape stage 的 SPDX 2.3/第三方通知草案、9 份精确源码归档及 15 份许可证/通知材料通过独立核验；绑定实际 MSI/Bundle 哈希的签名顺序计划通过检查。材料完整性是技术断言，法律审查、发行授权和实际签名仍未完成。
- 该 stage 另以非并行方式完成 x64/Win32 各 10 轮真实词库崩溃恢复（共 40 次 Broker 退出），没有发现输入重放或遗留预编辑；受控探针未测已安装首次切换时间。
- 尚未在已注册的真实 Windows 宿主中操作设置中心并观察候选变化；这项人工验收须在可回滚 Windows 11 x64 VM 内完成，步骤见 `docs/phase-0/VM-INSTALLER-TEST.md`。
