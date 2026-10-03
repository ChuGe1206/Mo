# Win10 记事本与设置有限实测

日期：2026-10-03；VM Windows 10 22H2 x64 build 19045。安装 0.0.10.0 unsigned DevelopmentTest Bundle exit 0；安装文件、权限、机器 COM 校验通过，默认输入法 override 未改。全部编辑内容为合成测试输入。

## 新发现与修复

原安装 DLL 单个按键重复进入预编辑。独立诊断记录显示 test/key 回调低字 repeat count 不同，其他缓存字段相同。修复只忽略低 16 位重复计数，详见 ADR 0054。原包自身的桌面输入不能判定通过；安装生命周期证据仍有效。

## 本轮有限通过

| 检查 | 实测结果 |
| --- | --- |
| Win+Space 手动切到 Mo | Win10 输入法列表可选、真实记事本加载 x64 Mo DLL |
| Broker bootstrap | 从固定 Program Files\Mo\bin\mo-broker.exe 自动启动 |
| 单键与候选窗 | 修复 DLL 的单键只出现一次，候选窗按 caret 定位 |
| 连续两次空格提交 | EDIT 精确回读四个 UTF-16 字符，均与预期一致；无重复上屏 |
| 设置 UI | 首次运行窗口可打开，保存全拼/繁体/深色后用户文件核验通过 |
| 设置生效后的输入 | 深色候选含繁体词，空格提交后 EDIT 精确回读六个 UTF-16 字符；前两次提交保留 |
| 恢复默认设置 | 通过实际 UI 回调恢复简体/跟随系统；测试创建的默认设置文件保留 |
| VM 原载荷恢复 | 测试窗口关闭、原 DLL 哈希恢复、132-file payload/ACL/COM 再核验通过 |

这段功能验证使用已安装的真实 Broker/词库与临时替换的默认构建 DLL。修复 DLL 尚未打包，因此不作为新安装包验收。原 DLL SHA-256 为 `5382DEE9882BA597D66B2276B50E9E4DBEF344BA4164A9ED248AD919D9DB65BC`；VM 功能测试 DLL 为 `0988EBA4D8742ADF36DC05285DD20A333504F7D61E59EE98A9E502AF4A2FDAA6`。后续关闭 trace 的独立重建 DLL 为 `7762663A3DFA9C6040A6F3AC31838D5208A3785162D6589CD5545E6F148DA08A`；按源码及各自哈希分别绑定，不声称两者字节相同。

证据目录：`build/win10-evidence-clean-v1/Desktop0100-v1`；包含 install-state、route metadata、两项记事本精确回读、设置保存/恢复、载荷恢复 JSON、合成截图、测试 DLL 与源码副本。`host-evidence-manifest.json` 绑定文件 size/SHA-256。

## 主机回归

- `build/win10-key-count-build.log` 与 `win10-key-count-default-final.log`：x64/Win32 编译、ABI/registrar 非变更探针通过；默认构建恢复。
- `build/win10-key-count-fake-smoke.log`：双架构 IPC、16-client pool、TIP/edit、候选/鼠标/布局/终止，以及各一轮双次 Broker 故障恢复通过。
- `build/win10-key-count-regression.json`：同一新探针，旧 DLL exit 1、修复 DLL exit 0；失败 marker 为 `TSF repeated query changed its pending key decision`。正负日志独立保存。

## 保留的失败与未覆盖项

真实词库完整脚本的两次 default 运行都在 x64 首键测试失败（47/46 ms 的外层整数计时）；trace 记录 `phase=3 error=1460 total_us=50209`，即等待响应头时超时。仅有 create 的 Actor 计时，没有失败 dispatch 的完成计时，根因仍不能定位到引擎。三份日志：`win10-key-count-rime-smoke.log`、`win10-key-count-rime-smoke-v2.log`、`win10-key-count-rime-trace.log`。真实 candidate API/IPC/pool 前段通过，后续 Win32 TIP/故障段未运行，整体失败。

VM 也观察到一次切回 Mo 后快速输入直接作为 ASCII 留在宿主；随后独立按键成功显示候选。该负向截图保留，未证明其原因，不将暖态功能成功升级为首次切换通过。一次误在 Microsoft 拼音下输入的尝试没有纳入 Mo 验收。另有测试日志根路径不可写导致的安装脚本预启动失败；改用证据目录后安装 exit 0。

尚未验证 Win10 x86 普通应用、WinUI/浏览器、鼠标真实交互、混合 DPI、多屏、忙预编辑中设置切换、loaded-TIP 升级/登录重启、首次切换 <500 ms、严格延迟或日常稳定。G2/G3 仍未整体通过。

## 前轮交接状态（后续已更新）

VM 当前装有原 0.0.10.0（未包含本轮按键修复），已恢复原安装树；前景输入法已切回 Microsoft 拼音，测试窗口关闭。用户默认设置文件由本轮测试创建并保留；未修改默认输入法 override。下一步先将 ADR 0054 修复重新打包并复测，继续定位响应超时，再补 Win10 普通宿主与登录/重启矩阵。

## 2026-10-03 0.0.11.0 后续交接

修复已进入新包，变更载荷升级、一次重启后安装状态及 x64/x86 记事本、x86 写字板精确回读有限通过。当前 VM 保留 0.0.11.0；新旧状态以 [0.0.11.0 续测证据](WIN10-PACKAGED-0110-EVIDENCE.md) 为准。主机失败 dispatch 已取得 Actor 完成计时（约 1.31/1.67 秒），直接探针缩到 native process_key；后续键也出现 252 ms。具体组件与 CPU/I/O 原因未定位，50 ms deadline 未放宽，G2/G3 不升级为整体通过。
