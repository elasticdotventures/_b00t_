use anyhow::{Context, Result};
use clap::Parser;

use b00t_cli::budget_controller::{BudgetAction, BudgetController, BudgetState, BudgetStatus};
use b00t_cli::datum_stack::StackDatum;
use b00t_cli::get_expanded_path;

#[derive(Parser)]
pub enum BudgetCommands {
    #[clap(
        about = "Check budget status for a stack",
        long_about = "Check current budget spending and status for a stack.\n\nExamples:\n  b00t-cli budget check llm-inference-pipeline\n  b00t-cli budget check llm-inference-pipeline --json"
    )]
    Check {
        #[clap(help = "Stack name")]
        name: String,
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Simulate job execution with budget check",
        long_about = "Simulate running a job and check if it would be allowed based on budget.\n\nExamples:\n  b00t-cli budget simulate llm-inference-pipeline\n  b00t-cli budget simulate llm-inference-pipeline --spent 50 --jobs 20"
    )]
    Simulate {
        #[clap(help = "Stack name")]
        name: String,
        #[clap(long, help = "Current spending today", default_value = "0")]
        spent: f64,
        #[clap(long, help = "Jobs completed today", default_value = "0")]
        jobs: u32,
    },
    #[clap(
        about = "Initialize budget tracking for a stack",
        long_about = "Initialize budget state for a stack. This would normally be done automatically by the k8s controller.\n\nExamples:\n  b00t-cli budget init llm-inference-pipeline"
    )]
    Init {
        #[clap(help = "Stack name")]
        name: String,
    },
    #[clap(
        about = "Record a job completion",
        long_about = "Record a completed job and update budget state. This would normally be done automatically by the k8s controller.\n\nExamples:\n  b00t-cli budget record llm-inference-pipeline"
    )]
    Record {
        #[clap(help = "Stack name")]
        name: String,
        #[clap(
            long,
            help = "Current budget state file",
            default_value = "/tmp/budget-state.json"
        )]
        state_file: String,
    },
}

impl BudgetCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            BudgetCommands::Check { name, json } => check_budget(name, path, *json),
            BudgetCommands::Simulate { name, spent, jobs } => {
                simulate_job(name, path, *spent, *jobs)
            }
            BudgetCommands::Init { name } => init_budget(name, path),
            BudgetCommands::Record { name, state_file } => record_job(name, path, state_file),
        }
    }
}

/// Check budget status for a stack
fn check_budget(name: &str, path: &str, json_output: bool) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;

    // Get budget constraints from orchestration metadata
    let Some(orchestration) = &stack.datum.orchestration else {
        anyhow::bail!("Stack '{}' has no orchestration metadata", name);
    };

    let Some(budget_constraint) = &orchestration.budget_constraint else {
        anyhow::bail!("Stack '{}' has no budget constraints defined", name);
    };

    let daily_limit = budget_constraint.daily_limit.unwrap_or(0.0);
    let cost_per_job = budget_constraint.cost_per_job.unwrap_or(0.0);
    let currency = orchestration.budget_currency.as_deref().unwrap_or("USD");

    // For demonstration, load or initialize budget state
    let state_file = format!("/tmp/budget-{}.json", name);
    let state = if std::path::Path::new(&state_file).exists() {
        let content = std::fs::read_to_string(&state_file)?;
        BudgetController::parse_budget_state(&content)?
    } else {
        BudgetController::init_budget_state()
    };

    if json_output {
        let output = serde_json::json!({
            "stack": name,
            "daily_limit": daily_limit,
            "cost_per_job": cost_per_job,
            "currency": currency,
            "spent_today": state.spent_today,
            "jobs_completed": state.jobs_completed,
            "remaining": daily_limit - state.spent_today,
            "status": format!("{:?}", state.status),
            "percentage": (state.spent_today / daily_limit) * 100.0,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("💰 Budget Status for '{}'", name);
        println!();
        println!("Daily Limit:      {:.2} {}", daily_limit, currency);
        println!("Cost per Job:     {:.2} {}", cost_per_job, currency);
        println!("Spent Today:      {:.2} {}", state.spent_today, currency);
        println!(
            "Remaining:        {:.2} {}",
            daily_limit - state.spent_today,
            currency
        );
        println!("Jobs Completed:   {}", state.jobs_completed);
        println!(
            "Usage:            {:.1}%",
            (state.spent_today / daily_limit) * 100.0
        );
        println!();
        println!(
            "Status: {}",
            match state.status {
                BudgetStatus::Ok => "✅ OK",
                BudgetStatus::Warning => "⚠️  WARNING (>80%)",
                BudgetStatus::Exceeded => "🚫 EXCEEDED",
            }
        );
    }

    Ok(())
}

/// Simulate job execution with budget check
fn simulate_job(name: &str, path: &str, spent_today: f64, jobs_completed: u32) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;

    let Some(orchestration) = &stack.datum.orchestration else {
        anyhow::bail!("Stack '{}' has no orchestration metadata", name);
    };

    let Some(budget_constraint) = &orchestration.budget_constraint else {
        anyhow::bail!("Stack '{}' has no budget constraints defined", name);
    };

    let daily_limit = budget_constraint.daily_limit.unwrap_or(0.0);
    let cost_per_job = budget_constraint.cost_per_job.unwrap_or(0.0);
    let on_exceeded = budget_constraint
        .on_budget_exceeded
        .as_deref()
        .unwrap_or("defer");
    let currency = orchestration.budget_currency.as_deref().unwrap_or("USD");

    // Create budget state
    let state = BudgetState {
        spent_today,
        jobs_completed,
        last_reset: chrono::Utc::now().to_rfc3339(),
        status: if spent_today > daily_limit {
            BudgetStatus::Exceeded
        } else if (spent_today / daily_limit) >= 0.8 {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Ok
        },
    };

    // Check budget
    let controller = BudgetController::new(None);
    let action = controller.check_budget(name, &state, daily_limit, cost_per_job, on_exceeded);

    println!("🧪 Budget Simulation for '{}'", name);
    println!();
    println!("Current State:");
    println!(
        "  Spent Today:     {:.2} {} ({:.1}%)",
        spent_today,
        currency,
        (spent_today / daily_limit) * 100.0
    );
    println!("  Jobs Completed:  {}", jobs_completed);
    println!("  Daily Limit:     {:.2} {}", daily_limit, currency);
    println!();
    println!("Simulating Job:");
    println!("  Cost per Job:    {:.2} {}", cost_per_job, currency);
    println!(
        "  Projected Total: {:.2} {} ({:.1}%)",
        spent_today + cost_per_job,
        currency,
        ((spent_today + cost_per_job) / daily_limit) * 100.0
    );
    println!();
    println!(
        "Decision: {}",
        match action {
            BudgetAction::Allow => "✅ ALLOW - Job can proceed",
            BudgetAction::Defer => "⏸️  DEFER - Wait until budget resets",
            BudgetAction::Alert => "⚠️  ALERT - Job allowed but send notification",
            BudgetAction::Cancel => "🚫 CANCEL - Job permanently cancelled",
        }
    );
    println!();
    println!("Policy on Exceeded: {}", on_exceeded);

    Ok(())
}

