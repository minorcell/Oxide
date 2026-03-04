use oxide::{
    AiClient, AiErrorCode, ContentPart, GenerateTextRequest, Message, MessageRole, ModelRef,
    ProviderKind,
};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn openai_request() -> GenerateTextRequest {
    GenerateTextRequest {
        model: ModelRef {
            provider: ProviderKind::OpenAi,
            model: "gpt-4o-mini".to_string(),
        },
        messages: vec![Message {
            role: MessageRole::User,
            parts: vec![ContentPart::Text("hello".to_string())],
            name: None,
        }],
        temperature: Some(0.2),
        top_p: None,
        max_output_tokens: Some(64),
        stop_sequences: vec![],
        tools: None,
    }
}

#[tokio::test]
async fn openai_generate_text_success() {
    let server = MockServer::start().await;
    let body = json!({
        "choices": [
            {
                "message": { "content": "Hello from OpenAI" },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer test-openai-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .openai_api_key("test-openai-key")
        .openai_base_url(server.uri())
        .build()
        .expect("client should build");

    let response = client
        .generate_text(openai_request())
        .await
        .expect("generate_text should succeed");

    assert_eq!(response.output_text, "Hello from OpenAI");
    assert_eq!(response.usage.total_tokens, 15);
}

#[tokio::test]
async fn openai_401_maps_to_auth_failed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .openai_api_key("test-openai-key")
        .openai_base_url(server.uri())
        .build()
        .expect("client should build");

    let err = client
        .generate_text(openai_request())
        .await
        .expect_err("request should fail");

    assert_eq!(err.code, AiErrorCode::AuthFailed);
    assert_eq!(err.status, Some(401));
    assert!(!err.retryable);
}

