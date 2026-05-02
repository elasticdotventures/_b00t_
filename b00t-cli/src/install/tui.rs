use crate::install::adapter::{AdapterRegistry, InstallScope, RuntimeId};
use crate::install::content::ContentPackId;
use anyhow::Result;
use inquire::{Confirm, MultiSelect, Select};

pub struct InstallSelection {
    pub runtimes: Vec<RuntimeId>,
    pub scope: InstallScope,
    pub content_packs: Vec<ContentPackId>,
}

pub fn run_tui(registry: &AdapterRegistry) -> Result<InstallSelection> {
    println!("🥾 b00t installer\n");

    // Runtime selection — detected runtimes shown with [detected] suffix
    let all_adapters = registry.all_adapters();
    let options: Vec<String> = all_adapters
        .iter()
        .map(|a| {
            let detected = if a.detect() {
                " [detected]"
            } else {
                " [not detected]"
            };
            format!("{}{}", a.id().display_name(), detected)
        })
        .collect();

    let selected_options = MultiSelect::new("Which runtimes to configure?", options.clone())
        .with_default(
            &all_adapters
                .iter()
                .enumerate()
                .filter(|(_, a)| a.detect())
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
        )
        .prompt()?;

    let runtimes: Vec<RuntimeId> = all_adapters
        .iter()
        .zip(options.iter())
        .filter(|(_, opt)| selected_options.contains(opt))
        .map(|(a, _)| a.id())
        .collect();

    if runtimes.is_empty() {
        anyhow::bail!("No runtimes selected. Aborting.");
    }

    // Scope selection
    let scope_choice = Select::new(
        "Install scope?",
        vec!["Global (user home dirs)", "Local (current directory)"],
    )
    .prompt()?;
    let scope = match scope_choice {
        "Global (user home dirs)" => InstallScope::Global,
        _ => InstallScope::Local(std::env::current_dir()?),
    };

    // Content pack selection — all selected by default
    let pack_names: Vec<String> = ContentPackId::all()
        .iter()
        .map(|p| p.display_name().to_string())
        .collect();
    let selected_packs = MultiSelect::new("Content packs?", pack_names.clone())
        .with_default(&(0..pack_names.len()).collect::<Vec<_>>())
        .prompt()?;

    let content_packs: Vec<ContentPackId> = ContentPackId::all()
        .into_iter()
        .filter(|p| selected_packs.contains(&p.display_name().to_string()))
        .collect();

    // Confirm
    let runtime_names: Vec<&str> = runtimes.iter().map(|r| r.display_name()).collect();
    let scope_str = match &scope {
        InstallScope::Global => "globally".to_string(),
        InstallScope::Local(p) => format!("locally in {}", p.display()),
    };
    let confirmed = Confirm::new(&format!(
        "Ready to install for [{}] {}?",
        runtime_names.join(", "),
        scope_str
    ))
    .with_default(true)
    .prompt()?;

    if !confirmed {
        anyhow::bail!("Installation cancelled.");
    }

    Ok(InstallSelection {
        runtimes,
        scope,
        content_packs,
    })
}

/// Non-interactive selection for CI/scripting (--yes flag)
pub fn headless_selection(
    runtimes: Vec<RuntimeId>,
    scope: InstallScope,
    content_packs: Vec<ContentPackId>,
) -> InstallSelection {
    InstallSelection {
        runtimes,
        scope,
        content_packs,
    }
}
