
use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;
use std::fs;
use regex::Regex;
use duct::cmd;
use semver::Version;

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    #[clap(short, long, default_value = "~/.dotfiles/_b00t_")]
    path: String,
}

#[derive(Parser)]
enum Commands {
    #[clap(about = "Detect the version of a command")]
    Detect {
        #[clap(help = "The command to detect")]
        command: String,
    },
    #[clap(about = "Show the desired version of a command")]
    Desires {
        #[clap(help = "The command to check")]
        command: String,
    },
    #[clap(about = "Install a command")]
    Install {
        #[clap(help = "The command to install")]
        command: String,
    },
    #[clap(about = "Update a command")]
    Update {
        #[clap(help = "The command to update")]
        command: String,
    },
    #[clap(name = ".", about = "Compare installed and desired versions")]
    Dot {
        #[clap(help = "The command to check")]
        command: String,
    },
    #[clap(about = "Update all commands")]
    Up,
}

#[derive(Deserialize, Debug)]
struct Config {
    b00t: BootConfig,
}

#[derive(Deserialize, Debug)]
struct BootConfig {
    name: String,
    desires: String,
    install: String,
    update: Option<String>,
    version: String,
    version_regex: Option<String>,
    hint: String,
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Detect { command } => detect(command, &cli.path),
        Commands::Desires { command } => desires(command, &cli.path),
        Commands::Install { command } => install(command, &cli.path),
        Commands::Update { command } => update(command, &cli.path),
        Commands::Dot { command } => dot(command, &cli.path),
        Commands::Up => up(&cli.path),
    }
}

fn get_config(command: &str, path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let mut path_buf = PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    path_buf.push(format!("{}.toml", command));

    if !path_buf.exists() {
        eprintln!("{} UNDEFINED", command);
        std::process::exit(100);
    }

    let content = fs::read_to_string(&path_buf)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

fn detect(command: &str, path: &str) {
    if let Some(version) = get_installed_version(command, path) {
        println!("{}", version);
    } else {
        eprintln!("🥾😱 {} MISSING", command);
        std::process::exit(2);
    }
}

fn desires(command: &str, path: &str) {
    match get_config(command, path) {
        Ok(config) => {
            println!("{}", config.b00t.desires);
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

fn install(command: &str, path: &str) {
    match get_config(command, path) {
        Ok(config) => {
            println!("Installing {}...", command);
            let install_cmd = &config.b00t.install;
            let result = cmd!("bash", "-c", install_cmd).run();
            match result {
                Ok(_) => println!("Installation complete."),
                Err(e) => eprintln!("Installation failed: {}", e),
            }
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

fn update(command: &str, path: &str) {
    match get_config(command, path) {
        Ok(config) => {
            let update_cmd = config.b00t.update.as_ref().unwrap_or(&config.b00t.install);
            println!("Updating {}...", command);
            let result = cmd!("bash", "-c", update_cmd).run();
            match result {
                Ok(_) => println!("Update complete."),
                Err(e) => eprintln!("Update failed: {}", e),
            }
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

fn get_installed_version(command: &str, path: &str) -> Option<String> {
    if let Ok(config) = get_config(command, path) {
        let version_cmd = &config.b00t.version;
        if let Ok(output) = cmd!("bash", "-c", version_cmd).read() {
            let re = Regex::new(config.b00t.version_regex.as_deref().unwrap_or("\\d+\\.\\d+\\.\\d+")).unwrap();
            if let Some(caps) = re.captures(&output) {
                return Some(caps[0].to_string());
            }
        }
    }
    None
}

fn dot(command: &str, path: &str) {
    let desired_version_str = match get_config(command, path) {
        Ok(config) => config.b00t.desires,
        Err(e) => {
            eprintln!("Error reading config for {}: {}", command, e);
            std::process::exit(1);
        }
    };

    let desired_version = Version::parse(&desired_version_str).unwrap();

    if let Some(installed_version_str) = get_installed_version(command, path) {
        let installed_version = Version::parse(&installed_version_str).unwrap();

        if installed_version == desired_version {
            println!("🥾👍🏻 {}", command);
            std::process::exit(0);
        } else if installed_version > desired_version {
            println!("🥾🐣 {} {}", command, installed_version);
            std::process::exit(0);
        } else {
            println!("🥾😭 {} IS {} WANTS {}", command, installed_version, desired_version);
            std::process::exit(1);
        }
    } else {
        println!("🥾😱 {} MISSING", command);
        std::process::exit(2);
    }
}

fn up(path: &str) {
    let expanded_path = shellexpand::tilde(path).to_string();
    let entries = match fs::read_dir(&expanded_path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading directory {}: {}", &expanded_path, e);
            return;
        }
    };

    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if entry_path.extension().and_then(|s| s.to_str()) == Some("toml") {
                if let Some(command) = entry_path.file_stem().and_then(|s| s.to_str()) {
                    let desired_version_str = match get_config(command, path) {
                        Ok(config) => config.b00t.desires,
                        Err(_) => continue,
                    };

                    let desired_version = Version::parse(&desired_version_str).unwrap();

                    if let Some(installed_version_str) = get_installed_version(command, path) {
                        let installed_version = Version::parse(&installed_version_str).unwrap();

                        if installed_version < desired_version {
                            println!("Updating {}...", command);
                            update(command, path);
                        }
                    } else {
                        println!("Installing {}...", command);
                        install(command, path);
                    }
                }
            }
        }
    }
}
