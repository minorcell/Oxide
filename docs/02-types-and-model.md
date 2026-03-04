# 02 Types and Model

## 1. 目标与职责
- 定义 SDK 的统一类型系统，屏蔽 Provider 协议差异。
- 固化请求、响应、流事件、工具调用的标准数据模型。
- 约束字段语义与默认值，为编码和测试提供单一事实源。

## 2. Public API/类型签名（最终形态）
```rust
use std::pin::Pin;
use futures_core::Stream;
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum ProviderKind { OpenAi, Anthropic }

#[derive(Debug, Clone)]
pub struct ModelRef {
    pub provider: ProviderKind,
    pub model: String,
}

#[derive(Debug, Clone)]
pub enum MessageRole { System, User, Assistant, Tool }

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub parts: Vec<ContentPart>,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ContentPart {
    Text(String),
    ToolCall(ToolCall),
    ToolResult(ToolResult),
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub call_id: String,
    pub tool_name: String,
    pub args_json: Value,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub call_id: String,
    pub output_json: Value,
    pub is_error: bool,
}

/// Shared request type for both `generate_text` and `stream_text`.
/// Whether the call is streaming is determined by which `AiClient` method is invoked.
#[derive(Debug, Clone)]
pub struct GenerateTextRequest {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RunToolsRequest {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub max_steps: u8, // default = 8
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct GenerateTextResponse {
    pub output_text: String,
    pub finish_reason: FinishReason,
    pub usage: Usage,
    pub tool_calls: Vec<ToolCall>,
    pub raw_provider_response: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct RunToolsResponse {
    pub output_text: String,
    pub steps: u8,
    pub transcript: Vec<Message>,
    pub usage_total: Usage,
}

#[derive(Debug, Clone)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Unknown(String),
}

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextDelta { text: String },
    ToolCallReady { call: ToolCall },
    Usage { usage: Usage },
    Done,
}

pub type TextStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, AiError>> + Send>>;
```

## 3. 输入输出与数据流
- 输入模型：
- 应用构造 `GenerateTextRequest`（非流式与流式共用）/ `RunToolsRequest`。
- 统一模型到 Provider 模型转换：
- `Message` + `ContentPart` 映射到对应 Provider payload。
- 输出模型：
- 非流式：`GenerateTextResponse`。
- 流式：`StreamEvent` 序列。
- 工具循环：`RunToolsResponse`（含 transcript 与 usage 汇总）。

## 4. 核心算法/状态机（含伪代码）
请求验证与归一化伪代码：
```text
validate_messages(messages):
    if messages.is_empty(): error InvalidRequest
    for msg in messages:
        if msg.parts.is_empty(): error InvalidRequest
        if msg.role == Tool and not contains ToolResult: error InvalidRequest

normalize_usage(input, output):
    total = input + output
    return Usage { input, output, total }
```

流事件归一化状态：
```text
Streaming -> (TextDelta | ToolCallReady | Usage)* -> Done
```

## 5. 边界条件与失败模式
- 边界：
- `max_steps` 必须在 `1..=32` 范围内；默认 8。
- `temperature` 必须在 `0.0..=2.0`（超范围按 `InvalidRequest`）。
- 失败模式：
- Provider 返回字段缺失导致 `InvalidResponse`。
- Tool 消息链路断裂（`call_id` 找不到匹配）导致 `InvalidRequest`。

## 6. 错误码与错误映射
- `InvalidRequest`：参数校验失败、消息结构非法、采样参数越界。
- `InvalidResponse`：Provider 响应结构与预期模型不匹配。
- `UnknownTool`：`ToolCall.tool_name` 不在 `RunToolsRequest.tools`。
- `InvalidToolArgs`：`args_json` 与工具 schema 不匹配。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 合法消息集合可正确序列化到 Provider payload。
- `Usage.total_tokens` 计算正确。
- 失败：
- 空消息列表返回 `InvalidRequest`。
- 非法 `temperature` 返回 `InvalidRequest`。
- 边界：
- `max_steps=1` 与 `max_steps=32` 行为正确。
- `FinishReason::Unknown(x)` 可保留原始值。

## 8. 与其他模块的依赖契约
- `03-client-api.md` 只接收和返回本文件定义的公开类型；`generate_text` 与 `stream_text` 均复用 `GenerateTextRequest`。
- `04/05-provider-*.md` 必须实现统一类型到 provider payload 的双向映射。
- `06-streaming.md` 必须产生本文件定义的 `StreamEvent`。
- `07-tool-loop.md` 必须复用 `ToolCall`/`ToolResult` 类型。
- `12-tool-definition.md` 定义 `ToolSpec`/`ToolExecutor` 的约束细节。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不定义图像/音频 content part。
- 当前不定义 JSON Schema 到强类型 Rust 代码生成。
- 扩展点：
- `ContentPart` 后续可新增 `ImageUrl`/`AudioRef`。
- `GenerateTextResponse` 可扩展 logprobs 与 provider metadata。
