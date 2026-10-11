# ADR 0060：紧凑候选、表情提交与原生皮肤设置

## 状态

已接受；2026-10-12 在 develop 实施。替代 ADR 0059 的呈现限制；历史验证记录仍有效。

## 呈现与设置

- 按已确认的 Figma 紧凑候选、表情、三页设置和应用图标实施。默认候选 560 × 86
  逻辑像素，工具栏四个 28 px 命中区。长词按真实字体测量；展开只展示当前后端页，
  三列网格可滚动，不生成额外候选。页大小仍由已编译 schema 决定。
- Snapshot 使用 Rime 页号、上一页/下一页可用性和高亮索引。没有总页数；
  没有元数据的旧端禁用翻页按钮。展开摘要显示当前真实数量。
- 五款皮肤和一份自定义配色；自定义限定 RGB24、圆角 8–20、字号 12–24、
  横排/竖排。设置页的窗口主题独立于候选皮肤。System 根据 Windows 应用主题切换；
  实际高对比度优先使用系统颜色、矩形与边框。
- 原生 Win32/GDI；中文 Microsoft YaHei UI，表情 Segoe UI Emoji。
  Windows 10 GDI 表情为单色。系统标题栏、阴影和字体度量与 Figma 有原生差异；
  不包含 WebView、下载字体或运行时 SVG 网络加载。
- 设置使用原生可访问的控件、键盘导航、弹出下拉框、滚动内容和固定保存区。
  自定义皮肤导入/导出为最多 1024 字节的 mo-skin 数据文件；
  不允许代码、外部路径、字体或未知字段。当前只有一份自定义设置，
  不提供命名皮肤库、最近表情或搜索；“保存为新皮肤”适配为导出文件。
- 新图标嵌入设置 EXE 和 TIP DLL 的图标资源；TSF 注册仍引用固定 DLL 的图标索引 0。
  本次没有重建安装包、修改 VM 安装或交付 WiX 图标验收。

## 表情与偏好

SYMBOLS_V1 固定 28 个 wire 索引，Rust/native 一致性由资产测试检查。
中文联想仍是独立 Rime emoji 偏好。手动面板可选择 Emoji/颜文字；
Esc 只关闭本地面板，保留组合文本。

手动插入经过连接 session、候选 revision、目录索引校验，再由唯一 Engine Actor
调用后端 Clear。只有清理返回无 composition/candidates/composing 时生成完整 Unicode
commit；清理失败不能伪造成功。TIP 延续原有 RW Edit Session、range 身份和焦点校验，
替换自己的预编辑范围，不直接 SendInput，也不重放旧拼音。

面板联想开关同时校验候选 revision 和设置 revision，复用 checked atomic save，
只改 emoji，保留其他偏好。损坏、未来格式、外部修改或过期请求拒绝写回；
稳定错误保留连接和当前组合文本。引擎偏好在空闲边界的新会话生效。

## 协议与文件兼容性

旧 kind 6/15 的 Snapshot、kind 14 的 18 字节设置和旧会话 ACK 不变。
新增 feature 位与消息仅在协商依赖满足时启用：

| Feature 位 | 消息 | 合同 |
| --- | --- | --- |
| 5 | 16 PresentationSnapshot | 8 字节页元数据前缀＋既有 DetailedSnapshot；依赖 details |
| 6 | 17 SymbolAction | expected revision u64＋索引 u32；依赖 candidate actions |
| 7 | 18 PresentationSettings | 旧设置 18 字节＋skin/三色/radius/font/vertical，共 34 字节 |
| 8 | 19 SetEmoji | 候选 revision u64＋设置 revision u64＋bool，共 17 字节 |

PresentationSettings 依赖 settings snapshot；QuickSettings 还依赖 candidate actions
和可写文件设置服务。presentation payload 协商下限 34268 字节。
无页元数据使用规范化空值，拒绝非法高亮、非布尔值、尾随字节和越界外观。

settings-v1.mo 的默认编码保持旧九字段；外观非默认时追加完整八字段。
**旧版本应用/ Broker 会拒绝新增字段，因此新设置应用、Broker、TIP 必须配套升级。**
旧格式读取和旧 wire 降级不等于新文件可被旧版读取，不能只替换设置 EXE。
升级/回滚安装矩阵待独立验证。

## 素材与验证

操作 SVG 保留 Figma 原始导出，生成七档 alpha mask，嵌入 PE 资源。
Smile 来自 Figma SDS MIT；固定源 revision、SVG/许可 SHA256、变换与完整通知
见 third_party/design-assets-20261011.md。其余图形为 Mo 原创 Apache-2.0。
发布材料由 15 增至 17 份；当前许可策略测试使用历史 stage，不是新产品打包证据。

最终源码、截图、成功与负向证据见
[原生 UI 改造记录](../design/MO-NATIVE-UI-20261012.md)。
真实 Rime 的首键失败保留；没有调整 50/400 ms，也未升级 G2/G3。
