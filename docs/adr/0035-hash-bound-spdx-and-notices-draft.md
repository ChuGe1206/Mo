# ADR 0035：哈希绑定的逐文件 SPDX 与通知草案

## 状态

已接受（开发阶段供应链清单闭环；法律审查和发行授权仍未完成）

## 背景

已验证 stage 能证明 131 个安装文件的字节和构建来源，但不能回答每个最终文件属于哪个
许可证组件、组合运行时包含哪些静态依赖，或 GPL 衍生数据是否已列入对应源义务。手工维护
一份不绑定 stage 的通知文本会随 payload 漂移，也容易把“有清单”误解为“已获发行批准”。

## 决策

1. `release-compliance-policy.json` 是 Phase 0 的显式审查输入。它锁定 Cargo.lock、
   rime-ice commit/source archive、librime 允许列表六个 Git archive、Boost/Lua archive、
   16 个包的 SPDX license 表达式与依赖边，以及六条互斥文件归属规则。
2. 每个 stage payload 文件必须匹配且只匹配一条规则。当前归属为：Mo Broker 1、Mo 原生
   文件 3、librime 组合 DLL 1、rime-ice 源/编译数据 93、OpenCC 数据 30、由 rime-ice
   派生的 OpenCC 数据 3。未知、重复或零命中的归属规则全部 fail closed。
3. 运行时插件列表必须精确为 `lua`，来源 commit/archive 必须与 stage provenance 一致；
   `octagram` 不得进入允许列表。Rust Broker 的 `windows-sys`/`windows-link` crate 版本和
   crate checksum 也进入包级清单。
4. 生成器输出 SPDX 2.3 JSON、`THIRD_PARTY_NOTICES.draft.txt` 和独立 evidence。SPDX 包含
   131 个文件的 SHA-1/SHA-256、六个文件组件包的标准 package verification code、包/文件
   CONTAINS 关系和静态依赖 DEPENDS_ON 关系；所有时间与排序来自固定 policy/stage，因此
   相同输入逐字节可重复。来源 archive hash 只写作 source provenance，不冒充 payload hash。
5. verifier 从 stage 和 policy 重建期望文档并逐字比较，同时核对三文件精确 inventory、
   evidence hash、组件计数和 release blocker。额外文件、篡改 SPDX/通知或把
   `release_authorized` 改成 true 都被拒绝。
6. librime 组合 DLL 的 `license_concluded` 保持 `NOASSERTION`，直到正式法律审查确认静态
   组合边界。evidence 固定 `development_only=true`、`release_authorized=false`、
   `legal_review_complete=false`，不能由生成清单自动升级。

## 验证

- 实际 137-file production-shape stage 中的 131 个 payload 文件全部完成唯一归属；SPDX
  含 17 个 package（含顶层 payload）、131 个 file 和完整关系图。
- 两次独立生成的 SPDX、通知和 evidence SHA-256 逐字节一致；独立 verifier 通过。
- 12 项测试覆盖真实 stage、确定性、精确组件计数，以及漏归属、重叠归属、过期
  Cargo.lock、来源归档脱钩、SPDX 篡改、通知篡改、伪造发行授权和额外输出拒绝。
- 全过程不执行 MSI/Bundle，不注册或启用输入法，不签名、不联网，也不宣称法律批准。

## 限制

通知文件仍是草案，没有携带每个许可证全文；GPL 对应源包、修改记录和书面提供方式尚未
组装。librime 组合结论仍待律师/合规人员确认，签名、时间戳、MSI ICE、安装目录 ACL 与
一次性 Windows 11 VM 生命周期也未通过。因此这项工作关闭的是“可审计清单生成”而不是
G3/G4 的最终发行门。
