use std::pin::Pin;
use std::sync::Arc;

use futures_core::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AiError;
use crate::tool::{Tool, ToolDescriptor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Google,
    OpenAiCompatible,
}

impl ProviderKind {
    pub fn from_slug(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAi),
            "anthropic" => Some(Self::Anthropic),
            "google" => Some(Self::Google),
            "openai-compatible" => Some(Self::OpenAiCompatible),
            _ => None,
        }
    }

    pub fn as_slug(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Google => "google",
            Self::OpenAiCompatible => "openai-compatible",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    provider: String,
    model: String,
}

impl ModelRef {
    pub fn from_kind(provider: ProviderKind, model: impl Into<String>) -> Result<Self, AiError> {
        let model = model.into();
        if model.trim().is_empty() {
            return Err(AiError::new(
                crate::error::AiErrorCode::InvalidRequest,
                "model name cannot be empty",
            ));
        }

        Ok(Self {
            provider: provider.as_slug().to_string(),
            model,
        })
    }

    pub fn openai(model: impl Into<String>) -> Result<Self, AiError> {
        Self::from_kind(ProviderKind::OpenAi, model)
    }

    pub fn anthropic(model: impl Into<String>) -> Result<Self, AiError> {
        Self::from_kind(ProviderKind::Anthropic, model)
    }

    pub fn google(model: impl Into<String>) -> Result<Self, AiError> {
        Self::from_kind(ProviderKind::Google, model)
    }

    pub fn openai_compatible(model: impl Into<String>) -> Result<Self, AiError> {
        Self::from_kind(ProviderKind::OpenAiCompatible, model)
    }

    pub fn id(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn provider_kind(&self) -> Option<ProviderKind> {
        ProviderKind::from_slug(&self.provider)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub parts: Vec<ContentPart>,
    pub name: Option<String>,
}

impl Message {
    pub fn system_text(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            parts: vec![ContentPart::Text(text.into())],
            name: None,
        }
    }

    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            parts: vec![ContentPart::Text(text.into())],
            name: None,
        }
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            parts: vec![ContentPart::Text(text.into())],
            name: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentPart {
    Text(String),
    ToolCall(ToolCall),
    ToolResult(ToolResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub tool_name: String,
    pub args_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub output_json: Value,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateTextRequest {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
    pub tools: Option<Vec<ToolDescriptor>>,
}

impl GenerateTextRequest {
    pub fn from_user_prompt(model: ModelRef, prompt: impl Into<String>) -> Self {
        Self {
            model,
            messages: vec![Message::user_text(prompt)],
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            stop_sequences: vec![],
            tools: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunToolsRequest {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub max_steps: Option<u8>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
    pub prepare_step: Option<PrepareStepCallback>,
    pub on_start: Option<StartCallback>,
    pub on_step_start: Option<StepStartCallback>,
    pub on_tool_call_start: Option<ToolCallStartCallback>,
    pub on_tool_call_finish: Option<ToolCallFinishCallback>,
    pub on_step_finish: Option<StepCallback>,
    pub on_finish: Option<FinishCallback>,
    pub stop_when: Option<StopWhen>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunToolsStart {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tool_count: usize,
    pub max_steps: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunToolsStepStart {
    pub step: u8,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunToolsToolCallStart {
    pub step: u8,
    pub tool_call: ToolCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunToolsToolCallFinish {
    pub step: u8,
    pub tool_call: ToolCall,
    pub tool_result: ToolResult,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunToolsStep {
    pub step: u8,
    pub output_text: String,
    pub finish_reason: FinishReason,
    pub usage: Usage,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunToolsFinish {
    pub output_text: String,
    pub step_count: u8,
    pub finish_reason: FinishReason,
    pub usage_total: Usage,
    pub transcript: Vec<Message>,
    pub step_results: Vec<RunToolsStep>,
}

#[derive(Debug, Clone)]
pub struct RunToolsPrepareStep {
    pub step: u8,
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
    pub previous_steps: Vec<RunToolsStep>,
}

#[derive(Debug, Clone)]
pub struct RunToolsPreparedStep {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
}

#[derive(Clone)]
pub struct PrepareStepCallback {
    inner: Arc<dyn Fn(&RunToolsPrepareStep) -> RunToolsPreparedStep + Send + Sync>,
}

impl std::fmt::Debug for PrepareStepCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrepareStepCallback(<fn>)")
    }
}

impl PrepareStepCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&RunToolsPrepareStep) -> RunToolsPreparedStep + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn call(&self, event: &RunToolsPrepareStep) -> RunToolsPreparedStep {
        (self.inner)(event)
    }
}

#[derive(Clone)]
pub struct StartCallback {
    inner: Arc<dyn Fn(&RunToolsStart) + Send + Sync>,
}

impl std::fmt::Debug for StartCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StartCallback(<fn>)")
    }
}

impl StartCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&RunToolsStart) + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn call(&self, event: &RunToolsStart) {
        (self.inner)(event);
    }
}

#[derive(Clone)]
pub struct StepStartCallback {
    inner: Arc<dyn Fn(&RunToolsStepStart) + Send + Sync>,
}

impl std::fmt::Debug for StepStartCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StepStartCallback(<fn>)")
    }
}

impl StepStartCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&RunToolsStepStart) + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn call(&self, event: &RunToolsStepStart) {
        (self.inner)(event);
    }
}

#[derive(Clone)]
pub struct ToolCallStartCallback {
    inner: Arc<dyn Fn(&RunToolsToolCallStart) + Send + Sync>,
}

