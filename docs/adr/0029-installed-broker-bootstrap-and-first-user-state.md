# ADR 0029：安装态 Broker 拉起与首次用户目录 bootstrap

- 日期：2026-09-20
- 状态：实现完成；固定路径策略与开发素材回归通过，尚未完成真实安装/系统路由验收。本文记录的用户 `build` 已由 ADR 0039 取消，现只创建 `LocalAppData\Mo\Rime`。
- 延续：ADR 0015、0016、0019、0026、0028；不改变 IPC、TSF 注册权限或默认输入法。

## 问题

此前 TIP 只会连接已经运行的 Broker；正常用户必须由外部脚本先启动
Broker。release Broker 又要求 `%LocalAppData%\Mo\Rime` 和其 `build`
目录预先存在。两者都不符合“一次安装后直接使用”的产品目标，也使
Broker 崩溃后的恢复依赖测试 harness 或未来托盘进程。

## 决策

TIP 将模块位置解析与进程创建拆为独立、可探测的小模块。连接身份仍
只从 TIP 自身路径推出，不接受环境变量、当前目录、注册表覆盖或调用者
参数。只有模块恰好位于系统机器根
`Program Files\Mo\tip\<x64|x86>\mo-tip.dll` 时才授予自动拉起权限；
仓库输出和任意搬迁的 `Mo\tip\...` fixture 仍是 connect-only。

64 位宿主用 `FOLDERID_ProgramFilesX64` 取得机器根。实际双架构探针发现
32 位进程在本机对该 Known Folder 返回 `ERROR_FILE_NOT_FOUND`，所以 32 位
回退只读 HKLM 的 64 位 `CurrentVersion\ProgramFilesDir` 视图。不会读取
`ProgramW6432`/`ProgramFiles` 环境变量。该回退仍是 OS 管理的机器配置，
但其 ACL/祖先路径与签名属于后续发行门。

当且仅当 16 个管道槽全部报告端点缺失，TIP 才以明确的绝对
`mo-broker.exe` application name 调用 `CreateProcessW`：不经过 shell、
不传参数、不继承 handle、不开控制台，并拒绝目录、相对路径、错误文件名
和 Broker image 自身的 reparse point。创建后立即关闭 process/thread
handle，不把 Broker 生命周期绑到任一宿主进程。连接建立后仍执行既有的
同登录会话 SID、服务端 PID 和文件身份校验；进程创建成功本身不构成信任。

首次激活、焦点恢复或崩溃后的下一次有界重连都可触发该路径。普通连接
退避保持 250 ms，进程创建另有 2 s 节流，避免冷启动期间同一宿主反复
创建 contender。多个应用同时首次激活时仍可能短暂创建多个进程；已有
16 槽 first-instance 原子绑定决定唯一存活 Broker，失败者在初始化引擎前
退出。本阶段保证“单一存活服务”，不虚构“全系统只调用一次 CreateProcess”。

release Broker 在核对自身固定映像、机器级 shared/prebuilt/OpenCC/DLL
全部完整后，才处理用户目录。它只逐级创建精确的
`LocalAppData\Mo\Rime\build`，要求 LocalAppData 根已存在，拒绝逃逸字段、
预先存在的文件或每一级 reparse point。空 user/staging 是合法首启状态；
预编译词库继续只来自机器安装根，不复制 YAML/词库给用户维护。并发创建
采用 `create_dir + AlreadyExists 后复核`，不会删除或覆盖已有用户数据。

## 验证证据

- debug/release 的安装布局单元测试分别通过 8/6 项；首次用户目录从完全
  不存在变为准确的 `Mo/Rime/build`，逃逸路径和文件占位均拒绝。
- x64/Win32 `/W4 /WX /sdl /guard:cf` 编译与 ABI probe 通过。探针验证
  Program Files 精确授权、搬迁/仓库布局拒绝自动拉起、相对 image 拒绝，
  并把自身复制为一次性 `mo-broker.exe`，证明无参数隐藏进程可启动且
  handle 不由产品代码保留。32 位 Known Folder 负向结果保留并由 64 位
  注册表视图回退关闭。
- workspace 默认 110 项运行时测试与 1 项 compile-fail、debug/release
  all-targets Clippy、fmt 通过。release 启动的五项参数/映像负向 smoke
  继续全部拒绝。
- fake 双架构完成 IPC、16 槽、候选/Edit Session 及每架构 3 轮故障恢复，
  共 12 次明确 Broker 退出；没有 commit replay。
- 全新开发素材位于 `build/mo-windows-stage-broker-bootstrap/stage`：72 份
  Mo source snapshot、137 个 payload/evidence 文件，清单 SHA-256 为
  `C90F600DEE154E293884B3835525101DCFAB2CAB6617BB3580746BEC99681FB1`。
  双架构原生 rebuild 均零警告，79 项 staging policy 通过。
- 同一素材完成 7 组 prebuilt-only golden、无输入资源准备与 Actor 候选
  翻页/选择；x64/Win32 各 10 轮真实 rime-ice 故障恢复，共 40 次明确
  Broker 退出。素材回归使用同树 diagnostic Broker，不冒充 release
  安装态自动拉起。
- 收尾只读状态仍为 x64/x86 COM missing、profile registered/enabled/active
  全 false；没有注册输入法、启动 UAC、安装文件或修改默认输入法。

## 明确保留的边界

本阶段没有把 release payload 写入 Program Files，因此尚未端到端证明
“系统注册 TIP -> 精确安装路径自动启动 release Broker -> 首次 LocalAppData
创建 -> 普通应用输入”。固定路径授权和进程创建分别有确定性探针，完整
组合必须在可回滚安装事务与管理员明确准备后验收。

`CreateProcessW` 是同步 Windows 调用，不受 IPC 的 50 ms deadline 安全
取消；若 Broker 在按键期间刚崩溃，触发恢复的那个按键仍 fail-open，且
进程创建本身可能增加该次回调延迟。后续应由安装态常驻/计划启动机制或
安全的异步监督器消除热路径创建进程；当前实现优先保证不吞键、不重放和
最终恢复。AppContainer/低完整性宿主的进程创建权限也未验收。

机器安装树祖先 ACL/reparse/hard-link、handle-based 抗并发替换、代码签名、
用户目录 DACL、跨会话协调、空闲退出、托盘/设置、更新与系统服务停机仍
未完成。当前 Broker 常驻到进程被显式结束或故障退出。以上限制意味着
G3 仍未通过，项目仍不可宣称日常使用稳定。
