# 07 Tool Loop

## 1. 目标与职责
- 定义 `run_tools` 自动循环语义，确保行为与评审结论一致。
- 明确并发执行、结果回填、步数上限与失败策略。
- 依赖 `12-tool-definition.md` 作为 `ToolRegistry`/`Tool`/`ToolExecutor`/`ToolExecError` 的唯一定义源。

## 2. Public API/类型签名（最终形态）
本模块只引用类型，不重复定义工具类型：
```rust
pub struct RunToolsRequest {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub max_steps: Option<u8>, // None => builder default (default: 8)
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
}

pub struct RunToolsResponse {
    pub output_text: String,
    pub steps: u8,
    pub transcript: Vec<Message>,
    pub usage_total: Usage,
}
```

## 3. 输入输出与数据流
输入：
- 初始会话消息、可执行工具列表、可选步数上限。

输出：
- 最终文本、执行步数、完整 transcript、累计 usage。

数据流：
1. 用 `req.tools` 构建 `ToolRegistry`。
2. 从 `req.tools` 投影出 `Vec<ToolDescriptor>`，填入 `GenerateTextRequest.tools`。
3. 调用模型得到文本或 `tool_calls`。
4. 有 `tool_calls` 时并发执行工具并回填 `MessageRole::Tool` 消息。
5. 循环直到收敛或达到步数上限。

## 4. 核心算法/状态机（含伪代码）
状态机：
```text
StepStart -> ModelCall -> (NeedTool? yes -> ExecuteTools -> AppendResults -> StepStart)
                           (NeedTool? no  -> Completed)
StepStart(max_steps exceeded) -> Failed(MaxStepsExceeded)
```

伪代码：
```text
messages = req.messages.clone()
usage_total = Usage::default()
resolved_max_steps = req.max_steps.unwrap_or(builder.default_max_steps)
registry = ToolRegistry::from_tools(req.tools)?
descriptors = req.tools.iter().map(|t| t.descriptor.clone()).collect()

for step in 1..=resolved_max_steps:
    model_req = GenerateTextRequest {
        model: req.model,
        messages,
        tools: Some(descriptors),
        temperature: req.temperature,
        max_output_tokens: req.max_output_tokens,
        stop_sequences: req.stop_sequences,
    }
    model_resp = client.generate_text(model_req)
    usage_total = usage_total + model_resp.usage // 简单逐步累加
    messages.push(assistant_message_from(model_resp))

    if model_resp.tool_calls.is_empty():
        return RunToolsResponse { output_text, steps: step, transcript: messages, usage_total }

    tool_results = execute_calls_concurrently(model_resp.tool_calls, registry)
    messages.extend(tool_results_as_tool_messages(tool_results))

return Err(MaxStepsExceeded { partial_transcript: messages, steps: resolved_max_steps })
```

并发规则：
- 同一步多个工具调用并发执行。
- 回填顺序按模型返回的 `tool_calls` 原始顺序，不按完成时间。

## 5. 边界条件与失败模式
- 边界：
- 实际步数 `resolved_max_steps` 必须在 `1..=32`。
- `max_steps=None` 时使用 builder 默认值（默认 8）。
- `tools` 为空时 `run_tools` 退化为单次 `generate_text`。

- 失败模式：
- 工具名未注册 -> `UnknownTool`（不可恢复）。
- 参数 schema 校验失败 -> `InvalidToolArgs`（不可恢复）。
- 达到步数上限仍未收敛 -> `MaxStepsExceeded`。
- 工具执行超时或执行失败 -> 可恢复，回填 `ToolResult.is_error=true`。

## 6. 错误码与错误映射
- `UnknownTool`：模型请求不存在工具，直接失败。
- `InvalidToolArgs`：参数 JSON 解析或 schema 校验失败，直接失败。
- `MaxStepsExceeded`：达到上限仍有待执行 tool call。
- `ToolExecError::Execution/Timeout`：映射为可恢复工具结果，回填 `is_error=true`。
- `ProviderServerError` / `RateLimited` / `Timeout`：来自模型调用链路。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 两步闭环：step1 tool call，step2 final text。
- 同一步两个工具并发执行，回填顺序稳定。
- `usage_total` 等于各步 `usage` 简单累加。

- 失败：
- 未知工具名返回 `UnknownTool`。
- 工具参数错误返回 `InvalidToolArgs`。
- `max_steps=Some(0)` 返回 `InvalidRequest`。

- 边界：
- `max_steps=None` 使用默认值。
- `max_steps=Some(1)` 且出现 tool call 时立即 `MaxStepsExceeded`。
- 工具执行失败回填 `is_error=true` 后可继续下一步。

## 8. 与其他模块的依赖契约
- 请求/响应类型依赖 `02-types-and-model.md`。
- 模型调用依赖 `03-client-api.md` 与 Provider adapters。
- 工具定义、注册、执行与错误模型依赖 `12-tool-definition.md`。
- 错误语义依赖 `08-error-handling.md`。
- 测试要求由 `10-testing-plan.md` 约束。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不实现无限代理循环策略。
- 当前不实现跨请求工具缓存。

- 扩展点：
- 增加并发上限和失败重试策略。
- 增加工具调用审计日志与 tracing span。
