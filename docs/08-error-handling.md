# 08 Error Handling

## 1. 目标与职责
- 定义全局统一错误模型，覆盖请求校验、网络、Provider、流式、工具执行。
- 固化错误码与重试语义，确保调用方可根据 `code/retryable` 稳定处理。
- 建立 HTTP/SSE/工具错误到统一错误的映射标准。

## 2. Public API/类型签名（最终形态）
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiErrorCode {
    InvalidRequest,
    AuthFailed,
    RateLimited,
    ProviderServerError,
    Transport,
    Timeout,
    StreamProtocol,
    UnknownTool,
    InvalidToolArgs,
    MaxStepsExceeded,
    InvalidResponse,
}

#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct AiError {
    pub code: AiErrorCode,
    pub message: String,
    pub provider: Option<ProviderKind>,
    pub status: Option<u16>,
    pub retryable: bool,
    pub request_id: Option<String>,
    pub raw_body: Option<String>,
}
```

重试相关（内部）：
```rust
pub(crate) fn classify_http_error(status: u16) -> AiErrorCode;
pub(crate) fn is_retryable(code: AiErrorCode) -> bool;
pub(crate) fn backoff_delay_ms(attempt: u8) -> u64;
```

## 3. 输入输出与数据流
输入：
- 参数校验错误、网络错误、HTTP 错误、SSE 协议错误、工具执行错误。
输出：
- 统一 `AiError`，带错误码、是否可重试、可选上下文字段。

分类流程：
1. 先判断本地校验错误（优先级最高）。
2. 再判断网络/超时。
3. 再判断 HTTP 状态码。
4. 再判断响应解析与流协议错误。
5. 工具循环阶段再映射工具相关错误。

## 4. 核心算法/状态机（含伪代码）
错误分类伪代码：
```text
fn classify(err):
    if err is local_validation: return InvalidRequest(retryable=false)
    if err is timeout: return Timeout(retryable=true)
    if err is transport: return Transport(retryable=true)
    if err is http:
        map status:
          401|403 => AuthFailed(false)
          429 => RateLimited(true)
          500|502|503|504|529 => ProviderServerError(true)
          _ => InvalidRequest(false)
    if err is sse_protocol: return StreamProtocol(false)
    if err is invalid_json_or_shape: return InvalidResponse(false)
```

退避策略伪代码：
```text
base = 200ms
cap = 2000ms
delay = min(base * 2^attempt + jitter(0..100), cap)
```

## 5. 边界条件与失败模式
- 边界：
- 仅在 `retryable=true` 且 `attempt < max_retries` 时重试。
- `AuthFailed` 永不重试。
- 失败模式：
- provider 返回非标准错误结构导致 message 丢失。
- 多层包装错误导致根因被覆盖。

## 6. 错误码与错误映射
- `InvalidRequest`: 本地参数错误、provider 4xx（非 401/403/429）。
- `AuthFailed`: 401/403。
- `RateLimited`: 429。
- `ProviderServerError`: 5xx/529。
- `Transport`: DNS/TLS/连接中断。
- `Timeout`: 客户端超时。
- `StreamProtocol`: SSE 帧、增量协议不合法。
- `UnknownTool`: 工具名不匹配。
- `InvalidToolArgs`: 参数解析或 schema 校验失败。
- `MaxStepsExceeded`: 工具循环未收敛。
- `InvalidResponse`: JSON 结构与预期不匹配。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 每类输入错误都可映射到唯一 `AiErrorCode`。
- `retryable` 与预期一致。
- 失败：
- 401/403 误判为可重试时应被测试阻断。
- 429 未标记可重试时应失败。
- 边界：
- `max_retries=0` 时即使 `retryable=true` 也不重试。
- request_id/raw_body 缺失时不影响错误分类。

## 8. 与其他模块的依赖契约
- `03-client-api.md` 使用本文件定义的重试与分类策略。
- `04/05-provider-*.md` 必须把 provider 错误信息映射到本错误模型。
- `06-streaming.md` 的协议错误必须归入 `StreamProtocol`。
- `07-tool-loop.md` 的工具异常必须归入 `UnknownTool` 或 `InvalidToolArgs`。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不做错误国际化（i18n）。
- 当前不定义 provider 专属错误码枚举。
- 扩展点：
- 增加 `source` 链路以保留原始错误树。
- 增加可观测性标签（error family、retry stage）。
