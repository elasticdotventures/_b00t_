//! Datum LSP - Language Server Protocol for b00t configuration
//!
//! 🤓 提供 TOML/TOMLLM datum 文件的 LSP intellisense:
//! - Completion: datum 类型、字段、枚举值
//! - Hover: 字段文档、示例、部落知识
//! - Validation: schema 验证、类型检查
//! - Dynamic: 从 b00t inspect 获取实时值更新

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Datum LSP 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumLspConfig {
    pub b00t_path: String,
    pub dynamic_inspection: bool,
}

impl Default for DatumLspConfig {
    fn default() -> Self {
        Self {
            b00t_path: "~/.b00t/_b00t_".to_string(),
            dynamic_inspection: true,
        }
    }
}

/// Datum schema 用于 LSP 补全
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumSchema {
    pub name: String,
    pub suffix: String,
    pub description: String,
    pub required_fields: Vec<String>,
    pub optional_fields: Vec<String>,
}

/// LSP 服务器状态
pub struct DatumLspServer {
    config: DatumLspConfig,
    schemas: Vec<DatumSchema>,
}

impl DatumLspServer {
    pub fn new(config: DatumLspConfig) -> Self {
        let schemas = Self::default_schemas();
        Self { config, schemas }
    }

    fn default_schemas() -> Vec<DatumSchema> {
        vec![
            DatumSchema {
                name: "mcp".to_string(),
                suffix: ".mcp.toml".to_string(),
                description: "MCP server configuration".to_string(),
                required_fields: vec!["name".into(), "type".into()],
                optional_fields: vec!["hint".into(), "install".into(), "env".into()],
            },
            DatumSchema {
                name: "cli".to_string(),
                suffix: ".cli.toml".to_string(),
                description: "CLI tool configuration".to_string(),
                required_fields: vec!["name".into(), "type".into()],
                optional_fields: vec!["hint".into(), "desires".into()],
            },
            DatumSchema {
                name: "ai".to_string(),
                suffix: ".ai.toml".to_string(),
                description: "AI model configuration".to_string(),
                required_fields: vec!["name".into(), "type".into()],
                optional_fields: vec!["provider".into(), "cost_per_1k_tokens".into()],
            },
        ]
    }

    /// 获取补全建议
    pub fn get_completions(&self, trigger: &str) -> Vec<String> {
        match trigger {
            "root" => self.schemas.iter().map(|s| s.name.clone()).collect(),
            "b00t" => {
                let mut fields = vec!["name", "type", "hint", "install", "env", "learn"];
                fields.iter().map(|s| s.to_string()).collect()
            }
            _ => vec![],
        }
    }

    /// 获取 hover 信息
    pub fn get_hover(&self, symbol: &str) -> Option<String> {
        for schema in &self.schemas {
            if schema.name == symbol {
                return Some(format!(
                    "**{}** {}\n\nRequired: {}\nOptional: {}",
                    schema.name,
                    schema.description,
                    schema.required_fields.join(", "),
                    schema.optional_fields.join(", ")
                ));
            }
        }
        None
    }
}

/// 运行 LSP 服务器 (stdio)
///
/// ⚠️ Not yet implemented: requires tower-lsp integration for proper
/// JSON-RPC/Content-Length framing. Use `b00t-lsp` binary once ready.
pub async fn run_lsp_server() -> Result<()> {
    Err(anyhow::anyhow!(
        "b00t LSP server not yet implemented: tower-lsp integration pending. \
         DatumLspServer schema types are available but the stdio transport \
         does not yet speak the LSP JSON-RPC protocol."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = DatumLspServer::default();
        assert_eq!(server.schemas.len(), 3);
    }

    #[test]
    fn test_completions() {
        let server = DatumLspServer::default();
        let completions = server.get_completions("root");
        assert!(completions.contains(&"mcp".to_string()));
    }
}

impl Default for DatumLspServer {
    fn default() -> Self {
        Self::new(DatumLspConfig::default())
    }
}
