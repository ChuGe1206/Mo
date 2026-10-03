# ADR 0048：安装载荷静态链接 MSVC 运行库

## 背景

`0.0.9.1` 开发安装包的五个 Mo PE（Broker、设置中心、registrar、x64/x86 TIP）直接导入 `VCRUNTIME140.dll`，原生 PE 还导入 `MSVCP140.dll`；Bundle 未安装 Visual C++ 可再发行包。干净 Windows 主机上，这会使安装后的程序或 TIP 在启动时缺少依赖。已单独构建的 `rime.dll` 没有这些导入。

## 决策

- 发布形态的 Rust MSVC 构建显式使用 `-C target-feature=+crt-static`；原生 MSBuild/CMake Release 目标使用 `MultiThreaded`（`/MT`）。这遵循 [Rust 静态 CRT 说明](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes)和 [MSVC `/MT` 文档](https://learn.microsoft.com/en-us/cpp/build/reference/md-mt-ld-use-run-time-library)。
- stage verifier 直接读取六个 PE 的常规及 delay import 表，拒绝 `VCRUNTIME`、`MSVCP`、`MSVCR`、`CONCRT` 等动态 Visual C++ 运行库导入。此检查与 stage manifest 哈希共同阻止构建参数回退；不依赖开发机已安装的运行库。
- C++ 与 librime 之间继续只交换 C API 值、opaque 句柄及各自 API 管理的内存，不能跨模块直接释放对方分配的对象。

## 验证与边界

旧 stage 的五个 Mo PE 被新门拒绝；新 stage 六个 PE 的导入表由内置解析器与 `dumpbin /DEPENDENTS` 交叉核对，均无 Visual C++ 可再发行包 DLL。新 stage 的 89 项 staging、21 项安装包作者层、真实 librime/机器数据及双架构故障恢复检查通过。CMake 构建另外补齐与 MSBuild 一致的 Unicode 宏；双架构 ABI 探针通过，八个 CMake PE 也通过运行库导入检查。未签名 ProductionShape `0.0.9.2` MSI/Bundle 已链接、反向核验并通过无警告 MSI ICE；它们仍未在干净 VM 安装、签名或取得发行授权。静态链接增加了载荷大小，且不代替目标 Windows 版本与真实宿主启动测试。
