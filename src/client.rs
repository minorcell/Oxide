use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use tokio::time::sleep;

use crate::error::{AiError, AiErrorCode};
use crate::model_adapters::ModelAdapter;
use crate::model_adapters::anthropic::{AnthropicAdapter, AnthropicAdapterSettings};
use crate::model_adapters::google::{GoogleAdapter, GoogleAdapterSettings};
use crate::model_adapters::openai::{OpenAiAdapter, OpenAiAdapterSettings};
use crate::model_adapters::openai_compatible::{
    OpenAiCompatibleAdapter, OpenAiCompatibleAdapterSettings,
};
use crate::tool::{ToolExecError, ToolRegistry};
use crate::types::{
    ContentPart, FinishCallback, FinishReason, GenerateTextRequest, GenerateTextResponse, Message,
    MessageRole, ModelRef, ProviderKind, RunToolsFinish, RunToolsPrepareStep, RunToolsPreparedStep,
    RunToolsRequest, RunToolsResponse, RunToolsStart, RunToolsStep, RunToolsStepStart,
    RunToolsToolCallFinish, RunToolsToolCallStart, TextStream, ToolCall, ToolCallFinishCallback,
    ToolCallStartCallback, ToolResult, Usage, validate_messages, validate_sampling,
};

enum ProviderRegistration {
    OpenAi(OpenAiAdapterSettings),
    Anthropic(AnthropicAdapterSettings),
    Google(GoogleAdapterSettings),
    OpenAiCompatible(OpenAiCompatibleAdapterSettings),
    Custom {
        provider: ProviderKind,
        adapter: Arc<dyn ModelAdapter>,
    },
}

pub struct AiClientBuilder {
    timeout: Duration,
    max_retries: u8,
    default_max_steps: u8,
    user_agent: String,
    registration: Option<ProviderRegistration>,
}

impl Default for AiClientBuilder {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_retries: 2,
            default_max_steps: 8,
            user_agent: format!("oxide-ai-sdk/{}", env!("CARGO_PKG_VERSION")),
            registration: None,
        }
    }
}

impl AiClientBuilder {
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

    pub fn with_openai(mut self, api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let mut settings = OpenAiAdapterSettings::new(api_key);
        settings.base_url = base_url.into();
        self.registration = Some(ProviderRegistration::OpenAi(settings));
        self
    }

    pub fn with_anthropic(
        mut self,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        api_version: impl Into<String>,
    ) -> Self {
        let mut settings = AnthropicAdapterSettings::new(api_key);
        settings.base_url = base_url.into();
        settings.api_version = api_version.into();
        self.registration = Some(ProviderRegistration::Anthropic(settings));
        self
    }

    pub fn with_google(mut self, api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let mut settings = GoogleAdapterSettings::new(api_key);
        settings.base_url = base_url.into();
        self.registration = Some(ProviderRegistration::Google(settings));
        self
    }

    pub fn with_openai_compatible(
        mut self,
        base_url: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        let mut settings = OpenAiCompatibleAdapterSettings::new(base_url);
        settings.api_key = api_key;
        self.registration = Some(ProviderRegistration::OpenAiCompatible(settings));
        self
    }

    pub fn with_openai_compatible_settings(
        mut self,
        settings: OpenAiCompatibleAdapterSettings,
    ) -> Self {
        self.registration = Some(ProviderRegistration::OpenAiCompatible(settings));
        self
    }

    pub fn with_adapter(mut self, provider: ProviderKind, adapter: Arc<dyn ModelAdapter>) -> Self {
        self.registration = Some(ProviderRegistration::Custom { provider, adapter });
        self
    }

