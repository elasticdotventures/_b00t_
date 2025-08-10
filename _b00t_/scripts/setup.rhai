// B00t System Setup Script - RHAI Implementation
// Replaces setup.sh with safer, more maintainable scripting
// Uses b00t context variables and standardized functions

log_info("🥾 Starting b00t system setup...");

// Environment detection
let is_ci = IS_CI;
let is_docker = IS_DOCKER;

log_info(`Environment: CI=${is_ci}, Docker=${is_docker}`);

// Package management helper
fn run_with_sudo(cmd, is_docker_env) {
    if is_docker_env {
        return run_cmd(cmd);
    } else {
        return run_cmd("sudo " + cmd);
    }
}

// Core system updates
log_info("📦 Updating package lists...");
run_with_sudo("apt update && apt upgrade -y", is_docker);
run_with_sudo("apt install software-properties-common -y", is_docker);

// GitHub CLI installation
if !command_exists("gh") {
    log_info("📥 Installing GitHub CLI...");
    
    // Ensure curl is available
    if !command_exists("curl") {
        run_with_sudo("apt update && apt install curl -y", is_docker);
    }
    
    let gh_install_cmd = `
        curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | ${if is_docker { "" } else { "sudo " }}dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg &&
        ${if is_docker { "" } else { "sudo " }}chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg &&
        echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | ${if is_docker { "" } else { "sudo " }}tee /etc/apt/sources.list.d/github-cli.list > /dev/null &&
        ${if is_docker { "" } else { "sudo " }}apt update &&
        ${if is_docker { "" } else { "sudo " }}apt install git gh -y
    `;
    
    run_cmd(gh_install_cmd);
}

// GitHub authentication
if file_exists("/run/secrets/GH_TOKEN") {
    log_info("🔐 Configuring GitHub authentication...");
    run_cmd("gh auth login --with-token < /run/secrets/GH_TOKEN");
}

// Check GitHub auth status
fn check_github_auth() {
    let status_output = run_cmd("gh auth status 2>&1");
    return status_output.contains("✓ Logged in to github.com");
}

if check_github_auth() {
    log_success("✅ GitHub authentication verified");
    
    // Install GitHub extensions
    run_cmd("gh extension install https://github.com/nektos/gh-act");
    run_cmd("gh extension install github/gh-copilot --force");
} else {
    log_warn("❌ GitHub not authenticated - skipping extensions");
}

// GNU Stow for dotfiles management
if !command_exists("stow") {
    log_info("📁 Installing GNU Stow...");
    run_with_sudo("apt install stow -y", is_docker);
}

// Clone dotfiles if needed
let dotfiles_dir = USER + "/.dotfiles";
if !file_exists(dotfiles_dir) {
    log_info("📥 Cloning dotfiles repository...");
    run_cmd(`gh repo clone elasticdotventures/dotfiles ${dotfiles_dir} -- --depth 1`);
}

// Apply dotfiles with stow
log_info("🔗 Linking dotfiles...");
run_cmd(`stow -d ${dotfiles_dir} -t ${USER} bash`);

// Essential development tools
log_info("🛠️  Installing development tools...");
run_with_sudo("apt install -y build-essential joe libnotify-bin bc", is_docker);

// Git configuration (skip in CI)
if !is_ci {
    log_info("⚙️  Configuring Git...");
    run_cmd(`git config --global user.email "brianh@elastic.ventures"`);
    run_cmd(`git config --global user.name "Brian H"`);
}

// Time synchronization (skip in Docker)
if !is_docker {
    log_info("🕐 Synchronizing system time...");
    run_with_sudo("apt install -y ntpdate", is_docker);
    run_with_sudo("ntpdate pool.ntp.org", is_docker);
}

// Core utilities
log_info("🔧 Installing core utilities...");
run_with_sudo("apt update -yy", is_docker);
run_with_sudo("apt-get install -y jq fzf ripgrep tree unzip", is_docker);

// OpenTofu
if !command_exists("tofu") {
    log_info("🏗️  Installing OpenTofu...");
    run_cmd("curl --proto '=https' --tlsv1.2 -fsSL 'https://packages.opentofu.org/install/repositories/opentofu/tofu/script.deb.sh?any=true' -o /tmp/tofu-repository-setup.sh");
    run_with_sudo("bash /tmp/tofu-repository-setup.sh", is_docker);
    run_cmd("rm /tmp/tofu-repository-setup.sh");
    run_with_sudo("apt-get install -y tofu", is_docker);
}

// Rust toolchain
if !command_exists("rustc") {
    log_info("🦀 Installing Rust toolchain...");
    run_cmd("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y");
    // Source cargo env in current context
    set_env("PATH", get_env("PATH") + ":" + get_env("HOME") + "/.cargo/bin");
} else {
    log_info("🦀 Updating Rust toolchain...");
    run_cmd("rustup update");
}

