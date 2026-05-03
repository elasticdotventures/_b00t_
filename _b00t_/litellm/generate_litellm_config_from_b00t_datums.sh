#!/bin/bash
# Generate LiteLLM configuration from b00t AI model datums
# Descriptive filename explains operational purpose and data source

set -euo pipefail

# Find b00t directory (parent of litellm directory)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B00T_DIR="$(dirname "$SCRIPT_DIR")"

# Tool detection - use toml command from PATH or fallback
TOML_CMD=$(which toml 2>/dev/null || echo "toml")

echo "# Auto-generated LiteLLM configuration from b00t AI model datums"
echo "# Generated at: $(date -Iseconds)"
echo "# Source directory: $B00T_DIR"
echo ""
echo "model_list:"

# Change to b00t directory to find AI model datums
cd "$B00T_DIR"
model_files=(*.ai_model.toml)

if [[ ! -f "${model_files[0]}" ]]; then
    echo "# No AI model datums found in $B00T_DIR directory"
    echo "# Create .ai_model.toml files to configure models"
else
    for file in "${model_files[@]}"; do
        [[ -f "$file" ]] || continue
        
        name=$(basename "$file" .ai_model.toml)
        echo "  - model_name: $name"
        echo "    litellm_params:"
        
        # Parse LiteLLM model configuration
        if litellm_model=$($TOML_CMD get "$file" ai_model.litellm_model 2>/dev/null); then
            litellm_model=$(echo "$litellm_model" | tr -d '"')
            echo "      model: $litellm_model"
        else
            echo "      model: openrouter/$name"  # Default to OpenRouter
        fi
        
        # API key configuration
        if api_key_env=$($TOML_CMD get "$file" ai_model.api_key_env 2>/dev/null); then
            api_key_env=$(echo "$api_key_env" | tr -d '"')
            [[ -n "$api_key_env" ]] && echo "      api_key: \"os.environ/$api_key_env\""
        fi
        
        # API base URL
        if api_base=$($TOML_CMD get "$file" ai_model.api_base 2>/dev/null); then
            api_base=$(echo "$api_base" | tr -d '"')
            [[ -n "$api_base" && "$api_base" != '""' ]] && echo "      api_base: $api_base"
        fi
        
        # Rate limiting
        if rpm_limit=$($TOML_CMD get "$file" ai_model.rpm_limit 2>/dev/null); then
            [[ -n "$rpm_limit" && "$rpm_limit" != "null" ]] && echo "      rpm: $rpm_limit"
        fi
        
        # Model parameters from TOML
        if max_tokens=$($TOML_CMD get "$file" ai_model.parameters.max_tokens 2>/dev/null); then
            [[ -n "$max_tokens" && "$max_tokens" != "null" ]] && echo "      max_tokens: $max_tokens"
        fi
        
        if temperature=$($TOML_CMD get "$file" ai_model.parameters.temperature 2>/dev/null); then
            [[ -n "$temperature" && "$temperature" != "null" ]] && echo "      temperature: $temperature"
        fi
        
        echo ""
    done
fi

echo "general_settings:"
echo "  master_key: \"os.environ/LITELLM_MASTER_KEY\""
echo ""
echo "litellm_settings:"
echo "  request_timeout: 600"
echo "  set_verbose: false"
echo "  json_logs: true"