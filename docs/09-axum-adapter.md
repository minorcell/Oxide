# 09 Axum Adapter

## 1. 目标与职责
- 定义可选 `axum` feature 下的流式输出适配层。
- 将 `TextStream` 统一事件转换为 SSE 响应，供 Web 服务端直接使用。
- 保持框架适配层与核心 SDK 解耦。

## 2. Public API/类型签名（最终形态）
```rust
#[cfg(feature = "axum")]
pub fn stream_to_sse(
    stream: TextStream,
) -> axum::response::sse::Sse<
    impl futures_core::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>
>;
```

SSE 事件名规范：
- `token`
- `tool_call`
- `usage`
- `done`
- `error`

## 3. 输入输出与数据流
输入：
- 统一 `TextStream`（`StreamEvent` 序列）。
输出：
- Axum `Sse` 响应。

映射规则：
- `StreamEvent::TextDelta` -> `event: token`, `data: {"text":"..."}`
- `StreamEvent::ToolCallReady` -> `event: tool_call`, `data: {...}`
- `StreamEvent::Usage` -> `event: usage`, `data: {...}`
- `StreamEvent::Done` -> `event: done`, `data: {}`
- `Err(AiError)` -> `event: error`, `data: {"code":"...","message":"..."}`

## 4. 核心算法/状态机（含伪代码）
伪代码：
```text
fn stream_to_sse(stream):
    return stream.map(|item| match item:
        Ok(TextDelta{text}) => Event("token", json{text})
        Ok(ToolCallReady{call}) => Event("tool_call", json{call})
        Ok(Usage{usage}) => Event("usage", json{usage})
        Ok(Done) => Event("done", "{}")
        Err(err) => Event("error", json{code, message})
    )
```

状态机：
```text
StreamOpen -> Emit(token/tool_call/usage)* -> Emit(done|error) -> Close
```

## 5. 边界条件与失败模式
- 边界：
- `axum` feature 关闭时不导出任何 adapter API。
- `done` 或 `error` 发出后必须关闭流。
- 失败模式：
- 事件序列错乱导致前端消费异常。
- 事件 data 不是合法 JSON。

## 6. 错误码与错误映射
- 适配层不新建错误码，直接转发 `AiError`。
- `AiError` -> SSE `error` event 的 `code` 字段使用 `AiErrorCode` 字符串值。
- 适配层内部序列化失败视为 `InvalidResponse` 并发送 `error` 事件后关闭连接。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 输入 `TextDelta -> Usage -> Done` 输出 SSE 顺序一致。
- 输入 `ToolCallReady` 事件可被正确序列化。
- 失败：
- 输入 `Err(AiError::RateLimited)` 输出 `error` 事件且包含 code/message。
- 边界：
- `done` 后无额外事件。
- 空文本 delta（`text=""`）仍能合法输出事件。

## 8. 与其他模块的依赖契约
- 依赖 `06-streaming.md` 的统一 `StreamEvent`。
- 错误字段依赖 `08-error-handling.md`。
- 仅是适配层，不依赖 `04/05-provider-*.md` 具体实现。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不支持 Warp/Actix 的并行适配。
- 当前不内置客户端断线重连协议。
- 扩展点：
- 新增 `with_heartbeat(interval)` 保活事件。
- 新增自定义事件名映射策略。
