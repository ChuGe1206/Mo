# ADR 0034：生产形态与开发故障注入的构建隔离

## 状态

已接受（两种 linked 形态及各自升级身份已验证；生产形态仍未签名、不可部署）

## 背景

ADR 0033 为一次性 VM 回滚矩阵加入了固定失败命令、MSI deferred action 和 Burn
链尾失败包。仅把开关默认设为零仍不足以形成发行边界：命令和故障包依然存在于最终
二进制中，也可能被安装参数重新开启。进入签名和真实 VM 验收前，需要证明测试能力
能从生产形态产物中物理消失，同时继续保留可重复的破坏性测试版本。

## 决策

1. 安装器只有两个显式 build flavor：`DevelopmentTest` 与 `ProductionShape`。两者都仍
   要求 `-AllowDevelopmentBuild`；后者还要求独立的 `-AllowProductionShapeBuild`，避免
   名称造成可发行的错误暗示。
2. registrar 的 `development-test-fail-fixed` 用
   `MO_DEVELOPMENT_FAULT_INJECTION` 编译期开关包围。CMake 选项默认 `OFF`，MSBuild 也只在
   显式属性为 `true` 时定义宏。关闭时命令不会进入 usage 或 dispatch，调用按未知参数
   返回 `E_INVALIDARG`；开启时才固定返回 `E_FAIL`。
3. `prepare-stage.ps1` 和原生 build probe 在构建后执行只读 `status -> fault probe ->
   status`。开发测试阶段必须看到固定 `E_FAIL`；生产形态必须看到 usage 与
   `E_INVALIDARG`，且两种情况下前后状态必须逐字一致。
4. WiX 以 `IncludeFaultInjection` 预处理常量控制安全属性、失败 custom action、隐藏 Burn
   变量、MSI 属性转发和链尾失败包。生产形态不会只把它们设为零，而是在链接前移除对应
   作者层节点，也不会生成或传入 failure-injector 文件。
5. linked verifier 以 build flavor 为输入，反编译 MSI 并提取 Burn 容器。开发测试版必须
   精确包含 8 个 custom action、2 个隐藏变量和 3 个 attached payload；生产形态必须只有
   7 个 custom action、无故障属性/变量/包，并只包含 MSI 与 current-user finalizer。
6. linked evidence 升级为 format 3，记录 `build_flavor` 与
   `fault_injection_included`。升级对要求两版 flavor 一致；VM lifecycle/matrix kit 只接受
   `DevelopmentTest + true`，禁止误把生产形态交给破坏性故障矩阵。

## 验证

- x64/Win32 registrar 的默认和显式故障注入构建均通过 `/W4 /WX`：默认产物拒绝该命令
  为 `0x80070057`，测试产物固定返回 `0x80004005`，四次探针均未改变 status。
- WiX 4.0.6 实际链接了 `ProductionShape` 的 `0.0.3.0` 与 `0.0.4.0`。反编译/提取验证
  通过；对输出作者层和清单的独立字符串扫描未发现六个故障注入标识；两版共享
  MSI/Bundle UpgradeCode，ProductCode、Bundle id 与产物哈希不同。
- 同一套条件作者层实际链接了 `DevelopmentTest` 的 `0.0.3.0` 与 `0.0.4.0`，故障节点和
  三份 payload 均被反向核对。format 3 evidence、format 2 upgrade pair、单版本 VM kit 与
  双版本 matrix kit 完成哈希绑定，没有执行安装器。
- 当前开发主机始终只进行构建、反编译、提取和只读 registrar status；未运行 MSI/Bundle，
  未注册或启用 Mo。

## 限制

`ProductionShape` 表示结构接近未来发行包，不表示 release candidate 或可分发版本。它仍
使用 development stage/品牌，未做 MSI ICE、Authenticode 签名、时间戳、安装树 ACL 与
逐文件 SBOM，也没有任何真实 VM 安装、回滚、升级或普通应用输入结果。完整 G3 仍未通过；
只有这些门全部关闭后，才可建立真正的签名发行构建。
