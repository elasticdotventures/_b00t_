//! `b00t zellij` — Zellij session interaction commands.
//!
//! Provides CLI subcommands for interacting with Zellij sessions:
//! - `detect` — check if running inside Zellij
//! - `menu` — launch fzf menu in floating pane
//! - `confirm` — Y/N confirm dialog
//! - `input` — free-text input dialog
//! - `subagent` — read-only agent report display
//! - `wizard` — multi-step TOML-driven wizard
//!
//! Uses b00t-c0re-lib types (InteractionMode, MenuItem, InputRequest, etc.)
//! and shells out to `zellij run` with proper Command quoting.

use anyhow::{Context, Result};
use b00t_c0re_lib::interaction::{AgentAction, EisenhowerQuadrant, MenuItem};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Top-level Zellij subcommand enum.
#[derive(Subcommand, Debug, Clone)]
pub enum ZellijCommand {
    /// Detect whether running inside a Zellij session.
    ///
    /// Checks ZELLIJ_SESSION_NAME env var. Prints detection info as JSON to stdout.
    /// Exit code 0 = inside Zellij, 1 = outside Zellij.
    Detect,

    /// Launch an interactive fzf menu in a Zellij floating pane.
    ///
    /// Accepts menu items via --items (JSON array of MenuItem) or --items-file (TOML file path).
    /// Returns the selected item's key on stdout.
    Menu {
        /// JSON array of MenuItem objects
        #[arg(long, group = "items_source")]
        items: Option<String>,

        /// Path to a TOML file containing [[items]] table
        #[arg(long, group = "items_source")]
        items_file: Option<PathBuf>,

        /// Title for the floating pane
        #[arg(long, default_value = "b00t Menu")]
        title: String,

        /// Prompt string shown above the fzf list
        #[arg(long, default_value = "Select an action:")]
        prompt: String,
    },

    /// Quick Y/N confirm dialog in a Zellij floating pane.
    ///
    /// Returns "yes" or "no" on stdout. Exit codes: 0=yes, 1=no.
    Confirm {
        /// The question/prompt to display
        #[arg(short, long)]
        prompt: String,

        /// Title for the floating pane
        #[arg(long, default_value = "b00t Confirm")]
        title: String,
    },

    /// Free-text input dialog in a Zellij floating pane.
    ///
    /// Returns the user's text on stdout.
    Input {
        /// The prompt text shown to the user
        #[arg(short, long)]
        prompt: String,

        /// Default value pre-filled in the input
        #[arg(long, default_value = "")]
        default: String,

        /// Title for the floating pane
        #[arg(long, default_value = "b00t Input")]
        title: String,
    },

    /// Display a read-only agent report in a Zellij floating pane.
    Subagent {
        /// Title for the floating pane
        #[arg(short, long)]
        title: String,

        /// Content to display (markdown or plain text)
        #[arg(short, long)]
        content: String,
    },

    /// Run a multi-step wizard from a TOML definition file.
    ///
    /// Each step is a sequential prompt; supports conditional branching
    /// based on previous answers. Returns JSON result with all step outputs.
    Wizard {
        /// Path to TOML wizard definition file
        #[arg(short, long)]
        file: PathBuf,

        /// Title for the floating pane
        #[arg(long, default_value = "b00t Wizard")]
        title: String,
    },
}

/// Handle a ZellijCommand by dispatching to the appropriate sub-handler.
pub fn handle(command: ZellijCommand) -> Result<()> {
    match command {
        ZellijCommand::Detect => cmd_detect(),
        ZellijCommand::Menu {
            items,
            items_file,
            title,
            prompt,
        } => cmd_menu(items, items_file, &title, &prompt),
        ZellijCommand::Confirm { prompt, title } => cmd_confirm(&prompt, &title),
        ZellijCommand::Input {
            prompt,
            default,
            title,
        } => cmd_input(&prompt, &default, &title),
        ZellijCommand::Subagent { title, content } => cmd_subagent(&title, &content),
        ZellijCommand::Wizard { file, title } => cmd_wizard(&file, &title),
    }
}

// ─── detect ───────────────────────────────────────────────────────────────────

