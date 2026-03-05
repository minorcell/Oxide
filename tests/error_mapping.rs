use aquaregia::{
    AiClient, AiErrorCode, ContentPart, GenerateTextRequest, Message, MessageRole, anthropic,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn anthropic_request() -> GenerateTextRequest {
    GenerateTextRequest {
        model: anthropic("claude-3-5-haiku-latest").expect("model should parse"),
        messages: vec![Message {
            role: MessageRole::User,
            parts: vec![ContentPart::Text("hi".to_string())],
            name: None,
        }],
        temperature: Some(0.2),
        top_p: None,
        max_output_tokens: Some(32),
        stop_sequences: vec![],
        tools: None,
    }
}

#[tokio::test]
async fn anthropic_429_maps_to_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .expect(1)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .with_anthropic("test-anthropic-key", server.uri(), "2023-06-01")
        .max_retries(0)
        .build()
        .expect("client should build");

    let err = client
        .generate_text(anthropic_request())
        .await
        .expect_err("request should fail");

    assert_eq!(err.code, AiErrorCode::RateLimited);
    assert_eq!(err.status, Some(429));
    assert!(err.retryable);
}
