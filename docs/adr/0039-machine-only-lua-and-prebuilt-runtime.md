# ADR 0039：安装态 Lua 与部署数据只来自机器目录

## 状态

已接受。源码策略、新运行时、最终 stage、真实 rime-ice golden 和未执行的
ProductionShape 链接包均已验证；真实安装仍等待隔离 VM。

## 背景

上游 `librime-lua` 会把用户 `lua` 目录置于模块搜索路径最前，并优先执行用户
`rime.lua`。librime 的 deployed resource resolver 又优先读取 `staging_dir`。
如果安装态把 staging 放在 `LocalAppData`，普通用户文件就能选择运行时代码，或
替换已经由 Mo 构建和验证的 schema/config。这与“安装即用、机器素材固定”的
发行边界不一致。

## 决策

1. release Broker 的 `staging_dir` 与 `prebuilt_data_dir` 都固定为
   `Program Files\Mo\data\rime-ice\build`；不再创建或读取用户 `build`。
2. `LocalAppData\Mo\Rime` 只保存用户词典、学习状态等可变数据。若已有
   `rime.lua` 或 `lua` 路径，Broker 在绑定 Pipe、加载运行时前拒绝启动。
3. 自构建 `librime-lua` 的 `package.path` 只包含机器 shared 目录下的
   `lua/?.lua` 与 `lua/?/init.lua`，`package.cpath` 置空，只执行机器
   shared 目录的 `rime.lua`。
4. 构建器在打补丁后解析源文件并 fail-closed；provenance 同时记录策略版本、
   补丁哈希及 12 份 Mo 输入。stage 验证再次核对完整清单和哈希绑定。
5. 这是一条来源边界，不是 Lua 沙箱。进入签名机器素材的 Lua 仍是受信任的
   可执行代码，必须接受来源、审查、签名和更新认证。

## 验证证据

- 新增 5 项 Lua 源码策略测试，并保留构建器原有 5 项 fail-closed、4 项路径和
  7 项严格 native-source 检查。
- 新运行时 `rime.dll` SHA-256 为
  `0B6581269DC58A179A0FADAD0E2A0BFADA057354A7A5411C83DB356A45CF5729`。
- 真实探针在用户目录放入会写哨兵的 `rime.lua` 和会报错的同名 Lua 模块；
  新运行时仍完成中文、Emoji、英文、日期、Unicode、数字、计算器 7 组
  exactly-once golden，用户 `build` 保持为空且哨兵不存在。受控旧运行时会执行
  哨兵并在日期用例失败，证明 canary 能检测旧行为。
- 最终 73-source/137-file stage manifest SHA-256 为
  `25F9D8C1098B0B34C72710E7D441BF6F8ED2194A8A035C1C62292399FE66C3E1`；
  82 项 staging 策略、双架构各 10 轮真实素材回归及 40 次明确 Broker 退出通过。
- 未执行的 `0.0.5.0` ProductionShape 已完成反向核验；MSI SHA-256 为
  `2DC3192ED6866A050B4F4DBCE661FC09D12CFA8E3B5B1B9624346D61A46D1297`，
  Bundle SHA-256 为
  `DAEBC866357D34F2FD03FB163D839C46F10CD989B3637DBF67B10E997CFA018E`。

## 后果与剩余边界

用户不能再通过配置部署目录或 Lua 搜索路径改变安装态运行代码和 schema；用户词典
与学习状态仍保留在用户目录。现有高级 Rime 用户若在 Mo 用户目录放置 Lua，启动会
明确失败，而不是静默执行。

产物仍未签名、不可分发，也没有执行 MSI/Bundle。隔离 VM 中的安装、修复、升级、
回滚、卸载，设置生成器、真实宿主组合、签名与更新认证仍是后续发行门。
