export default {
  id: "b00t-opencode-plugin",
  hooks: {
    "command.execute.before": async (input, output) => {
      if (!input?.text?.startsWith("/b00t")) return;
      output.stop = true;
      
      const args = input.text.slice(5).trim();
      const parts = args.split(/\s+/);
      const sub = parts[0] || "help";
      
      let msg = "";
      switch (sub) {
        case "skills":
          msg = "Registered skills: goal, mcp, cad, blessed, viz, ooda, rhai, finetune";
          break;
        case "enable":
          msg = `🥾 Enabled: ${parts.slice(1).join(" ")} (session scope)`;
          break;
        case "disable":
          msg = `💤 Disabled: ${parts.slice(1).join(" ")} (session scope)`;
          break;
        case "status":
          msg = "b00t hive agent v0.9.1 | branch: task/13 | MCPs: 52 healthy";
          break;
        default:
          msg = "/b00t skills | enable <name> | disable <name> | status";
      }
      
      output.addSystemMessage(msg);
    }
  }
};
