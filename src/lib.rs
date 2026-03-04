pub mod client;
pub mod error;
pub mod provider;
pub mod stream;
pub mod tool;
pub mod types;

#[cfg(feature = "axum")]
pub mod axum_sse;

pub use client::{AiClient, AiClientBuilder};
pub use error::{AiError, AiErrorCode};
pub use tool::{Tool, ToolDescriptor, ToolExecError, ToolExecutor, ToolRegistry};
pub use types::{
    ContentPart, FinishReason, GenerateTextRequest, GenerateTextResponse, Message, MessageRole,
    ModelRef, ProviderKind, RunToolsRequest, RunToolsResponse, StreamEvent, TextStream, ToolCall,
    ToolResult, Usage,
};
