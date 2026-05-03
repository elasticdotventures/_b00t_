import { Env, McpTool, ToolExecutionResult } from '../types';

/**
 * b00t tool implementations for Cloudflare Workers
 */

/**
 * Get available b00t tools for a user
 */
export async function getB00tTools(username: string): Promise<McpTool[]> {
  return [
    {
      name: 'b00t_whoami',
      description: 'Get information about the current user and environment',
      inputSchema: {
        type: 'object',
        properties: {},
        required: [],
      },
    },
    {
      name: 'b00t_learn',
      description: 'Learn about a specific topic or technology',
      inputSchema: {
        type: 'object',
        properties: {
          topic: {
            type: 'string',
            description: 'Topic to learn about (e.g., "rust", "docker", "git")',
          },
        },
        required: ['topic'],
      },
    },
    {
      name: 'b00t_status',
      description: 'Get status of tools and environment',
      inputSchema: {
        type: 'object',
        properties: {
          filter: {
            type: 'string',
            description: 'Filter by subsystem (optional)',
          },
        },
        required: [],
      },
    },
    {
      name: 'github_user_info',
      description: 'Get GitHub user information',
      inputSchema: {
        type: 'object',
        properties: {
          username: {
            type: 'string',
            description: 'GitHub username (defaults to authenticated user)',
          },
        },
        required: [],
      },
    },
    {
      name: 'github_repositories',
      description: 'List GitHub repositories for a user',
      inputSchema: {
        type: 'object',
        properties: {
          username: {
            type: 'string', 
            description: 'GitHub username (defaults to authenticated user)',
          },
          type: {
            type: 'string',
            description: 'Repository type: all, owner, public, private, member',
            enum: ['all', 'owner', 'public', 'private', 'member'],
          },
          sort: {
            type: 'string',
            description: 'Sort by: created, updated, pushed, full_name',
            enum: ['created', 'updated', 'pushed', 'full_name'],
          },
          limit: {
            type: 'number',
            description: 'Maximum number of repositories to return (default: 30)',
            minimum: 1,
            maximum: 100,
          },
        },
        required: [],
      },
    },
  ];
}

/**
 * Execute a b00t tool
 */
export async function executeB00tTool(
  toolName: string,
  args: Record<string, unknown>,
  username: string,
  env: Env
): Promise<ToolExecutionResult> {
  try {
    switch (toolName) {
      case 'b00t_whoami':
        return await executeb00tWhoami(username, env);
      
      case 'b00t_learn':
        return await executeb00tLearn(args.topic as string, username);
      
      case 'b00t_status':
        return await executeb00tStatus(args.filter as string, username);
      
      case 'github_user_info':
        return await executeGitHubUserInfo(args.username as string || username, env);
      
      case 'github_repositories':
        return await executeGitHubRepositories(
          args.username as string || username,
          args.type as string || 'owner',
          args.sort as string || 'updated',
          args.limit as number || 30,
          env
        );
      
      default:
        return {
          success: false,
          error: `Unknown tool: ${toolName}`,
        };
    }
  } catch (error) {
    console.error(`Tool execution error for ${toolName}:`, error);
    return {
      success: false,
      error: `Tool execution failed: ${String(error)}`,
    };
  }
}

/**
 * Execute b00t whoami command
 */
async function executeb00tWhoami(username: string, env: Env): Promise<ToolExecutionResult> {
  const info = {
    username,
    instance: `${username}.b00t.promptexecution.com`,
    environment: env.ENVIRONMENT,
    platform: 'Cloudflare Workers',
    timestamp: new Date().toISOString(),
    version: '1.0.0',
    deployment: 'edge',
    region: 'global',
  };

  const output = `
🥾 b00t-mcp Cloudflare Instance
═══════════════════════════════

👤 User: ${info.username}
🌐 Instance: ${info.instance}
🏷️  Environment: ${info.environment}
⚡ Platform: ${info.platform}
📦 Version: ${info.version}
🌍 Deployment: ${info.deployment}
📍 Region: ${info.region}
🕐 Timestamp: ${info.timestamp}

✅ Authentication: GitHub OAuth
🔐 Isolation: Durable Object per user
📡 Protocol: MCP (Model Context Protocol)
  `.trim();

  return {
    success: true,
    output,
  };
}

/**
 * Execute b00t learn command
 */