/// Initialize budget tracking for a stack
fn init_budget(name: &str, path: &str) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let state = BudgetController::init_budget_state();
    let state_json = BudgetController::serialize_budget_state(&state)?;

    // Save to file
    let state_file = format!("/tmp/budget-{}.json", name);
    std::fs::write(&state_file, &state_json)
        .context(format!("Failed to write budget state to {}", state_file))?;

    println!("✅ Initialized budget tracking for '{}'", name);
    println!("   State file: {}", state_file);
    println!();
    println!("Budget state:");
    println!("  Spent today: {:.2}", state.spent_today);
    println!("  Jobs completed: {}", state.jobs_completed);
    println!("  Status: {:?}", state.status);

    Ok(())
}

/// Record a job completion
fn record_job(name: &str, path: &str, state_file: &str) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;

    let Some(orchestration) = &stack.datum.orchestration else {
        anyhow::bail!("Stack '{}' has no orchestration metadata", name);
    };

    let Some(budget_constraint) = &orchestration.budget_constraint else {
        anyhow::bail!("Stack '{}' has no budget constraints defined", name);
    };

    let daily_limit = budget_constraint.daily_limit.unwrap_or(0.0);
    let cost_per_job = budget_constraint.cost_per_job.unwrap_or(0.0);

    // Load current state
    let state_content = std::fs::read_to_string(state_file)
        .context(format!("Failed to read state file: {}", state_file))?;
    let mut state = BudgetController::parse_budget_state(&state_content)?;

    // Record completion
    let mut controller = BudgetController::new(None);
    controller.record_job_completion(name, cost_per_job, &mut state, daily_limit)?;

    // Save updated state
    let state_json = BudgetController::serialize_budget_state(&state)?;
    std::fs::write(state_file, &state_json)
        .context(format!("Failed to write budget state to {}", state_file))?;

    println!("✅ Recorded job completion for '{}'", name);
    println!();
    println!("Updated Budget State:");
    println!(
        "  Spent Today:     {:.2} ({:.1}%)",
        state.spent_today,
        (state.spent_today / daily_limit) * 100.0
    );
    println!("  Jobs Completed:  {}", state.jobs_completed);
    println!("  Status:          {:?}", state.status);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_commands_exist() {
        // Just ensure commands parse correctly
        let check_cmd = BudgetCommands::Check {
            name: "test".to_string(),
            json: false,
        };
        assert!(matches!(check_cmd, BudgetCommands::Check { .. }));

        let simulate_cmd = BudgetCommands::Simulate {
            name: "test".to_string(),
            spent: 50.0,
            jobs: 20,
        };
        assert!(matches!(simulate_cmd, BudgetCommands::Simulate { .. }));
    }
}
