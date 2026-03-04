# 04 Provider OpenAI

## 1. 目标与职责
- 定义 OpenAI Provider 适配层的请求映射、响应归一化和流解析规则。
- 作为 `ProviderAdapter` 的一个实现，严格输出统一类型，不泄漏 OpenAI 特定结构。
- 支持非流式、流式、工具调用三条路径。

## 2. Public API/类型签名（最终形态）
内部 API（`pub(crate)`）：
```rust
pub(crate) struct OpenAiAdapter {
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) http: reqwest::Client,
}

#[async_trait]
impl ProviderAdapter for OpenAiAdapter {
    async fn generate_text(&self, req: &GenerateTextRequest) -> Result<GenerateTextResponse, AiError>;
    async fn stream_text(&self, req: &GenerateTextRequest) -> Result<TextStream, AiError>;
    async fn generate_tool_step(&self, req: &GenerateTextRequest) -> Result<GenerateTextResponse, AiError>;
}
```

目标端点（MVP）：
- `POST /v1/chat/completions`
- 请求头：`Authorization: Bearer <OPENAI_API_KEY>`

## 3. 输入输出与数据流
请求映射：
- `ModelRef.model` -> `model`
- `Message` -> OpenAI `messages[]`
- `ToolSpec` -> `tools[]`
- `stream=true` 时走 SSE 流

响应映射：
- `choices[0].message.content` -> `GenerateTextResponse.output_text`
- `choices[0].finish_reason` -> `FinishReason`
- `usage` -> `Usage`
- `tool_calls` -> `Vec<ToolCall>`

## 4. 核心算法/状态机（含伪代码）
非流式伪代码：
```text
payload = map_generate_request(req)
resp = http.post("/v1/chat/completions", payload)
json = parse_json(resp)
return normalize_openai_response(json)
```

流式伪代码：
```text
payload.stream = true
for frame in sse_frames(resp.bytes_stream()):
    if frame.data == "[DONE]": emit Done; break
    chunk = parse_json(frame.data)
    emit mapped StreamEvent from chunk
```

工具调用映射规则：
- OpenAI `tool_calls[].id` -> `ToolCall.call_id`
- OpenAI `tool_calls[].function.name` -> `ToolCall.tool_name`
- OpenAI `tool_calls[].function.arguments` JSON string -> `ToolCall.args_json`

## 5. 边界条件与失败模式
- 边界：
- MVP 仅覆盖 `chat.completions`，不覆盖 `responses` endpoint。
- 仅处理文本与工具调用，不处理多模态内容。
- 失败模式：
- OpenAI 返回非 JSON（或 JSON schema 变更）导致 `InvalidResponse`。
- SSE frame 为非法 JSON 导致 `StreamProtocol`。
- function arguments 不是合法 JSON 导致 `InvalidToolArgs`。

## 6. 错误码与错误映射
- HTTP 401/403 -> `AuthFailed`
- HTTP 429 -> `RateLimited`（可重试）
- HTTP 500/502/503/504 -> `ProviderServerError`（可重试）
- HTTP 400/404/422 -> `InvalidRequest`
- 网络错误 -> `Transport`
- 请求超时 -> `Timeout`
- 解析失败 -> `InvalidResponse` 或 `StreamProtocol`

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 非流式文本输出映射正确。
- 流式 token delta 映射为 `TextDelta`。
- 工具调用字段映射正确。
- 失败：
- 401/429/500 错误映射正确。
- `tool_calls.function.arguments` 非 JSON 字符串时失败为 `InvalidToolArgs`。
- 边界：
- 空 `choices` 返回 `InvalidResponse`。
- `finish_reason` 未知值映射到 `FinishReason::Unknown`。

## 8. 与其他模块的依赖契约
- 实现 `01-architecture.md` 的 `ProviderAdapter` 契约。
- 输入/输出必须使用 `02-types-and-model.md` 类型。
- 流式解析需遵循 `06-streaming.md` 的 SSE 与事件规范。
- 错误分类遵循 `08-error-handling.md`。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不支持 OpenAI Realtime/WebSocket。
- 当前不支持并发分段上传或文件引用。
- 扩展点：
- 增加 `responses` endpoint 适配。
- 增加 logprobs、response metadata 的归一化。
