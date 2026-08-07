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
