# ADR 0025：可搬迁的 OpenCC 资源边界与准备接口 v2

- 日期：2026-09-18
- 状态：实现完成，最终产物回归记录见下文；当时保留的 Lua/user/staging 边界由 ADR 0039 关闭
- 延续：ADR 0024；不改变 IPC、私有保活会话、50 ms 传输预算或歧义请求不重发的约束。

## 决策

Mo 转换资源只来自实际加载的 `rime.dll` 旁的 `opencc` 目录。
开发布局是 `dist/lib/rime.dll` 与 `dist/lib/opencc`；安装布局是
`Mo/runtime/librime/rime.dll` 与 `Mo/runtime/librime/opencc`。
它们一起搬迁，绝不从 Broker EXE、环境变量、cwd、用户目录、共享目录或
构建 prefix 推导转换资源。机器 shared/prebuilt 与 managed user/staging
仍维持 ADR 0016 的独立布局。

模块地址锚点配合 `GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT)`
与 Unicode `GetModuleFileNameW` 获取自身目录。Engine 的 LoadedLibrary 保持
DLL 所有权；不能对借来的模块 handle 调用 FreeLibrary。
依据：[Microsoft GetModuleHandleExW](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulehandleexw)。

独立导出升级为 `mo_rime_prepare_resources_v2(uintptr_t session_id) -> int`。
Rust 只解析同一个 DLL 的 v2，不回退 v1，不新增上游 RimeApi slots。
恰好返回 1 才成功。空闲、无待提交、非 Switcher 的 `rime_ice` 会话必须
准备两个实际 Simplifier owner，且资源种类分别为 Emoji/traditionalization；
两个重复 Emoji owner 不算成功。只接受 `emoji.json` 与 `s2t.json`。
准备仍不发按键、不清空、不提交、不修改 options，也不创建旁路转换器。
安装模式及 `--rime-prepared` 在 ready 前必须成功；旧 DLL、缺资源或解析
错误即退出。普通开发 `--rime` 不是 prepared-mode 的降级路径；使用新 DLL
时它也不会绕过固定资源目录。

Mo Simplifier 使用新增的 OpenCC 私有 C++ 严格加载方法。旧 OpenCC CLI
接口保留上游查找行为供构建工具使用，不是 Mo 的运行时加载路径。
严格方法以 RAII FILE 消费已经验证的同一个 Win32 handle，不在验证后
按路径重新 fopen。root/leaf 的 reparse point、目录伪装、多个硬链接、
不匹配的 final path 均拒绝；只接受本地 fixed drive。文件只读、非继承，
解析期间不共享写入或删除。资源路径要求平坦 ASCII 文件名，禁止绝对路径、
斜杠、ADS、NUL、设备保留名和任意扩展；词典只能是预编译 `.ocd2`。
依据：[Microsoft CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)、
[GetDriveTypeW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew)、
[_open_osfhandle 所有权](https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/open-osfhandle?view=msvc-170)。

JSON 文件最多 64 KiB，每份字典文件最多 64 MiB。严格 JSON 解析不修改
调用方字符串，采用迭代解析并验证 UTF-8；拒绝原始/转义 NUL、重复属性、
JSON 深度达到 32、dict 深度达到 16、空/超过 64 项 group、空/超过 16 项
conversion chain 及非对象 chain 元素。长度上限不是二进制反序列化内存
上界的证明，更不是资源内容认证。

构建器新增必填的已校验 Emoji pack 输入。编译前快照 Mo 补丁、native
源、构建脚本与 pack，仍只消费六份固定 Git archive。OpenCC/core 共用
Marisa 0.3.1、MT/CMP0091、Lua 允许列表及 cache 来源守卫不变。
format 2 provenance 记录快照输入、DLL、每份相邻资源及 pack manifest 的
SHA-256；保留 `development_only=true`、`redistributable=false`。
安装配置预检查新增固定 OpenCC 目录与六个必要文件，仍不创建或部署资源。

## 验证与证据

首份实验产物 `build/mo-runtime-relocatable` 的全新构建成功；28 项原生
same-handle 边界和 27 项真实引擎搬迁/解析检查通过，四项准备边界包含
旧 v1 拒绝，三个失败 Broker 均未宣布 ready。随后补强 fixed-drive 检查
和严格 JSON 非 in-situ 解析，用另一个全新目录构建最终产物，未覆盖实验
DLL。初版 provenance 检查脚本因 JSON 的根 archive key 是空字符串而拒绝
解析；修正为 PowerShell 7 的 `ConvertFrom-Json -AsHashtable`，未修改或
忽略 provenance 字段。UTF-8 fixture 后续改为只破坏合法 JSON 的字符串
字节，避免把普通语法错误误当编码验证证据。
原生 probe 的首条 MSVC 命令曾因带引号 `/Fo` 目录的末尾反斜杠转义报
D8036；修正命令转义后才完成 28 项检查，不把编译失败计作边界测试通过。

