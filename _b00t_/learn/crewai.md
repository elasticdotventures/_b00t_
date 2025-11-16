# CrewAI: Multi-Agent Collaboration Framework

## Overview

CrewAI is a cutting-edge framework for orchestrating role-playing, autonomous AI agents. By fostering collaborative intelligence, CrewAI empowers agents to work together seamlessly, tackling complex tasks.

**Key Philosophy**: Enable AI agents with specific roles, goals, and tools to collaborate like a well-oiled crew, achieving outcomes that surpass individual capabilities.

## Core Concepts

### 1. Agents

Agents are autonomous units with:
- **Role**: Defines the agent's function (e.g., Researcher, Writer, Analyst)
- **Goal**: What the agent aims to achieve
- **Backstory**: Context that shapes decision-making
- **Tools**: Capabilities the agent can use
- **LLM**: Language model powering the agent

### 2. Tasks

Tasks define work to be performed:
- **Description**: What needs to be done
- **Agent**: Who performs the task
- **Expected Output**: Success criteria
- **Tools**: Task-specific capabilities
- **Dependencies**: Task execution order

### 3. Crew

The crew orchestrates agents and tasks:
- **Process**: Sequential or hierarchical execution
- **Memory**: Shared knowledge across tasks
- **Verbose**: Logging level
- **Max RPM**: Rate limiting

## Quick Start

### Installation

```bash
# Install CrewAI and tools
pip install crewai crewai-tools

# Or with UV (recommended)
uv add crewai crewai-tools
```

### Create New Crew

```bash
# Initialize a new crew project
crewai create crew jollyroger

# Navigate to project
cd jollyroger

# Install dependencies
crewai install

# Run the crew
crewai run
```

### Project Structure

```
jollyroger/
├── src/
│   └── jollyroger/
│       ├── config/
│       │   ├── agents.yaml    # Agent definitions
│       │   └── tasks.yaml     # Task definitions
│       ├── tools/             # Custom tools
│       ├── crew.py            # Crew orchestration
│       └── main.py            # Entry point
├── .env                       # API keys
├── pyproject.toml
└── README.md
```

## Configuration

### agents.yaml

```yaml
researcher:
  role: >
    Senior Research Analyst
  goal: >
    Uncover cutting-edge developments in AI and data science
  backstory: >
    You're a seasoned researcher with a knack for uncovering the latest
    developments in AI and data science. Known for your ability to find
    the most relevant information and present it in a clear manner.
  llm: gpt-4
  tools:
    - SerperDevTool
  verbose: true

writer:
  role: >
    Tech Content Strategist
  goal: >
    Craft compelling content on tech advancements
  backstory: >
    You are a renowned Content Strategist, known for your insightful
    and engaging articles on technology and innovation.
  llm: claude-3-sonnet-20240229
  verbose: true
```

### tasks.yaml

```yaml
research_task:
  description: >
    Conduct a thorough research about {topic}.
    Make sure you find any interesting and relevant information given
    the current year is 2024.
  expected_output: >
    A list with 10 bullet points of the most relevant information about {topic}
  agent: researcher

writing_task:
  description: >
    Review the research and expand each topic into a full section for a blog post.
    Make sure the blog post is detailed and uses proper formatting.
  expected_output: >
    A well written blog post about {topic} in markdown format
  agent: writer
  output_file: report.md
```

### crew.py

```python
from crewai import Agent, Crew, Process, Task
from crewai.project import CrewBase, agent, crew, task

@CrewBase
class JollyrogerCrew():
    """Jollyroger crew"""
    agents_config = 'config/agents.yaml'
    tasks_config = 'config/tasks.yaml'

    @agent
    def researcher(self) -> Agent:
        return Agent(
            config=self.agents_config['researcher'],
            verbose=True
        )

    @agent
    def writer(self) -> Agent:
        return Agent(
            config=self.agents_config['writer'],
            verbose=True
        )

    @task
    def research_task(self) -> Task:
        return Task(
            config=self.tasks_config['research_task'],
        )

    @task
    def writing_task(self) -> Task:
        return Task(
            config=self.tasks_config['writing_task'],
        )

    @crew
    def crew(self) -> Crew:
        return Crew(
            agents=self.agents,
            tasks=self.tasks,
            process=Process.sequential,
            verbose=True,
        )
```

## Advanced Features

### Custom Tools

Create tools in `tools/` directory:

```python
from crewai_tools import BaseTool

class MyCustomTool(BaseTool):
    name: str = "Custom Research Tool"
    description: str = "Searches proprietary database"

    def _run(self, query: str) -> str:
        # Implementation
        return result
```

### Memory Systems

```python
crew = Crew(
    agents=[agent1, agent2],
    tasks=[task1, task2],
    memory=True,  # Enable memory
    embedder={
        "provider": "openai",
        "config": {"model": "text-embedding-3-small"}
    }
)
```

### Hierarchical Process

```python
crew = Crew(
    agents=[manager, researcher, writer],
    tasks=[research, write, review],
    process=Process.hierarchical,
    manager_llm="gpt-4"  # Manager coordinates other agents
)
```

## Integration with b00t Multi-Agent POC

CrewAI concepts align with b00t's multi-agent architecture:

### Mapping Concepts

| CrewAI | b00t POC | Notes |
|--------|----------|-------|
| `Agent` | `Agent` struct | Role, skills, personality |
| `Task` | Message::Vote proposals | Collaborative decisions |
| `Crew` | Crew formation | `/crew form` command |
| `Process.hierarchical` | Captain/Mate roles | Authority delegation |
| `Tools` | Agent skills | Specialist capabilities |

### Integration Example