    pub fn build(self) -> Result<AiClient, AiError> {
        let AiClientBuilder {
            timeout,
            max_retries,
            default_max_steps,
            user_agent,
            registration,
        } = self;

        if default_max_steps == 0 || default_max_steps > 32 {
            return Err(AiError::new(
                AiErrorCode::InvalidRequest,
                "default_max_steps must be in 1..=32",
            ));
        }

        let registration = registration.ok_or_else(|| {
            AiError::new(
                AiErrorCode::InvalidRequest,
                "provider is required; call one of `.with_openai(...)`, `.with_anthropic(...)`, `.with_google(...)`, `.with_openai_compatible(...)`",
            )
        })?;

        let http = Arc::new(
            reqwest::Client::builder()
                .timeout(timeout)
                .user_agent(user_agent)
                .build()
                .map_err(|e| AiError::new(AiErrorCode::Transport, e.to_string()))?,
        );

        let (provider, adapter): (ProviderKind, Arc<dyn ModelAdapter>) = match registration {
            ProviderRegistration::OpenAi(settings) => (
                ProviderKind::OpenAi,
                Arc::new(OpenAiAdapter::from_settings(settings, Arc::clone(&http))),
            ),
            ProviderRegistration::Anthropic(settings) => (
                ProviderKind::Anthropic,
                Arc::new(AnthropicAdapter::from_settings(settings, Arc::clone(&http))),
            ),
            ProviderRegistration::Google(settings) => (
                ProviderKind::Google,
                Arc::new(GoogleAdapter::from_settings(settings, Arc::clone(&http))),
            ),
            ProviderRegistration::OpenAiCompatible(settings) => (
                ProviderKind::OpenAiCompatible,
                Arc::new(OpenAiCompatibleAdapter::from_settings(
                    settings,
                    Arc::clone(&http),
                )),
            ),
            ProviderRegistration::Custom { provider, adapter } => (provider, adapter),
        };

        Ok(AiClient {
            max_retries,
            default_max_steps,
            provider,
            adapter,
        })
    }
}

pub struct AiClient {
    max_retries: u8,
    default_max_steps: u8,
    provider: ProviderKind,
    adapter: Arc<dyn ModelAdapter>,
}

impl AiClient {
    pub fn builder() -> AiClientBuilder {
        AiClientBuilder::default()
    }

    pub async fn generate_prompt(
        &self,
        model: ModelRef,
        prompt: impl Into<String>,
    ) -> Result<GenerateTextResponse, AiError> {
        let req = GenerateTextRequest::from_user_prompt(model, prompt);
        self.generate_text(req).await
    }

    pub async fn stream_prompt(
        &self,
        model: ModelRef,
        prompt: impl Into<String>,
    ) -> Result<TextStream, AiError> {
        let req = GenerateTextRequest::from_user_prompt(model, prompt);
        self.stream_text(req).await
    }

    pub async fn generate_text(
        &self,
        req: GenerateTextRequest,
    ) -> Result<GenerateTextResponse, AiError> {
        validate_messages(&req.messages)?;
        validate_sampling(req.temperature, req.top_p)?;

        let adapter = self.adapter_for_model(&req.model)?;
        self.call_with_retry(|| async { adapter.generate_text(&req).await })
            .await
    }

