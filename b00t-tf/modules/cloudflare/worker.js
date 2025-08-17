// b00t MCP Proxy Worker
export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    
    // CORS headers
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    };

    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    // Route: /mcp/providers - List AI providers
    if (url.pathname === '/mcp/providers') {
      const providers = [
        { id: 'anthropic', name: 'Anthropic Claude', enabled: true },
        { id: 'openrouter', name: 'OpenRouter', enabled: false },
        { id: 'openai', name: 'OpenAI', enabled: false },
      ];
      return Response.json(providers, { headers: corsHeaders });
    }

    // Route: /mcp/tools - List available tools
    if (url.pathname === '/mcp/tools') {
      const tools = [
        { id: 'filesystem', name: 'Filesystem', description: 'File operations' },
        { id: 'git', name: 'Git', description: 'Version control' },
        { id: 'terminal', name: 'Terminal', description: 'Shell commands' },
        { id: 'terraform', name: 'Terraform', description: 'Infrastructure provisioning' },
      ];
      return Response.json(tools, { headers: corsHeaders });
    }

    // Route: /mcp/generate - Generate .dxt file
    if (url.pathname === '/mcp/generate' && request.method === 'POST') {
      const config = await request.json();
      
      // Generate manifest.json
      const manifest = {
        dxt_version: '0.1',
        name: 'b00t-local-proxy',
        version: '1.0.0',
        description: 'Local MCP proxy for b00t infrastructure management',
        server: {
          type: 'node',
          entry_point: 'proxy-server.js'
        },
        user_config: {
          type: 'object',
          properties: {
            anthropic_api_key: {
              type: 'string',
              description: 'Anthropic API key',
              secret: true
            },
            proxy_port: {
              type: 'integer', 
              description: 'Local proxy port',
              default: config.local_proxy?.port || 8787
            }
          },
          required: ['anthropic_api_key']
        },
        tools: config.tools || []
      };

      return Response.json({ 
        manifest,
        download_url: '/mcp/download'
      }, { headers: corsHeaders });
    }

    return new Response('Not Found', { 
      status: 404, 
      headers: corsHeaders 
    });
  },
};