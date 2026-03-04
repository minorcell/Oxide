use oxide::{AiClient, OpenAiCompatibleAdapterSettings, openai_compatible};

const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";

/// 场景：演示 DeepSeek(OpenAI-compatible) 的两种接入方式。
///
/// 运行：
/// DEEPSEEK_API_KEY=... cargo run --example provider_selection_demo
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("DEEPSEEK_API_KEY")?;
    let base_url = std::env::var("DEEPSEEK_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_DEEPSEEK_BASE_URL.to_string());
    let model =
        std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| DEFAULT_DEEPSEEK_MODEL.to_string());

    // 1) 直接方式：with_openai_compatible(base_url, api_key)
    let simple_client = AiClient::builder()
        .with_openai_compatible(base_url.clone(), Some(api_key.clone()))
        .build()?;

    let response = simple_client
        .generate_prompt(
            openai_compatible(model.clone())?,
            "Reply with exactly: deepseek-ok",
        )
        .await?;
    println!("simple client result: {}", response.output_text);

    // 2) 高级方式：with_openai_compatible_settings(...)
    let mut settings = OpenAiCompatibleAdapterSettings::new(base_url);
    settings.api_key = Some(api_key);
    settings
        .headers
        .insert("x-demo".to_string(), "provider-selection".to_string());

    let settings_client = AiClient::builder()
        .with_openai_compatible_settings(settings)
        .build()?;

    let second = settings_client
        .generate_prompt(
            openai_compatible(model)?,
            "Reply with exactly: deepseek-settings-ok",
        )
        .await?;
    println!("settings client result: {}", second.output_text);
    Ok(())
}