/// Detection info emitted as JSON by `b00t zellij detect`.
#[derive(Serialize)]
struct DetectionInfo {
    inside_zellij: bool,
    session_name: Option<String>,
    pane_id: Option<String>,
    pane_index: Option<String>,
}

fn detect_zellij_vars() -> DetectionInfo {
    let session = std::env::var("ZELLIJ_SESSION_NAME").ok();
    let pane_id = std::env::var("ZELLIJ_PANE_ID").ok();
    let pane_index = std::env::var("ZELLIJ_PANE_INDEX").ok();

    DetectionInfo {
        inside_zellij: session.is_some(),
        session_name: session,
        pane_id,
        pane_index,
    }
}

fn cmd_detect() -> Result<()> {
    let info = detect_zellij_vars();
    let json = serde_json::to_string(&info).context("Failed to serialize detection info")?;
    println!("{json}");

    if info.inside_zellij {
        Ok(())
    } else {
        // Exit code 1 when NOT in Zellij
        std::process::exit(1);
    }
}

// ─── menu ─────────────────────────────────────────────────────────────────────

/// TOML structure for menu items loaded from a file.
#[derive(Deserialize)]
struct MenuItemsToml {
    items: Vec<MenuItemToml>,
}

#[derive(Deserialize)]
struct MenuItemToml {
    key: String,
    label: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    quadrant: Option<String>,
    #[serde(default)]
    action: Option<String>,
}

fn parse_quadrant(s: &str) -> EisenhowerQuadrant {
    match s {
        "urgent-important" | "UrgentImportant" | "do-now" | "do_now" => {
            EisenhowerQuadrant::UrgentImportant
        }
        "not-urgent-important" | "NotUrgentImportant" | "schedule" => {
            EisenhowerQuadrant::NotUrgentImportant
        }
        "urgent-not-important" | "UrgentNotImportant" | "delegate" => {
            EisenhowerQuadrant::UrgentNotImportant
        }
        "not-urgent-not-important" | "NotUrgentNotImportant" | "eliminate" => {
            EisenhowerQuadrant::NotUrgentNotImportant
        }
        _ => EisenhowerQuadrant::UrgentImportant,
    }
}

fn parse_action(s: &str) -> AgentAction {
    match s {
        "reject" | "Reject" => AgentAction::Reject,
        "delegate" | "Delegate" => AgentAction::Delegate,
        "audit" | "Audit" => AgentAction::Audit,
        _ => AgentAction::Approve,
    }
}

fn load_menu_items(
    items_json: Option<&str>,
    items_file: Option<&PathBuf>,
) -> Result<Vec<MenuItem>> {
    if let Some(json_str) = items_json {
        let items: Vec<MenuItem> =
            serde_json::from_str(json_str).context("Failed to parse --items JSON")?;
        return Ok(items);
    }

    if let Some(path) = items_file {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read items file: {}", path.display()))?;
        let toml_data: MenuItemsToml =
            toml::from_str(&content).context("Failed to parse items TOML file")?;

        let items = toml_data
            .items
            .into_iter()
            .map(|i| MenuItem {
                key: i.key,
                label: i.label,
                quadrant: i
                    .quadrant
                    .as_deref()
                    .map(parse_quadrant)
                    .unwrap_or(EisenhowerQuadrant::UrgentImportant),
                action: i
                    .action
                    .as_deref()
                    .map(parse_action)
                    .unwrap_or(AgentAction::Approve),
                description: i.description,
            })
            .collect();
        return Ok(items);
    }

    anyhow::bail!("Either --items or --items-file must be provided");
}

