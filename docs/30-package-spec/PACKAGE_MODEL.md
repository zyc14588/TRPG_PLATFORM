---
document_id: SPEC-PACKAGE-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 游戏包模型、身份、清单与安装生命周期

<a id="SPEC-PACKAGE-TYPES"></a>
## 1. 包类型

- `game-system`：席位、状态、命令、事件、规则、AI 契约和基础 UI；
- `content`：模组、Campaign、场景、角色、卡牌、地图配置与教程；
- `assets`：图片、音频、地图、本地化和来源元数据；
- `ui-extension`：受限自定义面板；
- `library`：共享 Lua 模块、Schema 和通用 UI，不可独立启动 Session；
- `bundle`：分发容器，不是运行时包类型。

<a id="SPEC-PACKAGE-IDENTITY"></a>
## 2. 身份

`package_id` 不可变，格式为发布者命名空间/稳定短名。显示名称、维护者和版本可变化。发布物由 `package_id + semantic_version + content_hash + build_provenance` 标识。

<a id="SPEC-PACKAGE-MANIFEST"></a>
## 3. 清单

版本化 TOML 清单声明包类型、版本、Lua Profile、Host API 范围、入口、能力、依赖、Feature、Schema、权利和构建信息。复杂结构使用 JSON Schema。

<a id="SPEC-PACKAGE-CAPABILITIES"></a>
## 4. 能力

实际能力 = 包声明 ∩ 信任等级允许 ∩ 当前执行上下文允许。必需能力缺失时拒绝安装/启动；可选能力必须有声明且测试过的替代路径。能力不能运行中动态升级。

<a id="SPEC-PACKAGE-DEPENDENCIES"></a>
## 5. 依赖

源清单可用兼容范围；构建产生精确版本、内容哈希、Feature 和传递依赖锁。部署可安装同包多个版本，但单 Session 依赖图只能有一个精确版本。循环依赖拒绝。

<a id="SPEC-PACKAGE-INSTALL"></a>
## 6. 导入安装

隔离暂存 → 路径/压缩安全 → 清单/Schema/Lua Profile → 依赖解析 → 签名/权利 → 生产 Profile 测试 → 数据迁移预检 → 原子工作区安装。

上传不等于安装。物理对象、工作区安装、房间选择和 Session 锁分层。

<a id="SPEC-PACKAGE-STORAGE"></a>
## 7. 不可变对象

包按内容哈希保存，拒绝路径穿越、绝对路径、链接逃逸、设备文件、压缩炸弹、规范化冲突、Bytecode 和未声明二进制。相同哈希可物理去重但不共享权限。

<a id="SPEC-PACKAGE-UNINSTALL"></a>
## 8. 停用与删除

有 Campaign、Session、恢复点、依赖、认证、审计、协议或法律保留引用时只能停用。安全撤销包必须保留为证据。

<a id="SPEC-PACKAGE-EXTENSIONS-M1-B010"></a>
## 9. M1-B010 通用 namespaced JSON extension 合同

### 9.1 版本、声明与兼容

现有 `schema_version = 1` TOML manifest 保持严格封闭，解析、验证和无 extension 行为不得改变。extension 只在新的 package manifest `schema_version = 2` 中声明；Bundle 不携带 extension descriptor。v1 导入后显示零 extension，确定性 repack 保留原 manifest 与内容字节，不得把 v2 字段偷偷写入 v1。

v2 使用重复的 `[[extensions]]` 表；每项只允许下列字段，未知 descriptor 字段 fail closed：

