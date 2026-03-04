use futures_util::StreamExt;
use oxide::{
    AiClient, ContentPart, GenerateTextRequest, Message, MessageRole, ModelRef, ProviderKind,
    StreamEvent,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let client = AiClient::builder().openai_api_key(api_key).build()?;

    let mut stream = client
        .stream_text(GenerateTextRequest {
            model: ModelRef {
                provider: ProviderKind::OpenAi,
                model: "gpt-4o-mini".to_string(),
            },
            messages: vec![Message {
                role: MessageRole::User,
                parts: vec![ContentPart::Text(
                    "Give me three Rust ownership tips.".to_string(),
                )],
                name: None,
            }],
            temperature: Some(0.2),
            top_p: None,
            max_output_tokens: Some(180),
            stop_sequences: vec![],
            tools: None,
        })
        .await?;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextDelta { text } => print!("{}", text),
            StreamEvent::Done => {
                println!();
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
