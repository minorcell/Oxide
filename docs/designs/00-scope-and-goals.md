# 00 Scope and Goals

## 1. 目标与职责
- 定义 Oxide MVP 的产品边界，避免后续实现出现范围漂移。
- 固化“文档先行”原则：先完成模块设计文档并评审通过，再进入编码。
- 统一术语和验收口径，保证后续实现、测试、发布使用同一语义。

MVP 目标：
- 交付一个可发布的 Rust crate。
- 支持统一多 Provider（OpenAI + Anthropic）。
- 支持 `generate_text`、`stream_text`、`run_tools(max_steps)`。
- 提供可选 `axum` SSE 适配能力。

非目标（本阶段不做）：
- Embeddings、Image、Audio、RAG、Agent Memory。
- 前端 Hook 对等层。
- Provider 插件生态与动态加载。

里程碑：
- M1: 完成 `docs/00-12` 设计文档并完成一致性审查。
- M2: 按 `10-testing-plan.md` 驱动实现核心 SDK。
- M3: 完成测试矩阵并满足 `11-release-plan.md` 发布门禁。

术语：
- Provider: 具体模型供应商（OpenAI/Anthropic）。
- Adapter: Provider 协议适配层。
- Unified Model: SDK 对外统一类型层。
- Tool Loop: 模型与工具执行的循环控制流程。

## 2. Public API/类型签名（最终形态）
```rust
pub struct AiClient;
pub struct AiClientBuilder;

impl AiClient {
    pub fn builder() -> AiClientBuilder;
    pub async fn generate_text(&self, req: GenerateTextRequest) -> Result<GenerateTextResponse, AiError>;
    pub async fn stream_text(&self, req: GenerateTextRequest) -> Result<TextStream, AiError>;
    pub async fn run_tools(&self, req: RunToolsRequest) -> Result<RunToolsResponse, AiError>;
}

pub enum ProviderKind { OpenAi, Anthropic }
pub struct ModelRef { pub provider: ProviderKind, pub model: String }
pub struct Message { /* 见 02-types-and-model.md */ }
pub enum StreamEvent { /* 见 06-streaming.md */ }
```

## 3. 输入输出与数据流
- 输入：
- 应用层传入请求（模型标识、消息、采样参数、工具列表）。
- 输出：
- 统一响应对象或统一流事件，不直接暴露 Provider 原始协议。
- 核心数据流：
1. 应用 -> `AiClient`。
2. `AiClient` -> Router（按 `ProviderKind` 选择 Adapter）。
3. Adapter -> Provider HTTP/SSE。
4. Provider 响应 -> Adapter 归一化。
5. 归一化结果 -> 应用。

## 4. 核心算法/状态机（含伪代码）
文档阶段流程状态机：
```text
Drafting -> CrossDocReview -> ReadyForImplementation
```

伪代码：
```text
if all docs(00..12) exist and pass consistency checks:
    state = ReadyForImplementation
else:
    state = Drafting or CrossDocReview
```

一致性检查项：
- 类型名称一致（`ModelRef`/`Message`/`StreamEvent`）。
- 错误码一致（`AiErrorCode` 集合和映射规则）。
- 默认值一致（例如 `max_steps=8`、`timeout=30s`）。
- 测试验收项一致（至少覆盖 6 个锁定场景）。

## 5. 边界条件与失败模式
- 边界：
- 仅支持 OpenAI 与 Anthropic。
- 仅支持文本生成与工具调用闭环，不支持多模态输入。
- 失败模式：
- 文档间接口不一致导致实现不可落地。
- 错误语义冲突导致测试不可判定。
- 未定义默认值导致行为不稳定。

## 6. 错误码与错误映射
顶层错误码（详见 `08-error-handling.md`）：
- `InvalidRequest`
- `AuthFailed`
- `RateLimited`
- `ProviderServerError`
- `Transport`
- `Timeout`
- `StreamProtocol`
- `UnknownTool`
- `InvalidToolArgs`
- `MaxStepsExceeded`
- `InvalidResponse`

映射原则：
- 文档层只定义语义与分类，不绑定具体实现细节。
- 每个模块文档必须引用同一错误码集合，不允许增删别名。

## 7. 测试用例列表（成功/失败/边界）
- 成功：
- OpenAI 非流式生成成功。
- Anthropic 流式 token 连续输出成功。
- `run_tools` 两步闭环成功。
- 失败：
- 401 -> `AuthFailed`。
- 429 -> `RateLimited`。
- 工具参数非法 -> `InvalidToolArgs`。
- 边界：
- `max_steps` 触顶 -> `MaxStepsExceeded` 且返回 partial transcript。
- SSE 中断或坏 JSON -> `StreamProtocol`。

## 8. 与其他模块的依赖契约
- 依赖 `01-architecture.md` 提供分层边界。
- 依赖 `02-types-and-model.md` 提供统一类型定义。
- 依赖 `12-tool-definition.md` 提供工具定义的权威来源。
- 依赖 `08-error-handling.md` 提供统一错误分类与重试策略。
- 依赖 `10-testing-plan.md` 提供验收矩阵。
- 依赖 `11-release-plan.md` 提供发布门禁和 semver 约束。

## 9. 非目标与后续扩展点
- 非目标：
- 不在本阶段实现 provider 生态扩展机制。
- 不在本阶段设计前端 API 或 JS 兼容 facade。
- 扩展点：
- 新增 Provider（Gemini/Ollama）时复用 Adapter 契约。
- 增加 `generate_object`/结构化输出能力。
- 增加 tracing/metrics 特性开关。