fn run_in_zellij_floating(script: &str, title: &str, close_on_exit: bool) -> Result<String> {
    let mut cmd = Command::new("zellij");
    cmd.arg("run").arg("--floating").arg("--name").arg(title);

    if close_on_exit {
        cmd.arg("--close-on-exit");
    }

    cmd.arg("--").arg("bash").arg("-c").arg(script);

    // Capture stdout
    let output = cmd.output().context("Failed to execute zellij run")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("zellij run failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn cmd_menu(
    items_json: Option<String>,
    items_file: Option<PathBuf>,
    title: &str,
    prompt: &str,
) -> Result<()> {
    let items = load_menu_items(items_json.as_deref(), items_file.as_ref())?;
    if items.is_empty() {
        anyhow::bail!("No menu items provided");
    }

    // Build fzf input text
    let fzf_input: String = items
        .iter()
        .map(|item| {
            format!("{}\t{}", item.key, item.label)
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Write fzf input to temp file
    let input_path = "/tmp/b00t_zellij_fzf_input";
    let output_path = "/tmp/b00t_zellij_fzf_output";

    std::fs::write(input_path, fzf_input.as_bytes())
        .context("Failed to write fzf input temp file")?;

    // Remove stale output file
    let _ = std::fs::remove_file(output_path);

    // Build the wrapper script
    let escaped_prompt = prompt.replace('\'', "'\\''");
    let escaped_title = title.replace('\'', "'\\''");

    // Script that runs fzf and captures the selection
    let script = format!(
        r#"fzf --prompt='{escaped_prompt}' \
    --header='{escaped_title}' \
    --delimiter='\t' --with-nth=1 \
    --preview='echo {{2}}' --preview-window=down:1:hidden \
    --bind='?:toggle-preview' \
    --print-query \
    < {input_path} 2>/dev/null | tail -1 > {output_path}"#
    );

    // Check if we're inside Zellij
    if std::env::var("ZELLIJ_SESSION_NAME").is_ok() {
        // Inside Zellij: use floating pane
        run_in_zellij_floating(&script, title, true)?;
    } else {
        // Outside Zellij: run fzf directly
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&script);
        let _output = cmd.output().context("Failed to run fzf")?;
    }

    // Read the selection from the output file
    match std::fs::read_to_string(output_path) {
        Ok(selection) => {
            let key = selection.trim().to_string();
            if key.is_empty() {
                // User cancelled
                std::process::exit(1);
            }
            println!("{key}");
            Ok(())
        }
        Err(_) => {
            // No selection made (user cancelled)
            std::process::exit(1);
        }
    }
}

// ─── confirm ──────────────────────────────────────────────────────────────────

fn cmd_confirm(prompt: &str, title: &str) -> Result<()> {
    let escaped_prompt = prompt.replace('\'', "'\\''");
    let escaped_title = title.replace('\'', "'\\''");
    let output_path = "/tmp/b00t_zellij_confirm_output";

    let _ = std::fs::remove_file(output_path);

    // Use fzf with yes/no as the two options for a nice TUI experience.
    // Fall back to plain read if fzf isn't available.
    let script = format!(
        r#"if command -v fzf >/dev/null 2>&1; then
    printf 'yes\nno\n' | fzf --prompt='{escaped_prompt} ' --header='{escaped_title}' --height=5 > {output_path} 2>/dev/null
else
    printf '%s\n' '{escaped_prompt}'
    printf '[y/N]: '
    read -r answer
    case "$answer" in
        [Yy]|[Yy][Ee][Ss]) echo 'yes' > {output_path} ;;
        *) echo 'no' > {output_path} ;;
    esac
fi"#
    );

    if std::env::var("ZELLIJ_SESSION_NAME").is_ok() {
        run_in_zellij_floating(&script, title, true)?;
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&script);
        let _output = cmd.output().context("Failed to run confirm dialog")?;
    }

    match std::fs::read_to_string(output_path) {
        Ok(answer) => {
            let answer = answer.trim().to_lowercase();
            println!("{answer}");
            if answer == "yes" {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        Err(_) => {
            println!("no");
            std::process::exit(1);
        }
    }
}

// ─── input ────────────────────────────────────────────────────────────────────

fn cmd_input(prompt: &str, default: &str, title: &str) -> Result<()> {
    let escaped_prompt = prompt.replace('\'', "'\\''");
    let escaped_title = title.replace('\'', "'\\''");
    let escaped_default = default.replace('\'', "'\\''");
    let output_path = "/tmp/b00t_zellij_input_output";

    let _ = std::fs::remove_file(output_path);

    // Use fzf print-query trick for text input, or fall back to read
    let script = format!(
        r#"if command -v fzf >/dev/null 2>&1; then
    printf '' | fzf --prompt='{escaped_prompt}' \
        --header='{escaped_title}' \
        --print-query \
        --preview='echo "Default: {escaped_default}"' \
        --preview-window=up:1 \
        2>/dev/null | head -1 > {output_path}
    # If fzf returned empty (user hit enter without typing), use default
    if [ ! -s {output_path} ]; then
        echo '{escaped_default}' > {output_path}
    fi
else
    printf '%s' '{escaped_prompt} [default: {escaped_default}]: '
    read -r answer
    if [ -z "$answer" ]; then
        echo '{escaped_default}' > {output_path}
    else
        echo "$answer" > {output_path}
    fi
fi"#
    );

    if std::env::var("ZELLIJ_SESSION_NAME").is_ok() {
        run_in_zellij_floating(&script, title, true)?;
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&script);
        let _output = cmd.output().context("Failed to run input dialog")?;
    }

    match std::fs::read_to_string(output_path) {
        Ok(text) => {
            let text = text.trim().to_string();
            println!("{text}");
            Ok(())
        }
        Err(_) => {
            println!("{default}");
            Ok(())
        }
    }
}

