// b00t-opencode-plugin — /b00t slash command with live b00t admin API integration
// Session-scoped skill enable/disable with system-prompt injection
export default {
  id: "b00t-opencode-plugin",

  activate: async (client) => {
    const ADMIN = "http://127.0.0.1:31337/api/admin";
    const state = { enabled: new Set(["goal","mcp","cad","blessed","viz","ooda","rhai","finetune"]) };

    async function fetchJSON(path) {
      try {
        const res = await fetch(`${ADMIN}${path}`);
        if (!res.ok) return null;
        return await res.json();
      } catch { return null; }
    }

    return {
      "command.execute.before": async (input, output) => {
        if (!input?.text?.startsWith("/b00t")) return;
        output.stop = true;
        
        const args = input.text.slice(5).trim();
        const parts = args.split(/\s+/);
        const sub = parts[0] || "help";
        const arg = parts.slice(1).join(" ");
        let msg = "";

        switch (sub) {
          case "skills": {
            const skills = [...state.enabled].sort();
            msg = `🥾 Enabled skills (${skills.length}): ${skills.join(", ")}`;
            break;
          }
          case "enable": {
            if (!arg) { msg = "Usage: /b00t enable <skill>"; break; }
            for (const s of arg.split(/[\s,]+/).filter(Boolean)) state.enabled.add(s);
            msg = `🥾 Enabled: ${arg} (session scope, ${state.enabled.size} active)`;
            break;
          }
          case "disable": {
            if (!arg) { msg = "Usage: /b00t disable <skill>"; break; }
            for (const s of arg.split(/[\s,]+/).filter(Boolean)) state.enabled.delete(s);
            msg = `💤 Disabled: ${arg} (${state.enabled.size} remaining)`;
            break;
          }
          case "status": {
            const health = await fetchJSON("/health");
            const types = await fetchJSON("/types");
            const displays = await fetchJSON("/display");
            const mcpOk = health ? "healthy" : "offline";
            const typeCount = types?.types?.length ?? "?";
            const displayCount = displays?.displays?.length ?? "?";
            msg = [
              `🥾 b00t v0.9.1 | admin: ${mcpOk}`,
              `   ${typeCount} types | ${displayCount} displays | ${state.enabled.size} skills active`,
              `   Branch: task/13-debug-opencode-b00t-shell`,
            ].join("\n");
            break;
          }
          default:
            msg = "/b00t skills | enable <name> | disable <name> | status";
        }
        
        output.addSystemMessage(msg);
      },

      // Inject enabled skills into system prompt so the LLM knows what's available
      "experimental.chat.system.transform": (input, output) => {
        if (!input?.sessionID) return;
        const skills = [...state.enabled].sort().join(", ");
        output.systemPrompt = (output.systemPrompt || "") + 
          `\n\n[b00t] Active skills: ${skills}. Use /b00t enable|disable to toggle.`;
      },
    };
  }
};
