# V1 十七项关闭矩阵（AR00 候选证据）

本矩阵只绑定同一个 clean candidate：

- Candidate commit：`68f1c1772993b00dd9c6390c237fa44d9ec14c7e`
- Candidate tree：`554b4e5aa82c55cf1f3ae216d34eb94566053f1c`
- 工具链：Python `3.14.6`、Node `v24.17.0`、pnpm `11.9.0`、Rust/Cargo `1.96.0`
- 执行模式：`LOCAL_CLEAN_CLONE_WORKFLOW_EQUIVALENT`
- `p00-negative-matrix`：5/5 case PASS；原始制品包 SHA-256 `239035a387472e83633eb22f460d0bd980ba38e4f61c4a2159505051332ee9e5`
- `release-readiness-evidence`：READY、0 blocker；原始制品包 SHA-256 `caa8f098da55575954ef4f2cfa37d0455f24a64f47edf529d5a0fdc82bc07e71`
- Release JUnit：756 tests、0 failures、0 skipped；SHA-256 `6648fecdc2146dd972b9f636e9ee8584f65ffaa52fdde7b716bb983eb50869b3`
- 原始制品根目录：`/tmp/trpg-ar00-68f1c1772993b00dd9c6390c237fa44d9ec14c7e/artifacts`

本机 `gh` 凭证失效，因此没有声称这些制品来自 GitHub Actions。P00 使用
`GITHUB_RUN_ID=LOCAL`；release evidence 中的 `680001/1` 是
production-security Compose 项目名所需的本地兼容命名空间，不是 GitHub run
ID/attempt。失败的本地预检尝试与最终通过输出一并保留在 release 制品包中。

| # | 验收项 | 状态 | Candidate SHA | 命令/工作流 | Exit | 原始日志 SHA-256 | 制品 SHA-256 | 备注 |
|---:|---|---|---|---|---:|---|---|---|
| 1 | Docker Compose 一键部署并完成首次初始化向导 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/production-security-smoke.sh`; `python3 scripts/ci/release_readiness.py ... --require-ready` | `0/0` | `1094d24499cad201dc0b6b4e25cf2979ef787e6c90308c565bf28ed11c90e242` | `e8b9ad1933e4297a687d43e26932632e3ff8d4fa3f6534fa67c9a3f4056e7344` | 完整 production Compose 产品图完成构建、迁移与 healthy 检查；readiness 为 READY/0 blocker，readiness JSON SHA-256 `ca66963da0139c90353e76294102b271bf14bd4a66b0218ee89153fd6549d979`。 |
| 2 | 云端 API、Ollama、llama.cpp 可配置并可执行 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Provider config/route、Ollama/llama.cpp 类型及 `provider_send` 精确 endpoint/model/bytes 边界通过。 |
| 3 | HUMAN_KP 与 AI_KP 均可创建 Campaign | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | S02 authority fixture、两种 AuthorityMode 命令路径及真实 Campaign/character repository API 测试通过。 |
| 4 | Campaign KP 权威模式创建后不可变 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | `shared_kernel_keeps_authority_contract_immutable` 与 in-place mode/owner change 拒绝测试通过。 |
| 5 | COC7 角色创建、审核与加入 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | COC7 派生属性、非法角色卡拒绝、Campaign invite/character repository 链通过。 |
| 6 | 完整 Tutorial Scenario 可玩通 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | `tutorial_runs_through_real_repository_event_store_outbox_and_witness` 与 HUMAN_KP tutorial slice 通过。 |
| 7 | 调查、检定、线索、SAN、NPC、战斗、追逐 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | 服务端骰、核心线索 fail-forward、SAN、NPC visibility、战斗和追逐状态机均通过。 |
| 8 | 多人在线、分队、旁观与断线恢复 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Realtime room sync、private-group default deny、spectator replay 授权及 replay 恢复路径通过。 |
| 9 | 隐私贯穿摘要、RAG、导出和回放 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Restricted summary、player RAG、player export 与 replay 的 visibility/redaction 负向测试通过。 |
| 10 | AI KP 决策经 Agent、工具门和事件提交 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | `ai_kp_orchestrator_tool_request_commits_through_event_store`、tool executor deny 与 Event Store provenance 测试通过。 |
| 11 | HUMAN_KP 下 AI 仅提供草稿，不越权裁定 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | `human_kp_agent_formal_tool_is_draft_only` 与 pending-decision 降级测试通过。 |
| 12 | Campaign fork 保持父子来源 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Locked child contract、父 Campaign 不变、canonical public snapshot lineage 测试通过。 |
| 13 | 玩家/KP/审计三类导出 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Player export allow/keeper-only deny、restricted export deny 与 Golden Scenario export diff fixture 通过。 |
| 14 | Golden Scenario 全链路通过 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Golden Scenario stage gate、正式决策事件路径、绕过拒绝和 tutorial/export diff 全部通过。 |
| 15 | 本地模型通过八类认证后才可担任 AI KP | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Model certification fixture 与 Level 4 keeper gate 测试通过。 |
| 16 | 禁止本地模型静默回退云端 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Silent local-to-cloud fallback deny 与 explicit audited route allow 测试通过。 |
| 17 | AI 决策可解释、可追溯、可审计 | PASS | `68f1c1772993b00dd9c6390c237fa44d9ec14c7e` | `release-readiness-evidence`: `bash scripts/ci/test-all.sh` | `0` | `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45` | `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a` | Decision record hash link/provenance/visibility、canonical evidence resolution 与 decision trace map 测试通过。 |

## 证据文件

- Release command manifest：`release-readiness/release-input.json`，SHA-256 `070553e3643355efec407cb966c2b3e6663611ca0c0832a8c498a8586fdf548a`
- Release raw log：`release-readiness/release-input.log`，SHA-256 `a78cfa1592828f8e08b6ebbd61c2f5385e05199baae47647603798b5c2035e45`
- Production-security manifest：`release-readiness/production-security-input.json`，SHA-256 `e8b9ad1933e4297a687d43e26932632e3ff8d4fa3f6534fa67c9a3f4056e7344`
- Production-security raw log：`release-readiness/production-security-input.log`，SHA-256 `1094d24499cad201dc0b6b4e25cf2979ef787e6c90308c565bf28ed11c90e242`
- Final readiness manifest：`release-readiness/evidence-manifest.json`，SHA-256 `4eea70e55eb626c5b8248a71c4be792ebe61fee5ae85c9ff652b766eddf47a29`
- File-level checksum lists：P00 `12121723d3fbb2d03df0b36c733619934caada1e6e17c70d43ce0ddea8f4fe93`；release `de7a1238fb83f932ca3ccd12e5cf32bd772a2f536b0e9bac821e2d706ada7143`

所有最终 evidence manifest 均在容器仍运行时以 `live_context=True`
重新校验通过；归档前对全部 raw artifacts 执行了通用 credential/private-key
模式扫描及本次运行时敏感值逐字扫描，均无命中。
