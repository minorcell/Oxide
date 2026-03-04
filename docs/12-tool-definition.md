# 12 Tool Definition

## 1. 目标与职责

- 定义工具（Tool）的完整设计规范，包括声明、注册、参数校验、执行与结果回填。
- 统一 OpenAI/Anthropic 的工具协议抽象，确保 `run_tools` 只依赖统一契约。
- 固化工具执行安全边界与失败语义，避免实现阶段出现不兼容行为。

本模块覆盖：

- `ToolSpec` 静态定义。
- `ToolExecutor` 执行契约。
- 工具参数 JSON Schema 约束。
- 工具注册表唯一性与查找规则。
- 工具执行结果回填格式。

## 2. Public API/类型签名（最终形态）

```rust
use std::sync::Arc;
use serde_json::Value;

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, args: Value) -> Result<Value, ToolExecError>;
}

#[derive(Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value, // JSON Schema Draft 2020-12 subset
    pub executor: Arc<dyn ToolExecutor>,
}

pub struct ToolRegistry {
    // name -> ToolSpec
}

impl ToolRegistry {
    pub fn from_tools(tools: Vec<ToolSpec>) -> Result<Self, AiError>;
    pub fn get(&self, name: &str) -> Option<&ToolSpec>;
    pub fn names(&self) -> Vec<&str>;
}

#[derive(Debug, thiserror::Error)]
pub enum ToolExecError {
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("timeout")]
    Timeout,
}
```

### JSON Schema 校验库

使用 `jsonschema` crate（底层为 `jsonschema-rs`），Cargo 约束：

```toml
[dependencies]
jsonschema = { version = "0.30", default-features = false }
```

选型理由：
- 原生支持 JSON Schema Draft 2020-12，满足本 SDK 的 schema 子集目标。
- 提供编译期预处理 API（`jsonschema::validator_for(&schema)?`），可在 `ToolRegistry::from_tools` 时一次性编译，运行时仅调用 `validator.validate(&instance)`，无重复解析开销。
- 直接与 `serde_json::Value` 集成，无额外转换层。

`ToolRegistry` 内部结构调整：存储 `name -> (ToolSpec, jsonschema::Validator)` 映射；`from_tools` 在注册阶段对每个 `input_schema` 编译并持有 `Validator`，若编译失败则返回 `InvalidRequest`。

---

命名约束：

- `ToolSpec.name` 使用 `^[a-zA-Z0-9_-]{1,64}$`。
- 名称大小写敏感。
- 同名工具禁止重复注册。

## 3. 输入输出与数据流

输入：

- 应用提供 `Vec<ToolSpec>`。
- 模型在响应中给出 `ToolCall { call_id, tool_name, args_json }`。

输出：

- 成功执行输出 `ToolResult { call_id, output_json, is_error=false }`。
- 失败执行输出 `ToolResult { call_id, output_json=<error_payload>, is_error=true }` 或抛出不可恢复错误。

数据流：

1. `run_tools` 初始化 `ToolRegistry`。
2. 每步从模型响应提取 `ToolCall` 列表。
3. 对每个 call：查找工具 -> 校验参数 -> 执行工具。
4. 将结果回填为 `MessageRole::Tool` 消息进入下一轮模型调用。

## 4. 核心算法/状态机（含伪代码）

工具执行伪代码：

```text
registry = ToolRegistry::from_tools(req.tools)?
// from_tools 内部：对每个 ToolSpec.input_schema 调用 jsonschema::validator_for()
// 若编译失败则立即返回 InvalidRequest

for call in model_resp.tool_calls:
    (tool, validator) = registry.get(call.tool_name) or Err(UnknownTool)
    validator.validate(call.args_json) or Err(InvalidToolArgs)
    result = tool.executor.execute(call.args_json)
    if result is Ok(v):
        emit ToolResult(call_id=call.call_id, output_json=v, is_error=false)
    else if result is recoverable:
        emit ToolResult(call_id=call.call_id, output_json=error_payload, is_error=true)
    else:
        return Err(mapped_error)
```

并发调度伪代码（同一步）：

```text
spawn all tool calls concurrently
join all results
sort by original call order
append as tool messages
```

## 5. 边界条件与失败模式

- 边界：
- `tools` 为空时不构建 registry，`run_tools` 退化为单步生成。
- `input_schema` 为空对象时视为“允许任意 JSON 输入”。
- 单步内多个相同工具名调用允许执行（通过 `call_id` 区分）。

- 失败模式：
- 工具名不存在：`UnknownTool`。
- 参数不符合 schema：`InvalidToolArgs`。
- 工具执行超时：默认回填 `is_error=true`（可配置为直接失败）。
- 工具返回非 JSON 可序列化结构：映射为 `ToolExecError::Execution`。

## 6. 错误码与错误映射

- `UnknownTool`：
- 触发条件：`tool_name` 未在 `ToolRegistry` 中命中。
- `InvalidToolArgs`：
- 触发条件：参数 JSON 解析失败或 schema 校验失败。
- `Timeout`：
- 工具执行超时（当配置为终止策略时）。
- `InvalidResponse`：
- call_id 缺失、tool call 结构不完整。

映射原则：

- 可恢复工具失败优先转为 `ToolResult.is_error=true` 继续闭环。
- 不可恢复错误（例如结构破坏、未知工具）直接返回 `AiError`。

## 7. 测试用例列表（成功/失败/边界）

- 成功：
- 工具注册成功并可按名称查找。
- schema 校验通过后执行并回填 `is_error=false`。
- 同一步多个工具并发执行后按原始顺序回填。

- 失败：
- 重复工具名注册失败（`InvalidRequest`）。
- 非法工具名（不符合正则）注册失败（`InvalidRequest`）。
- 参数不匹配 schema 返回 `InvalidToolArgs`。
- 未知工具名返回 `UnknownTool`。

- 边界：
- 空 schema（允许任意 JSON）路径可正常执行。
- 工具执行返回错误时回填 `is_error=true` 并继续下一步。
- 大参数 payload（例如 >64KB）仍可正确传递与校验。

## 8. 与其他模块的依赖契约

- 被 `07-tool-loop.md` 直接依赖，作为执行和校验规范。
- 与 `02-types-and-model.md` 共享 `ToolCall`/`ToolResult` 结构。
- 与 `03-client-api.md` 共同定义 `run_tools` 行为。
- 错误归类必须符合 `08-error-handling.md`。
- 测试用例必须纳入 `10-testing-plan.md` 的 `tests/tools.rs`。

## 9. 非目标与后续扩展点

- 非目标：
- 当前不实现远程工具发现协议（MCP 等）。
- 当前不提供内置沙箱执行器。
- 当前不支持流式工具输出（tool output streaming）。

- 扩展点：
- 增加工具级超时/重试策略（每个工具可配置）。
- 增加工具权限模型（allowlist/denylist）。
- 增加 schema 编译缓存与性能优化（基础编译缓存已内置于 `ToolRegistry`，此处指跨请求共享 registry 实例）。