```rust
// b00t agent with CrewAI backend
pub struct CrewAIAgent {
    agent: b00t_ipc::Agent,
    crewai_config: CrewAIConfig,
}

impl CrewAIAgent {
    pub async fn execute_task(&self, task: &str) -> Result<String> {
        // 1. Create CrewAI task.yaml from b00t task
        // 2. Spawn Python CrewAI process
        // 3. Pipe results back to b00t IPC bus
        // 4. Broadcast completion to crew
        Ok(result)
    }
}
```

## Best Practices

### 1. Agent Design

- **Single Responsibility**: Each agent has one clear purpose
- **Complementary Skills**: Agents should have diverse, non-overlapping capabilities
- **Clear Goals**: Specific, measurable objectives

### 2. Task Orchestration

- **Sequential for Dependencies**: Use when output of one task feeds another
- **Hierarchical for Complexity**: Manager agent coordinates when tasks are interdependent
- **Parallel for Independence**: (Future feature) Run unrelated tasks concurrently

### 3. Prompt Engineering

- **Backstory Matters**: Rich context improves decision-making
- **Expected Output**: Be explicit about format and content
- **Examples**: Provide reference outputs in task descriptions

### 4. Tool Selection

- **Built-in Tools**: SerperDev (search), FileRead, DirectoryRead
- **Custom Tools**: For domain-specific operations
- **Tool Combinations**: Agents can use multiple tools

## Environment Variables

```bash
# .env
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GROQ_API_KEY=gsk_...
SERPER_API_KEY=...  # For web search tool
```

## Comparison with Other Frameworks

| Feature | CrewAI | AutoGen | LangGraph | b00t POC |
|---------|--------|---------|-----------|----------|
| YAML Config | ✅ | ❌ | ❌ | TOML datums |
| Role-based | ✅ | ❌ | ❌ | ✅ |
| Hierarchical | ✅ | ✅ | ✅ | Captain/Mate |
| Memory | ✅ | ✅ | ✅ | Planned |
| Tools | ✅ | ✅ | ✅ | Skills |
| Multi-LLM | ✅ | ✅ | ✅ | ✅ |

## Common Patterns

### Research + Writing Pipeline

```python
# Researcher → Writer → Editor
crew = Crew(
    agents=[researcher, writer, editor],
    tasks=[research, write, edit],
    process=Process.sequential
)
```

### Analysis + Decision Making

```python
# Multiple analysts → Manager makes decision
crew = Crew(
    agents=[analyst1, analyst2, analyst3, manager],
    tasks=[analyze_market, analyze_tech, analyze_risk, decide],
    process=Process.hierarchical
)
```

### Data Processing Pipeline

```python
# Scraper → Cleaner → Analyzer → Reporter
crew = Crew(
    agents=[scraper, cleaner, analyzer, reporter],
    tasks=[scrape, clean, analyze, report],
    process=Process.sequential
)
```

## Related Projects

### TaskGen
Open-sourced LLM agentic framework focused on:
- Task-based execution
- Memory-infused agents
- StrictJSON for structured outputs
- Alternative to AutoGen

**GitHub**: https://github.com/tanchongmin/agentjo

### Beehive AI
Collaborative AI application framework:
- Nested crew structures ("hives")
- CrewAI integration
- Team composition patterns

**GitHub**: https://github.com/BeehiveHQ/beehive-ai

## Learning Resources

### Courses

1. **DeepLearning.AI - Practical Multi-AI Agents**
   - Hands-on with CrewAI
   - Real-world use cases
   - Advanced patterns
   - https://www.deeplearning.ai/short-courses/practical-multi-ai-agents-and-advanced-use-cases-with-crewai/

### Tutorials

1. **Analytics Vidhya - Agentic Workflow with CrewAI and Groq**
   - Fast inference with Groq
   - Cost optimization
   - https://www.analyticsvidhya.com/blog/2024/06/agentic-workflow-with-crewai-and-groq/

2. **Geeky Gadgets - Build AI Workers**
   - Beginner-friendly
   - Practical examples
   - https://www.geeky-gadgets.com/build-ai-workers-using-crewai/

### Videos

1. **CrewAI Overview** - https://www.youtube.com/watch?v=i-txsBoTJtI
2. **Step-by-Step Real Use Case** - https://search.app/Emfs
3. **Human-Friendly Agent Communities** - https://youtube.com/watch?v=F3usuxs2p1Y

## Future Integration with b00t

Potential enhancements to b00t multi-agent POC:

1. **CrewAI Backend**: Python agents using CrewAI, IPC via message bus
2. **Hybrid Crews**: Mix Rust-native and CrewAI-backed agents
3. **YAML → TOML**: Convert CrewAI configs to b00t datums
4. **Task Templates**: Reusable task patterns in justfile recipes
5. **Discord Bridge**: CrewAI agent as Discord proxy (per datum reference)

## Troubleshooting

### Common Issues

**Import errors**: Ensure virtual environment activated
```bash
source .venv/bin/activate
```

**API rate limits**: Use `max_rpm` in crew config
```python
crew = Crew(..., max_rpm=10)
```

**Tool failures**: Check API keys in `.env`

**Memory issues**: Disable if not needed
```python
crew = Crew(..., memory=False)
```

## Community & Support

- **Discord**: https://discord.com/invite/X4JWnZnxPb
- **GitHub Issues**: https://github.com/joaomdmoura/crewai/issues
- **Documentation**: https://docs.crewai.com
- **Chat Support**: https://chatg.pt/DWjSBZn

---

**Version**: Compatible with CrewAI 0.x+
**Python**: 3.10 - 3.12
**License**: MIT

Yei can now leverage CrewAI's mature multi-agent orchestration alongside b00t's Rust-native IPC! 🍰
