#!/usr/bin/env node
/**
 * Recall script - Search Nori knowledge base
 *
 * IMPORTANT: This file is BUNDLED during the build process.
 *
 * Build Process:
 * 1. TypeScript compiles this file to build/src/cli/features/claude-code/profiles/config/senior-swe/skills/paid-recall/script.js
 * 2. tsc-alias converts @ imports to relative paths
 * 3. scripts/bundle-skills.ts uses esbuild to create a standalone bundle
 * 4. The bundle REPLACES the compiled output at the same location
 * 5. Installation copies the bundled script to ~/.claude/skills/recall/script.js
 *
 * Why Bundling:
 * The @ imports below (e.g., @/api/index.js) get converted to relative paths
 * like '../../../../../api/index.js'. When installed to ~/.claude/skills/,
 * those paths don't exist. Bundling inlines all dependencies into a single
 * standalone executable.
 *
 * @see scripts/bundle-skills.ts - The bundler that processes this file
 * @see src/cli/features/claude-code/profiles/skills/loader.ts - Installation to ~/.claude/skills/
 */
/**
 * Main execution function
 */
export declare const main: () => Promise<void>;
//# sourceMappingURL=script.d.ts.map