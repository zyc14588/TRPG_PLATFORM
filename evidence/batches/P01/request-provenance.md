# P01 Request Provenance

- Source: user attachment `pasted-text.txt`
- SHA-256: `72ce8e93062d3e0204509276b3b1d8cb54aba344ad2e96e0ed2b74b083a55cc6`
- Classification: user-supplied P01 execution request; no repository Prompt ID is inferred.
- Execution boundary: P01 only; P02 remains forbidden.

## Captured request

# P01 — 建立产品执行入口与规范模块边界

> 这是一个可直接交给 Codex 的独立执行提示词。
> **执行序号**：2 / 15
> **最高严重程度**：Blocker（本批次问题构成：Blocker 1，Medium 4，Low 1）
> **主责问题**：AUD-001, AUD-018, AUD-026, AUD-039, AUD-040, AUD-044
> **前置批次**：P00
> **后续批次**：P02；本提示词明确禁止提前执行。
> **估算工作量**：L

---

你现在是本仓库的实施工程师、代码审计修复工程师和本批次验收负责人。

## 一、唯一任务

只执行 **P01：建立产品执行入口与规范模块边界**。

批次目标：

> 产出真实 API、Realtime、Agent Worker、Admin、Migration Runner 和 Web 产品入口，建立允许的依赖方向、统一错误/事件契约与可验证进程健康；本批次不实现完整业务纵向链。

不得开始 `P02` 或任何其他后续批次。即使本批次提前完成，也必须在输出验收报告后停止。

## 二、开始前的强制门禁

1. 阅读仓库根目录的 `AGENTS.md`、顶层设计、当前 Cargo/包管理配置以及本批次涉及的源码和测试。
2. 执行并记录：

```bash
git rev-parse HEAD
git status --porcelain=v1
git diff --check
```

3. 检查前置批次 `P00` 的完成证据。前置未完成时：

   - 不修改代码；
   - 输出 `BATCH_STATUS: BLOCKED`；
   - 列出缺失的具体前置验收；
   - 立即停止。
4. 运行本批次相关的现有测试，记录修改前基线。不得把基线失败隐瞒为本批次通过。
5. 修改前列出预计变更文件。若实际需要超出“允许修改范围”，输出 `SCOPE_BLOCKED` 并停止，不得顺手扩大范围。

## 三、本批次必须关闭的问题