`build/mo-runtime-relocatable-final` 的第二份干净构建也成功，但生成工程
复核显示 deferred hook 的 source property 目录作用域不属于 `rime` target，
两个 wrapper 没有实际 W4/WX metadata。修改为 `TARGET_DIRECTORY rime`，
补充 CMake property 检查及编译前 MSBuild XML 守卫；旧工程明确被新守卫
拒绝，OpenCC 的原有 W4/WX source 则接受。匹配、缺 source、Level3、关闭
WX、重复 source、错误 configuration 条件和缺 metadata 的七项策略检查
通过。第三份新输出保留全部更正后的快照，不覆盖前两份实验记录。
依据：[CMake source property 的目录可见性](https://cmake.org/cmake/help/v3.31/command/set_source_files_properties.html)。

最终干净构建位于 `build/mo-runtime-relocatable-verified`，builder 完整命令
退出 0。DLL SHA-256：
`12498660ca4ad436da8a1ec3a7f277df356b624a9b25fad36f4e0883aa50fd15`。
33 份资源与 11 份 Mo 快照哈希核对通过；生成工程明确含两个 wrapper 的
Release/x64 W4/WX metadata，opener 也含 W4/WX/SDL/GS/CFG。公共 header
hash 仍为 `85caf744b4e5405a9a1de9c7aef3affc4ae315f4ae5d7ebdd08e191a2c16dad4`，
C/Rust ABI 的 51 项断言通过。导出检查只有 Mo v2，没有 v1 准备导出。
上游 core/Lua 仍有既有 warnings，不把整个 runtime 宣称为 W4/WX 零警告。

最终产物重新执行 27 项 `test-relocation.ps1` 全部通过，包括改进后的
UTF-8 fixture；四项 `test-preparation.ps1`（成功、缺资源、官方 DLL 缺
扩展、旧 v1）通过。三个失败 Broker 明确报 v2 准备错误且未宣布 ready。
成功/失败均核对空会话 context 不变、没有 commit；忙会话拒绝不清空，
新会话仍有 Emoji 👋，当前页选择准确提交一次 `你好`。28 项原生文件
边界通过。运行时自身 CLI/Marisa 读回两份字典全部 4857/1498 个 key，
每个 key 的有序 values 与源一致。五项 builder、四项 cache、七项生成
工程策略检查通过，测试只修改/删除本次新建、路径已核对的 fixture。

完整 prepared/trace 真实命令退出 0，x64/Win32 各 100/100 轮，合计
400 次实际 Broker 退出；同一命令包括 IPC/pool、候选窗、鼠标/翻页、
布局/焦点/延迟锁/重入、恢复后最终 EDIT/context 核对。没有放宽 50 ms、
禁用 Emoji 或重发。此轮只保留 std/err 文件日志与工具终端记录，PowerShell
Write-Host 的 info-stream 首键汇总没有完整进入该文件，不给出全样本
min/max；不能用局部可见计时冒充完整分布。

Rust 默认 109 项 runtime/1 项 compile-fail、trace 111 项/1 项 compile-fail、
fmt、默认/trace debug/release 四种 all-targets Clippy 与 doc 通过。五项
release 启动负向 smoke、16 项注册事务内存策略、20 项既有 OpenCC 检查
通过。CI 已接入必填 pack、opener、搬迁与生成工程守卫；未推送或运行远端。
收尾恢复默认关闭诊断构建，完整命令 x64/Win32 各 20/20 轮以及全部
IPC/pool/UI/edit 检查通过，另含 80 次实际退出；证明不依赖 trace feature。
两份 TIP 及各 probe 的 MSBuild 构建零警告/错误。默认收尾日志使用全部
PowerShell streams 捕获，核对各 20 个 trial 与完整成功 marker。
8 份 PowerShell AST 检查通过。最终只读 registrar 状态仍是双视图 COM
缺失、profile 未注册/启用/激活；未设默认输入法、未启动 UAC。

## 未通过的边界

- 本阶段只关闭 Mo 的 OpenCC 搜索路径；不是全安装树/祖先目录的原子
  reparse/ACL 验证，不保证目录被恶意并发修改时的整包一致性。
- provenance 与自声明哈希不是可信签名或更新认证。Native 没有据此认证
  所有资源内容；Rime/Lua/user/staging 的完整覆盖策略仍未闭环。
- 未实际注入网络/可移动卷、所有文件系统或内核取消故障；当前本机为
  Windows 10 的本地固定盘，不能升级为 Windows 11 全环境通过。
- 允许列表仍不等于许可证批准；逐文件 SBOM、通知、对应源/修改记录、
  签名与安装权限、更新及安装/升级/卸载回滚仍待完成。
- 注册后的系统路由、Notepad/WinUI/AppContainer、普通软件与混合 DPI
  仍未通过。受控 TSF 压力不构成桌面宿主、机器冷启动或完整 COM 返回
  时限保证，也不抹去 ADR 0022/0023 的历史负向证据。尚不可日常使用。