async function executeb00tLearn(topic: string, username: string): Promise<ToolExecutionResult> {
  if (!topic) {
    return {
      success: false,
      error: 'Topic parameter is required',
    };
  }

  // Simulate learning resource retrieval
  const learningResources = getb00tLearningResources(topic.toLowerCase());
  
  if (!learningResources) {
    return {
      success: true,
      output: `
🎓 Learning: ${topic}

No specific learning resources found for "${topic}".

🔍 Try these popular topics:
• rust - Rust programming language
• docker - Container platform
• git - Version control
• kubernetes - Container orchestration
• typescript - TypeScript language
• cloudflare - Cloudflare platform

💡 Tip: Use more specific terms or check available topics with 'b00t status'
      `.trim(),
    };
  }

  const output = `
🎓 Learning: ${topic}
═══════════════

📖 ${learningResources.description}

🔗 Resources:
${learningResources.resources.map(r => `• ${r.title}: ${r.url}`).join('\n')}

💡 Quick Start:
${learningResources.quickStart.map(step => `${step.step}. ${step.description}`).join('\n')}

📚 Next Steps:
${learningResources.nextSteps.map(step => `• ${step}`).join('\n')}
  `.trim();

  return {
    success: true,
    output,
  };
}

/**
 * Execute b00t status command
 */
async function executeb00tStatus(filter: string, username: string): Promise<ToolExecutionResult> {
  const status = {
    tools: 'Available',
    auth: 'GitHub OAuth',
    storage: 'R2 + Durable Objects',
    network: 'Cloudflare Edge',
    instance_health: 'Healthy',
    last_activity: new Date().toISOString(),
  };

  let output = `
🥾 b00t-mcp Status
═════════════════

🛠️  Tools: ${status.tools}
🔐 Auth: ${status.auth}
💾 Storage: ${status.storage}
🌐 Network: ${status.network}
❤️  Health: ${status.instance_health}
🕐 Last Activity: ${status.last_activity}
  `.trim();

  if (filter) {
    output += `\n\n🔍 Filter: ${filter}`;
    // Add filter-specific information
    switch (filter.toLowerCase()) {
      case 'auth':
        output += '\n• GitHub OAuth enabled\n• Session-based authentication\n• JWT access tokens';
        break;
      case 'tools':
        output += '\n• b00t_whoami, b00t_learn, b00t_status\n• GitHub integration tools\n• MCP protocol support';
        break;
      case 'network':
        output += '\n• Deployed on Cloudflare Workers\n• Global edge distribution\n• HTTPS/TLS 1.3';
        break;
    }
  }

  return {
    success: true,
    output,
  };
}

/**
 * Execute GitHub user info
 */
async function executeGitHubUserInfo(username: string, env: Env): Promise<ToolExecutionResult> {
  try {
    const response = await fetch(`https://api.github.com/users/${username}`, {
      headers: {
        'User-Agent': 'b00t-mcp-cloudflare/1.0',
        'Accept': 'application/vnd.github.v3+json',
      },
    });

    if (!response.ok) {
      return {
        success: false,
        error: `GitHub API error: ${response.status} ${response.statusText}`,
      };
    }

    const user = await response.json();
    
    const output = `
👤 GitHub User: ${user.login}
═══════════════════════

📛 Name: ${user.name || 'Not provided'}
🏢 Company: ${user.company || 'Not provided'}
📍 Location: ${user.location || 'Not provided'}
📧 Email: ${user.email || 'Not public'}
🌐 Website: ${user.blog || 'None'}
📝 Bio: ${user.bio || 'No bio provided'}

📊 Stats:
• Public Repos: ${user.public_repos}
• Followers: ${user.followers}
• Following: ${user.following}
• Created: ${new Date(user.created_at).toLocaleDateString()}

🔗 Profile: ${user.html_url}
🖼️  Avatar: ${user.avatar_url}
    `.trim();

    return {
      success: true,
      output,
    };
  } catch (error) {
    return {
      success: false,
      error: `Failed to fetch GitHub user info: ${String(error)}`,
    };
  }
}

/**
 * Execute GitHub repositories listing
 */
