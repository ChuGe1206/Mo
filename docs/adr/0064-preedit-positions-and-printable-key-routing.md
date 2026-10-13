# ADR 0064：预编辑位置与可打印键转换

## 状态与原因

已接受；2026-10-13，develop。用户在 Win10 0160 确认候选、箭头翻页、表情及 Esc 正常，
但左右键改变引擎编辑位置而可见光标不动；标点先进入文档并留下候选。
Broker 原转换表遗漏 OEM 标点，协议也丢弃了领域 Composition 的 cursor/selection 元数据。

## 决策

- Broker 将支持的中文/US-style Windows OEM 键和 Shift 数字转为对应可打印逻辑符号。
  不修改 KeyEvent 线字节。CapsLock 只参与字母大小写；keyup 或 Ctrl/Alt/Super 不生成文本。
  功能键、Print/Insert 和修饰键转为独立 X11 keysym，未知 VK 使用 NoSymbol，避免数字碰撞。
  Space 保持控制键语义，空闲 Space 不产生新的预编辑。
  编码依据 [Microsoft VK 表](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes)。
  OEM_1 等随键盘布局变化，当前转换范围为现有中文/US-style 布局，非 US OEM 布局未验收。
- FEATURE_COMPOSITION_POSITIONS 为位 9，仅在 details＋presentation 和足够载荷预算下协商。
  新响应 CompositionSnapshot=20；13 字节前缀为 presence 和三个 little-endian u32 UTF-8 byte offset，
  依次 cursor、selection_start、selection_end，再接完整既有 PresentationSnapshot。
  原 Snapshot/DetailedSnapshot/PresentationSnapshot 编码和设置格式不变，旧端继续使用旧响应。
- 存在非空预编辑时必须有位置；空预编辑必须没有位置且三个编码偏移均为 0。
  所有位置必须落在字符串内且为 UTF-8 scalar boundary，selection_start≤selection_end。
  文本与候选限额、64 KiB frame 上限、50/400 ms 预算及关联 revision/generation 保持。
- 原生解析后按 UTF-16 code unit 转换，补充平面字符占两个 unit。当前 RW edit cookie 下
  克隆自有 composition range、ShiftStart 到光标并 Collapse，使用
  [ITfContext::SetSelection](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-setselection)
  同步文档 caret；每个可重入 COM 步骤后检查原有 context/range/epoch/revision/generation/token。
  不把 librime 分段变为宿主高亮文本选择，不抢焦点，不重放引擎动作。
- 候选头部显示静态光标线及当前分段下划线；过长预编辑向光标处水平滚动并裁剪到头部。
  维持默认 299×60 和全部五词布局。旧端没有位置时维持旧绘制。
- 左右/Home/End 用于拼写编辑；PageUp/PageDown 用于翻页。配套 default.yaml 绑定 -/= 翻页，
  原生翻页 tooltip 标明这些按键；不把左右编辑键改为翻页。
- FakeBackend 扩展光标移动及合成标点完成，服务于确定性回归，不作为 Rime 预测/标点规则。

## 验证与边界

独立协议 prefix golden、UTF-8 边界/错误偏移/缺失元数据、能力依赖和低载荷预算回退通过。
Broker 跨组件检查标点、Shift 数字、系统键隔离、左右光标位置和标点后空 composition。
双架构真实 TSF 夹具检查 collapsed caret、Home/End/PageUp/PageDown、标点完成、候选关闭、
重入/取消、16 槽池和每架构两次 Broker 退出恢复，通过；这些输入使用合成 fake engine。
Win10 原生 renderer 20 幅、exit 0；补充 Unicode 偏移独立验证，光标/分段图为 299×60。

真实 Rime/记事本中的修复仍需配套 0170 安装后验证。GUI 自动捕获超时，用户执行真实交互；
安装态 0160 的用户确认不能当作 0170 通过。非 US OEM、物理多 DPI、高对比度、加载宿主
升级矩阵和真实首键 G2/G3 仍待验收。失败日志和首轮 0170 v1 封存包保留；升级使用 v2。