// Starship prompt
log_info("🚀 Installing Starship prompt...");
run_with_sudo("apt-get update", is_docker);
run_with_sudo("apt install -y cmake pkg-config", is_docker);
run_cmd("cargo install starship --locked");

let bashrc_path = get_env("HOME") + "/.bashrc";
if file_exists(bashrc_path) {
    let bashrc_content = read_file(bashrc_path);
    if !bashrc_content.contains("starship init bash") {
        write_file(bashrc_path, bashrc_content + "\neval \"$(starship init bash)\"\n");
    }
}

// Bun JavaScript runtime
if !command_exists("bun") {
    log_info("🍞 Installing Bun...");
    run_cmd("curl -fsSL https://bun.sh/install | bash");
    set_env("PATH", get_env("PATH") + ":" + get_env("HOME") + "/.bun/bin");
}

if command_exists("bun") {
    run_cmd("bun add dotenv");
}

// Dotenvy for Rust
log_info("📄 Installing dotenvy CLI...");
run_cmd("cargo install dotenvy --bin dotenvy --features cli");

// Ripgrep (ensure it's available)
if !command_exists("rg") {
    run_with_sudo("apt-get install -y ripgrep", is_docker);
}

// Just command runner
if !command_exists("just") {
    log_info("⚡ Installing Just command runner...");
    run_cmd("curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | " + (if is_docker { "bash -s -- --to /usr/local/bin" } else { "sudo bash -s -- --to /usr/local/bin" }));
}

// Clean up old yq PPA
let old_yq_ppa = "/etc/apt/sources.list.d/rmescandon-ubuntu-yq-noble.sources";
if file_exists(old_yq_ppa) {
    log_info("🧹 Removing old yq PPA...");
    run_with_sudo(`rm ${old_yq_ppa}`, is_docker);
}

// eget for GitHub releases
if !command_exists("eget") {
    log_info("📦 Installing eget...");
    run_cmd("curl -s https://zyedidia.github.io/eget.sh | sh");
    create_dir(get_env("HOME") + "/.local/bin");
    run_cmd("mv eget " + get_env("HOME") + "/.local/bin/eget");
    set_env("PATH", get_env("PATH") + ":" + get_env("HOME") + "/.local/bin");
}

// yq using eget
log_info("📊 Installing yq...");
run_cmd(get_env("HOME") + "/.local/bin/eget mikefarah/yq --upgrade-only --tag v4.44.1 --asset yq_linux_amd64.tar.gz --all");
run_cmd("mv yq_linux_amd64 " + get_env("HOME") + "/.local/bin/yq");

// bat (better cat)
log_info("🦇 Installing bat...");
run_with_sudo("apt install bat -y", is_docker);
let bat_link = get_env("HOME") + "/.local/bin/bat";
if !file_exists(bat_link) {
    run_cmd(`ln -s /usr/bin/batcat ${bat_link}`);
}

// Development dependencies
log_info("🔗 Installing development dependencies...");
run_with_sudo("apt install -y libclang-dev", is_docker);

// wasm-to-oci
if !command_exists("wasm-to-oci") {
    log_info("🌐 Installing wasm-to-oci...");
    run_cmd("wget https://github.com/engineerd/wasm-to-oci/releases/download/v0.1.2/linux-amd64-wasm-to-oci");
    run_cmd("mv linux-amd64-wasm-to-oci wasm-to-oci");
    run_cmd("chmod +x wasm-to-oci");
    run_with_sudo("cp wasm-to-oci /usr/local/bin", is_docker);
}

// AWS CLI
if !command_exists("aws") {
    log_info("☁️  Installing AWS CLI...");
    run_cmd("cd /tmp");
    run_cmd("curl \"https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip\" -o \"awscliv2.zip\"");
    run_cmd("unzip -q awscliv2.zip");
    run_with_sudo("./aws/install", is_docker);
}

// Google Cloud CLI
if !command_exists("gcloud") {
    log_info("🌤️  Installing Google Cloud CLI...");
    run_with_sudo("apt-get install apt-transport-https ca-certificates gnupg curl -y", is_docker);
    run_cmd("curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | " + (if is_docker { "" } else { "sudo " }) + "gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg");
    run_cmd("echo \"deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main\" | " + (if is_docker { "" } else { "sudo " }) + "tee -a /etc/apt/sources.list.d/google-cloud-sdk.list");
    run_with_sudo("apt-get update && apt-get install google-cloud-cli -y", is_docker);
    
    if !is_ci {
        let gcloud_configs = run_cmd("gcloud config configurations list");
        if !gcloud_configs.contains("NAME") {
            run_cmd("gcloud init");
        }
        run_cmd("gcloud config set compute/zone australia-southeast2-c");
    }
}

