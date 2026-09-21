# ADR 0036：来源材料包与 Authenticode 内到外顺序

## 状态

已接受（技术材料闭环和签名顺序已固定；法律审查、实际签名与发行授权仍未完成）

## 背景

ADR 0035 已把安装 payload 唯一映射到 16 个组件，但 SPDX 中记录来源哈希并不等于随发行物
提供了许可证全文或 GPL 对应源。另一方面，直接给已经链接好的 MSI/Bundle 补签会把未签名的
PE 文件封装进去；在已封口 stage 中原地签名又会破坏 stage 清单。需要分别解决材料完整性和
签名构建顺序，并阻止“脚本运行成功”被误解为法律批准或发行授权。

## 决策

1. `release-materials-policy.json` 锁定九份原始源码归档：rime-ice、librime、librime-lua、
   LevelDB、MARISA Trie、OpenCC、yaml-cpp、Boost 1.84.0 和 Lua 5.4.9。每份归档必须与
   stage/runtime provenance 的 SHA-256 一致；rime-ice 是唯一显式标记的 GPL 对应源。
2. 同一策略锁定 15 份许可证或通知文档，覆盖 Mo、rime-ice、六个 librime 组成部分、
   Boost、Lua 以及 `windows-sys`/`windows-link` 的 Apache/MIT 双许可证文本。来源包括仓库、
   已验证 stage、运行时构建输入和 Cargo registry；任何缺失、漂移或额外输出都拒绝。
3. 材料 manifest 同时绑定 stage manifest、ADR 0035 compliance evidence 和策略哈希。
   `technical_materials_complete=true` 仅表示策略列出的字节全部存在；evidence 永久固定
   `legal_review_complete=false` 与 `release_authorized=false`，不能替代正式合规结论。
4. Authenticode 必须按内到外六步执行：签五个 PE；验证签名和 RFC 3161 时间戳；从新的
   已签名 stage 重建 MSI；签 MSI；用已签 MSI 与已签 registrar/finalizer 重建 Bundle；
   最后签 Bundle。禁止在已封口 stage 中原地修改、禁止只签外壳、禁止复用当前未签名链接物。
5. 当前脚本只读取 Authenticode 状态、记录基线哈希并生成顺序合同，不接触证书、不联网、
   不签名。只有 `ProductionShape` format-3 linked evidence 能进入计划，DevelopmentTest 或
   任意哈希脱钩都会被拒绝。

## 验证

- 真实材料包包含 9 份源码归档、15 份许可证/通知和两份 manifest/evidence；独立 verifier
  对全部文件复算大小与 SHA-256，并重新验证前置 SPDX evidence。
- 8 项材料测试覆盖真实生成/复核、过期 Boost pin、缺失 GPL 标记、许可证篡改、伪造发行
  授权和额外文件。
- 7 项签名计划测试覆盖五个内层 PE、MSI/Bundle 两个未签名基线、六步顺序、开发 flavor、
  脱钩 installer hash、计划篡改和额外输出。
- 全过程没有执行 MSI/Bundle，没有注册或启用输入法，没有签名、联网或改动系统输入状态。

## 后果

许可证全文和 GPL 对应源的“技术组装”不再是待办；仍须由法律/合规审查确认内容和 librime
组合结论。当前所有 PE、MSI 和 Bundle 仍未签名。下一阶段必须在受控签名环境按本 ADR 重建，
再进行 MSI ICE、安装树 ACL/reparse 检查以及一次性 Windows 11 VM 全生命周期矩阵。
