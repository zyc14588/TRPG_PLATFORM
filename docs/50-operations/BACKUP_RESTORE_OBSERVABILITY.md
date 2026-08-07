---
document_id: SPEC-OPS-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 备份、恢复演练、可观测性与保留

<a id="SPEC-OPS-BACKUP"></a>
## 1. 完整备份集合

包含 PostgreSQL 一致性备份、对象清单、包/素材/恢复点、签名与认证、创作者协议、加密凭据库、配置快照和完整性哈希。明文密钥不进入备份；恢复主密钥独立保存。

<a id="SPEC-OPS-RETENTION"></a>
## 2. 保留

提供每日、每周、每月和升级前分层保留。安全事件、许可协议、内容争议和司法要求可设置独立保留锁，普通管理员和自动轮转不能删除。

<a id="SPEC-OPS-ENCRYPTION"></a>
## 3. 加密

任何可搬离主机的备份在传输前加密，并具有独立完整性校验。恢复密钥不嵌入备份或与其存放在同一位置。

<a id="SPEC-OPS-DRILL"></a>
## 4. 恢复演练

定期在隔离数据库、对象存储和精确发行版本中恢复，验证对象哈希、依赖、事件重放、Lua 检查点、权限、凭据库和最小一致性游戏。只检查“文件存在”不构成已验证备份。

<a id="SPEC-OPS-OBSERVABILITY"></a>
## 5. 可观测性

结构化日志、指标和追踪统一使用 request/correlation/command/session/execution/task/package/state/incident ID。自托管默认本地保存，可选标准化导出；提示词、聊天和私密状态默认不外发。

<a id="SPEC-OPS-LOG-CLASSIFICATION"></a>
## 6. 分类

运行日志、安全审计、玩家日志、Lua 回调审计和取证证据分别权限、脱敏和保留。任何类别不得记录原始密码、BYOK、令牌、签名私钥或加密主密钥。

<a id="SPEC-OPS-SLA"></a>
## 7. 恢复声明

V1 不承诺零停机、自动故障转移或商业 SLA。只公开可测量的最近备份、恢复演练、备份完整性和实测恢复时长。
