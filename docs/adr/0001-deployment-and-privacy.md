# ADR-0001：双部署形态与隐私感知模型路由

- 状态：Accepted
- 决策：同时支持平台云服务和 Compose 自托管；房间使用 standard/private_hybrid/local_only。
- 约束：Provider fallback 不得跨越隐私边界；local_only 故障时停止 Agent。
- 后果：Provider Router 必须感知数据分类、模型能力、健康度与预算。
