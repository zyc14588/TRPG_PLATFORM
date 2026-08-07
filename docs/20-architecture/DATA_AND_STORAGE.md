---
document_id: SPEC-DATA-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 数据、事件、对象存储与租户隔离

<a id="SPEC-DATA-POSTGRES"></a>
## 1. PostgreSQL

PostgreSQL 是权威数据库。账户、工作区、权限、包索引和关系数据使用显式关系模型；事件表追加写入；快照独立；JSONB 只用于绑定 Schema 的游戏状态边界。Go 使用显式 SQL，不用重型 ORM隐藏事务。

<a id="SPEC-DATA-EVENT"></a>
## 2. 事件与快照

事件原始负载和模式版本不可改写。新代码使用兼容投影或确定性 Upcaster 读取旧事件。快照、Lua 检查点和派生投影可迁移和重建。

<a id="SPEC-DATA-NAMESPACE"></a>
## 3. 包数据

私人包使用版本化文档/键值命名空间、JSON Schema 和声明式索引。官方/受信包可安装受审查关系表和命名操作。任何影响胜负、资源、角色和秘密的数据写入必须关联权威事件。

<a id="SPEC-DATA-OBJECT"></a>
## 4. 对象存储

包、素材、导出和证据使用内容寻址对象存储抽象。开发/简单自托管可用本地目录，托管可用 S3-compatible。数据库保存元数据、权限和哈希；对象键不授予权限。

<a id="SPEC-DATA-TENANT"></a>
## 5. 工作区隔离

所有对象有明确工作区归属。相同哈希可物理去重，但安装、许可、可见性、保留和权限独立。查询必须默认带租户边界。

<a id="SPEC-DATA-EXPORT"></a>
## 6. 导入导出

支持工作区、包、Campaign、Session、角色、事件、快照、恢复点和必要配置的版本化校验迁移。原始密钥、服务端主密钥和会话令牌不得导出。
