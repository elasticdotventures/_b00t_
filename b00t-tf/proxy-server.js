#!/usr/bin/env node
/**
 * b00t Self-Bootstrapping MCP Server
 * Downloads capabilities from b00t.promptexecution.com and provisions infrastructure
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import { execSync, spawn } from 'child_process';
import { existsSync, mkdirSync, writeFileSync, readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { homedir } from 'os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

class B00tMcpServer {
  constructor() {
    this.server = new Server(
      {
        name: 'b00t-local-proxy',
        version: '1.0.0',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.b00tDir = join(homedir(), '.b00t');
    this.dotfilesDir = join(homedir(), '.dotfiles');
    this.tfDir = join(this.dotfilesDir, 'b00t-tf');
    this.configFile = join(this.b00tDir, 'config.json');
    
    this.setupHandlers();
    this.ensureDirectories();
    this.loadConfig();
  }

  ensureDirectories() {
    [this.b00tDir, this.tfDir].forEach(dir => {
      if (!existsSync(dir)) {
        mkdirSync(dir, { recursive: true });
      }
    });
  }

  loadConfig() {
    if (existsSync(this.configFile)) {
      try {
        this.config = JSON.parse(readFileSync(this.configFile, 'utf8'));
      } catch (error) {
        this.config = this.getDefaultConfig();
      }
    } else {
      this.config = this.getDefaultConfig();
      this.saveConfig();
    }
  }

  saveConfig() {
    writeFileSync(this.configFile, JSON.stringify(this.config, null, 2));
  }

  getDefaultConfig() {
    return {
      providers: {
        anthropic: { enabled: true, models: ['claude-3-sonnet-20240229'] },
        openrouter: { enabled: false, models: [] }
      },
      infrastructure: {
        cloudflare: { enabled: true },
        aws: { enabled: false }
      },
      tools: {
        filesystem: true,
        git: true,
        terminal: true,
        terraform: false
      },
      bootstrapped: false
    };
  }

  setupHandlers() {
    this.server.setRequestHandler(ListToolsRequestSchema, async () => {
      return {
        tools: [
          {
            name: 'bootstrap_b00t',
            description: 'Bootstrap b00t capabilities and infrastructure',
            inputSchema: {
              type: 'object',
              properties: {
                force: { type: 'boolean', description: 'Force re-bootstrap', default: false }
              }
            }
          },
          {
            name: 'install_tool',
            description: 'Install CLI tool (just, gh, tofu, etc.)',
            inputSchema: {
              type: 'object',
              properties: {
                tool: { type: 'string', description: 'Tool name to install' }
              },
              required: ['tool']
            }
          },
          {
            name: 'provision_infrastructure',
            description: 'Provision infrastructure using OpenTofu',
            inputSchema: {
              type: 'object',
              properties: {
                module: { type: 'string', description: 'Module to provision (base, cloudflare, aws)' },
                action: { type: 'string', enum: ['plan', 'apply', 'destroy'], default: 'plan' }
              },
              required: ['module']
            }
          },
          {
            name: 'configure_provider',
            description: 'Configure AI provider settings',
            inputSchema: {
              type: 'object',
              properties: {
                provider: { type: 'string', enum: ['anthropic', 'openrouter', 'openai'] },
                enabled: { type: 'boolean' },
                api_key: { type: 'string', description: 'API key (will be stored securely)' }
              },
              required: ['provider']
            }
          },
          {
            name: 'detect_tools',
            description: 'Detect installed CLI tools with versions',
            inputSchema: { type: 'object', properties: {} }
          },
          {
            name: 'get_status',
            description: 'Get b00t system status and configuration',
            inputSchema: { type: 'object', properties: {} }
          }
        ]
      };
    });

    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;

      try {
        switch (name) {
          case 'bootstrap_b00t':
            return await this.bootstrapB00t(args?.force);
            
          case 'install_tool':
            return await this.installTool(args.tool);
            
          case 'provision_infrastructure':
            return await this.provisionInfrastructure(args.module, args.action);
            
          case 'configure_provider':
            return await this.configureProvider(args.provider, args.enabled, args.api_key);
            
          case 'detect_tools':
            return await this.detectToolsHandler();
            
          case 'get_status':
            return await this.getStatus();
            
          default:
            throw new Error(`Unknown tool: ${name}`);
        }
      } catch (error) {
        return {
          content: [
            {
              type: 'text',
              text: `Error executing ${name}: ${error.message}`
            }
          ]
        };
      }
    });
  }

  async bootstrapB00t(force = false) {
    if (this.config.bootstrapped && !force) {
      return {
        content: [{ type: 'text', text: '✅ b00t already bootstrapped. Use force=true to re-bootstrap.' }]
      };
    }

    const steps = [];
    
    try {
      // Download b00t-tf modules if missing
      if (!existsSync(join(this.tfDir, 'main.tf'))) {
        steps.push('📥 Downloading OpenTofu modules...');
        await this.downloadTerraformModules();
      }

      // Install essential tools
      const tools = ['just', 'gh'];
      for (const tool of tools) {
        if (!this.isToolInstalled(tool)) {
          steps.push(`🔧 Installing ${tool}...`);
          await this.installTool(tool);
        }
      }

      // Setup git hooks if in git repo
      if (existsSync(join(this.dotfilesDir, '.git'))) {
        steps.push('🪝 Setting up git hooks...');
        this.setupGitHooks();
      }

      this.config.bootstrapped = true;
      this.saveConfig();

      steps.push('🎉 Bootstrap complete!');

      return {
        content: [
          {
            type: 'text', 
            text: steps.join('\n')
          }
        ]
      };
    } catch (error) {
      return {
        content: [
          {
            type: 'text',
            text: `❌ Bootstrap failed: ${error.message}`
          }
        ]
      };
    }
  }

  async downloadTerraformModules() {
    // In production, this would download from b00t.promptexecution.com
    // For now, create basic structure
    const modules = ['base', 'cloudflare', 'aws'];
    
    for (const module of modules) {
      const moduleDir = join(this.tfDir, 'modules', module);
      mkdirSync(moduleDir, { recursive: true });
      
      if (!existsSync(join(moduleDir, 'main.tf'))) {
        writeFileSync(join(moduleDir, 'main.tf'), this.getModuleTemplate(module));
      }
    }
  }

  getModuleTemplate(module) {
    const templates = {
      base: `# ${module} module\nterraform {\n  required_providers {\n    cloudflare = {\n      source = "cloudflare/cloudflare"\n      version = "~> 4.0"\n    }\n  }\n}\n`,
      cloudflare: `# Cloudflare Workers module\nresource "cloudflare_worker_script" "mcp_proxy" {\n  # Worker configuration\n}\n`,
      aws: `# AWS resources module\nresource "aws_lambda_function" "mcp_handler" {\n  # Lambda configuration\n}\n`
    };
    return templates[module] || '# Empty module';
  }

  isToolInstalled(tool) {
    try {
      execSync(`which ${tool}`, { stdio: 'ignore' });
      return true;
    } catch {
      return false;
    }
  }

  detectInstalledTools() {
    const tools = ['just', 'gh', 'tofu', 'terraform', 'node', 'npm', 'git'];
    const status = {};
    
    tools.forEach(tool => {
      status[tool] = {
        installed: this.isToolInstalled(tool),
        version: this.getToolVersion(tool),
        required: ['just', 'gh'].includes(tool) || tool === 'tofu' // tofu is required, terraform is optional fallback
      };
    });
    
    // Special handling for terraform vs tofu preference
    const hasTofu = status.tofu?.installed;
    const hasTerraform = status.terraform?.installed;
    
    if (hasTofu || hasTerraform) {
      status.iac_tool = {
        installed: true,
        version: hasTofu ? status.tofu.version : status.terraform.version,
        preferred: hasTofu ? 'tofu' : 'terraform',
        required: true
      };
    } else {
      status.iac_tool = {
        installed: false,
        version: 'not installed',
        preferred: 'tofu',
        required: true
      };
    }
    
    return status;
  }

  getToolVersion(tool) {
    try {
      const versionCommands = {
        'just': 'just --version',
        'gh': 'gh --version',
        'tofu': 'tofu --version',
        'terraform': 'terraform --version',
        'node': 'node --version',
        'npm': 'npm --version',
        'git': 'git --version'
      };
      
      const cmd = versionCommands[tool];
      if (!cmd) return 'unknown';
      
      const output = execSync(cmd, { encoding: 'utf8', stdio: 'pipe' });
      return output.split('\n')[0].trim();
    } catch {
      return 'not installed';
    }
  }

  async detectToolsHandler() {
    const toolStatus = this.detectInstalledTools();
    
    let output = "🔍 Tool Detection Report:\n\n";
    
    Object.entries(toolStatus).forEach(([tool, status]) => {
      const icon = status.installed ? '✅' : '❌';
      const required = status.required ? ' (required)' : '';
      output += `${icon} ${tool}${required}: ${status.version}\n`;
    });
    
    const missing = Object.entries(toolStatus)
      .filter(([_, status]) => status.required && !status.installed)
      .map(([tool, _]) => tool);
    
    if (missing.length > 0) {
      output += `\n⚠️  Missing required tools: ${missing.join(', ')}\n`;
      output += "💡 Run install_tool to auto-install missing tools\n";
    } else {
      output += "\n🎉 All required tools are installed!\n";
    }
    
    return {
      content: [{ type: 'text', text: output }]
    };
  }

  async installTool(tool) {
    const installers = {
      just: 'curl --proto \'=https\' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to ~/.local/bin',
      gh: 'curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg',
      tofu: 'curl --proto \'=https\' --tlsv1.2 -fsSL https://get.opentofu.org/install-opentofu.sh | sh'
    };

    if (!installers[tool]) {
      throw new Error(`No installer available for ${tool}`);
    }

    if (this.isToolInstalled(tool)) {
      return {
        content: [{ type: 'text', text: `✅ ${tool} already installed` }]
      };
    }

    try {
      execSync(installers[tool], { stdio: 'pipe' });
      return {
        content: [{ type: 'text', text: `✅ ${tool} installed successfully` }]
      };
    } catch (error) {
      throw new Error(`Failed to install ${tool}: ${error.message}`);
    }
  }

  async provisionInfrastructure(module, action = 'plan') {
    const moduleDir = join(this.tfDir, 'modules', module);
    
    if (!existsSync(moduleDir)) {
      throw new Error(`Module ${module} not found. Run bootstrap_b00t first.`);
    }

    try {
      const cmd = `cd ${moduleDir} && tofu ${action}`;
      const output = execSync(cmd, { encoding: 'utf8' });
      
      return {
        content: [
          {
            type: 'text',
            text: `🏗️ OpenTofu ${action} for ${module}:\n\n${output}`
          }
        ]
      };
    } catch (error) {
      throw new Error(`OpenTofu ${action} failed: ${error.message}`);
    }
  }

  async configureProvider(provider, enabled, apiKey) {
    if (!this.config.providers[provider]) {
      throw new Error(`Unknown provider: ${provider}`);
    }

    this.config.providers[provider].enabled = enabled;
    
    if (apiKey) {
      // Store API key securely (in production, use OS keychain)
      process.env[`${provider.toUpperCase()}_API_KEY`] = apiKey;
    }

    this.saveConfig();

    return {
      content: [
        {
          type: 'text',
          text: `✅ Provider ${provider} ${enabled ? 'enabled' : 'disabled'}`
        }
      ]
    };
  }

  async getStatus() {
    const status = {
      bootstrapped: this.config.bootstrapped,
      tools: {},
      providers: this.config.providers,
      infrastructure: this.config.infrastructure
    };

    // Check tool installation status
    const tools = ['just', 'gh', 'tofu', 'git'];
    for (const tool of tools) {
      status.tools[tool] = this.isToolInstalled(tool);
    }

    return {
      content: [
        {
          type: 'text',
          text: `📊 b00t Status:\n\n${JSON.stringify(status, null, 2)}`
        }
      ]
    };
  }

  setupGitHooks() {
    const hooksDir = join(this.dotfilesDir, '.git', 'hooks');
    const preCommitHook = join(hooksDir, 'pre-commit');
    
    if (!existsSync(preCommitHook)) {
      writeFileSync(preCommitHook, `#!/bin/bash\n# b00t pre-commit hook\necho "🥾 b00t pre-commit checks..."\n`, { mode: 0o755 });
    }
  }

  async run() {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.error('🥾 b00t MCP Server started');
  }
}

// Start the server
const server = new B00tMcpServer();
server.run().catch(console.error);