use futures_util::StreamExt;
use oxide::{AiClient, StreamEvent, openai_compatible};

const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";

/// 场景：流式输出，适合 CLI/Chat UI 一边生成一边展示。
///
/// 运行：
/// DEEPSEEK_API_KEY=... cargo run --example basic_stream
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("DEEPSEEK_API_KEY")?;
    let base_url = std::env::var("DEEPSEEK_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_DEEPSEEK_BASE_URL.to_string());
    let model =
        std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| DEFAULT_DEEPSEEK_MODEL.to_string());

    let client = AiClient::builder()
        .with_openai_compatible(base_url, Some(api_key))
        .build()?;

    let mut stream = client
        .stream_prompt(
            openai_compatible(model)?,
            "Write a short release note for a Rust SDK refactor (Chinese).",
        )
        .await?;

    let mut full_text = String::new();

    println!("=== streaming output ===");
    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextDelta { text } => {
                full_text.push_str(&text);
                print!("{}", text);
            }
            StreamEvent::ToolCallReady { call } => {
                println!("\n[tool call] {} args={}", call.tool_name, call.args_json);
            }
            StreamEvent::Usage { usage } => {
                println!(
                    "\n[usage] input={} output={} total={}",
                    usage.input_tokens, usage.output_tokens, usage.total_tokens
                );
            }
            StreamEvent::Done => {
                println!("\n=== stream done ===");
                break;
            }
        }
    }

    println!(
        "\n--- final text length: {} chars ---",
        full_text.chars().count()
    );
    Ok(())
}
