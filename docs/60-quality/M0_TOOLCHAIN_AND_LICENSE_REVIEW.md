---
document_id: RECORD-M0-TOOLCHAIN-001
authority: implementation-record
status: ACTIVE
language: zh-Hans
baseline: M0
---

# M0 工具链、稳定性与许可审查

## 锁定结果

M0 使用 `tools/toolchain.lock.json` 作为精确机器锁。Go、Node、pnpm、Just、Wails、React、Vite、TypeScript、Vitest 与容器基础镜像不得以 `latest`、`main` 或无版本范围引用。

## Wails 审查

- 选择：`v2.13.0`，官方 Release 标记为非 draft、非 prerelease，发布日期为 2026-07-06。
- 稳定性：Wails v2 是官方稳定线；v3 在本次审查时仍为 alpha，因此不进入 M0。
- 许可：`v2.13.0` 标签下官方 `LICENSE` 为 MIT License，可用于本项目的 Wails 空壳依赖；第三方告知义务仍由依赖报告追踪。
- 范围：M0 只创建窗口与静态基线页面，不绑定 Creator API，不实现编辑、导入、构建或分发功能。

## Just 审查

- 选择：`1.58.0`，官方 Release 标记为非 draft、非 prerelease，发布日期为 2026-08-03。
- 许可：官方 `LICENSE` 为 CC0 1.0 Universal。
- 范围：Justfile 只允许调用 `projectctl`，不承载重复工程判断。

## 前端审查

React、React DOM、Vite、Vite React 插件、Vitest 与类型包为 MIT；TypeScript 为 Apache-2.0。依赖全部使用精确版本，`pnpm-lock.yaml` 保存完整 registry integrity。M0 页面不加载远程字体、脚本、图像或分析服务。

## 容器审查

Builder 与 runtime 同时锁定可读 Tag 和多架构 manifest digest。M0 Compose 只运行三个无业务状态的进程空壳，不包含 PostgreSQL、对象存储、迁移或业务 Schema。

本记录是工程许可筛查，不替代法律意见。最终递归依赖清单由 M0 证据中的 `DEPENDENCY_LICENSE_REPORT.md` 给出。
