use anyhow::Result;
use clap::CommandFactory;
use rmcp::model::Tool;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::Arc;

/// Trait that allows CLAP structures to describe themselves for MCP tool generation
pub trait McpReflection: CommandFactory {
    /// Get the MCP tool name for this command
    fn mcp_tool_name() -> String;

    /// Get the full command path (e.g., ["mcp", "list"])
    fn command_path() -> Vec<String>;

    /// Generate MCP tool from this command structure
    fn to_mcp_tool() -> Tool {
        let cmd = Self::command();
        let name = Self::mcp_tool_name();
        let description = cmd
            .get_about()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("b00t-cli {}", Self::command_path().join(" ")));

        let mut tool = Tool::default();
        tool.name = name.clone().into();
        tool.title = Some(name);
        tool.description = Some(description.into());
        tool.input_schema = std::sync::Arc::new(Self::generate_json_schema());
        tool
    }

    /// Generate JSON schema from CLAP command structure
    fn generate_json_schema() -> Map<String, Value> {
        let cmd = Self::command();
        let mut schema = Map::new();
        let mut properties = Map::new();
        let mut required = Vec::new();

        schema.insert("type".to_string(), json!("object"));

        // Process arguments and options
        for arg in cmd.get_arguments() {
            let arg_name = arg.get_id().as_str();
            let mut arg_schema = Map::new();

            // Determine type based on CLAP value type
            let arg_type = if arg.is_positional() {
                "string"
            } else if arg.get_action().takes_values() {
                "string"
            } else {
                "boolean"
            };

            arg_schema.insert("type".to_string(), json!(arg_type));

            if let Some(help) = arg.get_help() {
                arg_schema.insert("description".to_string(), json!(help.to_string()));
            }

            if let Some(default) = arg.get_default_values().first() {
                arg_schema.insert("default".to_string(), json!(default.to_string_lossy()));
            }

            properties.insert(arg_name.replace('-', "_"), Value::Object(arg_schema));

            if arg.is_required_set() {
                required.push(arg_name.replace('-', "_"));
            }
        }

        schema.insert("properties".to_string(), Value::Object(properties));

        if !required.is_empty() {
            schema.insert("required".to_string(), json!(required));
        }

        schema
    }
}

/// Trait for executing MCP tool calls by dispatching to actual CLAP commands
pub trait McpExecutor {
    /// Execute the command with the given parameters
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String>;

    /// Return positional argument names in declaration order for this command.
    /// Default: empty (all params treated as --flags).
    /// Override for commands that use positional args.
    fn positional_arg_names() -> Vec<&'static str> {
        vec![]
    }

    /// Convert MCP parameters back to CLAP arguments.
    ///
    /// Positional args (from `positional_arg_names()`) are emitted in order
    /// without a `--` prefix. All remaining named args are emitted as `--flag value`.
    fn params_to_args(params: &HashMap<String, Value>) -> Vec<String> {
        let positionals = Self::positional_arg_names();
        let mut args = Vec::new();

        // Emit positionals first, in declaration order
        for pos_name in &positionals {
            let key = pos_name.replace('-', "_");
            if let Some(value) = params.get(key.as_str()).or_else(|| params.get(*pos_name)) {
                match value {
                    Value::String(s) => args.push(s.clone()),
                    Value::Number(n) => args.push(n.to_string()),
                    _ => args.push(value.to_string().trim_matches('"').to_string()),
                }
            }
        }

        // Emit remaining named (flag) params
        for (key, value) in params {
            // Skip keys that were already emitted as positionals
            let normalized = key.replace('-', "_");
            if positionals
                .iter()
                .any(|p| p.replace('-', "_") == normalized)
            {
                continue;
            }
            match value {
                Value::Bool(true) => {
                    args.push(format!("--{}", key.replace('_', "-")));
                }
                Value::Bool(false) => {
                    // Skip false boolean values
                }
                Value::String(s) => {
                    args.push(format!("--{}", key.replace('_', "-")));
                    args.push(s.clone());
                }
                Value::Number(n) => {
                    args.push(format!("--{}", key.replace('_', "-")));
                    args.push(n.to_string());
                }
                Value::Array(arr) => {
                    for item in arr {
                        args.push(format!("--{}", key.replace('_', "-")));
                        args.push(item.to_string().trim_matches('"').to_string());
                    }
                }
                _ => {
                    args.push(format!("--{}", key.replace('_', "-")));
                    args.push(value.to_string().trim_matches('"').to_string());
                }
            }
        }

        args
    }
}

/// Result of a tool search query
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub schema: Map<String, Value>,
}

/// Registry of all MCP-enabled commands
#[derive(Clone)]
pub struct McpCommandRegistry {
    commands: Arc<Vec<Box<dyn Fn() -> Tool + Send + Sync>>>,
    executors:
        Arc<HashMap<String, Box<dyn Fn(&HashMap<String, Value>) -> Result<String> + Send + Sync>>>,
}

