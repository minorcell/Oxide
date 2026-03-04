use oxide::{
    AiClient, ContentPart, GenerateTextRequest, Message, MessageRole, ModelRef, ProviderKind,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let client = AiClient::builder().openai_api_key(api_key).build()?;

    let response = client
        .generate_text(GenerateTextRequest {
            model: ModelRef {
                provider: ProviderKind::OpenAi,
                model: "gpt-4o-mini".to_string(),
            },
            messages: vec![Message {
                role: MessageRole::User,
                parts: vec![ContentPart::Text(
                    "Explain Rust lifetimes in one paragraph.".to_string(),
                )],
                name: None,
            }],
            temperature: Some(0.2),
            top_p: None,
            max_output_tokens: Some(200),
            stop_sequences: vec![],
            tools: None,
        })
        .await?;

    println!("{}", response.output_text);
    Ok(())
}
