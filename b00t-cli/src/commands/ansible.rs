use crate::{ansible::run_playbook, get_config};
use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
use shellexpand;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser)]
pub enum AnsibleCommands {
    #[clap(about = "Run an Ansible playbook either directly or via a datum")]
    Run {
        #[clap(long, help = "Datum name that supplies [ansible] metadata (name.type)")]
        datum: Option<String>,

        #[clap(long, help = "Playbook path (overrides datum)")]
        playbook: Option<String>,

        #[clap(long, help = "Inventory file path (overrides datum)")]
        inventory: Option<String>,

        #[clap(
            long = "extra-arg",
            help = "Additional arguments to forward to ansible-playbook",
            action = ArgAction::Append,
            value_name = "ARG"
        )]
        extra_args: Vec<String>,

        #[clap(
            long = "var",
            help = "Extra vars in KEY=VALUE form",
            action = ArgAction::Append,
            value_name = "KEY=VALUE"
        )]
        vars: Vec<String>,
    },
}

impl AnsibleCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            AnsibleCommands::Run {
                datum,
                playbook,
                inventory,
                extra_args,
                vars,
            } => {
                let workspace = PathBuf::from(shellexpand::tilde(path).into_owned());
                let mut config = if let Some(name) = datum {
                    let (cfg, _) = get_config(name, path).map_err(|e| {
                        anyhow!("Failed to load datum '{name}': {e}", name = name, e = e)
                    })?;
                    if let Some(ansible) = cfg.b00t.ansible.clone() {
                        ansible
                    } else {
                        return Err(anyhow!("Datum '{}' has no [ansible] section", name));
                    }
                } else {
                    AnsibleConfigDefaults::empty()
                };

                if let Some(pb) = playbook {
                    config.playbook = pb.clone();
                }

                if let Some(inv) = inventory {
                    config.inventory = Some(inv.clone());
                }

                if !extra_args.is_empty() {
                    config.extra_args = Some(extra_args.clone());
                }

                if !vars.is_empty() {
                    config.extra_vars = Some(parse_vars(vars));
                }

                if config.playbook.trim().is_empty() {
                    return Err(anyhow!("Playbook path required (from datum or --playbook)"));
                }

                run_playbook(&config, Some(workspace.as_path()))
            }
        }
    }
}

fn parse_vars(vars: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for entry in vars {
        let mut parts = entry.splitn(2, '=');
        let key = parts.next().unwrap_or("").to_string();
        let value = parts.next().unwrap_or("").to_string();
        map.insert(key, value);
    }
    map
}

struct AnsibleConfigDefaults;

impl AnsibleConfigDefaults {
    fn empty() -> crate::ansible::AnsibleConfig {
        crate::ansible::AnsibleConfig {
            playbook: String::new(),
            inventory: None,
            extra_vars: None,
            extra_args: None,
        }
    }
}