// kubectl completion (skip in CI)
if !is_ci && command_exists("kubectl") {
    let kube_completion = get_env("HOME") + "/.kube/completion.bash.inc";
    if !file_exists(kube_completion) {
        log_info("⚙️  Setting up kubectl completion...");
        create_dir(get_env("HOME") + "/.kube");
        run_cmd("kubectl completion bash > " + kube_completion);
        
        let bash_profile = get_env("HOME") + "/.bash_profile";
        let completion_source = "\n# kubectl shell completion\nsource '" + kube_completion + "'\n";
        
        if file_exists(bash_profile) {
            write_file(bash_profile, read_file(bash_profile) + completion_source);
        } else {
            write_file(bash_profile, completion_source);
        }
    }
}

// Additional tools
log_info("🛠️  Installing additional tools...");

// inotify-tools
if !command_exists("inotifywait") {
    run_with_sudo("apt-get install -y inotify-tools", is_docker);
}

// uv Python package manager
if !command_exists("uv") {
    log_info("🐍 Installing uv Python package manager...");
    run_cmd("curl -LsSf https://astral.sh/uv/install.sh | sh");
}

// DataFusion CLI
if !command_exists("datafusion-cli") {
    log_info("📊 Installing DataFusion CLI...");
    run_cmd("cargo install datafusion-cli");
}

// yek
log_info("🔑 Installing yek...");
run_cmd("cargo install --git https://github.com/bodo-run/yek");

// direnv
if !command_exists("direnv") {
    log_info("📁 Installing direnv...");
    run_cmd("curl -sfL https://direnv.net/install.sh | bash");
}

// pixi
if !command_exists("pixi") {
    log_info("🥧 Installing pixi...");
    run_cmd("curl -fsSL https://pixi.sh/install.sh | bash");
}

// cargo-binstall
if !command_exists("cargo-binstall") {
    log_info("📦 Installing cargo-binstall...");
    run_cmd("curl -LsSf https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-gnu.tgz | tar -xz -C ~/.cargo/bin");
}
run_cmd("cargo binstall podlet");

// Package management for non-CI environments
if !is_ci {
    // Homebrew
    if !command_exists("brew") {
        log_info("🍺 Installing Homebrew...");
        run_cmd("/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"");
        
        let linuxbrew_bin = "/home/linuxbrew/.linuxbrew/bin/brew";
        if file_exists(linuxbrew_bin) {
            run_cmd(linuxbrew_bin + " shellenv");
            let bashrc_content = read_file(get_env("HOME") + "/.bashrc");
            write_file(get_env("HOME") + "/.bashrc", bashrc_content + "\neval \"$(" + linuxbrew_bin + " shellenv)\"\n");
        }
    }
    
    // asdf via Homebrew
    if command_exists("brew") && !command_exists("asdf") {
        log_info("🔧 Installing asdf via Homebrew...");
        run_cmd("brew install asdf");
    }
    
    // Go via asdf
    if command_exists("asdf") {
        let golang_check = run_cmd("asdf list golang 2>&1");
        if golang_check.contains("No versions installed") {
            log_info("🐹 Installing Go via asdf...");
            run_cmd("asdf plugin add golang https://github.com/asdf-community/asdf-golang.git");
            run_cmd("asdf install golang 1.24.1");
            run_cmd("asdf global golang 1.24.1");
        }
    }
}

// Python tools via uv
log_info("🐍 Installing Python tools...");
run_cmd("uv tool install huggingface_hub[cli]");
run_cmd("uv tool install ramalama");

// Additional utilities
run_cmd("uvx honcho version");
run_cmd("uvx bpytop -v");

// System utilities
log_info("🔧 Installing system utilities...");
run_with_sudo("apt-get -y install tmate", is_docker);  // Terminal sharing
run_with_sudo("apt-get install -y entr", is_docker);   // File watcher
run_with_sudo("apt-get install -y nmap", is_docker);   // Network mapper

// Node.js tools
if command_exists("bun") {
    log_info("📦 Installing Node.js tools...");
    run_cmd("bun install -g @buger/probe");
}

// Python multiplexer
run_cmd("uv tool install multiplex-sh");
run_cmd("multiplex --help");

// Claude Code plugins
log_info("🤖 Installing Claude Code plugins...");
run_cmd("curl -sSL https://raw.githubusercontent.com/brennercruvinel/CCPlugins/main/install.sh | bash");

// OpenCode AI
if command_exists("npm") {
    run_cmd("npm i -g opencode-ai@latest");
}

log_success("🎉 b00t system setup completed successfully!");
log_info(`Setup completed for user: ${USER} on ${HOSTNAME}`);
log_info(`Total setup time: ${TIMESTAMP}`);