import { Env } from './types';
import { handleOAuth } from './lib/oauth';
import { extractSubdomain, generateErrorResponse } from './lib/utils';

/**
 * Main Cloudflare Worker
 * Routes requests to user-specific Durable Object instances
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    try {
      const url = new URL(request.url);
      const subdomain = extractSubdomain(url.hostname);
      
      // Handle root domain requests
      if (!subdomain || subdomain === 'www') {
        return handleRootDomain(request);
      }
      
      // Handle OAuth callbacks and auth endpoints
      if (url.pathname.startsWith('/auth/')) {
        return handleOAuth(request, env, subdomain);
      }
      
      // Route to user-specific Durable Object instance
      const userInstanceId = env.B00T_MCP_INSTANCE.idFromName(subdomain);
      const userInstance = env.B00T_MCP_INSTANCE.get(userInstanceId);
      
      // Forward request to user's b00t-mcp instance
      return await userInstance.fetch(request);
      
    } catch (error) {
      console.error('Worker error:', error);
      return generateErrorResponse(
        500, 
        'Internal server error',
        { error: String(error) }
      );
    }
  },
};

/**
 * Handle root domain requests (landing page, docs, etc.)
 */
async function handleRootDomain(request: Request): Promise<Response> {
  const url = new URL(request.url);
  
  if (url.pathname === '/' || url.pathname === '') {
    return new Response(getWelcomePage(), {
      headers: { 'Content-Type': 'text/html' },
    });
  }
  
  if (url.pathname === '/health') {
    return Response.json({ 
      status: 'healthy',
      timestamp: new Date().toISOString(),
      service: 'b00t-mcp-cloudflare'
    });
  }
  
  return new Response('Not Found', { status: 404 });
}

/**
 * Welcome page HTML
 */
function getWelcomePage(): string {
  return `
<!DOCTYPE html>
<html>
<head>
    <title>🥾 b00t-mcp Cloud</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body { 
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            max-width: 800px; margin: 0 auto; padding: 40px 20px; 
            background: #0f0f0f; color: #e0e0e0;
        }
        .header { text-align: center; margin-bottom: 60px; }
        .logo { font-size: 4em; margin-bottom: 20px; }
        .subtitle { font-size: 1.2em; color: #a0a0a0; }
        .section { margin: 40px 0; }
        .code { 
            background: #1a1a1a; border: 1px solid #333; 
            padding: 15px; border-radius: 8px; 
            font-family: "Fira Code", Consolas, monospace;
            font-size: 0.9em;
        }
        .highlight { color: #4CAF50; }
        a { color: #2196F3; text-decoration: none; }
        a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <div class="header">
        <div class="logo">🥾</div>
        <h1>b00t-mcp Cloud</h1>
        <p class="subtitle">GitHub-authenticated MCP servers for Claude</p>
    </div>
    
    <div class="section">
        <h2>🚀 Get Started</h2>
        <p>Access your personal b00t-mcp instance:</p>
        <div class="code">
            https://<span class="highlight">{your-github-username}</span>.b00t.promptexecution.com
        </div>
        <p>Example: <a href="https://elasticdotventures.b00t.promptexecution.com">elasticdotventures.b00t.promptexecution.com</a></p>
    </div>
    
    <div class="section">
        <h2>🔐 Authentication</h2>
        <p>Each instance uses GitHub OAuth for secure authentication. Your GitHub username determines your subdomain and access permissions.</p>
    </div>
    
    <div class="section">
        <h2>⚡ Features</h2>
        <ul>
            <li>GitHub OAuth authentication</li>
            <li>Personal b00t-cli tool access</li>
            <li>MCP protocol compatibility</li>
            <li>Edge deployment on Cloudflare</li>
            <li>Per-user configuration and sessions</li>
        </ul>
    </div>
    
    <div class="section">
        <h2>🛠️ Integration</h2>
        <p>Configure Claude with your instance URL as a Custom Connector:</p>
        <div class="code">
MCP Server URL: https://<span class="highlight">{username}</span>.b00t.promptexecution.com/mcp<br>
OAuth Authorization: https://<span class="highlight">{username}</span>.b00t.promptexecution.com/oauth/authorize<br>
OAuth Token: https://<span class="highlight">{username}</span>.b00t.promptexecution.com/oauth/token
        </div>
    </div>
    
    <footer style="text-align: center; margin-top: 80px; color: #666; font-size: 0.9em;">
        <p>Powered by Cloudflare Workers • <a href="https://github.com/elasticdotventures/dotfiles">Source Code</a></p>
    </footer>
</body>
</html>`;
}

// Export the Durable Object class
export { B00tMcpInstance } from './durable-objects/b00t-mcp-instance';