    pub async fn stream_text(&self, req: GenerateTextRequest) -> Result<TextStream, AiError> {
        validate_messages(&req.messages)?;
        validate_sampling(req.temperature, req.top_p)?;

        let adapter = self.adapter_for_model(&req.model)?;
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
        let mut step_results = Vec::new();

        if let Some(callback) = &req.on_start {
            callback.call(&RunToolsStart {
                model: req.model.clone(),
                messages: messages.clone(),
                tool_count: req.tools.len(),
                max_steps: resolved_max_steps,
            });
        }

        for step in 1..=resolved_max_steps {
            let mut prepared_step = RunToolsPreparedStep {
                model: req.model.clone(),
                messages: messages.clone(),
                tools: req.tools.clone(),
                temperature: req.temperature,
                max_output_tokens: req.max_output_tokens,
                stop_sequences: req.stop_sequences.clone(),
            };
            if let Some(callback) = &req.prepare_step {
                prepared_step = callback.call(&RunToolsPrepareStep {
                    step,
                    model: req.model.clone(),
                    messages: messages.clone(),
                    tools: req.tools.clone(),
                    temperature: req.temperature,
                    max_output_tokens: req.max_output_tokens,
                    stop_sequences: req.stop_sequences.clone(),
                    previous_steps: step_results.clone(),
                });
            }

            validate_messages(&prepared_step.messages)?;
            validate_sampling(prepared_step.temperature, None)?;

            if let Some(callback) = &req.on_step_start {
                callback.call(&RunToolsStepStart {
                    step,
                    messages: prepared_step.messages.clone(),
                });
            }

            let response = self
                .generate_text(GenerateTextRequest {
                    model: prepared_step.model.clone(),
                    messages: prepared_step.messages.clone(),
                    temperature: prepared_step.temperature,
                    top_p: None,
                    max_output_tokens: prepared_step.max_output_tokens,
                    stop_sequences: prepared_step.stop_sequences.clone(),
                    tools: if prepared_step.tools.is_empty() {
                        None
                    } else {
                        Some(
                            prepared_step
                                .tools
                                .iter()
                                .map(|tool| tool.descriptor.clone())
                                .collect(),
                        )
                    },
                })
                .await?;
            usage_total += response.usage.clone();
            let mut next_messages = prepared_step.messages.clone();
            next_messages.push(assistant_message_from_response(&response));

            if response.tool_calls.is_empty() {
                let step_state = RunToolsStep {
                    step,
                    output_text: response.output_text.clone(),
                    finish_reason: response.finish_reason.clone(),
                    usage: response.usage.clone(),
                    tool_calls: Vec::new(),
                    tool_results: Vec::new(),
                };
                step_results.push(step_state.clone());
                if let Some(callback) = &req.on_step_finish {
                    callback.call(&step_state);
                }
                let final_response = RunToolsResponse {
                    output_text: response.output_text,
                    steps: step,
                    transcript: next_messages,
                    usage_total,
                };
                emit_on_finish(
                    req.on_finish.as_ref(),
                    &final_response,
                    &step_state.finish_reason,
                    &step_results,
                );
                return Ok(final_response);
            }

            let tool_registry = ToolRegistry::from_tools(prepared_step.tools.clone())?;
            let executed_tool_calls = execute_tool_calls(
                &tool_registry,
                &response.tool_calls,
                step,
                req.on_tool_call_start.as_ref(),
                req.on_tool_call_finish.as_ref(),
            )
            .await?;
            let mut tool_messages = executed_tool_calls
                .iter()
                .map(|entry| Message {
                    role: MessageRole::Tool,
                    parts: vec![ContentPart::ToolResult(entry.result.clone())],
                    name: None,
                })
                .collect::<Vec<_>>();
            let step_state = RunToolsStep {
                step,
                output_text: response.output_text.clone(),
                finish_reason: response.finish_reason.clone(),
                usage: response.usage.clone(),
                tool_calls: response.tool_calls.clone(),
                tool_results: executed_tool_calls
                    .iter()
                    .map(|entry| entry.result.clone())
                    .collect(),
            };
            step_results.push(step_state.clone());
            next_messages.append(&mut tool_messages);
            if let Some(callback) = &req.on_step_finish {
                callback.call(&step_state);
            }
            if req
                .stop_when
                .as_ref()
                .is_some_and(|predicate| predicate.should_stop(&step_state))
            {
                let final_response = RunToolsResponse {
                    output_text: response.output_text,
                    steps: step,
                    transcript: next_messages,
                    usage_total,
                };
                emit_on_finish(
                    req.on_finish.as_ref(),
                    &final_response,
                    &step_state.finish_reason,
                    &step_results,
                );
                return Ok(final_response);
            }

            messages = next_messages;
        }

        Err(AiError::new(
            AiErrorCode::MaxStepsExceeded,
            format!(
                "run_tools reached max_steps ({}) without final answer",
                resolved_max_steps
            ),
        ))
    }

