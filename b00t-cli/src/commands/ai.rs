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
    #[clap(about = "Agent identity & scoped token operations (#1104)")]
    Agent {
        #[clap(subcommand)]
        cmd: AgentSubcommand,
    },
}

#[derive(Parser)]
pub enum AgentSubcommand {
    #[clap(about = "Agent-scoped token operations")]
    Token {
        #[clap(subcommand)]
        cmd: AgentTokenSubcommand,
    },
}

#[derive(Parser)]
pub enum AgentTokenSubcommand {
    #[clap(
        about = "Request a scoped, budget-checked agent token",
        long_about = "Check cake balance (fail before privilege), ensure a k8s ServiceAccount+RoleBinding exist for the requested #1102 shard scope, mint a short-lived (15m) scoped token via k8s TokenRequest, and record the issuance as a ledger transaction.\n\nExample:\n  b00t-cli ai agent token request --agent claude-worker-7 --scope datum:my-datum --cost 3"
    )]
    Request {
        #[clap(long, help = "Requesting agent's identity")]
        agent: String,
        #[clap(
            long,
            help = "#1102 shard scope 'kind:id', e.g. 'datum:my-datum' — kinds: project system agent skill tool datum"
        )]
        scope: String,
        #[clap(long, help = "Cake cost to debit for this issuance")]
        cost: i64,
    },
}

impl AiCommands {
    pub async fn execute(&self, _path: &str) -> Result<()> {
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
            AiCommands::Agent { cmd } => match cmd {
                AgentSubcommand::Token { cmd } => match cmd {
                    AgentTokenSubcommand::Request { agent, scope, cost } => {
                        use crate::agent_token::{AgentTokenRequest, request_agent_token};
                        use crate::cake_ledger::CakeLedger;
                        use crate::soul_scope::SoulScope;

                        let parsed_scope = SoulScope::parse_flag(scope)?;

                        // Pre-flight cost menu: cake is the real, enforced
                        // gate (request_agent_token re-checks it atomically
                        // before any k8s call). Budget-stack integration is
                        // deliberately not wired here — b00t-cli's
                        // `budget` subcommand is per-job-stack, not
                        // per-agent, and there's no natural stack for an
                        // ad-hoc token request to belong to without a
                        // separate design decision; that's left for a
                        // follow-on rather than a forced, mismatched check.
                        if let Ok(ledger) = CakeLedger::open() {
                            if let Ok(balance) = ledger.balance(agent) {
                                println!("💰 '{agent}' cake balance: {balance} (cost: {cost})");
                            }
                        }

                        let issuance = request_agent_token(AgentTokenRequest {
                            agent_id: agent.clone(),
                            scope: parsed_scope,
                            cost: *cost,
                        })
                        .await?;

                        println!("🔑 Agent token issued for '{agent}' (scope: {scope})");
                        println!("   token:      {}", issuance.token);
                        println!("   tx_id:      {}", issuance.tx_id);
                        println!("   expires_in: {}s", issuance.expires_in_seconds);
                        println!("   balance:    {}", issuance.remaining_balance);
                        Ok(())
                    }
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ai_commands_exist() {
        let add_cmd = AiCommands::Add {
            file: "test.toml".to_string(),
        };

        assert!(add_cmd.execute("test").await.is_ok());
    }
}
