# 04 Provider OpenAI

## 1. 目标与职责
- 定义 OpenAI Adapter 的请求映射、响应归一化和流式解析规则。
- 严格输出统一类型，不泄漏 OpenAI 私有结构到上层。
- 支持普通生成与带工具描述的生成（仅返回 `tool_calls`，不自动执行工具）。

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
}
```

目标端点（MVP）：
- `POST /v1/chat/completions`
- 请求头：`Authorization: Bearer <OPENAI_API_KEY>`

## 3. 输入输出与数据流
请求映射：
- `ModelRef.model` -> `model`
- `Message.parts` -> OpenAI `messages[].content`
- `GenerateTextRequest.tools` -> OpenAI `tools[]`（仅映射 `ToolDescriptor`）
- `stream=true` 时走 SSE

`ContentPart` 映射伪代码：
```text
to_openai_message(message):
    if message.role in {System, User, Assistant}:
        if message.parts is single Text:
            content = text
        else:
            content = map each ContentPart to OpenAI content part
    if message.role == Tool:
        // ToolResult 回填消息
        role = "tool"
        tool_call_id = tool_result.call_id
        content = json_string(tool_result.output_json)
```

响应映射：
- `choices[0].message.content` -> `GenerateTextResponse.output_text`
- `choices[0].message.tool_calls` -> `GenerateTextResponse.tool_calls`
- `choices[0].finish_reason` -> `FinishReason`
- `usage` -> `Usage`

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
    if frame.data == "[DONE]":
        emit Done
        break
    chunk = parse_json(frame.data)
    emit mapped StreamEvent from chunk
```

工具调用映射规则：
- `tool_calls[].id` -> `ToolCall.call_id`
- `tool_calls[].function.name` -> `ToolCall.tool_name`
- `tool_calls[].function.arguments`（JSON 字符串）-> `ToolCall.args_json`

`finish_reason` 映射表：
- `stop` -> `FinishReason::Stop`
- `length` -> `FinishReason::Length`
- `tool_calls` -> `FinishReason::ToolCalls`
- `content_filter` -> `FinishReason::ContentFilter`
- 其他值 -> `FinishReason::Unknown(raw)`

## 5. 边界条件与失败模式
- 边界：
- MVP 仅覆盖 `chat.completions`，不覆盖 `responses`。
- 仅处理文本与工具调用，不处理多模态输入。
- `generate_text` 即使返回 `tool_calls` 也不执行工具。

- 失败模式：
- 返回非 JSON（或 schema 变化）-> `InvalidResponse`。
- SSE frame 非法 JSON -> `StreamProtocol`。
- function arguments 非法 JSON -> `InvalidToolArgs`。

## 6. 错误码与错误映射
- HTTP 401/403 -> `AuthFailed`
- HTTP 429 -> `RateLimited`（可重试）
- HTTP 500-599 或 529 -> `ProviderServerError`（可重试）
- HTTP 400/404/422 -> `InvalidRequest`
- 网络错误 -> `Transport`
- 请求超时 -> `Timeout`
- 解析失败 -> `InvalidResponse` 或 `StreamProtocol`

权威口径：
- 状态码集合以 `08-error-handling.md` 为准，本文件不单独扩展。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 非流式文本映射正确。
- 流式 token delta 映射为 `TextDelta`。
- `tools[]` 映射只包含描述字段（不包含执行器）。
- `finish_reason` 映射表覆盖所有已知值。

- 失败：
- 401/429/5xx/529 映射正确。
- `tool_calls.function.arguments` 非 JSON -> `InvalidToolArgs`。

- 边界：
- 空 `choices` -> `InvalidResponse`。
- 未知 `finish_reason` -> `FinishReason::Unknown`。

## 8. 与其他模块的依赖契约
- 实现 `01-architecture.md` 的 `ProviderAdapter` 契约。
- 输入输出必须使用 `02-types-and-model.md` 的统一类型。
- 工具描述来自 `12-tool-definition.md` 的 `ToolDescriptor`。
- 流式解析遵循 `06-streaming.md`。
- 错误分类遵循 `08-error-handling.md`。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不支持 OpenAI Realtime/WebSocket。
- 当前不支持上传类输入或文件引用。

- 扩展点：
- 增加 `responses` endpoint 适配。
- 增加 logprobs、metadata 的归一化输出。
