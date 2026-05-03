# 🥾 b00t-c0re

Core Rust functionality for the b00t framework with WebAssembly bindings.

## 🚀 Installation

```bash
npm install b00t-c0re
```

## 🔧 Usage

```typescript
import init, { b00t_greet, b00t_version, is_slash_command, parse_slash_command } from 'b00t-c0re';

// Initialize the WASM module
await init();

// Use b00t core functions
console.log(b00t_version()); // "0.1.0"
console.log(b00t_greet("Agent")); // "🥾 Hello Agent, welcome to b00t! Stay aligned, get cake! 🍰"

// Check slash commands
console.log(is_slash_command("/help")); // true
console.log(parse_slash_command("/learn rust")); // "Command: learn, Args: ["rust"]"
```

## 🤓 API

### Functions

- `b00t_version()` - Returns the current version
- `b00t_greet(name: string)` - Greets with b00t alignment message
- `is_slash_command(input: string)` - Checks if input is a slash command
- `parse_slash_command(input: string)` - Parses slash command structure

## 🔗 Related Packages

- **`b00t-mcp`** - Model Context Protocol server
- **`k0mmand3r`** - Full slash command parser with Rust/Python/TypeScript bindings

## 📋 Development

```bash
# Build for Node.js
npm run build

# Build for web
npm run build:web

# Test
npm test
```

## 🥾 About b00t

Part of the b00t extreme programming agent framework.
Visit: https://github.com/elasticdotventures/dotfiles

*Stay aligned, get cake!* 🍰

## 📄 License

MIT