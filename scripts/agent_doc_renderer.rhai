// Agent doc renderer – produces deterministic, tool-focused guidance per agent

const TEMPLATE = r###"---
name: ${name}
description: ${description}
skills: [${skills}]
datums: [${datums}]
---

# ${title}

## Specialization & Role

This agent specializes in **${specialization}**, combining its skills with the following datums:

- Skills: ${skills}
- Datums: ${datums}
- Activation command: `${activation}`

## Philosophy

${philosophy}

## Tools & Concrete Commands

${tool_list}

## Workflow Steps

${workflow_steps}

## Learning Topics to Prime

${learn_topics}

## Authorization & Environment

1. Every instantiation must be executed via `b00t agent run ${name}` per IETF 2119.
2. The framework will prime the context, environment, and datums under `.b00t/` before running.
3. Always gather project context with `b00t flashback --context` before executing commands above.

${optional_notes}
"###;

let agent_specs = #{ 
    "cli-master": #{
        description: "Ship CLI development analyses that combine agent-aware commands with human readable documentation.",
        skills: ["CLI architecture", "human+AI collaboration", "automation orchestration"],
        datums: ["b00t-cli", "ansible.cli", "learn"],
        learn_topics: ["cli design", "automation workflows"],
        tool_examples: [
            "b00t cli up --yes  # ensure CLI datums are current",
            "b00t cli check <tool>  # verify version for each tool",
            "b00t learn cli design  # capture design lessons",
            "b00t learn automation  # discover automation patterns"
        ],
        workflow_steps: [
            "1. Run `b00t flashback --context` to capture the code snapshot and pass that context bundle to downstream steps.",
            "2. Execute `b00t cli up --yes` to align CLI tool versions with their configured datums before crafting recommendations.",
            "3. Inspect the outcomes of `b00t cli check <tool>` for each relevant CLI and propagate status into the agent response.",
            "4. Pull actionable prose from `b00t learn cli design` and `b00t learn automation` to explain required commands.",
            "5. Finalize the CLI blueprint, verify it against the user's requirements, and confirm via `b00t cli check` on any new scripts."
        ],
        title: "CLI Master Agent",
        specialization: "Designing CLI stacks that serve both AI agents and humans",
        philosophy: "Lead with CLI design that is fully reproducible by b00t-managed tooling; humans read the results, agents run them.",
        activation: "b00t agent run cli-master --context /tmp/context.bundle",
        optional_notes: "## Tooling Note\nFavor datums maintained in .b00t/ over ad-hoc external installs."
    }
};

for spec_name in agent_specs.keys() {
    let spec = agent_specs[spec_name];
    let title = spec.title;
    let skills = spec.skills.join(", ");
    let datums = spec.datums.iter().map(|d| format!("`{}`", d)).join(", ");
    let tool_list = spec.tool_examples.iter().map(|cmd| format!("- `{}`", cmd)).join("\n");
    let workflow_steps = spec.workflow_steps.join("\n\n");
    let learn_topics = spec.learn_topics.iter().map(|topic| format!("- `b00t learn {}`", topic)).join("\n");

    let content = TEMPLATE
        .replace("${name}", spec_name)
        .replace("${description}", spec.description)
        .replace("${skills}", skills)
        .replace("${datums}", datums)
        .replace("${title}", title)
        .replace("${specialization}", spec.specialization)
        .replace("${philosophy}", spec.philosophy)
        .replace("${activation}", spec.activation)
        .replace("${tool_list}", tool_list)
        .replace("${workflow_steps}", workflow_steps)
        .replace("${learn_topics}", learn_topics)
        .replace("${optional_notes}", spec.optional_notes);

    let path = format!(".claude/agents/{}.md", spec_name);
    let dir_path = path.split("/");
    let dir_parts = dir_path[..dir_path.len() - 1];
    create_dir(dir_parts.join("/"));
    write_file(&path, &content);
}
