# ADR 0040：离线素材重建与 VM 验收基线封口

## 状态

已接受。离线重建、双架构运行时回归、linked 结构和两套 VM 传输包均已验证；
真实安装仍等待可销毁 Windows 11 x64 VM。

## 背景

素材准备原先只接受一个位于锁定 commit 的 rime-ice Git checkout，再由脚本现场
执行 `git archive`。这能排除 checkout 中未跟踪文件，却让重复构建依赖网络或一份
仍保留 Git 对象的本地仓库。stage 本身已经保存了同 commit、固定 SHA-256 的源码
归档，因此后续构建不应为了得到完全相同的输入再次访问网络。

此外，ADR 0039 后对 Lua 补丁文件做了不改变语义的 patch-hunk/尾部空白修正。
旧运行时 provenance 按设计拒绝当前仓库字节。不能绕过这个拒绝；运行时、stage、
DevelopmentTest、ProductionShape 和 VM kit 必须从当前源码重新封口。

## 决策

1. `prepare-stage.ps1` 必须且只能接收 `RimeIceSourceDir` 或
   `RimeIceArchivePath` 之一。
2. checkout 模式继续核对精确 commit，再生成 tar；archive 模式只接受本地绝对
   普通文件，并在解包前核对同一固定 SHA-256
   `CD1895FBC961131A62F23277F636C27A6FB941DAC66DAF43C4A10D4E9E6ADAD3`。
3. 两种模式都把精确归档复制进 stage evidence，后续只从该归档解包。输入选择、
   歧义、缺失、错误哈希及非法哈希全部 fail-closed。
4. VM 包仍只是一份哈希锁定传输物，不携带执行授权。开发主机不运行 MSI/Bundle，
   不注册或启用输入法。

## 验证证据

- 新增 6 项 pinned checkout/archive 策略用例，完整 staging policy 增至 88 项。
- 当前自构建运行时 `rime.dll` SHA-256 为
  `5B9FBB1429A19B15B53AAF7BFE480DAB808F59106F2093FE36ACA8BAF7F85A4B`；
  provenance 核对 12 份 Mo 输入和 `machine-shared-only-v1` Lua 策略。
- DevelopmentTest stage manifest SHA-256 为
  `63F6B60AF071A338A96D973727607913FBC9419E68E3E53A48E496E2BE61B8BC`；
  ProductionShape stage manifest SHA-256 为
  `863F8EE70007967654904DBA956BFF05617EB7F2486DAB27FFD4F0478536117C`。
  两者各有 73 份 Mo source、137 份清单文件和 131 份 payload，分别完成
  x64/Win32 各 10 轮、合计 80 次明确 Broker 退出恢复；七组真实词库、机器 Lua
  canary、preparation 和 Actor 候选检查均通过。
- DevelopmentTest `0.0.4.0 -> 0.0.5.0` linked major-upgrade pair 已反向核验。
  base Bundle SHA-256 为
  `B7B8AB66CB78929044693833E310BE9385C11B27677E865D1FFA9E8549FDA3FA`，
  upgrade Bundle SHA-256 为
  `E8FD0A15175419C85666964624D02FE93B98158AC3BA51803A0155239D3A3BB8`。
- clean lifecycle kit manifest SHA-256 为
  `7E05F04B18B816E7AA514BD013FC3ECDFC6F0CD024DAAFD5023C6CD642C780A6`；
  rollback/upgrade matrix kit manifest SHA-256 为
  `0541F0E7C887B24FABEDB49FD7BEE32EBB706B1AE9D8F4B7B52534FFCB4CBF40`。
  两套实际 inventory 与 33 项 VM 生命周期策略检查通过。
- 未执行的 `0.0.5.0` ProductionShape MSI SHA-256 为
  `66DF5B460A7222DC7432E38B01DDDA6F69927B264336A419DCE85B611FE92D33`，
  Bundle SHA-256 为
  `15D222807830893AC516F0872A398E713240748ABA012547CA278D2A3AB4DB2C`；
  反编译、Burn container 和嵌入字节核验通过。

## 后果与剩余边界

锁定归档现在足以离线、可重复地重建完整素材和 VM 验收包，不再要求保留可变
checkout 或重新联网。checkout 模式仍保留，方便从精确上游 commit 产生第一份归档。

当前主机没有可用 Hyper-V、VirtualBox 或 VMware 入口，因此没有执行安装、修复、
强制回滚、升级或卸载。MSI ICE、真实 TSF 宿主、签名、多用户和 UAC 组合仍是
VM/发行门；本 ADR 不把项目升级为可安装或可日常使用。
