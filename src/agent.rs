use std::sync::Arc;

use crate::client::AiClient;
use crate::error::{AiError, AiErrorCode};
use crate::tool::Tool;
use crate::types::{
    FinishCallback, Message, ModelRef, PrepareStepCallback, RunToolsFinish, RunToolsPrepareStep,
    RunToolsPreparedStep, RunToolsRequest, RunToolsResponse, RunToolsStart, RunToolsStep,
    RunToolsStepStart, RunToolsToolCallFinish, RunToolsToolCallStart, StartCallback, StepCallback,
    StepStartCallback, StopWhen, ToolCallFinishCallback, ToolCallStartCallback,
};

#[derive(Debug, Clone)]
pub struct AgentCallPlan {
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub max_steps: Option<u8>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
}

#[derive(Clone)]
pub struct PrepareCallCallback {
    inner: Arc<dyn Fn(&AgentCallPlan) -> AgentCallPlan + Send + Sync>,
}

impl std::fmt::Debug for PrepareCallCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrepareCallCallback(<fn>)")
    }
}

impl PrepareCallCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&AgentCallPlan) -> AgentCallPlan + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn call(&self, event: &AgentCallPlan) -> AgentCallPlan {
        (self.inner)(event)
    }
}

pub struct Agent {
    client: Arc<AiClient>,
    model: ModelRef,
    instructions: Option<String>,
    tools: Vec<Tool>,
    max_steps: Option<u8>,
    temperature: Option<f32>,
    max_output_tokens: Option<u32>,
    stop_sequences: Vec<String>,
    prepare_call: Option<PrepareCallCallback>,
    prepare_step: Option<PrepareStepCallback>,
    on_start: Option<StartCallback>,
    on_step_start: Option<StepStartCallback>,
    on_tool_call_start: Option<ToolCallStartCallback>,
    on_tool_call_finish: Option<ToolCallFinishCallback>,
    on_step_finish: Option<StepCallback>,
    on_finish: Option<FinishCallback>,
    stop_when: Option<StopWhen>,
}

impl Agent {
    pub fn builder(client: AiClient) -> AgentBuilder {
        AgentBuilder::new(Arc::new(client))
    }

    pub fn builder_with_client(client: Arc<AiClient>) -> AgentBuilder {
        AgentBuilder::new(client)
    }

    pub fn model_id(&self) -> String {
        self.model.id()
    }

    pub async fn generate_prompt(
        &self,
        prompt: impl Into<String>,
    ) -> Result<RunToolsResponse, AiError> {
        let mut messages = Vec::new();
        if let Some(instructions) = &self.instructions {
            messages.push(Message::system_text(instructions.clone()));
        }
        messages.push(Message::user_text(prompt));
        self.generate(messages).await
    }

    pub async fn generate(&self, messages: Vec<Message>) -> Result<RunToolsResponse, AiError> {
        let mut call_plan = AgentCallPlan {
            model: self.model.clone(),
            messages,
            tools: self.tools.clone(),
            max_steps: self.max_steps,
            temperature: self.temperature,
            max_output_tokens: self.max_output_tokens,
            stop_sequences: self.stop_sequences.clone(),
        };
        if let Some(callback) = &self.prepare_call {
            call_plan = callback.call(&call_plan);
        }

        self.client
            .run_tools(RunToolsRequest {
                model: call_plan.model,
                messages: call_plan.messages,
                tools: call_plan.tools,
                max_steps: call_plan.max_steps,
                temperature: call_plan.temperature,
                max_output_tokens: call_plan.max_output_tokens,
                stop_sequences: call_plan.stop_sequences,
                prepare_step: self.prepare_step.clone(),
                on_start: self.on_start.clone(),
                on_step_start: self.on_step_start.clone(),
                on_tool_call_start: self.on_tool_call_start.clone(),
                on_tool_call_finish: self.on_tool_call_finish.clone(),
                on_step_finish: self.on_step_finish.clone(),
                on_finish: self.on_finish.clone(),
                stop_when: self.stop_when.clone(),
            })
            .await
    }
}

pub struct AgentBuilder {
    client: Arc<AiClient>,
    model: Option<ModelRef>,
    instructions: Option<String>,
    tools: Vec<Tool>,
    max_steps: Option<u8>,
    temperature: Option<f32>,
    max_output_tokens: Option<u32>,
    stop_sequences: Vec<String>,
    prepare_call: Option<PrepareCallCallback>,
    prepare_step: Option<PrepareStepCallback>,
    on_start: Option<StartCallback>,
    on_step_start: Option<StepStartCallback>,
    on_tool_call_start: Option<ToolCallStartCallback>,
    on_tool_call_finish: Option<ToolCallFinishCallback>,
    on_step_finish: Option<StepCallback>,
    on_finish: Option<FinishCallback>,
    stop_when: Option<StopWhen>,
}

impl AgentBuilder {
    fn new(client: Arc<AiClient>) -> Self {
        Self {
            client,
            model: None,
            instructions: None,
            tools: Vec::new(),
            max_steps: None,
            temperature: None,
            max_output_tokens: None,
            stop_sequences: Vec::new(),
            prepare_call: None,
            prepare_step: None,
            on_start: None,
            on_step_start: None,
            on_tool_call_start: None,
            on_tool_call_finish: None,
            on_step_finish: None,
            on_finish: None,
            stop_when: None,
        }
    }

    pub fn model(mut self, model: ModelRef) -> Self {
        self.model = Some(model);
        self
    }

