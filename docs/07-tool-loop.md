# 07 Tool Loop

## 1. 目标与职责
- 定义 `run_tools` 的自动循环语义，确保行为与 AI SDK 体验一致。
- 明确 `max_steps` 限制、并发执行规则、结果回填顺序。
- 规范工具执行错误传播方式，确保调用方可诊断。

## 2. Public API/类型签名（最终形态）
```rust
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, ToolExecError>;
}

pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub executor: std::sync::Arc<dyn ToolExecutor>,
}

pub struct RunToolsRequest {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub max_steps: u8, // default: 8
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
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
- 初始会话消息、工具列表、步数上限。
输出：
- 最终文本、执行步数、完整 transcript、累计 token usage。

数据流：
1. 调用模型，得到 assistant 文本或 tool calls。
2. 若有 tool calls，则在同一步并发执行工具。
3. 将每个工具结果追加为 `MessageRole::Tool` 消息。
4. 进入下一步模型调用。
5. 收敛后返回，或触发 `MaxStepsExceeded`。

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
for step in 1..=req.max_steps:
    model_resp = client.generate_text(messages, tools=req.tools)
    usage_total += model_resp.usage
    messages.push(assistant_message_from(model_resp))

    if model_resp.tool_calls.is_empty():
        return RunToolsResponse {
            output_text: model_resp.output_text,
            steps: step,
            transcript: messages,
            usage_total,
        }

    calls = model_resp.tool_calls
    validate_tool_names(calls, req.tools)?
    tool_results = execute_concurrently_preserve_input_order(calls)
    messages.extend(tool_results_as_tool_messages(tool_results))

return Err(MaxStepsExceeded { partial_transcript: messages, steps: req.max_steps })
```

并发规则：
- 同一步多个工具调用并发执行。
- 回填顺序按模型返回的 `tool_calls` 原始顺序，不按完成时间。
- 任一工具执行失败时：
- 默认将失败包装为 `ToolResult { is_error: true }` 回填，并继续当前步骤闭环。
- 若错误是不可恢复（例如未知工具），立即失败返回。

## 5. 边界条件与失败模式
- 边界：
- `max_steps` 范围 `1..=32`，默认 8。
- `tools` 可为空；为空时 `run_tools` 退化为单次 `generate_text`。
- 失败模式：
- tool name 未注册 -> `UnknownTool`。
- tool args 不符合 schema -> `InvalidToolArgs`。
- 循环到达上限仍未收敛 -> `MaxStepsExceeded`。

## 6. 错误码与错误映射
- `UnknownTool`: 模型请求了不存在的工具。
- `InvalidToolArgs`: 参数 JSON 解析或 schema 验证失败。
- `MaxStepsExceeded`: 达到步数上限仍有待执行 tool call。
- `ProviderServerError` / `RateLimited` / `Timeout`: 来自模型调用链路。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- 两步闭环：step1 tool_call，step2 final text。
- 同一步两个工具并发执行，回填顺序稳定。
- 失败：
- 未知工具名返回 `UnknownTool`。
- 工具参数错误返回 `InvalidToolArgs`。
- 边界：
- `max_steps=1` 时若出现 tool call 立即 `MaxStepsExceeded`。
- 工具执行错误可回填 `is_error=true` 并进入下一步模型修复。

## 8. 与其他模块的依赖契约
- 请求/响应类型依赖 `02-types-and-model.md`。
- 模型调用依赖 `03-client-api.md` 与 Provider adapters。
- 工具定义、注册和参数校验依赖 `12-tool-definition.md`。
- 错误语义依赖 `08-error-handling.md`。
- 测试要求由 `10-testing-plan.md` 约束。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不实现“无限代理循环”或策略学习。
- 当前不实现跨请求工具缓存。
- 扩展点：
- 增加策略参数（并发上限、失败重试策略）。
- 增加工具调用审计日志与 tracing span。
