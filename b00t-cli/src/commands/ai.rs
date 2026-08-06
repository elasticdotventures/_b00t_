use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
pub enum AiCommands {
    #[clap(
        about = "Add AI provider configuration from TOML file",
        long_about = "Add AI provider configuration from TOML file.\n\nExamples:\n  b00t-cli ai add ./openai.ai.toml\n  b00t-cli ai add ~/.dotfiles/_b00t_/anthropic.ai.toml"
    )]
    Add {
        #[clap(help = "Path to AI provider TOML file")]
        file: String,
    },
    #[clap(
        about = "List available AI provider configurations",
        long_about = "List available AI provider configurations.\n\nExamples:\n  b00t-cli ai list\n  b00t-cli ai list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Select ch0nky model based on GPU availability",
        long_about = "GPU-gate: returns the best ch0nky model endpoint given current GPU state.\n\nWhen GPU free ≥ 4000MB → local Qwen3-Coder-30B (vLLM)\nWhen GPU claimed (finetune running) → claude-fable-5 via Anthropic API\n\nExamples:\n  b00t-cli ai ch0nky-select\n  b00t-cli ai ch0nky-select --export   # emit 'export B00T_AI_CH0NKY_MODEL=...' for eval\n  b00t-cli ai ch0nky-select --json"
    )]
    Ch0nkySelect {
        #[clap(long, help = "Emit shell export statement for eval")]
        export: bool,
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },
    #[clap(
        about = "Output AI providers in various formats",
        long_about = "Output AI providers in various formats.\n\nExamples:\n  b00t-cli ai output --kv openai,anthropic\n  b00t-cli ai output --b00t openai\n  b00t-cli ai output anthropic"
    )]
    Output {
        #[clap(long = "b00t", help = "Output in b00t TOML format (default)", action = clap::ArgAction::SetTrue)]
        b00t: bool,
        #[clap(long = "kv", help = "Output environment variables in KEY=VALUE format", action = clap::ArgAction::SetTrue)]
        kv: bool,
        #[clap(help = "Comma-separated list of AI provider names to output")]
        providers: String,
    },
}

impl AiCommands {
    pub fn execute(&self, _path: &str) -> Result<()> {
        match self {
            AiCommands::Add { .. } => {
                println!("🤖 AI add functionality coming soon...");
                Ok(())
            }
            AiCommands::List { .. } => {
                println!("📋 AI list functionality coming soon...");
                Ok(())
            }
            AiCommands::Ch0nkySelect { export, json } => {
                use crate::budget_controller::{ChonkyModelGate, ChonkyTierSource};
                let gate = ChonkyModelGate::default();
                let sel = gate.select();
                let source = match sel.tier_source {
                    ChonkyTierSource::LocalVllm => "local-vllm",
                    ChonkyTierSource::FableFallback => "fable-fallback",
                    ChonkyTierSource::EnvOverride => "env-override",
                };
                if *json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "model": sel.model,
                            "base_url": sel.base_url,
                            "tier_source": source,
                            "gpu_free_mb": sel.gpu_free_mb,
                        })
                    );
                } else if *export {
                    println!("{}", gate.export_env(&sel));
                } else {
                    println!(
                        "model={} base={} source={} gpu_free_mb={:?}",
                        sel.model, sel.base_url, source, sel.gpu_free_mb
                    );
                }
                Ok(())
            }
            AiCommands::Output { .. } => {
                println!("📤 AI output functionality coming soon...");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_commands_exist() {
        let add_cmd = AiCommands::Add {
            file: "test.toml".to_string(),
        };

        assert!(add_cmd.execute("test").is_ok());
    }
}
