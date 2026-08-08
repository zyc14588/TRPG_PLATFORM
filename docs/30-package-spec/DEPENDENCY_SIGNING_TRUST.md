---
document_id: SPEC-PACKAGE-TRUST-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 包依赖、签名、信任与安全撤销

<a id="SPEC-PACKAGE-TRUST-AXES"></a>
## 1. 三条状态轴

完成度：DRAFT/IMPORTABLE/PLAYABLE/CERTIFIED。  
来源：UNSIGNED/PUBLISHER_SIGNED/TRUSTED_SIGNED/OFFICIAL。  
安全：ACTIVE/DISABLED/QUARANTINED/REVOKED。  

安全状态优先；`CERTIFIED + OFFICIAL + REVOKED` 仍禁止运行。

<a id="SPEC-PACKAGE-TRUST-SIGNATURES"></a>
## 2. 双签名

发布者签名证明来源与内容完整性。平台认证签名证明精确哈希、依赖锁、Lua Profile、Host API 和测试套件通过指定验证。二者不能互相替代。

<a id="SPEC-PACKAGE-TRUST-KEYS"></a>
## 3. 密钥生命周期

发布者密钥支持 ACTIVE、RETIRED、REVOKED、COMPROMISED、EXPIRED。正常轮换不影响历史签名；安全撤销声明受影响时间和发布物范围并触发强制终止。

<a id="SPEC-PACKAGE-TRUST-UNSIGNED"></a>
## 4. 未签名包

只允许 Studio/开发/CI 和明确风险提示的私人房间。所有参与者确认精确依赖图；内容或哈希变化后重新确认。未签名依赖使整个图显示风险，不能进入官方发行。

<a id="SPEC-PACKAGE-TRUST-PROMOTION"></a>
## 5. 信任提升

发布者身份与签名 → 权利/依赖声明 → 自动结构、安全、恢复和 AI 测试 → 必要人工审核 → 平台认证签名。清单中的自报 trust、下载量和房主勾选无效。

<a id="SPEC-PACKAGE-TRUST-REVOCATION"></a>
## 6. 安全撤销

严重沙箱逃逸、数据库越权、密钥泄露、系统性秘密泄露、签名不一致或恶意供应链触发：

1. 标记发布物及传递依赖 REVOKED；
2. 阻止安装和新 Session；
3. 立即停止受影响活跃 Session；
4. 回滚未提交事务；
5. 冻结最后已提交状态；
6. 使用平台可信事件标记 `SECURITY_TERMINATED`；
7. 保全包、签名、事件、审计和任务证据；
8. 审计完成前禁止恢复。

原 Session 永久保持安全终止，继续游戏需建立后继 Session。
