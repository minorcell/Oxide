# aquaregia-macros

Procedural macros for the [aquaregia](https://crates.io/crates/aquaregia) crate.

**Do not add this crate as a direct dependency.** It is re-exported by `aquaregia` and intended to be used through it.

Switch to [中文文档](./README_CN.md).

## The `#[tool]` macro

`#[tool]` turns an `async fn` into an `aquaregia::Tool` with zero boilerplate.
It generates the JSON Schema for the function's parameters automatically and wires up the execution handler.

```rust
use aquaregia::{Agent, LlmClient, tool};
use serde_json::{Value, json};

#[tool(description = "Get current weather for a city")]
async fn get_weather(city: String, unit: String) -> Result<Value, String> {
    Ok(json!({ "city": city, "unit": unit, "temp": 23 }))
}
```

At compile time this expands to a `get_weather()` function that returns an `aquaregia::Tool` — ready to be passed to `Agent::builder`.

### Requirements

| Constraint | Detail |
|---|---|
| Must be `async fn` | Sync functions are not supported |
| Parameters must be simple identifiers | `city: String`, not `(a, b): (String, String)` |
| No `self` | Free functions only |
| No generics | Generic parameters are not supported yet |
| Return type | `Result<T: Serialize, E: ToString>` |

### Arguments

| Argument | Required | Description |
|---|---|---|
| `description` | No | Natural-language description sent to the model |

## License

MIT — see [LICENSE](../LICENSE).
