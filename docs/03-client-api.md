# 03 Client API

## 1. 目标与职责
- 定义 `AiClient` 与 `AiClientBuilder` 的公开契约和默认行为。
- 规定调用入口、配置项、超时/重试/鉴权策略。
- 保证调用侧只需统一 API，不需感知 Provider 差异。

## 2. Public API/类型签名（最终形态）
```rust
use std::time::Duration;

pub struct AiClientBuilder {
    // provider keys
    // base urls
    // timeout / retries / defaults
}

impl AiClientBuilder {
    pub fn openai_api_key(self, key: impl Into<String>) -> Self;
    pub fn anthropic_api_key(self, key: impl Into<String>) -> Self;
    pub fn openai_base_url(self, base_url: impl Into<String>) -> Self;
    pub fn anthropic_base_url(self, base_url: impl Into<String>) -> Self;
    pub fn timeout(self, timeout: Duration) -> Self; // default: 30s
    pub fn max_retries(self, retries: u8) -> Self;   // default: 2
    pub fn default_max_steps(self, max_steps: u8) -> Self; // default: 8
    pub fn user_agent(self, ua: impl Into<String>) -> Self;
    pub fn build(self) -> Result<AiClient, AiError>;
}

pub struct AiClient;

impl AiClient {
    pub fn builder() -> AiClientBuilder;
    pub async fn generate_text(&self, req: GenerateTextRequest) -> Result<GenerateTextResponse, AiError>;
    pub async fn stream_text(&self, req: GenerateTextRequest) -> Result<TextStream, AiError>;
    pub async fn run_tools(&self, req: RunToolsRequest) -> Result<RunToolsResponse, AiError>;
}
```

默认值：
- `timeout = 30s`
- `max_retries = 2`
- `default_max_steps = 8`
- `user_agent = "oxide-ai-sdk/<version>"`

## 3. 输入输出与数据流
`generate_text`：
1. 参数校验。
2. 选择 provider adapter。
3. 执行网络请求（带重试策略）。
4. 返回统一响应。

`stream_text`：
1. 参数校验。
2. 创建 Provider 流请求。
3. 输出统一 `TextStream`。

`run_tools`：
1. 校验工具注册与 `max_steps`。
2. 每步请求模型，若有 tool call 则执行工具并追加消息。
3. 收敛为最终文本或到达步数上限。

## 4. 核心算法/状态机（含伪代码）
重试与调度伪代码：
```text
call_with_retry(op):
    for attempt in 0..=max_retries:
        result = op()
        if result is Ok: return Ok
        if !retryable(result.err): return Err
        if attempt == max_retries: return Err
        sleep(backoff(attempt))
```

`run_tools` 主循环伪代码：
```text
messages = req.messages
resolved_max_steps = req.max_steps.unwrap_or(builder.default_max_steps)
descriptors = req.tools.iter().map(|t| t.descriptor.clone()).collect()
for step in 1..=resolved_max_steps:
    resp = generate(messages, tools=descriptors)
    if resp.tool_calls.is_empty():
        return final(resp, step, messages)
    results = execute_tools_concurrently(resp.tool_calls)
    messages.append(tool_results_as_messages(results))
return MaxStepsExceeded(partial=messages)
```

## 5. 边界条件与失败模式
- 边界：
- 请求中的 `model.provider` 必须已启用对应 feature。
- `run_tools.max_steps=Some(0)` 视为 `InvalidRequest`。
- `run_tools.max_steps=None` 时使用 builder 默认值（默认 8）。
- 失败模式：
- API key 缺失导致请求前失败。
- 重试后仍失败，返回最后一次错误并保留上下文。
- stream 中断时生成可诊断错误。

## 6. 错误码与错误映射
- 缺少 API key -> `InvalidRequest`（message 指明缺失字段）。
- Provider 401/403 -> `AuthFailed`。
- Provider 429 -> `RateLimited`（`retryable=true`）。
- 网络超时 -> `Timeout`（`retryable=true`）。
- 工具循环超步 -> `MaxStepsExceeded`（携带 partial transcript）。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- Builder 默认值生效。
- `generate_text` 与 `stream_text` 路由正确。
- `run_tools` 在 2 步内成功收敛。
- 失败：
- API key 缺失错误。
- Provider 返回 401/429/500 映射正确。
- 边界：
- `max_retries=0` 不触发 sleep/backoff。
- `default_max_steps` 被 `run_tools.max_steps=Some(x)` 覆盖。

## 8. 与其他模块的依赖契约
- 依赖 `02-types-and-model.md` 的请求/响应/流类型。
- 依赖 `04/05-provider-*.md` 的 adapter 实现。
- 依赖 `07-tool-loop.md` 定义的工具执行语义。
- 依赖 `12-tool-definition.md` 定义的工具注册、schema 校验与执行契约。
- 依赖 `08-error-handling.md` 的重试判定和错误结构。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不提供同步阻塞 API。
- 当前不提供内置会话持久化。
- 扩展点：
- 增加 `with_middleware` 以支持 tracing/metrics。
- 增加请求级超时与取消 token。