async function executeGitHubRepositories(
  username: string,
  type: string,
  sort: string,
  limit: number,
  env: Env
): Promise<ToolExecutionResult> {
  try {
    const params = new URLSearchParams({
      type,
      sort,
      per_page: Math.min(limit, 100).toString(),
    });

    const response = await fetch(`https://api.github.com/users/${username}/repos?${params}`, {
      headers: {
        'User-Agent': 'b00t-mcp-cloudflare/1.0',
        'Accept': 'application/vnd.github.v3+json',
      },
    });

    if (!response.ok) {
      return {
        success: false,
        error: `GitHub API error: ${response.status} ${response.statusText}`,
      };
    }

    const repos = await response.json();
    
    if (repos.length === 0) {
      return {
        success: true,
        output: `No repositories found for user "${username}" with type "${type}".`,
      };
    }

    const output = `
📚 ${username}'s Repositories (${type}, sorted by ${sort})
═══════════════════════════════════════════════════

${repos.slice(0, limit).map((repo: any, index: number) => `
${index + 1}. 📦 ${repo.name}${repo.private ? ' 🔒' : ''}
   ${repo.description || 'No description'}
   ⭐ ${repo.stargazers_count} • 🍴 ${repo.forks_count} • ${repo.language || 'Unknown'}
   Updated: ${new Date(repo.updated_at).toLocaleDateString()}
   🔗 ${repo.html_url}
`).join('')}

Showing ${Math.min(limit, repos.length)} of ${repos.length} repositories
    `.trim();

    return {
      success: true,
      output,
    };
  } catch (error) {
    return {
      success: false,
      error: `Failed to fetch GitHub repositories: ${String(error)}`,
    };
  }
}

/**
 * Get learning resources for a topic
 */
function getb00tLearningResources(topic: string) {
  const resources: Record<string, any> = {
    rust: {
      description: 'Rust is a systems programming language focused on safety, speed, and concurrency.',
      resources: [
        { title: 'The Rust Book', url: 'https://doc.rust-lang.org/book/' },
        { title: 'Rust by Example', url: 'https://doc.rust-lang.org/rust-by-example/' },
        { title: 'Rustlings', url: 'https://github.com/rust-lang/rustlings' },
      ],
      quickStart: [
        { step: 1, description: 'Install Rust via rustup: curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh' },
        { step: 2, description: 'Create new project: cargo new hello_world' },
        { step: 3, description: 'Build and run: cargo run' },
      ],
      nextSteps: [
        'Read "The Rust Book" chapters 1-4',
        'Complete Rustlings exercises',
        'Build a CLI tool with clap',
        'Learn about ownership and borrowing',
      ],
    },
    
    docker: {
      description: 'Docker is a platform for developing, shipping, and running applications in containers.',
      resources: [
        { title: 'Docker Official Docs', url: 'https://docs.docker.com/' },
        { title: 'Docker Hub', url: 'https://hub.docker.com/' },
        { title: 'Play with Docker', url: 'https://labs.play-with-docker.com/' },
      ],
      quickStart: [
        { step: 1, description: 'Install Docker Desktop or Docker Engine' },
        { step: 2, description: 'Run hello world: docker run hello-world' },
        { step: 3, description: 'Create Dockerfile and build image' },
      ],
      nextSteps: [
        'Learn Dockerfile best practices',
        'Use docker-compose for multi-container apps',
        'Understand Docker networking',
        'Explore container orchestration with Kubernetes',
      ],
    },

    git: {
      description: 'Git is a distributed version control system for tracking changes in source code.',
      resources: [
        { title: 'Pro Git Book', url: 'https://git-scm.com/book' },
        { title: 'Git Tutorial', url: 'https://git-scm.com/docs/gittutorial' },
        { title: 'GitHub Git Handbook', url: 'https://guides.github.com/introduction/git-handbook/' },
      ],
      quickStart: [
        { step: 1, description: 'Configure: git config --global user.name "Your Name"' },
        { step: 2, description: 'Initialize repo: git init' },
        { step: 3, description: 'Make first commit: git add . && git commit -m "Initial commit"' },
      ],
      nextSteps: [
        'Learn branching and merging',
        'Understand Git workflows (GitFlow, GitHub Flow)',
        'Master rebasing and cherry-picking',
        'Set up Git hooks for automation',
      ],
    },

    cloudflare: {
      description: 'Cloudflare provides web performance and security solutions including Workers, Pages, and R2.',
      resources: [
        { title: 'Cloudflare Docs', url: 'https://developers.cloudflare.com/' },
        { title: 'Workers Examples', url: 'https://developers.cloudflare.com/workers/examples/' },
        { title: 'Wrangler CLI', url: 'https://developers.cloudflare.com/workers/wrangler/' },
      ],
      quickStart: [
        { step: 1, description: 'Install Wrangler: npm install -g wrangler' },
        { step: 2, description: 'Create Worker: wrangler generate my-worker' },
        { step: 3, description: 'Deploy: wrangler deploy' },
      ],
      nextSteps: [
        'Learn Durable Objects for stateful apps',
        'Use R2 for object storage',
        'Implement KV for caching',
        'Set up Pages for static sites',
      ],
    },
  };

  return resources[topic] || null;
}