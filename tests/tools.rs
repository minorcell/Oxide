use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use oxide::{
    AiClient, AiErrorCode, ContentPart, FinishCallback, Message, MessageRole, PrepareStepCallback,
    RunToolsPreparedStep, RunToolsRequest, StartCallback, StepCallback, StepStartCallback, Tool,
    ToolCallFinishCallback, ToolCallStartCallback, ToolDescriptor, ToolExecError, ToolExecutor,
    openai,
};
use serde_json::{Value, json};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct DummyWeatherTool;

#[async_trait]
impl ToolExecutor for DummyWeatherTool {
    async fn execute(&self, args: Value) -> Result<Value, ToolExecError> {
        let city = args
            .get("city")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        Ok(json!({ "city": city, "temp_c": 22 }))
    }
}

fn make_weather_tool() -> Tool {
    Tool {
        descriptor: ToolDescriptor {
            name: "get_weather".to_string(),
            description: "Get weather by city".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }),
        },
        executor: Arc::new(DummyWeatherTool),
    }
}

fn tool_request(tools: Vec<Tool>) -> RunToolsRequest {
    RunToolsRequest {
        model: openai("gpt-4o-mini").expect("model should parse"),
        messages: vec![Message {
            role: MessageRole::User,
            parts: vec![ContentPart::Text(
                "What's the weather in Shanghai?".to_string(),
            )],
            name: None,
        }],
        tools,
        max_steps: Some(3),
        temperature: Some(0.2),
        max_output_tokens: Some(256),
        stop_sequences: vec![],
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

#[tokio::test]
async fn run_tools_two_step_success() {
    let server = MockServer::start().await;

    let step1 = json!({
        "choices": [{
            "message": {
                "content": "",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "arguments": "{\"city\":\"Shanghai\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });

    let step2 = json!({
        "choices": [{
            "message": { "content": "Shanghai is about 22C." },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 8,
            "completion_tokens": 4,
            "total_tokens": 12
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("\"role\":\"tool\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(step2))
        .expect(1)
        .with_priority(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(step1))
        .expect(1)
        .with_priority(5)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .with_openai("test-openai-key", server.uri())
        .build()
        .expect("client should build");

    let response = client
        .run_tools(tool_request(vec![make_weather_tool()]))
        .await
        .expect("run_tools should succeed");

    assert_eq!(response.output_text, "Shanghai is about 22C.");
    assert_eq!(response.steps, 2);
    assert_eq!(response.usage_total.total_tokens, 27);
    assert!(response.transcript.len() >= 4);
}

#[tokio::test]
async fn run_tools_unknown_tool_fails() {
    let server = MockServer::start().await;
    let step1 = json!({
        "choices": [{
            "message": {
                "content": "",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "missing_tool",
                        "arguments": "{\"city\":\"Shanghai\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(step1))
        .expect(1)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .with_openai("test-openai-key", server.uri())
        .build()
        .expect("client should build");

    let err = client
        .run_tools(tool_request(vec![make_weather_tool()]))
        .await
        .expect_err("run_tools should fail for unknown tool");

    assert_eq!(err.code, AiErrorCode::UnknownTool);
}

#[tokio::test]
async fn run_tools_lifecycle_hooks_fire() {
    let server = MockServer::start().await;

    let step1 = json!({
        "choices": [{
            "message": {
                "content": "",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "arguments": "{\"city\":\"Shanghai\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });

    let step2 = json!({
        "choices": [{
            "message": { "content": "Shanghai is about 22C." },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 8,
            "completion_tokens": 4,
            "total_tokens": 12
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("\"role\":\"tool\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(step2))
        .expect(1)
        .with_priority(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(step1))
        .expect(1)
        .with_priority(5)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .with_openai("test-openai-key", server.uri())
        .build()
        .expect("client should build");

    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    fn push_event(events: &Arc<Mutex<Vec<String>>>, label: String) {
        events
            .lock()
            .expect("events mutex should not be poisoned")
            .push(label);
    }

    let mut request = tool_request(vec![make_weather_tool()]);
    {
        let events = Arc::clone(&events);
        request.on_start = Some(StartCallback::new(move |_| {
            push_event(&events, "start".to_string())
        }));
    }
    {
        let events = Arc::clone(&events);
        request.on_step_start = Some(StepStartCallback::new(move |event| {
            push_event(&events, format!("step_start:{}", event.step))
        }));
    }
    {
        let events = Arc::clone(&events);
        request.on_tool_call_start = Some(ToolCallStartCallback::new(move |event| {
            push_event(
                &events,
                format!("tool_call_start:{}", event.tool_call.tool_name),
            )
        }));
    }
    {
        let events = Arc::clone(&events);
        request.on_tool_call_finish = Some(ToolCallFinishCallback::new(move |event| {
            push_event(
                &events,
                format!(
                    "tool_call_finish:{}:{}",
                    event.tool_call.tool_name, event.tool_result.is_error
                ),
            )
        }));
    }
    {
        let events = Arc::clone(&events);
        request.on_step_finish = Some(StepCallback::new(move |event| {
            push_event(&events, format!("step_finish:{}", event.step))
        }));
    }
    {
        let events = Arc::clone(&events);
        request.on_finish = Some(FinishCallback::new(move |event| {
            push_event(&events, format!("finish:{}", event.step_count))
        }));
    }

    let response = client
        .run_tools(request)
        .await
        .expect("run_tools should succeed");

    assert_eq!(response.steps, 2);
    let observed = events
        .lock()
        .expect("events mutex should not be poisoned")
        .clone();
    assert_eq!(
        observed,
        vec![
            "start",
            "step_start:1",
            "tool_call_start:get_weather",
            "tool_call_finish:get_weather:false",
            "step_finish:1",
            "step_start:2",
            "step_finish:2",
            "finish:2"
        ]
    );
}

#[tokio::test]
async fn run_tools_prepare_step_can_override_step_input() {
    let server = MockServer::start().await;

    let body = json!({
        "choices": [{
            "message": { "content": "prepared-step-ok" },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 5,
            "completion_tokens": 2,
            "total_tokens": 7
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("from-prepare-step"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .with_openai("test-openai-key", server.uri())
        .build()
        .expect("client should build");

    let mut request = tool_request(vec![]);
    request.max_steps = Some(1);
    request.prepare_step = Some(PrepareStepCallback::new(|event| RunToolsPreparedStep {
        model: event.model.clone(),
        messages: vec![
            Message::system_text("from-prepare-step"),
            Message::user_text("hello"),
        ],
        tools: event.tools.clone(),
        temperature: event.temperature,
        max_output_tokens: event.max_output_tokens,
        stop_sequences: event.stop_sequences.clone(),
    }));

    let response = client
        .run_tools(request)
        .await
        .expect("run_tools should succeed");

    assert_eq!(response.output_text, "prepared-step-ok");
    assert_eq!(response.steps, 1);
}
