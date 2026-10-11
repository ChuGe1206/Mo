# ADR 0059：Figma 候选设计的 Win10 原生呈现

## 状态

已接受；2026-10-11 本机 x64/Win32 受控回归和原生渲染检查通过。

## 决策

- 从 Figma 的纸白竖排 4:69、夜墨竖排 4:120 的高保真上下文实现。
  继续使用 CandidateWindow / Win32 GDI；没有引入 WebView、React 或字体包。
- 现有 Light / Dark 映射纸白 / 夜墨色板；System 保持系统配色。
  不新增设置字段或 IPC。其余三款皮肤、自定义、横排和设置页面仍待实施。
- 340 px 基准宽度、12 px 外边距/圆角、34 px 行加 4 px 间隙；
  Microsoft YaHei UI 使用 14/16/12 px 预编辑/候选/辅助文字，
  按 owner DPI 缩放。字体仅在 DPI 改变或创建失败后重建。
  原生字体与 Figma 的 Noto Sans SC 字形有差异；不分发字体文件。
- 标签、正文、注释分开绘制，长文本省略，超长标签有界。
  首项采用主建议强调，按压时强调被按项目；不是 librime 高亮游标状态的投影。
  仍按真实页内索引和严格 revision 派发，不改引擎排序或空格语义。
- 页脚保留可操作的上一页/下一页。当前 Snapshot 没有页号和总页数，
  不展示设计稿合成的 1/3；也不把标签承诺成引擎数字选择键。
- 普通窗口用圆角 region 和细描边，阴影由 Windows 的 CS_DROPSHADOW 策略控制，
  不复刻 Figma 的软阴影像素。SetWindowRgn 成功后系统拥有 region，失败时释放。
  高对比度使用矩形与系统颜色，遵从 ADR 0058，边框取 COLOR_WINDOWFRAME。
- NOACTIVATE、owner、work area、鼠标 capture 与旧 revision 取消继续使用既有合同。
  行间空隙和外边距不提交；失去 capture 时清理并重绘。

## 验证

build-probe 增加双架构的 synthetic-only candidate preview 探针，CMake/MSBuild
目标均声明，实际验证 MSBuild 路径。探针不注册 TIP、不运行 Rime、不改用户设置；
创建自己的临时 owner 和候选窗口，仅通过 WM_PRINTCLIENT 保存受控合成文本。

- 纸白/夜墨截图 340 × 272，视觉复核通过；末次缓存优化后 BMP 与复核版本 SHA 相同。
- 三主题外观消息后的 focus/bounds 保持、鼠标 nonactivation、第二项点选、
  行间/外边距不提交、按压后 Update 的旧 revision 不提交均通过。
- 32 条合成长候选贴近工作区右下角时 popup 保持在 work area 内。
- 同 DPI 连续 200 次 Update 后 GDI 对象数未持续增长。
- /W4 /WX 双架构、ABI/registrar、fake Broker 16 槽与 TSF 提交、
  延迟取消、每架构两次 Broker 崩溃恢复通过。
- Rust fmt、Clippy -D warnings、workspace 163 项测试通过；
  1 个 ignored child 入口仍由父测试调用。

## 边界

本轮没有打包新 MSI、更新 VM 或运行真实 librime 输入；
0.0.13.0 开发包/0.0.11.0 最后已验证 VM 保持历史身份。
真实 OS 高对比度切换、安装态视觉、多个真实 DPI 和系统阴影尚待验证。
未放宽 50/400 ms 时限，G2/G3 不升级。

截图与绑定见 [实施证据](../design/MO-NATIVE-CANDIDATE-20261011.md)。
