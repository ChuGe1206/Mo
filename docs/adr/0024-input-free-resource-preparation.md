# ADR 0024：无输入资源准备与允许列表开发运行时

- 日期：2026-09-18
- 状态：资源准备、干净构建与双架构受控压力本机通过；不是正式发行运行时
- 延续：ADR 0022/0023；50 ms 传输预算、Emoji 功能与歧义请求不重发的约束不变。

## 决策

资源保活会话只能防止 weak owner 失效，不能消除首次转换时的延迟加载。
在锁定 librime 上增加独立版本化 C 导出 `mo_rime_prepare_resources_v1`，
由 Broker 的私有空会话调用实际已拥有的两个 Simplifier，显式初始化
Emoji/繁简转换资源。它不创建临时转换器，不发送伪输入，不清空输入或提交，
不修改 schema/options，也不改变上游公共 RimeApi 表。

仅空闲、无待提交内容、未切换到 Switcher 的 `rime_ice` 会话能准备资源。
必须恰好准备两个预期 filter，返回值恰好为 1 才被 Rust 接受。
未知/忙会话、其他 schema、缺失导出或资源错误均拒绝；C 包装器捕获异常。
私有保活会话仍没有 wire token，也不接收用户按键。

安装模式与显式开发参数 `--rime-prepared` 要求成功准备，失败即退出，不能
自动回退。显式开发 `--rime` 保留为旧行为的对照。准备位于现有 30 秒启动
watchdog 内、管道就绪信号之前；输入热路径不再负责这一步。

## 构建边界

`tools/runtime-build/build.ps1` 只消费锁定 Git HEAD 导出的六份 source archive，
校验 commit 与 archive hash；不会消费脏工作树、旧 include/lib/CMake cache
或自动发现其他插件。输出必须是仓库 build 下的新目录，拒绝覆盖；工具包
CMake 3.31.10、Boost 1.84.0、Lua 5.4.9 的官方压缩包哈希也被固定。

仅合并 Lua 插件，关闭外部插件与 native 内容日志；使用一致的静态 MSVC
runtime。Lua 补丁把两处 lua_gettop 计数恢复为有符号 int，避免 lua_pop
负数运算在 /sdl 下触发 C4146；另排除 Lua/luac 的命令行 main 文件，不把
可执行程序入口合入 DLL。没有关闭 /sdl 来掩盖这些错误。

新 C 包装器 /W4 /WX；核心与 Lua 保留 /sdl /GS /guard:cf，链接保留
ASLR/NX。上游仍有 DLL-interface/size-conversion 警告，不声称整套上游
零警告。命令完整输出留在输出目录 commands；provenance 记录六份 source、
工具包、两份补丁、包装器、CMake hook 与 DLL 哈希，不是签名发行 manifest。

构建过程中出现过 PATH/Path 重复、静态 runtime 不一致、Lua 无符号栈计数
和重复 main 的失败。依赖复核还发现 OpenCC 默认安装 bundled Marisa 0.2.6，
会覆盖 core 的 0.3.1 库而保留 0.3.1 头文件。旧构建压力命令在 x64 100 轮
之后、Win32 第 31 轮开始时主动停止，不作为最终通过证据；原先 22.4 ms
到 1.18 ms 的直接 API 对照也只保留为旧构建实验。修正后 OpenCC 和 core
显式使用同一份 pinned 0.3.1，并核对安装库未被替换。
OpenCC 的旧默认语言模式不能编译 Marisa 0.3.1 的 string_view 接口，首次
统一构建因此失败；显式 C++17 的独立实验已通过，再创建最终干净构建。
最终证据必须来自修正脚本创建的全新目录，不能用
反复改写的旧对象目录冒充干净构建。

## 本机验收记录

最终干净构建位于 `build/mo-runtime-accepted`，完整 builder 命令退出 0。
DLL SHA-256：`40c3b8cbbf0b9b3b37693d7ce79878ead16cb19e2b5cfe885820e0b813fed403`。
公共 header hash 仍为锁定 `85caf744...c16dad4`，所有 core/Lua target 的
runtime 都为 MultiThreaded；安装后的 Marisa 库 hash 与 0.3.1 构建产物一致。
三个原生准备边界（成功、缺 Emoji 字典、官方 DLL 缺导出）、忙会话不清空、
Emoji 👋 保留、Unicode 用户目录通过；两个 Broker 子进程负向检查准确返回
准备错误并在 ready 前退出。四项 builder scope/overwrite/archive/commit
拒绝通过。最终运行时也完整读回 4857/1498 条字典，每个 key 的有序 values
与源一致。

