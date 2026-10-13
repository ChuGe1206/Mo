# Win10 紧凑候选注释与 0.0.18.0 配套包

2026-10-13，develop。[设计](../design/MO-COMPACT-EXPANDED-20261013.md) ·
[结构化证据](evidence/WIN10-COMPACT-EXPANDED-0180-20261013.json)。
当前实际安装 0170；最终 0180 包已核验并复制到客机，普通权限 driver 已启动，正等待审计 UAC。

## 实现与门禁

- 无注释时不展开；有注释时按内容排列、完整换行及像素滚动，无额外标题和固定三列。
- 具名视图枚举、具名最大宽360逻辑像素/注释间距；共用默认候选文本槽、命中区域和滚动。
- Win10最终22幅合成渲染exit0：六词带注释291×81，无注释nihao299×60；长注释末尾可达。
- 枚举迁移前后22/22像素一致；Figma六皮肤主组件和评审实例同步291×81，视觉检查通过。
- 固定Rust1.97.1：fmt、默认/latency-trace strict Clippy、199通过（含doc）、1ignored。
- x64/Win32 /W4 /WX、原生文本几何/生命周期/GDI、Broker IPC/16槽/选择/表情/重入，
  每架构两次Broker崩溃恢复通过。没有修改协议、设置格式或50/400ms合同。
- 最终stage126源文件库存，3个变更源码逐项哈希匹配；MSI/Burn、ICE、Changed pair、14文件kit通过。

## 唯一当前升级入口

- stage：build/mo-windows-stage-expanded-0180-v3/stage。
- installer：build/mo-linked-win10-expanded-0180-v3。
- kit：build/mo-vm-changed-upgrade-kit-0170-0180-v2。
- guest：C:/MoDev/Packages/0.0.18.0-v2；共享Package0180-v2。
- 新证据：C:/MoDev/Evidence/Upgrade0180-v2；不得重复启动已发起的driver。
- bundle SHA256：47EA4494074CB9D5B6971982BE16EE80CF599FA47480D08339892C469EF40FDA。

用户授权下自主重启；新boot UTC为2026-10-13T12:06:56.5000000Z，session1、Broker/设置均0。
启动通道曾未就绪、截图横线及guestcontrol20；用户处理桌面后最小通道恢复。
长复制命令超时不代表复制失败：原回执随后完成14文件核验，未重复复制或重启。
现在截图已确认Windows PowerShell/Microsoft Windows审计UAC，安全按钮仍由用户处理。

## 失败保留与缓存

早期stable已变为1.98.1，被固定工具链门禁拒绝；最终显式使用1.97.1。
一次安装包调用早于stage完成，被路径检查拒绝；枚举迁移遗漏表情三元分支被严格编译拒绝并修正。
日志均保留；v2 stage/installer和v1 kit不用于升级。Figma第一次只读输出超过20KiB，缩小范围后成功。
本轮不据合成结果宣称0180安装态已通过；0170用户光标/翻页/标点有限验收保持。

20:14（Asia/Shanghai）任务空闲、metadata默认target和无重解析点核验后cargo clean：
5,463文件、1,004,029,757逻辑字节（约0.94GiB）→0，磁盘空闲增加约0.90GiB。
封存stage、安装包、runtime、证据和下载缓存保留；不为预热重建。
物理多DPI、高对比度、非US OEM、加载宿主矩阵和G2/G3仍开放；产物为unsigned DevelopmentTest。
