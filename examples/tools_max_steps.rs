use std::sync::Arc;

use async_trait::async_trait;
use oxide::{
    AiClient, ContentPart, Message, MessageRole, ModelRef, ProviderKind, RunToolsRequest, Tool,
    ToolDescriptor, ToolExecError, ToolExecutor,
};
use serde_json::{Value, json};

struct WeatherTool;

#[async_trait]
impl ToolExecutor for WeatherTool {
    async fn execute(&self, args: Value) -> Result<Value, ToolExecError> {
        let city = args
            .get("city")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        Ok(json!({ "city": city, "temp_c": 23, "condition": "sunny" }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let client = AiClient::builder().openai_api_key(api_key).build()?;

    let weather_tool = Tool {
        descriptor: ToolDescriptor {
            name: "get_weather".to_string(),
            description: "Get current weather for a city".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }),
        },
        executor: Arc::new(WeatherTool),
    };

    let response = client
        .run_tools(RunToolsRequest {
            model: ModelRef {
                provider: ProviderKind::OpenAi,
                model: "gpt-4o-mini".to_string(),
            },
            messages: vec![Message {
                role: MessageRole::User,
                parts: vec![ContentPart::Text(
                    "What is the weather in Shanghai? Use tools if needed.".to_string(),
                )],
                name: None,
            }],
            tools: vec![weather_tool],
            max_steps: Some(4),
            temperature: Some(0.2),
            max_output_tokens: Some(300),
            stop_sequences: vec![],
        })
        .await?;

    println!("{}", response.output_text);
    Ok(())
}
