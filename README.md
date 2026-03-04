# Oxide

Rust AI SDK（OpenAI + Anthropic）统一接口实现，支持：

- `generate_text`：非流式生成
- `stream_text`：流式事件输出
- `run_tools`：工具调用自动循环（`max_steps`）
- 可选 `axum` SSE 适配

当前交付形态：库 + `examples/`。

## Features

`Cargo.toml` 默认：

- `default = ["openai", "anthropic"]`
- `axum` 为可选 feature

常用组合：

- 仅 OpenAI：`--no-default-features --features openai`
- 仅 Anthropic：`--no-default-features --features anthropic`
- 启用 Axum 适配：`--features axum`

## 快速开始

```rust
use oxide::{
    AiClient, ContentPart, GenerateTextRequest, Message, MessageRole, ModelRef, ProviderKind,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AiClient::builder()
        .openai_api_key(std::env::var("OPENAI_API_KEY")?)
        .build()?;

    let resp = client
        .generate_text(GenerateTextRequest {
            model: ModelRef {
                provider: ProviderKind::OpenAi,
                model: "gpt-4o-mini".to_string(),
            },
            messages: vec![Message {
                role: MessageRole::User,
                parts: vec![ContentPart::Text("Explain Rust ownership briefly.".to_string())],
                name: None,
            }],
            temperature: Some(0.2),
            top_p: None,
            max_output_tokens: Some(200),
            stop_sequences: vec![],
            tools: None,
        })
        .await?;

    println!("{}", resp.output_text);
    Ok(())
}
```

## Examples

运行示例：

```bash
cargo run --example basic_generate
cargo run --example basic_stream
cargo run --example tools_max_steps
```

需要环境变量（按示例使用的 provider）：

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`

## 开发与验证

```bash
cargo test
cargo check --no-default-features
cargo check --no-default-features --features openai
cargo check --no-default-features --features anthropic
cargo check --features axum
```

## 项目结构

- `src/client.rs`：`AiClient` 与重试/工具循环
- `src/provider/openai.rs`：OpenAI 适配
- `src/provider/anthropic.rs`：Anthropic 适配
- `src/tool.rs`：`ToolDescriptor` / `Tool` / `ToolRegistry`
- `src/types.rs`：统一请求/响应/流事件类型
- `src/error.rs`：错误码与映射
