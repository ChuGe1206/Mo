# ADR 0017：当前页候选选择与翻页能力

- 状态：接受
- 日期：2026-09-16

## 背景

Mo 已能完成空格提交第一候选，但可见候选界面还需要可靠的当前页选择和翻页操作。已有领域命令 `SelectCandidate`/`ChangePage` 不能长期在真实后端返回 unsupported；同时 Windows PageUp/PageDown 尚未映射成 librime 的 X11 按键符号，会被误当成 ASCII 标点。

## 决策

- 保持 `RimeApi` 到 `free_status` 的基础必需前缀不变，新增可选候选 API extension。只允许调用锁定 header 中的 `select_candidate_on_current_page(session, size_t)` 与 `change_page(session, Bool)`。
- 中间 47/22 个函数指针槽只作为不读取、不调用的 layout-only 保留跨度；官方 C probe 核对整个尾部 size/alignment、跨度起点、两个 typed slot offset 与 data_size，总计 51 项断言。不把未知函数签名加入可调用 allowlist。
- 安全层独立验证每个完整 optional field 的 advertised range，再从原始完整分配来源的指针读取函数指针；不在旧表上创建完整 extension 引用。旧表、字段差一字节或空指针均保持不可用，不影响基础键盘功能。
- 安全 Session 和 RimeBackend 将页内零基索引/方向映射到 native API，保留 native false 作为未处理结果。Actor 继续统一分配 revision 并复制 owned commit/context/status。
- Broker 补齐 PageUp/PageDown/Home/End 的 VK 到 X11 keysym 映射，不改变 wire 协议；数字选词继续由 schema 的按键处理完成。
- 真实 smoke 增加 Actor API 翻页/选第二候选，以及 x64/Win32 C++ IPC 的 `ni -> PageDown -> PageUp -> 2`，核对页变化、原页恢复、翻页无 commit 和准确候选提交。每条真实链路使用独立可回收的用户目录。

## 结果与剩余工作

候选交互所需的真实引擎命令已可用，键盘翻页与数字选词有跨架构真实验证。此轮不包含候选点击的 wire 消息、revision 防陈旧点击、页信息/标签的 IPC 投影或候选窗，因此不宣称普通 Windows 宿主的候选体验已经完成。下一检查点是这些命令与自主候选界面的连接。

函数顺序与签名依据 [锁定版官方 rime_api.h](https://github.com/rime/librime/blob/33e78140250125871856cdc5b42ddc6a5fcd3cd4/src/rime_api.h)。
