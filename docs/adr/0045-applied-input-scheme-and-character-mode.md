# ADR 0045：在新建输入会话应用方案与简繁模式

## 状态

已实现；本机真实 librime/rime-ice 的独立用户目录验证通过。已连接会话的热迁移和真实注册宿主仍待验收。

## 背景与边界

设置文件已经保存输入方案与字符模式，但此前 Broker 只把它们回传给 TIP，真正的 librime 会话始终使用默认配置。界面不得允许用户选择一个不会应用的选项。rime-ice 锁定版本的五个产品方案 ID 分别是 `rime_ice`、`double_pinyin`、`double_pinyin_flypy`、`double_pinyin_mspy`、`double_pinyin_sogou`；所有方案均使用 `traditionalization` 开关。

## 决策

1. Broker 在生产共享 Actor 的每次 `OpenSession` 前刷新强类型设置快照，映射方案 ID 与 `traditionalization` 布尔值，作为单次会话初始配置。损坏/未来版本文件返回 `ERROR_SETTINGS_UNAVAILABLE`，不创建半配置会话。诊断本地 fake 路径保持原行为。
2. Rust FFI 对 `set_option`、`get_option`、`select_schema` 逐槽校验 librime 公布的 `data_size`，旧/截断/空槽明确失败。ABI 布局由锁定官方 C header 的探针对照，不读取未宣告的尾字段。
3. 后端仅接受上述五个 schema 和 `traditionalization`，选方案失败或设置/回读失败时回收刚创建的 native 会话，Broker 不发 wire token。私有准备/保活会话继续固定 `rime_ice`，前端会话各自独立选方案。
4. Windows 设置中心把方案、简繁与主题一次原子保存。候选数量、Emoji、学习及隐私等未实现项仍只读且不声称生效。主题沿现有广播即时刷新；方案/简繁不改写正在输入的预编辑，仅在新建 Broker 会话时生效。恢复默认设置遵守同一边界。
5. IPC 帧与设置文件格式均不变。旧版 Broker 仍可能忽略新设置，发行时必须确保设置中心、Broker 与 TIP 同版本部署；不宣称混合版本热升级安全。

## 验证与未完成项

- Rust workspace 测试覆盖十种映射、FFI 尾槽边界、失败会话回收、原子保存与旧字段保留。x64 官方 header 对照通过 55 项布局断言；本机 stable 未安装 Rust i686 target，x86 Rust FFI 对照未运行，原生 TIP 的 x86 构建是独立验证。
- `settings_smoke` 使用打包的锁定 librime/rime-ice、准备资源和独立用户目录，验证五种方案各两种字符模式均能创建、接收首键并销毁；全拼 `zhongguo` 在简/繁模式分别得到“中国”与“中國”候选。未用本机输入法注册或真实用户配置。
- 最终全新离线 stage（`mo-windows-stage-engine-preferences-v2`）源码 85 份、payload 132 份、stage 总计 139 份，manifest SHA-256 `605017E0465896FDEFD78A8DD13036A4B50FAADB2609EA3B1569A14BA2751725`。88 项 staging、20 项 WiX 作者层、12 项合规、8 项来源材料、7 项签名顺序检查通过；7 组机器词库 golden 通过。此前 v1 与其他检查并行的 10 轮/架构恢复回归在 x64 第 9 轮遇到一次首键 78 ms 超时；v1 无并行负载重跑及最终 v2 独立重跑均在 x64/Win32 各 10 轮、各自总计 40 次实际 Broker 退出/恢复通过。负向结果未删除，也不据此宣称尾延迟已解决。
- WiX 4.0.6 已链接并反向核验未执行的最终 `0.0.8.0` ProductionShape：MSI SHA-256 `6B1FA032E2DD6DE094C0E4322E47BDE713D84B24B9DDDAE34FBC0975C00F88DD`，Bundle SHA-256 `93BB9186E06D05E14AF87E58B210009C33D4401CD2EAC747816091F1C60A1D29`。二者均未签名、不可部署；MSI ICE 未运行。
- 设置 GUI 未在此开发宿主启动。设置生效仍需新会话，当前已连接 TIP 不会因为广播立即迁移引擎会话；真实安装/普通宿主、延迟、旧 Broker 混用和 UI 操作需隔离 VM 验收。