impl std::fmt::Debug for ToolCallStartCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ToolCallStartCallback(<fn>)")
    }
}

impl ToolCallStartCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&RunToolsToolCallStart) + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn call(&self, event: &RunToolsToolCallStart) {
        (self.inner)(event);
    }
}

#[derive(Clone)]
pub struct ToolCallFinishCallback {
    inner: Arc<dyn Fn(&RunToolsToolCallFinish) + Send + Sync>,
}

impl std::fmt::Debug for ToolCallFinishCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ToolCallFinishCallback(<fn>)")
    }
}

impl ToolCallFinishCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&RunToolsToolCallFinish) + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn call(&self, event: &RunToolsToolCallFinish) {
        (self.inner)(event);
    }
}

#[derive(Clone)]
pub struct StepCallback {
    inner: Arc<dyn Fn(&RunToolsStep) + Send + Sync>,
}

impl std::fmt::Debug for StepCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StepCallback(<fn>)")
    }
}

impl StepCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&RunToolsStep) + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn call(&self, step: &RunToolsStep) {
        (self.inner)(step);
    }
}

#[derive(Clone)]
pub struct FinishCallback {
    inner: Arc<dyn Fn(&RunToolsFinish) + Send + Sync>,
}

impl std::fmt::Debug for FinishCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FinishCallback(<fn>)")
    }
}

impl FinishCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&RunToolsFinish) + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn call(&self, event: &RunToolsFinish) {
        (self.inner)(event);
    }
}

#[derive(Clone)]
pub struct StopWhen {
    inner: Arc<dyn Fn(&RunToolsStep) -> bool + Send + Sync>,
}

impl std::fmt::Debug for StopWhen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StopWhen(<fn>)")
    }
}

impl StopWhen {
    pub fn new<F>(predicate: F) -> Self
    where
        F: Fn(&RunToolsStep) -> bool + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(predicate),
        }
    }

    pub fn should_stop(&self, step: &RunToolsStep) -> bool {
        (self.inner)(step)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateTextResponse {
    pub output_text: String,
    pub finish_reason: FinishReason,
    pub usage: Usage,
    pub tool_calls: Vec<ToolCall>,
    pub raw_provider_response: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunToolsResponse {
    pub output_text: String,
    pub steps: u8,
    pub transcript: Vec<Message>,
    pub usage_total: Usage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Unknown(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

impl std::ops::Add for Usage {
    type Output = Usage;

    fn add(self, rhs: Self) -> Self::Output {
        Usage {
            input_tokens: self.input_tokens.saturating_add(rhs.input_tokens),
            output_tokens: self.output_tokens.saturating_add(rhs.output_tokens),
            total_tokens: self.total_tokens.saturating_add(rhs.total_tokens),
        }
    }
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, rhs: Self) {
        self.input_tokens = self.input_tokens.saturating_add(rhs.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(rhs.output_tokens);
        self.total_tokens = self.total_tokens.saturating_add(rhs.total_tokens);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    TextDelta { text: String },
    ToolCallReady { call: ToolCall },
    Usage { usage: Usage },
    Done,
}

pub type TextStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, AiError>> + Send>>;

pub(crate) fn validate_messages(messages: &[Message]) -> Result<(), AiError> {
    if messages.is_empty() {
        return Err(AiError::new(
            crate::error::AiErrorCode::InvalidRequest,
            "messages cannot be empty",
        ));
    }
    for msg in messages {
        if msg.parts.is_empty() {
            return Err(AiError::new(
                crate::error::AiErrorCode::InvalidRequest,
                "message parts cannot be empty",
            ));
        }
        if msg.role == MessageRole::Tool
            && !msg
                .parts
                .iter()
                .any(|p| matches!(p, ContentPart::ToolResult(_)))
        {
            return Err(AiError::new(
                crate::error::AiErrorCode::InvalidRequest,
                "tool role message must include a ToolResult part",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_sampling(
    temperature: Option<f32>,
    top_p: Option<f32>,
) -> Result<(), AiError> {
    if let Some(temp) = temperature {
        if !(0.0..=2.0).contains(&temp) {
            return Err(AiError::new(
                crate::error::AiErrorCode::InvalidRequest,
                "temperature must be within 0.0..=2.0",
            ));
        }
    }
    if let Some(p) = top_p {
        if !(0.0..=1.0).contains(&p) {
            return Err(AiError::new(
                crate::error::AiErrorCode::InvalidRequest,
                "top_p must be within 0.0..=1.0",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{GenerateTextRequest, ModelRef, ProviderKind};

    #[test]
    fn builds_openai_model() {
        let model = ModelRef::openai("gpt-4o-mini").expect("model should parse");
        assert_eq!(model.provider(), "openai");
        assert_eq!(model.model(), "gpt-4o-mini");
    }

    #[test]
    fn rejects_empty_model_name() {
        let err = ModelRef::openai("  ").expect_err("empty model should fail");
        assert!(
            err.message.contains("cannot be empty"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn builds_request_from_prompt() {
        let request = GenerateTextRequest::from_user_prompt(
            ModelRef::from_kind(ProviderKind::Anthropic, "claude-3-5-haiku-latest")
                .expect("model should parse"),
            "hello",
        );

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.model.provider(), "anthropic");
        assert_eq!(request.model.model(), "claude-3-5-haiku-latest");
    }

    #[test]
    fn accepts_google_model() {
        let model = ModelRef::google("gemini-2.0-flash").expect("model should parse");
        assert_eq!(model.provider(), "google");
    }
}