impl McpCommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(Vec::new()),
            executors: Arc::new(HashMap::new()),
        }
    }

    /// Create a registry builder that can be mutated
    pub fn builder() -> McpCommandRegistryBuilder {
        McpCommandRegistryBuilder::new()
    }

    /// Get all tools
    pub fn get_tools(&self) -> Vec<Tool> {
        self.commands.iter().map(|f| f()).collect()
    }

    /// Search tools by query with k0mmand3r-style first-token routing.
    ///
    /// Splits query on whitespace. First token is treated as a potential
    /// category/path prefix; remaining tokens match name/description.
    /// Returns top `limit` results sorted by relevance score.
    pub fn search_tools(&self, query: &str, category: Option<&str>, limit: usize) -> Vec<SearchResult> {
        let tools = self.get_tools();
        let query_lower = query.to_lowercase();
        let tokens: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(i32, SearchResult)> = tools.into_iter().map(|tool| {
            let name = tool.name.as_ref().to_lowercase();
            let desc = tool.description.as_ref().map(|d| d.to_lowercase()).unwrap_or_default();

            // Derive category from name: b00t_<category>_<cmd>
            let derived_category = name.split('_').nth(1).unwrap_or("").to_string();

            let mut score: i32 = 0;

            if let Some(cat_filter) = category {
                let cat_lower = cat_filter.to_lowercase();
                if derived_category == cat_lower {
                    score += 50;
                }
            }

            if !tokens.is_empty() {
                // First token: category/path prefix match
                let first = tokens[0];
                if derived_category == first {
                    score += 40;
                } else if name.contains(&format!("b00t_{first}_")) {
                    score += 25;
                }

                // Remaining tokens: name/description match
                for token in &tokens[1..] {
                    if name.contains(token) {
                        score += 30;
                    }
                    if desc.contains(token) {
                        score += 10;
                    }
                }

                // If only one token, also score it against name/desc
                if tokens.len() == 1 {
                    if name.contains(first) {
                        score += 30;
                    }
                    if desc.contains(first) {
                        score += 10;
                    }
                }
            }

            // Exact name match bonus
            if name == query_lower {
                score += 100;
            }

            let result = SearchResult {
                name: tool.name.as_ref().to_string(),
                description: tool.description.as_ref().map(|s| s.to_string()),
                category: derived_category,
                schema: tool.input_schema.as_ref().clone(),
            };

            (score, result)
        }).collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        // Take top limit with positive scores
        scored.into_iter()
            .filter(|(score, _)| *score > 0)
            .take(limit)
            .map(|(_, result)| result)
            .collect()
    }

    /// Execute a tool call
    pub fn execute(&self, tool_name: &str, params: &HashMap<String, Value>) -> Result<String> {
        if let Some(executor) = self.executors.get(tool_name) {
            executor(params)
        } else {
            anyhow::bail!("Unknown tool: {}", tool_name)
        }
    }
}

/// Builder for McpCommandRegistry that allows mutation
pub struct McpCommandRegistryBuilder {
    commands: Vec<Box<dyn Fn() -> Tool + Send + Sync>>,
    executors:
        HashMap<String, Box<dyn Fn(&HashMap<String, Value>) -> Result<String> + Send + Sync>>,
}

impl McpCommandRegistryBuilder {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            executors: HashMap::new(),
        }
    }

    /// Register a command that implements both McpReflection and McpExecutor
    pub fn register<T>(&mut self) -> &mut Self
    where
        T: McpReflection + McpExecutor + Send + Sync + 'static,
    {
        let tool_name = T::mcp_tool_name();

        // Register tool generator
        self.commands.push(Box::new(|| T::to_mcp_tool()));

        // Register executor
        self.executors
            .insert(tool_name, Box::new(|params| T::execute_mcp_call(params)));

        self
    }

    /// Build the registry
    pub fn build(self) -> McpCommandRegistry {
        McpCommandRegistry {
            commands: Arc::new(self.commands),
            executors: Arc::new(self.executors),
        }
    }
}

impl Default for McpCommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCommand {
        #[arg(help = "Test argument")]
        name: String,

        #[arg(long, help = "Enable verbose output")]
        verbose: bool,

        #[arg(long, default_value = "10", help = "Timeout in seconds")]
        timeout: u32,
    }

    impl McpReflection for TestCommand {
        fn mcp_tool_name() -> String {
            "b00t_test".to_string()
        }

        fn command_path() -> Vec<String> {
            vec!["test".to_string()]
        }
    }

    impl McpExecutor for TestCommand {
        fn execute_mcp_call(_params: &HashMap<String, Value>) -> Result<String> {
            Ok("Test executed".to_string())
        }
    }

    #[test]
    fn test_schema_generation() {
        let schema = TestCommand::generate_json_schema();
        assert!(schema.contains_key("properties"));
        assert!(schema.contains_key("type"));

        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("verbose"));
        assert!(properties.contains_key("timeout"));
    }

    #[test]
    fn test_tool_generation() {
        let tool = TestCommand::to_mcp_tool();
        assert_eq!(tool.name.as_ref(), "b00t_test");
        assert!(tool.description.is_some());
    }

    #[test]
    fn test_registry() {
        let mut builder = McpCommandRegistry::builder();
        builder.register::<TestCommand>();
        let registry = builder.build();

        let tools = registry.get_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "b00t_test");
    }
}
