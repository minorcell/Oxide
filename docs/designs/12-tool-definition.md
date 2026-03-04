# 12 Tool Definition

## 1. 目标与职责
- 定义工具系统的唯一权威规范，覆盖描述层、执行层、注册表、参数校验和执行语义。
- 明确 `ToolDescriptor`（可序列化入模）与 `Tool`（可执行）的边界，避免执行器语义泄漏到通用生成请求。
- 统一工具错误到 `ToolResult` 回填或 `AiError` 失败的转换规则，保证 `run_tools` 行为一致。

本文件是以下类型的单一权威定义源：
- `ToolDescriptor`
- `Tool`
- `ToolExecutor`
- `ToolExecError`
- `ToolRegistry`

## 2. Public API/类型签名（最终形态）
```rust
use std::sync::Arc;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value, // JSON Schema Draft 2020-12 subset
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, args: Value) -> Result<Value, ToolExecError>;
}

#[derive(Clone)]
pub struct Tool {
    pub descriptor: ToolDescriptor,
    pub executor: Arc<dyn ToolExecutor>,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolExecError {
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("timeout")]
    Timeout,
}

pub struct ToolRegistry {
    // name -> (Tool, jsonschema::Validator)
}

impl ToolRegistry {
    pub fn from_tools(tools: Vec<Tool>) -> Result<Self, AiError>;
    pub fn get(&self, name: &str) -> Option<&Tool>;
    pub fn names(&self) -> Vec<&str>;
}
```

JSON Schema 校验库：
```toml
[dependencies]
jsonschema = { version = "0.30", default-features = false }
```

命名约束：
- `ToolDescriptor.name` 使用 `^[a-zA-Z0-9_-]{1,64}$`。
- 名称大小写敏感。
- 同名工具禁止重复注册。

## 3. 输入输出与数据流
输入：
- 应用层传入 `Vec<Tool>`（描述+执行器）。
- 生成请求侧传入 `Option<Vec<ToolDescriptor>>`（纯描述）。
- 模型返回 `ToolCall { call_id, tool_name, args_json }`。

输出：
- 成功执行：`ToolResult { call_id, output_json, is_error=false }`。
- 可恢复执行失败：`ToolResult { call_id, output_json=<error_payload>, is_error=true }`。
- 不可恢复失败：抛出 `AiError` 终止当前 `run_tools`。

数据流：
1. `run_tools` 用 `Vec<Tool>` 初始化 `ToolRegistry`。
2. 发送模型请求时，将 `Vec<Tool>` 投影为 `Vec<ToolDescriptor>` 入模。
3. 模型返回 tool calls 后，按名称从 registry 查找工具并执行。
4. 结果回填为 tool 消息进入下一步模型调用。

## 4. 核心算法/状态机（含伪代码）
工具执行伪代码：
```text
registry = ToolRegistry::from_tools(req.tools)?
// from_tools: compile each descriptor.input_schema via jsonschema::validator_for()
// compile failure => InvalidRequest

for call in model_resp.tool_calls:
    (tool, validator) = registry.get(call.tool_name) or Err(UnknownTool)
    validator.validate(call.args_json) or Err(InvalidToolArgs)
    result = tool.executor.execute(call.args_json)
    if result is Ok(v):
        emit ToolResult(call_id=call.call_id, output_json=v, is_error=false)
    else if result is ToolExecError::Execution | ToolExecError::Timeout:
        emit ToolResult(call_id=call.call_id, output_json=error_payload(result), is_error=true)
    else:
        return Err(mapped_ai_error)
```

并发调度伪代码（同一步）：
```text
spawn all tool calls concurrently
join all results
sort by original tool_call order
append as tool messages
```

## 5. 边界条件与失败模式
- 边界：
- `tools` 为空时不构建 registry，`run_tools` 退化为单次生成。
- `input_schema` 为 `{}` 时表示允许任意 JSON 输入。
- 同一步允许重复调用同一工具（按 `call_id` 区分）。

- 失败模式：
- 工具名不存在：`UnknownTool`（不可恢复，直接失败）。
- 参数不符合 schema：`InvalidToolArgs`（不可恢复，直接失败）。
- 工具执行超时/执行失败：默认回填 `is_error=true`，可继续闭环。

## 6. 错误码与错误映射
- `UnknownTool`：
- 条件：`tool_name` 未在 `ToolRegistry` 命中。
- `InvalidToolArgs`：
- 条件：参数 JSON 解析失败或 schema 校验失败。
- `Timeout`（工具执行层）：
- 默认映射为可恢复工具失败，回填 `ToolResult.is_error=true`。
- `InvalidRequest`：
- 工具注册阶段 schema 编译失败、名称非法、重名冲突。

映射原则：
- 可恢复错误：`ToolExecError::Execution`、`ToolExecError::Timeout` -> `ToolResult.is_error=true`。
- 不可恢复错误：`UnknownTool`、`InvalidToolArgs`、结构破坏错误 -> 直接 `AiError`。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- `ToolRegistry::from_tools` 成功注册并可查找。
- schema 校验通过后执行并回填 `is_error=false`。
- 多工具并发执行后按原始 call 顺序回填。

- 失败：
- 重复工具名注册失败（`InvalidRequest`）。
- 非法工具名注册失败（`InvalidRequest`）。
- 参数不匹配 schema 返回 `InvalidToolArgs`。
- 未知工具名返回 `UnknownTool`。

- 边界：
- 空 schema 路径可执行。
- 工具执行失败回填 `is_error=true` 后可继续下一步。
- 大参数 payload（例如 >64KB）可正确传递与校验。

## 8. 与其他模块的依赖契约
- `02-types-and-model.md` 引用本文件的 `ToolDescriptor` 与 `Tool` 类型。
- `07-tool-loop.md` 依赖本文件的 `ToolRegistry`/`ToolExecutor`/`ToolExecError`。
- `04/05-provider-*.md` 仅消费 `ToolDescriptor` 进行 provider 映射。
- `08-error-handling.md` 依本文件定义可恢复与不可恢复工具错误边界。
- `10-testing-plan.md` 的 `tests/tools.rs` 覆盖本模块规范。

## 9. 非目标与后续扩展点
- 非目标：
- 当前不实现远程工具发现协议（MCP 等）。
- 当前不提供内置沙箱执行器。
- 当前不支持工具输出流式传输。

- 扩展点：
- 增加工具级超时/重试策略（每工具配置）。
- 增加工具权限模型（allowlist/denylist）。
- 增加跨请求共享 registry 缓存。
