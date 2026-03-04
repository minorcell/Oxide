use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;
use tokio::time::sleep;

use crate::error::{AiError, AiErrorCode};
use crate::provider::ProviderAdapter;
use crate::tool::{ToolExecError, ToolRegistry};
use crate::types::{
    ContentPart, GenerateTextRequest, GenerateTextResponse, Message, MessageRole, ProviderKind,
    RunToolsRequest, RunToolsResponse, TextStream, ToolCall, ToolResult, Usage, validate_messages,
    validate_sampling,
};

const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com";
const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";

pub struct AiClientBuilder {
    openai_api_key: Option<String>,
    anthropic_api_key: Option<String>,
    openai_base_url: String,
    anthropic_base_url: String,
    timeout: Duration,
    max_retries: u8,
    default_max_steps: u8,
    user_agent: String,
    anthropic_api_version: String,
}

impl Default for AiClientBuilder {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            anthropic_api_key: None,
            openai_base_url: DEFAULT_OPENAI_BASE_URL.to_string(),
            anthropic_base_url: DEFAULT_ANTHROPIC_BASE_URL.to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 2,
            default_max_steps: 8,
            user_agent: format!("oxide-ai-sdk/{}", env!("CARGO_PKG_VERSION")),
            anthropic_api_version: "2023-06-01".to_string(),
        }
    }
}

impl AiClientBuilder {
    pub fn openai_api_key(mut self, key: impl Into<String>) -> Self {
        self.openai_api_key = Some(key.into());
        self
    }

    pub fn anthropic_api_key(mut self, key: impl Into<String>) -> Self {
        self.anthropic_api_key = Some(key.into());
        self
    }

    pub fn openai_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.openai_base_url = base_url.into();
        self
    }

    pub fn anthropic_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.anthropic_base_url = base_url.into();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_retries(mut self, retries: u8) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn default_max_steps(mut self, max_steps: u8) -> Self {
        self.default_max_steps = max_steps;
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    pub fn anthropic_api_version(mut self, version: impl Into<String>) -> Self {
        self.anthropic_api_version = version.into();
        self
    }

    pub fn build(self) -> Result<AiClient, AiError> {
        if self.default_max_steps == 0 || self.default_max_steps > 32 {
            return Err(AiError::new(
                AiErrorCode::InvalidRequest,
                "default_max_steps must be in 1..=32",
            ));
        }

        let _http = Arc::new(
            reqwest::Client::builder()
                .timeout(self.timeout)
                .user_agent(self.user_agent)
                .build()
                .map_err(|e| AiError::new(AiErrorCode::Transport, e.to_string()))?,
        );

        #[cfg(feature = "openai")]
        let openai_adapter = self.openai_api_key.clone().map(|key| {
            Arc::new(crate::provider::openai::OpenAiAdapter {
                base_url: self.openai_base_url.clone(),
                api_key: key,
                http: Arc::clone(&_http),
            }) as Arc<dyn ProviderAdapter>
        });
        #[cfg(not(feature = "openai"))]
        let openai_adapter = None;

        #[cfg(feature = "anthropic")]
        let anthropic_adapter = self.anthropic_api_key.clone().map(|key| {
            Arc::new(crate::provider::anthropic::AnthropicAdapter {
                base_url: self.anthropic_base_url.clone(),
                api_key: key,
                api_version: self.anthropic_api_version.clone(),
                http: Arc::clone(&_http),
            }) as Arc<dyn ProviderAdapter>
        });
        #[cfg(not(feature = "anthropic"))]
        let anthropic_adapter = None;

        Ok(AiClient {
            max_retries: self.max_retries,
            default_max_steps: self.default_max_steps,
            openai_adapter,
            anthropic_adapter,
        })
    }
}

pub struct AiClient {
    max_retries: u8,
    default_max_steps: u8,
    openai_adapter: Option<Arc<dyn ProviderAdapter>>,
    anthropic_adapter: Option<Arc<dyn ProviderAdapter>>,
}

impl AiClient {
    pub fn builder() -> AiClientBuilder {
        AiClientBuilder::default()
    }

    pub async fn generate_text(
        &self,
        req: GenerateTextRequest,
    ) -> Result<GenerateTextResponse, AiError> {
        validate_messages(&req.messages)?;
        validate_sampling(req.temperature, req.top_p)?;

        let adapter = self.adapter_for(req.model.provider)?;
        self.call_with_retry(|| async { adapter.generate_text(&req).await })
            .await
    }

    pub async fn stream_text(&self, req: GenerateTextRequest) -> Result<TextStream, AiError> {
        validate_messages(&req.messages)?;
        validate_sampling(req.temperature, req.top_p)?;

        let adapter = self.adapter_for(req.model.provider)?;
        self.call_with_retry(|| async { adapter.stream_text(&req).await })
            .await
    }