| ID | 严重程度 | 模块 | 文件/区域 | 问题 | 关键证据 | 修复验收 |
|---|---|---|---|---|---|---|
| AUD-001 | Blocker | 产品运行架构 | Cargo.toml；crates/**；缺失 apps/**、main.rs、src/bin | 全仓只有 Library crate，没有 API、Realtime、Agent Worker、Web 或 Admin 的产品执行入口。 | 12 个 Cargo package 均无 [[bin]]/main.rs；package.json 无产品 start/build；无 apps/**。 | cargo build --workspace --all-targets 生成规定的服务二进制；每个服务由自身进程返回深层健康结果。 |
| AUD-018 | Medium | 工程结构 | 多个 crate 的 contract descriptor/Prompt 元数据 | 大量施工合同、空 Service/Repository 和 Prompt 元数据进入生产 crate，静态描述代替行为。 | 多处宏生成 unit struct、字段名数组和固定 capability。 | 生产 crate 不再暴露空壳服务；contract metadata 有单一生成来源。 |
| AUD-026 | Medium | Shared Kernel/API | CommandEnvelope::governed | 生产 public API 暴露固定 Fixture ID 与幂等键的命令工厂。 | 工厂使用固定测试值且位于生产接口。 | 生产 crate 无固定 fixture 工厂；测试通过专用 test-support 使用。 |
| AUD-039 | Medium | 维护性 | domain 重复模块；多组 *_impl wrapper | 完全重复生产模块和过量公共 wrapper 形成高发散技术债务。 | 三组文件内容相同，公共函数比例约 93.7%。 | 重复检测无完全相同生产模块；公开 API 有清单与兼容测试。 |
| AUD-044 | Medium | Event Contract/测试 | fixtures event streams；生产 event constructors | Fixture 和生产 Event Contract 漂移。 | 11 个关键 Fixture event 在生产字面量中无 exact match；Golden 只检查 marker。 | Schema、生产构造器、Fixture、OpenAPI 和投影的事件名/版本自动一致。 |
| AUD-040 | Low | 错误契约 | AgentError::code | 错误码命名规范不一致。 | ToolPermissionDenied 使用 CamelCase，其余为 SCREAMING_SNAKE_CASE。 | 错误码契约测试覆盖全部枚举。 |

## 四、允许修改范围

### 允许目录和模块

- 新增 apps/**
- crate 依赖图
- 统一错误/事件 Registry
- 移出生产 crate 的施工元数据
- 新增 apps/web/** 与根前端 workspace 配置

### 预计新增或修改文件

- apps/api-server/src/main.rs
- apps/realtime-server/src/main.rs
- apps/agent-worker/src/main.rs
- apps/admin-server/src/main.rs
- apps/migration-runner/src/main.rs
- crates/trpg-contracts/**
- apps/web/package.json
- apps/web/src/**
- apps/web/index.html

### 允许的数据库范围

- 无

### 允许的 API 范围

- /health/live
- /health/ready

### 必须新增或强化的测试范围

- Binary build/start smoke
- 错误码和 Event Registry contract

除以上范围外，不得修改其他产品功能。允许为编译所需做最小的 import、依赖声明或接口适配，但必须逐项在最终报告说明；若出现跨领域设计变化，停止并报告，而不是自行扩大批次。

## 五、必须保持的架构不变量

- 遵守顶层设计中的 Authority、事件化、Visibility、Agent 治理和一键部署边界。

## 六、明确任务列表

- 定义端口/适配器架构和允许的依赖方向，并用 CI 静态检查固化
- 新增 API、Realtime、Agent Worker、Admin、Migration Runner 可执行二进制
- 新增可构建、可启动的最小 Web 产品入口；只提供应用壳、配置加载和健康/版本展示，不实现业务 UI
- 建立统一 Event Registry/Schema 和 Wire Error Code，修复事件/错误命名漂移
- 移除生产 public API 中的固定 Fixture 工厂，把测试工厂迁入 test-support
- 收敛完全重复模块和空 Service/Repository；Prompt/批次元数据迁移到测试或工具层
- 让每个进程的 liveness/readiness 来自真实初始化状态，而非固定文本

## 七、实施顺序

必须按以下顺序执行，不得跳步：

1. 冻结依赖规则
2. 创建二进制
3. 统一契约
4. 迁移最小调用
5. 构建/启动 smoke

每一步完成后，先运行该步最小测试，再进入下一步。任何一步失败且无法在当前范围修复时，批次状态为 `BLOCKED` 或 `PARTIAL_NOT_ACCEPTED`，不得继续后续批次。

## 八、禁止事项

### 本批次特定禁止事项

- 不在本批次实现完整业务功能
- 不做全仓命名或目录大重构
- 不引入微服务间远程调用，先保持模块化单体/少量进程

### 全局禁止事项

- 不执行任何后续批次，不预建后续批次的表、API、UI、Worker 或占位实现。
- 不用固定返回值、静态 HTTP 200、预写 PASS、Marker-only 断言或 Mock 全链路冒充真实功能。
- 不删除或跳过失败测试，不降低断言，不关闭 Lint，不添加 `continue-on-error`。
- 不进行与问题无关的全仓格式化、重命名、目录重排或依赖升级。
- 不把秘密、Token、私密游戏内容或模型上下文写入日志、测试快照或提交记录。
- 不修改历史审计问题 ID，不把未验证项目标记为已通过。

## 九、验收标准

本批次只有同时满足以下标准才可完成：

- [ ] Release build 产出 API、Realtime、Agent Worker、Admin、Migration Runner 二进制和 Web 构建产物
- [ ] 每个进程能启动、优雅退出并暴露由自身实现的 liveness/readiness
- [ ] Web 应用可构建并由本地开发/预览命令启动，不使用 Compose Nginx 占位替代
- [ ] 禁止依赖方向、统一 Event Registry 和统一错误码由 CI 契约测试约束
- [ ] 生产 crate 不再公开固定 Fixture 工厂或完全重复实现

还必须逐项满足上表中每个 `AUD-ID` 的“修复验收”。任何一项未满足，均不得输出 `COMPLETE`。

## 十、必须执行的测试命令

实现完成后，至少实际执行以下命令，并记录退出码和关键输出：

```bash
cargo build --workspace --all-targets --release --locked
cargo test -p trpg-contracts --all-targets
pnpm --filter ./apps/web... build
./scripts/ci/service-process-smoke.sh
```

同时执行：

```bash
git diff --check
git diff --name-only
git status --porcelain=v1
```

测试纪律：

- 命令不存在时，若创建该命令属于本批次范围，则必须补齐；否则状态为 `BLOCKED`。
- 依赖服务不可用时，不得跳过；应使用本批次规定的真实测试环境，无法建立则状态不是 `COMPLETE`。
- 必须包含至少一个负向测试，证明旧缺陷或旧绕过路径已经失败。
- 对数据库、网络、并发、权限或模型边界的修复，纯函数测试不能替代 Integration/E2E。

## 十一、回归测试

必须重新验证：

- 全部 Library 单元测试
- 事件名和错误码兼容测试

不得因本批次修改破坏已通过的前置批次不变量。

## 十二、风险与回滚

### 已知风险

- 依赖方向调整可能暴露循环依赖

### 回滚方案

按服务骨架 Commit 回滚；保留 canonical Event/Error Schema，不恢复重复协议。

如包含 Migration、事件版本或持久化格式变更，必须优先使用前向兼容和备份恢复；不得使用删除正史数据的 Down Migration 作为常规应用回滚。

## 十三、完成定义

本批次的唯一完成定义：

> 存在可构建、可启动的真实服务进程和 Web 产品入口，并建立单一 Event/Error 契约及可强制的模块边界；完整业务纵向调用链明确留给 P07。

并且必须同时满足：

- 所有主责 `AUD-ID` 均有代码证据、测试证据和明确状态；
- 所有必需命令均实际通过；
- `git diff --name-only` 没有越界文件；
- 没有预写 PASS、跳过测试或临时安全放宽；
- 没有执行后续批次内容；
- 最终回复符合全局执行契约的固定格式。

## 十四、最终输出与停止

最终回复必须以以下两行之一开头：

```text
BATCH_STATUS: COMPLETE
BATCH_ID: P01
```

或：

```text
BATCH_STATUS: BLOCKED | PARTIAL_NOT_ACCEPTED
BATCH_ID: P01
```

然后给出变更、命令、验收矩阵、未验证项、风险和回滚说明。

**输出完成后立即停止。不得开始 P02。**
