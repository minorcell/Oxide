# 10 Testing Plan

## 1. 目标与职责
- 定义 MVP 的完整测试矩阵、Mock 策略和验收门禁。
- 约束测试优先级：先验证统一模型与错误语义，再验证 Provider 协议细节。
- 保证实现前就有可执行的验收标准。

## 2. Public API/类型签名（最终形态）
本模块不新增生产 Public API；测试侧约定以下内部 helper：
```rust
pub(crate) async fn mock_openai_server() -> wiremock::MockServer;
pub(crate) async fn mock_anthropic_server() -> wiremock::MockServer;
pub(crate) fn build_test_client(...) -> AiClient;
pub(crate) fn collect_stream(stream: TextStream) -> Vec<Result<StreamEvent, AiError>>;
```

测试入口文件规划：
- `tests/generate.rs`
- `tests/stream.rs`
- `tests/tools.rs`
- `tests/error_mapping.rs`
- `tests/axum_sse.rs`（`cfg(feature = "axum")`）

## 3. 输入输出与数据流
输入：
- Mock Provider 响应（HTTP body、SSE frame、错误状态码）。
- 构造的统一请求对象。
输出：
- SDK 响应对象或错误对象，以及流事件序列。

测试数据流：
1. 启动 `wiremock` server。
2. 将 `AiClientBuilder` 的 base URL 指向 mock server。
3. 发起 SDK 调用。
4. 断言返回值、事件顺序、错误码、重试次数。

## 4. 核心算法/状态机（含伪代码）
测试编排伪代码：
```text
for scenario in test_matrix:
    setup_mock(scenario)
    result = execute_sdk_call(scenario.request)
    assert_match(result, scenario.expected)
```

流式断言状态：
```text
CollectingEvents -> VerifyOrder -> VerifyTerminalEvent -> Pass/Fail
```

## 5. 边界条件与失败模式
- 边界：
- 覆盖最小和最大 `max_steps`。
- 覆盖空文本、仅工具调用、无 usage 场景。
- 失败模式：
- 测试仅覆盖 happy path，未覆盖错误语义回归。
- SSE 测试未覆盖分片边界导致生产解析问题。

## 6. 错误码与错误映射
必须验证映射一致性：
- 401/403 -> `AuthFailed`
- 429 -> `RateLimited`
- 500-599/529 -> `ProviderServerError`
- 非法 JSON -> `InvalidResponse` 或 `StreamProtocol`
- 未知工具 -> `UnknownTool`
- 参数非法 -> `InvalidToolArgs`
- 超步数 -> `MaxStepsExceeded`

## 7. 测试用例列表（成功/失败/边界）
必测成功场景：
1. OpenAI 非流式成功与 usage 汇总正确。
2. Anthropic 流式 token 连续输出并 `Done`。
3. `run_tools` 两步闭环成功（tool call -> result -> final text）。

必测失败场景：
1. OpenAI 401/429/500 映射正确。
2. Anthropic tool_use 增量 JSON 非法 -> `InvalidToolArgs`。
3. SSE 中断或坏 JSON -> `StreamProtocol`。
4. 工具名不存在 -> `UnknownTool`。

必测边界场景：
1. `max_steps` 触顶 -> `MaxStepsExceeded` 且包含 partial transcript。
2. 同一步多个工具并发执行，结果回填顺序稳定。
3. `max_retries=0` 与 `max_retries=2` 行为差异正确。
4. `axum` SSE 事件顺序：`token* -> usage? -> done|error`。

验收标准：
- `cargo test` 全绿。
- provider 相关测试不得依赖真实外网。
- 每个错误码至少一个直接测试覆盖。

## 8. 与其他模块的依赖契约
- 类型断言基于 `02-types-and-model.md`。
- API 行为断言基于 `03-client-api.md`。
- Provider 协议断言基于 `04/05-provider-*.md`。
- 流式断言基于 `06-streaming.md`。
- 工具循环断言基于 `07-tool-loop.md`。
- 工具注册与 schema 校验断言基于 `12-tool-definition.md`。
- 错误断言基于 `08-error-handling.md`。
- Axum 适配断言基于 `09-axum-adapter.md`。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不做基准压测与性能回归门禁。
- 当前不做模糊测试（fuzz）与故障注入平台。
- 扩展点：
- 增加 property-based tests（例如 `proptest`）验证解析器稳健性。
- 增加 CI 覆盖率阈值门禁（line/branch coverage）。
