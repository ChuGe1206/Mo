# ADR 0028：隔离 TSF 探针与组合终止来源

- 状态：已采用（受控未注册探针）；产品宿主边界仍需真实注册验证
- 日期：2026-09-19

## 问题

未注册的候选生命周期探针直接调用 Mo 的 key sink，但文档与 ACP 文本存储来自一个用
`ITfThreadMgr::Activate` 启动的真实 TSF Thread Manager。这个调用同时允许当前系统文本服务进入该线程。
探针窗口虽然没有取得 active/foreground，偶发的 `OnCompositionTerminated` 仍会在一次成功按键之后异步到达，
清除刚建立的 Mo 组合和候选窗。重复运行通过不能说明它已消失。

## 证据

development-only trace 增加了两类不含文本、按键、token 和绝对地址的证据：

- `ITfThreadMgrEventSink` / `ITfThreadFocusSink` 的 32 项有界事件环，以及当前 document focus 分类；
- 组合终止时最多 24 个模块编号和模块相对 RVA。调用方结构变化后使用新的诊断 IID，旧 IID 必须返回
  `E_NOINTERFACE`，默认构建仍不暴露诊断接口。

`build/mo-candidate-origin-trace-build-1.log` 中 x64 20/20 完成；Win32 前 5 轮完成，第 6 轮在第二次初始组合
后、故障注入前失败。Broker 请求 13 成功（3.188 ms），dispatch 3.646 ms；窗口 owner 不是 active/foreground，
也不在同步 SendMessage 中。失败时 `GetFocus` 仍返回预期文档，事件环只有该文档的 init/push，没有 focus 切换
或 thread-focus 丢失。终止栈为 24 帧，经消息泵回到 `msctf.dll`；显式 owner termination 对照为 15/20 帧。

第二次 20+20 轮在补齐模块分类后通过，明确显示正常显式终止经过 `textinputframework.dll` 的
`CComposition::_SendOnTerminated`、`CComposition::_Terminate`、
`CInputContext::_TerminateCompositionWithLock`、`CInputContext::OnLockGranted`、
`CACPWrap::OnLockGranted` 和 `CInputContext::_QueueItem`。微软公开 PDB 只用于离线名称解析，CodeView
GUID+age、PDB SHA-256 和解析器输出均保留在忽略的 build 证据目录；它不进入产品或 stage。第三次旧模型
压力在 x64 100/100、Win32 33/100 后主动停止，因为隔离修复已经落地；这份不完整运行不计为通过矩阵。

隔离实现随后在 `build/mo-candidate-isolated-trace-final.log` 完成 x64/Win32 各 100 轮：200 个 host
都报告 `no_other_tip=1`、`created_foreground=0`、`created_active=0`；终止计数 1/2 各 200 次，
400 次写回重入均 fail-open，200 个完整故障 rounds / 400 次明确 Broker 退出通过，没有额外宿主终止。
诊断结构最终滚动 IID 后，当前源码又在 `build/mo-candidate-isolated-v4-final.log` 完成双架构各 20 轮。
旧 v3 trace probe 与最终双架构 TIP 配对时都在读取 caller-owned buffer 前得到 `E_NOINTERFACE`，见
`build/mo-candidate-retired-v3-abi.log`。
这些仍是受控宿主证据，不是注册应用验收。

这些证据说明受控探针的偶发终止不是 Broker 超时、候选 owner 激活或 TSF document focus 丢失；它来自真实
TSF/TextInputFramework 的异步路径。旧探针同时允许系统文本服务和直接激活的 Mo 操作同一测试 context，
因此不能把该失败归因于产品中的单一活动输入法。

## 决策

未注册且非激活的受控模型改用 `ITfThreadMgrEx::ActivateEx`，同时指定：

- `TF_TMAE_NOACTIVATETIP`；
- `TF_TMAE_NOACTIVATEKEYBOARDLAYOUT`。

探针读取 `GetActiveFlags` 并要求 `TF_TMF_NOACTIVATETIP`，同时继续要求窗口不取得 active/foreground。微软文档
说明文本服务会在调用线程异步取得焦点时激活，所以这两个约束缺一不可。`--activating-test-host` 和注册路由
继续使用普通 `Activate`，作为有意包含系统路由的不同模型，不能混入隔离结果。

trace 还对匹配的组合终止通知计数。每轮只有两次显式 owner termination，分别必须得到 1、2；任何更早或额外
通知立即失败。生产 TIP 的宿主终止清理、写回重入保护和 50 ms 传输截止时间没有放宽或绕过。

## 辅助工具边界

`MoStackSymbolResolver.vcxproj` 是 development-only 离线 DIA 工具：必须显式传入 image、从该 image 的
CodeView GUID+age 取得的 PDB、`msdia140.dll` 和 RVA。它不访问符号服务器、不加载目标 image、不输出绝对地址，
也不进入部署 stage。下载微软公开符号是一次显式诊断准备步骤，不是构建或测试的隐式网络依赖。

## 未关闭项

本决策关闭“未注册受控探针被当前系统文本服务污染”的根因，不证明所有真实宿主终止都来自同一来源。
注册后的 Notepad/WinUI/AppContainer 仍应把宿主终止当合法事件处理，并分别验证真实 profile 路由、焦点切换、
窗口销毁和 Broker 故障恢复。隔离压力通过也不能替代这些 G3 验收。

API 依据：[ITfThreadMgrEx::ActivateEx](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfthreadmgrex-activateex)、
[ITfThreadMgrEx::GetActiveFlags](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfthreadmgrex-getactiveflags)、
[ITfThreadMgrEventSink::OnSetFocus](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfthreadmgreventsink-onsetfocus)、
[CaptureStackBackTrace](https://learn.microsoft.com/en-us/windows/win32/debug/capturestackbacktrace)、
[Microsoft Symbol Store](https://learn.microsoft.com/en-us/windows/win32/debug/using-symsrv)。
