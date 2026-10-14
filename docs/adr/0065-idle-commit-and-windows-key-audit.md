# ADR 0065：空闲直接提交与 Windows 全键映射审查

## 状态与问题

已接受；2026-10-14，develop。用户在 Win10 0180 反馈主键盘 /? 键英文及时上屏，
中文慢按不出字、快按仅少量出字；要求排查全部按键。

封存真实 Rime/Rime Ice 的合成审查中，中英文各 94 个可打印字符均有提交、预编辑或
交还宿主；中文斜杠快慢各四次均产生一次提交，抬键无重放。问题可在真实 TSF ACP
上下文与 fake engine 夹具稳定复现：空闲标点被 TestKey 判为消费，而插入返回 E_INVALIDARG；
已有预编辑时的标点完成通过。此前只测试后一路径，未覆盖空闲直接提交。

原空闲 CommitText 使用 TF_IAS_NOQUERY 和空 range 输出。该组合在
[Microsoft API 文档](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinsertatselection-inserttextatselection)
中合法；本项目 Win10 夹具实际拒绝该路径，因此改用返回 range 的插入方式。
原失败后断开并进入 250 ms 重连退避，在退避内按键交还应用；这能解释快按偶有输出，
但本轮未据此声称实测了用户每个物理按键的时序。

## 决策

- 无 owned composition 时，当前同步 RW edit cookie 下 InsertTextAtSelection(flags=0)，
  用 ComPtr 接收插入范围，Collapse 到尾端并 SetSelection 为折叠光标。
  不建立候选预编辑，不重放引擎请求。原已有 composition 的提交路径保留。
- 文本插入成功立即标记 applied，即使后续光标操作失败也消费原键，避免宿主再次插入。
  检查每个 HRESULT/空 range，错误仍断开；不放宽 50/400 ms 或退避合同。
  [SetSelection 合同](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-setselection)。
- Windows VK→逻辑键转换集中在 Broker windows_keys 模块。依据
  [Microsoft VK 表](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes)，
  保留字母、数字、OEM、Shift、Caps、导航和 F1–F24；补齐数字小键盘及运算符、
  OEM_102、Cancel/Clear/Pause/Help/Menu/NumLock/ScrollLock。
  小键盘数字不受 Shift/Caps 的主键盘转换影响。VK 编码及 keysym 表为合同数据，范围有语义名称。
- Ctrl/Alt/Super 和抬键不推断文本；未知、媒体、IME/dead/国际键用 NoSymbol，避免 ASCII 碰撞。
  当前实现限中文/US-style 布局；小键盘 decimal/separator 的其他区域行为及非 US OEM 未验收。
  这次不增加布局文本的线协议字段；wire、设置及已有协商格式不变。
- FakeBackend 的 ASCII 标点合成直接提交用于验证 TSF 插入及一次提交，不替代 Rime 标点规则。
  原生夹具在自己的线程设置合成键盘状态，结束恢复；不注入用户窗口、不注册系统 TIP。
- 光标和内容测试覆盖普通/Shift OEM 及小键盘符号、快慢和抬键；完整读取 EDIT/TSF 文本，
  原 32/64 单元夹具缓冲区被扩大矩阵暴露截断，改为完整读取并限定合成文档最多 4096 UTF-16 单元。

## 验证与边界

- 全 256 VK × 32 个已知修饰组合 × 按下/抬起 × repeat，共 32768 个转换情形，
  独立 golden 数据核对；另有超界 VK、跨 Broker 转换与 FakeBackend 一次提交检查。
- fmt、默认及 latency-trace strict Clippy、workspace 202 通过（含 doc）、1 ignored。
- x64/Win32 /W4 /WX、ABI、渲染/GDI、IPC、16 槽、真实 TSF 编辑锁、
  空闲标点每夹具 68 个直接提交、光标、抬键、重入、每架构两次 Broker 崩溃恢复通过。
- 真实 Rime 的 188 可打印字符状态情形、NoSymbol、八次斜杠快慢/抬键通过；
  21 个 Shift 符号按真实按下/抬起顺序不重放、不错误切为英文，Ctrl/Alt/Super+斜杠交还宿主。
  使用封存 runtime 和独立无学习用户目录，日志只含合成 case 编号/结果类别，无输入内容。
- 原插入 E_INVALIDARG、两次扩大夹具后截断失败日志保留；不把已修复测试失败隐藏为首次通过。

Win10 安装态物理按键仍需 0190 修复包后的用户反馈；转译矩阵不是所有硬件/布局的真实路由验收。
其他输入方案、非 US/dead/AltGr、媒体/系统 OS 路由、多 DPI、加载宿主升级、50/400 ms
首键及 G2/G3 继续按各自合同验收。
