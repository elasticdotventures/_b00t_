


# Define TOML configuration file path
function toml_init() {
    export TOML_CFGFILE=$(expandPath "~/.dotfiles/$HOSTNAME.toml")
    local TOML_DIR=$(dirname "$TOML_CFGFILE")

    if [ ! -d "$TOML_DIR" ]; then
        log_📢_记录 "🐭 creating TOML config directory: $TOML_DIR"
        mkdir -p "$TOML_DIR"
        chmod 750 "$TOML_DIR"
    fi

    if [ ! -f "$TOML_CFGFILE" ]; then
        log_📢_记录 "🐭 initializing TOML config file: $TOML_CFGFILE"
        # Initialize with default values
        echo "[b00t]" >> "$TOML_CFGFILE"
    else
        log_📢_记录 "🐭 TOML config file exists: $TOML_CFGFILE"
    fi
}

function toml_set() {
    local section=$1
    local key=$2
    local value=$3
    toml set "$TOML_CFGFILE" "${section}.${key}" "$value"
}

function toml_get() {
    local section=$1
    local key=$2
    toml get "$TOML_CFGFILE" "${section}.${key}"
}

function toml_seq() {
    local seqlabel=$1
    local x=$(toml_get "b00t" "$seqlabel")
    if [ -z "$x" ]; then x="0"; fi
    x=$((x + 1))
    toml_set "b00t" "$seqlabel" "$x"
    echo "$x"
}

function toml_ok() {
    if [ -f "$TOML_CFGFILE" ]; then
        local x=$(toml_seq "crudini_check")
        log_📢_记录 "🐭🥾 TOML _seq: #$x $TOML_CFGFILE"
        return 0
    else
        log_📢_记录 "🐭🍒 TOML config missing: $TOML_CFGFILE"
        return 1
    fi
}

# Initialize TOML configuration
toml_init
#toml_ok

