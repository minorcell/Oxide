use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use jsonschema::{Validator, validator_for};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AiError, AiErrorCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, args: Value) -> Result<Value, ToolExecError>;
}

#[derive(Clone)]
pub struct Tool {
    pub descriptor: ToolDescriptor,
    pub executor: Arc<dyn ToolExecutor>,
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tool")
            .field("descriptor", &self.descriptor)
            .field("executor", &"<dyn ToolExecutor>")
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToolExecError {
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("timeout")]
    Timeout,
}

pub(crate) struct RegisteredTool {
    pub tool: Tool,
    pub validator: Validator,
}

pub struct ToolRegistry {
    entries: HashMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn from_tools(tools: Vec<Tool>) -> Result<Self, AiError> {
        let mut entries = HashMap::new();
        let name_re = Regex::new(r"^[a-zA-Z0-9_-]{1,64}$")
            .expect("tool name regex must be valid at compile time");

        for tool in tools {
            let name = tool.descriptor.name.clone();
            if !name_re.is_match(&name) {
                return Err(AiError::new(
                    AiErrorCode::InvalidRequest,
                    format!("invalid tool name `{}`", name),
                ));
            }
            if entries.contains_key(&name) {
                return Err(AiError::new(
                    AiErrorCode::InvalidRequest,
                    format!("duplicate tool name `{}`", name),
                ));
            }

            let validator = validator_for(&tool.descriptor.input_schema).map_err(|e| {
                AiError::new(
                    AiErrorCode::InvalidRequest,
                    format!("invalid JSON schema for tool `{}`: {}", name, e),
                )
            })?;

            entries.insert(name, RegisteredTool { tool, validator });
        }

        Ok(Self { entries })
    }

    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.entries.get(name).map(|entry| &entry.tool)
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }

    pub(crate) fn resolve(&self, name: &str) -> Option<&RegisteredTool> {
        self.entries.get(name)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;

    use super::{Tool, ToolDescriptor, ToolExecError, ToolExecutor, ToolRegistry};

    struct EchoTool;

    #[async_trait]
    impl ToolExecutor for EchoTool {
        async fn execute(
            &self,
            args: serde_json::Value,
        ) -> Result<serde_json::Value, ToolExecError> {
            Ok(args)
        }
    }

    fn make_tool(name: &str) -> Tool {
        Tool {
            descriptor: ToolDescriptor {
                name: name.to_string(),
                description: "echo".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": { "x": { "type": "number" } },
                    "required": ["x"]
                }),
            },
            executor: Arc::new(EchoTool),
        }
    }

    #[test]
    fn rejects_duplicate_tool_names() {
        let tools = vec![make_tool("echo"), make_tool("echo")];
        let result = ToolRegistry::from_tools(tools);
        assert!(result.is_err());
    }

    #[test]
    fn compiles_and_validates_schema() {
        let registry = ToolRegistry::from_tools(vec![make_tool("echo")]).unwrap();
        let entry = registry.resolve("echo").unwrap();
        assert!(entry.validator.validate(&json!({"x": 1})).is_ok());
        assert!(entry.validator.validate(&json!({"y": 1})).is_err());
    }
}
