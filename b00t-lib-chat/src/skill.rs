//! Helper utilities for mapping `/b00t …` slash commands to actionable skills.

use k0mmand3r::KmdLine;
use serde_json::Value;

/// Model management verbs available through `/b00t model …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAction {
    List,
    Download,
    Remove,
    Env,
    Serve,
    Stop,
}

/// Parsed representation of a `/b00t` slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootCommand {
    Model {
        action: ModelAction,
        target: Option<String>,
    },
}

impl BootCommand {
    /// Convert the parsed command into CLI arguments compatible with `b00t-cli`.
    pub fn to_cli_args(&self) -> Vec<String> {
        match self {
            BootCommand::Model { action, target } => {
                let mut args = vec!["model".to_string()];
                match action {
                    ModelAction::List => args.push("list".to_string()),
                    ModelAction::Download => {
                        args.push("download".to_string());
                        if let Some(model) = target {
                            args.push(model.clone());
                        }
                    }
                    ModelAction::Remove => {
                        args.push("remove".to_string());
                        if let Some(model) = target {
                            args.push(model.clone());
                        }
                        args.push("--yes".to_string());
                    }
                    ModelAction::Env => {
                        args.push("env".to_string());
                        if let Some(model) = target {
                            args.push(model.clone());
                        }
                    }
                    ModelAction::Serve => {
                        args.push("serve".to_string());
                        if let Some(model) = target {
                            args.push(model.clone());
                        }
                    }
                    ModelAction::Stop => {
                        args.push("stop".to_string());
                        if let Some(container) = target {
                            args.push(container.clone());
                        }
                    }
                }
                args
            }
        }
    }
}

/// Parse a `/b00t …` slash command emitted from chat UIs using the canonical
/// k0mmand3r parser. Returns `None` for non-b00t verbs or malformed input.
pub fn parse_b00t_command(input: &str) -> Option<BootCommand> {
    let mut slice = input.trim();
    if slice.is_empty() {
        return None;
    }

    let parsed = KmdLine::parse(&mut slice).ok()?;
    let json: Value = serde_json::to_value(&parsed).ok()?;

    let verb = json
        .get("verb")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if verb != "b00t" {
        return None;
    }

    let content = json
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let mut tokens = content.split_whitespace();
    let subcommand = tokens.next().unwrap_or("model");

    match subcommand {
        "model" | "models" => parse_model_command(tokens.collect()),
        _ => None,
    }
}

fn parse_model_command(parts: Vec<&str>) -> Option<BootCommand> {
    let mut iter = parts.iter().copied();
    let action_token = iter.next().unwrap_or("list");

    match action_token {
        "list" | "ls" => Some(BootCommand::Model {
            action: ModelAction::List,
            target: None,
        }),
        "install" | "add" | "load" | "download" => {
            let target = iter.next()?.to_string();
            Some(BootCommand::Model {
                action: ModelAction::Download,
                target: Some(target),
            })
        }
        "remove" | "rm" | "unload" => {
            let target = iter.next()?.to_string();
            Some(BootCommand::Model {
                action: ModelAction::Remove,
                target: Some(target),
            })
        }
        "env" | "export" => Some(BootCommand::Model {
            action: ModelAction::Env,
            target: iter.next().map(|s| s.to_string()),
        }),
        "serve" => Some(BootCommand::Model {
            action: ModelAction::Serve,
            target: iter.next().map(|s| s.to_string()),
        }),
        "stop" => Some(BootCommand::Model {
            action: ModelAction::Stop,
            target: iter.next().map(|s| s.to_string()),
        }),
        "" => Some(BootCommand::Model {
            action: ModelAction::List,
            target: None,
        }),
        other => Some(BootCommand::Model {
            action: ModelAction::Download,
            target: Some(other.to_string()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_list_command() {
        let parsed = parse_b00t_command("/b00t model list").unwrap();
        assert_eq!(
            parsed,
            BootCommand::Model {
                action: ModelAction::List,
                target: None
            }
        );
        assert_eq!(parsed.to_cli_args(), vec!["model", "list"]);
    }

    #[test]
    fn parses_install_aliases() {
        for verb in ["install", "add", "load", "download"] {
            let input = format!("/b00t model {} llava", verb);
            let parsed = parse_b00t_command(&input).unwrap();
            assert_eq!(
                parsed,
                BootCommand::Model {
                    action: ModelAction::Download,
                    target: Some("llava".into())
                }
            );
            assert_eq!(
                parsed.to_cli_args(),
                vec![
                    "model".to_string(),
                    "download".to_string(),
                    "llava".to_string()
                ]
            );
        }
    }

    #[test]
    fn parses_remove_aliases() {
        for verb in ["remove", "rm", "unload"] {
            let input = format!("/b00t model {} deepseek", verb);
            let parsed = parse_b00t_command(&input).unwrap();
            assert_eq!(
                parsed,
                BootCommand::Model {
                    action: ModelAction::Remove,
                    target: Some("deepseek".into())
                }
            );
            assert_eq!(
                parsed.to_cli_args(),
                vec![
                    "model".to_string(),
                    "remove".to_string(),
                    "deepseek".to_string(),
                    "--yes".to_string()
                ]
            );
        }
    }

    #[test]
    fn parses_env_command_with_optional_target() {
        let parsed = parse_b00t_command("/b00t model env").unwrap();
        assert_eq!(
            parsed,
            BootCommand::Model {
                action: ModelAction::Env,
                target: None
            }
        );

        let parsed_with_target = parse_b00t_command("/b00t model env llava").unwrap();
        assert_eq!(
            parsed_with_target,
            BootCommand::Model {
                action: ModelAction::Env,
                target: Some("llava".into())
            }
        );
    }

    #[test]
    fn rejects_non_b00t_commands() {
        assert!(parse_b00t_command("/hello world").is_none());
        assert!(parse_b00t_command("b00t model list").is_none());
    }

    #[test]
    fn parses_serve_command() {
        let parsed = parse_b00t_command("/b00t model serve llava").unwrap();
        assert_eq!(
            parsed.to_cli_args(),
            vec![
                "model".to_string(),
                "serve".to_string(),
                "llava".to_string()
            ]
        );
    }

    #[test]
    fn parses_stop_command_with_container() {
        let parsed = parse_b00t_command("/b00t model stop vllm-llava").unwrap();
        assert_eq!(
            parsed.to_cli_args(),
            vec![
                "model".to_string(),
                "stop".to_string(),
                "vllm-llava".to_string()
            ]
        );
    }
}
