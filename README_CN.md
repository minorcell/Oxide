# Oxide

使用 Oxide 快速构建您的 Rust AI 应用，具有统一的多供应商接口和强大的工具执行能力。

[English README](./README.md)

## 项目亮点

- 统一 API：OpenAI / Anthropic / Google / OpenAI-compatible
- 模型选择：`openai(...)`、`anthropic(...)`、`google(...)`、`openai_compatible(...)`
- 内置工具循环运行时（`max_steps` 保护）
- 动态规划 hook：`prepare_call`、`prepare_step`
- 提供完整生命周期回调：`on_start`、`on_step_start`、`on_tool_call_start`、`on_tool_call_finish`、`on_step_finish`、`on_finish`、`stop_when`
- `Agent` 封装：模型 + 指令 + 工具的可复用工作流

## Provider 模型

一个 `AiClient` 只绑定一个 Provider 配置。

如果要同时使用多个 Provider，请创建多个 `AiClient`。

## 支持的模型适配

| 适配类型         | 注册方法                                                                               | 模型选择                               |
| ---------------- | -------------------------------------------------------------------------------------- | -------------------------------------- |
| OpenAI GPT       | `.with_openai(api_key, base_url)`                                                      | `openai("gpt-4o-mini")`                |
| Anthropic Claude | `.with_anthropic(api_key, base_url, api_version)`                                      | `anthropic("claude-3-5-haiku-latest")` |
| Google Gemini    | `.with_google(api_key, base_url)`                                                      | `google("gemini-2.0-flash")`           |
| OpenAI 兼容接口  | `.with_openai_compatible(base_url, api_key)` / `.with_openai_compatible_settings(...)` | `openai_compatible("deepseek-chat")`   |

## 安装

```toml
[dependencies]
oxide = { path = "." }
# 发布到 crates.io 后可替换为版本号：
# oxide = "x.y.z"
```

## 快速开始（3 个示例）

### 1）一次性调用（DeepSeek）

```rust
use oxide::{AiClient, openai_compatible};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("DEEPSEEK_API_KEY")?;
    let base_url =
        std::env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".to_string());

    let client = AiClient::builder()
        .with_openai_compatible(base_url, Some(api_key))
        .build()?;

    let resp = client
        .generate_prompt(openai_compatible("deepseek-chat")?, "用 3 个要点解释 Rust 所有权。")
        .await?;

    println!("{}", resp.output_text);
    Ok(())
}
```

### 2）循环 + 工具调用（Agent + step 回调）

```rust
use oxide::{tool, Agent, AiClient, RunToolsStep, openai_compatible};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("DEEPSEEK_API_KEY")?;
    let base_url =
        std::env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".to_string());

    let client = AiClient::builder()
        .with_openai_compatible(base_url, Some(api_key))
        .build()?;

    let weather = tool("get_weather")
        .description("Get weather by city")
        .input_schema(json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        }))
        .execute(|args| async move {
            let city = args.get("city").and_then(|v| v.as_str()).unwrap_or("unknown");
            Ok(json!({ "city": city, "temp_c": 23, "condition": "sunny" }))
        });

    let agent = Agent::builder(client)
        .model(openai_compatible("deepseek-chat")?)
        .instructions("回答前可以先调用工具。")
        .tool(weather)
        .max_steps(4)
        .on_step_finish(|step: &RunToolsStep| {
            println!("step={} tool_calls={}", step.step, step.tool_calls.len());
        })
        .build()?;

    let out = agent
        .generate_prompt("上海天气如何？必要时请先调用工具。")
        .await?;

    println!("{}", out.output_text);
    Ok(())
}
```

### 3）DeepSeek 两种 Client 配置方式

```rust
use oxide::{AiClient, OpenAiCompatibleAdapterSettings, openai_compatible};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("DEEPSEEK_API_KEY")?;
    let base_url =
        std::env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".to_string());

    let simple_client = AiClient::builder()
        .with_openai_compatible(base_url.clone(), Some(api_key.clone()))
        .build()?;

    let mut settings = OpenAiCompatibleAdapterSettings::new(base_url);
    settings.api_key = Some(api_key);

    let settings_client = AiClient::builder()
        .with_openai_compatible_settings(settings)
        .build()?;

    let _ = simple_client
        .generate_prompt(openai_compatible("deepseek-chat")?, "hello")
        .await?;
    let _ = settings_client
        .generate_prompt(openai_compatible("deepseek-chat")?, "hello again")
        .await?;

    Ok(())
}
```

## 模型选择

推荐使用 provider helper，而不是手写模型 ID 字符串：

- `openai("gpt-4o-mini")`
- `anthropic("claude-3-5-haiku-latest")`
- `google("gemini-2.0-flash")`
- `openai_compatible("deepseek-chat")`

## 动态步骤控制

`Agent` 提供：

- `prepare_call`：一次调用开始前，动态改 model/messages/tools/sampling
- `prepare_step`：每一步开始前，动态改 model/messages/tools/sampling

```rust
use oxide::{
    Agent, AgentCallPlan, Message, RunToolsPrepareStep, RunToolsPreparedStep,
    openai_compatible,
};

let agent = Agent::builder(client)
    .model(openai_compatible("deepseek-chat")?)
    .prepare_call(|plan: &AgentCallPlan| {
        let mut next = plan.clone();
        next.temperature = Some(0.2);
        next
    })
    .prepare_step(|event: &RunToolsPrepareStep| {
        let mut next = RunToolsPreparedStep {
            model: event.model.clone(),
            messages: event.messages.clone(),
            tools: event.tools.clone(),
            temperature: event.temperature,
            max_output_tokens: event.max_output_tokens,
            stop_sequences: event.stop_sequences.clone(),
        };
        next.messages.push(Message::system_text(format!("step={}", event.step)));
        next
    })
    .build()?;
```

## 可运行示例

```bash
cargo run --example basic_generate
cargo run --example basic_stream
cargo run --example agent_minimal
cargo run --example tools_max_steps
cargo run --example provider_selection_demo
cargo run --example google_generate
cargo run --example openai_compatible_custom
cargo run --example mini_claude_code
cargo run --example prepare_hooks
```

完整场景说明见：[examples/README.md](./examples/README.md)

## 开发

```bash
cargo fmt
cargo test
cargo check --examples
cargo check --no-default-features
cargo check --no-default-features --features openai
cargo check --no-default-features --features anthropic
cargo check --features axum
```

## 贡献

欢迎提 Issue 和 PR。涉及行为变更请附带测试。

## 许可证

见仓库中的 License 文件。