    pub fn model_ref(self, model: ModelRef) -> Self {
        self.model(model)
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    pub fn tools(mut self, tools: impl IntoIterator<Item = Tool>) -> Self {
        self.tools.extend(tools);
        self
    }

    pub fn stop_when_step_count(mut self, max_steps: u8) -> Self {
        self.max_steps = Some(max_steps);
        self
    }

    pub fn max_steps(self, max_steps: u8) -> Self {
        self.stop_when_step_count(max_steps)
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn max_output_tokens(mut self, max_output_tokens: u32) -> Self {
        self.max_output_tokens = Some(max_output_tokens);
        self
    }

    pub fn stop_sequence(mut self, stop_sequence: impl Into<String>) -> Self {
        self.stop_sequences.push(stop_sequence.into());
        self
    }

    pub fn stop_sequences(mut self, stop_sequences: impl IntoIterator<Item = String>) -> Self {
        self.stop_sequences.extend(stop_sequences);
        self
    }

    pub fn prepare_call<F>(mut self, callback: F) -> Self
    where
        F: Fn(&AgentCallPlan) -> AgentCallPlan + Send + Sync + 'static,
    {
        self.prepare_call = Some(PrepareCallCallback::new(callback));
        self
    }

    pub fn prepare_step<F>(mut self, callback: F) -> Self
    where
        F: Fn(&RunToolsPrepareStep) -> RunToolsPreparedStep + Send + Sync + 'static,
    {
        self.prepare_step = Some(PrepareStepCallback::new(callback));
        self
    }

    pub fn on_step_finish<F>(mut self, callback: F) -> Self
    where
        F: Fn(&RunToolsStep) + Send + Sync + 'static,
    {
        self.on_step_finish = Some(StepCallback::new(callback));
        self
    }

    pub fn on_start<F>(mut self, callback: F) -> Self
    where
        F: Fn(&RunToolsStart) + Send + Sync + 'static,
    {
        self.on_start = Some(StartCallback::new(callback));
        self
    }

    pub fn on_step_start<F>(mut self, callback: F) -> Self
    where
        F: Fn(&RunToolsStepStart) + Send + Sync + 'static,
    {
        self.on_step_start = Some(StepStartCallback::new(callback));
        self
    }

    pub fn on_tool_call_start<F>(mut self, callback: F) -> Self
    where
        F: Fn(&RunToolsToolCallStart) + Send + Sync + 'static,
    {
        self.on_tool_call_start = Some(ToolCallStartCallback::new(callback));
        self
    }

    pub fn on_tool_call_finish<F>(mut self, callback: F) -> Self
    where
        F: Fn(&RunToolsToolCallFinish) + Send + Sync + 'static,
    {
        self.on_tool_call_finish = Some(ToolCallFinishCallback::new(callback));
        self
    }

    pub fn on_finish<F>(mut self, callback: F) -> Self
    where
        F: Fn(&RunToolsFinish) + Send + Sync + 'static,
    {
        self.on_finish = Some(FinishCallback::new(callback));
        self
    }

    pub fn stop_when<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&RunToolsStep) -> bool + Send + Sync + 'static,
    {
        self.stop_when = Some(StopWhen::new(predicate));
        self
    }

    pub fn build(self) -> Result<Agent, AiError> {
        let Some(model) = self.model else {
            return Err(AiError::new(
                AiErrorCode::InvalidRequest,
                "agent model is required; call .model(openai(\"gpt-4o-mini\")?)",
            ));
        };

        if let Some(max_steps) = self.max_steps {
            if !(1..=32).contains(&max_steps) {
                return Err(AiError::new(
                    AiErrorCode::InvalidRequest,
                    "max_steps must be in 1..=32",
                ));
            }
        }

        Ok(Agent {
            client: self.client,
            model,
            instructions: self.instructions,
            tools: self.tools,
            max_steps: self.max_steps,
            temperature: self.temperature,
            max_output_tokens: self.max_output_tokens,
            stop_sequences: self.stop_sequences,
            prepare_call: self.prepare_call,
            prepare_step: self.prepare_step,
            on_start: self.on_start,
            on_step_start: self.on_step_start,
            on_tool_call_start: self.on_tool_call_start,
            on_tool_call_finish: self.on_tool_call_finish,
            on_step_finish: self.on_step_finish,
            on_finish: self.on_finish,
            stop_when: self.stop_when,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Agent;
    use crate::{AiClient, AiErrorCode, openai};

    #[test]
    fn builder_requires_model() {
        let client = AiClient::builder()
            .with_openai("test-key", "https://api.openai.com")
            .build()
            .expect("client should build");
        let err = match Agent::builder(client).build() {
            Ok(_) => panic!("missing model should fail"),
            Err(err) => err,
        };
        assert_eq!(err.code, AiErrorCode::InvalidRequest);
    }

    #[test]
    fn builder_rejects_invalid_max_steps() {
        let client = AiClient::builder()
            .with_openai("test-key", "https://api.openai.com")
            .build()
            .expect("client should build");
        let err = match Agent::builder(client)
            .model(openai("gpt-4o-mini").expect("model should parse"))
            .max_steps(0)
            .build()
        {
            Ok(_) => panic!("invalid max_steps should fail"),
            Err(err) => err,
        };
        assert_eq!(err.code, AiErrorCode::InvalidRequest);
    }

    #[test]
    fn builder_accepts_model_ref() {
        let client = AiClient::builder()
            .with_openai("test-key", "https://api.openai.com")
            .build()
            .expect("client should build");
        let agent = Agent::builder(client)
            .model(openai("gpt-4o-mini").expect("model should parse"))
            .build()
            .expect("agent should build");

        assert_eq!(agent.model_id(), "openai/gpt-4o-mini");
    }
}