| 字段 | 合同 |
|---|---|
| `namespace` | 唯一 ASCII namespace；语法 `^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?){2,7}$`，总长不超过 128 bytes。`trpg.platform` 及其子 namespace 对所有包一律保留并拒绝，不存在官方例外。 |
| `required` | boolean；不支持的 required extension 返回 `ERR_EXTENSION_REQUIRED_UNSUPPORTED`，不得降级加载。 |
| `contract_version` | 正整数；M1-B010 只支持 `1`。未知版本仅在 optional 时以只读方式保留。 |
| `schema_path` | package-local JSON Schema 路径，必须位于 `extensions/<namespace>/` 下并以 `.schema.json` 结尾。 |
| `schema_sha256` | `sha256:` 加 64 个小写十六进制字符；绑定 schema 文件原始 bytes，不绑定重新序列化结果。 |
| `payload_path` | package-local JSON document 路径，必须位于同一 namespace 目录并以 `.json` 结尾，且不得等于 `schema_path`。只允许引用文件，不允许 inline payload。 |
| `host_api_major` | 非负整数；复用 package Host API major 语义。 |
| `host_api_min_minor` | 非负整数；支持区间下界。 |
| `host_api_max_minor` | 非负整数且不小于下界；支持区间上界。 |

第一方、受信包、私人包和第三方包使用同一 parser、model、validator、Creator service 和 exporter。平台不得按 package ID、publisher、官方身份或 rules-residue 身份分支。rules-residue 只能在仓外从其稳定 package ID 派生 namespace 并消费本合同。

### 9.2 路径、Schema 与支持状态

所有 archive/project 路径必须是 UTF-8、NFC、slash 分隔的 portable relative path，最长 240 bytes；拒绝空段、`.`、`..`、absolute/drive/UNC/device path、反斜杠、控制字符、NUL、大小写折叠或 Unicode 规范化碰撞、symlink 和其他 special file。`extensions/` 下每个 regular entry 必须恰好由一个 descriptor 声明为 schema 或 payload；undeclared、重复占用和跨 namespace 引用全部拒绝。extension 路径只能承载 JSON，不能声明 Lua、Wasm、native binary、模板执行或 Host path。

`contract_version = 1` 的 schema 必须是有效 UTF-8 JSON、声明 Draft 2020-12、原始 bytes digest 匹配，且 `$ref` 只允许同一 schema 内的 `#` fragment。任何 URI、scheme、network、file、absolute 或跨文件 `$ref` 均拒绝；验证器不得安装 network loader、执行代码或读取 archive/project 根之外的路径。schema dialect/host range 不支持或 optional schema 不可用时，Host 和 Creator 只读保存 descriptor 与现有引用 bytes，不解释、不删除；相同情况若 `required = true` 则返回 typed failure。存在但 digest 不匹配的 schema 在 required/optional 两种情况下都拒绝。

### 9.3 固定安全上限

| 对象 | 上限 |
|---|---|
| 每个 package 的 extensions | 64 |
| manifest | 1 MiB |
| 每个 schema | 1 MiB |
| 每个 payload | 4 MiB |
| JSON nesting depth | 64 |
| JSON number token / exponent | 128 bytes / absolute exponent 308 |
| archive entries | 4096 |
| 单一 archive entry expanded bytes | 8 MiB |
| archive expanded total | 64 MiB |
| imported compressed ratio | 100:1 |

任何 limit 在分配或解压前检查并 fail closed。JSON 还必须拒绝 BOM、invalid UTF-8、重复 key（包括转义后同名）、trailing value 和无效 surrogate；数字不得经 `float64` 往返丢失。

### 9.4 canonical model、serialization 与 content hash

Host canonical model 保存有序、唯一的 typed descriptor；supported payload 保存为严格 JSON value，unsupported optional 保存 descriptor 和引用文件原始 bytes。canonical JSON 按 UTF-8 byte order 排序 object keys，无无意义空白，使用最小 JSON escaping，数组顺序不变；十进制数精确展开 exponent、去除非必要前后零并把负零写成 `0`。canonical TOML 使用固定 core-field/table/extension 顺序、LF 与 UTF-8。任何相同 canonical model 的二次序列化必须 byte-identical。

