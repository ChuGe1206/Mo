# Phase 0 验收门

Phase 0 的目标是用可抛弃的最小代码验证最高风险，不是提前铺开完整产品。

## G0：工程基线

- 固定 Rust toolchain 和 MSVC target。
- 根 workspace 可执行 `cargo fmt --check`、`cargo clippy --workspace --all-targets` 和 `cargo test --workspace`。
- 所有源文件有明确许可证；第三方依赖有锁定版本与来源。

## G1：librime FFI

- `mo-rime-sys` 只暴露固定 `rime_api.h` 的最小 allowlist。
- C probe 与 Rust 测试核对关键大小、对齐、offset 和 `data_size` 语义。
- 所有 C 输出对象严格配对 `free_*`。
- 真实 librime runner 完成 `nihao -> 你好`，并输出 owned snapshot。

## G2：TSF 与 IPC

- Mo 自主 TSF 壳能在 x64 Notepad 中激活、收到按键并与 Rust Broker 往返。
- x86 TIP 能连接同一个 x64 Broker。
- IPC 拒绝超长、截断、错误版本和过时 generation。
- Broker 超时或退出不会卡住宿主，也不会重复上屏。
- AppContainer/WinUI 宿主能连接受限 pipe，ACL 有自动验证。

## G3：安装

- 一个测试 `Setup.exe` 可以安装、注册、为当前用户启用、修复和卸载。
- 安装不抢默认输入法、不删除现有输入法。
- 文件、MSI 和外层 Setup 的签名链可验证。
- 首次切换小于 500 ms，且不编译词典。

## G4：数据与许可证

- rime-ice 上游 commit 和全部输入文件有 hash。
- 能从源构建 Mo 预编译数据并运行 golden smoke。
- SPDX/SBOM、`THIRD_PARTY_NOTICES`、GPL 对应源和修改记录齐全。
- 默认捆绑前完成正式许可证结论。

## 当前完成定义

只有 G0–G4 都有可重复执行的命令、输出和限制说明，Phase 0 才算完成。脚手架、模拟测试或文档本身不等于通过真实 TSF/librime/安装验证。