    fn adapter_for_model(&self, model: &ModelRef) -> Result<&dyn ModelAdapter, AiError> {
        let requested_provider = model.provider_kind().ok_or_else(|| {
            AiError::new(
                AiErrorCode::InvalidRequest,
                format!("unknown provider `{}`", model.provider()),
            )
        })?;

        if self.provider != requested_provider {
            return Err(AiError::new(
                AiErrorCode::InvalidRequest,
                format!(
                    "provider mismatch: this client is configured for `{}`, but model requires `{}`",
                    self.provider.as_slug(),
                    requested_provider.as_slug()
                ),
            ));
        }

        Ok(self.adapter.as_ref())
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

fn emit_on_finish(
    callback: Option<&FinishCallback>,
    response: &RunToolsResponse,
    finish_reason: &FinishReason,
    step_results: &[RunToolsStep],
) {
    let Some(callback) = callback else {
        return;
    };

    callback.call(&RunToolsFinish {
        output_text: response.output_text.clone(),
        step_count: response.steps,
        finish_reason: finish_reason.clone(),
        usage_total: response.usage_total.clone(),
        transcript: response.transcript.clone(),
        step_results: step_results.to_vec(),
    });
}

#[derive(Debug, Clone)]
struct ExecutedToolCall {
    result: ToolResult,
}

async fn execute_tool_calls(
    registry: &ToolRegistry,
    calls: &[ToolCall],
    step: u8,
    on_tool_call_start: Option<&ToolCallStartCallback>,
    on_tool_call_finish: Option<&ToolCallFinishCallback>,
) -> Result<Vec<ExecutedToolCall>, AiError> {
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

        if let Some(callback) = on_tool_call_start {
            callback.call(&RunToolsToolCallStart {
                step,
                tool_call: call.clone(),
            });
        }

        let executor = Arc::clone(&registered.tool.executor);
        let call = call.clone();
        let args_json = call.args_json.clone();
        tasks.push(async move {
            let started_at = Instant::now();
            let result = executor.execute(args_json).await;
            let duration_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
            (call, result, duration_ms)
        });
    }

    let results = join_all(tasks).await;
    let mut executions = Vec::with_capacity(results.len());
    for (call, result, duration_ms) in results {
        let (output_json, is_error) = match result {
            Ok(output_json) => (output_json, false),
            Err(ToolExecError::Execution(message)) => {
                (serde_json::json!({ "error": message }), true)
            }
            Err(ToolExecError::Timeout) => (serde_json::json!({ "error": "timeout" }), true),
        };

        let tool_result = ToolResult {
            call_id: call.call_id.clone(),
            output_json,
            is_error,
        };

        if let Some(callback) = on_tool_call_finish {
            callback.call(&RunToolsToolCallFinish {
                step,
                tool_call: call.clone(),
                tool_result: tool_result.clone(),
                duration_ms,
            });
        }

        executions.push(ExecutedToolCall {
            result: tool_result,
        });
    }

    Ok(executions)
}

#[cfg(test)]
mod tests {
    use super::AiClient;
    use crate::{AiErrorCode, openai};

    #[tokio::test]
    async fn build_requires_provider_registration() {
        let err = match AiClient::builder().build() {
            Ok(_) => panic!("provider registration should be required"),
            Err(err) => err,
        };
        assert_eq!(err.code, AiErrorCode::InvalidRequest);
        assert!(err.message.contains("provider is required"));
    }

    #[tokio::test]
    async fn rejects_provider_mismatch() {
        let client = AiClient::builder()
            .with_google("key", "https://generativelanguage.googleapis.com/v1beta")
            .build()
            .expect("client should build");
        let err = client
            .generate_prompt(openai("gpt-4o-mini").expect("model should parse"), "hello")
            .await
            .expect_err("provider mismatch should fail");

        assert_eq!(err.code, AiErrorCode::InvalidRequest);
        assert!(err.message.contains("provider mismatch"));
    }
}
