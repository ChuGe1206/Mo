# ADR 0019：固定容量的安全并发连接槽

- 状态：接受
- 日期：2026-09-17
- 演进：替代 ADR 0013 的生产串行监听及 ADR 0018 中尚未并发的限制；不撤销服务端身份校验或发布模式约束。

## 背景

输入法需要同时服务多个应用的长期连接。当前 protected logon SID DACL 刻意排除 `FILE_CREATE_PIPE_INSTANCE`；Windows 的同名多实例创建涉及该权限，不能仅因为 TIP 已校验服务端映像就扩大客户端权限。第一阶段选取固定数量的独立名称，继续使用每个名称的 first-instance 保护和原权限掩码。

## 决策

- 固定 16 槽：`\\.\pipe\LOCAL\Mo.Input.Broker.v1` 与后缀 `.s01` 至 `.s15`（十进制两位）。每个名称只有一个实例，全部使用 `FILE_FLAG_FIRST_PIPE_INSTANCE`、protected logon SID DACL、远程拒绝和原客户端认证。槽数/名称不是来自配置、环境变量或 peer。
- `PipePool::bind` 在开始接收前绑定整池。任何槽冲突或创建失败会释放本次已创建的槽，保留原冲突对象，不降级为部分池。引擎工厂和全部工作线程创建成功后释放 startup gate，随后输出统一就绪信号。
- 每槽保留原始 server handle，accepted stream 通过 `File::try_clone` 持有同一内核对象的副本。stream 析构先断开客户端，再释放副本和独占 lease；下一客户端复用原对象。保留 handle 期间不销毁/重建名称、不改 DACL，第二 live accept 或混用旧消费式 accept 被拒绝。
- 一个槽一个同步连接工作线程，全部共享同一个 EngineService/Actor。librime 的创建、操作和销毁始终在唯一引擎线程。固定上限为 16 个已认证或未认证连接；每连接既有的 64 个 session 上限不变，不按新客户端无限创建线程。
- C++ 客户端以轮转起点扫描全部固定槽，繁忙时最多间隔 5 ms 再扫描，扫描/重试/握手共用既有总 deadline。首次全部名称缺失立即 fail-open；此前认证过的 Broker 可在同一时限内等待重启。每次拿到 handle 都先校验 PID/logon/映像身份，异常身份或非预期系统错误直接拒绝，不跳过可疑槽继续连接。
- IPC 1.0 编码与 feature 不变。每连接有独立 generation/session/request id，候选授权仍绑定本会话最后成功编码的页 revision，而非全局最新 revision。另一连接推进 Actor 不会使未修改的候选页失效，也不能拿其 revision 授权本会话。
- generation 使用进程内一次初始化的种子与不会回绕的单调计数器异或，跳过零。不同并发计数在同一进程内不会碰撞；种子不是秘密，generation 不是认证凭据，也不声称跨进程重启的密码学唯一性。

## 验证

Rust 内核对象测试核对全部槽的实际 DACL/远程拒绝/first-instance，冲突绑定的原子回滚、live lease 排他、断线复用期间名称不被夺取。Broker 测试验证两路保持会话时隔离候选授权、槽复用生成新会话、静默首帧不阻塞另一槽，并发 1024 个 generation 非零且无重复。

x64/Win32 C++ probe 分别对 fake 和锁定真实 librime/rime-ice 保持 16 路连接，验证第 17 路在总时限内失败、释放中间槽后的接入、以及其他原连接各自以旧的有效页版本精确提交自己的词。原候选窗/鼠标/布局/延迟锁取消/重连受控 TSF 回归仍通过。

这些证明有界并发传输和真实引擎会话隔离，不替代注册后的普通应用验收，也不代表发布目录、签名或 AppContainer 安全性已通过。

## 限制与后续

- 静默首帧及开始组装的帧仍有 2 秒时限；已认证完全空闲连接仍永久占一个槽。16 槽满载时 fail-open，而非等待无限增长。后续需要租约、空闲回收和真实多应用容量评估。
- 服务端同步读写仍未覆盖完整逐请求 deadline，慢读响应、引擎长操作、连接工作线程 panic/异常退出、协调停机及 Broker 崩溃恢复仍需故障注入。连接池不是这一整套可靠性问题的终点。
- 旧串行/rearm API 保留用于兼容测试，生产入口只使用池。未来可替换为 overlapped 服务端或经审查的更大容量，不破坏领域/Actor/IPC 边界。
- 真实注册宿主、WinUI/AppContainer、Windows 11 多屏 DPI、正式自构建 librime、资源许可证/SBOM、签名安装与升级卸载仍是发布门。

权限与内核对象规则参考 Microsoft 官方 [Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights) 和 [Named Pipe Instances](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-instances)。
