# ADR 0058：候选窗口遵从 Windows 高对比度配色

## 状态

已接受；Win10 本机双架构受控回归通过。真实高对比度切换、安装态宿主与 VM 视觉验收尚未完成。

## 背景

候选窗口的 Light/Dark 固定颜色优先于系统辅助功能，开启高对比度时仍可能显示原配色。
System 模式的预编辑使用灰色文本，页脚使用按钮色；高对比度下应使用窗口正文的前景/背景组合。

## 决策

1. 将配色解析抽成纯策略。HCF_HIGHCONTRASTON 优先于所有保存主题；
   正文、预编辑和页脚使用 COLOR_WINDOW/COLOR_WINDOWTEXT，
   按压态使用 COLOR_HIGHLIGHT/COLOR_HIGHLIGHTTEXT。
   查询失败保守使用系统配色，不把 HCF_AVAILABLE/HCF_HOTKEYACTIVE 当成开启。
2. 保存的 theme 值不变；高对比度退出后恢复原 Light/Dark/System 配色。
   没有新增 settings 字段、IPC 请求或引擎行为。
3. 窗口首次创建及 WM_SYSCOLORCHANGE/WM_SETTINGCHANGE/WM_THEMECHANGED
   读取辅助功能状态并使绘制失效。普通 Paint 和后续逐键更新不重复查询；
   首次候选创建仍有一次查询，不声称这次查询完全不在首键路径。
   外观消息不更新页面 revision、按压身份、焦点、锚点或 Broker 会话。
4. 使用 SystemParametersInfoW / SPI_GETHIGHCONTRAST，只读 dwFlags。
   不读取、保存或释放 Unicode scheme 指针。Microsoft WPF 使用同样的 flags-only
   查询。HIGHCONTRASTW 文档的 LocalFree 描述与本机实际行为存在差异：
   本轮早期照此释放指针导致 x64 候选创建时 0xC0000374；去掉释放后同一双架构
   烟测通过。没有采用猜测式所有权判断、修改系统高对比度设置或改用 ANSI 查询。

## 验证

- 双架构 /W4 /WX Release 编译、DLL 加载、COM 生命周期和纯配色策略通过。
  三种主题的高对比度覆盖、关闭后的原配色、查询失败和辅助标志区分通过。
- 真实 EDIT/受控 TSF 窗口上发送三个外观消息，验证失效后重新绘制，
  焦点、文本和窗口矩形不变；其后候选分页、鼠标选择、失效取消和提交继续通过。
- tools/tip-broker-smoke.ps1 -Architecture All -RustToolchain 1.97.1 通过：
  身份拒绝、16 连接池/三轮恢复、候选/编辑及每架构两次 Broker 崩溃恢复。
  不注册或启用 TIP，只有合成输入。
- Rust fmt、Clippy -D warnings、workspace 测试通过。一个 ignored 设置 child
  入口由父测试显式调用；没有隐藏失败。
- 原堆损坏日志与实施脚本保留；失败 PE 已被最终构建覆盖，不声称归档了失败 PE。
  修复后的实际 PE、源码和日志绑定在
  [证据清单](../phase-0/evidence/WIN10-CANDIDATE-CONTRAST-20261008.json)。

## 边界与参考

未执行系统高对比度开关切换、真实宿主/混合 DPI/屏幕阅读器测试，
未重建 stage/安装包或操作真实用户设置。真实 Rime 烟测未在本轮运行；
50 ms key / 400 ms activation 与 G2/G3 仍未整体通过。时延专项按用户要求暂缓。

- [Microsoft 高对比度参数](https://learn.microsoft.com/en-us/windows/win32/winauto/high-contrast-parameter)
- [HIGHCONTRASTW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-highcontrastw)
- [Microsoft WPF SystemParameters.HighContrast 实现](https://github.com/dotnet/wpf/blob/main/src/Microsoft.DotNet.Wpf/src/PresentationFramework/System/Windows/SystemParameters.cs)
