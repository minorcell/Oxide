# 05 Provider Anthropic

## 1. 目标与职责
- 定义 Anthropic Provider 适配层的协议映射与流式事件归一化。
- 保证 Anthropic `messages` API 输出统一类型，不向上层暴露原生事件细节。
- 支持文本生成、流式输出与工具调用链路。

## 2. Public API/类型签名（最终形态）
内部 API（`pub(crate)`）：
```rust
pub(crate) struct AnthropicAdapter {
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) api_version: String, // default: "2023-06-01"
    pub(crate) http: reqwest::Client,
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    async fn generate_text(&self, req: &GenerateTextRequest) -> Result<GenerateTextResponse, AiError>;
    async fn stream_text(&self, req: &GenerateTextRequest) -> Result<TextStream, AiError>;
    async fn generate_tool_step(&self, req: &GenerateTextRequest) -> Result<GenerateTextResponse, AiError>;
}
```

目标端点（MVP）：
- `POST /v1/messages`
- 请求头：`x-api-key`、`anthropic-version`

## 3. 输入输出与数据流
请求映射：
- `messages[]` 映射到 Anthropic content blocks。
- system 消息映射到 `system` 字段。
- 工具描述映射到 `tools[]`（含 input schema）。

响应映射：
- `content[].type=text` -> `output_text` 片段拼接。
- `content[].type=tool_use` -> `ToolCall`。
- `stop_reason` -> `FinishReason`。
- `usage.input_tokens/output_tokens` -> `Usage`。

流式事件映射（Anthropic SSE）：
- `content_block_delta`(text_delta) -> `StreamEvent::TextDelta`
- `content_block_start` + `tool_use` -> 聚合 tool call
- `message_delta` usage 更新 -> `StreamEvent::Usage`
- `message_stop` -> `StreamEvent::Done`

## 4. 核心算法/状态机（含伪代码）
流式解析伪代码：
```text
for frame in sse_frames(bytes):
    evt = parse_json(frame.data)
    match evt.type:
        "content_block_delta" if text_delta => emit TextDelta
        "content_block_start" if tool_use => start tool_call_buffer
        "content_block_delta" if tool_use_delta => append args chunk
        "content_block_stop" for tool_use => emit ToolCallReady(buffered_call)
        "message_delta" => maybe emit Usage
        "message_stop" => emit Done and break
```

工具参数处理：
- Anthropic 工具输入已是 JSON 对象时直接映射 `args_json`。
- 若分片增量构造 JSON，必须在 `content_block_stop` 时完成校验。

## 5. 边界条件与失败模式
- 边界：
- MVP 不覆盖 beta headers 与实验模型特性。
- 仅覆盖文本与工具相关 content block。
- 失败模式：
- tool_use 分片 JSON 拼接失败 -> `InvalidToolArgs`。
- 未收到 `message_stop` 连接中断 -> `StreamProtocol`。
- `content` 为空且无错误说明 -> `InvalidResponse`。

## 6. 错误码与错误映射
- HTTP 401/403 -> `AuthFailed`
- HTTP 429 -> `RateLimited`
- HTTP 500/529 -> `ProviderServerError`
- HTTP 400/404 -> `InvalidRequest`
- 网络/TLS 错误 -> `Transport`
- 流事件结构非法 -> `StreamProtocol`
- 响应字段缺失 -> `InvalidResponse`

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 非流式文本与 usage 映射正确。
- 流式 text delta 连续输出并最终 `Done`。
- tool_use 映射为 `ToolCallReady`。
- 失败：
- `tool_use` 增量 JSON 非法 -> `InvalidToolArgs`。
- 429/500 映射为可重试错误。
- 边界：
- 仅 tool_use 无 text 的响应可被正确处理。
- 未知 `stop_reason` 映射为 `FinishReason::Unknown`。

## 8. 与其他模块的依赖契约
- 实现 `ProviderAdapter` 契约（`01-architecture.md`）。
- 输出统一类型（`02-types-and-model.md`）。
- 流式处理遵循 `06-streaming.md` 的事件约定。
- 错误分类遵循 `08-error-handling.md`。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不支持 Anthropic 附件/文件类输入。
- 当前不支持 server tool use 托管能力。
- 扩展点：
- 增加对 beta 头和新事件类型的兼容策略。
- 增加 provider metadata（request id）回传。

