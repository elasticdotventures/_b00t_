use wasm_bindgen::prelude::*;

// Import the `console.log` function from the browser
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Define a macro for easier console logging
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

/// Core b00t framework version info
#[wasm_bindgen]
pub fn b00t_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Core b00t greeting - stay aligned!
#[wasm_bindgen]
pub fn b00t_greet(name: &str) -> String {
    format!("🥾 Hello {}, welcome to b00t! Stay aligned, get cake! 🎂", name)
}

/// Check if a command looks like a slash command
#[wasm_bindgen]
pub fn is_slash_command(input: &str) -> bool {
    input.trim_start().starts_with('/')
}

/// Parse slash command (simplified version)
#[wasm_bindgen]
pub fn parse_slash_command(input: &str) -> String {
    let trimmed = input.trim_start();
    if let Some(command_part) = trimmed.strip_prefix('/') {
        let parts: Vec<&str> = command_part.split_whitespace().collect();
        if let Some(cmd) = parts.first() {
            format!("Command: {}, Args: {:?}", cmd, &parts[1..])
        } else {
            "Empty command".to_string()
        }
    } else {
        "Not a slash command".to_string()
    }
}

/// Initialize b00t core - call this when loading
#[wasm_bindgen(start)]
pub fn main() {
    console_log!("🥾 b00t-c0re initialized - version {}", b00t_version());
}