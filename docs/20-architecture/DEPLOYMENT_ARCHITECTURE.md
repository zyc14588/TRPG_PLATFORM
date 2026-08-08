---
document_id: SPEC-DEPLOYMENT-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 托管、自托管与单节点部署架构

<a id="SPEC-DEPLOYMENT-MODES"></a>
## 1. 模式

同一核心代码支持官方托管和自托管。数据属于当前部署；自托管不依赖中央账户、许可证服务器或控制平面。

<a id="SPEC-DEPLOYMENT-SINGLE-HOST"></a>
## 2. 单主机

正式 Compose 分别运行反向代理、`platformd`、`workerd`、`lua-runner`、PostgreSQL 和对象存储，全部固定同一主机。禁止水平复制和多权威实例。

<a id="SPEC-DEPLOYMENT-SELFHOST"></a>
## 3. 自托管

Linux amd64 + Docker Compose 是基线；Windows 通过 WSL2/Docker Desktop。准备好包、素材和本地模型后可纯局域网离线运行账户、房间、游戏、存档和 Studio 本地工作流。

<a id="SPEC-DEPLOYMENT-TLS"></a>
## 4. TLS 与网络

外部反向代理负责 TLS 和 WebSocket 转发。PostgreSQL、lua-runner、workerd 和对象存储管理接口不向公网暴露。只接受可信代理转发头。

<a id="SPEC-DEPLOYMENT-SECRETS"></a>
## 5. 秘密

环境变量引用只读秘密文件。数据库密码、Cookie 密钥、签名私钥、凭据主密钥和备份恢复材料不得写仓库、普通配置、命令行或日志。

<a id="SPEC-DEPLOYMENT-MAINTENANCE"></a>
## 6. 维护

V1 不承诺零停机。升级进入维护模式、停止新 Session、创建恢复点、执行迁移与自动验收，失败恢复旧一致状态。