// ─── subagent ─────────────────────────────────────────────────────────────────

fn cmd_subagent(title: &str, content: &str) -> Result<()> {
    let escaped_title = title.replace('\'', "'\\''");
    let content_path = "/tmp/b00t_zellij_subagent_content";

    std::fs::write(content_path, content.as_bytes())
        .context("Failed to write subagent content temp file")?;

    // Display using less or cat in a floating pane
    let script = format!(
        r#"clear
echo "═══════════════════════════════════════════════════"
echo "  {escaped_title}"
echo "═══════════════════════════════════════════════════"
echo
cat {content_path}
echo
echo "───────────────────────────────────────────────────"
echo "Press Enter to close this pane"
read -r _dummy"#
    );

    if std::env::var("ZELLIJ_SESSION_NAME").is_ok() {
        run_in_zellij_floating(&script, title, false)?;
    } else {
        // Outside Zellij: just print to stdout
        println!("═══ {escaped_title} ═══");
        println!();
        println!("{content}");
    }

    Ok(())
}

// ─── wizard ───────────────────────────────────────────────────────────────────

/// A single step in a wizard definition (TOML).
#[derive(Deserialize, Debug)]
struct WizardStep {
    /// Prompt text for this step.
    prompt: String,
    /// Variable name to store the result.
    var: String,
    /// Default value.
    #[serde(default)]
    default: Option<String>,
    /// Input type: "text", "confirm", "menu".
    #[serde(default = "default_input_type")]
    input_type: String,
    /// Menu items (only for input_type = "menu").
    #[serde(default)]
    options: Vec<String>,
    /// Condition to skip this step (simple var=value check).
    #[serde(default)]
    condition: Option<String>,
    /// If condition is not met, skip to this step index.
    #[serde(default)]
    skip_to: Option<usize>,
}

fn default_input_type() -> String {
    "text".to_string()
}

/// TOML wizard definition file structure.
#[derive(Deserialize, Debug)]
struct WizardToml {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    steps: Vec<WizardStep>,
}

/// A result from one wizard step.
#[derive(Serialize, Debug)]
struct WizardStepResult {
    var: String,
    value: String,
}

/// Full wizard result.
#[derive(Serialize, Debug)]
struct WizardResult {
    steps: Vec<WizardStepResult>,
}

fn evaluate_condition(
    condition: &str,
    answers: &std::collections::HashMap<String, String>,
) -> bool {
    // Simple condition format: "var=value" or "var!=value"
    if let Some((var, expected)) = condition.split_once('=') {
        // Handle !=
        if var.ends_with('!') {
            let var = &var[..var.len() - 1];
            let actual = answers.get(var).map(String::as_str).unwrap_or("");
            return actual != expected;
        }
        let actual = answers.get(var).map(String::as_str).unwrap_or("");
        return actual == expected;
    }
    // Default: condition is a var name, check if truthy
    let val = answers.get(condition).map(String::as_str).unwrap_or("");
    !val.is_empty() && val != "false" && val != "no"
}

