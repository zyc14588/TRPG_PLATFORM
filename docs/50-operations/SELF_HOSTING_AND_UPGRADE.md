---
document_id: SPEC-OPS-SELFHOST-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 自托管、Compose、秘密与升级

<a id="SPEC-OPS-COMPOSE"></a>
## 1. Compose

正式基线为 Linux amd64 单主机 Docker Compose，分别运行反向代理、platformd、workerd、lua-runner、PostgreSQL 和对象存储。禁止 replicas 和多权威实例。Windows 通过 WSL2/Docker Desktop 文档路径。

<a id="SPEC-OPS-NETWORK"></a>
## 2. 网络

只有反向代理外部可达。PostgreSQL、lua-runner、workerd 和对象存储管理端口不向公网开放。可信代理清单外的转发头忽略。

<a id="SPEC-OPS-SECRETS"></a>
## 3. 秘密

生产通过环境变量引用只读秘密文件。示例配置只包含占位和路径，不包含真实秘密。加密主密钥、签名私钥和备份恢复材料分别保管。

<a id="SPEC-OPS-MIGRATION"></a>
## 4. 数据库迁移

进入维护模式、创建 Session 恢复点、取得迁移锁、预检当前/目标版本、完成一致性备份后执行。失败时保持不可写并恢复旧一致状态。

<a id="SPEC-OPS-RELEASE"></a>
## 5. 发行升级

验证签名 Release Manifest 与镜像摘要 → 主机/磁盘/数据库/对象预检 → 备份 → 维护 → 迁移 → 启动精确产物 → 健康、最小游戏、事件重放与恢复验收。任何失败阻止启用。

<a id="SPEC-OPS-OFFLINE"></a>
## 6. 离线

首次准备完成后，自托管可在无互联网环境使用本地账户、私人房间、局域网同步、本地模型、Campaign、存档和包导入。云模型、外部邮件和远程下载自然不可用。
