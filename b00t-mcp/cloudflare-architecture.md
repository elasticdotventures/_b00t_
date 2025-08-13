# 🌤️ Cloudflare Deployment Architecture for b00t-mcp

## Overview
Deploy b00t-mcp servers with GitHub username-based subdomains on Cloudflare infrastructure.

**Domain Pattern**: `{github-username}.b00t.promptexecution.com`

## Architecture Options

### Option 1: Cloudflare Workers + Durable Objects (Recommended)
```
Internet → Cloudflare Edge
         → Worker (Router)
         → Durable Object (Per-user b00t-mcp instance)
         → R2 Storage (Session/Config data)
```

**Pros:**
- ✅ True multi-tenancy with isolated user instances
- ✅ Auto-scaling and edge distribution
- ✅ Persistent state via Durable Objects
- ✅ Cost-effective for sporadic usage

**Cons:**
- 🔄 Need to port Rust to JS/WASM
- 🔄 Axum → Hono/Itty Router migration

### Option 2: Cloudflare Workers + Remote Rust Servers
```
Internet → Cloudflare Edge
         → Worker (Proxy/Router)
         → Container Registry (Per-user containers)
         → External hosting (Railway/Fly.io)
```

**Pros:**
- ✅ Keep existing Rust codebase
- ✅ Easy GitHub OAuth integration
- ✅ Full Axum compatibility

**Cons:**
- 💰 Higher cost (always-on containers)
- 🔧 Container orchestration complexity

### Option 3: Cloudflare Workers + Wasm (Hybrid)
```
Internet → Cloudflare Edge
         → Worker (Host)
         → WASM Module (Rust b00t-mcp core)
         → R2/KV Storage
```

**Pros:**
- ✅ Reuse Rust business logic
- ✅ Cloudflare-native scaling
- ✅ Edge performance

**Cons:**
- 🔄 Axum → CF Workers HTTP integration
- 🔧 WASM compilation complexity

## Recommended Implementation: Option 1 (Pure Workers)

### Phase 1: Core Worker Setup

#### 1.1 Domain & DNS Configuration
```bash
# Cloudflare DNS Records
*.b00t.promptexecution.com → Worker Route
```

#### 1.2 Worker Architecture
```typescript
// Main worker: routes requests to user instances
export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const subdomain = url.hostname.split('.')[0];
    
    // Route to user-specific Durable Object
    const userInstance = env.B00T_MCP.get(
      env.B00T_MCP.idFromName(subdomain)
    );
    
    return userInstance.fetch(request);
  }
};

// Durable Object: per-user b00t-mcp instance
export class B00tMcpInstance {
  async fetch(request: Request): Promise<Response> {
    // Implement b00t-mcp MCP server logic
    // Handle OAuth, tool execution, session management
  }
}
```

### Phase 2: Core Features Port

#### 2.1 OAuth Integration
- GitHub OAuth flow in Worker
- JWT token generation/validation
- User session management in Durable Object state

#### 2.2 MCP Protocol Implementation
- JSON-RPC 2.0 handling
- Tool discovery and execution
- Streaming responses via Server-Sent Events

#### 2.3 b00t-cli Integration
- Core tool implementations in TypeScript
- GitHub API integration
- Safe command execution

### Phase 3: User Management

#### 3.1 Automatic Provisioning
```typescript
// When user first accesses their subdomain
if (!await userExists(githubUsername)) {
  await provisionUserInstance(githubUsername);
  await forkUserRepository(githubUsername);
  await setupUserConfig(githubUsername);
}
```

#### 3.2 Configuration Storage
- User configs in R2 (b00t-mcp-acl.toml)
- Session data in Durable Object storage
- User-specific tool permissions

## Implementation Plan

### Step 1: Basic Worker Setup
1. Create Cloudflare Worker project
2. Configure wildcard domain routing
3. Implement subdomain extraction
4. Basic health check endpoints

### Step 2: MVP MCP Server
1. Port OAuth minimal to TypeScript
2. Basic JSON-RPC handling
3. Simple tool implementations (whoami, learn)
4. GitHub OAuth integration

### Step 3: Durable Objects Integration
1. Per-user instance creation
2. State persistence
3. Session management
4. Tool execution isolation

### Step 4: Production Features
1. Rate limiting per user
2. Usage analytics
3. Error handling and logging
4. Monitoring and alerts

## Cost Analysis (Cloudflare Workers)

### Pricing Tiers
- **Free**: 100K requests/day, 10ms CPU time
- **Paid ($5/month)**: 10M requests/month, 50ms CPU time
- **Durable Objects**: $0.15/million requests + storage

### Expected Usage
- Active users: ~50-100
- Requests per user/day: ~100-500
- Total monthly requests: ~150K-1.5M

**Estimated Cost**: $5-15/month total

## Security Considerations

### Authentication
- GitHub OAuth for user identity
- JWT tokens with short expiration
- Subdomain isolation prevents cross-user access

### Execution Safety
- Sandboxed tool execution
- ACL enforcement per user
- Rate limiting and quotas

### Data Privacy
- User data isolated in Durable Objects
- No cross-tenant data leakage
- GDPR compliance via data deletion APIs

## Next Steps

1. Set up Cloudflare Workers development environment
2. Create basic router with subdomain handling
3. Implement GitHub OAuth flow in Worker context
4. Port minimal MCP server functionality
5. Test with elasticdotventures.b00t.promptexecution.com

## Files to Create
- `cloudflare/worker.ts` - Main router worker
- `cloudflare/durable-objects/b00t-mcp.ts` - Per-user instance
- `cloudflare/lib/oauth.ts` - GitHub OAuth handling
- `cloudflare/lib/mcp.ts` - MCP protocol implementation
- `cloudflare/lib/tools.ts` - b00t tool implementations
- `cloudflare/wrangler.toml` - Deployment configuration