# ADR 0018：首版原生候选窗与版本绑定的鼠标动作

- 状态：接受
- 日期：2026-09-16

## 背景

当前页选择与翻页已经过真实引擎验证，但日常输入必须有可见候选和鼠标交互。Windows TSF 鼠标回调不能照搬按键回调的同步写锁；被排队的旧点击也不能在继续输入、切换焦点或重建会话后误提交新词。

## 决策

- 第一版自主候选界面采用 TIP 内独立的 C++17 Win32/GDI 表现层 `CandidateWindow`。这是 Rust-first 路线的阶段性原生适配：它只负责窗口、文字绘制、布局与点击，不加载词库、不排序、不维护旁路输入状态。领域命令、引擎、页版本授权与 IPC 编解码仍在 Rust。跨平台界面、主题与富候选模型后续可以替换这一表现层，不依赖小狼毫前端。
- 沿用 1.0 Snapshot 编码，新增 feature bit `FEATURE_CANDIDATE_ACTIONS = 2` 和请求 kind `CandidateAction = 12`。只有 HelloAck 协商成功才显示可点击的自主候选窗。旧 peer 不协商该 bit 时仍能使用原按键协议，不发送新 kind。
- CandidateAction 是严格 13 字节：`expected_revision:u64 + action:u8 + index:u32`，小端编码；action 0/1/2 对应选择/前页/后页。revision 不得为零，选择是当前页零基 ordinal（小于 32），导航 index 必须为零，拒绝未知值、截断和尾随字节。
- Broker 按连接所属会话保存最近一次成功编码的非空候选页 revision/count。动作必须匹配这个版本并在当前页范围内，才进入 Engine Actor；无页/陈旧页返回错误码 8，未协商/越界/格式错误返回 1。每次有效命令尝试前撤销旧页授权，原生失败或编码失败不能留下可点击的旧页。另一会话的版本不能授权本会话。
- 候选窗为纵向列表，行号仅表示显示 ordinal，不伪装为 schema 实际选择键。提供鼠标前后翻页；未知页边界时保留两按钮，让原生 false 返回同页快照，不猜测最后一页。UTF-8 严格转换，显示控制字符替换为空格，长文本单行省略。候选数量超出屏幕高度时列表支持滚轮本页滚动。
- 使用 `WS_EX_NOACTIVATE`、`WS_EX_TOOLWINDOW`、`WM_MOUSEACTIVATE -> MA_NOACTIVATE`，以文档 view HWND 为 owner。按宿主 DPI 缩放，限定在对应显示器 work area，底部不足时放到预编辑上方；不以系统鼠标位置或其他进程窗口作为回退锚点。隐藏/无 layout/文本被裁剪/宿主要求 UI-element-only 时不显示自绘窗。允许有高度但零宽度的 collapsed caret rectangle；全零不可见矩形仍隐藏。
- 按下/抬起需命中同一项、同一 revision；页面刷新、隐藏或鼠标捕获丢失会清理按下状态。未被引擎消耗的 key-up 也更新候选页版本与界面，避免正常松键使可见页面立即陈旧。
- 鼠标点击创建持有 owner/context COM 引用的 CandidateEditSession，以 `TF_ES_ASYNCDONTCARE | TF_ES_READWRITE` 请求编辑。进入 DoEditSession 后再次检查焦点、context、连接 generation、session token、revision 与 owned range，再发送动作和应用快照。排队阶段不预先调用引擎、不持有借用快照、不缓存待上屏 commit。新按键或焦点丢失使排队动作无副作用取消；同一次点击隐藏界面，避免重复排队。
- composition 期间对称 advise/unadvise `ITfTextLayoutSink`。layout change 隐藏旧窗，再以有 request identity 的异步只读会话重新查询 caret；旧 read request 不能覆盖新 composition 的候选窗。宿主销毁 view 时撤销本地状态并关闭引擎会话。
- 曾成功连接过的 Broker 在 endpoint 重建间隙允许对 `ERROR_FILE_NOT_FOUND` 做 5 ms 以内的短重试，仍受同一端到端 deadline 限制，并对每个新 handle 重新校验服务端身份。首次 Broker 缺席立即 fail-open。重新获得焦点重置退避并尝试连接。
- 同一快照的 commit 和剩余 composition 分两段应用：先提交前缀，再创建剩余预编辑。已经写入的前缀仍标记 applied，后续错误不会把按键再次放给宿主。

## 验证与限制

Rust 增加协议严格编解码、feature gate、空页/跨会话/越界、翻页恢复、页内第二候选选择、松键版本、重放及原生失败撤销授权测试。

x64/Win32 受控 TSF 探针连接同一个 x64 Broker，分别对 fake 与真实锁定 librime/rime-ice 验证候选窗显隐、不抢焦点、鼠标翻页/第一候选上屏、松键刷新与旧按下保护、文档布局移动后的跟随；文本存储故意拒绝同步锁并延迟异步写锁，验证点击被新按键/焦点丢失取消，恢复焦点后再选词。最终从 EDIT 和 TSF context 双重核对三轮 committed text。IPC 探针另行核对真正跨页变化、页内第二候选 commit、陈旧动作被拒绝并重置客户端。

这些仍是受控文本存储和真实引擎证据，不是普通软件验收。真实注册后的 Notepad/WinUI/AppContainer、混合 DPI/多屏人工矩阵、secure desktop、TSF UIElement 宿主协作、schema 标签/高亮/注释/页边界投影、主题、异常进程恢复与安装发行仍待完成。单实例管道尚不能同时服务多个长期连接的应用，因此仍不可日常使用。

鼠标编辑锁与定位规则依据 Microsoft 官方 [RequestEditSession](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession)、[GetTextExt](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextview-gettextext) 和 [不激活的交互窗口说明](https://devblogs.microsoft.com/oldnewthing/20160912-00/?p=94295)。