/// Build a single-step interactive prompt script.
fn build_step_script(
    prompt: &str,
    default: &str,
    input_type: &str,
    options: &[String],
    output_path: &str,
    title: &str,
) -> String {
    let escaped_prompt = prompt.replace('\'', "'\\''");
    let escaped_default = default.replace('\'', "'\\''");
    let escaped_title = title.replace('\'', "'\\''");

    match input_type {
        "confirm" => {
            format!(
                r#"if command -v fzf >/dev/null 2>&1; then
    printf 'yes\nno\n' | fzf --prompt='{escaped_prompt} ' --header='{escaped_title}' --height=5 > {output_path} 2>/dev/null
else
    printf '%s\n' '{escaped_prompt}'
    printf '[y/N]: '
    read -r answer
    case "$answer" in
        [Yy]|[Yy][Ee][Ss]) echo 'yes' > {output_path} ;;
        *) echo 'no' > {output_path} ;;
    esac
fi"#
            )
        }
        "menu" => {
            let options_str = options.join("\n");
            let opt_path = format!("{output_path}_opts");
            format!(
                r#"printf '%s\n' '{options_str}' > {opt_path}
if command -v fzf >/dev/null 2>&1; then
    fzf --prompt='{escaped_prompt} ' --header='{escaped_title}' < {opt_path} > {output_path} 2>/dev/null
else
    printf '%s\n' '{escaped_prompt}'
    cat -n {opt_path}
    printf 'Choice: '
    read -r answer
    echo "$answer" > {output_path}
fi"#
            )
        }
        _ => {
            // text input
            format!(
                r#"if command -v fzf >/dev/null 2>&1; then
    printf '' | fzf --prompt='{escaped_prompt}' \
        --header='{escaped_title}' \
        --print-query \
        --preview='echo "Default: {escaped_default}"' \
        --preview-window=up:1 \
        2>/dev/null | head -1 > {output_path}
    if [ ! -s {output_path} ]; then
        echo '{escaped_default}' > {output_path}
    fi
else
    printf '%s' '{escaped_prompt} [default: {escaped_default}]: '
    read -r answer
    if [ -z "$answer" ]; then
        echo '{escaped_default}' > {output_path}
    else
        echo "$answer" > {output_path}
    fi
fi"#
            )
        }
    }
}

