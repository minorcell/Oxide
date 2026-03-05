# aquaregia-macros

[aquaregia](https://crates.io/crates/aquaregia) 的过程宏实现。

**请勿直接依赖本 crate。** 它由 `aquaregia` 重新导出，应通过主库使用。

切换到 [English README](./README.md)。

## `#[tool]` 宏

`#[tool]` 将一个 `async fn` 转换为 `aquaregia::Tool`，无需手写任何模板代码。
它会自动为函数参数生成 JSON Schema，并绑定执行处理器。

```rust
use aquaregia::{Agent, LlmClient, tool};
use serde_json::{Value, json};

#[tool(description = "获取指定城市的当前天气")]
async fn get_weather(city: String, unit: String) -> Result<Value, String> {
    Ok(json!({ "city": city, "unit": unit, "temp": 23 }))
}
```

编译时，上述代码会展开为一个 `get_weather()` 函数，返回 `aquaregia::Tool`，可直接传入 `Agent::builder`。

### 使用限制

| 限制 | 说明 |
|---|---|
| 必须是 `async fn` | 不支持同步函数 |
| 参数必须是简单标识符 | 支持 `city: String`，不支持 `(a, b): (String, String)` |
| 不支持 `self` | 仅支持自由函数 |
| 不支持泛型 | 暂不支持泛型参数 |
| 返回类型 | `Result<T: Serialize, E: ToString>` |

### 宏参数

| 参数 | 是否必填 | 说明 |
|---|---|---|
| `description` | 否 | 传给模型的自然语言描述 |

## 许可证

MIT — 详见 [LICENSE](../LICENSE)。
