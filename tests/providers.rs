use aquaregia::{AiClient, OpenAiCompatibleAdapterSettings, google, openai_compatible};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn google_generate_text_success() {
    let server = MockServer::start().await;

    let body = json!({
        "candidates": [
            {
                "content": {
                    "parts": [{ "text": "Hello from Gemini" }]
                },
                "finishReason": "STOP"
            }
        ],
        "usageMetadata": {
            "promptTokenCount": 8,
            "candidatesTokenCount": 4,
            "totalTokenCount": 12
        }
    });

    Mock::given(method("POST"))
        .and(path("/models/gemini-2.0-flash:generateContent"))
        .and(header("x-goog-api-key", "test-google-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .with_google("test-google-key", server.uri())
        .build()
        .expect("client should build");

    let response = client
        .generate_prompt(
            google("gemini-2.0-flash").expect("model should parse"),
            "hello",
        )
        .await
        .expect("request should succeed");

    assert_eq!(response.output_text, "Hello from Gemini");
    assert_eq!(response.usage.total_tokens, 12);
}

#[tokio::test]
async fn openai_compatible_generate_text_success() {
    let server = MockServer::start().await;

    let body = json!({
        "choices": [
            {
                "message": { "content": "Hello from compatible endpoint" },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 6,
            "completion_tokens": 3,
            "total_tokens": 9
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer test-compatible-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let mut settings = OpenAiCompatibleAdapterSettings::new(server.uri());
    settings.api_key = Some("test-compatible-key".to_string());

    let client = AiClient::builder()
        .with_openai_compatible_settings(settings)
        .build()
        .expect("client should build");

    let response = client
        .generate_prompt(
            openai_compatible("deepseek-chat").expect("model should parse"),
            "hello",
        )
        .await
        .expect("request should succeed");

    assert_eq!(response.output_text, "Hello from compatible endpoint");
    assert_eq!(response.usage.total_tokens, 9);
}
