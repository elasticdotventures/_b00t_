// k0mmand3r - A library for parsing /slash commands
// Export WASM bindings for Node.js usage

try {
  // Try to load WASM bindings
  const wasm = require('./pkg/k0mmand3r');
  module.exports = wasm;
} catch (e) {
  console.error('k0mmand3r WASM bindings not found. Please build with `wasm-pack build --target nodejs`');
  module.exports = {
    error: 'WASM bindings not available',
    message: 'Run `wasm-pack build --target nodejs` to build k0mmand3r WASM bindings'
  };
}