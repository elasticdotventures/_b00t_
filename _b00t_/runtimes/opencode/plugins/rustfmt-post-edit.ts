// b00t rustfmt post-edit plugin for OpenCode
// Mirrors _b00t_/hooks/rustfmt-post-edit for Claude Code.
// Runs `rustfmt --edition 2024` on any .rs file after edit/write tool calls.
//
// 🤓 OpenCode plugin hooks use tool.execute.after — different from Claude Code's
//    PostToolUse JSON hook. Input shape: { tool_name, input: { file_path?, ... } }
import type { Plugin } from "@opencode-ai/plugin";

export default (async ({ $ }) => {
  return {
    "tool.execute.after": async (ctx: {
      tool_name: string;
      input: Record<string, unknown>;
    }) => {
      const name = ctx.tool_name ?? "";
      if (!["edit", "write"].includes(name.toLowerCase())) return;

      const filePath = ctx.input?.file_path as string | undefined;
      if (!filePath?.endsWith(".rs")) return;

      try {
        await $`rustfmt --edition 2024 ${filePath}`;
        console.error(`rustfmt: formatted ${filePath}`);
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        console.error(`rustfmt WARNING on ${filePath}: ${msg}`);
      }
    },
  };
}) satisfies Plugin;
