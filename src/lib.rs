pub mod agent;
pub mod client;
pub mod error;
pub mod model_adapters;
pub mod provider;
pub mod stream;
pub mod tool;
pub mod types;

#[cfg(feature = "axum")]
pub mod axum_sse;

pub use agent::{Agent, AgentBuilder, AgentCallPlan, PrepareCallCallback};
pub use client::{AiClient, AiClientBuilder};
pub use error::{AiError, AiErrorCode};
pub use model_adapters::ModelAdapter;
pub use model_adapters::anthropic::AnthropicAdapterSettings;
pub use model_adapters::google::GoogleAdapterSettings;
pub use model_adapters::openai::OpenAiAdapterSettings;
pub use model_adapters::openai_compatible::OpenAiCompatibleAdapterSettings;
pub use tool::{
    Tool, ToolBuilder, ToolDescriptor, ToolExecError, ToolExecutor, ToolRegistry, tool,
};
pub use types::{
    ContentPart, FinishCallback, FinishReason, GenerateTextRequest, GenerateTextResponse, Message,
    MessageRole, ModelRef, PrepareStepCallback, ProviderKind, RunToolsFinish, RunToolsPrepareStep,
    RunToolsPreparedStep, RunToolsRequest, RunToolsResponse, RunToolsStart, RunToolsStep,
    RunToolsStepStart, RunToolsToolCallFinish, RunToolsToolCallStart, StartCallback, StepCallback,
    StepStartCallback, StopWhen, StreamEvent, TextStream, ToolCall, ToolCallFinishCallback,
    ToolCallStartCallback, ToolResult, Usage,
};

pub fn openai(model: impl Into<String>) -> Result<ModelRef, AiError> {
    ModelRef::openai(model)
}

pub fn anthropic(model: impl Into<String>) -> Result<ModelRef, AiError> {
    ModelRef::anthropic(model)
}

pub fn google(model: impl Into<String>) -> Result<ModelRef, AiError> {
    ModelRef::google(model)
}

pub fn openai_compatible(model: impl Into<String>) -> Result<ModelRef, AiError> {
    ModelRef::openai_compatible(model)
}