content hash 的字节域固定为 project 中除 `META-INF/platform.lock.json` 与 `META-INF/artifact.json` 外的所有 canonical regular entries；路径排序后逐项输入 `u32be(path_byte_length) || path_utf8 || u64be(content_byte_length) || content_bytes`，最后计算 SHA-256 并写作 `sha256:<lowerhex>`。两个 `META-INF` 文件只能在 content hash 得出后由平台生成，作为外层 lock/identity envelope；它们和输出 archive 均不得进入 content hash，因而没有自引用。已有普通内容路径（包括 rules-residue 自身的 `package.lock.json`）仍属于 content set。

确定性 archive 只使用 ZIP `Store`，按 canonical path 排序，不写 directory entry，固定 regular mode `0644`、时间 `1980-01-01T00:00:00Z`，清空 extra/comment。archive SHA-256 是对最终 ZIP bytes 的外部 identity；同一输入、lock 与工具 identity 的两次 build/repack 必须 byte-identical。导入可接受 Store/Deflate，但必须执行上述 ratio、size、path、duplicate 和 symlink 检查。

### 9.5 Host 与 Creator 边界

Host load、export、repack、reload 共享同一 canonical model。supported extension 的 parse/load/export/reload 必须语义相等；unsupported optional 的 raw schema/payload bytes 必须无损；required unsupported 在 Host/Creator 均 typed fail。extension payload 永远与 core manifest model 隔离，不能覆盖 `package_id`、rights、capabilities、dependencies、entrypoint、Host API 或其他 core field。

Creator 最低形态固定为 `SCHEMA_VALIDATED_GENERIC_JSON_EDITOR`：真实 Creator Studio binary 与 Wails UI 绑定同一个窄 service，支持 archive import、descriptor inspect、JSON textarea/value edit、declared-schema validate、export 和 re-import。自动验收可调用同一 binary 的 headless interface，但不得用外部编辑器、直接改 ZIP、只读 opaque view 或仅 export 冒充 edit。每次 edit 以输入 archive SHA-256 作为 conflict token；token 不匹配返回 `ERR_CREATOR_CONFLICT`，成功 overwrite 使用同目录临时文件、fsync 和 atomic rename。M1-B010 不实现完整项目管理、preview、语义 diff UI、装备表单或游戏专用 UI。

exact Creator identity 由 `go build -trimpath` 的 binary SHA-256、嵌入的 platform commit/tree、版本和 build command 共同组成；跨仓库证据必须保存这四项及 stdout/stderr/exit code。没有真实 binary 执行的 edit → validate → export → re-import 证据时，不得报告 PASS。

### 9.6 验收矩阵

正向用例必须逐项独立断言：

1. v1 / 无 extension 旧包行为不变；
2. 一个第三方 namespace；
3. 多个第三方 namespace；
4. required + supported；
5. optional + unknown 的只读无损保留；
6. parse / load / export / reload 语义一致；
7. canonical serialization 确定；
8. package build/repack byte-deterministic；
9. Creator import/export；
10. Creator JSON edit/schema validate/export/re-import；
11. raw schema digest 验证；
12. 第一方与第三方走同一代码路径。

负向用例必须逐项拒绝或产生规定 typed failure：

1. undeclared payload；
2. duplicate namespace；
3. invalid namespace；
4. `trpg.platform.*` reserved spoof；
5. schema digest mismatch；
6. required missing schema；
7. path traversal；
8. absolute path；
9. symlink escape；
10. remote/cross-file `$ref`；
11. required unsupported；
12. core field/path override；
13. payload too large；
14. nesting too deep；
15. nondeterministic serialization/build；
16. Creator field loss；
17. Host field loss；
18. extension executable-code path/content；
19. schema/extension Host path access；
20. package-ID、official 或 rules-residue special case。

若 INT-010 需要正式 B002 equipment schema，而 B002 又等待 INT-010，则停止为 `REPLAN_REQUIRED_CIRCULAR_DEPENDENCY`；合法证明只使用明确标注为非正式语义的 minimal probe schema。
