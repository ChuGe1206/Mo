# Mo 中英融合墨签设计 · 2026-10-29

[Figma 设计展示](https://www.figma.com/design/pFUWDZYKrLw9dxdNQ4XanU/Mo?node-id=2833-637)
位于「04 · Mo 应用图标」，主组件集为 `2833:636`；完整节点清单见
[figma-state.json](figma-state.json)。

## 视觉与交互意图

- 以统一圆角墨签外框和底部短墨点承接原 Mo 图标的圆角及墨点特征。
- 中心「中」表示中文输入，「A」表示英文直输。小尺寸优先显示模式，避免挤入额外品牌字母。
- 模式主组件为 16×16px 可编辑矢量；中文、英文、未知、不可用四种状态与浅／深两种主题，共八个变体。
- 展示帧使用组件实例，覆盖 16／20／24px、浅／深任务栏；沿用现有 Mo 色彩、间距变量和 Noto Sans SC 文字样式。
- 目标交互：左键或 Shift 切换模式；右键提供设置、表情及更多入口；悬停说明产品和当前模式。
  本轮仅设计展示，右键整合菜单尚未实现。

## Windows 集成边界

Windows 原生 Input Indicator 分别控制输入法品牌图标与模式图标。前者来自 Profile 注册资源，
后者由 `GUID_LBI_INPUTMODE` 提供。本稿展示用户期望的单入口目标视觉，不能将隐藏了品牌槽的
效果图当作已完成的 Win10 集成。更换模式图形本身不会删除系统品牌槽；不采用透明图标假装减少占位，
也不在切换模式时重写 Profile 注册或默认隐藏系统输入指示器。

采用现有 `TF_LBI_STYLE_TEXTCOLORICON` 时，生产模式图标应为单色；展示中的绿色卡片仅为品牌预览。
单槽集成方式需要另行验证并保留系统输入法切换能力。

依据：微软 [IME requirements](https://learn.microsoft.com/en-us/windows/apps/develop/input/input-method-editor-requirements)
与 [GetIcon](https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/nf-ctfutb-itflangbaritembutton-geticon)。

## 检查结果

- Figma 结构：34 Frame、28 Text、9 Instance、9 Group、28 Vector，无图像填充；文字均为 Noto Sans SC。
- 1060×798 展示图已检查，无裁切、重叠或未替换占位；最终预览见 [preview.png](preview.png)。
- 本轮没有修改产品代码、安装资源或封存包；已安装 0.0.20.0 的状态同步行为保持当前实现。

图形为项目原创，沿用仓库 Apache-2.0 许可证。预览包含合成日期及时间，不含真实输入。
