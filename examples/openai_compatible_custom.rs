use oxide::{AiClient, OpenAiCompatibleAdapterSettings, openai_compatible};

const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";

/// 场景：接入 OpenAI-Compatible 服务，并配置自定义 headers/query/path。
///
/// 运行示例（以 deepseek 为例）：
/// DEEPSEEK_API_KEY=... cargo run --example openai_compatible_custom
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url = std::env::var("DEEPSEEK_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_DEEPSEEK_BASE_URL.to_string());
    let model =
        std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| DEFAULT_DEEPSEEK_MODEL.to_string());

    let mut settings = OpenAiCompatibleAdapterSettings::new(base_url);
    settings.api_key = Some(std::env::var("DEEPSEEK_API_KEY")?);

    // 可选：部分兼容服务需要额外 header 或 query 参数。
    settings
        .headers
        .insert("x-trace-source".to_string(), "oxide-example".to_string());
    settings
        .query_params
        .insert("source".to_string(), "oxide".to_string());

    // 默认是 /v1/chat/completions，这里保持默认也可。
    settings.chat_completions_path = "/v1/chat/completions".to_string();

    let client = AiClient::builder()
        .with_openai_compatible_settings(settings)
        .build()?;

    let response = client
        .generate_prompt(openai_compatible(model)?, "Say hello in Chinese.")
        .await?;

    println!("{}", response.output_text);
    Ok(())
}