同一最终 DLL、预编译 pack、两个独立新进程中：仅保活时首个 process_key
23,748 µs、context 1,110 µs；显式准备耗时 94,803 µs 且 input_free=true，
之后首个 process_key 1,189 µs、context 1,064 µs。后续 19 个新会话首键
prepared process_key 为 776–1,778 µs。未清文件缓存，不是总体延迟 SLA；
收益是把转换器准备移出输入请求，准备本身不是零成本。

最终同一份 prepared/trace 完整命令退出 0，x64/Win32 各 100/100 轮通过，
包括 IPC/pool、候选显示/鼠标/布局/延迟锁/焦点恢复、确定性重入、两次实际
Broker 退出/恢复和最终 EDIT/context 文本核对。200 轮共 400 次实际退出，
记录的 800 个首键传输计时范围 2,017–5,221 µs；不是整个 COM/渲染返回上界。
不放宽 50 ms，不关 Emoji，不重发，也不以此抹去 ADR 0022/0023 的历史失败。
此前偶发候选消失未在此次命令复现，但并未确认全部根因或普通宿主表现。
收尾重建默认关闭诊断的 Broker/TIP，另一份完整命令 x64/Win32 各 20/20
故障轮次及 IPC/pool/UI/edit 检查通过，证明不依赖诊断 feature。最终只读
registrar 状态仍为双视图 COM 缺失，profile 未注册/启用/激活；没有设默认。

收尾新增 CMake cache 来源路径守卫，在 OpenCC/core build 前拒绝选择外部
header/library；匹配、外部路径、缺项、重复项四项测试通过。该纯校验守卫
另已针对最终实际 cache 执行，core/OpenCC/Boost 都指向本输出的 pinned
资源，不改变最终 DLL 的编译内容。

Rust workspace 默认 109 个运行时测试、诊断 feature 111 项，各有 1 项
compile-fail，通过；包含新增三个资源准备/回收契约与一个显式模式契约。
fmt、默认/诊断 debug/release 四种 all-targets Clippy、release 启动拒绝
caller-selected prepared runtime 等五项负向 smoke 通过。16 项注册事务
内存检查、20 项既有 OpenCC 检查、8 份 PowerShell AST 检查通过。
尚无远端 CI 结果。全 workspace 曾在并行构建负载下出现
既有启动 watchdog 注入 marker 未到达的失败：进程 fast-fail 符合预期，但
80 ms 测试预算可能在 fixture 闭包开始前耗尽。保留这个负向结果，不放宽
生产预算或删掉 marker 断言；编译结束后的默认与诊断全套检查均通过。

运行时字典兼容检查的初版文本 hash 比较失败，原因是 TextDict/MarisaDict
的 key 枚举次序不同。最终按 ordinal key 集合逐条比较原样有序 value 序列，
不排序 values；在统一依赖实验中全部 4857/1498 条匹配。最终产物也复测通过。

## 仍未通过的发行/产品门

- 上游 OpenCC 仍有构建 prefix/CWD 搜索行为，未实现可重定位、fail-closed
  的正式资源加载；本 DLL 明确 development_only、redistributable=false。
- 允许列表不等于许可证批准；逐文件 SBOM、通知、对应源/修改记录、签名、
  安装 ACL/reparse 防护与更新来源验证仍待完成。
- 当前性能实验只使用新进程与独立 fixture，不清除系统文件缓存，不构成
  机器冷启动或普通应用延迟保证。50 ms 也不保证同步 COM/渲染返回上界。
- 注册后的系统路由、Notepad/WinUI/AppContainer、真实多应用/混合 DPI、
  安装/升级/卸载回滚仍未通过；不声称可日常使用。

依据：[锁定 Simplifier 源码](https://github.com/rime/librime/blob/33e78140250125871856cdc5b42ddc6a5fcd3cd4/src/rime/gear/simplifier.cc)、
[CMake CMP0091](https://cmake.org/cmake/help/v3.31/policy/CMP0091.html)、
[Microsoft /sdl](https://learn.microsoft.com/en-us/cpp/build/reference/sdl-enable-additional-security-checks?view=msvc-170)、
[Lua 官方发布目录](https://www.lua.org/ftp/)。
