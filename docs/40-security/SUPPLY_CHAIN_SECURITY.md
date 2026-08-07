---
document_id: SPEC-SUPPLY-CHAIN-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 依赖、许可、构建与包供应链安全

<a id="SPEC-SUPPLY-CHAIN-DEPENDENCY"></a>
## 1. 依赖准入

所有 Go、Node、Wails、Actions、容器、Lua 模块和工具依赖在合并前完成版本、来源、许可证、维护性和漏洞审查。强 Copyleft 与其他源码可用依赖默认禁止核心发行物，例外需书面审查。

<a id="SPEC-SUPPLY-CHAIN-LOCK"></a>
## 2. 精确锁定

禁止 `latest`、`main` 和运行时浮动解析。锁定工具版本、依赖版本、Action Commit SHA、容器 digest、包内容哈希和 Feature。

<a id="SPEC-SUPPLY-CHAIN-SBOM"></a>
## 3. SBOM 与来源

正式发行包含 SBOM、许可清单、源码/镜像/前端/官方包哈希、构建工具版本、签名 Tag 和来源证明。相同候选只按原哈希晋级。

<a id="SPEC-SUPPLY-CHAIN-PACKAGE"></a>
## 4. 游戏包

包经过内容寻址、解包安全、清单/Schema、签名、权利、依赖和生产 Profile 测试后原子安装。安全撤销影响根包与传递依赖。

<a id="SPEC-SUPPLY-CHAIN-EXCEPTIONS"></a>
## 5. 例外

安全和许可扫描例外必须有所有者、范围、理由、补偿措施和到期时间；不得永久忽略或仅为通过 CI 删除检查。