    pub async fn run_tools(&self, req: RunToolsRequest) -> Result<RunToolsResponse, AiError> {
        validate_messages(&req.messages)?;
        validate_sampling(req.temperature, None)?;

        let resolved_max_steps = req.max_steps.unwrap_or(self.default_max_steps);
        if !(1..=32).contains(&resolved_max_steps) {
            return Err(AiError::new(
                AiErrorCode::InvalidRequest,
                "max_steps must be in 1..=32",
            ));
        }

        let mut messages = req.messages.clone();
        let mut usage_total = Usage::default();

        if req.tools.is_empty() {
            let response = self
                .generate_text(GenerateTextRequest {
                    model: req.model,
                    messages: messages.clone(),
                    temperature: req.temperature,
                    top_p: None,
                    max_output_tokens: req.max_output_tokens,
                    stop_sequences: req.stop_sequences,
                    tools: None,
                })
                .await?;
            usage_total += response.usage.clone();
            messages.push(assistant_message_from_response(&response));
            return Ok(RunToolsResponse {
                output_text: response.output_text,
                steps: 1,
                transcript: messages,
                usage_total,
            });
        }

        let tool_registry = ToolRegistry::from_tools(req.tools.clone())?;
        let tool_descriptors = req
            .tools
            .iter()
            .map(|tool| tool.descriptor.clone())
            .collect::<Vec<_>>();

        for step in 1..=resolved_max_steps {
            let response = self
                .generate_text(GenerateTextRequest {
                    model: req.model.clone(),
                    messages: messages.clone(),
                    temperature: req.temperature,
                    top_p: None,
                    max_output_tokens: req.max_output_tokens,
                    stop_sequences: req.stop_sequences.clone(),
                    tools: Some(tool_descriptors.clone()),
                })
                .await?;
            usage_total += response.usage.clone();
            messages.push(assistant_message_from_response(&response));

            if response.tool_calls.is_empty() {
                return Ok(RunToolsResponse {
                    output_text: response.output_text,
                    steps: step,
                    transcript: messages,
                    usage_total,
                });
            }

            let mut tool_messages =
                execute_tool_calls(&tool_registry, &response.tool_calls).await?;
            messages.append(&mut tool_messages);
        }

        Err(AiError::new(
            AiErrorCode::MaxStepsExceeded,
            format!(
                "run_tools reached max_steps ({}) without final answer",
                resolved_max_steps
            ),
        ))
    }

    fn adapter_for(&self, provider: ProviderKind) -> Result<&dyn ProviderAdapter, AiError> {
        match provider {
            ProviderKind::OpenAi => self.openai_adapter.as_deref().ok_or_else(|| {
                AiError::new(
                    AiErrorCode::InvalidRequest,
                    "OpenAI adapter unavailable: missing API key or feature disabled",
                )
            }),
            ProviderKind::Anthropic => self.anthropic_adapter.as_deref().ok_or_else(|| {
                AiError::new(
                    AiErrorCode::InvalidRequest,
                    "Anthropic adapter unavailable: missing API key or feature disabled",
                )
            }),
        }
    }

    async fn call_with_retry<T, F, Fut>(&self, mut op: F) -> Result<T, AiError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, AiError>>,
    {
        let mut attempt = 0u8;
        loop {
            match op().await {
                Ok(v) => return Ok(v),
                Err(err) => {
                    if !err.retryable || attempt >= self.max_retries {
                        return Err(err);
                    }
                    attempt = attempt.saturating_add(1);
                    let delay = backoff_delay(attempt);
                    sleep(delay).await;
                }
            }
        }
    }
}

fn backoff_delay(attempt: u8) -> Duration {
    let base_ms = 200u64;
    let cap_ms = 2_000u64;
    let exp = 2u64.saturating_pow(attempt as u32);
    Duration::from_millis((base_ms.saturating_mul(exp)).min(cap_ms))
}

fn assistant_message_from_response(response: &GenerateTextResponse) -> Message {
    let mut parts = Vec::new();
    if !response.output_text.is_empty() {
        parts.push(ContentPart::Text(response.output_text.clone()));
    }
    for call in &response.tool_calls {
        parts.push(ContentPart::ToolCall(call.clone()));
    }
    if parts.is_empty() {
        parts.push(ContentPart::Text(String::new()));
    }
    Message {
        role: MessageRole::Assistant,
        parts,
        name: None,
    }
}

async fn execute_tool_calls(
    registry: &ToolRegistry,
    calls: &[ToolCall],
) -> Result<Vec<Message>, AiError> {
    let mut tasks = Vec::with_capacity(calls.len());
    for call in calls {
        let Some(registered) = registry.resolve(&call.tool_name) else {
            return Err(AiError::new(
                AiErrorCode::UnknownTool,
                format!("unknown tool `{}`", call.tool_name),
            ));
        };

        registered
            .validator
            .validate(&call.args_json)
            .map_err(|e| {
                AiError::new(
                    AiErrorCode::InvalidToolArgs,
                    format!(
                        "tool args for `{}` failed schema validation: {}",
                        call.tool_name, e
                    ),
                )
            })?;

        let executor = Arc::clone(&registered.tool.executor);
        let call_id = call.call_id.clone();
        let args_json = call.args_json.clone();
        tasks.push(async move {
            let result = executor.execute(args_json).await;
            (call_id, result)
        });
    }

    let results = join_all(tasks).await;
    let mut messages = Vec::with_capacity(results.len());
    for (call_id, result) in results {
        let (output_json, is_error) = match result {
            Ok(output_json) => (output_json, false),
            Err(ToolExecError::Execution(message)) => {
                (serde_json::json!({ "error": message }), true)
            }
            Err(ToolExecError::Timeout) => (serde_json::json!({ "error": "timeout" }), true),
        };

        messages.push(Message {
            role: MessageRole::Tool,
            parts: vec![ContentPart::ToolResult(ToolResult {
                call_id,
                output_json,
                is_error,
            })],
            name: None,
        });
    }

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::AiClient;

    #[tokio::test]
    async fn builder_defaults_are_valid() {
        let client = AiClient::builder().build();
        assert!(client.is_ok());
    }
}
