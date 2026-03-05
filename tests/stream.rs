use futures_util::StreamExt;
use aquaregia::{
    AiClient, ContentPart, GenerateTextRequest, Message, MessageRole, StreamEvent, anthropic,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn anthropic_stream_emits_text_usage_done() {
    let server = MockServer::start().await;
    let sse_body = concat!(
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
        "data: {\"type\":\"message_delta\",\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n"
    );

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body.to_string()),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = AiClient::builder()
        .with_anthropic("test-anthropic-key", server.uri(), "2023-06-01")
        .build()
        .expect("client should build");

    let req = GenerateTextRequest {
        model: anthropic("claude-3-5-haiku-latest").expect("model should parse"),
        messages: vec![Message {
            role: MessageRole::User,
            parts: vec![ContentPart::Text("hello".to_string())],
            name: None,
        }],
        temperature: Some(0.2),
        top_p: None,
        max_output_tokens: Some(32),
        stop_sequences: vec![],
        tools: None,
    };

    let mut stream = client
        .stream_text(req)
        .await
        .expect("stream_text should succeed");

    let mut saw_text = false;
    let mut saw_usage = false;
    let mut saw_done = false;

    while let Some(event) = stream.next().await {
        let event = event.expect("stream event should parse");
        match event {
            StreamEvent::TextDelta { text } => {
                if text == "Hello" {
                    saw_text = true;
                }
            }
            StreamEvent::Usage { usage } => {
                if usage.input_tokens == 3 && usage.output_tokens == 1 {
                    saw_usage = true;
                }
            }
            StreamEvent::Done => {
                saw_done = true;
                break;
            }
            _ => {}
        }
    }

    assert!(saw_text);
    assert!(saw_usage);
    assert!(saw_done);
}
