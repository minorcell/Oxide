use std::sync::Arc;

use async_trait::async_trait;
use oxide::{
    AiClient, AiErrorCode, ContentPart, Message, MessageRole, ModelRef, ProviderKind,
    RunToolsRequest, Tool, ToolDescriptor, ToolExecError, ToolExecutor,
};
use serde_json::{Value, json};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct DummyWeatherTool;

#[async_trait]
impl ToolExecutor for DummyWeatherTool {
    async fn execute(&self, args: Value) -> Result<Value, ToolExecError> {
        let city = args.get("city").and_then(Value::as_str).unwrap_or("unknown");
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
        model: ModelRef {
            provider: ProviderKind::OpenAi,
            model: "gpt-4o-mini".to_string(),
        },
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
        .openai_api_key("test-openai-key")
        .openai_base_url(server.uri())
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
        .openai_api_key("test-openai-key")
        .openai_base_url(server.uri())
        .build()
        .expect("client should build");

    let err = client
        .run_tools(tool_request(vec![make_weather_tool()]))
        .await
        .expect_err("run_tools should fail for unknown tool");

    assert_eq!(err.code, AiErrorCode::UnknownTool);
}

