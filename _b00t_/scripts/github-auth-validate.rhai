// GitHub Authentication Validator for MCP Server
// Ensures GITHUB_PERSONAL_ACCESS_TOKEN is available before starting github-mcp-server
//
// Returns:
//   - Token string if successful
//   - Error with actionable message if failed
//
// Usage:
//   b00t script run github-auth-validate
//   Result will be the token or an error explaining what to do

log_info("🔐 Validating GitHub authentication for MCP server...");

// Check if token is already set in environment
let existing_token = get_env("GITHUB_PERSONAL_ACCESS_TOKEN");
if existing_token != "" {
    log_success("✅ GITHUB_PERSONAL_ACCESS_TOKEN already set in environment");
    return existing_token;
}

// Check if .env file has the token
let workspace = get_env("WORKSPACE_ROOT");
if workspace == "" {
    workspace = get_env("HOME");
}

let env_file = workspace + "/.env";
if file_exists(env_file) {
    let env_content = read_file(env_file);
    let lines = env_content.split("\n");

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("GITHUB_PERSONAL_ACCESS_TOKEN=") {
            let token = trimmed.sub_string(30); // Length of "GITHUB_PERSONAL_ACCESS_TOKEN="
            if token != "" && !token.starts_with("#") {
                log_success("✅ Found GITHUB_PERSONAL_ACCESS_TOKEN in .env file");
                set_env("GITHUB_PERSONAL_ACCESS_TOKEN", token);
                return token;
            }
        }
    }
}

// Token not in env or .env file - try gh CLI
log_info("ℹ️  GITHUB_PERSONAL_ACCESS_TOKEN not found in environment");
log_info("ℹ️  Attempting to retrieve token from gh CLI...");

// Check if gh CLI is available
if !command_exists("gh") {
    log_error("❌ GitHub CLI (gh) not found");
    log_error("ℹ️  Install gh CLI: https://cli.github.com/manual/installation");
    log_error("ℹ️  Or set GITHUB_PERSONAL_ACCESS_TOKEN manually:");
    log_error("   export GITHUB_PERSONAL_ACCESS_TOKEN=ghp_xxxxxxxxxxxx");
    throw "GitHub authentication failed: gh CLI not available and no PAT in environment";
}

// Check if gh is authenticated
let auth_status_result = try {
    run_cmd("gh auth status 2>&1")
} catch(e) {
    log_error("❌ Failed to check gh auth status: " + e);
    throw "GitHub authentication check failed: " + e;
};

if !auth_status_result.contains("Logged in to github.com") {
    log_error("❌ GitHub CLI not authenticated");
    log_error("ℹ️  Run: gh auth login");
    log_error("ℹ️  Or set GITHUB_PERSONAL_ACCESS_TOKEN manually:");
    log_error("   export GITHUB_PERSONAL_ACCESS_TOKEN=ghp_xxxxxxxxxxxx");
    throw "GitHub authentication failed: gh CLI not authenticated (run 'gh auth login'). Check: " + auth_status_result;
}

// Get token from gh CLI
log_info("ℹ️  Retrieving token from gh CLI...");
let token = try {
    let output = run_cmd("gh auth token");
    output.trim()
} catch(e) {
    log_error("❌ Failed to get token from gh CLI: " + e);
    log_error("ℹ️  Try re-authenticating: gh auth login");
    throw "Failed to retrieve GitHub token from gh CLI: " + e;
};

// Validate token format (should start with ghp_, gho_, or github_pat_)
if token == "" {
    log_error("❌ gh auth token returned empty string");
    log_error("ℹ️  Try re-authenticating: gh auth login");
    throw "GitHub token is empty - gh CLI may not be properly authenticated";
}

if !token.starts_with("ghp_") && !token.starts_with("gho_") && !token.starts_with("github_pat_") {
    log_warn("⚠️  Token format unexpected (doesn't start with ghp_/gho_/github_pat_)");
    log_warn("ℹ️  Token: " + token.sub_string(0, 12) + "...");
}

// Set token in environment for MCP server
set_env("GITHUB_PERSONAL_ACCESS_TOKEN", token);

log_success("✅ GitHub token retrieved successfully from gh CLI");
log_info(`ℹ️  Token prefix: ${token.sub_string(0, 8)}...`);
log_info("ℹ️  Token available for github-mcp-server startup");

// Return token for use by caller
return token;
