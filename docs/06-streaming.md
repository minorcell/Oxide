# 06 Streaming

## 1. 目标与职责
- 定义统一流式协议层，将 Provider SSE 事件转换为 `StreamEvent`。
- 规定流结束、异常中断、坏帧处理的行为，保证调用侧可预测。
- 提供可复用的 SSE 帧解析规则，供 OpenAI 与 Anthropic 共享。

## 2. Public API/类型签名（最终形态）
```rust
pub enum StreamEvent {
    TextDelta { text: String },
    ToolCallReady { call: ToolCall },
    Usage { usage: Usage },
    Done,
}

pub type TextStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, AiError>> + Send>>;

// internal
pub(crate) fn parse_sse_lines(input: &str) -> Vec<SseFrame>;
pub(crate) struct SseFrame {
    pub event: Option<String>,
    pub data: String,
}
```

## 3. 输入输出与数据流
输入：
- Provider 返回的 bytes stream（HTTP chunked/SSE）。
输出：
- 统一 `StreamEvent` 事件流。

数据流分层：
1. `bytes` -> UTF-8 `line`。
2. `line` -> `SseFrame`（按空行分隔消息）。
3. `SseFrame` -> provider-specific event。
4. provider event -> unified `StreamEvent`。

## 4. 核心算法/状态机（含伪代码）
SSE 解析伪代码：
```text
buffer = ""
for each incoming chunk:
    buffer += decode_utf8(chunk)
    while buffer contains "\n\n":
        raw_frame = split_once(buffer, "\n\n")
        frame = parse_frame(raw_frame) // read "event:" and "data:"
        yield frame
```

统一流状态机：
```text
Init -> Streaming -> Done
          |           ^
          v           |
        Error --------
```

规则：
- 在 `Done` 之后不得再发任何事件。
- 若出现协议错误，立刻输出错误并终止流。

## 5. 边界条件与失败模式
- 边界：
- 不保证 provider 事件与 chunk 边界对齐，必须支持跨 chunk 组帧。
- 支持空 `event:` 字段，按 `data` 默认事件解析。
- 失败模式：
- UTF-8 解码失败 -> `StreamProtocol`。
- `data:` 非法 JSON（当 provider 要求 JSON）-> `StreamProtocol`。
- 提前断流无终止标记 -> `StreamProtocol`。

## 6. 错误码与错误映射
- 帧格式错误（无法构造完整消息）-> `StreamProtocol`
- Provider 特定事件无法识别且无法降级 -> `InvalidResponse`
- 读取超时 -> `Timeout`
- 上游连接断开 -> `Transport`（若无可恢复上下文）

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 单 chunk 多 frame 与多 chunk 单 frame 都可正确解析。
- OpenAI `[DONE]` 与 Anthropic `message_stop` 均映射到 `Done`。
- 失败：
- 非法 UTF-8 / 非法 JSON 触发 `StreamProtocol`。
- 边界：
- 空行、注释行、keepalive `ping` 被正确忽略或转换。
- 同一响应中既有文本又有工具调用事件时顺序稳定。

## 8. 与其他模块的依赖契约
- 由 `04-provider-openai.md` 和 `05-provider-anthropic.md` 调用。
- 产物类型必须是 `02-types-and-model.md` 的 `StreamEvent`。
- 错误分类遵循 `08-error-handling.md`。
- 被 `09-axum-adapter.md` 直接消费。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不支持 WebSocket/Reatime 双向流协议。
- 当前不提供流事件持久化存储。
- 扩展点：
- 增加 `StreamEvent::ProviderRaw` 调试模式（feature gated）。
- 增加背压配置与流量控制指标输出。