fn cmd_wizard(file: &PathBuf, title: &str) -> Result<()> {
    let content = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read wizard file: {}", file.display()))?;
    let wizard: WizardToml =
        toml::from_str(&content).context("Failed to parse wizard TOML file")?;

    let wizard_title = wizard.title.as_deref().unwrap_or(title);
    let mut answers: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut step_results: Vec<WizardStepResult> = Vec::new();

    let mut step_index = 0;
    while step_index < wizard.steps.len() {
        let step = &wizard.steps[step_index];

        // Check condition
        if let Some(ref cond) = step.condition {
            if !evaluate_condition(cond, &answers) {
                // Skip this step
                if let Some(skip_idx) = step.skip_to {
                    step_index = skip_idx;
                    continue;
                }
                step_index += 1;
                continue;
            }
        }

        // Build output path for this step
        let output_path = format!("/tmp/b00t_zellij_wizard_step_{step_index}");
        let _ = std::fs::remove_file(&output_path);

        let default_val = step.default.as_deref().unwrap_or("");

        let script = build_step_script(
            &step.prompt,
            default_val,
            &step.input_type,
            &step.options,
            &output_path,
            &format!("{wizard_title} — Step {}", step_index + 1),
        );

        if std::env::var("ZELLIJ_SESSION_NAME").is_ok() {
            run_in_zellij_floating(
                &script,
                &format!("{wizard_title} Step {}", step_index + 1),
                true,
            )?;
        } else {
            let mut cmd = Command::new("bash");
            cmd.arg("-c").arg(&script);
            let _output = cmd.output().context("Failed to run wizard step")?;
        }

        // Read the result
        let value = match std::fs::read_to_string(&output_path) {
            Ok(v) => v.trim().to_string(),
            Err(_) => default_val.to_string(),
        };

        let value = if value.is_empty() {
            default_val.to_string()
        } else {
            value
        };

        answers.insert(step.var.clone(), value.clone());
        step_results.push(WizardStepResult {
            var: step.var.clone(),
            value: value.clone(),
        });

        step_index += 1;
    }

    let result = WizardResult {
        steps: step_results,
    };

    let json = serde_json::to_string(&result).context("Failed to serialize wizard result")?;
    println!("{json}");
    Ok(())
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_info_struct() {
        let info = DetectionInfo {
            inside_zellij: false,
            session_name: None,
            pane_id: None,
            pane_index: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"inside_zellij\":false"));
    }

    #[test]
    fn test_parse_quadrant() {
        assert_eq!(
            parse_quadrant("urgent-important"),
            EisenhowerQuadrant::UrgentImportant
        );
        assert_eq!(
            parse_quadrant("schedule"),
            EisenhowerQuadrant::NotUrgentImportant
        );
        assert_eq!(
            parse_quadrant("delegate"),
            EisenhowerQuadrant::UrgentNotImportant
        );
        assert_eq!(
            parse_quadrant("eliminate"),
            EisenhowerQuadrant::NotUrgentNotImportant
        );
        // Unknown defaults to UrgentImportant
        assert_eq!(
            parse_quadrant("unknown"),
            EisenhowerQuadrant::UrgentImportant
        );
    }

    #[test]
    fn test_parse_action() {
        assert_eq!(parse_action("reject"), AgentAction::Reject);
        assert_eq!(parse_action("delegate"), AgentAction::Delegate);
        assert_eq!(parse_action("audit"), AgentAction::Audit);
        // Default is Approve
        assert_eq!(parse_action("unknown"), AgentAction::Approve);
        assert_eq!(parse_action(""), AgentAction::Approve);
    }

    #[test]
    fn test_evaluate_condition_equals() {
        let mut answers = std::collections::HashMap::new();
        answers.insert("color".to_string(), "red".to_string());
        assert!(evaluate_condition("color=red", &answers));
        assert!(!evaluate_condition("color=blue", &answers));
    }

    #[test]
    fn test_evaluate_condition_not_equals() {
        let mut answers = std::collections::HashMap::new();
        answers.insert("color".to_string(), "red".to_string());
        assert!(evaluate_condition("color!=blue", &answers));
        assert!(!evaluate_condition("color!=red", &answers));
    }

    #[test]
    fn test_evaluate_condition_truthy() {
        let mut answers = std::collections::HashMap::new();
        answers.insert("active".to_string(), "true".to_string());
        assert!(evaluate_condition("active", &answers));

        let mut answers2 = std::collections::HashMap::new();
        answers2.insert("inactive".to_string(), "false".to_string());
        assert!(!evaluate_condition("inactive", &answers2));
    }

    #[test]
    fn test_load_menu_items_from_json() {
        let json = r#"[
            {"key": "build", "label": "Build", "quadrant": "urgent-important", "action": "approve"},
            {"key": "test", "label": "Test", "quadrant": "not-urgent-important", "action": "audit", "description": "Run tests"}
        ]"#;
        let items = load_menu_items(Some(json), None).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].key, "build");
        assert_eq!(items[0].quadrant, EisenhowerQuadrant::UrgentImportant);
        assert_eq!(items[1].key, "test");
        assert_eq!(items[1].action, AgentAction::Audit);
        assert_eq!(items[1].description, Some("Run tests".to_string()));
    }

    #[test]
    fn test_wizard_toml_parse() {
        let toml_content = r#"
title = "Test Wizard"

[[steps]]
prompt = "Enter your name"
var = "name"
default = "world"

[[steps]]
prompt = "Proceed?"
var = "confirm"
input_type = "confirm"
"#;
        let wizard: WizardToml = toml::from_str(toml_content).unwrap();
        assert_eq!(wizard.title, Some("Test Wizard".to_string()));
        assert_eq!(wizard.steps.len(), 2);
        assert_eq!(wizard.steps[0].prompt, "Enter your name");
        assert_eq!(wizard.steps[0].var, "name");
        assert_eq!(wizard.steps[1].input_type, "confirm");
    }
}
