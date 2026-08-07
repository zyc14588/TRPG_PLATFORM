<!-- SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 -->
---
document_id: LEGAL-LICENSE-BOUNDARY-001
status: ACTIVE
---

# 新旧许可边界

## 旧项目冻结

- Repository: `github.com/zyc14588/TRPG_PLATFORM`
- Legacy default branch: `master`
- Legacy frozen commit: `73bcee9500720f0d26d2b6f9676d4a2e6f30d964`
- Legacy tree hash: `6e4e7e7100a977f97d1dbb4be4564450081767a9`
- Signed annotated tag: `legacy/pre-r0-restart-20260807`
- Signature type: `SSH`
- Signer fingerprint: `SHA256:zR55HATrI3+mzVZW3uc8MsCxE+g5fyImXdD8X7WKkt4`
- Legacy repository-wide license finding: no repository-level `LICENSE`, `COPYING`, or `NOTICE` file was found; Rust package manifests declared `license = "UNLICENSED"`; no license grant is inferred and default rights remain reserved.
- Freeze evidence artifact: `runtime-output/m0-freeze/SHA256SUMS`, manifest hash `0b4aecfa8558af40e5cf121ccd74394759a8e6ca5011e9cc3ca78772a33191d5`.

旧 Tag 与此前历史不因新主线 LICENSE 而追溯重许可。

## 新主线

- Restart branch: `restart/v1-baseline`
- First new baseline commit: the signed commit that first adds this file and root `LICENSE`; its immutable SHA is pinned in the immediately following governance commit and the external M0 implementation evidence.
- Active branch after normalization: `main`
- Program code license: `PolyForm-Noncommercial-1.0.0`
- Rights holder: `Yucheng Zhao`

## 零迁移声明

新活动树没有从旧项目复制、移动、Cherry-pick 或改名迁入源代码、测试、迁移、设计、Codex 提示词和素材。旧项目只通过签名 Tag 与 Git 历史供审计参考。
